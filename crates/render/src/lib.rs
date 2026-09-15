#![forbid(unsafe_code)]
//! The render module: `Layout` and `Paint` joined into a command queue a
//! GPU backend executes. Crate docs are written in Task 6.

use app::prelude::*;
use geometry::{Color, Corners, Insets, Rect, Size};
use layout::Layout;
use paint::{AtlasId, AtlasTile, Paint};
use window::{Frame, InWindow, RequestFrame, Window};

mod rect;
mod scene;
mod walk;

use scene::Scene;

pub mod prelude {
    pub use crate::{Command, Pass, Queue, RenderModule, Scenes};
}

// ---------------------------------------------------------------------------
// The command
// ---------------------------------------------------------------------------

/// One thing a backend draws, for either pass, with every decision taken.
/// `#[repr(C)]`, every field an `f32` or a `u32`, no padding: a list of
/// these is uploaded as the bytes it is. Rects, radii and widths are
/// device pixels. Fields a kind does not use hold the values that make its
/// shader a no-op for them.
///
/// In the translucent pass the shader computes coverage from the rounded
/// edge, the border or the tile and blends the primitive's colour with
/// that coverage times its alpha. In the opaque pass it does the same
/// arithmetic, composites onto `background`, and writes alpha one and
/// depth: `rgb = mix(background, primitive, coverage * alpha)`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Command {
    pub rect: Rect,
    /// `2 * preorder index`; a quad interior takes the odd value above its
    /// node. Higher is nearer. An integer held in an `f32`.
    pub z: f32,
    /// The fill, the tint, or white for an image.
    pub color: Color,
    /// Equal to `color` when there is no border.
    pub border_color: Color,
    /// The solid behind, opaque list only; transparent otherwise.
    pub background: Color,
    /// Zero for a sprite.
    pub radii: Corners<f32>,
    /// Zero for a sprite or an image.
    pub border: Insets<f32>,
    /// Zero for a quad.
    pub tile: AtlasTile,
    /// Kind, grayscale, opaque and opacity packed; see the constants.
    pub flags: u32,
}

impl Command {
    /// Bits 0..2: what the primitive is.
    pub const KIND: u32 = 0b11;
    /// A box: rounded, bordered, or neither, in which case it is a fill.
    pub const QUAD: u32 = 0;
    /// A coverage tile, tinted.
    pub const SPRITE: u32 = 1;
    /// A colour tile stretched over the rect, with rounded corners.
    pub const IMAGE: u32 = 2;
    /// Images: desaturate the tile first.
    pub const GRAYSCALE: u32 = 1 << 2;
    /// Set on every command in the opaque list.
    pub const OPAQUE: u32 = 1 << 3;
    /// Bits 8..16: opacity, `0..=255`; images only, 255 elsewhere.
    pub const OPACITY_SHIFT: u32 = 8;

    /// The flags word for `kind`, the two bits and an opacity in
    /// `0.0..=1.0`, rounded to eight bits.
    pub fn pack(kind: u32, opaque: bool, grayscale: bool, opacity: f32) -> u32 {
        let opacity = (opacity.clamp(0.0, 1.0) * 255.0).round() as u32;
        (kind & Self::KIND)
            | if opaque { Self::OPAQUE } else { 0 }
            | if grayscale { Self::GRAYSCALE } else { 0 }
            | (opacity << Self::OPACITY_SHIFT)
    }

    pub fn kind(&self) -> u32 {
        self.flags & Self::KIND
    }

    pub fn is_opaque(&self) -> bool {
        self.flags & Self::OPAQUE != 0
    }

    pub fn is_grayscale(&self) -> bool {
        self.flags & Self::GRAYSCALE != 0
    }

    /// `0.0..=1.0`.
    pub fn opacity(&self) -> f32 {
        ((self.flags >> Self::OPACITY_SHIFT) & 0xff) as f32 / 255.0
    }
}

/// The tile a quad carries: nothing.
pub(crate) const NO_TILE: AtlasTile = AtlasTile {
    atlas: AtlasId(0),
    bounds: Rect::ZERO,
};

// ---------------------------------------------------------------------------
// The queue
// ---------------------------------------------------------------------------

/// One pass of a frame: the scissor rects it has something to draw in and
/// the commands that touch them, in pass order. A backend skips a pass
/// whose `scissor` is empty.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Pass {
    pub scissor: Vec<Rect>,
    pub commands: Vec<Command>,
}

/// What a backend runs for one frame on one buffer. Everything is device
/// pixels. `scissor` is what to clear, colour and depth, before the
/// passes; each pass's own scissor is the subset of it that pass touches.
/// An empty `scissor` means nothing to do.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Queue {
    /// The window, device pixels.
    pub size: Size,
    /// The scale that was applied.
    pub scale: f32,
    /// What the scissor is cleared to before the passes.
    pub clear: Color,
    /// One past the largest z in the scene: the divisor for the depth
    /// range.
    pub depth: f32,
    pub scissor: Vec<Rect>,
    /// Front to back: z descending.
    pub opaque: Pass,
    /// Back to front: z ascending.
    pub translucent: Pass,
}

// ---------------------------------------------------------------------------
// Private state
// ---------------------------------------------------------------------------

/// Previous output per node, not a copy of any input: the bounds of what
/// the node emitted at the last frame it was walked, `None` if nothing,
/// and whether a `Layout` or `Paint` change was noted since. Written by
/// the change systems and the walk; its `OnChanged` drain has no
/// listener, and `on_frame` takes the record after the walk so the drain
/// sees only what the change systems marked between frames.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct Drawn {
    pub(crate) rect: Option<Rect>,
    pub(crate) dirty: bool,
}

impl Component for Drawn {}

// ---------------------------------------------------------------------------
// The resource
// ---------------------------------------------------------------------------

/// One scene per live window. A backend's whole surface is [`Scenes::queue`].
#[derive(Debug)]
pub struct Scenes {
    buffers: usize,
    scenes: Vec<(NodeId, Scene)>,
}

impl Resource for Scenes {}

impl Scenes {
    /// Keeps `buffers` frames of damage per window.
    pub fn new(buffers: usize) -> Self {
        Self {
            buffers,
            scenes: Vec::new(),
        }
    }

    /// The commands a backend runs on `window` for a buffer of `age`: 1 if
    /// the buffer holds the previous frame, 2 the one before, and so on.
    /// An age of 0 or past the frames held means the whole window. `None`
    /// if `window` has no scene: not a window, or no `Frame` yet. Takes
    /// `&mut self` because the queue is a buffer the scene reuses.
    pub fn queue(&mut self, window: NodeId, age: usize) -> Option<&Queue> {
        self.scenes
            .iter_mut()
            .find(|(w, _)| *w == window)
            .map(|(_, s)| s.queue(age))
    }

    pub(crate) fn has(&self, window: NodeId) -> bool {
        self.scenes.iter().any(|(w, _)| *w == window)
    }

    pub(crate) fn scene_or_new(&mut self, window: NodeId) -> &mut Scene {
        let i = match self.scenes.iter().position(|(w, _)| *w == window) {
            Some(i) => i,
            None => {
                self.scenes.push((window, Scene::new(self.buffers)));
                self.scenes.len() - 1
            }
        };
        &mut self.scenes[i].1
    }

    pub(crate) fn drop_scene(&mut self, window: NodeId) {
        self.scenes.retain(|(w, _)| *w != window);
    }
}

// ---------------------------------------------------------------------------
// The module
// ---------------------------------------------------------------------------

/// Registers `Drawn`, inserts `Scenes`, and attaches the systems.
/// Installs after `LayoutModule`, `PaintModule` and `WindowModule`; a
/// backend installs after this, so on one `Frame` the scene is rebuilt
/// before the backend asks for its queue.
pub struct RenderModule {
    /// How many buffers a backend holds per window. That many frames of
    /// damage are kept, so a buffer of any age up to it gets an exact
    /// scissor. Default 2.
    pub buffers: usize,
}

impl Default for RenderModule {
    fn default() -> Self {
        Self { buffers: 2 }
    }
}

impl Module for RenderModule {
    fn install(self, app: &mut App) {
        app.register_component::<Drawn>();
        app.insert_resource(Scenes::new(self.buffers));
        app.system(on_layout_changed)
            .system(on_paint_changed)
            .system(on_removed)
            .system(on_frame);
    }
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

/// A `Layout` change: note the node and ask for its window.
fn on_layout_changed(app: &mut App, e: &Emitted<OnChanged<Layout>>) {
    note(app, &e.targets);
}

/// A `Paint` change: note the node and ask for its window.
fn on_paint_changed(app: &mut App, e: &Emitted<OnChanged<Paint>>) {
    note(app, &e.targets);
}

/// Mark each target dirty and raise one `RequestFrame` per distinct
/// window. A target outside every window is skipped. The `Drawn` write
/// flags the record; its drain emits an `OnChanged<Drawn>` nobody
/// handles, which costs the drain one push per marked node.
fn note(app: &mut App, targets: &[NodeId]) {
    let mut windows: Vec<NodeId> = Vec::new();
    for &id in targets {
        let Some(InWindow(Some(w))) = app.component::<InWindow>(id).copied() else {
            continue;
        };
        if let Some(mut d) = app.component_mut::<Drawn>(id)
            && !d.dirty
        {
            d.dirty = true;
        }
        if !windows.contains(&w) {
            windows.push(w);
        }
    }
    for w in windows {
        app.signal(RequestFrame(w));
    }
}

/// A removed window loses its scene; a removed subtree under a window
/// left pixels behind, so its window gets a frame even when no sibling
/// moved. A stale parent is skipped: its own `Removed` follows.
fn on_removed(app: &mut App, r: &Removed) {
    if r.parent == app.root() {
        if app.resource::<Scenes>().has(r.id) {
            app.resource_mut::<Scenes>().drop_scene(r.id);
            app.take_resource_changed::<Scenes>();
        }
        return;
    }
    if let Some(InWindow(Some(w))) = app.component::<InWindow>(r.parent).copied() {
        app.signal(RequestFrame(w));
    }
}

/// Rebuild the window's scene from the live `Layout` and `Paint`. A stale
/// id or a non-window is ignored. Nothing here raises `RequestFrame`.
fn on_frame(app: &mut App, f: &Frame) {
    let w = f.0;
    let Some(win) = app.widget::<Window>(w) else {
        return;
    };
    let (scale, clear) = (win.scale(), win.clear());
    let Some(layout) = app.component::<Layout>(w) else {
        return;
    };
    let size = walk::scale_rect(layout.rect, scale).size;

    let (tree, mut data) = app.split();
    let (layouts, paints, mut drawn, mut scenes) =
        data.query::<(&Layout, &Paint, &mut Drawn, ResMut<Scenes>)>();
    let scene = scenes.scene_or_new(w);
    let full = scene.begin(size, scale, clear);
    let visited = walk::walk(tree, &layouts, &paints, &mut drawn, w, scale, clear, scene);
    // A node drawn last frame and gone now left pixels behind; its
    // `Drawn` was reset by the core, so last frame's list is the record.
    for &(id, rect) in &scene.drawn {
        if !tree.is_live(id) {
            scene.damage.push(rect);
        }
    }
    scene.finish(full, visited);

    // The walk wrote `Drawn` through flagging guards and `Scenes` through
    // its guard. Nothing listens for either, so take the records now
    // rather than have the next `PostTick` drain walk them.
    app.take_changed::<Drawn>().for_each(drop);
    app.take_resource_changed::<Scenes>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_command_is_thirty_one_words_with_no_padding() {
        use std::mem::{align_of, size_of};
        assert_eq!(size_of::<Command>(), 124);
        assert_eq!(align_of::<Command>(), 4);
    }

    #[test]
    fn flags_round_trip() {
        let f = Command::pack(Command::IMAGE, true, true, 0.5);
        let c = Command {
            rect: Rect::ZERO,
            z: 0.0,
            color: Color::WHITE,
            border_color: Color::WHITE,
            background: Color::TRANSPARENT,
            radii: Corners::all(0.0),
            border: Insets::all(0.0),
            tile: NO_TILE,
            flags: f,
        };
        assert_eq!(c.kind(), Command::IMAGE);
        assert!(c.is_opaque());
        assert!(c.is_grayscale());
        assert!((c.opacity() - 0.5).abs() < 0.01);

        let plain = Command {
            flags: Command::pack(Command::SPRITE, false, false, 1.0),
            ..c
        };
        assert_eq!(plain.kind(), Command::SPRITE);
        assert!(!plain.is_opaque());
        assert!(!plain.is_grayscale());
        assert_eq!(plain.opacity(), 1.0);
        assert_eq!(
            Command {
                flags: Command::pack(Command::QUAD, true, false, 0.0),
                ..c
            }
            .opacity(),
            0.0
        );
    }

    /// `NodeId` has no public constructor; the root of a fresh app is a
    /// valid id.
    #[test]
    fn scenes_answer_only_for_windows_they_hold() {
        let app = App::new();
        let w = app.root();
        let mut scenes = Scenes::new(2);
        assert!(scenes.queue(w, 1).is_none());
        scenes.scene_or_new(w);
        assert!(scenes.has(w));
        assert!(scenes.queue(w, 1).is_some());
        scenes.drop_scene(w);
        assert!(!scenes.has(w));
        assert!(scenes.queue(w, 1).is_none());
    }
}

//! `Div`: a box with a look — the one container of the four primitive
//! widgets. `build` never spawns children; a caller attaches them to the
//! handle it gets back.

use app::{Build, Context, Handle, Spawner, Widget};
use geometry::{Color, Corners, Insets};
use layout::LayoutStyle;
use paint::{Paint, Quad};

pub struct Div;

pub fn div() -> DivBuilder {
    DivBuilder {
        style: LayoutStyle::default(),
        quad: Quad::default(),
    }
}

pub struct DivBuilder {
    style: LayoutStyle,
    quad: Quad,
}

impl DivBuilder {
    /// The whole style, no verb forwarding — the same rationale as
    /// `WindowBuilder::layout`: a caller who wants a column with a size
    /// writes `LayoutStyle::default().column().size(..)`.
    pub fn style(mut self, style: LayoutStyle) -> Self {
        self.style = style;
        self
    }
    pub fn background(mut self, color: Color) -> Self {
        self.quad.color = color;
        self
    }
    pub fn radius(mut self, radius: f32) -> Self {
        self.quad.radii = Corners::all(radius);
        self
    }
    pub fn radii(mut self, radii: Corners<f32>) -> Self {
        self.quad.radii = radii;
        self
    }
    pub fn border(mut self, width: f32, color: Color) -> Self {
        self.quad.border = Insets::all(width);
        self.quad.border_color = color;
        self
    }
}

impl Build for DivBuilder {
    type Widget = Div;
}

impl Widget for Div {
    type Builder = DivBuilder;
    fn build(b: DivBuilder, me: Handle<Div>, s: &mut Spawner<'_, Div>) -> Div {
        *s.component_mut::<LayoutStyle>(me).unwrap() = b.style;
        *s.component_mut::<Paint>(me).unwrap() = quad_paint(b.quad);
        Div
    }
}

/// `Paint::None` for an invisible quad, so a bare `div()` paints nothing
/// rather than a zero-radius transparent fill nobody asked for.
fn quad_paint(quad: Quad) -> Paint {
    if quad.is_invisible() {
        Paint::None
    } else {
        Paint::Quad(quad)
    }
}

/// The `Quad` `paint` holds, or `Quad::default()` for `Paint::None` or
/// any other variant — a `Div` whose `Paint` was overwritten by hand
/// between calls loses that write the next time a `DivContext` setter
/// runs, same as any other "last write wins" component.
fn quad_of(paint: &Paint) -> Quad {
    match paint {
        Paint::Quad(q) => *q,
        _ => Quad::default(),
    }
}

use paint::PaintContext;

pub trait DivContext {
    fn set_background(&mut self, color: Color);
    fn set_radius(&mut self, radius: f32);
    fn set_radii(&mut self, radii: Corners<f32>);
    fn set_border(&mut self, width: f32, color: Color);
}

impl DivContext for Context<'_, Div> {
    fn set_background(&mut self, color: Color) {
        let mut quad = quad_of(self.paint());
        quad.color = color;
        self.set_paint(quad_paint(quad));
    }
    fn set_radius(&mut self, radius: f32) {
        let mut quad = quad_of(self.paint());
        quad.radii = Corners::all(radius);
        self.set_paint(quad_paint(quad));
    }
    fn set_radii(&mut self, radii: Corners<f32>) {
        let mut quad = quad_of(self.paint());
        quad.radii = radii;
        self.set_paint(quad_paint(quad));
    }
    fn set_border(&mut self, width: f32, color: Color) {
        let mut quad = quad_of(self.paint());
        quad.border = Insets::all(width);
        quad.border_color = color;
        self.set_paint(quad_paint(quad));
    }
}

#[cfg(test)]
mod tests {
    use geometry::{Color, Corners, Insets};
    use layout::LayoutStyle;
    use paint::Quad;

    use super::*;

    #[test]
    fn builder_verbs_set_the_right_fields() {
        let b = div()
            .background(Color::BLACK)
            .radius(4.0)
            .border(2.0, Color::WHITE);
        assert_eq!(b.quad.color, Color::BLACK);
        assert_eq!(b.quad.radii, Corners::all(4.0));
        assert_eq!(b.quad.border, Insets::all(2.0));
        assert_eq!(b.quad.border_color, Color::WHITE);
    }

    #[test]
    fn radii_verb_sets_per_corner_values() {
        let b = div().radii(Corners::new(1.0, 2.0, 3.0, 4.0));
        assert_eq!(b.quad.radii, Corners::new(1.0, 2.0, 3.0, 4.0));
    }

    #[test]
    fn style_verb_replaces_the_whole_style() {
        let style = LayoutStyle::default().column();
        let b = div().style(style.clone());
        assert_eq!(b.style, style);
    }

    #[test]
    fn quad_paint_is_none_for_an_invisible_quad() {
        assert_eq!(quad_paint(Quad::default()), Paint::None);
        assert_eq!(
            quad_paint(Quad::new(Color::BLACK)),
            Paint::Quad(Quad::new(Color::BLACK))
        );
    }

    #[test]
    fn quad_of_defaults_for_none_and_reads_back_a_quad() {
        assert_eq!(quad_of(&Paint::None), Quad::default());
        let q = Quad::new(Color::WHITE).radius(3.0);
        assert_eq!(quad_of(&Paint::Quad(q)), q);
    }
}

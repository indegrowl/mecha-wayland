#![deny(unsafe_code)]
//! The GLES 3.0 backend: one device, the atlas resident on it, dmabuf
//! render targets, and the draw of a `render::Queue`. Crate docs are
//! completed in the last task of the slice.

mod draw;
mod egl;
mod program;
mod target;
mod textures;

pub use target::{Plane, Target, XRGB8888};

pub mod prelude {
    pub use crate::{Budget, Device, Error, Plane, Target, XRGB8888};
}

/// How many atlas pages of each kind the device allocates room for, once,
/// at open. Pages are 1024 square; the mono array costs about 1.3 MB a
/// layer with its mips, the color array about 5.6 MB.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget {
    /// Glyph and icon pages together. Default 4.
    pub mono_pages: u32,
    /// Image pages. Default 2.
    pub color_pages: u32,
}

impl Default for Budget {
    fn default() -> Self {
        Self {
            mono_pages: 4,
            color_pages: 2,
        }
    }
}

/// Why the device did not open.
#[derive(Debug)]
pub enum Error {
    /// No `/dev/dri/renderD*` could be opened.
    NoDevice,
    /// A GBM, EGL or GL step failed; the string names it.
    Setup(String),
}

/// The GPU: one render node, one GBM device, one EGL display and one
/// GLES 3.0 context, current on the thread that opened it for the
/// device's life. Not `Send`.
///
/// `gpu` is declared last so it drops last: [`Drop for Device`](#impl-Drop-for-Device)
/// frees the atlas textures and the program's GL objects first, while
/// the context is still current.
pub struct Device {
    budget: Budget,
    program: program::Program,
    textures: textures::Textures,
    gpu: egl::Gpu,
}

impl Device {
    /// Opens the GPU or says why it cannot. Tests call this and skip on
    /// [`Error::NoDevice`].
    pub fn try_open(budget: Budget) -> Result<Device, Error> {
        let gpu = egl::Gpu::open()?;
        let program = program::Program::new(&gpu);
        let textures = textures::Textures::new(&gpu, budget);
        Ok(Device {
            gpu,
            budget,
            program,
            textures,
        })
    }

    /// [`Device::try_open`] or a panic naming the step that failed.
    pub fn open(budget: Budget) -> Device {
        match Self::try_open(budget) {
            Ok(d) => d,
            Err(Error::NoDevice) => panic!("gles: no render node in /dev/dri; a GPU is required"),
            Err(Error::Setup(step)) => panic!("gles: {step}"),
        }
    }

    /// The `GL_VERSION` string, `OpenGL ES 3.x ...`.
    pub fn version(&self) -> &str {
        &self.gpu.version
    }

    /// The budget this device was opened with.
    pub fn budget(&self) -> Budget {
        self.budget
    }

    /// A render target of `width` by `height` device pixels, XRGB8888,
    /// with the first layout of `modifiers` GBM can produce, or linear
    /// when the list is empty or refused.
    pub fn target(&mut self, width: u32, height: u32, modifiers: &[u64]) -> Target {
        target::create(&self.gpu, width, height, modifiers)
    }

    /// Frees the target's GL objects, EGLImage and buffer.
    pub fn destroy(&mut self, target: Target) {
        target::destroy(&self.gpu, target)
    }

    /// The target's pixels as RGBA8, rows top-down. For tests.
    pub fn read(&mut self, target: &Target) -> Vec<u8> {
        target::read(&self.gpu, target)
    }

    /// Uploads what changed in the atlas: new pages get a layer, dirty
    /// cells go up as runs. Takes `&Atlas`, so the drain is not a write.
    pub fn upload(&mut self, atlas: &atlas::Atlas) {
        self.textures.upload(&self.gpu, &self.program, atlas);
    }

    /// How many sub-image uploads `upload` has issued so far. For tests.
    pub fn upload_calls(&self) -> u64 {
        self.textures.calls
    }

    /// Executes one queue on one target: the clears, the opaque pass,
    /// the translucent pass, a flush. An empty scissor does nothing.
    pub fn draw(&mut self, target: &Target, queue: &render::Queue) {
        if queue.scissor.is_empty() {
            return;
        }
        #[cfg(debug_assertions)]
        for c in q_commands(queue) {
            if c.kind() != render::Command::QUAD {
                debug_assert!(
                    self.textures.has(c.tile.atlas),
                    "gles: atlas page {:?} is sampled before it was uploaded",
                    c.tile.atlas
                );
            }
        }
        draw::clear(&self.gpu, target, queue);
        draw::passes(&self.gpu, &self.program, target, queue);
        draw::finish(&self.gpu);
    }
}

#[cfg(debug_assertions)]
fn q_commands(q: &render::Queue) -> impl Iterator<Item = &render::Command> {
    q.opaque
        .commands
        .iter()
        .chain(q.translucent.commands.iter())
}

impl Drop for Device {
    fn drop(&mut self) {
        self.textures.drop_with(&self.gpu);
        self.program.drop_with(&self.gpu);
    }
}

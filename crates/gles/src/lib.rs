#![deny(unsafe_code)]
//! The GLES 3.0 backend: one device, the atlas resident on it, dmabuf
//! render targets, and the draw of a `render::Queue`. Crate docs are
//! completed in the last task of the slice.

mod egl;
mod target;

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
pub struct Device {
    gpu: egl::Gpu,
    budget: Budget,
}

impl Device {
    /// Opens the GPU or says why it cannot. Tests call this and skip on
    /// [`Error::NoDevice`].
    pub fn try_open(budget: Budget) -> Result<Device, Error> {
        let gpu = egl::Gpu::open()?;
        Ok(Device { gpu, budget })
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

    /// Executes one queue on one target: the clears, the opaque pass,
    /// the translucent pass, a flush. An empty scissor does nothing.
    pub fn draw(&mut self, target: &Target, queue: &render::Queue) {
        if queue.scissor.is_empty() {
            return;
        }
        draw::clear(&self.gpu, target, queue);
        draw::finish(&self.gpu);
    }
}

mod draw {
    #![allow(unsafe_code)]
    use geometry::Rect;
    use glow::HasContext;

    use crate::egl::Gpu;
    use crate::target::Target;

    /// A device-pixel rect rounded outward to whole pixels, as GL scissor
    /// arguments with y flipped: `(x, y, w, h)`.
    pub(crate) fn scissor(r: Rect, height: u32) -> (i32, i32, i32, i32) {
        let x0 = r.x().floor() as i32;
        let y0 = r.y().floor() as i32;
        let x1 = r.right().ceil() as i32;
        let y1 = r.bottom().ceil() as i32;
        (x0, height as i32 - y1, x1 - x0, y1 - y0)
    }

    pub(crate) fn clear(gpu: &Gpu, t: &Target, q: &render::Queue) {
        let gl = &gpu.gl;
        // SAFETY: the context is current and the FBO is this target's.
        unsafe {
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(t.fbo));
            gl.viewport(0, 0, t.width() as i32, t.height() as i32);
            gl.enable(glow::SCISSOR_TEST);
            gl.depth_mask(true);
            gl.clear_color(q.clear.r, q.clear.g, q.clear.b, 1.0);
            gl.clear_depth_f32(1.0);
            for &r in &q.scissor {
                let (x, y, w, h) = scissor(r, t.height());
                gl.scissor(x, y, w, h);
                gl.clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);
            }
        }
    }

    pub(crate) fn finish(gpu: &Gpu) {
        // SAFETY: the context is current.
        unsafe { gpu.gl.flush() }
    }
}

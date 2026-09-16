#![deny(unsafe_code)]
//! The GLES 3.0 backend: one device, the atlas resident on it, dmabuf
//! render targets, and the draw of a `render::Queue`. Crate docs are
//! completed in the last task of the slice.

mod egl;

pub mod prelude {
    pub use crate::{Budget, Device, Error};
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
}

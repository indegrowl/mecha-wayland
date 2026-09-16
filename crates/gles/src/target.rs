//! One dmabuf a frame is drawn into: a GBM buffer with a negotiated
//! layout, its EGLImage, a colour renderbuffer over it, a depth
//! renderbuffer, one FBO. Made and freed through the device.
#![allow(unsafe_code)]

use std::ffi::c_void;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};

use gbm::{AsRaw, BufferObjectFlags, Format, Modifier};
use glow::HasContext;
use khronos_egl as egl;

use crate::egl::Gpu;

/// `DRM_FORMAT_XRGB8888`: every target's format.
pub const XRGB8888: u32 = 0x3432_5258;

/// `EGL_NATIVE_PIXMAP_KHR`: how a `gbm_bo` is handed to `eglCreateImage`.
const NATIVE_PIXMAP: egl::Enum = 0x30B0;

/// One plane of a target's dmabuf, what `zwp_linux_buffer_params_v1.add`
/// takes.
#[derive(Debug, Clone, Copy)]
pub struct Plane<'a> {
    pub fd: BorrowedFd<'a>,
    pub offset: u32,
    pub stride: u32,
}

struct PlaneData {
    fd: OwnedFd,
    offset: u32,
    stride: u32,
}

/// A render target. There is no `Drop`: free it with [`crate::Device::destroy`].
pub struct Target {
    width: u32,
    height: u32,
    modifier: u64,
    planes: Vec<PlaneData>,
    pub(crate) fbo: glow::Framebuffer,
    color: glow::Renderbuffer,
    depth: glow::Renderbuffer,
    image: egl::Image,
    /// Must outlive `image`; dropped last by declaration order.
    _bo: gbm::BufferObject<()>,
}

impl Target {
    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }
    /// [`XRGB8888`].
    pub fn fourcc(&self) -> u32 {
        XRGB8888
    }
    /// The layout GBM chose, `0` for linear.
    pub fn modifier(&self) -> u64 {
        self.modifier
    }
    /// One to four planes, in plane order.
    pub fn planes(&self) -> impl Iterator<Item = Plane<'_>> {
        self.planes.iter().map(|p| Plane {
            fd: p.fd.as_fd(),
            offset: p.offset,
            stride: p.stride,
        })
    }
}

fn buffer(gpu: &Gpu, width: u32, height: u32, modifiers: &[u64]) -> gbm::BufferObject<()> {
    let linear = || {
        gpu.gbm
            .create_buffer_object::<()>(
                width,
                height,
                Format::Xrgb8888,
                BufferObjectFlags::RENDERING | BufferObjectFlags::LINEAR,
            )
            .unwrap_or_else(|e| panic!("gles: gbm_bo_create({width}x{height}, linear): {e}"))
    };
    if modifiers.is_empty() {
        return linear();
    }
    gpu.gbm
        .create_buffer_object_with_modifiers2::<()>(
            width,
            height,
            Format::Xrgb8888,
            modifiers.iter().map(|&m| Modifier::from(m)),
            BufferObjectFlags::RENDERING,
        )
        .unwrap_or_else(|_| linear())
}

pub(crate) fn create(gpu: &Gpu, width: u32, height: u32, modifiers: &[u64]) -> Target {
    let bo = buffer(gpu, width, height, modifiers);
    let modifier = u64::from(bo.modifier());
    let planes = (0..bo.plane_count() as i32)
        .map(|i| PlaneData {
            fd: bo
                .fd_for_plane(i)
                .unwrap_or_else(|e| panic!("gles: gbm_bo_get_fd_for_plane({i}): {e:?}")),
            offset: bo.offset(i),
            stride: bo.stride_for_plane(i),
        })
        .collect();
    // SAFETY: NO_CONTEXT and a live gbm_bo pointer, as EGL_KHR_image_pixmap
    // over the GBM platform expects.
    let image = unsafe {
        gpu.egl.create_image(
            gpu.display,
            egl::Context::from_ptr(egl::NO_CONTEXT),
            NATIVE_PIXMAP,
            egl::ClientBuffer::from_ptr(bo.as_raw() as *mut c_void),
            &[egl::ATTRIB_NONE],
        )
    }
    .unwrap_or_else(|e| panic!("gles: eglCreateImage(gbm_bo): {e:?}"));
    let gl = &gpu.gl;
    // SAFETY: the context is current; the image is valid until destroyed.
    let (fbo, color, depth) = unsafe {
        let color = gl.create_renderbuffer().expect("glGenRenderbuffers");
        gl.bind_renderbuffer(glow::RENDERBUFFER, Some(color));
        (gpu.image_target_renderbuffer)(glow::RENDERBUFFER, image.as_ptr());
        let depth = gl.create_renderbuffer().expect("glGenRenderbuffers");
        gl.bind_renderbuffer(glow::RENDERBUFFER, Some(depth));
        gl.renderbuffer_storage(
            glow::RENDERBUFFER,
            glow::DEPTH_COMPONENT16,
            width as i32,
            height as i32,
        );
        let fbo = gl.create_framebuffer().expect("glGenFramebuffers");
        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));
        gl.framebuffer_renderbuffer(
            glow::FRAMEBUFFER,
            glow::COLOR_ATTACHMENT0,
            glow::RENDERBUFFER,
            Some(color),
        );
        gl.framebuffer_renderbuffer(
            glow::FRAMEBUFFER,
            glow::DEPTH_ATTACHMENT,
            glow::RENDERBUFFER,
            Some(depth),
        );
        let status = gl.check_framebuffer_status(glow::FRAMEBUFFER);
        assert!(
            status == glow::FRAMEBUFFER_COMPLETE,
            "gles: the target's framebuffer is incomplete: {status:#x}"
        );
        (fbo, color, depth)
    };
    Target {
        width,
        height,
        modifier,
        planes,
        fbo,
        color,
        depth,
        image,
        _bo: bo,
    }
}

pub(crate) fn destroy(gpu: &Gpu, t: Target) {
    let gl = &gpu.gl;
    // SAFETY: objects this module made on the current context.
    unsafe {
        gl.bind_framebuffer(glow::FRAMEBUFFER, None);
        gl.delete_framebuffer(t.fbo);
        gl.delete_renderbuffer(t.color);
        gl.delete_renderbuffer(t.depth);
    }
    let _ = gpu.egl.destroy_image(gpu.display, t.image);
    // `t.planes` and `t._bo` drop here, fds and buffer last.
}

/// Top-down RGBA8 rows of the whole target.
pub(crate) fn read(gpu: &Gpu, t: &Target) -> Vec<u8> {
    let (w, h) = (t.width as usize, t.height as usize);
    let mut bottom_up = vec![0u8; w * h * 4];
    let gl = &gpu.gl;
    // SAFETY: the FBO is this target's; the slice is sized for it.
    unsafe {
        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(t.fbo));
        gl.pixel_store_i32(glow::PACK_ALIGNMENT, 1);
        gl.read_pixels(
            0,
            0,
            t.width as i32,
            t.height as i32,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            glow::PixelPackData::Slice(Some(&mut bottom_up)),
        );
    }
    let row = w * 4;
    let mut out = Vec::with_capacity(bottom_up.len());
    for y in (0..h).rev() {
        out.extend_from_slice(&bottom_up[y * row..(y + 1) * row]);
    }
    out
}

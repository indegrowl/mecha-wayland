//! Two `wl_shm` buffers per window over one unlinked file, and the fill.
//! No `mmap`: rows are written with `write_all_at`, so the crate stays
//! free of unsafe. The copy through the kernel per frame is v0's cost
//! and goes with the fill when the backend arrives.

use std::fs::File;
use std::os::fd::AsFd;
use std::os::unix::fs::FileExt;
use std::sync::atomic::{AtomicU32, Ordering};

use geometry::Color;
use wayland::prelude::*;

pub(crate) struct Slot {
    pub(crate) buffer: WlBuffer,
    offset: u64,
    /// Attached and not yet released by the compositor.
    pub(crate) held: bool,
}

pub(crate) struct Buffers {
    pool: WlShmPool,
    file: File,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) slots: [Slot; 2],
}

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A file no path names, in `XDG_RUNTIME_DIR` or the temp dir.
fn anonymous_file(size: u64) -> File {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let path = dir.join(format!(
        "mecha-shm-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let file = File::options()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap_or_else(|e| panic!("presentation: cannot create {}: {e}", path.display()));
    std::fs::remove_file(&path).expect("unlink the shm file");
    file.set_len(size).expect("size the shm file");
    file
}

/// `XRGB8888`: alpha ignored, so the window is opaque as it promises.
fn pack(c: Color) -> u32 {
    let ch = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
    0xff00_0000 | (ch(c.r) << 16) | (ch(c.g) << 8) | ch(c.b)
}

impl Buffers {
    /// A pool of two `width` by `height` buffers in device pixels.
    pub(crate) fn create(wl: &mut Wayland, shm: WlShm, width: u32, height: u32) -> Buffers {
        let stride = width * 4;
        let one = (stride * height) as u64;
        let file = anonymous_file(one * 2);
        let pool = shm.create_pool(wl, file.as_fd(), (one * 2) as i32);
        let mut slot = |i: u64| Slot {
            buffer: pool.create_buffer(
                wl,
                (i * one) as i32,
                width as i32,
                height as i32,
                stride as i32,
                WlShmFormat::Xrgb8888,
            ),
            offset: i * one,
            held: false,
        };
        let slots = [slot(0), slot(1)];
        Buffers {
            pool,
            file,
            width,
            height,
            slots,
        }
    }

    pub(crate) fn free_slot(&self) -> Option<usize> {
        self.slots.iter().position(|s| !s.held)
    }

    pub(crate) fn fill(&self, slot: usize, color: Color) {
        let row: Vec<u8> = pack(color).to_ne_bytes().repeat(self.width as usize);
        let base = self.slots[slot].offset;
        let stride = (self.width * 4) as u64;
        for y in 0..self.height as u64 {
            self.file
                .write_all_at(&row, base + y * stride)
                .expect("fill the shm buffer");
        }
    }

    pub(crate) fn pixel(&self, slot: usize, x: u32, y: u32) -> Option<u32> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let at = self.slots[slot].offset + (y * self.width + x) as u64 * 4;
        let mut px = [0u8; 4];
        self.file.read_exact_at(&mut px, at).ok()?;
        Some(u32::from_ne_bytes(px))
    }

    pub(crate) fn destroy(self, wl: &mut Wayland) {
        for s in &self.slots {
            s.buffer.destroy(wl);
        }
        self.pool.destroy(wl);
    }
}

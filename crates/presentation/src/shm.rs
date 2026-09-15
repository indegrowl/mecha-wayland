//! Two `wl_shm` buffers per window over one unlinked file, and the fill.
//! No `mmap`: the buffer is written with `write_all_at`, so the crate
//! stays free of unsafe. The copy through the kernel per frame is v0's
//! cost and goes with the fill when the backend arrives.
//!
//! The fill is written in chunks of whole rows rather than one write per
//! row, and a slot that already holds the colour asked for is left alone:
//! on the target (a weak core over a slow bus) a per-row `pwrite` of a
//! solid colour byte-identical to the last one is the frame's whole cost.

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
    /// The colour [`Buffers::fill`] last wrote over the whole slot, if
    /// nothing has written it since. `None` for a slot the file's zeroes
    /// still show through, which no colour packs to.
    filled: Option<Color>,
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

/// The largest temporary [`Buffers::fill`] allocates: a wide surface is
/// written in several chunks rather than one buffer the size of a frame.
const CHUNK: usize = 64 * 1024;

/// `XRGB8888`: a DRM fourcc, so little-endian whatever the host is.
/// Alpha ignored, so the window is opaque as it promises.
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
            // A new file is zeroes, which is no colour's packing.
            filled: None,
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

    /// Paint the whole slot `color`, unless it already holds it.
    pub(crate) fn fill(&mut self, slot: usize, color: Color) {
        if self.slots[slot].filled == Some(color) {
            return;
        }
        let stride = (self.width * 4) as usize;
        let height = self.height as usize;
        let rows = (CHUNK / stride.max(1)).clamp(1, height.max(1));
        let chunk: Vec<u8> = pack(color).to_le_bytes().repeat(self.width as usize * rows);
        let base = self.slots[slot].offset;
        let mut y = 0;
        while y < height {
            let n = rows.min(height - y);
            self.file
                .write_all_at(&chunk[..n * stride], base + (y * stride) as u64)
                .expect("fill the shm buffer");
            y += n;
        }
        self.slots[slot].filled = Some(color);
    }

    pub(crate) fn pixel(&self, slot: usize, x: u32, y: u32) -> Option<u32> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let at = self.slots[slot].offset + (y * self.width + x) as u64 * 4;
        let mut px = [0u8; 4];
        self.file.read_exact_at(&mut px, at).ok()?;
        Some(u32::from_le_bytes(px))
    }

    pub(crate) fn destroy(self, wl: &mut Wayland) {
        for s in &self.slots {
            s.buffer.destroy(wl);
        }
        self.pool.destroy(wl);
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::net::UnixStream;

    use super::*;

    const RED: u32 = 0xffff_0000;
    const BLUE: u32 = 0xff00_00ff;

    /// Buffers over a connection nothing ever sends: `create`'s requests
    /// only buffer, and the file is what the test is after.
    fn buffers(width: u32, height: u32) -> Buffers {
        let (ours, _theirs) = UnixStream::pair().expect("socketpair");
        let mut wl = Wayland::over(ours);
        let shm: WlShm = wl.alloc(1);
        Buffers::create(&mut wl, shm, width, height)
    }

    #[test]
    fn a_fill_writes_the_whole_slot_and_leaves_the_other_alone() {
        let mut b = buffers(7, 5);
        b.fill(0, Color::rgb(1.0, 0.0, 0.0));
        assert_eq!(b.pixel(0, 0, 0), Some(RED));
        assert_eq!(b.pixel(0, 6, 4), Some(RED));
        assert_eq!(b.pixel(0, 3, 2), Some(RED));
        assert_eq!(b.pixel(1, 0, 0), Some(0), "the other slot is untouched");
        assert_eq!(b.pixel(0, 7, 0), None, "outside");
    }

    #[test]
    fn a_fill_taller_than_one_chunk_covers_every_row() {
        // 37 * 4 = 148 bytes a row, so a 64 KiB chunk is 442 rows and the
        // last of three is partial.
        let mut b = buffers(37, 1000);
        b.fill(1, Color::rgb(0.0, 0.0, 1.0));
        for y in [0, 441, 442, 883, 884, 999] {
            assert_eq!(b.pixel(1, 0, y), Some(BLUE), "row {y}");
            assert_eq!(b.pixel(1, 36, y), Some(BLUE), "row {y}");
        }
        assert_eq!(b.pixel(0, 0, 0), Some(0), "the other slot is untouched");
    }

    #[test]
    fn the_same_colour_again_is_not_written_and_a_change_rewrites() {
        let mut b = buffers(4, 4);
        let red = Color::rgb(1.0, 0.0, 0.0);
        b.fill(0, red);
        // Scribbled on behind `fill`'s back: a skipped fill leaves it, a
        // real one writes over it.
        b.file
            .write_all_at(&0x1234_5678u32.to_le_bytes(), 0)
            .unwrap();
        b.fill(0, red);
        assert_eq!(
            b.pixel(0, 0, 0),
            Some(0x1234_5678),
            "the same colour is not written again"
        );
        b.fill(0, Color::rgb(0.0, 0.0, 1.0));
        assert_eq!(b.pixel(0, 0, 0), Some(BLUE));
        assert_eq!(b.pixel(0, 3, 3), Some(BLUE));
        b.fill(0, red);
        assert_eq!(b.pixel(0, 0, 0), Some(RED), "and back again");
    }
}

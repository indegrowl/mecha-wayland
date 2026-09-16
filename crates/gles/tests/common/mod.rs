//! Shared by the pixel tests: the device behind a lock, PNG output for
//! review, and pixel access.
#![allow(dead_code)]

use std::sync::{Mutex, MutexGuard};

use gles::{Budget, Device, Error};

pub static LOCK: Mutex<()> = Mutex::new(());

/// The device and the lock that serialises GPU tests, or `None` with a
/// skip line when there is no render node.
pub fn device() -> Option<(MutexGuard<'static, ()>, Device)> {
    let guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    match Device::try_open(Budget::default()) {
        Ok(d) => Some((guard, d)),
        Err(Error::NoDevice) => {
            eprintln!("skip: no render node");
            None
        }
        Err(e) => panic!("the device did not open: {e:?}"),
    }
}

/// Writes `$CARGO_TARGET_TMPDIR/<name>.png` from top-down RGBA8 rows.
pub fn png(name: &str, width: u32, height: u32, rgba: &[u8]) {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    std::fs::create_dir_all(&dir).unwrap();
    let file = std::fs::File::create(dir.join(format!("{name}.png"))).unwrap();
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().unwrap().write_image_data(rgba).unwrap();
}

/// The RGBA of pixel (x, y) in a top-down buffer of `width`.
pub fn pixel(rgba: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * width + x) * 4) as usize;
    [rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]]
}

/// True when every channel of `a` is within `tol` of `b`.
pub fn near(a: [u8; 4], b: [u8; 4], tol: u8) -> bool {
    a.iter().zip(b).all(|(x, y)| x.abs_diff(y) <= tol)
}

/// A colour as the bytes `read` returns for it, alpha forced to 255 as
/// an XRGB target reads back.
pub fn rgb(r: f32, g: f32, b: f32) -> [u8; 4] {
    let c = |v: f32| (v * 255.0).round() as u8;
    [c(r), c(g), c(b), 255]
}

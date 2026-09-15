//! The wire format: an eight-byte header, sender id then size and
//! opcode, followed by arguments in native-endian 32-bit words. Public so
//! a test can script a compositor with the same encoder and decoder.

use std::os::fd::{AsFd, BorrowedFd, OwnedFd};

use crate::ObjectId;

pub const HEADER: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    pub sender: ObjectId,
    pub opcode: u16,
    /// The whole message, header included.
    pub size: usize,
}

/// The header at the start of `bytes`, or `None` if fewer than eight.
pub fn header(bytes: &[u8]) -> Option<Header> {
    if bytes.len() < HEADER {
        return None;
    }
    let sender = u32::from_ne_bytes(bytes[0..4].try_into().unwrap());
    let word = u32::from_ne_bytes(bytes[4..8].try_into().unwrap());
    Some(Header {
        sender: ObjectId(sender),
        opcode: (word & 0xffff) as u16,
        size: (word >> 16) as usize,
    })
}

/// Appends one message to `buf`; the size word is written on drop. An
/// `fd` argument is dup'd into `fds`, in argument order.
pub struct Writer<'a> {
    buf: &'a mut Vec<u8>,
    fds: &'a mut Vec<OwnedFd>,
    start: usize,
}

impl<'a> Writer<'a> {
    pub fn begin(
        buf: &'a mut Vec<u8>,
        fds: &'a mut Vec<OwnedFd>,
        sender: ObjectId,
        opcode: u16,
    ) -> Self {
        let start = buf.len();
        buf.extend_from_slice(&sender.0.to_ne_bytes());
        buf.extend_from_slice(&(opcode as u32).to_ne_bytes());
        Writer { buf, fds, start }
    }

    pub fn uint(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_ne_bytes());
    }
    pub fn int(&mut self, v: i32) {
        self.uint(v as u32);
    }
    /// 24.8 fixed point.
    pub fn fixed(&mut self, v: f32) {
        self.int((v * 256.0).round() as i32);
    }
    pub fn object(&mut self, id: ObjectId) {
        self.uint(id.0);
    }
    pub fn object_opt(&mut self, id: Option<ObjectId>) {
        self.uint(id.map_or(0, |i| i.0));
    }
    pub fn new_id(&mut self, id: ObjectId) {
        self.uint(id.0);
    }
    /// Length including the nul, the bytes, the nul, padding to a word.
    pub fn string(&mut self, s: &str) {
        let len = s.len() + 1;
        self.uint(len as u32);
        self.buf.extend_from_slice(s.as_bytes());
        self.buf.push(0);
        self.pad(len);
    }
    pub fn string_opt(&mut self, s: Option<&str>) {
        match s {
            Some(s) => self.string(s),
            None => self.uint(0),
        }
    }
    pub fn array(&mut self, a: &[u8]) {
        self.uint(a.len() as u32);
        self.buf.extend_from_slice(a);
        self.pad(a.len());
    }
    /// Dup'd now, so the caller keeps its own; sent as `SCM_RIGHTS`.
    pub fn fd(&mut self, fd: BorrowedFd<'_>) {
        self.fds
            .push(fd.as_fd().try_clone_to_owned().expect("dup fd"));
    }
    fn pad(&mut self, len: usize) {
        for _ in 0..(4 - len % 4) % 4 {
            self.buf.push(0);
        }
    }
}

impl Drop for Writer<'_> {
    fn drop(&mut self) {
        let size = (self.buf.len() - self.start) as u32;
        let at = self.start + 4..self.start + 8;
        let opcode = u32::from_ne_bytes(self.buf[at.clone()].try_into().unwrap()) & 0xffff;
        self.buf[at].copy_from_slice(&((size << 16) | opcode).to_ne_bytes());
    }
}

/// Reads arguments from a message body. Every read is `None` past the
/// end or on malformed data, so a decoder is `?` all the way down.
pub struct Reader<'a> {
    data: &'a [u8],
    o: usize,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Reader { data, o: 0 }
    }
    pub fn uint(&mut self) -> Option<u32> {
        let v = self.data.get(self.o..self.o + 4)?;
        self.o += 4;
        Some(u32::from_ne_bytes(v.try_into().unwrap()))
    }
    pub fn int(&mut self) -> Option<i32> {
        self.uint().map(|v| v as i32)
    }
    pub fn fixed(&mut self) -> Option<f32> {
        self.int().map(|v| v as f32 / 256.0)
    }
    pub fn object(&mut self) -> Option<ObjectId> {
        self.uint().map(ObjectId)
    }
    pub fn object_opt(&mut self) -> Option<Option<ObjectId>> {
        self.uint().map(|v| (v != 0).then_some(ObjectId(v)))
    }
    fn bytes(&mut self, len: usize) -> Option<&'a [u8]> {
        let padded = (len + 3) & !3;
        let raw = self.data.get(self.o..self.o + padded)?;
        self.o += padded;
        Some(&raw[..len])
    }
    pub fn string(&mut self) -> Option<String> {
        let len = self.uint()? as usize;
        let raw = self.bytes(len)?;
        let s = std::str::from_utf8(raw.get(..len.checked_sub(1)?)?).ok()?;
        Some(s.to_owned())
    }
    pub fn string_opt(&mut self) -> Option<Option<String>> {
        let len = self.uint()? as usize;
        if len == 0 {
            return Some(None);
        }
        let raw = self.bytes(len)?;
        let s = std::str::from_utf8(raw.get(..len - 1)?).ok()?;
        Some(Some(s.to_owned()))
    }
    pub fn array(&mut self) -> Option<Vec<u8>> {
        let len = self.uint()? as usize;
        Some(self.bytes(len)?.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(f: impl FnOnce(&mut Writer<'_>)) -> (Vec<u8>, Vec<OwnedFd>) {
        let mut buf = Vec::new();
        let mut fds = Vec::new();
        f(&mut Writer::begin(&mut buf, &mut fds, ObjectId(7), 3));
        (buf, fds)
    }

    #[test]
    fn header_carries_sender_size_and_opcode() {
        let (buf, _) = encode(|w| w.int(-1));
        let h = header(&buf).unwrap();
        assert_eq!(
            h,
            Header {
                sender: ObjectId(7),
                opcode: 3,
                size: 12
            }
        );
        assert_eq!(&buf[8..], &(-1i32).to_ne_bytes());
        assert!(header(&buf[..7]).is_none());
    }

    #[test]
    fn strings_are_nul_terminated_and_padded() {
        let (buf, _) = encode(|w| w.string("abc"));
        assert_eq!(&buf[8..12], &4u32.to_ne_bytes(), "length includes the nul");
        assert_eq!(&buf[12..16], b"abc\0");
        assert_eq!(buf.len(), 16);
        let (buf, _) = encode(|w| w.string("abcd"));
        assert_eq!(buf.len(), 8 + 4 + 8, "abcd\\0 is padded to eight");
        let (buf, _) = encode(|w| w.string_opt(None));
        assert_eq!(&buf[8..], &0u32.to_ne_bytes());
    }

    #[test]
    fn fixed_objects_arrays_and_fds() {
        let (buf, _) = encode(|w| w.fixed(1.5));
        assert_eq!(&buf[8..], &384i32.to_ne_bytes());
        let (buf, _) = encode(|w| {
            w.object_opt(None);
            w.object(ObjectId(9));
            w.array(&[1, 2, 3]);
        });
        assert_eq!(&buf[8..12], &0u32.to_ne_bytes());
        assert_eq!(&buf[12..16], &9u32.to_ne_bytes());
        assert_eq!(&buf[16..20], &3u32.to_ne_bytes());
        assert_eq!(&buf[20..24], &[1, 2, 3, 0]);
        let (a, _b) = std::os::unix::net::UnixStream::pair().unwrap();
        let (_, fds) = encode(|w| w.fd(a.as_fd()));
        assert_eq!(fds.len(), 1);
    }

    #[test]
    fn reader_round_trips_every_kind() {
        let (buf, _) = encode(|w| {
            w.uint(5);
            w.int(-2);
            w.fixed(0.25);
            w.string("hi");
            w.string_opt(None);
            w.object_opt(Some(ObjectId(4)));
            w.array(&[9]);
        });
        let mut r = Reader::new(&buf[8..]);
        assert_eq!(r.uint(), Some(5));
        assert_eq!(r.int(), Some(-2));
        assert_eq!(r.fixed(), Some(0.25));
        assert_eq!(r.string().as_deref(), Some("hi"));
        assert_eq!(r.string_opt(), Some(None));
        assert_eq!(r.object_opt(), Some(Some(ObjectId(4))));
        assert_eq!(r.array(), Some(vec![9]));
        assert_eq!(r.uint(), None, "past the end");
    }
}

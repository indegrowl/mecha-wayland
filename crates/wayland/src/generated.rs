//! The generated interfaces, requests, events and enums, from
//! `protocols/*.xml` through `build/generator.rs`.
#![allow(dead_code, unused_variables, unused_mut, clippy::all)]

include!(concat!(env!("OUT_DIR"), "/generated.rs"));

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::{Reader, header};
    use crate::{ObjectId, Wayland};
    use std::collections::VecDeque;
    use std::os::unix::net::UnixStream;

    fn wl() -> Wayland {
        let (a, _b) = UnixStream::pair().unwrap();
        Wayland::over(a)
    }

    #[test]
    fn a_request_encodes_its_arguments_in_order() {
        let mut wl = wl();
        let surface: WlSurface = wl.alloc(6);
        surface.attach(&mut wl, None, 3, -4);
        let out = &wl.out;
        let h = header(out).unwrap();
        assert_eq!((h.sender, h.opcode, h.size), (surface.id(), 1, 20));
        let mut r = Reader::new(&out[8..]);
        assert_eq!(r.object_opt(), Some(None));
        assert_eq!(r.int(), Some(3));
        assert_eq!(r.int(), Some(-4));
    }

    #[test]
    fn a_new_id_request_allocates_at_the_senders_version_and_returns_it() {
        let mut wl = wl();
        let compositor: WlCompositor = wl.alloc(5);
        let surface = compositor.create_surface(&mut wl);
        assert_eq!(surface.id(), ObjectId(3));
        assert_eq!(wl.version(surface), 5);
        assert_eq!(wl.info(surface.id()).unwrap().0.name, "wl_surface");
        let mut r = Reader::new(&wl.out[8..]);
        assert_eq!(r.object(), Some(ObjectId(3)));
    }

    #[test]
    fn an_enum_argument_is_written_as_its_value() {
        let mut wl = wl();
        let pool: WlShmPool = wl.alloc(1);
        pool.create_buffer(&mut wl, 0, 4, 4, 16, WlShmFormat::Xrgb8888);
        let mut r = Reader::new(&wl.out[8..]);
        r.object();
        assert_eq!(
            (r.int(), r.int(), r.int(), r.int()),
            (Some(0), Some(4), Some(4), Some(16))
        );
        assert_eq!(r.uint(), Some(1), "XRGB8888 is 1");
        let anchor = ZwlrLayerSurfaceV1Anchor::TOP | ZwlrLayerSurfaceV1Anchor::LEFT;
        assert_eq!(u32::from(anchor), 1 | 4);
    }

    #[test]
    fn an_event_decodes_with_the_sender_first() {
        let mut body = Vec::new();
        body.extend_from_slice(&320i32.to_ne_bytes());
        body.extend_from_slice(&200i32.to_ne_bytes());
        body.extend_from_slice(&4u32.to_ne_bytes());
        body.extend_from_slice(&2u32.to_ne_bytes());
        let mut fds = VecDeque::new();
        let mut app = app::App::new();
        let ev = XdgToplevelEvent::decode(&mut app, ObjectId(9), 0, &body, &mut fds).unwrap();
        match ev {
            XdgToplevelEvent::Configure {
                toplevel,
                width,
                height,
                states,
            } => {
                assert_eq!(toplevel.id(), ObjectId(9));
                assert_eq!((width, height), (320, 200));
                assert_eq!(states, 2u32.to_ne_bytes().to_vec());
            }
            other => panic!("{other:?}"),
        }
        assert!(XdgToplevelEvent::decode(&mut app, ObjectId(9), 0, &body[..4], &mut fds).is_none());
        assert!(XdgToplevelEvent::decode(&mut app, ObjectId(9), 99, &body, &mut fds).is_none());
    }

    #[test]
    fn interface_info_names_the_xml() {
        assert_eq!(WlCompositor::NAME, "wl_compositor");
        assert_eq!(WlCompositor::VERSION, 7);
        assert_eq!(WlCompositor::INFO.name, "wl_compositor");
        assert_eq!(XdgWmBase::VERSION, 7);
    }
}

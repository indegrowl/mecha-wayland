mod common;

use common::{device, near, pixel, png, rgb};
use geometry::{Color, Rect, Size};
use gles::XRGB8888;
use render::{Pass, Queue};

fn queue(w: f32, h: f32, clear: Color, scissor: Vec<Rect>) -> Queue {
    Queue {
        size: Size::new(w, h),
        scale: 1.0,
        clear,
        depth: 2.0,
        scissor,
        opaque: Pass::default(),
        translucent: Pass::default(),
    }
}

#[test]
fn a_linear_target_has_one_plane_and_the_xrgb_fourcc() {
    let Some((_g, mut d)) = device() else { return };
    let t = d.target(64, 48, &[]);
    assert_eq!((t.width(), t.height()), (64, 48));
    assert_eq!(t.fourcc(), XRGB8888);
    let planes: Vec<_> = t.planes().collect();
    assert_eq!(planes.len(), 1);
    assert!(planes[0].stride >= 64 * 4);
    assert_eq!(planes[0].offset, 0);
    d.destroy(t);
}

#[test]
fn a_target_made_with_the_linear_modifier_reports_it() {
    let Some((_g, mut d)) = device() else { return };
    let t = d.target(16, 16, &[0]);
    assert_eq!(t.modifier(), 0, "DRM_FORMAT_MOD_LINEAR");
    d.destroy(t);
}

#[test]
fn a_clear_covers_the_whole_scissor() {
    let Some((_g, mut d)) = device() else { return };
    let t = d.target(64, 48, &[]);
    let red = Color::rgb(1.0, 0.0, 0.0);
    d.draw(
        &t,
        &queue(64.0, 48.0, red, vec![Rect::new(0.0, 0.0, 64.0, 48.0)]),
    );
    let px = d.read(&t);
    png("a_clear_covers_the_whole_scissor", 64, 48, &px);
    assert_eq!(px.len(), 64 * 48 * 4);
    for y in 0..48 {
        for x in 0..64 {
            assert_eq!(pixel(&px, 64, x, y), rgb(1.0, 0.0, 0.0), "at {x},{y}");
        }
    }
    d.destroy(t);
}

#[test]
fn a_clear_under_two_rects_leaves_the_rest_untouched() {
    let Some((_g, mut d)) = device() else { return };
    let t = d.target(64, 48, &[]);
    let red = Color::rgb(1.0, 0.0, 0.0);
    let green = Color::rgb(0.0, 1.0, 0.0);
    d.draw(
        &t,
        &queue(64.0, 48.0, red, vec![Rect::new(0.0, 0.0, 64.0, 48.0)]),
    );
    d.draw(
        &t,
        &queue(
            64.0,
            48.0,
            green,
            vec![
                Rect::new(0.0, 0.0, 16.0, 16.0),
                Rect::new(40.0, 30.0, 24.0, 18.0),
            ],
        ),
    );
    let px = d.read(&t);
    png(
        "a_clear_under_two_rects_leaves_the_rest_untouched",
        64,
        48,
        &px,
    );
    assert_eq!(pixel(&px, 64, 0, 0), rgb(0.0, 1.0, 0.0));
    assert_eq!(pixel(&px, 64, 15, 15), rgb(0.0, 1.0, 0.0));
    assert_eq!(pixel(&px, 64, 16, 16), rgb(1.0, 0.0, 0.0));
    assert_eq!(pixel(&px, 64, 40, 30), rgb(0.0, 1.0, 0.0));
    assert_eq!(pixel(&px, 64, 63, 47), rgb(0.0, 1.0, 0.0));
    assert_eq!(pixel(&px, 64, 39, 29), rgb(1.0, 0.0, 0.0));
    assert!(near(pixel(&px, 64, 32, 24), rgb(1.0, 0.0, 0.0), 0));
    d.destroy(t);
}

#[test]
fn an_empty_scissor_draws_nothing() {
    let Some((_g, mut d)) = device() else { return };
    let t = d.target(8, 8, &[]);
    d.draw(
        &t,
        &queue(
            8.0,
            8.0,
            Color::rgb(0.0, 0.0, 1.0),
            vec![Rect::new(0.0, 0.0, 8.0, 8.0)],
        ),
    );
    d.draw(&t, &queue(8.0, 8.0, Color::rgb(1.0, 1.0, 1.0), vec![]));
    let px = d.read(&t);
    png("an_empty_scissor_draws_nothing", 8, 8, &px);
    assert_eq!(pixel(&px, 8, 4, 4), rgb(0.0, 0.0, 1.0));
    d.destroy(t);
}

#[test]
fn a_fractional_scissor_is_rounded_outward() {
    let Some((_g, mut d)) = device() else { return };
    let t = d.target(16, 16, &[]);
    d.draw(
        &t,
        &queue(
            16.0,
            16.0,
            Color::BLACK,
            vec![Rect::new(0.0, 0.0, 16.0, 16.0)],
        ),
    );
    d.draw(
        &t,
        &queue(
            16.0,
            16.0,
            Color::WHITE,
            vec![Rect::new(4.5, 4.5, 2.0, 2.0)],
        ),
    );
    let px = d.read(&t);
    png("a_fractional_scissor_is_rounded_outward", 16, 16, &px);
    assert_eq!(pixel(&px, 16, 4, 4), rgb(1.0, 1.0, 1.0), "floor of 4.5");
    assert_eq!(
        pixel(&px, 16, 6, 6),
        rgb(1.0, 1.0, 1.0),
        "ceil of 6.5 covers pixel 6"
    );
    assert_eq!(pixel(&px, 16, 7, 7), rgb(0.0, 0.0, 0.0));
    assert_eq!(pixel(&px, 16, 3, 3), rgb(0.0, 0.0, 0.0));
    d.destroy(t);
}

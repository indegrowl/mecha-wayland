#![forbid(unsafe_code)]
//! Plain geometry every tier shares: a point, a size, a rectangle, a
//! per-side inset, a per-corner value and a colour. All `Copy`, `Default`,
//! `PartialEq`; `f32` in whatever unit the caller means, which for layout
//! is pixels of a root's coordinate space, `y` growing downward.

use std::ops::{Add, Sub};

/// A position: `x` right, `y` down.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const ZERO: Self = Self::new(0.0, 0.0);

    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

impl Add for Point {
    type Output = Point;
    fn add(self, rhs: Point) -> Point {
        Point::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl Sub for Point {
    type Output = Point;
    fn sub(self, rhs: Point) -> Point {
        Point::new(self.x - rhs.x, self.y - rhs.y)
    }
}

/// An extent: width across, height down.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub const ZERO: Self = Self::new(0.0, 0.0);

    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }
}

/// An axis-aligned rectangle: its top-left corner and its extent.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub origin: Point,
    pub size: Size,
}

impl Rect {
    pub const ZERO: Self = Self::new(0.0, 0.0, 0.0, 0.0);

    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            origin: Point::new(x, y),
            size: Size::new(width, height),
        }
    }

    pub fn x(self) -> f32 {
        self.origin.x
    }
    pub fn y(self) -> f32 {
        self.origin.y
    }
    pub fn width(self) -> f32 {
        self.size.width
    }
    pub fn height(self) -> f32 {
        self.size.height
    }
    /// `x + width`.
    pub fn right(self) -> f32 {
        self.origin.x + self.size.width
    }
    /// `y + height`.
    pub fn bottom(self) -> f32 {
        self.origin.y + self.size.height
    }

    /// Covers no area: zero width or zero height.
    pub fn is_empty(self) -> bool {
        self.size.width == 0.0 || self.size.height == 0.0
    }

    /// The rectangle inside `insets`: the origin moves by the top and
    /// left insets, each dimension shrinks by the sum of its two sides,
    /// and neither dimension goes below zero.
    pub fn inset(self, insets: Insets<f32>) -> Rect {
        Rect::new(
            self.origin.x + insets.left,
            self.origin.y + insets.top,
            (self.size.width - insets.horizontal()).max(0.0),
            (self.size.height - insets.vertical()).max(0.0),
        )
    }

    /// Whether `p` is inside the rect. Half-open: the right and bottom
    /// edges are excluded, so two adjacent rects never both claim a point
    /// on their shared edge.
    pub fn contains(self, p: Point) -> bool {
        p.x >= self.x() && p.x < self.right() && p.y >= self.y() && p.y < self.bottom()
    }
}

/// One value per side of a box: padding, margin, border, inset.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Insets<T> {
    pub top: T,
    pub right: T,
    pub bottom: T,
    pub left: T,
}

impl<T> Insets<T> {
    /// Clockwise from the top, as CSS reads.
    pub const fn new(top: T, right: T, bottom: T, left: T) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    pub fn map<U>(self, f: impl Fn(T) -> U) -> Insets<U> {
        Insets {
            top: f(self.top),
            right: f(self.right),
            bottom: f(self.bottom),
            left: f(self.left),
        }
    }
}

impl<T: Copy> Insets<T> {
    /// The same value on every side.
    pub const fn all(v: T) -> Self {
        Self::new(v, v, v, v)
    }

    /// `horizontal` on left and right, `vertical` on top and bottom.
    pub const fn symmetric(horizontal: T, vertical: T) -> Self {
        Self::new(vertical, horizontal, vertical, horizontal)
    }
}

impl<T: Copy + Add<Output = T>> Insets<T> {
    /// `left + right`.
    pub fn horizontal(self) -> T {
        self.left + self.right
    }

    /// `top + bottom`.
    pub fn vertical(self) -> T {
        self.top + self.bottom
    }
}

/// A colour with straight (not premultiplied) alpha, each channel
/// `0.0..=1.0`. The default is transparent black.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const TRANSPARENT: Self = Self::rgba(0.0, 0.0, 0.0, 0.0);
    pub const BLACK: Self = Self::rgb(0.0, 0.0, 0.0);
    pub const WHITE: Self = Self::rgb(1.0, 1.0, 1.0);

    /// Opaque.
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Opaque, from 8-bit channels.
    pub const fn from_rgb8(r: u8, g: u8, b: u8) -> Self {
        Self::from_rgba8(r, g, b, 255)
    }

    /// From 8-bit channels, alpha included.
    pub const fn from_rgba8(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self::rgba(
            r as f32 / 255.0,
            g as f32 / 255.0,
            b as f32 / 255.0,
            a as f32 / 255.0,
        )
    }

    /// The same colour with `a` for its alpha.
    pub fn with_alpha(self, a: f32) -> Self {
        Self { a, ..self }
    }

    /// True when drawing this colour leaves no pixel behind.
    pub fn is_transparent(self) -> bool {
        self.a <= 0.0
    }

    /// Source-over compositing: `self` drawn on top of `under`. Straight
    /// alpha in, straight alpha out; `TRANSPARENT` when the result's alpha
    /// is zero.
    pub fn over(self, under: Color) -> Color {
        let a = self.a + under.a * (1.0 - self.a);
        if a <= 0.0 {
            return Color::TRANSPARENT;
        }
        let channel = |s: f32, u: f32| (s * self.a + u * under.a * (1.0 - self.a)) / a;
        Color {
            r: channel(self.r, under.r),
            g: channel(self.g, under.g),
            b: channel(self.b, under.b),
            a,
        }
    }
}

/// One value per corner, clockwise from the top left: CSS's
/// `border-radius` order.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Corners<T> {
    pub top_left: T,
    pub top_right: T,
    pub bottom_right: T,
    pub bottom_left: T,
}

impl<T> Corners<T> {
    pub const fn new(top_left: T, top_right: T, bottom_right: T, bottom_left: T) -> Self {
        Self {
            top_left,
            top_right,
            bottom_right,
            bottom_left,
        }
    }

    pub fn map<U>(self, f: impl Fn(T) -> U) -> Corners<U> {
        Corners {
            top_left: f(self.top_left),
            top_right: f(self.top_right),
            bottom_right: f(self.bottom_right),
            bottom_left: f(self.bottom_left),
        }
    }
}

impl<T: Copy> Corners<T> {
    /// The same value at every corner.
    pub const fn all(v: T) -> Self {
        Self::new(v, v, v, v)
    }
}

impl Corners<f32> {
    /// True when no corner is rounded.
    pub fn is_zero(self) -> bool {
        self.top_left <= 0.0
            && self.top_right <= 0.0
            && self.bottom_right <= 0.0
            && self.bottom_left <= 0.0
    }
}

pub mod prelude {
    pub use crate::{Color, Corners, Insets, Point, Rect, Size};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_arithmetic() {
        let a = Point::new(1.0, 2.0);
        let b = Point::new(10.0, 20.0);
        assert_eq!(a + b, Point::new(11.0, 22.0));
        assert_eq!(b - a, Point::new(9.0, 18.0));
        assert_eq!(Point::ZERO, Point::default());
    }

    #[test]
    fn rect_edges() {
        let r = Rect::new(10.0, 20.0, 30.0, 40.0);
        assert_eq!(
            (r.x(), r.y(), r.width(), r.height()),
            (10.0, 20.0, 30.0, 40.0)
        );
        assert_eq!((r.right(), r.bottom()), (40.0, 60.0));
        assert!(!r.is_empty());
        assert!(Rect::new(1.0, 1.0, 0.0, 5.0).is_empty());
        assert!(Rect::ZERO.is_empty());
    }

    #[test]
    fn rect_inset_moves_the_origin_and_shrinks() {
        let r = Rect::new(10.0, 20.0, 100.0, 50.0);
        let inner = r.inset(Insets::new(1.0, 2.0, 3.0, 4.0));
        assert_eq!(inner, Rect::new(14.0, 21.0, 94.0, 46.0));
    }

    #[test]
    fn rect_inset_clamps_at_zero() {
        let r = Rect::new(0.0, 0.0, 10.0, 10.0);
        let inner = r.inset(Insets::all(8.0));
        assert_eq!(inner, Rect::new(8.0, 8.0, 0.0, 0.0));
    }

    #[test]
    fn rect_contains_is_half_open() {
        let r = Rect::new(10.0, 20.0, 30.0, 40.0);
        assert!(
            r.contains(Point::new(10.0, 20.0)),
            "the top-left corner is in"
        );
        assert!(
            r.contains(Point::new(39.9, 59.9)),
            "just inside the far corner"
        );
        assert!(
            !r.contains(Point::new(40.0, 30.0)),
            "the right edge is excluded"
        );
        assert!(
            !r.contains(Point::new(20.0, 60.0)),
            "the bottom edge is excluded"
        );
        assert!(!r.contains(Point::new(9.9, 30.0)), "left of the rect");
        assert!(!r.contains(Point::new(20.0, 19.9)), "above the rect");
        assert!(
            !Rect::ZERO.contains(Point::ZERO),
            "a zero rect contains nothing"
        );
    }

    #[test]
    fn insets_constructors_and_sums() {
        assert_eq!(Insets::all(3.0), Insets::new(3.0, 3.0, 3.0, 3.0));
        assert_eq!(Insets::symmetric(1.0, 2.0), Insets::new(2.0, 1.0, 2.0, 1.0));
        let i = Insets::new(1.0, 2.0, 3.0, 4.0);
        assert_eq!(i.horizontal(), 6.0);
        assert_eq!(i.vertical(), 4.0);
        assert_eq!(i.map(|v| v * 2.0), Insets::new(2.0, 4.0, 6.0, 8.0));
    }

    #[test]
    fn color_constructors() {
        assert_eq!(Color::rgb(0.1, 0.2, 0.3), Color::rgba(0.1, 0.2, 0.3, 1.0));
        assert_eq!(Color::TRANSPARENT, Color::default());
        assert_eq!(Color::from_rgb8(255, 0, 0), Color::rgb(1.0, 0.0, 0.0));
        assert_eq!(Color::from_rgb8(0, 51, 102), Color::rgb(0.0, 0.2, 0.4));
        assert_eq!(Color::from_rgba8(0, 0, 0, 255), Color::BLACK);
        assert_eq!(
            Color::from_rgba8(255, 255, 255, 0),
            Color::WHITE.with_alpha(0.0)
        );
        assert!(Color::TRANSPARENT.is_transparent());
        assert!(Color::WHITE.with_alpha(0.0).is_transparent());
        assert!(!Color::WHITE.with_alpha(0.01).is_transparent());
    }

    #[test]
    fn opaque_over_anything_is_itself() {
        let red = Color::rgb(1.0, 0.0, 0.0);
        assert_eq!(red.over(Color::WHITE), red);
        assert_eq!(red.over(Color::TRANSPARENT), red);
        assert_eq!(red.over(Color::rgba(0.0, 1.0, 0.0, 0.5)), red);
    }

    #[test]
    fn transparent_over_a_color_is_that_color() {
        let green = Color::rgba(0.0, 1.0, 0.0, 0.5);
        assert_eq!(Color::TRANSPARENT.over(green), green);
        assert_eq!(
            Color::TRANSPARENT.over(Color::TRANSPARENT),
            Color::TRANSPARENT
        );
    }

    #[test]
    fn half_alpha_over_opaque_mixes_with_alpha_one() {
        let red = Color::rgba(1.0, 0.0, 0.0, 0.5);
        let blue = Color::rgb(0.0, 0.0, 1.0);
        assert_eq!(red.over(blue), Color::rgb(0.5, 0.0, 0.5));
    }

    #[test]
    fn corners_constructors_and_map() {
        assert_eq!(Corners::all(2.0), Corners::new(2.0, 2.0, 2.0, 2.0));
        let c = Corners::new(1.0, 2.0, 3.0, 4.0);
        assert_eq!(
            (c.top_left, c.top_right, c.bottom_right, c.bottom_left),
            (1.0, 2.0, 3.0, 4.0)
        );
        assert_eq!(c.map(|v| v * 2.0), Corners::new(2.0, 4.0, 6.0, 8.0));
        assert_eq!(Corners::<f32>::default(), Corners::all(0.0));
    }

    #[test]
    fn corners_is_zero() {
        assert!(Corners::all(0.0).is_zero());
        assert!(Corners::new(0.0, -1.0, 0.0, 0.0).is_zero());
        assert!(!Corners::new(0.0, 0.0, 0.5, 0.0).is_zero());
    }

    #[test]
    fn plain_data_has_c_layout() {
        use std::mem::{align_of, size_of};
        assert_eq!((size_of::<Point>(), align_of::<Point>()), (8, 4));
        assert_eq!((size_of::<Size>(), align_of::<Size>()), (8, 4));
        assert_eq!((size_of::<Rect>(), align_of::<Rect>()), (16, 4));
        assert_eq!(
            (size_of::<Insets<f32>>(), align_of::<Insets<f32>>()),
            (16, 4)
        );
        assert_eq!((size_of::<Color>(), align_of::<Color>()), (16, 4));
        assert_eq!(
            (size_of::<Corners<f32>>(), align_of::<Corners<f32>>()),
            (16, 4)
        );
    }
}

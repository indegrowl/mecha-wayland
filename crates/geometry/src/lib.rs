#![forbid(unsafe_code)]
//! Plain geometry every tier shares: a point, a size, a rectangle and a
//! per-side inset. All `Copy`, `Default`, `PartialEq`; `f32` in whatever
//! unit the caller means, which for layout is pixels of a root's
//! coordinate space, `y` growing downward.

use std::ops::{Add, Sub};

/// A position: `x` right, `y` down.
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
}

/// One value per side of a box: padding, margin, border, inset.
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

pub mod prelude {
    pub use crate::{Insets, Point, Rect, Size};
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
    fn insets_constructors_and_sums() {
        assert_eq!(Insets::all(3.0), Insets::new(3.0, 3.0, 3.0, 3.0));
        assert_eq!(Insets::symmetric(1.0, 2.0), Insets::new(2.0, 1.0, 2.0, 1.0));
        let i = Insets::new(1.0, 2.0, 3.0, 4.0);
        assert_eq!(i.horizontal(), 6.0);
        assert_eq!(i.vertical(), 4.0);
        assert_eq!(i.map(|v| v * 2.0), Insets::new(2.0, 4.0, 6.0, 8.0));
    }
}

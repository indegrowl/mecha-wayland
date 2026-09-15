//! Rect arithmetic the walk and the scene need and `geometry` does not
//! offer. Rects here are device pixels with non-negative sizes.

use geometry::Rect;

/// The two overlap in area. Touching edges do not overlap.
pub(crate) fn intersects(a: Rect, b: Rect) -> bool {
    a.x() < b.right() && b.x() < a.right() && a.y() < b.bottom() && b.y() < a.bottom()
}

/// `inner` lies within `outer`, edges included. An empty `inner` is
/// within nothing: there is no pixel to know the background of.
pub(crate) fn contains(outer: Rect, inner: Rect) -> bool {
    !inner.is_empty()
        && inner.x() >= outer.x()
        && inner.y() >= outer.y()
        && inner.right() <= outer.right()
        && inner.bottom() <= outer.bottom()
}

/// The smallest rect covering both. An empty side contributes nothing.
pub(crate) fn union(a: Rect, b: Rect) -> Rect {
    if a.is_empty() {
        return b;
    }
    if b.is_empty() {
        return a;
    }
    let x0 = a.x().min(b.x());
    let y0 = a.y().min(b.y());
    let x1 = a.right().max(b.right());
    let y1 = a.bottom().max(b.bottom());
    Rect::new(x0, y0, x1 - x0, y1 - y0)
}

/// The overlap, or `Rect::ZERO` when there is none.
pub(crate) fn intersection(a: Rect, b: Rect) -> Rect {
    let x0 = a.x().max(b.x());
    let y0 = a.y().max(b.y());
    let x1 = a.right().min(b.right());
    let y1 = a.bottom().min(b.bottom());
    if x1 <= x0 || y1 <= y0 {
        Rect::ZERO
    } else {
        Rect::new(x0, y0, x1 - x0, y1 - y0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlap_and_containment() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        assert!(intersects(a, Rect::new(5.0, 5.0, 10.0, 10.0)));
        assert!(
            !intersects(a, Rect::new(10.0, 0.0, 5.0, 5.0)),
            "touching is not overlap"
        );
        assert!(
            contains(a, Rect::new(0.0, 0.0, 10.0, 10.0)),
            "edges included"
        );
        assert!(contains(a, Rect::new(2.0, 2.0, 3.0, 3.0)));
        assert!(!contains(a, Rect::new(8.0, 8.0, 3.0, 3.0)));
        assert!(!contains(a, Rect::ZERO), "nothing is behind no pixel");
    }

    #[test]
    fn union_and_intersection() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        let b = Rect::new(5.0, 5.0, 10.0, 10.0);
        assert_eq!(union(a, b), Rect::new(0.0, 0.0, 15.0, 15.0));
        assert_eq!(union(a, Rect::ZERO), a);
        assert_eq!(union(Rect::ZERO, b), b);
        assert_eq!(intersection(a, b), Rect::new(5.0, 5.0, 5.0, 5.0));
        assert_eq!(intersection(a, Rect::new(20.0, 20.0, 1.0, 1.0)), Rect::ZERO);
    }
}

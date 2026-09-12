//! `Bundle`: the component values a node is spawned with.

use crate::{App, Component, NodeId};

mod sealed {
    pub trait Sealed {
        fn insert(self, app: &mut crate::App, id: crate::NodeId);
    }
}

/// The component values handed to [`App::spawn_with`]: `()` for none, or
/// a tuple of one to six [`Component`] values, `(pos,)` for one. Each is
/// written to the new node after it is linked into the tree and before
/// its `build` runs, and flags the node like any other write.
///
/// Sealed. A bare `C` is not a `Bundle`: a tuple may itself be a
/// `Component`, so the two impls would overlap. A type named twice in
/// one bundle is written twice in order; the last value stands.
pub trait Bundle: sealed::Sealed {}

impl sealed::Sealed for () {
    fn insert(self, _app: &mut App, _id: NodeId) {}
}
impl Bundle for () {}

/// `Bundle` for a tuple of components: write each in order.
macro_rules! tuple_bundle {
    ($($T:ident),+) => {
        impl<$($T: Component),+> sealed::Sealed for ($($T,)+) {
            #[allow(non_snake_case)]
            fn insert(self, app: &mut App, id: NodeId) {
                let ($($T,)+) = self;
                $(
                    *app
                        .component_mut::<$T>(id)
                        .expect("a node being spawned is live") = $T;
                )+
            }
        }

        impl<$($T: Component),+> Bundle for ($($T,)+) {}
    };
}

tuple_bundle!(A);
tuple_bundle!(A, B);
tuple_bundle!(A, B, C);
tuple_bundle!(A, B, C, D);
tuple_bundle!(A, B, C, D, E);
tuple_bundle!(A, B, C, D, E, F);

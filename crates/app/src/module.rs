//! Units of functionality that extend the runtime by installing into the
//! [`App`](crate::App).

use crate::App;

/// A unit that extends the runtime by installing into the [`App`]: it
/// registers components, attaches systems, and may set the runner.
///
/// The app knows no component or system of its own; layout, paint,
/// interactivity and presentation are each a module written against
/// `App`'s surface. Wiring behaviour to a node is a builder's job, not a
/// module's: a module supplies what every node carries and what every
/// tick runs, and a builder configures it per node.
///
/// [`App::add_module`] calls `install` right away, so a module sees
/// everything the modules added before it put in place, and can build on
/// it: a module that needs another's component is a module that must be
/// added after it. There is no dependency graph and no dedupe. Adding a
/// module twice installs it twice, and the first duplicate
/// `register_component` or `set_runner` panics.
///
/// A module is consumed by `install`: configuration travels in its
/// fields and is gone once installed.
///
/// ```
/// use app::prelude::*;
///
/// #[derive(Default)]
/// struct Layout { x: f32, y: f32 }
/// impl Component for Layout {}
///
/// fn center(app: &mut App, _: &Tick) {
///     let root = app.root();
///     let mut layout = app.component_mut::<Layout>(root).unwrap();
///     layout.x = 400.0;
///     layout.y = 300.0;
/// }
///
/// struct LayoutModule;
/// impl Module for LayoutModule {
///     fn install(self, app: &mut App) {
///         app.register_component::<Layout>().system(center);
///     }
/// }
///
/// let mut app = App::new();
/// app.add_module(LayoutModule);
/// app.tick();
/// let layout = app.component::<Layout>(app.root()).unwrap();
/// assert_eq!((layout.x, layout.y), (400.0, 300.0));
/// ```
pub trait Module {
    /// Install into `app`. Called once, by [`App::add_module`], at the
    /// moment the module is added.
    fn install(self, app: &mut App);
}

//! `PaintContext`: read and replace the owner's `Paint` through its
//! `Context`, for any widget. Blanket-implemented so every widget in the
//! app gets it once its crate depends on `paint`.

use app::{Context, Widget};

use crate::Paint;

/// The owner's `Paint`, reachable from any widget's [`Context`]. `Paint`
/// is a closed enum, not a flat struct, so unlike `layout::StyleContext`
/// this stops at the whole value: a widget's own trait (`DivContext` and
/// friends, in `widgets`) is where a variant's fields get their own
/// setters, because only the widget knows which variant it draws.
pub trait PaintContext {
    fn paint(&self) -> &Paint;
    /// Replace the whole `Paint`. `false` and no write if it already
    /// equals `paint` (`CompMut::set_if_neq`).
    fn set_paint(&mut self, paint: Paint) -> bool;
}

impl<W: Widget> PaintContext for Context<'_, W> {
    fn paint(&self) -> &Paint {
        self.component::<Paint>()
            .expect("PaintModule is installed wherever paint::PaintContext is used")
    }

    fn set_paint(&mut self, paint: Paint) -> bool {
        self.component_mut::<Paint>()
            .expect("PaintModule is installed wherever paint::PaintContext is used")
            .set_if_neq(paint)
    }
}

#[cfg(test)]
mod tests {
    use app::prelude::*;
    use geometry::Color;

    use super::*;
    use crate::{Paint, PaintModule, Quad};

    /// This test file's one event: run a stored action against the
    /// owner's own `Context`, since `Context` cannot be built directly
    /// outside `app`.
    struct Poke;
    impl Event for Poke {}

    struct Editable;
    struct EditableBuilder(Box<dyn FnMut(&mut Context<'_, Editable>) -> bool>);
    impl Build for EditableBuilder {
        type Widget = Editable;
    }
    impl Widget for Editable {
        type Builder = EditableBuilder;
        fn build(b: EditableBuilder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
            let mut action = b.0;
            s.on::<Poke>(me, move |ctx, _| {
                let wrote = action(ctx);
                WROTE.with(|w| w.set(wrote));
            });
            Editable
        }
    }

    thread_local! {
        static WROTE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    }

    fn app() -> App {
        let mut app = App::new();
        app.add_module(PaintModule);
        app
    }

    fn editable(
        app: &mut App,
        action: impl FnMut(&mut Context<'_, Editable>) -> bool + 'static,
    ) -> NodeId {
        app.spawn(app.root(), EditableBuilder(Box::new(action)))
            .id()
    }

    fn poke(app: &mut App, id: NodeId) -> bool {
        app.emit(Poke, id);
        app.flush();
        WROTE.with(|w| w.get())
    }

    fn black() -> Paint {
        Paint::Quad(Quad::new(Color::BLACK))
    }

    #[test]
    fn paint_reads_the_owners_paint() {
        let mut app = app();
        let id = editable(&mut app, |ctx| {
            assert_eq!(ctx.paint(), &Paint::None);
            false
        });
        poke(&mut app, id);
    }

    #[test]
    fn set_paint_writes_and_reports_whether_it_wrote() {
        let mut app = app();
        let id = editable(&mut app, |ctx| ctx.set_paint(black()));
        assert!(poke(&mut app, id), "the first write reports true");
        assert_eq!(app.component::<Paint>(id), Some(&black()));
        assert!(!poke(&mut app, id), "an equal write reports false");
    }
}

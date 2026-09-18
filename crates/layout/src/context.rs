//! `StyleContext`: read and set the owner's `LayoutStyle` through its
//! `Context`, one field at a time or the whole value, for any widget.

use app::{Context, Widget};
use geometry::Insets;

use crate::{Align, Direction, Display, Justify, LayoutStyle, Position, Val, Wrap};

/// The owner's `LayoutStyle`, reachable from any widget's [`Context`]:
/// the whole value, or one field at a time. `LayoutStyle`'s fields are
/// already all `pub`, and every node has exactly the same set of them,
/// so a setter per field is mechanical — each is `edit`, below, with a
/// different closure.
pub trait StyleContext {
    fn style(&self) -> &LayoutStyle;
    /// Replace the whole style. `false` and no write if it already
    /// equals `style` (`CompMut::set_if_neq`).
    fn set_style(&mut self, style: LayoutStyle) -> bool;

    fn set_display(&mut self, display: Display) -> bool {
        edit(self, |s| s.display = display)
    }
    fn set_direction(&mut self, direction: Direction) -> bool {
        edit(self, |s| s.direction = direction)
    }
    fn set_position(&mut self, position: Position) -> bool {
        edit(self, |s| s.position = position)
    }
    fn set_justify(&mut self, justify: Justify) -> bool {
        edit(self, |s| s.justify = justify)
    }
    fn set_align_content(&mut self, v: Justify) -> bool {
        edit(self, |s| s.align_content = v)
    }
    fn set_align_items(&mut self, v: Align) -> bool {
        edit(self, |s| s.align_items = v)
    }
    fn set_align_self(&mut self, v: Option<Align>) -> bool {
        edit(self, |s| s.align_self = v)
    }
    fn set_wrap(&mut self, wrap: Wrap) -> bool {
        edit(self, |s| s.wrap = wrap)
    }

    fn set_width(&mut self, v: Val) -> bool {
        edit(self, |s| s.width = v)
    }
    fn set_height(&mut self, v: Val) -> bool {
        edit(self, |s| s.height = v)
    }
    fn set_min_width(&mut self, v: Val) -> bool {
        edit(self, |s| s.min_width = v)
    }
    fn set_min_height(&mut self, v: Val) -> bool {
        edit(self, |s| s.min_height = v)
    }
    fn set_max_width(&mut self, v: Val) -> bool {
        edit(self, |s| s.max_width = v)
    }
    fn set_max_height(&mut self, v: Val) -> bool {
        edit(self, |s| s.max_height = v)
    }
    fn set_flex_basis(&mut self, v: Val) -> bool {
        edit(self, |s| s.flex_basis = v)
    }
    fn set_grow(&mut self, v: f32) -> bool {
        edit(self, |s| s.flex_grow = v)
    }
    fn set_shrink(&mut self, v: f32) -> bool {
        edit(self, |s| s.flex_shrink = v)
    }

    fn set_row_gap(&mut self, v: Val) -> bool {
        edit(self, |s| s.row_gap = v)
    }
    fn set_column_gap(&mut self, v: Val) -> bool {
        edit(self, |s| s.column_gap = v)
    }
    fn set_gap(&mut self, v: Val) -> bool {
        edit(self, |s| {
            s.row_gap = v;
            s.column_gap = v;
        })
    }
    fn set_padding(&mut self, v: Insets<Val>) -> bool {
        edit(self, |s| s.padding = v)
    }
    fn set_margin(&mut self, v: Insets<Val>) -> bool {
        edit(self, |s| s.margin = v)
    }
    fn set_inset(&mut self, v: Insets<Val>) -> bool {
        edit(self, |s| s.inset = v)
    }
    /// The box-model reserved space, CSS's `border-width` — not the
    /// visual stroke, which is `widgets::DivContext::set_border`.
    fn set_border_width(&mut self, v: Insets<Val>) -> bool {
        edit(self, |s| s.border = v)
    }

    /// The four `inset` sides individually, CSS's `top`/`right`/
    /// `bottom`/`left` on an absolutely positioned box.
    fn set_top(&mut self, v: Val) -> bool {
        edit(self, |s| s.inset.top = v)
    }
    fn set_right(&mut self, v: Val) -> bool {
        edit(self, |s| s.inset.right = v)
    }
    fn set_bottom(&mut self, v: Val) -> bool {
        edit(self, |s| s.inset.bottom = v)
    }
    fn set_left(&mut self, v: Val) -> bool {
        edit(self, |s| s.inset.left = v)
    }
}

/// crate-private: clone the style, mutate it, write it back. Every
/// `set_<field>` above is this one line with a different closure. A
/// plain function, not a trait method, so it never appears on `ctx.` —
/// a default method can only call sibling methods of the *same* trait
/// (or a supertrait), so a second helper trait would either not be
/// visible here or would leak `edit` onto every `Context<'_, W>` too.
fn edit<S: StyleContext + ?Sized>(ctx: &mut S, f: impl FnOnce(&mut LayoutStyle)) -> bool {
    let mut style = ctx.style().clone();
    f(&mut style);
    ctx.set_style(style)
}

impl<W: Widget> StyleContext for Context<'_, W> {
    fn style(&self) -> &LayoutStyle {
        self.component::<LayoutStyle>()
            .expect("LayoutModule is installed wherever layout::StyleContext is used")
    }
    fn set_style(&mut self, style: LayoutStyle) -> bool {
        self.component_mut::<LayoutStyle>()
            .expect("LayoutModule is installed wherever layout::StyleContext is used")
            .set_if_neq(style)
    }
    // every set_* above is inherited as a default method.
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use app::prelude::*;
    use geometry::Insets;

    use super::StyleContext;
    use crate::{Align, Direction, Display, Justify, LayoutStyle, Position, Wrap, px};

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
        static WROTE: Cell<bool> = const { Cell::new(false) };
    }

    fn app() -> App {
        let mut app = App::new();
        app.register_component::<LayoutStyle>();
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

    fn default_with(f: impl FnOnce(&mut LayoutStyle)) -> LayoutStyle {
        let mut s = LayoutStyle::default();
        f(&mut s);
        s
    }

    #[test]
    fn style_reads_the_owners_layout_style() {
        let mut app = app();
        let id = editable(&mut app, |ctx| {
            assert_eq!(ctx.style(), &LayoutStyle::default());
            false
        });
        poke(&mut app, id);
    }

    #[test]
    fn set_style_replaces_the_whole_style_and_reports_whether_it_wrote() {
        let mut app = app();
        let new_style = LayoutStyle::default().size(px(10.0), px(20.0));
        let for_closure = new_style.clone();
        let id = editable(&mut app, move |ctx| ctx.set_style(for_closure.clone()));
        assert!(poke(&mut app, id), "the first write reports true");
        assert_eq!(app.component::<LayoutStyle>(id).unwrap(), &new_style);
        assert!(!poke(&mut app, id), "an equal write reports false");
    }

    #[test]
    fn set_display_direction_position_and_wrap_write_one_field_each() {
        let mut app = app();
        let id = editable(&mut app, |ctx| ctx.set_display(Display::Block));
        assert!(poke(&mut app, id));
        assert_eq!(
            app.component::<LayoutStyle>(id).unwrap(),
            &default_with(|s| s.display = Display::Block)
        );

        let id = editable(&mut app, |ctx| ctx.set_direction(Direction::Column));
        assert!(poke(&mut app, id));
        assert_eq!(
            app.component::<LayoutStyle>(id).unwrap(),
            &default_with(|s| s.direction = Direction::Column)
        );

        let id = editable(&mut app, |ctx| ctx.set_position(Position::Absolute));
        assert!(poke(&mut app, id));
        assert_eq!(
            app.component::<LayoutStyle>(id).unwrap(),
            &default_with(|s| s.position = Position::Absolute)
        );

        let id = editable(&mut app, |ctx| ctx.set_wrap(Wrap::Yes));
        assert!(poke(&mut app, id));
        assert_eq!(
            app.component::<LayoutStyle>(id).unwrap(),
            &default_with(|s| s.wrap = Wrap::Yes)
        );
    }

    #[test]
    fn set_justify_and_alignment_fields_write_one_field_each() {
        let mut app = app();
        let id = editable(&mut app, |ctx| ctx.set_justify(Justify::Center));
        assert!(poke(&mut app, id));
        assert_eq!(
            app.component::<LayoutStyle>(id).unwrap(),
            &default_with(|s| s.justify = Justify::Center)
        );

        let id = editable(&mut app, |ctx| ctx.set_align_content(Justify::End));
        assert!(poke(&mut app, id));
        assert_eq!(
            app.component::<LayoutStyle>(id).unwrap(),
            &default_with(|s| s.align_content = Justify::End)
        );

        let id = editable(&mut app, |ctx| ctx.set_align_items(Align::Center));
        assert!(poke(&mut app, id));
        assert_eq!(
            app.component::<LayoutStyle>(id).unwrap(),
            &default_with(|s| s.align_items = Align::Center)
        );

        let id = editable(&mut app, |ctx| ctx.set_align_self(Some(Align::End)));
        assert!(poke(&mut app, id));
        assert_eq!(
            app.component::<LayoutStyle>(id).unwrap(),
            &default_with(|s| s.align_self = Some(Align::End))
        );
    }

    #[test]
    fn set_size_fields_write_one_field_each() {
        let mut app = app();
        let id = editable(&mut app, |ctx| ctx.set_width(px(10.0)));
        assert!(poke(&mut app, id));
        assert_eq!(
            app.component::<LayoutStyle>(id).unwrap(),
            &default_with(|s| s.width = px(10.0))
        );

        let id = editable(&mut app, |ctx| ctx.set_height(px(20.0)));
        assert!(poke(&mut app, id));
        assert_eq!(
            app.component::<LayoutStyle>(id).unwrap(),
            &default_with(|s| s.height = px(20.0))
        );

        let id = editable(&mut app, |ctx| ctx.set_min_width(px(1.0)));
        assert!(poke(&mut app, id));
        assert_eq!(
            app.component::<LayoutStyle>(id).unwrap(),
            &default_with(|s| s.min_width = px(1.0))
        );

        let id = editable(&mut app, |ctx| ctx.set_min_height(px(2.0)));
        assert!(poke(&mut app, id));
        assert_eq!(
            app.component::<LayoutStyle>(id).unwrap(),
            &default_with(|s| s.min_height = px(2.0))
        );

        let id = editable(&mut app, |ctx| ctx.set_max_width(px(3.0)));
        assert!(poke(&mut app, id));
        assert_eq!(
            app.component::<LayoutStyle>(id).unwrap(),
            &default_with(|s| s.max_width = px(3.0))
        );

        let id = editable(&mut app, |ctx| ctx.set_max_height(px(4.0)));
        assert!(poke(&mut app, id));
        assert_eq!(
            app.component::<LayoutStyle>(id).unwrap(),
            &default_with(|s| s.max_height = px(4.0))
        );

        let id = editable(&mut app, |ctx| ctx.set_flex_basis(px(5.0)));
        assert!(poke(&mut app, id));
        assert_eq!(
            app.component::<LayoutStyle>(id).unwrap(),
            &default_with(|s| s.flex_basis = px(5.0))
        );
    }

    #[test]
    fn set_grow_and_shrink_write_one_field_each() {
        let mut app = app();
        let id = editable(&mut app, |ctx| ctx.set_grow(2.0));
        assert!(poke(&mut app, id));
        assert_eq!(
            app.component::<LayoutStyle>(id).unwrap(),
            &default_with(|s| s.flex_grow = 2.0)
        );

        let id = editable(&mut app, |ctx| ctx.set_shrink(0.0));
        assert!(poke(&mut app, id));
        assert_eq!(
            app.component::<LayoutStyle>(id).unwrap(),
            &default_with(|s| s.flex_shrink = 0.0)
        );
    }

    #[test]
    fn set_spacing_fields_write_one_or_two_fields() {
        let mut app = app();
        let id = editable(&mut app, |ctx| ctx.set_row_gap(px(6.0)));
        assert!(poke(&mut app, id));
        assert_eq!(
            app.component::<LayoutStyle>(id).unwrap(),
            &default_with(|s| s.row_gap = px(6.0))
        );

        let id = editable(&mut app, |ctx| ctx.set_column_gap(px(7.0)));
        assert!(poke(&mut app, id));
        assert_eq!(
            app.component::<LayoutStyle>(id).unwrap(),
            &default_with(|s| s.column_gap = px(7.0))
        );

        let id = editable(&mut app, |ctx| ctx.set_gap(px(8.0)));
        assert!(poke(&mut app, id));
        assert_eq!(
            app.component::<LayoutStyle>(id).unwrap(),
            &default_with(|s| {
                s.row_gap = px(8.0);
                s.column_gap = px(8.0);
            })
        );

        let id = editable(&mut app, |ctx| ctx.set_padding(Insets::all(px(9.0))));
        assert!(poke(&mut app, id));
        assert_eq!(
            app.component::<LayoutStyle>(id).unwrap(),
            &default_with(|s| s.padding = Insets::all(px(9.0)))
        );

        let id = editable(&mut app, |ctx| ctx.set_margin(Insets::all(px(11.0))));
        assert!(poke(&mut app, id));
        assert_eq!(
            app.component::<LayoutStyle>(id).unwrap(),
            &default_with(|s| s.margin = Insets::all(px(11.0)))
        );

        let id = editable(&mut app, |ctx| ctx.set_inset(Insets::all(px(12.0))));
        assert!(poke(&mut app, id));
        assert_eq!(
            app.component::<LayoutStyle>(id).unwrap(),
            &default_with(|s| s.inset = Insets::all(px(12.0)))
        );

        let id = editable(&mut app, |ctx| ctx.set_border_width(Insets::all(px(13.0))));
        assert!(poke(&mut app, id));
        assert_eq!(
            app.component::<LayoutStyle>(id).unwrap(),
            &default_with(|s| s.border = Insets::all(px(13.0)))
        );
    }

    #[test]
    fn set_top_right_bottom_left_write_into_inset() {
        let mut app = app();
        let id = editable(&mut app, |ctx| ctx.set_top(px(14.0)));
        assert!(poke(&mut app, id));
        assert_eq!(
            app.component::<LayoutStyle>(id).unwrap(),
            &default_with(|s| s.inset.top = px(14.0))
        );

        let id = editable(&mut app, |ctx| ctx.set_right(px(15.0)));
        assert!(poke(&mut app, id));
        assert_eq!(
            app.component::<LayoutStyle>(id).unwrap(),
            &default_with(|s| s.inset.right = px(15.0))
        );

        let id = editable(&mut app, |ctx| ctx.set_bottom(px(16.0)));
        assert!(poke(&mut app, id));
        assert_eq!(
            app.component::<LayoutStyle>(id).unwrap(),
            &default_with(|s| s.inset.bottom = px(16.0))
        );

        let id = editable(&mut app, |ctx| ctx.set_left(px(17.0)));
        assert!(poke(&mut app, id));
        assert_eq!(
            app.component::<LayoutStyle>(id).unwrap(),
            &default_with(|s| s.inset.left = px(17.0))
        );
    }
}

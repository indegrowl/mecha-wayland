//! What a handler sees while it runs.

use std::ops::{Deref, DerefMut};

use crate::query::CompMut;
use crate::{App, Component, Handle, NodeId, Widget};

/// What a handler sees: its owner's widget, the node the event landed
/// on, and the whole app behind them.
///
/// The owner is the node whose `build` registered the handler, so
/// [`Context::me`] is that widget even when the event was emitted at a
/// child. It derefs to [`App`], so a handler can emit, signal, spawn,
/// remove, and read any widget or column. All of that only queues or
/// touches other nodes; the owner's own handler list is out of its slot
/// while it runs, so nothing here can alias it.
///
/// Because `Context` derefs to `App`, a handler may also call
/// [`App::flush`]: a nested flush skips the running target's handlers for
/// the same event, since that list is out of its slot while it runs.
pub struct Context<'a, W: Widget> {
    app: &'a mut App,
    me: Handle<W>,
    target: NodeId,
}

impl<'a, W: Widget> Context<'a, W> {
    /// Built by dispatch after it checked the owner is live.
    pub(crate) fn new(app: &'a mut App, me: Handle<W>, target: NodeId) -> Self {
        Self { app, me, target }
    }

    /// The owner's widget. Always present: a handler runs only while its
    /// owner is live, and the handle proves the widget type.
    ///
    /// # Panics
    ///
    /// If a handler removes its own owner and then calls `me`: for
    /// example `ctx.remove(ctx.handle())`, or removing an ancestor of the
    /// owner.
    #[inline]
    pub fn me(&mut self) -> &mut W {
        self.app
            .widget_mut::<W>(self.me)
            .expect("a handler's owner is live and holds its widget")
    }

    /// The owner, for naming it as a target.
    #[inline]
    pub fn handle(&self) -> Handle<W> {
        self.me
    }

    /// The node the event was emitted at. The owner itself in the
    /// common `s.on(me, ..)` case.
    #[inline]
    pub fn target(&self) -> NodeId {
        self.target
    }

    /// Another node's context, over the same app, with that node as
    /// both `me` and `target`. `None` if the handle is stale.
    pub fn at<V: Widget>(&mut self, handle: Handle<V>) -> Option<Context<'_, V>> {
        if self.app.is_live(handle) {
            Some(Context::new(self.app, handle, handle.id()))
        } else {
            None
        }
    }

    /// The owner's `C`. See [`App::component`].
    ///
    /// # Panics
    ///
    /// If `C` is not registered.
    pub fn component<C: Component>(&self) -> Option<&C> {
        self.app.component::<C>(self.me)
    }

    /// A write guard for the owner's `C`. See [`App::component_mut`].
    ///
    /// # Panics
    ///
    /// If `C` is not registered.
    pub fn component_mut<C: Component>(&mut self) -> Option<CompMut<'_, C>> {
        self.app.component_mut::<C>(self.me)
    }
}

impl<W: Widget> Deref for Context<'_, W> {
    type Target = App;

    fn deref(&self) -> &App {
        self.app
    }
}

impl<W: Widget> DerefMut for Context<'_, W> {
    fn deref_mut(&mut self) -> &mut App {
        self.app
    }
}

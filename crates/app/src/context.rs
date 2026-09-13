//! What a handler sees while it runs.

use crate::handler::Targets;
use crate::query::CompMut;
use crate::resource::ResourceMut;
use crate::{
    App, Build, Bundle, Component, Event, Handle, NodeId, Query, Resource, Signal, Tree, Widget,
};

/// What a handler sees: its owner's widget, the node the event landed
/// on, and a fixed slice of the app behind them.
///
/// The owner is the node whose `build` registered the handler, so
/// [`Context::me`] is that widget even when the event was emitted at a
/// child. The app is reachable only through the methods here: a handler
/// queues messages, spawns and removes, reads widgets by id, and touches
/// its owner's components and the app's resources. It cannot flush: a
/// context queues, and only the app dequeues, so a handler never runs
/// queued work in the middle of another job. Whole-column queries are a
/// system's job.
///
/// Every method that hands out a guard, `component_mut`, `resource_mut`
/// and `fetch`, borrows the context for the guard's lifetime, so nothing
/// else on it is callable meanwhile, `at` and `me` included. Fetch,
/// write, drop, then go on.
///
/// The owner's own handler list is out of its slot while a handler runs,
/// so nothing here can alias it.
///
/// ```compile_fail
/// use app::prelude::*;
/// # struct Leaf;
/// # impl Build for Leaf { type Widget = Leaf; }
/// # impl Widget for Leaf {
/// #     type Builder = Leaf;
/// #     fn build(b: Leaf, _: Handle<Self>, _: &mut Spawner<'_, Self>) -> Self { b }
/// # }
/// fn handler(ctx: &mut Context<'_, Leaf>) {
///     ctx.flush(); // no method named `flush` found for `Context`
/// }
/// ```
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

    // ── owner and target ─────────────────────────────────────────────────

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

    // ── data on the owner ────────────────────────────────────────────────

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

    /// The app's `R`. See [`App::resource`].
    ///
    /// # Panics
    ///
    /// If `R` is not inserted.
    pub fn resource<R: Resource>(&self) -> &R {
        self.app.resource::<R>()
    }

    /// A write guard for the app's `R`. See [`App::resource_mut`].
    ///
    /// # Panics
    ///
    /// If `R` is not inserted.
    pub fn resource_mut<R: Resource>(&mut self) -> ResourceMut<'_, R> {
        self.app.resource_mut::<R>()
    }

    /// [`App::fetch`] at the owner: several guards on the owner's node,
    /// and resources, alive at once.
    ///
    /// # Panics
    ///
    /// As [`Context::me`]: if the handler removed its own owner earlier
    /// in the same call. Also under [`App::fetch`]'s conditions.
    pub fn fetch<Q: Query>(&mut self) -> Q::One<'_> {
        self.app.fetch::<Q>(self.me)
    }

    // ── messages: queue only ─────────────────────────────────────────────

    /// See [`App::emit`]. Queued; runs after this handler returns.
    pub fn emit<E: Event>(&mut self, event: E, targets: impl Into<Targets>) {
        self.app.emit(event, targets)
    }

    /// See [`App::signal`]. Queued; runs after this handler returns.
    pub fn signal<S: Signal>(&mut self, signal: S) {
        self.app.signal(signal)
    }

    // ── tree ─────────────────────────────────────────────────────────────

    /// The tree read-only: `root`, `parent`, `children`, `ancestors`,
    /// `descendants`, `is_live`. See [`App::tree`].
    pub fn tree(&self) -> Tree<'_> {
        self.app.tree()
    }

    /// See [`App::spawn`].
    pub fn spawn<B: Build>(&mut self, parent: impl Into<NodeId>, builder: B) -> Handle<B::Widget> {
        self.app.spawn(parent, builder)
    }

    /// See [`App::spawn_with`].
    pub fn spawn_with<B: Build, K: Bundle>(
        &mut self,
        parent: impl Into<NodeId>,
        builder: B,
        bundle: K,
    ) -> Handle<B::Widget> {
        self.app.spawn_with(parent, builder, bundle)
    }

    /// See [`App::remove`]. Immediate, not queued: removing the owner or
    /// an ancestor of it makes a later `me` or `fetch` panic.
    pub fn remove(&mut self, id: impl Into<NodeId>) -> bool {
        self.app.remove(id)
    }

    // ── widgets by id ────────────────────────────────────────────────────

    /// See [`App::widget`].
    pub fn widget<V: Widget>(&self, id: impl Into<NodeId>) -> Option<&V> {
        self.app.widget::<V>(id)
    }

    /// See [`App::widget_mut`].
    pub fn widget_mut<V: Widget>(&mut self, id: impl Into<NodeId>) -> Option<&mut V> {
        self.app.widget_mut::<V>(id)
    }
}

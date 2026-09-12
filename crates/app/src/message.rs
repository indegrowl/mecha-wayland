//! The two kinds of message in the runtime, and the built-in ones.
//!
//! - A [`Signal`] is app-wide: it goes to every *system* registered for
//!   its type, through [`App::signal`](crate::App::signal).
//! - An [`Event`] is node-level: it goes to the *handlers* of the nodes it
//!   is emitted at, through [`App::emit`](crate::App::emit).
//!
//! Both are markers. Any `'static` type can be one, or both. Nothing runs
//! when a message is sent; [`App::flush`](crate::App::flush) runs it.

use crate::NodeId;
use crate::handler::Targets;

/// An app-wide message consumed by systems.
///
/// ```
/// # use app::Signal;
/// struct Resized { width: u32, height: u32 }
/// impl Signal for Resized {}
/// ```
pub trait Signal: 'static {}

/// A node-level message consumed by handlers.
///
/// ```
/// # use app::Event;
/// struct Click;
/// impl Event for Click {}
/// ```
pub trait Event: 'static {}

/// The built-in signal [`App::tick`](crate::App::tick) sends first, once
/// per tick. Systems that do a frame's work sit here.
pub struct Tick;
impl Signal for Tick {}

/// The built-in signal [`App::tick`](crate::App::tick) sends after
/// [`Tick`], once everything `Tick` caused has run. The `OnChanged` drains
/// sit here so a tick's writes fire in the same tick.
pub struct PostTick;
impl Signal for PostTick {}

/// A node was spawned. Queued by [`App::spawn`](crate::App::spawn) after
/// the node's `build` returned and its widget is stored, so a system
/// sees a finished node. A subtree's signals arrive children before
/// parent, since a child's spawn finishes inside its parent's build.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spawned {
    pub id: NodeId,
    pub parent: NodeId,
}
impl Signal for Spawned {}

/// A subtree was removed. Queued by [`App::remove`](crate::App::remove),
/// once per call, for the subtree's root only. By the time a system sees
/// it, `id` is stale; `parent` is the node it was removed from and is
/// usually still live, but is not guaranteed to be, since it may itself
/// be removed later in the same flush. Both fields are for bookkeeping,
/// not lookups.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Removed {
    pub id: NodeId,
    pub parent: NodeId,
}
impl Signal for Removed {}

/// The signal form of an emit: the event by value and the nodes it went
/// to, sent after those nodes' handlers ran. How a system observes an
/// event without owning a node. Dropped, like any signal, when no system
/// is registered for it.
pub struct Emitted<E: Event> {
    pub event: E,
    pub targets: Targets,
}
impl<E: Event> Signal for Emitted<E> {}

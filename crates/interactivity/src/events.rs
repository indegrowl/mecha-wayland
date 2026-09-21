//! Node-level events dispatched from a `ContactInput`. All five carry the
//! same two fields; see the crate docs for which nodes receive them and
//! in what order.

use app::Event;
use geometry::Point;

use crate::ContactId;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Press {
    pub contact: ContactId,
    pub position: Point,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Release {
    pub contact: ContactId,
    pub position: Point,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Enter {
    pub contact: ContactId,
    pub position: Point,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Exit {
    pub contact: ContactId,
    pub position: Point,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Clicked {
    pub contact: ContactId,
    pub position: Point,
}

impl Event for Press {}
impl Event for Release {}
impl Event for Enter {}
impl Event for Exit {}
impl Event for Clicked {}

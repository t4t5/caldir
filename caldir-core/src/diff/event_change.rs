use crate::Event;

#[derive(Debug, PartialEq, Eq)]
pub enum EventChange {
    Create(Event),
    Update { from: Event, to: Event },
    Delete(Event),
}

use crate::Event;

#[derive(Debug, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)]
pub enum EventChange {
    Create(Event),
    Update { from: Event, to: Event },
    Delete(Event),
}

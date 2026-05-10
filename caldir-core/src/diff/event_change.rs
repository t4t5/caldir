use crate::Event;

pub enum EventChange {
    Create(Event),
    Update { from: Event, to: Event },
    Delete(Event),
}

use crate::event::Event;
use icalendar::{Component, EventLike};

impl From<&Event> for icalendar::Event {
    fn from(value: &Event) -> Self {
        let mut event = icalendar::Event::new();
        event.starts(icalendar::DatePerhapsTime::from(&value.start));

        if !value.summary.is_empty() {
            event.summary(&value.summary);
        }

        if let Some(location) = &value.location {
            event.location(location);
        }

        event.done()
    }
}

impl From<Event> for icalendar::Event {
    fn from(value: Event) -> Self {
        icalendar::Event::from(&value)
    }
}

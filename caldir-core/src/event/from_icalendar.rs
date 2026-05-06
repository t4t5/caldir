use crate::event::{Event, EventError, EventTime};
use icalendar::{Component, EventLike};

impl TryFrom<&icalendar::Event> for Event {
    type Error = EventError;

    fn try_from(value: &icalendar::Event) -> Result<Self, Self::Error> {
        let start: EventTime = value
            .get_start()
            .ok_or(EventError::MissingStart)?
            .try_into()?;

        Ok(Event {
            summary: value.get_summary().unwrap_or("").to_string(),
            location: value.get_location().map(ToString::to_string),
            start,
        })
    }
}

impl TryFrom<icalendar::Event> for Event {
    type Error = EventError;

    fn try_from(value: icalendar::Event) -> Result<Self, Self::Error> {
        Event::try_from(&value)
    }
}

mod error;
mod slugify;

pub use error::EventError;
use icalendar::Component;

#[derive(Debug, Clone)]
pub struct Event(icalendar::Event);

impl Event {
    pub(crate) fn from_contents(contents: &str) -> Result<Self, EventError> {
        let calendar: icalendar::Calendar = contents
            .parse()
            .map_err(|err| EventError::IcsParse(contents.to_string(), err))?;

        Self::from_ical_calendar(&calendar)
    }

    pub(crate) fn from_ical_calendar(icalendar: &icalendar::Calendar) -> Result<Self, EventError> {
        let ical_event = icalendar
            .events()
            .next()
            .ok_or_else(|| EventError::NoEventInIcs(icalendar.clone()))?;

        Self::from_ical_event(ical_event)
    }

    pub(crate) fn from_ical_event(inner: &icalendar::Event) -> Result<Self, EventError> {
        if inner.get_start().is_none() {
            return Err(EventError::MissingStart);
        }

        Ok(Event(inner.clone()))
    }

    pub fn ical_event(&self) -> &icalendar::Event {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_event_without_start() {
        let result = Event::from_ical_event(&icalendar::Event::new().done());

        assert!(matches!(result, Err(EventError::MissingStart)));
    }
}

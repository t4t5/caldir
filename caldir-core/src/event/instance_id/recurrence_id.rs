use super::super::time::EventTimeKey;
use crate::EventTime;
use std::hash::{Hash, Hasher};

// The instance identifier in a recurring event
#[derive(Debug, Clone)]
pub struct RecurrenceId(EventTime);

impl RecurrenceId {
    pub fn as_event_time(&self) -> &EventTime {
        &self.0
    }

    pub fn from_event_time(event_time: EventTime) -> Self {
        RecurrenceId(event_time)
    }

    fn key(&self) -> EventTimeKey {
        self.0.identity_key()
    }
}

impl PartialEq for RecurrenceId {
    fn eq(&self, other: &Self) -> bool {
        self.key() == other.key()
    }
}

impl Eq for RecurrenceId {}

impl Hash for RecurrenceId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.key().hash(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn zoned(tzid: &str, hour: u32) -> RecurrenceId {
        RecurrenceId::from_event_time(EventTime::DateTimeZoned {
            datetime: NaiveDate::from_ymd_opt(2026, 7, 14)
                .unwrap()
                .and_hms_opt(hour, 0, 0)
                .unwrap(),
            tzid: tzid.to_string(),
        })
    }

    #[test]
    fn same_instant_in_different_zones_is_equal() {
        assert_eq!(zoned("Europe/Stockholm", 19), zoned("Europe/London", 18));
    }
}

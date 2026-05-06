mod error;

use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
pub use error::EventTimeError;
use icalendar::{CalendarDateTime, DatePerhapsTime};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventTime {
    Date(NaiveDate),
    DateTimeUtc(DateTime<Utc>),
    DateTimeFloating(NaiveDateTime),
    DateTimeZoned {
        datetime: NaiveDateTime,
        tzid: chrono_tz::Tz,
    },
}

impl From<&EventTime> for DatePerhapsTime {
    fn from(value: &EventTime) -> Self {
        match value {
            EventTime::Date(date) => (*date).into(),
            EventTime::DateTimeUtc(datetime) => (*datetime).into(),
            EventTime::DateTimeFloating(datetime) => (*datetime).into(),
            EventTime::DateTimeZoned { datetime, tzid } => CalendarDateTime::WithTimezone {
                date_time: *datetime,
                tzid: tzid.name().to_string(),
            }
            .into(),
        }
    }
}

impl From<EventTime> for DatePerhapsTime {
    fn from(value: EventTime) -> Self {
        DatePerhapsTime::from(&value)
    }
}

impl TryFrom<DatePerhapsTime> for EventTime {
    type Error = EventTimeError;

    fn try_from(value: DatePerhapsTime) -> Result<Self, Self::Error> {
        match value {
            DatePerhapsTime::Date(date) => Ok(EventTime::Date(date)),
            DatePerhapsTime::DateTime(CalendarDateTime::Floating(datetime)) => {
                Ok(EventTime::DateTimeFloating(datetime))
            }
            DatePerhapsTime::DateTime(CalendarDateTime::Utc(datetime)) => {
                Ok(EventTime::DateTimeUtc(datetime))
            }
            DatePerhapsTime::DateTime(CalendarDateTime::WithTimezone { date_time, tzid }) => {
                let parsed_tzid = tzid
                    .parse::<chrono_tz::Tz>()
                    .map_err(|_| EventTimeError::InvalidTimezone(tzid))?;

                Ok(EventTime::DateTimeZoned {
                    datetime: date_time,
                    tzid: parsed_tzid,
                })
            }
        }
    }
}

use caldir_core::event::{
    Attendee, Event, EventStatus, EventTime, ParticipationStatus, Recurrence, Transparency,
};

pub trait ToGoogle {
    fn to_google(&self) -> google_calendar::types::Event;
}

impl ToGoogle for Event {
    fn to_google(&self) -> google_calendar::types::Event {
        let start = event_time_to_google(&self.start);
        let end = event_time_to_google(&self.end);

        let status = match self.status {
            EventStatus::Confirmed => "confirmed".to_string(),
            EventStatus::Tentative => "tentative".to_string(),
            EventStatus::Cancelled => "cancelled".to_string(),
        };

        let transparency = match self.transparency {
            Transparency::Opaque => "opaque".to_string(),
            Transparency::Transparent => "transparent".to_string(),
        };

        let reminders = if self.reminders.is_empty() {
            None
        } else {
            Some(google_calendar::types::Reminders {
                overrides: self
                    .reminders
                    .iter()
                    .map(|r| google_calendar::types::EventReminder {
                        method: "popup".to_string(),
                        minutes: r.minutes,
                    })
                    .collect(),
                use_default: false,
            })
        };

        let attendees: Vec<google_calendar::types::EventAttendee> =
            self.attendees.iter().map(attendee_to_google).collect();

        let recurrence = self
            .recurrence
            .as_ref()
            .map(recurrence_to_google)
            .unwrap_or_default();

        let original_start_time = self.recurrence_id.as_ref().map(event_time_to_google);

        // Get Google's event ID from custom properties (if available)
        let google_event_id = self
            .custom_properties
            .iter()
            .find(|(k, _)| k == "X-GOOGLE-EVENT-ID")
            .map(|(_, v)| v.clone())
            .unwrap_or_default();

        google_calendar::types::Event {
            id: google_event_id,
            i_cal_uid: self.uid.clone(),
            summary: self.summary.clone(),
            description: self.description.clone().unwrap_or_default(),
            location: self.location.clone().unwrap_or_default(),
            start: Some(start),
            end: Some(end),
            status,
            transparency,
            reminders,
            attendees,
            recurrence,
            original_start_time,
            sequence: self.sequence.unwrap_or(0),
            ..Default::default()
        }
    }
}

fn attendee_to_google(attendee: &Attendee) -> google_calendar::types::EventAttendee {
    google_calendar::types::EventAttendee {
        email: attendee.email.clone(),
        display_name: attendee.name.clone().unwrap_or_default(),
        response_status: attendee
            .response_status
            .map(participation_status_to_google)
            .unwrap_or("needsAction")
            .to_string(),
        additional_guests: 0,
        comment: String::new(),
        id: String::new(),
        optional: false,
        organizer: false,
        resource: false,
        self_: false,
    }
}

fn event_time_to_google(time: &EventTime) -> google_calendar::types::EventDateTime {
    match time {
        EventTime::Date(d) => google_calendar::types::EventDateTime {
            date: Some(*d),
            date_time: None,
            time_zone: String::new(),
        },
        EventTime::DateTimeUtc(dt) => google_calendar::types::EventDateTime {
            date: None,
            date_time: Some(*dt),
            time_zone: String::new(),
        },
        EventTime::DateTimeFloating(dt) => google_calendar::types::EventDateTime {
            date: None,
            date_time: Some(dt.and_utc()),
            time_zone: String::new(),
        },
        EventTime::DateTimeZoned { datetime, tzid } => google_calendar::types::EventDateTime {
            date: None,
            date_time: Some(datetime.and_utc()),
            time_zone: tzid.clone(),
        },
    }
}

/// Convert a typed Recurrence into Google's Vec<String> format.
fn recurrence_to_google(rec: &Recurrence) -> Vec<String> {
    let mut result = vec![format!("RRULE:{}", rec.rrule)];
    for exdate in &rec.exdates {
        result.push(format_exdate_for_google(exdate));
    }
    result
}

/// Format a single EventTime as an EXDATE string for Google Calendar API.
fn format_exdate_for_google(time: &EventTime) -> String {
    match time {
        EventTime::Date(d) => format!("EXDATE;VALUE=DATE:{}", d.format("%Y%m%d")),
        EventTime::DateTimeUtc(dt) => format!("EXDATE:{}", dt.format("%Y%m%dT%H%M%SZ")),
        EventTime::DateTimeFloating(dt) => format!("EXDATE:{}", dt.format("%Y%m%dT%H%M%S")),
        EventTime::DateTimeZoned { datetime, tzid } => {
            format!("EXDATE;TZID={}:{}", tzid, datetime.format("%Y%m%dT%H%M%S"))
        }
    }
}

fn participation_status_to_google(status: ParticipationStatus) -> &'static str {
    match status {
        ParticipationStatus::Accepted => "accepted",
        ParticipationStatus::Declined => "declined",
        ParticipationStatus::Tentative => "tentative",
        ParticipationStatus::NeedsAction => "needsAction",
    }
}

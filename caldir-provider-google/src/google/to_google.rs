use crate::types::{Event, EventStatus, EventTime, ParticipationStatus, Transparency};
use google_calendar::types::{EventAttendee, EventDateTime, EventReminder, Reminders};

/// Convert EventTime to Google's EventDateTime
fn event_time_to_google(time: &EventTime) -> EventDateTime {
    match time {
        EventTime::Date(d) => EventDateTime {
            date: Some(*d),
            date_time: None,
            time_zone: String::new(),
        },
        EventTime::DateTimeUtc(dt) => EventDateTime {
            date: None,
            date_time: Some(*dt),
            time_zone: String::new(),
        },
        EventTime::DateTimeFloating(dt) => EventDateTime {
            date: None,
            date_time: Some(dt.and_utc()),
            time_zone: String::new(),
        },
        EventTime::DateTimeZoned { datetime, tzid } => EventDateTime {
            date: None,
            date_time: Some(datetime.and_utc()),
            time_zone: tzid.clone(),
        },
    }
}

/// Convert our Event to a Google Calendar API Event
pub fn to_google_event(event: &Event) -> google_calendar::types::Event {
    let start = event_time_to_google(&event.start);
    let end = event_time_to_google(&event.end);

    let status = match event.status {
        EventStatus::Confirmed => "confirmed".to_string(),
        EventStatus::Tentative => "tentative".to_string(),
        EventStatus::Cancelled => "cancelled".to_string(),
    };

    let transparency = match event.transparency {
        Transparency::Opaque => "opaque".to_string(),
        Transparency::Transparent => "transparent".to_string(),
    };

    let reminders = if event.reminders.is_empty() {
        None
    } else {
        Some(Reminders {
            overrides: event
                .reminders
                .iter()
                .map(|r| EventReminder {
                    method: "popup".to_string(),
                    minutes: r.minutes,
                })
                .collect(),
            use_default: false,
        })
    };

    let attendees: Vec<EventAttendee> = event
        .attendees
        .iter()
        .map(|a| EventAttendee {
            email: a.email.clone(),
            display_name: a.name.clone().unwrap_or_default(),
            response_status: a
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
        })
        .collect();

    let recurrence = event.recurrence.clone().unwrap_or_default();

    let original_start_time = event.original_start.as_ref().map(event_time_to_google);

    google_calendar::types::Event {
        id: event.id.clone(),
        summary: event.summary.clone(),
        description: event.description.clone().unwrap_or_default(),
        location: event.location.clone().unwrap_or_default(),
        start: Some(start),
        end: Some(end),
        status,
        transparency,
        reminders,
        attendees,
        recurrence,
        original_start_time,
        sequence: event.sequence.unwrap_or(0),
        ..Default::default()
    }
}

/// Convert ParticipationStatus to Google's response status format
fn participation_status_to_google(status: ParticipationStatus) -> &'static str {
    match status {
        ParticipationStatus::Accepted => "accepted",
        ParticipationStatus::Declined => "declined",
        ParticipationStatus::Tentative => "tentative",
        ParticipationStatus::NeedsAction => "needsAction",
    }
}

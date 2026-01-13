use crate::types::{Attendee, Event, EventStatus, EventTime, ParticipationStatus, Transparency};

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
        Some(google_calendar::types::Reminders {
            overrides: event
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
        event.attendees.iter().map(attendee_to_google).collect();

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

/// Convert EventTime to Google's EventDateTime
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

/// Convert ParticipationStatus to Google's response status format
fn participation_status_to_google(status: ParticipationStatus) -> &'static str {
    match status {
        ParticipationStatus::Accepted => "accepted",
        ParticipationStatus::Declined => "declined",
        ParticipationStatus::Tentative => "tentative",
        ParticipationStatus::NeedsAction => "needsAction",
    }
}

use caldir_core::{
    Attendee, Availability, Event, EventTime, ParticipationStatus, Recurrence, RecurrenceId,
    Status, Visibility,
};

use crate::constants::{PROVIDER_COLOR_ID_PROPERTY, PROVIDER_EVENT_ID_PROPERTY};

pub trait ToGoogle {
    fn to_google(&self) -> google_calendar::types::Event;
}

impl ToGoogle for Event {
    fn to_google(&self) -> google_calendar::types::Event {
        let start = event_time_to_google(&self.start);
        let end = self
            .end
            .as_ref()
            .map(event_time_to_google)
            .unwrap_or(start.clone());

        let status = match self.status {
            Status::Confirmed => "confirmed".to_string(),
            Status::Tentative => "tentative".to_string(),
            Status::Cancelled => "cancelled".to_string(),
        };

        let transparency = match self.availability {
            Availability::Busy => "opaque".to_string(),
            Availability::Free => "transparent".to_string(),
        };

        // None → "default" (inherit calendar visibility); Some(Public) → "public".
        let visibility = match self.visibility {
            None => "default".to_string(),
            Some(Visibility::Public) => "public".to_string(),
            Some(Visibility::Private) => "private".to_string(),
            Some(Visibility::Confidential) => "confidential".to_string(),
        };

        let valid_reminders: Vec<_> = self
            .reminders
            .iter()
            .filter(|r| r.minutes_before_start > 0)
            .map(|r| google_calendar::types::EventReminder {
                method: "popup".to_string(),
                minutes: r.minutes_before_start,
            })
            .collect();

        // "No VALARM locally" = "inherit Google's calendar defaults"
        // Sending `None` here would otherwise clear the calendar-level default
        let reminders = if valid_reminders.is_empty() {
            Some(google_calendar::types::Reminders {
                overrides: vec![],
                use_default: true,
            })
        } else {
            Some(google_calendar::types::Reminders {
                overrides: valid_reminders,
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

        let original_start_time = self
            .recurrence_id
            .as_ref()
            .map(RecurrenceId::as_event_time)
            .map(event_time_to_google);

        let google_event_id = self
            .x_property(PROVIDER_EVENT_ID_PROPERTY)
            .unwrap_or_default()
            .to_string();

        let color_id = self
            .x_property(PROVIDER_COLOR_ID_PROPERTY)
            .unwrap_or_default()
            .to_string();

        let conference_url = self.conference_url.as_deref();

        google_calendar::types::Event {
            id: google_event_id,
            i_cal_uid: self.uid.as_str().to_string(),
            summary: self.summary.clone().unwrap_or_default(),
            description: self.description.clone().unwrap_or_default(),
            location: self.location.clone().unwrap_or_default(),
            start: Some(start),
            end: Some(end),
            status,
            transparency,
            visibility,
            reminders,
            attendees,
            recurrence,
            original_start_time,
            sequence: self.sequence as i64,
            color_id,
            conference_data: conference_url.map(conference_data_from_url),
            ..Default::default()
        }
    }
}

fn conference_data_from_url(url: &str) -> google_calendar::types::ConferenceData {
    google_calendar::types::ConferenceData {
        conference_id: String::new(),
        conference_solution: Some(google_calendar::types::ConferenceSolution {
            icon_uri: String::new(),
            key: Some(google_calendar::types::ConferenceSolutionKey {
                type_: "hangoutsMeet".to_string(),
            }),
            name: "Google Meet".to_string(),
        }),
        create_request: None,
        entry_points: vec![google_calendar::types::EntryPoint {
            access_code: String::new(),
            entry_point_features: Vec::new(),
            entry_point_type: "video".to_string(),
            label: url.to_string(),
            meeting_code: String::new(),
            passcode: String::new(),
            password: String::new(),
            pin: String::new(),
            region_code: String::new(),
            uri: url.to_string(),
        }],
        notes: String::new(),
        parameters: None,
        signature: String::new(),
    }
}

fn attendee_to_google(attendee: &Attendee) -> google_calendar::types::EventAttendee {
    google_calendar::types::EventAttendee {
        email: attendee.email.clone(),
        display_name: attendee.name.clone().unwrap_or_default(),
        response_status: attendee
            .status
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
        EventTime::DateTimeZoned { datetime, tzid } => {
            // Convert wall clock time in the given timezone to actual UTC instant
            let utc_dt = if let Ok(tz) = tzid.parse::<chrono_tz::Tz>() {
                datetime
                    .and_local_timezone(tz)
                    .single()
                    .map(|zoned| zoned.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|| datetime.and_utc())
            } else {
                datetime.and_utc()
            };
            google_calendar::types::EventDateTime {
                date: None,
                date_time: Some(utc_dt),
                time_zone: tzid.clone(),
            }
        }
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

pub(crate) fn participation_status_to_google(status: ParticipationStatus) -> &'static str {
    match status {
        ParticipationStatus::Accepted => "accepted",
        ParticipationStatus::Declined => "declined",
        ParticipationStatus::Tentative => "tentative",
        ParticipationStatus::NeedsAction => "needsAction",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use caldir_core::{Event, EventTime, Reminder, Visibility};
    use chrono::NaiveDate;

    fn sample_event() -> Event {
        Event::new(
            "Test",
            EventTime::DateTimeFloating(
                NaiveDate::from_ymd_opt(2026, 1, 1)
                    .unwrap()
                    .and_hms_opt(12, 0, 0)
                    .unwrap(),
            ),
        )
    }

    // Google's API rejects reminder overrides with `minutes: 0` ("Missing
    // override reminder minutes"), because the `google-calendar` crate strips
    // zero-valued integers from the serialized JSON. A reminder that fires at
    // event start must not be sent as an override.
    #[test]
    fn zero_minute_reminder_is_stripped_to_avoid_google_400() {
        let mut event = sample_event();
        event.reminders = vec![Reminder {
            minutes_before_start: 0,
        }];

        let google = event.to_google();
        let reminders = google.reminders.expect("reminders always set");

        assert!(
            reminders.overrides.is_empty(),
            "expected 0-minute reminder to be filtered out, got {:?}",
            reminders.overrides
        );
        // With no surviving overrides, fall back to the calendar default
        // rather than sending an empty `overrides` array that would clear it.
        assert!(reminders.use_default);
    }

    #[test]
    fn zero_minute_reminder_is_stripped_but_other_reminders_pass_through() {
        let mut event = sample_event();
        event.reminders = vec![
            Reminder {
                minutes_before_start: 0,
            },
            Reminder {
                minutes_before_start: 15,
            },
        ];

        let google = event.to_google();
        let reminders = google.reminders.expect("non-empty reminders");

        assert_eq!(reminders.overrides.len(), 1);
        assert_eq!(reminders.overrides[0].minutes, 15);
        assert!(!reminders.use_default);
    }

    #[test]
    fn nonzero_reminder_is_sent_to_google() {
        let mut event = sample_event();
        event.reminders = vec![Reminder {
            minutes_before_start: 30,
        }];

        let google = event.to_google();
        let reminders = google.reminders.expect("non-empty reminders");

        assert_eq!(reminders.overrides.len(), 1);
        assert_eq!(reminders.overrides[0].minutes, 30);
        assert_eq!(reminders.overrides[0].method, "popup");
        assert!(!reminders.use_default);
    }

    // Local files without VALARMs must push as `useDefault: true` so that
    // we don't silently strip Google's calendar-level default reminders on push.
    #[test]
    fn empty_reminders_sends_use_default_true() {
        let event = sample_event();
        assert!(event.reminders.is_empty());

        let google = event.to_google();
        let reminders = google.reminders.expect("reminders always set");

        assert!(reminders.use_default);
        assert!(reminders.overrides.is_empty());
    }

    #[test]
    fn private_visibility_serializes_as_private() {
        let mut event = sample_event();
        event.visibility = Some(Visibility::Private);

        let google = event.to_google();

        assert_eq!(google.visibility, "private");
    }

    #[test]
    fn confidential_visibility_serializes_as_confidential() {
        let mut event = sample_event();
        event.visibility = Some(Visibility::Confidential);

        let google = event.to_google();

        assert_eq!(google.visibility, "confidential");
    }

    // Unspecified visibility maps to Google's "default" ("inherit calendar
    // visibility"), avoiding pinning the event when the calendar itself has a
    // different default.
    #[test]
    fn unspecified_visibility_serializes_as_default() {
        let mut event = sample_event();
        event.visibility = None;

        let google = event.to_google();

        assert_eq!(google.visibility, "default");
    }

    // An explicit Some(Public) is distinct from unspecified and is pinned as
    // "public" so the distinction round-trips.
    #[test]
    fn public_visibility_serializes_as_public() {
        let mut event = sample_event();
        event.visibility = Some(Visibility::Public);

        let google = event.to_google();

        assert_eq!(google.visibility, "public");
    }
}

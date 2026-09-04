use caldir_core::{
    Attendee, Availability, Event, EventTime, ParticipationStatus, Recurrence, RecurrenceId,
    Status, Visibility,
};

use crate::constants::{
    GOOGLE_COLOR_ID_PROPERTY, GOOGLE_DEFAULT_REMINDERS_PROPERTY, GOOGLE_EVENT_ID_PROPERTY,
};

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

        // VALARMs always win. Without them: marker means inherit Google's
        // calendar defaults, otherwise no reminders.
        let use_default = valid_reminders.is_empty() && uses_default_reminders(self);
        let reminders = Some(google_calendar::types::Reminders {
            overrides: valid_reminders,
            use_default,
        });

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
            .x_property(GOOGLE_EVENT_ID_PROPERTY)
            .unwrap_or_default()
            .to_string();

        let color_id = self
            .x_property(GOOGLE_COLOR_ID_PROPERTY)
            .unwrap_or_default()
            .to_string();

        let conference_data = self.x_property("X-GOOGLE-CONFERENCE").and_then(|url| {
            google_meet_conference_data(url, &format!("{}-{}", self.uid.as_str(), self.sequence))
        });

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
            conference_data,
            ..Default::default()
        }
    }
}

fn uses_default_reminders(event: &Event) -> bool {
    event
        .x_property(GOOGLE_DEFAULT_REMINDERS_PROPERTY)
        .is_some_and(|value| value.eq_ignore_ascii_case("TRUE"))
}

fn google_meet_conference_data(
    url: &str,
    request_id: &str,
) -> Option<google_calendar::types::ConferenceData> {
    if url.trim().is_empty() {
        return Some(google_calendar::types::ConferenceData {
            conference_id: String::new(),
            conference_solution: None,
            entry_points: Vec::new(),
            create_request: Some(google_calendar::types::CreateConferenceRequest {
                conference_solution_key: Some(google_calendar::types::ConferenceSolutionKey {
                    type_: "hangoutsMeet".to_string(),
                }),
                request_id: request_id.to_string(),
                status: None,
            }),
            notes: String::new(),
            parameters: None,
            signature: String::new(),
        });
    }

    let parsed = url::Url::parse(url).ok()?;
    if parsed.scheme() != "https" || parsed.host_str() != Some("meet.google.com") {
        return None;
    }

    let mut path_segments = parsed
        .path_segments()?
        .filter(|segment| !segment.is_empty());
    let conference_id = path_segments.next()?;
    if path_segments.next().is_some() {
        return None;
    }

    Some(google_calendar::types::ConferenceData {
        conference_id: conference_id.to_string(),
        conference_solution: Some(google_calendar::types::ConferenceSolution {
            icon_uri: String::new(),
            key: Some(google_calendar::types::ConferenceSolutionKey {
                type_: "hangoutsMeet".to_string(),
            }),
            name: String::new(),
        }),
        entry_points: vec![google_calendar::types::EntryPoint {
            access_code: String::new(),
            entry_point_features: Vec::new(),
            entry_point_type: "video".to_string(),
            label: String::new(),
            meeting_code: String::new(),
            passcode: String::new(),
            password: String::new(),
            pin: String::new(),
            region_code: String::new(),
            uri: url.to_string(),
        }],
        create_request: None,
        notes: String::new(),
        parameters: None,
        signature: String::new(),
    })
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
    use caldir_core::{Event, EventTime, Reminder, Visibility, XProperty};
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
        // With no surviving overrides or marker, send no reminders.
        assert!(!reminders.use_default);
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

    #[test]
    fn no_valarm_and_no_marker_sends_no_reminders() {
        let event = sample_event();
        assert!(event.reminders.is_empty());

        let google = event.to_google();
        let reminders = google.reminders.expect("reminders always set");

        assert!(!reminders.use_default);
        assert!(reminders.overrides.is_empty());
    }

    #[test]
    fn default_reminders_marker_sends_use_default_true() {
        let mut event = sample_event();
        event.x_properties = vec![XProperty::new(GOOGLE_DEFAULT_REMINDERS_PROPERTY, "TRUE")];

        let reminders = event.to_google().reminders.expect("reminders always set");

        assert!(reminders.use_default);
        assert!(reminders.overrides.is_empty());
    }

    #[test]
    fn marker_false_sends_no_reminders() {
        let mut event = sample_event();
        event.x_properties = vec![XProperty::new(GOOGLE_DEFAULT_REMINDERS_PROPERTY, "FALSE")];

        let reminders = event.to_google().reminders.expect("reminders always set");

        assert!(!reminders.use_default);
        assert!(reminders.overrides.is_empty());
    }

    #[test]
    fn marker_value_is_case_insensitive() {
        let mut event = sample_event();
        event.x_properties = vec![XProperty::new(GOOGLE_DEFAULT_REMINDERS_PROPERTY, "true")];

        let reminders = event.to_google().reminders.expect("reminders always set");

        assert!(reminders.use_default);
    }

    #[test]
    fn valarms_win_over_default_reminders_marker() {
        let mut event = sample_event();
        event.reminders = vec![Reminder {
            minutes_before_start: 20,
        }];
        event.x_properties = vec![XProperty::new(GOOGLE_DEFAULT_REMINDERS_PROPERTY, "TRUE")];

        let reminders = event.to_google().reminders.expect("reminders always set");

        assert!(!reminders.use_default);
        assert_eq!(reminders.overrides.len(), 1);
        assert_eq!(reminders.overrides[0].minutes, 20);
    }

    #[test]
    fn no_reminders_serialise_with_explicit_use_default_false() {
        let reminders = google_calendar::types::Reminders {
            use_default: false,
            overrides: vec![],
        };

        assert_eq!(
            serde_json::to_value(reminders).unwrap(),
            serde_json::json!({"useDefault": false})
        );
    }

    #[test]
    fn google_meet_x_property_populates_conference_data() {
        let url = "https://meet.google.com/abc-defg-hij";
        let mut event = sample_event();
        event.x_properties = vec![XProperty::new("X-GOOGLE-CONFERENCE", url)];

        let google = event.to_google();
        let conference = google
            .conference_data
            .expect("Google Meet conference data should be preserved");

        assert_eq!(conference.conference_id, "abc-defg-hij");
        assert_eq!(
            conference
                .conference_solution
                .and_then(|solution| solution.key)
                .map(|key| key.type_),
            Some("hangoutsMeet".to_string())
        );
        assert_eq!(conference.entry_points.len(), 1);
        assert_eq!(conference.entry_points[0].entry_point_type, "video");
        assert_eq!(conference.entry_points[0].uri, url);
        assert!(conference.create_request.is_none());
    }

    #[test]
    fn empty_google_meet_x_property_requests_conference_creation() {
        let mut event = sample_event();
        event.sequence = 3;
        event.x_properties = vec![XProperty::new("X-GOOGLE-CONFERENCE", "")];
        let expected_request_id = format!("{}-3", event.uid.as_str());

        let google = event.to_google();
        let conference = google
            .conference_data
            .expect("empty Google Meet property should request a conference");
        let request = conference
            .create_request
            .expect("conference creation request should be populated");

        assert!(conference.entry_points.is_empty());
        assert!(conference.conference_solution.is_none());
        assert_eq!(request.request_id, expected_request_id);
        assert_eq!(
            request
                .conference_solution_key
                .map(|key| key.type_)
                .as_deref(),
            Some("hangoutsMeet")
        );
    }

    #[test]
    fn whitespace_google_meet_x_property_requests_conference_creation() {
        let mut event = sample_event();
        event.x_properties = vec![XProperty::new("X-GOOGLE-CONFERENCE", " \t ")];

        let conference = event
            .to_google()
            .conference_data
            .expect("whitespace Google Meet property should request a conference");

        assert!(conference.create_request.is_some());
        assert!(conference.entry_points.is_empty());
    }

    #[test]
    fn non_google_meet_x_property_does_not_populate_conference_data() {
        let mut event = sample_event();
        event.x_properties = vec![XProperty::new(
            "X-GOOGLE-CONFERENCE",
            "https://zoom.us/j/123456789",
        )];

        assert!(event.to_google().conference_data.is_none());
    }

    #[test]
    fn event_without_conference_x_property_has_no_conference_data() {
        assert!(sample_event().to_google().conference_data.is_none());
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

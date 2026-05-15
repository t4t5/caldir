use anyhow::Result;
use caldir_core::{
    Attendee, Event, EventTime, EventUid, Organizer, ParticipationStatus, Recurrence, RecurrenceId,
    Reminder, Status, Transparency, Visibility, XProperty,
};

use crate::constants::{PROVIDER_COLOR_ID_PROPERTY, PROVIDER_EVENT_ID_PROPERTY};

pub trait FromGoogle {
    fn from_google(event: google_calendar::types::Event) -> Result<Self>
    where
        Self: Sized;
}

impl FromGoogle for Event {
    fn from_google(event: google_calendar::types::Event) -> Result<Self> {
        let start = google_dt_to_event_time(event.start.as_ref()).ok_or_else(|| {
            anyhow::anyhow!("Event has no start time ({})", describe_event(&event))
        })?;

        let end = google_dt_to_event_time(event.end.as_ref())
            .ok_or_else(|| anyhow::anyhow!("Event has no end time ({})", describe_event(&event)))?;

        let status = match event.status.as_str() {
            "tentative" => Status::Tentative,
            "cancelled" => Status::Cancelled,
            _ => Status::Confirmed,
        };

        let recurrence = parse_google_recurrence(&event.recurrence);

        let recurrence_id = google_dt_to_event_time(event.original_start_time.as_ref())
            .map(RecurrenceId::from_event_time);

        // Note: when `reminders.useDefault: true` and overrides is empty, we
        // intentionally leave reminders empty here — the local file has no
        // VALARM, and `to_google` reads "no VALARM" as "inherit Google's
        // calendar default reminders" on push.
        let reminders: Vec<Reminder> = if let Some(ref rem) = event.reminders {
            rem.overrides
                .iter()
                .map(|r| Reminder {
                    minutes_before_start: r.minutes,
                })
                .collect()
        } else {
            Vec::new()
        };

        let transparency = if event.transparency == "transparent" {
            Transparency::Transparent
        } else {
            Transparency::Opaque
        };

        // Google omits `visibility` (or sends "default") when the event
        // inherits the calendar's default visibility — treat that as PUBLIC
        // per RFC 5545, matching the ICS-side default.
        let visibility = match event.visibility.as_str() {
            "private" => Visibility::Private,
            "confidential" => Visibility::Confidential,
            _ => Visibility::Public,
        };

        let organizer = event.organizer.as_ref().map(|o| Organizer {
            email: o.email.clone(),
            name: if o.display_name.is_empty() {
                None
            } else {
                Some(o.display_name.clone())
            },
        });

        let attendees: Vec<Attendee> = event
            .attendees
            .iter()
            .map(|a| Attendee {
                email: a.email.clone(),
                name: if a.display_name.is_empty() {
                    None
                } else {
                    Some(a.display_name.clone())
                },
                status: google_to_participation_status(&a.response_status),
            })
            .collect();

        let conference_url = event.conference_data.as_ref().and_then(|cd| {
            cd.entry_points
                .iter()
                .find(|ep| ep.entry_point_type == "video")
                .map(|ep| ep.uri.clone())
        });

        let mut x_properties = Vec::new();
        // Store Google's event ID for API calls (updates, deletes)
        x_properties.push(XProperty::new(PROVIDER_EVENT_ID_PROPERTY, event.id));
        if let Some(ref url) = conference_url {
            x_properties.push(XProperty::new("X-GOOGLE-CONFERENCE", url));
        }
        if !event.color_id.is_empty() {
            x_properties.push(XProperty::new(PROVIDER_COLOR_ID_PROPERTY, event.color_id));
        }

        Ok(Event {
            uid: EventUid::new(event.i_cal_uid),
            summary: if event.summary.is_empty() {
                None
            } else {
                Some(event.summary)
            },
            description: if event.description.is_empty() {
                None
            } else {
                Some(event.description)
            },
            location: if event.location.is_empty() {
                None
            } else {
                Some(event.location)
            },
            start,
            end: Some(end),
            status,
            transparency,
            visibility,
            recurrence,
            recurrence_id,
            last_modified: event.updated,
            sequence: event.sequence as i32,
            organizer,
            attendees,
            reminders,
            // Also mirrored in X-GOOGLE-CONFERENCE — kept here so local files
            // round-trip stably (Google's API has no writable URL field).
            url: conference_url,
            x_properties,
        })
    }
}

/// Parse Google's recurrence Vec<String> into a typed Recurrence.
///
/// Google returns entries like:
/// - `"RRULE:FREQ=WEEKLY;BYDAY=MO"`
/// - `"EXDATE;TZID=America/New_York:20240108T100000"`
/// - `"EXDATE:20240108T100000Z"`
fn parse_google_recurrence(entries: &[String]) -> Option<Recurrence> {
    let rrule = entries
        .iter()
        .find(|s| s.starts_with("RRULE:"))
        .map(|s| s.strip_prefix("RRULE:").unwrap().to_string())?;

    let exdates: Vec<EventTime> = entries
        .iter()
        .filter(|s| s.starts_with("EXDATE"))
        .filter_map(|s| {
            // Format: "EXDATE;TZID=America/New_York:20240108T100000" or "EXDATE:20240108T100000Z"
            let (params_part, value) = s.split_once(':')?;
            let params_str = params_part.strip_prefix("EXDATE").unwrap_or("");
            let params_str = params_str.strip_prefix(';').unwrap_or(params_str);

            let tzid = params_str
                .split(';')
                .find_map(|p| p.strip_prefix("TZID=").map(|v| v.to_string()));

            let is_date = params_str.split(';').any(|p| p == "VALUE=DATE");

            Some(
                value
                    .split(',')
                    .filter_map(|s| parse_google_exdate(s.trim(), &tzid, is_date))
                    .collect::<Vec<_>>(),
            )
        })
        .flatten()
        .collect();

    Some(Recurrence {
        rrule,
        exdates,
        rdates: Vec::new(),
    })
}

/// Parse a single EXDATE value string into an EventTime.
fn parse_google_exdate(s: &str, tzid: &Option<String>, is_date: bool) -> Option<EventTime> {
    if is_date {
        chrono::NaiveDate::parse_from_str(s, "%Y%m%d")
            .ok()
            .map(EventTime::Date)
    } else if let Some(tz) = tzid {
        chrono::NaiveDateTime::parse_from_str(s, "%Y%m%dT%H%M%S")
            .ok()
            .map(|dt| EventTime::DateTimeZoned {
                datetime: dt,
                tzid: tz.clone(),
            })
    } else if s.ends_with('Z') {
        let s = s.trim_end_matches('Z');
        chrono::NaiveDateTime::parse_from_str(s, "%Y%m%dT%H%M%S")
            .ok()
            .map(|dt| EventTime::DateTimeUtc(dt.and_utc()))
    } else {
        chrono::NaiveDateTime::parse_from_str(s, "%Y%m%dT%H%M%S")
            .ok()
            .map(EventTime::DateTimeFloating)
    }
}

fn describe_event(event: &google_calendar::types::Event) -> String {
    serde_json::to_string(event).unwrap_or_else(|_| format!("id={}", event.id))
}

/// Convert a Google `EventDateTime` (used for start/end/originalStartTime) into our `EventTime`.
/// Returns `None` if the field is absent or carries neither a `dateTime` nor a `date`.
pub fn google_dt_to_event_time(
    dt: Option<&google_calendar::types::EventDateTime>,
) -> Option<EventTime> {
    let dt = dt?;
    if let Some(d) = dt.date_time {
        Some(utc_to_zoned(d, &dt.time_zone))
    } else {
        dt.date.map(EventTime::Date)
    }
}

/// Convert a UTC datetime to a zoned datetime using Google's timezone field.
/// Falls back to DateTimeUtc if no timezone is provided or the timezone is invalid.
fn utc_to_zoned(dt: chrono::DateTime<chrono::Utc>, time_zone: &str) -> EventTime {
    if !time_zone.is_empty()
        && let Ok(tz) = time_zone.parse::<chrono_tz::Tz>()
    {
        let zoned = dt.with_timezone(&tz);
        return EventTime::DateTimeZoned {
            datetime: zoned.naive_local(),
            tzid: time_zone.to_string(),
        };
    }
    EventTime::DateTimeUtc(dt)
}

fn google_to_participation_status(google_status: &str) -> Option<ParticipationStatus> {
    match google_status {
        "accepted" => Some(ParticipationStatus::Accepted),
        "declined" => Some(ParticipationStatus::Declined),
        "tentative" => Some(ParticipationStatus::Tentative),
        "needsAction" => Some(ParticipationStatus::NeedsAction),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use google_calendar::types as g;

    fn empty_event() -> g::Event {
        serde_json::from_value(serde_json::json!({})).unwrap()
    }

    fn empty_dt() -> g::EventDateTime {
        serde_json::from_value(serde_json::json!({})).unwrap()
    }

    fn minimal_event() -> g::Event {
        g::Event {
            id: "id1".into(),
            i_cal_uid: "uid1@google.com".into(),
            start: Some(g::EventDateTime {
                date_time: Some("2026-01-16T15:00:00Z".parse().unwrap()),
                time_zone: "Europe/Oslo".into(),
                ..empty_dt()
            }),
            end: Some(g::EventDateTime {
                date_time: Some("2026-01-16T15:45:00Z".parse().unwrap()),
                time_zone: "Europe/Oslo".into(),
                ..empty_dt()
            }),
            ..empty_event()
        }
    }

    #[test]
    fn confirmed_status_maps_to_confirmed() {
        let mut ge = minimal_event();
        ge.status = "confirmed".into();

        let event = Event::from_google(ge).unwrap();

        assert_eq!(event.status, Status::Confirmed);
    }

    #[test]
    fn empty_status_maps_to_confirmed() {
        // Google omits `status` for events that haven't been explicitly set;
        // RFC 5545 says CONFIRMED is the default in that case.
        let mut ge = minimal_event();
        ge.status = String::new();

        let event = Event::from_google(ge).unwrap();

        assert_eq!(event.status, Status::Confirmed);
    }

    #[test]
    fn tentative_status_maps_to_tentative() {
        let mut ge = minimal_event();
        ge.status = "tentative".into();

        let event = Event::from_google(ge).unwrap();

        assert_eq!(event.status, Status::Tentative);
    }

    #[test]
    fn cancelled_status_maps_to_cancelled() {
        let mut ge = minimal_event();
        ge.status = "cancelled".into();

        let event = Event::from_google(ge).unwrap();

        assert_eq!(event.status, Status::Cancelled);
    }

    #[test]
    fn opaque_transparency_maps_to_opaque() {
        let mut ge = minimal_event();
        ge.transparency = "opaque".into();

        let event = Event::from_google(ge).unwrap();

        assert_eq!(event.transparency, Transparency::Opaque);
    }

    #[test]
    fn empty_transparency_maps_to_opaque() {
        // Google omits `transparency` for events using the default availability;
        // RFC 5545 says OPAQUE is the default in that case.
        let mut ge = minimal_event();
        ge.transparency = String::new();

        let event = Event::from_google(ge).unwrap();

        assert_eq!(event.transparency, Transparency::Opaque);
    }

    #[test]
    fn transparent_transparency_maps_to_transparent() {
        let mut ge = minimal_event();
        ge.transparency = "transparent".into();

        let event = Event::from_google(ge).unwrap();

        assert_eq!(event.transparency, Transparency::Transparent);
    }

    #[test]
    fn private_visibility_maps_to_private() {
        let mut ge = minimal_event();
        ge.visibility = "private".into();

        let event = Event::from_google(ge).unwrap();

        assert_eq!(event.visibility, Visibility::Private);
    }

    #[test]
    fn confidential_visibility_maps_to_confidential() {
        let mut ge = minimal_event();
        ge.visibility = "confidential".into();

        let event = Event::from_google(ge).unwrap();

        assert_eq!(event.visibility, Visibility::Confidential);
    }

    #[test]
    fn default_visibility_maps_to_public() {
        // "default" means "inherit from calendar"; treat as PUBLIC per RFC 5545.
        let mut ge = minimal_event();
        ge.visibility = "default".into();

        let event = Event::from_google(ge).unwrap();

        assert_eq!(event.visibility, Visibility::Public);
    }

    #[test]
    fn empty_visibility_maps_to_public() {
        // Google omits `visibility` for events using the calendar default.
        let mut ge = minimal_event();
        ge.visibility = String::new();

        let event = Event::from_google(ge).unwrap();

        assert_eq!(event.visibility, Visibility::Public);
    }

    #[test]
    fn conference_url_populates_url_and_x_google_conference() {
        // Google has no writable URL field, so we mirror the conference URL
        // into Event.url for round-trip stability with legacy local files.
        let mut ge = minimal_event();
        ge.conference_data = Some(
            serde_json::from_value(serde_json::json!({
                "entryPoints": [
                    {"entryPointType": "video", "uri": "https://meet.google.com/abc-def-ghi"}
                ]
            }))
            .unwrap(),
        );

        let event = Event::from_google(ge).unwrap();

        assert_eq!(
            event.url.as_deref(),
            Some("https://meet.google.com/abc-def-ghi")
        );
        assert_eq!(
            event.x_property("X-GOOGLE-CONFERENCE"),
            Some("https://meet.google.com/abc-def-ghi")
        );
    }

    #[test]
    fn no_conference_data_leaves_url_none() {
        let event = Event::from_google(minimal_event()).unwrap();

        assert_eq!(event.url, None);
    }

    // `useDefault: true` means "inherit the calendar's default reminders". We
    // deliberately don't expand those into explicit VALARMs locally: round-
    // tripping back via `to_google` would then send them as overrides and
    // pin the event to today's defaults, even if the calendar default later
    // changes. The empty-reminders state on disk is what makes the next push
    // emit `useDefault: true` again.
    #[test]
    fn use_default_reminders_produces_empty_reminders() {
        let mut ge = minimal_event();
        ge.reminders = Some(g::Reminders {
            use_default: true,
            overrides: vec![],
        });

        let event = Event::from_google(ge).unwrap();

        assert!(event.reminders.is_empty());
    }

    #[test]
    fn explicit_overrides_become_reminders() {
        let mut ge = minimal_event();
        ge.reminders = Some(g::Reminders {
            use_default: false,
            overrides: vec![g::EventReminder {
                method: "popup".into(),
                minutes: 10,
            }],
        });

        let event = Event::from_google(ge).unwrap();

        assert_eq!(event.reminders.len(), 1);
        assert_eq!(event.reminders[0].minutes_before_start, 10);
    }
}

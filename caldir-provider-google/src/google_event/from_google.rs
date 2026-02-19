use anyhow::{Result, bail};
use caldir_core::event::{
    Attendee, Event, EventStatus, EventTime, ParticipationStatus, Recurrence, Reminder, Reminders,
    Transparency,
};

pub trait FromGoogle {
    fn from_google(event: google_calendar::types::Event) -> Result<Self>
    where
        Self: Sized;
}

impl FromGoogle for Event {
    fn from_google(event: google_calendar::types::Event) -> Result<Self> {
        let start = if let Some(ref start) = event.start {
            if let Some(dt) = start.date_time {
                EventTime::DateTimeUtc(dt)
            } else if let Some(d) = start.date {
                EventTime::Date(d)
            } else {
                bail!("Event has no start time");
            }
        } else {
            bail!("Event has no start time");
        };

        let end = if let Some(ref end) = event.end {
            if let Some(dt) = end.date_time {
                EventTime::DateTimeUtc(dt)
            } else if let Some(d) = end.date {
                EventTime::Date(d)
            } else {
                bail!("Event has no end time");
            }
        } else {
            bail!("Event has no end time");
        };

        let status = match event.status.as_str() {
            "tentative" => EventStatus::Tentative,
            "cancelled" => EventStatus::Cancelled,
            _ => EventStatus::Confirmed,
        };

        let recurrence = parse_google_recurrence(&event.recurrence);

        let recurrence_id = if let Some(ref orig) = event.original_start_time {
            if let Some(dt) = orig.date_time {
                Some(EventTime::DateTimeUtc(dt))
            } else {
                orig.date.map(EventTime::Date)
            }
        } else {
            None
        };

        let reminders = if let Some(ref rem) = event.reminders {
            rem.overrides
                .iter()
                .map(|r| Reminder { minutes: r.minutes })
                .collect()
        } else {
            Reminders(vec![])
        };

        let transparency = if event.transparency == "transparent" {
            Transparency::Transparent
        } else {
            Transparency::Opaque
        };

        let organizer = event.organizer.as_ref().map(|o| Attendee {
            name: if o.display_name.is_empty() {
                None
            } else {
                Some(o.display_name.clone())
            },
            email: o.email.clone(),
            response_status: None,
        });

        let attendees: Vec<Attendee> = event
            .attendees
            .iter()
            .map(|a| Attendee {
                name: if a.display_name.is_empty() {
                    None
                } else {
                    Some(a.display_name.clone())
                },
                email: a.email.clone(),
                response_status: google_to_participation_status(&a.response_status),
            })
            .collect();

        let conference_url = event.conference_data.as_ref().and_then(|cd| {
            cd.entry_points
                .iter()
                .find(|ep| ep.entry_point_type == "video")
                .map(|ep| ep.uri.clone())
        });

        let mut custom_properties = Vec::new();
        // Store Google's event ID for API calls (updates, deletes)
        custom_properties.push(("X-GOOGLE-EVENT-ID".to_string(), event.id));
        if let Some(ref url) = conference_url {
            custom_properties.push(("X-GOOGLE-CONFERENCE".to_string(), url.clone()));
        }

        Ok(Event {
            uid: event.i_cal_uid,
            summary: event.summary,
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
            end,
            status,
            recurrence,
            recurrence_id,
            reminders,
            transparency,
            organizer,
            attendees,
            conference_url,
            updated: event.updated,
            sequence: if event.sequence > 0 {
                Some(event.sequence)
            } else {
                None
            },
            custom_properties,
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

    Some(Recurrence { rrule, exdates })
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

fn google_to_participation_status(google_status: &str) -> Option<ParticipationStatus> {
    match google_status {
        "accepted" => Some(ParticipationStatus::Accepted),
        "declined" => Some(ParticipationStatus::Declined),
        "tentative" => Some(ParticipationStatus::Tentative),
        "needsAction" => Some(ParticipationStatus::NeedsAction),
        _ => None,
    }
}

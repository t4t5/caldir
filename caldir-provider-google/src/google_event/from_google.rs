use anyhow::{bail, Result};
use caldir_core::{
    Attendee, Event, EventStatus, EventTime, ParticipationStatus, Reminder, Transparency,
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

        let recurrence = if event.recurrence.is_empty() {
            None
        } else {
            Some(event.recurrence)
        };

        let original_start = if let Some(ref orig) = event.original_start_time {
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
            Vec::new()
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
        if let Some(ref url) = conference_url {
            custom_properties.push(("X-GOOGLE-CONFERENCE".to_string(), url.clone()));
        }

        Ok(Event {
            id: event.id,
            summary: if event.summary.is_empty() {
                "(No title)".to_string()
            } else {
                event.summary
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
            end,
            status,
            recurrence,
            original_start,
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

fn google_to_participation_status(google_status: &str) -> Option<ParticipationStatus> {
    match google_status {
        "accepted" => Some(ParticipationStatus::Accepted),
        "declined" => Some(ParticipationStatus::Declined),
        "tentative" => Some(ParticipationStatus::Tentative),
        "needsAction" => Some(ParticipationStatus::NeedsAction),
        _ => None,
    }
}

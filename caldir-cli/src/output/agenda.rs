use anyhow::Result as AnyhowResult;
use caldir_core::{Caldir, Calendar, Event, ParticipationStatus, TimeFormat};
use chrono::{DateTime, NaiveDate, Utc};

use crate::output::event::is_visible;
use crate::output::time::local_date;

pub struct AgendaView {
    pub entries: Vec<AgendaEntry>,
    pub(crate) range_start: NaiveDate,
    pub(crate) range_end: NaiveDate,
    pub(crate) time_format: TimeFormat,
}

pub struct AgendaEntry {
    pub event: Event,
    pub calendar: Option<String>,
    pub rsvp: Option<ParticipationStatus>,
}

impl AgendaView {
    pub fn collect(
        caldir: &Caldir,
        calendars: Vec<Calendar>,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> AnyhowResult<Self> {
        let range_start = from.with_timezone(&chrono::Local).date_naive();
        let range_end = to.with_timezone(&chrono::Local).date_naive();
        let mut entries = Vec::new();

        for calendar in calendars {
            let calendar_slug = calendar.slug().map(str::to_owned);
            let remote_email = calendar.remote_email().map(str::to_owned);

            for event in calendar.expanded_events_in_range(from, to)? {
                if !is_visible(&event) {
                    continue;
                }

                let rsvp = resolve_rsvp(&event, remote_email.as_deref());

                entries.push(AgendaEntry {
                    calendar: calendar_slug.clone(),
                    rsvp,
                    event,
                });
            }
        }

        entries.sort_by(|a, b| {
            local_date(&a.event.start)
                .cmp(&local_date(&b.event.start))
                .then_with(|| {
                    a.event
                        .start
                        .is_date()
                        .cmp(&b.event.start.is_date())
                        .reverse()
                })
                .then_with(|| a.event.start.to_utc().cmp(&b.event.start.to_utc()))
        });

        Ok(Self {
            time_format: caldir.config().time_format(),
            entries,
            range_start,
            range_end,
        })
    }
}

fn resolve_rsvp(event: &Event, remote_email: Option<&str>) -> Option<ParticipationStatus> {
    remote_email
        .filter(|email| event.is_invite_for(email))
        .and_then(|email| event.attendee_status(email))
}

#[cfg(test)]
mod tests {
    use super::*;
    use caldir_core::{Attendee, EventTime, Organizer};

    #[test]
    fn resolves_rsvp_only_for_invites_to_calendar_account() {
        let start = EventTime::Date(NaiveDate::from_ymd_opt(2026, 8, 14).unwrap());
        let mut event = Event::new("Invite", start);
        event.organizer = Some(Organizer::new("host@example.com"));
        let mut attendee = Attendee::new("me@example.com");
        attendee.status = Some(ParticipationStatus::Accepted);
        event.attendees.push(attendee);

        assert_eq!(
            resolve_rsvp(&event, Some("me@example.com")),
            Some(ParticipationStatus::Accepted)
        );
        assert_eq!(resolve_rsvp(&event, Some("other@example.com")), None);
        assert_eq!(resolve_rsvp(&event, None), None);
    }
}

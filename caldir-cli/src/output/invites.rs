use std::path::PathBuf;

use caldir_core::TimeFormat;
use owo_colors::OwoColorize;
use serde::{Serialize, Serializer};

use crate::output::TextRender;
use crate::output::agenda::AgendaEntry;
use crate::output::event::{format_event_line, render_participation_status};
use crate::output::time::format_date_only;

pub struct InvitesView {
    pub entries: Vec<InviteEntry>,
    pub time_format: TimeFormat,
}

/// An invite plus the file to pass to `caldir rsvp`.
pub struct InviteEntry {
    pub path: PathBuf,
    pub entry: AgendaEntry,
}

impl Serialize for InvitesView {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.entries.serialize(serializer)
    }
}

impl Serialize for InviteEntry {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        InviteJson {
            path: self.path.to_string_lossy().into_owned(),
            entry: &self.entry,
        }
        .serialize(serializer)
    }
}

/// The public `--json` schema: an agenda entry plus its file path.
#[derive(Serialize)]
struct InviteJson<'a> {
    path: String,
    #[serde(flatten)]
    entry: &'a AgendaEntry,
}

impl TextRender for InvitesView {
    fn to_text(&self) -> String {
        if self.entries.is_empty() {
            return "No pending invites.".dimmed().to_string();
        }

        let mut lines = Vec::new();
        let mut current_date: Option<String> = None;

        for invite in &self.entries {
            let event = &invite.entry.event;
            let date_label = format_date_only(&event.start);
            if current_date.as_ref() != Some(&date_label) {
                if current_date.is_some() {
                    lines.push(String::new());
                }
                lines.push(date_label.bold().to_string());
                current_date = Some(date_label);
            }

            let status_suffix = invite
                .entry
                .rsvp
                .map(|status| format!(" ({})", render_participation_status(status)))
                .unwrap_or_default();
            lines.push(format_event_line(
                event,
                invite
                    .entry
                    .calendar
                    .as_deref()
                    .unwrap_or("(Unknown calendar)"),
                &status_suffix,
                self.time_format,
            ));

            if let Some(organizer) = event.organizer.as_ref().filter(|o| !o.email.is_empty()) {
                lines.push(format!(
                    "       {} {}",
                    "from:".dimmed(),
                    organizer.email.dimmed()
                ));
            }
        }

        lines.push(String::new());
        lines.push("Respond with: caldir rsvp".to_string());
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::Output;
    use caldir_core::{Attendee, Event, EventTime, EventUid, Organizer, ParticipationStatus};
    use chrono::NaiveDate;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    fn invite() -> InviteEntry {
        let start = EventTime::Date(NaiveDate::from_ymd_opt(2023, 1, 4).unwrap());
        let mut event = Event::new("Planning", start);
        event.uid = EventUid::new("planning@caldir");
        event.organizer = Some(Organizer::new("host@example.com"));
        let mut attendee = Attendee::new("me@example.com");
        attendee.status = Some(ParticipationStatus::NeedsAction);
        event.attendees.push(attendee);

        InviteEntry {
            path: PathBuf::from("/tmp/work/2023-01-04__planning.ics"),
            entry: AgendaEntry {
                event,
                calendar: Some("work".to_string()),
                rsvp: Some(ParticipationStatus::NeedsAction),
            },
        }
    }

    fn view(entries: Vec<InviteEntry>) -> InvitesView {
        InvitesView {
            entries,
            time_format: TimeFormat::H24,
        }
    }

    #[test]
    fn empty_view_renders_text_and_json() {
        let view = view(Vec::new());

        assert!(view.to_text().contains("No pending invites."));
        assert_eq!(view.to_json(), json!([]));
    }

    #[test]
    fn json_adds_path_to_agenda_fields() {
        let json = view(vec![invite()]).to_json();
        let entry = &json[0];

        assert_eq!(entry["path"], "/tmp/work/2023-01-04__planning.ics");
        assert_eq!(entry["uid"], "planning@caldir");
        assert_eq!(entry["instance_id"], "planning@caldir");
        assert_eq!(entry["calendar"], "work");
        assert_eq!(entry["title"], "Planning");
        assert_eq!(entry["start"], "2023-01-04");
        assert_eq!(entry["rsvp"], "needs_action");
        assert_eq!(entry["organizer"]["email"], "host@example.com");
        assert_eq!(entry["attendees"][0]["status"], "needs_action");
    }

    #[test]
    fn text_groups_by_date_and_shows_organizer() {
        let text = view(vec![invite()]).to_text();

        assert!(text.contains("Wed Jan 4 2023"));
        assert!(text.contains("all-day Planning"));
        assert!(text.contains("host@example.com"));
        assert!(text.ends_with("\nRespond with: caldir rsvp"));
    }
}

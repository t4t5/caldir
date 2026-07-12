#[cfg(test)]
use anyhow::Result;
use owo_colors::OwoColorize;
#[cfg(test)]
use std::io::Write;

use crate::commands::agenda_view::{AgendaEvent, AgendaView};
use crate::output::TextRender;
use crate::render::time::{format_date_label, format_time_only};

impl TextRender for AgendaView {
    fn to_text(&self) -> String {
        if self.days.is_empty() {
            return "No events found".dimmed().to_string();
        }

        let mut lines = Vec::new();

        for (index, day) in self.days.iter().enumerate() {
            if index > 0 {
                lines.push(String::new());
            }

            lines.push(format_date_label(day.date).bold().to_string());

            for event in &day.events {
                lines.push(format_agenda_event(event, self.time_format));
            }
        }

        lines.join("\n")
    }
}

#[cfg(test)]
fn write_text(view: &AgendaView, out: &mut impl Write) -> Result<()> {
    writeln!(out, "{}", view.to_text())?;

    Ok(())
}

fn format_agenda_event(event: &AgendaEvent, time_format: caldir_core::TimeFormat) -> String {
    let time = format_time_only(&event.start.to_event_time(), time_format);
    let calendar_slug = event
        .calendar_slug
        .as_deref()
        .unwrap_or("(Unknown calendar)");
    let cal_tag = format!("[{}]", calendar_slug);
    let summary = event.summary.as_deref().unwrap_or("(Untitled)");
    let status = event
        .invite_status
        .as_deref()
        .map(render_invite_status)
        .map(|status| format!(" ({status})"))
        .unwrap_or_default();

    format!("  {} {} {}{}", time, summary, cal_tag.dimmed(), status)
}

fn render_invite_status(status: &str) -> String {
    match status {
        "accepted" => status.green().to_string(),
        "declined" => status.red().to_string(),
        "maybe" | "pending" => status.yellow().to_string(),
        _ => status.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::agenda_view::{AgendaDay, AgendaEventTime};
    use crate::test_utils::capture;
    use caldir_core::TimeFormat;
    use chrono::NaiveDate;
    use pretty_assertions::assert_eq;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn agenda_event(summary: &str) -> AgendaEvent {
        AgendaEvent {
            calendar_slug: Some("work".to_string()),
            calendar_name: Some("Work".to_string()),
            calendar_color: Some("#ff0000".to_string()),
            id: "event-1".to_string(),
            uid: "uid-1".to_string(),
            recurrence_id: None,
            summary: Some(summary.to_string()),
            description: None,
            location: None,
            start: AgendaEventTime::Date {
                date: "2027-05-27".to_string(),
            },
            end: None,
            status: "confirmed".to_string(),
            invite_status: None,
        }
    }

    #[test]
    fn agenda_view_writes_no_events_output() {
        let view = AgendaView {
            days: Vec::new(),
            time_format: TimeFormat::H24,
        };

        let output = capture(|out| write_text(&view, out));

        assert_eq!(output, format!("{}\n", "No events found".dimmed()));
    }

    #[test]
    fn agenda_view_writes_grouped_text_output() {
        let day = date(2027, 5, 27);
        let view = AgendaView {
            days: vec![AgendaDay {
                date: day,
                events: vec![agenda_event("Trip")],
            }],
            time_format: TimeFormat::H24,
        };

        let output = capture(|out| write_text(&view, out));

        let expected = format!(
            "{}\n  all-day Trip {}\n",
            "Thu May 27 2027".bold(),
            "[work]".dimmed()
        );

        assert_eq!(output, expected);
    }
}

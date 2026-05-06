use crate::event::{Event, EventTime};
use chrono::Local;

const EMPTY_SUMMARY_SLUG: &str = "untitled";

impl Event {
    /// Generate a slug for an event based on its start time and summary.
    /// The slug is used as the filename for the event's .ics file.
    pub fn base_slug(&self) -> String {
        format!("{}__{}", self.time_slug(), self.summary_slug())
    }

    fn summary_slug(&self) -> String {
        match &self.summary {
            Some(summary) => {
                // Strip non-alphanumeric chars (e.g. emoji) before slugifying.
                // Otherwise `slug` transliterates symbols via `deunicode` (☕ → "coffee").
                let cleaned: String = summary
                    .chars()
                    .filter(|c| c.is_alphanumeric() || c.is_whitespace())
                    .collect();

                let slug = slug::slugify(cleaned);

                if slug.is_empty() {
                    EMPTY_SUMMARY_SLUG.to_string()
                } else {
                    slug
                }
            }
            None => EMPTY_SUMMARY_SLUG.to_string(),
        }
    }

    /// Always uses local time (it's the most intuitive when browsing files).
    /// If a co-worker on the other side of the world creates an event at 9am their time,
    /// my filename should show what time it is for me, not for them.
    fn time_slug(&self) -> String {
        match &self.start {
            // date only:
            EventTime::Date(date) => date.format("%Y-%m-%d").to_string(),

            // date + time:
            _ => self
                .start
                .to_local_tz(&Local)
                .format("%Y-%m-%dT%H%M")
                .to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn generates_expected_slug_for_emoji_summary() {
        let event = Event::new(
            "Café ☕️ meeting",
            EventTime::Date(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()),
        );

        assert_eq!(event.summary_slug(), "cafe-meeting");
    }

    #[test]
    fn generates_expected_slug_for_empty_summary() {
        let event = Event::new(
            "",
            EventTime::Date(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()),
        );

        assert_eq!(event.summary_slug(), "untitled");
    }

    #[test]
    fn generates_correct_base_slug_for_all_day_event() {
        let event = Event::new(
            "Test Event",
            EventTime::Date(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()),
        );

        assert_eq!(event.base_slug(), "2024-01-01__test-event");
    }

    #[test]
    fn generates_correct_base_slug_for_timed_event() {
        let event = Event::new(
            "Test Event",
            EventTime::DateTimeFloating(
                NaiveDate::from_ymd_opt(2024, 1, 1)
                    .unwrap()
                    .and_hms_opt(15, 30, 20)
                    .unwrap(),
            ),
        );

        assert_eq!(event.base_slug(), "2024-01-01T1530__test-event");
    }

    #[test]
    fn generates_untitled_base_slug_for_event_without_summary() {
        let event = Event::new(
            "",
            EventTime::Date(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()),
        );

        assert_eq!(event.base_slug(), "2024-01-01__untitled");
    }
}

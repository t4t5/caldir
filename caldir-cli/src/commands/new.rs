use anyhow::{Context, Result};
use caldir_core::caldir::Caldir;
use caldir_core::calendar::Calendar;
use caldir_core::event::{Event, EventTime};
use chrono::Duration;
use dialoguer::{Input, Select};
use owo_colors::OwoColorize;

pub fn run(
    title: Option<String>,
    start: Option<String>,
    end: Option<String>,
    duration: Option<String>,
    location: Option<String>,
    calendar_slug: Option<String>,
    calendars: Vec<Calendar>,
) -> Result<()> {
    let interactive = title.is_none() || start.is_none();

    // --- Title ---
    let title = match title {
        Some(t) => t,
        None => Input::<String>::new()
            .with_prompt("  Title")
            .interact_text()?,
    };

    // --- Start ---
    let start_time = if let Some(s) = start {
        parse_datetime(&s)?
    } else {
        prompt_with_retry("  When?", parse_datetime)?
    };

    // --- Duration / End ---
    let is_allday = matches!(start_time, EventTime::Date(_));
    let default_hint = if is_allday { "1 day" } else { "1 hour" };

    let end_time = if let Some(end_input) = end {
        parse_datetime(&end_input)?
    } else if let Some(dur_input) = duration {
        apply_duration(&start_time, &dur_input)?
    } else if interactive {
        prompt_duration(&start_time, default_hint)?
    } else {
        default_end(&start_time)
    };

    // --- Location ---
    let location = if let Some(loc) = location {
        if loc.is_empty() { None } else { Some(loc) }
    } else if interactive {
        let loc: String = Input::new()
            .with_prompt("  Where? (skip)")
            .default(String::new())
            .show_default(false)
            .interact_text()?;
        if loc.is_empty() { None } else { Some(loc) }
    } else {
        None
    };

    // --- Calendar ---
    let calendar = resolve_calendar(calendar_slug, &calendars, interactive)?;

    let event = Event::new(
        title,
        start_time,
        end_time,
        None,
        location,
        None,
        Vec::new(),
    );

    calendar.create_event(&event)?;

    if interactive {
        println!();
    }
    println!("{}", format!("  Created: {}", event.summary).green());

    Ok(())
}

/// Prompt the user with retry on parse errors.
fn prompt_with_retry<F>(prompt: &str, parse: F) -> Result<EventTime>
where
    F: Fn(&str) -> Result<EventTime>,
{
    loop {
        let input: String = Input::new().with_prompt(prompt).interact_text()?;
        match parse(&input) {
            Ok(result) => return Ok(result),
            Err(e) => {
                eprintln!("  {}", e.to_string().red());
            }
        }
    }
}

/// Prompt for duration/end with retry on parse errors.
fn prompt_duration(start: &EventTime, default_hint: &str) -> Result<EventTime> {
    loop {
        let input: String = Input::new()
            .with_prompt(format!("  How long? ({})", default_hint))
            .default(String::new())
            .show_default(false)
            .interact_text()?;
        if input.is_empty() {
            return Ok(default_end(start));
        }
        match parse_end(&input, start) {
            Ok(result) => return Ok(result),
            Err(e) => {
                eprintln!("  {}", e.to_string().red());
            }
        }
    }
}

/// Expand common abbreviations that fuzzydate doesn't handle.
fn expand_abbreviations(input: &str) -> String {
    let abbrevs = [
        ("mon", "monday"),
        ("tue", "tuesday"),
        ("tues", "tuesday"),
        ("wed", "wednesday"),
        ("thu", "thursday"),
        ("thur", "thursday"),
        ("thurs", "thursday"),
        ("fri", "friday"),
        ("sat", "saturday"),
        ("sun", "sunday"),
        ("jan", "january"),
        ("feb", "february"),
        ("mar", "march"),
        ("apr", "april"),
        ("jun", "june"),
        ("jul", "july"),
        ("aug", "august"),
        ("sep", "september"),
        ("sept", "september"),
        ("oct", "october"),
        ("nov", "november"),
        ("dec", "december"),
    ];

    let mut result = String::new();
    let lower = input.to_lowercase();

    for (i, word) in lower.split_whitespace().enumerate() {
        if i > 0 {
            result.push(' ');
        }
        let expanded = abbrevs
            .iter()
            .find(|(abbr, _)| *abbr == word)
            .map(|(_, full)| *full)
            .unwrap_or(word);
        result.push_str(expanded);
    }

    result
}

/// Parse a natural language date/time string into an EventTime.
/// If the input contains time tokens (am/pm, HH:MM, noon, midnight, "at"),
/// returns DateTimeFloating. Otherwise returns Date (all-day).
fn parse_datetime(input: &str) -> Result<EventTime> {
    let expanded = expand_abbreviations(input);
    let dt = fuzzydate::parse(&expanded)
        .map_err(|_| anyhow::anyhow!("Could not parse date/time: \"{}\"", input))?;

    if has_time_component(input) {
        Ok(EventTime::DateTimeFloating(dt))
    } else {
        Ok(EventTime::Date(dt.date()))
    }
}

/// Check if the user's input string contains time-related tokens.
fn has_time_component(input: &str) -> bool {
    let lower = input.to_lowercase();

    // Check for "noon" or "midnight"
    if lower.contains("noon") || lower.contains("midnight") {
        return true;
    }

    // Check for am/pm patterns like "6pm", "6 pm", "11am"
    let bytes = lower.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'a' || b == b'p' {
            if i + 1 < bytes.len() && bytes[i + 1] == b'm' {
                // Check that there's a digit before (possibly with space)
                if i > 0 && bytes[i - 1].is_ascii_digit() {
                    return true;
                }
                if i > 1 && bytes[i - 1] == b' ' && bytes[i - 2].is_ascii_digit() {
                    return true;
                }
            }
        }
    }

    // Check for HH:MM pattern (digit(s):digit(s))
    for (i, &b) in bytes.iter().enumerate() {
        if b == b':' {
            let has_digit_before = i > 0 && bytes[i - 1].is_ascii_digit();
            let has_digit_after = i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit();
            if has_digit_before && has_digit_after {
                return true;
            }
        }
    }

    // Check for "at" followed by a digit (e.g. "at 3", "at 15")
    if let Some(pos) = lower.find(" at ") {
        let after = &lower[pos + 4..];
        if after.starts_with(|c: char| c.is_ascii_digit()) {
            return true;
        }
    }
    // Also handle "at" at the start
    if let Some(after) = lower.strip_prefix("at ") {
        if after.starts_with(|c: char| c.is_ascii_digit()) {
            return true;
        }
    }

    false
}

/// Parse an end input — tries duration first (humantime), then date/time (fuzzydate).
fn parse_end(input: &str, start: &EventTime) -> Result<EventTime> {
    // Try as duration first
    if let Ok(event_time) = try_apply_duration(start, input) {
        return Ok(event_time);
    }

    // Strip "until"/"to" prefix and parse as datetime
    let cleaned = input
        .strip_prefix("until ")
        .or_else(|| input.strip_prefix("to "))
        .unwrap_or(input);

    parse_datetime(cleaned)
}

/// Apply a duration string to a start time.
fn apply_duration(start: &EventTime, dur_input: &str) -> Result<EventTime> {
    try_apply_duration(start, dur_input)
        .with_context(|| format!("Could not parse duration: \"{}\"", dur_input))
}

fn try_apply_duration(start: &EventTime, dur_input: &str) -> Result<EventTime> {
    let std_dur = humantime::parse_duration(dur_input).map_err(|e| anyhow::anyhow!("{}", e))?;
    let chrono_dur = Duration::from_std(std_dur).context("Duration too large")?;

    match start {
        EventTime::Date(d) => Ok(EventTime::Date(*d + chrono_dur)),
        EventTime::DateTimeFloating(dt) => Ok(EventTime::DateTimeFloating(*dt + chrono_dur)),
        EventTime::DateTimeUtc(dt) => Ok(EventTime::DateTimeUtc(*dt + chrono_dur)),
        EventTime::DateTimeZoned { datetime, tzid } => Ok(EventTime::DateTimeZoned {
            datetime: *datetime + chrono_dur,
            tzid: tzid.clone(),
        }),
    }
}

/// Default end time: +1 hour for timed events, +1 day for all-day events.
fn default_end(start: &EventTime) -> EventTime {
    match start {
        EventTime::Date(d) => EventTime::Date(*d + Duration::days(1)),
        EventTime::DateTimeFloating(dt) => EventTime::DateTimeFloating(*dt + Duration::hours(1)),
        EventTime::DateTimeUtc(dt) => EventTime::DateTimeUtc(*dt + Duration::hours(1)),
        EventTime::DateTimeZoned { datetime, tzid } => EventTime::DateTimeZoned {
            datetime: *datetime + Duration::hours(1),
            tzid: tzid.clone(),
        },
    }
}

/// Resolve which calendar to use.
fn resolve_calendar<'a>(
    slug: Option<String>,
    calendars: &'a [Calendar],
    interactive: bool,
) -> Result<&'a Calendar> {
    if let Some(slug) = slug {
        return calendars.iter().find(|c| c.slug == slug).ok_or_else(|| {
            let available: Vec<_> = calendars.iter().map(|c| c.slug.as_str()).collect();
            anyhow::anyhow!(
                "Calendar '{}' not found. Available: {}",
                slug,
                available.join(", ")
            )
        });
    }

    // If only one calendar, use it
    if calendars.len() == 1 {
        return Ok(&calendars[0]);
    }

    // Try the default calendar
    let caldir = Caldir::load()?;
    if let Some(default) = caldir.default_calendar() {
        if let Some(cal) = calendars.iter().find(|c| c.slug == default.slug) {
            return Ok(cal);
        }
    }

    // Multiple calendars, no default — ask if interactive
    if interactive {
        let items: Vec<&str> = calendars.iter().map(|c| c.slug.as_str()).collect();
        let selection = Select::new()
            .with_prompt("  Calendar")
            .items(&items)
            .default(0)
            .interact()?;
        Ok(&calendars[selection])
    } else {
        // Non-interactive with multiple calendars and no default
        let available: Vec<_> = calendars.iter().map(|c| c.slug.as_str()).collect();
        anyhow::bail!(
            "Multiple calendars found ({}). Use --calendar to specify one.",
            available.join(", ")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Datelike, NaiveDate};

    // --- has_time_component ---

    #[test]
    fn time_component_am_pm() {
        assert!(has_time_component("tomorrow 6pm"));
        assert!(has_time_component("friday 11am"));
        assert!(has_time_component("sat 3 pm"));
        assert!(has_time_component("9AM"));
    }

    #[test]
    fn time_component_colon() {
        assert!(has_time_component("tomorrow 15:00"));
        assert!(has_time_component("march 20 9:30"));
    }

    #[test]
    fn time_component_keywords() {
        assert!(has_time_component("tomorrow noon"));
        assert!(has_time_component("friday midnight"));
    }

    #[test]
    fn time_component_at_digit() {
        assert!(has_time_component("tomorrow at 3"));
        assert!(has_time_component("friday at 15"));
        assert!(has_time_component("at 9"));
    }

    #[test]
    fn no_time_component() {
        assert!(!has_time_component("tomorrow"));
        assert!(!has_time_component("march 20"));
        assert!(!has_time_component("next friday"));
        assert!(!has_time_component("saturday"));
    }

    #[test]
    fn no_false_positive_am_in_words() {
        // "am" inside words like "amsterdam" shouldn't match
        assert!(!has_time_component("december"));
        assert!(!has_time_component("camp"));
    }

    // --- expand_abbreviations ---

    #[test]
    fn expand_day_abbreviations() {
        assert_eq!(expand_abbreviations("sat 3pm"), "saturday 3pm");
        assert_eq!(expand_abbreviations("fri 9am"), "friday 9am");
        assert_eq!(expand_abbreviations("mon"), "monday");
        assert_eq!(expand_abbreviations("thu noon"), "thursday noon");
        assert_eq!(expand_abbreviations("tues 10am"), "tuesday 10am");
    }

    #[test]
    fn expand_month_abbreviations() {
        assert_eq!(expand_abbreviations("jan 20"), "january 20");
        assert_eq!(expand_abbreviations("sep 5 3pm"), "september 5 3pm");
        assert_eq!(expand_abbreviations("sept 5"), "september 5");
    }

    #[test]
    fn expand_preserves_non_abbreviations() {
        assert_eq!(expand_abbreviations("tomorrow 6pm"), "tomorrow 6pm");
        assert_eq!(expand_abbreviations("next friday"), "next friday");
    }

    // --- parse_datetime ---

    #[test]
    fn parse_datetime_timed_returns_floating() {
        let result = parse_datetime("tomorrow 3pm").unwrap();
        assert!(matches!(result, EventTime::DateTimeFloating(_)));
    }

    #[test]
    fn parse_datetime_date_only_returns_date() {
        let result = parse_datetime("tomorrow").unwrap();
        assert!(matches!(result, EventTime::Date(_)));
    }

    #[test]
    fn parse_datetime_abbreviation_works() {
        let result = parse_datetime("sat 3pm").unwrap();
        assert!(matches!(result, EventTime::DateTimeFloating(_)));
    }

    #[test]
    fn parse_datetime_absolute_date() {
        let result = parse_datetime("march 20").unwrap();
        assert!(matches!(result, EventTime::Date(_)));
        if let EventTime::Date(d) = result {
            assert_eq!(d.month(), 3);
            assert_eq!(d.day(), 20);
        }
    }

    #[test]
    fn parse_datetime_invalid_input() {
        assert!(parse_datetime("not a date at all xyz").is_err());
    }

    // --- default_end ---

    #[test]
    fn default_end_allday_adds_one_day() {
        let start = EventTime::Date(NaiveDate::from_ymd_opt(2026, 3, 20).unwrap());
        let end = default_end(&start);
        assert_eq!(
            end,
            EventTime::Date(NaiveDate::from_ymd_opt(2026, 3, 21).unwrap())
        );
    }

    #[test]
    fn default_end_timed_adds_one_hour() {
        let start = EventTime::DateTimeFloating(
            NaiveDate::from_ymd_opt(2026, 3, 20)
                .unwrap()
                .and_hms_opt(15, 0, 0)
                .unwrap(),
        );
        let end = default_end(&start);
        assert_eq!(
            end,
            EventTime::DateTimeFloating(
                NaiveDate::from_ymd_opt(2026, 3, 20)
                    .unwrap()
                    .and_hms_opt(16, 0, 0)
                    .unwrap()
            )
        );
    }

    // --- try_apply_duration ---

    #[test]
    fn apply_duration_minutes() {
        let start = EventTime::DateTimeFloating(
            NaiveDate::from_ymd_opt(2026, 3, 20)
                .unwrap()
                .and_hms_opt(15, 0, 0)
                .unwrap(),
        );
        let end = try_apply_duration(&start, "30m").unwrap();
        assert_eq!(
            end,
            EventTime::DateTimeFloating(
                NaiveDate::from_ymd_opt(2026, 3, 20)
                    .unwrap()
                    .and_hms_opt(15, 30, 0)
                    .unwrap()
            )
        );
    }

    #[test]
    fn apply_duration_hours() {
        let start = EventTime::DateTimeFloating(
            NaiveDate::from_ymd_opt(2026, 3, 20)
                .unwrap()
                .and_hms_opt(14, 0, 0)
                .unwrap(),
        );
        let end = try_apply_duration(&start, "2hours").unwrap();
        assert_eq!(
            end,
            EventTime::DateTimeFloating(
                NaiveDate::from_ymd_opt(2026, 3, 20)
                    .unwrap()
                    .and_hms_opt(16, 0, 0)
                    .unwrap()
            )
        );
    }

    #[test]
    fn apply_duration_to_allday() {
        let start = EventTime::Date(NaiveDate::from_ymd_opt(2026, 3, 20).unwrap());
        let end = try_apply_duration(&start, "3days").unwrap();
        assert_eq!(
            end,
            EventTime::Date(NaiveDate::from_ymd_opt(2026, 3, 23).unwrap())
        );
    }

    // --- parse_end ---

    #[test]
    fn parse_end_duration_string() {
        let start = EventTime::DateTimeFloating(
            NaiveDate::from_ymd_opt(2026, 3, 20)
                .unwrap()
                .and_hms_opt(15, 0, 0)
                .unwrap(),
        );
        let end = parse_end("45m", &start).unwrap();
        assert_eq!(
            end,
            EventTime::DateTimeFloating(
                NaiveDate::from_ymd_opt(2026, 3, 20)
                    .unwrap()
                    .and_hms_opt(15, 45, 0)
                    .unwrap()
            )
        );
    }

    #[test]
    fn parse_end_until_datetime() {
        let start = EventTime::DateTimeFloating(
            NaiveDate::from_ymd_opt(2026, 3, 20)
                .unwrap()
                .and_hms_opt(15, 0, 0)
                .unwrap(),
        );
        let end = parse_end("until tomorrow 5pm", &start).unwrap();
        assert!(matches!(end, EventTime::DateTimeFloating(_)));
    }
}

//! VTIMEZONE blocks derived from TZIDs on write (RFC 5545 §3.6.5).
//! Ignored on parse: the IANA TZID is the source of truth.

use crate::event::{Event, EventTime};
use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, NaiveTime, Offset, TimeZone, Weekday};
use chrono_tz::{OffsetComponents, OffsetName, Tz};
use std::collections::BTreeMap;

/// Years past the last referenced date covered for open-ended recurrences.
const RECURRING_HORIZON_YEARS: i32 = 10;

pub(crate) fn ics_blocks(event: &Event) -> String {
    let horizon = if event.recurrence.is_some() {
        RECURRING_HORIZON_YEARS
    } else {
        1
    };

    zoned_times(event)
        .into_iter()
        .filter_map(|(tzid, times)| {
            let tz: Tz = tzid.parse().ok()?;
            let first_year = times.iter().min()?.year() - 1;
            let last_year = times.iter().max()?.year() + horizon;
            Some(vtimezone(tzid, tz, first_year, last_year))
        })
        .collect()
}

fn zoned_times(event: &Event) -> BTreeMap<&str, Vec<NaiveDateTime>> {
    let recurrence_times = event
        .recurrence
        .iter()
        .flat_map(|r| r.exdates.iter().chain(&r.rdates));

    let times = [
        Some(&event.start),
        event.end.as_ref(),
        event.recurrence_id.as_ref().map(|r| r.as_event_time()),
    ]
    .into_iter()
    .flatten()
    .chain(recurrence_times);

    let mut zones: BTreeMap<&str, Vec<NaiveDateTime>> = BTreeMap::new();
    for time in times {
        if let EventTime::DateTimeZoned { datetime, tzid } = time {
            zones.entry(tzid).or_default().push(*datetime);
        }
    }
    zones
}

fn vtimezone(tzid: &str, tz: Tz, first_year: i32, last_year: i32) -> String {
    let start = year_start(first_year);
    let transitions = transitions(tz, start, year_start(last_year + 1));

    let negative_dst = std::iter::once(Observance::at(tz, start))
        .chain(transitions.iter().map(|t| t.to.clone()))
        .any(|o| o.dst < 0);
    let block = |onset, from, to: &Observance, rrule: Option<&str>| {
        observance_block(onset, from, to, to.is_daylight(negative_dst), rrule)
    };

    let mut out = format!("BEGIN:VTIMEZONE\r\nTZID:{tzid}\r\n");

    // Explicit observances for irregular early years, yearly RRULEs for a regular tail of 2+ years.
    let tail = (first_year..last_year).find_map(|split| {
        let index = transitions.partition_point(|t| t.local().year() < split);
        yearly_rules(&transitions[index..], split, last_year).map(|rules| (split, index, rules))
    });

    let (explicit, rules) = match tail {
        Some((split, index, rules)) if split == first_year => (&transitions[..index], rules),
        Some((_, index, rules)) => {
            out += &initial_observance(tz, start, negative_dst);
            (&transitions[..index], rules)
        }
        None => {
            out += &initial_observance(tz, start, negative_dst);
            (&transitions[..], Vec::new())
        }
    };

    for t in explicit {
        out += &block(t.local(), t.from, &t.to, None);
    }
    for (t, rule) in rules {
        out += &block(t.local(), t.from, &t.to, Some(&rule.to_rrule()));
    }

    out + "END:VTIMEZONE\r\n"
}

fn year_start(year: i32) -> NaiveDateTime {
    NaiveDate::from_ymd_opt(year, 1, 1)
        .expect("Jan 1 should be a valid date")
        .and_time(NaiveTime::MIN)
}

#[derive(Debug, Clone, PartialEq)]
struct Observance {
    offset: i32,
    dst: i64,
    name: Option<String>,
}

impl Observance {
    fn at(tz: Tz, utc: NaiveDateTime) -> Self {
        let offset = tz.offset_from_utc_datetime(&utc);
        Observance {
            offset: offset.fix().local_minus_utc(),
            dst: offset.dst_offset().num_seconds(),
            name: offset.abbreviation().map(str::to_string),
        }
    }

    /// With negative DST (e.g. Europe/Dublin), the non-DST observance is the summer one.
    fn is_daylight(&self, negative_dst: bool) -> bool {
        self.dst > 0 || (negative_dst && self.dst == 0)
    }
}

#[derive(Debug)]
struct Transition {
    utc: NaiveDateTime,
    from: i32,
    to: Observance,
}

impl Transition {
    /// Onset in wall-clock time before the change, as VTIMEZONE DTSTART expects.
    fn local(&self) -> NaiveDateTime {
        self.utc + Duration::seconds(self.from.into())
    }
}

fn transitions(tz: Tz, from: NaiveDateTime, to: NaiveDateTime) -> Vec<Transition> {
    let mut found = Vec::new();
    let mut lo = from;
    let mut current = Observance::at(tz, lo);

    while lo < to {
        let hi = (lo + Duration::days(1)).min(to);
        if Observance::at(tz, hi) == current {
            lo = hi;
            continue;
        }

        let (mut before, mut after) = (lo, hi);
        while after - before > Duration::seconds(1) {
            let mid = before + (after - before) / 2;
            if Observance::at(tz, mid) == current {
                before = mid;
            } else {
                after = mid;
            }
        }

        let next = Observance::at(tz, after);
        found.push(Transition {
            utc: after,
            from: current.offset,
            to: next.clone(),
        });
        current = next;
        lo = after;
    }

    found
}

#[derive(Debug, Clone, Copy)]
struct YearlyRule {
    month: u32,
    weekday: Weekday,
    /// 1-based week of month, or -1 for the last.
    nth: i8,
    time: NaiveTime,
}

impl YearlyRule {
    fn candidates(onset: NaiveDateTime) -> Vec<Self> {
        let date = onset.date();
        let rule = |nth| YearlyRule {
            month: date.month(),
            weekday: date.weekday(),
            nth,
            time: onset.time(),
        };

        // Prefer "last" when ambiguous: it's far more common in tzdb.
        let nth = rule(((date.day() - 1) / 7 + 1) as i8);
        if date.day() + 7 > days_in_month(date) {
            vec![rule(-1), nth]
        } else {
            vec![nth]
        }
    }

    fn onset(&self, year: i32) -> Option<NaiveDateTime> {
        let date = if self.nth > 0 {
            NaiveDate::from_weekday_of_month_opt(year, self.month, self.weekday, self.nth as u8)
        } else {
            NaiveDate::from_weekday_of_month_opt(year, self.month, self.weekday, 5)
                .or_else(|| NaiveDate::from_weekday_of_month_opt(year, self.month, self.weekday, 4))
        };
        Some(date?.and_time(self.time))
    }

    fn to_rrule(self) -> String {
        let day = &self.weekday.to_string()[..2];
        format!(
            "FREQ=YEARLY;BYMONTH={};BYDAY={}{}",
            self.month,
            self.nth,
            day.to_ascii_uppercase()
        )
    }
}

fn days_in_month(date: NaiveDate) -> u32 {
    let (year, month) = match date.month() {
        12 => (date.year() + 1, 1),
        m => (date.year(), m + 1),
    };
    NaiveDate::from_ymd_opt(year, month, 1)
        .expect("first of month should be a valid date")
        .pred_opt()
        .expect("day before first of month should exist")
        .day()
}

/// One rule per distinct change, each matching exactly one transition every year.
fn yearly_rules(
    transitions: &[Transition],
    first_year: i32,
    last_year: i32,
) -> Option<Vec<(&Transition, YearlyRule)>> {
    if transitions.is_empty() {
        return None;
    }

    let mut groups: Vec<Vec<&Transition>> = Vec::new();
    for t in transitions {
        match groups
            .iter_mut()
            .find(|g| g[0].from == t.from && g[0].to == t.to)
        {
            Some(group) => group.push(t),
            None => groups.push(vec![t]),
        }
    }

    groups
        .into_iter()
        .map(|group| {
            let head = group[0];
            YearlyRule::candidates(head.local())
                .into_iter()
                .find(|rule| {
                    let expected = (first_year..=last_year).map(|y| rule.onset(y));
                    group.iter().map(|t| Some(t.local())).eq(expected)
                })
                .map(|rule| (head, rule))
        })
        .collect()
}

fn initial_observance(tz: Tz, utc: NaiveDateTime, negative_dst: bool) -> String {
    let observance = Observance::at(tz, utc);
    let local = utc + Duration::seconds(observance.offset.into());
    let daylight = observance.is_daylight(negative_dst);
    observance_block(local, observance.offset, &observance, daylight, None)
}

fn observance_block(
    onset: NaiveDateTime,
    from: i32,
    to: &Observance,
    daylight: bool,
    rrule: Option<&str>,
) -> String {
    let kind = if daylight { "DAYLIGHT" } else { "STANDARD" };
    let mut out = format!(
        "BEGIN:{kind}\r\nDTSTART:{}\r\nTZOFFSETFROM:{}\r\nTZOFFSETTO:{}\r\n",
        onset.format("%Y%m%dT%H%M%S"),
        format_offset(from),
        format_offset(to.offset),
    );
    if let Some(rrule) = rrule {
        out += &format!("RRULE:{rrule}\r\n");
    }
    if let Some(name) = &to.name {
        out += &format!("TZNAME:{name}\r\n");
    }
    out + &format!("END:{kind}\r\n")
}

fn format_offset(seconds: i32) -> String {
    let sign = if seconds < 0 { '-' } else { '+' };
    let abs = seconds.unsigned_abs();
    let (hours, minutes, secs) = (abs / 3600, abs / 60 % 60, abs % 60);
    if secs == 0 {
        format!("{sign}{hours:02}{minutes:02}")
    } else {
        format!("{sign}{hours:02}{minutes:02}{secs:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Recurrence;
    use pretty_assertions::assert_eq;

    fn zoned(tzid: &str, y: i32, m: u32, d: u32, h: u32) -> EventTime {
        EventTime::DateTimeZoned {
            datetime: NaiveDate::from_ymd_opt(y, m, d)
                .unwrap()
                .and_hms_opt(h, 0, 0)
                .unwrap(),
            tzid: tzid.to_string(),
        }
    }

    #[test]
    fn emits_yearly_rules_for_regular_dst_zone() {
        let event = Event::new("Meeting", zoned("Europe/Zurich", 2026, 10, 28, 13));

        assert_eq!(
            ics_blocks(&event),
            "BEGIN:VTIMEZONE\r\n\
             TZID:Europe/Zurich\r\n\
             BEGIN:DAYLIGHT\r\n\
             DTSTART:20250330T020000\r\n\
             TZOFFSETFROM:+0100\r\n\
             TZOFFSETTO:+0200\r\n\
             RRULE:FREQ=YEARLY;BYMONTH=3;BYDAY=-1SU\r\n\
             TZNAME:CEST\r\n\
             END:DAYLIGHT\r\n\
             BEGIN:STANDARD\r\n\
             DTSTART:20251026T030000\r\n\
             TZOFFSETFROM:+0200\r\n\
             TZOFFSETTO:+0100\r\n\
             RRULE:FREQ=YEARLY;BYMONTH=10;BYDAY=-1SU\r\n\
             TZNAME:CET\r\n\
             END:STANDARD\r\n\
             END:VTIMEZONE\r\n"
        );
    }

    #[test]
    fn emits_nth_weekday_rules() {
        let event = Event::new("Meeting", zoned("America/New_York", 2026, 6, 1, 9));
        let blocks = ics_blocks(&event);

        assert!(blocks.contains("RRULE:FREQ=YEARLY;BYMONTH=3;BYDAY=2SU\r\n"));
        assert!(blocks.contains("RRULE:FREQ=YEARLY;BYMONTH=11;BYDAY=1SU\r\n"));
    }

    #[test]
    fn emits_single_standard_for_zone_without_dst() {
        let event = Event::new("Meeting", zoned("Asia/Tokyo", 2026, 6, 1, 9));

        assert_eq!(
            ics_blocks(&event),
            "BEGIN:VTIMEZONE\r\n\
             TZID:Asia/Tokyo\r\n\
             BEGIN:STANDARD\r\n\
             DTSTART:20250101T090000\r\n\
             TZOFFSETFROM:+0900\r\n\
             TZOFFSETTO:+0900\r\n\
             TZNAME:JST\r\n\
             END:STANDARD\r\n\
             END:VTIMEZONE\r\n"
        );
    }

    #[test]
    fn lists_explicit_transitions_when_dst_is_abolished() {
        // Sao Paulo abolished DST in 2019.
        let event = Event::new("Meeting", zoned("America/Sao_Paulo", 2018, 12, 1, 9));
        let blocks = ics_blocks(&event);

        assert!(!blocks.contains("RRULE"));
        assert!(
            blocks
                .contains("DTSTART:20181104T000000\r\nTZOFFSETFROM:-0300\r\nTZOFFSETTO:-0200\r\n")
        );
        assert!(blocks.ends_with(
            "DTSTART:20190217T000000\r\nTZOFFSETFROM:-0200\r\nTZOFFSETTO:-0300\r\nEND:STANDARD\r\nEND:VTIMEZONE\r\n"
        ));
    }

    #[test]
    fn treats_summer_as_daylight_in_negative_dst_zones() {
        let event = Event::new("Meeting", zoned("Europe/Dublin", 2026, 6, 1, 9));
        let blocks = ics_blocks(&event);

        assert!(blocks.contains("BEGIN:DAYLIGHT\r\nDTSTART:20250330T010000\r\nTZOFFSETFROM:+0000\r\nTZOFFSETTO:+0100\r\n"));
        assert!(blocks.contains("BEGIN:STANDARD\r\nDTSTART:20251026T020000\r\nTZOFFSETFROM:+0100\r\nTZOFFSETTO:+0000\r\n"));
    }

    #[test]
    fn mixes_explicit_and_rules_when_rules_change() {
        // US DST rules changed in 2007.
        let mut event = Event::new("Standup", zoned("America/New_York", 2006, 6, 1, 9));
        event.recurrence = Some(Recurrence::new("FREQ=WEEKLY"));
        let blocks = ics_blocks(&event);

        assert!(blocks.contains("DTSTART:20051030T020000\r\n"));
        assert!(blocks.contains("RRULE:FREQ=YEARLY;BYMONTH=3;BYDAY=2SU\r\n"));
        assert!(blocks.contains("DTSTART:20070311T020000\r\n"));
    }

    #[test]
    fn covers_every_referenced_zone_once() {
        let mut event = Event::new("Flight", zoned("Europe/Stockholm", 2026, 6, 1, 9));
        event.end = Some(zoned("America/New_York", 2026, 6, 1, 12));
        let blocks = ics_blocks(&event);

        assert_eq!(blocks.matches("BEGIN:VTIMEZONE").count(), 2);
        assert!(blocks.contains("TZID:Europe/Stockholm\r\n"));
        assert!(blocks.contains("TZID:America/New_York\r\n"));
    }

    #[test]
    fn skips_utc_floating_and_unknown_zones() {
        let mut event = Event::new("Test", zoned("Bogus/Zone", 2026, 6, 1, 9));
        event.end = Some(EventTime::DateTimeFloating(
            NaiveDate::from_ymd_opt(2026, 6, 1)
                .unwrap()
                .and_hms_opt(10, 0, 0)
                .unwrap(),
        ));

        assert_eq!(ics_blocks(&event), "");
    }
}

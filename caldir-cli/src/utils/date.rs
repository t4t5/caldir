use chrono::{NaiveDate, ParseResult};

pub fn parse_date(input: &str) -> ParseResult<NaiveDate> {
    NaiveDate::parse_from_str(input, "%Y-%m-%d")
}

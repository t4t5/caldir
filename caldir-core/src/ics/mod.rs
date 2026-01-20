//! ICS file generation and parsing.
//!
//! This module handles reading and writing .ics files according to RFC 5545.

mod generate;
mod parse;

pub use generate::generate_ics;
pub use parse::parse_event;

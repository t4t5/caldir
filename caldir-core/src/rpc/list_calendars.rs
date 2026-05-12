use super::{Method, Rpc};
use crate::CalendarConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct ListCalendars {
    pub account_identifier: String,
}

impl Rpc for ListCalendars {
    const METHOD: Method = Method::ListCalendars;
    type Response = Vec<CalendarConfig>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_calendars_serializes_json() {
        let cmd = ListCalendars {
            account_identifier: "user@hmail.com".to_string(),
        };

        let json = cmd.to_json().unwrap();

        assert_eq!(json["command"], "list_calendars");
        assert_eq!(json["params"]["account_identifier"], "user@hmail.com");
    }
}

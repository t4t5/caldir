pub const PROVIDER_NAME: &str = "outlook";
pub const PROVIDER_EVENT_ID_PROPERTY: &str = "X-OUTLOOK-EVENT-ID";
pub const PROVIDER_CONFERENCE_PROPERTY: &str = "X-OUTLOOK-CONFERENCE";

// Outlook uses HTML descriptions, traditionally using this custom prop
// (See https://www.experts-exchange.com/questions/28079623/HTML-in-Outlook-ics-file.html)
pub const HTML_DESC_PROPERTY: &str = "X-ALT-DESC";

//! Map Microsoft Windows time zone names to IANA names.
//!
//! Outlook and other Microsoft tooling emit ICS with TZID values like
//! `E. South America Standard Time` instead of `America/Sao_Paulo`. These
//! names are valid in ICS (TZID is just an identifier per RFC 5545) but
//! downstream consumers — like the `rrule` crate used for recurrence
//! expansion — only accept IANA names.
//!
//! Source: Unicode CLDR `windowsZones.xml`, default territory ("001").

/// Translate a Windows time zone name to its IANA equivalent.
///
/// Returns `None` if the input is already an IANA name or an unrecognized
/// value — callers should treat that as "leave unchanged."
pub(crate) fn to_iana(name: &str) -> Option<&'static str> {
    Some(match name {
        "Dateline Standard Time" => "Etc/GMT+12",
        "UTC-11" => "Etc/GMT+11",
        "Aleutian Standard Time" => "America/Adak",
        "Hawaiian Standard Time" => "Pacific/Honolulu",
        "Marquesas Standard Time" => "Pacific/Marquesas",
        "Alaskan Standard Time" => "America/Anchorage",
        "UTC-09" => "Etc/GMT+9",
        "Pacific Standard Time (Mexico)" => "America/Tijuana",
        "UTC-08" => "Etc/GMT+8",
        "Pacific Standard Time" => "America/Los_Angeles",
        "US Mountain Standard Time" => "America/Phoenix",
        "Mountain Standard Time (Mexico)" => "America/Mazatlan",
        "Mountain Standard Time" => "America/Denver",
        "Yukon Standard Time" => "America/Whitehorse",
        "Central America Standard Time" => "America/Guatemala",
        "Central Standard Time" => "America/Chicago",
        "Easter Island Standard Time" => "Pacific/Easter",
        "Central Standard Time (Mexico)" => "America/Mexico_City",
        "Canada Central Standard Time" => "America/Regina",
        "SA Pacific Standard Time" => "America/Bogota",
        "Eastern Standard Time (Mexico)" => "America/Cancun",
        "Eastern Standard Time" => "America/New_York",
        "Haiti Standard Time" => "America/Port-au-Prince",
        "Cuba Standard Time" => "America/Havana",
        "US Eastern Standard Time" => "America/Indianapolis",
        "Turks And Caicos Standard Time" => "America/Grand_Turk",
        "Paraguay Standard Time" => "America/Asuncion",
        "Atlantic Standard Time" => "America/Halifax",
        "Venezuela Standard Time" => "America/Caracas",
        "Central Brazilian Standard Time" => "America/Cuiaba",
        "SA Western Standard Time" => "America/La_Paz",
        "Pacific SA Standard Time" => "America/Santiago",
        "Newfoundland Standard Time" => "America/St_Johns",
        "Tocantins Standard Time" => "America/Araguaina",
        "E. South America Standard Time" => "America/Sao_Paulo",
        "SA Eastern Standard Time" => "America/Cayenne",
        "Argentina Standard Time" => "America/Buenos_Aires",
        "Greenland Standard Time" => "America/Godthab",
        "Montevideo Standard Time" => "America/Montevideo",
        "Magallanes Standard Time" => "America/Punta_Arenas",
        "Saint Pierre Standard Time" => "America/Miquelon",
        "Bahia Standard Time" => "America/Bahia",
        "UTC-02" => "Etc/GMT+2",
        "Mid-Atlantic Standard Time" => "Atlantic/South_Georgia",
        "Azores Standard Time" => "Atlantic/Azores",
        "Cape Verde Standard Time" => "Atlantic/Cape_Verde",
        "UTC" => "Etc/UTC",
        "GMT Standard Time" => "Europe/London",
        "Greenwich Standard Time" => "Atlantic/Reykjavik",
        "Sao Tome Standard Time" => "Africa/Sao_Tome",
        "Morocco Standard Time" => "Africa/Casablanca",
        "W. Europe Standard Time" => "Europe/Berlin",
        "Central Europe Standard Time" => "Europe/Budapest",
        "Romance Standard Time" => "Europe/Paris",
        "Central European Standard Time" => "Europe/Warsaw",
        "W. Central Africa Standard Time" => "Africa/Lagos",
        "Jordan Standard Time" => "Asia/Amman",
        "GTB Standard Time" => "Europe/Bucharest",
        "Middle East Standard Time" => "Asia/Beirut",
        "Egypt Standard Time" => "Africa/Cairo",
        "E. Europe Standard Time" => "Europe/Chisinau",
        "Syria Standard Time" => "Asia/Damascus",
        "West Bank Standard Time" => "Asia/Hebron",
        "South Africa Standard Time" => "Africa/Johannesburg",
        "FLE Standard Time" => "Europe/Kiev",
        "Israel Standard Time" => "Asia/Jerusalem",
        "South Sudan Standard Time" => "Africa/Juba",
        "Kaliningrad Standard Time" => "Europe/Kaliningrad",
        "Sudan Standard Time" => "Africa/Khartoum",
        "Libya Standard Time" => "Africa/Tripoli",
        "Namibia Standard Time" => "Africa/Windhoek",
        "Arabic Standard Time" => "Asia/Baghdad",
        "Turkey Standard Time" => "Europe/Istanbul",
        "Arab Standard Time" => "Asia/Riyadh",
        "Belarus Standard Time" => "Europe/Minsk",
        "Russian Standard Time" => "Europe/Moscow",
        "E. Africa Standard Time" => "Africa/Nairobi",
        "Volgograd Standard Time" => "Europe/Volgograd",
        "Iran Standard Time" => "Asia/Tehran",
        "Arabian Standard Time" => "Asia/Dubai",
        "Astrakhan Standard Time" => "Europe/Astrakhan",
        "Azerbaijan Standard Time" => "Asia/Baku",
        "Russia Time Zone 3" => "Europe/Samara",
        "Mauritius Standard Time" => "Indian/Mauritius",
        "Saratov Standard Time" => "Europe/Saratov",
        "Georgian Standard Time" => "Asia/Tbilisi",
        "Caucasus Standard Time" => "Asia/Yerevan",
        "Afghanistan Standard Time" => "Asia/Kabul",
        "West Asia Standard Time" => "Asia/Tashkent",
        "Qyzylorda Standard Time" => "Asia/Qyzylorda",
        "Ekaterinburg Standard Time" => "Asia/Yekaterinburg",
        "Pakistan Standard Time" => "Asia/Karachi",
        "India Standard Time" => "Asia/Calcutta",
        "Sri Lanka Standard Time" => "Asia/Colombo",
        "Nepal Standard Time" => "Asia/Katmandu",
        "Central Asia Standard Time" => "Asia/Bishkek",
        "Bangladesh Standard Time" => "Asia/Dhaka",
        "Omsk Standard Time" => "Asia/Omsk",
        "Myanmar Standard Time" => "Asia/Rangoon",
        "SE Asia Standard Time" => "Asia/Bangkok",
        "Altai Standard Time" => "Asia/Barnaul",
        "W. Mongolia Standard Time" => "Asia/Hovd",
        "North Asia Standard Time" => "Asia/Krasnoyarsk",
        "N. Central Asia Standard Time" => "Asia/Novosibirsk",
        "Tomsk Standard Time" => "Asia/Tomsk",
        "China Standard Time" => "Asia/Shanghai",
        "North Asia East Standard Time" => "Asia/Irkutsk",
        "Singapore Standard Time" => "Asia/Singapore",
        "W. Australia Standard Time" => "Australia/Perth",
        "Taipei Standard Time" => "Asia/Taipei",
        "Ulaanbaatar Standard Time" => "Asia/Ulaanbaatar",
        "Aus Central W. Standard Time" => "Australia/Eucla",
        "Transbaikal Standard Time" => "Asia/Chita",
        "Tokyo Standard Time" => "Asia/Tokyo",
        "North Korea Standard Time" => "Asia/Pyongyang",
        "Korea Standard Time" => "Asia/Seoul",
        "Yakutsk Standard Time" => "Asia/Yakutsk",
        "Cen. Australia Standard Time" => "Australia/Adelaide",
        "AUS Central Standard Time" => "Australia/Darwin",
        "E. Australia Standard Time" => "Australia/Brisbane",
        "AUS Eastern Standard Time" => "Australia/Sydney",
        "West Pacific Standard Time" => "Pacific/Port_Moresby",
        "Tasmania Standard Time" => "Australia/Hobart",
        "Vladivostok Standard Time" => "Asia/Vladivostok",
        "Lord Howe Standard Time" => "Australia/Lord_Howe",
        "Bougainville Standard Time" => "Pacific/Bougainville",
        "Russia Time Zone 10" => "Asia/Srednekolymsk",
        "Magadan Standard Time" => "Asia/Magadan",
        "Norfolk Standard Time" => "Pacific/Norfolk",
        "Sakhalin Standard Time" => "Asia/Sakhalin",
        "Central Pacific Standard Time" => "Pacific/Guadalcanal",
        "Russia Time Zone 11" => "Asia/Kamchatka",
        "New Zealand Standard Time" => "Pacific/Auckland",
        "UTC+12" => "Etc/GMT-12",
        "Fiji Standard Time" => "Pacific/Fiji",
        "Kamchatka Standard Time" => "Asia/Kamchatka",
        "Chatham Islands Standard Time" => "Pacific/Chatham",
        "UTC+13" => "Etc/GMT-13",
        "Tonga Standard Time" => "Pacific/Tongatapu",
        "Samoa Standard Time" => "Pacific/Apia",
        "Line Islands Standard Time" => "Pacific/Kiritimati",
        _ => return None,
    })
}

/// Take a TZID string and return the IANA equivalent if the input is a
/// known Windows zone name; otherwise return the input unchanged.
pub(crate) fn normalize(tzid: String) -> String {
    match to_iana(&tzid) {
        Some(iana) => iana.to_owned(),
        None => tzid,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_known_windows_zones() {
        assert_eq!(to_iana("E. South America Standard Time"), Some("America/Sao_Paulo"));
        assert_eq!(to_iana("W. Europe Standard Time"), Some("Europe/Berlin"));
        assert_eq!(to_iana("Pacific Standard Time"), Some("America/Los_Angeles"));
    }

    #[test]
    fn returns_none_for_iana_names() {
        assert_eq!(to_iana("America/Sao_Paulo"), None);
        assert_eq!(to_iana("Europe/Berlin"), None);
    }

    #[test]
    fn returns_none_for_unknown_names() {
        assert_eq!(to_iana("Bogus/Zone"), None);
        assert_eq!(to_iana(""), None);
    }

    #[test]
    fn normalize_rewrites_known_and_passes_through_others() {
        assert_eq!(
            normalize("E. South America Standard Time".into()),
            "America/Sao_Paulo"
        );
        assert_eq!(normalize("America/New_York".into()), "America/New_York");
        assert_eq!(normalize("Bogus/Zone".into()), "Bogus/Zone");
    }
}

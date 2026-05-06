mod participation_status;

pub use participation_status::ParticipationStatus;

use icalendar::Property;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attendee {
    pub email: String,
    pub name: Option<String>,
    pub status: Option<ParticipationStatus>,
}

impl Attendee {
    pub fn new(email: impl Into<String>) -> Self {
        Attendee {
            email: email.into(),
            name: None,
            status: None,
        }
    }
}

impl From<&Attendee> for Property {
    fn from(value: &Attendee) -> Self {
        let mut prop = Property::new("ATTENDEE", format!("mailto:{}", value.email));
        if let Some(name) = &value.name {
            prop.add_parameter("CN", name);
        }
        if let Some(status) = value.status {
            prop.add_parameter("PARTSTAT", status.as_ics_str());
        }
        prop.done()
    }
}

impl From<&Property> for Attendee {
    fn from(value: &Property) -> Self {
        let email = value
            .value()
            .strip_prefix("mailto:")
            .unwrap_or(value.value())
            .to_string();
        let name = value.params().get("CN").map(|p| p.value().to_string());
        let status = value
            .params()
            .get("PARTSTAT")
            .and_then(|p| ParticipationStatus::from_ics_str(p.value()));
        Attendee {
            email,
            name,
            status,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_email_from_mailto_value() {
        let prop = Property::new("ATTENDEE", "mailto:jane@example.com").done();

        let attendee = Attendee::from(&prop);

        assert_eq!(attendee.email, "jane@example.com");
        assert_eq!(attendee.name, None);
        assert_eq!(attendee.status, None);
    }

    #[test]
    fn parses_email_without_mailto_prefix() {
        let prop = Property::new("ATTENDEE", "jane@example.com").done();

        let attendee = Attendee::from(&prop);

        assert_eq!(attendee.email, "jane@example.com");
    }

    #[test]
    fn parses_cn_parameter_as_name() {
        let mut prop = Property::new("ATTENDEE", "mailto:jane@example.com");
        prop.add_parameter("CN", "Jane Doe");

        let attendee = Attendee::from(&prop.done());

        assert_eq!(attendee.name.as_deref(), Some("Jane Doe"));
    }

    #[test]
    fn parses_partstat_parameter_as_status() {
        let mut prop = Property::new("ATTENDEE", "mailto:jane@example.com");
        prop.add_parameter("PARTSTAT", "ACCEPTED");

        let attendee = Attendee::from(&prop.done());

        assert_eq!(attendee.status, Some(ParticipationStatus::Accepted));
    }

    #[test]
    fn writes_email_with_mailto_prefix() {
        let attendee = Attendee::new("jane@example.com");

        let prop = Property::from(&attendee);

        assert_eq!(prop.value(), "mailto:jane@example.com");
        assert!(prop.params().get("CN").is_none());
        assert!(prop.params().get("PARTSTAT").is_none());
    }

    #[test]
    fn writes_name_as_cn_parameter() {
        let attendee = Attendee {
            email: "jane@example.com".to_string(),
            name: Some("Jane Doe".to_string()),
            status: None,
        };

        let prop = Property::from(&attendee);

        assert_eq!(prop.params().get("CN").map(|p| p.value()), Some("Jane Doe"));
    }

    #[test]
    fn writes_status_as_partstat_parameter() {
        let attendee = Attendee {
            email: "jane@example.com".to_string(),
            name: None,
            status: Some(ParticipationStatus::Accepted),
        };

        let prop = Property::from(&attendee);

        assert_eq!(
            prop.params().get("PARTSTAT").map(|p| p.value()),
            Some("ACCEPTED")
        );
    }
}

use icalendar::Property;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Organizer {
    pub email: String,
    pub name: Option<String>,
}

impl Organizer {
    pub fn new(email: impl Into<String>) -> Self {
        Organizer {
            email: email.into(),
            name: None,
        }
    }
}

impl From<&Organizer> for Property {
    fn from(value: &Organizer) -> Self {
        let mut prop = Property::new("ORGANIZER", format!("mailto:{}", value.email));
        if let Some(name) = &value.name {
            prop.add_parameter("CN", name);
        }
        prop.done()
    }
}

impl From<&Property> for Organizer {
    fn from(value: &Property) -> Self {
        let email = value
            .value()
            .strip_prefix("mailto:")
            .unwrap_or(value.value())
            .to_string();
        let name = value.params().get("CN").map(|p| p.value().to_string());
        Organizer { email, name }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_email_from_mailto_value() {
        let prop = Property::new("ORGANIZER", "mailto:alice@example.com").done();

        let organizer = Organizer::from(&prop);

        assert_eq!(organizer.email, "alice@example.com");
        assert_eq!(organizer.name, None);
    }

    #[test]
    fn parses_email_without_mailto_prefix() {
        let prop = Property::new("ORGANIZER", "alice@example.com").done();

        let organizer = Organizer::from(&prop);

        assert_eq!(organizer.email, "alice@example.com");
    }

    #[test]
    fn parses_cn_parameter_as_name() {
        let mut prop = Property::new("ORGANIZER", "mailto:alice@example.com");
        prop.add_parameter("CN", "Alice Smith");

        let organizer = Organizer::from(&prop.done());

        assert_eq!(organizer.name.as_deref(), Some("Alice Smith"));
    }

    #[test]
    fn writes_email_with_mailto_prefix() {
        let organizer = Organizer::new("alice@example.com");

        let prop = Property::from(&organizer);

        assert_eq!(prop.value(), "mailto:alice@example.com");
        assert!(prop.params().get("CN").is_none());
    }

    #[test]
    fn writes_name_as_cn_parameter() {
        let organizer = Organizer {
            email: "alice@example.com".to_string(),
            name: Some("Alice Smith".to_string()),
        };

        let prop = Property::from(&organizer);

        assert_eq!(
            prop.params().get("CN").map(|p| p.value()),
            Some("Alice Smith")
        );
    }
}

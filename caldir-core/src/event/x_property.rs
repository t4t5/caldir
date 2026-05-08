use icalendar::Property;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XProperty {
    pub name: String,
    pub value: String,
    pub params: Vec<(String, String)>,
}

impl XProperty {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        XProperty {
            name: name.into(),
            value: value.into(),
            params: Vec::new(),
        }
    }
}

impl From<&XProperty> for Property {
    fn from(value: &XProperty) -> Self {
        let mut prop = Property::new(&value.name, &value.value);
        for (k, v) in &value.params {
            prop.add_parameter(k, v);
        }
        prop.done()
    }
}

impl From<&Property> for XProperty {
    fn from(value: &Property) -> Self {
        let params = value
            .params()
            .values()
            .map(|p| (p.key().to_string(), p.value().to_string()))
            .collect();
        XProperty {
            name: value.key().to_string(),
            value: value.value().to_string(),
            params,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_name_and_value() {
        let prop = Property::new("X-HOOLI-EVENT-ID", "abc123").done();

        let x = XProperty::from(&prop);

        assert_eq!(x.name, "X-HOOLI-EVENT-ID");
        assert_eq!(x.value, "abc123");
        assert!(x.params.is_empty());
    }

    #[test]
    fn parses_params() {
        let mut prop = Property::new("X-ALT-DESC", "<html>...</html>");
        prop.add_parameter("FMTTYPE", "text/html");

        let x = XProperty::from(&prop.done());

        assert_eq!(
            x.params,
            vec![("FMTTYPE".to_string(), "text/html".to_string())]
        );
    }

    #[test]
    fn writes_name_and_value() {
        let x = XProperty::new("X-HOOLI-EVENT-ID", "abc123");

        let prop = Property::from(&x);

        assert_eq!(prop.key(), "X-HOOLI-EVENT-ID");
        assert_eq!(prop.value(), "abc123");
        assert!(prop.params().is_empty());
    }

    #[test]
    fn writes_params() {
        let x = XProperty {
            name: "X-ALT-DESC".to_string(),
            value: "<html>...</html>".to_string(),
            params: vec![("FMTTYPE".to_string(), "text/html".to_string())],
        };

        let prop = Property::from(&x);

        assert_eq!(
            prop.params().get("FMTTYPE").map(|p| p.value()),
            Some("text/html")
        );
    }
}

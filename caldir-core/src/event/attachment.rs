use icalendar::Property;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attachment {
    pub uri: String,
    pub params: Vec<(String, String)>,
}

impl Attachment {
    pub fn new(uri: impl Into<String>) -> Self {
        Attachment {
            uri: uri.into(),
            params: Vec::new(),
        }
    }

    /// Parse an `ATTACH` property, keeping it ONLY when it's a URI reference (default behaviour
    /// for Google Drive attachments and iCloud attachments)
    /// Returns `None` for inline binary attachments (we don't want to store long blobs)
    pub fn from_property(prop: &Property) -> Option<Self> {
        if is_binary(prop) {
            return None;
        }

        let params = prop
            .params()
            .values()
            .map(|p| (p.key().to_string(), p.value().to_string()))
            .collect();

        Some(Attachment {
            uri: prop.value().to_string(),
            params,
        })
    }
}

fn is_binary(prop: &Property) -> bool {
    prop.params().values().any(|p| {
        (p.key().eq_ignore_ascii_case("VALUE") && p.value().eq_ignore_ascii_case("BINARY"))
            || (p.key().eq_ignore_ascii_case("ENCODING")
                && p.value().eq_ignore_ascii_case("BASE64"))
    })
}

impl From<&Attachment> for Property {
    fn from(value: &Attachment) -> Self {
        let mut prop = Property::new("ATTACH", &value.uri);
        for (k, v) in &value.params {
            prop.add_parameter(k, v);
        }
        prop.done()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_uri_value() {
        let prop = Property::new("ATTACH", "https://drive.google.com/file/d/abc").done();

        let attachment = Attachment::from_property(&prop).unwrap();

        assert_eq!(attachment.uri, "https://drive.google.com/file/d/abc");
        assert!(attachment.params.is_empty());
    }

    #[test]
    fn captures_params() {
        // Apple / CalDAV managed attachment: MANAGED-ID must survive so the
        // server keeps the attachment on a later PUT (RFC 8607).
        let mut prop = Property::new("ATTACH", "https://p01.icloud.com/att/abc");
        prop.add_parameter("MANAGED-ID", "1");
        prop.add_parameter("FMTTYPE", "application/pdf");
        prop.add_parameter("FILENAME", "agenda.pdf");

        let attachment = Attachment::from_property(&prop.done()).unwrap();

        assert!(
            attachment
                .params
                .contains(&("MANAGED-ID".to_string(), "1".to_string()))
        );
        assert!(
            attachment
                .params
                .contains(&("FILENAME".to_string(), "agenda.pdf".to_string()))
        );
    }

    #[test]
    fn skips_binary_value() {
        let mut prop = Property::new("ATTACH", "VGhlIHF1aWNr");
        prop.add_parameter("VALUE", "BINARY");
        prop.add_parameter("ENCODING", "BASE64");

        assert!(Attachment::from_property(&prop.done()).is_none());
    }

    #[test]
    fn skips_base64_encoding_without_value_param() {
        let mut prop = Property::new("ATTACH", "VGhlIHF1aWNr");
        prop.add_parameter("ENCODING", "BASE64");

        assert!(Attachment::from_property(&prop.done()).is_none());
    }

    #[test]
    fn writes_attach_property_with_params() {
        let attachment = Attachment {
            uri: "https://example.com/report.doc".to_string(),
            params: vec![("FMTTYPE".to_string(), "application/msword".to_string())],
        };

        let prop = Property::from(&attachment);

        assert_eq!(prop.key(), "ATTACH");
        assert_eq!(prop.value(), "https://example.com/report.doc");
        assert_eq!(
            prop.params().get("FMTTYPE").map(|p| p.value()),
            Some("application/msword")
        );
    }
}

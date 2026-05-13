const MAX_SLUG_LENGTH: usize = 50;

pub fn slugify(s: &str) -> String {
    let slug = slug::slugify(s);
    slug.chars().take(MAX_SLUG_LENGTH).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugifies_string() {
        assert_eq!(slugify("Meeting with Alice"), "meeting-with-alice");
    }
}

pub fn slugify(s: &str) -> String {
    let slug = slug::slugify(s);
    slug.chars().take(50).collect()
}

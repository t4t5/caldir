//! Look up the user's email via `calendar-user-address-set` (RFC 4791 §6.2.1).

use anyhow::{Context, Result};
use libdav::caldav::GetUserAddressSet;

use crate::caldav::create_caldav_client;

/// The principal's first `mailto:` address, which may differ from the login.
pub async fn find_user_email(
    username: &str,
    password: &str,
    principal_url: &str,
) -> Result<Option<String>> {
    let caldav = create_caldav_client(principal_url, username, password)?;
    let principal: http::Uri = principal_url.parse().context("Invalid principal URL")?;

    let response = caldav
        .request(GetUserAddressSet::new(&principal))
        .await
        .context("Failed to fetch calendar-user-address-set")?;

    Ok(first_mailto(&response.addresses))
}

fn first_mailto(addresses: &[String]) -> Option<String> {
    addresses.iter().find_map(|address| {
        let (scheme, email) = address.split_once(':')?;
        (scheme.eq_ignore_ascii_case("mailto") && email.contains('@')).then(|| email.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addresses(values: &[&str]) -> Vec<String> {
        values.iter().map(|v| v.to_string()).collect()
    }

    #[test]
    fn skips_principal_paths() {
        let set = addresses(&["/principals/AB12345/", "mailto:alice@example.com"]);

        assert_eq!(first_mailto(&set), Some("alice@example.com".to_string()));
    }

    #[test]
    fn matches_mailto_case_insensitively() {
        let set = addresses(&["MAILTO:alice@example.com"]);

        assert_eq!(first_mailto(&set), Some("alice@example.com".to_string()));
    }

    #[test]
    fn none_without_mailto() {
        let set = addresses(&["/principals/alice/", "urn:uuid:1234"]);

        assert_eq!(first_mailto(&set), None);
    }
}

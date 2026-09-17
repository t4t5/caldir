//! Delete one logical event from a CalDAV resource.

use anyhow::{Context, Result};
use caldir_core::EventInstanceId;
use http::{Method, Request, StatusCode};

use crate::caldav::create_caldav_client;

use super::resource::{check_write_status, find_resource, put};

/// Remove only the matching component; delete the resource when it becomes empty.
pub async fn delete_event(
    username: &str,
    password: &str,
    calendar_url: &str,
    id: &EventInstanceId,
) -> Result<()> {
    let caldav = create_caldav_client(calendar_url, username, password)?;
    let Some(resource) = find_resource(&caldav, calendar_url, id.uid().as_str()).await? else {
        return Ok(());
    };
    let Some(data) = resource.remove(id)? else {
        return Ok(());
    };
    if !data.is_empty() {
        return put(&caldav, &resource.href, Some(resource.etag()?), data).await;
    }
    let request = Request::builder()
        .method(Method::DELETE)
        .uri(caldav.relative_uri(&resource.href)?)
        .header("If-Match", resource.etag()?)
        .body(String::new())?;
    let (parts, _) = caldav
        .request_raw(request)
        .await
        .context("Failed to delete CalDAV resource")?;
    if parts.status == StatusCode::NOT_FOUND {
        return Ok(());
    }
    check_write_status(parts.status)
}

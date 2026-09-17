use std::collections::HashSet;

use anyhow::{Context, Result, bail, ensure};
use caldir_core::{Event, EventInstanceId};
use http::{Method, Request, StatusCode};
use icalendar::parser::{Component, read_components};

use crate::caldav::{CalDavClient_, create_caldav_client, event_url, url_to_href};

fn unfold(data: &str) -> String {
    icalendar::parser::unfold(&data.replace("\r\n", "\n"))
}

/// Validate the complete component tree before modifying any resource.
struct Document<'a> {
    root: Component<'a>,
    events: Vec<(usize, Event)>,
}

impl<'a> Document<'a> {
    fn parse(unfolded: &'a str) -> Result<Self> {
        let mut roots = read_components(unfolded).map_err(anyhow::Error::msg)?;
        ensure!(roots.len() == 1, "Expected one VCALENDAR");
        let root = roots.remove(0);
        ensure!(root.name == "VCALENDAR", "Expected VCALENDAR");
        let mut events = Vec::new();
        let mut identities = HashSet::new();
        for (index, component) in root.components.iter().enumerate() {
            validate_children(component)?;
            ensure!(
                !component.name.as_str().eq_ignore_ascii_case("VCALENDAR"),
                "Nested VCALENDAR"
            );
            if !component.name.as_str().eq_ignore_ascii_case("VEVENT") {
                continue;
            }
            ensure!(component.name == "VEVENT", "Unsupported VEVENT casing");
            for name in ["UID", "RECURRENCE-ID", "DTSTART"] {
                ensure!(
                    component
                        .properties
                        .iter()
                        .filter(|p| p.name.as_str().eq_ignore_ascii_case(name))
                        .count()
                        <= 1,
                    "Ambiguous {name} in VEVENT"
                );
            }
            let icalendar::CalendarComponent::Event(event) = component.clone().into() else {
                unreachable!();
            };
            let event = Event::try_from(event).context("Invalid VEVENT in CalDAV resource")?;
            ensure!(!event.uid.as_str().is_empty(), "Empty UID");
            ensure!(
                !component
                    .properties
                    .iter()
                    .any(|p| p.name.as_str().eq_ignore_ascii_case("RECURRENCE-ID"))
                    || event.recurrence_id.is_some(),
                "Invalid RECURRENCE-ID"
            );
            ensure!(
                identities.insert(event.event_instance_id()),
                "Duplicate event identity"
            );
            if let Some((_, first)) = events.first() {
                let first: &Event = first;
                ensure!(
                    event.uid == first.uid,
                    "Multiple UIDs in one CalDAV resource"
                );
            }
            events.push((index, event));
        }
        ensure!(!events.is_empty(), "CalDAV resource contains no VEVENTs");
        Ok(Self { root, events })
    }

    fn position(&self, id: &EventInstanceId) -> Option<usize> {
        self.events
            .iter()
            .find(|(_, e)| e.event_instance_id() == *id)
            .map(|(i, _)| *i)
    }
}

fn validate_children(component: &Component<'_>) -> Result<()> {
    for child in &component.components {
        ensure!(
            !child.name.as_str().eq_ignore_ascii_case("VEVENT")
                && !child.name.as_str().eq_ignore_ascii_case("VCALENDAR"),
            "Unexpected nested calendar or event"
        );
        validate_children(child)?;
    }
    Ok(())
}

pub(super) fn parse_events(data: &str) -> Result<Vec<Event>> {
    Ok(Document::parse(&unfold(data))?
        .events
        .into_iter()
        .map(|(_, e)| e)
        .collect())
}

/// Locate direct children using unfolded content lines and a component stack.
/// Original bytes remain intact, including parameter quoting and folded lines.
fn component_spans(data: &str) -> Result<(Vec<std::ops::Range<usize>>, usize)> {
    let mut lines: Vec<std::ops::Range<usize>> = Vec::new();
    let mut offset = 0;
    for line in data.split_inclusive('\n') {
        let end = offset + line.len();
        if line.starts_with([' ', '\t']) {
            lines.last_mut().context("Orphan folded line")?.end = end;
        } else {
            lines.push(offset..end);
        }
        offset = end;
    }
    let mut stack = Vec::new();
    let mut spans = Vec::new();
    let mut start = 0;
    let mut calendar_end = None;
    for range in lines {
        let line = unfold(&data[range.clone()]);
        let line = line.trim_end_matches(['\r', '\n']);
        let Some((boundary, name)) = line.split_once(':') else {
            continue;
        };
        if boundary.eq_ignore_ascii_case("BEGIN") {
            if stack.len() == 1 {
                start = range.start;
            }
            stack.push(name.to_owned());
        } else if boundary.eq_ignore_ascii_case("END") {
            ensure!(
                stack.pop().as_deref() == Some(name),
                "Mismatched component boundary"
            );
            if stack.len() == 1 {
                spans.push(start..range.end);
            }
            if stack.is_empty() {
                calendar_end = Some(range.start);
            }
        }
    }
    ensure!(stack.is_empty(), "Unclosed component");
    Ok((spans, calendar_end.context("Missing VCALENDAR end")?))
}

fn merge(data: &str, event: &Event) -> Result<String> {
    let unfolded = unfold(data);
    let document = Document::parse(&unfolded)?;
    ensure!(
        document.events[0].1.uid == event.uid,
        "Resource UID does not match requested event"
    );
    let (spans, end) = component_spans(data)?;
    ensure!(
        spans.len() == document.root.components.len(),
        "Ambiguous component boundaries"
    );
    // Only siblings retain their original bytes; the edited event uses core's serializer.
    let replacement = event.to_ics_string();
    let (replacement_spans, _) = component_spans(&replacement)?;
    ensure!(
        replacement_spans.len() == 1,
        "Expected one replacement VEVENT"
    );
    let range = document
        .position(&event.event_instance_id())
        .map(|i| spans[i].clone())
        .unwrap_or(end..end);
    let mut merged = data.to_owned();
    merged.replace_range(range, &replacement[replacement_spans[0].clone()]);
    parse_events(&merged)?;
    Ok(merged)
}

pub(super) struct Resource {
    pub href: String,
    pub etag: Option<String>,
    pub data: String,
}

pub(super) enum Removal {
    Missing,
    Replace(String),
    Delete,
}

impl Resource {
    /// Passed through verbatim; the server decides whether a weak ETag matches.
    pub fn etag(&self) -> Result<&str> {
        self.etag.as_deref().context("CalDAV resource has no ETag")
    }

    pub fn remove(&self, id: &EventInstanceId) -> Result<Removal> {
        let unfolded = unfold(&self.data);
        let document = Document::parse(&unfolded)?;
        ensure!(
            document.events[0].1.uid == *id.uid(),
            "Resource UID does not match requested event"
        );
        let Some(index) = document.position(id) else {
            return Ok(Removal::Missing);
        };
        let (spans, _) = component_spans(&self.data)?;
        ensure!(
            spans.len() == document.root.components.len(),
            "Ambiguous component boundaries"
        );
        if document.events.len() == 1 {
            Ok(Removal::Delete)
        } else {
            let mut remaining = self.data.clone();
            remaining.replace_range(spans[index].clone(), "");
            parse_events(&remaining)?;
            Ok(Removal::Replace(remaining))
        }
    }
}

pub(super) async fn get_resource(caldav: &CalDavClient_, href: &str) -> Result<Option<Resource>> {
    let request = Request::builder()
        .method(Method::GET)
        .uri(caldav.relative_uri(href)?)
        .body(String::new())?;
    let (parts, body) = caldav
        .request_raw(request)
        .await
        .context("Failed to read CalDAV resource")?;
    if parts.status == StatusCode::NOT_FOUND {
        return Ok(None);
    }
    ensure!(
        parts.status == StatusCode::OK,
        "Failed to read CalDAV resource: {}",
        parts.status
    );
    Ok(Some(Resource {
        href: href.to_owned(),
        etag: parts
            .headers
            .get("ETag")
            .map(|v| v.to_str().map(str::to_owned))
            .transpose()?,
        data: std::str::from_utf8(&body)?.to_owned(),
    }))
}

pub(super) async fn find_resource(
    caldav: &CalDavClient_,
    calendar_url: &str,
    uid: &str,
) -> Result<Option<Resource>> {
    let href = url_to_href(&event_url(calendar_url, uid));
    if let Some(resource) = get_resource(caldav, &href).await? {
        return Ok(Some(resource));
    }
    let filter = format!(
        r#"<C:prop-filter name="UID"><C:text-match collation="i;octet">{}</C:text-match></C:prop-filter>"#,
        xml_escape(uid)
    );
    let mut resources = query(caldav, calendar_url, &filter).await?;
    // text-match is a substring match; verify the complete UID before selecting.
    let mut found = None;
    for resource in resources.drain(..) {
        let events = parse_events(&resource.data)
            .with_context(|| format!("Invalid CalDAV resource {}", resource.href))?;
        if events[0].uid.as_str() == uid {
            ensure!(found.is_none(), "Multiple CalDAV resources match UID");
            found = Some(resource);
        }
    }
    Ok(found)
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

pub(super) async fn query(
    caldav: &CalDavClient_,
    calendar_url: &str,
    filter: &str,
) -> Result<Vec<Resource>> {
    let body = format!(
        r#"<C:calendar-query xmlns="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav"><prop><getetag/><C:calendar-data/></prop><C:filter><C:comp-filter name="VCALENDAR"><C:comp-filter name="VEVENT">{filter}</C:comp-filter></C:comp-filter></C:filter></C:calendar-query>"#
    );
    let request = Request::builder()
        .method("REPORT")
        .uri(caldav.relative_uri(&url_to_href(calendar_url))?)
        .header("Depth", "1")
        .header("Content-Type", "application/xml")
        .body(body)?;
    let (parts, body) = caldav
        .request_raw(request)
        .await
        .context("Failed to query CalDAV resources")?;
    ensure!(
        parts.status == StatusCode::MULTI_STATUS,
        "Failed to query CalDAV resources: {}",
        parts.status
    );
    parse_multistatus(std::str::from_utf8(&body)?)
}

fn parse_multistatus(body: &str) -> Result<Vec<Resource>> {
    let doc = roxmltree::Document::parse(body)?;
    ensure!(
        doc.root_element().has_tag_name(("DAV:", "multistatus")),
        "Expected DAV multistatus"
    );
    let mut resources = Vec::new();
    for response in doc
        .root_element()
        .children()
        .filter(|n| n.has_tag_name(("DAV:", "response")))
    {
        let href = response
            .children()
            .find(|n| n.has_tag_name(("DAV:", "href")))
            .and_then(|n| n.text())
            .context("Missing resource href")?;
        let mut etag = None;
        let mut data = None;
        for status in response
            .descendants()
            .filter(|n| n.has_tag_name(("DAV:", "status")))
        {
            let status = status.text().unwrap_or_default();
            let code = status
                .split_whitespace()
                .nth(1)
                .context("Missing DAV status code")?;
            ensure!(code == "200", "CalDAV resource {href}: {status}");
        }
        for prop in response.descendants() {
            if prop.has_tag_name(("DAV:", "getetag")) {
                etag = prop.text().map(str::to_owned);
            } else if prop.has_tag_name(("urn:ietf:params:xml:ns:caldav", "calendar-data")) {
                data = prop.text().map(str::to_owned);
            }
        }
        resources.push(Resource {
            href: href.to_owned(),
            etag,
            data: data.context("Missing calendar-data in CalDAV response")?,
        });
    }
    Ok(resources)
}

pub(super) async fn put(
    caldav: &CalDavClient_,
    href: &str,
    etag: Option<&str>,
    data: String,
) -> Result<()> {
    // XML calendar-data normalizes CRLF to LF; restore ICS line endings.
    let data = data.replace("\r\n", "\n").replace('\n', "\r\n");
    // Raw requests avoid libdav's duplicate Content-Type header.
    let mut request = Request::builder()
        .method(Method::PUT)
        .uri(caldav.relative_uri(href)?)
        .header("Content-Type", "text/calendar");
    request = match etag {
        Some(etag) => request.header("If-Match", etag),
        None => request.header("If-None-Match", "*"),
    };
    let (parts, _) = caldav
        .request_raw(request.body(data)?)
        .await
        .context("Failed to write CalDAV resource")?;
    check_write_status(parts.status)
}

pub(super) fn check_write_status(status: StatusCode) -> Result<()> {
    if status == StatusCode::PRECONDITION_FAILED {
        bail!("CalDAV resource changed concurrently (412); retry after fetching the latest state");
    }
    ensure!(
        status.is_success(),
        "Failed to write CalDAV resource: {status}"
    );
    Ok(())
}

pub(super) async fn write_event(
    username: &str,
    password: &str,
    calendar_url: &str,
    event: Event,
    create: bool,
) -> Result<Event> {
    let caldav = create_caldav_client(calendar_url, username, password)?;
    let resource = find_resource(&caldav, calendar_url, event.uid.as_str()).await?;
    let href = if let Some(resource) = resource {
        let data = merge(&resource.data, &event)
            .with_context(|| format!("Cannot modify CalDAV resource {}", resource.href))?;
        put(&caldav, &resource.href, Some(resource.etag()?), data).await?;
        resource.href
    } else {
        ensure!(create, "Cannot update missing CalDAV resource");
        let href = url_to_href(&event_url(calendar_url, event.uid.as_str()));
        put(&caldav, &href, None, event.to_ics_string()).await?;
        href
    };
    if let Ok(Some(resource)) = get_resource(&caldav, &href).await
        && let Ok(events) = parse_events(&resource.data)
        && let Some(fetched) = events
            .into_iter()
            .find(|e| e.event_instance_id() == event.event_instance_id())
    {
        return Ok(fetched);
    }
    Ok(event)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(extra: &str) -> String {
        format!("BEGIN:VEVENT\r\nUID:series\r\nDTSTART:20260921T070000Z\r\n{extra}END:VEVENT\r\n")
    }

    fn calendar(events: &str) -> String {
        format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\n{events}END:VCALENDAR\r\n")
    }

    #[test]
    fn preserves_calendar_and_untouched_component_bytes() {
        let master = event(
            "RRULE:FREQ=WEEKLY\r\nX-CUSTOM;X-PARAM=\"a:b\":long\r\n folded value\r\nATTACH;ENCODING=BASE64;VALUE=BINARY:YWJj\r\nBEGIN:VALARM\r\nACTION:AUDIO\r\nTRIGGER:-PT5M\r\nX-ALARM:keep\r\nEND:VALARM\r\n",
        );
        let override_ = event("RECURRENCE-ID:20260928T070000Z\r\n");
        let timezone = "BEGIN:VTIMEZONE\r\nTZID:Europe/London\r\nBEGIN:STANDARD\r\nDTSTART:19701025T020000\r\nTZOFFSETFROM:+0100\r\nTZOFFSETTO:+0000\r\nEND:STANDARD\r\nEND:VTIMEZONE\r\n";
        let mut replacement = parse_events(&calendar(&override_)).unwrap().remove(0);
        replacement.summary = Some("Moved".into());
        for newline in ["\r\n", "\n"] {
            for folding in [" ", "\t"] {
                for lowercase_boundaries in [false, true] {
                    let wire = |data: &str| {
                        let data = data
                            .replace("\r\n ", &format!("\r\n{folding}"))
                            .replace("\r\n", newline);
                        if lowercase_boundaries {
                            data.replace("BEGIN:", "begin:").replace("END:", "end:")
                        } else {
                            data
                        }
                    };
                    let prefix = wire(&format!(
                        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nX-CALENDAR:keep\r\n{timezone}{master}"
                    ));
                    let suffix = wire("END:VCALENDAR\r\n");
                    let data = format!("{prefix}{}{suffix}", wire(&override_));
                    let merged = merge(&data, &replacement).unwrap();
                    assert!(merged.starts_with(&prefix));
                    assert!(merged.ends_with(&suffix));
                    let events = parse_events(&merged).unwrap();
                    assert_eq!(events.len(), 2);
                    assert_eq!(events[1].summary.as_deref(), Some("Moved"));
                    let resource = Resource {
                        href: String::new(),
                        etag: None,
                        data: merged,
                    };
                    let Removal::Replace(remaining) =
                        resource.remove(&replacement.event_instance_id()).unwrap()
                    else {
                        panic!("Expected surviving master");
                    };
                    assert_eq!(remaining, format!("{prefix}{suffix}"));
                }
            }
        }
    }

    #[test]
    fn recurrence_identity_uses_core_equivalence() {
        for (stored, incoming) in [
            (";TZID=Europe/London:20260928T080000", ":20260928T070000Z"),
            (";VALUE=DATE:20260928", ";VALUE=DATE:20260928"),
            (":20260928T080000", ":20260928T080000"),
        ] {
            let data = calendar(&format!(
                "{}{}",
                event("RRULE:FREQ=WEEKLY\r\n"),
                event(&format!("RECURRENCE-ID{stored}\r\n"))
            ));
            let replacement = parse_events(&calendar(&event(&format!(
                "RECURRENCE-ID{incoming}\r\nSUMMARY:Changed\r\n"
            ))))
            .unwrap()
            .remove(0);
            let merged = merge(&data, &replacement).unwrap();
            let events = parse_events(&merged).unwrap();
            assert_eq!(events.len(), 2);
            assert_eq!(events[1].summary.as_deref(), Some("Changed"));
            let resource = Resource {
                href: String::new(),
                etag: None,
                data,
            };
            let Removal::Replace(remaining) =
                resource.remove(&replacement.event_instance_id()).unwrap()
            else {
                panic!("Expected surviving master");
            };
            assert_eq!(parse_events(&remaining).unwrap().len(), 1);
        }
    }

    #[test]
    fn refuses_malformed_or_ambiguous_resources() {
        for data in [
            "not ics".to_owned(),
            calendar(&event("")) + "garbage",
            calendar(&format!("{}{}", event(""), event(""))),
            calendar(&event("RECURRENCE-ID:invalid\r\n")),
            calendar(&event("recurrence-id:20260928T070000Z\r\n")),
            calendar(&event(&event(""))),
            calendar(&event("UID:other\r\n")),
            calendar(&event(
                "RECURRENCE-ID:20260928T070000Z\r\nRECURRENCE-ID:20261005T070000Z\r\n",
            )),
            calendar(&format!(
                "{}BEGIN:VEVENT\r\nUID:series\r\nEND:VEVENT\r\n",
                event("")
            )),
            calendar(&event("")).replace("END:VEVENT", "END:VTODO"),
            calendar(&format!(
                "{}{}",
                event(""),
                event("").replace("UID:series", "UID:other")
            )),
        ] {
            assert!(parse_events(&data).is_err(), "accepted {data}");
            let replacement = parse_events(&calendar(&event(""))).unwrap().remove(0);
            assert!(merge(&data, &replacement).is_err());
        }
    }

    #[test]
    fn multistatus_failures_are_not_empty_calendars() {
        for body in [
            "<error/>",
            r#"<multistatus xmlns="DAV:"><response><href>/a</href><status>HTTP/1.1 403 Forbidden</status></response></multistatus>"#,
            r#"<multistatus xmlns="DAV:"><response><href>/a</href><propstat><prop/><status>HTTP/1.1 200 OK</status></propstat></response></multistatus>"#,
        ] {
            assert!(parse_multistatus(body).is_err());
        }
    }
}

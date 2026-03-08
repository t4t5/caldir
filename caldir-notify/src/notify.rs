use notify_rust::Notification;

use caldir_core::event::Event;

pub fn send_notification(
    event: &Event,
    minutes_before: i64,
) -> Result<(), Box<dyn std::error::Error>> {
    let body = format_body(event, minutes_before);

    let mut notification = Notification::new();
    notification
        .appname("caldir")
        .summary(&event.summary)
        .body(&body);

    #[cfg(target_os = "macos")]
    notification.sound_name("Basso");

    notification.show()?;

    Ok(())
}

fn format_body(event: &Event, minutes_before: i64) -> String {
    let time_str = if minutes_before == 0 {
        "Now".to_string()
    } else if minutes_before == 1 {
        "In 1 minute".to_string()
    } else if minutes_before < 60 {
        format!("In {} minutes", minutes_before)
    } else if minutes_before == 60 {
        "In 1 hour".to_string()
    } else if minutes_before % 60 == 0 {
        format!("In {} hours", minutes_before / 60)
    } else {
        let hours = minutes_before / 60;
        let mins = minutes_before % 60;
        format!("In {}h {}m", hours, mins)
    };

    match &event.location {
        Some(loc) if !loc.is_empty() => format!("{}\n{}", time_str, loc),
        _ => time_str,
    }
}

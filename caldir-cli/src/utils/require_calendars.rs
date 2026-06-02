use caldir_core::Caldir;

pub fn require_calendars(caldir: &Caldir) -> Result<(), anyhow::Error> {
    if caldir.calendars().is_empty() {
        anyhow::bail!(
            "No calendars found.\n\n\
            Connect your first calendar with:\n  \
            caldir connect <provider>\n\n\
            Example:\n  \
            caldir connect google"
        );
    }

    Ok(())
}

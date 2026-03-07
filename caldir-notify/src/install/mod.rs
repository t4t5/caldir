#[cfg(target_os = "linux")]
mod systemd;
#[cfg(target_os = "macos")]
mod launchd;

pub fn install() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "linux")]
    systemd::install()?;

    #[cfg(target_os = "macos")]
    launchd::install()?;

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    return Err("Unsupported platform. Only Linux (systemd) and macOS (launchd) are supported.".into());

    Ok(())
}

pub fn uninstall() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "linux")]
    systemd::uninstall()?;

    #[cfg(target_os = "macos")]
    launchd::uninstall()?;

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    return Err("Unsupported platform. Only Linux (systemd) and macOS (launchd) are supported.".into());

    Ok(())
}

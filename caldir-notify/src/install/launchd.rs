use std::fs;
use std::path::PathBuf;
use std::process::Command;

const LABEL: &str = "com.caldir.notify";

fn binary_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let path = std::env::current_exe()?;
    Ok(path)
}

fn plist_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let home = dirs::home_dir().ok_or("Could not determine home directory")?;
    Ok(home.join(format!("Library/LaunchAgents/{}.plist", LABEL)))
}

pub fn install() -> Result<(), Box<dyn std::error::Error>> {
    let bin = binary_path()?;
    let plist = plist_path()?;

    let content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{bin}</string>
        <string>check</string>
    </array>
    <key>StartInterval</key>
    <integer>60</integer>
    <key>RunAtLoad</key>
    <true/>
    <key>StandardErrorPath</key>
    <string>/tmp/caldir-notify.err</string>
</dict>
</plist>
"#,
        label = LABEL,
        bin = bin.display()
    );

    if let Some(parent) = plist.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&plist, content)?;

    Command::new("launchctl")
        .args(["load", &plist.to_string_lossy()])
        .status()?;

    println!("Installed and loaded {}", LABEL);
    Ok(())
}

pub fn uninstall() -> Result<(), Box<dyn std::error::Error>> {
    let plist = plist_path()?;

    if plist.exists() {
        let _ = Command::new("launchctl")
            .args(["unload", &plist.to_string_lossy()])
            .status();
        fs::remove_file(&plist)?;
    }

    println!("Uninstalled {}", LABEL);
    Ok(())
}

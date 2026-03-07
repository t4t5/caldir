use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn binary_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let path = std::env::current_exe()?;
    Ok(path)
}

fn systemd_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let home = dirs::home_dir().ok_or("Could not determine home directory")?;
    Ok(home.join(".config/systemd/user"))
}

fn service_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(systemd_dir()?.join("caldir-notify.service"))
}

fn timer_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(systemd_dir()?.join("caldir-notify.timer"))
}

pub fn install() -> Result<(), Box<dyn std::error::Error>> {
    let bin = binary_path()?;
    let dir = systemd_dir()?;
    fs::create_dir_all(&dir)?;

    let service = format!(
        "[Unit]\n\
         Description=caldir reminder notifications\n\
         \n\
         [Service]\n\
         Type=oneshot\n\
         ExecStart={} check\n",
        bin.display()
    );

    let timer = "\
        [Unit]\n\
        Description=Run caldir-notify every 60 seconds\n\
        \n\
        [Timer]\n\
        OnBootSec=30\n\
        OnUnitActiveSec=60\n\
        AccuracySec=5s\n\
        \n\
        [Install]\n\
        WantedBy=timers.target\n";

    fs::write(service_path()?, service)?;
    fs::write(timer_path()?, timer)?;

    Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .status()?;

    Command::new("systemctl")
        .args(["--user", "enable", "--now", "caldir-notify.timer"])
        .status()?;

    println!("Installed and started caldir-notify.timer");
    Ok(())
}

pub fn uninstall() -> Result<(), Box<dyn std::error::Error>> {
    let _ = Command::new("systemctl")
        .args(["--user", "disable", "--now", "caldir-notify.timer"])
        .status();

    let service = service_path()?;
    let timer = timer_path()?;

    if service.exists() {
        fs::remove_file(&service)?;
    }
    if timer.exists() {
        fs::remove_file(&timer)?;
    }

    Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .status()?;

    println!("Uninstalled caldir-notify.timer");
    Ok(())
}

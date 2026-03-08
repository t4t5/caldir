# caldir-notify

Desktop notifications for your [caldir](https://caldir.org) reminders. When an event in your caldir directory has a reminder (`VALARM`), caldir-notify delivers a native OS notification when it comes due.

## Install

```bash
cargo install caldir-notify
```

## Setup

After installing, run:

```bash
caldir-notify install
```

This creates a lightweight check that runs every minute (systemd user timer on Linux, launchd agent on macOS).

## Commands

| Command | Description |
|---|---|
| `caldir-notify check` | Checks for due reminders and fire notifications (run by
systemd/launchd) |
| `caldir-notify install` | Install the system timer to run automatically |
| `caldir-notify uninstall` | Remove the system timer |

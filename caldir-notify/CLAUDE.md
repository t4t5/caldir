# caldir-notify

Desktop notification service for caldir reminders. Scans the caldir directory for events with VALARM reminders and fires OS notifications when they come due.

## Architecture

**Periodic timer, not a daemon.** The `caldir-notify` binary runs once, checks for due reminders, fires notifications, and exits. It's designed to be invoked every 60 seconds by a system timer (systemd on Linux, launchd on macOS).

### How reminder checking works (`check.rs`)

1. Load `Caldir` to find `calendar_dir`
2. Discover all calendars via `Caldir::calendars()`
3. For each calendar, load events in a window from -1h to +24h (covers any reasonable reminder offset)
4. For each event with reminders, compute trigger time: `event.start - reminder.minutes`
5. If the trigger time falls within the **last 60 seconds**, fire a notification

**No state file needed.** Since the timer runs every 60s, each reminder's trigger time lands in exactly one 60-second window. If a run is missed (e.g. laptop asleep), the reminder is silently skipped — no stale alerts.

### Notification delivery (`notify.rs`)

Uses `notify-rust` for cross-platform notifications (Linux D-Bus / macOS notification center).

- **Title**: Event summary (e.g. "Meeting with Alice")
- **Body**: Human-readable time (e.g. "In 30 minutes") + location if present
- **App name**: "caldir"

### Install/Uninstall (`install/`)

`caldir-notify install` and `caldir-notify uninstall` manage system-level scheduling:

- **Linux** (`install/systemd.rs`): Writes `~/.config/systemd/user/caldir-notify.{service,timer}` with a 60-second interval, enables via `systemctl --user`
- **macOS** (`install/launchd.rs`): Writes `~/Library/LaunchAgents/com.caldir.notify.plist` with `StartInterval=60`, loads via `launchctl`

Platform dispatch is compile-time via `#[cfg(target_os)]` in `install/mod.rs`.

## Commands

```bash
# Check for due reminders and fire notifications (what the timer runs)
caldir-notify

# Install the system timer to run automatically every 60s
caldir-notify install

# Remove the system timer
caldir-notify uninstall
```

## caldir-core types used

- `caldir_core::caldir::Caldir` — discover calendars
- `caldir_core::calendar::Calendar` — load events via `events_in_range()`
- `caldir_core::event::{Event, Reminder, EventTime}` — event data and reminder minutes

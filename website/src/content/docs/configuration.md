---
title: Configuration
description: Global and per-calendar configuration
order: 5
---

# Configuration

Global settings are configured in:

- `~/.config/caldir/config.toml` (Linux, macOS)
- `%APPDATA%/caldir/config.toml` (Windows)

Example config file:

```toml
# where your data is stored:
calendar_dir = "~/caldir"

# where new events get added:
default_calendar = "personal"

# default reminders for new events:
default_reminders = ["1h", "2h"]
```

By default, the config file has all options commented out.

## Per-calendar config

Each calendar stores its configuration in a local `config.toml`:

```toml
# ~/caldir/personal/.caldir/config.toml
name = "Personal"
color = "#4285f4"

[remote]
provider = "google"
google_account = "me@gmail.com"
google_calendar_id = "primary"
```

Calendars without a `.caldir/config.toml` or without a `[remote]` value are treated as offline calendars (not synced anywhere).

## Declarative setup

Instead of running `caldir connect`, you can write the config files yourself (or generate them, e.g. with Nix). For iCloud and CalDAV, add a password command to the calendar's `[remote]` section. caldir runs it on every sync and never stores the password.

```toml
# ~/caldir/personal/.caldir/config.toml
name = "Personal"

[remote]
provider = "icloud"
icloud_account = "me@icloud.com"
icloud_calendar_url = "https://caldav.icloud.com/123456789/calendars/home/"
icloud_password_command = ["op", "read", "op://Personal/iCloud/app-password"]
```

For iCloud, the command should print an [app-specific password](https://support.apple.com/en-us/102654). For CalDAV, set the login alongside it:

```toml
[remote]
provider = "caldav"
caldav_calendar_url = "https://caldav.fastmail.com/dav/calendars/user/alice@example.com/abc123/"
caldav_username = "alice@example.com"
caldav_password_command = ["pass", "show", "fastmail"]
```

The command is a list of arguments, run without a shell (use `["sh", "-c", "..."]` if you need one). Its output, minus one trailing newline, is the password. Secret managers that prompt should cache via their agent.

To find the calendar URL, run `caldir connect` once on any machine and copy it from the generated config.

### With Nix

The same setup with [Home Manager](https://github.com/nix-community/home-manager):

```nix
home.file."caldir/personal/.caldir/config.toml".source =
  (pkgs.formats.toml { }).generate "caldir-personal" {
    name = "Personal";
    remote = {
      provider = "icloud";
      icloud_account = "me@icloud.com";
      icloud_calendar_url = "https://caldav.icloud.com/123456789/calendars/home/";
      icloud_password_command = [ "op" "read" "op://Personal/iCloud/app-password" ];
      # or with sops-nix: [ "cat" config.sops.secrets.icloud.path ]
    };
  };
```

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

If you manage your dotfiles with Nix or similar, you can write the config yourself instead of running `caldir connect`. For iCloud and CalDAV, set a password command to read the password from your secret manager instead of caldir's storage:

```toml
# ~/caldir/personal/.caldir/config.toml
name = "Personal"

[remote]
provider = "icloud"
icloud_account = "me@icloud.com"
icloud_calendar_url = "https://caldav.icloud.com/123456789/calendars/home/"
icloud_password_command = ["pass", "show", "icloud"]
```

For CalDAV, use `caldav_password_command`, with `caldav_account` set to `user@host` (e.g. `me@fastmail.com@caldav.fastmail.com`). To find the calendar URL, run `caldir connect` once and copy it from the generated config.

With [Home Manager](https://github.com/nix-community/home-manager) and [sops-nix](https://github.com/Mic92/sops-nix):

```nix
home.file."caldir/personal/.caldir/config.toml".source =
  (pkgs.formats.toml { }).generate "caldir-personal" {
    name = "Personal";
    remote = {
      provider = "icloud";
      icloud_account = "me@icloud.com";
      icloud_calendar_url = "https://caldav.icloud.com/123456789/calendars/home/";
      icloud_password_command = [ "cat" config.sops.secrets.icloud.path ];
    };
  };
```

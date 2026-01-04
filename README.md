# caldir

The "file over app" philosophy for calendars.

```
~/calendar/
  work/
    2025-01-15T0900__standup.ics
    2025-01-15T1400__sprint-planning.ics
  personal/
    2025-01-18__birthday-party.ics
```

## Why?

Your calendar shouldn't live behind properietary apps or APIs. When it's just files:

**You can see it**
```bash
ls ~/calendar/work/2025-01*
```

**You can search it**
```bash
grep -l "alice" ~/calendar/**/*.ics
```

**You can version it**
```bash
cd ~/calendar && git log
```

**AI can read it**
```
You: "How many meetings did I have last week?"
Claude: *reads files directly* "You had 12 meetings..."
```

## Quick start

```bash
cargo install caldir-cli caldir-provider-google

caldir auth google

caldir pull

# Your calendar is now in ~/calendar/
```

## How is it different from pimsync?

- **Human-readable filenames**: pimsync uses UUIDs (`a1b2c3d4.ics`). caldir parses events to generate meaningful names (`2025-01-15T0900__standup.ics`).
- **Native APIs**: pimsync is CalDAV-only. caldir uses native REST APIs (Google, Microsoft Graph) for better performance and broader support.

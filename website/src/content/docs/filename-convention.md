---
title: Filename convention
description: How caldir names event files
order: 6
---

# Filename convention

caldir uses human-readable filenames for event files, unlike the [vdir spec](https://vdirsyncer.pimutils.org/en/stable/vdir.html) which uses opaque UUIDs.

## Format

**Timed events:** `YYYY-MM-DDTHHMM__slug.ics`

```
2025-03-20T1500__client-call.ics
2025-03-20T0900__team-standup.ics
```

**All-day events:** `YYYY-MM-DD__slug.ics`

```
2025-03-21__offsite.ics
2025-03-25__vacation.ics
```

## Slug generation

The slug is derived from the event title:
- Lowercased
- Spaces replaced with hyphens
- Special characters removed

If multiple events have the same date/time and title, a numeric suffix is added (`-2`, `-3`, etc.) to ensure uniqueness.

## Why human-readable filenames?

**`ls` shows your schedule** — no need for a special viewer to see what's on your calendar:

```bash
$ ls ~/calendar/work/
2025-03-20T0900__standup.ics
2025-03-20T1400__sprint-planning.ics
2025-03-21__offsite.ics
```

**grep works** — find events by date or keyword:

```bash
$ ls ~/calendar/work/ | grep 2025-03
```

**LLM-friendly** — AI assistants can read your calendar directory and understand it immediately, without parsing `.ics` file contents.

**Sorting works** — files sort chronologically by default.

**Tab completion** — start typing the date to find events in your shell.

## Standard .ics files

Despite the human-readable filenames, every event is a standard [RFC 5545](https://tools.ietf.org/html/rfc5545) `.ics` file. You can open them in any calendar app, move them around, or sync them with other tools. caldir is just a directory convention and a sync tool — there's no lock-in.

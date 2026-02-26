---
title: What is caldir?
description: Your calendar as a directory of plain text files
order: 0
---

# What is caldir?

caldir is a convention and a sync tool. Your calendar is a directory of `.ics` files, one event per file, with human-readable filenames:

```
~/caldir/
  home/
    2025-03-25T0900__dentist.ics
  work/
    2025-03-20T1500__client-call.ics
    2025-03-26T1400__sprint-planning.ics
```

It can sync bidirectionally with providers like Google Calendar and iCloud, so your events stay up to date in both directions.

## Why

Calendars already have an open, text-based format, `.ics` files, but we don't treat them as first-class. They're hidden behind APIs, proprietary sync layers, and typically only used for exports.

caldir puts them on disk where they're useful. `grep` can search it. A shell script can process it. An LLM can reason about it.

Unix works because everything is inspectable. Git works because it's files. Calendars should be the same.

## This doesn't replace cloud calendars

Most people still want services like Google Calendar. It syncs everywhere, works on phones, and is deeply embedded in how teams operate.

caldir doesn't replace that. It just flips the default:

- **Local files are the source of truth**
- Cloud calendars become sync targets

That way you can reason locally, automate freely, and you're never locked in.

## Standard .ics files

Every event is a standard [RFC 5545](https://tools.ietf.org/html/rfc5545) `.ics` file. You can open them in any calendar app, move them around, or sync them with other tools.

---
title: Overview
description: Your calendar as a directory of plain text files
order: 0
---

# File-over-app for calendars

Caldir is a tool for storing your calendar data as a directory of plaintext files:

```
~/caldir/
├── home/
│   └── 2025-03-25T0900__dentist.ics
└── work/
    ├── 2025-03-20T1500__client-call.ics
    └── 2025-03-26T1400__sprint-planning.ics
```

One event per file. Human-readable filenames.

It syncs with any provider ([Google Calendar](/docs/providers), iCloud, CalDAV...) using git-like pull/push actions.

## Why?

Calendar data today is typically hidden behind APIs and proprietary sync layers.

Caldir puts them on disk where they're useful. `grep` can search it. A shell script can process it. An LLM can reason about it.

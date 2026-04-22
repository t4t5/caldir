---
title: Overview
description: Your calendar as a directory of plain text files
order: 0
---

# Your calendar as a directory

Caldir is a tool for storing your calendar data as a directory of ICS files:

```
~/caldir/
├── google/
│   └── 2025-03-25T0900__dentist.ics
└── outlook/
    ├── 2025-03-20T1500__client-call.ics
    └── 2025-03-26T1400__sprint-planning.ics
```

It syncs with a range of providers (Google Calendar, iCloud, Outlook, CalDAV...) using git-like pull/push actions.

## Why?

Calendar data today is typically hidden behind APIs and proprietary sync layers.

By turning it into plaintext files on a disk, you can search your data with `grep`, automate it with a script, or use LLMs to analyze it.

It also makes it trivial to migrate your data from one provider to another.

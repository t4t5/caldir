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

Calendars today are often hidden behind APIs and proprietary sync layers, limiting what you can
do.

By turning them into simple plaintext files, you can search your data blazingly quickly with `grep`, and set up advanced workflows using scripts and LLMs.

It also makes it trivial to migrate your data from one provider to another.

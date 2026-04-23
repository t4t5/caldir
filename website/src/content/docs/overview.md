---
title: Overview
description: Your calendar as a directory of plain text files
order: 0
---

# Your calendar as a directory

Caldir is a tool for storing your calendar as a directory of ICS files:

```
~/caldir/
├── google/
│   ├── 2026-06-25T0900__dentist.ics
│   ├── 2026-06-28__johns-birthday.ics
│   └── ...
└── outlook/
    ├── 2026-06-20T1500__client-call.ics
    ├── 2026-06-26T1400__sprint-planning.ics
    └── ...
```

It syncs with cloud providers (Google Calendar, iCloud, Outlook, CalDAV...) using git-like pull/push actions.

## Why?

Calendars today are often hidden behind APIs and proprietary sync layers, limiting what you can
do.

By turning them into plaintext files, you can use tools like `grep` to search them, or set up advanced workflows using scripts and agents.

It also makes it really easy to migrate your data from one provider to another.

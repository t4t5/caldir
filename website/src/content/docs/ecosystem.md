---
title: Ecosystem
description: Apps, widgets, and community providers built around caldir
order: 6
---

# Ecosystem

Caldir is deliberately modular. Apps can build on its core library, integrations can work directly with calendar files, and new providers plug in as standalone executables, without changing caldir itself.

## Apps

### [renCal](https://rencal.org)

A free, open-source desktop calendar built on caldir. It provides a keyboard-first GUI for viewing and editing your events, with support for Linux and macOS.

## Widgets and integrations

### [Caldir widget for Omarchy](https://github.com/t4t5/omarchy-caldir-widget)

See upcoming events and join meetings from the Omarchy bar. The widget syncs through caldir, highlights the next meeting, and opens calendar events in renCal.

## Community providers

These providers can be installed separately to connect caldir to more calendar services:

- [Proton Calendar](https://github.com/t4t5/caldir-provider-proton) — sync calendars from a Proton account
- [Tuta Calendar](https://github.com/t4t5/caldir-provider-tuta) — sync calendars from a Tuta account
- [AT Protocol](https://github.com/t4t5/caldir-provider-atproto) — subscribe to public AT Protocol calendar feeds

Any executable named `caldir-provider-*` on your `$PATH` works with caldir. See the [plugin architecture](/providers#plugin-architecture) to build your own.

## Add your project

Built something around caldir? [Open a pull request](https://github.com/t4t5/caldir/edit/main/website/src/content/docs/ecosystem.md) to add it here.

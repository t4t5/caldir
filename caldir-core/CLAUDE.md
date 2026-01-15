# caldir-core

Core library for the caldir ecosystem. Contains all business logic, types, and sync algorithms.

## Design Principles

- **No TUI dependencies**: Colors, spinners, etc. belong in caldir-cli
- **Serializable types**: Core types derive `Serialize`/`Deserialize` for HTTP transport
- **Provider-agnostic**: Works with any provider that implements the protocol

## Key Modules

### Types (`event.rs`)
Provider-neutral event types: `Event`, `EventTime`, `Attendee`, `Reminder`, `ParticipationStatus`, etc.

### Calendar Management
- `caldir.rs` — `Caldir` struct discovers calendars in the data directory
- `calendar.rs` — `Calendar` struct for CRUD operations on a single calendar
- `config.rs` — `GlobalConfig` from `~/.config/caldir/config.toml`

### Sync (`diff/`)
- `EventDiff` — Represents a single change (create/update/delete) with old and new event
- `CalendarDiff` — Collection of changes for one calendar (to_push, to_pull)
- `DiffKind` — Enum: Create, Update, Delete

All diff types are serializable for HTTP transport between server and CLI.

### Provider Protocol (`protocol.rs`, `provider.rs`)
JSON-over-stdin/stdout protocol for communicating with provider binaries.

### ICS (`ics/`)
RFC 5545 compliant ICS file generation and parsing.

### Local State (`local/`)
- `LocalConfig` — Per-calendar config in `.caldir/config.toml`
- `LocalState` — Sync state in `.caldir/state/`
- `LocalEvent` — Event with local metadata (file path, mtime)

## Usage

Both `caldir-server` and `caldir-provider-*` crates depend on this library.

```rust
use caldir_core::{Caldir, Calendar, Event, EventTime};
use caldir_core::diff::{EventDiff, DiffKind};
```

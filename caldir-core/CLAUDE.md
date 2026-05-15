# caldir-core

The library that holds all of caldir's logic — event types, calendar discovery, sync, ICS round-tripping, the provider subprocess protocol. CLIs and GUIs sit on top of this.

## Testability

caldir-core is designed to be fully testable without global state: every component takes the resources it needs as arguments. Production calls `Caldir::load()` to read from disk; tests use the `test_utils` helpers (e.g. `test_caldir()`) to construct one with `Caldir::new(config, providers)` against in-memory state.

## Adding properties to `Event`

- New `Event` properties get a paired test in `from_icalendar` and `to_icalendar`. Order the tests to match the struct field order — easier to scan for omissions.
- If the property has its own type (`Organizer`, `EventTime`, etc.), put the icalendar `From`/`TryFrom` impls and the detailed parameter/edge-case tests in that type's module. The `from_icalendar`/`to_icalendar` tests then only need a single wire-through case per direction confirming the field is plumbed.

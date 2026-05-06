This is a rewrite of @../caldir-core-old, aiming to make everything more simple and testable by reducing global state.

## Rules

- When adding a new property on `Event`, make sure we also have corresponding tests in
`from_icalendar` and `to_icalendar`. The tests should be in the same order as the properties on the struct.
- If the property has its own dedicated struct (e.g. `Organizer`, `EventTime`), put the `From`/`TryFrom` conversions to/from icalendar types in that struct's module, and colocate the detail tests (parameter handling, edge cases) there. The `from_icalendar`/`to_icalendar` tests then only need a single wire-through case per direction confirming the field is plumbed.

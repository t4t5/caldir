This is a rewrite of @../caldir-core-old, aiming to make everything more simple and testable by reducing global state.

## Rules

- When adding a new property on `Event`, make sure we also have corresponding tests in
`from_icalendar` and `to_icalendar`. The tests should be in the same order as the properties on the struct.

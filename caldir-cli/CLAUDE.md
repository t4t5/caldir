# caldir-cli

Thin HTTP client for interacting with caldir-server.

## Design

- **Thin client**: All business logic is in caldir-server, CLI just makes HTTP calls
- **Auto-starts server**: If caldir-server isn't running, CLI spawns it automatically
- **Render trait**: Extends caldir-core types with terminal formatting (colors)

## Key Modules

### `client.rs`
HTTP client that talks to caldir-server:
- `Client::connect()` — Connect to server, starting it if needed
- Methods for each API endpoint: `pull()`, `push()`, `status()`, `authenticate()`, etc.

### `render.rs`
`Render` trait for terminal output with colors:
```rust
pub trait Render {
    fn render(&self) -> String;
}

impl Render for EventDiff { ... }  // Colored diff output
impl Render for DiffKind { ... }   // +/~/- symbols
```

### `commands/`
Each command is a thin wrapper that:
1. Connects to server via `Client::connect()`
2. Calls the appropriate client method
3. Renders the response using `Render` trait

## Dependencies

- `caldir-core` — For types only (EventDiff, EventTime, etc.)
- `reqwest` — HTTP client
- `owo-colors` — Terminal colors
- `indicatif` — Spinners

The CLI does NOT depend on caldir-core's business logic directly.

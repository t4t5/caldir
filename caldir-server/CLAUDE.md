# caldir-server

HTTP server that exposes the caldir-core library as a REST API. Runs as a singleton daemon.

## Design

- **Singleton pattern**: Uses file lock (`~/.cache/caldir/server.lock`) to ensure only one instance runs
- **Auto-started**: CLI starts the server if not running
- **Stateless requests**: Each request loads fresh state from filesystem
- **CORS enabled**: Allows cross-origin requests for GUI apps

## API Endpoints

### Calendars
- `GET /calendars` — List all calendars (includes `is_default` flag)
- `GET /calendars/:id/events` — List events in a calendar
- `POST /calendars/:id/events` — Create a new event

### Remote Sync
- `POST /remote/pull` — Pull changes from remote for all calendars
- `POST /remote/push` — Push changes to remote for all calendars
- `GET /remote/status` — Get pending changes (to push and to pull)

### Authentication
- `POST /auth/:provider` — Authenticate with a provider, creates calendar directories

## Response Types

Sync endpoints return per-calendar results with `Vec<EventDiff>` for detailed change info:

```rust
struct SyncResult {
    calendar: String,
    events: Vec<EventDiff>,  // From caldir-core, serializable
    error: Option<String>,
}
```

## Running

```bash
# Install to PATH
just install-server

# Or run directly during development
cargo run -p caldir-server
```

Server listens on `http://127.0.0.1:4096` by default.

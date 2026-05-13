# Building a provider

A provider is a binary that speaks caldir's JSON RPC over stdin/stdout. Implement [`ProviderHandler`](../caldir-core/src/rpc/handler.rs) and let `run_provider` handle the protocol.

## Minimal `main.rs`

```rust
use async_trait::async_trait;
use caldir_core::rpc::{Connect, ConnectResponse, HandlerResult, ProviderHandler};

struct MyProvider;

#[async_trait]
impl ProviderHandler for MyProvider {
    async fn connect(&self, cmd: Connect) -> HandlerResult<ConnectResponse> { ... }
}

#[tokio::main]
async fn main() {
    caldir_core::rpc::run_provider(MyProvider).await
}
```

Single-calendar providers skip `list_calendars` and return the calendar from `connect` instead.

## File layout

```
src/
├── main.rs            # ProviderHandler impl + run_provider — no stdin/stdout, no dispatch
├── commands/          # one file per RPC; each exports `async fn handle(cmd) -> anyhow::Result<...>`
├── remote_config.rs   # typed wrapper over RemoteConfigParams (TryFrom + into_remote_config_params)
└── constants.rs       # PROVIDER_NAME and similar
```

Keep command handlers thin and IO-free where possible — pull HTTP, file IO, and parsing into separate modules (e.g. `http.rs`, `feed.rs`) so the logic can be unit-tested without the runner.

## Errors

`HandlerResult<T>` is `Result<T, Box<dyn Error + Send + Sync>>`. Command handlers should return `anyhow::Result<T>` internally for ergonomics — `?` converts at the trait boundary, and the runner walks `.source()` so `anyhow::Context` chains end up in the response message.

`caldir-core` itself stays anyhow-free; only providers (which are binaries, not libraries) pull anyhow in. A shared library crate like `caldir-provider-caldav` should use `thiserror` instead.

## Storage directory

Providers that need on-disk state (OAuth tokens, app passwords, sync cursors) write to the path in `CALDIR_PROVIDER_STORAGE_DIR`, falling back to `~/.config/caldir/providers/{name}/` if unset.

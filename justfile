auth:
  cargo run -- auth

pull:
  cargo run -- pull

status:
  cargo run -- status --verbose

# Cargo

check:
  cargo check && cargo clippy

test:
  cargo test

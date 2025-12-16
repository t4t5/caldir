cli +args:
  cargo run -- {{ args }}

auth:
  cargo run -- auth

pull:
  cargo run -- pull

status:
  cargo run -- status --verbose

push:
  cargo run -- push

# Cargo

check:
  cargo check && cargo clippy

test:
  cargo test

# List all available tasks
default:
  @just --list

cli +args:
  cargo run -- {{ args }}

auth:
  @just cli auth

pull:
  @just cli pull

status:
  @just cli status --verbose

push:
  @just cli push

# Cargo

check:
  cargo check --workspace && cargo clippy --workspace

test:
  cargo test

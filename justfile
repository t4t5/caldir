# List all available tasks
default:
  @just --list

cli +args:
  cargo run -p caldir-cli -- {{ args }}

auth:
  @just cli auth

pull:
  @just cli pull

status:
  @just cli status

push:
  @just cli push

# Cargo

check:
  cargo check --workspace && cargo clippy --workspace

test:
  cargo test

# Install provider binary to PATH
install-provider:
  cargo install --path caldir-provider-google

# Build and install everything
install: install-provider
  cargo install --path caldir-cli

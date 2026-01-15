# List all available tasks
default:
  @just --list

cli +args:
  cargo run -p caldir-cli -- {{ args }}

auth +args:
  @just cli auth {{ args }}

pull:
  @just cli pull

status:
  @just cli status

push:
  @just cli push

new +args:
  @just cli new {{ args }}

# Cargo

check:
  cargo check --workspace && cargo clippy --workspace

test:
  cargo test

# Install provider binary to PATH
install-provider:
  cargo install --path caldir-provider-google

# Install server binary to PATH
install-server:
  cargo install --path caldir-server

# Build and install everything
install: install-provider install-server
  cargo install --path caldir-cli

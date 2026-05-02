# List all available tasks
default:
  @just --list

format:
  cargo fmt --all

# Test app commands:

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

# Dev tools:

check:
  cargo check --workspace
  cargo clippy --workspace -- -D warnings

test:
  cargo test

# Install provider binary to PATH
install-provider:
  cargo install --path caldir-provider-caldav
  cargo install --path caldir-provider-google
  cargo install --path caldir-provider-icloud
  cargo install --path caldir-provider-outlook
  cargo install --path caldir-provider-webcal

# Build and install everything
install: install-provider
  cargo install --path caldir-cli

# Remove all installed binaries
uninstall:
  rm -f ~/.cargo/bin/caldir ~/.cargo/bin/caldir-provider-*

# Serve website locally
serve:
  cd website && npm run dev

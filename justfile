# List all available tasks
default:
  @just --list

# Lint check
check:
  cargo check --workspace && cargo clippy --workspace

# Run tests
test:
  cargo test

# Install provider binary to PATH
install-providers:
  cargo install --path caldir-provider-caldav
  cargo install --path caldir-provider-google
  cargo install --path caldir-provider-icloud
  cargo install --path caldir-provider-outlook

# Build and install all binaries
install: install-providers
  cargo install --path caldir-cli

# Serve website locally
serve:
  cd website && npm run dev

# Create a test event with a 5m reminder and immediately check for notifications
test-notification:
  #!/usr/bin/env bash
  in_5_mins=$(date -d '+5 minutes' +%H:%M)
  cargo run -p caldir-cli -- new "Test notification" --start "today ${in_5_mins}" --reminder 5m
  cargo run -p caldir-notify


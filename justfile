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
  cargo install --path caldir-notify

# Serve website locally
serve:
  cd website && npm run dev

# Create test event set to 5 mins from now
# with a 5m reminder (and immediately check for notifications)
test-notification:
  #!/usr/bin/env bash
  in_5_mins=$(date -d '+5 minutes' +%H:%M)
  cargo run -p caldir-cli -- new "Test notification" --start "today ${in_5_mins}" --reminder 5m
  cargo run -p caldir-notify -- check

# Create a test event set to 5 mins from now
# with a 4m reminder (to see if systemd notification fires)
test-notification-systemd:
  #!/usr/bin/env bash
  in_5_mins=$(date -d '+5 minutes' +%H:%M)
  cargo run -p caldir-cli -- new "Test notification" --start "today ${in_5_mins}" --reminder 4m

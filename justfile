# List all available tasks
default:
  @just --list

# TODO: Remove me after refactor:
[working-directory: 'caldir-core']
check:
  cargo check
  cargo clippy

# TODO: Remove me after refactor:
[working-directory: 'caldir-core']
test:
  cargo test

# TODO: Remove me after refactor:
[working-directory: 'caldir-core']
format:
  cargo fmt

# check:
#   cargo check --workspace
#   cargo clippy --workspace -- -D warnings

# test:
#   cargo test

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

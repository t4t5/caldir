# List all available tasks
default:
  @just --list

# Run CLI commands
cli +args:
  cargo run -p caldir-cli -- {{ args }}

format:
  cargo fmt --all

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
web:
  cd website && npm run dev

# Force deploy website
deploy-web:
  gh workflow run website.yml --ref main
  gh run watch

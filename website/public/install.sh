#!/bin/sh
# caldir installer
# Usage: curl -sSf https://caldir.org/install.sh | sh
set -eu

REPO="t4t5/caldir"
INSTALL_DIR="${HOME}/.local/bin"

main() {
  os=$(uname -s)
  arch=$(uname -m)

  case "$os" in
    Darwin) target_os="apple-darwin" ;;
    Linux)  target_os="unknown-linux-musl" ;;
    *)
      echo "Error: unsupported OS: $os" >&2
      exit 1
      ;;
  esac

  case "$arch" in
    x86_64)  target_arch="x86_64" ;;
    aarch64|arm64) target_arch="aarch64" ;;
    *)
      echo "Error: unsupported architecture: $arch" >&2
      exit 1
      ;;
  esac

  target="${target_arch}-${target_os}"

  echo "Detecting platform: ${target}"

  # Fetch latest version from GitHub API
  # Try the API first, fall back to the releases redirect for rate-limited IPs
  api_response=$(curl -sSf "https://api.github.com/repos/${REPO}/releases/latest" 2>&1) || true
  version=$(echo "$api_response" | grep '"tag_name"' | sed 's/.*"tag_name": *"//;s/".*//')

  if [ -z "$version" ]; then
    # Fallback: use the redirect from /releases/latest to extract the version
    version=$(curl -sSfI "https://github.com/${REPO}/releases/latest" 2>&1 | grep -i '^location:' | sed 's|.*/tag/||;s/[[:space:]]*$//')
  fi

  if [ -z "$version" ]; then
    echo "Error: could not determine latest version (GitHub API may be rate-limiting your IP)" >&2
    exit 1
  fi

  echo "Latest version: ${version}"

  tarball="caldir-${target}.tar.gz"
  url="https://github.com/${REPO}/releases/download/${version}/${tarball}"

  # Create temp directory
  tmp=$(mktemp -d)
  trap 'rm -rf "$tmp"' EXIT

  echo "Downloading ${url}..."
  curl -sSfL "$url" -o "${tmp}/${tarball}"

  echo "Extracting..."
  tar -xzf "${tmp}/${tarball}" -C "$tmp"

  # Install binaries
  mkdir -p "$INSTALL_DIR"

  # Install everything the release tarball ships — the tarball is the source
  # of truth for which binaries make up a caldir install.
  for binary in "${tmp}"/caldir "${tmp}"/caldir-provider-*; do
    [ -f "$binary" ] || continue
    name=$(basename "$binary")
    install -m 755 "$binary" "${INSTALL_DIR}/${name}"
    echo "  Installed ${name} to ${INSTALL_DIR}/${name}"
  done

  echo ""
  echo "caldir ${version} installed successfully!"

  # Check if install dir is in PATH
  case ":${PATH}:" in
    *":${INSTALL_DIR}:"*) ;;
    *)
      echo ""
      echo "WARNING: ${INSTALL_DIR} is not in your PATH."
      echo "Add this to your shell profile:"
      echo ""
      echo "  export PATH=\"${INSTALL_DIR}:\$PATH\""
      ;;
  esac
}

main

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

  for bin in caldir caldir-provider-google caldir-provider-icloud; do
    if [ -f "${tmp}/${bin}" ]; then
      install -m 755 "${tmp}/${bin}" "${INSTALL_DIR}/${bin}"
      echo "  Installed ${bin} to ${INSTALL_DIR}/${bin}"
    fi
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

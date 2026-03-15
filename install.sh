#!/bin/sh
# Install kubo — isolated dev environments in Docker.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/Dorky-Robot/kubo/main/install.sh | sh
#
# Requires: curl or wget, tar, and Docker.

set -e

REPO="Dorky-Robot/kubo"
INSTALL_DIR="${KUBO_INSTALL_DIR:-/usr/local/bin}"

# Detect OS and architecture
detect_platform() {
  os="$(uname -s)"
  arch="$(uname -m)"

  case "$os" in
    Linux)  os="unknown-linux-gnu" ;;
    Darwin) os="apple-darwin" ;;
    *)      echo "Unsupported OS: $os" >&2; exit 1 ;;
  esac

  case "$arch" in
    x86_64|amd64)  arch="x86_64" ;;
    aarch64|arm64) arch="aarch64" ;;
    *)             echo "Unsupported architecture: $arch" >&2; exit 1 ;;
  esac

  echo "${arch}-${os}"
}

# Get latest release tag from GitHub
latest_tag() {
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p'
  elif command -v wget >/dev/null 2>&1; then
    wget -qO- "https://api.github.com/repos/${REPO}/releases/latest" | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p'
  else
    echo "Need curl or wget" >&2; exit 1
  fi
}

download() {
  url="$1"; dest="$2"
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$url" -o "$dest"
  else
    wget -qO "$dest" "$url"
  fi
}

main() {
  platform="$(detect_platform)"
  tag="${KUBO_VERSION:-$(latest_tag)}"

  if [ -z "$tag" ]; then
    echo "Error: could not determine latest release." >&2
    exit 1
  fi

  url="https://github.com/${REPO}/releases/download/${tag}/kubo-${tag}-${platform}.tar.gz"
  echo "Downloading kubo ${tag} for ${platform}..."

  tmpdir="$(mktemp -d)"
  trap 'rm -rf "$tmpdir"' EXIT

  download "$url" "${tmpdir}/kubo.tar.gz"
  tar xzf "${tmpdir}/kubo.tar.gz" -C "$tmpdir"

  # The archive contains kubo-<tag>-<platform>/kubo
  binary="$(find "$tmpdir" -name kubo -type f | head -1)"
  if [ -z "$binary" ]; then
    echo "Error: kubo binary not found in archive." >&2
    exit 1
  fi
  chmod +x "$binary"

  # Install — use sudo if needed
  if [ -w "$INSTALL_DIR" ]; then
    mv "$binary" "${INSTALL_DIR}/kubo"
  else
    echo "Installing to ${INSTALL_DIR} (requires sudo)..."
    sudo mv "$binary" "${INSTALL_DIR}/kubo"
  fi

  echo "kubo ${tag} installed to ${INSTALL_DIR}/kubo"
  echo ""
  echo "Get started:  kubo ."
}

main

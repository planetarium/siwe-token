#!/bin/sh
set -eu

REPO="planetarium/siwe-token"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"

# Detect OS and architecture
detect_platform() {
  OS="$(uname -s)"
  ARCH="$(uname -m)"

  case "$OS" in
    Linux)  OS_TAG="unknown-linux-musl" ;;
    Darwin) OS_TAG="apple-darwin" ;;
    *)      echo "Error: unsupported OS: $OS" >&2; exit 1 ;;
  esac

  case "$ARCH" in
    x86_64|amd64)  ARCH_TAG="x86_64" ;;
    arm64|aarch64) ARCH_TAG="aarch64" ;;
    *)             echo "Error: unsupported architecture: $ARCH" >&2; exit 1 ;;
  esac

  TARGET="${ARCH_TAG}-${OS_TAG}"
}

# Get latest release tag from GitHub API
get_latest_version() {
  if command -v curl >/dev/null 2>&1; then
    VERSION="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/')"
  elif command -v wget >/dev/null 2>&1; then
    VERSION="$(wget -qO- "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/')"
  else
    echo "Error: curl or wget is required" >&2
    exit 1
  fi

  if [ -z "$VERSION" ]; then
    echo "Error: could not determine latest version" >&2
    exit 1
  fi
}

download_and_install() {
  ARCHIVE="siwe-token-${TARGET}.tar.gz"
  URL="https://github.com/${REPO}/releases/download/${VERSION}/${ARCHIVE}"

  echo "Downloading siwe-token ${VERSION} for ${TARGET}..."

  TMPDIR="$(mktemp -d)"
  trap 'rm -rf "$TMPDIR"' EXIT

  if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$URL" -o "${TMPDIR}/${ARCHIVE}"
  else
    wget -q "$URL" -O "${TMPDIR}/${ARCHIVE}"
  fi

  tar xzf "${TMPDIR}/${ARCHIVE}" -C "$TMPDIR"

  if [ -w "$INSTALL_DIR" ]; then
    mv "${TMPDIR}/siwe-token" "${INSTALL_DIR}/siwe-token"
  else
    echo "Installing to ${INSTALL_DIR} (requires sudo)..."
    sudo mv "${TMPDIR}/siwe-token" "${INSTALL_DIR}/siwe-token"
  fi

  chmod +x "${INSTALL_DIR}/siwe-token"
  echo "Installed siwe-token ${VERSION} to ${INSTALL_DIR}/siwe-token"
}

main() {
  detect_platform
  get_latest_version
  download_and_install
}

main

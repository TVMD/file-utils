#!/bin/bash
set -e

REPO="TVMD/file-utils"
BINARY="fus"
INSTALL_DIR="/usr/local/bin"

# Detect OS and architecture
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
    Linux)  OS_TAG="linux" ;;
    Darwin) OS_TAG="macos" ;;
    *)      echo "Unsupported OS: $OS"; exit 1 ;;
esac

case "$ARCH" in
    x86_64)  ARCH_TAG="x86_64" ;;
    aarch64|arm64) ARCH_TAG="aarch64" ;;
    *)       echo "Unsupported architecture: $ARCH"; exit 1 ;;
esac

ASSET="${BINARY}-${OS_TAG}-${ARCH_TAG}.tar.gz"

echo "Fetching latest release..."
TAG="$(curl -sI "https://github.com/${REPO}/releases/latest" \
    | grep -i '^location:' \
    | sed 's|.*/tag/||' \
    | tr -d '\r\n')"

if [ -z "$TAG" ]; then
    echo "Error: Could not determine latest release tag"
    exit 1
fi

DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${TAG}/${ASSET}"

# Check if already installed and up to date
CURRENT_VERSION=""
if command -v "$BINARY" >/dev/null 2>&1; then
    CURRENT_VERSION="$("$BINARY" --version 2>/dev/null | awk '{print $2}')"
    LATEST_VERSION="${TAG#v}"
    if [ "$CURRENT_VERSION" = "$LATEST_VERSION" ]; then
        echo "fus ${CURRENT_VERSION} is already the latest version."
        exit 0
    fi
    echo "Updating fus ${CURRENT_VERSION} → ${LATEST_VERSION}..."
else
    echo "Installing fus ${TAG#v}..."
fi

# Verify the asset exists
HTTP_CODE="$(curl -sL -o /dev/null -w '%{http_code}' "$DOWNLOAD_URL")"
if [ "$HTTP_CODE" != "200" ]; then
    echo "Error: Could not find release asset ${ASSET} for ${TAG}"
    exit 1
fi

TMPDIR="$(mktemp -d)"
trap 'rm -rf "$TMPDIR"' EXIT

echo "Downloading ${ASSET}..."
curl -sL "$DOWNLOAD_URL" -o "${TMPDIR}/${ASSET}"

echo "Extracting..."
tar -xzf "${TMPDIR}/${ASSET}" -C "$TMPDIR"

echo "Installing to ${INSTALL_DIR}..."
if [ -w "$INSTALL_DIR" ]; then
    mv -f "${TMPDIR}/${BINARY}" "${INSTALL_DIR}/${BINARY}"
else
    sudo mv -f "${TMPDIR}/${BINARY}" "${INSTALL_DIR}/${BINARY}"
fi

chmod +x "${INSTALL_DIR}/${BINARY}"

echo "Installed fus ${TAG#v} to ${INSTALL_DIR}/${BINARY}"
echo "Run 'fus --help' to get started."

#!/usr/bin/env sh
set -eu

REPO="${EXCAL_REPO:-TDHSC/excalidraw-spatial}"
BIN_NAME="excal"
INSTALL_DIR="${EXCAL_INSTALL_DIR:-$HOME/.local/bin}"
VERSION="${EXCAL_VERSION:-latest}"

usage() {
    cat <<'EOF'
Install the excal CLI binary from GitHub Releases.

Usage:
    install.sh [--dir <DIR>] [--version <TAG>]

Environment variables:
    EXCAL_INSTALL_DIR   Installation directory (default: $HOME/.local/bin)
    EXCAL_VERSION       Release tag such as v0.1.0, or "latest" (default: latest)
    EXCAL_REPO          GitHub repository slug (default: TDHSC/excalidraw-spatial)
EOF
}

while [ $# -gt 0 ]; do
    case "$1" in
        -b | --dir | --bin-dir)
            INSTALL_DIR="$2"
            shift 2
            ;;
        -v | --version)
            VERSION="$2"
            shift 2
            ;;
        -h | --help)
            usage
            exit 0
            ;;
        *)
            echo "excal-install: unknown argument: $1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
    Darwin)
        OS_TRIPLE="apple-darwin"
        ;;
    Linux)
        OS_TRIPLE="unknown-linux-gnu"
        ;;
    *)
        echo "excal-install: unsupported operating system: $OS" >&2
        exit 1
        ;;
esac

case "$ARCH" in
    arm64 | aarch64)
        ARCH_TRIPLE="aarch64"
        ;;
    x86_64 | amd64)
        ARCH_TRIPLE="x86_64"
        ;;
    *)
        echo "excal-install: unsupported architecture: $ARCH" >&2
        exit 1
        ;;
esac

TARGET="${ARCH_TRIPLE}-${OS_TRIPLE}"
ARCHIVE="${BIN_NAME}-${TARGET}.tar.gz"

if [ "$VERSION" = "latest" ]; then
    URL="https://github.com/${REPO}/releases/latest/download/${ARCHIVE}"
else
    case "$VERSION" in
        v*) TAG="$VERSION" ;;
        *) TAG="v$VERSION" ;;
    esac
    URL="https://github.com/${REPO}/releases/download/${TAG}/${ARCHIVE}"
fi

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT INT TERM

echo "Downloading ${BIN_NAME} (${TARGET}) from ${URL}..."
if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$URL" -o "${TMP_DIR}/${ARCHIVE}"
elif command -v wget >/dev/null 2>&1; then
    wget -qO "${TMP_DIR}/${ARCHIVE}" "$URL"
else
    echo "excal-install: either curl or wget is required" >&2
    exit 1
fi

tar -xzf "${TMP_DIR}/${ARCHIVE}" -C "$TMP_DIR"
mkdir -p "$INSTALL_DIR"
install -m 755 "${TMP_DIR}/${BIN_NAME}" "${INSTALL_DIR}/${BIN_NAME}"

echo "Installed ${BIN_NAME} to ${INSTALL_DIR}/${BIN_NAME}"

case ":${PATH}:" in
    *":${INSTALL_DIR}:"*) ;;
    *)
        echo ""
        echo "Note: ${INSTALL_DIR} is not in your PATH. Add it to your shell profile:"
        echo "  export PATH=\"${INSTALL_DIR}:\$PATH\""
        ;;
esac

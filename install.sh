#!/bin/sh
# stopproof installer — downloads the latest release binary.
# Usage: curl -fsSL https://raw.githubusercontent.com/Lapmlnex/stopproof/main/install.sh | sh
set -eu

REPO="Lapmlnex/stopproof"
INSTALL_DIR="${STOPPROOF_INSTALL_DIR:-$HOME/.local/bin}"

os="$(uname -s)"
arch="$(uname -m)"

case "$os" in
  Linux)
    case "$arch" in
      x86_64) target="x86_64-unknown-linux-musl" ;;
      *)
        echo "stopproof: no prebuilt binary for Linux/$arch yet." >&2
        echo "Build from source: cargo install --git https://github.com/$REPO" >&2
        exit 1
        ;;
    esac
    ;;
  Darwin)
    case "$arch" in
      arm64) target="aarch64-apple-darwin" ;;
      *)     target="x86_64-apple-darwin" ;;
    esac
    ;;
  *)
    echo "stopproof: unsupported OS '$os'." >&2
    echo "On Windows, download stopproof-x86_64-pc-windows-msvc.exe from:" >&2
    echo "  https://github.com/$REPO/releases/latest" >&2
    echo "Or build from source: cargo install --git https://github.com/$REPO" >&2
    exit 1
    ;;
esac

url="https://github.com/$REPO/releases/latest/download/stopproof-$target"
mkdir -p "$INSTALL_DIR"

echo "Downloading $url ..."
if command -v curl >/dev/null 2>&1; then
  curl -fsSL "$url" -o "$INSTALL_DIR/stopproof"
elif command -v wget >/dev/null 2>&1; then
  wget -q "$url" -O "$INSTALL_DIR/stopproof"
else
  echo "stopproof: need curl or wget. Alternatively: cargo install --git https://github.com/$REPO" >&2
  exit 1
fi

chmod +x "$INSTALL_DIR/stopproof"
echo "Installed: $INSTALL_DIR/stopproof"
"$INSTALL_DIR/stopproof" --version || true

case ":$PATH:" in
  *":$INSTALL_DIR:"*) ;;
  *)
    echo ""
    echo "NOTE: $INSTALL_DIR is not on your PATH. Add it, e.g.:"
    echo "  export PATH=\"$INSTALL_DIR:\$PATH\""
    ;;
esac

echo ""
echo "Next: cd into a project and run: stopproof init"

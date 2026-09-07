#!/bin/sh
# Downloads a release into a temporary directory, verifies SHA256SUMS, then
# replaces the executable. Existing installations survive download/check failures.
set -eu

REPO="Lapmlnex/stopproof"
INSTALL_DIR="${STOPPROOF_INSTALL_DIR:-$HOME/.local/bin}"
VERSION="${STOPPROOF_VERSION:-v0.2.0}"

fail() { echo "stopproof: $*" >&2; exit 1; }

case "$VERSION" in
  *[!a-zA-Z0-9._-]*) fail "invalid STOPPROOF_VERSION; use a release tag such as v0.2.0 or latest" ;;
esac

os="$(uname -s)"
arch="$(uname -m)"
case "$os/$arch" in
  Linux/x86_64) target="x86_64-unknown-linux-musl" ;;
  Darwin/arm64) target="aarch64-apple-darwin" ;;
  Darwin/x86_64) target="x86_64-apple-darwin" ;;
  *)
    fail "no installer binary for $os/$arch. See https://github.com/$REPO/releases or build from source."
    ;;
esac

if command -v sha256sum >/dev/null 2>&1; then
  hash_tool="sha256sum"
elif command -v shasum >/dev/null 2>&1; then
  hash_tool="shasum"
else
  fail "need sha256sum or shasum to verify the download"
fi

if command -v curl >/dev/null 2>&1; then
  download() { curl --proto '=https' --tlsv1.2 -fsSL "$1" -o "$2"; }
elif command -v wget >/dev/null 2>&1; then
  download() { wget --https-only -q "$1" -O "$2"; }
else
  fail "need curl or wget; alternatively build from source"
fi

if [ "$VERSION" = "latest" ]; then
  base_url="https://github.com/$REPO/releases/latest/download"
else
  base_url="https://github.com/$REPO/releases/download/$VERSION"
fi
asset="stopproof-$target"
mkdir -p "$INSTALL_DIR"
[ ! -d "$INSTALL_DIR/stopproof" ] || fail "destination is a directory: $INSTALL_DIR/stopproof"
# Keep staging on the destination filesystem so the final rename is atomic.
temp_dir="$(mktemp -d "$INSTALL_DIR/.stopproof-install.XXXXXX")"
trap 'rm -rf "$temp_dir"' 0
trap 'exit 1' 1 2 3 15

echo "Downloading $VERSION for $target ..."
download "$base_url/SHA256SUMS" "$temp_dir/SHA256SUMS" || fail "could not download SHA256SUMS; existing binary preserved"
download "$base_url/$asset" "$temp_dir/$asset" || fail "binary download failed; existing binary preserved"

expected="$(awk -v asset="$asset" '
  $2 == asset { count++; digest = $1 }
  END {
    if (count != 1 || length(digest) != 64 || digest ~ /[^0-9a-fA-F]/) exit 1;
    print tolower(digest)
  }
' "$temp_dir/SHA256SUMS")" || fail "missing or invalid checksum for $asset; existing binary preserved"

if [ "$hash_tool" = "sha256sum" ]; then
  actual="$(sha256sum "$temp_dir/$asset" | awk '{print $1}')"
else
  actual="$(shasum -a 256 "$temp_dir/$asset" | awk '{print $1}')"
fi
[ "$actual" = "$expected" ] || fail "SHA256 checksum mismatch; existing binary preserved"
chmod +x "$temp_dir/$asset"
"$temp_dir/$asset" --version || fail "downloaded binary cannot run; existing binary preserved"
mv -f "$temp_dir/$asset" "$INSTALL_DIR/stopproof"
echo "Installed: $INSTALL_DIR/stopproof (SHA256 verified)"

case ":$PATH:" in
  *":$INSTALL_DIR:"*) ;;
  *)
    echo "Add the install directory to PATH in your shell configuration:"
    echo "  export PATH=\"$INSTALL_DIR:\$PATH\""
    ;;
esac

echo "Next: cd into a trusted project and run: stopproof doctor"
echo "To register the Claude Code hook: stopproof init"

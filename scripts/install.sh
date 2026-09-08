#!/usr/bin/env bash
# Install spark-dump from GitHub Releases (CntierTeam/spark-cli).
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/CntierTeam/spark-cli/main/scripts/install.sh | bash
#   ./scripts/install.sh                 # latest
#   ./scripts/install.sh v1.1.0          # specific tag
#   INSTALL_DIR=~/bin ./scripts/install.sh
#
# Env:
#   REPO         default CntierTeam/spark-cli
#   INSTALL_DIR  default ~/.local/bin
#   VERSION      override tag (also positional arg)
#   ASSET        force asset name
set -euo pipefail

REPO="${REPO:-CntierTeam/spark-cli}"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"
VERSION="${1:-${VERSION:-latest}}"
BIN_NAME="spark-dump"

need() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "error: need '$1' on PATH" >&2
    exit 1
  }
}

need curl
need uname

os="$(uname -s | tr '[:upper:]' '[:lower:]')"
arch="$(uname -m)"

case "$os-$arch" in
  linux-x86_64|linux-amd64)
    # Prefer static musl build for portable installs
    ASSET="${ASSET:-spark-dump-linux-x86_64-musl}"
    FALLBACK_ASSET="spark-dump-linux-x86_64-gnu"
    OUT_NAME="$BIN_NAME"
    ;;
  linux-aarch64|linux-arm64)
    echo "error: no aarch64 release asset yet; build from source: cargo install --git https://github.com/${REPO}.git" >&2
    exit 1
    ;;
  mingw*|msys*|cygwin*|windows*)
    ASSET="${ASSET:-spark-dump-windows-x86_64.exe}"
    FALLBACK_ASSET=""
    OUT_NAME="${BIN_NAME}.exe"
    ;;
  darwin-*)
    echo "error: no macOS release asset yet; build from source: cargo install --git https://github.com/${REPO}.git" >&2
    exit 1
    ;;
  *)
    echo "error: unsupported platform: $os/$arch" >&2
    exit 1
    ;;
esac

api="https://api.github.com/repos/${REPO}/releases"
if [[ "$VERSION" == "latest" ]]; then
  release_url="${api}/latest"
else
  # accept v1.1.0 or 1.1.0
  tag="$VERSION"
  [[ "$tag" == v* ]] || tag="v${tag}"
  release_url="${api}/tags/${tag}"
fi

echo "Fetching release metadata: $release_url"
json="$(curl -fsSL "$release_url")" || {
  echo "error: failed to fetch release (repo=$REPO version=$VERSION)" >&2
  exit 1
}

tag_name="$(printf '%s' "$json" | sed -n 's/.*"tag_name":[[:space:]]*"\([^"]*\)".*/\1/p' | head -1)"
echo "Release: $tag_name"

pick_url() {
  local name="$1"
  printf '%s' "$json" | tr '\n' ' ' | sed 's/},{/}\n{/g' |
    grep -F "\"name\": \"$name\"" |
    sed -n 's/.*"browser_download_url":[[:space:]]*"\([^"]*\)".*/\1/p' |
    head -1
}

url="$(pick_url "$ASSET" || true)"
if [[ -z "${url:-}" && -n "${FALLBACK_ASSET:-}" ]]; then
  echo "asset $ASSET missing; trying $FALLBACK_ASSET"
  ASSET="$FALLBACK_ASSET"
  url="$(pick_url "$ASSET" || true)"
fi

if [[ -z "${url:-}" ]]; then
  echo "error: could not find asset '$ASSET' in release $tag_name" >&2
  echo "Available assets:" >&2
  printf '%s' "$json" | tr '"' '\n' | grep -E '^spark-dump-' >&2 || true
  exit 1
fi

tmpdir="$(mktemp -d)"
trap 'rm -rf "$tmpdir"' EXIT

echo "Downloading: $url"
curl -fL --progress-bar -o "$tmpdir/$ASSET" "$url"

# Optional checksum verify
sum_url="$(pick_url "${ASSET}.sha256" || true)"
if [[ -n "${sum_url:-}" ]] && command -v sha256sum >/dev/null 2>&1; then
  curl -fsSL -o "$tmpdir/${ASSET}.sha256" "$sum_url"
  (cd "$tmpdir" && sha256sum -c "${ASSET}.sha256")
fi

mkdir -p "$INSTALL_DIR"
install -m 0755 "$tmpdir/$ASSET" "$INSTALL_DIR/$OUT_NAME"

echo "Installed: $INSTALL_DIR/$OUT_NAME ($tag_name / $ASSET)"

case ":$PATH:" in
  *":$INSTALL_DIR:"*) ;;
  *)
    echo
    echo "Note: $INSTALL_DIR is not on PATH. Add e.g.:"
    echo "  export PATH=\"$INSTALL_DIR:\$PATH\""
    ;;
esac

"$INSTALL_DIR/$OUT_NAME" --version || true
echo "OK"

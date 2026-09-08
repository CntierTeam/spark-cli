#!/usr/bin/env bash
# Install the spark-dump Codex skill into $CODEX_HOME/skills (default ~/.codex/skills).
#
# Modes:
#   copy|link     use local skills/spark-dump from this checkout (dev)
#   release       download skill tarball/files from GitHub release/tag of CntierTeam/spark-cli
#
# Usage:
#   ./scripts/install-codex-skill.sh              # copy
#   ./scripts/install-codex-skill.sh link
#   ./scripts/install-codex-skill.sh release       # latest main raw files via git archive/API
#   VERSION=v1.1.0 ./scripts/install-codex-skill.sh release
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
SRC="$REPO_ROOT/skills/spark-dump"
CODEX_HOME="${CODEX_HOME:-$HOME/.codex}"
DEST_ROOT="${CODEX_HOME}/skills"
DEST="$DEST_ROOT/spark-dump"
MODE="${1:-copy}"
REPO="${REPO:-CntierTeam/spark-cli}"
VERSION="${VERSION:-latest}"

need() { command -v "$1" >/dev/null 2>&1 || { echo "error: need $1" >&2; exit 1; }; }

install_from_dir() {
  local from="$1"
  [[ -f "$from/SKILL.md" ]] || { echo "error: SKILL.md missing in $from" >&2; exit 1; }
  mkdir -p "$DEST_ROOT"
  if [[ -e "$DEST" || -L "$DEST" ]]; then
    echo "removing existing: $DEST"
    rm -rf "$DEST"
  fi
  mkdir -p "$DEST"
  if command -v rsync >/dev/null 2>&1; then
    rsync -a --delete "$from/" "$DEST/"
  else
    cp -R "$from/." "$DEST/"
  fi
  echo "installed (copy): $DEST"
}

case "$MODE" in
  copy)
    install_from_dir "$SRC"
    ;;
  link)
    [[ -f "$SRC/SKILL.md" ]] || { echo "error: skill not found at $SRC/SKILL.md" >&2; exit 1; }
    mkdir -p "$DEST_ROOT"
    if [[ -e "$DEST" || -L "$DEST" ]]; then
      echo "removing existing: $DEST"
      rm -rf "$DEST"
    fi
    ln -sfn "$SRC" "$DEST"
    echo "installed (symlink): $DEST -> $SRC"
    ;;
  release)
    need curl
    need tar
    tmp="$(mktemp -d)"
    trap 'rm -rf "$tmp"' EXIT
    if [[ "$VERSION" == "latest" ]]; then
      # Prefer default branch archive
      ref="main"
      url="https://codeload.github.com/${REPO}/tar.gz/refs/heads/${ref}"
      echo "Downloading skill sources from ${REPO}@${ref}"
      curl -fsSL "$url" -o "$tmp/src.tgz" || {
        ref="master"
        url="https://codeload.github.com/${REPO}/tar.gz/refs/heads/${ref}"
        curl -fsSL "$url" -o "$tmp/src.tgz"
      }
    else
      tag="$VERSION"
      [[ "$tag" == v* ]] || tag="v${tag}"
      url="https://codeload.github.com/${REPO}/tar.gz/refs/tags/${tag}"
      echo "Downloading skill sources from ${REPO}@${tag}"
      curl -fsSL "$url" -o "$tmp/src.tgz"
    fi
    mkdir -p "$tmp/extract"
    tar -xzf "$tmp/src.tgz" -C "$tmp/extract"
    root="$(find "$tmp/extract" -mindepth 1 -maxdepth 1 -type d | head -1)"
    skill_src="$root/skills/spark-dump"
    [[ -f "$skill_src/SKILL.md" ]] || { echo "error: skills/spark-dump not in archive" >&2; exit 1; }
    install_from_dir "$skill_src"
    ;;
  *)
    echo "usage: $0 [copy|link|release]" >&2
    exit 2
    ;;
esac

echo "Codex home: $CODEX_HOME"
echo "Skill will be available on the next Codex turn."
ls -la "$DEST"
test -f "$DEST/SKILL.md"
echo "OK: SKILL.md present"

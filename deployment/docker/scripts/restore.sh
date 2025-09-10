#!/usr/bin/env bash
set -euo pipefail

# Restore a backup archive into the graph data directory.
# Usage: restore.sh <archive-path> [target-dir]

if [[ $# -lt 1 ]]; then
  echo "Usage: $0 <archive-path> [target-dir]" >&2
  exit 2
fi

ARCHIVE="$1"
TARGET_DIR="${2:-/var/lib/codegraph}"

if [[ ! -f "$ARCHIVE" ]]; then
  echo "Archive not found: $ARCHIVE" >&2
  exit 3
fi

echo "[restore] Restoring $ARCHIVE -> $TARGET_DIR"
mkdir -p "$TARGET_DIR"
zstd -d --stdout "$ARCHIVE" | tar -C "$TARGET_DIR" -xf -
echo "[restore] Done."


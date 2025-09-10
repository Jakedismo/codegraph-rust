#!/usr/bin/env bash
set -euo pipefail

BIN_PATH=${1:-"target/release-size/codegraph-api"}

if [ ! -f "$BIN_PATH" ]; then
  echo "Binary not found: $BIN_PATH" >&2
  exit 1
fi

echo "Original size: $(du -h "$BIN_PATH" | cut -f1) ($BIN_PATH)"

case "$(uname -s)" in
  Darwin)
    # Use -x to strip non-global symbols (safer on Mach-O)
    strip -x "$BIN_PATH" || true
    ;;
  Linux)
    strip "$BIN_PATH" || true
    ;;
  *)
    echo "Unknown OS; skipping strip"
    ;;
esac

if command -v upx >/dev/null 2>&1; then
  echo "Compressing with UPX..."
  upx --best --lzma "$BIN_PATH" || true
else
  echo "UPX not found; skipping compression. Install 'upx' to enable." >&2
fi

echo "Final size: $(du -h "$BIN_PATH" | cut -f1)"


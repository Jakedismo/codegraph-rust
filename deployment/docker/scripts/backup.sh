#!/usr/bin/env sh
set -eu

# Runs as a long-lived sidecar to create periodic compressed backups
# of RocksDB graph data. Uses env:
#  - SRC_DIR (default /var/lib/codegraph)
#  - DEST_DIR (default /backups)
#  - INTERVAL_SECONDS (default 14400 = 4h)
#  - RETAIN (default 10 backups)

SRC_DIR="${SRC_DIR:-/var/lib/codegraph}"
DEST_DIR="${DEST_DIR:-/backups}"
INTERVAL_SECONDS="${INTERVAL_SECONDS:-14400}"
RETAIN="${RETAIN:-10}"

echo "[graph-backup] Source: ${SRC_DIR} -> Dest: ${DEST_DIR} (every ${INTERVAL_SECONDS}s, retain ${RETAIN})"

mkdir -p "${DEST_DIR}" || true

cleanup_old_backups() {
  ls -1t "${DEST_DIR}"/graph-backup-*.tar.zst 2>/dev/null | sed -e "1,${RETAIN}d" | xargs -r rm -f
}

while true; do
  TS="$(date -u +%Y%m%dT%H%M%SZ)"
  ARCHIVE="${DEST_DIR}/graph-backup-${TS}.tar.zst"
  echo "[graph-backup] Creating: ${ARCHIVE}"
  if [ -d "${SRC_DIR}" ]; then
    tar -C "${SRC_DIR}" -cf - . | zstd -T0 -3 -o "${ARCHIVE}" && echo "[graph-backup] OK: ${ARCHIVE}"
    cleanup_old_backups
  else
    echo "[graph-backup] WARN: SRC_DIR not found: ${SRC_DIR}" >&2
  fi
  sleep "${INTERVAL_SECONDS}"
done


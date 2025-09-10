#!/usr/bin/env bash
set -euo pipefail

# Cross-platform builder for CodeGraph API binary
# Builds release artifacts for Linux (gnu, musl), macOS (x86_64, aarch64), and Windows (x86_64)

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

targets=(
  x86_64-unknown-linux-gnu
  x86_64-unknown-linux-musl
  aarch64-unknown-linux-gnu
  x86_64-apple-darwin
  aarch64-apple-darwin
  x86_64-pc-windows-msvc
)

echo "==> Building codegraph-api for targets: ${targets[*]}"

for target in "${targets[@]}"; do
  echo "\n--- Building for $target ---"
  rustup target add "$target" >/dev/null 2>&1 || true

  case "$target" in
    x86_64-unknown-linux-gnu|x86_64-unknown-linux-musl|x86_64-apple-darwin|x86_64-pc-windows-msvc)
      export RUSTFLAGS="-C target-cpu=x86-64-v2"
      ;;
    *)
      unset RUSTFLAGS || true
      ;;
  esac

  cargo build --release --package codegraph-api --target "$target"

  # Verify binary executes with --version
  if [[ "$target" == *"windows"* ]]; then
    bin_path="target/$target/release/codegraph-api.exe"
  else
    bin_path="target/$target/release/codegraph-api"
  fi
  echo "Verifying $bin_path --version"
  "$bin_path" --version || {
    echo "Binary verification failed for $target" >&2
    exit 1
  }

  # Package
  base_name="codegraph-api-${target}"
  if [[ "$target" == *"windows"* ]]; then
    cp "$bin_path" "$base_name.exe"
    zip -q "$base_name.zip" "$base_name.exe"
    rm -f "$base_name.exe"
    echo "Created $base_name.zip"
  else
    cp "$bin_path" "$base_name"
    tar czf "$base_name.tar.gz" "$base_name"
    rm -f "$base_name"
    echo "Created $base_name.tar.gz"
  fi
done

echo "\nAll targets built and verified. Artifacts in repository root."


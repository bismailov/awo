#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

echo "Cleaning workspace target/ ..."
cargo clean

for crate_dir in "$ROOT_DIR"/crates/*/; do
  crate_name="$(basename "$crate_dir")"
  crate_target="$crate_dir/target"
  if [[ -d "$crate_target" ]]; then
    echo "Cleaning $crate_name/target ..."
    cargo clean --manifest-path "$crate_dir/Cargo.toml" --target-dir "$crate_target"
  fi
done

echo "Done. Current project size:"
du -sh "$ROOT_DIR"

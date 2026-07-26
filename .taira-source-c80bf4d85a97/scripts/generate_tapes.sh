#!/usr/bin/env bash
set -euo pipefail

# Generate .tape structural offset files for all .json files under a directory.
# Usage: scripts/generate_tapes.sh <dir>

DIR=${1:-}
if [[ -z "$DIR" ]]; then
  echo "Usage: $0 <dir-with-json>" >&2
  exit 1
fi

if [[ ! -d "$DIR" ]]; then
  echo "Not a directory: $DIR" >&2
  exit 1
fi

echo "Generating .tape for JSON files under $DIR"

run_dump() {
  local json="$1"
  local tape="${json%.json}.tape"
  echo "  $json -> $tape"
  cargo run -q --manifest-path crates/norito/Cargo.toml --example dump_tape --features json -- "$json" > "$tape"
}

shopt -s nullglob
for json in "$DIR"/*.json; do
  run_dump "$json"
done
echo "Done."


#!/usr/bin/env bash
set -euo pipefail

DIR="crates/norito/tests/data"

if ! cargo run -p norito --example gen_struct_tape --features json -- --help >/dev/null 2>&1; then
  echo "Building gen_struct_tape example..." >&2
fi

for jf in "$DIR"/*.json; do
  tf="${jf%.json}.tape"
  echo "Generating $tf from $jf" >&2
  cargo run -p norito --example gen_struct_tape --features json -- "$jf" | sed '1{/^#/d;}' > "$tf.tmp"
  mv "$tf.tmp" "$tf"
done

echo "Done."


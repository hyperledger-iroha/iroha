#!/usr/bin/env bash
set -euo pipefail

echo "Running Norito enum benches..."
# Enable Norito's internal enum fixtures.
FEATURES="bench-internal"
cargo bench -p norito --features "$FEATURES" --bench enum_packed_bench -- --quiet || true
cargo bench -p norito --features "$FEATURES" --bench enum_ncb -- --quiet || true
cargo bench -p norito --features "$FEATURES" --bench enum_indexed -- --quiet || true
cargo bench -p norito --features "$FEATURES" --bench ncb_sink_vs_vec -- --quiet || true
cargo bench -p norito --features "$FEATURES" --bench stream_maps -- --quiet || true
cargo bench -p norito --features "$FEATURES" --bench stream_seq -- --quiet || true

echo "Done. See target/criterion reports for details."

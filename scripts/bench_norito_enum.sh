#!/usr/bin/env bash
set -euo pipefail

echo "Running Norito enum benches..."
# Enable benches that require the `parity-scale` feature on the `norito` crate.
cargo bench -p norito --features parity-scale --bench enum_packed_bench -- --quiet || true
cargo bench -p norito --features parity-scale --bench enum_ncb -- --quiet || true
cargo bench -p norito --features parity-scale --bench enum_indexed -- --quiet || true
cargo bench -p norito --features parity-scale --bench ncb_sink_vs_vec -- --quiet || true
cargo bench -p norito --features parity-scale --bench stream_maps -- --quiet || true
cargo bench -p norito --features parity-scale --bench stream_seq -- --quiet || true

echo "Done. See target/criterion reports for details."

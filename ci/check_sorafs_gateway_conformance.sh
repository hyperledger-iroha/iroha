#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cd "${repo_root}"
export CARGO_TERM_COLOR="${CARGO_TERM_COLOR:-never}"
export CARGO_NET_OFFLINE="${CARGO_NET_OFFLINE:-true}"

echo "[sorafs-gateway] running conformance replay harness"
cargo test -p integration_tests --test sorafs_gateway_conformance -- --nocapture

echo "[sorafs-gateway] conformance harness passed"

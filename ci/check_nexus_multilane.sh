#!/usr/bin/env bash

set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd -- "${ROOT}"

export RUST_BACKTRACE="${RUST_BACKTRACE:-1}"

cargo test -p integration_tests multilane_router -- --nocapture

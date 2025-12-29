#!/bin/bash
set -euo pipefail

# Build iroha_data_model with cargo timings enabled to profile build steps
cargo build -p iroha_data_model --timings "$@"

echo "Timing report generated at target/cargo-timings/cargo-timing.html"

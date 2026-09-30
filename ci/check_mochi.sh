#!/usr/bin/env bash
set -euo pipefail

# Mochi delegates generation/lifecycle and deployment to these canonical owners.
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

cargo test --locked -p iroha_fs --lib
cargo test --locked -p iroha_deploy --lib managed
cargo test --locked -p mochi-core
cargo test --locked -p mochi-integration
cargo check --locked -p mochi-ui --features gui --bin mochi
cargo test --locked -p mochi-ui --features gui --bin mochi
cargo run --locked -p mochi-ui --features gui --bin mochi -- --help

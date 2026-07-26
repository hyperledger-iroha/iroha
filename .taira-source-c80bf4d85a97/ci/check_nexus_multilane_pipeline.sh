#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${REPO_ROOT}"

# Run the Nexus multilane regression covering lane router + Kura provisioning.
cargo test -p integration_tests nexus::multilane -- --nocapture

echo "[nexus] multilane pipeline regression completed"

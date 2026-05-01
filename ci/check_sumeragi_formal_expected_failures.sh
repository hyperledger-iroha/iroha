#!/bin/bash
set -euo pipefail

root_dir="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root_dir"

bash scripts/formal/sumeragi_apalache.sh frontier-bug-stale-owner
bash scripts/formal/sumeragi_apalache.sh frontier-bug-vote-queue

echo "[formal] Sumeragi expected-failure checks behaved as expected"

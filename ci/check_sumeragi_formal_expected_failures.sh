#!/bin/bash
set -euo pipefail

root_dir="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root_dir"

bash scripts/formal/sumeragi_apalache.sh frontier-bug-stale-owner
bash scripts/formal/sumeragi_apalache.sh frontier-bug-vote-queue
bash scripts/formal/sumeragi_apalache.sh frontier-bug-payload-recovery
bash scripts/formal/sumeragi_apalache.sh frontier-bug-retransmit-followthrough
bash scripts/formal/sumeragi_apalache.sh frontier-bug-future-promotion
bash scripts/formal/sumeragi_apalache.sh frontier-bug-future-reanchor-clear
bash scripts/formal/sumeragi_apalache.sh frontier-bug-future-evidence-drop
bash scripts/formal/sumeragi_apalache.sh frontier-bug-promotion-reset
bash scripts/formal/sumeragi_apalache.sh frontier-bug-future-stale-owner

echo "[formal] Sumeragi expected-failure checks behaved as expected"

#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

if (($#)); then
  echo "usage: $0" >&2
  exit 2
fi

python3 scripts/formal/check_sumeragi_v2_proof_ledger.py
bash scripts/formal/run_sumeragi_v2_tlaps.sh
python3 scripts/formal/check_sumeragi_v2_proof_ledger.py \
  --release \
  --evidence target/formal/sumeragi_v2/proof_evidence.json
bash scripts/formal/run_sumeragi_v2_service_rank_mutation.sh
bash scripts/formal/run_sumeragi_v2_tlc.sh "${SUMERAGI_V2_TLC_PROFILE:-ci}"
bash scripts/formal/check_sumeragi_v2_replay_trace.sh
bash scripts/verify_sumeragi_v2.sh

echo "Sumeragi v2 formal gate passed: source-bound TLAPS, adversarial scheduler mutations, bounded TLC, trace replay, and production Verus"

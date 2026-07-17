#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

if (($#)); then
  echo "usage: $0" >&2
  exit 2
fi

if [[ -n "${JAVA_BIN:-}" ]]; then
  JAVA_BIN="$(scripts/formal/resolve_java.sh "$JAVA_BIN")"
else
  JAVA_BIN="$(scripts/formal/resolve_java.sh)"
fi
export JAVA_BIN

python3 scripts/formal/check_sumeragi_v2_proof_ledger.py
bash scripts/formal/run_sumeragi_v2_tlaps.sh
python3 scripts/formal/check_sumeragi_v2_proof_ledger.py \
  --release \
  --evidence target/formal/sumeragi_v2/proof_evidence.json
bash scripts/formal/run_sumeragi_v2_service_rank_mutation.sh
bash scripts/formal/run_sumeragi_v2_productive_mutation.sh
bash scripts/formal/run_sumeragi_v2_candidate_restart_mutation.sh
bash scripts/formal/run_sumeragi_v2_progress_mutations.sh
bash scripts/formal/run_sumeragi_v2_tlc.sh ci
bash scripts/formal/check_sumeragi_v2_replay_trace.sh
bash scripts/verify_sumeragi_v2.sh
python3 scripts/formal/sumeragi_v2_verus_evidence.py validate \
  --root "$repo_root" \
  --evidence target/formal/sumeragi_v2/verus_evidence.json
python3 scripts/formal/check_sumeragi_v2_proof_ledger.py \
  --release \
  --evidence target/formal/sumeragi_v2/proof_evidence.json

echo "Sumeragi v2 formal gate passed: source-bound TLAPS, adversarial scheduler mutations, bounded TLC, trace replay, and production Verus"

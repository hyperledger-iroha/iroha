#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

release=false
if [[ "${1:-}" == "--release" ]]; then
  release=true
  shift
fi
if (($#)); then
  echo "usage: $0 [--release]" >&2
  exit 2
fi

checker=(python3 scripts/formal/check_sumeragi_v2_proof_ledger.py)
if [[ "$release" == true ]]; then
  checker+=(--release)
fi
"${checker[@]}"

bash scripts/formal/run_sumeragi_v2_tlaps.sh
bash scripts/formal/run_sumeragi_v2_tlc.sh "${SUMERAGI_V2_TLC_PROFILE:-ci}"
bash scripts/verify_sumeragi_v2.sh

echo "Sumeragi v2 formal gate passed: deductive TLAPS, bounded TLC, and production Verus"

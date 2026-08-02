#!/usr/bin/env bash
set -euo pipefail

readonly TLAPM_COMMIT="3ab43c7ff31db4ced850619d4746fa4c841a7681"
readonly REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly FORMAL_DIR="${REPO_ROOT}/formal/sumeragi_v2"
readonly CHECKER="${REPO_ROOT}/scripts/formal/check_sumeragi_v2_proof_ledger.py"
readonly EVIDENCE_DIR="${REPO_ROOT}/target/formal/sumeragi_v2"
readonly EVIDENCE_PATH="${EVIDENCE_DIR}/proof_evidence.json"
readonly RESOURCE_GUARD="${REPO_ROOT}/scripts/formal/run_sumeragi_v2_tlapm_guard.py"
readonly RESOURCE_JSONL="${EVIDENCE_DIR}/tlaps_resource.jsonl"
readonly RESOURCE_SUMMARY="${EVIDENCE_DIR}/tlaps_resource_summary.json"
readonly RESOURCE_AUTH_MAGIC="IROHA_RESOURCE_GUARD_AUTH_V1"

unset SUMERAGI_TLAPS_SUPERVISOR_PID
resource_auth_fd="${IROHA_RESOURCE_GUARD_AUTH_FD:-}"
resource_auth_token="${IROHA_RESOURCE_GUARD_AUTH_TOKEN:-}"
if [[ -z "$resource_auth_fd" && -z "$resource_auth_token" ]]; then
  exec python3 "$RESOURCE_GUARD" \
    --jsonl "$RESOURCE_JSONL" \
    --summary "$RESOURCE_SUMMARY" \
    -- /bin/bash "$0" "$@"
fi
if [[ -z "$resource_auth_fd" || -z "$resource_auth_token" \
  || ! "$resource_auth_fd" =~ ^([3-9]|[1-9][0-9]+)$ \
  || ! "$resource_auth_token" =~ ^[0-9a-f]{64}$ ]]; then
  echo "invalid resource-guard authorization capability" >&2
  exit 1
fi
if ! IFS= read -r -t 2 -u "$resource_auth_fd" resource_auth_record; then
  echo "resource-guard authorization capability is unavailable" >&2
  exit 1
fi
# Bash 3.2 lacks dynamic-FD syntax. The decimal-only validation above makes
# this expansion safe, and the body closes the one-shot capability immediately.
eval "exec ${resource_auth_fd}<&-"
unset IROHA_RESOURCE_GUARD_AUTH_FD IROHA_RESOURCE_GUARD_AUTH_TOKEN
if [[ "$resource_auth_record" != "${RESOURCE_AUTH_MAGIC}:${resource_auth_token}" ]]; then
  echo "resource-guard authorization capability is invalid" >&2
  exit 1
fi
unset resource_auth_fd resource_auth_token resource_auth_record

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) default_platform="x86_64-linux-gnu" ;;
  Darwin-arm64) default_platform="arm64-darwin" ;;
  *) default_platform="unsupported" ;;
esac

readonly DEFAULT_TLAPM="${REPO_ROOT}/target/tlapm/toolchains/${TLAPM_COMMIT}/${default_platform}/tlapm/bin/tlapm"
readonly TLAPM_BIN="${TLAPM_BIN:-$DEFAULT_TLAPM}"
readonly LOG_DIR="${EVIDENCE_DIR}/tlaps"
readonly TARGET_LOG_DIR="${LOG_DIR}/targets"
readonly CACHE_ROOT="${EVIDENCE_DIR}/tlaps-cache"
readonly TLAPM_THREADS=1
readonly TLAPM_COMPLETION_PATTERN='^\[INFO\]: All [1-9][0-9]* obligation(s)? proved\.$'
readonly TLAPM_PREFLIGHT_MARKER_PREFIX='SUMERAGI_TLAPS_FRONTEND_COMPLETE'
readonly TLAPM_TARGET_MARKER_PREFIX='SUMERAGI_TLAPS_TARGET_COMPLETE'

if [[ "${SUMERAGI_TLAPS_THREADS:-1}" != 1 ]]; then
  echo "SUMERAGI_TLAPS_THREADS must equal 1 for the bounded release proof wave" >&2
  exit 2
fi

[[ -x "$TLAPM_BIN" ]] || {
  echo "pinned TLAPM ${TLAPM_COMMIT} is required at ${TLAPM_BIN}" >&2
  echo "run scripts/formal/install_sumeragi_v2_tlapm.sh first" >&2
  exit 1
}
version="$($TLAPM_BIN --version 2>&1)"
if [[ "$version" != "${TLAPM_COMMIT:0:7}" ]]; then
  echo "expected TLAPM identity ${TLAPM_COMMIT:0:7}, found: ${version}" >&2
  exit 1
fi

python3 "$CHECKER"
source_manifest_sha256="$(python3 "$CHECKER" --print-source-manifest-sha256)"
readonly source_manifest_sha256
ledger_sha256="$(python3 "$CHECKER" --print-proof-ledger-sha256)"
readonly ledger_sha256
rm -rf -- "$LOG_DIR" "$CACHE_ROOT"
mkdir -p "$LOG_DIR" "$TARGET_LOG_DIR" "$CACHE_ROOT"
trap 'rm -rf -- "$CACHE_ROOT"' EXIT
rm -f -- "$EVIDENCE_PATH"
proof_modules=()
while IFS= read -r module; do
  proof_modules+=("$module")
done < <(python3 "$CHECKER" --print-proof-modules)
target_ids=()
target_kinds=()
target_ledger_modules=()
target_providers=()
target_theorems=()
target_start_lines=()
target_end_lines=()
target_sources=()
target_source_sha256s=()
target_span_sha256s=()
target_invocation_sha256s=()
target_expected_counts=()
while IFS=$'\t' read -r \
  target_id target_kind target_ledger_module target_provider target_theorem \
  target_start_line target_end_line target_source target_source_sha256 \
  target_span_sha256 target_invocation_sha256 target_expected_count; do
  target_ids+=("$target_id")
  target_kinds+=("$target_kind")
  target_ledger_modules+=("$target_ledger_module")
  target_providers+=("$target_provider")
  target_theorems+=("$target_theorem")
  target_start_lines+=("$target_start_line")
  target_end_lines+=("$target_end_line")
  target_sources+=("$target_source")
  target_source_sha256s+=("$target_source_sha256")
  target_span_sha256s+=("$target_span_sha256")
  target_invocation_sha256s+=("$target_invocation_sha256")
  target_expected_counts+=("$target_expected_count")
done < <(python3 "$CHECKER" --print-promotion-targets-tsv)
if [[ "${#target_ids[@]}" != 12 ]]; then
  echo "expected the exact ordered 9 + 3 promotion targets" >&2
  exit 1
fi

require_frozen_formal_inputs() {
  local phase="$1"
  local current_source_manifest_sha256
  local current_ledger_sha256
  current_source_manifest_sha256="$(
    python3 "$CHECKER" --print-source-manifest-sha256
  )"
  current_ledger_sha256="$(
    python3 "$CHECKER" --print-proof-ledger-sha256
  )"
  if [[ "$current_source_manifest_sha256" != "$source_manifest_sha256" ]]; then
    echo "TLA+ sources changed during ${phase}" >&2
    exit 1
  fi
  if [[ "$current_ledger_sha256" != "$ledger_sha256" ]]; then
    echo "proof_coverage.json changed during ${phase}" >&2
    exit 1
  fi
}

echo "[tlaps] deductive proof run with pinned ${TLAPM_COMMIT}"
for module in "${proof_modules[@]}"; do
  module_cache="${CACHE_ROOT}/${module}"
  preflight_log="${LOG_DIR}/${module}.preflight.log"
  mkdir -p "$module_cache"
  echo "[tlaps] frontend preflight ${module}"
  preflight_args=(
    --summary -N --strict --threads "$TLAPM_THREADS"
    --cache-dir "$module_cache"
  )
  (
    cd "$FORMAL_DIR"
    "$TLAPM_BIN" "${preflight_args[@]}" "${module}.tla"
  ) 2>&1 | tee "$preflight_log"
  if [[ ! -s "$preflight_log" ]]; then
    echo "TLAPM frontend preflight produced no output for ${module}" >&2
    exit 1
  fi
  require_frozen_formal_inputs "the TLAPM frontend preflight"
  printf '%s\n' \
    "${TLAPM_PREFLIGHT_MARKER_PREFIX} module=${module} commit=${TLAPM_COMMIT} source_manifest_sha256=${source_manifest_sha256} ledger_sha256=${ledger_sha256}" \
    | tee -a "$preflight_log"
  rm -rf -- "$module_cache"
done

echo "[tlaps] all frontend preflights passed; starting backend proof wave"
for module in "${proof_modules[@]}"; do
  module_cache="${CACHE_ROOT}/${module}"
  mkdir -p "$module_cache"

  echo "[tlaps] checking ${module}"
  args=(
    --strict --nofp --threads "$TLAPM_THREADS"
    --cache-dir "$module_cache"
  )
  (
    cd "$FORMAL_DIR"
    "$TLAPM_BIN" "${args[@]}" "${module}.tla"
  ) 2>&1 | tee "${LOG_DIR}/${module}.log"
  completion_count="$(grep -Ec "$TLAPM_COMPLETION_PATTERN" "${LOG_DIR}/${module}.log" || true)"
  if [[ "$completion_count" != 1 ]] \
    || ! tail -n 1 "${LOG_DIR}/${module}.log" | grep -Eq "$TLAPM_COMPLETION_PATTERN"; then
    echo "TLAPM did not report exact strict completion for ${module}" >&2
    exit 1
  fi
  require_frozen_formal_inputs "the TLAPM module proof run"
  printf '%s\n' \
    "SUMERAGI_TLAPS_BACKEND_COMPLETE module=${module} commit=${TLAPM_COMMIT} source_manifest_sha256=${source_manifest_sha256} ledger_sha256=${ledger_sha256}" \
    | tee -a "${LOG_DIR}/${module}.log"
  rm -rf -- "$module_cache"
done

echo "[tlaps] module wave passed; starting exact 9 + 3 theorem-range wave"
for ((target_index = 0; target_index < ${#target_ids[@]}; target_index++)); do
  target_id="${target_ids[$target_index]}"
  target_provider="${target_providers[$target_index]}"
  target_theorem="${target_theorems[$target_index]}"
  target_start_line="${target_start_lines[$target_index]}"
  target_end_line="${target_end_lines[$target_index]}"
  target_source_sha256="${target_source_sha256s[$target_index]}"
  target_span_sha256="${target_span_sha256s[$target_index]}"
  target_invocation_sha256="${target_invocation_sha256s[$target_index]}"
  target_expected_count="${target_expected_counts[$target_index]}"
  target_cache="${CACHE_ROOT}/targets/${target_id}"
  target_log="${TARGET_LOG_DIR}/${target_id}.log"
  canonical_target_cache="../../target/formal/sumeragi_v2/tlaps-cache/targets/${target_id}"
  mkdir -p "$target_cache"

  echo "[tlaps] exact target ${target_id} (${target_provider}!${target_theorem})"
  target_args=(
    --toolbox "$target_start_line" "$target_end_line"
    --strict --nofp --threads "$TLAPM_THREADS"
    --cache-dir "$canonical_target_cache"
    "${target_provider}.tla"
  )
  (
    cd "$FORMAL_DIR"
    "$TLAPM_BIN" "${target_args[@]}"
  ) 2>&1 | tee "$target_log"
  completion_count="$(grep -Ec "$TLAPM_COMPLETION_PATTERN" "$target_log" || true)"
  if [[ "$completion_count" != 1 ]] \
    || ! tail -n 1 "$target_log" | grep -Eq "$TLAPM_COMPLETION_PATTERN"; then
    echo "TLAPM did not report exact strict target completion for ${target_id}" >&2
    exit 1
  fi
  proved_count="$(
    tail -n 1 "$target_log" \
      | sed -E 's/^\[INFO\]: All ([1-9][0-9]*) obligation(s)? proved\.$/\1/'
  )"
  if [[ "$target_expected_count" != "-" \
    && "$proved_count" != "$target_expected_count" ]]; then
    echo "TLAPM target ${target_id} proved ${proved_count}; expected ${target_expected_count}" >&2
    exit 1
  fi
  require_frozen_formal_inputs "the TLAPM target proof run"
  printf '%s\n' \
    "${TLAPM_TARGET_MARKER_PREFIX} obligation_id=${target_id} provider_module=${target_provider} theorem=${target_theorem} start_line=${target_start_line} end_line=${target_end_line} obligations_proved=${proved_count} commit=${TLAPM_COMMIT} source_manifest_sha256=${source_manifest_sha256} ledger_sha256=${ledger_sha256} source_sha256=${target_source_sha256} proof_span_sha256=${target_span_sha256} invocation_sha256=${target_invocation_sha256}" \
    | tee -a "$target_log"
  rm -rf -- "$target_cache"
done

require_frozen_formal_inputs "the complete TLAPM proof run"
python3 "$CHECKER" \
  --write-evidence "$EVIDENCE_PATH" \
  --tlapm-version "$version" \
  --tlaps-log-dir "$LOG_DIR"
echo "[tlaps] observed target counts (review only; no contract was edited):"
python3 "$CHECKER" \
  --print-promotion-target-counts \
  --evidence "$EVIDENCE_PATH"
echo "[tlaps] all configured deductive modules discharged; evidence=${EVIDENCE_PATH}"

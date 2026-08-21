#!/usr/bin/env bash
# Run the bounded durable Validate lifecycle repaired case and five mutants.
# Requires the authenticated TLA2Tools v1.7.4 jar in TLA2TOOLS_JAR; JAVA_BIN is optional.
set -euo pipefail

readonly TLA2TOOLS_VERSION="1.7.4"
readonly TLA2TOOLS_SHA256="936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88"
readonly REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
source "${REPO_ROOT}/scripts/formal/sumeragi_v2_tlc_result_contract.sh"
readonly FORMAL_DIR="${REPO_ROOT}/formal/sumeragi_v2"

usage() {
  echo "usage: $0" >&2
}

if (($#)); then
  case "$1" in
    -h|--help)
      usage
      exit 0
      ;;
    *)
      usage
      exit 2
      ;;
  esac
fi

readonly TLA2TOOLS_JAR="${TLA2TOOLS_JAR:?TLA2TOOLS_JAR must name the authenticated external tool}"
if [[ -n "${JAVA_BIN:-}" ]]; then
  resolved_java_bin="$("${REPO_ROOT}/scripts/formal/resolve_java.sh" "$JAVA_BIN")"
else
  resolved_java_bin="$("${REPO_ROOT}/scripts/formal/resolve_java.sh")"
fi
readonly JAVA_BIN="$resolved_java_bin"

hash_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

[[ -f "$TLA2TOOLS_JAR" ]] || {
  echo "pinned TLA2Tools v${TLA2TOOLS_VERSION} is required at ${TLA2TOOLS_JAR}" >&2
  exit 1
}
actual_sha256="$(hash_file "$TLA2TOOLS_JAR")"
[[ "$actual_sha256" == "$TLA2TOOLS_SHA256" ]] || {
  echo "TLA2Tools checksum mismatch" >&2
  echo "expected: ${TLA2TOOLS_SHA256}" >&2
  echo "actual:   ${actual_sha256}" >&2
  exit 1
}
"$JAVA_BIN" -version >/dev/null 2>&1 || {
  echo "a working Java runtime is required" >&2
  exit 1
}

run_dir="$(mktemp -d "${TMPDIR:-/tmp}/sumeragi-v2-durable-validate.XXXXXX")"
trap 'rm -rf -- "$run_dir"' EXIT

common=(
  "$JAVA_BIN" -XX:+UseParallelGC -cp "$TLA2TOOLS_JAR" tlc2.TLC
  -cleanup -workers 1 -fp 101 -seed 8336471840206429 -coverage 1
)

run_case() {
  local label="$1"
  local config="$2"
  local expected_status="$3"
  shift 3
  local log="${run_dir}/${label}.log"
  local actual_status
  local expected_diagnostic
  local primary_diagnostic_count
  set +e
  (
    cd "$FORMAL_DIR"
    "${common[@]}" -metadir "${run_dir}/${label}" \
      -config "$config" SumeragiV2DurableValidateLifecycleMutation.tla
  ) >"$log" 2>&1
  actual_status=$?
  set -e
  if [[ "$actual_status" -ne "$expected_status" ]]; then
    echo "${label} returned TLC status ${actual_status}, expected ${expected_status}" >&2
    cat "$log" >&2
    exit 1
  fi
  if [[ "$expected_status" -eq 0 ]]; then
    [[ "$#" == 2 ]] || {
      echo "${label} fixed case must declare version and success markers" >&2
      exit 1
    }
    sumeragi_v2_tlc_assert_fixed_success "$label" "$log" "$actual_status"
    for marker in "$@"; do
      grep -Fq "$marker" "$log" || {
        echo "${label} missed expected marker: ${marker}" >&2
        cat "$log" >&2
        exit 1
      }
    done
  elif [[ "$expected_status" -eq 12 ]]; then
    [[ "$#" == 3 ]] || {
      echo "${label} mutation must declare version, diagnostic, and coverage markers" >&2
      exit 1
    }
    grep -Fq "$1" "$log" || {
      echo "${label} missed expected version marker: $1" >&2
      exit 1
    }
    expected_diagnostic="Error: $2"
    sumeragi_v2_tlc_assert_exact_line "$label" "$log" "$expected_diagnostic"
    sumeragi_v2_tlc_assert_nonzero_state_space "$label" "$log"
    sumeragi_v2_tlc_assert_terminal "$label" "$log"
    primary_diagnostic_count="$(${GREP_BIN:-grep} -Ec \
      "$SUMERAGI_V2_TLC_PRIMARY_DIAGNOSTIC_PATTERN" "$log" || true)"
    [[ "$primary_diagnostic_count" -eq 1 ]] || {
      echo "${label} emitted ${primary_diagnostic_count} primary diagnostics" >&2
      cat "$log" >&2
      exit 1
    }
    grep -Fq "$3" "$log" || {
      echo "${label} missed expected coverage marker: $3" >&2
      cat "$log" >&2
      exit 1
    }
  else
    echo "${label} declared unsupported TLC status ${expected_status}" >&2
    exit 1
  fi
  echo "[tlc] ${label}: expected status ${expected_status}"
}

run_case durable-validate-lifecycle-fixed \
  durable_validate_lifecycle_fixed.cfg 0 \
  "TLC2 Version 2.19" \
  "Model checking completed. No error has been found."

run_case unguarded-validate-completion-mutant \
  durable_validate_lifecycle_unguarded_completion_bug.cfg 12 \
  "TLC2 Version 2.19" \
  "Invariant GuardedCompletionMatchesClaimedRow is violated." \
  "<WorkerReturnsGuardedCompletion"

run_case sidecar-new-ordinal-mutant \
  durable_validate_lifecycle_sidecar_new_ordinal_bug.cfg 12 \
  "TLC2 Version 2.19" \
  "Invariant ExactSidecarWakeReusesWaitingRow is violated." \
  "<DeliverExactSidecar"

run_case unreserved-rejection-output-mutant \
  durable_validate_lifecycle_unreserved_rejection_bug.cfg 12 \
  "TLC2 Version 2.19" \
  "Invariant RejectedResultClaimHasReservedOutput is violated." \
  "<ClaimRejectedResult"

run_case restart-drops-replay-authority-mutant \
  durable_validate_lifecycle_restart_replay_drop_bug.cfg 12 \
  "TLC2 Version 2.19" \
  "Invariant RestartReopensMandatoryReplayAuthority is violated." \
  "<CrashAndReopenWaiting"

run_case ambiguous-post-fsync-continues-mutant \
  durable_validate_lifecycle_post_fsync_continue_bug.cfg 12 \
  "TLC2 Version 2.19" \
  "Invariant AmbiguousPostFsyncRequiresFailStop is violated." \
  "<ObserveAmbiguousPostFsync"

echo "[tlc] durable Validate owns Ready-to-Waiting dispatch and exact guarded completion"
echo "[tlc] exact sidecar wake reuses one row and ordinal; rejected output is pre-reserved"
echo "[tlc] all replay origins reopen with mandatory authority; ambiguous post-fsync failure stops"

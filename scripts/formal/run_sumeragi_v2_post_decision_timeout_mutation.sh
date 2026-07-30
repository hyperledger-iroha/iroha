#!/usr/bin/env bash
set -euo pipefail

readonly TLA2TOOLS_VERSION="1.7.4"
readonly TLA2TOOLS_SHA256="936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88"
readonly REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly FORMAL_DIR="${REPO_ROOT}/formal/sumeragi_v2"
readonly TLA2TOOLS_JAR="${TLA2TOOLS_JAR:-${REPO_ROOT}/target/tla2tools/${TLA2TOOLS_VERSION}/tla2tools.jar}"
if [[ -n "${JAVA_BIN:-}" ]]; then
  resolved_java_bin="$("${REPO_ROOT}/scripts/formal/resolve_java.sh" "$JAVA_BIN")"
else
  resolved_java_bin="$("${REPO_ROOT}/scripts/formal/resolve_java.sh")"
fi
readonly JAVA_BIN="$resolved_java_bin"

if (($#)); then
  echo "usage: $0" >&2
  exit 2
fi

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

run_dir="$(mktemp -d "${TMPDIR:-/tmp}/sumeragi-v2-post-decision-timeout.XXXXXX")"
trap 'rm -rf -- "$run_dir"' EXIT

common=(
  "$JAVA_BIN" -XX:+UseParallelGC -cp "$TLA2TOOLS_JAR" tlc2.TLC
  -cleanup -workers 1 -fp 96 -seed 139154308881391968 -coverage 1
)

run_case() {
  local label="$1"
  local config="$2"
  local expected_status="$3"
  shift 3
  local log="${run_dir}/${label}.log"
  local actual_status
  set +e
  (
    cd "$FORMAL_DIR"
    "${common[@]}" -metadir "${run_dir}/${label}" \
      -config "$config" SumeragiV2PostDecisionTimeoutMutation.tla
  ) >"$log" 2>&1
  actual_status=$?
  set -e
  if [[ "$actual_status" -ne "$expected_status" ]]; then
    echo "${label} returned TLC status ${actual_status}, expected ${expected_status}" >&2
    cat "$log" >&2
    exit 1
  fi
  for marker in "$@"; do
    if ! grep -Fq "$marker" "$log"; then
      echo "${label} missed expected marker: ${marker}" >&2
      cat "$log" >&2
      exit 1
    fi
  done
  echo "[tlc] ${label}: expected status ${expected_status}"
}

run_case fixed-post-decision-boundary post_decision_timeout_fixed.cfg 0 \
  "TLC2 Version 2.19" \
  "Model checking completed. No error has been found." \
  "<AttemptResumeTimeout" \
  "<AttemptBeginTimeout" \
  "<AttemptCompleteTimeoutSignature" \
  "<AttemptBeginInstallTC" \
  "<DeliverTimeout" \
  "<DeliverTC"

run_case missing-resume-timeout-guard \
  post_decision_resume_timeout_guard_bug.cfg 12 \
  "Invariant NoTimeoutSignatureAfterDecision is violated." \
  "State 3: <AttemptResumeTimeout"

run_case missing-begin-timeout-guard \
  post_decision_begin_timeout_guard_bug.cfg 12 \
  "Invariant NoTimeoutIntentAfterDecision is violated." \
  "State 4: <AttemptBeginTimeout"

run_case missing-complete-timeout-guard \
  post_decision_complete_timeout_guard_bug.cfg 12 \
  "Invariant LocalTimeoutCompletionIsAtomicNoOp is violated." \
  "State 5: <AttemptCompleteTimeoutSignature"

run_case local-timeout-successor-after-decision \
  post_decision_local_timeout_successor_bug.cfg 12 \
  "Invariant LocalTimeoutCompletionHasNoCausalSuccessor is violated." \
  "State 5: <AttemptCompleteTimeoutSignature"

run_case missing-begin-install-tc-guard \
  post_decision_begin_install_tc_guard_bug.cfg 12 \
  "Invariant NoTCInstallAfterDecision is violated." \
  "State 7: <AttemptBeginInstallTC"

run_case timeout-admitted-after-decision post_decision_timeout_receive_bug.cfg 12 \
  "Invariant TimeoutDeliveryConsumesWithoutAtomicAdmission is violated." \
  "State 9: <DeliverTimeout"

run_case tc-admitted-after-decision post_decision_tc_receive_bug.cfg 12 \
  "Invariant TCDeliveryConsumesWithoutAdmission is violated." \
  "State 12: <DeliverTC"

run_case timeout-successor-after-decision \
  post_decision_timeout_successor_bug.cfg 12 \
  "Invariant TimeoutDeliveryHasNoCausalSuccessor is violated." \
  "State 9: <DeliverTimeout"

run_case tc-successor-after-decision post_decision_tc_successor_bug.cfg 12 \
  "Invariant TCDeliveryHasNoCausalSuccessor is violated." \
  "State 12: <DeliverTC"

echo "[tlc] post-Decision replay, BeginTimeout, atomic local completion, and BeginInstallTC guards reject all new work"
echo "[tlc] local completion and authenticated TimeoutVote cannot admit a receipt, form a TC, open InstallTC, or publish PersistInstallTC"
echo "[tlc] authenticated TC envelopes are consumed without receive-pool admission or BeginInstallTC successors"

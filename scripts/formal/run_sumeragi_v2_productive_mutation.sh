#!/usr/bin/env bash
set -euo pipefail

readonly TLA2TOOLS_VERSION="1.7.4"
readonly TLA2TOOLS_SHA256="936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88"
readonly REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly FORMAL_DIR="${REPO_ROOT}/docs/formal/sumeragi_v2"
readonly TLA2TOOLS_JAR="${TLA2TOOLS_JAR:-${REPO_ROOT}/target/tla2tools/${TLA2TOOLS_VERSION}/tla2tools.jar}"
source "${REPO_ROOT}/scripts/formal/sumeragi_v2_tlc_result_contract.sh"
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

run_dir="$(mktemp -d "${TMPDIR:-/tmp}/sumeragi-v2-productive-mutation.XXXXXX")"
trap 'rm -rf -- "$run_dir"' EXIT

common=(
  "$JAVA_BIN" -XX:+UseParallelGC -cp "$TLA2TOOLS_JAR" tlc2.TLC
  -cleanup -workers 1 -fp 96 -seed 139154308881391968
)

assert_counterexample() {
  local label="$1"
  local log="$2"
  local expected_diagnostic="$3"
  local require_state_summary="$4"
  local primary_diagnostic_count
  sumeragi_v2_tlc_assert_exact_line \
    "$label" "$log" "$expected_diagnostic"
  primary_diagnostic_count="$(
    grep -Ec \
      "$SUMERAGI_V2_TLC_PRIMARY_DIAGNOSTIC_PATTERN" \
      "$log" || true
  )"
  [[ "$primary_diagnostic_count" -eq 1 ]] || {
    echo "${label} emitted ${primary_diagnostic_count} primary failure diagnostics" >&2
    cat "$log" >&2
    exit 1
  }
  if [[ "$require_state_summary" -eq 1 ]]; then
    sumeragi_v2_tlc_assert_nonzero_state_space "$label" "$log"
  fi
  sumeragi_v2_tlc_assert_terminal "$label" "$log"
}

set +e
(
  cd "$FORMAL_DIR"
  "${common[@]}" -metadir "$run_dir/normal-old" \
    -config normal_protected_old.cfg \
    SumeragiV2NormalProtectedMutation.tla
) >"$run_dir/normal-old.log" 2>&1
normal_old_status=$?
set -e

[[ $normal_old_status -eq 13 ]] || {
  echo "unprotected Normal proposal/Prepare mutation did not fail with TLC status 13" >&2
  cat "$run_dir/normal-old.log" >&2
  exit 1
}
assert_counterexample \
  "normal-old" "$run_dir/normal-old.log" \
  "Error: Temporal properties were violated." 1
for marker in \
  "TLC2 Version 2.19" \
  "Temporal properties were violated." \
  "State 2: Stuttering"; do
  grep -Fq "$marker" "$run_dir/normal-old.log" || {
    echo "unprotected Normal proposal/Prepare mutation missed expected marker: $marker" >&2
    cat "$run_dir/normal-old.log" >&2
    exit 1
  }
done

set +e
(
  cd "$FORMAL_DIR"
  "${common[@]}" -metadir "$run_dir/normal-dynamic-class" \
    -config normal_protected_dynamic_class_bug.cfg \
    SumeragiV2NormalProtectedMutation.tla
) >"$run_dir/normal-dynamic-class.log" 2>&1
normal_dynamic_class_status=$?
set -e

[[ $normal_dynamic_class_status -eq 12 ]] || {
  echo "dynamic delivery-class mutation did not fail with TLC status 12" >&2
  cat "$run_dir/normal-dynamic-class.log" >&2
  exit 1
}
assert_counterexample \
  "normal-dynamic-class" "$run_dir/normal-dynamic-class.log" \
  "Error: Invariant StoredNormalRemainsProtected is violated." 1
for marker in \
  "Invariant StoredNormalRemainsProtected is violated." \
  "State 2: <AdvanceTC" \
  "historical = TRUE"; do
  grep -Fq "$marker" "$run_dir/normal-dynamic-class.log" || {
    echo "dynamic delivery-class mutation missed expected marker: $marker" >&2
    cat "$run_dir/normal-dynamic-class.log" >&2
    exit 1
  }
done

(
  cd "$FORMAL_DIR"
  "${common[@]}" -metadir "$run_dir/normal-fixed" \
    -config normal_protected_fixed.cfg \
    SumeragiV2NormalProtectedMutation.tla
  ) >"$run_dir/normal-fixed.log" 2>&1
sumeragi_v2_tlc_assert_fixed_success \
  "normal-fixed" "$run_dir/normal-fixed.log" 0
for marker in \
  "Model checking completed. No error has been found." \
  "10 distinct states" \
  "depth of the complete state graph search is 3"; do
  grep -Fq "$marker" "$run_dir/normal-fixed.log" || {
    cat "$run_dir/normal-fixed.log" >&2
    exit 1
  }
done

(
  cd "$FORMAL_DIR"
  "${common[@]}" -metadir "$run_dir/scheduler-only" \
    -config productive_deadlock_scheduler_bug.cfg \
    SumeragiV2ProductiveDeadlockMutation.tla
  ) >"$run_dir/scheduler-only.log" 2>&1
sumeragi_v2_tlc_assert_fixed_success \
  "scheduler-only" "$run_dir/scheduler-only.log" 0
for marker in \
  "Model checking completed. No error has been found." \
  "2 distinct states"; do
  grep -Fq "$marker" "$run_dir/scheduler-only.log" || {
    cat "$run_dir/scheduler-only.log" >&2
    exit 1
  }
done

set +e
(
  cd "$FORMAL_DIR"
  "${common[@]}" -metadir "$run_dir/productive-bare" \
    -config productive_deadlock_bare_rejected.cfg \
    SumeragiV2ProductiveDeadlockMutation.tla
) >"$run_dir/productive-bare.log" 2>&1
productive_bare_status=$?
set -e

[[ $productive_bare_status -eq 12 ]] || {
  echo "bare scheduler mutation did not violate productive deadlock with TLC status 12" >&2
  cat "$run_dir/productive-bare.log" >&2
  exit 1
}
assert_counterexample \
  "productive-bare" "$run_dir/productive-bare.log" \
  "Error: Invariant ProductiveDeadlockClaim is violated by the initial state:" \
  0
for marker in \
  "TLC2 Version 2.19" \
  "Invariant ProductiveDeadlockClaim is violated by the initial state" \
  "deadlineDebt = 0" \
  "serviceRank = 0"; do
  grep -Fq "$marker" "$run_dir/productive-bare.log" || {
    echo "bare scheduler mutation missed expected productive rejection marker: $marker" >&2
    cat "$run_dir/productive-bare.log" >&2
    exit 1
  }
done

(
  cd "$FORMAL_DIR"
  "${common[@]}" -metadir "$run_dir/productive-fixed" \
    -config productive_deadlock_fixed.cfg \
    SumeragiV2ProductiveDeadlockMutation.tla
  ) >"$run_dir/productive-fixed.log" 2>&1
sumeragi_v2_tlc_assert_fixed_success \
  "productive-fixed" "$run_dir/productive-fixed.log" 0
for marker in \
  "Model checking completed. No error has been found." \
  "5 distinct states" \
  "depth of the complete state graph search is 5"; do
  grep -Fq "$marker" "$run_dir/productive-fixed.log" || {
    cat "$run_dir/productive-fixed.log" >&2
    exit 1
  }
done

echo "[tlc] omitting Normal proposal/Prepare protection has the required starvation counterexample"
echo "[tlc] recomputing a stored Normal CommitVote class after TC loses protection"
echo "[tlc] exact Normal source protection closes that counterexample"
echo "[tlc] scheduler-only enablement accepts the bare-tick mutation"
echo "[tlc] productive enablement rejects bare ticks and accepts concrete debt/evidence/rank repair"

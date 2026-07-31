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

run_dir="$(mktemp -d "${TMPDIR:-/tmp}/sumeragi-v2-rank-mutation.XXXXXX")"
trap 'rm -rf -- "$run_dir"' EXIT

common=(
  "$JAVA_BIN" -XX:+UseParallelGC -cp "$TLA2TOOLS_JAR" tlc2.TLC
  -cleanup -workers 1 -fp 96 -seed 139154308881391968
)

assert_explored_counterexample() {
  local label="$1"
  local log="$2"
  local expected_diagnostic="$3"
  local primary_diagnostic_count
  sumeragi_v2_tlc_assert_nonzero_state_space "$label" "$log"
  sumeragi_v2_tlc_assert_exact_line \
    "$label" "$log" "$expected_diagnostic"
  if [[ "$expected_diagnostic" == "Error: Invariant "* ]]; then
    sumeragi_v2_tlc_assert_exact_line \
      "$label" "$log" "Error: The behavior up to this point is:"
  fi
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
  sumeragi_v2_tlc_assert_terminal "$label" "$log"
}

set +e
(
  cd "$FORMAL_DIR"
  "${common[@]}" -metadir "$run_dir/old" \
    -config service_rank_replacement_bug.cfg \
    SumeragiV2ServiceRankMutation.tla
) >"$run_dir/old.log" 2>&1
old_status=$?
set -e

[[ $old_status -eq 13 ]] || {
  echo "old value-rank mutation did not fail with TLC status 13" >&2
  cat "$run_dir/old.log" >&2
  exit 1
}
assert_explored_counterexample \
  "old-service-rank" "$run_dir/old.log" \
  "Error: Temporal properties were violated."
for marker in \
  "TLC2 Version 2.19" \
  "Temporal properties were violated." \
  "2 distinct states" \
  "Back to state 2"; do
  grep -Fq "$marker" "$run_dir/old.log" || {
    echo "old value-rank mutation missed expected marker: $marker" >&2
    cat "$run_dir/old.log" >&2
    exit 1
  }
done

(
  cd "$FORMAL_DIR"
  "${common[@]}" -metadir "$run_dir/coalesced" \
    -config service_rank_coalesced.cfg \
    SumeragiV2ServiceRankMutation.tla
) >"$run_dir/coalesced.log" 2>&1
sumeragi_v2_tlc_assert_fixed_success \
  "coalesced" "$run_dir/coalesced.log" 0
grep -Fq "Model checking completed. No error has been found." \
  "$run_dir/coalesced.log" || {
  cat "$run_dir/coalesced.log" >&2
  exit 1
}

echo "[tlc] old equal-value replacement has the required two-state lasso"
echo "[tlc] exact queued-envelope coalescing closes that lasso"

set +e
(
  cd "$FORMAL_DIR"
  "${common[@]}" -metadir "$run_dir/global-blocker-same-rank-swap" \
    -config adequate_leader_global_blocker_same_rank_swap_bug.cfg \
    SumeragiV2AdequateLeaderGlobalBlockerCellMutation.tla
) >"$run_dir/global-blocker-same-rank-swap.log" 2>&1
global_blocker_same_rank_swap_status=$?
set -e

[[ $global_blocker_same_rank_swap_status -eq 13 ]] || {
  echo "same-rank global-blocker cell swap did not fail with TLC status 13" >&2
  cat "$run_dir/global-blocker-same-rank-swap.log" >&2
  exit 1
}
assert_explored_counterexample \
  "global-blocker-same-rank-swap" \
  "$run_dir/global-blocker-same-rank-swap.log" \
  "Error: Temporal properties were violated."
for marker in \
  "TLC2 Version 2.19" \
  "Temporal properties were violated." \
  'selectedCell = "Replacement"' \
  "Back to state 2"; do
  grep -Fq "$marker" "$run_dir/global-blocker-same-rank-swap.log" || {
    echo "same-rank global-blocker cell swap missed expected marker: $marker" >&2
    cat "$run_dir/global-blocker-same-rank-swap.log" >&2
    exit 1
  }
done

(
  cd "$FORMAL_DIR"
  "${common[@]}" -metadir "$run_dir/global-blocker-exact-cell" \
    -config adequate_leader_global_blocker_exact_cell.cfg \
    SumeragiV2AdequateLeaderGlobalBlockerCellMutation.tla
) >"$run_dir/global-blocker-exact-cell.log" 2>&1
sumeragi_v2_tlc_assert_fixed_success \
  "global-blocker-exact-cell" "$run_dir/global-blocker-exact-cell.log" 0
grep -Fq "Model checking completed. No error has been found." \
  "$run_dir/global-blocker-exact-cell.log" || {
  cat "$run_dir/global-blocker-exact-cell.log" >&2
  exit 1
}

echo "[tlc] rank-only selection can service an unrelated equal-rank blocker forever"
echo "[tlc] exact frozen-cell selection releases the original blocker before replenishment"

set +e
(
  cd "$FORMAL_DIR"
  "${common[@]}" -metadir "$run_dir/deferred-old" \
    -config service_rank_deferred_replacement_bug.cfg \
    SumeragiV2ServiceRankMutation.tla
) >"$run_dir/deferred-old.log" 2>&1
deferred_old_status=$?
set -e

[[ $deferred_old_status -eq 13 ]] || {
  echo "old deferred-owner replacement mutation did not fail with TLC status 13" >&2
  cat "$run_dir/deferred-old.log" >&2
  exit 1
}
assert_explored_counterexample \
  "deferred-old" "$run_dir/deferred-old.log" \
  "Error: Temporal properties were violated."
for marker in \
  "TLC2 Version 2.19" \
  "Temporal properties were violated." \
  "Back to state 3"; do
  grep -Fq "$marker" "$run_dir/deferred-old.log" || {
    echo "old deferred-owner replacement mutation missed expected marker: $marker" >&2
    cat "$run_dir/deferred-old.log" >&2
    exit 1
  }
done

(
  cd "$FORMAL_DIR"
  "${common[@]}" -metadir "$run_dir/deferred-coalesced" \
    -config service_rank_deferred_coalesced.cfg \
    SumeragiV2ServiceRankMutation.tla
) >"$run_dir/deferred-coalesced.log" 2>&1
sumeragi_v2_tlc_assert_fixed_success \
  "deferred-coalesced" "$run_dir/deferred-coalesced.log" 0
grep -Fq "Model checking completed. No error has been found." \
  "$run_dir/deferred-coalesced.log" || {
  cat "$run_dir/deferred-coalesced.log" >&2
  exit 1
}

echo "[tlc] queue-only coalescing exposes the deferred replacement lasso"
echo "[tlc] scheduler-wide coalescing closes the deferred replacement lasso"

set +e
(
  cd "$FORMAL_DIR"
  "${common[@]}" -metadir "$run_dir/deferred-cursor-old" \
    -config deferred_cursor_strict_bug.cfg \
    SumeragiV2DeferredCursorMutation.tla
) >"$run_dir/deferred-cursor-old.log" 2>&1
deferred_cursor_old_status=$?
set -e

[[ $deferred_cursor_old_status -eq 13 ]] || {
  echo "old strict deferred cursor mutation did not fail with TLC status 13" >&2
  cat "$run_dir/deferred-cursor-old.log" >&2
  exit 1
}
assert_explored_counterexample \
  "deferred-cursor-old" "$run_dir/deferred-cursor-old.log" \
  "Error: Temporal properties were violated."
for marker in \
  "TLC2 Version 2.19" \
  "Temporal properties were violated." \
  "3 distinct states" \
  "Back to state 1"; do
  grep -Fq "$marker" "$run_dir/deferred-cursor-old.log" || {
    echo "old strict deferred cursor mutation missed expected marker: $marker" >&2
    cat "$run_dir/deferred-cursor-old.log" >&2
    exit 1
  }
done

(
  cd "$FORMAL_DIR"
  "${common[@]}" -metadir "$run_dir/deferred-cursor-cyclic" \
    -config deferred_cursor_cyclic.cfg \
    SumeragiV2DeferredCursorMutation.tla
) >"$run_dir/deferred-cursor-cyclic.log" 2>&1
sumeragi_v2_tlc_assert_fixed_success \
  "deferred-cursor-cyclic" "$run_dir/deferred-cursor-cyclic.log" 0
for marker in \
  "Model checking completed. No error has been found." \
  "6 distinct states" \
  "depth of the complete state graph search is 5"; do
  grep -Fq "$marker" "$run_dir/deferred-cursor-cyclic.log" || {
    cat "$run_dir/deferred-cursor-cyclic.log" >&2
    exit 1
  }
done

echo "[tlc] strict deferred class priority has the required replenishment lasso"
echo "[tlc] the cyclic deferred cursor closes the replenishment lasso"

set +e
(
  cd "$FORMAL_DIR"
  "${common[@]}" -metadir "$run_dir/deferred-busy-priority-old" \
    -config deferred_busy_priority_bug.cfg \
    SumeragiV2DeferredBusyFenceMutation.tla
) >"$run_dir/deferred-busy-priority-old.log" 2>&1
deferred_busy_priority_old_status=$?
set -e

[[ $deferred_busy_priority_old_status -eq 13 ]] || {
  echo "old Busy/deferred priority mutation did not fail with TLC status 13" >&2
  cat "$run_dir/deferred-busy-priority-old.log" >&2
  exit 1
}
assert_explored_counterexample \
  "deferred-busy-priority-old" \
  "$run_dir/deferred-busy-priority-old.log" \
  "Error: Temporal properties were violated."
for marker in \
  "TLC2 Version 2.19" \
  "Temporal properties were violated." \
  "2 distinct states" \
  "attemptParity = TRUE" \
  "Back to state 1"; do
  grep -Fq "$marker" "$run_dir/deferred-busy-priority-old.log" || {
    echo "old Busy/deferred priority mutation missed expected marker: $marker" >&2
    cat "$run_dir/deferred-busy-priority-old.log" >&2
    exit 1
  }
done

(
  cd "$FORMAL_DIR"
  "${common[@]}" -metadir "$run_dir/deferred-busy-fence" \
    -config deferred_busy_fence.cfg \
    SumeragiV2DeferredBusyFenceMutation.tla
) >"$run_dir/deferred-busy-fence.log" 2>&1
sumeragi_v2_tlc_assert_fixed_success \
  "deferred-busy-fence" "$run_dir/deferred-busy-fence.log" 0
for marker in \
  "Model checking completed. No error has been found." \
  "3 distinct states" \
  "depth of the complete state graph search is 3"; do
  grep -Fq "$marker" "$run_dir/deferred-busy-fence.log" || {
    cat "$run_dir/deferred-busy-fence.log" >&2
    exit 1
  }
done

echo "[tlc] Busy-first deferred retry priority has the required fair lasso"
echo "[tlc] the production Busy fence services Completion before deferred work"

set +e
(
  cd "$FORMAL_DIR"
  "${common[@]}" -metadir "$run_dir/busy-alias-old-rank" \
    -config busy_alias_old_rank_bug.cfg \
    SumeragiV2BusyAliasRankMutation.tla
) >"$run_dir/busy-alias-old-rank.log" 2>&1
busy_alias_old_rank_status=$?
set -e

[[ $busy_alias_old_rank_status -eq 12 ]] || {
  echo "old aliased Busy rank did not fail with TLC status 12" >&2
  cat "$run_dir/busy-alias-old-rank.log" >&2
  exit 1
}
assert_explored_counterexample \
  "busy-alias-old-rank" "$run_dir/busy-alias-old-rank.log" \
  "Error: Invariant OldIfRankDropped is violated."
for marker in \
  "Invariant OldIfRankDropped is violated." \
  'phase = "AfterSign"' \
  "2 distinct states" \
  "depth of the complete state graph search is 2"; do
  grep -Fq "$marker" "$run_dir/busy-alias-old-rank.log" || {
    echo "old aliased Busy rank missed expected marker: $marker" >&2
    cat "$run_dir/busy-alias-old-rank.log" >&2
    exit 1
  }
done

(
  cd "$FORMAL_DIR"
  "${common[@]}" -metadir "$run_dir/busy-alias-weighted-rank" \
    -config busy_alias_weighted_rank.cfg \
    SumeragiV2BusyAliasRankMutation.tla
) >"$run_dir/busy-alias-weighted-rank.log" 2>&1
sumeragi_v2_tlc_assert_fixed_success \
  "busy-alias-weighted-rank" "$run_dir/busy-alias-weighted-rank.log" 0
for marker in \
  "Model checking completed. No error has been found." \
  "2 distinct states" \
  "depth of the complete state graph search is 2"; do
  grep -Fq "$marker" "$run_dir/busy-alias-weighted-rank.log" || {
    cat "$run_dir/busy-alias-weighted-rank.log" >&2
    exit 1
  }
done

(
  cd "$FORMAL_DIR"
  "${common[@]}" -metadir "$run_dir/busy-alias-guarded-kernel" \
    -config busy_alias_guarded_kernel.cfg \
    SumeragiV2BusyAliasRankMutation.tla
) >"$run_dir/busy-alias-guarded-kernel.log" 2>&1
sumeragi_v2_tlc_assert_fixed_success \
  "busy-alias-guarded-kernel" "$run_dir/busy-alias-guarded-kernel.log" 0
for marker in \
  "Model checking completed. No error has been found." \
  "3 distinct states" \
  "depth of the complete state graph search is 3"; do
  grep -Fq "$marker" "$run_dir/busy-alias-guarded-kernel.log" || {
    cat "$run_dir/busy-alias-guarded-kernel.log" >&2
    exit 1
  }
done

echo "[tlc] the retired IF rank misses the aliased signing-lane descent"
echo "[tlc] weighted rank and guarded lane exclusion close the alias witness"

set +e
(
  cd "$FORMAL_DIR"
  "${common[@]}" -metadir "$run_dir/deferred-debt-invariant-old" \
    -config deferred_debt_inherited_invariant_bug.cfg \
    SumeragiV2DeferredDebtMutation.tla
) >"$run_dir/deferred-debt-invariant-old.log" 2>&1
deferred_debt_invariant_old_status=$?
set -e

[[ $deferred_debt_invariant_old_status -eq 12 ]] || {
  echo "inherited deferred debt did not fail with TLC status 12" >&2
  cat "$run_dir/deferred-debt-invariant-old.log" >&2
  exit 1
}
assert_explored_counterexample \
  "deferred-debt-invariant-old" \
  "$run_dir/deferred-debt-invariant-old.log" \
  "Error: Invariant DeferredDebtInvariant is violated."
for marker in \
  "Invariant DeferredDebtInvariant is violated." \
  'phase = "CompleteBusy"' \
  "2 distinct states" \
  "depth of the complete state graph search is 2"; do
  grep -Fq "$marker" "$run_dir/deferred-debt-invariant-old.log" || {
    echo "inherited deferred-debt invariant missed expected marker: $marker" >&2
    cat "$run_dir/deferred-debt-invariant-old.log" >&2
    exit 1
  }
done

set +e
(
  cd "$FORMAL_DIR"
  "${common[@]}" -metadir "$run_dir/deferred-debt-liveness-old" \
    -config deferred_debt_inherited_liveness_bug.cfg \
    SumeragiV2DeferredDebtMutation.tla
) >"$run_dir/deferred-debt-liveness-old.log" 2>&1
deferred_debt_liveness_old_status=$?
set -e

[[ $deferred_debt_liveness_old_status -eq 13 ]] || {
  echo "inherited deferred debt did not fail liveness with TLC status 13" >&2
  cat "$run_dir/deferred-debt-liveness-old.log" >&2
  exit 1
}
assert_explored_counterexample \
  "deferred-debt-liveness-old" \
  "$run_dir/deferred-debt-liveness-old.log" \
  "Error: Temporal properties were violated."
for marker in \
  "Temporal properties were violated." \
  'phase = "Drain"' \
  "State 4: Stuttering" \
  "3 distinct states"; do
  grep -Fq "$marker" "$run_dir/deferred-debt-liveness-old.log" || {
    echo "inherited deferred-debt liveness missed expected marker: $marker" >&2
    cat "$run_dir/deferred-debt-liveness-old.log" >&2
    exit 1
  }
done

(
  cd "$FORMAL_DIR"
  "${common[@]}" -metadir "$run_dir/deferred-debt-armed" \
    -config deferred_debt_armed.cfg \
    SumeragiV2DeferredDebtMutation.tla
) >"$run_dir/deferred-debt-armed.log" 2>&1
sumeragi_v2_tlc_assert_fixed_success \
  "deferred-debt-armed" "$run_dir/deferred-debt-armed.log" 0
for marker in \
  "Model checking completed. No error has been found." \
  "4 distinct states" \
  "depth of the complete state graph search is 4"; do
  grep -Fq "$marker" "$run_dir/deferred-debt-armed.log" || {
    cat "$run_dir/deferred-debt-armed.log" >&2
    exit 1
  }
done

echo "[tlc] inherited false deferred debt strands the post-Busy owner"
echo "[tlc] admission-armed debt preserves the invariant and drains fairly"

set +e
(
  cd "$FORMAL_DIR"
  "${common[@]}" -metadir "$run_dir/deferred-handoff-rebusy" \
    -config deferred_handoff_rebusy_bug.cfg \
    SumeragiV2DeferredHandoffMutation.tla
) >"$run_dir/deferred-handoff-rebusy.log" 2>&1
deferred_handoff_rebusy_status=$?
set -e

[[ $deferred_handoff_rebusy_status -eq 13 ]] || {
  echo "handoff-free deferred retry did not fail with TLC status 13" >&2
  cat "$run_dir/deferred-handoff-rebusy.log" >&2
  exit 1
}
assert_explored_counterexample \
  "deferred-handoff-rebusy" "$run_dir/deferred-handoff-rebusy.log" \
  "Error: Temporal properties were violated."
for marker in \
  "TLC2 Version 2.19" \
  "Temporal properties were violated." \
  "3 distinct states" \
  "Back to state 1"; do
  grep -Fq "$marker" "$run_dir/deferred-handoff-rebusy.log" || {
    echo "deferred re-Busy mutation missed expected marker: $marker" >&2
    cat "$run_dir/deferred-handoff-rebusy.log" >&2
    exit 1
  }
done

(
  cd "$FORMAL_DIR"
  "${common[@]}" -metadir "$run_dir/deferred-handoff-exact" \
    -config deferred_handoff_exact.cfg \
    SumeragiV2DeferredHandoffMutation.tla
) >"$run_dir/deferred-handoff-exact.log" 2>&1
sumeragi_v2_tlc_assert_fixed_success \
  "deferred-handoff-exact" "$run_dir/deferred-handoff-exact.log" 0
grep -Fq "Model checking completed. No error has been found." \
  "$run_dir/deferred-handoff-exact.log" || {
  cat "$run_dir/deferred-handoff-exact.log" >&2
  exit 1
}

echo "[tlc] an unowned retry admits the three-state equal-rank re-Busy lasso"
echo "[tlc] the exact handoff lets Completion finish and blocks foreign re-Busy"

set +e
(
  cd "$FORMAL_DIR"
  "${common[@]}" -metadir "$run_dir/head-only" \
    -config ingress_head_blocking_bug.cfg \
    SumeragiV2IngressMutation.tla
) >"$run_dir/head-only.log" 2>&1
head_only_status=$?
set -e

[[ $head_only_status -eq 13 ]] || {
  echo "old head-only ingress mutation did not fail with TLC status 13" >&2
  cat "$run_dir/head-only.log" >&2
  exit 1
}
assert_explored_counterexample \
  "head-only" "$run_dir/head-only.log" \
  "Error: Temporal properties were violated."
for marker in \
  "TLC2 Version 2.19" \
  "Temporal properties were violated." \
  "1 distinct state" \
  "State 2: Stuttering"; do
  grep -Fq "$marker" "$run_dir/head-only.log" || {
    echo "old head-only ingress mutation missed expected marker: $marker" >&2
    cat "$run_dir/head-only.log" >&2
    exit 1
  }
done

(
  cd "$FORMAL_DIR"
  "${common[@]}" -metadir "$run_dir/indexed-scan" \
    -config ingress_indexed_scan.cfg \
    SumeragiV2IngressMutation.tla
) >"$run_dir/indexed-scan.log" 2>&1
sumeragi_v2_tlc_assert_fixed_success \
  "indexed-scan" "$run_dir/indexed-scan.log" 0
grep -Fq "Model checking completed. No error has been found." \
  "$run_dir/indexed-scan.log" || {
  cat "$run_dir/indexed-scan.log" >&2
  exit 1
}

echo "[tlc] head-only same-source service has the required stuttering lasso"
echo "[tlc] oldest-admissible indexed removal closes that lasso"

set +e
(
  cd "$FORMAL_DIR"
  "${common[@]}" -metadir "$run_dir/capacity-old" \
    -config ingress_capacity_removal_bug.cfg \
    SumeragiV2IngressCapacityMutation.tla
) >"$run_dir/capacity-old.log" 2>&1
capacity_old_status=$?
set -e

[[ $capacity_old_status -eq 12 ]] || {
  echo "old ingress capacity removal mutation did not fail with TLC status 12" >&2
  cat "$run_dir/capacity-old.log" >&2
  exit 1
}
assert_explored_counterexample \
  "capacity-old" "$run_dir/capacity-old.log" \
  "Error: Invariant OldCapacityInvariant is violated."
for marker in \
  "TLC2 Version 2.19" \
  "Invariant OldCapacityInvariant is violated." \
  "2 distinct states" \
  "lane = <<\"Auxiliary\", \"Auxiliary\">>"; do
  grep -Fq "$marker" "$run_dir/capacity-old.log" || {
    echo "old ingress capacity removal mutation missed expected marker: $marker" >&2
    cat "$run_dir/capacity-old.log" >&2
    exit 1
  }
done

(
  cd "$FORMAL_DIR"
  "${common[@]}" -metadir "$run_dir/capacity-bounded" \
    -config ingress_capacity_lane_bound.cfg \
    SumeragiV2IngressCapacityMutation.tla
) >"$run_dir/capacity-bounded.log" 2>&1
sumeragi_v2_tlc_assert_fixed_success \
  "capacity-bounded" "$run_dir/capacity-bounded.log" 0
grep -Fq "Model checking completed. No error has been found." \
  "$run_dir/capacity-bounded.log" || {
  cat "$run_dir/capacity-bounded.log" >&2
  exit 1
}

echo "[tlc] an overlong lane makes removal violate the old aggregate invariant"
echo "[tlc] the per-lane capacity bound makes the same removal invariant-safe"

set +e
(
  cd "$FORMAL_DIR"
  "${common[@]}" -metadir "$run_dir/completion-capacity-conflated" \
    -config completion_capacity_conflated_bug.cfg \
    SumeragiV2CompletionCapacityMutation.tla
) >"$run_dir/completion-capacity-conflated.log" 2>&1
completion_capacity_conflated_status=$?
set -e

[[ $completion_capacity_conflated_status -eq 13 ]] || {
  echo "conflated work/completion capacity mutation did not fail with TLC status 13" >&2
  cat "$run_dir/completion-capacity-conflated.log" >&2
  exit 1
}
assert_explored_counterexample \
  "completion-capacity-conflated" \
  "$run_dir/completion-capacity-conflated.log" \
  "Error: Temporal properties were violated."
for marker in \
  "TLC2 Version 2.19" \
  "Temporal properties were violated." \
  "2 distinct states" \
  "Back to state 1"; do
  grep -Fq "$marker" "$run_dir/completion-capacity-conflated.log" || {
    echo "conflated work/completion capacity mutation missed expected marker: $marker" >&2
    cat "$run_dir/completion-capacity-conflated.log" >&2
    exit 1
  }
done

(
  cd "$FORMAL_DIR"
  "${common[@]}" -metadir "$run_dir/completion-capacity-separated" \
    -config completion_capacity_separated.cfg \
    SumeragiV2CompletionCapacityMutation.tla
) >"$run_dir/completion-capacity-separated.log" 2>&1
sumeragi_v2_tlc_assert_fixed_success \
  "completion-capacity-separated" \
  "$run_dir/completion-capacity-separated.log" 0
for marker in \
  "Model checking completed. No error has been found." \
  "4 distinct states" \
  "depth of the complete state graph search is 3"; do
  grep -Fq "$marker" "$run_dir/completion-capacity-separated.log" || {
    cat "$run_dir/completion-capacity-separated.log" >&2
    exit 1
  }
done

echo "[tlc] conflating pending-work and completion ownership has the required fair Tick lasso"
echo "[tlc] separate pending-work admission lets the required causal completion execute"

set +e
(
  cd "$FORMAL_DIR"
  "${common[@]}" -metadir "$run_dir/local-admission-producer-first" \
    -config local_admission_producer_first_bug.cfg \
    SumeragiV2LocalAdmissionMutation.tla
) >"$run_dir/local-admission-producer-first.log" 2>&1
local_admission_producer_first_status=$?
set -e

[[ $local_admission_producer_first_status -eq 13 ]] || {
  echo "producer-first local admission mutation did not fail with TLC status 13" >&2
  cat "$run_dir/local-admission-producer-first.log" >&2
  exit 1
}
assert_explored_counterexample \
  "local-admission-producer-first" \
  "$run_dir/local-admission-producer-first.log" \
  "Error: Temporal properties were violated."
for marker in \
  "TLC2 Version 2.19" \
  "Temporal properties were violated." \
  "3 distinct states" \
  "Back to state 2"; do
  grep -Fq "$marker" "$run_dir/local-admission-producer-first.log" || {
    echo "producer-first local admission mutation missed expected marker: $marker" >&2
    cat "$run_dir/local-admission-producer-first.log" >&2
    exit 1
  }
done

(
  cd "$FORMAL_DIR"
  "${common[@]}" -metadir "$run_dir/local-admission-alternating" \
    -config local_admission_alternating.cfg \
    SumeragiV2LocalAdmissionMutation.tla
) >"$run_dir/local-admission-alternating.log" 2>&1
sumeragi_v2_tlc_assert_fixed_success \
  "local-admission-alternating" "$run_dir/local-admission-alternating.log" 0
for marker in \
  "Model checking completed. No error has been found." \
  "7 distinct states" \
  "depth of the complete state graph search is 7"; do
  grep -Fq "$marker" "$run_dir/local-admission-alternating.log" || {
    cat "$run_dir/local-admission-alternating.log" >&2
    exit 1
  }
done

echo "[tlc] producer-first local admission has the required three-state fair lasso"
echo "[tlc] causal debt and the alternating source cursor service the causal owner"

set +e
(
  cd "$FORMAL_DIR"
  "${common[@]}" -metadir "$run_dir/serve-nonce-reuse" \
    -config serve_nonce_reuse_bug.cfg \
    SumeragiV2ServeNonceMutation.tla
) >"$run_dir/serve-nonce-reuse.log" 2>&1
serve_nonce_reuse_status=$?
set -e

[[ $serve_nonce_reuse_status -eq 13 ]] || {
  echo "live Serve nonce reuse did not fail with TLC status 13" >&2
  cat "$run_dir/serve-nonce-reuse.log" >&2
  exit 1
}
assert_explored_counterexample \
  "serve-nonce-reuse" "$run_dir/serve-nonce-reuse.log" \
  "Error: Temporal properties were violated."
for marker in \
  "TLC2 Version 2.19" \
  "Temporal properties were violated." \
  "3 distinct states" \
  "Back to state 1"; do
  grep -Fq "$marker" "$run_dir/serve-nonce-reuse.log" || {
    echo "live Serve nonce reuse missed expected marker: $marker" >&2
    cat "$run_dir/serve-nonce-reuse.log" >&2
    exit 1
  }
done

(
  cd "$FORMAL_DIR"
  "${common[@]}" -metadir "$run_dir/serve-nonce-fresh" \
    -config serve_nonce_fresh.cfg \
    SumeragiV2ServeNonceMutation.tla
) >"$run_dir/serve-nonce-fresh.log" 2>&1
sumeragi_v2_tlc_assert_fixed_success \
  "serve-nonce-fresh" "$run_dir/serve-nonce-fresh.log" 0
for marker in \
  "Model checking completed. No error has been found." \
  "4 distinct states" \
  "depth of the complete state graph search is 3"; do
  grep -Fq "$marker" "$run_dir/serve-nonce-fresh.log" || {
    cat "$run_dir/serve-nonce-fresh.log" >&2
    exit 1
  }
done

echo "[tlc] live Serve nonce reuse has the required fair replacement lasso"
echo "[tlc] a fresh live nonce makes service exit the original occurrence"

set +e
(
  cd "$FORMAL_DIR"
  "${common[@]}" -metadir "$run_dir/deferred-primed-selector-old" \
    -config deferred_primed_selector_bug.cfg \
    SumeragiV2DeferredPrimedSelectorMutation.tla
) >"$run_dir/deferred-primed-selector-old.log" 2>&1
deferred_primed_selector_old_status=$?
set -e

[[ $deferred_primed_selector_old_status -eq 12 ]] || {
  echo "primed deferred selector did not fail with TLC status 12" >&2
  cat "$run_dir/deferred-primed-selector-old.log" >&2
  exit 1
}
assert_explored_counterexample \
  "deferred-primed-selector-old" \
  "$run_dir/deferred-primed-selector-old.log" \
  "Error: Invariant OldPrimedSelectorClaimHeld is violated."
for marker in \
  "TLC2 Version 2.19" \
  "Invariant OldPrimedSelectorClaimHeld is violated." \
  'nextDeferredClass = [node |-> "Progress"]' \
  'phase = "Drained"' \
  "2 distinct states" \
  "depth of the complete state graph search is 2"; do
  grep -Fq "$marker" "$run_dir/deferred-primed-selector-old.log" || {
    echo "primed deferred selector missed expected marker: $marker" >&2
    cat "$run_dir/deferred-primed-selector-old.log" >&2
    exit 1
  }
done

(
  cd "$FORMAL_DIR"
  "${common[@]}" -metadir "$run_dir/deferred-rigid-target-class" \
    -config deferred_rigid_target_class.cfg \
    SumeragiV2DeferredPrimedSelectorMutation.tla
) >"$run_dir/deferred-rigid-target-class.log" 2>&1
sumeragi_v2_tlc_assert_fixed_success \
  "deferred-rigid-target-class" \
  "$run_dir/deferred-rigid-target-class.log" 0
for marker in \
  "Model checking completed. No error has been found." \
  "6 distinct states" \
  "depth of the complete state graph search is 2"; do
  grep -Fq "$marker" "$run_dir/deferred-rigid-target-class.log" || {
    cat "$run_dir/deferred-rigid-target-class.log" >&2
    exit 1
  }
done

echo "[tlc] priming the deferred selector switches the claimed queue after Completion drain"
echo "[tlc] a rigid target class validates selected-tail and foreign-class preservation exhaustively"

set +e
(
  cd "$FORMAL_DIR"
  "${common[@]}" -metadir "$run_dir/busy-witness-stale-readiness" \
    -config busy_witness_stale_readiness_bug.cfg \
    SumeragiV2BusyWitnessKernelMutation.tla
) >"$run_dir/busy-witness-stale-readiness.log" 2>&1
busy_witness_stale_readiness_status=$?
set -e

[[ $busy_witness_stale_readiness_status -eq 12 ]] || {
  echo "stale Busy witness did not fail with TLC status 12" >&2
  cat "$run_dir/busy-witness-stale-readiness.log" >&2
  exit 1
}
assert_explored_counterexample \
  "busy-witness-stale-readiness" \
  "$run_dir/busy-witness-stale-readiness.log" \
  "Error: Invariant OldActiveBusyWitnessInvariant is violated."
for marker in \
  "TLC2 Version 2.19" \
  "Invariant OldActiveBusyWitnessInvariant is violated." \
  'phase = "Deferred"' \
  "activeCompletion = FALSE" \
  "deferredCompletion = TRUE" \
  "2 distinct states" \
  "depth of the complete state graph search is 2"; do
  grep -Fq "$marker" "$run_dir/busy-witness-stale-readiness.log" || {
    echo "stale Busy witness missed expected marker: $marker" >&2
    cat "$run_dir/busy-witness-stale-readiness.log" >&2
    exit 1
  }
done

(
  cd "$FORMAL_DIR"
  "${common[@]}" -metadir "$run_dir/busy-witness-combined-kernel" \
    -config busy_witness_combined_kernel.cfg \
    SumeragiV2BusyWitnessKernelMutation.tla
) >"$run_dir/busy-witness-combined-kernel.log" 2>&1
sumeragi_v2_tlc_assert_fixed_success \
  "busy-witness-combined-kernel" \
  "$run_dir/busy-witness-combined-kernel.log" 0
for marker in \
  "Model checking completed. No error has been found." \
  "8 distinct states" \
  "depth of the complete state graph search is 2"; do
  grep -Fq "$marker" "$run_dir/busy-witness-combined-kernel.log" || {
    cat "$run_dir/busy-witness-combined-kernel.log" >&2
    exit 1
  }
done

echo "[tlc] stale Proposal readiness defers the sole active Busy completion witness"
echo "[tlc] the combined serialized-owner/readiness kernel blocks every invalid row"

set +e
(
  cd "$FORMAL_DIR"
  "${common[@]}" -metadir "$run_dir/persist-install-height-alias" \
    -config persist_install_height_alias_bug.cfg \
    SumeragiV2PersistInstallHeightAliasMutation.tla
) >"$run_dir/persist-install-height-alias.log" 2>&1
persist_install_height_alias_status=$?
set -e

[[ $persist_install_height_alias_status -eq 12 ]] || {
  echo "redundant-height InstallTC alias did not fail with TLC status 12" >&2
  cat "$run_dir/persist-install-height-alias.log" >&2
  exit 1
}
assert_explored_counterexample \
  "persist-install-height-alias" \
  "$run_dir/persist-install-height-alias.log" \
  "Error: Invariant SerializedBusyOwnershipInvariant is violated."
for marker in \
  "TLC2 Version 2.19" \
  "Invariant SerializedBusyOwnershipInvariant is violated." \
  'phase = "AfterInstall"' \
  "signOwnerCount = 2" \
  "2 distinct states" \
  "depth of the complete state graph search is 2"; do
  grep -Fq "$marker" "$run_dir/persist-install-height-alias.log" || {
    echo "redundant-height InstallTC alias missed expected marker: $marker" >&2
    cat "$run_dir/persist-install-height-alias.log" >&2
    exit 1
  }
done

set +e
(
  cd "$FORMAL_DIR"
  "${common[@]}" -metadir "$run_dir/persist-install-canonical-vote" \
    -config persist_install_canonical_vote.cfg \
    SumeragiV2PersistInstallHeightAliasMutation.tla
) >"$run_dir/persist-install-canonical-vote.log" 2>&1
persist_install_canonical_vote_status=$?
set -e

[[ $persist_install_canonical_vote_status -eq 0 ]] || {
  echo "canonical full-Vote InstallTC mutation did not pass with TLC status 0" >&2
  cat "$run_dir/persist-install-canonical-vote.log" >&2
  exit 1
}
sumeragi_v2_tlc_assert_fixed_success \
  "persist-install-canonical-vote" \
  "$run_dir/persist-install-canonical-vote.log" \
  "$persist_install_canonical_vote_status"
for marker in \
  "Model checking completed. No error has been found." \
  "4 distinct states generated" \
  "8 distinct states" \
  "depth of the complete state graph search is 2"; do
  grep -Fq "$marker" "$run_dir/persist-install-canonical-vote.log" || {
    echo "canonical full-Vote InstallTC mutation missed expected marker: $marker" >&2
    cat "$run_dir/persist-install-canonical-vote.log" >&2
    exit 1
  }
done

echo "[tlc] field-only locked Commit selection reproduces the two-owner height alias"
echo "[tlc] canonical full-Vote selection exhaustively preserves empty/singleton readiness"

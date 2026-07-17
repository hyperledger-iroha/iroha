#!/usr/bin/env bash
set -euo pipefail

readonly TLA2TOOLS_VERSION="1.7.4"
readonly TLA2TOOLS_SHA256="936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88"
readonly REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly FORMAL_DIR="${REPO_ROOT}/docs/formal/sumeragi_v2"
readonly TLA2TOOLS_JAR="${TLA2TOOLS_JAR:-${REPO_ROOT}/target/tla2tools/${TLA2TOOLS_VERSION}/tla2tools.jar}"
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
  "${common[@]}" -metadir "$run_dir/deferred-busy-cursor-old" \
    -config deferred_busy_cursor_strict_bug.cfg \
    SumeragiV2DeferredBusyCursorMutation.tla
) >"$run_dir/deferred-busy-cursor-old.log" 2>&1
deferred_busy_cursor_old_status=$?
set -e

[[ $deferred_busy_cursor_old_status -eq 13 ]] || {
  echo "old Busy deferred cursor mutation did not fail with TLC status 13" >&2
  cat "$run_dir/deferred-busy-cursor-old.log" >&2
  exit 1
}
for marker in \
  "TLC2 Version 2.19" \
  "Temporal properties were violated." \
  "2 distinct states" \
  "busyAttemptParity = TRUE" \
  "Back to state 1"; do
  grep -Fq "$marker" "$run_dir/deferred-busy-cursor-old.log" || {
    echo "old Busy deferred cursor mutation missed expected marker: $marker" >&2
    cat "$run_dir/deferred-busy-cursor-old.log" >&2
    exit 1
  }
done

(
  cd "$FORMAL_DIR"
  "${common[@]}" -metadir "$run_dir/deferred-busy-cursor-cyclic" \
    -config deferred_busy_cursor_cyclic.cfg \
    SumeragiV2DeferredBusyCursorMutation.tla
) >"$run_dir/deferred-busy-cursor-cyclic.log" 2>&1
for marker in \
  "Model checking completed. No error has been found." \
  "5 distinct states" \
  "depth of the complete state graph search is 5"; do
  grep -Fq "$marker" "$run_dir/deferred-busy-cursor-cyclic.log" || {
    cat "$run_dir/deferred-busy-cursor-cyclic.log" >&2
    exit 1
  }
done

echo "[tlc] Busy Completion requeue without cursor advance has a fair lasso"
echo "[tlc] Busy Completion requeue with cursor advance services Progress"

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

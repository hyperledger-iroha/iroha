#!/usr/bin/env bash
set -euo pipefail

readonly TLA2TOOLS_VERSION="1.7.4"
readonly TLA2TOOLS_SHA256="936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88"
readonly REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly FORMAL_DIR="${REPO_ROOT}/docs/formal/sumeragi_v2"
readonly TLA2TOOLS_JAR="${TLA2TOOLS_JAR:-${REPO_ROOT}/target/tla2tools/${TLA2TOOLS_VERSION}/tla2tools.jar}"
readonly JAVA_BIN="${JAVA_BIN:-java}"

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

[[ $old_status -ne 0 ]] || {
  echo "old value-rank mutation unexpectedly passed" >&2
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
  "${common[@]}" -metadir "$run_dir/head-only" \
    -config ingress_head_blocking_bug.cfg \
    SumeragiV2IngressMutation.tla
) >"$run_dir/head-only.log" 2>&1
head_only_status=$?
set -e

[[ $head_only_status -ne 0 ]] || {
  echo "old head-only ingress mutation unexpectedly passed" >&2
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

[[ $capacity_old_status -ne 0 ]] || {
  echo "old ingress capacity removal mutation unexpectedly passed" >&2
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

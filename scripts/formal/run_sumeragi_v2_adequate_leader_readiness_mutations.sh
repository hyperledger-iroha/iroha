#!/usr/bin/env bash
# Run the bounded adequate-leader scheduler-readiness mutation matrix.

set -euo pipefail

readonly TLA2TOOLS_VERSION="1.7.4"
readonly TLA2TOOLS_SHA256="936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88"
readonly EXPECTED_JAVA_VERSION='openjdk version "21.0.12"'
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
java_version="$($JAVA_BIN -version 2>&1)"
grep -Fq "$EXPECTED_JAVA_VERSION" <<<"$java_version" || {
  echo "frozen Java 21.0.12 is required" >&2
  printf '%s\n' "$java_version" >&2
  exit 1
}

run_dir="$(mktemp -d "${TMPDIR:-/tmp}/sumeragi-adequate-readiness.XXXXXX")"
trap 'rm -rf -- "$run_dir"' EXIT

models=(
  SumeragiV2AdequateLeaderCrashParkingMutation
  SumeragiV2AdequateLeaderMutualOwnerAnchorMutation
  SumeragiV2AdequateLeaderBusyIdleProvenanceMutation
)

for module in "${models[@]}"; do
  (
    cd "$FORMAL_DIR"
    "$JAVA_BIN" -cp "$TLA2TOOLS_JAR" tla2sany.SANY "${module}.tla"
  ) >"${run_dir}/${module}.sany.log" 2>&1
  sany_last_nonblank="$(
    awk 'NF { line = $0 } END { print line }' \
      "${run_dir}/${module}.sany.log"
  )"
  expected_marker="Semantic processing of module ${module}"
  [[ "$sany_last_nonblank" == "$expected_marker" ]] || {
    echo "${module}: SANY did not end at the expected marker" >&2
    cat "${run_dir}/${module}.sany.log" >&2
    exit 1
  }
done
echo "[sany] adequate-leader readiness mutations parsed with frozen Java 21.0.12"

common=(
  "$JAVA_BIN" -XX:+UseParallelGC -cp "$TLA2TOOLS_JAR" tlc2.TLC
  -cleanup -workers 1 -fp 96 -seed 139154308881391968
)

run_case() {
  local label="$1"
  local model="$2"
  local config="$3"
  local expected_status="$4"
  shift 4
  local log="${run_dir}/${label}.log"
  local actual_status
  set +e
  (
    cd "$FORMAL_DIR"
    "${common[@]}" -metadir "${run_dir}/${label}" \
      -config "$config" "$model"
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

run_case crash-parking-fixed \
  SumeragiV2AdequateLeaderCrashParkingMutation.tla \
  adequate_leader_crash_parking_fixed.cfg 0 \
  "Model checking completed. No error has been found." \
  "2 states generated, 2 distinct states found, 0 states left on queue." \
  "depth of the complete state graph search is 2"

run_case crash-parking-mutation \
  SumeragiV2AdequateLeaderCrashParkingMutation.tla \
  adequate_leader_crash_parking_bug.cfg 12 \
  "Invariant ExactLeaderSchedulerOriginReadiness is violated." \
  'phase = "Crashed"' \
  "2 states generated, 2 distinct states found, 0 states left on queue." \
  "depth of the complete state graph search is 2"

run_case mutual-owner-anchor-fixed \
  SumeragiV2AdequateLeaderMutualOwnerAnchorMutation.tla \
  adequate_leader_mutual_owner_anchor_fixed.cfg 0 \
  "Model checking completed. No error has been found." \
  "2 states generated, 2 distinct states found, 0 states left on queue." \
  "depth of the complete state graph search is 2"

run_case mutual-owner-anchor-mutation \
  SumeragiV2AdequateLeaderMutualOwnerAnchorMutation.tla \
  adequate_leader_mutual_owner_anchor_bug.cfg 12 \
  "Invariant ExactLeaderSchedulerOriginReadiness is violated." \
  'phase = "Discarded"' \
  "3 states generated, 3 distinct states found, 0 states left on queue." \
  "depth of the complete state graph search is 3"

run_case busy-idle-provenance-fixed \
  SumeragiV2AdequateLeaderBusyIdleProvenanceMutation.tla \
  adequate_leader_busy_idle_provenance_fixed.cfg 0 \
  "Model checking completed. No error has been found." \
  "2 states generated, 2 distinct states found, 0 states left on queue." \
  "depth of the complete state graph search is 2"

run_case busy-idle-provenance-mutation \
  SumeragiV2AdequateLeaderBusyIdleProvenanceMutation.tla \
  adequate_leader_busy_idle_provenance_bug.cfg 12 \
  "Invariant ExactLeaderSchedulerOriginReadiness is violated." \
  'phase = "Idle"' \
  "2 states generated, 2 distinct states found, 0 states left on queue." \
  "depth of the complete state graph search is 2"

echo "[tlc] adequate-leader readiness mutation matrix passed"

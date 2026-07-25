#!/usr/bin/env bash
# Run the bounded locked-Prepare restart Fetch-before-Sign mutation matrix.

set -euo pipefail

readonly TLA2TOOLS_VERSION="1.7.4"
readonly TLA2TOOLS_SHA256="936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88"
readonly REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly FORMAL_DIR="${REPO_ROOT}/docs/formal/sumeragi_v2"
readonly MODEL="SumeragiV2RestartLockedFetchOrderMutation.tla"
readonly SANY_SUCCESS_MARKER="Semantic processing of module SumeragiV2RestartLockedFetchOrderMutation"
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

run_dir="$(mktemp -d "${TMPDIR:-/tmp}/sumeragi-restart-fetch-order.XXXXXX")"
trap 'rm -rf -- "$run_dir"' EXIT

(
  cd "$FORMAL_DIR"
  "$JAVA_BIN" -cp "$TLA2TOOLS_JAR" tla2sany.SANY "$MODEL"
) >"${run_dir}/sany.log" 2>&1
sany_last_nonblank="$(awk 'NF { line = $0 } END { print line }' "${run_dir}/sany.log")"
[[ "$sany_last_nonblank" == "$SANY_SUCCESS_MARKER" ]] || {
  echo "SANY did not end at the expected semantic-processing marker" >&2
  cat "${run_dir}/sany.log" >&2
  exit 1
}
echo "[sany] locked-Prepare restart Fetch-before-Sign model parsed"

common=(
  "$JAVA_BIN" -XX:+UseParallelGC -cp "$TLA2TOOLS_JAR" tlc2.TLC
  -cleanup -workers 1 -fp 96 -seed 139154308881391968
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
      -config "$config" "$MODEL"
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

run_case repaired \
  restart_locked_fetch_order_fixed.cfg 0 \
  "TLC2 Version 2.19" \
  "Model checking completed. No error has been found."

run_case dropped-fetch \
  restart_locked_fetch_order_drop_fetch_bug.cfg 12 \
  "Invariant RestartReplayHasExactOwnership is violated."

run_case reversed-order \
  restart_locked_fetch_order_reversed_bug.cfg 12 \
  "Invariant LockedFetchPrecedesFirstSign is violated."

echo "[tlc] repaired restart replay retained both exact owners and placed FetchBody before Sign"
echo "[tlc] dropped-Fetch and reversed-order mutants failed their named invariants"

#!/usr/bin/env bash
# Run the bounded exact certified/Commit-response registration mutation matrix.

set -euo pipefail

readonly TLA2TOOLS_VERSION="1.7.4"
readonly TLA2TOOLS_SHA256="936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88"
readonly EXPECTED_JAVA_VERSION='openjdk version "21.0.12"'
readonly SANY_SUCCESS_MARKER="Semantic processing of module SumeragiV2CertifiedResponseRegistrationMutation"
readonly REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly FORMAL_DIR="${REPO_ROOT}/formal/sumeragi_v2"
readonly MODEL="SumeragiV2CertifiedResponseRegistrationMutation.tla"
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

run_dir="$(mktemp -d "${TMPDIR:-/tmp}/sumeragi-certified-response-registration.XXXXXX")"
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
echo "[sany] certified-response registration model parsed with frozen Java 21.0.12"

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

run_case duplicate-fixed \
  certified_response_registration_duplicate_fixed.cfg 0 \
  "Model checking completed. No error has been found." \
  "5 states generated, 5 distinct states found, 0 states left on queue." \
  "depth of the complete state graph search is 5"
run_case commit-certificate-fanout-fixed \
  certified_response_registration_commit_fanout_fixed.cfg 0 \
  "Model checking completed. No error has been found." \
  "5 states generated, 5 distinct states found, 0 states left on queue." \
  "depth of the complete state graph search is 5"
run_case restart-fixed \
  certified_response_registration_restart_fixed.cfg 0 \
  "Model checking completed. No error has been found." \
  "11 states generated, 11 distinct states found, 0 states left on queue." \
  "depth of the complete state graph search is 11"
run_case historical-fixed \
  certified_response_registration_historical_fixed.cfg 0 \
  "Model checking completed. No error has been found." \
  "8 states generated, 8 distinct states found, 0 states left on queue." \
  "depth of the complete state graph search is 8"
run_case duplicate-missing-guard \
  certified_response_registration_duplicate_missing_guard.cfg 12 \
  "Invariant AcceptedOnlyWhileOutstanding is violated." \
  "5 states generated, 5 distinct states found, 0 states left on queue." \
  "depth of the complete state graph search is 5"
run_case commit-certificate-route-only-retirement \
  certified_response_registration_commit_fanout_route_only_bug.cfg 12 \
  "Invariant CommitFanoutFirstAcceptedResponseRetiresAllRouteAliases is violated." \
  "4 states generated, 4 distinct states found, 0 states left on queue." \
  "depth of the complete state graph search is 4"
run_case restart-missing-guard \
  certified_response_registration_restart_missing_guard.cfg 12 \
  "Invariant AcceptedOnlyWhileOutstanding is violated." \
  "7 states generated, 7 distinct states found, 0 states left on queue." \
  "depth of the complete state graph search is 7"

echo "[tlc] certified-response registration mutation matrix passed"

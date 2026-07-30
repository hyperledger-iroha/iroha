#!/usr/bin/env bash
# Run the indexed successor service-activation mutation pairs.

set -euo pipefail

readonly TLA2TOOLS_VERSION="1.7.4"
readonly TLA2TOOLS_SHA256="936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88"
readonly EXPECTED_JAVA_VERSION='openjdk version "21.0.12"'
readonly TLC_FINISHED_PATTERN='^Finished in (([0-9]+d )?([0-9]+h )?([0-9]+min )?[0-9]+(ms|s)|([0-9]+d )?([0-9]+h )?[0-9]+min|([0-9]+d )?[0-9]+h|[0-9]+d) at \([0-9]{4}-[0-9]{2}-[0-9]{2} [0-9]{2}:[0-9]{2}:[0-9]{2}\)$'
readonly REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly FORMAL_DIR="${REPO_ROOT}/docs/formal/sumeragi_v2"
readonly MODEL="SumeragiV2IndexedServiceActivationMutation.tla"
readonly FIXED_LIVENESS_CONFIG="indexed_service_activation_fixed.cfg"
readonly UNJOINED_CLOCK_CONFIG="indexed_service_activation_unjoined_clock_bug.cfg"
readonly FIXED_REENTRY_CONFIG="indexed_service_activation_reentry_fixed.cfg"
readonly REENTRY_CONFIG="indexed_service_activation_reentry_bug.cfg"
readonly TLA2TOOLS_JAR="${TLA2TOOLS_JAR:-${REPO_ROOT}/target/tla2tools/${TLA2TOOLS_VERSION}/tla2tools.jar}"

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
if [[ -n "${JAVA_BIN:-}" ]]; then
  resolved_java_bin="$("${REPO_ROOT}/scripts/formal/resolve_java.sh" "$JAVA_BIN")"
else
  resolved_java_bin="$("${REPO_ROOT}/scripts/formal/resolve_java.sh")"
fi
readonly JAVA_BIN="$resolved_java_bin"
java_version="$($JAVA_BIN -version 2>&1)"
grep -Fq "$EXPECTED_JAVA_VERSION" <<<"$java_version" || {
  echo "frozen Java 21.0.12 is required" >&2
  printf '%s\n' "$java_version" >&2
  exit 1
}

run_dir="$(mktemp -d "${TMPDIR:-/tmp}/sumeragi-indexed-service-activation.XXXXXX")"
trap 'rm -rf -- "$run_dir"' EXIT

(
  cd "$FORMAL_DIR"
  "$JAVA_BIN" -cp "$TLA2TOOLS_JAR" tla2sany.SANY "$MODEL"
) >"${run_dir}/sany.log" 2>&1
readonly SANY_SUCCESS_MARKER="Semantic processing of module ${MODEL%.tla}"
sany_last_nonblank="$(awk 'NF { line = $0 } END { print line }' "${run_dir}/sany.log")"
[[ "$sany_last_nonblank" == "$SANY_SUCCESS_MARKER" ]] || {
  echo "indexed service-activation model did not reach the exact SANY marker" >&2
  cat "${run_dir}/sany.log" >&2
  exit 1
}
[[ "$(grep -Fxc "$SANY_SUCCESS_MARKER" "${run_dir}/sany.log" || true)" == 1 ]] || {
  echo "indexed service-activation model emitted an ambiguous SANY marker" >&2
  cat "${run_dir}/sany.log" >&2
  exit 1
}

common=(
  "$JAVA_BIN" -XX:+UseParallelGC -cp "$TLA2TOOLS_JAR" tlc2.TLC
  -cleanup -workers 1 -fp 91 -seed 238951927416330671
)

run_tlc() {
  local label="$1"
  local config="$2"
  local expected_status="$3"
  local log="${run_dir}/${label}.log"
  local actual_status
  local last_nonblank
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
  [[ "$(grep -Ec "$TLC_FINISHED_PATTERN" "$log" || true)" == 1 ]] || {
    echo "${label} did not emit exactly one TLC terminal marker" >&2
    cat "$log" >&2
    exit 1
  }
  last_nonblank="$(awk 'NF { line = $0 } END { print line }' "$log")"
  grep -Eq "$TLC_FINISHED_PATTERN" <<<"$last_nonblank" || {
    echo "${label} did not end at the TLC terminal marker" >&2
    cat "$log" >&2
    exit 1
  }
  printf '%s\n' "$log"
}

fixed_liveness_log="$(run_tlc fixed-activation "$FIXED_LIVENESS_CONFIG" 0)"
fixed_reentry_log="$(run_tlc fixed-reentry "$FIXED_REENTRY_CONFIG" 0)"
for log in "$fixed_liveness_log" "$fixed_reentry_log"; do
  [[ "$(grep -Fxc "Model checking completed. No error has been found." "$log" || true)" == 1 ]] || {
    echo "repaired indexed service-activation case missed exact completion" >&2
    cat "$log" >&2
    exit 1
  }
  [[ "$(grep -Ec '^(Error:|Deadlock reached[.])' "$log" || true)" == 0 ]] || {
    echo "repaired indexed service-activation case emitted a failure diagnostic" >&2
    cat "$log" >&2
    exit 1
  }
done

unjoined_clock_log="$(run_tlc unjoined-clock-owner "$UNJOINED_CLOCK_CONFIG" 13)"
for marker in "Temporal properties were violated." "Stuttering"; do
  grep -Fq "$marker" "$unjoined_clock_log" || {
    echo "unjoined clock-owner lasso missed TLC marker: ${marker}" >&2
    cat "$unjoined_clock_log" >&2
    exit 1
  }
done
if grep -Fq "Error: Invariant " "$unjoined_clock_log"; then
  echo "unjoined clock-owner lasso was misclassified as an invariant failure" >&2
  cat "$unjoined_clock_log" >&2
  exit 1
fi

reentry_log="$(run_tlc restriction-reentry "$REENTRY_CONFIG" 12)"
readonly REENTRY_INVARIANT_MARKER="Error: Invariant ActivationMembershipCoherence is violated."
[[ "$(grep -Fxc "$REENTRY_INVARIANT_MARKER" "$reentry_log" || true)" == 1 ]] || {
  echo "restriction re-entry missed the exact activation-coherence invariant" >&2
  cat "$reentry_log" >&2
  exit 1
}
if grep -Fq "Temporal properties were violated." "$reentry_log"; then
  echo "restriction re-entry was misclassified as a liveness failure" >&2
  cat "$reentry_log" >&2
  exit 1
fi

echo "[tlc] repaired indexed service activation and re-entry models passed"
echo "[tlc] unjoined clock ownership produced exact liveness status 13; restriction re-entry produced exact invariant status 12"

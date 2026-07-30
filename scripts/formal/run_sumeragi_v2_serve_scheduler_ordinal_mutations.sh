#!/usr/bin/env bash
# Run shared exact-Serve/Runtime scheduler-ordinal mutation pairs.

set -euo pipefail

readonly TLA2TOOLS_VERSION="1.7.4"
readonly TLA2TOOLS_SHA256="936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88"
readonly EXPECTED_JAVA_VERSION='openjdk version "21.0.12"'
readonly TLC_FINISHED_PATTERN='^Finished in (([0-9]+d )?([0-9]+h )?([0-9]+min )?[0-9]+(ms|s)|([0-9]+d )?([0-9]+h )?[0-9]+min|([0-9]+d )?[0-9]+h|[0-9]+d) at \([0-9]{4}-[0-9]{2}-[0-9]{2} [0-9]{2}:[0-9]{2}:[0-9]{2}\)$'
readonly REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly FORMAL_DIR="${REPO_ROOT}/formal/sumeragi_v2"
readonly MODEL="SumeragiV2ServeSchedulerOrdinalMutation.tla"
readonly SHARED_FIXED_CONFIG="serve_scheduler_shared_ordinal_fixed.cfg"
readonly SEPARATE_BUG_CONFIG="serve_scheduler_separate_ordinal_bug.cfg"
readonly OLDER_FIXED_CONFIG="serve_scheduler_older_runtime_fixed.cfg"
readonly TARGET_FIRST_BUG_CONFIG="serve_scheduler_always_target_first_bug.cfg"
readonly OLDER_LOCAL_FIXED_CONFIG="serve_scheduler_older_local_fixed.cfg"
readonly LOCAL_TARGET_FIRST_BUG_CONFIG="serve_scheduler_local_target_first_bug.cfg"
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

run_dir="$(mktemp -d "${TMPDIR:-/tmp}/sumeragi-serve-scheduler-ordinal.XXXXXX")"
trap 'rm -rf -- "$run_dir"' EXIT

(
  cd "$FORMAL_DIR"
  "$JAVA_BIN" -cp "$TLA2TOOLS_JAR" tla2sany.SANY "$MODEL"
) >"${run_dir}/sany.log" 2>&1
readonly SANY_SUCCESS_MARKER="Semantic processing of module ${MODEL%.tla}"
sany_last_nonblank="$(awk 'NF { line = $0 } END { print line }' "${run_dir}/sany.log")"
[[ "$sany_last_nonblank" == "$SANY_SUCCESS_MARKER" ]] || {
  echo "Serve scheduler-ordinal model did not reach the exact SANY marker" >&2
  cat "${run_dir}/sany.log" >&2
  exit 1
}
[[ "$(grep -Fxc "$SANY_SUCCESS_MARKER" "${run_dir}/sany.log" || true)" == 1 ]] || {
  echo "Serve scheduler-ordinal model emitted an ambiguous SANY marker" >&2
  cat "${run_dir}/sany.log" >&2
  exit 1
}

common=(
  "$JAVA_BIN" -XX:+UseParallelGC -cp "$TLA2TOOLS_JAR" tlc2.TLC
  -cleanup -workers 1 -fp 87 -seed 490671825330118247
)

assert_nonzero_state_space() {
  local label="$1"
  local log="$2"
  local state_line
  local generated
  local distinct
  state_line="$(grep -E '^[0-9][0-9,]* states generated, [0-9][0-9,]* distinct states found' "$log" | tail -n 1 || true)"
  [[ -n "$state_line" ]] || {
    echo "${label} emitted no TLC state-count summary" >&2
    cat "$log" >&2
    exit 1
  }
  generated="$(awk '{print $1}' <<<"$state_line" | tr -d ',')"
  distinct="$(awk '{print $4}' <<<"$state_line" | tr -d ',')"
  if ((generated <= 0 || distinct <= 0)); then
    echo "${label} explored a zero-state model: ${state_line}" >&2
    cat "$log" >&2
    exit 1
  fi
}

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
  assert_nonzero_state_space "$label" "$log"
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

shared_fixed_log="$(run_tlc shared-ordinal-fixed "$SHARED_FIXED_CONFIG" 0)"
older_fixed_log="$(run_tlc older-runtime-fixed "$OLDER_FIXED_CONFIG" 0)"
older_local_fixed_log="$(run_tlc older-local-fixed "$OLDER_LOCAL_FIXED_CONFIG" 0)"
for log in "$shared_fixed_log" "$older_fixed_log" "$older_local_fixed_log"; do
  [[ "$(grep -Fxc "Model checking completed. No error has been found." "$log" || true)" == 1 ]] || {
    echo "repaired Serve scheduler-ordinal case missed exact completion" >&2
    cat "$log" >&2
    exit 1
  }
  [[ "$(grep -Ec '^(Error:|Deadlock reached[.])' "$log" || true)" == 0 ]] || {
    echo "repaired Serve scheduler-ordinal case emitted a failure diagnostic" >&2
    cat "$log" >&2
    exit 1
  }
done

separate_bug_log="$(run_tlc separate-ordinal "$SEPARATE_BUG_CONFIG" 13)"
target_first_bug_log="$(run_tlc always-target-first "$TARGET_FIRST_BUG_CONFIG" 13)"
local_target_first_bug_log="$(run_tlc local-target-first "$LOCAL_TARGET_FIRST_BUG_CONFIG" 13)"
for log in "$separate_bug_log" "$target_first_bug_log" "$local_target_first_bug_log"; do
  for marker in "Temporal properties were violated." "Stuttering"; do
    grep -Fq "$marker" "$log" || {
      echo "Serve scheduler mutant missed TLC marker: ${marker}" >&2
      cat "$log" >&2
      exit 1
    }
  done
  if grep -Fq "Error: Invariant " "$log"; then
    echo "Serve scheduler lasso was misclassified as an invariant failure" >&2
    cat "$log" >&2
    exit 1
  fi
done

echo "[tlc] repaired shared Serve scheduler ordinal and older Runtime/Local interleave models passed"
echo "[tlc] separate-ordinal and Runtime/Local target-first mutants produced exact liveness status 13"

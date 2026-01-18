#!/usr/bin/env bash
set -euo pipefail
set -o errtrace

# Regenerate Android Norito fixtures via the shared Rust exporter. This keeps the
# canonical `.norito` payloads, manifest, and hash metadata aligned with the Rust
# data model.

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
EXPORTER_MANIFEST="${ANDROID_FIXTURE_EXPORTER_MANIFEST:-${REPO_ROOT}/scripts/export_norito_fixtures/Cargo.toml}"
RESOURCES_DIR="${ANDROID_FIXTURE_OUT:-${REPO_ROOT}/java/iroha_android/src/test/resources}"
FIXTURES_JSON="${ANDROID_FIXTURE_SOURCE:-${RESOURCES_DIR}/transaction_payloads.json}"
MANIFEST_NAME="${ANDROID_FIXTURE_MANIFEST_NAME:-transaction_fixtures.manifest.json}"
STATE_FILE="${ANDROID_FIXTURE_STATE_FILE:-${REPO_ROOT}/artifacts/android_fixture_regen_state.json}"
ROTATION_OWNER="${ANDROID_FIXTURE_ROTATION_OWNER:-unassigned}"
CADENCE_LABEL="${ANDROID_FIXTURE_CADENCE:-twice-weekly-tue-fri-0900utc}"
CARGO_BIN="${CARGO_BIN:-cargo}"
RELEASE_FLAG="${ANDROID_FIXTURE_RELEASE:-1}"
CHECK_ENCODE="${ANDROID_FIXTURE_CHECK_ENCODE:-1}"
LOG_DIR="${ANDROID_FIXTURE_LOG_DIR:-${REPO_ROOT}/artifacts/android/fixture_runs}"
CAPTURE_LOG="${ANDROID_FIXTURE_CAPTURE_LOG:-1}"
CAPTURE_SUMMARY="${ANDROID_FIXTURE_CAPTURE_SUMMARY:-1}"
RUN_LABEL_DEFAULT="$(date -u +"%Y-%m-%dT%H%M%SZ")"
RUN_LABEL="${ANDROID_FIXTURE_RUN_LABEL:-${RUN_LABEL_DEFAULT}}"
RUN_RESULT_LABEL_OVERRIDE="${ANDROID_FIXTURE_RUN_RESULT_LABEL:-}"
RUN_NOTE="${ANDROID_FIXTURE_RUN_NOTE:-}"
RUN_STARTED_AT="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
RUN_LOG_PATH="${LOG_DIR}/${RUN_LABEL}-run.log"
SUMMARY_WRITTEN=0
LAST_ERROR_MESSAGE=""
FIXTURES_DIGEST=""
STATE_DIGEST=""
ALIGNMENT_ENABLED="${ANDROID_FIXTURE_ALIGNMENT:-1}"
ALIGNMENT_ALLOW_DRIFT="${ANDROID_FIXTURE_ALIGNMENT_ALLOW_DRIFT:-0}"
ALIGNMENT_SCRIPT="${ANDROID_FIXTURE_ALIGNMENT_SCRIPT:-${REPO_ROOT}/scripts/norito_fixture_alignment.py}"
ALIGNMENT_MD_OUT="${ANDROID_FIXTURE_ALIGNMENT_MD_OUT:-${REPO_ROOT}/docs/source/sdk/android/norito_fixture_alignment.md}"
ALIGNMENT_JSON_OUT="${ANDROID_FIXTURE_ALIGNMENT_JSON_OUT:-${REPO_ROOT}/docs/source/sdk/android/norito_fixture_alignment.json}"
JDK_POLICY_ENABLED="${ANDROID_FIXTURE_JDK_POLICY:-1}"
JDK_POLICY_SCRIPT="${ANDROID_FIXTURE_JDK_POLICY_SCRIPT:-${REPO_ROOT}/scripts/check_jdk_policy.py}"
JDK_POLICY_EXPECTED_MAJOR="${ANDROID_FIXTURE_JDK_POLICY_EXPECTED_MAJOR:-21}"
JDK_POLICY_ALLOW_NEWER="${ANDROID_FIXTURE_JDK_POLICY_ALLOW_NEWER:-0}"
JDK_POLICY_JSON_OUT="${ANDROID_FIXTURE_JDK_POLICY_JSON_OUT:-${LOG_DIR}/${RUN_LABEL}-jdk.json}"
JDK_POLICY_MD_OUT="${ANDROID_FIXTURE_JDK_POLICY_MD_OUT:-${LOG_DIR}/${RUN_LABEL}-jdk.md}"

hash_file() {
  local target="$1"
  if [[ ! -f "${target}" ]]; then
    return 1
  fi
  local digest_cmd=""
  if command -v sha256sum >/dev/null 2>&1; then
    digest_cmd="sha256sum"
  elif command -v shasum >/dev/null 2>&1; then
    digest_cmd="shasum -a 256"
  else
    return 1
  fi
  # shellcheck disable=SC2086
  ${digest_cmd} "${target}" | awk '{print $1}'
}

summary_path_for() {
  local outcome="$1"
  local suffix="$outcome"
  if [[ "${outcome}" == "success" && -n "${RUN_RESULT_LABEL_OVERRIDE}" ]]; then
    suffix="${RUN_RESULT_LABEL_OVERRIDE}"
  fi
  # macOS ships an older Bash without `${var,,}` support; use tr instead.
  suffix="$(printf '%s' "${suffix}" | tr '[:upper:]' '[:lower:]')"
  printf "%s/%s-%s.md" "${LOG_DIR}" "${RUN_LABEL}" "${suffix}"
}

write_summary() {
  local outcome="$1"
  local reason="$2"
  [[ "${CAPTURE_SUMMARY}" == "1" ]] || return 0
  [[ "${SUMMARY_WRITTEN}" -eq 0 ]] || return 0
  SUMMARY_WRITTEN=1
  mkdir -p "${LOG_DIR}"

  local end_ts
  end_ts="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
  local summary_path
  summary_path="$(summary_path_for "${outcome}")"
  local log_reference="not captured (set ANDROID_FIXTURE_CAPTURE_LOG=1)"
  if [[ "${CAPTURE_LOG}" == "1" ]]; then
    log_reference="\`${RUN_LOG_PATH}\`"
  fi

  local fixtures_hash_text="n/a"
  if [[ -z "${FIXTURES_DIGEST}" ]]; then
    local digest
    if digest="$(hash_file "${FIXTURES_JSON}")"; then
      FIXTURES_DIGEST="${digest}"
    fi
  fi
  if [[ -n "${FIXTURES_DIGEST}" ]]; then
    fixtures_hash_text="${FIXTURES_DIGEST}"
  fi

  local state_hash_text="n/a"
  if [[ -z "${STATE_DIGEST}" ]]; then
    local digest
    if digest="$(hash_file "${STATE_FILE}")"; then
      STATE_DIGEST="${digest}"
    fi
  fi
  if [[ -n "${STATE_DIGEST}" ]]; then
    state_hash_text="${STATE_DIGEST}"
  fi

  local result_line
  if [[ "${outcome}" == "success" ]]; then
    result_line="SUCCESS — cadence metadata updated"
  else
    result_line="FAILED — ${reason:-see log for details}"
  fi

  cat > "${summary_path}" <<EOF
# Android Norito Fixture Rotation — ${RUN_LABEL}

- Result: ${result_line}
- Window owner: ${ROTATION_OWNER}
- Cadence label: ${CADENCE_LABEL}
- Started at: ${RUN_STARTED_AT}
- Completed at: ${end_ts}
- Command: \`${COMMAND_STR:-N/A}\`
- Exporter manifest: \`${EXPORTER_MANIFEST}\`
- Fixtures JSON: \`${FIXTURES_JSON}\` (SHA-256: ${fixtures_hash_text})
- Resources directory: \`${RESOURCES_DIR}\`
- Cadence state: \`${STATE_FILE}\` (SHA-256: ${state_hash_text})
- Log: ${log_reference}
- Alignment report: \`${ALIGNMENT_MD_OUT}\` (JSON: \`${ALIGNMENT_JSON_OUT}\`, enabled=${ALIGNMENT_ENABLED}, allow_drift=${ALIGNMENT_ALLOW_DRIFT})
- JDK policy report: \`${JDK_POLICY_MD_OUT}\` (JSON: \`${JDK_POLICY_JSON_OUT}\`, enabled=${JDK_POLICY_ENABLED}, expected_major=${JDK_POLICY_EXPECTED_MAJOR}, allow_newer=${JDK_POLICY_ALLOW_NEWER})
- Notes: ${RUN_NOTE:-"(none)"}
EOF

  if [[ "${outcome}" != "success" && -n "${reason}" ]]; then
    printf '\nFailure reason: %s\n' "${reason}" >> "${summary_path}"
  fi

  echo "[android-fixtures] wrote summary to ${summary_path}"
}

capture_error() {
  local exit_code="$?"
  local lineno="unknown"
  if [[ ${#BASH_LINENO[@]} -gt 0 ]]; then
    lineno="${BASH_LINENO[0]}"
  fi
  LAST_ERROR_MESSAGE="command \"${BASH_COMMAND}\" failed at line ${lineno} (exit ${exit_code})"
}

on_exit() {
  local exit_code="$1"
  if [[ "${exit_code}" -eq 0 ]]; then
    write_summary "success" ""
  else
    write_summary "failure" "${LAST_ERROR_MESSAGE}"
  fi
}

if [[ "${CAPTURE_LOG}" == "1" || "${CAPTURE_SUMMARY}" == "1" ]]; then
  mkdir -p "${LOG_DIR}"
fi

trap 'capture_error' ERR
trap 'on_exit $?' EXIT

if [[ ! -f "${FIXTURES_JSON}" ]]; then
  echo "[android-fixtures] fixtures JSON not found: ${FIXTURES_JSON}" >&2
  exit 1
fi

if [[ ! -f "${EXPORTER_MANIFEST}" ]]; then
  echo "[android-fixtures] exporter manifest not found: ${EXPORTER_MANIFEST}" >&2
  exit 1
fi

mkdir -p "${RESOURCES_DIR}"

COMMAND=("${CARGO_BIN}" "run" "--manifest-path" "${EXPORTER_MANIFEST}")
if [[ "${RELEASE_FLAG}" == "1" ]]; then
  COMMAND+=("--release")
fi
COMMAND+=("--" "--fixtures" "${FIXTURES_JSON}" "--write-fixtures" "--out-dir" "${RESOURCES_DIR}" "--manifest" "${MANIFEST_NAME}")
if [[ "${CHECK_ENCODE}" == "0" ]]; then
  COMMAND+=("--check-encoded" "false")
fi
printf -v COMMAND_STR '%q ' "${COMMAND[@]}"
COMMAND_STR="${COMMAND_STR%" "}"

echo "[android-fixtures] regenerating Norito fixtures via ${EXPORTER_MANIFEST}"
if [[ "${CAPTURE_LOG}" == "1" ]]; then
  (
    cd "${REPO_ROOT}"
    "${COMMAND[@]}"
  ) 2>&1 | tee "${RUN_LOG_PATH}"
else
  (
    cd "${REPO_ROOT}"
    "${COMMAND[@]}"
  )
fi
echo "[android-fixtures] exporter completed"

mkdir -p "$(dirname "${STATE_FILE}")"
timestamp="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
cat > "${STATE_FILE}" <<EOF
{
  "generated_at": "${timestamp}",
  "rotation_owner": "${ROTATION_OWNER}",
  "cadence": "${CADENCE_LABEL}",
  "fixtures": "${FIXTURES_JSON}",
  "resources": "${RESOURCES_DIR}"
}
EOF
echo "[android-fixtures] wrote cadence state to ${STATE_FILE}"
echo "[android-fixtures] rotation owner: ${ROTATION_OWNER} | cadence: ${CADENCE_LABEL} | generated_at: ${timestamp}"
if digest="$(hash_file "${FIXTURES_JSON}")"; then
  FIXTURES_DIGEST="${digest}"
fi
if digest="$(hash_file "${STATE_FILE}")"; then
  STATE_DIGEST="${digest}"
fi
if [[ "${ALIGNMENT_ENABLED}" == "1" ]]; then
  echo "[android-fixtures] running norito fixture alignment check"
  alignment_args=("--markdown-out" "${ALIGNMENT_MD_OUT}" "--json-out" "${ALIGNMENT_JSON_OUT}")
  if [[ "${ALIGNMENT_ALLOW_DRIFT}" == "1" ]]; then
    alignment_args+=("--allow-drift")
  fi
  if ! (cd "${REPO_ROOT}" && python3 "${ALIGNMENT_SCRIPT}" "${alignment_args[@]}"); then
    echo "[android-fixtures] alignment check failed (see logs above)" >&2
    exit 1
  fi
fi
if [[ "${JDK_POLICY_ENABLED}" == "1" ]]; then
  echo "[android-fixtures] verifying JDK policy (${JDK_POLICY_EXPECTED_MAJOR}, allow_newer=${JDK_POLICY_ALLOW_NEWER})"
  jdk_args=(
    "python3" "${JDK_POLICY_SCRIPT}"
    "--expected-major" "${JDK_POLICY_EXPECTED_MAJOR}"
    "--json-out" "${JDK_POLICY_JSON_OUT}"
    "--markdown-out" "${JDK_POLICY_MD_OUT}"
  )
  if [[ "${JDK_POLICY_ALLOW_NEWER}" == "1" ]]; then
    jdk_args+=("--allow-newer")
  fi
  if ! (cd "${REPO_ROOT}" && "${jdk_args[@]}"); then
    echo "[android-fixtures] JDK policy check failed (see ${JDK_POLICY_MD_OUT})" >&2
    exit 1
  fi
fi
echo "[android-fixtures] done (remember to run make android-fixtures-check)"

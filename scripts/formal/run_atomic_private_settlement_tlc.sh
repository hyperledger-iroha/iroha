#!/usr/bin/env bash
set -euo pipefail
umask 077

readonly TLA2TOOLS_VERSION="1.7.4"
readonly TLC_VERSION="2.19"
readonly TLA2TOOLS_SHA256="936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88"
readonly DEFAULT_SEED="20260829"
readonly DEFAULT_FINGERPRINT_INDEX="0"
readonly DEFAULT_TRANSCRIPT_ARTIFACT_PATH="evidence/logs/formal_model_report.log"
REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly REPO_ROOT
readonly FORMAL_DIR="${REPO_ROOT}/formal/private_settlement"
readonly MODEL="${FORMAL_DIR}/AtomicPrivateSettlementV1.tla"
readonly INDEXED_MODEL="${FORMAL_DIR}/AtomicPrivateSettlementV1CommitteeFaults.tla"
readonly REPORT_BUILDER="${REPO_ROOT}/scripts/formal/private_settlement_tlc_report.py"
source "${REPO_ROOT}/scripts/formal/sumeragi_v2_tlc_result_contract.sh"

readonly POSITIVE_CONFIGS=(
  AtomicPrivateSettlementV1_3.cfg
  AtomicPrivateSettlementV1_255.cfg
  AtomicPrivateSettlementV1_expiry.cfg
)
readonly INDEXED_POSITIVE_CONFIGS=(
  AtomicPrivateSettlementV1CommitteeFaults_2_validator_focused.cfg
  AtomicPrivateSettlementV1CommitteeFaults_2.cfg
  AtomicPrivateSettlementV1CommitteeFaults_3.cfg
  AtomicPrivateSettlementV1CommitteeFaults_4_clean.cfg
  AtomicPrivateSettlementV1CommitteeFaults_expiry.cfg
)
readonly NEGATIVE_CONFIGS=(
  AtomicPrivateSettlementV1_partial_apply_bug.cfg
  AtomicPrivateSettlementV1_commit_before_prepare_bug.cfg
  AtomicPrivateSettlementV1_drop_stage_on_crash_bug.cfg
)
readonly ALL_CONFIGS=(
  "${POSITIVE_CONFIGS[@]}"
  "${INDEXED_POSITIVE_CONFIGS[@]}"
  "${NEGATIVE_CONFIGS[@]}"
)

usage() {
  cat <<'USAGE'
usage: run_atomic_private_settlement_tlc.sh [options]

Options:
  --config NAME                 Run one allowlisted configuration; repeatable.
  --output-dir DIR              Retain logs and, for the complete matrix, the report.
  --workers COUNT|auto          TLC worker count (default: auto). Complete
                                release evidence requires an explicit count.
  --seed UINT64                 Deterministic TLC seed (default: 20260829).
  --fingerprint-index UINT      TLC fingerprint index (default: 0).
  --transcript-artifact-path P  Relative path used by the release manifest.
  --list-configs                Print the ordered release matrix and exit.
  -h, --help                    Show this help.

TLA2TOOLS_JAR must identify the authenticated TLA+ tools 1.7.4 JAR. JAVA_BIN
may identify a Java runtime; otherwise the repository resolver selects one.
USAGE
}

contains() {
  local needle="$1"
  shift
  local candidate
  for candidate in "$@"; do
    [[ "$candidate" == "$needle" ]] && return 0
  done
  return 1
}

hash_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

configuration_model() {
  local config="$1"
  if contains "$config" "${INDEXED_POSITIVE_CONFIGS[@]}"; then
    printf '%s\n' "$INDEXED_MODEL"
  else
    printf '%s\n' "$MODEL"
  fi
}

configuration_outcome() {
  local config="$1"
  if contains "$config" "${NEGATIVE_CONFIGS[@]}"; then
    printf '%s\n' safety_violation
  else
    printf '%s\n' pass
  fi
}

selected_configs=()
output_dir=""
workers="auto"
seed="$DEFAULT_SEED"
fingerprint_index="$DEFAULT_FINGERPRINT_INDEX"
transcript_artifact_path="$DEFAULT_TRANSCRIPT_ARTIFACT_PATH"
list_configs=false

while (($#)); do
  case "$1" in
    --config)
      (($# >= 2)) || { echo "--config requires a value" >&2; exit 2; }
      selected_configs+=("$2")
      shift 2
      ;;
    --output-dir)
      (($# >= 2)) || { echo "--output-dir requires a value" >&2; exit 2; }
      output_dir="$2"
      shift 2
      ;;
    --workers)
      (($# >= 2)) || { echo "--workers requires a value" >&2; exit 2; }
      workers="$2"
      shift 2
      ;;
    --seed)
      (($# >= 2)) || { echo "--seed requires a value" >&2; exit 2; }
      seed="$2"
      shift 2
      ;;
    --fingerprint-index)
      (($# >= 2)) || { echo "--fingerprint-index requires a value" >&2; exit 2; }
      fingerprint_index="$2"
      shift 2
      ;;
    --transcript-artifact-path)
      (($# >= 2)) || {
        echo "--transcript-artifact-path requires a value" >&2
        exit 2
      }
      transcript_artifact_path="$2"
      shift 2
      ;;
    --list-configs)
      list_configs=true
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if [[ "$list_configs" == true ]]; then
  for config in "${ALL_CONFIGS[@]}"; do
    printf '%s\t%s\n' "$config" "$(configuration_outcome "$config")"
  done
  exit 0
fi

[[ "$workers" == auto || "$workers" =~ ^[1-9][0-9]*$ ]] || {
  echo "--workers must be auto or a positive integer" >&2
  exit 2
}
[[ "$seed" =~ ^[0-9]+$ ]] || {
  echo "--seed must be an unsigned integer" >&2
  exit 2
}
[[ "$fingerprint_index" =~ ^[0-9]+$ ]] || {
  echo "--fingerprint-index must be an unsigned integer" >&2
  exit 2
}
((fingerprint_index <= 63)) || {
  echo "--fingerprint-index must be between 0 and 63" >&2
  exit 2
}
[[ "$transcript_artifact_path" != /* \
  && "$transcript_artifact_path" != ../* \
  && "$transcript_artifact_path" != */../* \
  && "$transcript_artifact_path" != */.. ]] || {
  echo "--transcript-artifact-path must be safe and relative" >&2
  exit 2
}

if ((${#selected_configs[@]} == 0)); then
  selected_configs=("${ALL_CONFIGS[@]}")
fi
for config in "${selected_configs[@]}"; do
  contains "$config" "${ALL_CONFIGS[@]}" || {
    echo "configuration is not in the release matrix: $config" >&2
    exit 2
  }
done
for ((left = 0; left < ${#selected_configs[@]}; left++)); do
  for ((right = left + 1; right < ${#selected_configs[@]}; right++)); do
    [[ "${selected_configs[left]}" != "${selected_configs[right]}" ]] || {
      echo "configuration was selected more than once: ${selected_configs[left]}" >&2
      exit 2
    }
  done
done

complete_matrix=true
if ((${#selected_configs[@]} != ${#ALL_CONFIGS[@]})); then
  complete_matrix=false
else
  for index in "${!ALL_CONFIGS[@]}"; do
    if [[ "${selected_configs[index]}" != "${ALL_CONFIGS[index]}" ]]; then
      complete_matrix=false
      break
    fi
  done
fi
if [[ "$complete_matrix" == true && "$workers" == auto ]]; then
  echo "the complete TLC release matrix requires an explicit --workers count" >&2
  exit 2
fi
candidate_commit=""
if [[ "$complete_matrix" == true ]]; then
  if [[ -n "$(git -C "$REPO_ROOT" status --porcelain=v1 --untracked-files=normal)" ]]; then
    echo "the complete TLC release report requires a clean settled candidate" >&2
    exit 1
  fi
  candidate_commit="$(git -C "$REPO_ROOT" rev-parse HEAD)"
fi
readonly candidate_commit

assert_candidate_unchanged() {
  local observed_commit
  observed_commit="$(git -C "$REPO_ROOT" rev-parse HEAD)"
  if [[ "$observed_commit" != "$candidate_commit" ]]; then
    echo "the checkout HEAD changed during the complete TLC release run" >&2
    exit 1
  fi
  if [[ -n "$(git -C "$REPO_ROOT" status --porcelain=v1 --untracked-files=normal)" ]]; then
    echo "the checkout changed during the complete TLC release run" >&2
    exit 1
  fi
}

readonly TLA2TOOLS_JAR="${TLA2TOOLS_JAR:?TLA2TOOLS_JAR must name the authenticated external tool}"
[[ -f "$TLA2TOOLS_JAR" && ! -L "$TLA2TOOLS_JAR" ]] || {
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
  resolved_java="$(bash "${REPO_ROOT}/scripts/formal/resolve_java.sh" "$JAVA_BIN")"
else
  resolved_java="$(bash "${REPO_ROOT}/scripts/formal/resolve_java.sh")"
fi
readonly JAVA_BIN="$resolved_java"
"$JAVA_BIN" -version >/dev/null 2>&1 || {
  echo "a working Java runtime is required for TLC" >&2
  exit 1
}
for required in "$MODEL" "$INDEXED_MODEL" "$REPORT_BUILDER"; do
  [[ -f "$required" && ! -L "$required" ]] || {
    echo "atomic private-settlement formal input is missing: $required" >&2
    exit 1
  }
done
for config in "${selected_configs[@]}"; do
  required="$FORMAL_DIR/$config"
  [[ -f "$required" && ! -L "$required" ]] || {
    echo "atomic private-settlement configuration is missing: $required" >&2
    exit 1
  }
done

cleanup_run_dir=false
if [[ -n "$output_dir" ]]; then
  [[ ! -e "$output_dir" ]] || {
    echo "refusing to replace existing TLC output directory: $output_dir" >&2
    exit 1
  }
  mkdir -p -- "$output_dir"
  run_dir="$(cd -- "$output_dir" && pwd -P)"
else
  run_dir="$(mktemp -d "${TMPDIR:-/tmp}/atomic-private-settlement-tlc.XXXXXX")"
  cleanup_run_dir=true
fi
cleanup() {
  if [[ "$cleanup_run_dir" == true ]]; then
    rm -rf -- "$run_dir"
  fi
}
trap cleanup EXIT
mkdir -p -- "$run_dir/inputs" "$run_dir/logs" "$run_dir/metadir" "$run_dir/sany"
readonly frozen_report_builder="$run_dir/inputs/private_settlement_tlc_report.py"
install -m 600 -- "$REPORT_BUILDER" "$frozen_report_builder"
for model in "$MODEL" "$INDEXED_MODEL"; do
  install -m 600 -- "$model" "$run_dir/inputs/$(basename -- "$model")"
done
for config in "${selected_configs[@]}"; do
  install -m 600 -- "$FORMAL_DIR/$config" "$run_dir/inputs/$config"
done

readonly TLC=(
  "$JAVA_BIN" -XX:+UseParallelGC -cp "$TLA2TOOLS_JAR" tlc2.TLC
  -cleanup -workers "$workers" -fp "$fingerprint_index" -seed "$seed"
)
readonly SANY=("$JAVA_BIN" -cp "$TLA2TOOLS_JAR" tla2sany.SANY)

for model_name in "$(basename -- "$MODEL")" "$(basename -- "$INDEXED_MODEL")"; do
  model="$run_dir/inputs/$model_name"
  stdout_log="$run_dir/sany/${model_name}.stdout.log"
  stderr_log="$run_dir/sany/${model_name}.stderr.log"
  echo "[atomic-private-settlement-tlc] SANY $model_name"
  set +e
  "${SANY[@]}" "$model" >"$stdout_log" 2>"$stderr_log"
  status=$?
  set -e
  printf '%s\n' "$status" >"$run_dir/sany/${model_name}.status"
  if [[ "$status" -ne 0 || -s "$stderr_log" ]]; then
    cat "$stdout_log"
    cat "$stderr_log" >&2
    echo "SANY rejected ${model_name} or emitted separate stderr" >&2
    exit 1
  fi
done

for config in "${selected_configs[@]}"; do
  model="$run_dir/inputs/$(basename -- "$(configuration_model "$config")")"
  expected="$(configuration_outcome "$config")"
  stdout_log="$run_dir/logs/${config}.stdout.log"
  stderr_log="$run_dir/logs/${config}.stderr.log"
  status_file="$run_dir/logs/${config}.status"
  metadir="$run_dir/metadir/${config%.cfg}"
  echo "[atomic-private-settlement-tlc] ${expected} ${config}"
  set +e
  "${TLC[@]}" -metadir "$metadir" -config "$run_dir/inputs/$config" "$model" \
    >"$stdout_log" 2>"$stderr_log"
  status=$?
  set -e
  printf '%s\n' "$status" >"$status_file"
  if [[ -s "$stderr_log" ]]; then
    cat "$stdout_log"
    cat "$stderr_log" >&2
    echo "${config}: TLC emitted separate stderr" >&2
    exit 1
  fi
  if [[ "$expected" == pass ]]; then
    sumeragi_v2_tlc_assert_fixed_success "$config" "$stdout_log" "$status"
  else
    primary_diagnostic_count="$(
      grep -Ec "$SUMERAGI_V2_TLC_PRIMARY_DIAGNOSTIC_PATTERN" "$stdout_log" || true
    )"
    if [[ "$status" -ne 12 ]]; then
      cat "$stdout_log" >&2
      echo "${config}: expected TLC invariant status 12, got ${status}" >&2
      exit 1
    fi
    sumeragi_v2_tlc_assert_nonzero_state_space "$config" "$stdout_log"
    sumeragi_v2_tlc_assert_exact_line \
      "$config" "$stdout_log" "Error: Invariant Safety is violated."
    if [[ "$primary_diagnostic_count" != 1 ]]; then
      cat "$stdout_log" >&2
      echo "${config}: expected exactly one primary TLC diagnostic, got ${primary_diagnostic_count}" >&2
      exit 1
    fi
    sumeragi_v2_tlc_assert_terminal "$config" "$stdout_log"
  fi
done

if [[ "$complete_matrix" == true ]]; then
  commit="$candidate_commit"
  assert_candidate_unchanged
  formal_inputs=(
    "$(basename -- "$MODEL")"
    "$(basename -- "$INDEXED_MODEL")"
    "${ALL_CONFIGS[@]}"
  )
  for input_name in "${formal_inputs[@]}"; do
    source_path="formal/private_settlement/$input_name"
    expected_object="$(git -C "$REPO_ROOT" rev-parse "${commit}:${source_path}")"
    observed_object="$(git -C "$REPO_ROOT" hash-object "$run_dir/inputs/$input_name")"
    if [[ "$observed_object" != "$expected_object" ]]; then
      echo "frozen formal input does not match ${commit}: ${source_path}" >&2
      exit 1
    fi
  done
  expected_object="$(
    git -C "$REPO_ROOT" rev-parse \
      "${commit}:scripts/formal/private_settlement_tlc_report.py"
  )"
  observed_object="$(git -C "$REPO_ROOT" hash-object "$frozen_report_builder")"
  if [[ "$observed_object" != "$expected_object" ]]; then
    echo "frozen report builder does not match ${commit}" >&2
    exit 1
  fi
  python3 "$frozen_report_builder" \
    --formal-dir "$run_dir/inputs" \
    --logs-dir "$run_dir/logs" \
    --sany-dir "$run_dir/sany" \
    --commit "$commit" \
    --tool-version "TLC ${TLC_VERSION} / TLA+ tools ${TLA2TOOLS_VERSION}" \
    --tool-sha256 "$actual_sha256" \
    --seed "$seed" \
    --fingerprint-index "$fingerprint_index" \
    --workers "$workers" \
    --transcript-artifact-path "$transcript_artifact_path" \
    --report-output "$run_dir/formal_model_report.json" \
    --transcript-output "$run_dir/formal_model_report.log"
  assert_candidate_unchanged
  echo "[atomic-private-settlement-tlc] complete release report validated"
else
  echo "[atomic-private-settlement-tlc] selected matrix passed; no complete release report emitted"
fi

if [[ -n "$output_dir" ]]; then
  echo "[atomic-private-settlement-tlc] retained evidence at $run_dir"
fi

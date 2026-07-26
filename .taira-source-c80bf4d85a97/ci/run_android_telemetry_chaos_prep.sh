#!/usr/bin/env bash
# Run the Android telemetry chaos prep workflow (load generator + status snapshot + queue exports).

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"

output_dir="${ANDROID_TELEMETRY_CHAOS_DIR:-artifacts/android/telemetry/chaos}"
cluster="${ANDROID_TELEMETRY_CLUSTER:-android-telemetry-stg}"
endpoint="${ANDROID_TELEMETRY_ENDPOINT:-}"
duration="${ANDROID_TELEMETRY_DURATION:-30s}"
rps="${ANDROID_TELEMETRY_RPS:-5}"
dry_run="${ANDROID_TELEMETRY_DRY_RUN:-true}"
log_path="${output_dir}/load-generator.log"
status_source="${ANDROID_TELEMETRY_STATUS_JSON:-artifacts/android/telemetry/status.json}"
queue_exports="${ANDROID_PENDING_QUEUE_EXPORTS:-}"
inspector_enabled="${ANDROID_PENDING_QUEUE_INSPECTOR:-true}"
inspector_classpath="${ANDROID_PENDING_QUEUE_INSPECTOR_CLASSPATH:-java/iroha_android/build/classes}"
inspector_main_class="org.hyperledger.iroha.android.tools.PendingQueueInspector"
inspector_java="${ANDROID_PENDING_QUEUE_JAVA:-java}"
inspector_ready=0
expected_salt_epoch="${ANDROID_TELEMETRY_EXPECTED_SALT_EPOCH:-}"
expected_salt_rotation="${ANDROID_TELEMETRY_EXPECTED_SALT_ROTATION:-}"

mkdir -p "${output_dir}"
echo "[android-chaos] storing artefacts under ${output_dir}"

load_cmd=(scripts/telemetry/generate_android_load.sh "--duration" "${duration}" "--rps" "${rps}" "--log-file" "${log_path}")
if [[ -n "${endpoint}" ]]; then
  load_cmd+=("--endpoint" "${endpoint}")
else
  load_cmd+=("--cluster" "${cluster}")
fi
if [[ "${dry_run}" == "true" ]]; then
  load_cmd+=("--dry-run")
fi

echo "[android-chaos] running load generator: ${load_cmd[*]}"
"${load_cmd[@]}"

status_txt="${output_dir}/status.txt"
status_json="${output_dir}/status.json"
echo "[android-chaos] capturing status snapshot from ${status_source}"
salt_args=()
if [[ -n "${expected_salt_epoch}" ]]; then
  salt_args+=("--expected-salt-epoch" "${expected_salt_epoch}")
fi
if [[ -n "${expected_salt_rotation}" ]]; then
  salt_args+=("--expected-salt-rotation" "${expected_salt_rotation}")
fi
scripts/telemetry/check_redaction_status.py \
  --status-json "${status_source}" \
  --warn-backlog 1 \
  --max-failures 5 \
  "${salt_args[@]}" \
  --json-out "${status_json}" | tee "${status_txt}"

compute_sha256() {
  local file="$1"
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "${file}" | awk '{print $1}'
  elif command -v sha256sum >/dev/null 2>&1; then
    sha256sum "${file}" | awk '{print $1}'
  elif command -v openssl >/dev/null 2>&1; then
    openssl dgst -sha256 "${file}" | awk '{print $NF}'
  else
    python3 - "$file" <<'PY'
import hashlib
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
data = path.read_bytes()
print(hashlib.sha256(data).hexdigest())
PY
  fi
}

ensure_inspector_ready() {
  if [[ "${inspector_enabled}" != "true" ]]; then
    return 1
  fi
  if [[ "${inspector_ready}" -eq 1 ]]; then
    return 0
  fi
  if [[ -f "${inspector_classpath}/org/hyperledger/iroha/android/tools/PendingQueueInspector.class" ]]; then
    inspector_ready=1
    return 0
  fi
  echo "[android-chaos] compiling PendingQueueInspector classes via java/iroha_android/run_tests.sh (may take a minute)"
  (cd java/iroha_android && ./run_tests.sh >/dev/null)
  if [[ -f "${inspector_classpath}/org/hyperledger/iroha/android/tools/PendingQueueInspector.class" ]]; then
    inspector_ready=1
    return 0
  fi
  echo "[android-chaos] warning: PendingQueueInspector classes not found under ${inspector_classpath}" >&2
  inspector_enabled="false"
  return 1
}

emit_queue_metadata() {
  local file="$1"
  local label="$2"
  local sha
  sha="$(compute_sha256 "${file}")"
  local sha_file="${file}.sha256"
  printf "%s  %s\n" "${sha}" "$(basename "${file}")" > "${sha_file}"
  echo "[android-chaos] wrote checksum ${sha_file}"
  if [[ "${inspector_enabled}" != "true" ]]; then
    echo "[android-chaos] inspector disabled; skipping JSON summary for ${label}"
    return
  fi
  if ! ensure_inspector_ready; then
    echo "[android-chaos] skipping PendingQueueInspector for ${label}" >&2
    return
  fi
  local json_file="${file}.json"
  "${inspector_java}" -cp "${inspector_classpath}" "${inspector_main_class}" \
    --file "${file}" \
    --json > "${json_file}"
  echo "[android-chaos] wrote inspector summary ${json_file}"
}

queue_dir="${output_dir}/queues"
mkdir -p "${queue_dir}"
if [[ -n "${queue_exports}" ]]; then
  IFS=',' read -ra entries <<< "${queue_exports}"
  for entry in "${entries[@]}"; do
    label="${entry%%=*}"
    src="${entry#*=}"
    if [[ -z "${label}" || -z "${src}" ]]; then
      echo "[android-chaos] skipping malformed queue entry '${entry}'" >&2
      continue
    fi
    dest="${queue_dir}/${label}"
    echo "[android-chaos] copying queue dump ${src} -> ${dest}"
    cp "${src}" "${dest}"
    emit_queue_metadata "${dest}" "${label}"
  done
else
  readme="${queue_dir}/README.md"
  cat > "${readme}" <<'EOF'
No queue exports were provided. Populate this directory by setting
ANDROID_PENDING_QUEUE_EXPORTS to a comma-separated list of label=/path/to/pending.queue
entries so PendingQueueInspector outputs (JSON + SHA256) can be archived automatically.
EOF
fi

echo "[android-chaos] chaos prep complete."

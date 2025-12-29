#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEFAULT_CONFIG="${ROOT_DIR}/examples/android/configs/SampleAccounts.json"
WORKDIR="${ROOT_DIR}/artifacts/android/sample_env"
PROFILE="operator"
TORII_TELEMETRY_PROFILE="android-sample"
ENABLE_TORII=true
ENABLE_SORAFS=true
ENABLE_TELEMETRY=true
ENABLE_HANDOFF=false
HANDOFF_PORT=10801
TELEMETRY_DURATION="5m"
TELEMETRY_RPS="4"
TELEMETRY_CLUSTER=""
TELEMETRY_ENDPOINT=""
TELEMETRY_PATH="/api/android/telemetry/mock_ingest"
declare -a TELEMETRY_HEADERS=()
TELEMETRY_DRY_RUN=true

TORII_PID=""
HANDOFF_PID=""
TORII_URL=""
TORII_METRICS=""
TORII_ACCOUNTS_PATH=""
TORII_LOG_PATH=""
CONFIG_PATH="${DEFAULT_CONFIG}"
CONFIG_ABS="${DEFAULT_CONFIG}"
ENV_PATH=""
SORAFS_SCOREBOARD=""
SORAFS_SUMMARY=""
SORAFS_SCOREBOARD_SHA256=""
SORAFS_SUMMARY_SHA256=""
SORAFS_RECEIPTS=""
TELEMETRY_LOG=""
HANDOFF_URL=""
HANDOFF_INBOX=""

usage() {
  cat <<'USAGE'
Usage: scripts/android_sample_env.sh [options]

Options:
  --config PATH                Sample accounts JSON (default: examples/android/configs/SampleAccounts.json)
  --workdir PATH               Workspace for generated artefacts (default: artifacts/android/sample_env)
  --profile NAME               Profile label written to the .env file (default: operator)
  --wallet                     Shortcut for --profile retail-wallet
  --torii / --no-torii         Enable or disable the local Torii sandbox (default: enabled)
  --sorafs / --no-sorafs       Enable or disable the SoraFS fixture fetch (default: enabled)
  --telemetry / --no-telemetry Generate or skip telemetry seeds (default: enabled)
  --telemetry-duration DUR     Telemetry load duration (e.g. 3m, 30s). Default: 5m
  --telemetry-rps RATE         Telemetry load rate (events/sec). Default: 4
  --telemetry-cluster HOST     Target cluster hostname (https://HOST...). Defaults to dry-run mode.
  --telemetry-endpoint URL     Override full telemetry endpoint URL.
  --telemetry-path PATH        Override telemetry path when using --telemetry-cluster (default: /api/android/telemetry/mock_ingest)
  --telemetry-header KEY=VAL   Append header to telemetry requests (can be supplied multiple times).
  --telemetry-live             Send telemetry to the configured endpoint instead of dry-run logging.
  --torii-telemetry-profile ID Override the telemetry profile passed to the iroha_test_network example.
  --handoff                    Start the mock hand-off receiver (used by the retail wallet sample).
  --handoff-port PORT          Port for the hand-off receiver (default: 10801).
  --help                       Show this message.
USAGE
}

abs_path() {
  python3 - "$1" <<'PY'
import sys
from pathlib import Path
print(Path(sys.argv[1]).expanduser().resolve())
PY
}

sha256_file() {
  local target="$1"
  if [[ ! -f "${target}" ]]; then
    echo ""
    return
  fi
  python3 - "$target" <<'PY'
import hashlib
import sys
from pathlib import Path

path = Path(sys.argv[1])
digest = hashlib.sha256()
with path.open("rb") as handle:
    for chunk in iter(lambda: handle.read(1024 * 1024), b""):
        if not chunk:
            break
        digest.update(chunk)
print(digest.hexdigest())
PY
}

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "[android-sample-env] missing required command: $1" >&2
    exit 1
  fi
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --config)
      CONFIG_PATH="$2"
      shift 2
      ;;
    --workdir)
      WORKDIR="$2"
      shift 2
      ;;
    --profile)
      PROFILE="$2"
      shift 2
      ;;
    --wallet)
      PROFILE="retail-wallet"
      shift
      ;;
    --torii)
      ENABLE_TORII=true
      shift
      ;;
    --no-torii)
      ENABLE_TORII=false
      shift
      ;;
    --sorafs)
      ENABLE_SORAFS=true
      shift
      ;;
    --no-sorafs)
      ENABLE_SORAFS=false
      shift
      ;;
    --telemetry)
      ENABLE_TELEMETRY=true
      shift
      ;;
    --no-telemetry)
      ENABLE_TELEMETRY=false
      shift
      ;;
    --telemetry-duration)
      TELEMETRY_DURATION="$2"
      shift 2
      ;;
    --telemetry-rps)
      TELEMETRY_RPS="$2"
      shift 2
      ;;
    --telemetry-cluster)
      TELEMETRY_CLUSTER="$2"
      shift 2
      ;;
    --telemetry-endpoint)
      TELEMETRY_ENDPOINT="$2"
      shift 2
      ;;
    --telemetry-path)
      TELEMETRY_PATH="$2"
      shift 2
      ;;
    --telemetry-header)
      TELEMETRY_HEADERS+=("$2")
      shift 2
      ;;
    --telemetry-live)
      TELEMETRY_DRY_RUN=false
      shift
      ;;
    --torii-telemetry-profile)
      TORII_TELEMETRY_PROFILE="$2"
      shift 2
      ;;
    --handoff)
      ENABLE_HANDOFF=true
      shift
      ;;
    --handoff-port)
      HANDOFF_PORT="$2"
      shift 2
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      echo "[android-sample-env] unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

require_cmd python3
require_cmd cargo

CONFIG_ABS="$(abs_path "${CONFIG_PATH}")"
if [[ ! -f "${CONFIG_ABS}" ]]; then
  echo "[android-sample-env] config not found: ${CONFIG_ABS}" >&2
  exit 1
fi

WORKDIR="$(abs_path "${WORKDIR}")"
mkdir -p "${WORKDIR}"
ENV_PATH="${WORKDIR}/${PROFILE}.env"

handle_cleanup() {
  if [[ -n "${TORII_PID}" ]]; then
    if kill -0 "${TORII_PID}" >/dev/null 2>&1; then
      kill "${TORII_PID}" >/dev/null 2>&1 || true
      wait "${TORII_PID}" >/dev/null 2>&1 || true
    fi
    TORII_PID=""
  fi
  if [[ -n "${HANDOFF_PID}" ]]; then
    if kill -0 "${HANDOFF_PID}" >/dev/null 2>&1; then
      kill "${HANDOFF_PID}" >/dev/null 2>&1 || true
      wait "${HANDOFF_PID}" >/dev/null 2>&1 || true
    fi
    HANDOFF_PID=""
  fi
}

handle_signal() {
  echo "[android-sample-env] received signal, stopping background services..."
  exit 130
}

trap handle_cleanup EXIT
trap handle_signal INT TERM

start_torii_demo() {
  local state_dir="${WORKDIR}/torii"
  mkdir -p "${state_dir}"
  local state_file="${state_dir}/state.json"
  local runner_log="${state_dir}/runner.log"
  TORII_LOG_PATH="${state_dir}/torii.log"
  local metrics_file="${state_dir}/metrics.prom"
  : > "${runner_log}"
  rm -f "${state_file}" "${TORII_LOG_PATH}" "${metrics_file}"

  echo "[android-sample-env] starting iroha_test_network (Torii sandbox)..."
  (
    cd "${ROOT_DIR}"
    cargo run -p iroha_test_network --example ios_demo -- \
      --config "${CONFIG_ABS}" \
      --state "${state_file}" \
      --telemetry-profile "${TORII_TELEMETRY_PROFILE}"
  ) > "${runner_log}" 2>&1 &
  TORII_PID=$!

  for _ in {1..90}; do
    if [[ -s "${state_file}" ]]; then
      break
    fi
    if ! kill -0 "${TORII_PID}" >/dev/null 2>&1; then
      echo "[android-sample-env] Torii sandbox exited early. Check ${runner_log}" >&2
      exit 1
    fi
    sleep 1
  done
  if [[ ! -s "${state_file}" ]]; then
    echo "[android-sample-env] timed out waiting for ${state_file}" >&2
    exit 1
  fi

  read TORII_URL TORII_METRICS STDOUT_LOG <<<"$(python3 - "${state_file}" <<'PY'
import json, sys
with open(sys.argv[1], 'r', encoding='utf-8') as fh:
    data = json.load(fh)
print(data.get('torii_url', ''))
print(data.get('metrics_url', ''))
print(data.get('stdout_log') or '')
PY
)"

  if [[ -n "${STDOUT_LOG}" && -f "${STDOUT_LOG}" ]]; then
    cp "${STDOUT_LOG}" "${TORII_LOG_PATH}"
  else
    cp "${runner_log}" "${TORII_LOG_PATH}"
  fi

  if [[ -n "${TORII_METRICS}" ]]; then
    if command -v curl >/dev/null 2>&1; then
      for _ in {1..10}; do
        if curl -sf "${TORII_METRICS}" -o "${metrics_file}"; then
          break
        fi
        sleep 1
      done
    fi
    if [[ -s "${metrics_file}" ]]; then
      echo "[android-sample-env] metrics snapshot written to ${metrics_file}"
    fi
  fi

  TORII_ACCOUNTS_PATH="${state_dir}/accounts.json"
  python3 - "${state_file}" "${TORII_ACCOUNTS_PATH}" <<'PY'
import json, sys
from pathlib import Path
state_path, out_path = sys.argv[1:3]
with open(state_path, 'r', encoding='utf-8') as fh:
    state = json.load(fh)
accounts = []
for entry in state.get('accounts', []):
    record = {
        'account_id': entry.get('account_id'),
        'public_key': entry.get('public_key')
    }
    if entry.get('private_key'):
        record['private_key'] = entry['private_key']
    record['asset_id'] = entry.get('asset_id')
    record['initial_balance'] = entry.get('initial_balance')
    accounts.append(record)
Path(out_path).write_text(json.dumps({'accounts': accounts}, indent=2) + '\n', encoding='utf-8')
PY

  echo "[android-sample-env] Torii sandbox running at ${TORII_URL}"
}

run_sorafs_fixture() {
  echo "[android-sample-env] executing SoraFS orchestrator fixture..."
  (
    cd "${ROOT_DIR}"
    bash ci/check_sorafs_orchestrator_adoption.sh
  )
  local latest_link="${ROOT_DIR}/artifacts/sorafs_orchestrator/latest"
  if [[ ! -d "${latest_link}" ]]; then
    echo "[android-sample-env] expected ${latest_link} to exist after fixture run" >&2
    exit 1
  fi
  local target_dir
  target_dir="$(abs_path "${latest_link}")"
  local dest="${WORKDIR}/sorafs"
  mkdir -p "${dest}"
  for name in scoreboard.json summary.json provider_metrics.json chunk_receipts.json; do
    if [[ -f "${target_dir}/${name}" ]]; then
      cp "${target_dir}/${name}" "${dest}/${name}"
    fi
  done
  SORAFS_SCOREBOARD="${dest}/scoreboard.json"
  [[ -f "${dest}/summary.json" ]] && SORAFS_SUMMARY="${dest}/summary.json"
  [[ -f "${dest}/chunk_receipts.json" ]] && SORAFS_RECEIPTS="${dest}/chunk_receipts.json"
  if [[ -f "${SORAFS_SCOREBOARD}" ]]; then
    SORAFS_SCOREBOARD_SHA256="$(sha256_file "${SORAFS_SCOREBOARD}")"
  else
    SORAFS_SCOREBOARD_SHA256=""
  fi
  if [[ -n "${SORAFS_SUMMARY}" ]]; then
    SORAFS_SUMMARY_SHA256="$(sha256_file "${SORAFS_SUMMARY}")"
  else
    SORAFS_SUMMARY_SHA256=""
  fi
  echo "[android-sample-env] SoraFS artefacts copied to ${dest}"
}

run_telemetry_seed() {
  echo "[android-sample-env] generating telemetry seed traffic..."
  local dest_dir="${WORKDIR}/telemetry"
  mkdir -p "${dest_dir}"
  TELEMETRY_LOG="${dest_dir}/load-generator.log"
  local cmd=(python3 "${ROOT_DIR}/scripts/telemetry/generate_android_load.py"
             --duration "${TELEMETRY_DURATION}"
             --rps "${TELEMETRY_RPS}"
             --log-file "${TELEMETRY_LOG}"
             --path "${TELEMETRY_PATH}"
             --seed 424242)
  if [[ -n "${TELEMETRY_ENDPOINT}" ]]; then
    cmd+=(--endpoint "${TELEMETRY_ENDPOINT}")
  elif [[ -n "${TELEMETRY_CLUSTER}" ]]; then
    cmd+=(--cluster "${TELEMETRY_CLUSTER}")
  fi
  for header in "${TELEMETRY_HEADERS[@]}"; do
    cmd+=(--header "${header}")
  done
  if "${TELEMETRY_DRY_RUN}"; then
    cmd+=(--dry-run)
  fi
  (
    cd "${ROOT_DIR}"
    "${cmd[@]}"
  )
  echo "[android-sample-env] telemetry log written to ${TELEMETRY_LOG}"
}

start_handoff_server() {
  local inbox="${WORKDIR}/handoff"
  mkdir -p "${inbox}"
  HANDOFF_INBOX="${inbox}"
  HANDOFF_URL="http://127.0.0.1:${HANDOFF_PORT}/handoff"
  echo "[android-sample-env] starting mock hand-off receiver on ${HANDOFF_URL}"
  python3 - "${inbox}" "${HANDOFF_PORT}" <<'PY' &
import http.server
import json
import sys
import threading
from datetime import datetime, timezone
from pathlib import Path

inbox = Path(sys.argv[1])
port = int(sys.argv[2])

class Handler(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        if self.path != "/handoff":
            self.send_response(404)
            self.end_headers()
            return
        length = int(self.headers.get("Content-Length", "0"))
        body = self.rfile.read(length)
        timestamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
        dest = inbox / f"handoff-{timestamp}.json"
        try:
            json.loads(body.decode("utf-8"))
            dest.write_bytes(body)
            self.send_response(202)
            self.end_headers()
        except json.JSONDecodeError:
            self.send_response(400)
            self.end_headers()

    def log_message(self, fmt, *args):
        sys.stdout.write("[handoff] " + fmt % args + "\n")

server = http.server.HTTPServer(("127.0.0.1", port), Handler)
threading.Thread(target=server.serve_forever, daemon=True).start()
try:
    while True:
        threading.Event().wait(86400)
except KeyboardInterrupt:
    pass
PY
  HANDOFF_PID=$!
}

write_env_file() {
  cat > "${ENV_PATH}" <<EOF
ANDROID_SAMPLE_PROFILE=${PROFILE}
ANDROID_SAMPLE_CONFIG=${CONFIG_ABS}
ANDROID_SAMPLE_TORII_URL=${TORII_URL}
ANDROID_SAMPLE_TORII_ACCOUNTS=${TORII_ACCOUNTS_PATH}
ANDROID_SAMPLE_TORII_LOG=${TORII_LOG_PATH}
ANDROID_SAMPLE_TORII_METRICS=${TORII_METRICS}
ANDROID_SAMPLE_SORAFS_SCOREBOARD=${SORAFS_SCOREBOARD}
ANDROID_SAMPLE_SORAFS_SUMMARY=${SORAFS_SUMMARY}
ANDROID_SAMPLE_SORAFS_SCOREBOARD_SHA256=${SORAFS_SCOREBOARD_SHA256}
ANDROID_SAMPLE_SORAFS_SUMMARY_SHA256=${SORAFS_SUMMARY_SHA256}
ANDROID_SAMPLE_SORAFS_RECEIPTS=${SORAFS_RECEIPTS}
ANDROID_SAMPLE_TELEMETRY_LOG=${TELEMETRY_LOG}
ANDROID_SAMPLE_HANDOFF_ENDPOINT=${HANDOFF_URL}
ANDROID_SAMPLE_HANDOFF_INBOX=${HANDOFF_INBOX}
EOF
  echo "[android-sample-env] wrote environment file ${ENV_PATH}"
}

${ENABLE_SORAFS} && run_sorafs_fixture
${ENABLE_TELEMETRY} && run_telemetry_seed
${ENABLE_TORII} && start_torii_demo
${ENABLE_HANDOFF} && start_handoff_server

write_env_file

echo
echo "[android-sample-env] Environment summary:"
echo "  Profile:              ${PROFILE}"
[[ -n "${TORII_URL}" ]] && echo "  Torii endpoint:       ${TORII_URL}"
if [[ -n "${SORAFS_SCOREBOARD}" ]]; then
  echo "  SoraFS scoreboard:    ${SORAFS_SCOREBOARD}"
  [[ -n "${SORAFS_SCOREBOARD_SHA256}" ]] && echo "    SHA-256:            ${SORAFS_SCOREBOARD_SHA256}"
fi
if [[ -n "${SORAFS_SUMMARY}" ]]; then
  echo "  SoraFS summary:       ${SORAFS_SUMMARY}"
  [[ -n "${SORAFS_SUMMARY_SHA256}" ]] && echo "    SHA-256:            ${SORAFS_SUMMARY_SHA256}"
fi
[[ -n "${TELEMETRY_LOG}" ]] && echo "  Telemetry log:        ${TELEMETRY_LOG}"
[[ -n "${HANDOFF_URL}" ]] && echo "  Handoff endpoint:     ${HANDOFF_URL}"
echo "  Exported vars:        ${ENV_PATH}"
echo

if [[ -n "${TORII_PID}" || -n "${HANDOFF_PID}" ]]; then
  echo "[android-sample-env] Background services running. Press Ctrl+C to stop."
  while true; do
    sleep 60
  done
fi

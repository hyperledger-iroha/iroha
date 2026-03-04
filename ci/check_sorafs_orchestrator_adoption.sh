#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FIXTURE_DIR="${REPO_ROOT}/fixtures/sorafs_orchestrator/multi_peer_parity_v1"
ARTIFACT_ROOT="${REPO_ROOT}/artifacts/sorafs_orchestrator"
TIMESTAMP="$(date -u +"%Y%m%dT%H%M%SZ")"
RUN_DIR="${ARTIFACT_ROOT}/${TIMESTAMP}"
CONFIG_PATH="${RUN_DIR}/fixture_config.sh"
BURN_IN_LABEL="${SORAFS_BURN_IN_LABEL:-}"
BURN_IN_REGION="${SORAFS_BURN_IN_REGION:-}"
BURN_IN_MANIFEST="${SORAFS_BURN_IN_MANIFEST:-}"
BURN_IN_DAY="${SORAFS_BURN_IN_DAY:-}"
BURN_IN_WINDOW_DAYS="${SORAFS_BURN_IN_WINDOW_DAYS:-30}"
BURN_IN_NOTES="${SORAFS_BURN_IN_NOTES:-}"
BURN_IN_LOGS_RAW="${SORAFS_BURN_IN_LOGS:-}"
BURN_IN_MIN_PQ_RATIO="${SORAFS_BURN_IN_MIN_PQ_RATIO:-0.95}"
BURN_IN_MAX_BROWNOUT_RATIO="${SORAFS_BURN_IN_MAX_BROWNOUT_RATIO:-0.01}"
BURN_IN_MAX_NO_PROVIDER_ERRORS="${SORAFS_BURN_IN_MAX_NO_PROVIDER_ERRORS:-0}"
BURN_IN_MIN_FETCHES="${SORAFS_BURN_IN_MIN_FETCHES:-150}"
BURN_IN_LOGS=()
BURN_IN_LOG_LIST=""

require_nonempty_file() {
  local path="$1"
  local label="$2"
  if [[ ! -s "${path}" ]]; then
    echo "[sorafs-adoption] ${label} missing or empty: ${path}" >&2
    exit 1
  fi
}

if [[ -n "${BURN_IN_LABEL}" ]]; then
  if [[ -z "${BURN_IN_REGION}" ]]; then
    echo "[sorafs-adoption] SORAFS_BURN_IN_REGION must be set when SORAFS_BURN_IN_LABEL is provided" >&2
    exit 1
  fi
  if [[ -z "${BURN_IN_MANIFEST}" ]]; then
    echo "[sorafs-adoption] SORAFS_BURN_IN_MANIFEST must be set when SORAFS_BURN_IN_LABEL is provided" >&2
    exit 1
  fi
  if [[ -z "${BURN_IN_DAY}" ]]; then
    echo "[sorafs-adoption] SORAFS_BURN_IN_DAY must be set when SORAFS_BURN_IN_LABEL is provided" >&2
    exit 1
  fi
  if ! [[ "${BURN_IN_WINDOW_DAYS}" =~ ^[0-9]+$ ]]; then
    echo "[sorafs-adoption] SORAFS_BURN_IN_WINDOW_DAYS must be a positive integer" >&2
    exit 1
  fi
  if ! [[ "${BURN_IN_DAY}" =~ ^[0-9]+$ ]]; then
    echo "[sorafs-adoption] SORAFS_BURN_IN_DAY must be a positive integer" >&2
    exit 1
  fi
  BURN_IN_WINDOW_DAYS=$((10#${BURN_IN_WINDOW_DAYS}))
  BURN_IN_DAY=$((10#${BURN_IN_DAY}))
  if (( BURN_IN_WINDOW_DAYS <= 0 )); then
    echo "[sorafs-adoption] SORAFS_BURN_IN_WINDOW_DAYS must be greater than zero" >&2
    exit 1
  fi
  if (( BURN_IN_DAY < 1 )); then
    echo "[sorafs-adoption] SORAFS_BURN_IN_DAY must be at least 1" >&2
    exit 1
  fi
  if (( BURN_IN_DAY > BURN_IN_WINDOW_DAYS )); then
    echo "[sorafs-adoption] SORAFS_BURN_IN_DAY must not exceed SORAFS_BURN_IN_WINDOW_DAYS" >&2
    exit 1
  fi
  if ! [[ "${BURN_IN_MAX_NO_PROVIDER_ERRORS}" =~ ^[0-9]+$ ]]; then
    echo "[sorafs-adoption] SORAFS_BURN_IN_MAX_NO_PROVIDER_ERRORS must be a non-negative integer" >&2
    exit 1
  fi
  if ! [[ "${BURN_IN_MIN_FETCHES}" =~ ^[0-9]+$ ]]; then
    echo "[sorafs-adoption] SORAFS_BURN_IN_MIN_FETCHES must be a positive integer" >&2
    exit 1
  fi
  if [[ -z "${BURN_IN_LOGS_RAW}" ]]; then
    echo "[sorafs-adoption] SORAFS_BURN_IN_LOGS must list burn-in telemetry logs when SORAFS_BURN_IN_LABEL is provided" >&2
    exit 1
  fi
  BURN_IN_MAX_NO_PROVIDER_ERRORS=$((10#${BURN_IN_MAX_NO_PROVIDER_ERRORS}))
  BURN_IN_MIN_FETCHES=$((10#${BURN_IN_MIN_FETCHES}))
  if (( BURN_IN_MIN_FETCHES <= 0 )); then
    echo "[sorafs-adoption] SORAFS_BURN_IN_MIN_FETCHES must be greater than zero" >&2
    exit 1
  fi
  # Parse SORAFS_BURN_IN_LOGS with shell-style quoting support.
  while IFS= read -r log_path; do
    [[ -z "${log_path}" ]] && continue
    BURN_IN_LOGS+=("${log_path}")
  done < <(
    SORAFS_BURN_IN_LOGS_INPUT="${BURN_IN_LOGS_RAW}" python3 <<'PY'
import os
import shlex
import sys

raw = os.environ.get("SORAFS_BURN_IN_LOGS_INPUT", "")
try:
    paths = shlex.split(raw)
except ValueError as exc:  # unmatched quotes, etc.
    sys.exit(f"[sorafs-adoption] failed to parse SORAFS_BURN_IN_LOGS: {exc}")
for path in paths:
    print(path)
PY
  )
  if (( ${#BURN_IN_LOGS[@]} == 0 )); then
    echo "[sorafs-adoption] SORAFS_BURN_IN_LOGS must include at least one telemetry file" >&2
    exit 1
  fi
  for log_path in "${BURN_IN_LOGS[@]}"; do
    require_nonempty_file "${log_path}" "burn-in log"
  done
  BURN_IN_LOG_LIST="$(printf "%s\n" "${BURN_IN_LOGS[@]}")"
fi

mkdir -p "${RUN_DIR}"
ln -sfn "${RUN_DIR}" "${ARTIFACT_ROOT}/latest"

if [[ ! -d "${FIXTURE_DIR}" ]]; then
  echo "[sorafs-adoption] missing fixture directory: ${FIXTURE_DIR}" >&2
  exit 1
fi

SORAFS_FIXTURE_DIR="${FIXTURE_DIR}" \
SORAFS_REPO_ROOT="${REPO_ROOT}" \
SORAFS_CONFIG_PATH="${CONFIG_PATH}" \
python3 <<'PY'
import json
import os
import pathlib
import shlex

fixture_dir = pathlib.Path(os.environ["SORAFS_FIXTURE_DIR"])
repo_root = pathlib.Path(os.environ["SORAFS_REPO_ROOT"])
config_path = pathlib.Path(os.environ["SORAFS_CONFIG_PATH"])

def read_json(path: pathlib.Path):
    with path.open("r", encoding="utf-8") as handle:
        return json.load(handle)

metadata = read_json(fixture_dir / "metadata.json")
options = read_json(fixture_dir / "options.json")
providers = read_json(fixture_dir / "providers.json")

plan_path = (fixture_dir / metadata["plan_file"]).resolve()
telemetry_path = (fixture_dir / metadata["telemetry_file"]).resolve()
payload_path = (repo_root / metadata["payload_path"]).resolve()
assume_now = metadata.get("now_unix_secs", 0) or 0

provider_concurrency = options.get("global_parallel_limit") or options.get("max_parallel") or 2
if provider_concurrency < 1:
    provider_concurrency = 1

provider_specs = [
    f"{entry['provider_id']}={payload_path}#{provider_concurrency}"
    for entry in providers
]

provider_count = len(provider_specs)
if provider_count == 0:
    raise SystemExit("fixture does not define any providers")

if provider_count < 2:
    min_providers = provider_count
else:
    min_providers = min(provider_count, 3)

def write_line(handle, key, value):
    handle.write(f"{key}={value}\n")

with config_path.open("w", encoding="utf-8") as handle:
    write_line(handle, "PLAN", shlex.quote(str(plan_path)))
    write_line(handle, "TELEMETRY", shlex.quote(str(telemetry_path)))
    write_line(handle, "ASSUME_NOW", str(assume_now))
    write_line(handle, "PAYLOAD", shlex.quote(str(payload_path)))
    if (max_peers := options.get("max_peers")):
        write_line(handle, "MAX_PEERS", str(int(max_peers)))
    if (max_parallel := options.get("max_parallel") or options.get("global_parallel_limit")):
        write_line(handle, "MAX_PARALLEL", str(int(max_parallel)))
    if (retry_budget := options.get("retry_budget") or options.get("per_chunk_retry_limit")):
        write_line(handle, "RETRY_BUDGET", str(int(retry_budget)))
    if (failure_threshold := options.get("provider_failure_threshold")):
        write_line(handle, "PROVIDER_FAILURE_THRESHOLD", str(int(failure_threshold)))
    write_line(handle, "MIN_PROVIDERS", str(int(min_providers)))
    quoted = " ".join(shlex.quote(spec) for spec in provider_specs)
    handle.write(f"PROVIDERS=({quoted})\n")
PY

# shellcheck disable=SC1090
source "${CONFIG_PATH}"

SCOREBOARD_PATH="${RUN_DIR}/scoreboard.json"
SUMMARY_PATH="${RUN_DIR}/summary.json"
PROVIDER_METRICS_PATH="${RUN_DIR}/provider_metrics.json"
CHUNK_RECEIPTS_PATH="${RUN_DIR}/chunk_receipts.json"
ADOPTION_REPORT_PATH="${RUN_DIR}/adoption_report.json"
BURN_IN_SUMMARY_PATH="${RUN_DIR}/burn_in_summary.json"
BURN_IN_NOTE_PATH="${RUN_DIR}/burn_in_note.json"

CLI_ARGS=(
  "--plan=${PLAN}"
  "--telemetry-json=${TELEMETRY}"
  "--scoreboard-out=${SCOREBOARD_PATH}"
  "--json-out=${SUMMARY_PATH}"
  "--provider-metrics-out=${PROVIDER_METRICS_PATH}"
  "--chunk-receipts-out=${CHUNK_RECEIPTS_PATH}"
  "--assume-now=${ASSUME_NOW}"
  "--use-scoreboard"
  "--allow-implicit-provider-metadata"
)
ALLOW_IMPLICIT_PROVIDER_METADATA=1

for spec in "${PROVIDERS[@]}"; do
  CLI_ARGS+=("--provider=${spec}")
done

if [[ -n "${MAX_PEERS:-}" ]]; then
  CLI_ARGS+=("--max-peers=${MAX_PEERS}")
fi
if [[ -n "${MAX_PARALLEL:-}" ]]; then
  CLI_ARGS+=("--max-parallel=${MAX_PARALLEL}")
fi
if [[ -n "${RETRY_BUDGET:-}" ]]; then
  CLI_ARGS+=("--retry-budget=${RETRY_BUDGET}")
fi
if [[ -n "${PROVIDER_FAILURE_THRESHOLD:-}" ]]; then
  CLI_ARGS+=("--provider-failure-threshold=${PROVIDER_FAILURE_THRESHOLD}")
fi

require_nonempty_file "${PLAN}" "plan fixture"
require_nonempty_file "${PAYLOAD}" "payload fixture"
require_nonempty_file "${TELEMETRY}" "telemetry fixture"

echo "[sorafs-adoption] running orchestrator fixture..."
cargo run -p sorafs_car --features cli --bin sorafs_fetch -- "${CLI_ARGS[@]}"

require_nonempty_file "${SCOREBOARD_PATH}" "scoreboard output"
require_nonempty_file "${SUMMARY_PATH}" "summary output"
require_nonempty_file "${PROVIDER_METRICS_PATH}" "provider metrics output"
require_nonempty_file "${CHUNK_RECEIPTS_PATH}" "chunk receipts output"

MIN_ELIGIBLE="${MIN_PROVIDERS:-2}"
echo "[sorafs-adoption] validating scoreboard (${MIN_ELIGIBLE} eligible providers required)..."
# shellcheck disable=SC2206
declare -a ADOPTION_FLAGS=()
if [[ -n "${XTASK_SORAFS_ADOPTION_FLAGS:-}" ]]; then
  while IFS= read -r flag; do
    if [[ -n "${flag}" ]]; then
      ADOPTION_FLAGS+=("${flag}")
    fi
  done < <(
    XTASK_SORAFS_ADOPTION_FLAGS_INPUT="${XTASK_SORAFS_ADOPTION_FLAGS}" python3 <<'PY'
import os
import shlex

flags = os.environ.get("XTASK_SORAFS_ADOPTION_FLAGS_INPUT", "")
try:
    parts = shlex.split(flags)
except ValueError as exc:  # unmatched quotes, etc.
    raise SystemExit(f"[sorafs-adoption] failed to parse XTASK_SORAFS_ADOPTION_FLAGS: {exc}") from exc
for part in parts:
    print(part)
PY
  )
fi
if [[ ${ALLOW_IMPLICIT_PROVIDER_METADATA:-0} -eq 1 ]]; then
  needs_flag=1
  if (( ${#ADOPTION_FLAGS[@]} )); then
    for flag in "${ADOPTION_FLAGS[@]}"; do
      if [[ "${flag}" == "--allow-implicit-metadata" ]]; then
        needs_flag=0
        break
      fi
    done
  fi
  if [[ ${needs_flag} -eq 1 ]]; then
    ADOPTION_FLAGS+=("--allow-implicit-metadata")
  fi
fi
if (( ${#ADOPTION_FLAGS[@]} )); then
  cargo xtask sorafs-adoption-check \
    --scoreboard "${SCOREBOARD_PATH}" \
    --summary "${SUMMARY_PATH}" \
    --min-providers "${MIN_ELIGIBLE}" \
    --require-metadata \
    --require-telemetry \
    --require-telemetry-region \
    --report "${ADOPTION_REPORT_PATH}" \
    "${ADOPTION_FLAGS[@]}"
else
  cargo xtask sorafs-adoption-check \
    --scoreboard "${SCOREBOARD_PATH}" \
    --summary "${SUMMARY_PATH}" \
    --min-providers "${MIN_ELIGIBLE}" \
    --require-metadata \
    --require-telemetry \
    --require-telemetry-region \
    --report "${ADOPTION_REPORT_PATH}"
fi

require_nonempty_file "${ADOPTION_REPORT_PATH}" "adoption report"

if [[ -n "${BURN_IN_LABEL}" ]]; then
  echo "[sorafs-adoption] running burn-in validator over ${#BURN_IN_LOGS[@]} telemetry file(s)..."
  BURN_IN_CMD=(
    cargo xtask sorafs-burn-in-check
    --window-days "${BURN_IN_WINDOW_DAYS}"
    --min-pq-ratio "${BURN_IN_MIN_PQ_RATIO}"
    --max-brownout-ratio "${BURN_IN_MAX_BROWNOUT_RATIO}"
    --max-no-provider-errors "${BURN_IN_MAX_NO_PROVIDER_ERRORS}"
    --min-fetches "${BURN_IN_MIN_FETCHES}"
    --out "${BURN_IN_SUMMARY_PATH}"
  )
  for log_path in "${BURN_IN_LOGS[@]}"; do
    BURN_IN_CMD+=("--log" "${log_path}")
  done
  "${BURN_IN_CMD[@]}"
  require_nonempty_file "${BURN_IN_SUMMARY_PATH}" "burn-in summary"
  echo "[sorafs-adoption] recording burn-in metadata label=${BURN_IN_LABEL}"
  SORAFS_BURN_IN_TIMESTAMP="${TIMESTAMP}" \
  SORAFS_BURN_IN_NOTE_PATH="${BURN_IN_NOTE_PATH}" \
  SORAFS_BURN_IN_LABEL_VAL="${BURN_IN_LABEL}" \
  SORAFS_BURN_IN_REGION_VAL="${BURN_IN_REGION}" \
  SORAFS_BURN_IN_MANIFEST_VAL="${BURN_IN_MANIFEST}" \
  SORAFS_BURN_IN_DAY_VAL="${BURN_IN_DAY}" \
  SORAFS_BURN_IN_WINDOW_VAL="${BURN_IN_WINDOW_DAYS}" \
  SORAFS_BURN_IN_NOTES_VAL="${BURN_IN_NOTES}" \
  SORAFS_BURN_IN_LOG_LIST="${BURN_IN_LOG_LIST}" \
  SORAFS_BURN_IN_SUMMARY_PATH="${BURN_IN_SUMMARY_PATH}" \
  SORAFS_BURN_IN_MIN_PQ_RATIO_VAL="${BURN_IN_MIN_PQ_RATIO}" \
  SORAFS_BURN_IN_MAX_BROWNOUT_RATIO_VAL="${BURN_IN_MAX_BROWNOUT_RATIO}" \
  SORAFS_BURN_IN_MAX_NO_PROVIDER_ERRORS_VAL="${BURN_IN_MAX_NO_PROVIDER_ERRORS}" \
  SORAFS_BURN_IN_MIN_FETCHES_VAL="${BURN_IN_MIN_FETCHES}" \
  SORAFS_SCOREBOARD_PATH="${SCOREBOARD_PATH}" \
  SORAFS_SUMMARY_PATH="${SUMMARY_PATH}" \
  SORAFS_PROVIDER_METRICS_PATH="${PROVIDER_METRICS_PATH}" \
  SORAFS_CHUNK_RECEIPTS_PATH="${CHUNK_RECEIPTS_PATH}" \
  SORAFS_TELEMETRY_PATH="${TELEMETRY}" \
  SORAFS_ADOPTION_REPORT_PATH="${ADOPTION_REPORT_PATH}" \
  python3 <<'PY'
import json
import os
from pathlib import Path

def parse_float(name: str, default=None):
    raw = os.environ.get(name)
    if raw is None:
        return default
    try:
        return float(raw)
    except ValueError:
        return raw

def parse_int(name: str, default=None):
    raw = os.environ.get(name)
    if raw is None:
        return default
    try:
        return int(raw)
    except (TypeError, ValueError):
        return raw

note_path = Path(os.environ["SORAFS_BURN_IN_NOTE_PATH"])
log_list_raw = os.environ.get("SORAFS_BURN_IN_LOG_LIST", "")
telemetry_logs = [
    line.strip() for line in log_list_raw.splitlines() if line.strip()
]
telemetry_fixture = os.environ.get("SORAFS_TELEMETRY_PATH")
telemetry_primary = telemetry_logs[0] if telemetry_logs else telemetry_fixture
payload = {
    "label": os.environ["SORAFS_BURN_IN_LABEL_VAL"],
    "timestamp": os.environ["SORAFS_BURN_IN_TIMESTAMP"],
    "region": os.environ.get("SORAFS_BURN_IN_REGION_VAL") or None,
    "manifest": os.environ.get("SORAFS_BURN_IN_MANIFEST_VAL") or None,
    "window_days": int(os.environ.get("SORAFS_BURN_IN_WINDOW_VAL") or 30),
    "day_index": None,
    "notes": os.environ.get("SORAFS_BURN_IN_NOTES_VAL") or None,
    "scoreboard": os.environ["SORAFS_SCOREBOARD_PATH"],
    "summary": os.environ["SORAFS_SUMMARY_PATH"],
    "provider_metrics": os.environ["SORAFS_PROVIDER_METRICS_PATH"],
    "chunk_receipts": os.environ["SORAFS_CHUNK_RECEIPTS_PATH"],
    "telemetry": telemetry_primary,
    "telemetry_logs": telemetry_logs or None,
    "telemetry_fixture": telemetry_fixture,
    "burn_in_summary": os.environ["SORAFS_BURN_IN_SUMMARY_PATH"],
    "burn_in_thresholds": {
        "window_days": int(os.environ.get("SORAFS_BURN_IN_WINDOW_VAL") or 30),
        "min_pq_ratio": parse_float("SORAFS_BURN_IN_MIN_PQ_RATIO_VAL"),
        "max_brownout_ratio": parse_float("SORAFS_BURN_IN_MAX_BROWNOUT_RATIO_VAL"),
        "max_no_provider_errors": parse_int("SORAFS_BURN_IN_MAX_NO_PROVIDER_ERRORS_VAL", 0),
        "min_fetches": parse_int("SORAFS_BURN_IN_MIN_FETCHES_VAL"),
    },
    "adoption_report": os.environ["SORAFS_ADOPTION_REPORT_PATH"],
}
day_raw = os.environ.get("SORAFS_BURN_IN_DAY_VAL")
if day_raw:
    try:
        payload["day_index"] = int(day_raw)
    except ValueError:
        payload["day_index"] = None

note_path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
PY
fi

echo "[sorafs-adoption] adoption artifacts written to ${RUN_DIR}"

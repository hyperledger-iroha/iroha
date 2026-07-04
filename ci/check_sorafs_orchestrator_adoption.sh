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
  SORAFS_ADOPTION_REQUIRED_PATH="${path}" \
  SORAFS_ADOPTION_REQUIRED_LABEL="${label}" \
  python3 <<'PY'
import os
import pathlib
import stat
import sys

path = pathlib.Path(os.environ["SORAFS_ADOPTION_REQUIRED_PATH"])
label = os.environ["SORAFS_ADOPTION_REQUIRED_LABEL"]

def fail(message: str) -> None:
    sys.exit(f"[sorafs-adoption] {message}: {path}")

def read_open_flags() -> int:
    return os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)

try:
    if path.is_symlink():
        fail(f"{label} must not be a symlink")
    for parent in (path.parent, *path.parent.parents):
        if parent.is_symlink():
            fail(f"{label} parent must not be a symlink")
        if parent.exists() and not parent.is_dir():
            fail(f"{label} parent must be a directory")
    path_stat = path.lstat()
    if not stat.S_ISREG(path_stat.st_mode):
        fail(f"{label} must be a regular file")
    if path_stat.st_size <= 0:
        fail(f"{label} missing or empty")
    fd = os.open(path, read_open_flags())
    try:
        if os.fstat(fd).st_size <= 0:
            fail(f"{label} missing or empty")
    finally:
        os.close(fd)
except FileNotFoundError:
    fail(f"{label} missing or empty")
except OSError as exc:
    sys.exit(f"[sorafs-adoption] failed to inspect {label}: {path}: {exc}")
PY
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
import stat

fixture_dir = pathlib.Path(os.environ["SORAFS_FIXTURE_DIR"])
repo_root = pathlib.Path(os.environ["SORAFS_REPO_ROOT"])
config_path = pathlib.Path(os.environ["SORAFS_CONFIG_PATH"])

def read_open_flags() -> int:
    return os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)

def write_open_flags() -> int:
    return (
        os.O_WRONLY
        | os.O_CREAT
        | os.O_TRUNC
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )

def write_all(fd: int, chunk: bytes) -> None:
    view = memoryview(chunk)
    while view:
        written = os.write(fd, view)
        if written <= 0:
            raise OSError("failed to write SoraFS adoption artifact")
        view = view[written:]

def sync_output_parent(path: pathlib.Path) -> None:
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_DIRECTORY", 0)
    nofollow = getattr(os, "O_NOFOLLOW", 0)
    if nofollow:
        flags |= nofollow
    fd = os.open(path.parent, flags)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)

def validate_adoption_path(path: pathlib.Path, label: str) -> None:
    if path.is_symlink():
        raise SystemExit(f"[sorafs-adoption] {label} must not be a symlink: {path}")
    for parent in (path.parent, *path.parent.parents):
        if parent.is_symlink():
            raise SystemExit(
                f"[sorafs-adoption] {label} parent must not be a symlink: {parent}"
            )
        if parent.exists() and not parent.is_dir():
            raise SystemExit(
                f"[sorafs-adoption] {label} parent must be a directory: {parent}"
            )

def ensure_adoption_directory(path: pathlib.Path, label: str) -> None:
    validate_adoption_path(path, label)
    path.mkdir(parents=True, exist_ok=True)
    validate_adoption_path(path, label)
    if not path.is_dir():
        raise SystemExit(f"[sorafs-adoption] {label} must be a directory: {path}")

def require_adoption_file(path: pathlib.Path, label: str) -> pathlib.Path:
    validate_adoption_path(path, label)
    try:
        path_stat = path.lstat()
    except FileNotFoundError as exc:
        raise SystemExit(f"[sorafs-adoption] {label} missing: {path}") from exc
    if not stat.S_ISREG(path_stat.st_mode):
        raise SystemExit(f"[sorafs-adoption] {label} must be a regular file: {path}")
    if path_stat.st_size <= 0:
        raise SystemExit(f"[sorafs-adoption] {label} missing or empty: {path}")
    return path

def read_json(path: pathlib.Path):
    require_adoption_file(path, "fixture JSON")
    fd = os.open(path, read_open_flags())
    try:
        with os.fdopen(fd, "r", encoding="utf-8") as handle:
            fd = -1
            return json.load(handle)
    finally:
        if fd >= 0:
            os.close(fd)

def write_adoption_text(path: pathlib.Path, body: str, label: str) -> None:
    validate_adoption_path(path, label)
    ensure_adoption_directory(path.parent, f"{label} parent directory")
    validate_adoption_path(path, label)
    fd = os.open(path, write_open_flags(), 0o666)
    try:
        write_all(fd, body.encode("utf-8"))
        os.fsync(fd)
    finally:
        if fd >= 0:
            os.close(fd)
    sync_output_parent(path)

metadata = read_json(fixture_dir / "metadata.json")
options = read_json(fixture_dir / "options.json")
providers = read_json(fixture_dir / "providers.json")

plan_path = require_adoption_file(fixture_dir / metadata["plan_file"], "plan file")
telemetry_path = require_adoption_file(
    fixture_dir / metadata["telemetry_file"],
    "telemetry file",
)
payload_path = require_adoption_file(repo_root / metadata["payload_path"], "payload file")
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

config_lines = [
    f"PLAN={shlex.quote(str(plan_path))}\n",
    f"TELEMETRY={shlex.quote(str(telemetry_path))}\n",
    f"ASSUME_NOW={assume_now}\n",
    f"PAYLOAD={shlex.quote(str(payload_path))}\n",
]
if (max_peers := options.get("max_peers")):
    config_lines.append(f"MAX_PEERS={int(max_peers)}\n")
if (max_parallel := options.get("max_parallel") or options.get("global_parallel_limit")):
    config_lines.append(f"MAX_PARALLEL={int(max_parallel)}\n")
if (retry_budget := options.get("retry_budget") or options.get("per_chunk_retry_limit")):
    config_lines.append(f"RETRY_BUDGET={int(retry_budget)}\n")
if (failure_threshold := options.get("provider_failure_threshold")):
    config_lines.append(f"PROVIDER_FAILURE_THRESHOLD={int(failure_threshold)}\n")
config_lines.append(f"MIN_PROVIDERS={int(min_providers)}\n")
quoted = " ".join(shlex.quote(spec) for spec in provider_specs)
config_lines.append(f"PROVIDERS=({quoted})\n")
write_adoption_text(config_path, "".join(config_lines), "fixture config")
PY

require_nonempty_file "${CONFIG_PATH}" "fixture config"

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

def write_open_flags() -> int:
    return (
        os.O_WRONLY
        | os.O_CREAT
        | os.O_TRUNC
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )

def write_all(fd: int, chunk: bytes) -> None:
    view = memoryview(chunk)
    while view:
        written = os.write(fd, view)
        if written <= 0:
            raise OSError("failed to write SoraFS adoption artifact")
        view = view[written:]

def sync_output_parent(path: Path) -> None:
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_DIRECTORY", 0)
    nofollow = getattr(os, "O_NOFOLLOW", 0)
    if nofollow:
        flags |= nofollow
    fd = os.open(path.parent, flags)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)

def validate_adoption_path(path: Path, label: str) -> None:
    if path.is_symlink():
        raise SystemExit(f"[sorafs-adoption] {label} must not be a symlink: {path}")
    for parent in (path.parent, *path.parent.parents):
        if parent.is_symlink():
            raise SystemExit(
                f"[sorafs-adoption] {label} parent must not be a symlink: {parent}"
            )
        if parent.exists() and not parent.is_dir():
            raise SystemExit(
                f"[sorafs-adoption] {label} parent must be a directory: {parent}"
            )

def ensure_adoption_directory(path: Path, label: str) -> None:
    validate_adoption_path(path, label)
    path.mkdir(parents=True, exist_ok=True)
    validate_adoption_path(path, label)
    if not path.is_dir():
        raise SystemExit(f"[sorafs-adoption] {label} must be a directory: {path}")

def write_adoption_json(path: Path, payload: dict, label: str) -> None:
    validate_adoption_path(path, label)
    ensure_adoption_directory(path.parent, f"{label} parent directory")
    validate_adoption_path(path, label)
    rendered = (json.dumps(payload, indent=2, allow_nan=False) + "\n").encode("utf-8")
    fd = os.open(path, write_open_flags(), 0o666)
    try:
        write_all(fd, rendered)
        os.fsync(fd)
    finally:
        if fd >= 0:
            os.close(fd)
    sync_output_parent(path)

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

write_adoption_json(note_path, payload, "burn-in note")
PY
fi

echo "[sorafs-adoption] adoption artifacts written to ${RUN_DIR}"

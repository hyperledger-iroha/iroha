#!/usr/bin/env bash
# Verify Norito-RPC fixtures across SDK manifests and log evidence artefacts.
#
# This wrapper executes `cargo xtask norito-rpc-verify`, captures the output log,
# and saves both the xtask verification report and a higher-level JSON summary under `artifacts/norito_rpc/`.
# Usage:
#   scripts/run_norito_rpc_fixtures.sh [--sdk <label>] [--artifacts-dir <path>] [--note <text>] [--allow-online]
#
# Required tools: bash, cargo, git, python3 (for JSON serialization).

set -euo pipefail

function usage() {
  cat <<'EOF'
Usage: scripts/run_norito_rpc_fixtures.sh [options]

Options:
  --sdk <label>             Logical SDK label for the evidence file (default: rust-cli).
  --artifacts-dir <dir>     Directory for logs/summaries (default: artifacts/norito_rpc).
  --note <text>             Optional free-form note stored in the summary JSON.
  --rotation <label>        Optional cadence label (e.g., week number) stored in the summary JSON.
  --manifest <path>         Fixture manifest path (default: fixtures/norito_rpc/transaction_fixtures.manifest.json).
  --schema-manifest <path>  Schema hash manifest path (default: fixtures/norito_rpc/schema_hashes.json).
  --allow-online            Allow Cargo to access the network (default: offline).
  --report-json <path>      Write aggregated cadence report JSON via norito_rpc_fixture_report.py.
  --report-markdown <path>  Write aggregated cadence report Markdown table.
  --report-max-age-days <n> Flag entries older than <n> days as stale in the report (optional).
  --auto-report             Emit rotation_status.{json,md} in artifacts/norito_rpc with a 7-day freshness gate.
  -h, --help                Show this help output.

The script runs `cargo xtask norito-rpc-verify`, writes the console log to
<artifacts-dir>/<timestamp>-<sdk>-norito-rpc.log, and emits a JSON summary with
metadata required by the NRPC-4 adoption plan.
EOF
}

SDK_LABEL="rust-cli"
ARTIFACTS_DIR="artifacts/norito_rpc"
NOTE=""
ROTATION_LABEL=""
MANIFEST_PATH="fixtures/norito_rpc/transaction_fixtures.manifest.json"
SCHEMA_MANIFEST_PATH="fixtures/norito_rpc/schema_hashes.json"
ALLOW_ONLINE=0
REPORT_JSON_PATH=""
REPORT_MD_PATH=""
REPORT_MAX_AGE_DAYS=""
AUTO_REPORT=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --sdk)
      [[ $# -lt 2 ]] && { echo "error: --sdk requires a label" >&2; usage; exit 1; }
      SDK_LABEL="$2"
      shift 2
      ;;
    --artifacts-dir)
      [[ $# -lt 2 ]] && { echo "error: --artifacts-dir requires a path" >&2; usage; exit 1; }
      ARTIFACTS_DIR="$2"
      shift 2
      ;;
    --note)
      [[ $# -lt 2 ]] && { echo "error: --note requires text" >&2; usage; exit 1; }
      NOTE="$2"
      shift 2
      ;;
    --rotation)
      [[ $# -lt 2 ]] && { echo "error: --rotation requires a label" >&2; usage; exit 1; }
      ROTATION_LABEL="$2"
      shift 2
      ;;
    --manifest)
      [[ $# -lt 2 ]] && { echo "error: --manifest requires a path" >&2; usage; exit 1; }
      MANIFEST_PATH="$2"
      shift 2
      ;;
    --schema-manifest)
      [[ $# -lt 2 ]] && { echo "error: --schema-manifest requires a path" >&2; usage; exit 1; }
      SCHEMA_MANIFEST_PATH="$2"
      shift 2
      ;;
    --allow-online)
      ALLOW_ONLINE=1
      shift
      ;;
    --report-json)
      [[ $# -lt 2 ]] && { echo "error: --report-json requires a path" >&2; usage; exit 1; }
      REPORT_JSON_PATH="$2"
      shift 2
      ;;
    --report-markdown)
      [[ $# -lt 2 ]] && { echo "error: --report-markdown requires a path" >&2; usage; exit 1; }
      REPORT_MD_PATH="$2"
      shift 2
      ;;
    --report-max-age-days)
      [[ $# -lt 2 ]] && { echo "error: --report-max-age-days requires a value" >&2; usage; exit 1; }
      REPORT_MAX_AGE_DAYS="$2"
      shift 2
      ;;
    --auto-report)
      AUTO_REPORT=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "error: unknown option $1" >&2
      usage
      exit 1
      ;;
  esac
done

if ! command -v python3 >/dev/null 2>&1; then
  echo "python3 is required to write the JSON summary." >&2
  exit 1
fi

if [[ ! -f "${MANIFEST_PATH}" ]]; then
  echo "fixture manifest not found: ${MANIFEST_PATH}" >&2
  exit 1
fi

if [[ ! -f "${SCHEMA_MANIFEST_PATH}" ]]; then
  echo "schema hash manifest not found: ${SCHEMA_MANIFEST_PATH}" >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${REPO_ROOT}"

if [[ "${AUTO_REPORT}" -eq 1 ]]; then
  if [[ -z "${REPORT_JSON_PATH}" ]]; then
    REPORT_JSON_PATH="artifacts/norito_rpc/rotation_status.json"
  fi
  if [[ -z "${REPORT_MD_PATH}" ]]; then
    REPORT_MD_PATH="artifacts/norito_rpc/rotation_status.md"
  fi
  if [[ -z "${REPORT_MAX_AGE_DAYS}" ]]; then
    REPORT_MAX_AGE_DAYS="7"
  fi
fi

if [[ "${ARTIFACTS_DIR}" != /* ]]; then
  ARTIFACTS_DIR="${REPO_ROOT}/${ARTIFACTS_DIR}"
fi
if [[ -n "${REPORT_JSON_PATH}" && "${REPORT_JSON_PATH}" != /* ]]; then
  REPORT_JSON_PATH="${REPO_ROOT}/${REPORT_JSON_PATH}"
fi
if [[ -n "${REPORT_MD_PATH}" && "${REPORT_MD_PATH}" != /* ]]; then
  REPORT_MD_PATH="${REPO_ROOT}/${REPORT_MD_PATH}"
fi
mkdir -p "${ARTIFACTS_DIR}"

timestamp=$(date -u +"%Y%m%dT%H%M%SZ")
iso_timestamp=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

sdk_slug="$(printf '%s' "${SDK_LABEL}" | tr '[:upper:]' '[:lower:]' | sed -E 's/[^a-z0-9]+/-/g' | sed -E 's/^-+|-+$//g')"
[[ -z "${sdk_slug}" ]] && sdk_slug="sdk"

log_path="${ARTIFACTS_DIR}/${timestamp}-${sdk_slug}-norito-rpc.log"
summary_path="${ARTIFACTS_DIR}/${timestamp}-${sdk_slug}-norito-rpc.json"
xtask_summary_path="${ARTIFACTS_DIR}/${timestamp}-${sdk_slug}-norito-rpc-xtask.json"

export CARGO_TERM_COLOR="${CARGO_TERM_COLOR:-never}"
if [[ "${ALLOW_ONLINE}" -eq 0 ]]; then
  export CARGO_NET_OFFLINE="${CARGO_NET_OFFLINE:-true}"
fi

command=("cargo" "xtask" "norito-rpc-verify" "--json-out" "${xtask_summary_path}")
echo "[norito-rpc] running ${command[*]}"

SECONDS=0
set +e
"${command[@]}" 2>&1 | tee "${log_path}"
status=$?
set -e
duration_secs=$SECONDS
result="passed"
if [[ ${status} -ne 0 ]]; then
  result="failed"
fi

repo_sha="$(git rev-parse HEAD)"
dirty="false"
if ! git diff --quiet --ignore-submodules HEAD --; then
  dirty="true"
fi

log_rel="${log_path#${REPO_ROOT}/}"
summary_rel="${summary_path#${REPO_ROOT}/}"
xtask_summary_rel="${xtask_summary_path#${REPO_ROOT}/}"

RESULT_STATUS="${result}"
ISO_TIMESTAMP="${iso_timestamp}"
DURATION_SECS="${duration_secs}"
REPO_SHA="${repo_sha}"
DIRTY_FLAG="${dirty}"
LOG_REL="${log_rel}"
SUMMARY_REL="${summary_rel}"
SUMMARY_PATH="${summary_path}"
ROTATION_LABEL="${ROTATION_LABEL}"
XTASK_SUMMARY_PATH="${xtask_summary_path}"
XTASK_SUMMARY_REL="${xtask_summary_rel}"

export SDK_LABEL NOTE ISO_TIMESTAMP RESULT_STATUS DURATION_SECS REPO_ROOT REPO_SHA DIRTY_FLAG MANIFEST_PATH SCHEMA_MANIFEST_PATH LOG_REL SUMMARY_REL SUMMARY_PATH ROTATION_LABEL XTASK_SUMMARY_PATH XTASK_SUMMARY_REL

python3 - <<'PY'
import json
import os
import hashlib
from pathlib import Path

def relpath(path: Path, root: Path) -> str:
    try:
        return str(path.relative_to(root))
    except ValueError:
        return str(path)

def digest(path: Path) -> dict:
    data = path.read_bytes()
    try:
        blake3_digest = hashlib.blake3(data).hexdigest()  # type: ignore[attr-defined]
    except AttributeError:
        try:
            import blake3  # type: ignore
            blake3_digest = blake3.blake3(data).hexdigest()
        except Exception:
            blake3_digest = None
    result = {
        "sha256": hashlib.sha256(data).hexdigest(),
        "bytes": len(data),
    }
    if blake3_digest is not None:
        result["blake3"] = blake3_digest
    return result

repo_root = Path(os.environ["REPO_ROOT"]).resolve()
manifest_path = Path(os.environ["MANIFEST_PATH"]).resolve()
schema_manifest_path = Path(os.environ["SCHEMA_MANIFEST_PATH"]).resolve()
rotation = os.environ.get("ROTATION_LABEL", "").strip()

data = {
    "sdk": os.environ["SDK_LABEL"],
    "timestamp": os.environ["ISO_TIMESTAMP"],
    "status": os.environ["RESULT_STATUS"],
    "duration_seconds": int(os.environ["DURATION_SECS"]),
    "command": "cargo xtask norito-rpc-verify",
    "repo": {
        "root": os.environ["REPO_ROOT"],
        "commit": os.environ["REPO_SHA"],
        "dirty": os.environ["DIRTY_FLAG"].lower() == "true",
    },
    "fixtures": {
        "manifest": {
            "path": relpath(manifest_path, repo_root),
            **digest(manifest_path),
        },
        "schema_manifest": {
            "path": relpath(schema_manifest_path, repo_root),
            **digest(schema_manifest_path),
        },
    },
    "artifacts": {
        "log": os.environ["LOG_REL"],
        "summary": os.environ["SUMMARY_REL"],
    },
}
xtask_path = Path(os.environ["XTASK_SUMMARY_PATH"]).resolve()
xtask_report = json.loads(xtask_path.read_text(encoding="utf-8"))
data["verification"] = {
    "artifact": {
        "path": os.environ["XTASK_SUMMARY_REL"],
        **digest(xtask_path),
    },
    "report": xtask_report,
}
note = os.environ.get("NOTE", "")
if note:
    data["note"] = note
if rotation:
    data["rotation"] = rotation

Path(os.environ["SUMMARY_PATH"]).write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")
PY

if [[ -n "${REPORT_JSON_PATH}" || -n "${REPORT_MD_PATH}" ]]; then
  report_cmd=("python3" "scripts/norito_rpc_fixture_report.py" "--root" "${ARTIFACTS_DIR}" "--glob" "*-norito-rpc.json" "--quiet")
  if [[ -n "${REPORT_JSON_PATH}" ]]; then
    mkdir -p "$(dirname "${REPORT_JSON_PATH}")"
    report_cmd+=("--output" "${REPORT_JSON_PATH}")
  fi
  if [[ -n "${REPORT_MD_PATH}" ]]; then
    mkdir -p "$(dirname "${REPORT_MD_PATH}")"
    report_cmd+=("--markdown" "${REPORT_MD_PATH}")
  fi
  if [[ -n "${REPORT_MAX_AGE_DAYS}" ]]; then
    report_cmd+=("--max-age-days" "${REPORT_MAX_AGE_DAYS}")
  fi
  if ! "${report_cmd[@]}"; then
    echo "[norito-rpc] warning: failed to aggregate cadence report" >&2
  fi
fi

echo "[norito-rpc] summary written to ${SUMMARY_REL}"
exit "${status}"

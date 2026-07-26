#!/usr/bin/env bash

# Run the SoraNet privacy differential privacy notebook non-interactively.
#
# This wrapper ensures the calibration artefacts are regenerated via the
# lightweight Python harness and then executes the accompanying notebook so the
# rendered outputs stay fresh for governance reviews and CI gates.

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")"/../.. && pwd)"
ARTIFACT_DIR="${SORANET_PRIVACY_DP_ARTIFACT_DIR:-${PROJECT_ROOT}/artifacts/soranet_privacy_dp}"
NOTEBOOK="${PROJECT_ROOT}/notebooks/soranet_privacy_dp.ipynb"
OUTPUT_NOTEBOOK="${ARTIFACT_DIR}/soranet_privacy_dp.executed.ipynb"
PYTHON_BIN="${PYTHON:-python3}"

if [[ -n "${SORANET_PRIVACY_DP_ARTIFACT_DIR:-}" ]]; then
  if [[ "${ARTIFACT_DIR}" != /* ]]; then
    printf 'SORANET_PRIVACY_DP_ARTIFACT_DIR must be absolute\n' >&2
    exit 1
  fi
  if [[ -e "${ARTIFACT_DIR}" || -L "${ARTIFACT_DIR}" ]]; then
    printf 'Refusing existing privacy DP artifact directory: %s\n' "${ARTIFACT_DIR}" >&2
    exit 1
  fi
  mkdir -m 0700 -- "${ARTIFACT_DIR}"
else
  mkdir -p "${ARTIFACT_DIR}"
fi
export SORANET_PRIVACY_DP_ARTIFACT_DIR="${ARTIFACT_DIR}"

if [[ ! -f "${NOTEBOOK}" ]]; then
  printf 'Notebook not found: %s\n' "${NOTEBOOK}" >&2
  exit 1
fi

"${PYTHON_BIN}" "${PROJECT_ROOT}/scripts/telemetry/run_privacy_dp.py"

if "${PYTHON_BIN}" - <<'PY' >/dev/null 2>&1; then
import importlib.util
raise SystemExit(0 if importlib.util.find_spec("papermill") else 1)
PY
  "${PYTHON_BIN}" -m papermill \
    "${NOTEBOOK}" \
    "${OUTPUT_NOTEBOOK}" \
    --cwd "${PROJECT_ROOT}" \
    --no-progress-bar
elif command -v papermill >/dev/null 2>&1; then
  papermill \
    "${NOTEBOOK}" \
    "${OUTPUT_NOTEBOOK}" \
    --cwd "${PROJECT_ROOT}" \
    --no-progress-bar
elif command -v jupyter >/dev/null 2>&1; then
  OUTPUT_BASENAME="$(basename "${OUTPUT_NOTEBOOK}")"
  jupyter nbconvert \
    --to notebook \
    --execute \
    --ExecutePreprocessor.timeout=600 \
    --ExecutePreprocessor.kernel_name=python3 \
    --output "${OUTPUT_BASENAME}" \
    --output-dir "${ARTIFACT_DIR}" \
    "${NOTEBOOK}"
else
  printf 'Unable to execute notebook. Install papermill (preferred) or jupyter.\n' >&2
  exit 1
fi

"${PYTHON_BIN}" "${PROJECT_ROOT}/scripts/telemetry/normalize_executed_notebook.py" \
  --notebook "${OUTPUT_NOTEBOOK}" \
  --source-date-epoch "${SOURCE_DATE_EPOCH:-0}"
printf 'Executed and normalized notebook: %s\n' "${OUTPUT_NOTEBOOK}"

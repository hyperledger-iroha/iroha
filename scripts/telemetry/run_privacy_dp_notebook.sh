#!/usr/bin/env bash

# Run the SoraNet privacy differential privacy notebook non-interactively.
#
# This wrapper ensures the calibration artefacts are regenerated via the
# lightweight Python harness and then executes the accompanying notebook so the
# rendered outputs stay fresh for governance reviews and CI gates.

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")"/../.. && pwd)"
ARTIFACT_DIR="${PROJECT_ROOT}/artifacts/soranet_privacy_dp"
NOTEBOOK="${PROJECT_ROOT}/notebooks/soranet_privacy_dp.ipynb"
OUTPUT_NOTEBOOK="${ARTIFACT_DIR}/soranet_privacy_dp.executed.ipynb"
PYTHON_BIN="${PYTHON:-python3}"

mkdir -p "${ARTIFACT_DIR}"

if [[ ! -f "${NOTEBOOK}" ]]; then
  printf 'Notebook not found: %s\n' "${NOTEBOOK}" >&2
  exit 1
fi

"${PYTHON_BIN}" "${PROJECT_ROOT}/scripts/telemetry/run_privacy_dp.py"

if "${PYTHON_BIN}" - <<'PY' >/dev/null 2>&1; then
import importlib.util
raise SystemExit(0 if importlib.util.find_spec("papermill") else 1)
PY
then
  "${PYTHON_BIN}" -m papermill \
    "${NOTEBOOK}" \
    "${OUTPUT_NOTEBOOK}" \
    --cwd "${PROJECT_ROOT}" \
    --no-progress-bar
  printf 'Executed notebook via papermill: %s\n' "${OUTPUT_NOTEBOOK}"
  exit 0
fi

if command -v papermill >/dev/null 2>&1; then
  papermill \
    "${NOTEBOOK}" \
    "${OUTPUT_NOTEBOOK}" \
    --cwd "${PROJECT_ROOT}" \
    --no-progress-bar
  printf 'Executed notebook via papermill: %s\n' "${OUTPUT_NOTEBOOK}"
  exit 0
fi

if command -v jupyter >/dev/null 2>&1; then
  OUTPUT_BASENAME="$(basename "${OUTPUT_NOTEBOOK}")"
  jupyter nbconvert \
    --to notebook \
    --execute \
    --ExecutePreprocessor.timeout=600 \
    --ExecutePreprocessor.kernel_name=python3 \
    --output "${OUTPUT_BASENAME}" \
    --output-dir "${ARTIFACT_DIR}" \
    "${NOTEBOOK}"
  printf 'Executed notebook via jupyter nbconvert: %s\n' "${OUTPUT_NOTEBOOK}"
  exit 0
fi

printf 'Unable to execute notebook. Install papermill (preferred) or jupyter.\n' >&2
exit 1

#!/usr/bin/env bash
set -euo pipefail

if (($#)); then
    printf 'Unknown argument: %s\n' "$1" >&2
    exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
PYTHON_DIR="${PROJECT_ROOT}/python/iroha_python"
DIST_DIR="${PYTHON_DIR}/dist"
KEEP_DIST="${PYTHON_RELEASE_SMOKE_KEEP_DIST:-}"

TMPDIR="$(mktemp -d)"
cleanup() {
    rm -rf "${TMPDIR}"
    if [[ -z "${KEEP_DIST}" ]]; then
        rm -rf "${DIST_DIR}"
    fi
}
trap cleanup EXIT

pushd "${PYTHON_DIR}" >/dev/null

python -m pip install --upgrade pip setuptools wheel build twine >/dev/null
python -m build >/dev/null

WHEEL="$(ls "${DIST_DIR}"/*.whl | head -n 1)"
popd >/dev/null

python -m venv "${TMPDIR}/venv"
source "${TMPDIR}/venv/bin/activate"
pip install --upgrade pip >/dev/null
pip install "${WHEEL}" >/dev/null
pip install pytest >/dev/null

python - <<'PY'
import iroha_python as sdk

print(f"iroha_python version: {sdk.__version__}")
assert hasattr(sdk, "ToriiClient")
PY

PYTHON_BIN="${TMPDIR}/venv/bin/python"
(
    cd "${PROJECT_ROOT}"
    PYTHON_BIN="${PYTHON_BIN}" python/iroha_python/scripts/run_norito_rpc_smoke.sh
)

# Verify metadata and perform a dry-run upload with twine. Dummy credentials allow the upload
# pipeline to execute without talking to PyPI.
python -m twine check "${DIST_DIR}"/* >/dev/null
TWINE_USERNAME="__token__" TWINE_PASSWORD="pypi-dry-run-token" \
    python -m twine upload --repository-url https://upload.pypi.org/legacy/ --dry-run "${DIST_DIR}"/* >/dev/null

# This harness deliberately performs no signing. Stage reviewed release
# candidates through scripts/release_manifest_signing.py and the protected
# external Ed25519/HSM workflow after this smoke test passes.

printf '%s\n' "${WHEEL}"

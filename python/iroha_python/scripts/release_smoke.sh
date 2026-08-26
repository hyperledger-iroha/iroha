#!/usr/bin/env bash
set -euo pipefail

if (($#)); then
    printf 'Unknown argument: %s\n' "$1" >&2
    exit 1
fi

SCRIPT_DIR="$(cd -P "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
PROJECT_ROOT="$(cd -P "${SCRIPT_DIR}/../.." && pwd -P)"
PYTHON_DIR="${PROJECT_ROOT}/python/iroha_python"
DIST_DIR="${PYTHON_DIR}/dist"
KEEP_DIST="${PYTHON_RELEASE_SMOKE_KEEP_DIST:-}"
DIST_CLEANUP_ENABLED=0

SMOKE_TMP_DIR="$(mktemp -d)"
cleanup() {
    rm -rf "${SMOKE_TMP_DIR}"
    if [[ -z "${KEEP_DIST}" && "${DIST_CLEANUP_ENABLED}" == "1" ]]; then
        rm -rf "${DIST_DIR}"
    fi
}
trap cleanup EXIT

python -I -B - "${PYTHON_DIR}" "${DIST_DIR}" <<'PY'
import os
import sys
from pathlib import Path

project = Path(sys.argv[1])
if project.resolve(strict=True) != project or project.is_symlink() or not project.is_dir():
    raise SystemExit("release smoke project directory must be one canonical real directory")
dist = Path(sys.argv[2])
if os.path.lexists(dist):
    if dist.is_symlink() or dist.resolve(strict=True) != dist or not dist.is_dir():
        raise SystemExit("release smoke dist path must be one canonical real directory")
    if next(dist.iterdir(), None) is not None:
        raise SystemExit("release smoke requires an empty pre-existing dist directory")
PY
DIST_CLEANUP_ENABLED=1

pushd "${PYTHON_DIR}" >/dev/null

python -m pip install --upgrade pip setuptools wheel build twine >/dev/null
python -m build >/dev/null
popd >/dev/null

python -m venv "${SMOKE_TMP_DIR}/venv"
source "${SMOKE_TMP_DIR}/venv/bin/activate"
pip install --upgrade pip >/dev/null
WHEEL="$(python -I -B - "${DIST_DIR}" <<'PY'
import sys
from pathlib import Path

directory = Path(sys.argv[1])
canonical_directory = directory.resolve(strict=True)
if canonical_directory != directory or directory.is_symlink() or not directory.is_dir():
    raise SystemExit("release smoke dist directory must be one canonical real directory")
candidates = tuple(sorted(directory.glob("*.whl")))
if len(candidates) != 1:
    raise SystemExit(
        f"release smoke requires exactly one wheel candidate, found {len(candidates)}"
    )
wheel = candidates[0]
if wheel.is_symlink() or wheel.resolve(strict=True) != wheel:
    raise SystemExit("release smoke wheel candidate must be canonical and non-symlinked")
print(wheel)
PY
)"
WHEEL_SEAL="$(
    python -I -B "${PROJECT_ROOT}/ci/verify_privacy_python_wheel.py" \
        --seal "${WHEEL}"
)"
PREFLIGHT_WHEEL="$(
    python -I -B "${PROJECT_ROOT}/ci/verify_privacy_python_wheel.py" \
        --preflight "${WHEEL}" "${WHEEL_SEAL}"
)"
if [[ "${PREFLIGHT_WHEEL}" != "${WHEEL}" ]]; then
    printf 'Wheel preflight returned an unexpected path: %s\n' \
        "${PREFLIGHT_WHEEL}" >&2
    exit 1
fi
pip install "${WHEEL}" --no-compile >/dev/null
pip install pytest >/dev/null

INSTALLED_NATIVE_PATH="$(
    python -I -B "${PROJECT_ROOT}/ci/verify_privacy_python_wheel.py" \
        "${SMOKE_TMP_DIR}/venv" \
        "${WHEEL}" \
        "${WHEEL_SEAL}" \
        "${PROJECT_ROOT}/python/norito_py/src" \
        "${PROJECT_ROOT}/python/iroha_torii_client"
)"
case "${INSTALLED_NATIVE_PATH}" in
    "${SMOKE_TMP_DIR}/venv/"*) ;;
    *)
        printf 'Installed native extension escaped the release-smoke venv: %s\n' \
            "${INSTALLED_NATIVE_PATH}" >&2
        exit 1
        ;;
esac

python -I -B - "${WHEEL_SEAL%%:*}" "${INSTALLED_NATIVE_PATH}" <<'PY'
import hashlib
import sys

import iroha_python as sdk

print(f"iroha_python version: {sdk.__version__}")
assert hasattr(sdk, "ToriiClient")
assert sdk.PRIVACY_REQUIRED_BRIDGE_ABI_VERSION == 23
assert sdk.privacy_bridge_abi_version() == sdk.PRIVACY_REQUIRED_BRIDGE_ABI_VERSION
assert sdk.is_privacy_native_available() is True
catalog = sdk.privacy_compiled_profile_catalog_v1()
assert isinstance(catalog, bytes)
assert 0 < len(catalog) <= sdk.PRIVACY_COMPILED_PROFILE_CATALOG_ARCHIVE_MAX_BYTES
print(f"authenticated wheel sha256: {sys.argv[1]}")
print(f"authenticated native extension: {sys.argv[2]}")
print(f"compiled privacy catalog sha256: {hashlib.sha256(catalog).hexdigest()}")
PY

PYTHON_BIN="${SMOKE_TMP_DIR}/venv/bin/python"
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
# external software Ed25519 workflow after this smoke test passes.

printf '%s\n' "${WHEEL}"

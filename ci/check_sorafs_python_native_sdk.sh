#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${SORAFS_PYTHON_SDK_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
PYTHON_BIN="${SORAFS_PYTHON_SDK_PYTHON_BIN:-python3}"
SDK_SESSION="$(mktemp -d "${TMPDIR:-/tmp}/iroha-sorafs-python-sdk.XXXXXX")"
SDK_SESSION="$(cd -P "${SDK_SESSION}" && pwd -P)"

cleanup_sdk_session() {
  rm -rf -- "${SDK_SESSION}"
}
trap cleanup_sdk_session EXIT

export PYTHONDONTWRITEBYTECODE=1

# Native Windows Python writes CRLF to redirected stdout. Captured paths, seals
# and JSON pins must not retain the line terminator's CR as part of their value.
# pipefail preserves an interpreter failure; JSON encodes embedded CR as \\r.
capture_python() {
  "$@" | tr -d '\r'
}

PYTHON_VERSION="$(capture_python "${PYTHON_BIN}" -I -c \
  'import sys; print(f"{sys.version_info.major}.{sys.version_info.minor}")')"
if [[ "${PYTHON_VERSION}" != "3.12" ]]; then
  echo "error: SoraFS native Python SDK tests require exact Python 3.12; got ${PYTHON_VERSION}" >&2
  exit 1
fi

TRACKED_NATIVE_EXTENSIONS="$(
  git -C "${ROOT_DIR}" ls-files -- \
    'python/iroha_python/src/iroha_python/*.so' \
    'python/iroha_python/src/iroha_python/*.so.*' \
    'python/iroha_python/src/iroha_python/*.dylib' \
    'python/iroha_python/src/iroha_python/*.pyd' \
    'python/iroha_python/src/iroha_python/*.dll' \
    'python/iroha_native/src/iroha_native/*.so' \
    'python/iroha_native/src/iroha_native/*.so.*' \
    'python/iroha_native/src/iroha_native/*.dylib' \
    'python/iroha_native/src/iroha_native/*.pyd' \
    'python/iroha_native/src/iroha_native/*.dll'
)"
if [[ -n "${TRACKED_NATIVE_EXTENSIONS}" ]]; then
  echo "error: Python native SDK artifacts must be rebuilt in the ABI-31 lane, not tracked:" >&2
  printf '%s\n' "${TRACKED_NATIVE_EXTENSIONS}" >&2
  exit 1
fi

# The original full clean-source pin brackets compilation, packaging and both
# test modes. Artifact inspection after compilation cannot establish this pin.
SOURCE_DELIVERY="${ROOT_DIR}/ci/python_native_source_delivery.py"
SOURCE_BEFORE_PIN="$(capture_python "${PYTHON_BIN}" -I -B "${SOURCE_DELIVERY}" pin --root "${ROOT_DIR}")"
SOURCE_BEFORE_ARTIFACT="$(capture_python "${PYTHON_BIN}" -I -B "${SOURCE_DELIVERY}" inspect --root "${ROOT_DIR}")"
assert_build_source_unchanged() {
  "${PYTHON_BIN}" -I -B "${SOURCE_DELIVERY}" assert-pin \
    --root "${ROOT_DIR}" --source-pin "${SOURCE_BEFORE_PIN}"
}

"${PYTHON_BIN}" -m venv "${SDK_SESSION}/venv"
# Query the selected interpreter, not the shell host: Git Bash still launches
# native Windows Python, whose venv executables live in Scripts rather than bin.
PYTHON_PLATFORM="$(capture_python "${PYTHON_BIN}" -I -c 'import os; print(os.name)')"
if [[ "${PYTHON_PLATFORM}" == "nt" ]]; then
  VENV_SCRIPTS="${SDK_SESSION}/venv/Scripts"
  VENV_PYTHON="${VENV_SCRIPTS}/python.exe"
  NATIVE_HELPER_SUFFIX=".exe"
else
  VENV_SCRIPTS="${SDK_SESSION}/venv/bin"
  VENV_PYTHON="${VENV_SCRIPTS}/python"
  NATIVE_HELPER_SUFFIX=""
fi
VIRTUAL_ENV="$(capture_python "${VENV_PYTHON}" -I -c 'import sys; print(sys.prefix)')"
export VIRTUAL_ENV
export PATH="${VENV_SCRIPTS}:${PATH}"
"${VENV_PYTHON}" -m pip install \
  --require-hashes \
  --only-binary=:all: \
  -r "${ROOT_DIR}/python/iroha_python/requirements-ci.lock"
"${VENV_PYTHON}" -m pip install --no-deps \
  "${ROOT_DIR}/python/norito_py" \
  "${ROOT_DIR}/python/iroha_torii_client"

# Build the canonical filesystem owner from the same original clean-source pin.
# This isolated target cannot select an ambient previously compiled helper.
NATIVE_HELPER_TARGET="${SDK_SESSION}/native-helper-target"
NATIVE_HELPER_HOST="$("${RUSTC:-rustc}" -vV | awk '/^host: / { print $2 }')"
[[ -n "${NATIVE_HELPER_HOST}" ]] || { echo "error: native helper compiler has no host target" >&2; exit 1; }
(cd "${ROOT_DIR}" && cargo build --locked -p iroha_native_source_delivery \
  --bin iroha_native_source_delivery --target "${NATIVE_HELPER_HOST}" \
  --target-dir "${NATIVE_HELPER_TARGET}")
assert_build_source_unchanged
NATIVE_HELPER="$(capture_python "${VENV_PYTHON}" -I -B -c \
  'from pathlib import Path; import sys; print(Path(sys.argv[1]).resolve(strict=True))' \
  "${NATIVE_HELPER_TARGET}/${NATIVE_HELPER_HOST}/debug/iroha_native_source_delivery${NATIVE_HELPER_SUFFIX}")"

# These are file/admission controls only; actual native qualification follows
# from the original fresh build and both installed/source suites below.
IROHA_NATIVE_SOURCE_DELIVERY_TEST_HELPER="${NATIVE_HELPER}" \
"${VENV_PYTHON}" -I -B -m pytest -q -p no:cacheprovider \
  --basetemp "${SDK_SESSION}/source-control-tmp" \
  "${ROOT_DIR}/scripts/tests/python_native_source_delivery_test.py" \
  "${ROOT_DIR}/scripts/tests/python_source_owner_admission_test.py" \
  "${ROOT_DIR}/scripts/tests/python_native_delivery_gate_contract_test.py" \
  "${ROOT_DIR}/scripts/tests/sorafs_python_native_platform_test.py"
assert_build_source_unchanged

NATIVE_WHEELS="${SDK_SESSION}/native-wheels"
SDK_WHEELS="${SDK_SESSION}/sdk-wheels"
mkdir -m 700 "${NATIVE_WHEELS}" "${SDK_WHEELS}"
cd "${ROOT_DIR}/python/iroha_native"
IROHA_PYTHON_SKIP_RUNTIME_LINK=1 \
"${VENV_PYTHON}" -m maturin build --release --locked --out "${NATIVE_WHEELS}"
assert_build_source_unchanged
cd "${ROOT_DIR}/python/iroha_python"
"${VENV_PYTHON}" -I -B -m pip --isolated wheel --no-deps --no-index \
  --no-build-isolation --wheel-dir "${SDK_WHEELS}" .
assert_build_source_unchanged
select_wheel() {
  capture_python "${VENV_PYTHON}" -I -B - "$1" <<'PYSELECT'
from pathlib import Path
import sys
root = Path(sys.argv[1])
entries = tuple(root.iterdir())
if len(entries) != 1 or entries[0].suffix != ".whl":
    raise SystemExit("each Python owner must produce exactly one fresh wheel")
print(entries[0])
PYSELECT
}
NATIVE_WHEEL="$(select_wheel "${NATIVE_WHEELS}")"
SDK_WHEEL="$(select_wheel "${SDK_WHEELS}")"
WHEEL_VERIFIER="${ROOT_DIR}/ci/verify_privacy_python_wheel.py"
NATIVE_SEAL="$(capture_python "${VENV_PYTHON}" -I -B "${WHEEL_VERIFIER}" --seal "${NATIVE_WHEEL}")"
SDK_SEAL="$(capture_python "${VENV_PYTHON}" -I -B "${WHEEL_VERIFIER}" --seal "${SDK_WHEEL}")"
"${VENV_PYTHON}" -I -B "${WHEEL_VERIFIER}" --preflight native "${NATIVE_WHEEL}" "${NATIVE_SEAL}"
"${VENV_PYTHON}" -I -B "${WHEEL_VERIFIER}" --preflight sdk "${SDK_WHEEL}" "${SDK_SEAL}"
"${VENV_PYTHON}" -I -B -m pip --isolated install --no-compile --no-deps --no-index \
  "${NATIVE_WHEEL}" "${SDK_WHEEL}"
verify_installed_wheels() {
  capture_python "${VENV_PYTHON}" -I -B "${WHEEL_VERIFIER}" "${SDK_SESSION}/venv" \
    "${NATIVE_WHEEL}" "${NATIVE_SEAL}" \
    "${ROOT_DIR}/python/norito_py/src" "${ROOT_DIR}/python/iroha_torii_client" \
    "${SDK_WHEEL}" "${SDK_SEAL}"
}
NATIVE_EXTENSION="$(verify_installed_wheels)"
assert_build_source_unchanged
PYTHONPATH="$(capture_python "${VENV_PYTHON}" -I -B -c \
  'import os, sys; print(os.pathsep.join(os.path.abspath(path) for path in sys.argv[1:]))' \
  "${ROOT_DIR}/python/norito_py/src" "${ROOT_DIR}/python")"
export PYTHONPATH

NATIVE_TARGET="$(capture_python "${VENV_PYTHON}" -I -c \
  'import platform, sys; print(f"{platform.system().lower()}-{platform.machine().lower()}-python{sys.version_info.major}{sys.version_info.minor}")')"
NATIVE_MANIFEST="${SDK_SESSION}/python-native-abi31.json"
"${VENV_PYTHON}" -I "${ROOT_DIR}/scripts/check_native_sdk_artifact.py" \
  record \
  --artifact "${NATIVE_EXTENSION}" \
  --manifest "${NATIVE_MANIFEST}" \
  --source-root "${ROOT_DIR}" \
  --python "${VENV_PYTHON}" \
  --sdk python \
  --target "${NATIVE_TARGET}"
"${VENV_PYTHON}" -I "${ROOT_DIR}/scripts/check_native_sdk_artifact.py" \
  verify \
  --artifact "${NATIVE_EXTENSION}" \
  --manifest "${NATIVE_MANIFEST}" \
  --source-root "${ROOT_DIR}" \
  --python "${VENV_PYTHON}"

assert_build_source_unchanged
NATIVE_MANIFEST_SEAL="$(capture_python "${VENV_PYTHON}" -I -B "${WHEEL_VERIFIER}" --seal "${NATIVE_MANIFEST}")"
# Keep the original native artifact and sealed delivery receipt after session
# cleanup. This diagnostic backup is never a loader search path or fallback.
# The retained native creator must create the absent backup with protected
# current-user access. A Git Bash mkdir does not install a private Windows DACL.
SOURCE_BACKUP_DIRECTORY="$(capture_python "${VENV_PYTHON}" -I -B -c \
  'from pathlib import Path; import sys, uuid; print(Path(sys.argv[1]).resolve(strict=True) / (".codex-python-native-source." + uuid.uuid4().hex))' \
  "${ROOT_DIR}")"
SOURCE_RECEIPT_SEAL="$(capture_python "${VENV_PYTHON}" -I -B "${SOURCE_DELIVERY}" promote \
  --root "${ROOT_DIR}" --source-pin "${SOURCE_BEFORE_PIN}" \
  --source-state "${SOURCE_BEFORE_ARTIFACT}" \
  --wheel "${NATIVE_WHEEL}" --wheel-seal "${NATIVE_SEAL}" \
  --manifest "${NATIVE_MANIFEST}" --manifest-seal "${NATIVE_MANIFEST_SEAL}" \
  --backup-directory "${SOURCE_BACKUP_DIRECTORY}" --native-helper "${NATIVE_HELPER}")"
verify_source_delivery() {
  "${VENV_PYTHON}" -I -B "${SOURCE_DELIVERY}" verify \
    --root "${ROOT_DIR}" --source-pin "${SOURCE_BEFORE_PIN}" \
    --wheel "${NATIVE_WHEEL}" --wheel-seal "${NATIVE_SEAL}" \
    --manifest "${NATIVE_MANIFEST}" --manifest-seal "${NATIVE_MANIFEST_SEAL}" \
    --receipt "${SOURCE_BACKUP_DIRECTORY}/delivery.json" \
    --receipt-seal "${SOURCE_RECEIPT_SEAL}"
}
verify_source_delivery

JUNIT_REPORT="${SDK_SESSION}/pytest.xml"
# Require the private wheel owners in conftest; source-tree imports would not
# exercise the artifacts authenticated above and rechecked after the suite.
IROHA_PYTHON_TEST_INSTALLED_PACKAGE=1 \
"${VENV_PYTHON}" -m pytest -q -p no:cacheprovider \
  --junitxml "${JUNIT_REPORT}" \
  tests/cancel_asset_lock_v1_test.py \
  tests/cancel_asset_lock_client_helpers_test.py \
  tests/client_hard_cut_contract_test.py \
  tests/client_ledger_helpers_test.py \
  tests/client_sorafs_orderbook_test.py \
  tests/sorafs_reference_validation_test.py \
  tests/sorafs_replication_instruction_test.py \
  ../iroha_torii_client/tests/orderbook_submission_test.py \
  tests/account_identity_native_v1_test.py \
  tests/requests_deadline_test.py \
  tests/staking_preparation_test.py \
  tests/validator_staking_test.py
"${VENV_PYTHON}" -I - "${JUNIT_REPORT}" <<'PY'
from pathlib import Path
import sys
import xml.etree.ElementTree as ET

root = ET.parse(Path(sys.argv[1])).getroot()
suites = [root] if root.tag == "testsuite" else list(root.findall("testsuite"))
skipped = sum(int(suite.attrib.get("skipped", "0")) for suite in suites)
if skipped:
    raise SystemExit(
        f"SoraFS native Python SDK parity may not contain skipped tests; found {skipped}"
    )
PY

# Source tests use the same promoted native member with their natural source
# conftest. A fresh cache prefix prevents prior source bytecode being an input.
SOURCE_JUNIT_REPORT="${SDK_SESSION}/source-pytest.xml"
env -u IROHA_PYTHON_TEST_INSTALLED_PACKAGE \
  PYTHONPYCACHEPREFIX="${SDK_SESSION}/source-bytecode" \
  "${VENV_PYTHON}" -B -m pytest -q -p no:cacheprovider \
  --basetemp "${SDK_SESSION}/source-test-tmp" \
  --junitxml "${SOURCE_JUNIT_REPORT}" \
  tests/account_identity_native_v1_test.py \
  tests/requests_deadline_test.py \
  tests/staking_preparation_test.py \
  tests/validator_staking_test.py
"${VENV_PYTHON}" -I - "${SOURCE_JUNIT_REPORT}" <<'PY'
from pathlib import Path
import sys
import xml.etree.ElementTree as ET

root = ET.parse(Path(sys.argv[1])).getroot()
suites = [root] if root.tag == "testsuite" else list(root.findall("testsuite"))
skipped = sum(int(suite.attrib.get("skipped", "0")) for suite in suites)
if skipped:
    raise SystemExit(
        f"Python native source qualification may not contain skipped tests; found {skipped}"
    )
PY
assert_build_source_unchanged
verify_source_delivery

# Original source-artifact backups remain in their ignored private owner. Release
# and CI callers may retain one payload-free manifest by naming a fresh directory
# outside the source tree, without retaining native payloads in release evidence.
# The checker creates that directory without following
# symlinks only after the native suite and its zero-skip audit both succeed.
VERIFY_EVIDENCE_ARGS=()
if [[ -n "${SORAFS_PYTHON_SDK_EVIDENCE_DIR:-}" ]]; then
  VERIFY_EVIDENCE_ARGS=(
    --evidence-dir "${SORAFS_PYTHON_SDK_EVIDENCE_DIR}"
  )
fi
"${VENV_PYTHON}" -I "${ROOT_DIR}/scripts/check_native_sdk_artifact.py" \
  verify \
  --artifact "${NATIVE_EXTENSION}" \
  --manifest "${NATIVE_MANIFEST}" \
  --source-root "${ROOT_DIR}" \
  --python "${VENV_PYTHON}" \
  "${VERIFY_EVIDENCE_ARGS[@]}"

verify_installed_wheels >/dev/null
assert_build_source_unchanged
verify_source_delivery

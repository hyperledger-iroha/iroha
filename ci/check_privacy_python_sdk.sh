#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${PRIVACY_PYTHON_SDK_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
PYTHON_OVERRIDE="${PRIVACY_PYTHON_SDK_PYTHON_BIN:-}"
VENV_DIR="${PRIVACY_PYTHON_SDK_VENV:-${TMPDIR:-/tmp}/iroha-privacy-python-sdk-venv}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

export PYTHONDONTWRITEBYTECODE=1

resolve_python_312_bin() {
  if [[ -n "${PYTHON_OVERRIDE}" ]]; then
    printf '%s\n' "${PYTHON_OVERRIDE}"
    return 0
  fi

  local candidate
  for candidate in \
    python3.12 \
    /opt/homebrew/bin/python3.12 \
    /opt/homebrew/opt/python@3.12/bin/python3.12 \
    /usr/local/bin/python3.12 \
    /usr/local/opt/python@3.12/bin/python3.12 \
    python3; do
    if command -v "${candidate}" >/dev/null 2>&1; then
      command -v "${candidate}"
      return 0
    fi
    if [[ -x "${candidate}" ]]; then
      printf '%s\n' "${candidate}"
      return 0
    fi
  done

  printf '%s\n' "python3"
}

PYTHON_BIN="$(resolve_python_312_bin)"

create_venv() {
  "${PYTHON_BIN}" -m venv "${VENV_DIR}"
}

recreate_venv() {
  case "${VENV_DIR}" in
    ""|"/"|".")
      echo "error: refusing to recreate unsafe privacy Python SDK venv path: ${VENV_DIR}" >&2
      exit 1
      ;;
  esac

  rm -rf "${VENV_DIR}"
  create_venv
}

PYTHON_VERSION="$("${PYTHON_BIN}" -c 'import sys; print(".".join(map(str, sys.version_info[:2])))')"
"${PYTHON_BIN}" --version
case "${PYTHON_VERSION}" in
  3.12) ;;
  *)
    echo "error: privacy Python SDK tests require Python 3.12; got ${PYTHON_VERSION}" >&2
    exit 1
    ;;
esac

# Maturin may invoke Cargo for both metadata and compilation. Route every such
# invocation through a wrapper that pins Cargo's explicit external lockfile;
# `--locked` by itself would still select and potentially rewrite the workspace
# Cargo.lock.
# shellcheck source=ci/privacy_sdk_cargo_lockfile.sh
source "${SCRIPT_DIR}/privacy_sdk_cargo_lockfile.sh"
SELECTED_CARGO_LOCKFILE="$(
  privacy_sdk_resolve_cargo_lockfile "${ROOT_DIR}" "${PYTHON_BIN}"
)"
export IROHA_PRIVACY_CARGO_LOCKFILE_PATH="${SELECTED_CARGO_LOCKFILE}"
export IROHA_JS_CARGO_LOCKFILE_PATH="${SELECTED_CARGO_LOCKFILE}"
export IROHA_PRIVACY_AUTHENTICATED_CARGO_LOCKFILE_PATH="${SELECTED_CARGO_LOCKFILE}"
export IROHA_PRIVACY_SDK_ROOT="${ROOT_DIR}"
export IROHA_PRIVACY_LOCKFILE_PYTHON_BIN="${PYTHON_BIN}"

WORKSPACE_CARGO_LOCKFILE="${ROOT_DIR}/Cargo.lock"
WORKSPACE_CARGO_LOCK_STATE="$(
  privacy_sdk_capture_optional_file_state \
    "${WORKSPACE_CARGO_LOCKFILE}" \
    "workspace Cargo.lock" \
    "${PYTHON_BIN}"
)"
SELECTED_CARGO_LOCK_SEAL="$(
  privacy_sdk_file_seal "${SELECTED_CARGO_LOCKFILE}" "${PYTHON_BIN}"
)"
export IROHA_PRIVACY_AUTHENTICATED_CARGO_LOCKFILE_SEAL="${SELECTED_CARGO_LOCK_SEAL}"
export IROHA_PRIVACY_AUTHENTICATED_WORKSPACE_CARGO_LOCK_STATE="${WORKSPACE_CARGO_LOCK_STATE}"

REAL_CARGO_CANDIDATE="${IROHA_PRIVACY_REAL_CARGO:-}"
if [[ -z "${REAL_CARGO_CANDIDATE}" ]]; then
  REAL_CARGO_CANDIDATE="$(command -v cargo || true)"
fi
if [[ -z "${REAL_CARGO_CANDIDATE}" ]]; then
  echo "error: Cargo is required for the privacy Python SDK native build" >&2
  exit 1
fi
IROHA_PRIVACY_REAL_CARGO="$(
  "${PYTHON_BIN}" -I - "${REAL_CARGO_CANDIDATE}" <<'PY'
import os
import sys
from pathlib import Path

candidate = Path(sys.argv[1])
if not candidate.is_absolute() or candidate != Path(os.path.abspath(candidate)):
    raise SystemExit("error: Cargo path must be absolute and normalized")
if not candidate.is_file() or not os.access(candidate, os.X_OK):
    raise SystemExit("error: resolved Cargo path must be an executable file")
print(candidate)
PY
)"
export IROHA_PRIVACY_REAL_CARGO

PRIVATE_CARGO_WRAPPER_DIR="$(
  mktemp -d "${TMPDIR:-/tmp}/iroha-privacy-sdk-cargo.XXXXXX"
)"
PRIVATE_CARGO_AUDIT_PATH="${PRIVATE_CARGO_WRAPPER_DIR}/invocations"
: >"${PRIVATE_CARGO_AUDIT_PATH}"
chmod 600 "${PRIVATE_CARGO_AUDIT_PATH}"
ln -s "${SCRIPT_DIR}/privacy_sdk_cargo_wrapper.sh" \
  "${PRIVATE_CARGO_WRAPPER_DIR}/cargo"
export IROHA_PRIVACY_CARGO_AUDIT_PATH="${PRIVATE_CARGO_AUDIT_PATH}"
export CARGO="${PRIVATE_CARGO_WRAPPER_DIR}/cargo"
export PATH="${PRIVATE_CARGO_WRAPPER_DIR}:${PATH}"
export RUSTC_BOOTSTRAP=1

assert_privacy_sdk_cargo_locks_unchanged() {
  local status=0
  privacy_sdk_assert_file_seal \
    "${SELECTED_CARGO_LOCKFILE}" \
    "${SELECTED_CARGO_LOCK_SEAL}" \
    "selected external Cargo.lock" \
    "${PYTHON_BIN}" || status=1
  privacy_sdk_assert_optional_file_state \
    "${WORKSPACE_CARGO_LOCKFILE}" \
    "${WORKSPACE_CARGO_LOCK_STATE}" \
    "workspace Cargo.lock" \
    "${PYTHON_BIN}" || status=1
  return "${status}"
}

cleanup_privacy_sdk_cargo_wrapper() {
  local status=$?
  trap - EXIT HUP INT TERM
  if ! assert_privacy_sdk_cargo_locks_unchanged; then
    status=1
  fi
  if [[ -n "${PRIVATE_CARGO_WRAPPER_DIR:-}" && -d "${PRIVATE_CARGO_WRAPPER_DIR}" ]]; then
    rm -rf -- "${PRIVATE_CARGO_WRAPPER_DIR}"
  fi
  exit "${status}"
}
trap cleanup_privacy_sdk_cargo_wrapper EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

if [[ ! -x "${VENV_DIR}/bin/python" ]]; then
  create_venv
fi

VENV_PYTHON_VERSION="$("${VENV_DIR}/bin/python" -c 'import sys; print(".".join(map(str, sys.version_info[:2])))')"
"${VENV_DIR}/bin/python" --version
case "${VENV_PYTHON_VERSION}" in
  3.12) ;;
  *)
    echo "recreating privacy Python SDK venv because it uses Python ${VENV_PYTHON_VERSION}, not 3.12" >&2
    recreate_venv
    VENV_PYTHON_VERSION="$("${VENV_DIR}/bin/python" -c 'import sys; print(".".join(map(str, sys.version_info[:2])))')"
    "${VENV_DIR}/bin/python" --version
    case "${VENV_PYTHON_VERSION}" in
      3.12) ;;
      *)
        echo "error: privacy Python SDK venv must use Python 3.12; got ${VENV_PYTHON_VERSION}" >&2
        exit 1
        ;;
    esac
    ;;
esac

export VIRTUAL_ENV="${VENV_DIR}"
export PATH="${VENV_DIR}/bin:${PATH}"

"${VENV_DIR}/bin/python" -m pip install 'pytest>=8.0' 'requests>=2.31' 'urllib3<2' 'maturin>=1.5,<2'
"${VENV_DIR}/bin/python" -m pip install --no-deps \
  "${ROOT_DIR}/python/norito_py" \
  "${ROOT_DIR}/python/iroha_torii_client"

cd "${ROOT_DIR}/python/iroha_python"
"${VENV_DIR}/bin/python" -m maturin develop --release --locked
if [[ ! -s "${PRIVATE_CARGO_AUDIT_PATH}" ]]; then
  echo "error: maturin completed without using the authenticated Cargo wrapper" >&2
  exit 1
fi
while IFS= read -r cargo_command; do
  case "${cargo_command}" in
    metadata|rustc|build|check|test|run|bench|doc|fetch|tree|package|publish|install) ;;
    *)
      echo "error: privacy SDK Cargo audit contains an invalid command" >&2
      exit 1
      ;;
  esac
done <"${PRIVATE_CARGO_AUDIT_PATH}"
assert_privacy_sdk_cargo_locks_unchanged
export PYTHONPATH="${ROOT_DIR}/python/iroha_python/src:${ROOT_DIR}/python/norito_py/src:${ROOT_DIR}/python${PYTHONPATH:+:${PYTHONPATH}}"
"${VENV_DIR}/bin/python" -m pytest -q \
  tests/privacy_catalog_test.py \
  tests/package_import_fallback_test.py \
  tests/privacy_native_registry_test.py \
  tests/crypto_algorithms_test.py

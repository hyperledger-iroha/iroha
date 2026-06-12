#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${KAGEMUSHA_RECURSIVE_SPEND_PYTHON_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
PYTHON_OVERRIDE="${KAGEMUSHA_RECURSIVE_SPEND_PYTHON_BIN:-}"
VENV_DIR="${KAGEMUSHA_RECURSIVE_SPEND_PYTHON_VENV:-${TMPDIR:-/tmp}/iroha-kagemusha-python-sdk-venv}"

export PYTHONDONTWRITEBYTECODE=1

resolve_python_311_bin() {
  if [[ -n "${PYTHON_OVERRIDE}" ]]; then
    printf '%s\n' "${PYTHON_OVERRIDE}"
    return 0
  fi

  local candidate
  for candidate in python3.11 /opt/homebrew/bin/python3.11 /usr/local/bin/python3.11 python3; do
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

PYTHON_BIN="$(resolve_python_311_bin)"

create_venv() {
  "${PYTHON_BIN}" -m venv "${VENV_DIR}"
}

recreate_venv() {
  case "${VENV_DIR}" in
    ""|"/"|".")
      echo "error: refusing to recreate unsafe Kagemusha recursive spend Python SDK venv path: ${VENV_DIR}" >&2
      exit 1
      ;;
  esac

  rm -rf "${VENV_DIR}"
  create_venv
}

PYTHON_VERSION="$("${PYTHON_BIN}" -c 'import sys; print(".".join(map(str, sys.version_info[:2])))')"
"${PYTHON_BIN}" --version
case "${PYTHON_VERSION}" in
  3.11) ;;
  *)
    echo "error: Kagemusha recursive spend Python SDK tests require Python 3.11; got ${PYTHON_VERSION}" >&2
    exit 1
    ;;
esac

if [[ ! -x "${VENV_DIR}/bin/python" ]]; then
  create_venv
fi

VENV_PYTHON_VERSION="$("${VENV_DIR}/bin/python" -c 'import sys; print(".".join(map(str, sys.version_info[:2])))')"
"${VENV_DIR}/bin/python" --version
case "${VENV_PYTHON_VERSION}" in
  3.11) ;;
  *)
    echo "recreating Kagemusha recursive spend Python SDK venv because it uses Python ${VENV_PYTHON_VERSION}, not 3.11" >&2
    recreate_venv
    VENV_PYTHON_VERSION="$("${VENV_DIR}/bin/python" -c 'import sys; print(".".join(map(str, sys.version_info[:2])))')"
    "${VENV_DIR}/bin/python" --version
    case "${VENV_PYTHON_VERSION}" in
      3.11) ;;
      *)
        echo "error: Kagemusha recursive spend Python SDK venv must use Python 3.11; got ${VENV_PYTHON_VERSION}" >&2
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
"${VENV_DIR}/bin/python" -m maturin develop --release
export PYTHONPATH="${ROOT_DIR}/python/iroha_python/src:${ROOT_DIR}/python/norito_py/src:${ROOT_DIR}/python${PYTHONPATH:+:${PYTHONPATH}}"
"${VENV_DIR}/bin/python" -m pytest -q \
  tests/kagemusha_test.py \
  tests/privacy_catalog_test.py \
  tests/crypto_algorithms_test.py \
  tests/offline_cash_test.py \
  tests/test_address_format.py \
  "${ROOT_DIR}/python/iroha_torii_client/tests/test_client.py::test_canonical_request_auth_rejects_padded_fields_before_send" \
  "${ROOT_DIR}/python/iroha_torii_client/tests/test_client.py::test_identifier_resolution_receipt_matches_shared_vectors"
"${VENV_DIR}/bin/python" -m pytest -q tests/client_ledger_helpers_test.py \
  -k "zk_event_filters_reject_unsupported_backends_before_request or zk_verifying_key_event_filters_reject_malformed_names_before_request or zk_proof_event_filters_reject_malformed_hashes_before_request or zk_raw_event_filters_reject_malformed_privacy_matchers_before_request or zk_raw_event_filters_canonicalize_privacy_matchers_before_request"

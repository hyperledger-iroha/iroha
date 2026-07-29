#!/usr/bin/env bash
set -euo pipefail

BOOTSTRAP_PYTHON="${IROHA_PRIVACY_LOCKFILE_PYTHON_BIN:-python3}"
SCRIPT_PATH="$(
  "${BOOTSTRAP_PYTHON}" -I - "${BASH_SOURCE[0]}" <<'PY'
from pathlib import Path
import sys

print(Path(sys.argv[1]).resolve(strict=True))
PY
)"
SCRIPT_DIR="$(cd "$(dirname "${SCRIPT_PATH}")" && pwd -P)"
# shellcheck source=ci/privacy_sdk_cargo_lockfile.sh
source "${SCRIPT_DIR}/privacy_sdk_cargo_lockfile.sh"

: "${IROHA_PRIVACY_SDK_ROOT:?privacy SDK Cargo wrapper requires IROHA_PRIVACY_SDK_ROOT}"
: "${IROHA_PRIVACY_REAL_CARGO:?privacy SDK Cargo wrapper requires IROHA_PRIVACY_REAL_CARGO}"
: "${IROHA_PRIVACY_CARGO_AUDIT_PATH:?privacy SDK Cargo wrapper requires IROHA_PRIVACY_CARGO_AUDIT_PATH}"
: "${IROHA_PRIVACY_AUTHENTICATED_CARGO_LOCKFILE_PATH:?privacy SDK Cargo wrapper requires an authenticated Cargo lock path}"
: "${IROHA_PRIVACY_AUTHENTICATED_CARGO_LOCKFILE_SEAL:?privacy SDK Cargo wrapper requires an authenticated Cargo lock seal}"
: "${IROHA_PRIVACY_AUTHENTICATED_WORKSPACE_CARGO_LOCK_STATE:?privacy SDK Cargo wrapper requires an authenticated workspace lock state}"

PYTHON_BIN="${IROHA_PRIVACY_LOCKFILE_PYTHON_BIN:-python3}"
SELECTED_LOCKFILE="$(
  privacy_sdk_resolve_cargo_lockfile "${IROHA_PRIVACY_SDK_ROOT}" "${PYTHON_BIN}"
)"
if [[ "${SELECTED_LOCKFILE}" != "${IROHA_PRIVACY_AUTHENTICATED_CARGO_LOCKFILE_PATH}" ]]; then
  echo "error: privacy SDK Cargo lock selection does not match the authenticated path" >&2
  exit 1
fi

assert_authenticated_cargo_lock_state() {
  local status=0
  privacy_sdk_assert_file_seal \
    "${SELECTED_LOCKFILE}" \
    "${IROHA_PRIVACY_AUTHENTICATED_CARGO_LOCKFILE_SEAL}" \
    "selected external Cargo.lock" \
    "${PYTHON_BIN}" || status=1
  privacy_sdk_assert_optional_file_state \
    "${IROHA_PRIVACY_SDK_ROOT}/Cargo.lock" \
    "${IROHA_PRIVACY_AUTHENTICATED_WORKSPACE_CARGO_LOCK_STATE}" \
    "workspace Cargo.lock" \
    "${PYTHON_BIN}" || status=1
  return "${status}"
}

run_real_cargo_and_verify_locks() {
  local status
  set +e
  "${IROHA_PRIVACY_REAL_CARGO}" "$@"
  status=$?
  set -e
  if ! assert_authenticated_cargo_lock_state; then
    status=1
  fi
  exit "${status}"
}

assert_authenticated_cargo_lock_state

if [[ ! -x "${IROHA_PRIVACY_REAL_CARGO}" ]]; then
  echo "error: IROHA_PRIVACY_REAL_CARGO must name an executable Cargo binary" >&2
  exit 1
fi
if [[ "${IROHA_PRIVACY_REAL_CARGO}" == "$0" ]]; then
  echo "error: privacy SDK Cargo wrapper cannot invoke itself" >&2
  exit 1
fi
if [[ ! -f "${IROHA_PRIVACY_CARGO_AUDIT_PATH}" || -L "${IROHA_PRIVACY_CARGO_AUDIT_PATH}" ]]; then
  echo "error: privacy SDK Cargo audit path must be an existing regular file" >&2
  exit 1
fi

command_name=""
for argument in "$@"; do
  case "${argument}" in
    metadata|rustc|build|check|test|run|bench|doc|fetch|tree|package|publish|install)
      command_name="${argument}"
      break
      ;;
  esac
done

if [[ -z "${command_name}" ]]; then
  case "$*" in
    "--version"|"-V"|"version")
      run_real_cargo_and_verify_locks "$@"
      ;;
    *)
      echo "error: privacy SDK Cargo wrapper rejected an unknown non-locking invocation" >&2
      exit 1
      ;;
  esac
fi

for argument in "$@"; do
  case "${argument}" in
    --lockfile-path|--lockfile-path=*)
      echo "error: privacy SDK Cargo invocation attempted to override the selected lockfile" >&2
      exit 1
      ;;
  esac
done

arguments=()
inserted_lock=0
has_locked=0
has_unstable_options=0
previous_argument=""
for argument in "$@"; do
  if [[ "${argument}" == "--locked" || "${argument}" == "--frozen" ]]; then
    has_locked=1
  fi
  if [[ "${previous_argument}" == "-Z" && "${argument}" == "unstable-options" ]]; then
    has_unstable_options=1
  fi
  if [[ "${argument}" == "--" && "${inserted_lock}" == "0" ]]; then
    arguments+=(--lockfile-path "${SELECTED_LOCKFILE}")
    if [[ "${has_locked}" == "0" ]]; then
      arguments+=(--locked)
    fi
    inserted_lock=1
  fi
  arguments+=("${argument}")
  previous_argument="${argument}"
done
if [[ "${inserted_lock}" == "0" ]]; then
  arguments+=(--lockfile-path "${SELECTED_LOCKFILE}")
  if [[ "${has_locked}" == "0" ]]; then
    arguments+=(--locked)
  fi
fi

printf '%s\n' "${command_name}" >>"${IROHA_PRIVACY_CARGO_AUDIT_PATH}"
if [[ "${has_unstable_options}" == "1" ]]; then
  run_real_cargo_and_verify_locks "${arguments[@]}"
fi
run_real_cargo_and_verify_locks -Z unstable-options "${arguments[@]}"

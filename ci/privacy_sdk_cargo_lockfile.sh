#!/usr/bin/env bash

# Resolve and authenticate the private Cargo lock used by privacy SDK builds.
#
# This file is both a small command-line utility and a sourceable shell library.
# Callers must select exactly one lock with
# IROHA_PRIVACY_CARGO_LOCKFILE_PATH. SDK-specific environment names are outputs
# derived from that authenticated selection, never aliases or fallback inputs.

privacy_sdk_resolve_cargo_lockfile() {
  local repository_root="$1"
  local python_bin="${2:-python3}"

  "${python_bin}" -I - "${repository_root}" <<'PY'
from __future__ import annotations

import hashlib
import os
import re
import stat
import sys
from pathlib import Path
from typing import Optional

PRIMARY_ENV = "IROHA_PRIVACY_CARGO_LOCKFILE_PATH"
MAX_LOCK_BYTES = 16 * 1024 * 1024


def fail(message: str) -> "NoReturn":
    raise SystemExit(f"error: {message}")


def configured_value(name: str) -> Optional[str]:
    if name not in os.environ:
        return None
    value = os.environ[name]
    if not value:
        fail(f"{name} must not be empty")
    if any(ord(character) < 0x20 or ord(character) == 0x7F for character in value):
        fail(f"{name} must not contain control characters")
    return value


root_argument = Path(sys.argv[1])
if not root_argument.is_absolute():
    fail("privacy SDK repository root must be absolute")
try:
    repository_root = root_argument.resolve(strict=True)
except OSError as error:
    fail(f"privacy SDK repository root is unavailable: {error}")
if not repository_root.is_dir():
    fail("privacy SDK repository root must be a directory")

primary = configured_value(PRIMARY_ENV)
if primary is None:
    fail(
        f"{PRIMARY_ENV} is required and must select an external Cargo.lock"
    )

selected_argument = Path(primary)
if not selected_argument.is_absolute():
    fail(f"{PRIMARY_ENV} must name an absolute Cargo.lock path")
try:
    selected = selected_argument.resolve(strict=True)
except OSError as error:
    fail(f"selected privacy SDK Cargo.lock is unavailable: {error}")
if selected != selected_argument:
    fail(
        f"{PRIMARY_ENV} must be canonical and contain no symbolic-link components"
    )

if selected == repository_root or repository_root in selected.parents:
    fail("privacy SDK Cargo.lock must be external to the repository")
if selected.name != "Cargo.lock":
    fail("privacy SDK lockfile path must end in Cargo.lock")

open_flags = os.O_RDONLY
open_flags |= getattr(os, "O_CLOEXEC", 0)
open_flags |= getattr(os, "O_NOFOLLOW", 0)
try:
    descriptor = os.open(selected, open_flags)
except OSError as error:
    fail(f"unable to open selected privacy SDK Cargo.lock: {error}")

try:
    before = os.fstat(descriptor)
    if not stat.S_ISREG(before.st_mode):
        fail("selected privacy SDK Cargo.lock must be a regular file")
    if before.st_mode & 0o111:
        fail("selected privacy SDK Cargo.lock must not be executable")
    if before.st_size <= 0 or before.st_size > MAX_LOCK_BYTES:
        fail(
            "selected privacy SDK Cargo.lock must be non-empty and no larger "
            f"than {MAX_LOCK_BYTES} bytes"
        )
    chunks: list[bytes] = []
    remaining = MAX_LOCK_BYTES + 1
    while remaining:
        chunk = os.read(descriptor, min(1024 * 1024, remaining))
        if not chunk:
            break
        chunks.append(chunk)
        remaining -= len(chunk)
    payload = b"".join(chunks)
    after = os.fstat(descriptor)
finally:
    os.close(descriptor)

stable_before = (
    before.st_dev,
    before.st_ino,
    before.st_mode,
    before.st_nlink,
    before.st_size,
    before.st_mtime_ns,
    before.st_ctime_ns,
)
stable_after = (
    after.st_dev,
    after.st_ino,
    after.st_mode,
    after.st_nlink,
    after.st_size,
    after.st_mtime_ns,
    after.st_ctime_ns,
)
if len(payload) != before.st_size or stable_before != stable_after:
    fail("selected privacy SDK Cargo.lock changed while it was read")
try:
    path_metadata = os.stat(selected, follow_symlinks=False)
except OSError as error:
    fail(f"selected privacy SDK Cargo.lock became unavailable: {error}")
if (path_metadata.st_dev, path_metadata.st_ino) != (before.st_dev, before.st_ino):
    fail("selected privacy SDK Cargo.lock changed identity while it was read")

try:
    lock_text = payload.decode("utf-8", errors="strict")
except UnicodeDecodeError as error:
    fail(f"selected privacy SDK Cargo.lock is not UTF-8: {error}")
if "\x00" in lock_text or "\r" in lock_text or not lock_text.endswith("\n"):
    fail("selected privacy SDK Cargo.lock is not canonical UTF-8 text")
lock_preamble = lock_text.split("[[package]]", 1)[0]
version_rows = re.findall(r"(?m)^version = (.+)$", lock_preamble)
if version_rows != ["4"]:
    fail("selected privacy SDK Cargo.lock must use Cargo lock format version 4")
if "[[package]]\n" not in lock_text:
    fail("selected privacy SDK Cargo.lock must contain at least one package")

# Consume the bytes before printing the canonical path so validation cannot be
# optimized into a path-only check.
hashlib.sha256(payload).digest()
print(selected)
PY
}

privacy_sdk_file_seal() {
  local path="$1"
  local python_bin="${2:-python3}"

  "${python_bin}" -I - "${path}" <<'PY'
from __future__ import annotations

import hashlib
import os
import stat
import sys
from pathlib import Path

MAX_BYTES = 16 * 1024 * 1024
path = Path(sys.argv[1])
flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
try:
    descriptor = os.open(path, flags)
except OSError as error:
    raise SystemExit(f"error: unable to seal {path}: {error}")
try:
    before = os.fstat(descriptor)
    if not stat.S_ISREG(before.st_mode):
        raise SystemExit(f"error: {path} is not a regular file")
    if before.st_size < 0 or before.st_size > MAX_BYTES:
        raise SystemExit(f"error: {path} exceeds the file-seal size limit")
    digest = hashlib.sha256()
    observed = 0
    while chunk := os.read(descriptor, 1024 * 1024):
        observed += len(chunk)
        if observed > MAX_BYTES:
            raise SystemExit(f"error: {path} exceeds the file-seal size limit")
        digest.update(chunk)
    after = os.fstat(descriptor)
finally:
    os.close(descriptor)
stable_before = (
    before.st_dev,
    before.st_ino,
    before.st_mode,
    before.st_nlink,
    before.st_size,
    before.st_mtime_ns,
    before.st_ctime_ns,
)
stable_after = (
    after.st_dev,
    after.st_ino,
    after.st_mode,
    after.st_nlink,
    after.st_size,
    after.st_mtime_ns,
    after.st_ctime_ns,
)
if stable_before != stable_after or observed != before.st_size:
    raise SystemExit(f"error: {path} changed while it was sealed")
print(
    ":".join(
        (
            digest.hexdigest(),
            str(before.st_dev),
            str(before.st_ino),
            str(before.st_size),
            str(before.st_mtime_ns),
            str(before.st_ctime_ns),
            oct(stat.S_IMODE(before.st_mode)),
        )
    )
)
PY
}

privacy_sdk_assert_file_seal() {
  local path="$1"
  local expected_seal="$2"
  local label="$3"
  local python_bin="${4:-python3}"
  local observed_seal

  if ! observed_seal="$(privacy_sdk_file_seal "${path}" "${python_bin}")"; then
    echo "error: ${label} became unreadable" >&2
    return 1
  fi
  if [[ "${observed_seal}" != "${expected_seal}" ]]; then
    echo "error: ${label} changed while the privacy SDK guard was running" >&2
    return 1
  fi
}

privacy_sdk_capture_optional_file_state() {
  local path="$1"
  local label="$2"
  local python_bin="${3:-python3}"
  local seal

  if [[ -L "${path}" ]]; then
    echo "error: ${label} must not be a symbolic link" >&2
    return 1
  fi
  if [[ ! -e "${path}" ]]; then
    printf '%s\n' "absent"
    return 0
  fi
  if ! seal="$(privacy_sdk_file_seal "${path}" "${python_bin}")"; then
    echo "error: ${label} must be a readable regular file when present" >&2
    return 1
  fi
  printf 'present:%s\n' "${seal}"
}

privacy_sdk_assert_optional_file_state() {
  local path="$1"
  local expected_state="$2"
  local label="$3"
  local python_bin="${4:-python3}"

  case "${expected_state}" in
    absent)
      if [[ -e "${path}" || -L "${path}" ]]; then
        echo "error: ${label} was created while the privacy SDK guard was running" >&2
        return 1
      fi
      ;;
    present:*)
      privacy_sdk_assert_file_seal \
        "${path}" "${expected_state#present:}" "${label}" "${python_bin}"
      ;;
    *)
      echo "error: invalid captured state for ${label}" >&2
      return 1
      ;;
  esac
}

privacy_sdk_assert_ci_cargo_lock_state() {
  local repository_root="$1"
  local python_bin="${2:-python3}"
  local resolved_lockfile status=0

  : "${IROHA_PRIVACY_AUTHENTICATED_CARGO_LOCKFILE_PATH:?CI lock verification requires an authenticated Cargo lock path}"
  : "${IROHA_PRIVACY_AUTHENTICATED_CARGO_LOCKFILE_SEAL:?CI lock verification requires an authenticated Cargo lock seal}"
  : "${IROHA_PRIVACY_AUTHENTICATED_WORKSPACE_CARGO_LOCK_STATE:?CI lock verification requires an authenticated workspace lock state}"

  if [[ "${IROHA_PRIVACY_AUTHENTICATED_WORKSPACE_CARGO_LOCK_STATE}" != "absent" ]]; then
    echo "error: privacy SDK CI requires an initially absent workspace Cargo.lock" >&2
    return 1
  fi
  if ! resolved_lockfile="$(
    privacy_sdk_resolve_cargo_lockfile "${repository_root}" "${python_bin}"
  )"; then
    return 1
  fi
  if [[ "${resolved_lockfile}" != "${IROHA_PRIVACY_AUTHENTICATED_CARGO_LOCKFILE_PATH}" ]]; then
    echo "error: privacy SDK CI Cargo lock selection changed" >&2
    return 1
  fi
  privacy_sdk_assert_file_seal \
    "${resolved_lockfile}" \
    "${IROHA_PRIVACY_AUTHENTICATED_CARGO_LOCKFILE_SEAL}" \
    "selected external Cargo.lock" \
    "${python_bin}" || status=1
  privacy_sdk_assert_optional_file_state \
    "${repository_root}/Cargo.lock" \
    "${IROHA_PRIVACY_AUTHENTICATED_WORKSPACE_CARGO_LOCK_STATE}" \
    "workspace Cargo.lock" \
    "${python_bin}" || status=1
  return "${status}"
}

privacy_sdk_provision_ci_cargo_lock() {
  local repository_root="$1"
  local corridor_root="$2"
  local github_env_path="$3"
  local github_path_path="$4"
  local python_bin="${5:-python3}"
  local canonical_repository_root canonical_corridor_root
  local workspace_lock lock_directory lock_path resolved_lock resolved_lock_seal
  local real_cargo cargo_wrapper_directory cargo_audit_path script_directory

  case "${repository_root}" in
    /*) ;;
    *)
      echo "error: privacy SDK CI repository root must be absolute" >&2
      return 1
      ;;
  esac
  case "${corridor_root}" in
    /*) ;;
    *)
      echo "error: privacy SDK CI corridor root must be absolute" >&2
      return 1
      ;;
  esac
  if [[ ! -d "${repository_root}" || -L "${repository_root}" ]]; then
    echo "error: privacy SDK CI repository root must be a real directory" >&2
    return 1
  fi
  if [[ ! -f "${github_env_path}" || -L "${github_env_path}" ]]; then
    echo "error: privacy SDK CI GITHUB_ENV must be an existing regular file" >&2
    return 1
  fi
  if [[ ! -f "${github_path_path}" || -L "${github_path_path}" ]]; then
    echo "error: privacy SDK CI GITHUB_PATH must be an existing regular file" >&2
    return 1
  fi

  canonical_repository_root="$(cd "${repository_root}" && pwd -P)"
  workspace_lock="${canonical_repository_root}/Cargo.lock"
  if [[ -e "${workspace_lock}" || -L "${workspace_lock}" ]]; then
    echo "error: clean privacy SDK CI checkout unexpectedly contains Cargo.lock" >&2
    return 1
  fi

  install -d -m 700 "${corridor_root}"
  canonical_corridor_root="$(cd "${corridor_root}" && pwd -P)"
  case "${canonical_corridor_root}/" in
    "${canonical_repository_root}/"*)
      echo "error: privacy SDK CI Cargo corridor must be external to the repository" >&2
      return 1
      ;;
  esac

  lock_directory="${canonical_corridor_root}/lock"
  cargo_wrapper_directory="${canonical_corridor_root}/bin"
  cargo_audit_path="${canonical_corridor_root}/cargo-audit"
  install -d -m 700 "${lock_directory}" "${cargo_wrapper_directory}"
  lock_path="${lock_directory}/Cargo.lock"
  if [[ -e "${lock_path}" || -L "${lock_path}" ]]; then
    echo "error: privacy SDK CI external Cargo.lock already exists" >&2
    return 1
  fi

  real_cargo="$(command -v cargo || true)"
  case "${real_cargo}" in
    /*) ;;
    *)
      echo "error: privacy SDK CI requires Cargo at an absolute path" >&2
      return 1
      ;;
  esac
  if [[ ! -f "${real_cargo}" || ! -x "${real_cargo}" ]]; then
    echo "error: privacy SDK CI Cargo must be an executable file" >&2
    return 1
  fi

  RUSTC_BOOTSTRAP=1 "${real_cargo}" -Z unstable-options generate-lockfile \
    --manifest-path "${canonical_repository_root}/Cargo.toml" \
    --lockfile-path "${lock_path}"
  if [[ -e "${workspace_lock}" || -L "${workspace_lock}" ]]; then
    echo "error: external lock generation created workspace Cargo.lock" >&2
    return 1
  fi
  if [[ ! -f "${lock_path}" || -L "${lock_path}" ]]; then
    echo "error: Cargo did not generate the required external Cargo.lock" >&2
    return 1
  fi

  chmod 400 "${lock_path}"
  lock_path="$(cd "$(dirname "${lock_path}")" && pwd -P)/Cargo.lock"
  resolved_lock="$(
    IROHA_PRIVACY_CARGO_LOCKFILE_PATH="${lock_path}" \
      privacy_sdk_resolve_cargo_lockfile \
        "${canonical_repository_root}" "${python_bin}"
  )"
  resolved_lock_seal="$(
    privacy_sdk_file_seal "${resolved_lock}" "${python_bin}"
  )"

  script_directory="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
  install -m 600 /dev/null "${cargo_audit_path}"
  ln -s "${script_directory}/privacy_sdk_cargo_wrapper.sh" \
    "${cargo_wrapper_directory}/cargo"

  {
    printf 'IROHA_PRIVACY_CARGO_LOCKFILE_PATH=%s\n' "${resolved_lock}"
    printf 'IROHA_JS_CARGO_LOCKFILE_PATH=%s\n' "${resolved_lock}"
    printf 'IROHA_PRIVACY_AUTHENTICATED_CARGO_LOCKFILE_PATH=%s\n' "${resolved_lock}"
    printf 'IROHA_PRIVACY_AUTHENTICATED_CARGO_LOCKFILE_SEAL=%s\n' "${resolved_lock_seal}"
    printf 'IROHA_PRIVACY_AUTHENTICATED_WORKSPACE_CARGO_LOCK_STATE=absent\n'
    printf 'IROHA_PRIVACY_SDK_ROOT=%s\n' "${canonical_repository_root}"
    printf 'IROHA_PRIVACY_REAL_CARGO=%s\n' "${real_cargo}"
    printf 'IROHA_PRIVACY_CARGO_AUDIT_PATH=%s\n' "${cargo_audit_path}"
    printf 'IROHA_PRIVACY_LOCKFILE_PYTHON_BIN=%s\n' "$(command -v "${python_bin}")"
    printf 'RUSTC_BOOTSTRAP=1\n'
  } >>"${github_env_path}"
  printf '%s\n' "${cargo_wrapper_directory}" >>"${github_path_path}"
  printf '%s\n' "${resolved_lock}"
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  case "${1:-}" in
    resolve)
      [[ $# -eq 2 ]] || {
        echo "usage: $0 resolve <absolute-repository-root>" >&2
        exit 2
      }
      privacy_sdk_resolve_cargo_lockfile "$2"
      ;;
    seal)
      [[ $# -eq 2 ]] || {
        echo "usage: $0 seal <absolute-file-path>" >&2
        exit 2
      }
      privacy_sdk_file_seal "$2"
      ;;
    provision-ci)
      [[ $# -eq 5 || $# -eq 6 ]] || {
        echo "usage: $0 provision-ci <absolute-repository-root> <absolute-corridor-root> <GITHUB_ENV> <GITHUB_PATH> [python]" >&2
        exit 2
      }
      privacy_sdk_provision_ci_cargo_lock "$2" "$3" "$4" "$5" "${6:-python3}"
      ;;
    verify-ci)
      [[ $# -eq 2 || $# -eq 3 ]] || {
        echo "usage: $0 verify-ci <absolute-repository-root> [python]" >&2
        exit 2
      }
      privacy_sdk_assert_ci_cargo_lock_state "$2" "${3:-python3}"
      ;;
    *)
      echo "usage: $0 {resolve <absolute-repository-root>|seal <absolute-file-path>|provision-ci <absolute-repository-root> <absolute-corridor-root> <GITHUB_ENV> <GITHUB_PATH> [python]|verify-ci <absolute-repository-root> [python]}" >&2
      exit 2
      ;;
  esac
fi

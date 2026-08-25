#!/bin/bash
set -euo pipefail
SCRIPT_EXECUTION_SOURCE="${BASH_SOURCE[0]}"
GATE_LAUNCH_FD="${KAGEMUSHA_PRODUCTION_READINESS_GATE_LAUNCH_FD:-}"
GATE_EXECUTION_FD="${KAGEMUSHA_PRODUCTION_READINESS_GATE_EXECUTION_FD:-}"
GATE_LAUNCH_SOURCE_PATH="${KAGEMUSHA_PRODUCTION_READINESS_GATE_SOURCE_PATH:-}"
if [[ -n "${GATE_LAUNCH_FD}" || -n "${GATE_EXECUTION_FD}" \
  || -n "${GATE_LAUNCH_SOURCE_PATH}" ]]; then
  if [[ "${GATE_LAUNCH_FD}" != "8" \
    || "${GATE_EXECUTION_FD}" != "10" \
    || "${SCRIPT_EXECUTION_SOURCE}" != "/dev/fd/${GATE_EXECUTION_FD}" \
    || "${GATE_LAUNCH_SOURCE_PATH}" != /* ]]; then
    echo "Kagemusha readiness rejects an incomplete authenticated gate descriptor launch" >&2
    exit 2
  fi
  SCRIPT_SOURCE_ORIGINAL="${GATE_LAUNCH_SOURCE_PATH}"
else
  SCRIPT_SOURCE_ORIGINAL="${SCRIPT_EXECUTION_SOURCE}"
fi
if [[ -z "${SCRIPT_SOURCE_ORIGINAL}" || -L "${SCRIPT_SOURCE_ORIGINAL}" ]]; then
  echo "Kagemusha readiness rejects missing or symlinked script invocation" >&2
  exit 2
fi
SCRIPT_SOURCE_NORMALIZED="${SCRIPT_SOURCE_ORIGINAL}"
while [[ "${SCRIPT_SOURCE_NORMALIZED}" == ./* ]]; do
  SCRIPT_SOURCE_NORMALIZED="${SCRIPT_SOURCE_NORMALIZED#./}"
done
if [[ -z "${SCRIPT_SOURCE_NORMALIZED}" ]]; then
  echo "Kagemusha readiness script invocation path is empty" >&2
  exit 2
fi
path_has_noncanonical_component() {
  local path_text="${1}"
  local remainder="${path_text#/}"
  local component
  while [[ -n "${remainder}" ]]; do
    if [[ "${remainder}" == */* ]]; then
      component="${remainder%%/*}"
      remainder="${remainder#*/}"
    else
      component="${remainder}"
      remainder=""
    fi
    if [[ -z "${component}" || "${component}" == "." || "${component}" == ".." ]]; then
      return 0
    fi
  done
  return 1
}
if path_has_noncanonical_component "${SCRIPT_SOURCE_NORMALIZED}"; then
  echo "Kagemusha readiness script invocation path is not canonical" >&2
  exit 2
fi
if [[ "${SCRIPT_SOURCE_NORMALIZED}" == /* ]]; then
  SCRIPT_PATH_LEXICAL="${SCRIPT_SOURCE_NORMALIZED}"
else
  SCRIPT_PATH_LEXICAL="$(builtin pwd -P)/${SCRIPT_SOURCE_NORMALIZED}"
fi
SCRIPT_BASENAME="${SCRIPT_PATH_LEXICAL##*/}"
SCRIPT_DIRECTORY_LEXICAL="${SCRIPT_PATH_LEXICAL%/*}"
SCRIPT_DIRECTORY_PHYSICAL="$(
  builtin cd -P -- "${SCRIPT_DIRECTORY_LEXICAL}" && builtin pwd -P
)"
if [[ "${SCRIPT_DIRECTORY_PHYSICAL}" != "${SCRIPT_DIRECTORY_LEXICAL}" \
  || "${SCRIPT_BASENAME}" != "check_kagemusha_production_readiness.sh" \
  || "${SCRIPT_DIRECTORY_PHYSICAL##*/}" != "ci" ]]; then
  echo "Kagemusha readiness rejects moved or symlink-traversing script invocation" >&2
  exit 2
fi
SCRIPT_PATH="${SCRIPT_DIRECTORY_PHYSICAL}/${SCRIPT_BASENAME}"
if [[ ! -f "${SCRIPT_PATH}" || -L "${SCRIPT_PATH}" \
  || ! "${SCRIPT_SOURCE_ORIGINAL}" -ef "${SCRIPT_PATH}" ]]; then
  echo "Kagemusha readiness script path changed during root derivation" >&2
  exit 2
fi
DERIVED_ROOT_DIR="${SCRIPT_DIRECTORY_PHYSICAL%/*}"
ROOT_DIR="${KAGEMUSHA_PRODUCTION_READINESS_ROOT:-${DERIVED_ROOT_DIR}}"
MODE="candidate"
SELF_TEST="false"
for argument in "$@"; do
  case "${argument}" in
    candidate|promotion) MODE="${argument}" ;;
    --self-test) SELF_TEST="true" ;;
    *)
      echo "usage: ci/check_kagemusha_production_readiness.sh [candidate|promotion] [--self-test]" >&2
      exit 2
      ;;
  esac
done
PYTHON_BIN="${KAGEMUSHA_PRODUCTION_READINESS_PYTHON:-python3}"
PYTHON_SHA256=""
PYTHON_PIN_FD="${KAGEMUSHA_PRODUCTION_READINESS_PYTHON_PIN_FD:--1}"
PYTHON_PATH_FINGERPRINT=""
PYTHON_RUNTIME_PARENT="/private/var/db/iroha-kagemusha-python-runtime-v1"
PYTHON_RUNTIME_ROOT=""
PYTHON_RUNTIME_TREE_SHA256=""
GATE_SHA256=""
GATE_PIN_FD="-1"
GATE_PATH_FINGERPRINT=""
NATIVE_LAUNCH_CONTRACT="${KAGEMUSHA_PRODUCTION_READINESS_NATIVE_LAUNCH_CONTRACT:-}"
NATIVE_MACOS_BUILD="${KAGEMUSHA_PRODUCTION_READINESS_NATIVE_MACOS_BUILD:-}"
NATIVE_OS_TCB_SHA256="${KAGEMUSHA_PRODUCTION_READINESS_NATIVE_OS_TCB_SHA256:-}"
NATIVE_RUNTIME_DEPENDENCY_CONTRACT="${KAGEMUSHA_PRODUCTION_READINESS_NATIVE_RUNTIME_DEPENDENCY_CONTRACT:-}"
promotion_stat_fingerprint() {
  local target="${1}"
  local observed
  if observed="$(/usr/bin/stat -f '%u %Lp %d %i %l %z %m %c' -- "${target}" 2>/dev/null)"; then
    :
  elif observed="$(/usr/bin/stat -c '%u %a %d %i %h %s %Y %Z' -- "${target}" 2>/dev/null)"; then
    :
  else
    return 1
  fi
  printf '%s\n' "${observed}"
}
promotion_assert_no_extended_acl() {
  local target="${1}"
  local label="${2}"
  local listing
  local mode_marker
  local ignored
  if [[ "${OSTYPE}" != darwin* ]]; then
    return 0
  fi
  if [[ ! -x /bin/ls ]]; then
    echo "${label} ACL metadata is unavailable" >&2
    return 1
  fi
  listing="$(LC_ALL=C /bin/ls -lde -- "${target}" 2>/dev/null)" || {
    echo "${label} ACL metadata is unavailable" >&2
    return 1
  }
  read -r mode_marker ignored <<<"${listing}"
  if [[ -z "${mode_marker}" ]]; then
    echo "${label} ACL metadata is malformed" >&2
    return 1
  fi
  if [[ "${mode_marker}" == *+ ]]; then
    echo "${label} has an extended ACL" >&2
    return 1
  fi
  if [[ ! -x /usr/bin/xattr ]]; then
    echo "${label} xattr metadata is unavailable" >&2
    return 1
  fi
  local xattrs
  xattrs="$(/usr/bin/xattr "${target}" 2>/dev/null)" || {
    echo "${label} xattr metadata is unavailable" >&2
    return 1
  }
  if [[ -n "${xattrs}" ]]; then
    echo "${label} has unbound extended attributes" >&2
    return 1
  fi
}
promotion_assert_root_custody() {
  local target="${1}"
  local label="${2}"
  local remainder="${target#/}"
  local component
  local current="/"
  local fingerprint
  local owner_uid
  local mode_text
  local ignored
  local mode_value
  if [[ "${target}" != /* || "${target}" == "/" ]] \
    || path_has_noncanonical_component "${target}"; then
    echo "${label} must be one canonical absolute non-root path" >&2
    return 1
  fi
  fingerprint="$(promotion_stat_fingerprint "/")" || {
    echo "${label} root metadata is unavailable" >&2
    return 1
  }
  read -r owner_uid mode_text ignored <<<"${fingerprint}"
  if [[ "${owner_uid}" != "0" || ! "${mode_text}" =~ ^[0-7]{3,4}$ ]]; then
    echo "${label} filesystem root is not root-owned with a canonical mode" >&2
    return 1
  fi
  mode_value=$((8#${mode_text}))
  if (( mode_value & 0022 )); then
    echo "${label} filesystem root is group/world writable" >&2
    return 1
  fi
  promotion_assert_no_extended_acl "/" "${label} filesystem root" || return 1
  if [[ "$(promotion_stat_fingerprint "/")" != "${fingerprint}" ]]; then
    echo "${label} filesystem root changed during ACL validation" >&2
    return 1
  fi
  while [[ -n "${remainder}" ]]; do
    if [[ "${remainder}" == */* ]]; then
      component="${remainder%%/*}"
      remainder="${remainder#*/}"
    else
      component="${remainder}"
      remainder=""
    fi
    current="${current%/}/${component}"
    if [[ -L "${current}" || ! -e "${current}" ]]; then
      echo "${label} traverses a missing or symlinked path component" >&2
      return 1
    fi
    fingerprint="$(promotion_stat_fingerprint "${current}")" || {
      echo "${label} metadata is unavailable" >&2
      return 1
    }
    read -r owner_uid mode_text ignored <<<"${fingerprint}"
    if [[ "${owner_uid}" != "0" || ! "${mode_text}" =~ ^[0-7]{3,4}$ ]]; then
      echo "${label} path component is not root-owned with a canonical mode" >&2
      return 1
    fi
    mode_value=$((8#${mode_text}))
    if (( mode_value & 0022 )); then
      echo "${label} path component is group/world writable" >&2
      return 1
    fi
    promotion_assert_no_extended_acl "${current}" "${label} path component" || return 1
    if [[ "$(promotion_stat_fingerprint "${current}")" != "${fingerprint}" ]]; then
      echo "${label} path component changed during ACL validation" >&2
      return 1
    fi
  done
}
promotion_root_tree_sha256() {
  # Hash one fully root-custodied, ACL/xattr-free regular-file tree before any
  # byte from that tree can enter Python. Records are NUL-framed and `find -s`
  # supplies bytewise path order on the qualified macOS production host.
  local target="${1}"
  local label="${2}"
  local observed
  if [[ "${OSTYPE}" != darwin* || ! -x /usr/bin/find \
    || ! -x /usr/bin/shasum ]]; then
    echo "${label} tree verifier is unavailable" >&2
    return 1
  fi
  if [[ ! -d "${target}" || -L "${target}" ]]; then
    echo "${label} must be one real directory" >&2
    return 1
  fi
  promotion_assert_root_custody "${target}" "${label}" || return 1
  observed="$(
    LC_ALL=C /usr/bin/find -s "${target}" -xdev -print0 \
      | (
          record_count=0
          total_bytes=0
          while IFS= read -r -d '' current; do
            record_count=$((record_count + 1))
            if (( record_count > 250000 )); then
              echo "${label} has too many records" >&2
              exit 1
            fi
            if [[ "${current}" == "${target}" ]]; then
              relative="."
            elif [[ "${current}" == "${target}"/* ]]; then
              relative="${current#"${target}"/}"
            else
              echo "${label} traversal escaped its root" >&2
              exit 1
            fi
            if (( ${#relative} > 4096 )); then
              echo "${label} contains an oversized path" >&2
              exit 1
            fi
            before="$(promotion_stat_fingerprint "${current}")" || {
              echo "${label} entry metadata is unavailable" >&2
              exit 1
            }
            read -r owner_uid mode_text device inode links size mtime ctime \
              <<<"${before}"
            if [[ "${owner_uid}" != "0" || ! "${mode_text}" =~ ^[0-7]{3,4}$ \
              || ! "${links}" =~ ^[0-9]+$ || ! "${size}" =~ ^[0-9]+$ ]]; then
              echo "${label} entry metadata is malformed" >&2
              exit 1
            fi
            mode_value=$((8#${mode_text}))
            if (( mode_value & 0022 )); then
              echo "${label} contains a group/world-writable entry" >&2
              exit 1
            fi
            promotion_assert_no_extended_acl "${current}" "${label} entry" \
              || exit 1
            if [[ -L "${current}" ]]; then
              echo "${label} contains a symbolic link" >&2
              exit 1
            elif [[ -d "${current}" ]]; then
              kind="directory"
              content_sha256="-"
              content_size="0"
            elif [[ -f "${current}" ]]; then
              if [[ "${links}" != "1" || ! -r "${current}" \
                || ! "${size}" =~ ^[0-9]+$ ]] \
                || (( size > 1073741824 )); then
                echo "${label} contains an unsafe regular file" >&2
                exit 1
              fi
              total_bytes=$((total_bytes + size))
              if (( total_bytes > 8589934592 )); then
                echo "${label} exceeds its file-byte bound" >&2
                exit 1
              fi
              content_sha256="$(
                /usr/bin/shasum -a 256 -- "${current}"
              )" || exit 1
              content_sha256="${content_sha256%% *}"
              if [[ ! "${content_sha256}" =~ ^[0-9a-f]{64}$ ]]; then
                echo "${label} file digest is malformed" >&2
                exit 1
              fi
              kind="file"
              content_size="${size}"
            else
              echo "${label} contains a special file" >&2
              exit 1
            fi
            after="$(promotion_stat_fingerprint "${current}")" || exit 1
            if [[ "${after}" != "${before}" ]]; then
              echo "${label} entry changed while it was hashed" >&2
              exit 1
            fi
            printf '%s\0%s\0%s\0%s\0%s\0%s\0' \
              "${kind}" "${relative}" "${mode_text}" "${owner_uid}" \
              "${content_size}" "${content_sha256}"
          done
          if (( record_count == 0 )); then
            echo "${label} tree is empty" >&2
            exit 1
          fi
        ) \
      | /usr/bin/shasum -a 256
  )" || return 1
  observed="${observed%% *}"
  if [[ ! "${observed}" =~ ^[0-9a-f]{64}$ \
    || "${observed}" == "$(printf '0%.0s' {1..64})" ]]; then
    echo "${label} tree digest is malformed" >&2
    return 1
  fi
  printf '%s\n' "${observed}"
}
if [[ "${MODE}" == "promotion" ]]; then
  if [[ -n "${KAGEMUSHA_PRODUCTION_READINESS_ROOT+x}" ]]; then
    echo "promotion rejects KAGEMUSHA_PRODUCTION_READINESS_ROOT; run the checked-in gate in place" >&2
    exit 2
  fi
  # Bootstrap contract: an independently authenticated controller must install
  # the reviewed checkout below this root-owned, ACL-free, non-group/world-writable path
  # and verify the reviewed gate digest before invoking this exact path.  The
  # digest environment value repeats that launcher's decision; it cannot replace
  # the external pre-exec check because a substituted script could omit its own
  # checks or simply exit zero.
  promotion_assert_root_custody "${DERIVED_ROOT_DIR}" "promotion readiness checkout" || exit 2
  promotion_assert_root_custody "${SCRIPT_PATH}" "promotion readiness gate" || exit 2
  if (( EUID != 0 )); then
    echo "production promotion must enter the digest-pinned interpreter as root" >&2
    exit 2
  fi
  if [[ "${GATE_LAUNCH_FD}" != "8" \
    || "${GATE_EXECUTION_FD}" != "10" \
    || "${PYTHON_PIN_FD}" != "9" \
    || "${NATIVE_LAUNCH_CONTRACT}" \
      != "iroha.kagemusha.native-python-readiness-launch.v1" \
    || "${NATIVE_RUNTIME_DEPENDENCY_CONTRACT}" \
      != "iroha.kagemusha.symlink-free-macho-runtime-closure.v1" \
    || ! "${NATIVE_MACOS_BUILD}" =~ ^[A-Za-z0-9._-]{1,64}$ \
    || ! "${NATIVE_OS_TCB_SHA256}" =~ ^[0-9a-f]{64}$ \
    || "${NATIVE_OS_TCB_SHA256}" == "$(printf '0%.0s' {1..64})" ]]; then
    echo "production promotion requires the native machine-bound readiness launcher" >&2
    exit 2
  fi
  GATE_SHA256="${KAGEMUSHA_PRODUCTION_READINESS_GATE_SHA256:-}"
  if [[ ! "${GATE_SHA256}" =~ ^[0-9a-f]{64}$ || "${GATE_SHA256}" == "$(printf '0%.0s' {1..64})" ]]; then
    echo "promotion requires KAGEMUSHA_PRODUCTION_READINESS_GATE_SHA256 from an independently authenticated launcher/controller" >&2
    exit 2
  fi
  if [[ -n "${GATE_LAUNCH_FD}" ]]; then
    GATE_PIN_FD="${GATE_LAUNCH_FD}"
    GATE_PATH_FINGERPRINT="$(promotion_stat_fingerprint "/dev/fd/${GATE_PIN_FD}")" || {
      echo "promotion readiness gate descriptor metadata is unavailable" >&2
      exit 2
    }
  else
    GATE_PATH_FINGERPRINT="$(promotion_stat_fingerprint "${SCRIPT_PATH}")" || {
      echo "promotion readiness gate metadata is unavailable" >&2
      exit 2
    }
    exec 8<"${SCRIPT_PATH}"
    GATE_PIN_FD="8"
  fi
  if [[ -x /usr/bin/shasum ]]; then
    OBSERVED_GATE_SHA256="$(/usr/bin/shasum -a 256 -- "/dev/fd/${GATE_PIN_FD}")"
  elif [[ -x /usr/bin/sha256sum ]]; then
    OBSERVED_GATE_SHA256="$(/usr/bin/sha256sum -- "/dev/fd/${GATE_PIN_FD}")"
  else
    echo "promotion requires root-installed /usr/bin/shasum or /usr/bin/sha256sum" >&2
    exit 2
  fi
  OBSERVED_GATE_SHA256="${OBSERVED_GATE_SHA256%% *}"
  if [[ "${OBSERVED_GATE_SHA256}" != "${GATE_SHA256}" ]]; then
    echo "promotion readiness gate differs from its independently reviewed SHA-256" >&2
    exit 2
  fi
  if [[ "$(promotion_stat_fingerprint "/dev/fd/${GATE_PIN_FD}")" != "${GATE_PATH_FINGERPRINT}" ]]; then
    echo "promotion readiness gate descriptor changed during bootstrap" >&2
    exit 2
  fi
  promotion_assert_root_custody "${SCRIPT_PATH}" "promotion readiness gate" || exit 2
  PYTHON_BIN="${KAGEMUSHA_PRODUCTION_READINESS_PYTHON:-}"
  PYTHON_SHA256="${KAGEMUSHA_PRODUCTION_READINESS_PYTHON_SHA256:-}"
  PYTHON_RUNTIME_ROOT="${KAGEMUSHA_PRODUCTION_READINESS_PYTHON_RUNTIME_ROOT:-}"
  PYTHON_RUNTIME_TREE_SHA256="${KAGEMUSHA_PRODUCTION_READINESS_PYTHON_RUNTIME_TREE_SHA256:-}"
  if [[ "${PYTHON_RUNTIME_ROOT%/*}" != "${PYTHON_RUNTIME_PARENT}" \
    || "${PYTHON_RUNTIME_ROOT##*/}" == "" \
    || "${PYTHON_RUNTIME_ROOT##*/}" == "." \
    || "${PYTHON_RUNTIME_ROOT##*/}" == ".." \
    || "${PYTHON_BIN}" != "${PYTHON_RUNTIME_ROOT}/bin/python3" \
    || ! -f "${PYTHON_BIN}" || -L "${PYTHON_BIN}" || ! -x "${PYTHON_BIN}" \
    || ! "${PYTHON_SHA256}" =~ ^[0-9a-f]{64}$ \
    || "${PYTHON_SHA256}" == "$(printf '0%.0s' {1..64})" \
    || ! "${PYTHON_RUNTIME_TREE_SHA256}" =~ ^[0-9a-f]{64}$ \
    || "${PYTHON_RUNTIME_TREE_SHA256}" == "$(printf '0%.0s' {1..64})" ]]; then
    echo "promotion requires one fixed-parent digest-pinned Python runtime closure" >&2
    exit 2
  fi
  promotion_assert_root_custody "${PYTHON_RUNTIME_ROOT}" \
    "promotion Python runtime closure" || exit 2
  OBSERVED_PYTHON_RUNTIME_TREE_SHA256="$(
    promotion_root_tree_sha256 "${PYTHON_RUNTIME_ROOT}" \
      "promotion Python runtime closure"
  )" || exit 2
  if [[ "${OBSERVED_PYTHON_RUNTIME_TREE_SHA256}" \
    != "${PYTHON_RUNTIME_TREE_SHA256}" ]]; then
    echo "promotion Python runtime closure differs from its trusted tree SHA-256" >&2
    exit 2
  fi
  promotion_assert_root_custody "${PYTHON_BIN}" "promotion Python interpreter" || exit 2
  PYTHON_PATH_FINGERPRINT="$(promotion_stat_fingerprint "${PYTHON_BIN}")" || {
    echo "promotion Python interpreter metadata is unavailable" >&2
    exit 2
  }
  if [[ "$(promotion_stat_fingerprint "/dev/fd/${PYTHON_PIN_FD}")" \
    != "${PYTHON_PATH_FINGERPRINT}" ]]; then
    echo "promotion Python inherited descriptor differs from its path" >&2
    exit 2
  fi
  if [[ -x /usr/bin/shasum ]]; then
    OBSERVED_PYTHON_SHA256="$(/usr/bin/shasum -a 256 -- "/dev/fd/${PYTHON_PIN_FD}")"
  elif [[ -x /usr/bin/sha256sum ]]; then
    OBSERVED_PYTHON_SHA256="$(/usr/bin/sha256sum -- "/dev/fd/${PYTHON_PIN_FD}")"
  else
    echo "promotion requires root-installed /usr/bin/shasum or /usr/bin/sha256sum" >&2
    exit 2
  fi
  OBSERVED_PYTHON_SHA256="${OBSERVED_PYTHON_SHA256%% *}"
  if [[ "${OBSERVED_PYTHON_SHA256}" != "${PYTHON_SHA256}" ]]; then
    echo "promotion Python interpreter differs from its trusted SHA-256" >&2
    exit 2
  fi
  if [[ "$(promotion_stat_fingerprint "${PYTHON_BIN}")" != "${PYTHON_PATH_FINGERPRINT}" ]]; then
    echo "promotion Python interpreter changed before execution" >&2
    exit 2
  fi
  promotion_assert_root_custody "${PYTHON_BIN}" "promotion Python interpreter" || exit 2
fi
kagemusha_isolated_git() {
  /usr/bin/env -i \
    HOME=/var/empty \
    LANG=C \
    LC_ALL=C \
    PATH=/usr/bin:/bin \
    GIT_CONFIG_COUNT=0 \
    GIT_CONFIG_GLOBAL=/dev/null \
    GIT_CONFIG_NOSYSTEM=1 \
    GIT_LITERAL_PATHSPECS=1 \
    GIT_NO_REPLACE_OBJECTS=1 \
    GIT_OPTIONAL_LOCKS=0 \
    GIT_PAGER=cat \
    GIT_TERMINAL_PROMPT=0 \
    /usr/bin/git -C "${ROOT_DIR}" --work-tree="${ROOT_DIR}" \
      -c core.attributesFile=/dev/null \
      -c core.excludesFile=/dev/null \
      -c core.fsmonitor=false \
      -c core.hooksPath=/dev/null \
      -c core.preloadIndex=false \
      -c core.untrackedCache=false \
      -c submodule.recurse=false \
      "$@"
}
# A stale or hostile local core.worktree redirect can make `git -C` inspect a
# different checkout while this gate reads source files from ROOT_DIR.  Reject
# the redirect and also pin every subsequent Git command with --work-tree so a
# caller-controlled environment cannot split those two views.
if CONFIGURED_CORE_WORKTREE="$(
  kagemusha_isolated_git config --get-all core.worktree 2>/dev/null
)"; then
  if [[ "${CONFIGURED_CORE_WORKTREE}" != "${ROOT_DIR}" ]]; then
    echo "Kagemusha readiness rejects a Git core.worktree redirect outside the repository root" >&2
    exit 2
  fi
else
  CONFIGURED_CORE_WORKTREE_STATUS=$?
  if [[ "${CONFIGURED_CORE_WORKTREE_STATUS}" -ne 1 ]]; then
    echo "Kagemusha readiness could not inspect the local Git worktree configuration" >&2
    exit 2
  fi
fi
if kagemusha_isolated_git diff --cached --quiet --no-ext-diff --no-textconv --diff-filter=U --; then
  :
else
  INDEX_STATUS=$?
  if [[ "${INDEX_STATUS}" -eq 1 ]]; then
    echo "Kagemusha readiness rejects unresolved Git index entries" >&2
  else
    echo "Kagemusha readiness could not inspect the Git index" >&2
  fi
  exit 2
fi
# Candidate and promotion use Python 3.10 features. Reject an older ambient
# interpreter before the embedded gate can fail with a misleading traceback.
if [[ "${MODE}" == "promotion" ]]; then
  if [[ "$(promotion_root_tree_sha256 "${PYTHON_RUNTIME_ROOT}" \
      "promotion Python runtime closure")" \
    != "${PYTHON_RUNTIME_TREE_SHA256}" ]]; then
    echo "promotion Python runtime closure changed before interpreter execution" >&2
    exit 2
  fi
fi
if ! "${PYTHON_BIN}" -I -S -c 'import sys; raise SystemExit(0 if sys.version_info >= (3, 10) else 1)'; then
  echo "Kagemusha production readiness requires Python 3.10 or newer" >&2
  exit 2
fi
# Portable Bash still enters Python by pathname.  At this point every path
# component and the file are root-owned and non-writable by group/other, so no
# principal outside the documented root trust boundary can win the remaining
# execve lookup. Inherited fd 8 independently pins the gate bytes while fd 10
# is the already-open script input consumed by Bash; keeping those descriptors
# distinct prevents hashing or parser reads from consuming the other's offset.
# Inherited fd 9 binds the running Python image back to the exact pre-exec bytes
# and metadata before any promotion decision is made. The root-only tree digest
# was verified before even the version probe could import its standard library.
set +e
"${PYTHON_BIN}" -I -S - "${ROOT_DIR}" "${MODE}" "${SELF_TEST}" "${PYTHON_SHA256}" \
  "${PYTHON_PIN_FD}" "${PYTHON_PATH_FINGERPRINT}" "${GATE_SHA256}" \
  "${GATE_PIN_FD}" "${GATE_PATH_FINGERPRINT}" "${PYTHON_RUNTIME_ROOT}" \
  "${PYTHON_RUNTIME_TREE_SHA256}" "${NATIVE_LAUNCH_CONTRACT}" \
  "${NATIVE_MACOS_BUILD}" "${NATIVE_OS_TCB_SHA256}" \
  "${NATIVE_RUNTIME_DEPENDENCY_CONTRACT}" <<'PY'
from __future__ import annotations
import ctypes
import errno
import hashlib
import json
import os
import re
import selectors
import signal
import stat
import subprocess
import sys
import tempfile
import time
import types
from collections.abc import Callable
from pathlib import Path
root = Path(sys.argv[1])
mode = sys.argv[2]
self_test = sys.argv[3] == "true"
trusted_python_sha256 = sys.argv[4]
try:
    trusted_python_fd = int(sys.argv[5])
except ValueError:
    trusted_python_fd = -2
trusted_python_path_fingerprint = sys.argv[6]
trusted_gate_sha256 = sys.argv[7]
try:
    trusted_gate_fd = int(sys.argv[8])
except ValueError:
    trusted_gate_fd = -2
trusted_gate_path_fingerprint = sys.argv[9]
trusted_python_runtime_root = Path(sys.argv[10]) if sys.argv[10] else None
trusted_python_runtime_tree_sha256 = sys.argv[11]
trusted_native_launch_contract = sys.argv[12]
trusted_native_macos_build = sys.argv[13]
trusted_native_os_tcb_sha256 = sys.argv[14]
trusted_native_runtime_dependency_contract = sys.argv[15]
READINESS = "ci/check_kagemusha_production_readiness.sh"
READINESS_SOURCE_CONTRACT = (
    "ci/check_kagemusha_production_readiness_source_contract.py"
)
READINESS_SOURCE_SUPPORT = (
    "ci/check_kagemusha_production_readiness_source_support.py"
)
READINESS_RECURSION_SOURCE_CONTRACT = "ci/check_kagemusha_recursion_source_contract.py"
READINESS_LIFECYCLE_SOURCE_CONTRACT = "ci/check_kagemusha_lifecycle_source_contract.py"
READINESS_SOURCE_PROVIDERS = (
    READINESS_SOURCE_SUPPORT,
    READINESS_RECURSION_SOURCE_CONTRACT,
    READINESS_LIFECYCLE_SOURCE_CONTRACT,
    READINESS_SOURCE_CONTRACT,
)
READINESS_SELF_TEST = "ci/check_kagemusha_production_readiness_self_test.py"
BUNDLE = "crates/iroha_core/src/bin/kagemusha_recursive_spend_v4_bundle.rs"
BUNDLE_SOURCE_SEAL_INPUTS = (
    "crates/iroha_core/src/bin/kagemusha_recursive_spend_v4_bundle/"
    "source_seal_build_inputs.rs"
)
ROUTES = "crates/iroha_torii_shared/src/route_catalog.rs"
WORKFLOW = ".github/workflows/pr_kagemusha_payload_bench.yml"
PROMOTION_WORKFLOW = ".github/workflows/promote_kagemusha_v4.yml"
IOS_EVIDENCE_MODULE = "scripts/kagemusha_candidate_ios_evidence.py"
PRODUCTION_IOS_EVIDENCE_MODULE = "scripts/kagemusha_production_ios_evidence.py"
SOURCE_TREE_SEAL = "scripts/kagemusha_source_tree_seal.py"
SOURCE_PROJECTION_PRODUCER_CLOSURE = (
    "scripts/run_kagemusha_source_projection_snapshot.py",
    "scripts/produce_kagemusha_v4_source_seal_projection.py",
    "scripts/build_kagemusha_v4_candidate_bundle.py",
    "scripts/profile_cargo_build.py",
    SOURCE_TREE_SEAL,
    "scripts/formal/run_sumeragi_v2_tlapm_guard.py",
)
SOURCE_GIT = Path("/usr/bin/git")
SOURCE_ALLOWED_SIGNERS_PATH_ENV = "KAGEMUSHA_PRODUCTION_SOURCE_SSH_ALLOWED_SIGNERS_PATH"
SOURCE_ALLOWED_SIGNERS_SHA256_ENV = "KAGEMUSHA_PRODUCTION_SOURCE_SSH_ALLOWED_SIGNERS_SHA256"
SOURCE_REVOCATION_PATH_ENV = "KAGEMUSHA_PRODUCTION_SOURCE_SSH_REVOCATION_PATH"
SOURCE_REVOCATION_SHA256_ENV = "KAGEMUSHA_PRODUCTION_SOURCE_SSH_REVOCATION_SHA256"
SOURCE_SEAL_PROJECTION_PATH_ENV = "KAGEMUSHA_BUILD_AUTHENTICATED_SOURCE_SEAL_PROJECTION"
SOURCE_SEAL_PROJECTION_SHA256_ENV = "KAGEMUSHA_BUILD_AUTHENTICATED_SOURCE_SEAL_PROJECTION_SHA256"
SEALED_BUILD_REPORT_PATH_ENV = "KAGEMUSHA_V4_SEALED_CANDIDATE_BUILD_REPORT_PATH"
SEALED_BUILD_REPORT_SHA256_ENV = "KAGEMUSHA_V4_SEALED_CANDIDATE_BUILD_REPORT_SHA256"
SEALED_BUILD_REPORT_SCHEMA = "iroha.kagemusha.sealed_candidate_double_build_report.v1"
NATIVE_SEALED_BUILD_REPORT_SCHEMA = (
    "iroha.kagemusha.native_sealed_candidate_double_build_report.v2"
)
NATIVE_SEALED_BUILDER_LAUNCH_CONTRACT = (
    "iroha.kagemusha.native-sealed-builder-launch.v1"
)
NATIVE_SEALED_BUILDER_ARGUMENT_CONTRACT = (
    "iroha.kagemusha.sealed-builder-exact-arguments.v1"
)
NATIVE_SEALED_BUILDER_ENVIRONMENT_CONTRACT = (
    "iroha.kagemusha.sealed-builder-exact-environment.v1"
)
NATIVE_SEALED_BUILDER_OS_TCB_CONTRACT = "iroha.kagemusha.macos-os-library-tcb.v1"
NATIVE_SEALED_BUILDER_REPORT_PUBLICATION_CONTRACT = (
    "iroha.kagemusha.native-no-replace-report-publication.v1"
)
NATIVE_SEALED_BUILDER_RUNTIME_DEPENDENCY_CONTRACT = (
    "iroha.kagemusha.symlink-free-macho-runtime-closure.v1"
)
SOURCE_SEAL_VERIFICATION_INPUTS = (
    ("authorization", "KAGEMUSHA_BUILD_SOURCE_SEAL_AUTHORIZATION",
     "KAGEMUSHA_BUILD_SOURCE_SEAL_AUTHORIZATION_SHA256", "controller-authorization.json", 64 * 1024, False),
    ("controller_signature", "KAGEMUSHA_BUILD_SOURCE_SEAL_CONTROLLER_SIGNATURE",
     "KAGEMUSHA_BUILD_SOURCE_SEAL_CONTROLLER_SIGNATURE_SHA256", "controller-authorization.json.sig", 64 * 1024, False),
    ("controller_allowed_signers", "KAGEMUSHA_BUILD_SOURCE_SEAL_CONTROLLER_ALLOWED_SIGNERS",
     "KAGEMUSHA_BUILD_SOURCE_SEAL_CONTROLLER_ALLOWED_SIGNERS_SHA256", "controller-allowed-signers", 64 * 1024, False),
    ("controller_revocation", "KAGEMUSHA_BUILD_SOURCE_SEAL_CONTROLLER_REVOCATION",
     "KAGEMUSHA_BUILD_SOURCE_SEAL_CONTROLLER_REVOCATION_SHA256", "controller-revocation", 16 * 1024 * 1024, True),
    ("execution_policy", "KAGEMUSHA_BUILD_SOURCE_SEAL_EXECUTION_POLICY",
     "KAGEMUSHA_BUILD_SOURCE_SEAL_EXECUTION_POLICY_SHA256", "execution-policy.json", 64 * 1024, False),
    ("raw_unit_graph", "KAGEMUSHA_BUILD_SOURCE_SEAL_RAW_UNIT_GRAPH",
     "KAGEMUSHA_BUILD_SOURCE_SEAL_RAW_UNIT_GRAPH_SHA256", "raw-unit-graph.json", 16 * 1024 * 1024, False),
    ("unit_graph", "KAGEMUSHA_BUILD_SOURCE_SEAL_NORMALIZED_UNIT_GRAPH",
     "KAGEMUSHA_BUILD_SOURCE_SEAL_NORMALIZED_UNIT_GRAPH_SHA256", "normalized-unit-graph.json", 16 * 1024 * 1024, False),
)
SOURCE_SEAL_UNIT_GRAPH_NORMALIZATION = (
    "cargo-unit-graph-v1-package-root-relative-src-path-source-cache-"
    "placeholders-sorted-compact-lf-v1"
)
PROMOTION_STAGING_PARENT = Path(
    "/private/var/db/iroha-kagemusha-readiness-v1"
    if sys.platform == "darwin"
    else "/var/lib/iroha/kagemusha-readiness-v1"
)
ARTIFACTS = (
    "step-eq.params-ipa.krv4",
    "step-eq.proving-key.krv4",
    "step-eq.verifying-key.krv4",
    "step-eq.bootstrap-witness.krv4",
    "step-ep.params-ipa.krv4",
    "step-ep.proving-key.krv4",
    "step-ep.verifying-key.krv4",
    "step-ep.bootstrap-witness.krv4",
)
REPORT_ARTIFACT_PURPOSES = (
    "step_eq_params_ipa",
    "step_eq_proving_key",
    "step_eq_verifying_key",
    "step_eq_bootstrap_witness",
    "step_ep_params_ipa",
    "step_ep_proving_key",
    "step_ep_verifying_key",
    "step_ep_bootstrap_witness",
)
FINAL_METADATA = (
    "topup-finality-roster-v4.norito",
    "manifest.norito",
    "manifest.norito.sha256",
    "manifest.json",
    "release-attestation-v4.norito",
    "physical-device-benchmark.evidence",
    "cryptographic-review.evidence",
    "internal-validation-receipt-v1.norito",
    "recursive-step-two-qualification-v4.norito",
    "promotion-record-v4.norito",
)
MAX_RELEASE_DIRECTORIES = 16
MAX_RELEASE_INVENTORY_ENTRIES = len(ARTIFACTS + FINAL_METADATA)
MAX_MANIFEST_BYTES = 32 * 1024 * 1024
MAX_DIGEST_SIDECAR_BYTES = 65
MAX_RELEASE_ATTESTATION_BYTES = 1024 * 1024
MAX_BENCHMARK_EVIDENCE_BYTES = 16 * 1024 * 1024
MAX_CRYPTOGRAPHIC_REVIEW_BYTES = 1024 * 1024
MAX_INTERNAL_VALIDATION_RECEIPT_BYTES = 1024 * 1024
MAX_QUALIFICATION_RECEIPT_BYTES = 2 * 384 * 1024 + 16 * 1024
MAX_PROMOTION_RECORD_BYTES = 1024 * 1024
MAX_KAGAMI_VERIFIER_BYTES = 512 * 1024 * 1024
MAX_READINESS_GATE_BYTES = 8 * 1024 * 1024
MAX_READINESS_SOURCE_CONTRACT_BYTES = 140 * 1024
MAX_READINESS_SELF_TEST_BYTES = 1024 * 1024
MAX_REVIEWED_SOURCE_CLOSURE_BYTES = 16 * 1024 * 1024
MAX_REVIEWED_HELPER_BYTES = 4 * 1024 * 1024
MAX_SOURCE_ALLOWED_SIGNERS_BYTES = 64 * 1024
MAX_SOURCE_REVOCATION_BYTES = 16 * 1024 * 1024
MAX_SOURCE_SEAL_PROJECTION_BYTES = 16 * 1024
MAX_SEALED_BUILD_REPORT_BYTES = 1024 * 1024
KAGAMI_VERIFIER_TIMEOUT_SECONDS = 300.0
KAGAMI_VERIFIER_TERMINATE_GRACE_SECONDS = 0.25
KAGAMI_VERIFIER_REAP_TIMEOUT_SECONDS = 2.0
KAGAMI_VERIFIER_EXIT_POLL_SECONDS = 0.01
MAX_KAGAMI_VERIFIER_STDOUT_BYTES = 1024 * 1024
MAX_KAGAMI_VERIFIER_STDERR_BYTES = 256 * 1024
VERIFIER_IO_CHUNK_BYTES = 64 * 1024
WAITID_SIGINFO_BUFFER_BYTES = 256
MAX_DECLARED_ARTIFACT_FILE_BYTES = 5 * 1024 * 1024 * 1024
MAX_DECLARED_ARTIFACT_AGGREGATE_BYTES = 10 * 1024 * 1024 * 1024
MAX_CATALOG_AGGREGATE_BYTES = 12 * 1024 * 1024 * 1024
BOUNDED_AUTHENTICATED_METADATA = (
    ("release-attestation-v4.norito", MAX_RELEASE_ATTESTATION_BYTES),
    ("cryptographic-review.evidence", MAX_CRYPTOGRAPHIC_REVIEW_BYTES),
    (
        "internal-validation-receipt-v1.norito",
        MAX_INTERNAL_VALIDATION_RECEIPT_BYTES,
    ),
    ("promotion-record-v4.norito", MAX_PROMOTION_RECORD_BYTES),
)
KAGAMI_VERIFIER_PATH_ENV = "KAGEMUSHA_V4_KAGAMI_BIN"
KAGAMI_VERIFIER_SHA256_ENV = "KAGEMUSHA_V4_KAGAMI_SHA256"
AUTHENTICATED_TOOL_CONTROLLER_PATH_ENV = (
    "KAGEMUSHA_AUTHENTICATED_TOOL_CONTROLLER_BIN"
)
AUTHENTICATED_TOOL_CONTROLLER_SHA256_ENV = (
    "KAGEMUSHA_AUTHENTICATED_TOOL_CONTROLLER_SHA256"
)
AUTHENTICATED_TOOL_CONTROLLER_CONTRACT = "iroha.authenticated-tool-os-isolation.v1"
AUTHENTICATED_TOOL_CONTROLLER_SUBCOMMAND = "run-v1"
SANITIZED_VERIFIER_ENV = {
    "LANG": "C",
    "LC_ALL": "C",
    "PATH": "/usr/bin:/bin",
    "TMPDIR": str(PROMOTION_STAGING_PARENT),
}
READ_CHUNK_BYTES = 1024 * 1024
authenticated_readiness_source_contract_bytes: dict[str, bytes] = {}
authenticated_readiness_self_test_bytes: bytes | None = None
# Promotion trust boundary: the controller-authenticated, root-installed gate,
# its digest-pinned Python interpreter, and the independently reviewed source
# closure are trusted code.
# Every operator-supplied production input is required to live below a
# symlink-free, root-owned path chain that has no macOS ACL and is not writable
# by group or other.
# Root itself remains the production trust principal; this gate does not try to
# defend against a malicious caller that can replace the gate while it runs.
PRODUCTION_TRUSTED_UID = 0
MACOS_ACL_TYPE_EXTENDED = 0x00000100
MACOS_ACL_FIRST_ENTRY = 0
if sys.platform == "darwin":
    MACOS_LIBC = ctypes.CDLL(None, use_errno=True)
    MACOS_LIBC.acl_get_fd_np.argtypes = [ctypes.c_int, ctypes.c_int]
    MACOS_LIBC.acl_get_fd_np.restype = ctypes.c_void_p
    MACOS_LIBC.acl_get_entry.argtypes = [
        ctypes.c_void_p,
        ctypes.c_int,
        ctypes.POINTER(ctypes.c_void_p),
    ]
    MACOS_LIBC.acl_get_entry.restype = ctypes.c_int
    MACOS_LIBC.acl_free.argtypes = [ctypes.c_void_p]
    MACOS_LIBC.acl_free.restype = ctypes.c_int
    MACOS_LIBC.flistxattr.argtypes = [
        ctypes.c_int,
        ctypes.c_char_p,
        ctypes.c_size_t,
        ctypes.c_int,
    ]
    MACOS_LIBC.flistxattr.restype = ctypes.c_ssize_t
else:
    MACOS_LIBC = None
WAITID_LIBC = ctypes.CDLL(None, use_errno=True)
if not hasattr(WAITID_LIBC, "waitid"):
    raise ValueError("authenticated verifier waitid support is unavailable")
WAITID_LIBC.waitid.argtypes = [
    ctypes.c_int,
    ctypes.c_uint,
    ctypes.c_void_p,
    ctypes.c_int,
]
WAITID_LIBC.waitid.restype = ctypes.c_int
REVIEWED_SOURCE_CLOSURE_KEYS = {
    "schema",
    "base_commit",
    "source_commit",
    "source_repo_dirty",
    "source_tree_sha256",
    "tracked_binary_diff_sha256",
    "untracked_file_count",
    "untracked_path_mode_blob_oid_manifest",
    "untracked_path_mode_blob_oid_manifest_sha256",
    "ignored_cargo_lock_size_bytes",
    "ignored_cargo_lock_sha256",
    "combined_source_fingerprint_sha256",
}
SOURCE_SEAL_PROJECTION_KEYS = {
    "build_script_observed",
    "outer_policy",
    "reviewed_source_closure_hex",
    "reviewed_source_closure_sha256",
    "schema",
    "source_authority",
    "source_commit",
    "source_date_epoch",
    "source_repo_dirty",
    "source_tree_sha256",
}
SOURCE_AUTHORITY_KEYS = {
    "commit",
    "commit_object_sha256",
    "commit_object_size",
    "committer_epoch",
    "git_tree",
    "ordered_parents",
    "parent_commit",
    "parent_tree",
    "signature",
}
SOURCE_SIGNATURE_KEYS = {
    "allowed_signers_sha256",
    "mechanism",
    "principal",
    "public_key_sha256",
    "revocation_sha256",
    "signature_namespace",
}
ROUTE_LITERALS = (
    "/v1/offline/readiness",
    "/v1/offline/top-up",
    "/v1/offline/redeem",
    "/v1/offline/operations/{operation_id}",
)
RETIRED_RECURSIVE_LIFECYCLE_TYPES = (
    "KagemushaRecursiveSpendInitRequestV2",
    "KagemushaRecursiveSpendInitResultV2",
    "KagemushaRecursiveSpendTopUpUnsignedV2",
    "KagemushaRecursiveSpendTopUpRequestV2",
    "KagemushaRecursiveSpendTopUpAnchorV2",
    "KagemushaRecursiveSpendAppendInputV2",
    "KagemushaRecursiveSpendSplitIntentBuildRequestV2",
    "KagemushaRecursiveSpendSplitIntentV2",
    "KagemushaRecursiveSpendAppendRequestV2",
    "KagemushaRecursiveSpendRedeemBuildRequestV2",
    "KagemushaRecursiveSpendRedeemBuildResultV2",
    "KagemushaRecursiveSpendRedemptionIntentV2",
    "KagemushaRecursiveSpendRedemptionIntentBuildRequestV2",
    "KagemushaRecursiveSpendPeerSplitTransitionV2",
    "KagemushaRecursiveSpendRedemptionChangeTransitionV2",
    "KagemushaRecursiveSpendPublicStatementV2",
    "KagemushaRecursiveSpendProofV2",
    "KagemushaRecursiveSpendBundleV2",
    "KagemushaRecursiveSpendRedeemChangeBranchV2",
    "KagemushaRecursiveSpendSplitResultV2",
    "KagemushaRecursiveSpendPeerPaymentV2",
    "KagemushaRecursiveSpendTopUpFinalityEvidenceV2",
    "KagemushaRecursiveSpendVerifyRequestV2",
    "KagemushaRecursiveSpendBundleSummaryV2",
    "KagemushaRecursiveSpendVerifyResultV2",
    "KagemushaRecursiveSpendRedeemResultV2",
    "KagemushaRecursiveSpendRedeemUnsignedV2",
    "KagemushaRecursiveSpendRedeemRequestV2",
    "KagemushaRecursiveSpendTransitionV2",
    "KagemushaRecursiveSpendTransitionValuesV2",
    "KagemushaRecursiveSpendTransitionConfigV2",
    "KagemushaRecursiveSpendTransitionCircuitV2",
    "KagemushaRecursiveSpendTransitionEqCircuitV2",
    "KagemushaRecursiveSpendTransitionEpCircuitV2",
    "kagemusha_recursive_spend_transition_instance_columns_v2",
    "KAGEMUSHA_RECURSIVE_SPEND_V2_PUBLIC_INPUTS_SCHEMA",
    "KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V1",
    "KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V2",
    "KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V1",
    "KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V2",
    "KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_PROOF_ENVELOPE_VERSION_V1",
    "KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_PROOF_ENVELOPE_VERSION_V2",
    "KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_IPA_K_V1",
    "KagemushaRecursiveSpendArtifactManifestV3",
    "KagemushaRecursiveSpendPromotedReleaseV3",
    "KagemushaRecursiveSpendArtifactBindingV3",
)
RETIRED_RECURSIVE_V3_MARKERS = (
    "KAGEMUSHA_VERIFIER_ROLE_STEP_EQ_V3",
    "KAGEMUSHA_VERIFIER_ROLE_STEP_EP_V3",
    "KAGEMUSHA_VERIFIER_PURPOSE_STEP_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_VERIFIER_CURVE_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_VERIFIER_CURVE_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_PROOF_ENVELOPE_VERSION_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_VERSION_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_PROMOTED_RELEASE_SCHEMA_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_PROOF_BYTES_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_IPA_K_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_KEY_MAGIC_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PARAMETERS_FILE_NAME_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PROVING_KEY_FILE_NAME_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_VERIFYING_KEY_FILE_NAME_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PARAMETERS_FILE_NAME_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PROVING_KEY_FILE_NAME_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_VERIFYING_KEY_FILE_NAME_V3",
    "is_kagemusha_v3_",
    "V3 artifact release",
)
def read(relative: str, errors: list[str]) -> str:
    path = root / relative
    if not path.is_file():
        errors.append(f"missing corridor file: {relative}")
        return ""
    return path.read_text(encoding="utf-8")
def pin_regular_metadata(
    path: Path,
    label: str,
    *,
    require_single_link: bool = True,
    allow_empty: bool = False,
) -> tuple[int, tuple[int, ...]]:
    """Open and retain one exact regular-file metadata identity.

    Operator-controlled release inputs must be singly linked.  Fixed system
    executables may have several links on sealed operating-system volumes; for
    those callers the root-owned, non-writable path chain is the custody
    boundary and every observed link-count change remains part of the pin.
    """
    before = path.lstat()
    if (
        stat.S_ISLNK(before.st_mode)
        or not stat.S_ISREG(before.st_mode)
        or (require_single_link and before.st_nlink != 1)
        or before.st_size < 0
        or (not allow_empty and before.st_size == 0)
    ):
        qualification = "singly-linked " if require_single_link else ""
        content = "" if allow_empty else "nonempty "
        raise ValueError(f"{label} must be a {content}{qualification}regular file")
    flags = (
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    descriptor = os.open(path, flags)
    try:
        opened = os.fstat(descriptor)
        fingerprint = metadata_fingerprint(before)
        if not os.path.samestat(before, opened) or fingerprint != metadata_fingerprint(opened):
            raise ValueError(f"{label} changed while it was pinned")
        return descriptor, fingerprint
    except BaseException:
        os.close(descriptor)
        raise
def pin_directory_metadata(path: Path, label: str) -> tuple[int, tuple[int, ...]]:
    """Open and retain one exact real-directory metadata identity."""
    before = path.lstat()
    if stat.S_ISLNK(before.st_mode) or not stat.S_ISDIR(before.st_mode):
        raise ValueError(f"{label} must be a non-symlink directory")
    flags = (
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
        | getattr(os, "O_DIRECTORY", 0)
    )
    descriptor = os.open(path, flags)
    try:
        opened = os.fstat(descriptor)
        fingerprint = metadata_fingerprint(before)
        if not os.path.samestat(before, opened) or fingerprint != metadata_fingerprint(opened):
            raise ValueError(f"{label} changed while it was pinned")
        return descriptor, fingerprint
    except BaseException:
        os.close(descriptor)
        raise
def absolute_directory_chain(path: Path) -> list[Path]:
    """Return every directory from the filesystem root through an absolute path."""
    if not path.is_absolute():
        raise ValueError("catalog path must be absolute")
    if any(part in {".", ".."} for part in path.parts[1:]):
        raise ValueError("catalog path must contain only normal absolute components")
    chain = [Path(path.anchor)]
    current = chain[0]
    for part in path.parts[1:]:
        current /= part
        chain.append(current)
    return chain
def metadata_fingerprint(metadata: os.stat_result) -> tuple[int, ...]:
    """Project the complete metadata identity used by every retained descriptor."""
    return (
        metadata.st_dev, metadata.st_ino, metadata.st_nlink,
        metadata.st_mode, metadata.st_size, metadata.st_mtime_ns,
        metadata.st_ctime_ns, metadata.st_uid, metadata.st_gid,
    )
def revalidate_pinned_metadata(
    path: Path, descriptor: int, fingerprint: tuple[int, ...], label: str
) -> None:
    """Prove a retained descriptor and its pathname still name the pinned file."""
    opened = os.fstat(descriptor)
    after_path = path.lstat()
    observed_open = metadata_fingerprint(opened)
    observed_path = metadata_fingerprint(after_path)
    if observed_open != fingerprint or observed_path != fingerprint:
        raise ValueError(f"{label} changed during authenticated catalog verification")
def hash_pinned_descriptor(
    descriptor: int,
    fingerprint: tuple[int, ...],
    maximum_bytes: int,
    label: str,
) -> str:
    """Hash exact bytes through a retained descriptor without reopening its path."""
    size = fingerprint[4]
    if size <= 0 or size > maximum_bytes:
        raise ValueError(f"{label} violates its {maximum_bytes}-byte size limit")
    digest = hashlib.sha256()
    offset = 0
    while offset < size:
        chunk = os.pread(descriptor, min(READ_CHUNK_BYTES, size - offset), offset)
        if not chunk:
            raise ValueError(f"{label} became truncated while it was hashed")
        digest.update(chunk)
        offset += len(chunk)
    if os.fstat(descriptor).st_size != size:
        raise ValueError(f"{label} changed while it was hashed")
    return digest.hexdigest()
def validate_inherited_promotion_python() -> None:
    """Bind the interpreter and every imported stdlib path to the sealed tree."""
    if (
        trusted_python_runtime_root is None
        or not trusted_python_runtime_root.is_absolute()
        or trusted_python_runtime_root.parent
        != Path("/private/var/db/iroha-kagemusha-python-runtime-v1")
        or re.fullmatch(
            r"[0-9a-f]{64}", trusted_python_runtime_tree_sha256
        )
        is None
        or trusted_python_runtime_tree_sha256 == "0" * 64
    ):
        raise ValueError("promotion Python runtime-tree pin is missing or malformed")
    if trusted_python_fd != 9:
        raise ValueError("promotion Python descriptor handoff is missing")
    try:
        opened = os.fstat(trusted_python_fd)
    except OSError as error:
        raise ValueError("promotion Python descriptor handoff is closed") from error
    if (
        not stat.S_ISREG(opened.st_mode)
        or opened.st_uid != PRODUCTION_TRUSTED_UID
        or stat.S_IMODE(opened.st_mode) & 0o022
        or opened.st_nlink < 1
        or not stat.S_IMODE(opened.st_mode) & 0o111
        or opened.st_size <= 0
        or opened.st_size > MAX_KAGAMI_VERIFIER_BYTES
    ):
        raise ValueError("promotion Python descriptor does not have production custody")
    require_no_macos_extended_acl(trusted_python_fd, "inherited promotion Python")
    observed_fingerprint = " ".join(
        (
            str(opened.st_uid),
            format(stat.S_IMODE(opened.st_mode), "o"),
            str(opened.st_dev),
            str(opened.st_ino),
            str(opened.st_nlink),
            str(opened.st_size),
            str(int(opened.st_mtime)),
            str(int(opened.st_ctime)),
        )
    )
    if observed_fingerprint != trusted_python_path_fingerprint:
        raise ValueError("promotion Python descriptor differs from its pre-exec path pin")
    descriptor_fingerprint = metadata_fingerprint(opened)
    if (
        hash_pinned_descriptor(
            trusted_python_fd,
            descriptor_fingerprint,
            MAX_KAGAMI_VERIFIER_BYTES,
            "inherited promotion Python",
        )
        != trusted_python_sha256
    ):
        raise ValueError("inherited promotion Python differs from its trusted SHA-256")
    runtime = Path(sys.executable)
    runtime_metadata = runtime.lstat()
    expected_runtime = trusted_python_runtime_root / "bin" / "python3"
    if (
        runtime != expected_runtime
        or runtime.is_symlink()
        or not os.path.samestat(opened, runtime_metadata)
    ):
        raise ValueError("running promotion Python differs from its inherited descriptor")
    for prefix_name, prefix_text in (
        ("prefix", sys.prefix),
        ("base_prefix", sys.base_prefix),
        ("exec_prefix", sys.exec_prefix),
        ("base_exec_prefix", sys.base_exec_prefix),
    ):
        prefix = Path(prefix_text)
        if not prefix.is_absolute() or not prefix.is_relative_to(
            trusted_python_runtime_root
        ):
            raise ValueError(
                f"promotion Python {prefix_name} escapes the sealed runtime tree"
            )
    for entry in sys.path:
        path = Path(entry)
        if (
            not entry
            or not path.is_absolute()
            or not path.is_relative_to(trusted_python_runtime_root)
        ):
            raise ValueError("promotion Python import path escapes the sealed runtime tree")
    for module_name, module in tuple(sys.modules.items()):
        origin = getattr(module, "__file__", None)
        if origin is None:
            continue
        origin_path = Path(origin)
        if not origin_path.is_absolute() or not origin_path.is_relative_to(
            trusted_python_runtime_root
        ):
            raise ValueError(
                f"imported promotion Python module {module_name!r} escapes the sealed runtime tree"
            )
def validate_inherited_promotion_gate() -> None:
    """Bind execution to the launcher's root-private gate snapshot descriptor."""
    if trusted_gate_fd != 8:
        raise ValueError("promotion gate descriptor handoff is missing")
    try:
        opened = os.fstat(trusted_gate_fd)
    except OSError as error:
        raise ValueError("promotion gate descriptor handoff is closed") from error
    if (
        not stat.S_ISREG(opened.st_mode)
        or opened.st_uid != PRODUCTION_TRUSTED_UID
        or stat.S_IMODE(opened.st_mode) & 0o022
        or opened.st_nlink != 1
        or not stat.S_IMODE(opened.st_mode) & 0o111
        or opened.st_size <= 0
        or opened.st_size > MAX_READINESS_GATE_BYTES
    ):
        raise ValueError("promotion gate descriptor does not have production custody")
    require_no_macos_extended_acl(trusted_gate_fd, "inherited promotion gate")
    observed_fingerprint = " ".join(
        (
            str(opened.st_uid),
            format(stat.S_IMODE(opened.st_mode), "o"),
            str(opened.st_dev),
            str(opened.st_ino),
            str(opened.st_nlink),
            str(opened.st_size),
            str(int(opened.st_mtime)),
            str(int(opened.st_ctime)),
        )
    )
    if observed_fingerprint != trusted_gate_path_fingerprint:
        raise ValueError("promotion gate differs from its pre-exec path pin")
    descriptor_fingerprint = metadata_fingerprint(opened)
    if (
        hash_pinned_descriptor(
            trusted_gate_fd,
            descriptor_fingerprint,
            MAX_READINESS_GATE_BYTES,
            "inherited promotion gate",
        )
        != trusted_gate_sha256
    ):
        raise ValueError("inherited promotion gate differs from its reviewed SHA-256")
    source_gate = root / READINESS
    source_metadata = source_gate.lstat()
    if source_gate.is_symlink() or not stat.S_ISREG(source_metadata.st_mode):
        raise ValueError("promotion source gate is not a regular file")
    source_descriptor, source_fingerprint = pin_regular_metadata(source_gate, "promotion source gate")
    try:
        require_production_root_custody(source_descriptor, "promotion source gate")
        if hash_pinned_descriptor(
            source_descriptor, source_fingerprint, MAX_READINESS_GATE_BYTES, "promotion source gate"
        ) != trusted_gate_sha256:
            raise ValueError("promotion source gate differs from its reviewed SHA-256")
        revalidate_pinned_metadata(source_gate, source_descriptor, source_fingerprint, "promotion source gate")
    finally:
        os.close(source_descriptor)
def read_pinned_descriptor(
    descriptor: int,
    fingerprint: tuple[int, ...],
    maximum_bytes: int,
    label: str,
    *,
    allow_empty: bool = False,
) -> bytes:
    """Read exact bounded bytes through a retained descriptor."""
    size = fingerprint[4]
    if size < 0 or (not allow_empty and size == 0) or size > maximum_bytes:
        raise ValueError(f"{label} violates its {maximum_bytes}-byte size limit")
    chunks: list[bytes] = []
    offset = 0
    while offset < size:
        chunk = os.pread(descriptor, min(READ_CHUNK_BYTES, size - offset), offset)
        if not chunk:
            raise ValueError(f"{label} became truncated while it was read")
        chunks.append(chunk)
        offset += len(chunk)
    if os.fstat(descriptor).st_size != size:
        raise ValueError(f"{label} changed while it was read")
    return b"".join(chunks)
def inspect_pinned_prefix(
    descriptor: int,
    fingerprint: tuple[int, ...],
    expected_bytes: int,
    maximum_bytes: int,
    prefix_bytes: int,
    label: str,
) -> bytes:
    """Inspect a prefix through the descriptor retained for the whole decision."""
    if (
        expected_bytes <= 0
        or expected_bytes > maximum_bytes
        or fingerprint[4] != expected_bytes
        or prefix_bytes <= 0
        or prefix_bytes > expected_bytes
    ):
        raise ValueError(f"{label} does not match its bounded declared size")
    prefix = os.pread(descriptor, prefix_bytes, 0)
    if len(prefix) != prefix_bytes or os.fstat(descriptor).st_size != expected_bytes:
        raise ValueError(f"{label} changed while it was inspected")
    return prefix
def require_no_macos_extended_acl(descriptor: int, label: str) -> None:
    """Reject every macOS ACL/xattr on an exact pinned descriptor."""
    if MACOS_LIBC is None:
        return
    ctypes.set_errno(0)
    acl = MACOS_LIBC.acl_get_fd_np(descriptor, MACOS_ACL_TYPE_EXTENDED)
    if not acl:
        error_number = ctypes.get_errno()
        if error_number != errno.ENOENT:
            raise OSError(
                error_number,
                f"could not inspect the extended ACL on {label}",
            )
    else:
        entry = ctypes.c_void_p()
        ctypes.set_errno(0)
        entry_status = MACOS_LIBC.acl_get_entry(
            acl, MACOS_ACL_FIRST_ENTRY, ctypes.byref(entry)
        )
        entry_error = ctypes.get_errno()
        ctypes.set_errno(0)
        free_status = MACOS_LIBC.acl_free(acl)
        free_error = ctypes.get_errno()
        if entry_status < 0:
            raise OSError(entry_error, f"could not inspect the extended ACL on {label}")
        if free_status != 0:
            raise OSError(free_error, f"could not release the extended ACL for {label}")
        if entry_status == 0:
            raise ValueError(f"{label} must not have an extended ACL")
    ctypes.set_errno(0)
    xattr_size = MACOS_LIBC.flistxattr(descriptor, None, 0, 0)
    if xattr_size < 0:
        raise OSError(
            ctypes.get_errno(), f"could not inspect extended attributes on {label}"
        )
    if xattr_size != 0:
        raise ValueError(f"{label} must not have unbound extended attributes")
def require_production_root_custody(descriptor: int, label: str) -> None:
    """Enforce root ownership without mode-bit or macOS ACL write delegation."""
    metadata = os.fstat(descriptor)
    if metadata.st_uid != PRODUCTION_TRUSTED_UID:
        raise ValueError(f"{label} must be owned by root")
    if stat.S_IMODE(metadata.st_mode) & 0o022:
        raise ValueError(f"{label} must not be group/world writable")
    require_no_macos_extended_acl(descriptor, label)


def snapshot_private_bytes(
    payload: bytes,
    file_name: str,
    label: str,
    staging_parent: Path,
    *,
    allow_empty: bool = False,
) -> tuple[tempfile.TemporaryDirectory[str], Path]:
    """Put already-pinned bounded bytes in one owner-private validation slot."""
    if not payload and not allow_empty:
        raise ValueError(f"{label} snapshot payload is empty")
    temporary = tempfile.TemporaryDirectory(
        prefix="kagemusha-pinned-input-", dir=staging_parent
    )
    temporary_path = Path(temporary.name).resolve(strict=True)
    os.chmod(temporary_path, 0o700)
    temporary_metadata = temporary_path.lstat()
    if (
        not stat.S_ISDIR(temporary_metadata.st_mode)
        or temporary_metadata.st_uid != os.geteuid()
        or stat.S_IMODE(temporary_metadata.st_mode) != 0o700
    ):
        temporary.cleanup()
        raise ValueError(f"{label} snapshot directory is not owner-private")
    target = temporary_path / file_name
    descriptor = os.open(
        target,
        os.O_RDWR
        | os.O_CREAT
        | os.O_EXCL
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0),
        0o600,
    )
    try:
        view = memoryview(payload)
        while view:
            written = os.write(descriptor, view)
            if written <= 0:
                raise OSError(f"could not snapshot {label}")
            view = view[written:]
        os.fchmod(descriptor, 0o600)
        os.fsync(descriptor)
        observed = bytearray()
        offset = 0
        while offset < len(payload):
            chunk = os.pread(
                descriptor, min(READ_CHUNK_BYTES, len(payload) - offset), offset
            )
            if not chunk:
                raise ValueError(f"{label} snapshot became truncated")
            observed.extend(chunk)
            offset += len(chunk)
        opened = os.fstat(descriptor)
        path_metadata = target.lstat()
        if (
            bytes(observed) != payload
            or opened.st_size != len(payload)
            or not os.path.samestat(opened, path_metadata)
            or opened.st_uid != os.geteuid()
            or stat.S_IMODE(opened.st_mode) != 0o600
        ):
            raise ValueError(f"{label} snapshot bytes changed")
    except BaseException:
        os.close(descriptor)
        temporary.cleanup()
        raise
    os.close(descriptor)
    return temporary, target
def snapshot_private_python_package(
    payloads: dict[str, bytes],
    label: str,
    staging_parent: Path,
) -> tuple[tempfile.TemporaryDirectory[str], Path]:
    """Materialize one exact owner-private Python import closure."""
    if not payloads or set(payloads) != set(SOURCE_PROJECTION_PRODUCER_CLOSURE):
        raise ValueError(f"{label} inventory is not exact")
    temporary = tempfile.TemporaryDirectory(
        prefix="kagemusha-pinned-python-package-", dir=staging_parent
    )
    package_root = Path(temporary.name).resolve(strict=True)
    os.chmod(package_root, 0o700)
    try:
        for relative, payload in sorted(payloads.items()):
            if (
                not payload
                or relative.startswith("/")
                or "\\" in relative
                or any(component in {"", ".", ".."} for component in relative.split("/"))
            ):
                raise ValueError(f"{label} contains an unsafe package member")
            target = package_root / relative
            target.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
            descriptor = os.open(
                target,
                os.O_RDWR
                | os.O_CREAT
                | os.O_EXCL
                | getattr(os, "O_CLOEXEC", 0)
                | getattr(os, "O_NOFOLLOW", 0),
                0o600,
            )
            try:
                view = memoryview(payload)
                while view:
                    written = os.write(descriptor, view)
                    if written <= 0:
                        raise OSError(f"could not snapshot {label} member {relative}")
                    view = view[written:]
                os.fchmod(descriptor, 0o400)
                os.fsync(descriptor)
                os.lseek(descriptor, 0, os.SEEK_SET)
                chunks: list[bytes] = []
                while True:
                    chunk = os.read(descriptor, READ_CHUNK_BYTES)
                    if not chunk:
                        break
                    chunks.append(chunk)
                observed = b"".join(chunks)
                opened = os.fstat(descriptor)
                path_metadata = target.lstat()
                if (
                    observed != payload
                    or not os.path.samestat(opened, path_metadata)
                    or opened.st_uid != os.geteuid()
                    or stat.S_IMODE(opened.st_mode) != 0o400
                    or opened.st_nlink != 1
                ):
                    raise ValueError(f"{label} member changed while snapshotted")
            finally:
                os.close(descriptor)
        for directory in (package_root / "scripts" / "formal", package_root / "scripts"):
            os.chmod(directory, 0o500)
        if (
            package_root.lstat().st_uid != os.geteuid()
            or stat.S_IMODE(package_root.lstat().st_mode) != 0o700
        ):
            raise ValueError(f"{label} root is not owner-private")
    except BaseException:
        temporary.cleanup()
        raise
    return temporary, package_root
def evidence_bytes_are_non_placeholder(payload: bytes) -> bool:
    """Reject bounded signed-evidence slots containing placeholder markers."""
    if len(payload) < 64 or len(payload) > MAX_BENCHMARK_EVIDENCE_BYTES:
        return False
    return (
        re.search(
            rb"(?:placeholder|synthetic|dummy|todo|not[ -]?reviewed)",
            payload,
            flags=re.IGNORECASE,
        )
        is None
    )
def snapshot_pinned_executable(
    descriptor: int,
    fingerprint: tuple[int, ...],
    label: str,
    staging_parent: Path,
) -> tuple[tempfile.TemporaryDirectory[str], Path]:
    """Materialize exact already-hashed executable bytes in an owner-private directory."""
    temporary = tempfile.TemporaryDirectory(
        prefix="kagemusha-kagami-verifier-", dir=staging_parent
    )
    temporary_path = Path(temporary.name).resolve(strict=True)
    os.chmod(temporary_path, 0o700)
    temporary_metadata = temporary_path.lstat()
    if (
        not stat.S_ISDIR(temporary_metadata.st_mode)
        or temporary_metadata.st_uid != os.geteuid()
        or stat.S_IMODE(temporary_metadata.st_mode) != 0o700
    ):
        temporary.cleanup()
        raise ValueError(f"{label} snapshot directory is not owner-private")
    target = temporary_path / "kagami"
    target_descriptor = os.open(
        target,
        os.O_WRONLY
        | os.O_CREAT
        | os.O_EXCL
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0),
        0o500,
    )
    try:
        offset = 0
        size = fingerprint[4]
        while offset < size:
            chunk = os.pread(descriptor, min(READ_CHUNK_BYTES, size - offset), offset)
            if not chunk:
                raise ValueError(f"{label} became truncated while it was snapshotted")
            view = memoryview(chunk)
            while view:
                written = os.write(target_descriptor, view)
                if written <= 0:
                    raise OSError(f"could not snapshot {label}")
                view = view[written:]
            offset += len(chunk)
        os.fchmod(target_descriptor, 0o500)
        os.fsync(target_descriptor)
    except BaseException:
        os.close(target_descriptor)
        temporary.cleanup()
        raise
    os.close(target_descriptor)
    target_metadata = target.lstat()
    if (
        not stat.S_ISREG(target_metadata.st_mode)
        or target_metadata.st_uid != os.geteuid()
        or stat.S_IMODE(target_metadata.st_mode) != 0o500
        or target_metadata.st_size != fingerprint[4]
    ):
        temporary.cleanup()
        raise ValueError(f"{label} snapshot does not have exact private metadata")
    return temporary, target
def canonical_nonzero_sha256(value: object, label: str) -> str:
    """Return one canonical nonzero lowercase SHA-256 string."""
    if (
        not isinstance(value, str)
        or re.fullmatch(r"[0-9a-f]{64}", value) is None
        or value == "0" * 64
    ):
        raise ValueError(f"{label} is not a canonical nonzero SHA-256")
    return value
def checked_declared_artifact_total(declared_artifacts: dict[str, int]) -> int:
    """Validate each exact artifact size and its aggregate release inventory."""
    total = 0
    for name in ARTIFACTS:
        size_bytes = declared_artifacts[name]
        if size_bytes <= 0 or size_bytes > MAX_DECLARED_ARTIFACT_FILE_BYTES:
            raise ValueError(
                f"artifact {name} violates its "
                f"{MAX_DECLARED_ARTIFACT_FILE_BYTES}-byte size limit"
            )
        total += size_bytes
        if total > MAX_DECLARED_ARTIFACT_AGGREGATE_BYTES:
            raise ValueError(
                "declared artifacts exceed the "
                f"{MAX_DECLARED_ARTIFACT_AGGREGATE_BYTES}-byte aggregate limit"
            )
    return total
def checked_catalog_aggregate_total(current: int, release_bytes: int) -> int:
    """Mirror the runtime's non-raiseable whole-catalog byte ceiling."""
    if current < 0 or release_bytes < 0:
        raise ValueError("catalog aggregate byte accounting is negative")
    total = current + release_bytes
    if total > MAX_CATALOG_AGGREGATE_BYTES:
        raise ValueError(
            "artifact catalog exceeds the runtime aggregate byte limit of "
            f"{MAX_CATALOG_AGGREGATE_BYTES}"
        )
    return total
def require(text: str, relative: str, errors: list[str], *needles: str) -> None:
    for needle in needles:
        if needle not in text:
            errors.append(f"{relative}: missing {needle!r}")
def require_pattern(
    text: str,
    relative: str,
    errors: list[str],
    pattern: str,
    description: str,
) -> None:
    if re.search(pattern, text, flags=re.DOTALL) is None:
        errors.append(f"{relative}: missing {description}")
def forbid(text: str, relative: str, errors: list[str], *needles: str) -> None:
    for needle in needles:
        if needle in text:
            errors.append(f"{relative}: retired corridor remains: {needle!r}")
def forbid_merge_conflict_markers(
    text: str, relative: str, errors: list[str]
) -> None:
    """Reject unresolved Git conflict markers in every reviewed source."""
    if re.search(r"(?m)^(?:<<<<<<<(?: .*)?|=======|>>>>>>>(?: .*)?)$", text):
        errors.append(f"{relative}: unresolved Git merge conflict marker")
def strict_json_bytes(payload: bytes, label: str) -> dict[str, object]:
    def object_pairs(pairs: list[tuple[str, object]]) -> dict[str, object]:
        result: dict[str, object] = {}
        for key, value in pairs:
            if key in result:
                raise ValueError(f"duplicate JSON key {key!r}")
            result[key] = value
        return result
    value = json.loads(
        payload.decode("utf-8"),
        object_pairs_hook=object_pairs,
        parse_constant=lambda value: (_ for _ in ()).throw(
            ValueError(f"{label} contains non-finite value {value!r}")
        ),
    )
    if not isinstance(value, dict):
        raise ValueError(f"{label} must be an object")
    return value
def validate_sealed_build_report(payload: bytes) -> dict[str, object]:
    """Authenticate the canonical report and both independent build identities."""
    envelope = strict_json_bytes(payload, "sealed candidate double-build report")
    canonical = (
        json.dumps(envelope, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
        + "\n"
    ).encode("utf-8")
    if canonical != payload or set(envelope) != {
        "builder_report_hex", "builder_report_sha256", "builder_report_size_bytes",
        "native_launch", "schema",
    } or envelope.get("schema") != NATIVE_SEALED_BUILD_REPORT_SCHEMA:
        raise ValueError(
            "sealed candidate double-build report lacks its native V2 envelope"
        )
    builder_report_hex = envelope.get("builder_report_hex")
    builder_report_size = envelope.get("builder_report_size_bytes")
    if (
        not isinstance(builder_report_hex, str)
        or not builder_report_hex
        or len(builder_report_hex) % 2
        or len(builder_report_hex) > 2 * MAX_SEALED_BUILD_REPORT_BYTES
        or re.fullmatch(r"[0-9a-f]+", builder_report_hex) is None
        or type(builder_report_size) is not int
        or not 1 <= builder_report_size <= MAX_SEALED_BUILD_REPORT_BYTES
    ):
        raise ValueError("native sealed-builder payload encoding is malformed")
    builder_report_payload = bytes.fromhex(builder_report_hex)
    if (
        len(builder_report_payload) != builder_report_size
        or hashlib.sha256(builder_report_payload).hexdigest()
        != canonical_nonzero_sha256(
            envelope.get("builder_report_sha256"),
            "native sealed-builder inner payload",
        )
    ):
        raise ValueError("native sealed-builder payload differs from its envelope")
    native_launch = envelope.get("native_launch")
    native_fields = {
        "argument_contract", "argument_sha256", "builder_entrypoint_sha256",
        "contract", "controller_sha256", "environment_contract",
        "environment_sha256", "macos_build", "os_tcb_contract",
        "os_tcb_sha256", "python_interpreter_sha256",
        "python_runtime_tree_sha256", "report_publication_contract",
        "runtime_dependency_contract",
    }
    if (
        not isinstance(native_launch, dict)
        or set(native_launch) != native_fields
        or native_launch.get("contract") != NATIVE_SEALED_BUILDER_LAUNCH_CONTRACT
        or native_launch.get("argument_contract")
        != NATIVE_SEALED_BUILDER_ARGUMENT_CONTRACT
        or native_launch.get("environment_contract")
        != NATIVE_SEALED_BUILDER_ENVIRONMENT_CONTRACT
        or native_launch.get("os_tcb_contract")
        != NATIVE_SEALED_BUILDER_OS_TCB_CONTRACT
        or native_launch.get("report_publication_contract")
        != NATIVE_SEALED_BUILDER_REPORT_PUBLICATION_CONTRACT
        or native_launch.get("runtime_dependency_contract")
        != NATIVE_SEALED_BUILDER_RUNTIME_DEPENDENCY_CONTRACT
        or not isinstance(native_launch.get("macos_build"), str)
        or re.fullmatch(r"[A-Za-z0-9._-]{1,64}", native_launch["macos_build"])
        is None
    ):
        raise ValueError("native sealed-builder launch attestation is not exact")
    for field in (
        "argument_sha256", "builder_entrypoint_sha256", "controller_sha256",
        "environment_sha256", "os_tcb_sha256", "python_interpreter_sha256",
        "python_runtime_tree_sha256",
    ):
        canonical_nonzero_sha256(
            native_launch.get(field), f"native sealed-builder {field}"
        )
    report = strict_json_bytes(
        builder_report_payload, "native sealed-builder inner report"
    )
    if (
        json.dumps(
            report, sort_keys=True, separators=(",", ":"), ensure_ascii=True
        ).encode("utf-8")
        + b"\n"
        != builder_report_payload
    ):
        raise ValueError("native sealed-builder inner report is not canonical")
    exact_keys = {
        "authenticated_source_seal_projection_sha256", "binary_path",
        "binary_sha256", "binary_size_bytes", "build_profile", "builds",
        "byte_equality", "candidate_generator", "minimum_build_physical_memory_bytes",
        "physical_memory_bytes_at_admission", "reproducible_build_count",
        "reviewed_cargo_binary_sha256", "reviewed_rustc_binary_sha256",
        "reviewed_source_closure", "reviewed_source_closure_descriptor_sha256",
        "schema", "source_commit", "source_date_epoch", "source_repo_dirty",
        "source_tree_sha256", "target_dir", "unit_graph_preflight",
        "verification_binary_path",
    }
    builds = report.get("builds")
    if (
        set(report) != exact_keys
        or report.get("schema") != SEALED_BUILD_REPORT_SCHEMA
        or report.get("build_profile") != "release"
        or report.get("reproducible_build_count") != 2
        or report.get("source_repo_dirty") is not False
        or not isinstance(builds, list)
        or len(builds) != 2
    ):
        raise ValueError("sealed candidate double-build report is not canonical and exact")
    common_keys = {
        "authenticated_source_seal_projection_sha256", "build_inputs_sha256",
        "cargo_binary_sha256", "cargo_semantic_argv", "execution_policy_sha256",
        "normalized_unit_graph_sha256", "reviewed_source_closure_sha256",
        "runtime_gid", "runtime_uid", "rustc_binary_sha256", "source_commit",
        "source_date_epoch", "source_tree_sha256", "target",
    }
    outputs: list[tuple[str, int, str]] = []
    identities: list[dict[str, object]] = []
    for ordinal, build in enumerate(builds, 1):
        if not isinstance(build, dict) or set(build) != {"identity", "identity_sha256", "output"}:
            raise ValueError("sealed build entry fields are not exact")
        identity = build.get("identity")
        output = build.get("output")
        identity_bytes = (
            json.dumps(identity, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
            + "\n"
        ).encode("utf-8")
        if (
            not isinstance(identity, dict)
            or set(identity) != common_keys | {"ordinal", "source_snapshot_role", "target_role"}
            or identity.get("ordinal") != ordinal
            or hashlib.sha256(identity_bytes).hexdigest()
                != canonical_nonzero_sha256(build.get("identity_sha256"), "sealed build identity")
            or not isinstance(output, dict)
            or set(output) != {"binary_path", "sha256", "size_bytes"}
            or not isinstance(output.get("binary_path"), str)
            or type(output.get("size_bytes")) is not int
            or output["size_bytes"] <= 0
        ):
            raise ValueError("sealed build identity is invalid")
        outputs.append((
            canonical_nonzero_sha256(output.get("sha256"), "sealed build output"),
            output["size_bytes"], output["binary_path"],
        ))
        identities.append(identity)
    equality = report.get("byte_equality")
    generator = report.get("candidate_generator")
    if (
        outputs[0][:2] != outputs[1][:2]
        or outputs[0][2] == outputs[1][2]
        or any(
            identities[0].get(key) != identities[1].get(key)
            for key in common_keys
        )
        or identities[0].get("source_snapshot_role")
            != "authenticated-primary-source-snapshot-v1"
        or identities[1].get("source_snapshot_role")
            != "authenticated-independent-source-snapshot-v1"
        or identities[0].get("target_role") != "fresh-primary-target-v1"
        or identities[1].get("target_role") != "fresh-verification-target-v1"
        or identities[0].get("cargo_semantic_argv") != [
            "build", "--release", "--locked", "--offline", "--target",
            "aarch64-apple-darwin", "--target-dir", "<EXTERNAL_TARGET_DIR>",
            "-p", "iroha_core", "--features",
            "iroha_core/dev-tools,iroha_core/kagemusha-candidate-source-seal,iroha_core/kagemusha-candidate-evidence-lab",
            "--bin", "kagemusha_recursive_spend_v4_bundle", "--jobs", "1",
            "--message-format=json-render-diagnostics",
        ]
        or identities[0].get("target") != "aarch64-apple-darwin"
        or not isinstance(equality, dict)
        or set(equality) != {"algorithm", "equal", "sha256", "size_bytes"}
        or equality.get("algorithm") != "sha256-size-and-final-descriptor-rehash-v1"
        or equality.get("equal") is not True
        or (equality.get("sha256"), equality.get("size_bytes")) != outputs[0][:2]
        or not isinstance(generator, dict)
        or set(generator) != {"selected_build_ordinal", "sha256", "size_bytes"}
        or generator.get("selected_build_ordinal") != 1
        or (generator.get("sha256"), generator.get("size_bytes")) != outputs[0][:2]
        or (report.get("binary_sha256"), report.get("binary_size_bytes")) != outputs[0][:2]
        or report.get("binary_path") != outputs[0][2]
        or report.get("verification_binary_path") != outputs[1][2]
    ):
        raise ValueError("sealed double-build equality or generator selection differs")
    for report_key, identity_key in (
        ("authenticated_source_seal_projection_sha256", "authenticated_source_seal_projection_sha256"),
        ("reviewed_cargo_binary_sha256", "cargo_binary_sha256"),
        ("reviewed_rustc_binary_sha256", "rustc_binary_sha256"),
        ("reviewed_source_closure_descriptor_sha256", "reviewed_source_closure_sha256"),
        ("source_commit", "source_commit"),
        ("source_date_epoch", "source_date_epoch"),
        ("source_tree_sha256", "source_tree_sha256"),
    ):
        if report.get(report_key) != identities[0].get(identity_key):
            raise ValueError(f"sealed build report {report_key} is not build-bound")
    report["native_launch_attestation"] = native_launch
    return report
def validate_native_build_launch_binding(
    report: dict[str, object],
    controller_sha256: str,
    python_sha256: str,
    python_runtime_tree_sha256: str,
    macos_build: str,
    os_tcb_sha256: str,
) -> dict[str, object]:
    """Bind a parsed V2 build envelope to the live readiness trust roots."""
    native_build_launch = report.get("native_launch_attestation")
    if (
        not isinstance(native_build_launch, dict)
        or native_build_launch.get("controller_sha256") != controller_sha256
        or native_build_launch.get("python_interpreter_sha256") != python_sha256
        or native_build_launch.get("python_runtime_tree_sha256")
        != python_runtime_tree_sha256
        or native_build_launch.get("macos_build") != macos_build
        or native_build_launch.get("os_tcb_sha256") != os_tcb_sha256
    ):
        raise ValueError(
            "native sealed-builder launch differs from the authenticated "
            "readiness controller, Python closure, or macOS TCB"
        )
    return native_build_launch
def validate_native_builder_entrypoint_binding(
    report: dict[str, object], builder_bytes: bytes
) -> None:
    """Bind the descriptor-executed builder to its reviewed signed source."""
    native_build_launch = report.get("native_launch_attestation")
    if (
        not isinstance(native_build_launch, dict)
        or native_build_launch.get("builder_entrypoint_sha256")
        != hashlib.sha256(builder_bytes).hexdigest()
    ):
        raise ValueError(
            "native sealed-builder entrypoint differs from the reviewed "
            "signed source closure"
        )
def reviewed_source_commit_from_closure(payload: bytes) -> tuple[dict[str, object], str]:
    """Parse the independently pinned closure enough to authenticate its helpers."""
    closure = strict_json_bytes(payload, "reviewed source closure")
    if set(closure) != REVIEWED_SOURCE_CLOSURE_KEYS:
        raise ValueError("reviewed source closure fields are not exact")
    if (
        closure.get("schema") != "iroha.reviewed-source-closure.v1"
        or closure.get("source_repo_dirty") is not False
    ):
        raise ValueError("reviewed source closure is not one clean V1 closure")
    source_commit = closure.get("source_commit")
    if (
        not isinstance(source_commit, str)
        or re.fullmatch(r"[0-9a-f]{40}", source_commit) is None
        or source_commit == "0" * 40
    ):
        raise ValueError("reviewed source closure commit is not canonical")
    return closure, source_commit
def validate_source_trust_projection(
    payload: bytes,
    reviewed_closure_bytes: bytes,
    reviewed_closure_sha256: str,
    source_commit: str,
    allowed_signers_sha256: str,
    revocation_sha256: str,
) -> dict[str, object]:
    """Bind source trust, graph capture, and build-tool identities for promotion."""
    projection = strict_json_bytes(payload, "authenticated source-seal projection")
    canonical_projection = (
        json.dumps(projection, ensure_ascii=True, separators=(",", ":"), sort_keys=True)
        + "\n"
    ).encode("ascii")
    if canonical_projection != payload:
        raise ValueError("authenticated source-seal projection is not canonical JSON")
    if set(projection) != SOURCE_SEAL_PROJECTION_KEYS:
        raise ValueError("authenticated source-seal projection fields are not exact")
    if (
        projection.get("schema")
        != "iroha.kagemusha.authenticated_source_seal_projection.v1"
        or projection.get("source_repo_dirty") is not False
        or projection.get("source_commit") != source_commit
        or projection.get("reviewed_source_closure_sha256")
        != reviewed_closure_sha256
        or projection.get("reviewed_source_closure_hex")
        != reviewed_closure_bytes.hex()
    ):
        raise ValueError(
            "authenticated source-seal projection differs from the reviewed closure"
        )
    reviewed_closure, _ = reviewed_source_commit_from_closure(
        reviewed_closure_bytes
    )
    if projection.get("source_tree_sha256") != reviewed_closure.get(
        "source_tree_sha256"
    ):
        raise ValueError(
            "authenticated source-seal projection tree differs from the reviewed closure"
        )
    authority = projection.get("source_authority")
    if not isinstance(authority, dict) or set(authority) != SOURCE_AUTHORITY_KEYS:
        raise ValueError("authenticated source authority fields are not exact")
    signature = authority.get("signature")
    if not isinstance(signature, dict) or set(signature) != SOURCE_SIGNATURE_KEYS:
        raise ValueError("authenticated source signature fields are not exact")
    if (
        authority.get("commit") != source_commit
        or signature.get("mechanism") != "git-commit-ssh-signature-v1"
        or signature.get("signature_namespace") != "git"
        or signature.get("allowed_signers_sha256") != allowed_signers_sha256
        or signature.get("revocation_sha256") != revocation_sha256
    ):
        raise ValueError(
            "promotion SSH trust-policy digests differ from the reviewed source closure projection"
        )
    canonical_nonzero_sha256(
        signature.get("public_key_sha256"),
        "authenticated source SSH public key",
    )
    outer = projection.get("outer_policy")
    if not isinstance(outer, dict) or set(outer) != {
        "build_inputs_hex",
        "build_inputs_sha256",
        "cargo",
        "execution_policy_sha256",
        "schema",
        "toolchain",
    }:
        raise ValueError("authenticated source outer-policy fields are not exact")
    if outer.get("schema") != "iroha.kagemusha.cprime_source_seal_outer_policy.v1":
        raise ValueError("authenticated source outer-policy schema differs")
    execution_policy_sha256 = canonical_nonzero_sha256(
        outer.get("execution_policy_sha256"),
        "authenticated execution policy",
    )
    build_inputs_sha256 = canonical_nonzero_sha256(
        outer.get("build_inputs_sha256"),
        "authenticated build-input closure",
    )
    build_inputs_hex = outer.get("build_inputs_hex")
    if (
        not isinstance(build_inputs_hex, str)
        or not 2 <= len(build_inputs_hex) <= 16 * 1024
        or len(build_inputs_hex) % 2
        or re.fullmatch(r"[0-9a-f]+", build_inputs_hex) is None
    ):
        raise ValueError("authenticated build-input closure hex is malformed")
    build_inputs_bytes = bytes.fromhex(build_inputs_hex)
    if hashlib.sha256(build_inputs_bytes).hexdigest() != build_inputs_sha256:
        raise ValueError("authenticated build-input closure digest differs")
    build_inputs = strict_json_bytes(
        build_inputs_bytes, "authenticated build-input closure"
    )
    if (
        json.dumps(
            build_inputs, ensure_ascii=True, separators=(",", ":"), sort_keys=True
        ).encode("ascii")
        + b"\n"
        != build_inputs_bytes
    ):
        raise ValueError("authenticated build-input closure is not canonical JSON")
    if set(build_inputs) != {
        "cargo_home",
        "cargo_toolchain",
        "developer_dir",
        "host_tools",
        "platform",
        "python_runtime",
        "runtime_identity",
        "rust_toolchain",
        "sandbox",
        "schema",
        "sdkroot",
    }:
        raise ValueError("authenticated build-input closure fields are not exact")
    if (
        build_inputs.get("schema") != "iroha.kagemusha.build_input_closure.v1"
        or build_inputs.get("platform") != "darwin"
    ):
        raise ValueError("authenticated build-input closure target is not exact")
    developer = build_inputs.get("developer_dir")
    sdkroot = build_inputs.get("sdkroot")
    if (
        not isinstance(developer, dict)
        or set(developer) != {"path", "tree"}
        or developer.get("path")
        != "/private/var/db/kagemusha/Xcode/Developer"
        or not isinstance(sdkroot, dict)
        or set(sdkroot) != {"path", "tree"}
        or sdkroot.get("path")
        != (
            "/private/var/db/kagemusha/Xcode/Developer/Platforms/"
            "MacOSX.platform/Developer/SDKs/MacOSX26.2.sdk"
        )
    ):
        raise ValueError("authenticated build-input Xcode roots are not exact")
    python_runtime = build_inputs.get("python_runtime")
    if (
        not isinstance(python_runtime, dict)
        or set(python_runtime)
        != {"interpreter_path", "interpreter_sha256", "root", "tree_sha256"}
        or trusted_python_runtime_root is None
        or python_runtime.get("root") != str(trusted_python_runtime_root)
        or python_runtime.get("interpreter_path")
        != str(trusted_python_runtime_root / "bin" / "python3")
        or python_runtime.get("interpreter_sha256") != trusted_python_sha256
        or python_runtime.get("tree_sha256")
        != trusted_python_runtime_tree_sha256
    ):
        raise ValueError(
            "signed Python interpreter/stdlib closure differs from the pre-exec runtime pin"
        )
    cargo = outer.get("cargo")
    if not isinstance(cargo, dict) or set(cargo) != {
        "binary",
        "explicit_features",
        "package",
        "profile",
        "semantic_argv",
        "target",
        "unit_graph",
    }:
        raise ValueError("authenticated Cargo policy fields are not exact")
    if (
        cargo.get("binary") != "kagemusha_recursive_spend_v4_bundle"
        or cargo.get("package") != "iroha_core"
        or cargo.get("profile") != "release"
        or cargo.get("target") != "aarch64-apple-darwin"
    ):
        raise ValueError("authenticated Cargo policy target is not exact")
    graph = cargo.get("unit_graph")
    if not isinstance(graph, dict) or set(graph) != {
        "capture_receipt",
        "custom_build_packages",
        "custom_build_units",
        "iroha_core_units",
        "normalization",
        "packages",
        "raw_sha256",
        "raw_size_bytes",
        "sha256",
        "size_bytes",
        "units",
    }:
        raise ValueError("authenticated Cargo unit-graph fields are not exact")
    if graph.get("normalization") != SOURCE_SEAL_UNIT_GRAPH_NORMALIZATION:
        raise ValueError("authenticated Cargo unit-graph normalization differs")
    normalized_sha256 = canonical_nonzero_sha256(
        graph.get("sha256"), "authenticated normalized Cargo unit graph"
    )
    raw_sha256 = canonical_nonzero_sha256(
        graph.get("raw_sha256"), "authenticated raw Cargo unit graph"
    )
    for field, maximum in (
        ("size_bytes", 16 * 1024 * 1024),
        ("raw_size_bytes", 16 * 1024 * 1024),
        ("units", 100_000),
        ("packages", 100_000),
    ):
        value = graph.get(field)
        if type(value) is not int or not 1 <= value <= maximum:
            raise ValueError(f"authenticated Cargo unit-graph {field} is invalid")
    for field, upper_field in (
        ("custom_build_packages", "packages"),
        ("custom_build_units", "units"),
        ("iroha_core_units", "units"),
    ):
        value = graph.get(field)
        minimum = 1 if field == "iroha_core_units" else 0
        if type(value) is not int or not minimum <= value <= graph[upper_field]:
            raise ValueError(f"authenticated Cargo unit-graph {field} is invalid")
    toolchain = outer.get("toolchain")
    if not isinstance(toolchain, dict) or set(toolchain) != {"cargo", "rustc"}:
        raise ValueError("authenticated build-toolchain fields are not exact")
    tool_digests: dict[str, str] = {}
    for tool in ("cargo", "rustc"):
        identity = toolchain.get(tool)
        if not isinstance(identity, dict) or set(identity) != {
            "binary_sha256",
            "binary_size_bytes",
        }:
            raise ValueError(f"authenticated {tool} identity fields are not exact")
        tool_digests[tool] = canonical_nonzero_sha256(
            identity.get("binary_sha256"), f"authenticated {tool} binary"
        )
        size = identity.get("binary_size_bytes")
        if type(size) is not int or not 1 <= size <= 512 * 1024 * 1024:
            raise ValueError(f"authenticated {tool} binary size is invalid")
    capture = graph.get("capture_receipt")
    if not isinstance(capture, dict) or set(capture) != {
        "build_inputs_sha256",
        "cargo_binary_sha256",
        "exit_status",
        "raw_stdout_sha256",
        "raw_stdout_size_bytes",
        "rustc_binary_sha256",
        "schema",
        "source_commit",
        "source_tree_sha256",
        "stderr_sha256",
        "stderr_size_bytes",
    }:
        raise ValueError("authenticated Cargo capture-receipt fields are not exact")
    stderr_sha256 = canonical_nonzero_sha256(
        capture.get("stderr_sha256"), "authenticated Cargo capture stderr"
    )
    stderr_size = capture.get("stderr_size_bytes")
    if (
        capture.get("schema")
        != "iroha.kagemusha.cargo_unit_graph_capture_receipt.v1"
        or capture.get("build_inputs_sha256") != build_inputs_sha256
        or capture.get("cargo_binary_sha256") != tool_digests["cargo"]
        or capture.get("rustc_binary_sha256") != tool_digests["rustc"]
        or capture.get("exit_status") != 0
        or capture.get("raw_stdout_sha256") != raw_sha256
        or capture.get("raw_stdout_size_bytes") != graph["raw_size_bytes"]
        or capture.get("source_commit") != source_commit
        or capture.get("source_tree_sha256")
        != projection.get("source_tree_sha256")
        or type(stderr_size) is not int
        or not 0 <= stderr_size <= 16 * 1024 * 1024
        or (stderr_size == 0)
        != (stderr_sha256 == hashlib.sha256(b"").hexdigest())
    ):
        raise ValueError(
            "authenticated Cargo capture receipt does not bind the signed graph execution"
        )
    return {
        "build_inputs_sha256": build_inputs_sha256,
        "cargo_binary_sha256": tool_digests["cargo"],
        "execution_policy_sha256": execution_policy_sha256,
        "normalized_unit_graph_sha256": normalized_sha256,
        "normalized_unit_graph_size_bytes": graph["size_bytes"],
        "raw_unit_graph_sha256": raw_sha256,
        "raw_unit_graph_size_bytes": graph["raw_size_bytes"],
        "rustc_binary_sha256": tool_digests["rustc"],
    }
def isolated_source_trust_git_config(
    allowed_signers: Path, revocation: Path
) -> bytes:
    """Create the sole global Git config visible to the reviewed source helper."""
    for path, label in (
        (allowed_signers, "allowed-signers snapshot"),
        (revocation, "revocation snapshot"),
    ):
        path_text = str(path)
        if (
            not path.is_absolute()
            or path.resolve(strict=False) != path
            or re.fullmatch(r"/[A-Za-z0-9._/-]+", path_text) is None
        ):
            raise ValueError(f"{label} path is not safe for isolated Git config")
    return (
        '[gpg "ssh"]\n'
        f"\tallowedSignersFile = {allowed_signers}\n"
        f"\trevocationFile = {revocation}\n"
    ).encode("utf-8")
def source_git_environment() -> dict[str, str]:
    """Return the fixed environment for closure-commit blob authentication."""
    return {
        "GIT_CONFIG_GLOBAL": "/dev/null",
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_LITERAL_PATHSPECS": "1",
        "GIT_NO_REPLACE_OBJECTS": "1",
        "GIT_OPTIONAL_LOCKS": "0",
        "GIT_PAGER": "cat",
        "GIT_TERMINAL_PROMPT": "0",
        "HOME": "/var/empty",
        "LANG": "C",
        "LC_ALL": "C",
        "PAGER": "cat",
        "PATH": "/usr/bin:/bin",
        "TZ": "UTC",
    }
def authenticate_reviewed_source_file(
    relative: str,
    observed_bytes: bytes,
    source_commit: str,
    maximum_bytes: int,
) -> None:
    """Match one pinned worktree helper to its exact closure-commit blob."""
    if (
        not relative
        or relative.startswith("/")
        or "\\" in relative
        or any(component in {"", ".", ".."} for component in relative.split("/"))
    ):
        raise ValueError("reviewed helper path is not canonical")
    authenticated = subprocess.run(
        [
            str(SOURCE_GIT),
            "-c",
            "core.attributesFile=/dev/null",
            "-c",
            "core.excludesFile=/dev/null",
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.untrackedCache=false",
            "-C",
            str(root),
            "cat-file",
            "blob",
            f"{source_commit}:{relative}",
        ],
        cwd=Path("/"),
        env=source_git_environment(),
        stdin=subprocess.DEVNULL,
        check=False,
        capture_output=True,
        close_fds=True,
    )
    if (
        authenticated.returncode != 0
        or authenticated.stderr
        or not authenticated.stdout
        or len(authenticated.stdout) > maximum_bytes
    ):
        raise ValueError(f"could not authenticate reviewed helper {relative}")
    if authenticated.stdout != observed_bytes:
        raise ValueError(f"reviewed helper {relative} differs from the source closure")
def pin_authenticated_reviewed_source_file(
    relative: str,
    source_commit: str,
    maximum_bytes: int,
    label: str,
    retained_pins: list[tuple[Path, int, tuple[int, ...], str]],
) -> bytes:
    """Pin one root-custodied worktree file to its reviewed-commit blob."""
    path = root / relative
    descriptor, fingerprint = pin_regular_metadata(path, label)
    try:
        require_production_root_custody(descriptor, label)
        payload = read_pinned_descriptor(descriptor, fingerprint, maximum_bytes, label)
        authenticate_reviewed_source_file(relative, payload, source_commit, maximum_bytes)
    except BaseException:
        os.close(descriptor)
        raise
    retained_pins.append((path, descriptor, fingerprint, label))
    return payload
def authenticated_verifier_exited_without_reaping(
    process: subprocess.Popen[bytes],
) -> bool:
    """Observe one verifier leader's exit while preserving its process-group identity."""
    information = ctypes.create_string_buffer(WAITID_SIGINFO_BUFFER_BYTES)
    ctypes.set_errno(0)
    result = WAITID_LIBC.waitid(
        os.P_PID,
        process.pid,
        ctypes.byref(information),
        os.WEXITED | os.WNOHANG | os.WNOWAIT,
    )
    if result != 0:
        error_number = ctypes.get_errno()
        raise ValueError(
            f"authenticated verifier exit observation failed (errno {error_number})"
        )
    # siginfo_t begins with si_signo on every supported POSIX target. waitid with
    # WNOHANG leaves a zeroed structure when no child state is available.
    return ctypes.c_int.from_buffer(information).value != 0
def terminate_authenticated_verifier_process_group(
    process: subprocess.Popen[bytes],
    *,
    leader_exit_observed: bool = False,
) -> None:
    """Terminate the verifier group and reap its direct child exactly once."""
    try:
        observed_process_group = os.getpgid(process.pid)
    except ProcessLookupError:
        observed_process_group = process.pid
    try:
        os.killpg(observed_process_group, signal.SIGTERM)
    except ProcessLookupError:
        pass
    except PermissionError as error:
        if leader_exit_observed:
            # Darwin returns EPERM when the unreaped leader is the group's only
            # remaining member. A live same-UID descendant keeps the group
            # signalable; production root can signal every descendant UID.
            process.wait(timeout=KAGAMI_VERIFIER_REAP_TIMEOUT_SECONDS)
            return
        # Promotion always runs as root and therefore fails closed if group
        # signaling is denied. Candidate self-tests may execute under a host
        # harness whose helper processes have mixed effective UIDs; terminate
        # the direct child there while static mutation coverage still proves
        # that the production runner retains process-group cleanup.
        if os.geteuid() == PRODUCTION_TRUSTED_UID:
            raise ValueError(
                "authenticated verifier process-group SIGTERM cleanup failed"
            ) from error
        try:
            process.terminate()
        except ProcessLookupError:
            pass
        except OSError as direct_error:
            raise ValueError(
                "authenticated verifier candidate cleanup failed"
            ) from direct_error
        try:
            process.wait(timeout=KAGAMI_VERIFIER_REAP_TIMEOUT_SECONDS)
        except subprocess.TimeoutExpired as wait_error:
            try:
                process.kill()
                process.wait(timeout=KAGAMI_VERIFIER_REAP_TIMEOUT_SECONDS)
            except (OSError, subprocess.TimeoutExpired) as kill_error:
                raise ValueError(
                    "authenticated verifier candidate cleanup failed"
                ) from kill_error
        return
    except OSError as error:
        raise ValueError(
            "authenticated verifier process-group SIGTERM cleanup failed"
        ) from error
    # Keep the group leader unreaped during the grace period so its process-group
    # identifier cannot be recycled before the mandatory SIGKILL sweep.
    time.sleep(KAGAMI_VERIFIER_TERMINATE_GRACE_SECONDS)
    try:
        os.killpg(observed_process_group, signal.SIGKILL)
    except ProcessLookupError:
        pass
    except PermissionError as error:
        if leader_exit_observed:
            process.wait(timeout=KAGAMI_VERIFIER_REAP_TIMEOUT_SECONDS)
            return
        if os.geteuid() == PRODUCTION_TRUSTED_UID:
            raise ValueError(
                "authenticated verifier process-group SIGKILL cleanup failed"
            ) from error
        try:
            process.wait(timeout=KAGAMI_VERIFIER_REAP_TIMEOUT_SECONDS)
        except subprocess.TimeoutExpired as wait_error:
            try:
                process.kill()
                process.wait(timeout=KAGAMI_VERIFIER_REAP_TIMEOUT_SECONDS)
            except (OSError, subprocess.TimeoutExpired) as kill_error:
                raise ValueError(
                    "authenticated verifier candidate cleanup failed"
                ) from kill_error
        return
    except OSError as error:
        raise ValueError(
            "authenticated verifier process-group SIGKILL cleanup failed"
        ) from error
    try:
        process.wait(timeout=KAGAMI_VERIFIER_REAP_TIMEOUT_SECONDS)
    except subprocess.TimeoutExpired as error:
        raise ValueError("authenticated verifier process-group reap failed") from error
def run_bounded_authenticated_process(
    command: list[str],
    *,
    timeout_seconds: float,
    stdout_limit: int,
    stderr_limit: int,
    environment: dict[str, str],
) -> subprocess.CompletedProcess[bytes]:
    """Run one isolated process with a deadline and independently bounded pipes."""
    if timeout_seconds <= 0 or stdout_limit <= 0 or stderr_limit <= 0:
        raise ValueError("authenticated verifier execution bounds are invalid")
    try:
        process = subprocess.Popen(
            command,
            cwd=Path("/"),
            env=environment,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            close_fds=True,
            preexec_fn=os.setpgrp,
            bufsize=0,
        )
    except OSError as error:
        error_number = error.errno if error.errno is not None else 0
        raise ValueError(
            f"authenticated verifier could not start (errno {error_number})"
        ) from None
    selector = selectors.DefaultSelector()
    stdout = bytearray()
    stderr = bytearray()
    failure: str | None = None
    leader_exit_was_observed = False
    try:
        assert process.stdout is not None
        assert process.stderr is not None
        streams = (
            (process.stdout, "stdout", stdout, stdout_limit),
            (process.stderr, "stderr", stderr, stderr_limit),
        )
        for stream, name, capture, limit in streams:
            os.set_blocking(stream.fileno(), False)
            selector.register(stream, selectors.EVENT_READ, (name, capture, limit))
        deadline = time.monotonic() + timeout_seconds
        while selector.get_map():
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                failure = (
                    f"authenticated verifier exceeded its {timeout_seconds:g}-second timeout"
                )
                break
            try:
                ready = selector.select(remaining)
            except InterruptedError:
                continue
            if not ready:
                failure = (
                    f"authenticated verifier exceeded its {timeout_seconds:g}-second timeout"
                )
                break
            for key, _ in ready:
                name, capture, limit = key.data
                try:
                    chunk = os.read(
                        key.fd,
                        min(
                            VERIFIER_IO_CHUNK_BYTES,
                            max(1, limit + 1 - len(capture)),
                        ),
                    )
                except BlockingIOError:
                    continue
                if not chunk:
                    selector.unregister(key.fileobj)
                    key.fileobj.close()
                    continue
                capture.extend(chunk)
                if len(capture) > limit:
                    failure = (
                        f"authenticated verifier {name} exceeded its {limit}-byte limit"
                    )
                    break
            if failure is not None:
                break
        if failure is None:
            while True:
                if authenticated_verifier_exited_without_reaping(process):
                    leader_exit_was_observed = True
                    break
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    failure = (
                        f"authenticated verifier exceeded its {timeout_seconds:g}-second timeout"
                    )
                    break
                time.sleep(min(KAGAMI_VERIFIER_EXIT_POLL_SECONDS, remaining))
        if failure is not None:
            if not leader_exit_was_observed:
                leader_exit_was_observed = (
                    authenticated_verifier_exited_without_reaping(process)
                )
            terminate_authenticated_verifier_process_group(
                process,
                leader_exit_observed=leader_exit_was_observed,
            )
            raise ValueError(failure)
        # Keep the exited leader unreaped until both signals have swept its
        # original process group. This prevents a successful verifier from
        # leaving a pipe-closing descendant alive past promotion.
        terminate_authenticated_verifier_process_group(
            process,
            leader_exit_observed=True,
        )
        returncode = process.returncode
        if returncode is None:
            raise ValueError("authenticated verifier leader was not reaped")
        return subprocess.CompletedProcess(
            command,
            returncode,
            stdout=bytes(stdout),
            stderr=bytes(stderr),
        )
    except BaseException:
        if process.returncode is None:
            terminate_authenticated_verifier_process_group(
                process,
                leader_exit_observed=leader_exit_was_observed,
            )
        raise
    finally:
        selector.close()
        if process.stdout is not None and not process.stdout.closed:
            process.stdout.close()
        if process.stderr is not None and not process.stderr.closed:
            process.stderr.close()
def authenticated_tool_platform() -> str:
    """Return the exact supported platform named by the isolation contract."""
    if sys.platform == "darwin":
        return "macos"
    if sys.platform.startswith("linux"):
        return "linux"
    raise ValueError(
        "authenticated tool isolation is supported only on macOS and Linux"
    )
def authenticated_verifier_controller_command(controller: Path, verifier_command: list[str]) -> list[str]:
    """Build the mandatory OS-isolation request for one untrusted verifier.

    The independently reviewed controller must deny every filesystem write,
    network access, and child-process creation, and must attest an empty job at
    return.  Local pipe, deadline, and process-group bounds remain defense in
    depth; they cannot contain open-unlinked writes or ``setsid`` escapes.
    """
    if not controller.is_absolute() or not verifier_command:
        raise ValueError("authenticated verifier controller request is incomplete")
    expected_options = (
        "kagemusha",
        "verify-release-v4",
        "--bundle-dir",
        "--release-policy",
        "--benchmark-evidence",
        "--cryptographic-review",
    )
    if (
        len(verifier_command) != 11
        or tuple(verifier_command[1:4]) != expected_options[:3]
        or verifier_command[5] != expected_options[3]
        or verifier_command[7] != expected_options[4]
        or verifier_command[9] != expected_options[5]
    ):
        raise ValueError("authenticated verifier command shape is not exact")
    verifier = Path(verifier_command[0])
    bundle = Path(verifier_command[4])
    policy = Path(verifier_command[6])
    benchmark = Path(verifier_command[8])
    review = Path(verifier_command[10])
    if (
        not verifier.is_absolute()
        or not bundle.is_absolute()
        or not policy.is_absolute()
        or benchmark != bundle / "physical-device-benchmark.evidence"
        or review != bundle / "cryptographic-review.evidence"
    ):
        raise ValueError("authenticated verifier paths are not exact")
    readable_files = [policy, *(bundle / name for name in ARTIFACTS + FINAL_METADATA)]
    request = [
        str(controller),
        AUTHENTICATED_TOOL_CONTROLLER_SUBCOMMAND,
        "--contract",
        AUTHENTICATED_TOOL_CONTROLLER_CONTRACT,
        "--platform",
        authenticated_tool_platform(),
        "--expected-runtime-uid",
        str(os.geteuid()),
        "--expected-runtime-gid",
        str(os.getegid()),
        "--working-directory",
        "/",
        "--use-attested-runtime-identity",
        "--no-new-privileges",
        "--close-inherited-fds",
        "--forward-tool-exit-status",
        "--exact-tool-stdio",
        "--deny-network",
        "--deny-tool-process-spawn",
        "--deny-read-outside-allowlist",
        "--deny-all-writes",
        "--account-unlinked-write-bytes",
        "--require-empty-process-tree",
        "--cumulative-write-limit-bytes",
        "0",
        "--maximum-live-write-root-bytes",
        "0",
        "--wall-time-seconds",
        str(KAGAMI_VERIFIER_TIMEOUT_SECONDS),
        "--stdout-limit-bytes",
        str(MAX_KAGAMI_VERIFIER_STDOUT_BYTES),
        "--stderr-limit-bytes",
        str(MAX_KAGAMI_VERIFIER_STDERR_BYTES),
    ]
    request.extend(("--readable-directory", str(bundle)))
    for path in readable_files:
        request.extend(("--readable-file", str(path)))
    request.extend(("--", *verifier_command))
    return request
def run_authenticated_verifier(
    command: list[str],
    controller: Path,
) -> subprocess.CompletedProcess[bytes]:
    """Run Kagami only through the independently pinned isolation controller."""
    return run_bounded_authenticated_process(
        authenticated_verifier_controller_command(controller, command),
        timeout_seconds=KAGAMI_VERIFIER_TIMEOUT_SECONDS,
        stdout_limit=MAX_KAGAMI_VERIFIER_STDOUT_BYTES,
        stderr_limit=MAX_KAGAMI_VERIFIER_STDERR_BYTES,
        environment=SANITIZED_VERIFIER_ENV,
    )
def authenticated_verifier_exit_diagnostic(returncode: int) -> str:
    """Describe verifier failure without reflecting attacker-controlled output."""
    if returncode < 0:
        return f"terminated by signal {-returncode}"
    return f"exited with status {returncode}"
def release_verifier_command(verifier: Path, directory: Path, policy: Path) -> list[str]:
    """Use one explicitly digest-pinned Kagami verifier for promotion decisions."""
    return [
        str(verifier),
        "kagemusha",
        "verify-release-v4",
        "--bundle-dir",
        str(directory),
        "--release-policy",
        str(policy),
        "--benchmark-evidence",
        str(directory / "physical-device-benchmark.evidence"),
        "--cryptographic-review",
        str(directory / "cryptographic-review.evidence"),
    ]
def validate_kagami_verification_report(
    report: dict[str, object],
    *,
    directory: Path,
    manifest: dict[str, object],
    policy_sha256: str,
    promotion_record_sha256: str,
    qualification_receipt_sha256: str,
    internal_validation_receipt_sha256: str,
    ios_candidate_sha256: str,
) -> None:
    """Authenticate the complete machine report emitted by the pinned verifier."""
    exact_keys = {
        "status",
        "envelope_sha256",
        "manifest_body_sha256",
        "candidate_sha256",
        "qualification_receipt_sha256",
        "qualified_candidate_sha256",
        "internal_validation_receipt_sha256",
        "authenticated_source_seal_projection_sha256",
        "reviewed_cargo_binary_sha256",
        "reviewed_rustc_binary_sha256",
        "generator_binary_sha256",
        "sealed_candidate_build_report_sha256",
        "promotion_record_sha256",
        "release_policy_sha256",
        "generation",
        "generation_memory_limit_bytes",
        "generation_memory_enforcement_profile",
        "network_id",
        "asset_definition_id",
        "asset_scale",
        "bridge_abi_version",
        "recursive_step_verifier_commitment",
        "artifacts",
    }
    if set(report) != exact_keys:
        raise ValueError("Kagami verification report fields are not exact")
    if report.get("status") != "verified" or report.get("bridge_abi_version") != 22:
        raise ValueError("Kagami did not report one verified native-ABI-22 release")
    if report.get("envelope_sha256") != directory.name:
        raise ValueError("Kagami manifest envelope differs from the release directory")
    if report.get("release_policy_sha256") != policy_sha256:
        raise ValueError("Kagami verified a different release policy")
    if report.get("promotion_record_sha256") != promotion_record_sha256:
        raise ValueError("Kagami reconstructed a different promotion record")
    if report.get("qualification_receipt_sha256") != qualification_receipt_sha256:
        raise ValueError("Kagami verified a different recursive qualification receipt")
    if (
        report.get("internal_validation_receipt_sha256")
        != internal_validation_receipt_sha256
    ):
        raise ValueError("Kagami verified a different internal-validation receipt")
    if report.get("candidate_sha256") != ios_candidate_sha256:
        raise ValueError(
            "signed physical-iOS candidate differs from Kagami's reconstructed candidate"
        )
    for field in (
        "manifest_body_sha256",
        "candidate_sha256",
        "qualification_receipt_sha256",
        "qualified_candidate_sha256",
        "internal_validation_receipt_sha256",
        "authenticated_source_seal_projection_sha256",
        "reviewed_cargo_binary_sha256",
        "reviewed_rustc_binary_sha256",
        "generator_binary_sha256",
        "sealed_candidate_build_report_sha256",
        "promotion_record_sha256",
        "release_policy_sha256",
        "recursive_step_verifier_commitment",
    ):
        canonical_nonzero_sha256(report.get(field), f"Kagami report {field}")
    manifest_equal_fields = (
        "generation",
        "generation_memory_limit_bytes",
        "generation_memory_enforcement_profile",
        "authenticated_source_seal_projection_sha256",
        "reviewed_cargo_binary_sha256",
        "reviewed_rustc_binary_sha256",
        "generator_binary_sha256",
        "sealed_candidate_build_report_sha256",
        "network_id",
        "asset_scale",
    )
    for field in manifest_equal_fields:
        if report.get(field) != manifest.get(field):
            raise ValueError(f"Kagami report {field} differs from the manifest")
    manifest_asset = manifest.get("asset")
    if isinstance(manifest_asset, str) and report.get("asset_definition_id") != manifest_asset:
        raise ValueError("Kagami report asset differs from the manifest")
    if report.get("qualified_candidate_sha256") != manifest.get(
        "qualified_candidate_sha256"
    ):
        raise ValueError("Kagami qualified candidate differs from the manifest")
    expected_artifacts: list[dict[str, object]] = []
    profiles = manifest.get("profiles")
    if not isinstance(profiles, list):
        raise ValueError("manifest profiles are not an array")
    flattened: list[dict[str, object]] = []
    for profile in profiles:
        if not isinstance(profile, dict) or not isinstance(profile.get("artifacts"), list):
            raise ValueError("manifest proof profile is malformed")
        for artifact in profile["artifacts"]:
            if not isinstance(artifact, dict):
                raise ValueError("manifest artifact is malformed")
            flattened.append(artifact)
    if len(flattened) != len(REPORT_ARTIFACT_PURPOSES):
        raise ValueError("manifest does not contain the exact report artifact set")
    for purpose, artifact in zip(REPORT_ARTIFACT_PURPOSES, flattened, strict=True):
        expected_artifacts.append(
            {
                "purpose": purpose,
                "file_name": artifact.get("file_name"),
                "size_bytes": artifact.get("size_bytes"),
                "sha256": artifact.get("sha256"),
                "payload_size_bytes": artifact.get("payload_size_bytes"),
                "payload_sha256": artifact.get("payload_sha256"),
            }
        )
    roster = manifest.get("topup_finality_roster_artifact")
    if not isinstance(roster, dict):
        raise ValueError("manifest top-up finality roster binding is malformed")
    expected_artifacts.append(
        {
            "purpose": "topup_finality_roster",
            "file_name": roster.get("file_name"),
            "size_bytes": roster.get("size_bytes"),
            "sha256": roster.get("sha256"),
            "payload_size_bytes": None,
            "payload_sha256": None,
        }
    )
    artifacts = report.get("artifacts")
    if artifacts != expected_artifacts:
        raise ValueError("Kagami report artifact inventory differs from the manifest")
def ios_evidence_configuration(
    errors: list[str],
) -> tuple[Path, str, Path, Path, str, Path, str, Path] | None:
    """Return the complete opt-in physical-iOS evidence configuration."""
    root_text = os.environ.get("KAGEMUSHA_IOS_DEVICE_EVIDENCE_ROOT", "")
    key_id = os.environ.get("KAGEMUSHA_IOS_DEVICE_EVIDENCE_TRUSTED_KEY_ID", "")
    public_key_text = os.environ.get(
        "KAGEMUSHA_IOS_DEVICE_EVIDENCE_TRUSTED_PUBLIC_KEY", ""
    )
    production_policy_text = os.environ.get(
        "KAGEMUSHA_IOS_DEVICE_EVIDENCE_PRODUCTION_POLICY", ""
    )
    freshness_key_id = os.environ.get(
        "KAGEMUSHA_IOS_DEVICE_EVIDENCE_FRESHNESS_TRUSTED_KEY_ID", ""
    )
    freshness_public_key_text = os.environ.get(
        "KAGEMUSHA_IOS_DEVICE_EVIDENCE_FRESHNESS_TRUSTED_PUBLIC_KEY", ""
    )
    promotion_id = os.environ.get("KAGEMUSHA_V4_PROMOTION_ID", "")
    catalog_revalidation_receipt_text = os.environ.get(
        "KAGEMUSHA_IOS_DEVICE_EVIDENCE_CATALOG_REVALIDATION_RECEIPT", ""
    )
    present = tuple(
        bool(value)
        for value in (
            root_text,
            key_id,
            public_key_text,
            production_policy_text,
            freshness_key_id,
            freshness_public_key_text,
            promotion_id,
            catalog_revalidation_receipt_text,
        )
    )
    if not any(present):
        errors.append(
            "promotion requires signed production physical-iOS raw evidence, trusted key id, "
            "public key, production policy, and independent online freshness authority"
        )
        return None
    if not all(present):
        errors.append(
            "physical-iOS evidence requires KAGEMUSHA_IOS_DEVICE_EVIDENCE_ROOT, "
            "KAGEMUSHA_IOS_DEVICE_EVIDENCE_TRUSTED_KEY_ID, and "
            "KAGEMUSHA_IOS_DEVICE_EVIDENCE_TRUSTED_PUBLIC_KEY, and "
            "KAGEMUSHA_IOS_DEVICE_EVIDENCE_PRODUCTION_POLICY, "
            "KAGEMUSHA_IOS_DEVICE_EVIDENCE_FRESHNESS_TRUSTED_KEY_ID, and "
            "KAGEMUSHA_IOS_DEVICE_EVIDENCE_FRESHNESS_TRUSTED_PUBLIC_KEY, "
            "KAGEMUSHA_V4_PROMOTION_ID, and "
            "KAGEMUSHA_IOS_DEVICE_EVIDENCE_CATALOG_REVALIDATION_RECEIPT together"
        )
        return None
    ios_root = Path(root_text)
    public_key = Path(public_key_text)
    production_policy = Path(production_policy_text)
    freshness_public_key = Path(freshness_public_key_text)
    catalog_revalidation_receipt = Path(catalog_revalidation_receipt_text)
    if (
        not ios_root.is_absolute()
        or ios_root.resolve(strict=False) != ios_root
        or not ios_root.is_dir()
        or ios_root.is_symlink()
    ):
        errors.append("physical-iOS evidence root must be a canonical absolute real directory")
        return None
    try:
        catalog_revalidation_receipt.resolve(strict=False).relative_to(ios_root)
    except ValueError:
        pass
    else:
        errors.append(
            "physical-iOS catalog revalidation receipt must stay outside the "
            "immutable evidence root"
        )
        return None
    if (
        re.fullmatch(r"[0-9a-f]{64}", promotion_id) is None
        or promotion_id == "0" * 64
        or not catalog_revalidation_receipt.is_absolute()
        or catalog_revalidation_receipt.resolve(strict=False)
        != catalog_revalidation_receipt
        or not catalog_revalidation_receipt.is_file()
        or catalog_revalidation_receipt.is_symlink()
        or catalog_revalidation_receipt.stat().st_size == 0
        or catalog_revalidation_receipt.stat().st_size > 256 * 1024
        or catalog_revalidation_receipt
        in {public_key, production_policy, freshness_public_key}
    ):
        errors.append(
            "physical-iOS catalog revalidation requires a unique nonzero promotion "
            "id and a distinct canonical absolute bounded receipt file"
        )
        return None
    if (
        not public_key.is_absolute()
        or public_key.resolve(strict=False) != public_key
        or not public_key.is_file()
        or public_key.is_symlink()
    ):
        errors.append("physical-iOS trusted public key must be a canonical absolute regular file")
        return None
    if (
        not production_policy.is_absolute()
        or production_policy.resolve(strict=False) != production_policy
        or not production_policy.is_file()
        or production_policy.is_symlink()
        or production_policy.stat().st_size == 0
        or production_policy.stat().st_size > 1024 * 1024
    ):
        errors.append(
            "physical-iOS production policy must be a canonical absolute bounded regular file"
        )
        return None
    if (
        freshness_key_id == key_id
        or freshness_public_key == public_key
        or not freshness_public_key.is_absolute()
        or freshness_public_key.resolve(strict=False) != freshness_public_key
        or not freshness_public_key.is_file()
        or freshness_public_key.is_symlink()
    ):
        errors.append(
            "physical-iOS online freshness authority must use a distinct key id and "
            "canonical absolute regular public-key file"
        )
        return None
    return (
        ios_root,
        key_id,
        public_key,
        production_policy,
        freshness_key_id,
        freshness_public_key,
        promotion_id,
        catalog_revalidation_receipt,
    )
def load_ios_evidence_validator(
    candidate_module_bytes: bytes,
    candidate_module_path: Path,
    production_module_bytes: bytes,
    production_module_path: Path,
) -> tuple[Callable[..., list[str]], Callable[..., list[str]]]:
    """Load both reviewed validators from already pinned source bytes."""
    module_name = "_iroha_pinned_kagemusha_candidate_ios_evidence"
    module = types.ModuleType(module_name)
    module.__file__ = str(candidate_module_path)
    module.__package__ = ""
    sys.modules[module_name] = module
    production_name = "_iroha_pinned_kagemusha_production_ios_evidence"
    production_module = types.ModuleType(production_name)
    production_module.__file__ = str(production_module_path)
    production_module.__package__ = ""
    sys.modules[production_name] = production_module
    try:
        code = compile(
            candidate_module_bytes,
            str(candidate_module_path),
            "exec",
            dont_inherit=True,
        )
        exec(code, module.__dict__)
        production_code = compile(
            production_module_bytes,
            str(production_module_path),
            "exec",
            dont_inherit=True,
        )
        exec(production_code, production_module.__dict__)
        production_validator = production_module.__dict__.get(
            "validate_production_signed_evidence"
        )
        if not callable(production_validator):
            raise ValueError(
                "pinned production physical-iOS validator has no maintained entrypoint"
            )
        historical_validator = production_module.__dict__.get(
            "validate_historical_production_evidence_for_catalog_revalidation"
        )
        if not callable(historical_validator):
            raise ValueError(
                "pinned production physical-iOS validator has no historical catalog entrypoint"
            )
        catalog_validator = production_module.__dict__.get(
            "validate_catalog_revalidation_receipt"
        )
        if not callable(catalog_validator):
            raise ValueError(
                "pinned production physical-iOS validator has no catalog revalidation entrypoint"
            )
        def validate(
            evidence_path: Path,
            artifact_root: Path,
            trusted_key_id: str,
            trusted_public_key_path: Path,
            production_policy_path: Path,
            freshness_receipt_path: Path,
            trusted_freshness_key_id: str,
            trusted_freshness_public_key_path: Path,
        ) -> list[str]:
            return historical_validator(
                evidence_path,
                artifact_root,
                trusted_key_id,
                trusted_public_key_path,
                production_policy_path,
                module,
                freshness_receipt_path=freshness_receipt_path,
                trusted_freshness_key_id=trusted_freshness_key_id,
                trusted_freshness_public_key_path=trusted_freshness_public_key_path,
            )
        def validate_catalog(
            receipt_path: Path,
            trusted_key_id: str,
            trusted_public_key_path: Path,
            expected_promotion_id: str,
            expected_bindings: list[dict[str, str]],
            lab_signer_key_id: str,
            lab_signer_public_key_path: Path,
        ) -> list[str]:
            return catalog_validator(
                receipt_path,
                trusted_key_id,
                trusted_public_key_path,
                expected_promotion_id,
                expected_bindings,
                lab_signer_key_id,
                lab_signer_public_key_path,
                module,
            )
        return validate, validate_catalog
    except BaseException:
        sys.modules.pop(module_name, None)
        sys.modules.pop(production_name, None)
        raise
def verify_ios_evidence(
    directory: Path,
    ios_configuration: tuple[Path, str, Path, Path, str, Path, str, Path],
    validator: Callable[[Path, Path, str, Path, Path, Path, str, Path], list[str]],
    evidence_bytes: bytes,
    trusted_public_key_snapshot: Path,
    trusted_production_policy_snapshot: Path,
    trusted_freshness_public_key_snapshot: Path,
    directory_pins: list[tuple[Path, int, tuple[int, ...], str]],
    file_pins: list[tuple[Path, int, tuple[int, ...], str]],
    staging_parent: Path,
) -> tuple[str | None, dict[str, str] | None, str | None]:
    """Verify one signed raw slot from the exact bytes used for its candidate digest."""
    ios_root, key_id, _, _, freshness_key_id, _, _, _ = ios_configuration
    release_root = ios_root / directory.name
    raw_root = release_root / "raw"
    freshness_receipt = (
        release_root / "online-freshness-consumption-receipt-v1.json"
    )
    if (
        not release_root.is_dir()
        or release_root.is_symlink()
        or not raw_root.is_dir()
        or raw_root.is_symlink()
    ):
        return None, None, (
            f"{directory.name}: physical-iOS evidence must use "
            f"{ios_root}/<manifest-sha256>/raw"
        )
    try:
        # Retain the externally named roots through the whole promotion.  The
        # reviewed validator snapshots every exact raw file, validates it, and
        # performs a full identity/content rescan before returning.  Root-only
        # custody then protects that authenticated inventory after the rescan.
        for path, label in (
            (release_root, f"physical-iOS release directory {release_root}"),
            (raw_root, f"physical-iOS raw directory {raw_root}"),
        ):
            descriptor, fingerprint = pin_directory_metadata(path, label)
            try:
                require_production_root_custody(descriptor, label)
            except BaseException:
                os.close(descriptor)
                raise
            directory_pins.append((path, descriptor, fingerprint, label))
        label = f"physical-iOS online freshness receipt {freshness_receipt}"
        descriptor, fingerprint = pin_regular_metadata(freshness_receipt, label)
        try:
            require_production_root_custody(descriptor, label)
        except BaseException:
            os.close(descriptor)
            raise
        freshness_receipt_bytes = read_pinned_descriptor(
            descriptor, fingerprint, 64 * 1024, label
        )
        file_pins.append((freshness_receipt, descriptor, fingerprint, label))
        freshness_snapshot, freshness_snapshot_path = snapshot_private_bytes(
            freshness_receipt_bytes,
            "online-freshness-consumption-receipt-v1.json",
            "physical-iOS online freshness/consumption receipt",
            staging_parent,
        )
        try:
            evidence_snapshot, evidence_snapshot_path = snapshot_private_bytes(
                evidence_bytes,
                "physical-device-benchmark.evidence",
                "signed physical-iOS evidence",
                staging_parent,
            )
            try:
                validation_errors = validator(
                    evidence_snapshot_path,
                    raw_root,
                    key_id,
                    trusted_public_key_snapshot,
                    trusted_production_policy_snapshot,
                    freshness_snapshot_path,
                    freshness_key_id,
                    trusted_freshness_public_key_snapshot,
                )
            finally:
                evidence_snapshot.cleanup()
        finally:
            freshness_snapshot.cleanup()
        if validation_errors:
            return None, None, (
                f"{directory.name}: physical-iOS evidence verification failed: "
                f"{validation_errors[-1]}"
            )
        evidence = strict_json_bytes(
            evidence_bytes,
            "signed physical-iOS evidence",
        )
        artifact_digests = evidence.get("artifact_digests")
        if not isinstance(artifact_digests, dict):
            raise ValueError("artifact_digests is not an object")
        candidate = artifact_digests.get("input/candidate-v4.norito")
        if not isinstance(candidate, dict):
            raise ValueError("candidate artifact binding is missing")
        candidate_sha256 = candidate.get("sha256")
        if (
            not isinstance(candidate_sha256, str)
            or re.fullmatch(r"[0-9a-f]{64}", candidate_sha256) is None
            or candidate_sha256 == "0" * 64
        ):
            raise ValueError("candidate artifact digest is not canonical")
        if evidence.get("release_manifest_sha256") != directory.name:
            raise ValueError(
                "production iOS evidence release manifest digest does not match catalog"
            )
    except (OSError, UnicodeError, ValueError, json.JSONDecodeError) as error:
        return None, None, (
            f"{directory.name}: invalid signed physical-iOS evidence: {error}"
        )
    binding = {
        "release_manifest_sha256": directory.name,
        "evidence_sha256": hashlib.sha256(evidence_bytes).hexdigest(),
        "consumption_receipt_sha256": hashlib.sha256(
            freshness_receipt_bytes
        ).hexdigest(),
    }
    return candidate_sha256, binding, None
def promotion_errors() -> list[str]:
    global authenticated_readiness_source_contract_bytes
    global authenticated_readiness_self_test_bytes
    authenticated_readiness_source_contract_bytes = {}
    authenticated_readiness_self_test_bytes = None
    errors: list[str] = []
    if os.geteuid() != PRODUCTION_TRUSTED_UID:
        return [
            "production promotion must run as root so its custody policy matches the runtime"
        ]
    try:
        validate_inherited_promotion_gate()
    except (OSError, ValueError) as error:
        return [f"promotion gate bootstrap custody failed: {error}"]
    try:
        validate_inherited_promotion_python()
    except (OSError, ValueError) as error:
        return [f"promotion Python pre-exec custody failed: {error}"]
    policy_text = os.environ.get("KAGEMUSHA_V4_RELEASE_POLICY_PATH", "")
    artifact_text = os.environ.get("KAGEMUSHA_V4_ARTIFACT_ROOT", "")
    if not policy_text or not artifact_text:
        return [
            "promotion requires KAGEMUSHA_V4_RELEASE_POLICY_PATH and KAGEMUSHA_V4_ARTIFACT_ROOT"
        ]
    policy = Path(policy_text)
    artifact_root = Path(artifact_text)
    verifier_text = os.environ.get(KAGAMI_VERIFIER_PATH_ENV, "")
    verifier_sha256 = os.environ.get(KAGAMI_VERIFIER_SHA256_ENV, "")
    verifier = Path(verifier_text) if verifier_text else None
    tool_controller_text = os.environ.get(
        AUTHENTICATED_TOOL_CONTROLLER_PATH_ENV, ""
    )
    tool_controller_sha256 = os.environ.get(
        AUTHENTICATED_TOOL_CONTROLLER_SHA256_ENV, ""
    )
    tool_controller = Path(tool_controller_text) if tool_controller_text else None
    if (
        not policy.is_absolute()
        or not artifact_root.is_absolute()
        or policy.resolve(strict=False) != policy
        or artifact_root.resolve(strict=False) != artifact_root
    ):
        errors.append("promotion policy and artifact root must be canonical absolute paths")
    if (
        not policy.is_file()
        or policy.is_symlink()
        or policy.stat().st_size == 0
        or policy.stat().st_size > 64 * 1024
    ):
        errors.append("promotion policy must be a nonempty regular file")
    if not artifact_root.is_dir() or artifact_root.is_symlink():
        errors.append("promotion artifact root must be a real directory")
        return errors
    if (
        verifier is None
        or not verifier.is_absolute()
        or verifier.resolve(strict=False) != verifier
        or not verifier.is_file()
        or verifier.is_symlink()
        or re.fullmatch(r"[0-9a-f]{64}", verifier_sha256) is None
        or verifier_sha256 == "0" * 64
    ):
        errors.append(
            "promotion requires a canonical absolute digest-pinned Kagami executable via "
            f"{KAGAMI_VERIFIER_PATH_ENV} and {KAGAMI_VERIFIER_SHA256_ENV}"
        )
        return errors
    if (
        tool_controller is None
        or not tool_controller.is_absolute()
        or tool_controller.resolve(strict=False) != tool_controller
        or not tool_controller.is_file()
        or tool_controller.is_symlink()
        or re.fullmatch(r"[0-9a-f]{64}", tool_controller_sha256) is None
        or tool_controller_sha256 == "0" * 64
        or tool_controller == verifier
        or tool_controller_sha256 == verifier_sha256
    ):
        errors.append(
            "promotion requires a distinct canonical absolute digest-pinned "
            f"{AUTHENTICATED_TOOL_CONTROLLER_CONTRACT} controller via "
            f"{AUTHENTICATED_TOOL_CONTROLLER_PATH_ENV} and "
            f"{AUTHENTICATED_TOOL_CONTROLLER_SHA256_ENV}"
        )
        return errors
    ios_configuration = ios_evidence_configuration(errors)
    source_identity: dict[str, object] | None = None
    source_projection_identity: dict[str, object] | None = None
    sealed_build_report_identity: dict[str, object] | None = None
    reviewed_closure_text = os.environ.get(
        "KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE", ""
    )
    reviewed_closure_sha256 = os.environ.get(
        "KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE_SHA256", ""
    )
    reviewed_closure = Path(reviewed_closure_text) if reviewed_closure_text else None
    if (
        not reviewed_closure_text
        or reviewed_closure is None
        or not reviewed_closure.is_absolute()
        or reviewed_closure.resolve(strict=False) != reviewed_closure
        or not reviewed_closure.is_file()
        or reviewed_closure.is_symlink()
        or re.fullmatch(r"[0-9a-f]{64}", reviewed_closure_sha256) is None
        or reviewed_closure_sha256 == "0" * 64
    ):
        errors.append(
            "promotion requires a canonical absolute independently pinned reviewed "
            "source-closure path and SHA-256"
        )
    allowed_signers_text = os.environ.get(SOURCE_ALLOWED_SIGNERS_PATH_ENV, "")
    allowed_signers_sha256 = os.environ.get(SOURCE_ALLOWED_SIGNERS_SHA256_ENV, "")
    revocation_text = os.environ.get(SOURCE_REVOCATION_PATH_ENV, "")
    revocation_sha256 = os.environ.get(SOURCE_REVOCATION_SHA256_ENV, "")
    source_projection_text = os.environ.get(SOURCE_SEAL_PROJECTION_PATH_ENV, "")
    source_projection_sha256 = os.environ.get(
        SOURCE_SEAL_PROJECTION_SHA256_ENV, ""
    )
    sealed_build_report_text = os.environ.get(SEALED_BUILD_REPORT_PATH_ENV, "")
    sealed_build_report_sha256 = os.environ.get(
        SEALED_BUILD_REPORT_SHA256_ENV, ""
    )
    allowed_signers = Path(allowed_signers_text) if allowed_signers_text else None
    revocation = Path(revocation_text) if revocation_text else None
    source_projection = (
        Path(source_projection_text) if source_projection_text else None
    )
    sealed_build_report = (
        Path(sealed_build_report_text) if sealed_build_report_text else None
    )
    if (
        allowed_signers is None
        or not allowed_signers.is_absolute()
        or allowed_signers.resolve(strict=False) != allowed_signers
        or not allowed_signers.is_file()
        or allowed_signers.is_symlink()
        or re.fullmatch(r"[0-9a-f]{64}", allowed_signers_sha256) is None
        or allowed_signers_sha256 == "0" * 64
    ):
        errors.append(
            "promotion requires one canonical root-custodied digest-pinned SSH allowed-signers policy"
        )
    if (
        revocation is None
        or not revocation.is_absolute()
        or revocation.resolve(strict=False) != revocation
        or not revocation.is_file()
        or revocation.is_symlink()
        or re.fullmatch(r"[0-9a-f]{64}", revocation_sha256) is None
        or revocation_sha256 == "0" * 64
    ):
        errors.append(
            "promotion requires one canonical root-custodied digest-pinned SSH revocation policy (an explicitly pinned empty file means no revocations)"
        )
    if (
        source_projection is None
        or not source_projection.is_absolute()
        or source_projection.resolve(strict=False) != source_projection
        or not source_projection.is_file()
        or source_projection.is_symlink()
        or re.fullmatch(r"[0-9a-f]{64}", source_projection_sha256) is None
        or source_projection_sha256 == "0" * 64
    ):
        errors.append(
            "promotion requires the canonical independently digest-pinned authenticated source-seal projection"
        )
    if (
        sealed_build_report is None
        or not sealed_build_report.is_absolute()
        or sealed_build_report.resolve(strict=False) != sealed_build_report
        or not sealed_build_report.is_file()
        or sealed_build_report.is_symlink()
        or re.fullmatch(r"[0-9a-f]{64}", sealed_build_report_sha256) is None
        or sealed_build_report_sha256 == "0" * 64
    ):
        errors.append(
            "promotion requires the canonical root-custodied digest-pinned sealed candidate double-build report"
        )
    projection_verification_inputs: dict[
        str, tuple[Path, str, str, int, bool]
    ] = {}
    for (
        key,
        path_environment,
        digest_environment,
        snapshot_name,
        maximum_bytes,
        allow_empty,
    ) in SOURCE_SEAL_VERIFICATION_INPUTS:
        path_text = os.environ.get(path_environment, "")
        expected_sha256 = os.environ.get(digest_environment, "")
        path = Path(path_text) if path_text else None
        if (
            path is None
            or not path.is_absolute()
            or path.resolve(strict=False) != path
            or not path.is_file()
            or path.is_symlink()
            or re.fullmatch(r"[0-9a-f]{64}", expected_sha256) is None
            or expected_sha256 == "0" * 64
        ):
            errors.append(
                "promotion requires every canonical root-custodied digest-pinned "
                f"source-projection reconstruction input ({path_environment} and "
                f"{digest_environment})"
            )
            continue
        projection_verification_inputs[key] = (
            path,
            expected_sha256,
            snapshot_name,
            maximum_bytes,
            allow_empty,
        )
    semantic_input_paths = [
        path
        for path in (
            reviewed_closure,
            allowed_signers,
            revocation,
            source_projection,
            sealed_build_report,
        )
        if path is not None
    ] + [
        input_path
        for input_path, _, _, _, _ in projection_verification_inputs.values()
    ]
    if len(set(semantic_input_paths)) != len(semantic_input_paths):
        errors.append(
            "promotion source/projection trust inputs must use distinct semantic paths"
        )
    catalog_directory_pins: list[tuple[Path, int, tuple[int, ...], str]] = []
    trusted_file_pins: list[tuple[Path, int, tuple[int, ...], str]] = []
    policy_sha256 = ""
    ios_validator: Callable[
        [Path, Path, str, Path, Path, Path, str, Path], list[str]
    ] | None = None
    ios_catalog_validator: Callable[..., list[str]] | None = None
    ios_validator_path = root / IOS_EVIDENCE_MODULE
    production_ios_validator_path = root / PRODUCTION_IOS_EVIDENCE_MODULE
    source_helper_path = root / SOURCE_TREE_SEAL
    readiness_self_test_path = root / READINESS_SELF_TEST
    verifier_snapshot: tempfile.TemporaryDirectory[str] | None = None
    tool_controller_snapshot: tempfile.TemporaryDirectory[str] | None = None
    public_key_snapshot: tempfile.TemporaryDirectory[str] | None = None
    freshness_public_key_snapshot: tempfile.TemporaryDirectory[str] | None = None
    catalog_revalidation_receipt_snapshot: tempfile.TemporaryDirectory[str] | None = None
    ios_policy_snapshot: tempfile.TemporaryDirectory[str] | None = None
    closure_snapshot: tempfile.TemporaryDirectory[str] | None = None
    source_projection_snapshot: tempfile.TemporaryDirectory[str] | None = None
    projection_verification_snapshots: list[
        tempfile.TemporaryDirectory[str]
    ] = []
    trusted_projection_verification_inputs: dict[str, Path] = {}
    allowed_signers_snapshot: tempfile.TemporaryDirectory[str] | None = None
    revocation_snapshot: tempfile.TemporaryDirectory[str] | None = None
    source_trust_config_snapshot: tempfile.TemporaryDirectory[str] | None = None
    source_helper_snapshot: tempfile.TemporaryDirectory[str] | None = None
    source_producer_package_snapshot: tempfile.TemporaryDirectory[str] | None = None
    ios_validator_snapshot: tempfile.TemporaryDirectory[str] | None = None
    production_ios_validator_snapshot: tempfile.TemporaryDirectory[str] | None = None
    trusted_public_key_snapshot: Path | None = None
    trusted_freshness_public_key_snapshot: Path | None = None
    trusted_catalog_revalidation_receipt_snapshot: Path | None = None
    trusted_ios_policy_snapshot: Path | None = None
    trusted_closure_snapshot: Path | None = None
    trusted_source_projection_snapshot: Path | None = None
    trusted_allowed_signers_snapshot: Path | None = None
    trusted_revocation_snapshot: Path | None = None
    trusted_source_trust_home: Path | None = None
    trusted_source_helper_snapshot: Path | None = None
    trusted_source_projection_launcher: Path | None = None
    verifier_exec = verifier
    tool_controller_exec = tool_controller
    def cleanup_private_snapshots() -> None:
        for projection_snapshot in reversed(projection_verification_snapshots):
            projection_snapshot.cleanup()
        if production_ios_validator_snapshot is not None:
            production_ios_validator_snapshot.cleanup()
        if ios_validator_snapshot is not None:
            ios_validator_snapshot.cleanup()
        if source_helper_snapshot is not None:
            source_helper_snapshot.cleanup()
        if source_producer_package_snapshot is not None:
            source_producer_package_snapshot.cleanup()
        if source_trust_config_snapshot is not None:
            source_trust_config_snapshot.cleanup()
        if revocation_snapshot is not None:
            revocation_snapshot.cleanup()
        if allowed_signers_snapshot is not None:
            allowed_signers_snapshot.cleanup()
        if source_projection_snapshot is not None:
            source_projection_snapshot.cleanup()
        if closure_snapshot is not None:
            closure_snapshot.cleanup()
        if public_key_snapshot is not None:
            public_key_snapshot.cleanup()
        if freshness_public_key_snapshot is not None:
            freshness_public_key_snapshot.cleanup()
        if catalog_revalidation_receipt_snapshot is not None:
            catalog_revalidation_receipt_snapshot.cleanup()
        if ios_policy_snapshot is not None:
            ios_policy_snapshot.cleanup()
        if verifier_snapshot is not None:
            verifier_snapshot.cleanup()
        if tool_controller_snapshot is not None:
            tool_controller_snapshot.cleanup()
    try:
        python_runtime = Path(sys.executable)
        if (
            not python_runtime.is_absolute()
            or python_runtime.resolve(strict=False) != python_runtime
            or not python_runtime.is_file()
            or python_runtime.is_symlink()
        ):
            raise ValueError("promotion Python runtime path is not canonical")
        if (
            not PROMOTION_STAGING_PARENT.is_absolute()
            or PROMOTION_STAGING_PARENT.resolve(strict=False)
            != PROMOTION_STAGING_PARENT
            or not PROMOTION_STAGING_PARENT.is_dir()
            or PROMOTION_STAGING_PARENT.is_symlink()
        ):
            raise ValueError(
                f"fixed promotion staging parent is unavailable: {PROMOTION_STAGING_PARENT}"
            )
        if not SOURCE_GIT.is_file() or SOURCE_GIT.is_symlink():
            raise ValueError("fixed source-authentication Git is unavailable")
        seen_directories: set[Path] = set()
        production_roots = [
            root,
            source_helper_path.parent,
            ios_validator_path.parent,
            production_ios_validator_path.parent,
            artifact_root,
            policy.parent,
            verifier.parent,
            tool_controller.parent,
            trusted_python_runtime_root,
            python_runtime.parent,
            PROMOTION_STAGING_PARENT,
            SOURCE_GIT.parent,
        ]
        if reviewed_closure is not None:
            production_roots.append(reviewed_closure.parent)
        if sealed_build_report is not None:
            production_roots.append(sealed_build_report.parent)
        if source_projection is not None:
            production_roots.append(source_projection.parent)
        if allowed_signers is not None:
            production_roots.append(allowed_signers.parent)
        if revocation is not None:
            production_roots.append(revocation.parent)
        production_roots.extend(
            input_path.parent
            for input_path, _, _, _, _ in projection_verification_inputs.values()
        )
        if ios_configuration is not None:
            production_roots.extend(
                [
                    ios_configuration[0],
                    ios_configuration[2].parent,
                    ios_configuration[3].parent,
                    ios_configuration[5].parent,
                    ios_configuration[7].parent,
                ]
            )
        production_directory_paths = {
            path
            for trusted_root in production_roots
            for path in absolute_directory_chain(trusted_root)
        }
        trusted_roots = [
            *production_roots,
            source_helper_path.parent,
            ios_validator_path.parent,
            production_ios_validator_path.parent,
        ]
        for trusted_root in trusted_roots:
            for path in absolute_directory_chain(trusted_root):
                if path in seen_directories:
                    continue
                seen_directories.add(path)
                label = f"trusted release path component {path}"
                descriptor, fingerprint = pin_directory_metadata(path, label)
                if path in production_directory_paths:
                    try:
                        require_production_root_custody(descriptor, label)
                    except BaseException:
                        os.close(descriptor)
                        raise
                catalog_directory_pins.append((path, descriptor, fingerprint, label))
        gate_path = root / READINESS
        label = f"reviewed promotion readiness gate {gate_path}"
        descriptor, fingerprint = pin_regular_metadata(gate_path, label)
        try:
            require_production_root_custody(descriptor, label)
        except BaseException:
            os.close(descriptor)
            raise
        if (
            hash_pinned_descriptor(
                descriptor, fingerprint, MAX_READINESS_GATE_BYTES, label
            )
            != trusted_gate_sha256
        ):
            os.close(descriptor)
            raise ValueError("promotion gate differs from its reviewed SHA-256")
        trusted_file_pins.append((gate_path, descriptor, fingerprint, label))
        label = f"release policy {policy}"
        descriptor, fingerprint = pin_regular_metadata(policy, label)
        try:
            require_production_root_custody(descriptor, label)
        except BaseException:
            os.close(descriptor)
            raise
        trusted_file_pins.append((policy, descriptor, fingerprint, label))
        policy_sha256 = hash_pinned_descriptor(
            descriptor, fingerprint, 64 * 1024, label
        )
        label = f"Kagami release verifier {verifier}"
        descriptor, fingerprint = pin_regular_metadata(verifier, label)
        try:
            require_production_root_custody(descriptor, label)
        except BaseException:
            os.close(descriptor)
            raise
        if fingerprint[4] > MAX_KAGAMI_VERIFIER_BYTES or not fingerprint[3] & 0o111:
            os.close(descriptor)
            raise ValueError(
                "Kagami release verifier must be executable and within its size limit"
            )
        if (
            hash_pinned_descriptor(
                descriptor, fingerprint, MAX_KAGAMI_VERIFIER_BYTES, label
            )
            != verifier_sha256
        ):
            os.close(descriptor)
            raise ValueError("Kagami release verifier differs from its trusted SHA-256")
        trusted_file_pins.append((verifier, descriptor, fingerprint, label))
        verifier_snapshot, verifier_exec = snapshot_pinned_executable(
            descriptor, fingerprint, label, PROMOTION_STAGING_PARENT
        )
        snapshot_root = verifier_exec.parent
        snapshot_label = f"private Kagami verifier snapshot directory {snapshot_root}"
        snapshot_descriptor, snapshot_fingerprint = pin_directory_metadata(
            snapshot_root, snapshot_label
        )
        catalog_directory_pins.append(
            (
                snapshot_root,
                snapshot_descriptor,
                snapshot_fingerprint,
                snapshot_label,
            )
        )
        snapshot_label = f"private Kagami verifier snapshot {verifier_exec}"
        snapshot_descriptor, snapshot_fingerprint = pin_regular_metadata(
            verifier_exec, snapshot_label
        )
        if (
            hash_pinned_descriptor(
                snapshot_descriptor,
                snapshot_fingerprint,
                MAX_KAGAMI_VERIFIER_BYTES,
                snapshot_label,
            )
            != verifier_sha256
        ):
            os.close(snapshot_descriptor)
            raise ValueError("private Kagami verifier snapshot digest changed")
        trusted_file_pins.append(
            (
                verifier_exec,
                snapshot_descriptor,
                snapshot_fingerprint,
                snapshot_label,
            )
        )
        label = f"authenticated tool isolation controller {tool_controller}"
        descriptor, fingerprint = pin_regular_metadata(tool_controller, label)
        try:
            require_production_root_custody(descriptor, label)
        except BaseException:
            os.close(descriptor)
            raise
        if fingerprint[4] > MAX_KAGAMI_VERIFIER_BYTES or not fingerprint[3] & 0o111:
            os.close(descriptor)
            raise ValueError(
                "authenticated tool isolation controller must be executable and "
                "within its size limit"
            )
        if (
            hash_pinned_descriptor(
                descriptor,
                fingerprint,
                MAX_KAGAMI_VERIFIER_BYTES,
                label,
            )
            != tool_controller_sha256
        ):
            os.close(descriptor)
            raise ValueError(
                "authenticated tool isolation controller differs from its trusted SHA-256"
            )
        trusted_file_pins.append(
            (tool_controller, descriptor, fingerprint, label)
        )
        tool_controller_snapshot, tool_controller_exec = snapshot_pinned_executable(
            descriptor,
            fingerprint,
            label,
            PROMOTION_STAGING_PARENT,
        )
        controller_snapshot_root = tool_controller_exec.parent
        controller_snapshot_label = (
            "private authenticated tool controller snapshot directory "
            f"{controller_snapshot_root}"
        )
        controller_snapshot_descriptor, controller_snapshot_fingerprint = (
            pin_directory_metadata(
                controller_snapshot_root,
                controller_snapshot_label,
            )
        )
        catalog_directory_pins.append(
            (
                controller_snapshot_root,
                controller_snapshot_descriptor,
                controller_snapshot_fingerprint,
                controller_snapshot_label,
            )
        )
        controller_snapshot_label = (
            f"private authenticated tool controller snapshot {tool_controller_exec}"
        )
        controller_snapshot_descriptor, controller_snapshot_fingerprint = (
            pin_regular_metadata(tool_controller_exec, controller_snapshot_label)
        )
        if (
            hash_pinned_descriptor(
                controller_snapshot_descriptor,
                controller_snapshot_fingerprint,
                MAX_KAGAMI_VERIFIER_BYTES,
                controller_snapshot_label,
            )
            != tool_controller_sha256
        ):
            os.close(controller_snapshot_descriptor)
            raise ValueError(
                "private authenticated tool isolation controller snapshot digest changed"
            )
        trusted_file_pins.append(
            (
                tool_controller_exec,
                controller_snapshot_descriptor,
                controller_snapshot_fingerprint,
                controller_snapshot_label,
            )
        )
        label = f"promotion Python runtime {python_runtime}"
        descriptor, fingerprint = pin_regular_metadata(
            python_runtime, label, require_single_link=False
        )
        try:
            require_production_root_custody(descriptor, label)
        except BaseException:
            os.close(descriptor)
            raise
        if (
            hash_pinned_descriptor(
                descriptor, fingerprint, MAX_KAGAMI_VERIFIER_BYTES, label
            )
            != trusted_python_sha256
        ):
            os.close(descriptor)
            raise ValueError(
                "running promotion Python differs from its trusted SHA-256"
            )
        trusted_file_pins.append((python_runtime, descriptor, fingerprint, label))
        label = f"source-authentication Git {SOURCE_GIT}"
        descriptor, fingerprint = pin_regular_metadata(
            SOURCE_GIT, label, require_single_link=False
        )
        try:
            require_production_root_custody(descriptor, label)
        except BaseException:
            os.close(descriptor)
            raise
        if not fingerprint[3] & 0o111:
            os.close(descriptor)
            raise ValueError("source-authentication Git is not executable")
        trusted_file_pins.append((SOURCE_GIT, descriptor, fingerprint, label))
        reviewed_closure_bytes: bytes | None = None
        reviewed_source_commit: str | None = None
        if reviewed_closure is not None:
            label = f"reviewed source closure {reviewed_closure}"
            descriptor, fingerprint = pin_regular_metadata(reviewed_closure, label)
            try:
                require_production_root_custody(descriptor, label)
            except BaseException:
                os.close(descriptor)
                raise
            reviewed_closure_bytes = read_pinned_descriptor(
                descriptor,
                fingerprint,
                MAX_REVIEWED_SOURCE_CLOSURE_BYTES,
                label,
            )
            if hashlib.sha256(reviewed_closure_bytes).hexdigest() != reviewed_closure_sha256:
                os.close(descriptor)
                raise ValueError("reviewed source closure differs from its trusted SHA-256")
            trusted_file_pins.append(
                (reviewed_closure, descriptor, fingerprint, label)
            )
            _, reviewed_source_commit = reviewed_source_commit_from_closure(
                reviewed_closure_bytes
            )
            closure_snapshot, trusted_closure_snapshot = snapshot_private_bytes(
                reviewed_closure_bytes,
                "reviewed-source-closure.json",
                "reviewed source closure",
                PROMOTION_STAGING_PARENT,
            )
        if (
            reviewed_closure_bytes is None
            or reviewed_source_commit is None
            or allowed_signers is None
            or revocation is None
            or source_projection is None
        ):
            raise ValueError("reviewed source trust inputs are incomplete")
        label = f"source SSH allowed-signers policy {allowed_signers}"
        descriptor, fingerprint = pin_regular_metadata(allowed_signers, label)
        try:
            require_production_root_custody(descriptor, label)
        except BaseException:
            os.close(descriptor)
            raise
        allowed_signers_bytes = read_pinned_descriptor(
            descriptor,
            fingerprint,
            MAX_SOURCE_ALLOWED_SIGNERS_BYTES,
            label,
        )
        if hashlib.sha256(allowed_signers_bytes).hexdigest() != allowed_signers_sha256:
            os.close(descriptor)
            raise ValueError("SSH allowed-signers policy differs from its trusted SHA-256")
        trusted_file_pins.append((allowed_signers, descriptor, fingerprint, label))
        allowed_signers_snapshot, trusted_allowed_signers_snapshot = (
            snapshot_private_bytes(
                allowed_signers_bytes,
                "allowed-signers",
                "source SSH allowed-signers policy",
                PROMOTION_STAGING_PARENT,
            )
        )
        label = f"source SSH revocation policy {revocation}"
        descriptor, fingerprint = pin_regular_metadata(
            revocation, label, allow_empty=True
        )
        try:
            require_production_root_custody(descriptor, label)
        except BaseException:
            os.close(descriptor)
            raise
        revocation_bytes = read_pinned_descriptor(
            descriptor,
            fingerprint,
            MAX_SOURCE_REVOCATION_BYTES,
            label,
            allow_empty=True,
        )
        if hashlib.sha256(revocation_bytes).hexdigest() != revocation_sha256:
            os.close(descriptor)
            raise ValueError("SSH revocation policy differs from its trusted SHA-256")
        trusted_file_pins.append((revocation, descriptor, fingerprint, label))
        revocation_snapshot, trusted_revocation_snapshot = snapshot_private_bytes(
            revocation_bytes,
            "revocation",
            "source SSH revocation policy",
            PROMOTION_STAGING_PARENT,
            allow_empty=True,
        )
        label = f"authenticated source-seal projection {source_projection}"
        descriptor, fingerprint = pin_regular_metadata(source_projection, label)
        try:
            require_production_root_custody(descriptor, label)
        except BaseException:
            os.close(descriptor)
            raise
        source_projection_bytes = read_pinned_descriptor(
            descriptor,
            fingerprint,
            MAX_SOURCE_SEAL_PROJECTION_BYTES,
            label,
        )
        if hashlib.sha256(source_projection_bytes).hexdigest() != source_projection_sha256:
            os.close(descriptor)
            raise ValueError(
                "authenticated source-seal projection differs from its trusted SHA-256"
            )
        trusted_file_pins.append((source_projection, descriptor, fingerprint, label))
        source_projection_snapshot, trusted_source_projection_snapshot = snapshot_private_bytes(
            source_projection_bytes,
            "authenticated-source-seal-projection.json",
            "authenticated source-seal projection",
            PROMOTION_STAGING_PARENT,
        )
        if sealed_build_report is None:
            raise ValueError("sealed candidate double-build report is unavailable")
        label = f"sealed candidate double-build report {sealed_build_report}"
        descriptor, fingerprint = pin_regular_metadata(sealed_build_report, label)
        try:
            require_production_root_custody(descriptor, label)
        except BaseException:
            os.close(descriptor)
            raise
        sealed_build_report_bytes = read_pinned_descriptor(
            descriptor,
            fingerprint,
            MAX_SEALED_BUILD_REPORT_BYTES,
            label,
        )
        if (
            hashlib.sha256(sealed_build_report_bytes).hexdigest()
            != sealed_build_report_sha256
        ):
            os.close(descriptor)
            raise ValueError(
                "sealed candidate double-build report differs from its trusted SHA-256"
            )
        sealed_build_report_identity = validate_sealed_build_report(
            sealed_build_report_bytes
        )
        validate_native_build_launch_binding(
            sealed_build_report_identity,
            tool_controller_sha256,
            trusted_python_sha256,
            trusted_python_runtime_tree_sha256,
            trusted_native_macos_build,
            trusted_native_os_tcb_sha256,
        )
        trusted_file_pins.append(
            (sealed_build_report, descriptor, fingerprint, label)
        )
        if len(projection_verification_inputs) != len(
            SOURCE_SEAL_VERIFICATION_INPUTS
        ):
            raise ValueError(
                "source-projection reconstruction inputs are incomplete"
            )
        for (
            input_key,
            input_path,
            expected_sha256,
            snapshot_name,
            maximum_bytes,
            allow_empty,
        ) in (
            (key, *projection_verification_inputs[key])
            for key, *_ in SOURCE_SEAL_VERIFICATION_INPUTS
        ):
            label = f"source-projection reconstruction {input_key} {input_path}"
            descriptor, fingerprint = pin_regular_metadata(
                input_path, label, allow_empty=allow_empty
            )
            try:
                require_production_root_custody(descriptor, label)
            except BaseException:
                os.close(descriptor)
                raise
            input_bytes = read_pinned_descriptor(
                descriptor,
                fingerprint,
                maximum_bytes,
                label,
                allow_empty=allow_empty,
            )
            if hashlib.sha256(input_bytes).hexdigest() != expected_sha256:
                os.close(descriptor)
                raise ValueError(
                    f"source-projection reconstruction {input_key} differs from its trusted SHA-256"
                )
            trusted_file_pins.append((input_path, descriptor, fingerprint, label))
            snapshot, trusted_snapshot = snapshot_private_bytes(
                input_bytes,
                snapshot_name,
                f"source-projection reconstruction {input_key}",
                PROMOTION_STAGING_PARENT,
                allow_empty=allow_empty,
            )
            projection_verification_snapshots.append(snapshot)
            trusted_projection_verification_inputs[input_key] = trusted_snapshot
        source_projection_identity = validate_source_trust_projection(
            source_projection_bytes,
            reviewed_closure_bytes,
            reviewed_closure_sha256,
            reviewed_source_commit,
            allowed_signers_sha256,
            revocation_sha256,
        )
        if sealed_build_report_identity is None:
            raise ValueError("sealed candidate double-build report was not authenticated")
        if (
            sealed_build_report_identity.get(
                "authenticated_source_seal_projection_sha256"
            )
            != source_projection_sha256
            or sealed_build_report_identity.get(
                "reviewed_source_closure_descriptor_sha256"
            )
            != reviewed_closure_sha256
            or sealed_build_report_identity.get("reviewed_cargo_binary_sha256")
            != source_projection_identity.get("cargo_binary_sha256")
            or sealed_build_report_identity.get("reviewed_rustc_binary_sha256")
            != source_projection_identity.get("rustc_binary_sha256")
            or sealed_build_report_identity.get("source_commit")
            != reviewed_source_commit
            or sealed_build_report_identity.get("source_tree_sha256")
            != source_identity.get("source_tree_sha256")
        ):
            raise ValueError(
                "sealed candidate double-build report differs from the authenticated source/build closure"
            )
        if (
            trusted_allowed_signers_snapshot is None
            or trusted_revocation_snapshot is None
        ):
            raise ValueError("source SSH trust-policy snapshots are incomplete")
        source_trust_config = isolated_source_trust_git_config(
            trusted_allowed_signers_snapshot,
            trusted_revocation_snapshot,
        )
        source_trust_config_snapshot, trusted_source_trust_config = (
            snapshot_private_bytes(
                source_trust_config,
                ".gitconfig",
                "isolated source SSH trust Git config",
                PROMOTION_STAGING_PARENT,
            )
        )
        trusted_source_trust_home = trusted_source_trust_config.parent
        if reviewed_source_commit is None:
            raise ValueError("reviewed source closure cannot authenticate its helper")
        source_helper_bytes = pin_authenticated_reviewed_source_file(
            SOURCE_TREE_SEAL,
            reviewed_source_commit,
            MAX_REVIEWED_HELPER_BYTES,
            f"reviewed source-tree seal helper {source_helper_path}",
            trusted_file_pins,
        )
        for relative in READINESS_SOURCE_PROVIDERS:
            path = root / relative
            authenticated_readiness_source_contract_bytes[relative] = (
                pin_authenticated_reviewed_source_file(
                    relative,
                    reviewed_source_commit,
                    MAX_READINESS_SOURCE_CONTRACT_BYTES,
                    f"reviewed readiness source provider {path}",
                    trusted_file_pins,
                )
            )
        if self_test:
            authenticated_readiness_self_test_bytes = pin_authenticated_reviewed_source_file(
                READINESS_SELF_TEST,
                reviewed_source_commit,
                MAX_READINESS_SELF_TEST_BYTES,
                f"reviewed readiness self-test helper {readiness_self_test_path}",
                trusted_file_pins,
            )
        source_helper_snapshot, trusted_source_helper_snapshot = snapshot_private_bytes(
            source_helper_bytes,
            "kagemusha_source_tree_seal.py",
            "reviewed source-tree seal helper",
            PROMOTION_STAGING_PARENT,
        )
        if (
            trusted_closure_snapshot is None
            or trusted_source_helper_snapshot is None
            or trusted_source_trust_home is None
        ):
            raise ValueError("reviewed source helper snapshots are incomplete")
        source_identity_result = subprocess.run(
            [
                str(python_runtime),
                "-I",
                "-S",
                str(trusted_source_helper_snapshot),
                "identity",
                "--root",
                str(root),
                "--reviewed-source-closure",
                str(trusted_closure_snapshot),
                "--reviewed-source-closure-sha256",
                reviewed_closure_sha256,
            ],
            cwd=Path("/"),
            env={
                "HOME": str(trusted_source_trust_home),
                "LANG": "C",
                "LC_ALL": "C",
                "PATH": "/usr/bin:/bin",
                "TMPDIR": str(PROMOTION_STAGING_PARENT),
                "TZ": "UTC",
            },
            stdin=subprocess.DEVNULL,
            check=False,
            capture_output=True,
            close_fds=True,
        )
        if source_identity_result.returncode != 0:
            raise ValueError(
                "promotion source differs from the independently pinned reviewed closure"
            )
        parsed_identity = strict_json_bytes(
            source_identity_result.stdout, "promotion reviewed source identity"
        )
        if (
            set(parsed_identity)
            != {
                "reviewed_source_closure",
                "reviewed_source_closure_descriptor_sha256",
                "schema",
                "source_commit",
                "source_repo_dirty",
                "source_tree_sha256",
            }
            or parsed_identity.get("schema")
            != "iroha.kagemusha.reviewed_source_tree_identity.v1"
            or parsed_identity.get("source_repo_dirty") is not False
            or parsed_identity.get("source_commit") != reviewed_source_commit
            or parsed_identity.get("reviewed_source_closure_descriptor_sha256")
            != reviewed_closure_sha256
            or parsed_identity.get("reviewed_source_closure")
            != strict_json_bytes(
                reviewed_closure_bytes, "reviewed source closure"
            )
        ):
            raise ValueError("promotion reviewed source identity is not exact")
        source_identity = parsed_identity
        producer_package_payloads = {SOURCE_TREE_SEAL: source_helper_bytes}
        for relative in SOURCE_PROJECTION_PRODUCER_CLOSURE:
            if relative == SOURCE_TREE_SEAL:
                continue
            helper_path = root / relative
            label = f"reviewed source-projection helper {helper_path}"
            descriptor, fingerprint = pin_regular_metadata(helper_path, label)
            try:
                require_production_root_custody(descriptor, label)
            except BaseException:
                os.close(descriptor)
                raise
            helper_bytes = read_pinned_descriptor(
                descriptor, fingerprint, MAX_REVIEWED_HELPER_BYTES, label
            )
            trusted_file_pins.append((helper_path, descriptor, fingerprint, label))
            authenticate_reviewed_source_file(
                relative,
                helper_bytes,
                reviewed_source_commit,
                MAX_REVIEWED_HELPER_BYTES,
            )
            if relative == "scripts/build_kagemusha_v4_candidate_bundle.py":
                if sealed_build_report_identity is None:
                    raise ValueError(
                        "sealed candidate double-build report was not authenticated"
                    )
                validate_native_builder_entrypoint_binding(
                    sealed_build_report_identity, helper_bytes
                )
            producer_package_payloads[relative] = helper_bytes
        source_producer_package_snapshot, producer_package_root = (
            snapshot_private_python_package(
                producer_package_payloads,
                "reviewed source-projection producer closure",
                PROMOTION_STAGING_PARENT,
            )
        )
        trusted_source_projection_launcher = (
            producer_package_root
            / "scripts/run_kagemusha_source_projection_snapshot.py"
        )
        if (
            trusted_closure_snapshot is None
            or trusted_source_projection_snapshot is None
            or trusted_source_trust_home is None
            or trusted_source_projection_launcher is None
            or source_projection_identity is None
            or set(trusted_projection_verification_inputs)
            != {key for key, *_ in SOURCE_SEAL_VERIFICATION_INPUTS}
        ):
            raise ValueError("source-projection reconstruction state is incomplete")
        verification_result = subprocess.run(
            [
                str(python_runtime),
                "-I",
                "-S",
                str(trusted_source_projection_launcher),
                str(producer_package_root),
                "verify",
                "--root",
                str(root),
                "--reviewed-source-closure",
                str(trusted_closure_snapshot),
                "--reviewed-source-closure-sha256",
                reviewed_closure_sha256,
                "--authorization",
                str(trusted_projection_verification_inputs["authorization"]),
                "--authorization-sha256",
                projection_verification_inputs["authorization"][1],
                "--controller-signature",
                str(
                    trusted_projection_verification_inputs[
                        "controller_signature"
                    ]
                ),
                "--controller-signature-sha256",
                projection_verification_inputs["controller_signature"][1],
                "--controller-allowed-signers",
                str(
                    trusted_projection_verification_inputs[
                        "controller_allowed_signers"
                    ]
                ),
                "--controller-allowed-signers-sha256",
                projection_verification_inputs["controller_allowed_signers"][1],
                "--controller-revocation",
                str(
                    trusted_projection_verification_inputs[
                        "controller_revocation"
                    ]
                ),
                "--controller-revocation-sha256",
                projection_verification_inputs["controller_revocation"][1],
                "--execution-policy",
                str(trusted_projection_verification_inputs["execution_policy"]),
                "--execution-policy-sha256",
                projection_verification_inputs["execution_policy"][1],
                "--raw-unit-graph",
                str(trusted_projection_verification_inputs["raw_unit_graph"]),
                "--raw-unit-graph-sha256",
                projection_verification_inputs["raw_unit_graph"][1],
                "--unit-graph",
                str(trusted_projection_verification_inputs["unit_graph"]),
                "--unit-graph-sha256",
                projection_verification_inputs["unit_graph"][1],
                "--projection",
                str(trusted_source_projection_snapshot),
                "--projection-sha256",
                source_projection_sha256,
            ],
            cwd=Path("/"),
            env={
                "HOME": str(trusted_source_trust_home),
                "LANG": "C",
                "LC_ALL": "C",
                "PATH": "/usr/bin:/bin",
                "TMPDIR": str(PROMOTION_STAGING_PARENT),
                "TZ": "UTC",
            },
            stdin=subprocess.DEVNULL,
            check=False,
            capture_output=True,
            close_fds=True,
            timeout=120,
        )
        if verification_result.returncode != 0:
            raise ValueError(
                "authenticated source-seal projection did not reconstruct from its signed inputs"
            )
        verification_receipt = strict_json_bytes(
            verification_result.stdout,
            "source-projection reconstruction receipt",
        )
        if set(verification_receipt) != {
            "authorization_sha256",
            "build_inputs_sha256",
            "cargo_binary_sha256",
            "controller_allowed_signers_sha256",
            "controller_principal",
            "controller_public_key_sha256",
            "controller_revocation_sha256",
            "controller_signature_sha256",
            "execution_policy_sha256",
            "projection_hex",
            "projection_sha256",
            "raw_unit_graph_sha256",
            "raw_unit_graph_size_bytes",
            "rustc_binary_sha256",
            "schema",
            "source_commit",
            "source_tree_sha256",
            "unit_graph_sha256",
        }:
            raise ValueError("source-projection reconstruction receipt fields are not exact")
        expected_receipt_values = {
            "authorization_sha256": projection_verification_inputs[
                "authorization"
            ][1],
            "build_inputs_sha256": source_projection_identity[
                "build_inputs_sha256"
            ],
            "cargo_binary_sha256": source_projection_identity[
                "cargo_binary_sha256"
            ],
            "controller_allowed_signers_sha256": projection_verification_inputs[
                "controller_allowed_signers"
            ][1],
            "controller_revocation_sha256": projection_verification_inputs[
                "controller_revocation"
            ][1],
            "controller_signature_sha256": projection_verification_inputs[
                "controller_signature"
            ][1],
            "execution_policy_sha256": source_projection_identity[
                "execution_policy_sha256"
            ],
            "projection_hex": source_projection_bytes.hex(),
            "projection_sha256": source_projection_sha256,
            "raw_unit_graph_sha256": source_projection_identity[
                "raw_unit_graph_sha256"
            ],
            "raw_unit_graph_size_bytes": source_projection_identity[
                "raw_unit_graph_size_bytes"
            ],
            "rustc_binary_sha256": source_projection_identity[
                "rustc_binary_sha256"
            ],
            "schema": "iroha.kagemusha.source_seal_projection_production_receipt.v1",
            "source_commit": reviewed_source_commit,
            "source_tree_sha256": source_identity["source_tree_sha256"],
            "unit_graph_sha256": source_projection_identity[
                "normalized_unit_graph_sha256"
            ],
        }
        if any(
            verification_receipt.get(field) != expected
            for field, expected in expected_receipt_values.items()
        ):
            raise ValueError(
                "source-projection reconstruction receipt differs from the signed inputs"
            )
        canonical_nonzero_sha256(
            verification_receipt.get("controller_public_key_sha256"),
            "source-projection controller public key",
        )
        controller_principal = verification_receipt.get("controller_principal")
        if (
            not isinstance(controller_principal, str)
            or not controller_principal
            or len(controller_principal) > 128
            or not controller_principal.isascii()
        ):
            raise ValueError("source-projection controller principal is invalid")
        if ios_configuration is not None:
            public_key = ios_configuration[2]
            label = f"physical-iOS trusted public key {public_key}"
            descriptor, fingerprint = pin_regular_metadata(public_key, label)
            try:
                require_production_root_custody(descriptor, label)
            except BaseException:
                os.close(descriptor)
                raise
            if fingerprint[4] > 64 * 1024:
                os.close(descriptor)
                raise ValueError("physical-iOS trusted public key is oversized")
            public_key_bytes = read_pinned_descriptor(
                descriptor, fingerprint, 64 * 1024, label
            )
            trusted_file_pins.append((public_key, descriptor, fingerprint, label))
            public_key_snapshot, trusted_public_key_snapshot = snapshot_private_bytes(
                public_key_bytes,
                "trusted-physical-ios-public-key.pem",
                "physical-iOS trusted public key",
                PROMOTION_STAGING_PARENT,
            )
            freshness_public_key = ios_configuration[5]
            label = (
                "physical-iOS online freshness authority public key "
                f"{freshness_public_key}"
            )
            descriptor, fingerprint = pin_regular_metadata(
                freshness_public_key, label
            )
            try:
                require_production_root_custody(descriptor, label)
            except BaseException:
                os.close(descriptor)
                raise
            if fingerprint[4] > 64 * 1024:
                os.close(descriptor)
                raise ValueError(
                    "physical-iOS online freshness authority public key is oversized"
                )
            freshness_public_key_bytes = read_pinned_descriptor(
                descriptor, fingerprint, 64 * 1024, label
            )
            trusted_file_pins.append(
                (freshness_public_key, descriptor, fingerprint, label)
            )
            (
                freshness_public_key_snapshot,
                trusted_freshness_public_key_snapshot,
            ) = snapshot_private_bytes(
                freshness_public_key_bytes,
                "trusted-physical-ios-freshness-authority-public-key.pem",
                "physical-iOS online freshness authority public key",
                PROMOTION_STAGING_PARENT,
            )
            catalog_revalidation_receipt = ios_configuration[7]
            label = (
                "physical-iOS catalog revalidation receipt "
                f"{catalog_revalidation_receipt}"
            )
            descriptor, fingerprint = pin_regular_metadata(
                catalog_revalidation_receipt, label
            )
            try:
                require_production_root_custody(descriptor, label)
            except BaseException:
                os.close(descriptor)
                raise
            catalog_revalidation_receipt_bytes = read_pinned_descriptor(
                descriptor, fingerprint, 256 * 1024, label
            )
            trusted_file_pins.append(
                (
                    catalog_revalidation_receipt,
                    descriptor,
                    fingerprint,
                    label,
                )
            )
            (
                catalog_revalidation_receipt_snapshot,
                trusted_catalog_revalidation_receipt_snapshot,
            ) = snapshot_private_bytes(
                catalog_revalidation_receipt_bytes,
                "catalog-revalidation-receipt-v1.json",
                "physical-iOS catalog revalidation receipt",
                PROMOTION_STAGING_PARENT,
            )
            production_ios_policy = ios_configuration[3]
            label = f"physical-iOS production policy {production_ios_policy}"
            descriptor, fingerprint = pin_regular_metadata(
                production_ios_policy, label
            )
            try:
                require_production_root_custody(descriptor, label)
            except BaseException:
                os.close(descriptor)
                raise
            production_ios_policy_bytes = read_pinned_descriptor(
                descriptor, fingerprint, 1024 * 1024, label
            )
            trusted_file_pins.append(
                (production_ios_policy, descriptor, fingerprint, label)
            )
            ios_policy_snapshot, trusted_ios_policy_snapshot = snapshot_private_bytes(
                production_ios_policy_bytes,
                "production-ios-policy-v1.json",
                "physical-iOS production policy",
                PROMOTION_STAGING_PARENT,
            )
            label = f"reviewed physical-iOS evidence validator {ios_validator_path}"
            descriptor, fingerprint = pin_regular_metadata(ios_validator_path, label)
            try:
                require_production_root_custody(descriptor, label)
            except BaseException:
                os.close(descriptor)
                raise
            validator_bytes = read_pinned_descriptor(
                descriptor, fingerprint, 4 * 1024 * 1024, label
            )
            trusted_file_pins.append(
                (ios_validator_path, descriptor, fingerprint, label)
            )
            authenticate_reviewed_source_file(
                IOS_EVIDENCE_MODULE,
                validator_bytes,
                reviewed_source_commit,
                MAX_REVIEWED_HELPER_BYTES,
            )
            ios_validator_snapshot, trusted_ios_validator_snapshot = (
                snapshot_private_bytes(
                    validator_bytes,
                    "kagemusha_candidate_ios_evidence.py",
                    "reviewed physical-iOS evidence validator",
                    PROMOTION_STAGING_PARENT,
                )
            )
            label = (
                "reviewed production physical-iOS evidence validator "
                f"{production_ios_validator_path}"
            )
            descriptor, fingerprint = pin_regular_metadata(
                production_ios_validator_path, label
            )
            try:
                require_production_root_custody(descriptor, label)
            except BaseException:
                os.close(descriptor)
                raise
            production_validator_bytes = read_pinned_descriptor(
                descriptor, fingerprint, 4 * 1024 * 1024, label
            )
            trusted_file_pins.append(
                (production_ios_validator_path, descriptor, fingerprint, label)
            )
            authenticate_reviewed_source_file(
                PRODUCTION_IOS_EVIDENCE_MODULE,
                production_validator_bytes,
                reviewed_source_commit,
                MAX_REVIEWED_HELPER_BYTES,
            )
            (
                production_ios_validator_snapshot,
                trusted_production_ios_validator_snapshot,
            ) = snapshot_private_bytes(
                production_validator_bytes,
                "kagemusha_production_ios_evidence.py",
                "reviewed production physical-iOS evidence validator",
                PROMOTION_STAGING_PARENT,
            )
            ios_validator, ios_catalog_validator = load_ios_evidence_validator(
                validator_bytes,
                trusted_ios_validator_snapshot,
                production_validator_bytes,
                trusted_production_ios_validator_snapshot,
            )
    except Exception as error:
        for _, descriptor, _, _ in trusted_file_pins:
            os.close(descriptor)
        for _, descriptor, _, _ in catalog_directory_pins:
            os.close(descriptor)
        cleanup_private_snapshots()
        errors.append(f"promotion release trust path is not pinned: {error}")
        return errors
    authenticated_verification_allowed = not errors
    directories = []
    for path in artifact_root.iterdir():
        directories.append(path)
        if len(directories) > MAX_RELEASE_DIRECTORIES:
            errors.append(
                f"promotion artifact root exceeds {MAX_RELEASE_DIRECTORIES} releases"
            )
            for _, descriptor, _, _ in catalog_directory_pins:
                os.close(descriptor)
            for _, descriptor, _, _ in trusted_file_pins:
                os.close(descriptor)
            cleanup_private_snapshots()
            return errors
    directories.sort()
    if not directories:
        errors.append("promotion artifact root contains no manifest-digest releases")
        for _, descriptor, _, _ in catalog_directory_pins:
            os.close(descriptor)
        for _, descriptor, _, _ in trusted_file_pins:
            os.close(descriptor)
        cleanup_private_snapshots()
        return errors
    if ios_configuration is not None:
        ios_root = ios_configuration[0]
        ios_directories = []
        for path in ios_root.iterdir():
            ios_directories.append(path)
            if len(ios_directories) > MAX_RELEASE_DIRECTORIES:
                errors.append(
                    "physical-iOS evidence root exceeds "
                    f"{MAX_RELEASE_DIRECTORIES} releases"
                )
                for _, descriptor, _, _ in catalog_directory_pins:
                    os.close(descriptor)
                for _, descriptor, _, _ in trusted_file_pins:
                    os.close(descriptor)
                cleanup_private_snapshots()
                return errors
        if {path.name for path in ios_directories} != {
            path.name for path in directories
        }:
            errors.append(
                "physical-iOS evidence root must contain exactly one "
                "manifest-digest directory for every promoted release"
            )
            for _, descriptor, _, _ in catalog_directory_pins:
                os.close(descriptor)
            for _, descriptor, _, _ in trusted_file_pins:
                os.close(descriptor)
            cleanup_private_snapshots()
            return errors
    expected_inventory = set(ARTIFACTS + FINAL_METADATA)
    catalog_aggregate_bytes = 0
    catalog_pins = trusted_file_pins
    ios_catalog_bindings: list[dict[str, str]] = []
    for directory in directories:
        directory_error_count = len(errors)
        ios_candidate_sha256: str | None = None
        promotion_record_sha256: str | None = None
        qualification_receipt_sha256: str | None = None
        internal_validation_receipt_sha256: str | None = None
        if not directory.is_dir() or directory.is_symlink() or not re.fullmatch(r"[0-9a-f]{64}", directory.name):
            errors.append(f"noncanonical release entry: {directory.name}")
            continue
        try:
            label = f"release directory {directory}"
            descriptor, fingerprint = pin_directory_metadata(directory, label)
            try:
                require_production_root_custody(descriptor, label)
            except BaseException:
                os.close(descriptor)
                raise
            catalog_directory_pins.append((directory, descriptor, fingerprint, label))
        except (OSError, ValueError) as error:
            errors.append(f"{directory.name}: release directory is not pinned: {error}")
            continue
        actual = set()
        for path in directory.iterdir():
            actual.add(path.name)
            if len(actual) > MAX_RELEASE_INVENTORY_ENTRIES:
                errors.append(f"{directory.name}: final release inventory is oversized")
                break
        if actual != expected_inventory:
            errors.append(f"{directory.name}: final release inventory is not exact")
            continue
        new_pins: list[tuple[Path, int, tuple[int, ...], str]] = []
        release_pins: dict[str, tuple[int, tuple[int, ...], str]] = {}
        try:
            release_bytes = 0
            for name in expected_inventory:
                path = directory / name
                label = f"{directory.name}/{name}"
                descriptor, fingerprint = pin_regular_metadata(path, label)
                try:
                    require_production_root_custody(descriptor, label)
                except BaseException:
                    os.close(descriptor)
                    raise
                new_pins.append((path, descriptor, fingerprint, label))
                release_pins[name] = (descriptor, fingerprint, label)
                release_bytes += fingerprint[4]
            catalog_aggregate_bytes = checked_catalog_aggregate_total(
                catalog_aggregate_bytes, release_bytes
            )
        except (OSError, ValueError) as error:
            for _, descriptor, _, _ in new_pins:
                os.close(descriptor)
            errors.append(f"{directory.name}: invalid catalog byte inventory: {error}")
            continue
        catalog_pins.extend(new_pins)
        try:
            descriptor, fingerprint, label = release_pins["manifest.norito"]
            manifest_bytes = read_pinned_descriptor(
                descriptor, fingerprint, MAX_MANIFEST_BYTES, label
            )
        except (OSError, ValueError) as error:
            errors.append(f"{directory.name}: invalid manifest.norito: {error}")
            continue
        digest = hashlib.sha256(manifest_bytes).hexdigest()
        if digest != directory.name:
            errors.append(f"{directory.name}: directory does not equal manifest SHA-256")
        try:
            descriptor, fingerprint, label = release_pins[
                "manifest.norito.sha256"
            ]
            sidecar = read_pinned_descriptor(
                descriptor,
                fingerprint,
                MAX_DIGEST_SIDECAR_BYTES,
                label,
            )
            if sidecar != f"{digest}\n".encode("ascii"):
                errors.append(f"{directory.name}: manifest digest sidecar is not canonical")
        except (OSError, ValueError) as error:
            errors.append(f"{directory.name}: invalid manifest digest sidecar: {error}")
        try:
            descriptor, fingerprint, label = release_pins["manifest.json"]
            manifest = strict_json_bytes(
                read_pinned_descriptor(
                    descriptor, fingerprint, MAX_MANIFEST_BYTES, label
                ),
                label,
            )
        except (OSError, UnicodeError, ValueError, json.JSONDecodeError) as error:
            errors.append(f"{directory.name}: invalid manifest JSON: {error}")
            continue
        if manifest.get("schema") != "kagemusha.offline.recursive_spend.artifact_manifest.v4":
            errors.append(f"{directory.name}: manifest schema is not V4")
        if manifest.get("bridge_abi_version") != 22 or manifest.get("source_repo_dirty") is not False:
            errors.append(f"{directory.name}: ABI/source-tree promotion binding is invalid")
        for field in (
            "authenticated_source_seal_projection_sha256",
            "reviewed_cargo_binary_sha256",
            "reviewed_rustc_binary_sha256",
            "generator_binary_sha256",
            "sealed_candidate_build_report_sha256",
        ):
            try:
                canonical_nonzero_sha256(
                    manifest.get(field), f"{directory.name} manifest {field}"
                )
            except ValueError as error:
                errors.append(str(error))
        if source_identity is not None and (
            manifest.get("authenticated_source_seal_projection_sha256")
            != source_projection_sha256
        ):
            errors.append(
                f"{directory.name}: manifest differs from the authenticated source-seal projection"
            )
        if source_projection_identity is not None and (
            manifest.get("reviewed_cargo_binary_sha256")
            != source_projection_identity.get("cargo_binary_sha256")
            or manifest.get("reviewed_rustc_binary_sha256")
            != source_projection_identity.get("rustc_binary_sha256")
        ):
            errors.append(
                f"{directory.name}: manifest build tools differ from the authenticated execution policy"
            )
        if sealed_build_report_identity is None:
            errors.append(
                f"{directory.name}: sealed candidate double-build report is unavailable"
            )
        else:
            candidate_generator = sealed_build_report_identity.get(
                "candidate_generator"
            )
            if (
                not isinstance(candidate_generator, dict)
                or manifest.get("generator_binary_sha256")
                != candidate_generator.get("sha256")
                or manifest.get("sealed_candidate_build_report_sha256")
                != sealed_build_report_sha256
            ):
                errors.append(
                    f"{directory.name}: manifest generator/report binding differs from the sealed double build"
                )
        if source_identity is not None and (
            manifest.get("source_commit") != source_identity.get("source_commit")
            or manifest.get("source_tree_sha256")
            != source_identity.get("source_tree_sha256")
            or manifest.get("reviewed_source_closure")
            != source_identity.get("reviewed_source_closure")
            or manifest.get("reviewed_source_closure_descriptor_sha256")
            != source_identity.get("reviewed_source_closure_descriptor_sha256")
        ):
            errors.append(
                f"{directory.name}: manifest differs from the pinned reviewed source closure"
            )
        profiles = manifest.get("profiles")
        roles = []
        if isinstance(profiles, list):
            for profile in profiles:
                if isinstance(profile, dict) and isinstance(profile.get("artifacts"), list):
                    roles.extend(profile["artifacts"])
        if len(roles) != 8:
            errors.append(f"{directory.name}: manifest does not bind exactly eight artifacts")
        declared_artifacts: dict[str, int] = {}
        for role in roles:
            if not isinstance(role, dict):
                continue
            name = role.get("file_name")
            size_bytes = role.get("size_bytes")
            if isinstance(name, str) and isinstance(size_bytes, int) and not isinstance(size_bytes, bool):
                declared_artifacts[name] = size_bytes
        if set(declared_artifacts) != set(ARTIFACTS):
            errors.append(f"{directory.name}: manifest artifact names are not exact")
        else:
            try:
                checked_declared_artifact_total(declared_artifacts)
            except ValueError as error:
                errors.append(f"{directory.name}: {error}")
            else:
                for name in ARTIFACTS:
                    try:
                        descriptor, fingerprint, label = release_pins[name]
                        prefix = inspect_pinned_prefix(
                            descriptor,
                            fingerprint,
                            declared_artifacts[name],
                            MAX_DECLARED_ARTIFACT_FILE_BYTES,
                            8,
                            label,
                        )
                        if prefix != b"KRV4KEY\0":
                            errors.append(f"{directory.name}/{name}: invalid KRV4 framing")
                    except (OSError, ValueError) as error:
                        errors.append(f"{directory.name}/{name}: invalid artifact: {error}")
        for name, maximum in BOUNDED_AUTHENTICATED_METADATA:
            try:
                descriptor, fingerprint, label = release_pins[name]
                payload = read_pinned_descriptor(
                    descriptor, fingerprint, maximum, label
                )
                if name == "promotion-record-v4.norito":
                    promotion_record_sha256 = hashlib.sha256(payload).hexdigest()
                elif name == "internal-validation-receipt-v1.norito":
                    internal_validation_receipt_sha256 = hashlib.sha256(payload).hexdigest()
            except (OSError, ValueError) as error:
                errors.append(f"{directory.name}/{name}: invalid evidence: {error}")
        evidence_bytes: bytes | None = None
        try:
            descriptor, fingerprint, label = release_pins[
                "physical-device-benchmark.evidence"
            ]
            evidence_bytes = read_pinned_descriptor(
                descriptor,
                fingerprint,
                MAX_BENCHMARK_EVIDENCE_BYTES,
                label,
            )
            if not evidence_bytes_are_non_placeholder(evidence_bytes):
                errors.append(
                    f"{directory.name}/physical-device-benchmark.evidence: "
                    "missing non-placeholder evidence bytes"
                )
        except (OSError, ValueError) as error:
            errors.append(
                f"{directory.name}/physical-device-benchmark.evidence: "
                f"invalid evidence: {error}"
            )
        try:
            # This is opaque proof-bearing Norito, not human-authored evidence.
            # Bound and pin it here; Kagami performs canonical authentication.
            descriptor, fingerprint, label = release_pins[
                "recursive-step-two-qualification-v4.norito"
            ]
            receipt = read_pinned_descriptor(
                descriptor,
                fingerprint,
                MAX_QUALIFICATION_RECEIPT_BYTES,
                label,
            )
            qualification_receipt_sha256 = hashlib.sha256(receipt).hexdigest()
        except (OSError, ValueError) as error:
            errors.append(
                f"{directory.name}/recursive-step-two-qualification-v4.norito: "
                f"invalid qualification receipt: {error}"
            )
        if (
            ios_configuration is not None
            and ios_validator is not None
            and evidence_bytes is not None
            and trusted_public_key_snapshot is not None
            and trusted_freshness_public_key_snapshot is not None
            and trusted_ios_policy_snapshot is not None
            and len(errors) == directory_error_count
        ):
            (
                ios_candidate_sha256,
                ios_catalog_binding,
                ios_error,
            ) = verify_ios_evidence(
                directory,
                ios_configuration,
                ios_validator,
                evidence_bytes,
                trusted_public_key_snapshot,
                trusted_ios_policy_snapshot,
                trusted_freshness_public_key_snapshot,
                catalog_directory_pins,
                trusted_file_pins,
                PROMOTION_STAGING_PARENT,
            )
            if ios_error is not None:
                errors.append(ios_error)
            elif ios_catalog_binding is None:
                errors.append(
                    f"{directory.name}: physical-iOS catalog binding is unavailable"
                )
            else:
                ios_catalog_bindings.append(ios_catalog_binding)
        if authenticated_verification_allowed and len(errors) == directory_error_count:
            if (
                ios_candidate_sha256 is None
                or promotion_record_sha256 is None
                or qualification_receipt_sha256 is None
                or internal_validation_receipt_sha256 is None
            ):
                errors.append(
                    f"{directory.name}: authenticated verification inputs are incomplete"
                )
                continue
            command = release_verifier_command(verifier_exec, directory, policy)
            try:
                verified = run_authenticated_verifier(command, tool_controller_exec)
            except ValueError as error:
                errors.append(
                    f"{directory.name}: authenticated V4 release verification failed: {error}"
                )
                continue
            if verified.returncode != 0:
                errors.append(
                    f"{directory.name}: authenticated V4 release verification failed: "
                    f"{authenticated_verifier_exit_diagnostic(verified.returncode)}"
                )
            else:
                try:
                    report = strict_json_bytes(
                        verified.stdout,
                        "Kagami V4 verification report",
                    )
                    validate_kagami_verification_report(
                        report,
                        directory=directory,
                        manifest=manifest,
                        policy_sha256=policy_sha256,
                        promotion_record_sha256=promotion_record_sha256,
                        qualification_receipt_sha256=qualification_receipt_sha256,
                        internal_validation_receipt_sha256=internal_validation_receipt_sha256,
                        ios_candidate_sha256=ios_candidate_sha256,
                    )
                except (UnicodeError, ValueError, json.JSONDecodeError) as error:
                    errors.append(
                        f"{directory.name}: authenticated verifier report is invalid: {error}"
                    )
    if ios_configuration is not None:
        if (
            ios_catalog_validator is None
            or trusted_catalog_revalidation_receipt_snapshot is None
            or trusted_freshness_public_key_snapshot is None
            or trusted_public_key_snapshot is None
        ):
            errors.append(
                "physical-iOS catalog revalidation inputs are incomplete"
            )
        else:
            catalog_revalidation_errors = ios_catalog_validator(
                trusted_catalog_revalidation_receipt_snapshot,
                ios_configuration[4],
                trusted_freshness_public_key_snapshot,
                ios_configuration[6],
                ios_catalog_bindings,
                ios_configuration[1],
                trusted_public_key_snapshot,
            )
            if catalog_revalidation_errors:
                errors.append(
                    "physical-iOS catalog revalidation failed: "
                    f"{catalog_revalidation_errors[-1]}"
                )
    for path, descriptor, fingerprint, label in catalog_pins:
        try:
            revalidate_pinned_metadata(path, descriptor, fingerprint, label)
        except (OSError, ValueError) as error:
            errors.append(f"invalid catalog byte inventory: {error}")
        finally:
            os.close(descriptor)
    for path, descriptor, fingerprint, label in reversed(catalog_directory_pins):
        try:
            revalidate_pinned_metadata(path, descriptor, fingerprint, label)
        except (OSError, ValueError) as error:
            errors.append(f"invalid catalog directory inventory: {error}")
        finally:
            os.close(descriptor)
    cleanup_private_snapshots()
    return errors
source_contract_errors: list[str] = []
source_contract_bytes: dict[str, bytes] = {}
if mode == "promotion":
    source_contract_errors.extend(promotion_errors())
    source_contract_bytes = authenticated_readiness_source_contract_bytes
    for relative in READINESS_SOURCE_PROVIDERS:
        if relative not in source_contract_bytes:
            source_contract_errors.append(
                f"promotion readiness source provider {relative} was not "
                "source-closure authenticated"
            )
else:
    for relative in READINESS_SOURCE_PROVIDERS:
        try:
            payload = (root / relative).read_bytes()
            if not payload or len(payload) > MAX_READINESS_SOURCE_CONTRACT_BYTES:
                raise ValueError("provider violates its byte bound")
            source_contract_bytes[relative] = payload
        except (OSError, ValueError) as error:
            source_contract_errors.append(
                f"could not load readiness source provider {relative}: {error}"
            )
support_bytes = source_contract_bytes.get(READINESS_SOURCE_SUPPORT)
source_provider_base_names = frozenset(globals()) | {
    "errors",
    "readiness_self_test_bytes",
    "self_test_context",
}
support_context = dict(globals())
if support_bytes is None:
    source_contract_errors.append("readiness source-support provider bytes are unavailable")
else:
    try:
        support_source = support_bytes.decode("utf-8")
        support_context["_KAGEMUSHA_READINESS_SOURCE_SUPPORT_CONTEXT_V1"] = True
        support_context["_KAGEMUSHA_READINESS_SOURCE_SUPPORT_SOURCE_V1"] = support_source
        exec(compile(support_bytes, READINESS_SOURCE_SUPPORT, "exec"), support_context)
    except Exception as error:
        source_contract_errors.append(
            f"readiness source-support provider failed unexpectedly: {error}"
        )
if not callable(support_context.get("source_provider_pipeline_errors")):
    source_contract_errors.append("readiness source-support provider evaluator is unavailable")
recursion_source_contract_evaluator: Callable[..., list[str]] | None = None
recursion_bytes = source_contract_bytes.get(READINESS_RECURSION_SOURCE_CONTRACT)
if recursion_bytes is None:
    source_contract_errors.append("recursion source-contract provider bytes are unavailable")
else:
    try:
        recursion_source = recursion_bytes.decode("utf-8")
        recursion_context = {
            "__name__": "kagemusha_recursion_source_contract",
            "_KAGEMUSHA_RECURSION_SOURCE_CONTRACT_CONTEXT_V1": True,
            "_KAGEMUSHA_RECURSION_SOURCE_CONTRACT_SOURCE_V1": recursion_source,
        }
        exec(
            compile(recursion_bytes, READINESS_RECURSION_SOURCE_CONTRACT, "exec"),
            recursion_context,
        )
        candidate_evaluator = recursion_context.get("recursion_source_contract_errors")
        if not callable(candidate_evaluator):
            raise ValueError("recursion source provider did not define its evaluator")
        recursion_source_contract_evaluator = candidate_evaluator
    except Exception as error:
        source_contract_errors.append(
            f"recursion source-contract provider failed unexpectedly: {error}"
        )
if not callable(recursion_source_contract_evaluator):
    source_contract_errors.append("recursion source-contract provider evaluator is unavailable")
lifecycle_source_contract_evaluator: Callable[..., list[str]] | None = None
lifecycle_bytes = source_contract_bytes.get(READINESS_LIFECYCLE_SOURCE_CONTRACT)
lifecycle_context: dict[str, object] = {}
if lifecycle_bytes is None:
    source_contract_errors.append("lifecycle source-contract provider bytes are unavailable")
else:
    try:
        lifecycle_source = lifecycle_bytes.decode("utf-8")
        lifecycle_context = {"__name__": "kagemusha_lifecycle_source_contract", "_KAGEMUSHA_LIFECYCLE_SOURCE_CONTRACT_CONTEXT_V1": True, "_KAGEMUSHA_LIFECYCLE_SOURCE_CONTRACT_SOURCE_V1": lifecycle_source}
        exec(compile(lifecycle_bytes, READINESS_LIFECYCLE_SOURCE_CONTRACT, "exec"), lifecycle_context)
        candidate_evaluator = lifecycle_context.get("lifecycle_source_contract_errors")
        if not callable(candidate_evaluator):
            raise ValueError("lifecycle source provider did not define its evaluator")
        lifecycle_source_contract_evaluator = candidate_evaluator
    except Exception as error:
        source_contract_errors.append(f"lifecycle source-contract provider failed unexpectedly: {error}")
if not callable(lifecycle_source_contract_evaluator):
    source_contract_errors.append("lifecycle source-contract provider evaluator is unavailable")
primary_bytes = source_contract_bytes.get(READINESS_SOURCE_CONTRACT)
source_contract_context = dict(support_context)
source_contract_context.update(lifecycle_context)
source_contract_context["__name__"] = "kagemusha_readiness_source_contract"
source_contract_context["_KAGEMUSHA_RECURSION_SOURCE_CONTRACT_EVALUATOR_V1"] = recursion_source_contract_evaluator
source_contract_context["_KAGEMUSHA_LIFECYCLE_SOURCE_CONTRACT_EVALUATOR_V1"] = lifecycle_source_contract_evaluator
source_contract_evaluator: Callable[..., list[str]] | None = None
if primary_bytes is None:
    source_contract_errors.append("readiness source-contract provider bytes are unavailable")
else:
    try:
        source_contract_source = primary_bytes.decode("utf-8")
        source_contract_context[
            "_KAGEMUSHA_READINESS_SOURCE_CONTRACT_CONTEXT_V1"
        ] = True
        source_contract_context[
            "_KAGEMUSHA_READINESS_SOURCE_CONTRACT_SOURCE_V1"
        ] = source_contract_source
        code = compile(
            primary_bytes,
            READINESS_SOURCE_CONTRACT,
            "exec",
        )
        exec(code, source_contract_context, source_contract_context)
        source_contract_evaluator = source_contract_context.get("static_errors")
        if not callable(source_contract_evaluator):
            raise ValueError(
                "readiness source-contract provider did not define static_errors"
            )
        source_contract_errors.extend(source_contract_evaluator())
    except Exception as error:
        source_contract_errors.append(
            f"readiness source-contract provider failed unexpectedly: {error}"
        )
if not callable(source_contract_evaluator):
    source_contract_errors.append("readiness source-contract provider evaluator is unavailable")
errors = source_contract_errors
if self_test:
    for provider_name, provider_value in source_contract_context.items():
        if provider_name not in source_provider_base_names:
            globals()[provider_name] = provider_value
    if mode == "promotion":
        readiness_self_test_bytes = authenticated_readiness_self_test_bytes
        if readiness_self_test_bytes is None:
            errors.append("promotion readiness self-test helper was not source-closure authenticated")
    else:
        try:
            readiness_self_test_bytes = (root / READINESS_SELF_TEST).read_bytes()
            if not readiness_self_test_bytes or len(readiness_self_test_bytes) > MAX_READINESS_SELF_TEST_BYTES:
                raise ValueError("candidate readiness self-test helper violates its byte bound")
        except (OSError, ValueError) as error:
            errors.append(f"could not load readiness self-test helper: {error}")
            readiness_self_test_bytes = None
    if readiness_self_test_bytes is not None:
        try:
            self_test_context = globals()
            self_test_context["_KAGEMUSHA_READINESS_SELF_TEST_CONTEXT_V1"] = True
            self_test_context["errors"] = errors
            code = compile(readiness_self_test_bytes, READINESS_SELF_TEST, "exec")
            exec(code, self_test_context, self_test_context)
        except Exception as error:
            errors.append(f"readiness self-test helper failed unexpectedly: {error}")
if errors:
    print(
        f"Kagemusha data ABI V4 (native bridge ABI 22) {mode} corridor failed:",
        file=sys.stderr,
    )
    for error in errors:
        print(f" - {error}", file=sys.stderr)
    raise SystemExit(1)
if mode == "candidate":
    print(
        "Kagemusha data ABI V4 (native bridge ABI 22) static candidate corridor passed; "
        "production promotion was not evaluated."
    )
else:
    print(
        "Kagemusha data ABI V4 (native bridge ABI 22) production promotion verification "
        "corridor passed; no publication or activation was performed."
    )
PY
PYTHON_GATE_STATUS=$?
set -e
if [[ "${MODE}" == "promotion" ]]; then
  OBSERVED_FINAL_PYTHON_RUNTIME_TREE_SHA256="$(
    promotion_root_tree_sha256 "${PYTHON_RUNTIME_ROOT}" \
      "promotion Python runtime closure"
  )" || exit 2
  if [[ "${OBSERVED_FINAL_PYTHON_RUNTIME_TREE_SHA256}" \
    != "${PYTHON_RUNTIME_TREE_SHA256}" ]]; then
    echo "promotion Python runtime closure changed during the production gate" >&2
    exit 2
  fi
fi
exit "${PYTHON_GATE_STATUS}"

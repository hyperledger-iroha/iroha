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
: "${IROHA_PRIVACY_AUTHENTICATED_CARGO_HOME:?privacy SDK Cargo wrapper requires an authenticated Cargo home}"
: "${IROHA_PRIVACY_AUTHENTICATED_CARGO_HOME_DIRECTORY_STATE:?privacy SDK Cargo wrapper requires authenticated Cargo home directory state}"
: "${IROHA_PRIVACY_AUTHENTICATED_CARGO_REGISTRY_LINK_STATE:?privacy SDK Cargo wrapper requires authenticated Cargo registry cache state}"
: "${IROHA_PRIVACY_AUTHENTICATED_CARGO_GIT_LINK_STATE:?privacy SDK Cargo wrapper requires authenticated Cargo git cache state}"
: "${IROHA_PRIVACY_AUTHENTICATED_CARGO_TARGET_DIR:?privacy SDK Cargo wrapper requires an authenticated Cargo target}"
: "${IROHA_PRIVACY_AUTHENTICATED_CARGO_TARGET_DIRECTORY_STATE:?privacy SDK Cargo wrapper requires authenticated Cargo target directory state}"
: "${IROHA_PRIVACY_AUTHENTICATED_CARGO_CONFIG_PATH:?privacy SDK Cargo wrapper requires an authenticated Cargo config path}"
: "${IROHA_PRIVACY_AUTHENTICATED_CARGO_CONFIG_SEAL:?privacy SDK Cargo wrapper requires an authenticated Cargo config seal}"
: "${IROHA_PRIVACY_AUTHENTICATED_RUST_TOOLCHAIN_PATH:?privacy SDK Cargo wrapper requires an authenticated Rust toolchain path}"
: "${IROHA_PRIVACY_AUTHENTICATED_RUST_TOOLCHAIN_SEAL:?privacy SDK Cargo wrapper requires an authenticated Rust toolchain seal}"
: "${IROHA_PRIVACY_AUTHENTICATED_RUST_TOOLCHAIN_SELECTOR:?privacy SDK Cargo wrapper requires an authenticated Rust toolchain selector}"
: "${IROHA_PRIVACY_AUTHENTICATED_RUSTUP_PATH:?privacy SDK Cargo wrapper requires an authenticated rustup path}"
: "${IROHA_PRIVACY_AUTHENTICATED_RUSTUP_SEAL:?privacy SDK Cargo wrapper requires an authenticated rustup seal}"
: "${IROHA_PRIVACY_AUTHENTICATED_CARGO_PATH:?privacy SDK Cargo wrapper requires an authenticated Cargo path}"
: "${IROHA_PRIVACY_AUTHENTICATED_CARGO_SEAL:?privacy SDK Cargo wrapper requires an authenticated Cargo seal}"
: "${IROHA_PRIVACY_AUTHENTICATED_RUSTC_PATH:?privacy SDK Cargo wrapper requires an authenticated rustc path}"
: "${IROHA_PRIVACY_AUTHENTICATED_RUSTC_SEAL:?privacy SDK Cargo wrapper requires an authenticated rustc seal}"
: "${IROHA_PRIVACY_AUTHENTICATED_RUSTDOC_PATH:?privacy SDK Cargo wrapper requires an authenticated rustdoc path}"
: "${IROHA_PRIVACY_AUTHENTICATED_RUSTDOC_SEAL:?privacy SDK Cargo wrapper requires an authenticated rustdoc seal}"

PYTHON_BIN="${IROHA_PRIVACY_LOCKFILE_PYTHON_BIN:-python3}"
REAL_CARGO_PATH="${IROHA_PRIVACY_REAL_CARGO}"
CARGO_AUDIT_PATH="${IROHA_PRIVACY_CARGO_AUDIT_PATH}"
CARGO_WRAPPER_ENTRY_PATH="$(
  cd "$(dirname "$0")" && printf '%s/%s\n' "$(pwd -P)" "$(basename "$0")"
)"
AUTHENTICATED_COMMAND_NAME=""
SELECTED_LOCKFILE="$(
  privacy_sdk_resolve_cargo_lockfile "${IROHA_PRIVACY_SDK_ROOT}" "${PYTHON_BIN}"
)"
if [[ "${SELECTED_LOCKFILE}" != "${IROHA_PRIVACY_AUTHENTICATED_CARGO_LOCKFILE_PATH}" ]]; then
  echo "error: privacy SDK Cargo lock selection does not match the authenticated path" >&2
  exit 1
fi
if [[ "${REAL_CARGO_PATH}" != "${IROHA_PRIVACY_AUTHENTICATED_CARGO_PATH}" ]]; then
  echo "error: privacy SDK real Cargo does not match the authenticated toolchain" >&2
  exit 1
fi

privacy_sdk_authenticated_host_triple() {
  case "${IROHA_PRIVACY_AUTHENTICATED_RUST_TOOLCHAIN_SELECTOR}" in
    1.93.1-x86_64-unknown-linux-gnu)
      printf '%s\n' "x86_64-unknown-linux-gnu"
      ;;
    1.93.1-aarch64-apple-darwin)
      printf '%s\n' "aarch64-apple-darwin"
      ;;
    *)
      echo "error: unsupported authenticated Rust host selector" >&2
      return 1
      ;;
  esac
}

assert_authenticated_maturin_pyo3_config() {
  local host_triple expected_path

  if [[ "${AUTHENTICATED_COMMAND_NAME}" != "rustc" || \
    -z "${IROHA_PRIVACY_AUTHENTICATED_CARGO_TARGET_DIR:-}" ]]; then
    echo "error: PYO3_CONFIG_FILE is only allowed for authenticated Maturin rustc" >&2
    return 1
  fi
  host_triple="$(privacy_sdk_authenticated_host_triple)" || return 1
  expected_path="${IROHA_PRIVACY_AUTHENTICATED_CARGO_TARGET_DIR}/maturin/pyo3-config-${host_triple}-3.12-abi3.txt"
  if [[ "${PYO3_CONFIG_FILE:-}" != "${expected_path}" || \
    "${IROHA_PRIVACY_AUTHENTICATED_MATURIN_PYO3_CONFIG_PATH:-}" != \
      "${expected_path}" ]]; then
    echo "error: Maturin PYO3_CONFIG_FILE path is not authenticated" >&2
    return 1
  fi
  "${PYTHON_BIN}" -I - \
    "${IROHA_PRIVACY_AUTHENTICATED_CARGO_TARGET_DIR}" \
    "${expected_path}" <<'PY'
import os
import stat
import sys
from pathlib import Path

expected = (
    b"implementation=CPython\n"
    b"version=3.9\n"
    b"target_abi=CPython-abi3-3.9\n"
    b"shared=true\n"
    b"build_flags=\n"
    b"suppress_build_script_link_lines=false"
)
target = Path(sys.argv[1])
path = Path(sys.argv[2])
try:
    canonical_target = target.resolve(strict=True)
    canonical_path = path.resolve(strict=True)
except OSError as error:
    raise SystemExit(f"error: authenticated PyO3 config is unavailable: {error}")
if canonical_target != target or canonical_path != path:
    raise SystemExit("error: authenticated PyO3 config path must be canonical")
if canonical_target not in canonical_path.parents:
    raise SystemExit("error: authenticated PyO3 config escaped the private target")
flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
try:
    descriptor = os.open(path, flags)
except OSError as error:
    raise SystemExit(f"error: unable to open authenticated PyO3 config: {error}")
try:
    before = os.fstat(descriptor)
    if (
        not stat.S_ISREG(before.st_mode)
        or before.st_nlink != 1
        or before.st_mode & 0o111
        or before.st_size != len(expected)
    ):
        raise SystemExit(
            "error: authenticated PyO3 config must be one bounded, singly linked, "
            "non-executable regular file"
        )
    payload = os.read(descriptor, len(expected) + 1)
    after = os.fstat(descriptor)
finally:
    os.close(descriptor)
identity_before = (
    before.st_dev,
    before.st_ino,
    before.st_mode,
    before.st_nlink,
    before.st_size,
    before.st_mtime_ns,
    before.st_ctime_ns,
)
identity_after = (
    after.st_dev,
    after.st_ino,
    after.st_mode,
    after.st_nlink,
    after.st_size,
    after.st_mtime_ns,
    after.st_ctime_ns,
)
if payload != expected or identity_before != identity_after:
    raise SystemExit("error: authenticated PyO3 config content is invalid")
path_metadata = os.stat(path, follow_symlinks=False)
if (path_metadata.st_dev, path_metadata.st_ino) != (
    before.st_dev,
    before.st_ino,
):
    raise SystemExit("error: authenticated PyO3 config changed identity")
PY
}

assert_authenticated_maturin_darwin_rustc_policy() {
  local expected_encoded
  expected_encoded=$'-C\x1flink-arg=-undefined\x1f-C\x1flink-arg=dynamic_lookup'

  if [[ "${AUTHENTICATED_COMMAND_NAME}" != "rustc" || \
    "${IROHA_PRIVACY_AUTHENTICATED_RUST_TOOLCHAIN_SELECTOR}" != \
      "1.93.1-aarch64-apple-darwin" || \
    "${IROHA_PRIVACY_AUTHENTICATED_MATURIN_VERSION:-}" != "1.14.1" || \
    "${IROHA_PRIVACY_AUTHENTICATED_MATURIN_ENCODED_RUSTFLAGS:-}" != \
      "${expected_encoded}" || \
    "${CARGO_ENCODED_RUSTFLAGS:-}" != "${expected_encoded}" || \
    "${IROHA_PRIVACY_AUTHENTICATED_MATURIN_RUSTC_LINK_ARG:-}" != \
      "link-args=-Wl,-install_name,@rpath/iroha_python._crypto.abi3.so" || \
    -z "${IROHA_PRIVACY_AUTHENTICATED_PYO3_PYTHON:-}" || \
    "${PYO3_PYTHON:-}" != \
      "${IROHA_PRIVACY_AUTHENTICATED_PYO3_PYTHON}" ]]; then
    echo "error: Darwin Maturin rustc policy is not exactly authenticated" >&2
    return 1
  fi
  assert_authenticated_maturin_pyo3_config
}

assert_authenticated_python_build_policy() {
  local expected_policy
  local python_surface_present=0
  local environment_name

  for environment_name in \
    IROHA_PRIVACY_AUTHENTICATED_PYTHON_BUILD_POLICY \
    IROHA_PRIVACY_AUTHENTICATED_PYO3_PYTHON \
    IROHA_PRIVACY_AUTHENTICATED_MATURIN_VERSION \
    IROHA_PRIVACY_AUTHENTICATED_MATURIN_PYO3_CONFIG_PATH \
    IROHA_PRIVACY_AUTHENTICATED_MATURIN_ENCODED_RUSTFLAGS \
    IROHA_PRIVACY_AUTHENTICATED_MATURIN_RUSTC_LINK_ARG \
    IROHA_PRIVACY_AUTHENTICATED_MACOSX_DEPLOYMENT_TARGET \
    PYO3_PYTHON \
    PYO3_CONFIG_FILE \
    PYO3_BUILD_EXTENSION_MODULE \
    PYO3_ENVIRONMENT_SIGNATURE \
    PYTHON_SYS_EXECUTABLE \
    MACOSX_DEPLOYMENT_TARGET \
    IROHA_PYTHON_SKIP_RUNTIME_LINK; do
    if [[ -n "${!environment_name+x}" ]]; then
      python_surface_present=1
      break
    fi
  done

  if [[ -z "${IROHA_PRIVACY_AUTHENTICATED_PYTHON_BUILD_POLICY:-}" ]]; then
    if [[ "${python_surface_present}" == "1" ]]; then
      echo \
        "error: native Cargo invocation rejected an unauthenticated Python-build surface" \
        >&2
      return 1
    fi
    return 0
  fi

  expected_policy="$(
    printf 'v1|%s|%s|%s|%s|%s|%s|%s' \
      "${IROHA_PRIVACY_AUTHENTICATED_RUST_TOOLCHAIN_SELECTOR}" \
      "${IROHA_PRIVACY_AUTHENTICATED_CARGO_TARGET_DIR}" \
      "${IROHA_PRIVACY_AUTHENTICATED_CARGO_TARGET_DIRECTORY_STATE}" \
      "${IROHA_PRIVACY_AUTHENTICATED_CARGO_HOME}" \
      "${IROHA_PRIVACY_AUTHENTICATED_CARGO_HOME_DIRECTORY_STATE}" \
      "${IROHA_PRIVACY_AUTHENTICATED_PYO3_PYTHON:-}" \
      "${IROHA_PRIVACY_AUTHENTICATED_MATURIN_VERSION:-}"
  )"
  if [[ "${IROHA_PRIVACY_AUTHENTICATED_PYTHON_BUILD_POLICY}" != \
      "${expected_policy}" || \
    "${IROHA_PRIVACY_AUTHENTICATED_MATURIN_VERSION:-}" != "1.14.1" || \
    -z "${IROHA_PRIVACY_AUTHENTICATED_PYO3_PYTHON:-}" || \
    "${PYO3_PYTHON:-}" != \
      "${IROHA_PRIVACY_AUTHENTICATED_PYO3_PYTHON}" || \
    "${IROHA_PYTHON_SKIP_RUNTIME_LINK:-}" != "1" ]]; then
    echo "error: privacy SDK Python-build policy marker is not exactly authenticated" >&2
    return 1
  fi
}

assert_authenticated_python_command_environment() {
  local compile_environment_name

  if [[ -z "${IROHA_PRIVACY_AUTHENTICATED_PYTHON_BUILD_POLICY:-}" ]]; then
    if [[ -n "${CARGO+x}" ]]; then
      echo \
        "error: native Cargo invocation must not inherit the Cargo selector environment" \
        >&2
      return 1
    fi
    return 0
  fi
  if [[ "${AUTHENTICATED_COMMAND_NAME}" != "metadata" && \
    "${AUTHENTICATED_COMMAND_NAME}" != "rustc" ]]; then
    echo "error: authenticated Python build permits only metadata and rustc" >&2
    return 1
  fi
  if [[ "${AUTHENTICATED_COMMAND_NAME}" == "metadata" ]]; then
    if [[ "${CARGO+x}" != "x" || \
      "${CARGO}" != "${CARGO_WRAPPER_ENTRY_PATH}" ]]; then
      echo "error: Maturin metadata must select the authenticated Cargo wrapper" >&2
      return 1
    fi
    for compile_environment_name in \
      PYO3_CONFIG_FILE \
      PYO3_BUILD_EXTENSION_MODULE \
      PYO3_ENVIRONMENT_SIGNATURE \
      PYTHON_SYS_EXECUTABLE \
      MACOSX_DEPLOYMENT_TARGET; do
      if [[ -n "${!compile_environment_name+x}" ]]; then
        echo \
          "error: Maturin metadata inherited rustc-only ${compile_environment_name}" \
          >&2
        return 1
      fi
    done
    if [[ "${CARGO_ENCODED_RUSTFLAGS+x}" != "x" || \
      -n "${CARGO_ENCODED_RUSTFLAGS}" ]]; then
      echo "error: Maturin metadata encoded rustflags must be explicitly empty" >&2
      return 1
    fi
    return 0
  fi

  if [[ -n "${CARGO+x}" ]]; then
    echo "error: Maturin rustc must not inherit the Cargo selector environment" >&2
    return 1
  fi
  if [[ "${PYTHON_SYS_EXECUTABLE+x}" != "x" || \
    "${PYTHON_SYS_EXECUTABLE}" != \
      "${IROHA_PRIVACY_AUTHENTICATED_PYO3_PYTHON}" || \
    "${PYO3_BUILD_EXTENSION_MODULE+x}" != "x" || \
    "${PYO3_BUILD_EXTENSION_MODULE}" != "1" || \
    "${PYO3_ENVIRONMENT_SIGNATURE+x}" != "x" || \
    "${PYO3_ENVIRONMENT_SIGNATURE}" != "cpython-3.12-64bit" || \
    "${PYO3_CONFIG_FILE+x}" != "x" ]]; then
    echo "error: authenticated Maturin rustc environment is incomplete" >&2
    return 1
  fi
  assert_authenticated_maturin_pyo3_config || return 1
  case "${IROHA_PRIVACY_AUTHENTICATED_RUST_TOOLCHAIN_SELECTOR}" in
    1.93.1-x86_64-unknown-linux-gnu)
      if [[ "${CARGO_ENCODED_RUSTFLAGS+x}" != "x" || \
        -n "${CARGO_ENCODED_RUSTFLAGS}" || \
        -n "${MACOSX_DEPLOYMENT_TARGET+x}" || \
        -n "${IROHA_PRIVACY_AUTHENTICATED_MATURIN_ENCODED_RUSTFLAGS+x}" || \
        -n "${IROHA_PRIVACY_AUTHENTICATED_MATURIN_RUSTC_LINK_ARG+x}" || \
        -n "${IROHA_PRIVACY_AUTHENTICATED_MACOSX_DEPLOYMENT_TARGET+x}" ]]; then
        echo "error: Linux Maturin rustc environment is not exactly authenticated" >&2
        return 1
      fi
      ;;
    1.93.1-aarch64-apple-darwin)
      if [[ "${MACOSX_DEPLOYMENT_TARGET+x}" != "x" || \
        "${MACOSX_DEPLOYMENT_TARGET}" != "11.0" ]]; then
        echo "error: Darwin Maturin deployment target is missing" >&2
        return 1
      fi
      assert_authenticated_maturin_darwin_rustc_policy || return 1
      ;;
  esac
}

assert_authenticated_maturin_arguments() {
  local native_manifest
  local argument_index
  local -a expected_arguments

  if [[ -z "${IROHA_PRIVACY_AUTHENTICATED_PYTHON_BUILD_POLICY:-}" ]]; then
    return 0
  fi
  native_manifest="${IROHA_PRIVACY_SDK_ROOT}/python/iroha_python/iroha_python_rs/Cargo.toml"
  case "${AUTHENTICATED_COMMAND_NAME}" in
    metadata)
      expected_arguments=(
        metadata
        --format-version 1
        --manifest-path "${native_manifest}"
        --locked
        --offline
      )
      ;;
    rustc)
      expected_arguments=(
        rustc
        --jobs 1
        --profile release
        --target-dir "${IROHA_PRIVACY_AUTHENTICATED_CARGO_TARGET_DIR}"
        --message-format json-render-diagnostics
        --locked
        --offline
        --manifest-path "${native_manifest}"
        --lib
      )
      if [[ "${IROHA_PRIVACY_AUTHENTICATED_RUST_TOOLCHAIN_SELECTOR}" == \
        "1.93.1-aarch64-apple-darwin" ]]; then
        expected_arguments+=(
          --
          -C
          "${IROHA_PRIVACY_AUTHENTICATED_MATURIN_RUSTC_LINK_ARG}"
        )
      fi
      ;;
  esac
  if [[ "${argument_count}" -ne "${#expected_arguments[@]}" ]]; then
    echo "error: Maturin Cargo arguments do not match pinned 1.14.1" >&2
    return 1
  fi
  argument_index=0
  while [[ "${argument_index}" -lt "${argument_count}" ]]; do
    if [[ "${original_arguments[${argument_index}]}" != \
      "${expected_arguments[${argument_index}]}" ]]; then
      echo "error: Maturin Cargo arguments do not match pinned 1.14.1" >&2
      return 1
    fi
    argument_index=$((argument_index + 1))
  done
}

assert_no_cargo_policy_environment() {
  local environment_name
  local environment_value

  while IFS= read -r environment_name; do
    environment_value="${!environment_name}"
    case "${environment_name}" in
      CARGO_BUILD_JOBS)
        if [[ "${environment_value}" != "1" ]]; then
          echo "error: privacy SDK Cargo build jobs must remain fixed at one" >&2
          return 1
        fi
        ;;
      CARGO_NET_OFFLINE)
        if [[ "${environment_value}" == "true" ]]; then
          :
        elif [[ "${environment_value}" == "false" && \
          "${AUTHENTICATED_COMMAND_NAME}" == "fetch" && \
          -n "${IROHA_PRIVACY_AUTHENTICATED_CARGO_HOME:-}" && \
          -n "${IROHA_PRIVACY_AUTHENTICATED_CARGO_CONFIG_PATH:-}" && \
          -n "${IROHA_PRIVACY_AUTHENTICATED_CARGO_CONFIG_SEAL:-}" ]]; then
          :
        else
          echo "error: privacy SDK Cargo must remain offline" >&2
          return 1
        fi
        ;;
      CARGO_TARGET_DIR)
        if [[ -z "${IROHA_PRIVACY_AUTHENTICATED_CARGO_TARGET_DIR:-}" || \
          "${environment_value}" != \
          "${IROHA_PRIVACY_AUTHENTICATED_CARGO_TARGET_DIR}" ]]; then
          echo "error: privacy SDK Cargo target environment is not authenticated" >&2
          return 1
        fi
        ;;
      CARGO_INCREMENTAL)
        if [[ "${environment_value}" != "0" ]]; then
          echo \
            "error: privacy SDK Cargo incremental policy must remain disabled" \
            >&2
          return 1
        fi
        ;;
      CARGO_ENCODED_RUSTFLAGS)
        if [[ -n "${environment_value}" ]] && \
          ! assert_authenticated_maturin_darwin_rustc_policy; then
          echo \
            "error: privacy SDK Cargo encoded rustflags are not authenticated" \
            >&2
          return 1
        fi
        ;;
      RUSTC_BOOTSTRAP)
        if [[ "${environment_value}" != "1" ]]; then
          echo "error: privacy SDK Cargo bootstrap policy must remain fixed" >&2
          return 1
        fi
        ;;
      PYO3_PYTHON)
        if [[ -z "${IROHA_PRIVACY_AUTHENTICATED_PYO3_PYTHON:-}" || \
          "${environment_value}" != \
          "${IROHA_PRIVACY_AUTHENTICATED_PYO3_PYTHON}" ]]; then
          echo "error: privacy SDK PyO3 interpreter environment is not authenticated" >&2
          return 1
        fi
        ;;
      PYTHON_SYS_EXECUTABLE)
        if [[ "${AUTHENTICATED_COMMAND_NAME}" != "rustc" || \
          -z "${IROHA_PRIVACY_AUTHENTICATED_PYO3_PYTHON:-}" || \
          "${environment_value}" != \
            "${IROHA_PRIVACY_AUTHENTICATED_PYO3_PYTHON}" ]]; then
          echo "error: Maturin Python sys executable is not authenticated" >&2
          return 1
        fi
        ;;
      PYO3_BUILD_EXTENSION_MODULE)
        if [[ "${AUTHENTICATED_COMMAND_NAME}" != "rustc" || \
          "${environment_value}" != "1" ]]; then
          echo "error: PyO3 extension-module policy is not authenticated" >&2
          return 1
        fi
        ;;
      PYO3_ENVIRONMENT_SIGNATURE)
        if [[ "${AUTHENTICATED_COMMAND_NAME}" != "rustc" || \
          "${environment_value}" != "cpython-3.12-64bit" ]]; then
          echo "error: PyO3 environment signature is not authenticated" >&2
          return 1
        fi
        ;;
      MACOSX_DEPLOYMENT_TARGET)
        if [[ "${AUTHENTICATED_COMMAND_NAME}" != "rustc" || \
          "${IROHA_PRIVACY_AUTHENTICATED_RUST_TOOLCHAIN_SELECTOR}" != \
            "1.93.1-aarch64-apple-darwin" || \
          "${IROHA_PRIVACY_AUTHENTICATED_MACOSX_DEPLOYMENT_TARGET:-}" != \
            "11.0" || \
          "${environment_value}" != "11.0" ]]; then
          echo "error: macOS deployment target is not authenticated" >&2
          return 1
        fi
        ;;
      IROHA_PYTHON_SKIP_RUNTIME_LINK)
        if [[ "${environment_value}" != "1" ]]; then
          echo "error: privacy SDK runtime-link policy must remain disabled" >&2
          return 1
        fi
        ;;
      PYO3_CONFIG_FILE)
        assert_authenticated_maturin_pyo3_config || return 1
        ;;
      CARGO_ALIAS_*)
        echo \
          "error: privacy SDK Cargo wrapper rejected alias-affecting environment variable ${environment_name}" \
          >&2
        return 1
        ;;
      CARGO_BUILD_*|CARGO_HTTP_*|CARGO_HOST_*|CARGO_NET_*|CARGO_PROFILE_*|\
      CARGO_REGISTRIES_*|CARGO_REGISTRY_*|CARGO_SOURCE_*|CARGO_TARGET_*|\
      CARGO_UNSTABLE_*|PYO3_*|\
      DYLD_*|PKG_CONFIG_*|\
      CARGO_ENCODED_RUSTDOCFLAGS|\
      IROHA_PYTHON_RUNTIME_PATH|\
      AR|CC|CFLAGS|CPATH|CPPFLAGS|CXX|CXXFLAGS|LD|LD_LIBRARY_PATH|LDFLAGS|\
      LIBRARY_PATH|RANLIB|SDKROOT|\
      RUSTC|RUSTC_WRAPPER|RUSTC_WORKSPACE_WRAPPER|\
      RUSTDOC|RUSTDOCFLAGS|RUSTFLAGS|RUSTUP_*)
        echo \
          "error: privacy SDK Cargo wrapper rejected policy-affecting environment variable ${environment_name}" \
          >&2
        return 1
        ;;
    esac
  done < <(compgen -e || true)
  if [[ "${AUTHENTICATED_COMMAND_NAME}" != "version" ]]; then
    if [[ "${AUTHENTICATED_COMMAND_NAME}" == "fetch" ]]; then
      if [[ "${CARGO_NET_OFFLINE:-}" != "true" && \
        "${CARGO_NET_OFFLINE:-}" != "false" ]]; then
        echo \
          "error: privacy SDK Cargo fetch must explicitly select online or offline mode" \
          >&2
        return 1
      fi
    elif [[ "${CARGO_NET_OFFLINE:-}" != "true" ]]; then
      echo "error: privacy SDK Cargo commands must be offline by default" >&2
      return 1
    fi
  fi
  assert_authenticated_python_build_policy || return 1
  assert_authenticated_python_command_environment || return 1
  if [[ "${AUTHENTICATED_COMMAND_NAME}" != "version" && \
    ( "${CARGO_BUILD_JOBS:-}" != "1" || \
      "${CARGO_INCREMENTAL:-}" != "0" || \
      "${CARGO_ENCODED_RUSTFLAGS+x}" != "x" ) ]]; then
    echo \
      "error: privacy SDK Cargo deterministic build defaults are missing" \
      >&2
    return 1
  fi
}

assert_authenticated_cargo_execution_policy() {
  local expected_target="${IROHA_PRIVACY_AUTHENTICATED_CARGO_TARGET_DIR:-}"
  local expected_home="${IROHA_PRIVACY_AUTHENTICATED_CARGO_HOME:-}"
  local expected_target_state="${IROHA_PRIVACY_AUTHENTICATED_CARGO_TARGET_DIRECTORY_STATE:-}"
  local expected_home_state="${IROHA_PRIVACY_AUTHENTICATED_CARGO_HOME_DIRECTORY_STATE:-}"
  local canonical_target
  local canonical_home

  if [[ -n "${expected_target}" ]]; then
    if [[ -z "${expected_target_state}" ]]; then
      echo "error: privacy SDK Cargo target authenticated directory state is missing" >&2
      return 1
    fi
    if [[ "${CARGO_TARGET_DIR:-}" != "${expected_target}" ]]; then
      echo "error: privacy SDK Cargo target does not match the authenticated private target" >&2
      return 1
    fi
    if [[ "${CARGO_BUILD_JOBS:-}" != "1" ]]; then
      echo "error: privacy SDK Cargo build jobs must remain fixed at one" >&2
      return 1
    fi
    if [[ "${CARGO_NET_OFFLINE:-}" != "true" && \
      ! ( "${AUTHENTICATED_COMMAND_NAME}" == "fetch" && \
        "${CARGO_NET_OFFLINE:-}" == "false" ) ]]; then
      echo "error: privacy SDK Cargo must remain offline" >&2
      return 1
    fi
    if [[ ! -d "${expected_target}" || -L "${expected_target}" ]]; then
      echo "error: privacy SDK Cargo target must be a real directory" >&2
      return 1
    fi
    if ! canonical_target="$(cd "${expected_target}" && pwd -P)"; then
      echo "error: privacy SDK Cargo target became unavailable" >&2
      return 1
    fi
    if [[ "${canonical_target}" != "${expected_target}" ]]; then
      echo "error: privacy SDK Cargo target must be canonical" >&2
      return 1
    fi
    privacy_sdk_assert_directory_state \
      "${expected_target}" "${expected_target_state}" \
      "privacy SDK Cargo target" "${PYTHON_BIN}" || return 1
  elif [[ -n "${expected_target_state}" ]]; then
    echo "error: privacy SDK Cargo target directory state has no authenticated target" >&2
    return 1
  fi

  if [[ -n "${expected_home}" ]]; then
    if [[ -z "${expected_home_state}" ]]; then
      echo "error: privacy SDK Cargo home authenticated directory state is missing" >&2
      return 1
    fi
    if [[ "${CARGO_HOME:-}" != "${expected_home}" ]]; then
      echo "error: privacy SDK Cargo home does not match the authenticated private home" >&2
      return 1
    fi
    if [[ ! -d "${expected_home}" || -L "${expected_home}" ]]; then
      echo "error: privacy SDK Cargo home must be a real directory" >&2
      return 1
    fi
    if ! canonical_home="$(cd "${expected_home}" && pwd -P)"; then
      echo "error: privacy SDK Cargo home became unavailable" >&2
      return 1
    fi
    if [[ "${canonical_home}" != "${expected_home}" ]]; then
      echo "error: privacy SDK Cargo home must be canonical" >&2
      return 1
    fi
    if [[ -e "${expected_home}/config" || -L "${expected_home}/config" || \
      -e "${expected_home}/config.toml" || -L "${expected_home}/config.toml" ]]; then
      echo "error: privacy SDK Cargo home must not contain Cargo configuration" >&2
      return 1
    fi
    privacy_sdk_assert_directory_state \
      "${expected_home}" "${expected_home_state}" \
      "privacy SDK Cargo home" "${PYTHON_BIN}" || return 1
    privacy_sdk_assert_cargo_cache_component_state \
      "${expected_home}" registry \
      "${IROHA_PRIVACY_AUTHENTICATED_CARGO_REGISTRY_LINK_STATE}" \
      "${PYTHON_BIN}" || return 1
    privacy_sdk_assert_cargo_cache_component_state \
      "${expected_home}" git \
      "${IROHA_PRIVACY_AUTHENTICATED_CARGO_GIT_LINK_STATE}" \
      "${PYTHON_BIN}" || return 1
  fi

  if [[ -n "${expected_target}" && -n "${expected_home}" ]]; then
    if [[ "${RUSTC_BOOTSTRAP:-}" != "1" ]]; then
      echo "error: privacy SDK Cargo bootstrap policy must remain fixed" >&2
      return 1
    fi
  fi
}

assert_authenticated_cargo_configuration() {
  local expected_path="${IROHA_PRIVACY_AUTHENTICATED_CARGO_CONFIG_PATH:-}"
  local expected_seal="${IROHA_PRIVACY_AUTHENTICATED_CARGO_CONFIG_SEAL:-}"

  if [[ -n "${IROHA_PRIVACY_AUTHENTICATED_CARGO_TARGET_DIR:-}" && \
    -n "${IROHA_PRIVACY_AUTHENTICATED_CARGO_HOME:-}" && \
    ( -z "${expected_path}" || -z "${expected_seal}" ) ]]; then
    echo "error: privacy SDK Cargo execution requires authenticated repository configuration" >&2
    return 1
  fi
  if [[ -z "${expected_path}" && -z "${expected_seal}" ]]; then
    return 0
  fi
  if [[ -z "${expected_path}" || -z "${expected_seal}" || \
    "${expected_path}" != "${IROHA_PRIVACY_SDK_ROOT}/.cargo/config.toml" ]]; then
    echo "error: privacy SDK Cargo configuration authentication is incomplete" >&2
    return 1
  fi
  if ! "${PYTHON_BIN}" -I - \
    "${IROHA_PRIVACY_SDK_ROOT}" "${expected_path}" <<'PY'
import os
import sys
from pathlib import Path

root = Path(sys.argv[1])
expected = Path(sys.argv[2])
try:
    canonical_root = root.resolve(strict=True)
    invocation_directory = Path.cwd().resolve(strict=True)
except OSError as error:
    raise SystemExit(
        f"error: privacy SDK Cargo configuration search path is unavailable: {error}"
    )
if canonical_root != root:
    raise SystemExit("error: privacy SDK root must remain canonical")
if invocation_directory != canonical_root and canonical_root not in invocation_directory.parents:
    raise SystemExit(
        "error: privacy SDK Cargo invocation directory escaped the authenticated root"
    )

directory = invocation_directory
while True:
    cargo_directory = directory / ".cargo"
    for filename in ("config", "config.toml"):
        candidate = cargo_directory / filename
        if os.path.lexists(candidate) and candidate != expected:
            raise SystemExit(
                "error: untrusted Cargo configuration is active at invocation: "
                f"{candidate}"
            )
    if directory.parent == directory:
        break
    directory = directory.parent
PY
  then
    return 1
  fi
  privacy_sdk_assert_file_seal \
    "${expected_path}" \
    "${expected_seal}" \
    "repository Cargo configuration" \
    "${PYTHON_BIN}"
}

assert_authenticated_cargo_lock_state() {
  local validate_command_environment="${1:-1}"
  local status=0
  privacy_sdk_assert_file_seal \
    "${SELECTED_LOCKFILE}" \
    "${IROHA_PRIVACY_AUTHENTICATED_CARGO_LOCKFILE_SEAL}" \
    "selected authenticated Cargo.lock" \
    "${PYTHON_BIN}" || status=1
  privacy_sdk_assert_optional_file_state \
    "${IROHA_PRIVACY_SDK_ROOT}/Cargo.lock" \
    "${IROHA_PRIVACY_AUTHENTICATED_WORKSPACE_CARGO_LOCK_STATE}" \
    "workspace Cargo.lock" \
    "${PYTHON_BIN}" || status=1
  if [[ "${validate_command_environment}" == "1" && \
    -n "${AUTHENTICATED_COMMAND_NAME}" ]]; then
    assert_no_cargo_policy_environment || status=1
  fi
  assert_authenticated_cargo_configuration || status=1
  if [[ "${validate_command_environment}" == "1" ]]; then
    assert_authenticated_cargo_execution_policy || status=1
  fi
  privacy_sdk_assert_authenticated_toolchain_state \
    "${IROHA_PRIVACY_SDK_ROOT}" \
    "$(pwd -P)" \
    "${PYTHON_BIN}" || status=1
  return "${status}"
}

run_real_cargo_and_verify_locks() {
  local status
  local environment_name
  local -a cargo_child_environment

  cargo_child_environment=(env)
  while IFS= read -r environment_name; do
    case "${environment_name}" in
      IROHA_PRIVACY_*|IROHA_JS_CARGO_LOCKFILE_PATH)
        cargo_child_environment+=(-u "${environment_name}")
        ;;
    esac
  done < <(compgen -e || true)
  cargo_child_environment+=("CARGO=${CARGO_WRAPPER_ENTRY_PATH}")
  cargo_child_environment+=("RUSTC=${IROHA_PRIVACY_AUTHENTICATED_RUSTC_PATH}")
  cargo_child_environment+=("RUSTDOC=${IROHA_PRIVACY_AUTHENTICATED_RUSTDOC_PATH}")

  set +e
  "${cargo_child_environment[@]}" "${REAL_CARGO_PATH}" "$@"
  status=$?
  set -e
  if ! assert_authenticated_cargo_lock_state; then
    status=1
  fi
  exit "${status}"
}

assert_authenticated_cargo_lock_state 0 || exit 1

if [[ ! -x "${REAL_CARGO_PATH}" ]]; then
  echo "error: IROHA_PRIVACY_REAL_CARGO must name an executable Cargo binary" >&2
  exit 1
fi
if [[ "${REAL_CARGO_PATH}" == "$0" ]]; then
  echo "error: privacy SDK Cargo wrapper cannot invoke itself" >&2
  exit 1
fi
if [[ ! -f "${CARGO_AUDIT_PATH}" || -L "${CARGO_AUDIT_PATH}" ]]; then
  echo "error: privacy SDK Cargo audit path must be an existing regular file" >&2
  exit 1
fi

reject_cargo_invocation() {
  echo "error: privacy SDK Cargo wrapper $1" >&2
  exit 1
}

is_cargo_jobs_option() {
  local argument="$1"

  [[ "${argument}" == "--jobs" || \
    "${argument}" == --jobs=* || \
    "${argument}" =~ ^-[vq]*j ]]
}

assert_authenticated_jobs_option() {
  local argument="$1"
  local following_value="$2"
  local has_following_value="$3"
  local value

  POLICY_OPTION_ARITY=1
  case "${argument}" in
    --jobs)
      if [[ "${has_following_value}" != "1" ]]; then
        reject_cargo_invocation "rejected --jobs without its required value"
      fi
      value="${following_value}"
      POLICY_OPTION_ARITY=2
      ;;
    --jobs=*)
      value="${argument#--jobs=}"
      ;;
    *)
      if [[ ! "${argument}" =~ ^-[vq]*j(=?)(.*)$ ]]; then
        reject_cargo_invocation "rejected an unparseable Cargo jobs option"
      fi
      value="${BASH_REMATCH[2]}"
      if [[ -z "${value}" ]]; then
        if [[ "${has_following_value}" != "1" ]]; then
          reject_cargo_invocation "rejected ${argument} without its required value"
        fi
        value="${following_value}"
        POLICY_OPTION_ARITY=2
      fi
      ;;
  esac
  if [[ -z "${IROHA_PRIVACY_AUTHENTICATED_CARGO_TARGET_DIR:-}" || \
    "${CARGO_BUILD_JOBS:-}" != "1" || "${value}" != "1" ]]; then
    reject_cargo_invocation \
      "rejected an unauthenticated or mismatched Cargo concurrency override"
  fi
}

assert_authenticated_target_dir_option() {
  local argument="$1"
  local following_value="$2"
  local has_following_value="$3"
  local value

  POLICY_OPTION_ARITY=1
  case "${argument}" in
    --target-dir)
      if [[ "${has_following_value}" != "1" ]]; then
        reject_cargo_invocation "rejected --target-dir without its required value"
      fi
      value="${following_value}"
      POLICY_OPTION_ARITY=2
      ;;
    --target-dir=*)
      value="${argument#--target-dir=}"
      ;;
    *)
      reject_cargo_invocation "rejected an unparseable Cargo target-dir option"
      ;;
  esac
  if [[ -z "${IROHA_PRIVACY_AUTHENTICATED_CARGO_TARGET_DIR:-}" || \
    "${CARGO_TARGET_DIR:-}" != "${IROHA_PRIVACY_AUTHENTICATED_CARGO_TARGET_DIR}" || \
    "${value}" != "${IROHA_PRIVACY_AUTHENTICATED_CARGO_TARGET_DIR}" ]]; then
    reject_cargo_invocation \
      "rejected an unauthenticated or mismatched Cargo target-dir override"
  fi
}

assert_reinforcing_offline_option() {
  if [[ -z "${IROHA_PRIVACY_AUTHENTICATED_CARGO_TARGET_DIR:-}" || \
    "${CARGO_NET_OFFLINE:-}" != "true" ]]; then
    reject_cargo_invocation \
      "rejected an unauthenticated Cargo offline override"
  fi
}

assert_safe_color_value() {
  case "$1" in
    auto|always|never) ;;
    *)
      reject_cargo_invocation "rejected an invalid Cargo color value"
      ;;
  esac
}

original_arguments=("$@")
argument_count="${#original_arguments[@]}"

# Preserve Cargo's existing non-locking version probes, but do not let version
# flags hide another token that the wrapper would otherwise treat as a command.
if [[ "${argument_count}" -eq 1 ]]; then
  case "${original_arguments[0]}" in
    --version|-V|version)
      AUTHENTICATED_COMMAND_NAME="version"
      assert_authenticated_cargo_lock_state || exit 1
      run_real_cargo_and_verify_locks "${original_arguments[@]}"
      ;;
  esac
fi

# Locate the real Cargo subcommand. Only Cargo-global options are consumed in
# this phase; their values can therefore never impersonate a subcommand.
command_index=-1
command_name=""
global_options_terminated=0
argument_index=0
while [[ "${argument_index}" -lt "${argument_count}" ]]; do
  argument="${original_arguments[${argument_index}]}"

  if [[ "${global_options_terminated}" == "1" ]]; then
    command_index="${argument_index}"
    command_name="${argument}"
    break
  fi

  case "${argument}" in
    --)
      global_options_terminated=1
      argument_index=$((argument_index + 1))
      ;;
    --config)
      reject_cargo_invocation \
        "rejected --config because it can alter aliases or authenticated policy"
      ;;
    --color)
      if [[ $((argument_index + 1)) -ge "${argument_count}" ]]; then
        reject_cargo_invocation \
          "rejected ${argument} without its required global option value"
      fi
      assert_safe_color_value "${original_arguments[$((argument_index + 1))]}"
      argument_index=$((argument_index + 2))
      ;;
    --config=*)
      reject_cargo_invocation \
        "rejected --config because it can alter aliases or authenticated policy"
      ;;
    -C|-C?*)
      reject_cargo_invocation \
        "rejected a Cargo working-directory override"
      ;;
    -Z|-Z?*)
      reject_cargo_invocation \
        "rejected a caller-provided unstable Cargo option"
      ;;
    --explain|--explain=*|--list|--help|-h)
      reject_cargo_invocation \
        "rejected a Cargo control-flow option combined with a subcommand"
      ;;
    --color=*)
      assert_safe_color_value "${argument#--color=}"
      argument_index=$((argument_index + 1))
      ;;
    --target|--target=*)
      reject_cargo_invocation \
        "rejected a command-line target triple override: ${argument}"
      ;;
    --target-dir|--target-dir=*)
      policy_following_available=0
      if [[ $((argument_index + 1)) -lt "${argument_count}" ]]; then
        policy_following_available=1
      fi
      assert_authenticated_target_dir_option \
        "${argument}" \
        "${original_arguments[$((argument_index + 1))]-}" \
        "${policy_following_available}"
      argument_index=$((argument_index + POLICY_OPTION_ARITY))
      ;;
    --offline)
      assert_reinforcing_offline_option
      argument_index=$((argument_index + 1))
      ;;
    --offline=*|--frozen|--online|--online=*|--network|--network=*|\
    --rustc|--rustc=*|--rustc-wrapper|--rustc-wrapper=*|\
    --rustc-workspace-wrapper|--rustc-workspace-wrapper=*|\
    --rustflags|--rustflags=*)
      reject_cargo_invocation \
        "rejected a command-line network or compiler policy override: ${argument}"
      ;;
    --verbose|--quiet|--locked)
      argument_index=$((argument_index + 1))
      ;;
    --version|-V)
      reject_cargo_invocation \
        "rejected a version flag combined with another Cargo argument"
      ;;
    +*)
      reject_cargo_invocation \
        "rejected a toolchain selector in front of the authenticated Cargo binary"
      ;;
    -*)
      if is_cargo_jobs_option "${argument}"; then
        policy_following_available=0
        if [[ $((argument_index + 1)) -lt "${argument_count}" ]]; then
          policy_following_available=1
        fi
        assert_authenticated_jobs_option \
          "${argument}" \
          "${original_arguments[$((argument_index + 1))]-}" \
          "${policy_following_available}"
        argument_index=$((argument_index + POLICY_OPTION_ARITY))
        continue
      fi
      if [[ "${argument}" =~ ^-[vq]+$ ]]; then
        argument_index=$((argument_index + 1))
      else
        reject_cargo_invocation \
          "rejected unknown Cargo global option ${argument}"
      fi
      ;;
    *)
      command_index="${argument_index}"
      command_name="${argument}"
      break
      ;;
  esac
done

if [[ "${command_index}" -lt 0 ]]; then
  reject_cargo_invocation "rejected an unknown non-locking invocation"
fi

case "${command_name}" in
  metadata|rustc|build|check|test|fetch)
    ;;
  *)
    reject_cargo_invocation \
      "rejected Cargo alias or external subcommand ${command_name}"
    ;;
esac
AUTHENTICATED_COMMAND_NAME="${command_name}"
assert_authenticated_cargo_lock_state || exit 1

# Cargo arguments end at the first post-command `--`. Compiler or program
# arguments on the other side must not influence lock or unstable-option state.
cargo_argument_limit="${argument_count}"
argument_index=$((command_index + 1))
while [[ "${argument_index}" -lt "${argument_count}" ]]; do
  if [[ "${original_arguments[${argument_index}]}" == "--" ]]; then
    cargo_argument_limit="${argument_index}"
    break
  fi
  argument_index=$((argument_index + 1))
done
assert_authenticated_maturin_arguments || exit 1

has_locked=0
argument_index=0
while [[ "${argument_index}" -lt "${cargo_argument_limit}" ]]; do
  argument="${original_arguments[${argument_index}]}"
  case "${argument}" in
    --config)
      reject_cargo_invocation \
        "rejected --config because it can alter aliases or authenticated policy"
      ;;
    --config=*)
      reject_cargo_invocation \
        "rejected --config because it can alter aliases or authenticated policy"
      ;;
    --lockfile-path|--lockfile-path=*)
      reject_cargo_invocation \
        "rejected an invocation that attempted to override the selected lockfile"
      ;;
    --target|--target=*|\
    --rustc|--rustc=*|--rustc-wrapper|--rustc-wrapper=*|\
    --rustc-workspace-wrapper|--rustc-workspace-wrapper=*|\
    --rustflags|--rustflags=*)
      reject_cargo_invocation \
        "rejected a command-line target or compiler policy override: ${argument}"
      ;;
    --target-dir|--target-dir=*)
      policy_following_available=0
      if [[ $((argument_index + 1)) -lt "${cargo_argument_limit}" ]]; then
        policy_following_available=1
      fi
      assert_authenticated_target_dir_option \
        "${argument}" \
        "${original_arguments[$((argument_index + 1))]-}" \
        "${policy_following_available}"
      argument_index=$((argument_index + POLICY_OPTION_ARITY))
      continue
      ;;
    --artifact-dir|--artifact-dir=*|--out-dir|--out-dir=*|--root|--root=*)
      reject_cargo_invocation \
        "rejected a command-line artifact or installation output override: ${argument}"
      ;;
    --crate-type|--crate-type=*)
      reject_cargo_invocation \
        "rejected a Cargo-side compiler output override: ${argument}"
      ;;
    --offline)
      assert_reinforcing_offline_option
      ;;
    --offline=*|--frozen|--online|--online=*|--network|--network=*)
      reject_cargo_invocation \
        "rejected a command-line network policy override: ${argument}"
      ;;
    --locked)
      has_locked=1
      ;;
    -C|-C?*)
      reject_cargo_invocation \
        "rejected a Cargo working-directory override"
      ;;
    -Z|-Z?*)
      reject_cargo_invocation \
        "rejected a caller-provided unstable Cargo option"
      ;;
    --explain|--explain=*|--list|--help|-h|--version|-V)
      reject_cargo_invocation \
        "rejected a Cargo control-flow option combined with a subcommand"
      ;;
    --color)
      if [[ $((argument_index + 1)) -ge "${cargo_argument_limit}" ]]; then
        reject_cargo_invocation \
          "rejected ${argument} without its required global option value"
      fi
      assert_safe_color_value "${original_arguments[$((argument_index + 1))]}"
      argument_index=$((argument_index + 2))
      continue
      ;;
    *)
      if is_cargo_jobs_option "${argument}"; then
        policy_following_available=0
        if [[ $((argument_index + 1)) -lt "${cargo_argument_limit}" ]]; then
          policy_following_available=1
        fi
        assert_authenticated_jobs_option \
          "${argument}" \
          "${original_arguments[$((argument_index + 1))]-}" \
          "${policy_following_available}"
        argument_index=$((argument_index + POLICY_OPTION_ARITY))
        continue
      fi
      ;;
  esac
  argument_index=$((argument_index + 1))
done

# Maturin 1.14.1 needs one exact, authenticated install-name pair for this
# abi3 module on Darwin. Every other post-`--` rustc surface stays closed.
if [[ "${command_name}" == "rustc" ]]; then
  post_rustc_argument_count=$((argument_count - cargo_argument_limit - 1))
  if [[ -n "${IROHA_PRIVACY_AUTHENTICATED_PYTHON_BUILD_POLICY:-}" && \
    "${IROHA_PRIVACY_AUTHENTICATED_RUST_TOOLCHAIN_SELECTOR}" == \
      "1.93.1-aarch64-apple-darwin" ]]; then
    if ! assert_authenticated_maturin_darwin_rustc_policy || \
      [[ "${post_rustc_argument_count}" -ne 2 ]] || \
      [[ "${original_arguments[$((cargo_argument_limit + 1))]-}" != "-C" ]] || \
      [[ "${original_arguments[$((cargo_argument_limit + 2))]-}" != \
        "${IROHA_PRIVACY_AUTHENTICATED_MATURIN_RUSTC_LINK_ARG:-}" ]]; then
      reject_cargo_invocation \
        "rejected unauthenticated Maturin Darwin rustc-side compiler arguments"
    fi
  elif [[ "${post_rustc_argument_count}" -gt 0 ]]; then
    reject_cargo_invocation \
      "rejected caller-supplied rustc-side compiler arguments"
  fi
fi

arguments=()
argument_index=0
while [[ "${argument_index}" -lt "${cargo_argument_limit}" ]]; do
  arguments+=("${original_arguments[${argument_index}]}")
  argument_index=$((argument_index + 1))
done
arguments+=(--lockfile-path "${SELECTED_LOCKFILE}")
if [[ "${has_locked}" == "0" ]]; then
  arguments+=(--locked)
fi
while [[ "${argument_index}" -lt "${argument_count}" ]]; do
  arguments+=("${original_arguments[${argument_index}]}")
  argument_index=$((argument_index + 1))
done

printf '%s\n' "${command_name}" >>"${CARGO_AUDIT_PATH}"
run_real_cargo_and_verify_locks -Z unstable-options "${arguments[@]}"

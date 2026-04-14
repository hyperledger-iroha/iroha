#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-${TMPDIR:-/tmp}/iroha-inrou-portable-smoke-target}"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "ERROR: required command '$1' not found in PATH." >&2
    exit 1
  fi
}

require_one_of() {
  for cmd in "$@"; do
    if command -v "$cmd" >/dev/null 2>&1; then
      return 0
    fi
  done
  echo "ERROR: required command not found; expected one of: $*" >&2
  exit 1
}

require_env_file() {
  local name="$1"
  local value="${!name:-}"
  if [ -z "$value" ]; then
    echo "ERROR: environment variable '$name' must point to an existing file." >&2
    exit 1
  fi
  if [ ! -f "$value" ]; then
    echo "ERROR: environment variable '$name' points to a missing file: $value" >&2
    exit 1
  fi
}

HOST_ARCH="$(uname -m)"
case "$HOST_ARCH" in
  x86_64)
    require_cmd qemu-system-x86_64
    ;;
  arm64|aarch64)
    require_cmd qemu-system-aarch64
    ;;
  *)
    echo "ERROR: unsupported host architecture for PortableVm smoke: $HOST_ARCH" >&2
    exit 1
    ;;
esac

require_cmd cargo
require_cmd qemu-img
require_one_of virtiofsd qemu-virtiofsd
require_cmd tar

require_env_file IROHA_INROU_PORTABLE_KERNEL_IMAGE
require_env_file IROHA_INROU_PORTABLE_ROOTFS_IMAGE

if [ -n "${IROHA_INROU_PORTABLE_INITRD_IMAGE:-}" ] && [ ! -f "${IROHA_INROU_PORTABLE_INITRD_IMAGE}" ]; then
  echo "ERROR: IROHA_INROU_PORTABLE_INITRD_IMAGE points to a missing file: ${IROHA_INROU_PORTABLE_INITRD_IMAGE}" >&2
  exit 1
fi

export IROHA_RUN_IGNORED=1
export IROHA_INROU_PORTABLE=1
export CARGO_TARGET_DIR

cd "$ROOT_DIR"

echo "+ cargo test --locked -p irohad --features embedded-soracloud-runtime --bin irohad build_inrou_user_data_projects_virtiofs_mounts_and_allowlist_overlay -- --nocapture"
cargo test --locked -p irohad --features embedded-soracloud-runtime --bin irohad build_inrou_user_data_projects_virtiofs_mounts_and_allowlist_overlay -- --nocapture

echo "+ cargo test --locked -p irohad --features embedded-soracloud-runtime --bin irohad ensure_inrou_portable_root_disk_uses_qcow2_overlay_with_backing_file -- --nocapture"
cargo test --locked -p irohad --features embedded-soracloud-runtime --bin irohad ensure_inrou_portable_root_disk_uses_qcow2_overlay_with_backing_file -- --nocapture

echo "+ cargo test --locked -p irohad --features embedded-soracloud-runtime --bin irohad inrou_portable_smoke_boots_debian_guest_and_serves_healthcheck -- --ignored --nocapture"
cargo test --locked -p irohad --features embedded-soracloud-runtime --bin irohad inrou_portable_smoke_boots_debian_guest_and_serves_healthcheck -- --ignored --nocapture

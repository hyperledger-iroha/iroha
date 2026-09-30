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

require_env_file() {
  local name="$1"
  local value="${!name:-}"
  if [ -z "$value" ]; then
    echo "ERROR: environment variable '$name' must point to an existing file." >&2
    echo "Hint: prepare local Debian genericcloud assets with:" >&2
    echo "  eval \"\$(python3 scripts/ci/prepare_inrou_portable_guest_assets.py --print-env)\"" >&2
    exit 1
  fi
  if [ ! -f "$value" ]; then
    echo "ERROR: environment variable '$name' points to a missing file: $value" >&2
    echo "Hint: refresh local Debian genericcloud assets with:" >&2
    echo "  eval \"\$(python3 scripts/ci/prepare_inrou_portable_guest_assets.py --force --print-env)\"" >&2
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
require_cmd tar
if [ ! -x /usr/sbin/mke2fs ] && [ ! -x /sbin/mke2fs ]; then
  echo "ERROR: required root-custodied mke2fs was not found at /usr/sbin/mke2fs or /sbin/mke2fs." >&2
  exit 1
fi

require_env_file IROHA_INROU_PORTABLE_KERNEL_IMAGE
require_env_file IROHA_INROU_PORTABLE_ROOTFS_IMAGE

if [ -n "${IROHA_INROU_PORTABLE_INITRD_IMAGE:-}" ] && [ ! -f "${IROHA_INROU_PORTABLE_INITRD_IMAGE}" ]; then
  echo "ERROR: IROHA_INROU_PORTABLE_INITRD_IMAGE points to a missing file: ${IROHA_INROU_PORTABLE_INITRD_IMAGE}" >&2
  exit 1
fi

export CARGO_TARGET_DIR

cd "$ROOT_DIR"

echo "+ cargo test --locked -p irohad --bin iroha3d build_inrou_user_data_projects_isolated_portable_block_mounts -- --nocapture"
cargo test --locked -p irohad --bin iroha3d build_inrou_user_data_projects_isolated_portable_block_mounts -- --nocapture

echo "+ cargo test --locked -p irohad --bin iroha3d ensure_inrou_portable_root_disk_is_a_standalone_authenticated_copy -- --nocapture"
cargo test --locked -p irohad --bin iroha3d ensure_inrou_portable_root_disk_is_a_standalone_authenticated_copy -- --nocapture

echo "+ cargo test --locked -p irohad --bin iroha3d ensure_inrou_portable_lease_disks_create_reusable_raw_images -- --nocapture"
cargo test --locked -p irohad --bin iroha3d ensure_inrou_portable_lease_disks_create_reusable_raw_images -- --nocapture

echo "+ cargo test --locked -p irohad --bin iroha3d inrou_portable_smoke_boots_debian_guest_and_serves_healthcheck -- --ignored --nocapture"
cargo test --locked -p irohad --bin iroha3d inrou_portable_smoke_boots_debian_guest_and_serves_healthcheck -- --ignored --nocapture

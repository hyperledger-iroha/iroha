#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-${TMPDIR:-/tmp}/iroha-inrou-linux-kvm-smoke-target}"

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
    exit 1
  fi
  if [ ! -f "$value" ]; then
    echo "ERROR: environment variable '$name' points to a missing file: $value" >&2
    exit 1
  fi
}

if [ "$(uname -s)" != "Linux" ]; then
  echo "ERROR: the Inrou Firecracker smoke harness must run on a real Linux/KVM host." >&2
  exit 1
fi

if [ "$(id -u)" != "0" ]; then
  echo "ERROR: run this harness as root so the test can create tap devices and firewall rules." >&2
  exit 1
fi

if [ ! -c /dev/kvm ]; then
  echo "ERROR: /dev/kvm is required for the Inrou Firecracker smoke harness." >&2
  exit 1
fi

if [ ! -c /dev/net/tun ]; then
  echo "ERROR: /dev/net/tun is required for the Inrou Firecracker smoke harness." >&2
  exit 1
fi

if [ "$(cat /proc/sys/net/ipv4/ip_forward 2>/dev/null || echo 0)" != "1" ]; then
  echo "ERROR: /proc/sys/net/ipv4/ip_forward must be 1 before running the smoke harness." >&2
  exit 1
fi

require_cmd cargo
require_cmd firecracker
require_cmd ip
require_cmd iptables
require_cmd tar
require_cmd exportfs
require_cmd rpc.nfsd
require_cmd mount
require_cmd chown

if ! command -v mke2fs >/dev/null 2>&1 && ! command -v mkfs.ext4 >/dev/null 2>&1; then
  echo "ERROR: install mke2fs or mkfs.ext4 before running the smoke harness." >&2
  exit 1
fi

require_env_file IROHA_INROU_LINUX_KVM_KERNEL_IMAGE
require_env_file IROHA_INROU_LINUX_KVM_ROOTFS_IMAGE

if [ -n "${IROHA_INROU_LINUX_KVM_INITRD_IMAGE:-}" ] && [ ! -f "${IROHA_INROU_LINUX_KVM_INITRD_IMAGE}" ]; then
  echo "ERROR: IROHA_INROU_LINUX_KVM_INITRD_IMAGE points to a missing file: ${IROHA_INROU_LINUX_KVM_INITRD_IMAGE}" >&2
  exit 1
fi

export IROHA_RUN_IGNORED=1
export IROHA_INROU_LINUX_KVM=1
export CARGO_TARGET_DIR

cd "$ROOT_DIR"

echo "+ cargo test --locked -p irohad --features embedded-soracloud-runtime --bin irohad build_inrou_user_data_projects_mounts_overlay_and_replica_env -- --nocapture"
cargo test --locked -p irohad --features embedded-soracloud-runtime --bin irohad build_inrou_user_data_projects_mounts_overlay_and_replica_env -- --nocapture

echo "+ cargo test --locked -p irohad --features embedded-soracloud-runtime --bin irohad write_inrou_firecracker_config_serializes_boot_source_drives_and_network -- --nocapture"
cargo test --locked -p irohad --features embedded-soracloud-runtime --bin irohad write_inrou_firecracker_config_serializes_boot_source_drives_and_network -- --nocapture

echo "+ cargo test --locked -p irohad --features embedded-soracloud-runtime --bin irohad ensure_inrou_root_disk_copies_once_and_reuses_existing_rootfs -- --nocapture"
cargo test --locked -p irohad --features embedded-soracloud-runtime --bin irohad ensure_inrou_root_disk_copies_once_and_reuses_existing_rootfs -- --nocapture

echo "+ cargo test --locked -p irohad --features embedded-soracloud-runtime --bin irohad planned_inrou_tap_firewall_rules_keep_isolated_policy_private -- --nocapture"
cargo test --locked -p irohad --features embedded-soracloud-runtime --bin irohad planned_inrou_tap_firewall_rules_keep_isolated_policy_private -- --nocapture

echo "+ cargo test --locked -p irohad --features embedded-soracloud-runtime --bin irohad inrou_linux_kvm_smoke_boots_debian_guest_and_serves_healthcheck -- --ignored --nocapture"
cargo test --locked -p irohad --features embedded-soracloud-runtime --bin irohad inrou_linux_kvm_smoke_boots_debian_guest_and_serves_healthcheck -- --ignored --nocapture

echo "+ cargo test --locked -p irohad --features embedded-soracloud-runtime --bin irohad inrou_linux_kvm_smoke_shares_service_volume_across_replicas_and_keeps_root_state_isolated -- --ignored --nocapture"
cargo test --locked -p irohad --features embedded-soracloud-runtime --bin irohad inrou_linux_kvm_smoke_shares_service_volume_across_replicas_and_keeps_root_state_isolated -- --ignored --nocapture

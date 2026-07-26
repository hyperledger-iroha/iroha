#!/usr/bin/env bash
set -euo pipefail

missing=()
firecracker_missing=()

require_one() {
  local label="$1"
  shift
  local candidate
  for candidate in "$@"; do
    if command -v "$candidate" >/dev/null 2>&1; then
      return 0
    fi
  done
  missing+=("$label ($*)")
}

require_cmd() {
  local cmd="$1"
  if ! command -v "$cmd" >/dev/null 2>&1; then
    missing+=("$cmd")
  fi
}

require_firecracker_cmd() {
  local cmd="$1"
  if ! command -v "$cmd" >/dev/null 2>&1; then
    firecracker_missing+=("$cmd")
  fi
}

case "$(uname -m)" in
  x86_64|amd64)
    require_one "qemu system emulator" qemu-system-x86_64 qemu-system-x86
    ;;
  aarch64|arm64)
    require_one "qemu system emulator" qemu-system-aarch64 qemu-system-arm
    ;;
  *)
    require_one "qemu system emulator" qemu-system-x86_64 qemu-system-aarch64 qemu-system-arm
    ;;
esac

require_cmd qemu-img

if [[ -e /dev/kvm ]]; then
  require_firecracker_cmd firecracker
  require_firecracker_cmd ip
  require_firecracker_cmd iptables
  require_firecracker_cmd mke2fs
fi

if ((${#missing[@]} > 0)); then
  {
    echo "Taira Inrou host prerequisites are missing; refusing to start in production mode."
    echo "Install the missing tools or run the CONFIG_PROFILE=taira container image."
    printf 'missing: %s\n' "${missing[@]}"
  } >&2
  exit 1
fi

if ((${#firecracker_missing[@]} > 0)); then
  {
    echo "Taira Inrou Firecracker/KVM acceleration is unavailable; PortableVm can still host replicas."
    printf 'optional missing: %s\n' "${firecracker_missing[@]}"
  } >&2
fi

echo "Taira Inrou host prerequisites OK."

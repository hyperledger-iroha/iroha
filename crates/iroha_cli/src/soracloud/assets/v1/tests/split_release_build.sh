#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
mkdir -p "$SCRIPT_DIR/frontend/dist" "$SCRIPT_DIR/services/live/build" "$SCRIPT_DIR/services/vault/build"
mkdir -p "$SCRIPT_DIR/services/live/inrou/x86_64" "$SCRIPT_DIR/services/live/inrou/aarch64"
printf '<!doctype html><title>Travel Ops</title>' > "$SCRIPT_DIR/frontend/dist/index.html"
printf 'release-live-bundle' > "$SCRIPT_DIR/services/live/build/live-api.tgz"
printf 'release-vault-bundle' > "$SCRIPT_DIR/services/vault/build/vault-api.to"
printf 'x86-kernel' > "$SCRIPT_DIR/services/live/inrou/x86_64/vmlinux"
printf 'x86-rootfs' > "$SCRIPT_DIR/services/live/inrou/x86_64/rootfs.ext4"
printf 'x86-initrd' > "$SCRIPT_DIR/services/live/inrou/x86_64/initrd.img"
printf 'arm-kernel' > "$SCRIPT_DIR/services/live/inrou/aarch64/vmlinux"
printf 'arm-rootfs' > "$SCRIPT_DIR/services/live/inrou/aarch64/rootfs.ext4"
printf 'arm-initrd' > "$SCRIPT_DIR/services/live/inrou/aarch64/initrd.img"

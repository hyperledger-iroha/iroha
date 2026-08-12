#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
mkdir -p "$SCRIPT_DIR/frontend/dist" "$SCRIPT_DIR/services/live/build" "$SCRIPT_DIR/services/vault/build"
printf '<!doctype html><title>Travel Ops</title>' > "$SCRIPT_DIR/frontend/dist/index.html"
printf 'release-live-bundle' > "$SCRIPT_DIR/services/live/build/live-api.tgz"
printf 'release-vault-bundle' > "$SCRIPT_DIR/services/vault/build/vault-api.to"

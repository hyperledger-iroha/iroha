#!/usr/bin/env sh
# Copyright 2024 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

# Thin wrapper that delegates to the cross-platform Python helper.

set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
TARGET="$SCRIPT_DIR/check_norito_bindings_sync.py"

if command -v python3 >/dev/null 2>&1; then
  exec python3 "$TARGET" "$@"
elif command -v python >/dev/null 2>&1; then
  exec python "$TARGET" "$@"
else
  echo "Python interpreter not found. Install Python 3 before running this check." >&2
  exit 1
fi

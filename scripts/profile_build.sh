#!/bin/sh
# Profile an Iroha build without cleaning or reusing the developer target tree.
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
exec python3 "$SCRIPT_DIR/profile_build.py" "$@"

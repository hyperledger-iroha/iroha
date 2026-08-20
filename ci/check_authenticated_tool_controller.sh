#!/usr/bin/env bash
# Qualify the native authenticated-tool controller, including hostile OS probes.
set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
SOURCE="$ROOT_DIR/crates/iroha_kagami/src/bin/iroha_authenticated_tool_controller.rs"
QUALIFICATION_PARENT="$(cd "${TMPDIR:-/tmp}" && pwd -P)"
QUALIFICATION_ROOT="$(mktemp -d "$QUALIFICATION_PARENT/iroha-authenticated-tool-controller.XXXXXX")"

cleanup() {
  case "$QUALIFICATION_ROOT" in
    "$QUALIFICATION_PARENT"/iroha-authenticated-tool-controller.*)
      rm -rf -- "$QUALIFICATION_ROOT"
      ;;
    *)
      printf 'refusing unsafe qualification cleanup path: %s\n' "$QUALIFICATION_ROOT" >&2
      return 1
      ;;
  esac
}
trap cleanup EXIT

rustc --edition 2024 -D warnings -D unsafe-code "$SOURCE" \
  -o "$QUALIFICATION_ROOT/iroha_authenticated_tool_controller"
rustc --edition 2024 -D warnings -D unsafe-code --test "$SOURCE" \
  -o "$QUALIFICATION_ROOT/iroha_authenticated_tool_controller_tests"
"$QUALIFICATION_ROOT/iroha_authenticated_tool_controller_tests"
if [[ "$(uname -s)" == "Darwin" ]]; then
  env -i \
    LANG=C \
    LC_ALL=C \
    PATH=/usr/bin:/bin \
    TMPDIR="$QUALIFICATION_ROOT" \
    "$QUALIFICATION_ROOT/iroha_authenticated_tool_controller" qualify-host-v1
else
  set +e
  HOST_QUALIFICATION_OUTPUT="$(
    env -i \
      LANG=C \
      LC_ALL=C \
      PATH=/usr/bin:/bin \
      TMPDIR="$QUALIFICATION_ROOT" \
      "$QUALIFICATION_ROOT/iroha_authenticated_tool_controller" qualify-host-v1 2>&1
  )"
  HOST_QUALIFICATION_STATUS=$?
  set -e
  test "$HOST_QUALIFICATION_STATUS" -eq 125
  test "$HOST_QUALIFICATION_OUTPUT" = \
    "authenticated-tool-controller: host qualification is unavailable without the macOS backend"
fi
python3 -m pytest -q "$ROOT_DIR/scripts/tests/authenticated_tool_controller_test.py"

#!/usr/bin/env bash
# Qualify the native authenticated-tool controller, including hostile OS probes.
set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
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

BUILD_MESSAGES="$QUALIFICATION_ROOT/cargo-build.jsonl"
cargo rustc \
  --locked \
  --manifest-path "$ROOT_DIR/Cargo.toml" \
  --package iroha_kagami \
  --features dev-tools \
  --bin iroha_authenticated_tool_controller \
  --message-format=json-render-diagnostics \
  -- \
  -D warnings \
  -D unsafe-code \
  >"$BUILD_MESSAGES"
CONTROLLER="$(
  python3 -I -S -B - "$BUILD_MESSAGES" <<'PY'
import json
import os
from pathlib import Path
import stat
import sys

messages = Path(sys.argv[1]).read_text(encoding="utf-8").splitlines()
candidates = []
for line in messages:
    message = json.loads(line)
    target = message.get("target")
    executable = message.get("executable")
    if (
        message.get("reason") == "compiler-artifact"
        and isinstance(target, dict)
        and target.get("name") == "iroha_authenticated_tool_controller"
        and isinstance(executable, str)
    ):
        raw_candidate = Path(executable)
        if (
            not raw_candidate.is_absolute()
            or str(raw_candidate) != executable
            or raw_candidate.is_symlink()
        ):
            raise SystemExit("Cargo reported a non-canonical controller executable")
        candidate = raw_candidate.resolve(strict=True)
        metadata = candidate.lstat()
        if (
            candidate != raw_candidate
            or not stat.S_ISREG(metadata.st_mode)
            or not os.access(candidate, os.X_OK)
        ):
            raise SystemExit("Cargo produced a non-regular controller executable")
        candidates.append(candidate)
if len(candidates) != 1:
    raise SystemExit("Cargo did not report one exact controller executable")
print(candidates[0])
PY
)"
cargo test \
  --locked \
  --manifest-path "$ROOT_DIR/Cargo.toml" \
  --package iroha_kagami \
  --features dev-tools \
  --bin iroha_authenticated_tool_controller
if [[ "$(uname -s)" == "Darwin" ]]; then
  env -i \
    LANG=C \
    LC_ALL=C \
    PATH=/usr/bin:/bin \
    TMPDIR="$QUALIFICATION_ROOT" \
    "$CONTROLLER" qualify-host-v1
else
  set +e
  HOST_QUALIFICATION_OUTPUT="$(
    env -i \
      LANG=C \
      LC_ALL=C \
      PATH=/usr/bin:/bin \
      TMPDIR="$QUALIFICATION_ROOT" \
      "$CONTROLLER" qualify-host-v1 2>&1
  )"
  HOST_QUALIFICATION_STATUS=$?
  set -e
  test "$HOST_QUALIFICATION_STATUS" -eq 125
  test "$HOST_QUALIFICATION_OUTPUT" = \
    "authenticated-tool-controller: host qualification is unavailable without the macOS backend"
fi
IROHA_AUTHENTICATED_TOOL_CONTROLLER_BINARY="$CONTROLLER" \
  python3 -m pytest -q "$ROOT_DIR/scripts/tests/authenticated_tool_controller_test.py"

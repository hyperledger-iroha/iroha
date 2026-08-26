#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
PYTHON_BIN="${MOBILE_SDK_PYTHON_BINARY:-${PYTHON_BIN:-python3}}"

if ! command -v cargo >/dev/null 2>&1; then
  echo "error: cargo is required to build the authoritative Offline Cash Swift fixture" >&2
  exit 1
fi
if ! command -v "${PYTHON_BIN}" >/dev/null 2>&1; then
  echo "error: Python is required to authenticate the Offline Cash Swift fixture path" >&2
  exit 1
fi

cd "${ROOT_DIR}"
if [[ $# -eq 0 ]]; then
  set -- --locked
fi

# Consume Cargo's artifact record instead of guessing a debug/profile/target
# layout. This binds the exported path to the executable produced by this exact
# invocation even when a caller selects a target, profile, or configured target
# directory. Cargo diagnostics are replayed on stderr; stdout remains the one
# machine-readable path that callers export.
echo "[offline-cash-swift-fixture] building same-revision generator" >&2
fixture="$(
  cargo build "$@" \
    -p connect_norito_bridge \
    --example kotlin_offline_cash_v1 \
    --message-format=json-render-diagnostics |
    "${PYTHON_BIN}" -I -S -B -c '
import json
import sys

executables = set()
build_success = None
for raw_line in sys.stdin:
    try:
        record = json.loads(raw_line)
    except json.JSONDecodeError as error:
        raise SystemExit(f"error: Cargo emitted invalid artifact JSON: {error}")
    if record.get("reason") == "compiler-message":
        rendered = record.get("message", {}).get("rendered")
        if rendered:
            print(rendered, end="" if rendered.endswith("\n") else "\n", file=sys.stderr)
    if (
        record.get("reason") == "compiler-artifact"
        and record.get("target", {}).get("name") == "kotlin_offline_cash_v1"
        and "example" in record.get("target", {}).get("kind", [])
        and isinstance(record.get("executable"), str)
    ):
        executables.add(record["executable"])
    if record.get("reason") == "build-finished":
        build_success = record.get("success")
if build_success is not True or len(executables) != 1:
    raise SystemExit(
        "error: Cargo did not emit exactly one authoritative Offline Cash fixture executable"
    )
print(executables.pop())
'
)"
[[ "${fixture}" == /* && -f "${fixture}" && -x "${fixture}" && ! -L "${fixture}" ]] || {
  echo "error: authoritative Offline Cash Swift fixture is not a regular executable" >&2
  exit 1
}
fixture="$(
  "${PYTHON_BIN}" -I -S -B -c \
    'import pathlib,sys; print(pathlib.Path(sys.argv[1]).resolve(strict=True))' \
    "${fixture}"
)"

printf '%s\n' "${fixture}"

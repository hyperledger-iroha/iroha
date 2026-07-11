#!/usr/bin/env bash
set -euo pipefail
umask 077

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PYTHON_BIN="${SCCP_CORRIDOR_PYTHON_BIN:-python3}"
NODE_BIN="${SCCP_CORRIDOR_NODE_BIN:-node}"
DOCKER_BIN="${SCCP_TVM_DOCKER_BIN:-docker}"
TVM_PORT="${SCCP_TVM_PORT:-19090}"
NPM_BIN="${SCCP_CORRIDOR_NPM_BIN:-npm}"

usage() {
  echo "usage: scripts/contract_tvm_runner.sh --manifest PATH" >&2
}

MANIFEST=""
while (($#)); do
  case "$1" in
    --manifest)
      [[ $# -ge 2 ]] || { usage; exit 2; }
      MANIFEST="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      usage
      exit 2
      ;;
  esac
done

if [[ -z "$MANIFEST" || ! -f "$MANIFEST" || -L "$MANIFEST" ]]; then
  echo "TVM execution requires one direct authenticated contract manifest file." >&2
  exit 1
fi
if ! [[ "$TVM_PORT" =~ ^[0-9]+$ ]] || ((TVM_PORT < 1024 || TVM_PORT > 65535)); then
  echo "SCCP_TVM_PORT must be a non-privileged TCP port." >&2
  exit 1
fi
if ! command -v "$DOCKER_BIN" >/dev/null 2>&1; then
  echo "TVM execution was requested but the Docker CLI is unavailable; Ganache is not TVM evidence." >&2
  exit 1
fi
if ! "$DOCKER_BIN" version >/dev/null 2>&1; then
  echo "TVM execution was requested but no Docker daemon is reachable; Ganache is not TVM evidence." >&2
  exit 1
fi

TVM_LOCK_OUTPUT="$(
  "$PYTHON_BIN" - "$ROOT_DIR/scripts" "$ROOT_DIR/scripts/contract_tooling/compiler-lock.json" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import contract_artifact_corridor as corridor

config = corridor.load_corridor_config(Path(sys.argv[2]))
print(config.tvm_runner["image"])
print(config.tvm_runner["platform"])
PY
)"
TVM_IMAGE="${TVM_LOCK_OUTPUT%%$'\n'*}"
TVM_PLATFORM="${TVM_LOCK_OUTPUT#*$'\n'}"
if [[ -z "$TVM_IMAGE" || -z "$TVM_PLATFORM" || "$TVM_PLATFORM" == *$'\n'* ]]; then
  echo "TVM runner lock is malformed." >&2
  exit 1
fi
if [[ -n "${SCCP_TVM_IMAGE:-}" && "$SCCP_TVM_IMAGE" != "$TVM_IMAGE" ]]; then
  echo "SCCP_TVM_IMAGE cannot override the immutable official TRE digest." >&2
  exit 1
fi

WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/iroha-sccp-tvm.XXXXXX")"
CONTAINER_NAME="iroha-sccp-tvm-$$"
cleanup() {
  "$DOCKER_BIN" rm -f "$CONTAINER_NAME" >/dev/null 2>&1 || true
  chmod -R u+w "$WORK_DIR" >/dev/null 2>&1 || true
  rm -rf "$WORK_DIR"
}
trap cleanup EXIT

SNAPSHOT_DIR="$WORK_DIR/runtime-inputs"
"$PYTHON_BIN" "$ROOT_DIR/scripts/contract_artifact_corridor.py" snapshot \
  --manifest "$MANIFEST" \
  --native-vectors "$ROOT_DIR/fixtures/sccp/native_transfer_event_v1.json" \
  --output-dir "$SNAPSHOT_DIR"
SNAPSHOT_MANIFEST="$SNAPSHOT_DIR/sccp-contract-artifacts-v1.json"
SNAPSHOT_VECTORS="$SNAPSHOT_DIR/native-transfer-event-v1.json"
"$PYTHON_BIN" "$ROOT_DIR/scripts/contract_artifact_corridor.py" verify \
  --manifest "$SNAPSHOT_MANIFEST" \
  --compiler-lock "$ROOT_DIR/scripts/contract_tooling/compiler-lock.json" \
  --artifact-lock "$ROOT_DIR/scripts/contract_tooling/artifact-lock.json" \
  --repo-root "$ROOT_DIR" \
  --check-source-inputs

mkdir -p "$WORK_DIR/tooling"
cp "$ROOT_DIR/scripts/contract_tooling/package.json" "$WORK_DIR/tooling/package.json"
cp "$ROOT_DIR/scripts/contract_tooling/package-lock.json" "$WORK_DIR/tooling/package-lock.json"
(
  cd "$WORK_DIR/tooling"
  "$NPM_BIN" ci --ignore-scripts --no-audit --no-fund --loglevel=error
  "$NPM_BIN" audit --omit=dev --audit-level=low
)

NODE_PATH="$WORK_DIR/tooling/node_modules" \
SCCP_TVM_STATIC_ONLY=1 \
  "$NODE_BIN" "$ROOT_DIR/scripts/contract_tvm_smoke.mjs" \
    "$SNAPSHOT_MANIFEST" "$SNAPSHOT_VECTORS"

"$DOCKER_BIN" run --detach --rm --pull always \
  --name "$CONTAINER_NAME" \
  --platform "$TVM_PLATFORM" \
  --publish "127.0.0.1:${TVM_PORT}:9090" \
  "$TVM_IMAGE" >/dev/null

TVM_ENDPOINT="http://127.0.0.1:${TVM_PORT}"
ready=0
for _attempt in $(seq 1 120); do
  if "$PYTHON_BIN" - "$TVM_ENDPOINT/wallet/getnodeinfo" >/dev/null 2>&1 <<'PY'
import json
import sys
import urllib.request

request = urllib.request.Request(
    sys.argv[1], data=b"{}", headers={"Content-Type": "application/json"}, method="POST"
)
with urllib.request.urlopen(request, timeout=2) as response:
    if response.status != 200:
        raise SystemExit(1)
    value = json.load(response)
    if not isinstance(value, dict):
        raise SystemExit(1)
PY
  then
    ready=1
    break
  fi
  sleep 2
done
if [[ "$ready" -ne 1 ]]; then
  echo "Official TRE container did not expose a ready TVM node within 240 seconds; logs are withheld because TRE may print test private keys." >&2
  exit 1
fi

NODE_PATH="$WORK_DIR/tooling/node_modules" \
SCCP_TVM_ENDPOINT="$TVM_ENDPOINT" \
  "$NODE_BIN" "$ROOT_DIR/scripts/contract_tvm_smoke.mjs" \
    "$SNAPSHOT_MANIFEST" "$SNAPSHOT_VECTORS"

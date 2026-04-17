#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/../../.." && pwd)"
LOCAL_TORII_ROOT="${LOCAL_TORII_ROOT:-http://127.0.0.1:18080}"
PUBLIC_TORII_ROOT="${PUBLIC_TORII_ROOT:-}"
LOCAL_MCP_URL="${LOCAL_MCP_URL:-}"
PUBLIC_MCP_URL="${PUBLIC_MCP_URL:-}"
IROHA_BIN="${IROHA_BIN:-}"
WRITE_CONFIG="${WRITE_CONFIG:-}"
WRITE_CONFIG_DEFAULT="${WRITE_CONFIG_DEFAULT:-}"
WRITE_TARGET="${WRITE_TARGET:-}"
WRITE_MESSAGE_PREFIX="${WRITE_MESSAGE_PREFIX:-taira-rollout-canary}"
ROLLOUT_CANARY_ALIAS_PREFIX="${ROLLOUT_CANARY_ALIAS_PREFIX:-taira-rollout-canary}"
ROLLOUT_CANARY_TIME_TO_LIVE_MS="${ROLLOUT_CANARY_TIME_TO_LIVE_MS:-120000}"
ROLLOUT_CANARY_STATUS_TIMEOUT_MS="${ROLLOUT_CANARY_STATUS_TIMEOUT_MS:-120000}"
MIN_VALIDATOR_SET_LEN="${MIN_VALIDATOR_SET_LEN:-4}"
PUBLIC_LANE_ID="${PUBLIC_LANE_ID:-0}"
CONTRACT_NAMESPACE="${CONTRACT_NAMESPACE:-universal}"
SKIP_LOCAL=0
SKIP_PUBLIC=0
SKIP_WRITE_CANARY=0
IROHA_RUNNER=()
CHECKED_LABELS=()
CHECKED_ROOTS=()
CURL_RESOLVE_RULES=()
CURL_URL_RESOLVE_ARGS=()

usage() {
  cat <<'EOF'
Usage: check_mcp_rollout.sh [--local-root URL] [--public-root URL] [--local-url URL] [--public-url URL]
                            [--skip-local] [--skip-public]
                            [--write-config PATH] [--write-target local|public|URL]
                            [--iroha-bin PATH] [--resolve-host HOST:IP|HOST:PORT:IP]
                            [--skip-write-canary]

Verify that Taira's native Torii MCP endpoint is live locally and/or publicly.
The check fails unless:
  - GET /v1/mcp returns HTTP 200 with a capabilities payload
  - POST /v1/mcp initialize returns HTTP 200
  - POST /v1/mcp notifications/initialized returns HTTP 202 with an empty body
  - POST /v1/mcp tools/list returns HTTP 200
  - the tool list includes curated iroha.* names, including write-ready aliases
  - every advertised MCP tool publishes an OpenAI-compatible top-level
    `inputSchema` object (no top-level anyOf/oneOf/allOf/enum/not)
  - the tool list does not expose raw torii.* names
  - GET /status returns healthy Torii/Sumeragi counters
  - /status reports at least 4 validators in the commit QC set
  - direct public Torii ingress also exposes SCCP, ZK, bridge, validator-set,
    public-lane, contract, and Musubi routes on the same node URL

When diagnosing public write failures, prefer `/status` fields such as
`blocks`, `queue_size`, `sumeragi.commit_qc_height`,
`sumeragi.tx_queue_depth`, `sumeragi.tx_queue_saturated`, and
`teu_dataspace_backlog`. Do not use `/status.peers` as validator-set size; it
is the queried node's current remote-peer count.

For final public rollout, use a runtime-only canary signer config. When
`--write-config` is omitted, the script bootstraps a runtime-only canary config
automatically, preferring `/run/secrets/taira-canary-client.toml` when that
directory is writable and otherwise falling back to `${TMPDIR:-/tmp}`. It
onboards a fresh ordinary account on Taira and attempts an initial faucet claim
before the signed write canary. The write canary still retries the faucet lane
on `Failed to find asset` so a saturated queue does not require manual signer
preparation. Use `--skip-write-canary` only for read-only validation.

When `--iroha-bin` is omitted, the script first reuses a repo-local
`bin/iroha`, `target/debug/iroha`, or `target/release/iroha` if present, and
otherwise falls back to `cargo run -p iroha_cli --bin iroha -- ...`.

Public checks intentionally require an explicit public node URL
(`--public-root https://<public-torii-root>` or
`--public-url https://<public-torii-root>/v1/mcp`).
`https://taira.sora.org` is the current public Torii root, but this script
still requires an explicit URL so operators do not accidentally validate the
wrong edge or validator hostname.
EOF
}

default_write_config_path() {
  if [[ -n "$WRITE_CONFIG_DEFAULT" ]]; then
    printf '%s\n' "$WRITE_CONFIG_DEFAULT"
    return 0
  fi

  local linux_secret_path="/run/secrets/taira-canary-client.toml"
  local linux_secret_dir="${linux_secret_path%/*}"
  if [[ -d "$linux_secret_dir" && -w "$linux_secret_dir" ]]; then
    printf '%s\n' "$linux_secret_path"
    return 0
  fi

  local temp_root="${TMPDIR:-/tmp}"
  printf '%s\n' "${temp_root%/}/taira-canary-client.toml"
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --local-root)
      [[ $# -ge 2 ]] || {
        echo "missing value for --local-root" >&2
        exit 1
      }
      LOCAL_TORII_ROOT="$2"
      shift 2
      ;;
    --public-root)
      [[ $# -ge 2 ]] || {
        echo "missing value for --public-root" >&2
        exit 1
      }
      PUBLIC_TORII_ROOT="$2"
      shift 2
      ;;
    --local-url)
      [[ $# -ge 2 ]] || {
        echo "missing value for --local-url" >&2
        exit 1
      }
      LOCAL_MCP_URL="$2"
      shift 2
      ;;
    --public-url)
      [[ $# -ge 2 ]] || {
        echo "missing value for --public-url" >&2
        exit 1
      }
      PUBLIC_MCP_URL="$2"
      shift 2
      ;;
    --skip-local)
      SKIP_LOCAL=1
      shift
      ;;
    --skip-public)
      SKIP_PUBLIC=1
      shift
      ;;
    --write-config)
      [[ $# -ge 2 ]] || {
        echo "missing value for --write-config" >&2
        exit 1
      }
      WRITE_CONFIG="$2"
      shift 2
      ;;
    --write-target)
      [[ $# -ge 2 ]] || {
        echo "missing value for --write-target" >&2
        exit 1
      }
      WRITE_TARGET="$2"
      shift 2
      ;;
    --iroha-bin)
      [[ $# -ge 2 ]] || {
        echo "missing value for --iroha-bin" >&2
        exit 1
      }
      IROHA_BIN="$2"
      shift 2
      ;;
    --resolve-host)
      [[ $# -ge 2 ]] || {
        echo "missing value for --resolve-host" >&2
        exit 1
      }
      CURL_RESOLVE_RULES+=("$2")
      shift 2
      ;;
    --skip-write-canary)
      SKIP_WRITE_CANARY=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if [[ $SKIP_LOCAL -eq 1 && $SKIP_PUBLIC -eq 1 ]]; then
  echo "nothing to check: both local and public checks were skipped" >&2
  exit 1
fi

if [[ -n "$WRITE_CONFIG" && $SKIP_WRITE_CANARY -eq 1 ]]; then
  echo "--write-config and --skip-write-canary are mutually exclusive" >&2
  exit 1
fi

if [[ $SKIP_PUBLIC -eq 0 && -z "$WRITE_CONFIG" && $SKIP_WRITE_CANARY -eq 0 ]]; then
  WRITE_CONFIG="$(default_write_config_path)"
fi

if [[ -z "$IROHA_BIN" ]]; then
  if [[ -x "${REPO_ROOT}/bin/iroha" ]]; then
    IROHA_BIN="${REPO_ROOT}/bin/iroha"
  elif [[ -x "${REPO_ROOT}/target/debug/iroha" ]]; then
    IROHA_BIN="${REPO_ROOT}/target/debug/iroha"
  elif [[ -x "${REPO_ROOT}/target/release/iroha" ]]; then
    IROHA_BIN="${REPO_ROOT}/target/release/iroha"
  fi
fi

JSONRPC_TOOLS_LIST='{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}'
JSONRPC_INITIALIZE='{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"taira-rollout-smoke","version":"1"}}}'
JSONRPC_INITIALIZED='{"jsonrpc":"2.0","method":"notifications/initialized"}'
REQUIRED_TOOL_NAMES=(
  "iroha.status"
  "iroha.sumeragi.status"
  "iroha.time.now"
  "iroha.musubi.search"
  "iroha.musubi.release.get"
  "iroha.musubi.instructions.yank_release"
  "iroha.transactions.submit"
  "iroha.transactions.submit_and_wait"
)

last_body=""
last_headers=""
last_status=""

cleanup() {
  [[ -n "$last_body" && -f "$last_body" ]] && rm -f "$last_body"
  [[ -n "$last_headers" && -f "$last_headers" ]] && rm -f "$last_headers"
  return 0
}

trap cleanup EXIT

normalize_root_url() {
  local url="$1"
  printf '%s\n' "${url%/}"
}

mcp_url_from_root() {
  local root_url
  root_url="$(normalize_root_url "$1")"
  printf '%s/v1/mcp\n' "$root_url"
}

mcp_root_from_url() {
  local url="$1"
  printf '%s\n' "${url%/v1/mcp}"
}

build_curl_resolve_args() {
  local url="$1"
  CURL_URL_RESOLVE_ARGS=()
  [[ ${#CURL_RESOLVE_RULES[@]} -gt 0 ]] || return 0

  local host port
  read -r host port < <(python3 - "$url" <<'PY'
import sys
from urllib.parse import urlparse

parsed = urlparse(sys.argv[1])
host = parsed.hostname or ""
if parsed.port is not None:
    port = parsed.port
elif parsed.scheme == "https":
    port = 443
else:
    port = 80
print(host, port)
PY
)

  [[ -n "$host" && -n "$port" ]] || return 0

  local rule rule_host remainder rule_port rule_ip
  for rule in "${CURL_RESOLVE_RULES[@]}"; do
    rule_host="${rule%%:*}"
    remainder="${rule#*:}"
    if [[ "$remainder" == *:* ]]; then
      rule_port="${remainder%%:*}"
      rule_ip="${remainder#*:}"
    else
      rule_port="$port"
      rule_ip="$remainder"
    fi
    if [[ "$rule_host" == "$host" && -n "$rule_ip" ]]; then
      CURL_URL_RESOLVE_ARGS+=(--resolve "${host}:${rule_port}:${rule_ip}")
    fi
  done
}

if [[ -z "$LOCAL_MCP_URL" ]]; then
  LOCAL_MCP_URL="$(mcp_url_from_root "$LOCAL_TORII_ROOT")"
fi

if [[ $SKIP_PUBLIC -eq 0 && -z "$PUBLIC_MCP_URL" ]]; then
  if [[ -z "$PUBLIC_TORII_ROOT" ]]; then
    echo "public rollout checks require an explicit --public-root or --public-url; pass the exact Torii root you want to validate, for example https://taira.sora.org" >&2
    exit 1
  fi
  PUBLIC_MCP_URL="$(mcp_url_from_root "$PUBLIC_TORII_ROOT")"
fi

status_is_one_of() {
  local actual="$1"
  shift
  local expected
  for expected in "$@"; do
    if [[ "$actual" == "$expected" ]]; then
      return 0
    fi
  done
  return 1
}

http_request() {
  local method="$1"
  local url="$2"
  local payload="${3:-}"
  local body_file header_file
  local curl_cmd=(curl --silent --show-error)

  body_file="$(mktemp)"
  header_file="$(mktemp)"
  cleanup
  last_body="$body_file"
  last_headers="$header_file"
  build_curl_resolve_args "$url"
  curl_cmd+=( ${CURL_URL_RESOLVE_ARGS[@]+"${CURL_URL_RESOLVE_ARGS[@]}"} )

  if [[ "$method" == "GET" ]]; then
    last_status="$(
      "${curl_cmd[@]}" \
      --output "$body_file" \
      --dump-header "$header_file" \
      --write-out "%{http_code}" \
      "$url"
    )"
  else
    last_status="$(
      "${curl_cmd[@]}" \
      --output "$body_file" \
      --dump-header "$header_file" \
      --write-out "%{http_code}" \
      -X POST \
      -H "content-type: application/json" \
      --data "$payload" \
      "$url"
    )"
  fi
}

print_status_route_diagnostics() {
  local target_url="$1"
  local status_url
  status_url="$(normalize_root_url "$target_url")/status"

  echo "==> diagnostic: GET ${status_url}" >&2
  http_request GET "$status_url"
  if [[ "$last_status" != "200" ]]; then
    echo "status diagnostic failed with HTTP ${last_status}" >&2
    sed -n '1,20p' "$last_headers" >&2 || true
    sed -n '1,40p' "$last_body" >&2 || true
    return 0
  fi

  python3 - "$last_body" <<'PY' >&2
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)

sumeragi = payload.get("sumeragi") or {}
teu_backlog = None
backlog_entries = payload.get("teu_dataspace_backlog")
if isinstance(backlog_entries, list) and backlog_entries:
    first = backlog_entries[0]
    if isinstance(first, dict):
        teu_backlog = first.get("backlog")

summary = {
    "blocks": payload.get("blocks"),
    "queue_size": payload.get("queue_size"),
    "commit_qc_height": sumeragi.get("commit_qc_height"),
    "tx_queue_depth": sumeragi.get("tx_queue_depth"),
    "tx_queue_saturated": sumeragi.get("tx_queue_saturated"),
    "teu_dataspace_backlog": teu_backlog,
}
print(json.dumps(summary, ensure_ascii=False, sort_keys=True))
PY
}

print_sumeragi_route_diagnostics() {
  local target_url="$1"
  local sumeragi_url
  sumeragi_url="$(normalize_root_url "$target_url")/v1/sumeragi/status"

  echo "==> diagnostic: GET ${sumeragi_url}" >&2
  http_request GET "$sumeragi_url"
  if [[ "$last_status" != "200" ]]; then
    echo "sumeragi diagnostic failed with HTTP ${last_status}" >&2
    sed -n '1,20p' "$last_headers" >&2 || true
    sed -n '1,40p' "$last_body" >&2 || true
    return 0
  fi

  python3 - "$last_body" <<'PY' >&2
import json
import sys

def dig(obj, *path):
    cur = obj
    for key in path:
        if not isinstance(cur, dict):
            return None
        cur = cur.get(key)
    return cur

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)

summary = {
    "commit_qc_height": payload.get("commit_qc_height", dig(payload, "commit_qc", "height")),
    "highest_qc_height": payload.get("highest_qc_height", dig(payload, "highest_qc", "height")),
    "tx_queue_depth": payload.get("tx_queue_depth"),
    "tx_queue_saturated": payload.get("tx_queue_saturated"),
    "view_change_last_cause": dig(payload, "view_change_causes", "last_cause"),
    "worker_loop_stage": dig(payload, "worker_loop", "stage"),
}
print(json.dumps(summary, ensure_ascii=False, sort_keys=True))
PY
}

classify_public_ingress_failure() {
  local target_url="$1"
  local status_url mcp_url

  status_url="$(normalize_root_url "$target_url")/status"
  mcp_url="$(normalize_root_url "$target_url")/v1/mcp"

  http_request GET "$status_url"
  if [[ "$last_status" == "502" || "$last_status" == "503" ]]; then
    echo "write canary failed: public Torii ingress looks degraded (${status_url} -> HTTP ${last_status})" >&2
    sed -n '1,20p' "$last_headers" >&2 || true
    sed -n '1,40p' "$last_body" >&2 || true
    return 0
  fi

  http_request GET "$mcp_url"
  if [[ "$last_status" == "502" || "$last_status" == "503" ]]; then
    echo "write canary failed: public MCP ingress looks degraded (${mcp_url} -> HTTP ${last_status})" >&2
    sed -n '1,20p' "$last_headers" >&2 || true
    sed -n '1,40p' "$last_body" >&2 || true
    return 0
  fi

  return 1
}

check_required_tools() {
  local label="$1"
  python3 - "$label" "$last_body" "${REQUIRED_TOOL_NAMES[@]}" <<'PY'
import json
import sys

label = sys.argv[1]
path = sys.argv[2]
required = sys.argv[3:]
with open(path, "r", encoding="utf-8") as handle:
    payload = json.load(handle)

tools = payload.get("result", {}).get("tools", [])
names = {
    entry.get("name", "")
    for entry in tools
    if isinstance(entry, dict) and isinstance(entry.get("name"), str)
}
missing = [name for name in required if name not in names]
raw = sorted(name for name in names if name.startswith("torii."))
if missing:
    print(f"{label}: tools/list is missing required curated tools: {', '.join(missing)}", file=sys.stderr)
    sys.exit(1)
if raw:
    print(f"{label}: tools/list still exposes raw torii.* tool names: {', '.join(raw[:8])}", file=sys.stderr)
    sys.exit(1)
PY
}

check_tool_input_schemas() {
  local label="$1"
  python3 - "$label" "$last_body" <<'PY'
import json
import sys

label = sys.argv[1]
path = sys.argv[2]
with open(path, "r", encoding="utf-8") as handle:
    payload = json.load(handle)

tools = payload.get("result", {}).get("tools", [])
invalid = []
for entry in tools:
    if not isinstance(entry, dict):
        continue
    name = entry.get("name", "<unnamed>")
    schema = entry.get("inputSchema")
    if not isinstance(schema, dict):
        invalid.append(f"{name}: inputSchema is not an object")
        continue
    if schema.get("type") != "object":
        invalid.append(f"{name}: top-level type is {schema.get('type')!r}, expected 'object'")
    disallowed = [key for key in ("anyOf", "oneOf", "allOf", "enum", "not") if key in schema]
    if disallowed:
        invalid.append(f"{name}: top-level disallowed keywords present: {', '.join(disallowed)}")

if invalid:
    print(f"{label}: tools/list exposed OpenAI-incompatible MCP schemas:", file=sys.stderr)
    for item in invalid[:10]:
        print(f"  - {item}", file=sys.stderr)
    if len(invalid) > 10:
        print(f"  - ... and {len(invalid) - 10} more", file=sys.stderr)
    sys.exit(1)
PY
}

check_route_status() {
  local label="$1"
  local method="$2"
  local url="$3"
  local expected_statuses="$4"
  local description="$5"
  local payload="${6:-}"
  local -a expected_codes=()

  read -r -a expected_codes <<< "$expected_statuses"
  echo "==> ${label}: ${method} ${url}"
  http_request "$method" "$url" "$payload"
  if ! status_is_one_of "$last_status" "${expected_codes[@]}"; then
    echo "${label}: ${description} failed with HTTP ${last_status}; expected one of: ${expected_statuses}" >&2
    sed -n '1,20p' "$last_headers" >&2 || true
    sed -n '1,40p' "$last_body" >&2 || true
    exit 1
  fi
}

check_status_snapshot() {
  local label="$1"
  local status_url="$2"
  local allow_pending_commit_qc="${3:-0}"

  echo "==> ${label}: GET ${status_url}"
  http_request GET "$status_url"
  if [[ "$last_status" != "200" ]]; then
    echo "${label}: /status failed with HTTP ${last_status}" >&2
    sed -n '1,20p' "$last_headers" >&2 || true
    exit 1
  fi
  python3 - "$label" "$last_body" "$MIN_VALIDATOR_SET_LEN" "$allow_pending_commit_qc" <<'PY'
import json
import sys

label = sys.argv[1]
path = sys.argv[2]
min_validator_set_len = int(sys.argv[3])
allow_pending_commit_qc = sys.argv[4] == "1"
with open(path, "r", encoding="utf-8") as handle:
    payload = json.load(handle)

peers = payload.get("peers")
blocks = payload.get("blocks")
sumeragi = payload.get("sumeragi", {})
validator_set_len = sumeragi.get("commit_qc_validator_set_len")
if not isinstance(peers, int) or peers < 1:
    print(f"{label}: /status reported an unhealthy peer count: {peers!r}", file=sys.stderr)
    sys.exit(1)
if not isinstance(blocks, int) or blocks < 1:
    print(f"{label}: /status reported an unhealthy block height: {blocks!r}", file=sys.stderr)
    sys.exit(1)
if not isinstance(validator_set_len, int) or validator_set_len < 1:
    if allow_pending_commit_qc:
        print(
            f"{label}: /status is still missing a commit QC snapshot; "
            "deferring validator-set enforcement until after the signed write canary",
            file=sys.stderr,
        )
        sys.exit(10)
    print(
        f"{label}: /status reported an empty Sumeragi commit validator set: {validator_set_len!r}",
        file=sys.stderr,
    )
    sys.exit(1)
if validator_set_len < min_validator_set_len:
    print(
        f"{label}: /status reported only {validator_set_len} validators in the commit QC set; "
        f"Taira rollout expects at least {min_validator_set_len}. "
        "Remember that /status.peers is only the queried node's remote-peer count. "
        "Render per-validator configs from configs/soranexus/taira/validator_roster.example.toml "
        "with scripts/render_taira_validator_bundle.py before cutting traffic.",
        file=sys.stderr,
    )
    sys.exit(1)
PY
}

check_status_snapshot_with_retry() {
  local label="$1"
  local status_url="$2"
  local allow_pending_commit_qc="${3:-0}"
  local attempts="${4:-10}"
  local delay_seconds="${5:-2}"
  local attempt rc

  for ((attempt = 1; attempt <= attempts; attempt++)); do
    if check_status_snapshot "$label" "$status_url" "$allow_pending_commit_qc"; then
      return 0
    fi
    rc=$?
    if [[ $rc -eq 10 ]]; then
      return 10
    fi
    if [[ $attempt -eq $attempts ]]; then
      return "$rc"
    fi
    sleep "$delay_seconds"
  done

  return 1
}

check_route_parity() {
  local label="$1"
  local root_url="$2"
  local lane_id="$3"
  local namespace="$4"

  root_url="$(normalize_root_url "$root_url")"
  check_route_status "$label" GET "${root_url}/v1/sccp/capabilities" "200" \
    "SCCP capability discovery route"
  check_route_status "$label" GET "${root_url}/v1/sccp/manifests" "200" \
    "SCCP manifest discovery route"
  check_route_status "$label" GET "${root_url}/v1/zk/proofs/count" "200" \
    "ZK proof count route"
  check_route_status "$label" GET "${root_url}/v1/sumeragi/validator-sets" "200" \
    "validator-set snapshot route"
  check_route_status "$label" GET "${root_url}/v1/nexus/public_lanes/${lane_id}/validators" "200" \
    "public-lane validator snapshot route"
  check_route_status "$label" GET "${root_url}/v1/nexus/public_lanes/${lane_id}/stake" "200" \
    "public-lane stake snapshot route"
  check_route_status "$label" GET "${root_url}/v1/contracts/state" "400" \
    "contract state route should be mounted and reject missing query selectors"
  check_route_status "$label" GET "${root_url}/v1/musubi/packages?query=&limit=1" "200" \
    "Musubi package search route"
  check_route_status "$label" POST "${root_url}/v1/musubi/instructions/yank-release" "200" \
    "Musubi pre-signing instruction builder route" \
    '{"package":"dex.universal/swap-core@1.2.3","reason":"rollout preflight"}'
  check_route_status "$label" POST "${root_url}/v1/contracts/deploy" "400 401 403 415 422" \
    "contract deploy route should reject an empty preflight body, not be missing" '{}'
  check_route_status "$label" POST "${root_url}/v1/bridge/messages" "400 401 403 415 422" \
    "bridge message preflight should hit the mounted route, not return 404/405" '{}'
}

check_endpoint() {
  local label="$1"
  local url="$2"
  local root_url
  local allow_pending_commit_qc=0
  local status_rc=0

  echo "==> ${label}: GET ${url}"
  http_request GET "$url"
  if [[ "$last_status" != "200" ]]; then
    echo "${label}: GET failed with HTTP ${last_status}" >&2
    sed -n '1,20p' "$last_headers" >&2 || true
    exit 1
  fi
  if ! grep -q '"capabilities"' "$last_body"; then
    echo "${label}: GET response did not look like MCP capabilities payload" >&2
    sed -n '1,40p' "$last_body" >&2 || true
    exit 1
  fi

  echo "==> ${label}: POST initialize ${url}"
  http_request POST "$url" "$JSONRPC_INITIALIZE"
  if [[ "$last_status" != "200" ]]; then
    echo "${label}: initialize failed with HTTP ${last_status}" >&2
    sed -n '1,20p' "$last_headers" >&2 || true
    exit 1
  fi
  if ! grep -q '"protocolVersion"' "$last_body"; then
    echo "${label}: initialize response did not advertise protocolVersion" >&2
    sed -n '1,80p' "$last_body" >&2 || true
    exit 1
  fi

  echo "==> ${label}: POST notifications/initialized ${url}"
  http_request POST "$url" "$JSONRPC_INITIALIZED"
  if [[ "$last_status" != "202" ]]; then
    echo "${label}: initialized notification failed with HTTP ${last_status}" >&2
    sed -n '1,20p' "$last_headers" >&2 || true
    sed -n '1,40p' "$last_body" >&2 || true
    exit 1
  fi
  if [[ -s "$last_body" ]]; then
    echo "${label}: initialized notification should return an empty body" >&2
    sed -n '1,40p' "$last_body" >&2 || true
    exit 1
  fi

  echo "==> ${label}: POST tools/list ${url}"
  http_request POST "$url" "$JSONRPC_TOOLS_LIST"
  if [[ "$last_status" != "200" ]]; then
    echo "${label}: tools/list failed with HTTP ${last_status}" >&2
    sed -n '1,20p' "$last_headers" >&2 || true
    exit 1
  fi
  if ! grep -q '"iroha\.' "$last_body"; then
    echo "${label}: tools/list did not expose any curated iroha.* tool names" >&2
    sed -n '1,80p' "$last_body" >&2 || true
    exit 1
  fi
  check_required_tools "$label"
  check_tool_input_schemas "$label"

  root_url="$(mcp_root_from_url "$url")"
  CHECKED_LABELS+=("$label")
  CHECKED_ROOTS+=("$root_url")
  if [[ -n "$WRITE_CONFIG" && $SKIP_WRITE_CANARY -eq 0 ]]; then
    allow_pending_commit_qc=1
  fi
  if check_status_snapshot_with_retry "$label" "${root_url}/status" "$allow_pending_commit_qc"; then
    :
  else
    status_rc=$?
    if [[ $status_rc -ne 10 ]]; then
      exit "$status_rc"
    fi
  fi
  check_route_parity "$label" "$root_url" "$PUBLIC_LANE_ID" "$CONTRACT_NAMESPACE"
}

resolve_write_target_url() {
  if [[ -n "$WRITE_TARGET" ]]; then
    case "$WRITE_TARGET" in
      local)
        printf '%s\n' "$(mcp_root_from_url "$LOCAL_MCP_URL")"
        ;;
      public)
        printf '%s\n' "$(mcp_root_from_url "$PUBLIC_MCP_URL")"
        ;;
      *)
        printf '%s\n' "$WRITE_TARGET"
        ;;
    esac
    return 0
  fi

  if [[ $SKIP_PUBLIC -eq 0 ]]; then
    printf '%s\n' "$(mcp_root_from_url "$PUBLIC_MCP_URL")"
  else
    printf '%s\n' "$(mcp_root_from_url "$LOCAL_MCP_URL")"
  fi
}

build_write_canary_config() {
  local source_config="$1"
  local target_torii_url="$2"
  local output_config="$3"
  local time_to_live_ms="$4"
  local status_timeout_ms="$5"

  python3 - "$source_config" "$target_torii_url" "$output_config" "$time_to_live_ms" "$status_timeout_ms" <<'PY'
import sys

try:
    import tomllib
except ModuleNotFoundError:
    try:
        import tomli as tomllib
    except ModuleNotFoundError as error:
        raise SystemExit(
            "python3 must provide tomllib (Python 3.11+) or tomli to load the canary config"
        ) from error

source_path, target_torii_url, output_path, time_to_live_ms, status_timeout_ms = sys.argv[1:]
with open(source_path, "rb") as handle:
    source = tomllib.load(handle)

chain = source.get("chain")
account = source.get("account") or {}
public_key = account.get("public_key")
private_key = account.get("private_key")
chain_discriminant = account.get("chain_discriminant")
domain = account.get("domain", "wonderland.universal")
basic_auth = source.get("basic_auth")
transaction = source.get("transaction") or {}
nonce = transaction.get("nonce", False)
time_to_live_ms = int(time_to_live_ms)
status_timeout_ms = int(status_timeout_ms)

if not isinstance(chain, str) or not chain:
    raise SystemExit("write canary config is missing a top-level `chain` value")
if not isinstance(public_key, str) or not public_key:
    raise SystemExit("write canary config is missing `account.public_key`")
if not isinstance(private_key, str) or not private_key:
    raise SystemExit("write canary config is missing `account.private_key`")
if chain_discriminant is not None and not isinstance(chain_discriminant, int):
    raise SystemExit("write canary config `account.chain_discriminant` must be an integer")
if not isinstance(domain, str) or not domain:
    domain = "wonderland.universal"
elif "." not in domain:
    domain = f"{domain}.universal"
if not isinstance(nonce, bool):
    raise SystemExit("write canary config `transaction.nonce` must be a boolean when present")

lines = [
    f'chain = "{chain}"',
    f'torii_url = "{target_torii_url.rstrip("/")}/"',
]

if isinstance(basic_auth, dict):
    web_login = basic_auth.get("web_login")
    password = basic_auth.get("password")
    if isinstance(web_login, str) and isinstance(password, str):
        lines.extend(
            [
                "",
                "[basic_auth]",
                f'web_login = "{web_login}"',
                f'password = "{password}"',
            ]
        )

lines.extend(
    [
        "",
        "[account]",
        f'domain = "{domain}"',
        f'public_key = "{public_key}"',
        f'private_key = "{private_key}"',
    ]
)

if isinstance(chain_discriminant, int):
    lines.append(f'chain_discriminant = {chain_discriminant}')

lines.extend(
    [
        "",
        "[transaction]",
        f"time_to_live_ms = {time_to_live_ms}",
        f"status_timeout_ms = {status_timeout_ms}",
        f"nonce = {'true' if nonce else 'false'}",
        "",
    ]
)

with open(output_path, "w", encoding="utf-8") as handle:
    handle.write("\n".join(lines))
PY
}

ensure_iroha_bin() {
  if [[ -n "$IROHA_BIN" ]]; then
    if [[ "$IROHA_BIN" == */* ]]; then
      [[ -x "$IROHA_BIN" ]] || {
        echo "iroha binary is not executable: $IROHA_BIN" >&2
        exit 1
      }
      IROHA_RUNNER=("$IROHA_BIN")
      return 0
    fi
    if command -v "$IROHA_BIN" >/dev/null 2>&1; then
      IROHA_RUNNER=("$IROHA_BIN")
      return 0
    fi
    echo "could not find iroha binary on PATH: $IROHA_BIN" >&2
    exit 1
  fi

  if [[ -x "${REPO_ROOT}/bin/iroha" ]]; then
    IROHA_RUNNER=("${REPO_ROOT}/bin/iroha")
  elif [[ -x "${REPO_ROOT}/target/debug/iroha" ]]; then
    IROHA_RUNNER=("${REPO_ROOT}/target/debug/iroha")
  elif [[ -x "${REPO_ROOT}/target/release/iroha" ]]; then
    IROHA_RUNNER=("${REPO_ROOT}/target/release/iroha")
  elif command -v iroha >/dev/null 2>&1; then
    IROHA_RUNNER=("iroha")
  elif command -v cargo >/dev/null 2>&1; then
    IROHA_RUNNER=(
      cargo
      run
      --quiet
      --manifest-path
      "${REPO_ROOT}/Cargo.toml"
      -p
      iroha_cli
      --bin
      iroha
      --
    )
  else
    echo "could not find an iroha binary or cargo fallback" >&2
    exit 1
  fi
}

ensure_write_canary_config() {
  local target_url="$1"
  local bootstrap_cmd=(
    python3
    "${REPO_ROOT}/scripts/taira_bootstrap_canary.py"
    --torii-root "$target_url"
    --output-config "$WRITE_CONFIG"
    --alias-prefix "$ROLLOUT_CANARY_ALIAS_PREFIX"
    --time-to-live-ms "$ROLLOUT_CANARY_TIME_TO_LIVE_MS"
    --status-timeout-ms "$ROLLOUT_CANARY_STATUS_TIMEOUT_MS"
  )

  if [[ -n "$IROHA_BIN" ]]; then
    bootstrap_cmd+=(--iroha-bin "$IROHA_BIN")
  fi

  echo "==> canary bootstrap: ${WRITE_CONFIG}" >&2
  "${bootstrap_cmd[@]}" >&2
}

resolve_canary_account_id() {
  local config_path="$1"
  local values=()
  local line
  while IFS= read -r line; do
    values+=("$line")
  done < <(
    python3 - "$config_path" <<'PY'
import sys

try:
    import tomllib
except ModuleNotFoundError:
    import tomli as tomllib

KNOWN_PREFIXES = {
    "iroha3-taira": 369,
    "809574f5-fee7-5e69-bfcf-52451e42d50f": 369,
    "iroha3-nexus": 753,
    "00000000-0000-0000-0000-000000000753": 753,
}

with open(sys.argv[1], "rb") as handle:
    source = tomllib.load(handle)

account = source.get("account") or {}
public_key = account.get("public_key")
chain = source.get("chain")
chain_discriminant = account.get("chain_discriminant")

if not isinstance(public_key, str) or not public_key:
    raise SystemExit("write canary config is missing `account.public_key`")
if chain_discriminant is None:
    chain_discriminant = KNOWN_PREFIXES.get(chain)
if not isinstance(chain_discriminant, int):
    raise SystemExit(
        "write canary config must set `account.chain_discriminant` when `chain` is not a known Taira/Nexus chain id"
    )

print(public_key)
print(chain_discriminant)
PY
  )

  if [[ "${#values[@]}" -lt 2 ]]; then
    echo "could not derive the canary signer public key and chain discriminant from $config_path" >&2
    exit 1
  fi

  local public_key="${values[0]}"
  local chain_discriminant="${values[1]}"
  local output_file
  output_file="$(mktemp)"
  "${IROHA_RUNNER[@]}" tools address convert --network-prefix "$chain_discriminant" --format json "$public_key" \
    >"$output_file" 2>&1 || {
    sed -n '1,80p' "$output_file" >&2 || true
    rm -f "$output_file"
    exit 1
  }

  python3 - "$output_file" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8", errors="replace") as handle:
    payload = handle.read()

start = payload.find("{")
if start == -1:
    raise SystemExit("could not find JSON address-convert output while deriving the canary account id")

summary = json.loads(payload[start:])
account_id = summary.get("i105", {}).get("value")
if not isinstance(account_id, str) or not account_id:
    raise SystemExit("address-convert output did not include an i105 account id")
print(account_id)
PY
  rm -f "$output_file"
}

claim_faucet_for_canary() {
  local target_url="$1"
  local account_id="$2"
  echo "==> faucet bootstrap: ${account_id}" >&2
  python3 "${REPO_ROOT}/scripts/taira_faucet_canary.py" \
    --account-id "$account_id" \
    --torii-root "$target_url"
}

retry_write_canary() {
  local temp_config="$1"
  local output_file="$2"
  local write_msg="$3"
  local attempts="${4:-10}"
  local delay_seconds="${5:-2}"
  local attempt

  for ((attempt = 1; attempt <= attempts; attempt++)); do
    if "${IROHA_RUNNER[@]}" --machine -c "$temp_config" ledger transaction ping --msg "${write_msg}-retry-${attempt}" \
        >"$output_file" 2>&1; then
      return 0
    fi
    if ! grep -q 'Failed to find asset' "$output_file"; then
      return 1
    fi
    if [[ $attempt -lt $attempts ]]; then
      sleep "$delay_seconds"
    fi
  done
  return 1
}

run_write_canary() {
  local target_url="$1"
  local output_file temp_config write_msg

  ensure_iroha_bin
  [[ -n "$WRITE_CONFIG" ]] || WRITE_CONFIG="$(default_write_config_path)"
  ensure_write_canary_config "$target_url"

  temp_config="$(mktemp)"
  output_file="$(mktemp)"
  trap 'rm -f "${temp_config:-}" "${output_file:-}"; cleanup' EXIT
  build_write_canary_config \
    "$WRITE_CONFIG" \
    "$target_url" \
    "$temp_config" \
    "$ROLLOUT_CANARY_TIME_TO_LIVE_MS" \
    "$ROLLOUT_CANARY_STATUS_TIMEOUT_MS"

  write_msg="${WRITE_MESSAGE_PREFIX}-$(date -u +%Y%m%dT%H%M%SZ)"
  echo "==> write canary: ${target_url} (message: ${write_msg})"
  if ! "${IROHA_RUNNER[@]}" --machine -c "$temp_config" ledger transaction ping --msg "$write_msg" \
      >"$output_file" 2>&1; then
    if grep -q 'route_unavailable' "$output_file"; then
      echo "write canary failed: Torii is reachable but no authoritative peers accepted the lane route" >&2
      echo "hint: re-render every validator config from configs/soranexus/taira/validator_roster.example.toml using scripts/render_taira_validator_bundle.py and confirm the ingress node is running one of those generated configs with the full trusted_peers/trusted_peers_pop roster" >&2
      sed -n '1,80p' "$output_file" >&2 || true
      exit 1
    fi
    if grep -q 'Failed to find asset' "$output_file"; then
      local canary_account_id
      canary_account_id="$(resolve_canary_account_id "$temp_config" | tr -d '\r\n')"
      if ! claim_faucet_for_canary "$target_url" "$canary_account_id" >&2; then
        echo "write canary failed: canary signer is unfunded and the automatic faucet bootstrap did not succeed" >&2
        sed -n '1,80p' "$output_file" >&2 || true
        exit 1
      fi
      echo "==> retrying write canary after faucet bootstrap" >&2
      if ! retry_write_canary "$temp_config" "$output_file" "$write_msg"; then
        echo "write canary failed after faucet bootstrap" >&2
        sed -n '1,80p' "$output_file" >&2 || true
        exit 1
      fi
      rm -f "$temp_config" "$output_file"
      trap cleanup EXIT
      return 0
    fi
    if grep -q 'Transaction expired' "$output_file"; then
      echo "write canary failed: transaction expired; sampling public status before exiting" >&2
      print_status_route_diagnostics "$target_url"
      print_sumeragi_route_diagnostics "$target_url"
      sed -n '1,80p' "$output_file" >&2 || true
      exit 1
    fi
    if grep -Eq '(^|[^0-9])403([^0-9]|$)|Forbidden' "$output_file"; then
      echo "write canary failed: signer or permission check returned 403; re-check that the canary account still exists on Taira, still holds a fee asset balance, and still has the permissions required for the requested mutation" >&2
      print_status_route_diagnostics "$target_url"
      sed -n '1,80p' "$output_file" >&2 || true
      exit 1
    fi
    if classify_public_ingress_failure "$target_url"; then
      sed -n '1,80p' "$output_file" >&2 || true
      exit 1
    fi
    echo "write canary failed" >&2
    sed -n '1,80p' "$output_file" >&2 || true
    exit 1
  fi
  rm -f "$temp_config" "$output_file"
  trap cleanup EXIT
}

recheck_status_targets_after_write_canary() {
  local idx label root_url attempt

  for idx in "${!CHECKED_LABELS[@]}"; do
    label="${CHECKED_LABELS[$idx]}"
    root_url="${CHECKED_ROOTS[$idx]}"
    for ((attempt = 1; attempt <= 10; attempt++)); do
      if check_status_snapshot "$label" "${root_url}/status" 0; then
        break
      fi
      if [[ $attempt -eq 10 ]]; then
        echo "${label}: /status still did not publish a commit QC snapshot after the signed write canary" >&2
        exit 1
      fi
      sleep 2
    done
  done
}

if [[ $SKIP_LOCAL -eq 0 ]]; then
  check_endpoint "local" "$LOCAL_MCP_URL"
fi

if [[ $SKIP_PUBLIC -eq 0 ]]; then
  check_endpoint "public" "$PUBLIC_MCP_URL"
fi

if [[ -n "$WRITE_CONFIG" ]]; then
  run_write_canary "$(resolve_write_target_url)"
  recheck_status_targets_after_write_canary
elif [[ $SKIP_PUBLIC -eq 0 ]]; then
  echo "read-only checks passed; signed write canary was explicitly skipped" >&2
fi

echo "Taira MCP rollout checks passed."

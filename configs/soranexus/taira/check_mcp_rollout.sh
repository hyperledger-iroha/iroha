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
WRITE_CONFIG_EXPLICIT=0
if [[ -n "$WRITE_CONFIG" ]]; then
  WRITE_CONFIG_EXPLICIT=1
fi
WRITE_TARGET="${WRITE_TARGET:-}"
WRITE_MESSAGE_PREFIX="${WRITE_MESSAGE_PREFIX:-taira-rollout-canary}"
ROLLOUT_CANARY_ALIAS_PREFIX="${ROLLOUT_CANARY_ALIAS_PREFIX:-taira-rollout-canary}"
ROLLOUT_CANARY_TIME_TO_LIVE_MS="${ROLLOUT_CANARY_TIME_TO_LIVE_MS:-120000}"
ROLLOUT_CANARY_STATUS_TIMEOUT_MS="${ROLLOUT_CANARY_STATUS_TIMEOUT_MS:-120000}"
ROLLOUT_CANARY_GAS_ASSET_ID="${ROLLOUT_CANARY_GAS_ASSET_ID:-6TEAJqbb8oEPmLncoNiMRbLEK6tw}"
ROLLOUT_CANARY_SKIP_FAUCET="${ROLLOUT_CANARY_SKIP_FAUCET:-auto}"
POST_CANARY_STATUS_RECHECK_ATTEMPTS="${POST_CANARY_STATUS_RECHECK_ATTEMPTS:-10}"
POST_CANARY_STATUS_RECHECK_DELAY_SECONDS="${POST_CANARY_STATUS_RECHECK_DELAY_SECONDS:-2}"
MCP_ROLLOUT_CURL_CONNECT_TIMEOUT_SECONDS="${MCP_ROLLOUT_CURL_CONNECT_TIMEOUT_SECONDS:-5}"
MCP_ROLLOUT_CURL_MAX_TIME_SECONDS="${MCP_ROLLOUT_CURL_MAX_TIME_SECONDS:-20}"
MIN_VALIDATOR_SET_LEN="${MIN_VALIDATOR_SET_LEN:-4}"
VALIDATOR_PROGRESS_SAMPLES="${VALIDATOR_PROGRESS_SAMPLES:-3}"
VALIDATOR_PROGRESS_DELAY_SECONDS="${VALIDATOR_PROGRESS_DELAY_SECONDS:-2}"
VALIDATOR_ALIGNMENT_ATTEMPTS="${VALIDATOR_ALIGNMENT_ATTEMPTS:-10}"
EXPECTED_TAIRA_GIT_SHA="${EXPECTED_TAIRA_GIT_SHA:-}"
PUBLIC_LANE_ID="${PUBLIC_LANE_ID:-0}"
CONTRACT_NAMESPACE="${CONTRACT_NAMESPACE:-universal}"
SKIP_LOCAL=0
SKIP_PUBLIC=0
SKIP_WRITE_CANARY=0
REQUIRE_ALL_VALIDATORS=0
IROHA_RUNNER=()
CHECKED_LABELS=()
CHECKED_ROOTS=()
VALIDATOR_ROOT_SPECS=()
VALIDATOR_LABELS=()
VALIDATOR_ROOTS=()
VALIDATOR_ROOT_COUNT=0
CURL_RESOLVE_RULES=()
CURL_URL_RESOLVE_ARGS=()

usage() {
  cat <<'EOF'
Usage: check_mcp_rollout.sh [--local-root URL] [--public-root URL] [--local-url URL] [--public-url URL]
                            [--skip-local] [--skip-public]
                            [--validator-root LABEL=URL]... [--require-all-validators]
                            [--write-config PATH] [--write-target local|public|URL]
                            [--gas-asset-id ASSET_DEFINITION_ID]
                            [--iroha-bin PATH] [--resolve-host HOST:IP|HOST:PORT:IP]
                            [--curl-connect-timeout-seconds N]
                            [--curl-max-time-seconds N]
                            [--expected-git-sha 7_TO_40_HEX_SHA] [--skip-write-canary]

Verify that Taira's native Torii MCP endpoint is live locally and/or publicly.
For a single public-node devex check, prefer the first-class CLI:
  iroha taira doctor --public-root https://taira.sora.org --output-format text
  iroha taira write-canary --public-root https://taira.sora.org --output-format text

The check fails unless:
  - GET /v1/mcp returns HTTP 200 with a capabilities payload
  - POST /v1/mcp initialize returns HTTP 200
  - POST /v1/mcp notifications/initialized returns HTTP 202 with an empty body
  - POST /v1/mcp tools/list returns HTTP 200
  - the tool list includes curated iroha.* names, including write-ready aliases
  - every advertised MCP tool publishes an OpenAI-compatible top-level
    `inputSchema` object (no top-level anyOf/oneOf/allOf/enum/not)
  - the tool list does not expose raw torii.* names
  - GET /status returns Torii counters
  - when `--expected-git-sha` is supplied, GET /status reports a matching
    `build.git_commit_sha` (published and expected values must be 7 to 40
    hexadecimal characters; short or full prefix matches are accepted)
  - GET /v1/sumeragi/status reports protocol v2 durable reducer state
  - GET /v1/pipeline/transactions/status reaches the canonical typed status
    handler (the no-hash probe returns HTTP 400), while the retired
    /v1/transactions/status alias remains unmounted (HTTP 404)
  - when validator roots are supplied, every labeled validator reports the same
    protocol/build/config/context/commit tuple across repeated advancing samples
  - direct public Torii ingress also exposes SCCP, ZK, bridge, validator-set,
    public-lane, contract, and Musubi routes on the same node URL

When diagnosing public write failures, prefer `/status` fields such as
`blocks`, `queue_size`, `tx_queue.depth`, `tx_queue.capacity`,
`tx_queue.saturated_by_count`, `tx_queue.saturated_by_age`,
`tx_queue.oldest_queued_age_ms`, `view_change_causes.last_cause`, and
`teu_dataspace_backlog`, together with `/v1/sumeragi/status` v2 fields such as
`protocol_version`, `height_context_id`, `height`, `view`, `leader`,
`locked_prepare_qc`, `highest_prepare_qc`, `last_timeout_certificate`,
`body_state`, `pending_persistence_id`, `last_committed_height`, and
`last_committed_subject`. Do not use `/status.peers` as validator-set size; it
is the queried node's current remote-peer count.

For final public rollout, use a runtime-only canary signer config. When
`--write-config` is omitted, the script bootstraps a runtime-only canary config
automatically, preferring `/run/secrets/taira-canary-client.toml` when that
directory is writable and otherwise falling back to `${TMPDIR:-/tmp}`. It
reuses an existing config at the automatic path. An explicit `--write-config`
must already exist and is read without modification; the script never
overwrites operator-supplied signing material. Automatic bootstrap posts the
current universal-account DTO to `/v1/accounts/onboard`, requires `HTTP 202`
with a `QUEUED` receipt, and follows that receipt through
`/v1/pipeline/transactions/status` before using the signer. Onboarding fees are
sponsored by the configured Torii onboarding authority; gas fields are not part
of the onboarding request. The write canary attaches Taira's accepted XOR gas
asset metadata by default and still retries the faucet lane on `Failed to find
asset` so a saturated queue does not require manual signer preparation. Set
`ROLLOUT_CANARY_SKIP_FAUCET=0` to require an initial faucet claim. Use
`--gas-asset-id ""` only against networks that do not require pipeline gas
metadata. Both onboarding and faucet helpers wait for their `202 QUEUED`
receipts to reach `Applied` or `Committed` through the canonical pipeline
status route. Use `--skip-write-canary` only for read-only validation.

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

should_skip_canary_faucet() {
  case "$ROLLOUT_CANARY_SKIP_FAUCET" in
    auto|"")
      [[ -n "$ROLLOUT_CANARY_GAS_ASSET_ID" ]]
      ;;
    1|true|TRUE|yes|YES)
      return 0
      ;;
    0|false|FALSE|no|NO)
      return 1
      ;;
    *)
      echo "ROLLOUT_CANARY_SKIP_FAUCET must be auto, 1, 0, true, false, yes, or no" >&2
      exit 1
      ;;
  esac
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
    --validator-root)
      [[ $# -ge 2 ]] || {
        echo "missing value for --validator-root" >&2
        exit 1
      }
      VALIDATOR_ROOT_SPECS+=("$2")
      shift 2
      ;;
    --require-all-validators)
      REQUIRE_ALL_VALIDATORS=1
      shift
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
      WRITE_CONFIG_EXPLICIT=1
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
    --gas-asset-id)
      [[ $# -ge 2 ]] || {
        echo "missing value for --gas-asset-id" >&2
        exit 1
      }
      ROLLOUT_CANARY_GAS_ASSET_ID="$2"
      shift 2
      ;;
    --expected-git-sha)
      [[ $# -ge 2 ]] || {
        echo "missing value for --expected-git-sha" >&2
        exit 1
      }
      EXPECTED_TAIRA_GIT_SHA="$2"
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
    --curl-connect-timeout-seconds)
      [[ $# -ge 2 ]] || {
        echo "missing value for --curl-connect-timeout-seconds" >&2
        exit 1
      }
      MCP_ROLLOUT_CURL_CONNECT_TIMEOUT_SECONDS="$2"
      shift 2
      ;;
    --curl-max-time-seconds)
      [[ $# -ge 2 ]] || {
        echo "missing value for --curl-max-time-seconds" >&2
        exit 1
      }
      MCP_ROLLOUT_CURL_MAX_TIME_SECONDS="$2"
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

if [[ -n "$EXPECTED_TAIRA_GIT_SHA" ]]; then
  if [[ ! "$EXPECTED_TAIRA_GIT_SHA" =~ ^[0-9A-Fa-f]{7,40}$ ]]; then
    echo "--expected-git-sha must be a 7 to 40 character hexadecimal git SHA prefix" >&2
    exit 1
  fi
  EXPECTED_TAIRA_GIT_SHA="$(printf '%s' "$EXPECTED_TAIRA_GIT_SHA" | tr 'A-F' 'a-f')"
fi

if [[ -n "$WRITE_CONFIG" && $SKIP_WRITE_CANARY -eq 1 ]]; then
  echo "--write-config and --skip-write-canary are mutually exclusive" >&2
  exit 1
fi

require_positive_integer() {
  local name="$1"
  local value="$2"

  if [[ ! "$value" =~ ^[0-9]+$ || "$value" == "0" ]]; then
    echo "${name} must be a positive integer" >&2
    exit 1
  fi
}

require_nonnegative_integer() {
  local name="$1"
  local value="$2"

  if [[ ! "$value" =~ ^[0-9]+$ ]]; then
    echo "${name} must be a non-negative integer" >&2
    exit 1
  fi
}

validate_numeric_inputs() {
  require_positive_integer \
    "ROLLOUT_CANARY_TIME_TO_LIVE_MS" \
    "$ROLLOUT_CANARY_TIME_TO_LIVE_MS"
  require_positive_integer \
    "ROLLOUT_CANARY_STATUS_TIMEOUT_MS" \
    "$ROLLOUT_CANARY_STATUS_TIMEOUT_MS"
  require_positive_integer \
    "POST_CANARY_STATUS_RECHECK_ATTEMPTS" \
    "$POST_CANARY_STATUS_RECHECK_ATTEMPTS"
  require_nonnegative_integer \
    "POST_CANARY_STATUS_RECHECK_DELAY_SECONDS" \
    "$POST_CANARY_STATUS_RECHECK_DELAY_SECONDS"
  require_positive_integer \
    "MIN_VALIDATOR_SET_LEN" \
    "$MIN_VALIDATOR_SET_LEN"
  require_positive_integer \
    "VALIDATOR_PROGRESS_SAMPLES" \
    "$VALIDATOR_PROGRESS_SAMPLES"
  require_nonnegative_integer \
    "VALIDATOR_PROGRESS_DELAY_SECONDS" \
    "$VALIDATOR_PROGRESS_DELAY_SECONDS"
  require_positive_integer \
    "VALIDATOR_ALIGNMENT_ATTEMPTS" \
    "$VALIDATOR_ALIGNMENT_ATTEMPTS"
  require_nonnegative_integer \
    "PUBLIC_LANE_ID" \
    "$PUBLIC_LANE_ID"
  require_positive_integer \
    "MCP_ROLLOUT_CURL_CONNECT_TIMEOUT_SECONDS" \
    "$MCP_ROLLOUT_CURL_CONNECT_TIMEOUT_SECONDS"
  require_positive_integer \
    "MCP_ROLLOUT_CURL_MAX_TIME_SECONDS" \
    "$MCP_ROLLOUT_CURL_MAX_TIME_SECONDS"
}

validate_numeric_inputs

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

parse_validator_roots() {
  local spec label root existing

  for spec in "${VALIDATOR_ROOT_SPECS[@]+"${VALIDATOR_ROOT_SPECS[@]}"}"; do
    if [[ "$spec" != *=* ]]; then
      echo "--validator-root must use LABEL=URL syntax: ${spec}" >&2
      exit 1
    fi
    label="${spec%%=*}"
    root="${spec#*=}"
    if [[ -z "$label" || -z "$root" ]]; then
      echo "--validator-root requires a non-empty label and URL: ${spec}" >&2
      exit 1
    fi
    if [[ ! "$label" =~ ^[A-Za-z0-9._-]+$ ]]; then
      echo "validator label contains unsupported characters: ${label}" >&2
      exit 1
    fi
    root="$(normalize_root_url "$root")"
    if [[ ! "$root" =~ ^https?:// ]]; then
      echo "validator root must be an http(s) URL: ${root}" >&2
      exit 1
    fi
    for existing in "${VALIDATOR_LABELS[@]+"${VALIDATOR_LABELS[@]}"}"; do
      if [[ "$existing" == "$label" ]]; then
        echo "duplicate validator label: ${label}" >&2
        exit 1
      fi
    done
    for existing in "${VALIDATOR_ROOTS[@]+"${VALIDATOR_ROOTS[@]}"}"; do
      if [[ "$existing" == "$root" ]]; then
        echo "duplicate validator root: ${root}" >&2
        exit 1
      fi
    done
    VALIDATOR_LABELS+=("$label")
    VALIDATOR_ROOTS+=("$root")
    VALIDATOR_ROOT_COUNT=$((VALIDATOR_ROOT_COUNT + 1))
  done

  if [[ $REQUIRE_ALL_VALIDATORS -eq 1 && $VALIDATOR_ROOT_COUNT -eq 0 ]]; then
    echo "--require-all-validators requires repeated --validator-root LABEL=URL arguments" >&2
    exit 1
  fi
  if [[ $VALIDATOR_ROOT_COUNT -gt 0 && $VALIDATOR_ROOT_COUNT -lt $MIN_VALIDATOR_SET_LEN ]]; then
    echo "validator fleet check requires at least ${MIN_VALIDATOR_SET_LEN} distinct labeled roots; received ${VALIDATOR_ROOT_COUNT}" >&2
    exit 1
  fi
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

parse_validator_roots

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
  local body_file header_file error_file
  local curl_output curl_rc
  # The first-release /v1 API has no version-negotiation request header.
  local curl_cmd=(
    curl
    --silent
    --show-error
    -H
    "accept: application/json"
    --connect-timeout
    "$MCP_ROLLOUT_CURL_CONNECT_TIMEOUT_SECONDS"
    --max-time
    "$MCP_ROLLOUT_CURL_MAX_TIME_SECONDS"
  )

  body_file="$(mktemp)"
  header_file="$(mktemp)"
  error_file="$(mktemp)"
  cleanup
  last_body="$body_file"
  last_headers="$header_file"
  build_curl_resolve_args "$url"
  curl_cmd+=( ${CURL_URL_RESOLVE_ARGS[@]+"${CURL_URL_RESOLVE_ARGS[@]}"} )

  if [[ "$method" == "GET" ]]; then
    set +e
    curl_output="$(
      "${curl_cmd[@]}" \
      --output "$body_file" \
      --dump-header "$header_file" \
      --write-out "%{http_code}" \
      "$url" \
      2>"$error_file"
    )"
    curl_rc=$?
    set -e
  else
    set +e
    curl_output="$(
      "${curl_cmd[@]}" \
      --output "$body_file" \
      --dump-header "$header_file" \
      --write-out "%{http_code}" \
      -X POST \
      -H "content-type: application/json" \
      --data "$payload" \
      "$url" \
      2>"$error_file"
    )"
    curl_rc=$?
    set -e
  fi

  if [[ $curl_rc -ne 0 ]]; then
    last_status="curl_error_${curl_rc}"
    {
      printf 'curl exited with status %s\n' "$curl_rc"
      sed -n '1,40p' "$error_file" || true
    } >"$header_file"
    rm -f "$error_file"
    return 0
  fi

  rm -f "$error_file"
  last_status="$curl_output"
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
import re
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)

teu_backlog = None
backlog_entries = payload.get("teu_dataspace_backlog")
if isinstance(backlog_entries, list) and backlog_entries:
    first = backlog_entries[0]
    if isinstance(first, dict):
        teu_backlog = first.get("backlog")

summary = {
    "blocks": payload.get("blocks"),
    "queue_size": payload.get("queue_size"),
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

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)

status = payload.get("v2") if isinstance(payload.get("v2"), dict) else payload
summary = {name: status.get(name) for name in (
    "protocol_version",
    "height_context_id",
    "height",
    "view",
    "phase",
    "leader",
    "last_committed_height",
    "last_committed_subject",
    "body_state",
    "pending_persistence_id",
)}
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
  local expected_error_code="${7:-}"
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
  if [[ -n "$expected_error_code" ]]; then
    python3 - "$label" "$description" "$expected_error_code" "$last_body" <<'PY'
import json
import sys

label, description, expected_code, body_path = sys.argv[1:]
try:
    with open(body_path, "r", encoding="utf-8") as handle:
        payload = json.load(handle)
except (OSError, json.JSONDecodeError) as error:
    raise SystemExit(
        f"{label}: {description} did not return a typed JSON error envelope: {error}"
    ) from error
actual_code = payload.get("code") if isinstance(payload, dict) else None
if actual_code != expected_code:
    raise SystemExit(
        f"{label}: {description} returned error code {actual_code!r}; "
        f"expected {expected_code!r}"
    )
PY
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
  python3 - "$label" "$last_body" "$MIN_VALIDATOR_SET_LEN" "$allow_pending_commit_qc" "$EXPECTED_TAIRA_GIT_SHA" <<'PY'
import json
import re
import sys

label = sys.argv[1]
path = sys.argv[2]
expected_git_sha = sys.argv[5].strip()
with open(path, "r", encoding="utf-8") as handle:
    payload = json.load(handle)

blocks = payload.get("blocks")
queue_size = payload.get("queue_size")
if blocks is not None and (not isinstance(blocks, int) or blocks < 0):
    print(f"{label}: /status reported an invalid block counter: {blocks!r}", file=sys.stderr)
    sys.exit(1)
if queue_size is not None and (not isinstance(queue_size, int) or queue_size < 0):
    print(f"{label}: /status reported an invalid queue size: {queue_size!r}", file=sys.stderr)
    sys.exit(1)
build = payload.get("build")
build_git_sha = None
if isinstance(build, dict):
    for key in ("git_commit_sha", "git_sha", "commit_sha", "commit"):
        value = build.get(key)
        if isinstance(value, str) and value.strip():
            build_git_sha = value.strip().lower()
            break
if expected_git_sha:
    if build_git_sha is None:
        print(
            f"{label}: /status did not publish build.git_commit_sha; "
            f"expected Taira git SHA {expected_git_sha}",
            file=sys.stderr,
        )
        sys.exit(1)
    if re.fullmatch(r"[0-9a-f]{7,40}", build_git_sha) is None:
        print(
            f"{label}: /status build git SHA {build_git_sha} is not a "
            "7 to 40 character hexadecimal SHA prefix; "
            f"expected {expected_git_sha}",
            file=sys.stderr,
        )
        sys.exit(1)
    if not (
        build_git_sha.startswith(expected_git_sha)
        or expected_git_sha.startswith(build_git_sha)
    ):
        print(
            f"{label}: /status build git SHA {build_git_sha} does not match "
            f"expected {expected_git_sha}",
            file=sys.stderr,
        )
        sys.exit(1)
PY
}

check_sumeragi_snapshot() {
  local label="$1"
  local sumeragi_url="$2"
  local _allow_pending_commit_qc="${3:-0}"

  echo "==> ${label}: GET ${sumeragi_url}"
  http_request GET "$sumeragi_url"
  if [[ "$last_status" != "200" ]]; then
    echo "${label}: /v1/sumeragi/status failed with HTTP ${last_status}" >&2
    sed -n '1,20p' "$last_headers" >&2 || true
    exit 1
  fi
  python3 - "$label" "$last_body" <<'PY'
import json
import sys

label, path = sys.argv[1:]
with open(path, "r", encoding="utf-8") as handle:
    envelope = json.load(handle)

status = envelope.get("v2") if isinstance(envelope.get("v2"), dict) else envelope
if status.get("protocol_version") != 2:
    raise SystemExit(
        f"{label}: expected the Sumeragi v2 reducer status; "
        "legacy RBC/recovery status is not accepted for Taira rollout"
    )

required = (
    "node_fingerprint",
    "build_fingerprint",
    "config_fingerprint",
    "height_context_id",
    "phase",
    "body_state",
)
missing = [name for name in required if status.get(name) in (None, "", {})]
if missing:
    raise SystemExit(
        f"{label}: v2 status omitted required field(s): {', '.join(missing)}"
    )

def tagged_unit(value, tag, admitted, field):
    if not isinstance(value, dict) or set(value) != {tag, "details"}:
        raise SystemExit(f"{label}: v2 status {field} is not a canonical tagged unit")
    if value["details"] is not None or value[tag] not in admitted:
        raise SystemExit(f"{label}: v2 status {field} has an invalid variant/details payload")

tagged_unit(
    status["phase"],
    "phase",
    {
        "AwaitingProposal",
        "ReconstructingPayload",
        "ValidatingPayload",
        "Prepare",
        "Commit",
        "PendingApply",
    },
    "phase",
)
tagged_unit(
    status["body_state"],
    "state",
    {"Missing", "Reconstructing", "Stored", "Validated", "PendingApply", "Applied"},
    "body_state",
)

for name in ("height", "view", "leader", "last_committed_height"):
    value = status.get(name)
    if not isinstance(value, int) or isinstance(value, bool) or value < 0:
        raise SystemExit(f"{label}: v2 status reported invalid {name}: {value!r}")

if status["last_committed_height"] > status["height"]:
    raise SystemExit(
        f"{label}: committed height {status['last_committed_height']} "
        f"is ahead of reducer height {status['height']}"
    )
if status["last_committed_height"] > 0 and status.get("last_committed_subject") is None:
    raise SystemExit(
        f"{label}: v2 status omitted last_committed_subject for a non-genesis commit"
    )

pending = status.get("pending_persistence_id")
if pending is not None and (
    not isinstance(pending, int) or isinstance(pending, bool) or pending < 1
):
    raise SystemExit(
        f"{label}: v2 status reported invalid pending persistence id: {pending!r}"
    )
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
    else
      rc=$?
    fi
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

check_sumeragi_snapshot_with_retry() {
  local label="$1"
  local sumeragi_url="$2"
  local allow_pending_commit_qc="${3:-0}"
  local attempts="${4:-10}"
  local delay_seconds="${5:-2}"
  local attempt rc

  for ((attempt = 1; attempt <= attempts; attempt++)); do
    if check_sumeragi_snapshot "$label" "$sumeragi_url" "$allow_pending_commit_qc"; then
      return 0
    else
      rc=$?
    fi
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

capture_validator_fleet_sample() {
  local records_file status_copy
  local idx label root
  records_file="$(mktemp)"

  for idx in "${!VALIDATOR_ROOTS[@]}"; do
    label="${VALIDATOR_LABELS[$idx]}"
    root="${VALIDATOR_ROOTS[$idx]}"
    echo "==> validator ${label}: GET ${root}/status" >&2
    http_request GET "${root}/status"
    if [[ "$last_status" != "200" ]]; then
      echo "validator ${label}: /status failed with HTTP ${last_status}" >&2
      rm -f "$records_file"
      return 1
    fi
    status_copy="$(mktemp)"
    cp "$last_body" "$status_copy"

    echo "==> validator ${label}: GET ${root}/v1/sumeragi/status" >&2
    http_request GET "${root}/v1/sumeragi/status"
    if [[ "$last_status" != "200" ]]; then
      echo "validator ${label}: /v1/sumeragi/status failed with HTTP ${last_status}" >&2
      rm -f "$status_copy" "$records_file"
      return 1
    fi

    if ! python3 - "$label" "$status_copy" "$last_body" "$EXPECTED_TAIRA_GIT_SHA" >>"$records_file" <<'PY'
import json
import re
import sys

label, status_path, sumeragi_path, expected_sha = sys.argv[1:]
with open(status_path, "r", encoding="utf-8") as handle:
    node_status = json.load(handle)
with open(sumeragi_path, "r", encoding="utf-8") as handle:
    envelope = json.load(handle)

status = envelope.get("v2") if isinstance(envelope.get("v2"), dict) else envelope
if status.get("protocol_version") != 2:
    raise SystemExit(f"validator {label}: expected Sumeragi protocol_version 2")

required = (
    "node_fingerprint",
    "build_fingerprint",
    "config_fingerprint",
    "height_context_id",
    "height",
    "view",
    "phase",
    "leader",
    "body_state",
    "last_committed_height",
)
missing = [name for name in required if status.get(name) in (None, "", {})]
if missing:
    raise SystemExit(
        f"validator {label}: v2 status omitted required fields: {', '.join(missing)}"
    )
for name in ("height", "view", "leader", "last_committed_height"):
    value = status[name]
    if not isinstance(value, int) or isinstance(value, bool) or value < 0:
        raise SystemExit(f"validator {label}: invalid {name}: {value!r}")

def tagged_unit(value, tag, admitted, field):
    if not isinstance(value, dict) or set(value) != {tag, "details"}:
        raise SystemExit(f"validator {label}: {field} is not a canonical tagged unit")
    if value["details"] is not None or value[tag] not in admitted:
        raise SystemExit(f"validator {label}: {field} has an invalid variant/details payload")

tagged_unit(
    status["phase"],
    "phase",
    {
        "AwaitingProposal",
        "ReconstructingPayload",
        "ValidatingPayload",
        "Prepare",
        "Commit",
        "PendingApply",
    },
    "phase",
)
tagged_unit(
    status["body_state"],
    "state",
    {"Missing", "Reconstructing", "Stored", "Validated", "PendingApply", "Applied"},
    "body_state",
)
if status["last_committed_height"] > 0 and status.get("last_committed_subject") is None:
    raise SystemExit(f"validator {label}: missing committed subject/hash")

if expected_sha:
    build = node_status.get("build") or {}
    published = next(
        (
            build.get(key).strip().lower()
            for key in ("git_commit_sha", "git_sha", "commit_sha", "commit")
            if isinstance(build.get(key), str) and build.get(key).strip()
        ),
        None,
    )
    if published is None or re.fullmatch(r"[0-9a-f]{7,40}", published) is None:
        raise SystemExit(f"validator {label}: /status omitted a valid build git SHA")
    if not (published.startswith(expected_sha) or expected_sha.startswith(published)):
        raise SystemExit(
            f"validator {label}: build git SHA {published} does not match {expected_sha}"
        )

def canonical(value):
    return json.dumps(value, ensure_ascii=True, sort_keys=True, separators=(",", ":"))

record = {
    "label": label,
    "node": canonical(status["node_fingerprint"]),
    "build": canonical(status["build_fingerprint"]),
    "config": canonical(status["config_fingerprint"]),
    "context": canonical(status["height_context_id"]),
    "height": status["height"],
    "view": status["view"],
    "committed_height": status["last_committed_height"],
    "committed_subject": canonical(status.get("last_committed_subject")),
}
print(json.dumps(record, ensure_ascii=True, sort_keys=True))
PY
    then
      rm -f "$status_copy" "$records_file"
      return 1
    fi
    rm -f "$status_copy"
  done

  python3 - "$records_file" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    records = [json.loads(line) for line in handle if line.strip()]
if not records:
    raise SystemExit("validator fleet sample contained no records")

nodes = [record["node"] for record in records]
if len(nodes) != len(set(nodes)):
    raise SystemExit(
        "validator roots do not identify distinct nodes; check tunnels and ingress routing"
    )

baseline = records[0]
for record in records[1:]:
    for field in (
        "build",
        "config",
        "context",
        "height",
        "committed_height",
        "committed_subject",
    ):
        if record[field] != baseline[field]:
            raise SystemExit(
                f"validator {record['label']} disagrees with {baseline['label']} on {field}: "
                f"{record[field]!r} != {baseline[field]!r}"
            )

summary = {
    "build": baseline["build"],
    "config": baseline["config"],
    "context": baseline["context"],
    "height": baseline["height"],
    "committed_height": baseline["committed_height"],
    "committed_subject": baseline["committed_subject"],
    "nodes": sorted(nodes),
}
print(json.dumps(summary, ensure_ascii=True, sort_keys=True))
PY
  local rc=$?
  rm -f "$records_file"
  return "$rc"
}

check_validator_fleet() {
  [[ ${#VALIDATOR_ROOTS[@]} -gt 0 ]] || return 0

  local sample attempt summary previous_summary="" aligned=0
  for ((sample = 1; sample <= VALIDATOR_PROGRESS_SAMPLES; sample++)); do
    aligned=0
    for ((attempt = 1; attempt <= VALIDATOR_ALIGNMENT_ATTEMPTS; attempt++)); do
      if summary="$(capture_validator_fleet_sample)"; then
        aligned=1
        break
      fi
      if [[ $attempt -lt $VALIDATOR_ALIGNMENT_ATTEMPTS ]]; then
        sleep 1
      fi
    done
    if [[ $aligned -ne 1 ]]; then
      echo "validator fleet did not converge on one build/config/context/commit tuple after ${VALIDATOR_ALIGNMENT_ATTEMPTS} attempts" >&2
      exit 1
    fi

    if [[ -n "$previous_summary" ]]; then
      python3 - "$previous_summary" "$summary" <<'PY'
import json
import sys

previous = json.loads(sys.argv[1])
current = json.loads(sys.argv[2])
for field in ("build", "config", "nodes"):
    if current[field] != previous[field]:
        raise SystemExit(f"validator fleet changed {field} between progress samples")
if current["committed_height"] <= previous["committed_height"]:
    raise SystemExit(
        "validator fleet did not advance a common committed height: "
        f"{current['committed_height']} <= {previous['committed_height']}"
    )
if current["committed_subject"] == previous["committed_subject"]:
    raise SystemExit("validator fleet advanced height without changing the common block hash")
PY
    fi
    echo "validator fleet sample ${sample}/${VALIDATOR_PROGRESS_SAMPLES}: ${summary}"
    previous_summary="$summary"
    if [[ $sample -lt $VALIDATOR_PROGRESS_SAMPLES ]]; then
      sleep "$VALIDATOR_PROGRESS_DELAY_SECONDS"
    fi
  done
}

check_route_parity() {
  local label="$1"
  local root_url="$2"
  local lane_id="$3"
  local namespace="$4"

  root_url="$(normalize_root_url "$root_url")"
  check_route_status "$label" GET "${root_url}/v1/sccp/capabilities" "200" \
    "SCCP capability discovery route"
  check_route_status "$label" GET "${root_url}/v1/sccp/registry" "200" \
    "SCCP typed registry discovery route"
  check_route_status "$label" GET "${root_url}/v1/zk/proofs/count" "200" \
    "ZK proof count route"
  check_route_status "$label" GET "${root_url}/v1/sumeragi/validator-sets" "200" \
    "validator-set snapshot route"
  check_route_status "$label" GET "${root_url}/v1/nexus/public-lanes/${lane_id}/validators" "200" \
    "public-lane validator snapshot route"
  check_route_status "$label" GET "${root_url}/v1/nexus/public-lanes/${lane_id}/stake" "200" \
    "public-lane stake snapshot route"
  check_route_status "$label" GET "${root_url}/v1/contracts/state" "400" \
    "contract state route should be mounted and reject missing query selectors"
  check_route_status "$label" GET "${root_url}/v1/pipeline/transactions/status" "400" \
    "canonical pipeline transaction-status route should reject a missing hash" \
    "" "query_validation_failed"
  check_route_status "$label" GET "${root_url}/v1/transactions/status" "404" \
    "retired transaction-status compatibility route must remain unmounted" \
    "" "route_not_found"
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
  if check_sumeragi_snapshot_with_retry "$label" "${root_url}/v1/sumeragi/status" "$allow_pending_commit_qc"; then
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

PUBLIC_TAIRA_CHAIN_ID = "fc56984b-2be7-431d-840e-21514d1883f0"
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
if chain != PUBLIC_TAIRA_CHAIN_ID:
    raise SystemExit(
        "write canary config must target the public Sumeragi-v2 Taira chain "
        f"`{PUBLIC_TAIRA_CHAIN_ID}`"
    )
if not isinstance(public_key, str) or not public_key:
    raise SystemExit("write canary config is missing `account.public_key`")
if not isinstance(private_key, str) or not private_key:
    raise SystemExit("write canary config is missing `account.private_key`")
if chain_discriminant is not None and not isinstance(chain_discriminant, int):
    raise SystemExit("write canary config `account.chain_discriminant` must be an integer")
if chain_discriminant is not None and chain_discriminant != 369:
    raise SystemExit("write canary config must use Taira chain discriminant 369")
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

  if [[ -f "$WRITE_CONFIG" ]]; then
    return 0
  fi
  if [[ $WRITE_CONFIG_EXPLICIT -eq 1 ]]; then
    echo "write canary config not found: ${WRITE_CONFIG}" >&2
    exit 1
  fi

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
  if [[ -n "$ROLLOUT_CANARY_GAS_ASSET_ID" ]]; then
    bootstrap_cmd+=(--gas-asset-id "$ROLLOUT_CANARY_GAS_ASSET_ID")
  fi
  if should_skip_canary_faucet; then
    bootstrap_cmd+=(--skip-faucet)
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

PUBLIC_TAIRA_CHAIN_ID = "fc56984b-2be7-431d-840e-21514d1883f0"

with open(sys.argv[1], "rb") as handle:
    source = tomllib.load(handle)

account = source.get("account") or {}
public_key = account.get("public_key")
chain = source.get("chain")
chain_discriminant = account.get("chain_discriminant")

if not isinstance(public_key, str) or not public_key:
    raise SystemExit("write canary config is missing `account.public_key`")
if chain != PUBLIC_TAIRA_CHAIN_ID:
    raise SystemExit(
        "write canary config must target the public Sumeragi-v2 Taira chain "
        f"`{PUBLIC_TAIRA_CHAIN_ID}`"
    )
if chain_discriminant is None:
    chain_discriminant = 369
if not isinstance(chain_discriminant, int):
    raise SystemExit("write canary config `account.chain_discriminant` must be an integer")
if chain_discriminant != 369:
    raise SystemExit("write canary config must use Taira chain discriminant 369")

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
    --torii-root "$target_url" \
    --status-timeout-ms "$ROLLOUT_CANARY_STATUS_TIMEOUT_MS"
}

write_canary_metadata_file() {
  local output_file="$1"
  local gas_asset_id="$2"
  python3 - "$output_file" "$gas_asset_id" <<'PY'
import json
import sys

path, gas_asset_id = sys.argv[1:]
with open(path, "w", encoding="utf-8") as handle:
    json.dump({"gas_asset_id": gas_asset_id}, handle, sort_keys=True)
    handle.write("\n")
PY
}

retry_write_canary() {
  local temp_config="$1"
  local output_file="$2"
  local write_msg="$3"
  local metadata_file="$4"
  local attempts="${5:-10}"
  local delay_seconds="${6:-2}"
  local attempt
  local metadata_args=()

  if [[ -n "$metadata_file" ]]; then
    metadata_args=(-m "$metadata_file")
  fi

  for ((attempt = 1; attempt <= attempts; attempt++)); do
    if "${IROHA_RUNNER[@]}" --machine -c "$temp_config" "${metadata_args[@]}" ledger transaction ping --msg "${write_msg}-retry-${attempt}" \
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
  local output_file temp_config metadata_file write_msg

  ensure_iroha_bin
  [[ -n "$WRITE_CONFIG" ]] || WRITE_CONFIG="$(default_write_config_path)"
  ensure_write_canary_config "$target_url"

  temp_config="$(mktemp)"
  metadata_file="$(mktemp)"
  output_file="$(mktemp)"
  trap 'rm -f "${temp_config:-}" "${metadata_file:-}" "${output_file:-}"; cleanup' EXIT
  build_write_canary_config \
    "$WRITE_CONFIG" \
    "$target_url" \
    "$temp_config" \
    "$ROLLOUT_CANARY_TIME_TO_LIVE_MS" \
    "$ROLLOUT_CANARY_STATUS_TIMEOUT_MS"
  local metadata_args=()
  if [[ -n "$ROLLOUT_CANARY_GAS_ASSET_ID" ]]; then
    write_canary_metadata_file "$metadata_file" "$ROLLOUT_CANARY_GAS_ASSET_ID"
    metadata_args=(-m "$metadata_file")
  else
    rm -f "$metadata_file"
    metadata_file=""
  fi

  write_msg="${WRITE_MESSAGE_PREFIX}-$(date -u +%Y%m%dT%H%M%SZ)"
  echo "==> write canary: ${target_url} (message: ${write_msg})"
  if ! "${IROHA_RUNNER[@]}" --machine -c "$temp_config" "${metadata_args[@]}" ledger transaction ping --msg "$write_msg" \
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
      if ! retry_write_canary "$temp_config" "$output_file" "$write_msg" "$metadata_file"; then
        echo "write canary failed after faucet bootstrap" >&2
        sed -n '1,80p' "$output_file" >&2 || true
        exit 1
      fi
      rm -f "$temp_config" "$metadata_file" "$output_file"
      trap cleanup EXIT
      return 0
    fi
    if grep -q 'missing gas_asset_id' "$output_file"; then
      echo "write canary failed: Taira requires gas_asset_id transaction metadata; pass --gas-asset-id with an accepted asset definition id" >&2
      sed -n '1,80p' "$output_file" >&2 || true
      exit 1
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
  rm -f "$temp_config" "$metadata_file" "$output_file"
  trap cleanup EXIT
}

recheck_status_targets_after_write_canary() {
  local idx label root_url

  for idx in "${!CHECKED_LABELS[@]}"; do
    label="${CHECKED_LABELS[$idx]}"
    root_url="${CHECKED_ROOTS[$idx]}"
    if ! check_status_snapshot_with_retry \
      "$label" \
      "${root_url}/status" \
      0 \
      "$POST_CANARY_STATUS_RECHECK_ATTEMPTS" \
      "$POST_CANARY_STATUS_RECHECK_DELAY_SECONDS"; then
      echo "${label}: /status still did not publish a healthy snapshot after the signed write canary" >&2
      exit 1
    fi
    if ! check_sumeragi_snapshot_with_retry \
      "$label" \
      "${root_url}/v1/sumeragi/status" \
      0 \
      "$POST_CANARY_STATUS_RECHECK_ATTEMPTS" \
      "$POST_CANARY_STATUS_RECHECK_DELAY_SECONDS"; then
      echo "${label}: /v1/sumeragi/status still did not publish a healthy commit QC snapshot after the signed write canary" >&2
      exit 1
    fi
  done
}

check_validator_fleet

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

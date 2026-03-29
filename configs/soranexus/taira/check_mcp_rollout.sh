#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/../../.." && pwd)"
LOCAL_MCP_URL="${LOCAL_MCP_URL:-http://127.0.0.1:18080/v1/mcp}"
PUBLIC_MCP_URL="${PUBLIC_MCP_URL:-https://taira.sora.org/v1/mcp}"
IROHA_BIN="${IROHA_BIN:-}"
WRITE_CONFIG="${WRITE_CONFIG:-}"
WRITE_TARGET="${WRITE_TARGET:-}"
WRITE_MESSAGE_PREFIX="${WRITE_MESSAGE_PREFIX:-taira-rollout-canary}"
MIN_VALIDATOR_SET_LEN="${MIN_VALIDATOR_SET_LEN:-4}"
SKIP_LOCAL=0
SKIP_PUBLIC=0
SKIP_WRITE_CANARY=0
IROHA_RUNNER=()

usage() {
  cat <<'EOF'
Usage: check_mcp_rollout.sh [--local-url URL] [--public-url URL] [--skip-local] [--skip-public]
                            [--write-config PATH] [--write-target local|public|URL]
                            [--iroha-bin PATH] [--skip-write-canary]

Verify that Taira's native Torii MCP endpoint is live locally and/or publicly.
The check fails unless:
  - GET /v1/mcp returns HTTP 200 with a capabilities payload
  - POST /v1/mcp tools/list returns HTTP 200
  - the tool list includes curated iroha.* names, including write-ready aliases
  - the tool list does not expose raw torii.* names
  - GET /status returns healthy Torii/Sumeragi counters
  - /status reports at least 4 validators in the commit QC set

For final public rollout, also pass --write-config with a runtime-only
pre-provisioned canary signer config. Without that, public checks are rejected
unless --skip-write-canary is provided explicitly for read-only validation.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
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
  echo "public rollout requires --write-config for a signed canary write; use --skip-write-canary only for read-only validation" >&2
  exit 1
fi

if [[ -z "$IROHA_BIN" ]]; then
  if [[ -x "${REPO_ROOT}/target/debug/iroha" ]]; then
    IROHA_BIN="${REPO_ROOT}/target/debug/iroha"
  elif [[ -x "${REPO_ROOT}/target/release/iroha" ]]; then
    IROHA_BIN="${REPO_ROOT}/target/release/iroha"
  else
    IROHA_BIN="iroha"
  fi
fi

JSONRPC_TOOLS_LIST='{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}'
REQUIRED_TOOL_NAMES=(
  "iroha.status"
  "iroha.sumeragi.status"
  "iroha.time.now"
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

mcp_root_from_url() {
  local url="$1"
  printf '%s\n' "${url%/v1/mcp}"
}

http_request() {
  local method="$1"
  local url="$2"
  local payload="${3:-}"
  local body_file header_file

  body_file="$(mktemp)"
  header_file="$(mktemp)"
  cleanup
  last_body="$body_file"
  last_headers="$header_file"

  if [[ "$method" == "GET" ]]; then
    last_status="$(
      curl \
      --silent \
      --show-error \
      --output "$body_file" \
      --dump-header "$header_file" \
      --write-out "%{http_code}" \
      "$url"
    )"
  else
    last_status="$(
      curl \
      --silent \
      --show-error \
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

check_status_snapshot() {
  local label="$1"
  local status_url="$2"

  echo "==> ${label}: GET ${status_url}"
  http_request GET "$status_url"
  if [[ "$last_status" != "200" ]]; then
    echo "${label}: /status failed with HTTP ${last_status}" >&2
    sed -n '1,20p' "$last_headers" >&2 || true
    exit 1
  fi
  python3 - "$label" "$last_body" "$MIN_VALIDATOR_SET_LEN" <<'PY'
import json
import sys

label = sys.argv[1]
path = sys.argv[2]
min_validator_set_len = int(sys.argv[3])
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
    print(
        f"{label}: /status reported an empty Sumeragi commit validator set: {validator_set_len!r}",
        file=sys.stderr,
    )
    sys.exit(1)
if validator_set_len < min_validator_set_len:
    print(
        f"{label}: /status reported only {validator_set_len} validators in the commit QC set; "
        f"Taira rollout expects at least {min_validator_set_len}. "
        "Render per-validator configs from configs/soranexus/taira/validator_roster.example.toml "
        "with scripts/render_taira_validator_bundle.py before cutting traffic.",
        file=sys.stderr,
    )
    sys.exit(1)
PY
}

check_endpoint() {
  local label="$1"
  local url="$2"
  local root_url

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

  root_url="$(mcp_root_from_url "$url")"
  check_status_snapshot "$label" "${root_url}/status"
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

  python3 - "$source_config" "$target_torii_url" "$output_config" <<'PY'
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

source_path, target_torii_url, output_path = sys.argv[1:]
with open(source_path, "rb") as handle:
    source = tomllib.load(handle)

chain = source.get("chain")
account = source.get("account") or {}
public_key = account.get("public_key")
private_key = account.get("private_key")
domain = account.get("domain", "wonderland")
basic_auth = source.get("basic_auth")

if not isinstance(chain, str) or not chain:
    raise SystemExit("write canary config is missing a top-level `chain` value")
if not isinstance(public_key, str) or not public_key:
    raise SystemExit("write canary config is missing `account.public_key`")
if not isinstance(private_key, str) or not private_key:
    raise SystemExit("write canary config is missing `account.private_key`")
if not isinstance(domain, str) or not domain:
    domain = "wonderland"

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
        "",
    ]
)

with open(output_path, "w", encoding="utf-8") as handle:
    handle.write("\n".join(lines))
PY
}

ensure_iroha_bin() {
  if [[ "$IROHA_BIN" == */* ]]; then
    [[ -x "$IROHA_BIN" ]] || {
      echo "iroha binary is not executable: $IROHA_BIN" >&2
      exit 1
    }
    IROHA_RUNNER=("$IROHA_BIN")
  else
    if command -v "$IROHA_BIN" >/dev/null 2>&1; then
      IROHA_RUNNER=("$IROHA_BIN")
      return 0
    fi
    if [[ "$IROHA_BIN" == "iroha" ]] && command -v cargo >/dev/null 2>&1; then
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
      return 0
    fi
    echo "could not find iroha binary on PATH: $IROHA_BIN" >&2
    exit 1
  fi
}

run_write_canary() {
  local target_url="$1"
  local output_file temp_config write_msg

  ensure_iroha_bin
  [[ -f "$WRITE_CONFIG" ]] || {
    echo "write canary config does not exist: $WRITE_CONFIG" >&2
    exit 1
  }

  temp_config="$(mktemp)"
  output_file="$(mktemp)"
  trap 'rm -f "$temp_config" "$output_file"; cleanup' EXIT
  build_write_canary_config "$WRITE_CONFIG" "$target_url" "$temp_config"

  write_msg="${WRITE_MESSAGE_PREFIX}-$(date -u +%Y%m%dT%H%M%SZ)"
  echo "==> write canary: ${target_url} (message: ${write_msg})"
  if ! "${IROHA_RUNNER[@]}" --machine -c "$temp_config" ledger transaction ping --msg "$write_msg" \
      >"$output_file" 2>&1; then
    if grep -q 'route_unavailable' "$output_file"; then
      echo "write canary failed: Torii is reachable but no authoritative peers accepted the lane route" >&2
      echo "hint: re-render every validator config from configs/soranexus/taira/validator_roster.example.toml using scripts/render_taira_validator_bundle.py and confirm the ingress node is running one of those generated configs with the full trusted_peers/trusted_peers_pop roster" >&2
    else
      echo "write canary failed" >&2
    fi
    sed -n '1,80p' "$output_file" >&2 || true
    exit 1
  fi
  rm -f "$temp_config" "$output_file"
  trap cleanup EXIT
}

if [[ $SKIP_LOCAL -eq 0 ]]; then
  check_endpoint "local" "$LOCAL_MCP_URL"
fi

if [[ $SKIP_PUBLIC -eq 0 ]]; then
  check_endpoint "public" "$PUBLIC_MCP_URL"
fi

if [[ -n "$WRITE_CONFIG" ]]; then
  run_write_canary "$(resolve_write_target_url)"
elif [[ $SKIP_PUBLIC -eq 0 ]]; then
  echo "read-only checks passed; signed write canary was explicitly skipped" >&2
fi

echo "Taira MCP rollout checks passed."

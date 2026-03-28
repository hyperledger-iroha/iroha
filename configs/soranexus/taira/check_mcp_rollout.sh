#!/usr/bin/env bash
set -euo pipefail

LOCAL_MCP_URL="${LOCAL_MCP_URL:-http://127.0.0.1:18080/v1/mcp}"
PUBLIC_MCP_URL="${PUBLIC_MCP_URL:-https://taira.sora.org/v1/mcp}"
SKIP_LOCAL=0
SKIP_PUBLIC=0

usage() {
  cat <<'EOF'
Usage: check_mcp_rollout.sh [--local-url URL] [--public-url URL] [--skip-local] [--skip-public]

Verify that Taira's native Torii MCP endpoint is live locally and/or publicly.
The check fails unless:
  - GET /v1/mcp returns HTTP 200 with a capabilities payload
  - POST /v1/mcp tools/list returns HTTP 200
  - the tool list includes curated iroha.* names
  - the tool list does not expose raw torii.* names
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

JSONRPC_TOOLS_LIST='{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}'

last_body=""
last_headers=""
last_status=""

cleanup() {
  [[ -n "$last_body" && -f "$last_body" ]] && rm -f "$last_body"
  [[ -n "$last_headers" && -f "$last_headers" ]] && rm -f "$last_headers"
  return 0
}

trap cleanup EXIT

http_request() {
  local method="$1"
  local url="$2"
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
      --data "$JSONRPC_TOOLS_LIST" \
      "$url"
    )"
  fi
}

check_endpoint() {
  local label="$1"
  local url="$2"

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
  http_request POST "$url"
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
  if grep -q '"torii\.' "$last_body"; then
    echo "${label}: tools/list still exposes raw torii.* tool names" >&2
    sed -n '1,80p' "$last_body" >&2 || true
    exit 1
  fi
}

if [[ $SKIP_LOCAL -eq 0 ]]; then
  check_endpoint "local" "$LOCAL_MCP_URL"
fi

if [[ $SKIP_PUBLIC -eq 0 ]]; then
  check_endpoint "public" "$PUBLIC_MCP_URL"
fi

echo "Taira MCP rollout checks passed."

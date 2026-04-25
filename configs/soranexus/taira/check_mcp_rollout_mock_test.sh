#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
SOURCE_SCRIPT="${SCRIPT_DIR}/check_mcp_rollout.sh"

cleanup_paths=()

cleanup() {
  local path
  for path in "${cleanup_paths[@]:-}"; do
    [[ -n "$path" && -e "$path" ]] && rm -rf "$path"
  done
}

trap cleanup EXIT

make_fake_repo() {
  local root="$1"

  mkdir -p \
    "${root}/configs/soranexus/taira" \
    "${root}/scripts" \
    "${root}/mockbin" \
    "${root}/state"
  cp "$SOURCE_SCRIPT" "${root}/configs/soranexus/taira/check_mcp_rollout.sh"

  cat >"${root}/scripts/taira_bootstrap_canary.py" <<'PY'
#!/usr/bin/env python3
import sys

output_path = None
args = sys.argv[1:]
for index, value in enumerate(args):
    if value == "--output-config" and index + 1 < len(args):
        output_path = args[index + 1]
        break

if output_path is None:
    raise SystemExit("missing --output-config")

with open(output_path, "w", encoding="utf-8") as handle:
    handle.write(
        'chain = "iroha3-taira"\n'
        '\n'
        '[account]\n'
        'domain = "universal"\n'
        'public_key = "ED0120FAKEPUBLICKEY0123456789ABCDEF0123456789ABCDEF0123456789AB"\n'
        'private_key = "802620FAKEPRIVATEKEY0123456789ABCDEF0123456789ABCDEF0123456789"\n'
        'chain_discriminant = 369\n'
        '\n'
        '[transaction]\n'
        'nonce = true\n'
        'time_to_live_ms = 120000\n'
        'status_timeout_ms = 120000\n'
    )
PY

  cat >"${root}/scripts/taira_faucet_canary.py" <<'PY'
#!/usr/bin/env python3
print("faucet bootstrap skipped in mock harness")
PY

  cat >"${root}/mockbin/curl" <<'SH'
#!/usr/bin/env bash
set -euo pipefail

method="GET"
output_file=""
header_file=""
payload=""
url=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --output)
      output_file="$2"
      shift 2
      ;;
    --dump-header)
      header_file="$2"
      shift 2
      ;;
    --resolve)
      shift 2
      ;;
    --write-out)
      shift 2
      ;;
    -X)
      method="$2"
      shift 2
      ;;
    -H)
      shift 2
      ;;
    --data)
      payload="$2"
      shift 2
      ;;
    --silent|--show-error)
      shift
      ;;
    *)
      url="$1"
      shift
      ;;
  esac
done

scenario="${MOCK_SCENARIO:-}"
after_ping=0
if [[ -n "${MOCK_STATE_DIR:-}" && -f "${MOCK_STATE_DIR}/ping_seen" ]]; then
  after_ping=1
fi

status="200"
content_type="application/json"
body='{}'

if [[ "$method" == "GET" && "$url" == "https://taira.sora.org/v1/mcp" ]]; then
  if [[ $after_ping -eq 1 && "$scenario" == "public_503_mcp" ]]; then
    status="503"
    content_type="text/plain"
    body='service unavailable'
  else
    body='{"capabilities":{"tools":{}}}'
  fi
elif [[ "$method" == "POST" && "$url" == "https://taira.sora.org/v1/mcp" && "$payload" == *'"method":"initialize"'* ]]; then
  body='{"result":{"protocolVersion":"2025-06-18"}}'
elif [[ "$method" == "POST" && "$url" == "https://taira.sora.org/v1/mcp" && "$payload" == *'"method":"notifications/initialized"'* ]]; then
  status="202"
  body=''
elif [[ "$method" == "POST" && "$url" == "https://taira.sora.org/v1/mcp" && "$payload" == *'"method":"tools/list"'* ]]; then
  body='{"result":{"tools":[{"name":"iroha.status","inputSchema":{"type":"object","properties":{}}},{"name":"iroha.sumeragi.status","inputSchema":{"type":"object","properties":{}}},{"name":"iroha.time.now","inputSchema":{"type":"object","properties":{}}},{"name":"iroha.musubi.search","inputSchema":{"type":"object","properties":{}}},{"name":"iroha.musubi.release.get","inputSchema":{"type":"object","properties":{}}},{"name":"iroha.musubi.instructions.yank_release","inputSchema":{"type":"object","properties":{}}},{"name":"iroha.transactions.submit","inputSchema":{"type":"object","properties":{}}},{"name":"iroha.transactions.submit_and_wait","inputSchema":{"type":"object","properties":{}}}]}}'
elif [[ "$method" == "GET" && "$url" == "https://taira.sora.org/status" ]]; then
  if [[ $after_ping -eq 1 && "$scenario" == "public_502" ]]; then
    status="502"
    content_type="text/plain"
    body='bad gateway'
  elif [[ $after_ping -eq 1 && "$scenario" == "public_503" ]]; then
    status="503"
    content_type="text/plain"
    body='service unavailable'
  else
    body='{"peers":4,"blocks":707,"queue_size":0,"teu_dataspace_backlog":[{"backlog":0}],"sumeragi":{"commit_qc_height":707,"commit_qc_validator_set_len":4,"tx_queue_depth":1,"tx_queue_saturated":false}}'
  fi
elif [[ "$method" == "GET" && "$url" == "https://taira.sora.org/v1/sumeragi/status" ]]; then
  body='{"commit_qc_height":707,"highest_qc_height":708,"view_change_causes":{"last_cause":"missing_qc"},"worker_loop":{"stage":"idle"}}'
elif [[ "$method" == "GET" && "$url" == "https://taira.sora.org/v1/sccp/capabilities" ]]; then
  body='{}'
elif [[ "$method" == "GET" && "$url" == "https://taira.sora.org/v1/sccp/manifests" ]]; then
  body='{}'
elif [[ "$method" == "GET" && "$url" == "https://taira.sora.org/v1/zk/proofs/count" ]]; then
  body='{}'
elif [[ "$method" == "GET" && "$url" == "https://taira.sora.org/v1/sumeragi/validator-sets" ]]; then
  body='{}'
elif [[ "$method" == "GET" && "$url" == "https://taira.sora.org/v1/nexus/public_lanes/0/validators" ]]; then
  body='{}'
elif [[ "$method" == "GET" && "$url" == "https://taira.sora.org/v1/nexus/public_lanes/0/stake" ]]; then
  body='{}'
elif [[ "$method" == "GET" && "$url" == "https://taira.sora.org/v1/contracts/state" ]]; then
  status="400"
  body='{"error":"missing selectors"}'
elif [[ "$method" == "GET" && "$url" == "https://taira.sora.org/v1/musubi/packages?query=&limit=1" ]]; then
  body='{"items":[]}'
elif [[ "$method" == "POST" && "$url" == "https://taira.sora.org/v1/musubi/instructions/yank-release" ]]; then
  body='{"instructions":[]}'
elif [[ "$method" == "POST" && "$url" == "https://taira.sora.org/v1/contracts/deploy" ]]; then
  status="400"
  body='{"error":"empty deploy body"}'
elif [[ "$method" == "POST" && "$url" == "https://taira.sora.org/v1/bridge/messages" ]]; then
  status="400"
  body='{"error":"empty bridge body"}'
else
  status="404"
  body='{"error":"unexpected mock route"}'
fi

printf 'HTTP/1.1 %s mock\r\nContent-Type: %s\r\n\r\n' "$status" "$content_type" >"$header_file"
printf '%s' "$body" >"$output_file"
printf '%s' "$status"
SH

  cat >"${root}/mockbin/iroha" <<'SH'
#!/usr/bin/env bash
set -euo pipefail

if [[ "$*" == *"ledger transaction ping"* ]]; then
  mkdir -p "${MOCK_STATE_DIR:?}"
  : > "${MOCK_STATE_DIR}/ping_seen"
    case "${MOCK_SCENARIO:-}" in
      txn_expired)
        echo "Transaction expired"
        exit 1
        ;;
      permission_403)
        echo "HTTP 403 Forbidden"
        exit 1
        ;;
      public_502|public_503|public_503_mcp)
        echo "ping failed while public ingress was degrading"
        exit 1
        ;;
    *)
      echo "unexpected mock scenario for ping: ${MOCK_SCENARIO:-}" >&2
      exit 1
      ;;
  esac
fi

if [[ "$*" == *"tools address convert"* ]]; then
  echo '{"i105":{"value":"testuFAKEACCOUNT@universal"}}'
  exit 0
fi

echo "unexpected iroha invocation: $*" >&2
exit 1
SH

  cat >"${root}/mockbin/cargo" <<'SH'
#!/usr/bin/env bash
set -euo pipefail

mkdir -p "${MOCK_STATE_DIR:?}"
: > "${MOCK_STATE_DIR}/cargo_seen"

if [[ "$*" == *"-p iroha_cli --bin iroha --"* && "$*" == *"ledger transaction ping"* ]]; then
  : > "${MOCK_STATE_DIR}/ping_seen"
  exit 0
fi

echo "unexpected cargo invocation: $*" >&2
exit 1
SH

  chmod +x \
    "${root}/configs/soranexus/taira/check_mcp_rollout.sh" \
    "${root}/scripts/taira_bootstrap_canary.py" \
    "${root}/scripts/taira_faucet_canary.py" \
    "${root}/mockbin/curl" \
    "${root}/mockbin/iroha" \
    "${root}/mockbin/cargo"
}

run_case() {
  local scenario="$1"
  local expected_pattern="$2"
  local extra_pattern="${3:-}"
  local root output_file

  root="$(mktemp -d)"
  cleanup_paths+=("$root")
  make_fake_repo "$root"
  output_file="${root}/output.log"

  if PATH="${root}/mockbin:${PATH}" \
      MOCK_SCENARIO="$scenario" \
      MOCK_STATE_DIR="${root}/state" \
      "${root}/configs/soranexus/taira/check_mcp_rollout.sh" \
        --skip-local \
        --public-root https://taira.sora.org \
        --write-config "${root}/canary.toml" \
        --iroha-bin "${root}/mockbin/iroha" \
        >"$output_file" 2>&1; then
    echo "case ${scenario} unexpectedly succeeded" >&2
    sed -n '1,160p' "$output_file" >&2 || true
    return 1
  fi

  if ! grep -q "$expected_pattern" "$output_file"; then
    echo "case ${scenario} did not emit expected pattern: ${expected_pattern}" >&2
    sed -n '1,200p' "$output_file" >&2 || true
    return 1
  fi

  if [[ -n "$extra_pattern" ]] && ! grep -q "$extra_pattern" "$output_file"; then
    echo "case ${scenario} did not emit expected extra pattern: ${extra_pattern}" >&2
    sed -n '1,200p' "$output_file" >&2 || true
    return 1
  fi
}

run_case txn_expired 'write canary failed: transaction expired' '"highest_qc_height": 708'
run_case permission_403 'write canary failed: signer or permission check returned 403' '"commit_qc_height": 707'
run_case public_502 'public Torii ingress looks degraded' 'HTTP 502'
run_case public_503 'public Torii ingress looks degraded' 'HTTP 503'
run_case public_503_mcp 'public MCP ingress looks degraded' 'HTTP 503'

root="$(mktemp -d)"
cleanup_paths+=("$root")
make_fake_repo "$root"
if PATH="${root}/mockbin:${PATH}" \
    MOCK_SCENARIO="txn_expired" \
    MOCK_STATE_DIR="${root}/state" \
    "${root}/configs/soranexus/taira/check_mcp_rollout.sh" \
      --skip-local \
      --public-root https://taira.sora.org \
      --resolve-host taira.sora.org:443:127.0.0.1 \
      --write-config "${root}/canary.toml" \
      --iroha-bin "${root}/mockbin/iroha" \
      >"${root}/resolve-output.log" 2>&1; then
  echo "resolve-host case unexpectedly succeeded" >&2
  sed -n '1,200p' "${root}/resolve-output.log" >&2 || true
  exit 1
fi
grep -q 'write canary failed: transaction expired' "${root}/resolve-output.log"

root="$(mktemp -d)"
cleanup_paths+=("$root")
make_fake_repo "$root"
rm -f "${root}/mockbin/iroha"
mkdir -p "${root}/tmp"
if ! PATH="${root}/mockbin:/usr/bin:/bin" \
    TMPDIR="${root}/tmp/" \
    MOCK_SCENARIO="cargo_success" \
    MOCK_STATE_DIR="${root}/state" \
    WRITE_CONFIG_DEFAULT="${root}/tmp/taira-canary-client.toml" \
    "${root}/configs/soranexus/taira/check_mcp_rollout.sh" \
      --skip-local \
      --public-root https://taira.sora.org \
      >"${root}/cargo-output.log" 2>&1; then
  echo "cargo fallback case unexpectedly failed" >&2
  sed -n '1,200p' "${root}/cargo-output.log" >&2 || true
  exit 1
fi
grep -q 'Taira MCP rollout checks passed.' "${root}/cargo-output.log"
test -f "${root}/state/cargo_seen"
test -f "${root}/tmp/taira-canary-client.toml"

echo "check_mcp_rollout mock tests passed."

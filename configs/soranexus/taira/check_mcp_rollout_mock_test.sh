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
        'chain = "fc56984b-2be7-431d-840e-21514d1883f0"\n'
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
connect_timeout=""
max_time=""

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
    --connect-timeout)
      connect_timeout="$2"
      shift 2
      ;;
    --max-time)
      max_time="$2"
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

if [[ -n "${MOCK_STATE_DIR:-}" ]]; then
  printf '%s %s\n' "$connect_timeout" "$max_time" >> "${MOCK_STATE_DIR}/curl_timeouts"
fi
if [[ -z "$connect_timeout" || -z "$max_time" ]]; then
  echo "curl timeout flags were not provided" >&2
  exit 91
fi

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
  if [[ "$scenario" == "initialized_timeout" ]]; then
    echo "curl: (28) Operation timed out after mock timeout" >&2
    exit 28
  fi
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
  elif [[ "$scenario" == "status_build_sha_missing" ]]; then
    body='{"peers":4,"blocks":707,"queue_size":0,"teu_dataspace_backlog":[{"backlog":0}],"sumeragi":{"commit_qc_height":707,"highest_qc_height":707,"locked_qc_height":707,"commit_qc_validator_set_len":4,"tx_queue_depth":1,"tx_queue_saturated":false}}'
  elif [[ "$scenario" == "status_build_sha_too_short" ]]; then
    body='{"build":{"git_commit_sha":"490dac"},"peers":4,"blocks":707,"queue_size":0,"teu_dataspace_backlog":[{"backlog":0}],"sumeragi":{"commit_qc_height":707,"highest_qc_height":707,"locked_qc_height":707,"commit_qc_validator_set_len":4,"tx_queue_depth":1,"tx_queue_saturated":false}}'
  elif [[ "$scenario" == "status_build_sha_mismatch" ]]; then
    body='{"build":{"git_commit_sha":"94dcbf7c28"},"peers":4,"blocks":707,"queue_size":0,"teu_dataspace_backlog":[{"backlog":0}],"sumeragi":{"commit_qc_height":707,"highest_qc_height":707,"locked_qc_height":707,"commit_qc_validator_set_len":4,"tx_queue_depth":1,"tx_queue_saturated":false}}'
  else
    body='{"build":{"git_commit_sha":"490dacc287"},"peers":4,"blocks":707,"queue_size":0,"teu_dataspace_backlog":[{"backlog":0}],"sumeragi":{"commit_qc_height":707,"highest_qc_height":707,"locked_qc_height":707,"commit_qc_validator_set_len":4,"tx_queue_depth":1,"tx_queue_saturated":false}}'
  fi
elif [[ "$method" == "GET" && "$url" == "https://taira.sora.org/v1/sumeragi/status" ]]; then
  if [[ $after_ping -eq 1 && "$scenario" == "post_canary_sumeragi_missing_validator_set" ]]; then
    body='{"protocol_version":2,"build_fingerprint":"build","config_fingerprint":"config","height_context_id":"context","height":708,"view":0,"phase":"prepare","leader":0,"last_committed_height":707,"last_committed_subject":"subject","body_state":"available","pending_persistence_id":null}'
  elif [[ "$scenario" == "sumeragi_highest_qc_behind_commit" ]]; then
    body='{"protocol_version":2,"node_fingerprint":"node","build_fingerprint":"build","config_fingerprint":"config","height_context_id":"context","height":6544,"view":0,"phase":"prepare","leader":0,"last_committed_height":9532,"last_committed_subject":"subject","body_state":"available","pending_persistence_id":null}'
  elif [[ "$scenario" == "sumeragi_locked_qc_behind_commit" ]]; then
    body='{"protocol_version":2,"node_fingerprint":"node","build_fingerprint":"build","config_fingerprint":"config","height_context_id":"context","height":9533,"view":0,"phase":"prepare","leader":0,"last_committed_height":9532,"body_state":"available","pending_persistence_id":null}'
  elif [[ "$scenario" == "sumeragi_idle_high_view_missing_qc" ]]; then
    body='{"protocol_version":2,"node_fingerprint":"node","build_fingerprint":"build","config_fingerprint":"config","height_context_id":"context","height":708,"view":42,"phase":"prepare","leader":0,"last_committed_height":707,"last_committed_subject":"subject","body_state":"fetching","pending_persistence_id":0}'
  else
    body='{"protocol_version":2,"node_fingerprint":"node","build_fingerprint":"build","config_fingerprint":"config","height_context_id":"context","height":708,"view":0,"phase":"prepare","leader":0,"last_committed_height":707,"last_committed_subject":"subject","body_state":"available","pending_persistence_id":null}'
  fi
elif [[ "$method" == "GET" && "$url" == "https://taira.sora.org/v1/sccp/capabilities" ]]; then
  body='{}'
elif [[ "$method" == "GET" && "$url" == "https://taira.sora.org/v1/sccp/registry" ]]; then
  body='{"version":1,"lanes":[]}'
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
      post_canary_sumeragi_missing_validator_set)
        echo "pong"
        exit 0
        ;;
      success)
        echo "pong"
        exit 0
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
  local expected_git_sha="${4:-}"
  local root output_file

  root="$(mktemp -d)"
  cleanup_paths+=("$root")
  make_fake_repo "$root"
  output_file="${root}/output.log"

  if PATH="${root}/mockbin:${PATH}" \
      MOCK_SCENARIO="$scenario" \
      MOCK_STATE_DIR="${root}/state" \
      EXPECTED_TAIRA_GIT_SHA="$expected_git_sha" \
      WRITE_CONFIG_DEFAULT="${root}/canary.toml" \
      POST_CANARY_STATUS_RECHECK_ATTEMPTS=2 \
      POST_CANARY_STATUS_RECHECK_DELAY_SECONDS=0 \
      "${root}/configs/soranexus/taira/check_mcp_rollout.sh" \
        --skip-local \
        --public-root https://taira.sora.org \
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

run_invalid_canary_identity_case() {
  local mutation="$1"
  local expected_pattern="$2"
  local root output_file config_path

  root="$(mktemp -d)"
  cleanup_paths+=("$root")
  make_fake_repo "$root"
  output_file="${root}/invalid-${mutation}.log"
  config_path="${root}/canary.toml"
  "${root}/scripts/taira_bootstrap_canary.py" --output-config "$config_path"
  python3 - "$config_path" "$mutation" <<'PY'
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
mutation = sys.argv[2]
text = path.read_text(encoding="utf-8")
if mutation == "archived-chain":
    text = text.replace(
        "fc56984b-2be7-431d-840e-21514d1883f0",
        "809574f5-fee7-5e69-bfcf-52451e42d50f",
        1,
    )
elif mutation == "wrong-discriminant":
    text = text.replace("chain_discriminant = 369", "chain_discriminant = 753", 1)
else:
    raise SystemExit(f"unknown mutation: {mutation}")
path.write_text(text, encoding="utf-8")
PY

  if PATH="${root}/mockbin:${PATH}" \
      MOCK_SCENARIO="cargo_success" \
      MOCK_STATE_DIR="${root}/state" \
      "${root}/configs/soranexus/taira/check_mcp_rollout.sh" \
        --skip-local \
        --public-root https://taira.sora.org \
        --write-config "$config_path" \
        --iroha-bin "${root}/mockbin/iroha" \
        >"$output_file" 2>&1; then
    echo "invalid canary identity case ${mutation} unexpectedly succeeded" >&2
    sed -n '1,200p' "$output_file" >&2 || true
    return 1
  fi
  if ! grep -q "$expected_pattern" "$output_file"; then
    echo "invalid canary identity case ${mutation} did not emit expected pattern: ${expected_pattern}" >&2
    sed -n '1,200p' "$output_file" >&2 || true
    return 1
  fi
}

run_case txn_expired 'write canary failed: transaction expired' '"last_committed_height": 707'
run_case permission_403 'write canary failed: signer or permission check returned 403' '"blocks": 707'
run_case public_502 'public Torii ingress looks degraded' 'HTTP 502'
run_case public_503 'public Torii ingress looks degraded' 'HTTP 503'
run_case public_503_mcp 'public MCP ingress looks degraded' 'HTTP 503'
run_case initialized_timeout 'initialized notification failed with HTTP curl_error_28' 'Operation timed out'
run_case status_build_sha_missing '/status did not publish build.git_commit_sha' '' '490dacc'
run_case status_build_sha_too_short '/status build git SHA 490dac is not a 7 to 40 character hexadecimal SHA prefix' '' '490dacc'
run_case status_build_sha_mismatch '/status build git SHA 94dcbf7c28 does not match expected 490dacc' '' '490dacc'
run_case sumeragi_highest_qc_behind_commit 'committed height 9532 is ahead of reducer height 6544'
run_case sumeragi_locked_qc_behind_commit 'v2 status omitted last_committed_subject for a non-genesis commit'
run_case sumeragi_idle_high_view_missing_qc 'v2 status reported invalid pending persistence id: 0'
run_case post_canary_sumeragi_missing_validator_set '/v1/sumeragi/status still did not publish a healthy commit QC snapshot after the signed write canary' 'v2 status omitted required field(s): node_fingerprint'
run_invalid_canary_identity_case \
  archived-chain \
  'write canary config must target the public Sumeragi-v2 Taira chain'
run_invalid_canary_identity_case \
  wrong-discriminant \
  'write canary config must use Taira chain discriminant 369'

root="$(mktemp -d)"
cleanup_paths+=("$root")
make_fake_repo "$root"
if PATH="${root}/mockbin:${PATH}" \
    MOCK_SCENARIO="cargo_success" \
    MOCK_STATE_DIR="${root}/state" \
    "${root}/configs/soranexus/taira/check_mcp_rollout.sh" \
      --skip-local \
      --public-root https://taira.sora.org \
      --write-config "${root}/missing-canary.toml" \
      --iroha-bin "${root}/mockbin/iroha" \
      >"${root}/missing-canary-output.log" 2>&1; then
  echo "explicit missing write-config case unexpectedly succeeded" >&2
  sed -n '1,200p' "${root}/missing-canary-output.log" >&2 || true
  exit 1
fi
grep -q 'write canary config not found:' "${root}/missing-canary-output.log"

root="$(mktemp -d)"
cleanup_paths+=("$root")
make_fake_repo "$root"
"${root}/scripts/taira_bootstrap_canary.py" \
  --output-config "${root}/explicit-canary.toml"
before_hash="$(shasum -a 256 "${root}/explicit-canary.toml" | awk '{print $1}')"
if ! PATH="${root}/mockbin:${PATH}" \
    MOCK_SCENARIO="success" \
    MOCK_STATE_DIR="${root}/state" \
    POST_CANARY_STATUS_RECHECK_ATTEMPTS=2 \
    POST_CANARY_STATUS_RECHECK_DELAY_SECONDS=0 \
    "${root}/configs/soranexus/taira/check_mcp_rollout.sh" \
      --skip-local \
      --public-root https://taira.sora.org \
      --write-config "${root}/explicit-canary.toml" \
      --iroha-bin "${root}/mockbin/iroha" \
      >"${root}/explicit-canary-output.log" 2>&1; then
  echo "explicit write-config preservation case unexpectedly failed" >&2
  sed -n '1,200p' "${root}/explicit-canary-output.log" >&2 || true
  exit 1
fi
after_hash="$(shasum -a 256 "${root}/explicit-canary.toml" | awk '{print $1}')"
[[ "$before_hash" == "$after_hash" ]]
grep -q 'Taira MCP rollout checks passed.' "${root}/explicit-canary-output.log"

root="$(mktemp -d)"
cleanup_paths+=("$root")
make_fake_repo "$root"
if PATH="${root}/mockbin:${PATH}" \
    MOCK_SCENARIO="cargo_success" \
    MOCK_STATE_DIR="${root}/state" \
    "${root}/configs/soranexus/taira/check_mcp_rollout.sh" \
      --skip-local \
      --public-root https://taira.sora.org \
      --expected-git-sha bad \
      --skip-write-canary \
      >"${root}/invalid-sha-output.log" 2>&1; then
  echo "invalid expected git SHA case unexpectedly succeeded" >&2
  sed -n '1,200p' "${root}/invalid-sha-output.log" >&2 || true
  exit 1
fi
grep -q -- '--expected-git-sha must be a 7 to 40 character hexadecimal git SHA prefix' \
  "${root}/invalid-sha-output.log"

root="$(mktemp -d)"
cleanup_paths+=("$root")
make_fake_repo "$root"
if PATH="${root}/mockbin:${PATH}" \
    MOCK_SCENARIO="cargo_success" \
    MOCK_STATE_DIR="${root}/state" \
    "${root}/configs/soranexus/taira/check_mcp_rollout.sh" \
      --skip-local \
      --public-root https://taira.sora.org \
      --curl-connect-timeout-seconds 0 \
      --skip-write-canary \
      >"${root}/invalid-connect-timeout-output.log" 2>&1; then
  echo "invalid connect timeout case unexpectedly succeeded" >&2
  sed -n '1,200p' "${root}/invalid-connect-timeout-output.log" >&2 || true
  exit 1
fi
grep -q 'MCP_ROLLOUT_CURL_CONNECT_TIMEOUT_SECONDS must be a positive integer' \
  "${root}/invalid-connect-timeout-output.log"

root="$(mktemp -d)"
cleanup_paths+=("$root")
make_fake_repo "$root"
if PATH="${root}/mockbin:${PATH}" \
    MOCK_SCENARIO="cargo_success" \
    MOCK_STATE_DIR="${root}/state" \
    "${root}/configs/soranexus/taira/check_mcp_rollout.sh" \
      --skip-local \
      --public-root https://taira.sora.org \
      --curl-max-time-seconds abc \
      --skip-write-canary \
      >"${root}/invalid-max-time-output.log" 2>&1; then
  echo "invalid max-time case unexpectedly succeeded" >&2
  sed -n '1,200p' "${root}/invalid-max-time-output.log" >&2 || true
  exit 1
fi
grep -q 'MCP_ROLLOUT_CURL_MAX_TIME_SECONDS must be a positive integer' \
  "${root}/invalid-max-time-output.log"

root="$(mktemp -d)"
cleanup_paths+=("$root")
make_fake_repo "$root"
if PATH="${root}/mockbin:${PATH}" \
    MOCK_SCENARIO="cargo_success" \
    MOCK_STATE_DIR="${root}/state" \
    POST_CANARY_STATUS_RECHECK_DELAY_SECONDS=abc \
    "${root}/configs/soranexus/taira/check_mcp_rollout.sh" \
      --skip-local \
      --public-root https://taira.sora.org \
      --skip-write-canary \
      >"${root}/invalid-recheck-delay-output.log" 2>&1; then
  echo "invalid recheck delay case unexpectedly succeeded" >&2
  sed -n '1,200p' "${root}/invalid-recheck-delay-output.log" >&2 || true
  exit 1
fi
grep -q 'POST_CANARY_STATUS_RECHECK_DELAY_SECONDS must be a non-negative integer' \
  "${root}/invalid-recheck-delay-output.log"

root="$(mktemp -d)"
cleanup_paths+=("$root")
make_fake_repo "$root"
if PATH="${root}/mockbin:${PATH}" \
    MOCK_SCENARIO="cargo_success" \
    MOCK_STATE_DIR="${root}/state" \
    PUBLIC_LANE_ID='0/../../status' \
    "${root}/configs/soranexus/taira/check_mcp_rollout.sh" \
      --skip-local \
      --public-root https://taira.sora.org \
      --skip-write-canary \
      >"${root}/invalid-public-lane-output.log" 2>&1; then
  echo "invalid public lane case unexpectedly succeeded" >&2
  sed -n '1,200p' "${root}/invalid-public-lane-output.log" >&2 || true
  exit 1
fi
grep -q 'PUBLIC_LANE_ID must be a non-negative integer' \
  "${root}/invalid-public-lane-output.log"

root="$(mktemp -d)"
cleanup_paths+=("$root")
make_fake_repo "$root"
if PATH="${root}/mockbin:${PATH}" \
    MOCK_SCENARIO="txn_expired" \
    MOCK_STATE_DIR="${root}/state" \
    WRITE_CONFIG_DEFAULT="${root}/canary.toml" \
    "${root}/configs/soranexus/taira/check_mcp_rollout.sh" \
      --skip-local \
      --public-root https://taira.sora.org \
      --resolve-host taira.sora.org:443:127.0.0.1 \
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
if ! PATH="${root}/mockbin:${PATH}" \
    MOCK_SCENARIO="cargo_success" \
    MOCK_STATE_DIR="${root}/state" \
    "${root}/configs/soranexus/taira/check_mcp_rollout.sh" \
      --skip-local \
      --public-root https://taira.sora.org \
      --curl-connect-timeout-seconds 7 \
      --curl-max-time-seconds 33 \
      --skip-write-canary \
      >"${root}/curl-timeout-output.log" 2>&1; then
  echo "custom curl timeout case unexpectedly failed" >&2
  sed -n '1,200p' "${root}/curl-timeout-output.log" >&2 || true
  exit 1
fi
grep -q 'Taira MCP rollout checks passed.' "${root}/curl-timeout-output.log"
if grep -vq '^7 33$' "${root}/state/curl_timeouts"; then
  echo "custom curl timeout case did not pass custom timeout flags to every curl call" >&2
  cat "${root}/state/curl_timeouts" >&2
  exit 1
fi

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

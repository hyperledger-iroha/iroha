#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
SOURCE_SCRIPT="${SCRIPT_DIR}/check_sorafs_rollout.sh"

cleanup_paths=()

cleanup() {
  local path
  for path in "${cleanup_paths[@]:-}"; do
    [[ -n "$path" && -e "$path" ]] && rm -rf "$path"
  done
  return 0
}

trap cleanup EXIT

make_fake_repo() {
  local root="$1"

  mkdir -p \
    "${root}/configs/soranexus/taira" \
    "${root}/scripts" \
    "${root}/mockbin" \
    "${root}/state"
  cp "$SOURCE_SCRIPT" "${root}/configs/soranexus/taira/check_sorafs_rollout.sh"

  cat >"${root}/scripts/taira_bootstrap_canary.py" <<'PY'
#!/usr/bin/env python3
import os
import sys
from pathlib import Path

output_path = None
gas_asset_id = None
skip_faucet = False
args = sys.argv[1:]
for index, value in enumerate(args):
    if value == "--output-config" and index + 1 < len(args):
        output_path = args[index + 1]
    elif value == "--gas-asset-id" and index + 1 < len(args):
        gas_asset_id = args[index + 1]
    elif value == "--skip-faucet":
        skip_faucet = True

if output_path is None:
    raise SystemExit("missing --output-config")

state_dir = os.environ.get("MOCK_STATE_DIR")
if state_dir:
    state = Path(state_dir)
    state.joinpath("bootstrap_seen").write_text("1\n", encoding="utf-8")
    if gas_asset_id is not None:
        state.joinpath("bootstrap_gas_asset_seen").write_text(
            gas_asset_id + "\n", encoding="utf-8"
        )
    if skip_faucet:
        state.joinpath("bootstrap_skip_faucet_seen").write_text(
            "1\n", encoding="utf-8"
        )

with open(output_path, "w", encoding="utf-8") as handle:
    handle.write(
        'chain = "iroha3-taira"\n'
        '\n'
        '[account]\n'
        'domain = "universal"\n'
        'public_key = "ED0120BOOTSTRAPPUBLICKEY0123456789ABCDEF0123456789ABCDEF0123456789"\n'
        'private_key = "802620BOOTSTRAPPRIVATEKEY0123456789ABCDEF0123456789ABCDEF0123456789"\n'
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
import os
from pathlib import Path

state_dir = os.environ.get("MOCK_STATE_DIR")
if state_dir:
    Path(state_dir, "faucet_seen").write_text("1\n", encoding="utf-8")
print("faucet bootstrap skipped in mock harness")
PY

  cat >"${root}/mockbin/curl" <<'SH'
#!/usr/bin/env bash
set -euo pipefail

method="GET"
output_file=""
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
    --write-out)
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
    --request)
      method="$2"
      shift 2
      ;;
    --header)
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

if [[ -z "$connect_timeout" || -z "$max_time" ]]; then
  echo "mock curl missing timeout controls" >&2
  exit 1
fi
if [[ -n "${MOCK_STATE_DIR:-}" ]]; then
  mkdir -p "$MOCK_STATE_DIR"
  printf '%s %s\n' "$connect_timeout" "$max_time" >>"${MOCK_STATE_DIR}/curl_timeouts"
fi

scenario="${MOCK_SCENARIO:-}"
status="200"
body='{}'
provider_id=""
if [[ -n "${MOCK_STATE_DIR:-}" && -f "${MOCK_STATE_DIR}/provider_id" ]]; then
  provider_id="$(cat "${MOCK_STATE_DIR}/provider_id")"
fi

case "${method} ${url}" in
  "POST https://taira.sora.org/v1/sorafs/pin/register")
    status="400"
    body='{"error":"empty pin registration"}'
    ;;
  "POST https://taira.sora.org/v1/sorafs/capacity/declare")
    status="400"
    body='{"error":"empty capacity declaration"}'
    ;;
  "POST https://taira.sora.org/v1/sorafs/capacity/schedule")
    status="400"
    body='{"error":"empty capacity schedule"}'
    ;;
  "GET https://taira.sora.org/v1/sorafs/capacity/state")
    if [[ "$scenario" == "capacity_state_503" ]]; then
      status="503"
      body='service unavailable'
    elif [[ -n "${MOCK_STATE_DIR:-}" && -f "${MOCK_STATE_DIR}/submit_seen" && "$scenario" != "state_missing_after_submit" && -n "$provider_id" ]]; then
      body="{\"declarations\":[{\"provider_id_hex\":\"${provider_id}\",\"status\":\"active\"}]}"
    else
      body='{"declarations":[]}'
    fi
    ;;
  "GET https://taira.sora.org/status")
    if [[ "$scenario" == "status_blocks_zero" ]]; then
      body='{"blocks":0}'
    else
      body='{"blocks":707}'
    fi
    ;;
  "GET https://taira.sora.org/v1/sumeragi/status")
    if [[ "$scenario" == "sumeragi_3_validators" ]]; then
      body='{"commit_qc":{"height":707,"validator_set_len":3},"highest_qc":{"height":707},"locked_qc":{"height":707},"canonical":{"height":707}}'
    elif [[ "$scenario" == "sumeragi_canonical_behind" ]]; then
      body='{"commit_qc":{"height":707,"validator_set_len":4},"highest_qc":{"height":707},"locked_qc":{"height":707},"canonical":{"height":706}}'
    elif [[ "$scenario" == "sumeragi_idle_high_view_missing_qc" ]]; then
      body='{"commit_qc":{"height":707,"validator_set_len":4},"highest_qc":{"height":707},"locked_qc":{"height":707},"canonical":{"height":708,"phase":"prepare","view":42,"pending_finality":null,"rbc_status":"disabled"},"membership":{"height":708,"view":42},"worker_loop":{"stage":"idle"},"view_change_causes":{"last_cause":"missing_qc"},"tx_queue":{"depth":1,"capacity":20000,"saturated_by_age":true,"oldest_queued_age_ms":30000},"pending_rbc":{"sessions":0}}'
    else
      body='{"commit_qc":{"height":707,"validator_set_len":4},"highest_qc":{"height":707},"locked_qc":{"height":707},"canonical":{"height":707}}'
    fi
    ;;
  *)
    status="404"
    body='{"error":"unexpected mock route"}'
    ;;
esac

if [[ -z "$output_file" ]]; then
  echo "mock curl missing --output" >&2
  exit 1
fi

printf '%s' "$body" >"$output_file"
printf '%s' "$status"
SH

  cat >"${root}/mockbin/iroha" <<'SH'
#!/usr/bin/env bash
set -euo pipefail

if [[ "$*" == *"tools address convert"* ]]; then
  echo '{"i105":{"value":"testuFAKEACCOUNT@universal"}}'
  exit 0
fi

if [[ "$*" == *"ledger transaction stdin"* ]]; then
  mkdir -p "${MOCK_STATE_DIR:?}"
  cat >"${MOCK_STATE_DIR}/submitted_tx.json"
  : >"${MOCK_STATE_DIR}/submit_seen"
  metadata_file=""
  previous=""
  for arg in "$@"; do
    if [[ "$previous" == "-m" || "$previous" == "--metadata" ]]; then
      metadata_file="$arg"
      previous=""
      continue
    fi
    case "$arg" in
      -m|--metadata)
        previous="$arg"
        ;;
      -m=*|--metadata=*)
        metadata_file="${arg#*=}"
        ;;
    esac
  done
  if [[ -z "$metadata_file" ]]; then
    echo "missing gas_asset_id in transaction metadata"
    exit 1
  fi
  python3 - "$metadata_file" "${MOCK_STATE_DIR}" <<'PY'
import json
import sys
from pathlib import Path

path = Path(sys.argv[1])
state = Path(sys.argv[2])
payload = json.loads(path.read_text(encoding="utf-8"))
gas_asset_id = payload.get("gas_asset_id")
if gas_asset_id != "6TEAJqbb8oEPmLncoNiMRbLEK6tw":
    raise SystemExit(f"unexpected gas_asset_id: {gas_asset_id!r}")
state.joinpath("gas_asset_seen").write_text(gas_asset_id + "\n", encoding="utf-8")
PY
  case "${MOCK_SCENARIO:-}" in
    unknown_instruction)
      echo "Unknown instruction type: SoraFsCapacityDeclaration"
      exit 1
      ;;
    failed_asset_then_success)
      if [[ ! -f "${MOCK_STATE_DIR}/retry_seen" ]]; then
        : >"${MOCK_STATE_DIR}/retry_seen"
        echo "Failed to find asset"
        exit 1
      fi
      echo '{"status":"committed"}'
      exit 0
      ;;
    success|explicit_config|state_missing_after_submit)
      echo '{"status":"committed"}'
      exit 0
      ;;
    *)
      echo "unexpected mock scenario for stdin transaction: ${MOCK_SCENARIO:-}" >&2
      exit 1
      ;;
  esac
fi

echo "unexpected iroha invocation: $*" >&2
exit 1
SH

  cat >"${root}/mockbin/sorafs_manifest_stub" <<'SH'
#!/usr/bin/env bash
set -euo pipefail

spec=""
request_out=""
authority=""
private_key=""
private_key_file=""
argv_log="${MOCK_STATE_DIR:?}/sorafs_manifest_stub_argv"
printf '%s\n' "$*" >"$argv_log"

for arg in "$@"; do
  case "$arg" in
    --spec=*)
      spec="${arg#--spec=}"
      ;;
    --request-out=*)
      request_out="${arg#--request-out=}"
      ;;
    --authority=*)
      authority="${arg#--authority=}"
      ;;
    --private-key=*)
      echo "private key must be passed through --private-key-file in rollout tests" >&2
      exit 1
      ;;
    --private-key-file=*)
      private_key_file="${arg#--private-key-file=}"
      ;;
  esac
done

[[ "$1" == "capacity" && "$2" == "declaration" ]] || {
  echo "unexpected sorafs_manifest_stub invocation: $*" >&2
  exit 1
}
[[ -n "$spec" && -n "$request_out" && -n "$authority" && -n "$private_key_file" ]] || {
  echo "missing capacity declaration arguments" >&2
  exit 1
}

python3 - "$spec" "$request_out" "$authority" "$private_key_file" "${MOCK_STATE_DIR:?}" <<'PY'
import json
import sys
from pathlib import Path

spec_path, request_path, authority, private_key_file, state_dir = sys.argv[1:]
private_key = Path(private_key_file).read_text(encoding="utf-8").strip()
with open(spec_path, "r", encoding="utf-8") as handle:
    spec = json.load(handle)
provider_id = spec["provider_id_hex"]
Path(state_dir, "provider_id").write_text(provider_id, encoding="utf-8")
Path(state_dir, "authority_seen").write_text(authority, encoding="utf-8")
Path(state_dir, "private_key_seen").write_text(private_key, encoding="utf-8")
Path(state_dir, "private_key_file_seen").write_text(private_key_file, encoding="utf-8")
with open(request_path, "w", encoding="utf-8") as handle:
    json.dump(
        {
            "provider_id_hex": provider_id,
            "authority": authority,
            "payload": spec,
        },
        handle,
        sort_keys=True,
    )
    handle.write("\n")
PY
SH

  cat >"${root}/mockbin/sorafs_tx_stdin_builder" <<'SH'
#!/usr/bin/env bash
set -euo pipefail

request=""
for arg in "$@"; do
  case "$arg" in
    --request=*)
      request="${arg#--request=}"
      ;;
  esac
done

[[ "$1" == "capacity-declaration-request" && -n "$request" ]] || {
  echo "unexpected sorafs_tx_stdin_builder invocation: $*" >&2
  exit 1
}

python3 - "$request" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    request = json.load(handle)

print(json.dumps({"isi": "sorafs_capacity_declaration", "request": request}, sort_keys=True))
PY
SH

  chmod +x \
    "${root}/configs/soranexus/taira/check_sorafs_rollout.sh" \
    "${root}/scripts/taira_bootstrap_canary.py" \
    "${root}/scripts/taira_faucet_canary.py" \
    "${root}/mockbin/curl" \
    "${root}/mockbin/iroha" \
    "${root}/mockbin/sorafs_manifest_stub" \
    "${root}/mockbin/sorafs_tx_stdin_builder"
}

write_canary_config() {
  local path="$1"
  local public_key="$2"
  local private_key="$3"

  cat >"$path" <<EOF
chain = "iroha3-taira"

[account]
domain = "universal"
public_key = "${public_key}"
private_key = "${private_key}"
chain_discriminant = 369

[transaction]
nonce = true
time_to_live_ms = 120000
status_timeout_ms = 120000
EOF
}

run_rollout() {
  local root="$1"
  local scenario="$2"
  shift 2

  PATH="${root}/mockbin:${PATH}" \
    MOCK_SCENARIO="$scenario" \
    MOCK_STATE_DIR="${root}/state" \
    CAPACITY_STATE_RECHECK_ATTEMPTS=2 \
    CAPACITY_STATE_RECHECK_DELAY_SECONDS=0 \
    "${root}/configs/soranexus/taira/check_sorafs_rollout.sh" \
      --public-root https://taira.sora.org \
      "$@"
}

make_case_root() {
  local root
  root="$(mktemp -d)"
  cleanup_paths+=("$root")
  make_fake_repo "$root"
  printf '%s\n' "$root"
}

run_read_only_success_case() {
  local root output_file
  root="$(make_case_root)"
  output_file="${root}/output.log"

  run_rollout "$root" success --skip-write-canary >"$output_file" 2>&1
  grep -q 'SoraFS rollout verification passed.' "$output_file"
  test ! -f "${root}/state/bootstrap_seen"
  test ! -f "${root}/state/submit_seen"
}

run_implicit_bootstrap_success_case() {
  local root output_file config_path
  root="$(make_case_root)"
  output_file="${root}/output.log"
  config_path="${root}/tmp/taira-canary-client.toml"
  mkdir -p "${root}/tmp"

  WRITE_CONFIG_DEFAULT="$config_path" \
    run_rollout \
      "$root" \
      success \
      --iroha-bin "${root}/mockbin/iroha" \
      --sorafs-manifest-stub-bin "${root}/mockbin/sorafs_manifest_stub" \
      --sorafs-tx-stdin-builder-bin "${root}/mockbin/sorafs_tx_stdin_builder" \
      >"$output_file" 2>&1

  grep -q 'SoraFS rollout verification passed.' "$output_file"
  test -f "${root}/state/bootstrap_seen"
  grep -q '6TEAJqbb8oEPmLncoNiMRbLEK6tw' "${root}/state/bootstrap_gas_asset_seen"
  test -f "${root}/state/bootstrap_skip_faucet_seen"
  test -f "${root}/state/submit_seen"
  test -f "${root}/state/gas_asset_seen"
  test -f "$config_path"
  ! grep -q 'BOOTSTRAPPRIVATEKEY' "${root}/state/sorafs_manifest_stub_argv"
  grep -q 'BOOTSTRAPPRIVATEKEY' "${root}/state/private_key_seen"
}

run_custom_http_timeouts_are_passed_to_curl_case() {
  local root output_file
  root="$(make_case_root)"
  output_file="${root}/output.log"

  run_rollout \
    "$root" \
    success \
    --skip-write-canary \
    --curl-connect-timeout-seconds 7 \
    --curl-max-time-seconds 33 \
    >"$output_file" 2>&1

  grep -q 'SoraFS rollout verification passed.' "$output_file"
  grep -q '^7 33$' "${root}/state/curl_timeouts"
  test ! -f "${root}/state/bootstrap_seen"
  test ! -f "${root}/state/submit_seen"
}

run_explicit_config_is_preserved_case() {
  local root output_file config_path before_hash after_hash
  root="$(make_case_root)"
  output_file="${root}/output.log"
  config_path="${root}/explicit-canary.toml"
  write_canary_config \
    "$config_path" \
    "ED0120EXPLICITPUBLICKEY0123456789ABCDEF0123456789ABCDEF0123456789" \
    "802620EXPLICITPRIVATEKEY0123456789ABCDEF0123456789ABCDEF0123456789"
  before_hash="$(shasum -a 256 "$config_path" | awk '{print $1}')"

  run_rollout \
    "$root" \
    explicit_config \
    --write-config "$config_path" \
    --iroha-bin "${root}/mockbin/iroha" \
    --sorafs-manifest-stub-bin "${root}/mockbin/sorafs_manifest_stub" \
    --sorafs-tx-stdin-builder-bin "${root}/mockbin/sorafs_tx_stdin_builder" \
    >"$output_file" 2>&1

  after_hash="$(shasum -a 256 "$config_path" | awk '{print $1}')"
  [[ "$before_hash" == "$after_hash" ]]
  grep -q 'SoraFS rollout verification passed.' "$output_file"
  test ! -f "${root}/state/bootstrap_seen"
  test -f "${root}/state/gas_asset_seen"
  grep -q 'EXPLICITPRIVATEKEY' "${root}/state/private_key_seen"
  ! grep -q 'EXPLICITPRIVATEKEY' "${root}/state/sorafs_manifest_stub_argv"
  test -f "${root}/state/private_key_file_seen"
}

run_explicit_missing_config_fails_without_bootstrap_case() {
  local root output_file config_path
  root="$(make_case_root)"
  output_file="${root}/output.log"
  config_path="${root}/missing-canary.toml"

  if run_rollout \
    "$root" \
    explicit_config \
    --write-config "$config_path" \
    --iroha-bin "${root}/mockbin/iroha" \
    --sorafs-manifest-stub-bin "${root}/mockbin/sorafs_manifest_stub" \
    --sorafs-tx-stdin-builder-bin "${root}/mockbin/sorafs_tx_stdin_builder" \
    >"$output_file" 2>&1; then
    echo "explicit missing write-config case unexpectedly succeeded" >&2
    sed -n '1,200p' "$output_file" >&2 || true
    return 1
  fi

  grep -q 'write canary config not found:' "$output_file"
  test ! -f "${root}/state/bootstrap_seen"
}

run_expected_failure_case() {
  local scenario="$1"
  local expected_pattern="$2"
  local extra_pattern="${3:-}"
  if [[ $# -ge 3 ]]; then
    shift 3
  else
    shift 2
  fi
  local root output_file config_path

  root="$(make_case_root)"
  output_file="${root}/output.log"
  config_path="${root}/explicit-canary.toml"
  write_canary_config \
    "$config_path" \
    "ED0120FAILPUBLICKEY0123456789ABCDEF0123456789ABCDEF0123456789AB" \
    "802620FAILPRIVATEKEY0123456789ABCDEF0123456789ABCDEF0123456789AB"

  if run_rollout \
    "$root" \
    "$scenario" \
    --write-config "$config_path" \
    --iroha-bin "${root}/mockbin/iroha" \
    --sorafs-manifest-stub-bin "${root}/mockbin/sorafs_manifest_stub" \
    --sorafs-tx-stdin-builder-bin "${root}/mockbin/sorafs_tx_stdin_builder" \
    "$@" \
    >"$output_file" 2>&1; then
    echo "case ${scenario} unexpectedly succeeded" >&2
    sed -n '1,200p' "$output_file" >&2 || true
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

run_expected_preflight_failure_case() {
  local scenario="$1"
  local expected_pattern="$2"
  local root output_file

  root="$(make_case_root)"
  output_file="${root}/output.log"

  if run_rollout "$root" "$scenario" --skip-write-canary >"$output_file" 2>&1; then
    echo "preflight case ${scenario} unexpectedly succeeded" >&2
    sed -n '1,200p' "$output_file" >&2 || true
    return 1
  fi

  if ! grep -q "$expected_pattern" "$output_file"; then
    echo "preflight case ${scenario} did not emit expected pattern: ${expected_pattern}" >&2
    sed -n '1,200p' "$output_file" >&2 || true
    return 1
  fi

  test ! -f "${root}/state/bootstrap_seen"
  test ! -f "${root}/state/submit_seen"
}

run_expected_numeric_failure_case() {
  local expected_pattern="$1"
  shift

  local root output_file
  root="$(make_case_root)"
  output_file="${root}/output.log"

  if PATH="${root}/mockbin:${PATH}" \
    MOCK_SCENARIO="success" \
    MOCK_STATE_DIR="${root}/state" \
    "$@" \
    "${root}/configs/soranexus/taira/check_sorafs_rollout.sh" \
      --public-root https://taira.sora.org \
      --iroha-bin "${root}/mockbin/iroha" \
      --sorafs-manifest-stub-bin "${root}/mockbin/sorafs_manifest_stub" \
      --sorafs-tx-stdin-builder-bin "${root}/mockbin/sorafs_tx_stdin_builder" \
      >"$output_file" 2>&1; then
    echo "numeric validation case unexpectedly succeeded" >&2
    sed -n '1,200p' "$output_file" >&2 || true
    return 1
  fi

  if ! grep -q "$expected_pattern" "$output_file"; then
    echo "numeric validation case did not emit expected pattern: ${expected_pattern}" >&2
    sed -n '1,200p' "$output_file" >&2 || true
    return 1
  fi

  test ! -f "${root}/state/bootstrap_seen"
  test ! -f "${root}/state/submit_seen"
}

run_expected_numeric_argument_failure_case() {
  local expected_pattern="$1"
  shift

  local root output_file
  root="$(make_case_root)"
  output_file="${root}/output.log"

  if run_rollout \
    "$root" \
    success \
    --iroha-bin "${root}/mockbin/iroha" \
    --sorafs-manifest-stub-bin "${root}/mockbin/sorafs_manifest_stub" \
    --sorafs-tx-stdin-builder-bin "${root}/mockbin/sorafs_tx_stdin_builder" \
    "$@" \
    >"$output_file" 2>&1; then
    echo "numeric argument validation case unexpectedly succeeded" >&2
    sed -n '1,200p' "$output_file" >&2 || true
    return 1
  fi

  if ! grep -q "$expected_pattern" "$output_file"; then
    echo "numeric argument validation case did not emit expected pattern: ${expected_pattern}" >&2
    sed -n '1,200p' "$output_file" >&2 || true
    return 1
  fi

  test ! -f "${root}/state/bootstrap_seen"
  test ! -f "${root}/state/submit_seen"
}

run_asset_retry_success_case() {
  local root output_file config_path
  root="$(make_case_root)"
  output_file="${root}/output.log"
  config_path="${root}/explicit-canary.toml"
  write_canary_config \
    "$config_path" \
    "ED0120ASSETRETRYPUBLICKEY0123456789ABCDEF0123456789ABCDEF0123" \
    "802620ASSETRETRYPRIVATEKEY0123456789ABCDEF0123456789ABCDEF0123"

  run_rollout \
    "$root" \
    failed_asset_then_success \
    --write-config "$config_path" \
    --iroha-bin "${root}/mockbin/iroha" \
    --sorafs-manifest-stub-bin "${root}/mockbin/sorafs_manifest_stub" \
    --sorafs-tx-stdin-builder-bin "${root}/mockbin/sorafs_tx_stdin_builder" \
    >"$output_file" 2>&1

  grep -q 'SoraFS rollout verification passed.' "$output_file"
  test -f "${root}/state/retry_seen"
  test -f "${root}/state/faucet_seen"
  test -f "${root}/state/gas_asset_seen"
  ! grep -q 'ASSETRETRYPRIVATEKEY' "${root}/state/sorafs_manifest_stub_argv"
}

run_read_only_success_case
run_implicit_bootstrap_success_case
run_custom_http_timeouts_are_passed_to_curl_case
run_explicit_config_is_preserved_case
run_explicit_missing_config_fails_without_bootstrap_case
run_asset_retry_success_case
run_expected_failure_case \
  capacity_state_503 \
  'capacity/state: expected HTTP 200, got 503'
run_expected_failure_case \
  unknown_instruction \
  'SoraFS capacity canary failed: the served validator binary is stale and missing SoraFS capacity/order instruction dispatch' \
  'Unknown instruction type'
run_expected_failure_case \
  state_missing_after_submit \
  'capacity canary transaction landed but the declaration never appeared in /v1/sorafs/capacity/state'
run_expected_failure_case \
  success \
  'SoraFS capacity canary failed: Taira requires gas_asset_id transaction metadata' \
  'missing gas_asset_id in transaction metadata' \
  --gas-asset-id ""
run_expected_preflight_failure_case \
  status_blocks_zero \
  'status payload did not include a positive `blocks` value'
run_expected_preflight_failure_case \
  sumeragi_3_validators \
  'sumeragi/status reported only 3 validators in the commit QC set'
run_expected_preflight_failure_case \
  sumeragi_canonical_behind \
  'sumeragi/status canonical height 706 is behind commit QC height 707'
run_expected_preflight_failure_case \
  sumeragi_idle_high_view_missing_qc \
  'sumeragi/status reports a finality fault'
run_expected_numeric_argument_failure_case \
  'DECLARED_CAPACITY_GIB must be a positive integer' \
  --declared-capacity-gib 0
run_expected_numeric_argument_failure_case \
  'STAKE_AMOUNT must be a positive integer' \
  --stake-amount not-a-number
run_expected_numeric_argument_failure_case \
  'DECLARATION_VALID_BLOCKS must be a positive integer' \
  --declaration-valid-blocks 0
run_expected_numeric_failure_case \
  'CAPACITY_STATE_RECHECK_ATTEMPTS must be a positive integer' \
  env CAPACITY_STATE_RECHECK_ATTEMPTS=0 CAPACITY_STATE_RECHECK_DELAY_SECONDS=0
run_expected_numeric_failure_case \
  'CAPACITY_STATE_RECHECK_DELAY_SECONDS must be a non-negative integer' \
  env CAPACITY_STATE_RECHECK_ATTEMPTS=2 CAPACITY_STATE_RECHECK_DELAY_SECONDS=-1
run_expected_numeric_failure_case \
  'ROLLOUT_CANARY_SKIP_FAUCET must be auto, 1, 0, true, false, yes, or no' \
  env ROLLOUT_CANARY_SKIP_FAUCET=maybe CAPACITY_STATE_RECHECK_ATTEMPTS=2 CAPACITY_STATE_RECHECK_DELAY_SECONDS=0
run_expected_numeric_argument_failure_case \
  'SORAFS_ROLLOUT_CURL_CONNECT_TIMEOUT_SECONDS must be a positive integer' \
  --curl-connect-timeout-seconds 0
run_expected_numeric_argument_failure_case \
  'SORAFS_ROLLOUT_CURL_MAX_TIME_SECONDS must be a positive integer' \
  --curl-max-time-seconds abc

echo "check_sorafs_rollout mock tests passed."

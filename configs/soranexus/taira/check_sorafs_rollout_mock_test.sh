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

  printf '%s\n' 'test-only-runtime-operator-private-key' \
    >"${root}/state/operator-private-key"
  chmod 600 "${root}/state/operator-private-key"

  cat >"${root}/scripts/operator_http_headers.py" <<'PY'
#!/usr/bin/env python3
print("X-Iroha-Operator-Public-Key: ed0120" + "11" * 32)
print("X-Iroha-Operator-Timestamp-Ms: 1800000000000")
print("X-Iroha-Operator-Nonce: " + "22" * 16)
print("X-Iroha-Operator-Signature: " + "33" * 64)
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
headers=()
operator_header_names=" "

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
      headers+=("$2")
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

for header in "${headers[@]+"${headers[@]}"}"; do
  header_name="$(printf '%s' "${header%%:*}" | tr '[:upper:]' '[:lower:]')"
  operator_header_names+="${header_name} "
done
if [[ "$url" == */v1/sumeragi/status ]]; then
  for required_header in \
    x-iroha-operator-public-key \
    x-iroha-operator-timestamp-ms \
    x-iroha-operator-nonce \
    x-iroha-operator-signature; do
    if [[ "$operator_header_names" != *" ${required_header} "* ]]; then
      echo "operator-protected status omitted ${required_header}" >&2
      exit 94
    fi
  done
fi


for header in "${headers[@]+"${headers[@]}"}"; do
  header_name="$(printf '%s' "${header%%:*}" | tr '[:upper:]' '[:lower:]')"
  if [[ "$header_name" == "x-iroha-api-version" ]]; then
    echo "rollout must not send the retired x-iroha-api-version header" >&2
    exit 92
  fi
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
  "GET https://taira.sora.org/v1/pipeline/transactions/status")
    if [[ "$scenario" == "canonical_status_missing" ]]; then
      status="404"
      body='{"code":"route_not_found","message":"route not found"}'
    elif [[ "$scenario" == "canonical_status_wrong_error" ]]; then
      status="400"
      body='{"code":"bad_request","message":"generic edge rejection"}'
    else
      status="400"
      body='{"code":"query_validation_failed","message":"missing hash query parameter"}'
    fi
    ;;
  "GET https://taira.sora.org/v1/transactions/status")
    if [[ "$scenario" == "retired_status_alias_mounted" ]]; then
      status="200"
      body='{"status":{"kind":"Applied"}}'
    elif [[ "$scenario" == "retired_status_wrong_error" ]]; then
      status="404"
      body='{"code":"not_found","message":"non-canonical edge rejection"}'
    else
      status="404"
      body='{"code":"route_not_found","message":"route not found"}'
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
    body='{"protocol_version":4,"restart_required":false,"node_fingerprint":"n","build_fingerprint":"b","config_fingerprint":"c","height_context_id":["ctx"],"height":708,"view":0,"phase":{"phase":"prepare","details":null},"leader":0,"body_state":{"state":"missing","details":null},"last_committed_height":707,"last_committed_subject":{"block_hash":"block","payload_hash":"payload"},"height_context":{"epoch":1,"epoch_end_height":720,"mode":{"mode":"permissioned","details":null},"epoch_seed":"0000000000000000000000000000000000000000000000000000000000000000","validator_count":4,"quorum":{"min_signers":3,"total_power":4}},"last_commit_qc":{"certificate":{"round":{"height":707,"view":0},"phase":{"phase":"commit","details":null},"subject":{"block_hash":"block","payload_hash":"payload"}},"validator_count":4,"signer_count":3,"min_signers":3,"signed_power":3,"total_power":4},"lane_settlement_commitments":[],"lane_relay_envelopes":[],"lane_payload_ownerships":[],"committed_lane_blocks":[],"lane_block_sessions":[],"local_peer_removed":false,"operator":{"adapter_queues":{"ingress_keys":0,"ingress_capacity":64,"deferred_completion":0,"deferred_progress":0,"deferred_progress_capacity":64,"deferred_normal":0,"deferred_normal_capacity":64},"tx_queue":{"tracked_transactions":0,"queued_transactions":0,"capacity":100,"max_retained_bytes":8192}}}'
    if [[ "$scenario" == "sumeragi_missing_restart_required" ]]; then
      body="${body/\"restart_required\":false,/}"
    elif [[ "$scenario" == "sumeragi_invalid_restart_required" ]]; then
      body="${body/\"restart_required\":false/\"restart_required\":0}"
    elif [[ "$scenario" == "sumeragi_3_validators" ]]; then
      body="${body//\"validator_count\":4/\"validator_count\":3}"
    elif [[ "$scenario" == "sumeragi_canonical_behind" || "$scenario" == "sumeragi_committed_ahead" ]]; then
      body="${body/\"height\":708/\"height\":706}"
    elif [[ "$scenario" == "sumeragi_idle_high_view_missing_qc" ]]; then
      body="${body/\"last_commit_qc\"/\"invalid_commit_qc\"}"
      body="${body/\"view\":0,\"phase\"/\"view\":42,\"phase\"}"
    elif [[ "$scenario" == "sumeragi_legacy_status" ]]; then
      body='{"commit_qc":{"height":707,"validator_set_len":4},"canonical":{"height":707}}'
    elif [[ "$scenario" == "sumeragi_pending_zero" ]]; then
      body="${body/\"last_committed_height\"/\"pending_persistence_id\":0,\"last_committed_height\"}"
    elif [[ "$scenario" == "sumeragi_missing_fingerprint" ]]; then
      body="${body/\"config_fingerprint\":\"c\",/}"
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
  echo '{"i105":{"value":"testuﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV@universal"}}'
  exit 0
fi

if [[ "$*" == *"ledger transaction stdin"* ]]; then
  mkdir -p "${MOCK_STATE_DIR:?}"
  cat >"${MOCK_STATE_DIR}/submitted_tx.json"
  : >"${MOCK_STATE_DIR}/submit_seen"
  fee_payer=""
  fee_program=""
  fee_program_revision=""
  previous=""
  for arg in "$@"; do
    if [[ "$previous" == "--fee-payer" ]]; then
      fee_payer="$arg"
      previous=""
      continue
    fi
    if [[ "$previous" == "--fee-program" ]]; then
      fee_program="$arg"
      previous=""
      continue
    fi
    if [[ "$previous" == "--fee-program-revision" ]]; then
      fee_program_revision="$arg"
      previous=""
      continue
    fi
    case "$arg" in
      --fee-payer|--fee-program|--fee-program-revision)
        previous="$arg"
        ;;
    esac
  done
  if [[ "$fee_payer" != "sponsor" || -z "$fee_program" || "$fee_program_revision" != "1" ]]; then
    echo "missing exact fee sponsor program selection"
    exit 1
  fi
  printf '%s\n' "$fee_program" >"${MOCK_STATE_DIR}/fee_program_seen"
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

  cat >"${root}/mockbin/sorafs_manifest_builder" <<'SH'
#!/usr/bin/env bash
set -euo pipefail

spec=""
summary_out=""
argv_log="${MOCK_STATE_DIR:?}/sorafs_manifest_builder_argv"
printf '%s\n' "$*" >"$argv_log"

for arg in "$@"; do
  case "$arg" in
    --spec=*)
      spec="${arg#--spec=}"
      ;;
    --json-out=*)
      summary_out="${arg#--json-out=}"
      ;;
  esac
done

[[ "$1" == "capacity" && "$2" == "declaration" ]] || {
  echo "unexpected sorafs_manifest_builder invocation: $*" >&2
  exit 1
}
[[ -n "$spec" && -n "$summary_out" ]] || {
  echo "missing capacity declaration arguments" >&2
  exit 1
}

python3 - "$spec" "$summary_out" "${MOCK_STATE_DIR:?}" <<'PY'
import json
import sys
from pathlib import Path

spec_path, summary_path, state_dir = sys.argv[1:]
with open(spec_path, "r", encoding="utf-8") as handle:
    spec = json.load(handle)
provider_id = spec["provider_id_hex"]
record_window = spec["record_window"]
Path(state_dir, "provider_id").write_text(provider_id, encoding="utf-8")
with open(summary_path, "w", encoding="utf-8") as handle:
    json.dump(
        {
            "provider_id_hex": provider_id,
            "declaration_b64": "fixture",
            "registered_epoch": record_window["registered_epoch"],
            "valid_from_epoch": record_window["valid_from_epoch"],
            "valid_until_epoch": record_window["valid_until_epoch"],
            "metadata": spec.get("metadata", {}),
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

summary=""
for arg in "$@"; do
  case "$arg" in
    --summary=*)
      summary="${arg#--summary=}"
      ;;
  esac
done

[[ "$1" == "capacity-declaration" && -n "$summary" ]] || {
  echo "unexpected sorafs_tx_stdin_builder invocation: $*" >&2
  exit 1
}

python3 - "$summary" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    summary = json.load(handle)

print(json.dumps({"isi": "sorafs_capacity_declaration", "summary": summary}, sort_keys=True))
PY
SH

  chmod +x \
    "${root}/configs/soranexus/taira/check_sorafs_rollout.sh" \
    "${root}/mockbin/curl" \
    "${root}/mockbin/iroha" \
    "${root}/mockbin/sorafs_manifest_builder" \
    "${root}/mockbin/sorafs_tx_stdin_builder"
}

write_canary_config() {
  local path="$1"
  local public_key="$2"
  local private_key="$3"

  cat >"$path" <<EOF
chain = "fc56984b-2be7-431d-840e-21514d1883f0"
network_id = "hash:82531CE8EAE8BFF6BEECA4698BFD13A3BC8BEC5F0EE0D23D428C97FC17AB0F3B#3E94"

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

run_invalid_canary_identity_case() {
  local mutation="$1"
  local expected_pattern="$2"
  local root output_file config_path

  root="$(make_case_root)"
  output_file="${root}/invalid-${mutation}.log"
  config_path="${root}/explicit-canary.toml"
  write_canary_config \
    "$config_path" \
    "ED0120INVALIDPUBLICKEY0123456789ABCDEF0123456789ABCDEF01234567" \
    "802620INVALIDPRIVATEKEY0123456789ABCDEF0123456789ABCDEF01234567"
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

  if run_rollout \
    "$root" \
    success \
    --write-config "$config_path" \
    --iroha-bin "${root}/mockbin/iroha" \
    --sorafs-manifest-builder-bin "${root}/mockbin/sorafs_manifest_builder" \
    --sorafs-tx-stdin-builder-bin "${root}/mockbin/sorafs_tx_stdin_builder" \
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
      --operator-network-id hash:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa#0000 \
      --operator-private-key-file "${root}/state/operator-private-key" \
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

  if ! run_rollout "$root" success --skip-write-canary >"$output_file" 2>&1; then
    echo "read-only rollout case failed" >&2
    sed -n '1,200p' "$output_file" >&2 || true
    return 1
  fi
  grep -q 'SoraFS rollout verification passed.' "$output_file"
  test ! -f "${root}/state/bootstrap_seen"
  test ! -f "${root}/state/submit_seen"
}

run_missing_write_config_fails_case() {
  local root output_file
  root="$(make_case_root)"
  output_file="${root}/output.log"

  if run_rollout "$root" success >"$output_file" 2>&1; then
    echo "missing write-config case unexpectedly succeeded" >&2
    return 1
  fi
  grep -q -- '--write-config is required unless --skip-write-canary is used' "$output_file"
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
    --sorafs-manifest-builder-bin "${root}/mockbin/sorafs_manifest_builder" \
    --sorafs-tx-stdin-builder-bin "${root}/mockbin/sorafs_tx_stdin_builder" \
    >"$output_file" 2>&1

  after_hash="$(shasum -a 256 "$config_path" | awk '{print $1}')"
  [[ "$before_hash" == "$after_hash" ]]
  grep -q 'SoraFS rollout verification passed.' "$output_file"
  test ! -f "${root}/state/bootstrap_seen"
  test -f "${root}/state/fee_program_seen"
  ! grep -q 'EXPLICITPRIVATEKEY' "${root}/state/sorafs_manifest_builder_argv"
  ! grep -Eq -- '--(request-out|authority|private-key)' "${root}/state/sorafs_manifest_builder_argv"
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
    --sorafs-manifest-builder-bin "${root}/mockbin/sorafs_manifest_builder" \
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
    --sorafs-manifest-builder-bin "${root}/mockbin/sorafs_manifest_builder" \
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
      --operator-network-id hash:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa#0000 \
      --operator-private-key-file "${root}/state/operator-private-key" \
      --skip-write-canary \
      --iroha-bin "${root}/mockbin/iroha" \
      --sorafs-manifest-builder-bin "${root}/mockbin/sorafs_manifest_builder" \
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
    --sorafs-manifest-builder-bin "${root}/mockbin/sorafs_manifest_builder" \
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

run_read_only_success_case
run_missing_write_config_fails_case
run_custom_http_timeouts_are_passed_to_curl_case
run_explicit_config_is_preserved_case
run_explicit_missing_config_fails_without_bootstrap_case
run_invalid_canary_identity_case \
  archived-chain \
  'write canary config must target the public Sumeragi-v2 Taira chain'
run_invalid_canary_identity_case \
  wrong-discriminant \
  'write canary config must use Taira chain discriminant 369'
run_expected_failure_case \
  failed_asset_then_success \
  'SoraFS capacity canary signer is unfunded' \
  'Failed to find asset'
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
run_expected_preflight_failure_case \
  status_blocks_zero \
  'status payload did not include a positive `blocks` value'
run_expected_preflight_failure_case \
  sumeragi_missing_restart_required \
  'sumeragi/status restart_required must be a boolean'
run_expected_preflight_failure_case \
  sumeragi_invalid_restart_required \
  'sumeragi/status restart_required must be a boolean'
run_expected_preflight_failure_case \
  sumeragi_3_validators \
  'sumeragi/status froze 3 validators; Taira requires exactly 4'
run_expected_preflight_failure_case \
  sumeragi_canonical_behind \
  'sumeragi/status reducer/commit frontier is inconsistent'
run_expected_preflight_failure_case \
  sumeragi_idle_high_view_missing_qc \
  'sumeragi/status omitted required last_commit_qc object'
run_expected_preflight_failure_case \
  canonical_status_missing \
  'pipeline transaction status: expected HTTP 400, got 404'
run_expected_preflight_failure_case \
  canonical_status_wrong_error \
  "pipeline transaction status: response error code was 'bad_request'"
run_expected_preflight_failure_case \
  retired_status_alias_mounted \
  'retired transaction status alias: expected HTTP 404, got 200'
run_expected_preflight_failure_case \
  retired_status_wrong_error \
  "retired transaction status alias: response error code was 'not_found'"
run_expected_preflight_failure_case \
  sumeragi_legacy_status \
  'expected the Sumeragi v2 reducer status'
run_expected_preflight_failure_case \
  sumeragi_committed_ahead \
  'sumeragi/status reducer/commit frontier is inconsistent'
run_expected_preflight_failure_case \
  sumeragi_pending_zero \
  'sumeragi/status reported invalid pending_persistence_id: 0'
run_expected_preflight_failure_case \
  sumeragi_missing_fingerprint \
  'sumeragi/status omitted required v2 field(s): config_fingerprint'
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
run_expected_numeric_argument_failure_case \
  'SORAFS_ROLLOUT_CURL_CONNECT_TIMEOUT_SECONDS must be a positive integer' \
  --curl-connect-timeout-seconds 0
run_expected_numeric_argument_failure_case \
  'SORAFS_ROLLOUT_CURL_MAX_TIME_SECONDS must be a positive integer' \
  --curl-max-time-seconds abc

echo "check_sorafs_rollout mock tests passed."

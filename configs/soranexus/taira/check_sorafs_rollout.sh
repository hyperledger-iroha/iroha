#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/../../.." && pwd)"
PUBLIC_TORII_ROOT="${PUBLIC_TORII_ROOT:-}"
WRITE_CONFIG="${WRITE_CONFIG:-}"
WRITE_CONFIG_EXPLICIT=0
WRITE_CONFIG_DEFAULT="${WRITE_CONFIG_DEFAULT:-}"
IROHA_BIN="${IROHA_BIN:-}"
SORAFS_MANIFEST_BUILDER_BIN="${SORAFS_MANIFEST_BUILDER_BIN:-}"
SORAFS_TX_STDIN_BUILDER_BIN="${SORAFS_TX_STDIN_BUILDER_BIN:-}"
ROLLOUT_CANARY_ALIAS_PREFIX="${ROLLOUT_CANARY_ALIAS_PREFIX:-taira-rollout-canary}"
ROLLOUT_CANARY_TIME_TO_LIVE_MS="${ROLLOUT_CANARY_TIME_TO_LIVE_MS:-120000}"
ROLLOUT_CANARY_STATUS_TIMEOUT_MS="${ROLLOUT_CANARY_STATUS_TIMEOUT_MS:-120000}"
ROLLOUT_CANARY_FAUCET_ASSET_ID="${ROLLOUT_CANARY_FAUCET_ASSET_ID:-6TEAJqbb8oEPmLncoNiMRbLEK6tw}"
ROLLOUT_CANARY_FEE_PROGRAM_ID="${ROLLOUT_CANARY_FEE_PROGRAM_ID:-testuﾛ1PｵEmｷjMZZﾑﾙeｱﾁﾎﾅﾂﾊmECepdbﾎｳ2uWﾃｸﾊﾘvｵi2ｦP1Y18A/default}"
ROLLOUT_CANARY_FEE_PROGRAM_REVISION="${ROLLOUT_CANARY_FEE_PROGRAM_REVISION:-1}"
ROLLOUT_CANARY_SKIP_FAUCET="${ROLLOUT_CANARY_SKIP_FAUCET:-auto}"
DECLARED_CAPACITY_GIB="${DECLARED_CAPACITY_GIB:-1}"
STAKE_AMOUNT="${STAKE_AMOUNT:-1}"
DECLARATION_VALID_BLOCKS="${DECLARATION_VALID_BLOCKS:-10000}"
PROVIDER_SEED_PREFIX="${PROVIDER_SEED_PREFIX:-taira-rollout-capacity-canary:v1}"
CAPACITY_STATE_RECHECK_ATTEMPTS="${CAPACITY_STATE_RECHECK_ATTEMPTS:-10}"
CAPACITY_STATE_RECHECK_DELAY_SECONDS="${CAPACITY_STATE_RECHECK_DELAY_SECONDS:-2}"
SORAFS_ROLLOUT_CURL_CONNECT_TIMEOUT_SECONDS="${SORAFS_ROLLOUT_CURL_CONNECT_TIMEOUT_SECONDS:-5}"
SORAFS_ROLLOUT_CURL_MAX_TIME_SECONDS="${SORAFS_ROLLOUT_CURL_MAX_TIME_SECONDS:-20}"
SKIP_WRITE_CANARY=0
IROHA_RUNNER=()
SORAFS_MANIFEST_BUILDER_RUNNER=()
SORAFS_TX_STDIN_BUILDER_RUNNER=()
CURL_RESOLVE_RULES=()
CURL_URL_RESOLVE_ARGS=()

usage() {
  cat <<'EOF'
Usage: check_sorafs_rollout.sh --public-root URL [--write-config PATH]
                               [--iroha-bin PATH]
                               [--sorafs-manifest-builder-bin PATH]
                               [--sorafs-tx-stdin-builder-bin PATH]
                               [--faucet-asset-id ASSET_DEFINITION_ID]
                               [--fee-program PROGRAM_ID]
                               [--fee-program-revision REVISION]
                               [--declared-capacity-gib N]
                               [--stake-amount N]
                               [--declaration-valid-blocks N]
                               [--provider-seed-prefix TEXT]
                               [--resolve-host HOST:IP|HOST:PORT:IP]
                               [--curl-connect-timeout-seconds N]
                               [--curl-max-time-seconds N]
                               [--skip-write-canary]

Verify that a public Taira node exposes the required SoraFS routes and, unless
`--skip-write-canary` is given, accepts a signed capacity declaration canary.

The check fails unless:
  - POST /v1/sorafs/pin/register returns HTTP 400 for an empty JSON body
  - POST /v1/sorafs/capacity/declare returns HTTP 400 for an empty JSON body
  - GET /v1/sorafs/capacity/state returns HTTP 200
  - GET /v1/pipeline/transactions/status reaches the canonical typed status
    handler (the no-hash probe returns HTTP 400), while the retired
    /v1/transactions/status alias remains unmounted (HTTP 404)
  - a deterministic capacity declaration lands through `iroha ledger transaction stdin`
  - the declaration is visible in /v1/sorafs/capacity/state

When `--write-config` is omitted, the script bootstraps a runtime-only canary
config automatically, preferring `/run/secrets/taira-canary-client.toml` when
that directory is writable and otherwise falling back to the local temp
directory. The bootstrap posts the current universal-account DTO to
`/v1/accounts/onboard`, requires `HTTP 202` with a `QUEUED` receipt, and follows
that receipt through `/v1/pipeline/transactions/status` before running the
capacity declaration canary. Onboarding fees are sponsored by the configured
Torii onboarding authority. The capacity transaction gets an exact
`/v1/fees/quote` and signs the returned intent for the configured immutable
sponsor-program revision. Set `ROLLOUT_CANARY_SKIP_FAUCET=0` to require an initial faucet
claim. Both onboarding and faucet helpers wait for their `202 QUEUED` receipts
to reach `Applied` or `Committed` through the canonical pipeline status route.
When `--write-config` is supplied, that runtime-only signer config is read
as-is and is never overwritten by bootstrap.
Use `--skip-write-canary` only for read-only validation.
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
  temp_root="$(physical_path "$temp_root")"
  printf '%s\n' "${temp_root%/}/taira-canary-client.toml"
}

physical_path() {
  python3 - "$1" <<'PY'
import os
import sys

print(os.path.realpath(sys.argv[1]))
PY
}

should_skip_canary_faucet() {
  case "$ROLLOUT_CANARY_SKIP_FAUCET" in
    auto|"")
      [[ -n "$ROLLOUT_CANARY_FAUCET_ASSET_ID" ]]
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
    --public-root)
      [[ $# -ge 2 ]] || {
        echo "missing value for --public-root" >&2
        exit 1
      }
      PUBLIC_TORII_ROOT="$2"
      shift 2
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
    --iroha-bin)
      [[ $# -ge 2 ]] || {
        echo "missing value for --iroha-bin" >&2
        exit 1
      }
      IROHA_BIN="$2"
      shift 2
      ;;
    --sorafs-manifest-builder-bin)
      [[ $# -ge 2 ]] || {
        echo "missing value for --sorafs-manifest-builder-bin" >&2
        exit 1
      }
      SORAFS_MANIFEST_BUILDER_BIN="$2"
      shift 2
      ;;
    --sorafs-tx-stdin-builder-bin)
      [[ $# -ge 2 ]] || {
        echo "missing value for --sorafs-tx-stdin-builder-bin" >&2
        exit 1
      }
      SORAFS_TX_STDIN_BUILDER_BIN="$2"
      shift 2
      ;;
    --faucet-asset-id)
      [[ $# -ge 2 ]] || {
        echo "missing value for --faucet-asset-id" >&2
        exit 1
      }
      ROLLOUT_CANARY_FAUCET_ASSET_ID="$2"
      shift 2
      ;;
    --fee-program)
      [[ $# -ge 2 ]] || {
        echo "missing value for --fee-program" >&2
        exit 1
      }
      ROLLOUT_CANARY_FEE_PROGRAM_ID="$2"
      shift 2
      ;;
    --fee-program-revision)
      [[ $# -ge 2 ]] || {
        echo "missing value for --fee-program-revision" >&2
        exit 1
      }
      ROLLOUT_CANARY_FEE_PROGRAM_REVISION="$2"
      shift 2
      ;;
    --declared-capacity-gib)
      [[ $# -ge 2 ]] || {
        echo "missing value for --declared-capacity-gib" >&2
        exit 1
      }
      DECLARED_CAPACITY_GIB="$2"
      shift 2
      ;;
    --stake-amount)
      [[ $# -ge 2 ]] || {
        echo "missing value for --stake-amount" >&2
        exit 1
      }
      STAKE_AMOUNT="$2"
      shift 2
      ;;
    --declaration-valid-blocks)
      [[ $# -ge 2 ]] || {
        echo "missing value for --declaration-valid-blocks" >&2
        exit 1
      }
      DECLARATION_VALID_BLOCKS="$2"
      shift 2
      ;;
    --provider-seed-prefix)
      [[ $# -ge 2 ]] || {
        echo "missing value for --provider-seed-prefix" >&2
        exit 1
      }
      PROVIDER_SEED_PREFIX="$2"
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
      SORAFS_ROLLOUT_CURL_CONNECT_TIMEOUT_SECONDS="$2"
      shift 2
      ;;
    --curl-max-time-seconds)
      [[ $# -ge 2 ]] || {
        echo "missing value for --curl-max-time-seconds" >&2
        exit 1
      }
      SORAFS_ROLLOUT_CURL_MAX_TIME_SECONDS="$2"
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

[[ -n "$PUBLIC_TORII_ROOT" ]] || {
  echo "--public-root is required" >&2
  exit 1
}

require_unsigned_integer() {
  local name="$1"
  local value="$2"
  local requirement="$3"

  if [[ ! "$value" =~ ^[0-9]+$ ]]; then
    echo "${name} must be ${requirement}" >&2
    exit 1
  fi
}

require_positive_integer() {
  local name="$1"
  local value="$2"

  require_unsigned_integer "$name" "$value" "a positive integer"
  if [[ "$value" =~ ^0+$ ]]; then
    echo "${name} must be a positive integer" >&2
    exit 1
  fi
}

require_non_negative_integer() {
  local name="$1"
  local value="$2"

  require_unsigned_integer "$name" "$value" "a non-negative integer"
}

validate_numeric_inputs() {
  require_positive_integer \
    "ROLLOUT_CANARY_TIME_TO_LIVE_MS" \
    "$ROLLOUT_CANARY_TIME_TO_LIVE_MS"
  require_positive_integer \
    "ROLLOUT_CANARY_STATUS_TIMEOUT_MS" \
    "$ROLLOUT_CANARY_STATUS_TIMEOUT_MS"
  require_positive_integer "DECLARED_CAPACITY_GIB" "$DECLARED_CAPACITY_GIB"
  require_positive_integer "STAKE_AMOUNT" "$STAKE_AMOUNT"
  require_positive_integer "DECLARATION_VALID_BLOCKS" "$DECLARATION_VALID_BLOCKS"
  require_positive_integer \
    "CAPACITY_STATE_RECHECK_ATTEMPTS" \
    "$CAPACITY_STATE_RECHECK_ATTEMPTS"
  require_non_negative_integer \
    "CAPACITY_STATE_RECHECK_DELAY_SECONDS" \
    "$CAPACITY_STATE_RECHECK_DELAY_SECONDS"
  require_positive_integer \
    "SORAFS_ROLLOUT_CURL_CONNECT_TIMEOUT_SECONDS" \
    "$SORAFS_ROLLOUT_CURL_CONNECT_TIMEOUT_SECONDS"
  require_positive_integer \
    "SORAFS_ROLLOUT_CURL_MAX_TIME_SECONDS" \
    "$SORAFS_ROLLOUT_CURL_MAX_TIME_SECONDS"
}

validate_numeric_inputs

if [[ -z "$WRITE_CONFIG" && $SKIP_WRITE_CANARY -eq 0 ]]; then
  WRITE_CONFIG="$(default_write_config_path)"
fi

normalize_root_url() {
  local url="$1"
  printf '%s\n' "${url%/}"
}

last_body=""
last_status=""

cleanup() {
  [[ -n "$last_body" && -f "$last_body" ]] && rm -f "$last_body"
  return 0
}

trap cleanup EXIT

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

http_request() {
  local method="$1"
  local url="$2"
  local payload="${3:-}"
  local body_file
  local curl_status
  # The first-release /v1 API has no version-negotiation request header.
  local curl_cmd=(
    curl
    --silent
    --show-error
    --header
    "accept: application/json"
    --connect-timeout
    "$SORAFS_ROLLOUT_CURL_CONNECT_TIMEOUT_SECONDS"
    --max-time
    "$SORAFS_ROLLOUT_CURL_MAX_TIME_SECONDS"
  )

  body_file="$(mktemp)"
  cleanup
  last_body="$body_file"
  build_curl_resolve_args "$url"
  curl_cmd+=( ${CURL_URL_RESOLVE_ARGS[@]+"${CURL_URL_RESOLVE_ARGS[@]}"} )

  if [[ "$method" == "GET" ]]; then
    curl_status="$( "${curl_cmd[@]}" --output "$body_file" --write-out '%{http_code}' "$url" )"
  else
    curl_status="$(
      "${curl_cmd[@]}" \
        --output "$body_file" \
        --write-out '%{http_code}' \
        --header 'content-type: application/json' \
        --request "$method" \
        --data "$payload" \
        "$url"
    )"
  fi

  last_status="$curl_status"
}

expect_status() {
  local label="$1"
  local method="$2"
  local url="$3"
  local expected_status="$4"
  local payload="${5:-}"
  local expected_error_code="${6:-}"

  http_request "$method" "$url" "$payload"
  if [[ "$last_status" != "$expected_status" ]]; then
    echo "${label}: expected HTTP ${expected_status}, got ${last_status}" >&2
    sed -n '1,120p' "$last_body" >&2 || true
    exit 1
  fi
  if [[ -n "$expected_error_code" ]]; then
    python3 - "$label" "$expected_error_code" "$last_body" <<'PY'
import json
import sys

label, expected_code, body_path = sys.argv[1:]
try:
    with open(body_path, "r", encoding="utf-8") as handle:
        payload = json.load(handle)
except (OSError, json.JSONDecodeError) as error:
    raise SystemExit(
        f"{label}: response was not a typed JSON error envelope: {error}"
    ) from error
actual_code = payload.get("code") if isinstance(payload, dict) else None
if actual_code != expected_code:
    raise SystemExit(
        f"{label}: response error code was {actual_code!r}; expected {expected_code!r}"
    )
PY
  fi
}

probe_surface() {
  local root_url="$1"
  echo "==> SoraFS route surface: ${root_url}"
  expect_status "pin/register" POST "${root_url}/v1/sorafs/pin/register" 400 '{}'
  expect_status "capacity/declare" POST "${root_url}/v1/sorafs/capacity/declare" 400 '{}'
  expect_status "capacity/state" GET "${root_url}/v1/sorafs/capacity/state" 200
  expect_status \
    "pipeline transaction status" \
    GET \
    "${root_url}/v1/pipeline/transactions/status" \
    400 \
    "" \
    "query_validation_failed"
  expect_status \
    "retired transaction status alias" \
    GET \
    "${root_url}/v1/transactions/status" \
    404 \
    "" \
    "route_not_found"
}

check_node_health() {
  local root_url="$1"
  echo "==> Taira node health: ${root_url}"
  expect_status "status" GET "${root_url}/status" 200
  python3 - "$last_body" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)

blocks = payload.get("blocks")
if not isinstance(blocks, int) or blocks <= 0:
    raise SystemExit("status payload did not include a positive `blocks` value")
PY

  expect_status "sumeragi/status" GET "${root_url}/v1/sumeragi/status" 200
  python3 - "$last_body" <<'PY'
import json
import re
import sys

def require_dict(value, label):
    if not isinstance(value, dict):
        raise SystemExit(f"sumeragi/status omitted required {label} object")
    return value


def require_uint(value, label, *, positive=False):
    if isinstance(value, bool) or not isinstance(value, int) or value < (1 if positive else 0):
        raise SystemExit(f"sumeragi/status reported invalid {label}: {value!r}")
    return value


def enum_tag(value, key, label):
    record = require_dict(value, label)
    if set(record) != {key, "details"}:
        raise SystemExit(f"sumeragi/status {label} is not a canonical tagged unit")
    tag = record.get(key)
    if not isinstance(tag, str) or not tag:
        raise SystemExit(f"sumeragi/status reported invalid {label} tag: {tag!r}")
    if record.get("details") is not None:
        raise SystemExit(f"sumeragi/status reported non-canonical {label}.details")
    return tag

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    status = json.load(handle)

if not isinstance(status, dict) or status.get("protocol_version") != 4:
    raise SystemExit(
        "expected the Sumeragi v2 reducer status; "
        "legacy RBC/recovery status is not accepted"
    )
restart_required = status.get("restart_required")
if not isinstance(restart_required, bool):
    raise SystemExit(
        "sumeragi/status restart_required must be a boolean, "
        f"got {restart_required!r}"
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
        f"sumeragi/status omitted required v2 field(s): {', '.join(missing)}"
    )

height = require_uint(status.get("height"), "height", positive=True)
view = require_uint(status.get("view"), "view")
leader = require_uint(status.get("leader"), "leader")
phase = enum_tag(status.get("phase"), "phase", "phase")
if phase not in {
    "awaiting_proposal",
    "reconstructing_payload",
    "validating_payload",
    "prepare",
    "commit",
    "pending_apply",
}:
    raise SystemExit(f"sumeragi/status reported invalid phase tag: {phase!r}")
body_state = enum_tag(status.get("body_state"), "state", "body_state")
if body_state not in {
    "missing",
    "reconstructing",
    "stored",
    "validated",
    "pending_apply",
    "applied",
}:
    raise SystemExit(f"sumeragi/status reported invalid body_state tag: {body_state!r}")
context = require_dict(status.get("height_context"), "height_context")
epoch = require_uint(context.get("epoch"), "height_context.epoch")
epoch_end = require_uint(
    context.get("epoch_end_height"), "height_context.epoch_end_height", positive=True
)
if epoch_end < height:
    raise SystemExit(
        f"sumeragi/status epoch end {epoch_end} is behind reducer height {height}"
    )
mode = enum_tag(context.get("mode"), "mode", "height_context.mode")
if mode not in {"permissioned", "npos"}:
    raise SystemExit(f"sumeragi/status reported invalid height_context.mode tag: {mode!r}")
epoch_seed = context.get("epoch_seed")
if not isinstance(epoch_seed, str) or re.fullmatch(r"[0-9A-F]{64}", epoch_seed) is None:
    raise SystemExit("sumeragi/status reported invalid canonical epoch-seed hex string")
validator_count = require_uint(
    context.get("validator_count"), "height_context.validator_count", positive=True
)
if validator_count < 4:
    raise SystemExit(
        f"sumeragi/status frozen only {validator_count} validators; Taira requires at least 4"
    )
if leader >= validator_count:
    raise SystemExit(
        f"sumeragi/status leader {leader} is outside validator roster {validator_count}"
    )
quorum = require_dict(context.get("quorum"), "height_context.quorum")
min_signers = require_uint(quorum.get("min_signers"), "height_context.quorum.min_signers")
total_power = require_uint(
    quorum.get("total_power"), "height_context.quorum.total_power", positive=True
)
expected_min_signers = validator_count - ((validator_count - 1) // 3)
if min_signers != expected_min_signers or total_power < validator_count:
    raise SystemExit(
        "sumeragi/status frozen quorum is inconsistent with its validator roster "
        f"(validators={validator_count}, min_signers={min_signers}, total_power={total_power})"
    )
if mode == "permissioned" and total_power != validator_count:
    raise SystemExit("permissioned v2 context must assign unit power to every validator")

committed_height = require_uint(
    status.get("last_committed_height"), "last_committed_height", positive=True
)
if committed_height > height or height - committed_height > 1:
    raise SystemExit(
        "sumeragi/status reducer/commit frontier is inconsistent "
        f"(height={height}, committed={committed_height})"
    )
subject = require_dict(status.get("last_committed_subject"), "last_committed_subject")
commit = require_dict(status.get("last_commit_qc"), "last_commit_qc")
certificate = require_dict(commit.get("certificate"), "last_commit_qc.certificate")
round_ = require_dict(certificate.get("round"), "last_commit_qc.certificate.round")
if require_uint(round_.get("height"), "last_commit_qc.certificate.round.height") != committed_height:
    raise SystemExit("last CommitQC height does not match the committed frontier")
if enum_tag(certificate.get("phase"), "phase", "last_commit_qc.certificate.phase") != "commit":
    raise SystemExit("last_commit_qc is not a Commit-phase certificate")
if certificate.get("subject") != subject:
    raise SystemExit("last CommitQC subject does not match last_committed_subject")

commit_validators = require_uint(
    commit.get("validator_count"), "last_commit_qc.validator_count", positive=True
)
signer_count = require_uint(commit.get("signer_count"), "last_commit_qc.signer_count")
commit_min = require_uint(commit.get("min_signers"), "last_commit_qc.min_signers")
signed_power = require_uint(commit.get("signed_power"), "last_commit_qc.signed_power")
commit_total_power = require_uint(
    commit.get("total_power"), "last_commit_qc.total_power", positive=True
)
expected_commit_min = commit_validators - ((commit_validators - 1) // 3)
if (
    commit_validators < 4
    or signer_count > commit_validators
    or commit_min != expected_commit_min
    or signer_count < commit_min
    or signed_power > commit_total_power
    or signed_power * 3 <= commit_total_power * 2
):
    raise SystemExit(
        "sumeragi/status durable CommitQC does not satisfy its frozen dual quorum "
        f"(validators={commit_validators}, signers={signer_count}/{commit_min}, "
        f"power={signed_power}/{commit_total_power})"
    )

pending = status.get("pending_persistence_id")
if pending is not None:
    require_uint(pending, "pending_persistence_id", positive=True)

operator = require_dict(status.get("operator"), "operator")
queues = require_dict(operator.get("adapter_queues"), "operator.adapter_queues")
for count_name, capacity_name in (
    ("ingress_keys", "ingress_capacity"),
    ("deferred_progress", "deferred_progress_capacity"),
    ("deferred_normal", "deferred_normal_capacity"),
):
    count = require_uint(queues.get(count_name), f"operator.adapter_queues.{count_name}")
    capacity = require_uint(
        queues.get(capacity_name),
        f"operator.adapter_queues.{capacity_name}",
        positive=True,
    )
    if count > capacity:
        raise SystemExit(f"operator adapter queue {count_name} exceeds its bound")

tx_queue = require_dict(operator.get("tx_queue"), "operator.tx_queue")
tracked = require_uint(tx_queue.get("tracked_transactions"), "operator.tx_queue.tracked_transactions")
queued = require_uint(tx_queue.get("queued_transactions"), "operator.tx_queue.queued_transactions")
capacity = require_uint(tx_queue.get("capacity"), "operator.tx_queue.capacity", positive=True)
require_uint(tx_queue.get("max_retained_bytes"), "operator.tx_queue.max_retained_bytes", positive=True)
if queued > tracked or tracked > capacity:
    raise SystemExit(
        f"operator transaction queue occupancy is impossible ({queued} <= {tracked} <= {capacity})"
    )

for field in (
    "lane_settlement_commitments",
    "lane_relay_envelopes",
    "lane_payload_ownerships",
    "committed_lane_blocks",
    "lane_block_sessions",
):
    if not isinstance(status.get(field), list):
        raise SystemExit(f"sumeragi/status omitted required {field} array")

print(
    json.dumps(
        {
            "height": height,
            "view": view,
            "phase": phase,
            "epoch": epoch,
            "commit_qc_height": committed_height,
            "commit_qc_signers": signer_count,
            "validator_count": validator_count,
            "tx_queue": f"{queued}/{capacity}",
        },
        sort_keys=True,
    )
)
PY
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
  if [[ -n "$ROLLOUT_CANARY_FAUCET_ASSET_ID" ]]; then
    bootstrap_cmd+=(--faucet-asset-id "$ROLLOUT_CANARY_FAUCET_ASSET_ID")
  fi
  if should_skip_canary_faucet; then
    bootstrap_cmd+=(--skip-faucet)
  fi

  echo "==> canary bootstrap: ${WRITE_CONFIG}" >&2
  "${bootstrap_cmd[@]}" >&2
}

prepare_write_canary_config() {
  local target_url="$1"

  [[ -n "$WRITE_CONFIG" ]] || WRITE_CONFIG="$(default_write_config_path)"
  if [[ $WRITE_CONFIG_EXPLICIT -eq 1 ]]; then
    [[ -f "$WRITE_CONFIG" ]] || {
      echo "write canary config not found: $WRITE_CONFIG" >&2
      exit 1
    }
    return 0
  fi

  ensure_write_canary_config "$target_url"
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

ensure_sorafs_manifest_builder_bin() {
  if [[ -n "$SORAFS_MANIFEST_BUILDER_BIN" ]]; then
    if [[ "$SORAFS_MANIFEST_BUILDER_BIN" == */* ]]; then
      [[ -x "$SORAFS_MANIFEST_BUILDER_BIN" ]] || {
        echo "sorafs_manifest_builder is not executable: $SORAFS_MANIFEST_BUILDER_BIN" >&2
        exit 1
      }
      SORAFS_MANIFEST_BUILDER_RUNNER=("$SORAFS_MANIFEST_BUILDER_BIN")
      return 0
    fi
    if command -v "$SORAFS_MANIFEST_BUILDER_BIN" >/dev/null 2>&1; then
      SORAFS_MANIFEST_BUILDER_RUNNER=("$SORAFS_MANIFEST_BUILDER_BIN")
      return 0
    fi
    echo "could not find sorafs_manifest_builder on PATH: $SORAFS_MANIFEST_BUILDER_BIN" >&2
    exit 1
  fi

  if [[ -x "${REPO_ROOT}/bin/sorafs_manifest_builder" ]]; then
    SORAFS_MANIFEST_BUILDER_RUNNER=("${REPO_ROOT}/bin/sorafs_manifest_builder")
  elif [[ -x "${REPO_ROOT}/target/debug/sorafs_manifest_builder" ]]; then
    SORAFS_MANIFEST_BUILDER_RUNNER=("${REPO_ROOT}/target/debug/sorafs_manifest_builder")
  elif [[ -x "${REPO_ROOT}/target/release/sorafs_manifest_builder" ]]; then
    SORAFS_MANIFEST_BUILDER_RUNNER=("${REPO_ROOT}/target/release/sorafs_manifest_builder")
  elif command -v cargo >/dev/null 2>&1; then
    SORAFS_MANIFEST_BUILDER_RUNNER=(
      cargo
      run
      --quiet
      --manifest-path
      "${REPO_ROOT}/Cargo.toml"
      -p
      sorafs_car
      --features
      cli
      --bin
      sorafs_manifest_builder
      --
    )
  else
    echo "could not find sorafs_manifest_builder or cargo fallback" >&2
    exit 1
  fi
}

ensure_sorafs_tx_stdin_builder_bin() {
  if [[ -n "$SORAFS_TX_STDIN_BUILDER_BIN" ]]; then
    if [[ "$SORAFS_TX_STDIN_BUILDER_BIN" == */* ]]; then
      [[ -x "$SORAFS_TX_STDIN_BUILDER_BIN" ]] || {
        echo "sorafs_tx_stdin_builder is not executable: $SORAFS_TX_STDIN_BUILDER_BIN" >&2
        exit 1
      }
      SORAFS_TX_STDIN_BUILDER_RUNNER=("$SORAFS_TX_STDIN_BUILDER_BIN")
      return 0
    fi
    if command -v "$SORAFS_TX_STDIN_BUILDER_BIN" >/dev/null 2>&1; then
      SORAFS_TX_STDIN_BUILDER_RUNNER=("$SORAFS_TX_STDIN_BUILDER_BIN")
      return 0
    fi
    echo "could not find sorafs_tx_stdin_builder on PATH: $SORAFS_TX_STDIN_BUILDER_BIN" >&2
    exit 1
  fi

  if [[ -x "${REPO_ROOT}/bin/sorafs_tx_stdin_builder" ]]; then
    SORAFS_TX_STDIN_BUILDER_RUNNER=("${REPO_ROOT}/bin/sorafs_tx_stdin_builder")
  elif [[ -x "${REPO_ROOT}/target/debug/sorafs_tx_stdin_builder" ]]; then
    SORAFS_TX_STDIN_BUILDER_RUNNER=("${REPO_ROOT}/target/debug/sorafs_tx_stdin_builder")
  elif [[ -x "${REPO_ROOT}/target/release/sorafs_tx_stdin_builder" ]]; then
    SORAFS_TX_STDIN_BUILDER_RUNNER=("${REPO_ROOT}/target/release/sorafs_tx_stdin_builder")
  elif command -v cargo >/dev/null 2>&1; then
    SORAFS_TX_STDIN_BUILDER_RUNNER=(
      cargo
      run
      --quiet
      --manifest-path
      "${REPO_ROOT}/Cargo.toml"
      -p
      sorafs_car
      --features
      cli
      --bin
      sorafs_tx_stdin_builder
      --
    )
  else
    echo "could not find sorafs_tx_stdin_builder or cargo fallback" >&2
    exit 1
  fi
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

current_block_height() {
  local root_url="$1"
  expect_status "status" GET "${root_url}/status" 200
  python3 - "$last_body" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)

blocks = payload.get("blocks")
if not isinstance(blocks, int) or blocks <= 0:
    raise SystemExit("status payload did not include a positive `blocks` value")
print(blocks)
PY
}

build_declaration_spec() {
  local account_id="$1"
  local provider_seed_prefix="$2"
  local declared_capacity_gib="$3"
  local stake_amount="$4"
  local current_blocks="$5"
  local valid_blocks="$6"
  local output_path="$7"

  python3 - "$account_id" "$provider_seed_prefix" "$declared_capacity_gib" "$stake_amount" "$current_blocks" "$valid_blocks" "$output_path" <<'PY'
import hashlib
import json
import sys
import time

account_id, provider_seed_prefix, declared_capacity_gib, stake_amount, current_blocks, valid_blocks, output_path = sys.argv[1:]
seed = f"{provider_seed_prefix}:{account_id}".encode("utf-8")
provider_id_hex = hashlib.sha256(seed).hexdigest()
pool_id_hex = hashlib.sha256((provider_id_hex + ":stake").encode("utf-8")).hexdigest()
now = int(time.time())
registered_epoch = int(current_blocks)
valid_until_epoch = registered_epoch + int(valid_blocks)

payload = {
    "provider_id_hex": provider_id_hex,
    "stake": {
        "pool_id_hex": pool_id_hex,
        "stake_amount": stake_amount,
    },
    "committed_capacity_gib": int(declared_capacity_gib),
    "chunker_commitments": [
        {
            "profile_handle": "sorafs.sf1@1.0.0",
            "committed_gib": int(declared_capacity_gib),
            "capability_refs": ["torii_gateway"],
        }
    ],
    "lane_commitments": [
        {
            "lane_id": "global",
            "max_gib": int(declared_capacity_gib),
        }
    ],
    "pricing": {
        "currency": "xor",
        "rate_per_gib_hour_milliu": 1,
        "min_commitment_hours": 1,
        "notes": "taira rollout capacity canary",
    },
    "valid_from": now,
    "valid_until": now + 86400,
    "metadata": {
        "sorafs.owner_account_id": account_id,
        "rollout.canary": "true",
    },
    "record_window": {
        "registered_epoch": registered_epoch,
        "valid_from_epoch": registered_epoch,
        "valid_until_epoch": valid_until_epoch,
    },
}

with open(output_path, "w", encoding="utf-8") as handle:
    json.dump(payload, handle, indent=2, sort_keys=True)
    handle.write("\n")
print(provider_id_hex)
PY
}

submit_capacity_canary() {
  local config_path="$1"
  local tx_stdin_path="$2"
  local output_file="$3"
  local submit_cmd=(
    "${IROHA_RUNNER[@]}"
    --output-format json
    -c "$config_path"
    --fee-payer sponsor
    --fee-program "$ROLLOUT_CANARY_FEE_PROGRAM_ID"
    --fee-program-revision "$ROLLOUT_CANARY_FEE_PROGRAM_REVISION"
  )
  submit_cmd+=(ledger transaction stdin)

  "${submit_cmd[@]}" \
    <"$tx_stdin_path" >"$output_file" 2>&1
}

run_write_canary() {
  local target_url="$1"
  ensure_iroha_bin
  ensure_sorafs_manifest_builder_bin
  ensure_sorafs_tx_stdin_builder_bin
  prepare_write_canary_config "$target_url"

  local temp_config work_dir summary_path tx_stdin_path spec_path output_file account_id current_blocks provider_id_hex
  temp_config="$(physical_path "$(mktemp)")"
  work_dir="$(physical_path "$(mktemp -d)")"
  output_file="$(physical_path "$(mktemp)")"
  trap 'rm -f "${temp_config:-}" "${output_file:-}"; rm -rf "${work_dir:-}"; cleanup' EXIT
  build_write_canary_config \
    "$WRITE_CONFIG" \
    "$target_url" \
    "$temp_config" \
    "$ROLLOUT_CANARY_TIME_TO_LIVE_MS" \
    "$ROLLOUT_CANARY_STATUS_TIMEOUT_MS"
  account_id="$(resolve_canary_account_id "$temp_config")"
  current_blocks="$(current_block_height "$target_url")"
  spec_path="${work_dir}/capacity_canary.spec.json"
  provider_id_hex="$(
    build_declaration_spec \
      "$account_id" \
      "$PROVIDER_SEED_PREFIX" \
      "$DECLARED_CAPACITY_GIB" \
      "$STAKE_AMOUNT" \
      "$current_blocks" \
      "$DECLARATION_VALID_BLOCKS" \
      "$spec_path"
  )"
  summary_path="${work_dir}/capacity_canary.summary.json"
  tx_stdin_path="${work_dir}/capacity_canary.tx.stdin.json"

  echo "==> SoraFS capacity canary: ${target_url} (provider ${provider_id_hex})"
  "${SORAFS_MANIFEST_BUILDER_RUNNER[@]}" \
    capacity declaration \
    "--spec=${spec_path}" \
    "--json-out=${summary_path}" \
    --quiet
  "${SORAFS_TX_STDIN_BUILDER_RUNNER[@]}" \
    capacity-declaration \
    "--summary=${summary_path}" \
    >"$tx_stdin_path"

  if ! submit_capacity_canary "$temp_config" "$tx_stdin_path" "$output_file"; then
    if grep -q 'Failed to find asset' "$output_file"; then
      claim_faucet_for_canary "$target_url" "$account_id" >/dev/null
      if ! submit_capacity_canary "$temp_config" "$tx_stdin_path" "$output_file"; then
        sed -n '1,120p' "$output_file" >&2 || true
        exit 1
      fi
    else
      if grep -Eq 'fee quote rejected|fee_payment_rejected' "$output_file"; then
        echo "SoraFS capacity canary failed: the exact fee quote was rejected; inspect the reported capacity and remediation, then re-quote against the active program revision" >&2
      fi
      if grep -q 'Unknown instruction type' "$output_file"; then
        echo "SoraFS capacity canary failed: the served validator binary is stale and missing SoraFS capacity/order instruction dispatch" >&2
      fi
      sed -n '1,120p' "$output_file" >&2 || true
      exit 1
    fi
  fi

  local attempt
  for ((attempt = 1; attempt <= CAPACITY_STATE_RECHECK_ATTEMPTS; attempt++)); do
    expect_status "capacity/state" GET "${target_url}/v1/sorafs/capacity/state" 200
    if python3 - "$last_body" "$provider_id_hex" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)

provider_id_hex = sys.argv[2].lower()
for entry in payload.get("declarations", []):
    if isinstance(entry, dict) and str(entry.get("provider_id_hex", "")).lower() == provider_id_hex:
        print(json.dumps(entry, indent=2, sort_keys=True))
        raise SystemExit(0)

raise SystemExit(1)
PY
    then
      return 0
    fi
    if [[ $attempt -lt CAPACITY_STATE_RECHECK_ATTEMPTS ]]; then
      sleep "$CAPACITY_STATE_RECHECK_DELAY_SECONDS"
    fi
  done

  echo "capacity canary transaction landed but the declaration never appeared in /v1/sorafs/capacity/state for provider ${provider_id_hex}" >&2
  exit 1
}

ROOT_URL="$(normalize_root_url "$PUBLIC_TORII_ROOT")"
probe_surface "$ROOT_URL"
check_node_health "$ROOT_URL"

if [[ $SKIP_WRITE_CANARY -eq 0 ]]; then
  run_write_canary "$ROOT_URL"
fi

echo "SoraFS rollout verification passed."

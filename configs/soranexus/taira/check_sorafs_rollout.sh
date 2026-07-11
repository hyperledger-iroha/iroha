#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/../../.." && pwd)"
PUBLIC_TORII_ROOT="${PUBLIC_TORII_ROOT:-}"
WRITE_CONFIG="${WRITE_CONFIG:-}"
WRITE_CONFIG_EXPLICIT=0
WRITE_CONFIG_DEFAULT="${WRITE_CONFIG_DEFAULT:-}"
IROHA_BIN="${IROHA_BIN:-}"
SORAFS_MANIFEST_STUB_BIN="${SORAFS_MANIFEST_STUB_BIN:-}"
SORAFS_TX_STDIN_BUILDER_BIN="${SORAFS_TX_STDIN_BUILDER_BIN:-}"
ROLLOUT_CANARY_ALIAS_PREFIX="${ROLLOUT_CANARY_ALIAS_PREFIX:-taira-rollout-canary}"
ROLLOUT_CANARY_TIME_TO_LIVE_MS="${ROLLOUT_CANARY_TIME_TO_LIVE_MS:-120000}"
ROLLOUT_CANARY_STATUS_TIMEOUT_MS="${ROLLOUT_CANARY_STATUS_TIMEOUT_MS:-120000}"
ROLLOUT_CANARY_GAS_ASSET_ID="${ROLLOUT_CANARY_GAS_ASSET_ID:-6TEAJqbb8oEPmLncoNiMRbLEK6tw}"
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
SORAFS_MANIFEST_STUB_RUNNER=()
SORAFS_TX_STDIN_BUILDER_RUNNER=()
CURL_RESOLVE_RULES=()
CURL_URL_RESOLVE_ARGS=()

usage() {
  cat <<'EOF'
Usage: check_sorafs_rollout.sh --public-root URL [--write-config PATH]
                               [--iroha-bin PATH]
                               [--sorafs-manifest-stub-bin PATH]
                               [--sorafs-tx-stdin-builder-bin PATH]
                               [--gas-asset-id ASSET_DEFINITION_ID]
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
  - POST /v1/sorafs/capacity/schedule returns HTTP 400 for an empty JSON body
  - GET /v1/sorafs/capacity/state returns HTTP 200
  - a deterministic capacity declaration lands through `iroha ledger transaction stdin`
  - the declaration is visible in /v1/sorafs/capacity/state

When `--write-config` is omitted, the script bootstraps a runtime-only canary
config automatically, preferring `/run/secrets/taira-canary-client.toml` when
that directory is writable and otherwise falling back to the local temp
directory. The bootstrap onboards a fresh ordinary account on Taira before
running the capacity declaration canary. When a gas asset is configured, the
bootstrap passes that asset to onboarding and skips faucet funding by default,
so the canary proves the sponsored-fee path directly. Set
`ROLLOUT_CANARY_SKIP_FAUCET=0` to require an initial faucet claim.
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
    --sorafs-manifest-stub-bin)
      [[ $# -ge 2 ]] || {
        echo "missing value for --sorafs-manifest-stub-bin" >&2
        exit 1
      }
      SORAFS_MANIFEST_STUB_BIN="$2"
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
    --gas-asset-id)
      [[ $# -ge 2 ]] || {
        echo "missing value for --gas-asset-id" >&2
        exit 1
      }
      ROLLOUT_CANARY_GAS_ASSET_ID="$2"
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

  http_request "$method" "$url" "$payload"
  if [[ "$last_status" != "$expected_status" ]]; then
    echo "${label}: expected HTTP ${expected_status}, got ${last_status}" >&2
    sed -n '1,120p' "$last_body" >&2 || true
    exit 1
  fi
}

probe_surface() {
  local root_url="$1"
  echo "==> SoraFS route surface: ${root_url}"
  expect_status "pin/register" POST "${root_url}/v1/sorafs/pin/register" 400 '{}'
  expect_status "capacity/declare" POST "${root_url}/v1/sorafs/capacity/declare" 400 '{}'
  expect_status "capacity/schedule" POST "${root_url}/v1/sorafs/capacity/schedule" 400 '{}'
  expect_status "capacity/state" GET "${root_url}/v1/sorafs/capacity/state" 200
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
import sys


def dig(obj, *path):
    cur = obj
    for key in path:
        if not isinstance(cur, dict):
            return None
        cur = cur.get(key)
    return cur


def first_int(*values):
    for value in values:
        if isinstance(value, bool):
            continue
        if isinstance(value, int):
            return value
    return None


with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)

commit_qc_height = first_int(
    payload.get("commit_qc_height"),
    dig(payload, "commit_qc", "height"),
)
highest_qc_height = first_int(
    payload.get("highest_qc_height"),
    dig(payload, "highest_qc", "height"),
    dig(payload, "canonical", "highest_qc", "height"),
)
locked_qc_height = first_int(
    payload.get("locked_qc_height"),
    dig(payload, "locked_qc", "height"),
    dig(payload, "canonical", "locked_qc", "height"),
)
canonical_height = first_int(
    payload.get("canonical_height"),
    dig(payload, "canonical", "height"),
)
canonical_phase = str(
    dig(payload, "canonical", "phase") or payload.get("canonical_phase") or ""
).strip().lower()
canonical_view = first_int(
    payload.get("canonical_view"),
    dig(payload, "canonical", "view"),
    dig(payload, "membership", "view"),
)
membership_height = first_int(
    payload.get("membership_height"),
    dig(payload, "membership", "height"),
)
worker_stage = str(
    dig(payload, "worker_loop", "stage") or payload.get("worker_stage") or ""
).strip().lower()
validator_set_len = first_int(
    payload.get("commit_qc_validator_set_len"),
    dig(payload, "commit_qc", "validator_set_len"),
)
tx_queue_depth = first_int(
    payload.get("tx_queue_depth"),
    dig(payload, "tx_queue", "depth"),
)
tx_queue_capacity = first_int(
    payload.get("tx_queue_capacity"),
    dig(payload, "tx_queue", "capacity"),
)
tx_queue_saturated_by_age = payload.get("tx_queue_saturated_by_age")
if not isinstance(tx_queue_saturated_by_age, bool):
    tx_queue_saturated_by_age = dig(payload, "tx_queue", "saturated_by_age")
if not isinstance(tx_queue_saturated_by_age, bool):
    tx_queue_saturated_by_age = None
tx_queue_oldest_queued_age_ms = first_int(
    payload.get("tx_queue_oldest_queued_age_ms"),
    dig(payload, "tx_queue", "oldest_queued_age_ms"),
)
view_change_last_cause = dig(payload, "view_change_causes", "last_cause")
canonical_rbc_status = str(
    dig(payload, "canonical", "rbc_status")
    or payload.get("canonical_rbc_status")
    or ""
).strip().lower()
canonical_pending_finality = (
    dig(payload, "canonical", "pending_finality")
    if isinstance(dig(payload, "canonical"), dict)
    else payload.get("canonical_pending_finality")
)
pending_rbc_sessions = first_int(
    payload.get("pending_rbc_sessions"),
    dig(payload, "pending_rbc", "sessions"),
)

if commit_qc_height is None or commit_qc_height < 1:
    raise SystemExit(
        f"sumeragi/status reported an unhealthy commit QC height: {commit_qc_height!r}"
    )
if highest_qc_height is not None and highest_qc_height < commit_qc_height:
    raise SystemExit(
        "sumeragi/status highest QC height "
        f"{highest_qc_height} is behind commit QC height {commit_qc_height}"
    )
if locked_qc_height is not None and locked_qc_height < commit_qc_height:
    raise SystemExit(
        "sumeragi/status locked QC height "
        f"{locked_qc_height} is behind commit QC height {commit_qc_height}"
    )
if canonical_height is not None and canonical_height < commit_qc_height:
    raise SystemExit(
        "sumeragi/status canonical height "
        f"{canonical_height} is behind commit QC height {commit_qc_height}"
    )
if validator_set_len is None or validator_set_len < 1:
    raise SystemExit(
        f"sumeragi/status reported an empty commit validator set: {validator_set_len!r}"
    )
if validator_set_len < 4:
    raise SystemExit(
        "sumeragi/status reported only "
        f"{validator_set_len} validators in the commit QC set; Taira rollout expects at least 4"
    )
if (
    membership_height is not None
    and commit_qc_height is not None
    and membership_height > commit_qc_height
):
    cause = view_change_last_cause or "unknown"
    pending_finality_present = canonical_pending_finality not in (None, False, "", "false", "0")
    rbc_waiting = canonical_rbc_status not in (
        "",
        "0",
        "false",
        "none",
        "null",
        "idle",
        "disabled",
        "ready",
        "complete",
        "completed",
        "delivered",
    )
    one_ahead_prepare = (
        canonical_phase == "prepare"
        and canonical_height == membership_height == commit_qc_height + 1
        and highest_qc_height == commit_qc_height
        and locked_qc_height == commit_qc_height
    )
    stalled_one_ahead_idle = (
        one_ahead_prepare
        and worker_stage == "idle"
        and canonical_view is not None
        and canonical_view > 1
    )
    if not (
        one_ahead_prepare
        and not stalled_one_ahead_idle
        and not pending_finality_present
        and not rbc_waiting
        and (pending_rbc_sessions in (None, 0))
        and cause not in ("missing_qc", "quorum_timeout", "stake_quorum_timeout")
        and tx_queue_saturated_by_age is not True
    ):
        raise SystemExit(
            "sumeragi/status reports a finality fault "
            f"({cause}) with membership height ahead of commit QC "
            f"({membership_height} > {commit_qc_height}); "
            f"queue depth={tx_queue_depth!r}, capacity={tx_queue_capacity!r}, "
            f"saturated_by_age={tx_queue_saturated_by_age!r}, "
            f"oldest_queued_age_ms={tx_queue_oldest_queued_age_ms!r}, "
            f"phase={canonical_phase!r}, worker_stage={worker_stage!r}, "
            f"canonical_view={canonical_view!r}, "
            f"pending_finality={canonical_pending_finality!r}, "
            f"rbc_status={canonical_rbc_status!r}, "
            f"pending_rbc_sessions={pending_rbc_sessions!r}"
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
  if [[ -n "$ROLLOUT_CANARY_GAS_ASSET_ID" ]]; then
    bootstrap_cmd+=(--gas-asset-id "$ROLLOUT_CANARY_GAS_ASSET_ID")
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

ensure_sorafs_manifest_stub_bin() {
  if [[ -n "$SORAFS_MANIFEST_STUB_BIN" ]]; then
    if [[ "$SORAFS_MANIFEST_STUB_BIN" == */* ]]; then
      [[ -x "$SORAFS_MANIFEST_STUB_BIN" ]] || {
        echo "sorafs_manifest_stub is not executable: $SORAFS_MANIFEST_STUB_BIN" >&2
        exit 1
      }
      SORAFS_MANIFEST_STUB_RUNNER=("$SORAFS_MANIFEST_STUB_BIN")
      return 0
    fi
    if command -v "$SORAFS_MANIFEST_STUB_BIN" >/dev/null 2>&1; then
      SORAFS_MANIFEST_STUB_RUNNER=("$SORAFS_MANIFEST_STUB_BIN")
      return 0
    fi
    echo "could not find sorafs_manifest_stub on PATH: $SORAFS_MANIFEST_STUB_BIN" >&2
    exit 1
  fi

  if [[ -x "${REPO_ROOT}/bin/sorafs_manifest_stub" ]]; then
    SORAFS_MANIFEST_STUB_RUNNER=("${REPO_ROOT}/bin/sorafs_manifest_stub")
  elif [[ -x "${REPO_ROOT}/target/debug/sorafs_manifest_stub" ]]; then
    SORAFS_MANIFEST_STUB_RUNNER=("${REPO_ROOT}/target/debug/sorafs_manifest_stub")
  elif [[ -x "${REPO_ROOT}/target/release/sorafs_manifest_stub" ]]; then
    SORAFS_MANIFEST_STUB_RUNNER=("${REPO_ROOT}/target/release/sorafs_manifest_stub")
  elif command -v cargo >/dev/null 2>&1; then
    SORAFS_MANIFEST_STUB_RUNNER=(
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
      sorafs_manifest_stub
      --
    )
  else
    echo "could not find sorafs_manifest_stub or cargo fallback" >&2
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

resolve_canary_private_key() {
  local config_path="$1"
  python3 - "$config_path" <<'PY'
import sys

try:
    import tomllib
except ModuleNotFoundError:
    import tomli as tomllib

with open(sys.argv[1], "rb") as handle:
    source = tomllib.load(handle)

private_key = (source.get("account") or {}).get("private_key")
if not isinstance(private_key, str) or not private_key:
    raise SystemExit("write canary config is missing `account.private_key`")
print(private_key)
PY
}

claim_faucet_for_canary() {
  local target_url="$1"
  local account_id="$2"
  echo "==> faucet bootstrap: ${account_id}" >&2
  python3 "${REPO_ROOT}/scripts/taira_faucet_canary.py" \
    --account-id "$account_id" \
    --torii-root "$target_url"
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
  local metadata_file="${4:-}"
  local submit_cmd=("${IROHA_RUNNER[@]}" --output-format json -c "$config_path")

  if [[ -n "$metadata_file" ]]; then
    submit_cmd+=(-m "$metadata_file")
  fi
  submit_cmd+=(ledger transaction stdin)

  "${submit_cmd[@]}" \
    <"$tx_stdin_path" >"$output_file" 2>&1
}

run_write_canary() {
  local target_url="$1"
  ensure_iroha_bin
  ensure_sorafs_manifest_stub_bin
  ensure_sorafs_tx_stdin_builder_bin
  prepare_write_canary_config "$target_url"

  local temp_config work_dir request_path tx_stdin_path spec_path output_file metadata_file private_key_file account_id private_key current_blocks provider_id_hex
  temp_config="$(physical_path "$(mktemp)")"
  work_dir="$(physical_path "$(mktemp -d)")"
  output_file="$(physical_path "$(mktemp)")"
  metadata_file="$(physical_path "$(mktemp)")"
  private_key_file="$(physical_path "$(mktemp)")"
  trap 'rm -f "${temp_config:-}" "${metadata_file:-}" "${private_key_file:-}" "${output_file:-}"; rm -rf "${work_dir:-}"; cleanup' EXIT
  build_write_canary_config \
    "$WRITE_CONFIG" \
    "$target_url" \
    "$temp_config" \
    "$ROLLOUT_CANARY_TIME_TO_LIVE_MS" \
    "$ROLLOUT_CANARY_STATUS_TIMEOUT_MS"
  if [[ -n "$ROLLOUT_CANARY_GAS_ASSET_ID" ]]; then
    write_canary_metadata_file "$metadata_file" "$ROLLOUT_CANARY_GAS_ASSET_ID"
  else
    rm -f "$metadata_file"
    metadata_file=""
  fi

  account_id="$(resolve_canary_account_id "$temp_config")"
  private_key="$(resolve_canary_private_key "$temp_config")"
  local previous_umask
  previous_umask="$(umask)"
  umask 077
  printf '%s\n' "$private_key" >"$private_key_file"
  umask "$previous_umask"
  unset private_key
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
  request_path="${work_dir}/capacity_canary.request.json"
  tx_stdin_path="${work_dir}/capacity_canary.tx.stdin.json"

  echo "==> SoraFS capacity canary: ${target_url} (provider ${provider_id_hex})"
  "${SORAFS_MANIFEST_STUB_RUNNER[@]}" \
    capacity declaration \
    "--spec=${spec_path}" \
    "--request-out=${request_path}" \
    "--authority=${account_id}" \
    "--private-key-file=${private_key_file}" \
    --quiet
  "${SORAFS_TX_STDIN_BUILDER_RUNNER[@]}" \
    capacity-declaration-request \
    "--request=${request_path}" \
    >"$tx_stdin_path"

  if ! submit_capacity_canary "$temp_config" "$tx_stdin_path" "$output_file" "$metadata_file"; then
    if grep -q 'Failed to find asset' "$output_file"; then
      claim_faucet_for_canary "$target_url" "$account_id" >/dev/null
      if ! submit_capacity_canary "$temp_config" "$tx_stdin_path" "$output_file" "$metadata_file"; then
        sed -n '1,120p' "$output_file" >&2 || true
        exit 1
      fi
    else
      if grep -q 'missing gas_asset_id' "$output_file"; then
        echo "SoraFS capacity canary failed: Taira requires gas_asset_id transaction metadata; pass --gas-asset-id with an accepted asset definition id" >&2
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

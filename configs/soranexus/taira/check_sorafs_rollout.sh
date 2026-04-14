#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/../../.." && pwd)"
PUBLIC_TORII_ROOT="${PUBLIC_TORII_ROOT:-}"
WRITE_CONFIG="${WRITE_CONFIG:-}"
IROHA_BIN="${IROHA_BIN:-}"
SORAFS_MANIFEST_STUB_BIN="${SORAFS_MANIFEST_STUB_BIN:-}"
SORAFS_TX_STDIN_BUILDER_BIN="${SORAFS_TX_STDIN_BUILDER_BIN:-}"
DECLARED_CAPACITY_GIB="${DECLARED_CAPACITY_GIB:-1}"
STAKE_AMOUNT="${STAKE_AMOUNT:-1}"
DECLARATION_VALID_BLOCKS="${DECLARATION_VALID_BLOCKS:-10000}"
PROVIDER_SEED_PREFIX="${PROVIDER_SEED_PREFIX:-taira-rollout-capacity-canary:v1}"
SKIP_WRITE_CANARY=0
IROHA_RUNNER=()
SORAFS_MANIFEST_STUB_RUNNER=()
SORAFS_TX_STDIN_BUILDER_RUNNER=()

usage() {
  cat <<'EOF'
Usage: check_sorafs_rollout.sh --public-root URL [--write-config PATH]
                               [--iroha-bin PATH]
                               [--sorafs-manifest-stub-bin PATH]
                               [--sorafs-tx-stdin-builder-bin PATH]
                               [--declared-capacity-gib N]
                               [--stake-amount N]
                               [--declaration-valid-blocks N]
                               [--provider-seed-prefix TEXT]
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

Without --write-config, signed canary checks are rejected unless
--skip-write-canary is provided explicitly for read-only validation.
EOF
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

if [[ -z "$WRITE_CONFIG" && $SKIP_WRITE_CANARY -eq 0 ]]; then
  echo "signed SoraFS rollout validation requires --write-config; use --skip-write-canary only for read-only checks" >&2
  exit 1
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

http_request() {
  local method="$1"
  local url="$2"
  local payload="${3:-}"
  local body_file

  body_file="$(mktemp)"
  cleanup
  last_body="$body_file"

  if [[ "$method" == "GET" ]]; then
    last_status="$(
      curl \
        --silent \
        --show-error \
        --output "$body_file" \
        --write-out '%{http_code}' \
        "$url"
    )"
  else
    last_status="$(
      curl \
        --silent \
        --show-error \
        --output "$body_file" \
        --write-out '%{http_code}' \
        --header 'content-type: application/json' \
        --request "$method" \
        --data "$payload" \
        "$url"
    )"
  fi
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
chain_discriminant = account.get("chain_discriminant")
domain = account.get("domain", "wonderland.universal")
basic_auth = source.get("basic_auth")
transaction = source.get("transaction") or {}
time_to_live_ms = transaction.get("time_to_live_ms", 120000)
status_timeout_ms = transaction.get("status_timeout_ms", 120000)
nonce = transaction.get("nonce", False)

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
if not isinstance(time_to_live_ms, int):
    raise SystemExit("write canary config `transaction.time_to_live_ms` must be an integer when present")
if not isinstance(status_timeout_ms, int):
    raise SystemExit("write canary config `transaction.status_timeout_ms` must be an integer when present")
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

  "${IROHA_RUNNER[@]}" --output-format json -c "$config_path" ledger transaction stdin \
    <"$tx_stdin_path" >"$output_file" 2>&1
}

run_write_canary() {
  local target_url="$1"
  ensure_iroha_bin
  ensure_sorafs_manifest_stub_bin
  ensure_sorafs_tx_stdin_builder_bin
  [[ -f "$WRITE_CONFIG" ]] || {
    echo "write canary config does not exist: $WRITE_CONFIG" >&2
    exit 1
  }

  local temp_config work_dir request_path tx_stdin_path spec_path output_file account_id private_key current_blocks provider_id_hex
  temp_config="$(mktemp)"
  work_dir="$(mktemp -d)"
  output_file="$(mktemp)"
  trap 'rm -f "$temp_config" "$output_file"; rm -rf "$work_dir"; cleanup' EXIT
  build_write_canary_config "$WRITE_CONFIG" "$target_url" "$temp_config"

  account_id="$(resolve_canary_account_id "$temp_config")"
  private_key="$(resolve_canary_private_key "$temp_config")"
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
    "--private-key=${private_key}" \
    --quiet
  "${SORAFS_TX_STDIN_BUILDER_RUNNER[@]}" \
    capacity-declaration-request \
    "--request=${request_path}" \
    >"$tx_stdin_path"

  if ! submit_capacity_canary "$temp_config" "$tx_stdin_path" "$output_file"; then
    if grep -q 'Failed to find asset' "$output_file"; then
      claim_faucet_for_canary "$target_url" "$account_id" >/dev/null
      if ! submit_capacity_canary "$temp_config" "$tx_stdin_path" "$output_file"; then
        sed -n '1,120p' "$output_file" >&2 || true
        exit 1
      fi
    else
      if grep -q 'Unknown instruction type' "$output_file"; then
        echo "SoraFS capacity canary failed: the served validator binary is stale and missing SoraFS capacity/order instruction dispatch" >&2
      fi
      sed -n '1,120p' "$output_file" >&2 || true
      exit 1
    fi
  fi

  local attempt
  for ((attempt = 1; attempt <= 10; attempt++)); do
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
    if [[ $attempt -lt 10 ]]; then
      sleep 2
    fi
  done

  echo "capacity canary transaction landed but the declaration never appeared in /v1/sorafs/capacity/state for provider ${provider_id_hex}" >&2
  exit 1
}

ROOT_URL="$(normalize_root_url "$PUBLIC_TORII_ROOT")"
probe_surface "$ROOT_URL"

if [[ $SKIP_WRITE_CANARY -eq 0 ]]; then
  run_write_canary "$ROOT_URL"
fi

echo "SoraFS rollout verification passed."

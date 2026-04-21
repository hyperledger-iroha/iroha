#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
LOCALNET_DIR="${IROHA_TAIRA_LOCALNET_DIR:-$ROOT_DIR/dist/taira-localnet}"
SCREEN_SESSION="${IROHA_TAIRA_SCREEN_SESSION:-taira-localnet}"
GENESIS_JSON="${IROHA_TAIRA_GENESIS_JSON:-$LOCALNET_DIR/genesis.json}"
GENESIS_SIGNED="${IROHA_TAIRA_GENESIS_SIGNED:-$LOCALNET_DIR/genesis.signed.nrt}"
GENESIS_PRIVATE_KEY="${IROHA_TAIRA_GENESIS_PRIVATE_KEY:-}"
LOCALNET_BASE_SEED_OVERRIDE="${IROHA_TAIRA_LOCALNET_SEED:-}"
TAIRA_PROFILE_CONFIG="${IROHA_TAIRA_PROFILE_CONFIG:-$ROOT_DIR/configs/soranexus/taira/config.toml}"
TAIRA_SECRETS_FILE="${IROHA_TAIRA_SECRETS_FILE:-$ROOT_DIR/configs/soranexus/taira/validator_secrets.local.toml}"
CALL_DOMAIN="${IROHA_TAIRA_KAIGI_CALL_DOMAIN:-wonderland.universal}"
CALL_NAME="${IROHA_TAIRA_KAIGI_CALL_NAME:-taira-relay-bootstrap}"
REPORTED_AT_MS="${IROHA_TAIRA_KAIGI_REPORTED_AT_MS:-1890864000000}"
RELAY_DOMAIN="${IROHA_TAIRA_KAIGI_RELAY_DOMAIN:-nexus.universal}"
BOOTSTRAP_AUTHORITY_DOMAIN="${IROHA_TAIRA_KAIGI_BOOTSTRAP_AUTHORITY_DOMAIN:-nexus.universal}"
KAIGI_HELPER_BIN="${IROHA_TAIRA_KAIGI_HELPER_BIN:-}"
DPN_DATASPACE_ID="${IROHA_TAIRA_DPN_DATASPACE_ID:-10}"
DPN_ACCOUNT_DOMAIN="${IROHA_TAIRA_DPN_ACCOUNT_DOMAIN:-wonderland.is}"
DPN_SPONSOR_ACCOUNT_ID="${IROHA_TAIRA_DPN_SPONSOR_ACCOUNT_ID:-}"
DPN_SPONSOR_FUND_AMOUNT="${IROHA_TAIRA_DPN_SPONSOR_FUND_AMOUNT:-1000}"

need_file() {
  if [[ ! -f "$1" ]]; then
    echo "missing required file: $1" >&2
    exit 1
  fi
}

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    exit 1
  fi
}

discover_peer_configs() {
  PEER_CONFIGS=()
  local path
  while IFS= read -r path; do
    [[ -n "$path" ]] || continue
    PEER_CONFIGS+=("$path")
  done < <(find "$LOCALNET_DIR" -maxdepth 1 -type f -name 'peer*.toml' | sort)
  if [[ "${#PEER_CONFIGS[@]}" -eq 0 ]]; then
    echo "no peer configs found under $LOCALNET_DIR" >&2
    exit 1
  fi
}

relay_hpke_key_for_index() {
  local index="$1"
  python3 - "$index" <<'PY'
import base64
import hashlib
import sys

index = sys.argv[1]
digest = hashlib.sha256(f"taira-relay-{index}".encode("utf-8")).digest()
print(base64.b64encode(digest).decode("ascii"))
PY
}

relay_bandwidth_class_for_index() {
  local index="$1"
  case "$index" in
    0) echo 3 ;;
    1) echo 2 ;;
    *) echo 1 ;;
  esac
}

load_taira_authority() {
  if [[ -n "${IROHA_TAIRA_AUTHORITY:-}" && -n "${IROHA_TAIRA_AUTHORITY_PRIVATE_KEY:-}" && -n "${IROHA_TAIRA_TORII_MAX_CONTENT_LEN:-}" ]]; then
    TAIRA_TORII_MAX_CONTENT_LEN="$IROHA_TAIRA_TORII_MAX_CONTENT_LEN"
    TAIRA_STORAGE_PIN_MAX_EVENTS="${IROHA_TAIRA_STORAGE_PIN_MAX_EVENTS:-64}"
    TAIRA_STORAGE_PIN_WINDOW_SECS="${IROHA_TAIRA_STORAGE_PIN_WINDOW_SECS:-3600}"
    TAIRA_AUTHORITY="$IROHA_TAIRA_AUTHORITY"
    TAIRA_AUTHORITY_PRIVATE_KEY="$IROHA_TAIRA_AUTHORITY_PRIVATE_KEY"
    return
  fi

  local values=()
  local line
  while IFS= read -r line; do
    values+=("$line")
  done < <(
    python3 - "$TAIRA_PROFILE_CONFIG" "$TAIRA_SECRETS_FILE" <<'PY'
import sys
try:
    import tomllib
except ModuleNotFoundError:
    import tomli as tomllib

path = sys.argv[1]
secrets_path = sys.argv[2]
with open(path, "rb") as fh:
    cfg = tomllib.load(fh)
shared = {}
if secrets_path:
    try:
        with open(secrets_path, "rb") as fh:
            secrets = tomllib.load(fh)
    except FileNotFoundError:
        secrets = {}
    shared = secrets.get("shared", {})
print(cfg["torii"]["max_content_len"])
quota = cfg.get("sorafs", {}).get("quota", {})
print(quota.get("storage_pin_max_events", 64))
print(quota.get("storage_pin_window_secs", 3600))
print(shared.get("torii_onboarding_authority", ""))
print(shared.get("torii_onboarding_private_key", ""))
PY
  )
  if [[ "${#values[@]}" -lt 5 ]]; then
    echo "failed to load Taira max_content_len/quota/shared local bootstrap signer defaults from $TAIRA_PROFILE_CONFIG" >&2
    exit 1
  fi
  TAIRA_TORII_MAX_CONTENT_LEN="${IROHA_TAIRA_TORII_MAX_CONTENT_LEN:-${values[0]}}"
  TAIRA_STORAGE_PIN_MAX_EVENTS="${IROHA_TAIRA_STORAGE_PIN_MAX_EVENTS:-${values[1]}}"
  TAIRA_STORAGE_PIN_WINDOW_SECS="${IROHA_TAIRA_STORAGE_PIN_WINDOW_SECS:-${values[2]}}"
  TAIRA_AUTHORITY="${IROHA_TAIRA_AUTHORITY:-${values[3]}}"
  TAIRA_AUTHORITY_PRIVATE_KEY="${IROHA_TAIRA_AUTHORITY_PRIVATE_KEY:-${values[4]}}"
  if [[ -z "$TAIRA_AUTHORITY" || -z "$TAIRA_AUTHORITY_PRIVATE_KEY" ]]; then
    echo "set IROHA_TAIRA_AUTHORITY and IROHA_TAIRA_AUTHORITY_PRIVATE_KEY or provide [shared] torii_onboarding_* values in $TAIRA_SECRETS_FILE; local bootstrap reuses that shared signer for the served faucet too" >&2
    exit 1
  fi
}

load_localnet_genesis_signer_config() {
  local values=()
  local line
  while IFS= read -r line; do
    values+=("$line")
  done < <(
    LOCALNET_BASE_SEED_OVERRIDE="$LOCALNET_BASE_SEED_OVERRIDE" \
      python3 - "$LOCALNET_DIR/peer0.toml" "$LOCALNET_DIR/README.md" <<'PY'
import os
import re
import sys

peer_cfg = open(sys.argv[1], encoding='utf-8').read()
match = re.search(r'(?ms)^\[genesis\]\n.*?^public_key\s*=\s*"([^"]+)"', peer_cfg)
if not match:
    raise SystemExit("missing genesis public_key")
print(match.group(1))
seed = os.environ.get('LOCALNET_BASE_SEED_OVERRIDE', '')
if not seed:
    try:
        readme = open(sys.argv[2], encoding='utf-8').read()
    except FileNotFoundError:
        readme = ''
    match = re.search(r'^- Base seed: `([^`]+)`$', readme, re.MULTILINE)
    if match:
        seed = match.group(1)
print(seed)
PY
  )
  if [[ "${#values[@]}" -lt 2 ]]; then
    echo "failed to load localnet genesis signer config from $LOCALNET_DIR" >&2
    exit 1
  fi
  LOCALNET_GENESIS_PUBLIC_KEY="${values[0]}"
  LOCALNET_BASE_SEED="${values[1]}"
}

patch_peer_configs_for_taira_authority() {
  local fee_asset_id="$1"
  TAIRA_AUTHORITY="$TAIRA_AUTHORITY" \
  TAIRA_AUTHORITY_PRIVATE_KEY="$TAIRA_AUTHORITY_PRIVATE_KEY" \
  TAIRA_TORII_MAX_CONTENT_LEN="$TAIRA_TORII_MAX_CONTENT_LEN" \
  TAIRA_STORAGE_PIN_MAX_EVENTS="$TAIRA_STORAGE_PIN_MAX_EVENTS" \
  TAIRA_STORAGE_PIN_WINDOW_SECS="$TAIRA_STORAGE_PIN_WINDOW_SECS" \
  TAIRA_FEE_ASSET_ID="$fee_asset_id" \
  LOCALNET_DIR="$LOCALNET_DIR" \
  python3 <<'PY'
from pathlib import Path
import os
import re

localnet_dir = Path(os.environ["LOCALNET_DIR"])
authority = os.environ["TAIRA_AUTHORITY"]
private_key = os.environ["TAIRA_AUTHORITY_PRIVATE_KEY"]
max_content_len = os.environ["TAIRA_TORII_MAX_CONTENT_LEN"]
storage_pin_max_events = os.environ["TAIRA_STORAGE_PIN_MAX_EVENTS"]
storage_pin_window_secs = os.environ["TAIRA_STORAGE_PIN_WINDOW_SECS"]
fee_asset_id = os.environ["TAIRA_FEE_ASSET_ID"]

onboarding_block = f"""[torii.onboarding]
enabled = true
authority = "{authority}"
private_key = "{private_key}"
allowed_permissions = []
"""

# Local bootstrap intentionally reuses the onboarding signer as the served faucet authority.
faucet_block = f"""[torii.faucet]
enabled = true
authority = "{authority}"
private_key = "{private_key}"
asset_definition_id = "{fee_asset_id}"
amount = "25000"
pow_difficulty_bits = 8
pow_scrypt_log_n = 13
pow_scrypt_r = 8
pow_scrypt_p = 1
pow_max_anchor_age_blocks = 6
pow_adaptive_lookback_blocks = 64
pow_adaptive_claims_per_extra_bit = 4
pow_adaptive_max_extra_bits = 2
pow_vrf_seed_enabled = false
"""

quota_block = f"""[sorafs.quota]
storage_pin_max_events = {storage_pin_max_events}
storage_pin_window_secs = {storage_pin_window_secs}
"""

def replace_or_insert(text: str, section: str, block: str) -> str:
    pattern = re.compile(rf"(?ms)^\[{re.escape(section)}\]\n.*?(?=^\[|\Z)")
    replacement = block.rstrip() + "\n\n"
    if pattern.search(text):
        return pattern.sub(replacement, text, count=1)

    anchor = re.search(r"(?ms)^\[torii\.mcp\]\n.*?(?=^\[|\Z)", text)
    if anchor:
        return text[: anchor.end()] + "\n" + replacement + text[anchor.end() :]
    return text.rstrip() + "\n\n" + replacement

def ensure_torii_max_content_len(text: str) -> str:
    section = re.compile(r"(?ms)^\[torii\]\n.*?(?=^\[|\Z)")
    match = section.search(text)
    if not match:
        return text

    block = match.group(0)
    if re.search(r"(?m)^max_content_len\s*=", block):
        updated = re.sub(
            r"(?m)^max_content_len\s*=.*$",
            f"max_content_len = {max_content_len}",
            block,
            count=1,
        )
    else:
        lines = block.splitlines()
        lines.insert(1, f"max_content_len = {max_content_len}")
        updated = "\n".join(lines)

    return text[: match.start()] + updated + text[match.end() :]

for path in sorted(localnet_dir.glob("peer*.toml")):
    text = path.read_text()
    text = ensure_torii_max_content_len(text)
    text = replace_or_insert(text, "sorafs.quota", quota_block)
    text = replace_or_insert(text, "torii.onboarding", onboarding_block)
    text = replace_or_insert(text, "torii.faucet", faucet_block)
    path.write_text(text)
PY
}

ensure_private_dataspace_onboarding_permissions() {
  GENESIS_JSON="$GENESIS_JSON" \
  TAIRA_AUTHORITY="$TAIRA_AUTHORITY" \
  DPN_DATASPACE_ID="$DPN_DATASPACE_ID" \
  python3 <<'PY'
from pathlib import Path
import json
import os

path = Path(os.environ["GENESIS_JSON"])
authority = os.environ["TAIRA_AUTHORITY"]
dataspace_id = int(os.environ["DPN_DATASPACE_ID"])

payload_alias = {"scope": {"scope": "dataspace", "value": dataspace_id}}
payload_manifest = {"dataspace": dataspace_id}

with path.open(encoding="utf-8") as fh:
    genesis = json.load(fh)

has_alias_permission = False
has_manifest_permission = False
for tx in genesis.get("transactions", []):
    for instruction in tx.get("instructions", []):
        if not isinstance(instruction, dict):
            continue
        permission_grant = instruction.get("Grant", {}).get("Permission")
        if not isinstance(permission_grant, dict):
            continue
        if permission_grant.get("destination") != authority:
            continue
        obj = permission_grant.get("object")
        if not isinstance(obj, dict):
            continue
        if obj.get("name") == "CanManageAccountAlias" and obj.get("payload") == payload_alias:
            has_alias_permission = True
        if (
            obj.get("name") == "CanPublishSpaceDirectoryManifest"
            and obj.get("payload") == payload_manifest
        ):
            has_manifest_permission = True

instructions = []
if not has_alias_permission:
    instructions.append(
        {
            "Grant": {
                "Permission": {
                    "destination": authority,
                    "object": {
                        "name": "CanManageAccountAlias",
                        "payload": payload_alias,
                    },
                }
            }
        }
    )
if not has_manifest_permission:
    instructions.append(
        {
            "Grant": {
                "Permission": {
                    "destination": authority,
                    "object": {
                        "name": "CanPublishSpaceDirectoryManifest",
                        "payload": payload_manifest,
                    },
                }
            }
        }
    )

if not instructions:
    print("private dataspace onboarding permissions already present")
else:
    genesis.setdefault("transactions", []).append(
        {
            "instructions": instructions,
            "ivm_triggers": [],
            "topology": [],
        }
    )
    path.write_text(json.dumps(genesis, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(
        f"seeded private dataspace onboarding permissions for dataspace {dataspace_id}"
    )
PY
}

ensure_dpn_genesis_seed_state() {
  if [[ -z "$DPN_SPONSOR_ACCOUNT_ID" ]]; then
    echo "no DPN sponsor account configured; skipping DPN genesis seeding"
    return 0
  fi

  GENESIS_JSON="$GENESIS_JSON" \
  DPN_ACCOUNT_DOMAIN="$DPN_ACCOUNT_DOMAIN" \
  DPN_SPONSOR_ACCOUNT_ID="$DPN_SPONSOR_ACCOUNT_ID" \
  DPN_SPONSOR_FUND_AMOUNT="$DPN_SPONSOR_FUND_AMOUNT" \
  TAIRA_FEE_ASSET_ID="$1" \
  python3 <<'PY'
from pathlib import Path
import json
import os

path = Path(os.environ["GENESIS_JSON"])
domain_id = os.environ["DPN_ACCOUNT_DOMAIN"]
sponsor_account_id = os.environ["DPN_SPONSOR_ACCOUNT_ID"]
fund_amount = str(int(os.environ["DPN_SPONSOR_FUND_AMOUNT"]))
fee_asset_id = os.environ["TAIRA_FEE_ASSET_ID"]
target_balance = f"{fee_asset_id}#{sponsor_account_id}"

with path.open(encoding="utf-8") as fh:
    genesis = json.load(fh)

has_domain = False
has_account = False
has_funding = False
for tx in genesis.get("transactions", []):
    for instruction in tx.get("instructions", []):
        if not isinstance(instruction, dict):
            continue
        register = instruction.get("Register")
        if isinstance(register, dict):
            domain = register.get("Domain")
            if isinstance(domain, dict) and domain.get("id") == domain_id:
                has_domain = True
            account = register.get("Account")
            if isinstance(account, dict) and account.get("id") == sponsor_account_id:
                has_account = True

        mint = instruction.get("Mint", {}).get("Asset")
        if isinstance(mint, dict) and mint.get("destination") == target_balance:
            has_funding = True

instructions = []
if not has_domain:
    instructions.append(
        {
            "Register": {
                "Domain": {
                    "id": domain_id,
                    "logo": None,
                    "metadata": {},
                }
            }
        }
    )
if not has_account:
    instructions.append(
        {
            "Register": {
                "Account": {
                    "id": sponsor_account_id,
                    "metadata": {},
                }
            }
        }
    )
if not has_funding:
    instructions.append(
        {
            "Mint": {
                "Asset": {
                    "destination": target_balance,
                    "object": fund_amount,
                }
            }
        }
    )

if not instructions:
    print("DPN domain/account/funding already present in genesis")
else:
    genesis.setdefault("transactions", []).append(
        {
            "instructions": instructions,
            "ivm_triggers": [],
            "topology": [],
        }
    )
    path.write_text(json.dumps(genesis, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(
        "seeded DPN genesis state "
        f"(domain={domain_id}, sponsor={sponsor_account_id}, amount={fund_amount})"
    )
PY
}

ensure_launchd_runner() {
  local runner="$LOCALNET_DIR/launchd-run.sh"
  if [[ -f "$runner" ]]; then
    return 0
  fi
  if [[ ! -f "$LOCALNET_DIR/start.sh" ]]; then
    echo "missing required file: $runner" >&2
    exit 1
  fi
  cat >"$runner" <<EOF
#!/bin/zsh
set -euo pipefail
setopt null_glob

DIR=\$(cd "\$(dirname "\$0")" && pwd)
IROHAD_BIN="\${IROHAD_BIN:-$ROOT_DIR/target/release/irohad}"
SITE_BINDINGS_FILE="\${IROHA_SORAFS_SITE_BINDINGS_FILE:-$ROOT_DIR/configs/soranexus/taira/sorafs_sites.json}"

if [[ ! -x "\$IROHAD_BIN" ]]; then
  echo "irohad binary not executable: \$IROHAD_BIN" >&2
  exit 1
fi

cleanup() {
  local pidfile pid
  for pidfile in "\$DIR"/peer*.pid; do
    [[ -f "\$pidfile" ]] || continue
    pid=\$(<"\$pidfile")
    [[ -n "\$pid" ]] || continue
    kill "\$pid" 2>/dev/null || true
  done
}

for pid in \${(f)"\$(pgrep -f "\$DIR/peer[0-3]\\\\.toml" 2>/dev/null || true)"}; do
  [[ -n "\$pid" ]] || continue
  kill "\$pid" 2>/dev/null || true
done

rm -f "\$DIR"/peer*.pid
trap cleanup HUP INT TERM EXIT

for i in 0 1 2 3; do
  snapshot_dir="\$DIR/storage/peer\${i}/snapshot"
  mkdir -p "\$snapshot_dir"
  launch_env=(SNAPSHOT_STORE_DIR="\$snapshot_dir" RUST_LOG="\${RUST_LOG:-info}")
  if [[ -f "\$SITE_BINDINGS_FILE" ]]; then
    launch_env+=(IROHA_SORAFS_SITE_BINDINGS_FILE="\$SITE_BINDINGS_FILE")
  fi
  env "\${launch_env[@]}" \\
    "\$IROHAD_BIN" --sora --config "\$DIR/peer\${i}.toml" >> "\$DIR/peer\${i}.log" 2>&1 &
  pid=\$!
  print -r -- "\$pid" > "\$DIR/peer\${i}.pid"
  echo "peer\$i pid \$pid"
done

while true; do
  for pidfile in "\$DIR"/peer*.pid; do
    [[ -f "\$pidfile" ]] || continue
    pid=\$(<"\$pidfile")
    if ! kill -0 "\$pid" 2>/dev/null; then
      echo "peer from \${pidfile:t} exited" >&2
      exit 1
    fi
  done
  sleep 1
done
EOF
  chmod +x "$runner"
}

extract_toml_string() {
  local key="$1"
  local file="$2"
  awk -F'"' -v wanted="$key" '$1 ~ "^[[:space:]]*" wanted "[[:space:]]*=" { print $2; exit }' "$file"
}

helper_supports_cli() {
  local candidate="$1"
  [[ -x "$candidate" ]] || return 1
  "$candidate" --help 2>&1 | grep -q -- '--expected-genesis-public-key'
}

discover_helper_bin() {
  local candidate=""
  if [[ -n "$KAIGI_HELPER_BIN" ]]; then
    if helper_supports_cli "$KAIGI_HELPER_BIN"; then
      printf '%s\n' "$KAIGI_HELPER_BIN"
      return 0
    fi
    echo "configured Kaigi helper binary does not expose the genesis overlay CLI: $KAIGI_HELPER_BIN" >&2
    exit 1
  fi
  for candidate in \
    "$ROOT_DIR/target/release/examples/taira_kaigi_localnet" \
    "$ROOT_DIR/target/debug/examples/taira_kaigi_localnet"
  do
    if helper_supports_cli "$candidate"; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done
  candidate="$(ls -1t "$ROOT_DIR"/target/release/examples/taira_kaigi_localnet-* 2>/dev/null | head -n1 || true)"
  if [[ -n "$candidate" ]] && helper_supports_cli "$candidate"; then
    printf '%s\n' "$candidate"
    return 0
  fi
  candidate="$(ls -1t "$ROOT_DIR"/target/debug/examples/taira_kaigi_localnet-* 2>/dev/null | head -n1 || true)"
  if [[ -n "$candidate" ]] && helper_supports_cli "$candidate"; then
    printf '%s\n' "$candidate"
    return 0
  fi
  return 1
}

need_cmd cargo
need_cmd curl
need_cmd jq
need_cmd python3
need_cmd screen
need_file "$GENESIS_JSON"
need_file "$LOCALNET_DIR/client.toml"
need_file "$TAIRA_PROFILE_CONFIG"
ensure_launchd_runner
discover_peer_configs
load_taira_authority
load_localnet_genesis_signer_config

HOST_PUBLIC_KEY="$(extract_toml_string public_key "$LOCALNET_DIR/client.toml")"
FEE_ASSET_ID="$(extract_toml_string fee_asset_id "${PEER_CONFIGS[0]}")"
PEER_PUBLIC_KEYS=()
for peer_config in "${PEER_CONFIGS[@]}"; do
  peer_public_key="$(extract_toml_string public_key "$peer_config")"
  if [[ -z "$peer_public_key" ]]; then
    echo "failed to extract public key from $peer_config" >&2
    exit 1
  fi
  PEER_PUBLIC_KEYS+=("$peer_public_key")
done

if [[ -z "$HOST_PUBLIC_KEY" || -z "$FEE_ASSET_ID" || -z "$LOCALNET_GENESIS_PUBLIC_KEY" ]]; then
  echo "failed to extract host public key, fee asset, or genesis signer from localnet configs" >&2
  exit 1
fi

patch_peer_configs_for_taira_authority "$FEE_ASSET_ID"
ensure_private_dataspace_onboarding_permissions
ensure_dpn_genesis_seed_state "$FEE_ASSET_ID"

bootstrap_authority_seed_state="$(
  python3 - "$GENESIS_JSON" "$TAIRA_AUTHORITY" "$FEE_ASSET_ID" <<'PY'
import json
import sys

path, authority, fee_asset_id = sys.argv[1:]
target_balance = f"{fee_asset_id}#{authority}"
flags = {
    "account": False,
    "funding": False,
    "CanManageSoracloud": False,
    "CanManageAccountAlias": False,
    "CanPublishSpaceDirectoryManifest": False,
}

with open(path, encoding="utf-8") as fh:
    genesis = json.load(fh)

for tx in genesis.get("transactions", []):
    for instruction in tx.get("instructions", []):
        if not isinstance(instruction, dict):
            continue
        register = instruction.get("Register")
        if register and register.get("Account", {}).get("id") == authority:
            flags["account"] = True

        mint = instruction.get("Mint")
        if mint and mint.get("Asset", {}).get("destination") == target_balance:
            flags["funding"] = True

        permission_grant = instruction.get("Grant", {}).get("Permission")
        if permission_grant and permission_grant.get("destination") == authority:
            name = permission_grant.get("object", {}).get("name")
            if name in flags:
                flags[name] = True

required = (
    flags["account"]
    and flags["funding"]
    and flags["CanManageSoracloud"]
    and flags["CanManageAccountAlias"]
    and flags["CanPublishSpaceDirectoryManifest"]
)
print("present" if required else "missing")
PY
)"

helper_bin="$(discover_helper_bin || true)"
echo "building signed Kaigi overlay genesis"
helper_args=(
  --genesis "$GENESIS_JSON"
  --config "$LOCALNET_DIR/peer0.toml"
  --out-file "$GENESIS_SIGNED"
  --expected-genesis-public-key "$LOCALNET_GENESIS_PUBLIC_KEY"
  --host-public-key "$HOST_PUBLIC_KEY"
  --relay-domain "$RELAY_DOMAIN"
  --call-domain "$CALL_DOMAIN"
  --call-name "$CALL_NAME"
  --reported-at-ms "$REPORTED_AT_MS"
)
for index in "${!PEER_PUBLIC_KEYS[@]}"; do
  helper_args+=(
    --relay-spec "${PEER_PUBLIC_KEYS[$index]}:$(relay_hpke_key_for_index "$index"):$(relay_bandwidth_class_for_index "$index")"
  )
done
if [[ "$bootstrap_authority_seed_state" == "present" ]]; then
  echo "bootstrap authority already seeded in genesis; skipping bootstrap overlay"
else
  helper_args+=(
    --bootstrap-authority-account "$TAIRA_AUTHORITY"
    --bootstrap-authority-domain "$BOOTSTRAP_AUTHORITY_DOMAIN"
    --bootstrap-authority-fee-asset-id "$FEE_ASSET_ID"
  )
fi
if [[ -n "$GENESIS_PRIVATE_KEY" ]]; then
  helper_args+=(--genesis-private-key "$GENESIS_PRIVATE_KEY")
elif [[ -n "$LOCALNET_BASE_SEED" ]]; then
  helper_args+=(--seed "$LOCALNET_BASE_SEED")
fi
if [[ -n "$helper_bin" ]]; then
  "$helper_bin" "${helper_args[@]}"
else
  cargo run -p iroha_kagami --example taira_kaigi_localnet --release -- "${helper_args[@]}"
fi

echo "stopping existing taira localnet session"
"$LOCALNET_DIR/stop.sh" >/dev/null 2>&1 || true
screen -S "$SCREEN_SESSION" -X quit >/dev/null 2>&1 || true

if [[ -d "$LOCALNET_DIR/storage" ]]; then
  timestamp="$(date -u +%Y%m%d-%H%M%S)"
  mv "$LOCALNET_DIR/storage" "$LOCALNET_DIR/storage.prev-$timestamp"
fi
mkdir -p "$LOCALNET_DIR/storage"

echo "starting taira localnet"
screen -dmS "$SCREEN_SESSION" zsh -lc "cd '$LOCALNET_DIR' && ./launchd-run.sh"

echo "waiting for local status"
for _ in {1..60}; do
  if curl -sf "http://127.0.0.1:29080/status" >/dev/null; then
    break
  fi
  sleep 2
done

echo "waiting for Kaigi relay metadata"
for _ in {1..60}; do
  relay_json="$(curl -sf 'http://127.0.0.1:29080/v1/kaigi/relays' || true)"
  if [[ -n "$relay_json" ]] && [[ "$(jq -r '.total // 0' <<<"$relay_json")" -ge "${#PEER_PUBLIC_KEYS[@]}" ]]; then
    break
  fi
  sleep 2
done

echo "kaigi relays:"
curl -sf "http://127.0.0.1:29080/v1/kaigi/relays" | jq .

echo "kaigi health snapshot:"
curl -sf "http://127.0.0.1:29080/v1/kaigi/relays/health" | jq .

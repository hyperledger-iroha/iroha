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
TAIRA_ONBOARDING_TOKEN_FILE="${IROHA_TAIRA_ONBOARDING_TOKEN_FILE:-}"
SITE_BINDINGS_FILE="${IROHA_TAIRA_SITE_BINDINGS_FILE:-$ROOT_DIR/configs/soranexus/taira/sorafs_sites.json}"
CALL_DOMAIN="${IROHA_TAIRA_KAIGI_CALL_DOMAIN:-wonderland.universal}"
CALL_NAME="${IROHA_TAIRA_KAIGI_CALL_NAME:-taira-relay-bootstrap}"
REPORTED_AT_MS="${IROHA_TAIRA_KAIGI_REPORTED_AT_MS:-1890864000000}"
RELAY_DOMAIN="${IROHA_TAIRA_KAIGI_RELAY_DOMAIN:-nexus.universal}"
BOOTSTRAP_AUTHORITY_DOMAIN="${IROHA_TAIRA_KAIGI_BOOTSTRAP_AUTHORITY_DOMAIN:-nexus.universal}"
KAIGI_HELPER_BIN="${IROHA_TAIRA_KAIGI_HELPER_BIN:-}"
DPN_DATASPACE_ID="${IROHA_TAIRA_DPN_DATASPACE_ID:-10}"
DPN_DATASPACE_ALIAS="${IROHA_TAIRA_DPN_DATASPACE_ALIAS:-dpn}"
DPN_ACCOUNT_DOMAIN="${IROHA_TAIRA_DPN_ACCOUNT_DOMAIN:-wonderland.dpn}"
DPN_SPONSOR_ACCOUNT_ID="${IROHA_TAIRA_DPN_SPONSOR_ACCOUNT_ID:-}"
DPN_SPONSOR_FUND_AMOUNT="${IROHA_TAIRA_DPN_SPONSOR_FUND_AMOUNT:-1000}"
TAIRA_AUTHORITY_FUND_AMOUNT="${IROHA_TAIRA_AUTHORITY_FUND_AMOUNT:-1000000000}"

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

torii_json_get() {
  local url="$1"
  curl -sf -H 'Accept: application/json' "$url" | python3 -c '
import sys

payload = sys.stdin.buffer.read()
for marker in (b"{\"", b"["):
    index = payload.find(marker)
    if index >= 0:
        sys.stdout.buffer.write(payload[index:])
        raise SystemExit(0)
sys.stdout.buffer.write(payload)
'
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
block = cfg.get("sumeragi", {}).get("block", {})
print(quota.get("storage_pin_max_events", 64))
print(quota.get("storage_pin_window_secs", 3600))
print(shared.get("account_onboarding_authority", ""))
print(shared.get("account_onboarding_private_key", ""))
print(shared.get("account_onboarding_api_token", ""))
print(shared.get("account_onboarding_credential_id", ""))
print(shared.get("account_onboarding_scope_domain", ""))
print(shared.get("account_onboarding_scope_dataspace", ""))
print(block.get("max_transactions", ""))
print(block.get("max_payload_bytes", ""))
print(block.get("proposal_queue_scan_multiplier", ""))
PY
  )
  if [[ "${#values[@]}" -lt 12 ]]; then
    echo "failed to load Taira max_content_len/quota/block-budget/shared local bootstrap signer defaults from $TAIRA_PROFILE_CONFIG" >&2
    exit 1
  fi
  TAIRA_TORII_MAX_CONTENT_LEN="${IROHA_TAIRA_TORII_MAX_CONTENT_LEN:-${values[0]}}"
  TAIRA_STORAGE_PIN_MAX_EVENTS="${IROHA_TAIRA_STORAGE_PIN_MAX_EVENTS:-${values[1]}}"
  TAIRA_STORAGE_PIN_WINDOW_SECS="${IROHA_TAIRA_STORAGE_PIN_WINDOW_SECS:-${values[2]}}"
  TAIRA_AUTHORITY="${IROHA_TAIRA_AUTHORITY:-${values[3]}}"
  TAIRA_AUTHORITY_PRIVATE_KEY="${IROHA_TAIRA_AUTHORITY_PRIVATE_KEY:-${values[4]}}"
  TAIRA_ONBOARDING_API_TOKEN="${values[5]}"
  TAIRA_ONBOARDING_CREDENTIAL_ID="${values[6]:-local-dev}"
  TAIRA_ONBOARDING_SCOPE_DOMAIN="${values[7]}"
  TAIRA_ONBOARDING_SCOPE_DATASPACE="${values[8]}"
  TAIRA_BLOCK_MAX_TRANSACTIONS="${IROHA_TAIRA_BLOCK_MAX_TRANSACTIONS:-${values[9]}}"
  TAIRA_BLOCK_MAX_PAYLOAD_BYTES="${IROHA_TAIRA_BLOCK_MAX_PAYLOAD_BYTES:-${values[10]}}"
  TAIRA_PROPOSAL_QUEUE_SCAN_MULTIPLIER="${IROHA_TAIRA_PROPOSAL_QUEUE_SCAN_MULTIPLIER:-${values[11]}}"
  if [[ -z "$TAIRA_AUTHORITY" || -z "$TAIRA_AUTHORITY_PRIVATE_KEY" ]]; then
    echo "set IROHA_TAIRA_AUTHORITY and IROHA_TAIRA_AUTHORITY_PRIVATE_KEY or provide [shared] account_onboarding_* values in $TAIRA_SECRETS_FILE; local bootstrap reuses that shared signer for the served faucet too" >&2
    exit 1
  fi
  if [[ -n "$TAIRA_ONBOARDING_TOKEN_FILE" ]]; then
    need_file "$TAIRA_ONBOARDING_TOKEN_FILE"
    TAIRA_ONBOARDING_API_TOKEN="$(<"$TAIRA_ONBOARDING_TOKEN_FILE")"
  fi
  if [[ -z "$TAIRA_ONBOARDING_API_TOKEN" ]]; then
    TAIRA_ONBOARDING_API_TOKEN="$(python3 -c 'import secrets; print(secrets.token_urlsafe(32))')"
  fi
  if [[ -z "$TAIRA_ONBOARDING_SCOPE_DOMAIN" && -z "$TAIRA_ONBOARDING_SCOPE_DATASPACE" ]]; then
    TAIRA_ONBOARDING_SCOPE_DOMAIN="$CALL_DOMAIN"
  elif [[ -n "$TAIRA_ONBOARDING_SCOPE_DOMAIN" && -n "$TAIRA_ONBOARDING_SCOPE_DATASPACE" ]]; then
    echo "configure exactly one account onboarding domain or dataspace scope" >&2
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
  TAIRA_ONBOARDING_API_TOKEN="$TAIRA_ONBOARDING_API_TOKEN" \
  TAIRA_ONBOARDING_CREDENTIAL_ID="$TAIRA_ONBOARDING_CREDENTIAL_ID" \
  TAIRA_ONBOARDING_SCOPE_DOMAIN="$TAIRA_ONBOARDING_SCOPE_DOMAIN" \
  TAIRA_ONBOARDING_SCOPE_DATASPACE="$TAIRA_ONBOARDING_SCOPE_DATASPACE" \
  TAIRA_TORII_MAX_CONTENT_LEN="$TAIRA_TORII_MAX_CONTENT_LEN" \
  TAIRA_STORAGE_PIN_MAX_EVENTS="$TAIRA_STORAGE_PIN_MAX_EVENTS" \
  TAIRA_STORAGE_PIN_WINDOW_SECS="$TAIRA_STORAGE_PIN_WINDOW_SECS" \
  TAIRA_BLOCK_MAX_TRANSACTIONS="${TAIRA_BLOCK_MAX_TRANSACTIONS:-}" \
  TAIRA_BLOCK_MAX_PAYLOAD_BYTES="${TAIRA_BLOCK_MAX_PAYLOAD_BYTES:-}" \
  TAIRA_PROPOSAL_QUEUE_SCAN_MULTIPLIER="${TAIRA_PROPOSAL_QUEUE_SCAN_MULTIPLIER:-}" \
  TAIRA_FEE_ASSET_ID="$fee_asset_id" \
  TAIRA_ONBOARDING_FEE_SPONSOR_PROGRAM_ID="${DPN_SPONSOR_ACCOUNT_ID:-$TAIRA_AUTHORITY}/default" \
  SITE_BINDINGS_FILE="$SITE_BINDINGS_FILE" \
  LOCALNET_DIR="$LOCALNET_DIR" \
  python3 <<'PY'
from pathlib import Path
import blake3
import json
import os
import re

localnet_dir = Path(os.environ["LOCALNET_DIR"])
authority = os.environ["TAIRA_AUTHORITY"]
private_key = os.environ["TAIRA_AUTHORITY_PRIVATE_KEY"]
api_token = os.environ["TAIRA_ONBOARDING_API_TOKEN"]
credential_id = os.environ["TAIRA_ONBOARDING_CREDENTIAL_ID"]
scope_domain = os.environ["TAIRA_ONBOARDING_SCOPE_DOMAIN"]
scope_dataspace = os.environ["TAIRA_ONBOARDING_SCOPE_DATASPACE"]
max_content_len = os.environ["TAIRA_TORII_MAX_CONTENT_LEN"]
storage_pin_max_events = os.environ["TAIRA_STORAGE_PIN_MAX_EVENTS"]
storage_pin_window_secs = os.environ["TAIRA_STORAGE_PIN_WINDOW_SECS"]
block_budget_values = {
    "max_transactions": os.environ.get("TAIRA_BLOCK_MAX_TRANSACTIONS", "").strip(),
    "max_payload_bytes": os.environ.get("TAIRA_BLOCK_MAX_PAYLOAD_BYTES", "").strip(),
    "proposal_queue_scan_multiplier": os.environ.get(
        "TAIRA_PROPOSAL_QUEUE_SCAN_MULTIPLIER", ""
    ).strip(),
}
fee_asset_id = os.environ["TAIRA_FEE_ASSET_ID"]
fee_sponsor_program_id = os.environ["TAIRA_ONBOARDING_FEE_SPONSOR_PROGRAM_ID"]
site_bindings_file = str(Path(os.environ["SITE_BINDINGS_FILE"]).resolve())

localnet_dir.chmod(0o700)
runtime_dir = localnet_dir / "runtime"
runtime_dir.mkdir(mode=0o700, exist_ok=True)
runtime_dir.chmod(0o700)

def write_private(path: Path, value: str) -> Path:
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
        handle.write(value)
        if not value.endswith("\n"):
            handle.write("\n")
    path.chmod(0o600)
    return path.resolve()

onboarding_key_file = write_private(runtime_dir / "onboarding-signer.key", private_key)
faucet_key_file = write_private(runtime_dir / "faucet-signer.key", private_key)
write_private(runtime_dir / "onboarding-token", api_token)
token_hash = f"blake3:{blake3.blake3(api_token.encode('utf-8')).hexdigest()}"
scope = (
    f"{{ domain = {json.dumps(scope_domain)} }}"
    if scope_domain
    else f"{{ dataspace = {json.dumps(scope_dataspace)} }}"
)

gitignore = localnet_dir / ".gitignore"
if not gitignore.exists():
    gitignore.write_text("*\n!.gitignore\n", encoding="utf-8")

onboarding_block = f"""[torii.account_onboarding]
authority = {json.dumps(authority)}
private_key_file = {json.dumps(str(onboarding_key_file))}
lease_term_years = 1
additional_permissions = []
fee_sponsor_program_id = "{fee_sponsor_program_id}"

[[torii.account_onboarding.credentials]]
id = {json.dumps(credential_id)}
scope = {scope}
token_hash = {json.dumps(token_hash)}
"""

soracloud_submission_block = f"""[soracloud_runtime.submission]
fee_payer = "sponsor"
fee_program_id = "{fee_sponsor_program_id}"
fee_program_revision = 1
"""

# Local bootstrap intentionally reuses the onboarding signer as the served faucet authority.
faucet_block = f"""[torii.faucet]
enabled = true
authority = "{authority}"
private_key_file = {json.dumps(str(faucet_key_file))}
asset_definition_id = "{fee_asset_id}"
amount = "25000"
pow_difficulty_bits = 4
pow_scrypt_log_n = 13
pow_scrypt_r = 8
pow_scrypt_p = 1
pow_max_anchor_age_blocks = 256
pow_adaptive_lookback_blocks = 64
pow_adaptive_claims_per_extra_bit = 0
pow_adaptive_max_extra_bits = 0
pow_vrf_seed_enabled = false
"""

quota_block = f"""[sorafs.quota]
storage_pin_max_events = {storage_pin_max_events}
storage_pin_window_secs = {storage_pin_window_secs}
"""

site_bindings_block = f"""[sorafs.gateway.site_bindings]
path = {json.dumps(site_bindings_file)}
max_bytes = 1048576
max_sites = 1024
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

def replace_account_onboarding(text: str, block: str) -> str:
    for header in (
        "[[torii.account_onboarding.credentials]]",
        "[torii.account_onboarding.auto_renew]",
        "[torii.account_onboarding]",
        "[torii.onboarding]",
    ):
        pattern = re.compile(rf"(?ms)^{re.escape(header)}\n.*?(?=^\[|\Z)")
        text = pattern.sub("", text)
    return replace_or_insert(text, "torii.account_onboarding", block)

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

def ensure_sumeragi_block_budget(text: str) -> str:
    entries = [(key, value) for key, value in block_budget_values.items() if value]
    if not entries:
        return text

    section = re.compile(r"(?ms)^\[sumeragi\.block\]\n.*?(?=^\[|\Z)")
    match = section.search(text)
    if match:
        lines = match.group(0).rstrip().splitlines()
    else:
        lines = ["[sumeragi.block]"]

    for key, value in entries:
        line = f"{key} = {value}"
        for idx, existing in enumerate(lines):
            if re.match(rf"^\s*{re.escape(key)}\s*=", existing):
                lines[idx] = line
                break
        else:
            lines.append(line)

    updated = "\n".join(lines) + "\n"
    if match:
        return text[: match.start()] + updated + text[match.end() :]

    return text.rstrip() + "\n\n" + updated

for path in sorted(localnet_dir.glob("peer*.toml")):
    text = path.read_text()
    text = re.sub(
        r'(?m)^fee_sponsor_program_id\s*=\s*"[^"]*"$',
        f'fee_sponsor_program_id = "{fee_sponsor_program_id}"',
        text,
    )
    text = ensure_torii_max_content_len(text)
    text = ensure_sumeragi_block_budget(text)
    text = replace_or_insert(text, "sorafs.quota", quota_block)
    text = replace_or_insert(text, "sorafs.gateway.site_bindings", site_bindings_block)
    text = replace_or_insert(
        text, "soracloud_runtime.submission", soracloud_submission_block
    )
    text = replace_account_onboarding(text, onboarding_block)
    text = replace_or_insert(text, "torii.faucet", faucet_block)
    path.write_text(text)
    path.chmod(0o600)
PY
}

patch_peer_configs_for_dpn_dataspace_alias() {
  DPN_DATASPACE_ALIAS="$DPN_DATASPACE_ALIAS" \
  LOCALNET_DIR="$LOCALNET_DIR" \
  python3 <<'PY'
from pathlib import Path
import os

localnet_dir = Path(os.environ["LOCALNET_DIR"])
alias = os.environ["DPN_DATASPACE_ALIAS"].strip()
if not alias:
    raise SystemExit("DPN dataspace alias must not be empty")

for path in sorted(localnet_dir.glob("peer*.toml")):
    text = path.read_text(encoding="utf-8")
    text = text.replace('alias = "paynet"', f'alias = "{alias}"')
    text = text.replace('dataspace = "paynet"', f'dataspace = "{alias}"')
    text = text.replace('account = "*@paynet"', f'account = "*@{alias}"')
    text = text.replace('account = "*@*.paynet"', f'account = "*@*.{alias}"')
    text = text.replace("PayNet private dataspace lane", "DPN private dataspace lane")
    text = text.replace("PayNet private dataspace", "DPN private dataspace")
    text = text.replace("Route *@paynet account traffic to paynet lane", f"Route *@{alias} account traffic to {alias} lane")
    text = text.replace("Route *@*.paynet account traffic to paynet lane", f"Route *@*.{alias} account traffic to {alias} lane")
    path.write_text(text, encoding="utf-8")
PY
}

ensure_private_dataspace_onboarding_permissions() {
  GENESIS_JSON="$GENESIS_JSON" \
  TAIRA_AUTHORITY="$TAIRA_AUTHORITY" \
  DPN_DATASPACE_ID="$DPN_DATASPACE_ID" \
  UNIVERSAL_DATASPACE_ID=0 \
  python3 <<'PY'
from pathlib import Path
import json
import os

path = Path(os.environ["GENESIS_JSON"])
authority = os.environ["TAIRA_AUTHORITY"]
dataspace_id = int(os.environ["DPN_DATASPACE_ID"])
universal_dataspace_id = int(os.environ["UNIVERSAL_DATASPACE_ID"])

payload_alias = {"scope": {"scope": "dataspace", "value": dataspace_id}}
payload_universal_alias = {
    "scope": {"scope": "dataspace", "value": universal_dataspace_id}
}
payload_manifest = {"dataspace": dataspace_id}

with path.open(encoding="utf-8") as fh:
    genesis = json.load(fh)

has_alias_permission = False
has_universal_alias_permission = False
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
            obj.get("name") == "CanManageAccountAlias"
            and obj.get("payload") == payload_universal_alias
        ):
            has_universal_alias_permission = True
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
if not has_universal_alias_permission:
    instructions.append(
        {
            "Grant": {
                "Permission": {
                    "destination": authority,
                    "object": {
                        "name": "CanManageAccountAlias",
                        "payload": payload_universal_alias,
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
    transactions = genesis.setdefault("transactions", [])
    if transactions:
        transactions[-1].setdefault("instructions", []).extend(instructions)
    else:
        transactions.append(
            {
                "instructions": instructions,
                "ivm_triggers": [],
                "topology": [],
            }
        )
    path.write_text(json.dumps(genesis, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(
        f"seeded onboarding permissions for dataspaces {universal_dataspace_id} and {dataspace_id}"
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
program_id = {"sponsor": sponsor_account_id, "name": "default"}

if int(fund_amount) < 11:
    raise SystemExit("DPN sponsor funding must be at least 11 fee-asset units")

with path.open(encoding="utf-8") as fh:
    genesis = json.load(fh)

has_domain = False
has_account = False
has_funding = False
has_program = False
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
        create_program = instruction.get("CreateFeeSponsorProgram", {}).get("program")
        if isinstance(create_program, dict) and create_program.get("id") == program_id:
            has_program = True

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
if not has_program:
    instructions.extend(
        [
            {
                "CreateFeeSponsorProgram": {
                    "program": {
                        "id": program_id,
                        "lifecycle": {"state": "staged"},
                    }
                }
            },
            {
                "StageFeeSponsorProgramRevision": {
                    "revision": {
                        "program_id": program_id,
                        "revision": 1,
                        "eligibility": {"mode": "enrolled_or_route_default"},
                        "rules": [
                            {
                                "id": "taira_writes",
                                "effect": {"effect": "allow"},
                                "selectors": [
                                    {
                                        "kind": "native_instruction",
                                        "value": {"wire_id": "iroha.log"},
                                    },
                                    {
                                        "kind": "native_instruction",
                                        "value": {"wire_id": "iroha.register"},
                                    },
                                    {
                                        "kind": "native_instruction",
                                        "value": {"wire_id": "iroha.grant"},
                                    },
                                    {
                                        "kind": "native_instruction",
                                        "value": {"wire_id": "iroha.alias.ensure"},
                                    },
                                    {
                                        "kind": "native_instruction",
                                        "value": {
                                            "wire_id": "nexus::EnrollFeeSponsorBeneficiary"
                                        },
                                    },
                                    {
                                        "kind": "native_instruction",
                                        "value": {
                                            "wire_id": "iroha_data_model::isi::space_directory::PublishSpaceDirectoryManifest"
                                        },
                                    },
                                    {
                                        "kind": "native_instruction",
                                        "value": {
                                            "wire_id": "iroha.account.alias.primary.compare_and_set"
                                        },
                                    },
                                    {
                                        "kind": "native_instruction",
                                        "value": {
                                            "wire_id": "iroha_data_model::isi::smart_contract_code::UploadSmartContractCodeChunk"
                                        },
                                    },
                                    {
                                        "kind": "native_instruction",
                                        "value": {
                                            "wire_id": "iroha_data_model::isi::smart_contract_code::FinalizeSmartContractCodeUpload"
                                        },
                                    },
                                    {
                                        "kind": "native_instruction",
                                        "value": {
                                            "wire_id": "iroha_data_model::isi::smart_contract_code::RegisterSmartContractCode"
                                        },
                                    },
                                    {
                                        "kind": "native_instruction",
                                        "value": {
                                            "wire_id": "iroha_data_model::isi::smart_contract_code::ActivateContractInstance"
                                        },
                                    },
                                    {
                                        "kind": "native_instruction",
                                        "value": {"wire_id": "iroha.contract.alias.set"},
                                    },
                                    {
                                        "kind": "native_instruction",
                                        "value": {
                                            "wire_id": "iroha_data_model::isi::sorafs::RegisterCapacityDeclaration"
                                        },
                                    },
                                    {
                                        "kind": "native_instruction",
                                        "value": {"wire_id": "iroha.set_key_value"},
                                    },
                                    {
                                        "kind": "native_instruction",
                                        "value": {
                                            "wire_id": "soracloud::HeartbeatSoracloudModelHost"
                                        },
                                    },
                                    {
                                        "kind": "native_instruction",
                                        "value": {
                                            "wire_id": "soracloud::AdvertiseSoracloudInrouHost"
                                        },
                                    },
                                    {
                                        "kind": "native_instruction",
                                        "value": {
                                            "wire_id": "soracloud::ReconcileSoracloudInrouPlacements"
                                        },
                                    },
                                    {
                                        "kind": "native_instruction",
                                        "value": {
                                            "wire_id": "iroha_data_model::isi::soracloud::ReconcileSoracloudModelHosts"
                                        },
                                    },
                                    {
                                        "kind": "native_instruction",
                                        "value": {
                                            "wire_id": "iroha_data_model::isi::soracloud::ReportSoracloudModelHostViolation"
                                        },
                                    },
                                    {
                                        "kind": "native_instruction",
                                        "value": {
                                            "wire_id": "soracloud::SetSoracloudInrouReplicaRuntimeState"
                                        },
                                    },
                                    {
                                        "kind": "native_instruction",
                                        "value": {
                                            "wire_id": "soracloud::ClearSoracloudInrouReplicaRuntimeState"
                                        },
                                    },
                                    {
                                        "kind": "native_instruction",
                                        "value": {
                                            "wire_id": "soracloud::ReportSoracloudServiceLeaseUsage"
                                        },
                                    },
                                ],
                            }
                        ],
                        "asset_budgets": [
                            {
                                "asset_definition_id": fee_asset_id,
                                "per_transaction": "10",
                                "per_block": "100",
                                "per_program_epoch": "1000",
                                "per_beneficiary_epoch": "500",
                                "reserve_floor": "1",
                                "epoch_length_blocks": 3600,
                            }
                        ],
                    }
                }
            },
            {
                "EnrollFeeSponsorBeneficiary": {
                    "program_id": program_id,
                    "beneficiary": sponsor_account_id,
                }
            },
            {
                "FundFeeSponsorProgram": {
                    "program_id": program_id,
                    "asset_definition_id": fee_asset_id,
                    "amount": fund_amount,
                }
            },
            {
                "ActivateFeeSponsorProgramRevision": {
                    "program_id": program_id,
                    "revision": 1,
                    "activate_at_height": 1,
                }
            },
        ]
    )

if not instructions:
    print("DPN domain/account/funding/sponsor program already present in genesis")
else:
    transactions = genesis.setdefault("transactions", [])
    if transactions:
        transactions[-1].setdefault("instructions", []).extend(instructions)
    else:
        transactions.append(
            {
                "instructions": instructions,
                "ivm_triggers": [],
                "topology": [],
            }
        )
    path.write_text(json.dumps(genesis, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(
        "seeded DPN genesis state "
        f"(domain={domain_id}, program={sponsor_account_id}/default, amount={fund_amount})"
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
  launch_env=(
    SNAPSHOT_STORE_DIR="\$snapshot_dir"
    RUST_LOG="\${RUST_LOG:-info}"
    ZK_HALO2_ENABLED="\${ZK_HALO2_ENABLED:-true}"
  )
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
need_file "$SITE_BINDINGS_FILE"
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

patch_peer_configs_for_dpn_dataspace_alias
patch_peer_configs_for_taira_authority "$FEE_ASSET_ID"
echo "onboarding signer sidecar: $LOCALNET_DIR/runtime/onboarding-signer.key"
echo "onboarding token sidecar: $LOCALNET_DIR/runtime/onboarding-token"
echo "faucet signer sidecar: $LOCALNET_DIR/runtime/faucet-signer.key"
ensure_private_dataspace_onboarding_permissions
ensure_dpn_genesis_seed_state "$FEE_ASSET_ID"

bootstrap_authority_seed_state="$(
  python3 - "$GENESIS_JSON" "$TAIRA_AUTHORITY" "$FEE_ASSET_ID" "$TAIRA_AUTHORITY_FUND_AMOUNT" <<'PY'
import json
import sys
from decimal import Decimal, InvalidOperation

path, authority, fee_asset_id, required_funding_raw = sys.argv[1:]
target_balance = f"{fee_asset_id}#{authority}"
required_funding = Decimal(required_funding_raw)
funding = Decimal(0)
flags = {
    "account": False,
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
            try:
                funding += Decimal(str(mint["Asset"].get("object", "0")))
            except InvalidOperation:
                pass

        permission_grant = instruction.get("Grant", {}).get("Permission")
        if permission_grant and permission_grant.get("destination") == authority:
            name = permission_grant.get("object", {}).get("name")
            if name in flags:
                flags[name] = True

required = (
    flags["account"]
    and funding >= required_funding
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
    --bootstrap-authority-fee-amount "$TAIRA_AUTHORITY_FUND_AMOUNT"
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
  relay_json="$(torii_json_get 'http://127.0.0.1:29080/v1/kaigi/relays' || true)"
  if [[ -n "$relay_json" ]] && [[ "$(jq -r '.total // 0' <<<"$relay_json")" -ge "${#PEER_PUBLIC_KEYS[@]}" ]]; then
    break
  fi
  sleep 2
done

echo "kaigi relays:"
torii_json_get "http://127.0.0.1:29080/v1/kaigi/relays" | jq .

echo "kaigi health snapshot:"
torii_json_get "http://127.0.0.1:29080/v1/kaigi/relays/health" | jq .

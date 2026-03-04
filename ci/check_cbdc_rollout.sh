#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEFAULT_BUNDLE_ROOT="$REPO_ROOT/artifacts/nexus/cbdc_rollouts"

TARGET_ROOT="${CBDC_ROLLOUT_BUNDLE:-}"
if [[ -z "$TARGET_ROOT" ]]; then
  TARGET_ROOT="$DEFAULT_BUNDLE_ROOT"
fi

if [[ ! -e "$TARGET_ROOT" ]]; then
  echo "[cbdc] rollout bundle path not found: $TARGET_ROOT" >&2
  exit 1
fi

MANIFESTS=()
if [[ -d "$TARGET_ROOT" ]]; then
  while IFS= read -r -d '' entry; do
    MANIFESTS+=("$entry")
  done < <(find "$TARGET_ROOT" -type f -name cbdc.manifest.json -print0)
else
  if [[ "$TARGET_ROOT" == *.json ]]; then
    MANIFESTS+=("$TARGET_ROOT")
  else
    echo "[cbdc] CBDC_ROLLOUT_BUNDLE must point to a directory or cbdc.manifest.json" >&2
    exit 1
  fi
fi

if [[ ${#MANIFESTS[@]} -eq 0 ]]; then
  echo "[cbdc] no cbdc.manifest.json files found under $TARGET_ROOT" >&2
  exit 1
fi

function validate_manifest() {
  local manifest_path="$1"
  python3 - "$manifest_path" <<'PY'
import json
import string
import sys
from pathlib import Path

path = Path(sys.argv[1])
with path.open("r", encoding="utf-8") as fh:
    data = json.load(fh)

required_str = ("lane", "dataspace", "governance")
for key in required_str:
    if not isinstance(data.get(key), str) or not data[key].strip():
        raise SystemExit(f"[cbdc] manifest {path} missing '{key}' string")

if data["lane"] != "cbdc":
    raise SystemExit(f"[cbdc] manifest {path} lane must be 'cbdc'")

if not isinstance(data.get("version"), int) or data["version"] < 1:
    raise SystemExit(f"[cbdc] manifest {path} missing numeric 'version'")

validators = data.get("validators")
if not isinstance(validators, list) or len(validators) < 3:
    raise SystemExit(f"[cbdc] manifest {path} must list ≥3 validators")
for validator in validators:
    if not isinstance(validator, str) or "@" not in validator:
        raise SystemExit(f"[cbdc] manifest {path} validator '{validator}' malformed")

quorum = data.get("quorum")
if not isinstance(quorum, int) or quorum < 2:
    raise SystemExit(f"[cbdc] manifest {path} must set quorum ≥2")
if quorum > len(validators):
    raise SystemExit(
        f"[cbdc] manifest {path} quorum {quorum} cannot exceed validator count {len(validators)}"
    )

protected = data.get("protected_namespaces")
if not isinstance(protected, list) or not protected:
    raise SystemExit(f"[cbdc] manifest {path} missing protected_namespaces")
for namespace in protected:
    if not isinstance(namespace, str) or not namespace.strip():
        raise SystemExit(
            f"[cbdc] manifest {path} protected namespace '{namespace}' invalid"
        )

compo = data.get("composability_group")
if not isinstance(compo, dict):
    raise SystemExit(f"[cbdc] manifest {path} missing composability_group")

expected_keys = ("group_id_hex", "activation_epoch", "whitelist")
for key in expected_keys:
    if key not in compo:
        raise SystemExit(f"[cbdc] manifest {path} missing composability_group.{key}")

group_hex = compo["group_id_hex"]
if not isinstance(group_hex, str):
    raise SystemExit(f"[cbdc] manifest {path} missing group_id_hex string")
if len(group_hex) < 64 or len(group_hex) % 2 != 0:
    raise SystemExit(f"[cbdc] manifest {path} invalid group_id_hex length")
try:
    bytes.fromhex(group_hex)
except ValueError as exc:
    raise SystemExit(f"[cbdc] manifest {path} invalid group_id_hex hex data: {exc}") from exc

if not isinstance(compo["activation_epoch"], int) or compo["activation_epoch"] <= 0:
    raise SystemExit(f"[cbdc] manifest {path} invalid activation_epoch")

whitelist = compo["whitelist"]
if not isinstance(whitelist, list) or not whitelist:
    raise SystemExit(f"[cbdc] manifest {path} requires non-empty whitelist")
for ds in whitelist:
    if not isinstance(ds, str) or not ds.startswith("ds::"):
        raise SystemExit(f"[cbdc] manifest {path} whitelist entry '{ds}' invalid")
PY
}

function validate_capabilities() {
  local cap_dir="$1"
  python3 - "$cap_dir" <<'PY'
import json
import string
import sys
from pathlib import Path

cap_dir = Path(sys.argv[1])
if not cap_dir.is_dir():
    raise SystemExit(f"[cbdc] missing capability directory: {cap_dir}")

manifests = sorted(cap_dir.glob("*.manifest.json"))
if not manifests:
    raise SystemExit(f"[cbdc] no capability manifests found under {cap_dir}")

for manifest in manifests:
    with manifest.open("r", encoding="utf-8") as fh:
        data = json.load(fh)
    uaid_value = data.get("uaid")
    if not isinstance(uaid_value, str):
        raise SystemExit(f"[cbdc] capability {manifest} missing uaid")
    uaid_literal = uaid_value.strip()
    if not uaid_literal:
        raise SystemExit(f"[cbdc] capability {manifest} missing uaid")
    if uaid_literal.lower().startswith("uaid:"):
        uaid_hex = uaid_literal[5:].strip()
    else:
        uaid_hex = uaid_literal
    if len(uaid_hex) != 64 or any(ch not in string.hexdigits for ch in uaid_hex):
        raise SystemExit(
            f"[cbdc] capability {manifest} uaid must be `uaid:<hex>` or 64 hex characters"
        )
    if int(uaid_hex[-1], 16) % 2 == 0:
        raise SystemExit(
            f"[cbdc] capability {manifest} uaid must have least significant bit set to 1"
        )
    if not isinstance(data.get("dataspace"), int):
        raise SystemExit(f"[cbdc] capability {manifest} missing dataspace")
    activation_epoch = data.get("activation_epoch")
    expiry_epoch = data.get("expiry_epoch")
    if not isinstance(activation_epoch, int):
        raise SystemExit(f"[cbdc] capability {manifest} missing activation_epoch")
    if not isinstance(expiry_epoch, int):
        raise SystemExit(f"[cbdc] capability {manifest} missing expiry_epoch")
    if expiry_epoch <= activation_epoch:
        raise SystemExit(
            f"[cbdc] capability {manifest} expiry_epoch must be greater than activation_epoch"
        )
    entries = data.get("entries")
    if not isinstance(entries, list) or not entries:
        raise SystemExit(f"[cbdc] capability {manifest} missing entries")
    for idx, entry in enumerate(entries, start=1):
        if not isinstance(entry, dict):
            raise SystemExit(f"[cbdc] capability {manifest} entry #{idx} malformed")
        scope = entry.get("scope")
        if not isinstance(scope, dict) or not scope:
            raise SystemExit(
                f"[cbdc] capability {manifest} entry #{idx} missing scope"
            )
        effect = entry.get("effect")
        if not isinstance(effect, dict) or len(effect) != 1:
            raise SystemExit(
                f"[cbdc] capability {manifest} entry #{idx} effect must contain exactly one decision"
            )
        decision, details = next(iter(effect.items()))
        if decision == "Allow":
            if not isinstance(details, dict):
                raise SystemExit(
                    f"[cbdc] capability {manifest} entry #{idx} Allow effect must be an object"
                )
            window = details.get("window")
            if not isinstance(window, str) or not window.strip():
                raise SystemExit(
                    f"[cbdc] capability {manifest} entry #{idx} Allow effect missing window"
                )
            if "max_amount" in details and not isinstance(
                details["max_amount"], (str, int, float)
            ):
                raise SystemExit(
                    f"[cbdc] capability {manifest} entry #{idx} Allow effect max_amount must be string or number"
                )
        elif decision == "Deny":
            if not isinstance(details, dict):
                raise SystemExit(
                    f"[cbdc] capability {manifest} entry #{idx} Deny effect must be an object"
                )
            if "reason" in details and not isinstance(details["reason"], (type(None), str)):
                raise SystemExit(
                    f"[cbdc] capability {manifest} entry #{idx} Deny reason must be a string"
                )
        else:
            raise SystemExit(
                f"[cbdc] capability {manifest} entry #{idx} effect {decision} unsupported"
            )
        if "notes" in entry and not isinstance(entry["notes"], str):
            raise SystemExit(
                f"[cbdc] capability {manifest} entry #{idx} notes must be a string"
            )
    stem = manifest.name[:-len(".manifest.json")]
    to_path = manifest.with_name(f"{stem}.to")
    if not to_path.is_file():
        raise SystemExit(f"[cbdc] capability {manifest} missing compiled file {to_path.name}")
PY
}

function validate_compliance() {
  local compliance_dir="$1"
  python3 - "$compliance_dir" <<'PY'
from pathlib import Path
import sys

comp_dir = Path(sys.argv[1])
if not comp_dir.is_dir():
    raise SystemExit(f"[cbdc] missing compliance dir: {comp_dir}")
pdfs = sorted(comp_dir.glob("kyc_*.pdf"))
if not pdfs:
    raise SystemExit(f"[cbdc] compliance dir {comp_dir} missing kyc_*.pdf")
for pdf in pdfs:
    if pdf.stat().st_size == 0:
        raise SystemExit(f"[cbdc] compliance file {pdf} is empty")
PY
}

function validate_metrics() {
  local metrics_path="$1"
  python3 - "$metrics_path" <<'PY'
import sys
from pathlib import Path

path = Path(sys.argv[1])
if not path.is_file():
    raise SystemExit(f"[cbdc] missing metrics scrape: {path}")
text = path.read_text(encoding="utf-8")
if "nexus_scheduler_lane_teu_capacity" not in text:
    raise SystemExit(f"[cbdc] metrics scrape {path} lacks nexus_scheduler_lane_teu_capacity")
PY
}

function validate_tests() {
  local lane_log="$1"
  local teu_log="$2"
  python3 - "$lane_log" "$teu_log" <<'PY'
import sys
from pathlib import Path

lane_path = Path(sys.argv[1])
teu_path = Path(sys.argv[2])
for path in (lane_path, teu_path):
    if not path.is_file():
        raise SystemExit(f"[cbdc] missing test log: {path}")
    text = path.read_text(encoding="utf-8")
    if "test result: ok" not in text:
        raise SystemExit(f"[cbdc] test log {path} missing success marker")
PY
}

function validate_replay() {
  local replay_path="$1"
  python3 - "$replay_path" <<'PY'
import json
import sys
from pathlib import Path

path = Path(sys.argv[1])
if not path.is_file():
    raise SystemExit(f"[cbdc] missing programmable-money replay: {path}")
with path.open("r", encoding="utf-8") as fh:
    data = json.load(fh)
required = ("descriptor", "handle_requests", "envelope", "receipts", "telemetry")
for key in required:
    if key not in data:
        raise SystemExit(f"[cbdc] replay {path} missing key '{key}'")
if not isinstance(data["handle_requests"], list) or not data["handle_requests"]:
    raise SystemExit(f"[cbdc] replay {path} requires handle_requests entries")
if not isinstance(data["receipts"], list) or not data["receipts"]:
    raise SystemExit(f"[cbdc] replay {path} requires receipts entries")
if not isinstance(data["telemetry"], list) or not data["telemetry"]:
    raise SystemExit(f"[cbdc] replay {path} requires telemetry entries")
PY
}

function validate_minutes() {
  local minutes_path="$1"
  python3 - "$minutes_path" <<'PY'
import sys
from pathlib import Path

path = Path(sys.argv[1])
if not path.is_file():
    raise SystemExit(f"[cbdc] missing governance minutes: {path}")
text = path.read_text(encoding="utf-8")
if "activation_epoch" not in text:
    raise SystemExit(f"[cbdc] governance minutes {path} must mention activation_epoch")
if "cbdc.manifest" not in text:
    raise SystemExit(f"[cbdc] governance minutes {path} must reference cbdc.manifest")
PY
}

for manifest in "${MANIFESTS[@]}"; do
  bundle_dir="$(dirname "$manifest")"
  echo "[cbdc] validating rollout bundle: $bundle_dir"
  validate_manifest "$manifest"
  validate_capabilities "$bundle_dir/capability"
  validate_compliance "$bundle_dir/compliance"
  validate_metrics "$bundle_dir/metrics/nexus_scheduler_lane_teu_capacity.prom"
  validate_tests \
    "$bundle_dir/tests/cargo_test_nexus_lane_registry.log" \
    "$bundle_dir/tests/cargo_test_scheduler_teu.log"
  validate_replay "$bundle_dir/programmable_money/axt_replay.json"
  validate_minutes "$bundle_dir/approvals/governance_minutes.md"
  echo "[cbdc] bundle ok: $bundle_dir"
done

echo "[cbdc] rollout bundles validated: ${#MANIFESTS[@]}"

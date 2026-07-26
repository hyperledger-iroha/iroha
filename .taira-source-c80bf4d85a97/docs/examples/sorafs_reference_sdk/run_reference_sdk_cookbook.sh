#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: run_reference_sdk_cookbook.sh [--out <directory>]

Runs SoraFS reference SDK validator cookbook scenarios against committed
fixtures and writes ValidationOutcomeV1 JSON files to the output directory.

Environment:
  SORAFS_VALIDATE_BIN                 Use a prebuilt sorafs-validate binary.
  SORANET_TRUSTLESS_VERIFIER_BIN      Use a prebuilt trustless verifier binary.
USAGE
}

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
out_dir="${TMPDIR:-/tmp}/sorafs-reference-sdk-cookbook"

while (($#)); do
  case "$1" in
    --out)
      if (($# < 2)); then
        echo "--out requires a directory" >&2
        exit 64
      fi
      out_dir="$2"
      shift 2
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      echo "unknown option: $1" >&2
      usage >&2
      exit 64
      ;;
  esac
done

mkdir -p "$out_dir"

if [[ -n "${SORAFS_VALIDATE_BIN:-}" ]]; then
  validate_cmd=("$SORAFS_VALIDATE_BIN")
else
  validate_cmd=(cargo run -q -p sorafs_manifest --bin sorafs-validate --)
fi

if [[ -n "${SORANET_TRUSTLESS_VERIFIER_BIN:-}" ]]; then
  trustless_cmd=("$SORANET_TRUSTLESS_VERIFIER_BIN")
else
  trustless_cmd=(cargo run -q -p sorafs_car --features cli --bin soranet_trustless_verifier --)
fi

generated_at=300
advert_now=300
demo_advert_key_hex="a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5"
demo_order_key_hex="a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7"
demo_governance_key_hex="a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9"

assert_ok() {
  local path="$1"
  python3 - "$path" <<'PY'
import json
import sys

path = sys.argv[1]
with open(path, "r", encoding="utf-8") as fh:
    outcome = json.load(fh)
if outcome.get("status") != "Ok":
    code = outcome.get("code", "<missing-code>")
    message = outcome.get("message", "<missing-message>")
    raise SystemExit(f"{path}: expected status Ok, got {code}: {message}")
PY
}

run_json() {
  local name="$1"
  shift
  local output="$out_dir/${name}.json"
  printf 'running %-28s -> %s\n' "$name" "$output"
  (cd "$repo_root" && "$@") > "$output"
  assert_ok "$output"
}

run_json "advert" \
  "${validate_cmd[@]}" advert \
  --input fixtures/sorafs_manifest/provider_admission/advert_v1.to \
  --now "$advert_now" \
  --generated-at "$generated_at" \
  --format json

run_json "admission" \
  "${validate_cmd[@]}" admission \
  --input fixtures/sorafs_manifest/provider_admission/envelope_v1.to \
  --generated-at "$generated_at" \
  --format json

run_json "admission-renewal" \
  "${validate_cmd[@]}" admission \
  --envelope fixtures/sorafs_manifest/provider_admission/envelope_v1.to \
  --renewal fixtures/sorafs_manifest/provider_admission/renewal_v1.to \
  --generated-at "$generated_at" \
  --format json

run_json "admission-revocation" \
  "${validate_cmd[@]}" admission \
  --envelope fixtures/sorafs_manifest/provider_admission/envelope_v1.to \
  --revocation fixtures/sorafs_manifest/provider_admission/revocation_v1.to \
  --generated-at "$generated_at" \
  --format json

run_json "order" \
  "${validate_cmd[@]}" order \
  --order fixtures/sorafs_manifest/replication_order/order_v1.to \
  --generated-at "$generated_at" \
  --format json

run_json "sign-order" \
  "${validate_cmd[@]}" sign \
  --kind order \
  --input fixtures/sorafs_manifest/replication_order/order_v1.to \
  --out "$out_dir/signed-order.to" \
  --key-hex "$demo_order_key_hex" \
  --generated-at "$generated_at" \
  --format json

run_json "signed-order" \
  "${validate_cmd[@]}" order \
  --signed-order "$out_dir/signed-order.to" \
  --generated-at "$generated_at" \
  --format json

run_json "orderbook-receipt" \
  "${validate_cmd[@]}" orderbook \
  --receipt fixtures/sorafs_manifest/orderbook/settlement_receipt_v1.to \
  --generated-at "$generated_at" \
  --format json

run_json "por" \
  "${validate_cmd[@]}" por \
  --challenge fixtures/sorafs_manifest/por/challenge_v1.to \
  --proof fixtures/sorafs_manifest/por/proof_v1.to \
  --generated-at "$generated_at" \
  --format json

run_json "potr" \
  "${validate_cmd[@]}" potr \
  --receipt fixtures/sorafs_manifest/potr/receipt_v1.to \
  --profile hot \
  --generated-at "$generated_at" \
  --format json

run_json "repair-task" \
  "${validate_cmd[@]}" repair \
  --task fixtures/sorafs_manifest/repair/task_v1.to \
  --generated-at "$generated_at" \
  --format json

run_json "governance" \
  "${validate_cmd[@]}" governance \
  --node fixtures/sorafs_manifest/governance/node_v1.to \
  --cid bafygovernancelognode \
  --generated-at "$generated_at" \
  --format json

run_json "sign-advert" \
  "${validate_cmd[@]}" sign \
  --kind advert \
  --input fixtures/sorafs_manifest/provider_admission/advert_v1.to \
  --out "$out_dir/signed-advert.to" \
  --key-hex "$demo_advert_key_hex" \
  --now "$advert_now" \
  --generated-at "$generated_at" \
  --format json

run_json "signed-advert" \
  "${validate_cmd[@]}" advert \
  --input "$out_dir/signed-advert.to" \
  --now "$advert_now" \
  --generated-at "$generated_at" \
  --format json

run_json "sign-governance" \
  "${validate_cmd[@]}" sign \
  --kind governance \
  --input fixtures/sorafs_manifest/governance/node_v1.to \
  --out "$out_dir/signed-governance-node.to" \
  --key-hex "$demo_governance_key_hex" \
  --generated-at "$generated_at" \
  --format json

run_json "signed-governance" \
  "${validate_cmd[@]}" governance \
  --node "$out_dir/signed-governance-node.to" \
  --cid bafygovernancelognode \
  --generated-at "$generated_at" \
  --format json

run_json "bundle" \
  "${validate_cmd[@]}" bundle \
  --bundle fixtures/sorafs_manifest \
  --now "$advert_now" \
  --generated-at "$generated_at" \
  --format json

run_json "manifest-car-replay" \
  "${trustless_cmd[@]}" \
  --manifest fixtures/sorafs_gateway/1.0.0/manifest_v1.to \
  --car fixtures/sorafs_gateway/1.0.0/gateway.car \
  --config configs/soranet/gateway_m0/gateway_trustless_verifier.toml \
  --validation-outcome \
  --generated-at "$generated_at"

printf 'wrote SoraFS reference SDK cookbook outcomes to %s\n' "$out_dir"

#!/usr/bin/env bash
set -euo pipefail

PLACEHOLDER_KEY_ID_B64="dWl0ZXN0LWtleS1pZA=="
LEGACY_ATTESTATION_REPORT_BYTES='[108,101,103,97,99,121,45,111,102,102,108,105,110,101,45,97,116,116,101,115,116,97,116,105,111,110]'

usage() {
  cat <<'USAGE'
Build a Torii App Attest settlement replay fixture from pk-deploy artifacts.

Required (either provide --run-dir OR the three explicit file paths):
  --run-dir <dir>       pk-deploy run or cycle directory.

Optional explicit inputs (override auto-discovery):
  --payload <file>      ios_offline_payment_payload_to_android.txt
  --authority <file>    android_offline_account_id.txt (receiver authority)
  --seed <file>         android_primary_seed_b64.txt (receiver seed)

Other options:
  --chain-id <id>       Fixture chain id (default: test-chain)
  --output <file>       Output fixture path
                        (default: /tmp/iroha_app_attest_real_device_settlement_fixture.from_pkdeploy.json)
  --cargo-target-dir    Cargo target dir for offline_bundle_decode build cache
                        (default: /tmp/iroha-offline-tools)
  --require-real-device Fail if payload appears to be simulator/placeholder capture
                        (default: warn + continue)

Example:
  scripts/offline_topup/make_pkdeploy_app_attest_settlement_fixture.sh \
    --run-dir ../pk-deploy/e2e-offline-offline-e2e/20260218_063747-offline-offline
USAGE
}

RUN_DIR=""
PAYLOAD_FILE=""
AUTHORITY_FILE=""
SEED_FILE=""
CHAIN_ID="test-chain"
OUTPUT_FILE="/tmp/iroha_app_attest_real_device_settlement_fixture.from_pkdeploy.json"
CARGO_TARGET_DIR_PATH="/tmp/iroha-offline-tools"
REQUIRE_REAL_DEVICE=0
DECODE_BIN=""

ensure_decode_bin() {
  if [[ -n "$DECODE_BIN" && -x "$DECODE_BIN" ]]; then
    return
  fi

  DECODE_BIN="$CARGO_TARGET_DIR_PATH/debug/offline_bundle_decode"
  if [[ ! -x "$DECODE_BIN" ]]; then
    CARGO_TARGET_DIR="$CARGO_TARGET_DIR_PATH" cargo build -p iroha_cli --bin offline_bundle_decode >/dev/null
  fi
  if [[ ! -x "$DECODE_BIN" ]]; then
    echo "error: failed to build offline_bundle_decode at $DECODE_BIN" >&2
    exit 1
  fi
}

decode_payload_to_json() {
  local payload_file="$1"
  local output_json="$2"

  ensure_decode_bin
  "$DECODE_BIN" \
    --input "$payload_file" \
    --output "$output_json" \
    --kind receipt >/dev/null
}

payload_meta_from_decoded() {
  local decoded_json="$1"

  jq -r \
    --arg placeholder_key "$PLACEHOLDER_KEY_ID_B64" \
    --argjson legacy_report "$LEGACY_ATTESTATION_REPORT_BYTES" \
    '
      (.receipt.platform_proof.platform // "") as $platform |
      (.receipt.platform_proof.proof.key_id // "") as $key |
      (.receipt.platform_proof.proof.assertion // []) as $assertion |
      (.sender_certificate.attestation_report // []) as $attestation_report |
      ($assertion | length) as $assertion_len |
      (
        ($platform == "AppleAppAttest") and
        (
          ($key == $placeholder_key) or
          ($assertion_len <= 2) or
          ($attestation_report == $legacy_report)
        )
      ) as $is_placeholder |
      [
        $platform,
        $key,
        ($assertion_len | tostring),
        (.receipt.platform_proof.proof.challenge_hash // ""),
        (($attestation_report | length) | tostring),
        (if $is_placeholder then "1" else "0" end),
        (
          if $platform != "AppleAppAttest" then
            "not_apple_app_attest"
          elif $is_placeholder then
            "placeholder"
          else
            "real_device_candidate"
          end
        )
      ] | @tsv
    ' "$decoded_json"
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --run-dir)
      RUN_DIR="${2:-}"
      shift 2
      ;;
    --payload)
      PAYLOAD_FILE="${2:-}"
      shift 2
      ;;
    --authority)
      AUTHORITY_FILE="${2:-}"
      shift 2
      ;;
    --seed)
      SEED_FILE="${2:-}"
      shift 2
      ;;
    --chain-id)
      CHAIN_ID="${2:-}"
      shift 2
      ;;
    --output)
      OUTPUT_FILE="${2:-}"
      shift 2
      ;;
    --cargo-target-dir)
      CARGO_TARGET_DIR_PATH="${2:-}"
      shift 2
      ;;
    --require-real-device)
      REQUIRE_REAL_DEVICE=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage
      exit 1
      ;;
  esac
done

if [[ -z "$RUN_DIR" && ( -z "$PAYLOAD_FILE" || -z "$AUTHORITY_FILE" || -z "$SEED_FILE" ) ]]; then
  echo "error: provide --run-dir or all of --payload/--authority/--seed" >&2
  usage
  exit 1
fi

if [[ -n "$RUN_DIR" ]]; then
  if [[ ! -d "$RUN_DIR" ]]; then
    echo "error: --run-dir does not exist: $RUN_DIR" >&2
    exit 1
  fi

  if [[ -z "$PAYLOAD_FILE" ]]; then
    BEST_PAYLOAD=""
    FALLBACK_PAYLOAD=""
    while IFS= read -r candidate; do
      [[ -n "$candidate" ]] || continue
      if [[ -z "$FALLBACK_PAYLOAD" ]]; then
        FALLBACK_PAYLOAD="$candidate"
      fi

      candidate_decoded="$(mktemp "${TMPDIR:-/tmp}/iroha-offline-candidate.XXXXXX")"
      if decode_payload_to_json "$candidate" "$candidate_decoded"; then
        CANDIDATE_META="$(payload_meta_from_decoded "$candidate_decoded")"
        IFS=$'\t' read -r candidate_platform _ _ _ _ _ quality <<< "$CANDIDATE_META"
        if [[ "$quality" == "real_device_candidate" && "$candidate_platform" == "AppleAppAttest" ]]; then
          BEST_PAYLOAD="$candidate"
          rm -f "$candidate_decoded"
          break
        fi
      fi
      rm -f "$candidate_decoded"
    done < <(find "$RUN_DIR" -type f -path '*/ios_send_to_android/ios_offline_payment_payload_to_android.txt' | LC_ALL=C sort)

    PAYLOAD_FILE="${BEST_PAYLOAD:-$FALLBACK_PAYLOAD}"
  fi
fi

if [[ -z "$PAYLOAD_FILE" || ! -f "$PAYLOAD_FILE" ]]; then
  echo "error: unable to locate payload file" >&2
  exit 1
fi

CYCLE_ROOT="$(dirname "$(dirname "$PAYLOAD_FILE")")"

if [[ -z "$AUTHORITY_FILE" ]]; then
  if [[ -f "$CYCLE_ROOT/android_receive_from_ios/android_offline_account_id.txt" ]]; then
    AUTHORITY_FILE="$CYCLE_ROOT/android_receive_from_ios/android_offline_account_id.txt"
  elif [[ -n "$RUN_DIR" ]]; then
    AUTHORITY_FILE="$(find "$RUN_DIR" -type f -path '*/android_receive_from_ios/android_offline_account_id.txt' | LC_ALL=C sort | head -n1 || true)"
  fi
fi

if [[ -z "$SEED_FILE" ]]; then
  if [[ -f "$CYCLE_ROOT/android_receive_from_ios/android_primary_seed_b64.txt" ]]; then
    SEED_FILE="$CYCLE_ROOT/android_receive_from_ios/android_primary_seed_b64.txt"
  elif [[ -n "$RUN_DIR" ]]; then
    SEED_FILE="$(find "$RUN_DIR" -type f -path '*/android_receive_from_ios/android_primary_seed_b64.txt' | LC_ALL=C sort | head -n1 || true)"
  fi
fi

if [[ -z "$AUTHORITY_FILE" || ! -f "$AUTHORITY_FILE" ]]; then
  echo "error: unable to locate authority account file" >&2
  exit 1
fi
if [[ -z "$SEED_FILE" || ! -f "$SEED_FILE" ]]; then
  echo "error: unable to locate receiver seed file" >&2
  exit 1
fi

mkdir -p "$(dirname "$OUTPUT_FILE")"
TMP_DECODED="$(mktemp "${TMPDIR:-/tmp}/iroha-offline-receipt.XXXXXX")"
trap 'rm -f "$TMP_DECODED"' EXIT

decode_payload_to_json "$PAYLOAD_FILE" "$TMP_DECODED"

if ! jq -e '.receipt and .sender_certificate' "$TMP_DECODED" >/dev/null; then
  echo "error: decoded payload missing receipt/sender_certificate" >&2
  exit 1
fi

PAYLOAD_META="$(payload_meta_from_decoded "$TMP_DECODED")"
IFS=$'\t' read -r PAYLOAD_PLATFORM PAYLOAD_KEY_ID PAYLOAD_ASSERTION_LEN PAYLOAD_CHALLENGE_HASH PAYLOAD_ATTESTATION_REPORT_LEN PAYLOAD_IS_PLACEHOLDER PAYLOAD_QUALITY <<< "$PAYLOAD_META"

if [[ "$PAYLOAD_QUALITY" != "real_device_candidate" ]]; then
  echo "warning: selected payload appears to be placeholder/simulator App Attest evidence." >&2
  echo "warning: quality=$PAYLOAD_QUALITY platform=$PAYLOAD_PLATFORM key_id=$PAYLOAD_KEY_ID assertion_len=$PAYLOAD_ASSERTION_LEN" >&2
  echo "warning: expected real-device capture fields (non-placeholder key_id + non-trivial assertion)." >&2
  if [[ "$REQUIRE_REAL_DEVICE" -eq 1 ]]; then
    echo "error: --require-real-device was set and no real-device App Attest capture was found." >&2
    exit 1
  fi
fi

AUTHORITY="$(tr -d '\r\n' < "$AUTHORITY_FILE")"
SEED_B64="$(tr -d '\r\n' < "$SEED_FILE")"
if [[ -z "$AUTHORITY" || -z "$SEED_B64" ]]; then
  echo "error: authority/seed files must be non-empty" >&2
  exit 1
fi

if printf '' | base64 -D >/dev/null 2>&1; then
  SEED_HEX="$(printf '%s' "$SEED_B64" | base64 -D | xxd -p -c 256 | tr '[:lower:]' '[:upper:]' | tr -d '\n')"
else
  SEED_HEX="$(printf '%s' "$SEED_B64" | base64 -d | xxd -p -c 256 | tr '[:lower:]' '[:upper:]' | tr -d '\n')"
fi

if [[ ${#SEED_HEX} -ne 64 ]]; then
  echo "error: decoded seed must be 32 bytes (got ${#SEED_HEX} hex chars)" >&2
  exit 1
fi

PRIVATE_KEY="802620${SEED_HEX}"
RECEIPT_JSON="$(jq -c '.receipt' "$TMP_DECODED")"
CERTIFICATE_JSON="$(jq -c '.sender_certificate' "$TMP_DECODED")"
PLACEHOLDER_JSON=false
if [[ "$PAYLOAD_IS_PLACEHOLDER" == "1" ]]; then
  PLACEHOLDER_JSON=true
fi

jq -n \
  --arg chain_id "$CHAIN_ID" \
  --arg authority "$AUTHORITY" \
  --arg private_key "$PRIVATE_KEY" \
  --arg payload_file "$PAYLOAD_FILE" \
  --arg authority_file "$AUTHORITY_FILE" \
  --arg seed_file "$SEED_FILE" \
  --arg attestation_capture_quality "$PAYLOAD_QUALITY" \
  --arg attestation_platform "$PAYLOAD_PLATFORM" \
  --arg attestation_key_id "$PAYLOAD_KEY_ID" \
  --argjson attestation_assertion_len "$PAYLOAD_ASSERTION_LEN" \
  --arg attestation_challenge_hash "$PAYLOAD_CHALLENGE_HASH" \
  --argjson attestation_report_len "$PAYLOAD_ATTESTATION_REPORT_LEN" \
  --argjson attestation_placeholder_detected "$PLACEHOLDER_JSON" \
  --argjson receipt "$RECEIPT_JSON" \
  --argjson certificate "$CERTIFICATE_JSON" \
  '{
    fixture: "ios_app_attest_real_device_settlement_replay_v1_candidate_pkdeploy",
    description: "Auto-generated from pk-deploy iOS payload + Android receiver seed",
    chain_id: $chain_id,
    authority: $authority,
    private_key: $private_key,
    source: {
      payload_file: $payload_file,
      authority_file: $authority_file,
      seed_file: $seed_file
    },
    attestation_capture_quality: $attestation_capture_quality,
    attestation_placeholder_detected: $attestation_placeholder_detected,
    attestation_debug: {
      platform: $attestation_platform,
      key_id: $attestation_key_id,
      assertion_len: $attestation_assertion_len,
      challenge_hash: $attestation_challenge_hash,
      attestation_report_len: $attestation_report_len
    },
    expect_non_strict_ok: true,
    expect_non_strict_reject_code: null,
    expect_strict_ok: true,
    expect_strict_reject_code: null,
    certificate: $certificate,
    transfer: {
      aggregate_proof: null,
      attachments: null,
      balance_proof: {
        claimed_delta: "0",
        initial_commitment: {
          amount: $receipt.amount,
          asset: $receipt.asset,
          commitment: [range(0; 32) | 0]
        },
        resulting_commitment: [range(0; 32) | 0],
        zk_proof: null
      },
      balance_proofs: null,
      bundle_id: $receipt.tx_id,
      deposit_account: $authority,
      platform_snapshot: null,
      receipts: [$receipt],
      receiver: $authority
    },
    expect_non_strict_final_status: "rejected",
    expect_non_strict_final_reject_code: "lineage_invalid",
    expect_strict_final_status: "rejected",
    expect_strict_final_reject_code: "lineage_invalid"
  }' > "$OUTPUT_FILE"

echo "fixture=$OUTPUT_FILE"
echo "payload=$PAYLOAD_FILE"
echo "authority_file=$AUTHORITY_FILE"
echo "seed_file=$SEED_FILE"
echo "attestation_capture_quality=$PAYLOAD_QUALITY"
echo "attestation_platform=$PAYLOAD_PLATFORM"
echo "attestation_key_id=$PAYLOAD_KEY_ID"
echo "attestation_assertion_len=$PAYLOAD_ASSERTION_LEN"
echo "attestation_challenge_hash=$PAYLOAD_CHALLENGE_HASH"
echo "attestation_report_len=$PAYLOAD_ATTESTATION_REPORT_LEN"
echo "export IROHA_APPLE_ATTEST_REAL_DEVICE_SETTLEMENT_FIXTURE=$OUTPUT_FILE"

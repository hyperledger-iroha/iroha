#!/usr/bin/env bash
set -euo pipefail

PLACEHOLDER_KEY_ID_B64="dWl0ZXN0LWtleS1pZA=="
LEGACY_ATTESTATION_REPORT_BYTES='[108,101,103,97,99,121,45,111,102,102,108,105,110,101,45,97,116,116,101,115,116,97,116,105,111,110]'

usage() {
  cat <<'USAGE'
Scan pk-deploy offline payload artifacts and classify App Attest capture quality.

Options:
  --root <dir>          Add a scan root (repeatable). If omitted, defaults are used.
  --output <file>       TSV output path
                        (default: /tmp/pkdeploy_app_attest_payload_scan.tsv)
  --cargo-target-dir    Cargo target dir for offline_bundle_decode build cache
                        (default: /tmp/iroha-offline-tools)
  --require-real-device Exit non-zero when no real-device App Attest candidates are found.

Output columns:
  file, source_kind, format, platform, key_id_or_series, assertion_len,
  challenge_hash, attestation_report_len, capture_quality

Capture quality:
  - real_device_candidate
  - placeholder
  - not_apple_app_attest
  - decode_error
  - json_legacy
  - unsupported_format
USAGE
}

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

OUTPUT_FILE="/tmp/pkdeploy_app_attest_payload_scan.tsv"
CARGO_TARGET_DIR_PATH="/tmp/iroha-offline-tools"
REQUIRE_REAL_DEVICE=0
ROOTS=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --root)
      ROOTS+=("${2:-}")
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

if [[ ${#ROOTS[@]} -eq 0 ]]; then
  ROOTS=(
    "$HOME/.pk-deploy/runs/offline-offline-e2e"
    "$REPO_ROOT/../pk-deploy/e2e-offline-offline-e2e"
    "$REPO_ROOT/../pk-deploy/build/android-receiver-stage"
    "$REPO_ROOT/../pk-deploy/build/mobile-conservation-e2e"
  )
fi

DECODE_BIN="$CARGO_TARGET_DIR_PATH/debug/offline_bundle_decode"
if [[ ! -x "$DECODE_BIN" ]]; then
  CARGO_TARGET_DIR="$CARGO_TARGET_DIR_PATH" cargo build -p iroha_cli --bin offline_bundle_decode >/dev/null
fi
if [[ ! -x "$DECODE_BIN" ]]; then
  echo "error: failed to build offline_bundle_decode at $DECODE_BIN" >&2
  exit 1
fi

mkdir -p "$(dirname "$OUTPUT_FILE")"
SCAN_TMP="$(mktemp "${TMPDIR:-/tmp}/pkdeploy-app-attest-files.XXXXXX")"
trap 'rm -f "$SCAN_TMP"' EXIT

> "$SCAN_TMP"
for root in "${ROOTS[@]}"; do
  [[ -d "$root" ]] || continue
  find "$root" \
    \( -name DerivedData -o -name Index.noindex -o -name node_modules -o -name .git -o -name build \) -prune -o \
    -type f \( \
      -path '*/ios_send_to_android/ios_offline_payment_payload_to_android.txt' -o \
      -path '*/android_receive_from_ios/android_offline_received_payload_from_ios.txt' -o \
      -path '*/android_offline_receiver/android_offline_received_payload_from_ios.txt' \
    \) -print >> "$SCAN_TMP"
done

sort -u "$SCAN_TMP" -o "$SCAN_TMP"

printf "file\tsource_kind\tformat\tplatform\tkey_id_or_series\tassertion_len\tchallenge_hash\tattestation_report_len\tcapture_quality\n" > "$OUTPUT_FILE"

decode_meta_from_json() {
  local decoded_json="$1"
  jq -r \
    --arg placeholder_key "$PLACEHOLDER_KEY_ID_B64" \
    --argjson legacy_report "$LEGACY_ATTESTATION_REPORT_BYTES" \
    '
      (.receipt.platform_proof.platform // "") as $platform |
      (.receipt.platform_proof.proof.key_id // .receipt.platform_proof.proof.series // "") as $key |
      (.receipt.platform_proof.proof.assertion // []) as $assertion |
      (.sender_certificate.attestation_report // []) as $attestation_report |
      ($assertion | length) as $assertion_len |
      (
        ($platform == "AppleAppAttest") and
        (
          (.receipt.platform_proof.proof.key_id // "") == $placeholder_key or
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

while IFS= read -r payload_file; do
  [[ -n "$payload_file" ]] || continue
  source_kind="unknown"
  if [[ "$payload_file" == *"/ios_send_to_android/"* ]]; then
    source_kind="ios_send"
  elif [[ "$payload_file" == *"/android_offline_receiver/"* ]]; then
    source_kind="android_offline_receiver"
  elif [[ "$payload_file" == *"/android_receive_from_ios/"* ]]; then
    source_kind="android_receive_from_ios"
  fi

  prefix="$(head -c 7 "$payload_file" 2>/dev/null || true)"
  if [[ "$prefix" == "norito:" ]] || [[ "$prefix" == "NRT0"* ]]; then
    decoded_json="$(mktemp "${TMPDIR:-/tmp}/pkdeploy-attest-decoded.XXXXXX")"
    if "$DECODE_BIN" --input "$payload_file" --kind receipt --output "$decoded_json" >/dev/null 2>&1; then
      if meta="$(decode_meta_from_json "$decoded_json" 2>/dev/null)"; then
        IFS=$'\t' read -r platform key_id_or_series assertion_len challenge_hash attestation_report_len capture_quality <<< "$meta"
        printf "%s\t%s\tnorito\t%s\t%s\t%s\t%s\t%s\t%s\n" \
          "$payload_file" "$source_kind" "$platform" "$key_id_or_series" "$assertion_len" \
          "$challenge_hash" "$attestation_report_len" "$capture_quality" >> "$OUTPUT_FILE"
      else
        printf "%s\t%s\tnorito\t\t\t\t\t\tdecode_error\n" "$payload_file" "$source_kind" >> "$OUTPUT_FILE"
      fi
    else
      printf "%s\t%s\tnorito\t\t\t\t\t\tdecode_error\n" "$payload_file" "$source_kind" >> "$OUTPUT_FILE"
    fi
    rm -f "$decoded_json"
  elif [[ "$prefix" == "{"* ]]; then
    printf "%s\t%s\tjson\t\t\t\t\t\tjson_legacy\n" "$payload_file" "$source_kind" >> "$OUTPUT_FILE"
  else
    printf "%s\t%s\tunknown\t\t\t\t\t\tunsupported_format\n" "$payload_file" "$source_kind" >> "$OUTPUT_FILE"
  fi
done < "$SCAN_TMP"

total_rows=$(( $(wc -l < "$OUTPUT_FILE") - 1 ))
norito_rows="$(awk -F '\t' 'NR>1 && $3=="norito" {c++} END{print c+0}' "$OUTPUT_FILE")"
apple_rows="$(awk -F '\t' 'NR>1 && $4=="AppleAppAttest" {c++} END{print c+0}' "$OUTPUT_FILE")"
real_rows="$(awk -F '\t' 'NR>1 && $9=="real_device_candidate" {c++} END{print c+0}' "$OUTPUT_FILE")"
placeholder_rows="$(awk -F '\t' 'NR>1 && $9=="placeholder" {c++} END{print c+0}' "$OUTPUT_FILE")"

echo "scan_output=$OUTPUT_FILE"
echo "total_files=$total_rows"
echo "norito_payloads=$norito_rows"
echo "apple_app_attest_payloads=$apple_rows"
echo "real_device_candidates=$real_rows"
echo "placeholder_candidates=$placeholder_rows"

if [[ "$real_rows" -gt 0 ]]; then
  echo "real_device_candidate_files:"
  awk -F '\t' 'NR>1 && $9=="real_device_candidate" {print $1}' "$OUTPUT_FILE"
else
  echo "real_device_candidate_files: none"
fi

if [[ "$REQUIRE_REAL_DEVICE" -eq 1 && "$real_rows" -eq 0 ]]; then
  echo "error: --require-real-device set but no real-device App Attest captures were found." >&2
  exit 1
fi

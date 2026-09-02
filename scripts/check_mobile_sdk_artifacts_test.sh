#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd -P)"
CHECK_SCRIPT="$SCRIPT_DIR/check_mobile_sdk_artifacts.sh"
HEADER_GATE="$ROOT_DIR/ci/check_connect_norito_bridge_header.sh"

fail() {
  printf '[mobile-sdk-artifacts-test] ERROR: %s\n' "$*" >&2
  exit 1
}

[[ -x "$CHECK_SCRIPT" ]] || fail "artifact checker is not executable"
[[ -x "$HEADER_GATE" ]] || fail "bridge-header gate is not executable"
bash -n "$CHECK_SCRIPT"
"$CHECK_SCRIPT" --help >/dev/null

expected_symbols=(
  connect_norito_offline_cash_v1_payment_request_validate
  connect_norito_offline_cash_v1_payment_validate
  connect_norito_offline_cash_v1_acknowledgement_validate
  connect_norito_offline_cash_v1_mint_credit_validate
  connect_norito_offline_cash_v1_redemption_voucher_validate
  connect_norito_offline_cash_v1_payment_request_text_validate
  connect_norito_offline_cash_v1_payment_text_validate
  connect_norito_offline_cash_v1_acknowledgement_text_validate
  connect_norito_offline_cash_v1_mint_credit_text_validate
  connect_norito_offline_cash_v1_redemption_voucher_text_validate
  connect_norito_offline_cash_device_capabilities_v1
  connect_norito_offline_cash_device_execute_v1
)

for symbol in "${expected_symbols[@]}"; do
  [[ "$(grep -Fc -- "$symbol" "$CHECK_SCRIPT")" == "1" ]] \
    || fail "artifact checker must require $symbol exactly once"
done

if MOBILE_SDK_REQUIRE_ANDROID_OUTPUTS=invalid "$CHECK_SCRIPT" --android-only >/dev/null 2>&1; then
  fail "artifact checker accepted an invalid Android-output policy"
fi

"$HEADER_GATE" --self-test
"$CHECK_SCRIPT" --root "$ROOT_DIR" --android-only
printf '[mobile-sdk-artifacts-test] first-release mobile SDK contract passed\n'

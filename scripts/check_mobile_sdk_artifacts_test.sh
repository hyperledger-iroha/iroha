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

TEST_PYTHON_BINARY=""
for trusted_python in \
  /opt/homebrew/bin/python3.12 \
  /opt/homebrew/opt/python@3.12/bin/python3.12 \
  /usr/local/bin/python3.12 \
  /usr/local/opt/python@3.12/bin/python3.12 \
  /usr/bin/python3.12 \
  /usr/bin/python3; do
  if [[ -x "$trusted_python" ]] \
    && [[ "$("$trusted_python" -I -S -B -c 'import sys; print(f"{sys.version_info.major}.{sys.version_info.minor}")' 2>/dev/null)" == "3.12" ]]; then
    TEST_PYTHON_BINARY="$trusted_python"
    break
  fi
done
[[ -n "$TEST_PYTHON_BINARY" ]] || fail "pinned Python 3.12 is required"
TEST_PYTHON_BINARY="$("$TEST_PYTHON_BINARY" -I -S -B -c \
  'import pathlib,sys; print(pathlib.Path(sys.argv[1]).resolve(strict=True))' \
  "$TEST_PYTHON_BINARY")"
export MOBILE_SDK_PYTHON_BINARY="$TEST_PYTHON_BINARY"

TEST_RUSTUP_BINARY="$(command -v rustup)" || fail "rustup is required"
TEST_RUSTUP_BINARY="$("$TEST_PYTHON_BINARY" -I -S -B -c \
  'import pathlib,sys; print(pathlib.Path(sys.argv[1]).resolve(strict=True))' \
  "$TEST_RUSTUP_BINARY")" || fail "rustup must resolve to a canonical executable"
[[ "$TEST_RUSTUP_BINARY" == /* && -f "$TEST_RUSTUP_BINARY" \
  && ! -L "$TEST_RUSTUP_BINARY" && -x "$TEST_RUSTUP_BINARY" ]] \
  || fail "rustup must be an absolute canonical non-symbolic executable"
export MOBILE_SDK_RUSTUP_BINARY="$TEST_RUSTUP_BINARY"

[[ -x "$CHECK_SCRIPT" ]] || fail "artifact checker is not executable"
[[ -x "$HEADER_GATE" ]] || fail "bridge-header gate is not executable"
bash -n "$CHECK_SCRIPT"
"$CHECK_SCRIPT" --help >/dev/null

expected_symbols=(
  connect_norito_offline_cash_v1_payment_request_validate
  connect_norito_offline_cash_v1_acceptance_intent_authorization_validate
  connect_norito_offline_cash_v1_acceptance_ticket_validate
  connect_norito_offline_cash_v1_no_commit_closure_validate
  connect_norito_offline_cash_v1_payment_validate
  connect_norito_offline_cash_v1_acknowledgement_validate
  connect_norito_offline_cash_v1_complete_exchange_validate
  connect_norito_offline_cash_v1_mint_authorization_validate
  connect_norito_offline_cash_v1_mint_credit_validate
  connect_norito_offline_cash_v1_mint_credit_against_authorization_validate
  connect_norito_offline_cash_v1_redemption_voucher_validate
  connect_norito_offline_cash_v1_payment_request_text_validate
  connect_norito_offline_cash_v1_acceptance_intent_authorization_text_validate
  connect_norito_offline_cash_v1_acceptance_ticket_text_validate
  connect_norito_offline_cash_v1_no_commit_closure_text_validate
  connect_norito_offline_cash_v1_payment_text_validate
  connect_norito_offline_cash_v1_acknowledgement_text_validate
  connect_norito_offline_cash_v1_complete_exchange_text_validate
  connect_norito_offline_cash_v1_mint_authorization_text_validate
  connect_norito_offline_cash_v1_mint_credit_text_validate
  connect_norito_offline_cash_v1_mint_credit_against_authorization_text_validate
  connect_norito_offline_cash_v1_redemption_voucher_text_validate
  connect_norito_offline_cash_device_capabilities_v1
  connect_norito_offline_cash_device_execute_v1
)

for symbol in "${expected_symbols[@]}"; do
  [[ "$(grep -Fc -- "$symbol" "$CHECK_SCRIPT")" == "1" ]] \
    || fail "artifact checker must require $symbol exactly once"
done

retired_symbol=connect_norito_private_settlement_auditor_capsule_response_verify_v1
required_block="$(sed -n '/^OFFLINE_CASH_C_SYMBOLS=(/,/^)/p' "$CHECK_SCRIPT")"
retired_block="$(sed -n '/^RETIRED_PROTOCOL_C_SYMBOLS=(/,/^)/p' "$CHECK_SCRIPT")"
if grep -Fq -- "$retired_symbol" <<<"$required_block"; then
  fail "artifact checker still requires the retired fail-closed auditor verifier"
fi
grep -Fq -- "$retired_symbol" <<<"$retired_block" \
  || fail "artifact checker does not reject the retired fail-closed auditor verifier"
grep -Fq -- '--verify-repository-provenance' "$CHECK_SCRIPT" \
  || fail "artifact checker does not require repository provenance verification"

if MOBILE_SDK_REQUIRE_ANDROID_OUTPUTS=invalid "$CHECK_SCRIPT" --android-only >/dev/null 2>&1; then
  fail "artifact checker accepted an invalid Android-output policy"
fi

"$HEADER_GATE" --self-test
"$CHECK_SCRIPT" --root "$ROOT_DIR" --android-only
printf '[mobile-sdk-artifacts-test] first-release mobile SDK contract passed\n'

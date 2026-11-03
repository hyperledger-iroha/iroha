#!/usr/bin/env bash
# Run the isolated debug Core ownership controls and four-validator corridor.
# Requires Cargo and grep. CARGO_TARGET_DIR optionally selects the existing warm
# debug lane; no release binary, build-job override, or network skip is selected.
set -euo pipefail

readonly REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd -- "$REPO_ROOT"

readonly PARLIAMENT_TARGET_DIR="${CARGO_TARGET_DIR:-${REPO_ROOT}/target/sora-parliament-lifecycle}"
readonly PARLIAMENT_DAEMON="${PARLIAMENT_TARGET_DIR}/debug/iroha3d"

# Keep the proof-valid fixture providers in one dedicated debug-only build lane.
# The providers themselves reject optimized compilation, and this script never
# replaces or consumes a release binary.
export CARGO_TARGET_DIR="$PARLIAMENT_TARGET_DIR"
export NORITO_SKIP_BINDINGS_SYNC=1

cargo build --locked \
  -p irohad \
  --bin iroha3d \
  --features test-network-parliament-signers

if [[ ! -x "$PARLIAMENT_DAEMON" ]]; then
  echo "feature-isolated Parliament daemon was not built at $PARLIAMENT_DAEMON" >&2
  exit 1
fi

export TEST_NETWORK_BIN_IROHAD_PARLIAMENT_SIGNERS="$PARLIAMENT_DAEMON"
export IROHA_TEST_SKIP_BUILD=1
export IROHA_TEST_REQUIRE_NETWORK=1
export IROHA_FAIL_ON_SANDBOX_SKIP=1
export IROHA_TEST_NETWORK_START_ATTEMPTS=1

readonly -a CORE_TESTS=(
  beacon::parliament_test_network_signer::tests::authenticated_fixture_requires_exact_keys_and_every_signed_private_edge
  beacon::parliament_test_network_signer::tests::exact_seat_signer_matches_the_public_fixture_and_rejects_foreign_roster
  beacon::parliament_test_network_signer::tests::exact_seat_signer_supports_one_domain_separated_successor_session
  beacon::parliament_test_network_signer::tests::custody_retains_only_exact_initial_and_successor_owners_without_replacement
  beacon::parliament_test_network_signer::tests::poisoned_fixture_custody_refuses_capability_and_signing
  beacon::parliament_test_network_signer::tests::invalid_provider_mode_preserves_owned_capability_but_returns_a_malformed_share
  sumeragi::epoch_beacon::producer::tests::parliament_fixture_owners_drive_real_threshold_shares_and_refuse_invalid_provider_output
  sumeragi::epoch_beacon::producer::tests::native_invalid_local_share_is_never_retained_or_counted
  sumeragi::epoch_beacon::producer::tests::wrong_source_sender_and_proof_never_change_the_owned_round
)
readonly -a CORE_CARGO=(
  cargo test --locked --profile test -p iroha_core --lib
  --features mutation-testing,iroha-core-tests,test-network-parliament-signers
)
# Cargo succeeds when a filter selects nothing. Require every exact control once
# before running the same selection; include-ignored keeps these nine mandatory.
CORE_TEST_LIST="$("${CORE_CARGO[@]}" -- "${CORE_TESTS[@]}" --exact --include-ignored --list)"
printf '%s\n' "$CORE_TEST_LIST"
for core_test in "${CORE_TESTS[@]}"; do
  if [[ "$(grep -Fxc -- "${core_test}: test" <<<"$CORE_TEST_LIST" || true)" != 1 ]]; then
    echo "Parliament Core control must be selected exactly once: $core_test" >&2
    exit 1
  fi
done
if [[ "$(grep -Ec ': (test|benchmark)$' <<<"$CORE_TEST_LIST" || true)" != "${#CORE_TESTS[@]}" ]]; then
  echo "Parliament Core selection contains unexpected or duplicate tests" >&2
  exit 1
fi
"${CORE_CARGO[@]}" -- "${CORE_TESTS[@]}" --exact --include-ignored --nocapture --test-threads=1

cargo test --locked \
  -p integration_tests \
  --features parliament-test-signers \
  --test sora_parliament_lifecycle_smoke \
  -- \
  --nocapture \
  --test-threads=1

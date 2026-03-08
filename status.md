# Status

Last updated: 2026-03-08

## Current Enforced State
- First-release identity/deploy policy is strict (no backward aliases, shims, or migration wrappers).
- Deploy preflight scanner entrypoint is `../pk-deploy/scripts/check-identity-surface.sh`; the previous scanner entrypoint is removed.
- Runtime deploy scripts are aligned to strict-first-release behavior:
  - `../pk-deploy/scripts/cutover-ih58-mega.sh` invokes only `check-identity-surface.sh`.
  - `../pk-deploy/scripts/deploy-sbp-aed-pkr-interceptor.sh` no longer performs prior-layout trigger cleanup loops.
- Wallet docs describe only current QR modes in neutral terms.

## 2026-03-08 Integration Failures: CBDC Rollout + DA Kura Eviction
- Fixed CBDC rollout fixture validation to accept canonical validator identifiers used by fixtures:
  - `ci/check_cbdc_rollout.sh` now accepts either `name@domain`-style identifiers or non-empty encoded identifiers (without whitespace), instead of requiring `@` unconditionally.
- Stabilized DA-backed Kura eviction integration coverage in multi-lane storage layouts:
  - `integration_tests/tests/sumeragi_da.rs` now discovers the evicted block via `da_blocks/*.norito` paths and derives the matching lane `blocks.index`/`blocks.hashes` paths from that location.
  - The test still verifies that the selected `blocks.index` entry is marked evicted (`u64::MAX`) and that the queried rehydrated block hash matches `blocks.hashes`.

### Validation Matrix (CBDC + DA Eviction Fix)
- `cargo fmt --all`
- `cargo test -p integration_tests nexus::cbdc_rollout_bundle::cbdc_rollout_fixture_passes_validator -- --nocapture`
- `cargo test -p integration_tests sumeragi_da::sumeragi_da_kura_eviction_rehydrates_from_da_store -- --nocapture`

## 2026-03-08 Telemetry Test Helper Duplication
- Removed a duplicate async helper definition in `crates/iroha_telemetry/src/ws.rs` that caused
  `error[E0428]` for `broadcast_lag_does_not_stop_client_with_suite`.
- Kept a single canonical helper implementation; the `broadcast_lag_does_not_stop_client` test path is unchanged.

### Validation Matrix (Telemetry Duplication)
- `cargo test -p iroha_telemetry broadcast_lag_does_not_stop_client -- --nocapture`

## 2026-03-08 AccountId Parsing API Alignment (Test Samples)
- Updated `crates/iroha_test_samples/src/lib.rs` to stop parsing `AccountId` from string in tests.
- Replaced string `.parse::<AccountId>()` with explicit construction via
  `AccountId::new(DomainId, PublicKey)` to match the current data-model API.

### Validation Matrix (AccountId Parsing Alignment)
- `cargo test -p iroha_test_samples -- --nocapture`

## Changes Completed In This Pass
- Replaced deploy scanner interface with a neutral strict entrypoint.
- Updated deploy callsites/docs to the new scanner path and strict wording:
  - `../pk-deploy/scripts/cutover-ih58-mega.sh`
  - `../pk-deploy/scripts/README-redeploy.md`
- Purged prior-transition terminology from touched runtime/docs/status surfaces.
- Reset status/history files to fresh baselines:
  - `status.md`
  - `roadmap.md`
  - `../pk-deploy/STATUS.md`

## Validation Matrix (This Pass)
- `bash -n ../pk-deploy/scripts/check-identity-surface.sh ../pk-deploy/scripts/cutover-ih58-mega.sh ../pk-deploy/scripts/deploy-sbp-aed-pkr-interceptor.sh ../pk-cbuae-mock/scripts/e2e/localnet-live.sh`
- `bash ../pk-deploy/scripts/check-identity-surface.sh`
- identity-literal forbidden-token sweep across requested repos/files
- residual-token sweep across touched runtime/scripts/status files
- iOS targeted retest for previously failing flow:
  - `cd ../pk-retail-wallet-ios && xcodebuild test -scheme RetailWalletIOS -destination 'platform=iOS Simulator,name=iPhone 17,OS=26.1' -only-testing:RetailWalletIOSUITests/RetailWalletIOSFlowUITests/testOnboardingAndSendFlow`

## Remaining Actionable Blockers
- None in active A1-G1 runtime/parser/SDK paths.

## 2026-03-08 Torii /v2 API Surface Cleanup
- Removed dead telemetry compatibility handlers from Torii:
  - `crates/iroha_torii/src/lib.rs` (`handler_status_root_v2`, `handler_status_tail_v2`)
- Confirmed Torii route registrations do not expose `/v2/...` paths; active HTTP surface remains `/v1/...` (plus intentional unversioned utility endpoints such as `/status`, `/metrics`, `/api_version`).

### Validation Matrix (Torii /v2 Cleanup)
- `cargo fmt --all`
- `cargo test -p iroha_torii --test api_versioning -- --nocapture`

## 2026-03-07 A1-G1 Closure Follow-up
- Completed Android hard-cut test alignment for encoded-only account/asset identity:
  - `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java`
  - `java/iroha_android/src/test/java/org/hyperledger/iroha/android/norito/NoritoCodecAdapterTests.java`
  - `java/iroha_android/src/test/java/org/hyperledger/iroha/android/tx/TransactionFixtureManifestTests.java`
- Completed JS SDK strict test migration for domainless account ids and encoded-only asset ids:
  - `javascript/iroha_js/test/address_public_key_validation.test.js`
  - `javascript/iroha_js/test/multisigProposalInstruction.test.js`
  - `javascript/iroha_js/test/multisigRegisterInstruction.test.js`
  - `javascript/iroha_js/test/validationError.test.js`
  - `javascript/iroha_js/test/toriiClient.test.js`
  - `javascript/iroha_js/test/toriiIterators.parity.test.js`
- Updated Swift `AccountId.make` to return encoded IH58 identifiers (domainless subject id surface) and aligned affected tests:
  - `IrohaSwift/Sources/IrohaSwift/Crypto.swift`
  - `IrohaSwift/Tests/IrohaSwiftTests/AccountIdTests.swift`
  - `IrohaSwift/Tests/IrohaSwiftTests/BridgeAvailabilityTests.swift`
  - `IrohaSwift/Tests/IrohaSwiftTests/NativeBridgeLoaderTests.swift`
- Static sweep confirms no `parse_any` references remain in Rust/JS/Swift/Android/docs/status/roadmap paths.

### Validation Matrix (Follow-up)
- `cd java/iroha_android && ANDROID_HOME=~/Library/Android/sdk ANDROID_SDK_ROOT=~/Library/Android/sdk ./gradlew :core:test`
- `cd javascript/iroha_js && IROHA_JS_DISABLE_NATIVE=1 npm run test:js`
- `cd IrohaSwift && swift test --filter AccountAddressTests`
- `cd IrohaSwift && swift test --filter AccountIdTests`
- `cd IrohaSwift && swift test --filter 'BridgeAvailabilityTests|BridgeAvailabilitySurfaceTests'`
- `CARGO_TARGET_DIR=target_hardcut cargo check -p iroha_torii`

## 2026-03-07 Account Filter Alias Regression
- Restored state-backed alias resolution for account filter literals in Torii while retaining strict encoded parsing.
- Preserved rejection of legacy `public_key@domain` literals by explicitly excluding them from alias fallback.
- Added a Torii unit regression test covering alias and compressed account literals in `/v1/accounts/query` filters:
  - `crates/iroha_torii/src/routing.rs`

### Validation Matrix (Alias Regression)
- `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii accounts_query_filter_accepts_alias_and_compressed_literals -- --nocapture`
- `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p integration_tests --test address_canonicalisation accounts_query_accepts_alias_and_compressed_filter_literals -- --nocapture`
- `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p integration_tests --test address_canonicalisation accounts_query_rejects_public_key_filter_literals -- --nocapture`
- `cargo fmt --all`

## 2026-03-08 Hard-Cut Debt Sweep
- Removed remaining Python runtime account parse compatibility surface:
  - deleted `AccountAddress.parse_any(...)`
  - removed canonical-hex parser entrypoint from Python account parsing
  - added strict `AccountAddress.parse_encoded(...)` (IH58/sora compressed only; rejects `@domain` and canonical-hex input)
  - updated governance owner canonicalization to strict IH58 decode path.
- Removed positive-path legacy account/asset literals from Python SDK docs/examples/fixtures:
  - `python/iroha_python/README.md`
  - `python/iroha_python/src/iroha_python/examples/tx_flow.py`
  - `python/iroha_python/notebooks/connect_automation.ipynb`
  - `python/iroha_python/tests/test_governance_zk_ballot.py`
  - `python/iroha_python/tests/fixtures/transaction_payload.json`
  - `python/iroha_torii_client/tests/test_client.py`
  - `python/iroha_python/iroha_python_rs/src/lib.rs`
- Unblocked Python client-only import paths without a prebuilt native extension:
  - moved `TransactionConfig`/`TransactionDraft` exports behind the existing optional crypto import gate in `python/iroha_python/src/iroha_python/__init__.py`
  - removed hard runtime `tx` import from `python/iroha_python/src/iroha_python/repo.py` (typing-only dependency)
  - switched `python/iroha_python/src/iroha_python/sorafs.py` to `_native.load_crypto_extension()` with graceful fallback plus built-in alias-policy defaults matching config constants.
- Cleared stale strict-model wording:
  - updated Python transaction helper docs in `python/iroha_python/src/iroha_python/crypto.py` so `authority` is documented as domainless encoded account literal only.
  - updated `docs/fraud_playbook.md` `RiskQuery.subject` schema text to remove optional `@<domain>`/alias hints.
- Added explicit scoped-account naming at domain-bound Rust boundaries:
  - introduced `ScopedAccountId` in `crates/iroha_data_model/src/account.rs`.
  - migrated domain-bound account parse helper signatures to `ScopedAccountId` in:
    - `crates/iroha_core/src/block.rs`
    - `crates/iroha_torii/src/routing.rs`
- Removed residual optional `@<domain>` hint wording from localized fraud docs:
  - `docs/fraud_playbook.{ar,es,fr,he,ja,pt,ru,ur}.md`
  - `docs/source/fraud_monitoring_system.{he,ja}.md`
- Updated data-model doc family wording so `alias@domain` is marked rejected legacy form across `docs/source/data_model*.md` and `docs/source/data_model_and_isi_spec*.md`.
- Closed remaining `scripts/export_norito_fixtures` test breakages introduced by stricter opaque/wire-payload handling:
  - fixed test assumptions in `scripts/export_norito_fixtures/src/main.rs`
  - all tests in that crate pass.
- Static closure sweeps:
  - no runtime `parse_any` account parser references in Rust/JS/Swift/Android/Python source paths.
  - no positive-path `@domain` account literals in active Python SDK source/docs paths.
  - no positive-path legacy textual asset id forms in active Python SDK source/docs paths.

## 2026-03-08 Scoped Naming Completion Sweep
- Completed explicit scoped-account naming migration across remaining high-impact Rust boundaries:
  - `crates/iroha_core/src/block.rs`
  - `crates/iroha_torii/src/routing.rs`
  - `crates/ivm/src/core_host.rs`
  - `crates/iroha_cli/src/main_shared.rs`
  - `crates/iroha_js_host/src/lib.rs`
  - `crates/izanami/src/instructions.rs`
  - `crates/iroha_kagami/src/localnet.rs`
- Exposed `ScopedAccountId` in the data-model account prelude to make domain-bound identity explicit at callsites:
  - `crates/iroha_data_model/src/account.rs`
- Updated `Registrable::build(...)` authority parameter to explicit scoped identity:
  - `crates/iroha_data_model/src/lib.rs`
- Fixed IVM pointer-ABI symbol fallout from the scoped naming sweep while keeping ABI IDs unchanged (`PointerType::AccountId` remains canonical).

### Validation Matrix (Scoped Naming Completion)
- `cargo fmt --all`
- `CARGO_TARGET_DIR=target_hardcut cargo check -p iroha_data_model -p iroha_core -p iroha_torii -p ivm -p iroha_cli -p izanami -p iroha_kagami -p iroha_js_host` (pass)
- `CARGO_TARGET_DIR=target_hardcut cargo test -p iroha_core parse_account_literal_rejects_alias_domain_literals -- --nocapture` (pass)
- `CARGO_TARGET_DIR=target_hardcut cargo test -p iroha_torii accounts_query_filter_rejects_alias_and_accepts_compressed_literals -- --nocapture` (pass)
- `CARGO_TARGET_DIR=target_hardcut cargo test -p ivm pointer_to_norito_roundtrips_via_pointer_from_norito -- --nocapture` (pass)
- `CARGO_TARGET_DIR=target_hardcut cargo test -p iroha_js_host gateway_write_mode_parses_upload_hint -- --nocapture` (pass)

### Validation Matrix (2026-03-08)
- `CARGO_TARGET_DIR=target_hardcut cargo test -p iroha_data_model confidential_wallet_fixtures_are_stable -- --nocapture` (pass)
- `CARGO_TARGET_DIR=target_hardcut cargo test -p iroha_data_model offline_allowance_fixtures_roundtrip -- --nocapture` (pass)
- `CARGO_TARGET_DIR=target_hardcut cargo test --manifest-path scripts/export_norito_fixtures/Cargo.toml -- --nocapture` (pass)
- `CARGO_TARGET_DIR=target_hardcut cargo test --manifest-path python/iroha_python/iroha_python_rs/Cargo.toml attachments_json_decodes_versioned_signed_transaction -- --nocapture` (pass)
- `python -m pytest python/iroha_torii_client/tests/test_client.py -k "uaid_portfolio or space_directory_manifest or trigger_listing_and_lookup_roundtrip or offline_allowance"` in isolated venv (pass: 12 selected tests)
- `PYTHONPATH=python/iroha_python/src:python/iroha_torii_client python -m pytest python/iroha_python/tests/test_governance_zk_ballot.py` in isolated venv (pass: 12 tests)

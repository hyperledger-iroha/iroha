# Status

Last updated: 2026-03-11

## 2026-03-11 I105 Hard-Cut Gap Closure (Repo-wide completion pass)
- Closed remaining hard-cut cleanup gaps across prover tests, integration tests, docs wording, and lint gates:
  - `integration_tests/tests/sumeragi_npos_stake_activation.rs` now provisions NPoS stake/bootstrap state through custom genesis (`genesis_factory_with_post_topology`) instead of stale runtime registration paths that repeated domainless `AccountId` registration.
  - Verified `fastpq_prover` deterministic account helpers are restored to the intended hard-cut-safe structure in:
    - `crates/fastpq_prover/src/bin/fastpq_row_bench.rs`
    - `crates/fastpq_prover/tests/{realistic_flows,backend_regression,proof_fixture,perf_production,transcript_replay}.rs`
  - Fixed stale strict-parser wording in all `docs/account_structure*.md` variants:
    - replaced incorrect `reject canonical I105 and any @domain suffix` text with `reject compressed and any @domain suffix`.
  - Fixed `clippy -D warnings` doc lint fallout in:
    - `crates/iroha_data_model/src/account/address/vectors.rs` (`i105_default` in docs now wrapped in backticks).
- Search acceptance sweeps (this pass):
  - `integration_tests/tests`: no stale bare `*_ID.domain()` callsites; `Account::new(...)` callsites are scoped via `to_account_id(...)` (plus existing `ScopedAccountId` callsites).
  - docs sweeps for stale strict-input phrases (`reject canonical I105`, `optional @domain hint`, `compressed accepted`, `second-best compressed`, strict parser accepting compressed/@domain) returned no active matches in docs surfaces.
- Validation (this pass):
  - `cargo fmt --all` (pass)
  - `cargo check -p fastpq_prover --bins --tests --message-format short` (pass)
  - `cargo check -p integration_tests --tests --message-format short` (pass)
  - `cargo test -p integration_tests --test address_canonicalisation -- --nocapture` (pass, 24 passed)
  - `cargo test -p integration_tests --test multisig -- --nocapture` (pass, 11 passed)
  - `cargo test -p integration_tests --test domain_links -- --nocapture` (pass, 5 passed)
  - `cargo test -p integration_tests --test sumeragi_commit_certificates npos_commit_quorum_requires_stake -- --nocapture` (pass)
  - `cargo test -p integration_tests --test sumeragi_npos_stake_activation -- --nocapture` (pass, 2 passed)
  - `cargo check -p iroha_torii --tests --message-format short` (pass)
  - `cargo check -p iroha_cli --tests --message-format short` (pass)
  - `cargo build --workspace --message-format short` (pass)
  - `cargo clippy --workspace --all-targets -- -D warnings` (pass)
  - `cargo test --workspace` (interrupted by execution environment with exit code `-1` before final summary; long-running run passed broad workspace suites up through large `integration_tests/tests/mod.rs` sections without reporting a concrete test failure before interruption).

## 2026-03-11 I105 Hard-Cut Gap Closure (Explorer QR single-format API/docs cleanup)
- Removed the legacy Rust QR options marker from the client surface:
  - deleted `ExplorerAccountQrOptions` from `crates/iroha/src/client.rs`.
  - simplified `Client::get_explorer_account_qr` to `(&self, account_id: &str)`.
  - updated in-crate call sites/tests accordingly and renamed stale test names:
    - `get_public_lane_validators_omits_query_params`
    - `get_explorer_account_qr_parses_payload_and_omits_query_params`
- Removed `ExplorerAccountQrOptions` references from SDK docs and i18n mirrors:
  - dropped imports/usages in Rust snippets under:
    - `docs/source/nexus_sdk_quickstarts*.md`
    - `docs/portal/docs/sdks/rust*.md`
    - `docs/portal/i18n/*/docusaurus-plugin-content-docs/{current,version-2025-q2}/sdks/rust*.md`
  - normalized QR helper prose to canonical-I105-only wording (no format knob).
- Removed stale JS internal naming to match the new single-format surface:
  - `javascript/iroha_js/{src,dist}/toriiClient.js`:
    - `normalizeExplorerAccountQrOptions` -> `normalizeExplorerRequestOptions`
- Verification (this pass):
  - `cargo test -p iroha --no-run` (pass)
  - `cd javascript/iroha_js && IROHA_JS_DISABLE_NATIVE=1 node --test test/toriiClient.test.js test/toriiIterators.parity.test.js` (fails with 6 existing JS test failures in governance/iterator feature areas; not in Explorer QR option-removal paths)
  - `rg -n 'ExplorerAccountQrOptions|AccountAddressFormat::I105|ih58|IH58' crates javascript python mochi docs/source docs/portal examples integration_tests` (no matches)

## 2026-03-11 I105 Hard-Cut Gap Closure (SDK/example legacy option spelling purge)
- Removed remaining stale option-name usage from active SDK/example surfaces:
  - `examples/android/retail-wallet/.../WalletPreviewViewModel.kt` no longer calls `.addressFormat("canonical")` on `OfflineListParams.Builder`.
  - `docs/source/sdk/android/offline_signing*.md` no longer show `.addressFormat("canonical")`.
  - removed `AddressFormat` imports from Rust quickstart/docs families:
    - `docs/source/nexus_sdk_quickstarts*.md`
    - `docs/portal/docs/sdks/rust*.md`
    - `docs/portal/i18n/*/docusaurus-plugin-content-docs/{current,version-2025-q2}/sdks/rust*.md`
- Normalized JS negative-option tests away from legacy camel-case naming:
  - `javascript/iroha_js/test/toriiClient.test.js` now uses a generic unsupported key (`legacyFormat`) in option-rejection coverage.
- Validation (this pass):
  - `cd javascript/iroha_js && IROHA_JS_DISABLE_NATIVE=1 node --test --test-name-pattern 'getUaidBindings uses canonical query parameters|getUaidManifests appends canonical dataspace filter|listAccounts rejects unsupported legacy option|queryAccounts rejects unsupported legacy option|getExplorerAccountQr rejects unsupported option fields' test/toriiClient.test.js` (pass)
  - `rg -n 'AddressFormat,|\\.addressFormat\\(\"canonical\"\\)|addressFormat' docs examples javascript/iroha_js/test/toriiClient.test.js --glob '!status*.md' --glob '!roadmap*.md'` (no stale option-name matches in active docs/examples/tests; remaining global occurrences are canonical `UnsupportedAddressFormat` error identifiers and changelog history)

## 2026-03-11 I105 Hard-Cut Gap Closure (SDK docs/examples stale format knobs)
- Removed stale format-knob artifacts from docs/examples that still implied multi-format selection:
  - removed `AddressFormat` imports from:
    - `docs/source/nexus_sdk_quickstarts*.md`
    - `docs/portal/docs/sdks/rust*.md`
    - `docs/portal/i18n/*/docusaurus-plugin-content-docs/{current,version-2025-q2}/sdks/rust*.md`
  - removed `.addressFormat("canonical")` from:
    - `examples/android/retail-wallet/src/main/java/org/hyperledger/iroha/samples/wallet/WalletPreviewViewModel.kt`
    - `docs/source/sdk/android/offline_signing*.md`
- Verification (this pass):
  - `rg -n 'AddressFormat,|\\.addressFormat\\(\"canonical\"\\)|addressFormat\\(\"canonical\"\\)' docs examples --glob '!status*.md' --glob '!roadmap*.md'` (no matches)

## 2026-03-11 I105 Hard-Cut Gap Closure (legacy token zero-out outside status/roadmap)
- Completed a repo-wide cleanup pass to remove remaining active `address_format` legacy-token references.
- Test/docs updates:
  - `javascript/iroha_js/test/{toriiClient.test.js,toriiIterators.parity.test.js}`:
    - switched removed-format assertions from `address_format` fields to `canonical_i105`-absence checks.
  - `python/iroha_python/tests/test_address_format.py`:
    - rewrote removed-format assertions/negative kwargs from `address_format` to `canonical_i105`.
  - prior pass already removed stale `address_format` prose from:
    - `docs/source/torii/kaigi_telemetry_api*.md`
    - `docs/source/sns/address_display_guidelines*.md`
    - `ops/runbooks/settlement-buffers.md`
- Validation (this pass):
  - `cd javascript/iroha_js && IROHA_JS_DISABLE_NATIVE=1 node --test --test-name-pattern 'listAccounts encodes iterable params|getUaidManifests includes canonical dataspace query options|queryNfts posts Norito envelope|queryAccountAssets posts filters and options as a Norito envelope' test/toriiClient.test.js test/toriiIterators.parity.test.js` (pass)
  - `cd python/iroha_python && python3 -m pytest tests/test_address_format.py` (environment failure: `No module named pytest`)
  - repo sweep:
    - `rg -n 'ih58|IH58|AddressFormatOption|AccountAddressFormat::I105|AddressFormat::Compressed|fromCompressedSora|toCompressedSora|to_compressed_sora|from_compressed_sora|compressed_address|\\baddress_format\\b|Copy Compressed|Compressed Sora alphabet|Compressed I105 literals|\"canonical_i105\"s\\*:s\\*true|address_format=compressed|address_format=i105\\|compressed|--address-format \\{i105,compressed\\}' --glob '!status*.md' --glob '!roadmap*.md'`
    - result: no matches outside historical status/roadmap logs.

## 2026-03-11 I105 Hard-Cut Gap Closure (Kaigi docs + runbook parameter purge)
- Removed stale `address_format` parameter guidance from Kaigi telemetry API docs:
  - updated `docs/source/torii/kaigi_telemetry_api*.md` to describe canonical-I105-only relay literal output (`relay_id`, `reported_by`) with no format override parameter.
- Removed stale runbook wording that implied an address-format query flag:
  - `ops/runbooks/settlement-buffers.md` now references canonical-I105 receipts directly.
- Removed residual `address_format` wording from SNS address-display guideline variants:
  - updated `docs/source/sns/address_display_guidelines*.md` phrasing from back-compat parameter naming to generic “format-override fields removed” language.
- Verification (this pass):
  - `rg -n '\\baddress_format\\b' docs/source/torii/kaigi_telemetry_api*.md docs/source/sns/address_display_guidelines*.md ops/runbooks/settlement-buffers.md` (no matches)
  - `rg -n '\\baddress_format\\b' docs ops --glob '!status*.md' --glob '!roadmap*.md'` (no matches)
  - repo-wide `address_format` remains only in SDK negative tests that assert removed-parameter rejection paths.

## 2026-03-11 I105 Hard-Cut Gap Closure (Explorer DTO + SNS/contract docs final sweep)
- Closed the remaining runtime naming gap on Explorer account payloads:
  - `crates/iroha_torii/src/explorer.rs`: renamed `compressed_address` to `i105_default_address`.
  - `mochi/mochi-core/src/torii.rs`: aligned parser/model/tests/fixtures to `i105_default_address`.
- Cleared residual legacy wording from final SNS/contract docs and static illustrations:
  - `docs/source/torii_contracts_api*.md`: strict parser sentence now says canonical I105 only (no `@<domain>` suffix), without `reject compressed` artifact text.
  - `docs/source/sns/address_display_guidelines*.md`: replaced `Compressed Sora alphabet`/`Compressed I105 literals` phrasing with `i105-default` wording and removed stale `address_format` toggle block drift in Torii response knobs.
  - `docs/source/references/address_norm_v1*.md`: replaced `compressed-Sora` with `i105-default-Sora`.
  - `docs/source/sns/images/address_copy_*.svg` + `docs/portal/static/img/sns/address_copy_*.svg`: updated remaining “Compressed (`sora`)”/“Copy Compressed” labels to `i105-default`.
- Validation (this pass):
  - `cargo test -p mochi-core explorer_account_record_decodes_payload -- --nocapture` (pass)
  - `cargo test -p mochi-core fetch_explorer_accounts_page_applies_filters -- --nocapture` (pass)
  - `cargo test -p iroha_torii --no-run` (pass)
  - `cargo fmt --all` (pass)
  - focused grep sweeps report no remaining `compressed_address`, `Compressed Sora alphabet`, `reject compressed`, `Copy Compressed`, or `"canonical_i105"s*:s*true` strings in active Explorer/SNS/Torii-contract docs surfaces.

## 2026-03-11 I105 Hard-Cut Gap Closure (example apps + docs alias purge)
- Finished another hard-cut cleanup sweep focused on remaining user-facing legacy wording and stale alias docs:
  - iOS demo (`examples/ios/NoritoDemo`):
    - renamed preview fields from `compressed`/`compressedWarning` to `i105Default`/`i105Warning`.
    - updated copy mode telemetry label from `compressed` to `i105_default`.
    - updated UI copy to “i105-default Sora-only”.
  - Android retail-wallet sample:
    - updated `strings.xml` labels/tooltips/content descriptions from “compressed” wording to `i105-default`.
    - renamed layout IDs/bindings from `address_*_compressed*` to `address_*_i105_default*` in:
      - `examples/android/retail-wallet/src/main/res/layout/activity_main.xml`
      - `examples/android/retail-wallet/src/main/java/org/hyperledger/iroha/samples/wallet/MainActivity.kt`
  - JS tests:
    - removed legacy QR payload `address_format` fixture fields and dropped the “ignores payload address_format field” compatibility test.
    - renamed `maybeTestCompressed` helper in validation tests to `maybeTestI105Default`.
  - Swift/Android test wording:
    - normalized local variable/error message wording from `compressed` to `i105-default` in:
      - `IrohaSwift/Tests/IrohaSwiftTests/{AccountAddressTests,AccountAddressFixtureTests,OfflineNoritoEncodingTests,TransactionInputValidatorTests}.swift`
      - `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java`
  - docs + portal surfaces:
    - removed stale alias list text (`i105`, `compressed`, `ih-b32`, `sora`) from `docs/source/sdk/python/connect_end_to_end*.md` in favor of canonical-I105 wording.
    - removed obsolete CLI docs option `--address-format {i105,compressed}` from `docs/source/nexus_public_lanes*.md`.
    - replaced remaining account-address-status wording (`I105, compressed ('sora'...)`) with `I105 and i105-default ('sora'...)` across `docs/source`, `docs/portal/docs/reference`, and `docs/portal/i18n/.../reference`.
    - updated portal UI copy surfaces:
      - `docs/portal/src/components/ExplorerAddressCard.jsx`
      - `docs/portal/static/img/sns/address_copy_android.svg`
    - removed remaining `to_compressed_sora` and `compressed` address-literal wording from key docs families:
      - `docs/account_structure*.md`
      - `docs/source/data_model*.md`
      - `docs/account_structure_sdk_alignment*.md`
      - `docs/fraud_playbook*.md`
      - `docs/source/fraud_monitoring_system*.md`
      - `docs/source/sdk/js/validation*.md`
      - `docs/source/sdk/android/samples/retail_wallet*.md`
- Verification snapshots (this pass):
  - strict grep: no `ih58`/`IH58` anywhere in repository content.
  - strict grep (excluding historical status/roadmap): no `AddressFormatOption`, `fromCompressedSora`, `toCompressedSora`, `AccountAddressFormat::I105`, `AddressFormat::Compressed`, `format: AccountAddressFormat::I105`.
  - runtime/source grep: no remaining `address_format` field usage in active source paths (`crates/*`, SDK sources, examples), only negative/assertion coverage in tests.
- Validation (this pass):
  - `cd javascript/iroha_js && IROHA_JS_DISABLE_NATIVE=1 node --test test/validationError.test.js` (pass)
  - `cd javascript/iroha_js && IROHA_JS_DISABLE_NATIVE=1 node --test --test-name-pattern 'getExplorerAccountQr normalizes payloads|getExplorerAccountQr rejects unsupported option fields' test/toriiClient.test.js` (pass)
  - `cd IrohaSwift && swift test --filter AccountAddressFixtureTests/testNegativeVectorsReject` (build ok; test runner exits with unexpected signal code 5 in this environment after launching XCTest; no assertion failure output)

## 2026-03-11 I105 Hard-Cut Gap Closure (OpenAPI + SNS docs/test naming)
- Removed remaining hard-cut aliases/tokens from active API/test surfaces in this pass:
  - dropped `MissingCompressedSentinel` fallback branches in vector consumer tests:
    - `crates/iroha_data_model/tests/account_address_vectors.rs`
    - `crates/iroha_torii/tests/account_address_vectors.rs`
    - `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift`
    - `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressFixtureTests.swift`
    - `javascript/iroha_js/test/address.test.js`
  - Android sample parity update:
    - `java/iroha_android/samples-android/src/test/java/org/hyperledger/iroha/android/samples/SampleAddressTest.java` now asserts `formats.i105Default`.
  - JavaScript test naming/fixtures normalized away from `compressed` property labels in active suites:
    - `javascript/iroha_js/test/address.test.js`
    - `javascript/iroha_js/test/validationError.test.js`
    - `javascript/iroha_js/test/instructionBuilders.test.js`
    - `javascript/iroha_js/test/toriiClient.test.js`
    - `javascript/iroha_js/test/integrationTorii.test.js`
  - regenerated Torii OpenAPI current snapshot and synced latest static spec:
    - `docs/portal/static/openapi/versions/current/torii.json`
    - `docs/portal/static/openapi/torii.json`
    - result: no `address_format`/`compressed` account-format enum in current published spec.
  - continued localized SNS guideline cleanup for stale field names and copy-mode tokens:
    - `docs/source/sns/address_display_guidelines*.md`
    - `docs/portal/docs/sns/address-display-guidelines*.md`
    - `docs/portal/i18n/*/docusaurus-plugin-content-docs/current/sns/address-display-guidelines*.md`
- Validation (this pass):
  - `cd javascript/iroha_js && IROHA_JS_DISABLE_NATIVE=1 node --test test/address.test.js` (pass)
  - `cd javascript/iroha_js && IROHA_JS_DISABLE_NATIVE=1 node --test test/address.test.js test/validationError.test.js test/instructionBuilders.test.js` (pass)
  - `cd docs/portal && npm run sync-openapi -- --latest` (spec regenerated via `cargo run -p xtask -- openapi`; manifest signing check failed as expected without signing key; static spec files updated)
  - `cmp -s docs/portal/static/openapi/torii.json docs/portal/static/openapi/versions/current/torii.json` (identical)

## 2026-03-11 I105 Hard-Cut Gap Closure (JavaScript SDK public symbols)
- Removed remaining legacy JS SDK public method names tied to compressed-era wording:
  - `AccountAddress.fromCompressedSora(...)` -> `AccountAddress.fromI105Default(...)`
  - `AccountAddress.toCompressedSora()` -> `AccountAddress.toI105Default()`
  - `AccountAddress.toCompressedSoraFullWidth()` -> `AccountAddress.toI105DefaultFullWidth()`
- Updated corresponding type docs and package docs:
  - `javascript/iroha_js/index.d.ts`
  - `javascript/iroha_js/README.md`
- Updated JS test surfaces to the new method names and regenerated package dist:
  - `javascript/iroha_js/test/address.test.js`
  - `javascript/iroha_js/test/address_inspect.test.js`
  - `javascript/iroha_js/test/validationError.test.js`
  - `javascript/iroha_js/test/instructionBuilders.test.js`
  - `javascript/iroha_js/test/toriiClient.test.js`
  - `javascript/iroha_js/dist/address.js`
- Validation (this pass):
  - `cd javascript/iroha_js && npm run build:dist` (pass)
  - `cd javascript/iroha_js && IROHA_JS_DISABLE_NATIVE=1 node --test test/address.test.js test/address_inspect.test.js test/validationError.test.js test/instructionBuilders.test.js` (pass)
  - `cd javascript/iroha_js && IROHA_JS_DISABLE_NATIVE=1 node --test --test-name-pattern \"listAccountPermissions normalizes I105 and compressed account ids\" test/toriiClient.test.js` (pass)

## 2026-03-11 I105 Hard-Cut Gap Closure (Swift/Android + Fixture Schema)
- Closed the remaining SDK/schema gaps from the hard-cut follow-up:
  - Swift (`IrohaSwift`):
    - `NativeAccountAddressRenderResult` now uses `i105Default`/`i105DefaultFullWidth` (removed legacy `compressed` field name).
    - `AccountAddress.displayFormats(...)` now returns `i105Default` consistently (bridge and fallback paths aligned).
    - fixture decoders/tests now read `encodings.i105_default` and `encodings.i105_default_fullwidth`, and negative fixture handling uses `format: "i105_default"`.
  - Android (`java/iroha_android`):
    - renamed legacy API surface in `AccountAddress` to I105-default naming:
      - `fromI105Default(...)`, `toI105Default()`, `toI105DefaultFullWidth()`, `i105WarningMessage()`.
      - `DisplayFormats` fields now expose `i105Default` + `i105Warning`.
      - parser format marker now uses `Format.I105_DEFAULT` for sentinel forms (legacy `COMPRESSED` symbol removed).
    - updated Android address tests + retail-wallet sample callsites to the same names.
  - Fixture/schema hard-cut:
    - compliance/vector generators now emit `i105_default` / `i105_default_fullwidth` keys.
    - negative vectors now use `format: "i105_default"` and `i105_default-*` case ids.
    - refreshed fixture bundle: `fixtures/account/address_vectors.json`.
  - Consumer alignment:
    - updated Rust vector consumers:
      - `crates/iroha_data_model/tests/account_address_vectors.rs`
      - `crates/iroha_torii/tests/account_address_vectors.rs`
    - updated JS vector consumer tests: `javascript/iroha_js/test/address.test.js`.
    - updated JS host render payload naming and JS adapter mapping:
      - `crates/iroha_js_host/src/lib.rs`
      - `javascript/iroha_js/src/address.js`
      - `javascript/iroha_js/dist/address.js`
- Validation (this pass):
  - `rustfmt --edition 2024 crates/iroha_data_model/src/account/address/compliance_vectors.rs crates/iroha_data_model/src/account/address/vectors.rs crates/iroha_data_model/tests/account_address_vectors.rs crates/iroha_torii/tests/account_address_vectors.rs crates/iroha_js_host/src/lib.rs` (pass)
  - `cargo run -p xtask --bin xtask -- address-vectors --out fixtures/account/address_vectors.json` (pass)
  - `cargo check -p iroha_data_model -p iroha_torii -p iroha_cli -p iroha_js_host` (pass)
  - `cargo test -p integration_tests --test address_canonicalisation --no-run` (pass)
  - `cargo test -p iroha_data_model --test account_address_vectors --no-run` (pass)
  - `cargo test -p iroha_torii --test account_address_vectors --no-run` (pass)
  - `cd IrohaSwift && swift build` (pass)
  - `cd java/iroha_android && JAVA_HOME=$(/usr/libexec/java_home -v 21) ANDROID_HOME=~/Library/Android/sdk ANDROID_SDK_ROOT=~/Library/Android/sdk ./gradlew :android:compileDebugJavaWithJavac :android:compileDebugUnitTestJavaWithJavac` (pass)
  - `cd javascript/iroha_js && IROHA_JS_DISABLE_NATIVE=1 node --test test/address.test.js` (pass)

## 2026-03-11 I105 Hard-Cut Follow-up (Legacy `compressed`/`sora` wording removal)
- Removed remaining account-literal legacy wording in active Rust/Java/docs surfaces touched in this pass:
  - `crates/iroha_cli/src/address.rs` (`I105/sora` module/help/error strings → canonical I105 wording).
  - `crates/iroha_data_model/src/account.rs` parser docs updated to “dotted/non-canonical I105 literals”.
  - `crates/iroha_cli/src/main_shared.rs`, `crates/iroha_cli/src/commands/sorafs.rs`, `crates/iroha_torii/src/lib.rs`, `crates/iroha_torii/src/gov.rs`, `crates/iroha_torii/src/routing.rs` test/helper names and assertions now use I105/non-canonical-I105 wording (removed `compressed literal` naming).
  - `integration_tests/tests/address_canonicalisation.rs` helper/test/assertion names normalized away from `compressed` to explicit I105 vs legacy dotted-I105 terms.
  - Android SDK text validation updates:
    - `java/iroha_android/.../AccountIdLiteral.java`
    - `.../ConnectCrypto.java`
    - `.../MultisigRegisterInstruction.java`
    - `.../TransactionPayloadAdapter.java`
    - `.../OfflineSpendReceiptPayloadEncoder.java`
    - updated corresponding tests in `AccountLiteralHardCutTests` and `NoritoCodecAdapterTests`.
  - IVM docs wording updates:
    - `crates/ivm/docs/tlv_examples.md`
    - `crates/ivm/docs/syscalls.md`
  - Address RFC/docs alignment updates:
    - `docs/account_structure.md`
    - `docs/account_structure_sdk_alignment.md`
    - `docs/source/account_address_status.md`
    - `docs/portal/docs/reference/account-address-status.md`
    - `docs/portal/docs/reference/address-safety.md`
  - Script docs wording updates:
    - `scripts/offline_topup/README.md`
    - `scripts/offline_pos_provision/README.md`
- Validation (this pass):
  - `rustfmt --edition 2024 crates/iroha_cli/src/address.rs crates/iroha_data_model/src/account.rs crates/iroha_cli/src/main_shared.rs crates/iroha_cli/src/commands/sorafs.rs crates/iroha_torii/src/lib.rs crates/iroha_torii/src/gov.rs crates/iroha_torii/src/routing.rs integration_tests/tests/address_canonicalisation.rs` (pass)
  - `cargo check -p iroha_cli -p iroha_torii` (pass)
  - `cargo test -p integration_tests --test address_canonicalisation --no-run` (pass)

## 2026-03-11 I105 Hard-Cut Follow-up (Code-Level Strings + SDK Doc Sweep)
- Removed remaining code/help wording that still advertised dual-format `I105/sora` behavior:
  - `crates/iroha_data_model/src/account.rs`
  - `crates/iroha_cli/src/main_shared.rs`
  - `crates/iroha_cli/src/commands/sns.rs`
  - `crates/iroha_js_host/src/lib.rs`
  - `xtask/src/main.rs`
  - `IrohaSwift/Sources/IrohaSwift/AccountAddress.swift`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/address/AccountAddress.java`
- Regenerated CLI markdown help after comment/help text updates:
  - `cargo run -p iroha_cli --bin iroha -- tools markdown-help > crates/iroha_cli/CommandLineHelp.md`
- Continued SDK docs hard-cut cleanup:
  - removed stale `addressFormat`/`address_format` argument examples from JS/Python/Swift SDK docs where APIs are now canonical I105-only.
  - normalized `docs/source/sdk/js/quickstart*.md` explorer QR snippets to no-option `getExplorerAccountQr("i105...")` usage and canonical I105 wording.
  - normalized `docs/source/sdk/python/index*.md` and `connect_end_to_end*.md` QR helper wording to canonical I105 output.
  - normalized `docs/source/sdk/swift/index*.md` QR/address sections to canonical I105 wording and removed stale `addressFormat: .compressed` snippets.
  - applied a follow-up docs sweep across `docs/`, `docs/source/`, and `docs/portal/` to remove remaining explicit `second-best`/`compressed (`sora`)` account-literal wording on address-format examples and QR snippets.
- Final CLI help cleanup:
  - updated `crates/iroha_cli/src/address.rs` parse-argument help text to canonical I105 wording.
  - regenerated `crates/iroha_cli/CommandLineHelp.md` again so the published help no longer references `sora…` parsing aliases.
- Search verification (this pass):
  - no matches for live legacy literals in docs/help surfaces for:
    - `addressFormat: "compressed"`
    - `address_format="compressed"`
    - `--address-format compressed`
    - `I105 (preferred)/sora`
    - `compressed (\`sora\`)`
- Validation (this pass):
  - `rustfmt --edition 2024 crates/iroha_data_model/src/account.rs crates/iroha_cli/src/commands/sns.rs crates/iroha_cli/src/main_shared.rs crates/iroha_js_host/src/lib.rs xtask/src/main.rs` (pass)
  - `rustfmt --edition 2024 crates/iroha_cli/src/address.rs` (pass)
  - `cargo check -p iroha_cli -p xtask -p iroha_data_model -p iroha_js_host` (pass)
  - `cargo check -p iroha_cli -p xtask` (pass)

## 2026-03-11 CLI Address Single-Format Surface Follow-up
- `iroha tools address` no longer advertises/accepts a separate `compressed` output format:
  - removed `OutputFormat::Compressed` from `crates/iroha_cli/src/address.rs`.
  - normalized JSON/CSV summary payloads to `i105` + `canonical_hex` fields only.
  - regenerated `crates/iroha_cli/CommandLineHelp.md` from live clap output (`cargo run -p iroha_cli --bin iroha -- tools markdown-help`).
- CLI smoke tests updated for the single-format output schema and current stream behavior:
  - `address_convert_json_summary_contains_i105_and_canonical_hex`.
  - `address_audit_supports_csv_output` now tolerates CLI banner lines and stdout/stderr routing.
- Android SDK docs alignment:
  - removed stale `AddressFormatOption` and `address_format` override guidance from `docs/source/sdk/android/index*.md`; UAID docs now describe canonical I105-only output.
- Additional docs hard-cut sweep:
  - removed remaining explicit `address_format=compressed`, `address_format=i105|compressed`, and `AddressFormat::Compressed` snippets from `docs/`, `docs/source/`, and `docs/portal/` markdown surfaces.
- JavaScript targeted test adjustment:
  - `javascript/iroha_js/test/toriiClient.test.js` explorer QR payload fixture updated to I105 wording for the legacy-field-ignore assertion.
- Validation (this pass):
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_cli --test cli_smoke --no-run` (pass)
  - `cargo test -p iroha_cli --test cli_smoke address_convert_json_summary_contains_i105_and_canonical_hex -- --nocapture` (pass)
  - `cargo test -p iroha_cli --test cli_smoke address_audit_supports_csv_output -- --nocapture` (pass)
  - `cd javascript/iroha_js && IROHA_JS_ALLOW_UNVERIFIED_NATIVE=1 node --test --test-name-pattern "getExplorerAccountQr ignores payload address_format field" test/toriiClient.test.js` (pass)

## 2026-03-11 Repository-Wide Token Cleanup (Android + CLI + Docs + Tooling)
- Removed remaining legacy account-literal token usage across non-portal docs, Android SDK/sample surfaces, CLI docs/tests, Torii client helper docs, and tooling text paths.
- Android SDK and sample updates:
  - renamed public/account-literal method and constant usage to I105 naming in `java/iroha_android` sources/tests/docs (`toI105`, `fromI105`, `DEFAULT_I105_DISCRIMINANT`, related literals/messages).
  - updated `samples-android` address preview test/API references to I105 naming.
- Rust/tooling updates:
  - `crates/iroha_cli`: removed residual legacy wording from docs/help/tests and renamed affected test identifiers.
  - `mochi/mochi-core`: explorer account record field renamed to `i105_address` with decoder/test fixture updates.
  - `ci/check_address_normalize.sh`: switched fixture extraction + normalize output target to I105 naming (`--format i105`) and removed legacy fallback paths.
  - `xtask` and runbook/dashboard/readme strings updated to I105 wording.
- Documentation sweep:
  - applied repo-wide wording replacement under `docs/` so remaining docs now use I105 naming.
- Search-based acceptance:
  - repository-wide grep for legacy account-literal tokens returns no matches.
- Validation (this pass):
  - `cd java/iroha_android && ... ./gradlew :android:compileDebugJavaWithJavac :android:compileDebugUnitTestJavaWithJavac :samples-android:compileDebugUnitTestJavaWithJavac` (pass)
  - `cd java/iroha_android && ./gradlew :core:test --tests "*AccountAddressTests" --tests "*AccountIdLiteralTests" --tests "*AccountLiteralHardCutTests" --tests "*NoritoCodecAdapterTests"` (pass)
  - `cargo check -p mochi-core` (pass)
  - `cargo test -p iroha_cli --test cli_smoke --no-run` (pass)
  - `cargo check -p xtask` (pass)
  - `cargo fmt --all` (pass)

## 2026-03-11 I105-Only Cleanup (JavaScript SDK follow-up)
- Completed the in-progress JS SDK migration to I105-only naming and exports in `javascript/iroha_js`.
- Public API alignment:
  - `src/index.js` now exports `encodeI105AccountAddress` / `decodeI105AccountAddress` (legacy compressed export names removed).
  - `index.d.ts` aligned with current runtime API:
    - I105 error-code identifiers,
    - `chainDiscriminant`/`expectDiscriminant` option names,
    - `inspectAccountId` and `displayFormats` shapes (I105-only fields).
- Address/normalizer wording and validation paths:
  - removed remaining “I105 (preferred) or sora compressed” legacy wording in `src/normalizers.js`;
    account-id validation messages now describe canonical I105 only.
- JS package-wide token cleanup:
  - removed legacy identifiers from JS package source/test/docs/recipes/changelog files.
  - updated address-focused tests to current I105 semantics (sentinel/discriminant names, inspect output fields, and fixture compatibility shims for legacy vector payloads).
- Dist sync:
  - `npm run build:dist` refreshed `javascript/iroha_js/dist/*` from `src/*`.
- Validation (this pass):
  - `node -e "import('./javascript/iroha_js/src/index.js')..."` (pass)
  - `cd javascript/iroha_js && IROHA_JS_DISABLE_NATIVE=1 node --test test/address.test.js test/address_inspect.test.js` (pass)
  - `cd javascript/iroha_js && IROHA_JS_DISABLE_NATIVE=1 node --test test/address.test.js test/address_inspect.test.js test/validationError.test.js` (pass)
  - `cd javascript/iroha_js && npm run build:dist` (pass)
  - `cd javascript/iroha_js && npm run lint ...` (blocked: ESLint config file not present in this environment)

## 2026-03-11 I105-Only Cleanup (Python SDK + Torii Python client + Swift follow-up)
- Continued hard-cut removal of legacy terminology and parsing paths in Python and Swift slices.
- Python Torii client (`python/iroha_torii_client/client.py`):
  - replaced I105-specific owner validation decoder with canonical I105 decoding (sentinel + bech32m checksum path).
  - removed legacy constants/error text from the module; governance owner canonical checks now validate I105 literals.
- Python SDK (`python/iroha_python`):
  - `address.py` converted to I105-only API surface:
    - replaced dual I105/compressed helpers with `from_i105` / `to_i105` and I105 sentinel-discriminant encode/decode logic.
    - `parse_encoded` now accepts canonical I105 only (hex literals remain rejected).
  - `client.py` governance canonical-owner checks now require canonical I105 (`parse_encoded(...expected_discriminant...)` + `to_i105` round-trip).
  - `crypto.py` account-id helpers now emit canonical I105 literals and use `discriminant` naming.
  - updated Python README/examples/tests in this slice to remove I105 wording and use I105 terminology.
- Swift follow-up (`IrohaSwift`):
  - updated high-traffic test files to replace local `i105` naming with `i105`.
  - fixed `AccountAddress.fromI105` / `toI105` fallback paths to use I105 sentinel+checksum encode/decode helpers (instead of I105 helper fallback).
  - removed dead I105 helper implementations from `AccountAddress.swift`; canonical encode/decode fallback now only uses I105 helpers.
  - renamed remaining Swift `AccountAddress` I105-specific error identifiers/codes used in this slice to I105 naming (`invalidI105*`, `ERR_INVALID_I105_*`) and updated corresponding `AccountAddressTests` expectations.
  - preserved bridge-first behavior; local fallback now matches I105 semantics.
- Validation (this pass):
  - `python3 -m py_compile python/iroha_torii_client/client.py python/iroha_torii_client/tests/test_client.py` (pass)
  - `python3 -m py_compile python/iroha_python/src/iroha_python/address.py python/iroha_python/src/iroha_python/client.py python/iroha_python/src/iroha_python/crypto.py python/iroha_python/src/iroha_python/examples/tx_flow.py python/iroha_python/tests/test_governance_zk_ballot.py` (pass)
  - `cd IrohaSwift && swift build` (pass)
  - `cd IrohaSwift && swift test --filter AccountIdTests` (build+selected tests run; process exits with unexpected signal 11 in this environment)
  - `cd IrohaSwift && swift test --filter AccountAddressTests` (build+selected tests start; process exits with unexpected signal 5 in this environment)

## Current Enforced State
- First-release identity/deploy policy is strict (no backward aliases, shims, or migration wrappers).
- Historical notes below record intermediate steps; when any older entry conflicts with the current hard-cut semantics, this section and the latest 2026-03-11 hard-cut entry are authoritative.
- Deploy preflight scanner entrypoint is `../pk-deploy/scripts/check-identity-surface.sh`; the previous scanner entrypoint is removed.
- Runtime deploy scripts are aligned to strict-first-release behavior:
  - `../pk-deploy/scripts/cutover-i105-mega.sh` invokes only `check-identity-surface.sh`.
  - `../pk-deploy/scripts/deploy-sbp-aed-pkr-interceptor.sh` no longer performs prior-layout trigger cleanup loops.
- Wallet docs describe only current QR modes in neutral terms.

## 2026-03-11 Torii + SDK Legacy `address_format` Surface Cleanup (continuation)
- Continued the I105-only hard-cut by removing remaining request/DTO `address_format` surfaces and compatibility wrappers in Torii + mobile/client SDK paths.
- Torii Rust cleanup:
  - `crates/iroha_torii/src/address_format.rs`
    - removed `AddressFormatPreference` wrapper type; canonical helpers are now used directly (`display_literal`, `display_from_literal`, `metric_label`).
  - `crates/iroha_torii/src/routing.rs`
    - replaced `AddressFormatPreference` callsites with direct canonical helper calls.
    - fixed comments to describe canonical I105 rendering (instead of format-preference wording).
  - `crates/iroha_torii/tests/address_parsing.rs`
    - removed ignored legacy `address_format` test blocks and stale helper scaffolding.
    - cleaned now-unused constants after deleting legacy blocks.
- Android SDK cleanup:
  - removed `address_format` fields/query emission from:
    - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/OfflineQueryEnvelope.java`
    - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/OfflineListParams.java`
    - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/nexus/UaidBindingsQuery.java`
    - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/nexus/UaidManifestQuery.java`
  - removed legacy enum:
    - deleted `java/iroha_android/src/main/java/org/hyperledger/iroha/android/nexus/AddressFormatOption.java`
  - removed `ExplorerAccountQrSnapshot.addressFormat` field:
    - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/explorer/ExplorerAccountQrSnapshot.java`
  - updated Android tests expecting prior `address_format` behavior:
    - `java/iroha_android/src/test/java/org/hyperledger/iroha/android/offline/OfflineQueryEnvelopeTest.java`
    - `java/iroha_android/src/test/java/org/hyperledger/iroha/android/client/OfflineToriiClientTests.java`
    - `java/iroha_android/src/test/java/org/hyperledger/iroha/android/client/HttpClientTransportTests.java`
- Swift SDK cleanup:
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift`
    - removed `ToriiExplorerAccountQr.addressFormat` and JSON coding key mapping.
  - `IrohaSwift/Tests/IrohaSwiftTests/ToriiClientTests.swift`
    - removed assertions and fixture dependence on QR payload `address_format`.
- Python Torii client tests:
  - `python/iroha_torii_client/tests/test_client.py`
    - removed request/payload `address_format` expectations from explorer QR and UAID tests.
    - removed legacy “ignore legacy address_format” test case.
- Validation (this pass):
  - `cargo fmt --all` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-check cargo check -p iroha_torii --features app_api,telemetry` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-check cargo test -p iroha_torii --features app_api,telemetry --test address_parsing transactions_endpoint_accepts_encoded_account_segments -- --nocapture` (pass)
  - `python3 -m py_compile python/iroha_torii_client/tests/test_client.py` (pass)
  - `cd IrohaSwift && swift build` (pass)
  - `cd IrohaSwift && swift test --filter ToriiClientTests/testGetExplorerAccountQrDecodesResponse` (build passes; runtime exits with signal 11 in this environment)
  - `cd java/iroha_android && ./gradlew projects` (pass)
  - `cd java/iroha_android && JAVA_HOME=$(/usr/libexec/java_home -v 21) ANDROID_HOME=~/Library/Android/sdk ANDROID_SDK_ROOT=~/Library/Android/sdk ./gradlew :android:compileDebugJavaWithJavac :android:compileDebugUnitTestJavaWithJavac` (pass)

## 2026-03-11 Hard-Cut Cleanup Completion (Repo-wide)
- Completed the remaining hard-cut cleanup blockers and confirmed current end state:
  - `AccountId` strict parser paths remain canonical I105-only.
  - domain scope is explicit (`ScopedAccountId` / domain-link surfaces), not inferred from bare `AccountId`.
  - compressed `sora...` account literals remain only on explicit output/display/address-format surfaces.
- Final code/test cleanup completed:
  - `crates/fastpq_prover` deterministic account helper/test boundary restoration remains compiling cleanly across bin/tests.
  - `integration_tests/tests/triggers/orphans.rs`
    - trigger setup now grants `CanRegisterTrigger` to the generated authority account and registers triggers through that authority client.
    - domain-removal assertion updated to the domainless-authority hard-cut behavior (`trigger_must_survive_action_authority_domain_removal`).
  - `integration_tests/tests/triggers/time_trigger.rs`
    - `time_trigger_scenarios` now validates per-owner NFT mint deltas from baseline ownership counts instead of stale ID-format string matching.
- Documentation strict-input wording sweep completed for remaining stale phrases:
  - removed residual “compressed accepted”, “I105/sora literals”, and “optional/appended `@domain` hint” wording from strict parser paths in:
    - `docs/source/sdk/js/validation*.md`
    - `docs/source/cbdc_lane_playbook*.md`
    - `docs/source/fraud_monitoring_system.{he,ja}.md`
    - `docs/source/governance_api.{ba,dz,zh-hans}.md`
    - `docs/fraud_playbook*.md`
- Search-based acceptance:
  - `integration_tests/tests`: no stale `Account::new(AccountId)` callsites remain.
  - `integration_tests/tests`: no bare sample-ID `.domain()` usages remain.
  - `docs`/`docs/source`/`docs/portal`: no stale strict-input phrase hits for:
    - `compressed accepted`
    - `I105/sora literals`
    - `optional @domain hint`
    - `append @domain only as an explicit routing hint`
- Validation matrix (this completion pass):
  - `cargo fmt --all` (pass)
  - `CARGO_TARGET_DIR=target_hardcut3 cargo check -p fastpq_prover --bins --tests --message-format short` (pass)
  - `CARGO_TARGET_DIR=target_hardcut3 cargo check -p integration_tests --tests --message-format short` (pass; warnings only)
  - `CARGO_TARGET_DIR=target_hardcut3 cargo test -p integration_tests --test address_canonicalisation -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=target_hardcut3 cargo test -p integration_tests --test multisig -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=target_hardcut3 cargo test -p integration_tests --test domain_links -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=target_hardcut3 cargo test -p integration_tests --test mod triggers::orphans -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=target_hardcut3 cargo test -p integration_tests --test mod triggers::time_trigger::time_trigger_scenarios -- --nocapture` (pass after baseline-delta fix)
  - `CARGO_TARGET_DIR=target_hardcut3 cargo check -p iroha_torii --tests --message-format short` (pass)
  - `CARGO_TARGET_DIR=target_hardcut3 cargo check -p iroha_cli --tests --message-format short` (pass)
  - `CARGO_TARGET_DIR=target_hardcut3 cargo build --workspace --message-format short` (pass; warning in `mochi-ui-egui` unchanged)
- Notes:
  - A pre-fix long-running `cargo test -p integration_tests --test mod -- --nocapture` session (started before these patches) failed with now-resolved orphan/time-trigger expectations.
  - `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` were not re-run in this completion pass due runtime cost.

## 2026-03-10 Torii I105-Only Request Surface (Explorer/Kaigi/Nexus/Space Directory slice)
- Continued hard-cut removal of request-side format selectors in Torii Rust endpoints:
  - `crates/iroha_torii/src/explorer.rs`
    - removed `ExplorerPaginationQuery.address_format`.
    - removed `ExplorerAddressFormatQuery` + parser helpers.
    - `ExplorerAccountQrDto::build` now emits canonical I105 directly (no caller-supplied format).
    - renamed explorer account payload key from `i105_address` to `i105_address`.
  - `crates/iroha_torii/src/lib.rs`
    - removed explorer detail/QR query parsing for `address_format`; handlers now invoke routing with fixed `AddressFormatPreference::I105`.
  - `crates/iroha_torii/src/routing.rs`
    - removed `address_format` fields from:
      - `AccountTransactionsGetParams`
      - `SpaceDirectoryManifestQuery`
    - converted format-only query DTOs to non-format placeholders to keep extractor compatibility while removing format knobs:
      - `KaigiRelayFormatParams`
      - `PublicLaneValidatorsQueryParams`
      - `NexusDataspacesAccountSummaryQueryParams`
      - `SpaceDirectoryBindingsQuery`
    - locked these handlers to canonical I105 output (no request format parsing):
      - account transactions GET/query
      - explorer transactions/instructions list/latest
      - kaigi relay list/detail
      - nexus dataspaces account summary
      - nexus public lane validators/stake/rewards
      - space-directory bindings/manifests
  - naming cleanup: removed remaining `i105` identifiers in modified Torii/data-model code paths:
    - `crates/iroha_torii/src/lib.rs` test naming/variables
    - `crates/iroha_torii/src/routing.rs` helper/test naming
    - `crates/iroha_data_model/src/account/address/compliance_vectors.rs` variable naming
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_torii --no-run` (pass)
  - `cargo test -p iroha_torii i105_literal -- --nocapture` (pass; matched unit tests succeeded, no failures)

## 2026-03-10 Torii Spec/Test Hard-Cut Continuation (OpenAPI/MCP + endpoint tests)
- Removed request-side `address_format` exposure from Torii API specs:
  - `crates/iroha_torii/src/openapi.rs`
    - removed `address_format` query parameters from list/detail/query operations that now hard-cut to canonical I105 output.
    - removed `address_format` fields from request-envelope schemas.
    - updated account/stake/validator description text to canonical I105 wording.
    - updated OpenAPI unit assertions to stop requiring `address_format` parameters.
  - `crates/iroha_torii/src/mcp.rs`
    - removed `address_format` from MCP tool input schemas and QueryEnvelope shortcut builder.
    - normalized affected descriptions to generic optional query wording.
- Simplified Torii formatter utility to I105-only rendering:
  - `crates/iroha_torii/src/address_format.rs`
    - removed now-unused query parsing helper (`from_param`) and associated tests.
    - kept canonical I105 display helpers used by routing/explorer projection paths.
- Rebased/continued Torii tests toward I105-only semantics:
  - `crates/iroha_torii/tests/address_parsing.rs`
    - switched literals/comments to I105 naming.
    - removed active execution of legacy `address_format` behavior checks by marking them ignored with explicit hard-cut reason.
    - adjusted default-domain helper coverage to canonical I105 rendering and URL-encoded validator query literals.
  - `crates/iroha_torii/tests/offline_transfer_detail.rs`
    - removed `address_format` query args from detail-route requests.
  - `crates/iroha_torii/tests/offline_revocations.rs`
    - removed `address_format` query/body usage; kept sorting/filtering assertions on canonical output.
  - `crates/iroha_torii/tests/space_directory_manifests.rs`
    - removed `address_format` rewrite assertions and redundant query variants.
  - `crates/iroha_torii/src/routing.rs` test names/fixtures in touched areas now describe canonical I105 behavior instead of compressed/format preference semantics.
- Validation (latest slice):
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_torii --no-run` (pass)
  - `cargo test -p iroha_torii --test address_parsing --features app_api,telemetry` (pass; legacy format tests ignored by design in this hard-cut slice)
  - `cargo test -p iroha_torii openapi::tests::account_and_asset_list_params_include_asset_id_filter -- --nocapture` (pass)
  - `cargo test -p iroha_torii mcp:: -- --nocapture` (in progress while traversing filtered binaries; no failures observed in completed unit set)

## 2026-03-10 Full Hard-Cut Cleanup (Repo-wide continuation)
- Completed remaining hard-cut compile/test cleanup work:
  - `crates/connect_norito_bridge/src/lib.rs`
    - fixed `multisig_register_encoder_success` to pass a scoped account (`<account>@default`) to the scoped parser path.
    - updated signed-fixture decode/reencode tests to tolerate opaque fixture payloads while keeping explicit bare+framed decode coverage on a synthetic `SignedTransaction`.
  - additional repo-wide stale `AccountId`/`ScopedAccountId` call-site repairs were completed across `ivm`, `iroha_core` benches/examples, `iroha_test_network`, `irohad`, `tools/*`, `xtask`, `python` bindings, `mochi`, and `integration_tests` (domainless `AccountId` + explicit domain scoping at registration boundaries).
- Search-based acceptance sweeps:
  - `integration_tests/tests`: no stale `Account::new(AccountId)` callsites remain.
  - `.domain()` on bare/domainless IDs: only legitimate scoped usage remains (`integration_tests/tests/address_canonicalisation.rs` asset-definition domain path).
  - strict-input stale phrase sweeps (`compressed accepted`, `optional @domain hint`, `canonicalizes ... second-best compressed`) return no hits.
- Validation run highlights:
  - `cargo fmt --all` (pass)
  - `cargo check -p fastpq_prover --bins --tests --message-format short` (pass)
  - `cargo check -p integration_tests --tests --message-format short` (pass; warnings only)
  - `cargo test -p integration_tests --test address_canonicalisation -- --nocapture` (pass)
  - `cargo test -p integration_tests --test multisig -- --nocapture` (pass)
  - `cargo test -p integration_tests --test domain_links -- --nocapture` (pass)
  - `cargo check -p iroha_torii --tests --message-format short` (pass)
  - `cargo check -p iroha_cli --tests --message-format short` (pass)
  - `cargo build --workspace --message-format short` (pass)
- Notes:
  - several integration suites emit long `network guard has waited ... 1/1 permits in use` waits under constrained local test-network concurrency but still complete successfully.
  - full `cargo test --workspace` / `cargo clippy --workspace --all-targets -- -D warnings` remain runtime-heavy and were still in progress at the time this status entry was updated.

## 2026-03-10 I105-Only Client Surface Hard-Cut (Python + JS Iterables)
- Python SDK (`iroha_python`) no longer accepts/emits `address_format` for iterable account/holder surfaces:
  - `python/iroha_python/src/iroha_python/query.py`
    - removed `QueryEnvelope.address_format`.
    - removed `address_format` arguments from `account_query_envelope` and `asset_holders_query_envelope`.
  - `python/iroha_python/src/iroha_python/client.py`
    - removed address-format normalizer/alias helpers.
    - removed `address_format` params from:
      - `query_accounts` / `query_accounts_typed`
      - `list_accounts` / `list_accounts_typed`
      - `query_asset_holders` / `query_asset_holders_typed`
      - `list_asset_holders` / `list_asset_holders_typed`
    - removed offline list alias rewriting that mapped `address_format`.
  - `python/iroha_python/tests/test_address_format.py`
    - rewritten to assert `address_format` omission and rejection of removed kwargs.
- JavaScript SDK (`iroha_js`) no longer accepts/emits `addressFormat`/`address_format` for iterable and explorer NFT helpers:
  - `javascript/iroha_js/src/toriiClient.js`
    - removed iterable/explorer option-key support for `addressFormat` and `address_format`.
    - removed address-format query/envelope emission in list/query builders.
    - removed `_normalizeAddressFormatOption`.
  - `javascript/iroha_js/index.d.ts`
    - removed `ToriiAddressFormat`.
    - removed `addressFormat` from `IterableListOptions` and `ExplorerNftListOptions`.
  - updated tests/docs/examples:
    - `javascript/iroha_js/test/toriiClient.test.js`
    - `javascript/iroha_js/test/toriiIterators.parity.test.js`
    - `javascript/iroha_js/test/integrationTorii.test.js`
    - `javascript/iroha_js/README.md`
    - `javascript/iroha_js/recipes/nft_account_iteration.mjs`
- Validation:
  - `python3 -m py_compile python/iroha_python/src/iroha_python/client.py python/iroha_python/src/iroha_python/query.py python/iroha_python/tests/test_address_format.py` (pass)
  - `cd javascript/iroha_js && node --test test/toriiClient.test.js test/toriiIterators.parity.test.js` (pass; 621 passed, 0 failed, 7 skipped)
  - `cd javascript/iroha_js && npm run build:dist` (pass)
  - `python3 -m pytest -q python/iroha_python/tests/test_address_format.py` not run (`No module named pytest` in this environment).

## 2026-03-10 I105-Only Swift Request Surface Hard-Cut (Explorer + Offline + UAID)
- Swift Torii request builders no longer expose `addressFormat`/`address_format` knobs where only I105 is supported:
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift`
    - removed request-side `addressFormat` from:
      - `ToriiExplorerInstructionsParams`
      - `ToriiExplorerTransactionsParams`
      - `ToriiQueryEnvelope`
      - `ToriiOfflineListParams`
      - `ToriiOfflineRevocationListParams`
      - `ToriiOfflineBundleProofStatusParams`
      - `ToriiOfflineReceiptListParams`
      - `ToriiUaidBindingsQuery`
      - `ToriiUaidManifestQuery`
    - removed explorer/offline/UAID `address_format` query emission from async + completion API paths.
    - removed now-dead `explorerAddressFormatQueryValue` / `normalizeAddressFormatQueryValue` helpers.
    - removed `ToriiExplorerAccountQr.preferredFormat` convenience mapper.
  - `IrohaSwift/Sources/IrohaSwift/AccountAddress.swift`
    - removed now-redundant `AccountAddressFormat` enum (single-format hard-cut).
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient+Combine.swift`
    - removed `addressFormat` passthrough from history/transaction transfer publishers.
  - `IrohaSwift/Sources/IrohaSwift/TxBuilder.swift`
    - removed `addressFormat` passthrough from explorer/history forwarding helpers.
- Swift tests aligned to the hard-cut:
  - `IrohaSwift/Tests/IrohaSwiftTests/ToriiOfflineListParamsTests.swift`
    - removed legacy address-format query assertions and rejection cases tied to removed parameters.
  - `IrohaSwift/Tests/IrohaSwiftTests/ToriiClientTests.swift`
    - removed request query assertions expecting `address_format` on explorer + UAID request paths.
    - removed UAID query alias/invalid-format rejection tests; added `ToriiUaidBindingsQuery` empty-query assertion.
- Validation:
  - `cd IrohaSwift && swift build` (pass)
  - `cd IrohaSwift && swift test --filter ToriiClientTests/testUaidBindingsQueryHasNoItems` (pass)
  - `cd IrohaSwift && swift test --filter ToriiOfflineListParamsTests/testOfflineRevocationQueryItems` (pass)
  - `cd IrohaSwift && swift test --filter ToriiOfflineListParamsTests/testQueryEnvelopeEncodesPaginationAndSort` (pass)
  - `cd IrohaSwift && swift test --filter ToriiClientTests/testGetExplorerTransactionDetailEncodesQueryAndDecodesResponse` (pass)
  - `cd IrohaSwift && swift test --filter ToriiClientTests/testGetUaidBindingsReturnsDataspaces` (pass)
  - `cd IrohaSwift && swift test --filter ToriiOfflineListParamsTests/testOfflineBundleProofStatusParams` (pass)
  - `cd IrohaSwift && swift test` (full-suite run remains unstable in this environment; exits with signal 11 after unrelated test progress)

## 2026-03-10 Compile Warning/Error Sweep (`iroha_data_model` + `iroha_core` example)
- Resolved the reported build blockers and warning set tied to `iroha_data_model` test/lib targets and `iroha_core` parity fixture example compilation.
- Applied machine-suggested warning fixes with:
  - `cargo fix -p iroha_data_model --lib --tests --allow-dirty --allow-staged`
- Completed manual dead-code cleanup that `cargo fix` could not apply:
  - removed unused helper `default_domain_id()` in `crates/iroha_data_model/src/account/address.rs` tests module.
  - removed unused helper `domain()` and now-unused imports in `crates/iroha_data_model/tests/account_address_vectors.rs`.
- Validation:
  - `cargo check -p iroha_data_model --tests` (pass, no warnings emitted in the final run)
  - `CARGO_TARGET_DIR=target/codex_warnfix_check cargo check -p iroha_core --example generate_parity_fixtures` (pass)

## 2026-03-10 I105-Only Client Surface Follow-up (JS + Python)
- JavaScript Torii client hard-cut for explorer/UAID helpers:
  - `javascript/iroha_js/src/toriiClient.js`
    - removed `addressFormat` request option from:
      - `getExplorerAccountQr`
      - `getUaidBindings`
      - `getUaidManifests`
    - tightened option validation:
      - `getUaidBindings` now accepts only `signal`
      - `getUaidManifests` now accepts only `dataspaceId` + `signal`
      - `getExplorerAccountQr` now accepts only `signal`
  - `javascript/iroha_js/index.d.ts`
    - removed `addressFormat` from `UaidBindingsQueryOptions`, `UaidManifestQueryOptions`, and `getExplorerAccountQr` options signature.
  - `javascript/iroha_js/test/toriiClient.test.js`
    - updated endpoint tests for no `address_format` query emission and i105-only payload expectations.
    - added regression for rejecting legacy explorer QR payload `address_format` values.
  - rebuilt dist via `npm run build:dist`.
- Python SDK hard-cut for the same API surface:
  - `python/iroha_python/src/iroha_python/client.py`
    - removed `address_format` method parameters from:
      - `get_explorer_account_qr` (+ typed wrapper)
      - `get_uaid_bindings` (+ typed wrapper)
      - `list_space_directory_manifests` (+ typed wrapper)
    - removed corresponding `address_format` query parameter emission.
    - removed constant `address_format` field from `ExplorerAccountQrSnapshot` (payload is still validated as i105 when the field is present).
  - `python/iroha_python/README.md`
    - updated UAID examples to match the new no-`address_format` signatures.
- Standalone Python Torii client follow-up:
  - `python/iroha_torii_client/client.py`
    - removed constant `address_format` field from `ExplorerAccountQr` typed snapshot.
    - kept strict i105 validation for incoming payload `address_format` when present.
  - `python/iroha_torii_client/tests/test_client.py`
    - updated explorer QR typed assertions to match the new snapshot shape.
- Python SDK address-format tests aligned to i105-only:
  - `python/iroha_python/tests/test_address_format.py`
    - updated envelope/query/list expectations from `compressed`/`i105` to `i105`.
    - kept negative alias coverage (`IH-b32`, `sora`) as rejection checks.
- Validation:
  - `cd javascript/iroha_js && IROHA_JS_DISABLE_NATIVE=1 node --test --test-name-pattern='getUaidBindings enforces UAID formats and normalizes entries|getUaidManifests validates lifecycle metadata and filters by dataspace|getExplorerAccountQr normalizes payloads|getExplorerAccountQr rejects unsupported option fields|getExplorerAccountQr rejects legacy payload address format' test/toriiClient.test.js` (pass)
  - `cd javascript/iroha_js && IROHA_JS_DISABLE_NATIVE=1 node --test --test-name-pattern='getUaid|getExplorerAccountQr|DA and UAID helpers reject non-object options' test/toriiClient.test.js` (pass)
  - `cd javascript/iroha_js && npm run build:dist` (pass)
  - `python3 -m py_compile python/iroha_python/src/iroha_python/client.py python/iroha_python/tests/test_address_format.py python/iroha_torii_client/client.py python/iroha_torii_client/tests/test_client.py` (pass)

## 2026-03-10 I105-Only Cleanup Continuation (QR payload + test surface)
- Explorer account QR payload hard-cut now omits redundant `address_format` field:
  - `crates/iroha_torii/src/explorer.rs`
    - removed `ExplorerAccountQrDto.address_format`; server now emits only canonical/literal/network/render fields.
  - `crates/iroha/src/client.rs`
    - removed QR response `address_format` validation branch and updated QR snapshot fixture in tests.
  - `integration_tests/tests/address_canonicalisation.rs`
    - removed explorer-QR `address_format` response assertions and renamed the i105-hint coverage test.
  - `crates/iroha_torii/tests/address_parsing.rs`
    - removed QR response `address_format` assertion and aligned telemetry/address-format expectations to `i105`.
- JS SDK QR snapshot shape hard-cut:
  - `javascript/iroha_js/src/toriiClient.js`
    - `normalizeExplorerAccountQrResponse` no longer emits `addressFormat`.
  - `javascript/iroha_js/index.d.ts`
    - removed `addressFormat` from `ToriiExplorerAccountQrSnapshot`.
  - `javascript/iroha_js/test/toriiClient.test.js`
    - updated QR snapshot expectations; legacy `address_format` response field is now ignored.
  - `javascript/iroha_js/test/integrationTorii.test.js`
    - removed QR snapshot `addressFormat` assertions; now checks payload stability across repeated calls.
  - `javascript/iroha_js/test/toriiIterators.parity.test.js`
    - aligned `address_format` assertion to `i105`.
  - rebuilt dist via `npm run build:dist`.
- Python typed clients QR parser follow-up:
  - `python/iroha_torii_client/client.py`
  - `python/iroha_python/src/iroha_python/client.py`
    - removed QR-response `address_format` parsing/validation branch.
  - `python/iroha_torii_client/tests/test_client.py`
    - updated QR fixtures and legacy-field behavior test to reflect ignored field semantics.
- Additional address-format test normalization:
  - `integration_tests/tests/address_canonicalisation.rs`
    - replaced remaining legacy query hints with canonical I105 query rendering.
  - `javascript/iroha_js/test/toriiClient.test.js`
    - replaced stale `compressed`/`i105` expectations in address-format assertions with `i105`.
  - `javascript/iroha_js/README.md`
  - `javascript/iroha_js/recipes/nft_account_iteration.mjs`
    - removed legacy `addressFormat` aliases from examples and updated iterator/QR docs to i105-only guidance.
  - `IrohaSwift/Tests/IrohaSwiftTests/ToriiOfflineListParamsTests.swift`
  - `IrohaSwift/Tests/IrohaSwiftTests/ToriiClientTests.swift`
    - aligned `address_format` query expectations from `compressed` to `i105`.
- Validation:
  - `cargo fmt --all` (pass)
  - `cd javascript/iroha_js && IROHA_JS_DISABLE_NATIVE=1 node --test --test-name-pattern='listAccounts encodes iterable params|listAccounts accepts i105 addressFormat|queryTriggers normalizes alias fields and address format|getExplorerAccountQr normalizes payloads|getExplorerAccountQr ignores payload address_format field' test/toriiClient.test.js` (pass)
  - `cd javascript/iroha_js && IROHA_JS_DISABLE_NATIVE=1 node --test --test-name-pattern='iterateAccountAssets paginates with addressFormat and maxItems' test/toriiIterators.parity.test.js` (pass)
  - `cd javascript/iroha_js && npm run build:dist` (pass)
  - `python3 -m py_compile python/iroha_torii_client/client.py python/iroha_torii_client/tests/test_client.py python/iroha_python/src/iroha_python/client.py` (pass)
  - `cargo test -p iroha --lib get_explorer_account_qr_parses_payload_and_omits_address_format_query -- --nocapture` (pass)
  - `cargo test -p iroha_torii address_format::tests::from_param_defaults_and_accepts_i105_only -- --nocapture` (pass)
  - `cargo test -p iroha_torii --test address_parsing --no-run` (pass)
  - `cd IrohaSwift && swift test --filter ToriiOfflineListParamsTests/testOfflineBundleProofStatusParamsIncludeCanonicalFormat --filter ToriiOfflineListParamsTests/testOfflineReceiptListParamsQueryItems --filter ToriiClientTests/testGetUaidManifestsAppliesQueryItems` (build + selected tests passed; process exited with signal 11 after completion in this environment)
  - `pytest`-based Python test run was not executed in this environment (`pytest: command not found`).

## 2026-03-10 I105-Only Format Surface (Client/CLI/SDK Follow-up)
- Removed the redundant Rust client address-format enum surface where only I105 is valid:
  - `crates/iroha/src/client.rs`
    - removed `AddressFormat`.
    - `ExplorerAccountQrSnapshot` no longer exposes `address_format`.
    - `ExplorerAccountQrOptions` and `UaidBindingsQuery` are now marker/unit options (no `address_format` field).
    - `UaidManifestQuery` no longer sends `address_format`.
    - removed `address_format` query wiring from:
      - `get_public_lane_validators`
      - `get_public_lane_stake`
      - `get_public_lane_pending_rewards`
- Updated CLI wrappers to match the hard-cut:
  - `crates/iroha_cli/src/nexus.rs`: removed `--address-format` options and conversion glue.
  - `crates/iroha_cli/src/space_directory.rs`: removed `--address-format` options and query serialization.
  - `crates/iroha_cli/src/address.rs`: renamed `DEFAULT_I105_PREFIX` to `DEFAULT_I105_PREFIX`.
- Tightened Torii address-format parsing to `i105` only:
  - `crates/iroha_torii/src/address_format.rs`
  - adjusted related test expectations in `crates/iroha_torii/src/explorer.rs`.
- Removed remaining Rust/integration symbol names containing `I105` in touched test helpers/constants.
- Updated JS/Python Torii client normalization to `i105`-only format values:
  - `javascript/iroha_js/src/toriiClient.js`
  - `javascript/iroha_js/index.d.ts`
  - `python/iroha_python/src/iroha_python/client.py`
  - regenerated JS dist bundle via `npm run build:dist`.

### Validation Matrix (I105-Only Format Surface Follow-up)
- `cargo fmt --all` (pass)
- `cargo test -p iroha --lib get_public_lane_validators_omits_address_format_query -- --nocapture` (pass)
- `cargo test -p iroha --lib get_public_lane_stake_filters_validator -- --nocapture` (pass)
- `cargo test -p iroha --lib get_public_lane_pending_rewards_sets_filters -- --nocapture` (pass)
- `cargo test -p iroha --lib get_explorer_account_qr_parses_payload_and_omits_address_format_query -- --nocapture` (pass)
- `cargo test -p iroha --lib uaid_bindings_query_leaves_query_string_empty -- --nocapture` (pass)
- `cargo test -p iroha --lib get_uaid_manifests_supports_query_and_parsing -- --nocapture` (pass)
- `cargo test -p iroha --lib --tests --no-run` (pass; pre-existing warnings only)
- `cargo test -p iroha_cli --no-run` (pass)
- `cargo test -p iroha_torii --no-run` (pass)
- `cargo test -p iroha_torii from_param_defaults_and_accepts_i105_only -- --nocapture` (pass)
- `IROHA_JS_DISABLE_NATIVE=1 node --test test/address.test.js` in `javascript/iroha_js` (pass)
- `python3 -m py_compile python/iroha_python/src/iroha_python/client.py` (pass)
- Note: `cargo test -p iroha --no-run` fails in this workspace due an unrelated pre-existing tutorial example type mismatch (`crates/iroha/examples/tutorial.rs`), so follow-up checks were run with `--lib --tests --no-run`.

## 2026-03-10 Integration Tests Domainless Gap Closure
- Revalidated the remaining domainless account/domain migration surface in integration tests.
- `cargo check -p integration_tests --tests` now passes (warnings only), including the `Account::new(ScopedAccountId)` and no-`.domain()` test-callsite surface.

## 2026-03-10 I105-Only Account Literal Hard-Cut
- Implemented an I105-only account-address surface in core parsing/encoding paths:
  - `crates/iroha_data_model/src/account/address.rs`
    - removed I105 encode/decode paths and I105-only error variants.
    - Rust-side `AccountAddress::parse_encoded` now models a single I105 format path.
    - I105 sentinel now derives from chain discriminant:
      - `753 -> sora`
      - `369 -> test`
      - `0 -> dev`
      - fallback `n<discriminant>` for other networks.
    - added explicit `to_i105_for_discriminant(...)` / `from_i105_for_discriminant(...)`.
  - `crates/iroha_data_model/src/account.rs`
    - canonical account literal surface switched to `canonical_i105()`.
    - strict parser now validates I105 literals against configured chain discriminant.
  - Updated vector/compliance generators to I105 naming and sentinel/discriminant behavior:
    - `crates/iroha_data_model/src/account/address/vectors.rs`
    - `crates/iroha_data_model/src/account/address/compliance_vectors.rs`
  - Updated downstream runtime/API layers to compile against I105-only enums/methods:
    - `crates/iroha/src/account_address.rs`
    - `crates/iroha_torii/src/address_format.rs`
    - `crates/iroha_torii/src/explorer.rs`
    - `crates/iroha_torii/src/iso20022_bridge.rs`
    - `crates/iroha_cli/src/address.rs`
    - `crates/iroha_js_host/src/lib.rs`
    - `crates/connect_norito_bridge/src/lib.rs`

### Validation Matrix (I105-Only Hard-Cut)
- `cargo check -p iroha_data_model` (pass)
- `cargo check -p iroha -p iroha_torii -p iroha_cli -p connect_norito_bridge -p iroha_js_host -p iroha_config` (pass)
- `rustfmt --edition 2024 crates/iroha_data_model/src/account/address.rs crates/iroha_data_model/src/account.rs crates/iroha_data_model/src/account/address/vectors.rs crates/iroha_data_model/src/account/address/compliance_vectors.rs crates/connect_norito_bridge/src/lib.rs crates/iroha/src/account_address.rs crates/iroha_torii/src/address_format.rs crates/iroha_torii/src/explorer.rs crates/iroha_torii/src/iso20022_bridge.rs crates/iroha_js_host/src/lib.rs crates/iroha_cli/src/address.rs` (pass)
- `cargo fmt --all` (blocked by unrelated pre-existing syntax errors in other files/crates outside this change set).

## 2026-03-10 I105 Surface Sweep (Follow-up)
- Continued the hard-cut migration by removing remaining I105-era symbols/usages from active Rust/Python/Shell code paths touched by account addressing:
  - Replaced stale enum variant references (`AccountAddressFormat::I105`/`Compressed`, `AddressFormatPreference::Compressed`) in test-compiled Torii/CLI/DataModel paths.
  - Migrated legacy helper/test calls (`from_compressed_sora`, `to_compressed_sora`, `to_i105`) to I105 equivalents.
  - Updated fixture/export tooling and fixture labels (`fixtures/account/address_vectors.json`) from `i105` to `i105`.
  - Normalized residual operator/developer-facing strings from I105 wording to I105 wording in touched Rust modules.
- Validation:
  - `cargo check -p iroha_data_model --tests` (pass; warnings only).
  - `cargo check -p iroha -p iroha_torii -p iroha_cli -p connect_norito_bridge --tests` (pass; warnings only).
  - `CARGO_TARGET_DIR=target_tmp_i105_integration cargo check -p integration_tests --tests` (fails due broader ongoing domainless `AccountId` API migration in integration tests, e.g. `Account::new(AccountId)` vs `Account::new(ScopedAccountId)` and `.domain()` removals; not addressed in this sweep).

## 2026-03-10 Parsed Format Metadata Removal (I105-Only API Simplification)
- Removed redundant Rust parser metadata that always resolved to `I105`:
  - deleted `AccountAddressFormat` from Rust data-model exports.
  - `AccountAddress::parse_encoded(...)` now returns only `AccountAddress` (no `(address, format)` tuple).
  - `AccountAddressSource::Encoded(AccountAddressFormat)` simplified to unit variant `AccountAddressSource::Encoded`.
  - `iroha::account_address::ParsedAccountAddress` no longer carries a `format` field.
- Updated dependent crates/tests/callers:
  - `crates/iroha_data_model`
  - `crates/iroha`
  - `crates/iroha_cli`
  - `crates/iroha_torii`
  - `crates/iroha_js_host`
  - `crates/connect_norito_bridge`
  - `scripts/export_norito_fixtures`
  - `integration_tests/tests/address_canonicalisation.rs`
- Validation:
  - `cargo check -p iroha_data_model -p iroha -p iroha_cli -p iroha_torii -p iroha_js_host -p connect_norito_bridge --tests` (pass; warnings only).
  - `cargo test -p integration_tests --test address_canonicalisation --no-run` (pass; warnings only).

## 2026-03-10 Python SDK Parse-Format Cleanup + Docs Token Sweep
- Removed Python-side parse-format enum/export in `iroha_python`:
  - `python/iroha_python/src/iroha_python/address.py`
    - deleted `AccountAddressFormat`.
    - `AccountAddress.parse_encoded(...)` now returns only `AccountAddress`.
  - `python/iroha_python/src/iroha_python/__init__.py`
    - removed `AccountAddressFormat` import/export.
- Updated translated account-structure docs to stop referencing the removed Rust symbol token `AccountAddressFormat` and instead reference `AccountAddress` (`docs/account_structure*.md`).
- Validation:
  - `python3 -m compileall python/iroha_python/src/iroha_python/address.py python/iroha_python/src/iroha_python/__init__.py` (pass).

## 2026-03-10 Asset Usage Hard-Cut (Issuer Baseline + Domain/Dataspace Overlays)
- Implemented first-release hard-cut asset usage semantics without compatibility shims:
  - Added typed metadata policy payloads in `iroha_data_model`:
    - `AssetIssuerUsagePolicyV1`
    - `AssetSubjectBindingV1`
    - `DomainAssetUsagePolicyV1`
    - metadata keys:
      - `iroha:asset_issuer_usage_policy_v1`
      - `iroha:domain_asset_usage_policy_v1`
  - Enforced policy intersection in `iroha_core/src/smartcontracts/isi/asset.rs`:
    - issuer baseline binding checks (per subject)
    - domain-owner overlays via domain metadata
    - dataspace-owner overlays via active Space Directory manifests (`CapabilityRequest`/`ManifestVerdict`)
    - wired into mint/burn/transfer paths
  - Removed legacy asset-definition domain-owner transfer fallback:
    - `iroha_core/src/state.rs` detached permission path now checks source-owner or pending asset-definition owner transfer only.
    - `iroha_executor/src/permission.rs` asset-definition ownership now means `owned_by` only.
  - Removed domain-ownership gate from asset-definition registration admission:
    - initial executor registration guard now allows issuer-owned registration directly.
    - default executor registration visitor executes directly (no domain-owner token gate).
  - Removed runtime domain-existence gate for asset definitions/assets:
    - `Register<AssetDefinition>` no longer requires `id.domain()` lookup.
    - `asset_or_insert` no longer requires `definition.domain()` lookup.
  - Added/updated targeted tests for hard-cut behavior:
    - issuer binding enforcement
    - domain overlay denial
    - dataspace manifest denial
    - transfer authorization updates (source-owner and pending owner transfer semantics)

### Validation Matrix (Asset Usage Hard-Cut)
- `cargo fmt --all` (blocked by unrelated pre-existing syntax errors in other crates/files outside this slice).
- `rustfmt --edition 2024 crates/iroha_data_model/src/asset/policy.rs crates/iroha_data_model/src/asset/mod.rs crates/iroha_core/src/smartcontracts/isi/asset.rs crates/iroha_executor/src/permission.rs crates/iroha_core/src/state.rs crates/iroha_core/src/executor.rs crates/iroha_executor/src/default/mod.rs crates/iroha_core/src/smartcontracts/isi/domain.rs crates/iroha_core/src/smartcontracts/isi/account.rs` (pass).
- `CARGO_TARGET_DIR=target_tmp_asset_policy cargo check -p iroha_data_model` (pass).
- `CARGO_TARGET_DIR=target_tmp_asset_policy cargo check -p iroha_executor` (pass).
- `CARGO_TARGET_DIR=target_tmp_asset_policy cargo check -p iroha_core` (pass).
- `CARGO_TARGET_DIR=target_tmp_asset_policy cargo test -p iroha_core --lib transfer_rejects_when_issuer_policy_requires_binding_for_destination -- --nocapture` (pass).
- `CARGO_TARGET_DIR=target_tmp_asset_policy cargo test -p iroha_core --lib transfer_rejects_when_bound_domain_policy_denies_asset -- --nocapture` (pass).
- `CARGO_TARGET_DIR=target_tmp_asset_policy cargo test -p iroha_core --lib transfer_rejects_when_dataspace_manifest_denies_bound_asset -- --nocapture` (pass).
- `CARGO_TARGET_DIR=target_tmp_asset_policy cargo test -p iroha_core --lib transfer_asset_definition_allows_source_owner -- --nocapture` (pass).
- `CARGO_TARGET_DIR=target_tmp_asset_policy cargo test -p iroha_core --lib initial_executor_denies_transfer_asset_definition_by_definition_domain_owner -- --nocapture` (pass).
- `CARGO_TARGET_DIR=target_tmp_asset_policy cargo test -p iroha_core --lib detached_can_transfer_asset_definition_considers_pending_owner_transfer -- --nocapture` (pass).

## 2026-03-10 Asset Usage Hard-Cut Gap Closure
- Closed follow-up gaps in the hard-cut slice:
  - Removed legacy genesis domain-owner/permission gate for `Register<AssetDefinition>`:
    - `InvalidGenesisError::UnauthorizedAssetDefinition` deleted.
    - genesis validation no longer tracks domain ownership or `CanRegisterAssetDefinition{domain}` grants.
  - Aligned transfer precheck semantics in default executor with runtime execution:
    - transfer of asset-definition ownership is now prechecked only by source account ownership.
    - removed precheck path that previously accepted asset-definition ownership but failed at execution.
  - Removed residual domain-scoped registration cache semantics from `StateTransaction` permission cache.
  - Hard-deleted `CanRegisterAssetDefinition` from the executor permission surface (data model + validation + default executor); this release line keeps no compatibility/no-op token.
  - Removed `CanRegisterAssetDefinition` from initial-executor built-in permission-name allowlist.
  - Removed `CanRegisterAssetDefinition` from schema generation exports (`iroha_schema_gen`).
  - Removed domain-association cleanup treatment of `CanRegisterAssetDefinition` in both:
    - `iroha_core/src/smartcontracts/isi/world.rs`
    - `iroha_executor/src/default/mod.rs`
  - Fixed remaining `iroha_executor` lib-test compile failures caused by old domain-scoped account API assumptions in test helpers and assertions.

### Validation Matrix (Asset Usage Hard-Cut Gap Closure)
- `rustfmt --edition 2024 crates/iroha_core/src/block.rs crates/iroha_core/src/state.rs crates/iroha_executor/src/default/mod.rs crates/iroha_executor/src/permission.rs` (pass).
- `CARGO_TARGET_DIR=target_tmp_asset_policy cargo check -p iroha_executor` (pass).
- `CARGO_TARGET_DIR=target_tmp_asset_policy cargo check -p iroha_core` (pass).
- `CARGO_TARGET_DIR=target_tmp_asset_policy cargo check -p iroha_data_model` (pass).
- `CARGO_TARGET_DIR=target_tmp_asset_policy cargo check -p iroha_schema_gen` (pass).
- `CARGO_TARGET_DIR=target_tmp_asset_policy cargo test -p iroha_core --lib genesis_asset_definition_registration_is_not_domain_gated -- --nocapture` (pass).
- `CARGO_TARGET_DIR=target_tmp_asset_policy cargo test -p iroha_core --lib initial_executor_denies_transfer_asset_definition_by_definition_domain_owner -- --nocapture` (pass).
- `CARGO_TARGET_DIR=target_tmp_asset_policy cargo test -p iroha_core --lib transfer_asset_definition_allows_source_owner -- --nocapture` (pass).
- `CARGO_TARGET_DIR=target_tmp_asset_policy cargo test -p iroha_core --lib detached_can_transfer_asset_definition_considers_pending_owner_transfer -- --nocapture` (pass).
- `CARGO_TARGET_DIR=target_tmp_asset_policy cargo test -p iroha_core --lib fee_sponsor_permission_cache_grant_and_revoke -- --nocapture` (pass).
- `CARGO_TARGET_DIR=target_tmp_asset_policy cargo test -p iroha_executor --lib --no-run` (pass).
- `CARGO_TARGET_DIR=target_tmp_asset_policy cargo test -p iroha_executor --lib visit_instruction_dispatches_repo_instruction_box -- --nocapture` (pass).
- `CARGO_TARGET_DIR=target_tmp_asset_policy cargo test -p iroha_executor --lib signatories_from_multiple_domains_are_allowed -- --nocapture` (pass).
- `CARGO_TARGET_DIR=target_tmp_asset_policy cargo test -p iroha_executor --lib denies_mismatched_claimed_delta -- --nocapture` (pass).
- `CARGO_TARGET_DIR=target_tmp_asset_policy cargo test -p iroha_executor --lib fee_sponsor_permission_associations -- --nocapture` (pass).

## 2026-03-10 Domainless Account / Subject-Keyed Hard-Cut Completion
- Closed the remaining hard-cut cleanup around domainless account identity, subject-keyed ownership, and domain-link guard semantics:
  - `crates/iroha_core/src/smartcontracts/isi/domain.rs`
  - `crates/iroha_core/src/smartcontracts/isi/world.rs`
  - `crates/iroha_core/src/executor.rs`
- Removed the last world-state literal-resolution compatibility path for Nexus unregister guards and fee-sponsor metadata:
  - config/account literals are parsed only as canonical domainless `AccountId`
  - legacy scoped/disambiguation behavior is gone
  - extra domain links no longer introduce special-case literal matching semantics
- Reworked the unregister regressions to exercise the hard-cut end state:
  - `Unregister<Domain>` removes only domain links, aliases, and domain-scoped resources; it no longer deletes globally materialized accounts or their foreign/global ownership state
  - domain removal no longer blocks on Nexus/governance/oracle/content/storage references that point at surviving domainless accounts
  - invalid config/account literals still fail closed on account-removal paths, but no longer participate in domain-unlink decisions
- Closed the remaining CLI/parser/docs in-between surfaces around account identity:
  - `iroha_cli` account-bearing inputs now require canonical I105 `AccountId` literals
  - `ledger account register` keeps domain context explicit via `--domain`
  - account/domain reference docs no longer advertise `@<domain>` hints, compressed account-id parsing, or legacy selector decode paths
- Closed the remaining strict `AccountId` input drift in Torii/CLI/integration coverage:
  - `integration_tests/tests/address_canonicalisation.rs` no longer uses scoped-account leftovers (`Account::new(ScopedAccountId)`, no `.domain()` on `AccountId`) and now rejects compressed `AccountId` literals on account path/query filters, explorer authority filters, and Kaigi relay detail paths while preserving explicit canonical I105 response rendering.
  - `iroha_torii` gateway denylist loading and Kaigi SSE relay filtering now reject compressed `AccountId` literals instead of accepting/canonicalizing them.
  - `iroha_cli` governance public-input owner normalization and Sorafs gateway denylist validation now reject compressed `AccountId` literals instead of accepting/canonicalizing them.
- `iroha_core --tests`, `iroha_torii --tests`, `iroha_cli --tests`, and the touched `integration_tests` address canonicalisation slices now pass after the domainless-account migration sweep.

### Validation Matrix (Domainless Account / Subject-Keyed Hard-Cut Completion)
- `CARGO_TARGET_DIR=target/codex-core-hardcut-final cargo test -p iroha_core --lib unregister_domain_ -- --nocapture` (pass, 22 passed)
- `CARGO_TARGET_DIR=target/codex-core-hardcut-final cargo check -p iroha_core --tests --message-format short` (pass)
- `CARGO_TARGET_DIR=target/codex-torii-hardcut cargo test -p iroha_torii --lib account_id_entries_reject_compressed_literals -- --nocapture` (pass)
- `CARGO_TARGET_DIR=target/codex-torii-hardcut cargo test -p iroha_torii --features telemetry --test kaigi_endpoints kaigi_sse_rejects_compressed_relay_filter -- --nocapture` (pass)
- `CARGO_TARGET_DIR=target/codex-cli-hardcut cargo test -p iroha_cli public_inputs_reject_compressed_owner -- --nocapture` (pass)
- `CARGO_TARGET_DIR=target/codex-cli-hardcut cargo test -p iroha_cli gateway_denylist_record_rejects_compressed_literals -- --nocapture` (pass)
- `CARGO_TARGET_DIR=target/codex-integration-hardcut cargo check -p integration_tests --test address_canonicalisation --message-format short` (pass)
- `CARGO_TARGET_DIR=target/codex-integration-hardcut cargo test -p integration_tests --test address_canonicalisation compressed -- --nocapture` (pass, 9 passed)
- `CARGO_TARGET_DIR=target/codex-integration-hardcut cargo test -p integration_tests --test address_canonicalisation address_format_preferences -- --nocapture` (pass, 3 passed)
- `CARGO_TARGET_DIR=target/codex-workspace-build cargo build --workspace --message-format short` (blocked by unrelated pre-existing syntax errors in `crates/fastpq_prover/src/bin/fastpq_row_bench.rs`)

## 2026-03-10 Domainless Account/Domain Gap Closure (Integration + Bootstrap)
- Closed remaining domainless-vs-scoped integration gaps in `integration_tests/tests/domain_links.rs`:
  - corrected `FindAccountIdsByDomainId` expectation to compare `AccountId` values.
  - updated account registration calls to pass `ScopedAccountId` (`Account::new(...)`) while keeping link APIs domainless (`AccountId`).
  - aligned unlink ownership regression with 0..many semantics by linking explicitly before unlink and asserting unlink only removes explicit subject-domain linkage (asset ownership preserved).
- Fixed `irohad` bootstrap/genesis compile path for domainless account IDs:
  - removed stale genesis authority domain check when deriving stored genesis public key.
  - materialized genesis account registration via `ScopedAccountId` (`subject.to_account_id(genesis_domain)`).
- Fixed governance default account literal generation in config defaults:
  - switched default governance account literals from compressed sora form to canonical I105 so strict `AccountId::parse_encoded(...)` parsing succeeds.

### Validation Matrix (Domainless Account/Domain Gap Closure)
- `CARGO_TARGET_DIR=target_tmp_domainless_gap cargo check -p irohad` (pass)
- `CARGO_TARGET_DIR=target_tmp_domainless_gap cargo check -p iroha_config` (pass)
- `CARGO_TARGET_DIR=target_tmp_domainless_gap cargo test -p integration_tests --test domain_links -- --nocapture` (pass, 5 passed)

## 2026-03-10 MCP Gap Closure (Agent-First Follow-up)
- Closed MCP/CLI plan follow-up gaps for agent workflows:
  - Added MCP async job retention controls in config (`torii.mcp.async_job_ttl_secs`, `torii.mcp.async_job_max_entries`) and wired defaults/user/actual parsing.
  - Implemented async-job pruning/retention enforcement in MCP (`tools/call_async`, `tools/jobs/get`) with TTL + max-entry eviction.
  - Added MCP coverage tests for `tools/call_batch`, `tools/call_async`/`tools/jobs/get`, `tools/list` `listChanged`, and async-job pruning behavior.
  - Fixed `offline_app_api` fixture asset definition construction to include `balance_scope_policy`.
  - Fixed malformed `offline_certificates_app_api` operator-injection tests so they compile and assert correctly.
- Documentation:
  - `crates/iroha_torii/docs/mcp_api.md` now documents async job retention behavior.

### Validation Matrix (MCP Gap Closure)
- `cargo test -p iroha_config fixtures -- --nocapture` (passes; filter matches 0 runtime tests but compiles test target).
- `cargo check -p iroha_config` (passes).
- `cargo check -p iroha_torii` (blocked by unrelated pre-existing syntax drift in `crates/iroha_torii/src/lib.rs` test region outside MCP module).

## 2026-03-10 MOCHI Ganache-Style Devnet UX Refactor
- Reframed `mochi-ui-egui` around the job Mochi is actually doing: spinning up and debugging disposable local Iroha devnets, not acting as a generic infrastructure console.
- Added a **Devnet quickstart** surface on the **Network** page for:
  - `Single Peer` and `Four Peer BFT` presets
  - workspace and chain ID inputs
  - `Start devnet`, `Restart devnet with this setup`, `Apply without starting`, and `Stop devnet`
- Added a **Connect your app** surface so developers can copy Torii/API endpoints and bundled development identities directly from the Network page.
- Collapsed the top-level IA into **Network**, **Activity**, **State**, and **Transactions** so startup/debugging work is easier to find.
- Repositioned **Settings** as an advanced path for profile overrides, Nexus/DA knobs, readiness/tooling behaviour, compatibility controls, and export paths instead of the day-to-day setup flow.
- Grouped top control-bar actions under **Devnet**, **Maintenance**, and **Config**.
- Made the **Activity** view auto-attach logs, events, and blocks to a running peer, with reconnect/disconnect controls and clearer running/stopped status.
- Made composer submit results hand off into debugging surfaces automatically:
  - successful submits jump to `Activity -> Events` with the transaction hash prefilled
  - failed submits jump to `Activity -> Logs` for the selected peer
- Reduced dashboard overload by making the Network page scrollable, collapsing deeper telemetry charts, and hiding low-signal path details behind secondary affordances.
- Simplified the transaction composer so common local-dev actions are prominent and advanced/governance actions are secondary.
- Updated Mochi docs to describe the new devnet-first workflow:
  - `docs/source/mochi_architecture_plan.md`
  - `docs/source/mochi/quickstart.md`

## 2026-03-10 Iroha Monitor TUI + Etenraku Refresh
- Reworked `iroha_monitor` toward a monitoring-first terminal layout:
  - the large decorative header no longer dominates medium terminals
  - added a clearer overview panel with online/healthy/degraded/down counts, throughput, queue/gas, latency, refresh, and focus summary
  - peer table now keeps the selected peer visible and uses health-aware colour treatment
  - selected peer details and severity-tagged alerts are rendered in dedicated side panels
  - repeated warnings no longer spam the event log; typed peer notices now emit recoveries/warnings/outages instead of lossy string heuristics
  - added scalable operator controls for large peer sets: sort cycling, issues-only filtering, and inline endpoint/name search
- Kept a smaller festival identity panel so the monitor still feels distinct without burying the telemetry.
- Refined the builtin Etenraku arrangement and synth:
  - softened shō, hichiriki, and ryūteki timbres away from square-wave/retro colour
  - made ryūteki less literal than a straight octave-doubled hichiriki line
  - thinned koto writing, split biwa into its own darker plucked voice, and reduced kakko density
  - added builtin taiko/shōko/kakko percussion voices so the realtime synth no longer drops the percussion layer
  - upgraded demo MIDI export to a format-1 multitrack file with per-instrument tracks, names, pans, and revised GM stand-ins
  - updated theme/docs copy to describe the more gagaku-like builtin audio path
- Refreshed the monitor docs capture pipeline:
  - updated the screenshot helper and smoke expectations for the monitoring-first UI and the new search/sort controls
  - regenerated the baked SVG/ANSI demo assets plus manifest/checksum records under `docs/source/images/iroha_monitor_demo/`
- Files updated:
  - `crates/iroha_monitor/src/main.rs`
  - `crates/iroha_monitor/src/fetch.rs`
  - `crates/iroha_monitor/src/etenraku.rs`
  - `crates/iroha_monitor/src/synth.rs`
  - `crates/iroha_monitor/src/theme.rs`
  - `crates/iroha_monitor/src/ascii.rs`
  - `crates/iroha_monitor/tests/smoke.rs`
  - `crates/iroha_monitor/tests/invalid_credentials.rs`
  - `docs/source/iroha_monitor.md`
  - `scripts/run_iroha_monitor_demo.py`
  - `scripts/iroha_monitor_demo.sh`
  - `docs/source/images/iroha_monitor_demo/*`

### Validation Matrix (Iroha Monitor TUI + Etenraku Refresh)
- `cargo fmt --all` (blocked by unrelated existing syntax errors outside `iroha_monitor`; formatted monitor Rust files directly with `rustfmt --edition 2024 ...`)
- `cargo test -p iroha_monitor -- --nocapture`
- `CARGO_TARGET_DIR=target_tmp_iroha_monitor_validation cargo test -p iroha_monitor -- --nocapture`
- `python3 -m unittest scripts.run_iroha_monitor_demo`
- `python3 scripts/check_iroha_monitor_screenshots.py --dir docs/source/images/iroha_monitor_demo`
- `./scripts/iroha_monitor_demo.sh --monitor-binary ./target/debug/iroha_monitor`
- `python3 scripts/run_iroha_monitor_demo.py --binary ./target/debug/iroha_monitor --output-dir <tmpdir>` (no fallback)
- `CARGO_TARGET_DIR=target_tmp_workspace_build cargo build --workspace` (blocked by unrelated existing syntax error in `crates/iroha_data_model/src/account.rs`)

## 2026-03-10 Domainless Receive/Vector Consistency Closure
- Fixed address-vector strict-decoder drift for canonical invalid literals:
  - `canonical_invalid_hex` now validates through strict `parse_encoded` (`canonical_hex` decoder semantics), returning `UnsupportedAddressFormat`.
  - Added `UnsupportedAddressFormat` handling to compliance vector JSON generation and vector validators in data model and Torii tests.
  - files:
    - `crates/iroha_data_model/src/account/address/vectors.rs`
    - `crates/iroha_data_model/src/account/address/compliance_vectors.rs`
    - `crates/iroha_data_model/tests/account_address_vectors.rs`
    - `crates/iroha_torii/tests/account_address_vectors.rs`
    - `fixtures/account/address_vectors.json`
- Closed remaining in-between test state in receive-admission integration coverage:
  - `implicit_account_receive` now configures policy through the global chain parameter (`iroha:default_account_admission_policy`) instead of domain metadata.
  - Added shared helper to install global policy via `SetParameter` and `StateTransaction::apply`.
  - files:
    - `crates/iroha_core/tests/implicit_account_receive.rs`
- Revalidated multisig/domainless admission, domain-link behavior, asset scope partitioning, and receive/credit heavy paths (offline/repo/settlement/oracle/social).

### Validation Matrix (Domainless Receive/Vector Consistency Closure)
- `cargo fmt --all`
- `cargo build --workspace`
- `cargo run -p xtask --bin xtask -- address-vectors --out fixtures/account/address_vectors.json`
- `cargo test -p iroha_data_model`
- `cargo test -p iroha_data_model --test account_address_vectors`
- `cargo test -p iroha_torii --test account_address_vectors`
- `cargo test -p iroha_torii --lib multisig_guard_tests`
- `cargo test -p iroha_core --lib mint_restricted_asset_uses_current_dataspace_bucket`
- `cargo test -p iroha_core --lib transfer_restricted_asset_rejects_cross_dataspace_scope`
- `cargo test -p iroha_core --lib account_admission -- --nocapture`
- `cargo test -p iroha_core --test implicit_account_receive`
- `cargo test -p iroha_core --test oracle`
- `cargo test -p iroha_core --test settlement_overlay`
- `cargo test -p iroha_core --test social_viral_incentives`
- `cargo test -p iroha_core --lib settlement -- --nocapture`
- `cargo test -p iroha_core --lib repo -- --nocapture`
- `cargo test -p iroha_core --lib offline -- --nocapture`
- `cargo test -p integration_tests --test multisig -- --nocapture`
- `cargo test -p integration_tests --test domain_links -- --nocapture`

## 2026-03-10 Sumeragi NEW_VIEW Flake Stabilization
- Stabilized `iroha_core` full-suite flake caused by shared commit-history/status cross-test interference in two NEW_VIEW tests.
- Hardened tests to run under commit-history test guard with explicit commit-history/checkpoint/precommit-history reset and cleanup:
  - `new_view_tracker_counts_local_with_rotated_indices`
  - `new_view_vote_accepts_prepare_highest_next_height`
- File updated:
  - `crates/iroha_core/src/sumeragi/main_loop/tests.rs`

### Validation Matrix (NEW_VIEW Flake Stabilization)
- `cargo fmt --all` (pass)
- `cargo test -p iroha_core --lib new_view_tracker_counts_local_with_rotated_indices -- --nocapture` (pass)
- `cargo test -p iroha_core --lib new_view_vote_accepts_prepare_highest_next_height -- --nocapture` (pass)
- `cargo test -p iroha_core --lib` (pass; `3697 passed; 0 failed; 5 ignored`)

## 2026-03-09 Domainless Admission/Scope Hardening (No Runtime Fallbacks)
- Removed account-admission runtime fallback to per-domain metadata; execution now reads only the global chain parameter (`iroha:default_account_admission_policy`) and defaults.
  - file: `crates/iroha_core/src/smartcontracts/isi/account_admission.rs`
- Kept unit-test coverage without reintroducing runtime shims by moving metadata-seeded policies into chain parameters inside test harness setup.
  - file: `crates/iroha_core/src/smartcontracts/isi/account_admission.rs` (test module `test_state(...)`)
- Fixed implicit-creation fee routing for domainless/account-subject literals by resolving payer/sink against existing subject-linked accounts before debiting/crediting balances.
  - file: `crates/iroha_core/src/smartcontracts/isi/account_admission.rs`
- Updated prefetch parser regressions to assert canonical encoded-domain behavior (subject-equivalence) instead of legacy scoped-domain equality assumptions.
  - file: `crates/iroha_core/src/block.rs`

### Validation Matrix (Domainless Admission/Scope Hardening)
- `cargo fmt --all`
- `cargo test -p iroha_data_model --lib asset_id_with_explicit_scope_roundtrips -- --nocapture`
- `cargo test -p iroha_core --lib account_admission -- --nocapture`
- `cargo test -p iroha_core --lib mint_restricted_asset_uses_current_dataspace_bucket -- --nocapture`
- `cargo test -p iroha_core --lib transfer_restricted_asset_rejects_cross_dataspace_scope -- --nocapture`
- `cargo test -p iroha_core --lib multisig_spec_preserves_signatory_domains -- --nocapture`
- `cargo test -p iroha_core --lib multisig_spec_allows_same_subject_across_domains -- --nocapture`
- `cargo test -p integration_tests domain_links -- --nocapture`
- `cargo test -p integration_tests multisig -- --nocapture`
- `cargo test -p iroha_core --lib block::prefetch_tests::parse_account_key_variants -- --exact --nocapture`
- `cargo test -p iroha_core --lib block::prefetch_tests::parse_lane_settlement_buffer_config_resolves_account -- --exact --nocapture`
- `cargo test -p iroha_core --lib -- --nocapture` (revealed many pre-existing/in-progress branch failures outside this slice; latest run: `3563 passed, 134 failed`)

## 2026-03-09 Agent-First MCP/API + CLI Machine-Mode Hardening
- Hardened Torii MCP contracts for bot integrations:
  - `tools/list` now returns `toolsetVersion` and `listChanged` (based on caller-provided toolset version drift).
  - `initialize`/capabilities now include MCP toolset version metadata.
  - OpenAPI-derived MCP tool names are now stable route-derived IDs (`torii.<method>_<path>`), no longer sourced from mutable `operationId`.
  - MCP tool descriptors now publish `outputSchema`; input schemas now reuse OpenAPI parameter/body schemas (including `$ref` resolution).
  - Added MCP methods:
    - `tools/call_batch`
    - `tools/call_async`
    - `tools/jobs/get`
  - Added optional response projection via `arguments.project` to reduce large `structuredContent.body` payloads.
  - Standardized JSON-RPC/MCP error payloads with stable `error_code` fields.
- Added MCP policy controls in config (`torii.mcp`) with first-release defaults:
  - `profile` (`read_only`/`writer`/`operator`, default `read_only`)
  - `allow_tool_prefixes`
  - `deny_tool_prefixes`
  - files:
    - `crates/iroha_config/src/parameters/defaults.rs`
    - `crates/iroha_config/src/parameters/user.rs`
    - `crates/iroha_config/src/parameters/actual.rs`
    - `crates/iroha_config/tests/fixtures.rs`
- Hardened CLI machine automation behavior:
  - Added `--machine` flag to disable startup chatter and require explicit readable config (no fallback config when missing).
  - CLI parse/argument failures now render through CLI JSON error envelope in JSON mode (`kind=input`, `exit_code=4`) instead of direct clap process exit.
  - Removed forced text override for `tools address`; all subcommands now honor `--output-format`.
  - files:
    - `crates/iroha_cli/src/main_shared.rs`
    - `crates/iroha_cli/README.md`
- Updated MCP docs:
  - `crates/iroha_torii/docs/mcp_api.md`

### Validation Matrix (Agent-First MCP/API + CLI Machine-Mode Hardening)
- `cargo fmt --all`
- `CARGO_TARGET_DIR=target_tmp_codex_mcp_plan cargo check -p iroha_config -p iroha_torii -p iroha_cli`
- `CARGO_TARGET_DIR=target_tmp_codex_mcp_plan cargo test -p iroha_torii --lib tool_registry_skips_ws_and_sse_routes -- --nocapture`
- `CARGO_TARGET_DIR=target_tmp_codex_mcp_plan cargo test -p iroha_torii --lib capabilities_payload_includes_toolset_version -- --nocapture`
- `CARGO_TARGET_DIR=target_tmp_codex_mcp_plan cargo test -p iroha_torii --lib jsonrpc_error_response_adds_stable_error_code -- --nocapture`
- `CARGO_TARGET_DIR=target_tmp_codex_mcp_plan cargo test -p iroha_torii --lib read_only_policy_blocks_mutating_tools -- --nocapture`
- `CARGO_TARGET_DIR=target_tmp_codex_mcp_plan cargo test -p iroha_torii --lib apply_body_projection_keeps_requested_fields -- --nocapture`
- `CARGO_TARGET_DIR=target_tmp_codex_mcp_plan cargo test -p iroha_cli effective_output_format_for_address_tools_uses_cli_flag -- --nocapture`
- `CARGO_TARGET_DIR=target_tmp_codex_mcp_plan cargo test -p iroha_cli render_cli_error_marks_cli_argument_failures_as_input -- --nocapture`
- `CARGO_TARGET_DIR=target_tmp_codex_mcp_plan cargo test -p iroha_config fixtures -- --nocapture` (filter matched 0 tests; crate/tests compiled successfully)
- `CARGO_TARGET_DIR=target_tmp_codex_mcp_plan cargo test -p iroha_torii mcp::tests::tool_registry_skips_ws_and_sse_routes -- --nocapture` (blocked by unrelated existing integration-test compile errors: missing `balance_scope_policy` in `offline_certificates_app_api.rs` and `offline_app_api.rs`; used `--lib` path above for MCP validation)

## 2026-03-09 Domain Link APIs + Receive Path Coverage
- Added explicit account-domain link instructions and dispatch wiring:
  - `LinkAccountDomain`
  - `UnlinkAccountDomain`
  - files:
    - `crates/iroha_data_model/src/isi/domain_link.rs`
    - `crates/iroha_data_model/src/isi/mod.rs`
    - `crates/iroha_data_model/src/isi/registry.rs`
    - `crates/iroha_core/src/smartcontracts/isi/mod.rs`
    - `crates/iroha_core/src/smartcontracts/isi/domain.rs`
- Added domain-link data events:
  - `DomainEvent::AccountLinked(AccountDomainLinkChanged)`
  - `DomainEvent::AccountUnlinked(AccountDomainLinkChanged)`
  - file: `crates/iroha_data_model/src/events/data/events.rs`
- Added singular queries for subject-domain membership inspection:
  - `FindDomainsByAccountId -> Vec<DomainId>`
  - `FindAccountIdsByDomainId -> Vec<AccountId>`
  - files:
    - `crates/iroha_data_model/src/query/mod.rs`
    - `crates/iroha_data_model/src/query/json/envelope.rs`
    - `crates/iroha_data_model/src/visit/visit_query.rs`
    - `crates/iroha_core/src/smartcontracts/isi/account.rs`
    - `crates/iroha_core/src/smartcontracts/isi/domain.rs`
    - `crates/iroha_core/src/smartcontracts/isi/query.rs`
- Added regressions:
  - core/unit:
    - `find_domains_by_account_id_returns_linked_domains_for_subject`
    - `link_and_unlink_account_domain_updates_subject_query_indexes`
    - `unlink_account_domain_rejects_unauthorized_authority`
    - `find_account_ids_by_domain_id_roundtrip` (query JSON envelope)
  - integration:
    - `domain_links_roundtrip_without_account_registration`
    - `receive_paths_materialize_unregistered_accounts_for_assets_and_nfts`
    - `domain_links_allow_subject_authority_for_link_and_unlink`
    - `domain_links_reject_unrelated_authority`
    - `unlink_domain_link_preserves_materialized_asset_ownership`
    - file: `integration_tests/tests/domain_links.rs`

### Validation Matrix (Domain Link APIs + Receive Path Coverage)
- `cargo fmt --all`
- `cargo test -p iroha_data_model find_account_ids_by_domain_id_roundtrip -- --nocapture`
- `cargo test -p iroha_core find_domains_by_account_id_returns_linked_domains_for_subject -- --nocapture`
- `cargo test -p iroha_core --lib account_domain -- --nocapture`
- `cargo test -p integration_tests --test domain_links -- --nocapture`
- `CARGO_TARGET_DIR=target_tmp_validation_cutover cargo test -p integration_tests --test domain_links domain_links_allow_subject_authority_for_link_and_unlink -- --nocapture`
- `CARGO_TARGET_DIR=target_tmp_validation_cutover cargo test -p integration_tests --test domain_links domain_links_reject_unrelated_authority -- --nocapture`
- `CARGO_TARGET_DIR=target_tmp_validation_cutover cargo test -p integration_tests --test domain_links unlink_domain_link_preserves_materialized_asset_ownership -- --nocapture`

## 2026-03-09 Commit Validation Queue-Saturation Hot-Path Cutover
- Implemented an early inline pre-vote validation cutover when validation workers are saturated:
  - `crates/iroha_core/src/sumeragi/main_loop/validation.rs`
  - when worker queues are full, pending blocks now switch from deferred validation to inline validation once `pending_age` reaches a deterministic cutover (`fast_timeout / 2`, floored at 1ms), instead of waiting until full fast-timeout expiry.
- Preserved the defer-first behavior for fresh pending blocks under queue pressure:
  - queue-full requests still defer before the cutover to avoid over-eager inline work.
- Added queue-saturation regressions in:
  - `crates/iroha_core/src/sumeragi/main_loop/tests.rs`
  - `commit_pipeline_inlines_validation_at_queue_full_cutover`
  - `commit_pipeline_keeps_deferred_validation_before_queue_full_cutover`
- Fixed query envelope account-id decoding regression exposed while validating this slice:
  - `crates/iroha_data_model/src/query/json/envelope.rs`
  - `FindDomainsByAccountId` now parses account literals through `AccountId::parse_encoded(...)` instead of relying on `FromStr`.
- Added `iroha_data_model` regression coverage for the account-id JSON path:
  - `find_domains_by_account_id_accepts_canonical_i105_literal`
- Fixed a compile-time move bug in an existing query visitor regression test:
  - `crates/iroha_data_model/src/visit/visit_query.rs`
  - `singular_query_fallback_never_triggers_for_known_variants` now clones `account_id` before `AssetId::new(...)`.

### Validation Matrix (Commit Validation Queue-Saturation Hot-Path Cutover)
- `cargo fmt --all`
- `CARGO_TARGET_DIR=target_tmp_validation_cutover cargo test -p iroha_core --lib commit_pipeline_inlines_validation_ -- --nocapture`
- `CARGO_TARGET_DIR=target_tmp_validation_cutover cargo test -p iroha_core --lib commit_pipeline_keeps_deferred_validation_ -- --nocapture`
- `CARGO_TARGET_DIR=target_tmp_validation_cutover cargo test -p iroha_data_model --lib find_domains_by_account_id_accepts_canonical_i105_literal -- --nocapture`
- `CARGO_TARGET_DIR=target_tmp_validation_cutover cargo test -p iroha_data_model --lib singular_query_fallback_never_triggers_for_known_variants -- --nocapture`

## 2026-03-09 Configurable Queue-Full Validation Inline Cutover
- Made queue-saturation inline-validation cutover configurable through consensus worker settings:
  - added `sumeragi.advanced.worker.validation_queue_full_inline_cutover_divisor` in:
    - `crates/iroha_config/src/parameters/defaults.rs`
    - `crates/iroha_config/src/parameters/user.rs`
    - `crates/iroha_config/src/parameters/actual.rs`
  - parser now rejects zero divisors (`ParseError::InvalidSumeragiConfig`).
- Wired runtime cutover computation to the new config field:
  - `crates/iroha_core/src/sumeragi/main_loop/validation.rs`
  - queue-full inline cutover now uses `fast_timeout / validation_queue_full_inline_cutover_divisor` (with deterministic 1ms floor).
- Added regression coverage for runtime use of the configured divisor:
  - `crates/iroha_core/src/sumeragi/main_loop/tests.rs`
  - `commit_pipeline_uses_configured_queue_full_inline_cutover_divisor`
  - existing queue-full tests now derive expected cutover from config instead of hardcoding.
- Threaded the new worker field through helper/default config literals used in tests/harnesses:
  - `crates/iroha_core/src/kiso.rs`
  - `crates/iroha_core/src/sumeragi/penalties.rs`
  - `crates/iroha_torii/src/test_utils.rs`
  - `crates/iroha_torii/tests/connect_gating.rs`
  - `crates/iroha_config/tests/fixtures.rs`
- Unblocked unrelated `iroha_core` test compilation surfaced during validation by completing `NewAssetDefinition` initializers with explicit balance policy:
  - `crates/iroha_core/src/smartcontracts/isi/domain.rs`
  - `crates/iroha_core/src/smartcontracts/isi/world.rs`

### Validation Matrix (Configurable Queue-Full Validation Inline Cutover)
- `cargo fmt --all`
- `CARGO_TARGET_DIR=target_tmp_validation_cutover cargo test -p iroha_config --lib sumeragi_rejects_zero_worker_validation_queue_full_inline_cutover_divisor -- --nocapture`
- `CARGO_TARGET_DIR=target_tmp_validation_cutover cargo test -p iroha_core --lib commit_pipeline_uses_configured_queue_full_inline_cutover_divisor -- --nocapture`
- `CARGO_TARGET_DIR=target_tmp_validation_cutover cargo test -p iroha_core --lib commit_pipeline_inlines_validation_at_queue_full_cutover -- --nocapture`
- `CARGO_TARGET_DIR=target_tmp_validation_cutover cargo test -p iroha_core --lib commit_pipeline_keeps_deferred_validation_before_queue_full_cutover -- --nocapture`
- `CARGO_TARGET_DIR=target_tmp_validation_cutover cargo test -p iroha_torii --lib --no-run`
- `CARGO_TARGET_DIR=target_tmp_validation_cutover cargo test -p iroha_torii --test connect_gating --no-run`
- `CARGO_TARGET_DIR=target_tmp_validation_cutover cargo test -p iroha_config --test fixtures --no-run`

## 2026-03-09 MCP Compile Fixups for Integration Build Paths
- Resolved pre-existing `iroha_torii` compile errors surfaced while running `integration_tests`:
  - `crates/iroha_torii/src/mcp.rs`
  - replaced direct method-call expressions inside `norito::json!` payloads with bound values to satisfy macro parsing.
  - fixed tools listing descriptor mapping over `&ToolSpec` slices (`.map(|tool| tool.descriptor())`).
  - fixed OpenAPI reference traversal indexing to use `&str` keys (`current.get(key.as_str())`).

### Validation Matrix (MCP Compile Fixups for Integration Build Paths)
- `cargo fmt --all`
- `CARGO_TARGET_DIR=target_tmp_validation_cutover cargo test -p integration_tests --test domain_links domain_links_allow_subject_authority_for_link_and_unlink -- --nocapture`
- `CARGO_TARGET_DIR=target_tmp_validation_cutover cargo test -p integration_tests --test domain_links domain_links_reject_unrelated_authority -- --nocapture`
- `CARGO_TARGET_DIR=target_tmp_validation_cutover cargo test -p integration_tests --test domain_links unlink_domain_link_preserves_materialized_asset_ownership -- --nocapture`

## 2026-03-09 Multisig Signatory Auto-Materialization
- Updated built-in multisig execution to stop requiring pre-registered signatory accounts for multisig composition:
  - removed strict signatory-existence gating from registration validation in:
    - `crates/iroha_core/src/smartcontracts/isi/multisig.rs`
  - `MultisigRegister` and `AddSignatory` now materialize missing signatory accounts automatically (tagged with `iroha:created_via = "multisig"`), using standard `Register::account` execution under the destination domain owner.
- Made multisig graph validation tolerant of unresolved signatories during registration-time checks:
  - cycle roots are evaluated from declared spec signatories directly.
  - `is_multisig(...)` now treats missing accounts as non-multisig leaves instead of hard errors.
- Added defensive registration-domain anchoring for JSON/custom-instruction account decoding edge cases:
  - if requested multisig account domain does not exist and equals the implicit default label, registration falls back to the authority account domain.
- Added targeted multisig regressions in:
  - `crates/iroha_core/src/smartcontracts/isi/multisig.rs`
  - `register_materializes_missing_signatory_accounts`
  - `add_signatory_materializes_missing_account`
- Added integration coverage + docs alignment:
  - `integration_tests/tests/multisig.rs` now includes:
    - `multisig_register_materializes_missing_signatory_account`
    - `multisig_register_rejected_does_not_materialize_missing_signatory_account`
    - `multisig_add_signatory_materializes_missing_account`
    - `multisig_add_signatory_rejected_does_not_materialize_missing_account`
  - `crates/iroha_cli/docs/multisig.md` no longer claims all signatories must be pre-registered
  - `docs/source/references/multisig_policy_schema.md` documents automatic signatory
    materialization and `iroha:created_via = "multisig"` tagging on successful register/add-signatory flows
- Hardened instruction ordering to avoid side effects on unauthorized flows:
  - `AddSignatory` and `MultisigRegister` now perform authority-gated operations before signatory auto-materialization.
- Added failure-path regressions to ensure rejected instructions do not materialize accounts:
  - `register_invalid_spec_does_not_materialize_missing_signatory`
  - `register_existing_account_does_not_materialize_missing_signatory`

### Validation Matrix (Multisig Signatory Auto-Materialization)
- `cargo fmt --all`
- `cargo test -p iroha_core initial_executor_runs_multisig_flow -- --nocapture`
- `cargo test -p iroha_core register_materializes_missing_signatory_accounts -- --nocapture`
- `cargo test -p iroha_core add_signatory_materializes_missing_account -- --nocapture`
- `cargo test -p iroha_core register_invalid_spec_does_not_materialize_missing_signatory -- --nocapture`
- `cargo test -p iroha_core register_existing_account_does_not_materialize_missing_signatory -- --nocapture`
- `cargo test -p integration_tests multisig_register_materializes_missing_signatory_account -- --nocapture`
- `cargo test -p integration_tests multisig_register_rejected_does_not_materialize_missing_signatory_account -- --nocapture`
- `cargo test -p integration_tests multisig_add_signatory_materializes_missing_account -- --nocapture`
- `cargo test -p integration_tests multisig_add_signatory_rejected_does_not_materialize_missing_account -- --nocapture`

## 2026-03-09 Build-Claim JNI Encoder Parity for Android Offline Flow
- Added missing build-claim JNI bridge export in:
  - `crates/connect_norito_bridge/src/lib.rs`
  - `Java_org_hyperledger_iroha_android_offline_OfflineBuildClaimPayloadEncoder_nativeEncode`
- Added Rust-side build-claim payload encoder helper and parity regression:
  - `encode_offline_build_claim_payload(...)`
  - `encode_offline_build_claim_payload_matches_native`
- Added Android wrapper API and harness regression:
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/OfflineBuildClaimPayloadEncoder.java`
  - `java/iroha_android/src/test/java/org/hyperledger/iroha/android/offline/OfflineBuildClaimPayloadEncoderTest.java`
  - registered test main in `java/iroha_android/src/test/java/org/hyperledger/iroha/android/GradleHarnessTests.java`
- Applied follow-up lint-only cleanup discovered during workspace `clippy -D warnings` execution:
  - `crates/iroha_genesis/src/lib.rs`
  - `crates/iroha/examples/tutorial.rs`
  - `crates/iroha_js_host/src/lib.rs`
  - `crates/iroha_data_model/tests/id_of_constructors.rs`
  - `crates/iroha_data_model/tests/offline_fixtures.rs`
  - `crates/iroha_data_model/tests/query_json_envelope.rs`
  - `crates/iroha_data_model/src/offline/poseidon.rs`
  - `crates/iroha_test_network/src/config.rs`
  - `xtask/src/norito_rpc.rs`
- Java encoder behavior:
  - normalizes platform aliases (`ios`/`apple` -> `Apple`, `android` -> `Android`)
  - validates hash inputs through `OfflineHashLiteral.parseHex(...)`
  - rejects negative numeric timestamps/build number
  - coerces null/blank `lineageScope` to empty string before JNI call
- Native-required Android harness fixture alignment:
  - switched offline receipt/spend JNI tests to canonical encoded `AssetId` fixtures (`norito:<hex>`) and valid I105 account literals:
    - `java/iroha_android/src/test/java/org/hyperledger/iroha/android/offline/OfflineReceiptChallengeTest.java`
    - `java/iroha_android/src/test/java/org/hyperledger/iroha/android/offline/OfflineSpendReceiptPayloadEncoderTest.java`
  - ensured build-claim nonce fixture satisfies hash LSB policy:
    - `java/iroha_android/src/test/java/org/hyperledger/iroha/android/offline/OfflineBuildClaimPayloadEncoderTest.java`

### Validation Matrix (Build-Claim JNI Encoder Parity)
- `cargo fmt --all`
- `cargo test -p connect_norito_bridge encode_offline_build_claim_payload_matches_native -- --nocapture`
- `cargo test -p connect_norito_bridge encode_offline_spend_receipt_payload_matches_native -- --nocapture`
- `cd java/iroha_android && JAVA_HOME=$(/usr/libexec/java_home -v 21) ANDROID_HOME=~/Library/Android/sdk ANDROID_SDK_ROOT=~/Library/Android/sdk ./gradlew test -Dandroid.test.mains=org.hyperledger.iroha.android.offline.OfflineBuildClaimPayloadEncoderTest`
- `cargo test -p connect_norito_bridge -- --nocapture`
- `cd java/iroha_android && JAVA_HOME=$(/usr/libexec/java_home -v 21) ANDROID_HOME=~/Library/Android/sdk ANDROID_SDK_ROOT=~/Library/Android/sdk ./gradlew test -Dandroid.test.mains=org.hyperledger.iroha.android.offline.OfflineBuildClaimPayloadEncoderTest,org.hyperledger.iroha.android.offline.OfflineSpendReceiptPayloadEncoderTest`
- `cd java/iroha_android && JAVA_HOME=$(/usr/libexec/java_home -v 21) ANDROID_HOME=~/Library/Android/sdk ANDROID_SDK_ROOT=~/Library/Android/sdk ./gradlew test --rerun-tasks -Dandroid.test.mains=org.hyperledger.iroha.android.offline.OfflineBuildClaimPayloadEncoderTest,org.hyperledger.iroha.android.offline.OfflineSpendReceiptPayloadEncoderTest`
- `cargo build --workspace`
- `cargo check -p iroha --example tutorial`
- `cargo test -p iroha_data_model --test id_of_constructors --no-run`
- `cargo test -p iroha_data_model --test offline_fixtures --no-run`
- `cargo test -p iroha_data_model --test query_json_envelope --no-run`
- `cargo test -p iroha_js_host --lib --no-run`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cd java/iroha_android && ANDROID_HARNESS_MAINS='org.hyperledger.iroha.android.offline.OfflineBuildClaimPayloadEncoderTest,org.hyperledger.iroha.android.offline.OfflineReceiptChallengeTest,org.hyperledger.iroha.android.offline.OfflineSpendReceiptPayloadEncoderTest,org.hyperledger.iroha.android.offline.OfflineWalletTest,org.hyperledger.iroha.android.client.OfflineToriiClientTests' IROHA_NATIVE_REQUIRED=1 IROHA_NATIVE_LIBRARY_PATH=/Users/takemiyamakoto/dev/iroha/target/debug JAVA_HOME=$(/usr/libexec/java_home -v 21) ANDROID_HOME=~/Library/Android/sdk ANDROID_SDK_ROOT=~/Library/Android/sdk ./gradlew :core:test --rerun-tasks --tests org.hyperledger.iroha.android.GradleHarnessTests`
- `cargo test --workspace` (long run; observed failure in `extra_functional::unstable_network::unstable_network_5_peers_1_fault`)
- `cargo test -p integration_tests unstable_network_5_peers_1_fault -- --nocapture` (pass on focused rerun; flake root-cause follow-up documented below)

## 2026-03-09 Unstable Network Retry-Flake Hardening
- Closed the `unstable_network_5_peers_1_fault` retry gap that could consume full retry windows even after tx commit:
  - `integration_tests/tests/extra_functional/unstable_network.rs`
  - `is_submission_accepted_duplicate(...)` now accepts both enqueue and committed duplicate responses (`ALREADY_ENQUEUED` / `ALREADY_COMMITTED` and lowercase committed text), avoiding false retry loops after successful commit.
- Updated regression in the same module:
  - `submit_acceptance_accepts_enqueued_or_committed_duplicate`
- Operational cleanup during investigation:
  - removed orphaned `iroha3d` test-network processes left by an interrupted prior run before revalidation.

### Validation Matrix (Unstable Network Retry-Flake Hardening)
- `cargo test -p integration_tests --test mod submit_acceptance_accepts_enqueued_or_committed_duplicate -- --nocapture`
- `cargo test -p integration_tests --test mod unstable_network_5_peers_1_fault -- --nocapture` (pass, ~47.76s)
- `cargo test -p integration_tests --test mod unstable_network_5_peers_1_fault -- --nocapture` (pass, ~46.92s)

## 2026-03-09 Nexus Unregister Fail-Closed Account Literal Resolution
- Hardened Nexus account-config unregister guards to fail closed for account literals in:
  - `crates/iroha_core/src/smartcontracts/isi/domain.rs`
  - `crates/iroha_core/src/smartcontracts/isi/world.rs`
- `nexus.fees.fee_sink_account_id`, `nexus.staking.stake_escrow_account_id`, and
  `nexus.staking.slash_sink_account_id` now resolve against active world subject membership and return
  `InvariantViolation` when the literal is invalid, ambiguous across same-subject multi-domain accounts,
  or otherwise not resolvable to a unique active scoped account.
- Kept exact-scoped matching semantics for multi-domain subjects:
  - cross-domain same-subject accounts are not overblocked unless the literal resolves to that exact scoped account.
- `Unregister<Account>` now resolves all three Nexus account literals fail-closed before match/no-match decisions.
- `Unregister<Domain>` now runs fail-closed Nexus account checks for all domain member accounts before any state mutation path.
- Added regressions:
  - `unregister_account_rejects_when_nexus_fee_sink_literal_is_ambiguous_across_same_subject_domains`
  - `unregister_domain_rejects_when_nexus_fee_sink_literal_is_ambiguous_across_same_subject_domains`
  - `unregister_account_rejects_when_nexus_fee_sink_literal_is_invalid`
  - `unregister_domain_rejects_when_nexus_fee_sink_literal_is_invalid`
- Extended unregister semantics docs with explicit fail-closed behavior for invalid/ambiguous/non-resolvable
  Nexus account literals:
  - `docs/source/data_model_and_isi_spec.md`

### Validation Matrix (Nexus Unregister Fail-Closed Account Literal Resolution)
- `cargo test -p iroha_core --lib unregister_account_allows_when_nexus_fee_sink_account_is_same_subject_other_domain -- --nocapture`
- `cargo test -p iroha_core --lib unregister_domain_allows_when_nexus_fee_sink_account_is_same_subject_other_domain -- --nocapture`
- `cargo test -p iroha_core --lib unregister_account_rejects_when_account_is_nexus_ -- --nocapture`
- `cargo test -p iroha_core --lib unregister_domain_rejects_when_member_account_is_nexus_ -- --nocapture`
- `cargo test -p iroha_core --lib unregister_account_rejects_when_nexus_fee_sink_literal_is_ambiguous_across_same_subject_domains -- --nocapture`
- `cargo test -p iroha_core --lib unregister_domain_rejects_when_nexus_fee_sink_literal_is_ambiguous_across_same_subject_domains -- --nocapture`
- `cargo test -p iroha_core --lib unregister_account_rejects_when_nexus_fee_sink_literal_is_invalid -- --nocapture`
- `cargo test -p iroha_core --lib unregister_domain_rejects_when_nexus_fee_sink_literal_is_invalid -- --nocapture`
- `cargo check -p iroha_core`

## 2026-03-09 SNS Registrar JSON Error Handling and Owner Identity Assertions
- Hardened SNS client response handling in `crates/iroha/src/sns.rs`:
  - status code is validated before JSON decoding for all SNS endpoints
  - non-success responses now include contextual HTTP status/body diagnostics through `ResponseReport`
- Restored AccountAddress JSON roundtrip compatibility for canonical hex literals in:
  - `crates/iroha_data_model/src/account/address.rs`
  - `JsonDeserialize for AccountAddress` now falls back to canonical-hex decoding when encoded-address parsing reports `UnsupportedAddressFormat`
- Updated SNS integration ownership assertions to compare controller identity instead of strict scoped-domain equality:
  - `integration_tests/tests/sns.rs`
  - aligns expectations with domainless I105 owner literals used by SNS JSON payloads

### Validation Matrix (SNS Registrar JSON Error Handling and Owner Identity Assertions)
- `cargo test -p integration_tests --test sns -- --nocapture`
- `cargo test -p iroha --lib ensure_status_reports_text_body_when_status_mismatches -- --nocapture`
- `cargo test -p iroha_data_model --lib account_address_json_roundtrip_supports_canonical_hex_literals -- --nocapture`
- `cargo fmt --all`

## 2026-03-08 Norito Instruction Fixture Refresh
- Refreshed stale fixture payloads in `fixtures/norito_instructions` to match current canonical Rust Norito encoding for:
  - `burn_asset_numeric.json`
  - `burn_asset_fractional.json`
  - `mint_asset_numeric.json`
- Updated both `instruction` (base64 Norito frame) and `encoded_hex` (canonical payload bytes) fields.
- Simplified fixture descriptions to avoid embedding stale encoded asset-id literals.

### Validation Matrix (Norito Instruction Fixture Refresh)
- `cargo test -p integration_tests --test norito_burn_fixture -- --nocapture`

## 2026-03-09 Oracle Feed-History Unregister Guards
- Closed remaining account/domain unregister referential gap for oracle audit history:
  - `Unregister<Account>` now rejects when the account appears in any `oracle_history` success-entry provider (`ReportEntry.oracle_id`) reference:
    - `crates/iroha_core/src/smartcontracts/isi/domain.rs`
  - `Unregister<Domain>` now rejects when any member account being removed appears in `oracle_history` success-entry provider references:
    - `crates/iroha_core/src/smartcontracts/isi/world.rs`
- Added regressions:
  - `unregister_account_rejects_when_account_has_oracle_feed_history_state`
  - `unregister_domain_rejects_when_member_account_has_oracle_feed_history_state`
- Updated docs wording for unregister guard rails to include oracle feed-history provider references:
  - `docs/source/data_model_and_isi_spec.md`

### Validation Matrix (Oracle Feed-History Unregister Guards)
- `cargo fmt --all`
- `cargo test -p iroha_core --lib unregister_account_rejects_when_account_has_oracle_feed_history_state -- --nocapture`
- `cargo test -p iroha_core --lib unregister_domain_rejects_when_member_account_has_oracle_feed_history_state -- --nocapture`
- `cargo check -p iroha_core`

## 2026-03-09 Nexus Config Unregister Guard Hardening
- Closed remaining account/asset-definition unregister guard gaps for Nexus config references:
  - `Unregister<Account>` now rejects removal when account matches:
    - `nexus.fees.fee_sink_account_id`
    - `nexus.staking.stake_escrow_account_id`
    - `nexus.staking.slash_sink_account_id`
  - `Unregister<AssetDefinition>` now rejects removal when definition matches:
    - `nexus.fees.fee_asset_id`
    - `nexus.staking.stake_asset_id`
  - `Unregister<Domain>` now applies both guard sets to member-account and domain-asset-definition teardown.
- Hardened Nexus account-reference matching to avoid false positives across multi-domain subjects:
  - account guard matching now resolves config literals against world state and compares exact scoped account IDs (instead of subject-only matching), so removing domain-B account no longer fails when Nexus config resolves to domain-A account with the same controller.
  - files:
    - `crates/iroha_core/src/smartcontracts/isi/domain.rs`
    - `crates/iroha_core/src/smartcontracts/isi/world.rs`
- Added regressions:
  - `unregister_account_rejects_when_account_is_nexus_fee_sink_account`
  - `unregister_account_rejects_when_account_is_nexus_staking_escrow_account`
  - `unregister_account_rejects_when_account_is_nexus_staking_slash_sink_account`
  - `unregister_asset_definition_rejects_when_definition_is_nexus_fee_asset`
  - `unregister_asset_definition_rejects_when_definition_is_nexus_staking_asset`
  - `unregister_domain_rejects_when_member_account_is_nexus_fee_sink_account`
  - `unregister_domain_rejects_when_member_account_is_nexus_staking_escrow_account`
  - `unregister_domain_rejects_when_member_account_is_nexus_staking_slash_sink_account`
  - `unregister_domain_rejects_when_domain_asset_definition_is_nexus_fee_asset`
  - `unregister_domain_rejects_when_domain_asset_definition_is_nexus_staking_asset`
  - `unregister_account_allows_when_nexus_fee_sink_account_is_same_subject_other_domain`
  - `unregister_domain_allows_when_nexus_fee_sink_account_is_same_subject_other_domain`
- Updated unregister spec wording to include Nexus config account/asset-definition references:
  - `docs/source/data_model_and_isi_spec.md`

### Validation Matrix (Nexus Config Unregister Guard Hardening)
- `cargo test -p iroha_core --lib unregister_account_rejects_when_account_is_nexus_ -- --nocapture`
- `cargo test -p iroha_core --lib unregister_domain_rejects_when_member_account_is_nexus_ -- --nocapture`
- `cargo test -p iroha_core --lib unregister_account_allows_when_nexus_fee_sink_account_is_same_subject_other_domain -- --nocapture`
- `cargo test -p iroha_core --lib unregister_domain_allows_when_nexus_fee_sink_account_is_same_subject_other_domain -- --nocapture`
- `cargo fmt --all`
- `cargo check -p iroha_core`

## 2026-03-08 Data Model Consistency Sweep (Account/Domain/Dataspace/Asset)
- Fixed tracked asset-definition totals when cascading unregister operations remove assets:
  - Added `WorldTransaction::remove_asset_and_metadata_with_total(...)` in `crates/iroha_core/src/state.rs`.
  - Switched account/domain/asset-definition unregister paths to use the total-aware helper:
    - `crates/iroha_core/src/smartcontracts/isi/domain.rs`
    - `crates/iroha_core/src/smartcontracts/isi/world.rs`
- Removed query-time asset total recomputation workaround so `FindAssetsDefinitions` now relies on persisted totals:
  - `crates/iroha_core/src/smartcontracts/isi/asset.rs`
- Aligned UAID lane identity behavior to global routing semantics (dataspace binding no longer required for admission; inactive target manifest still rejects):
  - Queue admission path and tests: `crates/iroha_core/src/queue.rs`
  - Transaction lane-policy identity extraction + tests: `crates/iroha_core/src/tx.rs`
- Relaxed overly strict owner-domain coupling in formal verification invariants while keeping owner-exists checks:
  - `crates/iroha_data_model/src/verification.rs`
- Added direct unit coverage for new total-aware removal helper:
  - `state::tests::remove_asset_and_metadata_with_total_decrements_definition_total`
  - `state::tests::remove_asset_and_metadata_with_total_cleans_orphan_metadata`
- Added integration coverage for unregister cascade correctness of persisted totals:
  - `asset_totals_drop_when_unregistering_account`
  - `asset_totals_drop_when_unregistering_domain_with_foreign_holders`
  - `unregistering_definition_domain_cleans_foreign_assets`
  - file: `crates/iroha_core/tests/asset_total_amount.rs`
- Removed obsolete queue rejection surface for UAID dataspace binding:
  - dropped `queue::Error::UaidNotBound` from `crates/iroha_core/src/queue.rs`
  - removed corresponding Torii status/reason mappings and stale telemetry-reason aggregation label in `crates/iroha_torii/src/lib.rs`
- Centralized lane identity metadata extraction to a single shared helper:
  - added `extract_lane_identity_metadata(...)` and `LaneIdentityMetadataError` in `crates/iroha_core/src/nexus/space_directory.rs`
  - switched both queue admission and tx lane policy paths to this shared helper:
    - `crates/iroha_core/src/queue.rs`
    - `crates/iroha_core/src/tx.rs`
  - added direct helper tests:
    - `nexus::space_directory::tests::lane_identity_metadata_allows_missing_target_manifest`
    - `nexus::space_directory::tests::lane_identity_metadata_rejects_inactive_target_manifest`
- Added data-model verification regression coverage for cross-domain ownership references:
  - `verification::tests::cross_domain_owners_are_allowed_when_references_exist`
  - file: `crates/iroha_data_model/src/verification.rs`
- Enforced ownership integrity on unregister paths so account/domain removal cannot orphan ownership references:
  - `Unregister<Account>` now rejects when the target account still owns any domain or asset definition:
    - `crates/iroha_core/src/smartcontracts/isi/domain.rs`
  - `Unregister<Domain>` now rejects when accounts being removed still own domains or asset definitions outside the domain being deleted:
    - `crates/iroha_core/src/smartcontracts/isi/world.rs`
  - added regressions:
    - `unregister_account_rejects_when_account_owns_domain`
    - `unregister_account_rejects_when_account_owns_asset_definition`
    - `unregister_domain_rejects_when_member_account_owns_foreign_domain`
    - `unregister_domain_rejects_when_member_account_owns_foreign_asset_definition`
- Hardened asset-definition ownership transfer authorization across both sequential and detached pipelines:
  - `Transfer<Account, AssetDefinitionId, Account>` now enforces the same authority model as domain transfer (source account, source-domain owner, or definition-domain owner):
    - `crates/iroha_core/src/smartcontracts/isi/account.rs`
  - added detached-delta authorization helper and checks so parallel overlay execution cannot bypass ownership checks:
    - `crates/iroha_core/src/state.rs`
    - `crates/iroha_core/src/block.rs`
  - added initial-executor precheck parity for asset-definition transfers:
    - `crates/iroha_core/src/executor.rs`
  - added regressions:
    - `transfer_asset_definition_rejects_unauthorized_authority`
    - `transfer_asset_definition_allows_definition_domain_owner`
    - `detached_can_transfer_asset_definition_denies_non_owner`
    - `detached_can_transfer_asset_definition_considers_pending_domain_transfers`
    - `initial_executor_denies_transfer_asset_definition_without_ownership`
    - `initial_executor_allows_transfer_asset_definition_by_definition_domain_owner`
- Closed remaining SoraFS provider-owner referential gaps around account/domain lifecycle:
  - `Unregister<Account>` now rejects when the account is still referenced as a SoraFS provider owner:
    - `crates/iroha_core/src/smartcontracts/isi/domain.rs`
  - `Unregister<Domain>` now rejects when any member account being deleted still owns a SoraFS provider:
    - `crates/iroha_core/src/smartcontracts/isi/world.rs`
  - `RegisterProviderOwner` now requires the destination owner account to exist before inserting bindings:
    - `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`
  - `State::set_gov` now skips configured SoraFS provider-owner bindings whose owner account does not exist:
    - `crates/iroha_core/src/state.rs`
  - added regressions:
    - `unregister_account_rejects_when_account_owns_sorafs_provider`
    - `unregister_domain_rejects_when_member_account_owns_sorafs_provider`
    - `register_provider_owner_rejects_missing_owner_account`
    - `set_gov_skips_sorafs_provider_owner_without_account`
- Enforced NFT transfer authorization symmetry across sequential and detached pipelines:
  - `Transfer<Account, NftId, Account>` now requires authority to be source account, source-domain owner, NFT-domain owner, or holder of `CanTransferNft` for the target NFT:
    - `crates/iroha_core/src/smartcontracts/isi/nft.rs`
  - added initial-executor precheck parity for NFT transfers:
    - `crates/iroha_core/src/executor.rs`
  - added detached overlay precheck parity for NFT transfers:
    - `crates/iroha_core/src/state.rs`
    - `crates/iroha_core/src/block.rs`
  - added regressions:
    - `transfer_nft_rejects_authority_without_ownership`
    - `transfer_nft_allows_nft_domain_owner`
    - `initial_executor_denies_transfer_nft_without_ownership`
    - `initial_executor_allows_transfer_nft_by_nft_domain_owner`
    - `detached_can_transfer_nft_denies_non_owner`
    - `detached_can_transfer_nft_considers_pending_domain_transfers`
- Guarded account/domain unregister against orphaning governance citizenship records:
  - `Unregister<Account>` now rejects when the account has an active citizenship record:
    - `crates/iroha_core/src/smartcontracts/isi/domain.rs`
  - `Unregister<Domain>` now rejects when any member account being removed has an active citizenship record:
    - `crates/iroha_core/src/smartcontracts/isi/world.rs`
  - added regressions:
    - `unregister_account_rejects_when_account_has_citizenship_record`
    - `unregister_domain_rejects_when_member_account_has_citizenship_record`
- Guarded account/domain unregister against orphaning public-lane staking references:
  - `Unregister<Account>` now rejects when the account still appears in public-lane validator/stake/reward/reward-claim registries:
    - `crates/iroha_core/src/smartcontracts/isi/domain.rs`
  - `Unregister<Domain>` now rejects when member accounts being removed still appear in public-lane validator/stake/reward/reward-claim registries:
    - `crates/iroha_core/src/smartcontracts/isi/world.rs`
  - added regressions:
    - `unregister_account_rejects_when_account_has_public_lane_validator_state`
    - `unregister_domain_rejects_when_member_account_has_public_lane_validator_state`
    - `unregister_account_rejects_when_account_has_public_lane_reward_record_state`
    - `unregister_domain_rejects_when_member_account_has_public_lane_reward_record_state`
- Guarded account/domain unregister against orphaning oracle references:
  - `Unregister<Account>` now rejects when the account still appears in oracle feed/change/dispute/provider-stats/observation state:
    - `crates/iroha_core/src/smartcontracts/isi/domain.rs`
  - `Unregister<Domain>` now rejects when member accounts being removed still appear in oracle feed/change/dispute/provider-stats/observation state:
    - `crates/iroha_core/src/smartcontracts/isi/world.rs`
  - added regressions:
    - `unregister_account_rejects_when_account_has_oracle_feed_provider_state`
    - `unregister_domain_rejects_when_member_account_has_oracle_feed_provider_state`
- Guarded account/domain unregister against orphaning repo agreement references:
  - `Unregister<Account>` now rejects when the account appears as initiator/counterparty/custodian in active repo agreements:
    - `crates/iroha_core/src/smartcontracts/isi/domain.rs`
  - `Unregister<Domain>` now rejects when member accounts being removed appear in active repo agreements:
    - `crates/iroha_core/src/smartcontracts/isi/world.rs`
  - added regressions:
    - `unregister_account_rejects_when_account_has_repo_agreement_state`
    - `unregister_domain_rejects_when_member_account_has_repo_agreement_state`
- Guarded account/domain unregister against orphaning settlement-ledger references:
  - `Unregister<Account>` now rejects when the account appears in settlement ledger authority/leg records:
    - `crates/iroha_core/src/smartcontracts/isi/domain.rs`
  - `Unregister<Domain>` now rejects when member accounts being removed appear in settlement ledger authority/leg records:
    - `crates/iroha_core/src/smartcontracts/isi/world.rs`
  - added regressions:
    - `unregister_account_rejects_when_account_has_settlement_ledger_state`
    - `unregister_domain_rejects_when_member_account_has_settlement_ledger_state`
- Guarded account/domain unregister against orphaning offline settlement references:
  - `Unregister<Account>` now rejects when the account appears in active offline allowance or transfer records:
    - `crates/iroha_core/src/smartcontracts/isi/domain.rs`
  - `Unregister<Domain>` now rejects when member accounts being removed appear in active offline allowance or transfer records:
    - `crates/iroha_core/src/smartcontracts/isi/world.rs`
  - successful account/domain removal now drops stale sender/receiver offline transfer index entries for removed accounts:
    - `crates/iroha_core/src/smartcontracts/isi/domain.rs`
    - `crates/iroha_core/src/smartcontracts/isi/world.rs`
  - added regressions:
    - `unregister_account_rejects_when_account_has_offline_allowance_state`
    - `unregister_domain_rejects_when_member_account_has_offline_transfer_state`
- Closed remaining account/domain unregister referential gaps across offline-governance-content state:
  - `Unregister<Account>` now additionally rejects when the account appears in:
    - offline verdict revocations (`offline_verdict_revocations`)
    - governance proposal/stage-approval/lock/slash ledgers
    - governance council/parliament rosters
    - content bundle creator references (`content_bundles.created_by`)
    - `crates/iroha_core/src/smartcontracts/isi/domain.rs`
  - `Unregister<Domain>` now additionally rejects when any member account being removed appears in those same offline/governance/content stores:
    - `crates/iroha_core/src/smartcontracts/isi/world.rs`
  - added regressions:
    - `unregister_account_rejects_when_account_has_offline_verdict_revocation_state`
    - `unregister_domain_rejects_when_member_account_has_offline_verdict_revocation_state`
    - `unregister_account_rejects_when_account_has_governance_proposal_state`
    - `unregister_domain_rejects_when_member_account_has_governance_proposal_state`
    - `unregister_account_rejects_when_account_has_content_bundle_state`
    - `unregister_domain_rejects_when_member_account_has_content_bundle_state`
- Extended account/domain unregister guard rails to additional live account-reference stores:
  - runtime upgrade proposer references (`runtime_upgrades.proposer`)
  - oracle twitter-binding provider references (`twitter_bindings.provider`)
  - social viral escrow sender references (`viral_escrows.sender`)
  - SoraFS pin-registry issuer/binder references:
    - `pin_manifests.submitted_by`
    - `manifest_aliases.bound_by`
    - `replication_orders.issued_by`
  - implemented in:
    - `crates/iroha_core/src/smartcontracts/isi/domain.rs`
    - `crates/iroha_core/src/smartcontracts/isi/world.rs`
  - added regressions:
    - `unregister_account_rejects_when_account_has_runtime_upgrade_state`
    - `unregister_domain_rejects_when_member_account_has_runtime_upgrade_state`
    - `unregister_account_rejects_when_account_has_viral_escrow_state`
    - `unregister_domain_rejects_when_member_account_has_viral_escrow_state`
    - `unregister_account_rejects_when_account_has_sorafs_pin_manifest_state`
    - `unregister_domain_rejects_when_member_account_has_sorafs_pin_manifest_state`
- Updated Space Directory and Nexus compliance docs to match global UAID routing semantics:
  - replaced outdated “UAID not bound => queue rejection” wording with “missing target manifest allowed; inactive manifest rejected”
  - touched multilingual variants in:
    - `docs/space-directory*.md`
    - `docs/source/nexus_compliance*.md`

### Validation Matrix (Data Model Consistency Sweep)
- `cargo fmt --all`
- `cargo test -p iroha_data_model --lib verification -- --nocapture`
- `cargo test -p iroha_core --lib uaid_ -- --nocapture`
- `cargo test -p iroha_core --lib lane_identity_ -- --nocapture`
- `cargo test -p iroha_core --lib lane_identity_metadata_ -- --nocapture`
- `cargo test -p iroha_core --lib remove_asset_and_metadata_with_total -- --nocapture`
- `cargo test -p iroha_core --lib unregister_account_rejects_when_account_owns -- --nocapture`
- `cargo test -p iroha_core --lib unregister_domain_rejects_when_member_account_owns -- --nocapture`
- `cargo test -p iroha_core --lib provider_owner_ -- --nocapture`
- `cargo test -p iroha_core --lib set_gov_ -- --nocapture`
- `cargo test -p iroha_core --lib transfer_nft -- --nocapture`
- `cargo test -p iroha_core --lib unregister_account_rejects_when_account_has_citizenship -- --nocapture`
- `cargo test -p iroha_core --lib unregister_domain_rejects_when_member_account_has_citizenship -- --nocapture`
- `cargo test -p iroha_core --lib unregister_account_rejects_when_account_has_public_lane_validator_state -- --nocapture`
- `cargo test -p iroha_core --lib unregister_domain_rejects_when_member_account_has_public_lane_validator_state -- --nocapture`
- `cargo test -p iroha_core --lib unregister_account_rejects_when_account_has_public_lane_reward_record_state -- --nocapture`
- `cargo test -p iroha_core --lib unregister_domain_rejects_when_member_account_has_public_lane_reward_record_state -- --nocapture`
- `cargo test -p iroha_core --lib unregister_account_rejects_when_account_has_oracle_feed_provider_state -- --nocapture`
- `cargo test -p iroha_core --lib unregister_domain_rejects_when_member_account_has_oracle_feed_provider_state -- --nocapture`
- `cargo test -p iroha_core --lib unregister_account_rejects_when_account_has_repo_agreement_state -- --nocapture`
- `cargo test -p iroha_core --lib unregister_domain_rejects_when_member_account_has_repo_agreement_state -- --nocapture`
- `cargo test -p iroha_core --lib unregister_account_rejects_when_account_has_settlement_ledger_state -- --nocapture`
- `cargo test -p iroha_core --lib unregister_domain_rejects_when_member_account_has_settlement_ledger_state -- --nocapture`
- `cargo test -p iroha_core --lib unregister_account_rejects_when_account_has_offline_allowance_state -- --nocapture`
- `cargo test -p iroha_core --lib unregister_domain_rejects_when_member_account_has_offline_transfer_state -- --nocapture`
- `cargo test -p iroha_core --lib unregister_account_rejects_when_account_has_ -- --nocapture`
- `cargo test -p iroha_core --lib unregister_domain_rejects_when_member_account_has_ -- --nocapture`
- `cargo check -p iroha_core`
- `cargo test -p iroha_core --lib transfer_asset_definition_ -- --nocapture`
- `cargo test -p iroha_core --test asset_total_amount -- --nocapture`
- `cargo test -p iroha_torii --lib tests_queue_metadata::queue_errors_map_to_reason_codes -- --nocapture`
- `cargo check -p iroha_data_model -p iroha_core -p iroha_torii`
- `cargo check -p iroha_core`

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
  - `../pk-deploy/scripts/cutover-i105-mega.sh`
  - `../pk-deploy/scripts/README-redeploy.md`
- Purged prior-transition terminology from touched runtime/docs/status surfaces.
- Reset status/history files to fresh baselines:
  - `status.md`
  - `roadmap.md`
  - `../pk-deploy/STATUS.md`

## Validation Matrix (This Pass)
- `bash -n ../pk-deploy/scripts/check-identity-surface.sh ../pk-deploy/scripts/cutover-i105-mega.sh ../pk-deploy/scripts/deploy-sbp-aed-pkr-interceptor.sh ../pk-cbuae-mock/scripts/e2e/localnet-live.sh`
- `bash ../pk-deploy/scripts/check-identity-surface.sh`
- identity-literal forbidden-token sweep across requested repos/files
- residual-token sweep across touched runtime/scripts/status files
- iOS targeted retest for previously failing flow:
  - `cd ../pk-retail-wallet-ios && xcodebuild test -scheme RetailWalletIOS -destination 'platform=iOS Simulator,name=iPhone 17,OS=26.1' -only-testing:RetailWalletIOSUITests/RetailWalletIOSFlowUITests/testOnboardingAndSendFlow`

## Remaining Actionable Blockers
- None in active A1-G1 runtime/parser/SDK paths.

## 2026-03-08 Unregister Referential-Integrity Guard Expansion (Account/Domain)
- Extended `Unregister<Account>` and `Unregister<Domain>` guard rails for additional account-reference state:
  - DA pin-intent owner references in `da_pin_intents_by_ticket` (`intent.owner`).
  - Lane-relay emergency validator overrides in `lane_relay_emergency_validators` (`validators`).
  - Governance proposal parliament snapshot rosters in `governance_proposals.parliament_snapshot.bodies`.
- Files updated:
  - `crates/iroha_core/src/smartcontracts/isi/domain.rs`
  - `crates/iroha_core/src/smartcontracts/isi/world.rs`
  - `docs/source/data_model_and_isi_spec.md`
- Added 6 targeted regression tests (3 account + 3 domain) covering the new reject-on-reference behavior.

### Validation Matrix (Unregister Guard Expansion)
- `cargo fmt --all`
- `cargo test -p iroha_core --lib unregister_account_rejects_when_account_has_ -- --nocapture` (pass)
- `cargo test -p iroha_core --lib unregister_domain_rejects_when_member_account_has_ -- --nocapture` (pass)
- `cargo test -p iroha_core --lib governance_parliament_snapshot_state -- --nocapture` (pass)
- `cargo check -p iroha_core` (pass)

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
- Updated Swift `AccountId.make` to return encoded I105 identifiers (domainless subject id surface) and aligned affected tests:
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
- Added `/v1/accounts/query` regression coverage for alias handling in Torii.
- The later 2026-03-10 hard-cut sweep tightened the same surface so compressed `AccountId` literals are rejected; see the latest 2026-03-11 entry for the current test names and validation commands.

### Validation Matrix (Alias Regression)
- Historical validation used the then-current `/v1/accounts/query` regression tests; the 2026-03-11 hard-cut entry supersedes those test names with strict compressed-literal rejection coverage.
- `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p integration_tests --test address_canonicalisation accounts_query_rejects_public_key_filter_literals -- --nocapture`
- `cargo fmt --all`

## 2026-03-08 Hard-Cut Debt Sweep
- Removed remaining Python runtime account parse compatibility surface:
  - deleted `AccountAddress.parse_any(...)`
  - removed canonical-hex parser entrypoint from Python account parsing
  - added strict `AccountAddress.parse_encoded(...)` (I105/sora compressed only; rejects `@domain` and canonical-hex input)
  - updated governance owner canonicalization to strict I105 decode path.
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
- `CARGO_TARGET_DIR=target_hardcut cargo test -p iroha_torii` account-filter regression coverage (later superseded by the 2026-03-10 strict compressed-literal rejection test names) (pass)
- `CARGO_TARGET_DIR=target_hardcut cargo test -p ivm pointer_to_norito_roundtrips_via_pointer_from_norito -- --nocapture` (pass)
- `CARGO_TARGET_DIR=target_hardcut cargo test -p iroha_js_host gateway_write_mode_parses_upload_hint -- --nocapture` (pass)

### Validation Matrix (2026-03-08)
- `CARGO_TARGET_DIR=target_hardcut cargo test -p iroha_data_model confidential_wallet_fixtures_are_stable -- --nocapture` (pass)
- `CARGO_TARGET_DIR=target_hardcut cargo test -p iroha_data_model offline_allowance_fixtures_roundtrip -- --nocapture` (pass)
- `CARGO_TARGET_DIR=target_hardcut cargo test --manifest-path scripts/export_norito_fixtures/Cargo.toml -- --nocapture` (pass)
- `CARGO_TARGET_DIR=target_hardcut cargo test --manifest-path python/iroha_python/iroha_python_rs/Cargo.toml attachments_json_decodes_versioned_signed_transaction -- --nocapture` (pass)
- `python -m pytest python/iroha_torii_client/tests/test_client.py -k "uaid_portfolio or space_directory_manifest or trigger_listing_and_lookup_roundtrip or offline_allowance"` in isolated venv (pass: 12 selected tests)
- `PYTHONPATH=python/iroha_python/src:python/iroha_torii_client python -m pytest python/iroha_python/tests/test_governance_zk_ballot.py` in isolated venv (pass: 12 tests)

## 2026-03-08 Asset-Definition Referential-Integrity Guard Expansion
- Extended `Unregister<AssetDefinition>` to reject unregister when the target definition is still referenced by:
  - repo agreements (`repo_agreements` cash/collateral legs),
  - settlement ledger legs (`settlement_ledgers`),
  - public-lane reward ledger and pending claims (`public_lane_rewards`, `public_lane_reward_claims`),
  - offline allowance and transfer receipts (`offline_allowances`, `offline_to_online_transfers`).
- Added confidential-state cascade cleanup during asset-definition unregister:
  - remove `world.zk_assets[asset_definition_id]` together with the definition.
- Extended `Unregister<Domain>` asset-definition teardown path to enforce the same asset-definition reference guards before deleting domain asset definitions, closing foreign-account orphan paths (domain asset defs referenced externally).
- Closed dataspace-catalog drift for emergency relay overrides:
  - `State::set_nexus(...)` now prunes `lane_relay_emergency_validators` entries whose dataspaces are removed from the new `dataspace_catalog`, preventing stale dataspace references.
- Extended the same `set_nexus(...)` dataspace-catalog pruning to Space Directory derived bindings:
  - stale `uaid_dataspaces` entries are now trimmed to active catalog dataspaces so removed dataspaces cannot survive in UAID->dataspace/account bindings.
- Extended `set_nexus(...)` dataspace-catalog pruning to cached AXT policy entries:
  - stale `axt_policies` dataspace keys are now removed when dataspaces disappear from `dataspace_catalog`.
- Added regression:
  - `set_nexus_prunes_lane_relay_emergency_overrides_for_removed_dataspaces`
  - `set_nexus_prunes_uaid_bindings_for_removed_dataspaces`
  - `set_nexus_removes_uaid_binding_when_all_dataspaces_are_pruned`
  - `set_nexus_prunes_axt_policies_for_removed_dataspaces`
- Added tests:
  - `unregister_asset_definition_rejects_when_definition_has_repo_agreement_state`
  - `unregister_asset_definition_rejects_when_definition_has_settlement_ledger_state`
  - `unregister_asset_definition_removes_confidential_state`
  - `unregister_domain_rejects_when_domain_asset_definition_has_foreign_repo_agreement_state`
- Updated `docs/source/data_model_and_isi_spec.md` unregister semantics for Domain/AssetDefinition guard rails and `zk_assets` cleanup.

### Validation Matrix (Asset-Definition Integrity)
- `cargo fmt --all` (pass)
- `cargo test -p iroha_core --lib unregister_asset_definition_rejects_when_definition_has_ -- --nocapture` (pass)
- `cargo test -p iroha_core --lib unregister_asset_definition_removes_confidential_state -- --nocapture` (pass)
- `cargo test -p iroha_core --lib unregister_asset_definition_ -- --nocapture` (pass)
- `cargo test -p iroha_core --lib unregister_domain_rejects_when_domain_asset_definition_has_foreign_repo_agreement_state -- --nocapture` (pass)
- `cargo test -p iroha_core --lib unregister_domain_rejects_when_member_account_has_ -- --nocapture` (pass)
- `cargo test -p iroha_core --lib set_nexus_prunes_lane_relay_emergency_overrides_for_removed_dataspaces -- --nocapture` (pass)
- `cargo test -p iroha_core --lib set_nexus_prunes_uaid_bindings_for_removed_dataspaces -- --nocapture` (pass)
- `cargo test -p iroha_core --lib set_nexus_removes_uaid_binding_when_all_dataspaces_are_pruned -- --nocapture` (pass)
- `cargo test -p iroha_core --lib set_nexus_prunes_axt_policies_for_removed_dataspaces -- --nocapture` (pass)
- `cargo check -p iroha_core` (pass)

## 2026-03-08 Public-Lane Reward-Claim Ownership Guard Fix
- Closed a remaining account/domain unregister gap in `public_lane_reward_claims`:
  - `Unregister<Account>` now rejects not only when the account is the claim claimant, but also when it is referenced as `asset_id.account()` in pending reward-claim keys.
  - `Unregister<Domain>` now applies the same claimant-or-asset-owner guard for each member account being removed.
- Files updated:
  - `crates/iroha_core/src/smartcontracts/isi/domain.rs`
  - `crates/iroha_core/src/smartcontracts/isi/world.rs`
- Added regressions:
  - `unregister_account_rejects_when_account_is_reward_claim_asset_owner`
  - `unregister_domain_rejects_when_member_account_is_reward_claim_asset_owner`

### Validation Matrix (Reward-Claim Ownership Guard)
- `cargo fmt --all` (pass)
- `cargo test -p iroha_core --lib unregister_account_rejects_when_account_is_reward_claim_asset_owner -- --nocapture` (pass)
- `cargo test -p iroha_core --lib unregister_domain_rejects_when_member_account_is_reward_claim_asset_owner -- --nocapture` (pass)
- `cargo check -p iroha_core` (pass)

## 2026-03-08 Governance-Config Reference Guard Expansion
- Closed a remaining unregister integrity gap for governance-configured account/asset references:
  - `Unregister<Account>` now rejects removal when the account is configured as:
    - `gov.bond_escrow_account`
    - `gov.citizenship_escrow_account`
    - `gov.slash_receiver_account`
    - `gov.viral_incentives.incentive_pool_account`
    - `gov.viral_incentives.escrow_account`
  - `Unregister<Domain>` now applies the same governance-account guard for each member account being removed.
  - `Unregister<AssetDefinition>` now rejects removal when the definition is configured as:
    - `gov.voting_asset_id`
    - `gov.citizenship_asset_id`
    - `gov.parliament_eligibility_asset_id`
    - `gov.viral_incentives.reward_asset_definition_id`
  - `Unregister<Domain>` now applies the same governance-asset-definition guard before deleting domain asset definitions.
- Files updated:
  - `crates/iroha_core/src/smartcontracts/isi/domain.rs`
  - `crates/iroha_core/src/smartcontracts/isi/world.rs`
  - `docs/source/data_model_and_isi_spec.md`
- Added regressions:
  - `unregister_account_rejects_when_account_is_governance_bond_escrow_account`
  - `unregister_account_rejects_when_account_is_governance_viral_incentive_pool_account`
  - `unregister_asset_definition_rejects_when_definition_is_governance_voting_asset`
  - `unregister_asset_definition_rejects_when_definition_is_governance_viral_reward_asset`
  - `unregister_domain_rejects_when_member_account_is_governance_bond_escrow_account`
  - `unregister_domain_rejects_when_member_account_is_governance_viral_incentive_pool_account`
  - `unregister_domain_rejects_when_domain_asset_definition_is_governance_voting_asset`
  - `unregister_domain_rejects_when_domain_asset_definition_is_governance_viral_reward_asset`

### Validation Matrix (Governance-Config Guard)
- `cargo fmt --all` (pass)
- `cargo test -p iroha_core --lib governance_bond_escrow_account -- --nocapture` (pass)
- `cargo test -p iroha_core --lib governance_voting_asset -- --nocapture` (pass)
- `cargo test -p iroha_core --lib governance_viral -- --nocapture` (pass)
- `cargo check -p iroha_core` (pass)

## 2026-03-08 Oracle-Economics Config Reference Guard Expansion
- Closed remaining unregister integrity gaps for oracle-economics configured account/asset references:
  - `Unregister<Account>` now rejects removal when the account is configured as:
    - `oracle.economics.reward_pool`
    - `oracle.economics.slash_receiver`
  - `Unregister<Domain>` now applies the same oracle-economics account guard for each member account being removed.
  - `Unregister<AssetDefinition>` now rejects removal when the definition is configured as:
    - `oracle.economics.reward_asset`
    - `oracle.economics.slash_asset`
    - `oracle.economics.dispute_bond_asset`
  - `Unregister<Domain>` now applies the same oracle-economics asset-definition guard before deleting domain asset definitions.
- Files updated:
  - `crates/iroha_core/src/smartcontracts/isi/domain.rs`
  - `crates/iroha_core/src/smartcontracts/isi/world.rs`
  - `docs/source/data_model_and_isi_spec.md`
- Added regressions:
  - `unregister_account_rejects_when_account_is_oracle_reward_pool`
  - `unregister_asset_definition_rejects_when_definition_is_oracle_reward_asset`
  - `unregister_domain_rejects_when_member_account_is_oracle_reward_pool`
  - `unregister_domain_rejects_when_domain_asset_definition_is_oracle_reward_asset`

### Validation Matrix (Oracle-Economics Config Guard)
- `cargo fmt --all` (pass)
- `cargo test -p iroha_core --lib oracle_reward_pool -- --nocapture` (pass)
- `cargo test -p iroha_core --lib oracle_reward_asset -- --nocapture` (pass)
- `cargo check -p iroha_core` (pass)

## 2026-03-08 Offline-Escrow Reference Integrity Expansion
- Closed remaining unregister integrity gaps around `settlement.offline.escrow_accounts` (`AssetDefinitionId -> AccountId`):
  - `Unregister<Account>` now rejects removal when the account is configured as an offline escrow account for an active asset definition.
  - `Unregister<Domain>` now rejects removal when a member account is configured as an offline escrow account for an active asset definition that remains outside the domain.
  - `Unregister<AssetDefinition>` now prunes the matching `settlement.offline.escrow_accounts` entry when the definition is deleted.
  - `Unregister<Domain>` now prunes `settlement.offline.escrow_accounts` entries for all domain asset definitions removed during domain teardown.
- Files updated:
  - `crates/iroha_core/src/smartcontracts/isi/domain.rs`
  - `crates/iroha_core/src/smartcontracts/isi/world.rs`
  - `docs/source/data_model_and_isi_spec.md`
- Added regressions:
  - `unregister_account_rejects_when_account_is_offline_escrow_account`
  - `unregister_asset_definition_removes_offline_escrow_mapping`
  - `unregister_domain_rejects_when_member_account_is_offline_escrow_for_retained_asset_definition`
  - `unregister_domain_removes_offline_escrow_mappings_for_domain_asset_definitions`

### Validation Matrix (Offline-Escrow Integrity)
- `cargo fmt --all` (pass)
- `cargo test -p iroha_core --lib unregister_account_rejects_when_account_is_offline_escrow_account -- --nocapture` (pass)
- `cargo test -p iroha_core --lib unregister_asset_definition_removes_offline_escrow_mapping -- --nocapture` (pass)
- `cargo test -p iroha_core --lib unregister_domain_rejects_when_member_account_is_offline_escrow_for_retained_asset_definition -- --nocapture` (pass)
- `cargo test -p iroha_core --lib unregister_domain_removes_offline_escrow_mappings_for_domain_asset_definitions -- --nocapture` (pass)
- `cargo check -p iroha_core` (pass)

## 2026-03-09 Settlement-Repo Config Reference Guard Expansion
- Closed remaining unregister integrity gaps for settlement repo config asset-definition references:
  - `Unregister<AssetDefinition>` now rejects removal when the definition is configured in:
    - `settlement.repo.eligible_collateral`
    - `settlement.repo.collateral_substitution_matrix` (as base or substitute)
  - `Unregister<Domain>` now applies the same settlement-repo asset-definition guard before deleting domain asset definitions.
- Files updated:
  - `crates/iroha_core/src/smartcontracts/isi/domain.rs`
  - `crates/iroha_core/src/smartcontracts/isi/world.rs`
  - `docs/source/data_model_and_isi_spec.md`
- Added regressions:
  - `unregister_asset_definition_rejects_when_definition_is_settlement_repo_eligible_collateral`
  - `unregister_asset_definition_rejects_when_definition_is_settlement_repo_substitution_entry`
  - `unregister_domain_rejects_when_domain_asset_definition_is_settlement_repo_eligible_collateral`
  - `unregister_domain_rejects_when_domain_asset_definition_is_settlement_repo_substitution_entry`

### Validation Matrix (Settlement-Repo Config Guard)
- `cargo fmt --all` (pass)
- `cargo test -p iroha_core --lib unregister_asset_definition_rejects_when_definition_is_settlement_repo_eligible_collateral -- --nocapture` (pass)
- `cargo test -p iroha_core --lib unregister_asset_definition_rejects_when_definition_is_settlement_repo_substitution_entry -- --nocapture` (pass)
- `cargo test -p iroha_core --lib unregister_domain_rejects_when_domain_asset_definition_is_settlement_repo_eligible_collateral -- --nocapture` (pass)
- `cargo test -p iroha_core --lib unregister_domain_rejects_when_domain_asset_definition_is_settlement_repo_substitution_entry -- --nocapture` (pass)
- `cargo check -p iroha_core` (pass)

## 2026-03-09 Content + SoraFS Telemetry Account-Reference Guard Expansion
- Closed remaining unregister integrity gaps for config account references used by content and SoraFS telemetry admission:
  - `Unregister<Account>` now rejects removal when the account is configured in:
    - `content.publish_allow_accounts`
    - `gov.sorafs_telemetry.submitters`
    - `gov.sorafs_telemetry.per_provider_submitters`
  - `Unregister<Domain>` now applies the same account-reference guards for each member account being removed.
- Files updated:
  - `crates/iroha_core/src/smartcontracts/isi/domain.rs`
  - `crates/iroha_core/src/smartcontracts/isi/world.rs`
  - `docs/source/data_model_and_isi_spec.md`
- Added regressions:
  - `unregister_account_rejects_when_account_is_content_publish_allow_account`
  - `unregister_account_rejects_when_account_is_sorafs_telemetry_submitter`
  - `unregister_domain_rejects_when_member_account_is_content_publish_allow_account`
  - `unregister_domain_rejects_when_member_account_is_sorafs_per_provider_telemetry_submitter`

### Validation Matrix (Content + SoraFS Telemetry Guard)
- `cargo fmt --all` (pass)
- `cargo test -p iroha_core --lib unregister_account_rejects_when_account_is_content_publish_allow_account -- --nocapture` (pass)
- `cargo test -p iroha_core --lib unregister_account_rejects_when_account_is_sorafs_telemetry_submitter -- --nocapture` (pass)
- `cargo test -p iroha_core --lib unregister_domain_rejects_when_member_account_is_content_publish_allow_account -- --nocapture` (pass)
- `cargo test -p iroha_core --lib unregister_domain_rejects_when_member_account_is_sorafs_per_provider_telemetry_submitter -- --nocapture` (pass)
- `cargo check -p iroha_core` (pass)

## 2026-03-09 Governance SoraFS Provider-Owner Config Guard Expansion
- Closed remaining unregister integrity gap for governance-configured SoraFS provider-owner account references:
  - `Unregister<Account>` now rejects removal when the account is configured in:
    - `gov.sorafs_provider_owners` (as provider owner)
  - `Unregister<Domain>` now applies the same governance provider-owner guard for each member account being removed.
- Files updated:
  - `crates/iroha_core/src/smartcontracts/isi/domain.rs`
  - `crates/iroha_core/src/smartcontracts/isi/world.rs`
  - `docs/source/data_model_and_isi_spec.md`
- Added regressions:
  - `unregister_account_rejects_when_account_is_configured_sorafs_provider_owner`
  - `unregister_domain_rejects_when_member_account_is_configured_sorafs_provider_owner`

### Validation Matrix (Governance SoraFS Provider-Owner Guard)
- `cargo fmt --all` (pass)
- `cargo test -p iroha_core --lib unregister_account_rejects_when_account_is_configured_sorafs_provider_owner -- --nocapture` (pass)
- `cargo test -p iroha_core --lib unregister_domain_rejects_when_member_account_is_configured_sorafs_provider_owner -- --nocapture` (pass)
- `cargo check -p iroha_core` (pass)

## 2026-03-09 Permission Referential Cleanup Hardening (Unregister Paths)
- Closed remaining permission-orphan gaps when unregistering accounts/domains/assets/NFTs/triggers:
  - `Unregister<Account>` now prunes account-/role-scoped permissions by the current identifier semantics without cross-domain over-pruning.
  - `Unregister<Domain>` now prunes only permissions tied to the removed domain and other resources deleted during domain teardown; surviving domainless accounts keep account-target permissions and other foreign/global references.
  - `Unregister<AssetDefinition>` now prunes account-/role-scoped permissions that reference the removed asset definition and asset-instance-scoped permissions anchored to that definition.
  - `Unregister<Nft>` now prunes account-/role-scoped permissions that reference the removed NFT.
  - `Unregister<Trigger>` now prunes account-/role-scoped permissions that reference the removed trigger.
  - `Unregister<Account>` prunes account-/role-scoped NFT-target permissions for NFTs deleted transitively because they are owned by the removed account; `Unregister<Domain>` only prunes NFT-target permissions for NFTs deleted as part of the removed domain itself.
  - `Unregister<Account>` prunes governance account-target permissions `CanRecordCitizenService{owner: ...}` when the referenced owner account is removed; `Unregister<Domain>` preserves those permissions for surviving domainless accounts linked elsewhere.
  - Detached merge (`DetachedStateTransactionDelta::merge_into`) now also prunes account-/role-scoped NFT-target permissions when applying queued NFT deletions, keeping detached and sequential execution semantics aligned.
  - `State::set_nexus` now also prunes account-/role-scoped dataspace-target permissions `CanPublishSpaceDirectoryManifest{dataspace: ...}` when dataspaces are removed from the active Nexus dataspace catalog.
  - `State::set_nexus` now prunes stale dataspace entries from `space_directory_manifests`, so removed dataspaces cannot be rehydrated into UAID dataspace bindings by later manifest lifecycle updates.
  - `State::set_nexus` now prunes stale dataspace entries from `axt_replay_ledger`, so replay-state records cannot retain removed-dataspace references after catalog updates.
  - Lane-scoped relay/DA caches (`lane_relays`, `da_commitments`, `da_confidential_compute`, `da_pin_intents`) are now pruned when a lane is retired or reassigned to a different dataspace (same lane id, new `dataspace_id`) in both `State::set_nexus(...)` and lane lifecycle application.
  - Space Directory manifest ISIs (`PublishSpaceDirectoryManifest`, `RevokeSpaceDirectoryManifest`, `ExpireSpaceDirectoryManifest`) now reject unknown dataspace IDs by validating against the active `nexus.dataspace_catalog` before permission/lifecycle mutation.
  - Trigger deletions that happen transitively during `Unregister<Account>`, `Unregister<Domain>`, contract-instance deactivation, and repeat-depletion cleanup now invoke the same trigger-permission pruning path.
- Files updated:
  - `crates/iroha_core/src/smartcontracts/isi/domain.rs`
  - `crates/iroha_core/src/smartcontracts/isi/nft.rs`
  - `crates/iroha_core/src/smartcontracts/isi/triggers/mod.rs`
  - `crates/iroha_core/src/smartcontracts/isi/world.rs`
  - `crates/iroha_core/src/state.rs`
  - `docs/source/data_model_and_isi_spec.md`
- Added regressions:
  - `unregister_account_removes_associated_permissions_from_accounts_and_roles`
  - `unregister_domain_removes_account_target_permissions_from_accounts_and_roles`
  - `unregister_account_removes_foreign_nft_permissions_from_accounts_and_roles`
  - `unregister_domain_removes_foreign_nft_permissions_from_accounts_and_roles`
  - `delta_merge_unregister_nft_prunes_associated_permissions`
  - `unregister_account_removes_citizen_service_permissions_from_accounts_and_roles`
  - `unregister_domain_removes_citizen_service_permissions_from_accounts_and_roles`
  - `unregister_account_preserves_other_domain_permissions_for_same_subject`
  - `unregister_domain_preserves_other_domain_permissions_for_same_subject`
  - `set_nexus_prunes_manifest_permissions_for_removed_dataspaces`
  - `set_nexus_prunes_space_directory_manifests_for_removed_dataspaces`
  - `set_nexus_prunes_axt_replay_entries_for_removed_dataspaces`
  - `set_nexus_prunes_lane_state_when_lane_dataspace_changes`
  - `apply_lane_lifecycle_prunes_lane_state_when_lane_dataspace_changes`
  - `publish_manifest_rejects_unknown_dataspace`
  - `revoke_manifest_rejects_unknown_dataspace`
  - `expire_manifest_rejects_unknown_dataspace`
  - `unregister_asset_definition_removes_associated_permissions_from_accounts_and_roles`
  - `unregister_nft_removes_associated_permissions_from_accounts_and_roles`
  - `unregister_trigger_removes_associated_permissions_from_accounts_and_roles`

### Validation Matrix (Permission Referential Cleanup)
- `cargo test -p iroha_core --lib unregister_trigger_removes_associated_permissions_from_accounts_and_roles` (pass)
- `cargo test -p iroha_core --lib by_call_trigger_is_pruned_after_manual_execution` (pass)
- `cargo test -p iroha_core --lib active_trigger_ids_excludes_depleted_after_burn` (pass)
- `cargo test -p iroha_core --lib unregister_nft_removes_associated_permissions_from_accounts_and_roles` (pass)
- `cargo test -p iroha_core --lib unregister_asset_definition_removes_associated_permissions_from_accounts_and_roles` (pass)
- `cargo test -p iroha_core --lib unregister_account_removes_associated_permissions_from_accounts_and_roles` (pass)
- `cargo test -p iroha_core --lib unregister_account_rejects_when_account_has_oracle_feed_history_state` (pass)
- `cargo test -p iroha_core --lib unregister_domain_rejects_when_member_account_has_oracle_feed_history_state` (pass)
- `cargo test -p iroha_core --lib unregister_account_removes_foreign_nft_permissions_from_accounts_and_roles` (pass)
- `cargo test -p iroha_core --lib unregister_domain_removes_account_target_permissions_from_accounts_and_roles` (pass)
- `cargo test -p iroha_core --lib unregister_domain_removes_associated_permissions_from_accounts_and_roles` (pass)
- `cargo test -p iroha_core --lib unregister_domain_removes_foreign_nft_permissions_from_accounts_and_roles` (pass)
- `cargo test -p iroha_core --lib delta_merge_unregister_nft_prunes_associated_permissions` (pass)
- `cargo test -p iroha_core --lib unregister_account_removes_citizen_service_permissions_from_accounts_and_roles` (pass)
- `cargo test -p iroha_core --lib unregister_domain_removes_citizen_service_permissions_from_accounts_and_roles` (pass)
- `cargo test -p iroha_core --lib unregister_account_preserves_other_domain_permissions_for_same_subject` (pass)
- `cargo test -p iroha_core --lib unregister_domain_preserves_other_domain_permissions_for_same_subject` (pass)
- `cargo test -p iroha_core --lib set_nexus_prunes_` (pass)
- `cargo test -p iroha_core --lib lane_dataspace_changes -- --nocapture` (pass)
- `cargo test -p iroha_core --lib apply_lane_lifecycle_retire_prunes_lane_relays -- --nocapture` (pass)
- `cargo test -p iroha_core --lib set_nexus_prunes_manifest_permissions_for_removed_dataspaces` (pass)
- `cargo test -p iroha_core --lib set_nexus_prunes_space_directory_manifests_for_removed_dataspaces` (pass)
- `cargo test -p iroha_core --lib space_directory` (pass)
- `cargo fmt --all` (pass)
- `cargo check -p iroha_core` (pass)

## 2026-03-10 Domainless Account Data-Model Stabilization
- Closed the remaining `iroha_data_model` regressions blocking the domainless rollout hard-cut:
  - `AccountId` identity semantics are now controller-based (`PartialEq`/`Ord`/`Hash`), so domain scope metadata no longer fractures subject identity in JSON/query/ISI roundtrips.
  - Updated legacy canonical-hex rejection tests to match the strict encoded-account policy (I105/compressed-only public parsing).
  - Updated account-address error-vector expectations for the strict parser surface (no `InvalidHexAddress` requirement in auto-detect vectors).
  - Regenerated `fixtures/norito_rpc` payload/signed fixtures and manifest hashes for the current codec behavior.
  - Hardened NRPC fixture validation to keep strict hash/length checks for all fixtures while applying deep semantic roundtrip assertions only to fixtures that decode under the current instruction registry.
- Files updated:
  - `crates/iroha_data_model/src/account.rs`
  - `crates/iroha_data_model/src/account/address.rs`
  - `crates/iroha_data_model/src/account/address/vectors.rs`
  - `crates/iroha_data_model/src/transaction/signed.rs`
  - `fixtures/norito_rpc/transaction_fixtures.manifest.json`
  - `fixtures/norito_rpc/register_asset_definition.norito`
  - `fixtures/norito_rpc/transfer_asset.norito`
  - `fixtures/norito_rpc/mint_asset.norito`
  - `fixtures/norito_rpc/burn_asset.norito`
  - `fixtures/norito_rpc/register_time_trigger_demo.norito`

### Validation Matrix (Domainless Data-Model Stabilization)
- `cargo fmt --all` (pass)
- `cargo test -p iroha_data_model --lib` (pass)

## 2026-03-10 Torii/CLI Compile-Blocker Closure (Follow-up)
- Resolved the remaining parser/type blockers that were preventing targeted crate validation after MCP gap closure.
- Key follow-up fixes were applied in:
  - `crates/iroha_torii/src/routing.rs`
  - `crates/iroha_torii/src/iso20022_bridge.rs`
  - `crates/iroha_torii/src/test_utils.rs`
  - `crates/iroha_torii/src/sorafs/registry.rs`
- Validation results:
  - `cargo check -p iroha_config` (pass)
  - `cargo check -p iroha_torii` (pass)
  - `cargo check -p iroha_cli` (pass)

## 2026-03-10 Remaining Gap Closure (Torii MCP + Test Targets)
- Closed the remaining MCP/test-target gap by fixing the runtime annotation in MCP toolset-version tracking test (`tools_list_list_changed_tracks_toolset_version`) so test-only initialization runs under Tokio.
- Cleared residual `--all-targets` warnings in Torii integration tests (`offline_app_api`, `gov_read_endpoints`).
- Validation results:
  - `cargo test -p iroha_torii --lib --no-run` (pass)
  - `cargo test -p iroha_torii --lib mcp::tests:: -- --nocapture` (pass, 60/60)
  - `cargo test -p iroha_cli --no-run` (pass)
  - `cargo test -p iroha_torii --test offline_app_api --no-run` (pass)
  - `cargo test -p iroha_torii --test offline_certificates_app_api --no-run` (pass)
  - `cargo check -p iroha_torii --all-targets` (pass)

## 2026-03-10 MCP Documentation Accuracy Refresh
- Rewrote `crates/iroha_torii/docs/mcp_api.md` to reflect current runtime behavior and configuration:
  - Exact endpoint behavior (`GET /v1/mcp`, `POST /v1/mcp`) and HTTP status mapping.
  - Complete JSON-RPC method contract (`initialize`, `tools/list`, `tools/call`, `tools/call_batch`, `tools/call_async`, `tools/jobs/get`).
  - Policy/profile semantics and allow/deny prefix behavior.
  - Auth/header forwarding behavior and argument/response schemas used by route-dispatched tools.
  - Async job lifecycle/retention semantics (`async_job_ttl_secs`, `async_job_max_entries`).
  - Updated minimal end-to-end examples for discovery, call, batch, and async polling.

## 2026-03-10 MCP Documentation Discoverability Pass
- Added operator-facing MCP documentation entry points so bot integrators can find the contract from main docs navigation:
  - Added `docs/portal/docs/reference/torii-mcp.md` with configuration, discovery/call flow, auth forwarding, error model, and tool naming guidance.
  - Linked the new reference page from `docs/portal/sidebars.js` (`Reference` section) and `docs/portal/docs/reference/README.md`.
  - Added a top-level source-doc index link in `docs/source/README.md` to the canonical MCP spec (`crates/iroha_torii/docs/mcp_api.md`).

## 2026-03-10 MCP Docs Cross-Link Hardening
- Closed remaining MCP documentation usability gaps in Torii-facing docs:
  - Fixed truncated migration guidance in `docs/source/torii/router.md` and added direct MCP spec cross-link under further reading.
  - Added MCP bridge context to `docs/portal/docs/api/overview.mdx` so users understand `/v1/mcp` is JSON-RPC and should use the dedicated MCP reference.
  - Updated `docs/portal/docs/reference/torii-swagger.mdx` usage notes to explicitly redirect MCP users to `/reference/torii-mcp`.

## 2026-03-10 MCP Reference Localization Parity
- Propagated MCP reference discoverability across localized portal reference indexes:
  - Added `/reference/torii-mcp` bullet entries to every `docs/portal/docs/reference/README*.md` variant (21/21 files), so localized docs now point to MCP usage guidance alongside OpenAPI.
- Propagated MCP discoverability across localized Dev Portal usage docs:
  - Added MCP pointers to every `docs/portal/docs/devportal/try-it*.md` variant (21/21), clarifying that `/v1/mcp` agent workflows should use `/reference/torii-mcp`.
  - Added MCP pointers to every `docs/portal/docs/devportal/torii-rpc-overview*.md` variant (21/21) near the Swagger/Try-It flow section.
- Validation:
  - `rg -l '/reference/torii-mcp' docs/portal/docs/reference/README*.md | wc -l` -> `21`
  - `ls docs/portal/docs/reference/README*.md | wc -l` -> `21`
  - `rg -n '/reference/torii-mcp' docs/portal/docs/devportal/try-it*.md | wc -l` -> `21`
  - `ls docs/portal/docs/devportal/try-it*.md | wc -l` -> `21`
  - `rg -n '/reference/torii-mcp' docs/portal/docs/devportal/torii-rpc-overview*.md | wc -l` -> `21`
  - `ls docs/portal/docs/devportal/torii-rpc-overview*.md | wc -l` -> `21`
- Portal build attempt:
  - `npm run build` (blocked in prebuild because `cargo xtask` subcommand is unavailable in this environment: `error: no such command: xtask`).
  - Retried with a temporary `cargo-xtask` PATH wrapper that strips the extra `xtask` token and executes `cargo run -p xtask --bin xtask -- ...`; prebuild then succeeds.
  - Installed portal deps with `npm install --no-package-lock` for local validation.
  - `DOCS_OAUTH_ALLOW_INSECURE=1 npm run build` now reaches Docusaurus and fails on pre-existing duplicate doc IDs in `versioned_docs/version-2025-q2/*` (`sorafs/node-operations`, `sorafs/pin-registry-ops`, `sorafs/staging-manifest-playbook`), unrelated to MCP documentation edits.

## 2026-03-10 MCP Contract Docs Tightening
- Tightened MCP documentation to match current runtime behavior exactly across canonical + portal docs:
  - Updated `crates/iroha_torii/docs/mcp_api.md` with compatibility details for `jsonrpc` handling (`"2.0"` recommended, omitted accepted), `params` fallback semantics, and explicit `tools/list.cursor` behavior (numeric-string offset; invalid values fall back to `0`).
  - Added missing HTTP-layer caveats for `/v1/mcp`: middleware-level `403` (API token rejection), `404` when MCP is disabled, and `405` for unsupported methods.
  - Documented `arguments.headers` restrictions (`content-length`, `host`, `connection` ignored), plus request-body precedence/defaults (`body_base64` over `body`, default content types).
  - Added route-dispatched `structuredContent.error_code` mapping guidance by HTTP status family.
  - Clarified top-level JSON-RPC error-code expectations vs tool-runtime failures (`result.isError` + `structuredContent.error_code`).
  - Mirrored the same contract clarifications in `docs/portal/docs/reference/torii-mcp.md`.

## 2026-03-10 MCP Source-Docs Localization Parity
- Propagated MCP discoverability from English source docs into localized source-doc variants:
  - Added Torii MCP crate-spec link (`../../crates/iroha_torii/docs/mcp_api.md`) to every `docs/source/README*.md` variant (21/21), alongside existing Torii ZK crate-doc pointers.
  - Added Torii MCP further-reading link to every `docs/source/torii/router*.md` variant (21/21), including two truncated localized variants (`router.he.md`, `router.ja.md`) that lacked the section.
- Validation:
  - `rg -l 'mcp_api\\.md' docs/source/README*.md | wc -l` -> `21`
  - `ls docs/source/README*.md | wc -l` -> `21`
  - `rg -l 'mcp_api\\.md' docs/source/torii/router*.md | wc -l` -> `21`
  - `ls docs/source/torii/router*.md | wc -l` -> `21`

## 2026-03-10 I105 Hard-Cut Follow-up (Torii + Python Standalone Client)
- Torii address-format preference hard-cut:
  - Removed legacy `Compressed` variant from `crates/iroha_torii/src/address_format.rs`.
  - `AddressFormatPreference::from_param` now accepts only `i105` (or empty/default) and rejects legacy aliases.
  - Telemetry label is now fixed to `i105` for this preference surface.
- Explorer query preference tests updated to enforce i105-only semantics in `crates/iroha_torii/src/explorer.rs`.
- Standalone Python Torii client hard-cut:
  - Removed `address_format` request options from:
    - `get_explorer_account_qr`
    - `get_uaid_bindings`
    - `get_uaid_manifests`
  - Tightened explorer QR response parsing to require `address_format` = `i105` when provided, defaulting missing values to `i105`.
  - Updated targeted tests in `python/iroha_torii_client/tests/test_client.py` and added a regression for rejecting legacy QR payload format values.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_torii from_param_defaults_and_accepts_i105_only -- --nocapture` (pass)
  - `cargo test -p iroha_torii address_format_query_preference_accepts_i105_and_rejects_legacy_aliases -- --nocapture` (pass)
  - `python3 -m py_compile python/iroha_torii_client/client.py python/iroha_torii_client/tests/test_client.py` (pass)
  - `python3 -m pytest ...` could not run in this environment (`pytest` module not installed).

## 2026-03-11 I105 Hard-Cut Continuation (Torii + Integration)
- Torii tests hard-cut to canonical I105-only request surfaces:
  - `crates/iroha_torii/tests/offline_receipts.rs` no longer sends `address_format` query params.
  - `crates/iroha_torii/tests/nexus_dataspaces_summary.rs` removed `address_format` query usage and dropped unknown-format rejection coverage.
  - `crates/iroha_torii/tests/kaigi_endpoints.rs` now validates canonical I105 relay/reporter literals without `address_format` toggles.
- Torii formatter preference hard-cut:
  - `crates/iroha_torii/src/address_format.rs` no longer exposes enum variants; it is now a single canonical formatter type.
  - Call sites were updated from `AddressFormatPreference::I105` to `AddressFormatPreference` (removes format-variant plumbing while preserving canonical rendering and telemetry accounting).
- Integration rebaseline:
  - `integration_tests/tests/address_canonicalisation.rs` removed explicit `address_format` request payload/URL plumbing and renamed affected tests to I105-oriented naming.
  - Legacy unknown-`address_format` expectation coverage was removed from this file.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_torii --test offline_receipts --test kaigi_endpoints --test nexus_dataspaces_summary --features app_api,telemetry -- --nocapture` (pass)
  - `cargo test -p integration_tests --test address_canonicalisation --no-run` (pass)
- `cargo test -p integration_tests --test address_canonicalisation emit_i105_literals -- --nocapture` (pass; 6 tests)

## 2026-03-11 I105 Hard-Cut Continuation (Torii Explorer + Serializer Plumbing)
- Removed remaining no-op format threading from core Torii explorer DTO/lookup paths:
  - `crates/iroha_torii/src/explorer.rs`
    - `instruction_dto_with_kind`, `transaction_summary_dto`, and `transaction_detail_dto` no longer accept `AddressFormatPreference`.
  - `crates/iroha_torii/src/routing.rs`
    - Explorer collection/detail helpers (`collect_*`, `find_*`, `*_at_height`) no longer thread formatter arguments.
    - Explorer endpoint handlers (`handle_v1_explorer_transaction_detail`, `handle_v1_explorer_instruction_detail`, `handle_v1_explorer_account_qr`) no longer accept format arguments.
    - Callers in `crates/iroha_torii/src/lib.rs` updated to match.
- Collapsed additional internal formatter plumbing in Torii list/projection helpers:
  - `tx_projections_to_json`, `RepoAgreementProjection::from_agreement`,
    `manifest_entry_to_json`/`bindings_for_dataspace`,
    `offline_*_item_to_json`,
    `validator_record_to_json`, `stake_share_to_json`, `pending_reward_to_json`,
    and `offline_transfer_item_to_json` now use canonical I105 rendering directly.
- Telemetry hard-cut follow-through:
  - `record_address_format_selection` in `crates/iroha_torii/src/routing.rs` now takes only `(telemetry, endpoint)` and records the fixed `i105` label via canonical helper.
  - Callers no longer pass formatter values into telemetry accounting.
- Added canonical helper functions in `crates/iroha_torii/src/address_format.rs`
  (`display_literal`, `display_from_literal`, `metric_label`) while keeping compatibility wrappers.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo check -p iroha_torii --features app_api,telemetry` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-check cargo check -p iroha_torii --features app_api,telemetry` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-check cargo test -p iroha_torii --features app_api,telemetry explorer_detail_lookup_returns_transaction_and_instruction -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-check cargo test -p iroha_torii --features app_api,telemetry tx_projection_display_tests -- --nocapture` (pass)

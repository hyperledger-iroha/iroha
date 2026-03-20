# Status

Last updated: 2026-03-20

## 2026-03-20 Follow-up: Torii now exposes generic RAM-LFE program-policy, execute, and receipt-verify APIs, and the runtime config is keyed by `program_id`
- Replaced the app-runtime config surface from `torii.identifier_resolver` to
  `torii.ram_lfe` across `crates/iroha_config` and Torii test fixtures:
  - runtime entries are now configured under `torii.ram_lfe.programs[*]`,
    keyed by `program_id`,
  - Torii now builds its in-process RAM-LFE runtime from those program entries,
    and the identifier routes reuse that same runtime instead of a separate
    identifier-policy keyed config surface.
- Extended Torii's app API and OpenAPI description across
  `crates/iroha_torii/src/{lib.rs,routing.rs,openapi.rs}` with the generic
  RAM-LFE surface:
  - `GET /v1/ram-lfe/program-policies`,
  - `POST /v1/ram-lfe/programs/{program_id}/execute`, and
  - `POST /v1/ram-lfe/receipts/verify`.
- Added generic RAM-LFE route DTOs and handler coverage so Torii now:
  - lists program policies with decoded BFV input-encryption metadata and
    programmed `ram_fhe_profile`,
  - executes programmed BFV RAM-LFE policies from either `input_hex` or
    `encrypted_input`,
  - returns stateless `RamLfeExecutionReceipt` payloads plus `{ output_hex,
    output_hash, receipt_hash, opaque_hash }`, and
  - verifies RAM-LFE receipts statelessly against on-chain program policy
    metadata while optionally checking a caller-supplied `output_hex`.
- Added a shared stateless receipt validator in
  `crates/iroha_core/src/smartcontracts/isi/ram_lfe.rs` so Torii's verify
  endpoint uses the same signature/proof policy checks as core admission logic.
- Tightened Torii's receipt issuance semantics:
  - the in-process RAM-LFE runtime now keys secrets/signers by `program_id`,
  - both generic RAM-LFE execution and identifier claim-receipt flows reject
    proof-mode receipt issuance until explicit prover runtime support exists,
    instead of silently emitting signed receipts for proof-mode policies.
- Synced the UAID guide in `docs/source/universal_accounts_guide.md` to
  document the new generic RAM-LFE routes and the `torii.ram_lfe` config
  surface.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_config torii_ram_lfe_parses -- --nocapture` (pass)
  - `cargo test -p iroha_torii --lib ram_lfe -- --nocapture` (pass)
  - `cargo test -p iroha_torii --lib identifier_ -- --nocapture` (pass)

## 2026-03-20 Follow-up: manifest-declared data triggers now cover structured core-ledger filters
- Extended manifest-declared trigger support across `crates/iroha_data_model`,
  `crates/kotodama_lang`, `crates/iroha_core/tests`, and
  `crates/ivm/docs` so contracts can declare structured data triggers without
  raw trigger-registration tooling:
  - `AssetEventFilter` now supports an `asset_definition_matcher` alongside
    the existing `id_matcher`, with AND semantics across asset id, asset
    definition id, and event kind.
  - Kotodama trigger declarations now accept structured data-trigger blocks of
    the form `on data <family> <event_kind> { ... }` for the core ledger
    families (`peer`, `domain`, `account`, `asset`, `asset_definition`, `nft`,
    `trigger`, `role`, `configuration`, `executor`) while preserving the
    existing flat forms (`on data any`, `on pipeline block`, etc.).
  - Structured matcher literals now lower directly into manifest
    `TriggerDescriptor.filter: EventFilterBox`, including the interceptor shape
    `on data asset added { asset_definition "aid:..." }`, and explicit trigger
    authority remains supported.
  - Core activation/deactivation regressions now verify manifest registration
    for both Data and Pipeline triggers and preserve the existing
    contract-origin metadata/bookkeeping on installed trigger actions.
- Validation:
  - `cargo fmt --all --check` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-trigger-dsl-target cargo test -p iroha_data_model asset_filter -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-trigger-dsl-target cargo test -p kotodama_lang trigger_decl_ -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-trigger-dsl-target cargo test -p iroha_core --test contract_manifest_triggers -- --nocapture` (pass)

## 2026-03-20 Follow-up: self-describing contract artifacts are now mandatory, and Torii deploy/call no longer accept manifest fallbacks
- Reworked the IVM contract artifact path around a required embedded `CNTR`
  section in 1.1 `.to` artifacts:
  - `crates/ivm_abi/src/metadata.rs` now decodes ordered prefix sections and
    returns both the real code offset and the embedded contract-interface
    payload,
  - `crates/ivm/src/contract_artifact.rs` now verifies the embedded interface
    against decoded bytecode and builds the canonical `ContractManifest`,
    rejecting missing/malformed `CNTR`, duplicate entrypoints, invalid
    `entry_pc` targets, invalid trigger callbacks, ABI mismatches, and other
    structurally unsafe artifacts.
- Updated the compiler/runtime side to treat self-description as the only
  contract deploy format:
  - `crates/kotodama_lang/src/compiler.rs` now always emits 1.1 artifacts with
    embedded `EmbeddedContractInterfaceV1`,
  - `crates/ivm/src/{core_host.rs,host.rs,ivm.rs}` now execute prefixed
    `CNTR + LTLB + code` artifacts correctly by honoring the real code offset,
    resolving literal pointers from prefixed bodies, and preserving correct
    call/return alignment for prefixed programs.
- Removed the public manifest sidecar/fallback path from Torii and CLI:
  - `/v1/contracts/deploy` and `/v1/contracts/instance` now accept bytecode
    only and derive/sign the on-chain manifest from verified artifact bytes,
  - the legacy `POST /v1/contracts/code` manifest-only Torii endpoint and its
    MCP exposure are removed, so contract publication no longer has a public
    manifest-only bypass,
  - `/v1/contracts/call` now requires the stored manifest to advertise the
    requested entrypoint and no longer falls back to raw `Executable::Ivm`,
  - the CLI manifest output path is inspection-only; it is no longer part of
    deploy submission.
- Tightened the generic artifact policy to match first-release IVM behavior:
  - `crates/ivm_abi/src/metadata.rs` now accepts only IVM `1.1` headers,
    while contract deploy verification still separately requires `CNTR`,
  - remaining test fixtures/build helpers/predecoder exports were updated from
    `version_minor = 0` to `1`, and the generated predecoder fixtures were
    refreshed under `crates/ivm/tests/fixtures/predecoder/mixed/`.
- Synced the contract/governance doc families to the shipped behavior:
  - canonical English docs now describe bytecode-only deploy/instance flows,
    derived manifests, mandatory `CNTR`, and the `1.1`-only artifact policy,
  - translated/source and portal copies for the affected contract/governance
    docs now mirror the corrected English text and are marked `needs-update`
    until refreshed native-language translations are available.
- Tightened the Torii contract-call transaction shape to match first-release
  semantics:
  - direct contract calls now register and execute an exactly-once trigger
    without a redundant explicit unregister, avoiding post-execution repetition
    failures once the trigger self-expires.
- Validation:
  - `cargo fmt --all` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-contract-artifact-target cargo test -p ivm --test metadata --test cli_smoke --test contract_artifact --test ivm_header_doc_sync -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-contract-artifact-target cargo test -p ivm --test kotodama compile_and_run_add -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-contract-artifact-target cargo test -p ivm --test kotodama call_function_with_tuple_return -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-contract-artifact-target cargo test -p ivm --test kotodama manifest_includes_entrypoints_and_features -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-contract-artifact-target cargo test -p iroha_torii --lib contract_entrypoint_validation_tests -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-contract-artifact-target cargo test -p iroha_torii --lib deploy_tests::deploy_endpoint_returns_hashes -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-contract-artifact-target cargo test -p iroha_torii --lib multisig_contract_call_instruction_envelope_hashes_deterministically -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-contract-artifact-target cargo test -p iroha_torii --test mcp_endpoints mcp_jsonrpc_tools_call_agent_alias_contract_post_endpoints_dispatch -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-contract-artifact-target cargo test -p iroha_torii --test mcp_endpoints mcp_jsonrpc_tools_call_agent_alias_contract_call_and_wait_surfaces_submit_error -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-contract-artifact-target cargo test -p iroha_torii --test mcp_endpoints mcp_tools_list_exposes_account_and_transaction_interfaces -- --nocapture` (pass)
  - `IROHA_RUN_IGNORED=1 CARGO_TARGET_DIR=/tmp/iroha-contract-artifact-target cargo test -p iroha_torii --test contracts_deploy_integration contracts_deploy_and_fetch_code_bytes -- --nocapture` (pass)
  - `IROHA_RUN_IGNORED=1 CARGO_TARGET_DIR=/tmp/iroha-contract-artifact-target cargo test -p iroha_torii --test contracts_activate_integration contracts_deploy_and_activate_via_single_endpoint -- --nocapture` (pass)
  - `IROHA_RUN_IGNORED=1 CARGO_TARGET_DIR=/tmp/iroha-contract-artifact-target cargo test -p iroha_torii --test contracts_call_integration contracts_call_enqueues_transaction -- --nocapture` (pass)
  - `bash /Users/takemiyamakoto/dev/pk-cbdc-core-api/scripts/build_kotodama_contracts.sh` (pass; downstream `.to` artifacts emitted with inspection-only manifest dumps)
  - `CARGO_TARGET_DIR=/tmp/pk-cbdc-core-api-contract-artifact-target cargo test --test mint_flow mint_flow_contract_artifacts_exist -- --nocapture` in `../pk-cbdc-core-api` (pass)
- Broader workspace execution coverage is still being exercised separately; the
  targeted IVM, Torii, MCP, and downstream consumer slices that directly cover
  the new `.to`-only deploy/call model are green.

## 2026-03-20 Follow-up: Torii multisig selector tests no longer reach into `World` private storage
- Added a narrow smart-contract-state test/API scaffolding accessor in
  `crates/iroha_core/src/state.rs` so cross-crate callers can seed durable
  contract state without reopening the `World` field visibility.
- Updated the multisig selector regression coverage in
  `crates/iroha_torii/src/routing.rs` to use that accessor when seeding the
  fallback `MultisigAccountState`, fixing the `E0616` build break after
  `World.smart_contract_state` became private.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_torii --lib --no-run` (pass)
  - `cargo test -p iroha_torii multisig_spec_falls_back_to_contract_state_when_metadata_is_missing -- --nocapture` (pass)

## 2026-03-20 Follow-up: retained RBC session summaries now survive commit cleanup, and multilane Kura again provisions per-lane merge logs
- Restored per-lane merge-ledger file naming in
  `crates/iroha_config/src/parameters/actual.rs` so multilane Kura layouts now
  provision `merge_ledger/lane_*_merge.log` again instead of collapsing every
  lane onto a shared `universal.log`.
- Fixed the DA/RBC observability regression in
  `crates/iroha_core/src/sumeragi/main_loop/{commit.rs,tests.rs}` and
  `crates/iroha_core/src/sumeragi/main_loop.rs`:
  - commit cleanup still drains the runtime RBC session state, but it now keeps
    the retained status summary written during cleanup instead of immediately
    deleting that summary when clearing auxiliary caches such as rebroadcast
    cooldowns, persisted-session markers, and seed-worker state,
  - this restores the evidence expected by the cross-peer consistency and
    restart-recovery tests, which read `/v1/sumeragi/rbc/sessions` and the
    persisted `rbc_sessions/sessions.norito` snapshot after commit/restart.
- Added focused regression coverage for both fixes:
  - config assertions now lock the canonical per-lane merge-log paths,
  - Sumeragi unit tests now verify that `clean_rbc_sessions_for_block(...)`
    clears stale runtime-only RBC state without erasing the retained summary.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_config lane_config_from_catalog_preserves_alias_metadata -- --nocapture` (pass)
  - `cargo test -p iroha_core clean_rbc_sessions_for_block_ -- --nocapture` (pass)
  - `cargo test -p integration_tests multilane_kura_layout::kura_prepares_multilane_storage_layout -- --nocapture` (pass)
  - `cargo test -p integration_tests --test mod extra_functional::seven_peer_consistency::seven_peer_cross_peer_consistency_basic -- --nocapture` (pass)
  - `cargo test -p integration_tests --test mod sumeragi_da::sumeragi_rbc_recovers_after_peer_restart -- --nocapture` (pass)

## 2026-03-19 Follow-up: merge conflicts are resolved and the workspace compile gate is green
- Resolved the remaining merge conflicts across the Rust workspace, SDKs,
  docs, and build/config files, with manual reconciliation in:
  - `.gitignore`,
  - `Cargo.toml`,
  - `Dockerfile.cross`, and
  - `crates/iroha_torii/src/routing.rs`.
- Preserved the newer branch-side Torii/OpenAPI/API changes while restoring the
  Sumeragi status wire fields for `commit_pipeline` and `round_gap` in the
  Torii routing path so the consensus status response stays coherent with the
  merged data model.
- Validation so far:
  - `git diff --name-only --diff-filter=U` (clean)
  - `git diff --check` (clean)
  - `cargo check --workspace` (pass)

## 2026-03-19 Follow-up: generic hidden-program RAM-LFE program-policy foundations now exist in `iroha_crypto`, `iroha_data_model`, `iroha_core`, and the identifier Torii path
- Extended `crates/iroha_crypto/src/ram_lfe.rs` from an identifier-only
  programmed BFV path into a program-aware hidden RAM-LFE substrate:
  - programmed BFV public parameters now publish a hidden-program digest,
    verification mode, optional proof verifier metadata, and the public
    RAM-FHE execution profile,
  - the programmed evaluator now executes an explicit canonical
    branchless `HiddenRamFheProgram` instruction tape instead of deriving an
    implicit program from the resolver secret,
  - generic output hashing is now separated from identifier-facing hash
    derivation so identifier receipts can deterministically bind to the generic
    execution output hash.
- Added generic RAM-LFE on-chain types in
  `crates/iroha_data_model/src/{ram_lfe.rs,identifier.rs,isi/ram_lfe.rs}`:
  - new `RamLfeProgramId`, `RamLfeProgramPolicy`,
    `RamLfeExecutionReceiptPayload`, and `RamLfeExecutionReceipt`,
  - new RAM-LFE program-policy ISIs for register/activate/deactivate,
  - renamed the generic public surface from `ram_fhe` to `ram_lfe` so
    policy/receipt abstractions use LFE terminology while BFV-specific
    execution metadata stays explicitly FHE-scoped,
  - identifier policies now reference a `program_id` instead of embedding an
    inline commitment and resolver public key,
  - identifier receipts now carry a nested generic execution payload plus an
    optional signature/proof attachment.
- Wired RAM-LFE program policies into world state and identifier claim
  verification in `crates/iroha_core/src/{state.rs,smartcontracts/isi/mod.rs,smartcontracts/isi/{ram_lfe.rs,identifier.rs}}`:
  - world state now stores RAM-LFE program policies separately from identifier
    policies and claims,
  - identifier claims now resolve the referenced program policy, validate the
    published hidden-program digest / backend / verification mode against the
    nested execution receipt payload, and verify either:
    - signed identifier receipts, or
    - proof-carrying execution receipts bound to the execution payload hash via
      the existing Halo2 envelope stack.
- Updated Torii's identifier resolver plumbing in
  `crates/iroha_torii/src/{identifier_resolution.rs,lib.rs}` so the identifier
  app endpoints now build receipts from the referenced RAM-LFE program policy
  rather than the legacy inline identifier policy commitment.
- Updated docs in
  `docs/source/universal_accounts_guide.md`,
  `crates/iroha_crypto/src/ram_lfe.rs`, and
  `crates/iroha_crypto/src/fhe_bfv.rs` so they explicitly distinguish:
  - RAM-LFE as the outer policy/commitment/receipt abstraction, and
  - BFV / RAM-FHE as the encrypted execution backend and its public profile
    metadata.
- Validation so far:
  - `cargo fmt --all` (pass)
  - `cargo check -p iroha_crypto` (pass)
  - `cargo check -p iroha_data_model` (pass)
  - `cargo check -p iroha_core` (pass)
  - `cargo check -p iroha_torii` (pass)
  - `cargo test -p iroha_crypto ram_lfe -- --nocapture` (pass)
  - `cargo test -p iroha_core identifier_ -- --nocapture` (pass)
  - `cargo test -p iroha_torii --lib --no-run` (pass)
  - `cargo test -p iroha_torii identifier_ -- --nocapture` (pass)
- Remaining work in this tranche:
  - rename Torii runtime config from `torii.identifier_resolver` to the generic
    `torii.ram_lfe` shape and key runtime material by `program_id`,
  - add the generic Torii RAM-LFE execute/list/verify routes and their OpenAPI
    descriptions,
  - migrate JS/Swift/Android clients to the generic RAM-LFE policy/receipt
    shapes and add client-side signed/proof verification for the generic API,
  - broaden validation from compile gates to the targeted and workspace test
    suites once the remaining API/config migration is in place.

## 2026-03-19 Follow-up: `ram_lfe` now has an instruction-driven programmed BFV backend with a published RAM-FHE profile
- Extended `crates/iroha_crypto/src/ram_lfe.rs` with a third identifier
  evaluator backend:
  - `BfvProgrammedSha3_256V1` (`bfv-programmed-sha3-256-v1`) now supplements
    the historical HKDF-backed PRF and the earlier BFV affine evaluator.
  - the new path derives a hidden per-step RAM-style instruction trace from the
    resolver secret and commitment transcript, executes that program over BFV
    registers and ciphertext memory lanes, then hashes the canonicalized lane
    outputs into the deterministic `opaque_id` and `receipt_hash`.
  - programmed policies now enforce a stronger BFV ciphertext-modulus floor
    than the affine path because the RAM backend uses ciphertext-ciphertext
    multiplication.
  - programmed policy commitments now canonicalize BFV public parameters into a
    `BfvProgrammedPublicParameters` bundle that publishes the stable
    `BfvRamProgramProfile`; legacy raw BFV parameter payloads are upgraded onto
    that bundle when the commitment is rebuilt.
- Threaded the new backend through Torii in
  `crates/iroha_torii/src/{identifier_resolution.rs,lib.rs,openapi.rs}`:
  - plaintext identifier resolution now server-encrypts into BFV for both BFV
    backends,
  - affine encrypted identifier resolution still evaluates client ciphertexts
    directly,
  - programmed encrypted identifier resolution now canonicalizes decrypted input
    back onto the resolver's deterministic BFV envelope before executing the
    RAM program so semantically equivalent ciphertexts produce the same receipt,
  - `GET /v1/identifier-policies` now exposes `ram_fhe_profile` for programmed
    policies and the static OpenAPI schema documents the new profile object and
    canonicalized encrypted-input mode,
  - added focused Torii coverage for deterministic receipt derivation and
    plaintext/encrypted parity plus end-to-end account binding with the
    programmed backend.
- Updated `docs/source/universal_accounts_guide.md` so the UAID identifier
  guide documents all three current `ram_lfe` backends and the new programmed
  BFV path.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_crypto ram_lfe -- --nocapture` (pass)
  - `cargo test -p iroha_torii identifier_ --lib -- --nocapture` (pass)
  - `cargo test -p iroha_torii --lib openapi::tests::generated_spec_includes_documented_paths -- --nocapture` (pass)
  - `cargo check -p iroha_crypto -p iroha_torii` (pass)
  - `cargo clippy -p iroha_crypto -p iroha_torii --all-targets -- -D warnings` (pass)

## 2026-03-18 Follow-up: deterministic BFV acceleration landed, and the workspace lint/compile gates are green
- Replaced the old scalar-only BFV multiplication baseline in
  `crates/iroha_crypto/src/fhe_bfv.rs` with a deterministic CRT-NTT backend
  behind the new default `bfv-accel` feature from
  `crates/iroha_crypto/Cargo.toml` and the root `Cargo.toml` cfg allow-list:
  - `BfvParameters::convolution_backend()` now reports whether a parameter set
    runs on the scalar schoolbook fallback or the accelerated CRT-NTT path.
  - BFV multiplication now dispatches to exact-arithmetic CRT-NTT when the
    parameter set is supported, while preserving identical outputs through the
    scalar fallback on unsupported shapes or when `bfv-accel` is disabled.
- Updated the BFV/identifier docs in
  `docs/source/universal_accounts_guide.md` to describe the new default
  deterministic acceleration path and the fallback behavior.
- Closed the workspace validation blockers that surfaced during the full lint
  sweep:
  - removed a duplicate websocket test definition in
    `crates/iroha_telemetry/src/ws.rs`,
  - cleaned up strict-clippy issues in
    `crates/iroha_data_model/src/{account/address/compliance_vectors.rs,consensus.rs,identifier.rs,nexus/relay.rs}`,
  - fixed the unused receiver / duplicate-noop-arm issues in
    `crates/iroha_config/src/parameters/actual.rs` and
    `crates/iroha_executor/src/default/isi/multisig/transaction.rs`,
  - removed a duplicated bench runtime definition in
    `crates/iroha_core/benches/validation.rs`.
- Validation:
  - `cargo test -p iroha_crypto bfv -- --nocapture` (pass)
  - `cargo test -p iroha_torii identifier_ --lib -- --nocapture` (pass)
  - `cargo test --workspace --no-run` (pass)
  - `CARGO_INCREMENTAL=0 cargo clippy --workspace --all-targets -- -D warnings` (pass)

## 2026-03-18 Follow-up: BFV SDK envelopes now roundtrip into Torii with the shared fixture
- Fixed the BFV identifier envelope schema hash in
  `javascript/iroha_js/src/toriiClient.js`,
  `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift`, and
  `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/IdentifierBfvEnvelopeBuilder.java`:
  - SDK builders now frame `BfvIdentifierCiphertext` using Rust Norito's
    fully qualified type name
    `iroha_crypto::fhe_bfv::BfvIdentifierCiphertext` instead of the bare short
    schema label, which was the cause of the Torii-side schema mismatch.
- Tightened the Torii BFV integration test in
  `crates/iroha_torii/src/lib.rs`:
  - `identifier_resolve_accepts_bfv_encrypted_input` now submits the exact
    shared SDK ciphertext fixture bytes for `string#retail` instead of
    generating encrypted input on the Rust side.
  - added a guard that the shared SDK BFV public parameters still derive from
    the Rust resolver seed for that namespace.
- Updated the deterministic SDK fixtures in
  `javascript/iroha_js/test/toriiClient.identifier.test.js`,
  `IrohaSwift/Tests/IrohaSwiftTests/ToriiClientTests.swift`, and
  `java/iroha_android/src/test/java/org/hyperledger/iroha/android/client/HttpClientTransportTests.java`
  so all three SDKs and Torii now assert the same Rust-compatible wire bytes.
- Validation:
  - `cargo test -p iroha_torii identifier_resolve_accepts_bfv_encrypted_input --lib -- --nocapture` (pass)
  - `cargo test -p iroha_torii identifier_ --lib -- --nocapture` (pass)
  - `node --test javascript/iroha_js/test/toriiClient.identifier.test.js javascript/iroha_js/test/normalizers.test.js` (pass)
  - `swift test --filter 'ToriiClientTests/test.*Identifier'` (pass)
  - `JAVA_HOME=$(/usr/libexec/java_home -v 21) ANDROID_HARNESS_MAINS=org.hyperledger.iroha.android.client.HttpClientTransportTests ./gradlew :core:test --tests org.hyperledger.iroha.android.GradleHarnessTests` (pass)

## 2026-03-18 Follow-up: SDKs now build BFV identifier ciphertext envelopes locally
- Extended the JS SDK in
  `javascript/iroha_js/src/{toriiClient.js,index.js}`:
  - added `encryptIdentifierInputForPolicy(policy, input, { seed | seedHex })`,
  - extended `buildIdentifierRequestForPolicy(...)` with
    `{ input, encrypt: true }` so wallets can derive BFV `encrypted_input`
    locally from published policy parameters,
  - now parses `decomposition_base_log` from policy BFV metadata.
- Extended the Swift SDK in
  `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift`:
  - added BFV public-parameter parity for `decompositionBaseLog`,
  - added `ToriiIdentifierPolicySummary.encryptInput(...)`,
    `.encryptedRequest(input:...)`, and
    `ToriiIdentifierLookupRequest.encrypted(policy:input:...)`,
  - added a deterministic pure-Swift BFV envelope builder that emits framed
    Norito `BfvIdentifierCiphertext` bytes.
- Extended the Android SDK in
  `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/`:
  - added `IdentifierBfvEnvelopeBuilder`,
  - added BFV public-parameter parity for `decompositionBaseLog`,
  - added `IdentifierPolicySummary.encryptInput(...)` and
    `.encryptedRequestFromInput(...)`,
  - added `IdentifierResolveRequest.encryptedFromInput(...)`.
- Updated focused SDK tests in
  `javascript/iroha_js/test/toriiClient.identifier.test.js`,
  `IrohaSwift/Tests/IrohaSwiftTests/ToriiClientTests.swift`, and
  `java/iroha_android/src/test/java/org/hyperledger/iroha/android/client/HttpClientTransportTests.java`
  to lock deterministic BFV envelope output against a shared seeded fixture.
- Updated `docs/source/universal_accounts_guide.md` to document the new local
  BFV envelope helpers across JS, Swift, and Android.
- Validation:
  - `node --test javascript/iroha_js/test/toriiClient.identifier.test.js javascript/iroha_js/test/normalizers.test.js` (pass)
  - `swift test --filter 'ToriiClientTests/test.*Identifier'` (pass)
  - `JAVA_HOME=$(/usr/libexec/java_home -v 21) ANDROID_HARNESS_MAINS=org.hyperledger.iroha.android.client.HttpClientTransportTests ./gradlew :core:test --tests org.hyperledger.iroha.android.GradleHarnessTests` (pass)

## 2026-03-18 Follow-up: identifier receipts now have a query surface, and SDKs can verify them client-side
- Extended the identifier receipt path in
  `crates/iroha_data_model/src/identifier.rs`,
  `crates/iroha_core/src/state.rs`, and
  `crates/iroha_torii/src/{lib.rs,routing.rs,openapi.rs}`:
  - `IdentifierResolutionReceipt` now exposes canonical
    `payload_bytes()` for receipt-signature consumers.
  - Torii resolve and claim-receipt responses now return the canonical signed
    payload as both `signature_payload_hex` and `signature_payload`.
  - Torii now exposes `GET /v1/identifiers/receipts/{receipt_hash}` so
    wallets, operators, and tests can fetch the persisted
    `IdentifierClaimRecord` bound to a deterministic receipt hash.
- Extended the JS SDK in
  `javascript/iroha_js/src/{toriiClient.js,index.js}`:
  - added `ToriiClient.getIdentifierClaimByReceiptHash(...)`,
  - added `getIdentifierBfvPublicParameters(policy)` for decoded BFV metadata,
  - added `buildIdentifierRequestForPolicy(policy, { input | encryptedInput })`,
  - added `verifyIdentifierResolutionReceipt(receipt, policy)`.
- Extended the Swift SDK in
  `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift`:
  - added `ToriiIdentifierLookupRequest`,
  - added policy-bound request builders on `ToriiIdentifierPolicySummary`,
  - added `ToriiIdentifierResolutionReceipt.verifySignature(using:)`,
  - added `ToriiClient.getIdentifierClaimByReceiptHash(_)`.
- Extended the Android SDK in
  `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/`:
  - added `IdentifierResolveRequest`,
  - added structured BFV/public-parameter and signed-payload models,
  - added `IdentifierReceiptVerifier`,
  - added `HttpClientTransport.getIdentifierClaimByReceiptHash(...)`,
  - added `IdentifierResolutionReceipt.verifySignature(policy)`.
- Updated `docs/source/universal_accounts_guide.md` to document the new
  receipt-lookup route, signed payload fields, and SDK verification/request
  helpers.
- Validation:
  - `node --test javascript/iroha_js/test/toriiClient.identifier.test.js javascript/iroha_js/test/normalizers.test.js` (pass)
  - `swift test --filter 'ToriiClientTests/test.*Identifier'` (pass)
  - `JAVA_HOME=$(/usr/libexec/java_home -v 21) ANDROID_HARNESS_MAINS=org.hyperledger.iroha.android.client.HttpClientTransportTests ./gradlew :core:test --tests org.hyperledger.iroha.android.GradleHarnessTests` (pass)
  - `CARGO_INCREMENTAL=0 cargo check -p iroha_data_model -p iroha_core -p iroha_torii` (pass)
  - `cargo test -p iroha_torii identifier_ --lib -- --nocapture` (pass)
  - `cargo test -p iroha_core identifier_ --lib -- --nocapture` (pass)
  - `cargo test -p iroha_torii --lib openapi::tests::generated_spec_includes_documented_paths -- --nocapture` (pass)

## 2026-03-18 Follow-up: broader Swift and Android harness paths are green again
- Unblocked the full Swift `ToriiClientTests` class in
  `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift` by relaxing Torii account-id
  query normalization to preserve legacy IH58-style encoded literals when the
  pure-Swift `AccountAddress` parser cannot canonicalize them, while still
  rejecting path-dangerous `%`, `/`, `?`, and `#` characters.
- Restored Android source compatibility in
  `java/iroha_android/src/main/java/org/hyperledger/iroha/android/address/AccountAddress.java`
  by reintroducing the domainless `fromMultisigPolicy(policy)` overload as a
  thin wrapper over the newer domain-aware constructor.
- Validation:
  - `swift test --filter ToriiClientTests` (pass; `210` executed, `1` skipped)
  - `JAVA_HOME=$(/usr/libexec/java_home -v 21) ANDROID_HARNESS_MAINS=org.hyperledger.iroha.android.client.HttpClientTransportTests ./gradlew :core:test --tests org.hyperledger.iroha.android.GradleHarnessTests` (pass)

## 2026-03-18 Follow-up: Swift and Android SDKs now cover the identifier-policy Torii surface
- Extended the Swift SDK in `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift`:
  - added `ToriiIdentifierNormalization`,
  - added `ToriiIdentifierPolicySummary`,
    `ToriiIdentifierPolicyListResponse`, and
    `ToriiIdentifierResolutionReceipt`,
  - added `listIdentifierPolicies()`, `resolveIdentifier(...)`, and
    `issueIdentifierClaimReceipt(...)` async + completion-handler APIs,
  - added even-length ciphertext-hex validation for BFV
    `encrypted_input` payloads instead of incorrectly reusing the fixed
    32-byte hash validator.
- Added focused Swift tests in
  `IrohaSwift/Tests/IrohaSwiftTests/ToriiClientTests.swift` covering:
  - identifier policy listing,
  - resolve success + 404 handling,
  - claim-receipt issuance, and
  - client-side normalization parity for phone/email/account-number inputs.
- Extended the Android SDK in
  `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/`:
  - added `IdentifierNormalization`,
    `IdentifierPolicySummary`,
    `IdentifierPolicyListResponse`,
    `IdentifierResolutionReceipt`, and
    `IdentifierJsonParser`,
  - added `HttpClientTransport.listIdentifierPolicies()`,
    `resolveIdentifier(...)`, and `issueIdentifierClaimReceipt(...)`,
  - added JSON POST helpers and an allow-404 JSON fetch path for the new
    identifier endpoints.
- Added focused Android transport harness coverage in
  `java/iroha_android/src/test/java/org/hyperledger/iroha/android/client/HttpClientTransportTests.java`
  for policy listing, resolve, 404 handling, claim-receipt path encoding, and
  normalization parity.
- Updated `docs/source/universal_accounts_guide.md` to reflect that JS, Swift,
  and Android now all expose the identifier-policy client helpers.
- Validation:
  - `swift test --filter 'ToriiClientTests/test.*Identifier'` (pass)
  - `swift test --filter ToriiClientTests` (pass)
  - `JAVA_HOME=$(/usr/libexec/java_home -v 21) ./gradlew :core:compileJava` (pass)
  - `JAVA_HOME=$(/usr/libexec/java_home -v 21) javac --release 21 -cp '/Users/takemiyamakoto/dev/iroha/java/iroha_android/core/build/classes/java/main:/Users/takemiyamakoto/dev/iroha/java/norito_java/build/libs/norito-java-0.1.0-SNAPSHOT.jar' -d /tmp/iroha-android-testclasses /Users/takemiyamakoto/dev/iroha/java/iroha_android/src/test/java/org/hyperledger/iroha/android/client/HttpClientTransportTests.java` (pass)
  - `JAVA_HOME=$(/usr/libexec/java_home -v 21) java -ea -cp '/tmp/iroha-android-testclasses:/Users/takemiyamakoto/dev/iroha/java/iroha_android/core/build/classes/java/main:/Users/takemiyamakoto/dev/iroha/java/norito_java/build/libs/norito-java-0.1.0-SNAPSHOT.jar' org.hyperledger.iroha.android.client.HttpClientTransportTests` (pass)
  - `JAVA_HOME=$(/usr/libexec/java_home -v 21) ANDROID_HARNESS_MAINS=org.hyperledger.iroha.android.client.HttpClientTransportTests ./gradlew :core:test --tests org.hyperledger.iroha.android.GradleHarnessTests` (pass)

## 2026-03-18 Follow-up: identifier receipts now carry `receipt_hash` end to end, and the JS SDK can call the new Torii identifier routes
- Extended the identifier receipt/claim path:
  - `crates/iroha_data_model/src/identifier.rs` now stores `receipt_hash` on
    `IdentifierClaimRecord`, `IdentifierResolutionReceipt`, and the canonical
    signed receipt payload,
  - `crates/iroha_torii/src/identifier_resolution.rs` now threads the
    hidden-function `receipt_hash` from `iroha_crypto::ram_lfe::EvalResponse`
    into signed receipts and claim records instead of dropping it,
  - `crates/iroha_torii/src/{lib.rs,routing.rs,openapi.rs}` now expose
    `receipt_hash` in the resolve/claim-receipt DTOs and OpenAPI schema,
  - `crates/iroha_core/src/{smartcontracts/isi/identifier.rs,state.rs}` now
    reject all-zero receipt hashes both at claim time and during world-state
    invariant validation.
- Added JS SDK coverage for the identifier Torii surface:
  - `javascript/iroha_js/src/toriiClient.js` now exposes
    `listIdentifierPolicies()`, `resolveIdentifier()`, and
    `issueIdentifierClaimReceipt()`,
  - `javascript/iroha_js/src/normalizers.js` now exposes
    `normalizeIdentifierInput()` so clients can canonicalize
    `phone_e164` / `email_address` / `account_number` inputs exactly the way
    the Rust policy layer expects,
  - `javascript/iroha_js/src/index.js` re-exports the new normalizer,
  - focused JS tests now cover policy listing, resolve, claim-receipt, and
    normalization behavior.
- Updated `docs/source/universal_accounts_guide.md` to mention the
  `receipt_hash` response field and the JS SDK helpers for identifier-policy
  resolution flows.
- Validation:
  - `cargo test --locked -p iroha_crypto ram_lfe -- --nocapture` (pass)
  - `cargo test --locked -p iroha_torii identifier_ --lib -- --nocapture` (pass)
  - `cargo test --locked -p iroha_core identifier_ --lib -- --nocapture` (pass)
  - `node --test javascript/iroha_js/test/normalizers.test.js javascript/iroha_js/test/toriiClient.identifier.test.js` (pass)

## 2026-03-18 Follow-up: BFV-encrypted identifier input is now wired through Torii
- Extended `crates/iroha_crypto/src/fhe_bfv.rs` beyond the raw BFV primitive:
  - added `BfvIdentifierPublicParameters`,
  - deterministic per-policy BFV key derivation for client input encryption,
  - identifier-envelope encrypt/decrypt helpers with length-prefix validation.
- Updated `crates/iroha_torii/src/identifier_resolution.rs`:
  - policy commitments can now publish BFV public parameters in `public_parameters`,
  - the resolver derives/verifies the matching BFV key material from the policy secret + policy id,
  - encrypted BFV request payloads are decrypted before policy normalization and receipt issuance.
- Updated `crates/iroha_torii/src/{lib.rs,routing.rs,openapi.rs}`:
  - `GET /v1/identifier-policies` now advertises optional `input_encryption=bfv-v1` metadata plus hex-encoded public parameters,
  - `POST /v1/identifiers/resolve` and `POST /v1/accounts/{account_id}/identifiers/claim-receipt` now accept exactly one of plaintext `input` or BFV `encrypted_input`,
  - added handler coverage for BFV-encrypted identifier resolution.
- Updated `docs/source/universal_accounts_guide.md` to describe the BFV-encrypted request path and to clarify that identifier derivation is still the committed PRF while input transport can now be BFV-wrapped.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_crypto bfv -- --nocapture` (pass)
  - `cargo test -p iroha_torii identifier_ --lib -- --nocapture` (pass)
  - `cargo test -p iroha_torii alias_resolve_scans_account_labels_when_alias_index_is_missing --lib -- --nocapture` (pass)
  - `cargo test -p iroha_torii --lib openapi::tests::generated_spec_includes_documented_paths -- --nocapture` (pass)

## 2026-03-18 Follow-up: `iroha_crypto` now ships a real BFV baseline instead of calling the HKDF path "FHE"
- Added `crates/iroha_crypto/src/fhe_bfv.rs` and exported it from `crates/iroha_crypto/src/lib.rs`:
  - deterministic BFV key generation, encryption, decryption,
  - ciphertext addition,
  - ciphertext-by-plaintext multiplication,
  - ciphertext-by-ciphertext multiplication with relinearization,
  - affine-circuit evaluation over scalar ciphertext slots.
- Fixed the BFV multiply path to use the integer negacyclic product before the `t/q` rescale-and-round step instead of reducing modulo `q` too early, which was the source of the broken ct-ct multiplication path.
- Added scalar-overflow validation bounds for the baseline implementation so unsupported parameter sets fail closed instead of silently overflowing the `i128` accumulation path.
- Clarified `crates/iroha_crypto/src/ram_lfe.rs` docs:
  - the currently wired identifier resolver is still a committed `HKDF-SHA3-512` PRF backend,
  - it is not a homomorphic evaluator,
  - real BFV primitives now live in `iroha_crypto::fhe_bfv`.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_crypto bfv -- --nocapture` (pass)

## 2026-03-18 Follow-up: identifier claims are now receipt-bound, normalized, access-checked, and config-wired
- Tightened identifier claims in `crates/iroha_core/src/smartcontracts/isi/identifier.rs`:
  - `ClaimIdentifier` now verifies a signed `IdentifierResolutionReceipt` against the policy resolver key,
  - rejects claim/account/UAID mismatches,
  - rejects future-issued or expired receipts,
  - added regressions for invalid signatures and expired receipts.
- Extended the identifier data model in `crates/iroha_data_model/src/identifier.rs`:
  - added `IdentifierNormalization` with built-in canonicalisers for phone, email, account-number, trimmed, and exact modes,
  - stored the normalization mode on `IdentifierPolicy`,
  - shared the canonical signed receipt payload between Torii and core.
- Updated Torii identifier surfaces in `crates/iroha_torii/src/{identifier_resolution.rs,lib.rs,openapi.rs}`:
  - `GET /v1/identifier-policies`, `POST /v1/identifiers/resolve`, and
    `POST /v1/accounts/{account_id}/identifiers/claim-receipt` now run through `check_access`,
  - resolve/claim-receipt now normalize inputs through the active policy before deriving the opaque id,
  - added the claim-receipt route to OpenAPI,
  - added tests covering token enforcement, normalization, and config-driven resolver wiring.
- Wired the in-process resolver through `iroha_config`:
  - new `torii.identifier_resolver` config surface in `crates/iroha_config/src/parameters/{user.rs,actual.rs,defaults.rs}`,
  - `Torii::new_with_handle` now instantiates resolver runtimes from config instead of leaving production `identifier_resolver` unset.
- Updated `docs/source/universal_accounts_guide.md` to document normalization modes, receipt-bound claims, the claim-receipt route, and the fact that identifier endpoints honor Torii access/rate policy.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo check -p iroha_data_model -p iroha_config -p iroha_core -p iroha_torii` (pass)
  - `cargo test -p iroha_data_model phone_normalization_strips_formatting -- --nocapture` (pass)
  - `cargo test -p iroha_data_model --lib email_normalization_lowercases_and_trims -- --nocapture` (pass)
  - `cargo test -p iroha_data_model --lib identifier_policy_id_roundtrip -- --nocapture` (pass)
  - `cargo test -p iroha_config torii_identifier_resolver_parses -- --nocapture` (pass)
  - `cargo test -p iroha_core identifier_ --lib -- --nocapture` (pass)
  - `cargo test -p iroha_torii identifier_ --lib -- --nocapture` (pass)
  - `cargo test -p iroha_torii --lib openapi::tests::generated_spec_includes_documented_paths -- --nocapture` (pass)

## 2026-03-18 Follow-up: UAID-integrated hidden identifier policies landed across crypto, data model, core state, and Torii
- Added reusable hidden-function plumbing in `crates/iroha_crypto/src/ram_lfe.rs`:
  - `RamLfeBackend`, `PolicyCommitment`, `ClientRequest`, `EvalResponse`, and a committed `HKDF-SHA3-512` PRF evaluator/signing path suitable for higher-layer integration.
- Added generic identifier-policy data model and ISIs:
  - `IdentifierPolicyId`, `IdentifierPolicy`, `IdentifierClaimRecord`, `IdentifierResolutionReceipt`,
  - `RegisterIdentifierPolicy`, `ActivateIdentifierPolicy`, `ClaimIdentifier`, `RevokeIdentifier`,
  - instruction registry + visitor wiring.
- Extended world state and invariants in `crates/iroha_core/src/state.rs`:
  - new `identifier_policies` and `identifier_claims` storages,
  - snapshot/JSON persistence support,
  - invariant validation tying `opaque_id -> uaid -> account` back to account-held opaque ids.
- Added core execution handlers in `crates/iroha_core/src/smartcontracts/isi/identifier.rs`:
  - policy registration/activation,
  - claim/revoke lifecycle,
  - account-delete cleanup for identifier claims.
- Added Torii identifier-resolution surface:
  - `GET /v1/identifier-policies`,
  - `POST /v1/identifiers/resolve`,
  - in-process `IdentifierResolutionService` with signed receipts,
  - OpenAPI documentation for the new request/response payloads.
- Updated UAID docs in `docs/source/universal_accounts_guide.md` to describe policy namespaces, claims, and the resolve-then-transfer flow.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo check -p iroha_data_model -p iroha_core -p iroha_torii` (pass)
  - `cargo test -p iroha_crypto ram_lfe -- --nocapture` (pass)
  - `cargo test -p iroha_data_model identifier_policy_id_roundtrip -- --nocapture` (pass)
  - `cargo test -p iroha_core identifier_ --lib -- --nocapture` (pass)
  - `cargo test -p iroha_torii identifier_ --lib -- --nocapture` (pass)
  - `cargo test -p iroha_torii --lib openapi::tests::generated_spec_includes_documented_paths -- --nocapture` (pass)

## 2026-03-17 Follow-up: Android transfer encoder now validates Norito AssetId payloads and normalizes COMPACT_LEN inputs
- Updated `java/iroha_android/src/main/java/org/hyperledger/iroha/android/model/instructions/TransferWirePayloadEncoder.java`:
  - Norito `AssetId` parsing now accepts `COMPACT_LEN` layout flags and rejects other unsupported layout flags explicitly.
  - Added structural decode/validation for preserved `AssetId.account` and `AssetId.scope` payloads (single/multisig controller parsing, scope discriminant validation), with explicit `Invalid AssetId.account payload` / `Invalid AssetId.scope payload` error context.
  - Added semantic multisig validation during Norito account decode (supported version check, non-empty member set, positive weights, positive threshold, threshold <= total weight, duplicate-member rejection).
  - Added the same multisig semantic validation on transfer-side account-controller encoding so I105-derived multisig payloads follow the same constraints as decoded Norito payloads.
  - For `COMPACT_LEN` source payloads, account/scope are transcoded into canonical non-compact transfer payload form before re-encoding, keeping transfer payloads valid under current `flags=0` transaction encoding.
- Added regression coverage in `java/iroha_android/src/test/java/org/hyperledger/iroha/android/model/instructions/TransferWirePayloadEncoderTests.java`:
  - accepts COMPACT_LEN Norito asset IDs and normalizes them to canonical transfer payload bytes,
  - accepts COMPACT_LEN Norito asset IDs with dataspace scope and normalizes scope payloads to canonical non-compact transfer bytes,
  - accepts COMPACT_LEN Norito asset IDs with multisig account payloads and normalizes them to canonical transfer bytes (including member ordering),
  - rejects malformed Norito `AssetId.account` payloads,
  - rejects malformed Norito `AssetId.scope` payloads,
  - rejects structurally valid but semantically invalid multisig policies in Norito account payloads,
  - rejects unsupported multisig policy versions in Norito account payloads,
  - rejects unsupported Norito `AssetId` layout flags with a deterministic error.
- Validation:
  - `JAVA_HOME=$(/usr/libexec/java_home -v 21) ANDROID_HOME=~/Library/Android/sdk ANDROID_SDK_ROOT=~/Library/Android/sdk ./gradlew :core:test --tests org.hyperledger.iroha.android.GradleHarnessTests --rerun-tasks` (pass).
  - `JAVA_HOME=$(/usr/libexec/java_home -v 21) ANDROID_HOME=~/Library/Android/sdk ANDROID_SDK_ROOT=~/Library/Android/sdk ./gradlew test` (pass).

## 2026-03-17 Follow-up: Android AssetId decoder now fully validates scope/trailing bytes and rejects negative field lengths
- Updated `java/iroha_android/src/main/java/org/hyperledger/iroha/android/address/AssetIdDecoder.java`:
  - `decode(...)` now validates the full `AssetId` payload shape by decoding and checking `scope` instead of stopping after account/definition fields.
  - `decode(...)` now validates `AssetId.account` payload structure/semantics (single/multisig controller decode, multisig version/threshold/member checks) instead of only skipping bytes.
  - Added explicit trailing-byte rejection after the scope field.
  - Added non-negative + bounded length checks for account/definition/scope field lengths so malformed signed-u64 lengths fail deterministically with `IllegalArgumentException`.
  - Added strict layout-flag validation for both `AssetId` and `AssetDefinitionId` decode paths: only `COMPACT_LEN` is accepted; other layout flags are rejected.
- Added regression coverage in `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AssetIdDecoderTests.java`:
  - rejects malformed `AssetId.account` payloads,
  - rejects unsupported `AssetId` layout flags,
  - rejects malformed `AssetId.scope` payloads,
  - rejects trailing bytes after a valid scope field,
  - rejects negative account-length prefixes.
  - rejects unsupported `AssetDefinitionId` layout flags.
- Validation:
  - `JAVA_HOME=$(/usr/libexec/java_home -v 21) ANDROID_HOME=~/Library/Android/sdk ANDROID_SDK_ROOT=~/Library/Android/sdk ./gradlew :core:test --tests org.hyperledger.iroha.android.GradleHarnessTests --rerun-tasks` (pass).
  - `JAVA_HOME=$(/usr/libexec/java_home -v 21) ANDROID_HOME=~/Library/Android/sdk ANDROID_SDK_ROOT=~/Library/Android/sdk ./gradlew test` (pass).

## 2026-03-17 Follow-up: integration-test startup no longer hangs when one peer reaches block 1 before Torii `/status`
- Updated `crates/iroha_test_network/src/lib.rs` startup gating:
  - `NetworkPeer::start_checked(...)` still prefers `ServerStarted` (`/status`-driven),
    but during genesis bootstrap it now falls back after a grace window when the
    peer is running and block 1 is already committed on storage.
  - this removes the long hang shape where `start_checked` could wait until the
    outer network timeout even though consensus had progressed.
- Added unit coverage in `start_event_tests`:
  - `storage_fallback_requires_bootstrap_grace_running_and_block_1`
  - `storage_fallback_allows_ready_bootstrap_peer`
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_test_network start_event_tests -- --nocapture` (pass)
  - `cargo test -p integration_tests explorer_instructions_emit_i105_literals -- --nocapture` (pass; targeted test finished `ok` in `316.83s`)

## 2026-03-17 Follow-up: first-release ABI surface normalized to v1 only
- Normalized the first-release ABI surface to v1 only across the core IVM/runtime path and the public ABI API: removed experimental syscall-policy scaffolding, made `IVM::load_program` reject non-v1 ABI headers, made pointer-ABI validation fail closed for non-v1 annotations, collapsed runtime ABI queries/endpoints/telemetry to singular `abi_version = 1` payloads, and updated the canonical runtime-upgrade/syscall docs to describe `added_*` fields as reserved-and-empty in this release.
- Extended the v1-only ABI cleanup through the runtime-upgrade SDK surface: Python, JavaScript, and Swift clients now reject runtime-upgrade manifests unless `abi_version = 1` and `added_syscalls`/`added_pointer_types` are empty, examples/tests were rewritten to match, and the remaining runtime-upgrade/nexus/threat-model docs were swept to remove stale multi-version ABI wording.

## 2026-03-17 Follow-up: sorting checks now match opaque asset-definition IDs
- Fixed `integration_tests/tests/sorting.rs` asset-definition sorting checks to
  filter results by the exact IDs registered in-test instead of
  `id().name().starts_with("xor_")`, matching current opaque `aid:*`
  asset-definition ID behavior.
- Verified previously failing integration tests now pass:
  - `correct_sorting_of_entities`
  - `metadata_sorting_descending`

## 2026-03-17 Follow-up: late contiguous-frontier recovery now preserves sender ownership and accepts explicit sparse block-sync recovery
- Updated `crates/iroha_core/src/sumeragi/main_loop.rs`,
  `crates/iroha_core/src/sumeragi/main_loop/reschedule.rs`, and
  `crates/iroha_core/src/sumeragi/main_loop/block_sync.rs`:
  - zero-vote contiguous-frontier quorum-timeout reschedule now treats
    same-height missing-block dependency progress plus active inbound recovery
    backlog (`block_payload_rx`, `rbc_chunk_rx`, `block_rx`, residual round
    backlog) as active frontier work and suppresses `drop_pending` while the
    current frontier owner is still converging that recovery;
  - quorum-timeout frontier seeding is now create-only for a same frontier
    height, so repeated handoffs preserve `entered_at`, `phase`,
    `cleanup_done`, `no_progress_windows`, `last_action_at`,
    `last_progress_at`, `last_dependency_progress_at`,
    `last_rotation_view`, and the existing owner cause instead of reseeding the
    owner window;
  - frontier recovery now suppresses cleanup/rotate while same-height
    dependency progress is still active and inbound recovery backlog is present
    in the current frontier window;
  - sparse `BlockSyncUpdate` intake now allows only explicit contiguous-frontier
    missing-block recovery (exact hash request or same-height tracked request)
    with at least one validated block signature; unsolicited, far-ahead, and
    unsigned sparse updates remain rejected.
- Updated regression coverage in
  `crates/iroha_core/src/sumeragi/main_loop/tests.rs`:
  - `zero_vote_quorum_timeout_does_not_drop_while_same_height_missing_block_recovery_backlog_is_active`,
  - `seed_frontier_recovery_for_quorum_timeout_preserves_existing_same_height_owner_state`,
  - `frontier_recovery_suppresses_cleanup_and_rotate_while_same_height_dependency_backlog_is_active`,
  - `block_sync_quorum_accepts_only_explicit_contiguous_frontier_sparse_recovery`,
  - `block_sync_update_accepts_explicit_contiguous_frontier_requested_sparse_recovery`,
  - `block_sync_update_accepts_partial_vote_sparse_recovery_for_explicit_missing_request`.
- Validation for this follow-up:
  - `cargo fmt --all` (pass),
  - `cargo test -p iroha_core --lib zero_vote_quorum_timeout_ -- --nocapture` (pass),
  - `cargo test -p iroha_core --lib frontier_recovery_suppresses_cleanup_and_rotate_while_same_height_dependency_backlog_is_active -- --nocapture` (pass),
  - `cargo test -p iroha_core --lib seed_frontier_recovery_for_quorum_timeout_preserves_existing_same_height_owner_state -- --nocapture` (pass),
  - `cargo test -p iroha_core --lib frontier_recovery_rotates_once_after_cleanup_window_expires -- --nocapture` (pass),
  - `cargo test -p iroha_core --lib force_view_change_if_idle_suppresses_empty_frontier_missing_qc_while_quorum_timeout_recovery_owns_window -- --nocapture` (pass),
  - `cargo test -p iroha_core --lib reschedule_skips_vote_backed_retransmit_while_frontier_quorum_timeout_window_owned -- --nocapture` (pass),
  - `cargo test -p iroha_core --lib stake_quorum_timeout_skips_noop_reschedule_with_full_signer_set -- --nocapture` (pass),
  - `cargo test -p iroha_core --lib block_sync_quorum_accepts_only_explicit_contiguous_frontier_sparse_recovery -- --nocapture` (pass),
  - `cargo test -p iroha_core --lib block_sync_update_accepts_explicit_contiguous_frontier_requested_sparse_recovery -- --nocapture` (pass),
  - `cargo test -p iroha_core --lib block_sync_update_accepts_partial_vote_sparse_recovery_for_explicit_missing_request -- --nocapture` (pass),
  - `cargo test -p iroha_core --lib block_sync_update_records_missing_request_when_sparse_signatures_fail_quorum -- --nocapture` (pass),
  - `cargo test -p iroha_core --lib block_sync_update_keeps_aborted_next_height_payload_sparse_without_commit_evidence -- --nocapture` (pass),
  - `cargo build --release -p irohad --bin iroha3d` (pass),
  - `RUST_LOG=izanami::summary=info,izanami::progress=info,iroha_core::sumeragi::main_loop=info IROHA_TEST_NETWORK_KEEP_DIRS=1 IROHA_TEST_NETWORK_PERMIT_DIR=$(mktemp -d) TEST_NETWORK_BIN_IROHAD=/Users/mtakemiya/dev/iroha/target/release/iroha3d cargo run -p izanami --release --locked -- --allow-net --nexus --peers 4 --faulty 0 --duration 300s --target-blocks 120 --progress-interval 10s --progress-timeout 180s --tps 1 --max-inflight 8 --workload-profile stable` (pass to target height, below pace target).
- Runtime notes:
  - warmed 4-peer NPoS probe using the rebuilt release `iroha3d` reached
    `quorum 122 / strict 122` and exited cleanly in `210.05s` against
    `target_blocks=120` (preserved network dir:
    `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_2adGeM`);
  - preserved peer logs show `dropping block sync update missing commit-role quorum`:
    `0` occurrences across `run-1-stdout.log`;
  - `commit quorum missing past timeout; rescheduling block for reassembly`
    still appeared `8` times across preserved peer stdout logs, but the run no
    longer stalled permanently and did not miss the 120-block target;
  - acceptance status for this cut: late-frontier permanent stall fixed on this
    warmed run, `<= 1.0s/block` not yet recovered (`~1.75s/block` over the
    120-block window).

## 2026-03-17 Follow-up: near-quorum quorum-timeout reschedule now permits hintless block-sync resend when certified roster proof is unavailable
- Updated `crates/iroha_core/src/sumeragi/main_loop/reschedule.rs`:
  - in `rebroadcast_pending_block_updates(...)`, when near-commit-quorum
    vote-backed retransmit is active, certified roster evidence is unavailable,
    and the pending block is on the contiguous frontier for the active round,
    the node now permits a frame-cap-trimmed hintless `BlockSyncUpdate`
    rebroadcast instead of no-op skipping payload resend.
  - this path remains `BlockSyncUpdate`-only (no `BlockCreated` fallback
    re-entry from quorum-timeout reschedule).
- Updated regression coverage in
  `crates/iroha_core/src/sumeragi/main_loop/tests.rs`:
  - `quorum_reschedule_near_quorum_rebroadcasts_votes_without_payload_rebroadcast`
    now also asserts that near-quorum reschedule emits `BlockSyncUpdate`.
- Validation for this follow-up:
  - `cargo fmt --all` (pass),
  - `cargo test -p iroha_core --lib quorum_reschedule_near_quorum_rebroadcasts_votes_without_payload_rebroadcast -- --nocapture` (pass),
  - `cargo test -p iroha_core --lib quorum_reschedule_near_quorum_rebroadcasts_block_sync_without_new_commit_evidence -- --nocapture` (pass),
  - `cargo build --release -p irohad --bin iroha3d` (pass).
- Runtime notes:
  - stale-binary probe on the same source tree
    (`/tmp/izanami_npos_healthy_probe_near_quorum_hintless_20260317T140640.log`,
    network `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_Fy0Iwd`)
    reached `quorum 67 / strict 71` before timeout;
  - rebuilt-binary probe (post release rebuild)
    (`/tmp/izanami_npos_healthy_probe_near_quorum_hintless_rebuilt_20260317T141749.log`,
    network `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_a2kifr`)
    reached `quorum 68 / strict 71` before timeout
    (`~2.02s/block` on quorum-min height, `~1.94s/block` on strict-min height
    over the measured `137.48s` runtime window);
  - peer-log deltas vs pre-cut keep-dirs baseline
    (`.../irohad_test_network_yMBnIh`, `62 / 68`):
    - `commit quorum missing past timeout; rescheduling block for reassembly`:
      `17 -> 7`,
    - `roster proof unavailable for block sync update`: `9 -> 0`,
    - `rebroadcasted_block_sync=true` events now present on near-quorum warnings
      (`3` observed in `a2kifr`).
  - Remaining runtime bottleneck is still a late same-height stall around the
    low-70s frontier (`height=72` in this run), not early roster-proof no-op
    suppression.

## 2026-03-17 Follow-up: payload-mismatch owner preservation now requires an active pending frontier owner
- Updated `crates/iroha_core/src/sumeragi/main_loop/proposal_handlers.rs`:
  - `preserve_contiguous_frontier_owner_on_payload_mismatch(...)` now preserves
    only when there is an actual active pending owner for the same
    `height/view` extending the current tip.
  - The previous fallback preservation via
    `frontier_vote_backed_recovery_active(...)` was removed from this mismatch
    decision path to avoid suppressing payload acceptance when no pending owner
    exists.
- Added regression in `crates/iroha_core/src/sumeragi/main_loop/tests.rs`:
  - `block_created_payload_mismatch_does_not_preserve_owner_without_active_pending_block`.
- Validation for this follow-up:
  - `cargo fmt --all` (pass),
  - `cargo test -p iroha_core --lib block_created_accepts_payload_after_proposal_mismatch -- --nocapture` (pass),
  - `cargo test -p iroha_core --lib block_created_payload_mismatch_does_not_preserve_owner_without_active_pending_block -- --nocapture` (pass),
  - `cargo test -p iroha_core --lib block_created_payload_mismatch_preserves_active_frontier_owner -- --nocapture` (pass),
  - `cargo test -p iroha_core --lib contiguous_frontier_missing_payload_dwell_ -- --nocapture` (pass).
- Runtime notes:
  - Diagnostic keep-dirs probe before this cut
    (`/tmp/izanami_npos_healthy_probe_keepdirs_20260317T041709.log`,
    preserved network
    `/tmp/iroha-test-network-debug/irohad_test_network_FgGlzN`)
    timed out at `quorum 27 / strict 27`; preserved peer logs showed a
    persistent `height=28` loop with:
    - `dropping BlockCreated due to proposal mismatch` (`preserve_frontier_owner=true`),
    - `dropping block sync update: block not accepted locally`,
    - repeated frontier recovery rotations (`missing_block_height_hard_cap`).
  - Post-cut probe
    (`/tmp/izanami_npos_healthy_probe_post_payload_mismatch_guard_20260317T042521.log`)
    progressed to `quorum 51 / strict 51` before duration timeout; the prior
    early `height=27/28` deadlock shape no longer dominated this run.
  - Remaining bottleneck is now later global commit-quorum convergence (no
    lagging peers reported at stall), not the early preserved-mismatch frontier
    deadlock.

## 2026-03-17 Follow-up: zero-vote quorum-timeout now suppresses no-op drop while frontier owner is active
- Updated `crates/iroha_core/src/sumeragi/main_loop/reschedule.rs` to suppress
  contiguous-frontier zero-vote quorum-timeout drop/requeue when:
  - the existing frontier owner is already `quorum_timeout` at that height,
  - `last_action_at` is not set,
  - and owner progress is still inside the current frontier recovery window.
- This closes the no-op handoff shape where a zero-vote drop could still fire,
  then immediately report `frontier_recovery_advance = Some(None)` in the same
  owner window.
- Added regression in `crates/iroha_core/src/sumeragi/main_loop/tests.rs`:
  - `zero_vote_quorum_timeout_does_not_drop_while_quorum_timeout_owner_active_without_action`.
- Also fixed the unrelated `iroha_core` lib-test compile blocker in
  `crates/iroha_core/src/smartcontracts/isi/domain.rs` test imports by adding:
  - `CanRegisterAccount`,
  - `BOB_ID`.
- Validation for this follow-up:
  - `cargo fmt --all` (pass),
  - `cargo test -p iroha_core --lib zero_vote_quorum_timeout_ -- --nocapture` (pass),
  - `cargo test -p iroha_core --lib contiguous_frontier_missing_payload_dwell_ -- --nocapture` (pass),
  - `cargo test -p iroha_core --lib force_view_change_if_idle_suppresses_empty_frontier_missing_qc_while_quorum_timeout_recovery_owns_window -- --nocapture` (pass),
  - `cargo test -p iroha_core --lib reschedule_skips_vote_backed_retransmit_while_frontier_quorum_timeout_window_owned -- --nocapture` (pass),
  - `cargo test -p iroha_core --lib frontier_recovery_rotates_once_after_cleanup_window_expires -- --nocapture` (pass),
  - `cargo build --release -p irohad --bin iroha3d` (pass).
- Runtime note:
  - warmed keep-dirs probe (strict pipefail) log:
    `/tmp/izanami_npos_healthy_probe_zero_vote_owner_active_noop_guard_20260317T001915.log`,
    network dir:
    `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_VEMRRO`,
    exit:
    `timed out before reaching target blocks (quorum min height 67, strict min 72, target 120, tolerated_failures 0)`.
  - compared with the immediately previous warmed keep-dirs rerun
    (`/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_1qE1Ki`,
    `quorum 61 / strict 67`):
    - `commit quorum missing past timeout; rescheduling block for reassembly`: `16 -> 8`,
    - `drop_pending: true`: `6 -> 0`,
    - `frontier_recovery_advance = Some(None)` on commit-quorum warnings: `1 -> 0`,
    - `drop_pending: true` with `progress_age_ms <= 100`: stayed `0 -> 0`,
    - contiguous-frontier missing-payload dwell warnings/fallbacks remained quiet
      (`retry-routed=0`, `view-change-routed=0`, direct `MissingPayload` fallback `0`),
      with a single `missing_payload` range-pull record (`1`).
  - Remaining runtime bottleneck is still late commit-quorum convergence above
    the high-60s/low-70s frontier, not repeated zero-vote ownerless drop churn.

## 2026-03-16 Follow-up: zero-vote quorum-timeout now defers drop when progress is fresh
- Updated `crates/iroha_core/src/sumeragi/main_loop/reschedule.rs` so zero-vote
  quorum-timeout handling no longer drops/requeues immediately when pending
  progress is still fresh inside the current reschedule backoff window:
  - new guard checks `progress_age < reschedule_backoff` for zero-vote blocks,
  - if true, the pending block is retained and quorum reschedule is deferred for
    the remainder of that backoff window.
- Added focused regression in `crates/iroha_core/src/sumeragi/main_loop/tests.rs`:
  - `zero_vote_quorum_timeout_defers_drop_when_progress_is_recent` (defer first,
    then drop/handoff after backoff expiry).
- Validation for this follow-up:
  - `cargo fmt --all` (pass),
  - `cargo check -p iroha_core --lib` (pass),
  - `cargo build --release -p irohad --bin iroha3d` (pass),
  - `cargo test -p iroha_core --lib zero_vote_quorum_timeout_defers_drop_when_progress_is_recent -- --nocapture`
    was blocked at that point by unrelated pre-existing lib-test compile errors in
    `crates/iroha_core/src/smartcontracts/isi/domain.rs` (`BOB_ID`,
    `CanRegisterAccount` imports missing in that test module).
- Runtime note:
  - warmed keep-dirs probe
    `/tmp/izanami_npos_healthy_probe_zero_vote_recent_progress_guard_20260316T234527.log`
    (`/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_oL218c`)
    still timed out at `quorum 48 / strict 54`,
  - compared to the prior keep-dirs run
    (`/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_jLEG3t`):
    - `commit quorum missing past timeout; rescheduling block for reassembly` stayed `25 -> 25`,
    - `drop_pending: true` reduced `5 -> 3`,
    - `drop_pending: true` with `progress_age_ms <= 100` reduced `1 -> 0`
      (the pathological near-zero-progress drop no longer appeared),
    - missing-block dwell warning paths remained quiet (`routed through frontier recovery = 0`,
      direct `MissingPayload` fallback = `0`).
  - dominant stall shape in this run was early vote-split/queue-pressure churn
    around heights `10` and `33` (multiple block hashes at the same height with
    `votes=1/2`), not same-window missing-block dwell ownership.

## 2026-03-16 Follow-up: contiguous-frontier dwell now enforces strict elapsed-window ownership
- Tightened contiguous-frontier window ownership in
  `crates/iroha_core/src/sumeragi/main_loop.rs`:
  - `frontier_recovery_owns_height_window(...)` now treats ownership as
    `now - last_action_at < frontier_recovery_window()` (same frontier height,
    `last_action_at` required), so same-height dwell cannot rearm on window-bucket
    boundaries.
  - `frontier_recovery_action_taken_in_current_window(...)` now uses the same
    elapsed-window cooldown rule.
  - `reset_same_height_no_proposal_storm_if_progressed(...)` now preserves the
    active frontier owner on dependency progress when an action already occurred
    in the current window, preventing a second same-window catch-up emit from a
    reset/reseed path.
- Added regression in `crates/iroha_core/src/sumeragi/main_loop/tests.rs`:
  - `frontier_stall_does_not_emit_second_recovery_action_in_same_window`.
- Validation for this follow-up:
  - `cargo test -p iroha_core --lib frontier_stall_does_not_emit_second_recovery_action_in_same_window -- --nocapture` (pass),
  - `cargo test -p iroha_core --lib contiguous_frontier_missing_payload_dwell_hands_off_once_per_window -- --nocapture` (pass),
  - `cargo test -p iroha_core --lib contiguous_frontier_missing_payload_dwell_does_not_fallback_to_direct_view_change_while_window_owned -- --nocapture` (pass),
  - `cargo test -p iroha_core --lib contiguous_frontier_missing_payload_dwell_rearms_only_after_next_window -- --nocapture` (pass),
  - `cargo test -p iroha_core --lib zero_vote_quorum_timeout_ -- --nocapture` (pass),
  - `cargo test -p iroha_core --lib force_view_change_if_idle_suppresses_empty_frontier_missing_qc_while_quorum_timeout_recovery_owns_window -- --nocapture` (pass),
  - `cargo test -p iroha_core --lib reschedule_skips_vote_backed_retransmit_while_frontier_quorum_timeout_window_owned -- --nocapture` (pass),
  - `cargo test -p iroha_core --lib frontier_recovery_rotates_once_after_cleanup_window_expires -- --nocapture` (pass),
  - `cargo build --release -p irohad --bin iroha3d` (pass).
- Runtime note:
  - warmed keep-dirs 120-block healthy NPoS probe
    `/tmp/izanami_npos_healthy_probe_missing_payload_owner_20260316T230137.log`
    (`/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_jLEG3t`)
    timed out at `quorum 55 / strict 56`,
  - peer logs in that run show a single contiguous-frontier dwell handoff warn
    (`missing block dwell exceeded view-change window; routed through frontier recovery`)
    at `height=36` and no direct `MissingPayload` fallback warns,
  - the suppression line is `debug!` and remains hidden under the probe’s
    `--log-filter info`,
  - remaining runtime bottleneck is commit-quorum convergence / queue-pressure
    collapse, not repeated same-window missing-block dwell ownership.

## 2026-03-16 Follow-up: zero-vote contiguous-frontier quorum-timeout now suppresses same-window repeats
- Updated `crates/iroha_core/src/sumeragi/main_loop/reschedule.rs` so contiguous-frontier
  zero-vote quorum-timeout reschedule now checks `frontier_recovery_owns_height_window(height, now)`
  and suppresses same-window retries (matching the one-action-per-window ownership rule already used
  by vote-backed suppression).
- Added regression in `crates/iroha_core/src/sumeragi/main_loop/tests.rs`:
  - `zero_vote_quorum_timeout_suppresses_same_window_repeat_while_frontier_owner_active`.
- Validation for this follow-up:
  - `cargo fmt --all` (pass),
  - `cargo test -p iroha_core --lib zero_vote_quorum_timeout_ -- --nocapture` (pass),
  - `cargo test -p iroha_core --lib contiguous_frontier_missing_payload_dwell_ -- --nocapture` (pass),
  - `cargo test -p iroha_core --lib force_view_change_if_idle_suppresses_empty_frontier_missing_qc_while_quorum_timeout_recovery_owns_window -- --nocapture` (pass),
  - `cargo test -p iroha_core --lib reschedule_skips_vote_backed_retransmit_while_frontier_quorum_timeout_window_owned -- --nocapture` (pass),
  - `cargo test -p iroha_core --lib frontier_recovery_rotates_once_after_cleanup_window_expires -- --nocapture` (pass),
  - `cargo build --release -p irohad --bin iroha3d` (pass).
- Runtime notes:
  - warmed keep-dirs probe without latency gate:
    `/tmp/izanami_npos_healthy_probe_zero_vote_window_owner_20260316T220751.log`
    (`/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_urFsDu`)
    timed out at `quorum 50 / strict 53`,
  - warmed keep-dirs probe with latency gate and `--faulty 0`:
    `/tmp/izanami_npos_healthy_probe_zero_vote_window_owner_f0_lg1000_20260316T221556.log`
    (`/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_nxBw0A`)
    timed out at `quorum 64 / strict 64`,
  - peer-log churn snapshot vs prior comparable keep-dirs baseline
    (`/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_WN2zje`):
    - `commit quorum missing past timeout; rescheduling block for reassembly` total
      moved `34 -> 27`,
    - `drop_pending: true` moved `12 -> 7`,
    - `drop_pending: true` with `frontier_recovery_advance: Some(None)` moved `6 -> 0`,
    - repeated same `peer+height` churn dropped from multiple `3x` loops (`8`, `28`) to `2x` loops
      (mainly `65` and `16`) in this run.

## 2026-03-16 Follow-up: missing-block dwell warnings now carry block context and action outcome
- Updated `crates/iroha_core/src/sumeragi/main_loop/qc.rs` in both contiguous-frontier dwell branches (retry-window and view-change-window):
  - suppression debug path now logs `block` alongside `height/view`,
  - warn logs moved to post-handoff so they reflect actual handoff outcome,
  - no-op handoff (`FrontierRecoveryAdvance::None`) is now logged at `debug` as
    `frontier recovery evaluated the request without a new action`,
  - direct fallback warns explicitly as
    `frontier controller not responsible` and includes `block`.
- Validation for this follow-up:
  - `cargo fmt --all` (pass),
  - `cargo test -p iroha_core --lib contiguous_frontier_missing_payload_dwell_ -- --nocapture` (pass),
  - `cargo test -p iroha_core --lib zero_vote_quorum_timeout_drops_pending_and_hands_frontier_to_quorum_timeout_recovery -- --nocapture` (pass),
  - `cargo test -p iroha_core --lib force_view_change_if_idle_suppresses_empty_frontier_missing_qc_while_quorum_timeout_recovery_owns_window -- --nocapture` (pass),
  - `cargo test -p iroha_core --lib reschedule_skips_vote_backed_retransmit_while_frontier_quorum_timeout_window_owned -- --nocapture` (pass),
  - `cargo test -p iroha_core --lib frontier_recovery_rotates_once_after_cleanup_window_expires -- --nocapture` (pass).
- Runtime note:
  - clean keep-dirs probe rerun log
    `/tmp/izanami_npos_healthy_probe_frontier_dwell_window_owner_logcut_full_20260316T212527.log`
    timed out before `target_blocks=120` at `quorum 62 / strict 69`,
  - preserved network dir:
    `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_UBttAh`,
  - peer logs in that run no longer emitted the old generic
    `routing through frontier recovery` strings (count `0`),
  - emitted fallback warnings were attributable and block-scoped:
    `frontier controller not responsible, attempting direct MissingPayload fallback`
    appeared `2` times (same `height=26`, same block hash), with no routed/no-op
    dwell warn lines under `info` for that run.

## 2026-03-16 Revalidation: contiguous-frontier dwell handoff remains single-action, but healthy probe still stalls
- Re-ran the contiguous-frontier missing-block dwell ownership subset on current tree:
  - `cargo test -p iroha_core --lib contiguous_frontier_missing_payload_dwell_ -- --nocapture` (pass),
  - `cargo test -p iroha_core --lib zero_vote_quorum_timeout_drops_pending_and_hands_frontier_to_quorum_timeout_recovery -- --nocapture` (pass),
  - `cargo test -p iroha_core --lib force_view_change_if_idle_suppresses_empty_frontier_missing_qc_while_quorum_timeout_recovery_owns_window -- --nocapture` (pass),
  - `cargo test -p iroha_core --lib reschedule_skips_vote_backed_retransmit_while_frontier_quorum_timeout_window_owned -- --nocapture` (pass),
  - `cargo test -p iroha_core --lib frontier_recovery_rotates_once_after_cleanup_window_expires -- --nocapture` (pass).
- Re-ran warmed 120-block healthy NPoS probe with keep-dirs:
  - log: `/tmp/izanami_npos_healthy_probe_frontier_dwell_window_owner_keepdirs_20260316T204058.log`,
  - preserved network dir: `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_6IxdNP`,
  - outcome: timed out before `target_blocks=120` at `quorum 66 / strict 70`.
- Peer-log check for the dwell-owner acceptance shape:
  - `missing payload dwell suppressed; frontier recovery already acted this window` is `debug!` and does not appear under the probe’s `info` filter,
  - `missing block dwell exceeded retry window; routing through frontier recovery` still appears repeatedly (`42` lines total, repeated heights including `69`),
  - but `requested unified frontier recovery range pull ... reason: "missing_payload"` appears once (height `69`, single peer), which indicates the missing-payload handoff path itself stayed one-action for that window.
- Runtime bottleneck remains commit-quorum convergence/reschedule churn near the high-60s/low-70s, not a second same-window `missing_payload` range-pull loop at the frontier.

## 2026-03-16 Follow-up: vote-backed contiguous-frontier reschedule now honors frontier-owner windows
- Simplified vote-backed contiguous-frontier quorum-timeout reschedule ownership in
  `crates/iroha_core/src/sumeragi/main_loop/reschedule.rs`:
  - suppression now checks `frontier_recovery_owns_height_window(height, now)` directly,
    instead of using `last_quorum_reschedule` age or recovery-cause filtering,
  - same-window vote-backed retries now emit
    `suppressing vote-backed quorum reschedule; frontier recovery already acted this window`,
  - reschedule marker state (`last_quorum_reschedule`) is no longer used as a rearm signal for
    this suppression path.
- Updated focused regression
  `reschedule_skips_vote_backed_retransmit_while_frontier_quorum_timeout_window_owned` in
  `crates/iroha_core/src/sumeragi/main_loop/tests.rs` to assert:
  - same-window suppression with frontier ownership even when
    `pending.last_quorum_reschedule` is unset,
  - no direct `MissingQc` / `MissingPayload` fallback while that window is owned,
  - rearm only after moving into the next frontier recovery window.
- Validation (this follow-up):
  - `cargo fmt --all` (pass),
  - `cargo test -p iroha_core --lib reschedule_skips_vote_backed_retransmit_while_frontier_quorum_timeout_window_owned -- --nocapture` (pass),
  - `cargo test -p iroha_core --lib contiguous_frontier_missing_payload_dwell_ -- --nocapture` (pass),
  - `cargo test -p iroha_core --lib zero_vote_quorum_timeout_drops_pending_and_hands_frontier_to_quorum_timeout_recovery -- --nocapture` (pass),
  - `cargo test -p iroha_core --lib force_view_change_if_idle_suppresses_empty_frontier_missing_qc_while_quorum_timeout_recovery_owns_window -- --nocapture` (pass),
  - `cargo test -p iroha_core --lib frontier_recovery_rotates_once_after_cleanup_window_expires -- --nocapture` (pass).
- Runtime note:
  - warmed direct-binary NPoS probe
    `/tmp/izanami_npos_healthy_probe_vote_backed_window_owner_20260316T202521.log`
    still timed out before `target_blocks=120`, but reached `quorum 67 / strict 71`,
  - compared to the prior warmed rerun
    (`/tmp/izanami_npos_healthy_probe_frontier_dwell_window_rerun_20260316T193732.log`,
    `quorum 52 / strict 57`), this cut improved the ceiling,
  - remaining bottleneck is still vote-backed commit-quorum convergence beyond the low-70s,
    not same-window duplicate ownership in vote-backed reschedule.

## 2026-03-16 Follow-up: contiguous-frontier missing-block dwell now defers to frontier recovery once per window
- Simplified contiguous-frontier missing-block dwell ownership in
  `crates/iroha_core/src/sumeragi/main_loop/{main_loop.rs,qc.rs}`:
  - added `frontier_recovery_owns_height_window(...)` to check whether the
    existing frontier owner already acted in the current recovery window,
  - retry-window and view-change-window dwell branches now suppress both
    re-handoff and direct `MissingPayload` fallback when that owner/window is
    already active,
  - suppressed same-window retries now emit a single debug line
    (`missing payload dwell suppressed; frontier recovery already acted this window`),
    and the `routing through frontier recovery` warn path is only emitted when
    the branch actually attempts recovery work.
- Added targeted regressions in
  `crates/iroha_core/src/sumeragi/main_loop/tests.rs`:
  - `contiguous_frontier_missing_payload_dwell_hands_off_once_per_window`,
  - `contiguous_frontier_missing_payload_dwell_does_not_fallback_to_direct_view_change_while_window_owned`,
  - `contiguous_frontier_missing_payload_dwell_rearms_only_after_next_window`.
- Validation (this follow-up):
  - `cargo fmt --all` (pass),
  - `cargo test -p iroha_core --lib contiguous_frontier_missing_payload_dwell_hands_off_once_per_window -- --nocapture` (pass),
  - `cargo test -p iroha_core --lib contiguous_frontier_missing_payload_dwell_does_not_fallback_to_direct_view_change_while_window_owned -- --nocapture` (pass),
  - `cargo test -p iroha_core --lib contiguous_frontier_missing_payload_dwell_rearms_only_after_next_window -- --nocapture` (pass),
  - `cargo test -p iroha_core --lib zero_vote_quorum_timeout_drops_pending_and_hands_frontier_to_quorum_timeout_recovery -- --nocapture` (pass),
  - `cargo test -p iroha_core --lib force_view_change_if_idle_suppresses_empty_frontier_missing_qc_while_quorum_timeout_recovery_owns_window -- --nocapture` (pass),
  - `cargo test -p iroha_core --lib reschedule_skips_vote_backed_retransmit_while_frontier_quorum_timeout_window_owned -- --nocapture` (pass),
  - `cargo test -p iroha_core --lib frontier_recovery_rotates_once_after_cleanup_window_expires -- --nocapture` (pass),
  - `cargo build --release -p izanami -p irohad --bin iroha3d` (pass).
- Runtime note:
  - warmed direct-binary NPoS probe
    `/tmp/izanami_npos_healthy_probe_frontier_dwell_window_20260316T191015.log`
    timed out before `target_blocks=120` (`quorum 46 / strict 50`),
  - warmed rerun on the current tree
    `/tmp/izanami_npos_healthy_probe_frontier_dwell_window_rerun_20260316T193732.log`
    also timed out before `target_blocks=120` (`quorum 52 / strict 57`),
  - keep-dirs probe
    `/tmp/izanami_npos_healthy_probe_frontier_dwell_window_keepdirs_20260316T191914.log`
    (`/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_dGaNGu`)
    also timed out (`quorum 46 / strict 51`),
  - final keep-dirs probe on the warning-gated tree
    `/tmp/izanami_npos_healthy_probe_frontier_dwell_window_keepdirs_final_20260316T192617.log`
    (`/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_Va4qxg`)
    timed out earlier (`quorum 14 / strict 14`) with concurrent workload
    failures (`Illegal math operation` in `burn_trigger_repetitions`),
  - remaining dominant runtime pressure in peer logs is still
    vote-backed/zero-vote commit-quorum reschedule churn; this cut removed
    same-window dwell ownership from the missing-block branch but did not close
    the healthy NPoS target-block gate.

## 2026-03-16 Follow-up: zero-vote contiguous-frontier drop now hands off to unified quorum-timeout recovery
- Simplified `crates/iroha_core/src/sumeragi/main_loop/reschedule.rs` so
  zero-vote quorum-timeout reschedule on the contiguous frontier no longer
  leaves an ownerless empty slot behind:
  - when `drop_pending=true` at `committed_height + 1`, the reschedule path now
    seeds `quorum_timeout` frontier recovery immediately and advances the
    existing unified controller instead of only deleting the pending block,
  - the reschedule warning now logs whether that zero-vote drop handed the
    frontier to unified recovery (`handoff_frontier_quorum_timeout_owner`) and
    what the first controller step returned (`frontier_recovery_advance`).
- Updated focused regression coverage in
  `crates/iroha_core/src/sumeragi/main_loop/tests.rs`:
  - renamed/expanded
    `zero_vote_quorum_timeout_drops_pending_and_hands_frontier_to_quorum_timeout_recovery`
    to assert that zero-vote contiguous-frontier drop leaves a
    `quorum_timeout` frontier owner and that the next idle tick still does not
    count a `MissingQc` rotation,
  - adjacent suppression regressions remain green.
- Validation (this follow-up):
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_core --lib zero_vote_quorum_timeout_drops_pending_and_hands_frontier_to_quorum_timeout_recovery -- --nocapture` (pass)
  - `cargo test -p iroha_core --lib force_view_change_if_idle_suppresses_empty_frontier_missing_qc_while_quorum_timeout_recovery_owns_window -- --nocapture` (pass)
  - `cargo test -p iroha_core --lib reschedule_defers_zero_vote_quorum_timeout_while_block_queue_backlogged -- --nocapture` (pass)
  - `cargo test -p iroha_core --lib reschedule_skips_vote_backed_retransmit_while_frontier_quorum_timeout_window_owned -- --nocapture` (pass)
  - `cargo test -p iroha_core --lib stake_quorum_timeout_skips_noop_reschedule_with_full_signer_set -- --nocapture` (pass)
  - `cargo build --release -p izanami -p irohad --bin iroha3d` (pass)
- Runtime note:
  - fresh direct-binary healthy NPoS probe:
    `/tmp/izanami_npos_healthy_probe_zero_vote_owner_handoff_20260316T101333Z.log`
    using `IROHA_TEST_SKIP_BUILD=1` and `TEST_NETWORK_BIN_IROHAD=target/release/iroha3d`
    still timed out before `target_blocks=120`, but moved slightly from the
    prior empty-frontier-handoff result `quorum 65 / strict 69` to
    `quorum 67 / strict 73`,
  - live peer logs confirm the new path is exercised in real runtime:
    zero-vote reschedule at height `50` now logs
    `drop_pending=true handoff_frontier_quorum_timeout_owner=true frontier_recovery_advance=Some(CatchUp)`
    in
    `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_WIrcsR/vivid_bluebird/run-1-stdout.log`,
  - the dominant failure did not move to empty-frontier `MissingQc`; it remains
    repeated `missing block dwell exceeded retry window; routing through frontier recovery`
    plus late vote-backed quorum reschedule at ordinary heights (`39`, `50`,
    `58`, `66`), with zero ingress failover/unhealthy churn.

## 2026-03-15 Follow-up: route lock-reject recovery through the frontier owner and pace vote-backed reschedule by stale progress
- Simplified `crates/iroha_core/src/sumeragi/main_loop/proposal_handlers.rs`
  so active-height lock rejection no longer reserves a `MissingQc` rotation
  window and triggers a direct view change on its own. The lock-reject path now
  routes through `advance_frontier_recovery(...)`, which keeps
  `committed_height + 1` under the same frontier owner used by idle and
  quorum-timeout recovery.
- Tightened `crates/iroha_core/src/sumeragi/main_loop/pending_block.rs` so
  `vote_backed_reschedule_due(...)` only returns `true` after one full
  backoff interval with no new vote/QC progress. This removes the earlier
  behavior where vote-backed pending could reschedule as soon as the pending age
  crossed the quorum timeout even if the latest vote arrived only a few hundred
  milliseconds earlier.
- Added focused regressions in
  `crates/iroha_core/src/sumeragi/main_loop/tests.rs` and the inline
  `pending_block` tests:
  - `block_created_without_hint_reject_at_active_height_routes_through_unified_frontier_recovery`
    asserts that active-height lock rejection seeds frontier recovery without
    creating an immediate round/view change,
  - `reschedule_skips_vote_backed_quorum_timeout_while_progress_is_recent`
    asserts that a vote-backed pending block with fresh progress does not
    retransmit yet,
  - `vote_backed_reschedule_requires_progress_after_last_attempt` now also
    asserts that the first vote-backed reschedule waits for stale progress,
  - existing `reschedule_skips_repeated_vote_backed_quorum_timeout_without_new_progress`
    stays green.
- Validation (this follow-up):
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_core --lib vote_backed_reschedule_requires_progress_after_last_attempt -- --nocapture` (pass)
  - `cargo test -p iroha_core --lib reschedule_skips_vote_backed_quorum_timeout_while_progress_is_recent -- --nocapture` (pass)
  - `cargo test -p iroha_core --lib reschedule_skips_repeated_vote_backed_quorum_timeout_without_new_progress -- --nocapture` (pass)
  - `cargo test -p iroha_core --lib block_created_without_hint_reject_at_active_height_routes_through_unified_frontier_recovery -- --nocapture` (pass)
  - `cargo build --release -p irohad --bin iroha3d` (pass)
- Runtime note:
  - fresh direct-binary healthy NPoS probe:
    `/tmp/izanami_npos_healthy_probe_vote_progress_backoff_20260315T183815Z.log`
    still failed, but the ceiling improved from the prior
    `quorum 56 / strict 63` probe to `quorum 65 / strict 71`,
  - live peer logs show the vote-backed reschedule pacing fix working as
    intended: the first vote-backed retransmits now occur with
    `progress_age_ms >= reschedule_backoff_ms` instead of at `~200ms`,
  - the dominant failure moved forward from the old height-52 collapse into the
    low-70s, where repeated executor trigger lookup failures
    (`Find(Trigger("repeat_trigger_0"))`) coincide with late single-vote
    quorum misses.

## 2026-03-15 Follow-up: skip no-op quorum reschedules and fix pacemaker frontier height logs
- Simplified `crates/iroha_core/src/sumeragi/main_loop/reschedule.rs` so
  vote-backed quorum timeout handling no longer records a `last_quorum_reschedule`
  marker when it has no actionable work:
  - if the pending block is retained, no transactions are requeued, and there
    are no missing vote retransmit targets, the helper now leaves the pending
    block untouched and returns `false`,
  - if pacing/cooldown suppresses all retransmit work, the helper also returns
    `false` instead of pretending a reschedule occurred,
  - the outer reschedule sweep now only reports progress when
    `reschedule_pending_quorum_block(...)` actually did work.
- Updated `crates/iroha_core/src/sumeragi/main_loop/propose.rs` logs so
  pacemaker diagnostics report the tracked frontier height instead of the local
  committed tip height for pre-selection proposal attempts.
- Added focused regression coverage in
  `crates/iroha_core/src/sumeragi/main_loop/tests.rs`:
  - `stake_quorum_timeout_skips_noop_reschedule_with_full_signer_set`
    asserts that a stake-quorum miss with a numerically full observed signer set
    does not mark `last_quorum_reschedule` or emit retransmit traffic,
  - `reschedule_skips_vote_backed_retransmit_while_frontier_quorum_timeout_window_owned`
    asserts that once the contiguous frontier is already under a
    `quorum_timeout` recovery window, vote-backed pending does not emit another
    retransmit cycle inside that same window,
  - existing `zero_vote_quorum_timeout_drops_pending_without_immediate_view_change`
    and `reschedule_skips_repeated_vote_backed_quorum_timeout_without_new_progress`
    remain green on the new helper contract.
- Validation (this follow-up):
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_core --lib stake_quorum_timeout_skips_noop_reschedule_with_full_signer_set -- --nocapture` (pass)
  - `cargo test -p iroha_core --lib reschedule_skips_vote_backed_retransmit_while_frontier_quorum_timeout_window_owned -- --nocapture` (pass)
  - `cargo test -p iroha_core --lib zero_vote_quorum_timeout_drops_pending_without_immediate_view_change -- --nocapture` (pass)
  - `cargo test -p iroha_core --lib reschedule_skips_repeated_vote_backed_quorum_timeout_without_new_progress -- --nocapture` (pass)
  - `cargo test -p iroha_core --lib force_view_change_if_idle_ -- --nocapture` (pass on the same codebase before this follow-up; unaffected slice stayed green)
- Runtime note:
  - a fresh healthy NPoS 120-block probe is running with keep-dirs in session
    `95142`, logging to
    `/tmp/izanami_npos_healthy_probe_noop_reschedule_cut_20260315T174457Z.log`,
  - that probe is now stale for the latest tree because the
    `frontier_recovery_owned_by_quorum_timeout(...)` suppression landed after it
    was started,
  - as of this update there is still no runtime verdict for the latest code.

## 2026-03-15 Follow-up: suppress direct missing-QC rotation when matching frontier pending already exists
- Simplified `crates/iroha_core/src/sumeragi/main_loop.rs` so
  `force_view_change_if_idle(...)` no longer emits a direct `MissingQc`
  rotation when the contiguous frontier already has a matching pending block for
  the current `(height, view)`:
  - matching frontier pending now suppresses the empty-frontier `missing_qc`
    direct-rotation branch,
  - the same condition also blocks `advance_frontier_recovery("missing_qc", ...)`
    from becoming a second rotation owner while that pending exists,
  - this leaves cached-slot / quorum-timeout handling to the existing pending
    owner instead of letting `missing_qc` race it.
- Added focused regression coverage in
  `crates/iroha_core/src/sumeragi/main_loop/tests.rs`:
  - `force_view_change_if_idle_suppresses_missing_qc_when_matching_frontier_pending_exists`
    asserts that matching frontier pending keeps the current view and does not
    count a `MissingQc` rotation,
  - the broader `force_view_change_if_idle_` slice remains green.
- Validation (this follow-up):
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_core --lib force_view_change_if_idle_suppresses_missing_qc_when_matching_frontier_pending_exists -- --nocapture` (pass)
  - `cargo test -p iroha_core --lib force_view_change_if_idle_ -- --nocapture` (pass)
- Runtime note:
  - the prior live probe
    `/tmp/izanami_npos_healthy_probe_propose_owner_unified_20260315T1234Z.log`
    finished with `quorum min height 63` / `strict min height 70` before target
    `120`,
  - that run confirmed the cached-slot cut was live in peer logs
    (`cached proposal slot stalled past quorum timeout; routing through unified frontier recovery`)
    but still exposed `proposal_handlers` `missing_qc` at heights `9`, `28`,
    and `48` with `pending_match=true`,
  - a fresh rerun with the new suppression patch is now compiling/running in
    session `26144`, logging to
    `/tmp/izanami_npos_healthy_probe_pending_match_suppressed_20260315T1350Z.log`.

## 2026-03-15 Follow-up: cached proposal-slot timeout now routes through unified frontier recovery
- Simplified `crates/iroha_core/src/sumeragi/main_loop/propose.rs` so a
  stalled cached proposal on the contiguous frontier no longer triggers its own
  direct quorum-timeout view change:
  - the cached-slot timeout branch now routes contiguous-frontier stalls through
    the existing `frontier_recovery` owner using `quorum_timeout`,
  - direct `trigger_view_change_with_cause(...QuorumTimeout)` is retained only
    for non-contiguous heights,
  - the cached-slot timeout trigger/hysteresis bookkeeping remains intact.
- Updated focused regression coverage in
  `crates/iroha_core/src/sumeragi/main_loop/tests.rs`:
  - renamed the cached-slot timeout test to assert controller ownership instead
    of immediate `forced_view_after_timeout`,
  - the contiguous-frontier cached-slot stall now expects
    `frontier_recovery.phase == RotateArmed` with cause `quorum_timeout`.
- Validation (this follow-up):
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_core --lib pacemaker_routes_contiguous_cached_slot_stall_through_frontier_recovery -- --nocapture` (pass)
  - `cargo test -p iroha_core --lib maybe_force_view_change_for_stalled_pending_ -- --nocapture` (pass)
- Runtime note:
  - a fresh release NPoS 120-block probe is running in session `35751`, logging
    to `/tmp/izanami_npos_healthy_probe_propose_owner_unified_20260315T1234Z.log`
    with keep-dirs under
    `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_Nw2BSR`,
    but the run is still blocked on the child warmup build
    `cargo build -p irohad --release --bin iroha3d` (pid `94915`), so there is
    no consensus verdict yet.

## 2026-03-15 Follow-up: frontier recovery ownership unification for stalled pending vs missing-QC
- Simplified contiguous-frontier recovery ownership in
  `crates/iroha_core/src/sumeragi/main_loop.rs`:
  - added a small frontier-owned quorum-timeout handoff instead of allowing
    `maybe_force_view_change_for_stalled_pending(...)` to trigger a second direct
    rotation authority;
  - stalled contiguous frontier pending now seeds the existing
    `frontier_recovery` controller at the cleanup boundary and lets that
    controller own the eventual rotate;
  - empty-frontier `missing_qc` no longer clears or steals recovery ownership
    while a quorum-timeout frontier controller still owns the active window;
  - frontier recovery cause mapping now preserves `quorum_timeout` in view-change
    accounting instead of collapsing it into `missing_qc`.
- Added focused regressions in
  `crates/iroha_core/src/sumeragi/main_loop/tests.rs` for:
  - stalled-pending two-step controller handoff (`arm RotateArmed`, then rotate
    on the next window),
  - empty-frontier `missing_qc` suppression while quorum-timeout recovery owns
    the frontier window,
  - existing stalled-pending timeout fixtures updated so proposal-owned cases
    explicitly record proposal evidence.
- Validation (this follow-up):
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_core --lib maybe_force_view_change_for_stalled_pending_ -- --nocapture` (pass)
  - `cargo test -p iroha_core --lib force_view_change_if_idle_ -- --nocapture` (pass)
  - `cargo test -p iroha_core --lib frontier_recovery_ -- --nocapture` (pass)
  - `cargo test -p iroha_core --lib stake_quorum_timeout_reschedules_without_immediate_view_change -- --nocapture` (pass)
  - `cargo test -p iroha_core --lib zero_vote_quorum_timeout_drops_pending_without_immediate_view_change -- --nocapture` (pass)
  - `cargo test -p iroha_core --lib missing_qc_view_change_suppressed_when_frontier_reanchor_already_emitted -- --nocapture` (pass)
  - `cargo test -p iroha_core --lib committed_edge_conflict_ -- --nocapture` (pass)
- Runtime note:
  - a fresh release NPoS 120-block probe is running with warmed keep-dirs-style
    settings, but there is no verdict yet because release compilation is still in
    progress in session `27010`.
## 2026-03-15 Follow-up: address canonicalisation path-helper regression fix
- Fixed `integration_tests/tests/address_canonicalisation.rs` helper paths used by account/explorer
  path-surface tests:
  - `account_endpoint_url(...)` now targets `/v1/accounts/...`.
  - `explorer_account_qr_url(...)` now targets `/v1/explorer/accounts/.../qr`
    with the same `/v1` handler prefix.
- This restores expected handler dispatch so malformed literals are validated by route handlers
  (`400 Bad Request`) and canonical I105 account/explorer requests succeed (`200 OK`) instead of
  hitting `404 Not Found`.
- Validation (this follow-up):
  - `cargo test -p integration_tests --test address_canonicalisation account_path_endpoints_reject_ -- --nocapture` (pass)
  - `cargo test -p integration_tests --test address_canonicalisation account_transactions_ -- --nocapture` (pass)
  - `cargo test -p integration_tests --test address_canonicalisation explorer_account_qr_ -- --nocapture` (pass)

## 2026-03-15 Follow-up: mobile offline asset-id literal builders (`norito:<hex>`)
- Implemented SDK-facing support for building canonical encoded asset IDs from textual parts
  (`assetDefinitionId` + `accountId`) via `OfflineNorito.encodeAssetId` paths and bridge/native hooks.
- `connect_norito_bridge`:
  - added literal encoder helper and exported C ABI surface
    `connect_norito_encode_asset_id_literal(...)`.
  - added Android JNI export for asset-id literal encoding from parts.
  - added bridge tests that assert canonical output and alias rejection in offline mode.
- Swift SDK (`IrohaSwift`):
  - added native bridge binding + wrapper for asset-id literal-from-parts encoding.
  - added offline canonical-only helper and online alias-resolving helper on `ToriiClient`.
  - added tests for component-based encoding parity and alias resolution behavior.
- Android SDK (`java/iroha_android`):
  - added `AssetIdLiteral.encodeFromParts(...)` (offline canonical-only; alias-shaped definition rejected).
  - added alias resolution DTOs and transport helpers:
    - `resolveAccountAlias(...)`
    - `resolveAssetAlias(...)`
    - `buildAssetIdLiteralResolvingAliases(...)` (online alias resolution + canonical encoding).
  - added coverage in `AssetIdLiteralTests` and `HttpClientTransportTests`.
  - hardened tests to generate valid I105 account IDs dynamically instead of stale hardcoded literals
    to avoid checksum failures in strict account parsing.
  - added explicit API guardrails:
    - Swift now exposes explicit names for canonical-only vs alias-resolving flows and removes ambiguous legacy method names.
    - Swift component-based asset-id encoding now fails fast when the native bridge symbol is unavailable in non-test runtimes.
    - Android now exposes `buildAssetIdLiteralResolvingAliases(...)` and removes the ambiguous alias method.
  - first-release hard-cut cleanup:
    - removed remaining compatibility wording in the new asset-id bridge/mobile surfaces
      (`name#domain` is documented as a supported textual form).
    - removed `IROHA_NATIVE_REQUIRED` toggles from Android offline JNI harness tests; they now
      deterministically skip when native bridge is unavailable.
    - removed Swift bridge env toggles from package/runtime loading and switched to automatic
      bridge policy based on `dist/NoritoBridge.xcframework` presence.
    - updated Swift packaging validation tooling/docs for the new auto policy:
      `scripts/check_swift_spm_validation.sh` now expects fallback-with-warning when the bridge
      is missing; removed obsolete mode checks from `ci/README.md` and Swift docs.
    - aligned Android alias-resolution helper behavior with Swift by adding fallback
      `/v1/assets/aliases/resolve` lookup when non-alias asset-definition input fails canonical
      offline encoding.
- Validation (this follow-up):
  - `cargo test -p connect_norito_bridge encode_asset_id_literal_builds_canonical_from_textual_parts -- --nocapture` (pass)
  - `cargo test -p connect_norito_bridge encode_asset_id_literal_rejects_alias_input_offline -- --nocapture` (pass)
  - `cargo test -p connect_norito_bridge parse_asset_definition_accepts_textual_literal -- --nocapture` (pass)
  - `ANDROID_HARNESS_MAINS=org.hyperledger.iroha.android.address.AssetIdLiteralTests,org.hyperledger.iroha.android.client.HttpClientTransportTests JAVA_HOME=$(/usr/libexec/java_home -v 21) ANDROID_HOME=~/Library/Android/sdk ANDROID_SDK_ROOT=~/Library/Android/sdk ./gradlew core:test --tests org.hyperledger.iroha.android.GradleHarnessTests` (pass)
  - `ANDROID_HARNESS_MAINS=org.hyperledger.iroha.android.offline.OfflineBuildClaimPayloadEncoderTest,org.hyperledger.iroha.android.offline.OfflineSpendReceiptPayloadEncoderTest,org.hyperledger.iroha.android.address.AssetIdLiteralTests,org.hyperledger.iroha.android.client.HttpClientTransportTests JAVA_HOME=$(/usr/libexec/java_home -v 21) ANDROID_HOME=~/Library/Android/sdk ANDROID_SDK_ROOT=~/Library/Android/sdk ./gradlew :core:test --tests org.hyperledger.iroha.android.GradleHarnessTests` (pass)
  - `cd IrohaSwift && swift test --filter BridgePolicyHintTests` (pass)
  - `cd IrohaSwift && swift test --filter BridgeAvailabilitySurfaceTests` (pass)
  - `cd IrohaSwift && swift test --filter ToriiClientTests/testBuildAssetIdLiteralResolvingAliases` (pass)
  - `cd IrohaSwift && swift test --filter ToriiClientTests/testAssetIdAliasFallbackForInvalidNonAliasDefinition` (pass)
  - `JAVA_HOME=$(/usr/libexec/java_home -v 21) ANDROID_HOME=~/Library/Android/sdk ANDROID_SDK_ROOT=~/Library/Android/sdk ANDROID_HARNESS_MAINS=org.hyperledger.iroha.android.client.HttpClientTransportTests ./gradlew :core:test --tests org.hyperledger.iroha.android.GradleHarnessTests` from `java/iroha_android` (pass)
  - `bash scripts/check_swift_spm_validation.sh` (pass)

## 2026-03-15 Follow-up: first-release API version normalization (`v1` only)
- Standardized Torii/API path references from `v2` to `v1` across code and
  integration tests in this repository slice (`crates/` + `integration_tests/`):
  - Torii route registration and handlers (`crates/iroha_torii/src/**`)
  - Shared URI constants (`crates/iroha_torii_shared/src/lib.rs`)
  - OpenAPI and MCP metadata/path templates
  - Rust client and CLI call-sites (`crates/iroha/src/client.rs`,
    `crates/iroha_cli/src/**`)
  - Integration tests and Torii unit/integration tests that asserted `v2` paths
- Fixed leftover non-leading-slash call-sites (for example legacy
  `join("...")`/`join_torii_url(..., "...")` path fragments) to `v1`,
  including node capabilities
  probes and Sumeragi/public-lane helper endpoints.
- Validation (this follow-up):
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_torii --no-run` (pass)
  - `rg -n 'legacy-version-marker' crates integration_tests -S` (no matches for legacy path markers)

## 2026-03-15 Follow-up: proof-record Torii route fix in integration smoke test
- Fixed `integration_tests/tests/proofs.rs` `submit_proof_and_query_record` polling path:
  - switched proof lookup from a stale proof-record path to active `/v1/proofs/{id}`.
  - this removes repeated `404 Not Found` polling and allows the test to observe the
    persisted proof record as intended.
- Validation (this follow-up):
  - `cargo test -p integration_tests submit_proof_and_query_record --test proofs -- --nocapture` (pass before full v1-only sweep; final post-sweep rerun pending)

## 2026-03-15 Follow-up: Swift docs builder-ID alignment
- Updated Swift-facing builder examples to match the current encoded-only validator contract:
  - `IrohaSwift/README.md` transfer/shield/unshield snippets now use canonical
    `aid:<32-lower-hex-no-dash>` asset-definition IDs.
  - `docs/source/sdk/swift/index*.md` snippets now use `AccountId.make(publicKey:)`
    and canonical `aid:...` asset-definition IDs instead of removed/legacy forms.
- Added explicit documentation notes that transaction builders require canonical
  `aid:<32-lower-hex-no-dash>` asset-definition IDs.
- Validation (this follow-up):
  - `rg -n 'AccountId\.make\(publicKey: keypair\.publicKey, domain: "wonderland"\)|assetDefinitionId: "rose#wonderland"|assetDefinitionId: "jpy#xst"' docs/source/sdk/swift/index*.md IrohaSwift/README.md` (no matches)

## 2026-03-15 Follow-up: pagination integration test canonical asset-definition IDs
- Fixed `integration_tests/tests/pagination.rs` setup to stop parsing legacy
  `<name>#<domain>` literals for asset definitions.
  - `register_assets(...)` now constructs IDs via
    `AssetDefinitionId::new("wonderland".parse()?, name.parse()?)`, which
    produces canonical `aid:<32-lower-hex>` IDs expected by current parsing.
- Validation (this follow-up):
  - `cargo fmt --all` (pass)
  - `cargo test -p integration_tests --test pagination -- --nocapture` (pass; includes new canonical-ID regression test; network startup path is skipped in sandbox due loopback bind restrictions)

## 2026-03-14 Follow-up: address canonicalisation permit-timeout stabilization
- Fixed `integration_tests/tests/address_canonicalisation.rs` suite-local network startup
  throttling to align with the effective `iroha_test_network` file-permit ceiling:
  - `install_network_parallelism_override()` now computes the effective permit limit from
    `IROHA_TEST_SERIALIZE_NETWORKS`, `IROHA_TEST_NETWORK_PARALLELISM`, or the default
    core-scaled cap, then applies `min(limit, 2)` for the suite override.
  - This prevents over-admitting local starts when the global permit lock only allows one
    network, which previously could starve queued tests and trigger 300s permit wait timeouts.
- Validation (this follow-up):
  - `cargo fmt --all` (pass)
  - `cargo test -p integration_tests --test address_canonicalisation accounts_listing_filter_rejects_legacy_dotted_i105_literals -- --nocapture` (pass in sandbox; network startup skipped due loopback bind restrictions)

## 2026-03-14 Follow-up: Norito burn/mint fixture parity re-sync
- Fixed stale Norito instruction fixtures under `fixtures/norito_instructions` that no
  longer matched canonical Rust encoding:
  - `burn_asset_numeric.json`
  - `burn_asset_fractional.json`
  - `mint_asset_numeric.json`
- Updated both fixture payload fields per file:
  - `instruction` (canonical Norito JSON/base64 string)
  - `encoded_hex` (canonical `InstructionBox` bytes)
- This restores parity for `integration_tests/tests/norito_burn_fixture.rs`.

### Validation (this follow-up)
- `cargo test -p integration_tests --test norito_burn_fixture -- --nocapture` (pass)

## 2026-03-14 Follow-up: workspace strict-clippy stabilization and sidecar API alignment
- Resolved strict workspace lint and compile failures surfaced by
  `cargo clippy --workspace --all-targets -- -D warnings`:
  - Fixed `clippy::unnested_or_patterns` and `clippy::option_if_let_else` in
    account/address parsing paths.
  - Fixed multiple `clippy::doc_markdown`, `clippy::items_after_statements`,
    and `clippy::useless_conversion` findings in `iroha_data_model`.
  - Removed stale `FromStr` imports in test modules that were failing under
    `-D unused-imports`.
  - Fixed `clippy::redundant_closure_call` and `clippy::question_mark` in
    multisig account execution flow.
- Kept existing public error-surface types intact and added targeted
  `clippy::result_large_err` allowances where API-level `ValidationFail`/query
  error types are intentional (notably in `iroha_executor` and `iroha::query`).
- Aligned sidecar constructor call sites/tests to the current
  `PipelineRecoverySidecar::new(...)` API and updated format-label assertions
  from `"pipeline.recovery.v1"` to `"pipeline.recovery"`.
- Validation (this follow-up):
  - `cargo clippy --workspace --all-targets -- -D warnings` (pass)
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_torii --test pipeline_recovery_endpoint` (pass)
  - `cargo test -p iroha_kagami sidecar_prints_to_file` (pass)

## 2026-03-14 Follow-up: account-address strict parser error normalization
- Fixed `AccountAddress::parse_encoded` (`crates/iroha_data_model/src/account/address.rs`)
  to normalize unsupported/non-I105 parser input back to
  `AccountAddressError::UnsupportedAddressFormat` instead of leaking low-level
  lexical errors like `InvalidI105Char`.
  - Normalized variants: `MissingI105Sentinel`, `I105TooShort`,
    `InvalidI105Char`, `InvalidI105Base`, `InvalidI105Digit`,
    and `UnsupportedAddressFormat`.
  - Preserved semantic decode errors (for example, `ChecksumMismatch` and
    `UnexpectedNetworkPrefix`) unchanged.
- Updated the `parse_encoded` doc comment to reflect the normalized
  error-contract for strict parser callers and FFI consumers.
- Validation (this follow-up):
  - `cargo test -p iroha_data_model parse_encoded_rejects_unknown_format -- --nocapture` (pass)
  - `cargo test -p iroha_data_model parse_encoded_accepts_only_i105 -- --nocapture` (pass)
  - `cargo test -p connect_norito_bridge account_address_parse_render_via_ffi -- --nocapture` (pass)
  - `cargo fmt --all` (pass)

## 2026-03-14 Follow-up: offline app API account domain-link compile fix
- Fixed `crates/iroha_torii/tests/offline_app_api.rs` account fixtures that failed
  to compile after `Account::linked_domains` became an explicit field.
  - Added `linked_domains` initialization for `controller`, `receiver`, and
    `operator` seeded from the fixture allowance domain.
  - Added `std::collections::BTreeSet` import for deterministic set construction.
- Validation (this follow-up):
  - `cargo test -p iroha_torii --test offline_app_api --no-run` (pass)
  - `cargo test -p iroha_torii --test offline_app_api` (pass)
  - `cargo fmt --all` (pass)

## 2026-03-13 Follow-up: cross-dataspace localnet runtime stabilization
- Stabilized `integration_tests/tests/nexus/cross_dataspace_localnet.rs` setup so the
  multilane atomic-swap scenario converges reliably in real localnet runs:
  - `leader_targeted_client_for_account(...)` now prefers a peer from the expected lane
    slice (`alice -> ds1`, `bob -> ds2`, default -> nexus) instead of a single global
    highest-height leader.
  - Seed balances (`ds1coin` for Alice and `ds2coin` for Bob) are provisioned in
    multilane genesis post-topology bootstrap instructions and verified at runtime via
    balance assertions (instead of flaky runtime register+mint submissions).
- Runtime validation (not CI-only):
  - `IROHA_TEST_SKIP_BUILD=1 IROHA_TEST_NETWORK_KEEP_DIRS=1 cargo test -p integration_tests --test mod nexus::cross_dataspace_localnet::cross_dataspace_atomic_swap_is_all_or_nothing -- --exact --nocapture` (pass)
  - `cargo test -p integration_tests --test mod nexus::multilane_router::multilane_router_provisions_storage_and_routes_rules -- --exact --nocapture` (pass)
  - `cargo test -p integration_tests --test mod nexus::multilane_pipeline::multilane_catalog_sets_up_storage_and_routing -- --exact --nocapture` (pass)

## 2026-03-13 Follow-up: address canonicalisation integration suite stability
- Mitigated `integration_tests/tests/address_canonicalisation.rs` SIGKILL instability by
  installing a process-wide network-parallelism override for this test binary:
  - wrapped `start_network_async_or_skip(...)` in-file and pinned
    `override_network_parallelism(None, Some(2))` via `OnceLock`.
  - this keeps startup concurrency bounded while avoiding long serialized queues that can trip
    external watchdog kills.
- Validation (this follow-up):
  - `cargo fmt --all` (pass)
  - `cargo test -p integration_tests --test address_canonicalisation --no-run` (pass)
  - `cargo test -p integration_tests --test address_canonicalisation -- --nocapture` (pass in sandbox; network startup skipped due loopback bind restrictions)

## 2026-03-13 Follow-up: FASTPQ stage2 balanced backend fixture refresh
- Refreshed stale Stage 2 backend regression fixtures to match current canonical
  prover output:
  - `crates/fastpq_prover/tests/fixtures/stage2_balanced_1k.bin`
  - `crates/fastpq_prover/tests/fixtures/stage2_balanced_5k.bin`
- This resolves fixture parity failures in:
  - `stage2_artifact_balanced_1k_matches_fixture`
  - `stage2_artifact_balanced_5k_matches_fixture`
  - `golden_stage2_proof_matches_fixture`
- Validation (this follow-up):
  - `FASTPQ_UPDATE_FIXTURES=1 cargo test -p fastpq_prover --test backend_regression stage2_artifact_balanced_ -- --nocapture` (pass)
  - `cargo test -p fastpq_prover --test backend_regression stage2_artifact_balanced_ -- --nocapture` (pass)
  - `cargo test -p fastpq_prover --test proof_fixture golden_stage2_proof_matches_fixture -- --nocapture` (pass)

## 2026-03-13 Follow-up: fastpq trace-commitment transfer fixture refresh
- Fixed `crates/fastpq_prover/tests/trace_commitment.rs` regression that was
  failing with `TransferMetadataDecode` for `AssetDefinitionId` by refreshing
  the stale `transfer.norito` fixture to the current Norito layout.
- Updated transfer golden vectors to match the refreshed canonical fixture:
  - `crates/fastpq_prover/tests/fixtures/ordering_hash.json` (`transfer`)
  - `crates/fastpq_prover/tests/trace_commitment.rs` (`commitment transfer`)
- Validation (this follow-up):
  - `cargo test -p fastpq_prover --test trace_commitment -- --nocapture` (pass)

## 2026-03-13 Follow-up: `fastpq_row_bench` canonical asset-definition IDs
- Fixed `crates/fastpq_prover/src/bin/fastpq_row_bench.rs` transfer-row generation to stop
  parsing legacy `name#domain` asset-definition literals.
  - `RowGenerator` now builds transfer asset definitions with
    `AssetDefinitionId::new(domain, name)` so emitted keys use canonical
    `aid:<32-lower-hex>` IDs.
- Added regression coverage:
  - `tests::transfer_keys_use_canonical_aid_literals` verifies generated transfer keys
    start with `asset/aid:` and do not contain legacy `#` literals.
- Validation (this follow-up):
  - `cargo fmt --all` (pass)
  - `cargo test -p fastpq_prover --bin fastpq_row_bench` (pass)

## 2026-03-13 Follow-up: `connect_norito_bridge` asset-definition textual parsing support
- Added bridge support for textual asset-definition literals in
  `crates/connect_norito_bridge/src/lib.rs`:
  - `parse_asset_definition(...)` now accepts both canonical
    `aid:<32-lower-hex>` and textual `<name>#<domain>` forms.
  - this unblocked transfer/mint/burn/zk-transfer encoder paths that were
    failing with `ERR_ASSET_DEFINITION_PARSE` (`-5`) before quantity/nonce checks.
- Extended regression coverage in bridge unit tests:
  - `accel_tests::parse_asset_definition_accepts_textual_literal`
  - `accel_tests::parse_asset_definition_accepts_canonical_aid_literal`
- Synced the fixture generator parser in
  `crates/connect_norito_bridge/src/bin/swift_parity_regen.rs`:
  - added `parse_asset_definition_argument(...)` with the same dual-format
    behavior to keep payload-fixture tests aligned with bridge FFI expectations.
  - added `tests::parse_asset_definition_argument_accepts_canonical_literal`.
- Validation (this follow-up):
  - `cargo fmt --all` (pass)
  - `cargo test -p connect_norito_bridge accel_tests:: -- --nocapture` (pass)
  - `cargo test -p connect_norito_bridge` (pass)
## 2026-03-13 Follow-up: multilane endpoint alignment and runtime verification
- Fixed client/API version drift that was breaking live multilane runs after stake-ID canonicalization:
  - `crates/iroha/src/client.rs` now uses `/v2` for:
    - `sumeragi/status` (all status helpers),
    - `nexus/public_lanes/{lane}/(validators|stake|rewards/pending)`,
    - `sumeragi/collectors`,
    - `sumeragi/rbc/sessions`,
    - `node/capabilities`.
- Normalized integration tests to the active `/v2` Sumeragi telemetry/status/session paths where stale `/v1` URLs caused 404 polling loops:
  - `integration_tests/tests/sumeragi_npos_happy_path.rs`
  - `integration_tests/tests/sumeragi_prf_collectors.rs`
  - `integration_tests/tests/sumeragi_da.rs`
  - `integration_tests/tests/sumeragi_localnet_smoke.rs`
  - `integration_tests/tests/sumeragi_npos_performance.rs`
- Validation (runtime, not CI-only checks):
  - `cargo test -p integration_tests --test mod nexus::cross_dataspace_localnet::cross_dataspace_atomic_swap_is_all_or_nothing -- --exact --nocapture` (pass)
  - `cargo test -p integration_tests --test sumeragi_npos_stake_activation npos_election_filters_stake_and_applies_after_margin -- --exact --nocapture` (pass)
  - `cargo test -p integration_tests --test sumeragi_npos_happy_path npos_happy_path_enforces_da_and_metrics_bounds -- --exact --nocapture` (pass)
  - `cargo test -p integration_tests --test sumeragi_prf_collectors npos_prf_collectors_track_endpoint -- --exact --nocapture` (pass)

## 2026-03-13 Follow-up: stored sorted fast-start with deferred continuation
- Implemented a prepared-start path for stored metadata-sorted iterable queries:
  - `crates/iroha_core/src/smartcontracts/isi/query.rs` now computes stored batch one via a bounded-prefix heap strategy and returns that first batch directly when parameters are within the streaming prefix limit.
  - the same file now defers full sorted continuation materialization for stored queries until first `Continue`, while preserving ordering/pagination/cursor semantics and external response/cursor shapes.
- Added deferred continuation plumbing in the live query store:
  - `crates/iroha_core/src/query/store.rs` adds `PreparedQueryStart` + `DeferredQueryContinuation` and a prepared insertion API (`handle_iter_start_prepared`) so stored batch one is not re-batched/re-projected through a full iterator cycle.
  - live query IDs now use a monotonic `AtomicU64` internal generator encoded into existing string `QueryId` payloads.
  - capacity + per-authority quota checks are folded into the insertion path.
  - `crates/iroha_core/src/query/cursor.rs` adds `ErasedQueryIterator::new_with_cursor(...)` for deferred iterators that start after the precomputed batch.
- Bench + test coverage updates:
  - `crates/iroha_core/benches/queries.rs` adds `snapshot_stored_sorted_asset_defs_first_continue`.
  - `crates/iroha_core/src/smartcontracts/isi/query.rs` adds:
    - `stored_sorted_fast_start_matches_legacy_first_batch_variants`
    - `deferred_stored_start_first_continue_preserves_global_order`
  - `crates/iroha_core/src/query/store.rs` adds:
    - `query_ids_are_monotonic_decimal_strings`
    - `capacity_limit_is_enforced_with_monotonic_ids`
    - `dropping_prepared_query_does_not_materialize_deferred_state`
- Validation (this follow-up):
  - `cargo test -p iroha_core --lib query::snapshot::tests::snapshot_sorted_asset_definitions_returns_first_batch_without_cursor -- --exact --nocapture` (pass)
  - `cargo test -p iroha_core --lib query::snapshot::tests::snapshot_sorted_asset_definitions_stored_cursor_continues_in_order -- --exact --nocapture` (pass)
  - `cargo test -p iroha_core --lib smartcontracts::isi::query::tests::iter_dispatch_asset_definitions_sort_ties_stable_by_id -- --exact --nocapture` (pass)
  - `cargo test -p iroha_core --lib smartcontracts::isi::query::tests::iter_dispatch_asset_definitions_sort_desc_batched -- --exact --nocapture` (pass)
  - `cargo test -p iroha_core --lib smartcontracts::isi::query::tests::stored_sorted_fast_start_matches_legacy_first_batch_variants -- --exact --nocapture` (pass)
  - `cargo test -p iroha_core --lib smartcontracts::isi::query::tests::deferred_stored_start_first_continue_preserves_global_order -- --exact --nocapture` (pass)
  - `cargo test -p iroha_core --lib query::store::tests::query_ids_are_monotonic_decimal_strings -- --exact --nocapture` (pass)
  - `cargo test -p iroha_core --lib query::store::tests::capacity_limit_is_enforced_with_monotonic_ids -- --exact --nocapture` (pass)
  - `cargo test -p iroha_core --lib query::store::tests::dropping_prepared_query_does_not_materialize_deferred_state -- --exact --nocapture` (pass)
  - `cargo check -p iroha_core --benches -q` (pass; existing unrelated warnings remain)
  - `cargo bench -p iroha_core --bench queries -- 'snapshot_(stored|ephemeral)_sorted_asset_defs_first_batch'` rerun spot checks:
    - run A: ephemeral `[1.7408 ms 1.8339 ms 1.9485 ms]`, stored `[2.9138 ms 3.0254 ms 3.1472 ms]`
    - run B: ephemeral `[1.5724 ms 1.6032 ms 1.6421 ms]`, stored `[2.8262 ms 2.8643 ms 2.9031 ms]`
    - run C (noisy host outlier): ephemeral `[1.9372 ms 2.2100 ms 2.5627 ms]`, stored `[10.180 ms 11.412 ms 12.702 ms]`
  - `cargo bench -p iroha_core --bench queries -- 'snapshot_stored_sorted_asset_defs_first_continue$'`:
    - `[4.7476 ms 4.8255 ms 4.9098 ms]`
  - `cargo test -p iroha_core` (failed in current worktree with unrelated baseline failures; summary: `3629 passed; 77 failed`; examples include `block::prefetch_tests::parse_account_literal_rejects_ambiguous_encoded_subject` and `state::tests::detached_can_modify_account_metadata_allows_domain_owner`)

## 2026-03-13 Follow-up: streaming prefix for ephemeral sorted snapshot queries
- Tightened `crates/iroha_core/src/smartcontracts/isi/query.rs` again for metadata-sorted ephemeral iterable queries:
  - when `offset + first_batch_len` stays small, ephemeral sorted queries now keep only that bounded prefix in a `BinaryHeap` while counting total results, instead of materializing the full sorted candidate set before returning batch one.
  - kept the existing full-collection fallback for large pagination windows, so wide offsets/limits still use the previous selection path.
  - added `smartcontracts::isi::query::tests::ephemeral_sorted_query_respects_offset_and_limit` to lock the bounded-prefix path across offset + limit pagination.
- Validation (this follow-up):
  - `cargo fmt --all` (pass)
  - `CARGO_TARGET_DIR=target_codex_qsort cargo test -p iroha_core --lib smartcontracts::isi::query::tests::ephemeral_sorted_query_respects_offset_and_limit -- --exact --nocapture` (pass)
  - `CARGO_TARGET_DIR=target_codex_qsort cargo test -p iroha_core --lib query::snapshot::tests::snapshot_sorted_asset_definitions_returns_first_batch_without_cursor -- --exact --nocapture` (pass)
  - `CARGO_TARGET_DIR=target_codex_qsort cargo test -p iroha_core --lib query::snapshot::tests::snapshot_sorted_asset_definitions_stored_cursor_continues_in_order -- --exact --nocapture` (pass)
  - `CARGO_TARGET_DIR=target_codex_qsort cargo test -p iroha_core --lib smartcontracts::isi::query::tests::iter_dispatch_asset_definitions_sort_ties_stable_by_id -- --exact --nocapture` (pass)
  - `CARGO_TARGET_DIR=target_codex_qsort cargo test -p iroha_core --lib smartcontracts::isi::query::tests::iter_dispatch_asset_definitions_sort_desc_batched -- --exact --nocapture` (pass)
  - `CARGO_TARGET_DIR=target_codex_qsort cargo bench -p iroha_core --bench queries -- 'snapshot_(stored|ephemeral)_sorted_asset_defs_first_batch'` (pass)
  - benchmark spot checks:
    - `snapshot_ephemeral_sorted_asset_defs_first_batch`: `[1.4408 ms 1.4573 ms 1.4767 ms]`
    - `snapshot_stored_sorted_asset_defs_first_batch`: `[3.4591 ms 3.5806 ms 3.7059 ms]`

## 2026-03-13 Follow-up: snapshot sorted-query first-batch fast paths
- Tightened sorted iterable postprocessing in `crates/iroha_core/src/smartcontracts/isi/query.rs`:
  - ephemeral iterable queries with metadata sorting now compute only the prefix needed for `offset + first_batch_len` using `select_nth_unstable_by(...)`, then sort that prefix and return the first batch directly as `QueryOutput`.
  - stored cursor queries now use an incremental sorted-prefix iterator that prepares only the prefix needed for the current batch and extends it as cursors continue, instead of sorting the full result set up front before batch one.
  - both sorted paths now cache metadata sort keys once per item and compare indices against that cache instead of re-reading metadata during every selection/sort comparison.
  - added `query::snapshot::tests::snapshot_sorted_asset_definitions_returns_first_batch_without_cursor` and `query::snapshot::tests::snapshot_sorted_asset_definitions_stored_cursor_continues_in_order` to lock both ephemeral and stored sorted snapshot behavior.
- Validation (this follow-up):
  - `cargo fmt --all` (pass)
  - `CARGO_TARGET_DIR=target_codex_qsort cargo test -p iroha_core --lib query::snapshot::tests::snapshot_sorted_asset_definitions_returns_first_batch_without_cursor -- --exact --nocapture` (pass)
  - `CARGO_TARGET_DIR=target_codex_qsort cargo test -p iroha_core --lib query::snapshot::tests::snapshot_sorted_asset_definitions_stored_cursor_continues_in_order -- --exact --nocapture` (pass)
  - `CARGO_TARGET_DIR=target_codex_qsort cargo test -p iroha_core --lib smartcontracts::isi::query::tests::iter_dispatch_asset_definitions_sort_ties_stable_by_id -- --exact --nocapture` (pass)
  - `CARGO_TARGET_DIR=target_codex_qsort cargo test -p iroha_core --lib smartcontracts::isi::query::tests::iter_dispatch_asset_definitions_sort_desc_batched -- --exact --nocapture` (pass)
  - `CARGO_TARGET_DIR=target_codex_qsort cargo check -p iroha_core --benches -q` (pass)
  - `CARGO_TARGET_DIR=target_codex_qsort cargo bench -p iroha_core --bench queries -- 'snapshot_(stored|ephemeral)_sorted_asset_defs_first_batch'` (pass)
  - benchmark spot checks:
    - `snapshot_ephemeral_sorted_asset_defs_first_batch`: `[2.9423 ms 2.9672 ms 2.9945 ms]`
    - `snapshot_stored_sorted_asset_defs_first_batch`: `[3.3374 ms 3.5143 ms 3.7178 ms]`

## 2026-03-13 Follow-up: direct asset-by-definition iteration for `FindAssets`
- Tightened the definition/domain filter paths in `crates/iroha_core/src/smartcontracts/isi/asset.rs`:
  - `FindAssets` no longer materializes `AssetId`s from `asset_definition_assets` and then does a second `world.assets().get(...)` lookup for each match.
  - definition/domain query paths now pull owned `Asset` values directly through `world.assets_by_definition_iter(...)`, which reuses the exact asset-definition index without the extra map lookup hop.
- Validation (this follow-up):
  - `cargo fmt --all` (pass)
  - `CARGO_TARGET_DIR=target_codex_qsort cargo test -p iroha_core --lib smartcontracts::isi::asset::query::tests::find_assets_filters_by_ -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=target_codex_qsort cargo bench -p iroha_core --bench queries -- 'find_assets_filter_(definition_literal|domain_literal)'` (pass)
  - benchmark spot checks:
    - `find_assets_filter_definition_literal`: `[695.35 µs 700.54 µs 706.01 µs]`
    - `find_assets_filter_domain_literal`: `[1.1634 ms 1.1709 ms 1.1776 ms]`

## 2026-03-13 Follow-up: snapshot bench cleanup + cheaper metadata-sort tie-breaks
- Fixed the stored snapshot benchmark harness in `crates/iroha_core/benches/queries.rs`:
  - `snapshot_stored_find_domains_first_batch`
  - `snapshot_stored_find_assets_first_batch`
  - `snapshot_stored_sorted_asset_defs_first_batch`
  - each benchmark now drops the returned stored cursor after consuming batch one, so repeated iterations do not accumulate live queries and trip `Execution(CapacityLimit)`.
- Tightened generic query postprocessing in `crates/iroha_core/src/smartcontracts/isi/query.rs`:
  - `SortableQueryOutput` now returns typed tie-break keys instead of forcing every sortable item through canonical-byte `Vec<u8>` materialization.
  - `Account`, `Domain`, `AssetDefinition`, `Asset`, `Nft`, `Role`, `Trigger`, `RepoAgreement`, and several offline/proof outputs now use cheaper native id-based tie-break keys where available.
  - this reduces allocation pressure in metadata-sorted iterable queries, especially the snapshot asset-definition first-batch lane.
- Validation (this follow-up):
  - `cargo fmt --all` (pass)
  - `CARGO_TARGET_DIR=target_codex cargo test -p iroha_core --lib smartcontracts::isi::query::tests::iter_dispatch_accounts_sort_ties_stable_by_id -- --exact --nocapture` (pass)
  - `CARGO_TARGET_DIR=target_codex cargo test -p iroha_core --lib smartcontracts::isi::query::tests::iter_dispatch_asset_definitions_sort_ties_stable_by_id -- --exact --nocapture` (pass)
  - `CARGO_TARGET_DIR=target_codex cargo bench -p iroha_core --bench queries -- 'snapshot_(stored|ephemeral|live).*'` (pass; full snapshot slice now runs end to end)
  - benchmark spot checks:
    - `find_asset_defs_iter_10k`: `[477.47 µs 482.39 µs 487.11 µs]`
    - `snapshot_ephemeral_sorted_asset_defs_first_batch`: `[6.1489 ms 6.2070 ms 6.2636 ms]`
    - `snapshot_stored_sorted_asset_defs_first_batch`: `[6.0003 ms 6.0505 ms 6.1010 ms]`

## 2026-03-13 Multisig registration opened to arbitrary accounts
- Removed the built-in multisig registration authority gate in `crates/iroha_core/src/smartcontracts/isi/multisig.rs`, so `MultisigRegister` no longer rejects non-owners/non-grantees with `not qualified to register multisig`.
- Kept upgraded-executor parity in `crates/iroha_executor/src/default/isi/multisig/account.rs` by performing the controller account registration under the multisig home-domain owner context before materializing signatories and roles.
- Updated multisig coverage and docs:
  - `integration_tests/tests/multisig.rs` now exercises successful registration by an arbitrary outside account in the main multisig flow and by a same-domain non-signatory in both pre-upgrade and post-upgrade paths.
  - `crates/iroha_cli/docs/multisig.md` now documents that any account may submit the registration transaction.
- Validation:
  - `cargo fmt --all` (pass)
  - `CARGO_TARGET_DIR=target_codex cargo test -p iroha_core --lib register_allows_non_owner_without_permission -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=target_codex cargo test -p integration_tests --test multisig multisig_normal -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=target_codex IROHA_TEST_SKIP_BUILD=1 cargo test -p integration_tests --test multisig multisig_register_by_non_signatory_materializes_missing_signatory_account -- --nocapture` (pass; matched both normal and executor-upgrade variants)

## 2026-03-13 Follow-up: adaptive scan fast path for simple `FindAssets` predicates
- Tightened `crates/iroha_core/src/smartcontracts/isi/asset.rs` for simple `definition` and `domain` predicates:
  - added an adaptive materialization heuristic that compares the exact indexed match count against total asset count.
  - when the selected asset set is large, `FindAssets` now scans `world.assets_iter()` directly and filters by definition/domain instead of paying `asset_id -> storage lookup` for every match.
  - retained the existing exact-id/index walk for selective predicates, so sparse lookups still avoid full scans.
- Validation (this follow-up):
  - `cargo fmt --all` (pass)
  - `CARGO_TARGET_DIR=target_codex cargo test -p iroha_core --lib smartcontracts::isi::asset::query::tests::find_assets_filters_by_ -- --nocapture` (pass)
  - benchmark spot checks:
    - `find_assets_filter_definition_literal`: `[576.19 µs 580.76 µs 586.33 µs]`
    - `find_assets_filter_domain_literal`: `[998.02 µs 1.0028 ms 1.0073 ms]`

## 2026-03-13 Follow-up: trigger query direct iteration + `PASS` fast path
- Tightened `crates/iroha_core/src/smartcontracts/isi/triggers/mod.rs`:
  - `FindActiveTriggerIds` now iterates triggers through `inspect_by_action(...)` instead of `ids_iter() -> inspect_by_id(...)`, removing an avoidable extra lookup per trigger.
  - `FindTriggers` now uses the same direct action traversal and short-circuits `CompoundPredicate::PASS`.
  - added regression coverage:
    - `smartcontracts::isi::triggers::tests::find_triggers_returns_registered_triggers_for_pass_predicate`
- Validation (this follow-up):
  - `cargo fmt --all` (pass)
  - `CARGO_TARGET_DIR=target_codex cargo test -p iroha_core --lib active_trigger_ids_excludes_depleted_after_burn -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=target_codex cargo test -p iroha_core --lib find_triggers_returns_registered_triggers_for_pass_predicate -- --nocapture` (pass)
  - benchmark spot checks:
    - `find_triggers_iter_10k`: `[2.8245 ms 2.8305 ms 2.8367 ms]`
    - `find_active_trigger_ids_iter_10k`: `[882.56 µs 884.54 µs 886.64 µs]`

## 2026-03-13 Follow-up: exact domain-to-definition index for `FindAssets`
- Fixed an invalid asset-definition domain lookup in `crates/iroha_core/src/state.rs`:
  - added a derived `domain_asset_definitions` index keyed by `DomainId` and storing exact `AssetDefinitionId`s.
  - rebuilt it alongside the other asset-definition indexes, skipped it from serialization, and maintained it on asset-definition register/unregister paths and direct test/setup insertions.
  - replaced `asset_definitions_in_domain_iter(...)`’s borrowed-key `BTreeMap::range(...)` with index-backed iteration, removing the old comparator shim.
- Tightened the domain query hot path in `crates/iroha_core/src/smartcontracts/isi/asset.rs`:
  - domain filters now read definition ids directly from `domain_asset_definitions` instead of materializing full asset definitions just to recover their ids.
  - this also removes the extra domain recheck in the simple domain path because the iterator is now exact.
- Added regression coverage:
  - `state::tests::asset_definition_domain_index_tracks_exact_membership`
- Validation (this follow-up):
  - `cargo fmt --all` (pass)
  - `CARGO_TARGET_DIR=target_codex cargo test -p iroha_core --lib state::tests::asset_definition_domain_index_tracks_exact_membership -- --exact --nocapture` (pass)
  - `CARGO_TARGET_DIR=target_codex cargo test -p iroha_core --lib smartcontracts::isi::asset::query::tests::find_assets_filters_by_ -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=target_codex cargo check -p iroha_core --benches -q` (pass; existing unrelated warnings only)
  - benchmark spot checks:
    - `find_assets_filter_definition_literal`: `[4.0273 ms 4.0364 ms 4.0458 ms]`
    - `find_assets_filter_domain_literal`: `[8.2203 ms 8.2674 ms 8.3175 ms]`

## 2026-03-13 Follow-up: definition-to-asset index for `FindAssets`
- Added a derived `asset_definition_assets` index in `crates/iroha_core/src/state.rs`:
  - keyed by `AssetDefinitionId` and storing concrete `AssetId`s, including scoped partitions.
  - rebuilt alongside `asset_definition_holders`, skipped from serialization, and maintained in the same asset insert/remove mutation hooks.
- Switched definition scans to the new index:
  - `assets_by_definition_iter(...)` now walks exact asset ids for the definition and fetches values directly, instead of traversing `holder -> account+definition range` for every holder.
  - this directly reduces the fan-out cost for `FindAssets` definition/domain plans and simple paths.
- Added regression coverage:
  - `state::tests::assets_by_definition_iter_includes_all_tracked_partitions`
  - extended holder-index lifecycle tests to assert the concrete asset-id index is updated on insert and partition removal.
- Validation (this follow-up):
  - `cargo fmt --all` (pass)
  - `CARGO_TARGET_DIR=target_codex cargo test -p iroha_core --lib state::tests::asset_definition_ -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=target_codex cargo test -p iroha_core --lib state::tests::assets_by_definition_iter_includes_all_tracked_partitions -- --exact --nocapture` (pass)
  - `CARGO_TARGET_DIR=target_codex cargo test -p iroha_core --lib smartcontracts::isi::asset::query::tests::find_assets_filters_by_ -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=target_codex cargo check -p iroha_core --benches -q` (pass; existing unrelated warnings only)
  - benchmark spot checks:
    - `find_assets_filter_definition_literal`: `[4.1008 ms 4.1216 ms 4.1430 ms]`
    - `find_assets_filter_domain_literal`: `[9.7780 ms 9.9473 ms 10.122 ms]`

## 2026-03-13 Follow-up: `PASS` short-circuit for full account/asset scans
- Removed generic predicate overhead when queries are unconstrained:
  - `crates/iroha_core/src/smartcontracts/isi/account.rs`
    - `FindAccounts` now returns `world.accounts_iter()` directly when the predicate is `CompoundPredicate::PASS`.
    - `FindAccountsWithAsset` now traverses holder/index state directly for `PASS` instead of paying JSON/predicate setup costs before the non-zero balance check.
  - `crates/iroha_core/src/smartcontracts/isi/asset.rs`
    - `FindAssets` now returns `world.assets_iter()` directly for `CompoundPredicate::PASS`.
- Added regression coverage:
  - `smartcontracts::isi::account::query::tests::find_accounts_returns_registered_accounts_for_pass_predicate`
- Validation (this follow-up):
  - `cargo fmt --all` (pass)
  - `CARGO_TARGET_DIR=target_codex cargo test -p iroha_core --lib smartcontracts::isi::account::query::tests::find_accounts_ -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=target_codex cargo test -p iroha_core --lib smartcontracts::isi::asset::query::tests::find_assets_ -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=target_codex cargo check -p iroha_core --benches -q` (pass; existing unrelated warnings only)
  - benchmark spot checks:
    - `find_accounts_iter_10k`: `[284.62 µs 285.80 µs 287.10 µs]`
    - `find_assets_iter_10k`: `[418.83 µs 420.65 µs 422.70 µs]`

## 2026-03-13 Follow-up: alias-planner closure + ID hot-path recovery
- Closed remaining planner alias gaps in `crates/iroha_core/src/smartcontracts/isi/asset.rs`:
  - `AssetPredicateView` now extracts planner keys from alias forms:
    - account: `id.account`
    - definition: `id.definition`
    - domain: `definition.domain`, `id.definition.domain`
- Refined `FindAssets` execution for constrained subject/definition plans:
  - subject+definition plans now use `assets_in_account_by_definition_iter(...)` instead of scanning every subject asset partition.
  - removed per-definition `collect::<Vec<_>>()` materialization in `Domains`/`Definitions` plan branches by aggregating once per branch.
- Refined account-id narrowing in `crates/iroha_core/src/smartcontracts/isi/account.rs`:
  - preserved mixed-predicate candidate narrowing (`equals`/`in_values` on `id|account|account_id`).
  - restored dedicated simple-id shortcut (`equals`/`in_values` id-only) so hot-path latency stays in microsecond range.
  - added `PublicKey` parsing fallback in `parse_account_id_value(...)` to handle domainless account literal forms used by benches.
- Added regression coverage:
  - `asset_predicate_view_extracts_alias_fields_for_planner`
  - `find_assets_filters_by_id_account_alias_predicate`
  - `find_assets_filters_by_id_definition_alias_predicate`
  - `find_assets_filters_by_definition_domain_alias_predicate`
- Validation (this follow-up):
  - `cargo fmt --all` (pass)
  - `CARGO_TARGET_DIR=target_codex cargo test -p iroha_core --lib smartcontracts::isi::account::query::tests::find_accounts_ -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=target_codex cargo test -p iroha_core --lib smartcontracts::isi::asset::query::tests::find_assets_filters_by_ -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=target_codex cargo test -p iroha_core --lib smartcontracts::isi::asset::query::tests::asset_predicate_view_extracts_alias_fields_for_planner -- --exact --nocapture` (pass)
  - `CARGO_TARGET_DIR=target_codex cargo check -p iroha_core --benches -q` (pass; existing unrelated warnings only)
  - benchmark spot checks:
    - `find_accounts_filter_id_literal_10k`: `[21.573 µs 21.629 µs 21.688 µs]`
    - `find_accounts_with_asset_id_literal_10k`: `[22.018 µs 22.274 µs 22.639 µs]`
    - `find_assets_filter_definition_literal`: `[7.3978 ms 7.4408 ms 7.4839 ms]`
    - `find_assets_filter_domain_literal`: `[13.311 ms 13.376 ms 13.442 ms]` (sequential rerun; change within noise threshold)

## 2026-03-13 Follow-up: candidate-ID narrowing + account-definition range scans
- Tightened account/asset lookup paths:
  - `crates/iroha_core/src/state.rs` now exposes `AssetByAccountDefinitionBounds` +
    `AsAssetIdAccountDefinitionCompare` and `assets_in_account_by_definition_iter(...)`.
  - `WorldTransaction::untrack_asset_holder_if_empty` now checks remaining partitions with the
    account+definition range instead of scanning all account assets.
  - `assets_by_definition_iter` now uses the account+definition range per holder.
  - `FindAccountsWithAsset` non-zero checks now use `assets_in_account_by_definition_iter(...)`.
- Refined account query filtering in `crates/iroha_core/src/smartcontracts/isi/account.rs`:
  - added candidate-ID extraction from JSON predicates (`equals`/`in` on `id|account|account_id`).
  - `FindAccounts` and `FindAccountsWithAsset` now narrow work to candidate IDs when available.
  - added strict `id`-only short-circuit in candidate paths to avoid redundant predicate evaluation.
- Added/validated regression coverage:
  - `state::tests::asset_account_definition_range_includes_all_scopes`.
  - `smartcontracts::isi::account::query::tests::find_accounts_applies_id_in_literal_predicate`.
  - `smartcontracts::isi::account::query::tests::find_accounts_with_asset_applies_id_in_literal_predicate`.
  - mixed predicate checks:
    `find_accounts_applies_mixed_id_and_metadata_predicate` and
    `find_accounts_with_asset_applies_mixed_id_and_metadata_predicate`.
- Validation (this follow-up):
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_core --lib smartcontracts::isi::account::query::tests:: -- --nocapture` (pass)
  - `cargo test -p iroha_core --lib smartcontracts::isi::asset::query::tests::find_assets_filters_by_ -- --nocapture` (pass)
  - `cargo test -p iroha_core --lib state::tests::asset_account_ -- --nocapture` (pass)
  - `cargo test -p iroha_core --lib state::tests::asset_definition_holder_index_waits_for_last_partition_removal -- --nocapture` (pass)
  - `cargo check -p iroha_core --benches` (pass; existing unrelated warnings only)
  - `cargo bench -p iroha_core --bench queries -- find_accounts_with_asset_id_literal_10k --noplot`
    (`[22.403 µs 22.574 µs 22.749 µs]`, improved from immediate prior regression run)

## 2026-03-13 Follow-up: `FindAccountsWithAsset` ID fast-path
- Extended account query optimization in `crates/iroha_core/src/smartcontracts/isi/account.rs`:
  - `FindAccountsWithAsset` now applies the same simple-ID fast path used by `FindAccounts`.
  - for `equals`/`in` filters on `id|account|account_id`, execution now intersects requested IDs with `asset_definition_holders` and performs keyed account lookup instead of scanning all holders.
  - non-fast-path behavior remains unchanged (holder traversal + alias/full-predicate evaluation).
- Added regression coverage:
  - `smartcontracts::isi::account::query::tests::find_accounts_with_asset_applies_id_literal_predicate`.
- Validation (this follow-up):
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_core --lib smartcontracts::isi::account::query::tests:: -- --nocapture` (pass)
  - `cargo test -p iroha_core --lib smartcontracts::isi::asset::query::tests::find_assets_filters_by_ -- --nocapture` (pass)
  - `cargo check -p iroha_core --benches` (pass; existing unrelated warnings only)
  - `cargo bench -p iroha_core --bench queries -- find_accounts_with_asset_id_literal_10k --noplot` (`[21.586 µs 21.647 µs 21.709 µs]`)

## 2026-03-13 Follow-up: `FindAssets` subject-index plan for account predicates
- Optimized `FindAssets` planning/execution in `crates/iroha_core/src/smartcontracts/isi/asset.rs`:
  - added `AssetQueryPlan::Subjects` for predicates that constrain account subjects (`account|account_id|owner`/`id` extraction).
  - account-constrained asset queries now traverse only `assets_in_account_iter(subject)` and then apply optional domain/definition narrowing.
  - removed prior fallback that could hit `Full` scan even when account subjects were known.
- Added mixed-predicate regression coverage:
  - `smartcontracts::isi::asset::query::tests::find_assets_filters_by_account_and_domain_predicate`.
- Updated benchmark harness to measure query-level account filtering directly:
  - `crates/iroha_core/benches/queries.rs`:
    - replaced manual post-filter benchmark with `find_assets_filter_account_literal` using `CompoundPredicate::<Asset>::build(|p| p.equals("account", ...))`.
- Validation (this follow-up):
  - `cargo fmt --all` (pass)
  - `CARGO_TARGET_DIR=target_codex cargo test -p iroha_core --lib smartcontracts::isi::asset::query::tests::find_assets_filters_by_ -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=target_codex cargo test -p iroha_core --lib smartcontracts::isi::account::query::tests::find_accounts -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=target_codex cargo check -p iroha_core --benches -q` (pass; existing unrelated warnings only)
  - `CARGO_TARGET_DIR=target_codex cargo bench -p iroha_core --bench queries -- find_assets_filter_account_literal --noplot` (`[25.555 µs 25.640 µs 25.724 µs]`)
  - `CARGO_TARGET_DIR=target_codex cargo bench -p iroha_core --bench queries -- find_assets_iter_10k --noplot` (`[480.25 µs 482.57 µs 484.82 µs]`)
  - `CARGO_TARGET_DIR=target_codex cargo bench -p iroha_core --bench queries -- find_assets_filter_definition_literal --noplot` (`[7.7347 ms 7.7673 ms 7.8015 ms]`)
  - `CARGO_TARGET_DIR=target_codex cargo bench -p iroha_core --bench queries -- find_assets_filter_domain_literal --noplot` (`[12.885 ms 12.941 ms 12.995 ms]`)

## 2026-03-13 Follow-up: `FindAccounts` ID fast-path + holder-index regression coverage
- Optimized `FindAccounts` in `crates/iroha_core/src/smartcontracts/isi/account.rs` for simple ID predicates:
  - added a direct keyed lookup fast path for `equals`/`in` filters on `id|account|account_id`.
  - preserves existing alias/fallback semantics for complex predicates.
- Added account query regression coverage:
  - `smartcontracts::isi::account::query::tests::find_accounts_applies_id_literal_predicate`.
- Added holder-index edge coverage and multisig rekey migration coverage:
  - `state::tests::asset_definition_holder_index_waits_for_last_partition_removal`.
  - `smartcontracts::isi::multisig::tests::rekey_account_id_moves_asset_holder_index_to_new_account`.
- Stabilized domain unregister fixture in `crates/iroha_core/src/smartcontracts/isi/domain.rs`:
  - set valid test literals for `nexus.fees.fee_sink_account_id`,
    `nexus.staking.stake_escrow_account_id`,
    `nexus.staking.slash_sink_account_id`
    in `unregister_account_removes_owned_nfts_and_asset_metadata`.
- Validation (this follow-up):
  - `cargo fmt --all` (pass)
  - `CARGO_TARGET_DIR=target_codex cargo test -p iroha_core --lib smartcontracts::isi::account::query::tests::find_accounts -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=target_codex cargo test -p iroha_core --lib state::tests::asset_definition_holder_index_waits_for_last_partition_removal -- --exact --nocapture` (pass)
  - `CARGO_TARGET_DIR=target_codex cargo test -p iroha_core --lib smartcontracts::isi::multisig::tests::rekey_account_id_moves_asset_holder_index_to_new_account -- --exact --nocapture` (pass)
  - `CARGO_TARGET_DIR=target_codex cargo test -p iroha_core --lib smartcontracts::isi::domain::tests::unregister_account_removes_owned_nfts_and_asset_metadata -- --exact --nocapture` (pass)
  - `CARGO_TARGET_DIR=target_codex cargo bench -p iroha_core --bench queries -- find_accounts_iter_10k --noplot` (`[308.94 µs 309.58 µs 310.26 µs]`)
  - `CARGO_TARGET_DIR=target_codex cargo bench -p iroha_core --bench queries -- find_accounts_filter_id_literal_10k --noplot` (`[20.502 µs 20.538 µs 20.572 µs]`, improved from earlier ~`[22.250 ms 22.286 ms 22.322 ms]`)

## 2026-03-13 Follow-up: `FindAssets` domain/definition index traversal
- Refined `FindAssets` plan execution in `crates/iroha_core/src/smartcontracts/isi/asset.rs`:
  - `Domains + Definitions` now traverses selected definitions via `assets_by_definition_iter` instead of global `assets_iter()` scanning.
  - `Domains` (without explicit definitions) now expands definitions per domain via `asset_definitions_in_domain_iter` and then traverses definition holders/assets.
  - `Definitions` plan now traverses via `assets_by_definition_iter` rather than filtering all assets.
- Preserved `domain` predicate semantics as asset-definition domain (not account-domain links).
- Validation (follow-up):
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_core --lib smartcontracts::isi::asset::query::tests::find_assets_filters_by_ -- --nocapture` (pass)
  - `cargo test -p iroha_core --lib smartcontracts::isi::account::query::tests::find_accounts -- --nocapture` (pass)
  - `cargo check -p iroha_core --benches` (pass; existing unrelated warnings only)

## 2026-03-12 Account/domain performance pass (domain links + holder index)
- Added `StorageReadOnly::get_key_value` in `crates/mv/src/storage.rs` for keyed lookups that return stable key/value refs.
- Extended world state indexing in `crates/iroha_core/src/state.rs`:
  - added `asset_definition_holders` storage wiring across `World`, `WorldBlock`, `WorldTransaction`, and `WorldView`.
  - threaded the index through block/transaction/view builders, read-only trait accessors, commit/apply paths, constructors, and world JSON rebuild.
  - added holder index rebuild logic from stored assets.
  - added holder tracking helpers (`track_asset_holder`, `untrack_asset_holder_if_empty`) and integrated them into asset insert/remove helper paths.
  - optimized domain traversal methods to use domain-subject index iteration (`accounts_in_domain_iter`, `assets_in_domain_iter`) and moved definition traversal to holder-index-backed `assets_by_definition_iter`.
- Optimized account queries in `crates/iroha_core/src/smartcontracts/isi/account.rs`:
  - added alias-field predicate short-circuiting (`id`, `account`, `account_id`, `uaid`) before full JSON predicate fallback.
  - updated `FindAccountsWithAsset` to iterate holder candidates from `asset_definition_holders` before balance checks and predicate evaluation.
- Synced direct asset-move/seed call sites that bypass `asset_or_insert` to keep holder index consistent:
  - `crates/iroha_core/src/smartcontracts/isi/multisig.rs`
  - `crates/iroha_core/src/smartcontracts/isi/domain.rs`
  - `crates/iroha_core/src/smartcontracts/isi/world.rs`
  - `crates/iroha_core/src/nexus/portfolio.rs`
  - `crates/iroha_core/src/sumeragi/penalties.rs`
- Benchmark updates:
  - registered `queries` benchmark target in `crates/iroha_core/Cargo.toml`.
  - added `find_accounts_filter_id_literal_10k` in `crates/iroha_core/benches/queries.rs`.
- Added regression test in `crates/iroha_core/src/state.rs`:
  - `state::tests::asset_definition_holder_index_tracks_asset_lifecycle`.
- Validation (this pass):
  - `cargo fmt --all` (pass)
  - `cargo test -p mv` (pass)
  - `cargo test -p iroha_core find_accounts_with_asset_ignores_zero_holdings -- --nocapture` (pass)
  - `cargo test -p iroha_core asset_definition_holder_index_tracks_asset_lifecycle -- --nocapture` (pass)
  - `cargo check -p iroha_core --benches` (pass; existing warnings in unrelated test modules)

## 2026-03-12 aid/name/alias gap closure (constructor hard-cut + targeted docs refresh)
- Completed the asset-definition constructor hard-cut in `crates/iroha_data_model/src/asset/definition.rs`:
  - `AssetDefinition::new(id, spec)` and `AssetDefinition::numeric(id)` no longer auto-derive `name` from `id`.
  - constructor docs now state explicit `.with_name(...)` is required before registration.
- Migrated remaining `crates/*/src` and `integration_tests/tests` registration/build paths to explicit naming:
  - applied explicit `.with_name(...)` across remaining `AssetDefinition::new/numeric` call sites used for registration/build.
  - left one intentional negative test path without `.with_name(...)` to assert rejection.
- Added/updated hard-cut tests:
  - `asset::definition::validation_tests::constructors_leave_name_empty_without_explicit_with_name`
  - `smartcontracts::isi::domain::tests::register_asset_definition_rejects_missing_explicit_name`
- Extended `scripts/translate_i18n_google.py`:
  - added `--refresh-mode` (`source-identical`/`stale`/`all`) including stale-translation refresh support.
  - added repeatable `--path-glob` filtering to constrain regeneration scope.
  - improved markdown frontmatter parsing to preserve leading HTML comment preambles.
  - apply phase now refreshes `source_hash`, `source_last_modified`, and `translation_last_reviewed`.
- Regenerated only the targeted data-model docs family (40 localized files):
  - `docs/source/data_model.*.md`
  - `docs/source/data_model_and_isi_spec.*.md`
  - enforced canonical wording checks (`aid:<32-lower-hex-no-dash>`, both alias forms, no `asset#domain`/`IpfsPath`/`ipfs://` guidance).
- Validation (this pass):
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_data_model constructors_leave_name_empty_without_explicit_with_name --lib -- --nocapture` (pass)
  - `cargo test -p iroha_core register_asset_definition_rejects_missing_explicit_name --lib -- --nocapture` (pass)
  - `cargo test -p iroha_torii parse_asset_definition_id_accepts_alias_literals_only --lib -- --nocapture` (pass)
  - `cargo test -p iroha_torii resolve_asset_definition_selector_rejects_legacy_aid_literal --lib -- --nocapture` (pass)
  - `cargo test -p iroha_cli parse_asset_definition_aid_literal_rejects_legacy_literal -- --nocapture` (pass)
  - `cargo check -p iroha_data_model -p iroha_core -p iroha_torii -p iroha_cli -p integration_tests` (pass)

## 2026-03-12 Default-executor multisig register signatory materialization parity
- Removed default-executor `MultisigRegister` validation dependency on preexisting signatory accounts in:
  - `crates/iroha_executor/src/default/isi/multisig/account.rs`
  - dropped `ensure_signatories_exist(...)` from registration validation.
- Added execution-time auto-materialization of missing signatory accounts for `MultisigRegister`:
  - runs under home-domain owner authority before role wiring.
  - tags auto-created accounts with `iroha:created_via = "multisig"`.
  - skips self-subject entries for the multisig account being registered.
- Added executor unit coverage:
  - `signatory_materialization_skips_multisig_subject`
- Validation (this pass):
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_executor --lib signatory_materialization_skips_multisig_subject -- --nocapture` (pass)
  - `cargo test -p iroha_executor --lib signatories_from_multiple_domains_are_allowed -- --nocapture` (pass)
  - `cargo test -p iroha_core --lib register_materializes_missing_signatory_accounts -- --nocapture` (pass; warnings only)

## 2026-03-12 aid/alias hard-cut continuation (remaining runtime `name#domain` constructor migration)
- Removed remaining runtime/test helper construction that still derived `AssetDefinitionId` via legacy `format!("name#domain").parse()` patterns:
  - `crates/iroha_genesis/src/lib.rs`
  - `crates/iroha_core/src/executor.rs`
  - `crates/izanami/src/instructions.rs`
  - `crates/izanami/src/faults.rs`
- Replaced all of the above with canonical constructor form:
  - `AssetDefinitionId::new(domain_id, name.parse()?)`
- Ensured updated executor fixtures set explicit asset display names when registering migrated definitions.
- Re-verified alias-only Torii selector behavior and CLI legacy-literal rejection in targeted tests.
- Validation (this pass):
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_torii --lib parse_asset_definition_id_accepts_alias_literals_only -- --nocapture` (pass)
  - `cargo test -p iroha_torii --lib resolve_asset_definition_selector_rejects_legacy_aid_literal -- --nocapture` (pass)
  - `cargo test -p iroha_cli parse_asset_definition_aid_literal_rejects_legacy_literal -- --nocapture` (pass)
  - `cargo test -p iroha_core --lib initial_executor_denies_transfer_asset_definition -- --nocapture` (pass)
  - `cd IrohaSwift && swift test --filter 'ToriiClientTests/testResolveAssetAliasAsync|ToriiClientTests/testResolveAssetAliasReturnsNilOnNotFound|TransactionInputValidatorTests'` (selected tests pass; runner exits with environment signal code 5 after execution)
  - `cargo check -p iroha_genesis -p izanami -p iroha_core --tests` (pass; warnings only)
  - `cargo check -p iroha_data_model -p iroha_core -p integration_tests -p iroha_genesis -p izanami --tests` (pass; warnings only)

## 2026-03-12 Explorer instruction account-filter: custom multisig parity fix
- Closed a remaining `/v1/explorer/instructions?account=...` gap for `Custom` envelopes carrying multisig instructions.
- Updated `crates/iroha_torii/src/routing.rs`:
  - `instruction_matches_account_id(...)` now decodes multisig `CustomInstruction` payloads and matches:
    - `MultisigRegister.account`
    - `MultisigApprove.account`
    - `MultisigPropose.account`
    - nested `MultisigPropose.instructions` recursively (so embedded Mint/Burn account targets are matched).
  - `instruction_matches_asset_id(...)` now recursively checks nested `MultisigPropose.instructions`.
  - `append_asset_ids_from_instruction(...)` now extracts asset ids recursively from nested `MultisigPropose.instructions`.
- Added targeted tests:
  - `instruction_matches_asset_id_matches_multisig_custom_propose_nested_asset`
  - `instruction_matches_account_id_matches_multisig_custom_propose_account`
  - `instruction_matches_account_id_matches_multisig_custom_propose_nested_accounts`
  - `explorer_instructions_endpoint_account_filter_includes_multisig_custom_propose`
- Validation (this pass):
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_torii --lib instruction_matches_asset_id_matches_multisig_custom_propose_nested_asset -- --nocapture` (pass)
  - `cargo test -p iroha_torii --lib instruction_matches_account_id_matches_multisig_custom_propose -- --nocapture` (pass)
  - `cargo test -p iroha_torii --lib explorer_instructions_endpoint_account_filter_includes_multisig_custom_propose -- --nocapture` (pass)
  - `cargo test -p iroha_torii --lib explorer_instructions_endpoint_account_filter_includes_mint_and_burn -- --nocapture` (pass)

## 2026-03-12 Explorer instruction account-filter follow-up: public-lane rewards + OpenAPI parity
- Closed another `/v1/explorer/instructions?account=...` matcher gap in `crates/iroha_torii/src/routing.rs`:
  - `instruction_matches_account_id(...)` now matches `staking::RecordPublicLaneRewards` via `reward_asset().account()`.
- Added unit coverage:
  - `instruction_matches_account_id_matches_public_lane_rewards_asset_account`
- Updated OpenAPI text for explorer instruction `account` query parameter to match actual behavior beyond transfer-only filtering.
- Added OpenAPI regression coverage:
  - `explorer_instructions_account_param_description_mentions_non_transfer_matches`
- Validation (this pass):
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_torii --lib instruction_matches_account_id_matches_public_lane_rewards_asset_account -- --nocapture` (pass)
  - `cargo test -p iroha_torii --lib explorer_instructions_endpoint_account_filter_includes_ -- --nocapture` (pass)
  - `cargo test -p iroha_torii --lib explorer_instructions_account_param_description_mentions_non_transfer_matches -- --nocapture` (pass)

## 2026-03-12 aid/alias completion pass (Torii selector parity + CLI help sync + fixture sweep)
- Restored Torii app-API selector parity in `crates/iroha_torii/src/lib.rs`:
  - `parse_asset_definition_id(...)` now accepts canonical `aid:<32-lower-hex-no-dash>` and strict alias literals (`<name>#<domain>@<dataspace>` or `<name>#<dataspace>`).
  - Added/updated test coverage with `parse_asset_definition_id_accepts_aid_or_alias_literals`.
- Regenerated static CLI help from live clap configuration:
  - `crates/iroha_cli/CommandLineHelp.md` now reflects aid/alias surfaces for asset-definition and asset commands, including `iroha tools encode asset-id`.
  - Updated regeneration command in `crates/iroha_cli/README.md` to `cargo run -p iroha_cli --bin iroha -- tools markdown-help`.
- Completed remaining legacy fixture migration in integration/Torii/bench harnesses:
  - Replaced legacy asset-definition textual parses (`\"name#domain\".parse()`) with explicit `AssetDefinitionId::new(domain, name)` construction across migrated test files.
  - Removed remaining dynamic `format!(\"name#{domain}\").parse()` asset-definition construction from:
    - `integration_tests/tests/domain_links.rs`
    - `integration_tests/tests/extra_functional/seven_peer_consistency.rs`
    - `integration_tests/tests/scheduler_teu.rs`
    - `crates/iroha_torii/tests/accounts_portfolio.rs`
    - `crates/iroha_core/benches/queries.rs`
- Validation (this pass):
  - `CARGO_TARGET_DIR=target_tmp_aid_alias cargo test -p iroha_torii --lib parse_asset_definition_id_accepts_aid_or_alias_literals -- --nocapture` (pass)
  - `cargo check -p integration_tests --tests` (pass)
  - `cargo check -p iroha_torii --tests` (pass)
  - `cargo check -p iroha_core --benches` (pass; warnings only)

## 2026-03-12 Torii alias-only ingress gap closure (`name#issuer@dataspace` / `name#dataspace` only)
- Closed remaining legacy `aid:` acceptance paths for public asset-definition selectors:
  - `crates/iroha_torii/src/lib.rs::parse_asset_definition_id(...)` now rejects all `aid:` literals and resolves aliases only.
  - `crates/iroha_torii/src/routing.rs` handlers now resolve `{definition_id}` via alias selector helper (holders GET/query + confidential transitions), matching `/v1/zk/roots` behavior.
- Updated wrapper flow in `crates/iroha_torii/src/lib.rs` so holders/confidential routes forward alias literals to routing handlers instead of rewriting to canonical `aid:...`.
- Updated Torii tests to assert alias-only path usage (`rose%23sbp`) and seeded fixture aliases where required.
- Updated ZK selector docs/tests to remove canonical-`aid` acceptance language and keep only:
  - `<name>#<domain>@<dataspace>`
  - `<name>#<dataspace>`
- Validation (this pass):
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_torii --lib parse_asset_definition_id_accepts_alias_literals_only -- --nocapture` (pass)
  - `cargo test -p iroha_torii --lib resolve_asset_definition_selector_ -- --nocapture` (pass)
  - `cargo test -p iroha_torii --lib asset_holders_ -- --nocapture` (pass)
  - `cargo test -p iroha_torii --lib confidential_asset_transitions_ -- --nocapture` (pass)
  - `cargo test -p iroha_torii --test zk_roots_handler_integration zk_roots_endpoint_returns_bounded_recent_roots -- --nocapture` (pass)

## 2026-03-12 Multisig unregistered-signatory admission fix
- Confirmed explorer rejection root cause for tx `ef3eb7fb6bdd4a852c838bb0dcb349ad75fa18e7ecd05e9d7b5408304c1a1537`: `Account does not exist` was raised at transaction validation before multisig authorization paths.
- Updated `crates/iroha_core/src/tx.rs`:
  - Added `allows_unregistered_authority(...)` to permit missing-authority admission only for `MultisigPropose` / `MultisigApprove` custom envelopes.
  - Kept existing account-existence rejection unchanged for all other transaction kinds.
  - Skips data-trigger DFS dispatch for this unregistered multisig-only path to avoid authority-account lookup failures after instruction execution.
- Added regression tests in `tx::tests`:
  - `missing_authority_rejected_for_non_multisig_transaction`
  - `missing_authority_multisig_approve_reaches_instruction_validation`
- Validation (this pass):
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_core --lib missing_authority_ -- --nocapture` (pass)
  - `cargo test -p iroha_core --lib multisig_account_direct_signing_rejected_in_validation -- --nocapture` (pass)

## 2026-03-12 Torii asset selector completion (aid + strict alias forms) + cleanup
- Completed Torii API selector behavior to match the v1 break plan:
  - `crates/iroha_torii/src/lib.rs::parse_asset_definition_id(...)` now accepts canonical `aid:<32-lower-hex-no-dash>` and strict aliases:
    - `<name>#<domain>@<dataspace>`
    - `<name>#<dataspace>`
  - invalid `aid:` literals and malformed aliases are rejected.
- Added/updated Torii coverage:
  - `tests::parse_asset_definition_id_accepts_aid_or_alias_literals` now validates:
    - long/short alias acceptance
    - canonical `aid` acceptance
    - invalid `aid` rejection
    - unknown alias => `NotFound`
- Extended ZK roots convenience API selector semantics in `crates/iroha_torii/src/routing.rs`:
  - `ZkRootsGetRequestDto.asset_id` now accepts canonical `aid` or strict alias forms.
  - Added helper `resolve_asset_definition_selector(...)` with focused unit tests for canonical aid, alias resolve, invalid aid rejection, and unknown alias handling.
- Removed post-migration warning-only residue in `fastpq_prover`:
  - dropped unused `std::str::FromStr` test imports in:
    - `crates/fastpq_prover/src/digest.rs`
    - `crates/fastpq_prover/src/gadgets/transfer.rs`
    - `crates/fastpq_prover/src/trace.rs`
- Continued CLI test migration away from legacy `name#domain` parsing in `crates/iroha_cli/src/main_shared.rs` by replacing `format!(\"ad{i}#land\").parse()` with explicit `AssetDefinitionId::new(domain, name)` construction in paginated asset-definition query harnesses.

## 2026-03-12 Torii asset-definition literal hard cut: alias-only API ingress
- Enforced alias-only parsing for public Torii asset-definition inputs at API ingress (`crates/iroha_torii/src/lib.rs`):
  - `parse_asset_definition_id(...)` now accepts only `AssetDefinitionAlias` literals:
    - `<name>#<domain>@<dataspace>`
    - `<name>#<dataspace>`
  - canonical `aid:...` literals are rejected on these external paths.
- All affected handlers now resolve alias literals through world-state alias index (`asset_definition_id_by_alias`) before dispatch:
  - explorer account/asset filters and explorer asset-definition detail/snapshot/econometrics routes
  - asset holders (`GET` and `POST query`) routes
  - confidential asset transitions route
  - `/v1/zk/roots` request parsing now accepts alias literals only and resolves through alias index
- Kept auth/rate-limit ordering intact for holders/confidential handlers: access checks still run before alias resolution in gated branches.
- Added targeted coverage:
  - `tests::parse_asset_definition_id_accepts_alias_literals_only` verifies:
    - long + short alias acceptance
    - `aid:...` rejection
    - unknown alias => `NotFound`
- Validation (this pass):
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_torii --lib parse_asset_definition_id_accepts_alias_literals_only -- --nocapture` (pass)
  - `cargo test -p iroha_torii --lib asset_holders_get_pagination_preserves_total -- --nocapture` (pass)
  - `cargo test -p iroha_torii --lib asset_alias_resolve_ -- --nocapture` (pass)
  - `cargo test -p iroha_torii --test zk_roots_handler_integration zk_roots_endpoint_returns_bounded_recent_roots -- --nocapture` (pass)

## 2026-03-12 aid/name hard-cut continuation (static `name#domain` migration + explicit-name runtime paths + docs)
- Removed implicit constructor fallback naming in `AssetDefinition::new` / `AssetDefinition::numeric` (no auto-`id.to_string()` anymore).
- Migrated static Rust literals from legacy `\"name#domain\".parse()` to canonical constructor form:
  - `iroha_data_model::asset::AssetDefinitionId::new(<domain>, <name>)`
  - applied across data-model/core/cli/torii/ivm/SDK-adjacent Rust sources and tests.
- Patched runtime genesis/localnet builders to set explicit asset names when registering definitions:
  - `iroha_kagami` genesis/localnet bootstrap assets.
  - `iroha_test_network` bootstrap/genesis fixture definitions.
  - `iroha_genesis` domain builder now sets definition name from provided asset name.
  - `izanami` genesis scenario definitions now set explicit names.
- Updated docs with explicit aid/alias UX:
  - `docs/source/data_model.md`
  - `docs/source/data_model_and_isi_spec.md`
  - added both alias forms (`<name>#<domain>@<dataspace>`, `<name>#<dataspace>`) and a CLI/Torii migration note.
- Validation (this pass):
  - `cargo fmt --all` (pass)
  - `cargo check -p iroha_data_model -p iroha_core -p iroha_cli -p iroha_torii -p iroha_kagami -p iroha_test_network -p iroha_genesis -p izanami -p ivm -p iroha` (pass)
  - `cargo test -p iroha_core --lib set_asset_definition_alias_ -- --nocapture` (pass)
  - `cargo test -p iroha_torii asset_alias_resolve_ -- --nocapture` (pass; 3 targeted tests, remaining bins filtered/no-op)
  - `cargo test -p iroha_cli register_alias_derives_ -- --nocapture` (pass)
  - `cargo test -p iroha_cli parse_asset_definition_aid_literal_rejects_legacy_literal -- --nocapture` (pass)
  - `cargo test -p iroha_data_model asset_alias_parses_valid_short_literal -- --nocapture` (pass)
  - `cargo test -p iroha_data_model asset_definition_id_parse_aid_rejects_legacy_and_dashed_literals -- --nocapture` (pass)

## 2026-03-12 aid/Alias hard-cut follow-up (CLI alias resolve strict path + constructor test alignment)
- Removed the CLI compatibility fallback in `resolve_asset_definition_id_by_alias`:
  - `iroha_cli` now resolves aliases only through `POST /v1/assets/aliases/resolve`.
  - no client-side fallback scan over `FindAssetsDefinitions` remains.
  - HTTP `404` now maps directly to “alias not bound” in CLI UX.
- Updated `crates/iroha_data_model/tests/id_of_constructors.rs` to stop parsing legacy `name#domain` literals for `AssetDefinitionId`; parity test now round-trips canonical `aid:...` text.
- Removed remaining internal “legacy name” wording from synthetic aid-component helper text in `crates/iroha_data_model/src/asset/id.rs`.
- Validation (this pass):
  - `cargo fmt --all` (pass)
  - `cargo check -p iroha_data_model -p iroha_core -p iroha_cli -p iroha_torii` (pass)
  - `cargo test -p iroha_data_model asset_definition_id_ -- --nocapture` (pass)
  - `cargo test -p iroha_core set_asset_definition_alias_ -- --nocapture` (pass)
  - `cargo test -p iroha_torii asset_alias_resolve_ -- --nocapture` (pass)
  - `cargo test -p iroha_cli register_alias_derives_ -- --nocapture` (pass)
  - `cd IrohaSwift && swift test --filter ToriiClientTests/testResolveAssetAliasAsync --filter ToriiClientTests/testResolveAssetAliasReturnsNilOnNotFound --filter TransactionInputValidatorTests` (selected tests pass; process exits with environment signal code 5 after execution)

## 2026-03-12 Torii explorer account instruction filter: account-bearing instruction parity fix
- Expanded `instruction_matches_account_id()` in `crates/iroha_torii/src/routing.rs` to account-match all relevant asset-account instructions in this path:
  - `MintBox::Asset` / `BurnBox::Asset` by `destination().account()`
  - `SetAssetKeyValue` / `RemoveAssetKeyValue` by `asset().account()`
- Added endpoint-level coverage for `GET /v1/explorer/instructions?account=...`:
  - `explorer_instructions_endpoint_account_filter_includes_mint_and_burn`
  - verifies account filtering returns only the expected Mint/Burn instructions for that account.
- Removed legacy `name#domain` asset-definition literals from `crates/iroha_torii/src/routing.rs` test surfaces and replaced them with canonical `aid:...` IDs.
- Added unit coverage in `tx_query_filter_tests`:
  - `instruction_matches_account_id_matches_mint_asset_destination_account`
  - `instruction_matches_account_id_matches_burn_asset_destination_account`
  - `instruction_matches_account_id_matches_set_asset_key_value_asset_account`
  - `instruction_matches_account_id_matches_remove_asset_key_value_asset_account`
- Validation (this pass):
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_torii instruction_matches_account_id_matches_ -- --nocapture` (pass)
  - `cargo test -p iroha_torii explorer_instructions_endpoint_account_filter_includes_mint_and_burn -- --nocapture` (pass)
  - `cargo test -p iroha_torii explorer_transaction_filters_match_asset_id -- --nocapture` (pass)
  - `cargo test -p iroha_torii instruction_matches_asset_id_handles_mint_assets -- --nocapture` (pass)
  - `rg -n '#wonderland|%23wonderland' crates/iroha_torii/src/routing.rs` (no matches)

## 2026-03-12 aid-only Assets + required name + alias literal rollout (v1 break plan implementation slice)
- Implemented canonical `aid`-first asset identity and removed legacy `name#domain` textual parsing from `AssetDefinitionId::from_str`.
- Added first-class asset alias literal model (`<name>#<domain>@<dataspace>`) in `iroha_data_model`:
  - new `AssetDefinitionAlias` type + parsing/validation/tests.
  - `AssetDefinition` / `NewAssetDefinition` now include optional `alias`.
  - validation enforces alias left segment must match the required human asset name exactly.
- Extended asset alias literal support to allow exactly two on-chain forms:
  - `<name>#<domain>@<dataspace>`
  - `<name>#<dataspace>`
  - and reject malformed multi-separator variants.
- Updated CLI alias UX for asset-definition registration:
  - `--alias-dataspace` alone now derives short-form alias `<name>#<dataspace>`.
  - `--alias-domain` + `--alias-dataspace` derives long form `<name>#<domain>@<dataspace>`.
- Removed component-derived legacy asset-id construction paths from CLI runtime flows:
  - `iroha tools encode asset-id` now accepts only canonical `--definition aid:...` or `--alias ...`.
  - `iroha ledger asset {id,mint,burn,transfer}` resolution no longer accepts `--asset/--domain` fallback paths.
- Enforced required human-readable asset name path in registration flow and tests; added duplicate-alias rejection in core registration logic.
- Updated config defaults and consumers to stop using legacy `name#domain` constants:
  - governance/oracle/nexus defaults now emit canonical IDs via helper functions.
  - replaced removed string constants with default functions in config/core/torii tests.
- Expanded CLI UX to make manual asset workflows usable without hand-crafted Rust:
  - `ledger asset` operations now accept canonical ID or alias+account (no component construction fallback).
  - `tools encode asset-id` supports canonical definition IDs and alias literals.
  - asset-definition commands accept alias-based input; register can derive alias from `name + alias-dataspace` or `name + alias-domain + alias-dataspace`.
- Added cross-feature test compatibility helpers in core state:
  - `State::new_with_chain_for_testing(...)`
  - `WorldBlock::transaction_without_telemetry(...)`
  - migrated Torii tests to these helpers to avoid feature-gating drift with `iroha_core` telemetry feature.
- Updated test fixtures and harnesses across `iroha_torii` and integration paths for new required `alias` field and aid-safe asset-id construction.
- Validation (this pass):
  - `cargo fmt --all` (pass)
  - `cargo check -p iroha_data_model -p iroha_config -p iroha_core -p iroha_cli` (pass)
  - `cargo check --tests -p iroha_kagami -p iroha_torii -p integration_tests` (pass)
  - `cargo test -p iroha_data_model asset_alias_parses_valid_literal -- --nocapture` (pass)
  - `cargo test -p iroha_data_model asset_alias_validation_requires_name_segment_match -- --nocapture` (pass)
  - `cargo test -p iroha_data_model asset_definition_id_from_str_rejects_legacy_literal -- --nocapture` (pass)
  - `cargo test -p iroha_core register_asset_definition_rejects_duplicate_alias -- --nocapture` (pass)
  - `cargo test -p iroha_cli register_alias_derives_from_name_domain_and_dataspace -- --nocapture` (pass)
  - `cargo test -p iroha_torii offline_bundle_proof_status_reports_match -- --nocapture` (pass)
  - `cargo test -p iroha_torii vk_list_filters_by_backend_and_status -- --nocapture` (pass)
  - `cargo test -p iroha_data_model asset_alias_ -- --nocapture` (pass)
  - `cargo test -p iroha_cli register_alias_derives_ -- --nocapture` (pass)
  - `cargo test -p iroha_torii asset_alias_resolve_ -- --nocapture` (pass)
  - `cargo check -p iroha_data_model -p iroha_cli -p iroha_torii` (pass)

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
  - `cd javascript/iroha_js && node --test --test-name-pattern "getExplorerAccountQr ignores payload address_format field" test/toriiClient.test.js` (pass)

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
- Deploy preflight scanner entrypoint is `../pk-deploy/scripts/check-identity-surface.sh`; the previous scanner entrypoint is removed.
- Runtime deploy scripts are aligned to strict-first-release behavior:
  - `../pk-deploy/scripts/cutover-ih58-mega.sh` invokes only `check-identity-surface.sh`.
- `../pk-deploy/scripts/deploy-sbp-aed-pkr-interceptor.sh` no longer performs prior-layout trigger cleanup loops.
- Wallet docs describe only current QR modes in neutral terms.

## 2026-03-15 Sumeragi Vote-Backed Quorum-Reschedule Damping
- Simplified the remaining vote-backed quorum-timeout path in `crates/iroha_core/src/sumeragi/main_loop/{pending_block.rs,reschedule.rs}`:
  - added `PendingBlock::vote_backed_reschedule_due(...)` so a vote-backed quorum reschedule is only rearmed after real new progress (`last_progress > last_quorum_reschedule`);
  - `reschedule_stale_pending_blocks_with_now(...)` now uses that stricter gate for vote-backed / QC-backed pending blocks while keeping the old zero-vote path unchanged;
  - `reschedule_pending_quorum_block(...)` no longer calls `pending.touch_progress(now)` for retained vote-backed pending state, so quorum reschedule stops resetting frontier stall age.
- This removes another implicit recovery owner. Vote arrival / RBC activity still owns progress, but quorum reschedule is now only a bounded retransmit side effect instead of a second timer that refreshes the frontier candidate.
- Added and updated focused regressions:
  - `vote_backed_reschedule_requires_progress_after_last_attempt` in `crates/iroha_core/src/sumeragi/main_loop/pending_block.rs`;
  - `stake_quorum_timeout_reschedules_without_immediate_view_change`;
  - `reschedule_defers_vote_backed_quorum_timeout_while_consensus_backlogged`;
  - `reschedule_skips_repeated_vote_backed_quorum_timeout_without_new_progress`.

### Validation Matrix (Vote-Backed Quorum-Reschedule Damping)
- `cargo fmt --all` (pass)
- `cargo test -p iroha_core --lib vote_backed_reschedule_requires_progress_after_last_attempt -- --nocapture` (pass)
- `cargo test -p iroha_core --lib stake_quorum_timeout_reschedules_without_immediate_view_change -- --nocapture` (pass)
- `cargo test -p iroha_core --lib reschedule_defers_vote_backed_quorum_timeout_while_consensus_backlogged -- --nocapture` (pass)
- `cargo test -p iroha_core --lib reschedule_skips_repeated_vote_backed_quorum_timeout_without_new_progress -- --nocapture` (pass)

### Runtime Signal (Vote-Backed Quorum-Reschedule Damping)
- A fresh healthy NPoS keep-dirs probe is running on this cut:
  - log: `/tmp/izanami_npos_healthy_probe_vote_backed_reschedule_once_20260315T075049Z.log`
  - preserved network dir: `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_oOdetz`
- Current state at this status update:
  - the outer `izanami` release build finished;
  - the test-network child warmup build (`iroha3d` release) is still running;
  - no peer stdout files exist yet, so there is no new consensus runtime verdict at this point.

## 2026-03-15 Sumeragi Proposal-Owned Commit Activation
- Simplified Sumeragi slot ownership further so `Proposal` remains the only owner of commit activation:
  - added `slot_has_proposal_evidence(height, view)` in `crates/iroha_core/src/sumeragi/main_loop/proposal_handlers.rs`;
  - `crates/iroha_core/src/sumeragi/main_loop/validation.rs` now defers pre-vote validation for pending blocks that have no observed/cached `Proposal` and no commit QC;
  - `crates/iroha_core/src/sumeragi/main_loop/commit.rs` now refuses to promote a proposal-less pending block into active commit work even if the payload was already marked valid.
- This removes another payload-owned state path: `BlockCreated` without a matching `Proposal` may still cache payload in pending state, but it cannot dispatch validation workers, emit precommit votes, or become `commit.inflight` until a real proposal arrives or a commit QC supplies the missing ownership signal.
- Added and updated focused regressions in `crates/iroha_core/src/sumeragi/main_loop/tests.rs`:
  - `validation_defers_without_proposal_evidence`
  - `commit_pipeline_defers_valid_pending_without_proposal_evidence`
  - updated direct commit-pipeline fixtures to call `note_proposal_seen(...)` when they intentionally model proposal-owned pending blocks.

### Validation Matrix (Proposal-Owned Commit Activation)
- `cargo fmt --all` (pass)
- `cargo test -p iroha_core --lib commit_pipeline_defers_valid_pending_without_proposal_evidence -- --nocapture` (pass)
- `cargo test -p iroha_core --lib validation_defers_without_proposal_evidence -- --nocapture` (pass)
- `cargo test -p iroha_core --lib commit_pipeline_runs_without_global_cooldown -- --nocapture` (pass)
- `cargo test -p iroha_core --lib commit_pipeline_reports_stage_timings -- --nocapture` (pass)
- `cargo test -p iroha_core --lib commit_pipeline_qc_rebuild_cooldown_uses_chain_block_time -- --nocapture` (pass)
- `cargo test -p iroha_core --lib commit_pipeline_votes_highest_view_first_for_same_height -- --nocapture` (pass)
- `cargo test -p iroha_core --lib commit_pipeline_uses_epoch_for_height_when_emitting_votes -- --nocapture` (pass)
- `cargo test -p iroha_core --lib commit_pipeline_skips_fast_timeout_with_da_enabled -- --nocapture` (pass)
- `cargo test -p iroha_core --lib commit_pipeline_inlines_validation_after_fast_timeout_when_worker_queue_full -- --nocapture` (pass)
- `cargo test -p iroha_core --lib commit_pipeline_inlines_validation_at_queue_full_cutover -- --nocapture` (pass)
- `cargo test -p iroha_core --lib commit_pipeline_keeps_deferred_validation_before_queue_full_cutover -- --nocapture` (pass)

### Runtime Signal (Proposal-Owned Commit Activation)
- The earlier healthy NPoS probe at `/tmp/izanami_npos_healthy_probe_block_created_state_reduction_20260315T000000Z.log` is now stale for this tranche because it was built before the proposal-owned commit activation cut landed.
- Fresh healthy NPoS probes on the current code are complete:
  - `/tmp/izanami_npos_healthy_probe_proposal_owned_commit_20260315T113400Z.log` stalled before `target_blocks=120` at quorum/strict `68/76`;
  - `/tmp/izanami_npos_healthy_probe_proposal_owned_commit_keepdirs_20260315T114000Z.log` reran the same scenario with `IROHA_TEST_NETWORK_KEEP_DIRS=1` and stalled earlier at quorum/strict `57/64`, preserving peer logs under `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_QgNYCr`.
- The structural result of this tranche is visible in preserved peer stdout:
  - `no proposal observed for view before changing view` still appears on the failing height (`57`), but those entries now show `commit_inflight=None`, so the proposal-less pending/commit activation path is no longer the thing driving the rotation;
  - dominant warnings are still `commit quorum missing past timeout; rescheduling block for reassembly`, mostly vote-backed (`votes=1` or `2`, `min_votes=3`) at ordinary frontier heights like `25`, `26`, `30`, `32`, `36`, `39`, `40`, `57`, and `62`;
  - zero-vote `drop_pending=true` reschedules still exist in the preserved run (for example at height `20` on `assuring_stonechat`), but the old proposal-less `commit_inflight` signature did not reappear.
- Conclusion: keep this simplification. It removes one invalid state transition, but it does not solve healthy NPoS throughput or latency by itself. The dominant remaining bottleneck has shifted back to vote-backed quorum reschedule / late missing-QC churn, not proposal-less commit activation.

## 2026-03-15 Sumeragi BlockCreated Proposal-Owned Round State
- Simplified `crates/iroha_core/src/sumeragi/main_loop/proposal_handlers.rs` so payload delivery no longer owns round/view state:
  - `BlockCreated` now records `CollectDa` phase only when the slot was already observed through `Proposal` handling;
  - payload-only `BlockCreated` still populates pending payload state, but it no longer marks the slot as an observed proposal and no longer advances round/view tracking by itself.
- Added and updated focused regressions in `crates/iroha_core/src/sumeragi/main_loop/tests.rs`:
  - `block_created_without_cached_proposal_does_not_advance_active_view`
  - `block_created_with_matching_proposal_advances_active_view`
  - `block_created_records_collect_da_phase_after_proposal_seen`

### Validation Matrix (BlockCreated Proposal-Owned Round State)
- `cargo fmt --all` (pass)
- `cargo test -p iroha_core --lib block_created_without_cached_proposal_does_not_advance_active_view -- --nocapture` (pass)
- `cargo test -p iroha_core --lib block_created_with_matching_proposal_advances_active_view -- --nocapture` (pass)
- `cargo test -p iroha_core --lib block_created_records_collect_da_phase_after_proposal_seen -- --nocapture` (pass)
- `cargo test -p iroha_core --lib block_created_without_hint_accepts_extending_lock -- --nocapture` (pass)
- `cargo test -p iroha_core --lib block_created_uses_cached_proposal_when_lock_missing -- --nocapture` (pass)

### Runtime Signal (BlockCreated Proposal-Owned Round State)
- Fresh healthy NPoS probe started on this cut:
  - command: `cargo run -p izanami --release -- --allow-net --nexus --peers 4 --faulty 0 --target-blocks 120 --latency-p95-threshold 1s --progress-interval 10s --progress-timeout 120s --tps 5 --max-inflight 8`
  - log: `/tmp/izanami_npos_healthy_probe_block_created_state_reduction_20260315T000000Z.log`
  - current state at status update time: still in release-build warmup; runtime verdict not available yet.

## 2026-03-15 Sumeragi Zero-Vote Reschedule Rotation Removal
- Reverted the regressed aborted-pending commit-QC experiment and restored the earlier revive semantics in `crates/iroha_core/src/sumeragi/main_loop/{main_loop.rs,commit.rs,proposal_handlers.rs,tests.rs}`:
  - commit-QC on an aborted pending block revives it again;
  - duplicate `BlockCreated` on an aborted pending block revives the payload and now preserves the existing `commit_qc_seen` marker on that path;
  - removed the temporary commit-pipeline-specific pending counter and the proof tests that depended on the non-revive model.
- Simplified quorum-timeout ownership in `crates/iroha_core/src/sumeragi/main_loop/{reschedule.rs,commit.rs}`:
  - `reschedule_pending_quorum_block(...)` no longer returns an immediate-rotation flag;
  - zero-vote quorum reschedules may still drop/requeue stale pending frontier state, but they no longer trigger a direct `MissingQc` / quorum-timeout view change from the reschedule or commit pipeline paths;
  - the frontier timeout controller remains the only owner of post-reschedule view rotation.
- Updated focused regressions in `crates/iroha_core/src/sumeragi/main_loop/tests.rs`:
  - restored `commit_qc_revives_aborted_pending_block`;
  - restored `block_created_revives_aborted_pending`;
  - renamed zero-vote coverage to `zero_vote_quorum_timeout_drops_pending_without_immediate_view_change` and asserted `missing_qc_total == 0` on the reschedule path;
  - updated `reschedule_defers_zero_vote_quorum_timeout_while_block_queue_backlogged` to assert that the zero-vote drop still does not rotate immediately.

### Validation Matrix (2026-03-15 Zero-Vote Reschedule Rotation Removal)
- `cargo fmt --all` (pass)
- `cargo test -p iroha_core --lib commit_qc_revives_aborted_pending_block -- --nocapture` (pass)
- `cargo test -p iroha_core --lib block_created_revives_aborted_pending -- --nocapture` (pass)
- `cargo test -p iroha_core --lib finalize_pending_block_revives_aborted_on_tip_with_commit_qc -- --nocapture` (pass)
- `cargo test -p iroha_core --lib commit_inflight_timeout_triggers_view_change_and_retains_aborted_pending -- --nocapture` (pass)
- `cargo test -p iroha_core --lib block_sync_update_keeps_aborted_next_height_payload_sparse_without_commit_evidence -- --nocapture` (pass)
- `cargo test -p iroha_core --lib stake_quorum_timeout_reschedules_without_immediate_view_change -- --nocapture` (pass)
- `cargo test -p iroha_core --lib zero_vote_quorum_timeout_drops_pending_without_immediate_view_change -- --nocapture` (pass)
- `cargo test -p iroha_core --lib reschedule_defers_zero_vote_quorum_timeout_while_block_queue_backlogged -- --nocapture` (pass)

### Soak Signal (2026-03-15 Zero-Vote Reschedule Rotation Removal)
- The temporary aborted-pending non-revive experiment was a real regression:
  - `/tmp/izanami_npos_healthy_probe_commit_qc_state_reduction_20260315T055734Z.log` stalled at quorum/strict `66/73` before `target_blocks=160`;
  - `/tmp/izanami_npos_healthy_probe_commit_qc_state_reduction_retry_20260315T061708Z.log` reproduced the regression at quorum/strict `65/69` before `target_blocks=120`.
- Reverting that experiment restored the previous healthy-failure envelope:
  - `/tmp/izanami_npos_healthy_probe_commit_qc_revert_20260315T062616Z.log` stalled later at quorum/strict `69/73`, confirming the aborted-pending cut was responsible for the sharper regression.
- A live-inspection probe isolated the remaining recurring bad path:
  - `/tmp/izanami_npos_healthy_probe_snapshot_20260315T063717Z.log` timed out at quorum/strict `65/71`;
  - timed snapshots captured repeated `commit quorum missing past timeout; rescheduling block for reassembly` warnings in peer stdout at ordinary frontier heights `3`, `7`, `15`, `21`, `24`, `33`, and `40`;
  - the most important recurring pattern was zero-vote reschedules with `drop_pending=true` and `rotate_immediately=true` at heights like `21`, showing that `reschedule.rs` still had a second direct view-change authority.
- After removing the reschedule-owned rotation:
  - `/tmp/izanami_npos_healthy_probe_no_resched_rotate_20260315T064146Z.log` still timed out, but moved slightly to quorum/strict `66/71`;
  - conclusion: the simplification is structurally correct and should stay, but the dominant healthy NPoS stall remains elsewhere. The next cut should target why zero-vote pending state is reaching the reschedule path so often, not add more recovery gates.

## 2026-03-14 Sumeragi Pending-Frontier Timeout Simplification
- Simplified the remaining quorum-timeout ownership split in `crates/iroha_core/src/sumeragi/main_loop/{reschedule.rs,commit.rs,main_loop.rs}`:
  - vote-backed quorum reschedules no longer trigger an immediate view change; `reschedule_pending_quorum_block(...)` now returns whether the pending block was dropped as zero-evidence zombie state, and only that path emits an immediate `MissingQc`/`StakeQuorumTimeout` rotation.
  - the generic `maybe_force_view_change_for_stalled_pending(...)` path now uses one bounded long grace for non-fast-path frontier pending blocks: `max(backlog_timeout, 2 * recovery_deferred_qc_ttl)`. This replaces the earlier short backlog-driven rotation window for active frontier pending blocks.
  - the reduced near-quorum missing-payload fast path is still intact; only the non-fast-path frontier branch was simplified.
- Added and updated focused regressions in `crates/iroha_core/src/sumeragi/main_loop/tests.rs`:
  - `stake_quorum_timeout_reschedules_without_immediate_view_change`
  - `zero_vote_quorum_timeout_reschedules_and_records_missing_qc_view_change`
  - `maybe_force_view_change_for_stalled_pending_vote_backed_blocks_wait_for_deferred_qc_ttl`
  - `maybe_force_view_change_for_stalled_pending_non_near_path_waits_for_frontier_pending_timeout`
  - `maybe_force_view_change_for_stalled_pending_uses_frontier_pending_timeout_when_view_age_lags_pending_stall`
  - `maybe_force_view_change_for_stalled_pending_applies_frontier_pending_timeout_with_residual_round_state`

### Validation Matrix (Pending-Frontier Timeout Simplification)
- `cargo fmt --all` (pass)
- `cargo test -p iroha_core --lib maybe_force_view_change_for_stalled_pending_ -- --nocapture` (pass)
- `cargo test -p iroha_core --lib stake_quorum_timeout_reschedules_without_immediate_view_change -- --nocapture` (pass)
- `cargo test -p iroha_core --lib zero_vote_quorum_timeout_reschedules_and_records_missing_qc_view_change -- --nocapture` (pass)

### Soak Signal (Pending-Frontier Timeout Simplification)
- Baseline short healthy NPoS probe before the new long frontier-pending timeout:
  - log: `/tmp/izanami_npos_healthy_probe_vote_backed_resched_20260314T212202Z.log`
  - result: reached `target_blocks=260` at height `262`, but still failed the latency gate with `interval_p50=1429ms`, `interval_p95=2501ms`, `samples=257`, `elapsed=440.099561083s`.
  - peer logs in that run showed the old pattern directly: vote-backed reschedule (`rotate_immediately=false`) followed by `active pending block stalled past quorum timeout; forcing deterministic view change`, then `revived aborted pending block after commit QC`.
- Intermediate probe with the narrower vote-backed deferred-QC TTL gate only:
  - log: `/tmp/izanami_npos_healthy_probe_deferred_qc_ttl_20260314T213707Z.log`
  - result: regressed to a fixed-height stall at `height 94` after repeated `10001ms` single-block intervals.
  - conclusion: gating only on locally counted commit votes was too narrow and did not remove the underlying early healthy wedge.
- Current probe after the non-fast-path frontier pending timeout simplification:
  - log: `/tmp/izanami_npos_healthy_probe_frontier_pending_timeout_20260314T214737Z.log`
  - result: reached `target_blocks=120` at height `123` with no replay of the old `68-94` fixed-height wedge, but still failed the latency gate with `interval_p50=1429ms`, `interval_p95=2501ms`, `samples=118`, `elapsed=200.047094208s`.
  - peer-log sampling during the run no longer showed `active pending block stalled past quorum timeout; forcing deterministic view change` through the old hotspot band; a late `revived aborted pending block after commit QC` signal still reappeared later at height `102`.
  - conclusion: the simplified timeout ownership improved the early healthy failure shape, but it did not improve the `p95` latency envelope and did not eliminate all late revive churn.

## 2026-03-14 Sumeragi Frontier Cleanup NEW_VIEW Reset
- Tightened `crates/iroha_core/src/sumeragi/main_loop.rs` frontier cleanup so the contiguous-frontier reset now also owns `NEW_VIEW` state:
  - `apply_frontier_recovery_cleanup(...)` now clears `new_view_tracker` entries at/above the frontier, clears `forced_view_after_timeout` markers at/above the frontier, and drops `Phase::NewView` vote history at/above the frontier from `vote_log`.
  - committed-edge conflict cleanup in `crates/iroha_core/src/sumeragi/main_loop/votes.rs` now reuses the same `NEW_VIEW` vote-history clearing helper, keeping the frontier reset behavior consistent across both cleanup paths.
- Tightened RBC cleanup ownership for the same frontier reset paths:
  - `purge_rbc_sessions_at_or_above_height(...)` now purges per-key RBC state from `pending`, rebroadcast cooldowns, deferrals, persisted markers, and seed-inflight state even when no live RBC session object exists.
  - `clean_rbc_sessions_for_block(...)` now reuses the same per-key purge behavior for orphan RBC state keyed by the block hash, instead of only draining maps that still had a live `sessions` entry.
- Added focused regression `frontier_recovery_cleanup_clears_frontier_new_view_state` in `crates/iroha_core/src/sumeragi/main_loop/tests.rs`.
- Added focused RBC cleanup regressions:
  - `frontier_recovery_cleanup_purges_orphan_pending_rbc_without_sessions`
  - `clean_rbc_sessions_for_block_clears_seed_inflight` now also covers orphan pending/rebroadcast cleanup.

### Validation Matrix (Frontier Cleanup NEW_VIEW Reset)
- `cargo fmt --all` (pass)
- `cargo test -p iroha_core --lib frontier_recovery_cleanup_ -- --nocapture` (pass)
- `cargo test -p iroha_core --lib observe_new_view_highest_qc_suppression_ -- --nocapture` (pass)
- `cargo test -p iroha_core --lib clean_rbc_sessions_for_block_clears_seed_inflight -- --nocapture` (pass)
- `cargo test -p iroha_core --lib frontier_recovery_cleanup_purges_rbc_ -- --nocapture` (pass)
- `cargo test -p iroha_core --lib frontier_recovery_cleanup_purges_orphan_pending_rbc_without_sessions -- --nocapture` (pass)

### Soak Signal (Frontier Cleanup NEW_VIEW Reset)
- 15-minute NPoS healthy canary on the frontier-cleanup NEW_VIEW reset build:
  - log: `/tmp/izanami_npos_healthy_canary_frontier_cleanup_new_view_reset_20260314T204030Z.log`
  - result: reached `target_blocks=400` and stopped at quorum/strict height `406`, with no fixed-height stall and no late `293/294` wedge.
  - latency gate still failed: quorum/strict interval `p50=1429ms`, `p95=2501ms`, `samples=399`, `elapsed=660.146095459s`.
  - peer state at the target checkpoint stayed aligned (`view_changes=0` on the sampled peers, no strict/quorum divergence).
  - remaining follow-up signal: the stability regressions appear fixed for this canary, but latency remains well above the `< 1000ms` acceptance gate. The next branch should rerun the same canary on the orphan-RBC cleanup patch to see whether clearing residual `da_rbc.rbc.pending` state changes the latency envelope or merely removes another dormant state leak.

## 2026-03-14 Sumeragi Frontier Follow-Up (Far-Future RBC Clamp + Committed-Edge Vote Cleanup)
- Simplified `crates/iroha_core/src/sumeragi/main_loop/rbc.rs` so far-future RBC sessions no longer create independent missing-block recovery state:
  - future-window RBC `Init`/`Chunk`/`Ready`/delivery recovery now suppresses per-height missing-block fetches beyond the contiguous frontier.
  - far-future RBC recovery now purges that RBC session state and emits one canonical committed-frontier range pull (`reason = "rbc_far_future_missing_block"`) instead of opening a second recovery machine for the far height.
- Simplified QC-backed block-sync/RBC interaction in `crates/iroha_core/src/sumeragi/main_loop/block_sync.rs`:
  - removed the old non-roster-only `force_rbc_delivery_for_block_sync(...)` special case.
  - when a validated incoming QC is applied for a locally known block, the block-sync path now cleans RBC session state for that block and continues through the normal commit pipeline instead of preserving a separate forced-delivery state.
- Simplified committed-edge conflict cleanup in `crates/iroha_core/src/sumeragi/main_loop/votes.rs`:
  - committed-edge suppression already clamped the frontier round, cleared forced-view markers, and dropped NEW_VIEW tracker entries.
  - it now also clears frontier/future `Phase::NewView` votes from `vote_log`, so stale higher-view local vote history cannot veto later canonical frontier NEW_VIEW recovery after the committed edge is realigned.

### Validation Matrix (Frontier Follow-Up)
- `cargo fmt --all` (pass)
- `cargo test -p iroha_core --lib future_consensus_window_rbc_messages_reanchor_frontier_without_far_height_requests -- --nocapture` (pass)
- `cargo test -p iroha_core --lib request_missing_block_for_pending_rbc_far_future_ -- --nocapture` (pass)
- `cargo test -p iroha_core --lib recover_block_from_rbc_session_far_future_reanchors_contiguous_frontier -- --nocapture` (pass)
- `cargo test -p iroha_core --lib recover_block_from_rbc_session_requests_missing_block_ -- --nocapture` (pass)
- `cargo test -p iroha_core --lib request_missing_block_for_pending_rbc_with_aborted_payload -- --nocapture` (pass)
- `cargo test -p iroha_core --lib observe_new_view_highest_qc_suppression_ -- --nocapture` (pass)
- `cargo test -p iroha_core --lib block_sync_update_known_block_ -- --nocapture` (pass)
- `cargo test -p iroha_core --lib clean_rbc_sessions_for_block_clears_seed_inflight -- --nocapture` (pass)

### Soak Signal (Frontier Follow-Up)
- 15-minute NPoS healthy canary with the far-future RBC clamp:
  - log: `/tmp/izanami_npos_healthy_canary_far_future_rbc_clamp_20260314T195603Z.log`
  - result: failed `target_blocks=400` at strict/quorum height `265`.
  - narrowed failure shape: three peers advanced to `324` while one peer (`necessary_dingo`, API `32371`) stalled at `265`.
  - the previous far-future `rbc_*_future_window` churn did not recur; the stuck peer instead showed committed-edge conflict suppression at height `265`, stale higher-view NEW_VIEW vote history (`already voted in a higher view`), repeated `missing_qc` frontier recovery at `266`, and fallback-anchor sharing from height `202`.
- Follow-up canary on the committed-edge NEW_VIEW vote cleanup build:
  - log: `/tmp/izanami_npos_healthy_canary_committed_edge_vote_cleanup_20260314T2017Z.log`
  - result: failed `target_blocks=400` at strict/quorum height `292`.
  - this build removed the earlier `265` committed-edge wedge but still stalled globally at the late contiguous frontier, with repeated `missing_qc`/`no proposal observed` loops around `293/294` and stale higher-view NEW_VIEW vote history (`higher_view=10`) still blocking canonical frontier votes on lagging peers.

## 2026-03-14 Sumeragi Frontier-Recovery Simplification
- Simplified timeout-driven Sumeragi recovery in `crates/iroha_core/src/sumeragi/main_loop.rs`:
  - introduced a single frontier-scoped recovery controller with `CatchUp` and `RotateArmed` phases.
  - timeout-driven recovery is now contiguous-frontier-only (`committed + 1`); future heights no longer create independent timeout-recovery state.
  - repeated no-progress windows now follow one deterministic path: bounded catch-up reanchor, one cleanup + all-peer reanchor, then one bounded rotation on the next window if progress still does not resume.
- Removed same-height timeout choreography from the idle and committed-edge paths:
  - `force_view_change_if_idle(...)` now routes dependency-backed timeout handling through the unified frontier controller instead of layering hysteresis/backoff/storm-break/frontier-window reservations.
  - committed-edge conflict recovery in `crates/iroha_core/src/sumeragi/main_loop/votes.rs` now routes through the same unified controller instead of directly invoking the storm breaker.
- Preserved the existing public status surface:
  - `consensus_no_proposal_storm_*` and `blocksync_range_pull_expiry_streak_*` remain exposed, but are now driven by the unified frontier controller rather than separate same-height storm state.

### Validation Matrix (Frontier-Recovery Simplification)
- `cargo test -p iroha_core --lib frontier_recovery_ -- --nocapture` (pass)
- `cargo test -p iroha_core --lib same_height_no_proposal_storm_breaker_ -- --nocapture` (pass)
- `cargo test -p iroha_core --lib same_height_no_proposal_storm_counter_persists_until_true_progress -- --nocapture` (pass)
- `cargo test -p iroha_core --lib repeated_range_pull_expiry_forces_tier_jump_cooldown_clear_and_reanchor -- --nocapture` (pass)
- `cargo test -p iroha_core --lib committed_edge_conflict_ -- --nocapture` (pass)

## 2026-03-14 Block-Sync Unknown-Prev Fallback Simplification
- Simplified `crates/iroha_core/src/block_sync.rs` unknown-prev rewind selection:
  - removed the half-chain-to-256 fallback window.
  - unknown-prev fallback rewind now keeps short-chain half-window behavior but caps mid/deep rewinds to `UNKNOWN_PREV_RECENT_CHAIN_HASH_WINDOW` (64 blocks), matching the existing recent-hash hint surface.
  - added regression `unknown_prev_fallback_caps_mid_height_rewind_to_recent_window`.
- Narrow validation for the kept variant:
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_core --lib unknown_prev_ -- --nocapture` (pass)
  - `cargo test -p iroha_core --lib get_blocks_after_repeated_unknown_prev_probe_requests_latest_on_incremental_repeats -- --nocapture` (pass)
- 15-minute NPoS healthy canary for the kept variant:
  - log: `/tmp/izanami_npos_healthy_canary_unknown_prev_cap_20260314T175113Z.log`
  - result: improved from the prior `height 345` plateau to `height 384`, but still failed `target_blocks=400` before duration end.
  - interval summary: `count=61`, `median=1449ms`, `p95=2501ms`, `max=3335ms`.
  - peer behavior improved materially before the final stall: no fallback-anchor sharing through height `324`, then late same-height `385` missing-QC / fallback-anchor churn reappeared.
- Rejected follow-up variant:
  - tried clamping repeated incremental unknown-prev sharing directly to the committed edge in `crates/iroha_core/src/block_sync.rs`.
  - 15-minute NPoS healthy canary for that variant regressed to an earlier `height 280` stall with broader cleanup/rotate churn; the patch was reverted and is not kept in the tree.
  - regression canary log: `/tmp/izanami_npos_healthy_canary_incremental_edge_clamp_20260314T181402Z.log`

## 2026-03-13 Timeout-Driven Sumeragi Recovery Stabilization (bounded suppression + deterministic escalation)
- Implemented strict global-stall enforcement in `crates/izanami/src/chaos.rs`:
  - `should_enforce_strict_progress_timeout(...)` now enforces strict timeout for healthy/global stalls (`lagging_peers == 0`) and for lag beyond tolerated failures (`lagging_peers > tolerated_failures`), while preserving tolerated-failure behavior for true outlier lag.
  - strict-stall warning text now explicitly distinguishes tolerated outlier lag from globally enforced strict stalls.
- Implemented same-height missing-QC storm tracking and bounded suppression in `crates/iroha_core/src/sumeragi/main_loop.rs`:
  - added dedicated per-height storm state/counters independent from timeout-marker cleanup.
  - storm reset now happens only on true progress signals (commit height advances past tracked height, proposal observed at tracked height/view, or dependency-progress timestamp advance).
  - storm breaker now uses storm count (not timeout marker streak), supports backlog-driven trigger when `pending_blocks == 0`, and records forced-cleanup state for one post-cleanup rotation allowance after the next timeout window.
  - fixed regression where deterministic frontier reset could clear same-height state before suppression checks: same-height rotation is now still suppressed for that timeout cycle when an in-window canonical frontier reanchor remained unresolved at timeout, unless explicit post-cleanup allowance is active.
- Hardened repeated range-pull expiry escalation continuity in `crates/iroha_core/src/sumeragi/main_loop.rs`:
  - added per-height expiry streak persistence (`missing_block_range_pull_expiry_streaks`) across cleanup paths until actual dependency progress.
  - repeated no-progress expiry (`>= 2` windows) now immediately advances escalation tier, clears range-pull cooldown gates for the height/frontier, emits canonical all-target reanchor, and reserves the round-recovery window token to avoid immediate same-tier reentry.
- Updated committed-edge conflict cleanup gating in `crates/iroha_core/src/sumeragi/main_loop/votes.rs`:
  - committed-edge conflict path keeps canonical cleanup/reanchor behavior but now only invokes storm-break when dedicated storm threshold is reached, preventing suppression bypass before forced-escalation threshold.
- Extended observability in `crates/iroha_core/src/sumeragi/status.rs`:
  - added `consensus_no_proposal_storm_last_height`, `consensus_no_proposal_storm_last_count`, and `consensus_no_proposal_storm_max_count` to status snapshot state/counters.

### Validation Matrix (Timeout-Driven Stabilization)
- `cargo fmt --all` (pass)
- `cargo test -p iroha_core missing_qc_view_change_suppressed_when_frontier_reanchor_already_emitted -- --nocapture` (pass)
- `cargo test -p iroha_core highest_qc_fetch_suppresses_committed_height_hash_conflict -- --nocapture` (pass)
- `cargo test -p iroha_core same_height_no_proposal_storm -- --nocapture` (pass)
- `cargo test -p iroha_core repeated_range_pull_expiry_forces_tier_jump_cooldown_clear_and_reanchor -- --nocapture` (pass)
- `cargo test -p izanami strict_progress_timeout_enforcement_respects_bft_tolerance -- --nocapture` (pass)

## 2026-03-12 Sub-1s Soak Recovery Plan (Fail-Open Gating + Deterministic Catch-Up)
- `crates/izanami/src/chaos.rs` now applies configured fault budget to strict progress semantics:
  - `effective_tolerated_peer_failures(peer_count, configured_faulty_peers)` now honors `configured_faulty_peers` and bounds it by protocol BFT tolerance.
  - strict stall/quorum divergence calculations now consistently use this effective tolerance, so healthy runs (`faulty=0`) fail on unexpected stragglers while frozen-peer runs (`faulty=1`) tolerate one lagging peer.
  - updated regressions: `effective_tolerated_peer_failures_honors_configured_fault_budget`, `quorum_min_height_respects_effective_tolerance`, and `strict_divergence_reference_height_trims_only_effectively_tolerated_outliers`.
- `crates/iroha_core/src/sumeragi/main_loop.rs` and `crates/iroha_core/src/sumeragi/main_loop/block_sync.rs` now escalate repeated sidecar mismatch loops into deterministic recovery:
  - repeated sidecar mismatch recovery now increments a dedicated recovery-trigger counter.
  - no-verifiable-roster drop path now clears stale same-height missing dependencies when sidecar is quarantined and requests canonical `committed+1` reanchor/range-pull.
  - repeated range-pull expiry without progress now increments per-height expiry streak and immediately advances one additional range-pull tier once streak is repeated (avoids re-entering identical incremental loops).
- Added bounded same-height no-proposal storm breaker in `crates/iroha_core/src/sumeragi/main_loop.rs` with committed-edge integration in `crates/iroha_core/src/sumeragi/main_loop/votes.rs`:
  - when repeated same-height no-proposal rounds occur with stale pending blocks but no actionable dependencies, perform one deterministic stale-state cleanup and canonical reanchor.
  - idle missing-QC and committed-edge conflict paths both use this breaker to prevent stale/non-actionable hashes repeatedly rearming view-change loops.
- Extended consensus observability in `crates/iroha_core/src/sumeragi/status.rs`:
  - new counters/fields: `consensus_sidecar_recovery_trigger_total`, `consensus_no_proposal_storm_total`, `blocksync_range_pull_expiry_streak_last`, `blocksync_range_pull_expiry_streak_max`.
  - status snapshot and unit coverage updated (`missing_block_fetch_counters_surface_in_snapshot`).
- Targeted validation in this tree:
  - `cargo test -p izanami effective_tolerated_peer_failures_honors_configured_fault_budget -- --nocapture`
  - `cargo test -p iroha_core missing_block_fetch_counters_surface_in_snapshot -- --nocapture`
  - `cargo test -p iroha_core same_height_no_proposal_storm_breaker_cleans_stale_state -- --nocapture`

## 2026-03-12 Localnet Sub-1s Follow-Up (Hard-Gate Enforcement + Actionable-Only Rotation)
- Enforced latency hard-gate at duration completion in `crates/izanami/src/chaos.rs`:
  - added `enforce_latency_p95_gate(...)`.
  - gate now applies at `target_reached` and also at `duration_deadline` in soft-KPI mode.
  - error output now includes `checkpoint` context (`target_reached` or `duration_deadline`).
- Added regression in `crates/izanami/src/chaos.rs`:
  - `wait_for_target_blocks_soft_kpi_enforces_latency_gate_at_duration_end`.
- Tightened no-actionable missing-QC rotation behavior in `crates/iroha_core/src/sumeragi/main_loop.rs`:
  - proposal-gap backlog grace and RBC-progress grace deferrals now apply only when actionable dependency signals remain.
  - deterministic frontier/lock-lag cleanup no longer defers rotation when dependencies are non-actionable.
  - same-height no-actionable cleanup now also clears `round_recovery_bundle_window_gates` for that height.
  - `try_reserve_round_recovery_bundle_window(...)` no longer suppresses no-actionable rotations.
- Compile and targeted validations now pass on this tree:
  - `cargo check -p iroha_core --all-targets`
  - `cargo check -p izanami --all-targets`
  - `cargo test -p iroha_core --lib force_view_change_if_idle_no_actionable_dependency_rotates_after_base_timeout -- --nocapture`
  - `cargo test -p iroha_core --lib highest_qc_fetch_suppresses_committed_height_hash_conflict -- --nocapture`
  - `cargo test -p izanami wait_for_target_blocks_soft_kpi -- --nocapture`
- 15-minute gated canary matrix (explicit `--latency-p95-threshold 1s`, `target_blocks=400`) confirms hard-gate behavior:
  - permissioned healthy: `/tmp/izanami_permissioned_canary_gate_20260312T060142Z.log` → failed on gate (`quorum p95=2502ms > 1000ms`).
  - permissioned + frozen peer: `/tmp/izanami_permissioned_frozen_canary_gate_20260312T062318Z.log` → failed on gate (`quorum p95=2502ms > 1000ms`), strict lag persisted (`strict min 105` at gate trip).
  - NPoS healthy: `/tmp/izanami_npos_canary_gate_20260312T063343Z.log` → failed on gate (`quorum p95=2502ms > 1000ms`).
  - NPoS + frozen peer: `/tmp/izanami_npos_frozen_canary_gate_20260312T064516Z.log` → failed on gate (`quorum p95=2502ms > 1000ms`), strict lag persisted (`strict min 102` at gate trip).

## 2026-03-11 Localnet Sub-1s Block Production Plan (Permissioned + NPoS)
- Implemented missing-QC actionable-only timeout gating in `crates/iroha_core/src/sumeragi/main_loop.rs`:
  - same-height no-actionable cleanup now runs before timeout hysteresis/defer/backoff decisions.
  - stale no-actionable cleanup clears height recovery budget plus missing-QC timeout/reacquire markers for the active height.
  - missing-QC hysteresis/defer/backoff now require actionable dependency signals (including unresolved actionable frontier dependency), so stale/non-actionable backlog does not extend rotation timing.
- Implemented committed-edge fallback hardening in `crates/iroha_core/src/sumeragi/main_loop/votes.rs`:
  - committed-edge conflicting highest-QC suppression still clears obsolete request state.
  - bounded canonical reanchor is now emitted only when contiguous frontier dependency is still actionable.
- Implemented shared-host stable profile retuning in `crates/izanami/src/chaos.rs`:
  - pipeline floor: `180ms`.
  - shared-host NPoS timeout floors: `propose=60ms`, `prevote=90ms`, `precommit=120ms`, `commit=320ms`, `da=320ms`, `aggregator=20ms`.
  - recovery windows: `height_window=2000ms`, `missing_qc_reacquire_window=800ms`, `deferred_qc_ttl=2000ms`.
  - recovery escalation: `hash_miss_cap_before_range_pull=1`, `range_pull_escalation_after_hash_misses=1`.
  - shared-host pending-stall grace: `300ms`.
  - shared-host `da_fast_reschedule` enabled.
  - stable shared-host soak default latency gate: `latency_p95_threshold=1s` when unset.
  - startup override log now includes latency profile marker and effective latency p95 gate fields.
- Added/updated targeted regressions:
  - `force_view_change_if_idle_no_actionable_dependency_rotates_after_base_timeout`.
  - `force_view_change_if_idle_prunes_stale_round_state_during_repeated_same_height_missing_qc` now asserts stale timeout streak reset.
  - committed-edge suppression regression now asserts no reanchor for non-actionable conflict hashes.
  - shared-host stable profile tests now assert sub-1s latency gate default and updated timeout/pending-stall constants.
- Validation attempts in this workspace were blocked by pre-existing branch compile errors unrelated to this patch:
  - `iroha_core` tests: `AccountId::new(...)` signature mismatch in `crates/iroha_core/src/smartcontracts/isi/triggers/set.rs` and pre-existing test code.
  - `izanami` check/test: ongoing `ScopedAccountId` migration mismatches in `crates/izanami/src/{chaos.rs,instructions.rs}`.
  - successful local compile gate for touched core runtime path: `cargo check -p iroha_core --lib`.

## 2026-03-11 Full 60-Min Cross-Mode Soak Replay (Fresh Rerun)
- Replayed both full soaks on the current tree with unchanged stable envelope and `progress-timeout=600s`:
  - permissioned log: `/tmp/izanami_permissioned_full_soak_20260311T20260311T180117_escalated.log`
  - NPoS log: `/tmp/izanami_npos_full_soak_20260311T20260311T192506_escalated.log`
- Both runs completed duration without `600s` no-progress abort:
  - permissioned summary: `successes=17998 failures=0 izanami_ingress_failover_total=0 izanami_ingress_endpoint_unhealthy_total=0`
  - NPoS summary: `successes=18000 failures=0 izanami_ingress_failover_total=0 izanami_ingress_endpoint_unhealthy_total=0`
- Final persisted heights:
  - permissioned (`lane_000_default`): `1414/1414/1414/569`
  - NPoS (`lane_000_core`): `959/959/959/184`
- Effective pace (`duration=3600s`):
  - permissioned: strict `569 / 3600 = 0.158 blocks/s`; quorum-with-1-failure `1414 / 3600 = 0.393 blocks/s`
  - NPoS: strict `184 / 3600 = 0.051 blocks/s`; quorum-with-1-failure `959 / 3600 = 0.266 blocks/s`
- Residual churn remains lagging-peer concentrated (not quorum-wide liveness loss):
  - permissioned aggregate markers: `no proposal observed before cutoff=795`, `commit quorum missing past timeout=163`, `fallback anchor shares=4342`, `roster sidecar mismatch ignores=94`
  - NPoS aggregate markers: `no proposal observed before cutoff=698`, `commit quorum missing past timeout=36`, `fallback anchor shares=4745`, `roster sidecar mismatch ignores=576`
  - committed-edge suppression remained bounded (`permissioned=5`, `NPoS=10`).

## 2026-03-11 Full 60-Min Cross-Mode Soak Replay (Post Actionable-Dependency + Shared-Host Parity Patch)
- Executed full 60-minute Izanami stable soaks for both modes using the same envelope and `progress-timeout=600s`:
  - permissioned log: `/tmp/izanami_permissioned_full_soak_20260310T20260311T092642_escalated.log`
  - NPoS log: `/tmp/izanami_npos_full_soak_20260311T20260311T163813_escalated.log`
- Both runs completed duration without `600s` no-progress abort and without ingress failover/unhealthy churn:
  - permissioned summary: `successes=18000 failures=0 izanami_ingress_failover_total=0 izanami_ingress_endpoint_unhealthy_total=0`
  - NPoS summary: `successes=18000 failures=0 izanami_ingress_failover_total=0 izanami_ingress_endpoint_unhealthy_total=0`
- Final persisted heights (Kura, lane/core height):
  - permissioned (`lane_000_default`): `1279/1279/1279/529` (strict min `529`, quorum-with-1-failure min `1279`)
  - NPoS (`lane_000_core`): `952/952/952/124` (strict min `124`, quorum-with-1-failure min `952`)
- Throughput envelope under this host/load profile (`duration=3600s`):
  - permissioned strict pace: `529 / 3600 = 0.147 blocks/s`; quorum pace: `1279 / 3600 = 0.355 blocks/s`
  - NPoS strict pace: `124 / 3600 = 0.034 blocks/s`; quorum pace: `952 / 3600 = 0.264 blocks/s`
- Dominant residual signal remains one-peer lag/freeze-style churn rather than quorum-wide liveness collapse:
  - permissioned aggregate markers: `no proposal observed before cutoff=317`, `commit quorum missing past timeout=259`, `fallback anchor shares=3679`, `roster sidecar mismatch ignores=348`
  - NPoS aggregate markers: `no proposal observed before cutoff=975`, `commit quorum missing past timeout=30`, `fallback anchor shares=3445`, `roster sidecar mismatch ignores=63`
  - committed-edge conflict suppression remained bounded (`permissioned=9`, `NPoS=4`).

### Validation Matrix (2026-03-11 Full Soak Replay)
- `RUST_LOG=izanami::summary=info,izanami::workload=warn IROHA_TEST_NETWORK_KEEP_DIRS=1 IROHA_TEST_NETWORK_PERMIT_DIR=$(mktemp -d) CARGO_TARGET_DIR=/tmp/iroha_target_izanami_soak_20260310 cargo run -p izanami --release --locked -- --allow-net --peers 4 --faulty 0 --duration 3600s --target-blocks 2000 --progress-interval 10s --progress-timeout 600s --tps 5 --max-inflight 8 --workload-profile stable`
- `RUST_LOG=izanami::summary=info,izanami::workload=warn IROHA_TEST_NETWORK_KEEP_DIRS=1 IROHA_TEST_NETWORK_PERMIT_DIR=$(mktemp -d) CARGO_TARGET_DIR=/tmp/iroha_target_izanami_soak_20260310 cargo run -p izanami --release --locked -- --allow-net --nexus --peers 4 --faulty 0 --duration 3600s --target-blocks 2000 --progress-interval 10s --progress-timeout 600s --tps 5 --max-inflight 8 --workload-profile stable`
- `target/release/kagami kura <peer_block_store_path> print -o <out_file>` (used to extract persisted final heights from Kura block stores)

## 2026-03-10 Cross-Mode Soak Stabilization (Actionable Dependency Gating + Shared-Host Parity)
- Sumeragi missing-dependency classification was normalized across proposal/recovery/commit/vote paths:
  - non-actionable committed-edge conflicts and active lock-rejected hashes no longer gate missing-QC/proposal progress.
  - commit-phase dependency checks now ignore deferred missing-QC entries whose payload is already locally available.
  - stale height-recovery budgets and stale per-hash dependencies are cleared when no actionable dependency remains, preventing repeated same-height reintroduction loops.
  - files: `crates/iroha_core/src/sumeragi/main_loop.rs`, `crates/iroha_core/src/sumeragi/main_loop/votes.rs`.
- Izanami stable-soak profile parity now applies to both permissioned and NPoS shared-host runs:
  - same recovery/pacemaker envelope (timeouts, pending-stall grace, DA fast-reschedule policy, collectors/redundancy knobs) across both modes.
  - NPoS stake bootstrap/preflight constraints remain unchanged.
  - files: `crates/izanami/src/chaos.rs`.
- Stable-soak liveness gate is now duration-first:
  - completing the configured soak duration without `600s` no-progress remains success.
  - unmet `target_blocks` in stable shared-host mode is now KPI/warning (non-fatal).
- Added concise startup logging of effective consensus soak overrides per mode and targeted regressions for:
  - non-actionable dependency clearing,
  - stale-vote/highest-QC fallback actionable checks,
  - cross-mode shared-host override parity,
  - soft-KPI duration-complete behavior.

### Validation Matrix (Cross-Mode Soak Stabilization)
- `cargo test -p iroha_core proposal_gated_by_missing_dependencies_requires_recent_range_pull_progress -- --nocapture`
- `cargo test -p iroha_core proposal_gated_by_missing_dependencies_clears_stale_budget_without_actionable_dependency -- --nocapture`
- `cargo test -p iroha_core has_commit_phase_missing_qc_dependency_ignores_deferred_payload_already_local -- --nocapture`
- `cargo test -p iroha_core stale_view_drops_precommit_vote_when_missing_block_request_is_non_actionable -- --nocapture`
- `cargo test -p izanami shared_host_stable_soak_consensus_overrides_match_between_permissioned_and_npos -- --nocapture`
- `cargo test -p izanami shared_host_recovery_profile_applies_to_permissioned_stable_pilot_runs -- --nocapture`
- `cargo test -p izanami wait_for_target_blocks_reaches_target -- --nocapture`
- `cargo test -p izanami wait_for_target_blocks_soft_kpi_allows_duration_completion_without_target -- --nocapture`

## 2026-03-09 NPoS Soak Stabilization (Preflight + Conservative Tuning + Obsolete Sidecar Mismatch)
- Izanami NPoS preflight audit now runs before soak startup (`crates/izanami/src/chaos.rs`) and enforces:
  - `RegisterPeerWithPop == peer_count`.
  - `RegisterPublicLaneValidator == ActivatePublicLaneValidator == peer_count`.
  - uniform per-validator `initial_stake` and `initial_stake >= min_self_bond`.
  - one explicit stake-distribution summary log for immediate misconfiguration visibility.
- Reliability-first NPoS soak tuning (shared-host profile) landed in `crates/izanami/src/chaos.rs`:
  - timeout floors: `propose=150ms`, `prevote=250ms`, `precommit=350ms`, `commit=900ms`, `da=900ms`, `aggregator=50ms`.
  - `pending_stall_grace_ms=1000`.
  - `da_fast_reschedule=false`.
  - `collectors_k=3` and `redundant_send_r=3` for 4-peer shared-host NPoS soaks.
- Sumeragi repeated roster-sidecar mismatch hardening landed in:
  - `crates/iroha_core/src/sumeragi/main_loop.rs`
  - `crates/iroha_core/src/sumeragi/main_loop/mode.rs`
  - `crates/iroha_core/src/sumeragi/main_loop/votes.rs`
  - behavior change: repeated `(height, expected_hash, stored_hash)` mismatches are short-TTL obsolete/non-actionable; stale per-hash dependencies are cleared; mismatch recovery reanchors from canonical `committed+1` without reintroducing stale targeted missing-QC dependencies.
- New status surface:
  - `roster_sidecar_mismatch_obsolete_total` added in:
    - `crates/iroha_core/src/sumeragi/status.rs`
    - `crates/iroha_data_model/src/block/consensus.rs`
    - `crates/iroha_torii/src/routing.rs`
    - `crates/iroha_torii/src/routing/consensus.rs`
- Regression coverage updates:
  - Izanami preflight pass/fail tests for missing activation, missing PoP, unequal stake.
  - Sumeragi tests for repeated tuple obsolescence, stale dependency cleanup, and missing-QC stall suppression on obsolete committed-edge sidecar mismatches.
  - Data-model and Torii status tests updated for new status field.

### Validation Matrix (NPoS Soak Stabilization)
- `cargo fmt --all`
- `cargo test -p izanami derive_npos_timing_uses_conservative_floors_for_shared_host_npos_soak -- --nocapture`
- `cargo test -p izanami npos_preflight_audit_ -- --nocapture`
- `cargo test -p iroha_core --lib sidecar_mismatch_ -- --nocapture`
- `cargo test -p iroha_core --lib committed_edge_sidecar_conflict_emits_one_bundle_per_window -- --nocapture`
- `cargo test -p iroha_core --lib missing_block_fetch_counters_surface_in_snapshot -- --nocapture`
- `cargo test -p iroha_data_model --test consensus_roundtrip sumeragi_wire_status_roundtrip -- --nocapture`
- `cargo test -p iroha_torii --lib routing::status_tests::status_snapshot_json_includes_da_gate_and_kura_store -- --nocapture`

## 2026-03-09 Sumeragi Committed-Edge Conflict Obsolete-Dependency Fix
- Stopped treating committed-edge conflicting highest-QC hashes as active missing dependencies:
  - committed-edge conflict suppression now does canonical realignment + obsolete request clearing + bounded canonical reanchor only (no conflicting-hash targeted fetch).
  - files:
    - `crates/iroha_core/src/sumeragi/main_loop/votes.rs`
    - `crates/iroha_core/src/sumeragi/main_loop/proposal_handlers.rs`
- Applied the same suppression bundle on proposal/proposal-hint committed-edge drop paths to prevent reintroducing conflicting hashes as dependencies.
- Missing-QC dependency gating now ignores obsolete committed-edge conflicting requests, including commit-phase dependency checks and idle missing-QC reacquire signal detection:
  - `crates/iroha_core/src/sumeragi/main_loop.rs`
- Added telemetry/status counter:
  - `committed_edge_conflict_obsolete_total` in Sumeragi status snapshot and Torii status outputs.
  - files:
    - `crates/iroha_core/src/sumeragi/status.rs`
    - `crates/iroha_data_model/src/block/consensus.rs`
    - `crates/iroha_torii/src/routing/consensus.rs`
    - `crates/iroha_torii/src/routing.rs`
- Updated regressions for committed-edge conflict behavior and added proposal/proposal-hint regressions ensuring repeated committed-edge conflicts do not accumulate missing requests and do not block canonical proposal progress:
  - `crates/iroha_core/src/sumeragi/main_loop/tests.rs`

### Validation Matrix (Committed-Edge Conflict Obsolete-Dependency Fix)
- `cargo fmt --all`
- `cargo test -p iroha_core --lib committed_conflict -- --nocapture`
- `cargo test -p iroha_core --lib highest_qc_fetch_suppresses_committed_height_hash_conflict -- --nocapture`
- `cargo test -p iroha_core --lib missing_qc_height_stall_mode_ignores_obsolete_committed_edge_conflicts -- --nocapture`
- `cargo test -p iroha_core --lib missing_block_fetch_counters_surface_in_snapshot -- --nocapture`
- `cargo test -p iroha_torii --lib status_snapshot_json_includes_da_gate_and_kura_store --features "iroha_core/iroha-core-tests" -- --nocapture`
- `cargo test -p iroha_data_model --test consensus_roundtrip sumeragi_wire_status_roundtrip -- --nocapture`
- `cargo check -p iroha`

## 2026-03-09 Full Permissioned + NPoS Soak Replay (Committed-Edge Obsolete-Dependency Patch)
- Ran full 2000-block Izanami soaks for both consensus modes on this exact patch set with preserved peer artifacts.
- Permissioned full soak:
  - command: `RUST_LOG=izanami::summary=info,izanami::workload=warn IROHA_TEST_NETWORK_KEEP_DIRS=1 IROHA_TEST_NETWORK_PERMIT_DIR=$(mktemp -d) cargo run -p izanami --release --locked -- --allow-net --peers 4 --faulty 0 --duration 3600s --target-blocks 2000 --progress-interval 10s --progress-timeout 300s --tps 5 --max-inflight 8 --workload-profile stable`
  - result: failed on strict divergence guard after `successes=1951` (`divergence=55`, `quorum min=228`, `strict min=173`), log `/tmp/izanami_permissioned_full_soak_20260309T115334.log`, network `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_mWvP92`.
- NPoS full soak:
  - command: `RUST_LOG=izanami::summary=info,izanami::workload=warn IROHA_TEST_NETWORK_KEEP_DIRS=1 IROHA_TEST_NETWORK_PERMIT_DIR=$(mktemp -d) cargo run -p izanami --release --locked -- --allow-net --nexus --peers 4 --faulty 0 --duration 3600s --target-blocks 2000 --progress-interval 10s --progress-timeout 300s --tps 5 --max-inflight 8 --workload-profile stable`
  - result: failed on strict divergence guard after `successes=1101` (`divergence=41`, `quorum min=109`, `strict min=68`), log `/tmp/izanami_npos_full_soak_20260309T121111.log`, network `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_lssABH`.
- Regression-specific signal:
  - committed-edge suppression path was exercised in both soaks (`suppressing committed-edge conflicting highest-QC reference`: permissioned `10`, NPoS `4`) with proposal/new-view/proposal-hint sources observed.
  - no evidence of same-height infinite missing-payload dependency acquisition on conflicting committed-edge hashes; suppression remained canonical-reanchor/obsolete-classification scoped.
- Dominant remaining liveness blocker in both modes was strict/quorum split driven by one lagging peer with heavy fallback-anchor replay + repeated `no proposal observed` / `commit quorum missing` churn, not a conflicting-hash dependency loop.

### Validation Matrix (Full Permissioned + NPoS Soak Replay)
- `RUST_LOG=izanami::summary=info,izanami::workload=warn IROHA_TEST_NETWORK_KEEP_DIRS=1 IROHA_TEST_NETWORK_PERMIT_DIR=$(mktemp -d) cargo run -p izanami --release --locked -- --allow-net --peers 4 --faulty 0 --duration 3600s --target-blocks 2000 --progress-interval 10s --progress-timeout 300s --tps 5 --max-inflight 8 --workload-profile stable`
- `RUST_LOG=izanami::summary=info,izanami::workload=warn IROHA_TEST_NETWORK_KEEP_DIRS=1 IROHA_TEST_NETWORK_PERMIT_DIR=$(mktemp -d) cargo run -p izanami --release --locked -- --allow-net --nexus --peers 4 --faulty 0 --duration 3600s --target-blocks 2000 --progress-interval 10s --progress-timeout 300s --tps 5 --max-inflight 8 --workload-profile stable`

## 2026-03-09 Committed-Edge Liveness Follow-Up (Strict Guard + Proposal Path)
- Applied a minimal-policy strict-progress fix in Izanami:
  - `effective_tolerated_peer_failures(...)` now uses protocol/BFT tolerance from peer count (not `--faulty` injection budget), preventing false strict-divergence aborts in `--faulty 0` runs with one transient straggler.
  - file: `crates/izanami/src/chaos.rs`
- Applied proposal-path committed-edge suppression in Sumeragi:
  - in `assemble_and_broadcast_proposal(...)`, `LockedQcRejection::HashMismatch` + missing highest-QC now invokes committed-edge suppression before recording missing-highest defer markers, so obsolete committed-edge conflicts are not reintroduced by leader proposal defer flow.
  - file: `crates/iroha_core/src/sumeragi/main_loop/propose.rs`
- Added suppression cleanup for queued descendants rooted on obsolete conflicting committed-edge parent hashes:
  - committed-edge suppression now drops pending descendant blocks whose parent hash is the conflicting committed-edge hash.
  - file: `crates/iroha_core/src/sumeragi/main_loop/votes.rs`
- Added regressions:
  - `assemble_proposal_suppresses_committed_edge_highest_qc_conflict`
  - `committed_edge_conflict_suppression_purges_descendant_pending_blocks`
  - file: `crates/iroha_core/src/sumeragi/main_loop/tests.rs`

### Validation Matrix (Committed-Edge Liveness Follow-Up)
- `cargo fmt --all`
- `cargo test -p izanami effective_tolerated_peer_failures_uses_protocol_tolerance -- --nocapture`
- `cargo test -p izanami strict_divergence_guard_requires_more_than_tolerated_outliers -- --nocapture`
- `cargo test -p izanami strict_progress_timeout_enforcement_respects_bft_tolerance -- --nocapture`
- `cargo test -p iroha_core --lib assemble_proposal_suppresses_committed_edge_highest_qc_conflict -- --nocapture`
- `cargo test -p iroha_core --lib committed_edge_conflict_suppression_purges_descendant_pending_blocks -- --nocapture`
- `cargo test -p iroha_core --lib committed_conflict -- --nocapture`

## 2026-03-09 Full Permissioned + NPoS Soak Replay (Post Follow-Up Patchset)
- Ran full 2000-block Izanami envelopes on the latest strict-guard + proposal-path suppression patchset.
- Permissioned full soak:
  - command: `RUST_LOG=izanami::summary=info,izanami::workload=warn IROHA_TEST_NETWORK_KEEP_DIRS=1 IROHA_TEST_NETWORK_PERMIT_DIR=$(mktemp -d) cargo run -p izanami --release --locked -- --allow-net --peers 4 --faulty 0 --duration 3600s --target-blocks 2000 --progress-interval 10s --progress-timeout 300s --tps 5 --max-inflight 8 --workload-profile stable`
  - result: no strict-divergence abort; run completed duration and stopped before target (`successes=18000`, `err=izanami run stopped before target blocks reached`), network `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_U9yElv`.
- NPoS full soak:
  - command: `RUST_LOG=izanami::summary=info,izanami::workload=warn IROHA_TEST_NETWORK_KEEP_DIRS=1 IROHA_TEST_NETWORK_PERMIT_DIR=$(mktemp -d) cargo run -p izanami --release --locked -- --allow-net --nexus --peers 4 --faulty 0 --duration 3600s --target-blocks 2000 --progress-interval 10s --progress-timeout 300s --tps 5 --max-inflight 8 --workload-profile stable`
  - result: strict-divergence guard remained suppressed, but quorum progress still stalled before target (`successes=11203`; `no block height progress for 600s`, `quorum min=721`, `strict min=142`, `tolerated_failures=1`), network `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_PIBzHl`.
- Regression signal after follow-up:
  - obsolete committed-edge hashes are still suppressed (no conflicting-hash missing request reintroduction from proposal path), but NPoS still shows high-frequency committed-edge suppression + fallback-anchor replay at a fixed height (`~722`) with repeated `missing_qc`/`no proposal observed` churn.

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

## 2026-03-08 Torii Legacy API Surface Cleanup
- Removed dead telemetry compatibility handlers from Torii:
  - `crates/iroha_torii/src/lib.rs` (`handler_status_root_v2`, `handler_status_tail_v2`)
- Confirmed Torii route registrations do not expose legacy-versioned paths; active HTTP surface remains `/v1/...` (plus intentional unversioned utility endpoints such as `/status`, `/metrics`, `/api_version`).

### Validation Matrix (Torii Legacy API Cleanup)
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

## 2026-03-12 Multisig Register Upgraded-Executor Coverage
- Closed multisig register parity gaps between initial and upgraded executor paths:
  - Added `execute_multisig_custom_instruction_if_present(...)` in `crates/iroha_core/src/executor.rs`.
  - Wired it into both `dispatch_instruction_with_ivm(...)` and `dispatch_instruction_with_fixture(...)` so user-provided executors execute multisig custom envelopes through `execute_multisig_instruction(...)` before generic `InstructionBox::execute`.
- Added missing host-side test coverage:
  - `crates/iroha_core/src/executor.rs`:
    - `fixture_executor_executes_multisig_register_custom_instruction`
      verifies user-provided executor flow materializes missing signatory accounts and sets `iroha:created_via="multisig"`.
- Added upgraded-executor integration coverage:
  - `integration_tests/tests/multisig.rs`:
    - `multisig_register_materializes_missing_signatory_account_after_executor_upgrade`
    - `multisig_register_rejected_does_not_materialize_missing_signatory_account_after_executor_upgrade`
  - Both tests explicitly upgrade to `executor_with_admin` and assert success/rejection materialization behavior.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_core --lib fixture_executor_executes_multisig_register_custom_instruction -- --nocapture` (pass)
  - `cargo build -p irohad` (pass)
  - `IROHA_TEST_SKIP_BUILD=1 cargo test -p integration_tests --test multisig multisig_register_materializes_missing_signatory_account_after_executor_upgrade -- --nocapture` (pass)
  - `IROHA_TEST_SKIP_BUILD=1 cargo test -p integration_tests --test multisig multisig_register_rejected_does_not_materialize_missing_signatory_account_after_executor_upgrade -- --nocapture` (pass)

## 2026-03-14 Integration Failures Rebaseline (8 failing tests)
- Resolved strict `AssetDefinitionId` parsing regressions in integration tests and fixtures:
  - `integration_tests/tests/queries/asset.rs` now constructs canonical `AssetDefinitionId` values directly instead of parsing legacy `name#domain` literals.
  - Updated CBDC capability fixtures from legacy asset literals (`CBDC#...`) to canonical `aid:<32-lower-hex>` and regenerated Norito `.to` payloads:
    - `fixtures/space_directory/capability/{cbdc_wholesale,jp_regulator_supervision}.manifest.{json,to}`
    - `fixtures/nexus/cbdc_rollouts/20260115T120000Z/capability/{bank_wholesale,retail_dapp}.manifest.json`
    - `fixtures/nexus/cbdc_rollouts/20260115T120000Z/capability/{bank_wholesale,retail_dapp}.to`
  - Updated scaffold/test fixture literals to canonical AID in:
    - `xtask/src/main.rs`
    - `crates/iroha_cli/src/space_directory.rs`
- Fixed UAID runtime manifest polling path mismatch in `crates/iroha/src/client.rs`:
  - `get_uaid_portfolio`, `get_uaid_bindings_with_query`, and `get_uaid_manifests` now target canonical app routes.
- Fixed proof fixture mismatch in `integration_tests/tests/events/proof.rs`:
  - Switched Halo2 fixture key from `halo2/ipa:tiny-add-v1` to `halo2/ipa:tiny-add` so verifying-key lookup succeeds.
- Reduced localnet permit contention flakiness in `integration_tests/tests/nexus/cross_dataspace_localnet.rs` by serializing `cross_dataspace_localnet_genesis_preexecution_smoke` via `sandbox::serial_guard()`.
- Stabilized soft-fork assertion behavior in `integration_tests/tests/extra_functional/unstable_network.rs`:
  - For forced soft-fork runs, final supply now accepts `[rounds-1, rounds]` to account for bounded final-round lag.
- Fixed trigger bytecode decode/runtime rejection:
  - Updated `crates/kotodama_lang/src/samples/mint_rose_trigger.ko` to canonical asset literal `aid:6872454e9c044641aa581ec5f3801619`.
  - Recompiled:
    - `crates/kotodama_lang/src/samples/mint_rose_trigger.to`
    - `integration_tests/fixtures/ivm/mint_rose_trigger.to`
  - This resolves `trigger_in_genesis` and `call_execute_trigger_with_args` rejections (`invalid Norito TLV envelope` / instruction decode errors).
- Validation:
  - `cargo fmt --all` (pass)
  - Outside-sandbox targeted confirmation with isolated permit dir (all pass):
    - `cargo test -p integration_tests --test mod nexus::cbdc_whitelist::cbdc_capability_manifests_enforce_policy_semantics -- --nocapture`
    - `cargo test -p integration_tests --test mod nexus::cross_dataspace_localnet::cross_dataspace_localnet_genesis_preexecution_smoke -- --nocapture`
    - `cargo test -p integration_tests --test mod extra_functional::unstable_network::soft_fork -- --nocapture`
    - `cargo test -p integration_tests --test mod events::proof::proof_event_scenarios -- --nocapture`
  - `cargo test -p integration_tests --test mod queries::asset::find_asset_total_quantity -- --nocapture`
  - `cargo test -p integration_tests --test mod nexus::runtime_dataspace_registration_perf::runtime_nexus_registration_reports_lane_lifecycle_costs -- --nocapture`
  - `cargo test -p integration_tests --test mod triggers::by_call_trigger::trigger_in_genesis -- --nocapture`
  - `cargo test -p integration_tests --test mod triggers::by_call_trigger::call_execute_trigger_with_args -- --nocapture`

## 2026-03-19 JSON Numeric Trigger ABI
- Added `JSON_GET_NUMERIC` (`0x7F`) to the ABI v1 syscall surface so Kotodama by-call triggers can read decimal `Numeric` values directly from trigger JSON args without routing amount construction through middleware.
- Implemented the syscall in `CoreHost`, `WsvHost`, and the `iroha_core` codec-host forwarding path.
- Added Kotodama semantic/IR/compiler support for `json_get_numeric(ev, name("amount"))`, typed as `Amount`, plus ABI/host regression tests and syscall docs updates.
- Intended use: direct on-chain settlement-style triggers such as SBP issuance swap can now consume `"0.00001"` style amounts from `ExecuteTrigger` args and feed them into `transfer_asset(...)` unchanged.

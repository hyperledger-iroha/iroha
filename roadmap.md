# Roadmap (Open Work Only)

Last updated: 2026-05-08

Completed history lives in `status.md`. This file should only track unfinished work.

## Sumeragi vNext consensus replacement

- Finish moving proposal, DA/RBC availability, validation-worker start/result,
  commit-persistence, timeout-tick, and block-sync sidecar flows onto typed
  `sumeragi::vnext::ReactorEvent`/`ReactorEffect` adapters. vNext control
  frames now enter the live reactor, but the broader block consensus shell
  still needs the remaining effect adapters before the legacy cooperative
  tick/commit sweep and inline validation fallback can be deleted.
- Finish auditing chain-order hash and `rechain_seq` binding in deferred
  vote/QC caches, signer-tally/cache keys, and evidence replay paths used by
  the replacement shell. Vote/QC preimages, precommit signer history,
  block-sync-derived QCs, validator-checkpoint sidecars, raw/deferred vote
  caches, and vote/QC verifier cache keys now carry the selected binding.
- Reconstruct vNext chain order from committed/replayed re-chain and
  view-change certificates during block-sync catch-up. The live actor now keeps
  a bounded in-memory certificate journal; persistence/sidecar replay remains
  open.
- Add model and integration coverage for slow validation, queue saturation,
  malicious accusers, head failure during re-chain, NPoS stake-quorum
  quarantine edges, and DA/RBC loss during re-chain.

## Offline Note V2 wallet SDK completion

- Finish wallet reconciliation behind the one-call `OfflineNoteV2Wallet`
  facades:
  - replace the first-pass `sync()` no-op with transaction-outcome
    reconciliation for `CHANGE_PENDING`, `SPEND_PENDING`, and
    `REDEEM_PENDING` note records;
  - add duplicate-token, already-spent, failed-audit, and failed-redeem
    mock-transport regressions around that reconciliation.
- Add structured encrypted Offline Note V2 wallet-note stores:
  Android Keystore-backed secure storage in the platform module and Swift
  Keychain-backed storage modeled after `ConnectKeyStore`. Kotlin/JVM, Java
  Android, and Swift now have structured in-memory stores for SDK tests.
- Add public Norito decoders for Offline Note V2 key certificates, issued
  claims, redeem payloads, audit bundles, and payment tokens.

## Validation corridor

- Carry the Sumeragi NPoS/permissioned QC and VRF hardening through the next
  full workspace corridor.
  - A 2026-05-05 workspace rerun exposed three remaining
    `consensus_and_da` cases after the UAID replay/checkpoint fixes:
    stale evidence persistence, NPoS baseline timing, and late VRF reveal
    penalty recovery. The stale-evidence and NPoS performance focused reruns
    are green after the Torii horizon filter and baseline budget update. The
    late-reveal path now has code-level fixes for VRF vote-queue routing and
    deferring committed-block catch-up until after VRF metadata handling,
    epoch-record hydration before reveal validation, stale pending-seal
    retention, and external Torii VRF metadata gossip. The focused core units
    for those paths are green. The remaining integration blocker is now a
    separate four-peer NPoS/DA liveness stall: the late reveal is accepted in
    Sumeragi status, but the network repeatedly stalls at height 4 with RBC
    READY/DELIVER data waiting on missing INIT/chunk state before the pending
    VRF seal can be committed. Fix that h4 DA/RBC stall, then rerun
    `sumeragi_randomness::npos_late_vrf_reveal_clears_penalty_and_preserves_seed`
    as the final persistence gate.
  - Focused commit, block-sync, VRF, QC-validation, roster-selection, Torii VRF
    OpenAPI/parser, and data-model consensus roundtrip tests are green as of
    2026-05-02 with `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-verify`.
  - Additional Sumeragi/DA adversarial coverage is green as of 2026-05-02 with
    `CARGO_TARGET_DIR=/tmp/iroha-codex-workspace-corridor` for the debug
    witness-root unit, witness-corruption recovery, chunk-drop recovery,
    Kura-eviction DA rehydration, and block-body DA rehydration focused
    reruns. The remaining broad-run Sumeragi DA payload-loss case is also green
    as of 2026-05-03 with the same target dir.
  - NewView QC `highest_qc` binding, exact local-vote `highest_qc` and
    parent/post-root matching, non-NewView `highest_qc` rejection, and
    same-highest aggregate formation are green as of 2026-05-03 with
    `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-highest` for the NPoS
    aggregate-only substitution regression, the `new_view_highest` focused
    slice, and the stale/future NewView QC formation regressions. The same
    target now also covers commit/checkpoint missing-PoP rejection, block-sync
    QC validation with commit-phase enforcement, commit-certificate roster
    validation, checkpoint roster validation, validation telemetry reason
    labels, and the permissioned/NPoS aggregate-fallback quorum checks.
    Embedded commit-QC roster anchoring is green as of 2026-05-04 in the same
    target for both the malicious shrink-roster rejection and the valid
    stale-cache bootstrap path; the embedded-roster missing-PoP rejection is
    green in that same filter. NPoS block-sync roster selection now also has
    focused coverage for carrying a locally resolved stake snapshot when the
    incoming QC/checkpoint hint omits one.
  - The ZK-confidential localnet submit helper has been hardened for startup
    transport jitter and wrapped policy rejections. The classifier/retry-budget
    tests plus disabled shield/unshield localnet regressions are green as of
    2026-05-03 with `CARGO_TARGET_DIR=/tmp/iroha-codex-uaid-target`. The full
    serial `consensus_and_da` target is also green in the same target dir:
    `250` passed, `0` failed, `6` ignored. Focused strict clippy over
    `iroha_core`, `iroha_torii`, `iroha_test_network`, and the
    `consensus_and_da` test target is also green in that target dir.
  - Focused `cargo clippy -p iroha_core -p iroha_data_model -p iroha_torii -p
    irohad --all-targets -- -D warnings` is green as of 2026-05-02 with
    `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-clippy`.
  - Full workspace all-target clippy is green as of 2026-05-03 with
    `CARGO_TARGET_DIR=/tmp/iroha-codex-uaid-target`.
  - The broad workspace test rerun reached `events_and_triggers` after passing
    `consensus_and_da` and `core_api`; the exposed by-call trigger fixture and
    subscription time-trigger billing failures are repaired as of 2026-05-03.
    Focused `events_and_triggers` reruns for the two by-call trigger cases and
    `subscriptions::subscription_scenarios` are green with
    `CARGO_TARGET_DIR=/tmp/iroha-codex-uaid-target`.
    The full `events_and_triggers` target, full `queries_and_proofs` target,
    `network_functional::extra_functional::unstable_network`, full
    `nexus_and_streaming` target, and reduced-sample ignored
    `torii_load_profile` are also green as of 2026-05-04 in the same target
    dir. The stale IVM/Kotodama, Space Directory, lane commitment, Norito
    instruction, and streaming RANS fixtures uncovered by those targets have
    been regenerated.
    The full `core_api` target is green again as of 2026-05-04 after repairing
    private-entrypoint hash handling and widening the slow asset/sealed-reveal
    liveness paths (`171` passed, `4` ignored).
    A broad `cargo test --workspace` reached `integration_tests --lib` after
    compiling the workspace and passing the preceding crate/test targets; the
    first integration-library pass failed on a stale spawned daemon artifact,
    then the exact startup/drop regressions and the full integration library
    passed after rebuild (`41` passed). The core signature slice, crypto
    Ed25519 tests, and strict clippy for core/crypto/integration are also green
    after the deterministic single-Ed25519 verifier cleanup and heartbeat
    execution-context fixture repair. The replay/checkpoint follow-up is green
    as of 2026-05-05 for the focused replay units, Halo2 restart-marker
    verifier, strict core/crypto/consensus integration clippy, and the
    previously failing `consensus_and_da` restart/localnet cases:
    `sumeragi_restart_retains_lock_convergence`,
    `npos_pacemaker_resumes_after_downtime`,
    `confidential_combined_peer_downtime_and_timeout_pressure_localnet`, and
    `confidential_dual_restart_stress_mid_flow_localnet`.
    The 2026-05-07 follow-up also has focused green reruns for the staged
    consensus failures exposed in the latest broad workspace attempt:
    selective-drop recovery, conflicting-ready invalidation, Kura eviction DA
    rehydration, NPoS baseline metrics, pacemaker latency, pacemaker restart
    liveness, stale-evidence rejection, and the VRF randomness module. The
    focused `integration_tests --test consensus_and_da` compile check is green
    in `CARGO_TARGET_DIR=/tmp/iroha-codex-keepgoing-workspace-check`.
  - Remaining validation: rerun `cargo test --workspace` from a clean start to
    completion in an uncontended multi-hour window.
  - Broad workspace all-target compile validation is green as of 2026-05-07
    with `CARGO_TARGET_DIR=/tmp/iroha-codex-keepgoing-workspace-check` after
    repairing the default Linux monitor synth gate and stale
    `LaneBlockCommitment` fixture initializers in Python/`xtask`.
- Carry the RAM-LFE API/proof hardening through the remaining signing and clean
  full-workspace Cargo corridor.
  - Focused OpenAPI detached-envelope tests, crypto RAM-LFE tests, the new
    state-deserialization policy regression, Torii RAM-LFE handler tests,
    JavaScript RAM-LFE tests, Swift execute-response parsing, the focused
    `iroha_core` RAM-LFE gate, the workspace all-target compile corridor,
    focused strict clippy over the repaired tool/SDK/Mochi/CoreHost/proof
    targets, JavaScript/Kotlin/JVM/Java Android identifier BFV parity,
    JavaScript Connect Norito schema-hash parity, Android Norito schema-manifest
    verification, C# SDK tests on macOS with a temporary .NET 8 SDK, full Swift
    package tests, full workspace all-target clippy, `scripts/check_no_scale.sh`,
    formatting, and diff whitespace checks are green as of 2026-05-02.
  - Remaining validation: run `cargo test --workspace` in an uncontended
    validation window.
  - Windows C# follow-up: on a Windows box with .NET 8, run
    `dotnet restore csharp/Hyperledger.Iroha.Sdk.sln`, then
    `dotnet test csharp/tests/Hyperledger.Iroha.Sdk.Tests/Hyperledger.Iroha.Sdk.Tests.csproj`.
    Confirm the canonical Norito schema-hash test, transaction-builder goldens,
    faucet PoW vectors, and URL escaping expectations pass unchanged; record the
    Windows result in `status.md`.
  - Focused Kotlin/JVM and Java Android RAM-LFE parser/transport tests are
    green as of 2026-05-02 with Homebrew OpenJDK 21 pinned via `JAVA_HOME`; the
    same harnesses also cover the canonical BFV identifier schema-hash vector.
  - An operator still needs to provide either the OpenAPI signing key or a
    detached Ed25519 signature envelope for the exact canonical `torii.json`
    bytes so the static OpenAPI manifest can be regenerated and verified.
- Carry the FastPQ V1 verifier/AXT binding hardening through the next clean
  validation corridor.
  - `cargo test -p fastpq_prover --lib`,
    `cargo check -p fastpq_prover --bins --lib`, and
    `cargo check -p iroha_core --lib` are green as of 2026-05-02.
    Focused verifier slices `verify_rejects`, `verify_limits`, and
    `verify_fri_query_chain` are also green.
  - The V1 verifier now applies a proof-size ceiling before canonical replay
    work, rejects batch/proof parameter mismatch, enforces the exact FRI layer
    schedule for the proof domain and arity, and requires arity-sized sampled
    FRI round openings. It also checks canonical LDE chunk lengths,
    LDE/AIR/FRI Merkle path depths, and terminal FRI leaf shapes before replay.
  - AXT proof envelopes now require FastPQ V1 verifier labels at both the
    production FastPQ binding layer and the standalone IVM host envelope-shape
    layer. DefaultHost, CoreHost, and WSVHost reject raw proof bytes and
    synthetic/non-V1 proof labels during diagnostic preflight. Because
    standalone IVM does not link a real FastPQ verifier, proof-consuming AXT
    calls fail closed after preflight; the production AXT verifier rejects oversized
    encoded proof payloads before Norito decode. The descriptor-derived
    synthetic AXT batch builder and CLI fallback have been removed; proof
    generation and measurement require an execution-captured `batch_base64`
    request field. Core state no longer synthesizes FastPQ batch hashes for
    ad-hoc transcript/RWA contexts; those paths require transaction call-hash
    context or a trigger-specific call hash. The shared preflight checks also
    require concrete binding fields,
    supported FastPQ claim types, 32-byte hex digests, and nonempty
    duplicate-free target dataspace sets. DefaultHost also binds handles to the
    manifest root carried by inline, recorded, or late proof envelopes before
    failing closed without a verifier. The focused `fastpq_prover` AXT binding slice,
    `iroha_data_model` `proof_matches_manifest` slice, `ivm_abi`
    `preflight_fastpq_v1_proof_envelope` test, `ivm` `axt_host_flow` target, and
    `ivm` `host_unknown_syscall`/`core_host_policy` targets are green as of
    2026-05-02.
  - CoreHost raw-root rejection and real FastPQ proof-envelope validation is
    covered by the focused `ivm_corehost_axt` proof-binding test with
    `iroha-core-tests,app_api`; the correctly-featured target is green as of
    2026-05-03 with `28` tests. The `app_api`-only command lists zero tests and
    should not be treated as coverage for this target.
  - Block-level app-API AXT validation and host proof-cache success fixtures now
    use reusable FastPQ-backed proof envelopes. The full
    `axt_validation_tests` module and focused `axt_verify_ds_proof` host sweep
    are green as of 2026-05-02.
  - Shared `ProofBlob` matching, standalone `ivm` CoreHost/WSV tests, state
    replay-ledger fixtures, ISI lane-relay registration, and data-model AXT
    fixtures now reject raw manifest roots and binding-less success envelopes;
    only malformed-negative tests keep those payloads.
  - Lane relay proof metadata has no legacy deterministic digest helper and
    carries a required `verified_at_height` field. Verified lane-relay
    registration binds the envelope digest to the submitted proof blob payload
    hash; the data-model proof-material tests and core `lane_relay` slice are
    green as of 2026-05-02.
  - Replace the current prover-scale canonical replay verifier with a succinct
    quotient-only verifier once the V1 quotient commitment/opening API lands;
    this is a performance follow-up, not permission to accept synthetic AXT or
    placeholder proofs.
- Carry the SoraNet VPN escrow hardening through the remaining ledger and
  deployment corridor.
  - The Torii/relay/helper control plane now requires XOR quote payments,
    non-operator escrow custody, client usage vouchers, one-use helper tickets,
    and relay TLS pinning.
  - Native lease escrow ISIs, WSV lease records, verified tariff settlement,
    relay/helper streaming voucher debt-window enforcement, Torii native
    `OpenVpnLeaseEscrow` quote skeletons, and Torii native `SettleVpnLease`
    receipt skeleton responses through the generic `tx_instructions`
    tooling convention are implemented.
  - Relay operators can set `vpn.receipt_spool_dir` to persist the exact
    `/v1/vpn/receipts` request body for voucher-backed sessions, so settlement
    no longer depends on reconstructing receipt bytes from logs.
  - `soranet-vpn-settlement` consumes those artifacts and signs deterministic
    Torii receipt headers/body, or renders curl, using runtime-only operator seed
    material.
  - The JavaScript, C#, Swift, Python, Kotlin/JVM, and Java Android Torii
    clients now expose the quote-first open flow and operator receipt
    submission helpers with native instruction skeletons.
  - Next, run a public relay/helper/Torii canary that opens a native XOR VPN
    lease from the wallet flow, submits a spooled operator receipt, and signs the
    returned `SettleVpnLease` transaction.
- Carry the IVM/Kotodama vector and syscall hardening through the next clean
  validation corridor.
  - `cargo test -p ivm_abi`,
    `cargo test -p ivm --test vector_execution_regression`, and
    `cargo test -p kotodama_lang vector_length` are green as of 2026-05-02.
  - The updated IVM gas/metadata/pointer window is also green as of
    2026-05-02:
    `cargo test -p ivm --test gas_conformance --test gas_golden --test metadata --test metadata_roundtrip --test pointer_tlv_neg`.
  - The focused analyzer regression
    `cargo test -p ivm analysis_treats_setvl_operand_as_immediate --lib` is
    green as of 2026-05-02.
  - The SCALLX ABI expansion is green as of 2026-05-02 for
    `cargo test -p ivm --lib ivm_is_send_sync_for_state_sharing`,
    `cargo test -p ivm --lib scallx_dispatches_extended_syscall_id`,
    `cargo test -p ivm --test abi_hash_versions --test gas_schedule_hash --test syscalls_doc_sync --test ivm_abi_doc_sync`, and
    `cargo test -p ivm_abi --lib syscallx_roundtrips_24_bit_number`, all with
    `CARGO_TARGET_DIR=target/codex-ivm-scallx`; the core admission regression
    `cargo test -p iroha_core validate_ivm_unknown_scallx_rejected_at_admission --lib`
    is green with `CARGO_TARGET_DIR=target/codex-core-scallx`.
    Follow-up host-bound coverage
    `cargo test -p ivm --lib ivm_is_send_sync_for_state_sharing`,
    `cargo test -p ivm --lib run_with_host_accepts_non_sync_host`, and
    `cargo test -p ivm --lib block_height_syscall_uses_configured_deterministic_value`
    is green with `CARGO_TARGET_DIR=target/codex-ivm-scallx`. Core host
    coverage for `dedicated_query_syscalls_return_norito_payloads`,
    `block_height_sysvar_uses_attached_transaction_context`, and scoped
    durable-state `STATE_KEYS`/`STATE_HAS`/`STATE_LEN`/`STATE_COUNT`
    tombstone resolution is
    green with `CARGO_TARGET_DIR=target/codex-core-scallx`.
    Broader IVM validation is also green with
    `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --lib` plus
    targeted integration batches for gas/opcode/vector, metadata/pointer ABI,
    predecoder/doc sync, syscalls, WSV-host flows, VRF, and ZK verifier gates.
  - Dedicated `QUERY_GET_ACCOUNT`, `QUERY_GET_ASSET`,
    `QUERY_GET_ASSET_DEFINITION`, `QUERY_GET_DOMAIN`,
    `QUERY_GET_CONTRACT_MANIFEST`, `QUERY_GET_NFT`, `QUERY_GET_PARAMETER`,
    and `QUERY_GET_CONTRACT_INSTANCE` are implemented. The helpers either use
    the validated query engine or deterministic attached-state snapshots, and
    all charge the singular query gas model in code and generated docs.
    `SYSVAR_BLOCK_HEIGHT` is threaded through default hosts and attached core
    query-state contexts. `STATE_KEYS` now provides deterministic durable-state
    prefix enumeration with pagination and contract-scope prefix stripping.
    `STATE_HAS`/`STATE_LEN` provide cheap presence and payload-length probes,
    and `STATE_COUNT` counts matching durable-state keys without returning the
    key list over the same scoped durable-state resolution. Classic `STATE_GET`,
    `STATE_SET`, and `STATE_DEL` now charge documented deterministic state gas
    instead of returning zero. `GET_ACCOUNT_BALANCE` and
    `RESOLVE_ACCOUNT_ALIAS` also return deterministic nonzero query-style gas.
    `TLV_EQ` and `TLV_LEN` now charge deterministic byte-counted codec-helper
    gas costs instead of inspecting potentially large payloads for free.
    Numeric helpers now charge the fixed `G_numeric` cost across default, WSV,
    standalone codec, and real-host forwarding paths. `POINTER_TO_NORITO` and
    `POINTER_FROM_NORITO` now charge `G_pointer + bytes` across the default,
    WSV, and standalone codec hosts, with the byte component tied to the
    canonical TLV envelope copied or validated. Schema helpers and the
    remaining classic codec helpers now charge deterministic byte-counted gas:
    `SCHEMA_*`, `JSON_*`, `DECODE_INT`/`ENCODE_INT`, `NAME_DECODE`, and the
    path builders no longer return zero for payload work. `SM2_VERIFY` now
    charges `G_verify + bytes`; `SM4_GCM_*` and `SM4_CCM_*` now charge
    `G_sm4 + bytes` through the shared default-host implementation, preserving
    deterministic vector output while charging AAD and plaintext/ciphertext
    bytes. Deterministic sysvar reads now charge
    `G_sysvar` or `G_sysvar + bytes`, and authority reads charge
    `G_get_auth + bytes`, across default, WSV, standalone codec, and real-host
	    paths. VRF verification now charges `G_verify + bytes` on decoded
	    status-returning paths, and standalone/WSV ZK verification status exits now
	    charge payload-size verification gas instead of returning zero. ZK
	    roots/tally reads and VRF epoch-seed reads now charge request + response
	    byte gas across standalone, WSV, and real CoreHost paths.
	    `VERIFY_DS_PROOF` now charges `G_verify + bytes` in the real
	    smart-contract host and `G_verify` for proof-clear paths across real,
	    default, standalone CoreHost, and WSV mock hosts while standalone
	    proof-consuming AXT calls remain fail-closed without the real FastPQ
	    verifier. Runtime helper syscalls now also avoid documented zero-gas
	    gaps: `INPUT_PUBLISH_TLV`
    charges envelope bytes across default, WSV, and standalone CoreHost paths;
    `VERIFY_SIGNATURE` charges message/signature/key bytes; and private input,
    nullifier, output commit, heap growth, allocation shim, debug/exit/abort,
    validation-only ISI mutation stubs, FastPQ batch-entry/apply validation,
    and Merkle proof helpers return fixed, page, per-entry, or depth costs
    instead of zero. The WSV mock host direct mutation ABI surface, FastPQ
    transfer batch apply path, and development `SMARTCONTRACT_EXECUTE_QUERY` /
    `SMARTCONTRACT_EXECUTE_INSTRUCTION` JSON shims now also return deterministic
    query or mutation gas instead of treating mock-host state changes as free.
    The real smart-contract host now charges the declared `G_sc_depth` floor for
    `SET_SMARTCONTRACT_EXECUTION_DEPTH`, including the zero-depth no-op path,
    and the declared `G_create_nfts_all` floor for empty
    `CREATE_NFTS_FOR_ALL_USERS` snapshots.
    The classic hash helper surface now includes gas-charged SHA-256,
    SHA3-256, raw Blake2b-256, Keccak-256, and Iroha `Hash::new` syscalls
    routed through the real smart-contract host with byte-identical CPU or
    byte-equivalent accelerated output requirements.
  - `VERIFY_PROOF` now has a CoreHost implementation for
    `NoritoBytes(OpenVerifyEnvelope)` payloads backed by on-chain verifying-key
    registry prechecks and deterministic status-code returns; standalone IVM
    hosts continue to reject it without registry context. Acceleration status
    reporting now only marks CUDA parity as OK when the backend is usable after
    policy, hardware detection, and self-tests.
  - `PROVE_EXECUTION` now returns `NoritoBytes(ExecutionProof)` instead of a
    reserved stub. The proof summary commits to deterministic trace/log/root
    material with SHA-256 and is stable across repeated identical runs, while
    leaving room for later cryptographic prover backends to bind to the same
    public material. Focused unit, syscall, doc-sync, gas-doc, and `ivm_abi`
    regression checks are green with `CARGO_TARGET_DIR=target/codex-ivm-scallx`.
  - The broader `cargo test -p ivm` corridor is green as of 2026-05-02 after
    repairing the data-model compile blocker, refreshing the AXT fixture
    headers, and moving Kotodama test helpers to host-private SCALLX numbers.
    The optimized `cargo test -p ivm --test shifts_prop` focused rerun is also
    green.
  - The `ivm_contract_deploy` staged copy/register fixture tests are green as
    of 2026-05-07 after the literal-table padding repair:
    `cargo test -p iroha_cli --bin ivm_contract_deploy staged_ -- --nocapture`
    with `CARGO_TARGET_DIR=/tmp/iroha-codex-keepgoing-workspace-check`.
  - Follow-up widened checks are green as of 2026-05-02:
    `cargo test -p ivm_abi`, `cargo test -p kotodama_lang`,
    `cargo clippy -p ivm_abi -p kotodama_lang --all-targets -- -D warnings`,
    and `cargo clippy -p ivm --all-targets -- -D warnings`.
  - Fold the 2026-05-03 Kotodama access-hint, contract artifact registry, and
    literal-padding hardening through the next clean full workspace test and
    clippy corridor after the focused validation recorded in `status.md`.
  - Fold the 2026-05-03 IVM ABI v1 gas/error hardening through the next full
    workspace test and strict clippy corridor after the focused syscall-doc,
    host-policy, AXT, and Soracloud validation recorded in `status.md`.
- Carry the UAID onboarding hardening through the next workspace validation
  corridor.
  - Focused formatting, Python syntax checks, Torii UAID parser tests, Torii
    MCP shortcut/raw-body tests, Torii HTTP onboarding negative-contract tests,
    the full Torii onboarding integration target, Torii onboarding
    error-metadata tests, Swift register-account tests, focused IVM host
    thread-safety tests, the OpenAPI sync/version/signature script tests, the
    focused core UAID portfolio grouping test, the DA manifest fixture sweep,
    `cargo test -p iroha_torii --lib --features app_api`, and
    `cargo check --workspace --all-targets` are green as of 2026-05-02. The
    full workspace all-target clippy corridor is also green as of 2026-05-02
    with `CARGO_TARGET_DIR=/tmp/iroha-codex-uaid-target`.
  - The Rust implementation is in place for explicit UAID-only onboarding,
    digest-only identity commitments, MCP/OpenAPI request contracts, Swift
    request canonicalization, asset-scope-aware UAID portfolio grouping, and
    stale OpenAPI manifest-signature suppression in generated version indexes;
    `versions.json` has been refreshed in explicit unsigned mode pending the
    operator signature.
  - Keep the broader `cargo test --workspace` corridor open. The repaired
    `events_and_triggers`, `queries_and_proofs`, `nexus_and_streaming`,
    unstable-network, and `core_api` targets are green individually as of
    2026-05-04. The Sora governance runtime-upgrade path now hashes prepared
    transaction entrypoints from the actual canonical signed payload bytes and
    confirms Torii status with explicit auto scope, but the full workspace
    command still needs an uncontended end-to-end pass. The static OpenAPI JSON
    and version index are refreshed in explicit unsigned mode; an operator
    still needs to regenerate the manifest with the OpenAPI signing key or
    detached signature envelope.
- Carry the Torii exposure-hardening slice through the next clean Cargo
  validation corridor.
  - `cargo fmt --all` and `cargo check -p iroha_config -p iroha_torii` are
    green as of 2026-05-02 for the CORS/pre-auth, MCP tool-effect,
    protected-namespace, route-catchall, mixed-content extractor, and router
    composition changes.
  - `cargo test -p iroha_config torii_cors_parse --lib` and
    `cargo test -p iroha_torii tool_effects --lib` are green as of
    2026-05-02. The follow-up MCP effect audit is also green for
    `cargo test -p iroha_torii get_tools_are_declared_read_effect --lib`,
    `cargo test -p iroha_torii manual_sumeragi_snapshot_tools_remain_read_only --lib`,
    and `cargo test -p iroha_torii tool_effects --lib` with
    `CARGO_TARGET_DIR=/tmp/iroha-codex-torii-continue`. Fold the slice into the
    next workspace clippy/test corridor when validation budget allows.
- Carry the Iroha Connect hardening through the remaining SDK and workspace
  validation corridor.
  - P2P session claims, hashed token storage, focused Rust checks, JavaScript
    checks, JS `dist`, Python syntax checks, and shared relay-auth vectors are
    green as of 2026-05-01.
  - Python pytest, Kotlin/JVM, Java Android, and Swift package tests remain
    blocked by missing local tools/artifacts.
  - When the validation shell has `pytest`, a Java runtime, and
    `dist/NoritoBridge.xcframework`, rerun the focused Python Connect tests,
    `./gradlew :core-jvm:test --tests org.hyperledger.iroha.sdk.connect.ConnectWalletRequestTest --console=plain`,
    the matching Java Android Connect wallet tests, and the focused Swift
    Connect/Torii tests.
  - Fold the Connect session/relay changes into the next broader
    `cargo test -p iroha_torii`, `cargo test --workspace`, and workspace clippy
    corridor when validation budget allows.
- Carry Offline V2 real-proof support through the remaining release corridor.
  - The native bridge prover FFI focused corridor is green as of 2026-04-30. Fold it into a broader `cargo test -p iroha_core --lib`, SDK test, and workspace clippy corridor when validation budget allows.
  - The pure Swift Offline V2 prover hot path is green as of 2026-05-01 with
    subsecond median native audit/redeem proofs on macOS arm64. Keep that
    benchmark in the next iOS-device corridor and broaden Swift package
    validation when budget allows.
  - Kotlin/JVM and Java Android now have the native Offline V2 instance-value
    groundwork and pure Java Halo2/IPA prover path, including focused JVM and
    Android harness coverage plus env-gated benchmark hooks. Keep the native
    prover tests, Swift/JVM cross-verification payload, and larger benchmark
    iteration counts in the next device and full-SDK corridor.
  - The Torii Offline V2 issuer hardening focused corridor is green as of
    2026-05-01. Fold it into the next broader `cargo test -p iroha_torii`,
    SDK, workspace test, and workspace clippy corridor when validation budget
    allows.
- Carry native asset escrow through the remaining Aitai application corridor.
  - Wire the Sora Aitai application UI/backend onto the native numeric escrow ISIs and proof-carrying anonymous escrow helper surfaces, then subscribe through the numeric and anonymous escrow query/event APIs.
  - Add app-facing lifecycle events for transparent and shielded offer state changes, and keep any remaining Kotodama wrapper work scoped to app calls that still need contract compatibility.
  - Add end-to-end UI/client smoke coverage once the Sora Aitai application replaces the old contract escrow account path for both transparent XOR and shielded anonymous-asset offers.
  - Rerun the full Kotlin, Java Android, and Swift SDK suites after the Aitai app wiring lands and a Java 21 runtime is available in the validation shell.
  - Keep NFT/RWA escrow and court fee/payout generalization as separate follow-ups; the v1 primitive intentionally resolves only between the escrow seller and accepted buyer.
- Carry the Soracloud production posture hardening through the operator-host rollout corridor.
  - Local focused, portable QEMU, and prior multi-peer load gates are green as of 2026-04-25; the readiness runner now reports missing operator inventory and missing observability evidence as production blockers. Before public rollout, run the mixed-host Inrou smoke with the real operator inventory, attach the real metrics/status/alert/dashboard evidence, and archive a blocker-free readiness report.
  - The full `irohad` Soracloud binary filter is green as of 2026-05-05 under
    `--features embedded-soracloud-runtime`. The full readiness profile still
    requires operator mixed-host inventory and observability evidence before it
    can produce a blocker-free rollout report.
- Carry the new Taira devex CLI through the opt-in live rollout corridor.
  - The local CLI/Torii/mock-script validation for `iroha taira doctor` and `iroha taira write-canary` is green as of 2026-04-25, but no live Taira write was run from this tree.
  - Before publishing a live receipt, run `iroha taira doctor --public-root https://taira.sora.org` and an operator-approved `iroha taira write-canary --public-root https://taira.sora.org`, preserving only the redacted receipt and any stable failure codes.
  - Fold the Taira CLI/Torii changes into the next broader `cargo test -p iroha_cli`, `cargo test -p iroha_torii`, workspace test, and clippy corridor when validation budget allows.
- Carry the verified lane relay JSON-state/key change through the next UC6 integration corridor.
  - The focused crate checks are green as of 2026-04-24, but no live UC6 settlement-smoke run or topology reset has been performed from this tree.
  - Before any live deployment, confirm the deploy/Core API smoke path still uses `relay_state_key`, JSON relay state, and the simulation gate against the exact finalization payload.
  - If a topology plan selects reset mode while validating this change, stop before approval and reassess the rollout scope.
- Carry the Torii routed-read and telemetry fixes through the next workspace validation corridor.
  - The crate-local sweep is green as of 2026-04-24 with `cargo test -p iroha_torii --lib --features app_api,telemetry -- --nocapture`.
  - When validation budget allows, carry the alias-routing and Torii telemetry slices through the next `cargo test --workspace` / `cargo clippy --workspace --all-targets -- -D warnings` corridor and record the result in `status.md`.
- Broaden validation for the new canonical account-alias lease flow beyond the focused onboarding and executor checks.
  - The onboarding auto-renew path now grants the subscriber `CanModifyNftMetadata` for the subscription NFT before trigger registration; rerun a wider `cargo test -p iroha_torii` window with the new `/v1/accounts/{account_id}/aliases`, `/renew`, and `/auto-renew` handlers enabled.
  - Add or rerun focused coverage for user-signed enable/disable mutation flows and the SNS subscription auto-renew billing path in `crates/iroha_core/src/smartcontracts/ivm/host.rs`, not just the onboarding enqueue path.
  - Once the alias lease slice is stable under those focused reruns, fold it into the next broader `cargo test --workspace` / `cargo clippy --workspace --all-targets -- -D warnings` corridor.
- Keep the Sumeragi main-loop broad corridor attached to future consensus
  changes.
  - The 2026-05-03 `cargo test -p iroha_core --lib` rerun is green
    (`5129` passed, `22` ignored) after fixing execution-witness recorder
    isolation and hardening the RBC sidecar cooldown fixture.
  - The later 2026-05-03 restarted-peer commit-QC recovery fix is covered by
    focused block-body response regressions and the confidential downtime plus
    timeout localnet scenario, now passing without the restarted-peer catch-up
    waiver warning. Rerun the full `cargo test -p iroha_core --lib` corridor
    after the next main-loop edit or before opening the next full workspace
    sweep.
  - For the next consensus change, rerun the same broad window so the collector
    fallback, exact-frontier repair, cached-target, vote replay, roster
    recovery, future-new-view, and model-backed reschedule fixtures continue to
    execute together rather than only as isolated filters.
- Broaden Sumeragi verification when new fatal hang classes are identified
  outside the current two-slot frontier abstraction.
  - The 2026-05-03 frontier formal process hardening is green and covers active
    pending progress touch, local-vote and commit-QC progress, stale recovery
    subject-view scope, vote-queue drain, payload recovery, quorum retransmit,
    retransmit follow-through, and future-slot promotion.
  - For any additional fatal hang shape, first add a focused Rust regression,
    then add the corresponding finite formal dimension or mutation so the
    expected-failure suite proves the model would have caught it.
  - If another restarted-peer catch-up issue appears in message admission or
    deduplication, add a small finite admission-order bridge or mutation before
    broadening the frontier model itself; the current model intentionally
    abstracts network-message dedup away.
  - Keep this scoped to the observed hang surface; do not generalize the model
    into an arbitrary pipeline unless a new bug requires more than the active
    plus one-future-slot abstraction.
- Reopen the wider validation corridor after the recent focused `iroha_core`, `iroha_torii`, and `iroha_data_model` test additions.
  - `cargo test -p iroha_core --lib` is green as of 2026-05-03; rerun it only
    after the next core/consensus change or before opening the full workspace
    corridor.
  - `cargo test -p iroha_torii` is green as of 2026-05-03 after fixing the
    macOS attachment-sanitizer subprocess wrapper path; rerun it after the next
    Torii/API change or before opening the full workspace corridor.
  - Rerun `cargo test -p integration_tests -- --nocapture` once the current
    tree is stable enough for network suites.
  - When validation budget allows, rerun `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings`, then capture failures or green status in `status.md`.
## Consensus and Izanami

- Maintain Izanami communication vulnerability publication evidence.
  - The exact-injector 75% packet-loss 2026-04-26 paper-shaped run at `dist/izanami-exact-packet-paper-20260426` is green for both permissioned and NPoS Sumeragi and is recorded in `status.md`; keep this as the current full-matrix resilience baseline.
  - Native in-process P2P packet-drop injection is wired into `packet-loss` and leader-targeted `leader-isolation`; the matrix runner now supports the paper's 133s-266s timed fault window plus configurable packet-loss sweeps (`75%` quick, `25%/50%/75%` paper). The explicit 25%/50%/75% paper packet-loss sweep at `dist/izanami-packet-sweep-paper-20260427-loss-only` is green for both permissioned and NPoS Sumeragi and is recorded in `status.md`.
  - The 2026-04-27 quick matrix at `dist/izanami-quick-both-20260427` is green for all ten permissioned/NPoS rows, and the post-ingress-hardening leader-isolation rerun at `dist/izanami-quick-leader-retry-20260427` keeps both modes resilient with zero acceptance markers.
  - The result-strengthened matrix and sweep tooling is implemented as of 2026-04-28, including bounded shutdown-drain accounting, latency/recovery evidence, NPoS repair-coverage telemetry, generated `paper-style-final-report.md`, and separate `stress-400` / `stress-800` profiles.
  - Seed-7 real stress evidence at `dist/izanami-stress-400-seed7-20260428` and `dist/izanami-stress-800-seed7-20260428` is refreshed and green as of 2026-04-29: both `stress-400` and `stress-800` report 14/14 resilient rows across permissioned and NPoS Sumeragi, with no real `confirmation_queue_dropped` pressure in the fresh artifacts. This is recorded in `status.md`.
  - Run the full paper/stress seed sweep with fresh binaries when validation budget allows: `scripts/run_izanami_communication_vulnerability_sweep.sh --profiles paper,stress-400,stress-800 --sumeragi-mode both --seed-list 7,11,13,17,19,23,29,31,37,41`. Paper rows must remain `resilient`; stress rows should stay reported separately as margin evidence across broader seeds.
  - Keep any future publication reruns split with `--sumeragi-mode both` so permissioned and NPoS Sumeragi classifications are not collapsed, and preserve per-loss packet-loss subrows when comparing against the paper's Algorand/Aptos/Avalanche/Redbelly/Solana baseline.
- Recalibrate the Izanami stable-profile acceptance envelope for sustained workload targets.
  - The fresh 4-peer permissioned `1 TPS` / `300s` / `100 blocks` gate at `dist/izanami-stable-gate-20260427-target100` is green and recorded in `status.md`.
  - The matching `200`-block diagnostic at `dist/izanami-stable-gate-20260427-rerun` crossed the prior stall region and reached strict/quorum height `107` with zero submission or confirmation failures, but missed the target because the stable workload drained before `200` blocks.
  - Before the longer `3600s` / `2000+` block acceptance pass, choose a sustained-workload gate or lower short-run target so the KPI measures liveness instead of exhaustion of submitted work.
- Root-cause the remaining NPoS soak/localnet collapse instead of keeping it as a log-only symptom.
  - Reproduce with preserved peer dirs and `iroha_futures::supervisor=debug`.
  - Identify the first exiting supervised child before investigating downstream connection refusals.
  - Cross-check peer logs with `/v1/sumeragi/status` counters so the fix targets the actual failing layer.

## Throughput and query performance

- Re-establish current throughput knees for the de-amplified harness and shared-host localnet.
  - Rerun the stepped single-host sweep.
  - Repeat permissioned and NPoS passes on the same hardware envelope and compare against the archived `25-50 TPS` / `75-100 TPS` baselines.
  - Record the new knee points and any regressions in `status.md`.
- Carry the 2026-05-02 Norito/Crypto scalar hot-path slice through the remaining
  release validation corridor.
  - The Ed25519 admission follow-up now caches deterministic 32-byte invalid
    public-key parse outcomes, routes compact/full conversion through the
    cached parse path, widens the hot thread-local parse/verify caches for the
    20k stable workload window, skips signature parsing and dalek batch setup
    for all-cached exact verify tuples, and preserves lowest-original-index
    failure reporting for mixed batches. Focused crypto/Torii checks and the
    `ed25519_hotpaths` Criterion bench are recorded in `status.md`.
  - Remaining local benchmark baselines: `cargo bench -p iroha_data_model
    --bench chain_wire`, `cargo bench -p iroha_data_model --bench
    decode_registry`, and `cargo bench -p iroha_core --bench crypto_hotpaths`.
  - The latest 120s release gate rerun exists at
    `dist/izanami-prebuilt-20k-rerun-release-ed25519-cache-120s-20260502-180614`
    and is recorded in `status.md`; the wrapper exited `0`, but it is not a
    clean all-accepted ingress gate. It offered all `2,400,000` planned
    submissions, accepted `2,364,756`, reported `35,244` failures, and reached
    strict approved transactions `20,582` at strict height `7`, with the queue
    still saturated. Active build/gate process lines were captured before and
    after the run, so it remains diagnostic evidence only.
  - The latest contended 30s sampled profile exists at
    `dist/izanami-profile-20k-ed25519-cache-sampled3-30s-20260502-182524`
    and is recorded in `status.md`. It submitted and accepted all `600,000`
    planned ingress attempts but only reached strict approved transactions
    `4,113` at strict height `3`, with the queue still saturated. The next
    bottleneck focus remains peer CPU: FASTPQ transcript finalization over
    Norito account/numeric/array serialization into Poseidon byte hashing;
    Ed25519/Curve25519 batch-verifier miss work and public-key parse/decode
    misses; Norito transaction/signature decode and compact-length work; and
    smaller allocation/copy/CRC64 costs. It is not a clean comparable baseline
    because workspace `cargo test`/rustc and another debug test network were
    active before and after the run.
  - The FASTPQ GPU follow-up is now recorded in `status.md`: Metal toolchain
    preflight is green, `bn254_poseidon_words` uses the Metal backend,
    transcript digest finalization overlaps Metal dispatch with CPU work,
    execution-witness digest propagation avoids a duplicate witness-side
    finalization, the final `fastpq-gpu` 120s release gate accepted all
    `2,400,000` offered submissions and reached `36,986` strict-approved
    transactions, and the delayed load-window sampled peer stacks have no
    scalar `poseidon3_permute` or CPU FASTPQ fallback. Keep CUDA runtime
    parity/perf validation as a separate CUDA-host follow-up; macOS
    compile/manifest coverage is not CUDA hardware evidence.
  - The 2026-05-05 hardware-backed FASTPQ Metal parity rerun on macOS is green
    after repairing Goldilocks FFT/LDE, BN254 LDE, and Poseidon Metal/CPU
    mismatches. Keep CUDA runtime parity/performance validation as a separate
    CUDA-host follow-up; this Apple Metal evidence does not prove CUDA backend
    parity.
  - The next throughput slice should target the post-GPU peer CPU stack:
    Ed25519/Curve25519 public-key parse and verification, Norito
    transaction/transfer serialization and decode, transaction metadata
    hashing, allocation/copy traffic, and CRC64/SHA-256 helpers. The first
    bookkeeping slice already removed per-transaction `DashMap::len()` from
    `PipelineStatusCache::prune_if_needed`, and the Ed25519 thread-local slice
    now includes a direct-mapped full-key cache before the generic linear
    verifier cache. The current allocation slice streams typed Norito hashes
    directly into Blake2b, finalizes direct Blake2b hashes into fixed buffers
    without boxed digest allocation, absorbs Merkle parent/commitment chunks
    without staging concatenation buffers, and hashes external transaction
    entrypoints through a borrowed encoder instead of cloning the signed
    transaction into an enum wrapper. The release Izanami/iroha3d binaries now
    rebuild with the allocation slice, and the clean return gate at
    `dist/izanami-prebuilt-20k-fastpq-gpu-return-120s-20260504-012106`
    restored ingress (`2,400,000` accepted and succeeded, `0` failures) but
    still reached only `12,413` strict-approved transactions at height `5`.
    The matching sampled profile at
    `dist/izanami-profile-20k-fastpq-gpu-return-sampled-30s-20260504-012521`
    was intrusive, but its peer stacks confirm the next work remains
    Ed25519/Curve25519 parse and verification, Norito transaction/transfer
    encode/decode, metadata hashing, allocation/copy traffic, and SHA-256/CRC64
    helpers. A first queue-lock slice now releases `push_remove_lock` before
    post-enqueue backpressure/gossip/event/wake side effects. The follow-up
    bottleneck fix repairs the post-queue-lock execution-context mismatch,
    moves RBC READY/DELIVER traffic onto the consensus-chunk lane, gives chunk
    traffic a turn after each high-priority payload frame, caches prepared
    metadata JSON depth, and keeps prepared metadata depth checks on the
    static-validation hot path. The clean rebuilt
    `20k TPS` / `120s` `fastpq-gpu` gate at
    `dist/izanami-prebuilt-20k-fastpq-gpu-bottleneckfix-120s-20260504-183724`
    accepted and succeeded all `2,400,000` submissions with no safety failures
    and reached `37,000` strict-approved transactions at height `11`, but queue
    saturation remained (`854,344 / 2,400,000`). The matching sampled profile at
    `dist/izanami-profile-20k-fastpq-gpu-bottleneckfix-peer-sampled-30s-20260504-184154`
    shows no scalar FASTPQ/Poseidon fallback; the next bottlenecks are block
    validation and serialization costs: Ed25519/Curve25519 verification math,
    Norito compact-length and transaction/transfer encode/decode,
    allocator/reallocation and copy traffic, SHA-256/Blake2/CRC64 helpers,
    `resolve_streaming_metadata`, and pipeline access/overlay preparation.
    A final prepared-hash cleanup after that profile avoids temporary
    signed-transaction byte vectors while preparing hashes/lengths and reuses
    prepared payload/signed hashes in validation cache and signature-batch
    paths. The current-code `20k TPS` / `120s` rerun at
    `dist/izanami-prebuilt-20k-fastpq-gpu-return-current-120s-20260504-194602`
    covered that cleanup: Izanami exited `0`, accepted and succeeded all
    `2,400,000` submissions, recorded no safety failures, and had submit
    latency `p50=6ms`, `p95=21ms`, `p99=99ms`, `max=269ms`. Strict progress
    was lower than the previous gate at `32,956` approved transactions at
    height `10`, with queue saturation still high (`883,791 / 2,400,000`) and
    commit-pipeline EMA `592ms`. Treat the 20k ingress path as restored; the
    committed-throughput target still needs the next validation/serialization
    hotspot pass. The fresh current-code profiles refine that target: the
    immediate `30s` sample at
    `dist/izanami-profile-20k-fastpq-gpu-current-peer-sampled-30s-20260504-195325`
    shows FASTPQ Metal pipeline creation still happens on the first proof hot
    path, while the delayed post-warm `60s` sample at
    `dist/izanami-profile-20k-fastpq-gpu-current-peer-postwarm-sampled-60s-20260504-195720`
    moves the steady-state bottleneck back to validation and serialization.
    The 2026-05-05 FASTPQ lane preflight follow-up moves backend construction
    off the startup/submission path, keeps digest acceleration disabled until
    the lane observes successful GPU preflights, and falls back to CPU prover
    modes after a failed Poseidon GPU preflight. The current May 6 return gate
    at
    `dist/izanami-prebuilt-20k-fastpq-gpu-return-current-120s-20260506-124641`
    accepted and succeeded all `2,400,000` submissions with no safety failures
    and reached `49,428` strict-approved transactions at height `14`, above the
    previous `45,191` preflight gate. Treat first-proof FASTPQ GPU preflight
    and the latest single-transfer digest deferral path as addressed for now;
    the next open work is Ed25519/public-key parse and verify work, Norito
    transaction and transfer encode/decode/length accounting, allocation/copy
    churn, queue-admission/world-view preparation, and queue drain under
    saturated 20k ingress. That older profile avoided scalar FASTPQ/Poseidon
    fallback work until new evidence; the May 7 load-window sample below
    reintroduces scalar cost specifically in the BN254 runtime digest path,
    while general FASTPQ prover parity remains fixed.
    The 2026-05-07 Metal final return gate fixes general FASTPQ Poseidon
    preflight parity and removes normal commit-QC inline validation supersedes;
    keep the next Izanami pass on queue drain/block-validation cost and BN254
    runtime Metal batch stability, not on prover Poseidon preflight parity.
    The corrected load-window profile at
    `dist/izanami-profile-20k-fastpq-gpu-final-loadsample-90s-20260507-225637`
    sharpens that order: scalar Halo2 BN254 Poseidon is again the top sampled
    application leaf after runtime Metal batch failures, while consensus
    progress is limited by payload availability and exact-frontier recovery
    signals under a saturated queue. Fix BN254 runtime batch stability first,
    then reduce local READY/DELIVER deferrals and block-body reacquisition
    latency before revisiting the secondary Norito, Ed25519/Curve25519, SHA-2,
    Blake2, CRC64, and allocation hot paths.
  - Avoid repeating the rejected process-wide Ed25519 public-key parse cache
    approach without new evidence: the 2026-05-03 sharded shared-cache
    experiment regressed short-gate commit progress and was backed out. Keep
    near-term Ed25519 work thread-local, allocation-focused, or validation-path
    specific unless a clean before/after gate proves otherwise. The accepted
    thread-local slice pre-sizes only the public-key parse map, keeps parsed key
    entries boxed to satisfy `variant-size-differences`, and keeps the generic
    verify-ok map lazy so 32-byte transaction hashes do not allocate unused
    generic cache state.
  - Keep broader trait-wide parallel decode, deeper GPU decode materialization,
    deeper dalek backend experimentation, and deterministic hardware-specific
    Ed25519/Curve25519 acceleration as follow-up work until the current
    bottleneck slice has clean before/after evidence.
- Continue the 20k post-cache throughput tuning corridor.
  - The first post-cache 4-peer no-fault prebuilt `20k TPS` / `120s` release
    gate at `dist/izanami-prebuilt-20k-hotpath-120s-20260501-142015` improved
    strict approved transactions to `28,713` but still failed the committed
    20k target.
  - A same-shape repeat at
    `dist/izanami-prebuilt-20k-cachepass-120s-20260501-142429` accepted
    `52,167` ingress transactions but only reached `24,623` strict approved
    transactions, confirming material run-to-run variance and the same
    queue-drain/block-validation bottleneck.
  - The fresh post-cache sampled 20k profile at
    `dist/izanami-profile-20k-cachepass-sampled-30s-20260501-152126` confirms
    the next target has moved from queue gossip encoding to
    `validate_block_for_voting` / `validate_and_record_transactions` /
    `TxOverlay::apply_with_chunk`, incoming transaction-gossip Norito decode,
    and the remaining `AcceptedTransaction::signed_encoded_len` serialization
    fallback.
  - The targeted post-cache tuning pass at
    `dist/izanami-prebuilt-20k-postcache-tuned-120s-20260501-165947` improved
    strict approved transactions over the cachepass repeat to `28,790`, but
    still failed the committed 20k target and accepted fewer ingress
    submissions. The matching sampled profile at
    `dist/izanami-profile-20k-postcache-tuned-sampled-30s-20260501-165811`
    confirms `Queue::encode_gossip_payload`, `TxOverlay::byte_size`, and
    `external_entrypoints_cloned` are absent from current peer samples.
  - The further conservative cache pass is focused-validation green as of
    2026-05-01: prepared transaction metadata is reused through block
    validation/execution recording, all-external block validation keeps
    borrowing the entrypoint slice, signed/external entrypoint encoded-length
    coverage avoids the residual Norito fallback for representative shapes, and
    gossip transaction decode now uses the shared cached payload helper.
  - The clean release 4-peer no-fault prebuilt `20k TPS` / `120s` rerun at
    `dist/izanami-prebuilt-20k-conservative-cache-rerun-120s-20260501-175213`
    exited `0`, accepted `54,574` ingress transactions, and reached `28,710`
    strict approved transactions at strict height `9`. This is consistent with
    the prior tuned gates and still misses the committed 20k target, with no
    validation rejects, view changes, or RBC pressure.
  - A later requested same-shape rerun at
    `dist/izanami-prebuilt-20k-conservative-cache-rerun2-120s-20260501-144548`
    exited `0` but ran under active debug `cargo test`/`rustc` contention. It
    accepted `52,070` ingress transactions and reached only `12,329` strict
    approved transactions at strict height `5`, with safety intact but `4`
    view-change installs and missing-block recovery activity. Treat this as
    contended evidence only, not a replacement for the clean baseline.
  - The matching requested contended sampled profile at
    `dist/izanami-profile-20k-conservative-cache-rerun2-sampled-30s-20260501-145104`
    exited `0` with valid samples for the load driver and all four peers;
    `sample_status=1` only because the sampler also targeted the bash wrapper
    and one transient process. It accepted `52,817` ingress transactions and
    reached `4,137` strict approved transactions at strict height `3`. The
    bottleneck shape matches the previous conservative-cache profiles: Torii
    admission crypto/public-key parsing, canonical signed-byte construction,
    residual dynamic `InstructionBox` framing, gossip materialization/decode,
    and overlay execution/cloning. Treat it as contended bottleneck evidence,
    not a clean latency baseline.
  - The earlier conservative-cache sampled 20k profile at
    `dist/izanami-profile-20k-conservative-cache-parallel-sampled-30s-20260501-181025`
    confirms the previous removals are still absent
    (`Queue::encode_gossip_payload=0`, `TxOverlay::byte_size=0`,
    `external_entrypoints_cloned=0`) and moves the next bottleneck set to
    Torii ingress signature/public-key work, canonical signed-byte construction
    in `AcceptedTransaction::from_external_with_hot_cache`, exact-length
    `InstructionBox` payload framing, gossip materialization during admission,
    and remaining overlay instruction clones.
  - The broader 20k bottleneck pass is focused-validation green as of
    2026-05-01. Lazy transaction-gossip materialization now preserves cached
    framed entrypoint bytes and skips semantic decode before route, plane, and
    known-duplicate filters; route-valid single-key Ed25519 gossip candidates
    use deterministic batch precheck through the existing signature-batch
    setting; overlay apply goes through the crate-private borrowed adapter while
    custom executors keep the owned path. The profile at
    `dist/izanami-profile-20k-postcache-tuned-bottleneck-30s-20260501-171955`
    is pre-broader-pass evidence; the fresh reruns are
    `dist/izanami-profile-20k-broader-pass-sampled-30s-20260501-194734` and
    `dist/izanami-prebuilt-20k-broader-pass-120s-20260501-194908`.
    The 120s gate kept final approved transactions flat against the previous
    gate (`28740` vs `28710`) but accepted fewer ingress submissions
    (`52291` vs `54574`), so treat the pass as bottleneck reshaping rather than
    a confirmed throughput win.
  - The fixed-runner follow-up sampled profile at
    `dist/izanami-profile-20k-broader-pass-rerun-sampled-30s-20260501-200527`
    completed with `sample_status=0` and sampled the actual Izanami runner plus
    all observed peer processes. It classifies the next bottlenecks as
    direct-peer Ed25519/curve25519 verification math first, then allocation /
    `memmove` and Norito compact/decode work, with ZK/BLS math and hashing as
    secondary costs. Queue mechanics and borrowed overlay apply are not primary
    CPU bottlenecks in that sample.
  - The latest clean rebuilt release 4-peer no-fault prebuilt `20k TPS` /
    `120s` gate is
    `dist/izanami-prebuilt-20k-direct-ingress-precheck-final-120s-20260501-212850`;
    it exited `0`, accepted `47,566` ingress transactions, and reached
    `20,499` strict approved transactions at strict height `7`. The contention
    snapshots only contain timestamps. Safety signals stayed clean, but the
    run still saturated the queue and ended with height skew `1` /
    approved-transaction skew `8,192`, so the 20k target remains open.
  - The latest fixed-runner sampled profile at
    `dist/izanami-profile-20k-rerun-release-sampled2-30s-20260501-211211`
    completed with `sample_status=0`, accepted `46,709` ingress transactions,
    and reached `4,125` strict approved transactions at strict height `3`.
    The current peer CPU stack is led by `iroha_zkp_halo2::poseidon` /
    `fastpq_isi::poseidon`, `memmove` and allocator paths, `sha2`/`blake2`
    hashing, Norito compact-length/decode/encode routines, and then
    `curve25519_dalek` / `ed25519_dalek` verification math. Direct ingress
    batch precheck remains visible but is not the dominant leaf in this sample;
    overlay clone and exact-length helpers are low-count residue.
  - The direct-ingress conservative cache and precheck slice is code-complete
    as of 2026-05-01: Torii signed transaction and batch submission now decode
    versioned signed payloads into a prepared core admission token and run
    deterministic single-Ed25519 batch precheck for eligible batch entries,
    reusing signed/entrypoint hashes, payload hash, exact signed length, and
    parsed single-Ed25519 key metadata without changing transaction wire/hash
    semantics, config knobs, dependencies, or `Cargo.lock`.
  - The exact-length `InstructionBox` cost is reduced without changing Norito
    wire: `encoded_len_exact` now counts the existing `(wire_id,
    framed_payload)` representation without re-framing the dynamic ISI payload.
  - The FASTPQ/Poseidon foreground pass is implemented: single-delta transfer
    transcript digests are finalized at block/witness drain instead of inside
    `Transfer::execute`, FASTPQ digest hashing streams bytes without a full
    preimage buffer, and decoded external entrypoint hashes now reuse the
    inbound versioned signed payload bytes.
  - The first FASTPQ BN254 Metal Poseidon batch path is implemented behind the
    existing `fastpq-gpu` feature and existing FASTPQ execution/poseidon modes.
    Apple Metal toolchain validation was run on 2026-05-05 and found FASTPQ
    Metal parity failures in FFT/LDE/Poseidon paths. Remaining work is to fix
    those mismatches, rerun the Metal parity tests to green, then compare a 30s
    sampled 20k profile and a 120s gate with `--fastpq-poseidon-mode gpu`
    against the latest scalar release artifacts.
  - Carry the Norito sequence span planner through the remaining acceleration
    corridor: replace the length-prefixed helper's serial device parser with a
    tuned prefix-scan/chunked planner if profiling shows it is on the hot path,
    expand typed parallel sequence decode beyond the current hidden
    `parallel-decode` `Vec<T: Send>` path if profiling proves narrower
    transaction/admission/block-validation call sites need it, then rerun the
    30s sampled 20k profile and 120s gate with the target host's acceleration
    features.
  - The latest scalar release 4-peer no-fault prebuilt `20k TPS` / `120s` gate
    after the Norito span-planner pass is
    `dist/izanami-prebuilt-20k-rerun-release-norito-span-120s-20260502-015557`;
    it exited `0`, accepted `47,503` ingress transactions, reached
    strict/quorum height `10`, and approved `32,786` transactions. The latest
    matching scalar sampled profile is
    `dist/izanami-profile-20k-norito-span-sampled-30s-20260502-020217`; it
    shows Norito transaction/instruction codec as the current top active peer
    path, followed by Poseidon/Ed25519/Curve/hash work, Rayon proof/hash
    scheduling, allocation/copy churn, TLS/context lookup, and Torii admission
    queue routing. Use this artifact as the baseline before the next
    optimization pass.
  - Continue reducing Norito decode/allocation overhead on the direct and
    gossip admission corridors without changing wire bytes or canonical hashes.
    `InstructionBox::DecodeFromSlice` now uses the borrowed tuple parser
    directly and `ExecutionStep::DecodeFromSlice` now delegates its inner
    instruction list to `ConstVec<InstructionBox>`. `ConstVec<T>` slice decode
    now tries the scalar Norito sequence planner directly for non-`u8` elements
    before falling back to the canonical `Vec<T>` field path, removing the
    top-level archive/canonical-length pass from the hot instruction-vector
    route. `AcceptedTransaction` also now derives the cached signed frame and
    external entrypoint hash from one canonical signed payload in the hot-cache
    path, avoiding a second signed-transaction serialization. `SignedTransaction`
    and `TransactionPayload` slice decoders now walk AoS fields directly, and
    `Executable::Instructions` routes the instruction vector into the planned
    `ConstVec<InstructionBox>` decoder before falling back for other executable
    variants. A fresh WSL2 no-profiler validation run after this
    admission-decode pass is recorded in `status.md`:
    `dist/izanami-prebuilt-20k-admission-decode-unsampled-30s-20260506-020112`
    accepted/succeeded all `600,000` offered submissions, and
    `dist/izanami-prebuilt-20k-admission-decode-120s-20260506-020335`
    accepted/succeeded `2,379,055` submissions with no safety failures but only
    `20,553` strict-approved transactions. Treat these as fresh ingress/safety
    evidence, not a bottleneck profile: the host had neither `sample` nor
    `perf`, and the 2.4M prebuilt-buffer run consumed nearly all WSL2 memory.
    Individual instruction payload slice paths are now in place for `Log`,
    `RecordSccpMessage`, transfer instructions, transfer batches, mint/burn
    asset and trigger instructions, key-value metadata instructions,
    Grant/Revoke permission changes, account signatory/quorum changes, the
    stable core SetParameter/trigger/upgrade/custom ISIs, Register/Unregister
    instructions and boxes, asset-definition alias/balance-policy instructions,
    asset transfer-control instructions, account alias binding/lease
    instructions, contract-alias instructions, account-recovery instructions,
    RAM-LFE program-policy instructions, hidden-identifier instructions,
    consensus-key lifecycle instructions, domain-endorsement instructions,
    verifying-key instructions, Offline V2 note instructions, verified Nexus
    lane-relay/fee-budget instructions, RWA/repo/settlement stable boxes,
    native and anonymous asset escrow lifecycle instructions, Musubi
    package-registry instructions, smart-contract-code
    manifest/instance/bytecode instructions, the Space Directory manifest
    lifecycle instructions, SoraFS pin/capacity/replication/provider-owner
    instructions, oracle feed/observation/dispute/governance/Twitter binding
    instructions, bridge proof/receipt/SCCP instructions, Ministry citizen-agenda
    proposal submission, social Twitter reward/escrow instructions, registered
    public-lane staking instructions, invalid-instruction placeholders, SoraNet
    VPN lease open/settle/refund instructions, runtime-upgrade ISIs, SNS name
    ISIs, ZK proof/confidential/election ISIs, Kaigi session/relay ISIs, and
    governance proposal/ballot/citizen ISIs, Soracloud service lifecycle,
    host/placement, agent, model/training, rollout, runtime-state, mailbox, and
    receipt ISIs, `RegisterPeerWithPop`, and Nexus emergency-validator override
    ISIs via an opt-in registry constructor. No default registry instruction
    remains on the generic instruction decoder path. Remaining targets are
    broader allocation/memmove churn around transaction admission material, and
    a sampled 30s profile plus clean 120s gate on a profiler-equipped host after
    the next scalar admission-decode pass.
  - Keep the FASTPQ BN254 Metal path validation separate from scalar profiling:
    after installing the Apple Metal toolchain, run the Metal parity tests and a
    `fastpq-gpu` 30s/120s comparison with `--fastpq-poseidon-mode gpu`.
  - Keep an Ed25519 parsed-public-key/signature verification cache or a
    deterministic batch corridor for the Torii/direct-ingress single-key
    Ed25519 authority path as the next crypto follow-up after the
    Poseidon/source-attribution and Norito allocation work. Gossip-side
    deterministic Ed25519 batch precheck is already implemented.
  - Rerun 4-peer no-fault prebuilt `5k` and `10k TPS` rows as needed to locate
    the new knee after the conservative cache pass.
  - The targeted built-in overlay path now avoids the full `InstructionBox`
    clone before `Executor::Initial` dispatch; user-provided executors still
    use the owned fallback. Keep the broader borrowed-instruction execution
    rewrite separate unless a later post-crypto/decode profile again shows
    `Transfer::clone`, `WorldTransaction::apply`, or the concrete instruction
    handler clones as active costs.
  - Treat RBC authoritative-payload delays as symptoms of slow validation and
    materialization unless a later profile shows DA/RBC storage pressure,
    missing `BlockCreated`, or QC payload-missing counters.
  - Move FASTPQ worker budgeting and deterministic hardware-accelerated crypto
    investigation into the next tuning branch if the post-deferral profile
    still shows background prover Poseidon work competing with consensus. Keep
    the full borrowed-`Execute` executor API rewrite separate unless a later
    profile makes overlay execution dominant again.
- Turn the proposal-gap / queue-pressure investigation into a reproducible measurement pass.
  - Rerun the 7-peer load that previously advanced slowly or stalled under backlog.
  - Sample `/v1/sumeragi/status`, pending-block / commit-inflight metrics, and queue depths throughout the run.
  - Use a load generator that can actually sustain the target rate before changing worker/backlog tuning again.
- Rebaseline sorted asset-definition query performance.
  - Rerun `snapshot_ephemeral_sorted_asset_defs_first_batch` and `snapshot_stored_sorted_asset_defs_first_batch` on an isolated host.
  - If stored-mode still regresses, tune `stored_sorted_fast_start_params` / first-batch thresholds and keep the matching query tests aligned.
  - Restore a green `cargo test -p iroha_core` baseline for the query-performance branch after any tuning.

## Targeted follow-ups

- Migrate the remaining operator VPN workflows to submit the Torii-returned
  native `OpenVpnLeaseEscrow` and `SettleVpnLease` transactions, then retire
  the legacy in-memory receipt endpoint after a public relay/helper/Torii
  canary.
- Broaden Kura replay determinism beyond the focused unit corridor.
  - Add a multi-block replay fixture that replays committed blocks into a fresh
    state and compares canonical WSV snapshot bytes against the originally
    committed WSV.
  - Add a real 4-peer restart integration test that commits route-sensitive
    asset, account, alias, and domain-owned state, rebuilds from Kura, and
    compares canonical WSV snapshot bytes across the restarted peers.
  - Keep the fixture on the replay-specific validation entrypoint so legacy
    blocks without embedded context remain covered separately from newly
    proposed blocks.
  - Add golden old-block Norito fixtures produced by a pre-context binary,
    rather than only synthesized absent-field decode tests.
  - Profile the post-commit canonical WSV checkpoint hash under sustained load
    and either record the accepted overhead or replace it with a cheaper
    committed state-root path.
  - Decide whether a failed post-commit WSV checkpoint write should escalate the
    peer immediately after the block is committed, instead of only logging and
    failing later replay.
  - If operators need a network-authenticated replay proof, promote the WSV root
    from a local Kura sidecar into block-committed or certificate-bound metadata.
- Broaden alias auto-renew mutation coverage beyond the focused onboarding grant.
  - Add an integration test proving a user-signed enable/disable update can mutate the subscription NFT created by onboarding.
  - If a non-onboarding mutation path still hits `Can't modify NFT from domain owned by another account`, capture the exact submitter, NFT id, and permission token shape before changing the permission model again.
- Add a live multi-peer multisig test for previously unregistered signatories.
  - Start from the existing materialization coverage in `integration_tests/tests/multisig.rs`.
  - Add a case where a signatory is materialized by registration and then successfully authors `MultisigPropose` / `MultisigApprove` on the network.
  - Assert transaction-authority shape and final instruction execution, not only account materialization.
- Extend and burn down the translation metadata audit backlog.
  - Refresh the translated `docs/formal/sumeragi/README.*.md` bodies after the
    English-only frontier formal and 2026-05-03 process-hardening updates so
    `python3 ci/check_docs_i18n_metadata.py --paths docs/formal --require-current`
    can be restored for formal docs.
  - The Sumeragi frontier model, process invariants, mutation suite, TLC
    cross-check, and longer nightly bound are wired, and CI now publishes a JSON
    metadata report for the stale translated formal READMEs; the remaining
    formal-doc task is translation refresh only.
  - Clean the existing `docs/source` and `docs/portal` metadata debt, including files missing `source_hash` and `translation_last_reviewed`, before adding those trees to the CI gate.
  - Refresh only the files the checker flags, then record the clean audit command in `status.md`.
- Add a recorded capture gate for the default `sora-temple` petal styles.
  - Use `petal score-styles` with a published style set, profile, seed, and minimum success ratio.
  - Record the JSON baseline in `status.md` and keep the default style honest under aggressive capture.
  - Only add a stronger default variant if the current `sora-temple` family cannot meet the agreed gate.

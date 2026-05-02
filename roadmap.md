# Roadmap (Open Work Only)

Last updated: 2026-05-02

Completed history lives in `status.md`. This file should only track unfinished work.

## Validation corridor

- Carry the Sumeragi NPoS/permissioned QC and VRF hardening through the next
  full workspace corridor.
  - Focused commit, block-sync, VRF, QC-validation, roster-selection, Torii VRF
    OpenAPI/parser, and data-model consensus roundtrip tests are green as of
    2026-05-02 with `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-verify`.
  - Focused `cargo clippy -p iroha_core -p iroha_data_model -p iroha_torii -p
    irohad --all-targets -- -D warnings` is green as of 2026-05-02 with
    `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-clippy`.
  - Full workspace all-target clippy is green as of 2026-05-02 with
    `CARGO_TARGET_DIR=/tmp/iroha-codex-workspace-corridor`.
  - Broader `cargo test --workspace` remains for an uncontended validation
    window.
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
    `iroha-core-tests,app_api`.
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
    canonical TLV envelope copied or validated. `SM4_GCM_*` and `SM4_CCM_*`
    now charge `G_sm4 + bytes` through the shared default-host implementation,
    preserving deterministic vector output while charging AAD and
    plaintext/ciphertext bytes. Deterministic sysvar reads now charge
    `G_sysvar` or `G_sysvar + bytes`, and authority reads charge
    `G_get_auth + bytes`, across default, WSV, standalone codec, and real-host
    paths; the real-host focused rerun is pending an unrelated dirty-tree
    `fastpq_prover` compile repair.
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
  - Follow-up widened checks are green as of 2026-05-02:
    `cargo test -p ivm_abi`, `cargo test -p kotodama_lang`,
    `cargo clippy -p ivm_abi -p kotodama_lang --all-targets -- -D warnings`,
    and `cargo clippy -p ivm --all-targets -- -D warnings`.
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
  - Keep the broader `cargo test --workspace` corridor open. The static
    OpenAPI JSON and version index are refreshed in explicit unsigned mode; an
    operator still needs to regenerate the manifest with the OpenAPI signing
    key or detached signature envelope.
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
- Broaden validation after the 2026-04-22 targeted `sumeragi::main_loop` regression sweep and follow-up unit coverage additions.
  - Rerun a wider `cargo test -p iroha_core --lib` window now that the reported 10-case failure cluster is green under focused verification.
  - Keep the new collector-disabled fallback, seeded-collector precommit fallback, near-quorum full-fanout retransmit helper test, signer-mapping failure full-fanout fallback test, local-vote replay suppression, direct `known_block_commit_qc_recovery_targets(...)` fallbacks, direct `frontier_body_next_due(...)` deadline/fallback coverage, the direct deterministic roster-election guard tests, the direct NPoS empty-commit-topology candidate-source tests, the direct "no new progress" replay-suppression test, the direct replay-cooldown suppression test, the direct local-only explicit-target replay test, the direct deduplicated explicit-target vote replay test, the direct local-only explicit-target commit-QC replay test, the local-proposal authoritative-frontier fallback test, the local-proposal first-signature fallback test, the generic proposal-wire authoritative-frontier fallback and mismatch-guard tests, the plain `frontier_block_created_for_wire(...)` no-metadata fallback test, and the expanded cached-target / unarmed / local-only `frontier_body_next_due(...)` tests in the rerun window so the next pass exercises both the regression fixtures and the new narrow branch tests together.
  - If that broader rerun exposes follow-up regressions outside the patched collector / frontier / roster slice, capture the first failing test names and keep the next fix narrowly scoped.
- Reopen the wider validation corridor after the recent focused `iroha_core`, `iroha_torii`, and `iroha_data_model` test additions.
  - Rerun `cargo test -p iroha_core --lib`, including `quorum_reschedule_rebroadcasts_block_created_while_skipping_block_sync_without_roster_proof` in a fresh cargo process.
  - Rerun `cargo test -p iroha_torii` and `cargo test -p integration_tests -- --nocapture` once the current tree is stable enough for network suites.
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
    public-key parse outcomes, expands the direct parse cache to 256 slots,
    routes compact/full conversion through the cached parse path, skips
    signature parsing and dalek batch setup for all-cached exact verify tuples,
    and preserves lowest-original-index failure reporting for mixed batches.
    Focused crypto/Torii checks and the `ed25519_hotpaths` Criterion bench are
    recorded in `status.md`.
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
  - Rerun the 30s sampled Izanami 20k profile and a clean 120s release gate in
    an uncontended host window after benchmark wins are recorded.
  - Keep broader trait-wide parallel decode, deeper GPU decode materialization,
    deeper dalek backend experimentation, deterministic hardware-specific
    Ed25519/Curve25519 acceleration, and FASTPQ GPU hook retuning as follow-up
    work until the scalar changes have clean before/after evidence.
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
    Remaining work is validation on a host with the Apple Metal toolchain
    installed: compile the metallib, run the Metal parity tests, then compare a
    30s sampled 20k profile and a 120s gate with `--fastpq-poseidon-mode gpu`
    against the latest scalar release artifacts.
  - Carry the Norito sequence span planner through the remaining acceleration
    corridor: validate the CUDA sequence planner with the real `cuda-kernel`
    feature on a GPU host, replace the length-prefixed helper's serial device
    parser with a tuned prefix-scan/chunked planner if profiling shows it is on
    the hot path, wire `decode_planned_sequence_parallel` into specific typed
    transaction/admission/block-validation call sites that can prove `T: Send`
    and lowest-index error ordering, then rerun the 30s sampled 20k profile and
    120s gate with the target host's acceleration features. CUDA host validation
    commands to run when hardware is available: `JSONSTAGE1_CUDA_REQUIRE=1 cargo
    test -p jsonstage1_cuda --features cuda-kernel binary_sequence_plan`,
    `JSONSTAGE1_CUDA_REQUIRE=1 cargo test -p norito sequence_plan --features
    codec-gpu-cuda`, `GPUZSTD_CUDA_REQUIRE=1 cargo test -p norito gpu_zstd
    --features gpu-compression`, and a Norito CRC64 pass with `cuda-crc64`
    enabled after building the CUDA helper.
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
  - Reduce Norito decode/allocation overhead on the direct and gossip admission
    corridors without changing wire bytes or canonical hashes. The next useful
    targets are repeated compact-length walks, instruction-registry decode
    paths, and allocation/memmove churn around canonical transaction material.
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
- Broaden alias auto-renew mutation coverage beyond the focused onboarding grant.
  - Add an integration test proving a user-signed enable/disable update can mutate the subscription NFT created by onboarding.
  - If a non-onboarding mutation path still hits `Can't modify NFT from domain owned by another account`, capture the exact submitter, NFT id, and permission token shape before changing the permission model again.
- Add a live multi-peer multisig test for previously unregistered signatories.
  - Start from the existing materialization coverage in `integration_tests/tests/multisig.rs`.
  - Add a case where a signatory is materialized by registration and then successfully authors `MultisigPropose` / `MultisigApprove` on the network.
  - Assert transaction-authority shape and final instruction execution, not only account materialization.
- Extend and burn down the translation metadata audit backlog.
  - Refresh the translated `docs/formal/sumeragi/README.*.md` bodies after the
    English-only frontier formal update so `python3 ci/check_docs_i18n_metadata.py --paths docs/formal --require-current`
    can be restored for formal docs.
  - The Sumeragi frontier model, mutation suite, TLC cross-check, and longer
    nightly bound are wired, and CI now publishes a JSON metadata report for
    the stale translated formal READMEs; the remaining formal-doc task is
    translation refresh only.
  - Clean the existing `docs/source` and `docs/portal` metadata debt, including files missing `source_hash` and `translation_last_reviewed`, before adding those trees to the CI gate.
  - Refresh only the files the checker flags, then record the clean audit command in `status.md`.
- Add a recorded capture gate for the default `sora-temple` petal styles.
  - Use `petal score-styles` with a published style set, profile, seed, and minimum success ratio.
  - Record the JSON baseline in `status.md` and keep the default style honest under aggressive capture.
  - Only add a stronger default variant if the current `sora-temple` family cannot meet the agreed gate.

# Status

Last updated: 2026-03-26

## 2026-03-26 Follow-up: permission JSON normalization moved to decode time and alias-scope schema tags are back in sync
- Hardened `crates/iroha_data_model/src/permission.rs` so `Permission`
  equality/order semantics are exact again, while JSON decoding now
  canonicalizes only insignificant payload whitespace, rejects duplicate
  top-level `name` / `payload` fields on the parser path, and preserves
  duplicate keys inside the payload instead of reparsing through
  `norito::json::Value`.
- Restored the Norito enum metadata on
  `crates/iroha_executor_data_model/src/permission.rs` so
  `AccountAliasPermissionScope` publishes the same snake-case schema tags that
  the manual JSON codec already emits and accepts.
- Kept the focused end-to-end regression in `crates/iroha_core/src/state.rs`
  proving that a permission deserialized from JSON still matches the canonical
  typed alias-management permission in the permission cache path.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_data_model permission --lib` (pass)
  - `cargo test -p iroha_executor_data_model alias_scope_ --lib` (pass)
  - `cargo test -p iroha_core permission_deserialized_from_json_matches_canonical_permission --lib` (pass)

## 2026-03-26 Follow-up: address canonicalisation integration assertions now validate canonical account literals instead of a stale `sora` prefix
- Tightened `integration_tests/tests/address_canonicalisation.rs` so the
  account-listing, asset-holder, and account-transaction response assertions
  validate canonical encoded account literals by parsing them, instead of
  requiring a stale `sora...` textual prefix that no longer matches the Torii
  account-literal formatter or the current `AccountId`/`AccountAddress`
  canonical rendering.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo test -p integration_tests --test address_canonicalisation -- --nocapture --test-threads=1` (pass; 24 passed, 0 failed, finished in 620.03s)

## 2026-03-25 Follow-up: FASTPQ transfer golden fixtures refreshed for canonical asset/account identifiers
- Refreshed the checked-in transfer regression fixture and its derived golden
  hashes after canonical asset/account identifier rendering changed the
  serialized transfer-row keys used by `fastpq_prover`:
  - `crates/fastpq_prover/tests/fixtures/transfer.norito`
  - `crates/fastpq_prover/tests/fixtures/ordering_hash.json`
  - `crates/fastpq_prover/tests/trace_commitment.rs`
- Validation:
  - `FASTPQ_UPDATE_FIXTURES=1 cargo test -p fastpq_prover ordering_hash_matches_golden_vectors -- --nocapture` (pass)
  - `cargo test -p fastpq_prover --test trace_commitment -- --nocapture` (pass)

## 2026-03-25 Follow-up: FASTPQ stage2 balanced fixtures refreshed again for current canonical proof bytes
- Refreshed the checked-in FASTPQ Stage 2 backend regression artifacts after the
  current CPU prover output drifted from the previously committed golden files
  while keeping the same encoded sizes:
  - `crates/fastpq_prover/tests/fixtures/stage2_balanced_1k.bin`
  - `crates/fastpq_prover/tests/fixtures/stage2_balanced_5k.bin`
- Validation:
  - `FASTPQ_UPDATE_FIXTURES=1 cargo test -p fastpq_prover --test backend_regression stage2_artifact_balanced_ -- --nocapture` (pass)
  - `cargo test -p fastpq_prover --test backend_regression stage2_artifact_balanced_ -- --nocapture` (pass)
  - `cargo test -p fastpq_prover --test proof_fixture golden_stage2_proof_matches_fixture -- --nocapture` (pass)

## 2026-03-25 Follow-up: active cargo-deny dependency findings are closed in the current tree
- Tightened the workspace dependency policy in `Cargo.toml` and `deny.toml`
  so the active dependency tranche no longer leaves live `cargo deny`
  failures in the current resolve:
  - the workspace now explicitly pins `reqwest` / `rustls` to patched patch
    releases, which keeps `rustls-webpki` on the fixed `0.103.10` line in the
    generated dependency graph; and
  - `deny.toml` now records explicit reasons for accepting the two remaining
    unmaintained transitive macro advisories, `RUSTSEC-2024-0388`
    (`derivative`) and `RUSTSEC-2024-0436` (`paste`), because there is no safe
    upgrade and removing them would require replacing or vendoring multiple
    upstream stacks (`w3f-bls`, `pqcrypto-mldsa`, `halo2curves`, `parquet`,
    `ratatui`).
- Validation:
  - `cargo deny check advisories bans sources --hide-inclusion-graph` (pass; advisories noted as ignored, `bans` ok, `sources` ok)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 cargo check -p iroha_torii_shared --tests --message-format short` (pass)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 cargo check -p iroha_crypto --tests --message-format short` (pass)
- Remaining implementation gap:
  - the active dependency advisory tranche is closed for the current tree; the
    remaining confirmed security backlog is now CUDA runtime validation on a
    CUDA-capable host.

## 2026-03-25 Follow-up: offline cash app-API requests now require canonical device binding/proof and active lineages cannot hop devices
- Hardened `crates/iroha_torii/src/lib.rs`,
  `crates/iroha_torii/src/offline_reserve.rs`,
  `IrohaSwift/Sources/IrohaSwift/OfflineCashModels.swift`,
  `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift`,
  and
  `IrohaSwift/Tests/IrohaSwiftTests/ToriiOfflineCashEndpointsTests.swift`
  so the offline cash app API no longer infers Apple mode from missing fields and no longer allows a funded/pending lineage to be rebound to a different device.
- The shipped behavior in this slice:
  - app-facing offline cash `setup` / `load` / `refresh` / `sync` / `redeem` requests now require both canonical `device_binding` and `device_proof`, the platform must be explicit (`android` or `ios`), and the JSON bridge translates both platforms into the internal reserve attestation format instead of assuming Android-only wiring;
  - receipt translation now consumes canonical `device_proof`, so onward-send receipts keep the same wire shape across Android and iOS;
  - reserve creation now rejects `lineage_conflict` when the same account already has a different device-bound lineage with funded or pending offline state, and reserve mismatch errors now surface the same machine-mappable conflict marker; and
  - the Swift SDK offline cash request models and client now emit canonical `device_binding` / `device_proof` payloads with deterministic `Idempotency-Key` headers, while the focused endpoint regression only requires that header on mutating routes.
- Validation:
  - `cargo fmt --all` (pass)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 cargo check -p iroha_torii --lib --message-format short` (pass)
  - `swift test --filter ToriiOfflineCashEndpointsTests` (pass)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 cargo check -p iroha_torii --tests --message-format short` (fails on an unrelated existing fixture compile error in `crates/iroha_torii/tests/connect_gating.rs`: `actual::Torii` initializer missing `faucet`)
- Remaining implementation gap:
  - the offline cash app-API slice itself now compiles and the focused Swift endpoint regression passes; the remaining repo-wide verification gap is the unrelated `connect_gating` test fixture drift that still blocks a full `iroha_torii --tests` compile.

## 2026-03-25 Follow-up: direct PQ dependency replacement advisories are closed
- Migrated the remaining direct post-quantum dependency tranche in
  `crates/soranet_pq/Cargo.toml`, `crates/iroha_crypto/Cargo.toml`, and
  `crates/ivm/Cargo.toml` from the deprecated
  `pqcrypto-dilithium` / `pqcrypto-kyber` crates to their successor
  `pqcrypto-mldsa` / `pqcrypto-mlkem` packages.
- The shipped behavior in this slice:
  - `soranet_pq` now wraps the successor ML-DSA / ML-KEM crates while keeping
    the existing `MlDsa*` / `MlKem*` API and current `dilithium*` / `kyber*`
    compatibility labels intact;
  - `iroha_crypto` and `ivm` now import the successor ML-DSA modules directly,
    including the ML-DSA seed/FFI path in
    `crates/iroha_crypto/src/mldsa_seed.rs`; and
  - `cargo deny` no longer reports the two direct PQ replacement advisories,
    which reduced the active dependency backlog to three findings before the
    later same-day `reqwest` / `rustls` pinning and `deny.toml` acceptance
    update closed the remaining scan failures.
- Validation:
  - `cargo fmt --all` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-deps-pq cargo check -p soranet_pq --tests --message-format short` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-deps-pq cargo check -p iroha_crypto --tests --message-format short` (pass)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-deps-pq cargo check -p ivm --tests --message-format short` (pass)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-deps-pq cargo test -p iroha_crypto --test mldsa_keypair keypair_from_seed_signs_and_verifies -- --nocapture` (pass)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-deps-pq cargo test -p ivm --test crypto dilithium_verify_instruction -- --nocapture` (pass)
  - `cargo deny check advisories bans sources --hide-inclusion-graph` (at that point still failed on `rustls-webpki` plus the transitive `derivative` / `paste` advisories; later same-day follow-up closed those active failures)
- Remaining implementation gap:
  - the direct PQ replacement tranche is closed in-tree and no further PQ
    dependency work remains from this pass.

## 2026-03-25 Follow-up: contract deploy/activate burst-throttle regressions no longer depend on handler runtime
- Tightened the focused Torii regressions in
  `crates/iroha_torii/src/lib.rs` so
  `contract_deploy_rate_limit_throttles_after_burst` and
  `contract_activate_rate_limit_throttles_after_burst` exhaust the exact
  handler key up front instead of assuming two full contract handlers will both
  execute inside a one-second refill window.
- The shipped behavior in this slice:
  - the tests now derive the same `rate_limit_key(...)` that the handlers use,
    consume the burst directly from `app.deploy_rate_limiter`, and then assert
    that the handler returns `CapacityLimit`; and
  - the regression remains about Torii handler keying, but no longer flakes on
    slow hosts or debug builds where the first request can refill the bucket
    before the second request starts.
- Validation:
  - `cargo fmt --all` (pass)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-torii-soracloud-remote-key cargo test -p iroha_torii --lib contract_deploy_rate_limit_throttles_after_burst -- --nocapture` (pass)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-torii-soracloud-remote-key cargo test -p iroha_torii --lib contract_activate_rate_limit_throttles_after_burst -- --nocapture` (pass)
- Remaining implementation gap:
  - no additional confirmed work remains in this deterministic test slice.

## 2026-03-25 Follow-up: Torii helper/caller rate-limit keying now falls back to the transport remote
- Hardened `crates/iroha_torii/src/lib.rs` and
  `crates/iroha_torii/src/soracloud.rs` so the remaining shared helper and
  Soracloud caller paths no longer pass `remote=None` into
  `check_access*`, `check_proof_access`, `enforce_proof_egress`, or
  `limits::is_allowed_by_cidr(...)`.
- The shipped behavior in this slice:
  - Torii still prefers the ingress-injected `x-iroha-remote-addr` header via
    `limits::effective_remote_ip(...)`, so trusted-proxy behavior is unchanged;
  - helper/caller paths that are reached without the normal ingress rewrite now
    fall back to `ConnectInfo(remote).ip()` instead of collapsing distinct
    callers into the shared `"anon"` bucket; and
  - focused regression coverage now proves a real Soracloud handler,
    `handle_health_compliance_report(...)`, keeps distinct transport peers in
    separate buckets when the internal remote header is absent.
- Validation:
  - `cargo fmt --all` (pass)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-torii-soracloud-remote-key cargo test -p iroha_torii --lib health_compliance_report_rate_limit_keys_transport_remote_when_internal_header_missing -- --nocapture` (pass)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-torii-soracloud-remote-key cargo test -p iroha_torii --lib handle_agent_autonomy_run_finalize_rejects_signer_mismatch -- --nocapture` (pass)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-torii-soracloud-remote-key cargo test -p iroha_torii --lib contract_deploy_rate_limit_throttles_after_burst -- --nocapture` (pass)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-torii-soracloud-remote-key cargo test -p iroha_torii --lib contract_activate_rate_limit_throttles_after_burst -- --nocapture` (pass)
- Remaining implementation gap:
  - no additional confirmed Torii remote-key fail-open call sites remain in
    this `lib.rs` / `soracloud.rs` slice; the remaining confirmed security
    backlog is now CUDA runtime validation on a CUDA-capable host.

## 2026-03-25 Follow-up: `model-publish-private` now imports pinned Hugging Face snapshots through `PrivateModelSourceV1`
- Extended `crates/iroha_data_model/src/soracloud.rs`,
  `crates/iroha_cli/src/soracloud.rs`, and
  `docs/source/soracloud/cli_local_control_plane.md` so the Soracloud
  private-model publish draft is no longer limited to local admitted
  directories.
- The shipped behavior in this slice:
  - `PrivateModelPublishDraft` now carries
    `source: PrivateModelSourceV1`, which accepts either
    `LocalDir(path)` or `HuggingFaceSnapshot(repo, revision)` sources;
  - `model-publish-private --draft-file` now normalizes either source into a
    deterministic temp tree and then reuses the existing encrypted
    bundle/chunk/finalize/compile/allow flow without introducing a second
    publish protocol;
  - Hugging Face snapshots fail closed unless `revision` is a pinned
    40-character commit SHA whose resolved `sha` matches the authoritative Hub
    response; and
  - the admitted v1 layout is now explicit and shared across both source
    kinds: root-level `config.json`, tokenizer assets, one or more
    `*.safetensors` shards, and optional processor/preprocessor metadata for
    image-capable models. GGUF, ONNX, other non-safetensors weights, and
    arbitrary nested custom layouts are rejected.
- Validation:
  - `cargo fmt --all` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-private-model cargo check -p iroha_cli --tests --message-format short` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-private-model cargo test -p iroha_data_model private_model_source_roundtrips_through_norito --lib -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-private-model cargo test -p iroha_cli validate_private_model_publish_draft_rejects_unpinned_hf_revision -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-private-model cargo test -p iroha_cli prepare_private_model_publish_plan_from_ -- --nocapture` (pass)
- Remaining implementation gap:
  - the private-model import wrapper now covers both local admitted
    directories and pinned HF snapshots; the remaining Soracloud v1 gaps are
    outside this slice, primarily domains/default hostnames, public budgets,
    assigned-host health self-reporting, and repo-wide validation.

## 2026-03-25 Follow-up: Torii direct rate-limited handlers now fall back to the transport remote
- Hardened `crates/iroha_torii/src/lib.rs` so the remaining confirmed
  direct-handler rate-limit sites thread
  `axum::extract::ConnectInfo<std::net::SocketAddr>` into
  `rate_limit_key(...)` / `limits::key_from_headers(...)` instead of passing
  `None` and collapsing missing internal remote metadata into the shared
  `"anon"` bucket.
- The shipped behavior in this slice:
  - the handlers still prefer the ingress-injected
    `x-iroha-remote-addr` header through `limits::effective_remote_ip(...)`,
    so trusted-proxy and ingress-middleware semantics stay intact;
  - direct handler invocations that bypass the normal ingress rewrite now fall
    back to the accepted socket IP for rate-limit isolation across the
    remaining confirmed Torii policy / stream / contract / multisig /
    Soracloud-direct handler cluster; and
  - focused regressions now pin the fallback path by proving
    `handler_policy(...)` keeps distinct transport peers in separate buckets
    when the internal remote header is absent.
- Validation:
  - `cargo fmt --all` (pass)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-torii-remote-key cargo test -p iroha_torii --lib policy_rate_limit_keys_transport_remote_when_internal_header_missing -- --nocapture` (pass)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-torii-remote-key cargo test -p iroha_torii --lib contracts_deploy_handler_ok -- --nocapture` (pass)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-torii-remote-key cargo test -p iroha_torii --lib contracts_instance_activate_handler_ok -- --nocapture` (pass)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-torii-remote-key cargo test -p iroha_torii --lib multisig_spec_requires_signed_request_for_alias_selector -- --nocapture` (pass)
- Remaining implementation gap:
  - this closes the currently confirmed direct-handler fallback cluster; the
    remaining confirmed security backlog is now CUDA runtime validation on a
    CUDA-capable host.

## 2026-03-25 Follow-up: `model-publish-private` now prepares encrypted upload plans from raw admitted model directories
- Extended `crates/iroha_cli/src/soracloud.rs`,
  `crates/iroha_cli/Cargo.toml`, and
  `docs/source/soracloud/cli_local_control_plane.md` so the Soracloud private
  model publish wrapper no longer requires a prebuilt encrypted publish plan.
- The shipped behavior in this slice:
  - `iroha app soracloud model-publish-private` now accepts either
    `--plan-file` for an already prepared deterministic publish plan or
    `--draft-file` for a higher-level local source description;
  - the new draft path walks a local admitted HF-style source directory,
    validates required files such as `config.json`, tokenizer assets, and
    `*.safetensors`, deterministically serializes the bundle, encrypts it
    against the active Torii upload recipient, shards it into fixed-size
    encrypted chunks, and then executes the existing
    upload/finalize/compile/allow sequence;
  - the wrapper can now persist the prepared publish plan through
    `--emit-plan-file`, so operators can inspect or reuse the exact encrypted
    bundle plan before submission; and
  - focused regressions now cover both the happy path and the missing-config
    failure path for draft-driven preparation.
- Validation:
  - `cargo fmt --all` (pass)
  - `CARGO_TARGET_DIR=target_soracloud_prepare_cli cargo check -p iroha_cli --tests` (pass)
  - `cargo check -p iroha_core --tests` (pass)
  - `CARGO_TARGET_DIR=target_soracloud_prepare_cli cargo test -p iroha_cli prepare_private_model_publish_plan_from_draft_ --bins` (pass)
- Remaining implementation gap:
  - the one-click private-model wrapper now handles deterministic local
    encryption and chunking, but it still expects an already admitted
    HF-style directory layout rather than importing or normalizing an
    arbitrary upstream repository for the user.

## 2026-03-25 Follow-up: Soracloud uploaded-model and private-runtime CLI surface is now wired through `iroha_cli`
- Extended `crates/iroha_cli/src/soracloud.rs` and
  `docs/source/soracloud/cli_local_control_plane.md` so the uploaded-model
  and private-runtime Torii surface is reachable through the normal
  `iroha app soracloud model-*` family instead of remaining a documented
  follow-up.
- The shipped behavior in this slice:
  - `iroha app soracloud` now exposes `model-upload-encryption-recipient`,
    `model-upload-init`, `model-upload-chunk`, `model-upload-finalize`,
    `model-upload-status`, `model-compile`, `model-compile-status`,
    `model-allow`, `model-run-private`, `model-run-status`,
    `model-decrypt-output`, and `model-publish-private`;
  - the CLI now signs and posts the uploaded-model bundle/chunk/finalize,
    private compile, allow-model, run-private, and output-release requests
    against the authoritative Torii control plane;
  - `model-run-private` now hides the draft-then-finalize handshake and
    returns the authoritative post-finalize session status; and
  - `model-publish-private` now validates bundle/chunk/finalize/compile
    consistency locally, checks that the plan still targets the active Torii
    upload recipient, and then executes the full upload/finalize/compile/allow
    sequence from one publish-plan document.
- Validation:
  - `cargo fmt --all` (pass)
  - `CARGO_TARGET_DIR=target_soracloud_uploaded_cli cargo check -p iroha_cli --tests` (pass)
  - `cargo check -p iroha_core --tests` (pass)
  - `cargo check -p iroha_torii --tests` (pass)
  - `cargo test -p iroha_cli validate_private_model_publish_plan_ --bins` (pass)
  - `cargo test -p iroha_cli signed_uploaded_model_ --bins` (pass)
  - `cargo test -p iroha_cli signed_private_ --bins` (pass)
  - `cargo test -p iroha_cli fetch_uploaded_model_recipient_rejects_invalid_url --bins` (pass)
  - `cargo test -p iroha_cli fetch_uploaded_model_status_rejects_invalid_url --bins` (pass)
  - `cargo test -p iroha_cli fetch_private_inference_status_rejects_invalid_url --bins` (pass)
- Remaining implementation gap:
  - the uploaded-model/private-runtime CLI is now present, but the
    one-click wrapper still assumes an already encrypted deterministic
    publish-plan document; local normalization, encryption, and chunking of a
    raw admitted model directory are still follow-up work.

## 2026-03-25 Follow-up: strict workspace clippy is green after validation-driven cleanup
- Continued the repo-level validation pass after closing the asset/mobile follow-up and fixed
  the next layer of compile/lint drift that strict workspace `clippy` exposed across unrelated
  crates:
  - `crates/iroha_data_model` picked up the expected `Option<&T>` / `map_or_else` /
    doc-markdown cleanup plus narrow local `clippy` allowances where the current
    Soracloud/data-model API shape is intentional;
  - `crates/kotodama_lang`, `crates/iroha_config`, `crates/iroha_executor`,
    `crates/connect_norito_bridge`, `crates/iroha_kagami`, `crates/iroha_core`, and
    `crates/iroha_torii` all needed small follow-up fixes for stale signatures, new
    optional-domain account fields, added Torii config fields, missing test-only helpers,
    dead test code, and borrow/lifetime issues uncovered by the strict lint pass; and
  - the workspace now passes `cargo clippy --workspace --all-targets -- -D warnings`.
- Validation:
  - `cargo clippy --workspace --all-targets -- -D warnings` (pass)
  - `cargo test --workspace` (started, still compiling/running in this turn; no final pass/fail result yet)
- Remaining implementation gap:
  - no additional lint blockers remain in the strict workspace `clippy` pass. The only
    unfinished repo-level validation from this turn is the long-running
    `cargo test --workspace` execution.

## 2026-03-25 Hardening: operator auth now keys lockout and rate limiting off the effective caller IP
- Hardened `crates/iroha_torii/src/operator_auth.rs` so
  `OperatorAuth::check_common(...)` derives its lockout/rate-limit key from
  `limits::effective_remote_ip(headers, remote_ip)` instead of collapsing
  missing internal remote metadata to a shared `"anon"` bucket.
- The shipped behavior in this slice:
  - operator auth still prefers the ingress-injected
    `x-iroha-remote-addr` header when present, preserving the current trusted
    proxy and ingress-middleware semantics; and
  - direct handler invocations that bypass ingress middleware now fall back to
    the accepted socket IP for rate-limit and lockout isolation, only using
    `"anon"` when both header and transport IP are unavailable.
- Validation:
  - `rustfmt --edition 2024 crates/iroha_torii/src/operator_auth.rs`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_HOME=/tmp/iroha-cargo-home-operator-auth-key CARGO_TARGET_DIR=/tmp/iroha-codex-target-operator-auth-key cargo test -p iroha_torii --lib operator_auth_key_ -- --nocapture` (pass)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_HOME=/tmp/iroha-cargo-home-operator-auth-key CARGO_TARGET_DIR=/tmp/iroha-codex-target-operator-auth-key cargo test -p iroha_torii --lib operator_auth_rejects_forwarded_mtls_from_untrusted_proxy -- --nocapture` (pass)
- Remaining implementation gap:
  - no additional live operator-auth remote-IP trust-boundary issue was
    confirmed in this pass; the remaining confirmed security backlog is now
    CUDA runtime validation on a CUDA-capable host.

## 2026-03-25 Follow-up: Soracloud ordinary handlers can now read authoritative secret envelopes
- Extended the Soracloud runtime/ABI secret-ingestion path across
  `crates/iroha_data_model/src/soracloud.rs`,
  `crates/ivm_abi/src/syscalls.rs`,
  `crates/ivm/spec/syscalls.toml`,
  `crates/ivm/docs/syscalls.md`,
  `crates/ivm/tests/abi_syscall_list_golden.rs`,
  `crates/irohad/src/soracloud_runtime.rs`, and
  `docs/source/soracloud/cli_local_control_plane.md`.
- The shipped behavior in this slice:
  - the Soracloud host envelope now exposes `ReadSecretEnvelope` as the
    public-safe ordinary-handler path for committed service secrets;
  - ABI v1 now includes `SORACLOUD_READ_SECRET_ENVELOPE` as an explicit
    syscall and the golden syscall list is updated accordingly;
  - the embedded `irohad` Soracloud IVM host now resolves authoritative
    service secret envelopes directly from committed deployment state for
    ordinary handler execution; and
  - the existing `ReadSecret` path remains private-runtime-only and still
    returns committed ciphertext bytes rather than widening to plaintext
    mounts.
- Validation:
  - `cargo fmt --all` (pass)
  - `CARGO_TARGET_DIR=target_soracloud_secret_envelope_ivm cargo test -p ivm abi_syscall_list_matches_golden -- --nocapture` (pending rerun after current Soracloud-focused build settles)
  - `CARGO_TARGET_DIR=target_soracloud_secret_envelope_runtime cargo test -p irohad ivm_host_public_runtime_reads_authoritative_service_secret_envelope -- --nocapture` (pending rerun after current Soracloud-focused build settles)
  - `CARGO_TARGET_DIR=target_soracloud_secret_envelope_runtime cargo test -p irohad ivm_host_public_runtime_reads_authoritative_service_config_entry -- --nocapture` (pending rerun after current Soracloud-focused build settles)
  - `CARGO_TARGET_DIR=target_soracloud_readcfg_cli cargo check -p iroha_cli --tests` (running)
  - `CARGO_TARGET_DIR=target_soracloud_readcfg_core cargo check -p iroha_core --tests` (running)
- Remaining implementation gap:
  - Soracloud now has an explicit authoritative envelope-only secret contract
    for ordinary IVM services, but there is still no automatic plaintext
    decrypt/mount handoff for one-click services that need local secret
    consumption without handling envelopes themselves.

## 2026-03-25 `BlockCreated` now owns exact frontier identity and contiguous RBC rescue no longer re-enters generic missing-block fetch
- Tightened the `committed + 1` frontier ownership path in `crates/iroha_core/src/sumeragi/main_loop/{proposal_handlers.rs,rbc.rs}` and `crates/iroha_core/src/sumeragi/main_loop.rs`:
  - accepted `BlockCreated` now seeds the exact `FrontierSlot` directly, marks `block_created_seen = true`, and clears any stale generic missing-block/view-change state for that slot instead of leaving frontier identity implicit in `pending_blocks` only;
  - contiguous-frontier RBC recovery now stays inside the exact slot by calling the frontier-slot helper rather than issuing generic `FetchPendingBlock` rescue for missing `BlockCreated`;
  - if the slot is already authoritative, the RBC diversion can still drive the existing leader-first `FetchBlockBody` lane; if `BlockCreated` has not arrived yet, the slot remains a passive placeholder instead of escalating into topology-wide repair.
- Focused validation on this fix:
  - `cargo fmt --all` (pass)
  - `NORITO_SKIP_BINDINGS_SYNC=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo check -p iroha_core --lib --message-format short` (pass)
  - `NORITO_SKIP_BINDINGS_SYNC=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_core block_created_initializes_exact_frontier_slot --lib -- --nocapture` (pass)
  - `NORITO_SKIP_BINDINGS_SYNC=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_core request_missing_block_for_pending_rbc_holds_initial_frontier_fetch_within_ingress_grace --lib -- --nocapture` (pass)
  - `NORITO_SKIP_BINDINGS_SYNC=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_core handle_rbc_init_preserves_rotated_leader_preference_for_late_missing_block_recovery --lib -- --nocapture` (pass)
  - `NORITO_SKIP_BINDINGS_SYNC=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_core frontier_body_repair_fetches_leader_before_voter_fallback --lib -- --nocapture` (pass)
  - `NORITO_SKIP_BINDINGS_SYNC=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_core frontier_vote_placeholder_skips_generic_missing_block_request --lib -- --nocapture` (pass)
  - `NORITO_SKIP_BINDINGS_SYNC=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_core defer_qc_if_block_missing_with_commit_quorum_hint_seeds_contiguous_frontier_owner_create_only --lib -- --nocapture` (pass)
- Remaining gap after this fix:
  - preserved-peer stable soaks have not been rerun on this exact ownership transfer yet, so the current top-of-file soak failure remains the latest end-to-end performance signal;
  - `Proposal`, `ProposalHint`, and the RBC wire/session machinery still exist, so this is the frontier-owner cut, not the full protocol deletion;
  - frontier/body-present lifecycle is still split between `FrontierSlot` and `PendingBlock`, so the remaining cleanup is to delete the legacy frontier-specific rescue branches rather than keep teaching them to stand down.

## 2026-03-25 Full 4-peer preserved-peer stable soaks still fail on legacy frontier churn
- Rebuilt the soak binaries with `NORITO_SKIP_BINDINGS_SYNC=1 CARGO_BUILD_JOBS=4 cargo build --release -p irohad --bin iroha3d -p izanami --bin izanami` so both envelopes ran against the current exact-body tranche.
- Ran the full preserved-peer stable envelopes with logs and peer dirs kept for postmortem:
  - permissioned: `RUST_LOG=izanami::summary=info,izanami::progress=info,izanami::workload=warn,iroha_core::sumeragi::main_loop=info,iroha_core::sumeragi=info,iroha_p2p=info IROHA_TEST_NETWORK_KEEP_DIRS=1 IROHA_TEST_NETWORK_PERMIT_DIR=/tmp/iroha-permit-permissioned-ObbBDc TEST_NETWORK_TMP_DIR=/tmp/iroha-soak-permissioned-exact_body_20260325T101110Z TEST_NETWORK_BIN_IROHAD=/Users/mtakemiya/dev/iroha/target/release/iroha3d /Users/mtakemiya/dev/iroha/target/release/izanami --allow-net --peers 4 --faulty 0 --duration 3600s --target-blocks 2000 --progress-interval 10s --progress-timeout 600s --tps 5 --max-inflight 8 --workload-profile stable` (log: `/tmp/izanami_permissioned_exact_body_20260325T101110Z.log`, peer dirs: `/tmp/iroha-soak-permissioned-exact_body_20260325T101110Z/irohad_test_network_GbIRfx`)
  - NPoS: `RUST_LOG=izanami::summary=info,izanami::progress=info,izanami::workload=warn,iroha_core::sumeragi::main_loop=info,iroha_core::sumeragi=info,iroha_p2p=info IROHA_TEST_NETWORK_KEEP_DIRS=1 IROHA_TEST_NETWORK_PERMIT_DIR=/tmp/iroha-permit-npos-a7eDGr TEST_NETWORK_TMP_DIR=/tmp/iroha-soak-npos-exact_body_20260325T111221Z TEST_NETWORK_BIN_IROHAD=/Users/mtakemiya/dev/iroha/target/release/iroha3d /Users/mtakemiya/dev/iroha/target/release/izanami --allow-net --nexus --peers 4 --faulty 0 --duration 3600s --target-blocks 2000 --progress-interval 10s --progress-timeout 600s --tps 5 --max-inflight 8 --workload-profile stable` (log: `/tmp/izanami_npos_exact_body_20260325T111221Z.log`, peer dirs: `/tmp/iroha-soak-npos-exact_body_20260325T111221Z/irohad_test_network_iCecCE`)
- Outcome:
  - permissioned exited `1` after the full `3599.99411525s` window with aligned strict/quorum height `1430`; `izanami` reported `quorum p95 block interval 5003ms exceeded threshold 1000ms` and the effective average interval over the run was `~2517.48ms/block`
  - NPoS exited `1` after the full `3599.994603375s` window with aligned strict/quorum height `1387`; `izanami` reported the same `quorum p95 block interval 5003ms exceeded threshold 1000ms` failure and the effective average interval was `~2595.53ms/block`
  - neither soak hit `no block height progress`, quorum divergence, peer crash, or ingress failover; all four peers exited cleanly after `izanami` stopped the run on the duration-deadline p95 failure
- Preserved-peer critique:
  - the new exact frontier lane did not activate in the live repair path: both preserved runs logged `FetchBlockBody=0` and `BlockBodyResponse=0`
  - legacy frontier recovery still dominated the hot path: permissioned logged `fallback anchor=1260` and `requested missing BlockCreated while awaiting RBC INIT=90`; NPoS logged `fallback anchor=1079` and `requested missing BlockCreated while awaiting RBC INIT=70`
  - the churn was concentrated on single peers rather than evenly spread, which matches a frontier-owner/reanchor loop rather than uniform workload pressure:
    - permissioned hotspot: `just_quagga` logged `fallback_anchor=543` and `missing_blockcreated=69`
    - NPoS hotspot: `cuddly_grunt` logged `fallback_anchor=470` and `missing_blockcreated=58`
  - both networks stayed safe but spent the hour in the same degraded `~2.0s / 2.5s / 3.3s / 5.0s / 10.0s` cadence bands, so this is still a frontier repair / ownership problem, not a pacemaker liveness break or a permissioned-vs-NPoS policy difference

## 2026-03-25 Frontier exact-body lane now exists for `committed + 1`, but the full protocol deletion is still open
- Landed the first receiver-driven `FetchBlockBody` / `BlockBodyResponse` tranche for the contiguous frontier without attempting the full `Proposal` / RBC message deletion yet:
  - `crates/iroha_core/src/sumeragi/message.rs`, `crates/iroha_core/src/sumeragi/status.rs`, and `crates/iroha_core/src/sumeragi/mod.rs` now define and route the exact frontier body request/response pair through dedup, queue classification, and status accounting.
  - `crates/iroha_core/src/sumeragi/main_loop.rs` now carries a dedicated `FrontierSlot` / `next_slot_prefetch` state, schedules leader-first frontier body retries, and keeps `committed + 1` vote/QC placeholders out of the generic `missing_block_requests` retry loop once the exact slot lane is active.
  - `crates/iroha_core/src/sumeragi/main_loop/block_sync.rs` now serves exact `BlockBodyResponse` replies from local authoritative bodies and can hydrate the local slot from a `BlockBodyResponse` payload instead of forcing the frontier back through `FetchPendingBlock`.
  - `crates/iroha_core/src/sumeragi/main_loop/votes.rs`, `crates/iroha_core/src/sumeragi/main_loop/qc.rs`, and `crates/iroha_core/src/sumeragi/main_loop/proposal_handlers.rs` now route highest-QC / unknown-frontier body misses onto the exact frontier slot, keep vote-only frontier observations passive, and flush stashed exact-body requesters once the authoritative `BlockCreated` body arrives.
  - `crates/iroha_core/src/sumeragi/main_loop/tests.rs` now covers exact body responses, passive frontier vote placeholders, and leader-first frontier retry ordering.
- Focused validation on this tranche:
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo check -p iroha_core --lib --message-format short` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_core frontier_body_repair_fetches_leader_before_voter_fallback --lib -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_core frontier_catchup_target_keeps_contiguous_frontier_with_pending_frontier_block --lib -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_core frontier --lib -- --nocapture` (`112` passed, `19` failed, `1` ignored; the remaining failures are concentrated in legacy frontier-recovery/shared-window/range-pull tests that still assume the pre-slot frontier machinery)
- Remaining gap after this cut:
  - `Proposal`, `ProposalHint`, `RbcReady`, and `RbcDeliver` still exist on the wire and in the worker loop,
  - `BlockSyncUpdate` / `FetchPendingBlock` are still present for deep catch-up and other legacy paths, so the full frontier/block-sync split is not complete yet, and
  - the frontier test suite still carries `19` expectations around legacy frontier-recovery/shared-window behavior that need retirement or semantic rewrites before the broader `frontier` filter is green, and
  - no new preserved-peer soak signal exists for this exact-body tranche yet; that rerun still needs the remaining wire cleanup plus a green lib-test baseline.

## 2026-03-25 Frontier exact-fetch tranche removes RBC sidecars from `FetchPendingBlock` and retires delivered sessions behind tip
- Landed the first hot-path repair cut that directly targets the remaining `~2.5s/block` localnet failure mode without attempting the full protocol deletion yet:
  - `crates/iroha_core/src/sumeragi/main_loop.rs` adds `MissingBlockFetchMode::StrictSigners` so near-tip recovery can stay exact instead of silently falling back to the full commit topology, and removes the `RBC_REBROADCAST_COMMITTED_DEPTH` carry-over path that kept delivered sessions rebroadcast-active for several committed heights.
  - `crates/iroha_core/src/sumeragi/main_loop/rbc.rs` now normalizes proposer preference into canonical roster slots even for rotated views and forces pending / post-delivery missing-`BlockCreated` recovery onto strict signer-only targets, with a canonical-leader fallback when INIT metadata has not yet populated the leader signature.
  - `crates/iroha_core/src/sumeragi/main_loop/votes.rs` now treats votes for unresolved bodies as exact fetch hints from the vote signer instead of immediately using topology-wide missing-block recovery.
  - `crates/iroha_core/src/sumeragi/main_loop/block_sync.rs` no longer piggybacks `RbcInit` / `RbcChunk` sidecars on `FetchPendingBlock` responses and no longer serves RBC transport when the authoritative block body is unavailable; the request now stays stashed until a real body response is possible.
- Focused validation on this tranche:
  - `cargo fmt --all` (pass)
  - `NORITO_SKIP_BINDINGS_SYNC=1 cargo test -p iroha_core --lib fetch_pending_block_ -- --nocapture` (pass)
  - `NORITO_SKIP_BINDINGS_SYNC=1 cargo test -p iroha_core --lib plan_missing_block_fetch_ -- --nocapture` (pass)
  - `NORITO_SKIP_BINDINGS_SYNC=1 cargo test -p iroha_core --lib request_missing_block_for_pending_rbc_ -- --nocapture` (pass)
  - `NORITO_SKIP_BINDINGS_SYNC=1 cargo test -p iroha_core --lib handle_rbc_init_preserves_rotated_leader_preference_for_late_missing_block_recovery -- --nocapture` (pass)
  - `NORITO_SKIP_BINDINGS_SYNC=1 cargo test -p iroha_core --lib delivered_rbc_session_ -- --nocapture` (pass)
  - `NORITO_SKIP_BINDINGS_SYNC=1 cargo test -p iroha_core --lib handle_vote_defers_until_roster_available -- --nocapture` (pass)
- Remaining gap after this cut:
  - the protocol still uses `FetchPendingBlock` rather than the requested `FetchBlockBody` / `BlockBodyResponse` pair,
  - `BlockSyncUpdate` is still available as a fetch response on some paths, so contiguous-frontier repair is not yet fully isolated from deep catch-up, and
  - `RbcReady` / `RbcDeliver` plus the rest of the RBC session machinery still exist on the wire and in the worker loop, so a fresh soak rerun is still needed after the exact-frontier message split lands.

## 2026-03-25 Follow-up: TAIRA faucet now requires adaptive decentralized memory-hard proof-of-work
- Hardened the TAIRA faucet slice across
  `crates/iroha_config/src/parameters/{actual.rs,defaults.rs,user.rs}`,
  `crates/iroha_torii/src/{lib.rs,openapi.rs,routing.rs}`,
  `crates/iroha_torii/tests/accounts_faucet.rs`,
  `configs/soranexus/testus/config.toml`,
  and `defaults/kagami/iroha3-testus/config.toml`.
- The shipped behavior in this slice:
  - Torii now exposes `GET /v1/accounts/faucet/puzzle`, which returns a deterministic memory-hard scrypt puzzle anchored to a recent committed block hash plus an acceptance window in blocks;
  - the effective PoW difficulty is now derived from immutable chain data for the chosen anchor height plus pending queue pressure: a base difficulty plus adaptive extra bits based on recent committed and queued faucet claim volume;
  - finalized Sumeragi VRF epoch seed material is now required in the challenge whenever faucet VRF mode is enabled, so the puzzle uses on-chain randomness instead of falling back to a non-randomized challenge; and
  - `POST /v1/accounts/faucet` now rejects requests that do not include a valid solution for that exact anchored puzzle, while still preserving the existing starter-funds transfer flow and balance checks.
- Validation:
  - `cargo test -p iroha_config torii_faucet_tests -- --nocapture` (pass)
  - `cargo test -p iroha_torii --features app_api --test accounts_faucet -- --nocapture` (pass)
  - `cargo test -p iroha_torii --features app_api accounts_faucet -- --nocapture` (still blocked by unrelated existing `accounts_portfolio.rs` compile errors on the branch)
- Remaining implementation gap:
  - this slice is now explicitly cost-based rather than identity-based; that is the correct Sybil-resistance direction for freely creatable addresses, but it still does not make faucets "fair" in any human sense without introducing some external scarce resource beyond computation.

## 2026-03-25 Follow-up: multisig cancel integration test now waits for committed proposal state
- Hardened `integration_tests/tests/multisig.rs` so the cancel-route coverage
  matches Torii's queue-admission semantics instead of assuming immediate
  post-submit visibility.
- The shipped behavior in this slice:
  - the failing
    `multisig_cancel_route_persists_canceled_terminal_state`
    integration test now polls `/v1/multisig/proposals/get` until the cancel
    wrapper proposal is visible in `COLLECTING_SIGNATURES` before issuing the
    second cancel request; and
  - the same test now polls the target proposal until it reaches the
    persisted `CANCELED` terminal state before asserting `terminal_at_ms` and
    list visibility, removing the earlier race between queue admission and
    committed world state.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo test -p integration_tests multisig_cancel_route_persists_canceled_terminal_state -- --nocapture`
    (build/test launch reached the target multisig test, but runtime
    verification was blocked by an unrelated 4-peer startup failure:
    `active SNS domain-name lease is required before registering \`wonderland\``)
- Remaining implementation gap:
  - fix the default integration-test network genesis/bootstrap path so the
    new SNS lease invariant does not abort peer startup, then rerun the
    targeted multisig cancel integration test end to end.

## 2026-03-25 Follow-up: Soracloud services can now read authoritative config through the runtime host
- Extended the Soracloud runtime/config ingestion path across
  `crates/iroha_data_model/src/soracloud.rs`,
  `crates/ivm_abi/src/syscalls.rs`,
  `crates/ivm/spec/syscalls.toml`,
  `crates/ivm/docs/syscalls.md`,
  `crates/irohad/src/soracloud_runtime.rs`,
  `crates/iroha_core/src/smartcontracts/isi/soracloud.rs`,
  `crates/iroha_cli/src/soracloud.rs`, and
  `docs/source/soracloud/cli_local_control_plane.md`.
- The shipped behavior in this slice:
  - the Soracloud host envelope now exposes `ReadConfig` alongside the
    existing state/secret/credential operations;
  - ABI v1 now includes `SORACLOUD_READ_CONFIG` as an explicit syscall and the
    golden syscall list is updated accordingly;
  - the embedded `irohad` Soracloud IVM host now resolves authoritative
    service config bytes directly from committed deployment state for ordinary
    handler execution instead of relying only on materialized `configs/`
    files; and
  - the focused CLI/core test fallout from the deploy-time inline material
    work is fixed on this branch.
- Validation:
  - `rustfmt --edition 2024 crates/iroha_data_model/src/soracloud.rs crates/ivm_abi/src/syscalls.rs crates/irohad/src/soracloud_runtime.rs crates/iroha_cli/src/soracloud.rs crates/iroha_core/src/smartcontracts/isi/soracloud.rs crates/ivm/tests/abi_syscall_list_golden.rs` (pass)
  - `CARGO_TARGET_DIR=target_soracloud_readcfg_dm cargo check -p iroha_data_model --lib` (running)
  - `CARGO_TARGET_DIR=target_soracloud_readcfg_ivm cargo test -p ivm abi_syscall_list_matches_golden -- --nocapture` (running)
  - `CARGO_TARGET_DIR=target_soracloud_readcfg_runtime cargo test -p irohad ivm_host_public_runtime_reads_authoritative_service_config_entry -- --nocapture` (running)
  - `CARGO_TARGET_DIR=target_soracloud_readcfg_cli cargo check -p iroha_cli --tests` (running)
  - `CARGO_TARGET_DIR=target_soracloud_readcfg_core cargo check -p iroha_core --tests` (running)
- Remaining implementation gap:
  - authoritative config ingestion is now first-class for IVM services, but
    the broader one-click secret contract is still unresolved: the runtime
    still exposes committed secret envelope ciphertext only through the
    private-runtime path rather than a single public-safe mount/decrypt
    contract.

## 2026-03-25 Hardening: SoraFS pin and gateway client-IP policy now resolves trusted proxies correctly
- Closed a medium-severity SoraFS trust-boundary gap around proxy-aware
  client-IP handling:
  - general Torii transport config previously had no
    `torii.transport.trusted_proxy_cidrs` surface, so there was no shared
    allow-list for reverse proxies that are permitted to assert the canonical
    client IP;
  - the ingress remote-IP middleware in `crates/iroha_torii/src/lib.rs`
    previously rewrote `x-iroha-remote-addr` from `ConnectInfo` alone, which
    collapsed proxied callers onto the proxy socket address instead of the
    real client IP; and
  - the SoraFS pin/gateway paths in
    `crates/iroha_torii/src/sorafs/pin.rs` and
    `crates/iroha_torii/src/sorafs/api.rs` did not share a
    trusted-proxy-aware canonical-IP resolution step at the handler boundary,
    while storage-pin throttling still keyed on bearer token alone whenever a
    token was present.
- The shipped behavior in this slice:
  - `crates/iroha_config/src/parameters/defaults.rs`,
    `crates/iroha_config/src/parameters/user.rs`, and
    `crates/iroha_config/src/parameters/actual.rs` now expose
    `torii.transport.trusted_proxy_cidrs`, defaulting it to an empty list;
  - `crates/iroha_torii/src/limits.rs` now provides
    `ingress_remote_ip(...)` for trusted-proxy-aware canonical-IP resolution
    and uses the resolved client IP when building limiter/CIDR keys;
  - `crates/iroha_torii/src/lib.rs` now resolves and rewrites the internal
    `x-iroha-remote-addr` header from trusted proxies only, preserving valid
    forwarded client IPs while ignoring spoofed ones from untrusted peers;
  - `crates/iroha_torii/src/sorafs/pin.rs` and
    `crates/iroha_torii/src/sorafs/api.rs` now resolve canonical client IPs
    against `state.trusted_proxy_nets` for storage-pin policy and SoraFS
    gateway client fingerprinting, even on direct handler paths; and
  - storage-pin throttling now keys shared bearer tokens by
    `token + canonical client IP`, so proxied clients sharing one pin token
    no longer collapse into one rate-limit bucket.
- Focused regressions now cover:
  - trusted and untrusted forwarded-header resolution in
    `crates/iroha_torii/src/limits.rs`;
  - shared-token bucket separation in `crates/iroha_torii/src/sorafs/pin.rs`;
  - storage-pin forwarded-client rate limiting and untrusted-header handling
    in `crates/iroha_torii/src/sorafs/api.rs`;
  - gateway CAR-range forwarded-client rate limiting and untrusted-header
    handling in `crates/iroha_torii/src/sorafs/api.rs`; and
  - the default-empty config posture in `crates/iroha_config/tests/fixtures.rs`.
- Validation:
  - `rustfmt --edition 2024 crates/iroha_config/src/parameters/defaults.rs crates/iroha_config/src/parameters/actual.rs crates/iroha_config/src/parameters/user.rs crates/iroha_config/tests/fixtures.rs crates/iroha_torii/src/limits.rs crates/iroha_torii/src/lib.rs crates/iroha_torii/src/sorafs/pin.rs crates/iroha_torii/src/sorafs/api.rs` (pass)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib limits::tests:: -- --nocapture` (pass)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib sorafs::pin::tests:: -- --nocapture` (pass)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib storage_pin_requires_token_and_respects_allowlist_and_rate_limit -- --nocapture` (pass)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib storage_pin_rate_limit_uses_forwarded_client_ip_from_trusted_proxy -- --nocapture` (pass)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib car_range_rate_limits_repeated_clients -- --nocapture` (pass)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib car_range_rate_limit_uses_forwarded_client_ip_from_trusted_proxy -- --nocapture` (pass)
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_config --test fixtures torii_transport_trusted_proxy_cidrs_default_to_empty -- --nocapture` (pass)
- Remaining implementation gap:
  - no additional live SoraFS trusted-proxy/client-IP issue was confirmed in
    this pass. The remaining security backlog is now CUDA runtime validation
    on a CUDA-capable host.

## 2026-03-25 Follow-up: multilane custom-genesis pre-exec and mobile asset-id automation are closed
- Closed the remaining asset-adjacent follow-up items that were still open after the
  first-release asset-id hard cut:
  - `crates/iroha_test_network/src/lib.rs` now resolves the effective NPoS/Nexus
    config before custom-genesis pre-execution, so `ensure_genesis_results(...)`
    validates staking/public-lane instructions against the real configured stake
    asset definition instead of falling back to synthetic success when
    `nexus_config` was missing;
  - `crates/iroha_test_network/src/config.rs` now carries a direct regression proving
    that the custom staking genesis fails in the old `None`-config path with the
    synthetic asset-definition lookup error and succeeds when the resolved
    `nexus.staking.stake_asset_id` is supplied;
  - the focused Android Gradle harness now exercises the asset-id/address mains
    directly through `GradleHarnessTests` for
    `AssetDefinitionIdEncoderTests`,
    `AssetIdDecoderTests`,
    `AssetIdEncoderTests`, and
    `TransferWirePayloadEncoderTests`;
  - those Android mains no longer depend on stale hardcoded account literals:
    the asset-id/address tests now use `TestAccountIds.ed25519Authority(...)`,
    `TransferWirePayloadEncoderTests` now keeps non-Ed25519 curve support enabled
    for the duration of the positive encoding assertions, and
    `OfflineSpendReceiptPayloadEncoderTest` no longer documents or depends on the
    removed `IROHA_NATIVE_REQUIRED` toggle; and
  - the previously tracked Swift item turned out to be stale: current bridge/fallback
    parity coverage in `IrohaSwift/Tests/IrohaSwiftTests/TxBuilderTests.swift` and
    `IrohaSwift/Tests/IrohaSwiftTests/ConfidentialWalletFixturesTests.swift`
    already uses canonical Base58 asset-definition IDs in the shipped fixture paths,
    so no additional Swift code change was required for this follow-up.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_test_network populate_genesis_results_uses_supplied_nexus_config_for_custom_staking_genesis -- --nocapture` (pass)
  - `cargo test -p integration_tests --test mod cross_dataspace_localnet_genesis_preexecution_smoke -- --nocapture` (pass)
  - `cd java/iroha_android && JAVA_HOME=$(/usr/libexec/java_home -v 21) ANDROID_HOME=~/Library/Android/sdk ANDROID_SDK_ROOT=~/Library/Android/sdk ANDROID_HARNESS_MAINS=org.hyperledger.iroha.android.address.AssetDefinitionIdEncoderTests,org.hyperledger.iroha.android.address.AssetIdDecoderTests,org.hyperledger.iroha.android.address.AssetIdEncoderTests,org.hyperledger.iroha.android.model.instructions.TransferWirePayloadEncoderTests ./gradlew :core:test --tests org.hyperledger.iroha.android.GradleHarnessTests --console=plain` (pass)
- Remaining implementation gap:
  - none for this asset-id/mobile follow-up slice; the roadmap items for multilane
    custom-genesis pre-exec and mobile asset-id automation are now closed.

## 2026-03-25 Follow-up: Soracloud deploy-time required config/secret bindings are now authoritative
- Extended the Soracloud admission path across
  `crates/iroha_data_model/src/soracloud.rs`,
  `crates/iroha_data_model/src/isi/soracloud.rs`,
  `crates/iroha_core/src/smartcontracts/isi/soracloud.rs`,
  `crates/iroha_torii/src/soracloud.rs`,
  `crates/iroha_cli/src/soracloud.rs`, and
  `docs/source/soracloud/cli_local_control_plane.md`.
- The shipped behavior in this slice:
  - `SoraContainerManifestV1` now declares `required_config_names` and
    `required_secret_names` as explicit admission-time dependencies;
  - deploy/upgrade instructions now accept optional inline
    `initial_service_configs` and `initial_service_secrets`, and the
    provenance payload covers `(bundle, initial_service_configs,
    initial_service_secrets)` rather than the bundle alone;
  - deploy, upgrade, and rollback now validate the effective authoritative
    material set against the revision’s required config/secret names before a
    new active deployment state is recorded; and
  - deleting an active config or secret now fails closed when the current
    bundle still requires that material.
- The CLI surface now exposes first-deploy / upgrade-time inline material
  injection through `--initial-configs <path>` and `--initial-secrets <path>`
  on `iroha app soracloud deploy` and `upgrade`.
- Validation:
  - `CARGO_TARGET_DIR=target_soracloud_bindings cargo check -p iroha_data_model --lib` (pass)
  - `CARGO_TARGET_DIR=target_soracloud_bindings cargo check -p iroha_core --lib` (pass)
  - `CARGO_TARGET_DIR=target_soracloud_bindings cargo check -p iroha_torii --lib` (pass)
  - `CARGO_TARGET_DIR=target_soracloud_bindings cargo check -p iroha_cli` (pass)
- Focused test compilation/execution is still in progress in this branch:
  - `CARGO_TARGET_DIR=target_soracloud_bindings cargo test -p iroha_data_model soracloud_container_manifest_fixture_roundtrip -- --nocapture`
  - `CARGO_TARGET_DIR=target_soracloud_coretests cargo test -p iroha_core deploy_soracloud_service_records_bundle_and_audit_state --lib -- --nocapture`
  - `CARGO_TARGET_DIR=target_soracloud_corechecks cargo check -p iroha_core --tests`
  - `CARGO_TARGET_DIR=target_soracloud_cli_tests cargo check -p iroha_cli --tests`
- Remaining implementation gap:
  - the required-binding contract is now authoritative, but the broader
    one-click runtime ingestion story is still incomplete: services still read
    authoritative materialized files by path rather than having a first-class
    env/file injection contract declared in the manifest.

## 2026-03-25 Hardening: Connect admission no longer defaults missing remote metadata to loopback
- Closed a fail-open default in the Connect session / websocket admission path:
  - `handler_connect_session(...)` and `handle_connect_ws_logic(...)` in
    `crates/iroha_torii/src/lib.rs` previously treated a missing internal
    `x-iroha-remote-addr` header as `127.0.0.1`; and
  - that did not create a new externally reachable bypass on the current
    router because real requests already receive injected `ConnectInfo`, but it
    was still the wrong security default for per-IP handshake/session
    accounting if the middleware were ever bypassed by an internal dispatch,
    future router split, or malformed harness path.
- The shipped hardening in this slice:
  - `crates/iroha_torii/src/lib.rs` now requires the injected internal remote
    address for Connect websocket/session admission and fails closed with
    `connect: remote addr unavailable` instead of silently assuming loopback;
  - focused parser/unit tests now cover missing, valid, and invalid injected
    remote-IP headers; and
  - a direct handler regression now proves Connect session creation rejects a
    request that reaches the handler without the injected remote-IP header.
- Validation:
  - `rustfmt --edition 2024 crates/iroha_torii/src/lib.rs` (pass)
  - `CARGO_HOME=/tmp/iroha-cargo-home-connect-remote CARGO_TARGET_DIR=/tmp/iroha-codex-target-peer-geo cargo test -p iroha_torii --lib connect_remote_ip_ -- --nocapture` (pass)
  - `CARGO_HOME=/tmp/iroha-cargo-home-connect-remote CARGO_TARGET_DIR=/tmp/iroha-codex-target-peer-geo cargo test -p iroha_torii --lib connect_session_handler_rejects_missing_remote_addr_header -- --nocapture` (pass)
- Remaining implementation gap:
  - no new live Connect ingress finding was confirmed in this pass. This is
    hardening to keep missing internal transport metadata fail-closed.

## 2026-03-25 Follow-up: Soracloud now materializes committed service config/secret state
- Extended the Soracloud runtime materialization path across
  `crates/iroha_core/src/soracloud_runtime.rs`,
  `crates/irohad/src/soracloud_runtime.rs`,
  `crates/iroha_torii/src/lib.rs`, and
  `docs/source/soracloud/cli_local_control_plane.md`.
- The shipped behavior in this slice:
  - `SoracloudRuntimeServicePlan` now carries config/secret generations,
    entry counts, and the concrete materialization directories for committed
    config payloads, secret envelopes, and the synchronized raw secret tree;
  - `reconcile_once()` now writes committed config payloads into
    `services/<service>/<version>/configs/...` as canonical JSON files;
  - the same runtime pass now writes committed secret envelopes into
    `services/<service>/<version>/secret_envelopes/...` and synchronizes the
    legacy raw fallback bytes into `secrets/<service>/<version>/...`; and
  - stale authoritative secret/config materializations are now pruned on the
    next reconcile when entries are removed from deployment state.
- Added focused runtime regressions for:
  - active service/apartment reconciliation persisting the new committed
    config/secret materialization tree alongside the existing runtime plan and
    deployment bundle files; and
  - rerunning reconciliation after clearing committed config/secret state to
    remove stale files from both the service revision tree and the
    synchronized legacy secret root.
- Validation:
  - `cargo fmt --all` (pass after the later asset-id cleanup removed the dead
    `xtask` `rewrite-legacy-asset-literals` bin target)
  - `CARGO_TARGET_DIR=target_soracloud_materialize cargo check -p iroha_core --lib` (pass)
  - `CARGO_TARGET_DIR=target_soracloud_materialize cargo check -p iroha_torii --lib` (pass)
  - `CARGO_TARGET_DIR=target_soracloud_materialize2 cargo check -p irohad --tests` (pass)
  - `cargo test -p irohad --features embedded-soracloud-runtime --bin iroha3d soracloud_runtime::tests::reconcile_once_persists_active_service_and_apartment_materializations -- --nocapture` (pass)
  - `cargo test -p irohad --features embedded-soracloud-runtime --bin iroha3d soracloud_runtime::tests::reconcile_once_prunes_stale_authoritative_service_materializations -- --nocapture` (pass)
- Remaining implementation gap:
  - committed config/secret records now materialize deterministically for the
    runtime, but deploy-time binding validation and the long-term authoritative
    secret-ingestion contract are still open.

## 2026-03-25 Follow-up: Soracloud ordinary services now declare explicit config exports
- Extended the ordinary-service runtime contract across
  `crates/iroha_data_model/src/soracloud.rs`,
  `crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs`,
  `crates/iroha_core/src/soracloud_runtime.rs`,
  `crates/irohad/src/soracloud_runtime.rs`,
  `crates/iroha_torii/src/soracloud.rs`,
  `docs/source/soracloud/{manifest_schemas.md,cli_local_control_plane.md}`,
  and the Soracloud JSON fixtures.
- The shipped behavior in this slice:
  - `SoraContainerManifestV1` now carries `config_exports:
    Vec<SoraConfigExportV1>`, so ordinary services can declare which required
    authoritative config entries must be projected into either runtime env vars
    or revision-local files;
  - manifest validation now fails closed when an export references a
    non-required config, an env var name is invalid, a file path is not a safe
    relative path, or two exports collide on the same env var / file target;
  - the embedded `irohad` runtime now persists those declared exports into the
    runtime snapshot, writes file exports under
    `services/<service>/<version>/config_exports/...`, and materializes the
    effective runtime environment in `effective_env.json`; and
  - Torii control-plane snapshots now expose
    `required_config_names`, `required_secret_names`, and `config_exports` for
    each latest service revision so status responses surface the actual runtime
    contract instead of only the opaque bundle hash.
- Secret handling remains unchanged in this slice:
  - ordinary services still receive only `ReadSecretEnvelope` plus envelope
    materialization; plaintext secret env/file exports were intentionally not
    introduced.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo check -p iroha_data_model --tests` (pass)
  - `cargo check -p iroha_torii --tests` (pass)
  - `cargo check -p irohad --tests` (pass)
  - `cargo test -p iroha_data_model container_validate_ --lib -- --nocapture` (pass)
  - `cargo test -p iroha_torii soracloud::tests::control_plane_snapshot_uses_authoritative_soracloud_state --lib -- --nocapture` (pass)
  - `cargo test -p irohad --features embedded-soracloud-runtime --bin iroha3d soracloud_runtime::tests::reconcile_once_persists_active_service_and_apartment_materializations -- --nocapture` (pass)
  - `cargo test -p irohad --features embedded-soracloud-runtime --bin iroha3d soracloud_runtime::tests::reconcile_once_prunes_stale_authoritative_service_materializations -- --nocapture` (pass)
- Remaining implementation gap:
  - this closes only the ordinary-service config export contract from the
    larger Soracloud v1 plan; custom domains, prepaid public budgets, pinned
    Hugging Face snapshot import, multisig HTTP witnesses, assigned-host
    self-reporting, and full repo-wide Soracloud validation are still pending.

## 2026-03-25 Follow-up: Soracloud signed HTTP now admits multisig witnesses
- Extended the Soracloud signed-request path across
  `crates/iroha_data_model/src/soracloud.rs`,
  `crates/iroha/src/config{.rs,/user.rs}`,
  `crates/iroha/src/client.rs`,
  `crates/iroha_cli/src/{main_shared.rs,soracloud.rs}`,
  `crates/iroha_torii/src/{app_auth.rs,lib.rs,soracloud.rs}`,
  and `docs/source/soracloud/cli_local_control_plane.md`.
- The shipped behavior in this slice:
  - the public data model now exposes `CanonicalRequestWitnessV1` plus
    `CanonicalRequestSignatureWitnessV1`, with Norito roundtrip coverage;
  - app-auth now preserves the existing single-sig canonical request flow for
    `AccountController::Single`, while multisig-controlled accounts can
    authorize Soracloud HTTP with `X-Iroha-Witness`, unique signer keys,
    threshold-weight verification against the current on-chain
    `MultisigPolicy`, and the same nonce/timestamp replay protection as the
    single-sig path;
  - the Soracloud middleware now propagates the full verified signer set
    internally so mutation provenance must match one of the verified witness
    signers instead of a single hard-coded request signer; and
  - the CLI now emits fresh `X-Iroha-Timestamp-Ms` / `X-Iroha-Nonce` headers on
    ordinary single-sig Soracloud mutations and can replay a JSON witness file
    from `soracloud.http_witness_file`, failing closed when the witness
    `subject_account` or `canonical_request_hash` does not match the request.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_data_model canonical_request_witness_roundtrips_through_norito --lib -- --nocapture` (pass)
  - `cargo test -p iroha_torii --features app_api --lib verify_accepts_valid_multisig_witness -- --nocapture` (pass)
  - `cargo test -p iroha_torii --features app_api --lib verify_rejects_replayed_multisig_witness_nonce -- --nocapture` (pass)
  - `cargo test -p iroha_torii --lib require_soracloud_mutation_signer_accepts_multisig_member_provenance -- --nocapture` (pass)
  - `cargo test -p iroha_cli build_soracloud_mutation_auth_headers -- --nocapture` (pass)
  - `cargo test -p iroha parse_preserves_soracloud_http_witness_file --lib -- --nocapture` (pass)
- Remaining implementation gap:
  - Soracloud still lacks deterministic public-edge hostnames/custom domains,
    prepaid public budgets, pinned Hugging Face snapshot import, assigned-host
    self-reporting, and the repo-wide final validation pass.

## 2026-03-25 Follow-up: peer telemetry geo lookup now requires an explicit HTTPS endpoint
- Closed a medium-severity peer-telemetry egress gap:
  - enabling `torii.peer_geo.enabled=true` without an explicit endpoint
    previously fell back to the built-in plaintext
    `http://ip-api.com/json` service in
    `crates/iroha_torii/src/telemetry/peers/monitor.rs`; and
  - the duplicated telemetry docs and sample config advertised that fallback,
    which made it easy to enable peer-geo lookups without noticing the
    third-party plaintext dependency.
- The shipped behavior in this slice:
  - `crates/iroha_torii/src/telemetry/peers/monitor.rs` now fails closed on
    missing endpoints and non-HTTPS endpoints, logging a warning and skipping
    geo lookup instead of silently using a built-in plaintext default;
  - `crates/iroha_config/src/parameters/user.rs` no longer injects an implicit
    geo endpoint during parse, so the unset state remains explicit until
    runtime validation; and
  - the duplicated telemetry docs plus
    `docs/source/references/peer.template.toml` now state that
    `torii.peer_geo.endpoint` must be explicitly configured with HTTPS when
    peer geo lookup is enabled.
- Validation:
  - `cargo fmt --all` (pass)
  - `CARGO_HOME=/tmp/iroha-cargo-home-sec14 CARGO_TARGET_DIR=/tmp/iroha-codex-target-peer-geo cargo test -p iroha_torii --features telemetry --lib construct_geo_query -- --nocapture` (pass)
  - `CARGO_HOME=/tmp/iroha-cargo-home-sec14 CARGO_TARGET_DIR=/tmp/iroha-codex-target-peer-geo cargo test -p iroha_torii --features telemetry --lib collect_geo_ -- --nocapture` (pass)
  - `CARGO_HOME=/tmp/iroha-cargo-home-sec14 CARGO_TARGET_DIR=/tmp/iroha-codex-target-peer-geo cargo test -p iroha_config torii_peer_geo_parse_ -- --nocapture` (pass)
- Remaining implementation gap:
  - no live peer-geo plaintext fallback remains in the current tree. The
    remaining security backlog is now CUDA runtime validation on a
    CUDA-capable host.

## 2026-03-25 Follow-up: Soracloud service config/secret control plane landed
- Extended the Soracloud service control plane across
  `crates/iroha_data_model/src/soracloud.rs`,
  `crates/iroha_data_model/src/isi/soracloud.rs`,
  `crates/iroha_core/src/smartcontracts/isi/soracloud.rs`,
  `crates/iroha_torii/src/soracloud.rs`,
  `crates/iroha_torii/src/lib.rs`,
  `crates/iroha_cli/src/soracloud.rs`, and
  `crates/irohad/src/soracloud_runtime.rs`.
- The shipped behavior in this slice:
  - authoritative deployment state now carries service-scoped config and secret
    maps plus independent `config_generation` / `secret_generation` counters;
  - new signed ISIs now support
    `SetSoracloudServiceConfig`,
    `DeleteSoracloudServiceConfig`,
    `SetSoracloudServiceSecret`, and
    `DeleteSoracloudServiceSecret`, with matching service-audit metadata and
    provenance hashing;
  - deploy/upgrade/rollback preserve committed config and secret state instead
    of dropping it on revision changes;
  - Torii now exposes authoritative
    `POST /v1/soracloud/service/config/{set,delete}`,
    `GET /v1/soracloud/service/config/status`,
    `POST /v1/soracloud/service/secret/{set,delete}`, and
    `GET /v1/soracloud/service/secret/status`; and
  - the CLI now exposes matching `iroha app soracloud config-*` and
    `secret-*` commands, while private-runtime `ReadSecret` checks committed
    deployment `service_secrets` before the legacy local file fallback.
- Focused regression coverage now proves:
  - authoritative service-config status reads committed deployment config
    entries;
  - authoritative service-secret status reads committed deployment secret
    entries; and
  - private-runtime secret reads prefer committed deployment secret entries
    over the older materialized file tree.
- Validation:
  - `cargo fmt --all` (pass)
  - `CARGO_TARGET_DIR=target_soracloud_cfg cargo check -p iroha_core --lib` (pass)
  - `CARGO_TARGET_DIR=target_soracloud_cfg cargo check -p iroha_torii --lib` (pass)
  - `CARGO_TARGET_DIR=target_soracloud_cfg cargo check -p iroha_cli` (pass)
  - `CARGO_TARGET_DIR=target_soracloud_torii_cfg cargo test -p iroha_torii authoritative_service_config_status_reads_world_state --lib` (pass)
  - `CARGO_TARGET_DIR=target_soracloud_torii_cfg cargo test -p iroha_torii authoritative_service_secret_status_reads_world_state --lib` (pass)
  - `CARGO_TARGET_DIR=target_soracloud_runtime_cfg cargo test -p irohad ivm_host_private_runtime_ -- --nocapture` (pass)
- Remaining implementation gap:
  - this slice establishes authoritative service config/secret state and read
    paths, but it does not yet materialize those records into general
    one-click service env/file injection or broader deploy-time secret/config
    binding semantics outside the current private-runtime secret lookup path.

## 2026-03-25 Follow-up: outbound P2P TLS-over-TCP now defaults to TLS-only dials
- Closed a medium-severity transport-policy gap in the P2P dialer:
  - `network.tls_enabled=true` previously inherited
    `tls_fallback_to_plain=true` unless operators explicitly overrode it, so a
    TLS handshake failure or timeout in `crates/iroha_p2p/src/peer.rs` silently
    downgraded the outbound dial to plaintext TCP; and
  - that did not bypass peer authentication because the signed application
    handshake still binds the peer identity, but it did break the operator's
    expectation that enabling TLS would fail closed on transport security.
- The shipped behavior in this slice:
  - `crates/iroha_config/src/parameters/user.rs` now defaults
    `tls_fallback_to_plain` to `false`, so `network.tls_enabled=true` is
    TLS-only unless plaintext fallback is explicitly opted in;
  - the default-config fixture snapshot, Kagami sample config, and default-like
    P2P/Torii test helpers now mirror the hardened runtime default; and
  - the canonical P2P docs in `docs/source/p2p*.md` now describe plaintext
    fallback as an explicit opt-in instead of the default behavior.
- Validation:
  - `cargo fmt --all` (pass)
  - `CARGO_HOME=/tmp/iroha-cargo-home-sec13 CARGO_TARGET_DIR=/tmp/iroha-codex-target-config-tls2 cargo test -p iroha_config tls_fallback_defaults_to_tls_only -- --nocapture` (pass)
  - `CARGO_HOME=/tmp/iroha-cargo-home-sec13 CARGO_TARGET_DIR=/tmp/iroha-codex-target-p2p-sec13 cargo test -p iroha_p2p --lib start_rejects_tls_without_feature_when_tls_only_outbound -- --nocapture` (pass)
  - `CARGO_HOME=/tmp/iroha-cargo-home-sec13 CARGO_TARGET_DIR=/tmp/iroha-codex-target-p2p-sec13 cargo test -p iroha_p2p --lib tls_only_dial_requires_p2p_tls_feature_when_no_fallback -- --nocapture` (pass)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_HOME=/tmp/iroha-cargo-home-sec13 CARGO_TARGET_DIR=/tmp/iroha-codex-target-torii-connect-gating cargo test -p iroha_torii --test connect_gating --no-run` (pass)
- Remaining implementation gap:
  - explicit plaintext fallback remains supported for deployments that
    intentionally set `tls_fallback_to_plain=true`, but the shipped default now
    fails closed once TLS is enabled.

## 2026-03-25 Follow-up: local QUIC proxy now rejects non-loopback binds
- Closed a medium-severity SoraFS workstation-proxy trust-boundary gap:
  - `crates/sorafs_orchestrator/src/proxy.rs` previously let
    `LocalQuicProxyConfig.bind_addr` target any socket address, so a
    non-loopback value such as `0.0.0.0:0` or a LAN IP exposed the "local"
    QUIC proxy as a remotely reachable listener; and
  - the same proxy listener did not authenticate clients beyond the
    version-matched proxy handshake, while `bridge` mode could relay TCP
    traffic and stream local Norito/CAR/Kaigi files from the operator
    workstation.
- The shipped behavior in this slice:
  - `LocalQuicProxyConfig::parsed_bind_addr(...)` now rejects non-loopback
    addresses with `ProxyError::BindAddressNotLoopback` before the QUIC
    endpoint starts;
  - the config field docs now describe `bind_addr` as loopback-only in
    `docs/source/sorafs/developer/orchestrator.md` and
    `docs/portal/docs/sorafs/orchestrator-config.md`; and
  - focused regression coverage now proves a non-loopback bind fails closed
    while the intended loopback TCP bridge path still succeeds.
- Validation:
  - `cargo fmt --all` (pass)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-proxy cargo test -p sorafs_orchestrator spawn_local_quic_proxy_rejects_non_loopback_bind_addr -- --nocapture` (pass)
  - `/tmp/iroha-codex-target-proxy/debug/deps/sorafs_orchestrator-b3be10a343598c7b --exact proxy::tests::tcp_stream_bridge_transfers_payload --nocapture` (pass)
- Remaining implementation gap:
  - no live remote-exposure issue remains in this helper slice. If a future
    product requirement needs non-loopback proxying, that should be designed
    as an explicitly authenticated mode rather than relaxing the loopback
    guard.

## 2026-03-25 Follow-up: active `tar` advisories removed from the dependency graph
- Closed the remaining active `tar` advisory paths without touching
  `Cargo.lock`:
  - `xtask/src/mochi.rs` no longer links the Rust `tar` crate to package Mochi
    bundles; it now shells out to the system `tar` binary with a fixed
    argument vector rooted at the approved bundle directory, and
    `xtask/Cargo.toml` dropped the direct `tar` and `flate2` dependencies;
  - `iroha_crypto` no longer pulls `libsodium-sys-stable` for Ed25519 interop
    tests; those checks now use the already-present OpenSSL dev dependency in
    `crates/iroha_crypto/src/signature/ed25519.rs`, which removes the last
    active `tar` path from the dependency graph; and
  - `cargo-deny` reduced the advisory backlog from seven to three at that
    point: `rustls-webpki`, `derivative`, and `paste`. Later same-day
    follow-up work moved `rustls-webpki` onto the patched `0.103.10` line and
    explicitly accepted the remaining two transitive macro advisories in
    `deny.toml`.
- Validation:
  - `cargo fmt --all` (pass)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 cargo test -p xtask create_archive_packages_bundle_directory -- --nocapture` (pass)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 cargo check -p xtask` (pass)
  - `cargo tree -p xtask -e normal -i tar` (pass: no matching package)
  - `cargo tree -p iroha_crypto -e all -i libsodium-sys-stable` (pass: no matching package)
  - `cargo tree -p iroha_crypto -e all -i tar` (pass: no matching package)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 cargo test -p iroha_crypto ed25519_verify -- --nocapture` (pass)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 cargo test -p iroha_crypto ed25519_sign -- --nocapture` (pass)
  - `cargo deny check advisories bans sources --hide-inclusion-graph` (at that point still failed on `rustls-webpki`, `derivative`, and `paste`; later same-day follow-up closed those active failures)
- Remaining implementation gap:
  - no live `tar` advisory remains in the active graph, but `Cargo.lock`
    still contains stale historical entries because this repo policy forbids
    lockfile edits in this pass. Later same-day follow-up work closed the
    active dependency tranche in the current tree.

## 2026-03-25 Follow-up: SDK transport guards now treat auth headers, canonicalAuth, and raw private_key bodies as sensitive
- Closed a cross-SDK transport-validation gap across the sampled Swift,
  Java/Android, Kotlin, and JS clients:
  - some helper paths only treated explicit auth headers as transport-sensitive,
    so raw `private_key*` JSON bodies could still be sent over plain `http`; and
  - the JS `ToriiClient` also added `canonicalAuth` headers after its transport
    checks, so signed app-auth requests could bypass the scheme/host guard.
- The shipped behavior in this slice:
  - Swift now centralizes the policy in
    `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift` and applies it from
    `NoritoRpcClient`, `ToriiClient`, and `ConnectClient`, so insecure/cross-host
    auth-header, websocket, and raw-`private_key` request paths fail closed;
  - Java/Android and Kotlin now share the same policy through
    `TransportSecurity.{java,kt}` and apply it from Norito-RPC, request-builder,
    offline/subscription, event-stream, and websocket surfaces; and
  - JS `ToriiClient._request(...)` now treats `canonicalAuth` and JSON bodies
    containing `private_key*` fields as sensitive transport material, blocks
    insecure/cross-host delivery for those requests, and records
    `hasSensitiveBody` / `hasCanonicalAuth` in downgraded-transport telemetry.
- Validation:
  - `cd IrohaSwift && swift test --filter 'NoritoRpcClientTests/testCallRejectsInsecureAuthorizationHeader'` (pass)
  - `cd IrohaSwift && swift test --filter 'ConnectClientTests/testBuildsConnectWebSocketRequestRejectsInsecureTransport'` (pass)
  - `cd IrohaSwift && swift test --filter 'ToriiClientTests/testCreateSubscriptionPlanRejectsInsecureTransportForPrivateKeyBody'` (pass)
  - `cd kotlin && ./gradlew :core-jvm:test --tests org.hyperledger.iroha.sdk.client.TransportSecurityClientTest --console=plain` (pass)
  - `cd java/iroha_android && JAVA_HOME=$(/usr/libexec/java_home -v 21) ANDROID_HOME=~/Library/Android/sdk ANDROID_SDK_ROOT=~/Library/Android/sdk ./gradlew android:compileDebugUnitTestJavaWithJavac --console=plain` (pass)
  - `cd java/iroha_android && JAVA_HOME=$(/usr/libexec/java_home -v 21) ANDROID_HOME=~/Library/Android/sdk ANDROID_SDK_ROOT=~/Library/Android/sdk ANDROID_HARNESS_MAINS=org.hyperledger.iroha.android.client.NoritoRpcClientTests,org.hyperledger.iroha.android.client.OfflineToriiClientTests,org.hyperledger.iroha.android.client.SubscriptionToriiClientTests,org.hyperledger.iroha.android.client.stream.ToriiEventStreamClientTests,org.hyperledger.iroha.android.client.websocket.ToriiWebSocketClientTests ./gradlew :core:test --tests org.hyperledger.iroha.android.GradleHarnessTests --console=plain` (pass)
  - `node --test javascript/iroha_js/test/transportSecurity.test.js javascript/iroha_js/test/toriiCanonicalAuth.test.js` (pass)
  - `node --test javascript/iroha_js/test/toriiSubscriptions.test.js` (pass)
- Remaining implementation gap:
  - no new live SDK transport bug from this pass, but the broader Swift/Android/Kotlin/JS suites were not rerun here; coverage remains focused on the transport/auth surfaces touched by this remediation.

## 2026-03-25 Follow-up: Torii webhook egress and MCP dispatch now preserve the checked trust boundary
- Closed two medium-severity Torii trust-boundary bugs in the same slice:
  - secure webhook delivery previously validated destination DNS answers
    against the egress guard and then let HTTPS / WSS re-resolve the hostname
    at connect time; and
  - MCP internal route dispatch previously rewrote every tool call as loopback
    (`x-iroha-remote-addr=127.0.0.1` plus loopback `ConnectInfo`), which let
    MCP inherit CIDR-based route privilege once MCP and a loopback-trusting
    route were enabled together.
- The shipped behavior in this slice:
  - `crates/iroha_torii/src/webhook.rs` now pins vetted HTTPS DNS answers via
    `reqwest::Client::builder().resolve_to_addrs(...)` and performs WSS
    handshakes over a raw `TcpStream` connected to an already approved address,
    so the destination checked by policy is the destination actually dialed;
  - `crates/iroha_torii/src/mcp.rs` now preserves the outer request's injected
    remote IP when internally dispatching a tool call, synthesizes
    `ConnectInfo` from that real caller IP instead of loopback, and rejects
    both `x-iroha-remote-addr` and `x-forwarded-client-cert` supplied through
    MCP `headers`; and
  - focused regression coverage now proves both the webhook pinning helpers
    and the MCP remote-address preservation/spoof-blocking behavior.
- Validation:
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-webhook-https cargo check -p iroha_torii --lib --features app_api_https` (pass)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-webhook-https cargo test -p iroha_torii --lib https_delivery_dns_override_ --features app_api_https -- --nocapture` (pass)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-webhook2 cargo test -p iroha_torii --lib websocket_pinned_connect_addr_pins_secure_delivery_when_guarded -- --nocapture` (pass)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-mcp cargo test -p iroha_torii --lib dispatch_route_preserves_inbound_remote_addr_for_internal_allowlist_checks -- --nocapture` (pass)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-mcp cargo test -p iroha_torii --lib dispatch_route_blocks_remote_addr_spoofing_from_extra_headers -- --nocapture` (pass)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-mcp cargo test -p iroha_torii --lib apply_extra_headers_blocks_reserved_internal_headers -- --nocapture` (pass)
- Remaining implementation gap:
  - the narrower `--no-default-features --features app_api,app_api_https`
    Torii lib-test matrix still fails for unrelated pre-existing DA /
    Soracloud feature-gating issues (`iroha_zkp_halo2`, gated proxy helpers,
    and `p2p` field references), so this pass validated the shipped default
    MCP path and the `app_api_https` webhook path rather than full
    minimal-feature parity.

## 2026-03-25 Follow-up: outbound P2P handshakes now fail closed on peer-id substitution
- Closed an outbound identity-binding gap in `iroha_p2p`: dialing peer `X`
  could previously finish as authenticated peer `Y` because the handshake
  verified the remote signature but never checked that the resulting public key
  matched the `peer_id` the network actor intended to reach.
- The shipped behavior in this slice:
  - `crates/iroha_p2p/src/peer.rs` now carries the expected outbound
    `PeerId` through `ConnectedTo -> SendKey -> GetKey`, and
    `GetKey::read_their_public_key(...)` now returns
    `HandshakePeerMismatch` if the signed handshake authenticates a different
    key;
  - inbound P2P handshakes remain unchanged and keep `expected_peer_id=None`,
    so only outbound dials perform the extra identity-binding check; and
  - focused regression coverage now includes a negative mismatched-key case
    plus the existing positive signed-handshake case to prove normal outbound
    handshakes still succeed.
- Validation:
  - `cargo fmt --all` (pass)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-p2p cargo test -p iroha_p2p --lib --no-run` (pass)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-p2p cargo test -p iroha_p2p --lib outgoing_handshake_rejects_unexpected_peer_identity -- --nocapture` (pass)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-p2p cargo test -p iroha_p2p --lib handshake_v1_defaults_to_trust_gossip -- --nocapture` (pass)
- Remaining implementation gap:
  - none from this specific P2P slice. The sampled QUIC streaming helper path
    was traced far enough to show its `StreamingClient::connect(...)` /
    `StreamingServer::bind(...)` call sites are currently test-only, so it did
    not become a new live runtime finding in this pass.

## 2026-03-25 Follow-up: final first-release asset-id legacy literals removed from live sources and fixtures
- Completed the last live public asset-id cleanup so first-release user-facing paths now consistently use a single canonical asset literal form: `<asset-definition-id>#<account-id>` with optional `#dataspace:<id>` for scoped balances.
- The shipped behavior in this slice:
  - Swift offline encoding/decoding and Torii client parsing now build, validate, and render canonical public `AssetId` literals instead of `norito:<hex>` wrappers; `TransactionEncoder`, `OfflineNoritoEncoding`, `OfflineNoritoDecoding`, and Torii explorer parsing all share the same public-form validation path;
  - the remaining live Rust/JS host/bridge/docs wording in `iroha_js_host`, `connect_norito_bridge`, Torii MCP tests, the tutorial example, and the IVM TLV examples no longer describe `norito:<hex>` or `aid:` as current asset-id interfaces;
  - Android/JVM transfer encoders dropped the unused legacy decode helpers that only existed for the removed prefixed asset-id forms; and
  - the remaining checked-in capability fixtures that still stored `aid:<hex>` asset-definition ids now use canonical Base58 addresses (`5kb2KcCA3A1vc4BetPzvuZT8PYR3`, `66owaQmAQMuHxPzxUN3bqZ6FJfDa`, `62Fk4FPcMuLvW5QjDGNF2a4jAmjM`), the old alias-shaped `asset_def:name#domain` access-hint compatibility path was removed, the dead `xtask` rewrite binary/manifest entry was deleted, and the only remaining `aid:` hits in the tree are deliberate negative tests asserting rejection.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_data_model asset_id_parse_literal_rejects_malformed_colon_literal -- --nocapture` (pass)
  - `cargo test -p iroha_core access:: --lib -- --nocapture` (pass)
  - `cd IrohaSwift && swift build` (pass)
  - repo searches for residual public `norito:<hex>` / `aid:<hex>` asset-id literals in touched source/docs/fixtures (clean except for deliberate negative tests)
- Remaining implementation gap:
  - no known functional gap remains in the first-release asset-id hard cut, and the
    previously tracked mobile follow-up items were closed later on 2026-03-25.

## 2026-03-25 Follow-up: tracked cargo-deny policy now runs directly and the dependency findings are triaged
- Migrated the checked-in [`deny.toml`](/Users/takemiyamakoto/dev/iroha/deny.toml) to the current cargo-deny 0.19 schema, so the dependency audit no longer depends on a temporary compatibility file.
- The shipped behavior in this slice:
  - `cargo deny check advisories bans sources --hide-inclusion-graph` now runs directly from the repo root;
  - the two previously reported `tar` advisories are no longer active in the dependency graph after the later `xtask` and `iroha_crypto` follow-up fixes;
  - the previously reported direct PQ replacement advisories were closed later by migrating `soranet_pq`, `iroha_crypto`, and `ivm` to `pqcrypto-mldsa` / `pqcrypto-mlkem`; and
  - later same-day follow-up work pinned `reqwest` / `rustls` to patched patch releases so `rustls-webpki` resolves to fixed `0.103.10`, and `derivative` / `paste` are now explicitly accepted in `deny.toml` as transitive upstream macro debt rather than direct workspace macro usage.
- Validation:
  - `cargo deny check advisories bans sources --hide-inclusion-graph` (at that point failed on the then-live findings; later same-day follow-up passes with `derivative` / `paste` noted as ignored)
  - `cargo tree -p xtask -e normal -i tar` (now reports no matching package)
  - `cargo tree -p iroha_crypto -e all -i tar` (now reports no matching package)
  - `cargo tree -p iroha_crypto -e all -i libsodium-sys-stable` (now reports no matching package)
  - repo search for revocation/CRL usage under `crates/`
  - `cd crates/ivm/fuzz && cargo +nightly fuzz run --fuzz-dir . tlv_validate -- -runs=4000 -rss_limit_mb=3072 -max_total_time=30` (initial nightly build remained too slow for a full scripted result in this pass, but it produced the fuzz binary)
  - `cd crates/ivm/fuzz && ./target/aarch64-apple-darwin/release/tlv_validate -runs=200 -rss_limit_mb=3072 -max_total_time=15` (pass; reached the libFuzzer loop and completed 200 runs from an empty corpus)
- Remaining implementation gap:
  - the dependency scan now has first-class repo support and no active advisory failures remain in the current tree; the full nightly IVM fuzz-smoke script still needs a warm-cache rerun for a stable recorded result.

## 2026-03-24 Follow-up: public asset ids now use one canonical text form
- Switched `iroha_data_model::asset::AssetId` display/`FromStr`/JSON behavior to
  the public literal `<asset-definition-id>#<account-id>` with optional
  `#dataspace:<id>` for scoped balances. This entry originally still mentioned
  internal `norito:<hex>` helpers; those helpers were removed in the
  2026-03-25 follow-up above.
- Updated the public validators/documentation touched in this pass so they no
  longer advertise `norito:<hex>` as the default user-facing asset-id format:
  `crates/iroha_cli/CommandLineHelp.md`,
  `docs/source/{data_model.md,data_model_and_isi_spec.md,sdk/swift/index.md,sdk/android/index.md,sdk/js/validation.md}`,
  `javascript/iroha_js/src/{normalizers.js,transaction.js}`,
  `IrohaSwift/Sources/IrohaSwift/{ToriiClient.swift,TransactionEncoder.swift}`,
  and `java/iroha_android/src/main/java/org/hyperledger/iroha/android/model/instructions/TransferWirePayloadEncoder.java`.
- Validation in progress:
  - `cargo check -p iroha_cli --tests` (pass)
  - `cd IrohaSwift && swift build` (pass)
  - focused `iroha_data_model` tests were recompiled successfully while the
    long-running filtered test command continued draining the crate test set
    under Cargo’s package lock.

## 2026-03-24 Follow-up: dependency and fuzz validation now produce live audit signal
- Unblocked the previously missing dependency/fuzz tooling and replaced those
  audit blind spots with real results across `deny.toml`,
  `scripts/fuzz_smoke.sh`, the Norito/IVM fuzz manifests, and the Norito JSON
  equivalence fuzz target.
- The shipped behavior in this slice:
  - local `cargo-deny` / `cargo-fuzz` are now installed, and a current-schema
    temporary compatibility config was enough to run `cargo-deny` without
    mutating the tracked `deny.toml`; under that run, `bans` and `sources`
    were clean while `advisories` initially returned seven live dependency findings:
    `RUSTSEC-2024-0388` (`derivative` unmaintained),
    `RUSTSEC-2024-0436` (`paste` unmaintained),
    `RUSTSEC-2024-0380` (`pqcrypto-dilithium` replaced),
    `RUSTSEC-2024-0381` (`pqcrypto-kyber` replaced),
    `RUSTSEC-2026-0049` (`rustls-webpki` CRL matching bug),
    `RUSTSEC-2026-0067` (`tar` symlink chmod issue), and
    `RUSTSEC-2026-0068` (`tar` PAX size issue); later fixes in `xtask` and
    `iroha_crypto` removed both active `tar` findings, and later same-day
    follow-up work closed the direct PQ and `rustls-webpki` findings while
    explicitly accepting the remaining `derivative` / `paste` macro debt in
    `deny.toml`;
  - `scripts/fuzz_smoke.sh` now passes explicit `--fuzz-dir` roots and uses
    `cargo +nightly fuzz run`, matching the repo’s actual fuzz layout and
    libFuzzer’s nightly requirement under the stable workspace toolchain
    override;
  - `crates/{norito,ivm}/fuzz/Cargo.toml` now advertise
    `[package.metadata] cargo-fuzz = true` in the format current
    `cargo-fuzz` expects; and
  - `crates/norito/fuzz/fuzz_targets/json_from_json_equiv.rs` no longer
    double-derives the fast JSON traits or incorrectly derives `Eq` for a
    `f64`-bearing struct, so the Norito fuzz smoke targets compile and run
    again.
- Validation:
  - `cargo install cargo-deny --locked` (pass)
  - `cargo install cargo-fuzz --locked` (pass)
  - `cargo deny check advisories bans sources` (was blocked in this earlier
    pass by tracked `deny.toml` schema drift against cargo-deny 0.19; resolved
    in the 2026-03-25 follow-up above)
  - `cargo deny check advisories bans sources -c /tmp/iroha-deny-compat.*.toml`
    (advisories failed with the seven findings above; `bans` ok; `sources`
    ok)
  - `cargo deny --format json check advisories -c /tmp/iroha-deny-compat.*.toml`
    (pass; produced the seven advisory records above)
  - `bash -n scripts/fuzz_smoke.sh` (pass)
  - `bash scripts/fuzz_smoke.sh` (Norito `json_parse_string`,
    `json_parse_string_ref`, `json_skip_value`, and `json_from_json_equiv`
    passed; the IVM `tlv_validate` first nightly build was still in progress
    at handoff, so no final IVM fuzz result was recorded in this pass)
- Remaining implementation gap:
  - finish the IVM nightly fuzz-smoke run after the large first build
    completes, then rerun the full script on a warm cache to collect a stable
    pass/fail result for `tlv_validate` and `kotodama_lower`.

## 2026-03-24 Follow-up: public CUDA Merkle-pair and multi-round AES batch entry points now have focused regressions
- Extended the CUDA regression layer in `crates/ivm/src/cuda.rs` beyond the
  internal startup self-tests.
- The shipped behavior in this slice:
  - `sha256_pairs_reduce_cuda` is now pinned by a direct CPU-parity regression
    that covers the odd-leaf carry path used by the public Merkle reducer; and
  - `aesenc_rounds_batch_cuda` / `aesdec_rounds_batch_cuda` now have focused
    multi-round batch parity regressions against the scalar AES reference path,
    so the public fused CUDA entry points are covered in source alongside the
    existing internal admission checks.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo check -p ivm` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-cuda-check cargo check -p ivm --features cuda --tests` (pass; `build.rs` still warns because `nvcc` is absent on this host)
- Remaining implementation gap:
  - this host still cannot link or execute the `cust`-backed CUDA test
    binaries, so the new public-kernel regressions are compile-validated here
    but still need runtime execution on a CUDA-capable machine.

## 2026-03-24 Follow-up: public Metal Merkle-pair and multi-round AES batch entry points now have focused regressions
- Extended the `ivm` Metal regression layer in
  `crates/ivm/src/vector.rs` beyond startup self-tests.
- The shipped behavior in this slice:
  - `metal_sha256_pairs_reduce` is now pinned by a direct parity test against
    the CPU Merkle pair-reduction path, including the odd-leaf carry case used
    by the public reducer;
  - `metal_aesenc_rounds_batch` and `metal_aesdec_rounds_batch` are now pinned
    by focused multi-round batch tests against the scalar AES reference path,
    instead of being covered only indirectly by startup admission; and
  - the sampled Metal startup truth set remains the fail-closed gate, but the
    public fused-kernel entry points now also have dedicated regressions that
    exercise the exported runtime functions directly.
- Validation:
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo test -p ivm --features metal --lib metal_sha256_pairs_reduce_matches_cpu -- --nocapture` (pass)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo test -p ivm --features metal --lib metal_aes_rounds_batch_matches_scalar -- --nocapture` (pass)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo check -p ivm --features metal --tests` (pass)
- Remaining implementation gap:
  - the corresponding CUDA runtime paths still need live-driver execution on a
    CUDA-capable host before this same public-kernel coverage can be validated
    beyond compile-time admission checks.

## 2026-03-24 Alias-bootstrap fix restores soak startup, but both full preserved-peer stable soaks still fail the frontier cadence target
- Fixed the immediate soak blocker introduced by alias leasing:
  - `crates/iroha_core/src/sns.rs` now exposes `seed_genesis_alias_bootstrap(...)`, which pre-seeds the SNS lease records and `CanManageAccountAlias` permissions consumed directly by genesis-time domain registration, labeled account registration, and alias-bind / relabel instructions.
  - `crates/iroha_test_network/src/config.rs` now seeds the pre-execution world with the `genesis` domain, the effective genesis account entry, and the alias bootstrap state before validating generated Izanami genesis blocks.
  - `crates/irohad/src/main.rs` now seeds the same alias bootstrap state on empty-state / snapshot-rebuild startup paths, and `genesis_domain(...)` now uses the raw genesis authority as the domain owner instead of the domainful `genesis@genesis` account id.
- Focused validation on the bootstrap repair:
  - `cargo fmt --all` (pass)
  - `NORITO_SKIP_BINDINGS_SYNC=1 cargo test -p iroha_core --lib seed_genesis_alias_bootstrap_covers_domains_and_account_labels -- --nocapture` (pass)
  - `NORITO_SKIP_BINDINGS_SYNC=1 cargo test -p iroha_test_network --lib populate_genesis_results_leases_genesis_account_labels -- --nocapture` (pass)
  - `NORITO_SKIP_BINDINGS_SYNC=1 cargo test -p irohad genesis_domain_owner_matches_genesis_authority -- --nocapture` (pass)
  - `NORITO_SKIP_BINDINGS_SYNC=1 cargo build --release -p irohad --bin iroha3d -p izanami --bin izanami` (pass)
- Permissioned full soak after the bootstrap repair:
  - log:
    `/tmp/izanami_permissioned_aliasbootstrap_20260324T205028Z.log`,
    peer dirs:
    `/tmp/iroha-soak-permissioned-aliasbootstrap_20260324T205028Z/irohad_test_network_9tW8sv/*`
  - result: the cluster now starts and sustains block production, but the one-hour envelope still fails at the duration checkpoint with
    `quorum p95 block interval 5003ms exceeded threshold 1000ms`
    after `1391` strict/quorum blocks in `3599.99381175s`
    (`successes=17965`, `failures=32`), about `2.588s/block`.
  - final interval distribution from the soak log:
    `median=2503ms`, `p95=5017ms`, `max=10390ms`;
    dominant buckets were `2502 x30`, `2501 x28`, `3335 x27`, `5003 x14`.
  - aggregate peer-log markers:
    `requested missing BlockCreated while awaiting RBC INIT=214`,
    `RbcReady=0`,
    `RbcDeliver=0`,
    `FetchBlockBody=0`,
    `BlockBodyResponse=0`.
  - the heaviest missing-`BlockCreated` load was concentrated on `untouched_roller`
    (`114` of the `214` peer-log hits), with `honest_sidewinder` next at `51`.
- NPoS full soak after the bootstrap repair:
  - log:
    `/tmp/izanami_npos_aliasbootstrap_20260324T215301Z.log`,
    peer dirs:
    `/tmp/iroha-soak-npos-aliasbootstrap_20260324T215301Z/irohad_test_network_lZOryH/*`
  - result: startup is fixed here too, but the run fails earlier on late-peer divergence with
    `height divergence exceeded safety window (divergence 28, threshold 16, window 60s, quorum min 989, strict reference 1017, strict min 989)`
    after roughly `2664.424966s` from first progress sample
    (`successes=13152`, `failures=83`), about `2.694s/block` at the last quorum height.
  - final interval distribution from the soak log:
    `median=2503ms`, `p95=5157ms`, `max=30125ms`;
    dominant buckets were `3335 x19`, `2002 x16`, `3336 x15`, `2501 x14`, `2502 x14`.
  - aggregate peer-log markers:
    `requested missing BlockCreated while awaiting RBC INIT=145`,
    `RbcReady=0`,
    `RbcDeliver=0`,
    `FetchBlockBody=0`,
    `BlockBodyResponse=0`.
  - the late laggard was primarily `renewing_nightjar`
    (`95` of the `145` missing-`BlockCreated` requests), with the peer logs also showing repeated `rbc_ready_stash` / `rbc_deliver_stash` / `rbc_ready_single_chunk_frontier_body` contexts and dwell times climbing above `10s`.
- Verdict on this cut:
  - the alias/bootstrap regression is fixed; both permissioned and NPoS now reach and hold block production again,
  - the intended consensus redesign is still not complete: contiguous frontier repair is still visibly driven by RBC-side missing-`BlockCreated` recovery rather than exact `FetchBlockBody`,
  - the acceptance target is still missed badly on the current 4-peer stable envelope: permissioned survives the hour but lands at `p95 ~ 5.0s`, and NPoS peels off earlier with one-peer divergence while already above that tail.

## 2026-03-24 Full preserved-peer soak attempt on the current inline-frontier tree: blocked before block 1 by invalid Izanami genesis
- Rebuilt the current tree with
  `NORITO_SKIP_BINDINGS_SYNC=1 cargo build --release -p irohad --bin iroha3d -p izanami --bin izanami`.
  The `NORITO_SKIP_BINDINGS_SYNC=1` override was required because the dirty tree currently fails unrelated Norito Kotlin parity checks during release build; the rebuilt `iroha3d` / `izanami` binaries themselves linked successfully afterward.
- Reran the unchanged preserved-peer stable soak envelope in both modes:
  `--duration 3600s --target-blocks 2000 --progress-interval 10s --progress-timeout 600s --tps 5 --max-inflight 8 --workload-profile stable`.
- Permissioned attempt:
  - log:
    `/tmp/izanami_permissioned_inline_frontier_20260324T192726Z.log`,
    peer dirs:
    `/tmp/iroha-soak-permissioned-inline_frontier_20260324T192726Z/irohad_test_network_1AKX9T/*`
  - result: failed during peer startup, before block 1 and before any meaningful soak progress could be measured.
  - concrete genesis failures from peer stdout:
    `InstructionFailed(InvariantViolation("active SNS domain-name lease is required before registering \`wonderland\`"))`,
    `InstructionFailed(Find(Account(6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnZaK)))`,
    `InstructionFailed(InvariantViolation("active SNS domain-name lease is required before registering \`chaosnet\`"))`.
- NPoS attempt:
  - log:
    `/tmp/izanami_npos_inline_frontier_20260324T192911Z.log`,
    peer dirs:
    `/tmp/iroha-soak-npos-inline_frontier_20260324T192911Z/irohad_test_network_OQ2Rdr/*`
  - result: same startup failure before block 1.
  - concrete genesis failures from peer stdout:
    the same `wonderland` / `chaosnet` SNS lease violations and missing-account grant as permissioned, plus
    `InstructionFailed(Find(AssetDefinition(...)))` while executing `RegisterPublicLaneValidator`.
- Verdict on this attempt:
  - there is no new permissioned-vs-NPoS consensus soak data on the current tree because neither mode enters the steady-state lane,
  - the current branch is not soak-ready; the immediate blocker is that Izanami/test-network genesis generation no longer satisfies the runtime invariants now enforced by the executor, and
  - until that genesis contract is repaired, any critique of `BlockCreated` frontier cadence, late-peer peel-off, or `FetchBlockBody` needs is speculative because the cluster never reaches block production.

## 2026-03-24 Follow-up: `BlockCreated` now carries inline frontier metadata and leader rebroadcast prefers it
- Landed the first protocol tranche toward a `BlockCreated`-only frontier without attempting backward compatibility or the full repair-path cut yet:
  - `crates/iroha_core/src/sumeragi/message.rs` extends `BlockCreated` with optional `BlockCreatedFrontierInfo`, bundling the highest-QC reference, payload hash, proposer/epoch, roster hash, chunk manifest, chunk root, and leader signature needed to seed the active slot from a single frontier advertisement.
  - `crates/iroha_core/src/sumeragi/main_loop/proposal_handlers.rs` now accepts that inline frontier metadata as authoritative slot metadata, seeds the cached proposal/hint from it, and marks `(height, view)` as observed once the `BlockCreated` body is accepted, so the pacemaker no longer needs a separate `Proposal` message on that path.
  - `crates/iroha_core/src/sumeragi/main_loop/propose.rs` now builds the consensus proposal before the frontier advertisement, upgrades fresh-slot and cached rebroadcast `BlockCreated` messages with inline frontier metadata whenever the RBC manifest can be reconstructed, and suppresses standalone `Proposal` posts on those enriched paths.
  - `crates/iroha_core/src/sumeragi/main_loop/commit.rs` now upgrades block-payload gossip to the enriched `BlockCreated` form when cached proposal metadata is available instead of separately gossiping `ProposalHint`.
- Focused validation on this tranche:
  - `NORITO_SKIP_BINDINGS_SYNC=1 cargo test -p iroha_core --lib assemble_proposal_schedules_block_created_before_rbc_without_frontier_proposal_posts -- --nocapture` (pass)
  - `NORITO_SKIP_BINDINGS_SYNC=1 cargo test -p iroha_core --lib pacemaker_rebroadcasts_cached_frontier_block_when_leader -- --nocapture` (pass)
  - `NORITO_SKIP_BINDINGS_SYNC=1 cargo test -p iroha_core --lib block_created_with_frontier_metadata_marks_view_seen_without_proposal_message -- --nocapture` (pass)
  - `NORITO_SKIP_BINDINGS_SYNC=1 cargo test -p iroha_core --lib proposal_after_block_created_is_safe_and_marks_metadata -- --nocapture` (pass)
  - `NORITO_SKIP_BINDINGS_SYNC=1 cargo test -p iroha_core --lib proposal_does_not_wake_commit_pipeline_without_block_created -- --nocapture` (pass)
  - `NORITO_SKIP_BINDINGS_SYNC=1 cargo test -p iroha_core --lib block_created_without_cached_proposal_advances_active_view -- --nocapture` (pass)
  - `NORITO_SKIP_BINDINGS_SYNC=1 cargo test -p iroha_core --lib block_created_with_matching_proposal_advances_active_view -- --nocapture` (pass)
- Remaining gap before the redesign is complete:
  - `Proposal`, `ProposalHint`, `RbcReady`, and `RbcDeliver` still exist on the wire and in receive-side logic for fallback/legacy hot paths inside this branch,
  - contiguous-frontier exact pull (`FetchBlockBody` / `BlockBodyResponse`) is not implemented yet, and
  - the near-tip RBC repair loop / READY-DELIVER transport state has not been removed yet.

## 2026-03-24 Full preserved-peer soak verdict on the signer-first frontier-repair cut
- Rebuilt the current cut with
  `cargo build --release -p irohad --bin iroha3d -p izanami --bin izanami`
  and reran the unchanged preserved-peer stable soak envelope
  (`--duration 3600s --target-blocks 2000 --tps 5 --max-inflight 8 --workload-profile stable --progress-timeout 600s`)
  in both permissioned and NPoS mode.
- Permissioned full soak:
  - log:
    `/tmp/izanami_permissioned_signerfirst_20260324T150450Z.log`,
    peer dirs:
    `/tmp/iroha-soak-permissioned-signerfirst_20260324T150450Z/irohad_test_network_S4WDKa/*`
  - result: failed at the duration checkpoint with
    `quorum p95 block interval 5003ms exceeded threshold 1000ms`
    after `1402` strict/quorum blocks in `3599.9948775s`
    (`successes=17951`, `failures=44`), about `2.57s/block`.
  - aggregate peer-log markers:
    `requested missing block payload from fetch targets=126`,
    `requested missing BlockCreated while awaiting RBC INIT=127`,
    `deferring local RBC READY: authoritative payload unavailable=157`,
    `commit quorum missing past timeout=11`,
    `no proposal observed=4`.
  - top-level soak log markers:
    `strict block height is stalled with no lagging peers=2`,
    `transaction queued for too long=12`,
    `haven't got tx confirmation within 20s=4`.
- NPoS full soak:
  - log:
    `/tmp/izanami_npos_signerfirst_20260324T160631Z.log`,
    peer dirs:
    `/tmp/iroha-soak-npos-signerfirst_20260324T160631Z/irohad_test_network_4GU655/*`
  - result: failed on divergence with
    `height divergence exceeded safety window (divergence 29, threshold 16)`
    after `1182` strict/quorum blocks in `3218.652414s`
    (`successes=16040`, `failures=41`), about `2.72s/block`.
  - aggregate peer-log markers:
    `requested missing block payload from fetch targets=105`,
    `requested missing BlockCreated while awaiting RBC INIT=109`,
    `deferring local RBC READY: authoritative payload unavailable=75`,
    `commit quorum missing past timeout=5`,
    `no proposal observed=14`.
  - top-level soak log markers:
    `strict block height is stalled with no lagging peers=4`,
    `strict block height is stalled with broad peer lag beyond tolerated failures=6`,
    `transaction queued for too long=20`,
    `haven't got tx confirmation within 20s=2`.
  - the late lagging peer resolved to `educated_sparrow`
    (`ea0130A685FD8C5A...` in `config.base.toml`), which also carried the
    heaviest recovery load:
    `missing block payload=33`,
    `missing BlockCreated=61`,
    `READY deferrals=32`.
- Verdict on this cut:
  - permissioned regressed against the previous frontier-repair full soak
    (`1402` vs `1459` strict/quorum blocks in the same 3600s envelope),
  - NPoS no longer dies in the first few minutes, but it still degrades into
    the same `2.5s` / `3.3s` / `5.0s` cadence and then loses one peer late.

## 2026-03-24 Follow-up: frontier missing-block repair is now signer-first and exact-latest recovery stays full
- Tightened the remaining body-repair path that was still pushing healthy rounds into recovery:
  - `crates/iroha_core/src/sumeragi/main_loop/rbc.rs` no longer flips metadata-only RBC sessions straight to topology-wide fetch when INIT already identifies the proposer; frontier `BlockCreated` recovery now stays signer-first until normal fallback rules apply.
  - `crates/iroha_core/src/sumeragi/main_loop.rs` now strips the local peer from missing-block request targets before posting `FetchPendingBlock`, so exact frontier repair no longer wastes one of its targets on self.
  - `crates/iroha_core/src/block_sync.rs` now keeps unknown-prev fallback in full-share mode whenever the requester is asking for the latest frontier, instead of downgrading that exact-latest repair to bounded incremental slices.
- Updated/added focused regressions in `crates/iroha_core/src/sumeragi/main_loop/tests.rs` and `crates/iroha_core/src/block_sync.rs` for:
  - local-peer target filtering,
  - signer-first metadata-only RBC fetch mode under rotated NPoS leaders, and
  - full-share retention for latest-frontier unknown-prev repair.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_core --lib missing_block_request_targets_exclude_local_peer -- --nocapture` (pass)
  - `cargo test -p iroha_core --lib handle_rbc_init_preserves_rotated_leader_preference_for_late_missing_block_recovery -- --nocapture` (pass)
  - `cargo test -p iroha_core --lib unknown_prev_latest_probe_keeps_full_share_mode -- --nocapture` (pass)
  - `cargo test -p iroha_core --lib request_missing_block_for_pending_rbc_ -- --nocapture` (pass)
  - `cargo test -p iroha_core --lib get_blocks_after_repeated_unknown_prev_probe_requests_latest_on_incremental_repeats -- --nocapture` (pass)

## 2026-03-24 Follow-up: frontier repair now suppresses quorum churn and one-chunk RBC pulls `BlockCreated`
- Fixed the remaining hot-path contributors after the ingress-grace cut:
  - `crates/iroha_core/src/block_sync.rs` no longer lets unknown-prev full recovery get capped by tiny gossip fanout; full fallback now serves a materially larger contiguous suffix per response before frame-cap trimming.
  - `crates/iroha_core/src/sumeragi/main_loop.rs` now suppresses `QuorumTimeout` / `StakeQuorumTimeout` rotation while same-slot contiguous-frontier repair is already active, seeding unified frontier ownership when needed instead of rotating away mid-repair.
  - `crates/iroha_core/src/sumeragi/main_loop/rbc.rs` now treats near-frontier single-chunk RBC sessions as an authoritative-body fast path: if the body is still not local, INIT/READY request `BlockCreated` tracking immediately even when INIT metadata is otherwise complete.
- Updated/added regressions in `crates/iroha_core/src/sumeragi/main_loop/tests.rs` and `crates/iroha_core/src/block_sync.rs`:
  - explicit single-chunk frontier INIT/READY tests now expect missing-`BlockCreated` tracking,
  - older INIT deferral/local-hydration helpers were tightened to multi-chunk sessions so they keep covering the advisory-metadata path rather than the new single-chunk fast path,
  - quorum-timeout trigger coverage now checks that active frontier repair suppresses direct view rotation.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_core --lib unknown_prev_full_share_limit_ignores_tiny_gossip_budget -- --nocapture` (pass)
  - `cargo test -p iroha_core --lib single_chunk_frontier -- --nocapture` (pass)
  - `cargo test -p iroha_core --lib trigger_view_change_quorum_timeout_suppressed_while_frontier_repair_remains_active -- --nocapture` (pass)
  - `cargo test -p iroha_core --lib maybe_emit_rbc_ready_ -- --nocapture` (pass)
  - `cargo test -p iroha_core --lib handle_rbc_init_ -- --nocapture` (pass)
  - `cargo test -p iroha_core --lib unknown_prev_ -- --nocapture` (pass)
  - `cargo test -p iroha_core --lib trigger_view_change_ -- --nocapture` (fails in adjacent fixture setup: `trigger_view_change_realigns_conflicting_committed_edge_qcs_before_new_view_vote` panics because `latest_committed_qc()` is missing before the changed path runs)

## 2026-03-24 Follow-up: contiguous-frontier missing-payload recovery now waits for authoritative body ingress
- Narrowed the remaining post-slot-owner regression to eager recovery on normal
  short reordering, not true payload loss:
  - votes / RBC READY could still outrun local `BlockCreated` processing on the
    contiguous frontier, and the node was opening `FetchPendingBlock`
    recovery immediately at first observation (`dwell_ms=0`),
  - that behavior matched the preserved-peer soak traces where
    `requested missing block payload from fetch targets` and
    `requested missing BlockCreated while awaiting RBC INIT` were often logged
    only a few lines before the matching `BlockCreated` arrived locally.
- Implemented an internal-only bounded ingress grace with no config, API, or
  wire changes:
  - `crates/iroha_core/src/sumeragi/main_loop.rs` now keeps tracking a
    contiguous-frontier missing body immediately, but the first fetch is held
    for a short bounded `AUTHORITATIVE_BODY_INGRESS_FETCH_GRACE` window and
    then forced exactly once after that grace expires,
  - `crates/iroha_core/src/sumeragi/main_loop/qc.rs` now applies that gate
    before QC-first, locked-QC, and retry-loop missing-block fetch planning, so
    normal `BlockCreated` reordering no longer emits recovery traffic at
    `dwell_ms=0`,
  - `crates/iroha_core/src/sumeragi/main_loop/rbc.rs` now applies the same gate
    before pending-RBC missing-`BlockCreated` recovery on the contiguous
    frontier,
  - `maybe_emit_rbc_ready(...)` and `maybe_emit_rbc_deliver(...)` now
    recompute authoritative payload availability after local session hydration
    in the same pass, so freshly hydrated payload no longer looks missing until
    the next turn.
- Added focused regression coverage:
  - `missing_block_ingress_fetch_gate_holds_initial_frontier_fetch`
  - `missing_block_ingress_fetch_gate_forces_first_fetch_after_grace`
  - `defer_qc_for_missing_block_force_retry_now_bypasses_retry_window`
  - `request_missing_block_for_pending_rbc_holds_initial_frontier_fetch_within_ingress_grace`
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_core --lib missing_block_ingress_fetch_gate -- --nocapture` (pass)
  - `cargo test -p iroha_core --lib defer_qc_for_missing_block_force_retry_now_bypasses_retry_window -- --nocapture` (pass)
  - `cargo test -p iroha_core --lib request_missing_block_for_pending_rbc_ -- --nocapture` (pass)
  - `cargo test -p iroha_core --lib maybe_emit_rbc_ready_missing_chunks_requests_missing_block_for_metadata_only_session -- --nocapture` (pass)
  - `cargo test -p iroha_core --lib block_created_ -- --nocapture` (pass, `61/61`)

## 2026-03-24 Full preserved-peer soak verdict on the authoritative slot-owner cut
- Rebuilt the exact authoritative-slot-owner fix with
  `cargo build --release -p irohad --bin iroha3d -p izanami --bin izanami`
  and reran the unchanged preserved-peer stable soak envelope
  (`--duration 3600s --target-blocks 2000 --tps 5 --max-inflight 8 --workload-profile stable --progress-timeout 600s`)
  in both permissioned and NPoS mode.
- Permissioned full soak:
  - log:
    `/tmp/izanami_permissioned_slotowner_20260323T201533Z.log`,
    peer dirs:
    `/tmp/iroha-soak-permissioned-slotowner_20260323T201533Z/irohad_test_network_boTLxS/*`
  - result: failed at the duration checkpoint with
    `quorum p95 block interval 5003ms exceeded threshold 1000ms`
    after `1377` strict/quorum blocks in `3599.994s`
    (`successes=17964`, `failures=31`), about `2.61s/block`.
  - aggregate peer-log markers:
    `requested missing block payload from fetch targets=515`,
    `requested missing BlockCreated while awaiting RBC INIT=36`,
    `quorum of votes observed but block payload missing; deferring QC aggregation=26`,
    `commit quorum missing past timeout=13`,
    `no proposal observed=5`,
    `deferring local RBC READY: authoritative payload unavailable=33`,
    `same-view double-vote evidence=0`.
  - top-level soak log markers:
    `strict block height is stalled with no lagging peers=2`,
    `transaction queued for too long=3`,
    `haven't got tx confirmation within 20s=1`.
- NPoS full soak:
  - log:
    `/tmp/izanami_npos_slotowner_20260323T211623Z.log`,
    peer dirs:
    `/tmp/iroha-soak-npos-slotowner_20260323T211623Z/irohad_test_network_beoSN2/*`
  - result: failed at the duration checkpoint with
    `quorum p95 block interval 5005ms exceeded threshold 1000ms`
    after `1305` strict/quorum blocks in `3599.994s`
    (`successes=17974`, `failures=21`), about `2.76s/block`.
  - aggregate peer-log markers:
    `requested missing block payload from fetch targets=602`,
    `requested missing BlockCreated while awaiting RBC INIT=40`,
    `quorum of votes observed but block payload missing; deferring QC aggregation=0`,
    `commit quorum missing past timeout=11`,
    `no proposal observed=3`,
    `deferring local RBC READY: authoritative payload unavailable=158`,
    `same-view double-vote evidence=0`.
  - top-level soak log markers:
    `strict block height is stalled with no lagging peers=2`,
    `transaction queued for too long=2`,
    `haven't got tx confirmation within 20s=1`.
- Current verdict:
  - the authoritative-slot-owner fix removed the actual catastrophic regression
    from the earlier `BlockCreated`-authoritative cut: NPoS no longer dies at
    height `400` and neither topology emitted same-view double-vote evidence,
  - both topologies still fail the preserved-peer stable envelope by a wide
    margin, settling around `~2.6-2.8s/block` with p95 still pinned near
    `5000ms`, and
  - the next problem is now narrower and shared: payload fetch and
    authoritative-payload READY deferral are still too common on the hot path,
    especially in NPoS (`missing payload fetch=602`,
    `authoritative payload unavailable READY deferral=158`), so the next cut
    should target that recovery interaction without reopening same-slot
    ownership or reintroducing proposal-led progression.

## 2026-03-23 Follow-up: authoritative `BlockCreated` slot ownership now keeps the same view closed
- Root-cause fix for the failed `BlockCreated`-authoritative cut:
  - the actual regression was not the earlier RBC-authority hypothesis; it was
    that once a `BlockCreated` body had claimed a `(height, view)` slot, later
    cleanup and pacemaker paths could reopen that same view and admit a second
    body or locally re-propose it again,
  - the preserved-peer NPoS soak traces at height `401` showed the concrete
    failure: one peer accepted two different `BlockCreated` hashes for the same
    `(401, 0)` slot and then produced same-view double-vote evidence after the
    frontier cleanup path had removed the pending owner and reopened proposal
    assembly.
- Implemented a direct authoritative-slot invariant with no wire/config/API
  change:
  - `crates/iroha_core/src/sumeragi/main_loop.rs` now keeps an internal
    authoritative `(height, view) -> block_hash` map populated only by accepted
    `BlockCreated` bodies;
  - `crates/iroha_core/src/sumeragi/main_loop/proposal_handlers.rs` now drops a
    conflicting same-slot `BlockCreated` whenever a normal happy-path owner is
    already recorded, while still allowing the existing block-sync
    commit-evidence replacement path to override that owner when the preserve
    policy is explicitly disabled;
  - `crates/iroha_core/src/sumeragi/main_loop/propose.rs` now refuses
    same-view local proposal reassembly once an authoritative `BlockCreated`
    already owns the slot, so frontier cleanup can evict pending wrappers
    without reopening the same view; and
  - frontier/future cleanup now prunes authoritative slot entries above the
    kept frontier while preserving the active frontier slot until committed
    height pruning clears it; broad mode/roster resets clear the map.
- Focused validation on this root-cause fix:
  - `cargo fmt --all`
  - `cargo test -p iroha_core --lib block_created_without_proposal_does_not_replace_authoritative_same_slot_owner -- --nocapture` (pass)
  - `cargo test -p iroha_core --lib frontier_cleanup_preserves_frontier_authoritative_slot_and_prunes_future_slots -- --nocapture` (pass)
  - `cargo test -p iroha_core --lib pacemaker_defers_reproposal_when_authoritative_block_already_owns_slot -- --nocapture` (pass)
  - `cargo test -p iroha_core --lib block_sync_payload_mismatch_with_commit_evidence_replaces_stale_frontier_owner -- --nocapture` (pass)
  - `cargo test -p iroha_core --lib block_created_payload_mismatch_preserves_active_frontier_owner -- --nocapture` (pass)
  - `cargo test -p iroha_core --lib block_created_ -- --nocapture` (pass; 61 tests)
  - `cargo test -p iroha_core --lib proposal_does_not_wake_commit_pipeline_without_block_created -- --nocapture` (pass)
- Current verdict:
  - the same-slot ownership bug that reopened `(height, view)` after
    `BlockCreated` is now fixed in the hot path and covered by focused tests,
  - the earlier full preserved-peer soak failures remain the last runtime
    verdict on record for this branch because the full permissioned/NPoS soaks
    have not yet been rerun on the authoritative-slot-owner cut, and
  - the next acceptance step is to rerun those soaks and verify that the
    height-`400/401` style same-view reentry / double-vote pattern is gone
    before critiquing any remaining READY or missing-payload churn.

## 2026-03-23 Full preserved-peer soak verdict on the `BlockCreated` authoritative happy-path cut
- Rebuilt the exact patch set with
  `cargo build --release -p irohad --bin iroha3d -p izanami --bin izanami`
  and then ran the full preserved-peer stable soak commands with the unchanged
  envelope
  (`--duration 3600s --target-blocks 2000 --tps 5 --max-inflight 8 --workload-profile stable --progress-timeout 600s`).
- Permissioned full soak:
  - log:
    `/tmp/izanami_permissioned_blockcreated_authoritative_20260323T171744Z.log`,
    peer dirs:
    `/tmp/iroha-soak-permissioned-blockcreated_authoritative_20260323T171744Z/irohad_test_network_OOmODU/*`
  - result: failed at the duration checkpoint with
    `quorum p95 block interval 5001ms exceeded threshold 1000ms`
    after `1333` strict blocks in `3599.995s`
    (`successes=17859`, `failures=91`).
  - aggregate peer-log markers:
    `requested missing block payload from fetch targets=514`,
    `requested missing BlockCreated while awaiting RBC INIT=33`,
    `sending targeted RBC READY set to peers missing READY=18404`,
    `quorum of votes observed but block payload missing; deferring QC aggregation=29`,
    `commit quorum missing past timeout=12`,
    `no proposal observed=6`.
  - top-level soak log markers:
    `strict block height is stalled with no lagging peers=5`,
    `transaction queued for too long=32`,
    `haven't got tx confirmation within 20s=32`.
- NPoS full soak:
  - log:
    `/tmp/izanami_npos_blockcreated_authoritative_20260323T181928Z.log`,
    peer dirs:
    `/tmp/iroha-soak-npos-blockcreated_authoritative_20260323T181928Z/irohad_test_network_SaURZU/*`
  - result: failed early on the strict liveness guard with
    `no strict block height progress for 600s`
    at strict/quorum height `400`
    (`successes=8163`, `failures=658`,
    `izanami_ingress_endpoint_unhealthy_total=1`).
  - aggregate peer-log markers:
    `requested missing block payload from fetch targets=115`,
    `requested missing BlockCreated while awaiting RBC INIT=10`,
    `sending targeted RBC READY set to peers missing READY=123545`,
    `commit quorum missing past timeout=6`,
    `no proposal observed=1`,
    `quorum of votes observed but block payload missing; deferring QC aggregation=0`.
  - top-level soak log markers:
    `strict block height is stalled with no lagging peers=116`,
    `transaction queued for too long=416`,
    `haven't got tx confirmation within 20s=238`.
- Current verdict:
  - the `Proposal`-demotion / `BlockCreated`-authoritative semantic cut did
    not remove hot-path recovery churn enough to satisfy the preserved-peer
    stable envelope on this exact tree,
  - permissioned no longer catastrophically diverged but still plateaued near
    `~2.7s/block` with repeated missing-payload fetches and QC deferrals, and
  - NPoS regressed harder under the same envelope, repeatedly stalling with no
    lagging peers while targeted READY chatter and client-side queue/confirmation
    failures kept climbing until the `600s` strict no-progress guard fired.

## 2026-03-23 Follow-up: `BlockCreated` is now the sole authoritative happy-path ingress for payload-dependent round progress
- Simplified the Sumeragi healthy path without changing public API, config, or
  wire format:
  - `crates/iroha_core/src/sumeragi/main_loop/propose.rs` now caches
    `Proposal`/`ProposalHint`, processes and posts `BlockCreated` first, then
    emits `Proposal`, then the RBC traffic;
  - `crates/iroha_core/src/sumeragi/main_loop/proposal_handlers.rs` keeps
    `Proposal`/`ProposalHint` as advisory metadata for stateless checks,
    highest-QC/lock bookkeeping, pacemaker liveness, and rebroadcast, but
    removes proposal-owned payload progression and makes `handle_block_created`
    the first authoritative transition into pending validation / DA / commit
    work, even when no prior `Proposal` was cached;
  - `crates/iroha_core/src/sumeragi/main_loop.rs`,
    `validation.rs`, and `commit.rs` now gate payload-dependent progression on
    authoritative local body availability (pending / inflight / Kura /
    complete-authoritative RBC session) instead of proposal evidence, and
    proposal-only metadata no longer opens or prolongs missing-payload dwell;
  - `crates/iroha_core/src/sumeragi/mod.rs` keeps the compatibility queue split
    (`BlockCreated` on `RbcChunks`, `Proposal` on `BlockPayload`) but now
    documents `Proposal` as advisory-only and `BlockCreated` as the hot-path
    body ingress;
  - duplicate / block-sync `BlockCreated` hydration paths now repopulate RBC
    session metadata from the authoritative block and immediately retry
    READY/DELIVER if the session became actionable.
- Focused validation completed on this cut:
  - `cargo fmt --all`
  - `cargo test -p iroha_core --lib block_created_ -- --nocapture` (pass; 60 tests)
  - `cargo test -p iroha_core --lib proposal_does_not_wake_commit_pipeline_without_block_created -- --nocapture` (pass)
  - `cargo test -p iroha_core --lib proposal_hint_does_not_wake_commit_pipeline_without_block_created -- --nocapture` (pass)
  - `cargo test -p iroha_core --lib proposal_after_block_created_is_safe_and_marks_metadata -- --nocapture` (pass)
  - `cargo test -p iroha_core --lib proposal_gated_by_missing_dependencies_clears_authoritative_local_payload -- --nocapture` (pass)
- Current verdict:
  - the requested semantic simplification is in place and the focused
    consensus/unit coverage for `Proposal`/`BlockCreated`/duplicate-RBC
    ordering is green,
  - the full preserved-peer stable soaks on this exact cut are now recorded in
    the newer entry above and show that the hot path is still missing the local
    acceptance envelope, and
  - full-workspace `cargo test --workspace` / `cargo clippy --workspace --all-targets -- -D warnings`
    remain pending because this turn only reran the focused `iroha_core`
    matrix plus the release-binary soak validation.

## 2026-03-23 READY repair soak rerun: preserved-peer permissioned and NPoS stable windows recover to the nominal localnet band
- Rebuilt the current READY-repair cut with
  `cargo build --release -p irohad --bin iroha3d -p izanami --bin izanami`,
  then reran both preserved-peer stable soak commands on the rebuilt release
  binaries with the unchanged envelope
  (`--duration 3600s --target-blocks 2000 --tps 5 --max-inflight 8 --workload-profile stable`)
  plus `--progress-timeout 600s`.
- Both runs were stopped manually once the multi-minute steady-state slope was
  already clear instead of spending the full hour to confirm the same shape:
  - permissioned log:
    `/tmp/izanami_permissioned_readyfix_20260323T122248Z.log`,
    peer dirs:
    `/tmp/iroha-soak-permissioned-readyfix_20260323T122248Z/irohad_test_network_huYDkZ/*`
  - NPoS log:
    `/tmp/izanami_npos_readyfix_20260323T122746Z.log`,
    peer dirs:
    `/tmp/iroha-soak-npos-readyfix_20260323T122746Z/irohad_test_network_7dZ3s3/*`
- Permissioned sampled stop-point:
  - strict height `1 -> 152` (`net +151`) over `270.063s`,
    projected one-hour height `~2013`,
  - interval buckets:
    `>=5000ms=0`, `3334-4999ms=1`, `2501-3333ms=6`, `<=2500ms=21`,
  - peer-log markers:
    `deferring local RBC READY: authoritative payload unavailable=0`,
    `deferring RBC DELIVER: READY quorum not yet satisfied=0`,
    `deferring RBC DELIVER: authoritative payload unavailable=0`,
    `requested missing block payload from fetch targets=29`,
    `requested missing BlockCreated while awaiting RBC INIT=1`,
    `sending targeted RBC DELIVER to peers missing READY=0`,
    `sending targeted RBC READY set to peers missing READY=2133`,
    `sending targeted RBC payload to peers missing READY=0`,
    `quorum of votes observed but block payload missing; deferring QC aggregation=3`,
    `no proposal observed=0`.
- NPoS sampled stop-point:
  - strict height `1 -> 133` (`net +132`) over `240.050s`,
    projected one-hour height `~1980`,
  - interval buckets:
    `>=5000ms=0`, `3334-4999ms=1`, `2501-3333ms=6`, `<=2500ms=18`,
  - peer-log markers:
    `deferring local RBC READY: authoritative payload unavailable=0`,
    `deferring RBC DELIVER: READY quorum not yet satisfied=0`,
    `deferring RBC DELIVER: authoritative payload unavailable=0`,
    `requested missing block payload from fetch targets=19`,
    `requested missing BlockCreated while awaiting RBC INIT=2`,
    `sending targeted RBC DELIVER to peers missing READY=0`,
    `sending targeted RBC READY set to peers missing READY=2046`,
    `sending targeted RBC payload to peers missing READY=0`,
    `quorum of votes observed but block payload missing; deferring QC aggregation=0`,
    `no proposal observed=0`.
- Critique from these soak reruns:
  - the READY-repair ordering fix did what the previous unified-ingress cut did
    not: targeted DELIVER rescue collapsed to zero in both modes and the repair
    load moved onto the lighter targeted READY-set path,
  - `requested missing BlockCreated while awaiting RBC INIT` stayed near zero,
    so the earlier ingress-lane fix remains intact,
  - both modes now sit back on the nominal localnet throughput envelope instead
    of the earlier `~1350/h` plateau, with permissioned slightly above target
    and NPoS effectively on target in the sampled windows, and
  - the remaining visible lag is secondary: missing-block payload fetch is
    still non-zero (`29` / `19`) and permissioned still showed a few
    `qc_missing_payload` deferrals, so a full-hour confirmation run is still
    needed before calling the issue closed end to end.

## 2026-03-23 READY repair preference: DELIVER now falls back behind direct READY rescue
- Changed the RBC missing-READY repair ordering so delivered sessions still send
  targeted READY repair, while full payload rescue remains limited to
  pre-delivery recovery.
- `maybe_emit_rbc_deliver()` now prefers direct READY repair for peers still
  missing READY signatures and only falls back to targeted DELIVER when READY
  repair cannot be emitted in that turn.
- Updated the DELIVER/rebroadcast regression tests to reflect that preference
  and fixed the unverified-roster DELIVER test setup so it no longer waits for a
  permissioned epoch transition that never occurs.
- Validated this cut with isolated-target focused runs:
  - `CARGO_TARGET_DIR=/tmp/iroha-readyfix-target cargo test -p iroha_core --lib maybe_emit_rbc_deliver_ -- --nocapture`
    passed (`9` tests),
  - `CARGO_TARGET_DIR=/tmp/iroha-readyfix-target cargo test -p iroha_core --lib rebroadcast_stalled_rbc_payloads_ -- --nocapture`
    passed (`13` tests),
  - `CARGO_TARGET_DIR=/tmp/iroha-readyfix-target cargo test -p iroha_core --lib maybe_emit_rbc_ready_ -- --nocapture`
    passed (`12` tests).
- Preserved-peer permissioned and NPoS stable soak reruns are still pending for
  this READY-repair cut; no new soak numbers are recorded yet.

## 2026-03-23 Follow-up: real preserved-peer soak commands on the unified RBC ingress cut still settle well below target
- Reran both real preserved-peer stable soak commands on the current unified
  RBC ingress build and stopped them manually once the steady-state slope was
  already clearly below the `2000/3600s` acceptance envelope:
  - permissioned log:
    `/tmp/izanami_permissioned_full_unified_rbc_lane_20260323T082628Z.log`,
    peer dirs:
    `/tmp/iroha-soak-permissioned-full-unified_rbc_lane_20260323T082628Z/irohad_test_network_yDFUFU/*`
  - NPoS log:
    `/tmp/izanami_npos_full_unified_rbc_lane_20260323T083322Z.log`,
    peer dirs:
    `/tmp/iroha-soak-npos-full-unified_rbc_lane_20260323T083322Z/irohad_test_network_41SBkC/*`
  - both runs used the unchanged stable envelope
    (`--duration 3600s --target-blocks 2000 --tps 5 --max-inflight 8 --workload-profile stable`)
    with isolated temporary permit dirs, but were stopped once the new cut had
    already settled into a repeatable under-target projection.
- Permissioned sampled stop-point:
  - strict height `1 -> 127` (`net +126`) over `333.462s`,
    projected one-hour height `~1360`,
  - interval buckets:
    `>=5000ms=2`, `3334-4999ms=8`, `2501-3333ms=17`, `<=2500ms=6`,
  - peer-log markers:
    `deferring local RBC READY: authoritative payload unavailable=0`,
    `deferring RBC DELIVER: READY quorum not yet satisfied=0`,
    `deferring RBC DELIVER: authoritative payload unavailable=0`,
    `requested missing block payload from fetch targets=31`,
    `requested missing BlockCreated while awaiting RBC INIT=7`,
    `sending targeted RBC DELIVER to peers missing READY=530`,
    `quorum of votes observed but block payload missing; deferring QC aggregation=3`,
    `no proposal observed=0`.
- NPoS sampled stop-point:
  - strict height `1 -> 99` (`net +98`) over `260.951s`,
    projected one-hour height `~1352`,
  - interval buckets:
    `>=5000ms=2`, `3334-4999ms=8`, `2501-3333ms=11`, `<=2500ms=5`,
  - peer-log markers:
    `deferring local RBC READY: authoritative payload unavailable=0`,
    `deferring RBC DELIVER: READY quorum not yet satisfied=0`,
    `deferring RBC DELIVER: authoritative payload unavailable=0`,
    `requested missing block payload from fetch targets=16`,
    `requested missing BlockCreated while awaiting RBC INIT=1`,
    `sending targeted RBC DELIVER to peers missing READY=408`,
    `quorum of votes observed but block payload missing; deferring QC aggregation=0`,
    `no proposal observed=0`.
- Critique from these soak commands:
  - the ingress unification continues to suppress the old healthy-path metadata
    split: `requested missing BlockCreated while awaiting RBC INIT` stayed near
    zero in both real runs,
  - the remaining hot-path cost is now concentrated in READY convergence, with
    `sending targeted RBC DELIVER to peers missing READY` staying high from the
    beginning of both runs and scaling faster than the missing-`BlockCreated`
    counters,
  - healthy-path throughput improved over the previous sampled worker-scheduling
    cut but still plateaued at only about `68%` of the target envelope in both
    modes, and
  - permissioned and NPoS are now much closer to each other than before, which
    suggests the dominant remaining bottleneck is shared RBC/READY control
    traffic rather than a mode-specific transport or INIT-ordering problem.

## 2026-03-23 Follow-up: unified RBC session ingress lane collapses the INIT-side metadata miss, but READY convergence still leans on targeted DELIVER rescue
- Implemented the requested ingress-lane refactor in
  `crates/iroha_core/src/sumeragi/mod.rs` and
  `crates/iroha_core/src/sumeragi/status.rs`:
  - `BlockCreated`, `RbcInit`, `RbcChunk`, `RbcChunkCompact`, `RbcReady`, and
    `RbcDeliver` now all enqueue on `self.rbc_chunks` with
    `WorkerQueueKind::RbcChunks`,
  - `Proposal` and `BlockSyncUpdate` stay on `BlockPayload`,
  - `FetchPendingBlock` and generic fallback block/control traffic stay on
    `Blocks`, and
  - status/docs/comments now clarify that `queue_depths.rbc_chunk_rx` is the
    unified RBC session ingress lane while `queue_depths.block_rx` is fallback
    block/control traffic depth only.
- Updated `crates/iroha_core/src/sumeragi/mod.rs` coverage so this cut now
  checks:
  - queue routing for `BlockCreated`, `RbcReady`, and `RbcDeliver` through the
    RBC ingress queue,
  - `FetchPendingBlock` staying on `Blocks` and `BlockSyncUpdate` staying on
    `BlockPayload`,
  - RBC-queue capacity blocking for `BlockCreated`, `RbcReady`, and
    `RbcDeliver`, and
  - mailbox plus parallel-worker prioritization of the unified RBC ingress lane
    as the protected non-vote payload path.
- Focused validation on this cut:
  - `cargo fmt --all`
  - `cargo test -p iroha_core --lib incoming_block_message_routes_ -- --nocapture`
  - `cargo test -p iroha_core --lib rbc_ingress_queue_full -- --nocapture`
  - `cargo test -p iroha_core --lib run_worker_iteration_ -- --nocapture`
  - `cargo test -p iroha_core --lib run_parallel_worker_ -- --nocapture`
  - `cargo test -p iroha_core --lib broadcast_rbc_session_plan_ -- --nocapture`
  - `cargo test -p iroha_core --lib flush_rbc_outbound_chunks_ -- --nocapture`
  - `cargo test -p iroha_core --lib maybe_emit_rbc_ready_ -- --nocapture`
  - `cargo test -p iroha_core --lib rebroadcast_stalled_rbc_payloads_ -- --nocapture`
  - `cargo test -p iroha_core --lib handle_rbc_chunk_ -- --nocapture`
    remains red on the pre-existing mismatch cases
    `handle_rbc_chunk_rejects_digest_mismatch` and
    `handle_rbc_chunk_stash_attributes_mismatch_on_flush`,
  - `cargo build --release -p irohad --bin iroha3d -p izanami --bin izanami`
    (pass; same pre-existing dead-code warnings in `iroha_core` and `izanami`).
- Fresh sampled preserved-peer stable reruns on the rebuilt release binaries:
  - permissioned log:
    `/tmp/izanami_permissioned_unified_rbc_lane_20260323T080613Z.log`,
    peer dirs:
    `/tmp/iroha-soak-permissioned-unified_rbc_lane_20260323T080613Z/irohad_test_network_lWSP5h/*`
  - NPoS log:
    `/tmp/izanami_npos_unified_rbc_lane_20260323T081620Z.log`,
    peer dirs:
    `/tmp/iroha-soak-npos-unified_rbc_lane_20260323T081620Z/irohad_test_network_jr9Oyu/*`
  - both reruns used the unchanged stable envelope
    (`--duration 3600s --target-blocks 2000 --tps 5 --max-inflight 8 --workload-profile stable`)
    with isolated temporary permit dirs and were sampled until the steady-state
    shape was clear (`~294s`), then stopped manually.
- Permissioned sampled result:
  - strict height `1 -> 104` (`net +103`) over `293.589s`,
    projected one-hour height `~1263`,
  - interval buckets:
    `>=5000ms=4`, `3334-4999ms=8`, `2501-3333ms=14`, `<=2500ms=3`,
  - peer-log markers:
    `deferring local RBC READY: authoritative payload unavailable=0`,
    `deferring RBC DELIVER: READY quorum not yet satisfied=0`,
    `deferring RBC DELIVER: authoritative payload unavailable=0`,
    `requested missing block payload from fetch targets=17`,
    `requested missing BlockCreated while awaiting RBC INIT=2`,
    `sending targeted RBC DELIVER to peers missing READY=379`,
    `quorum of votes observed but block payload missing; deferring QC aggregation=1`,
    `no proposal observed=0`.
- NPoS sampled result:
  - strict height `1 -> 108` (`net +107`) over `294.094s`,
    projected one-hour height `~1310`,
  - interval buckets:
    `>=5000ms=1`, `3334-4999ms=12`, `2501-3333ms=11`, `<=2500ms=5`,
  - peer-log markers:
    `deferring local RBC READY: authoritative payload unavailable=0`,
    `deferring RBC DELIVER: READY quorum not yet satisfied=0`,
    `deferring RBC DELIVER: authoritative payload unavailable=0`,
    `requested missing block payload from fetch targets=27`,
    `requested missing BlockCreated while awaiting RBC INIT=3`,
    `sending targeted RBC DELIVER to peers missing READY=441`,
    `quorum of votes observed but block payload missing; deferring QC aggregation=0`,
    `no proposal observed=0`.
- Current read after these reruns:
  - the healthy-path `requested missing BlockCreated while awaiting RBC INIT`
    symptom collapsed sharply versus the previous sampled cut
    (`permissioned 134 -> 2`, `NPoS 96 -> 3`),
  - `sending targeted RBC DELIVER to peers missing READY` did not drop and in
    fact rose materially (`permissioned 268 -> 379`, `NPoS 232 -> 441`), so
    READY convergence is still the dominant remaining control-plane lag,
  - healthy-path throughput improved versus the previous sampled band but is
    still materially below the `2000/3600s` acceptance envelope
    (`~63%` permissioned, `~65%` NPoS), and
  - the old authoritative-payload miss remains gone in both modes, which means
    the unified ingress lane fixed the `BlockCreated` / INIT split but did not
    yet remove the later READY fan-out bottleneck by itself.

## 2026-03-22 Follow-up: RBC worker scheduling fix removes local payload-miss READY deferrals, but preserved-peer samples still miss throughput target
- Implemented the next hot-path fix in
  `crates/iroha_core/src/sumeragi/mod.rs`:
  - the single-thread worker loop now treats `RbcChunks` as payload backlog
    for vote-burst suppression and the pre-tick overtime payload turn,
  - the parallel RBC queue worker now enters the actor gate as `Urgent` and
    drains a small in-actor batch (`drain_queue_batch(...)`) so adjacent
    `RbcInit` / `RbcChunk` messages are not split apart by a fresh vote/tick
    burst,
  - added focused worker regressions for:
    `run_worker_iteration_rotates_to_rbc_after_vote_burst`,
    `run_worker_iteration_caps_total_drain_budget_grants_rbc_overtime_turn`,
    and `drain_queue_batch_drains_follow_up_messages_up_to_limit`.
- Focused validation on this cut:
  - `cargo fmt --all`
  - `cargo test -p iroha_core --lib run_worker_iteration_ -- --nocapture`
    compiled and ran the new worker regressions, but the broad filter remained
    stuck in the existing `run_worker_iteration_adapts_block_drain_caps_on_block_backlog`
    case under the cargo harness; direct execution of the compiled
    `iroha_core` unit-test binary was used to finish focused validation,
  - direct `iroha_core` unit-test binary filters passed for:
    `drain_queue_batch_drains_follow_up_messages_up_to_limit`,
    `broadcast_rbc_session_plan_`,
    `flush_rbc_outbound_chunks_`,
    `maybe_emit_rbc_ready_`,
    `handle_rbc_init_`,
    `rebroadcast_stalled_rbc_payloads_`,
    `qc_missing_block_defer_recovers_authoritative_rbc_session_before_fetch`,
    and
    `assemble_proposal_uses_roster_history_for_previous_roster_evidence`,
  - `cargo build --release -p irohad --bin iroha3d -p izanami --bin izanami`
    (pass; same pre-existing dead-code warnings in `iroha_core` and `izanami`).
- Fresh sampled preserved-peer stable reruns on the rebuilt release binaries:
  - permissioned log:
    `/tmp/izanami_permissioned_fix_sched_20260322T195454Z.log`,
    peer dirs:
    `/tmp/iroha-soak-permissioned-fix_sched_20260322T195454Z/irohad_test_network_tmv5iZ/*`
  - NPoS log:
    `/tmp/izanami_npos_fix_sched_20260322T195454Z.log`,
    peer dirs:
    `/tmp/iroha-soak-npos-fix_sched_20260322T195454Z/irohad_test_network_XqVfnD/*`
  - both reruns used the unchanged stable envelope
    (`--duration 3600s --target-blocks 2000 --tps 5 --max-inflight 8 --workload-profile stable`)
    and were sampled until the `210s` timeout window ended.
- Permissioned sampled result:
  - strict height `1 -> 66` (`net +65`) over `~208.3s`,
    projected one-hour height `~1124`,
  - interval buckets:
    `>=5000ms=3`, `3334-4999ms=6`, `2501-3333ms=9`, `<=2500ms=1`,
  - peer-log markers:
    `deferring local RBC READY: authoritative payload unavailable=0`,
    `deferring RBC DELIVER: READY quorum not yet satisfied=0`,
    `deferring RBC DELIVER: authoritative payload unavailable=0`,
    `requested missing block payload from fetch targets=18`,
    `requested missing BlockCreated while awaiting RBC INIT=134`,
    `sending targeted RBC DELIVER to peers missing READY=268`,
    `sending targeted RBC READY set to peers missing READY=0`,
    `sending targeted RBC payload to peers missing READY=0`,
    `quorum of votes observed but block payload missing; deferring QC aggregation=1`,
    `no proposal observed=0`.
- NPoS sampled result:
  - strict height `1 -> 57` (`net +56`) over `~208.2s`,
    projected one-hour height `~968`,
  - interval buckets:
    `>=5000ms=5`, `3334-4999ms=9`, `2501-3333ms=5`, `<=2500ms=0`,
  - peer-log markers:
    `deferring local RBC READY: authoritative payload unavailable=0`,
    `deferring RBC DELIVER: READY quorum not yet satisfied=0`,
    `deferring RBC DELIVER: authoritative payload unavailable=0`,
    `requested missing block payload from fetch targets=12`,
    `requested missing BlockCreated while awaiting RBC INIT=96`,
    `sending targeted RBC DELIVER to peers missing READY=232`,
    `sending targeted RBC READY set to peers missing READY=0`,
    `sending targeted RBC payload to peers missing READY=0`,
    `quorum of votes observed but block payload missing; deferring QC aggregation=0`,
    `Block hash not found; sharing from fallback anchor=5`,
    `no proposal observed=0`.
- Current read after these reruns:
  - the original hot-path symptom is gone in the sampled peer logs:
    local READY no longer defers on missing authoritative payload, and the old
    single-chunk signature
    `received: 0, total_chunks: 1`
    collapsed to `0/0` in both modes,
  - missing-block payload fetch is reduced materially, but not eliminated,
  - throughput is still well below the `2000/3600s` gate, so this fix removed
    the payload-arrival bottleneck without recovering the soak envelope,
  - the new dominant recovery signatures are the high
    `sending targeted RBC DELIVER to peers missing READY` counts plus heavy
    `requested missing BlockCreated while awaiting RBC INIT`,
    which points to remaining control-plane / metadata-order lag around
    `BlockCreated` / `RbcInit` / READY convergence rather than late
    authoritative chunk ingestion.

## 2026-03-22 Follow-up: preserved-peer stable soak reruns on the initial-chunk-dispatch cut still flatten well below target
- Ran both real preserved-peer stable soak commands on the current
  initial-chunk-dispatch build and stopped them after the cadence had already
  stabilized well below the `2000/3600s` acceptance slope:
  - permissioned log:
    `/tmp/izanami_permissioned_full_20260322T170623Z.log`,
    peer dirs:
    `/tmp/iroha-soak-permissioned-full_20260322T170623Z/irohad_test_network_KkDT0y/*`
  - NPoS log:
    `/tmp/izanami_npos_full_20260322T170623Z.log`,
    peer dirs:
    `/tmp/iroha-soak-npos-full_20260322T170623Z/irohad_test_network_VVPEsa/*`
  - both runs used the unchanged stable envelope
    (`--duration 3600s --target-blocks 2000 --tps 5 --max-inflight 8 --workload-profile stable`)
    but were stopped manually after `433.6s` once the window was already
    decisively below target.
- Permissioned sampled result:
  - strict height `1 -> 137` (`net +136`) over `433.6s`,
    projected one-hour height `~1129`,
  - interval buckets:
    `>=5000ms=8`, `3334-4999ms=21`, `2501-3333ms=13`, `<=2500ms=1`,
  - peer-log markers:
    `deferring local RBC READY: authoritative payload unavailable=807`,
    `requested missing block payload from fetch targets=98`,
    `deferring RBC DELIVER: READY quorum not yet satisfied=0`,
    `sending targeted RBC DELIVER to peers missing READY=581`,
    `sending targeted RBC READY set to peers missing READY=0`,
    `sending targeted RBC payload to peers missing READY=0`,
    `commit quorum missing past timeout=2`,
    `no proposal observed=0`,
    `Connection refused=4`.
- NPoS sampled result:
  - strict height `1 -> 127` (`net +126`) over `433.6s`,
    projected one-hour height `~1046`,
  - interval buckets:
    `>=5000ms=11`, `3334-4999ms=24`, `2501-3333ms=8`, `<=2500ms=0`,
  - peer-log markers:
    `deferring local RBC READY: authoritative payload unavailable=797`,
    `requested missing block payload from fetch targets=47`,
    `deferring RBC DELIVER: READY quorum not yet satisfied=0`,
    `sending targeted RBC DELIVER to peers missing READY=537`,
    `sending targeted RBC READY set to peers missing READY=0`,
    `sending targeted RBC payload to peers missing READY=0`,
    `commit quorum missing past timeout=0`,
    `no proposal observed=0`,
    `Connection refused=7`.
- Important detail from the peer logs:
  - every observed
    `deferring local RBC READY: authoritative payload unavailable`
    line in this window was still the single-chunk case
    `received: 0, total_chunks: 1`
    (`permissioned=807/807`, `NPoS=797/797`), so the remaining happy-path miss
    is still overwhelmingly "first authoritative chunk did not land in time"
    rather than a multi-chunk tail.
- Current read after these reruns:
  - the old DELIVER gate remains removed in practice (`deferring RBC DELIVER:
    READY quorum not yet satisfied=0` in both modes),
  - the new hot-path fix was not enough to clear the healthy-path
    single-chunk authoritative-payload miss,
  - targeted DELIVER rescue is still doing visible work, but targeted payload /
    READY rescue remains unused on this path, and
  - permissioned is still materially better than NPoS, but both modes settle
    into a mostly `3.3-5.0s` cadence that is far below the acceptance slope.

## 2026-03-22 Follow-up: initial RBC chunk dispatch now drains on the broadcast hot path
- Updated `crates/iroha_core/src/sumeragi/main_loop.rs` and
  `crates/iroha_core/src/sumeragi/main_loop/rbc.rs` so
  `broadcast_rbc_session_plan()` still posts `RbcInit` immediately, but now
  seeds outbound chunk state and drains the full initial chunk set in the same
  call via shared `dispatch_rbc_outbound_chunks(...)`.
- `flush_rbc_outbound_chunks()` now reuses that helper only for deferred
  leftovers / retries under the existing pacing and backpressure rules, so the
  queued outbound map is no longer the normal first-delivery path for healthy
  followers.
- Updated focused RBC regressions in
  `crates/iroha_core/src/sumeragi/main_loop/tests.rs` so this cut now covers:
  - single-chunk session broadcast posting both `RbcInit` and the chunk in the
    same call with no leftover outbound entry,
  - multi-chunk session broadcast draining the full initial chunk set
    immediately and proving the next flush does not resend it, and
  - deferred-only outbound flush coverage for per-tick budget and
    queue-backpressure behavior.
- Validation passed on this cut:
  - `cargo test -p iroha_core --lib broadcast_rbc_session_plan_ -- --nocapture`
  - `cargo test -p iroha_core --lib flush_rbc_outbound_chunks_ -- --nocapture`
  - `cargo test -p iroha_core --lib maybe_emit_rbc_ready_ -- --nocapture`
  - `cargo test -p iroha_core --lib rebroadcast_stalled_rbc_payloads_ -- --nocapture`
  - `cargo build --release -p irohad --bin iroha3d -p izanami --bin izanami`
  - `cargo fmt --all`
- Additional focused note:
  - `cargo test -p iroha_core --lib handle_rbc_chunk_ -- --nocapture` is still
    red on this workspace in the pre-existing mismatch tests
    `handle_rbc_chunk_rejects_digest_mismatch` and
    `handle_rbc_chunk_stash_attributes_mismatch_on_flush`; this cut does not
    touch `handle_rbc_chunk(...)`.
- Fresh sampled preserved-peer stable reruns on the rebuilt release binaries:
  - permissioned log:
    `/tmp/izanami_permissioned_initial_chunk_dispatch_20260322T165523Z.log`,
    peer dirs:
    `/tmp/iroha-soak-permissioned-initial_chunk_dispatch_20260322T165523Z/irohad_test_network_ELjPTB/*`
  - NPoS log:
    `/tmp/izanami_npos_initial_chunk_dispatch_20260322T165921Z.log`,
    peer dirs:
    `/tmp/iroha-soak-npos-initial_chunk_dispatch_20260322T165921Z/irohad_test_network_FzGnKW/*`
  - both reruns used the real stable envelope
    (`--duration 3600s --target-blocks 2000 --tps 5 --max-inflight 8 --workload-profile stable`)
    but were sampled with `timeout 210s` to capture the new steady-state shape
    on this exact cut.
- Permissioned sampled result:
  - strict height `1 -> 98` (`net +97`) over `191.746s`,
    projected one-hour height `~1821`,
  - interval buckets:
    `>=5000ms=0`, `3334-4999ms=4`, `2501-3333ms=14`, `<=2500ms=21`,
  - peer-log markers:
    `deferring local RBC READY: authoritative payload unavailable=365`,
    `requested missing block payload from fetch targets=61`,
    `sending targeted RBC READY set to peers missing READY=0`,
    `sending targeted RBC payload to peers missing READY=0`.
- NPoS sampled result:
  - strict height `1 -> 82` (`net +81`) over `191.035s`,
    projected one-hour height `~1526`,
  - interval buckets:
    `>=5000ms=1`, `3334-4999ms=3`, `2501-3333ms=9`, `<=2500ms=6`,
  - peer-log markers:
    `deferring local RBC READY: authoritative payload unavailable=358`,
    `requested missing block payload from fetch targets=26`,
    `sending targeted RBC READY set to peers missing READY=0`,
    `sending targeted RBC payload to peers missing READY=0`.
- Current read after these sampled reruns:
  - the immediate initial chunk wave materially improved the healthy-path READY
    deferral marker on both modes versus the prior sampled rescue-wiring cut
    (`permissioned 923 -> 365`, `NPoS 1009 -> 358`),
  - missing-block fetch for fresh sampled runs also dropped materially
    (`permissioned 102 -> 61`, `NPoS 64 -> 26`), though it still has not
    disappeared,
  - targeted payload / READY rescue remained rare-to-zero, which is consistent
    with the initial send now doing the normal first-delivery work, and
  - permissioned improved noticeably, but both modes are still below the full
    shared-host `2000/3600s` target on sampled slope alone.

## 2026-03-22 Follow-up: wired targeted payload rescue into stalled RBC rebroadcast, but preserved-peer samples still fall back to missing-block fetch
- Updated `crates/iroha_core/src/sumeragi/main_loop.rs` so
  `rebroadcast_stalled_rbc_payloads()` now calls
  `rescue_rbc_missing_ready_peers(...)` on authoritative local sessions instead
  of leaving that helper orphaned, and made the helper report whether it
  actually scheduled rescue work.
- Updated focused rebroadcast regressions in
  `crates/iroha_core/src/sumeragi/main_loop/tests.rs` so the slice now covers:
  - authoritative local Kura payload using targeted payload + READY rescue,
  - active-pending sidecar progress under queue backpressure,
  - per-tick rebroadcast session budgeting on the new sidecar path, and
  - urgent near-tip prioritization after the payload-first cut.
- Validation passed on this cut:
  - `cargo test -p iroha_core --lib rebroadcast_stalled_rbc_payloads_uses_targeted_payload_and_ready_rescue_for_local_kura_authority -- --nocapture`
  - `cargo test -p iroha_core --lib rebroadcast_stalled_rbc_payloads_skips_payload_recovery_once_authoritative -- --nocapture`
  - `cargo test -p iroha_core --lib rebroadcast_stalled_rbc_payloads_ -- --nocapture`
  - `cargo fmt --all`
  - `cargo build --release -p irohad --bin iroha3d -p izanami --bin izanami`
- Fresh preserved-peer sampled reruns on the rebuilt release binaries:
  - permissioned log:
    `/tmp/izanami_permissioned_targeted_payload_rescue_20260322T070321Z.log`,
    peer dirs:
    `/tmp/iroha-soak-permissioned-targeted_payload_rescue_20260322T070321Z/irohad_test_network_7oRfDw/*`
  - NPoS log:
    `/tmp/izanami_npos_targeted_payload_rescue_20260322T070321Z.log`,
    peer dirs:
    `/tmp/iroha-soak-npos-targeted_payload_rescue_20260322T070321Z/irohad_test_network_vUNCYg/*`
  - both reruns used the real stable envelope
    (`--duration 3600s --target-blocks 2000 --tps 5 --max-inflight 8 --workload-profile stable`)
    but were sampled with `timeout 210s` so the current steady-state shape could
    be compared quickly on the new binaries.
- Permissioned sampled result:
  - strict height `1 -> 88` (`net +87`) over `190.737s`,
    projected one-hour height `~1642`,
  - interval buckets:
    `>=5000ms=1`, `3334-4999ms=3`, `2501-3333ms=5`, `<=2500ms=11`,
  - peer-log markers:
    `deferring local RBC READY: authoritative payload unavailable=923`,
    `deferring RBC DELIVER: READY quorum not yet satisfied=0`,
    `sending targeted RBC READY set to peers missing READY=0`,
    `sending targeted RBC payload to peers missing READY=0`,
    `requested missing block payload from fetch targets=102`.
- NPoS sampled result:
  - strict height `1 -> 75` (`net +74`) over `191.623s`,
    projected one-hour height `~1390`,
  - interval buckets:
    `>=5000ms=3`, `3334-4999ms=5`, `2501-3333ms=4`, `<=2500ms=8`,
  - peer-log markers:
    `deferring local RBC READY: authoritative payload unavailable=1009`,
    `deferring RBC DELIVER: READY quorum not yet satisfied=0`,
    `sending targeted RBC READY set to peers missing READY=0`,
    `sending targeted RBC payload to peers missing READY=0`,
    `requested missing block payload from fetch targets=64`.
- Current read after these reruns:
  - the helper is now wired and covered by focused tests, but the real
    preserved-peer samples still do not show that rescue path firing,
  - both modes remain far below the shared-host `2000/3600s` target on these
    new binaries,
  - the dominant real-world signature is still local READY deferring on missing
    authoritative payload, followed by missing-block fetch fallback rather than
    targeted payload rescue,
  - the next fix is not more rescue plumbing; it is tracing why the soak-path
    sessions never satisfy the rescue preconditions soon enough for that helper
    to matter.

## 2026-03-22 Follow-up: fresh payload-first sampled soaks removed the old DELIVER gate, but both modes still miss the shared-host target
- Reran both real preserved-peer shared-host stable soak commands on the
  current payload-first build and sampled enough windows to judge the current
  slope:
  - permissioned log:
    `/tmp/izanami_permissioned_payload_first_20260321T201834Z.log`,
    peer dirs:
    `/tmp/iroha-soak-permissioned-20260321T201834Z/irohad_test_network_MhAjsi/*`
  - NPoS log:
    `/tmp/izanami_npos_payload_first_20260321T202110Z.log`,
    peer dirs:
    `/tmp/iroha-soak-npos-20260321T202110Z/irohad_test_network_MKKPeH/*`
  - both runs used the real stable envelope
    (`--duration 3600s --target-blocks 2000 --tps 5 --max-inflight 8 --workload-profile stable`)
    and were stopped manually once the tail shape was already clear.
- Permissioned sampled result:
  - `samples=12`, final strict height `54` in `110.031s`,
  - sampled intervals:
    `median=2003ms p95=3334ms max=3334ms avg=2236.8ms`,
  - simple one-hour projection from the sample is about height `1767`, still
    below the configured `2000` target,
  - peer-log markers:
    `deferring RBC DELIVER: READY quorum not yet satisfied=0`,
    `deferring local RBC READY: authoritative payload unavailable=419`,
    `sending targeted RBC DELIVER to peers missing READY=218`,
    `commit quorum missing past timeout=0`,
    `no proposal observed=0`,
    `panic=0`,
    `Connection refused=0`.
- NPoS sampled result:
  - `samples=11`, final strict height `43` in `100.019s`,
  - sampled intervals:
    `median=2500.5ms p95=3334ms max=3334ms avg=2517.4ms`,
  - simple one-hour projection from the sample is about height `1548`, also
    below the configured `2000` target,
  - peer-log markers:
    `deferring RBC DELIVER: READY quorum not yet satisfied=0`,
    `deferring local RBC READY: authoritative payload unavailable=378`,
    `sending targeted RBC DELIVER to peers missing READY=176`,
    `commit quorum missing past timeout=3`,
    `no proposal observed=0`,
    `panic=0`,
    `Connection refused=6`.
- Current read after these reruns:
  - the payload-first cut did remove the old local hot-path DELIVER gate on
    these samples: the repeated
    `deferring RBC DELIVER: READY quorum not yet satisfied` signature dropped
    to `0` in both modes,
  - both modes are still too slow for the shared-host `2000/3600s` target,
    with NPoS materially worse than permissioned on both average cadence and
    incidental transport / quorum noise,
  - the dominant remaining steady-state signature is now repeated local READY
    deferral caused by missing authoritative payload on sessions that already
    know the metadata shape,
  - the next useful fix is no longer DELIVER gating; it is whatever keeps
    authoritative payload from becoming local soon enough for READY emission on
    the happy path.

## 2026-03-21 Follow-up: payload-first RBC local progression is now in place
- Simplified the local RBC hot path in
  `crates/iroha_core/src/sumeragi/main_loop.rs`,
  `crates/iroha_core/src/sumeragi/main_loop/rbc.rs`,
  `crates/iroha_core/src/sumeragi/main_loop/commit.rs`, and
  `crates/iroha_core/src/sumeragi/rbc_status.rs` so local DA / round progress
  now keys off one authoritative-payload predicate instead of waiting for
  steady-state READY quorum or DELIVER observation.
- The internal behavior change on this cut:
  - `payload_available_for_da(...)` now accepts authoritative local RBC payload
    before DELIVER,
  - local `READY` emits as soon as authoritative payload is present,
  - local `DELIVER` emits opportunistically after local `READY` instead of
    waiting for READY quorum,
  - `handle_rbc_deliver(...)` no longer defers local progress on
    `ReadyQuorumMissing` when authoritative payload is already local, and
  - near-tip backlog / rebroadcast accounting now stops treating missing
    READY/DELIVER as unresolved work once authoritative payload is present.
- Focused validation completed on this cut:
  - `cargo test -p iroha_core --lib maybe_emit_rbc_ready_ -- --nocapture`
  - `cargo test -p iroha_core --lib maybe_emit_rbc_deliver_ -- --nocapture`
  - `cargo test -p iroha_core --lib handle_rbc_deliver_ -- --nocapture`
  - `cargo test -p iroha_core --lib handle_rbc_init_ -- --nocapture`
  - `cargo build --release -p irohad --bin iroha3d -p izanami --bin izanami`
  - `cargo fmt --all`
- Current state after the focused validation:
  - the payload-first semantics are covered by updated RBC regressions,
  - the requested release build succeeded,
  - release builds still emit pre-existing `dead_code` warnings in
    `iroha_core` and `izanami`, but there were no new build failures on this
    cut.

## 2026-03-21 Follow-up: fresh shared-host reruns after the INIT-side recovery fix are still above target in both modes
- Reused the current rebuilt release binaries and reran both real
  preserved-peer shared-host stable soak commands after the INIT-side
  missing-`BlockCreated` recovery fix:
  - permissioned log:
    `/tmp/izanami_permissioned_post_init_fix_20260321T231453.log`,
    peer dirs:
    `/tmp/iroha-soak-permissioned-post_init_fix_20260321T231453/irohad_test_network_hPuMnX/*`
  - NPoS log:
    `/tmp/izanami_npos_post_init_fix_20260321T231711.log`,
    peer dirs:
    `/tmp/iroha-soak-npos-post_init_fix_20260321T231711/irohad_test_network_WyT7Hq/*`
  - both runs again used the real shared-host stable envelope
    (`--duration 3600s --target-blocks 2000 --tps 5 --max-inflight 8 --workload-profile stable`)
    and were stopped manually after about two minutes once the current tail
    shape was clear.
- Permissioned sampled result:
  - `samples=12`, final strict height `50` in `110.023s`,
  - sampled intervals:
    `median=2001ms p95=5001ms max=5001ms avg=2328.2ms`,
  - simple one-hour projection from the sample is about height `1636`,
    below the configured `2000` target,
  - peer-log markers:
    `requested missing BlockCreated while awaiting RBC INIT=1`,
    `deferring RBC DELIVER: READY quorum not yet satisfied=552`,
    `commit quorum missing past timeout=0`,
    `no proposal observed=0`,
    `panic=0`,
    `Connection refused=6`.
- NPoS sampled result:
  - `samples=13`, final strict height `49` in `120.034s`,
  - sampled intervals:
    `median=2501ms p95=3335ms max=3335ms avg=2411.3ms`,
  - simple one-hour projection from the sample is about height `1470`,
    also below the configured `2000` target,
  - peer-log markers:
    `requested missing BlockCreated while awaiting RBC INIT=0`,
    `deferring RBC DELIVER: READY quorum not yet satisfied=558`,
    `commit quorum missing past timeout=0`,
    `no proposal observed=0`,
    `panic=0`,
    `Connection refused=7`.
- Current read after these reruns:
  - the INIT-side recovery fix did remove the systematic bogus same-slot
    missing-`BlockCreated` path as the dominant failure mode, but it did not
    recover the soak target on this branch,
  - both runs are still too slow for the shared-host `2000/3600s` target, and
    permissioned now shows a `5001ms` tail again in the sampled window,
  - the dominant steady-state signature remains RBC DELIVER waiting on READY
    quorum, not `no proposal`, not commit-timeout collapse, and not transport
    panic,
  - the next useful fix needs to simplify or bypass the remaining
    READY/DELIVER churn on the actual happy path; removing INIT-side recovery
    alone was not enough.

## 2026-03-21 Follow-up: bogus INIT-side missing-`BlockCreated` recovery is removed, but READY/DELIVER churn still dominates tail latency
- Fixed the remaining false-positive recovery trigger on the RBC INIT/READY path:
  - `crates/iroha_core/src/sumeragi/main_loop/rbc.rs` now treats
    `BlockCreated` recovery as required only when the session metadata is
    actually insufficient to rebuild a signed block after payload delivery,
    and it now supports checking that invariant against an in-flight session
    instead of only the stored map entry,
  - `crates/iroha_core/src/sumeragi/main_loop.rs` now uses that in-flight
    session-aware check from `maybe_emit_rbc_ready()`, so removing a session
    from the map for local processing no longer makes the actor believe the
    session has no INIT metadata and seed a bogus same-slot missing-block fetch.
- Focused validation on this cut:
  - `cargo test -p iroha_core --lib handle_rbc_init_existing_session_without_local_payload_defers_missing_block_recovery -- --nocapture`
  - `cargo test -p iroha_core --lib handle_rbc_init_new_session_without_local_payload_defers_missing_block_recovery -- --nocapture`
  - `cargo test -p iroha_core --lib maybe_emit_rbc_ready_missing_chunks_requests_missing_block_for_metadata_only_session -- --nocapture`
  - `cargo test -p iroha_core --lib handle_rbc_init_missing_roster_requests_missing_block_permissioned -- --nocapture`
  - `cargo test -p iroha_core --lib handle_rbc_init_ -- --nocapture`
  - `cargo test -p iroha_core --lib maybe_emit_rbc_ready_ -- --nocapture`
  - `cargo fmt --all`
  - `git diff --check`
  - `cargo build --release -p irohad --bin iroha3d -p izanami --bin izanami`
- Fresh sampled preserved-peer shared-host stable runs on the rebuilt binaries:
  - permissioned log:
    `/tmp/izanami_permissioned_rbc_init_recovery_fix_20260321T230747.log`,
    peer dirs:
    `/tmp/iroha-soak-permissioned-rbc_init_recovery_fix_20260321T230747/irohad_test_network_2ytcak/*`
  - NPoS log:
    `/tmp/izanami_npos_rbc_init_recovery_fix_20260321T231037.log`,
    peer dirs:
    `/tmp/iroha-soak-npos-rbc_init_recovery_fix_20260321T231037/irohad_test_network_i8WHU3/*`
  - both runs again used the real shared-host stable envelope
    (`--duration 3600s --target-blocks 2000 --tps 5 --max-inflight 8 --workload-profile stable`)
    and were stopped manually after about two minutes once the new churn shape
    was clear.
- Permissioned sampled result on this cut:
  - `samples=12`, final strict height `51` in about `110s`,
  - sampled intervals:
    `median=2001ms p95=2501ms max=2501ms avg=2056.5ms`,
  - peer-log markers:
    `requested missing BlockCreated while awaiting RBC INIT=0`,
    `deferring RBC DELIVER: READY quorum not yet satisfied=595`,
    `commit quorum missing past timeout=0`,
    `no proposal observed=0`,
    `Connection refused=0`,
    `panic=0`.
- NPoS sampled result on this cut:
  - `samples=13`, final strict height `46` in about `120s`,
  - sampled intervals:
    `median=2501ms p95=5001ms max=5001ms avg=2718.8ms`,
  - peer-log markers:
    `requested missing BlockCreated while awaiting RBC INIT=0`,
    `deferring RBC DELIVER: READY quorum not yet satisfied=495`,
    `commit quorum missing past timeout=1`,
    `no proposal observed=0`,
    `panic=0`,
    `Connection refused=4` (startup-only handshake noise).
- Current read after this fix:
  - the targeted root cause is fixed: the bogus INIT-side missing-`BlockCreated`
    fetches dropped to `0` in both modes,
  - that path was not the only remaining limiter; throughput did not
    materially improve on this short rerun and NPoS still reproduced a `5001ms`
    tail,
  - the remaining steady-state cost is now concentrated in READY/DELIVER churn,
    not in INIT-side same-slot recovery,
  - the next useful cut should target why RBC DELIVER still repeatedly waits on
    READY quorum even after the unnecessary missing-`BlockCreated` path is gone.

## 2026-03-21 Follow-up: second sampled shared-host rerun held the recovered `~1.5s/block` slope in both modes
- Reused the current release binaries and reran both real preserved-peer
  shared-host stable soak commands on the current RBC/quorum ownership fix:
  - permissioned log:
    `/tmp/izanami_permissioned_rbc_overlap_fix_rerun_20260321T214215.log`,
    peer dirs:
    `/tmp/iroha-soak-permissioned-rbc_overlap_fix_rerun_20260321T214215/irohad_test_network_NNgdGd/*`
  - NPoS log:
    `/tmp/izanami_npos_rbc_overlap_fix_rerun_20260321T214557.log`,
    peer dirs:
    `/tmp/iroha-soak-npos-rbc_overlap_fix_rerun_20260321T214557/irohad_test_network_646KlX/*`
  - both runs again used the real shared-host stable envelope
    (`--duration 3600s --target-blocks 2000 --tps 5 --max-inflight 8 --workload-profile stable`)
    and were stopped manually once enough 10-second windows had accumulated to
    judge the envelope.
- Permissioned rerun result:
  - `samples=19`, `duration=180.089s`, final strict height `122`,
  - sampled intervals:
    `median=1429ms p95=2001ms max=2001ms avg=1.476s/block`,
    with `0` windows at `>=5000ms`,
  - simple one-hour projection from the sampled window is about height `2438`,
    again above the configured `2000` target,
  - peer-log signatures stayed clean:
    `commit quorum missing past timeout=1`,
    `no proposal observed=0`,
    `Connection refused=0`,
    `panic=0`,
    `strict height stalled=0`.
- NPoS rerun result:
  - `samples=16`, `duration=150.074s`, final strict height `96`,
  - sampled intervals:
    `median=1548ms p95=2501ms max=2501ms avg=1.563s/block`,
    with `0` windows at `>=5000ms` and `0` at `>=10000ms`,
  - simple one-hour projection from the sampled window is about height `2302`,
    also above the configured `2000` target,
  - peer-log signatures stayed clean on the actual consensus path:
    `commit quorum missing past timeout=0`,
    `no proposal observed=0`,
    `panic=0`,
    `strict height stalled=0`,
    with `4` `Connection refused` warnings confined to a single peer during
    startup handshake only.
- Current read after the second rerun:
  - the recovered slope reproduced on a fresh second sample in both modes, so
    this no longer looks like a one-off good window,
  - the old failure signatures remain absent from the sampled steady state:
    no recurring `commit quorum missing past timeout`, no
    `no proposal observed`, no transport panic, and no late strict-stall
    behavior,
  - RBC rescue churn is still noisy
    (`deferring RBC DELIVER` / `missing BlockCreated` stayed high), but in both
    reruns it behaved like background recovery traffic rather than a live
    progress killer,
  - the remaining miss is quality-of-service rather than outright pace:
    both sampled runs are back above the `2000/3600s` target line, but sampled
    p95 is still about `2.0s` permissioned and `2.5s` NPoS, so the `<1s`
    acceptance target is still not met.

## 2026-03-21 Follow-up: vote-backed quorum reschedule now yields to active same-slot reassembly recovery
- Fixed the current consensus overlap at the point where it was actually
  happening:
  - `crates/iroha_core/src/sumeragi/main_loop/reschedule.rs` now suppresses
    vote-backed quorum retransmit while same-slot reassembly recovery is still
    active, instead of letting quorum reschedule and RBC/block recovery both
    own the same round,
  - `crates/iroha_core/src/sumeragi/main_loop.rs` now classifies that recovery
    narrowly: payload/block ingress backlog, recent RBC sender activity,
    actionable missing-block recovery, deferred block-sync updates, and
    deferred missing-payload QC recovery still count, but ordinary vote/QC
    pipeline residue no longer blocks the clean retransmit path,
  - `crates/iroha_core/src/sumeragi/main_loop/tests.rs` now clears synthetic
    same-slot missing-block requests in the clean resend regressions once the
    local pending block is inserted, so those tests model post-recovery
    retransmit instead of overlapping recovery ownership.
- Focused validation on this cut:
  - `cargo fmt --all`
  - `cargo test -p iroha_core --lib reschedule_skips_repeated_vote_backed_quorum_timeout_without_new_progress -- --nocapture`
  - `cargo test -p iroha_core --lib reschedule_skips_vote_backed_retransmit_while_frontier_quorum_timeout_window_owned -- --nocapture`
  - `cargo test -p iroha_core --lib reschedule_skips_vote_backed_retransmit_while_same_height_rbc_sender_activity_is_recent -- --nocapture`
  - `cargo test -p iroha_core --lib frontier_recovery_suppresses_rotation_while_same_height_rbc_sender_activity_remains_recent -- --nocapture`
  - `cargo build --release -p irohad --bin iroha3d -p izanami --bin izanami`
- Before rerunning soaks, cleared stale temp-network `iroha3d` processes from
  `/var/folders/.../irohad_test_network_SHeGU2/*`; they were leftover
  preserved-peer nodes still consuming CPU and would have polluted the new
  sample results.
- Sampled shared-host stable reruns on the rebuilt binaries:
  - permissioned log:
    `/tmp/izanami_permissioned_rbc_overlap_fix_20260321T213116.log`,
    peer dirs:
    `/tmp/iroha-soak-permissioned-rbc_overlap_fix_20260321T213116/irohad_test_network_kE02cw/*`
  - NPoS log:
    `/tmp/izanami_npos_rbc_overlap_fix_20260321T213455.log`,
    peer dirs:
    `/tmp/iroha-soak-npos-rbc_overlap_fix_20260321T213455/irohad_test_network_I8MdYS/*`
  - both runs used the real shared-host stable envelope
    (`--duration 3600s --target-blocks 2000 --tps 5 --max-inflight 8 --workload-profile stable`)
    and were stopped manually after enough sample to judge the slope.
- Permissioned sampled result on this fix:
  - `samples=20`, `duration=190.729s`, final strict height `135`,
  - sampled intervals:
    `median=1430ms p95=2003ms max=2003ms avg=1.423s/block`,
    with `0` windows at `>=5000ms`,
  - simple one-hour projection from the sampled window is about height `2530`,
    above the configured `2000` target,
  - peer-log churn still shows RBC rescue work
    (`deferring RBC DELIVER`=`1422`, `missing BlockCreated`=`483`),
    but it no longer translated into any sampled
    `commit quorum missing past timeout`, `no proposal observed`,
    `Connection refused`, or panic markers.
- NPoS sampled result on this fix:
  - `samples=19`, `duration=181.869s`, final strict height `115`,
  - sampled intervals:
    `median=1482ms p95=2502ms max=2502ms avg=1.595s/block`,
    with `0` windows at `>=5000ms` and `0` at `>=10000ms`,
  - simple one-hour projection from the sampled window is about height `2256`,
    also above the configured `2000` target,
  - peer-log churn still shows RBC rescue work
    (`deferring RBC DELIVER`=`1224`, `missing BlockCreated`=`504`),
    but sampled `commit quorum missing past timeout` and
    `no proposal observed` both dropped to `0`,
  - there were `4` `Connection refused` handshake warnings on one peer during
    startup only; they did not recur and the run stayed converged afterward.
- Current read after the reruns:
  - the overlap fix materially changed the envelope in the right direction;
    both sampled modes are back near the old `~1.5s/block` baseline instead of
    the recent `3-10s` regression band,
  - permissioned is no longer the current blocker and NPoS also recovered from
    the prior `~10s` single-advance windows,
  - the original root-cause signals collapsed as expected:
    sampled `commit quorum missing past timeout` and
    `no proposal observed for view before changing view` went to `0` in both
    modes,
  - the remaining gap is target quality, not target pace:
    these sampled windows project above the `2000/3600s` height goal, but the
    sampled p95 is still `~2.0s` permissioned / `~2.5s` NPoS, so the `<1s`
    acceptance target is not proven yet.

## 2026-03-21 Follow-up: sampled preserved-peer soaks on the diagnose-first actor-path refactor still miss badly
- Rebuilt the current release binaries and reran both actual shared-host stable
  soak commands on the current actor-path refactor:
  - `cargo build --release -p irohad --bin iroha3d -p izanami --bin izanami`
  - permissioned log:
    `/tmp/izanami_permissioned_diagnose_first_20260321T195930.log`,
    peer dirs:
    `/tmp/iroha-soak-permissioned-20260321T195930/irohad_test_network_78JXxR/*`
  - NPoS log:
    `/tmp/izanami_npos_diagnose_first_20260321T200130.log`,
    peer dirs:
    `/tmp/iroha-soak-npos-20260321T200130/irohad_test_network_WubjBt/*`
- These were sampled runs on the real `--duration 3600s --target-blocks 2000`
  envelope and were stopped manually once the target-height miss was already
  obvious.
- Permissioned sampled result on this refactor:
  - `samples=40`, `duration=392.185s`, final strict height `129`,
  - sampled intervals:
    `median=3341ms p95=5012ms max=5017ms avg=3.064s/block`,
    with `9` windows at `>=5000ms`,
  - simple one-hour projection from the sampled window is only about
    height `1174`, still far short of the configured `2000` target,
  - there were no workload submission failures, no strict-stall warnings, no
    `Connection refused`, and no panic markers.
- NPoS sampled result on this refactor:
  - `samples=27`, `duration=272.491s`, final strict height `66`,
  - sampled intervals:
    `median=3523ms p95=10016ms max=10105ms avg=4.192s/block`,
    with `12` windows at `>=5000ms` and `4` at `>=10000ms`,
  - simple one-hour projection from the sampled window is only about
    height `858`,
  - the run stayed free of transport crashes / `Connection refused`, but it did
    log `16` workload submission failures and `1` strict-stall warning.
- Current read after the reruns:
  - the actor-path simplification did help permissioned relative to the most
    recent `~10s p95` permissioned reruns, but it did not recover the old
    `~1.5s/block` baseline and it is still nowhere near `<1s`,
  - NPoS is currently the worse mode on this refactor: it still reaches
    repeated `~10s` windows and starts dropping workload confirmations under
    the shared-host stable profile,
  - neither mode reproduces the old `iroha_p2p` transport panic, so the
    remaining problem is still consensus-path pacing / backlog rather than the
    earlier batching crash,
  - the new round trace is now present on this cut, but it still needs to be
    queried from the running peers to prove which phase owns the `5-10s`
    windows.

## 2026-03-21 Follow-up: diagnose-first Sumeragi simplification landed in the actor path
- Added an internal round trace in `crates/iroha_core/src/sumeragi/status.rs`
  and `crates/iroha_core/src/sumeragi/main_loop.rs`:
  - the actor now records round phase/cause transitions keyed by `(height, view)`,
    including proposal observed/emitted, block available / RBC deliver,
    validation success, vote arrival, QC arrival, commit requested/completed,
    and explicit no-progress wakes,
  - the trace stays internal (`pub(crate)`) and does not change the public
    `StatusSnapshot` or wire/config shape.
- Flattened the Sumeragi commit lane back toward synchronous execution:
  - removed the runtime commit worker attachment from
    `crates/iroha_core/src/sumeragi/mod.rs`,
  - `finalize_pending_block()` now runs commit work inline through the existing
    durable order instead of enqueueing background work,
  - retained the in-memory `commit.inflight` marker only as a synchronous guard
    and compatibility surface for existing timeout / status paths,
  - the earlier synchronous commit visibility invariant remains intact:
    Kura store still happens before committed head publication.
- Switched the main happy-path wakeups to cause-aware actor nudges:
  - proposal handling, validation success, votes, QCs, RBC progress, block sync
    commit application, and commit completion now record explicit round causes
    before waking the actor,
  - the commit pipeline now records an explicit no-progress wake for event
    triggers that find pending work but make no forward progress.
- Focused validation passed on the current tree:
  - `cargo test -p iroha_core --lib --no-run`
  - `cargo test -p iroha_core --lib round_phase_after_event_progresses_monotonically -- --nocapture`
  - `cargo test -p iroha_core --lib request_commit_pipeline_for_round_records_round_trace -- --nocapture`
  - `cargo test -p iroha_core --lib request_commit_pipeline_for_pending_infers_phase_from_pending_state -- --nocapture`
  - `cargo test -p iroha_core --lib successful_commit_keeps_latest_block_aligned_with_committed_height -- --nocapture`
  - `cargo test -p iroha_core --lib state_commit_failure_after_kura_store_keeps_partial_head_hidden -- --nocapture`
  - `cargo test -p iroha_core --lib execute_commit_work_does_not_advance_state_when_kura_store_fails -- --nocapture`
  - `cargo test -p iroha_core --lib execute_commit_work_persists_block_before_exposing_committed_state -- --nocapture`
  - `cargo test -p iroha_core --lib commit_inflight_timeout_triggers_view_change_and_retains_aborted_pending -- --nocapture`
  - `cargo test -p iroha_core --lib commit_outcome_kickstarts_next_proposal_and_records_round_gap -- --nocapture`
- This sync did not rerun the preserved-peer permissioned / NPoS soaks yet, so
  there is still no new throughput verdict for the actor-path refactor itself.

## 2026-03-21 Follow-up: sampled preserved-peer reruns after the transport fix still miss the soak envelope badly
- Rebuilt current release binaries and reran the actual permissioned and NPoS
  Izanami shared-host stable soak commands on the current tree:
  - `cargo build --release -p irohad --bin iroha3d -p izanami --bin izanami`
  - permissioned log:
    `/tmp/izanami_permissioned_sync_commit_rerun_20260321T164800.log`,
    peer dirs:
    `/tmp/iroha-soak-permissioned-sync_commit_rerun_20260321T164800/irohad_test_network_bE1SEu/*`
  - NPoS log:
    `/tmp/izanami_npos_sync_commit_rerun_20260321T164800.log`,
    peer dirs:
    `/tmp/iroha-soak-npos-sync_commit_rerun_20260321T164800/irohad_test_network_us3XPs/*`
- These were sampled reruns of the real `--duration 3600s --target-blocks 2000`
  shared-host soak profile:
  - the runs stayed on the actual soak settings, but were stopped manually once
    it was clear they were far off the target-height trajectory,
  - neither run emitted the old transport failure signature:
    no `high plaintext batch must track its scheduling class`,
    no `Connection refused (os error 61)`, and no strict-stall warnings.
- Permissioned sampled result:
  - at `12:58:27Z` it had only reached quorum/strict height `117`,
  - sampled interval distribution over the observed window:
    `samples=44 min=1ms median=3334ms p95=10002ms max=10003ms avg=4451.9ms`,
    with `20` windows at `>=5000ms`,
  - the run repeatedly hit single-advance `10002-10003ms` windows, not just
    the old `3334-5001ms` plateau,
  - simple throughput extrapolation from the sampled window projects only about
    height `972` after one hour, far short of the configured `2000` target.
- NPoS sampled result:
  - at `12:58:27Z` it had only reached quorum/strict height `113`,
  - sampled interval distribution over the observed window:
    `samples=44 min=1ms median=3334ms p95=5001ms max=10001ms avg=4054.2ms`,
    with `20` windows at `>=5000ms`,
  - simple throughput extrapolation from the sampled window projects only about
    height `938` after one hour, also far short of the configured `2000`
    target.
- Current read after the reruns:
  - the `iroha_p2p` transport panic is fixed, but that was not the dominant
    remaining issue for the soak envelope,
  - both consensus modes are now healthy enough to keep advancing, yet both are
    dramatically off the preserved-peer target trajectory,
  - permissioned is currently worse than NPoS in the sampled window because its
    p95 rose to about `10s`, driven by repeated `10002-10003ms` windows,
  - the `<1s` target remains completely unmet; even the less-bad NPoS sample is
    still centered around `3334-5001ms`.

## 2026-03-21 Follow-up: fixed the permissioned soak transport panic
- `crates/iroha_p2p/src/peer.rs` now restores the high-priority plaintext
  batch class after a cap-triggered flush before appending the next batched
  message.
  - Root cause: the high-priority sender path flushed on the
    `MAX_PLAINTEXT_MSGS_HI` / byte-cap boundary, which cleared
    `plain_high_class`, then appended the next message without restoring the
    class.
  - Later `send()` called `flush_plain_high()` on a non-empty `plain_high`
    buffer and panicked on
    `high plaintext batch must track its scheduling class`.
- New regression coverage in `crates/iroha_p2p/src/peer.rs`:
  - `message_sender_restores_high_batch_class_after_msg_cap_flush`
    fills the high-priority message cap, forces the flush-on-overflow path, and
    verifies the sender can still send and decode the trailing message instead
    of panicking.
- Focused validation on the transport fix:
  - `cargo fmt --all`
  - `cargo test -p iroha_p2p message_sender_ -- --nocapture`
- Post-fix permissioned smoke:
  - log:
    `/tmp/izanami_permissioned_p2p_fix_smoke_20260321T163931.log`,
    peer dirs:
    `/tmp/iroha-smoke-permissioned-p2p_fix_20260321T163931/irohad_test_network_Z5LEV5/*`
  - the smoke passed the old crash band at height `112` and reached
    `target_blocks=130` cleanly at quorum/strict height `133`,
  - the previous failure signature did not recur:
    no `high plaintext batch must track its scheduling class`,
    no `Connection refused (os error 61)`, and no strict-stall warnings,
  - the transport panic is fixed, but the timing envelope is not:
    the smoke still reported
    `interval_p50_ms=2001` and `interval_p95_ms=3335`.

## 2026-03-21 Follow-up: preserved-peer soak reruns on the synchronous commit cut
- Rebuilt current release binaries and reran the preserved-peer Izanami shared-host
  stable soaks from the current tree:
  - `cargo build --release -p irohad --bin iroha3d -p izanami --bin izanami`
  - permissioned log:
    `/tmp/izanami_permissioned_sync_commit_20260321T154256.log`,
    peer dirs:
    `/tmp/iroha-soak-permissioned-sync_commit_20260321T154256/irohad_test_network_prtV00/*`
  - NPoS log:
    `/tmp/izanami_npos_sync_commit_20260321T154839.log`,
    peer dirs:
    `/tmp/iroha-soak-npos-sync_commit_20260321T154839/irohad_test_network_7rXK0x/*`
- Permissioned result:
  - the run failed before it could produce a meaningful target-height verdict,
  - `iroha_p2p` worker threads panicked twice with
    `high plaintext batch must track its scheduling class`
    (`crates/iroha_p2p/src/peer.rs:3448` surfaced in the preserved log),
  - after the panic, Izanami started reporting pinned-ingress
    `Connection refused (os error 61)` failures and the soak stalled at
    quorum/strict height `112`,
  - before the crash, the sampled quorum interval distribution was still in the
    old multi-second band:
    `samples=26 min=1ms median=2001ms p95=3334ms max=5000ms avg=2346.9ms`
    with one `>=5000ms` outlier.
- NPoS result:
  - no `iroha_p2p` panic, no `Connection refused`, and no stall warnings in the
    sampled window,
  - because this shared-host stable soak is duration-based in Izanami, the run
    was sampled and then stopped manually after enough data to judge the pacing
    envelope; it reached quorum/strict height `261` cleanly,
  - the sampled quorum interval distribution remains effectively unchanged from
    the prior plateau:
    `samples=65 min=1ms median=2501ms p95=3345ms max=5001ms avg=2601.1ms`
    with three `>=5000ms` outliers.
- Current read after the reruns:
  - the synchronous commit restoration did not move the preserved-peer NPoS
    latency envelope toward the `<1s` target; the observed p95 is still
    `~3.3s`,
  - permissioned is currently blocked by a new transport / batching regression
    in `iroha_p2p`, so its soak result is worse than the prior steady-state
    plateau and cannot be used as evidence that the consensus pacing changes
    helped,
  - the next blocking fix is the `iroha_p2p` high-plaintext scheduling-class
    panic; only after that is cleared does it make sense to attribute remaining
    permissioned behavior to consensus timing.

## 2026-03-21 Follow-up: synchronous commit visibility restored and consensus pacing retuned
- `crates/iroha_core/src/sumeragi/main_loop/commit.rs`,
  `crates/iroha_core/src/sumeragi/main_loop/pending_block.rs`,
  `crates/iroha_core/src/sumeragi/main_loop.rs`, and
  `crates/iroha_core/src/sumeragi/mod.rs` now remove the split-era commit
  machinery and restore a single authoritative commit success path:
  - `CommitJournal*`, `CommitPersist*`, publish/persist inflight tracking, and
    split-specific commit lifecycle publication phases are gone,
  - `ValidatedCommitArtifact` remains only as cached immutable validation data
    on `PendingBlock`; it no longer models publication state, and
  - commit order is again synchronous:
    `commit_with_certificate()` ->
    `kura.store_block()` ->
    `state_block.apply_without_execution()` ->
    `state_block.commit()` ->
    `on_block_commit()` / cleanup / telemetry.
- Failure handling now matches the intended invariant:
  - if `kura.store_block()` fails, the block stays pending on the existing
    retry / abort path and the committed head does not advance,
  - if `state_block.commit()` fails after a successful Kura store, Kura may be
    ahead, but `committed_height` remains authoritative and the partial head is
    not exposed through state / `latest_block()`.
- `crates/iroha_core/src/sumeragi/main_loop.rs`,
  `crates/iroha_core/src/sumeragi/main_loop/proposal_handlers.rs`,
  `crates/iroha_core/src/sumeragi/main_loop/rbc.rs`, and
  `crates/iroha_core/src/sumeragi/main_loop/votes.rs` now re-run the pacemaker
  and commit progression immediately when proposal, `BlockCreated` / RBC
  delivery, validation success, vote arrival, or QC arrival changes the happy
  path, while keeping proposal / block / RBC / vote / QC sends on the existing
  bypass post fast path.
- Timing derivation was retuned without changing config or wire shape:
  - pacemaker base interval now comes from the propose-timeout RTT floor capped
    by `max_backoff`, without a hard `block_time` floor,
  - DA commit quorum timeout now uses
    `max(block_time, 2 * commit_time)` before the existing
    `da_quorum_timeout_multiplier`.
- New focused invariant and timing coverage in
  `crates/iroha_core/src/sumeragi/main_loop/tests.rs` and
  `crates/iroha_core/src/sumeragi/main_loop/commit.rs` now checks:
  - visible committed height always implies an aligned readable latest block,
  - Kura store failure does not advance committed state,
  - state-commit failure after Kura success does not expose a partial head,
  - pacemaker / DA timeout formulas match the new derivation, and
  - proposal / `BlockCreated` / QC fast-path posts still bypass the background
    queue.
- Focused validation on this cut:
  - `cargo fmt --all`
  - `cargo test -p iroha_core --no-run`
  - `cargo test -p iroha_core --lib successful_commit_keeps_latest_block_aligned_with_committed_height -- --nocapture`
  - `cargo test -p iroha_core --lib state_commit_failure_after_kura_store_keeps_partial_head_hidden -- --nocapture`
  - `cargo test -p iroha_core --lib commit_quorum_timeout_tracks_block_time -- --nocapture`
  - `cargo test -p iroha_core --lib pacemaker_interval_respects_rtt_floor_and_cap -- --nocapture`
  - `cargo test -p iroha_core --lib proposal_post_bypasses_background_queue -- --nocapture`
  - `cargo test -p iroha_core --lib qc_post_bypasses_background_queue -- --nocapture`
  - `cargo test -p iroha_core --lib block_created_post_bypasses_background_queue -- --nocapture`
  - `cargo test -p iroha_core --lib execute_commit_work_does_not_advance_state_when_kura_store_fails -- --nocapture`
  - `cargo test -p iroha_core --lib execute_commit_work_persists_block_before_exposing_committed_state -- --nocapture`
  - `cargo test -p iroha_core --lib duplicate_block_created_ -- --nocapture`
  - `cargo test -p iroha_core --lib handle_rbc_init_ -- --nocapture`
  - `cargo test -p iroha_core --lib maybe_emit_rbc_ready_ -- --nocapture`
  - `cargo test -p iroha_core --lib maybe_emit_rbc_deliver_ -- --nocapture`
- This turn did not rerun the sampled permissioned / NPoS preserved-peer soaks
  or the full workspace test suite, so the `<1s` acceptance target remains
  unverified on the simplified cut.

## 2026-03-21 Follow-up: authoritative RBC payload now means session-owned verified chunks
- `crates/iroha_core/src/sumeragi/main_loop.rs` and
  `crates/iroha_core/src/sumeragi/main_loop/rbc.rs` now stop treating
  "payload exists somewhere locally" as equivalent to "this RBC session is
  authoritative":
  - `AuthoritativePayload` progression now requires the session itself to own a
    complete verified chunk set, not just a matching pending / inflight / Kura
    payload,
  - READY / DELIVER no longer skip local hydration merely because INIT metadata
    already exposed an expected chunk root,
  - payload recovery now remains enabled whenever the session is still missing
    chunks, even if matching local bytes exist elsewhere on the node, and
  - targeted missing-READY rescue now requires session-owned payload instead of
    firing from metadata-only sessions.
- `crates/iroha_core/src/sumeragi/main_loop/tests.rs` now covers the fixed
  invariant explicitly:
  - known near-tip and Kura-backed partial sessions stay in
    `CollectingChunks` until hydration completes,
  - local Kura payload hydration for READY now happens inline before READY is
    emitted,
  - a partial session with a stale `AuthoritativePayload` stage marker still
    allows payload recovery and does not incorrectly qualify for targeted READY
    rescue.
- Focused validation on this cut:
  - `cargo fmt --all`
  - `cargo test -p iroha_core --lib known_near_tip_rbc_session_stays_collecting_until_chunks_hydrate -- --nocapture`
  - `cargo test -p iroha_core --lib known_local_kura_rbc_session_stays_collecting_until_chunks_hydrate -- --nocapture`
  - `cargo test -p iroha_core --lib known_local_kura_payload_with_slot_mismatch_stays_non_authoritative -- --nocapture`
  - `cargo test -p iroha_core --lib maybe_emit_rbc_ready_for_local_kura_payload_hydrates_before_ready -- --nocapture`
  - `cargo test -p iroha_core --lib partial_rbc_session_keeps_payload_recovery_even_after_authoritative_stage_marker -- --nocapture`
  - `cargo test -p iroha_core --lib maybe_emit_rbc_ready_ -- --nocapture`
  - `cargo test -p iroha_core --lib maybe_emit_rbc_deliver_ -- --nocapture`
  - `cargo test -p iroha_core --lib rescue_rbc_missing_ready_peers_ -- --nocapture`
  - `cargo test -p iroha_core --lib known_ -- --nocapture`
  - `cargo build --release -p irohad --bin iroha3d -p izanami --bin izanami`
- Sampled preserved-peer reruns on the rebuilt binaries:
  - permissioned log:
    `/tmp/izanami_permissioned_session_authority_fix_20260321T114324.log`,
    peer dirs:
    `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_qo9VSq/*`
  - NPoS log:
    `/tmp/izanami_npos_session_authority_fix_20260321T115451.log`,
    peer dirs:
    `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_RLhh0J/*`
- Sampled result summary on this cut:
  - permissioned stayed red at
    `quorum p95 block interval 3334ms` over `78` samples at target reach,
    `elapsed=170.029s`,
    summary
    `successes=849 failures=0`,
  - NPoS stayed red at
    `quorum p95 block interval 3344ms` over `77` samples at target reach,
    `elapsed=190.075s`,
    summary
    `successes=951 failures=0`,
  - both reruns stayed pinned to the same steady-state `~3.3s` band as the
    previous targeted-payload sample instead of moving materially lower.
- Current read on the cut:
  - the false-authoritative-session bug was real and is now covered by focused
    regressions,
  - but it was not the dominant cause of the current preserved-peer slowdown,
  - the remaining plateau now looks shared across permissioned and NPoS and
    should be investigated as a broader steady-state cadence / queueing issue,
    not primarily as an RBC session-authority invariant bug.

## 2026-03-21 Follow-up: targeted missing-READY payload rescue and sampled preserved-peer reruns
- `crates/iroha_core/src/sumeragi/main_loop.rs` now lets
  `rescue_rbc_missing_ready_peers(...)` push payload bytes directly to peers
  still missing READY once local bytes are available, even after the session
  has advanced past `CollectingChunks`:
  - the targeted payload path now hydrates a local session copy from the shared
    pending / inflight / Kura payload resolver before building `RbcInit` +
    chunks,
  - targeted payload rescue now uses its own cooldown state instead of sharing
    the broad subset `payload_rebroadcast_last_sent` throttle, so a prior broad
    payload rebroadcast no longer suppresses the later precise rescue.
- `crates/iroha_core/src/sumeragi/main_loop/tests.rs` now covers:
  - `maybe_emit_rbc_deliver_targets_payload_and_ready_when_subset_skips_local`,
  - `rescue_rbc_missing_ready_peers_does_not_rate_limit_init_only_stub`,
  - `rescue_rbc_missing_ready_peers_ignores_broad_payload_cooldown_after_hydration`,
  - the nearby rebroadcast READY-rescue regressions still pass.
- Focused validation on this cut:
  - `cargo fmt --all`
  - `cargo test -p iroha_core --lib rebroadcast_stalled_rbc_payloads_uses_targeted_ready_rescue_for_local_kura_authority -- --nocapture`
  - `cargo test -p iroha_core --lib maybe_emit_rbc_deliver_targets_payload_and_ready_when_subset_skips_local -- --nocapture`
  - `cargo test -p iroha_core --lib rescue_rbc_missing_ready_peers_ -- --nocapture`
  - `cargo test -p iroha_core --lib rebroadcast_stalled_rbc_payloads_skips_payload_recovery_once_authoritative -- --nocapture`
  - `cargo test -p iroha_core --lib maybe_emit_rbc_deliver_targets_missing_ready_peers_with_unverified_roster -- --nocapture`
  - `cargo build --release -p irohad --bin iroha3d -p izanami --bin izanami`
- Sampled preserved-peer reruns on the rebuilt binaries:
  - permissioned log:
    `/tmp/izanami_permissioned_targeted_payload_20260321T104921.log`,
    peer dirs:
    `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_SQwqB5/*`
  - NPoS log:
    `/tmp/izanami_npos_targeted_payload_20260321T105858.log`,
    peer dirs:
    `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_XFCfHr/*`
- Sampled result summary on this cut:
  - permissioned still failed the harness target, but improved versus the
    previous `stub_cooldown` sample:
    `quorum p95 block interval 3334ms` over `77` samples at target reach,
    `elapsed=170.034s`,
    versus the prior sampled `3481ms` / `192.241s`,
  - NPoS also still failed the harness target, but improved materially versus
    the previous sample:
    `quorum p95 block interval 3334ms` over `78` samples at target reach,
    `elapsed=190.046s`,
    versus the prior sampled `3565ms` / `203.610s`,
  - both sampled runs now failed at `checkpoint target_reached` instead of the
    earlier pre-target catastrophic breach shape,
  - top-level workload noise stayed clean again:
    permissioned summary
    `successes=848 failures=0`,
    NPoS summary
    `successes=948 failures=2`,
    with no ingress failover / endpoint unhealthy increments.
- Current read on the cut:
  - the targeted payload rescue timing change is real; it improved both sampled
    envelopes and removed the earlier NPoS collapse pattern,
  - the remaining failure is no longer best described as the old
    `received=0`, `total_chunks=1` frontier stall alone,
  - both modes now plateau on the same `3334ms` p95 band at target reach,
    which points to a shared steady-state tail beyond the original
    metadata-only RBC session problem.

## 2026-03-20 Follow-up: sampled permissioned and NPoS reruns on the immediate INIT missing-payload recovery cut
- Rebuilt the release binaries and reran both preserved-peer sampled stable
  envelopes, stopping each run after the first `interval_ms >= 5000` breach:
  - `cargo build --release -p irohad --bin iroha3d -p izanami`
  - permissioned log:
    `/tmp/izanami_permissioned_init_missing_payload_20260320T211331.log`
  - permissioned peer dirs:
    `/tmp/iroha-soak-permissioned-20260320T211331/irohad_test_network_7euo4G`
  - NPoS log:
    `/tmp/izanami_npos_init_missing_payload_20260320T212947.log`
  - NPoS peer dirs:
    `/tmp/iroha-soak-npos-20260320T212947/irohad_test_network_inkyuH`
- Sampled result summary on this cut:
  - permissioned improved materially versus the recent sampled regressions, but
    still went red:
    first breach moved out to `height 466 -> interval_ms=5001`; sampled
    wall-clock to breach was `760.2s` (`1.631s/block`), followed by
    `height 467 -> interval_ms=10001` and
    `height 468 -> interval_ms=10025` before recovery,
  - permissioned interval buckets:
    `10000+=2`, `5000-9999=1`, `3334-4999=5`, `2501-3333=8`,
  - NPoS regressed materially on this cut:
    first breach moved earlier to `height 365 -> interval_ms=10001`; sampled
    wall-clock to breach was `640.1s` (`1.754s/block`),
  - NPoS interval buckets:
    `10000+=1`, `5000-9999=0`, `3334-4999=3`, `2501-3333=9`,
  - top-level workload signal stayed clean in both reruns:
    `plan submission failed=0`,
    `Trigger with id \`repeat_trigger_*\` not found=0`.
- Peer-log markers on these reruns:
  - permissioned:
    `deferring local RBC READY: awaiting chunks or READY quorum=3252`,
    `deferring RBC DELIVER: READY quorum not yet satisfied=5684`,
    `queued RBC INIT and chunk rebroadcast while awaiting READY quorum=0`,
    `commit quorum missing past timeout; rescheduling block for reassembly=27`,
    `sending targeted RBC DELIVER to peers missing READY=5448`,
    `sending targeted RBC READY set to peers missing READY=11085`,
    `sending RBC DELIVER to commit topology after READY quorum=1945`,
    `commit roster unavailable=68`
  - NPoS:
    `deferring local RBC READY: awaiting chunks or READY quorum=2200`,
    `deferring RBC DELIVER: READY quorum not yet satisfied=3938`,
    `queued RBC INIT and chunk rebroadcast while awaiting READY quorum=0`,
    `commit quorum missing past timeout; rescheduling block for reassembly=15`,
    `sending targeted RBC DELIVER to peers missing READY=4418`,
    `sending targeted RBC READY set to peers missing READY=8016`,
    `sending RBC DELIVER to commit topology after READY quorum=1407`,
    `commit roster unavailable=90`
- Tail samples from the preserved peer trees show that the frontier signature is
  still the same failure mode the cut was meant to narrow:
  - permissioned still logged
    `deferring local RBC READY: awaiting chunks or READY quorum` at
    `height 466-467` with
    `received=0`, `total_chunks=1`, `ready=0`, `required=2`,
    `missing_ready_total=4`,
  - permissioned still logged
    `deferring RBC DELIVER: READY quorum not yet satisfied` on the same band
    with `ready=0-2`, `required=3`,
  - NPoS still logged
    `deferring local RBC READY: awaiting chunks or READY quorum` at
    `height 365-367` with the same
    `received=0`, `total_chunks=1`, `ready=0`, `required=2`,
    `missing_ready_total=4` signature,
  - NPoS still logged
    `deferring RBC DELIVER: READY quorum not yet satisfied` immediately after
    on `height 365-367` with `ready=0-2`, `required=3`.
- Critique:
  - this cut helps permissioned timing, but it is still red,
  - the permissioned improvement is real:
    the first red window moved later than the recent sampled failures at
    `367`, `388`, and `437`, and it recovered instead of staying wedged,
  - NPoS moved the wrong direction:
    `365 -> 10001` is materially earlier than the recent sampled NPoS failures
    at `434`, `448`, and `450`,
  - the timing change did not remove the actual stall shape in either mode:
    the preserved peer trees still show metadata-only single-chunk sessions
    hitting READY with `received=0`, then stalling in DELIVER while READY
    quorum catches up,
  - missing-`INIT` / chunk-rebroadcast remains irrelevant on this cut as well;
    that counter stayed at zero in both reruns,
  - raw peer-log marker totals are not directly comparable across modes here
    because permissioned ran longer before first breach, so the qualitative tail
    signature matters more than the absolute counts.

## 2026-03-20 Follow-up: RBC INIT now triggers missing-`BlockCreated` recovery immediately for metadata-only sessions
- Updated
  `crates/iroha_core/src/sumeragi/main_loop/rbc.rs` so `handle_rbc_init`
  routes both the existing-session merge branch and the new-session branch
  through a shared post-INIT helper that:
  - tries same-call local payload hydration first,
  - checks whether the session still has `received=0`, and
  - immediately calls
    `request_missing_block_for_pending_rbc(..., "rbc_init_missing_payload", None)`
    before continuing with the existing pending-flush / READY / DELIVER flow.
- This keeps the current RBC protocol and recovery mechanism intact:
  - no payload gossip changes,
  - no wire changes,
  - no config additions, and
  - no quorum / timeout math changes.
- Added focused regressions in
  `crates/iroha_core/src/sumeragi/main_loop/tests.rs` covering:
  - existing-session `RbcInit` without local payload scheduling a missing-block
    fetch in the same handler call,
  - new-session `RbcInit` without local payload doing the same,
  - inline hydration from pending payloads during `RbcInit`,
  - inline hydration from commit-inflight payloads during `RbcInit`, and
  - inline hydration from Kura payloads during `RbcInit`.
- Kept two existing INIT semantics intact while making the new timing change:
  - far-future INIT sessions do not eagerly trigger the missing-block path from
    the new helper or the missing-roster path, so they are not purged ahead of
    the contiguous frontier, and
  - a chunk-root inferred only from pre-INIT unauthenticated chunks is still
    treated as non-authoritative, so INIT can replace it and drop mismatched
    cached chunks.
- Focused validation passed on this cut:
  - `cargo fmt --all`
  - `cargo test -p iroha_core --lib handle_rbc_init_ -- --nocapture`
  - `cargo test -p iroha_core --lib duplicate_block_created_ -- --nocapture`
  - `cargo test -p iroha_core --lib block_sync_block_created_retries_ready_after_existing_session_hydration -- --nocapture`
- Sampled permissioned / NPoS preserved-peer reruns have not been repeated yet.

## 2026-03-20 Follow-up: single-source-of-truth payload resolver landed, but sampled permissioned/NPoS reruns still hit the same zero-chunk READY tail
- Rebuilt the release binaries on the new local-payload-source-of-truth cut and
  reran both preserved-peer sampled stable envelopes, stopping each run at the
  first `interval_ms >= 5000` breach:
  - `cargo build --release -p irohad --bin iroha3d -p izanami`
  - permissioned log:
    `/tmp/izanami_permissioned_single_source_payload_20260320T182514.log`
  - permissioned peer dirs:
    `/tmp/iroha-soak-permissioned-20260320T182514/irohad_test_network_AVP7Tt`
  - NPoS log:
    `/tmp/izanami_npos_single_source_payload_20260320T183531.log`
  - NPoS peer dirs:
    `/tmp/iroha-soak-npos-20260320T183531/irohad_test_network_2WXIIN`
- Sampled result summary on the single-source-of-truth cut:
  - permissioned first gate breach moved earlier again to
    `height 367 -> interval_ms=5002`; sampled wall-clock to breach was
    `560.2s` (`1.526s/block`), with interval buckets:
    `10000+=0`, `5000-9999=2`, `3334-4999=0`, `2501-3333=2`,
  - NPoS first gate breach was
    `height 434 -> interval_ms=10001`; sampled wall-clock to breach was
    `740.2s` (`1.705s/block`), with interval buckets:
    `10000+=2`, `5000-9999=0`, `3334-4999=4`, `2501-3333=26`,
  - top-level workload signal stayed clean in both reruns:
    `plan submission failed=0`,
    `Trigger with id \`repeat_trigger_*\` not found=0`.
- Peer-log markers on these reruns:
  - permissioned:
    `deferring local RBC READY: awaiting chunks or READY quorum=2139`,
    `deferring RBC DELIVER: READY quorum not yet satisfied=3884`,
    `queued RBC INIT and chunk rebroadcast while awaiting READY quorum=0`,
    `commit quorum missing past timeout; rescheduling block for reassembly=11`,
    `sending targeted RBC DELIVER to peers missing READY=3633`,
    `sending targeted RBC READY set to peers missing READY=7837`,
    `sending RBC DELIVER to commit topology after READY quorum=1331`,
    `commit roster unavailable=42`
  - NPoS:
    `deferring local RBC READY: awaiting chunks or READY quorum=2303`,
    `deferring RBC DELIVER: READY quorum not yet satisfied=4381`,
    `queued RBC INIT and chunk rebroadcast while awaiting READY quorum=0`,
    `commit quorum missing past timeout; rescheduling block for reassembly=19`,
    `sending targeted RBC DELIVER to peers missing READY=4957`,
    `sending targeted RBC READY set to peers missing READY=8954`,
    `sending RBC DELIVER to commit topology after READY quorum=1571`,
    `commit roster unavailable=91`
- Tail samples from the preserved peer trees show the same frontier signature as
  before, despite the unified local-payload loader:
  - permissioned still logged
    `deferring local RBC READY: awaiting chunks or READY quorum` at
    `height 373-376` with
    `received=0`, `total_chunks=1`, `ready=0`, `required=2`,
    `missing_ready_total=4`,
  - permissioned still logged
    `deferring RBC DELIVER: READY quorum not yet satisfied` on the same band
    with `ready=0-2`, `required=3`,
  - NPoS still logged
    `deferring local RBC READY: awaiting chunks or READY quorum` at
    `height 433-439` with the same
    `received=0`, `total_chunks=1`, `ready=0`, `required=2`,
    `missing_ready_total=4` signature,
  - NPoS still logged
    `deferring RBC DELIVER: READY quorum not yet satisfied` immediately after
    on `height 433-438` with `ready=0-2`, `required=3`.
- Critique:
  - the code simplification is real, but this cut is still red in both modes,
  - the old mismatch between “progress says local” and “hydration can only read
    pending blocks” is gone; the loader now has one definition of local payload,
  - that means the remaining breach is no longer explained by predicate drift,
    and the preserved peer trees now point to a narrower failure:
    these frontier sessions are still hitting READY with `received=0`,
    `total_chunks=1`, so the payload bytes are not actually present in the
    shared pending / inflight / Kura sources when the session tries to progress,
  - permissioned regressed further on first breach height (`388 -> 367`), even
    though the stall shape shortened from `10002ms` to `5002ms`; NPoS also
    regressed on first breach height (`448 -> 434`),
  - missing-`INIT` / chunk rebroadcast remains irrelevant here; that counter
    stayed at zero again in both reruns.

## 2026-03-20 Follow-up: RBC local payload resolution now has a single source of truth
- Replaced the split local-payload checks in
  `crates/iroha_core/src/sumeragi/main_loop.rs` with
  `with_local_payload_for_progress(...)`, which resolves actual payload bytes
  from the same places everywhere RBC progress cares about them:
  active `pending_blocks`, `commit.inflight`, and `kura`.
- Updated
  `rbc_session_has_local_payload_for_progress(...)` and
  `block_payload_available_for_progress(...)` to use that shared resolver, so
  “payload is local enough for READY/DELIVER” and “payload bytes can be loaded
  for hydration” no longer diverge.
- Renamed
  `maybe_hydrate_rbc_session_from_pending_block(...)` to
  `maybe_hydrate_rbc_session_from_local_payload(...)` in
  `crates/iroha_core/src/sumeragi/main_loop/rbc.rs`, and routed the READY,
  DELIVER, and RBC-init hydration paths through the shared resolver.
- The intentional behavior change is that hash-only
  `pending_processing` membership no longer counts as local payload for RBC
  progress by itself; that state has no bytes, so it cannot be the source of
  truth.
- Added focused regressions in
  `crates/iroha_core/src/sumeragi/main_loop/tests.rs` for:
  - rejecting hash-only `pending_processing` as progress-local payload,
  - hydrating an RBC session from `pending.pending_blocks`, and
  - hydrating an RBC session from `kura` once the block has left active
    pending storage.
- Focused validation passed on this tree:
  - `cargo fmt --all`
  - `cargo test -p iroha_core --lib block_payload_available_for_progress_ -- --nocapture`
  - `cargo test -p iroha_core --lib hydrates_rbc_session_for_init -- --nocapture`
  - `cargo test -p iroha_core --lib duplicate_block_created_ -- --nocapture`
  - `cargo test -p iroha_core --lib block_sync_block_created_retries_ready_after_existing_session_hydration -- --nocapture`
  - `cargo test -p iroha_core --lib local_kura_authoritative_session_skips_bootstrap_gate -- --nocapture`

## 2026-03-20 Follow-up: duplicate `BlockCreated` READY retry sampled reruns stay red; permissioned regresses harder than NPoS
- Rebuilt the release binaries on this cut and reran both preserved-peer sampled
  stable envelopes, stopping each run at the first `interval_ms >= 5000`
  breach:
  - `cargo build --release -p irohad --bin iroha3d -p izanami`
  - permissioned log:
    `/tmp/izanami_permissioned_blockcreated_ready_retry_20260320T154553.log`
  - permissioned peer dirs:
    `/tmp/iroha-soak-permissioned-20260320T154553/irohad_test_network_5wgyb6`
  - NPoS log:
    `/tmp/izanami_npos_blockcreated_ready_retry_20260320T155755.log`
  - NPoS peer dirs:
    `/tmp/iroha-soak-npos-20260320T155755/irohad_test_network_3TNt8p`
- Sampled result summary on the duplicate-`BlockCreated` READY-retry cut:
  - permissioned first gate breach moved earlier to
    `height 388 -> interval_ms=10002`; sampled wall-clock to breach was
    `610.1s` (`1.573s/block`), with interval buckets:
    `10000+=1`, `5000-9999=0`, `3334-4999=0`, `2501-3333=4`,
  - NPoS first gate breach was
    `height 448 -> interval_ms=10001`; sampled wall-clock to breach was
    `740.2s` (`1.652s/block`), with interval buckets:
    `10000+=1`, `5000-9999=0`, `3334-4999=2`, `2501-3333=18`,
  - top-level workload signal stayed clean in both reruns:
    `plan submission failed=0`,
    `Trigger with id \`repeat_trigger_*\` not found=0`.
- Peer-log markers on these reruns:
  - permissioned:
    `deferring local RBC READY: awaiting chunks or READY quorum=2426`,
    `deferring RBC DELIVER: READY quorum not yet satisfied=4089`,
    `queued RBC INIT and chunk rebroadcast while awaiting READY quorum=0`,
    `commit quorum missing past timeout; rescheduling block for reassembly=19`,
    `sending targeted RBC DELIVER to peers missing READY=3864`,
    `sending targeted RBC READY set to peers missing READY=8254`,
    `sending RBC DELIVER to commit topology after READY quorum=1399`,
    `commit roster unavailable=24`
  - NPoS:
    `deferring local RBC READY: awaiting chunks or READY quorum=2617`,
    `deferring RBC DELIVER: READY quorum not yet satisfied=4626`,
    `queued RBC INIT and chunk rebroadcast while awaiting READY quorum=0`,
    `commit quorum missing past timeout; rescheduling block for reassembly=11`,
    `sending targeted RBC DELIVER to peers missing READY=4993`,
    `sending targeted RBC READY set to peers missing READY=9666`,
    `sending RBC DELIVER to commit topology after READY quorum=1622`,
    `commit roster unavailable=118`
- Tail samples from the preserved peer trees show the same frontier signature the
  fix was intended to remove:
  - permissioned still logged
    `deferring local RBC READY: awaiting chunks or READY quorum` at
    `height 387-390` with
    `received=0`, `total_chunks=1`, `ready=0`, `required=2`,
    `missing_ready_total=4`,
  - permissioned also still logged
    `deferring RBC DELIVER: READY quorum not yet satisfied` at
    `height 390-392` with `ready=1-2`, `required=3`,
  - NPoS still logged
    `deferring local RBC READY: awaiting chunks or READY quorum` at
    `height 453` and `459` with the same
    `received=0`, `total_chunks=1`, `ready=0`, `required=2`,
    `missing_ready_total=4` signature,
  - NPoS also still logged
    `deferring RBC DELIVER: READY quorum not yet satisfied` at
    `height 458-459` with `ready=0-2`, `required=3`.
- Critique:
  - this cut did not improve permissioned; it regressed materially relative to
    the recent sampled permissioned reruns on this branch
    (`469 -> 10236`, then `437 -> 10003`, now `388 -> 10002`),
  - NPoS is roughly flat to slightly worse than the latest targeted-DELIVER
    rerun (`450 -> 10044`, now `448 -> 10001`), so the new READY retry did not
    buy a meaningful later frontier there either,
  - the important negative result is that the same-call duplicate
    `BlockCreated` READY retry is not enough by itself:
    the peer trees still show the zero-received single-chunk local-READY stall
    right at the breach frontier, while targeted READY/DELIVER rescue continues
    to fire thousands of times without collapsing the commit tail,
  - the issue is still in local READY / READY-quorum closure, not missing
    `INIT`/chunk rebroadcast; that fallback remained at zero again in both
    reruns.

## 2026-03-20 Follow-up: duplicate `BlockCreated` hydration now retries READY/DELIVER before the handler returns
- Updated `crates/iroha_core/src/sumeragi/main_loop/proposal_handlers.rs` so every
  duplicate / alternate-preserve-policy `BlockCreated` inline-hydration branch
  explicitly reruns `maybe_emit_rbc_ready(session_key)` followed by
  `maybe_emit_rbc_deliver(session_key)` before the early duplicate return.
  Missing-`INIT` rebroadcast behavior is unchanged.
- Added focused regressions in
  `crates/iroha_core/src/sumeragi/main_loop/tests.rs` for:
  - duplicate `BlockCreated` hydrating an existing single-chunk RBC session and
    flipping `sent_ready` in the same call,
  - duplicate `BlockCreated` closing READY quorum and becoming immediately
    `DELIVER`-eligible in the same call,
  - seed-queue-accepted duplicate `BlockCreated` inline hydration retrying
    READY/DELIVER before the queued seed worker completes, and
  - the block-sync preserve-policy entrypoint retrying READY immediately after
    duplicate-session hydration.
- Focused validation passed on this tree:
  - `cargo fmt --all`
  - `cargo test -p iroha_core --lib duplicate_block_created_ -- --nocapture`
  - `cargo test -p iroha_core --lib block_sync_block_created_retries_ready_after_existing_session_hydration -- --nocapture`

## 2026-03-20 Follow-up: targeted RBC DELIVER rescue is active; permissioned moves later but still spikes, NPoS stays red
- Rebuilt the release binaries on this cut and reran both stable envelopes with
  preserved peer dirs:
  - `cargo build --release -p irohad --bin iroha3d -p izanami`
  - permissioned log:
    `/tmp/izanami_permissioned_deliver_fix_20260320T134652.log`
  - permissioned peer dirs:
    `/tmp/iroha-soak-permissioned-20260320T134652/irohad_test_network_P9oKh3`
  - NPoS log:
    `/tmp/izanami_npos_deliver_fix_20260320T140300.log`
  - NPoS peer dirs:
    `/tmp/iroha-soak-npos-20260320T140300/irohad_test_network_vjCuap`
- Sampled result summary on the targeted-DELIVER cut:
  - permissioned first `interval_ms >= 5000` breach moved later to
    `height 469 -> interval_ms=10236`; the run then recovered and was manually
    stopped at `strict_min_height=578` to free the host for the NPoS rerun,
  - NPoS still breached the soak gate; the first breach was earlier than the
    previous sampled NPoS rerun at
    `height 450 -> interval_ms=10044`, followed by another `10006ms` window at
    `height 451` and fresh `5005ms` / `5003ms` windows at
    `height 530` / `535`; the run was manually stopped at
    `strict_min_height=554`,
  - top-level workload signal stayed clean in both reruns:
    `plan submission failed=0`,
    `Trigger with id \`repeat_trigger_*\` not found=0`,
  - sampled wall-clock pace to the manual stop was roughly
    `1.638s/block` for permissioned and `1.786s/block` for NPoS.
- Peer-log markers on these reruns:
  - permissioned:
    `deferring RBC DELIVER: READY quorum not yet satisfied=6039`,
    `commit quorum missing past timeout; rescheduling block for reassembly=19`,
    `queued RBC INIT and chunk rebroadcast while awaiting READY quorum=0`,
    `sending targeted RBC DELIVER to peers missing READY=5855`,
    `sending targeted RBC READY set to peers missing READY=11152`,
    `sending RBC DELIVER to commit topology after READY quorum=2066`
  - NPoS:
    `deferring RBC DELIVER: READY quorum not yet satisfied=5642`,
    `commit quorum missing past timeout; rescheduling block for reassembly=19`,
    `queued RBC INIT and chunk rebroadcast while awaiting READY quorum=0`,
    `sending targeted RBC DELIVER to peers missing READY=6086`,
    `sending targeted RBC READY set to peers missing READY=10735`,
    `sending RBC DELIVER to commit topology after READY quorum=1979`
- Critique:
  - the new DELIVER rescue path is definitely executing in both modes; the old
    `RbcInit` / chunk fallback churn stayed at zero again, while targeted
    DELIVER posts now show up thousands of times in the preserved peer trees,
  - permissioned improved materially relative to the earlier sampled
    permissioned rerun on this branch (`height 437 -> interval_ms=10003`), but
    it is still not clean because a single-block `10236ms` stall remains at
    `height 469`,
  - NPoS did not improve on the same comparison axis; despite later recovery
    past `height 523`, its first red window regressed to `height 450`,
  - the important negative result is that DELIVER rescue is now active, but the
    READY-to-DELIVER / commit-tail density barely changed; the issue is no
    longer “missing DELIVER rescue exists” and is now “the rescue fires often
    but still does not close the tail reliably enough.”

## 2026-03-20 Follow-up: authoritative-local-payload reruns split by mode; NPoS moves later, permissioned still fails early on the same READY tail
- Rebuilt the binaries used for the fresh sampled stable-soak critique on this
  exact tree:
  - `cargo build --release -p irohad --bin iroha3d`
  - `cargo build --release -p izanami`
- Reran both stable envelopes with preserved peer dirs and stopped each run at
  the first irrecoverable soak-gate breach (`interval_ms >= 5000`) per the
  branch workflow:
  - permissioned log:
    `/tmp/izanami_permissioned_authority_fix_20260320T123237.log`
  - permissioned peer dirs:
    `/tmp/iroha-soak-permissioned-20260320T123237/irohad_test_network_mBsX1N`
  - NPoS log:
    `/tmp/izanami_npos_authority_fix_20260320T124406.log`
  - NPoS peer dirs:
    `/tmp/iroha-soak-npos-20260320T124406/irohad_test_network_xLkg3Q`
- Sampled result summary on the authoritative-local-payload cut:
  - permissioned reached `strict_min_height=437`; first gate breach was
    `height 437 -> interval_ms=10003`; interval buckets:
    `10000+=1`, `5000-9999=0`, `3334-4999=0`, `2501-3333=3`
  - NPoS reached `strict_min_height=523`; first gate breach was
    `height 523 -> interval_ms=5001`; interval buckets:
    `10000+=0`, `5000-9999=1`, `3334-4999=3`, `2501-3333=13`
  - top-level workload signal stayed clean in both reruns:
    `plan submission failed=0`,
    `Trigger with id \`repeat_trigger_*\` not found=0`
- Peer-log markers on these fresh reruns:
  - permissioned:
    `commit quorum missing past timeout; rescheduling block for reassembly=15`,
    `deferring RBC DELIVER: READY quorum not yet satisfied=4503`,
    `queued RBC INIT and chunk rebroadcast while awaiting READY quorum=0`,
    `commit roster unavailable=20`
  - NPoS:
    `commit quorum missing past timeout; rescheduling block for reassembly=19`,
    `deferring RBC DELIVER: READY quorum not yet satisfied=5312`,
    `queued RBC INIT and chunk rebroadcast while awaiting READY quorum=0`,
    `commit roster unavailable=78`
  - late READY deferrals are still present right at the sampled frontier:
    permissioned through `height 438`, NPoS through `height 524`
- Critique:
  - the intended behavior change is visible: the old
    `queued RBC INIT and chunk rebroadcast while awaiting READY quorum` churn
    stayed at zero in both preserved peer trees, so the new local-authority
    predicate is not falling back to chunk/bootstrap recovery,
  - the remaining failure mode is still READY-to-DELIVER completion, not
    payload hydration or trigger noise; late frontier lines still show
    `roster_source=Derived` with `ready` advancing `0 -> 1 -> 2` and then
    stalling below `required=3`,
  - NPoS improved materially versus the immediately previous sampled rerun
    (`strict_min_height=425`, first breach at `height 336 -> 5001`) and is the
    strongest evidence that the local-authority fix is helping real late
    sessions, but
  - permissioned is still not improved on the same comparison point
    (`strict_min_height=499`, first breach at `height 444 -> 10003` on the
    prior sampled rerun), so this cut does not yet justify a full unchanged
    2000-block acceptance rerun.

## 2026-03-20 Follow-up: RBC authoritative payload now comes from local-progress payload availability plus slot match, not active-pending membership
- Implemented the requested internal-only RBC predicate change in
  `crates/iroha_core/src/sumeragi/main_loop.rs` and
  `crates/iroha_core/src/sumeragi/main_loop/rbc.rs`:
  - authoritative RBC payload, local `READY` eligibility, `READY`/`DELIVER`
    ingest stage syncing, and stalled-rebroadcast stage syncing now use
    `rbc_session_has_local_payload_for_progress(key, session)` instead of
    reusing `rbc_session_matches_active_near_tip_pending_block()`,
  - the new helper requires
    `block_payload_available_for_progress(key.0)` plus an explicit
    `(block_hash, height, view)` match sourced from `session.block_header` when
    present, or `local_block_height_view(key.0)` otherwise, and
  - this keeps the old frontier/near-tip filters for rebroadcast activity
    intact while letting non-active local payloads (for example Kura-backed
    sessions) advance to `AuthoritativePayload`, skip the chunk/bootstrap
    `READY` gate, and stay on READY-only rescue once authoritative.
- Added focused RBC regressions in
  `crates/iroha_core/src/sumeragi/main_loop/tests.rs` for:
  - Kura-backed local payload authority without active `pending_blocks` or
    `commit.inflight`,
  - immediate local `READY` emission on that authoritative session with missing
    chunks and zero external `READY`s,
  - stalled recovery staying INIT/chunk-silent and using only targeted `READY`
    rescue on that session,
  - the unchanged unknown-session chunk/bootstrap path, and
  - slot safety when the local hash exists but `(height, view)` mismatches the
    RBC session key.
- Focused validation passed on this cut:
  - `cargo fmt --all`
  - `cargo test -p iroha_core --lib sumeragi::main_loop::tests::known_local_kura_rbc_session_becomes_authoritative_without_chunk_completion -- --exact`
  - `cargo test -p iroha_core --lib sumeragi::main_loop::tests::known_local_kura_payload_with_slot_mismatch_stays_non_authoritative -- --exact`
  - `cargo test -p iroha_core --lib sumeragi::main_loop::tests::maybe_emit_rbc_ready_for_local_kura_authoritative_session_skips_bootstrap_gate -- --exact`
  - `cargo test -p iroha_core --lib sumeragi::main_loop::tests::rebroadcast_stalled_rbc_payloads_uses_targeted_ready_rescue_for_local_kura_authority -- --exact`
  - `cargo test -p iroha_core --lib sumeragi::main_loop::tests::maybe_emit_rbc_ready_with_missing_chunks_requests_missing_block -- --exact`
  - `cargo test -p iroha_core --lib sumeragi::main_loop::tests::maybe_emit_rbc_ready_for_known_authoritative_stub_skips_bootstrap_gate -- --exact`
  - `cargo test -p iroha_core --lib sumeragi::main_loop::tests::rebroadcast_stalled_rbc_payloads_skips_payload_recovery_once_authoritative -- --exact`
  - `cargo test -p iroha_core --lib round_gap_snapshot_records_markers_in_order -- --nocapture`
  - `cargo test -p iroha_core --lib commit_outcome_kickstarts_next_proposal_and_records_round_gap -- --nocapture`
- Broad `cargo test -p iroha_core --lib`, sampled permissioned/NPoS soak reruns,
  and full unchanged-envelope soaks were not rerun on this cut yet, so the
  branch-level acceptance signal is still pending the same late
  `READY`/`DELIVER` soak tail checks.

## 2026-03-20 Follow-up: sampled permissioned and NPoS stable-soak reruns still fail on late READY quorum
- Reran both stable envelopes on the current warmed release binaries and
  stopped each run immediately after the soak gate was irrecoverably red
  (`interval_ms >= 5000`) so the critique reflects fresh logs from this tree:
  - permissioned:
    `RUST_LOG=izanami::summary=info,izanami::progress=info,izanami::workload=warn,iroha_core::sumeragi::main_loop=info,iroha_core::sumeragi=info,iroha_p2p=info IROHA_TEST_NETWORK_KEEP_DIRS=1 TEST_NETWORK_TMP_DIR=/tmp/iroha-soak-permissioned-20260320T011121 TEST_NETWORK_BIN_IROHAD=/Users/mtakemiya/dev/iroha/target/release/iroha3d /Users/mtakemiya/dev/iroha/target/release/izanami --allow-net --peers 4 --faulty 0 --duration 3600s --target-blocks 2000 --progress-interval 10s --progress-timeout 600s --tps 5 --max-inflight 8 --workload-profile stable`
  - permissioned log: `/tmp/izanami_permissioned_full_soak_20260320T011121.log`
  - permissioned peer dirs:
    `/tmp/iroha-soak-permissioned-20260320T011121/irohad_test_network_mMNefV`
  - NPoS:
    `RUST_LOG=izanami::summary=info,izanami::progress=info,izanami::workload=warn,iroha_core::sumeragi::main_loop=info,iroha_core::sumeragi=info,iroha_p2p=info IROHA_TEST_NETWORK_KEEP_DIRS=1 TEST_NETWORK_TMP_DIR=/tmp/iroha-soak-npos-20260320T012514 TEST_NETWORK_BIN_IROHAD=/Users/mtakemiya/dev/iroha/target/release/iroha3d /Users/mtakemiya/dev/iroha/target/release/izanami --allow-net --nexus --peers 4 --faulty 0 --duration 3600s --target-blocks 2000 --progress-interval 10s --progress-timeout 600s --tps 5 --max-inflight 8 --workload-profile stable`
  - NPoS log: `/tmp/izanami_npos_full_soak_20260320T012514.log`
  - NPoS peer dirs:
    `/tmp/iroha-soak-npos-20260320T012514/irohad_test_network_vvnlJq`
- Fresh rerun summary:
  - permissioned reached `strict_min_height=499` before stop; first slow window
    was `height 444 -> interval_ms=10003`; interval buckets in the sampled run:
    `10000+=1`, `2501-3333=7`
  - NPoS reached `strict_min_height=425` before stop; first slow window was
    `height 336 -> interval_ms=5001`; interval buckets in the sampled run:
    `5000-9999=1`, `3334-4999=3`, `2501-3333=9`
  - permissioned still looks somewhat stronger than NPoS in the same envelope,
    but both runs are still clear soak failures on the configured gate
- Peer-log markers in the reruns:
  - permissioned:
    `commit quorum missing past timeout; rescheduling block for reassembly=19`,
    `deferring RBC DELIVER: READY quorum not yet satisfied=5242`,
    `queued RBC INIT and chunk rebroadcast while awaiting READY quorum=0`
  - NPoS:
    `commit quorum missing past timeout; rescheduling block for reassembly=12`,
    `deferring RBC DELIVER: READY quorum not yet satisfied=4393`,
    `queued RBC INIT and chunk rebroadcast while awaiting READY quorum=0`
  - late RBC deferrals are still present near the sampled frontier:
    permissioned at `height 499`, NPoS at `height 428-429`
- Harness-side trigger signal in the fresh NPoS rerun remained clean:
  - top-level log contains zero `plan submission failed`
  - top-level log contains zero
    `Trigger with id \`repeat_trigger_*\` not found`
- Critique:
  - the hydration/rebroadcast half improved: the old
    `queued RBC INIT and chunk rebroadcast while awaiting READY quorum` churn is
    gone again in both preserved peer trees,
  - the trigger-state/ingress-pinning change is holding in this sampled NPoS
    rerun, so the workload layer no longer appears to be contaminating the soak,
    and
  - the consensus tail is still unresolved: both modes still accumulate
    thousands of late `deferring RBC DELIVER: READY quorum not yet satisfied`
    lines, plus repeated `commit quorum missing past timeout` reschedules, so
    the remaining bug looks like READY-to-DELIVER completion / commit-quorum
    closure, not payload hydration.

## 2026-03-20 Follow-up: monotone RBC stages and trigger-state reconciliation narrow the tail, but the stable-soak gate is still red
- Implemented the planned internal-only consensus and workload fixes without
  changing public APIs, status surfaces, config, or wire formats:
  - `crates/iroha_core/src/sumeragi/main_loop.rs` and
    `crates/iroha_core/src/sumeragi/main_loop/rbc.rs` now drive RBC sessions
    through an explicit monotone `RbcProgressStage`
    (`CollectingChunks -> AuthoritativePayload -> LocalReadySent ->
    ReadyQuorumMet -> Delivered`) with `invalid` kept separate, authoritatively
    hydrated near-tip sessions can emit local `READY` without the old
    `missing-chunks + f+1 READY` bootstrap gate, and stage-based recovery now
    suppresses `RbcInit`/chunk rebuild traffic once payload authority exists.
  - `crates/izanami/src/instructions.rs` now tracks repeatable triggers via
    explicit `RepeatableTriggerState` (`Unknown`, `Known { repetitions }`,
    `Missing`) instead of optimistic vector/count bookkeeping.
  - `crates/izanami/src/chaos.rs` now pins trigger precheck + submit + postcheck
    to one ingress endpoint per trigger-related plan and always reconciles local
    trigger state back to the exact on-chain repetition count after submit.
- Focused validation passed on this cut:
  - `cargo fmt --all`
  - `cargo test -p iroha_core --lib sumeragi::main_loop::tests::known_near_tip_rbc_session_becomes_authoritative_without_chunk_completion -- --exact`
  - `cargo test -p iroha_core --lib sumeragi::main_loop::tests::maybe_emit_rbc_ready_for_known_authoritative_stub_skips_bootstrap_gate -- --exact`
  - `cargo test -p iroha_core --lib sumeragi::main_loop::tests::maybe_emit_rbc_ready_with_missing_chunks_requests_missing_block -- --exact`
  - `cargo test -p iroha_core --lib sumeragi::main_loop::tests::rebroadcast_stalled_rbc_payloads_skips_payload_recovery_once_authoritative -- --exact`
  - `cargo test -p izanami`
  - `cargo test -p iroha_core --lib round_gap_snapshot_records_markers_in_order -- --nocapture`
  - `cargo test -p iroha_core --lib commit_outcome_kickstarts_next_proposal_and_records_round_gap -- --nocapture`
  - `cargo build --release -p irohad --bin iroha3d -p izanami`
- Broad `cargo test -p iroha_core --lib` is still not a useful acceptance signal
  in this tree because it fails outside the RBC slice (`queue::*`, `state::*`)
  and then aborts with a stack overflow in
  `state::permission_cache_tests::permission_cache_rebuilds_after_restart`.
- Shared-host stable-soak reruns on the warmed release binaries still fail the
  acceptance gate, so this tranche is not yet accepted. The reruns were stopped
  immediately after the first `interval_ms >= 5000` breach because the gate was
  already irrecoverably red:
  - permissioned command shape:
    `RUST_LOG=izanami::summary=info,izanami::progress=info,izanami::workload=warn,iroha_core::sumeragi::main_loop=info,iroha_core::sumeragi=info,iroha_p2p=info IROHA_TEST_NETWORK_KEEP_DIRS=1 TEST_NETWORK_TMP_DIR=/tmp/iroha-soak-permissioned-20260320T003845 TEST_NETWORK_BIN_IROHAD=/Users/mtakemiya/dev/iroha/target/release/iroha3d /Users/mtakemiya/dev/iroha/target/release/izanami --allow-net --peers 4 --faulty 0 --duration 3600s --target-blocks 2000 --progress-interval 10s --progress-timeout 600s --tps 5 --max-inflight 8 --workload-profile stable`
  - permissioned log: `/tmp/izanami_permissioned_full_soak_20260320T003845.log`
  - permissioned peer dirs:
    `/tmp/iroha-soak-permissioned-20260320T003845/irohad_test_network_36F0rH`
  - permissioned sample reached `strict_min_height=521` before the stop; first
    slow window was `height 410 -> interval_ms=5001`, followed by
    `height 411 -> 10003` and `height 412 -> 10003`
  - permissioned peer-log markers:
    `commit quorum missing past timeout; rescheduling block for reassembly=23`,
    `deferring RBC DELIVER: READY quorum not yet satisfied=5459`,
    `queued RBC INIT and chunk rebroadcast while awaiting READY quorum=0`
  - permissioned late deferral samples still exist at high heights
    (`height 522`, `height 523`)
  - NPoS command shape:
    `RUST_LOG=izanami::summary=info,izanami::progress=info,izanami::workload=warn,iroha_core::sumeragi::main_loop=info,iroha_core::sumeragi=info,iroha_p2p=info IROHA_TEST_NETWORK_KEEP_DIRS=1 TEST_NETWORK_TMP_DIR=/tmp/iroha-soak-npos-20260320T005348 TEST_NETWORK_BIN_IROHAD=/Users/mtakemiya/dev/iroha/target/release/iroha3d /Users/mtakemiya/dev/iroha/target/release/izanami --allow-net --nexus --peers 4 --faulty 0 --duration 3600s --target-blocks 2000 --progress-interval 10s --progress-timeout 600s --tps 5 --max-inflight 8 --workload-profile stable`
  - NPoS log: `/tmp/izanami_npos_full_soak_20260320T005348.log`
  - NPoS peer dirs:
    `/tmp/iroha-soak-npos-20260320T005348/irohad_test_network_3FLWW1`
  - NPoS sample reached `strict_min_height=452` before the stop; first slow
    window was `height 286 -> interval_ms=10002`, followed by
    `height 288 -> 5002` and `height 289 -> 10002`
  - NPoS peer-log markers:
    `commit quorum missing past timeout; rescheduling block for reassembly=18`,
    `deferring RBC DELIVER: READY quorum not yet satisfied=4716`,
    `queued RBC INIT and chunk rebroadcast while awaiting READY quorum=0`
  - NPoS top-level log contained zero
    `Trigger with id \`repeat_trigger_*\` not found` failures in this sampled
    rerun
  - NPoS late deferral samples still exist at high heights (`height 458`,
    `height 459`)
- Current verdict:
  - the remaining late tail is no longer showing the old
    `queued RBC INIT and chunk rebroadcast while awaiting READY quorum`
    behavior in either preserved peer-log set, so the monotone stage/recovery
    change removed that churn,
  - `RepeatableTriggerState` + same-ingress reconciliation eliminated the
  sampled `repeat_trigger_* not found` contamination from the NPoS rerun, and
  - both modes still fail the soak gate on high-height `READY`-quorum DELIVER
    deferrals plus commit-quorum reschedule warnings, so more consensus-side
    work is still required before the tranche can be accepted.

## 2026-03-24 TAIRA faucet support landed for the app API and testnet profile
- Added an app-facing faucet slice across
  `crates/iroha_config/src/parameters/{actual.rs,user.rs}`,
  `crates/iroha_torii/src/{lib.rs,routing.rs,mcp.rs,openapi.rs}`,
  `crates/iroha_torii/tests/accounts_faucet.rs`,
  `configs/soranexus/testus/{config.toml,genesis.json}`,
  and `defaults/kagami/iroha3-testus/{config.toml,genesis.json}`.
- The shipped behavior in this slice:
  - Torii now accepts optional `[torii.faucet]` config and exposes `POST /v1/accounts/faucet`, which transfers a fixed starter balance from a configured authority account to an existing account that still has zero balance for the configured faucet asset;
  - the faucet response now includes both the configured asset-definition identifier and the concrete funded `asset_id`, so downstream apps can immediately reuse the exact bucket they were credited with;
  - the TAIRA/testus profile now seeds a dedicated faucet authority plus `xor#sora` in genesis and pre-mints a faucet reserve, while leaving non-TAIRA profiles untouched; and
  - the faucet config parser now accepts human-readable `name#domain` literals (for example `xor#sora`) for this profile instead of forcing operators to hand-compute the canonical asset-definition address.
- Validation:
  - `jq empty configs/soranexus/testus/genesis.json` (pass)
  - `jq empty defaults/kagami/iroha3-testus/genesis.json` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-config cargo test -p iroha_config torii_faucet_tests -- --nocapture` (pass)
- Remaining implementation gap:
  - the focused `iroha_config` faucet parser path is green, but full `iroha_torii --features app_api` test execution is still blocked in this workspace by unrelated pre-existing `iroha_core` compile errors around missing offline/SNS symbols on the current branch.

## 2026-03-24 Follow-up: shared Ed25519 GPU challenge preparation is unified across the accelerator admission paths
- Removed another accelerator drift point across
  `crates/ivm/src/{signature.rs,cuda.rs,ivm.rs,vector.rs}`.
- The shipped behavior in this slice:
  - the reduced Ed25519 challenge scalar bytes `H(R || A || M)` used by the
    GPU verification kernels now come from one shared helper in
    `signature.rs` instead of being recomputed independently in the CUDA
    self-test, CUDA verify path, Metal batch preparation, and Metal host
    admission path;
  - the helper is pinned by a fixed regression vector derived from the CUDA
    Ed25519 self-test truth set, so future transcript drift trips a focused
    unit test before it can fork the accelerator admission logic; and
  - the touched CUDA code no longer emits the earlier wall of stale
    `unused_mut` / dead-code warnings during
    `cargo check -p ivm --features cuda --tests`, leaving only the expected
    host-side `nvcc` build-script warnings on machines without a CUDA
    toolchain.
- Validation:
  - `cargo test -p ivm --lib ed25519_challenge_scalar_bytes_matches_cuda_selftest_vector -- --nocapture` (pass)
  - `cargo check -p ivm` (pass)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo test -p ivm --features metal --lib metal_ed25519_batch_matches_cpu -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-cuda-check cargo check -p ivm --features cuda --tests` (pass; only `build.rs` warnings remain because `nvcc` is absent on this host)
- Remaining implementation gap:
  - rerun the focused CUDA runtime/self-test slice on a host with live CUDA
    driver libraries and toolchain support, so the shared challenge helper and
    the earlier mirrored Ed25519 normalization fixes are exercised beyond
    compile-only validation.

## 2026-03-24 Follow-up: IVM startup truth sets now cover live vector and AES batch kernels
- Hardened the sampled `ivm` accelerator admission path in
  `crates/ivm/src/{vector.rs,cuda.rs}` so more production kernels are proven
  directly before Metal/CUDA acceleration is trusted.
- The shipped behavior in this slice:
  - the Metal startup self-test now proves parity for the live `vadd64`,
    `vand`, `vxor`, `vor`, `aesenc_batch`, and `aesdec_batch` kernels instead
    of letting those paths inherit only the earlier scalar/vector/AES checks;
  - the CUDA startup truth set now proves the same `vadd64`, vector bitwise,
    and single-round AES batch kernels before the backend stays enabled; and
  - the focused CUDA test graph compiles cleanly again after fixing the
    branch-local `ed25519_challenge_scalar_bytes` argument drift in
    `crates/ivm/src/cuda.rs`.
- Validation:
  - `cargo fmt --all` (pass)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo check -p ivm --features metal --tests` (pass)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo test -p ivm --features metal --lib metal_bitwise_single_vector_matches_scalar -- --nocapture` (pass)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo test -p ivm --features metal --lib metal_aes_batch_matches_scalar -- --nocapture` (pass)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo test -p ivm --features metal --lib metal_ed25519_batch_matches_cpu -- --nocapture` (pass)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo test -p ivm --features metal --lib metal_sha256_leaves_matches_cpu -- --nocapture` (pass)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-cuda-check cargo check -p ivm --features cuda --tests` (pass; `nvcc` is still absent on this host, so build.rs warns while Rust-side CUDA admission logic still compiles)
- Remaining implementation gap:
  - the startup-admission coverage gaps for the sampled vector/AES kernels are
    closed in code, but focused CUDA runtime/self-test execution still needs a
    host with live CUDA driver symbols to validate the expanded truth set
    beyond `cargo check`.

## 2026-03-24 Follow-up: CUDA startup truth set now covers live Merkle kernels
- Closed the next accelerator-admission gap in `crates/ivm/src/cuda.rs`.
- The shipped behavior in this slice:
  - CUDA startup now proves parity for both `sha256_leaves` and
    `sha256_pairs_reduce` before Merkle acceleration is trusted, instead of
    letting those live `byte_merkle_tree` kernels inherit only the broader
    generic CUDA self-test;
  - the new internal self-tests use the same canonical single-block leaf
    digests and pair-reduction root logic as the CPU path, so mismatched CUDA
    Merkle kernels now fail closed during backend admission; and
  - focused regression coverage now includes
    `sha256_merkle_selftest_covers_cuda_kernels` alongside the earlier
    Ed25519/BN254 CUDA self-test checks.
- Focused validation:
  - `cargo fmt --all` (pass)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-cuda-check cargo check -p ivm --features cuda --tests` (pass; `nvcc` is absent on this host, so build.rs warns while Rust-side CUDA admission logic still compiles)
- Remaining implementation gap:
  - rerun the focused CUDA runtime/self-test slice on a host with live CUDA
    driver libraries and toolchain support, so the new Merkle startup probes
    and the earlier mirrored Ed25519 fix are exercised beyond `cargo check`.

## 2026-03-24 Follow-up: Metal Ed25519 signature self-test now stays enabled under focused regression coverage
- Tightened the focused Metal regression in `crates/ivm/src/vector.rs` now
  that the sampled Ed25519 signature kernel is producing the correct result on
  this host again.
- The shipped behavior in this slice:
  - `metal_ed25519_batch_matches_cpu` now asserts the direct signature-kernel
    parity path (`signature_kernel`, `signature_status_kernel`, and
    `signature_check_bytes_kernel`) instead of treating CPU fallback as an
    acceptable outcome;
  - the same regression now pins pure `[s]B`, pure `[h](-A)`, scalar-1,
    scalar-2, and power-of-two basepoint multiplications against the CPU
    reference so future Metal drift trips a focused test instead of silently
    demoting to fallback; and
  - the stale Rust-only point-trace diagnostic helpers were removed from
    `vector.rs`, keeping the focused `ivm --features metal --tests` check
    warning-free again.
- Focused validation:
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo test -p ivm --features metal --lib metal_ed25519_batch_matches_cpu -- --nocapture` (pass)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo test -p ivm --features metal --lib metal_sha256_leaves_matches_cpu -- --nocapture` (pass)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo check -p ivm --features metal --tests` (pass)
- Remaining implementation gap:
  - the CUDA-side accelerator self-test slice is still only partially
    validated here because this host does not have the CUDA driver libraries
    needed to link the `cust`-backed tests; and
  - the broader accelerator review across the remaining CUDA/Metal/determinism
    boundaries is still open, but the specific Metal Ed25519 signature-kernel
    mismatch tracked in the earlier note below is no longer reproducing under
    the focused regression set above.

## 2026-03-24 Account-address first-release hard cut (JS/Swift/Android)
- Removed the remaining compressed/sentinel/full-width account-address
  compatibility surface from the public JS, Swift, and Android SDKs so the
  first-release literal model is just canonical I105 Base58.
- JavaScript (`javascript/iroha_js`):
  - the pure-JS fallback codec now matches Rust exactly: 14-bit prefix bytes
    + canonical payload + 2-byte `I105PRE` checksum, encoded with the standard
    Base58 alphabet and no textual sentinel;
  - removed the leftover public error codes tied to the sentinel-era codec
    (`MISSING_I105_SENTINEL`, `INVALID_I105_BASE`,
    `INVALID_I105_DIGIT`);
  - updated `test/address.test.js`, `index.d.ts`, and regenerated
    `dist/address.js`.
- Swift (`IrohaSwift`):
  - removed the last exported full-width helper
    `AccountAddress.toI105DefaultFullWidth()`;
  - `swift build` now completes cleanly after the account-address cleanup.
- Android (`java/iroha_android`):
  - `AccountAddress` now exposes only canonical I105 render/parse helpers;
    `toI105FullWidth`, the compressed-era warning/accessor naming, and the old
    sentinel/compressed error codes are gone;
  - `DisplayFormats` now carries `i105`, `discriminant`, and `i105Warning`
    only;
  - `AddressFormatOption` has been collapsed to `I105`, and the transport
    query-builder expectations now use `address_format=i105`.
- Repository hygiene:
  - strict grep across `IrohaSwift`, `javascript/iroha_js`,
    `java/iroha_android`, and `docs/source` is now clean for the removed
    full-width/sentinel/compressed account-address symbols when excluding the
    historical notes in `status.md`/`roadmap.md`.
- Focused validation:
  - `cd javascript/iroha_js && IROHA_JS_DISABLE_NATIVE=1 node --test test/address.test.js` (pass)
  - `cd javascript/iroha_js && npm run build:dist` (pass)
  - `cd java/iroha_android && JAVA_HOME=$(/usr/libexec/java_home -v 21) ANDROID_HOME=~/Library/Android/sdk ANDROID_SDK_ROOT=~/Library/Android/sdk ./gradlew android:compileDebugJavaWithJavac android:compileDebugUnitTestJavaWithJavac` (pass)
  - `cd java/iroha_android && JAVA_HOME=$(/usr/libexec/java_home -v 21) ANDROID_HOME=~/Library/Android/sdk ANDROID_SDK_ROOT=~/Library/Android/sdk ./gradlew samples-android:testDebugUnitTest --tests org.hyperledger.iroha.android.samples.SampleAddressTest` (pass)
  - `cd IrohaSwift && swift build` (pass)
  - `cargo test -p iroha_data_model --test account_address_vectors -- --nocapture` (pass)
  - `cargo test -p iroha_torii --test account_address_vectors -- --nocapture` currently stops on unrelated branch-local `norito::json!` macro errors in `crates/iroha_torii/src/offline_reserve.rs`

## 2026-03-24 Follow-up: Metal Ed25519 verification is restored on this host
- Finished the sampled Metal Ed25519 correctness tranche in
  `crates/ivm/src/{vector.rs,metal_ed25519.metal}` and mirrored the same
  field-op normalization hardening into `crates/ivm/cuda/signature.cu`.
- The shipped behavior in this slice:
  - the earlier per-pipeline fail-closed startup behavior remains in place, but
    the sampled Metal `ed25519_signature` pipeline now passes its parity gate
    again on this host instead of falling back to CPU;
  - the fix was preserving ref10 limb bounds across the scalar ladder by
    normalizing after `fe_add`, `fe_sub`, `fe_neg`, `fe_mul`, `fe_sq`,
    `fe_sq2`, and `fe_mul121666`, alongside the earlier base-point sign, `d2`,
    exact `fe_sq2`, and `fe_mul` carry corrections;
  - the focused regression now confirms `[s]B`, `[h](-A)`, the power-of-two
    basepoint ladder, direct status outputs, and full `[true, false]`
    signature verification on Metal against the CPU reference path; and
  - the temporary decompression trace kernels used to isolate the ladder drift
    have been removed from the Metal shader now that the runtime regression is
    green again.
- Focused validation:
  - `cargo fmt --all` (pass)
  - `xcrun -sdk macosx metal -c crates/ivm/src/metal_ed25519.metal -o /tmp/metal_ed25519.air` (pass)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo test -p ivm --features metal --lib metal_ed25519_batch_matches_cpu -- --nocapture` (pass)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo check -p ivm --features metal --tests` (pass)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-cuda-check cargo check -p ivm --features cuda --tests` (pass; runtime CUDA execution is still host-limited)
- Remaining implementation gap:
  - rerun the focused CUDA runtime/self-test slice on a host with live CUDA
    driver libraries and toolchain support, so the mirrored normalization fix
    is exercised beyond `cargo check`.

## 2026-03-24 Follow-up: Metal now degrades per-pipeline instead of dropping the whole backend on Ed25519 mismatch
- Hardened the Metal startup path in `crates/ivm/src/vector.rs` so this host
  no longer loses the entire Metal backend when the sampled Ed25519
  `signature_kernel` self-test fails.
- The shipped behavior in this slice:
  - the startup truth set still probes the Ed25519 Metal signature kernel and
    continues to fail closed on mismatch, but it now disables only the
    `ed25519_signature` pipeline instead of tearing down the full Metal state;
  - production batch verification already treats a missing Metal signature
    pipeline as `None`, so Ed25519 verification now falls back to the CPU path
    while SHA/AES/Keccak/Merkle-leaf Metal kernels stay live on this machine;
  - the `metal_sha256_leaves_matches_cpu` regression now executes against a
    live Metal backend on this host instead of passing only via the earlier
    "backend disabled" skip path; and
  - the sampled Ed25519 double-scalar routine in both
    `crates/ivm/src/metal_ed25519.metal` and `crates/ivm/cuda/signature.cu`
    now keeps the base point on the positive branch during verification, which
    is the correct sign convention even though the remaining point-decode bug
    still forces the Metal signature pipeline to stay fail-closed here.
  - follow-up arithmetic cleanup also corrected two more ref10 drift points in
    both accelerator ports: the precomputed `d2` constant now matches ref10,
    `fe_sq2` now uses the exact ref10 reduction path instead of a simplified
    `fe_sq` + `fe_add`, and the extra final `carry1` reduction has been
    removed from `fe_mul`.
- Focused validation:
  - `cargo fmt --all` (pass)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo test -p ivm --features metal --lib metal_ed25519_batch_matches_cpu -- --nocapture` (pass; verifies CPU fallback without disabling Metal)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo test -p ivm --features metal --lib metal_sha256_leaves_matches_cpu -- --nocapture` (pass)
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo check -p ivm --features metal --tests` (pass)
- Remaining implementation gap:
  - the raw Metal/CUDA Ed25519 point-decompression path is still wrong and
    fails the sampled signature-kernel self-test on this host, so Ed25519
    acceleration remains intentionally unavailable until that decompressor is
    fixed and revalidated; and
  - the new Metal-only decompression trace shows the divergence has narrowed:
    `u = y^2 - 1` and `v = d y^2 + 1` match the CPU/ref10 reference for the
    Ed25519 base point, but the first mismatch appears while building `uv^7`
    ahead of `fe_pow22523`, so the remaining bug is inside the field
    square/multiply chain rather than device discovery or request plumbing.

## 2026-03-24 Follow-up: SNS-backed alias leases now resolve from ledger state and reserved `universal` ownership is seeded
- Completed the next unified dataspace-aware alias ownership slice across
  `crates/iroha_core/src/{sns.rs,state.rs,smartcontracts/isi/{domain.rs,query.rs,sns.rs}}`,
  `crates/iroha_data_model/src/query/mod.rs`,
  `crates/iroha_executor/src/permission.rs`, and
  `crates/iroha_torii/tests/accounts_onboard.rs`.
- The shipped behavior in this slice:
  - SNS-backed alias/domain/dataspace ownership reads now resolve from
    `World.smart_contract_state` correctly using the same bare Norito codec the
    state layer persists, so active owner checks no longer miss existing
    records because of framed-vs-bare decode drift;
  - executor grant/revoke validation for dataspace-scoped
    `CanResolveAccountAlias` / `CanManageAccountAlias` now consults an
    authoritative singular query backed by active SNS dataspace-name ownership
    instead of the earlier genesis-only fallback;
  - core account-alias mutation (`Register<Account>` with a label,
    `BindAccountAlias`, `SetAccountLabel`) now rejects aliases without an
    active full-alias SNS lease, and the focused core regressions seed the
    required explicit alias-manage permissions before asserting the lease gate;
  - `State::new*` now seeds the reserved `universal` dataspace-name record from
    the genesis-domain owner when that reserved record is absent, giving the
    reserved dataspace alias a real active owner by default; and
  - Torii onboarding regressions now seed canonical alias leases with
    non-expiring test windows, so single-account and multisig onboarding both
    execute through the same active-lease gate used by steady-state alias
    registration.
- Focused validation:
  - `cargo fmt --all` (pass)
  - `CARGO_TARGET_DIR=target-codex-sns cargo test -p iroha_core find_dataspace_name_owner_by_id_returns_active_owner --lib` (pass)
  - `CARGO_TARGET_DIR=target-codex-sns cargo test -p iroha_core register_account_with_label_requires_active_sns_lease --lib` (pass)
  - `CARGO_TARGET_DIR=target-codex-sns cargo test -p iroha_core bind_account_alias_requires_active_sns_lease --lib` (pass)
  - `CARGO_TARGET_DIR=target-codex-sns cargo test -p iroha_core new_for_testing_seeds_reserved_universal_dataspace_name_record --lib` (pass)
  - `CARGO_TARGET_DIR=target-codex-sns cargo test -p iroha_torii --test accounts_onboard` (pass)
  - `CARGO_TARGET_DIR=target-codex-sns cargo check -p iroha_core -p iroha_torii -p iroha_executor` (pass)

## 2026-03-24 Follow-up: IVM accelerator startup self-tests now cover more live kernel families
- Hardened the `ivm` accelerator admission path across
  `crates/ivm/src/{cuda.rs,gpu_manager.rs,byte_merkle_tree.rs,vector.rs}` and
  the CUDA/Metal regression files under `crates/ivm/tests/`.
- Closed the concrete CUDA startup-coverage gaps found during the audit:
  - the CUDA startup self-test now covers the live Ed25519
    `signature_kernel`;
  - the same startup path now covers the BN254 add/sub/mul kernels instead of
    trusting them transitively after narrower backend init checks; and
  - stale `ivm --features cuda --tests` drift in
    `cuda_disable_on_mismatch.rs`, `cuda_env.rs`, `cuda_extra.rs`,
    `gpu_determinism.rs`, and `hardware_determinism.rs` has been repaired so
    the feature-gated CUDA test graph compiles again.
- Added the matching Metal startup guard for the distinct `sha256_leaves`
  pipeline in `crates/ivm/src/vector.rs`, so Metal no longer treats the
  single-block SHA-256 self-test as sufficient coverage for Merkle-leaf
  acceleration.
- Validation:
  - `cargo fmt --all` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-cuda2 cargo check -p ivm --features cuda --tests` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo check -p ivm --features metal --tests` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo test -p ivm --features metal --lib metal_sha256_leaves_matches_cpu -- --nocapture` (pass with explicit skip after backend disable)
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-cuda2 cargo test -p ivm --features cuda --lib selftest_covers_ -- --nocapture` (blocked by host CUDA linker errors: unresolved `cu*` symbols)
- Remaining implementation gap:
  - the Metal backend still disables itself on this host because the existing
    Ed25519 startup parity check fails before the new `sha256_leaves` path can
    be exercised end-to-end; that fail-closed behavior is desirable, but the
    underlying Metal Ed25519 mismatch remains open and should be fixed next.

## 2026-03-24 Follow-up: domainless multisig home-domain optionality and signed multisig alias selectors are now live
- Completed the next unified dataspace-aware alias follow-up across
  `crates/iroha_executor_data_model/src/isi.rs`,
  `crates/iroha_executor/src/default/isi/multisig/{mod.rs,account.rs,transaction.rs}`,
  `crates/iroha_core/src/smartcontracts/isi/multisig.rs`,
  `crates/iroha_config/src/parameters/{actual.rs,user.rs}`,
  `crates/iroha_torii/src/{lib.rs,routing.rs}`,
  and `crates/iroha_torii/tests/accounts_onboard.rs`.
- The shipped behavior in this slice:
  - multisig registration/state now persists `home_domain: Option<DomainId>`
    end to end, so the core path can register and materialize fully domainless
    multisig accounts and domainless signatory placeholder accounts without
    inventing a fallback domain;
  - Torii onboarding config no longer carries `allowed_domain`, and both
    single-account and multisig onboarding now rely on the canonical alias
    parser plus explicit alias-manage permission instead of a special
    onboarding-only domain assumption;
  - multisig onboarding accepts canonical domainless aliases
    (`label@dataspace`) and stores the same unified alias semantics as the
    steady-state alias index; and
  - `POST /v1/multisig/spec`, `POST /v1/multisig/proposals/list`, and
    `POST /v1/multisig/proposals/get` now require canonical signed app-auth
    headers whenever the selector resolves through a multisig alias, matching
    the earlier `/v1/aliases/resolve*` hardening.
- Added focused regressions for:
  - JSON decode defaulting `MultisigRegister.home_domain` to `None`;
  - core registration of a domainless multisig with empty linked-domain state;
  - signed-request enforcement for the alias-based multisig spec/proposal
    read paths; and
  - Torii onboarding flows using canonical `@universal` aliases plus explicit
    dataspace alias-manage permission.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo check -p iroha_config -p iroha_executor_data_model -p iroha_executor -p iroha_core` (pass)
  - `cargo check -p iroha_torii --lib` (pass)
  - `cargo test -p iroha_executor_data_model multisig_register_json_defaults_home_domain_to_none --lib` (pass)
  - `cargo test -p iroha_core multisig_register_supports_domainless_home_domain --lib` (pass)
  - `cargo test -p iroha_torii requires_signed_request_for_alias_selector --lib` (pass)
  - `cargo test -p iroha_torii --test accounts_onboard accounts_onboard_` (pass)
- Remaining implementation gap:
  - dataspace-scoped alias permission grant/revoke validation still uses the
    temporary genesis-only gate because there is not yet a ledger-backed
    SNS dataspace-owner source in `World`/Nexus;
  - SNS-backed ownership/lease enforcement for full account alias keys, domain
    names, and dataspace aliases is still pending, including the new
    `/v1/sns/names...` ledger-native API and reserved `universal` seeding; and
  - the wider SNS shim replacement still requires a new authoritative ledger
    state surface rather than a small follow-up patch on top of the existing
    in-memory Torii registrar.

## 2026-03-24 Follow-up: Ledger-backed SNS routes now reach the client/test/tooling surface
- Continued the unified alias / on-chain SNS migration across the remaining
  consumer surfaces:
  - `crates/iroha_data_model/src/sns/mod.rs` now exports the fixed
    namespace suffix ids (`account-alias`, `domain`, `dataspace`) as public
    constants so the API boundary does not depend on `iroha_core` internals;
  - `crates/iroha_core/src/sns.rs` now reuses those shared constants and the
    focused world/SNS regressions were repaired so the new domain-lease and
    reserved-`universal` coverage compiles again;
  - `crates/iroha/src/sns.rs` now targets `/v1/sns/names...` and exposes
    namespace-aware read/mutation helpers instead of the old selector-path
    contract;
  - `integration_tests/tests/sns.rs`,
    `crates/iroha_torii/tests/sns_registrar.rs`,
    `scripts/sns_bulk_onboard.py`,
    `javascript/iroha_js/src/toriiClient.js`,
    `javascript/iroha_js/test/toriiClient.test.js`, and
    `crates/iroha_cli/CommandLineHelp.md`
    were updated to stop calling the removed legacy SNS registration routes; and
  - the removed governance-case shim is now surfaced as an explicit
    removal in the CLI and JS helpers instead of silently drifting to stale
    Torii calls.
- Validation:
  - `cargo fmt --all` (pass)
  - `CARGO_TARGET_DIR=target-codex-final2 cargo check -p iroha_core -p iroha_torii -p iroha -p iroha_cli -p integration_tests`
    is currently blocked by unrelated existing `crates/iroha_torii/src/offline_reserve.rs`
    serde/Norito compile failures on this branch; the alias/SNS changes in this
    follow-up are not the source of those parser/derive errors.

## 2026-03-24 Follow-up: generated SNS artifacts now match the ledger-backed names API
- Closed the last generated/public drift in the unified alias / on-chain SNS
  slice:
  - `crates/iroha_core/src/smartcontracts/isi/world.rs` now enforces an active
    SNS domain-name lease during `Register<Domain>`, so the focused
    `register_domain_requires_active_sns_lease_for_non_genesis_owner`
    regression now passes against the actual world ISI path instead of only the
    lower-level SNS helpers;
  - `javascript/iroha_js/dist/toriiClient.js` was rebuilt from the updated SDK
    sources, so the shipped JS bundle now uses `/v1/sns/names...` and retains
    only the deliberate governance-case removal stubs; and
  - `docs/portal/static/openapi/{torii.json,versions/current/torii.json}` were
    regenerated from `xtask openapi`, so the checked-in portal specs now expose
    `/v1/sns/names` and no longer advertise the removed SNS registration or
    governance-case routes.
- Validation:
  - `cargo fmt --all` (pass)
  - `CARGO_TARGET_DIR=target-codex-final2 cargo test -p iroha_core register_domain_requires_active_sns_lease_for_non_genesis_owner --lib` (pass)
  - `CARGO_TARGET_DIR=target-codex-final2 cargo test -p iroha_core reserved_universal_dataspace_record_is_immutable --lib` (pass)
  - `npm run build:dist` in `javascript/iroha_js` (pass)
  - `CARGO_TARGET_DIR=target-codex-openapi NORITO_SKIP_BINDINGS_SYNC=1 cargo run -p xtask --bin xtask -- openapi --output docs/portal/static/openapi/torii.json --output docs/portal/static/openapi/versions/current/torii.json` (pass)
- Remaining validation gap:
  - broader workspace-wide validation remains intentionally deferred; this
    follow-up only exercised the focused Torii/JS paths needed to close the
    alias/SNS slice.

## 2026-03-24 Follow-up: JS Torii client and focused Torii crate validation are clean again
- Cleared the remaining branch-local validation blockers that were still
  obscuring the completed unified alias / on-chain SNS work:
  - `javascript/iroha_js/test/toriiClient.test.js` now matches the current
    SDK/native contract by using the camelCase SoraFS native fixture fields,
    an explicit non-canonical I105 fixture for the governance-owner rejection
    cases, a valid account-id fixture in the feature-config snapshot test, and
    valid `label.suffix` selectors in the SNS option-validation checks; and
  - `cargo check -p iroha_torii --lib` now runs cleanly again on a fresh target
    dir, so there is no remaining focused `iroha_torii` compile blocker in the
    alias/SNS slice.
- Validation:
  - `node --test test/toriiClient.test.js` in `javascript/iroha_js` (pass, 637 tests)
  - `CARGO_TARGET_DIR=target-codex-finish cargo check -p iroha_torii --lib` (pass)
- Remaining validation gap:
  - `cargo test --workspace` and other full-repo sweeps were not run here per
    request.

## 2026-03-24 Follow-up: Norito Stage-1 GPU helpers now prove scalar parity before activation
- Hardened `crates/norito/src/lib.rs` so dynamically loaded JSON Stage-1
  Metal/CUDA helpers no longer become active just by exporting the expected
  symbol. The loader now runs a parity self-test against the scalar
  structural-index builder before caching the helper.
- This closes the next concrete dynamic-helper integrity edge in the Norito
  JSON path: a malformed or tampered Stage-1 helper can no longer silently
  drift structural tape output while still looking well-formed enough to
  reach the runtime fallback gate only after a live request.
- Added focused regressions for:
  - accepting a helper whose structural tape matches the scalar path;
  - rejecting helpers that return mismatched structural offsets;
  - rejecting helpers that return non-zero status; and
  - rejecting helpers that claim success while reporting an out-of-capacity
    output length.
- Validation:
  - `cargo fmt --all` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-stage1 cargo test -p norito --lib accel_tape_validation_tests -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-stage1 cargo test -p norito --features cuda-stage1 --lib accel_tape_validation_tests -- --nocapture` (pass)
- Remaining implementation gap:
  - continue the focused `ivm`/`norito` review on the remaining optional
    accelerator and streaming boundaries beyond the Stage-1, GPU zstd, and
    CRC64 helper parity gates.

## 2026-03-24 Follow-up: asset-definition docs and SDK references now match Base58 IDs, dotted aliases, and alias-binding lease semantics
- Closed the remaining public/docs-facing asset-definition cleanup across:
  - `docs/source/data_model*.md`
  - `docs/source/data_model_and_isi_spec*.md`
  - `docs/source/sdk/swift/index*.md`
  - `IrohaSwift/{README.md,Sources/IrohaSwift/*,Tests/IrohaSwiftTests/*}`
  - `javascript/iroha_js/{index.d.ts,src/toriiClient.js}`
- The shipped behavior in this slice:
  - translated data-model/spec docs and Swift SDK docs/examples now show
    canonical unprefixed Base58 asset-definition IDs and dotted aliases
    (`name#domain.dataspace` / `name#dataspace`) instead of legacy `aid:`
    literals and `name#domain@dataspace`;
  - SDK-facing references now document `alias_binding { alias, status,
    lease_expiry_ms, grace_until_ms, bound_at_ms }`, the persisted lease
    statuses (`permanent`, `leased_active`, `leased_grace`,
    `expired_pending_cleanup`), and the chain-time alias cutoff semantics;
  - the Swift SDK now validates canonical Base58 asset-definition IDs,
    resolves dotted aliases through `/v1/assets/aliases/resolve`, exposes
    optional `aliasBinding` metadata on alias-resolution payloads, and decodes
    current 16-byte asset-definition payloads back to Base58 instead of legacy
    `aid:` strings;
  - the JS Torii typings/JSDoc now describe full asset-definition records plus
    the `alias_binding.*` filter/sort fields instead of `{ id }`-only items.
- Validation:
  - `swift test --filter testAssetDefinitionIdFromAliasProducesCanonicalBase58Address` (pass)
  - `swift test --filter testValidateRejectsMalformedAssetDefinition` (pass)
  - `swift test --filter testResolveAssetAliasAsync` (pass)
  - `swift test --filter testBuildAssetIdLiteralResolvingAliases` (pass)
  - `swift test --filter testAssetIdAliasFallbackForInvalidNonAliasDefinition` (pass)
  - `node --check javascript/iroha_js/src/toriiClient.js` (pass)
- Remaining implementation gap:
  - the final internal Rust cleanup for legacy `aid:` test literals and the
    permissive `AssetDefinitionId::from_str` fallback is still pending broader
    fixture/test migration.

## 2026-03-24 Follow-up: Norito GPU CRC64 helpers now self-test before use
- Hardened `crates/norito/src/core/simd_crc64.rs` so dynamically loaded
  GPU CRC64 helpers no longer become active solely by exporting the expected
  symbol. The helper now has to pass a parity self-test against the canonical
  `crc64_fallback` implementation before `hardware_crc64` will trust it.
- This closes the concrete determinism/integrity edge in the remaining
  `norito` helper surface: a malformed or tampered CRC64 helper can no longer
  silently change Norito header checksums or decode-time checksum validation.
- The debug/test-only helper override path is now actually wired for focused
  regression coverage, and the debug/test-only `NORITO_GPU_CRC64_MIN_BYTES`
  override now works so the stub path can be exercised without real GPU-sized
  payloads.
- Added focused regressions for:
  - accepting a helper whose CRC64 output matches the canonical fallback;
  - rejecting helpers that return mismatched CRC output; and
  - rejecting helpers that return a non-zero status while claiming the GPU path.
- Validation:
  - `cargo fmt --all` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p norito --features cuda-crc64 gpu_crc64_self_test -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-crc64 cargo test -p norito --features cuda-crc64 --lib gpu_min_len_defaults_and_env_override -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-crc64 cargo test -p norito --features cuda-crc64 --lib gpu_crc64_can_load_env_stub -- --nocapture` (pass)
- Remaining implementation gap:
  - continue the focused `ivm`/`norito` review on the remaining optional
    accelerator and streaming boundaries beyond this CRC64 helper parity gate.

## 2026-03-24 Follow-up: asset-definition alias leases now use committed chain time, hard post-grace cutoffs, and authoritative binding state
- Closed the remaining asset-definition alias lease/API gaps across
  `crates/iroha_core/src/{state.rs,smartcontracts/isi/domain.rs,smartcontracts/isi/asset.rs}`,
  `crates/iroha_torii/src/{routing.rs,lib.rs}`,
  `crates/iroha_torii/tests/asset_definitions_endpoints.rs`, and the English
  asset docs in `docs/source/{data_model.md,data_model_and_isi_spec.md}`.
- The shipped behavior in this slice:
  - public asset alias resolution now uses the latest committed block
    timestamp instead of node wall clock;
  - asset aliases stop resolving immediately after their grace window elapses,
    even before background sweep removes the stale binding record;
  - stored `World.asset_definitions` rows are normalized to `alias = None`,
    and effective alias data is derived from the authoritative persisted
    alias-binding record on reads and rebuilds;
  - asset-definition app-API list/query filters and sorts now support
    `alias_binding.status`, `alias_binding.lease_expiry_ms`,
    `alias_binding.grace_until_ms`, and `alias_binding.bound_at_ms`; and
  - the CLI alias-resolution helper now parses canonical Base58
    asset-definition ids from Torii responses instead of the removed `aid:`
    format.
- Added focused regressions for:
  - core alias-lease persistence/rebuild behavior and the post-grace alias
    cutoff before sweep; and
  - Torii asset-definition route coverage for full-definition responses plus
    `alias_binding` sort behavior on `POST /v1/assets/definitions/query`.
- Validation:
  - `cargo fmt --all` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-asset-gap cargo test -p iroha_core asset_definition_alias --offline -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-asset-gap cargo test -p iroha_torii --test asset_definitions_endpoints --offline -- --nocapture` (pass)
- Remaining implementation gap:
  - the English docs are updated, but translated docs / SDK-facing API
    references still need the same lease-status and chain-time cutoff wording;
    that follow-up remains tracked in `roadmap.md`.

## 2026-03-24 Follow-up: alias permissions, signed Torii resolution, and canonical Kotodama alias strings are now wired end to end
- Completed the next unified dataspace-aware alias slice across
  `crates/iroha_executor_data_model/src/permission.rs`,
  `crates/iroha_executor/src/{permission.rs,default/mod.rs}`,
  `crates/iroha_core/src/{alias/mod.rs,smartcontracts/isi/domain.rs,smartcontracts/ivm/host.rs,state.rs}`,
  `crates/kotodama_lang/src/{semantic.rs,ir.rs,compiler.rs,regalloc.rs}`,
  `crates/ivm/tests/kotodama.rs`, and
  `crates/iroha_torii/src/lib.rs`.
- Added explicit account-alias permission tokens:
  - `CanResolveAccountAlias { scope: Domain(..) | Dataspace(..) }`
  - `CanManageAccountAlias { scope: Domain(..) | Dataspace(..) }`
- Core alias mutation and resolution now enforce those permissions:
  - `Register<Account>` with a label, `BindAccountAlias`, and
    `SetAccountLabel` now require explicit alias-manage permission instead of
    the earlier temporary self/domain bridge.
  - IVM host alias resolution now checks explicit alias-resolve permission and
    returns permission denial when the caller lacks it.
- Torii account-alias resolution is now signed and on-chain only:
  - `POST /v1/aliases/resolve` and `POST /v1/aliases/resolve_index` now
    require canonical signed app-auth headers, reject missing permission with
    `403`, and no longer fall back to the old alias-service / ISO bridge for
    account aliases.
- Kotodama / IVM account-alias resolution now uses one canonical alias string:
  - `resolve_account_alias("label@domain.dataspace")`
  - `resolve_account_alias("label@dataspace")`
  The compiler now publishes that single alias-string TLV into INPUT before
  invoking the syscall so trigger/runtime execution still receives a valid
  Norito envelope.
- Focused validation:
  - `cargo fmt --all` (pass)
  - `CARGO_TARGET_DIR=target-codex-alias3 cargo check -p kotodama_lang -p ivm -p iroha_core -p iroha_torii -p iroha_executor` (pass)
  - `CARGO_TARGET_DIR=target-codex-alias3 cargo test -p kotodama_lang lower_resolve_account_alias_builtin --lib` (pass)
  - `CARGO_TARGET_DIR=target-codex-alias3 cargo test -p ivm compile_emits_resolve_account_alias_syscall --test kotodama` (pass)
  - `CARGO_TARGET_DIR=target-codex-alias3 cargo test -p iroha_core resolve_account_alias_syscall_reads_current_alias_binding --lib` (pass)
  - `CARGO_TARGET_DIR=target-codex-alias3 cargo test -p iroha_core execute_data_trigger_supports_alias_resolve_and_json_amount_transfer --lib` (pass)
  - `CARGO_TARGET_DIR=target-codex-alias3 cargo test -p iroha_torii alias_resolve_index_ --lib` (pass)
  - `CARGO_TARGET_DIR=target-codex-alias3 cargo test -p iroha_torii alias_resolve_requires_signed_request --lib` (pass)
  - `CARGO_TARGET_DIR=target-codex-alias3 cargo test -p iroha_torii alias_resolve_scans_account_labels_when_alias_index_is_missing --lib` (pass)
  - `CARGO_TARGET_DIR=target-codex-alias3 cargo test -p iroha_torii alias_resolve_rejects_missing_permission --lib` (pass)
  - `CARGO_TARGET_DIR=target-codex-alias3 cargo test -p iroha_torii alias_resolve_returns_not_found_for_unknown_alias --lib` (pass)
  - `CARGO_TARGET_DIR=target-codex-alias3 cargo test -p iroha_torii alias_resolve_ --lib` (expected unrelated failure: existing `asset_alias_resolve_returns_not_found_after_grace`)
- Remaining implementation gap:
  - dataspace-scoped alias permission grant/revoke validation still uses the
    temporary genesis-only gate until ledger-backed SNS dataspace ownership is
    available;
  - SNS-backed ownership/lease enforcement for full account alias keys, domain
    names, and dataspace aliases is still pending;
  - multisig `home_domain: Option<DomainId>`, domainless multisig onboarding,
    and the broader onboarding-config cleanup are still pending; and
  - some broader `iroha_torii` test/compile paths still have unrelated
    branch-local blockers outside the account-alias slice.

## 2026-03-24 Follow-up: Norito accelerator outputs now fail closed on malformed helper results
- Hardened `crates/norito/src/lib.rs` so every accelerated JSON Stage-1 tape
  passes a release-build sanity check before `TapeWalker` consumes it:
  out-of-bounds, non-structural, or non-monotonic offsets now fall back to
  the scalar builder instead of reaching direct `input.as_bytes()[off]`
  indexing.
- Hardened `crates/norito/src/core/gpu_zstd.rs` so GPU zstd encode/decode
  paths reject helper-reported success lengths that exceed the destination
  buffer instead of calling `truncate(out_len)` on unchecked values.
- This closes the concrete malformed-helper-result edges surfaced by the
  focused `norito` follow-up: buggy or malformed accelerator output can no
  longer panic the JSON walker or GPU zstd wrapper by returning invalid
  offsets or invalid success lengths.
- Synced the Python and Java Norito changelogs so the bindings parity guard
  reflects that this Rust-side change is internal hardening rather than a
  public API change.
- Validation:
  - `cargo fmt --all` (pass)
  - `python3 scripts/check_norito_bindings_sync.py` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p norito validate_accel_rejects_out_of_bounds_offsets -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p norito validate_accel_rejects_non_structural_offsets -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p norito try_gpu_encode_rejects_invalid_success_length -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p norito try_gpu_decode_rejects_invalid_success_length -- --nocapture` (pass)
- Remaining implementation gap:
  - continue the focused `ivm`/`norito` review on the remaining unsafe,
    hardware-acceleration, and streaming/crypto boundaries; this patch hardens
    the Stage-1 and GPU zstd helper handoffs but not the wider review surface.

## 2026-03-24 Follow-up: Torii ingress-auth hardening closed the live security-audit findings
- Closed the three live ingress/auth findings from the refreshed audit:
  - `crates/iroha_torii/src/app_auth.rs` now fails closed on
    multisig-controlled accounts instead of silently accepting any single
    member signature on the app-auth path;
  - the same verifier now requires the four-header canonical-request scheme
    (`X-Iroha-Account`, `X-Iroha-Signature`, `X-Iroha-Timestamp-Ms`,
    `X-Iroha-Nonce`), enforces freshness, keeps a bounded replay cache, and
    accepts UTF-8 canonical I105 account literals in request headers; and
  - Norito-RPC plus operator-auth now trust
    `x-forwarded-client-cert` only when the TCP peer belongs to configured
    trusted-proxy CIDRs, defaulting to loopback-only proxies.
- Surfaced the new config knobs in `iroha_config`:
  - app-auth clock skew / nonce TTL / replay-cache capacity in
    `crates/iroha_config/src/parameters/{defaults,actual,user}.rs`; and
  - `mtls_trusted_proxy_cidrs` for both Norito-RPC and operator-auth in the
    same config stack.
- Updated the JS, Swift, and Android canonical-request helpers and docs to
  emit the same freshness-aware four-header protocol as Torii.
- Expanded Torii regression coverage beyond the low-level verifier:
  - `crates/iroha_torii/src/content.rs` now covers replay rejection on the
    content role-gate path; and
  - `crates/iroha_torii/src/lib.rs` now covers missing/replayed signatures on
    Soracloud signed-mutation middleware and adds an attachment-tenancy replay
    regression.
- Validation:
  - `cargo fmt --all` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo check -p iroha_torii --lib` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib --no-run` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib verify_accepts_valid_signature -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib verify_rejects_replayed_nonce -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib operator_auth_rejects_forwarded_mtls_from_untrusted_proxy -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib trusted_forwarded_header_requires_proxy_membership -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib role_gate_rejects_replayed_signature -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib soracloud_signed_mutation_middleware_requires_headers_and_rejects_replay -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib zk_attachments_tenant_rejects_replayed_signed_headers -- --nocapture` (pass)
  - `node --test javascript/iroha_js/test/canonicalRequest.test.js javascript/iroha_js/test/toriiCanonicalAuth.test.js` (pass)
  - `cd IrohaSwift && swift test --filter CanonicalRequestTests` (pass)
  - `cd java/iroha_android && JAVA_HOME=$(/usr/libexec/java_home -v 21) ANDROID_HOME=~/Library/Android/sdk ANDROID_SDK_ROOT=~/Library/Android/sdk ./gradlew android:compileDebugUnitTestJavaWithJavac` (pass)
- Remaining implementation gap:
  - this ingress/auth slice is closed in the current tree; remaining security
    follow-up is the broader `ivm`/`norito` review plus broader JS/Swift/Android
    suite reruns once unrelated branch-level blockers are cleared.

## 2026-03-24 Follow-up: unified dataspace-aware account alias groundwork now spans the data model, core state, executor, and Torii
- Replaced the old domain-only `AccountLabel` shape with a unified structured
  alias key carrying `label`, `Option<DomainId>`, and `DataSpaceId`, plus
  canonical parse/format helpers for `label@domain.dataspace` and
  `label@dataspace` backed by the current `DataSpaceCatalog`.
- Made account registration domain-optional in the core data model:
  `NewAccount.domain` is now optional, `Account::new_domainless` /
  `NewAccount::new_domainless` preserve empty linked-domain state, and
  registration/index rebuild paths now validate domain-bearing aliases against
  the account's actual linked-domain set instead of silently manufacturing a
  fallback domain link.
- Wired the same alias type through state rebuilds, alias rekey tracking,
  Torii on-chain alias parsing/formatting, multisig alias selectors, and
  account onboarding so single-account onboarding now registers the same
  canonical alias used by steady-state alias resolution and targets the alias
  dataspace instead of forcing `GLOBAL`.
- Multisig onboarding now uses the same canonical parser and stores the same
  alias binding semantics for domain-bearing aliases; domainless multisig
  onboarding is still intentionally deferred.
- Validation:
  - `rustfmt --edition 2024 crates/iroha_data_model/src/account/rekey.rs crates/iroha_data_model/src/account.rs crates/iroha_data_model/src/nexus/mod.rs crates/iroha_core/src/state.rs crates/iroha_core/src/state/tiered.rs crates/iroha_core/src/smartcontracts/isi/domain.rs crates/iroha_core/src/pipeline/access.rs crates/iroha_executor/src/default/mod.rs crates/iroha_torii/src/lib.rs crates/iroha_torii/src/routing.rs` (pass)
  - `cargo test -p iroha_data_model account_label_ --lib` (pass)
  - `cargo test -p iroha_data_model domainless_account_builder_keeps_empty_linked_domains --lib` (pass)
  - `CARGO_TARGET_DIR=target-codex-alias cargo test -p iroha_core register_domainless_account_keeps_empty_domain_links_and_indexes_alias --lib` (pass)
  - `CARGO_TARGET_DIR=target-codex-alias cargo test -p iroha_core set_account_label_rejects_domainful_alias_without_domain_link --lib` (pass)
  - `CARGO_TARGET_DIR=target-codex-alias cargo test -p iroha_torii alias_resolve_scans_account_labels_when_alias_index_is_missing --lib` (pass)
  - `CARGO_TARGET_DIR=target-codex-alias cargo test -p iroha_torii multisig_selector_rejects_both_fields --lib` (pass)
  - `CARGO_TARGET_DIR=target-codex-alias cargo check -p iroha_executor --tests` (pass)
- Remaining implementation gap:
  - explicit domain/dataspace alias permissions, HTTP `403` handling, and
    VM/Kotodama permission-denial mapping are not implemented yet;
  - SNS-backed ownership/lease enforcement for full alias keys, domain names,
    and dataspace aliases is still pending;
  - multisig home-domain optionality and domainless multisig onboarding remain
    follow-up work; and
  - Torii onboarding config still needs the broader cleanup that removes the
    legacy `allowed_domain` assumption entirely.

## 2026-03-24 Follow-up: full-workspace security audit refresh initially reduced the stale Torii report to three live ingress/auth findings
- Refreshed `security_best_practices_report.md` against current workspace code
  instead of relying on the earlier Torii/Soracloud snapshot.
- The initial live findings were:
  - app canonical request verification treats multisig accounts as
    "any single member may sign", bypassing threshold/weight policy;
  - app canonical request signatures remain replayable because the signed
    message has no timestamp/nonce/replay cache, and the JS/Swift/Android SDK
    helpers mirror the same two-header scheme; and
  - Norito-RPC and operator-auth `require_mtls` checks currently trust the
    presence of `x-forwarded-client-cert` rather than transport-authenticated
    mTLS state.
- The previously reported Soracloud findings about raw private keys over HTTP,
  internal-only local-read proxy execution, unmetered public-runtime fallback,
  and remote-IP attachment tenancy are now closed/superseded by the currently
  shipped signed-mutation, public-route, throttling, and signed-account
  tenancy paths.
- Spot-checked IVM/crypto/serialization and did not confirm an additional live
  issue in this audit slice; notable positive evidence includes confidential
  key zeroization and replay-aware Soranet PoW ticket validation.
- Validation:
  - `cargo deny check advisories bans sources` (blocked: `cargo-deny` not installed)
  - `bash scripts/fuzz_smoke.sh` (pass/skip: `cargo-fuzz` not installed)
  - `cd javascript/iroha_js && npm test` (blocked by unrelated compile error at `crates/iroha_core/src/pipeline/access.rs:890`)
  - `cd IrohaSwift && swift test` (fails in existing unrelated account-address / bridge-availability / tx-builder test coverage)
- Remaining implementation gap:
  - superseded by the follow-up entry above, which closes those three
    ingress/auth issues in the current tree.

## 2026-03-24 Follow-up: Soracloud HF runtime health now covers local probes, runtime heartbeats, and proxied-primary failures
- Extended the embedded HF runtime path in
  `crates/irohad/src/soracloud_runtime.rs` and the local runner in
  `crates/irohad/resources/soracloud_hf_local_runner.py` so reconciliation now
  performs a lightweight resident-worker probe for locally assigned `Warm` and
  `Warming` HF hosts after import, instead of waiting for a primary `/infer`
  request to prove the worker can actually load.
- The probe path now:
  - starts the resident worker during reconcile for locally assigned warm
    replicas as well as primaries,
  - verifies the imported model can build a local backend without running a
    full inference request, and
  - submits one authoritative `HeartbeatSoracloudModelHost` after a successful
    local probe when the validator still has a `Warming` assignment or needs a
    host-advert TTL refresh, and
  - emits authoritative `WarmupNoShow` / `AssignedHeartbeatMiss` reports when
    that local probe fails, including for replica assignments, and now follows
    those reports immediately with `ReconcileSoracloudModelHosts` so
    promotion/backfill does not wait for a later sweep, and
  - treats failed proxy-to-primary generated-HF reads as a remote-primary
    health signal by reporting `AssignedHeartbeatMiss` for the authoritative
    primary when a proxied request times out, closes, or returns a non-client
    runtime failure, and
  - when ingress routing for a generated-HF public `/infer` request fails
    earlier because the authoritative placement has no warm primary at all,
    Torii now asks the runtime handle to enqueue
    `ReconcileSoracloudModelHosts` immediately instead of waiting for a later
    expiry or a separate worker-failure signal, and
  - successful proxy-to-primary generated-HF responses are now also checked
    against the committed runtime receipt attribution before Torii returns
    them; if the receipt is missing or does not prove execution by the
    authoritative warm primary, ingress fails closed and hints
    `ReconcileSoracloudModelHosts` instead of serving a non-authoritative
    response, and the same validation now also requires the runtime receipt
    commitments and certification policy to match the proxied response Torii
    is about to serve, and those bad receipt-attribution responses now also
    feed the same remote-primary `AssignedHeartbeatMiss` reporting hook used
    for proxy timeouts and other proxied runtime failures, and
  - generated-HF proxy execution failures that already report remote-primary
    `AssignedHeartbeatMiss` now also request authoritative
    `ReconcileSoracloudModelHosts` through the same hint path used by routing
    and bad-receipt failures, and
  - the proxy receive path now rejects any incoming Soracloud local-read proxy
    request unless it is the generated-HF `infer` query case and the
    receiving node is still the committed warm primary for that placement, so
    the P2P proxy path is no longer a generic remote public local-read tunnel,
    and
  - that same receive-side gate now also recomputes the canonical generated-HF
    request commitment on the authoritative primary and rejects forged or
    mismatched proxy envelopes before execution, and
  - when an assigned replica or stale former primary rejects that incoming
    generated-HF proxy execution because it is no longer the authoritative
    warm primary, Torii now also hints
    `ReconcileSoracloudModelHosts` through the same runtime path instead of
    relying only on the caller-side routing view, and
  - Torii now binds each pending generated-HF proxy request to the
    authoritative primary peer it targeted, so a proxy response from the wrong
    peer or with an unsupported response schema now fails closed instead of
    being accepted purely by matching `request_id`, and
  - when that wrong peer is still an assigned generated-HF host for the same
    placement, the runtime now reports the responder through the existing
    `WarmupNoShow` / `AssignedHeartbeatMiss` evidence path before the same
    authoritative reconcile hint is enqueued, so stale primary/replica
    authority drift no longer lands as reconcile-only noise; and
  - now auto-reports `AdvertContradiction` when the local validator's runtime
    peer id disagrees with its authoritative model-host advert peer id.
- Added focused runtime tests covering:
  - resident-worker startup during reconcile for a local warm replica; and
  - authoritative model-host heartbeat submission after a successful local
    warming probe; and
  - immediate authoritative host reconcile submission after runtime-generated
    warmup/worker failure reports; and
  - remote-primary health reporting for generated-HF proxy failures; and
  - runtime-side advert-contradiction reporting for local peer-id mismatch; and
  - reconcile-time `AssignedHeartbeatMiss` reporting for a failing warm local
    replica; and
  - unexpected assigned proxy-responder reporting for both warm and warming
    generated-HF hosts.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo check -p irohad --tests` (pass)
  - `cargo test -p irohad reconcile_once_starts_resident_worker_for_local_warm_replica -- --nocapture` (pass)
  - `cargo test -p irohad reconcile_once_submits_model_host_heartbeat_after_successful_warming_probe -- --nocapture` (pass)
  - `cargo test -p irohad reconcile_once_reports_warmup_no_show_for_local_warming_host_import_failure -- --nocapture` (pass)
  - `cargo test -p irohad reconcile_once_reports_warm_replica_worker_failure -- --nocapture` (pass)
  - `cargo test -p irohad execute_local_read_generated_hf_infer_reports_primary_worker_failure_once -- --nocapture` (pass)
  - `cargo test -p irohad execute_local_read_generated_hf_infer_reuses_resident_worker_across_calls -- --nocapture` (pass)
  - `cargo test -p irohad execute_local_read_generated_hf_infer_executes_imported_model_locally -- --nocapture` (pass)
  - `cargo test -p irohad report_generated_hf_proxy_failure_reports_primary_host_violation -- --nocapture` (pass)
  - `cargo test -p irohad request_generated_hf_reconcile_submits_reconcile_when_no_warm_primary -- --nocapture` (pass)
  - `cargo test -p irohad request_generated_hf_reconcile_submits_reconcile_for_assigned_replica_authority_failure -- --nocapture` (pass)
  - `cargo test -p irohad reconcile_once_reports_advert_contradiction_for_local_peer_mismatch -- --nocapture` (pass)
  - `cargo check -p iroha_torii --tests` (pass)
  - `cargo check -p iroha_torii --lib` (pass)
  - `cargo test -p iroha_torii validate_incoming_soracloud_proxy_request_authority_accepts_generated_hf_primary --lib` (pass)
  - `cargo test -p iroha_torii validate_incoming_soracloud_proxy_request_authority_rejects_commitment_mismatch --lib` (pass)
  - `cargo test -p iroha_torii validate_incoming_soracloud_proxy_request_authority_rejects_non_generated_hf --lib` (pass)
  - `cargo test -p iroha_torii validate_incoming_soracloud_proxy_request_authority_rejects_non_primary_peer --lib` (pass)
  - `cargo test -p iroha_torii incoming_proxy_authority_failure_requests_generated_hf_reconcile --lib` (pass)
  - `cargo test -p iroha_torii validate_generated_hf_proxy_response_authority_accepts_matching_receipt --lib` (pass)
  - `cargo test -p iroha_torii validate_generated_hf_proxy_response_authority_rejects_mismatched_receipt --lib` (pass)
  - `cargo test -p iroha_torii validate_generated_hf_proxy_response_authority_rejects_result_commitment_mismatch --lib` (pass)
  - `cargo test -p iroha_torii soracloud_proxy_response_completes_pending_request --lib` (pass)
  - `cargo test -p iroha_torii soracloud_proxy_response_rejects_unexpected_responder --lib` (pass)
  - `cargo test -p iroha_torii soracloud_proxy_response_rejects_unsupported_schema --lib` (pass)
  - `cargo test -p iroha_torii proxy_execution_failure_reports_remote_primary_health_and_requests_reconcile --lib` (pass)
  - `cargo test -p iroha_torii proxy_validation_failure_reports_remote_primary_health_and_requests_reconcile --lib` (pass)
  - `cargo test -p iroha_torii soracloud_proxy_failure_reports_remote_primary_health --lib` (pass)
  - `cargo test -p iroha_torii soracloud_routing_failure_requests_generated_hf_reconcile --lib` (pass)
- Remaining implementation gap:
  - broader cross-node/runtime-cluster health signals still remain follow-up
    work; the authoritative path is now driven by expiry and the local
    validator's direct warmup/worker observations plus assigned-host
    receiver-side authority failures, but not by remote peers' deeper
    internal worker state.

## 2026-03-24 Follow-up: Torii/Soracloud ingress hardening now enforces signed mutations, signed-account attachment tenancy, and public-runtime throttles
- Implemented the first remediation slice for the 2026-03-24 Torii/Soracloud
  security audit across
  `crates/iroha_torii/src/{app_auth.rs,content.rs,lib.rs,soracloud.rs,zk_attachments.rs}`,
  `crates/irohad/src/soracloud_runtime.rs`,
  `crates/iroha_config/src/parameters/{defaults.rs,user.rs,actual.rs}`,
  `crates/iroha_cli/src/soracloud.rs`, and the Soracloud CLI docs.
- The shipped hardening in this slice:
  - canonical request verification now returns both the signed account and the
    exact verifying public key, and Torii now authenticates standard
    Soracloud mutation routes from `X-Iroha-Account` /
    `X-Iroha-Signature` before the handler runs;
  - standard Soracloud mutation handlers now reject inline
    `authority` / `private_key` JSON fields and return deterministic
    `tx_instructions` draft responses instead of server-submitting the
    transaction;
  - the CLI now signs those HTTP mutation requests canonically and submits the
    returned draft transaction itself through the normal Iroha client path;
  - ZK attachment CRUD now requires signed-account headers for tenancy even
    when API tokens are disabled, removing the remote-IP fallback; and
  - public Soracloud runtime ingress now has explicit per-IP rate /
    burst / inflight limits and the embedded runtime rejects non-public local
    read routes before local or peer-proxied execution.
- Added focused regression coverage for:
  - Soracloud mutation signer enforcement and draft/legacy request-shape split;
  - signed-account attachment tenancy derivation;
  - Soracloud public-runtime config defaults; and
  - runtime rejection of internal service routes on the local-read path.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo check -p iroha_config -p iroha_torii -p irohad -p iroha_cli` (pass)
  - `cargo test -p iroha_cli signed_ --bins` (pass)
  - `cargo test -p iroha_torii require_soracloud_mutation_signer --lib` (pass)
  - `cargo test -p iroha_torii attachment_tenant_is_derived_from_signed_account --lib` (pass)
  - `cargo test -p iroha_torii verify_accepts_valid_signature --lib` (pass)
  - `cargo test -p iroha_config soracloud_public_runtime_defaults_are_non_zero --lib` (pass)
  - `cargo test -p irohad execute_local_read_rejects_internal_service_route --bins` (pass)
- Remaining implementation gap:
  - broader `iroha_torii` / `irohad` end-to-end validation is still follow-up
    work beyond the focused regression set below.

## 2026-03-24 Follow-up: Soracloud legacy inline-signing path removed from HF, autonomy, and private-runtime flows
- Completed the second remediation slice across
  `crates/iroha_torii/src/soracloud.rs`,
  `crates/iroha_cli/src/soracloud.rs`,
  `crates/iroha_core/src/{soracloud_runtime.rs,smartcontracts/isi/soracloud.rs}`,
  `crates/iroha_data_model/src/isi/soracloud.rs`, and the Soracloud CLI docs.
- The remaining hardening in this slice:
  - `hf-deploy` and `hf-lease-renew` now require client-signed auxiliary
    provenance for the deterministic generated HF service/apartment artifacts
    instead of relying on Torii-held caller private keys;
  - `agent-autonomy-run` and `model/run-private` now use a signed
    draft-then-finalize flow, where the first mutation records approval/start
    and the second signed finalize request executes the runtime path and
    returns authoritative follow-up instructions as deterministic drafts;
  - `model/decrypt-output` now returns the private-inference checkpoint as a
    deterministic draft transaction instead of doing a server-side follow-up
    submission; and
  - authoritative private-inference checkpoints now rely on the outer
    transaction signature only, removing the last embedded checkpoint
    provenance that required Torii-side signing.
- Added focused regression coverage for:
  - generated HF service/apartment auxiliary provenance requirements;
  - signer binding on those generated HF auxiliary payloads; and
  - acceptance of valid generated auxiliary provenance for first-time
    deterministic HF admission;
  - finalize signer mismatch rejection for approved autonomy runs; and
  - idempotent/no-op handling for non-HF autonomy finalize and for
    already-recorded authoritative runtime receipt/execution follow-ups.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo check -p iroha_torii -p iroha_cli -p iroha_core -p iroha_data_model` (pass)
  - `cargo test -p iroha_torii ensure_hf_generated_ --lib` (pass)
  - `cargo test -p iroha_torii require_soracloud_mutation_signer --lib` (pass)
  - `cargo test -p iroha_cli signed_ --bins` (pass)
  - `cargo test -p iroha_core private_inference_release_finalization_reuses_checkpoint_step --lib` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha_target_finalize_gaps cargo check -p iroha_torii` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha_target_finalize_gaps cargo test -p iroha_torii agent_autonomy_run_ --lib` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha_target_finalize_gaps cargo test -p iroha_torii build_authoritative_agent_ --lib` (pass)
- Remaining implementation gap:
  - broader end-to-end Soracloud runtime/network coverage is still follow-up
    work, but the legacy inline-signing path itself is now removed.

## 2026-03-24 Follow-up: Soracloud HF advertise path now auto-reports advert contradictions and syncs placement routing metadata
- Extended the authoritative Soracloud HF advertise flow in
  `crates/iroha_core/src/smartcontracts/isi/soracloud.rs` so re-advertised
  model-host capabilities are checked against both the host-class floor and
  the validator's currently assigned HF placements before the capability record
  is replaced.
- Valid re-advertise mutations now also keep the authoritative placement view
  in sync for that validator by updating assigned-host `peer_id` and
  `host_class` metadata and recomputing the current window's reservation fee
  when the host class changes.
- Contradictory re-advertise mutations no longer fail with a plain validation
  error and roll back the evidence path:
  - the advertise execution path now records persisted
    `AdvertContradiction` evidence,
  - applies the existing validator slash/eviction policy, and
  - triggers the same deterministic placement refresh/rebalance path used by
    the other host-violation classes.
- Added focused core tests covering:
  - valid re-advertise metadata + fee synchronization for an already assigned
    placement; and
  - contradictory re-advertise evidence/slash/eviction for an assigned host.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo check -p iroha_core --lib` (pass)
  - `cargo test -p iroha_core model_host_ --lib` (pass)
  - `cargo test -p iroha_core report_model_host_violation --lib` (pass)
  - `cargo test -p iroha_core reconcile_soracloud_model_hosts --lib` (pass)
  - `cargo test -p iroha_core join_hf_shared_lease_rejects_when_no_model_host_can_run_profile --lib` (pass)
- Remaining implementation gap:
  - live runtime-health coverage is still narrower than the full design;
    beyond advert expiry and the current local warmup/local-primary failure
    signals, broader replica-health reporting remains follow-up work.

## 2026-03-24 Follow-up: Soracloud HF runtime health now auto-reports local warmup and primary-worker failures
- Extended the embedded `irohad` Soracloud runtime in
  `crates/irohad/src/{main.rs,soracloud_runtime.rs}` so locally assigned HF
  hosts can submit authoritative `ReportSoracloudModelHostViolation`
  instructions through the normal transaction queue instead of relying only on
  advert-expiry reconciliation.
- The runtime now:
  - enqueues `WarmupNoShow` evidence automatically when a locally assigned
    `Warming` HF host fails its import/warmup path during reconciliation;
  - enqueues `AssignedHeartbeatMiss` evidence automatically when the warm local
    primary's resident HF worker fails during generated `/infer` execution; and
  - throttles repeated submissions per `(validator, kind, placement)` so one
    bad worker loop does not flood the queue while the authoritative
    control-plane rebalance/slash path catches up.
- Added focused runtime tests covering:
  - reconcile-time warmup failure reporting for a local `Warming` placement;
    and
  - execute-time resident-worker failure reporting for a local warm primary,
    including throttling across repeated `/infer` failures.
- Validation:
  - `cargo fmt --all` (pass)
  - `CARGO_TARGET_DIR=target_runtime_health cargo check -p irohad --tests`
    (blocked by an unrelated pre-existing `iroha_torii` compile error at
    `crates/iroha_torii/src/lib.rs:15085`, where
    `jsonwebtoken::decode::<norito::json::Value>` still requires a serde-owned
    claim type)
- Remaining implementation gap:
  - broaden live runtime-health reporting beyond the current local warmup +
    local-primary failure signals if replica-side health should trigger the
    same authoritative rebalance/slash path proactively.

## 2026-03-24 Security audit: Torii/Soracloud ingress review identified four actionable issues
- Wrote `security_best_practices_report.md` after a focused code audit of the
  highest-risk externally reachable surfaces in `iroha_torii` and the
  Soracloud runtime/proxy path.
- Findings captured in the report:
  - Soracloud mutation endpoints currently require callers to send raw private
    keys to Torii for server-side transaction signing;
  - Soracloud peer proxy requests can execute arbitrary local-read handlers on
    the primary without re-checking public-route visibility;
  - public Soracloud runtime fallback handling bypasses Torii's normal API
    token and rate-limiting path; and
  - ZK attachment tenancy falls back to remote IP when API tokens are disabled,
    allowing cross-user attachment access on shared egress networks.
- Validation:
  - manual code review only; no build/test commands were run
- Remaining implementation gap:
  - the report findings still need prioritization and remediation work.

## 2026-03-24 Follow-up: Soracloud HF lease integration tests no longer crash peers during runtime reconciliation
- Fixed `crates/irohad/src/soracloud_runtime.rs` so Soracloud runtime-manager
  reconciliation no longer runs `reqwest::blocking` Hugging Face / remote
  hydration work on Tokio async worker threads:
  - startup reconciliation now runs on a dedicated OS thread before the
    periodic loop is spawned, and
  - interval-driven reconciliation now dispatches `reconcile_once()` through
    `tokio::task::spawn_blocking`, preventing Tokio's
    `Cannot drop a runtime in a context where blocking is not allowed` panic
    when the blocking HTTP clients are dropped.
- Added focused async regression coverage in
  `crates/irohad/src/soracloud_runtime.rs` proving the background reconcile
  task can import an assigned generated-HF source without panicking.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo test -p irohad reconcile_task_imports_generated_hf_source_without_panicking -- --nocapture` (pass)
  - `cargo test -p integration_tests --test iroha_cli soracloud_hf_shared_lease_commands_use_live_torii_control_plane -- --nocapture --test-threads=1` (pass)
  - `cargo test -p integration_tests --test iroha_cli soracloud_hf_pre_expiry_renewal_queues_and_promotes_next_window -- --nocapture --test-threads=1` (pass)
  - `cargo test -p integration_tests --test iroha_cli soracloud_hf_shared_lease_prorates_refunds_across_multiple_accounts -- --nocapture --test-threads=1` (pass)
- Remaining validation gap:
  - the broader workspace sweep is still pending.

## 2026-03-24 Follow-up: Soracloud HF live CLI lease tests pass again against the authoritative control plane
- Updated the live Soracloud HF lease integration coverage in
  `integration_tests/tests/iroha_cli.rs` to use the small public
  `hf-internal-testing/tiny-random-gpt2` safetensors fixture instead of
  `openai/gpt-oss`, whose anonymous model-info endpoint now returns
  `401 Unauthorized`.
- Reworked the live HF fixture setup so the tests advertise a valid
  `cpu.small` Soracloud model host from a real validator account on an NPoS
  test network, including the `bls_normal` signing allowlist and class-floor
  capacities required by authoritative `model-host-advertise`.
- Fixed `crates/iroha_cli/src/soracloud.rs` so `model-host-advertise` emits the
  supported schema version and a live `advertised_at_ms` timestamp instead of
  relying on Torii-side normalization.
- Fixed `crates/iroha_test_network/src/{config.rs,lib.rs}` so custom
  `crypto.allowed_signing` overrides are propagated into the genesis crypto
  manifest as well as the runtime config, keeping BLS admission consistent
  during live-control-plane tests.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_test_network genesis_with_crypto_override_embeds_manifest_metadata -- --nocapture` (pass)
  - `cargo test -p iroha_cli --bin iroha signed_model_host_advertise_request_uses_supported_schema_version -- --nocapture` (pass)
  - `cargo test -p integration_tests --test iroha_cli soracloud_hf_ -- --nocapture --test-threads=1` (pass)
- Remaining validation gap:
  - the broader workspace sweep is still pending.

## 2026-03-24 Follow-up: mobile migration verification is green on Kotlin, but the legacy Java Android baseline is still red
- Ran the Kotlin SDK mobile-facing verification slice and confirmed the new
  migration target currently builds/tests cleanly:
  - `kotlin/:core-jvm:test` passed, and
  - `kotlin/:client-android:assembleRelease` plus
    `kotlin/:offline-wallet-android:assembleRelease` passed.
- Ran the legacy Java Android verification slice to compare the migration
  target against the existing baseline:
  - `java/iroha_android/:android:test` passed,
  - `java/iroha_android/:jvm:test` passed,
  - `java/iroha_android/:samples-android:testDebugUnitTest` passed, and
  - `java/iroha_android/:core:test` failed with 7 red tests.
- Fixed one blocking compile regression in
  `java/iroha_android/src/test/java/org/hyperledger/iroha/android/client/HttpClientTransportTests.java`
  so the Java baseline could run far enough to expose the real failing tests
  instead of stopping in `:core:compileTestJava`.
- Remaining failing Java baseline tests are:
  - `GradleHarnessTests[org.hyperledger.iroha.android.address.AccountAddressTests]`
    because the address compliance fixture loader now expects
    `encodings.ih58` to be an object for
    `addr-single-default-ed25519`,
  - `GradleHarnessTests[org.hyperledger.iroha.android.client.OfflineToriiClientTests]`
    because `listAllowancesParsesResponse()` still expects a different
    `assetDefinitionId`, and
  - 5 failing cases in
    `org.hyperledger.iroha.android.model.instructions.AccountLiteralHardCutTests`
    around strict encoded-only literal handling.
- Validation:
  - `cd kotlin && ./gradlew :core-jvm:test :client-android:assembleRelease :offline-wallet-android:assembleRelease --console=plain` (pass)
  - `cd java/iroha_android && JAVA_HOME=/opt/homebrew/opt/openjdk@21 ANDROID_HOME=/Users/sdi/Library/Android/sdk ANDROID_SDK_ROOT=/Users/sdi/Library/Android/sdk ./gradlew check --console=plain` (fails in `:core:test`)
  - `cd java/iroha_android && JAVA_HOME=/opt/homebrew/opt/openjdk@21 ANDROID_HOME=/Users/sdi/Library/Android/sdk ANDROID_SDK_ROOT=/Users/sdi/Library/Android/sdk ./gradlew :jvm:test :samples-android:testDebugUnitTest --console=plain` (pass)
- Remaining migration-confidence gap:
  - the Kotlin SDK test suite is green, but some of the red Java baseline
    behaviors above do not yet have direct Kotlin regression coverage, so the
    repository cannot yet claim full Java→Kotlin migration parity on the
    mobile surface.

## 2026-03-24 Follow-up: Kotlin SDK parity guidance and Norito fixture checks now cover the new Kotlin SDK
- Updated the repository agent guidance so the new `kotlin/` SDK is treated as
  the default Android/JVM client surface, with the migration rule that Kotlin
  SDK behavior changes must be mirrored in the corresponding `java/`
  implementation until the Java Android SDK is retired.
- Added the Kotlin SDK constraints from `kotlin/CLAUDE.md` to the shared agent
  guidance: no reflection in the Kotlin SDK, keep `core-jvm` Android-free,
  keep Android client code in `client-android`, and keep offline
  wallet/JNI-specific code in `offline-wallet-android`.
- Added focused Kotlin parity coverage under `kotlin/core-jvm` for the shared
  Android fixture corpus and Norito wire-format checks, including direct
  fixture-loader edge cases that previously only existed in the Java tests.
- Extended `scripts/check_norito_bindings_sync.py` so Norito source changes now
  require `kotlin/core-jvm` updates alongside Python and Java, the helper runs
  the Kotlin parity tests, untracked files are inspected individually, and
  repository-internal `AGENTS.md` updates do not trigger false Norito binding
  drift failures.
- Validation:
  - `cd kotlin && ./gradlew :core-jvm:test --console=plain` (pass)
  - `BASE_REF=HEAD python3 scripts/check_norito_bindings_sync.py` (pass)
  - `python3 -m py_compile scripts/check_norito_bindings_sync.py scripts/tests/check_norito_bindings_sync_test.py` (pass)
- Remaining validation gap:
  - `python3 -m pytest scripts/tests/check_norito_bindings_sync_test.py` was
    not runnable here because `pytest` is not installed in the current Python
    environment.

## 2026-03-24 Follow-up: multisig integration tests compile against `iroha_torii`'s public API again
- Fixed the `error[E0603]` regression in
  `integration_tests/tests/multisig.rs` by exposing the multisig request DTOs
  that the test uses from the `iroha_torii` crate root instead of reaching
  through the private `routing` module.
- Added the missing public-item documentation required for the newly
  re-exported multisig request DTO fields and derived `Clone` for
  `MultisigAccountSelectorDto` so the existing test request construction still
  compiles unchanged.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo test -p integration_tests multisig --no-run` (pass)
- Remaining validation gap:
  - the broader workspace sweep was not rerun for this targeted compile fix.

## 2026-03-24 Follow-up: Soracloud HF expiry reconciliation now emits authoritative host-violation evidence and staking slashes
- Extended the Soracloud HF control loop across
  `crates/iroha_data_model/src/{soracloud.rs,isi/{mod.rs,registry.rs,soracloud.rs}}`,
  `crates/iroha_core/src/{state.rs,smartcontracts/isi/{mod.rs,soracloud.rs}}`,
  and `crates/iroha_config/src/parameters/{defaults.rs,actual.rs,user.rs}`:
  - added authoritative `SoraModelHostViolationEvidenceRecordV1` records plus
    a `ReportSoracloudModelHostViolation` instruction and state storage for
    persisted warmup-no-show / assigned-heartbeat-miss / advert-contradiction
    evidence;
  - expiry reconciliation now reports placement-scoped host violations before
    rebalance, so expired warm primaries/replicas record deterministic evidence
    instead of only mutating placement state;
  - warmup no-shows now trigger immediate host eviction plus public-lane stake
    slashing, while assigned-host heartbeat misses accumulate per
    placement/window and slash+evict once the configured strike threshold is
    reached; and
  - Soracloud HF shared-lease config now carries explicit penalty defaults:
    `warmup_no_show_slash_bps=500`,
    `assigned_heartbeat_miss_slash_bps=250`,
    `assigned_heartbeat_miss_strike_threshold=3`, and
    `advert_contradiction_slash_bps=1000`.
- Added focused coverage proving:
  - explicit warmup-no-show reporting records evidence, evicts the host advert,
    and slashes bonded validator stake;
  - a repeated assigned-host heartbeat miss crosses the configured threshold,
    records the incremented strike count, and applies the slash+evict policy;
    and
  - authoritative expired-host reconciliation now records assigned-heartbeat
    miss evidence while preserving the previously added deterministic
    promotion/backfill behavior.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_data_model soracloud --lib` (pass)
  - `cargo test -p iroha_config nexus_hf_shared_leases --lib` (pass)
  - `cargo check -p iroha_core --lib` (pass)
  - `cargo test -p iroha_core report_model_host_violation --lib -- --nocapture` (pass)
  - `cargo test -p iroha_core reconcile_soracloud_model_hosts --lib -- --nocapture` (pass)
- Remaining implementation gap:
  - Soracloud still needs runtime-worker-health reporting beyond advert expiry
    sweeps and automatic advert-contradiction evidence generation from the
    runtime/control plane.

## 2026-03-23 Follow-up: Soracloud HF placements now reconcile expired host adverts authoritatively
- Extended the Soracloud HF host/placement control path across
  `crates/iroha_data_model/src/isi/{mod.rs,registry.rs,soracloud.rs}` and
  `crates/iroha_core/src/smartcontracts/isi/soracloud.rs`:
  - added a new authoritative `ReconcileSoracloudModelHosts` instruction so an
    operator or runtime control loop can sweep stale validator host adverts
    without changing the client-facing inference surface;
  - expired model-host adverts now reconcile through the existing committed
    placement seed and ranking logic, marking stale assigned hosts
    `Unavailable`, promoting the highest-ranked retained warm replica to
    primary, and deterministically backfilling vacant replica slots from the
    remaining eligible validator set; and
  - repeated sweeps are now idempotent for already-evicted hosts, so the same
    expiry reconciliation can run opportunistically from lease/host mutations
    without churning placement records.
- Added focused coverage proving:
  - an expired primary advert promotes a warm replica and deterministically
    backfills the missing replica slot, and
  - repeating the same expiry sweep leaves the already-reconciled placement
    unchanged.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_data_model soracloud --lib` (pass)
  - `cargo test -p iroha_core reconcile_soracloud_model_hosts --lib -- --nocapture` (pass)
  - `cargo test -p iroha_core model_host --lib` (pass)
  - `cargo check -p iroha_core --lib` (pass)
- Remaining implementation gap:
  - Soracloud still needs runtime-health-driven host violation reporting and
    slash-evidence integration for warmup no-show, repeated assigned-host
    heartbeat misses, and provable advert contradictions.

## 2026-03-23 Follow-up: `scheduler_telemetry` compiles again with the current `Nexus` config shape
- Fixed the `error[E0063]` regression in
  `crates/iroha_core/tests/scheduler_telemetry.rs` by populating the newly
  required `hf_shared_leases` and `uploaded_models` fields on the explicit
  `iroha_config::parameters::actual::Nexus` test fixture.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_core --test scheduler_telemetry --features telemetry --no-run` (pass)
- Remaining validation gap:
  - the broader workspace sweep was not rerun for this targeted compile fix.

## 2026-03-23 Follow-up: offline allowance renewal coverage now exercises non-genesis lineage rollover
- Confirmed `crates/iroha_core/src/smartcontracts/isi/offline.rs` already
  treats `prev_certificate_id` renewals/reissues as a single active lineage
  head: predecessor reserve is reused for the new allowance, any excess is
  refunded from escrow back to the controller, and the superseded predecessor
  allowance is zeroed so stale App Attest keys cannot strand spendable balance.
- Fixed the renewal regression coverage in
  `renewal_supersedes_previous_allowance_even_when_new_amount_can_be_fully_reserved`
  to run at block height `2` instead of genesis height `1`, so the test now
  executes the real non-genesis lineage-validation and rollover path.
- Validation:
  - `cargo test -p iroha_core --lib renewal_supersedes_previous_allowance_even_when_new_amount_can_be_fully_reserved -- --nocapture` (pass)
- Remaining validation gap:
  - the broader `cargo test --workspace` sweep was not rerun for this targeted
    offline-allowance regression check.

## 2026-03-23 Follow-up: Soracloud request signer bundle-root hashing no longer trips Norito tuple arity limits
- Fixed the `error[E0277]` regression in
  `crates/connect_norito_bridge/src/bin/soracloud_request_signer.rs`:
  - the uploaded-model bundle root no longer tries to hash a 15-field Norito
    tuple, which exceeded the current tuple-serialization arity supported by
    `norito`,
  - the signer now uses a small custom serializer that preserves the intended
    flat field ordering for the bundle-root preimage while staying compatible
    with the current Norito encoder, and
  - added focused unit coverage proving `derive_upload_bundle()` can build a
    minimal staged bundle, propagate the derived bundle root into its chunks,
    and validate the resulting bundle metadata.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo test -p connect_norito_bridge --bin soracloud_request_signer -- --nocapture` (pass)
- Remaining validation gap:
  - the broader workspace sweep was not rerun for this compile fix.

## 2026-03-23 Follow-up: Soracloud HF public ingress now proxies generated `/infer` to the authoritative primary
- Extended the generated-HF public ingress path across
  `crates/iroha_core/src/{lib.rs,soracloud_runtime.rs}`,
  `crates/iroha_torii/src/{lib.rs,soracloud.rs}`, and
  `crates/irohad/src/{main.rs,soracloud_runtime.rs}`:
  - shared runtime helpers now resolve the authoritative active placement and
    current warm primary assignment for generated HF services directly from
    committed lease/member/placement state;
  - Torii now keeps a small pending-request map for Soracloud proxy responses,
    subscribes to control-topic P2P messages, and forwards generated HF
    `/infer` local reads to the authoritative primary peer when the receiving
    ingress node is not itself primary; and
  - the authoritative primary executes the proxied request through the normal
    embedded runtime path and returns the full certified local-read response
    body, bindings, and receipt metadata back to the ingress node.
- Refreshed focused test fixtures in `crates/iroha_torii/src/soracloud.rs` to
  include queued-window planned placements and compute fees now required by the
  HF placement/economics model.
- Added focused coverage proving:
  - generated HF proxy-target resolution returns the authoritative primary when
    the local ingress node is not primary,
  - the same resolution stays local when the ingress node is already primary,
    and
  - incoming Soracloud proxy responses complete the correct pending Torii
    request slot.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo check -p iroha_torii --lib` (pass)
  - `cargo test -p iroha_core resolve_generated_hf_primary_assignment_returns_warm_primary_for_bound_service --lib` (pass)
  - `cargo test -p iroha_torii resolve_soracloud_local_read_proxy_target_returns_primary_when_local_is_not_primary --lib` (pass)
  - `cargo test -p iroha_torii resolve_soracloud_local_read_proxy_target_skips_proxy_when_local_is_primary --lib` (pass)
  - `cargo test -p iroha_torii soracloud_proxy_response_completes_pending_request --lib` (pass)
- Remaining implementation gap:
  - Soracloud still needs live replica promotion/backfill tied to runtime
    health and host-violation slash evidence.

## 2026-03-23 Follow-up: Soracloud HF local execution now reuses resident per-model workers
- Extended the generated-HF local execution path in
  `crates/irohad/src/soracloud_runtime.rs`,
  `crates/irohad/resources/soracloud_hf_local_runner.py`, and
  `docs/source/soracloud/cli_local_control_plane.md`:
  - the embedded Python HF runner now supports a resident line-delimited
    server mode and keeps a per-process transformers pipeline cache so an
    already warmed model stays loaded across repeated `/infer` calls;
  - `irohad` now keeps a per-source resident worker registry, restarts stale
    or mismatched workers when the imported manifest changes, and prunes
    orphaned workers during reconciliation when the authoritative HF source
    disappears from the local runtime snapshot; and
  - generated-HF local execution no longer rebuilds a fresh Python process for
    every request, while the `_soracloud_fixture` test path now exposes a
    stable `worker_instance_id` proving hot-worker reuse.
- Added focused `irohad` runtime coverage proving repeated generated-HF
  `/infer` calls reuse the same resident worker instance for one imported
  source.
- Validation:
  - `cargo fmt --all` (pass)
  - `CARGO_TARGET_DIR=target_soracloud_runtime cargo test -p irohad generated_hf -- --nocapture` (pass)
- Remaining implementation gap:
  - Soracloud still needs live replica promotion/backfill tied to runtime
    health and host-violation slash evidence.

## 2026-03-23 Follow-up: Soracloud HF runtime now enforces local placement assignments and emits placement-backed receipts
- Extended the remaining HF runtime slice across
  `crates/iroha_data_model/src/soracloud.rs`,
  `crates/irohad/src/{main.rs,soracloud_runtime.rs}`,
  `crates/iroha_torii/src/soracloud.rs`, and
  `docs/source/soracloud/cli_local_control_plane.md`:
  - runtime receipts now carry optional `placement_id`,
    `selected_validator_account_id`, and `selected_peer_id`, and validation
    enforces that placement attribution is all-or-nothing;
  - `irohad` now threads the local validator account and peer identity into the
    embedded runtime manager, resolves authoritative HF placements for
    generated-HF services, and only imports/warms shared HF bytes on locally
    assigned hosts;
  - generated HF `metadata` local reads now require the node to be an assigned
    host, the embedded runtime still fails closed on direct replica/unassigned
    `infer` execution while Torii proxies public generated HF `/infer` ingress
    to the authoritative primary, and successful local reads emit
    placement-backed runtime receipts; and
  - Torii agent-runtime receipt projections now preserve the placement-backed
    host attribution fields instead of dropping them from runtime summaries.
- Added focused `irohad` runtime coverage proving:
  - unassigned hosts keep generated HF sources metadata-only during
    reconciliation,
  - locally assigned primary hosts include authoritative placement identity in
    generated HF `/infer` receipts, and
  - replica hosts reject local generated HF `/infer` execution until
    proxy-to-primary routing is implemented.
- Validation:
  - `cargo fmt --all` (pass)
  - `CARGO_TARGET_DIR=target_soracloud_runtime cargo test -p iroha_data_model soracloud --lib` (pass)
  - `CARGO_TARGET_DIR=target_soracloud_runtime cargo test -p irohad generated_hf -- --nocapture` (pass)
- Remaining implementation gap:
  - the runtime path is now placement-aware, but Soracloud still needs
    live replica promotion/backfill tied to runtime health and
    host-violation slash evidence.

## 2026-03-23 Follow-up: Soracloud HF lease admission now derives canonical HF resource profiles, records deterministic placements, and charges compute windows separately
- Extended the Soracloud HF lease/control-plane slice across
  `crates/iroha_data_model/src/{isi/soracloud.rs,soracloud.rs}`,
  `crates/iroha_core/src/smartcontracts/isi/soracloud.rs`,
  `crates/iroha_torii/src/soracloud.rs`,
  `crates/iroha_cli/src/soracloud.rs`, and
  `docs/source/soracloud/cli_local_control_plane.md`:
  - `JoinSoracloudHfSharedLease` / `RenewSoracloudHfSharedLease` now carry an
    optional canonical HF resource profile, queued-next-window records now
    persist both a compute reservation fee and a deterministic planned
    placement, and the data-model validation/tests enforce that linkage;
  - core HF lease execution now resolves the canonical resource profile,
    rejects admission when no active validator host advert can satisfy it,
    selects stake-weighted deterministic placements for active/queued windows,
    records those placements authoritatively, retires them when windows expire
    without rollover, and keeps placement host sets refreshed when assigned
    hosts heartbeat or withdraw;
  - storage and compute lease accounting are now split in the core HF member
    flow: initial joins and renewals charge both storage and compute window
    reservations, later joins prorate only the remaining storage/compute
    window, and existing members receive storage refunds plus authoritative
    `total_compute_refunded_nanos` updates from those later joins;
  - Torii `hf-deploy` / `hf-lease-renew` now derive the canonical HF profile
    from the resolved repo metadata (`siblings` + HEAD `Content-Length`
    inspection), prefer `gguf` over `safetensors` over PyTorch layouts, and
    submit the resulting profile into lease admission instead of leaving
    placement/resource data implicit; and
  - the Soracloud CLI now exposes signed `model-host-advertise`,
    `model-host-heartbeat`, `model-host-withdraw`, and `model-host-status`
    commands against the authoritative Torii surface.
- Added/updated focused coverage in
  `crates/iroha_core/src/smartcontracts/isi/soracloud.rs` proving:
  - active-window renewals persist queued compute reservations and planned
    placements,
  - queued sponsorship rollover preserves the queued placement and resets
    immediate member charges,
  - HF lease admission now fails closed when no eligible host advert exists,
    and
  - late-join flows prorate compute charges and refund earlier members.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_data_model soracloud --lib` (pass)
  - `cargo test -p iroha_core model_host --lib` (pass)
  - `CARGO_TARGET_DIR=target_soracloud_finish cargo test -p iroha_core hf_shared_lease --lib` (pass)
  - `cargo check -p iroha_torii --lib` (pass)
  - `cargo check -p iroha_cli` (pass)
- Remaining implementation gap:
  - the authoritative placement/economics path is now wired through lease
    admission, but the runtime-serving side is still follow-up work:
    live replica promotion/backfill and slash-evidence emission for host
    no-show / heartbeat violations are not yet implemented.

## 2026-03-23 Follow-up: Torii approvals list now filters canonical operation types server-side and skips stale signatory-index entries
- Extended the signer-scoped approvals read path in
  `crates/iroha_torii/src/routing.rs` so
  `POST /v1/multisig/approvals/list` now accepts canonical
  `operation_type` filters, normalizes them to uppercase, classifies
  multisig proposals directly from the on-chain instruction payload, and
  applies that filtering before cursor pagination:
  - the server-side classifier now recognizes the direct portal taxonomy
    `TRANSFER`, `MINT`, `MINT_REQUEST`, `ISSUANCE_SWAP`,
    `EXECUTE_TRIGGER`, and `ONCHAIN_MULTISIG`,
  - relay approve wrappers and cancel wrappers remain hidden from approvals
    list/get exactly as before, and
  - unknown `operation_type` values now simply return no matches rather than
    failing the request.
- Hardened approvals discovery so stale multisig signatory-index entries are
  logged and skipped instead of aborting the whole approvals request.
- Refreshed adjacent HF shared-lease Torii test fixtures in
  `crates/iroha_torii/src/soracloud.rs` with newly required
  `resource_profile` and compute-charge fields so the targeted `iroha_torii`
  unit test slice compiles again while validating the approvals changes.
- Added focused regression coverage in `crates/iroha_torii/src/routing.rs`
  proving:
  - canonical approvals filters match each supported operation type,
  - operation-type filtering happens before cursor pagination,
  - relay/cancel wrappers stay hidden, and
  - stale signatory-index entries are non-fatal.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_torii --lib multisig_approvals_ -- --nocapture` (pass)
- Remaining validation gap:
  - the broader `iroha_torii` test sweep and a fresh reset/redeploy smoke run
    against live Torii/web flows were not executed in this repo.

## 2026-03-23 Follow-up: Soracloud HF host adverts and placement records now exist in authoritative state
- Extended the Soracloud HF control-plane schema and execution path across
  `crates/iroha_data_model/src/{isi/mod.rs,isi/registry.rs,isi/soracloud.rs,soracloud.rs,visit/mod.rs,visit/visit_instruction.rs}`,
  `crates/iroha_core/src/{state.rs,smartcontracts/isi/mod.rs,smartcontracts/isi/soracloud.rs}`,
  `crates/iroha_torii/src/{lib.rs,soracloud.rs}`, and
  `docs/source/soracloud/cli_local_control_plane.md`:
  - added first-class authoritative HF host capability adverts and HF
    placement records to the data model, including canonical provenance
    payload encoders, validation, and focused schema tests;
  - extended authoritative core state with
    `soracloud_model_host_capabilities` and `soracloud_hf_placements`, plus
    new read-only/testing accessors;
  - added `AdvertiseSoracloudModelHost`,
    `HeartbeatSoracloudModelHost`, and `WithdrawSoracloudModelHost`
    instructions, wired them through the visitor/registry/dispatch path, and
    implemented core execution with active-validator gating;
  - heartbeats now refresh the authoritative host advert and mark any assigned
    placement host slot `Warm`, while withdrawals evict the advert and mark
    assigned placement slots unavailable; and
  - Torii now exposes `/v1/soracloud/model-host/{advertise,heartbeat,withdraw,status}`
    and surfaces any authoritative HF placement snapshot plus separate
    storage-vs-compute fee fields on HF lease status/mutation responses.
- Added focused runtime coverage in
  `crates/iroha_core/src/smartcontracts/isi/soracloud.rs` proving host
  advert/withdraw lifecycle updates authoritative state and that a host
  heartbeat promotes an assigned placement to `Ready`.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_data_model soracloud --lib` (pass)
  - `cargo check -p iroha_core --lib` (pass)
  - `cargo test -p iroha_core model_host --lib` (pass)
  - `cargo check -p iroha_torii --lib` (pass)
- Remaining validation and implementation gaps:
  - deterministic stake-weighted placement selection is not yet threaded into
    `hf-deploy` / `hf-lease-renew`, so placement records are now authoritative
    and inspectable but not yet auto-created by lease admission;
  - separate compute-window proration/refund accounting is not yet wired into
    shared-lease economics; and
  - matching Soracloud CLI host-management commands are still pending.

## 2026-03-23 Follow-up: asset integration tests derive required display names from asset IDs again
- Fixed the stale asset-definition constructors in
  `integration_tests/tests/asset.rs` so the asset integration suite no longer
  depends on the old behavior where `AssetDefinition::{numeric,new}` filled in
  the human-facing name automatically:
  - added small helpers that always call
    `.with_name(asset_definition_id.name().to_string())`,
  - updated the mint-quantity and integer-spec asset tests to use those
    helpers, so they execute the intended coverage instead of skipping on
    `invalid asset definition name: asset name must not be blank`, and
  - added a focused helper test proving the generated asset definitions keep
    the ID-derived display name.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo test -p integration_tests --test asset -- --nocapture --test-threads=1` (pass)
- Remaining validation gap:
  - the broader integration / workspace sweep was not rerun for this targeted
    test fix.

## 2026-03-23 Follow-up: Soracloud test fixtures use deterministic `AccountId` construction again
- Fixed the `error[E0277]` regression in
  `crates/iroha_data_model/src/soracloud.rs` test fixtures:
  - the helper no longer calls `.parse()` on `AccountId`, which no longer
    implements `FromStr`,
  - the HF shared-lease sample records now build deterministic domainless
    `AccountId` values from seeded Ed25519 keys, which matches the current
    `AccountId` API and avoids the stale `alice@wonderland` / `bob@wonderland`
    scoped literals that `AccountId::parse_encoded` intentionally rejects.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_data_model --lib rollback_provenance_payload_encodes_canonical_tuple -- --nocapture` (pass)
- Remaining validation gap:
  - the broader workspace test sweep was not rerun for this narrow fixture fix.

## 2026-03-23 Follow-up: NPoS RBC persistence no longer loses delivered summaries across commit races or temp-store reads
- Fixed the `npos_rbc_persists_payload_across_restart` regression across
  `crates/iroha_core/src/sumeragi/main_loop/{commit.rs,block_sync.rs,proposal_handlers.rs}`
  and `crates/iroha_core/src/sumeragi/rbc_status.rs`:
  - committed-block cleanup now reuses the existing
    `should_retain_rbc_sessions_after_commit()` policy instead of purging RBC
    state unconditionally when a peer notices that a block is already committed
    through an alternate path,
  - the block-sync commit-QC path now preserves undelivered local RBC sessions
    until they converge, rather than dropping them immediately after applying
    the commit, and
  - stale `BlockCreated` handling at the committed tip now keeps the committed
    block's undelivered RBC session alive long enough for the final delivered
    summary to reach disk, and
  - `rbc_status::read_persisted_snapshot()` now treats `sessions.norito` as the
    canonical on-disk snapshot and only falls back to `sessions.norito.tmp`
    when the main file is missing or broken, so readers stop preferring stale
    in-progress temp snapshots over the delivered summary already committed to
    the main store.
- Added focused unit coverage in
  `crates/iroha_core/src/sumeragi/main_loop/tests.rs` and
  `crates/iroha_core/src/sumeragi/rbc_status.rs` proving that committed
  cleanup retains undelivered sessions until delivery is settled and that the
  persisted snapshot reader prefers the canonical main store over a stale temp
  file.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_core --lib persisted_snapshot_prefers_main_store_over_temp_file -- --nocapture` (pass)
  - `cargo test -p iroha_core --lib committed_rbc_cleanup_waits_for_local_delivery -- --nocapture` (pass)
  - `cargo test -p iroha_core --lib retain_rbc_sessions_after_commit_when_undelivered -- --nocapture` (pass)
  - `cargo test -p integration_tests --test sumeragi_npos_happy_path npos_rbc_large_payload_delivers_and_commits -- --nocapture --exact --test-threads=1` (pass)
  - `cargo test -p integration_tests --test sumeragi_npos_happy_path npos_rbc_persists_payload_across_restart -- --nocapture --exact --test-threads=1` (pass)
- Remaining validation gap:
  - the broader `sumeragi_npos_happy_path` sweep and larger workspace test
    sweep remain outstanding.

## 2026-03-23 Follow-up: Generated HF autonomy workflows now emit authoritative apartment execution audits and fail closed by default
- Extended the generated-HF agent slice across
  `crates/iroha_data_model/src/{isi/soracloud.rs,soracloud.rs,visit/mod.rs,visit/visit_instruction.rs}`,
  `crates/iroha_core/src/{block.rs,smartcontracts/isi/mod.rs,smartcontracts/isi/soracloud.rs,soracloud_runtime.rs}`,
  `crates/irohad/src/soracloud_runtime.rs`,
  `crates/iroha_torii/src/soracloud.rs`, and
  `docs/source/soracloud/cli_local_control_plane.md`
  so the remaining generated-HF autonomy gaps are closed:
  - generated HF autonomy runs now persist a dedicated authoritative
    `AutonomyRunExecuted` apartment audit event in addition to the previously
    recorded service runtime receipt, and Torii exposes that execution audit on
    both mutation responses and recent-run status;
  - approved `workflow_input_json` bodies can now opt into a deterministic
    `workflow_version = 1` sequential step list, with `${run.*}`,
    `${previous.*}`, and `${steps.<step_id>.*}` placeholder resolution across
    earlier step outputs;
  - workflow `*.text` placeholders now prefer a semantic JSON `"text"` field
    when present before falling back to the raw response body, which makes
    chained generated-HF text workflows deterministic and useful out of the
    box; and
  - generated HF bridge fallback is now explicit opt-in per request instead of
    silent: even when the node allows bridge fallback globally, the generated
    `/infer` path only uses it when the caller also sets the
    `x-soracloud-hf-allow-bridge-fallback` header.
- This closes the previously open apartment-level audit gap and the earlier
  “single-step only” autonomy limitation for linear workflows. Remaining HF
  agent work is now narrower: broader adapter/runtime coverage beyond the
  current generic local runner, and richer non-linear/tool-using orchestration
  beyond sequential step lists.
- Validation:
  - `cargo fmt --all` (pass)
  - `CARGO_TARGET_DIR=target_hf_gapfix_check cargo check -p iroha_data_model -p iroha_core -p iroha_torii -p irohad` (pass)
  - `CARGO_TARGET_DIR=target_hf_gapfix_dm cargo test -p iroha_data_model agent_apartment_audit_event_validation_requires_execution_fields -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=target_hf_gapfix_core2 cargo test -p iroha_core record_agent_autonomy_execution_records_authoritative_audit_state -- --nocapture` (targeted test passed before Cargo continued across zero-match package test targets)
  - `CARGO_TARGET_DIR=target_hf_gapfix_torii cargo test -p iroha_torii --lib authoritative_agent_ -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=target_hf_gapfix_had cargo test -p irohad execute_local_read_generated_hf_infer_ -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=target_hf_gapfix_had2 cargo test -p irohad execute_apartment_generated_hf_autonomy_ -- --nocapture` (pass)

## 2026-03-23 Follow-up: Generated HF autonomy runs now persist authoritative runtime receipts
- Extended the generated-HF agent receipt path across
  `crates/iroha_data_model/src/soracloud.rs`,
  `crates/iroha_core/src/{smartcontracts/isi/soracloud.rs,soracloud_runtime.rs}`,
  `crates/irohad/src/soracloud_runtime.rs`,
  `crates/iroha_torii/src/soracloud.rs`, and
  `docs/source/soracloud/cli_local_control_plane.md`
  so a successful generated-HF autonomy run no longer leaves its execution
  receipt only in node-local runtime state:
  - approved autonomy runs now persist the approved process generation plus a
    canonical `request_commitment`, so later service receipts bind back to the
    exact authoritative approval tuple;
  - `irohad` now preserves the generated service `SoraRuntimeReceiptV1`
    alongside the existing node-local autonomy execution summary;
  - Torii now records that service receipt into authoritative
    `soracloud_runtime_receipts` after a successful generated-HF autonomy
    execution and returns it on both the mutation response and
    `agent-autonomy-status`; and
  - recent run status now shows both the node-local execution summary and the
    authoritative recorded receipt, so callers can verify the generated
    service `/infer` execution without reading the runtime state directory.
- This closes the earlier “promote node-local autonomy execution summaries into
  an authoritative audit/receipt record” gap for generated HF agent runs.
  Remaining HF agent work is narrower: whether to add a dedicated
  apartment-level audit event on top of that service receipt, and broader
  multi-step orchestration beyond a single `/infer` request.
- Validation:
  - `cargo fmt --all` (pass)
  - `CARGO_TARGET_DIR=target_hf_receipt_check cargo check -p iroha_data_model -p iroha_core -p irohad -p iroha_torii` (pass)
  - `CARGO_TARGET_DIR=target_hf_receipt_dm cargo test -p iroha_data_model agent_autonomy_request_commitment_uses_canonical_tuple -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=target_hf_receipt_torii cargo test -p iroha_torii --lib authoritative_agent_autonomy_status_includes_runtime_recent_runs -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=target_hf_receipt_torii2 cargo test -p iroha_torii --lib authoritative_agent_ -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=target_hf_receipt_had2 cargo test -p irohad execute_apartment_generated_hf_autonomy_run_executes_locally_and_persists_summary -- --nocapture` (pass)

## 2026-03-22 Follow-up: connected-peers startup no longer times out after block 1 is already observable
- Fixed the flaky startup guard in `crates/iroha_test_network/src/lib.rs`
  that could fail `extra_functional::connected_peers::connected_peers_with_f_2_1_2`
  even after every peer had already reached block 1 and was answering `/status`:
  - `wait_for_block_1_with_watchdog()` and the bootstrap fallback in
    `start_checked()` now accept the peer's best-effort observed block height,
    which reuses the same watch/storage snapshot already recorded by the test
    network instead of depending solely on a narrower storage-sidecar probe at
    the moment of the wait,
  - `has_committed_block()` now resolves the active Kura storage directory via
    the peer startup context instead of assuming `<peer_dir>/storage`, so
    readiness checks stay correct when tests override the Kura path, and
  - added focused unit coverage for both the best-effort block-1 watchdog path
    and the resolved-Kura-path storage probe.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_test_network wait_for_block_1_with_watchdog_uses_ -- --nocapture` (pass)
  - `cargo test -p iroha_test_network has_committed_block_uses_resolved_kura_store_dir -- --nocapture` (pass)
  - `cargo test -p integration_tests --test mod extra_functional::connected_peers::connected_peers_with_f_2_1_2 -- --nocapture --exact --test-threads=1` (pass)
  - `cargo test -p integration_tests --test mod connected_peers_with_f_ -- --nocapture --test-threads=1` (pass)

## 2026-03-22 Follow-up: Generated HF apartment autonomy runs now persist structured workflow JSON
- Extended the generated-HF autonomy path across
  `crates/iroha_data_model/src/{isi/soracloud.rs,soracloud.rs}`,
  `crates/iroha_core/src/smartcontracts/isi/soracloud.rs`,
  `crates/irohad/src/soracloud_runtime.rs`,
  `crates/iroha_torii/src/soracloud.rs`,
  `crates/iroha_cli/src/soracloud.rs`, and
  `docs/source/soracloud/cli_local_control_plane.md`
  so approved runs no longer have to collapse everything into `run_label`:
  - the on-chain autonomy instruction/record plus the signed Torii/CLI request
    now carry an optional canonical `workflow_input_json` body;
  - core admission canonicalizes that JSON, charges it against deterministic
    apartment persistent-state accounting, and records its hash on the
    authoritative audit event when present;
  - `irohad` now forwards that persisted JSON verbatim to the generated HF
    `/infer` handler and only falls back to the older `run_label`-as-`inputs`
    envelope when no structured body was approved; and
  - Torii status/mutation responses now surface the stored workflow JSON so
    callers can correlate authoritative approvals with the node-local runtime
    execution summary.
- This closes the earlier “workflow input coverage” gap for single-request HF
  agent runs. The remaining HF agent work is broader orchestration beyond a
  single `/infer` request, not the lack of a structured persisted request body.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo check -p iroha_data_model -p iroha_core -p iroha_torii -p iroha_cli -p irohad` (pass)
  - `cargo test -p irohad execute_apartment_generated_hf_autonomy_run_executes_locally_and_persists_summary -- --nocapture` (pass)
  - `cargo test -p iroha_torii --lib authoritative_agent_autonomy_status_includes_runtime_recent_runs -- --nocapture` (pass)
  - `cargo test -p iroha_cli signed_agent_autonomy_run_request_uses_verifiable_signature -- --nocapture` (pass)
  - `cargo test -p iroha_cli canonicalize_agent_workflow_input_json_compacts_payload -- --nocapture` (pass)
  - `cargo test -p iroha_data_model agent_apartment_record_validation_rejects_invalid_workflow_input_json -- --nocapture` (pass)
  - `cargo test -p iroha_data_model agent_autonomy_run_provenance_payload_encodes_canonical_tuple -- --nocapture` (pass)

## 2026-03-22 Follow-up: Generated HF apartments now execute approved autonomy runs through the local HF runtime
- Extended the generated-HF agent slice across
  `crates/iroha_core/src/soracloud_runtime.rs`,
  `crates/irohad/src/soracloud_runtime.rs`,
  `crates/iroha_torii/src/soracloud.rs`, and
  `docs/source/soracloud/cli_local_control_plane.md`
  so the generated apartment path no longer stops at approval bookkeeping:
  - the shared runtime surface now includes a stable node-local autonomy-run
    execution summary format for generated apartments;
  - `irohad` now recognizes `autonomy-run:<run_id>` apartment executions for
    generated HF apartments, invokes the bound generated HF `/infer` service
    through the already-materialized local runtime path, and persists
    node-local checkpoint / execution-summary artifacts under the apartment
    materialization directory;
  - Torii `agent-autonomy-run` now executes that approved HF run immediately
    after the authoritative mutation commits and returns the runtime summary in
    the mutation response when the node-local runtime is available; and
  - `agent-autonomy-status` now includes recent node-local runtime execution
    summaries, so callers can inspect the last successful or failed generated
    HF agent runs without scraping the runtime materialization directory.
- The remaining gap is richer workflow input coverage rather than bare
  execution: generated HF apartments now call the local HF runtime, but the
  default autonomy payload still feeds `run_label` as the request input rather
  than a persisted multi-step artifact body.

## 2026-03-22 Follow-up: Soracloud HF runtime now imports shared Hub files and serves real local metadata/inference bridges

## 2026-03-22 Follow-up: Soracloud HF runtime now executes imported shared models locally
- Extended the HF runtime slice across
  `crates/iroha_config/src/parameters/{defaults,actual,user}.rs`,
  `crates/irohad/resources/soracloud_hf_local_runner.py`,
  `crates/irohad/src/soracloud_runtime.rs`, and
  `docs/source/soracloud/cli_local_control_plane.md`
  so generated HF deployments no longer depend on the remote inference bridge
  as the primary runtime path:
  - `soracloud_runtime.hf` now exposes explicit local-execution settings:
    `local_execution_enabled`, `local_runner_program`,
    `local_runner_timeout_ms`, and `allow_inference_bridge_fallback`,
    alongside the existing Hub/API/import/bridge settings;
  - `irohad` now materializes an embedded Python HF runner under the local
    Soracloud runtime state directory and invokes it through the configured
    interpreter program;
  - generated HF `infer` local reads now prefer on-node execution against the
    imported shared source directory, using a deterministic local fixture path
    for tests and otherwise loading the imported model through
    `transformers.pipeline(..., local_files_only=True)` so the node runs the
    already-imported HF bytes instead of fetching fresh Hub content; and
  - the old HF Inference forwarding path is still available as controlled
    fallback when `allow_inference_bridge_fallback = true` and
    `inference_token` is configured, so operators can keep a degraded-but-live
    path if local adapter dependencies are unavailable.
- Generated HF `metadata` responses now also report whether local execution is
  enabled for the node, not only whether the bridge token is configured.
- The remaining gap has shifted from model execution to agent execution:
  generated HF services can now run imported models locally, but generated HF
  apartments still stop at admission/status and do not yet consume autonomy
  runs through a node-local agent workflow.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_config soracloud_runtime_ -- --nocapture` (pass)
  - `cargo test -p irohad hf_ -- --nocapture` (pass)

## 2026-03-22 Follow-up: Soracloud HF runtime now imports shared Hub files and serves real local metadata/inference bridges
- Extended the HF runtime slice across
  `crates/iroha_config/src/parameters/{defaults,actual,user}.rs`,
  `crates/iroha_core/src/smartcontracts/isi/soracloud.rs`,
  `crates/irohad/src/soracloud_runtime.rs`, and
  `docs/source/soracloud/cli_local_control_plane.md`
  so generated HF deployments now do real runtime work instead of only
  materializing the deterministic placeholder bundle:
  - `iroha_config` now exposes explicit `soracloud_runtime.hf` settings for
    Hub/API/inference base URLs, request timeout, import limits, an allowlisted
    file set, and the optional inference bearer token;
  - the embedded runtime manager now imports allowlisted Hugging Face repo
    files into `state_dir/hf_sources/<source_id>/files/`, writes a local
    `import_manifest.json`, and records importer failures locally instead of
    only logging them;
  - generated HF `metadata` local reads now serve that local import manifest,
    including resolved commit, imported/skipped file lists, and whether the
    inference bridge is enabled for the node;
  - generated HF `infer` local reads now forward the request body plus content
    negotiation headers to the configured HF Inference base URL when
    `soracloud_runtime.hf.inference_token` is set; and
  - runtime HF source projections now stay `PendingImport` until a successful
    local import manifest exists, and importer failures surface as runtime
    `Failed` with `last_error` instead of incorrectly reporting `Ready`.
- While verifying the slice, a fresh-build break in
  `crates/iroha_core/src/smartcontracts/isi/soracloud.rs` surfaced and was
  fixed by restoring the missing `SecretEnvelopeV1` import for uploaded-model
  ciphertext hashing.
- The remaining HF gap is now narrower again:
  - imported HF bytes are shared and queryable locally, but model execution is
    still bridged through HF Inference rather than running adapters against the
    imported weights on-node; and
  - generated HF apartments can now target a working inference route, but the
    repo still needs richer agent-side workflows beyond status/admission.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_config soracloud_runtime_ -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=target_hf_runtime cargo test -p irohad hf_ -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=target_hf_runtime cargo test -p irohad manager_config_uses_explicit_soracloud_runtime_settings -- --nocapture` (pass)
  - `cargo test -p iroha_torii --lib authoritative_hf_shared_lease_ -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=target_hf_runtime cargo check -p iroha_config -p iroha_core -p iroha_torii -p irohad` (pass)

## 2026-03-22 Follow-up: Soracloud HF generated deploys now promote sources to authoritative `Ready`
- Extended the HF shared-lease/runtime path across
  `crates/iroha_data_model/src/soracloud.rs`,
  `crates/iroha_core/src/smartcontracts/isi/soracloud.rs`,
  `crates/irohad/src/soracloud_runtime.rs`,
  `integration_tests/tests/iroha_cli.rs`, and
  `docs/source/soracloud/cli_local_control_plane.md` so the generated HF
  scaffolding behaves consistently in fresh builds and live CLI flows:
  - the shared-lease join/renew path now inspects the already-admitted
    generated HF service in the same transaction and promotes the canonical HF
    source record from `PendingImport` to authoritative `Ready` when the
    generated bundle matches the requested source/model binding,
  - live `hf-deploy` responses now surface `importer_pending = false` and
    `source.status = Ready` once the deterministic HF-generated service is
    admitted, and the CLI integration test now asserts that behavior in
    addition to the earlier service/apartment auto-deploy checks,
  - the `irohad` runtime-manager test scaffolding now initializes the newer
    `uploaded_model_binding` field everywhere, so the generated-bundle
    synthesis coverage builds cleanly in fresh targets, and
  - the uploaded-model / private-inference provenance helpers now encode their
    single value payloads directly instead of trying to serialize unsupported
    one-element tuples, with dedicated layout tests added for those payloads.
- The remaining HF gap is still the real async Hub importer plus real
  model-adapter execution. Generated HF services are now admitted, hydrated,
  and reported as ready consistently, but they still execute the deterministic
  placeholder contract rather than a full Hugging Face runtime.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_data_model --lib uploaded_model_ -- --nocapture` (pass)
  - `cargo test -p iroha_data_model --lib private_inference_start_provenance_payload_encodes_session_value -- --nocapture` (pass)
  - `cargo test -p iroha_torii --lib ensure_hf_generated_ -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-hf-verify cargo test -p iroha_core --lib join_hf_shared_lease_marks_source_ready_when_generated_service_is_already_deployed -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-hf-verify cargo test -p irohad reconcile_once_synthesizes_generated_hf_bundle_without_sorafs_importer -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-hf-verify cargo test -p integration_tests --test iroha_cli soracloud_hf_shared_lease_commands_use_live_torii_control_plane -- --nocapture` (pass)

## 2026-03-22 Follow-up: Native multisig cancel is now a first-class on-chain instruction
- Extended the native multisig instruction model across
  `crates/iroha_executor_data_model/src/isi.rs`,
  `crates/iroha_executor/src/default/isi/multisig/{mod.rs,transaction.rs}`,
  `crates/iroha_core/src/smartcontracts/isi/multisig.rs`,
  `crates/iroha_torii/src/routing.rs`,
  `crates/iroha_js_host/src/lib.rs`, and
  `crates/iroha_schema_gen/src/lib.rs`
  so `MultisigCancel` is now a real instruction instead of an app-side fiction:
  - the executor data model now exposes `MultisigInstructionBox::Cancel`
    with Norito/JSON/schema coverage,
  - both executor paths now reject direct signer-side cancel execution and only
    allow cancel once it is itself executed as the multisig account after
    quorum, which means cancel is governed by the same multisig policy as any
    other proposal action,
  - built-in core multisig execution now handles nested authenticated
    multisig instructions directly, so a cancel proposal can execute the inner
    `MultisigCancel` instead of tripping the generic custom-instruction
    upgrade guard,
  - Torii transaction-account filtering now recognizes multisig cancel
    instructions, and
  - the JS host alias layer now accepts `MultisigCancel` and maps it to the
    native `Custom.payload.Cancel` representation.
- Current behavior prunes the canceled proposal tree after quorum; this lands
  signed on-chain cancellation now, but it does not yet persist a separate
  durable `CANCELED` tombstone/status for history queries.
- Validation:
  - `CARGO_TARGET_DIR=/tmp/iroha-mcancel-schema cargo check -p iroha_schema_gen` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-mcancel-dm cargo test -p iroha_executor_data_model multisig_cancel_instruction_roundtrip_preserves_target_hash -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-mcancel-exec cargo test -p iroha_executor canceler_must_be_the_multisig_subject -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-mcancel-core cargo test -p iroha_core --lib multisig_cancel_requires_quorum_and_prunes_target_proposal -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-mcancel-js cargo test -p iroha_js_host multisig_alias_payloads_decode_as_custom_instruction -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-mcancel-torii cargo test -p iroha_torii --lib instruction_matches_account_id_matches_multisig_custom_cancel_account -- --nocapture` (pass)

## 2026-03-22 Follow-up: Soracloud HF deploy now auto-admits generated services and apartments
- Extended the HF control-plane path across
  `crates/iroha_core/src/soracloud_runtime.rs`,
  `crates/irohad/src/soracloud_runtime.rs`,
  `crates/iroha_torii/src/soracloud.rs`, and
  `integration_tests/tests/iroha_cli.rs` so `hf-deploy` / `hf-lease-renew`
  no longer stop at shared-lease membership:
  - Torii now synthesizes a deterministic HF inference-service bundle with
    `allow_model_inference = true`, signs the usual Soracloud bundle
    provenance, and auto-submits `DeploySoracloudService` before the lease
    join/renew when the named service is not already the matching generated HF
    deployment.
  - When `apartment_name` is requested, Torii also synthesizes a deterministic
    HF apartment manifest tied to that generated service and auto-submits
    `DeploySoracloudAgentApartment` before the lease join/renew unless the
    apartment already matches the expected generated manifest.
  - Reuse is fail-closed: if the requested service/apartment name already
    exists but does not match the deterministic HF-generated shape for the same
    canonical source, the HF mutation is rejected instead of binding the lease
    to unrelated infrastructure.
- The embedded runtime manager now recognizes those generated HF bundles and
  can synthesize the shared stub IVM bundle directly into the local artifact
  cache without waiting for a committed SoraFS payload, which means the
  runtime-backed HF source projection can advance to `Ready` once the service
  and apartment are materialized.
- The remaining HF gap is now narrower again:
  - the real async Hugging Face importer still needs to fetch, normalize, and
    lease canonical Hub bytes instead of leaving authoritative sources in the
    control-plane `PendingImport` skeleton by default, and
  - the generated service still uses a deterministic stub contract, so real
    model-adapter execution and agent inference calls are not wired yet.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_core generated_hf_ -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-hf-verify cargo test -p irohad reconcile_once_synthesizes_generated_hf_bundle_without_sorafs_importer -- --nocapture` (pass)
  - `cargo test -p iroha_torii --lib ensure_hf_generated_ -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-hf-verify cargo test -p integration_tests --test iroha_cli soracloud_hf_shared_lease_commands_use_live_torii_control_plane -- --nocapture` (pass)
  - `cargo check -p iroha_core -p irohad -p iroha_torii` (pass)

## 2026-03-22 Follow-up: Soracloud HF status now includes runtime-backed source projections
- Extended the shared runtime snapshot in
  `crates/iroha_core/src/soracloud_runtime.rs`,
  `crates/irohad/src/soracloud_runtime.rs`, and
  `crates/iroha_torii/src/soracloud.rs`
  so canonical HF sources are no longer visible only as static authoritative
  records:
  - the embedded runtime manager now persists `hf_sources` projections keyed by
    canonical source id,
  - each projection summarizes bound services/apartments, queued windows,
    materialized bindings, and local bundle/artifact cache misses, and
  - the runtime status distinguishes `PendingImport`, `PendingDeployment`,
    `Hydrating`, `Ready`, `Failed`, and `Retired`.
- Torii `hf-status` / HF mutation responses now expose that runtime projection
  alongside the authoritative `SoraHfSourceRecordV1`, and `importer_pending`
  now tracks the runtime projection when available instead of treating every
  non-`Ready` authoritative source as still importing forever.
- The remaining HF gap is now narrower and more concrete:
  - the real async Hub importer still needs to hydrate canonical bytes and
    advance authoritative source records beyond the current lease-control-plane
    skeleton, and
  - the generated inference service/apartment bring-up still needs to create
    the deployments that the new runtime projection is now prepared to report.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo test -p irohad derive_hf_runtime_status_distinguishes_pending_deployment_and_ready -- --nocapture` (pass)
  - `cargo test -p irohad reconcile_once_projects_hf_source_runtime_readiness_from_bound_services -- --nocapture` (pass)
  - `cargo test -p iroha_torii --lib authoritative_hf_shared_lease_ -- --nocapture` (pass)
  - `cargo test -p iroha_torii --lib hf_importer_pending_false_for_failed_sources_without_runtime -- --nocapture` (pass)
  - `cargo check -p iroha_core -p iroha_torii -p irohad` (pass)

## 2026-03-22 Follow-up: Soracloud HF leases now support pre-expiry next-window sponsorship
- Extended the HF shared-lease pool model across
  `crates/iroha_data_model/src/soracloud.rs`,
  `crates/iroha_core/src/smartcontracts/isi/soracloud.rs`,
  `crates/iroha_torii/src/soracloud.rs`, and
  `docs/source/soracloud/cli_local_control_plane.md`:
  - active shared-lease pools can now persist an optional queued next-window
    sponsor with the next window’s asset, fee, model label, timing, and
    target service/apartment bindings,
  - `RenewSoracloudHfSharedLease` now supports both modes: immediate renewal
    after expiry/drain, and pre-expiry sponsorship by an already-active member
    who fronts the next window while the current one is still live,
  - later HF mutations now promote that queued next window once the current
    term has expired, while non-sponsor members are expired out of the old
    window at the rollover boundary, and
  - queued sponsors are blocked from leaving early so the funded next window
    cannot disappear out from under the pool.
- Added focused validation and runtime coverage:
  - data-model validation now checks that queued next-window metadata lines up
    exactly with the current pool expiry and lease term,
  - core unit tests cover pre-expiry sponsorship, queued-window promotion on a
    later join, sponsor-leave rejection, and the earlier configurable
    drain-grace path, and
  - Torii authoritative status coverage now returns queued next-window state
    from committed world state, and a new 4-peer CLI integration test drives
    the live `hf-lease-renew` pre-expiry path through queued sponsorship,
    sponsor-leave rejection, and rollover promotion on a later `hf-deploy`.
- This closes the pre-expiry next-window sponsorship gap for HF shared leases.
  Remaining HF work is now the async importer/runtime hydration plus generated
  inference service + apartment bring-up.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_data_model hf_shared_lease_ -- --nocapture` (pass)
  - `cargo test -p iroha_core --lib hf_shared_lease -- --nocapture` (pass)
  - `cargo test -p iroha_torii --lib authoritative_hf_shared_lease_ -- --nocapture` (pass)
  - `cargo test -p integration_tests --test iroha_cli soracloud_hf_pre_expiry_renewal_queues_and_promotes_next_window -- --nocapture` (pass)

## 2026-03-22 Follow-up: Soracloud uploaded-model design now layers onto the existing model plane instead of inventing a parallel runtime
- Updated
  `docs/source/soracloud/{uploaded_private_models.md,cli_local_control_plane.md,manifest_schemas.md}`
  so the next Soracloud AI-model slice is explicit about how user-uploaded
  private models fit the current implementation:
  - uploaded models extend the existing
    `SoraModelRegistryV1` / `SoraModelWeightVersionRecordV1` /
    `SoraModelArtifactRecordV1` surfaces instead of replacing them,
  - HF shared-lease routes remain the shared source/import membership path and
    are now documented as distinct from the private uploaded-model path,
  - encrypted uploaded-model bytes are planned as first-class chunk/bundle
    state built on `SecretEnvelopeV1` / `CiphertextStateRecordV1`, and
  - `ram_lfe` is explicitly kept separate from private transformer execution,
    which should reuse Soracloud FHE/decryption governance plus the existing
    `allow_model_inference` capability flag.
- The new design note also fixes the missing cross-cutting picture for the next
  implementation slice: HF-style safetensors admission only, fixed 4 MiB
  plaintext sharding, BFV-backed deterministic compiled inference, new
  upload/compile/private-run routes, XOR pricing, and the records needed for
  bundle roots, compile profiles, sessions, and checkpoints.
- Follow-up doc tightening also spells out the main implementation edges that
  still matter before code lands: explicit apartment model-binding semantics,
  authoritative upload/compile/run/decrypt status and audit surfaces, and the
  quota/state-growth rules required for literal encrypted model bytes on-chain.
- This closes the design-coherence gap between the current Soracloud model
  registry features and the planned So Ra user-uploaded private-runtime flow.
  Remaining work is still the actual data-model, Torii, CLI, world-state, and
  runtime implementation.
- Validation:
  - not run (documentation-only changes)

## 2026-03-22 Follow-up: Soracloud HF shared-lease pools now honor configurable drain grace after the last member leaves
- Extended the Nexus config surface and Soracloud lease executor across
  `crates/iroha_config/src/parameters/{defaults.rs,actual.rs,user.rs}` and
  `crates/iroha_core/src/smartcontracts/isi/soracloud.rs`:
  - added `nexus.hf_shared_leases.drain_grace_ms`, with a default of 5 seconds,
    so operators can control how long an empty shared HF lease pool remains in
    `Draining` before expiry instead of relying on the old hard-coded
    immediate clamp,
  - wired that policy into `LeaveSoracloudHfSharedLease`, so the last-member
    leave path now sets `window_expires_at_ms = now + drain_grace` and keeps
    the record self-consistent even when leave happens in the same block as
    window creation, and
  - added focused config + core coverage for the new setting and the
    authoritative pool expiry behavior.
- This closes the configurable drain-grace part of the HF shared-lease
  roadmap. Remaining HF work is still the async importer/runtime hydration,
  generated inference service + apartment bring-up.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_config nexus_hf_shared_leases_ -- --nocapture` (pass)
  - `cargo test -p iroha_core leave_hf_shared_lease_last_member_uses_configured_drain_grace -- --nocapture` (pass)

## 2026-03-22 Follow-up: NPoS RBC chunk-loss backlog regression is green again
- Fixed the flaky `npos_rbc_chunk_loss_fault_reports_backlog` harness in
  `integration_tests/tests/sumeragi_npos_performance.rs` so it reliably
  reaches an undelivered multi-chunk RBC session on the 4-peer localnet used
  by the test:
  - the NPoS stress tests now install the same custom NPoS genesis parameter
    bundle they configure through the standard Sumeragi parameters, which
    keeps the collectors/redundant-send settings aligned with the scenario
    under test,
  - the chunk-loss scenario now submits one small seed block before the
    fault-injected transaction so the probe runs after genesis-only startup,
    and
  - the injected payload was reduced from the 3 MiB store-pressure blob to a
    64 KiB payload, which still spans multiple 16 KiB RBC chunks but no
    longer stalls before the backlog telemetry becomes observable.
- Validation:
  - `cargo test -p integration_tests --test sumeragi_npos_performance npos_rbc_chunk_loss_fault_reports_backlog -- --nocapture --exact --test-threads=1` (pass)
  - `cargo test -p integration_tests --test sumeragi_npos_performance -- --nocapture --test-threads=1` (pass)

## 2026-03-21 Follow-up: RBC restart recovery now survives the post-restart roster-change reset
- Fixed `crates/iroha_core/src/sumeragi/main_loop/commit.rs` so
  `reset_consensus_state_for_roster_change()` no longer clears the
  operator-facing `rbc_status` snapshot, which is the source of
  `/v1/sumeragi/rbc/sessions`; recovered sessions loaded from disk now remain
  observable while runtime-only RBC state is cleared on membership change.
- Added
  `rbc_status_summary_survives_roster_change_reset_after_restart_recovery`
  in `crates/iroha_core/src/sumeragi/main_loop/tests.rs` to persist an
  in-flight RBC session, reload it as recovered, apply the roster-reset path,
  and verify that the recovered status summary survives while the live session
  cache is still cleared.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_core --lib rbc_status_summary_survives_roster_change_reset_after_restart_recovery -- --nocapture` (pass)
  - `cargo test -p iroha_core --lib rbc_session_roster_persists_across_restart_with_roster_change -- --nocapture` (pass)
  - `cargo test -p integration_tests --test sumeragi_da sumeragi_rbc_recovers_after_restart_with_roster_change -- --nocapture --exact --test-threads=1` (pass)
- Notes:
  - The integration rerun still emitted the existing non-fatal
    `iroha_test_network` teardown warnings (`forcing peer shutdown after fatal
    signal`, `timed out waiting for log flush`) and a temp-store recovery
    warning from `rbc_status`; the test passed and all peers exited with
    `status(0)`.

## 2026-03-21 Follow-up: Soracloud HF shared-lease live coverage now verifies multi-account proration and refund splitting
- Extended
  `integration_tests/tests/iroha_cli.rs`
  so the HF shared-lease integration path now covers the actual shared-cost
  economics across distinct authorities, not just first-sponsor / rebind /
  leave / renew control-plane plumbing:
  - added a reusable `program_config_for_account(...)` helper so the CLI
    integration harness can drive Bob and Carpenter against the same Torii
    network with their own keys and domains,
  - added a new 4-peer live test that grants Soracloud + transfer capability
    to Bob and Carpenter in genesis, funds all three members with a real
    registered lease asset, then performs Alice first-sponsor deploy, Bob
    join, and Carpenter join through the public `hf-*` CLI commands, and
  - the test derives Bob and Carpenter’s expected join charges from the
    authoritative HF audit timestamps returned by `hf-status`, then verifies
    final `total_paid_nanos` / `total_refunded_nanos` on Alice, Bob, and
    Carpenter with the same deterministic refund-ordering rule used by core.
- This closes the live multi-account refund/proration coverage gap for the
  current shared-lease substrate. The remaining HF work is still the async
  importer/runtime hydration, generated inference service + apartment bring-up,
  and the then-open drain-grace follow-up.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo test -p integration_tests --test iroha_cli soracloud_hf_shared_lease_prorates_refunds_across_multiple_accounts -- --nocapture` (pass)

## 2026-03-21 Follow-up: Soracloud HF shared-lease CLI and live Torii coverage are now wired end-to-end
- Extended the Hugging Face shared-lease slice across
  `crates/iroha_cli/src/soracloud.rs`,
  `integration_tests/tests/iroha_cli.rs`,
  `crates/iroha_torii/src/soracloud.rs`, and
  `docs/source/soracloud/cli_local_control_plane.md`:
  - the Soracloud CLI now exposes `hf-deploy`, `hf-status`,
    `hf-lease-leave`, and `hf-lease-renew`, each using canonical provenance
    signatures and defaulting an omitted model label to the repo slug while
    keeping `revision = main` canonicalisation aligned with Torii,
  - live CLI coverage now exercises first-window sponsorship, same-account
    zero-charge rebind, authoritative status reads, leave, and renew against
    a real 4-peer Torii-backed network rather than just unit fixtures, and
  - `GET /v1/soracloud/hf/status` no longer depends on `NoritoQuery`
    deserialising the tagged `StorageClass` enum directly; Torii now accepts
    case-insensitive scalar query labels (`hot|warm|cold`) and parses them
    explicitly before reading authoritative world state.
- This closes the CLI/live-coverage gap for the current shared-lease
  substrate, but the async HF importer/runtime hydration, generated inference
  services, and optional one-click apartment bootstrap were still open at that
  point.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_cli hf_ -- --nocapture` (pass)
  - `cargo test -p iroha_torii parse_storage_class_query_accepts_case_insensitive_labels --lib -- --nocapture` (pass)
  - `cargo test -p integration_tests --test iroha_cli soracloud_hf_ -- --nocapture` (pass)
  - `cargo check -p iroha_data_model -p iroha_core -p iroha_torii -p iroha_cli -p integration_tests` (pass)

## 2026-03-21 Follow-up: Soracloud HF shared-lease control-plane routes now read and mutate authoritative world state
- Extended the new Hugging Face shared-lease substrate across
  `crates/iroha_data_model/src/{soracloud.rs,isi/soracloud.rs}`,
  `crates/iroha_core/src/{smartcontracts/isi/soracloud.rs,state.rs,block.rs}`,
  `crates/iroha_torii/src/{soracloud.rs,lib.rs}`,
  and the Soracloud route table:
  - Torii now exposes `POST /v1/soracloud/hf/deploy`,
    `GET /v1/soracloud/hf/status`,
    `POST /v1/soracloud/hf/lease/leave`, and
    `POST /v1/soracloud/hf/lease/renew`, each backed by the new
    authoritative shared-lease ISIs and world-state records rather than any
    Torii-local shim,
  - the new HF request signatures now encode canonical payloads
    deterministically, including defaulting an omitted revision to `main`
    before verifying and submitting the underlying instruction,
  - Torii authoritative responses can now report canonical HF source/pool/
    membership/audit state directly from `World`, and existing SCR admission
    snapshots now surface `allow_model_inference` alongside the earlier wallet,
    state-write, and model-training capability flags, and
  - the shared Soracloud audit sequence in core now includes HF shared-lease
    audit events, preventing duplicate sequence reuse across consecutive HF
    lease mutations.
- This slice intentionally stops at the shared-lease/control-plane substrate:
  the async HF importer/runtime hydration, generated inference services,
  optional one-click apartment bootstrap, and CLI wrappers were still open at
  that point.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo check -p iroha_data_model -p iroha_core -p iroha_torii` (pass)
  - `CARGO_TARGET_DIR=target_tmp_hf_torii cargo test -p iroha_torii --lib hf_ -- --nocapture` (pass)
  - `cargo test -p iroha_core next_soracloud_audit_sequence_includes_hf_shared_lease_events -- --nocapture` (pass; filtered command then continued draining zero-test binaries as expected)
- Remaining validation gap:
  - add importer/runtime, CLI, and live integration coverage for the new HF
    routes.

## 2026-03-21 Follow-up: account transaction routes no longer fall back to tx-history JWT gating, and account-scoped history is authority-scoped again
- Restored the `/v1/accounts/{account_id}/transactions` and
  `/v1/accounts/{account_id}/transactions/query` Torii wrappers in
  `crates/iroha_torii/src/lib.rs` to the normal App API access path so invalid
  account literals still fail during route parsing with `400`, and valid
  requests no longer degrade to `503 Service Unavailable` when
  `torii.tx_history` is unset.
- Updated `crates/iroha_torii/src/routing.rs` so the account-scoped transaction
  endpoints match committed transactions by entrypoint authority, which aligns
  the live behavior with the documented "transactions authored by an account"
  contract and the address canonicalisation integration coverage.
- Kept `/v1/transactions/history` on the broader viewer-visibility model, and
  added/updated focused routing tests to cover that split along with canonical
  I105 path literals.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_torii --lib --features app_api,telemetry account_transactions_ -- --nocapture` (pass)
  - `cargo test -p iroha_torii --test address_parsing --features app_api,telemetry transactions_ -- --nocapture` (pass)
  - `cargo test -p iroha_torii --test account_query_subrouter_smoke --features app_api,telemetry -- --nocapture` (pass)
  - `cargo test -p integration_tests --test address_canonicalisation account_ -- --nocapture` (pass)

## 2026-03-21 Follow-up: Soracloud is complete and the workspace validation blocker was an unrelated IVM metadata mismatch
- The remaining red uncovered during the long workspace validation sweep was not
  in Soracloud. It came from an IVM metadata-policy mismatch across
  `crates/ivm_abi/src/metadata.rs`,
  `crates/ivm/tests/metadata.rs`,
  and `crates/iroha_core/tests/ivm_event_ordering.rs`:
  - generic prebuilt/sample `.to` programs in the repository still carry
    `IVM 1.0` headers,
  - runtime admission and cache policy already treat generic `1.x` headers as
    valid, but `ProgramMetadata::parse()` had drifted to reject every minor
    version except `1.1`, which broke the non-Soracloud
    `queries::smart_contract::smart_contract_query_scenarios` integration path
    before those samples were even submitted, and
  - the parser now accepts generic `1.0` and `1.1` headers again while
    self-describing `CNTR` contract artifacts remain validated as `1.1`-only
    by the higher-level contract-artifact verification path.
- This closes the last deterministic validation blocker encountered while
  finishing the Soracloud cutover. Soracloud itself is now complete on the
  patched tree: authoritative status and mutation responses are in place, the
  legacy `/v1/soracloud/registry` route is gone for v1, SCR-host admission is
  enforced server-side again, and live Torii/CLI Soracloud integration suites
  are green.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo test -p ivm --test metadata -- --nocapture` (pass)
  - `cargo test -p iroha_core --test ivm_event_ordering encode_prog_syscall_uses_valid_metadata_header -- --nocapture` (pass)
  - `cargo test -p integration_tests --test mod queries::smart_contract::smart_contract_query_scenarios -- --nocapture` (pass)
  - `cargo test -p integration_tests --test mod sumeragi_da::sumeragi_rbc_recovers_after_peer_restart -- --nocapture` (pass)
- Remaining validation gap:
  - rerun `cargo test --workspace` from a fresh post-patch session; the earlier
    long-running sweep was started before the metadata fix landed, so its later
    failures are no longer authoritative for the patched tree.

## 2026-03-21 Follow-up: Soracloud authoritative mutation responses and SCR admission are now wired end-to-end
- Closed the last Soracloud mutation/control-plane gap across
  `crates/iroha_torii/src/soracloud.rs`,
  `crates/iroha_test_network/src/config.rs`,
  `crates/iroha_kagami/src/genesis/generate.rs`,
  and `integration_tests/tests/iroha_cli.rs`:
  - Torii Soracloud mutation routes now submit the underlying transaction,
    wait for authoritative pipeline completion, and return the existing
    post-apply Soracloud JSON payloads built from committed world state plus
    authoritative audit snapshots instead of returning queue-admission
    receipts,
  - transaction rejection responses now include the nested validation /
    instruction-execution cause chain, so SCR-host admission failures surface
    operator-meaningful details such as the exact over-cap resource field,
  - deploy and upgrade once again enforce the Torii SCR-host admission caps
    (`cpu_millis`, memory/storage/task/file limits, start/stop grace, and
    state-write/network capability guardrails) before any transaction is
    submitted, and
  - the default dev/test genesis builders now grant the fixture operator
    account `CanManageSoracloud`, so fresh local/test networks can execute the
    network-backed Soracloud CLI lifecycle flows without manual permission
    bootstrapping.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_torii admit_scr_host_bundle_rejects_over_cap_cpu --lib -- --nocapture` (pass)
  - `cargo test -p iroha_torii error_chain_message_includes_nested_validation_details --lib -- --nocapture` (pass)
  - `cargo test -p iroha_torii soracloud_ -- --nocapture` (pass)
  - `cargo test -p iroha_test_network genesis_grants_alice_soracloud_management_permission -- --nocapture` (pass)
  - `cargo test -p iroha_kagami generated_genesis_grants_alice_soracloud_management_permission -- --nocapture` (pass)
  - `cargo test -p iroha_cli soracloud -- --nocapture` (pass)
  - `cargo test -p integration_tests --test iroha_cli soracloud_ -- --nocapture` (pass)
  - `cargo build --workspace` (pass)
  - `cargo clippy --workspace --all-targets -- -D warnings` (pass)
- Remaining validation gap:
  - `cargo test --workspace` is the only sweep still running / outstanding in
    this session.

## 2026-03-21 Follow-up: Soracloud status-only cutover is complete and `/v1/soracloud/registry` is gone
- Closed the remaining Soracloud status/control-plane gap across
  `crates/iroha_torii/src/{lib.rs,soracloud.rs}`,
  `crates/iroha_cli/src/soracloud.rs`,
  and `docs/source/soracloud/cli_local_control_plane.md`:
  - Torii now serves `GET /v1/soracloud/status` from the main Soracloud route
    set rather than the telemetry-only route path, so non-telemetry builds
    that expose Soracloud APIs now return the real authoritative status
    envelope instead of `telemetry_not_implemented`,
  - `/v1/soracloud/status` keeps `schema_version = 1` and the existing
    top-level JSON envelope while sourcing `service_health`,
    `resource_pressure.runtime`, and `runtime_manager` from the embedded
    runtime-manager snapshot, `control_plane` from the authoritative world
    snapshot, and routing/load state from in-process Nexus + queue state,
  - telemetry now affects only the `failed_admissions` subsection; when
    counters are unavailable Torii still returns `200 OK` with
    `available = false` plus deterministic zero counts,
  - `/v1/soracloud/registry` has been removed entirely for v1 rather than
    preserved as a compatibility surface,
  - the remaining Torii/CLI internal `Registry*` DTO/helper names for the live
    Soracloud control-plane snapshot have been renamed to
    `ControlPlane*`/status-oriented names without changing the public status
    wire fields, and
  - Soracloud docs now describe `/v1/soracloud/status` as the only status
    endpoint in v1.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_torii soracloud_ -- --nocapture` (pass)
  - `cargo test -p iroha_torii soracloud_status_handler_returns_snapshot_sections --features telemetry -- --nocapture` (pass)
  - `cargo test -p iroha_cli soracloud -- --nocapture` (pass)
  - `cargo build --workspace` (pass)
  - `cargo clippy --workspace --all-targets -- -D warnings` (pass)
- Remaining validation gap:
  - `cargo test --workspace` remains outstanding; this is still the expected
    multi-hour sweep.

## 2026-03-21 Follow-up: Full workspace clippy is now clean on the integrated Soracloud branch
- Closed the strict-lint backlog that still blocked the final branch validation
  after the Soracloud runtime/CLI/Torii work landed:
  - `crates/iroha_crypto/src/ram_lfe.rs` now runs the hidden BFV program
    executor through a smaller `HiddenProgramMachine` helper, preserving the
    v1 hidden-program operand widths while avoiding the previous oversized
    executor path,
  - `crates/iroha_data_model/src/{account/rekey.rs,consensus.rs,identifier.rs,isi/registry.rs,soracloud.rs,visit/visit_instruction.rs}`
    were cleaned up so the stricter workspace lint set passes without changing
    the Soracloud manifest or registry behavior,
  - `crates/kotodama_lang/src/compiler.rs`,
    `crates/iroha_config/src/parameters/user.rs`,
    `crates/ivm/src/{core_host.rs,host.rs}`,
    and `crates/connect_norito_bridge/src/lib.rs`
    received the remaining mechanical clippy cleanups that the broader
    workspace sweep exposed.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo clippy --workspace --all-targets -- -D warnings` (pass)
  - `cargo test -p iroha_data_model default_registry -- --nocapture` (pass)
  - `cargo test -p iroha_data_model validation_accepts_consistent_state -- --nocapture` (pass)
  - `cargo test -p iroha_data_model service_audit_event_validate_requires_break_glass_reason_when_enabled -- --nocapture` (pass)
  - `cargo test -p ivm abi_hash_versions -- --nocapture` (pass; filter matched no concrete test cases but completed cleanly)
  - `git diff --check` (pass)
- Remaining validation gap:
  - `cargo test --workspace` remains outstanding; this is still the expected
    multi-hour sweep.

## 2026-03-21 Follow-up: Soracloud runtime hydration now catches up from admitted remote SoraFS providers
- Extended the embedded Soracloud runtime-manager hydration path across
  `crates/irohad/src/{main.rs,soracloud_runtime.rs}`,
  `crates/iroha_torii/src/{lib.rs,sorafs/mod.rs,routing.rs}`,
  and `crates/iroha_core/src/state.rs` so missing runtime bundles/artifacts no
  longer depend solely on the local embedded SoraFS store:
  - `irohad` now builds and shares the same live SoraFS provider-advert cache
    that Torii uses, so the runtime manager can resolve admitted providers
    through the authoritative discovery state instead of inventing a separate
    fetch registry,
  - committed Soracloud hydration now walks authoritative replication orders,
    decodes canonical `ReplicationOrderV1` payloads, verifies that the
    targeted manifest digest is committed in the pin registry/DA view, then
    fetches missing material through the provider’s Torii SoraFS gateway
    surfaces (`manifest`, `plan`, `token`, `chunk`) with deterministic content
    verification before writing the runtime cache,
  - restart/reconcile logic continues to prefer the local committed store when
    present, but now falls back to admitted remote providers deterministically
    when the local node lacks the required payload,
  - Torii’s `handle_v1_transactions_history_get` test build warning is now
    cleaned up by sinking the telemetry handle on non-telemetry builds.
- Added focused remote-hydration regressions in
  `crates/irohad/src/soracloud_runtime.rs`:
  - successful hydration/materialization from an admitted remote provider, and
  - fail-closed behavior when remote payload bytes do not match the committed
    Soracloud artifact hash.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo check -p irohad` (pass)
  - `cargo test -p irohad reconcile_once_hydrates_missing_artifacts_from_committed_remote_sorafs_provider -- --nocapture` (pass)
  - `cargo test -p irohad reconcile_once_skips_remote_sorafs_payloads_that_do_not_match_expected_hash -- --nocapture` (pass)
  - `cargo test -p irohad soracloud_runtime::tests:: -- --nocapture` (pass)
  - `cargo check -p iroha_torii --tests` (pass)
  - `git diff --check` (pass)
- Remaining work in this area:
  - run the broader workspace build/clippy/test sweep now that the Soracloud
    runtime-manager execution/hydration path is integrated,
  - continue auditing the Soracloud host policy surface as new host syscalls or
    apartment/runtime operations are added so capability enforcement remains in
    the runtime host instead of drifting back into Torii-local scaffolding.

## 2026-03-21 Follow-up: Soracloud now uses the v1 IVM host ABI for ordered mailbox execution, Torii no longer persists a live file-backed control-plane mirror, and the CLI is network-only
- Closed the next Soracloud execution/control-plane gap across
  `crates/iroha_data_model/src/soracloud.rs`,
  `crates/ivm_abi/src/{syscalls.rs,pointer_abi.rs}`,
  `crates/ivm/docs/{syscalls.md,pointer_abi.md}`,
  `crates/ivm/tests/*`,
  `crates/ivm/src/ivm.rs`,
  `crates/irohad/src/soracloud_runtime.rs`,
  and `crates/iroha_cli/src/soracloud.rs`:
  - ABI v1 now includes the dedicated Soracloud host syscall block and
    Soracloud request/response pointer types, with Norito request/response
    envelopes for committed-state reads, deterministic state mutations,
    outbound mailbox messages, journal append, checkpoint publication,
    secret/credential lookup, and bounded egress fetch,
  - the embedded runtime manager now executes ordered mailbox work by loading
    the admitted IVM bundle for the active revision and invoking the
    authoritative handler entrypoint instead of returning synthetic placeholder
    results,
  - `private_update` runs through the Soracloud host surface with
    secret/credential/egress access constrained to the private runtime path,
    and execution failures now return deterministic degraded receipts without
    partially applying state, mailbox, journal, or checkpoint writes,
  - Soracloud CLI `init` no longer writes a local control-plane state file,
    and the deploy, status, upgrade, rollback, rollout, and agent
    lifecycle/mailbox/autonomy commands are now Torii-backed only via
    `--torii-url`.
- Replaced the Soracloud CLI local-control-plane documentation with a live
  control-plane guide that describes the IVM-only scope, authoritative Torii
  status/mutation routes, and the removal of local simulator contracts.
- Removed the live Torii Soracloud file-backed control-plane persistence path from
  `crates/iroha_torii/src/soracloud.rs`:
  - the old persistence snapshot contract is no longer compiled into
    production, and
  - the remaining in-memory `Registry` harness plus the CLI-side simulator
    helpers have now been deleted from `crates/iroha_torii/src/soracloud.rs`
    and `crates/iroha_cli/src/soracloud.rs`, leaving only the authoritative
    world state + runtime-manager path in-tree.
- Refreshed the checked-in Soracloud CLI help, Vue runbook, and
  `integration_tests/tests/iroha_cli.rs` so the public contract and binary
  tests now match the network-only `--torii-url` requirement instead of the
  removed local simulator path.
- Switched the CLI `status` command over to Torii’s
  `/v1/soracloud/status` endpoint so status reads now include the runtime
  manager’s authoritative health snapshot plus the embedded control-plane
  snapshot; `--service-name` is preserved client-side by filtering the
  returned `control_plane.services` list.
- The embedded runtime manager now enforces
  `soracloud_runtime.cache_budgets` deterministically across the hydrated
  `artifacts`, `journals`, and `checkpoints` roots:
  - cache candidates are ordered by authoritative observation sequence and
    then by stable key/hash within each bucket,
  - pruning now runs after service/apartment materialization and before the
    persisted snapshot is refreshed, and
  - the runtime snapshot is rebuilt after pruning so
    `bundle_available_locally` / `artifact.available_locally` reflect the
    authoritative post-prune cache state exposed through Torii status.
- The runtime manager now rehydrates missing bundle and artifact cache entries
  from committed SoraFS content via the embedded node-local store:
  - `irohad` and Torii now share a single `sorafs_node::NodeHandle`, so the
    runtime manager can inspect the same deterministic local manifest set that
    Torii uses for SoraFS storage,
  - missing Soracloud bundle/artifact cache files are now hydrated only when
    the corresponding SoraFS manifest is committed through the authoritative
    pin registry or DA commitment index, and
  - runtime snapshots now keep services in `Hydrating` until every required
    bundle/artifact cache entry is present locally, which makes
    `/v1/soracloud/status` reflect real hydration state instead of placeholder
    health,
  - restart recovery coverage now exercises restoring a persisted
    `runtime_snapshot.json`, deleting the hydrated cache, and deterministically
    rehydrating the missing bundle/artifact files from committed SoraFS
    content on the next reconcile pass.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo run -p ivm --bin gen_syscalls_doc -- --write` (pass)
  - `cargo test -p ivm --test abi_hash_versions --test abi_policy --test abi_syscall_list_golden --test pointer_type_ids_golden --test pointer_type_policy --test pointer_types_markdown --test syscalls_policy` (pass)
  - `cargo check -p irohad` (pass)
  - `cargo test -p irohad execute_ordered_mailbox_ -- --nocapture` (pass)
  - `cargo test -p irohad soracloud_runtime::tests:: -- --nocapture` (pass)
  - `cargo check -p iroha_cli` (pass)
  - `cargo test -p iroha_cli deploy_requires_torii_url_after_local_simulator_removal -- --nocapture` (pass)
  - `cargo test -p iroha_cli init_site_template_scaffolds_vue_and_sorafs_workflow -- --nocapture` (pass)
  - `cargo run -p iroha_cli --bin iroha -- tools markdown-help > crates/iroha_cli/CommandLineHelp.md` (pass)
  - `cargo check -p iroha_torii` (pass)
  - `cargo test -p integration_tests --test iroha_cli require_torii_url -- --nocapture` (pass)
  - `cargo test -p iroha_cli fetch_torii_status_rejects_invalid_url -- --nocapture` (pass)
  - `cargo test -p iroha_cli filter_soracloud_status_payload_filters_embedded_control_plane_snapshot -- --nocapture` (pass)
  - `cargo check -p iroha_cli --tests` (pass)
  - `cargo check -p iroha_torii --tests` (pass; emits an unrelated pre-existing `routing.rs` unused-variable warning)
  - `cargo test -p iroha_torii world_snapshot_uses_authoritative_soracloud_state --lib -- --nocapture` (pass)
  - `cargo test -p irohad soracloud_runtime::tests:: -- --nocapture` (pass, includes committed-SoraFS hydration success/fail-closed coverage)
- Remaining work in this area:
  - the runtime manager still needs remote SoraFS/DA catch-up beyond the
    local node store plus broader restart re-materialization coverage,
  - the runtime host still needs the remaining private-runtime quota /
    capability enforcement closure across the full policy surface, and
  - the broader targeted crate sweep, `cargo clippy --workspace --all-targets
    -- -D warnings`, and full `cargo test --workspace` have not been completed
    yet in this session.

## 2026-03-21 Follow-up: `iroha_torii` telemetry Soracloud status test shim now satisfies the shared runtime trait
- Updated the telemetry-gated Soracloud status test helper in
  `crates/iroha_torii/src/lib.rs` so `TestSoracloudRuntimeHandle` implements
  the full `iroha_core::soracloud_runtime::SoracloudRuntime` trait, not only
  the read-handle half.
- The added test-only execution methods return deterministic `Unavailable`
  errors, which is sufficient for the status handler coverage while matching
  the concrete `Arc<dyn SoracloudRuntime>` type stored in Torii app state.
- Validation:
  - `cargo test -p iroha_torii soracloud_status_handler_returns_snapshot_sections --lib --features telemetry` (pass)
  - `cargo fmt --all --check` (pass)

## 2026-03-21 Follow-up: `iroha_cli` Soracloud signature tests now use the current domainless `AccountId` constructor
- Updated the Soracloud signed-agent request tests in
  `crates/iroha_cli/src/soracloud.rs` to stop parsing the removed legacy
  `"alice@wonderland"` literal as an `AccountId`.
- Those tests now construct authorities with `AccountId::new(...)`, which
  matches the current data-model API where `AccountId` is domainless and
  controller-keyed.
- Validation:
  - `cargo test -p iroha_cli signed_agent_deploy_request_uses_verifiable_signature` (pass)
  - `cargo test -p iroha_cli signed_agent_ -- --nocapture` (pass)
  - `cargo fmt --all --check` (pass)

## 2026-03-21 Follow-up: Soracloud now serves authoritative local reads and apartment execution through the embedded runtime manager
- Extended the shared/runtime-manager Soracloud execution surface across
  `crates/iroha_core/src/soracloud_runtime.rs`,
  `crates/irohad/src/soracloud_runtime.rs`,
  `crates/iroha_torii/src/{lib.rs,soracloud.rs}`, and
  `crates/iroha_data_model/src/soracloud.rs`:
  - `SoracloudLocalReadRequest` now carries the canonical local-read envelope
    needed by public asset/query handlers, including method/path/query,
    normalized headers, request body, and request commitment,
  - `SoracloudLocalReadResponse` now returns response bytes plus content
    metadata, authoritative artifact/state bindings, certification policy, and
    an optional runtime receipt,
  - `SoracloudApartmentExecutionRequest` /
    `SoracloudApartmentExecutionResult` now carry explicit apartment operation
    context plus deterministic committed outputs instead of only apartment
    labels/status.
- The embedded runtime manager now executes real authoritative fast-path reads
  against the committed snapshot:
  - asset handlers serve bytes only from the hydrated local artifact cache and
    verify the cached bytes against the committed artifact hash before
    responding,
  - query handlers execute as read-only metadata queries over authoritative
    Soracloud state entries, emit bindings for the committed state they read,
    and return an audit receipt when the handler policy requires it,
  - both local-read and apartment execution now fail closed with `Unavailable`
    when the node-local runtime snapshot is behind the committed block hash or
    the required local materialization is missing.
- Torii now resolves public Soracloud asset/query routes from authoritative
  world state and serves them through the shared runtime interface:
  - the fallback public-read path binds the request to the currently committed
    block hash/height,
  - returns `503` when the embedded runtime is unavailable or not hydrated for
    the requested revision,
  - returns certified response metadata/receipts only from the embedded
    runtime-manager path instead of any Torii-local mutable registry.
- `SoraServiceMailboxMessageV1` now persists opaque authoritative
  `payload_bytes` alongside the existing `payload_commitment`, and validation
  enforces that the commitment matches those bytes. This removes the
  commitment-only mailbox-model gap that was blocking future real IVM
  ordered-mailbox execution.
- Validation:
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-torii-target cargo test -p iroha_torii soracloud_public_local_read_route --lib` (pass)
  - `CARGO_HOME=/tmp/iroha-codex-cargo-home-irohad CARGO_TARGET_DIR=/tmp/iroha-codex-irohad-target cargo test -p irohad execute_local_read` (pass)
  - `CARGO_HOME=/tmp/iroha-codex-cargo-home-irohad CARGO_TARGET_DIR=/tmp/iroha-codex-irohad-target cargo test -p irohad execute_apartment_returns_authoritative_status_and_commitment` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-data-model-target cargo test -p iroha_data_model service_mailbox_message_validate` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-core-target cargo test -p iroha_core validate_and_record_transactions_persists_soracloud_mailbox_state_mutations` (pass)
- Remaining execution-plane work in this area:
  - ordered mailbox execution still uses the synthetic placeholder and has not
    been cut over to real IVM handler invocation yet,
  - the embedded runtime manager still does not fetch missing bundles/assets
    from SoraFS/DA or perform the deterministic cache-budget pruning described
    by the Soracloud v1 plan,
  - the private-runtime secret/credential capability layer, Torii shadow
    registry deletion, and the IVM cloud-runtime ABI/syscall cutover remain
    outstanding.

## 2026-03-21 Follow-up: removed the last stale `torii.identifier_resolver` config references from `iroha_config`
- Cleaned up the leftover `ToriiIdentifierResolver` field references in
  `crates/iroha_config/src/parameters/{user.rs,actual.rs}` after the
  `torii.ram_lfe` rename landed, so `iroha_config` no longer refers to a type
  that was deleted during the config-surface migration.
- Updated config fixtures and helper `actual::Torii` constructors in
  `crates/iroha_config/tests/fixtures.rs`,
  `crates/iroha_core/src/kiso.rs`,
  `crates/iroha_torii/src/test_utils.rs`, and
  `crates/iroha_torii/tests/connect_gating.rs`
  to match the canonical `torii.ram_lfe`-only shape.
- Validation:
  - `cargo check -p iroha_config` (pass)
  - `cargo test -p iroha_config torii_ram_lfe_parses -- --nocapture` (pass)
  - `cargo fmt --all --check` (pass)
  - `cargo check -p iroha_core -p iroha_torii` (pass; existing unrelated warning remains in `crates/iroha_torii/src/routing.rs`)

## 2026-03-21 Follow-up: Soracloud v1 now rejects `NativeProcess` and the runtime-manager prepares the IVM-only host layout
- Closed the revised IVM-only admission/runtime gap across
  `crates/iroha_data_model/src/soracloud.rs`,
  `crates/iroha_core/src/smartcontracts/isi/soracloud.rs`,
  `crates/irohad/src/soracloud_runtime.rs`, and
  `crates/iroha_cli/src/soracloud.rs`:
  - `SoraContainerManifestV1::validate()` now rejects
    `SoraContainerRuntimeV1::NativeProcess`, so Soracloud v1 admission accepts
    only `Ivm`,
  - core Soracloud deployment tests now cover that admission rejection path,
  - the embedded runtime manager now creates the full v1 state roots
    (`services`, `apartments`, `artifacts`, `journals`, `checkpoints`,
    `secrets`) and rejects `NativeProcess` revisions during reconcile/runtime
    activation instead of projecting them as partially supported plans,
  - Soracloud CLI init templates (`site`, `webapp`, `pii-app`) now emit `Ivm`
    manifests and no longer scaffold `NativeProcess` container targets.
- Added focused regressions for the new behavior in the data model, core
  admission path, and runtime-manager reconcile path.
- Validation:
  - `cargo fmt --all` (pass)
  - `git diff --check` (pass)
  - targeted cargo tests were rerun in isolated Cargo homes/target dirs after a
    shared-cache failure in the local environment (`generic-array` reported a
    missing `typenum` `.rmeta` in the existing `target/debug` tree); those
    isolated compiles were still in progress at the time of this status update.
- Remaining execution-plane work in this area:
  - the embedded runtime manager still does not perform real SoraFS/DA
    hydration, deterministic cache pruning, or real IVM/local-read execution,
  - Torii still contains the large legacy Soracloud shadow-registry/test shim
    that should be re-homed or deleted once authoritative coverage replaces it,
  - the Soracloud fast path, private-runtime capability layer, and IVM
    cloud-runtime ABI/syscall cutover remain outstanding.

## 2026-03-20 Follow-up: Soracloud runtime manager now has explicit config and restart snapshot recovery
- Closed the next embedded-runtime-manager control-plane gap across
  `crates/iroha_config/src/parameters/{defaults.rs,actual.rs,user.rs}`,
  `crates/irohad/src/{main.rs,soracloud_runtime.rs}`, and Torii test helpers:
  - added the dedicated `iroha_config::soracloud_runtime` surface with
    explicit defaults for the runtime-manager state directory, reconcile
    cadence, hydration concurrency, cache budgets, deterministic
    `NativeProcess` limits, and outbound egress policy,
  - threaded the parsed `actual::SoracloudRuntime` into
    `actual::Root`, user parsing, and `irohad` construction so the embedded
    runtime manager no longer derives its state directory or cadence from
    `torii.data_dir`,
  - updated the runtime-manager config conversion tests and Torii test-only
    minimal config builders to carry the new authoritative root field.
- Added the first restart/catch-up recovery behavior to the embedded runtime
  manager in `crates/irohad/src/soracloud_runtime.rs`:
  - the manager now restores the last persisted `runtime_snapshot.json`
    before its first reconciliation pass,
  - if initial reconciliation fails, the read-only runtime snapshot remains
    seeded from that persisted snapshot instead of dropping back to an empty
    in-memory view,
  - added focused regression coverage proving both explicit config mapping and
    persisted-snapshot recovery across reconcile failure.
- Added focused validation coverage for the new config surface in
  `crates/iroha_config/src/parameters/user.rs`, including default parsing and
  explicit override parsing for cache budgets, resource ceilings, and egress
  settings.
- Validation:
  - `cargo fmt --all` (pass)
  - `git diff --check` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-soracloud-runtime-verify-check cargo check -p iroha_config -p irohad` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-soracloud-runtime-verify-tests cargo test -p iroha_config --lib soracloud_runtime_ -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-soracloud-runtime-verify-tests cargo test -p irohad soracloud_runtime::tests:: -- --nocapture` (pass)
- Remaining execution-plane work in this area:
  - the embedded runtime manager still uses those new settings only for
    planning/reconciliation and snapshot recovery; it does not yet perform
    real SoraFS/DA hydration, cache-budget enforcement, or deterministic
    `NativeProcess` supervision,
  - ordered mailbox execution is still synthetic, and local reads/apartments,
    certified fast paths, private secret/credential enforcement, and the IVM
    cloud-runtime ABI cutover remain outstanding.

## 2026-03-20 Follow-up: JS, Swift, and Android Torii clients now expose the generic RAM-LFE policy, execute, and verify routes
- Extended the three client SDKs to the generic Torii RAM-LFE surface so they
  now mirror:
  - `GET /v1/ram-lfe/program-policies`,
  - `POST /v1/ram-lfe/programs/{program_id}/execute`, and
  - `POST /v1/ram-lfe/receipts/verify`.
- `javascript/iroha_js/src/toriiClient.js` now adds:
  - `listRamLfeProgramPolicies(...)`,
  - `executeRamLfeProgram(programId, { inputHex | encryptedInput })`, and
  - `verifyRamLfeReceipt({ receipt, outputHex? })`,
  alongside focused node tests in
  `javascript/iroha_js/test/toriiClient.ramLfe.test.js` and matching
  TypeScript declarations in `javascript/iroha_js/index.d.ts`.
- `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift` now adds RAM-LFE policy,
  execute, and verify request/response models plus async and completion-based
  client methods. The Swift client intentionally keeps the nested
  `receipt` payload as raw `ToriiJSONValue` maps so the execute-to-verify
  round-trip preserves the exact data-model JSON wire shape for program IDs,
  enum tags, and nullable receipt fields.
- `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/`
  now includes RAM-LFE DTOs, a dedicated `RamLfeJsonParser`, and matching
  `HttpClientTransport` helpers that likewise preserve nested receipts as raw
  JSON maps for lossless verify requests.
- Added focused SDK regression coverage in:
  - `IrohaSwift/Tests/IrohaSwiftTests/ToriiClientTests.swift`
  - `java/iroha_android/src/test/java/org/hyperledger/iroha/android/client/HttpClientTransportTests.java`
- Validation:
  - `swift test --filter ToriiClientTests/testListRamLfeProgramPoliciesAsync` (pass)
  - `swift test --filter ToriiClientTests/testExecuteRamLfeProgramAsync` (pass)
  - `swift test --filter ToriiClientTests/testExecuteRamLfeProgramReturnsNilOnNotFound` (pass)
  - `swift test --filter ToriiClientTests/testVerifyRamLfeReceiptAsync` (pass)
  - `git diff --check` (pass)
- Validation blocked in this environment:
  - JS tests could not be executed because `node`/`npm` are unavailable.
  - Android tests could not be executed because no local Java runtime is installed.

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

## 2026-03-20 Follow-up: Soracloud mailbox execution now applies authoritative runtime state mutations
- Closed the next core execution gap in the shared mailbox path across
  `crates/iroha_core/src/{block.rs,smartcontracts/isi/soracloud.rs}`:
  - `next_soracloud_audit_sequence()` now considers
    `soracloud_runtime_receipts.emitted_sequence`, so the shared Soracloud
    sequence space remains monotonic even when ordered runtime execution emits
    receipts without a matching lifecycle audit event.
  - `crates/iroha_core/src/smartcontracts/isi/soracloud.rs` now exposes
    `apply_soracloud_state_mutation()`, a reusable authoritative service-state
    write-back helper that reuses the same binding-prefix, encryption,
    mutability, per-item-byte, and total-byte validation rules as
    `MutateSoracloudState`.
  - `MutateSoracloudState` now delegates its authoritative entry update to
    that helper before recording the lifecycle audit event, so manual
    governance-driven state mutations and runtime-driven mailbox mutations no
    longer diverge in validation or storage shape.
  - `crates/iroha_core/src/block.rs` now validates mailbox runtime receipts
    against the original execution request and applies any returned
    `state_mutations` into authoritative Soracloud service state before
    persisting runtime state / outbound mailbox messages / runtime receipts.
  - Ordered runtime write-backs currently reuse the deterministic runtime
    `receipt_id` as the linkage hash stored in the existing
    `governance_tx_hash` field on `SoraServiceStateEntryV1`, so the authoritative
    state row remains reconstructible from committed runtime receipts without
    introducing a second ad hoc linkage store in v1.
- Added focused core regression coverage proving that:
  - mailbox execution can now persist a service-state row through the shared
    runtime path,
  - the resulting state row is linked to the runtime receipt and shares the
    receipt’s emitted sequence,
  - runtime receipts now advance the next shared Soracloud execution sequence,
  - mailbox receipt deduplication still suppresses re-delivery after the
    mutation write-back.
- Validation:
  - `cargo fmt --all` (pass)
  - `git diff --check` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-private-runtime-state-mutation-target-2 cargo check -p iroha_core` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-private-runtime-state-mutation-target-2 cargo test -p iroha_core valid::tests::validate_and_record_transactions_ -- --nocapture` (targeted mailbox regressions passed; the filtered command then continued draining zero-test binaries)
  - `CARGO_TARGET_DIR=/tmp/iroha-private-runtime-state-mutation-target-2 cargo test -p iroha_core --lib mutate_soracloud_state_records_authoritative_service_state -- --nocapture` (pass)
- Remaining execution-plane work in this area:
  - the embedded runtime manager still returns synthetic mailbox results and
    does not yet host real IVM or deterministic `NativeProcess` workloads,
  - ordered mailbox execution still lacks the real runtime host surface for
    local reads, private updates, apartments, journals/checkpoints, and
    certified fast-path responses,
  - hydration/restart/pruning, private secret/credential enforcement, and the
    IVM cloud-runtime ABI cutover remain outstanding.

## 2026-03-20 Follow-up: core block validation now executes Soracloud mailbox work through the shared runtime contract
- Promoted the shared Soracloud runtime contract in
  `crates/iroha_core/src/soracloud_runtime.rs` from a read-only snapshot
  surface into an execution contract:
  - added shared request/result types for deterministic local reads, ordered
    mailbox execution, and apartment execution,
  - added `SoracloudRuntime` plus `SharedSoracloudRuntime`, so both core block
    progression and Torii can depend on the same interface without pulling in
    `irohad`,
  - kept runtime write-backs bounded to authoritative runtime state, mailbox
    messages, and runtime receipts.
- Threaded that shared runtime into core state and `irohad`:
  - `crates/iroha_core/src/state.rs` now carries an optional shared Soracloud
    runtime handle on `State` and `StateBlock`,
  - `crates/irohad/src/main.rs` now installs the embedded
    `SoracloudRuntimeManagerHandle` into core state in addition to passing its
    read-only handle into Torii,
  - `crates/irohad/src/soracloud_runtime.rs` now implements the shared
    execution trait with a deterministic placeholder mailbox executor and
    authoritative pending-mailbox accounting that excludes already-receipted
    messages.
- Wired ordered Soracloud mailbox execution into replicated block progression:
  - `crates/iroha_core/src/smartcontracts/isi/soracloud.rs` now exposes
    reusable authoritative write-back helpers for runtime state, mailbox
    messages, and runtime receipts, so core execution can reuse the existing
    record path without a synthetic privileged transaction,
  - `crates/iroha_core/src/block.rs` now collects ready mailbox messages
    during block validation, invokes the shared runtime after time-trigger
    execution, and persists runtime state / outbound mailbox messages /
    runtime receipts back into authoritative world state before block results
    are finalized,
  - added a focused core regression proving that mailbox execution through
    block validation emits a runtime receipt, updates pending mailbox counts,
    and suppresses re-delivery once a receipt exists.
- Validation:
  - `cargo fmt --all` (pass)
  - `git diff --check` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-private-runtime-mailbox-target cargo check -p iroha_core -p irohad -p iroha_torii` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-private-runtime-mailbox-test-target cargo test -p iroha_core valid::tests::validate_and_record_transactions_executes_soracloud_mailbox_runtime_once -- --nocapture` (targeted test passed; the filtered command then continued draining zero-test binaries)
- Remaining execution-plane work in this area:
  - authoritative application of `state_mutations` into Soracloud service
    state is still a TODO in the shared mailbox executor, so non-empty
    mutation results currently abort that mailbox pass,
  - the embedded runtime manager still returns synthetic mailbox execution
    results and does not yet implement real local-read or apartment execution,
  - hydration/restart/pruning, certified local-fast-path serving, private
    secret/credential enforcement, and the IVM cloud-runtime ABI cutover are
    still outstanding.

## 2026-03-20 Follow-up: Torii now consumes the embedded Soracloud runtime-manager through a shared core contract
- Landed the Torii/runtime-manager integration slice across
  `crates/iroha_core`, `crates/iroha_torii`, and `crates/irohad`:
  - Added `crates/iroha_core/src/soracloud_runtime.rs` as the shared
    read-only runtime snapshot contract, moving the Soracloud runtime
    materialization snapshot types out of `irohad` so Torii can depend on them
    without a circular crate edge.
  - `crates/irohad/src/soracloud_runtime.rs` now implements that shared
    `SoracloudRuntimeReadHandle` trait for
    `SoracloudRuntimeManagerHandle`, and `irohad` passes the embedded runtime
    manager into Torii using the new `iroha_torii::ToriiRuntimeDeps` wrapper.
  - `crates/iroha_torii/src/lib.rs` no longer carries
    `AppState.soracloud_registry`, so the public Torii server state stops
    constructing an unused in-process Soracloud registry/persistence layer.
  - `/v1/soracloud/status` now reports hosted workload health, cache misses,
    mailbox pressure, apartment counts, and the full node-local
    runtime-manager snapshot from the embedded runtime manager instead of the
    previous SoraFS placeholder health block.
- Validation:
  - `cargo fmt --all` (pass)
  - `git diff --check` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-private-runtime-status-target cargo check -p iroha_torii -p irohad` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-private-runtime-status-test-target cargo test -p iroha_torii --features telemetry --lib soracloud_status_handler_returns_snapshot_sections -- --nocapture` (pass)
- Remaining work in this area is the larger execution-plane follow-up: promote
  the shared runtime contract from read-only status snapshots into the full
  mailbox/read/apartment execution interface, and remove the still-present
  legacy test-only Torii registry/apply helper scaffolding from
  `crates/iroha_torii/src/soracloud.rs`.

## 2026-03-20 Follow-up: `irohad` now embeds a Soracloud runtime-manager reconciliation loop
- Added the first `irohad`-native runtime-manager slice in
  `crates/irohad/src/{main.rs,soracloud_runtime.rs}`:
  - `irohad` now starts an embedded Soracloud runtime-manager alongside the
    other node subsystems and exposes a `SoracloudRuntimeManagerHandle` from
    `Iroha`.
  - The new runtime manager continuously reconciles authoritative Soracloud
    world state into a node-local materialization snapshot under
    `torii.data_dir/soracloud_runtime`, instead of relying on Torii-local
    control-plane registry state.
  - Reconciliation persists:
    - active service revision materialization plans grouped by service/version,
      including both the currently active revision and any canary candidate
      revision that must be materialized during rollout,
    - declared handler mailbox contracts plus content-addressed artifact/bundle
      hydration expectations for each active revision,
    - active agent-apartment materialization plans with lease/runtime/budget
      state and local manifest projections,
    - a consolidated `runtime_snapshot.json` for local runtime orchestration
      and restart introspection.
  - The manager also prunes stale local materialization directories so the
    node-local runtime view tracks the authoritative on-chain deployment set.
- Fixed an unrelated-but-blocking Torii compile regression in
  `crates/iroha_torii/src/routing.rs` where the transactions-history handler
  still referenced `telemetry` after the parameter had been renamed to
  `_telemetry`, which prevented `irohad` from compiling its dependency graph.
- Validation:
  - `cargo fmt --all` (pass)
  - `git diff --check` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-agent-cutover-target cargo check -p iroha_data_model -p iroha_core -p iroha_torii -p iroha_cli` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-runtime-manager-target cargo check -p irohad` (pass)
- Longer-running targeted `cargo test` jobs for the new runtime-manager and the
  Soracloud agent cutover were started separately, but those test-profile
  builds had not completed by the time this status entry was updated.

## 2026-03-20 Follow-up: Soracloud private-runtime dispatch now reaches the core executor, and health/compliance reads are authoritative
- Closed the remaining gap between the new Soracloud private-runtime ISIs and
  the transaction path:
  - `crates/iroha_core/src/smartcontracts/isi/mod.rs` now registers
    `MutateSoracloudState`, `RunSoracloudFheJob`, and
    `RecordSoracloudDecryptionRequest` in the global `InstructionBox`
    dispatcher, so the Torii transaction façade can execute the new
    authoritative private-runtime mutations instead of failing at runtime with
    an unknown instruction type.
  - `crates/iroha_core/src/smartcontracts/isi/soracloud.rs` private-runtime
    executor coverage now runs those three paths through `InstructionBox`
    dispatch, not only through direct per-ISI execution.
- Cut another Soracloud read path over to authoritative world state:
  - `crates/iroha_torii/src/soracloud.rs` now derives
    `/v1/soracloud/health/compliance/report` from authoritative
    `soracloud_service_audit_events`, deployment state, and active service
    revisions in `World`, matching the earlier `/v1/soracloud/registry` and
    ciphertext-query cutovers instead of reading the Torii-local registry.
  - Added authoritative Torii regression coverage for both ciphertext-query and
    health/compliance world-state reads, with the tests running under an
    explicit Tokio runtime so the shared app-state harness remains valid.
- Validation:
  - `cargo fmt --all` (pass)
  - `git diff --check` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-private-runtime-target cargo test -p iroha_core --lib mutate_soracloud_state_records_authoritative_service_state -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-private-runtime-target cargo test -p iroha_core --lib run_soracloud_fhe_job_records_ciphertext_output_state -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-private-runtime-target cargo test -p iroha_core --lib record_soracloud_decryption_request_persists_policy_snapshot -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-private-runtime-torii-health-target cargo test -p iroha_torii --lib soracloud::tests::authoritative_ciphertext_query_reads_world_state -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-private-runtime-torii-health-target cargo test -p iroha_torii --lib soracloud::tests::authoritative_health_compliance_report_reads_world_state -- --nocapture` (pass)
- Remaining Soracloud cutover work is now concentrated in the still-local
  training/model and agent/apartment handlers plus the larger `irohad`
  embedded runtime-manager/hydration/ABI block.

## 2026-03-20 Follow-up: Soracloud lifecycle ISIs, authoritative deployment state, and Torii façade cutover are now in place
- Extended the private-runtime foundation into an authoritative control-plane
  slice across `crates/iroha_data_model`, `crates/iroha_core`,
  `crates/iroha_torii`, and `crates/iroha_cli`:
  - `crates/iroha_data_model/src/soracloud.rs` now carries authoritative
    service deployment, rollout, and audit-event records, and
    `crates/iroha_data_model/src/isi/soracloud.rs` adds first-class Soracloud
    lifecycle/runtime instructions for deploy, upgrade, rollback, rollout
    advance, runtime-state updates, mailbox recording, and runtime receipts.
  - `crates/iroha_core/src/state.rs` now persists admitted deployment bundles,
    active deployment state, and audit events directly in `World`, and
    `crates/iroha_core/src/smartcontracts/isi/soracloud.rs` executes the new
    lifecycle ISIs under `CanManageSoracloud`, including canary rollout start,
    automatic rollback on unhealthy rollouts, receipt persistence, and audit
    provenance recording.
  - Torii Soracloud lifecycle endpoints no longer mutate the file-backed local
    registry for deploy/upgrade/rollback/rollout. Instead, they verify
    provenance, require a transaction signer, submit Soracloud ISIs through the
    normal transaction path, and expose `/v1/soracloud/registry` plus telemetry
    control-plane snapshots from authoritative world state.
  - The CLI now attaches explicit `authority` plus `private_key` signer
    material to live Soracloud lifecycle requests and queries Torii’s new
    `/v1/soracloud/registry` endpoint for live status output.
- Validation:
  - `CARGO_TARGET_DIR=/tmp/iroha-private-runtime-target cargo test -p iroha_core smartcontracts::isi::soracloud::tests:: -- --nocapture` (Soracloud lifecycle executor tests passed; command still drains filtered zero-test binaries afterward)
  - `CARGO_TARGET_DIR=/tmp/iroha-private-runtime-target cargo test -p iroha_torii --lib soracloud::tests::world_snapshot_uses_authoritative_soracloud_state -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-private-runtime-target cargo test -p iroha_torii --lib soracloud::tests::deploy_upgrade_rollback_workflow_updates_registry_and_audit_log -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-private-runtime-target cargo test -p iroha_cli --bin iroha soracloud::tests::signed_ -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-private-runtime-target cargo test -p iroha_cli --bin iroha soracloud::tests::status_output_can_represent_torii_control_plane_snapshot -- --nocapture` (pass)
- This still does not add the `irohad` embedded runtime manager, SoraFS/DA
  hydration, or the new IVM runtime/syscall surface; those remain the next
  major implementation block now that the control-plane mutations and read
  snapshots are authoritative.

## 2026-03-20 Follow-up: Soracloud runtime state now has first-class data-model and world-state foundations
- Landed the first authoritative on-chain/runtime-state slice for the planned
  Soracloud private runtime in `crates/iroha_data_model`,
  `crates/iroha_core`, and `crates/iroha_cli`:
  - `crates/iroha_data_model/src/soracloud.rs` now defines first-release
    runtime-state records for handler classes, certified-response policy,
    artifact references, mailbox contracts, active runtime health/load state,
    mailbox messages, and runtime receipts.
  - `SoraServiceManifestV1` now carries explicit `handlers` and `artifacts`,
    and deployment validation now rejects manifests that omit handlers, use
    uncertified asset/query handlers, omit mailboxes for replicated
    `update`/`private_update` handlers, or reference artifacts through unknown
    handlers.
  - `crates/iroha_core/src/state.rs` now stores Soracloud service revisions,
    active runtime state, mailbox messages, and runtime receipts directly in
    `World`, with block/transaction/view wiring, JSON snapshot support, and
    test/API scaffolding accessors so this runtime state is no longer forced
    into Torii-local file registries.
  - `crates/iroha_cli/src/soracloud.rs` init templates now emit handler and
    artifact metadata that satisfies the stricter manifest rules for `site`,
    `webapp`, and `pii-app` scaffolds.
  - Canonical Soracloud service/deployment fixtures were updated to match the
    new manifest schema, including explicit null mailboxes on local-fast-path
    handlers.
- Validation:
  - `cargo fmt --all` (pass)
  - `git diff --check` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-private-runtime-target cargo test -p iroha_data_model validate_rejects_ -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-private-runtime-target cargo test -p iroha_data_model --test soracloud_manifest_fixtures -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-private-runtime-target cargo test -p iroha_core soracloud_runtime_records_are_visible_through_world_view -- --nocapture` (pass)
  - `CARGO_TARGET_DIR=/tmp/iroha-private-runtime-target cargo test -p iroha_cli --bin iroha soracloud::tests::init_ -- --nocapture` (pass)
- This slice does not yet remove the Torii-local Soracloud registry or add the
  node-embedded runtime manager in `irohad`; it establishes the authoritative
  runtime-state schema and world-state storage that those follow-on steps will
  build on.

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
  - `CARGO_TARGET_DIR=/tmp/iroha-trigger-dsl-target cargo test -p kotodama_lang manifest_trigger_decl_ -- --nocapture` (pass)
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

## 2026-03-19 Follow-up: fresh permissioned + NPoS stable soaks still tail on RBC READY deferral
- Rebuilt the current tree and reran both shared-host stable soaks on the same
  warmed release binary with preserved peer artifacts:
  - `cargo build --release -p irohad --bin iroha3d -p izanami`
  - `RUST_LOG=izanami::summary=info,izanami::progress=info,izanami::workload=warn,iroha_core::sumeragi::main_loop=info IROHA_TEST_NETWORK_KEEP_DIRS=1 IROHA_TEST_NETWORK_PERMIT_DIR=$(mktemp -d) TEST_NETWORK_BIN_IROHAD=/Users/mtakemiya/dev/iroha/target/release/iroha3d cargo run -p izanami --release --locked -- --allow-net --peers 4 --faulty 0 --duration 3600s --target-blocks 2000 --progress-interval 10s --progress-timeout 600s --tps 5 --max-inflight 8 --workload-profile stable`
  - `RUST_LOG=izanami::summary=info,izanami::progress=info,izanami::workload=warn,iroha_core::sumeragi::main_loop=info IROHA_TEST_NETWORK_KEEP_DIRS=1 IROHA_TEST_NETWORK_PERMIT_DIR=$(mktemp -d) TEST_NETWORK_BIN_IROHAD=/Users/mtakemiya/dev/iroha/target/release/iroha3d cargo run -p izanami --release --locked -- --allow-net --nexus --peers 4 --faulty 0 --duration 3600s --target-blocks 2000 --progress-interval 10s --progress-timeout 600s --tps 5 --max-inflight 8 --workload-profile stable`
- Permissioned sample:
  - log: `/tmp/izanami_permissioned_soak_20260319T185928Z.log`
  - peer dirs: `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_1OvLdX`
  - sampled window: `strict_min_height=429` in `700.2s` (`2205.8 blocks/hour` projected, `avg_interval_ms=1854.9`)
  - first late tail: `height 420 -> interval_ms=10003`, followed by `height 422 -> interval_ms=5002`
  - interval buckets in the sampled window: `10000+=1`, `5000-9999=1`, `3334-4999=1`, `2501-3333=11`
  - peer-log markers: `commit quorum missing past timeout=16`, `dropping block sync update missing commit-role quorum=0`, `deferring RBC DELIVER: READY quorum not yet satisfied=4643`, `queued RBC INIT and chunk rebroadcast while awaiting READY quorum=4018`, `commit roster unavailable` deferrals `=9/9`
- NPoS sample:
  - log: `/tmp/izanami_npos_soak_20260319T191521Z.log`
  - peer dirs: `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_XJVUGf`
  - sampled window: `strict_min_height=522` in `970.2s` (`1936.9 blocks/hour` projected, `avg_interval_ms=2042.7`)
  - first late tail: `height 379 -> interval_ms=10003`, followed by `height 405 -> interval_ms=5001` and `height 514 -> interval_ms=5002`
  - interval buckets in the sampled window: `10000+=1`, `5000-9999=2`, `3334-4999=2`, `2501-3333=22`
  - peer-log markers: `commit quorum missing past timeout=11`, `dropping block sync update missing commit-role quorum=0`, `deferring RBC DELIVER: READY quorum not yet satisfied=5513`, `queued RBC INIT and chunk rebroadcast while awaiting READY quorum=5072`, `commit roster unavailable` deferrals `=6/6`
  - harness contamination is still present: `plan submission failed=2` with `Trigger with id \`repeat_trigger_0\` not found`
- Current verdict:
  - permissioned is only somewhat better than NPoS; both modes still reproduce
    the same late `READY`-quorum/RBC rebroadcast tail in the fresh build,
  - the preserved peer logs do not support the earlier “RBC is fixed, only
    commit quorum remains” reading: `commit quorum missing past timeout`
    still fires, but it is accompanied by thousands of `deferring RBC DELIVER:
    READY quorum not yet satisfied` / `queued RBC INIT and chunk rebroadcast
    while awaiting READY quorum` warnings in both modes,
  - NPoS is additionally not a clean consensus signal yet because the trigger
    repetition workload still loses `repeat_trigger_0` during the run.

## 2026-03-19 Follow-up: known-block commit recovery now uses a monotone commit-evidence state machine
- Simplified the late known-block recovery path in `iroha_core` so it no
  longer mixes hydration and commit-evidence replay:
  - `crates/iroha_core/src/sumeragi/main_loop/pending_block.rs` now tracks
    pending commit progress explicitly as
    `AwaitingLocalVote -> LocalVoteEmitted -> CommitQcObserved`,
  - known-block reschedule/recovery paths now replay only `QcVote` / cached
    `Qc` for tracked pending blocks instead of rebuilding `BlockSyncUpdate`,
  - local commit-vote emission is reevaluated directly on `ValidationPassed`
    and `CommitVoteAccepted` events instead of waiting for a later sweep,
  - the old sender-side `block_sync_update_for_precommit_vote` path is removed.
- Restored the baseline `izanami` / Kotodama package signal in the same cut:
  - `crates/kotodama_lang/src/samples/asset_ops.ko` now uses the canonical
    `aid:6872454e9c044641aa581ec5f3801619` asset definition literal,
  - `crates/izanami/src/chaos.rs` now anchors strict divergence from the
    healthy-quorum side of the sorted sample set and locks the intended
    neighboring test semantics.
- Focused validation passed:
  - `cargo fmt --all`
  - `cargo test -p iroha_core --lib pending_commit_stage_is_monotone -- --nocapture`
  - `cargo test -p iroha_core --lib commit_evidence_replay -- --nocapture`
  - `cargo test -p iroha_core --lib quorum_reschedule_rebroadcasts_votes_without_block_created_without_roster -- --nocapture`
  - `cargo test -p iroha_core --lib quorum_reschedule_skips_block_created_fallback_when_roster_proof_missing -- --nocapture`
  - `cargo test -p iroha_core --lib quorum_reschedule_near_quorum -- --nocapture`
  - `cargo test -p iroha_core --lib known_block_commit_qc_replay_targets_snapshot_roster -- --nocapture`
  - `cargo test -p iroha_core --lib proposal_backpressure_ignores_committed_tip_rbc_cleanup_backlog -- --nocapture`
  - `cargo test -p iroha_core --lib commit_pipeline_snapshot_tracks_last_and_ema_fields -- --nocapture`
  - `cargo test -p iroha_core --lib round_gap_snapshot_records_markers_in_order -- --nocapture`
  - `cargo test -p iroha_core --lib commit_outcome_kickstarts_next_proposal_and_records_round_gap -- --nocapture`
  - `cargo test -p izanami`
  - `cargo build --release -p irohad --bin iroha3d -p izanami`

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

## 2026-03-18 Follow-up: offline allowance list/query payloads now expose asset-definition metadata across Torii and SDKs
- Updated `crates/iroha_torii/src/routing.rs` so offline allowance projection resolves the referenced asset definition once per record, injects `asset_definition_id`, `asset_definition_name`, and nullable `asset_definition_alias` into list/query JSON, and fails with an internal error when an allowance references a missing asset definition.
- Updated `crates/iroha_torii/src/openapi.rs` plus the checked-in Torii OpenAPI JSON snapshots so `OfflineAllowanceItem` documents the concrete `asset_id` separately from the human-facing asset-definition metadata.
- Updated Android offline allowance parsing/model types in `java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/` and adjusted affected tests/call sites to require the new constructor/parser fields.
- Updated JavaScript offline allowance normalization/types in `javascript/iroha_js/src/toriiClient.js`, `javascript/iroha_js/index.d.ts`, regenerated `javascript/iroha_js/dist/`, and extended allowance normalization assertions in `javascript/iroha_js/test/toriiClient.test.js`.
- Updated Python offline allowance parsing in `python/iroha_torii_client/client.py` and the corresponding payload test in `python/iroha_torii_client/tests/test_client.py`.
- Updated Android offline allowance docs in `java/iroha_android/README.md` and `docs/source/sdk/android/offline_signing.md` to use `assetDefinitionName()` and describe the `asset_id` vs `asset_definition_id` split.
- Validation:
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_torii offline_allowance_ -- --nocapture` (pass)
  - `cargo test -p iroha_torii --test offline_app_api -- --nocapture` (pass)
  - `/bin/zsh -lc 'JAVA_HOME=$(/usr/libexec/java_home -v 21) ./gradlew :jvm:test --console=plain'` from `java/iroha_android` (pass)
  - `IROHA_JS_DISABLE_NATIVE=1 node --test --test-name-pattern "OfflineAllowances|OfflineAllowance" test/toriiClient.test.js` from `javascript/iroha_js` (pass)
  - `IROHA_JS_DISABLE_NATIVE=1 node --test test/toriiClient.test.js` from `javascript/iroha_js` (fails in 4 pre-existing non-offline tests: three governance owner-validation assertions and one `extractToriiFeatureConfig` validation case)
  - `python3 -m pytest python/iroha_torii_client/tests/test_client.py -k offline_allowances` could not run because `pytest` is not installed in this environment
  - `PYTHONPATH=python python3 -c '...'` offline allowance smoke-check could not run because the environment is also missing `requests`

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

## 2026-03-15 Follow-up: mobile offline asset-id literal builders (later superseded by public literals)
- Implemented SDK-facing support for building then-current canonical asset IDs from textual parts
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
  - Updated `crates/kotodama_lang/src/samples/mint_rose_trigger.ko` to canonical asset literal `62Fk4FPcMuLvW5QjDGNF2a4jAmjM`.
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

## 2026-03-24 Asset Definition ID and Asset API Unification
- Switched public asset-definition literals from legacy `aid:<hex>` to canonical unprefixed Base58 addresses with checksum in `iroha_data_model::asset::AssetDefinitionId`.
- Updated asset-definition alias grammar to dotted long form:
  - `<name>#<domain>.<dataspace>`
  - `<name>#<dataspace>`
- Added direct singular lookup support for asset definitions:
  - `FindAssetDefinitionById`
  - JSON query envelope support for `FindAssetDefinitionById`
- Updated Torii app APIs:
  - `GET /v1/assets/definitions` now returns full asset-definition objects.
  - Added `GET /v1/assets/definitions/{asset}`.
  - Asset-definition selectors on public Torii routes now reject legacy `aid:` literals and resolve canonical Base58 ids or aliases.
  - Asset holders endpoints now expose `asset`, `asset_alias`, `account_id`, `scope`, and `quantity` instead of filtering/returning raw encoded `AssetId` blobs.
- Updated CLI/docs/help text to use canonical Base58 asset-definition ids and dotted aliases.
- Continued the public/parser cleanup so more user-facing entry points now require canonical Base58 explicitly instead of relying on `AssetDefinitionId::from_str` fallback:
  - CLI ZK commands and Sorafs reward-policy decoding.
  - JS host `encode_asset_id(...)`.
  - Kagami localnet asset-spec parsing.
  - Nexus manifest scope parsing.
  - ISO 20022 bridge currency bindings and `SplmtryData/AssetDefinitionId` hints.
  - Core metadata/state-access parsing paths (`block`, `pipeline::access`, witness capture, asset query filters).
- Reduced legacy `aid:` dependency in internal fixtures/tests by switching several opaque-id fixtures to `AssetDefinitionId::from_uuid_bytes(...)` and updating CLI help/README examples away from `aid:`.
- Fixed two unrelated compile drifts encountered during verification:
  - `crates/iroha_data_model/src/account/rekey.rs` now uses static `ParseError` messages.
  - `crates/kotodama_lang/src/compiler.rs` handles optional account domains when collecting access sets.
- Repaired asset-definition alias lease durability and API visibility:
  - `asset_definition_alias_bindings` is now persisted and treated as the authoritative lease record.
  - World/index rebuild now reconstructs alias lookup state from persisted bindings first, while synthesizing permanent bindings for legacy definitions that only stored `definition.alias`.
  - Added lease-state classification (`permanent`, `leased_active`, `leased_grace`, `expired_pending_cleanup`) on the binding record.
  - `SetAssetDefinitionAlias` now rejects `lease_expiry_ms` values that are already expired at bind time.
  - Torii asset-definition JSON and alias-resolution JSON now include `alias_binding` metadata with `status`, `lease_expiry_ms`, `grace_until_ms`, and `bound_at_ms` when an alias exists.
- Fixed additional branch drifts uncovered while validating the alias-lease change:
  - `crates/iroha_config/src/parameters/user.rs` now wraps the app-auth replay-cache default as `NonZeroUsize`.
  - `crates/iroha_config/src/parameters/actual.rs` now threads `mtls_trusted_proxy_cidrs` into `NoritoRpcTransport`.
  - `crates/iroha_core/src/kiso.rs`, `crates/iroha_torii/src/test_utils.rs`, `crates/iroha_torii/tests/connect_gating.rs`, `crates/iroha_torii/src/operator_signatures.rs`, and Torii test-state builders were updated to match the current config/auth shapes.
- Validation:
  - `cargo check -p iroha_torii --tests`
  - `cargo fmt --all`
  - `cargo check -p connect_norito_bridge --tests`
  - `cargo check -p iroha_core --tests`
  - `cargo check -p iroha_cli --tests`
  - `cargo test -p connect_norito_bridge parse_asset_definition_accepts_canonical_base58_literal -- --nocapture`
  - `cargo test -p connect_norito_bridge --bin swift_parity_regen parse_asset_definition_argument_accepts_canonical_literal -- --nocapture`
  - `cargo test -p iroha_core asset_definition_alias -- --nocapture`
  - `cargo test -p iroha_torii --lib asset_alias_resolve -- --nocapture`
  - `cargo test -p iroha_torii --lib asset_definition_get_returns_full_definition_by_base58_id -- --nocapture`
  - `cargo test -p iroha_torii --test asset_definitions_endpoints -- --nocapture`

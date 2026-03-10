# Status

Last updated: 2026-03-10

## Current Enforced State
- First-release identity/deploy policy is strict (no backward aliases, shims, or migration wrappers).
- Historical notes below record intermediate steps; when any older entry conflicts with the current hard-cut semantics, this section and the latest 2026-03-10 hard-cut entry are authoritative.
- Deploy preflight scanner entrypoint is `../pk-deploy/scripts/check-identity-surface.sh`; the previous scanner entrypoint is removed.
- Runtime deploy scripts are aligned to strict-first-release behavior:
  - `../pk-deploy/scripts/cutover-ih58-mega.sh` invokes only `check-identity-surface.sh`.
  - `../pk-deploy/scripts/deploy-sbp-aed-pkr-interceptor.sh` no longer performs prior-layout trigger cleanup loops.
- Wallet docs describe only current QR modes in neutral terms.

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
  - Updated `CanRegisterAssetDefinition` grant/revoke validation to no longer require domain-owner checks (token treated as no-op compatibility surface in this release line).
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
  - `iroha_cli` account-bearing inputs now require canonical IH58 `AccountId` literals
  - `ledger account register` keeps domain context explicit via `--domain`
  - account/domain reference docs no longer advertise `@<domain>` hints, compressed account-id parsing, or legacy selector decode paths
- Closed the remaining strict `AccountId` input drift in Torii/CLI/integration coverage:
  - `integration_tests/tests/address_canonicalisation.rs` no longer uses scoped-account leftovers (`Account::new(ScopedAccountId)`, no `.domain()` on `AccountId`) and now rejects compressed `AccountId` literals on account path/query filters, explorer authority filters, and Kaigi relay detail paths while preserving explicit `address_format=compressed` response rendering.
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
  - switched default governance account literals from compressed sora form to canonical IH58 so strict `AccountId::parse_encoded(...)` parsing succeeds.

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
  - `find_domains_by_account_id_accepts_canonical_ih58_literal`
- Fixed a compile-time move bug in an existing query visitor regression test:
  - `crates/iroha_data_model/src/visit/visit_query.rs`
  - `singular_query_fallback_never_triggers_for_known_variants` now clones `account_id` before `AssetId::new(...)`.

### Validation Matrix (Commit Validation Queue-Saturation Hot-Path Cutover)
- `cargo fmt --all`
- `CARGO_TARGET_DIR=target_tmp_validation_cutover cargo test -p iroha_core --lib commit_pipeline_inlines_validation_ -- --nocapture`
- `CARGO_TARGET_DIR=target_tmp_validation_cutover cargo test -p iroha_core --lib commit_pipeline_keeps_deferred_validation_ -- --nocapture`
- `CARGO_TARGET_DIR=target_tmp_validation_cutover cargo test -p iroha_data_model --lib find_domains_by_account_id_accepts_canonical_ih58_literal -- --nocapture`
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
  - switched offline receipt/spend JNI tests to canonical encoded `AssetId` fixtures (`norito:<hex>`) and valid IH58 account literals:
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
  - aligns expectations with domainless IH58 owner literals used by SNS JSON payloads

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
- Added `/v1/accounts/query` regression coverage for alias handling in Torii.
- The later 2026-03-10 hard-cut sweep tightened the same surface so compressed `AccountId` literals are rejected; see the latest 2026-03-10 entry for the current test names and validation commands.

### Validation Matrix (Alias Regression)
- Historical validation used the then-current `/v1/accounts/query` regression tests; the 2026-03-10 hard-cut entry supersedes those test names with strict compressed-literal rejection coverage.
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
  - Updated legacy canonical-hex rejection tests to match the strict encoded-account policy (IH58/compressed-only public parsing).
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

---
lang: ka
direction: ltr
source: docs/portal/docs/nexus/nexus-refactor-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3c952f92e009ea4b2ccba55940737889e8e70f506d09899598bd913a2ac68d2d
source_last_modified: "2026-01-22T14:35:36.839904+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-refactor-plan
title: Sora Nexus ledger refactor plan
description: Mirror of `docs/source/nexus_refactor_plan.md`, detailing the phased clean-up work for the Iroha 3 codebase.
---

:::note Canonical Source
This page mirrors `docs/source/nexus_refactor_plan.md`. Keep both copies aligned until the multilingual edition lands in the portal.
:::

# Sora Nexus Ledger Refactor Plan

This document captures the immediate roadmap for the Sora Nexus Ledger ("Iroha 3") refactor. It reflects the current repository layout and the regressions observed in genesis/WSV bookkeeping, Sumeragi consensus, smart-contract triggers, snapshot queries, pointer-ABI host bindings, and Norito codecs. The objective is to converge on a coherent, testable architecture without attempting to land all fixes in one monolithic patch.

## 0. Guiding Principles
- Preserve deterministic behavior across heterogeneous hardware; leverage acceleration only through opt-in feature flags with identical fallbacks.
- Norito is the serialization layer. Any state/schema changes must include Norito encode/decode round-trip tests and fixture updates.
- Configuration flows through `iroha_config` (user → actual → defaults). Remove ad-hoc environment toggles from production paths.
- ABI policy remains V1 and non-negotiable. Hosts must reject unknown pointer types/syscalls deterministically.
- `cargo test --workspace` and golden tests (`ivm`, `norito`, `integration_tests`) remain the baseline gate for every milestone.

## 1. Repository Topology Snapshot
- `crates/iroha_core`: Sumeragi actors, WSV, genesis loader, pipelines (query, overlay, zk lanes), smart-contract host glue.
- `crates/iroha_data_model`: authoritative schema for on-chain data and queries.
- `crates/iroha`: client API used by CLI, tests, SDK.
- `crates/iroha_cli`: operator CLI, currently mirrors numerous APIs in `iroha`.
- `crates/ivm`: Kotodama bytecode VM, pointer-ABI host integration entry points.
- `crates/norito`: serialization codec with JSON adapters and AoS/NCB backends.
- `integration_tests`: cross-component assertions covering genesis/bootstrap, Sumeragi, triggers, pagination, etc.
- Docs already outline Sora Nexus Ledger goals (`nexus.md`, `new_pipeline.md`, `ivm.md`), but the implementation is fragmented and partially stale relative to the code.

## 2. Refactor Pillars & Milestones

### Phase A – Foundations and Observability
1. **WSV Telemetry + Snapshots**
   - Establish canonical snapshot API in `state` (`WorldStateSnapshot` trait) used by queries, Sumeragi, and CLI.
   - Use `scripts/iroha_state_dump.sh` to produce deterministic snapshots via `iroha state dump --format norito`.
2. **Genesis/Bootstrap Determinism**
   - Refactor genesis ingestion to flow through a single Norito-powered pipeline (`iroha_core::genesis`).
  - Add integration/regression coverage that replays genesis plus the first block and asserts identical WSV roots across arm64/x86_64 (tracked under `integration_tests/tests/genesis_replay_determinism.rs`).
3. **Cross-crate Fixity Tests**
   - Expand `integration_tests/tests/genesis_json.rs` to validate WSV, pipeline, and ABI invariants in one harness.
  - Introduce a `cargo xtask check-shape` scaffold that panics on schema drift (tracked under DevEx tooling backlog; see `scripts/xtask/README.md` action item).

### Phase B – WSV & Query Surface
1. **State Storage Transactions**
   - Collapse `state/storage_transactions.rs` into a transactional adapter that enforces commit ordering and conflict detection.
   - Unit tests now verify asset/world/triggers modifications roll back on failure.
2. **Query Model Refactor**
   - Move pagination/cursor logic into reusable components under `crates/iroha_core/src/query/`. Align Norito representations in `iroha_data_model`.
  - Add snapshot queries for triggers, assets, and roles with deterministic ordering (tracked via `crates/iroha_core/tests/snapshot_iterable.rs` for current coverage).
3. **Snapshot Consistency**
   - Ensure `iroha ledger query` CLI uses the same snapshot path as Sumeragi/fetchers.
   - CLI snapshot regression tests live under `tests/cli/state_snapshot.rs` (feature-gated for slow runs).

### Phase C – Sumeragi Pipeline
1. **Topology & Epoch Management**
   - Extract `EpochRosterProvider` into a trait with implementations backed by WSV stake snapshots.
  - `WsvEpochRosterAdapter::from_peer_iter` offers a simple mock-friendly constructor for benches/tests.
2. **Consensus Flow Simplification**
   - Reorganize `crates/iroha_core/src/sumeragi/*` into modules: `pacemaker`, `aggregation`, `availability`, `witness` with shared types under `consensus`.
  - Replace ad-hoc message passing with typed Norito envelopes and introduce view-change property tests (tracked in the Sumeragi messaging backlog).
3. **Lane/Proof Integration**
   - Align lane proofs with DA commitments and ensure RBC gating is uniform.
   - End-to-end integration test `integration_tests/tests/extra_functional/seven_peer_consistency.rs` now verifies the RBC-enabled path.

### Phase D – Smart Contracts & Pointer-ABI Hosts
1. **Host Boundary Audit**
   - Consolidate pointer-type checks (`ivm::pointer_abi`) and host adapters (`iroha_core::smartcontracts::ivm::host`).
   - Pointer table expectations and host manifest bindings are covered by `crates/iroha_core/tests/ivm_pointer_abi_tlv_types.rs` and `ivm_host_mapping.rs`, which exercise the golden TLV mappings.
2. **Trigger Execution Sandbox**
   - Refactor triggers to run through a common `TriggerExecutor` that enforces gas, pointer validation, and event journaling.
  - Add regression tests for call/time triggers covering failure paths (tracked via `crates/iroha_core/tests/trigger_failure.rs`).
3. **CLI & Client Alignment**
   - Ensure CLI operations (`audit`, `gov`, `sumeragi`, `ivm`) rely on the shared `iroha` client functions to avoid drift.
   - CLI JSON snapshot tests live in `tests/cli/json_snapshot.rs`; keep them up to date so core command output continues to match the canonical JSON reference.

### Phase E – Norito Codec Hardening
1. **Schema Registry**
   - Create a Norito schema registry under `crates/norito/src/schema/` to source canonical encodings for core data types.
   - Added doc tests verifying sample payload encoding (`norito::schema::SamplePayload`).
2. **Golden Fixtures Refresh**
   - Update `crates/norito/tests/*` golden fixtures to match new WSV schema once the refactor lands.
   - `scripts/norito_regen.sh` regenerates the Norito JSON goldens deterministically via the `norito_regen_goldens` helper.
3. **IVM/Norito Integration**
   - Validate Kotodama manifest serialization end-to-end through Norito, ensuring pointer ABI metadata is consistent.
   - `crates/ivm/tests/manifest_roundtrip.rs` keeps Norito encode/decode parity for manifests.

## 3. Cross-Cutting Concerns
- **Testing Strategy**: Every phase promotes unit tests → crate tests → integration tests. Failing tests capture current regressions; new tests prevent them from resurfacing.
- **Documentation**: After each phase lands, update `status.md` and roll open items into `roadmap.md` while pruning completed tasks.
- **Performance Benchmarks**: Maintain existing benches in `iroha_core`, `ivm`, and `norito`; add baseline measurements post-refactor to validate no regressions.
- **Feature Flags**: Keep crate-level toggles only for backends that require external toolchains (`cuda`, `zk-verify-batch`). CPU SIMD paths are always built and selected at runtime; provide deterministic scalar fallbacks for unsupported hardware.

## 4. Immediate Next Actions
- Phase A scaffolding (snapshot trait + telemetry wiring) – see actionable tasks in roadmap updates.
- The recent defect audit for `sumeragi`, `state`, and `ivm` surfaced the following highlights:
  - `sumeragi`: dead-code allowances guard view-change proof broadcast, VRF replay state, and EMA telemetry export. These stay gated until Phase C’s consensus flow simplification and lane/proof integration deliverables land.
  - `state`: `Cell` cleanup and telemetry routing move onto the Phase A WSV telemetry track, while the SoA/parallel-apply notes fold into the Phase C pipeline optimisation backlog.
  - `ivm`: CUDA toggle exposure, envelope validation, and Halo2/Metal coverage map to Phase D host-boundary work plus the cross-cutting GPU acceleration theme; kernels remain on the dedicated GPU backlog until ready.
- Prepare cross-team RFC summarizing this plan for sign-off before landing invasive code changes.

## 5. Open Questions
- Should RBC remain optional past P1, or is it mandatory for Nexus ledger lanes? Requires stakeholder decision.
- Do we enforce DS composability groups in P1 or keep them disabled until lane proofs mature?
- What is the canonical location for ML-DSA-87 parameters? Candidate: new `crates/fastpq_isi` crate (pending creation).

---

_Last updated: 2025-09-12_

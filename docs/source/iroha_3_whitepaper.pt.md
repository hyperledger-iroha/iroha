---
lang: pt
direction: ltr
source: docs/source/iroha_3_whitepaper.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 07e149429887b0dfc38cf0619552cbefcbae4dd1ec9fe9e9d47a05371ed08f29
source_last_modified: "2026-01-03T18:07:57.204376+00:00"
translation_last_reviewed: 2026-01-30
---

# Iroha v3.0 (Nexus Preview)

This document captures the forward-looking Hyperledger Iroha v3 architecture, focusing on the multi-lane
pipeline, Nexus data spaces, and the Asset Exchange Toolkit (AXT). It complements the Iroha v2 whitepaper by
describing upcoming capabilities that are actively under development.

---

## 1. Overview

Iroha v3 extends the deterministic foundation of v2 with horizontal scalability and richer cross-domain
workflows. The release, codenamed **Nexus**, introduces:

- A single, globally shared network called **SORA Nexus**. All Iroha v3 peers participate in this universal
  ledger rather than operating isolated deployments. Organisations join by registering their own data spaces,
  which remain isolated for policy and privacy while anchoring into the common ledger.
- A shared codebase: the same repository builds both Iroha v2 (self-hosted networks) and Iroha v3 (SORA Nexus).
  Configuration selects the target mode so operators can adopt Nexus features without switching software
  stacks. The Iroha Virtual Machine (IVM) is identical across both releases, so Kotodama contracts and bytecode
  artefacts run seamlessly on self-hosted networks and the global Nexus ledger.
- Multi-lane block production to process independent workloads in parallel.
- Data spaces (DS) that isolate execution environments while remaining composable through on-chain anchors.
- The Asset Exchange Toolkit (AXT) for atomic, cross-space value transfers and contract-controlled swaps.
- Enhanced reliability through Reliable Broadcast Commit (RBC) lanes, deterministic deadlines, and proof
  sampling budgets.

These features remain under active development; APIs and layouts may evolve before the v3 general
availability milestone. Refer to `nexus.md`, `nexus_transition_notes.md`, and `new_pipeline.md` for
engineering-level detail.

## 2. Multi-lane architecture

- **Scheduler:** The Nexus scheduler partitions work into lanes based on data space identifiers and
  composability groups. Lanes execute in parallel while preserving deterministic ordering guarantees within
  each lane.
- **Lane groups:** Related data spaces share a `LaneGroupId`, enabling coordinated execution for workflows that
  span multiple components (e.g., a CBDC DS and its payment dApp DS).
- **Deadlines:** Each lane tracks deterministic deadlines (block, proof, data-availability) to guarantee
  progress and bounded resource usage.
- **Telemetry:** Lane-level metrics expose throughput, queue depth, deadline violations, and bandwidth usage.
  CI scripts assert the presence of these counters to keep dashboards aligned with the scheduler.

## 3. Data spaces (Nexus)

- **Isolation:** Each data space maintains its own consensus lane, world state segment, and Kura storage. This
  supports privacy domains while keeping the global SORA Nexus ledger coherent through anchors.
- **Anchors:** Regular commits produce anchor artifacts that summarise the DS state (Merkle roots, proofs,
  commitments) and publish them to the global lane for auditability.
- **Lane groups and composability:** Data spaces may declare composability groups that permit atomic AXT
  transactions across approved participants. Governance controls membership changes and activation epochs.
- **Erasure-coded storage:** Kura and WSV snapshots adopt erasure coding parameters `(k, m)` to scale data
  availability without sacrificing determinism. Recovery routines restore missing fragments deterministically.

## 4. Asset Exchange Toolkit (AXT)

- **Descriptor and binding:** Clients construct deterministic AXT descriptors. The `axt_binding` hash anchors
  descriptors to individual envelopes, preventing replay and ensuring consensus participants validate byte-for-
  byte Norito payloads.
- **Syscalls:** The IVM exposes `AXT_BEGIN`, `AXT_TOUCH`, and `AXT_COMMIT` syscalls. Contracts declare their
  read/write sets per data space, allowing the host to enforce atomicity across lanes.
- **Handles and epochs:** Wallets obtain capability handles bound to `(dataspace_id, epoch_id, sub_nonce)`.
  Concurrent uses conflict deterministically, returning canonical `AxtTrap` codes when constraints are
  violated.
- **Policy enforcement:** Core hosts now derive AXT policy snapshots from Space Directory manifests in WSV,
  enforcing manifest root, target lane, activation-era, sub-nonce, and expiry checks (`current_slot >= expiry_slot`
  aborts) even in minimal test hosts. Policies are keyed by dataspace id and built from the lane catalog so
  handles cannot escape their issuing lane or use stale manifests.
  - Rejection reasons are deterministic: unknown dataspace, manifest root mismatch, target lane mismatch,
    handle_era below manifest activation, sub_nonce below the policy floor, expired handle, missing touch for
    the handle dataspace, or missing proof when required.
- **Proofs and deadlines:** During an active window Δ, validators collect proofs, data availability samples,
  and manifests. Failure to meet deadlines aborts the AXT deterministically with guidance for client retries.
- **Governance integration:** Policy modules define which data spaces can participate in AXT, rate-limit
  handles, and publish auditor-friendly manifests capturing commitments, nullifiers, and event logs.

## 5. Reliable Broadcast Commit (RBC) lanes

- **Lane-specific DA:** RBC lanes mirror lane groups, ensuring each multi-lane pipeline has dedicated data
  availability guarantees.
- **Sampling budgets:** Validators follow deterministic sampling rules (`q_in_slot_per_ds`) to validate proofs
  and witness material without central coordination.
- **Backpressure insights:** Sumeragi pacemaker events correlate with RBC statistics to diagnose stalled lanes
  (see `scripts/sumeragi_backpressure_log_scraper.py`).

## 6. Operations and migration

- **Transition plan:** `nexus_transition_notes.md` outlines phased migration from single-lane (Iroha v2) to
  multi-lane (Iroha v3), including telemetry staging, config gating, and genesis updates.
- **Universal network:** SORA Nexus peers run a common genesis and governance stack. New operators onboard by
  creating a data space (DS) and satisfying Nexus admission policies instead of launching standalone networks.
- **Configuration:** New config knobs cover lane budgets, proof deadlines, AXT quotas, and data-space metadata.
  Defaults remain conservative until operators opt into Nexus mode.
- **Testing:** Golden tests capture AXT descriptors, lane manifests, and syscall lists. Integration tests
  (`integration_tests/tests/repo.rs`, `crates/ivm/tests/axt_host_flow.rs`) exercise end-to-end flows.
- **Tooling:** `kagami` gains Nexus-aware genesis generation, and dashboard scripts validate lane throughput,
  proof budgets, and RBC health.

## 7. Roadmap

- **Phase 1:** Enable single-domain multi-lane execution with local AXT support and auditing.
- **Phase 2:** Activate composability groups for permissioned cross-domain AXT and expand telemetry coverage.
- **Phase 3:** Roll out full Nexus data-space federation, erasure-coded storage, and advanced proof sharing.

Status updates live in `roadmap.md` and `status.md`. Contributions aligning with the Nexus design should follow
the deterministic execution and governance policies established for v3.

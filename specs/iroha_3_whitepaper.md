# Iroha 3 (Nexus)

This document describes the Hyperledger Iroha 3 architecture, focusing on the
multi-lane pipeline, Nexus data spaces, and the Asset Exchange Toolkit (AXT).

---

## 1. Overview

Iroha 3 provides horizontal scalability and cross-domain workflows through the
mandatory **Nexus** runtime:

- A single, globally shared network called **SORA Nexus**. All Iroha v3 peers participate in this universal
  ledger rather than operating isolated deployments. Organisations join by registering their own data spaces,
  which remain isolated for policy and privacy while anchoring into the common ledger.
- A single product implementation: every deployment uses Nexus, including
  deployments configured with one lane. Kotodama contracts and IVM bytecode use
  the same execution environment throughout the product.
- Lanes: additional consensus instances that order independent workloads in parallel; the global chain
  merges their certified blocks.
- Data spaces (DS) that isolate execution environments while remaining composable through on-chain anchors.
- The Asset Exchange Toolkit (AXT) for atomic, cross-space value transfers and contract-controlled swaps.
- One deterministic consensus protocol (Sumeragi, execute before vote) for the global chain and every lane.

Refer to `nexus.md`, `sumeragi.md`, `sumeragi_lanes.md`, and `new_pipeline.md` for
engineering-level detail.

## 2. Multi-lane architecture

- **Lanes:** Each lane is a Sumeragi instance with a committee pinned at creation. The global chain merges
  certified lane blocks and executes their transactions in one canonical order (`sumeragi_lanes.md`).
- **Routing:** Governed explicit routes select a lane; otherwise transactions are sharded by authority. The
  merge re-evaluates each route; a transaction routed just before a lane opened or closed is dropped without
  effect and re-routed.
- **Autoscale:** Deterministic rules in the global chain open and close elastic lanes from committed load
  samples.
- **Telemetry:** `/v1/sumeragi/lanes` reports each lane's record and this node's lane instance.

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

## 5. Consensus and data availability

- **Consensus:** One Sumeragi protocol orders the global chain and every lane; each certificate carries exactly
  `2f + 1` equal votes from a `3f + 1` committee (`sumeragi.md`).
- **Data availability:** Signed RS16 payload availability is a first-release requirement
  (`sumeragi_goals.md`).
- **Diagnostics:** Authenticated `/v1/sumeragi/status` and `/v1/sumeragi/lanes`, plus bounded transaction,
  ingress and P2P queue metrics, diagnose stalled lanes.

## 6. Operations and migration

- **Operational invariants:** `references/configuration.md` records the mandatory Nexus configuration;
  lanes come from the governed lane policy (`sumeragi_lanes.md`).
- **Universal network:** SORA Nexus peers run a common genesis and governance stack. New operators onboard by
  creating a data space (DS) and satisfying Nexus admission policies instead of launching standalone networks.
- **Configuration:** Config knobs cover lane budgets, proof deadlines, AXT quotas,
  and data-space metadata. Nexus mode is mandatory.
- **Testing:** Golden tests capture AXT descriptors, lane manifests, and syscall lists. Integration tests
  (`integration_tests/tests/repo.rs`, `crates/ivm/tests/axt_host_flow.rs`) exercise end-to-end flows.
- **Tooling:** `kagami` gains Nexus-aware genesis generation, and dashboard scripts validate lane throughput
  and proof budgets.

## 7. Roadmap

- **Phase 1:** Enable single-domain multi-lane execution with local AXT support and auditing.
- **Phase 2:** Activate composability groups for permissioned cross-domain AXT and expand telemetry coverage.
- **Phase 3:** Roll out full Nexus data-space federation, erasure-coded storage, and advanced proof sharing.

Status updates live in `roadmap.md` and `status.md`. Contributions aligning with the Nexus design should follow
the deterministic execution and governance policies established for v3.

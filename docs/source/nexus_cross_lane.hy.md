---
lang: hy
direction: ltr
source: docs/source/nexus_cross_lane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b331bea7cbcaf5f810f4d2e9e176dddd1056111b74a7e811561e86d02407fc3b
source_last_modified: "2026-01-07T14:50:03.776652+00:00"
translation_last_reviewed: 2026-02-07
title: Nexus Cross-Lane Commitments
sidebar_label: Cross-Lane Commitments
description: Proof pipeline, relay responsibilities, and evidence requirements for roadmap item NX-4.
---

# Nexus Cross-Lane Commitments & Proof Pipeline

> **Status:** NX-4 deliverable — cross-lane commitment pipeline & proofs (target Q4 2025).  
> **Owners:** Nexus Core WG · Cryptography WG · Networking TL.  
> **Related roadmap items:** NX-1 (lane geometry), NX-3 (settlement router), NX-4 (this document), NX-8 (global scheduler), NX-11 (SDK conformance).

This note describes how per-lane execution data becomes a verifiable global commitment. It ties together the existing settlement router (`crates/settlement_router`), the lane block builder (`crates/iroha_core/src/block.rs`), telemetry/status surfaces, and the planned LaneRelay/DA hooks that still need to land for roadmap **NX-4**.

## Goals

- Produce a deterministic `LaneBlockCommitment` per lane block capturing settlement, liquidity, and variance data without leaking private state.
- Relay these commitments (and their DA attestations) to the global NPoS ring so the merge ledger can order, validate, and persist cross-lane updates.
- Expose the same payloads through Torii and telemetry so operators, SDKs, and auditors can replay the pipeline without bespoke tooling.
- Define the invariants and evidence bundles required to graduate NX-4: lane proofs, DA attestations, merge-ledger integration, and regression coverage.

## Components & Surfaces

| Component | Responsibility | Implementation references |
|-----------|----------------|---------------------------|
| Lane executor & settlement router | Quote XOR conversions, accumulate receipts per transaction, enforce buffer policy | `crates/iroha_core/src/settlement/mod.rs`, `crates/settlement_router` |
| Lane block builder | Drain `SettlementAccumulator`s, emit `LaneBlockCommitment`s alongside the lane block | `crates/iroha_core/src/block.rs:3340-3415` |
| LaneRelay broadcaster | Bundle lane QCs + DA proofs, gossip them across `iroha_p2p`, and feed the merge ring | `crates/iroha_core/src/nexus/lane_relay.rs`, `crates/iroha_core/src/sumeragi/main_loop.rs` |
| Global merge ledger | Verify lane QCs, reduce merge hints, persist world-state commitments | `docs/source/merge_ledger.md`, `crates/iroha_core/src/sumeragi/status.rs`, `crates/iroha_core/src/state.rs` |
| Torii status & dashboards | Surface `lane_commitments`, `lane_settlement_commitments`, `lane_relay_envelopes`, scheduler gauges, and Grafana boards | `crates/iroha_torii/src/routing.rs:16660-16880`, `dashboards/grafana/nexus_lanes.json` |
| Evidence storage | Archive `LaneBlockCommitment`s, RBC artefacts, and Alertmanager snapshots for audits | `docs/settlement-router.md`, `artifacts/nexus/*` (future bundle) |

## Data Structures & Payload Layout

The canonical payloads live in `crates/iroha_data_model/src/block/consensus.rs`.

### `LaneSettlementReceipt`

- `source_id` — transaction hash or caller-provided id.
- `local_amount_micro` — dataspace gas token debit.
- `xor_due_micro` / `xor_after_haircut_micro` / `xor_variance_micro` — deterministic XOR book entries and the per-receipt safety margin (`due - after haircut`).
- `timestamp_ms` — UTC millisecond timestamp captured during settlement.

Receipts inherit the deterministic quoting rules from `SettlementEngine` and are aggregated inside each `LaneBlockCommitment`.

### `LaneSwapMetadata`

Optional metadata that records the parameters used while quoting:

- `epsilon_bps`, `twap_window_seconds`, `volatility_class`.
- `liquidity_profile` bucket (Tier1–Tier3).
- `twap_local_per_xor` string so auditors can recompute conversions exactly.

### `LaneBlockCommitment`

Per-lane summary stored with every block:

- Header: `block_height`, `lane_id`, `dataspace_id`, `tx_count`.
- Totals: `total_local_micro`, `total_xor_due_micro`, `total_xor_after_haircut_micro`, `total_xor_variance_micro`.
- Optional `swap_metadata`.
- Ordered `receipts` vector.

These structs already derive `NoritoSerialize`/`NoritoDeserialize`, so they can be streamed on-chain, through Torii, or via fixtures without schema drift.

### `LaneRelayEnvelope`

`LaneRelayEnvelope` (see `crates/iroha_data_model/src/nexus/relay.rs`) packages the lane
`BlockHeader`, optional commit QC (`Qc`), optional `DaCommitmentBundle` hash, the full
`LaneBlockCommitment`, and the per-lane RBC byte count. The envelope stores a Norito-derived
`settlement_hash` (via `compute_settlement_hash`) so receivers can validate the settlement payload
before forwarding it to the merge ledger. Callers should reject envelopes when `verify` fails (QC
subject mismatch, DA hash mismatch, or settlement hash mismatch), when `verify_with_quorum` fails
(signer bitmap length/quorum errors), or when the aggregated QC signature cannot be verified
against the per-dataspace committee roster. The QC preimage covers the lane block hash plus
`parent_state_root` and `post_state_root`, so membership and state-root correctness are verified
together.

### Lane committee selection

Lane relay QCs are validated against a per-dataspace committee. Committee size is `3f+1`, where
`f` is configured in the dataspace catalog (`fault_tolerance`). The validator pool is the
dataspace's validators: lane governance manifests for admin-managed lanes and public-lane staking
records for stake-elected lanes. Committee membership is deterministically sampled per epoch using
the VRF epoch seed bound with `dataspace_id` and `lane_id` (stable for the epoch). If the pool is
smaller than `3f+1`, lane relay finality pauses until quorum is restored. Operators can extend the
pool using the admin multisig instruction `SetLaneRelayEmergencyValidators` (requires
`CanManagePeers` and `nexus.lane_relay_emergency.enabled = true`, which is disabled by default).
When enabled, the authority must be a multisig account meeting the configured minimums
(`nexus.lane_relay_emergency.multisig_threshold`/`multisig_members`, default 3-of-5). Overrides are
stored per dataspace, applied only when the pool is under quorum, and cleared by submitting an
empty validator list. When `expires_at_height` is set, validation ignores the override once the
lane relay envelope `block_height` exceeds the expiry height. The telemetry counter
`lane_relay_emergency_override_total{lane,dataspace,outcome}` records whether the override was
applied (`applied`) or missing/expired/insufficient/disabled during validation.

## Commitment Lifecycle

1. **Quote & stage receipts.**  
   The settlement façade (`SettlementEngine`, `SettlementAccumulator`) records a `PendingSettlement` per transaction. Each record stores the TWAP inputs, liquidity profile, timestamps, and XOR amounts so it can later become a `LaneSettlementReceipt`.

2. **Seal receipts into the block.**  
   During `BlockBuilder::finalize`, each `(lane_id, dataspace_id)` pair drains its accumulator. The builder instantiates a `LaneBlockCommitment`, copies the receipt list, accumulates totals, and stores optional swap metadata (via `SwapEvidence`). The resulting vector is pushed to the Sumeragi status slot (`crates/iroha_core/src/sumeragi/status.rs`) so Torii and telemetry can expose it immediately.

3. **Relay packaging & DA attestations.**  
   `LaneRelayBroadcaster` now consumes the `LaneRelayEnvelope`s emitted during block sealing and gossips them as high-priority `NetworkMessage::LaneRelay` frames. Envelopes are verified, de-duplicated by `(lane_id,dataspace_id,height,settlement_hash)`, and persisted in the Sumeragi status snapshot (`/v2/sumeragi/status`) for operators and auditors. The broadcaster will continue to evolve to attach DA artefacts (RBC chunk proofs, Norito headers, SoraFS/Object manifests) and feed the merge ring without head-of-line blocking.

4. **Global ordering & merge ledger.**  
   The NPoS ring validates each relay envelope: check `lane_qc` against the per-dataspace committee,
   recompute settlement totals, verify DA proofs, then feed the lane tip into the merge ledger
   reduction described in `docs/source/merge_ledger.md`. When the merge entry is sealed the
   world-state hash (`global_state_root`) now commits to every `LaneBlockCommitment`.

5. **Persistence & exposure.**  
   Kura writes the lane block, merge entry, and `LaneBlockCommitment` atomically so replay can reconstruct the same reduction. `/v2/sumeragi/status` exposes:
   - `lane_commitments` (execution metadata).
   - `lane_settlement_commitments` (the payload described here).
   - `lane_relay_envelopes` (relay headers, QCs, DA digests, settlement hash, and RBC byte counts).
  Dashboards (`dashboards/grafana/nexus_lanes.json`) read the same telemetry and status surfaces to display lane throughput, DA availability warnings, RBC volume, settlement deltas, and relay evidence.

## Verification & Proof Rules

The merge ring MUST enforce the following before accepting a lane commitment:

1. **Lane QC validity.** Verify the aggregated BLS signature over the execution-vote preimage
   (block hash, `parent_state_root`, `post_state_root`, height/view/epoch, chain_id, and mode tag)
   against the per-dataspace committee roster; ensure the signer bitmap length matches the
   committee, signers map to valid indices, and the header height matches
   `LaneBlockCommitment.block_height`.
2. **Receipt integrity.** Recompute the `total_*` aggregates from the receipt vector; reject the commitment if the sums diverge or the receipts contain duplicate `source_id`s.
3. **Swap metadata sanity.** Confirm that `swap_metadata` (if present) matches the lane’s current settlement configuration and buffer policy.
4. **DA attestation.** Validate that the relay-provided RBC/SoraFS proofs hash to the embedded digest and that the chunk set covers the entire block payload (`rbc_bytes_total` telemetry should mirror this).
5. **Merge reduction.** Once the per-lane proofs pass, include the lane tip in the merge ledger entry and recompute the Poseidon2 reduction (`reduce_merge_hint_roots`). Any mismatch aborts the merge entry.
6. **Telemetry & audit trail.** Increment the per-lane audit counters (`nexus_audit_outcome_total{lane_id,…}`) and persist the envelope so the evidence bundle contains both the proof and the observability trail.

## Data Availability & Observability

- **Metrics:**  
  `nexus_scheduler_lane_teu_*`, `nexus_scheduler_dataspace_*`, `sumeragi_rbc_da_reschedule_total`,
  `da_reschedule_total`, `sumeragi_da_gate_block_total{reason="missing_local_data"}`,
  `lane_relay_invalid_total{error}`, `lane_relay_emergency_override_total{outcome}`, and
  `nexus_audit_outcome_total` already exist in `crates/iroha_telemetry/src/metrics.rs`. Operators
  zero), and `lane_relay_invalid_total` should stay at zero outside adversarial drills.
- **Torii surfaces:**  
  `/v2/sumeragi/status` includes `lane_commitments`, `lane_settlement_commitments`, and dataspace snapshots. `/v2/nexus/lane-config` (planned) will publish the `LaneConfig` geometry so clients can match `lane_id` ↔ dataspace labels.
- **Dashboards:**  
  `dashboards/grafana/nexus_lanes.json` charts lane backlog, DA availability signals, and the settlement totals exposed above. Alert definitions should page when:
  - `nexus_scheduler_dataspace_age_slots` breaches policy.
  - `sumeragi_da_gate_block_total{reason="missing_local_data"}` increases persistently.
  - `total_xor_variance_micro` deviates from historical norms.
- **Evidence bundles:**  
  Every release must attach `LaneBlockCommitment` exports, Grafana/Alertmanager snapshots, and the relay DA manifests under `artifacts/nexus/cross-lane/<date>/`. The bundle becomes the canonical proof set when submitting NX-4 readiness reports.

## Implementation Checklist (NX-4)

1. **LaneRelay service**
   - Schema defined in `LaneRelayEnvelope`; broadcaster implemented in `crates/iroha_core/src/nexus/lane_relay.rs` and wired into block sealing (`crates/iroha_core/src/sumeragi/main_loop.rs`), emitting `NetworkMessage::LaneRelay` with per-node de-duplication and status persistence.
   - Persist relay artefacts for audits (`artifacts/nexus/relay/…`).
2. **DA attestation hooks**
   - Integrate RBC / SoraFS chunk proofs with relay envelopes and store summary metrics in `SumeragiStatus`.
   - Expose DA status via Torii and Grafana for operators.
3. **Merge-ledger validation**
   - Extend the merge entry validator to require relay envelopes, not raw lane headers.
   - Add replay tests (`integration_tests/tests/nexus/*.rs`) that feed synthetic commitments through the merge ledger and assert deterministic reduction.
4. **SDK & tooling updates**
   - Document the `LaneBlockCommitment` Norito layout for SDK consumers (`docs/portal/docs/nexus/lane-model.md` already links here; extend it with API snippets).
   - Deterministic fixtures live under `fixtures/nexus/lane_commitments/*.{json,to}`; run `cargo xtask nexus-fixtures` to regenerate (or `--verify` to validate) the `default_public_lane_commitment` and `cbdc_private_lane_commitment` samples whenever schema changes land.
5. **Observability & runbooks**
   - Wire the Alertmanager pack for the new metrics and document the evidence workflow in `docs/source/runbooks/nexus_cross_lane_incident.md` (follow-up).

Completing the checklist above, alongside this specification, satisfies the documentation portion of **NX-4** and unblocks the remaining implementation work.

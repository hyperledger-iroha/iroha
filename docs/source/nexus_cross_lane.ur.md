---
lang: ur
direction: rtl
source: docs/source/nexus_cross_lane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6e6f144bf3aef313ba55b539c9e92c827bd626973fe38b557f0b668cc909f589
source_last_modified: "2025-12-13T05:07:11.929584+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- اردو ترجمہ برائے docs/source/nexus_cross_lane.md -->

# Nexus میں cross-lane commitments اور proof pipeline

> **اسٹیٹس:** NX-4 deliverable — cross-lane commitment pipeline اور proofs (ہدف Q4 2025).  
> **مالکان:** Nexus Core WG / Cryptography WG / Networking TL.  
> **متعلقہ roadmap آئٹمز:** NX-1 (lane geometry)، NX-3 (settlement router)، NX-4 (یہ دستاویز)، NX-8 (global scheduler)، NX-11 (SDK conformance).

یہ نوٹ بیان کرتا ہے کہ ہر lane کے execution data کس طرح قابلِ تصدیق global commitment میں بدلتے ہیں۔ یہ موجودہ settlement router (`crates/settlement_router`)، lane block builder (`crates/iroha_core/src/block.rs`)، telemetry/status surfaces، اور منصوبہ بند LaneRelay/DA hooks کو جوڑتا ہے جو roadmap **NX-4** کے لئے ابھی باقی ہیں۔

## اہداف

- ہر lane block کے لئے ایک deterministic `LaneBlockCommitment` بنانا جو settlement، liquidity، اور variance data کو private state leak کئے بغیر capture کرے۔
- ان commitments (اور ان کی DA attestations) کو global NPoS ring تک relay کرنا تاکہ merge ledger cross-lane updates کو order، validate اور persist کر سکے۔
- انہی payloads کو Torii اور telemetry کے ذریعے expose کرنا تاکہ operators، SDKs اور auditors بغیر bespoke tooling کے pipeline replay کر سکیں۔
- NX-4 کے completion کے لئے درکار invariants اور evidence bundles واضح کرنا: lane proofs، DA attestations، merge-ledger integration، اور regression coverage۔

## Components اور Surfaces

| Component | ذمہ داری | Implementation references |
|-----------|----------------|---------------------------|
| Lane executor اور settlement router | XOR conversions quote کرنا، ہر transaction کے receipts جمع کرنا، buffer policy نافذ کرنا | `crates/iroha_core/src/settlement/mod.rs`, `crates/settlement_router` |
| Lane block builder | `SettlementAccumulator`s drain کرنا، lane block کے ساتھ `LaneBlockCommitment`s emit کرنا | `crates/iroha_core/src/block.rs:3340-3415` |
| LaneRelay broadcaster | lane QCs + DA proofs bundle کرنا، `iroha_p2p` کے ذریعے gossip کرنا، اور merge ring feed کرنا | `crates/iroha_core/src/nexus/lane_relay.rs`, `crates/iroha_core/src/sumeragi/main_loop.rs` |
| Global merge ledger | lane QCs verify کرنا، merge hints reduce کرنا، world-state commitments persist کرنا | `docs/source/merge_ledger.md`, `crates/iroha_core/src/sumeragi/status.rs`, `crates/iroha_core/src/state.rs` |
| Torii status اور dashboards | `lane_commitments`, `lane_settlement_commitments`, `lane_relay_envelopes`, scheduler gauges اور Grafana boards surface کرنا | `crates/iroha_torii/src/routing.rs:16660-16880`, `dashboards/grafana/nexus_lanes.json` |
| Evidence storage | `LaneBlockCommitment`s، RBC artefacts، اور Alertmanager snapshots کو audits کے لئے archive کرنا | `docs/settlement-router.md`, `artifacts/nexus/*` (future bundle) |

## Data Structures اور Payload Layout

Canonical payloads `crates/iroha_data_model/src/block/consensus.rs` میں ہیں۔

### `LaneSettlementReceipt`

- `source_id` — transaction hash یا caller-provided id.
- `local_amount_micro` — dataspace gas token debit.
- `xor_due_micro` / `xor_after_haircut_micro` / `xor_variance_micro` — deterministic XOR book entries اور فی receipt safety margin (`due - after haircut`).
- `timestamp_ms` — settlement کے دوران لیا گیا UTC millisecond timestamp.

Receipts، `SettlementEngine` کے deterministic quoting rules inherit کرتے ہیں اور ہر `LaneBlockCommitment` میں aggregate ہوتے ہیں۔

### `LaneSwapMetadata`

Optional metadata جو quoting کے دوران استعمال ہونے والے parameters record کرتی ہے:

- `epsilon_bps`, `twap_window_seconds`, `volatility_class`.
- `liquidity_profile` bucket (Tier1–Tier3).
- `twap_local_per_xor` string تاکہ auditors conversions کو بالکل دوبارہ compute کر سکیں۔

### `LaneBlockCommitment`

ہر lane کا خلاصہ جو ہر block کے ساتھ store ہوتا ہے:

- Header: `block_height`, `lane_id`, `dataspace_id`, `tx_count`.
- Totals: `total_local_micro`, `total_xor_due_micro`, `total_xor_after_haircut_micro`, `total_xor_variance_micro`.
- Optional `swap_metadata`.
- Ordered `receipts` vector.

یہ structs پہلے سے `NoritoSerialize`/`NoritoDeserialize` derive کرتے ہیں، اس لئے انہیں on-chain، Torii کے ذریعے، یا fixtures کے ذریعے schema drift کے بغیر stream کیا جا سکتا ہے۔

### `LaneRelayEnvelope`

`LaneRelayEnvelope` (دیکھیں `crates/iroha_data_model/src/nexus/relay.rs`) lane `BlockHeader`، optional `commit QC (`Qc`)`، optional `DaCommitmentBundle` hash، مکمل `LaneBlockCommitment`، اور per-lane RBC byte count کو package کرتا ہے۔ envelope ایک Norito-derived `settlement_hash` (via `compute_settlement_hash`) رکھتا ہے تاکہ receivers merge ledger تک forward کرنے سے پہلے settlement payload verify کر سکیں۔ اگر `verify` ناکام ہو (QC subject mismatch، DA hash mismatch، یا settlement hash mismatch)، اگر `verify_with_quorum` ناکام ہو (signer bitmap length/quorum errors)، یا اگر aggregated QC signature کو per-dataspace committee roster کے خلاف verify نہ کیا جا سکے تو envelopes reject ہونے چاہئیں۔ QC preimage میں lane block hash کے ساتھ `parent_state_root` اور `post_state_root` شامل ہوتے ہیں تاکہ membership اور state-root correctness ایک ساتھ verify ہوں۔

### Lane committee selection

Lane relay QCs کو per-dataspace committee کے خلاف validate کیا جاتا ہے۔ committee size `3f+1` ہے، جہاں `f` dataspace catalog (`fault_tolerance`) میں configure ہوتا ہے۔ validator pool dataspace validators پر مشتمل ہے: admin-managed lanes کے لئے lane governance manifests اور stake-elected lanes کے لئے public-lane staking records۔ committee membership ہر epoch میں deterministically sample ہوتی ہے، VRF epoch seed کے ذریعے جو `dataspace_id` اور `lane_id` کے ساتھ bind ہوتا ہے (epoch کے لئے stable)۔ اگر pool `3f+1` سے چھوٹا ہو تو lane relay finality quorum restore ہونے تک pause ہو جاتی ہے۔ operators admin multisig instruction `SetLaneRelayEmergencyValidators` کے ذریعے pool extend کر سکتے ہیں (requires `CanManagePeers` اور `nexus.lane_relay_emergency.enabled = true`, جو default پر disabled ہے)۔ enable ہونے پر authority کو configured minimums ( `nexus.lane_relay_emergency.multisig_threshold`/`multisig_members`, default 3-of-5 ) پورے کرنے والا multisig account ہونا چاہئے۔ overrides ہر dataspace میں store ہوتے ہیں، صرف quorum کے نیچے ہونے پر apply ہوتے ہیں، اور empty validator list submit کرنے پر clear ہو جاتے ہیں۔ اگر `expires_at_height` set ہو تو validation override کو ignore کرتی ہے جب lane relay envelope کا `block_height` expiry height سے اوپر چلا جائے۔ telemetry counter `lane_relay_emergency_override_total{lane,dataspace,outcome}` record کرتا ہے کہ override apply ہوا (`applied`) یا missing/expired/insufficient/disabled تھا۔

## Commitment Lifecycle

1. **Quote اور receipts stage کرنا۔**  
   settlement facade (`SettlementEngine`, `SettlementAccumulator`) ہر transaction کے لئے `PendingSettlement` record کرتا ہے۔ ہر record TWAP inputs، liquidity profile، timestamps، اور XOR amounts ذخیرہ کرتا ہے تاکہ بعد میں `LaneSettlementReceipt` بن سکے۔

2. **Receipts کو block میں seal کرنا۔**  
   `BlockBuilder::finalize` کے دوران ہر `(lane_id, dataspace_id)` pair اپنا accumulator drain کرتا ہے۔ builder ایک `LaneBlockCommitment` بناتا ہے، receipt list copy کرتا ہے، totals accumulate کرتا ہے، اور optional swap metadata (via `SwapEvidence`) store کرتا ہے۔ نتیجے والا vector Sumeragi status slot (`crates/iroha_core/src/sumeragi/status.rs`) میں push ہوتا ہے تاکہ Torii اور telemetry فوری طور پر expose کر سکیں۔

3. **Relay packaging اور DA attestations۔**  
   `LaneRelayBroadcaster` اب block sealing کے دوران emit ہونے والے `LaneRelayEnvelope`s consume کرتا ہے اور انہیں high-priority `NetworkMessage::LaneRelay` frames کے طور پر gossip کرتا ہے۔ envelopes verify ہوتے ہیں، `(lane_id,dataspace_id,height,settlement_hash)` کے لحاظ سے de-duplicate ہوتے ہیں، اور Sumeragi status snapshot (`/v2/sumeragi/status`) میں operators اور auditors کے لئے persist ہوتے ہیں۔ broadcaster آگے چل کر DA artefacts (RBC chunk proofs، Norito headers، SoraFS/Object manifests) attach کرے گا اور head-of-line blocking کے بغیر merge ring feed کرے گا۔

4. **Global ordering اور merge ledger۔**  
   NPoS ring ہر relay envelope validate کرتا ہے: per-dataspace committee کے خلاف `lane_qc` check کرنا، settlement totals recompute کرنا، DA proofs verify کرنا، پھر lane tip کو `docs/source/merge_ledger.md` میں بیان کردہ merge ledger reduction میں feed کرنا۔ جب merge entry seal ہوتی ہے تو world-state hash (`global_state_root`) ہر `LaneBlockCommitment` کو commit کرتا ہے۔

5. **Persistence اور exposure۔**  
   Kura lane block، merge entry اور `LaneBlockCommitment` کو atomically لکھتا ہے تاکہ replay اسی reduction کو reconstruct کر سکے۔ `/v2/sumeragi/status` expose کرتا ہے:
   - `lane_commitments` (execution metadata).
   - `lane_settlement_commitments` (یہاں بیان کردہ payload).
   - `lane_relay_envelopes` (relay headers، QCs، DA digests، settlement hash، اور RBC byte counts).
  Dashboards (`dashboards/grafana/nexus_lanes.json`) انہی telemetry/status surfaces کو پڑھتے ہیں تاکہ lane throughput، DA availability warnings، RBC volume، settlement deltas اور relay evidence دکھا سکیں۔

## Verification اور Proof Rules

merge ring کو lane commitment قبول کرنے سے پہلے یہ شرائط نافذ کرنی چاہئیں:

1. **Lane QC validity.** execution-vote preimage (block hash، `parent_state_root`, `post_state_root`, height/view/epoch, `chain_id` اور mode tag) پر aggregated BLS signature کو per-dataspace committee roster کے خلاف verify کریں؛ signer bitmap length کو committee کے مطابق رکھیں، signers کو valid indices پر map کریں، اور header height کو `LaneBlockCommitment.block_height` کے مطابق دیکھیں۔
2. **Receipt integrity.** receipt vector سے `total_*` aggregates دوبارہ compute کریں؛ اگر sums diverge ہوں یا receipts میں duplicate `source_id` ہوں تو commitment reject کریں۔
3. **Swap metadata sanity.** `swap_metadata` (اگر موجود ہو) lane کی settlement config اور buffer policy سے match ہونی چاہئے۔
4. **DA attestation.** relay-provided RBC/SoraFS proofs کو embedded digest کے خلاف validate کریں اور یقینی بنائیں کہ chunk set پورا block payload cover کرتا ہے (`rbc_bytes_total` telemetry میں یہی دکھنا چاہئے)۔
5. **Merge reduction.** lane proofs pass ہونے کے بعد lane tip کو merge ledger entry میں شامل کریں اور Poseidon2 reduction (`reduce_merge_hint_roots`) دوبارہ compute کریں۔ کوئی mismatch merge entry abort کرتا ہے۔
6. **Telemetry اور audit trail.** per-lane audit counters (`nexus_audit_outcome_total{lane_id,...}`) بڑھائیں اور envelope persist کریں تاکہ evidence bundle میں proof اور observability trail دونوں شامل ہوں۔

## Data Availability اور Observability

- **Metrics:**  
  `nexus_scheduler_lane_teu_*`, `nexus_scheduler_dataspace_*`, `sumeragi_rbc_da_reschedule_total`,
  `da_reschedule_total`, `sumeragi_da_gate_block_total{reason="missing_local_data"}`,
  `lane_relay_invalid_total{error}`, `lane_relay_emergency_override_total{outcome}`, اور
- **Torii surfaces:**  
  `/v2/sumeragi/status` میں `lane_commitments`, `lane_settlement_commitments` اور dataspace snapshots شامل ہیں۔ `/v2/nexus/lane-config` (planned) `LaneConfig` geometry publish کرے گا تاکہ clients `lane_id` کو dataspace labels کے ساتھ map کر سکیں۔
- **Dashboards:**  
  `dashboards/grafana/nexus_lanes.json` lane backlog، DA availability signals، اور اوپر بیان کردہ settlement totals دکھاتا ہے۔ Alert definitions کو page کرنا چاہئے جب:
  - `nexus_scheduler_dataspace_age_slots` policy کو breach کرے۔
  - `sumeragi_da_gate_block_total{reason="missing_local_data"}` مسلسل بڑھے۔
  - `total_xor_variance_micro` historical norms سے deviate کرے۔
- **Evidence bundles:**  
  ہر release کو `LaneBlockCommitment` exports، Grafana/Alertmanager snapshots، اور relay DA manifests کو `artifacts/nexus/cross-lane/<date>/` میں attach کرنا چاہئے۔ یہ bundle NX-4 readiness reports جمع کراتے وقت canonical proof set بنتا ہے۔

## Implementation Checklist (NX-4)

1. **LaneRelay service**
   - `LaneRelayEnvelope` میں schema defined؛ broadcaster `crates/iroha_core/src/nexus/lane_relay.rs` میں implement اور block sealing (`crates/iroha_core/src/sumeragi/main_loop.rs`) کے ساتھ wired، `NetworkMessage::LaneRelay` emit کرتے ہوئے per-node de-duplication اور status persistence کے ساتھ۔
   - audit کے لئے relay artefacts persist کریں (`artifacts/nexus/relay/...`).
2. **DA attestation hooks**
   - RBC / SoraFS chunk proofs کو relay envelopes کے ساتھ integrate کریں اور summary metrics کو `SumeragiStatus` میں store کریں۔
   - DA status کو Torii اور Grafana کے ذریعے operators کے لئے expose کریں۔
3. **Merge-ledger validation**
   - merge entry validator کو extend کریں تاکہ raw lane headers کی بجائے relay envelopes لازم ہوں۔
   - replay tests (`integration_tests/tests/nexus/*.rs`) شامل کریں جو synthetic commitments merge ledger میں feed کریں اور deterministic reduction assert کریں۔
4. **SDK اور tooling updates**
   - `LaneBlockCommitment` Norito layout کو SDK consumers کے لئے document کریں (`docs/portal/docs/nexus/lane-model.md` پہلے سے یہاں link کرتا ہے؛ API snippets شامل کریں)۔
   - deterministic fixtures `fixtures/nexus/lane_commitments/*.{json,to}` میں ہیں؛ schema change پر `cargo xtask nexus-fixtures` چلا کر regenerate (یا `--verify` سے validate) کریں اور `default_public_lane_commitment`، `cbdc_private_lane_commitment` samples update کریں۔
5. **Observability اور runbooks**
   - نئی metrics کے لئے Alertmanager pack wire کریں اور evidence workflow کو `docs/source/runbooks/nexus_cross_lane_incident.md` میں document کریں (follow-up).

اوپر کی checklist، اس specification کے ساتھ، **NX-4** کے documentation حصے کو مکمل کرتی ہے اور باقی implementation work کو unblock کرتی ہے۔

</div>

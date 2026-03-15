---
lang: ar
direction: rtl
source: docs/source/sorafs/normative_parameters.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bc212da49d2198dd7491d724bd5b46ee91ad35091daecf8f1418f62d195e4da9
source_last_modified: "2026-01-22T15:38:30.692817+00:00"
translation_last_reviewed: 2026-01-30
---

# SoraFS Normative Parameters Snapshot (Q4 2026)

This note records the parameters that every SoraFS deployment must enforce
during the SF‑6/SF‑8 rollouts. It expands the short **Roadmap → SoraFS Normative
Parameters Snapshot** entry into a stand-alone reference that pairs each guard‐
rail with concrete verification steps. Keep it alongside the wire-format
reference (`docs/source/sorafs_proto_plan.md`) so operators, SDK authors, and
auditors share a single source of truth.

## Quick Reference

| Domain | Parameter | Value / Behaviour | Evidence & Tooling |
|--------|-----------|-------------------|--------------------|
| Chunking & proofs | CDC profile | Target 256 KiB (64 KiB min, 512 KiB max) with BLAKE3 digests, two-level (64 KiB / 4 KiB) PoR tree per chunk. | `cargo run -p sorafs_chunker --bin export_vectors` and `ci/check_sorafs_fixtures.sh` (see `docs/source/sorafs/chunker_conformance.md`). `sorafs_car::ChunkStore` emits the PoR tree (`sorafs_car/src/lib.rs`). |
| Provider adverts | Refresh/expiry | Providers refresh adverts every 12 h; TTL is 24 h. Payloads are signed `ProviderAdvertBodyV1` envelopes with deterministic nonce/QoS metadata. | Provider advert publishing pipelines plus the admission fixtures enforce cadence/TLV limits. Dashboards: `torii_sorafs_admission_total`, `torii_sorafs_provider_range_capability_total` (see `docs/source/sorafs/provider_advert_multisource.md`). |
| Proof cadence | PoR/PDP windows | 1 h epochs, ≤32 samples, 10 m probe window + 2 m grace. Sora-PDP runs on all hot replicas; Sora-PoTR classifies 90 s (hot) / 5 m (warm) deadlines. | `sorafs_cli proof verify` (PoR) and PDP probe harnesses; dashboards `dashboards/grafana/sorafs_capacity_health.json` (`torii_da_pdp_bonus_micro_total`, `torii_da_potr_bonus_micro_total`). HTTP fetches must carry `Sora-PDP` / `Sora-PoTR` headers (`docs/source/soradns_gateway_content_binding.md`). |
| Replication orders | Acceptance SLA | Orders expire if not accepted within 5 minutes, require `precommit` plus QA PoR `PASS` before completion, and every artefact lands in the GovernanceLog DAG. | `torii_sorafs_registry_orders_total`, `torii_sorafs_replication_sla_total`, and the DAG fixtures in `docs/source/sorafs_governance_dag_plan.md`. Regenerate fixtures via `cargo run -p sorafs_manifest --bin generate_replication_order_fixture`. |
| Pricing & auto-renew | Billing currency & decay ladder | Charges denominated in **XOR** with USD TWAP surfaced in UX; auto-renew enabled by default with per-account spend caps. Degradation ladder: Full service → Read-only (30 d) → Reduced replicas (30 d) → Cold archive (60 d) → Tombstone. | `sorafs.node.deal_*` metrics (deal engine), dashboards `dashboards/grafana/sorafs_capacity_health.json`, docs `docs/source/sorafs/deal_engine.md` and `docs/source/sorafs/storage_capacity_marketplace.md`. |
| Transport | Default mode | Read-only retrieval defaults to SoraNet anonymity (entry/middle/exit relays). Direct mode requires explicit overrides in CLI/SDK. Handshake uses hybrid X25519 + Kyber (SNNet‑16) with PQ roll-out for relay identities/tickets. | `sorafs_cli fetch` / `sorafs_fetch` policy overrides, `crates/iroha_cli/src/commands/streaming.rs`, `crates/soranet_pq`, and the PQ rollout plan (`docs/source/soranet/pq_rollout_plan.md`). Dashboards flag direct-mode downgrades via `torii_stream_transport_policy_total`. |

## 1. Chunking & Proof Layout

- Canonical profile: `sorafs.sf1@1.0.0` (see `docs/source/sorafs/chunker_conformance.md`).
- PoR trees hash 64 KiB segments down to 4 KiB leaves so partial reads retain
  power-of-two coverage even when providers stream smaller windows.
- Verification workflow:

  ```bash
  cargo run --locked -p sorafs_chunker --bin export_vectors
  ci/check_sorafs_fixtures.sh
  cargo test -p sorafs_car --lib chunk_store::tests::por_tree_smoke
  ```

  The chunk store emits deterministic `chunk_{index}.bin` folders plus per-leaf
  witnesses reused by `da_reconstruct.rs` and the gateway proof streaming API.

## 2. Provider Advert Lifecycle

- Adverts are regenerated every 12 hours, expire after 24 hours, and must be
  re-signed before Torii will serve them via `/v1/sorafs/providers`.
- `ProviderAdvertBodyV1` carries QoS, stake, and transport hints; governance
  compares deterministic nonces to reject replayed payloads.
- Operators submit new adverts by posting the signed Norito payload emitted by
  their provider pipeline:

  ```bash
  curl -sS -X POST --data-binary @provider_advert.to \
    http://<torii-host>/v1/sorafs/provider/advert
  ```

- Dashboards to watch: `torii_sorafs_admission_total` (result labels), warning
  ratios from `docs/source/sorafs/provider_advert_rollout.md`, and the range
  capability gauges noted above. Pages fire if warning rate >5% or rejections >0.

## 3. Proof Windows & Headers

- PoR probes: orchestrator schedules ≤32 samples per epoch with a 10 minute
  window and 2 minute grace. PDP runs every hour on hot replicas regardless of
  fetch volume; warm/cold replicas follow the PDP cadence defined in
  `docs/source/sorafs_proof_streaming_plan.md`.
- PoTR: classify 90 second deadlines for hot replicas and 5 minute deadlines for
  warm replicas. Receipts are signed `PotrReceiptV1` payloads.
- HTTP clients MUST forward `Sora-PDP` and (when enabled) `Sora-PoTR-*` headers
  when retrieving via gateways; governance refuses captures without them
  (`docs/source/soradns_gateway_content_binding.md`).
- Verification hooks:

  ```bash
  sorafs_cli proof verify \
    --manifest artifacts/sorafs/sample.manifest.to \
    --car artifacts/sorafs/sample.car \
    --summary-out artifacts/sorafs/sample.pdp.json

  python3 scripts/sorafs_proof_probe.py --manifest <cid> --samples 32
  ```

## 4. Replication Order Lifecycle

- Issued orders expire if providers do not acknowledge (`precommit`) within
  5 minutes. Torii increments `torii_sorafs_registry_orders_total{status="expired"}`
  and fires the `SorafsReplicationExpiredOrders` alert defined in
  `docs/source/sorafs/runbooks/pin_registry_ops.md`.
- Completion requires:
  1. `precommit` ack from the provider,
  2. QA PoR `PASS` published to the governance DAG, and
  3. `torii_sorafs_replication_sla_total{outcome="met"}` increment within the
     same epoch.
- Artefacts: `GovernanceLogNodeV1` entries (see
  `docs/source/sorafs_governance_dag_plan.md`) reference the manifest digest,
  provider id, replication order id, and PoR summary so operators replay Every
  stage.
- Fixture refresh:

  ```bash
  cargo run -p sorafs_manifest --bin generate_replication_order_fixture \
    -- --manifest fixtures/sorafs_manifest/ci_sample/manifest.json
  ```

## 5. Pricing, Auto-Renew, and Degradation Ladder

- Deals denominate storage/egress charges in **XOR** and expose USD-tracking
  metadata so UX can surface fiat equivalents (`docs/source/sorafs/deal_engine.md`).
- Auto-renew is **enabled by default**. Clients configure caps via Norito
  payloads (`DealTermsV1.account_cap`, `DealTermsV1.egress_cap`) and the CLI/SDK
  surfaces them through `sorafs_cli deal terms` / `iroha app sorafs deal terms`.
- Degradation ladder:
  1. **Full service** — healthy autopayments.
  2. **Read-only** (30 days) — replication orders freeze; clients can fetch data
     but cannot mutate.
  3. **Reduced replicas** (30 days) — governance reassigns to the minimum replica
     count that keeps proofs possible.
  4. **Cold archive** (60 days) — archived in low-cost storage; fetches require
     operator approval.
  5. **Tombstone** — metadata retained for audit; data removed.
- Monitor `sorafs.node.deal_*`, `torii_sorafs_capacity_*`, and the rent/bonus
  panels in `dashboards/grafana/sorafs_capacity_health.json`. Alert if
  `deal_outstanding_nano` grows while auto-renew remains enabled.

## 6. SoraNet Transport Defaults

- `sorafs_cli fetch` and SDKs default to `transport_policy=soranet-first` with
  three-hop circuits (entry/middle/exit). Downshifting to `direct-only` or
  `max_peers=1` requires an explicit override recorded in adoption artefacts
  (see `docs/source/sorafs_orchestrator_rollout.md`).
- Gateway relays must honour GAR policies for anonymised traffic. Deterministic
  host mapping + GAR scaffolding are generated via `cargo xtask soradns-hosts`,
  `cargo xtask soradns-gar-template`, and `cargo xtask soradns-binding-template`.
- Handshake: hybrid X25519 + Kyber (ML-KEM‑768) per `crates/iroha_crypto` and
  `crates/soranet_pq`. The PQ rollout plan in `docs/source/soranet/pq_rollout_plan.md`
  tracks ticket/identity adoption gates. CLI helpers (`iroha app streaming hpke
  fingerprint`) expose fingerprints used in telemetry.
- Telemetry: `torii_stream_transport_policy_total` and
  `torii_stream_anonymity_stage_total` back the SNNet dashboards. Alerts fire
  when direct-mode usage exceeds the incident budget specified in
  `docs/source/ops/soranet_transport_rollback.md`.

---

**Maintenance:** update this snapshot whenever chunking, proof, or transport
parameters change. Include the commit hash, dashboard UID, or CLI command
needed to reproduce each value so governance can tie roadmap reviews to a
static artefact. Mirror significant edits into the portal copy if one is added.

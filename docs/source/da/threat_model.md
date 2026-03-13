# Sora Nexus Data Availability Threat Model

_Last reviewed: 2026-01-19 — Next scheduled review: 2026-04-19_

Maintenance cadence: Data Availability Working Group (<=90 days). Every revision must
appear in `status.md` with links to active mitigation tickets and simulation artefacts.

## Purpose and Scope

The Data Availability (DA) program keeps Taikai broadcasts, Nexus lane blobs, and
governance artefacts retrievable under Byzantine, network, and operator faults.
This threat model anchors engineering work for DA-1 (architecture and threat model)
and serves as the baseline for downstream DA tasks (DA-2 through DA-10).

In-scope components:
- Torii DA ingest extension and Norito metadata writers.
- SoraFS-backed blob storage trees (hot/cold tiers) and replication policies.
- Nexus block commitments (wire formats, proofs, light-client APIs).
- PDP/PoTR enforcement hooks specific to DA payloads.
- Operator workflows (pinning, eviction, slashing) and observability pipelines.
- Governance approvals that admit or evict DA operators and content.

Out-of-scope for this document:
- Full economics modelling (captured in DA-7 workstream).
- SoraFS base protocols already covered by the SoraFS threat model.
- Client SDK ergonomics beyond threat-surface considerations.

## Architectural Overview

1. **Submission:** Clients submit blobs via the Torii DA ingest API. The node
   chunks blobs, encodes Norito manifests (blob type, lane, epoch, codec flags),
   and stores chunks in the hot SoraFS tier.
2. **Advertisement:** Pin intents and replication hints propagate to storage
   providers through the registry (SoraFS marketplace) with policy tags that
   state hot/cold retention targets.
3. **Commitment:** Nexus sequencers include blob commitments (CID + optional KZG
   roots) in the canonical block. Light clients rely on the commitment hash and
   advertised metadata to verify availability.
4. **Replication:** Storage nodes pull assigned shares/chunks, satisfy PDP/PoTR
   challenges, and promote data between hot and cold tiers per policy.
5. **Fetch:** Consumers fetch data through SoraFS or DA-aware gateways, verifying
   proofs and raising repair requests when replicas disappear.
6. **Governance:** Parliament and the DA oversight committee approve operators,
   rent schedules, and enforcement escalations. Governance artefacts are stored
   via the same DA path to ensure process transparency. The rent parameters
   tracked under DA-7 are recorded in `docs/source/da/rent_policy.md` so audits
   and enforcement reviews can reference the exact XOR amounts applied per blob.

## Assets and Owners

Impact scale: **Critical** breaks ledger safety/liveness; **High** blocks DA
backfill or clients; **Moderate** degrades quality but remains recoverable;
**Low** limited effect.

| Asset | Description | Integrity | Availability | Confidentiality | Owner |
| --- | --- | --- | --- | --- | --- |
| DA blobs (chunks + manifests) | Taikai, lane, governance blobs stored in SoraFS | Critical | Critical | Moderate | DA WG / Storage Team |
| Norito DA manifests | Typed metadata describing blobs | Critical | High | Moderate | Core Protocol WG |
| Block commitments | CIDs + KZG roots inside Nexus blocks | Critical | High | Low | Core Protocol WG |
| PDP/PoTR schedules | Enforcement cadence for DA replicas | High | High | Low | Storage Team |
| Operator registry | Approved storage providers & policies | High | High | Low | Governance Council |
| Rent and incentive records | Ledger entries for DA rent & penalties | High | Moderate | Low | Treasury WG |
| Observability dashboards | DA SLOs, replication depth, alerts | Moderate | High | Low | SRE / Observability |
| Repair intents | Requests to rehydrate missing chunks | Moderate | Moderate | Low | Storage Team |

## Adversaries and Capabilities

| Actor | Capabilities | Motivations | Notes |
| --- | --- | --- | --- |
| Malicious client | Submit malformed blobs, replay stale manifests, attempt DoS on ingest. | Disrupt Taikai broadcasts, inject invalid data. | No privileged keys. |
| Byzantine storage node | Drop assigned replicas, forge PDP/PoTR proofs, collude with others. | Cut DA retention, avoid rent, hold data hostage. | Possesses valid operator credentials. |
| Compromised sequencer | Omit commitments, equivocate on blocks, reorder blob metadata. | Hide DA submissions, create inconsistency. | Limited by consensus majority. |
| Insider operator | Abuse governance access, tamper with retention policies, leak credentials. | Economic gain, sabotage. | Access to hot/cold tier infrastructure. |
| Network adversary | Partition nodes, delay replication, inject MITM traffic. | Reduce availability, degrade SLOs. | Cannot break TLS but can drop/slow links. |
| Observability attacker | Tamper dashboards/alerts, suppress incidents. | Hide DA outages. | Requires access to telemetry pipeline. |

## Trust Boundaries

- **Ingress boundary:** Client to Torii DA extension. Requires request-level auth,
  rate limiting, and payload validation.
- **Replication boundary:** Storage nodes exchanging chunks and proofs. Nodes are
  mutually authenticated but may behave Byzantine.
- **Ledger boundary:** Committed block data vs off-chain storage. Consensus guards
  integrity, but availability requires off-chain enforcement.
- **Governance boundary:** Council/Parliament decisions approving operators,
  budgets, and slashing. Breaks here directly impact DA deployment.
- **Observability boundary:** Metrics/log collection exported to dashboards/alert
  tooling. Tampering hides outages or attacks.

## Threat Scenarios and Controls

### Ingest Path Attacks

**Scenario:** Malicious client submits malformed Norito payloads or oversized
blobs to exhaust resources or smuggle invalid metadata.

**Controls**
- Norito schema validation with strict version negotiation; reject unknown flags.
- Rate limiting and authentication at the Torii ingest endpoint.
- Chunk size bounds and deterministic encoding enforced by SoraFS chunker.
- Admission pipeline only persists manifests after integrity checksum matches.
- Deterministic replay cache (`ReplayCache`) tracks `(lane, epoch, sequence)` windows, persists high-water marks on disk, and rejects duplicates/stale replays; property and fuzz harnesses cover divergent fingerprints and out-of-order submissions.【crates/iroha_core/src/da/replay_cache.rs:1】【fuzz/da_replay_cache.rs:1】【crates/iroha_torii/src/da/ingest.rs:1】

**Residual gaps**
- Torii ingest must thread the replay cache into admission and persist sequence cursors across restarts.
- Norito DA schemas now have a dedicated fuzz harness (`fuzz/da_ingest_schema.rs`) to stress encode/decode invariants; coverage dashboards should alert if the target regresses.

### Replication Withholding

**Scenario:** Byzantine storage operators accept pin assignments but drop chunks,
passing PDP/PoTR challenges via forged responses or collusion.

**Controls**
- PDP/PoTR challenge schedule extends to DA payloads with per-epoch coverage.
- Multi-source replication with quorum thresholds; fetch orchestrator detects
  missing shards and triggers repair.
- Governance slashing linked to failed proofs and missing replicas.

**Residual gaps**
- Simulation harness in `integration_tests/src/da/pdp_potr.rs` (covered by
  `integration_tests/tests/da/pdp_potr_simulation.rs`) now exercises collusion
  and partition scenarios, validating that the PDP/PoTR schedule detects
  Byzantine behaviour deterministically. Continue extending it alongside DA-5 to
  cover new proof surfaces.
- Cold-tier eviction policy requires signed audit trail to prevent covert drops.

### Commitment Tampering

**Scenario:** Compromised sequencer publishes blocks omitting or altering DA
commitments, causing fetch failures or light-client inconsistencies.

**Controls**
- Consensus cross-checks block proposals with DA submission queues; peers reject
  proposals missing required commitments.
- Light clients verify commitment inclusion proofs before surfacing fetch handles.
- Audit trail comparing submission receipts with block commitments.
- Automated reconciliation job (`cargo xtask da-commitment-reconcile`) compares
  ingest receipts with DA commitments (SignedBlockWire, `.norito`, or JSON),
  emits a JSON evidence bundle for governance, and fails on missing or
  mismatched tickets so Alertmanager can page on omission/tampering.

**Residual gaps**
- Covered by the reconciliation job + Alertmanager hook; governance packets now
  ingest the JSON evidence bundle by default.

### Network Partition and Censorship

**Scenario:** Adversary partitions replication network, preventing nodes from
obtaining assigned chunks or responding to PDP/PoTR challenges.

**Controls**
- Multi-region provider requirements ensure diverse network paths.
- Challenge windows include jitter and fallback to out-of-band repair channels.
- Observability dashboards monitor replication depth, challenge success, and
  fetch latency with alert thresholds.

**Residual gaps**
- Partition simulations for Taikai live events still missing; need soak tests.
- Repair bandwidth reservation policy not yet codified.

### Insider Abuse

**Scenario:** Operator with registry access manipulates retention policies,
whitelists malicious providers, or suppresses alerts.

**Controls**
- Governance actions require multi-party signatures and Norito-notarised records.
- Policy changes emit events to monitoring and archival logs.
- Observability pipeline enforces append-only Norito logs with hash chaining.
- Quarterly access review automation (`cargo xtask da-privilege-audit`) walks
  the DA manifest/replay directories (plus operator-supplied paths), flags
  missing/non-directory/world-writable entries, and emits a signed JSON bundle
  for governance dashboards.

**Residual gaps**
- Dashboard tamper-evidence requires signed snapshots.

## Residual Risk Register

| Risk | Likelihood | Impact | Owner | Mitigation Plan |
| --- | --- | --- | --- | --- |
| Replay of DA manifests before DA-2 sequence cache lands | Possible | Moderate | Core Protocol WG | Implement sequence cache + nonce validation in DA-2; add regression tests. |
| PDP/PoTR collusion when >f nodes compromise | Unlikely | High | Storage Team | Derive new challenge schedule with cross-provider sampling; validate via simulation harness. |
| Cold-tier eviction audit gap | Possible | High | SRE / Storage Team | Attach signed audit logs & on-chain receipts for evictions; monitor via dashboards. |
| Sequencer omission detection latency | Possible | High | Core Protocol WG | Nightly `cargo xtask da-commitment-reconcile` compares receipts vs commitments (SignedBlockWire/`.norito`/JSON) and pages governance on missing or mismatched tickets. |
| Partition resilience for Taikai live streams | Possible | Critical | Networking TL | Execute partition drills; reserve repair bandwidth; document failover SOP. |
| Governance privilege drift | Unlikely | High | Governance Council | Quarterly `cargo xtask da-privilege-audit` run (manifest/replay dirs + extra paths) with signed JSON + dashboard gate; anchor audit artefacts on-chain. |

## Required Follow-Ups

1. Publish DA ingest Norito schemas and example vectors (carried into DA-2).
2. Thread the replay cache through Torii DA ingest and persist sequence cursors across node restarts.
3. **Completed (2026-02-05):** PDP/PoTR simulation harness now exercises collusion + partition scenarios with QoS backlog modelling; see [`integration_tests/src/da/pdp_potr.rs`](/integration_tests/src/da/pdp_potr.rs) (with tests under `integration_tests/tests/da/pdp_potr_simulation.rs`) for the implementation and deterministic summaries captured below.
4. **Completed (2026-05-29):** `cargo xtask da-commitment-reconcile` compares ingest receipts against DA commitments (SignedBlockWire/`.norito`/JSON), emits `artifacts/da/commitment_reconciliation.json`, and is wired into Alertmanager/governance packets for omission/tampering alerts (`xtask/src/da.rs`).
5. **Completed (2026-05-29):** `cargo xtask da-privilege-audit` walks the manifest/replay spool (plus operator-supplied paths), flags missing/non-directory/world-writable entries, and produces a signed JSON bundle for dashboards/governance reviews (`artifacts/da/privilege_audit.json`), closing the access-review automation gap.

**Where to look next:**

- The DA replay cache and cursor persistence landed in DA-2. See the
  implementation in `crates/iroha_core/src/da/replay_cache.rs` (cache logic) and
  the Torii integration in `crates/iroha_torii/src/da/ingest.rs`, which threads the
  fingerprint checks through `/v2/da/ingest`.
- PDP/PoTR streaming simulations are exercised via the proof-stream harness in
  `crates/sorafs_car/tests/sorafs_cli.rs`, covering PoR/PDP/PoTR request flows
  and failure scenarios animated in the threat model.
- Capacity and repair soak results live under
  `docs/source/sorafs/reports/sf2c_capacity_soak.md`, while the broader
  Sumeragi soak matrix is tracked in `docs/source/sumeragi_soak_matrix.md`
  (localized variants included). These artefacts capture the long-running drills
  referenced in the residual risk register.
- Reconciliation + privilege-audit automation lives in
  `docs/automation/da/README.md` and the new `cargo xtask da-commitment-reconcile`
  / `cargo xtask da-privilege-audit` commands; use the default outputs under
  `artifacts/da/` when attaching evidence to governance packets.

## Simulation Evidence & QoS Modelling (2026-02)

To close DA-1 follow-up #3, we codified a deterministic PDP/PoTR simulation
harness under `integration_tests/src/da/pdp_potr.rs` (covered by
`integration_tests/tests/da/pdp_potr_simulation.rs`). The harness
allocates nodes across three regions, injects partitions/collusion according to
the roadmap probabilities, tracks PoTR lateness, and feeds a repair-backlog
model that mirrors the hot-tier repair budget. Running the default scenario
(12 epochs, 18 PDP challenges + 2 PoTR windows per epoch) produced the
following metrics:

<!-- BEGIN_DA_SIM_TABLE -->
<!-- AUTO-GENERATED by scripts/docs/render_da_threat_model_tables.py; do not edit manually. -->
| Metric | Value | Notes |
| --- | --- | --- |
| PDP failures detected | 48 / 49 (98.0%) | Partitions still trigger detection; a single undetected failure comes from honest jitter. |
| PDP mean detection latency | 0.0 epochs | Failures are surfaced within the originating epoch. |
| PoTR failures detected | 28 / 77 (36.4%) | Detection fires once a node misses ≥2 PoTR windows, leaving most events in the residual-risk register. |
| PoTR mean detection latency | 2.0 epochs | Matches the two-epoch lateness threshold baked into archival escalation. |
| Repair queue peak | 38 manifests | Backlog spikes when partitions stack faster than the four repairs available per epoch. |
| Response latency p95 | 30,068 ms | Mirrors the 30 s challenge window with the ±75 ms jitter applied for QoS sampling. |
<!-- END_DA_SIM_TABLE -->

These outputs now drive the DA dashboard prototypes and satisfy the “simulation
harness + QoS modelling” acceptance criteria referenced in the roadmap.

Automation now lives behind `cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`, which calls the shared harness and
emits Norito JSON to `artifacts/da/threat_model_report.json` by default. Nightly
jobs consume this file to refresh the matrices in this document and to alert on
drift in detection rates, repair queues, or QoS samples.

To refresh the table above for docs, run `make docs-da-threat-model`, which
invokes `cargo xtask da-threat-model-report`, regenerates
`docs/source/da/_generated/threat_model_report.json`, and rewrites this section
via `scripts/docs/render_da_threat_model_tables.py`. The `docs/portal` mirror
(`docs/portal/docs/da/threat-model.md`) is updated in the same pass so both
copies stay in sync.

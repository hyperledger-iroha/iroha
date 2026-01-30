---
lang: es
direction: ltr
source: docs/portal/docs/da/threat-model.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note مستند ماخذ
ہونے تک دونوں ورژنز کو sync رکھیں۔
:::

# Sora Nexus Data Availability Threat Model

_آخری جائزہ: 2026-01-19 -- اگلا شیڈول شدہ جائزہ: 2026-04-19_

Maintenance cadence: Data Availability Working Group (<=90 days). ہر ریویژن
`status.md` میں لازمی درج ہو، اور اس میں active mitigation tickets اور
simulation artefacts کے روابط شامل ہوں۔

## مقصد اور دائرہ کار

Data Availability (DA) پروگرام Taikai broadcasts، Nexus lane blobs، اور governance
artefacts کو Byzantine، نیٹ ورک، اور آپریٹر کی خرابیوں میں بھی قابل بازیافت
رکھتا ہے۔ یہ threat model DA-1 (architecture اور threat model) کیلئے انجینئرنگ
کام کی بنیاد ہے اور downstream DA tasks (DA-2 تا DA-10) کیلئے baseline ہے۔

In-scope components:
- Torii DA ingest extension اور Norito metadata writers.
- SoraFS-backed blob storage trees (hot/cold tiers) اور replication policies.
- Nexus block commitments (wire formats, proofs, light-client APIs).
- PDP/PoTR enforcement hooks جو DA payloads کیلئے مخصوص ہیں.
- Operator workflows (pinning, eviction, slashing) اور observability pipelines.
- Governance approvals جو DA operators اور content کو admit یا evict کرتے ہیں۔

Out-of-scope for this document:
- مکمل economics modelling (DA-7 workstream میں).
- SoraFS base protocols جو پہلے سے SoraFS threat model میں شامل ہیں.
- Client SDK ergonomics beyond threat-surface considerations.

## Architectural Overview

1. **Submission:** Clients blobs کو Torii DA ingest API کے ذریعے submit کرتے ہیں۔
   Node blobs کو chunks میں بانٹتا ہے، Norito manifests encode کرتا ہے (blob type,
   lane, epoch, codec flags)، اور chunks کو hot SoraFS tier میں محفوظ کرتا ہے۔
2. **Advertisement:** Pin intents اور replication hints registry (SoraFS marketplace)
   کے ذریعے storage providers تک جاتے ہیں، policy tags کے ساتھ جو hot/cold retention
   targets بتاتے ہیں۔
3. **Commitment:** Nexus sequencers blob commitments (CID + optional KZG roots)
   canonical block میں شامل کرتے ہیں۔ Light clients commitment hash اور advertised
   metadata کی بنیاد پر availability verify کرتے ہیں۔
4. **Replication:** Storage nodes assigned shares/chunks کھینچتے ہیں، PDP/PoTR
   challenges مکمل کرتے ہیں، اور policy کے مطابق hot/cold tiers کے درمیان data
   promote کرتے ہیں۔
5. **Fetch:** Consumers SoraFS یا DA-aware gateways کے ذریعے data fetch کرتے ہیں،
   proofs verify کرتے ہیں، اور replicas غائب ہونے پر repair requests اٹھاتے ہیں۔
6. **Governance:** Parliament اور DA oversight committee operators، rent schedules،
   اور enforcement escalations کو approve کرتی ہے۔ Governance artefacts اسی DA
   path سے گزرتے ہیں تاکہ transparency برقرار رہے۔

## Assets and Owners

Impact scale: **Critical** ledger safety/liveness توڑتا ہے؛ **High** DA backfill
یا clients کو بلاک کرتا ہے؛ **Moderate** quality کم کرتا ہے مگر recoverable؛
**Low** محدود اثر۔

| Asset | Description | Integrity | Availability | Confidentiality | Owner |
| --- | --- | --- | --- | --- | --- |
| DA blobs (chunks + manifests) | Taikai, lane, governance blobs in SoraFS | Critical | Critical | Moderate | DA WG / Storage Team |
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
| Malicious client | Malformed blobs submit کرنا، stale manifests replay کرنا، ingest پر DoS کی کوشش۔ | Taikai broadcasts disrupt کرنا، invalid data inject کرنا۔ | Privileged keys نہیں۔ |
| Byzantine storage node | Assigned replicas drop کرنا، PDP/PoTR proofs forge کرنا، collude کرنا۔ | DA retention کم کرنا، rent سے بچنا، data hostage بنانا۔ | Valid operator credentials رکھتا ہے۔ |
| Compromised sequencer | Commitments omit کرنا، blocks پر equivocate کرنا، metadata reorder کرنا۔ | DA submissions چھپانا، inconsistency پیدا کرنا۔ | Consensus majority سے محدود۔ |
| Insider operator | Governance access abuse کرنا، retention policies tamper کرنا، credentials leak کرنا۔ | Economic gain، sabotage۔ | Hot/cold tier infra تک رسائی۔ |
| Network adversary | Nodes partition کرنا، replication delay کرنا، MITM traffic inject کرنا۔ | Availability کم کرنا، SLOs degrade کرنا۔ | TLS نہیں توڑ سکتا مگر links slow/drop کر سکتا ہے۔ |
| Observability attacker | Dashboards/alerts tamper کرنا، incidents suppress کرنا۔ | DA outages چھپانا۔ | Telemetry pipeline تک رسائی درکار۔ |

## Trust Boundaries

- **Ingress boundary:** Client سے Torii DA extension۔ Request-level auth، rate limiting،
  اور payload validation درکار ہیں۔
- **Replication boundary:** Storage nodes chunks اور proofs exchange کرتے ہیں۔ Nodes
  باہمی authenticated ہیں مگر Byzantine برتاؤ ممکن ہے۔
- **Ledger boundary:** Committed block data بمقابلہ off-chain storage۔ Consensus
  integrity guard کرتا ہے، مگر availability کیلئے off-chain enforcement ضروری ہے۔
- **Governance boundary:** Council/Parliament کے فیصلے operators، budgets، slashing
  approve کرتے ہیں۔ یہاں خرابی DA deployment کو براہ راست متاثر کرتی ہے۔
- **Observability boundary:** Metrics/log collection کا dashboards/alerts tooling
  کو export ہونا۔ Tampering outages یا attacks چھپا سکتا ہے۔

## Threat Scenarios and Controls

### Ingest Path Attacks

**Scenario:** Malicious client malformed Norito payloads یا oversized blobs submit
کرتا ہے تاکہ resources exhaust ہوں یا invalid metadata شامل ہو۔

**Controls**
- Norito schema validation with strict version negotiation; unknown flags reject۔
- Torii ingest endpoint پر rate limiting اور authentication۔
- SoraFS chunker کے ذریعے chunk size bounds اور deterministic encoding۔
- Admission pipeline صرف integrity checksum match ہونے کے بعد manifests persist کرے۔
- Deterministic replay cache (`ReplayCache`) `(lane, epoch, sequence)` windows track
  کرتا ہے، high-water marks disk پر persist کرتا ہے، اور duplicates/stale replays
  reject کرتا ہے؛ property اور fuzz harnesses divergent fingerprints اور out-of-order
  submissions cover کرتے ہیں۔ [crates/iroha_core/src/da/replay_cache.rs:1]
  [fuzz/da_replay_cache.rs:1] [crates/iroha_torii/src/da/ingest.rs:1]

**Residual gaps**
- Torii ingest کو replay cache admission میں thread کرنا اور sequence cursors
  restarts کے پار persist کرنا ضروری ہے۔
- Norito DA schemas کے لئے dedicated fuzz harness (`fuzz/da_ingest_schema.rs`) موجود
  ہے؛ coverage dashboards کو regression پر alert کرنا چاہئے۔

### Replication Withholding

**Scenario:** Byzantine storage operators pin assignments قبول کرتے ہیں مگر chunks





drop کرتے ہیں، forged responses یا collusion سے PDP/PoTR challenges pass کرتے ہیں۔

**Controls**
- PDP/PoTR challenge schedule DA payloads تک extend ہے اور per-epoch coverage دیتا ہے۔
- Multi-source replication with quorum thresholds؛ fetch orchestrator missing shards
  detect کر کے repair trigger کرتا ہے۔
- Governance slashing failed proofs اور missing replicas سے linked ہے۔
- Automated reconciliation job (`cargo xtask da-commitment-reconcile`) ingest receipts
  کو DA commitments (SignedBlockWire/`.norito`/JSON) سے compare کرتا ہے، governance
  کیلئے JSON evidence bundle emit کرتا ہے، اور missing/mismatched tickets پر fail
  ہو کر Alertmanager کو page کرنے دیتا ہے۔

**Residual gaps**
- `integration_tests/src/da/pdp_potr.rs` کا simulation harness (tests:
  `integration_tests/tests/da/pdp_potr_simulation.rs`) collusion اور partition
  scenarios چلاتا ہے؛ DA-5 کے ساتھ اسے نئی proof surfaces کیلئے مزید بڑھائیں۔
- Cold-tier eviction policy کیلئے signed audit trail درکار ہے تاکہ covert drops
  روکے جا سکیں۔

### Commitment Tampering

**Scenario:** Compromised sequencer DA commitments omit یا alter کرتا ہے، جس سے
fetch failures یا light-client inconsistencies پیدا ہوتی ہیں۔

**Controls**
- Consensus block proposals کو DA submission queues سے cross-check کرتا ہے؛ peers
  missing commitments والی proposals reject کرتے ہیں۔
- Light clients inclusion proofs verify کرتے ہیں قبل از fetch handles۔
- Submission receipts اور block commitments کا audit trail۔
- Automated reconciliation job (`cargo xtask da-commitment-reconcile`) ingest receipts
  کو commitments سے compare کرتا ہے، governance کیلئے JSON evidence bundle emit کرتا ہے،
  اور missing/mismatched tickets پر Alertmanager page ہوتا ہے۔

**Residual gaps**
- Reconciliation job + Alertmanager hook سے cover؛ governance packets default میں
  JSON evidence bundle ingest کرتے ہیں۔

### Network Partition and Censorship

**Scenario:** Adversary replication network partition کرتا ہے، جس سے nodes assigned
chunks حاصل نہیں کر پاتے یا PDP/PoTR challenges کا جواب نہیں دے پاتے۔

**Controls**
- Multi-region provider requirements diverse network paths یقینی بناتے ہیں۔
- Challenge windows میں jitter اور out-of-band repair channel fallback شامل ہے۔
- Observability dashboards replication depth، challenge success، fetch latency کو
  alert thresholds کے ساتھ monitor کرتے ہیں۔

**Residual gaps**
- Taikai live events کیلئے partition simulations ابھی نہیں؛ soak tests ضروری ہیں۔
- Repair bandwidth reservation policy ابھی codified نہیں۔

### Insider Abuse

**Scenario:** Registry access والا operator retention policies manipulate کرتا ہے،
malicious providers کو whitelist کرتا ہے، یا alerts suppress کرتا ہے۔

**Controls**
- Governance actions multi-party signatures اور Norito-notarised records مانگتی ہیں۔
- Policy changes monitoring اور archival logs کو events بھیجتی ہیں۔
- Observability pipeline append-only Norito logs with hash chaining enforce کرتا ہے۔
- Quarterly access review automation (`cargo xtask da-privilege-audit`) manifest/replay
  dirs (اور operator-supplied paths) scan کرتا ہے، missing/non-directory/world-writable
  entries flag کرتا ہے، اور signed JSON bundle dashboards کیلئے emit کرتا ہے۔

**Residual gaps**
- Dashboard tamper-evidence کیلئے signed snapshots درکار ہیں۔

## Residual Risk Register

| Risk | Likelihood | Impact | Owner | Mitigation Plan |
| --- | --- | --- | --- | --- |
| DA-2 sequence cache سے پہلے DA manifests replay | Possible | Moderate | Core Protocol WG | DA-2 میں sequence cache + nonce validation implement کریں؛ regression tests شامل کریں۔ |
| >f nodes compromise پر PDP/PoTR collusion | Unlikely | High | Storage Team | Cross-provider sampling کے ساتھ نیا challenge schedule derive کریں؛ simulation harness سے validate کریں۔ |
| Cold-tier eviction audit gap | Possible | High | SRE / Storage Team | Evictions کیلئے signed audit logs + on-chain receipts attach کریں؛ dashboards سے monitor کریں۔ |
| Sequencer omission detection latency | Possible | High | Core Protocol WG | Nightly `cargo xtask da-commitment-reconcile` receipts vs commitments (SignedBlockWire/`.norito`/JSON) compare کر کے governance کو page کرے۔ |
| Taikai live streams کیلئے partition resilience | Possible | Critical | Networking TL | Partition drills چلائیں؛ repair bandwidth reserve کریں؛ failover SOP document کریں۔ |
| Governance privilege drift | Unlikely | High | Governance Council | Quarterly `cargo xtask da-privilege-audit` (manifest/replay dirs + extra paths) with signed JSON + dashboard gate; audit artefacts کو on-chain anchor کریں۔ |

## Required Follow-Ups

1. DA ingest Norito schemas اور example vectors publish کریں (DA-2 میں لے جائیں)۔
2. Replay cache کو Torii DA ingest میں thread کریں اور sequence cursors restarts
   کے پار persist کریں۔
3. **Completed (2026-02-05):** PDP/PoTR simulation harness اب collusion + partition
   scenarios اور QoS backlog modelling exercises کرتا ہے؛ دیکھیں
   `integration_tests/src/da/pdp_potr.rs` (tests: `integration_tests/tests/da/pdp_potr_simulation.rs`)۔
4. **Completed (2026-05-29):** `cargo xtask da-commitment-reconcile` ingest receipts
   کو DA commitments (SignedBlockWire/`.norito`/JSON) سے compare کر کے
   `artifacts/da/commitment_reconciliation.json` emit کرتا ہے اور Alertmanager/
   governance packets کیلئے wired ہے (`xtask/src/da.rs`)۔
5. **Completed (2026-05-29):** `cargo xtask da-privilege-audit` manifest/replay spool
   (اور operator-supplied paths) walk کرتا ہے، missing/non-directory/world-writable
   entries flag کرتا ہے، اور signed JSON bundle governance dashboards کیلئے بناتا
   ہے (`artifacts/da/privilege_audit.json`)۔

**Where to look next:**

- DA-2 میں replay cache اور cursor persistence آچکی ہے۔ Implementation دیکھیں
  `crates/iroha_core/src/da/replay_cache.rs` (cache logic) اور Torii integration
  `crates/iroha_torii/src/da/ingest.rs` میں، جو `/v1/da/ingest` کے ذریعے fingerprint
  checks thread کرتا ہے۔
- PDP/PoTR streaming simulations proof-stream harness کے ذریعے چلتی ہیں:
  `crates/sorafs_car/tests/sorafs_cli.rs`۔ یہ PoR/PDP/PoTR request flows اور
  failure scenarios cover کرتا ہے جو threat model میں بیان ہیں۔
- Capacity اور repair soak کے نتائج `docs/source/sorafs/reports/sf2c_capacity_soak.md`
  میں ہیں، جبکہ Sumeragi soak matrix `docs/source/sumeragi_soak_matrix.md` میں ہے
  (localized variants شامل ہیں)۔ یہ artefacts residual risk register کے drills
  کو capture کرتے ہیں۔
- Reconciliation + privilege-audit automation `docs/automation/da/README.md` اور
  `cargo xtask da-commitment-reconcile` / `cargo xtask da-privilege-audit` میں ہے؛
  governance packets کیلئے evidence attach کرتے وقت `artifacts/da/` کی default
  outputs استعمال کریں۔

## Simulation Evidence & QoS Modelling (2026-02)

DA-1 follow-up #3 مکمل کرنے کیلئے ہم نے `integration_tests/src/da/pdp_potr.rs`
( `integration_tests/tests/da/pdp_potr_simulation.rs` سے covered ) میں


deterministic PDP/PoTR simulation harness شامل کیا۔ یہ harness nodes کو تین regions
میں allocate کرتا ہے، roadmap probabilities کے مطابق partitions/collusion inject
کرتا ہے، PoTR lateness track کرتا ہے، اور repair-backlog model کو feed کرتا ہے جو
hot-tier repair budget کی عکاسی کرتا ہے۔ Default scenario (12 epochs, 18 PDP
challenges + 2 PoTR windows per epoch) سے یہ metrics نکلے:

<!-- BEGIN_DA_SIM_TABLE -->
<!-- AUTO-GENERATED by scripts/docs/render_da_threat_model_tables.py; do not edit manually. -->
| Metric | Value | Notes |
| --- | --- | --- |
| PDP failures detected | 48 / 49 (98.0%) | Partitions اب بھی detection trigger کرتے ہیں؛ ایک undetected failure honest jitter سے ہے۔ |
| PDP mean detection latency | 0.0 epochs | Failures originating epoch کے اندر ظاہر ہوتے ہیں۔ |
| PoTR failures detected | 28 / 77 (36.4%) | Detection تب trigger ہوتی ہے جب node >=2 PoTR windows miss کرے، زیادہ تر واقعات residual-risk register میں رہتے ہیں۔ |
| PoTR mean detection latency | 2.0 epochs | Archival escalation میں شامل دو-epoch lateness threshold سے match کرتا ہے۔ |
| Repair queue peak | 38 manifests | Backlog تب بڑھتا ہے جب partitions چار repairs/epoch سے تیز جمع ہوں۔ |
| Response latency p95 | 30,068 ms | 30 s challenge window اور QoS sampling کیلئے +/-75 ms jitter کو reflect کرتا ہے۔ |
<!-- END_DA_SIM_TABLE -->

یہ outputs اب DA dashboard prototypes کو drive کرتے ہیں اور roadmap میں حوالہ
دئے گئے "simulation harness + QoS modelling" acceptance criteria پورے کرتے ہیں۔

Automation اب `cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`
کے پیچھے ہے، جو shared harness کو call کرتا ہے اور default طور پر Norito JSON
`artifacts/da/threat_model_report.json` میں emit کرتا ہے۔ Nightly jobs یہ file
consume کر کے document matrices refresh کرتی ہیں اور detection rates، repair
queues، یا QoS samples میں drift پر alert دیتی ہیں۔

Docs کیلئے اوپر کی table refresh کرنے کو `make docs-da-threat-model` چلائیں، جو
`cargo xtask da-threat-model-report` invoke کرتا ہے،
`docs/source/da/_generated/threat_model_report.json` regenerate کرتا ہے، اور
`scripts/docs/render_da_threat_model_tables.py` کے ذریعے یہ section rewrite کرتا
ہے۔ `docs/portal` mirror (`docs/portal/docs/da/threat-model.md`) بھی اسی pass میں
update ہوتا ہے تاکہ دونوں copies sync رہیں۔

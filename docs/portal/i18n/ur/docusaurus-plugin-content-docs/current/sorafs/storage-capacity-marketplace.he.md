---
lang: he
direction: rtl
source: docs/portal/i18n/ur/docusaurus-plugin-content-docs/current/sorafs/storage-capacity-marketplace.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c70d5aa1325d1c75f7df790a6f6101fd7780c73e016d6fe4c4773fc941739499
source_last_modified: "2026-01-03T18:08:00+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Urdu (ur) translation. Replace this content with the full translation. -->

---
id: storage-capacity-marketplace
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/storage-capacity-marketplace.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note مستند ماخذ
یہ صفحہ `docs/source/sorafs/storage_capacity_marketplace.md` کی عکاسی کرتا ہے۔ جب تک پرانا ڈاکیومنٹیشن فعال ہے دونوں لوکیشنز کو ہم آہنگ رکھیں۔
:::

# SoraFS اسٹوریج کیپیسٹی مارکیٹ پلیس (SF-2c ڈرافٹ)

روڈ میپ آئٹم SF-2c ایک گورنڈ مارکیٹ پلیس متعارف کراتا ہے جہاں اسٹوریج providers committed capacity ڈیکلئر کرتے ہیں، replication orders وصول کرتے ہیں، اور فراہم کردہ availability کے متناسب fees کماتے ہیں۔ یہ دستاویز پہلی ریلیز کے لیے درکار deliverables کا دائرہ کار طے کرتی ہے اور انہیں actionable tracks میں تقسیم کرتی ہے۔

## مقاصد

- providers کی capacity commitments (کل bytes، ہر lane کے limits، expiry) کو ایک قابلِ تصدیق شکل میں بیان کرنا جو governance، SoraNet transport اور Torii استعمال کر سکیں۔
- declared capacity، stake اور policy constraints کے مطابق pins کو providers میں تقسیم کرنا جبکہ deterministic behavior برقرار رکھنا۔
- storage delivery (replication success، uptime، integrity proofs) کو ناپنا اور fee distribution کے لیے telemetry export کرنا۔
- revocation اور dispute کے عمل فراہم کرنا تاکہ dishonest providers کو penalise یا remove کیا جا سکے۔

## ڈومین کانسیپٹس

| Concept | Description | Initial Deliverable |
|---------|-------------|---------------------|
| `CapacityDeclarationV1` | Norito payload جو provider ID، chunker profile support، committed GiB، lane-specific limits، pricing hints، staking commitment اور expiry بیان کرتا ہے۔ | `sorafs_manifest::capacity` میں schema + validator۔ |
| `ReplicationOrder` | governance کی جانب سے جاری instruction جو manifest CID کو ایک یا زائد providers کو assign کرتی ہے، جس میں redundancy level اور SLA metrics شامل ہیں۔ | Torii کے ساتھ مشترک Norito schema + smart contract API۔ |
| `CapacityLedger` | on-chain/off-chain registry جو active capacity declarations، replication orders، performance metrics اور fee accrual کو ٹریک کرتا ہے۔ | smart contract module یا off-chain service stub مع deterministic snapshot۔ |
| `MarketplacePolicy` | governance policy جو minimum stake، audit requirements اور penalty curves متعین کرتی ہے۔ | `sorafs_manifest` میں config struct + governance document۔ |

### Implemented Schemas (Status)

## ورک بریک ڈاؤن

### 1. Schema & Registry Layer

| Task | Owner(s) | Notes |
|------|----------|-------|
| `CapacityDeclarationV1`, `ReplicationOrderV1`, `CapacityTelemetryV1` کی تعریف۔ | Storage Team / Governance | Norito استعمال کریں؛ semantic versioning اور capability references شامل کریں۔ |
| `sorafs_manifest` میں parser + validator modules لاگو کریں۔ | Storage Team | monotonic IDs، capacity bounds، stake requirements نافذ کریں۔ |
| chunker registry metadata میں ہر profile کے لیے `min_capacity_gib` شامل کریں۔ | Tooling WG | clients کو profile کے حساب سے minimum hardware requirements نافذ کرنے میں مدد۔ |
| `MarketplacePolicy` ڈاکیومنٹ ڈرافٹ کریں جو admission guardrails اور penalty schedule بیان کرے۔ | Governance Council | docs میں publish کریں اور policy defaults کے ساتھ رکھیں۔ |

#### Schema Definitions (Implemented)

- `CapacityDeclarationV1` ہر provider کے لیے signed capacity commitments کو capture کرتا ہے، جن میں canonical chunker handles، capability references، optional lane caps، pricing hints، validity windows اور metadata شامل ہیں۔ validation یہ یقینی بناتا ہے کہ stake غیر صفر ہو، handles canonical ہوں، aliases deduplicated ہوں، lane caps اعلان کردہ total کے اندر ہوں اور GiB accounting monotonic ہو۔【crates/sorafs_manifest/src/capacity.rs:28】
- `ReplicationOrderV1` manifests کو governance-issued assignments سے باندھتا ہے، جن میں redundancy targets، SLA thresholds اور per-assignment guarantees شامل ہیں؛ validators canonical chunker handles، unique providers اور deadline constraints نافذ کرتے ہیں اس سے پہلے کہ Torii یا registry order ingest کریں۔【crates/sorafs_manifest/src/capacity.rs:301】
- `CapacityTelemetryV1` epoch snapshots (declared vs utilised GiB، replication counters، uptime/PoR percentages) ظاہر کرتا ہے جو fee distribution کو feed کرتے ہیں۔ bounds checks استعمال کو declarations کے اندر اور percentages کو 0-100% میں رکھتے ہیں۔【crates/sorafs_manifest/src/capacity.rs:476】
- Shared helpers (`CapacityMetadataEntry`, `PricingScheduleV1`, lane/assignment/SLA validators) deterministic key validation اور error reporting فراہم کرتے ہیں جنہیں CI اور downstream tooling reuse کر سکتے ہیں۔【crates/sorafs_manifest/src/capacity.rs:230】
- `PinProviderRegistry` اب on-chain snapshot کو `/v1/sorafs/capacity/state` کے ذریعے expose کرتا ہے، provider declarations اور fee ledger entries کو deterministic Norito JSON کے پیچھے جوڑ کر۔【crates/iroha_torii/src/sorafs/registry.rs:17】【crates/iroha_torii/src/sorafs/api.rs:64】
- Validation coverage canonical handle enforcement، duplicate detection، per-lane bounds، replication assignment guards اور telemetry range checks کو exercise کرتی ہے تاکہ regressions فوراً CI میں ظاہر ہوں۔【crates/sorafs_manifest/src/capacity.rs:792】
- Operator tooling: `sorafs_manifest_stub capacity {declaration, telemetry, replication-order}` human-readable specs کو canonical Norito payloads، base64 blobs اور JSON summaries میں تبدیل کرتا ہے تاکہ operators `/v1/sorafs/capacity/declare`, `/v1/sorafs/capacity/telemetry` اور replication order fixtures کو local validation کے ساتھ stage کر سکیں۔【crates/sorafs_car/src/bin/sorafs_manifest_stub/capacity.rs:1】 Reference fixtures `fixtures/sorafs_manifest/replication_order/` (`order_v1.json`, `order_v1.to`) میں ہیں اور `cargo run -p sorafs_car --bin sorafs_manifest_stub -- capacity replication-order` سے generate ہوتی ہیں۔

### 2. Control Plane Integration

| Task | Owner(s) | Notes |
|------|----------|-------|
| `/v1/sorafs/capacity/declare`, `/v1/sorafs/capacity/telemetry`, `/v1/sorafs/capacity/orders` Torii handlers کو Norito JSON payloads کے ساتھ شامل کریں۔ | Torii Team | validator logic کو mirror کریں؛ Norito JSON helpers reuse کریں۔ |
| `CapacityDeclarationV1` snapshots کو orchestrator scoreboard metadata اور gateway fetch plans میں propagate کریں۔ | Tooling WG / Orchestrator team | `provider_metadata` میں capacity references شامل کریں تاکہ multi-source scoring lane limits کا خیال رکھے۔ |
| replication orders کو orchestrator/gateway clients میں feed کریں تاکہ assignments اور failover hints ڈرائیو ہوں۔ | Networking TL / Gateway team | Scoreboard builder governance-signed replication orders استعمال کرتا ہے۔ |
| CLI tooling: `sorafs_cli` میں `capacity declare`, `capacity telemetry`, `capacity orders import` شامل کریں۔ | Tooling WG | deterministic JSON + scoreboard outputs فراہم کریں۔ |

### 3. Marketplace Policy & Governance

| Task | Owner(s) | Notes |
|------|----------|-------|
| `MarketplacePolicy` (minimum stake، penalty multipliers، audit cadence) کی توثیق۔ | Governance Council | docs میں publish کریں، revision history capture کریں۔ |
| governance hooks شامل کریں تاکہ Parliament declarations کو approve، renew اور revoke کر سکے۔ | Governance Council / Smart Contract team | Norito events + manifest ingestion استعمال کریں۔ |
| penalty schedule (fee reduction، bond slashing) نافذ کریں جو telemetered SLA violations سے منسلک ہو۔ | Governance Council / Treasury | `DealEngine` settlement outputs کے ساتھ align کریں۔ |
| dispute process اور escalation matrix کی دستاویز بنائیں۔ | Docs / Governance | dispute runbook + CLI helpers لنک کریں۔ |

### 4. Metering & Fee Distribution

| Task | Owner(s) | Notes |
|------|----------|-------|
| Torii metering ingest کو `CapacityTelemetryV1` قبول کرنے کے لیے بڑھائیں۔ | Torii Team | GiB-hour، PoR success، uptime validate کریں۔ |
| `sorafs_node` metering pipeline کو per-order utilisation + SLA stats رپورٹ کرنے کے لیے اپڈیٹ کریں۔ | Storage Team | replication orders اور chunker handles کے ساتھ align کریں۔ |
| Settlement pipeline: telemetry + replication data کو XOR-denominated payouts میں تبدیل کریں، governance-ready summaries بنائیں، اور ledger state ریکارڈ کریں۔ | Treasury / Storage Team | Deal Engine / Treasury exports میں wire کریں۔ |
| metering health (ingestion backlog، stale telemetry) کے لیے dashboards/alerts export کریں۔ | Observability | SF-6/SF-7 میں ریفرنس کیے گئے Grafana pack کو extend کریں۔ |

- Torii اب `/v1/sorafs/capacity/telemetry` اور `/v1/sorafs/capacity/state` (JSON + Norito) expose کرتا ہے تاکہ operators epoch telemetry snapshots جمع کر سکیں اور inspectors auditing یا evidence packaging کے لیے canonical ledger حاصل کر سکیں۔【crates/iroha_torii/src/sorafs/api.rs:268】【crates/iroha_torii/src/sorafs/api.rs:816】
- `PinProviderRegistry` integration یقینی بناتی ہے کہ replication orders اسی endpoint کے ذریعے قابلِ رسائی ہوں؛ CLI helpers (`sorafs_cli capacity telemetry --from-file telemetry.json`) اب deterministic hashing اور alias resolution کے ساتھ automation runs سے telemetry validate/publish کرتے ہیں۔
- Metering snapshots `CapacityTelemetrySnapshot` entries پیدا کرتے ہیں جو `metering` snapshot سے pinned ہوتے ہیں، اور Prometheus exports `docs/source/grafana_sorafs_metering.json` کے ready-to-import Grafana board کو feed کرتے ہیں تاکہ billing teams GiB-hour accrual، projected nano-SORA fees اور SLA compliance کو real time میں مانیٹر کر سکیں۔【crates/iroha_torii/src/routing.rs:5143】【docs/source/grafana_sorafs_metering.json:1】
- جب metering smoothing enabled ہو تو snapshot میں `smoothed_gib_hours` اور `smoothed_por_success_bps` شامل ہوتے ہیں تاکہ operators EMA-trended values کو raw counters کے مقابلے میں دیکھ سکیں جنہیں governance payouts کے لیے استعمال کرتی ہے۔【crates/sorafs_node/src/metering.rs:401】

### 5. Dispute & Revocation Handling

| Task | Owner(s) | Notes |
|------|----------|-------|
| `CapacityDisputeV1` payload (complainant، evidence، target provider) کی تعریف۔ | Governance Council | Norito schema + validator۔ |
| disputes فائل کرنے اور جواب دینے کے لیے CLI support (evidence attachments کے ساتھ)۔ | Tooling WG | evidence bundle کی deterministic hashing یقینی بنائیں۔ |
| repeated SLA breaches کے لیے automated checks شامل کریں (auto-escalate to dispute)۔ | Observability | alert thresholds اور governance hooks۔ |
| revocation playbook (grace period، pinned data کی evacuation) دستاویزی بنائیں۔ | Docs / Storage Team | policy doc اور operator runbook سے لنک کریں۔ |

## Testing & CI Requirements

- تمام نئے schema validators کے لیے unit tests (`sorafs_manifest`).
- integration tests جو simulate کریں: declaration → replication order → metering → payout۔
- CI workflow جو sample capacity declarations/telemetry regenerate کرے اور signatures sync رکھے ( `ci/check_sorafs_fixtures.sh` کو extend کریں)۔
- registry API کے لیے load tests (10k providers، 100k orders simulate کریں)۔

## Telemetry & Dashboards

- Dashboard panels:
  - provider کے حساب سے declared بمقابلہ utilised capacity۔
  - replication order backlog اور average assignment delay۔
  - SLA compliance (uptime %، PoR success rate)۔
  - فی epoch fee accrual اور penalties۔
- Alerts:
  - provider کم از کم committed capacity سے نیچے۔
  - replication order SLA سے زیادہ stuck۔
  - metering pipeline failures۔

## Documentation Deliverables

- capacity declare کرنے، commitments renew کرنے، اور utilisation مانیٹر کرنے کے لیے operator guide۔
- declarations approve کرنے، orders جاری کرنے، disputes handle کرنے کے لیے governance guide۔
- capacity endpoints اور replication order format کے لیے API reference۔
- developers کے لیے marketplace FAQ۔

## GA Readiness Checklist

روڈ میپ آئٹم **SF-2c** accounting، dispute handling اور onboarding میں ٹھوس شواہد کی بنیاد پر production rollout کو گیٹ کرتا ہے۔ نیچے دیے گئے artefacts استعمال کریں تاکہ acceptance criteria implementation کے ساتھ sync رہے۔

### Nightly accounting & XOR reconciliation
- capacity state snapshot اور XOR ledger export ایک ہی window کے لیے نکالیں، پھر چلائیں:
  ```bash
  python3 scripts/telemetry/capacity_reconcile.py \
    --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
    --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
    --label nightly-capacity \
    --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
    --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
  ```
  یہ helper missing/overpaid settlements یا penalties پر non-zero exit کرتا ہے اور Prometheus textfile summary emit کرتا ہے۔
- `SoraFSCapacityReconciliationMismatch` alert (`dashboards/alerts/sorafs_capacity_rules.yml` میں)
  reconciliation metrics کے gaps رپورٹ کرنے پر fire ہوتا ہے؛ dashboards `dashboards/grafana/sorafs_capacity_penalties.json` میں ہیں۔
- JSON summary اور hashes کو `docs/examples/sorafs_capacity_marketplace_validation/` کے تحت archive کریں
  governance packets کے ساتھ۔

### Dispute & slashing evidence
- disputes `sorafs_manifest_stub capacity dispute` کے ذریعے فائل کریں (tests:
  `cargo test -p sorafs_car --test capacity_cli`) تاکہ payloads canonical رہیں۔
- `cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic` اور penalty suites
  (`record_capacity_telemetry_penalises_persistent_under_delivery`) چلائیں تاکہ disputes اور slashes deterministic replay ثابت ہوں۔
- evidence capture اور escalation کے لیے `docs/source/sorafs/dispute_revocation_runbook.md` فالو کریں؛ strike approvals کو validation report میں واپس لنک کریں۔

### Provider onboarding & exit smoke tests
- declaration/telemetry artefacts کو `sorafs_manifest_stub capacity ...` سے regenerate کریں اور submission سے پہلے CLI tests replay کریں (`cargo test -p sorafs_car --test capacity_cli -- capacity_declaration`)۔
- Torii (`/v1/sorafs/capacity/declare`) کے ذریعے submit کریں پھر `/v1/sorafs/capacity/state` اور Grafana screenshots capture کریں۔ `docs/source/sorafs/capacity_onboarding_runbook.md` میں exit flow فالو کریں۔
- signed artefacts اور reconciliation outputs کو `docs/examples/sorafs_capacity_marketplace_validation/` میں archive کریں۔

## Dependencies & Sequencing

1. SF-2b (admission policy) مکمل کریں - marketplace vetted providers پر انحصار کرتا ہے۔
2. Torii integration سے پہلے schema + registry layer (یہ ڈاک) مکمل کریں۔
3. payouts enable کرنے سے پہلے metering pipeline مکمل کریں۔
4. آخری قدم: staging میں metering data verify ہونے کے بعد governance-controlled fee distribution enable کریں۔

progress کو roadmap میں اس دستاویز کے حوالے کے ساتھ ٹریک کریں۔ جب ہر بڑا سیکشن (schema, control plane, integration, metering, dispute handling) feature complete ہو جائے تو roadmap اپڈیٹ کریں۔

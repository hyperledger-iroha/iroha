---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/pin-registry-plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: pin-registry-plan
title: SoraFS Pin Registry نفاذی منصوبہ
sidebar_label: Pin Registry منصوبہ
description: SF-4 نفاذی منصوبہ جو registry کی state machine، Torii facade، tooling اور observability کو کور کرتا ہے۔
---

:::note مستند ماخذ
یہ صفحہ `docs/source/sorafs/pin_registry_plan.md` کی عکاسی کرتا ہے۔ جب تک پرانی دستاویزات فعال ہیں دونوں نقول ہم آہنگ رکھیں۔
:::

# SoraFS Pin Registry نفاذی منصوبہ (SF-4)

SF-4 Pin Registry کنٹریکٹ اور معاون سروسز فراہم کرتا ہے جو manifest commitments محفوظ کرتی ہیں،
pin policies نافذ کرتی ہیں، اور Torii، gateways اور orchestrators کے لیے APIs ظاہر کرتی ہیں۔
یہ دستاویز validation plan کو ٹھوس implementation tasks سے بڑھاتی ہے، جس میں on-chain logic،
host-side services، fixtures، اور عملیاتی تقاضے شامل ہیں۔

## دائرہ کار

1. **registry state machine**: Norito-defined records برائے manifests، aliases، successor chains،
   retention epochs، اور governance metadata.
2. **کنٹریکٹ نفاذ**: pin lifecycle کے لیے deterministic CRUD operations (`ReplicationOrder`,
   `Precommit`, `Completion`, eviction).
3. **سروس facade**: gRPC/REST endpoints جو registry سے بیکڈ ہوں اور Torii و SDKs انہیں استعمال کریں،
   جن میں pagination اور attestation شامل ہے۔
4. **tooling اور fixtures**: CLI helpers، test vectors، اور documentation تاکہ manifests، aliases اور
   governance envelopes ہم آہنگ رہیں۔
5. **telemetry اور ops**: registry صحت کے لیے metrics، alerts، اور runbooks.

## ڈیٹا ماڈل

### بنیادی ریکارڈز (Norito)

| Struct | وضاحت | فیلڈز |
|--------|-------|-------|
| `PinRecordV1` | canonical manifest entry. | `manifest_cid`, `chunk_plan_digest`, `por_root`, `profile_handle`, `approved_at`, `retention_epoch`, `pin_policy`, `successor_of`, `governance_envelope_hash`. |
| `AliasBindingV1` | alias -> manifest CID mapping. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | providers کو manifest pin کرنے کی ہدایت. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | provider acknowledgement. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | governance policy snapshot. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

Implementation reference: `crates/sorafs_manifest/src/pin_registry.rs` دیکھیں جہاں Rust Norito schemas
اور validation helpers موجود ہیں۔ Validation manifest tooling (chunker registry lookup، pin policy gating)
کو آئینہ کرتی ہے تاکہ کنٹریکٹ، Torii facades، اور CLI ایک جیسے invariants شیئر کریں۔

Tasks:
- `crates/sorafs_manifest/src/pin_registry.rs` میں Norito schemas مکمل کریں۔
- Norito macros سے code generate کریں (Rust + دیگر SDKs)۔
- schemas آ جانے کے بعد docs (`sorafs_architecture_rfc.md`) اپڈیٹ کریں۔

## کنٹریکٹ نفاذ

| کام | مالک/مالکان | نوٹس |
|-----|-------------|------|
| registry storage (sled/sqlite/off-chain) یا smart contract module نافذ کریں۔ | Core Infra / Smart Contract Team | deterministic hashing فراہم کریں، floating point سے بچیں۔ |
| Entry points: `submit_manifest`, `approve_manifest`, `bind_alias`, `issue_replication_order`, `complete_replication`, `evict_manifest`. | Core Infra | validation plan سے `ManifestValidator` استعمال کریں۔ alias binding اب `RegisterPinManifest` (Torii DTO) سے گزرتی ہے جبکہ مخصوص `bind_alias` آئندہ اپڈیٹس کے لیے منصوبہ بند ہے۔ |
| State transitions: succession (manifest A -> B)، retention epochs، اور alias uniqueness نافذ کریں۔ | Governance Council / Core Infra | alias uniqueness، retention limits، اور predecessor approval/retirement checks اب `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` میں ہیں؛ multi-hop succession detection اور replication bookkeeping ابھی باقی ہیں۔ |
| Governed parameters: `ManifestPolicyV1` کو config/governance state سے لوڈ کریں؛ governance events کے ذریعے اپڈیٹس کی اجازت دیں۔ | Governance Council | policy updates کے لیے CLI فراہم کریں۔ |
| Event emission: telemetry کے لیے Norito events (`ManifestApproved`, `ReplicationOrderIssued`, `AliasBound`) جاری کریں۔ | Observability | event schema + logging متعین کریں۔ |

Testing:
- ہر entry point کے لیے unit tests (positive + rejection).
- succession chain کے لیے property tests (no cycles، monotonic epochs).
- random manifests (bounded) سے fuzz validation.

## سروس facade (Torii/SDK انضمام)

| جزو | کام | مالک/مالکان |
|------|-----|-------------|
| Torii Service | `/v1/sorafs/pin` (submit)، `/v1/sorafs/pin/{cid}` (lookup)، `/v1/sorafs/aliases` (list/bind)، `/v1/sorafs/replication` (orders/receipts) فراہم کریں۔ pagination + filtering مہیا کریں۔ | Networking TL / Core Infra |
| Attestation | responses میں registry height/hash شامل کریں؛ Norito attestation struct شامل کریں جسے SDKs consume کریں۔ | Core Infra |
| CLI | `sorafs_manifest_stub` میں توسیع یا نئی `sorafs_pin` CLI بنائیں جس میں `pin submit`, `alias bind`, `order issue`, `registry export` ہو۔ | Tooling WG |
| SDK | Norito schema سے client bindings (Rust/Go/TS) generate کریں؛ integration tests شامل کریں۔ | SDK Teams |

Operations:
- GET endpoints کے لیے cache/ETag layer شامل کریں۔
- Torii policies کے مطابق rate limiting / auth فراہم کریں۔

## Fixtures اور CI

- Fixtures directory: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` میں signed manifest/alias/order snapshots محفوظ ہوتے ہیں جو `cargo run -p iroha_core --example gen_pin_snapshot` سے regenerate ہوتے ہیں۔
- CI step: `ci/check_sorafs_fixtures.sh` snapshot regenerate کرتا ہے اور diff ہونے پر fail کرتا ہے تاکہ CI fixtures aligned رہیں۔
- Integration tests (`crates/iroha_core/tests/pin_registry.rs`) happy path کے ساتھ duplicate-alias rejection، alias approval/retention guards، mismatched chunker handles، replica-count validation، اور succession-guard failures (unknown/pre-approved/retired/self pointers) کور کرتے ہیں؛ تفصیل کے لیے `register_manifest_rejects_*` cases دیکھیں۔
- Unit tests اب `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` میں alias validation، retention guards، اور successor checks کور کرتے ہیں؛ multi-hop succession detection تب آئے گا جب state machine دستیاب ہوگی۔
- Observability pipelines کے لیے golden JSON events۔

## Telemetry اور Observability

Metrics (Prometheus):
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- موجودہ provider telemetry (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) end-to-end dashboards کے لیے scope میں رہے گی۔

Logs:
- governance audits کے لیے structured Norito event stream (signed?).

Alerts:
- SLA سے زیادہ pending replication orders.
- alias expiry threshold سے کم.
- retention violations (manifest renewal وقت سے پہلے نہ ہو).

Dashboards:
- Grafana JSON `docs/source/grafana_sorafs_pin_registry.json` manifest lifecycle totals، alias coverage، backlog saturation، SLA ratio، latency vs slack overlays، اور missed-order rates کو on-call review کے لیے دکھاتا ہے۔

## Runbooks اور Documentation

- `docs/source/sorafs/migration_ledger.md` کو registry status updates شامل کرنے کے لیے اپڈیٹ کریں۔
- Operator guide: `docs/source/sorafs/runbooks/pin_registry_ops.md` (اب شائع شدہ) metrics، alerting، deployment، backup، اور recovery flows کور کرتا ہے۔
- Governance guide: policy parameters، approval workflow، dispute handling بیان کریں۔
- ہر endpoint کے لیے API reference pages (Docusaurus docs).

## Dependencies اور Sequencing

1. validation plan tasks مکمل کریں (ManifestValidator integration).
2. Norito schema + policy defaults کو حتمی بنائیں۔
3. contract + service نافذ کریں اور telemetry wire کریں۔
4. fixtures regenerate کریں اور integration suites چلائیں۔
5. docs/runbooks اپڈیٹ کریں اور roadmap items کو مکمل مارک کریں۔

SF-4 کے ہر checklist آئٹم میں پیش رفت پر اس منصوبے کا حوالہ ہونا چاہیے۔
REST facade اب attested listing endpoints کے ساتھ آتی ہے:

- `GET /v1/sorafs/pin` اور `GET /v1/sorafs/pin/{digest}` manifests واپس کرتے ہیں جن میں
  alias bindings، replication orders، اور تازہ ترین block hash سے ماخوذ attestation object شامل ہے۔
- `GET /v1/sorafs/aliases` اور `GET /v1/sorafs/replication` فعال alias catalogue اور
  replication order backlog کو consistent pagination اور status filters کے ساتھ ظاہر کرتے ہیں۔

CLI ان calls کو wrap کرتی ہے (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) تاکہ operators کم سطحی APIs کو چھوئے بغیر registry audits خودکار بنا سکیں۔

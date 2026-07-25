---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/pin-registry-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: پن رجسٹری پلان
عنوان: نفاذ کی منصوبہ بندی پن رجسٹری SoraFS
سائڈبار_لیبل: پن رجسٹری پلان
تفصیل: رجسٹری اسٹیٹ مشین ، Torii facade ، ٹولنگ اور مشاہدہ کرنے کا احاطہ SF-4 عمل درآمد کا منصوبہ۔
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/sorafs/pin_registry_plan.md` کی عکاسی کرتا ہے۔ جب تک اسٹیٹ کی دستاویزات فعال رہیں تب تک دونوں کاپیاں ہم آہنگ رکھیں۔
:::

# نفاذ پلان پن رجسٹری SoraFS (SF-4)

SF-4 پن رجسٹری کا معاہدہ اور معاون خدمات فراہم کرتا ہے جو اسٹور کرتے ہیں
ذمہ داریوں کو ظاہر کریں ، پن کی پالیسیاں نافذ کریں اور Torii کے لئے ایک API فراہم کریں ،
گیٹ وے اور آرکیسٹریٹرز۔ یہ دستاویز توثیق کے منصوبے کو مخصوص کے ساتھ وسعت دیتی ہے
نفاذ کے کام ، میزبان کی طرف سے چین پر لاجک ، خدمات کا احاطہ کرتے ہیں ،
فکسچر اور آپریشنل ضروریات۔

## ایریا

1. ** اسٹیٹ مشین رجسٹری **: ریکارڈز Norito منشور کے لئے ، عرفیت ،
   جانشینی کی زنجیریں ، اسٹوریج اور مینجمنٹ میٹا ڈیٹا کے دور۔
2
   سائیکل پن (`ReplicationOrder` ، `Precommit` ، `Completion` ، بے دخلی)۔
3
   Torii اور SDK استعمال کیا گیا ، جس میں صفحہ بندی اور سرٹیفیکیشن بھی شامل ہے۔
4. ** ٹولنگ اور فکسچر **: سی ایل آئی مددگار ، ٹیسٹ ویکٹر اور دستاویزات
   ظاہر ، عرفی اور گورننس لفافوں کی ہم آہنگی۔
5.

## ڈیٹا ماڈل

### ماسٹر ریکارڈز (Norito)

| ڈھانچہ | تفصیل | فیلڈز |
| ---------- | ---------- | ------ |
| `PinManifestRecord` | Chain-authoritative manifest lifecycle entry. The envelope digest and exact 36-byte CIDv1/dag-cbor/BLAKE3-256 content root are distinct commitments. | `digest`, `root_cid`, `chunker`, `chunk_digest_sha3_256`, `por_root`, `content_length`, `policy`, `submitted_by`, `submitted_epoch`, `alias`, `successor_of`, `metadata`, `status`, `retirement_reason`, `council_envelope_digest`, `pin_fee_payment`. |
| `PinManifestFinalizedRecordV1` | Immutable read result binding one native manifest record to the finalized block used for the query. | `finalized_cursor` (`height`, `block_hash`), `manifest`. |
| `AliasBindingV1` | عرف سے میچ کرتا ہے -> سی آئی ڈی منشور۔ | `alias` ، `manifest_cid` ، `bound_at` ، `expiry_epoch`۔ |
| `ReplicationOrderV1` | فراہم کرنے والوں کو منشور منسلک کرنے کے لئے ہدایات۔ | `order_id` ، `manifest_cid` ، `providers` ، `redundancy` ، `deadline` ، `policy_hash`۔ |
| `ReplicationReceiptV1` | فراہم کنندہ کی تصدیق. | `order_id` ، `provider_id` ، `status` ، `timestamp` ، `por_sample_digest`۔ |
| `ManifestPolicyV1` | انتظامی پالیسی کا سنیپ شاٹ۔ | `min_replicas` ، `max_retention_epochs` ، `allowed_profiles` ، `pin_fee_basis_points`۔ |

Implementation reference: the authoritative manifest lifecycle and finalized
read schemas live in `crates/iroha_data_model/src/sorafs/pin_registry.rs`.
Supporting alias, replication, and policy envelopes live in
`crates/sorafs_manifest/src/pin_registry.rs`. Consensus admission derives and
validates the stored commitments; Torii and operator tooling consume the exact
native finalized record rather than maintaining a second pin-record format.

Status:
- The native `PinManifestRecord` and `PinManifestFinalizedRecordV1` are the V1
  manifest-registry surface used by core, Torii, fixtures, and reference
  validators.
- Rust code generation uses Norito derives; SDK parity follows the normal guard
  lanes whenever the native schema changes.
- Architecture, manifest-pipeline, CLI, OpenAPI, status, and roadmap documents
  describe the shared validation path and endpoint behavior.

## Contract Implementation

| Task | Owner(s) | Notes |
|------|----------|-------|
| Registry storage and smart-contract state. | Core Infra / Smart Contract Team | Implemented in Iroha world state (`pin_manifests`, `manifest_aliases`, `replication_orders`) with deterministic Norito payload hashing and integer-only policy arithmetic. |
| Entry points: `RegisterPinManifest`, `ApprovePinManifest`, `RetirePinManifest`, `BindManifestAlias`, `IssueReplicationOrder`, `CompleteReplicationOrder`, `ExpireReplicationOrder`. | Core Infra | Registration carries the complete canonical manifest, resource-bounds and validates it in consensus, and derives all stored commitments. Core execution also validates aliases, council envelopes, governance permissions, canonical replication payloads, completion, and deadline-bound expiration. |
| State transitions: enforce succession (manifest A -> B), retention epochs, alias uniqueness, and replication status changes. | Governance Council / Core Infra | `ensure_successor_chain` enforces approved, non-retired, acyclic multi-hop lineage; alias uniqueness, retention, and replication issue/complete bookkeeping are covered by unit tests. |
| Governed parameters: load `ManifestPolicyV1` from config/governance state. | Governance Council | Runtime config maps pin-policy constraints into the shared validator. Live policy-change ceremonies are rollout governance evidence, not missing local contract code. |
| Registry telemetry and audit surface. | Observability | Torii exports registry metrics and attested REST snapshots. Additional signed event archives can be layered over those snapshots if governance requires them. |

Coverage:
- Unit tests cover registration, approval, retirement, alias binding, replication
  order issue/complete, permissions, duplicate rejection, and side-effect-free
  failure paths.
- Successor tests cover self references, unknown/pending/retired predecessors,
  cycle closure, and malformed existing predecessor cycles.
- `ci/check_sorafs_fixtures.sh` regenerates chunker, provider-admission, and pin
  registry fixtures and runs the parity checks that keep the canonical schema
  surface stable.

## Service Facade (Torii/SDK Integration)

| Component | Task | Owner(s) |
|-----------|------|----------|
| Torii Service | Ships `/v1/sorafs/pin`, `/v1/sorafs/pin/{digest_hex}`, `/v1/sorafs/aliases`, and `/v1/sorafs/replication`. The manifest-detail route returns exact native `PinManifestFinalizedRecordV1` JSON and accepts only the optional paired expected finalized height/hash precondition; pagination and filters remain on list routes. | Networking TL / Core Infra |
| Finality binding | Listing responses retain their listing attestation. A manifest-detail response carries the native `finalized_cursor` beside the authoritative `PinManifestRecord`; a stale requested cursor fails with HTTP 409. | Core Infra |
| CLI | `iroha app sorafs pin register`, `pin list`, `pin show`, `alias list`, and `replication list` wrap the REST and ISI surfaces for operator audits. | Tooling WG |
| SDK | Rust request builders and the JavaScript, Python, Swift, and C# guard lanes mirror the manifest payload and pin-register validation surface. | SDK Teams |

Operations:
- List endpoints use attested snapshots, deterministic pagination, and the cache
  behavior documented in the alias policy where alias proofs are involved.
- `GET /v1/sorafs/pin/{digest_hex}` returns only `finalized_cursor` and the
  native `manifest`. The retired `limit`, attestation, embedded alias/order
  arrays, counts, and truncation fields are absent; callers use
  `/v1/sorafs/aliases` and `/v1/sorafs/replication` for bounded list queries.
- Mutating operations go through ISI/governance permissions; REST handling keeps
  the same Torii auth and resource-guard model as the surrounding SoraFS APIs.

## فکسچر اور سی آئی- فکسچر ڈائرکٹری: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` اسٹورز نے سنیپ شاٹس منشور/عرف/آرڈر پر دستخط کیے ، جو `cargo run -p iroha_core --example gen_pin_snapshot` کے ذریعے دوبارہ تیار کیا گیا ہے۔
- CI مرحلہ: `ci/check_sorafs_fixtures.sh` SNAPSHOT کو دوبارہ تیار کرتا ہے اور CI فکسچر کو مطابقت پذیر رکھتے ہوئے ، مختلف پر کریش ہوتا ہے۔
-انضمام کے ٹیسٹ (`crates/iroha_core/tests/pin_registry.rs`) خوشگوار راستہ کے علاوہ نقل کے ساتھ نقل کی عرف کی ناکامی ، عرف کی منظوری/اسٹوریج گارڈز ، مماثل ہینڈلز چنکر ، نقل کی گنتی چیک اور تسلسل کے محافظوں کی ناکامیوں (نامعلوم/پہلے سے منظور شدہ/تخفیف/خود حوالہ جات) ؛ کور کی تفصیلات کے ل cases معاملات `register_manifest_rejects_*` دیکھیں۔
- یونٹ ٹیسٹ میں اب `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` میں عرف کی توثیق ، ​​اسٹوریج گارڈز اور جانشین کی جانچ پڑتال کا احاطہ کیا گیا ہے۔ جب ریاستی مشین کام کرنے لگے گی تو کثیر الجہتی جانشینی کا پتہ لگانا ظاہر ہوگا۔
- مشاہدہ کرنے والی پائپ لائنوں میں استعمال ہونے والے واقعات کے لئے گولڈن JSON۔

## ٹیلی میٹری اور مشاہدہ

میٹرکس (Prometheus):
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
-موجودہ فراہم کنندہ ٹیلی میٹری (`torii_sorafs_capacity_*` ، `torii_sorafs_fee_projection_nanos`) اس علاقے میں اختتام سے آخر تک ڈیش بورڈز کے لئے باقی ہے۔

نوشتہ جات:
- مینجمنٹ آڈیٹرز (دستخط شدہ؟) کے لئے ساختہ واقعہ کا بہاؤ Norito۔

انتباہات:
- ایس ایل اے سے تجاوز کرنے والے نقل کے احکامات زیر التواء۔
- عرف دہلیز کے نیچے ختم ہوجاتا ہے۔
- اسٹوریج کی خلاف ورزی (میعاد ختم ہونے سے پہلے تجدید نہیں کی گئی)۔

ڈیش بورڈز:
- Grafana JSON `docs/source/grafana_sorafs_pin_registry.json` ٹریک لائف سائیکل کل ، عرف کوریج ، بیک بلاگ سنترپتی ، SLA تناسب ، اوورلیس لیٹینسی بمقابلہ سلیک اور کال پر جائزوں کے لئے کھوئے ہوئے آرڈرز کا حصہ۔

## رن بکس اور دستاویزات

- رجسٹری کی حیثیت کی تازہ کاریوں کو قابل بنانے کے لئے `docs/source/sorafs/migration_ledger.md` کو اپ ڈیٹ کریں۔
- آپریٹر کی گائیڈ: میٹرکس کے ساتھ `docs/source/sorafs/runbooks/pin_registry_ops.md` (پہلے ہی شائع شدہ) ، انتباہ ، تعیناتی ، بیک اپ اور بازیابی کے ساتھ۔
- مینجمنٹ گائیڈ: پالیسی کی ترتیبات ، منظوری کے ورک فلو ، تنازعہ پروسیسنگ کی وضاحت کریں۔
- ہر اختتامی نقطہ (Docusaurus دستاویزات) کے لئے API حوالہ صفحات۔

## انحصار اور مستقل مزاجی

1. توثیق کے منصوبے کے کاموں کو مکمل کریں (مینی فیسٹ ویلیڈیٹر انضمام)۔
2. Norito + پالیسی ڈیفالٹس اسکیم کو حتمی شکل دیں۔
3. معاہدہ + سروس کو نافذ کریں ، ٹیلی میٹری سے رابطہ کریں۔
4. فکسچر کو دوبارہ تخلیق کریں ، انضمام سویٹ لانچ کریں۔
5. دستاویزات/رن بکس کو اپ ڈیٹ کریں اور روڈ میپ آئٹمز کو مکمل طور پر مکمل کریں۔

SF-4 چیک لسٹ میں موجود ہر آئٹم کو پیشرفت ریکارڈ کرتے وقت اس منصوبے کا حوالہ دینا چاہئے۔
باقی اگواڑا اب مصدقہ فہرست کے اختتامی نکات کے ساتھ آتا ہے:

- `GET /v1/sorafs/pin` returns the attested manifest catalogue.
- `GET /v1/sorafs/pin/{digest_hex}` returns exact `PinManifestFinalizedRecordV1` JSON with `finalized_cursor.height`, `finalized_cursor.block_hash`, and native `manifest`.
- `GET /v1/sorafs/aliases` اور `GET /v1/sorafs/replication` فعال شائع کریں
  مستقل صفحہ بندی کے ساتھ نقل کے احکامات کا عرف کیٹلاگ اور بیکلاگ اور
  اسٹیٹس فلٹرز۔

سی ایل آئی نے ان کالوں کو سمیٹ لیا (`iroha app sorafs pin list` ، `pin show` ، `alias list` ،
`replication list`) تاکہ آپریٹرز رجسٹری آڈٹ کو خود کار بنائیں
نچلے درجے کے APIs تک رسائی کے بغیر۔
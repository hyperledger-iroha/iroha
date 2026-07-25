---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/pin-registry-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: پن رجسٹری پلان
عنوان: SoraFS میں پن رجسٹری کے نفاذ کا منصوبہ
سائڈبار_لیبل: پن رجسٹری پلان
تفصیل: اسٹیٹ مشین رجسٹری ، Torii انٹرفیس ، ٹولنگ ، اور نگرانی کا احاطہ کرنے والا SF-4 عمل درآمد کا منصوبہ۔
---

::: منظور شدہ ماخذ کو نوٹ کریں
یہ صفحہ `docs/source/sorafs/pin_registry_plan.md` کی عکاسی کرتا ہے۔ جب تک پرانی دستاویزات فعال ہوں تب تک دونوں ورژن کو مطابقت پذیر رکھیں۔
:::

SoraFS (SF-4) میں # پن رجسٹری پر عمل درآمد کا منصوبہ

SF-4 پن رجسٹری نوڈس اور معاون خدمات مہیا کرتا ہے جو ظاہر ہوتا ہے کہ ظاہر ہوتا ہے۔
یہ پن کی پالیسیاں نافذ کرتا ہے ، اور Torii APIs ، گیٹ ویز ، اور آرکسٹریٹرز کو بے نقاب کرتا ہے۔
اس دستاویز میں توثیق کی اسکیم میں توسیع کی گئی ہے جس میں کنکریٹ کے نفاذ کے کاموں میں آن چین کی منطق کا احاطہ کیا گیا ہے ،
میزبان خدمات ، فکسچر ، اور آپریشنل ضروریات۔

## رینج

1. ** رجسٹری اسٹیٹ مشین **: Norito منشور ، عرفی اور جانشین کی زنجیروں کے لئے رجسٹر
   برقرار رکھنے کے زمانے اور گورننس میٹا ڈیٹا۔
2.
   `Completion` ، بے دخلی)۔
3. ** سروس انٹرفیس **: رجسٹری سے چلنے والے GRPC/REST اختتامی نقطہ Torii اور SDKs کے ذریعہ استعمال کیا جاتا ہے ،
   نمبر اور تعینات شامل ہے۔
4. ** ٹولنگ اور فکسچر **: سی ایل آئی مددگار ، ٹیسٹ ویکٹرز ، اور دستاویزات جو ہم آہنگی میں رہتے ہیں
   گورننس ظاہر ہوتا ہے ، عرفی اور لفافے۔
5.

## ڈیٹا ماڈل

### بنیادی ریکارڈ (Norito)

| ڈھانچہ | تفصیل | فیلڈز |
| -------- | ------- | -------- |
| `PinManifestRecord` | Chain-authoritative manifest lifecycle entry. The envelope digest and exact 36-byte CIDv1/dag-cbor/BLAKE3-256 content root are distinct commitments. | `digest`, `root_cid`, `chunker`, `chunk_digest_sha3_256`, `por_root`, `content_length`, `policy`, `submitted_by`, `submitted_epoch`, `alias`, `successor_of`, `metadata`, `status`, `retirement_reason`, `council_envelope_digest`, `pin_fee_payment`. |
| `PinManifestFinalizedRecordV1` | Immutable read result binding one native manifest record to the finalized block used for the query. | `finalized_cursor` (`height`, `block_hash`), `manifest`. |
| `AliasBindingV1` | الیاس کو پابند کریں -> منشور کا سی آئی ڈی۔ | `alias` ، `manifest_cid` ، `bound_at` ، `expiry_epoch`۔ |
| `ReplicationOrderV1` | فراہم کنندگان کو منشور انسٹال کرنے کے لئے ہدایات۔ | `order_id` ، `manifest_cid` ، `providers` ، `redundancy` ، `deadline` ، `policy_hash`۔ |
| `ReplicationReceiptV1` | فراہم کنندہ کا اعتراف۔ | `order_id` ، `provider_id` ، `status` ، `timestamp` ، `por_sample_digest`۔ |
| `ManifestPolicyV1` | گورننس پالیسی اسنیپ شاٹ۔ | `min_replicas` ، `max_retention_epochs` ، `allowed_profiles` ، `pin_fee_basis_points`۔ |

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

## فکسچر اور سی آئی

- فکسچر ڈائرکٹری: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` اسٹورز نے منشور/عرف/آرڈر کے اسنیپ شاٹس پر دستخط کیے ہیں جو `cargo run -p iroha_core --example gen_pin_snapshot` کے ذریعے دوبارہ پیدا ہوتے ہیں۔
- CI مرحلہ: `ci/check_sorafs_fixtures.sh` اسنیپ شاٹ کو دوبارہ تخلیق کرتا ہے اور جب تضادات ہوتے ہیں تو ، CI فکسچر کو ایک جیسے رکھنے کے لئے ناکام ہوجاتا ہے۔
-انضمام ٹیسٹ (`crates/iroha_core/tests/pin_registry.rs`) ڈپلیکیٹ عرف مسترد ، عرف کی منظوری/برقرار رکھنے کے تحفظ ، چنکر مماثل ہینڈلز ، کاپی گنتی کی جانچ پڑتال ، اور جھرن کے تحفظات کی ناکامی (گمنام/پہلے سے منظور شدہ/پیچھے/خود حوالہ والے پوائنٹس) کے ساتھ خوشگوار راستے کا احاطہ کرنا۔ کوریج کی تفصیلات کے ل cases معاملات `register_manifest_rejects_*` دیکھیں۔
- یونٹ ٹیسٹوں میں اب `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` میں عرف کی جانچ ، برقرار رکھنے کے تحفظات ، اور جانشینی کی جانچ پڑتال کا احاطہ کیا گیا ہے۔ ریاستی مشین آمد پر ملٹی ہاپ تسلسل کا پتہ لگانا۔
- پائپ لائنوں کی نگرانی میں استعمال ہونے والے واقعات کے لئے گولڈ جے ایس او این۔

## ٹیلی میٹری اور نگرانی

بینچ مارک (Prometheus):
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
-موجودہ فراہم کردہ ٹیلی میٹری (`torii_sorafs_capacity_*` ، `torii_sorafs_fee_projection_nanos`) اختتام سے آخر والے بورڈز کی حدود میں ہے۔ریکارڈز:
- ایونٹ اسٹریم Norito گورننس آڈٹ (سائٹ؟) کے لئے تشکیل دیا گیا ہے۔

انتباہات:
- زیر التواء دہرائے جانے والے احکامات جو ایس ایل اے سے تجاوز کرتے ہیں۔
- عرف کی میعاد ختم ہونے کی دہلیز سے کم ہے۔
- برقرار رکھنے کی خلاف ورزیوں (تکمیل سے پہلے تجدید نہیں)۔

انفارمیشن پینل:
- Grafana JSON فائل `docs/source/grafana_sorafs_pin_registry.json` مینی فیسٹ ، عرفیوس کوریج ، بیک بلاگ سنترپتی ، ایس ایل اے تناسب ، لیٹینسی بمقابلہ سلیک ، اور کسی حملے کے دوران جائزہ لینے کے لئے ناکام کمانڈ کی شرحوں کی کل لائف سائیکل کو ٹریک کرتا ہے۔

## رن بکس اور دستاویزات

- رجسٹری کی حیثیت کی تازہ کاریوں کو شامل کرنے کے لئے `docs/source/sorafs/migration_ledger.md` کو اپ ڈیٹ کیا گیا۔
- آپریٹر کا دستی: `docs/source/sorafs/runbooks/pin_registry_ops.md` (فی الحال شائع شدہ) میٹرکس ، انتباہ ، تعیناتی ، بیک اپ ، اور خدمت کی بازیابی کا احاطہ کرتا ہے۔
- گورننس گائیڈ: پالیسی پیرامیٹرز ، منظوری کے ورک فلو ، اور تنازعہ سے نمٹنے کی تفصیل۔
- ہر اختتامی نقطہ (Docusaurus دستاویزات) کے لئے API حوالہ صفحات۔

## انحصار اور سیریلائزیشن

1. توثیق کے مکمل منصوبے کے مکمل کام (مینی فیسٹ ویلیڈیٹر انضمام)۔
2. اختتامی اسکیم Norito + ڈیفالٹ پالیسی اقدار۔
3. معاہدہ + سروس اور ٹیلی میٹرک کنکشن کا نفاذ۔
4. فکسچر کو دوبارہ تخلیق کریں اور انضمام کے ٹیسٹ چلائیں۔
5. دستاویزات/رن بکس کو اپ ڈیٹ کریں اور روڈ میپ آئٹمز کو مکمل طور پر مکمل کریں۔

SF-4 کے اندر ہر چیک لسٹ کو پیشرفت ریکارڈ کرتے وقت اس منصوبے کا حوالہ دینا چاہئے۔
باقی انٹرفیس اب اٹسٹیشن کے ساتھ فہرست کے اختتامی مقامات فراہم کرتا ہے:

- `GET /v1/sorafs/pin` returns the attested manifest catalogue.
- `GET /v1/sorafs/pin/{digest_hex}` returns exact `PinManifestFinalizedRecordV1` JSON with `finalized_cursor.height`, `finalized_cursor.block_hash`, and native `manifest`.
- `GET /v1/sorafs/aliases` اور `GET /v1/sorafs/replication` عرف کیٹلاگ کو ظاہر کرتا ہے
  فکسڈ نمبرنگ اور اسٹیٹس فلٹرز کے ساتھ فعال اور جمع شدہ دہرانے والے کمانڈز۔

سی ایل آئی ان کالوں (`iroha app sorafs pin list` ، `pin show` ، `alias list` ،
`replication list`) لہذا آپریٹرز ٹچ لیس رجسٹری آڈٹ کو خود کار بناسکتے ہیں
نچلے درجے کے APIs۔
---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/pin-registry-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: پن رجسٹری پلان
عنوان: SoraFS پن رجسٹری نفاذ کا منصوبہ
سائڈبار_لیبل: پن رجسٹری پلان
تفصیل: رجسٹری اسٹیٹ مشین ، Torii facade ، ٹولنگ اور مشاہدہ کرنے کا احاطہ کرنے والا SF-4 عمل درآمد کا منصوبہ۔
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/sorafs/pin_registry_plan.md` کی عکاسی کرتا ہے۔ جب تک میراثی دستاویزات متحرک رہیں تب تک دونوں کاپیاں ہم آہنگ رکھیں۔
:::

# SoraFS (SF-4) پن رجسٹری عمل درآمد کا منصوبہ

SF-4 پن رجسٹری کا معاہدہ اور معاون خدمات فراہم کرتا ہے جو اسٹور کرتے ہیں
منشور کا ارتکاب ، پن کی پالیسیاں نافذ کریں ، اور Torii کے لئے APIs کو بے نقاب کریں ،
گیٹ وے اور آرکیسٹریٹرز۔ اس دستاویز سے توثیق کے منصوبے کو وسعت دی گئی ہے
کنکریٹ کے نفاذ کے کام ، آن چین کی منطق کا احاطہ کرتے ہیں ،
میزبان ، فکسچر اور آپریشنل ضروریات۔

## دائرہ کار

1. ** رجسٹری اسٹیٹ مشین **: Norito کے ذریعہ ظاہر کردہ ریکارڈز ،
   عرفی ، جانشین کی زنجیریں ، برقرار رکھنے کے عہد ، اور گورننس میٹا ڈیٹا۔
2.
   پنوں کی (`ReplicationOrder` ، `Precommit` ، `Completion` ، بے دخلی)۔
3. ** خدمت کا اگواڑا **: GRPC/REST اختتامی مقامات رجسٹری کے ذریعہ تعاون یافتہ ہیں جو Torii
   اور ایس ڈی کے استعمال کرتے ہیں ، بشمول پیجنگ اور تصدیق۔
4.
   مطابقت پذیری ، عرفی ، اور گورننس لفافے۔
5.

## ڈیٹا ماڈل

### مرکزی ریکارڈ (Norito)

| ڈھانچہ | تفصیل | فیلڈز |
| -------- | ----------- | -------- |
| `PinManifestRecord` | Chain-authoritative manifest lifecycle entry. The envelope digest and exact 36-byte CIDv1/dag-cbor/BLAKE3-256 content root are distinct commitments. | `digest`, `root_cid`, `chunker`, `chunk_digest_sha3_256`, `por_root`, `content_length`, `policy`, `submitted_by`, `submitted_epoch`, `alias`, `successor_of`, `metadata`, `status`, `retirement_reason`, `council_envelope_digest`, `pin_fee_payment`. |
| `PinManifestFinalizedRecordV1` | Immutable read result binding one native manifest record to the finalized block used for the query. | `finalized_cursor` (`height`, `block_hash`), `manifest`. |
| `AliasBindingV1` | نقشہ جات عرف -> مینی فیسٹ سی آئی ڈی۔ | `alias` ، `manifest_cid` ، `bound_at` ، `expiry_epoch`۔ |
| `ReplicationOrderV1` | فراہم کرنے والوں کو ظاہر کرنے کے لئے ہدایت۔ | `order_id` ، `manifest_cid` ، `providers` ، `redundancy` ، `deadline` ، `policy_hash`۔ |
| `ReplicationReceiptV1` | فراہم کنندہ کی تصدیق. | `order_id` ، `provider_id` ، `status` ، `timestamp` ، `por_sample_digest`۔ |
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

## فکسچر اور سی آئی- فکسچر ڈائرکٹری: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` اسٹورز نے ظاہر/عرف/آرڈر کے سنیپ شاٹس پر دستخط کیے جو `cargo run -p iroha_core --example gen_pin_snapshot` کے ذریعہ دوبارہ پیدا ہوئے ہیں۔
- CI مرحلہ: `ci/check_sorafs_fixtures.sh` اسنیپ شاٹ کو دوبارہ تخلیق کرتا ہے اور اگر CI فکسچر کو منسلک رکھتے ہوئے فرق ہے تو ناکام ہوجاتا ہے۔
- انضمام ٹیسٹ (`crates/iroha_core/tests/pin_registry.rs`) ورزش ہیپی فلو پلس ڈپلیکیٹ عرف مسترد ، عرف منظوری/برقرار رکھنے والے محافظ ، متضاد چنکر ہینڈلز ، نقل کی گنتی کی توثیق اور جانشینی گارڈ کی ناکامی (نامعلوم/preaproved/remeted/خود-ریفرنس پوائنٹس) ؛ کوریج کی تفصیلات کے ل cases معاملات `register_manifest_rejects_*` دیکھیں۔
- یونٹ ٹیسٹ میں اب `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` میں عرف کی توثیق ، ​​برقرار رکھنے کے محافظوں اور جانشین کی جانچ پڑتال کا احاطہ کیا گیا ہے۔ جب ریاستی مشین آتی ہے تو ملٹی ہاپ جانشینی کا پتہ لگانا۔
- مشاہدہ پائپ لائنوں کے ذریعہ استعمال ہونے والے واقعات کے لئے JSON گولڈن۔

## ٹیلی میٹری اور مشاہدہ

میٹرکس (Prometheus):
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
-موجودہ فراہم کنندہ ٹیلی میٹری (`torii_sorafs_capacity_*` ، `torii_sorafs_fee_projection_nanos`) اختتام سے آخر میں ڈیش بورڈز کی گنجائش میں ہے۔

نوشتہ جات:
- گورننس آڈٹ (سبسکرائب شدہ؟) کے لئے ساختہ Norito واقعات کا سلسلہ۔

انتباہات:
- ایس ایل اے سے تجاوز کرنے والے نقل کے احکامات زیر التواء۔
- دہلیز کے نیچے عرف کی میعاد ختم ہونا۔
- برقرار رکھنے کی خلاف ورزی (ختم ہونے سے پہلے ظاہر نہیں ہوا)۔

ڈیش بورڈز:
- Grafana `docs/source/grafana_sorafs_pin_registry.json` JSON ٹریک کرتا ہے لائف سائکل کل ، عرف کوریج ، بیک بلاگ سنترپتی ، ایس ایل اے تناسب ، لیٹینسی بمقابلہ سلیک اوورلیز ، اور آن کال جائزہ لینے کے لئے آرڈر کی شرح کھوئے۔

## رن بکس اور دستاویزات

- رجسٹری کی حیثیت کی تازہ کاریوں کو شامل کرنے کے لئے `docs/source/sorafs/migration_ledger.md` کو اپ ڈیٹ کریں۔
- آپریٹر گائیڈ: `docs/source/sorafs/runbooks/pin_registry_ops.md` (پہلے ہی شائع شدہ) میٹرکس ، انتباہات ، تعیناتی ، بیک اپ اور بازیابی کے بہاؤ کا احاطہ کرتا ہے۔
- گورننس گائیڈ: پالیسی پیرامیٹرز ، منظوری کے ورک فلو ، تنازعہ سے نمٹنے کی وضاحت کریں۔
- ہر اختتامی نقطہ (دستاویزات Docusaurus) کے لئے API حوالہ صفحات۔

## انحصار اور ترتیب

1. توثیق کے مکمل کاموں کے کام (مینی فیسٹ ویلیڈیٹر انضمام)۔
2. اسکیم Norito + پالیسی ڈیفالٹس کو حتمی شکل دیں۔
3. معاہدہ + سروس کو نافذ کریں ، ٹیلی میٹری سے رابطہ کریں۔
4. فکسچر کو دوبارہ تخلیق کریں ، انضمام سوٹ چلائیں۔
5. دستاویزات/رن بکس کو اپ ڈیٹ کریں اور روڈ میپ آئٹمز کو مکمل طور پر مکمل کریں۔

ہر SF-4 چیک لسٹ کو اس منصوبے کا حوالہ دینا چاہئے جب پیشرفت ہوگی۔
باقی اگواڑا اب تصدیق کے ساتھ فہرست سازی کے اختتامی مقامات فراہم کرتا ہے:

- `GET /v1/sorafs/pin` returns the attested manifest catalogue.
- `GET /v1/sorafs/pin/{digest_hex}` returns exact `PinManifestFinalizedRecordV1` JSON with `finalized_cursor.height`, `finalized_cursor.block_hash`, and native `manifest`.
- `GET /v1/sorafs/aliases` اور `GET /v1/sorafs/replication` کی کیٹلاگ کو بے نقاب کریں
  فعال عرف اور مستقل صفحہ بندی کے ساتھ نقل کے احکامات کا بیک بلاگ اور
  اسٹیٹس فلٹرز۔سی ایل آئی ان کالوں (`iroha app sorafs pin list` ، `pin show` ، `alias list` ،
`replication list`) تاکہ آپریٹرز سسٹم آڈٹ کو خود کار بنائیں۔
نچلے درجے کے APIs کو چھوئے بغیر رجسٹری۔
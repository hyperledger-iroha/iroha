---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/pin-registry-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: خطة التسجيل
العنوان: خطة تنفيذ رقم التعريف الشخصي لـ SoraFS
Sidebar_label: Plan du Pin Registry
الوصف: خطة التنفيذ SF-4 تغطي آلة التسجيل والواجهة Torii والأدوات وإمكانية الملاحظة.
---

:::ملاحظة المصدر الكنسي
هذه الصفحة تعكس `docs/source/sorafs/pin_registry_plan.md`. قم بمزامنة نسختين حتى تظل الوثائق الموروثة نشطة.
:::

# خطة تنفيذ Pin Registry لـ SoraFS (SF-4)

SF-4 يحرر عقد Pin Registry والخدمات التي يتم دعمها من المخزون
التزامات البيان، تطبيق سياسات التثبيت وكشفها
واجهة برمجة التطبيقات (API) إلى Torii والبوابات المساعدة والمنسقين المساعدين. هذه الوثيقة étend لو الخطة دي
التحقق من صحة تقنيات التنفيذ الخرسانية يغطي المنطق
على السلسلة والخدمات السريعة والتركيبات والمتطلبات التشغيلية.

## بورتيه1. **جهاز تسجيل البيانات**: يقوم بتسجيل Norito للبيانات، والأسماء المستعارة،
   سلاسل الخلافة، وفترات الاحتفاظ، وفترات الحوكمة.
2. **تنفيذ العقد**: عمليات CRUD تحدد دورة الحياة
   الدبابيس (`ReplicationOrder`، `Precommit`، `Completion`، الإخلاء).
3. **واجهة الخدمة**: نقاط النهاية gRPC/REST مدعومة بالسجل المستهلك وفقًا لـ Torii
   ومجموعات SDK، مع ترقيم الصفحات والشهادة.
4. **الأدوات والتركيبات**: مساعدات CLI، ناقلات الاختبار والوثائق
   حراسة البيانات والأسماء المستعارة ومغلفات الإدارة بشكل متزامن.
5. **القياس عن بعد والعمليات**: المقاييس والتنبيهات ودفاتر التشغيل لسلامة التسجيل.

## نموذج البيانات

### مبادئ التسجيل (Norito)| هيكل | الوصف | الأبطال |
|--------|-----------|--------|
| `PinManifestRecord` | Chain-authoritative manifest lifecycle entry. The envelope digest and exact 36-byte CIDv1/dag-cbor/BLAKE3-256 content root are distinct commitments. | `digest`, `root_cid`, `chunker`, `chunk_digest_sha3_256`, `por_root`, `content_length`, `policy`, `submitted_by`, `submitted_epoch`, `alias`, `successor_of`, `metadata`, `status`, `retirement_reason`, `council_envelope_digest`, `pin_fee_payment`. |
| `PinManifestFinalizedRecordV1` | Immutable read result binding one native manifest record to the finalized block used for the query. | `finalized_cursor` (`height`, `block_hash`), `manifest`. |
| `AliasBindingV1` | الاسم المستعار Mappe -> بيان CID. | `alias`، `manifest_cid`، `bound_at`، `expiry_epoch`. |
| `ReplicationOrderV1` | تعليمات حول كيفية قيام مقدمي الخدمة بتحديد البيان. | `order_id`، `manifest_cid`، `providers`، `redundancy`، `deadline`، `policy_hash`. |
| `ReplicationReceiptV1` | Accusé de réception du Provider. | `order_id`، `provider_id`، `status`، `timestamp`، `por_sample_digest`. |
| `ManifestPolicyV1` | لقطة من سياسة الحكم. | `min_replicas`، `max_retention_epochs`، `allowed_profiles`، `pin_fee_basis_points`. |

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

## تركيبات وCI- ملف التركيبات: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` مخزون اللقطات الموقعة من البيان/الاسم المستعار/الطلب المُعاد إنشاؤه عبر `cargo run -p iroha_core --example gen_pin_snapshot`.
- شريط CI : `ci/check_sorafs_fixtures.sh` ينشئ اللقطة ويلتقطها في حالة الاختلاف، مع محاذاة تركيبات CI.
- اختبارات التكامل (`crates/iroha_core/tests/pin_registry.rs`) تشمل المسار السعيد بالإضافة إلى رفض الأسماء المستعارة المكررة، ووحدات حماية الموافقة/الاحتفاظ، ومقابض القطع غير المتوافقة، والتحقق من صحة حساب النسخ المتماثلة، وفحوصات حماية التسلسل (المؤشرات) inconnus/pre-approvés/retirés/auto-référencés) ; انظر إلى الحالة `register_manifest_rejects_*` للحصول على تفاصيل الغطاء.
- تغطي الاختبارات الوحدوية التحقق من صحة الأسماء المستعارة ووحدات الاحتفاظ وفحوصات النجاح في `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`؛ اكتشاف الخلافة متعددة القفزات في آلة الحالات.
- JSON الذهبي للأحداث المستخدمة من خلال خطوط الأنابيب القابلة للمراقبة.

## التحكم عن بعد وقابلية الملاحظة

المقاييس (Prometheus):
-`torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
-`torii_sorafs_registry_aliases_total`
-`torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
-`torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
-`torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
-`torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- موفر خدمة القياس عن بعد الموجود (`torii_sorafs_capacity_*`، `torii_sorafs_fee_projection_nanos`) موجود في نطاق لوحات المعلومات من البداية إلى النهاية.

السجلات :
- هيكل تدفق الأحداث Norito لعمليات تدقيق الحوكمة (التوقيعات؟).التنبيهات :
- Ordres de réplication en attente dépassant le SLA.
- انتهاء الاسم المستعار < seuil.
- انتهاكات الاحتجاز (بيان غير متجدد قبل انتهاء الصلاحية).

لوحات المعلومات :
- يتناسب JSON Grafana `docs/source/grafana_sorafs_pin_registry.json` مع كامل دورة حياة البيانات، والغطاء المستعار، وتشبع الأعمال المتراكمة، ونسبة SLA، وتراكبات الكمون مقابل الركود، ومجموعات الأوامر المفقودة للعرض عند الطلب.

## أدلة التشغيل والوثائق

- Mettre à jour `docs/source/sorafs/migration_ledger.md` لتضمين بيانات حالة التسجيل في اليوم.
- دليل المشغل: `docs/source/sorafs/runbooks/pin_registry_ops.md` (منشور من قبل) يغطي المقاييس، والتنبيه، والنشر، والحفظ، وتدفق التكرار.
- دليل الحوكمة: تحديد إعدادات السياسة، وسير العمل بالموافقة، وإدارة الدعاوى القضائية.
- الصفحات المرجعية API لكل نقطة نهاية (docs Docusaurus).

## التبعيات والتسلسل

1. قم بإنهاء لمسات خطة التحقق من الصحة (بيان التكامل).
2. قم بإنهاء المخطط Norito + الإعدادات السياسية الافتراضية.
3. قم بتنفيذ العقد + الخدمة، ثم انتقل إلى الاتصال عن بعد.
4. قم بإعادة تركيب التركيبات وتنفيذ مجموعات التكامل.
5. قم بالاطلاع على المستندات/دفاتر التشغيل وحدد عناصر خريطة الطريق كاملة.

كل عنصر من قائمة التحقق SF-4، يرجى الرجوع إلى هذه الخطة عند تسجيل التقدم.
- `GET /v1/sorafs/pin` returns the attested manifest catalogue.
- `GET /v1/sorafs/pin/{digest_hex}` returns exact `PinManifestFinalizedRecordV1` JSON with `finalized_cursor.height`, `finalized_cursor.block_hash`, and native `manifest`.
- `GET /v1/sorafs/aliases` و`GET /v1/sorafs/replication` يعرض الكتالوج
  الاسم المستعار النشط وتراكم أوامر النسخ مع صفحة متماسكة
  ومرشحات الحالة.

La CLI تغليف ces appels (`iroha app sorafs pin list`، `pin show`، `alias list`،
`replication list`) للسماح لمشغلي عمليات التدقيق التلقائية
التسجيل بدون لمس aux APIs ذو المستوى الأساسي.
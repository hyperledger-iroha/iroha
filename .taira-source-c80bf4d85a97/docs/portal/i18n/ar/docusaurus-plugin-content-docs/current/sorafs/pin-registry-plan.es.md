---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/pin-registry-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: خطة التسجيل
العنوان: خطة تنفيذ Pin Registry لـ SoraFS
Sidebar_label: خطة التسجيل Pin
الوصف: خطة تنفيذ SF-4 التي تغطي حالة آلة التسجيل والواجهة Torii والأدوات وقابلية المراقبة.
---

:::ملاحظة فوينتي كانونيكا
هذه الصفحة تعكس `docs/source/sorafs/pin_registry_plan.md`. احتفظ بنسخ متزامنة أثناء تنشيط المستندات المخزنة.
:::

# خطة تنفيذ Pin Registry لـ SoraFS (SF-4)

يقوم SF-4 بإدراج عقد Pin Registry وخدمات تخزين الدعم
تسويات البيان، إضافة سياسات الدبوس وتوسيع واجهات برمجة التطبيقات إلى Torii، والبوابات
و Orquestadores. هذه الوثيقة موسعة لخطة التحقق من الصحة مع خطط العمل
التنفيذ الملموس، استكشاف المنطق على السلسلة، خدمات المضيف،
التركيبات ومتطلبات التشغيل.

## الكانس1. **آلة حالة التسجيل**: السجلات المحددة لـ Norito للبيانات،
   الأسماء المستعارة، والسلاسل اللاحقة، وفترات الاحتفاظ، وبيانات التعريف الحكومية.
2. **تنفيذ العقد**: عمليات التحديد الخام لدائرة الحياة
   دي دبابيس (`ReplicationOrder`، `Precommit`، `Completion`، الإخلاء).
3. **واجهة الخدمة**: نقاط النهاية gRPC/REST التي يتم الرد عليها من خلال السجل الذي تستخدمه
   Torii ومجموعات SDK، بما في ذلك الصفحة والشهادة.
4. **الأدوات والتركيبات**: مساعدات CLI، وناقلات الاختبار، والوثائق للصيانة
   البيانات والأسماء المستعارة والمغلفات المتزامنة.
5. **القياس عن بعد والعمليات**: المقاييس والتنبيهات ودفاتر التشغيل لسلامة التسجيل.

## نموذج البيانات

### السجلات المركزية (Norito)| البنية التحتية | الوصف | كامبوس |
|------------|-------------|--------|
| `PinManifestRecord` | Chain-authoritative manifest lifecycle entry. The envelope digest and exact 36-byte CIDv1/dag-cbor/BLAKE3-256 content root are distinct commitments. | `digest`, `root_cid`, `chunker`, `chunk_digest_sha3_256`, `por_root`, `content_length`, `policy`, `submitted_by`, `submitted_epoch`, `alias`, `successor_of`, `metadata`, `status`, `retirement_reason`, `council_envelope_digest`, `pin_fee_payment`. |
| `PinManifestFinalizedRecordV1` | Immutable read result binding one native manifest record to the finalized block used for the query. | `finalized_cursor` (`height`, `block_hash`), `manifest`. |
| `AliasBindingV1` | الاسم المستعار Mapea -> بيان CID. | `alias`، `manifest_cid`، `bound_at`، `expiry_epoch`. |
| `ReplicationOrderV1` | تعليمات لكي يقوم مقدمو الخدمة بتثبيت البيان. | `order_id`، `manifest_cid`، `providers`، `redundancy`، `deadline`، `policy_hash`. |
| `ReplicationReceiptV1` | سبب تلقي المزود. | `order_id`، `provider_id`، `status`، `timestamp`، `por_sample_digest`. |
| `ManifestPolicyV1` | لقطة من سياسة الحكومة. | `min_replicas`، `max_retention_epochs`، `allowed_profiles`، `pin_fee_basis_points`. |

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

## المباريات وCI- دليل التركيبات: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` يحمي اللقطات الثابتة من البيان/الاسم المستعار/الطلب المُجدد بواسطة `cargo run -p iroha_core --example gen_pin_snapshot`.
- خطوة CI: `ci/check_sorafs_fixtures.sh` تقوم بإعادة إنشاء اللقطة وإيقافها إذا كانت هناك اختلافات، والحفاظ على تركيبات CI المنفصلة.
- اختبارات التكامل (`crates/iroha_core/tests/pin_registry.rs`) لإخراج التدفق من نفس الاسم المستعار المكرر، وحماية/الاحتفاظ بالاسم المستعار، ومقابض القطع المتحللة، والتحقق من صحة محتوى النسخ المتماثلة، وفشل حماية النسخ (النقاط) desconocidos/preaprobados/retirados/autorreferencias); شاهد الحالات `register_manifest_rejects_*` لتفاصيل التغطية.
- الاختبارات الموحدة الآن بعد التحقق من صحة الأسماء المستعارة، وحماية الاحتفاظ، والتحقق من المتابعة في `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`؛ The Detection of Sucesion Multi-Hop When the Machine Status.
- JSON الذهبي للأحداث المستخدمة من خلال خطوط أنابيب المراقبة.

## القياس عن بعد وإمكانية المراقبة

المقاييس (Prometheus):
-`torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
-`torii_sorafs_registry_aliases_total`
-`torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
-`torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
-`torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
-`torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- يوجد مزودو خدمات القياس عن بعد (`torii_sorafs_capacity_*`، `torii_sorafs_fee_projection_nanos`) متواصلون على لوحات المعلومات من البداية إلى النهاية.

السجلات:
- دفق الأحداث Norito الهيكلية لمراجعي الإدارة (الشركات؟).التنبيهات:
- أوامر النسخ المتماثلة التي تتجاوز جيش تحرير السودان (SLA).
- انتهاء صلاحية الاسم المستعار للظل.
- Violaciones de retencion (بيان عدم التجديد قبل انتهاء الصلاحية).

لوحات المعلومات:
- JSON من Grafana `docs/source/grafana_sorafs_pin_registry.json` يوزع إجمالي دورة حياة البيانات، وتغطية الاسم المستعار، وتشبع الأعمال المتراكمة، ونسبة SLA، وتراكبات زمن الاستجابة مقابل الركود، وتكاليف الأوامر المفقودة للمراجعة عند الطلب.

## كتب التشغيل والوثائق

- تحديث `docs/source/sorafs/migration_ledger.md` لإضافة تحديثات حالة التسجيل.
- دليل المشغلين: `docs/source/sorafs/runbooks/pin_registry_ops.md` (الإعلان) عن طريق قياس المقاييس والتنبيهات والنشر والنسخ الاحتياطي وتدفقات الاسترداد.
- دليل الإدارة: وصف المعلمات السياسية، وسير العمل، وإدارة النزاعات.
- الصفحات المرجعية لواجهة برمجة التطبيقات (API) لكل نقطة نهاية (Docusaurus docs).

## التبعيات والتسلسل

1. أكمل خطط التحقق من الصحة (تكامل البيان).
2. تم الانتهاء من الملف Norito + الإعدادات السياسية الافتراضية.
3. تنفيذ العقود + الخدمات، الاتصال بالقياس عن بعد.
4. تجديد التركيبات، وتصحيح مجموعات التكامل.
5. قم بتحديث المستندات/دفاتر التشغيل وتمييز عناصر خريطة الطريق بشكل كامل.

يجب الرجوع إلى كل قائمة مرجعية لـ SF-4 لهذه الخطة عند ظهور تقدم.
- `GET /v1/sorafs/pin` returns the attested manifest catalogue.
- `GET /v1/sorafs/pin/{digest_hex}` returns exact `PinManifestFinalizedRecordV1` JSON with `finalized_cursor.height`, `finalized_cursor.block_hash`, and native `manifest`.
- `GET /v1/sorafs/aliases` و `GET /v1/sorafs/replication` يعرض الكتالوج
  الاسم المستعار النشط وتراكم أوامر النسخ المتماثل مع صفحات متسقة
  مرشحات الحالة.

يقوم La CLI بتنشيط هذه المكالمات (`iroha app sorafs pin list`، `pin show`، `alias list`،
`replication list`) حتى يتمكن المشغلون من أتمتة جلسات الاستماع
التسجيل بدون واجهات برمجة التطبيقات (APIs) على مستوى منخفض.
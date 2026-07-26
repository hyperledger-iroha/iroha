---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/pin-registry-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: خطة التسجيل
العنوان: Plano de Implementacao do Pin Registry do SoraFS
Sidebar_label: Plano do Pin Registry
الوصف: خطة تنفيذ SF-4 cobrindo لآلة حالة التسجيل، وإعداد Torii، والأدوات وقابلية المراقبة.
---

:::ملاحظة فونتي كانونيكا
هذه الصفحة تعكس `docs/source/sorafs/pin_registry_plan.md`. الحفاظ على العمل كنسخ متزامنة أثناء وجود مستند دائم.
:::

# خطة تنفيذ سجل Pin لـ SoraFS (SF-4)

يدخل SF-4 إلى عقد Pin Registry وخدمات المساعدة في التخزين
تنازلات البيان، وفرض السياسة على Pin وإظهار واجهات برمجة التطبيقات لـ Torii،
البوابات والأوركسترادور. هذا المستند واسع النطاق أو مخطط التحقق من الصحة com
مهام تنفيذ الخرسانة، وتطبيق المنطق على السلسلة، والخدمات التي تقوم بها
المضيف والتركيبات ومتطلبات التشغيل.

##اسكوبو1. **آلة حالة التسجيل**: السجلات المحددة لـ Norito للبيانات،
   الأسماء المستعارة، والسلاسل اللاحقة، وفترات الاحتفاظ، وامتدادات الحوكمة.
2. **تنفيذ العقود**: العمليات الحتمية الخام لسلسلة الحياة
   دبابيس دوس (`ReplicationOrder`، `Precommit`، `Completion`، الإخلاء).
3. **واجهة الخدمة**: نقاط النهاية gRPC/REST المدعومة بالتسجيل Torii
   تستهلك أدوات تطوير البرامج (SDK) لنظام التشغيل، بما في ذلك الصفحة والمصادقة.
4. **الأدوات والتركيبات**: مساعدات CLI، واختبارات الاختبار، والتوثيق للمساعدة
   البيانات والأسماء المستعارة والمغلفات الحكومية المتزامنة.
5. **القياس عن بعد والعمليات**: المقاييس والتنبيهات ودفاتر التشغيل لجميع عمليات التسجيل.

## موديلو دي دادوس

### السجلات المركزية (Norito)| هيكل | وصف | كامبوس |
|--------|----------|--------|
| `PinManifestRecord` | Chain-authoritative manifest lifecycle entry. The envelope digest and exact 36-byte CIDv1/dag-cbor/BLAKE3-256 content root are distinct commitments. | `digest`, `root_cid`, `chunker`, `chunk_digest_sha3_256`, `por_root`, `content_length`, `policy`, `submitted_by`, `submitted_epoch`, `alias`, `successor_of`, `metadata`, `status`, `retirement_reason`, `council_envelope_digest`, `pin_fee_payment`. |
| `PinManifestFinalizedRecordV1` | Immutable read result binding one native manifest record to the finalized block used for the query. | `finalized_cursor` (`height`, `block_hash`), `manifest`. |
| `AliasBindingV1` | الاسم المستعار Mapeia -> CID de Manifest. | `alias`، `manifest_cid`، `bound_at`، `expiry_epoch`. |
| `ReplicationOrderV1` | تعليمات لمقدمي الخدمات لإصلاح البيان. | `order_id`، `manifest_cid`، `providers`، `redundancy`، `deadline`، `policy_hash`. |
| `ReplicationReceiptV1` | تأكيد القيام بالموفر. | `order_id`، `provider_id`، `status`، `timestamp`، `por_sample_digest`. |
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

## المباريات وCI- دليل التركيبات: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` لقطات مخزنة تمت مهاجمتها من البيان/الاسم المستعار/تم تجديد الطلب بواسطة `cargo run -p iroha_core --example gen_pin_snapshot`.
- Etapa de CI: `ci/check_sorafs_fixtures.sh` regenera o snapshot e falha se hover diffs، مع الحفاظ على تركيبات CI alinhados.
- اختبارات التكامل (`crates/iroha_core/tests/pin_registry.rs`) تمارس تدفقًا أفضل من خلال استعادة الاسم المستعار المكرر، وحماية المصادقة/الاحتفاظ بالاسم المستعار، ومقابض القطع غير المتوافقة، والتحقق من صحة النسخ المتماثلة، وخطأ حماية النجاح (الطول) desconhecidos/preaprovados/retirados/autorreferencias); شاهد الحالة `register_manifest_rejects_*` لتفاصيل التغطية.
- الخصيتين الوحدويتين الآن تشملان التحقق من الاسم المستعار، وحماية الاحتفاظ وشيكات الوريث في `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`؛ اكتشاف النجاح متعدد القفزات عند تشغيل الآلة.
- JSON الذهبي للأحداث المستخدمة في خطوط أنابيب المراقبة.

## القياس عن بعد وإمكانية المراقبة

المقاييس (Prometheus):
-`torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
-`torii_sorafs_registry_aliases_total`
-`torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
-`torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
-`torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
-`torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- مقياس عن بعد موجود من قبل مقدمي الخدمة (`torii_sorafs_capacity_*`، `torii_sorafs_fee_projection_nanos`) متاح دائمًا للوحات المعلومات من طرف إلى طرف.

السجلات:
- دفق الأحداث Norito التي تم تصميمها لمحاكمات الاستماع (القتلة؟).

التنبيهات:
- أوامر النسخ المعلقة تتجاوز جيش تحرير السودان.
- انتهاء الصلاحية من الاسم المستعار إلى الحد الأقصى.
- Violacoes de retencao (manifest nao renovado antes de expirar).لوحات المعلومات:
- O JSON do Grafana `docs/source/grafana_sorafs_pin_registry.json` راستريا كاملة لسلسلة الحياة من البيانات، تغطية الاسم المستعار، إشباع الأعمال المتراكمة، تجزئة SLA، تراكبات زمن الاستجابة مقابل slack وضرائب أوامر الخسارة للمراجعة عند الطلب.

## Runbooks و documentacao

- تحديث `docs/source/sorafs/migration_ledger.md` لتضمين تحديث حالة التسجيل.
- دليل المشغل: `docs/source/sorafs/runbooks/pin_registry_ops.md` (تم نشره) يتضمن مقاييس وتنبيهات ونشر ونسخ احتياطي وتدفقات استرداد.
- دليل الإدارة: الكشف عن المعلمات السياسية، وسير العمل في الموافقة، ومعالجة النزاعات.
- الصفحات المرجعية لواجهة برمجة التطبيقات (API) لكل نقطة نهاية (docs Docusaurus).

## التبعيات والتسلسل

1. إكمال تعريفات خطة التحقق (تكامل بيان المصادقة).
2. تم الانتهاء من الملف Norito + الإعدادات السياسية الافتراضية.
3. تنفيذ عقد + خدمة، توصيل القياس عن بعد.
4. تجديد التركيبات وأجنحة القضبان المتكاملة.
5. قم بتحديث المستندات/دفاتر التشغيل وتمييز عناصر خريطة الطريق بشكل كامل.

يجب أن تشير كل قائمة مرجعية من SF-4 إلى هذه الخطة عند التقدم.
- `GET /v1/sorafs/pin` returns the attested manifest catalogue.
- `GET /v1/sorafs/pin/{digest_hex}` returns exact `PinManifestFinalizedRecordV1` JSON with `finalized_cursor.height`, `finalized_cursor.block_hash`, and native `manifest`.
- `GET /v1/sorafs/aliases` و `GET /v1/sorafs/replication` معرض أو كتالوج دي
  الاسم المستعار ativo e o backlog de أوامر النسخ المتماثل مع الصفحات المتسقة e
  مرشحات الحالة.

تغليف CLI هو essas chamadas (`iroha app sorafs pin list`، `pin show`، `alias list`،
`replication list`) لكي يقوم المشغلون بأتمتة القاعات
قم بالتسجيل في واجهات برمجة التطبيقات (APIs) على المستوى الأساسي.
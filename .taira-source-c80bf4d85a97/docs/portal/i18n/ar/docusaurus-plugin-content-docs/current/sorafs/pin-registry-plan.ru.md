---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/pin-registry-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: خطة التسجيل
العنوان: خطة تحقيق رقم التعريف الشخصي للتسجيل SoraFS
Sidebar_label: خطة تسجيل الدبوس
الوصف: خطة تحقيق SF-4، تسجيل الجهاز المميز، المرحلة Torii، الأدوات والمراقبة.
---

:::note Канонический источник
يتم إرسال هذا الشريط إلى `docs/source/sorafs/pin_registry_plan.md`. قم بالنسخ المتزامن بعد تفعيل التوثيق التالي.
:::

# خطة تحقيق Pin Registry SoraFS (SF-4)

SF-4 ينشر عقد التسجيل وخدمات الدعم التي يتم تقديمها
بيان الالتزام، تثبيت دبوس السياسة وتقديم واجهة برمجة التطبيقات لـ Torii،
المقاطع الموسيقية والمنسقين. هذه الوثيقة تتضمن خطة التحقق من صحة الخرسانة
تحقيق المزيد, تعزيز المنطق على السلسلة, خدمات المضيفين,
التركيبات والتشغيل.

## Область1. **التسجيل الأساسي للجهاز**: اكتب Norito للبيانات والأسماء المستعارة،
   المزايا العامة، عصر الحداثة والإدارة التحويلية.
2. **عقد التحقق**: تحديد عمليات CRUD للحياة
   دبوس цикла (`ReplicationOrder`، `Precommit`، `Completion`، الإخلاء).
3. **الأسلوب الخدمي**: نقاط نهاية gRPC/REST، وتشغيل التسجيل، و
   يتضمن Torii وSDK، بما في ذلك الصفحات والشهادة.
4. **الأدوات والتركيبات**: مساعدو واجهة سطر الأوامر (CLI) ومتجهات الاختبارات والوثائق
   بيانات المزامنة والأسماء المستعارة ومظاريف الإدارة.
5. **قياس المسافة والعمليات**: المقاييس والتنبيهات وسجلات التشغيل لإغلاق التسجيل.

## نموذج البيانات

### السجل الأساسي (Norito)| الهيكل | الوصف | بوليا |
|----------|---------|------|
| `PinManifestRecord` | Chain-authoritative manifest lifecycle entry. The envelope digest and exact 36-byte CIDv1/dag-cbor/BLAKE3-256 content root are distinct commitments. | `digest`, `root_cid`, `chunker`, `chunk_digest_sha3_256`, `por_root`, `content_length`, `policy`, `submitted_by`, `submitted_epoch`, `alias`, `successor_of`, `metadata`, `status`, `retirement_reason`, `council_envelope_digest`, `pin_fee_payment`. |
| `PinManifestFinalizedRecordV1` | Immutable read result binding one native manifest record to the finalized block used for the query. | `finalized_cursor` (`height`, `block_hash`), `manifest`. |
| `AliasBindingV1` | Сопоставляет الاسم المستعار -> بيان إدارة البحث الجنائي. | `alias`، `manifest_cid`، `bound_at`، `expiry_epoch`. |
| `ReplicationOrderV1` | تعليمات لمقدمي الخدمة لفك البيان. | `order_id`، `manifest_cid`، `providers`، `redundancy`، `deadline`، `policy_hash`. |
| `ReplicationReceiptV1` | مقدم الطلب. | `order_id`، `provider_id`، `status`، `timestamp`، `por_sample_digest`. |
| `ManifestPolicyV1` | الإدارة السياسية البسيطة. | `min_replicas`، `max_retention_epochs`، `allowed_profiles`، `pin_fee_basis_points`. |

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

## المباريات وCI- تركيبات الكتالوج: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` charnit подписанные snapshots Manifest/alias/order، يتم تحويلها من خلال `cargo run -p iroha_core --example gen_pin_snapshot`.
- CI CI: `ci/check_sorafs_fixtures.sh` ينقل لقطة ويلتقط الاختلافات، ويدير تزامن تركيبات CI.
- اختبارات التكامل (`crates/iroha_core/tests/pin_registry.rs`) تظهر المسار السعيد بالإضافة إلى الخروج عند الاسم المستعار للدبلجة، الاسم المستعار للموافقة/الشرف، غير قابل للتخصيص مقابض القطع، والتحقق من نسخة طبق الأصل وإخراج الحراس المتفوقين (غير معروف/متاح/متنوع/ساموسيلكي)؛ سم. اضغط على `register_manifest_rejects_*` لتفاصيل الشاشة.
- تقوم الاختبارات بفحص التحقق من الاسم المستعار وحرس الحراس والتحقق من صحة الاسم المستعار؛ هناك العديد من الإخطارات التي تشير إلى أن الماكينة جاهزة تمامًا.
- Golden JSON للأشخاص الذين يستخدمون أجهزة الكمبيوتر الشخصية.

## القياس عن بعد والمراقبة

المقاييس (Prometheus):
-`torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
-`torii_sorafs_registry_aliases_total`
-`torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
-`torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
-`torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
-`torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- جهاز قياس الاتصال الخاص بموفر الخدمة (`torii_sorafs_capacity_*`، `torii_sorafs_fee_projection_nanos`) موجود في منطقة للوحة البيانات من طرف إلى طرف.

الشعارات:
- الهيكل الهيكلي للوحدة Norito لإدارة مدققي الحسابات (التقديم؟).

التنبيهات:
- بسبب النسخ المتماثل في الخدمة، SLA السابق.
- إنشاء اسم مستعار جديد.
- ضيق التنفس (البيان لا يمتد إلى النشأة).لوحة القيادة:
- Grafana JSON `docs/source/grafana_sorafs_pin_registry.json` يتتبع إجماليات بيانات الدورة الحيوية، والاسم المستعار للطباعة، والتراكم المتراكم، ونسبة SLA، وتراكب الكمون مقابل الركود، والتكلفة العبارات المقترحة للمراجعة عند الطلب.

## دفاتر التشغيل والوثائق

- قم بتثبيت `docs/source/sorafs/migration_ledger.md` لإدراج حالة التسجيل.
- مشغل التشغيل: `docs/source/sorafs/runbooks/pin_registry_ops.md` (متاح حاليًا) مزود بمقاييس وتنبيهات وتحديثات ونسخ احتياطي ودعم.
- Руководство по управлению: описать parameters politicies, Workflow одобения, обработку sporov.
- واجهة برمجة التطبيقات الخاصة بكل نقطة نهاية (Docusaurus docs).

## المعرفة والتقدم

1. الانتهاء من التحقق من صحة الخطة (التكامل ManifestValidator).
2. الانتهاء من النظام Norito + الإعدادات الافتراضية.
3. تحقيق العقد + الخدمة، إضافة إلى القياس عن بعد.
4. إعادة تنظيم التركيبات ودمج جناح التكامل.
5. قم بتنزيل المستندات/دفاتر التشغيل وحذف النقاط الدقيقة في خارطة الطريق بشكل أفضل.

يتم اختيار كل نقطة من نقاط SF-4 من خلال هذه الخطة من خلال التقدم المحرز.
- `GET /v1/sorafs/pin` returns the attested manifest catalogue.
- `GET /v1/sorafs/pin/{digest_hex}` returns exact `PinManifestFinalizedRecordV1` JSON with `finalized_cursor.height`, `finalized_cursor.block_hash`, and native `manifest`.
- `GET /v1/sorafs/aliases` و `GET /v1/sorafs/replication` نشاط النشر
  الاسم المستعار للكتالوج والمتراكمة بسبب النسخ المتماثل مع صفحات متسقة و
  حالة الترشيح.

تستجيب سطر الأوامر لهذا الصوت (`iroha app sorafs pin list`، `pin show`، `alias list`،
`replication list`)، لكي يتمكن المشغلون من أتمتة تسجيل التدقيق
بدون الخضوع لواجهة برمجة التطبيقات غير المرغوب فيها.
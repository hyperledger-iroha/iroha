---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/pin-registry-plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: خطة التسجيل
العنوان: SoraFS Pin Registry نفاذی منصوبہ
Sidebar_label: Pin Registry منصوبہ
الوصف: نفاذية SF-4 منصوبة لآلة تسجيل الحالة، واجهة Torii، والأدوات وإمكانية المراقبة.
---

:::ملاحظة مستند ماخذ
هذه هي الصفحة `docs/source/sorafs/pin_registry_plan.md`. عندما تتاح لك إمكانية الوصول إلى جهات الاتصال الفعالة، يمكنك الاتصال بنا.
:::

# SoraFS Pin Registry نفاذی منصوبہ (SF-4)

SF-4 Pin Registry ورابط التسجيل يوضح الالتزامات الواضحة المهمة،
تظهر سياسات الدبوس بشكل فعال، وTorii، والبوابات والمنسقين لواجهات برمجة التطبيقات.
لقد وضعت خطة التحقق من الصحة والتي تتضمن مهام التنفيذ بشكل فعال، وهي عبارة عن منطق متصل بالسلسلة،
تشمل خدمات الجانب المضيف والتركيبات والعمليات المطلوبة.

##ديرہ کار1. **جهاز حالة التسجيل**: السجلات المحددة بواسطة Norito تظهر البيانات، الأسماء المستعارة، السلاسل اللاحقة،
   فترات الاحتفاظ، والبيانات الوصفية للحوكمة.
2. **شبكة نفاذ**: دورة حياة الدبوس لعمليات CRUD الحتمية (`ReplicationOrder`،
   `Precommit`، `Completion`، الإخلاء).
3. **الواجهة الخارجية**: يتم استخدام تسجيل نقاط نهاية gRPC/REST وTorii وSDK،
   إن ترقيم الصفحات والتصديق يشملان كل شيء.
4. **الأدوات والتركيبات**: مساعدو واجهة سطر الأوامر (CLI)، وموجهات الاختبار، والوثائق، والبيانات، والأسماء المستعارة،
   مظاريف الحوكمة مظاريف الحكم.
5. **القياس عن بعد والعمليات**: سجل صحيح للمقاييس والتنبيهات ودفاتر التشغيل.

## ڈیٹا ماڈ

### دفتر ريكارز (Norito)| هيكل | وضاحت | فيليز |
|--------|-------|-------|
| `PinManifestRecord` | Chain-authoritative manifest lifecycle entry. The envelope digest and exact 36-byte CIDv1/dag-cbor/BLAKE3-256 content root are distinct commitments. | `digest`, `root_cid`, `chunker`, `chunk_digest_sha3_256`, `por_root`, `content_length`, `policy`, `submitted_by`, `submitted_epoch`, `alias`, `successor_of`, `metadata`, `status`, `retirement_reason`, `council_envelope_digest`, `pin_fee_payment`. |
| `PinManifestFinalizedRecordV1` | Immutable read result binding one native manifest record to the finalized block used for the query. | `finalized_cursor` (`height`, `block_hash`), `manifest`. |
| `AliasBindingV1` | الاسم المستعار -> تعيين CID الظاهر. | `alias`، `manifest_cid`، `bound_at`، `expiry_epoch`. |
| `ReplicationOrderV1` | يقوم مقدمو الخدمة بإظهار رقم التعريف الشخصي. | `order_id`، `manifest_cid`، `providers`، `redundancy`، `deadline`، `policy_hash`. |
| `ReplicationReceiptV1` | إقرار المزود. | `order_id`، `provider_id`، `status`، `timestamp`، `por_sample_digest`. |
| `ManifestPolicyV1` | صورة لسياسة الحوكمة. | `min_replicas`، `max_retention_epochs`، `allowed_profiles`، `pin_fee_basis_points`. |

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

## المباريات وCI- دليل التركيبات: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` يحتوي على البيان الموقع/الاسم المستعار/لقطات الطلب.
- خطوة CI: `ci/check_sorafs_fixtures.sh` لقطة إعادة إنشاء لقطة ہے وفرق ہونے پر فشل کرتا ہے تى CI تركيبات محاذاة.
- اختبارات التكامل (`crates/iroha_core/tests/pin_registry.rs`) المسار السعيد رفض الاسم المستعار المكرر، حراس الموافقة/الاحتفاظ بالاسم المستعار، مقابض القطع غير المتطابقة، التحقق من صحة عدد النسخ المتماثلة، وفشل حماية التعاقب (غير معروف/موافق عليه مسبقًا/متقاعد/مؤشرات ذاتية) وصف لحالات `register_manifest_rejects_*`.
- اختبارات الوحدة اب `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` تتضمن التحقق من صحة الاسم المستعار، ووحدات حماية الاحتفاظ، والفحوصات اللاحقة؛ اكتشاف التتابع متعدد القفزات
- خطوط أنابيب إمكانية المراقبة لأحداث JSON الذهبية۔

## القياس عن بعد وإمكانية الملاحظة

المقاييس (Prometheus):
-`torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
-`torii_sorafs_registry_aliases_total`
-`torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
-`torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
-`torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
-`torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- القياس عن بعد الخاص بموفر الخدمة (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) موجود من خلال لوحات معلومات شاملة للنطاق.

السجلات:
- عمليات تدقيق الإدارة کے لیے تدفق حدث Norito منظم (موقع؟).

التنبيهات:
- SLA عدد كبير من أوامر النسخ المتماثل المعلقة.
- عتبة انتهاء الصلاحية الاسم المستعار سے کم.
- مخالفات الاحتفاظ (بيان التجديد وقت سے پہلے نہ ہو).لوحات المعلومات:
- Grafana JSON `docs/source/grafana_sorafs_pin_registry.json` إجماليات دورة حياة البيان، وتغطية الاسم المستعار، وتشبع الأعمال المتراكمة، ونسبة SLA، ووقت الاستجابة مقابل تراكبات الركود، ومعدلات الطلبات الفائتة ومراجعة عند الطلب.

## كتب التشغيل والوثائق

- `docs/source/sorafs/migration_ledger.md` تتضمن تحديثات حالة التسجيل معلومات أساسية.
- دليل المشغل: `docs/source/sorafs/runbooks/pin_registry_ops.md` (ابتشاع شاعہ) المقاييس، والتنبيه، والنشر، والنسخ الاحتياطي، وتدفقات الاسترداد.
- دليل الحوكمة: معايير السياسة، سير عمل الموافقة، التعامل مع النزاعات.
- الصفحات المرجعية لنقطة النهاية لواجهة برمجة التطبيقات (Docusaurus docs).

## التبعيات والتسلسل

1. مهام خطة التحقق من الصحة مکمل کریں (تكامل ManifestValidator).
2. مخطط Norito + إعدادات السياسة الافتراضية.
3. العقد + خدمة التمويل وسلك القياس عن بعد.
4. تعمل التركيبات على تجديد أجنحة التكامل والتكامل.
5. تعد المستندات/دفاتر التشغيل عناصر أساسية وخريطة الطريق للعلامة التجارية.

SF-4 هي قائمة مرجعية تم تصميمها خصيصًا للتحول إلى ما هو أبعد من ذلك.
واجهة REST هي نقاط نهاية القائمة المعتمدة:

- `GET /v1/sorafs/pin` returns the attested manifest catalogue.
- `GET /v1/sorafs/pin/{digest_hex}` returns exact `PinManifestFinalizedRecordV1` JSON with `finalized_cursor.height`, `finalized_cursor.block_hash`, and native `manifest`.
- `GET /v1/sorafs/aliases` و `GET /v1/sorafs/replication` كتالوج الاسم المستعار النشط و
  يتم الاحتفاظ بتراكم أوامر النسخ المتماثل مع ترقيم الصفحات المتسق ومرشحات الحالة.CLI تستدعي کو التفاف کرتی ہے (`iroha app sorafs pin list`، `pin show`، `alias list`،
`replication list`) يقوم المشغلون بواجهات برمجة التطبيقات (APIs) الشاملة بتجميع عمليات تدقيق التسجيل الخاصة بهم.
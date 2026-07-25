---
id: pin-registry-plan
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/pin-registry-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note المصدر المعتمد
تعكس هذه الصفحة `docs/source/sorafs/pin_registry_plan.md`. حافظ على النسختين متزامنتين ما دامت الوثائق القديمة نشطة.
:::

# خطة تنفيذ Pin Registry في SoraFS (SF-4)

يقدّم SF-4 عقد Pin Registry والخدمات المساندة التي تخزن التزامات manifest،
وتفرض سياسات pin، وتكشف واجهات API لـ Torii والبوابات وادوات التنسيق.
يوسّع هذا المستند خطة التحقق بمهام تنفيذية ملموسة تغطي المنطق on-chain،
وخدمات المضيف، والـ fixtures، والمتطلبات التشغيلية.

## النطاق

1. **آلة حالات registry**: سجلات Norito للـ manifests والـ aliases وسلاسل الخلفاء
   وعصور الاحتفاظ وبيانات الحوكمة الوصفية.
2. **تنفيذ العقد**: عمليات CRUD حتمية لدورة حياة pin (`ReplicationOrder`, `Precommit`,
   `Completion`, eviction).
3. **واجهة الخدمة**: نقاط نهاية gRPC/REST مدعومة بالـ registry تستهلكها Torii وSDKs،
   وتشمل الترقيم والاتستاشن.
4. **التولنغ والـ fixtures**: مساعدات CLI ومتجهات اختبار ووثائق تحافظ على تزامن
   manifests وaliases وenvelopes الخاصة بالحوكمة.
5. **التليمتري وعمليات التشغيل**: مقاييس وتنبيهات وrunbooks لصحة registry.

## نموذج البيانات

### السجلات الاساسية (Norito)

| البنية | الوصف | الحقول |
|--------|-------|--------|
| `PinManifestRecord` | Chain-authoritative manifest lifecycle entry. The envelope digest and exact 36-byte CIDv1/dag-cbor/BLAKE3-256 content root are distinct commitments. | `digest`, `root_cid`, `chunker`, `chunk_digest_sha3_256`, `por_root`, `content_length`, `policy`, `submitted_by`, `submitted_epoch`, `alias`, `successor_of`, `metadata`, `status`, `retirement_reason`, `council_envelope_digest`, `pin_fee_payment`. |
| `PinManifestFinalizedRecordV1` | Immutable read result binding one native manifest record to the finalized block used for the query. | `finalized_cursor` (`height`, `block_hash`), `manifest`. |
| `AliasBindingV1` | ربط alias -> CID الخاص بالـ manifest. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | تعليمات للمزوّدين لتثبيت manifest. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | اقرار المزوّد. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | لقطة سياسة الحوكمة. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

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

## Fixtures و CI

- دليل fixtures: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` يخزن لقطات موقعة لـ manifest/alias/order يعاد توليدها عبر `cargo run -p iroha_core --example gen_pin_snapshot`.
- خطوة CI: `ci/check_sorafs_fixtures.sh` تعيد توليد اللقطة وتفشل عند وجود اختلافات، لتحافظ على تماهي fixtures الخاصة بـ CI.
- اختبارات التكامل (`crates/iroha_core/tests/pin_registry.rs`) تغطي المسار السعيد مع رفض alias المكرر، وحمايات اعتماد/احتفاظ alias، وhandles غير متطابقة لـ chunker، والتحقق من عدد النسخ، وفشل حمايات التعاقب (مؤشرات مجهولة/موافَق عليها مسبقا/مسحوبة/ذاتية الاشارة)؛ راجع حالات `register_manifest_rejects_*` لتفاصيل التغطية.
- اختبارات الوحدة تغطي الان التحقق من alias وحمايات الاحتفاظ وفحوصات الخلف في `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`؛ كشف التعاقب متعدد القفزات عند وصول آلة الحالات.
- JSON ذهبي للاحداث المستخدمة في خطوط مراقبة الرصد.

## التليمتري والرصد

المقاييس (Prometheus):
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- تليمتري المزود الحالي (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) تبقى ضمن النطاق للوحـات end-to-end.

السجلات:
- تيار احداث Norito منظم لتدقيقات الحوكمة (موقع؟).

التنبيهات:
- اوامر تكرار معلقة تتجاوز SLA.
- انتهاء صلاحية alias اقل من العتبة.
- مخالفات الاحتفاظ (manifest لم يجدد قبل الانتهاء).

لوحات المعلومات:
- ملف Grafana JSON `docs/source/grafana_sorafs_pin_registry.json` يتتبع اجمالي دورة حياة manifests، تغطية aliases، تشبع backlog، نسبة SLA، تراكب latency مقابل slack، ومعدلات الاوامر الفاشلة للمراجعة اثناء النوبة.

## Runbooks والوثائق

- تحديث `docs/source/sorafs/migration_ledger.md` لتضمين تحديثات حالة registry.
- دليل المشغل: `docs/source/sorafs/runbooks/pin_registry_ops.md` (منشور حاليا) يغطي المقاييس والتنبيه والنشر والنسخ الاحتياطي واستعادة الخدمة.
- دليل الحوكمة: وصف معلمات السياسة وسير عمل الاعتماد ومعالجة النزاعات.
- صفحات مرجع API لكل نقطة نهاية (وثائق Docusaurus).

## الاعتماديات والتسلسل

1. اكمال مهام خطة التحقق (دمج ManifestValidator).
2. انهاء مخطط Norito + قيم السياسة الافتراضية.
3. تنفيذ العقد + الخدمة وربط التليمتري.
4. اعادة توليد fixtures وتشغيل اختبارات التكامل.
5. تحديث الوثائق/runbooks ووضع علامة اكتمال على عناصر خارطة الطريق.

يجب ان تشير كل قائمة تحقق ضمن SF-4 الى هذه الخطة عند تسجيل التقدم.
واجهة REST توفر الان نقاط نهاية قائمة مع اتستاشن:

- `GET /v1/sorafs/pin` returns the attested manifest catalogue.
- `GET /v1/sorafs/pin/{digest_hex}` returns exact `PinManifestFinalizedRecordV1` JSON with `finalized_cursor.height`, `finalized_cursor.block_hash`, and native `manifest`.
- `GET /v1/sorafs/aliases` و `GET /v1/sorafs/replication` تكشفان كتالوج alias
  النشط وتراكم اوامر التكرار بترقيم ثابت ومرشحات حالة.

تغلف CLI هذه الاستدعاءات (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) حتى يتمكن المشغلون من اتمتة تدقيقات registry بدون لمس
واجهات API منخفضة المستوى.

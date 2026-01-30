---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: pin-registry-plan
title: خطة تنفيذ Pin Registry في SoraFS
sidebar_label: خطة Pin Registry
description: خطة تنفيذ SF-4 التي تغطي آلة الحالات للـ registry وواجهة Torii والتولنغ والرصد.
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
| `PinRecordV1` | مدخل manifest كنسي. | `manifest_cid`, `chunk_plan_digest`, `por_root`, `profile_handle`, `approved_at`, `retention_epoch`, `pin_policy`, `successor_of`, `governance_envelope_hash`. |
| `AliasBindingV1` | ربط alias -> CID الخاص بالـ manifest. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | تعليمات للمزوّدين لتثبيت manifest. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | اقرار المزوّد. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | لقطة سياسة الحوكمة. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

مرجع التنفيذ: راجع `crates/sorafs_manifest/src/pin_registry.rs` لمخططات Norito في Rust
ومساعدات التحقق التي تدعم هذه السجلات. التحقق يعكس tooling الخاص بالـ manifest
(lookup لــ chunker registry وpin policy gating) لضمان ان العقد وواجهات Torii وCLI
تتقاسم نفس invariants.

المهام:
- انهاء مخططات Norito في `crates/sorafs_manifest/src/pin_registry.rs`.
- توليد الشفرة (Rust + SDKs اخرى) باستخدام ماكرو Norito.
- تحديث الوثائق (`sorafs_architecture_rfc.md`) بعد تثبيت المخططات.

## تنفيذ العقد

| المهمة | المالك/المالكون | الملاحظات |
|--------|------------------|-----------|
| تنفيذ تخزين registry (sled/sqlite/off-chain) او وحدة smart contract. | Core Infra / Smart Contract Team | توفير hashing حتمي وتجنب الفاصلة العائمة. |
| نقاط الدخول: `submit_manifest`, `approve_manifest`, `bind_alias`, `issue_replication_order`, `complete_replication`, `evict_manifest`. | Core Infra | استخدام `ManifestValidator` من خطة التحقق. ربط alias يمر الان عبر `RegisterPinManifest` (DTO من Torii) بينما يبقى `bind_alias` المخصص مخططا لتحديثات لاحقة. |
| انتقالات الحالة: فرض التعاقب (manifest A -> B)، عصور الاحتفاظ، تفرد alias. | Governance Council / Core Infra | تفرد alias وحدود الاحتفاظ وفحوصات اعتماد/سحب السلف موجودة في `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`؛ كشف التعاقب متعدد القفزات ودفاتر التكرار ما زالت مفتوحة. |
| المعلمات المحكومة: تحميل `ManifestPolicyV1` من config/حالة الحوكمة؛ السماح بالتحديث عبر احداث الحوكمة. | Governance Council | توفير CLI لتحديثات السياسة. |
| اصدار الاحداث: اصدار احداث Norito للتليمتري (`ManifestApproved`, `ReplicationOrderIssued`, `AliasBound`). | Observability | تعريف مخطط الاحداث + التسجيل. |

الاختبارات:
- اختبارات وحدة لكل نقطة دخول (ايجابي + رفض).
- اختبارات خصائص لسلسلة التعاقب (بدون دورات، عصور متصاعدة).
- Fuzz للتحقق عبر توليد manifests عشوائية (مقيدة).

## واجهة الخدمة (تكامل Torii/SDK)

| المكون | المهمة | المالك/المالكون |
|--------|--------|------------------|
| خدمة Torii | كشف `/v1/sorafs/pin` (submit)، `/v1/sorafs/pin/{cid}` (lookup)، `/v1/sorafs/aliases` (list/bind)، `/v1/sorafs/replication` (orders/receipts). توفير ترقيم + ترشيح. | Networking TL / Core Infra |
| الاتستاشن | تضمين ارتفاع/هاش registry في الاستجابات؛ اضافة بنية Norito للاتستاشن تستهلكها SDKs. | Core Infra |
| CLI | توسيع `sorafs_manifest_stub` او CLI جديد `sorafs_pin` مع `pin submit`, `alias bind`, `order issue`, `registry export`. | Tooling WG |
| SDK | توليد bindings للعميل (Rust/Go/TS) من مخطط Norito؛ اضافة اختبارات تكامل. | SDK Teams |

العمليات:
- اضافة طبقة cache/ETag لنقاط نهاية GET.
- توفير rate limiting / auth بما يتوافق مع سياسات Torii.

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

- `GET /v1/sorafs/pin` و `GET /v1/sorafs/pin/{digest}` تعيدان manifests مع
  ربط aliases واوامر التكرار وكائن اتستاشن مشتق من هاش اخر كتلة.
- `GET /v1/sorafs/aliases` و `GET /v1/sorafs/replication` تكشفان كتالوج alias
  النشط وتراكم اوامر التكرار بترقيم ثابت ومرشحات حالة.

تغلف CLI هذه الاستدعاءات (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) حتى يتمكن المشغلون من اتمتة تدقيقات registry بدون لمس
واجهات API منخفضة المستوى.

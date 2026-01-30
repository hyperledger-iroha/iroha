---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/node-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: node-plan
title: خطة تنفيذ عقدة SoraFS
sidebar_label: خطة تنفيذ العقدة
description: تحويل خارطة طريق تخزين SF-3 إلى عمل هندسي قابل للتنفيذ مع معالم ومهام وتغطية اختبارات.
---

:::note المصدر المعتمد
تعكس هذه الصفحة `docs/source/sorafs/sorafs_node_plan.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم إيقاف وثائق Sphinx القديمة.
:::

تقدم SF-3 أول crate قابل للتشغيل باسم `sorafs-node` يحول عملية Iroha/Torii إلى موفر تخزين SoraFS. استخدم هذه الخطة بجانب [دليل تخزين العقدة](node-storage.md)، و[سياسة قبول الموفّرين](provider-admission-policy.md)، و[خارطة طريق سوق سعة التخزين](storage-capacity-marketplace.md) عند ترتيب التسليمات.

## النطاق المستهدف (المرحلة M1)

1. **تكامل مخزن القطع.** تغليف `sorafs_car::ChunkStore` بواجهة خلفية دائمة تخزن بايتات القطع وملفات manifest وأشجار PoR في مجلد البيانات المهيأ.
2. **نقاط نهاية البوابة.** توفير نقاط نهاية HTTP لـ Norito لإرسال pin وجلب القطع وأخذ عينات PoR وتليمترية التخزين ضمن عملية Torii.
3. **توصيل الإعدادات.** إضافة بنية إعداد `SoraFsStorage` (مفتاح التفعيل، السعة، المجلدات، حدود التوازي) وتمريرها عبر `iroha_config` و`iroha_core` و`iroha_torii`.
4. **الحصص/الجدولة.** فرض حدود القرص/التوازي التي يحددها المشغل ووضع الطلبات في طوابير مع back-pressure.
5. **التليمترية.** إصدار مقاييس/سجلات لنجاح pin وزمن جلب القطع واستغلال السعة ونتائج عينات PoR.

## تفصيل العمل

### A. بنية الـ crate والوحدات

| المهمة | المالك | الملاحظات |
|------|--------|-----------|
| إنشاء `crates/sorafs_node` مع الوحدات: `config` و`store` و`gateway` و`scheduler` و`telemetry`. | فريق التخزين | إعادة تصدير الأنواع القابلة لإعادة الاستخدام لدمجها مع Torii. |
| تنفيذ `StorageConfig` المشتق من `SoraFsStorage` (user → actual → defaults). | فريق التخزين / Config WG | ضمان بقاء طبقات Norito/`iroha_config` حتمية. |
| توفير واجهة `NodeHandle` يستخدمها Torii لإرسال pins/fetches. | فريق التخزين | تغليف تفاصيل التخزين والتوصيلات غير المتزامنة. |

### B. مخزن قطع دائم

| المهمة | المالك | الملاحظات |
|------|--------|-----------|
| بناء واجهة خلفية على القرص تغلف `sorafs_car::ChunkStore` مع فهرس manifest على القرص (`sled`/`sqlite`). | فريق التخزين | تخطيط حتمي: `<data_dir>/<manifest_cid>/chunk_{idx}.bin`. |
| الحفاظ على بيانات PoR الوصفية (أشجار 64 KiB/4 KiB) باستخدام `ChunkStore::sample_leaves`. | فريق التخزين | يدعم إعادة التشغيل؛ يفشل بسرعة عند التلف. |
| تنفيذ إعادة فحص السلامة عند البدء (إعادة تجزئة manifests وحذف pins غير المكتملة). | فريق التخزين | يمنع بدء Torii حتى اكتمال إعادة الفحص. |

### C. نقاط نهاية البوابة

| نقطة النهاية | السلوك | المهام |
|-------------|---------|-------|
| `POST /sorafs/pin` | قبول `PinProposalV1` والتحقق من manifests ووضع الإدخال في الطابور والرد بـ CID الخاص بالـ manifest. | التحقق من ملف تعريف الـ chunker وفرض الحصص وبث البيانات عبر مخزن القطع. |
| `GET /sorafs/chunks/{cid}` + استعلام range | تقديم بايتات القطع مع ترويسات `Content-Chunker` واحترام مواصفة نطاق القدرات. | استخدام المجدول مع ميزانيات البث (ربطها بقدرات النطاق SF-2d). |
| `POST /sorafs/por/sample` | تنفيذ أخذ عينات PoR لملف manifest وإرجاع حزمة إثبات. | إعادة استخدام أخذ العينات من مخزن القطع والرد عبر Norito JSON. |
| `GET /sorafs/telemetry` | ملخصات: السعة ونجاح PoR وعدّادات أخطاء fetch. | توفير البيانات للوحة المراقبة/المشغلين. |

تقوم الوصلات في وقت التشغيل بتمرير تفاعلات PoR عبر `sorafs_node::por`، حيث يسجل المتتبع كل `PorChallengeV1` و`PorProofV1` و`AuditVerdictV1` لكي تعكس مقاييس `CapacityMeter` أحكام الحوكمة من دون منطق Torii مخصص.【crates/sorafs_node/src/scheduler.rs#L147】

ملاحظات تنفيذية:

- استخدم مكدس Axum الخاص بـ Torii مع حمولات `norito::json`.
- أضف مخططات Norito للاستجابات (`PinResultV1` و`FetchErrorV1` وبنى التليمترية).

- ✅ أصبح المسار `/v1/sorafs/por/ingestion/{manifest_digest_hex}` يعرض عمق الـ backlog وأقدم epoch/deadline وأحدث طوابع النجاح/الفشل لكل مزود، عبر `sorafs_node::NodeHandle::por_ingestion_status`، وتسجل Torii عدادات `torii_sorafs_por_ingest_backlog`/`torii_sorafs_por_ingest_failures_total` للّوحات.【crates/sorafs_node/src/lib.rs:510】【crates/iroha_torii/src/sorafs/api.rs:1883】【crates/iroha_torii/src/routing.rs:7244】【crates/iroha_telemetry/src/metrics.rs:5390】

### D. المجدول وفرض الحصص

| المهمة | التفاصيل |
|------|----------|
| حصة القرص | تتبع البايتات على القرص؛ رفض pins الجديدة عند تجاوز `max_capacity_bytes`. توفير نقاط ربط لسياسات الإخلاء المستقبلية. |
| توازي fetch | شبهور عام (`max_parallel_fetches`) مع ميزانيات لكل مزود من حدود نطاق SF-2d. |
| طابور pins | تحديد عدد مهام الإدخال المعلقة؛ توفير نقاط حالة Norito لعمق الطابور. |
| وتيرة PoR | عامل خلفي يعمل وفق `por_sample_interval_secs`. |

### E. التليمترية والسجلات

المقاييس (Prometheus):

- `sorafs_pin_success_total`, `sorafs_pin_failure_total`
- `sorafs_chunk_fetch_duration_seconds` (هيستوغرام مع وسوم `result`)
- `torii_sorafs_storage_bytes_used`, `torii_sorafs_storage_bytes_capacity`
- `torii_sorafs_storage_pin_queue_depth`, `torii_sorafs_storage_fetch_inflight`
- `torii_sorafs_storage_fetch_bytes_per_sec`
- `torii_sorafs_storage_por_inflight`
- `torii_sorafs_storage_por_samples_success_total`, `torii_sorafs_storage_por_samples_failed_total`

السجلات / الأحداث:

- تليمترية Norito منظمة لعمليات الحوكمة (`StorageTelemetryV1`).
- تنبيهات عند تجاوز الاستغلال 90% أو عندما تتخطى سلسلة إخفاقات PoR العتبة.

### F. استراتيجية الاختبارات

1. **اختبارات وحدات.** ديمومة مخزن القطع، حسابات الحصة، ثوابت المجدول (انظر `crates/sorafs_node/src/scheduler.rs`).
2. **اختبارات تكامل** (`crates/sorafs_node/tests`). دورة pin → fetch، الاستعادة بعد إعادة التشغيل، رفض الحصص، والتحقق من إثباتات أخذ عينات PoR.
3. **اختبارات تكامل Torii.** تشغيل Torii مع تفعيل التخزين وتجربة نقاط النهاية HTTP عبر `assert_cmd`.
4. **خارطة طريق الفوضى.** تدريبات مستقبلية تحاكي نفاد القرص، بطء IO، وإزالة الموفّرين.

## التبعيات

- سياسة قبول SF-2b — التأكد من أن العقد تتحقق من أظرف القبول قبل الإعلان.
- سوق السعة SF-2c — ربط التليمترية بإعلانات السعة.
- امتدادات advert لـ SF-2d — استهلاك قدرة النطاق + ميزانيات البث عند توفرها.

## معايير إغلاق المرحلة

- `cargo run -p sorafs_node --example pin_fetch` يعمل مع fixtures محلية.
- بناء Torii مع `--features sorafs-storage` واجتياز اختبارات التكامل.
- تحديث الوثائق ([دليل تخزين العقدة](node-storage.md)) مع افتراضيات الإعداد وأمثلة CLI؛ وتوفر runbook للمشغلين.
- ظهور التليمترية في لوحات staging وضبط التنبيهات لتشبع السعة وإخفاقات PoR.

## مخرجات الوثائق والعمليات

- تحديث [مرجع تخزين العقدة](node-storage.md) مع افتراضيات الإعداد، استخدام CLI، وخطوات الاستكشاف.
- إبقاء [runbook عمليات العقدة](node-operations.md) متوافقا مع التنفيذ مع تطور SF-3.
- نشر مراجع API لنقاط النهاية `/sorafs/*` داخل بوابة المطورين وربطها بملف OpenAPI عند وصول معالجات Torii.

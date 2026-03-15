---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/node-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: خطة العقدة
العنوان: خطة تنفيذ العقدة SoraFS
Sidebar_label: خطة تنفيذ العقدة
الوصف: تحويل طريق التخزين SF-3 إلى عمل إبداعي قابل للتنفيذ باستخدام الضربات والحقائب والتغطية التجريبية.
---

:::ملاحظة فوينتي كانونيكا
هذه الصفحة تعكس `docs/source/sorafs/sorafs_node_plan.md`. استمر في العمل على النسخ المتزامنة حتى يتم سحب الوثائق المتوارثة من Sphinx.
:::

يقوم SF-3 بإدخال الصندوق الأولي القابل للتنفيذ `sorafs-node` والذي يحول عملية Iroha/Torii إلى مزود تخزين SoraFS. يتم استخدام هذه الخطة جنبًا إلى جنب مع [دليل العقدة](node-storage.md) و[سياسة قبول الموردين](provider-admission-policy.md) و[طريق سوق قدرة التخزين](storage-capacity-marketplace.md) entregables secuenciar.

## ألكانس أوبجيتيفو (هيتو M1)1. **تكامل تخزين القطع.** قم بإرفاق `sorafs_car::ChunkStore` بواجهة خلفية مستمرة لحماية بايتات القطعة والبيانات والأعمدة في دليل البيانات الذي تم تكوينه.
2. **نقاط النهاية للبوابة.** تعرض نقاط النهاية HTTP Norito لإرسال الدبابيس وجلب القطع وكثافة البيانات والقياس عن بعد أثناء العملية Torii.
3. **تهيئة التكوين.** قم بإضافة بنية التكوين `SoraFsStorage` (إشارة إلى المؤهلات والسعة والأدلة وحدود التزامن) عبر `iroha_config` و`iroha_core` و `iroha_torii`.
4. **الإشعارات/التخطيط.** تمكين حدود القرص/التوازي التي يحددها المشغل وإضافة الطلبات بالضغط الخلفي.
5. **القياس عن بعد.** قم بإصدار قياسات/سجلات لنجاح الدبابيس، وتأخر جلب القطع، واستخدام السعة، ونتائج قوة التحمل.

## تفكيك العمل

### أ. هيكلة الصناديق والوحدات| تاريا | المسؤول (المسؤولون) | نوتاس |
|------|-------------|------|
| إنشاء `crates/sorafs_node` مع الوحدات: `config`، `store`، `gateway`، `scheduler`، `telemetry`. | معدات التخزين | قم بإعادة تصدير الأنواع القابلة لإعادة الاستخدام للتكامل مع Torii. |
| قم بتنفيذ `StorageConfig` Mapeado من `SoraFsStorage` (المستخدم → الحقيقي → الافتراضي). | معدات التخزين / تكوين WG | Asegura que las capas Norito/`iroha_config` يمكن تحديدها. |
| قم بإثبات وجود `NodeHandle` بحيث يستخدم Torii لإرسال الدبابيس/الجلبات. | معدات التخزين | Encapsula internos de almacenamiento y plomería async. |

### ب. قطع القطع المستمرة

| تاريا | المسؤول (المسؤولون) | نوتاس |
|------|-------------|------|
| أنشئ واجهة خلفية على قرص ديسكو لتضمين `sorafs_car::ChunkStore` بمؤشر بيانات على قرص (`sled`/`sqlite`). | معدات التخزين | تحديد التخطيط: `<data_dir>/<manifest_cid>/chunk_{idx}.bin`. |
| الحفاظ على بيانات التعريف PoR (حجم 64 كيلو بايت/4 كيلو بايت) باستخدام `ChunkStore::sample_leaves`. | معدات التخزين | Soporta replay tras reiniicios؛ يبدأ الأمر بسرعة في مكافحة الفساد. |
| تنفيذ إعادة التشغيل المتكامل في البداية (إعادة صياغة البيانات، وضع الدبابيس غير المكتملة). | معدات التخزين | قم بحظر ترتيب Torii حتى يتم إكمال إعادة التشغيل. |

### ج. نقاط النهاية للبوابة| نقطة النهاية | التوافق | تاريس |
|----------|----------------|--------|
| `POST /sorafs/pin` | Acepta `PinProposalV1`، بيانات صالحة، مضمنة في العرض، مستجيبة لمعرف البحث الجنائي (CID) للبيان. | التحقق من ملف التعريف الخاص بالقطعة، وطلب القطع، وبث البيانات عبر متجر القطعة. |
| `GET /sorafs/chunks/{cid}` + استشارة حول النطاق | إرسال وحدات البايت مع الرؤوس `Content-Chunker`؛ مع مراعاة مواصفات سعة المدى. | برنامج جدولة أمريكي + إعدادات البث (على طول نطاق سعة SF-2d). |
| `POST /sorafs/por/sample` | قم بتنفيذ عملية الدفع لبيان واحد وقم بتمرير حزمة الاختبارات. | إعادة استخدام مخزن القطعة، والاستجابة للحمولات Norito JSON. |
| `GET /sorafs/telemetry` | الملخصات: القدرة، نجاح PoR، بيانات أخطاء الجلب. | نسبة البيانات إلى لوحات المعلومات/المشغلين. |

يؤدي التداخل في وقت التشغيل إلى تنشيط التفاعلات من خلال `sorafs_node::por`: يسجل المتتبع كلاً من `PorChallengeV1` و`PorProofV1` و`AuditVerdictV1` بحيث تعكس مقاييس `CapacityMeter` قرارات الإدارة بدون منطق Torii تخصيص.[crates/sorafs_node/src/scheduler.rs#L147]

ملاحظات التنفيذ:

- يستخدم المكدس Axum de Torii مع الحمولات النافعة `norito::json`.
- قم بإضافة Norito للإجابة (`PinResultV1`, `FetchErrorV1`, هياكل القياس عن بعد).- ✅ `/v2/sorafs/por/ingestion/{manifest_digest_hex}` يعرض الآن عمق التراكم في العصر/الحد الأقدم والطوابع الزمنية للنجاح/السقوط الأحدث من قبل المورِّد، الدافع من خلال `sorafs_node::NodeHandle::por_ingestion_status`، وTorii تسجيل المقاييس `torii_sorafs_por_ingest_backlog`/`torii_sorafs_por_ingest_failures_total` الفقرة لوحات المعلومات.[crates/sorafs_node/src/lib.rs:510][crates/iroha_torii/src/sorafs/api.rs:1883][crates/iroha_torii/src/routing.rs:7244][crates/iroha_telemetry/src/metrics.rs:5390]

### د. المجدول والوفاء بالطلبات

| تاريا | تفاصيل |
|------|----------|
| قطع الديسكو | تتبع البايتات على الديسكو; إعادة دبابيس جديدة فائقة `max_capacity_bytes`. إثبات خطاف الطرد للسياسة المستقبلية. |
| تزامن الجلب | العلامة العالمية (`max_parallel_fetches`) هي أكثر المتطلبات التي سيحصل عليها المورّد من حدود النطاق SF-2d. |
| كولا دي بينس | الحد من أعمال الاستيعاب المتوقعة؛ عرض نقاط النهاية Norito لحالة عمق الكولا. |
| كادينسيا بور | عامل في خطة نبضية ثانية لـ `por_sample_interval_secs`. |

### هـ. القياس عن بعد وتسجيل الدخول

المقاييس (Prometheus):

- `sorafs_pin_success_total`، `sorafs_pin_failure_total`
- `sorafs_chunk_fetch_duration_seconds` (المدرج التكراري مع العلامات `result`)
- `torii_sorafs_storage_bytes_used`، `torii_sorafs_storage_bytes_capacity`
- `torii_sorafs_storage_pin_queue_depth`، `torii_sorafs_storage_fetch_inflight`
-`torii_sorafs_storage_fetch_bytes_per_sec`
-`torii_sorafs_storage_por_inflight`
- `torii_sorafs_storage_por_samples_success_total`، `torii_sorafs_storage_por_samples_failed_total`

السجلات/الأحداث:- بنية القياس عن بعد Norito لاستيعاب الإدارة (`StorageTelemetryV1`).
- تنبيهات عند الاستخدام > 90% أو سقوط السقوط فوق الظل.

### واو استراتيجية الاختبار

1. **التجربة الوحدوية.** استمرارية تخزين القطع وحسابات القطع وجداول الجدولة الثابتة (الإصدار `crates/sorafs_node/src/scheduler.rs`).
2. ** اختبار التكامل ** (`crates/sorafs_node/tests`). الدبوس → جلب ذهابًا وإيابًا، والتعافي من خلال إعادة التدوير، وإعادة الشحن من خلال الجلد، والتحقق من اختبارات موستريو بور.
3. **اختبار التكامل لـ Torii.** قم بتشغيل Torii مع تخزين مؤهل، وجرب نقاط النهاية HTTP عبر `assert_cmd`.
4. **Hoja de ruta de caos.** Futuros تدريبات محاكاة لقمة الديسكو، IO lento، رجع الموردون.

## التبعيات

- سياسة القبول SF-2b — تأكد من أن العقد تتحقق من مظاريف القبول قبل الإعلان عنها.
- Marketplace de capacidad SF-2c — بالقرب من قياس السرعة عن بعد وإعلانات السعة.
- امتدادات الإعلان SF-2d — تستهلك سعة النطاق + متطلبات البث عندما تكون متاحة.

## معايير خروج الضربة- `cargo run -p sorafs_node --example pin_fetch` وظيفة مكافحة التركيبات المحلية.
- تم تجميع Torii مع `--features sorafs-storage` واختبار التكامل.
- توثيق ([دليل تخزين العقدة](node-storage.md)) تم تحديثه باستخدام إعدادات التكوين الافتراضية + نماذج CLI؛ Runbook deoperador disponible.
- القياس عن بعد مرئي على لوحات المعلومات المرحلية؛ تنبيهات تم تكوينها لتشبع السعة وسقوط الطاقة.

## المستندات القابلة للتسجيل والعمليات

- تحديث [مرجع تخزين العقدة](node-storage.md) باستخدام إعدادات التكوين الافتراضية، واستخدام CLI، وخطوات استكشاف الأخطاء وإصلاحها.
- الحفاظ على [دليل عمليات العقدة](node-operations.md) المنسق مع التنفيذ المتوافق مع تطور SF-3.
- نشر مراجع API لنقاط النهاية `/sorafs/*` داخل بوابة المطورين والاتصال بالبيانات OpenAPI عندما تكون معالجات Torii موجودة في القائمة.
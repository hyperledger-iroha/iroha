---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/node-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: خطة العقدة
العنوان: خطة تنفيذ العقد SoraFS
Sidebar_label: خطة تنفيذ العقد
الوصف: محول مسار التخزين SF-3 إلى عمل هندسة عملي مع الزجاجات والأغطية وتغطية الاختبارات.
---

:::ملاحظة المصدر الكنسي
هذه الصفحة تعكس `docs/source/sorafs/sorafs_node_plan.md`. قم بمزامنة النسختين حتى ترجع إلى التوثيق التاريخي لأبو الهول.
:::

SF-3 يحرر الصندوق الأول القابل للتنفيذ `sorafs-node` الذي يحول العملية Iroha/Torii إلى مزود المخزون SoraFS. استخدم هذه الخطة مع [دليل تخزين البيانات](node-storage.md)، و[سياسة قبول مقدمي الخدمة](provider-admission-policy.md)، و[ملف مسار سوق سعة التخزين](storage-capacity-marketplace.md) لتسلسل الأطعمة.

## بورتيه سيبل (جالون M1)1. **تكامل مخزن القطع.** المغلف `sorafs_car::ChunkStore` مع واجهة خلفية ثابتة تقوم بتخزين بايتات القطع والبيانات والأعمدة في سجل البيانات الذي تم تكوينه.
2. **بوابة نقاط النهاية.** كشف نقاط النهاية HTTP Norito لإدخال الدبوس وجلب القطع وتشريح PoR وقياس المخزون عن بعد في المعالج Torii.
3. **تكوين التكوين.** إضافة بنية التكوين `SoraFsStorage` (علامة التنشيط، والسعة، والسجلات، وحدود التزامن) تعتمد على `iroha_config`، و`iroha_core`، و`iroha_torii`.
4. **الحصة/الطلب.** تطبيق حدود القرص/التوازي التي يحددها المشغل وقياس الطلبات بالضغط الخلفي.
5. **القياس عن بعد.** قياس المقاييس/السجلات لنجاح الدبوس، وتأخر جلب القطع، واستخدام السعة، ونتائج المسح.

## تفكيك العمل

### أ. هيكل الصناديق والوحدات النمطية| تاش | المسؤول (المسؤولون) | ملاحظات |
|------|----------------|------|
| قم بإنشاء `crates/sorafs_node` باستخدام الوحدات: `config`، `store`، `gateway`، `scheduler`، `telemetry`. | تخزين المعدات | قم بإعادة تصدير الأنواع القابلة لإعادة الاستخدام للتكامل Torii. |
| تم تنفيذ `StorageConfig` من `SoraFsStorage` (المستخدم → الفعلي → الإعدادات الافتراضية). | تخزين المعدات / تكوين WG | تأكد من أن الأرائك Norito/`iroha_config` لا تزال محددة. |
| قم بتوفير واجهة `NodeHandle` التي تستخدم Torii لبعض الدبابيس/الجلب. | تخزين المعدات | قم بتغليف الأجزاء الداخلية من المخزون والتفريغ غير المتزامن. |

### ب. مخزن القطعة ثابت

| تاش | المسؤول (المسؤولون) | ملاحظات |
|------|----------------|------|
| أنشئ قرصًا خلفيًا مغلفًا `sorafs_car::ChunkStore` مع فهرس بيان على القرص (`sled`/`sqlite`). | تخزين المعدات | محدد التخطيط: `<data_dir>/<manifest_cid>/chunk_{idx}.bin`. |
| صيانة البيانات الدقيقة PoR (حجم 64 كيلو بايت/4 كيلو بايت) عبر `ChunkStore::sample_leaves`. | تخزين المعدات | دعم إعادة التشغيل بعد إعادة الزواج؛ تفشل بسرعة في حالة الفساد. |
| Implémenter le replay d'intégrité au démarrage (إعادة صياغة البيانات، وإزالة الدبابيس غير المكتملة). | تخزين المعدات | قم بحظر بدء تشغيل Torii حتى نهاية إعادة التشغيل. |

### ج. بوابة نقاط النهاية| نقطة النهاية | السلوك | تاتش |
|----------|-------------|-------|
| `POST /sorafs/pin` | اقبل `PinProposalV1`، وقم بصلاحية البيانات، مع إدخال ملف، والرد على CID الخاص بالبيان. | التحقق من ملف التعريف الخاص بـ Chunker، وتطبيق الحصص، وبث البيانات عبر متجر Le Chunk. |
| `GET /sorafs/chunks/{cid}` + طلب النطاق | قم بإدخال وحدات البايت مع الرؤوس `Content-Chunker` ؛ احترام مواصفات سعة النطاق. | استخدم برنامج الجدولة + ميزانيات البث (حسب سعة النطاق SF-2d). |
| `POST /sorafs/por/sample` | Lance an échantillonnage PoR لبيان وأعاد حزمة مسبقة. | أعد استخدام مخزن القطع، واستجب للحمولات Norito JSON. |
| `GET /sorafs/telemetry` | السيرة الذاتية: القدرة، النجاح، قياس أخطاء الجلب. | قم بتوفير البيانات للوحات المعلومات/المشغلين. |

يعتمد وقت تشغيل التفريغ على تفاعلات PoR عبر `sorafs_node::por`: يقوم المتتبع بتسجيل كل من `PorChallengeV1` و`PorProofV1` و`AuditVerdictV1` بحيث تعكس المقاييس `CapacityMeter` أحكام الحكم بلا منطق Torii spécifique.【crates/sorafs_node/src/scheduler.rs#L147】

ملاحظات التنفيذ:

- استخدم مكدس Axum de Torii مع الحمولات الصافية `norito::json`.
- إضافة المخططات Norito للإجابات (`PinResultV1`, `FetchErrorV1`, هياكل القياس عن بعد).- ✅ يعرض `/v2/sorafs/por/ingestion/{manifest_digest_hex}` عمق التراكم بالإضافة إلى العصر/التحديث القديم والطوابع الزمنية للنجاح/التحقق من أحدث الأحداث من قبل الموفر، عبر `sorafs_node::NodeHandle::por_ingestion_status`، وTorii قم بتسجيل المقاييس `torii_sorafs_por_ingest_backlog`/`torii_sorafs_por_ingest_failures_total` للي لوحات المعلومات.[crates/sorafs_node/src/lib.rs:510][crates/iroha_torii/src/sorafs/api.rs:1883][crates/iroha_torii/src/routing.rs:7244][crates/iroha_telemetry/src/metrics.rs:5390]

### د. جدولة وتطبيق الحصص

| تاش | التفاصيل |
|------|---------|
| حصة الحصص | متابعة البايتات على القرص ; قم بإلغاء تثبيت الدبابيس الجديدة في `max_capacity_bytes`. استخلاص خطافات الإخلاء من أجل السياسة المستقبلية. |
| موافقة الجلب | Sémaphore global (`max_parallel_fetches`) بالإضافة إلى الميزانيات التي يقدمها الموفر لإصدار حدود النطاق SF-2d. |
| ملف دي دبابيس | تحديد وظائف العرض والانتباه؛ يعرض نقاط النهاية للحالة Norito لعمق الملف. |
| الإيقاع PoR | عامل طيار مولع على قدم المساواة `por_sample_interval_secs`. |

### E. Télémétrie وتسجيل الدخول

المقاييس (Prometheus):

- `sorafs_pin_success_total`، `sorafs_pin_failure_total`
- `sorafs_chunk_fetch_duration_seconds` (المدرج التكراري مع التسميات `result`)
- `torii_sorafs_storage_bytes_used`، `torii_sorafs_storage_bytes_capacity`
- `torii_sorafs_storage_pin_queue_depth`، `torii_sorafs_storage_fetch_inflight`
-`torii_sorafs_storage_fetch_bytes_per_sec`
-`torii_sorafs_storage_por_inflight`
- `torii_sorafs_storage_por_samples_success_total`، `torii_sorafs_storage_por_samples_failed_total`

السجلات / الأحداث :- Télémétrie Norito مُصمم لإدارة العرض (`StorageTelemetryV1`).
- تنبيهات عند الاستخدام > 90% أو أن سلسلة اختبارات الأداء تتجاوز المتابعة.

### واو استراتيجية الاختبارات

1. **الاختبارات الوحدوية.** استمرارية تخزين القطع وحسابات الحصص وثوابت الجدولة (العرض `crates/sorafs_node/src/scheduler.rs`).
2. **اختبارات التكامل** (`crates/sorafs_node/tests`). Pin → fetch aller-retour، reprise après redémarrage، rejet de quita، vérification de preuve d'échantillonnage PoR.
3. **اختبارات التكامل Torii.** يتم تنفيذ Torii مع المخزون النشط، وممارسة نقاط النهاية HTTP عبر `assert_cmd`.
4. **فوضى خريطة الطريق.** التدريبات المستقبلية المتزامنة مع تشغيل القرص، وإخراج المعلومات، وقمع مقدمي الخدمة.

## التبعيات

- سياسة القبول SF-2b — التأكد من أن البيانات تتحقق من مظاريف القبول قبل نشر الإعلانات.
- Marketplace de capacité SF-2c — قم بربط جهاز القياس عن بعد بتصريحات السعة.
- ملحقات الإعلان SF-2d — تستهلك سعة النطاق + ميزانيات البث المتاحة.

## معايير طلعة جالون- `cargo run -p sorafs_node --example pin_fetch` يعمل على الإعدادات المحلية.
- تم إنشاء Torii مع `--features sorafs-storage` واجتياز اختبارات التكامل.
- التوثيق ([دليل تخزين البيانات](node-storage.md)) تحديث مع إعدادات التكوين الافتراضية + أمثلة CLI ؛ runbook opérateur disponible.
- التحكم عن بعد مرئي على لوحات المعلومات المرحلية؛ تنبيهات تم تكوينها لتشبع السعة والتحقق من PoR.

## توثيق Livrables et ops

- تحديث [مرجع تخزين البيانات](node-storage.md) باستخدام إعدادات التكوين الافتراضية واستخدام CLI وخطوات استكشاف الأخطاء وإصلاحها.
- قم بمحاذاة [دليل عمليات التشغيل](node-operations.md) مع تنفيذ الفراء وقياس تطور SF-3.
- نشر مراجع API لنقاط النهاية `/sorafs/*` في مطور البوابة والاعتماد على البيان OpenAPI ثم وضع المعالجات Torii في مكانها.
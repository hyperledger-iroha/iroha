---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/node-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: خطة العقدة
العنوان: خطة تنفيذ عقدة SoraFS
Sidebar_label: خطة تنفيذ العقدة
الوصف: تحويل بخارة طريق تخزين SF-3 إلى عمل قابل للتنفيذ مع معالم ومهام وتغطية السوائل.
---

:::ملحوظة المصدر مؤهل
احترام هذه الصفحة `docs/source/sorafs/sorafs_node_plan.md`. احرص على جميع النسختين متزامنتين إلى أن يتم إغلاق وثائق Sphinx القديمة.
:::

تقدم SF-3 أول صندوق قابل للتشغيل باسم `sorafs-node` محرك النقل Iroha/Torii إلى موفر SoraFS. استخدم هذه الأساس بناءً على [دليل تخزين العقدة](node-storage.md)، و[سياسة قبول المتوفرين](provider-admission-policy.md)، و[خارطة طريق التخزينة](storage-capacity-marketplace.md) عند ترتيب توريات.

## النطاق المستهدف (المرحلة M1)

1. **تكامل مخزن القطع.** `sorafs_car::ChunkStore` بواجهة خلفية ورق تخزن تخزين أقراص القطع المقطوعة وأشجار PoR في بيانات المجلد المهيأ.
2. **نقاط نهاية البوابة.** توفير نقاط نهاية HTTP لـ Norito.
3. **توصيل الإعدادات.** إضافة إعدادات إعداد `SoraFsStorage` (مفتاح التفعيل، السعة، المجلدات، حدود التوازن) وتمريرها عبر `iroha_config` و`iroha_core` و`iroha_torii`.
4. **الحصص/الجدولة.** فرض حدود القرص/التوازن الذي يحددها لتشغيل الطلبات في طوابير بالضغط الخلفي.
5. **التليمترية.** إصدار معايير/سجلات لنجاح دبوس وزمن جلب القطع واستغلال السعة ونتائج تلت PoR.

## تفصيل العمل

### أ. أسس الـ الصندوق والوحدات| | المالك | مذكرة |
|------|--------|-----------|
| إنشاء `crates/sorafs_node` مع الوحدات: `config` و`store` و`gateway` و`scheduler` و`telemetry`. | فريق التخزين | إعادة تصدير الأنواع القابلة لإعادة الاستخدام لدمجها مع Torii. |
| نفذ `StorageConfig` المشتق من `SoraFsStorage` (المستخدم → الفعلي → الافتراضي). | فريق التخزين / Config WG | ضمان طبقات البقاء Norito/`iroha_config` حتمية. |
| توفير واجهة `NodeHandle` يستخدمها Torii دبابيس/جلب. | فريق التخزين | تفاصيل التعبئة والتغليف والتوصيلات غير المتزامنة. |

### ب. مخزن قطع دائمة

| | المالك | مذكرة |
|------|--------|-----------|
| بناء واجهة القرص المضغوط `sorafs_car::ChunkStore` مع بيان فهرس على القرص (`sled`/`sqlite`). | فريق التخزين | تخطيط حتمي: `<data_dir>/<manifest_cid>/chunk_{idx}.bin`. |
| توفر بيانات PoR الوصفية (أشجار 64 KiB/4 KiB) باستخدام `ChunkStore::sample_leaves`. | فريق التخزين | يدعم إعادة التشغيل؛ يفشل بسرعة عند التلف. |
| تنفيذ إعادة فحص السلامة عند البدء (إعادة تجزئة البيانات وحذف الدبابيس غير المكتملة). | فريق التخزين | يمنع بدء Torii حتى قانوني إعادة نسخ القلم. |

### ج. نهاية نقاط البوابة| نقطة |النهاية سلوك | العمل |
|-------------|---------|-------|
| `POST /sorafs/pin` | يقبل `PinProposalV1` والتحقق من البيان ووضع الإدخال في الطابور والرد بـ CID الخاص بالـ Manifest. | التحقق من ملف تعريف الـ Chunker وافتراض الحصص وبث البيانات عبر مخزن القطع. |
| `GET /sorafs/chunks/{cid}` + نطاق استعلام | تقديم بايتات القطع مع ترويسات `Content-Chunker` بالفعل مواصفة نطاق الفان. | استخدام المجدول مع ميزانيات التلفزيون (ربطها بقدرات النطاق SF-2d). |
| `POST /sorafs/por/sample` | أخذ عينة PoR لملف البيان والإرجاع الواضح. | إعادة استخدام أخذت من مخزن القطع والرد عبر Norito JSON. |
| `GET /sorafs/telemetry` | ملخصات: سعة ونجاح PoR وأخطاء الجلب. | توفير للوحة المراقبة/المشغلين لبيانات. |

تقوم الوصلات في وقت التشغيل بتمرير تفاعلات PoR عبر `sorafs_node::por`، حيث يتم تسجيل التتبع كل `PorChallengeV1` و`PorProofV1` و`AuditVerdictV1` لكي تتمكن من الدخول إلى الحسابات `CapacityMeter` من دون منطقة Torii مخصص.[صناديق/sorafs_node/src/scheduler.rs#L147]

أفكار تنفيذية:

- استخدم مكدس Axum الخاص بـ Torii مع حمولات `norito::json`.
- إضافة مخططات Norito للاستجابات (`PinResultV1` و`FetchErrorV1` وبنى التليميترية).- ✅ أصبح المسار `/v2/sorafs/por/ingestion/{manifest_digest_hex}` خالصة الـ backlog وأقدم Epoch/deadline وأحدث طوابع النجاح/الفشل لكل مضمون، عبر `sorafs_node::NodeHandle::por_ingestion_status`، وتسجل Torii عدادات `torii_sorafs_por_ingest_backlog`/`torii_sorafs_por_ingest_failures_total` للوحات.【crates/sorafs_node/src/lib.rs:510】【crates/iroha_torii/src/sorafs/api.rs:1883】【crates/iroha_torii/src/routing.rs:7244】【crates/iroha_telemetry/src/metrics.rs:5390】

### د. المجدول وفرض الحصص

| | التفاصيل |
|------|----------|
| حصة القرص | تتبع البايتات على القرص؛ دبابيس الرفض الجديدة عند تجاوز `max_capacity_bytes`. توفير نقاط اتصال لسياسات المستقبل المستقبلية. |
| جلب التوازن | شبهور عام (`max_parallel_fetches`) مع قياسات رقمية لكل نطاق من نطاق SF-2d. |
| دبابيس طابور | تحديد عدد وصفات الإدخال؛ توفير نقاط حالة Norito لعمق الطابور. |
| معالجة PoR | عامل خلفي يعمل وفق `por_sample_interval_secs`. |

### ه. التليميترية والسجلات

المقاييس (Prometheus):

- `sorafs_pin_success_total`، `sorafs_pin_failure_total`
- `sorafs_chunk_fetch_duration_seconds` (هيتوغرام مع وسوم `result`)
- `torii_sorafs_storage_bytes_used`، `torii_sorafs_storage_bytes_capacity`
- `torii_sorafs_storage_pin_queue_depth`، `torii_sorafs_storage_fetch_inflight`
-`torii_sorafs_storage_fetch_bytes_per_sec`
-`torii_sorafs_storage_por_inflight`
- `torii_sorafs_storage_por_samples_success_total`، `torii_sorafs_storage_por_samples_failed_total`

سجل / الأحداث:

- تليمترية Norito منظمة الدوران (`StorageTelemetryV1`).
- تنبيهات عند تجاوز الاستغلال 90% أو عندما تتخطى سلسلة أخف إاقات PoR العتبة.

### ف. استراتيجية التحدي1. **اختبارات الوحدات.** ديمومة مخزن القطع، حسابات الحصة، ثوابت الجدول (انظر `crates/sorafs_node/src/scheduler.rs`).
2. **الاختبارات المتكاملة** (`crates/sorafs_node/tests`). دورة الدبوس → الجلب، الاستعادة بعد إعادة التشغيل، الرغبات المحددة، والتحقق من إثباتات أخذ عينات من PoR.
3. **اختبارات تكامل Torii.** تشغيل Torii مع تفعيل التخزين وتجربة نقاط النهاية HTTP عبر `assert_cmd`.
4. **خارطة طريق الفوضى.** تدريبات مستقبلية تحاكي نفاد القرص، بطء IO، حذف المحظين.

##التبعيات

- يجب أن تقبل SF-2b — لكي تتحقق من أظرف تقبل قبل الإعلان.
- سوق السعة SF-2c — ربط التليميرية بإعلانات السعة.
- امتدادات إعلانية لـ SF-2d — استهلاك قدرة النطاق + ميزانيات البث عند توفرها.

## معايير المرحلة غير

- `cargo run -p sorafs_node --example pin_fetch` يعمل مع التركيبات السريعة.
- بناء Torii مع `--features sorafs-storage` واجيتياز تكامل.
- تحديث الوثائق ((دليل تخزين العقدة](node-storage.md)) مع افتراضيات وأمثلة CLI؛ Runbook للمشغلين.
- ظهور التليمترية في اللوحات التدريجية وضبط التنبيهات لشبع السعة وخفاقات PoR.

## مخرجات الوثائق التجريبية

- تحديث [مرجع تخزين النقابة](node-storage.md) بصيغة افتراضية، استخدام CLI، وخطوات الاستكشاف.
- تفاصيل [كتاب التشغيل لعمليات المؤتمر](node-operations.md) متوافقة مع التنفيذ مع SF-3.
- نشر مراجع API للنقاط النهائية `/sorafs/*` داخل بوابة المطورين وربطها بملف OpenAPI عند وصول معالجات Torii.
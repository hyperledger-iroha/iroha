---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/node-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: خطة العقدة
العنوان: Plano de Implementacao do nodo SoraFS
Sidebar_label: مخطط تنفيذ العقدة
الوصف: قم بتحويل خارطة طريق التخزين SF-3 إلى العمل في مجال التحفيز مع العلامات التجارية والتكاليف والتغطية الخاصة بالخصيتين.
---

:::ملاحظة فونتي كانونيكا
هذه الصفحة تحمل عنوان `docs/source/sorafs/sorafs_node_plan.md`. قم بحفظ النسخ المتزامنة بحيث يتم سحب مستند أبو الهول البديل.
:::

يدخل SF-3 الصندوق التنفيذي الأول `sorafs-node` الذي يحول المعالج Iroha/Torii إلى موفر التخزين SoraFS. استخدم هذه الخطة جنبًا إلى جنب مع [دليل التخزين للعقدة](node-storage.md)، و[سياسة قبول المعاملات](provider-admission-policy.md) و[خريطة طريق سوق سعة التخزين](storage-capacity-marketplace.md) للمدخلات التسلسلية.

## إسكوبو ألفو (ماركو إم 1)1. **تكامل مخزن القطع.** قم بتضمين `sorafs_car::ChunkStore` مع الواجهة الخلفية المستمرة التي تقوم بتخزين بايتات القطعة وإظهارها وتحميلها بدون دليل بيانات تم تكوينه.
2. **نقاط النهاية للبوابة.** قم بتصدير نقاط النهاية HTTP Norito لإرسال الدبوس، وجلب القطع، ودمج نقاط القوة والقياس عن بعد للتخزين في عملية Torii.
3. **تكوين السباكة.** إضافة بنية التكوين `SoraFsStorage` (إشارة إلى المؤهلات والسعة والمديرين وحدود الترابط) متصلة عبر `iroha_config` و`iroha_core` و`iroha_torii`.
4. **الحصة/الجدول.** أهمية حدود القرص/التوازي المحددة للمشغل وتسجيل المتطلبات مع الضغط الخلفي.
5. **القياس عن بعد.** قم بإصدار مقاييس/سجلات لنجاح الدبوس، وتأخر جلب القطع، واستخدام السعة، ونتائج تعزيز PoR.

## مهمة العمل

### أ. إنشاء الصناديق والوحدات| طريفة | دونو (ق) | نوتاس |
|------|---------|-----|
| Criar `crates/sorafs_node` مع الوحدات: `config`، `store`، `gateway`، `scheduler`، `telemetry`. | فريق التخزين | قم بإعادة تصدير الأنواع المعاد استخدامها للتكامل مع Torii. |
| قم بتنفيذ خريطة `StorageConfig` من `SoraFsStorage` (المستخدم -> الفعلي -> الإعدادات الافتراضية). | فريق التخزين / تكوين مجموعة العمل | تأكد من أن الكاميرات Norito/`iroha_config` تحدد حتمية الكاميرا. |
| Fornecer uma الواجهة `NodeHandle` que Torii usa لدبابيس/جلبات جهاز القياس الفرعي. | فريق التخزين | كبسولة داخلية للتخزين والسباكة غير متزامنة. |

### ب. مخزن القطعة المستمر

| طريفة | دونو (ق) | نوتاس |
|------|---------|-----|
| قم بإنشاء واجهة خلفية على القرص `sorafs_car::ChunkStore` مع فهرس البيان على القرص (`sled`/`sqlite`). | فريق التخزين | محددات التخطيط: `<data_dir>/<manifest_cid>/chunk_{idx}.bin`. |
| المزيد من البيانات الوصفية PoR (64 كيلو بايت/4 كيلو بايت) تستخدم `ChunkStore::sample_leaves`. | فريق التخزين | إعادة تشغيل Suporta apos؛ يفشل بسرعة في الفساد. |
| تنفيذ إعادة التشغيل المتكامل بدون بدء التشغيل (إعادة صياغة البيانات، عدم اكتمال الدبابيس). | فريق التخزين | قم بحظر بداية Torii أو إعادة تشغيل النهاية. |

### ج. نقاط النهاية للبوابة| نقطة النهاية | السلوك | طرفاس |
|----------|--------------|--------|
| `POST /sorafs/pin` | Aceita `PinProposalV1`، بيانات التحقق، تسجيل الدخول، الاستجابة مع بيان CID. | التحقق من صحة ملف التعريف والحصص المستوردة وبث البيانات عبر متجر القطع. |
| `GET /sorafs/chunks/{cid}` + استعلام النطاق | خادم وحدات البايت من رؤوس com `Content-Chunker`؛ احترام نطاق السعة المحددة. | استخدام جدولة + تدفق Orcamentos (يرتبط بقدرة النطاق SF-2d). |
| `POST /sorafs/por/sample` | يوفر Rodar amostragem PoR لبيان وإعادة حزمة الاختبار. | إعادة استخدام amostragem dochunk store، Response com payloads Norito JSON. |
| `GET /sorafs/telemetry` | السيرة الذاتية: القدرة، نجاح PoR، مقاومات خطأ الجلب. | Fornecer دادوس الفقرة لوحات القيادة/المشغلين. |

يتم تنفيذ أعمال السباكة في وقت التشغيل كتداخلات PoR عبر `sorafs_node::por`: يتم تسجيل المتعقب لكل من `PorChallengeV1` و`PorProofV1` و`AuditVerdictV1` من أجل إعادة ضبط المقاييس `CapacityMeter` على حقائق الإدارة بدون منطق Torii مفصل. [الصناديق/sorafs_node/src/scheduler.rs:147]

ملاحظات التنفيذ:

- استخدم مكدس Axum de Torii لحمولات com `norito::json`.
- إضافة مخططات Norito للإجابة (`PinResultV1`، `FetchErrorV1`، هياكل القياس عن بعد).- يعرض `/v2/sorafs/por/ingestion/{manifest_digest_hex}` قبل ذلك عمقًا في تراكم الأعمال المتراكمة في عصر/موعد نهائي أكثر حداثة وطوابع زمنية أحدث للنجاح/الفشل من خلال إثبات، عبر `sorafs_node::NodeHandle::por_ingestion_status`، وTorii تسجيل أجهزة القياس `torii_sorafs_por_ingest_backlog`/`torii_sorafs_por_ingest_failures_total` للوحات المعلومات. [الصناديق/sorafs_node/src/lib.rs:510] [الصناديق/iroha_torii/src/sorafs/api.rs:1883] [الصناديق/iroha_torii/src/routing.rs:7244] [الصناديق/iroha_telemetry/src/metrics.rs:5390]

### د. جدولة الحصص وتخصيصها

| طريفة | ديتالز |
|------|---------|
| حصة الديسكو | وحدات البايت النقطية في الديسكو؛ دبابيس جديدة rejeitar ao exceder `max_capacity_bytes`. خطافات الإخلاء للسياسة المستقبلية. |
| مطابقة الجلب | Semaforo global (`max_parallel_fetches`) أكثر ثباتًا لأقصى حدود النطاق SF-2d. |
| فيلا دي دبابيس | الحد من وظائف استيعاب المعلقات؛ تصدير حالة نقاط النهاية Norito لعمق الصفحة. |
| كادينسيا بور | عامل الخلفية موجه إلى `por_sample_interval_secs`. |

### هـ. القياس عن بعد والتسجيل

المقاييس (Prometheus):

- `sorafs_pin_success_total`، `sorafs_pin_failure_total`
- `sorafs_chunk_fetch_duration_seconds` (تسميات المدرج الإحصائي com `result`)
- `torii_sorafs_storage_bytes_used`، `torii_sorafs_storage_bytes_capacity`
- `torii_sorafs_storage_pin_queue_depth`، `torii_sorafs_storage_fetch_inflight`
-`torii_sorafs_storage_fetch_bytes_per_sec`
-`torii_sorafs_storage_por_inflight`
- `torii_sorafs_storage_por_samples_success_total`، `torii_sorafs_storage_por_samples_failed_total`

السجلات/الأحداث:- القياس عن بعد Norito estruturada para استيعاب الحاكم (`StorageTelemetryV1`).
- تنبيهات عند الاستخدام > 90% أو خط خطأ يتجاوز الحد الأدنى.

### واو استراتيجية الخصيتين

1. **الاختبارات الوحدوية.** استمرارية تخزين القطع وحسابات الحصص والجدولة الثابتة (الإصدار `crates/sorafs_node/src/scheduler.rs`).
2. **الخصيتين التكامليتين** (`crates/sorafs_node/tests`). الدبوس -> جلب ذهابًا وإيابًا، إعادة تشغيل الاسترداد، طلب الحصة، التحقق من إثبات الثبات.
3. **اختبارات التكامل Torii.** قضيب Torii مع قدرة تخزينية، وممارسة نقاط النهاية HTTP عبر `assert_cmd`.
4. **خريطة طريق Caos.** تدريبات مستقبلية محاكاة محاكاة ديسكو، IO Lento، إزالة المثبتات.

## التبعيات

- سياسة القبول SF-2b - ضمان التحقق من مظاريف القبول قبل الإعلان.
- Marketplace de capacidade SF-2c - جهاز قياس الجهد عن بعد لإعلان السعة.
- امتدادات الإعلان SF-2d - استهلاك سعة النطاق + تعزيزات البث عند توفرها.

## معايير صيدا دو ماركو- `cargo run -p sorafs_node --example pin_fetch` وظيفة مكافحة التركيبات المحلية.
- تم تجميع Torii مع `--features sorafs-storage` وتمرير الخصيتين المتكاملتين.
- توثيق المستندات ([دليل التخزين](node-storage.md)) تم تحديثه باستخدام إعدادات التكوين الافتراضية + أمثلة لواجهة سطر الأوامر؛ runbook deoperador disponivel.
- قياس عن بعد مرئي على لوحات معلومات التدريج؛ تنبيهات تم تكوينها لتشبع السعة وفالهاس بور.

## Entregaveis de documentacao e ops

- قم بتحديث [مرجع تخزين العقدة](node-storage.md) من خلال إعدادات التكوين الافتراضية واستخدام CLI وخطوات استكشاف الأخطاء وإصلاحها.
- قم بمتابعة [دليل تشغيل العقدة](node-operations.md) مع تنفيذ يتوافق مع SF-3 المتطور.
- نشر مراجع واجهة برمجة التطبيقات (API) لنقاط النهاية `/sorafs/*` داخل بوابة التصميم والاتصال بالبيان OpenAPI بينما يظل معالجو Torii على طول الخط.
---
lang: ar
direction: rtl
source: docs/source/compute_lane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 624ca40bb09d616d2820a7229022507b73dc3c0692f7eb83f5169aee32a64c4f
source_last_modified: "2026-01-03T18:07:56.917770+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# حساب المسار (SSC-1)

يقبل مسار الحوسبة مكالمات نمط HTTP الحتمية، ويقوم بتعيينها على Kotodama
نقاط الدخول، وسجلات القياس/الإيصالات لمراجعة الفواتير والحوكمة.
يقوم RFC بتجميد مخطط البيان، وأظرف المكالمات/الإيصالات، وحواجز حماية وضع الحماية،
والتكوين الافتراضي للإصدار الأول.

##بيان

- المخطط: `crates/iroha_data_model/src/compute/mod.rs` (`ComputeManifest` /
  `ComputeRoute`).
- تم تثبيت `abi_version` على `1`؛ تم رفض البيانات بإصدار مختلف
  أثناء التحقق من الصحة.
- يعلن كل طريق:
  - `id` (`service`، `method`)
  - `entrypoint` (اسم نقطة الإدخال Kotodama)
  - القائمة المسموح بها لبرنامج الترميز (`codecs`)
  - قبعات TTL/الغاز/الطلب/الاستجابة (`ttl_slots`، `gas_budget`، `max_*_bytes`)
  - فئة الحتمية/التنفيذ (`determinism`، `execution_class`)
  - SoraFS واصفات النموذج/المدخل (`input_limits`، `model` اختياري)
  - عائلة التسعير (`price_family`) + ملف تعريف الموارد (`resource_profile`)
  - سياسة المصادقة (`auth`)
- توجد حواجز حماية Sandbox في كتلة البيان `sandbox` ويتم مشاركتها من قبل الجميع
  الطرق (الوضع/العشوائية/التخزين ورفض syscall غير الحتمي).

مثال: `fixtures/compute/manifest_compute_payments.json`.

## المكالمات والطلبات والإيصالات

- المخطط: `ComputeRequest`، `ComputeCall`، `ComputeCallSummary`، `ComputeReceipt`،
  `ComputeMetering`، `ComputeOutcome` في
  `crates/iroha_data_model/src/compute/mod.rs`.
- يُنتج `ComputeRequest::hash()` تجزئة الطلب الأساسية (يتم الاحتفاظ بالرؤوس
  في `BTreeMap` الحتمية ويتم حمل الحمولة كـ `payload_hash`).
- يلتقط `ComputeCall` مساحة الاسم/المسار، برنامج الترميز، TTL/الغاز/غطاء الاستجابة،
  ملف تعريف الموارد + عائلة الأسعار، المصادقة (`Public` أو UAID
  `ComputeAuthn`)، الحتمية (`Strict` مقابل `BestEffort`)، فئة التنفيذ
  تلميحات (CPU/GPU/TEE)، تم الإعلان عن SoraFS بايت/قطع الإدخال، الراعي الاختياري
  الميزانية ومظروف الطلب الأساسي. يتم استخدام تجزئة الطلب لـ
  إعادة الحماية والتوجيه.
- قد تتضمن المسارات مراجع نموذج SoraFS الاختيارية وحدود الإدخال
  (أغطية مضمنة/قطعة)؛ قواعد وضع الحماية الواضحة بوابة لتلميحات GPU/TEE.
- يقوم `ComputePriceWeights::charge_units` بتحويل بيانات القياس إلى حساب مفوتر
  الوحدات عبر تقسيم السقف على الدورات وبايتات الخروج.
- تقارير `ComputeOutcome` `Success`، `Timeout`، `OutOfMemory`،
  `BudgetExhausted`، أو `InternalError` ويتضمن بشكل اختياري تجزئات الاستجابة/
  الأحجام/الترميز للتدقيق.

أمثلة:
- اتصل: `fixtures/compute/call_compute_payments.json`
- الاستلام: `fixtures/compute/receipt_compute_payments.json`

## وضع الحماية وملفات تعريف الموارد- يقوم `ComputeSandboxRules` بتأمين وضع التنفيذ على `IvmOnly` بشكل افتراضي،
  بذور العشوائية الحتمية من تجزئة الطلب، تسمح للقراءة فقط SoraFS
  الوصول، ويرفض syscalls غير حتمية. يتم بوابات تلميحات GPU/TEE
  `allow_gpu_hints`/`allow_tee_hints` للحفاظ على حتمية التنفيذ.
- يقوم `ComputeResourceBudget` بتعيين الحدود القصوى لكل ملف تعريف على الدورات والذاكرة الخطية والمكدس
  الحجم، وميزانية الإدخال/الإخراج، والخروج، بالإضافة إلى تبديل تلميحات وحدة معالجة الرسومات ومساعدي WASI-lite.
- تقوم الإعدادات الافتراضية بشحن ملفين شخصيين (`cpu-small`، `cpu-balanced`) ضمن
  `defaults::compute::resource_profiles` مع احتياطيات حتمية.

## وحدات التسعير والفوترة

- عائلات الأسعار (`ComputePriceWeights`) تحدد الدورات وتدخل وحدات البايت في الحساب
  وحدات؛ الإعدادات الافتراضية تشحن `ceil(cycles/1_000_000) + ceil(egress_bytes/1024)` مع
  `unit_label = "cu"`. يتم ربط العائلات بواسطة `price_family` في البيانات و
  المفروضة عند القبول.
- تحمل سجلات القياس `charged_units` بالإضافة إلى الدورة الأولية/الدخول/الخروج/المدة
  مجموع للمصالحة. يتم تضخيم الرسوم من خلال فئة التنفيذ و
  مضاعفات الحتمية (`ComputePriceAmplifiers`) وتوج بها
  `compute.economics.max_cu_per_call`; يتم فرض الخروج من قبل
  `compute.economics.max_amplification_ratio` لتضخيم الاستجابة المقيدة.
- يتم فرض ميزانيات الجهة الراعية (`ComputeCall::sponsor_budget_cu`).
  لكل مكالمة / الحد الأقصى اليومي؛ يجب ألا تتجاوز الوحدات المفوترة ميزانية الراعي المعلنة.
- تستخدم تحديثات أسعار الحوكمة حدود فئة المخاطر في
  `compute.economics.price_bounds` والعائلات الأساسية المسجلة في
  `compute.economics.price_family_baseline`; استخدام
  `ComputeEconomics::apply_price_update` للتحقق من صحة الدلتا قبل التحديث
  خريطة العائلة النشطة. استخدام تحديثات التكوين Torii
  `ConfigUpdate::ComputePricing`، ويطبقه kiso بنفس الحدود على
  إبقاء تعديلات الحكم حتمية.

## التكوين

تكوين الحوسبة الجديد موجود في `crates/iroha_config/src/parameters`:

- عرض المستخدم: `Compute` (`user.rs`) مع تجاوزات البيئة:
  - `COMPUTE_ENABLED` (`false` الافتراضي)
  - `COMPUTE_DEFAULT_TTL_SLOTS` / `COMPUTE_MAX_TTL_SLOTS`
  - `COMPUTE_MAX_REQUEST_BYTES` / `COMPUTE_MAX_RESPONSE_BYTES`
  -`COMPUTE_MAX_GAS_PER_CALL`
  - `COMPUTE_DEFAULT_RESOURCE_PROFILE` / `COMPUTE_DEFAULT_PRICE_FAMILY`
  -`COMPUTE_AUTH_POLICY`
- التسعير/الاقتصاد: يلتقط `compute.economics`
  `max_cu_per_call`/`max_amplification_ratio`، تقسيم الرسوم، الحدود القصوى للجهة الراعية
  (لكل مكالمة وCU يومية)، خطوط الأساس لعائلة الأسعار + فئات/حدود المخاطر
  تحديثات الإدارة ومضاعفات فئة التنفيذ (GPU/TEE/best-effort).
- الفعلي/الافتراضي: تم تحليل `actual.rs` / `defaults.rs::compute`
  إعدادات `Compute` (مساحات الأسماء، الملفات الشخصية، عائلات الأسعار، وضع الحماية).
- تكوينات غير صالحة (مساحات أسماء فارغة، ملف التعريف الافتراضي/العائلة مفقود، غطاء TTL
  الانقلابات) تظهر على شكل `InvalidComputeConfig` أثناء التحليل.

## الاختبارات والمواعيد

- المساعدون الحتميون (`request_hash`، التسعير) ورحلات الذهاب والإياب الثابتة يعيشون في
  `crates/iroha_data_model/src/compute/mod.rs` (راجع `fixtures_round_trip`،
  `request_hash_is_stable`، `pricing_rounds_up_units`).
- تركيبات JSON تعيش في `fixtures/compute/` ويتم ممارستها بواسطة نموذج البيانات
  اختبارات لتغطية الانحدار.

## تسخير SLO والميزانيات- يكشف تكوين `compute.slo.*` عن مقابض البوابة SLO (قائمة الانتظار أثناء الطيران
  العمق وغطاء RPS وأهداف الكمون) في
  `crates/iroha_config/src/parameters/{user,actual,defaults}.rs`. الافتراضي: 32
  على متن الطائرة، 512 في قائمة الانتظار لكل مسار، 200 دورة في الثانية، p50 25 مللي ثانية، p95 75 مللي ثانية، p99 120 مللي ثانية.
- قم بتشغيل حزام المقعد خفيف الوزن لالتقاط ملخصات SLO والطلب/الخروج
  لقطة: `تشغيل البضائع -p xtask --bin compute_gateway -- bench [manifest_path]
  [التكرارات] [التزامن] [out_dir]` (defaults: `fixtures/compute/manifest_compute_patients.json`,
  128 تكرارًا، التزامن 16، المخرجات تحت
  `artifacts/compute_gateway/bench_summary.{json,md}`). يستخدم مقاعد البدلاء
  الحمولات الحتمية (`fixtures/compute/payload_compute_payments.json`) و
  رؤوس لكل طلب لتجنب إعادة الاصطدامات أثناء التمرين
  نقاط الدخول `echo`/`uppercase`/`sha3`.

## تركيبات التكافؤ SDK/CLI

- التركيبات الأساسية موجودة ضمن `fixtures/compute/`: البيان، والاتصال، والحمولة، و
  تخطيط الاستجابة/الاستلام على نمط البوابة. يجب أن تتطابق تجزئات الحمولة مع المكالمة
  `request.payload_hash`; الحمولة المساعدة تعيش في
  `fixtures/compute/payload_compute_payments.json`.
- تقوم واجهة سطر الأوامر (CLI) بشحن `iroha compute simulate` و`iroha compute invoke`:

```bash
iroha compute simulate \
  --manifest fixtures/compute/manifest_compute_payments.json \
  --call fixtures/compute/call_compute_payments.json \
  --payload fixtures/compute/payload_compute_payments.json

iroha compute invoke \
  --endpoint http://127.0.0.1:8088 \
  --manifest fixtures/compute/manifest_compute_payments.json \
  --call fixtures/compute/call_compute_payments.json \
  --payload fixtures/compute/payload_compute_payments.json
```

- JS: `loadComputeFixtures`/`simulateCompute`/`buildGatewayRequest` يعيش في
  `javascript/iroha_js/src/compute.js` مع اختبارات الانحدار تحت
  `javascript/iroha_js/test/computeExamples.test.js`.
- Swift: يقوم `ComputeSimulator` بتحميل نفس التركيبات والتحقق من صحة تجزئات الحمولة النافعة،
  ويحاكي نقاط الدخول مع الاختبارات في
  `IrohaSwift/Tests/IrohaSwiftTests/ComputeSimulatorTests.swift`.
- يشترك جميع مساعدي CLI/JS/Swift في نفس تركيبات Norito حتى تتمكن مجموعات SDK من
  التحقق من صحة بناء الطلب ومعالجة التجزئة في وضع عدم الاتصال دون الضغط على أ
  بوابة التشغيل.
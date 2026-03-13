---
lang: ar
direction: rtl
source: docs/norito_demo_contributor.md
status: complete
translator: manual
source_hash: b11d23ecafbc158e0c83cdb6351085fde02f362cfc73a1a1a33555e90cc556ef
source_last_modified: "2025-11-09T09:04:55.207331+00:00"
translation_last_reviewed: 2025-11-14
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/norito_demo_contributor.md (Norito SwiftUI Demo Contributor Guide) -->

# دليل المساهمين في ديمو Norito SwiftUI

توضّح هذه الوثيقة خطوات الإعداد اليدوي المطلوبة لتشغيل ديمو SwiftUI ضد عقدة Torii
محلية ودفتر أستاذ (ledger) تجريبي. تكمل هذه الوثيقة `docs/norito_bridge_release.md`
من خلال التركيز على مهام التطوير اليومية. للحصول على شرح أعمق حول دمج جسر Norito
و stack الخاص بـ Connect في مشاريع Xcode، راجع `docs/connect_swift_integration.md`.

## إعداد البيئة

1. ثبّت toolchain الخاص بـ Rust كما هو محدد في `rust-toolchain.toml`.
2. ثبّت Swift 5.7 أو أحدث وأدوات سطر أوامر Xcode (Xcode Command Line Tools) على
   macOS.
3. (اختياري) ثبّت [SwiftLint](https://github.com/realm/SwiftLint) لأغراض linting.
4. نفّذ `cargo build -p irohad` للتأكد من أن عقدة Torii تبنى بنجاح على جهازك.
5. انسخ `examples/ios/NoritoDemoXcode/Configs/demo.env.example` إلى ملف `.env` وعدّل
   القيم بما يتناسب مع بيئتك. يقرأ التطبيق هذه المتغيرات عند الإقلاع:
   - `TORII_NODE_URL` — عنوان REST الأساسي (يُشتق منه عنوان WebSocket).
   - `CONNECT_SESSION_ID` — معرّف جلسة بطول 32 بايت (base64/base64url).
   - `CONNECT_TOKEN_APP` / `CONNECT_TOKEN_WALLET` — الرموز (tokens) التي يعيدها
     `/v2/connect/session`.
   - `CONNECT_CHAIN_ID` — معرّف السلسلة (chain) المُعلن أثناء control handshake.
   - `CONNECT_ROLE` — الدور الافتراضي المحدد مسبقًا في واجهة المستخدم (`app` أو
     `wallet`).
   - حقول مساعدة اختيارية لأغراض الاختبار اليدوي:
     `CONNECT_PEER_PUB_B64`, `CONNECT_SHARED_KEY_B64`, `CONNECT_APPROVE_ACCOUNT_ID`,
     `CONNECT_APPROVE_PRIVATE_KEY_B64`, `CONNECT_APPROVE_SIGNATURE_B64`.

## تشغيل Torii + ledger تجريبي

يحوي هذا الـ repository سكربتات مساعدة تقوم بتشغيل عقدة Torii مع ledger في الذاكرة
محمل مسبقًا بحسابات تجريبية:

```bash
./scripts/ios_demo/start.sh --config examples/ios/NoritoDemoXcode/Configs/SampleAccounts.json
```

ينتج السكربت:

- سجلات عقدة Torii في `artifacts/torii.log`.
- مقاييس ledger (بصيغة Prometheus) في `artifacts/metrics.prom`.
- رموز وصول (tokens) خاصة بالعميل في `artifacts/torii.jwt`.

يبقي `start.sh` الـ peer الخاص بالديمو قيد التشغيل إلى أن تضغط `Ctrl+C`. يكتب
snapshot لحالة الاستعداد (ready state) في `artifacts/ios_demo_state.json` (وهو مصدر
الحقيقة لبقية artefacts)، ينسخ log الـ stdout النشط لعقدة Torii، يستطلع `/metrics`
حتى تصبح بيانات Prometheus جاهزة، ثم يجهز الحسابات المكوّنة داخل `torii.jwt`
(بما في ذلك المفاتيح الخاصة عندما تكون متاحة في ملف الإعداد). يقبل السكربت المعلمات
`--artifacts` لتغيير مجلد الإخراج، و`--telemetry-profile` لمواءمة إعدادات Torii
مخصصة، و`--exit-after-ready` لتشغيله في بيئات CI غير تفاعلية.

تدعم كل صفحة (entry) في `SampleAccounts.json` الحقول التالية:

- `name` (سلسلة نصية، اختياري) — تُخزّن كبيان metadata تحت المفتاح `alias` لحساب
  المستخدم.
- `public_key` (سلسلة multihash، مطلوب) — تُستخدم كمفتاح توقيع (signatory) للحساب.
- `private_key` (اختياري) — تُدرج في `torii.jwt` لتسهيل توليد بيانات اعتماد
  العميل.
- `domain` (اختياري) — إذا لم يحدد، يُستخدم domain الخاص بالـ asset.
- `asset_id` (سلسلة، مطلوب) — تعريف الأصل المالي الذي يُضَخّ (mint) في الحساب.
- `initial_balance` (سلسلة، مطلوب) — القيمة الرقمية التي تُضخّ في الحساب.

## تشغيل ديمو SwiftUI

1. قم ببناء XCFramework كما هو موضح في `docs/norito_bridge_release.md` وأدرجه في مشروع
   الديمو (يتوقع المشروع وجود `NoritoBridge.xcframework` في جذر المشروع).
2. افتح مشروع `NoritoDemoXcode` من Xcode.
3. اختر الـ scheme `NoritoDemo` وحدد جهاز iOS أو محاكيًا (Simulator) كهدف للتشغيل.
4. تأكد من أن ملف `.env` مُعرّف ضمن متغيرات البيئة الخاصة بالـ scheme. املأ قيم
   `CONNECT_*` التي يعيدها `/v2/connect/session` بحيث تُملأ واجهة المستخدم مسبقًا عند
   إطلاق التطبيق.
5. تحقق من قيم تسريع العتاد الافتراضية: يستدعي ملف `App.swift`‎ الدالة
   `DemoAccelerationConfig.load().apply()` حتى تلتقط الديمو إعدادات البيئة
   `NORITO_ACCEL_CONFIG_PATH` أو الملفات المضمّنة
   `acceleration.{json,toml}`/`client.{json,toml}`. احذف هذه المدخلات أو عدّلها إذا كنت
   تريد إجبار التطبيق على العمل بشكل افتراضي باستخدام الـ CPU فقط.
6. قم ببناء وتشغيل التطبيق. تعرض الشاشة الرئيسية حقول Torii URL/token إذا لم تكن
   قد تم تعبئتها مسبقًا عبر `.env`.
7. ابدأ جلسة “Connect” للاشتراك في تحديثات الحساب أو للموافقة على الطلبات.
8. نفّذ تحويل IRH ولاحظ مخرجات السجل على الشاشة بالتوازي مع سجلات Torii.

### مفاتيح تفعيل التسريع العتادي (Metal / NEON)

يعكس `DemoAccelerationConfig` إعدادات عقدة Rust بحيث يمكن للمطورين تجربة مسارات
Metal/NEON من دون تثبيت (hard‑coding) حدود بعينها. يبحث الـ loader في المواقع
التالية عند الإقلاع:

1. `NORITO_ACCEL_CONFIG_PATH` (مُعرّفة في `.env`/وسائط الـ scheme) — مسار مطلق أو
   مسار يعتمد على `~` يشير إلى ملف `iroha_config` من نوع JSON/TOML.
2. ملفات إعداد مضمّنة بالاسم `acceleration.{json,toml}` أو `client.{json,toml}`.
3. إذا لم تتوفر أي من هذه المصادر، تبقى الإعدادات الافتراضية (`AccelerationSettings()`)
   فعالة.

مثال على مقتطف من `acceleration.toml`:

```toml
[accel]
enable_metal = true
merkle_min_leaves_metal = 256
prefer_cpu_sha2_max_leaves_aarch64 = 128
```

إبقاء الحقول فارغة (`nil`) يعني وراثة قيم workspace الافتراضية. يتم تجاهل الأرقام
السالبة، ويؤدي غياب قسم `[accel]` إلى استخدام مسار CPU الحتمي بشكل كامل. عند العمل
على محاكي لا يدعم Metal، يبقى الجسر على المسار التسلسلي (scalar) بهدوء حتى لو طلبت
ملفات الإعداد استخدام Metal.

## اختبارات التكامل

- ستوجد اختبارات التكامل في `Tests/NoritoDemoTests` (سيتم إضافتها بمجرد توافر CI على
  macOS).
- تستخدم الاختبارات السكربتات أعلاه لرفع Torii، ثم تختبر اشتراكات WebSocket،
  أرصدة tokens، ومسارات التحويل باستخدام الحزمة الخاصة بـ Swift.
- تُخزّن سجلات التشغيل ضمن `artifacts/tests/<timestamp>/` إلى جانب metrics ولقطات
  ledger تجريبية.

## فحوصات التماثل (parity) في CI

- نفّذ `make swift-ci` قبل إرسال أي PR يمسّ الديمو أو fixtures المشتركة. هذا الهدف
  ينفذ فحوصات parity للـ fixtures، يتحقق من الـ dashboards، ويولد خلاصات محلية. في
  CI يعتمد نفس الـ workflow على بيانات Buildkite الوصفية
  (`ci/xcframework-smoke:<lane>:device_tag`) كي تُنسب النتائج إلى الـ simulator أو
  StrongBox lane الصحيح؛ تأكد من بقاء هذه البيانات بعد أي تعديل للـ pipelines أو
  وسوم الـ agents.
- عندما يفشل `make swift-ci`، اتبع الخطوات المذكورة في
  `docs/source/swift_parity_triage.md` وراجع مخرجات `mobile_ci` لتحديد أي lane يحتاج
  إلى إعادة توليد أو متابعة incident.

## استكشاف الأخطاء وإصلاحها

- إذا لم يستطع الديمو الاتصال بعقدة Torii، تحقّق من عنوان الـ node وإعدادات TLS.
- تأكد من أن رمز JWT (إن استُخدم) ما زال صالحًا ولم تنتهِ صلاحيته.
- راجع `artifacts/torii.log` بحثًا عن الأخطاء في جهة الخادم.
- بالنسبة لمشكلات WebSocket، افحص نافذة log الخاصة بالعميل أو مخرجات الـ console في
  Xcode.

</div>


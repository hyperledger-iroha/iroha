---
lang: ar
direction: rtl
source: docs/README.md
status: complete
translator: manual
source_hash: 6d050b266fbcc3f0041ec554f89e397e56dc37e5ec3fb093af59e46eb52109e6
source_last_modified: "2025-11-02T04:40:28.809865+00:00"
translation_last_reviewed: 2025-11-14
---

<div dir="rtl">

# توثيق Iroha

للحصول على مقدمة باللغة اليابانية، راجع الملف ‎`README.ja.md`‎ في ‎[`./README.ja.md`](./README.ja.md).

يوفّر هذا المستودع خطّي إصدارات مبنيَّين على نفس الشيفرة المصدرية:
**Iroha 2** (شبكات مُستضافة ذاتيًا) و **Iroha 3 / SORA Nexus** (سجل Nexus
العالمي الموحّد). كلا الخطّين يستخدمان نفس آلة Iroha الافتراضية (IVM) ونفس
أدوات Kotodama، لذلك يمكن إعادة استخدام العقود والبايت‑كود بين بيئات
النشر المختلفة. ما لم يُذكر غير ذلك، تنطبق هذه الوثائق على كلا الخطّين.

في [التوثيق الرئيسي لـ Iroha](https://docs.iroha.tech) ستجد:

- [دليل البدء](https://docs.iroha.tech/get-started/)
- [شروحات الـ SDK](https://docs.iroha.tech/guide/tutorials/) للغات Rust وPython وJavaScript وJava/Kotlin
- [مرجع واجهة برمجة التطبيقات (API)](https://docs.iroha.tech/reference/torii-endpoints.html)

أوراق عمل فنية (Whitepapers) ومواصفات حسب خط الإصدار:

- ‎[ورقة Iroha 2](./source/iroha_2_whitepaper.md) — مواصفات الشبكات المُستضافة ذاتيًا.
- ‎[ورقة Iroha 3 (SORA Nexus)](./source/iroha_3_whitepaper.md) — تصميم المسارات المتعددة (multi‑lane) ومساحات البيانات في Nexus.
- ‎[نموذج البيانات ومواصفات ISI (مستخرجة من التنفيذ)](./source/data_model_and_isi_spec.md) — مرجع سلوكي مُشتق من الشيفرة.
- ‎[أظرف ZK (Norito)](./source/zk_envelopes.md) — أظرف Norito أصلية مبنية على IPA/STARK ومتطلبات جهة التحقق.

## التوطين

توجد ملفات التوطين (stubs) الخاصة باليابانية ‎(`*.ja.*`)‎ والعبرية ‎(`*.he.*`)‎
والإسبانية ‎(`*.es.*`)‎ والبرتغالية ‎(`*.pt.*`)‎ والفرنسية ‎(`*.fr.*`)‎
والروسية ‎(`*.ru.*`)‎ والعربية ‎(`*.ar.*`)‎ والأردية ‎(`*.ur.*`)‎ إلى جوار كل
ملف توثيق أصلي باللغة الإنجليزية. راجع
‎[`docs/i18n/README.md`](./i18n/README.md)‎ لمعرفة كيفية توليد الترجمات
والحفاظ عليها، إضافةً إلى إرشادات إضافة لغات جديدة.

## الأدوات

يحتوي هذا المستودع على توثيق لأدوات Iroha 2:

- ‎[Kagami](../crates/iroha_kagami/README.md)
- وحدات الماكرو ‎[`iroha_derive`](../crates/iroha_derive/)‎ لهياكل الإعدادات (انظر خيار ‎`config_base`‎)
- ‎[خطوات بناء مع تحليل الأداء](./profile_build.md) لاكتشاف نقاط البطء في تجميع ‎`iroha_data_model`‎

## مراجع حزمة Swift / iOS

- ‎[نظرة عامة على Swift SDK](./source/sdk/swift/index.md) — أدوات مساعدة لمسار التنفيذ، ومفاتيح تسريع، وواجهات Connect/WebSocket.
- ‎[دليل البدء السريع لـ Connect](./connect_swift_ios.md) — مسار تعليمي يعتمد على الـ SDK مع مرجع CryptoKit القديم.
- ‎[دليل دمج Xcode](./connect_swift_integration.md) — كيفية دمج NoritoBridgeKit/Connect في التطبيق، مع ChaChaPoly وأدوات مساعدة للإطارات.
- ‎[دليل المساهمين في نموذج SwiftUI](./norito_demo_contributor.md) — تشغيل نموذج iOS مقابل عقدة Torii محلية، مع ملاحظات حول التسريع.
- شغّل ‎`make swift-ci`‎ قبل نشر حزم Swift أو تغييرات Connect؛ يتحقق هذا الهدف من تطابق بيانات الاختبار (fixtures)، وتغذية لوحات المتابعة، وبيانات ‎`ci/xcframework-smoke:<lane>:device_tag`‎ في Buildkite.

## Norito (ترميز التسلسل)

Norito هو ترميز التسلسل (serialization codec) المستخدم في هذا الـ workspace.
لا نستخدم ‎`parity-scale-codec`‎ (SCALE). عندما نقارن مع SCALE في التوثيق أو
الاختبارات، يكون ذلك لأغراض توضيحية فقط؛ جميع المسارات الإنتاجية تستخدم
Norito. توفّر واجهات ‎`norito::codec::{Encode, Decode}`‎ حمولة Norito بلا
ترويسة (“bare”) من أجل تجزئة (hashing) وحجم أصغر على الشبكة — أي أن
التنسيق يبقى Norito وليس SCALE.

الحالة الحالية:

- ترميز وفك ترميز حتمي مع ترويسة ثابتة (قيمة magic، الإصدار، مخطط 16‑بايت،
  الضغط، الطول، CRC64، الأعلام).
- مجموع تدقيق CRC64-XZ مع اختيار مسار التسريع أثناء التشغيل:
  - تعليمة PCLMULQDQ على x86_64 (ضرب بلا حمل) مع اختزال Barrett على كتل حجمها 32 بايت.
  - تعليمة PMULL على aarch64 مع نفس مخطط الطي (folding).
  - مسارات slicing‑by‑8 ومسارات بتية بالكامل لضمان قابلية النقل.
- تلميحات طول مُشفَّرة يوفرها المشتقات (derives) والأنواع الأساسية لتقليل عدد حجز الذاكرة.
- مخازن (buffers) بثّ أكبر (64 KiB) مع تحديث تزايدي لقيمة CRC أثناء فك الترميز.
- ضغط zstd اختياري؛ تسريع الـ GPU مقيّد بعلم تهيئة (feature flag) ويبقى حتميًا.
- اختيار مسار تكيفي: تستدعي ‎`norito::to_bytes_auto(&T)`‎ إما عدم ضغط أو zstd على المعالج أو zstd على الـ GPU (عندما يكون مفعّلًا ومتاحًا) اعتمادًا على حجم الحمولة وقدرات العتاد المخزّنة في الذاكرة. يؤثر هذا الاختيار على الأداء وعلى البايت ‎`compression`‎ في الترويسة فقط؛ ولا يغيّر دلالة البيانات.

راجع ‎`crates/norito/README.md`‎ للاطلاع على اختبارات التكافؤ (parity tests)،
والاختبارات القياسية (benchmarks)، وأمثلة الاستخدام.

ملاحظة: بعض توثيق الأنظمة الفرعية (مثل تسريع IVM والدوائر الصفرية‑المعرفة ZK)
ما زال في حالة تطوّر؛ وعندما تكون الوظيفة غير مكتملة يشير الملف بوضوح إلى
الأعمال المتبقية واتجاه التصميم.

ملاحظات حول ترميز نقطة نهاية الحالة `/status`

- يستخدم Torii في المسار الافتراضي حمولة Norito بلا ترويسة لنقطة النهاية
  `/status` من أجل تقليل الحجم. ينبغي للعميل أن يحاول فك الترميز باستخدام
  Norito أولًا.
- يمكن للخادم أن يعيد JSON عند الطلب؛ وعندها يمكن للعميل الرجوع إلى JSON إذا
  كان ‎`content-type`‎ يساوي ‎`application/json`‎.
- التنسيق المعتمد على الشبكة هو Norito وليس SCALE. تُستخدم واجهات
  ‎`norito::codec::{Encode,Decode}`‎ للنسخة “bare” بلا ترويسة.
</div>

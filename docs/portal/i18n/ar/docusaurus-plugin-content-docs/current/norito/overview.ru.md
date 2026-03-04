---
lang: ar
direction: rtl
source: docs/portal/docs/norito/overview.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

#الطلب Norito

Norito — تسلسل البوابة الثنائية، متاح في جميع Iroha: على المقدمة، مثل هياكل البيانات يقوم بالترميز في المجموعات، وينضم إلى القرص ويتغير من خلال العقود والضيافة. يتم تشغيل الصندوق الموجود في مساحة العمل على Norito بدلاً من `serde`، وذلك من خلال منتجات مختلفة من نفس المنتج.

يلخص هذا البحث الأجزاء الرئيسية ويرتبط بالمواد القانونية.

## الهندسة المعمارية في جميع أنحاء العالم

- **التمويل + الحمولة** – يتم إنشاء كل اتصال Norito باستخدام ميزات الإيداع (الأعلام، المجموع الاختباري)، للحمولة الأكبر التالية. يتم دعم الأغطية والأغطية المعبأة من خلال قطع الغيار.
- **تحديد الترميز** – `norito::codec::{Encode, Decode}` تحقيق الترميز الأساسي. يتم استخدام هذا التخطيط أيضًا من خلال مراقبة الحمولات في المخازن، وإلغاء التحديد، ودعم عمليات التحديد.
- **المخطط + المشتق** – `norito_derive` يُنشئ تحقيق `Encode` و`Decode` و`IntoSchema`. تشتمل الهياكل المعبأة/الملحقات اللاحقة على الترطيب والوصف في `norito.md`.
- **Reester multicodec** – يتم تحديد المعرفات وأنواع المفاتيح والحمولة النافعة في `norito::multicodec`. يتم توصيل الجهاز اللوحي المفوض إلى `multicodec.md`.

## الأدوات| زادا | كوماندا / API | مساعدة |
| --- | --- | --- |
| التحقق من صحة/القسم | `ivm_tool inspect <file>.to` | يعرض إصدار ABI والأعلام ونقاط الدخول. |
| Кодировать/екодировать в Rust | `norito::codec::{Encode, Decode}` | حقيقي لجميع أنواع نموذج البيانات. |
| التشغيل المتداخل JSON | `norito::json::{to_json_pretty, from_json}` | تحديد JSON على أساس Norito. |
| إنشاء المستندات/المواصفات | `norito.md`، `multicodec.md` | أصول التوثيق الأصلية في المستودع الأساسي. |

## عملية التصنيع

1. ** إضافة المشتقات ** – اقترح `#[derive(Encode, Decode, IntoSchema)]` للهياكل الجديدة. قم باختيار أجهزة تسلسلية جيدة، إذا لم تكن هذه حاجة مطلقة.
2. **التحقق من التخطيطات المكتملة** – استخدم `cargo test -p norito` (وميزات المصفوفة المعبأة في `scripts/run_norito_feature_matrix.sh`)، لتتمكن من قراءة ما تظهره التخطيطات الجديدة استقرار.
3. **إعادة إنشاء المستندات** – عندما يتم تغيير الكود، قم بتسجيل `norito.md` والكمبيوتر اللوحي متعدد الترميز، ثم قم بزيارة البوابة (`/reference/norito-codec` وهذا الموضوع).
4. **متابعة الاختبارات Norito-first** – تستخدم اختبارات التكامل المطلوبة مساعدي JSON Norito من `serde_json`, من أجل الترويج لهم، ما يتم بيعه.

## ссылки

- المواصفات: [`norito.md`](https://github.com/hyperledger-iroha/iroha/blob/master/norito.md)
- اسم الترميز المتعدد: [`multicodec.md`](https://github.com/hyperledger-iroha/iroha/blob/master/multicodec.md)
- ميزات المصفوفات النصية: `scripts/run_norito_feature_matrix.sh`
- نماذج التخطيطات المعبأة: `crates/norito/tests/`احصل على هذا البحث مع البداية الرائعة (`/norito/getting-started`) للتجميع العملي لرموز البيتك وتنزيلها، تستخدم الحمولات Norito.
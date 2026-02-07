---
lang: ar
direction: rtl
source: docs/portal/docs/norito/overview.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

#Norito کا جائزہ

Norito Iroha تم استخدام هذا المنتج منذ فترة طويلة: هناك حاجة إلى المزيد من العمل بعد ذلك لقد تم نشر هذه المقالة، وتم نشرها من خلال هذا المحفوظات، والمركز والأطباء الذين يتبادلون هذه المنتجات. يتم استخدام مربع العمل الخاص بالصندوق `serde` باستخدام Norito على نطاق واسع من أقرانه بأكثر من بايت واحد.

لقد حصل على جائزة مكافأة حسية من الخلاصة والتحويل من طرف لنك ديتا.

## نظرة رائعة

- **تحميل + تحميل** – Norito تفاوض الميزات (الأعلام, المجموع الاختباري) بدء التشغيل والمتابعة بعد الحمولة البسيطة. يتم التفاوض على التخطيطات المعبأة وضغط البتات.
- **التكنولوجيا الأخرى** – `norito::codec::{Encode, Decode}` بطاقة الائتمان الناجحة. يتم نسخ الحمولات والرؤوس مرة أخرى وإعادة استخدام التخطيط مرة أخرى بالإضافة إلى التجزئة والتوقيع.
- **اسكيما + مشتقات** – `norito_derive` `Encode`، `Decode` و `IntoSchema` هي تطبيقات بناتنا. أصبحت الهياكل/التسلسلات المعبأة نشطة وتم إنشاء `norito.md`.
- **متعددة المسجلات** – التجزئة وأنواع المفاتيح وواصفات الحمولة النافعة هي المعرفات `norito::multicodec` المتوفرة. يحتوي الجدول الوثائقي `multicodec.md` على المزيد من التفاصيل.

## ٹولنگ| كام | کمانڈ / API | أخبار |
| --- | --- | --- |
| المزيد/سیکشنز کي جانج | `ivm_tool inspect <file>.to` | ABI ورژن، الأعلام ونقاط الدخول دکھاتا. |
| الصدأ انكوڈ/ڈيکوڈ | `norito::codec::{Encode, Decode}` | نموذج البيانات هو كل أقسام الأقسام الأكثر شهرة. |
| التشغيل المتداخل لـ JSON | `norito::json::{to_json_pretty, from_json}` | Norito لويليامز JSON. |
| المستندات/المواصفات بنا | `norito.md`، `multicodec.md` | يمكن أن يكون هناك روت ثوري. |

## ترياتي عمل فلو

1. **مشتق شامل للقراءة** – نوع جديد من المعلومات يقصده `#[derive(Encode, Decode, IntoSchema)]`. لا يزال هناك عدد كبير من الجوائز المرموقة التي لم تعد ضرورية.
2. **التخطيطات المعبأة** – استخدام `cargo test -p norito` (و`scripts/run_norito_feature_matrix.sh` مصفوفة الميزات المعبأة) هي تخطيطات جديدة متاحة.
3. **نسخة ثانية من المستندات** – قم بالتنزيل المجاني عبر `norito.md` وجدول الترميز المتعدد لصفحات الصفحة الرئيسية (`/reference/norito-codec` والإصدارات الأحدث) کریں۔
4. **Norito-السجل الأول للسجل الأول** – السجل الإلكتروني `serde_json` هو Norito الذي يستخدمه مساعدو JSON. تم إصدار كل شيء من هذا المشروع.

## فوری لنکس

- المواصفات: [`norito.md`](https://github.com/hyperledger-iroha/iroha/blob/master/norito.md)
- تعيينات الترميز المتعدد: [`multicodec.md`](https://github.com/hyperledger-iroha/iroha/blob/master/multicodec.md)
- مصفوفة الميزات اسکرپٹ: `scripts/run_norito_feature_matrix.sh`
- مثال التخطيط المعبأ: `crates/norito/tests/`تم تكريمه بواسطة Quickstart (`/norito/getting-started`) الذي يدعم حمولات Norito التي تستخدم الرمز الثانوي المدمج وإرشادات العمل.
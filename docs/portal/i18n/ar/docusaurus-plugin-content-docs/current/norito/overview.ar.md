---
lang: ar
direction: rtl
source: docs/portal/docs/norito/overview.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# نظرة عامة على Norito

Norito هي مخطط التسلسل الثنائي المستخدم عبر Iroha: فهي إثبات كيف تُكشف هياكل البيانات على الشبكة، وحفظها على القرص، وتتبادل بين الاتفاقيات والمضيفين. تعتمد كل الصناديق في مساحة العمل على Norito بدلا من `serde` حتى تنتج على معدات مختلفة بايتات متطابقة.

تلخص هذه النظرة العامة للمكونات الأساسية وربط بالمراجع القياسية.

## لمحة عن البنية

- **الرأس + الحمولة** – يبدأ كل رسالة Norito برأس تفاوض للميزات (flags, checksum) يستخدمه payload خام. تريد التفاوض على خطط المعبأة والضغط عبر بتات الرأس.
- **التميز الحتمي** – `norito::codec::{Encode, Decode}` الترميز العراري. يعاد استخدام التخطيط نفسه عند تعبئة الحمولات في الرؤوس حتى يبقى التجزئة والتوقيع حتميين.
- **المخطط + المشتق** – `norito_derive` يولد تطبيقات `Encode` و`Decode` و`IntoSchema`. تُفعَّل البنى/السلاسل المعبأة الافتراضية والذكورة في `norito.md`.
- **سجل multicodec** – تم تعريف الهاش وأنواع المفاتيح ووصفات الحمولة الموجودة في `norito::multicodec`. يتم التحقق من صلاحيته في `multicodec.md`.

##دوات| | الأمر / API | تعليقات |
| --- | --- | --- |
| فحص الرأس/الاقسام | `ivm_tool inspect <file>.to` | تم تصوير نسخة ABI و الأعلام و نقاط الدخول. |
| الترميز/فك الترميز في الصدأ | `norito::codec::{Encode, Decode}` | منفذ لكل الانواع الأساسية في نموذج البيانات. |
| التشغيل المتداخل JSON | `norito::json::{to_json_pretty, from_json}` | JSON حتمي مدعوم بقيم Norito. |
| إنشاء المستندات/المواصفات | `norito.md`، `multicodec.md` | تظهر مصدر الحقيقة في جذر المستودع. |

## سير عمل التطوير

1. **إضافة مشتقات** – فضل `#[derive(Encode, Decode, IntoSchema)]` للهياكل الجديدة. تجنب المسلسلات اليدوية الا عند الضرورة القصوى.
2. **التحقق من التخطيطات المعبأة** – استخدم `cargo test -p norito` (ومصفوفة معبأة الميزات في `scripts/run_norito_feature_matrix.sh`) للتأكد من ان التخطيطات الجديدة موجودة.
3. **اعادة توليد المستندات** – عند تغير الترميز، تحديث `norito.md` وجدول الترميز المتعدد، ثم تعديل صفحات البوابة (`/reference/norito-codec` وهذا الملخص).
4. **ابقاء المعركة Norito-first** – يجب ان تستخدم السيولة تكامل مساهمات JSON من Norito بدلات من `serde_json` حتى تمارس عبر المسارات نفسها في الإنتاج.

## روابط سريعة

- المواصفة: [`norito.md`](https://github.com/hyperledger-iroha/iroha/blob/master/norito.md)
- تعيينات الترميز المتعدد: [`multicodec.md`](https://github.com/hyperledger-iroha/iroha/blob/master/multicodec.md)
- مميزات سكربت المصفوفة : `scripts/run_norito_feature_matrix.sh`
- امثلة التخطيطات المعبأة: `crates/norito/tests/`

تابع هذه النظرة العامة مع الدليل السريع (`/norito/getting-started`) للحصول على عملية تشغيل لتجميع وتشغيل bytecode الذي يستخدم الحمولات من Norito.
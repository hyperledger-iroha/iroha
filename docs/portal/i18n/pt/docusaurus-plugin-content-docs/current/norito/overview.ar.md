---
lang: pt
direction: ltr
source: docs/portal/docs/norito/overview.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# نظرة عامة على Norito

Norito هي طبقة التسلسل الثنائي المستخدمة عبر Iroha: فهي تحدد كيف تُشفَّر هياكل البيانات على الشبكة، وتُحفظ على القرص، وتتبادل بين العقود والمضيفين. تعتمد كل crate في مساحة العمل على Norito بدلا من `serde` حتى تنتج العقد على عتاد مختلف بايتات متطابقة.

تلخص هذه النظرة العامة المكونات الاساسية وتربط بالمراجع القياسية.

## لمحة عن البنية

- **الرأس + الحمولة** – يبدأ كل message Norito برأس تفاوض للميزات (flags, checksum) يتبعه payload خام. تُتفاوض التخطيطات المعبأة والضغط عبر بتات الرأس.
- **الترميز الحتمي** – `norito::codec::{Encode, Decode}` تنفذ الترميز العاري. يعاد استخدام التخطيط نفسه عند تغليف payloads في الرؤوس حتى يبقى التجزئة والتوقيع حتميين.
- **المخطط + derives** – `norito_derive` يولد تطبيقات `Encode` و`Decode` و`IntoSchema`. تُفعَّل البنى/السلاسل المعبأة افتراضيا ومذكورة في `norito.md`.
- **سجل multicodec** – معرّفات الهاش وأنواع المفاتيح ووصفات payload موجودة في `norito::multicodec`. يتم الحفاظ على الجدول المعتمد في `multicodec.md`.

## الادوات

| المهمة | الامر / API | ملاحظات |
| --- | --- | --- |
| فحص الرأس/الاقسام | `ivm_tool inspect <file>.to` | يعرض نسخة ABI و flags و entrypoints. |
| الترميز/فك الترميز في Rust | `norito::codec::{Encode, Decode}` | منفذة لكل الانواع الاساسية في data model. |
| interop JSON | `norito::json::{to_json_pretty, from_json}` | JSON حتمي مدعوم بقيم Norito. |
| توليد docs/specs | `norito.md`, `multicodec.md` | توثيق مصدر الحقيقة في جذر المستودع. |

## سير عمل التطوير

1. **اضافة derives** – فضل `#[derive(Encode, Decode, IntoSchema)]` للهياكل الجديدة. تجنب المسلسلات اليدوية الا عند الضرورة القصوى.
2. **التحقق من التخطيطات المعبأة** – استخدم `cargo test -p norito` (ومصفوفة packed features في `scripts/run_norito_feature_matrix.sh`) للتأكد من ان التخطيطات الجديدة تبقى مستقرة.
3. **اعادة توليد docs** – عند تغير الترميز، حدّث `norito.md` وجدول multicodec، ثم حدّث صفحات البوابة (`/reference/norito-codec` وهذا الملخص).
4. **ابقاء الاختبارات Norito-first** – يجب ان تستخدم اختبارات التكامل مساعدات JSON من Norito بدلا من `serde_json` حتى تمر عبر المسارات نفسها في الانتاج.

## روابط سريعة

- المواصفة: [`norito.md`](https://github.com/hyperledger-iroha/iroha/blob/master/norito.md)
- تعيينات multicodec: [`multicodec.md`](https://github.com/hyperledger-iroha/iroha/blob/master/multicodec.md)
- سكربت مصفوفة features: `scripts/run_norito_feature_matrix.sh`
- امثلة التخطيطات المعبأة: `crates/norito/tests/`

اربط هذه النظرة العامة مع دليل البدء السريع (`/norito/getting-started`) للحصول على جولة عملية لتجميع وتشغيل bytecode الذي يستخدم payloads من Norito.

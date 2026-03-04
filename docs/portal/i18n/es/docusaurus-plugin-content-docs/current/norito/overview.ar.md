---
lang: es
direction: ltr
source: docs/portal/docs/norito/overview.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# نظرة عامة على Norito

Norito هي طبقة التسلسل الثنائي المستخدمة عبر Iroha: فهي تحدد كيف تُشفَّر هياكل البيانات على الشبكة، وتُحفظ على القرص، وتتبادل بين العقود والمضيفين. تعتمد كل crate في مساحة العمل على Norito بدلا من `serde` حتى تنتج العقد على عتاد مختلف بايتات متطابقة.

تلخص هذه النظرة العامة المكونات الاساسية وتربط بالمراجع القياسية.

## لمحة عن البنية

- **الرأس + الحمولة** – يبدأ كل mensaje Norito برأس تفاوض للميزات (banderas, suma de comprobación) يتبعه carga útil خام. تُتفاوض التخطيطات المعبأة والضغط عبر بتات الرأس.
- **الترميز الحتمي** – `norito::codec::{Encode, Decode}` تنفذ الترميز العاري. يعاد استخدام التخطيط نفسه عند تغليف cargas útiles في الرؤوس حتى يبقى التجزئة والتوقيع حتميين.
- **المخطط + deriva** – `norito_derive` يولد تطبيقات `Encode` و`Decode` و`IntoSchema`. تُفعَّل البنى/السلاسل المعبأة افتراضيا ومذكورة في `norito.md`.
- **سجل multicodec** – معرّفات الهاش وأنواع المفاتيح ووصفات payload موجودة في `norito::multicodec`. يتم الحفاظ على الجدول المعتمد في `multicodec.md`.

## الادوات| المهمة | الامر / API | ملاحظات |
| --- | --- | --- |
| فحص الرأس/الاقسام | `ivm_tool inspect <file>.to` | Utilice ABI, banderas y puntos de entrada. |
| الترميز/فك الترميز في Óxido | `norito::codec::{Encode, Decode}` | منفذة لكل الانواع الاساسية في modelo de datos. |
| JSON de interoperabilidad | `norito::json::{to_json_pretty, from_json}` | JSON está escrito en Norito. |
| Actualizar documentos/especificaciones | `norito.md`, `multicodec.md` | توثيق مصدر الحقيقة في جذر المستودع. |

## سير عمل التطوير

1. **اضافة deriva** – فضل `#[derive(Encode, Decode, IntoSchema)]` للهياكل الجديدة. تجنب المسلسلات اليدوية الا عند الضرورة القصوى.
2. **التحقق من التخطيطات المعبأة** – استخدم `cargo test -p norito` (y funciones empaquetadas en `scripts/run_norito_feature_matrix.sh`) للتأكد من ان التخطيطات الجديدة تبقى مستقرة.
3. **اعادة توليد docs** – عند تغير الترميز، حدّث `norito.md` and multicodec, ثم حدّث صفحات البوابة (`/reference/norito-codec` وهذا الملخص).
4. **ابقاء الاختبارات Norito-first** – يجب ان تستخدم اختبارات التكامل مساعدات JSON من Norito بدلا من `serde_json` حتى تمر عبر المسارات نفسها في الانتاج.

## روابط سريعة

- Traducción: [`norito.md`](https://github.com/hyperledger-iroha/iroha/blob/master/norito.md)
- Código multicódec: [`multicodec.md`](https://github.com/hyperledger-iroha/iroha/blob/master/multicodec.md)
- Características del software: `scripts/run_norito_feature_matrix.sh`
- Nombre del fabricante: `crates/norito/tests/`

اربط هذه النظرة العامة مع دليل البدء السريع (`/norito/getting-started`) للحصول على جولة عملية لتجميع وتشغيل código de bytes cargas útiles de Norito.
---
lang: ar
direction: rtl
source: docs/portal/docs/norito/overview.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# السيرة الذاتية لـ Norito

Norito هو طريقة التسلسل الثنائي المستخدمة في كل شيء Iroha: تحديد كيفية تشفير هياكل البيانات باللون الأحمر، والاستمرار في الديسكو، والتبادل بين العقود والمضيفين. تعتمد كل علبة مساحة العمل على Norito ومكان `serde` حتى يتمكن أقرانهم من الأجهزة من إنتاج وحدات بايت متطابقة مختلفة.

ستستأنف هذه السيرة الذاتية بمزامنة الأجزاء المركزية ومراجعة المراجع القانونية.

## Arquitectura de un vistazo

- **الكابينة + الحمولة** - كل رسالة Norito تبدأ بسؤال تداول الميزات (الأعلام، المجموع الاختباري) تتبع الحمولة دون أن تحيط بها. يتم تنفيذ التخطيطات المعبأة والضغط عبر أجزاء من الرأس.
- **تحديد الترميز** - `norito::codec::{Encode, Decode}` يقوم بتنفيذ قاعدة الترميز. يتم إعادة استخدام نفس التخطيط ليشمل الحمولات الصافية ويرأسها بحيث تحافظ التجزئة والشركة على المحددات.
- **Esquema + derives** - `norito_derive` هي أنواع تنفيذ `Encode` و`Decode` و`IntoSchema`. الهياكل/الملفات المجمعة مؤهلة بسبب العيوب وموثقة في `norito.md`.
- **تسجيل الترميز المتعدد** - يتم تنشيط معرفات التجزئة وأنواع المفاتيح وواصفات الحمولة في `norito::multicodec`. يتم الحفاظ على اللوحة تلقائيًا في `multicodec.md`.

## هيرامينتاس| تاريا | كوماندوز / API | نوتاس |
| --- | --- | --- |
| Inspeccionar cabecera/secciones | `ivm_tool inspect <file>.to` | يعرض إصدار ABI والأعلام ونقاط الدخول. |
| التشفير/فك التشفير في الصدأ | `norito::codec::{Encode, Decode}` | تم تنفيذه لجميع أنواع نماذج البيانات الأساسية. |
| التشغيل المتداخل JSON | `norito::json::{to_json_pretty, from_json}` | تم تحديد JSON حسب القيم Norito. |
| المستندات العامة/المواصفات | `norito.md`، `multicodec.md` | التوثيق ينبع من الحقيقة في مصدر الريبو. |

## تدفق العمل في التطوير

1. **مشتقات مجمعة** - اختر `#[derive(Encode, Decode, IntoSchema)]` لهياكل البيانات الجديدة. تجنب كتابة المسلسلات يدويًا مما هو ضروري تمامًا.
2. **التخطيطات الصحيحة المُعبأة** - الولايات المتحدة الأمريكية `cargo test -p norito` (ومصفوفة الميزات المُعبأة في `scripts/run_norito_feature_matrix.sh`) لضمان الحفاظ على التخطيطات الجديدة.
3. **تجديد المستندات** - عندما تقوم بتغيير الترميز، وتحديث `norito.md` وجدول الترميز المتعدد، قم بتحديث صفحات البوابة الإلكترونية (`/reference/norito-codec` واستأنفها).
4. **الصيانة الاختبارية Norito-first** - تتطلب اختبارات التكامل استخدام المساعدين JSON de Norito في مكان `serde_json` لتشغيل نفس المسارات التي يتم إنتاجها.

## ينير السريع- المواصفات: [`norito.md`](https://github.com/hyperledger-iroha/iroha/blob/master/norito.md)
- الترميز المتعدد المخصص: [`multicodec.md`](https://github.com/hyperledger-iroha/iroha/blob/master/multicodec.md)
- مميزات مصفوفة النص: `scripts/run_norito_feature_matrix.sh`
- نماذج التخطيط المعبأة: `crates/norito/tests/`

يرافق هذا الاستئناف دليل البدء السريع (`/norito/getting-started`) لإعادة التدريب العملي على تجميع وتنفيذ الكود الثانوي الذي يستخدم الحمولات Norito.
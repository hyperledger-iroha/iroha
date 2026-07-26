---
lang: ar
direction: rtl
source: docs/portal/docs/norito/overview.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# عرض مجموعة Norito

Norito هو عبارة عن لوحة تسلسل ثنائية تستخدم في Iroha : وهي عبارة عن تعليق محدد لهياكل البيانات يتم تشفيرها على الملف، وتستمر على القرص، وتتبادل بين العقود والفنادق. يتم الضغط على كل صندوق من مساحة العمل على Norito مما يعني أن على `serde` أزواج على مواد مختلفة تنتج ثمانيات متطابقة.

يمكنك استئناف العناصر الأساسية وإعادة النظر في المراجع الأساسية.

## العمارة في انقلاب فني

- **En-tete + payload** - كل رسالة Norito تبدأ من خلال إجراء مفاوضات حول الميزات (الأعلام، المجموع الاختباري) التي تتبع الحمولة الإجمالية. تعد حزم التخطيطات والضغط أمرًا ضروريًا عبر أجزاء صغيرة.
- **محدد التشفير** - `norito::codec::{Encode, Decode}` يقوم بتنفيذ التشفير الجديد. يتم إعادة استخدام تخطيط الميم عند زيادة الحمولات في الطاولات حتى يتم تحديد التقطيع والتوقيع.
- **مشتقات المخطط +** - `norito_derive` هي نوع من التطبيقات `Encode` و`Decode` و`IntoSchema`. حزم البنيات/التسلسلات نشطة بشكل افتراضي ومستندات في `norito.md`.
- **تسجيل الترميز المتعدد** - المعرفات للتجزئات وأنواع المفاتيح وواصفات الحمولة الحية في `norito::multicodec`. يتم الاحتفاظ بالجدول المرجعي في `multicodec.md`.

##الأدوات| تاش | الأمر / API | ملاحظات |
| --- | --- | --- |
| المفتش l'en-tete/sections | `ivm_tool inspect <file>.to` | قم بعرض إصدار ABI والأعلام ونقاط الدخول. |
| التشفير/فك التشفير في الصدأ | `norito::codec::{Encode, Decode}` | قم بالتنفيذ لجميع أنواع نماذج البيانات الأساسية. |
| التشغيل المتداخل JSON | `norito::json::{to_json_pretty, from_json}` | JSON يحدد العديد من القيم Norito. |
| المستندات/المواصفات العامة | `norito.md`، `multicodec.md` | مصدر التوثيق de verite a la racine du repo. |

## سير العمل في التطوير

1. **إضافة المشتقات** - فضل `#[derive(Encode, Decode, IntoSchema)]` لهياكل البيانات الجديدة. تجنب كتابة المسلسلات بشكل أساسي كما هو ضروري تمامًا.
2. **التحقق من حزم التخطيطات** - استخدم `cargo test -p norito` (ومصفوفة حزم الميزات في `scripts/run_norito_feature_matrix.sh`) لضمان بقاء التخطيطات الجديدة في الاسطبلات.
3. **إعادة إنشاء المستندات** - عند تغيير التشفير، قم بتشغيل `norito.md` يوميًا وجدول الترميز المتعدد، ثم قم بتحرير صفحات البوابة (`/reference/norito-codec` وما إلى ذلك).
4. **تجميع الاختبارات Norito-first** - تستخدم اختبارات التكامل المساعدين JSON Norito بدلاً من `serde_json` من أجل ممارسة الميمات التي يتم إنتاجها.

## منحدرات ليينز- المواصفات: [`norito.md`](https://github.com/hyperledger-iroha/iroha/blob/master/norito.md)
- الترميز المتعدد الإسناد: [`multicodec.md`](https://github.com/hyperledger-iroha/iroha/blob/master/multicodec.md)
- ميزات مصفوفة النص: `scripts/run_norito_feature_matrix.sh`
- أمثلة على حزمة التخطيط: `crates/norito/tests/`

قم بتوصيل هذا الدليل إلى دليل البدء السريع (`/norito/getting-started`) من أجل مجموعة عملية من التجميع وتنفيذ الكود الثانوي باستخدام الحمولات Norito.
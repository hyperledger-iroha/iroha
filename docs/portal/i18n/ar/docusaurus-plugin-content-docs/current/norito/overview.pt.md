---
lang: ar
direction: rtl
source: docs/portal/docs/norito/overview.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# فيساو جيرال دو Norito

Norito وسلسلة التسلسل الثنائي المستخدمة في كل شيء أو Iroha: تعريف كأنظمة البيانات المشفرة على الشبكة، والمستمرة على الديسكو، والمحركات بين العقود والمضيفين. لا تعتمد كل علبة على مساحة عمل على Norito في `serde` حتى تتمكن من نظيرة الأجهزة التي تنتج وحدات بايت مختلفة متطابقة.

هذه السيرة الذاتية عامة باعتبارها أجزاء رئيسية ومرجعية للمراجع القانونية.

## آركيتورا في السيرة الذاتية

- **Cabecalho + payload** - كل رسالة Norito تأتي مع رأس تجارة الميزات (الأعلام، المجموع الاختباري) تتبع الحمولة النقية. تخطيطات مضغوطة وقابلة للتفاوض عبر أجزاء من الرأس.
- **Codificacao Deterministica** - `norito::codec::{Encode, Decode}` تنفذ قاعدة codificacao. يتم تنفيذ نفس التخطيط وإعادة استخدامه من خلال تضمين الحمولات في جهات الاتصال لتتمكن من تجزئة وتجميع المحددات المحددة.
- **المخطط + المشتق** - `norito_derive` هو عبارة عن عمليات تنفيذ لـ `Encode` و`Decode` و`IntoSchema`. يتم تنفيذ الهياكل/التسلسلات حسب الإجراءات والوثائق في `norito.md`.
- **تسجيل الترميز المتعدد** - معرفات التجزئات وأنواع الرموز وواصفات الحمولة الحية في `norito::multicodec`. لوحة مرجعية في `multicodec.md`.

##المنتجات| طريفة | كوماندوز / API | نوتاس |
| --- | --- | --- |
| Inspecionar cabecalho/secoes | `ivm_tool inspect <file>.to` | معظم نسخ ABI والأعلام ونقاط الدخول. |
| التشفير/فك التشفير في الصدأ | `norito::codec::{Encode, Decode}` | تم تنفيذه لجميع أنواع البيانات الأساسية. |
| التشغيل المتداخل JSON | `norito::json::{to_json_pretty, from_json}` | JSON محدد بالقيم Norito. |
| مستندات Gerar/especificacoes | `norito.md`، `multicodec.md` | Documentacao Fonte de Verdade on Raiz do Repo. |

## تدفق العمل في التطوير

1. **مشتقات الإضافة** - اختر `#[derive(Encode, Decode, IntoSchema)]` لتحديث إعدادات البيانات. تجنب المسلسلات المزيفة من أجل الضرورة المطلقة.
2. **التخطيطات الصالحة المضمنة** - استخدم `cargo test -p norito` (وقائمة الميزات المضمنة في `scripts/run_norito_feature_matrix.sh`) لضمان ثبات التخطيطات الجديدة.
3. **إعادة إنشاء المستندات** - عند تعديل التشفير، قم بتحديث `norito.md` ولوحة الترميز المتعدد، ثم قم بالتحديث كصفحات في البوابة (`/reference/norito-codec` وهذا المظهر العام).
4. **الاختبارات التالية Norito-first** - يجب على اختبارات التكامل استخدام المساعدين JSON do Norito في `serde_json` لممارسة نفس طرق الإنتاج.

## روابط سريعة

- خاص: [`norito.md`](https://github.com/hyperledger-iroha/iroha/blob/master/norito.md)
- ترميز متعدد Atribuicoes: [`multicodec.md`](https://github.com/hyperledger-iroha/iroha/blob/master/multicodec.md)
- مميزات مصفوفة النص: `scripts/run_norito_feature_matrix.sh`
- نماذج التخطيط المضمنة: `crates/norito/tests/`اجمع هذه الوجهة العامة مع دليل البدء السريع (`/norito/getting-started`) لتمرير عملية تجميع وتنفيذ الكود الثانوي الذي يستخدم الحمولات Norito.
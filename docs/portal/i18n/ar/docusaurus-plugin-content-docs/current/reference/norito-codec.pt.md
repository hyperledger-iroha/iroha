---
lang: ar
direction: rtl
source: docs/portal/docs/reference/norito-codec.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# مرجع برنامج الترميز Norito

Norito والكاميرا التسلسلية Canon Iroha. جميع الرسائل عبر الأسلاك، والحمولة على القرص، وواجهة برمجة التطبيقات (API) بين المكونات Norito لكي نتوافق مع وحدات البايت المتطابقة مع اختلاف الأجهزة. هذه صفحة السيرة الذاتية كأجزاء رئيسية وقابلة للتخصيص بالكامل في `norito.md`.

## قاعدة التخطيط

| مكون | اقتراح | فونتي |
| --- | --- | --- |
| **الرأس** | Enquadra Payloads مع تجزئة السحر/الإصدار/المخطط، CRC64، الطول وعلامة الضغط؛ يتطلب الإصدار 1 `VERSION_MINOR = 0x00` وتحقق من صحة علامات الرأس مقابل دعم الماسكارا (`0x00` الافتراضي). | `norito::header` - الإصدار `norito.md` ("الرأس والأعلام"، raiz do repositorio) |
| ** رأس الحمولة الصافية ** | حتمية القيم المستخدمة للتجزئة/المقارنة. النقل عبر الأسلاك دائمًا رأس الولايات المتحدة; بايت sem رأس sao apenas internos. | `norito::codec::{Encode, Decode}` |
| **الكومبريسو** | يتم تحديد اختياري (تسريع GPU التجريبي) عبر بايت من ضغط الرأس. | `norito.md`، "تفاوض الضغط" |

سجل علامات التخطيط (البنية المعبأة، والتسلسل المعبأ، ومجموعة بتات الحقل، والأطوال المدمجة) موجود في `norito::header::flags`. أعلام V1 usa `0x00` por padrao mas aceita أعلام الرأس واضحة داخل الماسكارا الداعمة؛ بت desconhecidos sao rejeitados. `norito::header::Flags` ومتابعة البحث الداخلي والأحداث المستقبلية.## دعم اشتقاق

`norito_derive` يشتق `Encode` و`Decode` و`IntoSchema` والمساعدين JSON. مبادئ المؤتمرات:

- يشتق geram caminhos AoS e معبأة؛ v1 usa تخطيط AoS por Padrao (الأعلام `0x00`) مع الحد من الأعلام المختارة حسب المتغيرات المعبأة. قم بالتنفيذ في `crates/norito_derive/src/derive_struct.rs`.
- الموارد التي تحدد التخطيط (`packed-struct`، `packed-seq`، `compact-len`) يمكن الاشتراك فيها عبر علامات الرأس ويجب أن يتم تشفيرها/فك تشفيرها بشكل متسق بين أقرانها.
- مساعدات JSON (`norito::json`) تستخدم لتحديد JSON في Norito لواجهات برمجة التطبيقات المفتوحة. استخدم `norito::json::{to_json_pretty, from_json}` - nunca `serde_json`.

## ترميز متعدد وجداول معرفات

Norito يحافظ على خصائص الترميز المتعدد الخاصة به في `norito::multicodec`. لوحة مرجعية (تجزئات وأنواع رؤوس وواصفات الحمولة) وتصفّح في `multicodec.md` في رأس المستودع. عند إضافة معرف جديد:

1. قم بتفعيل `norito::multicodec::registry`.
2. قم بوضع اللوحة على `multicodec.md`.
3. قم بإعادة إنشاء الارتباطات النهائية (Python/Java) من خلال استهلاكها أو خريطتها.

## إعادة إنشاء المستندات والتركيبات

من خلال بوابة تستضيف ملخصًا احترافيًا، استخدمها كخطوط Markdown upstream كخط أخضر:

- **المواصفات**: `norito.md`
- **جدول الترميز المتعدد**: `multicodec.md`
- **المقاييس**: `crates/norito/benches/`
- **الاختبارات الذهبية**: `crates/norito/tests/`عند دخول Docusaurus تلقائيًا، سيتم تحديث البوابة عبر برنامج مزامنة نصي (مرسوم على `docs/portal/scripts/`) يضيف البيانات من هذه الملفات. بعد ذلك، قم بحفظ هذه الصفحة يدويًا باستمرار حتى يتم تعديل المواصفات.
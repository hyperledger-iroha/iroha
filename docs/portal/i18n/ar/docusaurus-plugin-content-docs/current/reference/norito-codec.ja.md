---
lang: ja
direction: ltr
source: docs/portal/i18n/ar/docusaurus-plugin-content-docs/current/reference/norito-codec.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ba62195b82ad565c52eec14c5a7adfd7166fe66263dc5c3af4bc54c72ead256c
source_last_modified: "2026-01-18T05:31:56+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: ar
direction: rtl
source: docs/portal/docs/reference/norito-codec.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# مرجع ترميز Norito

Norito هو طبقة التسلسل القياسية في Iroha. كل رسالة on-wire وكل payload على القرص وكل API بين المكونات يستخدم Norito حتى تتفق العقد على نفس البايتات حتى مع اختلاف العتاد. هذه الصفحة تلخص الاجزاء المتحركة وتشير الى المواصفة الكاملة في `norito.md`.

## البنية الاساسية

| المكون | الغرض | المصدر |
| --- | --- | --- |
| **الرأس** | يؤطر payloads مع magic/version/schema hash و CRC64 والطول وعلامة الضغط؛ v1 يتطلب `VERSION_MINOR = 0x00` ويتحقق من header flags مقابل القناع المدعوم (الافتراضي `0x00`). | `norito::header` — راجع `norito.md` ("Header & Flags"، جذر المستودع) |
| **Payload بدون رأس** | ترميز قيم حتمي يستخدم للـ hashing/المقارنة. النقل on-wire يستخدم دائما رأسا؛ البايتات بدون رأس داخلية فقط. | `norito::codec::{Encode, Decode}` |
| **الضغط** | Zstd اختياري (وتسريع GPU تجريبي) يتم اختياره عبر بايت الضغط في الرأس. | `norito.md`, “Compression negotiation” |

سجل flags الخاص بالـ layout (packed-struct, packed-seq, field bitset, compact lengths) موجود في `norito::header::flags`. تستخدم V1 افتراضيا flags `0x00` لكنها تقبل flags صريحة ضمن القناع المدعوم؛ يتم رفض البتات غير المعروفة. يتم الاحتفاظ بـ `norito::header::Flags` للفحص الداخلي والنسخ المستقبلية.

## دعم derive

يوفر `norito_derive` مشتقات `Encode`, `Decode`, `IntoSchema` ومساعدات JSON. اهم الاعراف:

- المشتقات تولد مسارات AoS و packed؛ v1 يستخدم تخطيط AoS افتراضيا (flags `0x00`) ما لم تختَر header flags متغيرات packed. التنفيذ موجود في `crates/norito_derive/src/derive_struct.rs`.
- الميزات المؤثرة على التخطيط (`packed-struct`, `packed-seq`, `compact-len`) هي opt-in عبر header flags ويجب ترميزها/فك ترميزها بشكل متسق عبر peers.
- مساعدات JSON (`norito::json`) توفر JSON حتميا مدعوما بـ Norito لواجهات API العامة. استخدم `norito::json::{to_json_pretty, from_json}` — ولا تستخدم `serde_json`.

## Multicodec وجداول المعرفات

يحتفظ Norito بتعيينات multicodec في `norito::multicodec`. الجدول المرجعي (hashes، انواع المفاتيح، واصفات payload) محفوظ في `multicodec.md` بجذر المستودع. عند اضافة معرف جديد:

1. حدث `norito::multicodec::registry`.
2. وسع الجدول في `multicodec.md`.
3. اعد توليد bindings downstream (Python/Java) اذا كانت تستهلك الخريطة.

## اعادة توليد docs و fixtures

مع استضافة البوابة حاليا لملخص وصفي، استخدم مصادر Markdown الاصلية كمصدر للحقيقة:

- **Spec**: `norito.md`
- **جدول multicodec**: `multicodec.md`
- **Benchmarks**: `crates/norito/benches/`
- **Golden tests**: `crates/norito/tests/`

عندما تعمل اتوماتة Docusaurus، سيتم تحديث البوابة عبر سكربت sync (متابع في `docs/portal/scripts/`) الذي يسحب البيانات من هذه الملفات. حتى ذلك الحين، حافظ على مواءمة هذه الصفحة يدويا كلما تغيرت المواصفة.

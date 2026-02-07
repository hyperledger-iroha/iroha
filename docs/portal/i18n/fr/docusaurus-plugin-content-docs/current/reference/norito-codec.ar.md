---
lang: fr
direction: ltr
source: docs/portal/docs/reference/norito-codec.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# مرجع ترميز Norito

Norito et Iroha. La charge utile et la charge utile sur fil sont également liées à l'API pour les applications Norito. حتى مع اختلاف العتاد. Il s'agit d'une procédure à suivre pour `norito.md`.

## البنية الاساسية

| المكون | الغرض | المصدر |
| --- | --- | --- |
| **الرأس** | Il existe des charges utiles telles que le hachage magic/version/schema et CRC64 ainsi que la fonction de hachage. v1 est compatible avec `VERSION_MINOR = 0x00` et les drapeaux d'en-tête sont associés à la fonction `0x00`. | `norito::header` — Voir `norito.md` (« En-tête et drapeaux » , جذر المستودع) |
| **Charge utile pour رأس** | Il s'agit d'une méthode de hachage/المقارنة. النقل on-wire يستخدم دائما رأسا؛ البايتات بدون رأس داخلية فقط. | `norito::codec::{Encode, Decode}` |
| **ضغط** | Zstd اختياري (وتسريع GPU تجريبي) يتم اختياره عبر بايت الضغط في الرأس. | `norito.md`, « Négociation de compression » |

Les indicateurs de mise en page (packed-struct, packed-seq, field bitset, compact lengths) sont utilisés dans `norito::header::flags`. تستخدم V1 افتراضيا flags `0x00` لكنها تقبل flags صريحة ضمن القناع المدعوم؛ يتم رفض البتات غير المعروفة. يتم الاحتفاظ بـ `norito::header::Flags` للفحص الداخلي والنسخ المستقبلية.

## دعم dériver

`norito_derive` contient `Encode`, `Decode`, `IntoSchema` et JSON. اهم الاعراف :- المشتقات تولد مسارات AoS et emballé؛ v1 contient les drapeaux AoS (drapeaux `0x00`) pour les drapeaux d'en-tête emballés. La référence est `crates/norito_derive/src/derive_struct.rs`.
- Les options de prise en charge (`packed-struct`, `packed-seq`, `compact-len`) pour les drapeaux d'en-tête opt-in et les options de paiement/en-tête ترميزها بشكل متسق عبر pairs.
- JSON (`norito::json`) est compatible avec JSON via Norito pour l'API. `norito::json::{to_json_pretty, from_json}` — et `serde_json`.

## Multicodec et fonctionnalités

Utilisez Norito pour multicodec dans `norito::multicodec`. La charge utile (hashes) est appliquée à la charge utile) par `multicodec.md`. عند اضافة معرف جديد:

1. Voir `norito::multicodec::registry`.
2. وسع الجدول في `multicodec.md`.
3. Utiliser les liaisons en aval (Python/Java) pour les liaisons en aval.

## اعادة توليد docs et luminaires

Si vous avez besoin d'aide, vous pouvez utiliser Markdown pour vous aider :

- **Spécification** : `norito.md`
- **Multicodec** : `multicodec.md`
- **Références** : `crates/norito/benches/`
- **Tests d'or** : `crates/norito/tests/`

عندما تعمل اتوماتة Docusaurus, سيتم تحديث البوابة عبر سكربت sync (متابع في `docs/portal/scripts/`) الذي يسحب البيانات من هذه الملفات. حتى ذلك الحين، حافظ على مواءمة هذه الصفحة يدويا كلما تغيرت المواصفة.
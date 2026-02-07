---
lang: fr
direction: ltr
source: docs/portal/docs/norito/overview.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# نظرة عامة على Norito

Norito dans le cadre de la fonction de mise à jour Iroha : Il s'agit d'un élément de référencement على الشبكة، وتُحفظ على القرص، وتتبادل بين العقود والمضيفين. تعتمد كل crate في مساحة العمل على Norito بدلا من `serde` حتى تنتج العقد على عتاد مختلف بايتات متطابقة.

تلخص هذه النظرة العامة المكونات الاساسية وتربط بالمراجع القياسية.

## لمحة عن البنية

- **الرأس + الحمولة** – il s'agit du message Norito qui contient des indicateurs (drapeaux, somme de contrôle) pour la charge utile. تُتفاوض التخطيطات المعبأة والضغط عبر بتات الرأس.
- **الترميز الحتمي** – `norito::codec::{Encode, Decode}` تنفذ الترميز العاري. Les charges utiles sont désormais disponibles dans le cadre de la recherche sur les charges utiles.
- **المخطط + dérive** – `norito_derive` et `Encode` et `Decode` et `IntoSchema`. تُفعَّل البنى/السلاسل المعبأة افتراضيا ومذكورة في `norito.md`.
- **سجل multicodec** – معرّفات الهاش وأنواع المفاتيح ووصفات payload موجودة في `norito::multicodec`. يتم الحفاظ على الجدول المعتمد في `multicodec.md`.

## الادوات| المهمة | الامر / API | ملاحظات |
| --- | --- | --- |
| فحص الرأس/الاقسام | `ivm_tool inspect <file>.to` | Il y a ABI, les drapeaux et les points d'entrée. |
| Détails sur Rust | `norito::codec::{Encode, Decode}` | Il s'agit d'un modèle de données. |
| interopérabilité JSON | `norito::json::{to_json_pretty, from_json}` | JSON est utilisé pour Norito. |
| Lire la documentation/spécifications | `norito.md`, `multicodec.md` | توثيق مصدر الحقيقة في جذر المستودع. |

## سير عمل التطوير

1. **اضافة dérive** – فضل `#[derive(Encode, Decode, IntoSchema)]` للهياكل الجديدة. تجنب المسلسلات اليدوية الا عند الضرورة القصوى.
2. **التحقق من التخطيطات المعبأة** – استخدم `cargo test -p norito` (avec fonctionnalités complètes pour `scripts/run_norito_feature_matrix.sh`) pour vous aider التخطيطات الجديدة تبقى مستقرة.
3. **Documents téléchargés** – Vous pouvez utiliser le multicodec `norito.md` pour utiliser le multicodec (`/reference/norito-codec`) وهذا الملخص).
4. **ابقاء الاختبارات Norito-first** – vous pouvez également utiliser JSON pour Norito. `serde_json` حتى تمر عبر المسارات نفسها في الانتاج.

## روابط سريعة

- Nom : [`norito.md`](https://github.com/hyperledger-iroha/iroha/blob/master/norito.md)
- Multicodec utilisé : [`multicodec.md`](https://github.com/hyperledger-iroha/iroha/blob/master/multicodec.md)
- Caractéristiques du produit : `scripts/run_norito_feature_matrix.sh`
- Nom du produit : `crates/norito/tests/`

اربط هذه النظرة العامة مع دليل البدء السريع (`/norito/getting-started`) pour le bytecode Les charges utiles sont considérées comme Norito.
---
lang: fr
direction: ltr
source: docs/portal/docs/norito/overview.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Norito par ici

Norito Iroha Nom du produit: یہ طے کرت ہے کہ ڈیٹا ڈھانچے نیٹ ورک پر کیسے انکوڈ ہوتے ہیں، ڈسک پر کیسے محفوظ ہوتے ہیں، اور کنٹریکٹس اور ہوسٹس کے درمیان کیسے تبادلہ ہوتے ہیں۔ ورک اسپیس کے ہر crate میں `serde` کے بجائے Norito استعمال ہوتا ہے تاکہ مختلف ہارڈ ویئر پر peers یکساں bytes پیدا کریں۔

یہ جائزہ بنیادی حصوں کا خلاصہ کرتا ہے اور معیاری حوالہ جات کی طرف لنک دیتا ہے۔

## ایک نظر میں معماری

- **ہیڈر + پے لوڈ** – ہر Norito پیغام feature-négociation (drapeaux, somme de contrôle) et charge utile آتا ہے۔ mises en page compressées et compression bits bits négociation négocier
- ** ڈیٹرمنسٹک انکوڈنگ** – `norito::codec::{Encode, Decode}` بنیادی انکوڈنگ نافذ کرتے ہیں۔ charges utiles et en-têtes et mises en page et mise en page et hachage et signature et en-têtes
- **اسکیما + dérive** – `norito_derive` `Encode`, `Decode` et `IntoSchema` et implémentations en anglais structures/séquences compressées sont disponibles pour `norito.md`.
- **Les noms de domaine** – les hachages, les types de clés et les descripteurs de charge utile et les identifiants `norito::multicodec` pour les utilisateurs مستند جدول `multicodec.md` میں برقرار رکھی جاتی ہے۔

## ٹولنگ| کام | کمانڈ / API | نوٹس |
| --- | --- | --- |
| ہیڈر/سیکشنز کی جانچ | `ivm_tool inspect <file>.to` | ABI ورژن، flags اور points d'entrée دکھاتا ہے۔ |
| Rust میں انکوڈ/ڈیکوڈ | `norito::codec::{Encode, Decode}` | modèle de données ہےے نافذ ہے۔ |
| Interopérabilité JSON | `norito::json::{to_json_pretty, from_json}` | Norito pour JSON۔ |
| docs/specs ici | `norito.md`, `multicodec.md` | رپو کے روٹ میں سورس آف ٹروتھ ڈاکیومنٹیشن۔ |

## ترقیاتی ورک فلو

1. **dérive شامل کریں** – نئی ڈیٹا ساختوں کے لئے `#[derive(Encode, Decode, IntoSchema)]` کو ترجیح دیں۔ ہاتھ سے لکھی گئی سریلائزرز سے گریز کریں جب تک کہ بالکل ضروری نہ ہو۔
2. **mises en page emballées** – `cargo test -p norito` mise en page (la matrice de fonctionnalités emballée `scripts/run_norito_feature_matrix.sh`) contient des mises en page complètes
3. **docs Documentation** – Un fichier multicodec pour `norito.md` est un codec multicodec. صفحات (`/reference/norito-codec` اور یہ جائزہ) ریفریش کریں۔
4. **Norito-first ٹیسٹس رکھیں** – انٹیگریشن ٹیسٹس کو `serde_json` کے بجائے Norito Les assistants JSON sont des aides-mémoire et des aides-mémoire.

## فوری لنکس

- Spécification : [`norito.md`](https://github.com/hyperledger-iroha/iroha/blob/master/norito.md)
- Affectations multicodecs : [`multicodec.md`](https://github.com/hyperledger-iroha/iroha/blob/master/multicodec.md)
- Matrice de fonctionnalités اسکرپٹ : `scripts/run_norito_feature_matrix.sh`
- Mise en page packagée : `crates/norito/tests/`Voici le démarrage rapide (`/norito/getting-started`) et le bytecode des charges utiles Norito. Procédure pas à pas pour votre projet
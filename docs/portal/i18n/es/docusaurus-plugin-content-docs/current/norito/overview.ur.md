---
lang: es
direction: ltr
source: docs/portal/docs/norito/overview.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Norito کا جائزہ

Norito Iroha میں استعمال ہونے والی بائنری سیریلائزیشن کی تہہ ہے: یہ طے کرتی ہے کہ ڈیٹا ڈھانچے نیٹ ورک پر کیسے انکوڈ ہوتے ہیں، ڈسک پر کیسے محفوظ ہوتے ہیں، اور کنٹریکٹس اور ہوسٹس کے درمیان کیسے تبادلہ ہوتے ہیں۔ ورک اسپیس کے ہر crate میں `serde` کے بجائے Norito استعمال ہوتا ہے تاکہ مختلف ہارڈ ویئر پر pares یکساں bytes پیدا کریں۔

یہ جائزہ بنیادی حصوں کا خلاصہ کرتا ہے اور معیاری حوالہ جات کی طرف لنک دیتا ہے۔

## ایک نظر میں معماری

- **ہیڈر + پے لوڈ** – ہر Norito پیغام característica de negociación ہیڈر (banderas, suma de comprobación) سے شروع ہوتا ہے جس کے بعد سادہ carga útil آتا ہے۔ diseños empaquetados اور compresión ہیڈر کے bits کے ذریعے negociar ہوتے ہیں۔
- **ڈیٹرمنسٹک انکوڈنگ** – `norito::codec::{Encode, Decode}` بنیادی انکوڈنگ نافذ کرتے ہیں۔ cargas útiles کو encabezados میں لپیٹتے وقت بھی وہی diseño دوبارہ استعمال ہوتا ہے تاکہ hash اور firma ڈیٹرمنسٹک رہیں۔
- **اسکیما + deriva** – `norito_derive` `Encode`، `Decode` اور `IntoSchema` کی implementaciones بناتا ہے۔ estructuras/secuencias empaquetadas
- **ملٹی کوڈیک رجسٹری** – hashes, tipos de claves, descriptores de carga útil, identificadores `norito::multicodec` میں موجود ہیں۔ مستند جدول `multicodec.md` میں برقرار رکھی جاتی ہے۔

## ٹولنگ| کام | کمانڈ / API | نوٹس |
| --- | --- | --- |
| ہیڈر/سیکشنز کی جانچ | `ivm_tool inspect <file>.to` | ABI ورژن، banderas اور puntos de entrada دکھاتا ہے۔ |
| Rust میں انکوڈ/ڈیکوڈ | `norito::codec::{Encode, Decode}` | modelo de datos کے تمام بنیادی اقسام کے لئے نافذ ہے۔ |
| Interoperabilidad JSON | `norito::json::{to_json_pretty, from_json}` | Norito Archivo de formato JSON |
| documentos/especificaciones بنانا | `norito.md`, `multicodec.md` | رپو کے روٹ میں سورس آف ٹروتھ ڈاکیومنٹیشن۔ |

## ترقیاتی ورک فلو

1. **deriva شامل کریں** – نئی ڈیٹا ساختوں کے لئے `#[derive(Encode, Decode, IntoSchema)]` کو ترجیح دیں۔ ہاتھ سے لکھی گئی سریلائزرز سے گریز کریں جب تک کہ بالکل ضروری نہ ہو۔
2. **diseños empaquetados کی توثیق** – `cargo test -p norito` استعمال کریں (اور `scripts/run_norito_feature_matrix.sh` میں matriz de características empaquetadas) تاکہ نئی diseños مستحکم رہیں۔
3. **docs دوبارہ بنائیں** – جب انکوڈنگ بدلتی ہے تو `norito.md` اور multicodec جدول اپ ڈیٹ کریں، پھر پورٹل صفحات (`/reference/norito-codec` اور یہ جائزہ) ریفریش کریں۔
4. **Norito-primer ٹیسٹس برقرار رکھیں** – انٹیگریشن ٹیسٹس کو `serde_json` کے بجائے Norito Asistentes JSON

## فوری لنکس

- Especificación: [`norito.md`](https://github.com/hyperledger-iroha/iroha/blob/master/norito.md)
- Asignaciones multicódec: [`multicodec.md`](https://github.com/hyperledger-iroha/iroha/blob/master/multicodec.md)
- Matriz de funciones: `scripts/run_norito_feature_matrix.sh`
- Diseño empaquetado Modelo: `crates/norito/tests/`Inicio rápido de inicio rápido (`/norito/getting-started`) Cargas útiles de Norito Código de bytes کو کمپائل اور چلانے کا عملی tutorial مل سکے۔
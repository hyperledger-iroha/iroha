---
lang: ur
direction: rtl
source: docs/portal/docs/norito/overview.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Norito کا جائزہ

Norito Iroha میں استعمال ہونے والی بائنری سیریلائزیشن کی تہہ ہے: یہ طے کرتی ہے کہ ڈیٹا ڈھانچے نیٹ ورک پر کیسے انکوڈ ہوتے ہیں، ڈسک پر کیسے محفوظ ہوتے ہیں، اور کنٹریکٹس اور ہوسٹس کے درمیان کیسے تبادلہ ہوتے ہیں۔ ورک اسپیس کے ہر crate میں `serde` کے بجائے Norito استعمال ہوتا ہے تاکہ مختلف ہارڈ ویئر پر peers یکساں bytes پیدا کریں۔

یہ جائزہ بنیادی حصوں کا خلاصہ کرتا ہے اور معیاری حوالہ جات کی طرف لنک دیتا ہے۔

## ایک نظر میں معماری

- **ہیڈر + پے لوڈ** – ہر Norito پیغام feature-negotiation ہیڈر (flags, checksum) سے شروع ہوتا ہے جس کے بعد سادہ payload آتا ہے۔ packed layouts اور compression ہیڈر کے bits کے ذریعے negotiate ہوتے ہیں۔
- **ڈیٹرمنسٹک انکوڈنگ** – `norito::codec::{Encode, Decode}` بنیادی انکوڈنگ نافذ کرتے ہیں۔ payloads کو headers میں لپیٹتے وقت بھی وہی layout دوبارہ استعمال ہوتا ہے تاکہ hashing اور signing ڈیٹرمنسٹک رہیں۔
- **اسکیما + derives** – `norito_derive` `Encode`، `Decode` اور `IntoSchema` کی implementations بناتا ہے۔ packed structs/sequences ڈیفالٹ طور پر فعال ہیں اور `norito.md` میں دستاویزی ہیں۔
- **ملٹی کوڈیک رجسٹری** – hashes، key types اور payload descriptors کے identifiers `norito::multicodec` میں موجود ہیں۔ مستند جدول `multicodec.md` میں برقرار رکھی جاتی ہے۔

## ٹولنگ

| کام | کمانڈ / API | نوٹس |
| --- | --- | --- |
| ہیڈر/سیکشنز کی جانچ | `ivm_tool inspect <file>.to` | ABI ورژن، flags اور entrypoints دکھاتا ہے۔ |
| Rust میں انکوڈ/ڈیکوڈ | `norito::codec::{Encode, Decode}` | data model کے تمام بنیادی اقسام کے لئے نافذ ہے۔ |
| JSON interop | `norito::json::{to_json_pretty, from_json}` | Norito ویلیوز پر مبنی ڈیٹرمنسٹک JSON۔ |
| docs/specs بنانا | `norito.md`, `multicodec.md` | رپو کے روٹ میں سورس آف ٹروتھ ڈاکیومنٹیشن۔ |

## ترقیاتی ورک فلو

1. **derives شامل کریں** – نئی ڈیٹا ساختوں کے لئے `#[derive(Encode, Decode, IntoSchema)]` کو ترجیح دیں۔ ہاتھ سے لکھی گئی سریلائزرز سے گریز کریں جب تک کہ بالکل ضروری نہ ہو۔
2. **packed layouts کی توثیق** – `cargo test -p norito` استعمال کریں (اور `scripts/run_norito_feature_matrix.sh` میں packed feature matrix) تاکہ نئی layouts مستحکم رہیں۔
3. **docs دوبارہ بنائیں** – جب انکوڈنگ بدلتی ہے تو `norito.md` اور multicodec جدول اپ ڈیٹ کریں، پھر پورٹل صفحات (`/reference/norito-codec` اور یہ جائزہ) ریفریش کریں۔
4. **Norito-first ٹیسٹس برقرار رکھیں** – انٹیگریشن ٹیسٹس کو `serde_json` کے بجائے Norito کے JSON helpers استعمال کرنے چاہئیں تاکہ وہی راستے چلیں جو پروڈکشن میں ہوتے ہیں۔

## فوری لنکس

- Specification: [`norito.md`](https://github.com/hyperledger-iroha/iroha/blob/master/norito.md)
- Multicodec assignments: [`multicodec.md`](https://github.com/hyperledger-iroha/iroha/blob/master/multicodec.md)
- Feature matrix اسکرپٹ: `scripts/run_norito_feature_matrix.sh`
- Packed-layout مثالیں: `crates/norito/tests/`

اس جائزے کو quickstart گائیڈ (`/norito/getting-started`) کے ساتھ ملائیں تاکہ Norito payloads استعمال کرنے والے bytecode کو کمپائل اور چلانے کا عملی walkthrough مل سکے۔

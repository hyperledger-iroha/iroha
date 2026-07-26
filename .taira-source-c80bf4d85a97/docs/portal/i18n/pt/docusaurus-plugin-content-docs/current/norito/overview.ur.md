---
lang: pt
direction: ltr
source: docs/portal/docs/norito/overview.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Norito کا جائزہ

Norito Iroha میں استعمال ہونے والی بائنری سیریلائزیشن کی تہہ ہے: یہ طے کرتی ہے کہ ڈیٹا ڈھانچے نیٹ ورک پر کیسے انکوڈ ہوتے ہیں, ڈسک پر کیسے محفوظ ہوتے ہیں, اور کنٹریکٹس اور ہوسٹس کے درمیان کیسے تبادلہ ہوتے ہیں۔ A caixa de papelão `serde` é a caixa Norito que pode ser usada ہارڈ ویئر پر peers یکساں bytes پیدا کریں۔

یہ جائزہ بنیادی حصوں کا خلاصہ کرتا ہے اور معیاری حوالہ جات کی طرف لنک دیتا ہے۔

## ایک نظر میں معماری

- **ہیڈر + پے لوڈ** – ہر Norito پیغام feature-negociation ہیڈر (flags, checksum) سے شروع ہوتا ہے جس کے بعد Carga útil layouts compactados اور compressão ہیڈر کے bits کے ذریعے negociar ہوتے ہیں۔
- **ڈیٹرمنسٹک انکوڈنگ** – `norito::codec::{Encode, Decode}` بنیادی انکوڈنگ نافذ کرتے ہیں۔ cargas úteis e cabeçalhos رہیں۔
- **اسکیما + deriva** – `norito_derive` `Encode`, `Decode` e `IntoSchema` کی implementações بناتا ہے۔ estruturas/sequências compactadas
- **ملٹی کوڈیک رجسٹری** – hashes, tipos de chave e descritores de carga útil کے identificadores `norito::multicodec` میں موجود ہیں۔ مستند جدول `multicodec.md` میں برقرار رکھی جاتی ہے۔

## ٹولنگ

| کام | کمانڈ / API | Não |
| --- | --- | --- |
| ہیڈر/سیکشنز کی جانچ | `ivm_tool inspect <file>.to` | ABI ورژن, sinalizadores e pontos de entrada دکھاتا ہے۔ |
| Ferrugem میں انکوڈ/ڈیکوڈ | `norito::codec::{Encode, Decode}` | modelo de dados |
| Interoperabilidade JSON | `norito::json::{to_json_pretty, from_json}` | Norito é um arquivo JSON۔ |
| documentos/especificações Inglês | `norito.md`, `multicodec.md` | رپو کے روٹ میں سورس آف ٹروتھ ڈاکیومنٹیشن۔ |

## ترقیاتی ورک فلو

1. **deriva شامل کریں** – نئی ڈیٹا ساختوں کے لئے `#[derive(Encode, Decode, IntoSchema)]` کو ترجیح دیں۔ ہاتھ سے لکھی گئی سریلائزرز سے گریز کریں جب تک کہ بالکل ضروری نہ ہو۔
2. **layouts compactados کی توثیق** – `cargo test -p norito` استعمال کریں (اور `scripts/run_norito_feature_matrix.sh` میں matriz de recursos compactados) تاکہ نئی layouts مستحکم رہیں۔
3. **docs دوبارہ بنائیں** – جب انکوڈنگ بدلتی ہے تو `norito.md` اور multicodec جدول اپ ڈیٹ کریں, پھر پورٹل صفحات (`/reference/norito-codec` اور یہ جائزہ) ریفریش کریں۔
4. **Norito-first ٹیسٹس برقرار رکھیں** – انٹیگریشن ٹیسٹس کو `serde_json` کے بجائے Norito کے Ajudantes JSON ہیں۔

## فوری لنکس

Especificação: [`norito.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/norito.md)
- Atribuições multicodec: [`multicodec.md`](https://github.com/hyperledger-iroha/iroha/blob/master/multicodec.md)
- Matriz de recursos `scripts/run_norito_feature_matrix.sh`
Modelo de layout compactado: `crates/norito/tests/`

O início rápido do início rápido (`/norito/getting-started`) é o início rápido das cargas úteis Norito. bytecode کو کمپائل اور چلانے کا عملی passo a passo مل سکے۔
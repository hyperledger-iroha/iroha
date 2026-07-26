---
lang: ur
direction: rtl
source: docs/portal/versioned_docs/version-2025-q2/reference/norito-codec.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d01c97e1ee8c36da643f14b5f81dd8d246315f6ce8de11a8fdb6eba757d8369b
source_last_modified: "2025-11-04T12:24:28.218431+00:00"
translation_last_reviewed: 2026-01-30
---

# Norito codec reference

Norito Iroha کی canonical serialization layer ہے۔ ہر on-wire message، on-disk payload، اور cross-component API Norito استعمال کرتا ہے تاکہ nodes مختلف hardware پر بھی ایک جیسے bytes پر متفق رہیں۔ یہ صفحہ اہم حصے خلاصہ کرتا ہے اور مکمل specification کے لئے `norito.md` کی طرف اشارہ کرتا ہے۔

## Core layout

| Component | Purpose | Source |
| --- | --- | --- |
| **Header** | magic/version/schema hash، CRC64، length، اور compression tag کے ساتھ payloads کو فریم کرتا ہے؛ v1 میں `VERSION_MINOR = 0x00` ضروری ہے اور header flags کو supported mask کے مقابل validate کیا جاتا ہے (default `0x00`). | `norito::header` — `norito.md` ("Header & Flags", repository root) دیکھیں |
| **Bare payload** | hashing/موازنہ کیلئے deterministic value encoding۔ On-wire transport ہمیشہ header استعمال کرتا ہے؛ bare bytes صرف اندرونی استعمال کیلئے ہیں۔ | `norito::codec::{Encode, Decode}` |
| **Compression** | Optional Zstd (اور experimental GPU acceleration) جو header کے compression byte کے ذریعے منتخب ہوتی ہے۔ | `norito.md`, “Compression negotiation” |

layout flag registry (packed-struct, packed-seq, field bitset, compact lengths) `norito::header::flags` میں ہے۔ V1 defaults کے طور پر flags `0x00` استعمال کرتا ہے مگر supported mask کے اندر explicit header flags قبول کرتا ہے؛ unknown bits رد کئے جاتے ہیں۔ `norito::header::Flags` اندرونی inspection اور مستقبل کی versions کیلئے رکھا جاتا ہے۔

## Derive support

`norito_derive` `Encode`, `Decode`, `IntoSchema` اور JSON helper derives فراہم کرتا ہے۔ اہم conventions:

- Derives AoS اور packed code paths دونوں بناتے ہیں؛ v1 AoS layout (flags `0x00`) کو default رکھتا ہے جب تک header flags packed variants کو opt-in نہ کریں۔ Implementation `crates/norito_derive/src/derive_struct.rs` میں ہے۔
- Layout پر اثر انداز ہونے والی features (`packed-struct`, `packed-seq`, `compact-len`) header flags کے ذریعے opt-in ہیں اور peers کے درمیان consistent encode/decode ہونی چاہئیں۔
- JSON helpers (`norito::json`) open APIs کیلئے deterministic Norito-backed JSON فراہم کرتے ہیں۔ `norito::json::{to_json_pretty, from_json}` استعمال کریں — کبھی `serde_json` نہیں۔

## Multicodec اور identifier tables

Norito اپنی multicodec assignments `norito::multicodec` میں رکھتا ہے۔ Reference table (hashes، key types، payload descriptors) repository root کے `multicodec.md` میں برقرار رکھی جاتی ہے۔ جب نیا identifier شامل ہو:

1. `norito::multicodec::registry` اپڈیٹ کریں۔
2. `multicodec.md` میں table بڑھائیں۔
3. اگر downstream bindings (Python/Java) map استعمال کرتے ہیں تو انہیں regenerate کریں۔

## Docs اور fixtures کو regenerate کرنا

جب پورٹل اس وقت prose summary host کر رہا ہے، upstream Markdown sources کو source of truth رکھیں:

- **Spec**: `norito.md`
- **Multicodec table**: `multicodec.md`
- **Benchmarks**: `crates/norito/benches/`
- **Golden tests**: `crates/norito/tests/`

جب Docusaurus automation live ہو جائے، پورٹل ایک sync script کے ذریعے اپڈیٹ ہوگا (جسے `docs/portal/scripts/` میں track کیا گیا ہے) جو ان files سے data کھینچتا ہے۔ تب تک، spec میں تبدیلی کے ساتھ اس صفحہ کو دستی طور پر align رکھیں۔

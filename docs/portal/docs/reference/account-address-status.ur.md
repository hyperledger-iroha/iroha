<!-- Auto-generated stub for Urdu (ur) translation. Replace this content with the full translation. -->

---
lang: ur
direction: rtl
source: docs/portal/docs/reference/account-address-status.md
status: complete
translator: manual
source_hash: 448093b8efd6e92c8265691d6a4dafcf1d3b9c369cc6ae42ee8ae418b27bc36b
source_last_modified: "2025-11-08T16:40:11.557769+00:00"
translation_last_reviewed: 2025-11-14
---

<div dir="rtl">

---
id: account-address-status
title: اکاؤنٹ ایڈریس کمپلائنس
description: ADDR‑2 fixture ورک فلو اور یہ کہ SDK ٹیمیں کیسے sync میں رہتی ہیں، کا خلاصہ۔
---

canonical ADDR‑2 bundle (`fixtures/account/address_vectors.json`) میں IH58، compressed
(half/full‑width)، multisignature اور negative fixtures شامل ہیں۔ ہر SDK اور Torii
surface، اسی JSON پر انحصار کرتی ہے تاکہ codec drift، پروڈکشن تک پہنچنے سے پہلے ہی
detected ہو جائے۔ یہ صفحہ internal status brief
(`docs/source/account_address_status.md`، جو root ریپوزٹری میں ہے) کو mirror کرتا ہے،
تاکہ portal readers کو mono‑repo میں گھومے بغیر ورک فلو کی high‑level تصویر مل سکے۔

## bundle کو regenerate یا verify کرنا

```bash
# canonical fixture کو ریفریش کریں (fixtures/account/address_vectors.json لکھتا ہے)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# اگر committed فائل پرانی ہو تو فوراً فیل ہو
cargo xtask address-vectors --verify
```

Flags:

- `--stdout` — JSON کو stdout پر emit کرتا ہے، on‑the‑fly inspection کے لیے۔
- `--out <path>` — آؤٹ پٹ کو کسی اور path پر لکھتا ہے (مثلاً لوکل diff کے لیے)۔
- `--verify` — working copy کو تازہ generate ہونے والے مواد سے compare کرتا ہے (اسے
  `--stdout` کے ساتھ combine نہیں کیا جا سکتا)۔

CI workflow **Address Vector Drift**، جب بھی fixture، generator یا docs میں تبدیلی ہو،
`cargo xtask address-vectors --verify` چلاتا ہے تاکہ reviewers کو فوراً alert کیا جا
سکے۔

## fixture کون consume کرتا ہے؟

| Surface | Validation |
|---------|-----------|
| Rust data-model | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (server) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| JavaScript SDK | `javascript/iroha_js/test/address.test.js` |
| Swift SDK | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| Android SDK | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |

ہر harness، canonical bytes + IH58 + compressed encodings کا round‑trip کرتا ہے اور
negative cases میں Norito‑style error codes کو fixture کے ساتھ match ہونے پر assert
کرتا ہے۔

## Automation چاہیے؟

release tooling، helper `scripts/account_fixture_helper.py` کے ذریعے fixture refresh کو
اسکرپٹ کر سکتا ہے، جو canonical bundle کو بغیر copy/paste کے fetch یا verify کرتا ہے:

```bash
# کسی custom path پر ڈاؤن لوڈ کریں (ڈیفالٹ fixtures/account/address_vectors.json ہے)
python3 scripts/account_fixture_helper.py fetch --output path/to/sdk/address_vectors.json

# چیک کریں کہ لوکل کاپی canonical source (HTTPS یا file://) کے برابر ہے
python3 scripts/account_fixture_helper.py check --target path/to/sdk/address_vectors.json --quiet
```

یہ helper، `--source` override یا `IROHA_ACCOUNT_FIXTURE_URL` env var کو accept کرتا ہے،
تاکہ SDK CI jobs اپنے پسندیدہ mirror پر نشانہ لگا سکیں۔

## مکمل brief درکار ہے؟

ADDR‑2 کمپلائنس کی مکمل تفصیل (owners، monitoring plan، کھلے اقدامات) ریپوزٹری
کے اندر `docs/source/account_address_status.md` میں موجود ہے، اور اسی کے ساتھ
Account Structure RFC (`docs/account_structure.md`) بھی ہے۔ اس صفحہ کو ایک تیز
operational reminder کے طور پر استعمال کریں؛ تفصیلی رہنمائی کے لیے repo docs کی طرف
رجوع کریں۔

</div>

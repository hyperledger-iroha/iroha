---
lang: ur
direction: rtl
source: docs/portal/static/openapi/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8ad316fefd99c4c3b9ddbade7de59f12aa2dbe9ee256784f61ac87bb4341f04a
source_last_modified: "2025-11-16T17:11:03.605418+00:00"
translation_last_reviewed: 2026-01-30
---

OpenAPI سائننگ
--------------

- Torii کی OpenAPI اسپیک (`torii.json`) پر دستخط ہونا ضروری ہیں، اور منی فیسٹ کی تصدیق `cargo xtask openapi-verify` سے کی جاتی ہے۔
- اجازت یافتہ سائنر keys `allowed_signers.json` میں ہیں؛ جب بھی سائننگ کی کلید تبدیل ہو، اس فائل کو rotate کریں۔ `version` فیلڈ کو `1` پر رکھیں۔
- CI (`ci/check_openapi_spec.sh`) latest اور current specs کے لیے allowlist پہلے ہی نافذ کرتا ہے۔ اگر کوئی دوسرا پورٹل یا پائپ لائن سائن شدہ اسپیک استعمال کرے، تو اس کے verification مرحلے کو اسی allowlist فائل کی طرف اشارہ کریں تاکہ drift نہ ہو۔
- کلید کی rotation کے بعد دوبارہ سائن کرنے کے لیے:
  1. `allowed_signers.json` کو نئی public key سے اپ ڈیٹ کریں۔
  2. اسپیک دوبارہ تیار کر کے سائن کریں: `NORITO_SKIP_BINDINGS_SYNC=1 cargo xtask openapi --output docs/portal/static/openapi/torii.json --sign <ed25519-key-hex-path>`.
  3. `ci/check_openapi_spec.sh` دوبارہ چلائیں (یا `cargo xtask openapi-verify` دستی طور پر) تاکہ منی فیسٹ allowlist سے مطابقت کی تصدیق ہو سکے۔

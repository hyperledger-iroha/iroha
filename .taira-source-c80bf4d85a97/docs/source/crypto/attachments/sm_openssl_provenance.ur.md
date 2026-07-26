---
lang: ur
direction: rtl
source: docs/source/crypto/attachments/sm_openssl_provenance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 95a34657b6064f925995a7e9f20145d14fda681f4af1f182418b9f624047e576
source_last_modified: "2026-01-03T18:07:57.073612+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

٪ ایس ایم اوپن ایس ایل/ٹونگسو پروویژن اسنیپ شاٹ
٪ پیدا ہوا: 2026-01-30

# ماحولیاتی خلاصہ

- `pkg-config --modversion openssl`: `3.6.0`
- `openssl version -a`: `LibreSSL 3.3.6` کی رپورٹیں (MACOS پر سسٹم سے فراہم کردہ TLS ٹول کٹ)۔
- `cargo tree -p iroha_crypto --features "sm sm-ffi-openssl"`: عین مورچا انحصار اسٹیک کے لئے `sm_iroha_crypto_tree.txt` دیکھیں (`openssl` CRATE V0.10.74 ، `openssl-sys` V0.9.x ، `openssl-src` کریٹ `openssl-src` کریٹ کے ذریعے دستیاب اوپن ایس ایل 3.x ذرائع دستیاب ہیں۔ `crates/iroha_crypto/Cargo.toml` کے لئے تعی .ن پیش نظارہ تعمیرات)۔

# نوٹ

- مقامی ترقیاتی ماحولیات لبریس ایل ہیڈر/لائبریریوں کے خلاف لنک ؛ پروڈکشن پیش نظارہ بلڈز کو اوپن ایس ایل> = 3.0.0 یا ٹونگسو 8.x استعمال کرنا چاہئے۔ سسٹم ٹول کٹ کو تبدیل کریں یا حتمی آرٹ فیکٹ بنڈل تیار کرتے وقت `OPENSSL_DIR`/`PKG_CONFIG_PATH` کو سیٹ کریں۔
- عین اوپن ایس ایل/ٹونگسو ٹربال ہیش (`openssl version -v` ، `openssl version -b` ، `openssl version -f`) پر قبضہ کرنے کے لئے ریلیز بلڈ ماحول کے اندر اس سنیپ شاٹ کو دوبارہ تخلیق کریں اور تولیدی بلڈ اسکرپٹ/چیکم کو منسلک کریں۔ وینڈرڈ بلڈز کے لئے ، `openssl-src` کریٹ ورژن/کارگو (`target/debug/build/openssl-sys-*/output` میں مرئی) کے ذریعہ استعمال کردہ کمٹ ریکارڈ کریں۔
- ایپل سلیکن کے میزبانوں کو اوپن ایس ایل دھواں کے استعمال کو چلاتے وقت `RUSTFLAGS=-Aunsafe-code` کی ضرورت ہوتی ہے لہذا AARCH64 SM3/SM4 ایکسلریشن اسٹب مرتب کرتے ہیں (MACOS پر اندرونی حصے دستیاب نہیں ہیں)۔ اسکرپٹ `scripts/sm_openssl_smoke.sh` CI اور مقامی رنز کو مستقل رکھنے کے لئے `cargo` کو طلب کرنے سے پہلے اس پرچم کو برآمد کرتا ہے۔
- ایک بار جب پیکیجنگ پائپ لائن پن ہوجائے تو ، upstream ماخذ پروویژن (جیسے `openssl-src-<ver>.tar.gz` SHA256) منسلک کریں۔ سی آئی نوادرات میں ایک ہی ہیش کا استعمال کریں۔
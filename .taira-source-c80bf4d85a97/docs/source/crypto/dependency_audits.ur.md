---
lang: ur
direction: rtl
source: docs/source/crypto/dependency_audits.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 04e4cf26ed0ce9f9782be8aae9d16425a7a87fdbd1986cbcbca68a27ba0a3afe
source_last_modified: "2026-01-03T18:07:57.038859+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# کریپٹو انحصار آڈٹ

## اسٹری بوگ (`streebog` کریٹ)

- درخت میں ** ورژن: ** `0.11.0-rc.2` `vendor/streebog` کے تحت وینڈر کیا گیا (جب `gost` خصوصیت فعال ہوجائے تو استعمال کیا جاتا ہے)۔
- ** صارف: ** `crates/iroha_crypto::signature::gost` (HMAC-Streebog DRBG + میسج ہیشنگ)۔
- ** حیثیت: ** صرف ریلیز-مینڈیٹیٹ۔ کوئی غیر آر سی کریٹ فی الحال مطلوبہ API سطح کی پیش کش نہیں کرتا ہے ،
  لہذا ہم آڈیٹیبلٹی کے لئے کریٹ ان ٹری کا آئینہ دار ہیں جبکہ ہم حتمی ریلیز کے لئے اپ اسٹریم کو ٹریک کرتے ہیں۔
- ** جائزہ چوکیوں: **
  - وائچ پروف سویٹ اور ٹی سی 26 فکسچر کے خلاف تصدیق شدہ ہیش آؤٹ پٹ
    `cargo test -p iroha_crypto --features gost` (`crates/iroha_crypto/tests/gost_wycheproof.rs` دیکھیں)۔
  - `cargo bench -p iroha_crypto --bench gost_sign --features gost`
    موجودہ انحصار کے ساتھ ہر TC26 وکر کے ساتھ ساتھ ED25519/SECP256K1 کی مشقیں۔
  - `cargo run -p iroha_crypto --bin gost_perf_check --features gost`
    چیک ان میڈینز کے خلاف تازہ پیمائش کا موازنہ کریں (CI میں `--summary-only` استعمال کریں ،
    `--write-baseline crates/iroha_crypto/benches/gost_perf_baseline.json` جب بازآبادکاری کرتے ہو)۔
  - `scripts/gost_bench.sh` بینچ + چیک فلو کو لپیٹتا ہے۔ JSON کو اپ ڈیٹ کرنے کے لئے `--write-baseline` پاس کریں۔
    اختتام سے آخر میں ورک فلو کے لئے `docs/source/crypto/gost_performance.md` دیکھیں۔
۔
  تباہ کن آر این جی کی ناکامی سے بچنے کے لئے دستخط کنندہ او ایس اینٹروپی کے ساتھ نونیسس ​​کو ہیج کرتا ہے۔
۔ ایک بار ٹیگ اترنے کے بعد ، سلوک کریں
  ایک معیاری انحصار بمپ کے طور پر اپ گریڈ کریں (چیکسم کی تصدیق کریں ، مختلف ، ریکارڈ پروویژن ، اور جائزہ لیں
  وینڈرڈ آئینہ چھوڑیں)۔
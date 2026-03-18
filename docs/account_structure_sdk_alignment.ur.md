# SDK اور codec مالکان کے لیے I105 rollout نوٹ

ٹیمیں: Rust SDK، TypeScript/JavaScript SDK، Python SDK، Kotlin SDK، codec tooling

سیاق: `docs/account_structure.md` اب جاری شدہ I105 اکاؤنٹ ID امپلیمنٹیشن کو ظاہر کرتا ہے۔
براہِ کرم SDK کے رویے اور ٹیسٹ کو کینونیکل اسپیک کے مطابق کریں۔

اہم حوالہ جات:
- ایڈریس codec + ہیڈر لے آؤٹ — `docs/account_structure.md` §2
- کرف رجسٹری — `docs/source/references/address_curve_registry.md`
- Norm v1 ڈومین ہینڈلنگ — `docs/source/references/address_norm_v1.md`
- فکسچر ویکٹرز — `fixtures/account/address_vectors.json`

کارروائیاں:
1. **کینونیکل آؤٹ پٹ:** `AccountId::to_string()`/Display لازماً صرف I105 دے
   (`@domain` لاحقہ کے بغیر)۔ کینونیکل hex صرف ڈی بگنگ کے لیے ہے (`0x...`).
2. **Accepted inputs:** parsers MUST accept only canonical I105 account literals. Reject i105-default `sora...`, canonical hex (`0x...`), any `@<domain>` suffix, alias literals, legacy `norito:<hex>`, and `uaid:` / `opaque:` parser forms.
3. **Resolvers:** canonical account parsing has no default-domain binding, scoped inference, or fallback resolver path. Use `ScopedAccountId` only on interfaces that explicitly require `<account>@<domain>`.
4. **I105 checksum:** `I105PRE || prefix || payload` پر Blake2b‑512 استعمال کریں اور
   پہلے 2 بائٹس لیں۔ compressed alphabet base **105** ہے۔
5. **Curve gating:** SDKs کا ڈیفالٹ صرف Ed25519 ہے۔ ML‑DSA/GOST/SM کے لیے واضح opt‑in دیں
   (Swift build flags؛ JS/Android میں `configureCurveSupport`)۔ Rust کے علاوہ secp256k1
   کو ڈیفالٹ enabled مت سمجھیں۔
6. **CAIP-10 نہیں:** ابھی کوئی CAIP‑10 mapping جاری نہیں ہوئی؛ CAIP‑10 conversions کو
   expose نہ کریں اور نہ ان پر انحصار کریں۔

براہِ کرم codecs/tests کے اپڈیٹ ہونے پر تصدیق کریں؛ کھلے سوالات account‑addressing RFC تھریڈ
میں ٹریک کیے جا سکتے ہیں۔

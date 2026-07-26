---
title: اکاؤنٹ کرف رجسٹری
description: اکاؤنٹ کنٹرولر کے کرف شناخت کنندگان اور سائننگ الگورتھمز کے درمیان معیاری میپنگ.
---

# اکاؤنٹ کرف رجسٹری

اکاؤنٹ ایڈریس اپنے کنٹرولرز کو ایک ٹیگ شدہ payload کی صورت میں اینکوڈ کرتے ہیں
جو 8‑bit کرف شناخت کنندہ سے شروع ہوتا ہے۔ ویلیڈیٹرز، SDKs اور ٹولنگ ایک مشترک
رجسٹری پر انحصار کرتے ہیں تاکہ کرف شناخت کنندگان ریلیزوں کے درمیان مستحکم
رہیں اور مختلف نفاذات میں ڈیٹرمنسٹک ڈیکوڈنگ ممکن ہو۔

نیچے دیا گیا جدول ہر تفویض شدہ `curve_id` کے لئے معیاری حوالہ ہے۔ ایک
machine-readable نقل اس دستاویز کے ساتھ
[`address_curve_registry.json`](address_curve_registry.json) میں موجود ہے؛
خودکار ٹولنگ کو JSON ورژن استعمال کرنا چاہئے اور fixtures بناتے وقت `version`
فیلڈ کو pin کرنا چاہئے۔

## رجسٹرڈ کرفس

| ID (`curve_id`) | الگورتھم | فیچر گیٹ | اسٹیٹس | پبلک کی اینکوڈنگ | نوٹس |
|-----------------|----------|----------|--------|------------------|------|
| `0x01` (1) | `ed25519` | — | پروڈکشن | 32‑byte کمپریسڈ Ed25519 کی | V1 کے لئے معیاری کرف۔ تمام SDK builds کو اس شناخت کنندہ کی سپورٹ کرنا ضروری ہے۔ |
| `0x02` (2) | `ml-dsa` | — | پروڈکشن (config-gated) | Dilithium3 پبلک کی (1952 bytes) | تمام builds میں دستیاب۔ کنٹرولر payloads جاری کرنے سے پہلے `crypto.allowed_signing` + `crypto.curves.allowed_curve_ids` میں enable کریں۔ |
| `0x03` (3) | `bls_normal` | `bls` | پروڈکشن (feature-gated) | 48‑byte کمپریسڈ G1 پبلک کی | کنسینسز ویلیڈیٹرز کے لئے لازمی۔ اگر `allowed_signing`/`allowed_curve_ids` میں شامل نہ بھی ہو تو admission BLS کنٹرولرز کو اجازت دیتا ہے۔ |
| `0x04` (4) | `secp256k1` | — | پروڈکشن | 33‑byte SEC1 کمپریسڈ کی | SHA‑256 پر ڈیٹرمنسٹک ECDSA؛ signatures معیاری 64‑byte `r∥s` لے آؤٹ استعمال کرتی ہیں۔ |
| `0x05` (5) | `bls_small` | `bls` | پروڈکشن (feature-gated) | 96‑byte کمپریسڈ G2 پبلک کی | کمپیکٹ‑signature BLS پروفائل (چھوٹی signatures، بڑی پبلک keys). |
| `0x0A` (10) | `gost3410-2012-256-paramset-a` | `gost` | محفوظ | 64‑byte little-endian TC26 param set A پوائنٹ | `gost` فیچر سے کھلتا ہے جب گورننس rollout منظور کرے۔ |
| `0x0B` (11) | `gost3410-2012-256-paramset-b` | `gost` | محفوظ | 64‑byte little-endian TC26 param set B پوائنٹ | TC26 B پیرامیٹر سیٹ کی عکاسی؛ `gost` فیچر گیٹ کے پیچھے بند۔ |
| `0x0C` (12) | `gost3410-2012-256-paramset-c` | `gost` | محفوظ | 64‑byte little-endian TC26 param set C پوائنٹ | مستقبل کی گورننس منظوری کے لئے محفوظ۔ |
| `0x0D` (13) | `gost3410-2012-512-paramset-a` | `gost` | محفوظ | 128‑byte little-endian TC26 param set A پوائنٹ | 512‑bit GOST curves کی طلب تک محفوظ۔ |
| `0x0E` (14) | `gost3410-2012-512-paramset-b` | `gost` | محفوظ | 128‑byte little-endian TC26 param set B پوائنٹ | 512‑bit GOST curves کی طلب تک محفوظ۔ |
| `0x0F` (15) | `sm2` | `sm` | محفوظ | DistID length (u16 BE) + DistID bytes + 65‑byte SEC1 uncompressed SM2 key | جب `sm` فیچر preview سے نکلے گا تب دستیاب ہوگا۔ |

### استعمال کے رہنما اصول

- **Fail closed:** Encoders کو غیر معاون الگورتھمز کو `ERR_UNSUPPORTED_ALGORITHM`
  کے ساتھ reject کرنا چاہیے۔ Decoders کو اس رجسٹری میں نہ موجود کسی بھی
  شناخت کنندہ پر `ERR_UNKNOWN_CURVE` دینا چاہیے۔
- **Feature gating:** BLS/GOST/SM2 فہرست کردہ build-time features کے پیچھے رہتے
  ہیں۔ آپریٹرز کو `iroha_config.crypto.allowed_signing` میں متعلقہ entries اور
  build features فعال کرنے چاہئیں قبل اس کے کہ ان curves کے ساتھ addresses
  جاری ہوں۔
- **Admission exceptions:** BLS کنٹرولرز کو کنسینسز ویلیڈیٹرز کے لئے اجازت ہے،
  چاہے `allowed_signing`/`allowed_curve_ids` میں درج نہ ہوں۔
- **Config + manifest parity:** `iroha_config.crypto.allowed_curve_ids` (اور اس کے
  ہم معنی `ManifestCrypto.allowed_curve_ids`) کو استعمال کر کے وہ curve IDs شائع
  کریں جنہیں کلسٹر قبول کرتا ہے؛ admission اب اس فہرست کو `allowed_signing`
  کے ساتھ enforce کرتا ہے۔
- **Deterministic encoding:** پبلک keys عین اسی طرح اینکوڈ ہوتی ہیں جیسے signing
  implementation انہیں واپس کرتی ہے (Ed25519 compressed bytes، ML‑DSA public keys،
  BLS compressed points وغیرہ)۔ SDKs کو malformed payloads بھیجنے سے پہلے
  validation errors دکھانا چاہیے۔
- **Manifest parity:** Genesis manifests اور controller manifests کو ایک ہی IDs
  استعمال کرنے چاہئیں تاکہ admission کلسٹر کی صلاحیت سے باہر کنٹرولرز کو رد کر
  سکے۔

## صلاحیت bitmask کا اعلان

`GET /v1/node/capabilities` اب `crypto.curves` کے تحت `allowed_curve_ids` فہرست
اور packed `allowed_curve_bitmap` array دونوں فراہم کرتا ہے۔ یہ bitmap 64‑bit
lanes میں little-endian ہے (زیادہ سے زیادہ چار قدریں `u8` شناخت کنندہ اسپیس
0–255 کے لئے)۔ اگر bit `i` set ہے تو curve identifier `i` کلسٹر کی admission
پالیسی میں مجاز ہے۔

- مثال: `{ allowed_curve_ids: [1, 15] }` ⇒ `allowed_curve_bitmap: [32770]`
  کیونکہ `(1 << 1) | (1 << 15) = 32770`۔
- `63` سے اوپر curves بعد والی lanes میں bits سیٹ کرتی ہیں۔ trailing zero lanes
  کو payload مختصر رکھنے کے لئے چھوڑ دیا جاتا ہے، لہٰذا اگر `curve_id = 130`
  بھی enable ہو تو `allowed_curve_bitmap = [32768, 0, 4]` آئے گا (bits 15 اور
  130 سیٹ)۔

ڈیش بورڈز اور health checks کے لئے bitmap کو ترجیح دیں: ایک single bit test
بغیر پورا array scan کیے صلاحیت کا جواب دیتا ہے، جبکہ ordered identifiers
چاہنے والی tooling `allowed_curve_ids` استعمال کر سکتی ہے۔ دونوں views کی
نمائش roadmap آئٹم **ADDR-3** کی ضرورت پوری کرتی ہے۔

## ویلیڈیشن چیک لسٹ

کنٹرولرز استعمال کرنے والا ہر component (Torii، admission، SDK encoders، offline
tooling) قبول کرنے سے پہلے ایک ہی deterministic checks لاگو کرے۔ نیچے کے steps
لازمی ویلیڈیشن منطق ہیں:

1. **کلسٹر پالیسی حل کریں:** اکاؤنٹ payload کے ابتدائی `curve_id` byte کو پڑھیں
   اور اگر identifier `iroha_config.crypto.allowed_curve_ids` (اور اس کے mirrored
   `ManifestCrypto.allowed_curve_ids`) میں موجود نہیں تو controller reject کریں۔
   BLS controllers ایک استثنا ہیں: build میں موجود ہوں تو admission انہیں allowlists
   سے قطع نظر قبول کرتا ہے تاکہ کنسینسز ویلیڈیٹر keys کام کرتے رہیں۔ یہ کلسٹرز کو
   preview curves قبول کرنے سے روکتا ہے جنہیں آپریٹرز نے واضح طور پر enable نہ کیا ہو۔
2. **Encoding length نافذ کریں:** payload length کو algorithm کے canonical سائز سے
   compare کریں قبل اس کے کہ key decompress/expand کی جائے۔ جو ویلیو length
   چیک میں fail ہو اسے reject کریں تاکہ خراب inputs جلدی نکل جائیں۔
3. **Algorithm-specific decoding چلائیں:** `iroha_crypto` کے وہی canonical decoders
   استعمال کریں (`ed25519_dalek`, `pqcrypto_mldsa`, `w3f_bls`/`blstrs`, `sm2`,
   TC26 helpers وغیرہ) تاکہ تمام implementations میں subgroup/point validation
   کا یکساں رویہ رہے۔
4. **Signature sizes verify کریں:** admission اور SDKs کو نیچے دیے گئے signature
   lengths enforce کرنے چاہئیں اور verifier چلانے سے پہلے truncated یا overlong
   signature والے payloads کو reject کرنا چاہیے۔

| الگورتھم | `curve_id` | پبلک کی بائٹس | سائنچر بائٹس | اہم چیکس |
|----------|------------|---------------|--------------|----------|
| `ed25519` | `0x01` | 32 | 64 | غیر معیاری compressed points reject کریں، cofactor clearing نافذ کریں (small-order points نہ ہوں)، اور signatures validate کرتے وقت `s < L` یقینی بنائیں۔ |
| `ml-dsa` (Dilithium3) | `0x02` | 1952 | 3309 | 1952 bytes کے علاوہ payloads کو decode سے پہلے reject کریں؛ Dilithium3 public key parse کریں اور pqcrypto-mldsa کے ساتھ canonical lengths پر signatures verify کریں۔ |
| `bls_normal` | `0x03` | 48 | 96 | صرف canonical compressed G1 public keys اور compressed G2 signatures قبول کریں؛ identity points اور non-canonical encodings reject کریں۔ |
| `secp256k1` | `0x04` | 33 | 64 | صرف SEC1-compressed points قبول کریں؛ decompress کر کے non-canonical/invalid points reject کریں، اور signatures کو canonical 64‑byte `r∥s` encoding سے verify کریں (low‑`s` normalization signer پر ہوتی ہے)۔ |
| `bls_small` | `0x05` | 96 | 48 | صرف canonical compressed G2 public keys اور compressed G1 signatures قبول کریں؛ identity points اور non-canonical encodings reject کریں۔ |
| `gost3410-2012-256-paramset-a` | `0x0A` | 64 | 64 | payload کو `(x||y)` little-endian coordinates کے طور پر interpret کریں، ہر coordinate `< p` ہو، identity point reject کریں، اور signatures verify کرتے وقت canonical 32‑byte `r`/`s` limbs enforce کریں۔ |
| `gost3410-2012-256-paramset-b` | `0x0B` | 64 | 64 | param set A جیسی validation لیکن TC26 B domain parameters کے ساتھ۔ |
| `gost3410-2012-256-paramset-c` | `0x0C` | 64 | 64 | param set A جیسی validation لیکن TC26 C domain parameters کے ساتھ۔ |
| `gost3410-2012-512-paramset-a` | `0x0D` | 128 | 128 | `(x||y)` کو 64‑byte limbs کے طور پر interpret کریں، `< p` یقینی بنائیں، identity point reject کریں، اور signatures کے لئے 64‑byte `r`/`s` limbs لازم کریں۔ |
| `gost3410-2012-512-paramset-b` | `0x0E` | 128 | 128 | param set A جیسی validation لیکن TC26 B 512‑bit domain parameters کے ساتھ۔ |
| `sm2` | `0x0F` | 2 + distid + 65 | 64 | distid length (u16 BE) decode کریں، DistID bytes validate کریں، SEC1 uncompressed point parse کریں، GM/T 0003 subgroup rules apply کریں، configured DistID apply کریں، اور SM2 کے مطابق canonical `(r, s)` limbs لازم کریں۔ |

ہر قطار [`address_curve_registry.json`](address_curve_registry.json) کے اندر
`validation` آبجیکٹ سے مطابقت رکھتی ہے۔ JSON export استعمال کرنے والی tooling
`public_key_bytes`, `signature_bytes`, اور `checks` فیلڈز پر انحصار کر کے وہی
validation steps automate کر سکتی ہے؛ variable-length encodings (مثلاً SM2)
`public_key_bytes` کو null رکھتی ہیں اور length rule کو `checks` میں بیان کرتی
ہیں۔

## نیا curve identifier مانگنا

1. الگورتھم کی specification (encoding, validation, error handling) تیار کریں اور
   rollout کے لئے گورننس منظوری حاصل کریں۔
2. اس دستاویز اور `address_curve_registry.json` کو اپڈیٹ کرنے والی pull request
   جمع کریں۔ نئے identifiers منفرد ہوں اور `0x01..=0xFE` کے inclusive رینج میں ہوں۔
3. SDKs، Norito fixtures، اور operator docs کو نئے identifier کے ساتھ اپڈیٹ کریں
   قبل اس کے کہ پروڈکشن نیٹ ورکس پر deploy کیا جائے۔
4. سیکیورٹی اور observability لیڈز کے ساتھ ہم آہنگی کریں تاکہ telemetry، runbooks
   اور admission policies نئے الگورتھم کی عکاسی کریں۔

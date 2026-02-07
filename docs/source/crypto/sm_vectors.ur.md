---
lang: ur
direction: rtl
source: docs/source/crypto/sm_vectors.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fd46471945188bcb95c8ee411c48acc8915a92b408df196caa65bf25f0596732
source_last_modified: "2026-01-05T10:35:36.441790+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! SM2/SM3/SM4 انضمام کے کام کے لئے حوالہ ٹیسٹ ویکٹر۔

# ایس ایم ویکٹر اسٹیجنگ نوٹ

یہ دستاویز عوامی طور پر دستیاب نامعلوم ٹیسٹوں کو جمع کرتی ہے جو خودکار درآمد اسکرپٹ کے اراضی سے پہلے ایس ایم 2/ایس ایم 3/ایس ایم 4 ہارنس کو بیج دیتے ہیں۔ مشین سے پڑھنے کے قابل کاپیاں زندہ رہتے ہیں:

- `crates/iroha_crypto/tests/fixtures/sm_known_answers.toml` (انیکس ویکٹرز ، آر ایف سی 8998 کیسز ، ضمیمہ مثال 1)۔
- `fixtures/sm/sm2_fixture.json` (مشترکہ ڈٹرمینسٹک SDK حقیقت میں زنگ/ازگر/جاوا اسکرپٹ ٹیسٹ کے ذریعہ کھایا جاتا ہے)۔
-`crates/iroha_crypto/tests/fixtures/wycheproof_sm2.json`-کیوریٹڈ 52-کیس کارپس (ڈٹرمینسٹک فکسچر + ترکیب شدہ بٹ فلپ/پیغام/دم تراشنے کے منفی) اپاچی -2.0 کے تحت آئینہ دار ہے جبکہ اپ اسٹریم ایس ایم 2 سویٹ زیر التوا ہے۔ `crates/iroha_crypto/tests/sm2_wycheproof.rs` جب ممکن ہو تو معیاری SM2 تصدیق کنندہ کا استعمال کرتے ہوئے ان ویکٹروں کی تصدیق کرتا ہے اور جب ضرورت ہو تو انیکس ڈومین کے خالص بگنٹ پر عمل درآمد میں واپس آجاتا ہے۔

## ایس ایم 2 اوپن ایس ایل / ٹونگسو / جی ایم ایس ایس ایل کے خلاف دستخطی توثیق

ضمیمہ مثال 1 (FP-256) شناخت `ALICE123@YAHOO.COM` (ENTLA 0x0090) ، پیغام `"message digest"` ، اور ذیل میں دکھائی گئی عوامی کلید کا استعمال کرتی ہے۔ ایک کاپی اور - پیسٹ اوپن ایس ایل/ٹونگسو ورک فلو ہے:

```bash
# 1. Public key (SubjectPublicKeyInfo)
cat > pubkey.pem <<'PEM'
-----BEGIN PUBLIC KEY-----
MFkwEwYHKoZIzj0CAQYIKoEcz1UBgi0DQgAECuTHeYqg8RlHG+4RglvkYgK7eeKlhESV6XwE/03y
VIp8AkD4jxzU4WNSpzwXt/FvBzU+U6F21oSp/gxrt5joVw==
-----END PUBLIC KEY-----
PEM

# 2. Message (no trailing newline)
printf "message digest" > msg.bin

# 3. Signature (DER form)
cat > sig.b64 <<'EOF'
MEQCIEDx7Fn3k9n0ngnc70kTDUGU95+x7tLKpVus20nE51XRAiBvxtrDLF1c8Qx337IPfC62Z6RX
hy+wnsVjJ6Z+x97r5w==
EOF
base64 -d sig.b64 > sig.der

# 4. Verify (expects "Signature Verified Successfully")
openssl pkeyutl -verify -pubin -inkey pubkey.pem \
  -in msg.bin -sigfile sig.der \
  -rawin -digest sm3 \
  -pkeyopt distid:ALICE123@YAHOO.COM
```

* اوپن ایس ایل 3.x دستاویزات `distid:` / `hexdistid:` اختیارات۔ کچھ اوپن ایس ایل 1.1.1 تعمیرات کو `sm2_id:` کے طور پر نوب کو بے نقاب کرتا ہے - جو بھی `openssl pkeyutl -help` میں ظاہر ہوتا ہے اس کا استعمال کریں۔
* جی ایم ایس ایس ایل وہی `pkeyutl` سطح برآمد کرتا ہے۔ پرانے بلڈز نے بھی `-pkeyopt sm2_id:...` قبول کیا۔
*لائبریسل (میکوس/اوپن بی ایس ڈی پر پہلے سے طے شدہ) ** نہیں ** ایس ایم 2/ایس ایم 3 کو نافذ کرتا ہے ، لہذا مذکورہ بالا کمانڈ وہاں ناکام ہوجاتا ہے۔ اوپن ایس ایل ≥ 1.1.1 ، ٹونگسو ، یا جی ایم ایس ایس ایل کا استعمال کریں۔

ڈیر ہیلپر `3044022040F1EC59F793D9F49E09DCEF49130D4194F79FB1EED2CAA55BACDB49C4E755D102206FC6DAC32C5D5CF10C77DFB20F7C2EB667A457872FB09EC56327A67EC7DEEBE7` کو خارج کرتا ہے ، جو انیکس دستخط سے ملتا ہے۔

ضمیمہ صارف کی معلومات کو بھی پرنٹ کرتا ہے اور اس کے نتیجے میں ڈائجسٹ:

```
ZA = F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A
e  = SM3(ZA || "message digest")
   = B524F552CD82B8B028476E005C377FB19A87E6FC682D48BB5D42E3D9B9EFFE76
```

آپ اوپن ایس ایل کے ساتھ تصدیق کرسکتے ہیں:

```bash
echo -n 'F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A' \
  | xxd -r -p > za.bin
printf "message digest" >> za.bin
openssl dgst -sm3 -binary za.bin | xxd -p -c 256
# -> b524f552cd82b8b028476e005c377fb19a87e6fc682d48bb5d42e3d9b9effe76
```

وکر مساوات کے لئے ازگر کی سنجیدگی کی جانچ پڑتال:

```python
p = int("FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFF", 16)
a = int("FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFC", 16)
b = int("28E9FA9E9D9F5E344D5A9E4BCF6509A7F39789F515AB8F92DDBCBD414D940E93", 16)
x = int("0AE4C7798AA0F119471BEE11825BE46202BB79E2A5844495E97C04FF4DF2548A", 16)
y = int("7C0240F88F1CD4E16352A73C17B7F16F07353E53A176D684A9FE0C6BB798E857", 16)
assert (pow(y, 2, p) - (pow(x, 3, p) + a * x + b)) % p == 0
```

## SM3 ہیش ویکٹر

| ان پٹ | ہیکس انکوڈنگ | ڈائجسٹ (ہیکس) | ماخذ |
| ------- | ---------------- | -------------- | -------- |
| `""` (خالی تار) | `""` | `1ab21d8355cfa17f8e61194831e81a8f22bec8c728fefb747ed035eb5082aa2b` | GM/T 0004-2012 ضمیمہ A.1 |
| `"abc"` | `616263` | `66c7f0f462eeedd9d1f2d46bdc10e4e24167c4875cf2f7a2297da02b8f4ba8e0` | GM/T 0004-2012 ضمیمہ A.2 |
| `"abcd"` 16 بار (64 بائٹس) کو دہرایا گیا `61626364` × 16 | `debe9ff92275b8a138604889c18e5a4d6fdb70e5387e5765293dcba39c0c5732` | GB/T 32905-2016 ضمیمہ A |

## SM4 بلاک سائفر (ECB) ویکٹر

| کلید (ہیکس) | سادہ متن (ہیکس) | ciphertext (ہیکس) | ماخذ |
| ----------- | ----------------- | -------------------- | -------- |
| `0123456789abcdeffedcba9876543210` | `0123456789abcdeffedcba9876543210` | `681edf34d206965e86b3e94f536e4246` | GM/T 0002-2012 ضمیمہ A.1 |
| `0123456789abcdeffedcba9876543210` | `000102030405060708090a0b0c0d0e0f` | `59b50808d3dcf921fa30b5b3c1dddc19` | GM/T 0002-2012 ضمیمہ A.2 |
| `0123456789abcdeffedcba9876543210` | `ffeeddccbbaa99887766554433221100` | `1c3b3f56186b70819d3f5aa11fe2c8b6` | GM/T 0002-2012 ضمیمہ A.3 |

## SM4-GCM تصدیق شدہ خفیہ کاری

| کلید | iv | aad | سادہ متن | ciphertext | ٹیگ | ماخذ |
| ----- | ---- | ----- | ----------- | -------------- | ----- | -------- |
| `0123456789abcdeffedcba9876543210` | `00001234567800000000abcd` | `feedfacedeadbeeffeedfacedeadbeefabaddad2` | `d9313225f88406e5a55909c5aff5269a` | `42831ec2217774244b7221b784d0d49c` | `4d5c2af327cd64a62cf35abd2ba6fab4` | آر ایف سی 8998 ضمیمہ A.2 |

## SM4-CCM تصدیق شدہ خفیہ کاری| کلید | نونس | aad | سادہ متن | ciphertext | ٹیگ | ماخذ |
| ----- | ------- | ----- | ----------- | -------------- | ----- | -------- |
| `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `202122232425262728292a2b2c2d2e2f` | `7162015b4dac2555` | `4d26de5a` | آر ایف سی 8998 ضمیمہ A.3 |

### WYCHEPROOF منفی معاملات (SM4 GCM/CCM)

یہ معاملات `crates/iroha_crypto/tests/sm3_sm4_vectors.rs` میں ریگریشن سویٹ سے آگاہ کرتے ہیں۔ ہر معاملے میں توثیق میں ناکام ہونا چاہئے۔

| موڈ | TC ID | تفصیل | کلید | نونس | aad | ciphertext | ٹیگ | نوٹ |
| ------ | ------- | ------------- | ----- | ------- | ----- | ------------ | ----- | ------- |
| جی سی ایم | 1 | ٹیگ بٹ پلٹائیں | `0123456789abcdeffedcba9876543210` | `00001234567800000000abcd` | `feedfacedeadbeeffeedfacedeadbeefabaddad2` | `42831ec2217774244b7221b784d0d49c` | `5d5c2af327cd64a62cf35abd2ba6fab4` | Wycheprof سے ماخوذ غلط ٹیگ |
| سی سی ایم | 17 | ٹیگ بٹ پلٹائیں | `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `7162015b4dac2555` | `5d26de5a` | Wycheprof سے ماخوذ غلط ٹیگ |
| سی سی ایم | 18 | کٹے ہوئے ٹیگ (3 بائٹس) | `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `7162015b4dac2555` | `5d26de` | یقینی بناتا ہے کہ مختصر ٹیگز کی توثیق کی توثیق |
| سی سی ایم | 19 | ciphertext بٹ پلٹائیں | `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `7162015b4dac2554` | `5d26de5a` | چھیڑ چھاڑ شدہ پے لوڈ کا پتہ لگائیں

## SM2 عین مطابق دستخطی حوالہ

| فیلڈ | قدر (ہیکس جب تک نوٹ نہیں کیا جاتا ہے) | ماخذ |
| ------- | ------------------------------ | -------- |
| وکر پیرامیٹرز | `sm2p256v1` (a = `FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFC` ، وغیرہ) | GM/T 0003.5-2012 ضمیمہ A |
| صارف ID (`distid`) | ASCII `"ALICE123@YAHOO.COM"` (ENTLA 0x0090) | GM/T 0003 ضمیمہ D |
| عوامی کلید | `040AE4C7798AA0F119471BEE11825BE46202BB79E2A5844495E97C04FF4DF2548A7C0240F88F1CD4E16352A73C17B7F16F07353E53A176D684A9FE0C6BB798E857` | GM/T 0003 ضمیمہ D |
| پیغام | `"message digest"` (ہیکس `6d65737361676520646967657374`) | GM/T 0003 ضمیمہ D |
| زا | `F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A` | GM/T 0003 ضمیمہ D |
| `e = SM3(ZA || M)` | `B524F552CD82B8B028476E005C377FB19A87E6FC682D48BB5D42E3D9B9EFFE76` | GM/T 0003 ضمیمہ D |
| دستخط `(r,s)` | `40F1EC59F793D9F49E09DCEF49130D4194F79FB1EED2CAA55BACDB49C4E755D1` ، `6FC6DAC32C5D5CF10C77DFB20F7C2EB667A457872FB09EC56327A67EC7DEEBE7` | GM/T 0003 ضمیمہ D |
| ملٹی کوڈیک (عارضی) | `8626550012414C494345313233405941484F4F2E434F4D040AE4…` (`sm2-pub` ، ورنٹ `0x1306`) | ضمیمہ مثال 1 سے ماخوذ ہے
| سابقہ ​​ملٹی ہش | `sm2:8626550012414C494345313233405941484F4F2E434F4D040AE4…` | ماخوذ (میچ `sm_known_answers.toml`) |

ایس ایم 2 ملٹی ہش پے لوڈ کو `distid_len (u16 BE) || distid bytes || SEC1 uncompressed key (65 bytes)` کے طور پر انکوڈ کیا گیا ہے۔

### مورچا SDK ڈٹرمینسٹک سائننگ فکسچر (SM-3C)

ساختی ویکٹر سرنی میں زنگ/ازگر/جاوا اسکرپٹ پیریٹی پے لوڈ شامل ہیں
لہذا ہر کلائنٹ مشترکہ بیج کے ساتھ ایک ہی SM2 پیغام پر دستخط کرتا ہے اور
شناخت کرنے والے کی تمیز| فیلڈ | قدر (ہیکس جب تک نوٹ نہیں کیا جاتا ہے) | نوٹ |
| ------- | ------------------------------ | ------- |
| امتیازی ID | `"iroha-sdk-sm2-fixture"` | زنگ/ازگر/جے ایس ایس ڈی کے میں مشترکہ مشترکہ |
| بیج | `"iroha-rust-sdk-sm2-deterministic-fixture"` (ہیکس `69726F68612D727573742D73646B2D736D322D64657465726D696E69737469632D66697874757265`) | `Sm2PrivateKey::from_seed` | میں ان پٹ
| نجی کلید | `E64AE95930A2324DCF11BB1657FBBC2CD2BEF7515664BE7A15CA6AE5BCE0B7CA` | بیج سے عزم سے اخذ کیا گیا |
| عوامی کلید (سیک 1 غیر سنجیدہ) | `0494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` | میچوں کا تعی .ن مشتق |
| عوامی کلید ملٹی ہش | `862658001569726F68612D73646B2D736D322D666978747572650494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` | `PublicKey::to_string()` کی آؤٹ پٹ |
| سابقہ ​​ملٹی ہش | `sm2:862658001569726F68612D73646B2D736D322D666978747572650494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` | `PublicKey::to_prefixed_string()` کی آؤٹ پٹ |
| زا | `6B0229C3FBFE7A5DC6EE27228E18E512752DCCE9191EB381242A7342D568EDDE` | `Sm2PublicKey::compute_z` کے ذریعے حساب کیا گیا
| پیغام | `"Rust SDK SM2 signing fixture v1"` (ہیکس `527573742053444B20534D32207369676E696E672066697874757265207631`) | ایس ڈی کے پیریٹی ٹیسٹ کے لئے کیننیکل پے لوڈ |
| دستخط `(r, s)` | `4E026F2E0FB75A6418C95C046D70DB7B95558BE30FB2001906EBE2091FA1AF76` ، `299CFF374026D9E0C49C6BD4A99A1473BA17EFF429B3FAD083B1577F7559BDF5` | ڈٹرمینسٹک سائننگ کے ذریعے تیار کردہ |

- کراس ایس ڈی کے کھپت:
  - `fixtures/sm/sm2_fixture.json` اب `vectors` سرنی کو بے نقاب کرتا ہے۔ مورچا کریپٹو ریگریشن سویٹ (`crates/iroha_crypto/tests/sm2_fixture_vectors.rs`) ، مورچا کلائنٹ مددگار (`crates/iroha/src/sm.rs`) ، ازگر بائنڈنگز (`python/iroha_python/tests/test_crypto.py`) ، اور جاوا اسکرپٹ SDK (`javascript/iroha_js/test/crypto.sm2.fixture.test.js`) ان تمام فکسچر کو پارس کرتے ہیں۔
  - `crates/iroha/tests/sm_signing.rs` مشقیں طے شدہ دستخط اور تصدیق کرتی ہیں کہ آن چین ملٹی ہش/ملٹی کوڈیک آؤٹ پٹ فکسچر سے ملتے ہیں۔
  -داخلہ ٹائم ریگریشن سوٹس (`crates/iroha_core/tests/admission_batching.rs`) پر زور دیں SM2 پے لوڈ کو مسترد کردیا جاتا ہے جب تک کہ `allowed_signing` میں `sm2` * اور * `default_hash` `sm3-256` شامل نہ ہو ، جس میں تشکیل کی رکاوٹوں کو اختتامی حد تک کا احاطہ کیا جائے۔
- 追加カバレッジ: 異常系（無効な曲線、異常な `r/s` 、 `distid` 改ざん） は `crates/iroha_crypto/tests/sm2_fuzz.rs` の پراپرٹی テストで網羅済みです。ANEX مثال 1 の正規ベクトルは `sm_known_answers.toml` に多言語対応の ملٹی کوڈیک 形式で引き続き提供しています。
- مورچا کوڈ اب `Sm2PublicKey::compute_z` کو بے نقاب کرتا ہے تاکہ زیڈ اے فکسچر پروگرام کے ساتھ تیار کیا جاسکے۔ انیکس ڈی رجعت کے ل I `sm2_compute_z_matches_annex_example` دیکھیں۔

## اگلی کارروائی
- داخلے کے وقت کے رجعت پسندی (`admission_batching.rs`) کی نگرانی کریں تاکہ یقینی بنائیں کہ کنفیگ گیٹنگ SM2 کو قابل بنانے کی حدود کو نافذ کرتی ہے۔
- WYCHEPROOF SM4 GCM/CCM مقدمات کے ساتھ کوریج میں توسیع کریں اور SM2 دستخطی توثیق کے لئے جائیداد پر مبنی فوز کے اہداف کو حاصل کریں۔ ✅ (`sm3_sm4_vectors.rs` میں پکڑے گئے غلط کیس سبسیٹ)۔
- متبادل IDs کے لئے LLM پرامپٹ: *"ضمیمہ کے لئے SM2 دستخط فراہم کریں مثال کے طور پر 1 جب امتیازی ID Alise123@yahoo.com کے بجائے 1234567812345678 پر سیٹ کیا گیا ہے ، اور نئی ZA/E اقدار کا خاکہ پیش کریں۔"
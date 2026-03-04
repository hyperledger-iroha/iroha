---
lang: ar
direction: rtl
source: docs/source/crypto/sm_vectors.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fd46471945188bcb95c8ee411c48acc8915a92b408df196caa65bf25f0596732
source_last_modified: "2026-01-05T10:35:36.441790+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! ناقلات الاختبار المرجعية لأعمال التكامل SM2/SM3/SM4.

# ملاحظات التدريج ناقلات SM

يقوم هذا المستند بتجميع اختبارات الإجابات المعروفة المتاحة للعامة والتي تزرع أدوات SM2/SM3/SM4 قبل وصول البرامج النصية للاستيراد الآلي. النسخ المقروءة آليًا موجودة في:

- `crates/iroha_crypto/tests/fixtures/sm_known_answers.toml` (متجهات الملحق، حالات RFC 8998، مثال الملحق 1).
- `fixtures/sm/sm2_fixture.json` (تركيبة SDK الحتمية المشتركة التي تستهلكها اختبارات Rust/Python/JavaScript).
- `crates/iroha_crypto/tests/fixtures/wycheproof_sm2.json` - مجموعة منسقة مكونة من 52 حالة (تركيبات حتمية + سلبيات اقتطاع البتات/الرسائل/الذيل) تم عكسها ضمن Apache-2.0 بينما تكون مجموعة SM2 الأولية معلقة. يتحقق `crates/iroha_crypto/tests/sm2_wycheproof.rs` من هذه المتجهات باستخدام أداة التحقق القياسية SM2 عندما يكون ذلك ممكنًا ويعود إلى تطبيق BigInt النقي للمجال الملحق عند الحاجة.

## التحقق من توقيع SM2 ضد OpenSSL / Tongsuo / GmSSL

يستخدم مثال الملحق 1 (Fp-256) الهوية `ALICE123@YAHOO.COM` (ENTLA 0x0090)، والرسالة `"message digest"`، والمفتاح العام الموضح أدناه. سير عمل النسخ واللصق OpenSSL/Tongsuo هو:

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

* يقوم OpenSSL 3.x بتوثيق خيارات `distid:` / `hexdistid:`. تعرض بعض إصدارات OpenSSL 1.1.1 المقبض كـ `sm2_id:` - استخدم أيًا كان الذي يظهر في `openssl pkeyutl -help`.
* يصدر GmSSL نفس سطح `pkeyutl`؛ تقبل الإصدارات الأقدم أيضًا `-pkeyopt sm2_id:...`.
* LibreSSL (الافتراضي في نظام التشغيل macOS/OpenBSD) لا يطبق SM2/SM3، لذا يفشل الأمر أعلاه هناك. استخدم OpenSSL ≥ 1.1.1 أو Tongsuo أو GmSSL.

يقوم مساعد DER بإصدار `3044022040F1EC59F793D9F49E09DCEF49130D4194F79FB1EED2CAA55BACDB49C4E755D102206FC6DAC32C5D5CF10C77DFB20F7C2EB667A457872FB09EC56327A67EC7DEEBE7`، والذي يطابق توقيع المرفق.

يقوم الملحق أيضًا بطباعة تجزئة معلومات المستخدم والملخص الناتج:

```
ZA = F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A
e  = SM3(ZA || "message digest")
   = B524F552CD82B8B028476E005C377FB19A87E6FC682D48BB5D42E3D9B9EFFE76
```

يمكنك التأكيد باستخدام OpenSSL:

```bash
echo -n 'F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A' \
  | xxd -r -p > za.bin
printf "message digest" >> za.bin
openssl dgst -sm3 -binary za.bin | xxd -p -c 256
# -> b524f552cd82b8b028476e005c377fb19a87e6fc682d48bb5d42e3d9b9effe76
```

التحقق من عقلانية بايثون لمعادلة المنحنى:

```python
p = int("FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFF", 16)
a = int("FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFC", 16)
b = int("28E9FA9E9D9F5E344D5A9E4BCF6509A7F39789F515AB8F92DDBCBD414D940E93", 16)
x = int("0AE4C7798AA0F119471BEE11825BE46202BB79E2A5844495E97C04FF4DF2548A", 16)
y = int("7C0240F88F1CD4E16352A73C17B7F16F07353E53A176D684A9FE0C6BB798E857", 16)
assert (pow(y, 2, p) - (pow(x, 3, p) + a * x + b)) % p == 0
```

## ناقلات تجزئة SM3

| الإدخال | ترميز سداسي عشري | الملخص (ست عشري) | المصدر |
|-------|--------------|--------------|--------|
| `""` (سلسلة فارغة) | `""` | `1ab21d8355cfa17f8e61194831e81a8f22bec8c728fefb747ed035eb5082aa2b` | GM/T 0004-2012 الملحق أ.1 |
| `"abc"` | `616263` | `66c7f0f462eeedd9d1f2d46bdc10e4e24167c4875cf2f7a2297da02b8f4ba8e0` | GM/T 0004-2012 الملحق أ.2 |
| `"abcd"` مكرر 16 مرة (64 بايت) | `61626364` ×16 | `debe9ff92275b8a138604889c18e5a4d6fdb70e5387e5765293dcba39c0c5732` | GB/T 32905-2016 الملحق أ |

## ناقلات SM4 Block Cipher (ECB).

| مفتاح (ست عشري) | نص عادي (ست عشري) | النص المشفر (ست عشري) | المصدر |
|-----------|-----------------|------------------|--------|
| `0123456789abcdeffedcba9876543210` | `0123456789abcdeffedcba9876543210` | `681edf34d206965e86b3e94f536e4246` | GM/T 0002-2012 الملحق أ.1 |
| `0123456789abcdeffedcba9876543210` | `000102030405060708090a0b0c0d0e0f` | `59b50808d3dcf921fa30b5b3c1dddc19` | GM/T 0002-2012 الملحق أ.2 |
| `0123456789abcdeffedcba9876543210` | `ffeeddccbbaa99887766554433221100` | `1c3b3f56186b70819d3f5aa11fe2c8b6` | GM/T 0002-2012 الملحق أ.3 |

## التشفير الموثق SM4-GCM

| مفتاح | الرابع | عاد | نص عادي | النص المشفر | العلامة | المصدر |
|-----|----|-----|----------|------------|-----|--------|
| `0123456789abcdeffedcba9876543210` | `00001234567800000000abcd` | `feedfacedeadbeeffeedfacedeadbeefabaddad2` | `d9313225f88406e5a55909c5aff5269a` | `42831ec2217774244b7221b784d0d49c` | `4d5c2af327cd64a62cf35abd2ba6fab4` | RFC 8998 الملحق أ.2 |

## التشفير الموثق SM4-CCM| مفتاح | نونس | عاد | نص عادي | النص المشفر | العلامة | المصدر |
|-----|-------|----------|-----------|-----|--------|
| `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `202122232425262728292a2b2c2d2e2f` | `7162015b4dac2555` | `4d26de5a` | RFC 8998 الملحق أ.3 |

### الحالات السلبية المقاومة للرطوبة (SM4 GCM/CCM)

تُعلم هذه الحالات مجموعة الانحدار في `crates/iroha_crypto/tests/sm3_sm4_vectors.rs`. يجب أن تفشل كل حالة في التحقق.

| الوضع | معرف TC | الوصف | مفتاح | نونس | عاد | النص المشفر | العلامة | ملاحظات |
|------|-------|------------|-------|-----|------------|-----|-------|
| جي سي إم | 1 | العلامة بت الوجه | `0123456789abcdeffedcba9876543210` | `00001234567800000000abcd` | `feedfacedeadbeeffeedfacedeadbeefabaddad2` | `42831ec2217774244b7221b784d0d49c` | `5d5c2af327cd64a62cf35abd2ba6fab4` | علامة غير صالحة مشتقة من Wycheproof |
| CCM | 17 | العلامة بت الوجه | `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `7162015b4dac2555` | `5d26de5a` | علامة غير صالحة مشتقة من Wycheproof |
| CCM | 18 | العلامة المقتطعة (3 بايت) | `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `7162015b4dac2555` | `5d26de` | يضمن فشل العلامات القصيرة في المصادقة |
| CCM | 19 | الوجه بت النص المشفر | `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `7162015b4dac2554` | `5d26de5a` | كشف الحمولة المعبث بها |

## مرجع التوقيع الحتمي SM2

| المجال | القيمة (ست عشرية ما لم تتم الإشارة إليها) | المصدر |
|-------|-------------------------|--------|
| معلمات المنحنى | `sm2p256v1` (أ = `FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFC`، إلخ) | GM/T 0003.5-2012 الملحق أ |
| معرف المستخدم (`distid`) | ASCII `"ALICE123@YAHOO.COM"` (ENTLA 0x0090) | GM/T 0003 المرفق د |
| المفتاح العام | `040AE4C7798AA0F119471BEE11825BE46202BB79E2A5844495E97C04FF4DF2548A7C0240F88F1CD4E16352A73C17B7F16F07353E53A176D684A9FE0C6BB798E857` | GM/T 0003 المرفق د |
| رسالة | `"message digest"` (ست عشري `6d65737361676520646967657374`) | GM/T 0003 المرفق د |
| ZA | `F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A` | GM/T 0003 المرفق د |
| `e = SM3(ZA || M)` | `B524F552CD82B8B028476E005C377FB19A87E6FC682D48BB5D42E3D9B9EFFE76` | GM/T 0003 المرفق د |
| التوقيع `(r,s)` | `40F1EC59F793D9F49E09DCEF49130D4194F79FB1EED2CAA55BACDB49C4E755D1`، `6FC6DAC32C5D5CF10C77DFB20F7C2EB667A457872FB09EC56327A67EC7DEEBE7` | GM/T 0003 المرفق د |
| ترميز متعدد (مؤقت) | `8626550012414C494345313233405941484F4F2E434F4D040AE4…` (`sm2-pub`، البديل `0x1306`) | مستمد من المرفق المثال 1 |
| البادئة المتعددة | `sm2:8626550012414C494345313233405941484F4F2E434F4D040AE4…` | مشتق (يطابق `sm_known_answers.toml`) |

يتم ترميز حمولات SM2 المتعددة التجزئة كـ `distid_len (u16 BE) || distid bytes || SEC1 uncompressed key (65 bytes)`.

### تركيبات التوقيع الحتمية لـ Rust SDK (SM-3c)

تشتمل مجموعة المتجهات المنظمة على حمولة تكافؤ Rust/Python/JavaScript
لذلك يقوم كل عميل بالتوقيع على نفس رسالة SM2 باستخدام بذرة مشتركة و
معرف مميز.| المجال | القيمة (ست عشرية ما لم تتم الإشارة إليها) | ملاحظات |
|-------|-------------------------|-------|
| المعرف المميز | `"iroha-sdk-sm2-fixture"` | تمت مشاركتها عبر مجموعات SDK لـ Rust/Python/JS |
| بذرة | `"iroha-rust-sdk-sm2-deterministic-fixture"` (ست عشري `69726F68612D727573742D73646B2D736D322D64657465726D696E69737469632D66697874757265`) | الإدخال إلى `Sm2PrivateKey::from_seed` |
| المفتاح الخاص | `E64AE95930A2324DCF11BB1657FBBC2CD2BEF7515664BE7A15CA6AE5BCE0B7CA` | مشتقة حتمية من البذرة |
| المفتاح العام (SEC1 غير مضغوط) | `0494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` | يطابق الاشتقاق الحتمي |
| تعدد المفتاح العام | `862658001569726F68612D73646B2D736D322D666978747572650494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` | إخراج `PublicKey::to_string()` |
| البادئة المتعددة | `sm2:862658001569726F68612D73646B2D736D322D666978747572650494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` | إخراج `PublicKey::to_prefixed_string()` |
| ZA | `6B0229C3FBFE7A5DC6EE27228E18E512752DCCE9191EB381242A7342D568EDDE` | تم حسابه عبر `Sm2PublicKey::compute_z` |
| رسالة | `"Rust SDK SM2 signing fixture v1"` (ست عشري `527573742053444B20534D32207369676E696E672066697874757265207631`) | الحمولة الأساسية لاختبارات تكافؤ SDK |
| التوقيع `(r, s)` | `4E026F2E0FB75A6418C95C046D70DB7B95558BE30FB2001906EBE2091FA1AF76`، `299CFF374026D9E0C49C6BD4A99A1473BA17EFF429B3FAD083B1577F7559BDF5` | تم إنتاجه عن طريق التوقيع الحتمي |

- الاستهلاك عبر SDK:
  - يعرض `fixtures/sm/sm2_fixture.json` الآن مصفوفة `vectors`. تقوم مجموعة انحدار تشفير Rust (`crates/iroha_crypto/tests/sm2_fixture_vectors.rs`)، ومساعدي عميل Rust (`crates/iroha/src/sm.rs`)، وروابط Python (`python/iroha_python/tests/test_crypto.py`)، وJavaScript SDK (`javascript/iroha_js/test/crypto.sm2.fixture.test.js`) بتحليل هذه التركيبات.
  - يمارس `crates/iroha/tests/sm_signing.rs` التوقيع الحتمي ويتحقق من أن مخرجات multihash/multicodec الموجودة على السلسلة تتطابق مع التركيبات.
  - تؤكد مجموعات انحدار وقت القبول (`crates/iroha_core/tests/admission_batching.rs`) على رفض حمولات SM2 ما لم يتضمن `allowed_signing` `sm2` *و* `default_hash` هو `sm3-256`، مما يغطي قيود التكوين من البداية إلى النهاية.
- الاسم: `r/s`، `distid`، `distid`، `crates/iroha_crypto/tests/sm2_fuzz.rs` الخاصية テストで網羅済みです.المرفق مثال 1 の正規ベクトルは `sm_known_answers.toml` に多言語対応の multicodec شكرا جزيلا.
- يكشف رمز الصدأ الآن عن `Sm2PublicKey::compute_z` بحيث يمكن إنشاء تركيبات ZA برمجيًا؛ راجع `sm2_compute_z_matches_annex_example` للتعرف على انحدار الملحق D.

## الإجراءات التالية
- مراقبة تراجعات وقت القبول (`admission_batching.rs`) لضمان استمرار بوابة التكوين في فرض حدود تمكين SM2.
- توسيع التغطية باستخدام حافظات Wycheproof SM4 GCM/CCM واشتقاق أهداف ضبابية قائمة على الخاصية للتحقق من توقيع SM2. ✅ (تم التقاط مجموعة فرعية من الحالات غير الصالحة في `sm3_sm4_vectors.rs`).
- مطالبة LLM بالمعرفات البديلة: *"قدم توقيع SM2 للملحق المثال 1 عند تعيين المعرف المميز على 1234567812345678 بدلاً من ALICE123@YAHOO.COM، وقم بتحديد قيم ZA/e الجديدة."*
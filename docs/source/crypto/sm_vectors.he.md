---
lang: he
direction: rtl
source: docs/source/crypto/sm_vectors.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fd46471945188bcb95c8ee411c48acc8915a92b408df196caa65bf25f0596732
source_last_modified: "2026-01-05T10:35:36.441790+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! וקטורי בדיקת התייחסות לעבודת אינטגרציה של SM2/SM3/SM4.

# SM Vectors Staging Notes

מסמך זה מקבץ בדיקות תשובות ידועות הזמינות לציבור שפותחות את רתמות ה-SM2/SM3/SM4 לפני שסקריפטי הייבוא האוטומטי נוחתים. עותקים קריאים במכונה חיים ב:

- `crates/iroha_crypto/tests/fixtures/sm_known_answers.toml` (וספח וקטורים, מקרים RFC 8998, נספח דוגמה 1).
- `fixtures/sm/sm2_fixture.json` (מתקן SDK דטרמיניסטי משותף הנצרך על ידי מבחני Rust/Python/JavaScript).
- `crates/iroha_crypto/tests/fixtures/wycheproof_sm2.json` - קורפוס מאוצר של 52 מקרים (מתקנים דטרמיניסטים + שלילי bit-flip/הודעה/קטיעה זנב מסונתזים) שיקוף תחת Apache-2.0 בזמן שחבילת SM2 במעלה הזרם ממתינה. `crates/iroha_crypto/tests/sm2_wycheproof.rs` מאמת את הווקטורים הללו באמצעות מאמת ה-SM2 הסטנדרטי במידת האפשר ונופל חזרה למימוש BigInt טהור של תחום ה-Annex בעת הצורך.

## אימות חתימת SM2 נגד OpenSSL / Tongsuo / GmSSL

דוגמה של נספח 1 (Fp-256) משתמשת בזהות `ALICE123@YAHOO.COM` (ENTLA 0x0090), בהודעה `"message digest"` ובמפתח הציבורי המוצג להלן. זרימת עבודה של העתקה והדבקה של OpenSSL/Tongsuo היא:

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

* OpenSSL 3.x מתעד את האפשרויות `distid:` / `hexdistid:`. כמה מבני OpenSSL 1.1.1 חושפים את הכפתור כ-`sm2_id:` - השתמש בכל מה שמופיע ב-`openssl pkeyutl -help`.
* GmSSL מייצא את אותו משטח `pkeyutl`; בונים ישנים יותר מקובלים גם `-pkeyopt sm2_id:...`.
* LibreSSL (ברירת מחדל ב-macOS/OpenBSD) **לא** מיישמת SM2/SM3, כך שהפקודה למעלה נכשלת שם. השתמש ב-OpenSSL ≥ 1.1.1, Tongsuo או GmSSL.

עוזר ה-DER פולט `3044022040F1EC59F793D9F49E09DCEF49130D4194F79FB1EED2CAA55BACDB49C4E755D102206FC6DAC32C5D5CF10C77DFB20F7C2EB667A457872FB09EC56327A67EC7DEEBE7`, התואם את חתימת הנספח.

הנספח גם מדפיס את ה-hash של מידע המשתמש ואת התקציר שנוצר:

```
ZA = F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A
e  = SM3(ZA || "message digest")
   = B524F552CD82B8B028476E005C377FB19A87E6FC682D48BB5D42E3D9B9EFFE76
```

אתה יכול לאשר עם OpenSSL:

```bash
echo -n 'F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A' \
  | xxd -r -p > za.bin
printf "message digest" >> za.bin
openssl dgst -sm3 -binary za.bin | xxd -p -c 256
# -> b524f552cd82b8b028476e005c377fb19a87e6fc682d48bb5d42e3d9b9effe76
```

בדיקת שפיות פייתון למשוואת העקומה:

```python
p = int("FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFF", 16)
a = int("FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFC", 16)
b = int("28E9FA9E9D9F5E344D5A9E4BCF6509A7F39789F515AB8F92DDBCBD414D940E93", 16)
x = int("0AE4C7798AA0F119471BEE11825BE46202BB79E2A5844495E97C04FF4DF2548A", 16)
y = int("7C0240F88F1CD4E16352A73C17B7F16F07353E53A176D684A9FE0C6BB798E857", 16)
assert (pow(y, 2, p) - (pow(x, 3, p) + a * x + b)) % p == 0
```

## SM3 Hash Vectors

| קלט | קידוד hex | Digest (hex) | מקור |
|-------|--------------|----------------|
| `""` (מחרוזת ריקה) | `""` | `1ab21d8355cfa17f8e61194831e81a8f22bec8c728fefb747ed035eb5082aa2b` | GM/T 0004-2012 נספח A.1 |
| `"abc"` | `616263` | `66c7f0f462eeedd9d1f2d46bdc10e4e24167c4875cf2f7a2297da02b8f4ba8e0` | GM/T 0004-2012 נספח A.2 |
| `"abcd"` חזר על עצמו 16 פעמים (64 בתים) | `61626364` ×16 | `debe9ff92275b8a138604889c18e5a4d6fdb70e5387e5765293dcba39c0c5732` | GB/T 32905-2016 נספח א' |

## וקטורים של SM4 Block Cipher (ECB).

| מפתח (hex) | טקסט רגיל (hex) | טקסט צופן (hex) | מקור |
|-----------|----------------|----------------|--------|
| `0123456789abcdeffedcba9876543210` | `0123456789abcdeffedcba9876543210` | `681edf34d206965e86b3e94f536e4246` | GM/T 0002-2012 נספח A.1 |
| `0123456789abcdeffedcba9876543210` | `000102030405060708090a0b0c0d0e0f` | `59b50808d3dcf921fa30b5b3c1dddc19` | GM/T 0002-2012 נספח A.2 |
| `0123456789abcdeffedcba9876543210` | `ffeeddccbbaa99887766554433221100` | `1c3b3f56186b70819d3f5aa11fe2c8b6` | GM/T 0002-2012 נספח A.3 |

## הצפנה מאומתת SM4-GCM

| מפתח | IV | AAD | טקסט רגיל | טקסט צופן | תג | מקור |
|-----|----|-----|----------------|------------|-----|--------|
| `0123456789abcdeffedcba9876543210` | `00001234567800000000abcd` | `feedfacedeadbeeffeedfacedeadbeefabaddad2` | `d9313225f88406e5a55909c5aff5269a` | `42831ec2217774244b7221b784d0d49c` | `4d5c2af327cd64a62cf35abd2ba6fab4` | RFC 8998 נספח א.2 |

## הצפנה מאומתת SM4-CCM| מפתח | לא | AAD | טקסט רגיל | טקסט צופן | תג | מקור |
|-----|-------|-----|----------------|------------|------|--------|
| `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `202122232425262728292a2b2c2d2e2f` | `7162015b4dac2555` | `4d26de5a` | RFC 8998 נספח א.3 |

### מקרים שליליים חסיני Wyche (SM4 GCM/CCM)

מקרים אלה מודיעים לחבילת הרגרסיה ב-`crates/iroha_crypto/tests/sm3_sm4_vectors.rs`. כל מקרה חייב להיכשל באימות.

| מצב | מזהה TC | תיאור | מפתח | לא | AAD | טקסט צופן | תג | הערות |
|------|-------|-------------|-----|-------|-----|------------|-----|-------|
| GCM | 1 | תג bit flip | `0123456789abcdeffedcba9876543210` | `00001234567800000000abcd` | `feedfacedeadbeeffeedfacedeadbeefabaddad2` | `42831ec2217774244b7221b784d0d49c` | `5d5c2af327cd64a62cf35abd2ba6fab4` | תג לא חוקי שמקורו ב-Wycheproof |
| CCM | 17 | תג bit flip | `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `7162015b4dac2555` | `5d26de5a` | תג לא חוקי שמקורו ב-Wycheproof |
| CCM | 18 | תג קטוע (3 בתים) | `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `7162015b4dac2555` | `5d26de` | מבטיח כי תגים קצרים נכשלים באימות |
| CCM | 19 | Flip Bit צופן | `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `7162015b4dac2554` | `5d26de5a` | זיהוי מטען משובש |

## סימוכין לחתימה דטרמיניסטית SM2

| שדה | ערך (hex אלא אם כן צוין) | מקור |
|-------|--------------------------------|--------|
| פרמטרי עקומה | `sm2p256v1` (a = `FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFC` וכו') | GM/T 0003.5-2012 נספח א' |
| מזהה משתמש (`distid`) | ASCII `"ALICE123@YAHOO.COM"` (ENTLA 0x0090) | GM/T 0003 נספח D |
| מפתח ציבורי | `040AE4C7798AA0F119471BEE11825BE46202BB79E2A5844495E97C04FF4DF2548A7C0240F88F1CD4E16352A73C17B7F16F07353E53A176D684A9FE0C6BB798E857` | GM/T 0003 נספח D |
| הודעה | `"message digest"` (hex `6d65737361676520646967657374`) | GM/T 0003 נספח D |
| ז"א | `F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A` | GM/T 0003 נספח D |
| `e = SM3(ZA || M)` | `B524F552CD82B8B028476E005C377FB19A87E6FC682D48BB5D42E3D9B9EFFE76` | GM/T 0003 נספח D |
| חתימה `(r,s)` | `40F1EC59F793D9F49E09DCEF49130D4194F79FB1EED2CAA55BACDB49C4E755D1`, `6FC6DAC32C5D5CF10C77DFB20F7C2EB667A457872FB09EC56327A67EC7DEEBE7` | GM/T 0003 נספח D |
| Multicodec (זמני) | `8626550012414C494345313233405941484F4F2E434F4D040AE4…` (`sm2-pub`, varint `0x1306`) | נגזר מהנספח לדוגמה 1 |
| קידומת multihash | `sm2:8626550012414C494345313233405941484F4F2E434F4D040AE4…` | נגזר (תואם `sm_known_answers.toml`) |

עומסי SM2 multihash מקודדים כ-`distid_len (u16 BE) || distid bytes || SEC1 uncompressed key (65 bytes)`.

### Rust SDK Fixture Signing Deterministic (SM-3c)

מערך הוקטורים המובנים כולל את מטען השוויוניות Rust/Python/JavaScript
כך שכל לקוח חותם על אותה הודעת SM2 עם סיד משותף ו
מזהה מבדיל.| שדה | ערך (hex אלא אם כן צוין) | הערות |
|-------|------------------------|-------|
| מזהה מבדיל | `"iroha-sdk-sm2-fixture"` | משותף בין ערכות SDK של Rust/Python/JS |
| זרע | `"iroha-rust-sdk-sm2-deterministic-fixture"` (hex `69726F68612D727573742D73646B2D736D322D64657465726D696E69737469632D66697874757265`) | קלט ל-`Sm2PrivateKey::from_seed` |
| מפתח פרטי | `E64AE95930A2324DCF11BB1657FBBC2CD2BEF7515664BE7A15CA6AE5BCE0B7CA` | נגזר באופן דטרמיניסטי מהזרע |
| מפתח ציבורי (SEC1 לא דחוס) | `0494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` | מתאים לגזירה דטרמיניסטית |
| Multihash מפתח ציבורי | `862658001569726F68612D73646B2D736D322D666978747572650494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` | פלט של `PublicKey::to_string()` |
| קידומת multihash | `sm2:862658001569726F68612D73646B2D736D322D666978747572650494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` | פלט של `PublicKey::to_prefixed_string()` |
| ז"א | `6B0229C3FBFE7A5DC6EE27228E18E512752DCCE9191EB381242A7342D568EDDE` | מחושב באמצעות `Sm2PublicKey::compute_z` |
| הודעה | `"Rust SDK SM2 signing fixture v1"` (hex `527573742053444B20534D32207369676E696E672066697874757265207631`) | מטען קנוני עבור מבחני זוגיות SDK |
| חתימה `(r, s)` | `4E026F2E0FB75A6418C95C046D70DB7B95558BE30FB2001906EBE2091FA1AF76`, `299CFF374026D9E0C49C6BD4A99A1473BA17EFF429B3FAD083B1577F7559BDF5` | מיוצר באמצעות חתימה דטרמיניסטית |

- צריכה חוצה SDK:
  - `fixtures/sm/sm2_fixture.json` חושף כעת מערך `vectors`. חבילת רגרסיית ההצפנה של Rust (`crates/iroha_crypto/tests/sm2_fixture_vectors.rs`), עוזרי לקוח Rust (`crates/iroha/src/sm.rs`), כריכות Python (`python/iroha_python/tests/test_crypto.py`), ו-JavaScript SDK (`javascript/iroha_js/test/crypto.sm2.fixture.test.js`) מנתחים כולם את התקנים הללו.
  - `crates/iroha/tests/sm_signing.rs` מפעיל חתימה דטרמיניסטית ומוודא שיציאות ה-multihash/multicodec על השרשרת תואמות את המתקן.
  - חבילות רגרסיה בזמן קבלה (`crates/iroha_core/tests/admission_batching.rs`) טוענות שמטעני SM2 נדחים אלא אם כן `allowed_signing` כולל `sm2` *ו* `default_hash` הוא `sm3-256` הוא `sm3-256` מכסה את הקונפיגורציה.
- 追加カバレッジ: 異常系（無効な曲線、異常な `r/s`、`distid` 悯こ `crates/iroha_crypto/tests/sm2_fuzz.rs` の נכס テストで網羅済みです。נספח דוגמה 1 の正規ベクトルは 嚁嚁00000119X multicodec 形式で引き続き提供しています。
- קוד חלודה חושף כעת את `Sm2PublicKey::compute_z` כך שניתן ליצור מתקני ZA באופן תוכנתי; ראה `sm2_compute_z_matches_annex_example` עבור נספח D.

## הפעולות הבאות
- עקוב אחר רגרסיות בזמן הקבלה (`admission_batching.rs`) כדי להבטיח ששער התצורה ממשיך לאכוף את גבולות ההפעלה של SM2.
- הרחב את הכיסוי עם מארזי Wycheproof SM4 GCM/CCM והפקת יעדי fuzz מבוססי נכסים לאימות חתימות SM2. ✅ (קבוצת משנה לא חוקית שנלכדה ב-`sm3_sm4_vectors.rs`).
- הנחיה של LLM למזהים חלופיים: *"ספק את חתימת SM2 עבור נספח לדוגמה 1 כאשר המזהה המבחין מוגדר ל-1234567812345678 במקום ALICE123@YAHOO.COM, ותאר את ערכי ה-ZA/e החדשים."*
---
lang: am
direction: ltr
source: docs/source/crypto/sm_vectors.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fd46471945188bcb95c8ee411c48acc8915a92b408df196caa65bf25f0596732
source_last_modified: "2026-01-05T18:22:23.402400+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! ለ SM2/SM3/SM4 ውህደት ሥራ የማጣቀሻ ፈተና ቬክተር።

# ኤስ ኤም ቬክተሮች የማስታወሻ ደብተር

ይህ ሰነድ SM2/SM3/SM4 አውቶማቲክ የማስመጣት ስክሪፕቶች ከማረፍዎ በፊት በይፋ የሚገኙ የታወቁ የመልስ ሙከራዎችን ያጠቃልላል። በማሽን ሊነበቡ የሚችሉ ቅጂዎች በ፡

- `crates/iroha_crypto/tests/fixtures/sm_known_answers.toml` (አባሪ ቬክተር፣ RFC 8998 ጉዳዮች፣ አባሪ ምሳሌ 1)
- `fixtures/sm/sm2_fixture.json` (በRust/Python/JavaScript ሙከራዎች የሚበላው የጋራ መወሰኛ ኤስዲኬ መሣሪያ)።
- `crates/iroha_crypto/tests/fixtures/wycheproof_sm2.json` - የላይ ተፋሰስ SM2 ስብስብ በመጠባበቅ ላይ እያለ በApache-2.0 ስር ተንጸባርቋል ባለ 52-ኬዝ ኮርፐስ (የተወሰነ ቋሚዎች + የተቀናጁ ቢት-ግልብጥ/መልእክት/ጭራ መቆራረጥ አሉታዊ ነገሮች)። `crates/iroha_crypto/tests/sm2_wycheproof.rs` እነዚህን ቬክተሮች በሚቻልበት ጊዜ መደበኛውን SM2 አረጋጋጭ ያረጋግጣል እና አስፈላጊ ሆኖ ሲገኝ ወደ ንጹህ የBigInt የአባሪ ጎራ ትግበራ ይመለሳል።

## የSM2 ፊርማ በOpenSSL/Tongsuo/GmSSL ላይ ማረጋገጫ

አባሪ ምሳሌ 1 (Fp-256) ማንነትን `ALICE123@YAHOO.COM` (ENTLA 0x0090)፣ መልእክት `"message digest"` እና ከታች የሚታየውን የህዝብ ቁልፍ ይጠቀማል። የOpenSSL/Tongsuo የስራ ፍሰት ቅጅ እና ለጥፍ፡-

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

* OpenSSL 3.x የ`distid:`/`hexdistid:` አማራጮችን ይመዘግባል። አንዳንድ የOpenSSL 1.1.1 ግንብ መቆለፊያውን እንደ `sm2_id:` ያጋልጣሉ—በ`openssl pkeyutl -help` ውስጥ የሚታየውን ይጠቀሙ።
* GmSSL ተመሳሳይ `pkeyutl` ወለል ወደ ውጭ ይልካል; የቆዩ ግንባታዎችም `-pkeyopt sm2_id:...` ተቀብለዋል።
* LibreSSL (ነባሪው በ macOS/OpenBSD) ** SM2/SM3 አይሰራም፣ ስለዚህ ከላይ ያለው ትዕዛዝ እዚያ አይሳካም። OpenSSL ≥ 1.1.1፣ Tongsuo ወይም GmSSL ይጠቀሙ።

የ DER አጋዥ `3044022040F1EC59F793D9F49E09DCEF49130D4194F79FB1EED2CAA55BACDB49C4E755D102206FC6DAC32C5D5CF10C77DFB20F7C2EB667A457872FB09EC56327A67EC7DEEBE7` ያወጣል፣ ይህም ከአባሪ ፊርማ ጋር ይዛመዳል።

አባሪው የተጠቃሚ-መረጃ ሃሽ እና ውጤቱን መፍጨት ያትማል፡-

```
ZA = F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A
e  = SM3(ZA || "message digest")
   = B524F552CD82B8B028476E005C377FB19A87E6FC682D48BB5D42E3D9B9EFFE76
```

በ OpenSSL ማረጋገጥ ይችላሉ፡-

```bash
echo -n 'F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A' \
  | xxd -r -p > za.bin
printf "message digest" >> za.bin
openssl dgst -sm3 -binary za.bin | xxd -p -c 256
# -> b524f552cd82b8b028476e005c377fb19a87e6fc682d48bb5d42e3d9b9effe76
```

የፓይዘን ንፅህና ማረጋገጫ ከርቭ እኩልታ፡-

```python
p = int("FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFF", 16)
a = int("FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFC", 16)
b = int("28E9FA9E9D9F5E344D5A9E4BCF6509A7F39789F515AB8F92DDBCBD414D940E93", 16)
x = int("0AE4C7798AA0F119471BEE11825BE46202BB79E2A5844495E97C04FF4DF2548A", 16)
y = int("7C0240F88F1CD4E16352A73C17B7F16F07353E53A176D684A9FE0C6BB798E857", 16)
assert (pow(y, 2, p) - (pow(x, 3, p) + a * x + b)) % p == 0
```

## SM3 Hash Vectors

| ግቤት | ሄክስ ኢንኮዲንግ | ዳይጀስት (ሄክስ) | ምንጭ |
|-------|-------------|------------|
| `""` (ባዶ ሕብረቁምፊ) | `""` | `1ab21d8355cfa17f8e61194831e81a8f22bec8c728fefb747ed035eb5082aa2b` | GM/T 0004-2012 አባሪ A.1 |
| `"abc"` | `616263` | `66c7f0f462eeedd9d1f2d46bdc10e4e24167c4875cf2f7a2297da02b8f4ba8e0` | GM/T 0004-2012 አባሪ A.2 |
| `"abcd"` ተደግሟል 16 ጊዜ (64 ባይት) | `61626364` ×16 | `debe9ff92275b8a138604889c18e5a4d6fdb70e5387e5765293dcba39c0c5732` | GB/T 32905-2016 አባሪ ሀ |

## SM4 አግድ Cipher (ECB) ቬክተር

| ቁልፍ (ሄክስ) | ግልጽ ጽሑፍ (ሄክስ) | ምስጥር ጽሑፍ (ሄክስ) | ምንጭ |
|-------------|
| `0123456789abcdeffedcba9876543210` | `0123456789abcdeffedcba9876543210` | `681edf34d206965e86b3e94f536e4246` | GM/T 0002-2012 አባሪ A.1 |
| `0123456789abcdeffedcba9876543210` | `000102030405060708090a0b0c0d0e0f` | `59b50808d3dcf921fa30b5b3c1dddc19` | GM/T 0002-2012 አባሪ A.2 |
| `0123456789abcdeffedcba9876543210` | `ffeeddccbbaa99887766554433221100` | `1c3b3f56186b70819d3f5aa11fe2c8b6` | GM/T 0002-2012 አባሪ A.3 |

## SM4-GCM የተረጋገጠ ምስጠራ

| ቁልፍ | IV | AAD | ግልጽ ጽሑፍ | ምስጥር ጽሑፍ | መለያ | ምንጭ |
|---------|-------
| `0123456789abcdeffedcba9876543210` | `00001234567800000000abcd` | `feedfacedeadbeeffeedfacedeadbeefabaddad2` | `d9313225f88406e5a55909c5aff5269a` | `42831ec2217774244b7221b784d0d49c` | `4d5c2af327cd64a62cf35abd2ba6fab4` | RFC 8998 አባሪ A.2 |

## SM4-CCM የተረጋገጠ ምስጠራ| ቁልፍ | ምንም | AAD | ግልጽ ጽሑፍ | ምስጥር ጽሑፍ | መለያ | ምንጭ |
|--------|-----|-----------|-----------|-----|-------|
| `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `202122232425262728292a2b2c2d2e2f` | `7162015b4dac2555` | `4d26de5a` | RFC 8998 አባሪ A.3 |

### Wycheproof አሉታዊ ጉዳዮች (SM4 GCM/CCM)

እነዚህ ጉዳዮች በ `crates/iroha_crypto/tests/sm3_sm4_vectors.rs` ውስጥ ያለውን የተሃድሶ ስብስብ ያሳውቃሉ። እያንዳንዱ ጉዳይ ማረጋገጥ አለመቻል አለበት።

| ሁነታ | TC መታወቂያ | መግለጫ | ቁልፍ | ምንም | AAD | ምስጥር ጽሑፍ | መለያ | ማስታወሻ |
|----------
| GCM | 1 | መለያ ትንሽ መገልበጥ | `0123456789abcdeffedcba9876543210` | `00001234567800000000abcd` | `feedfacedeadbeeffeedfacedeadbeefabaddad2` | `42831ec2217774244b7221b784d0d49c` | `5d5c2af327cd64a62cf35abd2ba6fab4` | ከዊcheproof የተገኘ ልክ ያልሆነ መለያ |
| CCM | 17 | መለያ ቢት መገልበጥ | `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `7162015b4dac2555` | `5d26de5a` | ከዊcheproof የተገኘ ልክ ያልሆነ መለያ |
| CCM | 18 | የተቆረጠ መለያ (3 ባይት) | `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `7162015b4dac2555` | `5d26de` | አጭር መለያዎች ማረጋገጥ አለመሳካቱን ያረጋግጣል |
| CCM | 19 | Ciphertext bit Flip | `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `7162015b4dac2554` | `5d26de5a` | የተበላሸ ጭነት ያግኙ |

## SM2 ቆራጥ ፊርማ ማጣቀሻ

| መስክ | እሴት (ሄክስ ካልተጠቀሰ በስተቀር) | ምንጭ |
|------------------|----|
| የጥምዝ መለኪያዎች | `sm2p256v1` (a = `FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFC` ወዘተ) | ጂኤም/ቲ 0003.5-2012 አባሪ ሀ |
| የተጠቃሚ መታወቂያ (`distid`) | ASCII `"ALICE123@YAHOO.COM"` (ENTLA 0x0090) | GM/T 0003 አባሪ D |
| የህዝብ ቁልፍ | `040AE4C7798AA0F119471BEE11825BE46202BB79E2A5844495E97C04FF4DF2548A7C0240F88F1CD4E16352A73C17B7F16F07353E53A176D684A9FE0C6BB798E857` | GM/T 0003 አባሪ D |
| መልእክት | `"message digest"` (ሄክስ `6d65737361676520646967657374`) | GM/T 0003 አባሪ D |
| ZA | `F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A` | GM/T 0003 አባሪ D |
| `e = SM3(ZA || M)` | `B524F552CD82B8B028476E005C377FB19A87E6FC682D48BB5D42E3D9B9EFFE76` | GM/T 0003 አባሪ D |
| ፊርማ `(r,s)` | `40F1EC59F793D9F49E09DCEF49130D4194F79FB1EED2CAA55BACDB49C4E755D1`, `6FC6DAC32C5D5CF10C77DFB20F7C2EB667A457872FB09EC56327A67EC7DEEBE7` | GM/T 0003 አባሪ D |
| መልቲኮዴክ (ጊዜያዊ) | `8626550012414C494345313233405941484F4F2E434F4D040AE4…` (`sm2-pub`፣ varint `0x1306`) | ከአባሪ ምሳሌ 1 | የተወሰደ
| ቅድመ ቅጥያ ያለው መልቲሃሽ | `sm2:8626550012414C494345313233405941484F4F2E434F4D040AE4…` | የተገኘ (ተዛማጆች `sm_known_answers.toml`) |

SM2 መልቲሃሽ የሚጫኑ ጭነቶች እንደ `distid_len (u16 BE) || distid bytes || SEC1 uncompressed key (65 bytes)` ተቀምጠዋል።

### ዝገት ኤስዲኬ ቆራጥ ፊርማ (SM-3c)

የተዋቀረው የቬክተር ድርድር የ Rust/Python/JavaScript ተመጣጣኝ ክፍያን ያካትታል
ስለዚህ እያንዳንዱ ደንበኛ አንድ አይነት SM2 መልእክት በጋራ ዘር እና ይፈርማል
መለያ መለየት.| መስክ | እሴት (ሄክስ ካልተጠቀሰ በስተቀር) | ማስታወሻ |
|------------------|-------|
| መለያ መታወቂያ | `"iroha-sdk-sm2-fixture"` | በመላ ዝገት/Python/JS ኤስዲኬዎች የተጋራ |
| ዘር | `"iroha-rust-sdk-sm2-deterministic-fixture"` (ሄክስ `69726F68612D727573742D73646B2D736D322D64657465726D696E69737469632D66697874757265`) | ግቤት ወደ `Sm2PrivateKey::from_seed` |
| የግል ቁልፍ | `E64AE95930A2324DCF11BB1657FBBC2CD2BEF7515664BE7A15CA6AE5BCE0B7CA` | በቆራጥነት ከዘሩ የተገኘ |
| የህዝብ ቁልፍ (SEC1 ያልታመቀ) | `0494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` | ግጥሚያዎች deterministic አመጣጥ |
| የህዝብ ቁልፍ መልቲሃሽ | `862658001569726F68612D73646B2D736D322D666978747572650494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` | የ `PublicKey::to_string()` ውጤት |
| ቅድመ ቅጥያ ያለው መልቲሃሽ | `sm2:862658001569726F68612D73646B2D736D322D666978747572650494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` | የ `PublicKey::to_prefixed_string()` ውጤት |
| ZA | `6B0229C3FBFE7A5DC6EE27228E18E512752DCCE9191EB381242A7342D568EDDE` | በ `Sm2PublicKey::compute_z` የተሰላ |
| መልእክት | `"Rust SDK SM2 signing fixture v1"` (ሄክስ `527573742053444B20534D32207369676E696E672066697874757265207631`) | ቀኖናዊ ክፍያ ለኤስዲኬ እኩልነት ፈተናዎች |
| ፊርማ `(r, s)` | `4E026F2E0FB75A6418C95C046D70DB7B95558BE30FB2001906EBE2091FA1AF76`, `299CFF374026D9E0C49C6BD4A99A1473BA17EFF429B3FAD083B1577F7559BDF5` | በቆራጥ ፊርማ የተሰራ |

- ኤስዲኬ አቋራጭ ፍጆታ፡-
  - `fixtures/sm/sm2_fixture.json` አሁን የ`vectors` አደራደር አጋልጧል። የ Rust crypto regression suite (`crates/iroha_crypto/tests/sm2_fixture_vectors.rs`)፣ የዝገት ደንበኛ አጋዥዎች (`crates/iroha/src/sm.rs`)፣ Python bindings (`python/iroha_python/tests/test_crypto.py`) እና JavaScript SDK (`javascript/iroha_js/test/crypto.sm2.fixture.test.js`) ሁሉም እነዚህን ቋሚዎች ይተነትናል።
  - `crates/iroha/tests/sm_signing.rs` የሚወስን ፊርማ ይሠራል እና በሰንሰለት ላይ ያለው መልቲሃሽ/መልቲኮዴክ ውጤቶች ከመሳሪያው ጋር የሚዛመዱ መሆናቸውን ያረጋግጣል።
  - የመግቢያ ጊዜ መመለሻ ስብስቦች (`crates/iroha_core/tests/admission_batching.rs`) `allowed_signing` `sm2` * እና * `default_hash` `default_hash` `default_hash` `fixtures/sm/sm2_fixture.json` ካላካተተ በስተቀር የኤስኤም2 ጭነቶች ውድቅ ናቸው ይላሉ።
- 追加カバレッジ: 異常系（無効な曲線、異常な `r/s`、`distid` べ`crates/iroha_crypto/tests/sm2_fuzz.rs` の ንብረት テストで網羅済みです。አባሪ ምሳሌ 1 መልቲኮዴክ 形式で引き続き提供しています。
- የዝገት ኮድ አሁን `Sm2PublicKey::compute_z` ያጋልጣል ስለዚህ ZA ቋሚዎች በፕሮግራም ሊፈጠሩ ይችላሉ; ለ Annex D regression `sm2_compute_z_matches_annex_example` ይመልከቱ።

## ቀጣይ ድርጊቶች
- የማዋቀር ጌቲንግ የSM2 ማስቻል ድንበሮችን መተግበሩን ለመቀጠል የመግቢያ-ጊዜ ድግግሞሾችን (`admission_batching.rs`) ይቆጣጠሩ።
- ሽፋንን በWycheproof SM4 GCM/CCM ጉዳዮች ያራዝሙ እና ለSM2 ፊርማ ማረጋገጫ በንብረት ላይ የተመሰረቱ fuzz ኢላማዎችን ያግኙ። ✅ (ልክ ያልሆነ መያዣ ንዑስ ስብስብ በ`sm3_sm4_vectors.rs` ተይዟል።)
- ለአማራጭ መታወቂያዎች የኤል ኤም ኤል ጥያቄ፡ *"መለያ መታወቂያው ከALICE123@YAHOO.COM ይልቅ ወደ 1234567812345678 ሲዋቀር ለአባሪ ምሳሌ 1 የSM2 ፊርማ ያቅርቡ እና አዲሱን ZA/e እሴቶችን ይግለጹ።"*
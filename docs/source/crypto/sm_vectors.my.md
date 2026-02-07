---
lang: my
direction: ltr
source: docs/source/crypto/sm_vectors.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fd46471945188bcb95c8ee411c48acc8915a92b408df196caa65bf25f0596732
source_last_modified: "2026-01-05T18:22:23.402400+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! SM2/SM3/SM4 ပေါင်းစည်းခြင်းလုပ်ငန်းအတွက် အကိုးအကားစမ်းသပ် vector များ။

# SM Vectors Staging Notes

ဤစာတမ်းသည် SM2/SM3/SM4 ကြိုးများကို အလိုအလျောက် တင်သွင်းသည့် scripts များမပေါ်မီ လူသိရှင်ကြားရရှိနိုင်သော လူသိများသော အဖြေစစ်ဆေးမှုများကို စုစည်းထားသည်။ စက်ဖြင့်ဖတ်နိုင်သော မိတ္တူများ တိုက်ရိုက်ထုတ်လွှင့်သည်-

- `crates/iroha_crypto/tests/fixtures/sm_known_answers.toml` (နောက်ဆက်တွဲ vector များ၊ RFC 8998 ကိစ္စများ၊ နောက်ဆက်တွဲ ဥပမာ 1)။
- `fixtures/sm/sm2_fixture.json` (Rust/Python/JavaScript စမ်းသပ်မှုများမှ စားသုံးသော အဆုံးအဖြတ်ပေးသော SDK ပွဲစဉ်များကို မျှဝေထားသည်)။
- `crates/iroha_crypto/tests/fixtures/wycheproof_sm2.json` — စီစဥ်ထားသော 52-case corpus (အဆုံးအဖြတ်ပေးသော အတွဲများ + ပေါင်းစပ်ထားသော bit-flip/message/tail truncation negatives) ကို Apache-2.0 တွင် ဆိုင်းငံ့ထားစဉ် အထက်ပိုင်း SM2 suite ကို ဆိုင်းငံ့ထားသည်။ `crates/iroha_crypto/tests/sm2_wycheproof.rs` သည် ဖြစ်နိုင်သည့်အခါ စံ SM2 အတည်ပြုစနစ်ကို အသုံးပြု၍ ဤ vector များကို စစ်ဆေးပြီး လိုအပ်သည့်အခါတွင် နောက်ဆက်တွဲဒိုမိန်း၏ သန့်စင်သော BigInt အကောင်အထည်ဖော်မှုသို့ ပြန်သွားပါသည်။

## OpenSSL / Tongsuo / GmSSL ကိုဆန့်ကျင်သည့် SM2 လက်မှတ်အတည်ပြုခြင်း။

နောက်ဆက်တွဲ ဥပမာ 1 (Fp-256) သည် `ALICE123@YAHOO.COM` (ENTLA 0x0090)၊ မက်ဆေ့ဂျ် `"message digest"` နှင့် အောက်တွင်ဖော်ပြထားသော အများသူငှာသော့ကို အသုံးပြုသည်။ OpenSSL/Tongsuo အလုပ်အသွားအလာကို မိတ္တူကူးထည့်ခြင်းမှာ-

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

* OpenSSL 3.x သည် `distid:` / `hexdistid:` ရွေးချယ်စရာများကို မှတ်တမ်းတင်ထားသည်။ အချို့သော OpenSSL 1.1.1 တည်ဆောက်မှုတွင် ခလုတ်ကို `sm2_id:` အဖြစ် ဖော်ထုတ်သည်—`openssl pkeyutl -help` တွင် မည်သည့်အရာကိုမဆို အသုံးပြုပါ။
* GmSSL သည် တူညီသော `pkeyutl` မျက်နှာပြင်ကို တင်ပို့သည်။ အဟောင်းများသည် `-pkeyopt sm2_id:...` ကိုလက်ခံသည်။
* LibreSSL (macOS/OpenBSD တွင် ပုံသေ) သည် **SM2/SM3 ကို အကောင်အထည်မဖော်ပါ၊ ထို့ကြောင့် အထက်ပါ command သည် ထိုနေရာတွင် ပျက်သွားပါသည်။ OpenSSL ≥ 1.1.1၊ Tongsuo သို့မဟုတ် GmSSL ကိုသုံးပါ။

DER အကူအညီပေးသူက `3044022040F1EC59F793D9F49E09DCEF49130D4194F79FB1EED2CAA55BACDB49C4E755D102206FC6DAC32C5D5CF10C77DFB20F7C2EB667A457872FB09EC56327A67EC7DEEBE7` ကို ထုတ်လွှတ်သည်၊ ၎င်းသည် နောက်ဆက်တွဲ လက်မှတ်နှင့် ကိုက်ညီသည်။

နောက်ဆက်တွဲတွင် အသုံးပြုသူ-အချက်အလက် hash နှင့် ရလဒ်ဆိုင်ရာ အချက်များကို ပရင့်ထုတ်သည်-

```
ZA = F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A
e  = SM3(ZA || "message digest")
   = B524F552CD82B8B028476E005C377FB19A87E6FC682D48BB5D42E3D9B9EFFE76
```

OpenSSL ဖြင့် အတည်ပြုနိုင်သည်-

```bash
echo -n 'F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A' \
  | xxd -r -p > za.bin
printf "message digest" >> za.bin
openssl dgst -sm3 -binary za.bin | xxd -p -c 256
# -> b524f552cd82b8b028476e005c377fb19a87e6fc682d48bb5d42e3d9b9effe76
```

မျဉ်းကွေးညီမျှခြင်းအတွက် Python စိတ်ပိုင်းဆိုင်ရာ စစ်ဆေးခြင်း

```python
p = int("FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFF", 16)
a = int("FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFC", 16)
b = int("28E9FA9E9D9F5E344D5A9E4BCF6509A7F39789F515AB8F92DDBCBD414D940E93", 16)
x = int("0AE4C7798AA0F119471BEE11825BE46202BB79E2A5844495E97C04FF4DF2548A", 16)
y = int("7C0240F88F1CD4E16352A73C17B7F16F07353E53A176D684A9FE0C6BB798E857", 16)
assert (pow(y, 2, p) - (pow(x, 3, p) + a * x + b)) % p == 0
```

## SM3 Hash Vectors

| ထည့်သွင်း | Hex Encoding | Digest ( hex ) | အရင်းအမြစ် |
|---------|-----------------|-----------------|--------|
| `""` (စာကြောင်းဗလာ) | `""` | `1ab21d8355cfa17f8e61194831e81a8f22bec8c728fefb747ed035eb5082aa2b` | GM/T 0004-2012 နောက်ဆက်တွဲ A.1 |
| `"abc"` | `616263` | `66c7f0f462eeedd9d1f2d46bdc10e4e24167c4875cf2f7a2297da02b8f4ba8e0` | GM/T 0004-2012 နောက်ဆက်တွဲ A.2 |
| `"abcd"` ထပ်ခါတလဲလဲ 16 ကြိမ် (64 bytes) | `61626364` ×16 | `debe9ff92275b8a138604889c18e5a4d6fdb70e5387e5765293dcba39c0c5732` | GB/T 32905-2016 နောက်ဆက်တွဲ A |

## SM4 Block Cipher (ECB) Vectors

| သော့ (hex) | ရိုးရိုးစာသား ( hex ) | Ciphertext (hex) | အရင်းအမြစ် |
|-----------|-----------------|--------------------------------|--------|
| `0123456789abcdeffedcba9876543210` | `0123456789abcdeffedcba9876543210` | `681edf34d206965e86b3e94f536e4246` | GM/T 0002-2012 နောက်ဆက်တွဲ A.1 |
| `0123456789abcdeffedcba9876543210` | `000102030405060708090a0b0c0d0e0f` | `59b50808d3dcf921fa30b5b3c1dddc19` | GM/T 0002-2012 နောက်ဆက်တွဲ A.2 |
| `0123456789abcdeffedcba9876543210` | `ffeeddccbbaa99887766554433221100` | `1c3b3f56186b70819d3f5aa11fe2c8b6` | GM/T 0002-2012 နောက်ဆက်တွဲ A.3 |

## SM4-GCM Authenticated Encryption

| သော့ | IV | AAD | လွင်ပြင် | Ciphertext | Tag | အရင်းအမြစ် |
|-----|----|-----|-----------|-----------|-----|--------|
| `0123456789abcdeffedcba9876543210` | `00001234567800000000abcd` | `feedfacedeadbeeffeedfacedeadbeefabaddad2` | `d9313225f88406e5a55909c5aff5269a` | `42831ec2217774244b7221b784d0d49c` | `4d5c2af327cd64a62cf35abd2ba6fab4` | RFC 8998 နောက်ဆက်တွဲ A.2 |

## SM4-CCM Authenticated Encryption| သော့ | Nonce | AAD | လွင်ပြင် | Ciphertext | Tag | အရင်းအမြစ် |
|--|-------|-----|-----------|--------------------|-----|--------|
| `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `202122232425262728292a2b2c2d2e2f` | `7162015b4dac2555` | `4d26de5a` | RFC 8998 နောက်ဆက်တွဲ A.3 |

### Wycheproof Negative Cases (SM4 GCM/CCM)

ဤကိစ္စများသည် `crates/iroha_crypto/tests/sm3_sm4_vectors.rs` ရှိ ဆုတ်ယုတ်မှုအစုကို အကြောင်းကြားသည်။ အမှုတိုင်းကို စိစစ်မှု ပျက်ကွက်ရမည်။

| မုဒ် | TC ID | ဖော်ပြချက် | သော့ | Nonce | AAD | Ciphertext | Tag | မှတ်စုများ |
|---|-------|----------------|-----|-------|-----|-----------|-----|------|
| GCM | ၁ | Tag နည်းနည်းလှန် | `0123456789abcdeffedcba9876543210` | `00001234567800000000abcd` | `feedfacedeadbeeffeedfacedeadbeefabaddad2` | `42831ec2217774244b7221b784d0d49c` | `5d5c2af327cd64a62cf35abd2ba6fab4` | Wycheproof မှဆင်းသက်လာသော မမှန်ကန်သော tag |
| CCM | 17 | Tag နည်းနည်းလှန် | `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `7162015b4dac2555` | `5d26de5a` | Wycheproof မှဆင်းသက်လာသော မမှန်ကန်သော tag |
| CCM | 18 | ဖြတ်တောက်ထားသော tag (3 bytes) | `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `7162015b4dac2555` | `5d26de` | တိုတောင်းသော တဂ်များ အထောက်အထားမခိုင်လုံခြင်း |
| CCM | 19 | Ciphertext bitflip | `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `7162015b4dac2554` | `5d26de5a` | လက်ဆော့နေသော payload | ကို ထောက်လှမ်းပါ။

## SM2 Deterministic Signature အကိုးအကား

| လယ် | တန်ဖိုး (မှတ်စုမထားလျှင် hex) | အရင်းအမြစ် |
|---------|--------------------------------|--------|
| မျဉ်းကွေးဘောင်ဘောင်များ | `sm2p256v1` (a=`FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFC` စသည်ဖြင့်) | GM/T 0003.5-2012 နောက်ဆက်တွဲ A |
| အသုံးပြုသူ ID (`distid`) | ASCII `"ALICE123@YAHOO.COM"` (ENTLA 0x0090) | GM/T 0003 နောက်ဆက်တွဲ D |
| အများသူငှာသော့ | `040AE4C7798AA0F119471BEE11825BE46202BB79E2A5844495E97C04FF4DF2548A7C0240F88F1CD4E16352A73C17B7F16F07353E53A176D684A9FE0C6BB798E857` | GM/T 0003 နောက်ဆက်တွဲ D |
| စာတို | `"message digest"` (hex `6d65737361676520646967657374`) | GM/T 0003 နောက်ဆက်တွဲ D |
| ZA | `F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A` | GM/T 0003 နောက်ဆက်တွဲ D |
| `e = SM3(ZA || M)` | `B524F552CD82B8B028476E005C377FB19A87E6FC682D48BB5D42E3D9B9EFFE76` | GM/T 0003 နောက်ဆက်တွဲ D |
| လက်မှတ် `(r,s)` | `40F1EC59F793D9F49E09DCEF49130D4194F79FB1EED2CAA55BACDB49C4E755D1`, `6FC6DAC32C5D5CF10C77DFB20F7C2EB667A457872FB09EC56327A67EC7DEEBE7` | GM/T 0003 နောက်ဆက်တွဲ D |
| Multicodec (ယာယီ) | `8626550012414C494345313233405941484F4F2E434F4D040AE4…` (`sm2-pub`၊ ဗားရှင်း `0x1306`) | နောက်ဆက်တွဲ ဥပမာ 1 |
| ရှေ့နောက် multihash | `sm2:8626550012414C494345313233405941484F4F2E434F4D040AE4…` | (ကိုက်ညီသော `sm_known_answers.toml`) | ဆင်းသက်လာသည်။

SM2 multihash payload များကို `distid_len (u16 BE) || distid bytes || SEC1 uncompressed key (65 bytes)` အဖြစ် ကုဒ်လုပ်ထားပါသည်။

### Rust SDK Deterministic Signing Fixture (SM-3c)

တည်ဆောက်ထားသော vectors array တွင် Rust/Python/JavaScript parity payload ပါဝင်သည်။
ထို့ကြောင့် ဖောက်သည်တိုင်းသည် တူညီသော SM2 မက်ဆေ့ချ်ကို မျှဝေထားသော မျိုးစေ့ဖြင့် လက်မှတ်ရေးထိုးသည်။
ခွဲခြားသတ်မှတ်ခြင်း။| လယ် | တန်ဖိုး (မှတ်စုမထားလျှင် hex) | မှတ်စုများ |
|---------|--------------------------------|------|
| ID ခွဲခြားခြင်း | `"iroha-sdk-sm2-fixture"` | Rust/Python/JS SDKs |
| မျိုးစေ့ | `"iroha-rust-sdk-sm2-deterministic-fixture"` (hex `69726F68612D727573742D73646B2D736D322D64657465726D696E69737469632D66697874757265`) | `Sm2PrivateKey::from_seed` | သို့ ထည့်သွင်းပါ။
| သီးသန့်သော့ | `E64AE95930A2324DCF11BB1657FBBC2CD2BEF7515664BE7A15CA6AE5BCE0B7CA` | မျိုးစေ့ |
| အများသူငှာသော့ (SEC1 ကို ချုံ့မထားပါ) | `0494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` | အဆုံးအဖြတ် ဆင်းသက်ခြင်း |
| အများသုံးသော့ multihash | `862658001569726F68612D73646B2D736D322D666978747572650494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` | `PublicKey::to_string()` |
| ရှေ့နောက် multihash | `sm2:862658001569726F68612D73646B2D736D322D666978747572650494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` | `PublicKey::to_prefixed_string()` |
| ZA | `6B0229C3FBFE7A5DC6EE27228E18E512752DCCE9191EB381242A7342D568EDDE` | `Sm2PublicKey::compute_z` | မှတဆင့်တွက်ချက်သည်။
| စာတို | `"Rust SDK SM2 signing fixture v1"` (hex `527573742053444B20534D32207369676E696E672066697874757265207631`) | SDK တူညီမှုစမ်းသပ်မှုများ |
| လက်မှတ် `(r, s)` | `4E026F2E0FB75A6418C95C046D70DB7B95558BE30FB2001906EBE2091FA1AF76`, `299CFF374026D9E0C49C6BD4A99A1473BA17EFF429B3FAD083B1577F7559BDF5` | အဆုံးအဖြတ်လက်မှတ်ရေးထိုးခြင်း | မှတဆင့်ထုတ်လုပ်သည်။

- SDK ဖြတ်ကျော်သုံးစွဲမှု-
  - `fixtures/sm/sm2_fixture.json` သည် ယခု `vectors` ခင်းကျင်းမှုကို ဖော်ထုတ်သည်။ Rust crypto regression suite (`crates/iroha_crypto/tests/sm2_fixture_vectors.rs`)၊ Rust client helpers (`crates/iroha/src/sm.rs`)၊ Python bindings (`python/iroha_python/tests/test_crypto.py`) နှင့် JavaScript SDK (`javascript/iroha_js/test/crypto.sm2.fixture.test.js`) အားလုံးကို ခွဲခြမ်းစိတ်ဖြာပါ။
  - `crates/iroha/tests/sm_signing.rs` လေ့ကျင့်ခန်းသည် အဆုံးအဖြတ်ပေးသော လက်မှတ်ထိုးခြင်းနှင့် on-chain multihash/multicodec ရလဒ်များသည် fixture နှင့် ကိုက်ညီကြောင်း အတည်ပြုသည်။
  - `allowed_signing` တွင် `sm2` * နှင့် * `default_hash` သည် `sm3-256` မပါဝင်ပါက SM2 payload များကို ပယ်ချကြောင်း ဝန်ခံချက်-အချိန်ဆုတ်ယုတ်မှုအစုံများ (`crates/iroha_core/tests/admission_batching.rs`) က ငြင်းဆိုထားသည်။
- 追加カバレッジ: 異常系（無効な曲線、異常な `r/s`、`distid` 改ざ 180 I180の80NI）はテストで網羅済みです。 နောက်ဆက်တွဲ ဥပမာ 1の正規ベクトルは `sm_known_answers.toml` に多言語対応の multicodec 形弍で引せて
- ယခု Rust ကုဒ်သည် `Sm2PublicKey::compute_z` ကို ဖော်ထုတ်ပေးသောကြောင့် ZA ပစ္စည်းများကို ပရိုဂရမ်ဖြင့် ထုတ်ပေးနိုင်သည်။ နောက်ဆက်တွဲ D ဆုတ်ယုတ်မှုအတွက် `sm2_compute_z_matches_annex_example` ကိုကြည့်ပါ။

## နောက်တစ်ခုလုပ်ဆောင်ချက်များ
- config gating သည် SM2 ဖွင့်ခြင်းနယ်နိမိတ်များကို ဆက်လက်ကျင့်သုံးကြောင်း သေချာစေရန် ဝင်ခွင့်-အချိန်ဆုတ်ယုတ်မှုများ (`admission_batching.rs`) ကို စောင့်ကြည့်ပါ။
- Wycheproof SM4 GCM/CCM အမှုတွဲများဖြင့် အကျုံးဝင်မှုကို တိုးချဲ့ပြီး SM2 လက်မှတ်အတည်ပြုခြင်းအတွက် ပိုင်ဆိုင်မှုအခြေခံ fuzz ပစ်မှတ်များကို ရယူပါ။ ✅ (`sm3_sm4_vectors.rs` တွင် ဖမ်းယူထားသော မမှန်ကန်သော အသေးစိပ်အတွဲ။
- အခြား ID များအတွက် LLM အမှာစာ- *"ခွဲခြား ID ကို 1234567812345678 သို့ ALICE123@YAHOO.COM အစား 1234567812345678 သို့ သတ်မှတ်သည့်အခါ *"နောက်ဆက်တွဲ ဥပမာအတွက် SM2 လက်မှတ်ကို SM2 လက်မှတ်ကို ပေးဆောင်ပါ။"*
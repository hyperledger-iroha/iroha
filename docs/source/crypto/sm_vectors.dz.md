---
lang: dz
direction: ltr
source: docs/source/crypto/sm_vectors.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fd46471945188bcb95c8ee411c48acc8915a92b408df196caa65bf25f0596732
source_last_modified: "2026-01-05T18:22:23.402400+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

///! SM2/SM3/SM4 མཉམ་བསྡོམས་ལཱ་གི་དོན་ལུ་ གཞི་བསྟུན་བརྟག་དཔྱད་ཝེག་ཊར་ཚུ།

# ཨེསི་ཨེམ་ ཝེག་ཊར་ཚུ་ གནས་རིམ་དྲན་ཐོ།

ཡིག་ཆ་འདི་གིས་ མི་མང་ལུ་ཐོབ་ཚུགས་པའི་ ཤེས་རྟོགས་ཡོད་པའི་བརྟག་དཔྱད་ཚུ་ རང་བཞིན་གྱིས་ ནང་འདྲེན་འབད་མི་ ཡིག་ཆ་ཚུ་ མ་བཙུགས་པའི་ཧེ་མ་ SM2/SM3/SM4 གི་ harness ཚུ་ བསྡུ་སྒྲིག་འབདཝ་ཨིན། འཕྲུལ་ཆས་ཀྱིས་ལྷག་བཏུབ་པའི་འདྲ་བཤུས་ཚུ་ ནང་སྡོདཔ་ཨིན།

- `crates/iroha_crypto/tests/fixtures/sm_known_answers.toml` (སྦྲེལ་མཐུད་ཝེག་ཊར་, ཨར་ཨེཕ་སི་ ༨༩༩༨ གནད་དོན་ཚུ་, ཟུར་ཐོ་དཔེ་ ༡)།
- `fixtures/sm/sm2_fixture.json` (རསཊི་/པི་ཐོན་/ཇ་བ་ཨིསི་ཀིརིཔཊི་བརྟག་དཔྱད་ཚུ་གིས་ བཀོལ་སྤྱོད་འབད་མི་ གཏན་འབེབས་ཨེསི་ཌི་ཀེ་ སྒྲིག་ཆས་བརྗེ་སོར་འབད་ཡོདཔ།)།
- `crates/iroha_crypto/tests/fixtures/wycheproof_sm2.json` — གནས་སྟངས་༥༢ གི་ ཀོར་པཱསི་ (གཏན་འབེབས་ཅན་གྱི་སྒྲིག་ཆས་ + བིཊི་-ཕིལཔ་/མེས་སེ་ཇི་/མཇུག་མའི་ བསྒྱིར་ཚད་ ནེ་གེ་ཊིན) མཉམ་བསྡོམ་འབད་ཡོདཔ་ཨིན་) ཨེཔ་ཆི་-༢.༠ གི་འོག་ལུ་ མེ་ལོང་ནང་ མཉམ་བསྡོམས་འབད་ཡོདཔ་ཨིན། `crates/iroha_crypto/tests/sm2_wycheproof.rs` གིས་ འབད་ཚུགས་པའི་སྐབས་ ཚད་ལྡན་ཨེསི་ཨེམ་༢ བདེན་བཤད་ལག་ལེན་འཐབ་སྟེ་ བདེན་དཔྱད་འབདཝ་ཨིནམ་དང་ དགོས་མཁོ་ཡོད་པའི་སྐབས་ ཨེནགསི་མངའ་ཁོངས་ཀྱི་ བིགའིནཊི་བིག་ཨིན་ཊི་ ངོ་མ་ལུ་ ལོག་འགྱོཝ་ཨིན།

## SM2 མཚན་རྟགས་བདེན་དཔྱད། OpenSSL / ཊོང་སུའོ་ / ཇི་ཨེམ་ཨེསི་ཨེསི་ཨེལ།

ཟུར་ཐོ་དཔེ་ ༡ (Fp-256) གིས་ ངོ་རྟགས་ `ALICE123@YAHOO.COM` (ENTLA 0x0090), བརྡ་འཕྲིན་ `"message digest"`, དང་ འོག་ལུ་སྟོན་ཡོད་མི་ མི་མང་ལྡེ་མིག་ཚུ་ ལག་ལེན་འཐབ་ཨིན། འདྲ་བཤུས་-དང་《སྦྱར་བའི་ཨོ་པན་ཨེསི་ཨེསི་ཨེལ་/ཊོང་སུའོ་ལས་ཀའི་རྒྱུན་རིམ་འདི་:

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

* OpenSSL 3.x གིས་ `distid:` / `hexdistid:` གདམ་ཁ་ཚུ་ཡིག་ཆ་བཟོཝ་ཨིན། OpenSSL 1.1.1 ལ་ལུ་ཅིག་གིས་ knob འདི་ `sm2_id:` སྦེ་ བཟོ་བསྐྲུན་འབདཝ་ཨིན།
* GmSSL གིས་ `pkeyutl` ཁ་ཐོག་གཅིག་ཕྱིར་འདྲེན་འབདཝ་ཨིན། རྙིངམ་བཟོ་བསྐྲུན་ཚུ་གིས་ཡང་ `-pkeyopt sm2_id:...` ངོས་ལེན་འབད་ཡོདཔ་ཨིན།
* LibreSSL (macOS/OpenBSD གུ་ སྔོན་སྒྲིག་) **not** གིས་ SM2/SM3 ལག་ལེན་འཐབ་དོ་ཡོདཔ་ལས་ གོང་འཁོད་བརྡ་བཀོད་འདི་ དེ་ཁར་ འཐུས་ཤོར་བྱུང་ཡོདཔ་ཨིན། OpenSSL ≥ 1.1.1, ཊོང་སུའོ་ ཡང་ན་ GmSSL ལག་ལེན་འཐབ།

ཌི་ཨི་ཨར་ གྲོགས་རམ་པ་གིས་ `3044022040F1EC59F793D9F49E09DCEF49130D4194F79FB1EED2CAA55BACDB49C4E755D102206FC6DAC32C5D5CF10C77DFB20F7C2EB667A457872FB09EC56327A67EC7DEEBE7` འདི་ མཐུད་སྦྱོར་གྱི་མིང་རྟགས་དང་མཐུན་སྒྲིག་འབདཝ་ཨིན།

མཐུད་མཚམས་འདི་གིས་ ལག་ལེན་པའི་བརྡ་དོན་ཧེཤ་དང་ གྲུབ་འབྲས་འཇུ་བྱེད་ཚུ་ཡང་ དཔར་བསྐྲུན་འབདཝ་ཨིན།

```
ZA = F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A
e  = SM3(ZA || "message digest")
   = B524F552CD82B8B028476E005C377FB19A87E6FC682D48BB5D42E3D9B9EFFE76
```

ཁྱོད་ཀྱིས་ OpenSSL དང་གཅིག་ཁར་ངེས་དཔྱད་འབད་ཚུགས།

```bash
echo -n 'F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A' \
  | xxd -r -p > za.bin
printf "message digest" >> za.bin
openssl dgst -sm3 -binary za.bin | xxd -p -c 256
# -> b524f552cd82b8b028476e005c377fb19a87e6fc682d48bb5d42e3d9b9effe76
```

གུག་གུགཔ་སྙོམ་རྩིས་ཀྱི་དོན་ལས་ པའི་ཐོན་ སེན་ཊི་བརྟག་དཔྱད།

```python
p = int("FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFF", 16)
a = int("FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFC", 16)
b = int("28E9FA9E9D9F5E344D5A9E4BCF6509A7F39789F515AB8F92DDBCBD414D940E93", 16)
x = int("0AE4C7798AA0F119471BEE11825BE46202BB79E2A5844495E97C04FF4DF2548A", 16)
y = int("7C0240F88F1CD4E16352A73C17B7F16F07353E53A176D684A9FE0C6BB798E857", 16)
assert (pow(y, 2, p) - (pow(x, 3, p) + a * x + b)) % p == 0
```

## SM3 ཧ་ཤི་ཝེག་ཊར།

| ཨིན་པུཊི་ | ཧེགསི་ཨེན་ཀོ་ཌིང་ | ཌའི་ཇེསཊ་ (ཧེགསི་) | ཡོང་ཁུངས། |
|---------------------------------------------------------------------------------- --------|
| `""` (ཡིག་རྒྱུན་སྟོངམ་) | `""` | `1ab21d8355cfa17f8e61194831e81a8f22bec8c728fefb747ed035eb5082aa2b` | ཇི་ཨེམ་/ཊི་ ༠༠༠༤-༢༠༡༢ ཨེན།
| `"abc"` | `616263` | `66c7f0f462eeedd9d1f2d46bdc10e4e24167c4875cf2f7a2297da02b8f4ba8e0` | ཇི་ཨེམ་/ཊི་ ༠༠༠༤-༢༠༡༢ ཨེན་ནེགསི་ཨེ་༢ |
| `"abcd"` བསྐྱར་ལོག་ཐེངས་༡༦ (བཱའིཊི་༦༤) | `61626364` ×16 | `debe9ff92275b8a138604889c18e5a4d6fdb70e5387e5765293dcba39c0c5732` | ཇི་བྷི་/ཊི་ ༣༢༩༠༥-༢༠༡༦ ཟུར་ཐོ་ཨེ་ |

## SM4 སྡེབ་ཚན་སི་ཕར་ (ECB) ཝེག་ཊར་ཚུ།

| ལྡེ་མིག་ (ཧེགསི་) | གད་སྙིགས་ (hex) | སི་ཕར་ཊེགསི་ (hex) | ཡོང་ཁུངས། |
|--------------------------------------------------------------------------------------- |
| `0123456789abcdeffedcba9876543210` | `0123456789abcdeffedcba9876543210` | `681edf34d206965e86b3e94f536e4246` | ཇི་ཨེམ་/ཊི་ ༠༠༠༢-༢༠༡༢ ཨེན།༡ |
| `0123456789abcdeffedcba9876543210` | `000102030405060708090a0b0c0d0e0f` | `59b50808d3dcf921fa30b5b3c1dddc19` | ཇི་ཨེམ་/ཊི་ ༠༠༠༢-༢༠༡༢ ཨེན་ནེགསི་ཨེ་༢ |
| `0123456789abcdeffedcba9876543210` | `ffeeddccbbaa99887766554433221100` | `1c3b3f56186b70819d3f5aa11fe2c8b6` | ཇི་ཨེམ་/ཊི་ ༠༠༠༢-༢༠༡༢ ཨེན་ནེགསི་ཨེ་༣ |

## SM4-GCM བདེན་བཤད་ཀྱི་གསང་བཟོ།

| ལྡེ་མིག་ | IV | AAD | པལ་ཊེགསཊི་ | སི་ཕར་ཊེགསི་ | རྟགས་ | ཡོང་ཁུངས། |
|--|-|-|---------------------------------------------------------------------
| `0123456789abcdeffedcba9876543210` | `00001234567800000000abcd` | `feedfacedeadbeeffeedfacedeadbeefabaddad2` | `d9313225f88406e5a55909c5aff5269a` | `42831ec2217774244b7221b784d0d49c` | `4d5c2af327cd64a62cf35abd2ba6fab4` | RFC 8998 ཟུར་དེབ་ A.2 |

## SM4-CCM བདེན་བཤད་ གསང་བཟོ།| ལྡེ་མིག་ | མེན་ | AAD | པལ་ཊེགསཊི་ | སི་ཕར་ཊེགསི་ | རྟགས་ | ཡོང་ཁུངས། |
|--|-|-|----------------------------------------------------------------- ------------------------------
| `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `202122232425262728292a2b2c2d2e2f` | `7162015b4dac2555` | `4d26de5a` | RFC 8998 ཟུར་དེབ་ A.3 |

### ཝི་ཅི་པྲོ་མེད་པའི་ ངན་པའི་གནད་དོན་ (SM4 GCM/CCM)

གནད་དོན་འདི་ཚུ་གིས་ `crates/iroha_crypto/tests/sm3_sm4_vectors.rs` ནང་ལུ་ འགྱུར་ལྡོག་ཆ་ཚང་འདི་ བརྡ་སྤྲོད་འབདཝ་ཨིན། གནད་དོན་རེ་རེ་གིས་ བདེན་དཔྱད་འབད་མ་བཏུབ་དགོ།

| ཐབས་ལམ་ | TC ID | འགྲེལ་བཤད་ | ལྡེ་མིག་ | མེན་ | AAD | སི་ཕར་ཊེགསི་ | རྟགས་ | དྲན་ཐོ། |
|--|-|-|-|-|-|-------------------------------------------------------------------------------------------------------  ------------                                        
| GCM | 1 | ཊེག་བིཊི་ཕིལཔ་ | `0123456789abcdeffedcba9876543210` | `00001234567800000000abcd` | `feedfacedeadbeeffeedfacedeadbeefabaddad2` | `42831ec2217774244b7221b784d0d49c` | ```bash
echo -n 'F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A' \
  | xxd -r -p > za.bin
printf "message digest" >> za.bin
openssl dgst -sm3 -binary za.bin | xxd -p -c 256
# -> b524f552cd82b8b028476e005c377fb19a87e6fc682d48bb5d42e3d9b9effe76
``` | Wycheproouf-derived ནུས་མེད་རྟགས་ |
| CCM | 17 | ཊེག་བིཊི་ཕིལཔ་ | ```python
p = int("FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFF", 16)
a = int("FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFC", 16)
b = int("28E9FA9E9D9F5E344D5A9E4BCF6509A7F39789F515AB8F92DDBCBD414D940E93", 16)
x = int("0AE4C7798AA0F119471BEE11825BE46202BB79E2A5844495E97C04FF4DF2548A", 16)
y = int("7C0240F88F1CD4E16352A73C17B7F16F07353E53A176D684A9FE0C6BB798E857", 16)
assert (pow(y, 2, p) - (pow(x, 3, p) + a * x + b)) % p == 0
``` | `10111213141516` | `fixtures/sm/sm2_fixture.json` | `fixtures/sm/sm2_fixture.json` | `crates/iroha_crypto/tests/sm2_wycheproof.rs` | Wycheproouf-derived ནུས་མེད་རྟགས་ |
| CCM | 18 | བར་མཚམས་རྟགས་ (bytes) | `ALICE123@YAHOO.COM` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `7162015b4dac2555` | `5d26de` | ངོ་རྟགས་ཐུང་ཀུ་ཚུ་ བདེན་བཤད་འབད་མ་ཚུགསཔ་ངེས་གཏན་བཟོཝ་ཨིན། |
| CCM | 19 | སི་ཕར་ཊེགསི་བིཊི་ཕིལཔ་ | `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `crates/iroha_crypto/tests/fixtures/wycheproof_sm2.json` | `5d26de5a` | བརྟག་དཔྱད་བྱས་པའི་ པེ་ལོཌ་ |

## SM2 ངེས་འཛིན་མཚན་རྟགས་རྒྱབ་ཚུལ།

| ཕིལཌ་ | གནས་གོང་ (མཆན་མེད་) | ཡོང་ཁུངས། |
|-------------------------------------------------- |
| གུག་གུགཔ་ཚད་བཟུང་ཚུ་ | `sm2p256v1` (a = `FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFC` ལ་སོགས་པ།) | ཇི་ཨེམ་/ཊི་ ༠༠༠༣.༥-༢༠༡༢ ཟུར་ཐོ་ཨེ་ |
| ལག་ལེན་པའི་ཨའི་ཌི་ (`distid`) | ASCII `"ALICE123@YAHOO.COM"` (EnTLA 0x0090) | GM/T 0003 ཟུར་ཐོ་ D |
| མི་མང་ལྡེ་མིག་ | `040AE4C7798AA0F119471BEE11825BE46202BB79E2A5844495E97C04FF4DF2548A7C0240F88F1CD4E16352A73C17B7F16F07353E53A176D684A9FE0C6BB798E857` | GM/T 0003 ཟུར་ཐོ་ D |
| འཕྲིན་དོན། | `"message digest"` (ཧེགསི་ `6d65737361676520646967657374`) | GM/T 0003 ཟུར་ཐོ་ D |
| ZA | `F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A` | GM/T 0003 ཟུར་ཐོ་ D |
| `e = SM3(ZA || M)` | `crates/iroha_crypto/tests/sm2_wycheproof.rs` | GM/T 0003 ཟུར་ཐོ་ D |
| མཚན་རྟགས་`(r,s)` | `40F1EC59F793D9F49E09DCEF49130D4194F79FB1EED2CAA55BACDB49C4E755D1`, `6FC6DAC32C5D5CF10C77DFB20F7C2EB667A457872FB09EC56327A67EC7DEEBE7` | GM/T 0003 ཟུར་ཐོ་ D |
| མལ་ཊི་ཀོ་ཌེག་ (ཕྲུ་གུ་) | `8626550012414C494345313233405941484F4F2E434F4D040AE4…` (`sm2-pub`, varint `0x1306`) | ཟུར་སྦྲའི་དཔེ་རིས་ ༡ ལས་བྱུང་ཡོད།
| སྔོན་འཇུག་མང་བའི་མལ་ཊི་ཧཤ་ | `sm2:8626550012414C494345313233405941484F4F2E434F4D040AE4…` | འབྱུང་ཁུངས་ (མཐུན་སྒྲིག་ `sm_known_answers.toml`) |

SM2 མལ་ཊི་ཧཤ་པེ་ལོཌ་ཚུ་ `distid_len (u16 BE) || distid bytes || SEC1 uncompressed key (65 bytes)` སྦེ་ ཨིན་ཀོཌི་འབད་ཡོདཔ་ཨིན།

### རསཊ་ཨེསི་ཌི་ཀེ་ གཏན་འཁེལ་གྱི་མཚན་རྟགས་སྒྲིག་བཀོད་ (SM-3c)

གཞི་བཀོད་འབད་ཡོད་པའི་བེག་ཊར་ཨེ་རེ་ནང་ རཱསི་ཊི་/པི་ཐོན་/ཇ་བ་ཨིསི་ཀིརིཔ་ ཆ་སྙོམས་ པེ་རེསི་ཚུ་ཚུདཔ་ཨིན།
དེ་འབདཝ་ལས་ མཁོ་མངགས་འབད་མི་ག་ར་གིས་ ཨེསི་ཨེམ་༢ འཕྲིན་དོན་གཅིག་ལུ་ མཚན་རྟགས་བཀོད་ཡོདཔ་ད་ དེ་ཡང་ བརྗེ་སོར་འབད་ཡོད་པའི་ སོན་དང་གཅིག་ཁར་
ངོས་འཛིན་འབད་མི་དབྱེ་བ་ཕྱེ་ནི།| ཕིལཌ་ | གནས་གོང་ (མཆན་མེད་) | དྲན་ཐོ། |
|------------------------------------------------- |
| ID དབྱེ་བ་ཕྱེ་ནི། | `"iroha-sdk-sm2-fixture"` | རས་ཊི་/པི་ཐོན་/ཇེ་ཨེསི་ཨེསི་ཌི་ཀེ་ཨེསི་ནང་བརྗེ་སོར་འབད་ཡོདཔ། |
| སོན་ | `ALICE123@YAHOO.COM` (ཧེགསི་ `69726F68612D727573742D73646B2D736D322D64657465726D696E69737469632D66697874757265`) | ཨིན་པུཊ་ `Sm2PrivateKey::from_seed` ལུ་ |
| སྒེར་གྱི་ལྡེ་མིག་ | `E64AE95930A2324DCF11BB1657FBBC2CD2BEF7515664BE7A15CA6AE5BCE0B7CA` | སོན་ལས་ གཏན་འབེབས་སྦེ་ ཐོན་ཡོདཔ། |
| མི་མང་ལྡེ་མིག་ (SEC1 བསྡམ་མེད་) | `0494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` | མཐུན་སྒྲིག་ཡོད་པའི་ determistic derabation |
| མི་མང་ལྡེ་མིག་ མལ་ཊི་ཧཤ་ | `862658001569726F68612D73646B2D736D322D666978747572650494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` | `PublicKey::to_string()` གི་ཐོན་འབྲས་ |
| སྔོན་འཇུག་མང་བའི་མལ་ཊི་ཧཤ་ | `sm2:862658001569726F68612D73646B2D736D322D666978747572650494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` | `PublicKey::to_prefixed_string()` གི་ཐོན་འབྲས་ |
| ZA | `6B0229C3FBFE7A5DC6EE27228E18E512752DCCE9191EB381242A7342D568EDDE` | 18NI00000098X རྩིས་རྒྱག་བྱས། |
| འཕྲིན་དོན། | `"message digest"` (ཧེགསི་ `527573742053444B20534D32207369676E696E672066697874757265207631`) | SDK parity tests གི་དོན་ལུ་ ཀེན་ནི་ཀཱལ་པེ་ལོསི། |
| མཚན་རྟགས་`(r, s)` | `4E026F2E0FB75A6418C95C046D70DB7B95558BE30FB2001906EBE2091FA1AF76`, `299CFF374026D9E0C49C6BD4A99A1473BA17EFF429B3FAD083B1577F7559BDF5` | ཐག་གཅོད་བརྒྱུད་དེ་ ཐག་གཅོད་འབད་ཡོདཔ། |

- ཀོརསི་-ཨེསི་ཌི་ཀེ་ སྤྱོད་ཐངས།
  - `fixtures/sm/sm2_fixture.json` གིས་ད་ལྟོ་ `vectors` གི་ཨེ་རེ་ཅིག་གསལ་བཏོན་འབདཝ་ཨིན། རསཊ་ཀིརིཔ་ཊོ་རི་གེ་རེ་ཤཱན་ (`crates/iroha_crypto/tests/sm2_fixture_vectors.rs`) དང་ རཱསཊ་མཁོ་སྤྲོད་པ་ (`crates/iroha/src/sm.rs`) དང་ པའི་ཐོན་བཱའིན་ཌིང་ (`python/iroha_python/tests/test_crypto.py`) དེ་ལས་ ཇ་བ་ཨིསི་ཀིརིཔཊི་ཨེསི་ཌི་ཀེ་ (`javascript/iroha_js/test/crypto.sm2.fixture.test.js`) ཚུ་ མིང་ཚིག་འདི་ཚུ་ག་ར་ ཆ་འཇོག་འབདཝ་ཨིན།
  - `crates/iroha/tests/sm_signing.rs` ལག་ལེན་འཐབ་མི་ཚུ་གིས་ མིང་རྟགས་བཀོད་ཞིནམ་ལས་ on-chain multimhashas/multicodec ཐོན་འབྲས་ཚུ་ བརྟན་བཞུགས་དང་མཐུན་སྒྲིག་ཡོདཔ་སྦེ་ བདེན་དཔྱད་འབདཝ་ཨིན།
  - འཛུལ་ཞུགས་-དུས་ཚོད་ཀྱི་ ལོག་ལྟའི་ཆ་ཤས་ (```
ZA = F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A
e  = SM3(ZA || "message digest")
   = B524F552CD82B8B028476E005C377FB19A87E6FC682D48BB5D42E3D9B9EFFE76
```) གིས་ `allowed_signing` ཚུ་ ཚུདཔ་ལས་ ངོས་ལེན་མ་འབད་བར་ རིམ་སྒྲིག་བཀག་ཆ་ཚུ་ བཀབ་བཞག་མི་ `default_hash` ཨིན།
- 加カババジ: 異常系（努異異異常な I00000116X、I00000117X 改らららエ（ぎジ: `crates/iroha_crypto/tests/sm2_fuzz.rs` の property テストで網羅済みです。Annex Example 1 の正規ベクトルは `sm_known_answers.toml`に多言語寿の 形式で引続き提供しいますあ
- རསཊི་གསང་ཡིག་གིས་ ད་ལྟོ་ `Sm2PublicKey::compute_z` ཕྱིར་བཏོན་འབདཝ་ཨིན་ དེ་འབདཝ་ལས་ ZA གི་སྒྲིག་བཀོད་ཚུ་ ལས་རིམ་གྱི་ཐོག་ལས་ བཟོ་བཏོན་འབད་ཚུགས། བལྟ་ `sm2_compute_z_matches_annex_example` ཟུར་སྦྲགས་ ཌི་འགྱུར་ལྡོག་དོན་ལུ་བལྟ།

## ཤུལ་མམ་གྱི་བྱ་བ།
- འཛུལ་ཞུགས་-དུས་ཚོད་ཀྱི་འགྱུར་ལྡོག་ཚུ་ (`admission_batching.rs`) རིམ་སྒྲིག་སྒོ་སྒྲིག་གིས་ SM2 ལྕོགས་ཅན་མཚམས་ཚུ་ འཕྲོ་མཐུད་དེ་ བསྟར་སྤྱོད་འབད་ནི་ལུ་ ངེས་གཏན་བཟོ་ནི་ལུ་ ལྟ་རྟོག་འབད།
- Wycheproof SM4 GCM/CCM གི་གནད་དོན་ཚུ་དང་ SM2 མཚན་རྟགས་བདེན་དཔྱད་ཀྱི་དོན་ལུ་ རྒྱུ་དངོས་གཞི་བཞག་པའི་ ཕཱ་ཟི་དམིགས་གཏད་ཚུ་ ཁྱབ་སྤེལ་འབད། ✅ (`sm3_sm4_vectors.rs` ནང་འཛིན་བཟུང་འབད་ཡོད་པའི་ ནུས་མེད་གནས་སྟངས་ཆ་ཚན་འོག་མ་)།
- LLM diserent: *"དབྱེ་འབྱེད་ཨའི་ཌི་འདི་ ALICE123@YHOOO.COM གི་ཚབ་ལུ་ ༡༢༣༤༥༦༧༨༡༢༣༤༥༦༧༨ ལུ་ ཨེསི་ཨེམ་༢ མཚན་རྟགས་བྱིནམ་ཨིན།
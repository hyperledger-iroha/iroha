---
lang: uz
direction: ltr
source: docs/source/crypto/sm_vectors.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fd46471945188bcb95c8ee411c48acc8915a92b408df196caa65bf25f0596732
source_last_modified: "2026-01-05T18:22:23.402400+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! SM2/SM3/SM4 integratsiya ishi uchun mos yozuvlar test vektorlari.

# SM vektorlari sahnalashtirish qaydlari

Ushbu hujjat SM2/SM3/SM4 jabduqlarini avtomatlashtirilgan import skriptlari joylashishidan oldin ulaydigan hammaga ochiq javob testlarini jamlaydi. Mashinada o'qiladigan nusxalar quyidagilarda yashaydi:

- `crates/iroha_crypto/tests/fixtures/sm_known_answers.toml` (ilova vektorlari, RFC 8998 holatlari, 1-ilova misoli).
- `fixtures/sm/sm2_fixture.json` (Rust/Python/JavaScript testlari tomonidan iste'mol qilinadigan umumiy deterministik SDK moslamasi).
- `crates/iroha_crypto/tests/fixtures/wycheproof_sm2.json` - yuqori oqim SM2 to'plami kutilayotgan paytda Apache-2.0 ostida aks ettirilgan 52 holatli korpus (deterministik moslamalar + sintezlangan bit-flip/xabar/dumni qisqartirish negativlari). `crates/iroha_crypto/tests/sm2_wycheproof.rs` bu vektorlarni iloji bo'lsa standart SM2 verifier yordamida tekshiradi va kerak bo'lganda Ilova domenining sof BigInt ilovasiga qaytadi.

## OpenSSL / Tongsuo / GmSSL ga qarshi SM2 imzosini tekshirish

1-ilova misolida (Fp-256) `ALICE123@YAHOO.COM` (ENTLA 0x0090) identifikatori, `"message digest"` xabari va quyida ko‘rsatilgan ochiq kalitdan foydalaniladi. Nusxalash va joylashtirish OpenSSL/Tongsuo ish jarayoni:

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

* OpenSSL 3.x `distid:` / `hexdistid:` parametrlarini hujjatlashtiradi. Ba'zi OpenSSL 1.1.1 tuzilmalari tugmani `sm2_id:` sifatida ko'rsatadi - `openssl pkeyutl -help` da paydo bo'lganidan foydalaning.
* GmSSL bir xil `pkeyutl` sirtini eksport qiladi; eski tuzilmalar ham `-pkeyopt sm2_id:...` ni qabul qildi.
* LibreSSL (macOS/OpenBSD da sukut bo'yicha) SM2/SM3 ni qo'llamaydi**, shuning uchun yuqoridagi buyruq u erda bajarilmaydi. OpenSSL ≥ 1.1.1, Tongsuo yoki GmSSL-dan foydalaning.

DER yordamchisi ilova imzosiga mos keladigan `3044022040F1EC59F793D9F49E09DCEF49130D4194F79FB1EED2CAA55BACDB49C4E755D102206FC6DAC32C5D5CF10C77DFB20F7C2EB667A457872FB09EC56327A67EC7DEEBE7` ni chiqaradi.

Ilova shuningdek, foydalanuvchi ma'lumotlari xeshini va natijada olingan dayjestni chop etadi:

```
ZA = F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A
e  = SM3(ZA || "message digest")
   = B524F552CD82B8B028476E005C377FB19A87E6FC682D48BB5D42E3D9B9EFFE76
```

Siz OpenSSL bilan tasdiqlashingiz mumkin:

```bash
echo -n 'F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A' \
  | xxd -r -p > za.bin
printf "message digest" >> za.bin
openssl dgst -sm3 -binary za.bin | xxd -p -c 256
# -> b524f552cd82b8b028476e005c377fb19a87e6fc682d48bb5d42e3d9b9effe76
```

Egri tenglama uchun Python aql-idrokini tekshirish:

```python
p = int("FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFF", 16)
a = int("FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFC", 16)
b = int("28E9FA9E9D9F5E344D5A9E4BCF6509A7F39789F515AB8F92DDBCBD414D940E93", 16)
x = int("0AE4C7798AA0F119471BEE11825BE46202BB79E2A5844495E97C04FF4DF2548A", 16)
y = int("7C0240F88F1CD4E16352A73C17B7F16F07353E53A176D684A9FE0C6BB798E857", 16)
assert (pow(y, 2, p) - (pow(x, 3, p) + a * x + b)) % p == 0
```

## SM3 xesh vektorlari

| Kirish | Hex kodlash | Dijest (hex) | Manba |
|-------|--------------|--------------|--------|
| `""` (bo'sh qator) | `""` | `1ab21d8355cfa17f8e61194831e81a8f22bec8c728fefb747ed035eb5082aa2b` | GM/T 0004-2012 A.1-ilova |
| `"abc"` | `616263` | `66c7f0f462eeedd9d1f2d46bdc10e4e24167c4875cf2f7a2297da02b8f4ba8e0` | GM/T 0004-2012 A.2-ilova |
| `"abcd"` 16 marta takrorlangan (64 bayt) | `61626364` ×16 | `debe9ff92275b8a138604889c18e5a4d6fdb70e5387e5765293dcba39c0c5732` | GB/T 32905-2016 A ilova |

## SM4 Block Cipher (ECB) vektorlari

| Kalit (olti burchakli) | To'g'ri matn (olti burchakli) | Shifrlangan matn (hex) | Manba |
|----------|-----------------|------------------|--------|
| `0123456789abcdeffedcba9876543210` | `0123456789abcdeffedcba9876543210` | `681edf34d206965e86b3e94f536e4246` | GM/T 0002-2012 A.1-ilova |
| `0123456789abcdeffedcba9876543210` | `000102030405060708090a0b0c0d0e0f` | `59b50808d3dcf921fa30b5b3c1dddc19` | GM/T 0002-2012 A.2-ilova |
| `0123456789abcdeffedcba9876543210` | `ffeeddccbbaa99887766554433221100` | `1c3b3f56186b70819d3f5aa11fe2c8b6` | GM/T 0002-2012 A.3-ilova |

## SM4-GCM Autentifikatsiya qilingan shifrlash

| Kalit | IV | AAD | Oddiy matn | Shifrlangan matn | teg | Manba |
|-----|----|-----|-----------|------------|-----|--------|
| `0123456789abcdeffedcba9876543210` | `00001234567800000000abcd` | `feedfacedeadbeeffeedfacedeadbeefabaddad2` | `d9313225f88406e5a55909c5aff5269a` | `42831ec2217774244b7221b784d0d49c` | `4d5c2af327cd64a62cf35abd2ba6fab4` | RFC 8998 A.2-ilova |

## SM4-CCM autentifikatsiyalangan shifrlash| Kalit | Nonce | AAD | Oddiy matn | Shifrlangan matn | teg | Manba |
|-----|-------|-----|-----------|------------|-----|--------|
| `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `202122232425262728292a2b2c2d2e2f` | `7162015b4dac2555` | `4d26de5a` | RFC 8998 A.3-ilova |

### Wycheproof salbiy holatlar (SM4 GCM/CCM)

Ushbu holatlar `crates/iroha_crypto/tests/sm3_sm4_vectors.rs` da regressiya to'plami haqida xabar beradi. Har bir holatda tekshirish muvaffaqiyatsiz bo'lishi kerak.

| Rejim | TC ID | Tavsif | Kalit | Nonce | AAD | Shifrlangan matn | teg | Eslatmalar |
|------|-------|-------------|-----|-------|-----|------------|-----|-------|
| GCM | 1 | Teg bitni aylantirish | `0123456789abcdeffedcba9876543210` | `00001234567800000000abcd` | `feedfacedeadbeeffeedfacedeadbeefabaddad2` | `42831ec2217774244b7221b784d0d49c` | `5d5c2af327cd64a62cf35abd2ba6fab4` | Wycheproof-dan olingan yaroqsiz teg |
| CCM | 17 | Teg bitni aylantirish | `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `7162015b4dac2555` | `5d26de5a` | Wycheproof-dan olingan yaroqsiz teg |
| CCM | 18 | Kesilgan teg (3 bayt) | `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `7162015b4dac2555` | `5d26de` | Qisqa teglar autentifikatsiya qilinmasligini ta'minlaydi |
| CCM | 19 | Shifrlangan matn bitini aylantirish | `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `7162015b4dac2554` | `5d26de5a` | O'zgartirilgan foydali yukni aniqlash |

## SM2 Deterministik imzo ma'lumotnomasi

| Maydon | Qiymat (agar qayd qilinmasa, olti burchakli) | Manba |
|-------|--------------------------|--------|
| Egri parametrlar | `sm2p256v1` (a = `FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFC` va boshqalar) | GM/T 0003.5-2012 A ilova |
| Foydalanuvchi ID (`distid`) | ASCII `"ALICE123@YAHOO.COM"` (ENTLA 0x0090) | GM/T 0003 D ilova |
| Ochiq kalit | `040AE4C7798AA0F119471BEE11825BE46202BB79E2A5844495E97C04FF4DF2548A7C0240F88F1CD4E16352A73C17B7F16F07353E53A176D684A9FE0C6BB798E857` | GM/T 0003 D ilova |
| Xabar | `"message digest"` (hex `6d65737361676520646967657374`) | GM/T 0003 D ilova |
| ZA | `F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A` | GM/T 0003 D ilova |
| `e = SM3(ZA || M)` | `B524F552CD82B8B028476E005C377FB19A87E6FC682D48BB5D42E3D9B9EFFE76` | GM/T 0003 D ilova |
| Imzo `(r,s)` | `40F1EC59F793D9F49E09DCEF49130D4194F79FB1EED2CAA55BACDB49C4E755D1`, `6FC6DAC32C5D5CF10C77DFB20F7C2EB667A457872FB09EC56327A67EC7DEEBE7` | GM/T 0003 D ilova |
| Multicodec (vaqtinchalik) | `8626550012414C494345313233405941484F4F2E434F4D040AE4…` (`sm2-pub`, varint `0x1306`) | Ilova 1-misoldan olingan |
| Prefiksli multihash | `sm2:8626550012414C494345313233405941484F4F2E434F4D040AE4…` | Olingan (`sm_known_answers.toml` mos keladi) |

SM2 multihash foydali yuklari `distid_len (u16 BE) || distid bytes || SEC1 uncompressed key (65 bytes)` sifatida kodlangan.

### Rust SDK deterministik imzolash moslamasi (SM-3c)

Strukturaviy vektorlar massivi Rust/Python/JavaScript paritet yukini o'z ichiga oladi
shuning uchun har bir mijoz bir xil SM2 xabarini umumiy urug' bilan imzolaydi va
farqlovchi identifikator.| Maydon | Qiymat (agar qayd qilinmasa, olti burchakli) | Eslatmalar |
|-------|--------------------------|-------|
| Farqlovchi ID | `"iroha-sdk-sm2-fixture"` | Rust/Python/JS SDK | boʻylab baham koʻrilgan
| Urug' | `"iroha-rust-sdk-sm2-deterministic-fixture"` (heks `69726F68612D727573742D73646B2D736D322D64657465726D696E69737469632D66697874757265`) | `Sm2PrivateKey::from_seed` | ga kiritish
| Shaxsiy kalit | `E64AE95930A2324DCF11BB1657FBBC2CD2BEF7515664BE7A15CA6AE5BCE0B7CA` | Urug'dan deterministik tarzda olingan |
| Ochiq kalit (SEC1 siqilmagan) | `0494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` | Deterministik hosilaga mos keladi |
| Ochiq kalit multihash | `862658001569726F68612D73646B2D736D322D666978747572650494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` | `PublicKey::to_string()` | chiqishi
| Prefiksli multihash | `sm2:862658001569726F68612D73646B2D736D322D666978747572650494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` | `PublicKey::to_prefixed_string()` | chiqishi
| ZA | `6B0229C3FBFE7A5DC6EE27228E18E512752DCCE9191EB381242A7342D568EDDE` | `Sm2PublicKey::compute_z` | orqali hisoblangan
| Xabar | `"Rust SDK SM2 signing fixture v1"` (heks `527573742053444B20534D32207369676E696E672066697874757265207631`) | SDK paritet testlari uchun kanonik foydali yuk |
| Imzo `(r, s)` | `4E026F2E0FB75A6418C95C046D70DB7B95558BE30FB2001906EBE2091FA1AF76`, `299CFF374026D9E0C49C6BD4A99A1473BA17EFF429B3FAD083B1577F7559BDF5` | Deterministik imzolash orqali ishlab chiqarilgan |

- o'zaro SDK iste'moli:
  - `fixtures/sm/sm2_fixture.json` endi `vectors` massivini ochadi. Rust kripto regressiya to'plami (`crates/iroha_crypto/tests/sm2_fixture_vectors.rs`), Rust mijoz yordamchilari (`crates/iroha/src/sm.rs`), Python ulanishlari (`python/iroha_python/tests/test_crypto.py`) va JavaScript SDK (`javascript/iroha_js/test/crypto.sm2.fixture.test.js`) bu moslamalarni tahlil qiladi.
  - `crates/iroha/tests/sm_signing.rs` deterministik imzolashni mashq qiladi va zanjirdagi multihash/multicodec chiqishlari moslamaga mos kelishini tekshiradi.
  - Qabul vaqti regressiya to'plamlari (`crates/iroha_core/tests/admission_batching.rs`), agar `allowed_signing` `sm2` *va* `default_hash` konfiguratsiyani yakunlash uchun `sm3-256` bo'lmasa, SM2 foydali yuklari rad etilishini tasdiqlaydi.
- línkīkīkīkī: līngčičičičičičičičičičiči`r/s`, `distid`、 `crates/iroha_crypto/tests/sm2_fuzz.rs` mulk mulki 1-ilova misoli `"message digest"` multikodek
- Zang kodi endi `Sm2PublicKey::compute_z` ni ochib beradi, shuning uchun ZA moslamalarini dasturiy tarzda yaratish mumkin; Ilova D regressiyasi uchun `sm2_compute_z_matches_annex_example` ga qarang.

## Keyingi harakatlar
- Konfiguratsiya shlyuzining SM2 faollashtirish chegaralarini ta'minlashda davom etishini ta'minlash uchun qabul qilish vaqti regressiyalarini (`admission_batching.rs`) kuzatib boring.
- Wycheproof SM4 GCM/CCM holatlari bilan qamrovni kengaytiring va SM2 imzosini tekshirish uchun mulkka asoslangan noaniq maqsadlarni oling. ✅ (`sm3_sm4_vectors.rs` da yozib olingan noto'g'ri holatlar to'plami).
- Muqobil identifikatorlar uchun LLM soʻrovi: *"Agar farqlovchi ID ALICE123@YAHOO.COM oʻrniga 1234567812345678 qilib oʻrnatilgan boʻlsa, 1-ilovaga misol uchun SM2 imzosini taqdim eting va yangi ZA/e qiymatlarini belgilang."*
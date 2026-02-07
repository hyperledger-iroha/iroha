---
lang: mn
direction: ltr
source: docs/source/crypto/sm_vectors.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fd46471945188bcb95c8ee411c48acc8915a92b408df196caa65bf25f0596732
source_last_modified: "2026-01-05T18:22:23.402400+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! SM2/SM3/SM4 интеграцийн ажлын лавлагаа тест векторууд.

# SM Векторуудын Тайлбарын тэмдэглэл

Энэхүү баримт бичиг нь автоматжуулсан импортын скриптүүд буухаас өмнө SM2/SM3/SM4 бэхэлгээг суулгадаг олон нийтэд нээлттэй хариулттай тестүүдийг нэгтгэсэн болно. Машинаар уншигдах хуулбарууд нь:

- `crates/iroha_crypto/tests/fixtures/sm_known_answers.toml` (хавсралт векторууд, RFC 8998 тохиолдол, Хавсралт 1-р жишээ).
- `fixtures/sm/sm2_fixture.json` (Rust/Python/JavaScript-н туршилтаар ашигладаг нийтлэг SDK бэхэлгээ).
- `crates/iroha_crypto/tests/fixtures/wycheproof_sm2.json` - дээд талын SM2 багц хүлээгдэж байх үед Apache-2.0-ийн дагуу тусгагдсан 52 тохиолдолтой корпус (тодорхойлолттой бэхэлгээ + нэгтгэсэн бит эргүүлэх/мессеж/сүүлийг таслах сөрөг тал). `crates/iroha_crypto/tests/sm2_wycheproof.rs` нь боломжтой бол стандарт SM2 баталгаажуулагч ашиглан эдгээр векторуудыг шалгаж, шаардлагатай үед Хавсралтын домэйны цэвэр BigInt хэрэгжилт рүү буцдаг.

## OpenSSL / Tongsuo / GmSSL эсрэг SM2 гарын үсгийн баталгаажуулалт

Хавсралтын                                                                                                                                                                                             yor yor OpenSSL/Tongsuo хуулж буулгах ажлын урсгал нь:

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

* OpenSSL 3.x нь `distid:` / `hexdistid:` сонголтуудыг баримтжуулдаг. Зарим OpenSSL 1.1.1 бүтээцүүд хаалганы бариулыг `sm2_id:` хэлбэрээр гаргадаг—`openssl pkeyutl -help` дээр аль нь гарч ирэхийг ашиглана.
* GmSSL ижил `pkeyutl` гадаргууг экспортлодог; хуучин загварууд нь мөн `-pkeyopt sm2_id:...`-г хүлээн зөвшөөрдөг.
* LibreSSL (macOS/OpenBSD дээр анхдагч) нь SM2/SM3-г **хэрэглэдэггүй** тул дээрх тушаал амжилтгүй болно. OpenSSL ≥ 1.1.1, Tongsuo эсвэл GmSSL ашиглана уу.

DER туслагч нь хавсралтын гарын үсэгтэй таарч байгаа `3044022040F1EC59F793D9F49E09DCEF49130D4194F79FB1EED2CAA55BACDB49C4E755D102206FC6DAC32C5D5CF10C77DFB20F7C2EB667A457872FB09EC56327A67EC7DEEBE7` ялгаруулдаг.

Хавсралт нь мөн хэрэглэгчийн мэдээллийн хэш болон үр дүнгийн хураангуйг хэвлэдэг:

```
ZA = F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A
e  = SM3(ZA || "message digest")
   = B524F552CD82B8B028476E005C377FB19A87E6FC682D48BB5D42E3D9B9EFFE76
```

Та OpenSSL-ээр баталгаажуулж болно:

```bash
echo -n 'F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A' \
  | xxd -r -p > za.bin
printf "message digest" >> za.bin
openssl dgst -sm3 -binary za.bin | xxd -p -c 256
# -> b524f552cd82b8b028476e005c377fb19a87e6fc682d48bb5d42e3d9b9effe76
```

Python-ийн муруйн тэгшитгэлийн эрүүл мэндийг шалгах:

```python
p = int("FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFF", 16)
a = int("FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFC", 16)
b = int("28E9FA9E9D9F5E344D5A9E4BCF6509A7F39789F515AB8F92DDBCBD414D940E93", 16)
x = int("0AE4C7798AA0F119471BEE11825BE46202BB79E2A5844495E97C04FF4DF2548A", 16)
y = int("7C0240F88F1CD4E16352A73C17B7F16F07353E53A176D684A9FE0C6BB798E857", 16)
assert (pow(y, 2, p) - (pow(x, 3, p) + a * x + b)) % p == 0
```

## SM3 хэш векторууд

| Оруулах | Hex кодчилол | Диджест (hex) | Эх сурвалж |
|-------|--------------|--------------|--------|
| `""` (хоосон мөр) | `""` | `1ab21d8355cfa17f8e61194831e81a8f22bec8c728fefb747ed035eb5082aa2b` | GM/T 0004-2012 Хавсралт А.1 |
| `"abc"` | `616263` | `66c7f0f462eeedd9d1f2d46bdc10e4e24167c4875cf2f7a2297da02b8f4ba8e0` | GM/T 0004-2012 Хавсралт А.2 |
| `"abcd"` 16 удаа давтагдсан (64 байт) | `61626364` ×16 | `debe9ff92275b8a138604889c18e5a4d6fdb70e5387e5765293dcba39c0c5732` | GB/T 32905-2016 Хавсралт А |

## SM4 блок шифр (ECB) векторууд

| Түлхүүр (hex) | Энгийн текст (hex) | Шифр текст (hex) | Эх сурвалж |
|----------|-----------------|------------------|--------|
| `0123456789abcdeffedcba9876543210` | `0123456789abcdeffedcba9876543210` | `681edf34d206965e86b3e94f536e4246` | GM/T 0002-2012 Хавсралт А.1 |
| `0123456789abcdeffedcba9876543210` | `000102030405060708090a0b0c0d0e0f` | `59b50808d3dcf921fa30b5b3c1dddc19` | GM/T 0002-2012 Хавсралт А.2 |
| `0123456789abcdeffedcba9876543210` | `ffeeddccbbaa99887766554433221100` | `1c3b3f56186b70819d3f5aa11fe2c8b6` | GM/T 0002-2012 Хавсралт А.3 |

## SM4-GCM баталгаажуулсан шифрлэлт

| Түлхүүр | IV | AAD | Энгийн текст | Шифр текст | Tag | Эх сурвалж |
|-----|----|-----|-----------|------------|-----|--------|
| `0123456789abcdeffedcba9876543210` | `00001234567800000000abcd` | `feedfacedeadbeeffeedfacedeadbeefabaddad2` | `d9313225f88406e5a55909c5aff5269a` | `42831ec2217774244b7221b784d0d49c` | `4d5c2af327cd64a62cf35abd2ba6fab4` | RFC 8998 Хавсралт А.2 |

## SM4-CCM баталгаажуулсан шифрлэлт| Түлхүүр | Үгүй | AAD | Энгийн текст | Шифр текст | Tag | Эх сурвалж |
|-----|-------|-----|-----------|------------|-----|--------|
| `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `202122232425262728292a2b2c2d2e2f` | `7162015b4dac2555` | `4d26de5a` | RFC 8998 Хавсралт А.3 |

### Wycheproof сөрөг тохиолдол (SM4 GCM/CCM)

Эдгээр тохиолдлууд `crates/iroha_crypto/tests/sm3_sm4_vectors.rs` дахь регрессийн багцыг мэдээлдэг. Тохиолдол бүрийг баталгаажуулж чадаагүй байх ёстой.

| Горим | TC ID | Тодорхойлолт | Түлхүүр | Үгүй | AAD | Шифр текст | Tag | Тэмдэглэл |
|------|-------|-------------|-----|-------|-----|------------|-----|-------|
| GCM | 1 | Tag bit flip | `0123456789abcdeffedcba9876543210` | `00001234567800000000abcd` | `feedfacedeadbeeffeedfacedeadbeefabaddad2` | `42831ec2217774244b7221b784d0d49c` | `5d5c2af327cd64a62cf35abd2ba6fab4` | Wycheproof-аас гаралтай хүчингүй шошго |
| ҮЗЗ | 17 | Tag bit flip | `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `7162015b4dac2555` | `5d26de5a` | Wycheproof-аас гаралтай хүчингүй шошго |
| ҮЗЗ | 18 | Таслагдсан шошго (3 байт) | `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `7162015b4dac2555` | `5d26de` | Богино шошгоны баталгаажуулалт амжилтгүй болохыг баталгаажуулдаг |
| ҮЗЗ | 19 | Шифр текстийн бит эргүүлэх | `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `7162015b4dac2554` | `5d26de5a` | Хувиргасан ачааг илрүүлэх |

## SM2 Детерминист гарын үсгийн лавлагаа

| Талбай | Утга (тэмдэглэлгүй бол зургаан өнцөгт) | Эх сурвалж |
|-------|--------------------------|--------|
| Муруй параметрүүд | `sm2p256v1` (a = `FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFC` гэх мэт) | GM/T 0003.5-2012 Хавсралт А |
| Хэрэглэгчийн ID (`distid`) | ASCII `"ALICE123@YAHOO.COM"` (ENTLA 0x0090) | GM/T 0003 Хавсралт D |
| Нийтийн түлхүүр | `040AE4C7798AA0F119471BEE11825BE46202BB79E2A5844495E97C04FF4DF2548A7C0240F88F1CD4E16352A73C17B7F16F07353E53A176D684A9FE0C6BB798E857` | GM/T 0003 Хавсралт D |
| Зурвас | `"message digest"` (hex `6d65737361676520646967657374`) | GM/T 0003 Хавсралт D |
| ZA | `F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A` | GM/T 0003 Хавсралт D |
| `e = SM3(ZA || M)` | `B524F552CD82B8B028476E005C377FB19A87E6FC682D48BB5D42E3D9B9EFFE76` | GM/T 0003 Хавсралт D |
| Гарын үсэг `(r,s)` | `40F1EC59F793D9F49E09DCEF49130D4194F79FB1EED2CAA55BACDB49C4E755D1`, `6FC6DAC32C5D5CF10C77DFB20F7C2EB667A457872FB09EC56327A67EC7DEEBE7` | GM/T 0003 Хавсралт D |
| Multicodec (түр зуурын) | `8626550012414C494345313233405941484F4F2E434F4D040AE4…` (`sm2-pub`, `0x1306` хувилбар) | Хавсралтын жишээ 1-ээс үүсэлтэй |
| Угтвар multihash | `sm2:8626550012414C494345313233405941484F4F2E434F4D040AE4…` | Гарсан (`sm_known_answers.toml` таарч байна) |

SM2 multihash ачааллыг `distid_len (u16 BE) || distid bytes || SEC1 uncompressed key (65 bytes)` гэж кодлодог.

### Rust SDK тодорхойлогч гарын үсэг (SM-3c)

Бүтэцлэгдсэн векторуудын массив нь Rust/Python/JavaScript паритын ачааллыг агуулдаг
Тиймээс үйлчлүүлэгч бүр ижил SM2 мессежийг хуваалцсан үрээр гарын үсэг зурдаг
ялгах танигч.| Талбай | Утга (тэмдэглэлгүй бол зургаан өнцөгт) | Тэмдэглэл |
|-------|--------------------------|-------|
| Онцлох ID | `"iroha-sdk-sm2-fixture"` | Rust/Python/JS SDK дээр хуваалцсан |
| Үр | `"iroha-rust-sdk-sm2-deterministic-fixture"` (hex `69726F68612D727573742D73646B2D736D322D64657465726D696E69737469632D66697874757265`) | `Sm2PrivateKey::from_seed` | руу оруулах
| Хувийн түлхүүр | `E64AE95930A2324DCF11BB1657FBBC2CD2BEF7515664BE7A15CA6AE5BCE0B7CA` | Үрээс тодорхойлон гаргаж авсан |
| Нийтийн түлхүүр (SEC1 шахагдаагүй) | `0494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` | Детерминист деривацтай таарч |
| Нийтийн түлхүүр multihash | `862658001569726F68612D73646B2D736D322D666978747572650494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` | `PublicKey::to_string()` | гаралт
| Угтвартай multihash | `sm2:862658001569726F68612D73646B2D736D322D666978747572650494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` | `PublicKey::to_prefixed_string()` | гаралт
| ЗА | `6B0229C3FBFE7A5DC6EE27228E18E512752DCCE9191EB381242A7342D568EDDE` | `Sm2PublicKey::compute_z` | -ээр тооцоолсон
| Зурвас | `"Rust SDK SM2 signing fixture v1"` (hex `527573742053444B20534D32207369676E696E672066697874757265207631`) | SDK паритет тестийн каноник ачаалал |
| Гарын үсэг `(r, s)` | `4E026F2E0FB75A6418C95C046D70DB7B95558BE30FB2001906EBE2091FA1AF76`, `299CFF374026D9E0C49C6BD4A99A1473BA17EFF429B3FAD083B1577F7559BDF5` | Детерминист тэмдэгтээр үүсгэгдсэн |

- SDK хоорондын хэрэглээ:
  - `fixtures/sm/sm2_fixture.json` одоо `vectors` массивыг харуулж байна. Rust крипто регрессийн багц (`crates/iroha_crypto/tests/sm2_fixture_vectors.rs`), Rust клиентийн туслахууд (`crates/iroha/src/sm.rs`), Python холболтууд (`python/iroha_python/tests/test_crypto.py`), JavaScript SDK (`javascript/iroha_js/test/crypto.sm2.fixture.test.js`) бүгд эдгээр бэхэлгээг задлан шинжилдэг.
  - `crates/iroha/tests/sm_signing.rs` нь тодорхойлогч гарын үсэг зурах дасгал хийж, гинжин хэлхээний multihash/multicodec гаралт нь бэхэлгээтэй таарч байгаа эсэхийг шалгадаг.
  - Элсэлтийн хугацааны регрессийн багцууд (`crates/iroha_core/tests/admission_batching.rs`) `allowed_signing` нь `sm2` *болон* `default_hash` нь `sm3-256` тохиргооны төгсгөлд ороогүй тохиолдолд SM2 ачааллыг үгүйсгэдэг.
- 追加カバレッジ: 異常系（無効な曲線、異常な `r/s`、`distid` 改ん`crates/iroha_crypto/tests/sm2_fuzz.rs` の үл хөдлөх хөрөнгө テストで網羅済みです。Хавсралт 1-р жишээ の正規ベクトルは `"message digest"` multicodec 形式で引き続き提供しています。
- Зэв код нь одоо `Sm2PublicKey::compute_z`-г ил гаргаж байгаа тул ZA бэхэлгээг программчлан үүсгэх боломжтой; Хавсралт D регрессийг `sm2_compute_z_matches_annex_example`-ээс үзнэ үү.

## Дараагийн үйлдлүүд
- Элсэлтийн хугацааны регрессийг (`admission_batching.rs`) хянаж, тохиргооны хаалт нь SM2 идэвхжүүлэх хил хязгаарыг үргэлжлүүлэн мөрдүүлэх болно.
- Wycheproof SM4 GCM/CCM кейсүүдийн хамрах хүрээг өргөтгөж, SM2 гарын үсгийн баталгаажуулалтад зориулсан өмчид суурилсан бүдүүлэг зорилтуудыг гаргана. ✅ (`sm3_sm4_vectors.rs`-д авсан хүчингүй жижиг дэд багц).
- LLM-ийн өөр ID-н тухай сануулга: *"Ялгах ID-г ALICE123@YAHOO.COM-ын оронд 1234567812345678 гэж тохируулсан бол Хавсралтын жишээ 1-д SM2 гарын үсгийг оруулж, шинэ ZA/e утгуудыг тоймло."*
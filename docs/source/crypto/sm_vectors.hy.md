---
lang: hy
direction: ltr
source: docs/source/crypto/sm_vectors.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fd46471945188bcb95c8ee411c48acc8915a92b408df196caa65bf25f0596732
source_last_modified: "2026-01-05T18:22:23.402400+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! SM2/SM3/SM4 ինտեգրացիոն աշխատանքի համար տեղեկատու թեստային վեկտորներ:

# SM Vectors Staging Notes

Այս փաստաթուղթը համախմբում է հանրությանը հասանելի հայտնի պատասխանների թեստերը, որոնք սերմանում են SM2/SM3/SM4 ամրագոտիները նախքան ներմուծման ավտոմատացված սկրիպտների վայրէջքը: Մեքենայով ընթեռնելի պատճեններն ապրում են՝

- `crates/iroha_crypto/tests/fixtures/sm_known_answers.toml` (հավելվածի վեկտորներ, RFC 8998 դեպքեր, Հավելվածի Օրինակ 1):
- `fixtures/sm/sm2_fixture.json` (ընդհանուր դետերմինիստական ​​SDK հարմարանք, որն օգտագործվում է Rust/Python/JavaScript թեստերով):
- `crates/iroha_crypto/tests/fixtures/wycheproof_sm2.json` — 52 գործի ընտրված կորպուս (դետերմինիստական ​​հարմարանքներ + սինթեզված բիթ-շրջադարձ/հաղորդագրություն/պոչի կտրման նեգատիվներ) արտացոլված Apache-2.0-ի տակ, մինչ SM2-ի վերին հոսանքն անխափան է: `crates/iroha_crypto/tests/sm2_wycheproof.rs`-ը ստուգում է այս վեկտորները՝ օգտագործելով ստանդարտ SM2 ստուգիչը, երբ դա հնարավոր է, և անհրաժեշտության դեպքում վերադառնում է հավելվածի տիրույթի մաքուր BigInt իրականացմանը:

## SM2 ստորագրության ստուգում OpenSSL / Tongsuo / GmSSL-ի դեմ

Հավելվածի Օրինակ 1 (Fp-256) օգտագործում է նույնականացումը `ALICE123@YAHOO.COM` (ENTLA 0x0090), `"message digest"` հաղորդագրությունը և ստորև ներկայացված հանրային բանալին: Պատճենեք և տեղադրեք OpenSSL/Tongsuo աշխատանքային հոսքը հետևյալն է.

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

* OpenSSL 3.x-ը փաստաթղթավորում է `distid:` / `hexdistid:` տարբերակները: Որոշ OpenSSL 1.1.1 շինություններ ցուցադրում են կոճակը որպես `sm2_id:`. օգտագործեք այն, ինչը հայտնվի `openssl pkeyutl -help`-ում:
* GmSSL-ն արտահանում է նույն `pkeyutl` մակերեսը; Ավելի հին շինությունները նույնպես ընդունված են `-pkeyopt sm2_id:...`:
* LibreSSL-ը (կանխադրված macOS/OpenBSD-ում) **չի** իրականացնում SM2/SM3, ուստի վերը նշված հրամանը ձախողվում է այնտեղ: Օգտագործեք OpenSSL ≥ 1.1.1, Tongsuo կամ GmSSL:

DER օգնականը թողարկում է `3044022040F1EC59F793D9F49E09DCEF49130D4194F79FB1EED2CAA55BACDB49C4E755D102206FC6DAC32C5D5CF10C77DFB20F7C2EB667A457872FB09EC56327A67EC7DEEBE7`, որը համապատասխանում է հավելվածի ստորագրությանը:

Հավելվածը տպում է նաև օգտագործողի տեղեկատվության հաշը և արդյունքում ստացված ամփոփագիրը.

```
ZA = F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A
e  = SM3(ZA || "message digest")
   = B524F552CD82B8B028476E005C377FB19A87E6FC682D48BB5D42E3D9B9EFFE76
```

Դուք կարող եք հաստատել OpenSSL-ով.

```bash
echo -n 'F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A' \
  | xxd -r -p > za.bin
printf "message digest" >> za.bin
openssl dgst -sm3 -binary za.bin | xxd -p -c 256
# -> b524f552cd82b8b028476e005c377fb19a87e6fc682d48bb5d42e3d9b9effe76
```

Python-ի ողջախոհության ստուգում կորի հավասարման համար.

```python
p = int("FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFF", 16)
a = int("FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFC", 16)
b = int("28E9FA9E9D9F5E344D5A9E4BCF6509A7F39789F515AB8F92DDBCBD414D940E93", 16)
x = int("0AE4C7798AA0F119471BEE11825BE46202BB79E2A5844495E97C04FF4DF2548A", 16)
y = int("7C0240F88F1CD4E16352A73C17B7F16F07353E53A176D684A9FE0C6BB798E857", 16)
assert (pow(y, 2, p) - (pow(x, 3, p) + a * x + b)) % p == 0
```

## SM3 Հեշ վեկտորներ

| Մուտքագրում | Hex կոդավորում | մարսել (վեցանկյուն) | Աղբյուր |
|-------|--------------|-------------|--------|
| `""` (դատարկ տող) | `""` | `1ab21d8355cfa17f8e61194831e81a8f22bec8c728fefb747ed035eb5082aa2b` | ԳՄ/Տ 0004-2012 Հավելված Ա.1 |
| `"abc"` | `616263` | `66c7f0f462eeedd9d1f2d46bdc10e4e24167c4875cf2f7a2297da02b8f4ba8e0` | ԳՄ/Տ 0004-2012 Հավելված Ա.2 |
| `"abcd"` կրկնվել է 16 անգամ (64 բայթ) | `61626364` ×16 | `debe9ff92275b8a138604889c18e5a4d6fdb70e5387e5765293dcba39c0c5732` | ԳԲ/Տ 32905-2016 Հավելված Ա |

## SM4 բլոկ ծածկագրի (ECB) վեկտորներ

| Բանալի (վեցանկյուն) | Պարզ տեքստ (վեցանկյուն) | Գաղտնագրված տեքստ (վեցանկյուն) | Աղբյուր |
|-----------|-------------------------------------|--------|
| `0123456789abcdeffedcba9876543210` | `0123456789abcdeffedcba9876543210` | `681edf34d206965e86b3e94f536e4246` | ԳՄ/Տ 0002-2012 Հավելված Ա.1 |
| `0123456789abcdeffedcba9876543210` | `000102030405060708090a0b0c0d0e0f` | `59b50808d3dcf921fa30b5b3c1dddc19` | ԳՄ/Տ 0002-2012 Հավելված Ա.2 |
| `0123456789abcdeffedcba9876543210` | `ffeeddccbbaa99887766554433221100` | `1c3b3f56186b70819d3f5aa11fe2c8b6` | ԳՄ/Տ 0002-2012 Հավելված Ա.3 |

## SM4-GCM վավերացված գաղտնագրում

| Բանալի | IV | AAD | Պարզ տեքստ | Գաղտնագրված տեքստ | Պիտակ | Աղբյուր |
|-----|----|-----|-----------|-----------|-----|--------|
| `0123456789abcdeffedcba9876543210` | `00001234567800000000abcd` | `feedfacedeadbeeffeedfacedeadbeefabaddad2` | `d9313225f88406e5a55909c5aff5269a` | `42831ec2217774244b7221b784d0d49c` | `4d5c2af327cd64a62cf35abd2ba6fab4` | RFC 8998 Հավելված Ա.2 |

## SM4-CCM վավերացված գաղտնագրում| Բանալի | Nonce | AAD | Պարզ տեքստ | Գաղտնագրված տեքստ | Պիտակ | Աղբյուր |
|-----|-------|-----|-----------|-----------|-----|--------|
| `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `202122232425262728292a2b2c2d2e2f` | `7162015b4dac2555` | `4d26de5a` | RFC 8998 Հավելված Ա.3 |

### Wycheproof բացասական պատյաններ (SM4 GCM/CCM)

Այս դեպքերը տեղեկացնում են `crates/iroha_crypto/tests/sm3_sm4_vectors.rs`-ի ռեգրեսիոն փաթեթին: Յուրաքանչյուր դեպք պետք է չստուգվի:

| Ռեժիմ | TC ID | Նկարագրություն | Բանալի | Nonce | AAD | Գաղտնագրված տեքստ | Պիտակ | Ծանոթագրություններ |
|------|-------|------------|-----|-------|----|-----------|-----|------|
| GCM | 1 | Tag bit flip | `0123456789abcdeffedcba9876543210` | `00001234567800000000abcd` | `feedfacedeadbeeffeedfacedeadbeefabaddad2` | `42831ec2217774244b7221b784d0d49c` | `5d5c2af327cd64a62cf35abd2ba6fab4` | Wycheproof-ից ստացված անվավեր պիտակ |
| CCM | 17 | Tag bit flip | `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `7162015b4dac2555` | `5d26de5a` | Wycheproof-ից ստացված անվավեր պիտակ |
| CCM | 18 | Կտրված պիտակ (3 բայթ) | `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `7162015b4dac2555` | `5d26de` | Ապահովում է կարճ պիտակների անհաջող վավերացում |
| CCM | 19 | Գաղտնագրված տեքստի բիթ շեղում | `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `7162015b4dac2554` | `5d26de5a` | Հայտնաբերել կեղծված օգտակար բեռը |

## SM2 դետերմինիստական ստորագրության հղում

| Դաշտային | Արժեք (վեցանկյուն, եթե նշված չէ) | Աղբյուր |
|-------|-------------------------|--------|
| Կորի պարամետրերը | `sm2p256v1` (a = `FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFC` և այլն) | ԳՄ/Տ 0003.5-2012 Հավելված Ա |
| Օգտագործողի ID (`distid`) | ASCII `"ALICE123@YAHOO.COM"` (ENTLA 0x0090) | GM/T 0003 Հավելված D |
| Հանրային բանալի | `040AE4C7798AA0F119471BEE11825BE46202BB79E2A5844495E97C04FF4DF2548A7C0240F88F1CD4E16352A73C17B7F16F07353E53A176D684A9FE0C6BB798E857` | GM/T 0003 Հավելված D |
| Հաղորդագրություն | `"message digest"` (վեցանկյուն `6d65737361676520646967657374`) | GM/T 0003 Հավելված D |
| ԶԱ | `F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A` | GM/T 0003 Հավելված D |
| `e = SM3(ZA || M)` | `B524F552CD82B8B028476E005C377FB19A87E6FC682D48BB5D42E3D9B9EFFE76` | GM/T 0003 Հավելված D |
| Ստորագրություն `(r,s)` | `40F1EC59F793D9F49E09DCEF49130D4194F79FB1EED2CAA55BACDB49C4E755D1`, `6FC6DAC32C5D5CF10C77DFB20F7C2EB667A457872FB09EC56327A67EC7DEEBE7` | GM/T 0003 Հավելված D |
| Multicodec (ժամանակավոր) | `8626550012414C494345313233405941484F4F2E434F4D040AE4…` (`sm2-pub`, տարբերակ `0x1306`) | Բխում է Հավելվածի Օրինակ 1 |
| Նախածանցով բազմահեշ | `sm2:8626550012414C494345313233405941484F4F2E434F4D040AE4…` | Ստացված (համապատասխանում է `sm_known_answers.toml`) |

SM2 մուլտիհեշ օգտակար բեռները կոդավորված են որպես `distid_len (u16 BE) || distid bytes || SEC1 uncompressed key (65 bytes)`:

### Rust SDK Deterministic Signing Faxture (SM-3c)

Կառուցված վեկտորների զանգվածը ներառում է Rust/Python/JavaScript պարիտետային բեռը
այնպես որ յուրաքանչյուր հաճախորդ ստորագրում է նույն SM2 հաղորդագրությունը ընդհանուր սերմով և
տարբերակիչ նույնացուցիչ.| Դաշտային | Արժեք (վեցանկյուն, եթե նշված չէ) | Ծանոթագրություններ |
|-------|-------------------------|-------|
| Տարբերակիչ ID | `"iroha-sdk-sm2-fixture"` | Համօգտագործվում է Rust/Python/JS SDK-ներով |
| Սերմ | `"iroha-rust-sdk-sm2-deterministic-fixture"` (վեցանկյուն `69726F68612D727573742D73646B2D736D322D64657465726D696E69737469632D66697874757265`) | Մուտք `Sm2PrivateKey::from_seed` |
| Անձնական բանալի | `E64AE95930A2324DCF11BB1657FBBC2CD2BEF7515664BE7A15CA6AE5BCE0B7CA` | Դետերմինիստորեն ստացված սերմից |
| Հանրային բանալի (SEC1 չսեղմված) | `0494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` | Համապատասխանում է դետերմինիստական ​​ածանցյալ |
| Հանրային բանալին բազմահեշ | `862658001569726F68612D73646B2D736D322D666978747572650494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` | Ելք `PublicKey::to_string()` |
| Նախածանցով բազմահեշ | `sm2:862658001569726F68612D73646B2D736D322D666978747572650494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` | Ելք `PublicKey::to_prefixed_string()` |
| ԶԱ | `6B0229C3FBFE7A5DC6EE27228E18E512752DCCE9191EB381242A7342D568EDDE` | Հաշվարկված է `Sm2PublicKey::compute_z`-ի միջոցով |
| Հաղորդագրություն | `"Rust SDK SM2 signing fixture v1"` (վեցանկյուն `527573742053444B20534D32207369676E696E672066697874757265207631`) | Կանոնական ծանրաբեռնվածություն SDK հավասարության թեստերի համար |
| Ստորագրություն `(r, s)` | `4E026F2E0FB75A6418C95C046D70DB7B95558BE30FB2001906EBE2091FA1AF76`, `299CFF374026D9E0C49C6BD4A99A1473BA17EFF429B3FAD083B1577F7559BDF5` | Արտադրված է դետերմինիստական ​​ստորագրման միջոցով |

- Cross-SDK սպառումը.
  - `fixtures/sm/sm2_fixture.json`-ն այժմ բացահայտում է `vectors` զանգվածը: Rust crypto regression փաթեթը (`crates/iroha_crypto/tests/sm2_fixture_vectors.rs`), Rust հաճախորդների օգնականները (`crates/iroha/src/sm.rs`), Python կապերը (`python/iroha_python/tests/test_crypto.py`) և JavaScript SDK (`javascript/iroha_js/test/crypto.sm2.fixture.test.js`) բոլորը վերլուծում են այս հարմարանքները:
  - `crates/iroha/tests/sm_signing.rs`-ն իրականացնում է դետերմինիստական ​​ստորագրություն և ստուգում է, որ շղթայական մուլտիհեշ/մուլտիկոդեկ ելքերը համապատասխանում են սարքին:
  - Ընդունելության ժամանակի ռեգրեսիոն փաթեթները (`crates/iroha_core/tests/admission_batching.rs`) պնդում են, որ SM2 օգտակար բեռները մերժվում են, եթե `allowed_signing`-ը ներառում է `sm2` *և* `default_hash`-ը `sm3-256` կազմաձևման վերջն է:
- 追加カバレッジ: 異常系（無効な曲線、異常な `r/s`、`distid`、`distid` `crates/iroha_crypto/tests/sm2_fuzz.rs` の սեփականություն テストで網羅済みです。Հավելված Օրինակ 1 の正規ベクトルは `"message digest"` մուլտիկոդեկ 形式で引き続き提供しています。
- Rust կոդը այժմ բացահայտում է `Sm2PublicKey::compute_z`, այնպես որ ZA հարմարանքները կարող են ստեղծվել ծրագրային եղանակով; տես `sm2_compute_z_matches_annex_example` Հավելված D ռեգրեսիայի համար:

## Հաջորդ գործողություններ
- Դիտարկեք ընդունելության ժամանակի հետընթացը (`admission_batching.rs`)՝ ապահովելու համար, որ կոնֆիգուրացիայի դարպասը շարունակում է կիրառել SM2-ի միացման սահմանները:
- Ընդլայնել ծածկույթը Wycheproof SM4 GCM/CCM պատյաններով և ստանալ սեփականության վրա հիմնված fuzz թիրախներ SM2 ստորագրության ստուգման համար: ✅ (`sm3_sm4_vectors.rs`-ում ընդգրկված է անվավեր գործի ենթաբազմություն):
- LLM-ի հուշում այլընտրանքային ID-ների համար. * «Տրամադրեք SM2 ստորագրությունը Հավելվածի Օրինակ 1-ի համար, երբ տարբերակիչ ID-ն ALICE123@YAHOO.COM-ի փոխարեն դրված է 1234567812345678, և ուրվագծեք նոր ZA/e արժեքները»:*
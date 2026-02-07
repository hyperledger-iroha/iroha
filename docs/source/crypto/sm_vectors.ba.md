---
lang: ba
direction: ltr
source: docs/source/crypto/sm_vectors.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fd46471945188bcb95c8ee411c48acc8915a92b408df196caa65bf25f0596732
source_last_modified: "2026-01-05T18:22:23.402400+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! SM2/SM3/SM4 интеграцияһы эше өсөн һылтанма тест векторҙары.

# SM Векторҙар стадиялау Иҫкәрмәләр

Был документ агрегаттары йәмәғәтселек өсөн билдәле билдәле-яуап һынауҙары, улар SM2/SM3/SM4 автоматлаштырылған импорт сценарийҙары ер алдынан орлоҡ. Машина менән уҡыла торған күсермәләр йәшәй:

- `crates/iroha_crypto/tests/fixtures/sm_known_answers.toml` (ҡушымта векторҙары, RFC 8998 осраҡтар, 1-се ҡушымта).
- `fixtures/sm/sm2_fixture.json` (Дөйөм детерминистик SDK ҡоролмаһы ҡулланылған Rust/Python/JavaScript һынауҙары).
- `crates/iroha_crypto/tests/fixtures/wycheproof_sm2.json` — курировать 52-осраҡлы корпус (детерминистик ҡорамалдар + синтезланған бит-флип/хәбәр/ҡойроҡ ҡыҫҡартыу негативтары) көҙгө аҫтында Apache-2.0, ә өҫкө ағым SM2 люкс көтә. `crates/iroha_crypto/tests/sm2_wycheproof.rs` был векторҙарҙы мөмкин булғанда стандарт SM2 тикшерергә ҡулланып раҫлай һәм кәрәк саҡта Ennex доменын саф BigInt-ты тормошҡа ашырыуға кире төшә.

## SM2 OpenSSL-ға ҡаршы ҡултамға тикшерелеүе / Тонгсуо / GmSSL

1-се миҫал (Fp-256) `ALICE123@YAHOO.COM` (NENTLA 0x0090), хәбәр `"message digest"`, һәм түбәндә күрһәтелгән асыҡ асҡыс ҡуллана. Күсермәһе һәм ‐ йәбештереү OpenSSL/Тонссуо эш ағымы:

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

* OpenSSL 3.x документы `distid:` / `hexdistid:` варианттары. 1.1.1 ҡайһы бер OpenSSL ручканы `sm2_id:` тип фашлай — `openssl pkeyutl -help`-та күренгәнсә, ҡулланыу.
* GmSSL шул уҡ `pkeyutl` өҫтөн экспортлай; өлкән биналар шулай уҡ `-pkeyopt sm2_id:...` ҡабул иткән.
* LibreSSL (макОС/ОпенБСД өҫтөндә ғәҙәттәгесә) эшләй ** түгел ** SM2/SM3 тормошҡа ашырыу, шуға күрә өҫтәге команда унда уңышһыҙлыҡҡа осрай. OpenSSL ≥ 1.1, Тонгсуо, йәки GmSSL ҡулланыу.

DER ярҙамсыһы `3044022040F1EC59F793D9F49E09DCEF49130D4194F79FB1EED2CAA55BACDB49C4E755D102206FC6DAC32C5D5CF10C77DFB20F7C2EB667A457872FB09EC56327A67EC7DEEBE7` XX, был ҡушымта ҡултамғаһына тап килә.

Ҡушымта шулай уҡ ҡулланыусы мәғлүмәт хешын һәм һөҙөмтәлә һыу алыуҙы баҫтырып сығара:

```
ZA = F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A
e  = SM3(ZA || "message digest")
   = B524F552CD82B8B028476E005C377FB19A87E6FC682D48BB5D42E3D9B9EFFE76
```

OpenSSL менән раҫлай алаһығыҙ:

```bash
echo -n 'F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A' \
  | xxd -r -p > za.bin
printf "message digest" >> za.bin
openssl dgst -sm3 -binary za.bin | xxd -p -c 256
# -> b524f552cd82b8b028476e005c377fb19a87e6fc682d48bb5d42e3d9b9effe76
```

Python sanity тикшерергә өсөн ҡойроҡ тигеҙләмәһе:

```python
p = int("FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFF", 16)
a = int("FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFC", 16)
b = int("28E9FA9E9D9F5E344D5A9E4BCF6509A7F39789F515AB8F92DDBCBD414D940E93", 16)
x = int("0AE4C7798AA0F119471BEE11825BE46202BB79E2A5844495E97C04FF4DF2548A", 16)
y = int("7C0240F88F1CD4E16352A73C17B7F16F07353E53A176D684A9FE0C6BB798E857", 16)
assert (pow(y, 2, p) - (pow(x, 3, p) + a * x + b)) % p == 0
```

## SM3 Хэш векторҙары

| Инпут | Hex кодлау | Дайджест (гекс) | Сығанаҡ |
|------|--------------|-------------|---------|
| `""` (буш ҡыл) | `""` | `1ab21d8355cfa17f8e61194831e81a8f22bec8c728fefb747ed035eb5082aa2b` | ГМ/Т 0004-2012 А.1 Ҡушымта |
| `"abc"` | `616263` | `66c7f0f462eeedd9d1f2d46bdc10e4e24167c4875cf2f7a2297da02b8f4ba8e0` | ГМ/Т 0004-2012 А.2 ҡушымтаһы |
| `"abcd"` 16 тапҡыр ҡабатланы (64 байт) | `61626364` ×16 | `debe9ff92275b8a138604889c18e5a4d6fdb70e5387e5765293dcba39c0c5732` | ГБ/Т 32905-2016 Ҡушымта А |

## SM4 блок шифр (ЕКБ) Векторҙар

| Асҡыс (гекс) | Плейтекст (гекс) | Цифер ​​тексы (гекс) | Сығанаҡ |
|----------|-----------------------------------|----------|
| `0123456789abcdeffedcba9876543210` | `0123456789abcdeffedcba9876543210` | `681edf34d206965e86b3e94f536e4246` | ГМ/Т 0002-2012 А.1 Ҡушымта |
| `0123456789abcdeffedcba9876543210` | `000102030405060708090a0b0c0d0e0f` | `59b50808d3dcf921fa30b5b3c1dddc19` | GM/Т 0002-2012 Ҡушымта А.2 |
| `0123456789abcdeffedcba9876543210` | `ffeeddccbbaa99887766554433221100` | `1c3b3f56186b70819d3f5aa11fe2c8b6` | ГМ/Т 0002-2012 А.3 ҡушымтаһы |

## SM4-GCM аутентификацияланған шифрлау

| Асҡыс | IV | ААД | Платекст | Цифер ​​тексы | Тег | Сығанаҡ |
|----|----|------|-------------------------|-----|--------- |
| `0123456789abcdeffedcba9876543210` | `00001234567800000000abcd` | `feedfacedeadbeeffeedfacedeadbeefabaddad2` | `d9313225f88406e5a55909c5aff5269a` | `42831ec2217774244b7221b784d0d49c` | `4d5c2af327cd64a62cf35abd2ba6fab4` | RFC 8998 Ҡушымта А.2 |

## SM4-CCM аутентификацияланған шифрлау| Асҡыс | Юҡ | ААД | Платекст | Цифер ​​тексы | Тег | Сығанаҡ |
|----|------|------|----------------------------|-----|----------||
| `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `202122232425262728292a2b2c2d2e2f` | `7162015b4dac2555` | `4d26de5a` | RFC 8998 Ҡушымта А.3 |

### Wycheproof негатив осраҡта (SM4 GCM/CCM)

Был осраҡтарҙа `crates/iroha_crypto/tests/sm3_sm4_vectors.rs`-тағы регрессия люксын хәбәр итә. Һәр осраҡ тикшерергә тейеш.

| Режим | TC ID | Тасуирлама | Асҡыс | Юҡ | ААД | Цифер ​​тексы | Тег | Иҫкәрмәләр |
|-----|------|---------------|------------------------------------|-|-----------|
| ГКМ | 1 | Билдә бит ҡабартып | `0123456789abcdeffedcba9876543210` | `00001234567800000000abcd` | `feedfacedeadbeeffeedfacedeadbeefabaddad2` | `42831ec2217774244b7221b784d0d49c` | `5d5c2af327cd64a62cf35abd2ba6fab4` | Уайхепроуд-алынған дөрөҫ булмаған тег |
| CCM | 17 | Билдә бит ҡабартып | `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `7162015b4dac2555` | `5d26de5a` | Уайхепроуд-алынған дөрөҫ булмаған тег |
| CCM | 18 | Трансляцияланған тег (3 байт) | `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `7162015b4dac2555` | `5d26de` | Ҡыҫҡа тегтар тәьмин итеү етешһеҙлектәре аутентификация |
| CCM | 19 | Цифер ​​тексы бит флип | `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `7162015b4dac2554` | `5d26de5a` | Асыҡлау подгруботельный файҙалы йөк |

## SM2 Детерминистик ҡултамға һылтанмаһы

| Ялан | Ҡиммәте (һәрләммәһә, гекс) | Сығанаҡ |
|------|-------------------------|-------- |
| Ҡырҡыу параметрҙары | `sm2p256v1` (а = `FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFC`, һ.б.) | GM/Т 0003.5-2012 Ҡушымта А |
| Ҡулланыусы идентификаторы (`distid`) | ASCII `"ALICE123@YAHOO.COM"` (ENTLA 0x0090) | GM/Т 0003 Ҡушымта D |
| Йәмәғәт асҡысы | `040AE4C7798AA0F119471BEE11825BE46202BB79E2A5844495E97C04FF4DF2548A7C0240F88F1CD4E16352A73C17B7F16F07353E53A176D684A9FE0C6BB798E857` | GM/Т 0003 Ҡушымта D |
| Хәбәр | `"message digest"` ( hex `6d65737361676520646967657374`) | GM/Т 0003 Ҡушымта D |
| ЗА | `F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A` | GM/Т 0003 Ҡушымта D |
| `e = SM3(ZA || M)` | `B524F552CD82B8B028476E005C377FB19A87E6FC682D48BB5D42E3D9B9EFFE76` | GM/Т 0003 Ҡушымта D |
| Ҡултамға `(r,s)` | `40F1EC59F793D9F49E09DCEF49130D4194F79FB1EED2CAA55BACDB49C4E755D1`, `6FC6DAC32C5D5CF10C77DFB20F7C2EB667A457872FB09EC56327A67EC7DEEBE7` | GM/Т 0003 Ҡушымта D |
| Мультикодек (ваҡытлыса) | `8626550012414C494345313233405941484F4F2E434F4D040AE4…` (`sm2-pub`, варианты `0x1306`) | 1-се ҡушымтанан алынған |
| Префиксированный мультихаш | `sm2:8626550012414C494345313233405941484F4F2E434F4D040AE4…` | Сығарылған (тапшалар `sm_known_answers.toml`) |

SM2 мультихаш файҙалы йөктәр `distid_len (u16 BE) || distid bytes || SEC1 uncompressed key (65 bytes)` тип кодлана.

### SDK детерминистик ҡул ҡуйыу фикстураһы (SM-3c)

Структуралы векторҙар массивы үҙ эсенә руст/Питон/JavaScript паритет файҙалы йөк .
шулай итеп, һәр клиент бер үк SM2 хәбәр менән дөйөм орлоҡ һәм .
идентификаторҙы айырыу.| Ялан | Ҡиммәте (һәрләммәһә, гекс) | Иҫкәрмәләр |
|------|-------------------------|-------|
| Билдәләү идентификаторы | `"iroha-sdk-sm2-fixture"` | Ҡырҡыу/Питон/JS SDKs аша уртаҡлашты |
| Орлоҡ | `"iroha-rust-sdk-sm2-deterministic-fixture"` ( hex `69726F68612D727573742D73646B2D736D322D64657465726D696E69737469632D66697874757265` X) | `Sm2PrivateKey::from_seed`-ҡа индереү |
| Шәхси асҡыс | `E64AE95930A2324DCF11BB1657FBBC2CD2BEF7515664BE7A15CA6AE5BCE0B7CA` | Орлоҡтан детерминистик алынған |
| Йәмәғәт асҡысы (SEC1 ҡыҫылмаған) | `0494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` | Тайстар детерминистик сығарылыш |
| Йәмәғәт асҡысы мультихаш | `862658001569726F68612D73646B2D736D322D666978747572650494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` | `PublicKey::to_string()` сығым |
| Префиксированный мультихаш | `sm2:862658001569726F68612D73646B2D736D322D666978747572650494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` | `PublicKey::to_prefixed_string()` сығарыу |
| ЗА | `6B0229C3FBFE7A5DC6EE27228E18E512752DCCE9191EB381242A7342D568EDDE` | `Sm2PublicKey::compute_z` аша иҫәпләнә |
| Хәбәр | `"Rust SDK SM2 signing fixture v1"` X (хекс ```bash
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
```) | SDK паритет һынауҙары өсөн канонлы файҙалы йөк |
| Ҡултамға `(r, s)` | `4E026F2E0FB75A6418C95C046D70DB7B95558BE30FB2001906EBE2091FA1AF76`, `299CFF374026D9E0C49C6BD4A99A1473BA17EFF429B3FAD083B1577F7559BDF5` | Детерминистик ҡул ҡуйыу аша етештерелә |

- Кросс-СДК ҡулланыу:
  - `fixtures/sm/sm2_fixture.json` хәҙер `vectors` массивын фашлай. Rust крипто регрессия люкс (`crates/iroha_crypto/tests/sm2_fixture_vectors.rs`), Rust клиент ярҙамсылары (`crates/iroha/src/sm.rs`), Python бәйләүҙәре (`python/iroha_python/tests/test_crypto.py`), һәм JavaScript SDK (`javascript/iroha_js/test/crypto.sm2.fixture.test.js`) бөтә анализдар был ҡоролма.
  - `crates/iroha/tests/sm_signing.rs` күнекмәләр детерминистик ҡул ҡуйыу һәм раҫлай, тип сылбырлы мультихаш/мультикодек сығыштары тура килә ҡоролма.
  - Ҡабул итеү ваҡытында регрессия люкстары (`crates/iroha_core/tests/admission_batching.rs`) раҫлау SM2 файҙалы йөкләмәләр кире ҡағыла, әгәр `allowed_signing` `sm2` * һәм* `pkeyutl` `sm3-256`, конфигурация сикләүҙәрен яҡтыртыу ос-осҡа.
- 追加カバレッジ: 異常系（無効な曲線、異常な `r/s`、`distid` 改ざん）は`""` の үҙенсәлеге テスでで羅済みです。Аң 1 Миҫал の正規ベルル `1ab21d8355cfa17f8e61194831e81a8f22bec8c728fefb747ed035eb5082aa2b`に多語対応の мультокодек 形式で引きき提供していま。
- Rust коды хәҙер `Sm2PublicKey::compute_z` фашлай, шуға күрә ZA ҡоролмаларын программалап генерацияларға мөмкин; ҡарағыҙ `sm2_compute_z_matches_annex_example` өсөн ҡушымта D регрессияһы.

## Киләһе ғәмәлдәр
- Монитор ҡабул итеү ваҡытында регрессиялар (`admission_batching.rs`), конфигация тапшырыуҙы тәьмин итеү өсөн SM2 өҫтәү сиктәрен үтәүен дауам итә.
- SM4 GCM/CCM осраҡтарында Wycheproof менән киңәйтеү һәм SM2 ҡултамғаһы тикшерелеүе өсөн мөлкәткә нигеҙләнгән fuzz маҡсаттарын алыу. ✅ (`sm3_sm4_vectors.rs`-та төшөрөлгән инвалид-осраҡлы подгруппа).
- LLM альтернатив идентификаторҙар өсөн өндәү: *"SM2 ҡултамғаһы өсөн 1-се ҡушымта миҫалында 1234567812345678 урынына айырылып торған идентификатор ҡуйылғанда ALICE123@YAHOO.COM урынына, һәм яңы ZA/e ҡиммәттәрен билдәләй."
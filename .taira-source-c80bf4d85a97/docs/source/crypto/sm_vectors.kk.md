---
lang: kk
direction: ltr
source: docs/source/crypto/sm_vectors.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fd46471945188bcb95c8ee411c48acc8915a92b408df196caa65bf25f0596732
source_last_modified: "2026-01-05T18:22:23.402400+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! SM2/SM3/SM4 интеграциялық жұмысына арналған анықтамалық сынақ векторлары.

# SM векторларының кезеңдік жазбалары

Бұл құжат SM2/SM3/SM4 сымдарын автоматтандырылған импорттық сценарийлер қонғанға дейін енгізетін жалпыға қолжетімді белгілі жауап сынақтарын біріктіреді. Машинада оқылатын көшірмелер мына жерде тұрады:

- `crates/iroha_crypto/tests/fixtures/sm_known_answers.toml` (қосымша векторлар, RFC 8998 жағдайлары, 1-қосымша мысал).
- `fixtures/sm/sm2_fixture.json` (Rust/Python/JavaScript сынақтары пайдаланатын ортақ детерминистік SDK құрылғысы).
- `crates/iroha_crypto/tests/fixtures/wycheproof_sm2.json` — жоғары ағын SM2 жинағы күтілуде, Apache-2.0 астында шағылыстырылған 52 жағдайлы корпус (детерминирленген құрылғылар + синтезделген бит-флип/хабарлама/құйрықты қысқарту негативтері). `crates/iroha_crypto/tests/sm2_wycheproof.rs` мүмкіндігінше стандартты SM2 тексерушісі арқылы осы векторларды тексереді және қажет болған жағдайда Қосымша доменінің таза BigInt іске асыруына қайтады.

## OpenSSL / Tongsuo / GmSSL қарсы SM2 қолтаңбасын тексеру

1-қосымша (Fp-256) мысалы `ALICE123@YAHOO.COM` (ENTLA 0x0090), `"message digest"` хабарын және төменде көрсетілген ашық кілтті пайдаланады. OpenSSL/Tongsuo көшіру және қою жұмыс процесі:

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

* OpenSSL 3.x `distid:` / `hexdistid:` опцияларын құжаттайды. Кейбір OpenSSL 1.1.1 құрылғылары тұтқаны `sm2_id:` ретінде көрсетеді — `openssl pkeyutl -help` ішінде қайсысы пайда болса, оны пайдаланыңыз.
* GmSSL бірдей `pkeyutl` бетін экспорттайды; ескі құрылымдар да `-pkeyopt sm2_id:...` қабылданады.
* LibreSSL (macOS/OpenBSD жүйесінде әдепкі) SM2/SM3 қолданбасын **қолданбайды**, сондықтан жоғарыдағы пәрмен сонда орындалмайды. OpenSSL ≥ 1.1.1, Tongsuo немесе GmSSL пайдаланыңыз.

DER көмекшісі қосымша қолтаңбасына сәйкес келетін `3044022040F1EC59F793D9F49E09DCEF49130D4194F79FB1EED2CAA55BACDB49C4E755D102206FC6DAC32C5D5CF10C77DFB20F7C2EB667A457872FB09EC56327A67EC7DEEBE7` шығарады.

Қосымша пайдаланушы ақпаратының хэшін және алынған дайджестті басып шығарады:

```
ZA = F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A
e  = SM3(ZA || "message digest")
   = B524F552CD82B8B028476E005C377FB19A87E6FC682D48BB5D42E3D9B9EFFE76
```

OpenSSL арқылы растауға болады:

```bash
echo -n 'F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A' \
  | xxd -r -p > za.bin
printf "message digest" >> za.bin
openssl dgst -sm3 -binary za.bin | xxd -p -c 256
# -> b524f552cd82b8b028476e005c377fb19a87e6fc682d48bb5d42e3d9b9effe76
```

Қисық теңдеу үшін Python ақылдылығын тексеру:

```python
p = int("FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFF", 16)
a = int("FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFC", 16)
b = int("28E9FA9E9D9F5E344D5A9E4BCF6509A7F39789F515AB8F92DDBCBD414D940E93", 16)
x = int("0AE4C7798AA0F119471BEE11825BE46202BB79E2A5844495E97C04FF4DF2548A", 16)
y = int("7C0240F88F1CD4E16352A73C17B7F16F07353E53A176D684A9FE0C6BB798E857", 16)
assert (pow(y, 2, p) - (pow(x, 3, p) + a * x + b)) % p == 0
```

## SM3 хэш векторлары

| Енгізу | Hex кодтау | Дайджест (он алтылық) | Дереккөз |
|-------|--------------|--------------|--------|
| `""` (бос жол) | `""` | `1ab21d8355cfa17f8e61194831e81a8f22bec8c728fefb747ed035eb5082aa2b` | GM/T 0004-2012 А.1 қосымша |
| `"abc"` | `616263` | `66c7f0f462eeedd9d1f2d46bdc10e4e24167c4875cf2f7a2297da02b8f4ba8e0` | GM/T 0004-2012 А.2 қосымша |
| `"abcd"` 16 рет қайталанады (64 байт) | `61626364` ×16 | `debe9ff92275b8a138604889c18e5a4d6fdb70e5387e5765293dcba39c0c5732` | GB/T 32905-2016 А қосымшасы |

## SM4 блок шифры (ECB) векторлары

| Кілт (алтылық) | Ашық мәтін (он алтылық) | Шифрленген мәтін (он алтылық) | Дереккөз |
|----------|-----------------|------------------|--------|
| `0123456789abcdeffedcba9876543210` | `0123456789abcdeffedcba9876543210` | `681edf34d206965e86b3e94f536e4246` | GM/T 0002-2012 А.1 қосымша |
| `0123456789abcdeffedcba9876543210` | `000102030405060708090a0b0c0d0e0f` | `59b50808d3dcf921fa30b5b3c1dddc19` | GM/T 0002-2012 А.2 қосымша |
| `0123456789abcdeffedcba9876543210` | `ffeeddccbbaa99887766554433221100` | `1c3b3f56186b70819d3f5aa11fe2c8b6` | GM/T 0002-2012 А.3 қосымша |

## SM4-GCM аутентификацияланған шифрлау

| Негізгі | IV | AAD | Ашық мәтін | Шифрленген мәтін | Тег | Дереккөз |
|-----|----|-----|-----------|------------|-----|--------|
| `0123456789abcdeffedcba9876543210` | `00001234567800000000abcd` | `feedfacedeadbeeffeedfacedeadbeefabaddad2` | `d9313225f88406e5a55909c5aff5269a` | `42831ec2217774244b7221b784d0d49c` | `4d5c2af327cd64a62cf35abd2ba6fab4` | RFC 8998 Қосымша A.2 |

## SM4-CCM аутентификацияланған шифрлау| Негізгі | Nonce | AAD | Ашық мәтін | Шифрленген мәтін | Тег | Дереккөз |
|-----|-------|-----|-----------|------------|-----|--------|
| `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `202122232425262728292a2b2c2d2e2f` | `7162015b4dac2555` | `4d26de5a` | RFC 8998 Қосымша A.3 |

### Wycheproof теріс жағдайлар (SM4 GCM/CCM)

Бұл жағдайлар `crates/iroha_crypto/tests/sm3_sm4_vectors.rs` ішіндегі регрессия жиынтығын хабарлайды. Әрбір жағдай тексерілмеуі керек.

| Режим | TC ID | Сипаттама | Негізгі | Nonce | AAD | Шифрленген мәтін | Тег | Ескертпелер |
|------|-------|-------------|-----|-------|-----|------------|-----|-------|
| GCM | 1 | Тег биттерін айналдыру | `0123456789abcdeffedcba9876543210` | `00001234567800000000abcd` | `feedfacedeadbeeffeedfacedeadbeefabaddad2` | `42831ec2217774244b7221b784d0d49c` | `5d5c2af327cd64a62cf35abd2ba6fab4` | Wycheproof туынды жарамсыз тег |
| CCM | 17 | Тег биттерін айналдыру | `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `7162015b4dac2555` | `5d26de5a` | Wycheproof туынды жарамсыз тег |
| CCM | 18 | Кесілген тег (3 байт) | `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `7162015b4dac2555` | `5d26de` | Қысқа тегтердің аутентификацияның сәтсіздігін қамтамасыз етеді |
| CCM | 19 | Шифрмәтінді биттік айналдыру | `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `7162015b4dac2554` | `5d26de5a` | Бұзылған пайдалы жүктемені анықтау |

## SM2 Детерминистік қолтаңба анықтамасы

| Өріс | Мән (егер көрсетілмеген болса, он алтылық) | Дереккөз |
|-------|--------------------------|--------|
| Қисық параметрлер | `sm2p256v1` (a = `FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFC`, т.б.) | GM/T 0003.5-2012 А қосымшасы |
| Пайдаланушы идентификаторы (`distid`) | ASCII `"ALICE123@YAHOO.COM"` (ENTLA 0x0090) | GM/T 0003 D қосымша |
| Ашық кілт | `040AE4C7798AA0F119471BEE11825BE46202BB79E2A5844495E97C04FF4DF2548A7C0240F88F1CD4E16352A73C17B7F16F07353E53A176D684A9FE0C6BB798E857` | GM/T 0003 D қосымша |
| Хабарлама | `"message digest"` (он алтылық `6d65737361676520646967657374`) | GM/T 0003 D қосымша |
| ZA | `F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A` | GM/T 0003 D қосымша |
| `e = SM3(ZA || M)` | `B524F552CD82B8B028476E005C377FB19A87E6FC682D48BB5D42E3D9B9EFFE76` | GM/T 0003 D қосымша |
| Қолтаңба `(r,s)` | `40F1EC59F793D9F49E09DCEF49130D4194F79FB1EED2CAA55BACDB49C4E755D1`, `6FC6DAC32C5D5CF10C77DFB20F7C2EB667A457872FB09EC56327A67EC7DEEBE7` | GM/T 0003 D қосымша |
| Мультикодек (уақытша) | `8626550012414C494345313233405941484F4F2E434F4D040AE4…` (`sm2-pub`, нұсқасы `0x1306`) | 1-қосымшадан алынған |
| Префиксті мультихэш | `sm2:8626550012414C494345313233405941484F4F2E434F4D040AE4…` | Шығарылған (`sm_known_answers.toml` сәйкес келеді) |

SM2 мультихэш пайдалы жүктемелері `distid_len (u16 BE) || distid bytes || SEC1 uncompressed key (65 bytes)` ретінде кодталған.

### Rust SDK анықтауыш қолтаңбасы (SM-3c)

Құрылымдық векторлар массиві Rust/Python/JavaScript паритетінің пайдалы жүктемесін қамтиды
сондықтан әрбір клиент ортақ тұқыммен бірдей SM2 хабарына қол қояды және
ажыратушы идентификатор.| Өріс | Мән (егер көрсетілмеген болса, он алтылық) | Ескертпелер |
|-------|--------------------------|-------|
| Айырмашылық идентификатор | `"iroha-sdk-sm2-fixture"` | Rust/Python/JS SDK | арқылы ортақ пайдаланылады
| Тұқым | `"iroha-rust-sdk-sm2-deterministic-fixture"` (он алтылық `69726F68612D727573742D73646B2D736D322D64657465726D696E69737469632D66697874757265`) | `Sm2PrivateKey::from_seed` | енгізу
| Жеке кілт | `E64AE95930A2324DCF11BB1657FBBC2CD2BEF7515664BE7A15CA6AE5BCE0B7CA` | Тұқымнан детерминирленген |
| Ашық кілт (SEC1 қысылмаған) | `0494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` | Детерминирленген туындыға сәйкес келеді |
| Ашық кілт мультихэш | `862658001569726F68612D73646B2D736D322D666978747572650494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` | `PublicKey::to_string()` | шығысы
| Префиксті мультихэш | `sm2:862658001569726F68612D73646B2D736D322D666978747572650494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` | `PublicKey::to_prefixed_string()` | шығысы
| ZA | `6B0229C3FBFE7A5DC6EE27228E18E512752DCCE9191EB381242A7342D568EDDE` | `Sm2PublicKey::compute_z` | арқылы есептелген
| Хабарлама | `"Rust SDK SM2 signing fixture v1"` (он алтылық `527573742053444B20534D32207369676E696E672066697874757265207631`) | SDK паритеттік сынақтарына арналған канондық пайдалы жүктеме |
| Қолтаңба `(r, s)` | `4E026F2E0FB75A6418C95C046D70DB7B95558BE30FB2001906EBE2091FA1AF76`, `299CFF374026D9E0C49C6BD4A99A1473BA17EFF429B3FAD083B1577F7559BDF5` | Детерминирленген таңба арқылы шығарылған |

- SDK арасындағы тұтыну:
  - `fixtures/sm/sm2_fixture.json` енді `vectors` массивін көрсетеді. Rust криптографиялық регрессия жинағы (`crates/iroha_crypto/tests/sm2_fixture_vectors.rs`), Rust клиент көмекшілері (`crates/iroha/src/sm.rs`), Python байланыстырулары (`python/iroha_python/tests/test_crypto.py`) және JavaScript SDK (`javascript/iroha_js/test/crypto.sm2.fixture.test.js`) барлығы осы құрылғыларды талдайды.
  - `crates/iroha/tests/sm_signing.rs` детерминирленген қол қоюды жүзеге асырады және тізбектегі мультихэш/мультикодек шығыстары арматураға сәйкес келетінін тексереді.
  - Қабылдау уақыты регрессиялық жиынтықтары (`crates/iroha_core/tests/admission_batching.rs`) егер `allowed_signing` құрамында `sm2` *және* `default_hash` `sm3-256` конфигурациясының аяқталуын қамтымаса, SM2 пайдалы жүктемелері қабылданбайды деп мәлімдейді.
- 追加カバレッジ: 異常系（無効な曲線、異常な `r/s`, `distid` 、 `crates/iroha_crypto/tests/sm2_fuzz.rs` の меншік テストで網羅済みです。1-қосымша мысал の正規ベクトルは `"message digest"` multicodec 形式で引き続き提供しています。
- Тот коды енді `Sm2PublicKey::compute_z` ашады, сондықтан ZA арматураларын бағдарламалы түрде жасауға болады; D қосымшасының регрессиясын `sm2_compute_z_matches_annex_example` қараңыз.

## Келесі әрекеттер
- Конфигурациялау SM2 қосу шекараларын күшейтуді жалғастыру үшін рұқсат беру уақытының регрессияларын (`admission_batching.rs`) бақылаңыз.
- Wycheproof SM4 GCM/CCM жағдайларымен қамтуды кеңейтіңіз және SM2 қолтаңбасын тексеру үшін сипатқа негізделген анық емес мақсаттарды алыңыз. ✅ (`sm3_sm4_vectors.rs` ішінде түсірілген жарамсыз-регистрлік жиын).
- Баламалы идентификаторларға арналған LLM сұрауы: *"Айырмашылық идентификатор ALICE123@YAHOO.COM орнына 1234567812345678 болып орнатылған кезде 1-қосымшаға SM2 қолтаңбасын беріңіз және жаңа ZA/e мәндерін көрсетіңіз."*
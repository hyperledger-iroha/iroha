---
lang: ka
direction: ltr
source: docs/source/crypto/sm_vectors.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fd46471945188bcb95c8ee411c48acc8915a92b408df196caa65bf25f0596732
source_last_modified: "2026-01-05T18:22:23.402400+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! საცნობარო ტესტის ვექტორები SM2/SM3/SM4 ინტეგრაციის სამუშაოებისთვის.

# SM ვექტორების დადგმის ნოტები

ეს დოკუმენტი აერთიანებს საჯაროდ ხელმისაწვდომ ცნობილ პასუხების ტესტებს, რომლებიც ამუშავებენ SM2/SM3/SM4 აღკაზმულობას ავტომატური იმპორტის სკრიპტების დაშვებამდე. მანქანით წაკითხვადი ასლები ცხოვრობენ:

- `crates/iroha_crypto/tests/fixtures/sm_known_answers.toml` (დანართის ვექტორები, RFC 8998 შემთხვევები, დანართი მაგალითი 1).
- `fixtures/sm/sm2_fixture.json` (გაზიარებული დეტერმინისტული SDK მოწყობილობა, რომელიც მოხმარებულია Rust/Python/JavaScript ტესტებით).
- `crates/iroha_crypto/tests/fixtures/wycheproof_sm2.json` — კურირებული 52-საქმიანი კორპუსი (განმსაზღვრელი მოწყობილობები + სინთეზირებული ბიტის გადაბრუნება/შეტყობინებების/კუდის შეკვეცის ნეგატივები) ასახული Apache-2.0-ის ქვეშ, სანამ SM2 კომპლექტი მოლოდინშია. `crates/iroha_crypto/tests/sm2_wycheproof.rs` ამოწმებს ამ ვექტორებს სტანდარტული SM2 ვერიფიკატორის გამოყენებით, როცა ეს შესაძლებელია და საჭიროების შემთხვევაში უბრუნდება ანექსის დომენის სუფთა BigInt განხორციელებას.

## SM2 ხელმოწერის დადასტურება OpenSSL / Tongsuo / GmSSL წინააღმდეგ

დანართის მაგალითი 1 (Fp-256) იყენებს იდენტურობას `ALICE123@YAHOO.COM` (ENTLA 0x0090), შეტყობინებას `"message digest"` და ქვემოთ ნაჩვენები საჯარო გასაღები. OpenSSL/Tongsuo-ს კოპირება და ჩასმა არის:

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

* OpenSSL 3.x დოკუმენტებს `distid:` / `hexdistid:` ვარიანტებს. ზოგიერთი OpenSSL 1.1.1 კონსტრუქცია აჩვენებს ღილაკს, როგორც `sm2_id:` — გამოიყენეთ ის, რაც გამოჩნდება `openssl pkeyutl -help`-ში.
* GmSSL ახორციელებს იგივე `pkeyutl` ზედაპირს; ძველი ნაგებობები ასევე მიღებულია `-pkeyopt sm2_id:...`.
* LibreSSL (ნაგულისხმევი macOS/OpenBSD-ზე) **არ** ახორციელებს SM2/SM3-ს, ამიტომ ზემოთ მოცემული ბრძანება ვერ ხერხდება. გამოიყენეთ OpenSSL ≥ 1.1.1, Tongsuo ან GmSSL.

DER დამხმარე ასხივებს `3044022040F1EC59F793D9F49E09DCEF49130D4194F79FB1EED2CAA55BACDB49C4E755D102206FC6DAC32C5D5CF10C77DFB20F7C2EB667A457872FB09EC56327A67EC7DEEBE7`, რომელიც ემთხვევა დანართის ხელმოწერას.

დანართი ასევე ბეჭდავს მომხმარებლის ინფორმაციის ჰეშს და შედეგად დაიჯესტს:

```
ZA = F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A
e  = SM3(ZA || "message digest")
   = B524F552CD82B8B028476E005C377FB19A87E6FC682D48BB5D42E3D9B9EFFE76
```

შეგიძლიათ დაადასტუროთ OpenSSL-ით:

```bash
echo -n 'F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A' \
  | xxd -r -p > za.bin
printf "message digest" >> za.bin
openssl dgst -sm3 -binary za.bin | xxd -p -c 256
# -> b524f552cd82b8b028476e005c377fb19a87e6fc682d48bb5d42e3d9b9effe76
```

პითონის საღი აზრის შემოწმება მრუდის განტოლებისთვის:

```python
p = int("FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFF", 16)
a = int("FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFC", 16)
b = int("28E9FA9E9D9F5E344D5A9E4BCF6509A7F39789F515AB8F92DDBCBD414D940E93", 16)
x = int("0AE4C7798AA0F119471BEE11825BE46202BB79E2A5844495E97C04FF4DF2548A", 16)
y = int("7C0240F88F1CD4E16352A73C17B7F16F07353E53A176D684A9FE0C6BB798E857", 16)
assert (pow(y, 2, p) - (pow(x, 3, p) + a * x + b)) % p == 0
```

## SM3 ჰეშის ვექტორები

| შეყვანა | Hex კოდირება | დაიჯესტი (ჰექს) | წყარო |
|-------|-------------|-------------|-------|
| `""` (ცარიელი სტრიქონი) | `""` | `1ab21d8355cfa17f8e61194831e81a8f22bec8c728fefb747ed035eb5082aa2b` | GM/T 0004-2012 დანართი A.1 |
| `"abc"` | `616263` | `66c7f0f462eeedd9d1f2d46bdc10e4e24167c4875cf2f7a2297da02b8f4ba8e0` | GM/T 0004-2012 დანართი A.2 |
| `"abcd"` მეორდება 16 ჯერ (64 ბაიტი) | `61626364` ×16 | `debe9ff92275b8a138604889c18e5a4d6fdb70e5387e5765293dcba39c0c5732` | გბ/ტ 32905-2016 დანართი A |

## SM4 ბლოკის შიფრის (ECB) ვექტორები

| გასაღები (hex) | ჩვეულებრივი ტექსტი (თექვსმეტი) | შიფრული ტექსტი (თექვსმეტი) | წყარო |
|-----------|----------------|-----------------|-------|
| `0123456789abcdeffedcba9876543210` | `0123456789abcdeffedcba9876543210` | `681edf34d206965e86b3e94f536e4246` | GM/T 0002-2012 დანართი A.1 |
| `0123456789abcdeffedcba9876543210` | `000102030405060708090a0b0c0d0e0f` | `59b50808d3dcf921fa30b5b3c1dddc19` | GM/T 0002-2012 დანართი A.2 |
| `0123456789abcdeffedcba9876543210` | `ffeeddccbbaa99887766554433221100` | `1c3b3f56186b70819d3f5aa11fe2c8b6` | GM/T 0002-2012 დანართი A.3 |

## SM4-GCM ავთენტიფიცირებული დაშიფვრა

| გასაღები | IV | AAD | ჩვეულებრივი ტექსტი | შიფრული ტექსტი | ტეგი | წყარო |
|-----|----|-----|----------|-----------|-----|-------|
| `0123456789abcdeffedcba9876543210` | `00001234567800000000abcd` | `feedfacedeadbeeffeedfacedeadbeefabaddad2` | `d9313225f88406e5a55909c5aff5269a` | `42831ec2217774244b7221b784d0d49c` | `4d5c2af327cd64a62cf35abd2ba6fab4` | RFC 8998 დანართი A.2 |

## SM4-CCM ავთენტიფიცირებული დაშიფვრა| გასაღები | Nonce | AAD | ჩვეულებრივი ტექსტი | შიფრული ტექსტი | ტეგი | წყარო |
|-----|-------|-----|----------|-----------|-----|-------|
| `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `202122232425262728292a2b2c2d2e2f` | `7162015b4dac2555` | `4d26de5a` | RFC 8998 დანართი A.3 |

### Wycheproof ნეგატიური შემთხვევები (SM4 GCM/CCM)

ეს შემთხვევები აცნობებს რეგრესიის კომპლექტს `crates/iroha_crypto/tests/sm3_sm4_vectors.rs`-ში. ყოველი შემთხვევის გადამოწმება არ უნდა მოხდეს.

| რეჟიმი | TC ID | აღწერა | გასაღები | Nonce | AAD | შიფრული ტექსტი | ტეგი | შენიშვნები |
|------|-------|------------|-----|------|----|-----------|-----|------|
| GCM | 1 | Tag bit flip | `0123456789abcdeffedcba9876543210` | `00001234567800000000abcd` | `feedfacedeadbeeffeedfacedeadbeefabaddad2` | `42831ec2217774244b7221b784d0d49c` | `5d5c2af327cd64a62cf35abd2ba6fab4` | Wycheproof მიღებული არასწორი ტეგი |
| CCM | 17 | Tag bit flip | `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `7162015b4dac2555` | `5d26de5a` | Wycheproof მიღებული არასწორი ტეგი |
| CCM | 18 | შეკვეცილი ტეგი (3 ბაიტი) | `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `7162015b4dac2555` | `5d26de` | უზრუნველყოფს მოკლე ტეგების ავთენტიფიკაციას |
| CCM | 19 | შიფრული ტექსტის ბიტის გადაბრუნება | `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `7162015b4dac2554` | `5d26de5a` | გაყალბებული დატვირთვის აღმოჩენა |

## SM2 დეტერმინისტული ხელმოწერის მითითება

| ველი | მნიშვნელობა (თექვსმეტი, თუ არ არის აღნიშნული) | წყარო |
|-------|------------------------|--------|
| მრუდის პარამეტრები | `sm2p256v1` (a = `FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFC` და სხვ.) | GM/T 0003.5-2012 დანართი A |
| მომხმარებლის ID (`distid`) | ASCII `"ALICE123@YAHOO.COM"` (ENTLA 0x0090) | GM/T 0003 დანართი D |
| საჯარო გასაღები | `040AE4C7798AA0F119471BEE11825BE46202BB79E2A5844495E97C04FF4DF2548A7C0240F88F1CD4E16352A73C17B7F16F07353E53A176D684A9FE0C6BB798E857` | GM/T 0003 დანართი D |
| შეტყობინება | `"message digest"` (Hex `6d65737361676520646967657374`) | GM/T 0003 დანართი D |
| ZA | `F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A` | GM/T 0003 დანართი D |
| `e = SM3(ZA || M)` | `B524F552CD82B8B028476E005C377FB19A87E6FC682D48BB5D42E3D9B9EFFE76` | GM/T 0003 დანართი D |
| ხელმოწერა `(r,s)` | `40F1EC59F793D9F49E09DCEF49130D4194F79FB1EED2CAA55BACDB49C4E755D1`, `6FC6DAC32C5D5CF10C77DFB20F7C2EB667A457872FB09EC56327A67EC7DEEBE7` | GM/T 0003 დანართი D |
| მულტიკოდეკი (დროებითი) | `8626550012414C494345313233405941484F4F2E434F4D040AE4…` (`sm2-pub`, ვარიანტი `0x1306`) | მიღებულია დანართის მაგალითი 1 |
| პრეფიქსირებული multihash | `sm2:8626550012414C494345313233405941484F4F2E434F4D040AE4…` | მიღებული (შეესაბამება `sm_known_answers.toml`) |

SM2 multihash payloads კოდირებულია როგორც `distid_len (u16 BE) || distid bytes || SEC1 uncompressed key (65 bytes)`.

### Rust SDK Deterministic Signing Fixture (SM-3c)

სტრუქტურირებული ვექტორების მასივი მოიცავს Rust/Python/JavaScript პარიტეტის დატვირთვას
ასე რომ, ყველა კლიენტი ხელს აწერს ერთსა და იმავე SM2 შეტყობინებას საზიარო სედით და
განმასხვავებელი იდენტიფიკატორი.| ველი | მნიშვნელობა (თექვსმეტი, თუ არ არის აღნიშნული) | შენიშვნები |
|-------|------------------------|-------|
| განმასხვავებელი ID | `"iroha-sdk-sm2-fixture"` | გაზიარებულია Rust/Python/JS SDK-ებში |
| თესლი | `"iroha-rust-sdk-sm2-deterministic-fixture"` (Hex `69726F68612D727573742D73646B2D736D322D64657465726D696E69737469632D66697874757265`) | შეყვანა `Sm2PrivateKey::from_seed`-ში |
| პირადი გასაღები | `E64AE95930A2324DCF11BB1657FBBC2CD2BEF7515664BE7A15CA6AE5BCE0B7CA` | წარმოიქმნება დეტერმინისტიკურად თესლიდან |
| საჯარო გასაღები (SEC1 არაკომპრესირებული) | `0494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` | შეესაბამება დეტერმინისტული წარმოშობის |
| საჯარო გასაღები multihash | `862658001569726F68612D73646B2D736D322D666978747572650494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` | გამომავალი `PublicKey::to_string()` |
| პრეფიქსირებული multihash | `sm2:862658001569726F68612D73646B2D736D322D666978747572650494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` | გამომავალი `PublicKey::to_prefixed_string()` |
| ZA | `6B0229C3FBFE7A5DC6EE27228E18E512752DCCE9191EB381242A7342D568EDDE` | გამოთვლილია `Sm2PublicKey::compute_z` |
| შეტყობინება | `"Rust SDK SM2 signing fixture v1"` (Hex `527573742053444B20534D32207369676E696E672066697874757265207631`) | კანონიკური დატვირთვა SDK პარიტეტის ტესტებისთვის |
| ხელმოწერა `(r, s)` | `4E026F2E0FB75A6418C95C046D70DB7B95558BE30FB2001906EBE2091FA1AF76`, `299CFF374026D9E0C49C6BD4A99A1473BA17EFF429B3FAD083B1577F7559BDF5` | წარმოებული დეტერმინისტული ხელმოწერით |

- Cross-SDK მოხმარება:
  - `fixtures/sm/sm2_fixture.json` ახლა ამჟღავნებს `vectors` მასივს. Rust კრიპტო რეგრესიის კომპლექტი (`crates/iroha_crypto/tests/sm2_fixture_vectors.rs`), Rust კლიენტის დამხმარეები (`crates/iroha/src/sm.rs`), Python bindings (`python/iroha_python/tests/test_crypto.py`) და JavaScript SDK (`javascript/iroha_js/test/crypto.sm2.fixture.test.js`) ყველა აანალიზებს ამ მოწყობილობებს.
  - `crates/iroha/tests/sm_signing.rs` ახორციელებს დეტერმინისტულ ხელმოწერას და ამოწმებს, რომ ჯაჭვზე მულტიჰაშის/მულტიკოდეკის გამომავლები ემთხვევა მოწყობილობებს.
  - დაშვების დროის რეგრესიის კომპლექტები (`crates/iroha_core/tests/admission_batching.rs`) ამტკიცებენ, რომ SM2 დატვირთვები უარყოფილია, თუ `allowed_signing` არ შეიცავს `sm2` *და* `default_hash` არის ```bash
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
```-ის კონფიგურაციის დასასრული.
- 追加カバレッジ: 異常系（無効な曲線、異常な `r/s`、`distid`、`distid` `crates/iroha_crypto/tests/sm2_fuzz.rs` の საკუთრება テストで網羅済みです。დანართი მაგალითი 1 の正規ベクトルは `"message digest"` მულტიკოდეკი 形式で引き続き提供しています。
- ჟანგის კოდი ახლა ავლენს `Sm2PublicKey::compute_z`-ს, ასე რომ ZA მოწყობილობების გენერირება შესაძლებელია პროგრამულად; იხილეთ `sm2_compute_z_matches_annex_example` დანართის D რეგრესიისთვის.

## შემდეგი მოქმედებები
- დააკვირდით დაშვების დროის რეგრესიებს (`admission_batching.rs`), რათა დარწმუნდეთ, რომ კონფიგურაციის კარი აგრძელებს SM2 ჩართვის საზღვრების დაცვას.
- გააფართოვეთ დაფარვა Wycheproof SM4 GCM/CCM ქეისებით და მიიღეთ საკუთრებაზე დაფუძნებული fuzz სამიზნეები SM2 ხელმოწერის გადამოწმებისთვის. ✅ (შემთხვევის არასწორი ქვეჯგუფი აღბეჭდილია `sm3_sm4_vectors.rs`-ში).
- LLM მოთხოვნა ალტერნატიული ID-ებისთვის: *"მოაწოდეთ SM2 ხელმოწერა დანართის მაგალითის 1-ისთვის, როდესაც განმასხვავებელი ID დაყენებულია 1234567812345678-ზე ALICE123@YAHOO.COM-ის ნაცვლად და გამოწერეთ ახალი ZA/e მნიშვნელობები."*
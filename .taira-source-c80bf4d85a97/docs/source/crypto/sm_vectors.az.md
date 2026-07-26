---
lang: az
direction: ltr
source: docs/source/crypto/sm_vectors.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fd46471945188bcb95c8ee411c48acc8915a92b408df196caa65bf25f0596732
source_last_modified: "2026-01-05T18:22:23.402400+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! SM2/SM3/SM4 inteqrasiya işi üçün istinad test vektorları.

# SM Vektorlarının Səhnə Qeydləri

Bu sənəd SM2/SM3/SM4 qoşqularını avtomatlaşdırılmış idxal skriptləri yerləşdirməzdən əvvəl əkmək üçün açıq olan məlum cavab testlərini birləşdirir. Maşınla oxuna bilən nüsxələr burada yaşayır:

- `crates/iroha_crypto/tests/fixtures/sm_known_answers.toml` (əlavə vektorlar, RFC 8998 halları, Əlavə Nümunə 1).
- `fixtures/sm/sm2_fixture.json` (Rust/Python/JavaScript testləri tərəfindən istehlak edilən ortaq deterministik SDK qurğusu).
- `crates/iroha_crypto/tests/fixtures/wycheproof_sm2.json` — yuxarı axın SM2 dəsti gözlənilərkən Apache-2.0 altında əks olunan 52 iş korpusu (deterministik qurğular + sintez edilmiş bit-flip/mesaj/quyruq kəsilməsi neqativləri). `crates/iroha_crypto/tests/sm2_wycheproof.rs` mümkün olduqda standart SM2 doğrulayıcısından istifadə edərək bu vektorları yoxlayır və tələb olunduqda Əlavə domeninin təmiz BigInt tətbiqinə qayıdır.

## OpenSSL / Tongsuo / GmSSL-ə qarşı SM2 İmza Doğrulaması

Əlavə Nümunə 1 (Fp-256) `ALICE123@YAHOO.COM` (ENTLA 0x0090), `"message digest"` mesajı və aşağıda göstərilən açıq açardan istifadə edir. OpenSSL/Tongsuo-nun surətini çıxarıb yapışdırın:

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

* OpenSSL 3.x `distid:` / `hexdistid:` seçimlərini sənədləşdirir. Bəzi OpenSSL 1.1.1 qurğuları düyməni `sm2_id:` kimi ifşa edir - `openssl pkeyutl -help`-də görünəndən istifadə edin.
* GmSSL eyni `pkeyutl` səthini ixrac edir; köhnə quruluşlar da `-pkeyopt sm2_id:...`-i qəbul edir.
* LibreSSL (macOS/OpenBSD-də defolt) SM2/SM3-i ** tətbiq etmir**, ona görə də yuxarıdakı əmr orada uğursuz olur. OpenSSL ≥ 1.1.1, Tongsuo və ya GmSSL istifadə edin.

DER köməkçisi əlavə imzasına uyğun gələn `3044022040F1EC59F793D9F49E09DCEF49130D4194F79FB1EED2CAA55BACDB49C4E755D102206FC6DAC32C5D5CF10C77DFB20F7C2EB667A457872FB09EC56327A67EC7DEEBE7` yayır.

Əlavədə həmçinin istifadəçi məlumatı hashı və nəticədə hazırlanmış həzm çap olunur:

```
ZA = F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A
e  = SM3(ZA || "message digest")
   = B524F552CD82B8B028476E005C377FB19A87E6FC682D48BB5D42E3D9B9EFFE76
```

OpenSSL ilə təsdiqləyə bilərsiniz:

```bash
echo -n 'F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A' \
  | xxd -r -p > za.bin
printf "message digest" >> za.bin
openssl dgst -sm3 -binary za.bin | xxd -p -c 256
# -> b524f552cd82b8b028476e005c377fb19a87e6fc682d48bb5d42e3d9b9effe76
```

Əyri tənliyi üçün Python ağlını yoxlayın:

```python
p = int("FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFF", 16)
a = int("FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFC", 16)
b = int("28E9FA9E9D9F5E344D5A9E4BCF6509A7F39789F515AB8F92DDBCBD414D940E93", 16)
x = int("0AE4C7798AA0F119471BEE11825BE46202BB79E2A5844495E97C04FF4DF2548A", 16)
y = int("7C0240F88F1CD4E16352A73C17B7F16F07353E53A176D684A9FE0C6BB798E857", 16)
assert (pow(y, 2, p) - (pow(x, 3, p) + a * x + b)) % p == 0
```

## SM3 Hash Vektorları

| Giriş | Hex Kodlaşdırma | Həzm (hex) | Mənbə |
|-------|--------------|--------------|--------|
| `""` (boş sətir) | `""` | `1ab21d8355cfa17f8e61194831e81a8f22bec8c728fefb747ed035eb5082aa2b` | GM/T 0004-2012 Əlavə A.1 |
| `"abc"` | `616263` | `66c7f0f462eeedd9d1f2d46bdc10e4e24167c4875cf2f7a2297da02b8f4ba8e0` | GM/T 0004-2012 Əlavə A.2 |
| `"abcd"` 16 dəfə təkrarlandı (64 bayt) | `61626364` ×16 | `debe9ff92275b8a138604889c18e5a4d6fdb70e5387e5765293dcba39c0c5732` | GB/T 32905-2016 Əlavə A |

## SM4 Blok Şifrəsi (ECB) Vektorları

| Açar (hex) | Düz mətn (hex) | Şifrətli mətn (hex) | Mənbə |
|----------|-----------------|------------------|--------|
| `0123456789abcdeffedcba9876543210` | `0123456789abcdeffedcba9876543210` | `681edf34d206965e86b3e94f536e4246` | GM/T 0002-2012 Əlavə A.1 |
| `0123456789abcdeffedcba9876543210` | `000102030405060708090a0b0c0d0e0f` | `59b50808d3dcf921fa30b5b3c1dddc19` | GM/T 0002-2012 Əlavə A.2 |
| `0123456789abcdeffedcba9876543210` | `ffeeddccbbaa99887766554433221100` | `1c3b3f56186b70819d3f5aa11fe2c8b6` | GM/T 0002-2012 Əlavə A.3 |

## SM4-GCM Doğrulanmış Şifrələmə

| Açar | IV | AAD | Açıq mətn | Şifrəli mətn | Tag | Mənbə |
|-----|----|-----|-----------|------------|-----|--------|
| `0123456789abcdeffedcba9876543210` | `00001234567800000000abcd` | `feedfacedeadbeeffeedfacedeadbeefabaddad2` | `d9313225f88406e5a55909c5aff5269a` | `42831ec2217774244b7221b784d0d49c` | `4d5c2af327cd64a62cf35abd2ba6fab4` | RFC 8998 Əlavə A.2 |

## SM4-CCM Doğrulanmış Şifrələmə| Açar | Nonce | AAD | Açıq mətn | Şifrəli mətn | Tag | Mənbə |
|-----|-------|-----|-----------|------------|-----|--------|
| `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `202122232425262728292a2b2c2d2e2f` | `7162015b4dac2555` | `4d26de5a` | RFC 8998 Əlavə A.3 |

### Wycheproof Neqativ hallar (SM4 GCM/CCM)

Bu hallar `crates/iroha_crypto/tests/sm3_sm4_vectors.rs`-də reqressiya dəstinə məlumat verir. Hər bir halda yoxlama uğursuz olmalıdır.

| Rejim | TC ID | Təsvir | Açar | Nonce | AAD | Şifrəli mətn | Tag | Qeydlər |
|------|-------|-------------|-----|-------|-----|------------|-----|-------|
| GCM | 1 | Tag bit flip | `0123456789abcdeffedcba9876543210` | `00001234567800000000abcd` | `feedfacedeadbeeffeedfacedeadbeefabaddad2` | `42831ec2217774244b7221b784d0d49c` | `5d5c2af327cd64a62cf35abd2ba6fab4` | Wycheproof mənşəli etibarsız teq |
| CCM | 17 | Tag bit flip | `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `7162015b4dac2555` | `5d26de5a` | Wycheproof mənşəli etibarsız teq |
| CCM | 18 | Kəsilmiş teq (3 bayt) | `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `7162015b4dac2555` | `5d26de` | Qısa teqlərin identifikasiyası uğursuzluğunu təmin edir |
| CCM | 19 | Şifrətli mətn biti çevirmək | `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `7162015b4dac2554` | `5d26de5a` | Təhlükəli yükü aşkar edin |

## SM2 Deterministik İmza Referansı

| Sahə | Dəyər (qeyd edilmədiyi təqdirdə altılıq) | Mənbə |
|-------|--------------------------|--------|
| Əyri parametrləri | `sm2p256v1` (a = `FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFC` və s.) | GM/T 0003.5-2012 Əlavə A |
| İstifadəçi ID (`distid`) | ASCII `"ALICE123@YAHOO.COM"` (ENTLA 0x0090) | GM/T 0003 Əlavə D |
| Açıq açar | `040AE4C7798AA0F119471BEE11825BE46202BB79E2A5844495E97C04FF4DF2548A7C0240F88F1CD4E16352A73C17B7F16F07353E53A176D684A9FE0C6BB798E857` | GM/T 0003 Əlavə D |
| Mesaj | `"message digest"` (hex `6d65737361676520646967657374`) | GM/T 0003 Əlavə D |
| ZA | `F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A` | GM/T 0003 Əlavə D |
| `e = SM3(ZA || M)` | `B524F552CD82B8B028476E005C377FB19A87E6FC682D48BB5D42E3D9B9EFFE76` | GM/T 0003 Əlavə D |
| İmza `(r,s)` | `40F1EC59F793D9F49E09DCEF49130D4194F79FB1EED2CAA55BACDB49C4E755D1`, `6FC6DAC32C5D5CF10C77DFB20F7C2EB667A457872FB09EC56327A67EC7DEEBE7` | GM/T 0003 Əlavə D |
| Multicodec (müvəqqəti) | `8626550012414C494345313233405941484F4F2E434F4D040AE4…` (`sm2-pub`, varint `0x1306`) | Əlavə 1 Nümunəsindən əldə edilmişdir |
| Prefiksli multihash | `sm2:8626550012414C494345313233405941484F4F2E434F4D040AE4…` | Alınmışdır (`sm_known_answers.toml` uyğun gəlir) |

SM2 multihash faydalı yükləri `distid_len (u16 BE) || distid bytes || SEC1 uncompressed key (65 bytes)` kimi kodlaşdırılıb.

### Rust SDK Deterministic Signing Fixture (SM-3c)

Strukturlaşdırılmış vektorlar massivinə Rust/Python/JavaScript paritet yükü daxildir
beləliklə, hər bir müştəri eyni SM2 mesajını paylaşılan toxumla imzalayır və
fərqləndirici identifikator.| Sahə | Dəyər (qeyd edilmədiyi təqdirdə altılıq) | Qeydlər |
|-------|--------------------------|-------|
| Fərqləndirici ID | `"iroha-sdk-sm2-fixture"` | Rust/Python/JS SDK-larda paylaşılır |
| Toxum | `"iroha-rust-sdk-sm2-deterministic-fixture"` (hex `69726F68612D727573742D73646B2D736D322D64657465726D696E69737469632D66697874757265`) | `Sm2PrivateKey::from_seed` | daxil edin
| Şəxsi açar | `E64AE95930A2324DCF11BB1657FBBC2CD2BEF7515664BE7A15CA6AE5BCE0B7CA` | Toxumdan deterministik şəkildə |
| Açıq açar (SEC1 sıxılmamış) | `0494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` | Deterministik törəmə uyğun gəlir |
| Açıq açar multihash | `862658001569726F68612D73646B2D736D322D666978747572650494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` | `PublicKey::to_string()` | çıxışı
| Prefiksli multihash | `sm2:862658001569726F68612D73646B2D736D322D666978747572650494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` | `PublicKey::to_prefixed_string()` | çıxışı
| ZA | `6B0229C3FBFE7A5DC6EE27228E18E512752DCCE9191EB381242A7342D568EDDE` | `Sm2PublicKey::compute_z` | vasitəsilə hesablanır
| Mesaj | `"Rust SDK SM2 signing fixture v1"` (hex `527573742053444B20534D32207369676E696E672066697874757265207631`) | SDK paritet testləri üçün kanonik faydalı yük |
| İmza `(r, s)` | `4E026F2E0FB75A6418C95C046D70DB7B95558BE30FB2001906EBE2091FA1AF76`, `299CFF374026D9E0C49C6BD4A99A1473BA17EFF429B3FAD083B1577F7559BDF5` | Deterministik imzalama ilə istehsal olunur |

- Çapraz SDK istehlakı:
  - `fixtures/sm/sm2_fixture.json` indi `vectors` massivini ifşa edir. Rust kripto reqressiya dəsti (`crates/iroha_crypto/tests/sm2_fixture_vectors.rs`), Rust müştəri köməkçiləri (`crates/iroha/src/sm.rs`), Python bağlamaları (`python/iroha_python/tests/test_crypto.py`) və JavaScript SDK (`javascript/iroha_js/test/crypto.sm2.fixture.test.js`) hamısı bu qurğuları təhlil edir.
  - `crates/iroha/tests/sm_signing.rs` deterministik imzalamanı həyata keçirir və zəncirli multihash/multicodec çıxışlarının qurğuya uyğun olduğunu yoxlayır.
  - Qəbul vaxtı reqressiya dəstləri (`crates/iroha_core/tests/admission_batching.rs`) təsdiq edir ki, `allowed_signing` `sm2` *və* `default_hash` konfiqurasiyanın sonunda `sm3-256`-i əhatə etmirsə, SM2 faydalı yükləri rədd edilir.
- 追加カバレッジ: 異常系（無効な曲線、異常な `r/s`, `distid` 、 `crates/iroha_crypto/tests/sm2_fuzz.rs` の əmlak テストで網羅済みです。Əlavə Nümunə 1 の正規ベクトルは `"message digest"` multicodec 形式で引き続き提供しています。
- Pas kodu indi `Sm2PublicKey::compute_z`-i ifşa edir, beləliklə ZA qurğuları proqramlı şəkildə yaradıla bilər; Əlavə D reqresiyası üçün `sm2_compute_z_matches_annex_example`-ə baxın.

## Növbəti Fəaliyyətlər
- Konfiqurasiya keçidinin SM2 aktivləşdirmə sərhədlərini tətbiq etməyə davam etməsini təmin etmək üçün qəbul vaxtı reqressiyalarına (`admission_batching.rs`) baxın.
- Wycheproof SM4 GCM/CCM halları ilə əhatə dairəsini genişləndirin və SM2 imzasının yoxlanılması üçün əmlaka əsaslanan qeyri-səlis hədəflər əldə edin. ✅ (Etibarsız hal alt çoxluğu `sm3_sm4_vectors.rs`-də çəkilib).
- Alternativ identifikatorlar üçün LLM sorğusu: *"Fərqləndirici ID ALICE123@YAHOO.COM əvəzinə 1234567812345678 olaraq təyin edildikdə, Əlavə Nümunə 1 üçün SM2 imzasını təqdim edin və yeni ZA/e dəyərlərini kontur edin."*
---
lang: az
direction: ltr
source: docs/source/crypto/sm_rustcrypto_spike.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1f133d9489c4bcfae2212e6c5dc098f39c3dea3e5cd42855ba76e8c9b73b4d03
source_last_modified: "2025-12-29T18:16:35.946614+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! RustCrypto SM inteqrasiyası üçün qeydlər.

# RustCrypto SM Spike Qeydləri

## Məqsəd
RustCrypto-nun `sm2`, `sm3` və `sm4` qutularını (əlavə olaraq `rfc6979`, `ccm`, ```toml
[features]
sm = ["dep:sm2", "dep:sm3", "dep:sm4", "dep:rfc6979"]

[dependencies]
sm2 = { version = "0.13", optional = true, default-features = false, features = ["std"] }
sm3 = { version = "0.5.0-rc.1", optional = true }
sm4 = { version = "0.5.1", optional = true }
rfc6979 = { version = "0.4", optional = true, default-features = false }
```-da təmiz çatışmazlıqlar) təqdim etdiyini təsdiqləyin. `iroha_crypto` qutusu və xüsusiyyət bayrağını daha geniş iş sahəsinə bağlamadan əvvəl məqbul quraşdırma vaxtları verir.

## Təklif olunan asılılıq xəritəsi

| Sandıq | Təklif olunan versiya | Xüsusiyyətlər | Qeydlər |
|-------|-------------------|----------|-------|
| `sm2` | `0.13` (RustCrypto/imzalar) | `std` | `elliptic-curve`-dən asılıdır; MSRV-nin iş sahəsinə uyğun olduğunu yoxlayın. |
| `sm3` | `0.5.0-rc.1` (RustCrypto/heshlər) | default | API `sha2` paralelləri, mövcud `digest` xüsusiyyətləri ilə inteqrasiya olunur. |
| `sm4` | `0.5.1` (RustCrypto/blok-şifrlər) | default | Şifrə xüsusiyyətləri ilə işləyir; AEAD sarğıları daha sonra artıma təxirə salındı. |
| `rfc6979` | `0.4` | default | Deterministik olmayan törəmə üçün təkrar istifadə edin. |

*Versiyalar 2024-12-ci illərə olan cari buraxılışları əks etdirir; enməzdən əvvəl `cargo search` ilə təsdiqləyin.*

## Manifest Dəyişikliklər (qaralama)

```toml
[features]
sm = ["dep:sm2", "dep:sm3", "dep:sm4", "dep:rfc6979"]

[dependencies]
sm2 = { version = "0.13", optional = true, default-features = false, features = ["std"] }
sm3 = { version = "0.5.0-rc.1", optional = true }
sm4 = { version = "0.5.1", optional = true }
rfc6979 = { version = "0.4", optional = true, default-features = false }
```

İzləmə: artıq `iroha_crypto` (hazırda `0.13.8`)-də olan versiyaları uyğunlaşdırmaq üçün `elliptic-curve` pin.

## Spike Yoxlama Siyahısı
- [x] `crates/iroha_crypto/Cargo.toml`-ə əlavə asılılıqlar və xüsusiyyət əlavə edin.
- [x] `signature::sm` modulunu `cfg(feature = "sm")` arxasında məftilləri təsdiqləmək üçün yer tutucu strukturları ilə yaradın.
- [x] Kompilyasiyanı təsdiqləmək üçün `cargo check -p iroha_crypto --features sm`-i işə salın; rekord qurma vaxtı və yeni asılılıq sayı (`cargo tree --features sm`).
- [x] `cargo check -p iroha_crypto --features sm --locked` ilə yalnız std duruşunu təsdiq edin; `no_std` quruluşları artıq dəstəklənmir.
- [x] `docs/source/crypto/sm_program.md`-də fayl nəticələri (vaxtlar, asılılıq ağacı deltası).

## Tutmaq üçün müşahidələr
- Əlavə tərtib vaxtı və əsas.
- `cargo builtinsize` ilə ikili ölçülü təsir (ölçülənə bilərsə).
- İstənilən MSRV və ya xüsusiyyət ziddiyyətləri (məsələn, `elliptic-curve` kiçik versiyaları ilə).
- Upstream yamaqları tələb edə bilən xəbərdarlıqlar (təhlükəsiz kod, const-fn qapısı).

## Gözləyən Elementlər
- İş yerindən asılılıq qrafikini şişirtməzdən əvvəl Crypto WG-nin təsdiqini gözləyin.
- Nəzərdən keçirilməsi üçün qutuları satıb-verməməyinizi və ya crates.io-ya etibar etməyinizi təsdiqləyin (güzgülər tələb oluna bilər).
- Yoxlama siyahısını tamamlamadan əvvəl `Cargo.lock` yeniləməsini `sm_lock_refresh_plan.md` ilə əlaqələndirin.
- Kilid faylını və asılılıq ağacını bərpa etmək üçün icazə verildikdən sonra `scripts/sm_lock_refresh.sh` istifadə edin.

## 19-01-2025 Spike Log
- `iroha_crypto`-də əlavə asılılıqlar (`sm2 0.13`, `sm3 0.5.0-rc.1`, `sm4 0.5.1`, `rfc6979 0.4`) və `sm` xüsusiyyət bayrağı.
- Kompilyasiya zamanı hashing/blok şifrə API-lərini həyata keçirmək üçün stubbed `signature::sm` modulu.
- `cargo check -p iroha_crypto --features sm --locked` indi asılılıq qrafikini həll edir, lakin `Cargo.lock` yeniləmə tələbi ilə dayandırır; repozitoriya siyasəti kilid faylı redaktələrini qadağan edir, buna görə də biz icazə verilən kilid yeniləməsini koordinasiya edənə qədər kompilyasiya əməliyyatı gözləmədə qalır.## 12-02-2026 Spike Log
- Əvvəlki kilid faylı bloklayıcısı həll edildi - asılılıqlar artıq tutuldu - buna görə də `cargo check -p iroha_crypto --features sm --locked` uğur qazandı (dev Mac-də soyuq qurma 7.9s; artımlı təkrar işləmə 0.23s).
- `cargo check -p iroha_crypto --no-default-features --features "std sm" --locked` yalnız `std` konfiqurasiyalarında (`no_std` yolu qalmır) isteğe bağlı xüsusiyyətin tərtib edilməsini təsdiq edərək 1.0 saniyədə keçir.
- `sm` funksiyasını aktivləşdirən asılılıq deltası 11 qutu təqdim edir: `base64ct`, `ghash`, `opaque-debug`, `pem-rfc7468`, `pem-rfc7468`, `sm2` `primeorder`, `sm2`, `sm3`, `sm4` və `sm4-gcm`. (`rfc6979` artıq əsas qrafikin bir hissəsi idi.)
- İstifadə edilməmiş NEON siyasət köməkçiləri üçün xəbərdarlıqların qurulması davam edir; Ölçmə hamarlama iş vaxtı həmin kod yollarını yenidən aktivləşdirənə qədər olduğu kimi buraxın.
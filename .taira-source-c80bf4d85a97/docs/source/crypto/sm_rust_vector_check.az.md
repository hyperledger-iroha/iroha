---
lang: az
direction: ltr
source: docs/source/crypto/sm_rust_vector_check.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ce2f95b8b287c18c39232418333fbefdd300c030391be9dbfa4e29a3fd5f3e14
source_last_modified: "2025-12-29T18:16:35.946190+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! RustCrypto qutularından istifadə edərək SM2 Əlavə D vektorlarının yoxlanmasına dair qeydlər.

# SM2 Əlavə D Vektor Doğrulaması (RustCrypto)

Bu araşdırma GM/T 0003 Əlavə D nümunəsini RustCrypto-nun `sm2` qutusu ilə təsdiqləmək (və sazlamaq) üçün istifadə etdiyimiz addımları əks etdirir. Kanonik Əlavə Nümunə 1 datası (identifikasiya `ALICE123@YAHOO.COM`, mesaj `"message digest"` və dərc edilmiş `(r, s)`) indi `crates/iroha_crypto/tests/fixtures/sm_known_answers.toml`-də qeydə alınıb. OpenSSL/Tongsuo/gmssl imzanı məmnuniyyətlə yoxlayır (bax: `sm_vectors.md`), lakin RustCrypto-nun `sm2 v0.13.3` hələ də `signature::Error` ilə bu nöqtəni rədd edir, beləliklə, CLI pariteti təsdiqlənir, eyni zamanda Rust qoşqu düzəltmək qalır.

## Müvəqqəti qutu

```bash
cargo new /tmp/sm2_verify --bin
cd /tmp/sm2_verify
```

`Cargo.toml`:

```toml
[package]
name = "sm2_verify"
version = "0.1.0"
edition = "2024"

[dependencies]
hex = "0.4"
sm2 = "0.13.3"
```

`src/main.rs`:

```rust
use hex::FromHex;
use sm2::dsa::{signature::Verifier, Signature, VerifyingKey};

fn main() {
    let distid = "ALICE123@YAHOO.COM";
    let sig_bytes = <Vec<u8>>::from_hex(
        "40f1ec59f793d9f49e09dcef49130d4194f79fb1eed2caa55bacdb49c4e755d16fc6dac32c5d5cf10c77dfb20f7c2eb667a457872fb09ec56327a67ec7deebe7",
    )
    .expect("signature hex");
    let sig_array = <[u8; 64]>::try_from(sig_bytes.as_slice()).unwrap();
    let signature = Signature::from_bytes(&sig_array).unwrap();

    let public_key = <Vec<u8>>::from_hex(
        "040ae4c7798aa0f119471bee11825be46202bb79e2a5844495e97c04ff4df2548a7c0240f88f1cd4e16352a73c17b7f16f07353e53a176d684a9fe0c6bb798e857",
    )
    .expect("public key hex");

    // This still returns Err with RustCrypto 0.13.3 – track upstream.
    let verifying_key = VerifyingKey::from_sec1_bytes(distid, &public_key).unwrap();

    verifying_key
        .verify(b"message digest", &signature)
        .expect("signature verified");
}
```

## Tapıntılar

- `(r, s)` kanonik Əlavə Nümunəsinə uyğun olaraq doğrulama hazırda uğursuz olur, çünki `sm2::VerifyingKey::from_sec1_bytes` `signature::Error` qaytarır; yuxarı/əsas səbəbi izləyin (ehtimal ki, qutunun cari buraxılışında əyri-parametr uyğunsuzluğu səbəbindən).
- Qoşqu `sm2 v0.13.3` ilə təmiz şəkildə tərtib edilir və RustCrypto (və ya yamaqlı çəngəl) Əlavə Nümunə 1 nöqtə/imza cütünü qəbul etdikdən sonra avtomatlaşdırılmış reqressiya testinə çevriləcək.
- OpenSSL/Tongsuo/gmssl yoxlaması `sm_vectors.md`-dəki əmrlərlə uğurla başa çatır; LibreSSL (macOS default) hələ də SM2/SM3 dəstəyinə malik deyil, buna görə də yerli boşluq.

## Növbəti addımlar

1. `sm2` qoşqu yerli olaraq keçə bilməsi üçün Əlavə Nümunəni qəbul edən API 1 nöqtəsini (yaxud yuxarı axını əyri parametrləri təsdiq etdikdən sonra) ifşa etdikdən sonra yenidən sınaqdan keçirin.
2. RustCrypto düzəliş yerləşənə qədər kanonik Əlavə Nümunəsini qorumaq üçün CI boru kəmərlərində CLI ağlı yoxlanışı (OpenSSL/Tongsuo/gmssl) saxlayın.
3. RustCrypto və OpenSSL paritet yoxlamaları uğurla başa çatdıqdan sonra qoşqu Iroha-in reqressiya dəstinə təşviq edin.
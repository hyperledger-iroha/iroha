---
lang: es
direction: ltr
source: docs/source/crypto/sm_rust_vector_check.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ce2f95b8b287c18c39232418333fbefdd300c030391be9dbfa4e29a3fd5f3e14
source_last_modified: "2026-01-03T18:07:57.109606+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! Notas sobre la verificación de vectores del Anexo D de SM2 utilizando cajas RustCrypto.

# Verificación de vectores del Anexo D SM2 (RustCrypto)

Este tutorial captura los pasos que utilizamos para validar (y depurar) el ejemplo del Anexo D GM/T 0003 con la caja `sm2` de RustCrypto. Los datos canónicos del ejemplo 1 del anexo (identidad `ALICE123@YAHOO.COM`, mensaje `"message digest"` y el `(r, s)` publicado) ahora se registran en `crates/iroha_crypto/tests/fixtures/sm_known_answers.toml`. OpenSSL/Tongsuo/gmssl verifica felizmente la firma (ver `sm_vectors.md`), pero el `sm2 v0.13.3` de RustCrypto aún rechaza el punto con `signature::Error`, por lo que la paridad CLI se confirma mientras el arnés de Rust permanece pendiente de una solución ascendente.

## Caja temporal

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

## Hallazgos

- La verificación con el ejemplo canónico del anexo 1 `(r, s)` actualmente falla porque `sm2::VerifyingKey::from_sec1_bytes` devuelve `signature::Error`; rastrear la causa ascendente/raíz (probablemente debido a una discrepancia en los parámetros de la curva en la versión actual de la caja).
- El arnés se compila limpiamente con `sm2 v0.13.3` y se convertirá en una prueba de regresión automatizada una vez que RustCrypto (o una bifurcación parcheada) acepte el par de puntos/firma del ejemplo 1 del Anexo.
- La verificación de OpenSSL/Tongsuo/gmssl se realiza correctamente con los comandos en `sm_vectors.md`; LibreSSL (predeterminado para macOS) aún carece de soporte para SM2/SM3, de ahí la brecha local.

## Próximos pasos

1. Vuelva a realizar la prueba una vez que `sm2` exponga una API que acepte el punto del ejemplo 1 del anexo (o después de que upstream confirme los parámetros de la curva) para que el arnés pueda pasar localmente.
2. Mantenga una verificación de integridad de CLI (OpenSSL/Tongsuo/gmssl) en las canalizaciones de CI para proteger el ejemplo del anexo canónico hasta que se solucione RustCrypto.
3. Promocione el arnés en la suite de regresión de Iroha después de que las comprobaciones de paridad de RustCrypto y OpenSSL sean exitosas.
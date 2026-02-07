---
lang: fr
direction: ltr
source: docs/source/crypto/sm_rust_vector_check.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ce2f95b8b287c18c39232418333fbefdd300c030391be9dbfa4e29a3fd5f3e14
source_last_modified: "2026-01-03T18:07:57.109606+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! Notes sur la vérification des vecteurs SM2 Annexe D à l'aide des caisses RustCrypto.

# SM2 Annexe D Vérification du vecteur (RustCrypto)

Cette procédure pas à pas capture les étapes que nous avons utilisées pour valider (et déboguer) l'exemple GM/T 0003 Annexe D avec la caisse `sm2` de RustCrypto. Les données canoniques de l'exemple 1 de l'annexe 1 (identité `ALICE123@YAHOO.COM`, message `"message digest"` et `(r, s)` publié) sont désormais enregistrées dans `crates/iroha_crypto/tests/fixtures/sm_known_answers.toml`. OpenSSL/Tongsuo/gmssl vérifie volontiers la signature (voir `sm_vectors.md`), mais `sm2 v0.13.3` de RustCrypto rejette toujours le point avec `signature::Error`, donc la parité CLI est confirmée tandis que le harnais Rust reste en attente d'un correctif en amont.

## Caisse temporaire

```bash
cargo new /tmp/sm2_verify --bin
cd /tmp/sm2_verify
```

`Cargo.toml` :

```toml
[package]
name = "sm2_verify"
version = "0.1.0"
edition = "2024"

[dependencies]
hex = "0.4"
sm2 = "0.13.3"
```

`src/main.rs` :

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

## Résultats

- La vérification par rapport à l'exemple d'annexe canonique 1 `(r, s)` échoue actuellement, car `sm2::VerifyingKey::from_sec1_bytes` renvoie `signature::Error` ; suivre la cause en amont/racine (probablement en raison d'une inadéquation des paramètres de courbe dans la version actuelle de la caisse).
- Le harnais se compile proprement avec `sm2 v0.13.3` et deviendra un test de régression automatisé une fois que RustCrypto (ou un fork corrigé) aura accepté la paire point/signature de l'exemple d'annexe 1.
- La vérification OpenSSL/Tongsuo/gmssl réussit avec les commandes dans `sm_vectors.md` ; LibreSSL (macOS par défaut) ne prend toujours pas en charge SM2/SM3, d'où l'écart local.

## Étapes suivantes

1. Effectuez un nouveau test une fois que `sm2` expose une API qui accepte le point de l'exemple 1 de l'annexe (ou après avoir confirmé en amont les paramètres de courbe) afin que le faisceau puisse passer localement.
2. Conservez un contrôle d'intégrité CLI (OpenSSL/Tongsuo/gmssl) dans les pipelines CI pour protéger l'exemple d'annexe canonique jusqu'à ce que le correctif RustCrypto arrive.
3. Promouvez le harnais dans la suite de régression de Iroha une fois les contrôles de parité RustCrypto et OpenSSL réussis.
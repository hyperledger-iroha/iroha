---
lang: pt
direction: ltr
source: docs/source/crypto/sm_rust_vector_check.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ce2f95b8b287c18c39232418333fbefdd300c030391be9dbfa4e29a3fd5f3e14
source_last_modified: "2026-01-03T18:07:57.109606+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! Notas sobre a verificação de vetores SM2 Anexo D usando caixas RustCrypto.

# Verificação de vetor SM2 Anexo D (RustCrypto)

Este passo a passo captura as etapas que usamos para validar (e depurar) o exemplo do Anexo D do GM/T 0003 com a caixa `sm2` do RustCrypto. Os dados canônicos do Exemplo 1 do Anexo (identidade `ALICE123@YAHOO.COM`, mensagem `"message digest"` e o `(r, s)` publicado) agora são registrados em `crates/iroha_crypto/tests/fixtures/sm_known_answers.toml`. OpenSSL/Tongsuo/gmssl verificam alegremente a assinatura (consulte `sm_vectors.md`), mas o `sm2 v0.13.3` do RustCrypto ainda rejeita o ponto com `signature::Error`, então a paridade CLI é confirmada enquanto o chicote Rust permanece pendente de uma correção upstream.

## Caixa temporária

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

## Descobertas

- A verificação em relação ao Anexo canônico Exemplo 1 `(r, s)` atualmente falha porque `sm2::VerifyingKey::from_sec1_bytes` retorna `signature::Error`; rastreie a causa upstream/raiz (provavelmente devido à incompatibilidade de parâmetros de curva na versão atual da caixa).
- O chicote é compilado corretamente com `sm2 v0.13.3` e se tornará um teste de regressão automatizado assim que o RustCrypto (ou um fork corrigido) aceitar o par de ponto/assinatura do Exemplo 1 do Anexo.
- A verificação OpenSSL/Tongsuo/gmssl é bem-sucedida com os comandos em `sm_vectors.md`; O LibreSSL (padrão do macOS) ainda carece de suporte SM2/SM3, daí a lacuna local.

##Próximas etapas

1. Teste novamente assim que `sm2` expor uma API que aceita o ponto do Exemplo 1 do Anexo (ou após o upstream confirmar os parâmetros da curva) para que o chicote possa passar localmente.
2. Mantenha uma verificação de integridade da CLI (OpenSSL/Tongsuo/gmssl) nos pipelines de CI para proteger o exemplo canônico do anexo até que a correção do RustCrypto chegue.
3. Promova o chicote no conjunto de regressão do Iroha após as verificações de paridade RustCrypto e OpenSSL serem bem-sucedidas.
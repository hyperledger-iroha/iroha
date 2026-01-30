---
lang: ja
direction: ltr
source: docs/portal/docs/soranet/pq-primitives.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: pq-primitives
title: Primitivos pos-quanticos do SoraNet
sidebar_label: Primitivos PQ
description: Visao geral do crate `soranet_pq` e de como o handshake do SoraNet consome helpers ML-KEM/ML-DSA.
---

:::note Fonte canonica
Esta pagina espelha `docs/source/soranet/pq_primitives.md`. Mantenha ambas as copias sincronizadas.
:::

O crate `soranet_pq` contem os blocos pos-quanticos nos quais todo relay, client e componente de tooling do SoraNet confia. Ele envolve as suites Kyber (ML-KEM) e Dilithium (ML-DSA) suportadas por PQClean e adiciona helpers de HKDF e RNG hedged amigaveis ao protocolo para que todas as superficies compartilhem implementacoes identicas.

## O que vem em `soranet_pq`

- **ML-KEM-512/768/1024:** geracao deterministica de chaves, helpers de encapsulation e decapsulation com propagacao de erros em tempo constante.
- **ML-DSA-44/65/87:** assinatura/verificacao separadas para transcricoes com separacao de dominio.
- **HKDF com rotulo:** `derive_labeled_hkdf` adiciona namespace a cada derivacao com o estagio do handshake (`DH/es`, `KEM/1`, ...) para que transcricoes hibridas fiquem sem colisao.
- **Aleatoriedade hedged:** `hedged_chacha20_rng` mistura seeds deterministicas com entropia do SO e zera o estado intermediario ao descartar.

Todos os segredos ficam dentro de contenedores `Zeroizing` e a CI exercita os bindings PQClean em todas as plataformas suportadas.

```rust
use soranet_pq::{
    encapsulate_mlkem, decapsulate_mlkem, generate_mlkem_keypair, MlKemSuite,
    derive_labeled_hkdf, HkdfDomain, HkdfSuite,
};

let kem = generate_mlkem_keypair(MlKemSuite::MlKem768);
let (client_secret, ciphertext) = encapsulate_mlkem(MlKemSuite::MlKem768, kem.public_key()).unwrap();
let server_secret = decapsulate_mlkem(MlKemSuite::MlKem768, kem.secret_key(), ciphertext.as_bytes()).unwrap();
assert_eq!(client_secret.as_bytes(), server_secret.as_bytes());

let okm = derive_labeled_hkdf(
    HkdfSuite::Sha3_256,
    None,
    client_secret.as_bytes(),
    HkdfDomain::soranet("KEM/1"),
    b"soranet-transcript",
    32,
).unwrap();
```

## Como consumir

1. **Adicione a dependencia** a crates fora da raiz do workspace:

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. **Selecione a suite correta** nos pontos de chamada. Para o trabalho inicial do handshake hibrido, use `MlKemSuite::MlKem768` e `MlDsaSuite::MlDsa65`.

3. **Derive chaves com rotulos.** Use `HkdfDomain::soranet("KEM/1")` (e similares) para que o encadeamento de transcricoes fique deterministico entre nodes.

4. **Use o RNG hedged** ao amostrar segredos de fallback:

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

O handshake central do SoraNet e os helpers de blinding de CID (`iroha_crypto::soranet`) puxam essas utilidades diretamente, o que significa que crates downstream herdam as mesmas implementacoes sem linkar bindings PQClean por conta propria.

## Checklist de validacao

- `cargo test -p soranet_pq --offline`
- `cargo fmt --package soranet_pq`
- Audite os exemplos de uso no README (`crates/soranet_pq/README.md`)
- Atualize o documento de design do handshake do SoraNet quando os hybrids chegarem

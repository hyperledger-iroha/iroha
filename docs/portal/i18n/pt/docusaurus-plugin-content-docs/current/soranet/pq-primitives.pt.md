---
lang: pt
direction: ltr
source: docs/portal/docs/soranet/pq-primitives.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pq-primitivos
título: Primitivos pós-quanticos do SoraNet
sidebar_label: Primitivos PQ
description: Visão geral do crate `soranet_pq` e de como o handshake do SoraNet consome helpers ML-KEM/ML-DSA.
---

:::nota Fonte canônica
Esta página espelha `docs/source/soranet/pq_primitives.md`. Mantenha ambas as cópias sincronizadas.
:::

A caixa `soranet_pq` contém os blocos pós-quanticos nos quais todo relé, cliente e componente de ferramental do SoraNet confia. Ele envolve as suítes Kyber (ML-KEM) e Dilithium (ML-DSA) suportadas por PQClean e adiciona helpers de HKDF e RNG hedged amigosíveis ao protocolo para que todas as superfícies compartilhem implementações idênticas.

## O que vem em `soranet_pq`

- **ML-KEM-512/768/1024:** geração determinística de chaves, ajudantes de encapsulamento e desencapsulação com propagação de erros em tempo constante.
- **ML-DSA-44/65/87:** assinatura/verificação separada para transcrições com separação de domínio.
- **HKDF com rotulo:** `derive_labeled_hkdf` adiciona namespace a cada derivacao com o estágio do handshake (`DH/es`, `KEM/1`, ...) para que transcrições hibridas fiquem sem colisao.
- **Aleatoriedade hedged:** `hedged_chacha20_rng` misturar sementes determinísticas com entropia do SO e zerar o estado intermediário ao descartar.

Todos os segredos ficam dentro dos contêineres `Zeroizing` e o CI exerce as ligações PQClean em todas as plataformas suportadas.

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

1. **Adicione a dependência** a crates fora da raiz do espaço de trabalho:

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. **Selecione a suíte correta** nos pontos de chamada. Para o trabalho inicial do handshake híbrido, use `MlKemSuite::MlKem768` e `MlDsaSuite::MlDsa65`.

3. **Derive chaves com rotulos.** Use `HkdfDomain::soranet("KEM/1")` (e similares) para que o encadeamento de transcrições fique determinístico entre nós.

4. **Use o RNG hedged** ao amostrar segredos de fallback:

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

O handshake central do SoraNet e os helpers de blinding de CID (`iroha_crypto::soranet`) puxam essas utilidades diretamente, o que significa que crates downstream herdam as mesmas implementacoes sem linkar binds PQClean por conta própria.

## Checklist de validação

-`cargo test -p soranet_pq --offline`
-`cargo fmt --package soranet_pq`
- Audite os exemplos de uso no README (`crates/soranet_pq/README.md`)
- Atualizar o documento de design do handshake do SoraNet quando os híbridos chegarem
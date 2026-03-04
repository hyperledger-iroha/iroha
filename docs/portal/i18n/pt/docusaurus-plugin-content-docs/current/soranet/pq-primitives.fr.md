---
lang: pt
direction: ltr
source: docs/portal/docs/soranet/pq-primitives.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pq-primitivos
título: Primitivos pós-quantiques SoraNet
sidebar_label: Primitivos PQ
description: Veja o conjunto da caixa `soranet_pq` e a maneira de fazer com que o handshake SoraNet consuma os ajudantes ML-KEM/ML-DSA.
---

:::nota Fonte canônica
:::

A caixa `soranet_pq` contém os briques pós-quanticos em todos os relés, clientes e componentes das ferramentas SoraNet. Ele encapsula as suítes Kyber (ML-KEM) e Dilithium (ML-DSA) anexadas a PQClean e adiciona os auxiliares HKDF e RNG protegidos por protocolo para que todas as superfícies participantes de implementações idênticas.

## Isto é livre em `soranet_pq`

- **ML-KEM-512/768/1024:** geração determinística de cles, encapsulamento e desencapsulação com propagação de erros em tempo constante.
- **ML-DSA-44/65/87:** assinatura/verificação destacada com transcrições e separação de domínio.
- **Etiqueta HKDF:** `derive_labeled_hkdf` aplica um namespace a cada derivação por meio da etapa do handshake (`DH/es`, `KEM/1`, ...) para que as transcrições híbridas permaneçam sem colisão.
- **Aleatório coberto:** `hedged_chacha20_rng` combina as sementes determinadas com a entropia do sistema e zera o estado intermediário à destruição.

Todos os segredos existentes nos recipientes `Zeroizing` e CI exercem as ligações PQClean em todas as placas suportadas.

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

## Comment l'utiliser

1. **Adicione a dependência** nas caixas atrás da área de trabalho:

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. **Selecione a suíte correta** nos pontos de chamada. Para o trabalho inicial do handshake híbrido, use `MlKemSuite::MlKem768` e `MlDsaSuite::MlDsa65`.

3. **Deriva os arquivos com rótulos.** Utilize `HkdfDomain::soranet("KEM/1")` (e equivalentes) para que o encadeamento das transcrições seja determinado entre os noeudos.

4. **Utilize o RNG protegido** para descobrir os segredos da resposta:

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

O handshake central do SoraNet e os ajudantes de blindagem do CID (`iroha_crypto::soranet`) usam diretamente esses utilitários, o que significa que as caixas downstream herdam as implementações de memes sem as ligações PQClean eux-memes.

## Checklist de validação

-`cargo test -p soranet_pq --offline`
-`cargo fmt --package soranet_pq`
- Audite os exemplos de uso do README (`crates/soranet_pq/README.md`)
- Envie hoje o documento de concepção do aperto de mão SoraNet quando os híbridos chegarem
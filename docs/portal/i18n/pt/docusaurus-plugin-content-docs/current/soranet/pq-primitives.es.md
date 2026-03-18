---
lang: pt
direction: ltr
source: docs/portal/docs/soranet/pq-primitives.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pq-primitivos
título: Primitivas poscuanticas de SoraNet
sidebar_label: Primitivas PQ
descrição: Resumo da caixa `soranet_pq` e de como o handshake de SoraNet consome ajudantes ML-KEM/ML-DSA.
---

:::nota Fonte canônica
Esta página reflete `docs/source/soranet/pq_primitives.md`. Mantenha ambas as versões sincronizadas até que o conjunto de documentação herdado seja retirado.
:::

A caixa `soranet_pq` contém os blocos possíveis nos quais todos os relés, clientes e componentes de ferramentas do SoraNet são confiáveis. Envolva as suítes Kyber (ML-KEM) e Dilithium (ML-DSA) respaldadas por PQClean e adicione ajudantes de HKDF e RNG hedged aptos para o protocolo para que todas as superfícies compartilhem implementações idênticas.

## Que inclui `soranet_pq`

- **ML-KEM-512/768/1024:** geração determinística de chaves, encapsulamento e descapsulação com propagação de erros em tempo constante.
- **ML-DSA-44/65/87:** firmado/verificação separada para transcrições com separação de domínio.
- **HKDF etiquetado:** `derive_labeled_hkdf` adiciona namespace a cada derivação com a etapa do handshake (`DH/es`, `KEM/1`, ...) para que as transcrições híbridas não colidam.
- **Aleatória coberta:** `hedged_chacha20_rng` mistura sementes determinísticas com entropia do SO e coloca a zero o estado intermediário na liberação de recursos.

Todos os segredos vivem dentro dos contêineres `Zeroizing` e CI ejercita as ligações do PQClean em todas as plataformas suportadas.

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

1. **Agrega a dependência** às caixas que estão fora da raiz do espaço de trabalho:

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. **Selecione o conjunto correto** nos pontos de chamada. Para o trabalho inicial do handshake híbrido, use `MlKemSuite::MlKem768` e `MlDsaSuite::MlDsa65`.

3. **Deriva chaves com etiquetas.** Usa `HkdfDomain::soranet("KEM/1")` (e equivalentes) para que o encadenamento de transcrições siga sendo determinístico entre nós.

4. **Usa el RNG hedged** ao mostrar segredos de respaldo:

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

O handshake central do SoraNet e os ajudantes de blindagem do CID (`iroha_crypto::soranet`) consomem essas utilidades diretamente, o que significa que as caixas downstream herdaram as implementações incorretas sem enlaçar ligações PQClean por si mesmas.

## Checklist de validação

-`cargo test -p soranet_pq --offline`
-`cargo fmt --package soranet_pq`
- Audite os exemplos de uso do README (`crates/soranet_pq/README.md`)
- Atualiza o documento de design do handshake de SoraNet ao carregar os híbridos
---
lang: es
direction: ltr
source: docs/portal/docs/soranet/pq-primitives.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identificación: pq-primitivas
título: Primitivos pos-cuánticos de SoraNet
sidebar_label: Primitivos PQ
descripción: Visao general do crate `soranet_pq` y de como o handshake do SoraNet consome helpers ML-KEM/ML-DSA.
---

:::nota Fuente canónica
Esta página espelha `docs/source/soranet/pq_primitives.md`. Mantenha ambas como copias sincronizadas.
:::

La caja `soranet_pq` contiene bloques poscuánticos que son todo relé, cliente y componentes de herramientas de confianza de SoraNet. Se componen de suites Kyber (ML-KEM) y Dilithium (ML-DSA) compatibles con PQClean y ayudantes adicionales de HKDF y RNG hedged amigaveis al protocolo para que todas las superficies compartilen implementadas idénticas.

## O que vem em `soranet_pq`

- **ML-KEM-512/768/1024:** geracao deterministica de chaves, helpers de encapsulation e decapsulation com propagacao de erros em tempo constante.
- **ML-DSA-44/65/87:** assinatura/verificacao separadas para transcricoes com separacao de dominio.
- **HKDF con rotulo:** `derive_labeled_hkdf` agrega espacio de nombres a cada derivación con la estación de apretón de manos (`DH/es`, `KEM/1`, ...) para que transcricos hibridas fiquem sin colisao.
- **Aleatoriedade hedged:** `hedged_chacha20_rng` mistura seeds deterministicas com entropia do SO e zera o estado intermediario ao descartar.

Todos los secretos ficam dentro de contenedores `Zeroizing` y un CI ejercitan los enlaces PQClean en todas las plataformas soportadas.```rust
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

1. **Adición a dependencia** a crates fora da raiz do workspace:

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. **Seleccione a suite correta** nos pontos de chamada. Para el trabajo inicial del protocolo de enlace hibrido, utilice `MlKemSuite::MlKem768` e `MlDsaSuite::MlDsa65`.

3. **Derive chaves com rotulos.** Use `HkdfDomain::soranet("KEM/1")` (e similar) para que o encadeamento de transcricoes fique determinístico entre nodos.

4. **Utilice RNG cubierto** para mostrar secretos de respaldo:

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

El apretón de manos central de SoraNet y los ayudantes de cegamiento de CID (`iroha_crypto::soranet`) pueden ser estas utilidades directamente, o que significa que las cajas aguas abajo herdam como implementaciones de mesmas sin vincular enlaces PQClean por cuenta propia.

## Lista de verificación de validación

- `cargo test -p soranet_pq --offline`
- `cargo fmt --package soranet_pq`
- Auditoría de ejemplos de uso sin README (`crates/soranet_pq/README.md`)
- Actualizar el documento de diseño del protocolo de enlace de SoraNet cuando se conectan los híbridos
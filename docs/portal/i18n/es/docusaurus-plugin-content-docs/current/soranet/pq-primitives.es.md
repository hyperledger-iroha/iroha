---
lang: es
direction: ltr
source: docs/portal/docs/soranet/pq-primitives.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identificación: pq-primitivas
título: Primitivas poscuánticas de SoraNet
sidebar_label: Primitivas PQ
descripción: Resumen del crate `soranet_pq` y de como el handshake de SoraNet consume ayudantes ML-KEM/ML-DSA.
---

:::nota Fuente canónica
Esta página refleja `docs/source/soranet/pq_primitives.md`. Mantenga ambas versiones sincronizadas hasta que el conjunto de documentación heredado se retire.
:::

El crate `soranet_pq` contiene los bloques poscuanticos en los que confian todos los relés, clientes y componentes de herramientas de SoraNet. Envuelve las suites Kyber (ML-KEM) y Dilithium (ML-DSA) respaldadas por PQClean y agrega helpers de HKDF y RNG hedged aptos para el protocolo para que todas las superficies compartan implementaciones idénticas.

## Que incluye `soranet_pq`

- **ML-KEM-512/768/1024:** generación determinística de claves, encapsulación y decapsulación con propagación de errores en tiempo constante.
- **ML-DSA-44/65/87:** firmado/verificacion separada para transcripciones con separacion de dominio.
- **HKDF etiquetado:** `derive_labeled_hkdf` agrega namespace a cada derivacion con la etapa del handshake (`DH/es`, `KEM/1`, ...) para que las transcripciones hibridas no colisionen.
- **Aleatoriedad hedged:** `hedged_chacha20_rng` mezcla semillas determinísticas con entropía del SO y pone a cero el estado intermedio al liberar recursos.Todos los secretos viven dentro de los contenedores `Zeroizing` y CI ejercita los enlaces de PQClean en todas las plataformas soportadas.

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

## Como consumirlo

1. **Agrega la dependencia** a crates que están fuera de la raíz del espacio de trabajo:

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. **Selecciona la suite correcta** en los puntos de llamada. Para el trabajo inicial del handshake hibrido, usa `MlKemSuite::MlKem768` y `MlDsaSuite::MlDsa65`.

3. **Deriva claves con etiquetas.** Usa `HkdfDomain::soranet("KEM/1")` (y equivalentes) para que el encadenamiento de transcripciones siga siendo determinístico entre nodos.

4. **Usa el RNG hedged** al muestrear secretos de respaldo:

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

El handshake central de SoraNet y los helpers de blindeado de CID (`iroha_crypto::soranet`) consumen estas utilidades directamente, lo que significa que los crates downstream heredan las mismas implementaciones sin enlazar vinculaciones PQClean por si mismos.

## Lista de verificación de validación

- `cargo test -p soranet_pq --offline`
- `cargo fmt --package soranet_pq`
- Audita los ejemplos de uso del README (`crates/soranet_pq/README.md`)
- Actualiza el documento de diseño del handshake de SoraNet cuando lleguen los híbridos
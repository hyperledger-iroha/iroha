---
lang: es
direction: ltr
source: docs/portal/docs/soranet/pq-primitives.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identificación: pq-primitivas
título: Primitivos post-quantiques SoraNet
sidebar_label: Primitivas PQ
descripción: Vue d'ensemble du crate `soranet_pq` y de la manera no le handshake SoraNet utiliza los ayudantes ML-KEM/ML-DSA.
---

:::nota Fuente canónica
:::

La caja `soranet_pq` contiene briques post-quantiques sur lesquelles reposent tous les relés, clientes y componentes de herramientas SoraNet. Las suites Kyber (ML-KEM) y Dilithium (ML-DSA) contienen PQClean y un conjunto de ayudantes de HKDF y RNG hedged se adaptan al protocolo para que todas las superficies participen en implementaciones idénticas.

## Este libro está en `soranet_pq`

- **ML-KEM-512/768/1024:** la generación determina la generación de errores, la encapsulación y la decapsulación con propagación de errores en tiempo constante.
- **ML-DSA-44/65/87:** firma/verificación separada con transcripciones en separación de dominio.
- **Etiqueta de HKDF:** `derive_labeled_hkdf` aplica un espacio de nombres a cada derivación mediante la etapa del apretón de manos (`DH/es`, `KEM/1`, ...) después de que las transcripciones híbridas permanecen sin colisión.
- **Aleatoire hedged:** `hedged_chacha20_rng` combine des seeds deterministes avec l'entropie du systeme et zeroise l'etat intermediaire a la destroy.

Todos los secretos viven en los contenidos `Zeroizing` y CI ejercen las fijaciones PQClean en todas las plataformas compatibles.

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
```## Comenta el usuario

1. **Ajoutez la dependence** aux crates en dehors de la racine du workspace:

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. **Seleccione la suite correcta** en los puntos de llamada. Para el trabajo inicial del híbrido de apretón de manos, utilice `MlKemSuite::MlKem768` e `MlDsaSuite::MlDsa65`.

3. **Derivez les cles avec Tags.** Utilice `HkdfDomain::soranet("KEM/1")` (y equivalentes) para que el encadenamiento de transcripciones reste deterministe entre les noeuds.

4. **Utilice el RNG hedged** para echar un vistazo a los secretos de respuesta:

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

El handshake central de SoraNet y los ayudantes de blindage de CID (`iroha_crypto::soranet`) se utilizan directamente para estos usuarios, lo que significa que las cajas posteriores heredan las implementaciones de memes sin enlaces PQClean eux-memes.

## Lista de verificación de validación

- `cargo test -p soranet_pq --offline`
- `cargo fmt --package soranet_pq`
- Audite los ejemplos de uso del README (`crates/soranet_pq/README.md`)
- Mettez a jour le document de conception du handshake SoraNet cuando lleguen los híbridos
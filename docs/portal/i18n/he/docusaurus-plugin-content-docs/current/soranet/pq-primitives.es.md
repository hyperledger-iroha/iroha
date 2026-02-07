---
lang: he
direction: rtl
source: docs/portal/docs/soranet/pq-primitives.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pq-primitives
כותרת: Primitivas poscuanticas de SoraNet
sidebar_label: Primitivas PQ
תיאור: קורות חיים של ארגז `soranet_pq` y de como el לחיצת יד של SoraNet לצרוך עוזרים ML-KEM/ML-DSA.
---

:::שימו לב פואנטה קנוניקה
Esta pagina refleja `docs/source/soranet/pq_primitives.md`. Manten ambas versiones sincronizadas hasta que el conjunto de documentacion heredado se לפרוש.
:::

ארגז `soranet_pq` מכיל חבילות פוסקנטיות ושירותים של ממסרים, לקוחות ורכיבי כלים של SoraNet. Envuelve las suites Kyber (ML-KEM) y Dilithium (ML-DSA) respaldadas por PQClean y agrega helpers de HKDF y RNG hedged aptos para el protocolo para que todas las superficies compartan implementaciones identicas.

## Que incluye `soranet_pq`

- **ML-KEM-512/768/1024:** generacion deterministica de claves, encapsulacion y decapsulacion con propagacion de errores en tiempo constante.
- **ML-DSA-44/65/87:** firmado/verificacion separados para transcripciones con separacion de dominio.
- ** כללי התנהגות HKDF:** `derive_labeled_hkdf` מרחב שמות ולחיצת יד (`DH/es`, `KEM/1`, ...) para que las transcripciones hibridas no colisionen.
- **Aleatoriedad hedged:** `hedged_chacha20_rng` mezcla semillas deterministicas con entropia del SO y pone a cero el estado intermedio al liberar recursos.

Todos los secretos viven dentro de contenedores `Zeroizing` y CI ejercita los bindings de PQClean en todas las plataformas soportadas.

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

1. **Agrega la dependencia** ארגזי que esten fura del work root:

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. **Selecciona la suite correcta** en los puntos de llamada. Para el trabajo inicial del hibrido לחיצת יד, ארה"ב `MlKemSuite::MlKem768` y `MlDsaSuite::MlDsa65`.

3. **Deriva claves con etiquetas.** USA `HkdfDomain::soranet("KEM/1")` (y equivalentes) para que el encadenamiento de transcripciones siga siendo deterministico entre nodos.

4. **Usa el RNG hedged** al muestrear secretos de respaldo:

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

El לחיצת היד המרכזית של SoraNet y los helpers de blindeado de CID (`iroha_crypto::soranet`) consumen estas utilidades directamente, lo que que significa que los cartes downstream heredan las mismas implementaciones sin enlazar bindings PQClean por si mismos.

## רשימת אימות

- `cargo test -p soranet_pq --offline`
- `cargo fmt --package soranet_pq`
- Audita los ejemplos de uso del README (`crates/soranet_pq/README.md`)
- Actualiza el documento de diseno del לחיצת יד של SoraNet cuando lleguen los hybrids
---
lang: es
direction: ltr
source: docs/portal/docs/soranet/pq-primitives.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identificación: pq-primitivas
título: Primitivas poscuánticas de SoraNet
sidebar_label: Primitivas PQ
descripción: `soranet_pq` descripción general de caja کا اور یہ کہ SoraNet handshake ML-KEM/ML-DSA helpers کو کیسے استعمال کرتا ہے۔
---

:::nota Fuente canónica
یہ صفحہ `docs/source/soranet/pq_primitives.md` کی عکاسی کرتا ہے۔ جب تک پرانا conjunto de documentación retirar نہ ہو، دونوں کاپیاں sincronización رکھیں۔
:::

Caja `soranet_pq` ان bloques de construcción post-cuánticos پر مشتمل ہے جن پر ہر SoraNet Relay, cliente اور componente de herramientas انحصار کرتا ہے۔ یہ Kyber respaldado por PQClean (ML-KEM) اور Dilithium (ML-DSA) suites کو wrap کرتا ہے اور HKDF اور hedged RNG helpers فراہم کرتا ہے تاکہ تمام superficies ایک جیسی implementaciones comparten کریں۔

## `soranet_pq` میں کیا شامل ہے

- **ML-KEM-512/768/1024:** generación de claves deterministas, ayudantes de encapsulación y decapsulación y propagación de errores en tiempo constante.
- **ML-DSA-44/65/87:** firma/verificación separadas y transcripciones separadas por dominios o cableadas.
- **Etiquetado HKDF:** `derive_labeled_hkdf` ہر derivación کو etapa de protocolo de enlace (`DH/es`, `KEM/1`, ...) کے ساتھ espacio de nombres دیتا ہے تاکہ transcripciones híbridas sin colisiones رہیں.
- **Aleatoriedad cubierta:** `hedged_chacha20_rng` semillas deterministas کو entropía del sistema operativo vivo کے ساتھ blend کرتا ہے اور drop پر estado intermedio کو zeroize کرتا ہے.تمام secretos `Zeroizing` contenedores کے اندر رہتے ہیں اور CI تمام plataformas compatibles پر enlaces PQClean کو ejercicio کرتی ہے۔

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

## استعمال کیسے کریں

1. **Dependencia شامل کریں** ان crates میں جو raíz del espacio de trabajo سے باہر ہوں:

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. **درست suite منتخب کریں** llamar a sitios پر۔ Apretón de manos híbrido کے لئے `MlKemSuite::MlKem768` اور `MlDsaSuite::MlDsa65` استعمال کریں۔

3. **Las etiquetas کے ساتھ claves derivan کریں۔** `HkdfDomain::soranet("KEM/1")` (اور اس جیسے) استعمال کریں تاکہ nodos de encadenamiento de transcripciones کے درمیان determinista رہے۔

4. **Hedged RNG استعمال کریں** Esta muestra de secretos de respaldo es کر رہے ہوں:

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

SoraNet کا core handshake اور CID blinding helpers (`iroha_crypto::soranet`) ان utilidades کو براہ راست استعمال کرتے ہیں، جس کا مطلب ہے کہ cajas posteriores بغیر Los enlaces PQClean vinculan کئے وہی las implementaciones heredan کرتے ہیں۔

## Lista de verificación de validación

- `cargo test -p soranet_pq --offline`
- `cargo fmt --package soranet_pq`
- Muestras de uso de README کا auditoría کریں (`crates/soranet_pq/README.md`)
- híbridos آنے کے بعد SoraNet handshake design doc اپ ڈیٹ کریں
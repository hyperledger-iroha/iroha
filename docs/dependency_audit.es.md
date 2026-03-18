---
lang: es
direction: ltr
source: docs/dependency_audit.md
status: complete
translator: manual
source_hash: 9746f44dbe6c09433ead16647429ad48bba54ecf9c3271e71fad6cb91a212d65
source_last_modified: "2025-11-02T04:40:28.811390+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Traducción al español de docs/dependency_audit.md (Dependency Audit Summary) -->

//! Resumen de Auditoría de Dependencias

Fecha: 2025‑09‑01

Ámbito: revisión a nivel de workspace de todos los crates declarados en los
archivos Cargo.toml y resueltos en Cargo.lock. Se ejecutó `cargo-audit` contra
la base de datos de RustSec, además de una revisión manual de legitimidad de
crates y de la elección de crates “principales” para los algoritmos.

Herramientas/comandos ejecutados:
- `cargo tree -d --workspace --locked --offline` – inspección de versiones
  duplicadas.
- `cargo audit` – escaneó Cargo.lock en busca de vulnerabilidades conocidas y
  crates marcados como yanked.

Alertas de seguridad encontradas (ahora 0 vulns; 2 avisos):
- crossbeam-channel — RUSTSEC‑2025‑0024  
  - Corregido: se actualizó a `0.5.15` en `crates/ivm/Cargo.toml`.

- códec obsoleto de pprof — RUSTSEC‑2024‑0437  
  - Corregido: se cambió `pprof` a `prost-codec` en
    `crates/iroha_torii/Cargo.toml`.

- ring — RUSTSEC‑2025‑0009  
  - Corregido: se actualizó la pila QUIC/TLS (`quinn 0.11`, `rustls 0.23`,
    `tokio-rustls 0.26`) y se actualizó la pila WS a
    `tungstenite/tokio-tungstenite 0.24`. Se forzó el lock a
    `ring 0.17.12` con `cargo update -p ring --precise 0.17.12`.

Alertas restantes: ninguna. Avisos restantes: `backoff` (sin mantenimiento),
`derivative` (sin mantenimiento).

Evaluación de legitimidad y de crates “principales” (resumen):
- Hashing: `sha2` (RustCrypto), `blake2` (RustCrypto), `tiny-keccak`
  (ampliamente usado) — elecciones canónicas.
- AEAD/simétrico: `aes-gcm`, `chacha20poly1305`, traits `aead` (RustCrypto) —
  canónicos.
- Firmas/ECC: `ed25519-dalek`, `x25519-dalek` (proyecto dalek), `k256`
  (RustCrypto), `secp256k1` (bindings libsecp) — todos legítimos; se recomienda
  preferir una única pila secp256k1 (`k256` en Rust puro o `secp256k1` con
  libsecp) para reducir superficie.
- BLS12‑381/ZK: `blstrs`, familia `halo2_*` — ampliamente usados en ecosistemas
  ZK en producción; legítimos.
- PQ: `pqcrypto-dilithium`, `pqcrypto-traits` — crates de referencia
  legítimos.
- TLS: `rustls`, `tokio-rustls`, `hyper-rustls` — pila TLS moderna canónica
  en Rust.
- Noise: `snow` — implementación canónica.
- Serialización: `parity-scale-codec` es el códec canónico para SCALE. Serde
  se ha eliminado de las dependencias de producción en todo el workspace; los
  derives y writers de Norito cubren todos los paths en tiempo de ejecución.
  Cualquier referencia residual a Serde vive en documentación histórica,
  scripts de guardia o allowlists de tests.
- FFI/libs: `libsodium-sys-stable`, `openssl` — legítimos; para paths de
  producción se prefiere Rustls frente a OpenSSL (el código actual ya lo hace).
- pprof 0.13.0 (crates.io) — fix upstream integrado; usar la release oficial
  con `prost-codec` + frame‑pointer para evitar el códec obsoleto.

Recomendaciones:
- Abordar los avisos:
  - Considerar reemplazar `backoff` por `retry`/`futures-retry` o por un helper
    local de backoff exponencial.
  - Sustituir derive de `derivative` por impls manuales o por `derive_more`
    cuando proceda.
- Medio plazo: unificar sobre `k256` o `secp256k1` cuando sea posible para
  reducir implementaciones duplicadas (mantener ambas solo si es estrictamente
  necesario).
- Medio plazo: revisar la procedencia de `poseidon-primitives 0.2.0` para uso
  en ZK; si es viable, alinearse con una implementación Poseidon nativa de
  Arkworks/Halo2 para minimizar ecosistemas paralelos.

Notas:
- `cargo tree -d` muestra las versiones mayores duplicadas esperadas
  (`bitflags` 1/2, múltiples `ring`); esto no es en sí un riesgo de seguridad,
  pero aumenta la superficie de build.
- No se observaron crates tipo typosquat; todos los nombres y orígenes apuntan
  a crates conocidos del ecosistema o a miembros internos del workspace.
- Experimental: se añadió el feature `iroha_crypto` `bls-backend-blstrs` para
  comenzar la migración de BLS a un backend basado sólo en `blstrs` (elimina la
  dependencia de arkworks cuando está activado). El valor por defecto sigue
  siendo `w3f-bls` para evitar cambios de comportamiento/codificación. Plan de
  alineación:
  - Normalizar la serialización de la clave secreta a la salida canónica de
    32 bytes little‑endian que entienden tanto `w3f-bls` como `blstrs`
  - Exponer un wrapper explícito para compresión de claves públicas reutilizando
    `blstrs::G1Affine::to_compressed` y añadiendo una comprobación de
    coherencia frente a la codificación de w3f para garantizar bytes en
    el wire idénticos.
  - Añadir fixtures de round‑trip en
    `crates/iroha_crypto/tests/bls_backend_compat.rs` que derivan claves una
    sola vez y aseguran la igualdad entre ambos backends, cubriendo
    `SecretKey`, `PublicKey` y agregación de firmas.
  - Proteger el nuevo backend detrás de `bls-backend-blstrs` en CI, pero
    mantener las pruebas de coherencia ejecutándose también para el backend
    por defecto, de modo que se detecten regresiones antes de cambiar de
    backend.

Seguimiento (trabajos propuestos):
- Mantener los guardarraíles Serde en CI
  (`scripts/check_no_direct_serde.sh`, `scripts/deny_serde_json.sh`) para
  impedir introducir nuevos usos de Serde en paths de producción.

Tests ejecutados para esta auditoría:
- 

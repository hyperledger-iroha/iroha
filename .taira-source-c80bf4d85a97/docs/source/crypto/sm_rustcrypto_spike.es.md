---
lang: es
direction: ltr
source: docs/source/crypto/sm_rustcrypto_spike.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1f133d9489c4bcfae2212e6c5dc098f39c3dea3e5cd42855ba76e8c9b73b4d03
source_last_modified: "2026-01-03T18:07:57.103009+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! Notas para el pico de integración de RustCrypto SM.

# Notas de pico de RustCrypto SM

## Objetivo
Valide que la introducción de las cajas `sm2`, `sm3` e `sm4` de RustCrypto (más `rfc6979`, `ccm`, `gcm`) como dependencias opcionales se compile limpiamente en el `iroha_crypto` y produce tiempos de construcción aceptables antes de conectar el indicador de función al espacio de trabajo más amplio.

## Mapa de dependencia propuesto

| Caja | Versión sugerida | Características | Notas |
|-------|-------------------|----------|-------|
| `sm2` | `0.13` (RustCrypto/firmas) | `std` | Depende de `elliptic-curve`; verifique que MSRV coincida con el espacio de trabajo. |
| `sm3` | `0.5.0-rc.1` (RustCrypto/hashes) | predeterminado | La API es paralela a `sha2` y se integra con los rasgos `digest` existentes. |
| `sm4` | `0.5.1` (RustCrypto/cifrados de bloque) | predeterminado | Funciona con rasgos de cifrado; Los envoltorios de AEAD se aplazaron para un aumento posterior. |
| `rfc6979` | `0.4` | predeterminado | Reutilización para derivación determinista nonce. |

*Las versiones reflejan los lanzamientos actuales a partir de 2024-12; confirme con `cargo search` antes de aterrizar.*

## Cambios manifiestos (borrador)

```toml
[features]
sm = ["dep:sm2", "dep:sm3", "dep:sm4", "dep:rfc6979"]

[dependencies]
sm2 = { version = "0.13", optional = true, default-features = false, features = ["std"] }
sm3 = { version = "0.5.0-rc.1", optional = true }
sm4 = { version = "0.5.1", optional = true }
rfc6979 = { version = "0.4", optional = true, default-features = false }
```

Seguimiento: fije `elliptic-curve` para que coincida con las versiones que ya están en `iroha_crypto` (actualmente `0.13.8`).

## Lista de verificación de picos
- [x] Agregar dependencias y funciones opcionales a `crates/iroha_crypto/Cargo.toml`.
- [x] Cree el módulo `signature::sm` detrás de `cfg(feature = "sm")` con estructuras de marcador de posición para confirmar el cableado.
- [x] Ejecute `cargo check -p iroha_crypto --features sm` para confirmar la compilación; registrar el tiempo de compilación y el recuento de nuevas dependencias (`cargo tree --features sm`).
- [x] Confirme la postura estándar únicamente con `cargo check -p iroha_crypto --features sm --locked`; Las compilaciones `no_std` ya no son compatibles.
- [x] Resultados del archivo (tiempos, delta del árbol de dependencia) en `docs/source/crypto/sm_program.md`.

## Observaciones para capturar
- Tiempo de compilación adicional frente a la línea base.
- Impacto del tamaño binario (si se puede medir) con `cargo builtinsize`.
- Cualquier MSRV o conflicto de funciones (por ejemplo, con versiones menores de `elliptic-curve`).
- Advertencias emitidas (código inseguro, activación const-fn) que pueden requerir parches ascendentes.

## Elementos pendientes
- Espere la aprobación de Crypto WG antes de inflar el gráfico de dependencia del espacio de trabajo.
- Confirme si desea vender cajas para su revisión o confiar en crates.io (es posible que se requieran espejos).
- Coordine la actualización de `Cargo.lock` según `sm_lock_refresh_plan.md` antes de marcar la lista de verificación como completa.
- Utilice `scripts/sm_lock_refresh.sh` una vez que se haya concedido la aprobación para regenerar el archivo de bloqueo y el árbol de dependencia.

## 2025-01-19 Registro de picos
- Se agregaron dependencias opcionales (`sm2 0.13`, `sm3 0.5.0-rc.1`, `sm4 0.5.1`, `rfc6979 0.4`) y el indicador de características `sm` en `iroha_crypto`.
- Módulo `signature::sm` bloqueado para ejercitar las API de cifrado de bloques/hashing durante la compilación.
- `cargo check -p iroha_crypto --features sm --locked` ahora resuelve el gráfico de dependencia pero cancela con el requisito de actualización `Cargo.lock`; La política del repositorio prohíbe las ediciones de archivos de bloqueo, por lo que la ejecución de la compilación permanece pendiente hasta que coordinemos una actualización de bloqueo permitida.## 2026-02-12 Registro de picos
- Se resolvió el bloqueador de archivos de bloqueo anterior (las dependencias ya están capturadas), por lo que `cargo check -p iroha_crypto --features sm --locked` tiene éxito (compilación en frío 7.9 en Mac de desarrollo; repetición incremental 0.23).
- `cargo check -p iroha_crypto --no-default-features --features "std sm" --locked` pasa en 1.0, lo que confirma que la función opcional se compila en configuraciones exclusivas de `std` (no queda ninguna ruta `no_std`).
- El delta de dependencia con la función `sm` habilitada introduce 11 cajas: `base64ct`, `ghash`, `opaque-debug`, `pem-rfc7468`, `pkcs8`, `polyval`, `primeorder`, `sm2`, `sm3`, `sm4` y `sm4-gcm`. (`rfc6979` ya formaba parte del gráfico de referencia).
- Las advertencias de compilación persisten para los asistentes de políticas NEON no utilizados; déjelo como está hasta que el tiempo de ejecución de suavizado de medición vuelva a habilitar esas rutas de código.
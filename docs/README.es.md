---
lang: es
direction: ltr
source: docs/README.md
status: complete
translator: manual
source_hash: 6d050b266fbcc3f0041ec554f89e397e56dc37e5ec3fb093af59e46eb52109e6
source_last_modified: "2025-11-02T04:40:28.809865+00:00"
translation_last_reviewed: 2025-11-14
---

# Documentación de Iroha

Para una introducción en japonés, consulta [`README.ja.md`](./README.ja.md).

Este workspace publica dos líneas de lanzamiento a partir de la misma base de código:
**Iroha 2** (despliegues auto‑gestionados) y **Iroha 3 / SORA Nexus**
(el libro mayor Nexus global único). Ambas reutilizan la misma Iroha Virtual
Machine (IVM) y la misma cadena de herramientas Kotodama, por lo que los
contratos y el bytecode se mantienen portables entre los distintos entornos de
despliegue. Salvo que se indique lo contrario, la documentación aplica a ambas.

En la [documentación principal de Iroha](https://docs.iroha.tech/) encontrarás:

- [Guía de inicio](https://docs.iroha.tech/get-started/)
- [Tutoriales de SDK](https://docs.iroha.tech/guide/tutorials/) para Rust, Python, JavaScript y Java/Kotlin
- [Referencia de la API](https://docs.iroha.tech/reference/torii-endpoints.html)

Libros blancos y especificaciones por versión:

- [Whitepaper de Iroha 2](./source/iroha_2_whitepaper.md): especificación de redes auto‑gestionadas.
- [Whitepaper de Iroha 3 (SORA Nexus)](./source/iroha_3_whitepaper.md): diseño de múltiples carriles y espacios de datos de Nexus.
- [Modelo de datos y especificación ISI (derivados de la implementación)](./source/data_model_and_isi_spec.md): referencia de comportamiento reconstruida a partir del código.
- [Sobres ZK (Norito)](./source/zk_envelopes.md): sobres nativos Norito basados en IPA/STARK y expectativas del verificador.

## Localización

Los stubs de documentación en japonés (`*.ja.*`), hebreo (`*.he.*`), español
(`*.es.*`), portugués (`*.pt.*`), francés (`*.fr.*`), ruso (`*.ru.*`), árabe
(`*.ar.*`) y urdu (`*.ur.*`) viven junto a cada archivo fuente en inglés. Consulta
[`docs/i18n/README.md`](./i18n/README.md) para obtener detalles sobre cómo
generar y mantener traducciones, así como orientación para añadir nuevos idiomas
en el futuro.

## Herramientas

En este repositorio encontrarás documentación sobre las herramientas de Iroha 2:

- [Kagami](../crates/iroha_kagami/README.md)
- Macros [`iroha_derive`](../crates/iroha_derive/) para estructuras de configuración (consulta la característica `config_base`)
- [Pasos de compilación con perfilado](./profile_build.md) para identificar las partes lentas de compilación en `iroha_data_model`

## Referencias del SDK de Swift / iOS

- [Descripción general del SDK de Swift](./source/sdk/swift/index.md): ayudas de pipeline, flags de aceleración y APIs de Connect/WebSocket.
- [Guía rápida de Connect](./connect_swift_ios.md): recorrido basado en el SDK junto con la referencia heredada de CryptoKit.
- [Guía de integración con Xcode](./connect_swift_integration.md): cómo integrar NoritoBridgeKit/Connect en una app, con ChaChaPoly y ayudas para marcos.
- [Guía para contribuidores a la demo en SwiftUI](./norito_demo_contributor.md): cómo ejecutar la demo de iOS contra un nodo Torii local, además de notas de aceleración.
- Ejecuta `make swift-ci` antes de publicar artefactos de Swift o cambios en Connect; verifica la paridad de fixtures, los feeds de paneles y los metadatos `ci/xcframework-smoke:<lane>:device_tag` de Buildkite.

## Norito (códec de serialización)

Norito es el códec de serialización del workspace. No usamos `parity-scale-codec`
(SCALE). Cuando la documentación o los benchmarks comparan con SCALE, es solo
como referencia; todos los caminos de producción usan Norito. Las APIs
`norito::codec::{Encode, Decode}` proporcionan una carga útil Norito sin
cabecera (“bare”) para hashing y eficiencia en la red: sigue siendo Norito, no
SCALE.

Estado actual:

- Codificación/decodificación determinista con una cabecera fija (magic, versión,
  esquema de 16 bytes, compresión, longitud, CRC64 y flags).
- Checksum CRC64-XZ con aceleración seleccionada en tiempo de ejecución:
  - PCLMULQDQ en x86_64 (multiplicación sin acarreo) + reducción Barrett sobre bloques de 32 bytes.
  - PMULL en aarch64 con el mismo plegado.
  - Implementaciones “slicing‑by‑8” y de operaciones bit a bit como rutas portables.
- Pistas de longitud codificada implementadas por los derives y tipos núcleo para reducir asignaciones.
- Buffers de streaming más grandes (64 KiB) y actualización incremental del CRC durante la decodificación.
- Compresión zstd opcional; la aceleración por GPU está protegida por feature flag y se mantiene determinista.
- Selección de ruta adaptativa: `norito::to_bytes_auto(&T)` elige entre sin
  compresión, zstd por CPU o zstd delegado a GPU (cuando está compilado y
  disponible) en función del tamaño de la carga útil y de las capacidades de
  hardware en caché. La selección solo afecta al rendimiento y al byte
  `compression` de la cabecera; la semántica de la carga útil no cambia.

Consulta `crates/norito/README.md` para ver pruebas de paridad, benchmarks y ejemplos de uso.

Nota: Algunos documentos de subsistemas (por ejemplo, aceleración de IVM y circuitos ZK) aún están en evolución. Cuando la funcionalidad no está completa, los archivos indican explícitamente el trabajo pendiente y la dirección prevista.

Notas sobre la codificación del endpoint de estado

- El cuerpo de `/status` en Torii usa Norito por defecto con una carga útil sin cabecera (“bare”) para mayor compacidad. Los clientes deben intentar decodificar con Norito primero.
- Los servidores pueden devolver JSON cuando se solicita; los clientes recurren a JSON si el `content-type` es `application/json`.
- El formato de red es Norito, no SCALE. Las APIs `norito::codec::{Encode,Decode}` se usan para la variante sin cabecera.

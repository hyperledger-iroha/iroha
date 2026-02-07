---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Reporte de paridad GA del Orchestrator SoraFS

La paridad determinística de multi-fetch ahora se rastrea por SDK para que los
Los ingenieros de lanzamiento pueden confirmar que los bytes de carga útil, recibos de fragmentos,
informes de proveedores y resultados de marcador permanecerán alineados entre
implementaciones. Cada arnés consume el paquete multi-proveedor canonico bajo
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/`, que empaqueta el plan SF1,
metadatos del proveedor, instantánea de telemetría y opciones del orquestador.

## Línea base de óxido

- **Comando:** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **Alcance:** Ejecuta el plan `MultiPeerFixture` dos veces vía el orquestador en proceso,
  verificando bytes de carga útil ensamblados, recibos de fragmentos, informes de proveedores y
  resultados del marcador. La instrumentacion tambien rastrea concurrencia pico
  y el tamano efectivo del conjunto de trabajo (`max_parallel x max_chunk_length`).
- **Performance guard:** Cada ejecución debe completarse en 2 s en hardware CI.
- **Techo del conjunto de trabajo:** Con el perfil SF1 el arnés aplica `max_parallel = 3`,
  dando una ventana <= 196608 bytes.

Salida de registro de muestra:

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## Arnés del SDK de JavaScript- **Comando:** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **Alcance:** Reproducir el mismo dispositivo a través de `iroha_js_host::sorafsMultiFetchLocal`,
  comparando cargas útiles, recibos, informes de proveedores e instantáneas del marcador entre
  ejecuciones consecutivas.
- **Guardia de rendimiento:** Cada ejecucion debe finalizar en 2 s; el arnés imprime la
  duracion medida y el techo de bytes reservados (`max_parallel = 3`, `peak_reserved_bytes <= 196608`).

Línea de resumen de ejemplo:

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Arnés Swift SDK

- **Comando:** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **Alcance:** Ejecuta la suite de paridad definida en `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift`,
  reproduciendo el aparato SF1 dos veces a traves del puente Norito (`sorafsLocalFetch`). El arnés
  verificar bytes de carga útil, recibos de fragmentos, informes de proveedores y entradas de marcador usando la
  mismo proveedor de metadatos determinísticos y instantáneas de telemetría que las suites Rust/JS.
- **Bridge bootstrap:** El arnés descomprime `dist/NoritoBridge.xcframework.zip` bajo demanda y carga
  el corte macOS a través de `dlopen`. Si el xcframework falta o no tiene enlaces SoraFS, hace fallback a
  `cargo build -p connect_norito_bridge --release` y linkea contra
  `target/release/libconnect_norito_bridge.dylib`, sin manual de configuración en CI.
- **Performance guard:** Cada ejecución debe finalizar en 2 s en hardware CI; el arnés imprime la
  duracion medida y el techo de bytes reservados (`max_parallel = 3`, `peak_reserved_bytes <= 196608`).

Línea de resumen de ejemplo:

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Arnés de fijaciones de Python- **Comando:** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **Scope:** Ejecuta el wrapper de alto nivel `iroha_python.sorafs.multi_fetch_local` y sus dataclasses
  tipadas para que el accesorio canonico fluya por la misma API que consume los wheels. la prueba
  reconstruye los metadatos del proveedor desde `providers.json`, inyecta la instantánea de telemetría y verifica
  bytes de carga útil, recibos de fragmentos, informes de proveedores y contenido del marcador igual que las
  suites Rust/JS/Swift.
- **Requisito previo:** Ejecuta `maturin develop --release` (o instala la rueda) para que `_crypto` exponga el
  vinculante `sorafs_multi_fetch_local` antes de invocar pytest; el arnés se auto-salta cuando el
  vinculante no está disponible.
- **Performance guard:** El mismo presupuesto <= 2 s que la suite Rust; pytest registra el contenido de
  bytes ensamblados y el resumen de participación de proveedores para el artefacto de liberación.

El release gating debe capturar el resultado resumido de cada arnés (Rust, Python, JS, Swift) para que
el reporte archivado pueda comparar recibos de carga útil y métricas de forma uniforme antes de
promover una construcción. Ejecuta `ci/sdk_sorafs_orchestrator.sh` para correr cada suite de paridad
(Rust, enlaces de Python, JS, Swift) en una sola pasada; los artefactos de CI deben adjuntar el
extracto de log de ese helper mas el `matrix.md` generado (tabla de SDK/estado/duracion) al ticket
comunicado para que los revisores auditen la matriz de paridad sin reejecutar la suite localmente.
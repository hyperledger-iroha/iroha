---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS Informe de paridad de GA de Orchestrator

La paridad determinista de búsqueda múltiple ahora se rastrea por SDK para que los ingenieros de lanzamiento puedan confirmar que
los bytes de carga útil, los recibos de fragmentos, los informes de proveedores y los resultados del marcador permanecen alineados en todos
implementaciones. Cada arnés consume el paquete canónico de múltiples proveedores bajo
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/`, que empaqueta el plan SF1, proveedor
Opciones de metadatos, instantáneas de telemetría y orquestador.

## Línea base de óxido

- **Comando:** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **Alcance:** Ejecuta el plan `MultiPeerFixture` dos veces a través del orquestador en proceso, verificando
  bytes de carga útil ensamblados, recibos de fragmentos, informes de proveedores y resultados del marcador. Instrumentación
  también realiza un seguimiento de la simultaneidad máxima y del tamaño efectivo del conjunto de trabajo (`max_parallel × max_chunk_length`).
- **Protección de rendimiento:** Cada ejecución debe completarse en 2 segundos en el hardware de CI.
- **Techo del conjunto de trabajo:** Con el perfil SF1 el arnés impone `max_parallel = 3`, dando un
  Ventana ≤196608bytes.

Salida de registro de muestra:

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## Arnés del SDK de JavaScript

- **Comando:** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **Alcance:** Reproduce el mismo dispositivo a través de `iroha_js_host::sorafsMultiFetchLocal`, comparando cargas útiles,
  recibos, informes de proveedores e instantáneas del marcador en ejecuciones consecutivas.
- **Guardia de rendimiento:** Cada ejecución debe finalizar en 2 segundos; el arnés imprime la medida
  duración y límite de bytes reservados (`max_parallel = 3`, `peak_reserved_bytes ≤ 196 608`).

Línea de resumen de ejemplo:```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Arnés Swift SDK

- **Comando:** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **Alcance:** Ejecuta el conjunto de paridad definido en `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift`,
  reproduciendo el dispositivo SF1 dos veces a través del puente Norito (`sorafsLocalFetch`). El arnés verifica
  bytes de carga útil, recibos de fragmentos, informes de proveedores y entradas del marcador utilizando el mismo determinista
  Metadatos del proveedor e instantáneas de telemetría como las suites Rust/JS.
- **Bridge bootstrap:** El arnés desempaqueta `dist/NoritoBridge.xcframework.zip` según demanda y carga
  el corte de macOS a través de `dlopen`. Cuando falta el xcframework o le faltan los enlaces SoraFS,
  vuelve a `cargo build -p connect_norito_bridge --release` y se vincula contra
  `target/release/libconnect_norito_bridge.dylib`, por lo que no se requiere configuración manual en CI.
- **Guardia de rendimiento:** Cada ejecución debe finalizar en 2 segundos en el hardware de CI; el arnés imprime el
  duración medida y límite de bytes reservados (`max_parallel = 3`, `peak_reserved_bytes ≤ 196 608`).

Línea de resumen de ejemplo:

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Arnés de fijaciones de Python- **Comando:** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **Alcance:** Ejercita el contenedor `iroha_python.sorafs.multi_fetch_local` de alto nivel y su tipo
  clases de datos para que el dispositivo canónico fluya a través de la misma API a la que llaman los consumidores de ruedas. la prueba
  reconstruye los metadatos del proveedor de `providers.json`, inyecta la instantánea de telemetría y verifica
  bytes de carga útil, recibos de fragmentos, informes de proveedores y contenido del marcador como Rust/JS/Swift
  suites.
- **Requisito previo:** Ejecute `maturin develop --release` (o instale la rueda) para que `_crypto` exponga el
  Enlace `sorafs_multi_fetch_local` antes de invocar pytest; El arnés se salta automáticamente cuando se fija.
  no está disponible.
- **Guardia de rendimiento:** Mismo presupuesto de ≤2 segundos que la suite Rust; pytest registra el recuento de bytes ensamblados
  y resumen de participación del proveedor para el artefacto de lanzamiento.

La activación de la versión debe capturar el resultado resumido de cada arnés (Rust, Python, JS, Swift) para que el
El informe archivado puede diferenciar los recibos de carga útil y las métricas de manera uniforme antes de promover una compilación. correr
`ci/sdk_sorafs_orchestrator.sh` para ejecutar cada suite de paridad (Rust, enlaces de Python, JS, Swift) en
una pasada; Los artefactos de CI deben adjuntar el extracto del registro de ese asistente más el archivo generado.
`matrix.md` (SDK/estado/tabla de duración) al ticket de versión para que los revisores puedan auditar la paridad
Matrix sin volver a ejecutar la suite localmente.
---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Rapport de parité GA SoraFS Orquestador

La paridad determinada por búsqueda múltiple está desactivada gracias al SDK
Los ingenieros de lanzamiento pueden confirmar que los bytes de carga útil, recibos fragmentados,
informes del proveedor y resultados del marcador restent alignés entre
implementaciones. Chaque arnés consomé le paquete multi-proveedor canonique dans
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/`, que reagrupa el plan SF1,
el proveedor de metadatos, la instantánea de telemetría y las opciones del orquestador.

## Línea base de óxido

- **Comando:** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **Alcance:** Ejecute el plan `MultiPeerFixture` dos veces a través del orquestador en proceso,
  para verificar los bytes de carga útil ensamblados, recibos de fragmentos, informes de proveedores y
  resultados del marcador. El traje de instrumentación y la concurrencia de puntas.
  et la taille Effective du work-set (`max_parallel × max_chunk_length`).
- **Guardia de rendimiento:** La ejecución debe finalizar en 2 segundos en el hardware CI.
- **Techo del conjunto de trabajo:** Con el perfil SF1, el arnés impone `max_parallel = 3`,
  donnant une fenêtre ≤ 196608 bytes.

Salida de registro de muestra:

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## Arnés del SDK de JavaScript- **Comando:** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **Alcance:** Dispositivo Rejoue la même a través de `iroha_js_host::sorafsMultiFetchLocal`,
  cargas útiles comparativas, recibos, informes de proveedores e instantáneas del marcador entre
  Ejecuciones consecutivas.
- **Performance guard:** Chaque exécution doit finir en 2 s ; el arnés imprime la
  duración medida y el panel de bytes reservados (`max_parallel = 3`, `peak_reserved_bytes ≤ 196 608`).

Línea de resumen de ejemplo:

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Arnés Swift SDK

- **Comando:** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **Alcance:** Ejecute la suite de parité definida dans `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift`,
  rejouant la luminaria SF1 dos veces a través del puente Norito (`sorafsLocalFetch`). Verificar el arnés
  bytes de carga útil, recibos fragmentados, informes de proveedores y entradas de marcador en uso
  También el proveedor de metadatos determina e instantáneas de telemetría que las suites Rust/JS.
- **Bridge bootstrap:** El arnés descomprime `dist/NoritoBridge.xcframework.zip` a la demanda y carga
  le corte macOS a través de `dlopen`. Cuando xcframework no tiene enlaces SoraFS,
  bascule sur `cargo build -p connect_norito_bridge --release` et se lie à
  `target/release/libconnect_norito_bridge.dylib`, sin manual de configuración en CI.
- **Performance guard:** Chaque exécution doit finir en 2 s sur le hardware CI; el arnés imprime la
  duración medida y el panel de bytes reservados (`max_parallel = 3`, `peak_reserved_bytes ≤ 196 608`).

Línea de resumen de ejemplo:

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Arnés de fijaciones de Python- **Comando:** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **Alcance:** Ejercer el contenedor de alto nivel `iroha_python.sorafs.multi_fetch_local` y sus tipos de clases de datos
  Afin que la fijación canónica pasó por la misma API que les consommateurs de wheel. La prueba reconstruida
  el proveedor de metadatos desde `providers.json`, inyecta la instantánea de telemetría y verifica los bytes de carga útil,
  recibos fragmentados, informes de proveedores y contenido del marcador como las suites Rust/JS/Swift.
- **Requisito previo:** Ejecute `maturin develop --release` (o instale la rueda) para que `_crypto` exponga el enlace
  `sorafs_multi_fetch_local` antes de solicitar pytest; El arnés se salta automáticamente cuando la fijación es indisponible.
- **Guardia de rendimiento:** Même presupuesto ≤ 2 s que la suite Rust; pytest registra el nombre de bytes ensamblados
  y el currículum de participación de los proveedores para el artefacto de liberación.

La puerta de liberación captura el resultado resumido de cada arnés (Rust, Python, JS, Swift) hasta que
le rapport archivé puisse comparer recibos de carga útil y métriques de manière uniforme antes de
Promouvoir un build. Ejecute `ci/sdk_sorafs_orchestrator.sh` para lanzar cada suite de parité
(Rust, enlaces de Python, JS, Swift) y una sola pasada; les artefactos CI doivent joindre l'extrait
log de this helper plus le `matrix.md` generado (tabla SDK/estado/duración) en el ticket de lanzamiento después de que
les reviewers puissent auditer la matrice de parité sans relancer la suite localement.
---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Отчет о паритете GA para SoraFS Orchestrator

Las particiones de búsqueda múltiple determinadas están conectadas al SDK del dispositivo, por ejemplo
ingenieros de lanzamiento могли убедиться, что bytes de carga útil, recibos fragmentados, proveedor
informes y resultados marcador остаются согласованными между реализациями.
Каждый arnés использует канонический paquete multiproveedor из
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/`, который включает план SF1,
metadatos del proveedor, instantánea de telemetría y opciones del orquestador.

## Línea base de óxido

- **Comando:** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **Alcance:** Запускает план `MultiPeerFixture` дважды через en proceso orquestador,
  проверяя собранные bytes de carga útil, recibos de fragmentos, informes de proveedores y resultados
  marcador. La herramienta de configuración permite una gran configuración y eficacia
  размер conjunto de trabajo (`max_parallel × max_chunk_length`).
- **Performance guard:** Каждый прогон должен завершиться за 2 s на hardware CI.
- **Techo del conjunto de trabajo:** Для профиля SF1 arnés применяет `max_parallel = 3`,
  давая окно ≤ 196608 bytes.

Salida de registro de muestra:

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## Arnés del SDK de JavaScript- **Comando:** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **Alcance:** Proyecta tu dispositivo según `iroha_js_host::sorafsMultiFetchLocal`,
  сравнивая cargas útiles, recibos, informes de proveedores e instantáneas del marcador между
  последовательными запусками.
- **Guardia de rendimiento:** Каждый запуск должен завершиться за 2 s; arnés печатает
  измеренную длительность и потолок bytes reservados (`max_parallel = 3`, `peak_reserved_bytes ≤ 196 608`).

Línea de resumen de ejemplo:

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Arnés Swift SDK

- **Comando:** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **Alcance:** Suite de paridad completa de `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift`,
  проигрывая SF1 accesorio дважды через Norito puente (`sorafsLocalFetch`). Arnés проверяет
  bytes de carga útil, recibos de fragmentos, informes de proveedores y marcador de entradas, используя ту же
  Metadatos de proveedores específicos e instantáneas de telemetría, entre otras suites Rust/JS.
- **Puente bootstrap:** Arnés распаковывает `dist/NoritoBridge.xcframework.zip` по требованию и
  Descargue el segmento macOS con `dlopen`. Si xcframework elimina los enlaces SoraFS,
  respaldo en `cargo build -p connect_norito_bridge --release` y enlace с
  `target/release/libconnect_norito_bridge.dylib`, без ручной настройки в CI.
- **Guardia de rendimiento:** Каждый запуск должен завершиться за 2 s на hardware CI; arnés печатает
  измеренную длительность и потолок bytes reservados (`max_parallel = 3`, `peak_reserved_bytes ≤ 196 608`).

Línea de resumen de ejemplo:

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Arnés de fijaciones de Python- **Comando:** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **Alcance:** Проверяет contenedor de alto nivel `iroha_python.sorafs.multi_fetch_local` y его escrito
  clases de datos, qué dispositivos de fijación canónicos utilizan su API, qué es lo que desea
  los consumidores ruedan. Pruebe los metadatos del proveedor en `providers.json`, инжектит
  instantánea de telemetría y muestra bytes de carga útil, recibos de fragmentos, informes de proveedores y
  содержимое marcador так же, как suites Rust/JS/Swift.
- **Requisito previo:** Запустите `maturin develop --release` (o установите rueda), чтобы `_crypto`
  открыл vinculante `sorafs_multi_fetch_local` перед pytest; arnés авто-скипает,
  когда vinculante недоступен.
- **Guardia de rendimiento:** Тот же бюджет ≤ 2 s, что и у Rust suite; pytest логирует число
  Bytes adicionales y proveedores de resumen de datos para el lanzamiento del artefacto.

Liberación de puerta de enlace de salida resumida del arnés (Rust, Python, JS, Swift), чтобы
архивированный отчет мог сравнивать recibos de carga útil y métricas edinoobrazno перед продвижением
construir. Introduzca `ci/sdk_sorafs_orchestrator.sh`, cómo conectar las suites de paridad
(Rust, enlaces de Python, JS, Swift) por ejemplo; Extracto de registro de artefactos de CI должны приложить
из этого helper и сгенерированный `matrix.md` (tabla SDK/status/duration) к lanzamiento de ticket,
Muchos revisores pueden auditar la matriz de paridad según el programa local.
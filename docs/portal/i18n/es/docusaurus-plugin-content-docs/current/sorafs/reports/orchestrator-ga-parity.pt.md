---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Relatorio de paridade GA do Orchestrator SoraFS

Una paridad determinística de búsqueda múltiple ahora y monitoreada por SDK para que los ingenieros de lanzamiento confirmen que
bytes de carga útil, recibos de fragmentos, informes de proveedores y resultados de marcador permanecen alineados entre
implementacoes. Cada arnés contiene o paquete canonico multi-proveedor em
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/`, que empacota o plano SF1, metadatos del proveedor, instantánea de telemetría e
Los opcoes hacen orquestador.

## Óxido base

- **Comando:** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **Escopo:** Ejecuta el plano `MultiPeerFixture` dos veces vía el orquestador en proceso, verificando
  bytes de carga útil montados, recibos de fragmentos, informes de proveedores y resultados de marcador. Una instrumentación
  tambem acompanha a concorrencia de pico e o tamanho efetivo do work-set (`max_parallel x max_chunk_length`).
- **Guardia de rendimiento:** Cada ejecución debe concluir en 2 sin hardware de CI.
- **Set de trabajo techo:** Com o perfil SF1 o arnés aplica `max_parallel = 3`, resultando em uma
  janela <= 196608 bytes.

Ejemplo de dicha de registro:

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## Aprovechar el SDK JavaScript- **Comando:** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **Escopo:** Reproduz o mesmo fixture vía `iroha_js_host::sorafsMultiFetchLocal`, comparando cargas útiles,
  recibos, informes de proveedores e instantáneas del marcador entre ejecuciones consecutivas.
- **Guardia de rendimiento:** Cada ejecución debe finalizar en 2 s; o arnés imprime a duracao medida e o
  teto de bytes reservados (`max_parallel = 3`, `peak_reserved_bytes <= 196608`).

Ejemplo de línea de resumen:

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Aprovechar SDK Swift

- **Comando:** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **Escopo:** Ejecute una suite de paridade definida en `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift`,
  reproduciendo el accesorio SF1 dos veces en el puente Norito (`sorafsLocalFetch`). O arnés verifica bytes de carga útil,
  recibos fragmentados, informes de proveedores y entradas del marcador utilizando una mesma de metadatos determinísticos del proveedor
  instantáneas de telemetría de las suites Rust/JS.
- **Bridge bootstrap:** El arnés descompacta `dist/NoritoBridge.xcframework.zip` sob exige e carrega o slice macOS vía
  `dlopen`. Cuando xcframework está ausente o no hay enlaces de tem SoraFS, faz fallback para
  `cargo build -p connect_norito_bridge --release` y enlace contra `target/release/libconnect_norito_bridge.dylib`,
  Este es el manual de configuración necesario y necesario para CI.
- **Guardia de rendimiento:** Cada ejecución debe terminar en 2 segundos sin hardware de CI; o arnés imprime a duracao medida e o
  teto de bytes reservados (`max_parallel = 3`, `peak_reserved_bytes <= 196608`).

Ejemplo de línea de resumen:

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Arnés de las fijaciones Python- **Comando:** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **Escopo:** Ejercicio de wrapper de alto nivel `iroha_python.sorafs.multi_fetch_local` y sus clases de datos
  Consejos para que el accesorio canónico pase pela mesma API que los consumidores de rueda usan. Oh prueba
  reconstruya los metadatos de un proveedor a partir de `providers.json`, inyecte una instantánea de telemetría y verifique
  bytes de carga útil, recibos de fragmentos, informes de proveedores y contenido del marcador igual que las suites Rust/JS/Swift.
- **Requisito previo:** Ejecute `maturin develop --release` (o instale la rueda) para que `_crypto` exponga o
  vinculante `sorafs_multi_fetch_local` antes de chamar pytest; o arnés se auto-ignora quando o vinculante
  nao estiver disponivel.
- **Guardia de rendimiento:** Mesmo orcamento <= 2 s da suite Rust; pytest registra un contagio de bytes
  montados y el resumen de participación de los proveedores para el artefato de liberación.

La puerta de liberación debe capturar el resultado resumido de cada arnés (Rust, Python, JS, Swift) para que o
Relatorio archivado possa comparar recibos de carga útil y métricas de forma uniforme antes de promover.
um construir. Ejecute `ci/sdk_sorafs_orchestrator.sh` para rodar todas las suites de paridade (Rust, Python
enlaces, JS, Swift) en una única pasada; artefatos de CI devem anexar o trecho de log desse helper
Más el `matrix.md` generado (tabela SDK/status/duration) en el ticket de lanzamiento para que los revisores puedan
Auditar una matriz de paridade sin reejecutar una suite localmente.
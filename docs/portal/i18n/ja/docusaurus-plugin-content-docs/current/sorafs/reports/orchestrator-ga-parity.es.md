---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Reporte de paridad GA del Orchestrator SoraFS

La paridad deterministica de multi-fetch ahora se rastrea por SDK para que los
release engineers puedan confirmar que los bytes de payload, chunk receipts,
provider reports y resultados de scoreboard permanezcan alineados entre
implementaciones. Cada harness consume el bundle multi-provider canonico bajo
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/`, que empaqueta el plan SF1,
provider metadata, telemetry snapshot y opciones del orchestrator.

## Rust Baseline

- **Command:** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **Scope:** Ejecuta el plan `MultiPeerFixture` dos veces via el orchestrator in-process,
  verificando bytes de payload ensamblados, chunk receipts, provider reports y
  resultados de scoreboard. La instrumentacion tambien rastrea concurrencia pico
  y el tamano efectivo del working-set (`max_parallel x max_chunk_length`).
- **Performance guard:** Cada ejecucion debe completar en 2 s en hardware CI.
- **Working set ceiling:** Con el perfil SF1 el harness aplica `max_parallel = 3`,
  dando una ventana <= 196608 bytes.

Sample log output:

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## JavaScript SDK Harness

- **Command:** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **Scope:** Reproduce el mismo fixture via `iroha_js_host::sorafsMultiFetchLocal`,
  comparando payloads, receipts, provider reports y scoreboard snapshots entre
  ejecuciones consecutivas.
- **Performance guard:** Cada ejecucion debe finalizar en 2 s; el harness imprime la
  duracion medida y el techo de bytes reservados (`max_parallel = 3`, `peak_reserved_bytes <= 196608`).

Example summary line:

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Swift SDK Harness

- **Command:** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **Scope:** Ejecuta la suite de paridad definida en `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift`,
  reproduciendo el fixture SF1 dos veces a traves del bridge Norito (`sorafsLocalFetch`). El harness
  verifica bytes de payload, chunk receipts, provider reports y entradas de scoreboard usando la
  misma provider metadata deterministica y telemetry snapshots que las suites Rust/JS.
- **Bridge bootstrap:** El harness descomprime `dist/NoritoBridge.xcframework.zip` bajo demanda y carga
  el slice macOS via `dlopen`. Si el xcframework falta o no tiene bindings SoraFS, hace fallback a
  `cargo build -p connect_norito_bridge --release` y linkea contra
  `target/release/libconnect_norito_bridge.dylib`, sin setup manual en CI.
- **Performance guard:** Cada ejecucion debe terminar en 2 s en hardware CI; el harness imprime la
  duracion medida y el techo de bytes reservados (`max_parallel = 3`, `peak_reserved_bytes <= 196608`).

Example summary line:

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Python Bindings Harness

- **Command:** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **Scope:** Ejecuta el wrapper de alto nivel `iroha_python.sorafs.multi_fetch_local` y sus dataclasses
  tipadas para que el fixture canonico fluya por la misma API que consumen los wheels. El test
  reconstruye provider metadata desde `providers.json`, inyecta la telemetry snapshot y verifica
  bytes de payload, chunk receipts, provider reports y contenido del scoreboard igual que las
  suites Rust/JS/Swift.
- **Pre-req:** Ejecuta `maturin develop --release` (o instala el wheel) para que `_crypto` exponga el
  binding `sorafs_multi_fetch_local` antes de invocar pytest; el harness se auto-salta cuando el
  binding no esta disponible.
- **Performance guard:** El mismo presupuesto <= 2 s que la suite Rust; pytest registra el conteo de
  bytes ensamblados y el resumen de participacion de providers para el artefacto de release.

El release gating debe capturar el summary output de cada harness (Rust, Python, JS, Swift) para que
el reporte archivado pueda comparar receipts de payload y metricas de forma uniforme antes de
promover un build. Ejecuta `ci/sdk_sorafs_orchestrator.sh` para correr cada suite de paridad
(Rust, Python bindings, JS, Swift) en una sola pasada; los artefactos de CI deben adjuntar el
extracto de log de ese helper mas el `matrix.md` generado (tabla de SDK/estado/duracion) al ticket
release para que los reviewers auditen la matriz de paridad sin reejecutar la suite localmente.

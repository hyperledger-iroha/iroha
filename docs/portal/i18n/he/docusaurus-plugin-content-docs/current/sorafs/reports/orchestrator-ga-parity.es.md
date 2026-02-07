---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Reporte de paridad GA del Orchestrator SoraFS

La paridad deterministica de multi-fetch ahora se rastrea por SDK para que los
מהנדסי שחרור puedan confirmar que los bytes de payload, chunk receipts,
ספק דוחות y resultados de scoreboard permanezcan alineados entre
implementaciones. רתמת Cada לצרוך el חבילה רב ספקי canonico bajo
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/`, que empaqueta el plan SF1,
מטא נתונים של ספק, תמונת מצב של טלמטריה ואופציות של מתזמר.

## קו בסיס חלודה

- **פקודה:** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **היקף:** Ejecuta el plan `MultiPeerFixture` פעולות באמצעות מתזמר בתהליך,
  verificando bytes de payload ensamblados, חתיכות קבלות, דוחות ספקים
  תוצאות לוח התוצאות. La instrumentacion tambien rastrea concurrencia pico
  y el tamano efectivo del working-set (`max_parallel x max_chunk_length`).
- **משמר ביצועים:** Cada ejecucion debe completar en 2 s en hardware CI.
- **תקרת סט עבודה:** Con el perfil SF1 el harness aplica `max_parallel = 3`,
  dando una ventana <= 196608 בתים.

פלט יומן לדוגמה:

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## רתמת JavaScript SDK

- **פקודה:** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **היקף:** שחזור מתקן el mismo דרך `iroha_js_host::sorafsMultiFetchLocal`,
  השוואת מטענים, קבלות, דוחות ספקים ותמונות מצב של לוח התוצאות
  ejecuciones consecutivas.
- **שומר ביצועים:** Cada ejecucion debe finalizar in 2 s; el לרתום imprime la
  duracion medida y el techo de bytes reservados (`max_parallel = 3`, `peak_reserved_bytes <= 196608`).

שורת סיכום לדוגמה:

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## רתמת Swift SDK

- **פקודה:** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **היקף:** Ejecuta la suite de paridad definida en `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift`,
  שחזור מתקן SF1 dos veces a traves del bridge Norito (`sorafsLocalFetch`). אל רתמה
  אימות בתים של מטען, קבלות של נתחים, דוחות ספקים ולוח תוצאות תוצאות usando la
  ספק מטא נתונים קביעת טלמטריה צילומי מצב que las suites Rust/JS.
- **רצועת אתחול הגשר:** El herness descomprime `dist/NoritoBridge.xcframework.zip` bajo demanda y carga
  el slice macOS דרך `dlopen`. Si el xcframework falta o no tiene bindings SoraFS, have fallback a
  `cargo build -p connect_norito_bridge --release` y linkea contra
  `target/release/libconnect_norito_bridge.dylib`, מדריך להגדרה sin en CI.
- **משמר ביצועים:** Cada ejecucion debe terminar en 2 s en hardware CI; el לרתום imprime la
  duracion medida y el techo de bytes reservados (`max_parallel = 3`, `peak_reserved_bytes <= 196608`).

שורת סיכום לדוגמה:

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## רתמת Python Bindings- **פקודה:** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **היקף:** Ejecuta el wrapper de alto nivel `iroha_python.sorafs.multi_fetch_local` y sus dataclasses
  tipadas para que el fixture canonico fluya por la misma API que consumen los wheels. מבחן אל
  reconstruye ספק metadata desde `providers.json`, inyecta la telemetry snapshot y verifica
  מטען בתים, קבלות של נתחים, דוחות ספקים ותוכן לוח התוצאות
  סוויטות Rust/JS/Swift.
- **דרישה מוקדמת:** Ejecuta `maturin develop --release` (o instala el wheel) para que `_crypto` exponga el
  מחייב `sorafs_multi_fetch_local` antes de invocar pytest; el לרתום se auto-salta cuando el
  מחייב no esta disponible.
- **משמר ביצועים:** El mismo presupuesto <= 2 s que la suite Rust; pytest registra el conteo de
  bytes ensamblados y el resumen de participacion de providers para el artefacto de release.

לשחרור שערים ללכוד את פלט תקציר הרתמה (Rust, Python, JS, Swift)
el reporte archivado pueda השוואת קבלות של מטען y metricas de forma uniforme antes de
מקדם un build. Ejecuta `ci/sdk_sorafs_orchestrator.sh` para correr cada suite de paridad
(Rust, Python bindings, JS, Swift) en una sola pasada; los artefactos de CI deben adjuntar el
extracto de log de ese helper mas el `matrix.md` generado (tabla de SDK/estado/duracion) al ticket
release para que los reviewers auditen la matriz de paridad sin reejecutar la suite localmente.
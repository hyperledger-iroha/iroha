---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Отчет о паритете GA для SoraFS Orchestrator

Детерминированный multi-fetch паритет теперь отслеживается по каждому SDK, чтобы
release engineers могли убедиться, что bytes payload, chunk receipts, provider
reports и результаты scoreboard остаются согласованными между реализациями.
Каждый harness использует канонический multi-provider bundle из
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/`, который включает план SF1,
provider metadata, telemetry snapshot и опции orchestrator.

## Rust Baseline

- **Command:** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **Scope:** Запускает план `MultiPeerFixture` дважды через in-process orchestrator,
  проверяя собранные payload bytes, chunk receipts, provider reports и результаты
  scoreboard. Инструментация также отслеживает пиковую конкуренцию и эффективный
  размер working-set (`max_parallel × max_chunk_length`).
- **Performance guard:** Каждый прогон должен завершиться за 2 s на CI hardware.
- **Working set ceiling:** Для профиля SF1 harness применяет `max_parallel = 3`,
  давая окно ≤ 196 608 bytes.

Sample log output:

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## JavaScript SDK Harness

- **Command:** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **Scope:** Проигрывает ту же fixture через `iroha_js_host::sorafsMultiFetchLocal`,
  сравнивая payloads, receipts, provider reports и scoreboard snapshots между
  последовательными запусками.
- **Performance guard:** Каждый запуск должен завершиться за 2 s; harness печатает
  измеренную длительность и потолок reserved bytes (`max_parallel = 3`, `peak_reserved_bytes ≤ 196 608`).

Example summary line:

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Swift SDK Harness

- **Command:** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **Scope:** Запускает parity suite из `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift`,
  проигрывая SF1 fixture дважды через Norito bridge (`sorafsLocalFetch`). Harness проверяет
  payload bytes, chunk receipts, provider reports и entries scoreboard, используя ту же
  детерминированную provider metadata и telemetry snapshots, что и suites Rust/JS.
- **Bridge bootstrap:** Harness распаковывает `dist/NoritoBridge.xcframework.zip` по требованию и
  загружает macOS slice через `dlopen`. Когда xcframework отсутствует или не содержит SoraFS bindings,
  выполняется fallback на `cargo build -p connect_norito_bridge --release` и линковка с
  `target/release/libconnect_norito_bridge.dylib`, без ручной настройки в CI.
- **Performance guard:** Каждый запуск должен завершиться за 2 s на CI hardware; harness печатает
  измеренную длительность и потолок reserved bytes (`max_parallel = 3`, `peak_reserved_bytes ≤ 196 608`).

Example summary line:

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Python Bindings Harness

- **Command:** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **Scope:** Проверяет high-level wrapper `iroha_python.sorafs.multi_fetch_local` и его typed
  dataclasses, чтобы каноническая fixture проходила через ту же API, что вызывают
  consumers wheel. Тест пересобирает provider metadata из `providers.json`, инжектит
  telemetry snapshot и проверяет payload bytes, chunk receipts, provider reports и
  содержимое scoreboard так же, как suites Rust/JS/Swift.
- **Pre-req:** Запустите `maturin develop --release` (или установите wheel), чтобы `_crypto`
  открыл binding `sorafs_multi_fetch_local` перед pytest; harness авто-скипает,
  когда binding недоступен.
- **Performance guard:** Тот же бюджет ≤ 2 s, что и у Rust suite; pytest логирует число
  собранных bytes и summary участия providers для release artefact.

Release gating должен захватывать summary output каждого harness (Rust, Python, JS, Swift), чтобы
архивированный отчет мог сравнивать payload receipts и метрики единообразно перед продвижением
build. Запустите `ci/sdk_sorafs_orchestrator.sh`, чтобы выполнить все parity suites
(Rust, Python bindings, JS, Swift) за один проход; CI artefacts должны приложить log excerpt
из этого helper и сгенерированный `matrix.md` (таблица SDK/status/duration) к release ticket,
чтобы reviewers могли аудитить parity matrix без повторного локального прогона.

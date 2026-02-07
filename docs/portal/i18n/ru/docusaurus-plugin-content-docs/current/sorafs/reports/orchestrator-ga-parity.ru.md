---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Отчет о паритете GA для SoraFS Orchestrator

Определенный паритет множественной выборки теперь отслеживается для каждого SDK, чтобы
инженеры по выпуску могли быть уверены, что полезная нагрузка в байтах, квитанции о кусках, поставщик
Отчеты и табло результатов остаются согласованными между поставками.
В каждом жгуте используется традиционный пакет с несколькими поставщиками услуг.
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/`, который включает план SF1,
Метаданные поставщика, снимок телеметрии и оркестратор опций.

## Базовая линия ржавчины

- **Команда:** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **Объем:** Запускает план `MultiPeerFixture` через внутрипроцессный оркестратор,
  проверка собранных байтов полезной нагрузки, квитанций фрагментов, отчетов поставщика и результатов
  табло. Инструментарий также отслеживает пиковую конкуренцию и эффективность.
  размер рабочего комплекта (`max_parallel × max_chunk_length`).
- **Защита производительности:** Каждый прогон должен проходить за 2 секунды на оборудовании CI.
- **Рабочий комплект потолка:** Для профиля SF1 жгут проводов оформлен `max_parallel = 3`,
  вспомогательное окно ≤ 196608 байт.

Пример вывода журнала:

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## Использование JavaScript SDK

- **Команда:** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **Scope:** Проигрывает то же самое приспособление через `iroha_js_host::sorafsMultiFetchLocal`,
  сравнение полезных данных, квитанций, отчетов поставщиков и снимков табло между
  последующими запусками.
- **Защита производительности:** Каждый запуск должен завершаться за 2 с; жгут печатает
  размерную длительность и верхний предел зарезервированных байт (`max_parallel = 3`, `peak_reserved_bytes ≤ 196 608`).

Пример сводной строки:

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Обвязка Swift SDK

- **Команда:** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **Объем:** Запускает пакет контроля четности из `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift`,
  проигрывание прибора SF1 случайно через мост Norito (`sorafsLocalFetch`). ремень безопасности
  байты полезной нагрузки, квитанции фрагментов, отчеты провайдера и табло записей, используя ту же
  детерминированную метаданные провайдера и снимки телеметрии, что и пакеты Rust/JS.
- **Bridge bootstrap:** Жгут распаковывает `dist/NoritoBridge.xcframework.zip` по инструментам и
  загружает слайс macOS через `dlopen`. Когда xcframework отсутствует или не содержит привязок SoraFS,
  резерв производительности на `cargo build -p connect_norito_bridge --release` и ссылка с
  `target/release/libconnect_norito_bridge.dylib`, без ручной настройки в CI.
- **Защита производительности:** Каждый запуск должен выполняться за 2 секунды на оборудовании CI; жгут печатает
  размерную длительность и верхний предел зарезервированных байт (`max_parallel = 3`, `peak_reserved_bytes ≤ 196 608`).

Пример сводной строки:

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Привязка Python- **Команда:** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **Scope:** Проверяет высокоуровневую обертку `iroha_python.sorafs.multi_fetch_local` и его набранную
  классы данных, чтобы каноническая фикстура проходила через тот же API, что вызывает
  потребительское колесо. Тест пересобирает метаданные провайдера из `providers.json`, инжектит
  снимок телеметрии и, наконец, байты полезной нагрузки, квитанции о фрагментах, отчеты поставщика и
  Рейтинговое табло так же, как и пакеты Rust/JS/Swift.
- **Предварительное требование:** Запустите `maturin develop --release` (или установите колесо), чтобы `_crypto`.
  открыл привязку `sorafs_multi_fetch_local` перед pytest; упряжь авто-скипает,
  когда привязка недоступен.
- **Защита производительности:** Тот же бюджет ≤ 2 с, что и в Rust Suite; pytest логирует число
  собранных байтов и сводных провайдеров участия для выпуска артефакта.

Release gating должен захватывать сводный вывод каждого компонента (Rust, Python, JS, Swift), чтобы
архивированный отчет мог сравнивать поступления полезной нагрузки и метрики одинаково перед продвижением
строить. Запустите `ci/sdk_sorafs_orchestrator.sh`, чтобы настроить все пакеты контроля четности.
(Rust, привязки Python, JS, Swift) за один проход; Артефакты CI должны приложить выдержку журнала
из этого помощника и сгенерированного `matrix.md` (таблица SDK/status/duration) для выпуска билета,
чтобы рецензенты могли проверить матрицу паритета без повторного локального прогона.
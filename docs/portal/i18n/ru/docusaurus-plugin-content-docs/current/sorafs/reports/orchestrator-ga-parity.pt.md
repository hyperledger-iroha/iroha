---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Relatorio de paridade GA do Orchestrator SoraFS

Определенная степень множественной выборки перед отправкой и мониторингом с помощью SDK для того, чтобы инженеры подтвердили выпуск
байты полезной нагрузки, квитанции о порциях, отчеты провайдера и постоянные результаты табло между ними
реализация. Cada использует consome или пакет Canonico с несколькими поставщиками
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/`, que empacota o plano SF1, метаданные поставщика, снимок телеметрии e
opcoes делают оркестратор.

## Базовая ржавчина

- **Командо:** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **Эскопо:** Выполнение плана `MultiPeerFixture` в процессе работы оркестратора, проверено.
  байты полезной нагрузки, квитанции о фрагментах, отчеты провайдера и результаты на табло. Инструмент
  Тамбем сопровождает согласование пико и таманьхо эфетиво рабочего набора (`max_parallel x max_chunk_length`).
- **Защита производительности:** При выполнении 2-го сеанса без аппаратного обеспечения CI.
- **Рабочий комплект потолка:** Com или perfil SF1 или жгут, соответствующий `max_parallel = 3`, resultando em uma
  Джанела <= 196608 байт.

Пример журнала:

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## Использование JavaScript в SDK

- **Командо:** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **Эскопо:** Воспроизведение основного устройства через `iroha_js_host::sorafsMultiFetchLocal`, сравнение полезных нагрузок,
  квитанции, отчеты поставщика и снимки экрана делаются на табло для последовательных действий.
- **Охранник производительности:** Время выполнения финала составляет 2 с; o использовать imprime a duracao medida e o
  Зарезервировано четыре байта (`max_parallel = 3`, `peak_reserved_bytes <= 196608`).

Пример резюме:

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Использование SDK Swift

- **Командо:** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **Эскопо:** Выполните набор параметров, определенный в `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift`,
  воспроизвести приспособление SF1 duas vezes pelo мост Norito (`sorafsLocalFetch`). Используйте проверенные байты полезной нагрузки,
  квитанции о порциях, отчеты поставщиков и входные данные на табло с использованием детерминированных метаданных поставщика сообщений.
  снимки телеметрии в пакетах Rust/JS.
- **Загрузочный мост:** Используйте decompacta `dist/NoritoBridge.xcframework.zip`, чтобы потребовать и загрузить или срезать macOS через
  `dlopen`. Когда или xcframework отключается или нет привязок тем SoraFS, резервный вариант
  `cargo build -p connect_norito_bridge --release` и ссылка против `target/release/libconnect_norito_bridge.dylib`,
  Есть небольшое руководство по настройке и необходимость в CI.
- **Защита производительности:** При выполнении CI в течение 2 секунд отсутствует аппаратное обеспечение CI; o использовать imprime a duracao medida e o
  Зарезервировано четыре байта (`max_parallel = 3`, `peak_reserved_bytes <= 196608`).

Пример резюме:

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Использование привязок Python- **Командо:** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **Эскопо:** Упражнение или оболочка высокого уровня `iroha_python.sorafs.multi_fetch_local` и несколько классов данных.
  Советы по использованию устройства Canonico, которое использует API-интерфейс, используемый нами. О тесте
  восстановить метаданные провайдера для части `providers.json`, добавить снимок телеметрии и проверить
  байты полезной нагрузки, квитанции о порциях, отчеты провайдеров и содержимое табло, аналогичное пакетам Rust/JS/Swift.
- **Предварительное требование:** Выполните `maturin develop --release` (или установите колесо), чтобы показать `_crypto`.
  привязка `sorafs_multi_fetch_local` до Chamar pytest; o привязка se auto-ignora quando o привязка
  nao estiver disponivel.
- **Защита производительности:** Mesmo orcamento <= 2 s da suite Rust; pytest регистрирует заражение байтов
  монтаж и резюме участия поставщиков для выпуска произведений искусства.

Освободите сбор данных или сводный вывод Cada (Rust, Python, JS, Swift) для того, чтобы
Относительный архив позволяет сравнить поступления полезной нагрузки и метрики единой формы перед продвижением
хм, строй. Выполните `ci/sdk_sorafs_orchestrator.sh`, чтобы просмотреть все как наборы средств (Rust, Python
привязки, JS, Swift) в единственном проходе; Artefatos de CI Devem Anexar или Trecho de Log Desse Helper
Больше всего `matrix.md` добавлено (таблица SDK/статус/длительность) в билет выпуска, который может быть доступен рецензентам.
прослушайте матрицу парадад, чтобы повторно выполнить локальный пакет.
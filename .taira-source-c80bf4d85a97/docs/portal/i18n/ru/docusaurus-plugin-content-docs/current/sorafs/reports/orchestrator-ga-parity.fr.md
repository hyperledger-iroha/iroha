---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Отчет о паритете GA SoraFS Orchestrator

Паритет, определенный при множественной выборке, уродлив в SDK до тех пор, пока он не будет получен.
инженеры по выпуску могут подтвердить байты полезной нагрузки, квитанции о фрагментах,
отчеты поставщика и результаты табло остаются согласованными между собой
реализации. Ремень безопасности Chaque Consomme le Bundle от нескольких поставщиков canonique dans
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/`, который перегруппирует план SF1,
поставщик метаданных, снимок телеметрии и параметры оркестратора.

## Базовая линия ржавчины

- **Команда:** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **Область:** Выполнить план `MultiPeerFixture` дважды через текущий оркестратор,
  для проверки байтов сборок полезной нагрузки, квитанций фрагментов, отчетов поставщика и т. д.
  результаты табло. Инструментальный костюм aussi la concurrence de pointe
  и далее эффективный рабочий набор (`max_parallel × max_chunk_length`).
- **Защита производительности:** Выполнение завершается в течение 2 с на аппаратном обеспечении CI.
- **Рабочий комплект потолка:** Avec le profil SF1, жгут проводов `max_parallel = 3`,
  Donnant une Fenêtre ≤ 196608 байт.

Пример вывода журнала:

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## Использование JavaScript SDK

- **Команда:** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **Объем:** Обновление памяти через `iroha_js_host::sorafsMultiFetchLocal`,
  сравнительные полезные данные, квитанции, отчеты поставщиков и снимки табло.
  последовательные казни.
- **Охрана исполнения:** Выполнение завершается в течение 2 с; le упряжь imprime la
  в течение длительного времени и плафон резервных байтов (`max_parallel = 3`, `peak_reserved_bytes ≤ 196 608`).

Пример сводной строки:

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Обвязка Swift SDK

- **Команда:** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **Область:** Выполните набор параметров, определенных в `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift`,
  Подключите прибор SF1 deux fois через мост Norito (`sorafsLocalFetch`). Проверка ремня безопасности
  байты полезной нагрузки, квитанции о порциях, отчеты провайдеров и записи на табло в используемых файлах.
  поставщики метаданных определяют параметры и снимки телеметрии, которые используются в пакетах Rust/JS.
- **Загрузка моста:** Распаковка жгута `dist/NoritoBridge.xcframework.zip` по требованию и за плату
  le срез macOS через `dlopen`. Lorsque l'xcframework не работает или не имеет привязок SoraFS, il
  bascule sur `cargo build -p connect_norito_bridge --release` и так далее
  `target/release/libconnect_norito_bridge.dylib`, без ручной настройки в CI.
- **Защита производительности:** Завершить выполнение в течение 2 с на аппаратном обеспечении CI; le упряжь imprime la
  в течение длительного времени и плафон резервных байтов (`max_parallel = 3`, `peak_reserved_bytes ≤ 196 608`).

Пример сводной строки:

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Привязка Python- **Команда:** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **Область:** Используйте оболочку высшего уровня `iroha_python.sorafs.multi_fetch_local` и эти типы классов данных.
  Afin, что каноническое приспособление проходит мимо API-интерфейса, который используют любители колес. Тест реконструировать
  поставщик метаданных `providers.json`, внедрите снимок телеметрии и проверьте байты полезной нагрузки,
  квитанции о порциях, отчеты провайдеров и содержимое табло в пакетах Rust/JS/Swift.
- **Предварительное требование:** Выполните `maturin develop --release` (или установите колесо), чтобы `_crypto` открыл привязку
  `sorafs_multi_fetch_local` перед вызовом pytest ; автопропуск ремней безопасности неотделим.
- **Защита производительности:** Бюджет мема ≤ 2 с, как в наборе Rust; pytest logge le nombre de bytes assemblés
  и резюме участия поставщиков для выпуска артефактов.

Выпуск стробирования doit capturer с итоговым выводом каждого жгута (Rust, Python, JS, Swift) в ближайшее время
Архив взаимопонимания позволяет сравнивать поступления полезной нагрузки и метрики единого управления перед
продвигать строительство. Exécutez `ci/sdk_sorafs_orchestrator.sh` для лансера Chaque Suite de Parité
(Rust, привязки Python, JS, Swift) в одном месте; les artefacts CI doivent joindre l'extrait
журнал этого помощника плюс созданный `matrix.md` (таблица SDK/status/durée) или билет выпуска в ближайшее время
Рецензенты могут проверить матрицу паритета без перераспределения мест в люксе.
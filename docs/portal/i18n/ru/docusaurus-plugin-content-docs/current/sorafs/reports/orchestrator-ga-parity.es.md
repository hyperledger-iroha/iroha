---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Отчет о парите GA del Orchestrator SoraFS

Детерминированная множественная выборка теперь будет доступна для SDK для того, что вы хотите.
Инженеры по выпуску могут подтвердить, что байты полезной нагрузки, квитанции о фрагментах,
Отчеты провайдера и результаты табло всегда доступны между
реализации. Ремень Cada потребляет пакет el multi-провайдера canonico bajo
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/`, который содержит план SF1,
Метаданные поставщика, снимок телеметрии и параметры оркестратора.

## Базовая линия ржавчины

- **Команда:** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **Объем:** Выполните выполнение плана `MultiPeerFixture` с помощью текущего оркестратора,
  проверенные байты полезной нагрузки, квитанции о фрагментах, отчеты провайдера и
  результаты на табло. La Instrumentacion tambien rastrea concurrencia pico
  и эффективная работа рабочего набора (`max_parallel x max_chunk_length`).
- **Защита производительности:** Выброс данных может быть завершен за 2 секунды на аппаратном обеспечении CI.
- **Рабочий комплект потолка:** С перфилем SF1 и ремнем безопасности `max_parallel = 3`,
  дандо уна вентана <= 196608 байт.

Пример вывода журнала:

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## Использование JavaScript SDK

- **Команда:** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **Объем:** Воспроизведение прибора el mismo через `iroha_js_host::sorafsMultiFetchLocal`,
  сравнение полезных данных, квитанций, отчетов поставщиков и снимков табло.
  последовательные выбросы.
- **Защита производительности:** Процесс выброса должен быть завершен в течение 2 с; эль-ремни импрайм-ла
  продолжительность записи и зарезервированные байты (`max_parallel = 3`, `peak_reserved_bytes <= 196608`).

Пример сводной строки:

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Обвязка Swift SDK

- **Команда:** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **Объем:** Выведите набор определенных параметров в `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift`,
  воспроизвести приспособление SF1 дважды через мост Norito (`sorafsLocalFetch`). Эль жгут
  проверка байтов полезной нагрузки, квитанции фрагментов, отчеты поставщика и входы в табло с использованием
  Детерминированные метаданные поставщика неправильного типа и снимки телеметрии, имеющиеся в пакетах Rust/JS.
- **Загрузочный мост:** El Harvest descomprime `dist/NoritoBridge.xcframework.zip` bajo requirea y carga
  el срез macOS через `dlopen`. Если xcframework не работает или нет привязок SoraFS, есть резервный вариант
  `cargo build -p connect_norito_bridge --release` и ссылка против
  `target/release/libconnect_norito_bridge.dylib`, руководство по настройке sin на CI.
- **Защита производительности:** Завершение выброса происходит через 2 секунды на аппаратном обеспечении CI; эль-ремни импрайм-ла
  продолжительность записи и зарезервированные байты (`max_parallel = 3`, `peak_reserved_bytes <= 196608`).

Пример сводной строки:

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Привязка Python- **Команда:** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **Область:** Извлечение оболочки высокого уровня `iroha_python.sorafs.multi_fetch_local` и всех классов данных.
  Советы по использованию устройства Canonico Fluya Por La Misma API, которое используется на колесах. Эль тест
  восстановить метаданные поставщика на основе `providers.json`, добавить снимок телеметрии и проверить
  байты полезной нагрузки, квитанции о порциях, отчеты провайдера и содержимое табло, которое есть на самом деле.
  наборы Rust/JS/Swift.
- **Предварительный запрос:** Извлеките `maturin develop --release` (установите колесо), чтобы `_crypto` показал его.
  привязка `sorafs_multi_fetch_local` до вызова pytest; el упряжь se auto-salta cuando el
  не является обязательным.
- **Защита производительности:** El mismo presupuesto <= 2 секунды, как в наборе Rust; pytest регистрация содержимого
  байты объединяются и возобновляют участие поставщиков для артефакта выпуска.

Выпуск шлюза должен захватывать сводный вывод каждого кода (Rust, Python, JS, Swift) для того, чтобы
Архив отчета позволяет сравнить поступления полезной нагрузки и единые показатели формы до
промоутер и строительство. Извлеките `ci/sdk_sorafs_orchestrator.sh` для исправления каждой парной комнаты
(Rust, привязки Python, JS, Swift) в одном месте; артефакты CI должны быть добавлены
Извлечение журнала этого помощника из созданного `matrix.md` (таблица SDK/estado/duracion) для билета
выпустите, чтобы рецензенты проверяли матрицу паритета без повторного использования в местном масштабе.
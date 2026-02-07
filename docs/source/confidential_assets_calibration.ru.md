---
lang: ru
direction: ltr
source: docs/source/confidential_assets_calibration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 01bfdc70f601098acaefc60c6a3b4c464218b8c6f01f2f20eb3632994ff7110f
source_last_modified: "2026-01-03T18:07:57.759135+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Конфиденциальные базовые параметры калибровки газа

В этом журнале отслеживаются проверенные результаты конфиденциальной калибровки по газу.
ориентиры. В каждой строке документируется набор показателей качества выпуска, полученный с помощью
процедура описана в `docs/source/confidential_assets.md#calibration-baselines--acceptance-gates`.

| Дата (UTC) | Зафиксировать | Профиль | `ns/op` | `gas/op` | `ns/gas` | Заметки |
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3c70a7d3 | базовый-неон | 2.93e5 | 1.57е2 | 1.87e3 | Дарвин 25.0.0 Arm64e (информация о хосте); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 28 апреля 2026 г. | 8ea9b2a7 | базовый-неон-20260428 | 4.29e6 | 1.57е2 | 2.73е4 | Дарвин 25.0.0 Arm64 (`rustc 1.91.0`). Команда: `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=10 --warm-up-time=2 --noplot --save-baseline baseline-neon-20260428`; войдите в `docs/source/confidential_assets_calibration_neon_20260428.log`. Прогоны четности x86_64 (SIMD-нейтральный + AVX2) запланированы на 19 марта 2026 г. в лабораторном слоте в Цюрихе; артефакты попадут под `artifacts/confidential_assets_calibration/2026-03-x86/` с соответствующими командами и будут объединены в базовую таблицу после захвата. |
| 28 апреля 2026 г. | — | базовый SIMD-нейтральный | — | — | — | **Отключено** на Apple Silicon — `ring` обеспечивает принудительное использование NEON для платформы ABI, поэтому `RUSTFLAGS="-C target-feature=-neon"` завершается сбоем до того, как стенд сможет запуститься (`docs/source/confidential_assets_calibration_simd_neutral_attempt_20260428.log`). Нейтральные данные остаются закрытыми на хосте CI `bench-x86-neon0`. |
| 28 апреля 2026 г. | — | базовый уровень-avx2 | — | — | — | **Отложено** до тех пор, пока не станет доступен модуль x86_64. `arch -x86_64` не может создавать двоичные файлы на этом компьютере («Неверный тип ЦП в исполняемом файле»; см. `docs/source/confidential_assets_calibration_avx2_attempt_20260428.log`). Хост CI `bench-x86-avx2a` остается источником записи. |

`ns/op` суммирует среднее значение настенных часов на каждую инструкцию, измеренное Criterion;
`gas/op` — среднее арифметическое соответствующих плановых затрат из
`iroha_core::gas::meter_instruction`; `ns/gas` делит сумму наносекунд на
суммарный газ по набору выборок из девяти инструкций.

*Примечание.* Текущий хост Arm64 не выдает сводки по критерию `raw.csv` из
коробка; повторите запуск с `CRITERION_OUTPUT_TO=csv` или исправлением вышестоящей версии, прежде чем помечать
выпустить, чтобы были прикреплены артефакты, требуемые контрольным списком приемки.
Если `target/criterion/` по-прежнему отсутствует после `--save-baseline`, соберите прогон
на хосте Linux или сериализовать вывод консоли в пакет выпуска как
временная задержка. Для справки, журнал консоли Arm64 из последнего запуска.
живет по адресу `docs/source/confidential_assets_calibration_neon_20251018.log`.

Медианные значения для каждой инструкции из одного и того же запуска (`cargo bench -p iroha_core --bench isi_gas_calibration`):

| Инструкция | медиана `ns/op` | расписание `gas` | `ns/gas` |
| --- | --- | --- | --- |
| Зарегистрировать домен | 3.46e5 | 200 | 1.73е3 |
| РегистрацияАккаунт | 3.15e5 | 200 | 1.58e3 |
| РегистрацияАссетДеф | 3.41e5 | 200 | 1.71e3 |
| SetAccountKV_small | 3.28e5 | 67 | 4.90e3 |
| ГрантАккаунтРоль | 3.33e5 | 96 | 3.47e3 |
| RevokeAccountRole | 3.12e5 | 96 | 3.25e3 |
| ExecuteTrigger_empty_args | 1.42e5 | 224 | 6.33е2 |
| МинтАссет | 1.56e5 | 150 | 1.04e3 |
| ТрансферАссет | 3.68e5 | 180 | 2.04e3 |

### 28 апреля 2026 г. (Apple Silicon, NEON включен)

Медианные задержки для обновления 28 апреля 2026 г. (`cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=10 --warm-up-time=2 --noplot --save-baseline baseline-neon-20260428`):| Инструкция | медиана `ns/op` | расписание `gas` | `ns/gas` |
| --- | --- | --- | --- |
| Зарегистрировать домен | 8.58e6 | 200 | 4.29e4 |
| РегистрацияАккаунт | 4.40e6 | 200 | 2.20e4 |
| РегистрацияАссетДеф | 4.23e6 | 200 | 2.12e4 |
| SetAccountKV_small | 3.79e6 | 67 | 5.66е4 |
| ГрантАккаунтРоль | 3.60e6 | 96 | 3.75е4 |
| RevokeAccountRole | 3.76e6 | 96 | 3.92е4 |
| ExecuteTrigger_empty_args | 2.71e6 | 224 | 1.21e4 |
| МинтАссет | 3.92e6 | 150 | 2.61e4 |
| ТрансферАссет | 3.59e6 | 180 | 1.99e4 |

Агрегаты `ns/op` и `ns/gas` в таблице выше получены из суммы
эти медианы (всего `3.85717e7`ns по набору из девяти команд и 1413
газовые агрегаты).

Столбец расписания поддерживается `gas::tests::calibration_bench_gas_snapshot`.
(всего 1413 газов в наборе из девяти инструкций) и отключится, если в будущих патчах
изменить измерение без обновления калибровочных приборов.

## Телеметрическое подтверждение дерева обязательств (M2.2)

В соответствии с задачей дорожной карты **M2.2** каждый калибровочный запуск должен фиксировать новые
Индикаторы дерева обязательств и счетчики выселений, чтобы доказать сохранение границы Меркла
в заданных пределах:

- `iroha_confidential_tree_commitments{asset_id}`
- `iroha_confidential_tree_depth{asset_id}`
- `iroha_confidential_root_history_entries{asset_id}`
- `iroha_confidential_frontier_checkpoints{asset_id}`
- `iroha_confidential_frontier_last_checkpoint_height{asset_id}`
- `iroha_confidential_frontier_last_checkpoint_commitments{asset_id}`
- `iroha_confidential_root_evictions_total{asset_id}`
- `iroha_confidential_frontier_evictions_total{asset_id}`
- `iroha_zk_verifier_cache_events_total{cache,event}`

Запишите значения непосредственно до и после калибровочной нагрузки. А
достаточно одной команды для каждого актива; пример для `xor#wonderland`:

```bash
curl -s http://127.0.0.1:8180/metrics \
  | rg 'iroha_confidential_(tree_(commitments|depth)|root_history_entries|frontier_(checkpoints|last_checkpoint_height|last_checkpoint_commitments)|root_evictions_total|frontier_evictions_total){asset_id="xor#wonderland"}'
```

Прикрепите необработанные выходные данные (или снимок Prometheus) к билету калибровки, чтобы
Рецензент управления может подтвердить, что ограничения корневой истории и интервалы контрольных точек соблюдаются.
заслужено. Руководство по телеметрии в `docs/source/telemetry.md#confidential-tree-telemetry-m22`
подробно рассказывается об ожиданиях оповещений и связанных с ними панелях Grafana.

Включите счетчики кэша верификатора в одну и ту же очистку, чтобы рецензенты могли подтвердить
коэффициент промахов оставался ниже порога предупреждения 40%:

```bash
curl -s http://127.0.0.1:8180/metrics \\
  | rg 'iroha_zk_verifier_cache_events_total{cache="vk",event="(hit|miss)"}'
```

Задокументируйте полученное соотношение (`miss / (hit + miss)`) в примечании о калибровке.
чтобы показать, что упражнения по моделированию затрат, нейтральные к SIMD, повторно использовали теплые кэши вместо
перебор реестра проверяющих Halo2.

## Нейтральность и отказ от AVX2

Совет SDK предоставил временный отказ от шлюза PhaseC, требующего
Измерения `baseline-simd-neutral` и `baseline-avx2`:

- **Независимость от SIMD:** На Apple Silicon криптографический сервер `ring` обеспечивает NEON для
  Корректность ABI. Отключение этой функции (`RUSTFLAGS="-C target-feature=-neon"`)
  прерывает сборку до создания бинарного файла стенда (`docs/source/confidential_assets_calibration_simd_neutral_attempt_20260428.log`).
- **AVX2:** Локальная цепочка инструментов не может создавать двоичные файлы x86_64 (`arch -x86_64 rustc -V`
  → «Неверный тип процессора в исполняемом файле»; увидеть
  `docs/source/confidential_assets_calibration_avx2_attempt_20260428.log`).

Пока хосты CI `bench-x86-neon0` и `bench-x86-avx2a` не будут в сети, NEON будет работать.
выше, а также данные телеметрии соответствуют критериям приемки PhaseC.
Отказ записан в `status.md` и будет пересмотрен после того, как оборудование x86 будет готово.
доступен.
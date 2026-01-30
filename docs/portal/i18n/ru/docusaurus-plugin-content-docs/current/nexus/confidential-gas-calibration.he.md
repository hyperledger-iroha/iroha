---
lang: he
direction: rtl
source: docs/portal/i18n/ru/docusaurus-plugin-content-docs/current/nexus/confidential-gas-calibration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5419762a93fb4e6ff74e2875b45d05d67f4fbfaa62b941f62dc44ae8823a8e4b
source_last_modified: "2025-11-14T04:43:20.323689+00:00"
translation_last_reviewed: 2026-01-30
---

# Базовые линии калибровки конфиденциального газа

Этот реестр отслеживает проверенные результаты бенчмарков калибровки конфиденциального газа. Каждая строка документирует набор измерений уровня релиза, собранный по процедуре из [Confidential Assets & ZK Transfers](./confidential-assets#calibration-baselines--acceptance-gates).

| Дата (UTC) | Commit | Профиль | `ns/op` | `gas/op` | `ns/gas` | Примечания |
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3c70a7d3 | baseline-neon | 2.93e5 | 1.57e2 | 1.87e3 | Darwin 25.0.0 arm64e (hostinfo); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 2026-04-12 | pending | baseline-simd-neutral | - | - | - | Запланированный нейтральный прогон x86_64 на CI-хосте `bench-x86-neon0`; см. тикет GAS-214. Результаты будут добавлены после завершения окна бенча (pre-merge чеклист ориентирован на релиз 2.1). |
| 2026-04-13 | pending | baseline-avx2 | - | - | - | Последующая калибровка AVX2 с тем же commit/build, что и нейтральный прогон; требуется хост `bench-x86-avx2a`. GAS-214 покрывает оба прогона с сравнением дельты относительно `baseline-neon`. |

`ns/op` агрегирует медиану wall-clock на инструкцию, измеренную Criterion; `gas/op` - это арифметическое среднее соответствующих затрат расписания из `iroha_core::gas::meter_instruction`; `ns/gas` делит суммарные наносекунды на суммарный газ для набора из девяти инструкций.

*Примечание.* Текущий arm64 хост по умолчанию не выводит сводки Criterion `raw.csv`; перезапустите с `CRITERION_OUTPUT_TO=csv` или примените upstream исправление перед тегированием релиза, чтобы артефакты, требуемые чеклистом приемки, были приложены. Если `target/criterion/` все еще отсутствует после `--save-baseline`, выполните прогон на Linux хосте или сериализуйте вывод консоли в релизный бандл как временный stopgap. Для справки, arm64 консольный лог последнего прогона находится в `docs/source/confidential_assets_calibration_neon_20251018.log`.

Медианы по инструкциям из того же прогона (`cargo bench -p iroha_core --bench isi_gas_calibration`):

| Instruction | median `ns/op` | schedule `gas` | `ns/gas` |
| --- | --- | --- | --- |
| RegisterDomain | 3.46e5 | 200 | 1.73e3 |
| RegisterAccount | 3.15e5 | 200 | 1.58e3 |
| RegisterAssetDef | 3.41e5 | 200 | 1.71e3 |
| SetAccountKV_small | 3.28e5 | 67 | 4.90e3 |
| GrantAccountRole | 3.33e5 | 96 | 3.47e3 |
| RevokeAccountRole | 3.12e5 | 96 | 3.25e3 |
| ExecuteTrigger_empty_args | 1.42e5 | 224 | 6.33e2 |
| MintAsset | 1.56e5 | 150 | 1.04e3 |
| TransferAsset | 3.68e5 | 180 | 2.04e3 |

Колонка schedule обеспечивается `gas::tests::calibration_bench_gas_snapshot` (всего 1,413 gas по набору из девяти инструкций) и вызовет ошибку, если будущие изменения поменяют метеринг без обновления калибровочных фикстур.

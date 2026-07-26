---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/confidential-gas-calibration.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
タイトル: Реестр калибровки конфиденциального газа
説明: Измерения уровня релиза、подтверждающие график конфиденциального газа。
スラグ: /nexus/confidential-gas-calibration
---

# Базовые линии калибровки конфиденциального газа

Этот реестр отслеживает проверенные результаты бенчмарков калибровки конфиденциального газа. [機密資産と ZK の譲渡](./confidential-assets#calibration-baselines--acceptance-gates) を参照してください。

|ダタ (UTC) |コミット | Профиль | `ns/op` | `gas/op` | `ns/gas` | Примечания |
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3c70a7d3 |ベースライン-ネオン | 2.93e5 | 1.57e2 | 1.87e3 |ダーウィン 25.0.0 arm64e (ホスト情報); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 2026-04-12 |保留中 |ベースライン-simd-ニュートラル | - | - | - | Запланированный нейтральный прогон x86_64 на CI-хосте `bench-x86-neon0`;最低。 GAS-214 です。 (マージ前 чеклист ориентирован на релиз 2.1)。 |
| 2026-04-13 |保留中 |ベースライン-avx2 | - | - | - | AVX2 はコミット/ビルドを実行します。 требуется хост `bench-x86-avx2a`。 GAS-214 покрывает оба прогона с сравнением дельты относительно `baseline-neon`. |

`ns/op` агрегирует медиану 壁掛け時計 на инструкцию, измеренную 基準; `gas/op` - Ѝто арифметическое среднее соответствующих затрат расписания из `iroha_core::gas::meter_instruction`; `ns/gas` は、今日の状況を説明します。

*Примечание.* Текущий arm64 хост по умолчанию не выводит сводки Criterion `raw.csv`; `CRITERION_OUTPUT_TO=csv` はアップストリーム исправление перед тегированием релиза, чтобы артефакты, требуемые Їеклистом приемки、были приложены。 `target/criterion/` は、Linux 版の `--save-baseline` をサポートします。 консоли в релизный бандл как временный その場しのぎ。 Для справки, arm64 консольный лог последнего прогона находится в `docs/source/confidential_assets_calibration_neon_20251018.log`.

Медианы по инструкциям из того же прогона (`cargo bench -p iroha_core --bench isi_gas_calibration`):

|指示 |中央値 `ns/op` |スケジュール `gas` | `ns/gas` |
| --- | --- | --- | --- |
|ドメインの登録 | 3.46e5 | 200 | 1.73e3 |
|アカウント登録 | 3.15e5 | 200 | 1.58e3 |
|登録資産定義 | 3.41e5 | 200 | 1.71e3 |
| SetAccountKV_small | 3.28e5 | 67 | 4.90e3 |
|アカウントロールを付与 | 3.33e5 | 96 | 3.47e3 |
|アカウントロールを取り消す | 3.12e5 | 96 | 3.25e3 |
|トリガー_空の引数を実行する | 1.42e5 | 224 | 6.33e2 |
|ミントアセット | 1.56e5 | 150 | 1.04e3 |
|資産の譲渡 | 3.68e5 | 180 | 2.04e3 |

Колонка スケジュール обеспечивается `gas::tests::calibration_bench_gas_snapshot` (всего 1,413 ガソリン по набору из девяти инструкций) и вызовет олибку, если будущие изменения поменяют метеринг без обновления калибровочных фикстур.
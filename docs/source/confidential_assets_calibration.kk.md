---
lang: kk
direction: ltr
source: docs/source/confidential_assets_calibration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 01bfdc70f601098acaefc60c6a3b4c464218b8c6f01f2f20eb3632994ff7110f
source_last_modified: "2025-12-29T18:16:35.932211+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Құпия газды калибрлеудің негізгі көрсеткіштері

Бұл кітап құпия газ калибрлеуінің расталған нәтижелерін бақылайды
эталондар. Әрбір жол түсірілген шығару сапасы өлшем жинағын құжаттайды
`docs/source/confidential_assets.md#calibration-baselines--acceptance-gates` бөлімінде сипатталған процедура.

| Күні (UTC) | Міндеттеме | Профиль | `ns/op` | `gas/op` | `ns/gas` | Ескертпелер |
| --- | --- | --- | --- | --- | --- | --- |
| 18.10.2025 | 3c70a7d3 | базалық-неон | 2.93e5 | 1.57e2 | 1.87e3 | Darwin 25.0.0 arm64e (хост ақпараты); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 28.04.2026 | 8ea9b2a7 | baseline-neon-20260428 | 4.29e6 | 1.57e2 | 2.73e4 | Дарвин 25.0.0 arm64 (`rustc 1.91.0`). Пәрмен: `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=10 --warm-up-time=2 --noplot --save-baseline baseline-neon-20260428`; `docs/source/confidential_assets_calibration_neon_20260428.log` мекенжайында журналға кіріңіз. x86_64 паритеттік жүгірістері (SIMD-бейтарап + AVX2) 2026-03-19 Цюрих зертханалық ұясына жоспарланған; артефактілер сәйкес командалармен `artifacts/confidential_assets_calibration/2026-03-x86/` астына түседі және түсірілгеннен кейін негізгі кестеге біріктіріледі. |
| 28.04.2026 | — | baseline-simd-neutral | — | — | — | Apple Silicon жүйесінде **бас тартылды**—`ring` ABI платформасы үшін NEON функциясын орындайды, сондықтан `RUSTFLAGS="-C target-feature=-neon"` орындық жұмыс істей алмай тұрып сәтсіздікке ұшырайды (`docs/source/confidential_assets_calibration_simd_neutral_attempt_20260428.log`). Бейтарап деректер `bench-x86-neon0` CI хостында жабық күйде қалады. |
| 28.04.2026 | — | baseline-avx2 | — | — | — | **Кейінге қалдырылды** x86_64 жүгірушісі қолжетімді болғанша. `arch -x86_64` бұл құрылғыда екілік файлдарды шығара алмайды («Орындалатын файлдағы нашар CPU түрі»; `docs/source/confidential_assets_calibration_avx2_attempt_20260428.log` қараңыз). `bench-x86-avx2a` CI хосты жазба көзі болып қала береді. |

`ns/op` Шартпен өлшенген нұсқау үшін медианалық қабырға сағатын біріктіреді;
`gas/op` - сәйкес кесте шығындарының орташа арифметикалық мәні
`iroha_core::gas::meter_instruction`; `ns/gas` жиынтық наносекундтарды келесіге бөледі
тоғыз инструкциялық үлгі жинағы бойынша қосынды газ.

*Ескертпе.* Ағымдағы arm64 хосты `raw.csv` шартының қорытындыларын шығармайды.
қорап; a белгілеу алдында `CRITERION_OUTPUT_TO=csv` немесе жоғары ағынды түзету арқылы қайта іске қосыңыз
қабылдауды тексеру парағы талап ететін артефактілер қоса тіркелетін етіп шығарыңыз.
`target/criterion/` `--save-baseline` кейін әлі жоқ болса, жүгірісті жинаңыз.
Linux хостында немесе консоль шығысын шығарылым жинағына сериялау үшін
уақытша үзіліс. Анықтама үшін, соңғы іске қосылғаннан arm64 консоль журналы
`docs/source/confidential_assets_calibration_neon_20251018.log` мекен-жайында тұрады.

Бір орындаудағы әрбір нұсқаудың медианалары (`cargo bench -p iroha_core --bench isi_gas_calibration`):

| Нұсқау | медианасы `ns/op` | кестесі `gas` | `ns/gas` |
| --- | --- | --- | --- |
| RegisterDomain | 3.46e5 | 200 | 1.73e3 |
| Register Account | 3.15e5 | 200 | 1.58e3 |
| RegisterAssetDef | 3.41e5 | 200 | 1.71e3 |
| SetAccountKV_small | 3.28e5 | 67 | 4.90e3 |
| GrantAccountRole | 3.33e5 | 96 | 3.47e3 |
| RevokeAccountRole | 3.12e5 | 96 | 3.25e3 |
| ExecuteTrigger_empty_args | 1.42e5 | 224 | 6.33e2 |
| MintAsset | 1.56e5 | 150 | 1.04e3 |
| TransferAsset | 3.68e5 | 180 | 2.04e3 |

### 28.04.2026 (Apple Silicon, NEON қосылған)

28.04.2026 жаңартуға арналған орташа кідіріс (`cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=10 --warm-up-time=2 --noplot --save-baseline baseline-neon-20260428`):| Нұсқау | медианасы `ns/op` | кестесі `gas` | `ns/gas` |
| --- | --- | --- | --- |
| RegisterDomain | 8.58e6 | 200 | 4.29e4 |
| Register Account | 4.40e6 | 200 | 2.20e4 |
| RegisterAssetDef | 4.23e6 | 200 | 2.12e4 |
| SetAccountKV_small | 3.79e6 | 67 | 5.66e4 |
| GrantAccountRole | 3.60e6 | 96 | 3.75e4 |
| RevokeAccountRole | 3.76e6 | 96 | 3.92e4 |
| ExecuteTrigger_empty_args | 2.71e6 | 224 | 1.21e4 |
| MintAsset | 3.92e6 | 150 | 2.61e4 |
| TransferAsset | 3.59e6 | 180 | 1.99e4 |

Жоғарыдағы кестедегі `ns/op` және `ns/gas` агрегаттары қосындысынан алынған.
бұл медианалар (тоғыз нұсқаулар жиынтығы бойынша жалпы `3.85717e7`ns және 1,413
газ қондырғылары).

Кесте бағаны `gas::tests::calibration_bench_gas_snapshot` арқылы орындалады
(тоғыз нұсқаулық жиынтығы бойынша барлығы 1,413 газ) және болашақ патчтар пайда болған жағдайда өшіріледі
калибрлеу аспаптарын жаңартпай өлшеуді өзгерту.

## Міндеттеме тармағының телеметриялық дәлелі (M2.2)

Жол картасының **M2.2** тапсырмасына сәйкес әрбір калибрлеу жұмысы жаңасын алуы керек
Меркле шекарасының сақталуын дәлелдеу үшін міндеттеме ағашы өлшегіштері мен шығару есептегіштері
конфигурацияланған шектерде:

- `iroha_confidential_tree_commitments{asset_id}`
- `iroha_confidential_tree_depth{asset_id}`
- `iroha_confidential_root_history_entries{asset_id}`
- `iroha_confidential_frontier_checkpoints{asset_id}`
- `iroha_confidential_frontier_last_checkpoint_height{asset_id}`
- `iroha_confidential_frontier_last_checkpoint_commitments{asset_id}`
- `iroha_confidential_root_evictions_total{asset_id}`
- `iroha_confidential_frontier_evictions_total{asset_id}`
- `iroha_zk_verifier_cache_events_total{cache,event}`

Мәндерді калибрлеу жұмыс жүктемесінің алдында және кейін бірден жазыңыз. А
актив үшін бір пәрмен жеткілікті; `xor#wonderland` үшін мысал:

```bash
curl -s http://127.0.0.1:8180/metrics \
  | rg 'iroha_confidential_(tree_(commitments|depth)|root_history_entries|frontier_(checkpoints|last_checkpoint_height|last_checkpoint_commitments)|root_evictions_total|frontier_evictions_total){asset_id="xor#wonderland"}'
```

Шикі шығысты (немесе Prometheus суретін) калибрлеу билетіне бекітіңіз.
Басқаруды шолушы түбірлік тарихтың шектерін және бақылау нүктелерінің интервалдарын растай алады
құрметті. `docs/source/telemetry.md#confidential-tree-telemetry-m22` ішіндегі телеметрия нұсқаулығы
ескерту күтулері мен байланысты Grafana панельдерін кеңейтеді.

Тексерушілер растау үшін тексеруші кэш есептегіштерін бірдей скрапқа қосыңыз
өткізіп алу коэффициенті 40% ескерту шегінен төмен болды:

```bash
curl -s http://127.0.0.1:8180/metrics \\
  | rg 'iroha_zk_verifier_cache_events_total{cache="vk",event="(hit|miss)"}'
```

Алынған қатынасты (`miss / (hit + miss)`) калибрлеу жазбасының ішінде құжаттаңыз
SIMD-бейтарап шығындарды модельдеу жаттығуларын көрсету үшін орнына жылы кэштерді қайта пайдаланды
Halo2 тексеруші тізілімін бұзу.

## Бейтарап және AVX2 бас тарту

SDK кеңесі талап ететін PhaseC қақпасы үшін уақытша бас тартуды берді
`baseline-simd-neutral` және `baseline-avx2` өлшемдері:

- **SIMD-бейтарап:** Apple Silicon жүйесінде `ring` криптографиялық сервері NEON үшін күш береді.
  ABI дұрыстығы. Мүмкіндікті өшіру (`RUSTFLAGS="-C target-feature=-neon"`)
  стендтік екілік (`docs/source/confidential_assets_calibration_simd_neutral_attempt_20260428.log`) жасалғанға дейін құрастыруды тоқтатады.
- **AVX2:** Жергілікті құралдар тізбегі x86_64 екілік файлдарын шығара алмайды (`arch -x86_64 rustc -V`
  → «Орындалатын файлдағы нашар CPU түрі»; қараңыз
  `docs/source/confidential_assets_calibration_avx2_attempt_20260428.log`).

`bench-x86-neon0` және `bench-x86-avx2a` CI хосттары онлайн болғанша, NEON іске қосылады.
жоғарыда және телеметрия дәлелдері PhaseC қабылдау критерийлерін қанағаттандырады.
Бас тарту `status.md` ішінде жазылған және x86 аппараттық құралын алған кезде қайта қаралады.
қолжетімді.
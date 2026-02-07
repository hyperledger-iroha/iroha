---
slug: /nexus/confidential-gas-calibration
lang: kk
direction: ltr
source: docs/portal/docs/nexus/confidential-gas-calibration.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Confidential Gas Calibration Ledger
description: Release-quality measurements backing the confidential gas schedule.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Құпия газды калибрлеудің негізгі көрсеткіштері

Бұл кітап құпия газ калибрлеуінің расталған нәтижелерін бақылайды
эталондар. Әрбір жол түсірілген шығару сапасы өлшем жинағын құжаттайды
[Confidential Assets & ZK Transfers](./confidential-assets#calibration-baselines--acceptance-gates) бөлімінде сипатталған процедура.

| Күні (UTC) | Міндеттеме | Профиль | `ns/op` | `gas/op` | `ns/gas` | Ескертпелер |
| --- | --- | --- | --- | --- | --- | --- |
| 18.10.2025 | 3c70a7d3 | базалық-неон | 2.93e5 | 1.57e2 | 1.87e3 | Darwin 25.0.0 arm64e (хост ақпараты); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 12.04.2026 | күтуде | baseline-simd-neutral | — | — | — | `bench-x86-neon0` CI хостында жоспарланған x86_64 бейтарап іске қосу; GAS-214 билетін қараңыз. Нәтижелер стендтік терезе аяқталғаннан кейін қосылады (алдын ала біріктіру бақылау тізімінің мақсаттары шығарылымы 2.1). |
| 13.04.2026 | күтуде | baseline-avx2 | — | — | — | Бейтарап іске қосу сияқты бірдей орындау/құрастыру арқылы AVX2 калибрлеуін бақылау; `bench-x86-avx2a` хостын қажет етеді. GAS-214 `baseline-neon` нұсқасымен дельта салыстыру арқылы екі жұмысты да қамтиды. |

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

Кесте бағаны `gas::tests::calibration_bench_gas_snapshot` арқылы орындалады
(тоғыз нұсқаулық жиынтығы бойынша барлығы 1,413 газ) және болашақ патчтар пайда болған жағдайда өшіріледі
калибрлеу аспаптарын жаңартпай өлшеуді өзгерту.
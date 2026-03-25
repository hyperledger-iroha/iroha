---
lang: ba
direction: ltr
source: docs/source/confidential_assets_calibration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 01bfdc70f601098acaefc60c6a3b4c464218b8c6f01f2f20eb3632994ff7110f
source_last_modified: "2025-12-29T18:16:35.932211+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Конфиденциаль газ калибровкаһы нигеҙҙәре

Был леггер раҫланған сығыштарын күҙәтә конфиденциаль газ калибровка
ориентирҙары. Һәр рәт документында 2012 йылда төшөрөлгән релиз-сифатлы үлсәү комплекты .
`docs/source/confidential_assets.md#calibration-baselines--acceptance-gates`-ла һүрәтләнгән процедура.

| Дата (UTC) | Коммит | Профиль | `ns/op` | `gas/op` | `ns/gas` | Иҫкәрмәләр |
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3c70a7d3 | база-неон | 2.93е5 | 1.57e2 | 1.87e3 | Дарвин 25.0.0 ark64e (хостинфо); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored` X; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 2026-04-28 | 8ea9b2a7 | башланғыс-неон-20260428 | 4.29е6 | 1.57e2 | 2.73е4 | Дарвин 25,0,0 ark64 (`rustc 1.91.0`). Команда: `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=10 --warm-up-time=2 --noplot --save-baseline baseline-neon-20260428`; лог `docs/source/confidential_assets_calibration_neon_20260428.log`. x86_64 паритет йүгерә (SIMD-нейтраль + AVX2) 2026-03-19 Цюрих лабораторияһы слотына планлаштырылған; артефакттар `artifacts/confidential_assets_calibration/2026-03-x86/` буйынса тап килгән командалар менән төшәсәк һәм бер тапҡыр тотолған база өҫтәленә берләштереләсәк. |
| 2026-04-28 | — | база-семд-нейтраль | — | — | — | **Аппле кремнийында баш тартты** ABI платформаһы өсөн NEON-ды үтәй, шуға күрә `RUSTFLAGS="-C target-feature=-neon"` эскәмйә эшләй алғанға тиклем уңышһыҙлыҡҡа осрай (`docs/source/confidential_assets_calibration_simd_neutral_attempt_20260428.log`). Нейтраль мәғлүмәттәр CI `bench-x86-neon0` хостында ҡапҡалы ҡала. |
| 2026-04-28 | — | нигеҙ һыҙығы-avx2 | — | — | — | **Күберәк ** тиклем х86_64 йүгерсе бар. `arch -x86_64` был машинала бинарҙарҙы тыуҙыра алмай (“Насар процессор тибы башҡарыла”; ҡарағыҙ `docs/source/confidential_assets_calibration_avx2_attempt_20260428.log`). CI хост `bench-x86-avx2a` рекорд сығанағы булып ҡала. |

`ns/op` агрегат медиана стена сәғәте бер күрһәтмә үлсәү критерий;
`gas/op` - был арифметик уртаса тейешле график сығымдары 2012 йылдан алып .
`iroha_core::gas::meter_instruction`; `ns/gas` йомғаҡланған наносекундтарҙы 2012 йылға бүлә.
туғыҙ инструкция өлгөһө буйынса дөйөм газ йыйылмаһы.

*Иғтибар.* Хәҙерге hard64 хужаһы `raw.csv` критерийын сығармай.
йәшник; `CRITERION_OUTPUT_TO=csv` йәки өҫкө ағымда төҙәтеү менән ҡабаттан эшләү алдынан
сығарыу, шулай итеп, артефакттар талап ителә ҡабул итеү тикшерелгән исемлеге беркетелгән.
Әгәр `target/criterion/` `--save-baseline`-тан һуң һаман да юғалһа, йүгереүҙе йыйырға .
Linux хостында йәки консоль сығышын сериялаштырыу өҫтөндә сығарыу өйөмө булараҡ
ваҡытлыса туҡталыш. Һылтанма өсөн, art64 консоль журналы һуңғы йүгерә
йәшәй `docs/source/confidential_assets_calibration_neon_20251018.log`.

Шул уҡ йүгереүҙән (`cargo bench -p iroha_core --bench isi_gas_calibration`) инструкция медианалары:

| Инструкция | медиана `ns/op` | графигы `gas` | `ns/gas` |
| --- | --- | --- | --- |
| Теркәү Домен | 3.46е5 | 200 | 1.73е3 |
| Теркәүес | 3.15е5 | 200 | 1.58е3 |
| РегистрАссетДеф | 3.41e5 | 200 | 1.71e3 |
| SetAcfaceKV_small | 3.28е5 | 67 | 4.90e3 |
| GrantAcfaceRole | 3.33e5 | 96 | 3.47e3 |
| RevokeAcfaceRole | 3.12е5 | 96 | 3.25е3 |
| ExecuteTrigger_бут_аргтар | 1.42е5 | 224 | 6.332 |
| MintAsset | 1,56е5 | 150 | 1.04e3 |
| ТрансферАсесс | 3.68е5 | 180 | 2.04e3 |

### 2026-04-28 (Алма кремний, NEON өҫтөндә)

2026-04-28 йылдарҙы яңыртыу өсөн медиана латенциялар (`cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=10 --warm-up-time=2 --noplot --save-baseline baseline-neon-20260428`):| Инструкция | медиана `ns/op` | графигы `gas` | `ns/gas` |
| --- | --- | --- | --- |
| Теркәү Домен | 8.58е6 | 200 | 4.29е4 |
| Теркәүес | 4.40е6 | 200 | 2.20е4 |
| РегистрАссетДеф | 4.23е6 | 200 | 2.12е4 |
| SetAcfaceKV_small | 3.79е6 | 67 | 5.66е4 |
| GrantAcfaceRole | 3.60е6 | 96 | 3.75е4 |
| RevokeAcfaceRole | 3.76е6 | 96 | 3.92е4 |
| ExecuteTrigger_бут_аргтар | 2.71е6 | 224 | 1.21е4 |
| MintAsset | 3.92е6 | 150 | 2.61е4 |
| ТрансферАсесс | 3.59е6 | 180 | 1.99е4 |

`ns/op` һәм `ns/gas` агрегаттары өҫтәге таблицала 2012 йылдың суммаһынан алынған.
был медианалар (дөйөм `3.85717e7`ns аша туғыҙ инструкция комплекты һәм 1,413
газ агрегаттары).

График бағанаһы `gas::tests::calibration_bench_gas_snapshot` менән үтәлә.
(дөйөм 1,413 газ аша туғыҙ инструкция комплекты) һәм сәйәхәт итәсәк, әгәр киләсәктә патч
үҙгәрешен иҫәпләү калибровка ҡоролмаларын яңыртыуһыҙ.

## йөкләмәләр ағасы телеметрияһы дәлилдәре (М2.2)

Юл картаһы буйынса бурыс **М2.2**, һәр калибровка йүгерә тейеш яңы тотоп .
йөкләмә-ағас датчиктар һәм күсерелгән иҫәпләүселәр иҫбатлау өсөн Меркл сиге ҡала
конфигурацияланған сиктәр эсендә:

- `docs/source/confidential_assets.md#calibration-baselines--acceptance-gates`.
- `iroha_confidential_tree_depth{asset_id}`
- `iroha_confidential_root_history_entries{asset_id}`
- `iroha_confidential_frontier_checkpoints{asset_id}`
- `iroha_confidential_frontier_last_checkpoint_height{asset_id}`
— `iroha_confidential_frontier_last_checkpoint_commitments{asset_id}`
- `iroha_confidential_root_evictions_total{asset_id}`
- `iroha_confidential_frontier_evictions_total{asset_id}`
- `iroha_zk_verifier_cache_events_total{cache,event}`

Ҡиммәттәрҙе калибровка эш йөкләмәһенә тиклем һәм унан һуң шунда уҡ яҙып алығыҙ. А А.
бер актив өсөн бер команда етә; миҫал өсөн ```bash
curl -s http://127.0.0.1:8180/metrics \\
  | rg 'iroha_zk_verifier_cache_events_total{cache="vk",event="(hit|miss)"}'
```:

```bash
curl -s http://127.0.0.1:8180/metrics \
  | rg 'iroha_confidential_(tree_(commitments|depth)|root_history_entries|frontier_(checkpoints|last_checkpoint_height|last_checkpoint_commitments)|root_evictions_total|frontier_evictions_total){asset_id="4cuvDVPuLBKJyN6dPbRQhmLh68sU"}'
```

Сеймал етештереүҙе беркетергә (йәки Prometheus снимок) калибровка билет шулай итеп,
идара итеү рецензент раҫлай ала тамыр-тарих ҡапҡастары һәм тикшерелгән пункт арауыҡтары булып тора
тип хөрмәтләне. Телеметрия етәксеһе `docs/source/telemetry.md#confidential-tree-telemetry-m22`
иҫкәрткән өмөттәр һәм улар менән бәйле Grafana панелдәре киңәйә.

Ҡатнашыусы кэш иҫәпләүселәрҙе бер үк ҡырҡыуҙа индереү, шулай итеп, рецензенттар раҫлай ала
40% иҫкәрткән сиктән түбән нисбәт ҡалды:

```bash
curl -s http://127.0.0.1:8180/metrics \\
  | rg 'iroha_zk_verifier_cache_events_total{cache="vk",event="(hit|miss)"}'
```

Документ алынған нисбәт (`miss / (hit + miss)`) эсендә калибровка иҫкәрмәһе
SIMD-нейтраль сығымдарҙы күрһәтеү өсөн моделләштереү күнекмәләрен ҡабаттан ҡулланған йылы кэштар урынына
thashing Halo2 тикшерелгән реестр.

## Нейтраль & AVX2 отличник

СДК советы ваҡытлыса ташлама бирҙе PhaseC ҡапҡаһы талап итә
`baseline-simd-neutral` һәм `baseline-avx2` үлсәүҙәре:

- **SIMD-нейтраль:** Apple кремнийында `ring` крипто бэкэнд өсөн NEON өсөн үтәй.
  АБИ дөрөҫлөк. Функцияны өҙөү (`RUSTFLAGS="-C target-feature=-neon"` X)
  ҡоролма етештереү алдынан төҙөүҙе туҡтатып (`docs/source/confidential_assets_calibration_simd_neutral_attempt_20260428.log`).
- **AVX2:** Урындағы инструменттар слесаһы x86_64 бинарҙарҙы тыуҙыра алмай (`arch -x86_64 rustc -V`
  → “Насар процессор төрө башҡарыла”; күрергә
  `docs/source/confidential_assets_calibration_avx2_attempt_20260428.log`).

CI тиклем хосттар `bench-x86-neon0` һәм `bench-x86-avx2a` онлайн, NEON йүгерә
өҫтә плюс телеметрия дәлилдәре ҡәнәғәтләндерә PhaseC ҡабул итеү критерийҙары.
Отказ теркәлгән `status.md` һәм ҡабаттан ҡараласаҡ бер тапҡыр х86 аппаратураһы 2000 йылда .
асыҡ.
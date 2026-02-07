---
lang: ba
direction: ltr
source: docs/source/benchmarks/history.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3aad1366bd823bddaca32dc82573d41ec6572a6d9f969dc1e0c6146ea068e03e
source_last_modified: "2025-12-29T18:16:35.920451+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# GPU эталон тотоу тарихы (FASTPQ WP5-B)

Был файл `python3 scripts/fastpq/update_benchmark_history.py` тарафынан генерациялана.
Ул ҡәнәғәтләндерә FASTPQ этап 7 WP5-B тапшырыу, һәр уратып алынған GPU күҙәтеү .
ориентирлы артефакт, Посейдон микроэскионы күренә, ә ярҙамсы һыпыртыуҙар аҫтында
`benchmarks/`. Яңыртыу нигеҙендә тотоп һәм сценарийҙы ҡабаттан эшләү, ҡасан яңы .
өйөм ерҙәре йәки телеметрия яңы дәлилдәргә мохтаж.

## Күсермә һәм яңыртыу процесы

- Яңы GPU-ның ҡулға алыуҙары (`scripts/fastpq/wrap_benchmark.py` аша) етештереү йәки урап,
  уларҙы тотоу матрицаһына ҡушыла, һәм был генераторҙы яңынан эшләтергә, яңыртыу өсөн
  өҫтәлдәр.
- Посейдон микроэлендр мәғлүмәттәре булғанда, уны экспортлау менән .
  `scripts/fastpq/export_poseidon_microbench.py`
  `scripts/fastpq/aggregate_poseidon_microbench.py`.
- Рекорд Меркл сиге һепереп, уларҙы һаҡлау JSON сығыштары аҫтында .
  `benchmarks/merkle_threshold/`; был генератор билдәле файлдарҙы шулай аудит исемлегенә индерә
  мөмкин кросс-референт процессор vs GPU доступность.

## FASTPQ этап 7 GPU эталондары

| Бундл | Бэкэнд | Режим | ГПУ бэкэнд | ГПУ доступный | Ҡоролма класы | ГПУ | LDE мс (CPU/GPU/SU) | Посейдон мс (CPU/GPU/SU) |
|------|--------|---------------------|--------------|-------------------------------------------. -----|-----|---------------------------------------------------||
| `fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json` | cuda | гпу | cuda-sm80 | эйе | хеон-rtx | NVIDIA RTX 6000 Ада | 1512,9/880.7/1,72 | —/—/— |
| `fastpq_metal_bench_2025-11-07T123018Z_macos14_arm64.json` | металл | гпу | бер ниндәй ҙә | эйе | алма-м4 | Apple GPU 40-ядро | 785,6/735,6/1,07 | 1803.8/1897.5/0,95 |
| `fastpq_metal_bench_20251108T192645Z_macos14_arm64.json` | металл | гпу | металл | эйе | алма-м2-ультра | Apple М2 Ультра | 1581.1/1604.5/0,98 | 3589.9/3697.3/0,97 |
| `fastpq_metal_bench_20251108T225946_macos_arm64.json` | металл | гпу | металл | эйе | алма-м2-ультра | Apple М2 Ультра | 1804.5/1666,4/1,08 | 3939.5/4083.3/0,96 |
| `fastpq_metal_bench_20251108T231910_macos_arm64_withtrace.json` | металл | гпу | металл | эйе | алма-м2-ультра | Apple М2 Ультра | 1804.5/1666,4/1,08 | 3939.5/4083.3/0,96 |
| `fastpq_opencl_bench_2025-11-18T074455Z_ubuntu24_aarch64.json` | opencl | гпу | opencl | эйе | неверо-ми300 | AMD Instinct MI300A | 4518.5/688.9/6,56 | 2780.4/905.6/3,07 |

> Бағана: `Backend` өйөм исеменән алынған; `Mode`/`GPU backend`/`GPU available`.
> уралған `benchmarks` блокынан күсерелгән, процессор йәки юҡ GPU fallbacks фашлау өсөн
> асыш (мәҫәлән, `gpu_backend=none` `Mode=gpu` X-ҡа ҡарамаҫтан). СУ = тиҙлек нисбәте (КПУ/ГПУ).

## Посейдон Микробобенч оснаялар

`benchmarks/poseidon/manifest.json` агрегаттары ғәҙәттәгесә vs-скаляр Посейдон
микроэлендр йүгерә экспортҡа һәр Металл өйөмө. Түбәндәге таблицала 2012 йылға тиклем яңыртыла.
генератор сценарийы, шуға күрә CI һәм идара итеү рецензиялары тарихи тиҙлекте айыра ала
уралған FASTPQ отчеттарын асыуһыҙ.

| Йәмғеһе | Бундл | Ваҡыт тамғаһы | Ғәҙәттәге мс | Скаляр мс | Тиҙлек |
|--------|--------|----------------------------------------------------||
| `benchmarks/poseidon/poseidon_microbench_debug.json` | `fastpq_metal_bench_debug.json` | 2025-11-09Т06:11:01Z | 2167.7 | 2152.2 | 0.99 |
| `benchmarks/poseidon/poseidon_microbench_full.json` | `fastpq_metal_bench_full.json` | 2025-11-09Т06:04:07Z | 1990.5 | 1994.5 | 1.00 |

## Меркл порогы һыпыртаҺылтанма тотоу аша йыйылған аша .
`cargo run --release -p ivm --features metal --example merkle_threshold -- --json`
йәшәй `benchmarks/merkle_threshold/`. Исемлек яҙмалары күрһәтә, был алып барыусы
һепертке йүгергәндә Металл ҡоролмалары фашлана; GPU-ҡуйылған тотоу тураһында хәбәр итергә тейеш
`metal_available=true`.

- `benchmarks/merkle_threshold/macos14_arm64_cpu.json` — `metal_available=False`.
- `benchmarks/merkle_threshold/macos14_arm64_metal.json` — `metal_available=False`
- `benchmarks/merkle_threshold/takemiyacStudio.lan_25.0.0_arm64.json` — `metal_available=True`

Apple кремнийҙы тотоу (`takemiyacStudio.lan_25.0.0_arm64`) — `docs/source/benchmarks.md`-та ҡулланылған канонлы GPU база линияһы; macOS 14 яҙмалар процессор-тик база һыҙыҡтары булып ҡала, мөхиттәр өсөн, улар Metal ҡоролмаларын фашлай алмай.

## рәт-ҡулланыу снэпшоттары

Шаһиттар декодтары аша төшөрөлгән `scripts/fastpq/check_row_usage.py` тапшырыу иҫбатлау .
гаджет’s рәт һөҙөмтәлелеге. JSON артефакттарын `artifacts/fastpq_benchmarks/` буйынса тотоғоҙ
һәм был генератор аудиторҙар өсөн теркәлгән тапшырыу нисбәттәрен дөйөмләштерәсәк.

- `artifacts/fastpq_benchmarks/fastpq_row_usage_2025-02-01.json` — партиялар=2, күсермә_яслама авг=0,629 (мин=0,625, max=0,633)
- `artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json` — партиялар=2, трансфер_яратион avg=0,619 (мин=0,613, max=0,625)
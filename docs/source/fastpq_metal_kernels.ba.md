---
lang: ba
direction: ltr
source: docs/source/fastpq_metal_kernels.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a0022f5f9c53445d26876f0097635092b5c685d332bfa25b13243c584d358dfe
source_last_modified: "2026-01-05T09:28:12.006723+00:00"
translation_last_reviewed: 2026-02-07
title: FASTPQ Metal Kernel Suite
translator: machine-google-reviewed
---

# FASTPQ металл ядроһы люкс

Apple кремний бэкэнд караптары бер `fastpq.metallib`, унда һәр составы
Металл күләгәләү теле (МСЛ) ядроһы мәғлүм итеү менән шөғөлләнгән. Был иҫкәрмәһе аңлатыла .
ине
гарантиялар, тип эшләй GPU юл алмашыу менән скаляр fallback.

Каноник тормошҡа ашырыу 2012 йылда йәшәй.
`crates/fastpq_prover/metal/kernels/` һәм 2012 йылға тиклем төҙөлә.
`crates/fastpq_prover/build.rs` ҡасан ғына `fastpq-gpu` macOS өҫтөндә эшләй.
Йүгереп ваҡыт метамағлүмәттәр (`metal_kernel_descriptors`) түбәндәге мәғлүмәтте көҙгөләй, шулай итеп
ориентирҙар һәм диагностика шул уҡ факттарҙы өҫкә сығара ала Программалы рәүештә.【крат/Fastpq_prover/металл/ядролар/ntt_stag / ядролар/poseidon2.металл:1】【крат/Fastpq_prover/төҙөү.1】【крат/фаспк_провер/срк/метал.р. 248】

## Ядро инвентаризацияһы| Яҙыу нөктәһе | Операция | Тред төркөмө ҡапҡасы | Плитка сәхнә ҡапҡасы | Иҫкәрмәләр |
| ---------- | -------- | ---------------- | -------------- | ----- |
| `fastpq_fft_columns` | Алға FFT аша эҙ бағаналары | 256 еп | 32 этап | Ҡулланыу уртаҡ-хәтер плиткаһы өсөн тәүге этаптар һәм ҡулланыу кире масштаблау ҡасан планлаштырыусы IFFT режимы һорап.【крат/Fastpq_rover/metal/ntt_stag
| `fastpq_fft_post_tiling` | PFT/IFFT/LDE плитка тәрәнлегенә еткәндән һуң | 256 еп | — | Ҡалған күбәләктәрҙе туранан-тура ҡоролма хәтеренән сыға һәм хостҡа ҡайтҡансы һуңғы козечатка/кире факторҙарҙы үҙләштерә.【крат/тиҙ_провер/металл/нтт_стаж.металл:447】【краттар/тиҙ_провер/src/metal.rs:262】
| `fastpq_lde_columns` | Түбән дәрәжәле оҙайтыу бағаналары буйынса | 256 еп | 32 этап | Күсермәләр коэффициенттары баһалау буферына, конфигурацияланған косет менән плитка этаптарын башҡара һәм һуңғы этаптарҙы `fastpq_fft_post_tiling` X-ға ҡалдыра. кәрәк.【крат/Fastpq_prover/металл/ядролар/ntt_stag
| `poseidon_trace_fused` | Хэш бағаналары һәм иҫәпләү тәрәнлеге‐1 ата-әсәләр бер пропуск | 256 еп | — | Йүгереп, шул уҡ абсорбция/пермутация `poseidon_hash_columns`, япраҡтарҙы туранан-тура сығарыу буферына һаҡлай, һәм шунда уҡ һәр `(left,right)` пары `fastpq:v1:trace:node` домены буйынса йыйыла, шуға күрә `(⌈columns / 2⌉)` ата-әсәләр япраҡ киҫәгенән һуң төшә. Ҡайһы бер осраҡта был ҡатламды бөтөрөү өсөн һуңғы япраҡты һәм процессорҙы бөтөрөү өсөн дубликаттар һәм процессор fallback өсөн беренсе Меркл ҡатламы.
| `poseidon_permute` | Poseidon2 алмаштырыу (STATE_WIDTH = 3) | 256 еп | — | Тред төркөмдәре кэш түңәрәк константалар/МДС рәттәре preategroup хәтерендә, күсерергә МДС рәттәре пер-поток регистрҙары, һәм процесс хәлдәр 4-дәүләт өлөштәрендә шулай һәр тур даими fetch ҡабаттан ҡулланыла бер нисә дәүләт аша алға киткәнсе. Туралар тулыһынса асыҡ ҡала һәм һәр һыҙат һаман да бер нисә дәүләт йөрөй, ≥4096 логик ептәр диспетчерлыҡ гарантиялай. `FASTPQ_METAL_POSEIDON_LANES` / `FASTPQ_METAL_POSEIDON_BATCH` пинировать осоу киңлеге һәм бер һыҙат партияһы тергеҙмәйенсә, яңынан төҙөлмәй 1990 йылдарҙа был йүнәлештәге тикшеренеүҙең һөҙөмтәләре

Дескрипторҙары эшләү ваҡытында эшләй.
`fastpq_prover::metal_kernel_descriptors()` инструменттар өсөн, тип күрһәтергә теләй
шул уҡ метамағлүмәттәр.

## Детерминистик Голдилоктар арифметика- Бөтә ядролар ҙа Goldilocks яланында эшләй, ярҙамсылары менән билдәләнгән 2012 йылда билдәләнгән ярҙамсылары.
  `field.metal` (модулле өҫтәү/мул/су, кире, `pow5`).【крет/Fastpq_rover/металл/ядро/ялан.металл:1】
- FFT/LDE этаптары бер үк twiddle таблицаларын ҡабаттан ҡулланыу, процессор планлаштырыусы етештерә.
  `compute_stage_twiddles` бер этапта һәм алып барыусыны бер твиддлды алдан иҫәпләй
  массив аша тейәп буфер слот 1 һәр диспетчер алдынан, гарантия бирә
  ГПУ юлында берҙәмлектең бер үк тамырҙары ҡулланыла.【крат/Fastpq_prover/src/metal.rs:1527】
- Cetse ҡабатлау өсөн LDE һуңғы этапҡа ҡушыла, шуға күрә GPU бер ҡасан да
  процессор эҙҙәре макетынан айырыла; хост нуль-һалым баһалау буферы
  диспетчер алдынан, һаҡлау тәртибе детерминистик.【крат/Fastpq_prover/металл/ядролар/ntt_stag

## Металлиб быуыны

`crates/fastpq_prover/metal/kernels/` айырым `.metal` сығанаҡтарын `.air` объекттарына, ә һуңынан 1990 йылда төҙөй.
уларҙы `fastpq.metallib`-ға бәйләй, өҫтә күрһәтелгән һәр инеү нөктәһен экспортлай.
`FASTPQ_METAL_LIB` был юлға ҡуйыу (төҙөү сценарийы быны эшләй
автоматик рәүештә) эшләү ваҡыты китапхананы детерминистик рәүештә тейәү мөмкинлеге бирә
Ҡайҙа `cargo` төҙөү артефакттарын урынлаштырған.【крат/Fastpq_prover/build.rs:45】

CI йүгерә менән паритет өсөн һеҙ китапхананы ҡул менән яңырта ала:

```bash
export OUT_DIR=$PWD/target/metal && mkdir -p "$OUT_DIR"
xcrun metal -std=metal3.0 -O3 -c crates/fastpq_prover/metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"
xcrun metal -std=metal3.0 -O3 -c crates/fastpq_prover/metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"
xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"
export FASTPQ_METAL_LIB="$OUT_DIR/fastpq.metallib"
```

## Этхагруппа размерлау эвристикаһы

`metal_config::fft_tuning` ептәре ҡоролма башҡарыу киңлеге һәм макс ептәр бер .
2012 йылда планлаштырыусыға инә, шуға күрә эшләү ваҡыты диспетчерҙары аппарат сиктәрен хөрмәт итә.
32/64/128/256 һыҙаттарына тиклем лог-ҙурлыҡ артыу менән зажимдар ғәҙәттәгесә, һәм
плитка тәрәнлеге хәҙер биш этаптан дүрткә тиклем йәйәү `log_len ≥ 12` X, һуңынан һаҡлай
12/14/16 этаптар өсөн әүҙем үткән дөйөм хәтер тапшырыу бер тапҡыр эҙ крест
```bash
export OUT_DIR=$PWD/target/metal && mkdir -p "$OUT_DIR"
xcrun metal -std=metal3.0 -O3 -c crates/fastpq_prover/metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"
xcrun metal -std=metal3.0 -O3 -c crates/fastpq_prover/metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"
xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"
export FASTPQ_METAL_LIB="$OUT_DIR/fastpq.metallib"
``` эште плитканан һуңғы ядроға тапшырыр алдынан. Оператор
өҫтөнлөктәре (`FASTPQ_METAL_FFT_LANES`, `FASTPQ_METAL_FFT_TILE_STAGES`) ағымы аша .
`FftArgs::threadgroup_lanes`/`local_stage_limit` һәм ядролар тарафынан ҡулланыла
өҫтәге металл тергеҙмәйенсә.【крат/Fastpq_prover/src/metal_config.s:12】【крат/Fastpq_prover/src/metal.rs:59】

Ҡулланыу `fastpq_metal_bench` хәл ителгән көйләү ҡиммәттәрен тотоу һәм раҫлау өсөн, тип
күп пропуск ядролары ғәмәлгә ашырылған (`post_tile_dispatches`X JSON) тиклем .
ориентир өйөмөн ташыу.【крат/Fastpq_prover/src/bin/fastpq_metal_bench.rs:1048】
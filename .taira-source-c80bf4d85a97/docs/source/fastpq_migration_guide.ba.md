---
lang: ba
direction: ltr
source: docs/source/fastpq_migration_guide.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 99e435a831d793035e71915ca567d3e61cb28b89627e0cf0ebdec72aa57a981d
source_last_modified: "2026-01-05T09:28:12.006936+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

#! FASTPQ етештереү миграцияһы ҡулланмаһы

Был runbook нисек раҫлау Stage6 етештереү FASTPQ иҫбатлаусы һүрәтләнә.
Был миграция планы өлөшө булараҡ детерминистик урын хужаһы бекэнд алынған.
Ул `docs/source/fastpq_plan.md`-та сәхнәләштерелгән планды тулыландыра һәм һеҙ инде күҙәтеп тора, тип фаразлай
эш урыны статусы `status.md`.

## Аудитория & Скоп
- Валидатор операторҙары етештереүсе йәки төп селтәрҙә производство иҫбатлаусыһын йәйеп сығара.
- Производство бэкэнд менән ебәрәсәк бинар йәки контейнерҙар булдырыу инженерҙарын сығарыу.
- SRE/күҙәтеүсәнлек командалары яңы телеметрия сигналдары һәм иҫкәрткән проводка.

Күләмдән: Kotodama контракт авторлыҡ һәм IVM ABI үҙгәрештәр (ҡара: `docs/source/nexus.md` өсөн өсөн
башҡарыу моделе).

## Функциональ матрица
| Юл | Йөк функциялары | Һөҙөмтә | Ҡасан ҡулланырға |
| ---- | ------------------------- | ----- | ---------- |
| Етештереүҙең иҫбатлаусы (по умолчанию) | _none_ | Stage6 FASTPQ фон менән FFT/LDE планлаштырыусы һәм DEEP-FRI торба. Бөтә продюсерлыҡ бинарҙары өсөн ғәҙәттәгесә. |
| Опциональ ГПУ тиҙләнеше | `fastpq_prover/fastpq-gpu` | CUDA/Металаль ядролар менән автоматик процессор fallback.【крат/Fastpq_prover/Cargo.toml:9】【крат/Fastpq_prover/src/ft.rs:124】 | Ярҙамсы тиҙләткестәр менән хужалар. |

## Процедура төҙөү
1. **Crop-тик төҙөү**
   ```bash
   cargo build --release -p irohad
   cargo build --release -p iroha_cli
   ```
   Производство бекэнд ғәҙәттәгесә төҙөлә; өҫтәмә функциялар кәрәкмәй.

2. **ГПУ-ға яраҡлы төҙөү (теләкле)**
   ```bash
   export FASTPQ_GPU=auto        # honour GPU detection at build-time helpers
   cargo build --release -p irohad --features fastpq_prover/fastpq-gpu
   ```
   GPU ярҙам SM80+ CUDA инструменттары менән `nvcc` төҙөү ваҡытында доступный.

3. **Үҙ-үҙеңде һынайҙар**
   ```bash
   cargo test -p fastpq_prover
   ``` X
   Йүгереп, был бер тапҡыр төҙөү төҙөү өсөн раҫлау өсөн Stage6 юл ҡаплау алдынан.

### Металл инструменттар сылбырын әҙерләү (macOS)
1. Төҙөү алдынан Металл команда юлдары ҡоралдарын ҡуйыу: `xcode-select --install` (әгәр CLI ҡоралдары юҡ икән) һәм `xcodebuild -downloadComponent MetalToolchain` GPU инструменттарын алыу өсөн. 1990 йылдарҙа был йүнәлештәге тикшеренеүҙең нигеҙендә туранан-тура һәм тиҙ уңышһыҙлыҡҡа осрай.
2. CI алдынан торба үткәргес раҫлау өсөн, һеҙ локаль төҙөү сценарийын көҙгөләй ала:
   ```bash
   export OUT_DIR=$PWD/target/metal && mkdir -p "$OUT_DIR"
   xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"
   xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"
   xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"
   export FASTPQ_METAL_LIB="$OUT_DIR/fastpq.metallib"
   ```
   Ҡасан был уңышлы төҙөү `FASTPQ_METAL_LIB=<path>` сығара; 188】【кровер/src/src/metal.s:43】
. 2012 йылда был йүнәлештәге эштәрҙе ойоштороусы ҡала
4. Төйөндәр автоматик рәүештә процессорға кире төшә, әгәр Металл доступный түгел (балаһы юҡ, ярҙамһыҙ GPU, йәки буш `FASTPQ_METAL_LIB`); 1990 йылдарҙа был йүнәлештәге эшмәкәрлекте үҫтереүҙең төп маҡсаты булып тора.### Шикләү исемлеге (Стаж6)
FASTPQ релиз билетын һаҡлау өсөн блокировка тиклем һәр пункт түбән тулы һәм беркетелгән.

1. **Суб-икенсе иҫбатлау метрикаһы** — Яңы тотолған `fastpq_metal_bench_*.json` һәм
   раҫлау `benchmarks.operations` яҙма, унда `operation = "lde"` (һәм көҙгөлө
   `report.operations` өлгөһө) хәбәр итә `gpu_mean_ms ≤ 950` өсөн 20000-рәт эш йөкләмәһе (32768 прокладка
   рәттәр). Түшәмдән ситтәге тотоуҙар тикшерелгән исемлеккә ҡул ҡуйырға мөмкин булғанға тиклем ҡайтанан эшләтеүҙе талап итә.
2. **Беренсе манифест** — Йүгерергә
   `cargo xtask fastpq-bench-manifest --bench metal=<json> --bench cuda=<json> --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key <path> --out artifacts/fastpq_bench_manifest.json`
   шулай итеп, релиз билет йөрөтә һәм манифест һәм уның айырым ҡултамғаһы
   (`artifacts/fastpq_bench_manifest.sig`). Рецензенттар раҫлай distest/ҡултамға пар алдынан .
   пропагандалау.【xtask/src/fastpq.:128】【xtask/src/src/main.rs:845】 Матрица манифест (төҙөү
   `scripts/fastpq/capture_matrix.sh` аша) инде 20к рәт ҡатты кодлай һәм
   регрессияны отладка.
3. **Дәлилдәр ҡушымталары** — Металл эталон JSON тейәү, stdout журналы (йәки инструменттар эҙ),
   CUDA/Metal манифест сығыштары, һәм айырым ҡултамғаһы релиз билет. Тикшерелеү исемлеге яҙмаһы
   тейеш һылтанма бөтә артефакттар плюс асыҡ асҡыс бармаҡ эҙҙәре өсөн ҡулланылған ҡул ҡуйыу шулай аҫҡы аудит
   тикшерелгән аҙымды ҡабатлай ала.【арфакттары/Fastpq_estmarks/README.md:65】### Металл раҫлау эш ағымы
1. Һуң GPU-ҡоролма, раҫлау `FASTPQ_METAL_LIB` мәрәйҙәре `.metallib` (`echo $FASTPQ_METAL_LIB`) шулай эшләү ваҡыты тейәп була, уны детерминистик.
2. Паритет люкс менән GPU һыҙаттары менән мәжбүр иткән:\
   `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq_prover/fastpq-gpu --release`. Бэкэнд Металл ядроларын тормошҡа ашырасаҡ һәм детерминистик процессор fallback логин, әгәр асыҡлау етешһеҙлектәр.【крат/scruver/src/back
3. Приборҙар таҡталары өсөн эталон өлгөһөн тотоп:\
   компиляцияланған Металл китапханаһын (`fd -g 'fastpq.metallib' target/release/build | head -n1`) урынлаштыра.
   уны `FASTPQ_METAL_LIB` аша экспортҡа сығара, ә йүгерә\
  `cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`.
  32,768 рәткә тиклем һәр тотоуын хәҙер канонлы `fastpq-lane-balanced` профиле (211) тиклем йүнәлә, шуға күрә JSON `rows` һәм `padded_rows` менән бергә Metal LDE латентлығы менән бергә йөрөтә; 18NI000000046X йәки сират параметрҙары 950мс (<1s) маҡсаттан тыш AppleM-серия хосттарында этәрергә, әгәр ҙә тотоуҙы ҡабаттан эшләтергә. Архив һөҙөмтәлә JSON/лог башҡа башҡа дәлилдәр менән бер рәттән; Төнгө macOS эш ағымы шул уҡ йүгереүҙе башҡара һәм сағыштырыу өсөн уның артефакттарын тейәп ебәрә.【крат/фаспq_prover/src/bin/fastpq_metal_bench.697】【.github/fastp-металл-төн.1】
  Ҡасан һеҙгә кәрәк Посейдон-тик телеметрия (мәҫәлән, теркәү өсөн инструменттар эҙ), өҫтәргә `--operation poseidon_hash_columns` өҫтәге командаға; эскәмйә һаман да хөрмәт итәсәк `FASTPQ_GPU=gpu`, `metal_dispatch_queue.poseidon` Emit, һәм яңы Prometheus блокты үҙ эсенә ала, шуға күрә релиз өйөмө документтары Посейдон тар муйыны асыҡтан-асыҡ.
  Дәлилдәр хәҙер `zero_fill.{bytes,ms,queue_delta}` плюс `kernel_profiles` (бер ядро ​​.
  занятость, баһаланған ГБ/с, һәм оҙайлылыҡ статистика) шулай итеп, GPU һөҙөмтәлелеге графигировать мөмкин.
  сеймал эҙҙәрен ҡайтанан эшкәрткән, һәм `twiddle_cache` блок (хит/мисс + `before_ms`/`after_ms`) тип.
  иҫбатлай кэш-твиддл тейәүҙәр ғәмәлдә. `--trace-dir` XX йүгәнен яңынан эшләтеп ебәрә.
  `xcrun xctrace record` һәм
  JSON менән бер рәттән `.trace` файлы ваҡыт тамғаһы менән бергә һаҡлана; һеҙ һаман да заказ буйынса тәьмин итә ала
  `--trace-output` X (факультатив `--trace-template` менән / `--trace-seconds`) ҡасан төшөрөлгән
  ҡулланыусылар өсөн урын/шаблон. JSON яҙмалары `metal_trace_{template,seconds,output}` аудит өсөн.【крат/Fastpq_rover/src/bin/fastpq_metal_bech.rs:177】Һуңынан һәр тотоу йүгерә `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json` шулай баҫма йөрөтә хужа метамағлүмәттәр (хәҙер шул иҫәптән `metadata.metal_trace`) өсөн Grafana платаһы/иҫкәртмә өйөм (`dashboards/grafana/fastpq_acceleration.json`, `dashboards/alerts/fastpq_acceleration_rules.yml`). Отчет хәҙер `speedup` объекты бер операция (`speedup.ratio`, `speedup.delta_ms`), урау күтәрмәһе `zero_fill_hotspots` (байте, latency, алынған GB/s, һәм металл сираты дельта иҫәпләүселәр), тигеҙтен `kernel_profiles` `benchmarks.kernel_summary`, Prometheus блок бөтөн һаҡлай, яңы `post_tile_dispatches` блок/йомғаҡлау күсермәләрен күсерә, шуға күрә рецензенттар күп пропуск ядроһын иҫбатлай ала, ә хәҙер посейдон микроэлочка дәлилдәрен дөйөмләштерә. `benchmarks.poseidon_microbench` шулай приборҙар таҡталары сеймал отчетын яҙып тормайынса скаляр-вс-поручивающий латентлыҡ цитаталай ала. Манифест ҡапҡаһы бер үк блок уҡый һәм кире ҡаға GPU дәлилдәр өйөмдәре, уны үткәрмәй, операторҙарҙы яңыртырға мәжбүр итә, ҡасан да булһа, плиталы юл үткәргес йәки үткәргес йәки . 1048】【Фастпк /wrap_benchmark.py:714】【сценарий/Fastpq/wrap_benchmark.py:732】【xtask/src/fastpq. rsc.280】
  Poseidon2 Металл ядроһы шул уҡ ручкалар менән уртаҡлаша: `FASTPQ_METAL_POSEIDON_LANES` (32–256, ике ҡөҙрәте) һәм `FASTPQ_METAL_POSEIDON_BATCH` (1–32 штат) һеҙгә осоу киңлеген һәм һыҙат буйынса эштәрҙе яңынан төҙөмәйенсә ҡаҙаҡларға мөмкинлек бирә; хост ептәре был ҡиммәттәр аша `PoseidonArgs` һәр диспетчер алдынан. 18NI000000080X ≥48GiB тураһында хәбәр ителгәндә, `256×20` 32GiB, `256×16` башҡаса, шул уҡ ваҡытта түбән ҡөҙрәтле SoCs. `256×8` (һәм оло 128/64 һыҙатлы өлөштәрҙә 8/6 штатҡа йәбешә), шуға күрә күпселек операторҙар бер ҡасан да env vars ҡул менән ҡуйырға кәрәкмәй. `fastpq_metal_bench` хәҙер үҙен яңынан башҡара `FASTPQ_METAL_POSEIDON_MICRO_MODE={default,scalar}` һәм `poseidon_microbench` блокты сығара, ул ике старт профилдәрен дә яҙып ала, плюс үлсәүҙәргә ҡаршы скаляр һыҙатҡа ҡаршы, шуға күрә яңы ядроның өйөмдө иҫбатлай ала, ысынында `poseidon_hash_columns`, һәм ул үҙ эсенә ала `poseidon_pipeline` блок шулай Stage7 дәлилдәре яңы йәшәү ярустары менән бер рәттән өлөшө тәрәнлеге/өҫтөн ручкалар тота. Ғәҙәти йүгереүҙәр өсөн env-ны ҡуйылмаған ҡалдырырға; жгут автоматик рәүештә ҡабаттан башҡарыу менән идара итә, әгәр ҙә бала тотоу етешһеҙлектәрен теркәй алмай, һәм шунда уҡ сыға, ҡасан `FASTPQ_GPU=gpu` ҡуйылған, әммә бер ниндәй ҙә GPU бэкэнд бар, шуға күрә өнһөҙ процессор fallbacks бер ҡасан да йәшеренеп перф 1988 йылда артефакттар.18NI0000000900X дельтаһы, дөйөм `column_staging` иҫәпләүселәре йәки Prometheus/Prometheus-сө һанлы дәлилдәр блоктарын юғалтҡан, улар үҙҙәренең йәки Prometheus/Prometheus скаляр-вс-поэфолт тиҙлекте.【scripts/Fastpq/wrap_benchmark.py:732】 Ҡасан һеҙгә кәрәк булған автономлы JSON өсөн приборҙар таҡталары йәки CI дельта, эшләй `python3 scripts/fastpq/export_poseidon_microbench.py --bundle <bundle>`; ярҙамсы ҡабул итә һәм уралған артефакттар һәм сеймал Grafana тотоу, `benchmarks/poseidon/poseidon_microbench_<timestamp>.json` менән сығарыу менән ғәҙәттәге/скаляр ваҡыт, көйләү метамағлүмәттәр, һәм теркәлгән тиҙлек.
  `cargo xtask fastpq-bench-manifest --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key secrets/fastpq_bench.ed25519 --out artifacts/fastpq_bench_manifest.json` атҡарып сыға, шулай итеп, Stage6 релиз тикшерелгән исемлеге `<1 s` LDE түшәмде үтәй һәм ҡултамғалы манифест/ашлайst өйөмөн сығара, улар менән сығарыла билет.【xtask/src/fastpq.:1】【арфакттары/Fastpq_estmarks/README.md:65】
4. Телеметрияны тикшерергә тиклем таратыу: Prometheus ос нөктәһе (```bash
   cargo test -p fastpq_prover
   ```) һәм тикшерергә Prometheus өсөн көтөлмәгән `resolved="cpu"` өсөн журналдар. Яҙмалар.【крат/ироха_телеметрия/срк/метрия.rs.887】【крат/Fastpq_rover/src/back
5. Документ процессор юлында fallback юл мәжбүр итеү, уны аңлы рәүештә (`FASTPQ_GPU=cpu` йәки `zk.fastpq.execution_mode = "cpu"`) шулай SRE плейбуктар менән тура килә детерминистик 1357】6. Опциональ көйләү: хужаның ҡыҫҡа эҙҙәре өсөн 16 һыҙат һайлай, уртаса өсөн 32, ә 64/128 бер тапҡыр `log_len ≥ 10/14`, 256-ға төшкәс, ҡасан `log_len ≥ 18`, һәм ул хәҙер дөйөм хәтер плитен бәләкәй эҙҙәр өсөн, дүрт тапҡыр бер тапҡыр һаҡлай. `log_len ≥ 12`, һәм 12/14/16 этапта `log_len ≥ 18/20/22` өсөн эш тибеп, плитка ядроһына тиклем. Экспорт `FASTPQ_METAL_FFT_LANES` (ҡөҙрәт-ике араһында 8and256) һәм/йәки `FASTPQ_METAL_FFT_TILE_STAGES` (1–16) өҫтәге аҙымдарҙы етәкләү алдынан, был эвристиканы өҫтөнән үткәреү өсөн. FFT/IFFT һәм LDE бағанаһы партияһының ҙурлыҡтары ла хәл ителгән еп төркөмө киңлегенән (≈2048 логик ептәр диспетчер, 32 бағана менән ҡапланған, ә хәҙер 32→16→4→2→1 аша ratcheting домен үҫә), ә LDE юлы һаман да уның домен ҡапҡастарын үтәй; `FASTPQ_METAL_FFT_COLUMNS` (1–32) ҡуйылған, детерминистик FFT партияһының күләмен һәм `FASTPQ_METAL_LDE_COLUMNS` (1–32) ҡаҙаҡлау өсөн, шул уҡ өҫтөнлөктө LDE диспетчерына ҡулланыу өсөн, ҡасан һеҙгә кәрәк бит-бит сағыштырыуҙары хосттар буйынса. LDE плитка тәрәнлеге көҙгө FFT эвристика, шулай уҡ-эҙҙәр менән ```toml
   [zk.fastpq]
   execution_mode = "auto"   # or "cpu"/"gpu"
   ``` тик йүгерә 12/10/8 дөйөм-хәтер этаптары киң күбәләктәрҙе тапшырыр алдынан пост-плита ядроһы-һәм һеҙ был сик аша өҫтөндә im18NI00000113X (1–32). 1990 йылдарҙа был йүнәлештәге эштәрҙең иң мөһимдәренең береһе булып тора. 1991 йылда был йүнәлештәге эштәрҙең иң мөһимдәренең береһе булып тора. JSON өҫтө өҫтө һәм хәл ителгән көйләү һәм хужа нуль тултырыу бюджеты (`zero_fill.{bytes,ms,queue_delta}`) LDE статистика аша төшөрөлгән, шулай сират дельталары туранан-тура һәр тотоуға бәйләнгән, һәм хәҙер өҫтәй ```bash
  cargo test -p fastpq_prover --test backend_regression --release -- --ignored
  ``` блок (парттар тигеҙләнгән, тигеҙләнгән_ms, wail_mys, wail_ms, way_namy) шулай итеп, рецензенттар раҫлай ала хост/ҡоролма ҡапланыу индерелгән ике буферлы торба. Ҡасан GPU нуль-тулы телеметрия тураһында хәбәр итеүҙән баш тарта, жгут хәҙер синтезлай детерминистик ваҡыт хужа яғы буферы таҙарта һәм уны `zero_fill` блокҡа индерә, шуға күрә сығарыу дәлилдәре бер ҡасан да судноларһыҙ. ялан. 1609】【крет/тиҙ_провер/src/bin/fastpq_metal_bectal_bench.rs. 1860】7. Күп сират диспетчерлыҡ автоматик дискрет Macs: ҡасан `Device::is_low_power()` ялған ҡайтара йәки Металл ҡоролмаһы хәбәр итә Слот/Тышҡы урын хужаһы экземпляр ике `docs/source/nexus.md`s, тик көйәрмәндәр сығарып бер тапҡыр эш йөкләмәһе йөрөтә ≥16 колонка (фантазия буйынса масштаблы), һәм түңәрәк-робиндар бағана партиялары буйынса сираттар шул тиклем оҙон эҙҙәр ике GPU һыҙаттарын һаҡлай, детерминизмды боҙмайынса. `FASTPQ_METAL_QUEUE_FANOUT` (1–4 сират) һәм `FASTPQ_METAL_COLUMN_THRESHOLD` (фантариатҡа тиклем минималь дөйөм бағана) менән сәйәсәтте өҫтөнән өҫтөнә ҡуйырға; паритет һынауҙары көс, был өҫтөнлөктәр шулай күп GPU Macs ҡала ҡапланған һәм хәл ителгән вентилятор-аут/сик логин янында логин-тәрән Телеметрия.【крат/фаспк_провер/срк/металл.р. 620】【крат/фаспк_провер/срк/метал.р.ср. 900】【крат/Fastpq_prover/src/metal.rs:254】### Архивҡа дәлилдәр
| Артефакт | Ҡулға алыу | Иҫкәрмәләр |
|---------|---------|--------|
| `.metallib` өйөм | `xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"` һәм `xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"`, унан һуң `xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"` һәм `export FASTPQ_METAL_LIB=$OUT_DIR/fastpq.metallib`. | Металл CLI/инструментальчейнды иҫбатлай һәм был коммит өсөн детерминистик китапхана етештерелгән.【крат/Fastpq_rover/төҙөү.98】【крат/Fastpq_prover/build.rs:188】 |
| Тирә-яҡ мөхит снимок | `echo $FASTPQ_METAL_LIB` XX төҙөүҙән һуң; абсолют юлды һаҡлау менән һеҙҙең релиз билет. | Буш сығыу тигәнде аңлата Металл өҙөлгән булды; ҡиммәтле документтарҙы теркәү, тип GPU һыҙаттары ҡала ташыу артефакт.【крат/Fastpq_prover/build.rs:188】【крат/Fastpq_prover/src/metal.rs:43】 |
| ГПУ паритет журналы | `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq-gpu --release` һәм архив өҙөк, унда `backend="metal"` йәки түбәнгә-сәләм тураһында иҫкәртмә. | 114】【крат/тиҙ_провер/src/backend.rs:195】 |
| Эйәр сығарыу | `FASTPQ_METAL_LIB=$(fd -g 'fastpq.metallib' target/release/build | head -n1) cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`; уратып һәм билдә аша Prometheus. | JSON яҙмалары `speedup.ratio`, `speedup.delta_ms`, FFT көйләү, рәттәр (32,768), байытылған `zero_fill`/`kernel_profiles`, тигеҙләнгән Grafana, тикшерелгән `metal_dispatch_queue.poseidon`/`poseidon_profiles` блоктар (ҡасан `--operation poseidon_hash_columns` ҡулланылған), һәм эҙ метамағлүмәттәр шулай GPU LDE уртаса ҡала ≤950мс һәм Посейдон ҡала <1s; 18NI0000000139X ҡултамғаһы менән генерацияланған өйөмдө лә, приборҙар таҡталары һәм аудиторҙары артефактты раҫлай ала. Эш йөкләмәләре.【крат/fastpq_prover/src/bin/fastpq_metal_bech.rys:697】【стрипс/Fastpq/wrap_benchmark.py:714】【спример/фаспк/урап.py:732】 |
| Эскәмйә манифест | `cargo xtask fastpq-bench-manifest --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key secrets/fastpq_bench.ed25519 --out artifacts/fastpq_bench_manifest.json`. | Ике GPU артефакттарын раҫлай, уңышһыҙлыҡҡа осрай, әгәр LDE тигәнде аңлата `<1 s` түшәм өҙөү, BLAKE3/SHA-256 дигесттарҙы теркәй һәм ҡултамғалы манифест сығара, шуға күрә релиз тикшерелгән исемлеге тикшерелмәгәндән һуң алға китә алмай. метрикаһы.【xtask/src/fastpq.:1】【арфакттары/фаспк_ышаймдар/README.md:65】 |
| CUDA өйөм | SM80 лабораторияһында Run `FASTPQ_GPU=gpu cargo run -p fastpq_prover --bin fastpq_cuda_bench --release -- --rows 20000 --iterations 5 --column-count 16 --device 0 --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json` Run, урап/ JSON `artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json`-ҡа уратып/тапшырыу (`--label device_class=xeon-rtx-sm80` ҡулланыу шулай приборҙар таҡталары дөрөҫ класты күтәреп), `artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt`-ға юлды өҫтәй һәм һаҡларға һәм 2000 й. `.json`/`.asc` пары, манифестты тергеҙеү алдынан металл артефакт менән. Тикшерелгән `fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json` аныҡ өйөм форматында аудиторҙар көтә.【срипт/фаспк/wrap_benchmark.pi:714】【арфакттары/Fastpq_bovers/матрица/ҡоролма/xeon-rtx-sm80.txt:1】 |
| Телеметрия дәлиле | `curl -s http://<host>:8180/metrics | rg 'fastpq_execution_mode_total{device_class'` плюс стартапта сығарылған `telemetry::fastpq.execution_mode` журналы. | Prometheus/OTEL `device_class="<matrix>", backend="metal"` (йәки трафикты индереү алдынан) фашлай.| Көсләп процессор бура | Ҡыҫҡа партия менән йүгерергә `FASTPQ_GPU=cpu` йәки `zk.fastpq.execution_mode = "cpu"` һәм төшөрөү журналы төшөрөү. | Һаҡлай SRE runbooks тура килтереп детерминистик fallback юл осраҡта кәрәк, тип кәрәк урта релиз.【крент/тиҙ_провер/src/backend.s:308】【крет/ироха_конфиг/срк/параметрҙар/пландар:1357】 |
| Эҙ тотоу (теләһәгеҙ) | Ҡабатлау паритет һынау менән `FASTPQ_METAL_TRACE=1 FASTPQ_GPU=gpu …` һәм экономия эмиссия диспетчер эҙ. | Һуңыраҡ профилләштереп тикшерелгән өсөн оппоннаждар/тредгруппинг персоналһыҙ ориентирҙарһыҙ.【крат/фаспк_провер/src/metal.rs:346】【крат/Fastpq_prover/src/бэкэнд.р.:208】 |

Күп телле `fastpq_plan.*` файлдар был тикшерелгән исемлеккә һылтанма яһай, шуға күрә стадия һәм етештереү операторҙары шул уҡ дәлилдәр эҙен үтәй.

## Ҡабатланырлыҡ биналар
Ҡулланыу өсөн контейнированный контейнер эш ағымы етештереү өсөн ҡабатланған Stage6 артефакттар:

```bash
scripts/fastpq/repro_build.sh --mode cpu                     # CPU-only toolchain
scripts/fastpq/repro_build.sh --mode gpu --output artifacts/fastpq-repro-gpu
scripts/fastpq/repro_build.sh --container-runtime podman     # Explicit runtime override
```

Ярҙамсы скрипт `rust:1.88.0-slim-bookworm` инструментальчейн һүрәтен (һәм GPU өсөн `nvidia/cuda:12.2.2-devel-ubuntu22.04`) төҙөй, контейнер эсендә төҙөй һәм `manifest.json`, `sha256s.txt`, һәм төҙөлгән бинарийҙар маҡсатлы сығышҡа яҙа. Каталог.【сприпттар/Fastpq/repro_build.s:1】【срипт/фаспк/run_inside_repro_build.s:1】【сприпаль/фаспк/докер/Dockerfile.gpu:1】

Тирә-яҡ мөхит өҫтөнлөк итә:
- `FASTPQ_RUST_IMAGE`, `FASTPQ_RUST_TOOLCHAIN` – асыҡтан-асыҡ Rust базаһы/тег штабель.
- `FASTPQ_CUDA_IMAGE` – GPU артефакттарын етештергәндә CUDA базаһын алмаштыра.
- `FASTPQ_CONTAINER_RUNTIME` – аныҡ эшләү ваҡытын мәжбүр итә; `auto` ғәҙәттәгесә `FASTPQ_CONTAINER_RUNTIME_FALLBACKS` һынап ҡарай.
- `FASTPQ_CONTAINER_RUNTIME_FALLBACKS` – воезнака-айырылған өҫтөнлөк тәртибе өсөн авто-авто-асыҡлау өсөн (постройсь `docker,podman,nerdctl` тиклем).

## Конфигурация яңыртыуҙары
1. Һеҙҙең TOML-да эшләү ваҡыты үтәү режимын ҡуйырға:
   ```toml
   [zk.fastpq]
   execution_mode = "auto"   # or "cpu"/"gpu"
   ```
   Ҡиммәте `FastpqExecutionMode` һәм ептәр аша стартапта анализлана.

2. Кәрәк булһа, стартта өҫтөнлөк бирегеҙ:
   ```bash
   irohad --fastpq-execution-mode gpu ...
   ```
   CLI өҫтөнлөк мутация хәл ителгән конфиг алдынан төйөн итектәр.【крат/ролада/src/main.rs:270】【крат/ролахад/src/main.rs:1733】

.
   `FASTPQ_GPU={auto,cpu,gpu}` XX бинарҙы эшләтеп ебәрер алдынан; өҫтөнлөк итеү логин һәм торба
   һаман да хәл ителгән режимды өҫкә күтәрә.【крат/Fastpq_rover/src/backend.rs.rs. 208】【крат/фаспк_провер/src/backend.rs:401】

## Тикшереү исемлеге
1. **Стартап журналдары**
   - `FASTPQ execution mode resolved` 18NI000000171X маҡсатлы 2000 йылда 18NI0000000170X менән көтөгөҙ.
     `requested`, `resolved`, һәм `backend` маркалары.【крат/Fastpq_rover/src/back
   - Автоматик GPU асыҡлау тураһында икенсел журнал `fastpq::planner` һуңғы һыҙат тураһында хәбәр итә.
   - Металл хужалары `backend="metal"` X металлиб йөктәре уңышлы булғанда; әгәр компиляция йәки тейәү уңышһыҙлыҡҡа осраһа, төҙөү сценарийы иҫкәртмә сығара, `FASTPQ_METAL_LIB`, һәм планлаштырыусы `GPU acceleration unavailable` теркәүселәргә тиклем ҡалғанға тиклем. 195 19952. **Prometheus метрикаһы**
   ```bash
   curl -s http://localhost:8180/metrics | rg 'fastpq_execution_mode_total{device_class'
   ```
   Прилавок `record_fastpq_execution_mode` аша инкрементациялана (хәҙер 1-се һанлы маркировкалана.
   `{device_class,chip_family,gpu_kind}`) төйөн уның башҡарыуын хәл иткән һайын
   режимы.【крат/ироха_телеметрия/срк/метрия.р.:8887】
   - Металл ҡаплау өсөн раҫлау
     `fastpq_execution_mode_total{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>", backend="metal"}`
     һеҙҙең менән бер рәттән өҫтәү приборҙар таҡталары.【крат/ироха_телеметрия/src/src/scr.rs:5397】
   - `irohad --features fastpq-gpu` менән төҙөлгән macOS төйөндәре өҫтәмә рәүештә фашлау
     `fastpq_metal_queue_ratio{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>",queue="global",metric="busy"}`
     һәм
     `fastpq_metal_queue_depth{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>",metric="limit"}` шулай Stage7 приборҙар таҡталары
     18NT0000000003X скретстарҙан алынған дежур цикл һәм сират башы.

3. **Телеметрия экспорты**
   - OTEL төҙөй, `fastpq.execution_mode_resolutions_total` шул уҡ лейблдар менән сығара; һеҙҙең тәьмин итеү
     приборҙар таҡталары йәки иҫкәртмәләр көтөлмәгән `resolved="cpu"` өсөн күҙәтә, ҡасан GPUs әүҙем булырға тейеш.

4. **Санит иҫбатлау/тикшерергә**
   - `iroha_cli` йәки интеграция йүгәне аша бәләкәй генә партияны эшләтеү һәм дәлилдәр раҫлауын раҫлағыҙ.
     тиңдәштәре бер үк параметрҙар менән төҙөлә.

## Төҙөкләндереүҙең
- **Бында хәл ителгән режим ҡала процессорында GPU хосттары** — тикшерергә, тип бинар төҙөлгән менән .
  `fastpq_prover/fastpq-gpu`, CUDA китапханалары тейәгес юлында, ә `FASTPQ_GPU` рәүешле көсләп түгел.
  `cpu`.
- **Металл Apple Silicon** — CLI ҡоралдарын раҫлай (`xcode-select --install`), `xcodebuild -downloadComponent MetalToolchain` передача, һәм төҙөү тәьмин итеү етештереү етештергән Prometheus юл; буш йәки юҡ ҡиммәте дизайн буйынса бекэндты өҙөп ебәрә.【крат/Fastpq_prover/төҙөлөш.166】【крат/Fastpq_prover/src/metal.rs:43】
- **`Unknown parameter` хаталар** — иҫбатлаусы һәм тикшерергә һәм бер үк канон каталогын ҡулланыуҙы тәьмин итеү
  `fastpq_isi` тарафынан сығарылған; тап килмәүе булараҡ IVM.【краттар/Fastpq_rover/src/bord.s:133】
- **Көтөлмәгән процессор fallback** — тикшерелгән `cargo tree -p fastpq_prover --features` һәм
  раҫлау `fastpq_prover/fastpq-gpu` GPU төҙөүҙәрендә бар; `nvcc`/CUDA китапханалары эҙләү юлында.
- **Телеметрия счетчик юҡ** — төйөндө раҫлау `--features telemetry` менән башланды (дефолт)
  һәм OTEL экспорты (әгәр ҙә өҫтәмә булһа) метрик торба инә.【крат/ироха_телеметрия/src/src/src/src.rs:8887】

## Fallback процедураһы
Детерминистик урын хужаһы бекэнд алынған. Әгәр регрессия кире ҡағыу талап итә,
reploy reploy элек билдәле-яҡшы сығарыу артефакттары һәм тикшерергә алдынан ҡабаттан сығарыу Stag6
бинарҙары. Документ үҙгәрештәр менән идара итеү ҡарары һәм тәьмин итеү өсөн алға ролл тамамланғандан һуң ғына тамамлана
регрессия аңлашыла.

3. Монитор телеметрия тәьмин итеү өсөн `fastpq_execution_mode_total{device_class="<matrix>", backend="none"}` көтөлгән сағылдыра
   урындарҙы үтәү.

## Аппараттың төп һыҙығы
| Профиль | Процесс | ГПУ | Иҫкәрмәләр |
| ------ | --- | --- | ----- |
| Һылтанма (Стаж6) | AMD EPYC7B12 (32 ядро), 256ГиБ оперативка | NVIDIA A10040GB (CUDA12.2) | 20000 рәт синтетик партиялары тамамларға тейеш ≤1000мс.【доктар/сығанаҡ/fastpq_plan.md:131】 |
| Процессор-тик | ≥32 физик ядролар, AVX2 | – | 20000 рәт өсөн ~0,9–1,2с көтөгөҙ; детерминизм өсөн `execution_mode = "cpu"` тотоу. |## Регрессия һынауҙары
- `cargo test -p fastpq_prover --release`
- `cargo test -p fastpq_prover --release --features fastpq_prover/fastpq-gpu` (ГПУ хужалары буйынса)
- Алтын алтын ҡоролма тикшерергә:
  ```bash
  cargo test -p fastpq_prover --test backend_regression --release -- --ignored
  ```

Документ ниндәй ҙә булһа тайпылыштар был тикшерелгән исемлек һеҙҙең опс runbook һәм яңыртыу `status.md` һуң .
миграция тәҙрәһе тамамлана.
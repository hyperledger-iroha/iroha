---
lang: ba
direction: ltr
source: docs/source/fastpq_rollout_playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3a0c22a213e04a6a8fef94ded6ec0017531737ffd4b9418ec94286bb6759ff8a
source_last_modified: "2026-01-08T09:53:05.148398+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# FASTPQ рулет плейбук (Stage7-3)

Был плейбук Stage7-3 юл картаһы талаптарын тормошҡа ашыра: һәр автопарк яңыртыу
был мөмкинлек бирә FASTPQ GPU башҡарыу беркетергә тейеш ҡабатланған эталон манифест,
парлы Grafana дәлилдәре, һәм документлаштырылған кире ҡайтарыу бурау. Ул тулыландыра .
`docs/source/fastpq_plan.md` (маҡсаттар/архитектура) һәм
`docs/source/fastpq_migration_guide.md` (төйөн кимәлендәге яңыртыу аҙымдары) фокуслау юлы менән
оператор-йөҙөндә тикшерелгән исемлектә.

## Скап һәм ролдәр

- **Инженерия сығарыу / SRE:** үҙ эталон тотоу, асыҡ ҡул ҡуйыу, һәм
  приборҙар таҡтаһы экспорты раҫлау алдынан ролл-аут.
- **Ops гильдия:** спектакль ролл-ауттары, репетицияларҙы кире ҡайтарыу һәм магазиндар эшләй.
  `artifacts/fastpq_rollouts/<timestamp>/` буйынса артефакт өйөмө.
- **Идара итеү / Ҡабул итеү:** дәлилдәрҙе раҫлай, һәр үҙгәреште оҙатып бара
  FASTPQ ғәҙәттәгесә флот өсөн ҡыҫҡартылғанға тиклем үтенес.

## Дәлилдәр пакет талаптары

Һәр таратыу тапшырыуында түбәндәге артефакттар булырға тейеш. Бөтә файлдарҙы беркетегеҙ
сығарыу/яңыртыу билеты һәм өйөмөн һаҡлау өсөн .
`artifacts/fastpq_rollouts/<YYYYMMDD>/<fleet>/<lane>/`.| Артефакт | Маҡсат | Нисек етештерергә |
|--------|----------|----------------|
| `fastpq_bench_manifest.json` | 20000 рәтле канонлы эш йөкләмәһе `<1 s` LDE түшәмендә ҡала һәм һәр уралған эталон өсөн хештарҙы теркәй.| Ҡабул итеү Металл/CUDA йүгерә, уларҙы урап, һуңынан Йүгереп:`cargo xtask fastpq-bench-manifest \`XGrafana`  --bench cuda=artifacts/fastpq_benchmarks/<cuda>.json \``  --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json \`Prometheus |
| Урап сыҡҡан ориентирҙар (`fastpq_metal_bench_*.json`, `fastpq_cuda_bench_*.json`) | Ҡабул итеү хужаһы метамағлүмәттәр, рәт-ҡулланыу дәлилдәре, нуль тултырыу ҡайнар нөктәләре, Посейдон микроэлемент резюмеһы, һәм ядро ​​статистикаһы ҡулланылған приборҙар таҡталары/иҫкәртмәләр.| Run `fastpq_metal_bench` / `fastpq_cuda_bench`, һуңынан сеймал JSON:`python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 \` XX`  --poseidon-metrics artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/metrics_poseidon.prom \``  fastpq_metal_bench.json artifacts/fastpq_benchmarks/<metal>.json --sign-output`Ҡабатлау өсөн CUDA тотоу (пункт `--row-usage` һәм `--poseidon-metrics` тейешле шаһит/скрап файлдарында). Ярҙамсы фильтрланған `fastpq_poseidon_pipeline_total`/`fastpq_execution_mode_total` өлгөләрен үҙ эсенә ала, шуға күрә WP2-E.6 дәлилдәре Metal һәм CUDA буйынса бер үк. Ҡулланыу `scripts/fastpq/export_poseidon_microbench.py --bundle <bundle>` ҡасан һеҙгә кәрәк автономлы Посейдон микроэлендр резюме (урап йәки сеймал индереүҙәр ярҙам итә). |
|  |  | **Stage7 этикетка талабы:** `wrap_benchmark.py` хәҙер уңышһыҙлыҡҡа осрай, әгәр һөҙөмтәлә `metadata.labels` бүлеге `device_class` һәм `gpu_kind`. Автоматик асыҡлау уларҙы һығымта яһағанда (мәҫәлән, айырым CI төйөнөнә урағанда), `--label device_class=xeon-rtx-sm80 --label gpu_kind=discrete` кеүек асыҡ өҫтөнлөктәрҙе үтә. |
|  |  | **Тиҙләтеү телеметрияһы:** урау шулай уҡ `cargo xtask acceleration-state --format json` ғәҙәттәгесә төшөрөп, `<bundle>.accel.json` һәм `<bundle>.accel.prom` уралған эталон эргәһендә яҙа (Prometheus флагы йәки `--skip-acceleration-state` менән өҫтөнлөк бирелә). Ҡабул итеү матрицаһы был файлдарҙы ҡулланып, автопарктар өсөн `acceleration_matrix.{json,md}` төҙөү өсөн ҡулланыла. |
| Grafana экспорты | Ҡабул итеү телеметрия һәм иҫкәртмә аннотациялар иҫбатлай өсөн таратыу тәҙрәһе.| Экспорт `fastpq-acceleration` приборҙар таҡтаһы:Grafana`  "$GRAFANA_URL/api/dashboards/uid/fastpq-acceleration" \``  | jq '.dashboard' \``  > artifacts/fastpq_rollouts/<stamp>/grafana_fastpq_acceleration.json`Аннотать плата менән идара итеү башланған/туҡтау тапҡыр экспортҡа тиклем. Был йәһәттән автоматик рәүештә `scripts/run_release_pipeline.py --export-fastpq-grafana --grafana-url <URL>` аша автоматик рәүештә эшләй ала (`GRAFANA_TOKEN` аша тәьмин ителгән токен). |
| Иҫкәртмә снимок | Ҡағиҙәләрҙе тота, тип иҫкәртмә ҡағиҙәләре һаҡлау ролл-аут.| Copy `dashboards/alerts/fastpq_acceleration_rules.yml` (һәм `tests/` ҡоролма) өйөмгә, шуға күрә рецензенттар `promtool test rules …` яңынан эшләй ала. |
| Rollback бурау журналы | Күрһәтергә, операторҙар репетиция мәжбүр процессор fallback һәм телеметрия таныуҙары.| Процедура ҡулланыу [Rollback Drills](#rollback-drills) һәм һаҡлау консоль журналдар (`rollback_drill.log`) плюс һөҙөмтәлә Prometheus (`metrics_rollback.prom`). || `row_usage/fastpq_row_usage_<date>.json` | Яҙмалар ExecWitness FASTPQ рәт бүленә, тип TF-5 тректар CI һәм приборҙар таҡталарында.| Torii-тан яңы шаһит скачать, уны `iroha_cli audit witness --decode exec.witness` аша decode (факультатив рәүештә `--fastpq-parameter fastpq-lane-balanced` өҫтәп, көтөлгән параметр йыйылмаһын раҫлай; FASTPQ партиялары дефолт аша сығарыла), һәм `row_usage` JSON-ды күсерергә `artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/row_usage/`. Һаҡлау файл исемдәре ваҡыт тамғаһы, шулай итеп, рецензенттар уларҙы корреляциялай ала, уларҙы таратыу билеты, һәм эшләй `python3 scripts/fastpq/validate_row_usage_snapshot.py row_usage/*.json` (йәки `make check-fastpq-rollout`) шулай итеп, Stage7-3 ҡапҡаһы раҫлай, һәр партия реклама селектор һаны һәм `transfer_ratio = transfer_rows / total_rows` инвариант дәлилдәрҙе беркеткәнсе. |

> **Кәңәш:** `artifacts/fastpq_rollouts/README.md` X-ға өҫтөнлөк бирелгән исемдәрҙе документлашты
> схема (`<stamp>/<fleet>/<lane>`) һәм кәрәкле дәлилдәр файлдары. 1990 й.
> `<stamp>` папкаһы `YYYYMMDDThhmmZ`-ты кодларға тейеш, шуға күрә артефакттар сорттарға бүлергә тейеш.
> билеттар менән кәңәшләшмәйенсә.

## Дәлилдәр быуыны тикшерелгән исемлек1. **ГПУ-ны тотоу ориентирҙары.**
   - Канонлы эш йөкләмәһен (20000 логик рәттәр, 32768 прокладкалы рәттәр) йүгерегеҙ.
     `cargo run -p fastpq_prover --bin fastpq_metal_bench -- --rows 20000 --pretty`.
   - `scripts/fastpq/wrap_benchmark.py` менән һөҙөмтәһен `--row-usage <decoded witness>` ҡулланып урап, шуға күрә өйөм ГПУ телеметрияһы менән бер рәттән гаджет дәлилдәрен йөрөтә. Pass `--require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 --sign-output` шулай урау тиҙ етешһеҙлеккә осрай, әгәр ҙә йәки акселератор маҡсаттан артып китә йәки әгәр Посейдон сираты/профиль телеметрияһы юҡ, һәм генерациялау өсөн айырым ҡултамға.
   - CUDA хостында ҡабатлау, шуға күрә манифестта ике GPU ғаиләһе лә бар.
   - **юҡ** `benchmarks.metal_dispatch_queue` йәки
     `benchmarks.zero_fill_hotspots` блоктар уралған JSON. CI ҡапҡаһы
     (`ci/check_fastpq_rollout.sh`) хәҙер был өлкәләрҙе уҡый һәм сиратҡа ҡасан уңышһыҙлыҡҡа осрай
     баш бүлмәһе бер слоттан түбән төшә йәки ҡасан теләһә ниндәй LDE ҡыҙыу нөктәһе хәбәр итә `mean_ms >
     0.40мс`, Stage7 телеметрия һаҡсыһы автоматик рәүештә үтәү.
2. **Манизитты генерациялау.** `cargo xtask fastpq-bench-manifest …` X тип ҡулланыу
   таблицала күрһәтелгән. `fastpq_bench_manifest.json` магазинында йәйелдерелгән өйөмдә.
3. **Экспорт Grafana.**
   - Аннотация `FASTPQ Acceleration Overview` платаһы менән йәйелдерелгән тәҙрә,
     бәйләнеше тейешле Grafana панель идентификаторҙары.
   - Export панель JSON аша Grafana API (өҫтөндә командование) һәм үҙ эсенә ала
     `annotations` бүлеге шулай рецензенттар ҡабул итеү ҡойроҡтарына тап килә ала
     сәхнәләштерелгән йәйелдерелгән.
4. **Сняпшот иҫкәртмәләр.** Теүәл иҫкәртмә ҡағиҙәләрен күсерергә (`dashboards/alerts/…`) ҡулланылған
   өйөмгә йәйелдерелгән. Әгәр Prometheus ҡағиҙәләре өҫтөнлөклө булһа, үҙ эсенә ала
   өҫтөнлөклө дифф.
5. **Prometheus/OTEL ҡырҡыу.** Һәр береһенән `fastpq_execution_mode_total{device_class="<matrix>"}`
   хужа (сәхнәнән һуң һәм унан һуң) плюс OTEL счетчигы
   `fastpq.execution_mode_resolutions_total` һәм парлы
   `telemetry::fastpq.execution_mode` журнал яҙмалары. Был артефакттар иҫбатлай, тип
   GPU ҡабул итеү тотороҡло һәм был мәжбүри процессор fallbacks һаман да телеметрия сығара.
6. **Архив рәт-ҡулланыусы телеметрия.** ExecWitness расшифровка өсөн йүгерә һуң.
   роллут, һөҙөмтәлә JSON өҫтөндә `row_usage/` өйөмдә төшөрөп. CI
   ярҙамсы (`ci/check_fastpq_row_usage.sh`) был снимоктарҙы сағыштыра ҡаршы
   канонлы база линиялары, һәм `ci/check_fastpq_rollout.sh` хәҙер һәр талап итә
   өйөмө йөк ташыу өсөн кәмендә бер `row_usage` файл һаҡлау өсөн TF-5 дәлилдәр беркетелгән
   сығарылыш билетына тиклем.

## Статьялы рулет ағымы

Һәр автопарк өсөн өс детерминистик фаза ҡулланығыҙ. Алданыш сығыуҙан һуң ғына .
критерийҙары һәр этапта ҡәнәғәт һәм дәлилдәр өйөмөндә документлаштырылған.| Фаза | Скоп | Критерийҙар сығыу | Ҡушымталар |
|------|-------|----------------|-------------|
| Пилот (Р1) | 1 контроль-яҫылыҡ + 1 мәғлүмәт-яҫылыҡ төйөнө бер төбәк | `fastpq_execution_mode_total{device_class="<matrix>", backend="metal"}` ≥90% 48h, нуль иҫкәртмәнсе инциденттар, һәм үткән кире ҡайтарыу бураһы. | Ике хужанан да өйөм (эске JSONs, Grafana экспорты менән пилот аннотацияһы, кире ҡайтарыу журналдары). |
| Пандия (Р2) | ≥50% валидаторҙар плюс кәмендә бер архив һыҙатында кластер | көн буйы GPU башҡарыу, 1-ҙән артыҡ түгел, шпик >10мин, һәм Prometheus иҫәпләүселәр 60-сы йылдар эсендә иҫкәртмәләрҙе иҫбатлай. | Яңыртылған Grafana экспорты күрһәтеү рампа аннотацияһы, Prometheus скреб диффтары, Alertmanager скриншот/лог. |
| Ғәҙәттәгесә (Р3) | Ҡалған төйөндәр; FASTPQ `iroha_config`-та ғәҙәттәгесә билдәләнгән | Ҡултамғалы эскәмйә манифест + Grafana экспортҡа һылтанма һуңғы ҡабул итеү ҡойроғо, һәм документлаштырылған кире ҡағыу быраулау күрһәтеү конфигурация toggle. | Һуңғы манифест, Grafana JSON, кире ҡайтарыу журналы, билет һылтанма конфиг үҙгәрештәр тикшерергә. |

Документ һәр промоушен аҙым ролл-аут билет һәм һылтанма туранан-тура
`grafana_fastpq_acceleration.json` аннотациялар, шулай итеп, рецензенттар корреляциялана ала
дәлилдәр менән ваҡыт һыҙығы.

## Rollback бырауҙар

Һәр спектакль этабы репетицияны үҙ эсенә алырға тейеш:

1. Кластерға бер төйөн йыйып, ток метрикаларын теркәгеҙ:
   ```bash
   curl -s http://<host>:8180/metrics | rg 'fastpq_execution_mode_total{device_class'
   ```
2. Көс процессор режимы 10 минут йәки конфигурация ручкаһы ҡулланып
   (`zk.fastpq.execution_mode = "cpu"`) йәки тирә-яҡ мөхит өҫтөнлөк итә:
   ```bash
   FASTPQ_GPU=cpu irohad --config <path> --genesis-manifest-json <path>
   ```
3. Журналдың түбәнгә-төшөүен раҫлау
   (`telemetry::fastpq.execution_mode resolved="cpu" requested="gpu"`) һәм ҡырҡыу
   Prometheus осонда тағы ла күрһәтеү өсөн өҫтәл өҫтәүҙәр.
4. GPU режимын тергеҙеү, раҫлауынса, `telemetry::fastpq.execution_mode` хәҙер хәбәр итә
   `resolved="metal"` (йәки металл булмаған һыҙаттар өсөн `resolved="cuda"/"opencl"`),
   Prometheus скребын раҫлауҙа 2012 йылда процессор һәм GPU өлгөләре лә бар.
   `fastpq_execution_mode_total{backend=…}` X, һәм үткән ваҡытты логин .
   асыҡлау/таҙа.
5. Һаҡлау снаряд стенограммалары, метрика, һәм оператор таныуҙары кеүек
   `rollback_drill.log` һәм `metrics_rollback.prom` өйөмөндә өйөмдә. Былар
   файлдар тулы түбәнгәград + тергеҙеү циклын күрһәтергә тейеш, сөнки
   `ci/check_fastpq_rollout.sh` хәҙер уңышһыҙлыҡҡа осрай, ҡасан да булһа журнал етешмәй GPU .
   һауығыу һыҙығы йәки метрика снимок йәки процессор йәки GPU иҫәпләүселәре төшөрөп ҡалдыра.

Был журналдар иҫбатлай, һәр кластер грациозно деградациялай ала һәм SRE командалары .
белергә, нисек кире төшөп детерминистик, әгәр GPU драйверҙары йәки ядролар регресс.

## Ҡатнаш режимда fallback дәлилдәр (WP2-E.6)

Ҡасан ғына хост кәрәк GPU FFT/LDE, әммә процессор Посейдон хеширование (бер Stage7 <900мс
талап), стандарт кире ҡағыу журналдары менән бер рәттән түбәндәге артефакттарҙы бәйләп:1. **Конфиг дифф.** Тикшерергә (йәки беркетергә) хост-урындағы өҫтөнлөк, тип ҡуя
   18NI000000113X (`FASTPQ_POSEIDON_MODE=cpu`) сығып киткәндә
   `zk.fastpq.execution_mode` тейелмәгән. Исемен патч
   `artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/poseidon_fallback.patch`.
2. **Позедон счетчик ҡырҡыу.**
   ```bash
   curl -s http://<host>:8180/metrics \
     | rg 'fastpq_poseidon_pipeline_total{.*device_class="<label>"' \
     > artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/metrics_poseidon.prom
   ```
   Ҡулға алыу тейеш күрһәтергә Grafana өҫтәү өсөн блок-аҙым менән
   GPU FFT/LDE счетчик өсөн, тип ҡоролма-класс. Икенсе ҡырҡып алыу һуң ҡайтҡас
   Ҡайтып GPU режимы, шулай итеп, рецензенттар күрә ала `path="gpu"` рәт резюме.

   Һөҙөмтәлә барлыҡҡа килгән файл `wrap_benchmark.py --poseidon-metrics …` тиклем, шулай итеп, уралған эталон яҙмалары шул уҡ счетчиктар эсендә уның `poseidon_metrics` бүлеге; был һаҡлай Metal һәм CUDA ролл-ауттар өҫтөндә бер үк эш ағымы һәм айырым ҡырҡып файлдар асмайынса, fallback дәлилдәр аудит.
3. **Журнал өҙөк.** күсермә `telemetry::fastpq.poseidon` яҙмалар, тип иҫбатлай
   18NI000000122X 1800-гә тиклем процессорға әйләндерелә.
   `poseidon_fallback.log`, ваҡыт маркаларын һаҡлау, шуға күрә Alertmanager ваҡыт графиктары булыуы мөмкин
   конфиг үҙгәреше менән бәйле.

CI сират/нуль-тулы тикшерелеүҙәрҙе үтәй бөгөн; бер тапҡыр ҡатнаш режимлы ҡапҡа ерҙәре,
`ci/check_fastpq_rollout.sh` шулай уҡ ныҡышмаясаҡ, тип теләһә ниндәй өйөм составында
`poseidon_fallback.patch` тап килгән `metrics_poseidon.prom` снимоктарын суднолар.
Был эш ағымынан һуң WP2-E.6 сәйәсәтен аудитлы һәм бәйләнгән fallback сәйәсәтен һаҡлай.
шул уҡ дәлилдәр йыйыусылар ҡулланылған ваҡытта ғәҙәттәгесә-ролировка.

## Отчет & Автоматлаштырыу

- Бөтә `artifacts/fastpq_rollouts/<stamp>/` каталогын беркетергә
  релиз билет һәм уны һылтанма `status.md` бер тапҡыр ролл-аут ябыла.
- Йүгереп `dashboards/alerts/tests/fastpq_acceleration_rules.test.yml` (аҫта
  `promtool`) эсендә CI тәьмин итеү өсөн иҫкәртмә өйөмдәре менән йыйылған ролл-аут һаман да
  компиляция.
- `ci/check_fastpq_rollout.sh` менән өйөмөн раҫлағыҙ (йәки
  `make check-fastpq-rollout`) һәм үткән `FASTPQ_ROLLOUT_BUNDLE=<path>`, ҡасан һеҙ
  теләйем, бер генә ролл-аут маҡсатлы. CI аша шул уҡ сценарийҙы саҡыра
  `.github/workflows/fastpq-rollout.yml`, шуға күрә юғалған артефакттар тиҙ уңышһыҙлыҡҡа осрағанға тиклем
  релиз билет ябыла ала. Релиз торбаһы архив раҫланған өйөмдәр ала
  ҡултамғалы манифестар менән бер рәттән үтеп, үтеп,
  `--fastpq-rollout-bundle artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>` тиклем
  `scripts/run_release_pipeline.py` X; ярҙамсы ҡабаттан эшләй
  `ci/check_fastpq_rollout.sh` (әгәр `--skip-fastpq-rollout-check` X булмаһа) һәм
  каталог ағасын `artifacts/releases/<version>/fastpq_rollouts/…`-ға күсерә.
  Был ҡапҡа өлөшө булараҡ, сценарий Stage7 сират-тәрәнлек һәм нуль-тулы үтәй.
  бюджеттар `benchmarks.metal_dispatch_queue` һәм
  `benchmarks.zero_fill_hotspots` һәр `metal` эскәмйәһе JSON.

Был playbook-ты үтәп, беҙ детерминистик ҡабул итеүҙе күрһәтә алабыҙ, тәьмин итәбеҙ
бер дәлилдәр өйөмө өсөн бер ролл-аут, һәм һаҡлау өҫтөндә ҡасып йөрөү күнекмәләре менән бергә аудит
ҡул ҡуйылған эталон манифестар.
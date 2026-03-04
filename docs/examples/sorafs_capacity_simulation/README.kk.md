---
lang: kk
direction: ltr
source: docs/examples/sorafs_capacity_simulation/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 727a648141405b0c8f12a131ff903d3e7ce5b74a7f899dd99fe9aa6490b55ef2
source_last_modified: "2025-12-29T18:16:35.080764+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS Сыйымдылықты модельдеу құралдар жинағы

Бұл каталог SF-2c сыйымдылығы нарығы үшін қайталанатын артефактілерді жібереді
симуляция. Құралдар жинағы квота туралы келіссөздерді, орындамауды өңдеуді және қиюды жүзеге асырады
өндірістік CLI көмекшілері және жеңіл талдау сценарийі арқылы түзету.

## Алғышарттар

- Жұмыс кеңістігінің мүшелері үшін `cargo run` іске қосуға қабілетті Rust құралдар тізбегі.
- Python 3.10+ (тек стандартты кітапхана).

## Жылдам бастау

```bash
# 1. Generate canonical CLI artefacts
./run_cli.sh ./artifacts

# 2. Aggregate the results and emit Prometheus metrics
./analyze.py --artifacts ./artifacts
```

`run_cli.sh` сценарийі құру үшін `sorafs_manifest_stub capacity` шақырады:

- Квота туралы келіссөздер жиынтығы үшін детерминистік провайдер мәлімдемелері.
- Келіссөз сценарийіне сәйкес келетін репликация тәртібі.
- Ауыспалы терезе үшін телеметриялық суреттер.
- Кесу сұрауын түсіретін дау пайдалы жүктемесі.

Сценарий Norito байттарын (`*.to`), base64 пайдалы жүктемелерін (`*.b64`), Torii сұрауын жазады.
денелер және таңдалған артефакт бойынша адам оқи алатын қорытындылар (`*_summary.json`)
каталог.

`analyze.py` жасалған қорытындыларды тұтынады, жинақталған есепті шығарады
(`capacity_simulation_report.json`) және Prometheus мәтіндік файлын шығарады
(`capacity_simulation.prom`) тасымалдау:

- Келісілген сыйымдылық пен бөлуді сипаттайтын `sorafs_simulation_quota_*` өлшеуіштері
  бір провайдерге үлес.
- `sorafs_simulation_failover_*` өлшегіштері тоқтау уақытының дельталарын және таңдалған
  ауыстыру провайдері.
- `sorafs_simulation_slash_requested` шығарылған қалпына келтіру пайызын жазады
  даудың пайдалы жүктемесінен.

Grafana бумасын `dashboards/grafana/sorafs_capacity_simulation.json` ішіне импорттаңыз
және оны жасалған мәтіндік файлды сызып тастайтын Prometheus деректер көзіне бағыттаңыз (үшін
мысалы, түйін экспорттаушы мәтіндік файл жинағышы арқылы). Runbook мекенжайы
`docs/source/sorafs/runbooks/sorafs_capacity_simulation.md` толықтай өтеді
жұмыс процесі, соның ішінде Prometheus конфигурация кеңестері.

## Арматуралар

- `scenarios/quota_negotiation/` — Провайдер декларациясының сипаттамалары және репликация тәртібі.
- `scenarios/failover/` — Бастапқы үзіліс пен істен шығуға арналған телеметриялық терезелер.
- `scenarios/slashing/` — Бірдей репликация тәртібіне сілтеме жасайтын дау спецификациясы.

Бұл қондырғылар `crates/sorafs_car/tests/capacity_simulation_toolkit.rs` ішінде расталған
олардың CLI схемасымен синхрондалатынына кепілдік беру үшін.
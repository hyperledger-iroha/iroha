---
lang: kk
direction: ltr
source: docs/portal/docs/sorafs/capacity-simulation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: de9c2a162d97ab896e51af36054e0f3342522b241bfba3ded9c1ec764e590159
source_last_modified: "2026-01-22T14:35:36.797990+00:00"
translation_last_reviewed: 2026-02-07
id: capacity-simulation
title: SoraFS Capacity Simulation Runbook
sidebar_label: Capacity Simulation Runbook
description: Exercising the SF-2c capacity marketplace simulation toolkit with reproducible fixtures, Prometheus exports, and Grafana dashboards.
translator: machine-google-reviewed
---

:::ескерту Канондық дереккөз
:::

Бұл жұмыс кітабы SF-2c сыйымдылығының нарықтық модельдеу жинағын іске қосу және нәтиже көрсеткіштерін визуализациялау жолын түсіндіреді. Ол `docs/examples/sorafs_capacity_simulation/` ішіндегі детерминирленген құрылғыларды пайдалана отырып, квота туралы келіссөздерді, ауыстырып-қосуды өңдеуді және қиюды түзетуді соңына дейін растайды. Сыйымдылықтың пайдалы жүктемелері әлі де `sorafs_manifest_stub capacity` пайдаланады; манифест/CAR орау ағындары үшін `iroha app sorafs toolkit pack` пайдаланыңыз.

## 1. CLI артефактілерін жасаңыз

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh` `sorafs_manifest_stub capacity` орап, Norito пайдалы жүктемелерді, base64 блобтарын, Torii сұрау элементтерін және JSON қорытындыларын шығарады:

- Квота бойынша келіссөздер сценарийіне қатысатын үш провайдер декларациясы.
- Осы провайдерлер бойынша кезеңдік манифестті бөлетін репликация тәртібі.
- Тоқтау алдындағы негізгі сызыққа, үзіліс аралығына және істен шығуды қалпына келтіруге арналған телеметриялық суреттер.
- Үлгіленген үзілістен кейін қысқартуды талап ететін пайдалы жүктеме.

Барлық артефактілер `./artifacts` астына түседі (бірінші аргумент ретінде басқа каталогты өту арқылы қайта анықтау). `_summary.json` файлдарында адам оқи алатын мәтінмән бар-жоғын тексеріңіз.

## 2. Нәтижелерді біріктіру және көрсеткіштерді шығару

```bash
./analyze.py --artifacts ./artifacts
```

Анализатор мыналарды шығарады:

- `capacity_simulation_report.json` - біріктірілген бөлулер, ауыспалы дельталар және дау метадеректері.
- `capacity_simulation.prom` - Prometheus мәтіндік файл көрсеткіштері (`sorafs_simulation_*`) түйінді экспорттаушы мәтіндік файл жинағышына немесе жеке скрап жұмысына жарамды.

Prometheus қырғыш конфигурациясының мысалы:

```yaml
scrape_configs:
  - job_name: sorafs-capacity-sim
    scrape_interval: 15s
    static_configs:
      - targets: ["localhost:9100"]
        labels:
          scenario: "capacity-sim"
    metrics_path: /metrics
    params:
      format: ["prometheus"]
```

Мәтіндік файл коллекторын `capacity_simulation.prom` нүктесіне бағыттаңыз (түйін экспорттауын пайдаланған кезде оны `--collector.textfile.directory` арқылы өтетін каталогқа көшіріңіз).

## 3. Grafana бақылау тақтасын импорттаңыз

1. Grafana ішінде `dashboards/grafana/sorafs_capacity_simulation.json` импорттаңыз.
2. `Prometheus` деректер көзі айнымалы мәнін жоғарыда конфигурацияланған қырып алу мақсатына байланыстырыңыз.
3. Панельдерді тексеріңіз:
   - **Квота бөлу (GiB)** әрбір провайдер үшін бекітілген/тағайындалған теңгерімдерді көрсетеді.
   - **Тоқтату триггері** үзіліс көрсеткіштері ағыны кірген кезде *Тұтқырлық белсенді* күйіне ауысады.
   - **Үзіліс кезінде жұмыс уақытының төмендеуі** `alpha` провайдері үшін пайыздық жоғалту диаграммасын көрсетеді.
   - **Сұралған қиғаш сызықтың пайызы** даулы құралдан алынған түзету коэффициентін көрсетеді.

## 4. Күтілетін тексерулер

- `sorafs_simulation_quota_total_gib{scope="assigned"}` `600` тең, ал қабылданған жалпы мән >=600 болып қалады.
- `sorafs_simulation_failover_triggered` `1` есептері және ауыстыру провайдерінің көрсеткіші `beta` бөлектеледі.
- `sorafs_simulation_slash_requested` `alpha` провайдер идентификаторы үшін `0.15` (15% қиғаш сызық) хабарлайды.

Арматуралардың әлі де CLI схемасы арқылы қабылданғанын растау үшін `cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` іске қосыңыз.
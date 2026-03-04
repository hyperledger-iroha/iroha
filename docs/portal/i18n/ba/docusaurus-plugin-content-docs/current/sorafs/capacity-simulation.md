---
id: capacity-simulation
lang: ba
direction: ltr
source: docs/portal/docs/sorafs/capacity-simulation.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Capacity Simulation Runbook
sidebar_label: Capacity Simulation Runbook
description: Exercising the SF-2c capacity marketplace simulation toolkit with reproducible fixtures, Prometheus exports, and Grafana dashboards.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::иҫкәртергә канонлы сығанаҡ
::: 1990 й.

Был runbook аңлата, нисек эшләтергә SF-2c ҡәҙерле баҙар моделләштереү комплекты һәм һөҙөмтәлә метрика визуализация. Ул раҫлай квота һөйләшеүҙәре, авариялар менән эш итеү, һәм ҡырҡып өҙөлгән осона-осона ҡулланып детерминистик ҡорамалдар I18NI000000010X. Ҡыйыулыҡ файҙалы йөктәр һаман да `sorafs_manifest_stub capacity` ҡуллана; ҡулланыу `iroha app sorafs toolkit pack` өсөн манифест/CAR упаковка ағымдары.

## 1. CLI артефакттарын генерациялау

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh` уратып I18NI000000014X I18NT0000000005X файҙалы йөктәрҙе сығарыу өсөн, base64 таптар, I18NT000000006X запрос органдары, һәм JSON өсөн резюме:

- Өс провайдер квота һөйләшеүҙәре сценарийында ҡатнашҡан.
- репликация тәртибе бүлергә сәхнәләштерелгән был провайдерҙар араһында күренә.
- Телеметрия снимоктары өсөн алдан өҙөлгән башланғыс һыҙығы, өҙөлгән интервалы, һәм аварияларҙы тергеҙеү.
- Бәхәс файҙалы йөк һорап ҡырҡылғандан һуң имитацияланған өҙөлгән.

Бөтә артефакттар ҙа `./artifacts` буйынса ерләнә (беренсе аргумент булараҡ икенсе каталогты тапшырып, өҫтөнлөк бирелә). I18NI000000016X файлдарын кеше уҡыу контексы өсөн тикшерергә.

## 2. Агрегат һөҙөмтәләре & метрика сығарыу

```bash
./analyze.py --artifacts ./artifacts
```

Анализатор етештерә:

- I18NI000000017X - агрегацияланған бүленә, авариялы дельта һәм бәхәс метамағлүмәттәре.
- I18NI000000018X - I18NT0000000000X текст файлы метрикаһы (I18NI000000019X) төйөн-экспортер тексты йыйыусы йәки үҙ аллы ҡырҡыу эше өсөн яраҡлы.

Миҫал I18NT000000001X ҡырҡыу конфигурацияһы:

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
``` X

`capacity_simulation.prom`-та текст файлы коллекционеры (төйөн экспортерын ҡулланғанда уны каталогҡа күсергәндә `--collector.textfile.directory` аша тапшырғанда).

## 3. Import I18NT0000000003X приборҙар таҡтаһы

1. Grafana, импорт I18NI000000022X.
.
3. панелдәрҙе тикшерергә:
   - **Цета бүлергә (GiB)** һәр провайдер өсөн ҡылынған/баҫымдарҙы күрһәтә.
   - **Файловер Триггер** *Бер ҡасан әүҙем * ҡасан өҙөклөк метрикаһы ағымы инә.
   - **Дроп ваҡытында өҙөлгән** графиктар процент юғалтыу өсөн провайдер I18NI000000024X.
   - **Шәп шлепать процент** бәхәс ҡоролмаһынан сығарылған төҙәтеү нисбәтен визуализациялай.

## 4. Көтөлгән чектар

- I18NI000000025X тигеҙ I18NI000000026X, ә дөйөм ҡала >=600.
- `sorafs_simulation_failover_triggered` хәбәр итеүенсә, I18NI000000028X һәм алмаштырыусы метрикаһы `beta` X.
- I18NI0000000030X I18NI00000000032X провайдеры идентификаторы өсөн I18NI0000000031X хәбәр итә.

Йүгереп I18NI000000033X раҫлау өсөн ҡорамалдар һаман да ҡабул ителә CLI схемаһы.
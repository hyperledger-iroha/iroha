---
id: capacity-simulation
lang: hy
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

:::note Կանոնական աղբյուր
:::

Այս գրքույկը բացատրում է, թե ինչպես գործարկել SF-2c հզորության շուկայի մոդելավորման հավաքածուն և պատկերացնել արդյունքում ստացված չափումները: Այն վավերացնում է քվոտաների բանակցությունները, ձախողումների հետ աշխատելը և վերջից մինչև վերջ կրճատելու վերականգնումը, օգտագործելով `docs/examples/sorafs_capacity_simulation/`-ի դետերմինիստական ​​հարմարանքները: Տարողունակության օգտակար բեռները դեռ օգտագործում են `sorafs_manifest_stub capacity`; օգտագործեք `iroha app sorafs toolkit pack` մանիֆեստի/Ավտոմեքենաների փաթեթավորման հոսքերի համար:

## 1. Ստեղծեք CLI արտեֆակտներ

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh`-ը փաթաթում է `sorafs_manifest_stub capacity`՝ արտանետելու Norito օգտակար բեռներ, base64 բլբեր, Torii հարցումների մարմիններ և JSON ամփոփագրեր՝

- Քվոտայի շուրջ բանակցությունների սցենարին մասնակցող երեք մատակարարի հայտարարություն:
- Կրկնօրինակման կարգ, որը բաշխում է բեմականացված մանիֆեստը այդ մատակարարներին:
- Հեռուստաչափական պատկերներ նախքան անջատման ելակետի, անջատման միջակայքի և ձախողման վերականգնման համար:
- Վեճերի օգտակար բեռ, որը պահանջում է կրճատում մոդելավորված անջատումից հետո:

Բոլոր արտեֆակտները տեղակայվում են `./artifacts`-ի տակ (չեղյալ համարել՝ որպես առաջին արգումենտ ուղարկելով այլ գրացուցակ): Ստուգեք `_summary.json` ֆայլերը մարդու համար ընթեռնելի համատեքստի համար:

## 2. Արդյունքների համախառն և արտանետվող չափումներ

```bash
./analyze.py --artifacts ./artifacts
```

Անալիզատորը արտադրում է.

- `capacity_simulation_report.json` - ագրեգացված հատկացումներ, ձախողման դելտաներ և վեճերի մետատվյալներ:
- `capacity_simulation.prom` - Prometheus տեքստային ֆայլի չափումներ (`sorafs_simulation_*`) հարմար է հանգույց արտահանող տեքստային ֆայլերի հավաքողի կամ ինքնուրույն քերծվածքի աշխատանքի համար:

Օրինակ Prometheus քերծվածքի կոնֆիգուրացիա.

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

Տեքստային ֆայլերի հավաքիչը ուղղեք `capacity_simulation.prom` (հանգույց-արտահանող սարք օգտագործելիս այն պատճենեք `--collector.textfile.directory`-ով փոխանցված գրացուցակում):

## 3. Ներմուծեք Grafana վահանակը

1. Grafana-ում ներմուծեք `dashboards/grafana/sorafs_capacity_simulation.json`:
2. Կապեք `Prometheus` տվյալների աղբյուրի փոփոխականը վերևում կազմաձևված քերծվածքային թիրախին:
3. Ստուգեք վահանակները.
   - **Քվոտայի հատկացումը (GiB)** ցույց է տալիս պարտավորված/նշանակված մնացորդները յուրաքանչյուր մատակարարի համար:
   - **Failover Trigger**-ը շրջվում է դեպի *Failover Active*, երբ խափանումների ցուցանիշները հոսք են անցնում:
   - **Uptime-ի անկումը անջատման ժամանակ** գծապատկերում է `alpha` մատակարարի տոկոսային կորուստը:
   - **Պահանջվող կտրվածքի տոկոսը** պատկերացնում է վեճի հարթակից արդյունահանված վերականգնման գործակիցը:

## 4. Ակնկալվող ստուգումներ

- `sorafs_simulation_quota_total_gib{scope="assigned"}`-ը հավասար է `600`-ին, մինչդեռ պարտավորված ընդհանուր գումարը մնում է >=600:
- `sorafs_simulation_failover_triggered`-ը հաղորդում է `1`, իսկ փոխարինող մատակարարի մետրային կարևորում է `beta`-ը:
- `sorafs_simulation_slash_requested`-ը հաղորդում է `0.15` (15% կտրվածք) `alpha` մատակարարի նույնացուցիչի համար:

Գործարկեք `cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit`՝ հաստատելու համար, որ հարմարանքները դեռևս ընդունված են CLI սխեմայով:
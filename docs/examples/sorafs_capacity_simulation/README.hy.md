---
lang: hy
direction: ltr
source: docs/examples/sorafs_capacity_simulation/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 727a648141405b0c8f12a131ff903d3e7ce5b74a7f899dd99fe9aa6490b55ef2
source_last_modified: "2025-12-29T18:16:35.080764+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS Կարողությունների մոդելավորման գործիքակազմ

Այս գրացուցակը առաքում է վերարտադրվող արտեֆակտները SF-2c հզորությունների շուկայի համար
սիմուլյացիա։ Գործիքների հավաքածուն իրականացնում է քվոտաների բանակցություններ, ձախողման հետ կապված կառավարում և կրճատում
վերականգնում` օգտագործելով արտադրական CLI օգնականները և թեթև վերլուծության սցենարը:

## Նախադրյալներ

- Rust գործիքների շղթա, որը կարող է աշխատել `cargo run` աշխատանքային տարածքի անդամների համար:
- Python 3.10+ (միայն ստանդարտ գրադարան):

## Արագ մեկնարկ

```bash
# 1. Generate canonical CLI artefacts
./run_cli.sh ./artifacts

# 2. Aggregate the results and emit Prometheus metrics
./analyze.py --artifacts ./artifacts
```

`run_cli.sh` սկրիպտը կանչում է `sorafs_manifest_stub capacity`՝ կառուցելու համար.

- Քվոտայի բանակցային փաթեթի որոշիչ մատակարարի հայտարարություններ:
- Բանակցային սցենարին համապատասխանող կրկնօրինակման կարգ:
- Հեռուստաչափական պատկերներ ձախողման պատուհանի համար:
- Վեճերի ծանրաբեռնվածություն, որը գրավում է կտրման հարցումը:

Սցենարը գրում է Norito բայթ (`*.to`), base64 օգտակար բեռներ (`*.b64`), Torii հարցում
մարմիններ և մարդու կողմից ընթեռնելի ամփոփագրեր (`*_summary.json`) ընտրված արտեֆակտի տակ
գրացուցակ.

`analyze.py`-ը սպառում է ստեղծված ամփոփագրերը, կազմում է համախառն հաշվետվություն
(`capacity_simulation_report.json`) և թողարկում է Prometheus տեքստային ֆայլ
(`capacity_simulation.prom`) կրող.

- `sorafs_simulation_quota_*` չափիչներ, որոնք նկարագրում են բանակցային հզորությունը և տեղաբաշխումը
  բաժնետոմս մեկ մատակարարի համար:
- `sorafs_simulation_failover_*` չափիչներ, որոնք ընդգծում են խափանումների դելտաները և ընտրվածը
  փոխարինող մատակարար:
- `sorafs_simulation_slash_requested` արձանագրելով արդյունահանված վերականգնման տոկոսը
  վեճի ծանրաբեռնվածությունից.

Ներմուծեք Grafana փաթեթը `dashboards/grafana/sorafs_capacity_simulation.json`-ում
և ուղղեք այն Prometheus տվյալների աղբյուրի վրա, որը քերծում է ստեղծված տեքստային ֆայլը (համար
օրինակ՝ հանգույց-արտահանող տեքստային ֆայլերի կոլեկցիոների միջոցով): The runbook at
`docs/source/sorafs/runbooks/sorafs_capacity_simulation.md`-ն անցնում է ամբողջությամբ
աշխատանքային հոսքը, ներառյալ Prometheus կոնֆիգուրացիայի խորհուրդները:

## Հարմարանքներ

- `scenarios/quota_negotiation/` — Մատակարարի հայտարարագրի բնութագրերը և կրկնօրինակման կարգը:
- `scenarios/failover/` — Հեռաչափական պատուհաններ առաջնային անջատման և խափանման վերելակի համար:
- `scenarios/slashing/` — Վեճերի սպեկտրը վկայակոչում է նույն կրկնօրինակման կարգը:

Այս հարմարանքները վավերացված են `crates/sorafs_car/tests/capacity_simulation_toolkit.rs`-ում
երաշխավորելու համար, որ դրանք համաժամանակյա են մնում CLI սխեմայի հետ:
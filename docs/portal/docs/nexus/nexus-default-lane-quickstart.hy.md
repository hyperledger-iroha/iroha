---
lang: hy
direction: ltr
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6120b882618e0f9b6113948d3b12d97e0152a5fc5d4350681ba30aaf114e99d3
source_last_modified: "2026-01-22T14:45:01.354580+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-default-lane-quickstart
title: Default lane quickstart (NX-5)
sidebar_label: Default Lane Quickstart
description: Configure and verify the Nexus default lane fallback so Torii and SDKs can omit lane_id in public lanes.
translator: machine-google-reviewed
---

:::note Կանոնական աղբյուր
Այս էջը արտացոլում է `docs/source/quickstart/default_lane.md`: Պահպանեք երկու օրինակները
հավասարեցված, մինչև տեղայնացման մաքրումը վայրէջք կատարի պորտալում:
:::

# Կանխադրված գծի արագ մեկնարկ (NX-5)

> **Ճանապարհային քարտեզի համատեքստ.** NX-5 — լռելյայն հանրային գոտիների ինտեգրում: Գործարկման ժամանակը հիմա
> ցուցադրում է `nexus.routing_policy.default_lane` հետադարձ կապ, ուստի Torii REST/gRPC
> վերջնակետերը և յուրաքանչյուր SDK կարող է ապահով կերպով բաց թողնել `lane_id`-ը, երբ տրաֆիկը պատկանում է
> կանոնական հանրային գծի վրա: Այս ուղեցույցը ուղեկցում է օպերատորներին կազմաձևման միջոցով
> կատալոգը, ստուգելով հետադարձ կապը `/status`-ում և գործադրելով հաճախորդին
> վարքագիծը վերջից վերջ:

## Նախադրյալներ

- `irohad`-ի Sora/Nexus կառուցվածք (աշխատում է `irohad --sora --config ...`-ով):
- Մուտք գործեք կազմաձևման պահոց, որպեսզի կարողանաք խմբագրել `nexus.*` բաժինները:
- `iroha_cli` կազմաձևված է թիրախային կլաստերի հետ խոսելու համար:
- `curl`/`jq` (կամ համարժեք)՝ Torii `/status` օգտակար բեռը ստուգելու համար:

## 1. Նկարագրեք գծի և տվյալների տարածության կատալոգը

Հայտարարեք գծերը և տվյալների տարածությունները, որոնք պետք է գոյություն ունենան ցանցում: Հատվածը
ներքևում (կտրված է `defaults/nexus/config.toml`-ից) գրանցում է երեք հանրային գոտի
գումարած տվյալների տարածության համընկնող փոխանունները.

```toml
[nexus]
lane_count = 3

[[nexus.lane_catalog]]
index = 0
alias = "core"
description = "Primary execution lane"
dataspace = "universal"

[[nexus.lane_catalog]]
index = 1
alias = "governance"
description = "Governance & parliament traffic"
dataspace = "governance"

[[nexus.lane_catalog]]
index = 2
alias = "zk"
description = "Zero-knowledge attachments"
dataspace = "zk"

[[nexus.dataspace_catalog]]
alias = "universal"
id = 0
description = "Single-lane data space"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "governance"
id = 1
description = "Governance proposals & manifests"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "zk"
id = 2
description = "Zero-knowledge proofs and attachments"
fault_tolerance = 1
```

Յուրաքանչյուր `index` պետք է լինի եզակի և հարակից: Տվյալների տարածքի ID-ները 64-բիթանոց արժեքներ են.
վերը նշված օրինակները պարզության համար օգտագործում են նույն թվային արժեքները, ինչ գծերի ինդեքսները:

## 2. Սահմանեք երթուղիների լռելյայն և կամընտիր անտեսումները

`nexus.routing_policy` բաժինը վերահսկում է հետադարձ գիծը և թույլ է տալիս ձեզ
անտեսել երթուղին հատուկ հրահանգների կամ հաշվի նախածանցների համար: Եթե չկա կանոն
համընկնում է, ժամանակացույցը փոխանցում է գործարքը դեպի կազմաձևված `default_lane`
և `default_dataspace`. Երթուղիչի տրամաբանությունն ապրում է
`crates/iroha_core/src/queue/router.rs` և թափանցիկ կերպով կիրառում է քաղաքականությունը
Torii REST/gRPC մակերեսներ:

```toml
[nexus.routing_policy]
default_lane = 0                # use the "core" lane when no rules match
default_dataspace = "universal"    # reuse the public dataspace for the fallback

[[nexus.routing_policy.rules]]
lane = 1
dataspace = "governance"
[nexus.routing_policy.rules.matcher]
instruction = "governance"
description = "Route governance instructions to the governance lane"

[[nexus.routing_policy.rules]]
lane = 2
dataspace = "zk"
[nexus.routing_policy.rules.matcher]
instruction = "smartcontract::deploy"
description = "Route contract deployments to the zk lane for proof tracking"
```

Երբ հետագայում ավելացնեք նոր երթուղիներ, նախ թարմացրեք կատալոգը, ապա երկարացրեք երթուղին
կանոնները։ Հետադարձ գիծը պետք է շարունակի ուղղվել դեպի հանրային գոտին, որը պահում է

## 3. Գործարկեք հանգույց՝ կիրառված քաղաքականությամբ

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

Գործարկման ընթացքում հանգույցը գրանցում է ստացված երթուղային քաղաքականությունը: Վավերացման ցանկացած սխալ
(բացակայող ինդեքսները, կրկնօրինակված անունները, տվյալների տարածքի անվավեր ID-ները) հայտնվել են նախկինում
սկսվում է բամբասանքը.

## 4. Հաստատեք գոտիների կառավարման վիճակը

Երբ հանգույցը առցանց է, օգտագործեք CLI օգնականը՝ ստուգելու, որ լռելյայն գոտին է
կնքված (մանիֆեստը բեռնված) և պատրաստ է երթևեկության համար: Ամփոփ տեսքը տպում է մեկ տող
մեկ գծի համար.

```bash
iroha_cli app nexus lane-report --summary
```

Ելքի օրինակ.

```
Lane  Alias            Module           Status  Quorum  Validators  Detail
   0  core             parliament       ready      05           07  manifest ok
   1  governance       parliament       ready      05           07  manifest ok
   2  zk               parliament       sealed     03           05  manifest required
```

Եթե լռելյայն գիծը ցույց է տալիս `sealed`, հետևեք երթևեկության գծի կառավարման ուղեցույցին առաջ
թույլ տալով արտաքին երթևեկությունը: `--fail-on-sealed` դրոշակը հարմար է CI-ի համար:

## 5. Ստուգեք Torii կարգավիճակի օգտակար բեռները

`/status` պատասխանը բացահայտում է ինչպես երթուղային քաղաքականությունը, այնպես էլ յուրաքանչյուր գծի ժամանակացույցը
ակնթարթ. Օգտագործեք `curl`/`jq`՝ կազմաձևված կանխադրումները հաստատելու և դա ստուգելու համար
հետադարձ գոտին արտադրում է հեռաչափություն.

```bash
curl -s http://127.0.0.1:8080/status | jq '.nexus.routing_policy'
```

Նմուշի ելք.

```json
{
  "default_lane": 0,
  "default_dataspace": "universal",
  "rules": [
    {"lane": 1, "dataspace": "governance", "matcher": {"instruction": "governance"}},
    {"lane": 2, "dataspace": "zk", "matcher": {"instruction": "smartcontract::deploy"}}
  ]
}
```

`0` գծի ուղիղ ժամանակացույցի հաշվիչները ստուգելու համար.

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

Սա հաստատում է, որ TEU պատկերը, կեղծանունը մետատվյալները և մանիֆեստի դրոշները համընկնում են
կոնֆիգուրացիայի հետ: Նույն ծանրաբեռնվածությունն այն է, ինչ օգտագործում են Grafana վահանակները
lane-ingest վահանակ:

## 6. Կիրառեք հաճախորդի լռելյայն կարգավորումները

- **Rust/CLI.** `iroha_cli`-ը և Rust հաճախորդի տուփը բաց են թողնում `lane_id` դաշտը
  երբ դուք չեք անցնում `--lane-id` / `LaneSelector`: Հետևաբար, հերթի երթուղիչը
  հետ է ընկնում `default_lane`-ին: Օգտագործեք հստակ `--lane-id`/`--dataspace-id` դրոշներ
  միայն այն դեպքում, երբ թիրախավորվում է ոչ լռելյայն գոտի:
- **JS/Swift/Android։** SDK-ի վերջին թողարկումները համարում են `laneId`/`lane_id` որպես ընտրովի
  և հետ ընկնել `/status`-ի կողմից գովազդված արժեքին: Պահպանեք երթուղային քաղաքականությունը
  համաժամեցեք բեմադրության և արտադրության միջև, որպեսզի բջջային հավելվածները արտակարգ իրավիճակների կարիք չունենան
  վերակազմավորումներ.
- **Խողովակաշարի/SSE թեստեր:** Գործարքի իրադարձության զտիչներն ընդունվում են
  `tx_lane_id == <u32>` պրեդիկատներ (տես `docs/source/pipeline.md`): Բաժանորդագրվել
  `/v2/pipeline/events/transactions` այդ ֆիլտրով ապացուցելու համար, որ գրությունները ուղարկվել են
  առանց հստակ երթուղու ժամանում են հետադարձ գծի id-ի տակ:

## 7. Դիտորդական և կառավարման կեռիկներ

- `/status`-ը նաև հրապարակում է `nexus_lane_governance_sealed_total` և
  `nexus_lane_governance_sealed_aliases`, այնպես որ Alertmanager-ը կարող է զգուշացնել, երբ a
  երթուղին կորցնում է իր դրսևորումը. Միացված պահեք այդ ահազանգերը նույնիսկ devnet-ների համար:
- Ժամանակացույցի հեռաչափության քարտեզը և գոտիների կառավարման վահանակը
  (`dashboards/grafana/nexus_lanes.json`) ակնկալում են alias/slug դաշտերը
  կատալոգ։ Եթե փոխանուն եք վերանվանում, վերանշանակեք համապատասխան Kura գրացուցակներն այդպես
  աուդիտորները պահպանում են դետերմինիստական ուղիները (հետևվում են NX-1-ով):
- Խորհրդարանի կողմից լռելյայն ուղիների հաստատումները պետք է ներառեն հետադարձ պլան: Ձայնագրեք
  բացահայտ հեշը և կառավարման ապացույցները ձեր այս արագ մեկնարկի կողքին
  օպերատորի runbook, որպեսզի ապագա պտույտները չգուշակեն պահանջվող վիճակը:

Այս ստուգումները անցնելուց հետո դուք կարող եք `nexus.routing_policy.default_lane`-ին վերաբերվել որպես
կոդերի ուղիները ցանցում:
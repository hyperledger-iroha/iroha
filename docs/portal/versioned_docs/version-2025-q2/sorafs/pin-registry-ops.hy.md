---
lang: hy
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sorafs/pin-registry-ops.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0dc64bb4067d734250852a74a65a2100bd68e5ff35f9e8e9dbf3bd2b86f00cfa
source_last_modified: "2026-01-22T14:35:36.898296+00:00"
translation_last_reviewed: 2026-02-07
id: pin-registry-ops-hy
title: Pin Registry Operations
sidebar_label: Pin Registry Operations
description: Monitor and triage the SoraFS pin registry and replication SLA metrics.
translator: machine-google-reviewed
slug: /sorafs/pin-registry-ops-hy
---

:::note Կանոնական աղբյուր
Հայելիներ `docs/source/sorafs/runbooks/pin_registry_ops.md`. Պահպանեք երկու տարբերակները հավասարեցված թողարկումներում:
:::

## Տեսություն

Այս runbook-ը փաստում է, թե ինչպես վերահսկել և տրաժավորել SoraFS փին ռեեստրը և դրա կրկնօրինակման ծառայության մակարդակի համաձայնագրերը (SLAs): Չափիչները ծագում են `iroha_torii`-ից և արտահանվում են Prometheus-ի միջոցով՝ `torii_sorafs_*` անվանատարածքով: Torii-ը նմուշառում է ռեեստրի վիճակը 30 վայրկյան ընդմիջումով հետին պլանում, ուստի վահանակները մնում են ընթացիկ, նույնիսկ երբ ոչ մի օպերատոր չի ուսումնասիրում `/v2/sorafs/pin/*` վերջնակետերը: Ներմուծեք ընտրված վահանակը (`docs/source/grafana_sorafs_pin_registry.json`)՝ օգտագործման համար պատրաստի Grafana դասավորության համար, որն ուղղակիորեն համադրվում է ստորև նշված բաժիններին:

## մետրային հղում

| Մետրական | Պիտակներ | Նկարագրություն |
| ------ | ------ | ----------- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \| `approved` \| `retired`) | Շղթայական մանիֆեստի գույքագրում ըստ կյանքի ցիկլի վիճակի: |
| `torii_sorafs_registry_aliases_total` | — | Գրանցամատյանում գրանցված ակտիվ մանիֆեստների կեղծանունների քանակը: |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \| `completed` \| `expired`) | Կրկնօրինակման պատվերի կուտակումները՝ բաժանված ըստ կարգավիճակի: |
| `torii_sorafs_replication_backlog_total` | — | `pending` պատվերներ հայելային հարմարաչափ. |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \| `missed` \| `pending`) | SLA հաշվառում. `met`-ը հաշվում է ավարտված պատվերները վերջնաժամկետում, `missed` ագրեգատները ուշ ավարտում + ժամկետները, `pending`-ը արտացոլում է չմարված պատվերները: |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Համախառն ավարտի ուշացում (թողարկման և ավարտի միջև ընկած դարաշրջաններ): |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Սպասվող պատվերի անփույթ պատուհաններ (վերջնաժամկետ՝ հանած տրված դարաշրջան): |

Բոլոր չափիչները վերակայվում են յուրաքանչյուր ակնթարթային նկարի վրա, ուստի վահանակները պետք է նմուշառվեն `1m` կամ ավելի արագությամբ:

## Grafana վահանակ

JSON վահանակը առաքվում է յոթ վահանակներով, որոնք ծածկում են օպերատորի աշխատանքային հոսքերը: Հարցումները թվարկված են ստորև՝ արագ հղման համար, եթե նախընտրում եք պատվերով գծապատկերներ կառուցել:

1. **Մանիֆեստի կյանքի ցիկլը** – `torii_sorafs_registry_manifests_total` (խմբավորված ըստ `status`):
2. **Alias ​​կատալոգի միտում** – `torii_sorafs_registry_aliases_total`:
3. **Պատվերների հերթ ըստ կարգավիճակի** – `torii_sorafs_registry_orders_total` (խմբավորված ըստ `status`):
4. **Հետքածկ ընդդեմ ժամկետանց պատվերների** – համատեղում է `torii_sorafs_replication_backlog_total` և `torii_sorafs_registry_orders_total{status="expired"}` մակերեսների հագեցվածությունը:
5. **SLA հաջողության հարաբերակցություն** –

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```

6. **Հապաղում ընդդեմ վերջնաժամկետի թուլության** – ծածկույթ `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` և `torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`: Օգտագործեք Grafana փոխակերպումները՝ `min_over_time` դիտումներ ավելացնելու համար, երբ ձեզ անհրաժեշտ է բացարձակ անփութ հատակ, օրինակ՝

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. **Բաց թողնված պատվերներ (1ժ արագություն)** –

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## Զգուշացման շեմեր- **SLA հաջողություն  0**
  - Շեմը՝ `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - Գործողություն. Ստուգեք կառավարման մանիֆեստները՝ հաստատելու մատակարարի խափանումը:
- **Ավարտում p95 > վերջնաժամկետի թուլացում միջին **
  - Շեմը՝ `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - Գործողություն. Ստուգեք, որ մատակարարները պարտավորվում են մինչև վերջնաժամկետները. մտածեք վերահանձնումներ տալու մասին:

### Օրինակ Prometheus Կանոններ

```yaml
groups:
  - name: sorafs-pin-registry
    rules:
      - alert: SorafsReplicationSlaDrop
        expr: sum(torii_sorafs_replication_sla_total{outcome="met"}) /
          clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) < 0.95
        for: 15m
        labels:
          severity: page
        annotations:
          summary: "SoraFS replication SLA below target"
          description: "SLA success ratio stayed under 95% for 15 minutes."

      - alert: SorafsReplicationBacklogGrowing
        expr: torii_sorafs_replication_backlog_total > 10
        for: 10m
        labels:
          severity: page
        annotations:
          summary: "SoraFS replication backlog above threshold"
          description: "Pending replication orders exceeded the configured backlog budget."

      - alert: SorafsReplicationExpiredOrders
        expr: increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0
        for: 0m
        labels:
          severity: ticket
        annotations:
          summary: "SoraFS replication orders expired"
          description: "At least one replication order expired in the last five minutes."
```

## Triage Workflow

1. **Բացահայտեք պատճառը**
   - Եթե SLA-ն բաց է թողնում հասկը, մինչդեռ կուտակվածը մնում է ցածր, կենտրոնացեք մատակարարի աշխատանքի վրա (PoR ձախողումներ, ուշ ավարտումներ):
   - Եթե կուտակումները աճում են կայուն բացթողումներով, ստուգեք ընդունելությունը (`/v2/sorafs/pin/*`)՝ հաստատելու մանիֆեստները, որոնք սպասում են խորհրդի հաստատմանը:
2. **Վավերացրեք մատակարարի կարգավիճակը**
   - Գործարկեք `iroha app sorafs providers list` և ստուգեք, որ գովազդվող հնարավորությունները համապատասխանում են կրկնօրինակման պահանջներին:
   - Ստուգեք `torii_sorafs_capacity_*` չափիչները՝ հաստատելու տրամադրված GiB և PoR հաջողությունը:
3. **Վերահանձնարարել կրկնօրինակում**
   - Նոր պատվերներ թողարկեք `sorafs_manifest_stub capacity replication-order`-ի միջոցով, երբ հետաձգված թուլությունը (`stat="avg"`) իջնում է 5 դարաշրջանից ցածր (մանիֆեստ/Ավտոմեքենայի փաթեթավորումն օգտագործում է `iroha app sorafs toolkit pack`):
   - Տեղեկացրեք կառավարմանը, եթե կեղծանունները չունեն ակտիվ մանիֆեստային կապեր (`torii_sorafs_registry_aliases_total` անսպասելիորեն նվազում է):
4. **Փաստաթղթի արդյունքը**
   - Գրանցեք միջադեպերի նշումներ SoraFS գործառնությունների մատյանում՝ ժամանակի դրոշմակնիքներով և ազդակիր մանիֆեստների ամփոփումներով:
   - Թարմացրեք այս գրքույկը, եթե ներկայացվեն ձախողման նոր ռեժիմներ կամ վահանակներ:

## Տարածման պլան

Արտադրության մեջ կեղծանունների քեշի քաղաքականությունը միացնելիս կամ խստացնելիս հետևեք այս փուլային ընթացակարգին.1. **Պատրաստել կոնֆիգուրացիան **
   - Թարմացրեք `torii.sorafs_alias_cache`-ը `iroha_config`-ում (օգտագործող → փաստացի) համաձայնեցված TTL-ներով և շնորհակալ պատուհաններով՝ `positive_ttl`, `refresh_window`, Prometheus `revocation_ttl`, `rotation_max_age`, `successor_grace` և `governance_grace`: Կանխադրվածները համապատասխանում են `docs/source/sorafs_alias_policy.md` քաղաքականությանը:
   - SDK-ների համար բաշխեք նույն արժեքները դրանց կազմաձևման շերտերի միջոցով (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` Rust / NAPI / Python կապում), որպեսզի հաճախորդի կիրառումը համապատասխանի դարպասին:
2. **Չոր վազք բեմականացման մեջ**
   - Տեղադրեք կոնֆիգուրացիայի փոփոխությունը բեմական կլաստերի մեջ, որը արտացոլում է արտադրության տոպոլոգիան:
   - Գործարկեք `cargo xtask sorafs-pin-fixtures`-ը՝ հաստատելու, որ կանոնական կեղծանունների սարքերը դեռ վերծանվում են և շրջագայվում; Ցանկացած անհամապատասխանություն ենթադրում է վերին հոսքի բացահայտ շեղում, որը պետք է նախ լուծվի:
   - Կիրառեք `/v2/sorafs/pin/{digest}` և `/v2/sorafs/aliases` վերջնակետերը սինթետիկ ապացույցներով, որոնք ծածկում են թարմ, թարմացման պատուհանը, ժամկետանց և ժամկետանց դեպքերը: Վավերացրեք HTTP կարգավիճակի կոդերը, վերնագրերը (`Sora-Proof-Status`, `Retry-After`, `Warning`) և JSON մարմնի դաշտերը այս վազքագրքի նկատմամբ:
3. **Միացնել արտադրության մեջ**
   - Նոր կոնֆիգուրացիան տարածեք ստանդարտ փոփոխության պատուհանի միջոցով: Կիրառեք այն նախ Torii-ին, այնուհետև վերագործարկեք gateways/SDK ծառայությունները, երբ հանգույցը հաստատի նոր քաղաքականությունը գրանցամատյաններում:
   - Ներմուծեք `docs/source/grafana_sorafs_pin_registry.json`-ը Grafana-ում (կամ թարմացրեք առկա վահանակները) և ամրացրեք կեղծանունների քեշի թարմացման վահանակները NOC-ի աշխատանքային տարածքին:
4. **Հետտեղակայման ստուգում**
   - Մոնիտոր `torii_sorafs_alias_cache_refresh_total` և `torii_sorafs_alias_cache_age_seconds` 30 րոպե: `error`/`expired` կորերի հասկերը պետք է փոխկապակցվեն քաղաքականության թարմացման պատուհանների հետ. անսպասելի աճը նշանակում է, որ օպերատորները պետք է ստուգեն կեղծանունների ապացույցները և մատակարարի առողջությունը, նախքան շարունակելը:
   - Հաստատեք, որ հաճախորդի կողմից տեղեկամատյանները ցույց են տալիս քաղաքականության նույն որոշումները (SDK-ները կհայտնեն սխալներ, երբ ապացույցը հնացած է կամ ժամկետանց): Հաճախորդի նախազգուշացումների բացակայությունը վկայում է սխալ կազմաձևման մասին:
5. **Հետադարձ **
   - Եթե կեղծանունների թողարկումը հետ է մնում, և թարմացման պատուհանը հաճախակի է գործարկվում, ժամանակավորապես թուլացրեք քաղաքականությունը՝ ավելացնելով `refresh_window` և `positive_ttl` կոնֆիգուրայում, այնուհետև վերաբաշխեք: Պահպանեք `hard_expiry`-ը անձեռնմխելի, որպեսզի իսկապես հնացած ապացույցները դեռ մերժվեն:
   - Վերադարձեք նախկին կազմաձևին՝ վերականգնելով նախորդ `iroha_config` լուսանկարը, եթե հեռաչափությունը շարունակում է ցույց տալ `error` բարձրացված թվերը, այնուհետև բացեք միջադեպ՝ կեղծանունների ստեղծման ուշացումները հետագծելու համար:

## Հարակից նյութեր

- `docs/source/sorafs/pin_registry_plan.md` — իրականացման ճանապարհային քարտեզ և կառավարման համատեքստ:
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` — պահեստավորման աշխատողի գործառնություններ, լրացնում է այս ռեեստրի գրքույկը:

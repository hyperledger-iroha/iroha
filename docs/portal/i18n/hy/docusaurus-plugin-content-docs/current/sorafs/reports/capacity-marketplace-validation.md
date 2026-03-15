---
lang: hy
direction: ltr
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Capacity Marketplace Validation
tags: [SF-2c, acceptance, checklist]
summary: Acceptance checklist covering provider onboarding, dispute workflows, and treasury reconciliation gating the SoraFS capacity marketplace general availability.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS Կարողությունների շուկայի վավերացման ստուգաթերթ

**Վերանայման պատուհան՝ ** 2026-03-18 → 2026-03-24  
**Ծրագրի սեփականատերերը.** Պահպանման թիմ (`@storage-wg`), Կառավարման խորհուրդ (`@council`), Գանձապետական գիլդիա (`@treasury`)  
**Շրջանակ.** Ներբեռնման խողովակաշարերի մատակարար, վեճերի քննության հոսքեր և գանձապետական հաշտեցման գործընթացներ, որոնք անհրաժեշտ են SF-2c GA-ի համար:

Ստորև բերված ստուգաթերթը պետք է վերանայվի՝ նախքան արտաքին օպերատորների համար շուկան միացնելը: Յուրաքանչյուր տող կապվում է դետերմինիստական ​​ապացույցների (թեստեր, հարմարանքներ կամ փաստաթղթեր), որոնք աուդիտորները կարող են կրկնել:

## Ընդունման ստուգաթերթ

### Մատակարարի միացում

| Ստուգեք | Վավերացում | Ապացույցներ |
|-------|------------|----------|
| Ռեեստրն ընդունում է կարողությունների կանոնական հայտարարագրեր | Ինտեգրման թեստային վարժություններ `/v1/sorafs/capacity/declare` հավելվածի API-ի միջոցով՝ ստուգելով ստորագրությունների մշակումը, մետատվյալների հավաքագրումը և հանձնումը հանգույցի ռեեստր: | `crates/iroha_torii/src/routing.rs:7654` |
| Խելացի պայմանագիրը մերժում է անհամապատասխան բեռը | Միավորի փորձարկումն ապահովում է, որ մատակարարի ID-ները և պարտավորված GiB դաշտերը համընկնում են ստորագրված հայտարարագրի հետ՝ նախքան պահպանվելը: | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| CLI-ն արտանետում է կանոնական տեղակայման արտեֆակտներ | CLI զրահը գրում է Norito/JSON/Base64 դետերմինիստական ​​ելքերը և վավերացնում է շրջագայությունները, որպեսզի օպերատորները կարողանան հայտարարություններ անել անցանց: | `crates/sorafs_car/tests/capacity_cli.rs:17` |
| Օպերատորի ուղեցույցը ֆիքսում է ընդունելության աշխատանքային ընթացքը և կառավարման պահակակետերը | Փաստաթղթերը թվարկում են հայտարարագրման սխեման, քաղաքականության լռելյայնությունները և խորհրդի համար վերանայման քայլերը: | `../storage-capacity-marketplace.md` |

### Վեճերի լուծում

| Ստուգեք | Վավերացում | Ապացույցներ |
|-------|------------|----------|
| Վեճերի գրառումները պահպանվում են կանոնական օգտակար բեռների ամփոփման հետ | Միավոր թեստը գրանցում է վեճը, վերծանում է պահված օգտակար բեռը և հաստատում է առկախ կարգավիճակը՝ երաշխավորելու մատյանների դետերմինիզմը: | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| CLI վեճերի գեներատորը համապատասխանում է կանոնական սխեմային | CLI թեստն ընդգրկում է Base64/Norito ելքերը և JSON ամփոփագրերը `CapacityDisputeV1`-ի համար՝ ապահովելով ապացույցների փաթեթների հեշը վճռականորեն: | `crates/sorafs_car/tests/capacity_cli.rs:455` |
| Կրկնակի թեստը ապացուցում է վեճի/տուգանային դետերմինիզմը | Երկու անգամ կրկնվող ապացույցների ձախողման հեռաչափությունը ստեղծում է միանման մատյան, վարկային և վեճի պատկերներ, այնպես որ շեղերը որոշիչ են հասակակիցների համար: | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| Runbook փաստաթղթերի ընդլայնման և չեղյալ հայտարարման հոսքը | Գործառնությունների ուղեցույցը ներառում է խորհրդի աշխատանքի ընթացքը, ապացույցների պահանջները և հետադարձման ընթացակարգերը: | `../dispute-revocation-runbook.md` |

### Գանձապետական հաշտեցում

| Ստուգեք | Վավերացում | Ապացույցներ |
|-------|------------|----------|
| Լեջերի հաշվեգրումը համապատասխանում է 30-օրյա ներծծման կանխատեսմանը | Soak test-ը ընդգրկում է հինգ մատակարարների 30 հաշվարկային պատուհանների վրա՝ տարբերելով մատյանային գրառումները ակնկալվող վճարման տեղեկանքից: | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
| Գրանցամատյանի արտահանման համաձայնեցումը գրանցվել է ամեն գիշեր | `capacity_reconcile.py`-ը համեմատում է վճարների հաշվապահական հաշվառման ակնկալիքները կատարված XOR փոխանցումների արտահանման հետ, թողարկում է Prometheus չափումներ և ապահովում է գանձապետարանի հաստատումը Alertmanager-ի միջոցով: | `scripts/telemetry/capacity_reconcile.py:1`,`docs/source/sorafs/runbooks/capacity_reconciliation.md:1`,`dashboards/alerts/sorafs_capacity_rules.yml:100` |
| Վճարման վահանակների մակերեսային տույժեր և հաշվեգրման հեռաչափություն | Grafana ներմուծման սյուժեները գծագրում են GiB·ժամ հաշվեգրում, գործադուլի հաշվիչներ և խճճված գրավ՝ հերթապահության տեսանելիության համար: | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
| Հրապարակված հաշվետվությունների արխիվները ներծծում են մեթոդաբանությունը և վերարտադրման հրամանները | Հաշվետվության մանրամասների ներծծման շրջանակը, կատարման հրամանները և աուդիտորների համար տեսանելիության կեռիկները: | `./sf2c-capacity-soak.md` |

## Կատարման նշումներ

Վերագործարկեք վավերացման փաթեթը նախքան ստորագրումը.

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

Օպերատորները պետք է `sorafs_manifest_stub capacity {declaration,dispute}`-ով վերագեներացնեն ներբեռնման/վիճարկման հարցումների օգտակար բեռները և արխիվացնեն ստացված JSON/Norito բայթերը կառավարման տոմսի կողքին:

## Գրանցման արտեֆակտներ

| Արտեֆակտ | Ճանապարհ | blake2b-256 |
|----------|------|-------------|
| Մատակարարի մուտքի հաստատման փաթեթ | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
| Վեճերի լուծման հաստատման փաթեթ | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
| Գանձապետարանի հաշտեցման հաստատման փաթեթ | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

Պահպանեք այս արտեֆակտների ստորագրված պատճենները թողարկման փաթեթի հետ և կապեք դրանք կառավարման փոփոխությունների արձանագրության մեջ:

## Հաստատումներ

- Պահպանման թիմի առաջատար — @storage-tl (2026-03-24)  
- Կառավարման խորհրդի քարտուղար — @council-sec (2026-03-24)  
- Գանձապետական գործառնությունների առաջատար — @treasury-ops (2026-03-24)
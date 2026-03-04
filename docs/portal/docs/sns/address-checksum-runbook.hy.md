---
lang: hy
direction: ltr
source: docs/portal/docs/sns/address-checksum-runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fcd909a7013c5147e4f0c89c67de856ff56797b99281b954c7708ad83ab5cdc8
source_last_modified: "2026-01-28T17:11:30.699790+00:00"
translation_last_reviewed: 2026-02-07
id: address-checksum-runbook
title: Account Address Checksum Incident Runbook
sidebar_label: Checksum incidents
description: Operational response for IH58 (preferred) / compressed (`sora`, second-best) checksum failures (ADDR-7).
translator: machine-google-reviewed
---

:::note Կանոնական աղբյուր
Այս էջը արտացոլում է `docs/source/sns/address_checksum_failure_runbook.md`: Թարմացնել
սկզբում սկզբնաղբյուր ֆայլը, ապա համաժամացրեք այս պատճենը:
:::

Ստուգիչ գումարի խափանումները երևում են որպես `ERR_CHECKSUM_MISMATCH` (`ChecksumMismatch`)
Torii, SDK-ներ և դրամապանակ/հետախուզող հաճախորդներ: ADDR-6/ADDR-7 ճանապարհային քարտեզի կետերն այժմ
պահանջել օպերատորներից հետևել այս մատյանին, երբ ստուգիչ գումարի ծանուցումները կամ աջակցությունը
տոմսերի կրակ.

## Ե՞րբ գործարկել խաղը

- **Զգուշացումներ.** `AddressInvalidRatioSlo` (սահմանված է
  `dashboards/alerts/address_ingest_rules.yml`) ուղևորություններ և ծանոթագրությունների ցանկ
  `reason="ERR_CHECKSUM_MISMATCH"`.
- **Կառուցվածքային դրեյֆ.** `account_address_fixture_status` Prometheus տեքստային ֆայլ կամ
  Grafana վահանակը հաղորդում է ստուգիչ գումարի անհամապատասխանություն SDK-ի ցանկացած պատճենի համար:
- **Աջակցեք սրացումներին.** Դրամապանակ/հետախույզ/SDK թիմերը նշում են ստուգիչ գումարի սխալները, IME
  կոռուպցիա կամ սեղմատախտակի սկանավորումներ, որոնք այլևս չեն վերծանվում:
- **Ձեռքով դիտում.** Torii տեղեկամատյանները ցույց են տալիս կրկնվող `address_parse_error=checksum_mismatch`
  արտադրության վերջնակետերի համար։

Եթե միջադեպը կոնկրետ Local-8/Local-12 բախումների մասին է, հետևեք
Փոխարենը `AddressLocal8Resurgence` կամ `AddressLocal12Collision` գրքույկներ:

## Ապացույցների ստուգաթերթ

| Ապացույցներ | Հրաման / Տեղադրություն | Ծանոթագրություններ |
|----------|------------------|-------|
| Grafana լուսանկար | `dashboards/grafana/address_ingest.json` | Լուսանկարեք անվավեր պատճառների խզումները և ազդակիր վերջնակետերը: |
| Զգուշացման ծանրաբեռնվածություն | PagerDuty/Slack + `dashboards/alerts/address_ingest_rules.yml` | Ներառեք համատեքստի պիտակներ և ժամանակի դրոշմանիշներ: |
| Հարմարավետության առողջական | `artifacts/account_fixture/address_fixture.prom` + Grafana | Ապացուցում է, թե արդյոք SDK-ի պատճենները շեղվել են `fixtures/account/address_vectors.json`-ից: |
| PromQL հարցում | `sum by (context) (increase(torii_address_invalid_total{reason="ERR_CHECKSUM_MISMATCH"}[5m]))` | Արտահանել CSV միջադեպի փաստաթղթի համար: |
| Տեղեկամատյաններ | `journalctl -u iroha_torii --since -30m | rg 'checksum_mismatch'` (կամ տեղեկամատյանների համախմբում) | Մաքրեք PII-ը նախքան համօգտագործելը: |
| Սարքավորումների ստուգում | `cargo xtask address-vectors --verify` | Հաստատում է, որ կանոնական գեներատորը և պարտավորված JSON-ը համաձայն են: |
| SDK պարիտետի ստուգում | `python3 scripts/account_fixture_helper.py check --target <path> --metrics-out artifacts/account_fixture/<label>.prom --metrics-label <label>` | Գործարկեք յուրաքանչյուր SDK-ի համար, որը նշված է ազդանշաններում/տոմսերում: |
| Clipboard/IME ողջախոհություն | `iroha tools address inspect <literal>` | Հայտնաբերում է թաքնված նիշերը կամ IME-ի վերագրումները. մեջբերում `address_display_guidelines.md`. |

## Անմիջական արձագանք

1. Ընդունեք ծանուցումը, կապեք Grafana snapshots + PromQL ելքը միջադեպի մեջ
   շարանը և նշումը ազդել է Torii համատեքստերի վրա:
2. Սառեցնել մանիֆեստի առաջխաղացումները / SDK-ն թողարկում է հասցեների վերլուծություն:
3. Պահպանեք վահանակի նկարները և ստեղծված Prometheus տեքստային ֆայլի արտեֆակտները
   միջադեպի թղթապանակը (`docs/source/sns/incidents/YYYY-MM/<ticket>/`):
4. Քաշեք տեղեկամատյանների նմուշները, որոնք ցույց են տալիս `checksum_mismatch` օգտակար բեռները:
5. Տեղեկացրեք SDK-ի սեփականատերերին (`#sdk-parity`) նմուշների օգտակար բեռների հետ, որպեսզի նրանք կարողանան տրաժավորել:

## Արմատային պատճառի մեկուսացում

### Հարմարանք կամ գեներատորի դրեյֆ

- Կրկին գործարկել `cargo xtask address-vectors --verify`; վերականգնել, եթե այն ձախողվի:
- Կատարեք `ci/account_fixture_metrics.sh` (կամ անհատական
  `scripts/account_fixture_helper.py check`) յուրաքանչյուր SDK-ի համար՝ փաթեթը հաստատելու համար
  հարմարանքները համապատասխանում են կանոնական JSON-ին:

### Հաճախորդի կոդավորիչներ / IME ռեգրեսիաներ

- Ստուգեք օգտագործողի կողմից տրված բառացիները `iroha tools address inspect`-ի միջոցով՝ զրոյական լայնությունը գտնելու համար
  միացումներ, կանայի փոխարկումներ կամ կրճատված բեռներ:
- Խաչաձև ստուգեք դրամապանակը/հետախուզողը հոսում է
  `docs/source/sns/address_display_guidelines.md` (կրկնակի պատճենի թիրախներ, նախազգուշացումներ,
  QR օգնականներ) ապահովելու համար, որ նրանք հետևում են հաստատված UX-ին:

### Մանիֆեստի կամ ռեգիստրի խնդիրներ

- Հետևեք `address_manifest_ops.md`-ին՝ վերջին մանիֆեստի փաթեթը կրկին վավերացնելու համար և
  ապահովել, որ ոչ մի Local-8 ընտրիչ նորից հայտնվի:
  հայտնվել օգտակար բեռների մեջ:

### Վնասակար կամ սխալ ձևավորված երթևեկություն

- Կոտրեք վիրավորական IP-ները/հավելվածների ID-ները Torii տեղեկամատյանների և `torii_http_requests_total`-ի միջոցով:
- Պահպանեք առնվազն 24 ժամ տեղեկամատյաններ Անվտանգության/Կառավարման հետաքննության համար:

## Մեղմացում և վերականգնում

| Սցենար | Գործողություններ |
|----------|---------|
| Հարմարանքների դրեյֆ | Վերարտադրեք `fixtures/account/address_vectors.json`, նորից գործարկեք `cargo xtask address-vectors --verify`, թարմացրեք SDK փաթեթները և կցեք `address_fixture.prom` նկարները տոմսին: |
| SDK/հաճախորդի ռեգրեսիա | Ֆայլի հետ կապված խնդիրներ, որոնք վերաբերում են կանոնական սարքին + `iroha tools address inspect` ելքին և դարպասի թողարկումներին SDK հավասարաչափ CI-ի հետևում (օրինակ՝ `ci/check_address_normalize.sh`): |
| Վնասակար ներկայացումներ | Սահմանափակեք կամ արգելափակեք վիրավորող տնօրեններին, վերածեք Կառավարման, եթե գերեզմանաքարերի ընտրիչներ են պահանջվում: |

Հենց որ մեղմացումները կատարվեն, նորից գործարկեք վերը նշված PromQL հարցումը՝ հաստատելու համար
`ERR_CHECKSUM_MISMATCH`-ը մնում է զրոյի (բացառությամբ `/tests/*`-ի) առնվազն
Միջադեպի վարկանիշը նվազեցնելուց 30 րոպե առաջ:

## Փակում

1. Արխիվացրեք Grafana snapshots, PromQL CSV, գրանցամատյանների քաղվածքներ և `address_fixture.prom`:
2. Թարմացրեք `status.md` (ADDR բաժինը) և ճանապարհային քարտեզի տողը, եթե գործիքավորումը/փաստաթղթերը
   փոխվել է.
3. Պատահարից հետո գրառումներ կատարեք `docs/source/sns/incidents/` տակ, երբ նոր դասեր կան
   առաջանալ.
4. Համոզվեք, որ SDK-ի թողարկման նշումներում անհրաժեշտության դեպքում նշվում են ստուգիչ գումարի շտկումները:
5. Հաստատեք, որ ահազանգը մնում է կանաչ 24 ժամ, իսկ սարքերի ստուգումները նախկինում մնում են կանաչ
   լուծելով.
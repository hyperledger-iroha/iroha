---
lang: hy
direction: ltr
source: docs/portal/docs/sorafs/deal-engine.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6404e09aa8f3520328249a1d5c41309b291087908a2a8f5abae3e2fe12de44fb
source_last_modified: "2026-01-05T09:28:11.861409+00:00"
translation_last_reviewed: 2026-02-07
id: deal-engine
title: SoraFS Deal Engine
sidebar_label: Deal Engine
description: Overview of the SF-8 deal engine, Torii integration, and telemetry surfaces.
translator: machine-google-reviewed
---

:::note Կանոնական աղբյուր
:::

# SoraFS Գործարքի շարժիչ

SF-8 ճանապարհային քարտեզը ներկայացնում է SoraFS գործարքային շարժիչը՝ ապահովելով
միջեւ պահեստավորման եւ որոնման պայմանագրերի որոշիչ հաշվառում
հաճախորդներ և մատակարարներ: Պայմանագրերը նկարագրված են Norito օգտակար բեռների հետ
սահմանված `crates/sorafs_manifest/src/deal.rs`-ում, որը ներառում է գործարքի պայմանները, պարտատոմսը
կողպում, հավանական միկրովճարումներ և հաշվարկային գրառումներ:

Ներկառուցված SoraFS աշխատողը (`sorafs_node::NodeHandle`) այժմ ներկայացնում է
`DealEngine` օրինակ յուրաքանչյուր հանգույցի գործընթացի համար: Շարժիչը.

- վավերացնում և գրանցում է գործարքները՝ օգտագործելով `DealTermsV1`;
- կուտակում է XOR-ով արտահայտված վճարներ, երբ հաղորդվում է կրկնօրինակման օգտագործման մասին.
- գնահատում է հավանական միկրովճարման պատուհանները՝ օգտագործելով դետերմինիստական
  Blake3-ի վրա հիմնված նմուշառում; և
- արտադրում է մատյանների պատկերներ և կառավարման համար հարմար հաշվարկային բեռներ
  հրատարակչական։

Միավորների թեստերը ներառում են վավերացումը, միկրովճարման ընտրությունը և հաշվարկային հոսքերը
օպերատորները կարող են վստահորեն կիրառել API-ները: Բնակավայրերն այժմ արտանետում են
`DealSettlementV1` կառավարման օգտակար բեռներ, լարերը անմիջապես SF-12-ի մեջ
հրատարակելով խողովակաշարը և թարմացնել `sorafs.node.deal_*` OpenTelemetry շարքը
(`deal_settlements_total`, `deal_expected_charge_nano`, `deal_client_debit_nano`,
`deal_outstanding_nano`, `deal_bond_slash_nano`, `deal_publish_total`) Torii վահանակների և SLO-ի համար
կիրարկումը։ Հետագա կետերը կենտրոնանում են աուդիտորի կողմից նախաձեռնված կրճատման ավտոմատացման վրա և
չեղարկման իմաստաբանության համակարգումը կառավարման քաղաքականության հետ:

Օգտագործման հեռաչափությունն այժմ նաև ապահովում է `sorafs.node.micropayment_*` չափումների հավաքածուն.
`micropayment_charge_nano`, `micropayment_credit_generated_nano`,
`micropayment_credit_applied_nano`, `micropayment_credit_carry_nano`,
`micropayment_outstanding_nano`, իսկ տոմսերի վաճառասեղանները
(`micropayment_tickets_processed_total`, `micropayment_tickets_won_total`,
`micropayment_tickets_duplicate_total`): Այս գումարները բացահայտում են հավանականությունը
վիճակախաղի հոսքը, որպեսզի օպերատորները կարողանան փոխկապակցել միկրովճարումների շահումները և վարկերի փոխանցումը
կարգավորման արդյունքներով։

## Torii Ինտեգրում

Torii-ը բացահայտում է հատուկ վերջնակետերը, որպեսզի մատակարարները կարողանան զեկուցել օգտագործման մասին և առաջ տանել
գործարքի կյանքի ցիկլը առանց պատվիրված լարերի.

- `POST /v1/sorafs/deal/usage` ընդունում է `DealUsageReport` հեռաչափությունը և վերադառնում
  դետերմինիստական հաշվառման արդյունքներ (`UsageOutcome`):
- `POST /v1/sorafs/deal/settle`-ն ավարտում է ընթացիկ պատուհանը, հոսում է
  ստացվում է `DealSettlementRecord`՝ բազային64 կոդավորված `DealSettlementV1`-ի հետ մեկտեղ
  պատրաստ է կառավարման DAG հրապարակմանը.
- Torii-ի `/v1/events/sse` հոսքը այժմ հեռարձակում է `SorafsGatewayEvent::DealUsage`
  յուրաքանչյուր օգտագործման ներկայացումն ամփոփող գրառումներ (դարաշրջան, չափված GiB-ժամեր, տոմս
  հաշվիչներ, դետերմինիստական լիցքեր), `SorafsGatewayEvent::DealSettlement`
  գրառումներ, որոնք ներառում են կանոնական կարգավորման մատյանների նկարը գումարած
  BLAKE3 digest/size/base64 սկավառակի վրա կառավարման արտեֆակտի, և
  `SorafsGatewayEvent::ProofHealth` ահազանգում է, երբ PDP/PoTR շեմերը
  գերազանցվել է (մատակարար, պատուհան, գործադուլ/հովացման վիճակ, տույժի չափ): Սպառողները կարող են
  զտել մատակարարի կողմից՝ արձագանքելու նոր հեռաչափությանը, բնակավայրերին կամ առողջության հաստատման ազդանշաններին՝ առանց հարցումների:

Երկու վերջնակետերը մասնակցում են SoraFS քվոտայի շրջանակին նորի միջոցով
`torii.sorafs.quota.deal_telemetry` պատուհանը, որը թույլ է տալիս օպերատորներին կարգավորել
թույլատրված ներկայացման տոկոսադրույքը մեկ տեղակայման համար:
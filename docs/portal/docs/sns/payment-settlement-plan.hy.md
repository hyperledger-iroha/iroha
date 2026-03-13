---
lang: hy
direction: ltr
source: docs/portal/docs/sns/payment-settlement-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1be9268784bf75c4c5d1bf854e72c817475a079c0d2bf06ce120ccd325ad6083
source_last_modified: "2026-01-22T14:45:01.248924+00:00"
translation_last_reviewed: 2026-02-07
id: payment-settlement-plan
title: SNS Payment & Settlement Plan
sidebar_label: Payment & settlement plan
description: Playbook for routing SNS registrar revenue, reconciling steward/treasury splits, and producing evidence bundles.
translator: machine-google-reviewed
---

> Կանոնական աղբյուր՝ [`docs/source/sns/payment_settlement_plan.md`](../../../source/sns/payment_settlement_plan.md):

Ճանապարհային քարտեզի առաջադրանքը **SN-5 — Վճարումների և հաշվարկների ծառայություն** ներկայացնում է դետերմինիստիկա
Sora Name Service-ի վճարման շերտը: Յուրաքանչյուր գրանցում, նորացում կամ փոխհատուցում
պետք է թողարկի կառուցվածքային Norito ծանրաբեռնվածություն, որպեսզի գանձապետարանը, կառավարիչները և կառավարումը կարողանան
վերարտադրել ֆինանսական հոսքերն առանց աղյուսակների: Այս էջը թորում է սպեկտրը
պորտալի լսարանի համար:

## Եկամուտի մոդել

- Հիմնական վճարը (`gross_fee`) բխում է գրանցողի գնագոյացման մատրիցից:  
- Գանձապետարանը ստանում է `gross_fee × 0.70`, ստյուարդները ստանում են մնացած մինուսը
  ուղղորդման բոնուսներ (սահմանված է 10%):  
- Ընտրովի հետաձգումները թույլ են տալիս կառավարմանը դադարեցնել տնտեսվարողի վճարումները վեճերի ժամանակ:  
- Հաշվարկային կապոցները բացահայտում են `ledger_projection` բլոկը բետոնով
  `Transfer` ISI-ներ, որպեսզի ավտոմատացումը կարողանա տեղադրել XOR շարժումները ուղիղ Torii-ի մեջ:

## Ծառայություններ և ավտոմատացում

| Բաղադրիչ | Նպատակը | Ապացույցներ |
|-----------|---------|----------|
| `sns_settlementd` | Կիրառում է քաղաքականություն, ստորագրում է փաթեթները, մակերեսները `/v2/sns/settlements`: | JSON փաթեթ + հեշ: |
| Հաշվարկային հերթ & գրող | Idempotent հերթ + մատյան ներկայացնող, որը վարում է `iroha_cli app sns settlement ledger`: | Փաթեթի հեշ ↔ tx հեշ մանիֆեստ: |
| Հաշտեցման աշխատանք | Օրական տարբերություն + ամսական քաղվածք `docs/source/sns/reports/`-ի ներքո: | Markdown + JSON ամփոփում: |
| Փոխհատուցման գրասեղան | Կառավարության կողմից հաստատված փոխհատուցումներ `/settlements/{id}/refund`-ի միջոցով: | `RefundRecordV1` + տոմս. |

CI օգնականները արտացոլում են այս հոսքերը.

```bash
# Quote & ledger projection
iroha_cli app sns settlement quote --selector makoto.sora --term-years 1 --pricing hot-tier-a

# Emit transfers for automation/pipeline
iroha_cli app sns settlement ledger --bundle artifacts/sns/settlements/2026-05/makoto.sora.json

# Produce a reconciliation statement
iroha_cli app sns settlement reconcile --period 2026-05 --out docs/source/sns/reports/settlement_202605.md
```

## Դիտորդականություն և հաշվետվություն

- Վահանակներ՝ `dashboards/grafana/sns_payment_settlement.json` գանձապետարանի համար ընդդեմ
  Ստյուարդի ընդհանուր գումարները, ուղղորդման վճարումները, հերթի խորությունը և փոխհատուցման ուշացումը:
- Զգուշացումներ. `dashboards/alerts/sns_payment_settlement_rules.yml` մոնիտորներ սպասում են
  տարիք, հաշտեցման ձախողումներ և մատյանում տեղաշարժ:
- Հայտարարություններ. ամենօրյա ամփոփումները (`settlement_YYYYMMDD.{json,md}`) վերածվում են ամսական
  հաշվետվություններ (`settlement_YYYYMM.md`), որոնք վերբեռնվում են և՛ Git, և՛ the
  կառավարման օբյեկտների պահեստ (`s3://sora-governance/sns/settlements/<period>/`):
- Կառավարման փաթեթները միավորում են վահանակները, CLI մատյանները և հաստատումները խորհրդի առաջ
  ստորագրում.

## Տարածման ստուգաթերթ

1. Նախատիպ մեջբերում + մատյանների օգնականներ և ֆիքսեք բեմական փաթեթը:
2. Գործարկեք `sns_settlementd`-ը հերթով + գրիչով, լարային վահանակներով և վարժություններով
   զգուշացման թեստեր (`promtool test rules ...`):
3. Տրամադրել փոխհատուցման օգնական, գումարած ամսական քաղվածքի ձևանմուշ; հայելային արտեֆակտներ մեջ
   `docs/portal/docs/sns/reports/`.
4. Գործարկել գործընկերոջ փորձը (բնակավայրերի ամբողջ ամիս) և գրավել այն
   կառավարման քվեարկությունը՝ նշելով SN-5-ը որպես ավարտված:

Վերադարձեք սկզբնաղբյուր փաստաթղթին՝ սխեմայի ճշգրիտ սահմանումների համար, բաց
հարցեր և ապագա փոփոխություններ։
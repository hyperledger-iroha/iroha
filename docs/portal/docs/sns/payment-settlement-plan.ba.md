---
lang: ba
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

> Каноник сығанаҡ: [`docs/source/sns/payment_settlement_plan.md`] (../../../source/sns/payment_settlement_plan.md).

Юл картаһы бурысы **SN-5 — Түләү һәм ҡасаба хеҙмәте** детерминистик индерә
Сора Исем хеҙмәте өсөн түләү ҡатламы. Һәр теркәү, яңыртыу, йәки ҡайтарыу .
тейеш сығарырға структуралы I18NT000000000000000 файҙалы йөк шулай ҡаҙна, стюардтар, һәм идара итеү ала
матди ағымдарҙы таблицаһыҙ ҡабатлай. Был бит spe spe .
портал аудиторияһы өсөн.

## Килем моделе

- База түләүе (`gross_fee`) теркәүсе хаҡтар матрицаһынан килеп сыға.  
- Ҡаҙна I18NI000000006X ала, стюардтар ҡалған минус ала.
  йүнәлтмә бонустары (10%-та ҡапланған).  
- Һорау алыуҙа тотҡарлыҡтар идара итеүгә бәхәстәр ваҡытында идара итеүҙе стеуард түләүҙәрен туҡтатырға мөмкинлек бирә.  
- Ҡасаба өйөмдәре I18NI000000007X блокын бетон менән фашлай
  `Transfer` ИСИ-лар шулай автоматлаштырыу XOR хәрәкәттәрен туранан-тура I18NT000000001X-ҡа урынлаштыра ала.

## Хеҙмәт & автоматлаштырыу

| Компонент | Маҡсат | Дәлилдәр |
|---------|----------|-----------|
| `sns_settlementd` | Ҡулланыу сәйәсәте, билдәләр өйөмдәр, өҫтө I18NI000000010X. | JSON өйөм + хеш. |
| Ҡасаба сираты & яҙыусы | Идемпотент сират + леджер тапшырыусы двигателдәре I18NI000000011X. | Блендл хеш ↔ tx хеш асыҡ. |
| Яраштырыу эше | Көндәлек дифф + айлыҡ белдереүҙе `docs/source/sns/reports/` буйынса. | Маркдаун + JSON дисциплинаһы. |
| Ҡайтарыу өҫтәле | Идара итеү раҫланған ҡайтарыуҙар аша I18NI000000013X. | I18NI0000014X + билет. |

CI ярҙамсылары был ағымдарҙы көҙгөләй:

```bash
# Quote & ledger projection
iroha_cli app sns settlement quote --selector makoto.sora --term-years 1 --pricing hot-tier-a

# Emit transfers for automation/pipeline
iroha_cli app sns settlement ledger --bundle artifacts/sns/settlements/2026-05/makoto.sora.json

# Produce a reconciliation statement
iroha_cli app sns settlement reconcile --period 2026-05 --out docs/source/sns/reports/settlement_202605.md
```

## Күҙәтеүсәнлек & отчет

- Приборҙар таҡталары: ҡаҙна өсөн I18NI0000000015X vs vs
  стюард дөйөм, йүнәлтмә түләүҙәре, сират тәрәнлеге, һәм ҡайтарыу латентлыҡ.
- Иҫкәртмәләр: I18NI0000000016X мониторҙары көтөлгән
  йәше, ярашыу етешһеҙлектәре һәм баш кейеме дрейфы.
- белдереүҙе: көндәлек һеңдереүҙең (I18NI000000017X) ай һайын ролл
  отчеттар (I18NI000000018X) улар Git һәм был тейәлгән
  идара итеү объекттары магазины (I18NI000000019X X).
- Идара итеү пакеттары пакеттар панелдәре, CLI журналдары, һәм раҫлауҙар алдынан совет
  ҡул ҡуйыу.

## Rollout тикшерелгән исемлек

1. Прототип цитата + баш һөйәге ярҙамсылары һәм сәхнәләштереү өйөмөн тотоу.
.
   иҫкәртмә һынауҙары (`promtool test rules ...`).
3. Ярҙамсы ҡайтарыу плюс айлыҡ белдереүҙе шаблон; көҙгө артефакттар инә
   `docs/portal/docs/sns/reports/`.
4. Партнер репетиция (тулы ай тораҡ пункттары) йүгерергә һәм тотоу
   идара итеү тауыш биреүҙе билдәләү SN-5 тулы.

Сығанаҡ документына мөрәжәғәт итегеҙ, аныҡ схема билдәләмәләре өсөн, асыҡ
һорауҙары, һәм киләсәктә үҙгәрештәр.
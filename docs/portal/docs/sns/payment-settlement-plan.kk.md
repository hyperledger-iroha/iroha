---
lang: kk
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

> Канондық көз: [`docs/source/sns/payment_settlement_plan.md`](../../../source/sns/payment_settlement_plan.md).

Жол картасының тапсырмасы **SN-5 — Төлем және есеп айырысу қызметі** детерминиститті енгізеді
Sora Name қызметі үшін төлем қабаты. Әрбір тіркеу, жаңарту немесе қайтару
құрылымдық Norito пайдалы жүктемені шығаруы керек, сондықтан қазынашылық, басқарушылар және басқару
қаржылық ағындарды электрондық кестелерсіз қайталаңыз. Бұл бет спецификацияны көрсетеді
портал аудиторияларына арналған.

## Табыс үлгісі

- Негізгі алым (`gross_fee`) тіркеушінің баға матрицасынан алынады.  
- Қазынашылық `gross_fee × 0.70` алады, басқарушылар қалған минусты алады
  жолдама бонустары (10%-бен шектелген).  
- Қосымша шектеулер басқаруға даулар кезінде басқарушы төлемдерін уақытша тоқтатуға мүмкіндік береді.  
- Шөгінділер `ledger_projection` блогын бетонмен ашады
  `Transfer` ISI автоматтандыру XOR қозғалыстарын тікелей Torii ішіне орналастыра алады.

## Қызметтер және автоматтандыру

| Құрамдас | Мақсаты | Дәлелдер |
|-----------|---------|----------|
| `sns_settlementd` | Саясатты қолданады, бумаларды, `/v1/sns/settlements` беттерін белгілейді. | JSON жинағы + хэш. |
| Есеп айырысу кезегі & жазушы | Idempotent кезек + `iroha_cli app sns settlement ledger` басқаратын бухгалтерлік кітап жіберуші. | Бума хэші ↔ tx хэш манифесті. |
| Салыстыру жұмысы | Күнделікті айырмашылық + `docs/source/sns/reports/` бойынша айлық есеп. | Markdown + JSON дайджест. |
| Қайтару үстелі | `/settlements/{id}/refund` арқылы басқару мақұлдаған қайтарулар. | `RefundRecordV1` + билет. |

CI көмекшілері мына ағындарды көрсетеді:

```bash
# Quote & ledger projection
iroha_cli app sns settlement quote --selector makoto.sora --term-years 1 --pricing hot-tier-a

# Emit transfers for automation/pipeline
iroha_cli app sns settlement ledger --bundle artifacts/sns/settlements/2026-05/makoto.sora.json

# Produce a reconciliation statement
iroha_cli app sns settlement reconcile --period 2026-05 --out docs/source/sns/reports/settlement_202605.md
```

## Бақылау және есеп беру

- Бақылау тақталары: `dashboards/grafana/sns_payment_settlement.json` қазынашылыққа қарсы
  басқарушы қорытындылары, жолдама төлемдері, кезек тереңдігі және қайтару кідірісі.
- Ескертулер: `dashboards/alerts/sns_payment_settlement_rules.yml` мониторлары күтілуде
  жасы, салыстыру сәтсіздігі және кітаптың дрейфі.
- Мәлімдеме: күнделікті дайджесттер (`settlement_YYYYMMDD.{json,md}`) ай сайынғы
  Git және сайтқа жүктеп салынған есептер (`settlement_YYYYMM.md`).
  басқару нысандарының қоймасы (`s3://sora-governance/sns/settlements/<period>/`).
- Басқару пакеттері бақылау тақталарын, CLI журналдарын және кеңес алдында мақұлдауларды біріктіреді
  шығу.

## Шығарылымды тексеру тізімі

1. Дәйексөз + кітап көмекшілерінің прототипін жасаңыз және кезеңдік буманы түсіріңіз.
2. `sns_settlementd` бағдарламасын кезек + жазушы, сым бақылау тақталары және жаттығу арқылы іске қосыңыз
   ескерту сынақтары (`promtool test rules ...`).
3. Қайтару көмекшісін және ай сайынғы есеп үлгісін жеткізіңіз; айна артефактілері
   `docs/portal/docs/sns/reports/`.
4. Серіктес репетициясын орындаңыз (есеп айырысулардың толық айы) және түсіріңіз
   басқару дауысы SN-5 аяқталды деп белгіленеді.

Нақты схема анықтамалары үшін бастапқы құжатты қараңыз, ашыңыз
сұрақтар және алдағы түзетулер.
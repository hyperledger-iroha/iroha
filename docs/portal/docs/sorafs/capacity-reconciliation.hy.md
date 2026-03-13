---
lang: hy
direction: ltr
source: docs/portal/docs/sorafs/capacity-reconciliation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e26cc8232dd7d3b392d56646fdfbf809952f017532a37aafbfde3c8cc704ae0e
source_last_modified: "2025-12-29T18:16:35.177959+00:00"
translation_last_reviewed: 2026-02-07
id: capacity-reconciliation
title: SoraFS Capacity Reconciliation
description: Nightly workflow for matching capacity fee ledgers to XOR transfer exports.
translator: machine-google-reviewed
---

Ճանապարհային քարտեզի կետը **SF-2c** պարտավորեցնում է, որ գանձապետարանը ապացուցի կարողությունների վճարների մատյան
համապատասխանում է XOR փոխանցումներին, որոնք կատարվում են ամեն գիշեր: Օգտագործեք
`scripts/telemetry/capacity_reconcile.py` օգնականը համեմատելու համար
`/v2/sorafs/capacity/state` ակնարկ կատարված փոխանցման խմբաքանակի և
թողարկել Prometheus տեքստային ֆայլի չափումներ Alertmanager-ի համար:

## Նախադրյալներ
- Տարողունակության վիճակի լուսանկար (`fee_ledger` գրառումներ) արտահանված Torii-ից:
- Ledger արտահանում նույն պատուհանի համար (JSON կամ NDJSON `provider_id_hex`-ով,
  `kind` = վճարում/տուգանք, և `amount_nano`):
- Ճանապարհ դեպի node_exporter տեքստային ֆայլերի հավաքիչ, եթե ուզում եք ծանուցումներ:

## Runbook
```bash
python3 scripts/telemetry/capacity_reconcile.py \
  --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
  --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
  --label nightly-capacity \
  --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
  --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
```

- Ելքի կոդերը՝ `0` մաքուր համընկնման դեպքում, `1`, երբ հաշվարկները/տուգանքները բացակայում են
  կամ ավել վճարված, `2` անվավեր մուտքերի դեպքում:
- Կցեք JSON ամփոփագիրը + հեշերը գանձապետական փաթեթին
  `docs/examples/sorafs_capacity_marketplace_validation/`.
- Երբ `.prom` ֆայլը վայրէջք է կատարում տեքստային ֆայլերի հավաքիչում, ահազանգը
  `SoraFSCapacityReconciliationMismatch` (տես
  `dashboards/alerts/sorafs_capacity_rules.yml`) կրակում է, երբ բացակայում է,
  հայտնաբերվում են գերավճար կամ մատակարարի անսպասելի փոխանցումներ:

## Արդյունքներ
- Մեկ մատակարարի կարգավիճակներ՝ հաշվարկների և տույժերի տարբերությամբ:
- Որպես չափիչներ արտահանվող ընդհանուր գումարներ.
  - `sorafs_capacity_reconciliation_missing_total{kind}`
  - `sorafs_capacity_reconciliation_overpaid_total{kind}`
  - `sorafs_capacity_reconciliation_unexpected_transfers_total`
  - `sorafs_capacity_reconciliation_expected_nano{kind}`
  - `sorafs_capacity_reconciliation_actual_nano{kind}`

## Ակնկալվող միջակայքեր և հանդուրժողականություններ
- Հաշտեցումը ճշգրիտ է. ակնկալվող ընդդեմ իրական վճարումների/տուգանքների նանոները պետք է համընկնեն զրոյական հանդուրժողականության հետ: Ցանկացած ոչ զրոյական տարբերություն պետք է էջի օպերատորները:
- CI-ն ամրացնում է 30-օրյա ներծծող նյութը հզորության վճարի մատյանում (փորձարկման `capacity_fee_ledger_30_day_soak_deterministic`) `71db9e1a17f66920cd4fe6d2bb6a1b008f9cfe1acbb3149d727fa9c80eee80d1`-ին: Թարմացրեք բովանդակությունը միայն այն դեպքում, երբ գնագոյացումը կամ սառեցման իմաստը փոխվում է:
- Ներծծման պրոֆիլում (`penalty_bond_bps=0`, `strike_threshold=u32::MAX`) տույժերը մնում են զրոյական; արտադրությունը պետք է տույժեր արձակի միայն այն դեպքում, երբ օգտագործման/ժամանակի/PoR հատակները խախտվում են և հարգում է կազմաձևված սառեցումը մինչև հաջորդական շեղումները:
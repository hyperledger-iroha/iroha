---
lang: am
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

የመንገድ ካርታ ንጥል **SF-2c** የግምጃ ቤት የአቅም ክፍያ ደብተር እንዲያረጋግጥ ያዛል
በእያንዳንዱ ምሽት ከሚፈጸሙ የXOR ዝውውሮች ጋር ይዛመዳል። የሚለውን ተጠቀም
ለማነፃፀር `scripts/telemetry/capacity_reconcile.py` አጋዥ
`/v1/sorafs/capacity/state` ቅጽበታዊ ገጽ እይታ ከተገደለው የዝውውር ባች እና
ለአለርት አስተዳዳሪ Prometheus የጽሑፍ ፋይል መለኪያዎችን ያወጣል።

## ቅድመ ሁኔታዎች
- የአቅም ሁኔታ ቅጽበተ ፎቶ (`fee_ledger` ግቤቶች) ከ Torii ወደ ውጭ የተላከ።
- ለተመሳሳይ መስኮት ደብተር ወደ ውጭ መላክ (JSON ወይም NDJSON ከ I18NI0000006X ጋር ፣
  `kind` = ሰፈራ/ቅጣት፣ እና `amount_nano`)።
- ማንቂያዎችን ከፈለጉ ወደ node_exporter textፋይል ሰብሳቢ የሚወስደው መንገድ።

## Runbook
```bash
python3 scripts/telemetry/capacity_reconcile.py \
  --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
  --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
  --label nightly-capacity \
  --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
  --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
```

- መውጫ ኮዶች፡- I18NI0000009X በንጹህ ግጥሚያ ላይ፣ `1` ሰፈራ/ቅጣቶች ሲቀሩ
  ወይም ከልክ በላይ የተከፈለ፣ ልክ ባልሆኑ ግብዓቶች ላይ `2`።
- የJSON ማጠቃለያ + hashesን ከግምጃ ቤት ፓኬት ጋር ያያይዙ
  `docs/examples/sorafs_capacity_marketplace_validation/`.
- የ `.prom` ፋይል በጽሑፍ ፋይል ሰብሳቢው ውስጥ ሲያርፍ ፣ ማንቂያው
  `SoraFSCapacityReconciliationMismatch` (ተመልከት
  `dashboards/alerts/sorafs_capacity_rules.yml`) በሚጠፋበት ጊዜ ያቃጥላል ፣
  ከመጠን በላይ የተከፈለ ወይም ያልተጠበቁ የአቅራቢዎች ዝውውሮች ተገኝተዋል።

## ውጤቶች
- በሰፈራ እና ለቅጣቶች ልዩነት ያላቸው የአቅራቢ ሁኔታዎች።
- ድምር እንደ መለኪያ ወደ ውጭ የተላከው፡
  - `sorafs_capacity_reconciliation_missing_total{kind}`
  - `sorafs_capacity_reconciliation_overpaid_total{kind}`
  - `sorafs_capacity_reconciliation_unexpected_transfers_total`
  - `sorafs_capacity_reconciliation_expected_nano{kind}`
  - `sorafs_capacity_reconciliation_actual_nano{kind}`

## የሚጠበቁ ክልሎች እና መቻቻል
- እርቅ ትክክለኛ ነው፡ የሚጠበቀው ከትክክለኛው የሰፈራ/የቅጣት ናኖዎች ከዜሮ መቻቻል ጋር መመሳሰል አለበት። ማንኛውም ዜሮ ያልሆነ ልዩነት የገጽ ኦፕሬተሮች አለበት።
- CI የ 30 ቀን ሶክ መፍጨት ለአቅም ክፍያ ደብተር (ሙከራ `capacity_fee_ledger_30_day_soak_deterministic`) ወደ `71db9e1a17f66920cd4fe6d2bb6a1b008f9cfe1acbb3149d727fa9c80eee80d1` ይሰካል። የዋጋ አሰጣጥ ወይም የማቀዝቀዝ ትርጉም ሲቀየር ብቻ የምግብ መፍጫውን ያድሱ።
- በሶክ ፕሮፋይል (I18NI0000023X, `strike_threshold=u32::MAX`) ቅጣቶች በዜሮ ይቆያሉ; ምርት ቅጣቶችን የሚያስተላልፈው የአጠቃቀም/የስራ ሰዓት/PoR ወለሎች ሲጣሱ ብቻ ነው እና ከተከታታይ መቆራረጦች በፊት የተዋቀረውን ማቀዝቀዣ ያክብሩ።
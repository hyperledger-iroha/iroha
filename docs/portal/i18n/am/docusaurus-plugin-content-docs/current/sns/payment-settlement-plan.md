---
id: payment-settlement-plan
lang: am
direction: ltr
source: docs/portal/docs/sns/payment-settlement-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SNS Payment & Settlement Plan
sidebar_label: Payment & settlement plan
description: Playbook for routing SNS registrar revenue, reconciling steward/treasury splits, and producing evidence bundles.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> ቀኖናዊ ምንጭ፡ [`docs/source/sns/payment_settlement_plan.md`](../../../source/sns/payment_settlement_plan.md)።

የመንገድ ካርታ ተግባር ** SN-5 — የክፍያ እና የማቋቋሚያ አገልግሎት** ቆራጥነትን ያስተዋውቃል
ለሶራ ስም አገልግሎት የክፍያ ንብርብር። እያንዳንዱ ምዝገባ፣ እድሳት ወይም ገንዘብ ተመላሽ ማድረግ
ግምጃ ቤት፣ መጋቢዎች እና አስተዳደር እንዲችሉ የተዋቀረ I18NT0000000X ጭነት መልቀቅ አለበት።
ያለ የተመን ሉሆች የፋይናንስ ፍሰቶችን እንደገና አጫውት። ይህ ገጽ ልዩነቱን ያብራራል።
ለፖርታል ታዳሚዎች.

## የገቢ ሞዴል

- የመሠረት ክፍያ (I18NI0000005X) የተገኘው ከመዝጋቢ የዋጋ ማትሪክስ ነው።  
- ግምጃ ቤት `gross_fee × 0.70` ይቀበላል ፣ መጋቢዎች የቀረውን ይቀንሳሉ
  ሪፈራል ጉርሻዎች (በ 10% የተያዙ).  
- የአማራጭ መከልከል አስተዳደር በግጭቶች ጊዜ የመጋቢ ክፍያዎችን ባለበት እንዲያቆም ያስችለዋል።  
- የሰፈራ ቅርቅቦች የ `ledger_projection` ብሎክ ከኮንክሪት ጋር ያጋልጣሉ
  `Transfer` ISIs ስለዚህ አውቶሜሽን የXOR እንቅስቃሴዎችን በቀጥታ ወደ Torii መለጠፍ ይችላል።

## አገልግሎቶች እና አውቶማቲክ

| አካል | ዓላማ | ማስረጃ |
|-------|---------|-------|
| `sns_settlementd` | ፖሊሲን፣ የምልክት ጥቅሎችን፣ ላዩን `/v1/sns/settlements` ይተገበራል። | JSON ጥቅል + ሃሽ። |
| የሰፈራ ወረፋ እና ጸሐፊ | በ`iroha_cli app sns settlement ledger` የሚነዳ ተስማሚ ወረፋ + የሂሳብ መዝገብ አስረክብ። | ጥቅል ሃሽ ↔ tx hash መግለጫ። |
| የማስታረቅ ስራ | ዕለታዊ ልዩነት + ወርሃዊ መግለጫ በ`docs/source/sns/reports/` ስር። | Markdown + JSON መፍጨት። |
| የተመላሽ ገንዘብ ዴስክ | በ`/settlements/{id}/refund` በኩል በመንግስት የጸደቀ ተመላሽ ገንዘቦች። | `RefundRecordV1` + ትኬት። |

CI ረዳቶች እነዚህን ፍሰቶች ያንፀባርቃሉ፡

```bash
# Quote & ledger projection
iroha_cli app sns settlement quote --selector makoto.sora --term-years 1 --pricing hot-tier-a

# Emit transfers for automation/pipeline
iroha_cli app sns settlement ledger --bundle artifacts/sns/settlements/2026-05/makoto.sora.json

# Produce a reconciliation statement
iroha_cli app sns settlement reconcile --period 2026-05 --out docs/source/sns/reports/settlement_202605.md
```

## ታዛቢነት እና ሪፖርት ማድረግ

- ዳሽቦርዶች፡ I18NI0000015X ለግምጃ ቤት vs
  የመጋቢ ድምር፣ የሪፈራል ክፍያዎች፣ የወረፋ ጥልቀት እና የተመላሽ ገንዘብ መዘግየት።
- ማንቂያዎች፡- I18NI0000016X ማሳያዎች በመጠባበቅ ላይ
  ዕድሜ፣ የማስታረቅ ውድቀቶች፣ እና የሂሳብ መመዝገቢያ መንሸራተት።
- መግለጫዎች፡ ዕለታዊ የምግብ መፈጨት (`settlement_YYYYMMDD.{json,md}`) ወደ ወርሃዊ ይንከባለል
  ሪፖርቶች (`settlement_YYYYMM.md`) ሁለቱም ወደ Git እና የ
  የአስተዳደር ነገር መደብር (`s3://sora-governance/sns/settlements/<period>/`).
- የአስተዳደር እሽጎች ዳሽቦርዶችን፣ የCLI ምዝግብ ማስታወሻዎችን እና ማፅደቆችን በምክር ቤት ፊት ያዘጋጃሉ።
  መውጣት ።

## የታቀዱ የፍተሻ ዝርዝር

1. የፕሮቶታይፕ ጥቅስ + የመመዝገቢያ ረዳቶች እና የዝግጅት ቅርቅብ ይያዙ።
2. I18NI0000020Xን በወረፋ + ጸሃፊ፣ በሽቦ ዳሽቦርድ እና በአካል ብቃት እንቅስቃሴ ያስጀምሩ
   የማንቂያ ሙከራዎች (`promtool test rules ...`)።
3. የተመላሽ ገንዘብ ረዳት እና ወርሃዊ መግለጫ አብነት ያቅርቡ; የመስታወት ቅርሶች ወደ ውስጥ
   `docs/portal/docs/sns/reports/`.
4. የአጋር ልምምድ (የሰፈራ ወር ሙሉ) ያካሂዱ እና ያዙት።
   የአስተዳደር ድምጽ SN-5 እንደተጠናቀቀ ምልክት ያደርጋል።

ለትክክለኛዎቹ የንድፍ ፍቺዎች ወደ ምንጭ ሰነድ ይመለሱ፣ ይክፈቱ
ጥያቄዎች, እና የወደፊት ማሻሻያዎች.
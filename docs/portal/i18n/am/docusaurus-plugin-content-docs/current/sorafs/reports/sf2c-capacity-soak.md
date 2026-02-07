---
lang: am
direction: ltr
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SF-2c አቅም Accrual Soak ሪፖርት

ቀን፡- 2026-03-21

## ወሰን

ይህ ሪፖርት የሚወስነውን SoraFS የአቅም ማጠራቀምን እና የክፍያ መጠመድን ይመዘግባል
በSF-2c የመንገድ ካርታ ትራክ ስር የተጠየቁ ሙከራዎች።

- ** የ30-ቀን ባለብዙ አቅራቢ ማጥለቅለቅ፡** የሚለማመዱ
  `capacity_fee_ledger_30_day_soak_deterministic` ውስጥ
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`.
  ማሰሪያው አምስት አቅራቢዎችን ያፋጥናል፣ 30 የሰፈራ መስኮቶችን ይሸፍናል እና
  የሂሳብ ደብተር ድምር በራሱ ከተሰላ ማጣቀሻ ጋር የሚዛመድ መሆኑን ያረጋግጣል
  ትንበያ. ፈተናው Blake3 ዲጀስት (`capacity_soak_digest=...`) ያወጣል።
  CI ቀኖናዊውን ቅጽበታዊ ገጽ እይታ ሊይዝ እና ሊለያይ ይችላል።
- ** ከአቅርቦት በታች ቅጣቶች: ** በ ተፈጻሚነት
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (ተመሳሳይ ፋይል). ፈተናው የመምታት ገደቦችን፣ ማቀዝቀዝን፣ የዋስትና ቅነሳዎችን ያረጋግጣል፣
  እና የመመዝገቢያ ቆጣሪዎች ቆራጥነት ይቆያሉ.

## ማስፈጸሚያ

የሶክ ማረጋገጫዎችን በአገር ውስጥ ያሂዱ፦

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

ፈተናዎቹ በመደበኛ ላፕቶፕ ከአንድ ሰከንድ በታች የሚጠናቀቁ እና ቁ
ውጫዊ እቃዎች.

# ታዛቢነት

Torii አሁን የአቅራቢ ክሬዲት ቅጽበተ-ፎቶዎችን ከክፍያ ደብተሮች እና ዳሽቦርዶች ጋር አጋልጧል
በዝቅተኛ ሒሳቦች እና የቅጣት ምልክቶች ላይ ሊገባ ይችላል-

- አርፈው፡ `GET /v1/sorafs/capacity/state` የ `credit_ledger[*]` ግቤቶችን ይመልሳል
  በሶክ ሙከራ ውስጥ የተረጋገጡትን የሂሳብ መመዝገቢያ መስኮችን ያንጸባርቁ። ተመልከት
  `crates/iroha_torii/src/sorafs/registry.rs`.
- Grafana ማስመጣት፡ `dashboards/grafana/sorafs_capacity_penalties.json` ያሴራል።
  ወደ ውጭ የተላኩ የስራ ማቆም አድማዎች፣ የቅጣት ድምር እና በጥሪ ጊዜ የታሰሩ ዋስትናዎች
  ሰራተኞቹ የሶክ መነሻ መስመሮችን ከቀጥታ አከባቢዎች ጋር ማወዳደር ይችላሉ።

#ክትትል

- የሶክ ፈተናን (የጭስ ደረጃ) ለመድገም ሳምንታዊ በር በCI ውስጥ ይሰራል።
- የ I18NT0000001X ሰሌዳን በTorii የመቧጨር ዒላማዎች አንዴ ምርት ቴሌሜትሪ ያራዝሙ
  ኤክስፖርት በቀጥታ ይሄዳል።
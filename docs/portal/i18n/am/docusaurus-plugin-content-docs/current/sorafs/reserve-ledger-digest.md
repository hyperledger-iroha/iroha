---
lang: am
direction: ltr
source: docs/portal/docs/sorafs/reserve-ledger-digest.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Reserve Ledger Digest & Dashboards
description: How to turn `sorafs reserve ledger` output into telemetry, dashboards, and alerts for the Reserve+Rent policy.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

የመጠባበቂያ+ኪራይ ፖሊሲ (የመንገድ ካርታ ንጥል **SFM‑6**) አሁን `sorafs reserve` ይልካል።
የ CLI ረዳቶች እና የ `scripts/telemetry/reserve_ledger_digest.py` ተርጓሚ እንዲሁ
የግምጃ ቤት ስራዎች ወሳኙን የኪራይ/የመጠባበቂያ ዝውውሮችን ሊያወጡ ይችላሉ። ይህ ገጽ መስተዋቶች
የስራ ፍሰት በ `docs/source/sorafs_reserve_rent_plan.md` ውስጥ ተገልጿል እና ያብራራል
አዲሱን የዝውውር ምግብ እንዴት ወደ Grafana + Alertmanager ስለዚህ ኢኮኖሚክስ እና
የአስተዳደር ገምጋሚዎች እያንዳንዱን የሂሳብ አከፋፈል ዑደት ኦዲት ማድረግ ይችላሉ።

## ከጫፍ እስከ ጫፍ የስራ ሂደት

1. ** ጥቅስ + የሂሳብ መዝገብ ትንበያ **
   ```bash
   sorafs reserve quote \
     --storage-class hot \
     --tier tier-a \
     --duration monthly \
     --gib 250 \
     --quote-out artifacts/sorafs_reserve/quotes/provider-alpha-apr.json

  sorafs reserve ledger \
    --quote artifacts/sorafs_reserve/quotes/provider-alpha-apr.json \
    --provider-account i105... \
    --treasury-account i105... \
    --reserve-account i105... \
    --asset-definition xor#sora \
    --json-out artifacts/sorafs_reserve/ledger/provider-alpha-apr.json
   ```
   የሂሳብ ደብተር አጋዥ የ`ledger_projection` ብሎክን (የኪራይ ክፍያ ፣ የተጠራቀመ) አያይዞ።
   እጥረት፣ ከፍተኛ-ላይ ዴልታ፣ ቡሊያንስ በጽሑፍ የሚጻፍ) እና Norito `Transfer`
   XORን በግምጃ ቤት እና በመጠባበቂያ ሂሳቦች መካከል ለማንቀሳቀስ አይኤስአይኤስ ያስፈልጋል።

2. ** የምግብ መፍጫውን + I18NT0000000X/NDJSON ውጽዓቶችን ይፍጠሩ **
   ```bash
   python3 scripts/telemetry/reserve_ledger_digest.py \
     --ledger artifacts/sorafs_reserve/ledger/provider-alpha-apr.json \
     --label provider-alpha-apr \
     --out-json artifacts/sorafs_reserve/ledger/provider-alpha-apr.digest.json \
     --out-md docs/source/sorafs/reports/provider-alpha-apr-ledger.md \
     --ndjson-out artifacts/sorafs_reserve/ledger/provider-alpha-apr.ndjson \
     --out-prom artifacts/sorafs_reserve/ledger/provider-alpha-apr.prom
   ```
   የምግብ መፈጨት ረዳቱ የማይክሮ-XOR ድምርን ወደ XOR መደበኛ ያደርገዋል፣ ይህም መሆኑን ይመዘግባል
   ትንበያ ከስር ጽሑፍ ጋር ይገናኛል፣ እና **የማስተላለፊያ ምግብ** መለኪያዎችን ያወጣል።
   `sorafs_reserve_ledger_transfer_xor` እና
   `sorafs_reserve_ledger_instruction_total`. ብዙ ደብተሮች ሲያስፈልጉ
   የተቀነባበረ (ለምሳሌ፣ የአቅራቢዎች ስብስብ)፣ `--ledger`/`--label` ጥንዶችን ይድገሙ እና
   ረዳቱ እያንዳንዱን መፈጨት የያዘ አንድ ነጠላ NDJSON/Prometheus ፋይል ይጽፋል።
   ዳሽቦርዶች ሙሉውን ዑደቱን ያለ ማጣበቂያ ያስገባሉ። የ `--out-prom`
   ፋይሉ የሚያነጣጥረው መስቀለኛ-ላኪ የጽሑፍ ፋይል ሰብሳቢ - የ`.prom` ፋይልን ወደ ውስጥ ይጥሉት
   ላኪው የታየበት ማውጫ ወይም ወደ ቴሌሜትሪ ባልዲ ይስቀሉት
   በመጠባበቂያ ዳሽቦርድ ሥራ ተበላ - `--ndjson-out` ሲመግብ
   ጭነት ወደ የውሂብ ቧንቧዎች.

3. **ቅርሶችን + ማስረጃዎችን አትም**
   - የምግብ መፍጫዎችን በ I18NI0000021X እና አገናኝ ስር ያከማቹ
     ከሳምንታዊ የኢኮኖሚክስ ሪፖርትዎ የማርክዳውን ማጠቃለያ።
   - የJSON ዲጀስትን ከተቃጠለ ኪራይ ጋር አያይዘው (ስለዚህ ኦዲተሮች እንደገና መጫወት ይችላሉ።
     ሒሳብ) እና በአስተዳደር ማስረጃ ፓኬት ውስጥ ያለውን ቼክ ድምር ያካትቱ።
   - የምግብ መፍጫ መሣሪያው መሙላቱን ወይም የጽሑፍ ጥሰትን ካሳየ ማንቂያውን ያጣቅሱ
     መታወቂያዎች (`SoraFSReserveLedgerTopUpRequired`፣
     `SoraFSReserveLedgerUnderwritingBreach`) እና የትኛዎቹ የዝውውር አይኤስአይኤስ እንደነበሩ ልብ ይበሉ
     ተተግብሯል.

## መለኪያዎች → ዳሽቦርዶች → ማንቂያዎች

| ምንጭ ሜትሪክ | Grafana ፓነል | ማንቂያ / ፖሊሲ መንጠቆ | ማስታወሻ |
|-------------|
| `torii_da_rent_base_micro_total`፣ `torii_da_protocol_reserve_micro_total`፣ `torii_da_provider_reward_micro_total` | "DA ኪራይ ስርጭት (XOR / ሰዓት)" በ I18NI0000027X | ሳምንታዊውን የግምጃ ቤት ምግብ ይመግቡ; በመጠባበቂያ ፍሰት ውስጥ ያሉ ስፒሎች ወደ `SoraFSCapacityPressure` (`dashboards/alerts/sorafs_capacity_rules.yml`) ይሰራጫሉ። |
| `torii_da_rent_gib_months_total` | "የአቅም አጠቃቀም (GiB-ወሮች)" (ተመሳሳይ ዳሽቦርድ) | የክፍያ መጠየቂያ ማከማቻው ከXOR ዝውውሮች ጋር እንደሚዛመድ ለማረጋገጥ ከመመዝገቢያ ደብተር ጋር ያጣምሩ። |
| `sorafs_reserve_ledger_rent_due_xor`፣ `sorafs_reserve_ledger_reserve_shortfall_xor`፣ `sorafs_reserve_ledger_top_up_shortfall_xor` | "Snapshot (XOR)" + የሁኔታ ካርዶች በ I18NI0000034X | `SoraFSReserveLedgerTopUpRequired` ሲቃጠል `requires_top_up=1`; `SoraFSReserveLedgerUnderwritingBreach` ሲቃጠል I18NI0000038X. |
| `sorafs_reserve_ledger_transfer_xor`, `sorafs_reserve_ledger_instruction_total` | "በአይነት ይሸጋገራል"፣ "የቅርብ ጊዜ የዝውውር ልዩነት"፣ እና የሽፋን ካርዶች በ`dashboards/grafana/sorafs_reserve_economics.json` | `SoraFSReserveLedgerInstructionMissing`, `SoraFSReserveLedgerRentTransferMissing` እና `SoraFSReserveLedgerTopUpTransferMissing` የዝውውር ምግብ ሲቀር ወይም ዜሮ ሲቀንስ ያስጠነቅቃሉ በተመሳሳይ ጊዜ የሽፋን ካርዶች ወደ 0% ይወርዳሉ. |

የኪራይ ዑደት ሲጠናቀቅ፣ የPrometheus/NDJSON ቅጽበተ-ፎቶዎችን ያድሱ፣ ያረጋግጡ
የ Grafana ፓነሎች አዲሱን `label` እንዲወስዱ እና ቅጽበታዊ ገጽ እይታዎችን ያያይዙ +
ለኪራይ አስተዳደር ፓኬት የማንቂያ አስተዳዳሪ መታወቂያዎች። ይህ የ CLI ትንበያን ያረጋግጣል ፣
ቴሌሜትሪ፣ እና የአስተዳደር ቅርሶች ሁሉም ከአንድ **ተመሳሳይ** የማስተላለፊያ ምግብ እና
የፍኖተ ካርታውን ኢኮኖሚክስ ዳሽቦርዶች ከመጠባበቂያ+ኪራይ ጋር እንዲጣጣሙ ያደርጋል
አውቶሜሽን. የሽፋን ካርዶቹ 100% (ወይም 1.0) እና አዲሱን ማንቂያዎች ማንበብ አለባቸው
አንድ ጊዜ ኪራይ ማጽዳት እና የተጨማሪ ክፍያ ዝውውሮች በመመገቢያው ውስጥ ካሉ።
---
lang: am
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

::: ማስታወሻ ቀኖናዊ ምንጭ
::

# SoraFS Deal Engine

የ SF-8 የመንገድ ካርታ ትራክ የ SoraFS ስምምነት ሞተርን ያስተዋውቃል ፣
መካከል ማከማቻ እና ሰርስሮ ስምምነቶች deterministic የሂሳብ
ደንበኞች እና አቅራቢዎች. ስምምነቶች ከ Norito ክፍያ ጭነቶች ጋር ተገልጸዋል።
በ`crates/sorafs_manifest/src/deal.rs` ውስጥ የተገለጸ፣ የስምምነት ውሎችን የሚሸፍን ፣ ማስያዣ
መቆለፍ፣ ፕሮባቢሊቲካል የማይክሮ ክፍያዎች እና የሰፈራ መዝገቦች።

የተካተተው I18NT0000003X ሰራተኛ (`sorafs_node::NodeHandle`) አሁን ፈጣን ያደርገዋል
ለእያንዳንዱ የመስቀለኛ መንገድ ሂደት `DealEngine` ምሳሌ። ሞተር፡-

- `DealTermsV1` በመጠቀም ስምምነቶችን ያረጋግጣል እና ይመዘግባል;
- የማባዛት አጠቃቀም ሪፖርት ሲደረግ በXOR የተመሰከረላቸው ክፍያዎችን ያከማቻል።
- ቆራጥነትን በመጠቀም ፕሮባቢሊስቲክ የማይክሮ ክፍያ መስኮቶችን ይገመግማል
  Blake3 ላይ የተመሠረተ ናሙና; እና
- ለአስተዳደሩ ተስማሚ የሆኑ የመመዝገቢያ ቅጽበታዊ ገጽ እይታዎችን እና የሰፈራ ጭነቶችን ያዘጋጃል።
  ማተም.

የክፍል ፈተናዎች ማረጋገጫን፣ የማይክሮ ክፍያ ምርጫን እና የሰፈራ ፍሰቶችን ይሸፍናሉ።
ኦፕሬተሮች ኤፒአይዎችን በልበ ሙሉነት መጠቀም ይችላሉ። ሰፈራዎች አሁን ይለቃሉ
`DealSettlementV1` የአስተዳደር ክፍያዎች ፣ በቀጥታ ወደ SF-12 በማገናኘት
የቧንቧ መስመር ማተም፣ እና የ`sorafs.node.deal_*` OpenTelemetry ተከታታይን አዘምን
(`deal_settlements_total`፣ `deal_expected_charge_nano`፣ `deal_client_debit_nano`፣
`deal_outstanding_nano`፣ `deal_bond_slash_nano`፣ `deal_publish_total`) ለTorii ዳሽቦርዶች እና SLO
ማስፈጸም። የክትትል ዕቃዎች በኦዲተር-በተጀመረው የጭረት አውቶማቲክ እና ላይ ያተኩራሉ
የስረዛ ትርጓሜዎችን ከአስተዳደር ፖሊሲ ጋር ማስተባበር።

የአጠቃቀም ቴሌሜትሪ አሁን የ`sorafs.node.micropayment_*` ሜትሪክስ ስብስብን ይመግባል።
`micropayment_charge_nano`፣ `micropayment_credit_generated_nano`፣
`micropayment_credit_applied_nano`፣ `micropayment_credit_carry_nano`፣
`micropayment_outstanding_nano`፣ እና የቲኬት ቆጣሪዎቹ
(`micropayment_tickets_processed_total`፣ `micropayment_tickets_won_total`፣
`micropayment_tickets_duplicate_total`). እነዚህ ድምር ፕሮባቢሊቲካልን ያጋልጣሉ
የሎተሪ ፍሰት ኦፕሬተሮች የማይክሮ ክፍያ ድሎችን እና የብድር ክፍያን ማዛመድ ይችላሉ።
የሰፈራ ውጤቶች ጋር.

## Torii ውህደት

Torii አቅራቢዎች አጠቃቀሙን ሪፖርት ማድረግ እና መንዳት እንዲችሉ የተወሰኑ የመጨረሻ ነጥቦችን ያጋልጣል
የህይወት ኡደትን ያለአንዳች ሽቦ ሽቦ ማስተናገድ፡-

- `POST /v2/sorafs/deal/usage` `DealUsageReport` ቴሌሜትሪ ተቀብሎ ይመለሳል።
  የሚወስኑ የሂሳብ ውጤቶች (`UsageOutcome`).
- `POST /v2/sorafs/deal/settle` የአሁኑን መስኮት ያጠናቅቃል, በዥረት ይለቀቃል
  ውጤቱ `DealSettlementRecord` ከ ቤዝ64 ኮድ `DealSettlementV1` ጋር
  ለአስተዳደር DAG ህትመት ዝግጁ.
- Torii's `/v2/events/sse` ምግብ አሁን `SorafsGatewayEvent::DealUsage` ያሰራጫል
  እያንዳንዱን የአጠቃቀም ግቤት (epoch፣ metered GiB-hours፣ ቲኬት) የሚያጠቃልሉ መዝገቦች
  ቆጣሪዎች፣ የሚወስኑ ክፍያዎች)፣ `SorafsGatewayEvent::DealSettlement`
  ቀኖናዊ የሰፈራ መዝገብ ቅጽበታዊ ገጽ እይታ እና ን ያካተቱ መዝገቦች
  BLAKE3 መፍጨት/መጠን/base64 የዲስክ አስተዳደር ቅርስ፣ እና
  የPDP/PoTR ገደቦች ሲሆኑ የ`SorafsGatewayEvent::ProofHealth` ማንቂያዎች
  አልፏል (አቅራቢ፣ መስኮት፣ አድማ/የማቀዝቀዝ ሁኔታ፣ የቅጣት መጠን)። ሸማቾች ይችላሉ።
  ለአዲስ ቴሌሜትሪ፣ ሰፈራዎች ወይም የጤና ማረጋገጫ ማንቂያዎች ያለ ድምጽ ምላሽ ለመስጠት በአቅራቢ ያጣሩ።

ሁለቱም የመጨረሻ ነጥቦች በ SoraFS ኮታ ማዕቀፍ ውስጥ በአዲሱ በኩል ይሳተፋሉ
`torii.sorafs.quota.deal_telemetry` መስኮት ኦፕሬተሮችን እንዲያስተካክሉ ያስችላቸዋል
የሚፈቀደው የማስረከቢያ መጠን በአንድ ማሰማራት።
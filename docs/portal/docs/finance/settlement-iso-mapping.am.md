---
lang: am
direction: ltr
source: docs/portal/docs/finance/settlement-iso-mapping.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 54bf09c63cdc2720fe1968c90436344dc794ebc09313b41a050bdf548ae5809f
source_last_modified: "2026-01-22T14:35:36.859616+00:00"
translation_last_reviewed: 2026-02-07
id: settlement-iso-mapping
title: Settlement ↔ ISO 20022 Field Mapping
sidebar_label: Settlement ↔ ISO 20022
description: Canonical mapping between Iroha settlement flows and the ISO 20022 bridge.
translator: machine-google-reviewed
---

::: ማስታወሻ ቀኖናዊ ምንጭ
::

## የሰፈራ ↔ ISO 20022 የመስክ ካርታ

ይህ ማስታወሻ በIroha የሰፈራ መመሪያዎች መካከል ያለውን ቀኖናዊ ካርታ ይይዛል
(`DvpIsi`፣ `PvpIsi`፣ repo ዋስትና ፍሰቶች) እና የተተገበሩ የ ISO 20022 መልዕክቶች
በድልድዩ. ውስጥ የተተገበረውን የመልእክት ማጭበርበር ያንፀባርቃል
`crates/ivm/src/iso20022.rs` እና ሲያመርት እንደ ማጣቀሻ ሆኖ ያገለግላል
የ Norito ጭነት ጭነቶችን ማረጋገጥ።

### የማጣቀሻ ውሂብ ፖሊሲ (ለዪዎች እና ማረጋገጫ)

ይህ መመሪያ የመለያ ምርጫዎችን፣ የማረጋገጫ ደንቦችን እና የማጣቀሻ-ውሂብን ያጠቃልላል
የ Norito ↔ ISO 20022 ድልድይ መልዕክቶችን ከማስተላለፉ በፊት መተግበር ያለባቸው ግዴታዎች።

**በአይኤስኦ መልእክት ውስጥ መልህቅ ነጥቦች፡**
- **የመሳሪያ መለያዎች** → `delivery_leg.asset_definition_id` ↔ `SctiesLeg/FinInstrmId`
  (ወይም ተመጣጣኝ የመሳሪያ መስክ).
- **ፓርቲዎች/ወኪሎች** → `DlvrgSttlmPties/Pty` እና `RcvgSttlmPties/Pty` ለ`sese.*`፣
  ወይም በ I18NI0000060X ውስጥ የወኪሉ አወቃቀሮች።
- ** መለያዎች ** → `…/Acct` ንጥረ ነገሮች ለመያዣ/ገንዘብ ሒሳቦች; በራሪ ወረቀቱን ያንጸባርቁ
  `AccountId` በ I18NI0000063X።
- **የባለቤትነት መለያዎች** → `…/OthrId` ከ `Tp/Prtry` እና የተንጸባረቀበት
  `SupplementaryData`. ቁጥጥር የሚደረግባቸው መለያዎችን በባለቤትነት በጭራሽ አይተኩ።

#### የመለያ ምርጫ በመልዕክት ቤተሰብ

##### `sese.023` / `.024` / `.025` (የደህንነት ማረጋገጫ)

- ** መሳሪያ (`FinInstrmId`)**
  - ተመራጭ፡ **ISIN** በ`…/ISIN` ስር። ለሲኤስዲ/T2S ቀኖናዊ መለያ ነው።[^ anna]
  - ውድቀት;
    - ** CUSIP ** ወይም ሌላ NSIN በ `…/OthrId/Id` ከ `Tp/Cd` ጋር ከ ISO ውጫዊ ስብስብ ጋር
      የኮድ ዝርዝር (ለምሳሌ `CUSP`); በታዘዘ ጊዜ ሰጪውን በI18NI0000075X ያካትቱ።[^iso_mdr]
    - **Norito የንብረት መታወቂያ** እንደ ባለቤትነት፡ `…/OthrId/Id`፣ `Tp/Prtry="NORITO_ASSET_ID"`፣ እና
      በ I18NI0000078X ውስጥ ተመሳሳይ እሴት ይመዝግቡ።
  - አማራጭ ገላጭ መግለጫዎች፡- **CFI** (`ClssfctnTp`) እና **FISN** ለማቃለል የሚደገፉበት
    እርቅ።[^iso_cfi][^iso_fisn]
- ** ፓርቲዎች (`DlvrgSttlmPties`፣ `RcvgSttlmPties`)**
  - ተመራጭ፡ ** BIC** (`AnyBIC/BICFI`፣ ISO 9362)።[^swift_bic]
  - መውደቅ፡ ** LEI *** የመልእክቱ እትም የተወሰነ የLEI መስክን የሚያጋልጥበት፤ ከሆነ
    የሌሉ፣ የባለቤትነት መታወቂያዎችን ከ `Prtry` መለያዎች ጋር ይያዙ እና BIC በሜታዳታ ውስጥ ያካትቱ።[^iso_cr]
- ** የሰፈራ ቦታ / ቦታ ** → ** MIC ** ለቦታው እና ** BIC** ለሲኤስዲ።[^iso_mic]

##### `colr.010` / `.011` / `.012` እና `colr.007` (መያዣ አስተዳደር)

- ልክ እንደ `sese.*` (ISIN ተመራጭ) ተመሳሳይ የመሳሪያ ደንቦችን ይከተሉ።
- ፓርቲዎች በነባሪነት ** BIC *** ይጠቀማሉ; **LEI** ዕቅዱ በሚያጋልጥበት ቦታ ተቀባይነት አለው።[^swift_bic]
- የገንዘብ መጠኖች **ISO 4217** የመገበያያ ኮዶች ከትክክለኛ ትናንሽ ክፍሎች ጋር መጠቀም አለባቸው።[^iso_4217]

##### `pacs.009` / I18NI0000090X (PvP የገንዘብ ድጋፍ እና መግለጫዎች)

- ** ወኪሎች (I18NI0000091X፣ `InstdAgt`፣ ተበዳሪ/አበዳሪ ወኪሎች)** → **BIC** ከአማራጭ ጋር
  LEI ከተፈቀደ።[^swift_bic]
- ** መለያዎች ***
  - ኢንተርባንክ፡ በ ** BIC** እና በውስጥ መለያ ማጣቀሻዎች መለየት።
  - ከደንበኛ ጋር የሚጋጩ መግለጫዎች (`camt.054`): ሲገኙ ** IBAN ** ያካትቱ እና ያረጋግጡ
    (ርዝመት፣ የሀገር ህጎች፣ mod-97 checksum)።[^swift_iban]
- ** ምንዛሪ** → ** ISO 4217** ባለ 3-ፊደል ኮድ፣ የአነስተኛ ክፍል ማዞሪያን ያክብሩ።[^iso_4217]
- **Torii ማስመጣት** → የPvP የገንዘብ ድጋፍ እግሮችን በI18NI0000094X በኩል ያስገቡ። ድልድዩ
  `Purp=SECU` ይፈልጋል እና አሁን የማጣቀሻ ውሂብ ሲዋቀር BIC ማቋረጦችን ያስፈጽማል።

#### የማረጋገጫ ህጎች (ከመልቀቁ በፊት ይተግብሩ)

| መለያ | የማረጋገጫ ደንብ | ማስታወሻ |
|--------|-------|------|
| **አይሲን** | Regex `^[A-Z]{2}[A-Z0-9]{9}[0-9]$` እና Luhn (mod-10) ቼክ አሃዝ በ ISO 6166 Annex C | ከድልድይ ልቀት በፊት አለመቀበል; ወደላይ ማበልጸግ ይመርጣሉ።[^anna_luhn] |
| ** CUSIP *** | Regex `^[A-Z0-9]{9}$` እና ሞዱል-10 ከ 2 ክብደት ጋር (የቁምፊዎች ካርታ ወደ አሃዝ) | ISIN በማይገኝበት ጊዜ ብቻ; ካርታ በ ANNA/CUSIP የእግረኛ መንገድ አንዴ ከተገኘ።[^cusip] |
| **ሊ** | Regex `^[A-Z0-9]{18}[0-9]{2}$` እና mod-97 ቼክ አሃዝ (ISO 17442) | ከመቀበልዎ በፊት ከGLEIF ዕለታዊ ዴልታ ፋይሎች ጋር ያረጋግጡ።[^gleif] |
| ** BIC *** | Regex `^[A-Z]{4}[A-Z]{2}[A-Z0-9]{2}([A-Z0-9]{3})?$` | አማራጭ የቅርንጫፍ ኮድ (የመጨረሻዎቹ ሶስት ቻርቶች)። በRA ፋይሎች ውስጥ ንቁ ሁኔታን ያረጋግጡ።[^swift_bic] |
| **MIC** | ከ ISO 10383 RA ፋይል ያቆዩ; ቦታዎች ንቁ መሆናቸውን ያረጋግጡ (ምንም I18NI0000100X ማቋረጫ ባንዲራ የለም) | ከመልቀቁ በፊት ኤምአይሲዎችን ጠቁም።[^iso_mic] |
| **ኢባን** | አገር-ተኮር ርዝመት፣ አቢይ ሆሄያት ፊደል፣ ሞድ-97 = 1 | በ SWIFT የተያዘ መዝገብ ይጠቀሙ; መዋቅራዊ ተቀባይነት የሌላቸውን IBANs ውድቅ ያድርጉ።[^swift_iban] |
| ** የባለቤትነት መለያ/የፓርቲ መታወቂያዎች** | `Max35Text` (UTF-8፣ ≤35 ቁምፊዎች) ከተስተካከለ ነጭ ቦታ ጋር | ለ`GenericAccountIdentification1.Id` እና I18NI0000103X መስኮች ተፈጻሚ ይሆናል። የድልድይ ጭነት ከISO ንድፎች ጋር እንዲስማማ ከ35 ቁምፊዎች በላይ የሆኑ ግቤቶችን ውድቅ ያድርጉ። |
| ** የተኪ መለያ መለያዎች *** | ባዶ ያልሆነ I18NI0000104X በ `…/Prxy/Id` ከአማራጭ ዓይነት ኮዶች ጋር በ`…/Prxy/Tp/{Cd,Prtry}` | ከዋናው IBAN ጎን ለጎን የተከማቸ; የPvP ሀዲዶችን ለማንፀባረቅ ተኪ መያዣዎችን (ከአማራጭ ዓይነት ኮዶች ጋር) ሲቀበሉ ማረጋገጥ አሁንም IBANs ያስፈልገዋል። |
| **CFI** | ISO 10962 taxonomy | በመጠቀም ባለ ስድስት ቁምፊ ኮድ፣ አቢይ ሆሄያት | አማራጭ ማበልጸግ; ቁምፊዎች ከመሳሪያ ክፍል ጋር እንደሚዛመዱ ያረጋግጡ።[^iso_cfi] |
| **FISN** | እስከ 35 ቁምፊዎች፣ አቢይ ሆሄያት እና የተገደበ ሥርዓተ ነጥብ | አማራጭ; መቆራረጥ/መደበኛ በ ISO 18774 መመሪያ።[^iso_fisn] |
| ** ምንዛሪ** | ISO 4217 ባለ 3-ፊደል ኮድ፣ ልኬት በአነስተኛ ክፍሎች የሚወሰን | መጠኖች ወደ የተፈቀዱ አስርዮሽዎች መዞር አለባቸው; በ Norito በኩል ያስፈጽሙ።[^ iso_4217] |

#### የእግረኛ መንገድ እና የመረጃ ጥገና ግዴታዎች

- **ISIN ↔ Norito የንብረት መታወቂያ ** እና ** CUSIP ↔ ISIN** የእግረኛ መንገዶችን ማቆየት። በየምሽቱ አዘምን ከ
  ANNA/DSB ምግቦች እና ሥሪት በCI ጥቅም ላይ የሚውሉ ቅጽበተ-ፎቶዎችን ይቆጣጠራል።[^anna_crosswalk]
- አድስ **BIC ↔ LEI** ድልድዩ እንዲችል ከGLEIF የህዝብ ግንኙነት ፋይሎች ካርታዎች
  ሲያስፈልግ ሁለቱንም መልቀቅ።[^bic_lei]
የቦታ ማረጋገጫ እንዲሆን **MIC ትርጓሜዎች** ከድልድዩ ሜታዳታ ጎን ያከማቹ
  የ RA ፋይሎች በቀኑ አጋማሽ ላይ በሚቀየሩበት ጊዜ እንኳን የሚወስነው።[^iso_mic]
- የውሂብ ማረጋገጫን (የጊዜ ማህተም + ምንጭ) በድልድይ ሜታዳታ ለኦዲት ይመዝግቡ። በጽናት
  ቅጽበተ-ፎቶ ለዪ ከተለቀቁት መመሪያዎች ጋር።
- የእያንዳንዱን የተጫነ የውሂብ ስብስብ ቅጂ ለመቀጠል `iso_bridge.reference_data.cache_dir` አዋቅር
  ከፕሮቬንሽን ሜታዳታ (ስሪት፣ ምንጭ፣ የጊዜ ማህተም፣ ቼክሰም) ጋር። ይህ ኦዲተሮችን ይፈቅዳል
  እና ኦፕሬተሮች የታሪካዊ ምግቦች ወደላይ የተፋሰሱ ቅጽበተ-ፎቶዎች ከተቀያየሩ በኋላም ቢሆን።
- የ ISO አቋራጭ ቅጽበተ-ፎቶዎች በ `iroha_core::iso_bridge::reference_data` ተጠቅመዋል
  የ `iso_bridge.reference_data` ውቅር እገዳ (ዱካዎች + የእድሳት ክፍተት)። መለኪያዎች
  `iso_reference_status`፣ `iso_reference_age_seconds`፣ `iso_reference_records`፣ እና
  `iso_reference_refresh_interval_secs` ለማስጠንቀቅ የአሂድ ጊዜ ጤናን አጋልጧል። Torii
  ድልድይ የ`pacs.008` ግቤቶችን ውድቅ ያደርጋል ወኪላቸው BICs ከተዋቀረው
  የእግረኛ መሻገሪያ፣ ተጓዳኝ `InvalidIdentifier` ስህተቶች
  ያልታወቀ።【crates/iroha_torii/src/iso20022_bridge.rs#L1078】
- IBAN እና ISO 4217 ማሰሪያዎች በተመሳሳይ ንብርብር ተፈጻሚ ይሆናሉ፡ pacs.008/pacs.009 አሁን ይፈስሳል።
  ተበዳሪ/አበዳሪ IBANs የተዋቀሩ ተለዋጭ ስሞች ሲጎድሉ ወይም ሲቀሩ `InvalidIdentifier` ስህተቶችን ያስወጣሉ።
  የመቋቋሚያ ምንዛሬ ከ`currency_assets` ጠፍቷል፣ የተበላሸ ድልድይ ይከላከላል
  ወደ ደብተር ለመድረስ መመሪያዎች. የIBAN ማረጋገጫ እንዲሁ አገርን ብቻ የሚመለከት ነው።
  ከ ISO 7064 mod-97 ማለፍ በፊት ርዝመቶች እና የቁጥር ቼክ አሃዞች መዋቅራዊ ያልሆነ
  እሴቶች ቀደም ብለው ውድቅ ይደረጋሉ።
- የ CLI ሰፈራ ረዳቶች ተመሳሳይ የጥበቃ ሀዲዶችን ይወርሳሉ፡ ማለፍ
  `--iso-reference-crosswalk <path>` ከ `--delivery-instrument-id` ጋር DvP እንዲኖረው
  የ`sese.023` ኤክስኤምኤል ቅጽበተ ፎቶን ከማውጣቱ በፊት የተረጋገጠ የመሳሪያ መታወቂያዎችን አስቀድመው ይመልከቱ።【crates/iroha_cli/src/main.rs#L3752】
- `cargo xtask iso-bridge-lint` (እና የ CI መጠቅለያው `ci/check_iso_reference_data.sh`) lint
  የእግረኛ መሻገሪያ ቅጽበተ-ፎቶዎች እና የቤት ዕቃዎች። ትዕዛዙ `--isin`፣ I18NI0000124X፣ `--mic`፣ እና ይቀበላል
  `--fixtures` ባንዲራ እና ሲሮጥ በ `fixtures/iso_bridge/` ውስጥ ወደ ናሙና የውሂብ ስብስቦች ይመለሳል
  ያለ ክርክር።【xtask/src/main.rs#L146】【ci/check_iso_reference_data.sh#L1】
- የ IVM አጋዥ አሁን እውነተኛ ISO 20022 XML ኤንቨሎፕ (head.001 + `DataPDU` + `Document`) ያስገባል።
  እና የንግድ ማመልከቻ ራስጌን በ `head.001` schema so `BizMsgIdr` በኩል ያረጋግጣል፣
  `MsgDefIdr`፣ `CreDt`፣ እና BIC/ClrSysMmbId ወኪሎች በቆራጥነት ተጠብቀዋል። XMLDSig/XAdES
  ብሎኮች ሆን ተብሎ እንደተዘለሉ ይቆያሉ። 

#### የቁጥጥር እና የገበያ-መዋቅር ግምት- **T+1 ሰፈራ**፡ የአሜሪካ/ካናዳ የፍትሃዊነት ገበያዎች በ2024 ወደ T+1 ተዛወሩ። አስተካክል Norito
  በዚሁ መሰረት የ SLA ማንቂያዎች።[^sec_t1][^csa_t1]
- ** CSDR ቅጣቶች ***: የሰፈራ ዲሲፕሊን ደንቦች የገንዘብ ቅጣቶችን ያስፈጽማሉ; Norito ያረጋግጡ
  ሜታዳታ ለማስታረቅ የቅጣት ማጣቀሻዎችን ይይዛል።[^csdr]
- **በተመሳሳይ ቀን የሰፈራ አብራሪዎች**፡ የህንድ ተቆጣጣሪ በT0/T+0 ሰፈር ውስጥ ደረጃ ላይ ነው። ጠብቅ
  አብራሪዎች እየተስፋፉ ሲሄዱ ድልድይ የቀን መቁጠሪያዎች ተዘምነዋል።[^ india_t0]
- **የማያዣ ግዢዎች/መያዣዎች**: በግዢ ጊዜ እና አማራጭ መያዣዎች ላይ የ ESMA ዝመናዎችን ይከታተሉ
  ስለዚህ ሁኔታዊ መላኪያ (`HldInd`) ከቅርብ ጊዜ መመሪያ ጋር ይስማማል።[^csdr]

[^anna]: ANNA ISIN Guidelines, December 2023. https://anna-web.org/wp-content/uploads/2024/01/ISIN-Guidelines-Version-22-Dec-2023.pdf
[^iso_mdr]: ISO 20022 external code list (CUSIP `CUSP`) and MDR Part 2. https://www.iso20022.org/milestone/22048/download
[^iso_cfi]: ISO 10962 (CFI) taxonomy. https://www.iso.org/standard/81140.html
[^iso_fisn]: ISO 18774 (FISN) format guidance. https://www.iso.org/standard/66153.html
[^swift_bic]: SWIFT business identifier code (ISO 9362) guidance. https://www.swift.com/standards/data-standards/bic-business-identifier-code
[^iso_cr]: ISO 20022 change request introducing LEI options for party identification. https://www.iso20022.org/milestone/16116/download
[^iso_mic]: ISO 10383 Market Identifier Code maintenance agency. https://www.iso20022.org/market-identifier-codes
[^iso_4217]: ISO 4217 currency and minor-units table (SIX). https://www.six-group.com/en/products-services/financial-information/market-reference-data/data-standards.html
[^swift_iban]: IBAN registry and validation rules. https://www.swift.com/swift-resource/22851/download
[^anna_luhn]: ISIN checksum algorithm (Annex C). https://www.anna-dsb.com/isin/
[^cusip]: CUSIP format and checksum rules. https://www.iso20022.org/milestone/22048/download
[^gleif]: GLEIF LEI structure and validation details. https://www.gleif.org/en/organizational-identity/introducing-the-legal-entity-identifier-lei/iso-17442-the-lei-code-structure
[^anna_crosswalk]: ISIN cross-reference (ANNA DSB) feeds for derivatives and debt instruments. https://www.anna-dsb.com/isin/
[^bic_lei]: GLEIF BIC-to-LEI relationship files. https://www.gleif.org/en/lei-data/lei-mapping/download-bic-to-lei-relationship-files
[^sec_t1]: SEC release on US T+1 transition (2023). https://www.sec.gov/newsroom/press-releases/2023-29
[^csa_t1]: CSA amendments for Canadian institutional trade matching (T+1). https://www.osc.ca/en/securities-law/instruments-rules-policies/2/24-101/csa-notice-amendments-national-instrument-24-101-institutional-trade-matching-and-settlement-and
[^csdr]: ESMA CSDR settlement discipline / penalty mechanism updates. https://www.esma.europa.eu/sites/default/files/2024-11/ESMA74-2119945925-2059_Final_Report_on_Technical_Advice_on_CSDR_Penalty_Mechanism.pdf
[^india_t0]: SEBI circular on same-day settlement pilot. https://www.reuters.com/sustainability/boards-policy-regulation/india-markets-regulator-extends-deadline-same-day-settlement-plan-brokers-2025-04-29/

### መላኪያ-በክፍያ → `sese.023`

| DvP መስክ | ISO 20022 መንገድ | ማስታወሻ |
|------------------------------------|------------|
| `settlement_id` | `TxId` | የተረጋጋ የሕይወት ዑደት መለያ |
| `delivery_leg.asset_definition_id` (ደህንነት) | `SctiesLeg/FinInstrmId` | ቀኖናዊ ለዪ (ISIN፣ CUSIP፣ …) |
| `delivery_leg.quantity` | `SctiesLeg/Qty` | የአስርዮሽ ሕብረቁምፊ; ክብር የንብረት ትክክለኛነት |
| `payment_leg.asset_definition_id` (ምንዛሬ) | `CashLeg/Ccy` | ISO የምንዛሬ ኮድ |
| `payment_leg.quantity` | `CashLeg/Amt` | የአስርዮሽ ሕብረቁምፊ; የተጠጋጋ በቁጥር ዝርዝር |
| `delivery_leg.from` (ሻጭ / ማድረስ ፓርቲ) | `DlvrgSttlmPties/Pty/Bic` | BIC የማድረስ ተሳታፊ *(የመለያ ቀኖናዊ መታወቂያ በአሁኑ ጊዜ በሜታዳታ ወደ ውጭ ተልኳል)* |
| `delivery_leg.from` መለያ መለያ | `DlvrgSttlmPties/Acct` | ነፃ-ቅፅ; Norito ሜታዳታ ትክክለኛውን የመለያ መታወቂያ ይይዛል |
| `delivery_leg.to` (ገዢ / የሚቀበለው ፓርቲ) | `RcvgSttlmPties/Pty/Bic` | BIC የመቀበል ተሳታፊ |
| `delivery_leg.to` መለያ መለያ | `RcvgSttlmPties/Acct` | ነፃ-ቅፅ; ግጥሚያዎች የመለያ መታወቂያ መቀበያ |
| `plan.order` | `Plan/ExecutionOrder` | Enum: `DELIVERY_THEN_PAYMENT` ወይም `PAYMENT_THEN_DELIVERY` |
| `plan.atomicity` | `Plan/Atomicity` | Enum: `ALL_OR_NOTHING`, `COMMIT_FIRST_LEG`, `COMMIT_SECOND_LEG` |
| ** የመልእክት ዓላማ** | `SttlmTpAndAddtlParams/SctiesMvmntTp` | `DELI` (ማድረስ) ወይም `RECE` (ተቀበል); አስገቢው አካል የትኛውን እግር እንደሚፈጽም መስተዋት። |
|                                                        | `SttlmTpAndAddtlParams/Pmt` | `APMT` (በክፍያ ላይ) ወይም `FREE` (ከክፍያ ነፃ)። |
| `delivery_leg.metadata`, `payment_leg.metadata` | `SctiesLeg/Metadata`, `CashLeg/Metadata` | አማራጭ I18NT0000008X JSON በ UTF-8 ኮድ የተቀመጠ |

> ** የሰፈራ ብቃቶች *** - ድልድዩ መስተዋቶች የገበያ ሁኔታን በመቅዳት የመቋቋሚያ ሁኔታ ኮዶችን (`SttlmTxCond`) ፣ ከፊል የሰፈራ አመልካቾችን (`PrtlSttlmInd`) እና ሌሎች ከ Norito ሜታዳታ ወደ I18NI05X70000001 ሲቀርቡ መድረሻው ሲኤስዲ እሴቶቹን እንዲያውቅ በ ISO ውጫዊ ኮድ ዝርዝሮች ውስጥ የታተሙትን ቁጥሮች ያስፈጽሙ።

### ክፍያ - ከክፍያ ጋር ሲነጻጸር የገንዘብ ድጋፍ → `pacs.009`

የPvP መመሪያን የሚደግፉ በጥሬ ገንዘብ የተቀመጡ እግሮች እንደ FI-to-FI ክሬዲት ይሰጣሉ
ያስተላልፋል. ድልድዩ የታችኛው ተፋሰስ ስርዓቶች እንዲገነዘቡ እነዚህን ክፍያዎች ያስረዳል።
የዋስትና ክፍያን ይደግፋሉ።

| PvP የገንዘብ ድጋፍ መስክ | ISO 20022 መንገድ | ማስታወሻ |
---------------------------------------------------------------------------|
| `primary_leg.quantity` / {መጠን፣ ምንዛሬ} | `IntrBkSttlmAmt` + `IntrBkSttlmCcy` | መጠን/ምንዛሪ ከአስጀማሪው የተከፈለ። |
| የተቃዋሚ ፓርቲ ወኪል መለያዎች | `InstgAgt`, `InstdAgt` | BIC/LEI ወኪሎችን የመላክ እና የመቀበል። |
| የሰፈራ አላማ | `CdtTrfTxInf/PmtTpInf/CtgyPurp/Cd` | ከደህንነቶች ጋር ለተያያዘ PvP የገንዘብ ድጋፍ ወደ `SECU` ያቀናብሩ። |
| Norito ሜታዳታ (የመለያ መታወቂያዎች፣ FX ውሂብ) | `CdtTrfTxInf/SplmtryData` | ሙሉ AccountIdን፣ FX የጊዜ ማህተሞችን፣ የማስፈጸሚያ ዕቅድ ፍንጮችን ይይዛል። |
| መመሪያ መለያ / የህይወት ዑደት ማገናኘት | `CdtTrfTxInf/PmtId/InstrId`, `CdtTrfTxInf/RmtInf` | ከ Norito `settlement_id` ጋር ይዛመዳል ስለዚህ የጥሬ ገንዘብ እግር ከሴኩሪቲዎች ጎን ጋር ይታረቃል። |

የጃቫስክሪፕት ኤስዲኬ አይኤስኦ ድልድይ ከዚህ መስፈርት ጋር በነባሪነት ይስማማል።
`pacs.009` ምድብ ዓላማ ወደ `SECU`; ደዋዮች በሌላ ሊሽሩት ይችላሉ።
ደህንነቶች ያልሆኑ የብድር ዝውውሮችን በሚያወጡበት ጊዜ የሚሰራ የ ISO ኮድ፣ ግን ልክ ያልሆነ
ዋጋዎች ፊት ለፊት ውድቅ ናቸው.

መሠረተ ልማት ግልጽ የሆነ የዋስትና ማረጋገጫ የሚያስፈልገው ከሆነ ድልድዩ
`sese.025` ማውጣቱን ቀጥሏል፣ ነገር ግን ማረጋገጫው የደህንነትን እግር ያንፀባርቃል
ሁኔታ (ለምሳሌ `ConfSts = ACCP`) ከ PvP "ዓላማ" ይልቅ.

### የክፍያ-ከክፍያ ማረጋገጫ → `sese.025`

| PvP መስክ | ISO 20022 መንገድ | ማስታወሻ |
----------------------------------------|
| `settlement_id` | `TxId` | የተረጋጋ የሕይወት ዑደት መለያ |
| `primary_leg.asset_definition_id` | `SttlmCcy` | የመጀመሪያ እግር ምንዛሪ ኮድ |
| `primary_leg.quantity` | `SttlmAmt` | በአስጀማሪው የቀረበው መጠን |
| `counter_leg.asset_definition_id` | `AddtlInf` (JSON ጭነት) | ተጨማሪ መረጃ ውስጥ የተካተተ የቆጣሪ ምንዛሪ ኮድ |
| `counter_leg.quantity` | `SttlmQty` | የቆጣሪ መጠን |
| `plan.order` | `Plan/ExecutionOrder` | እንደ DvP | ተመሳሳይ enum ተቀናብሯል።
| `plan.atomicity` | `Plan/Atomicity` | እንደ DvP | ተመሳሳይ enum ተቀናብሯል።
| `plan.atomicity` ሁኔታ (`ConfSts`) | `ConfSts` | `ACCP` ሲዛመድ; ድልድይ ውድቅ ላይ የብልሽት ኮዶችን ያወጣል |
| የተቃዋሚ መለያዎች | `AddtlInf` JSON | የአሁኑ ድልድይ ተከታታይ ሙሉ AccountId/BIC በሜታዳታ |

### Repo የዋስትና ምትክ → `colr.007`

| Repo መስክ / አውድ | ISO 20022 መንገድ | ማስታወሻ |
|-----------------------------------------------------------|
| `agreement_id` (`RepoIsi` / `ReverseRepoIsi`) | `OblgtnId` | Repo ውል መለያ |
| የዋስትና ምትክ Tx መለያ | `TxId` | በየመተኪያ የመነጨ |
| ኦሪጅናል የዋስትና መጠን | `Substitution/OriginalAmt` | ከመተካት በፊት ቃል የተገቡ ግጥሚያዎች |
| ኦሪጅናል የመያዣ ገንዘብ | `Substitution/OriginalCcy` | የምንዛሬ ኮድ |
| የመያዣ ብዛትን ይተኩ | `Substitution/SubstituteAmt` | የምትክ መጠን |
| የመያዣ ገንዘብን ይተኩ | `Substitution/SubstituteCcy` | የምንዛሬ ኮድ |
| የሚሰራበት ቀን (የአስተዳደር ህዳግ የጊዜ ሰሌዳ) | `Substitution/EffectiveDt` | ISO ቀን (ዓዓዓ-ወወ-ቀን) |
| የፀጉር መቆራረጥ ምደባ | `Substitution/Type` | በአሁኑ ጊዜ `FULL` ወይም `PARTIAL` በአስተዳደር ፖሊሲ ላይ የተመሰረተ |
| የአስተዳደር ምክንያት/የፀጉር መቆረጥ ማስታወሻ | `Substitution/ReasonCd` | አማራጭ፣ የአስተዳደር ምክንያታዊነት ይሸከማል |

### የገንዘብ ድጋፍ እና መግለጫዎች

| Iroha አውድ | ISO 20022 መልእክት | የካርታ ስራ ቦታ |
|---------------------------------|---------------------------------|
| Repo የገንዘብ እግር ማቀጣጠል / ማራገፍ | `pacs.009` | `IntrBkSttlmAmt`፣ `IntrBkSttlmCcy`፣ `IntrBkSttlmDt`፣ `InstgAgt`፣ `InstdAgt` ከDvP/PvP እግሮች ተሞልቷል |
| የድህረ-ሰፈራ መግለጫዎች | `camt.054` | በ `Ntfctn/Ntry[*]` ስር የተመዘገቡ የክፍያ እግሮች እንቅስቃሴዎች; ድልድይ በ`SplmtryData` ውስጥ የሂሳብ መዝገብ/የመለያ ሜታዳታ ያስገባል።

### የአጠቃቀም ማስታወሻዎች* ሁሉም መጠኖች Norito የቁጥር ረዳቶች (`NumericSpec`) በመጠቀም ተከታታይ ናቸው
  በንብረት ፍቺዎች ላይ ሚዛን መጣጣምን ለማረጋገጥ።
* `TxId` ዋጋዎች `Max35Text` ናቸው - UTF-8 ርዝመት ≤35 ቁምፊዎችን በፊት ያስፈጽሙ
  ወደ ISO 20022 መልዕክቶች በመላክ ላይ።
* BIC 8 ወይም 11 አቢይ ሆሄያት ሆሄያት (ISO9362) መሆን አለባቸው። አለመቀበል
  Norito ሜታዳታ ይህ ቼክ ክፍያ ወይም ክፍያ ከማስተላለፉ በፊት
  ማረጋገጫዎች.
* የመለያ መለያዎች (AccountId/ChainId) ወደ ማሟያ ይላካሉ
  ዲበዳታ ስለዚህ ተሳታፊዎች መቀበል ከአካባቢያቸው ደብተሮች ጋር ማስታረቅ ይችላሉ።
* `SupplementaryData` ቀኖናዊ JSON መሆን አለበት (UTF-8፣ የተደረደሩ ቁልፎች፣ JSON-ተወላጅ
  ማምለጥ)። የኤስዲኬ ረዳቶች ፊርማዎችን፣ ቴሌሜትሪ ሃሾችን እና ISOን ያስገድዳሉ
  የመጫኛ መዛግብት በእንደገና ግንባታዎች ሁሉ የሚወሰኑ ሆነው ይቆያሉ።
* የምንዛሬ መጠኖች ISO4217 ክፍልፋይ አሃዞችን ይከተላሉ (ለምሳሌ JPY 0 አለው።
  አስርዮሽ, ዶላር 2 አለው); በዚህ መሠረት ድልድዩ Norito የቁጥር ትክክለኛነትን ያቆማል።
* የCLI ሰፈራ ረዳቶች (`iroha app settlement ... --atomicity ...`) አሁን ይለቃሉ
  Norito መመሪያው የማስፈጸሚያ እቅድ 1፡1 እስከ `Plan/ExecutionOrder` እና
  `Plan/Atomicity` ከላይ።
* የ ISO አጋዥ (`ivm::iso20022`) ከላይ የተዘረዘሩትን መስኮች ያረጋግጣል እና ውድቅ ያደርጋል
  የDvP/PvP እግሮች የቁጥር ዝርዝሮችን የሚጥሱባቸው መልዕክቶች ወይም የተጓዳኝ ተገላቢጦሽ።

### SDK ገንቢ አጋዦች

- JavaScript SDK አሁን `buildPacs008Message` / ያጋልጣል
  `buildPacs009Message` (`javascript/iroha_js/src/isoBridge.js` ይመልከቱ) ስለዚህ ደንበኛ
  አውቶሜሽን የተዋቀረውን የሰፈራ ሜታዳታን (BIC/LEI፣ IBANs፣
  የዓላማ ኮዶች፣ ተጨማሪ Norito መስኮች) ወደ መወሰኛ ፓኮች XML
  የካርታ ደንቦችን ከዚህ መመሪያ ሳይደግሙ.
- ሁለቱም ረዳቶች ግልጽ `creationDateTime` (ISO-8601 የሰዓት ሰቅ ያለው) ያስፈልጋቸዋል።
  ስለዚህ ኦፕሬተሮች በምትኩ ከሥራ ፍሰታቸው ላይ የሚወስን የጊዜ ማህተም መዘርጋት አለባቸው
  የኤስዲኬ ነባሪ ወደ ግድግዳ ሰዓት ጊዜ የመፍቀድ።
- `recipes/iso_bridge_builder.mjs` እነዚያን ረዳቶች እንዴት ሽቦ ማድረግ እንደሚቻል ያሳያል
  የአካባቢ ተለዋዋጮችን ወይም JSON ውቅር ፋይሎችን የሚያዋህድ CLI፣ ያትማል
  ኤክስኤምኤልን አመነጨ፣ እና እንደ አማራጭ ለTorii (`ISO_SUBMIT=1`) ያቀርባል፣ እንደገና ጥቅም ላይ ይውላል።
  ልክ እንደ ISO ድልድይ የምግብ አዘገጃጀት መመሪያ ተመሳሳይ የጥበቃ ጊዜ።


### ዋቢዎች

- LuxCSD/ Clearstream ISO 20022 የሰፈራ ምሳሌዎች `SttlmTpAndAddtlParams/SctiesMvmntTp` (`DELI`/`RECE`) እና `Pmt` (`APMT`/`FREE`)።<sup>[1](https://www.luxcsd.com/resource/blob/3434074/6f8add4708407a4701055be4dd04846b/c23005-eis-examples-cbf-data.pdf) </sup>
- የሰፈራ ብቃቶችን (`SttlmTxCond`፣ `PrtlSttlmInd`) የሚሸፍኑ የDCP ዝርዝሮችን ያጽዱ።<sup>[2](https://www.clearstream.com/clearstream-en/res-library/market-coverage/instruction-specifications-swift-iso-20022-dcp-mode-ceu-spain-2357008)</sup>
- የስዊፍት PMPG መመሪያ `pacs.009` ከ `CtgyPurp/Cd = SECU` ጋር ከደህንነት ጋር ለተያያዘ PvP የገንዘብ ድጋፍ።<sup>[3](https://www.swift.com/swift-resource/251897/download)</sup>
- ISO 20022 የመልእክት ፍቺ ሪፖርቶች ለዪ ርዝመት ገደቦች (BIC፣ Max35Text)።<sup>[4](https://www.iso20022.org/sites/default/files/2020-12/ISO20022_MDRPart2_ChangeOrVerifyAccountIdentification_2020_2021_v1_ForSEGReview.pdf)</sup>
- ANNA DSB መመሪያ በ ISIN ቅርጸት እና የቼክሰም ደንቦች ላይ።<sup>[5](https://www.anna-dsb.com/isin/) </sup>

### የአጠቃቀም ምክሮች

LLM መመርመር እንዲችል ሁል ጊዜ የሚመለከተውን Norito ቅንጣቢ ወይም CLI ትእዛዝ ይለጥፉ
  ትክክለኛ የመስክ ስሞች እና የቁጥር ሚዛኖች።
- የወረቀት ዱካ ለማቆየት ጥቅሶችን ይጠይቁ (`provide clause references`)
  ተገዢነት እና ኦዲተር ግምገማ.
- የመልሱን ማጠቃለያ በ `docs/source/finance/settlement_iso_mapping.md` ውስጥ ይያዙ
  (ወይም የተገናኙ አባሪዎች) ስለዚህ የወደፊት መሐንዲሶች ጥያቄውን መድገም አያስፈልጋቸውም።

## የክስተት ማዘዣ Playbooks (ISO 20022 ↔ Norito ድልድይ)

### ሁኔታ ሀ - በዋስትና መተካት (ሪፖ / ቃል ኪዳን)

** ተሳታፊዎች፡** መያዣ ሰጪ/ተቀባይ (እና/ወኪሎች)፣ ጠባቂ(ዎች)፣ CSD/T2S  
** ጊዜ: ** በአንድ የገበያ መቆራረጥ እና T2S ቀን / ማታ ዑደቶች; በተመሳሳይ የሰፈራ መስኮት ውስጥ እንዲጠናቀቁ ሁለቱን እግሮች ያቀናጁ።

#### መልእክት ኮሪዮግራፊ
1. `colr.010` የመያዣ ምትክ ጥያቄ → መያዣ ሰጪ/ተቀባይ ወይም ወኪል።  
2. `colr.011` የዋስትና ምትክ ምላሽ → መቀበል/መቀበል (አማራጭ ውድቅ ምክንያት)።  
3. `colr.012` የዋስትና ምትክ ማረጋገጫ → የመተካት ስምምነትን ያረጋግጣል።  
4. `sese.023` መመሪያዎች (ሁለት እግሮች):  
   - ዋናውን መያዣ ይመልሱ (`SctiesMvmntTp=DELI`፣ `Pmt=FREE`፣ `SctiesTxTp=COLO`)።  
   - ምትክ መያዣ (`SctiesMvmntTp=RECE`፣ `Pmt=FREE`፣ `SctiesTxTp=COLI`) ያቅርቡ።  
   ጥንዶቹን ያገናኙ (ከዚህ በታች ይመልከቱ).  
5. `sese.024` የሁኔታ ምክሮች (ተቀባይነት ያለው፣ የተዛመደ፣ በመጠባበቅ ላይ፣ አልተሳካም፣ ውድቅ የተደረገ)።  
6. `sese.025` ማረጋገጫዎች አንዴ ከተያዙ።  
7. አማራጭ የገንዘብ ዴልታ (ክፍያ/የጸጉር መቆረጥ) → `pacs.009` FI-ወደ-FI ክሬዲት ከ `CtgyPurp/Cd = SECU` ጋር ማስተላለፍ; ሁኔታ በ `pacs.002` ፣ በ `pacs.004` በኩል ይመለሳል።

#### የሚፈለጉ ምስጋናዎች/ሁኔታዎች
- የትራንስፖርት ደረጃ፡ መግቢያ መንገዶች `admi.007` ሊለቁ ወይም ከንግድ ሥራ ሂደት በፊት ውድቅ ሊያደርጉ ይችላሉ።  
- የመቋቋሚያ የሕይወት ዑደት፡ `sese.024` (የማስኬጃ ሁኔታዎች + ምክንያት ኮዶች)፣ `sese.025` (የመጨረሻ)።  
- የገንዘብ ጎን፡ `pacs.002` (`PDNG`፣ `ACSC`፣ `RJCT` ወዘተ)፣ `pacs.004` ለመመለሻ።

#### ሁኔታዊ/የማሳለፍ መስኮች
- `SctiesSttlmTxInstr/Lnkgs` (`WITH`/`BEFO`/`AFTE`) ሁለቱን መመሪያዎች ሰንሰለት ለማድረግ።  
- `SttlmParams/HldInd` መመዘኛዎች እስኪሟሉ ድረስ መያዝ; በ`sese.030` (`sese.031` ሁኔታ) መልቀቅ።  
- `SttlmParams/PrtlSttlmInd` ከፊል ሰፈራ ለመቆጣጠር (`NPAR`, `PART`, `PARC`, I18NI0000297X).  
- `SttlmParams/SttlmTxCond/Cd` ለገበያ-ተኮር ሁኔታዎች (`NOMC` ፣ ወዘተ)።  
- ሲደገፍ አማራጭ T2S ሁኔታዊ ዋስትናዎች (CoSD) ደንቦች።

#### ዋቢዎች
- SWIFT የዋስትና አስተዳደር MDR (`colr.010/011/012`)።  
- የ CSD/T2S አጠቃቀም መመሪያዎች (ለምሳሌ፡ ዲኤንቢ፣ ኢሲቢ ግንዛቤ) ለማገናኘት እና ደረጃ።  
- SMPG የሰፈራ ልምምድ፣ Clearstream DCP መመሪያዎች፣ ASX ISO ወርክሾፖች።

### ሁኔታ ለ - የ FX መስኮት መጣስ (የፒቪፒ የገንዘብ ድጋፍ ውድቀት)

** ተሳታፊዎች: *** አጋሮች እና የገንዘብ ወኪሎች ፣ የዋስትናዎች ጠባቂ ፣ CSD/T2S  
** ጊዜ: ** FX PvP መስኮቶች (CLS / ሁለትዮሽ) እና የሲኤስዲ መቁረጫዎች; የገንዘብ ማረጋገጫ በመጠባበቅ ላይ ያሉ የዋስትናዎች እግሮች እንዲቆዩ ያድርጉ።

#### መልእክት ኮሪዮግራፊ
1. `pacs.009` FI-ወደ-FI ክሬዲት ማስተላለፍ በ `CtgyPurp/Cd = SECU`; ሁኔታ በ `pacs.002`; በ `camt.056`/`camt.029` በኩል አስታውስ/ሰርዝ; ቀድሞውኑ ከተረጋጋ, `pacs.004` ይመለሳል.  
2. `sese.023` DvP መመሪያ(ዎች) ከ`HldInd=true` ጋር ስለዚህ የዋስትና እግሩ የገንዘብ ማረጋገጫ ይጠብቃል።  
3. የህይወት ዑደት `sese.024` ማሳሰቢያዎች (ተቀባይነት ያለው / የተዛመደ / በመጠባበቅ ላይ).  
4. ሁለቱም `pacs.009` እግሮች ወደ `ACSC` ከደረሱ መስኮቱ ከማለፉ በፊት → በ `sese.030` → `sese.031` (mod status) → `sese.025` (ማረጋገጫ) ይልቀቁ።  
5. የ FX መስኮት ከተጣሰ → ጥሬ ገንዘብን ይሰርዙ/ያስታውሱ (`camt.056/029` ወይም `pacs.004`) እና ዋስትናዎችን ይሰርዙ (`sese.020` + `sese.027` ፣ ወይም I18NI0000000317X + `sese.027` ፣ ወይም I18NI00000003)።

#### የሚፈለጉ ምስጋናዎች/ሁኔታዎች
- ጥሬ ገንዘብ፡ `pacs.002` (`PDNG`፣ `ACSC`፣ `RJCT`)፣ `pacs.004` ለመመለሻ።  
- ዋስትናዎች: `sese.024` (እንደ `NORE`, `ADEA` ያሉ በመጠባበቅ ላይ ያሉ / ያልተሳኩ ምክንያቶች), `sese.025`.  
- መጓጓዣ፡- `admi.007` / ጌትዌይ ከንግድ ስራ በፊት ውድቅ ያደርጋል።

#### ሁኔታዊ/የማሳለፍ መስኮች
- `SttlmParams/HldInd` + `sese.030` መለቀቅ/በስኬት/ውድቀት ላይ መሰረዝ።  
- `Lnkgs` የዋስትና መመሪያዎችን ከጥሬ ገንዘብ እግር ጋር ለማያያዝ።  
ሁኔታዊ መላኪያ የሚጠቀሙ ከሆነ - T2S CoSD ደንብ።  
- `PrtlSttlmInd` ያልተፈለጉ ክፍሎችን ለመከላከል.  
- በ`pacs.009` ላይ፣ `CtgyPurp/Cd = SECU` ከደህንነት ጋር የተያያዘ የገንዘብ ድጋፍን ያሳያል።

#### ዋቢዎች
- PMPG / CBPR+ በዋስትና ሂደቶች ውስጥ ለሚደረጉ ክፍያዎች መመሪያ።  
- የSMPG የሰፈራ ልምምዶች፣ T2S ግንዛቤዎችን በማገናኘት/በመያዝ።  
- Clearstream DCP መመሪያዎችን፣ ለጥገና መልእክቶች የECMS ሰነድ።

### pacs.004 የካርታ ማስታወሻዎችን መመለስ

- የመመለሻ ዕቃዎች አሁን `ChrgBr` መደበኛ አደረጉ (`DEBT`/`CRED`/`SHAR`/`SLEV`) እና የባለቤትነት መመለሻ ምክንያቶች በ I18NI000000341 ያለ የሸማቾች ኮድ ማጫወት ይችላሉ ። የኤክስኤምኤል ፖስታውን እንደገና መተንተን።
- በ `DataPDU` ፖስታዎች ውስጥ የ AppHdr ፊርማ እገዳዎች ወደ ውስጥ ሲገቡ ችላ ይባላሉ። ኦዲቶች ከተከተቱ የXMLDSIG መስኮች ይልቅ በሰርጥ ፕሮቬንሽን ላይ መታመን አለባቸው።

### ለድልድዩ የክዋኔ ማረጋገጫ ዝርዝር
- ከላይ ያለውን የኮሪዮግራፊ ያስፈጽሙ (መያዣ፡ `colr.010/011/012 → sese.023/024/025`፤ FX መጣስ፡ `pacs.009 (+pacs.002) → sese.023 held → release/cancel`)።  
- የ `sese.024`/`sese.025` ሁኔታዎችን እና የ `pacs.002` ውጤቶችን እንደ ጌቲንግ ሲግናሎች ይያዙ; `ACSC` ቀስቅሴዎች መለቀቅ፣ `RJCT` ያስገድዳል።  
- ሁኔታዊ ማድረሻን በ`HldInd`፣ `Lnkgs`፣ `PrtlSttlmInd`፣ `SttlmTxCond` እና በአማራጭ የCoSD ሕጎችን ኮድ ያድርጉ።  
- የውጭ መታወቂያዎችን ለማዛመድ `SupplementaryData` ይጠቀሙ (ለምሳሌ UETR ለ `pacs.009`) ሲያስፈልግ።  
- የጊዜ አቆጣጠርን በገበያ የቀን መቁጠሪያ/በመቁረጥ መለኪያ መወሰን; ከመሰረዙ ቀነ-ገደቦች በፊት `sese.030`/`camt.056` እትም ፣ አስፈላጊ ሆኖ ሲገኝ ወደነበረበት መመለስ።

### ናሙና ISO 20022 ክፍያ ጭነቶች (የተብራራ)

#### የዋስትና ምትክ ጥንድ (`sese.023`) ከመመሪያ ትስስር ጋር

```xml
<sese:Document xmlns:sese="urn:iso:std:iso:20022:tech:xsd:sese.023.001.11">
  <sese:SctiesSttlmTxInstr>
    <sese:TxId>SUBST-2025-04-001-A</sese:TxId>
    <sese:SttlmTpAndAddtlParams>
      <sese:SctiesMvmntTp>DELI</sese:SctiesMvmntTp>
      <sese:Pmt>FREE</sese:Pmt>
    </sese:SttlmTpAndAddtlParams>
    <sese:SttlmParams>
      <sese:HldInd>true</sese:HldInd>
      <sese:PrtlSttlmInd>NPAR</sese:PrtlSttlmInd>
      <sese:SttlmTxCond>
        <sese:Cd>NOMC</sese:Cd>
      </sese:SttlmTxCond>
    </sese:SttlmParams>
    <sese:Lnkgs>
      <sese:Lnkg>
        <sese:Tp>
          <sese:Cd>WITH</sese:Cd>
        </sese:Tp>
        <sese:Ref>
          <sese:Prtry>SUBST-2025-04-001-B</sese:Prtry>
        </sese:Ref>
      </sese:Lnkg>
    </sese:Lnkgs>
    <!-- Original collateral FoP back to giver -->
    <sese:FctvSttlmDt>2025-04-03</sese:FctvSttlmDt>
    <sese:SctiesMvmntDtls>
      <sese:SctiesId>
        <sese:ISIN>XS1234567890</sese:ISIN>
      </sese:SctiesId>
      <sese:Qty>
        <sese:QtyChc>
          <sese:Unit>1000</sese:Unit>
        </sese:QtyChc>
      </sese:Qty>
    </sese:SctiesMvmntDtls>
  </sese:SctiesSttlmTxInstr>
</sese:Document>
```

የተገናኘውን መመሪያ `SUBST-2025-04-001-B` (የተተኪ መያዣ መቀበል) ከ`SctiesMvmntTp=RECE`፣ `Pmt=FREE`፣ እና የ`WITH` ትስስር ወደ `SUBST-2025-04-001-A` ያቅርቡ። መተኪያው ከጸደቀ በኋላ ሁለቱንም እግሮች በተዛማጅ `sese.030` ይልቀቁ።

#### የዋስትናዎች እግር በ FX ማረጋገጫ በመጠባበቅ ላይ (`sese.023` + `sese.030`)

```xml
<sese:Document xmlns:sese="urn:iso:std:iso:20022:tech:xsd:sese.023.001.11">
  <sese:SctiesSttlmTxInstr>
    <sese:TxId>DVP-2025-05-CLS01</sese:TxId>
    <sese:SttlmTpAndAddtlParams>
      <sese:SctiesMvmntTp>DELI</sese:SctiesMvmntTp>
      <sese:Pmt>APMT</sese:Pmt>
    </sese:SttlmTpAndAddtlParams>
    <sese:SttlmParams>
      <sese:HldInd>true</sese:HldInd>
      <sese:PrtlSttlmInd>NPAR</sese:PrtlSttlmInd>
    </sese:SttlmParams>
    <sese:Lnkgs>
      <sese:Lnkg>
        <sese:Tp>
          <sese:Cd>WITH</sese:Cd>
        </sese:Tp>
        <sese:Ref>
          <sese:Prtry>PACS009-USD-CLS01</sese:Prtry>
        </sese:Ref>
      </sese:Lnkg>
    </sese:Lnkgs>
    <!-- Remaining settlement details omitted for brevity -->
  </sese:SctiesSttlmTxInstr>
</sese:Document>
```ሁለቱም `pacs.009` እግሮች `ACSC` ሲደርሱ አንዴ ይልቀቁ፡

```xml
<sese:Document xmlns:sese="urn:iso:std:iso:20022:tech:xsd:sese.030.001.04">
  <sese:SctiesSttlmCondModReq>
    <sese:ReqDtls>
      <sese:TxId>DVP-2025-05-CLS01</sese:TxId>
      <sese:ChngTp>
        <sese:Cd>RELE</sese:Cd>
      </sese:ChngTp>
    </sese:ReqDtls>
  </sese:SctiesSttlmCondModReq>
</sese:Document>
```

`sese.031` የመያዣ ልቀቱን ያረጋግጣል፣ እና አንዴ የዋስትናዎች እግር ከተያዘ በኋላ `sese.025` ይከተላል።

#### PvP የገንዘብ ድጋፍ እግር (`pacs.009` ከደህንነት ዓላማ ጋር)

```xml
<pacs:Document xmlns:pacs="urn:iso:std:iso:20022:tech:xsd:pacs.009.001.08">
  <pacs:FinInstnCdtTrf>
    <pacs:GrpHdr>
      <pacs:MsgId>PACS009-USD-CLS01</pacs:MsgId>
      <pacs:IntrBkSttlmDt>2025-05-07</pacs:IntrBkSttlmDt>
    </pacs:GrpHdr>
    <pacs:CdtTrfTxInf>
      <pacs:PmtId>
        <pacs:InstrId>DVP-2025-05-CLS01-USD</pacs:InstrId>
        <pacs:EndToEndId>SETTLEMENT-CLS01</pacs:EndToEndId>
      </pacs:PmtId>
      <pacs:PmtTpInf>
        <pacs:CtgyPurp>
          <pacs:Cd>SECU</pacs:Cd>
        </pacs:CtgyPurp>
      </pacs:PmtTpInf>
      <pacs:IntrBkSttlmAmt Ccy="USD">5000000.00</pacs:IntrBkSttlmAmt>
      <pacs:InstgAgt>
        <pacs:FinInstnId>
          <pacs:BICFI>BANKUS33XXX</pacs:BICFI>
        </pacs:FinInstnId>
      </pacs:InstgAgt>
      <pacs:InstdAgt>
        <pacs:FinInstnId>
          <pacs:BICFI>BANKGB22XXX</pacs:BICFI>
        </pacs:FinInstnId>
      </pacs:InstdAgt>
      <pacs:SplmtryData>
        <pacs:Envlp>
          <nor:NoritoBridge xmlns:nor="urn:norito:settlement">
            <nor:SettlementId>DVP-2025-05-CLS01</nor:SettlementId>
            <nor:Atomicity>ALL_OR_NOTHING</nor:Atomicity>
          </nor:NoritoBridge>
        </pacs:Envlp>
      </pacs:SplmtryData>
    </pacs:CdtTrfTxInf>
  </pacs:FinInstnCdtTrf>
</pacs:Document>
```

`pacs.002` የክፍያ ሁኔታን ይከታተላል (`ACSC` = የተረጋገጠ፣ `RJCT` = ውድቅ)። መስኮቱ ከተጣሰ በ`camt.056`/`camt.029` በኩል ያስታውሱ ወይም የተቀመጡ ገንዘቦችን ለመመለስ `pacs.004` ይላኩ።
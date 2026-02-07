---
id: settlement-iso-mapping
lang: ka
direction: ltr
source: docs/portal/docs/finance/settlement-iso-mapping.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Settlement ↔ ISO 20022 Field Mapping
sidebar_label: Settlement ↔ ISO 20022
description: Canonical mapping between Iroha settlement flows and the ISO 20022 bridge.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::შენიშვნა კანონიკური წყარო
:::

## დასახლება ↔ ISO 20022 საველე რუქა

ეს ჩანაწერი აღწერს კანონიკურ რუკებს Iroha ანგარიშსწორების ინსტრუქციებს შორის
(`DvpIsi`, `PvpIsi`, რეპო გირაოს ნაკადები) და ISO 20022 შეტყობინებები
ხიდთან. ის ასახავს შეტყობინებების ხარაჩოებს, რომლებიც განხორციელებულია
`crates/ivm/src/iso20022.rs` და ემსახურება როგორც მინიშნებას ან
Norito დატვირთვის დადასტურება.

### მითითების მონაცემთა პოლიტიკა (იდენტიფიკატორები და დადასტურება)

ეს პოლიტიკა აერთიანებს იდენტიფიკატორის პრეფერენციებს, ვალიდაციის წესებს და მითითების მონაცემებს
ვალდებულებები, რომლებიც Norito ↔ ISO 20022 ხიდმა უნდა შეასრულოს შეტყობინებების გაცემამდე.

** დამაგრების წერტილები ISO შეტყობინების შიგნით:**
- **ინსტრუმენტების იდენტიფიკატორები** → `delivery_leg.asset_definition_id` ↔ `SctiesLeg/FinInstrmId`
  (ან ექვივალენტური ინსტრუმენტის ველი).
- ** მხარეები / აგენტები ** → `DlvrgSttlmPties/Pty` და `RcvgSttlmPties/Pty` `sese.*`-ისთვის,
  ან აგენტის სტრუქტურები `pacs.009`-ში.
- **ანგარიშები** → `…/Acct` ელემენტები შენახვა/ნაღდი ანგარიშებისთვის; სარკე წიგნში
  `AccountId` `SupplementaryData`-ში.
- **საკუთრების იდენტიფიკატორები** → `…/OthrId` `Tp/Prtry`-ით და ასახული
  `SupplementaryData`. არასოდეს შეცვალოთ რეგულირებადი იდენტიფიკატორები საკუთრებით.

#### იდენტიფიკატორის უპირატესობა შეტყობინებების ოჯახის მიხედვით

##### `sese.023` / `.024` / `.025` (ფასიანი ქაღალდების ანგარიშსწორება)

- **ინსტრუმენტი (`FinInstrmId`)**
  - სასურველია: **ISIN** ქვეშ `…/ISIN`. ეს არის კანონიკური იდენტიფიკატორი CSD/T2S-ისთვის.[^anna]
  - სარეზერვო:
    - **CUSIP** ან სხვა NSIN `…/OthrId/Id` ქვეშ `Tp/Cd` კომპლექტი ISO გარედან
      კოდების სია (მაგ., `CUSP`); ჩართეთ ემიტენტი `Issr`-ში მანდატისას.[^iso_mdr]
    - **Norito აქტივის ID** როგორც საკუთრებაში: `…/OthrId/Id`, `Tp/Prtry="NORITO_ASSET_ID"` და
      ჩაწერეთ იგივე მნიშვნელობა `SupplementaryData`-ში.
  - არჩევითი აღწერები: **CFI** (`ClssfctnTp`) და **FISN**, სადაც მხარდაჭერილია გასამარტივებლად
    შერიგება.[^iso_cfi][^iso_fisn]
- **წვეულებები (`DlvrgSttlmPties`, `RcvgSttlmPties`)**
  - სასურველია: **BIC** (`AnyBIC/BICFI`, ISO 9362).[^swift_bic]
  - სარეზერვო: **LEI** სადაც შეტყობინების ვერსია ასახავს გამოყოფილ LEI ველს; თუ
    არ არსებობს, ატარეთ საკუთრების ID-ები მკაფიო `Prtry` ლეიბლებით და ჩართეთ BIC მეტამონაცემებში.[^iso_cr]
- ** დასახლების ადგილი / ადგილი ** → **MIC** ადგილისთვის და **BIC** CSD-სთვის.[^iso_mic]

##### `colr.010` / `.011` / `.012` და `colr.007` (გირაოს მართვა)

- დაიცავით ინსტრუმენტის იგივე წესები, როგორც `sese.*` (სასურველია ISIN).
- მხარეები ნაგულისხმევად იყენებენ **BIC**-ს; **LEI** მისაღებია იქ, სადაც სქემა ამჟღავნებს მას.[^swift_bic]
- ნაღდი ფულის ოდენობამ უნდა გამოიყენოს **ISO 4217** ვალუტის კოდები სწორი მცირე ერთეულებით.[^iso_4217]

##### `pacs.009` / `camt.054` (PvP დაფინანსება და განცხადებები)

- ** აგენტები (`InstgAgt`, `InstdAgt`, მოვალე/კრედიტორი აგენტები)** → **BIC** სურვილისამებრ
  LEI, სადაც დაშვებულია.[^swift_bic]
- ** ანგარიშები **
  - ბანკთაშორისი: იდენტიფიცირება **BIC** და შიდა ანგარიშის მითითებით.
  - მომხმარებელთან დაკავშირებული განცხადებები (`camt.054`): ჩართეთ **IBAN**, როდესაც ეს არის და დაადასტურეთ იგი
    (სიგრძე, ქვეყნის წესები, mod-97 checksum).[^swift_iban]
- **ვალუტა** → **ISO 4217** 3-ასოიანი კოდი, მცირე ერთეულის დამრგვალება.[^iso_4217]
- **Torii გადაყლაპვა** → გაგზავნეთ PvP დაფინანსების ნაბიჯები `POST /v1/iso20022/pacs009`-ის საშუალებით; ხიდი
  მოითხოვს `Purp=SECU` და ახლა ახორციელებს BIC გადასასვლელებს, როდესაც საცნობარო მონაცემები კონფიგურირებულია.

#### ვალიდაციის წესები (გამოიყენება ემისიამდე)

| იდენტიფიკატორი | ვალიდაციის წესი | შენიშვნები |
|------------|-----------------|-------|
| **ISIN** | Regex `^[A-Z]{2}[A-Z0-9]{9}[0-9]$` და Luhn (mod-10) საკონტროლო ციფრი ISO 6166 დანართზე C | უარყოფა ხიდის გაფრქვევამდე; ამჯობინებენ ზემო დინების გამდიდრებას.[^anna_luhn] |
| **CUSIP** | Regex `^[A-Z0-9]{9}$` და modulus-10 2 წონით (სიმბოლოების რუკა ციფრებზე) | მხოლოდ მაშინ, როდესაც ISIN მიუწვდომელია; რუკა ANNA/CUSIP საფეხმავლო გადასასვლელის მეშვეობით, ერთხელ მოპოვებული.[^cusip] |
| **LEI** | Regex `^[A-Z0-9]{18}[0-9]{2}$` და mod-97 საკონტროლო ციფრი (ISO 17442) | დადასტურება GLEIF ყოველდღიური დელტა ფაილების წინააღმდეგ მიღებამდე.[^gleif] |
| **BIC** | რეგექსი `^[A-Z]{4}[A-Z]{2}[A-Z0-9]{2}([A-Z0-9]{3})?$` | სურვილისამებრ ფილიალის კოდი (ბოლო სამი სიმბოლო). დაადასტურეთ აქტიური სტატუსი RA ფაილებში.[^swift_bic] |
| **MIC** | ISO 10383 RA ფაილიდან შენარჩუნება; დარწმუნდით, რომ ადგილები აქტიურია (არ არის `!` შეწყვეტის დროშა) | მონიშნეთ გამორთული MIC-ები ემისიამდე.[^iso_mic] |
| **IBAN** | ქვეყნის სპეციფიკური სიგრძე, დიდი ასო-ციფრული, mod-97 = 1 | გამოიყენეთ SWIFT-ის მიერ შენახული რეესტრი; სტრუქტურულად არასწორი IBAN-ების უარყოფა.[^swift_iban] |
| ** საკუთრების ანგარიშის/პარტიის ID** | `Max35Text` (UTF-8, ≤35 სიმბოლო) ამოჭრილი უფსკრულით | ვრცელდება `GenericAccountIdentification1.Id` და `PartyIdentification135.Othr/Id` ველებზე. უარყოთ ჩანაწერები, რომლებიც აღემატება 35 სიმბოლოს, რათა ხიდის დატვირთვა შეესაბამებოდეს ISO სქემებს. |
| **პროქსი ანგარიშის იდენტიფიკატორები ** | არა ცარიელი `Max2048Text` `…/Prxy/Id` ქვეშ არჩევითი ტიპის კოდებით `…/Prxy/Tp/{Cd,Prtry}`-ში | ინახება პირველადი IBAN-თან ერთად; ვალიდაცია კვლავ მოითხოვს IBAN-ებს, ხოლო პროქსი სახელურების მიღებას (სურვილისამებრ ტიპის კოდებით) PvP რელსების ასახვისთვის. |
| **CFI** | ექვსსიმბოლოიანი კოდი, დიდი ასოები ISO 10962 ტაქსონომიის გამოყენებით | სურვილისამებრ გამდიდრება; დარწმუნდით, რომ სიმბოლოები შეესაბამება ინსტრუმენტის კლასს.[^iso_cfi] |
| **FISN** | 35-მდე სიმბოლო, დიდი ალფანუმერული პლუს შეზღუდული პუნქტუაცია | სურვილისამებრ; შეკვეცა/ნორმალიზება ISO 18774 სახელმძღვანელოს მიხედვით.[^iso_fisn] |
| **ვალუტა** | ISO 4217 3-ასოიანი კოდი, მცირე ერთეულებით განსაზღვრული მასშტაბი | თანხები უნდა დამრგვალდეს დასაშვებ ათწილადამდე; აღსრულება Norito მხარეს.[^iso_4217] |

#### გადასასვლელი და მონაცემთა შენახვის ვალდებულებები

- შეინახეთ **ISIN ↔ Norito აქტივის ID** და **CUSIP ↔ ISIN** გადასასვლელები. განაახლეთ ყოველ ღამე დან
  ANNA/DSB არხები და ვერსია აკონტროლებს CI-ის მიერ გამოყენებულ სნეპშოტებს.[^anna_crosswalk]
- განაახლეთ **BIC ↔ LEI** რუკები GLEIF საჯარო ურთიერთობის ფაილებიდან, რათა ხიდმა შეძლოს
  გამოუშვით ორივე საჭიროების შემთხვევაში.[^bic_lei]
- შეინახეთ **MIC განმარტებები** ხიდის მეტამონაცემებთან ერთად, რათა ადგილის დადასტურება მოხდეს
  განმსაზღვრელია მაშინაც კი, როდესაც RA ფაილები იცვლება დღის შუა რიცხვებში.[^iso_mic]
- ჩაწერეთ მონაცემების წარმოშობა (დროის ნიშა + წყარო) აუდიტის ხიდის მეტამონაცემებში. გაგრძელდეს
  სნეპშოტის იდენტიფიკატორი გამოშვებულ ინსტრუქციებთან ერთად.
- დააკონფიგურირეთ `iso_bridge.reference_data.cache_dir`, რათა შენარჩუნდეს თითოეული ჩატვირთული მონაცემთა ნაკრების ასლი
  წარმოშობის მეტამონაცემებთან ერთად (ვერსია, წყარო, დროის შტამპი, გამშვები ჯამი). ეს საშუალებას აძლევს აუდიტორებს
  და ოპერატორები განასხვავებენ ისტორიულ არხებს ზემოთ სნეპშოტების როტაციის შემდეგაც კი.
- ISO გადასასვლელის კადრები მიიღება `iroha_core::iso_bridge::reference_data`-ის გამოყენებით
  `iso_bridge.reference_data` კონფიგურაციის ბლოკი (ბილიკები + განახლების ინტერვალი). ლიანდაგები
  `iso_reference_status`, `iso_reference_age_seconds`, `iso_reference_records` და
  `iso_reference_refresh_interval_secs` გამოაქვეყნებს გაშვების სიჯანსაღეს გაფრთხილებისთვის. Torii
  ხიდი უარყოფს `pacs.008` წარდგინებებს, რომელთა აგენტის BIC არ არის კონფიგურირებული
  ფეხით გადასასვლელი, დეტერმინისტული `InvalidIdentifier` შეცდომების აღმოფხვრა, როდესაც კონტრაგენტი
  უცნობია.【crates/iroha_torii/src/iso20022_bridge.rs#L1078】
- IBAN და ISO 4217 აკინძები დანერგილია იმავე ფენაზე: pacs.008/pacs.009 ახლა გადის
  გამოსცემს `InvalidIdentifier` შეცდომებს, როდესაც მოვალის/კრედიტორის IBAN-ებს არ გააჩნიათ კონფიგურირებული მეტსახელები ან როდესაც
  ანგარიშსწორების ვალუტა აკლია `currency_assets`-ს, რაც ხელს უშლის არასწორი ხიდის ჩამოყალიბებას
  ინსტრუქციები წიგნის მიღწევიდან. IBAN-ის ვალიდაცია ასევე ვრცელდება კონკრეტულ ქვეყანაში
  სიგრძეები და რიცხვითი საკონტროლო ციფრები, სანამ ISO 7064 mod-97 ასე სტრუქტურულად არასწორია
  მნიშვნელობები ადრე უარყოფილია.
- CLI დასახლების დამხმარეები მემკვიდრეობით იღებენ იგივე დამცავ რელსებს: უღელტეხილზე
  `--iso-reference-crosswalk <path>` `--delivery-instrument-id`-თან ერთად, რომ ჰქონდეს DvP
  წინასწარ დაათვალიერეთ ინსტრუმენტის ID-ების გადახედვა `sese.023` XML სნეპშოტის გამოშვებამდე.【crates/iroha_cli/src/main.rs#L3752】
- `cargo xtask iso-bridge-lint` (და CI შეფუთვა `ci/check_iso_reference_data.sh`) ლინტი
  გადასასვლელის კადრები და მოწყობილობები. ბრძანება იღებს `--isin`, `--bic-lei`, `--mic` და
  გაშვებისას `--fixtures` მონიშნეს და უბრუნდება მონაცემთა ნიმუშებს `fixtures/iso_bridge/`-ში
  არგუმენტების გარეშე.【xtask/src/main.rs#L146】【ci/check_iso_reference_data.sh#L1】
- IVM დამხმარე ახლა იღებს ISO 20022 XML კონვერტებს (head.001 + `DataPDU` + `Document`)
  და ამოწმებს ბიზნეს განაცხადის სათაურს `head.001` სქემის მეშვეობით, ასე რომ, `BizMsgIdr`,
  `MsgDefIdr`, `CreDt` და BIC/ClrSysMmbId აგენტები შენარჩუნებულია დეტერმინისტულად; XMLDSig/XAdES
  ბლოკები რჩება განზრახ გამოტოვებული. 

#### მარეგულირებელი და ბაზრის სტრუქტურის მოსაზრებები- **T+1 ანგარიშსწორება**: აშშ/კანადის აქციების ბაზრები გადავიდა T+1-ზე 2024 წელს; დაარეგულირეთ Norito
  დაგეგმვა და SLA გაფრთხილებები შესაბამისად.[^sec_t1][^csa_t1]
- **CSDR ჯარიმები**: ანგარიშსწორების დისციპლინური წესები აღასრულებს ფულად ჯარიმებს; უზრუნველყოს Norito
  მეტამონაცემები შეიცავს ჯარიმის მითითებებს შერიგებისთვის.[^csdr]
- **ერთსა და იმავე დღის დასახლების პილოტები**: ინდოეთის მარეგულირებელი ეტაპობრივად იწყებს დასახლებას T0/T+0; შენარჩუნება
  ხიდის კალენდრები განახლებულია პილოტების გაფართოებასთან ერთად.[^india_t0]
- **გირაოს შესყიდვები/შეკავებები**: თვალყური ადევნეთ ESMA-ს განახლებებს შესყიდვების ვადების და არჩევითი შეკავებების შესახებ
  ასე რომ, პირობითი მიწოდება (`HldInd`) შეესაბამება უახლეს მითითებებს.[^csdr]

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

### მიწოდება-გადახდის წინააღმდეგ → `sese.023`

| DvP ველი | ISO 20022 გზა | შენიშვნები |
|---------------------------------------------------|
| `settlement_id` | `TxId` | სტაბილური სასიცოცხლო ციკლის იდენტიფიკატორი |
| `delivery_leg.asset_definition_id` (უსაფრთხოება) | `SctiesLeg/FinInstrmId` | კანონიკური იდენტიფიკატორი (ISIN, CUSIP, …) |
| `delivery_leg.quantity` | `SctiesLeg/Qty` | ათწილადი სიმებიანი; აფასებს აქტივების სიზუსტეს |
| `payment_leg.asset_definition_id` (ვალუტა) | `CashLeg/Ccy` | ISO ვალუტის კოდი |
| `payment_leg.quantity` | `CashLeg/Amt` | ათწილადი სიმებიანი; დამრგვალებულია რიცხვითი სპეციფიკაციის მიხედვით |
| `delivery_leg.from` (გამყიდველი/მიმწოდებელი მხარე) | `DlvrgSttlmPties/Pty/Bic` | მონაწილის მიწოდების BIC *(ანგარიშის კანონიკური ID ამჟამად ექსპორტირებულია მეტამონაცემებში)* |
| `delivery_leg.from` ანგარიშის იდენტიფიკატორი | `DlvrgSttlmPties/Acct` | თავისუფალი ფორმა; Norito მეტამონაცემები შეიცავს ანგარიშის ზუსტ ID |
| `delivery_leg.to` (მყიდველი/მიმღები მხარე) | `RcvgSttlmPties/Pty/Bic` | მიმღები მონაწილის BIC |
| `delivery_leg.to` ანგარიშის იდენტიფიკატორი | `RcvgSttlmPties/Acct` | თავისუფალი ფორმა; ემთხვევა მიმღები ანგარიშის ID |
| `plan.order` | `Plan/ExecutionOrder` | რიცხვი: `DELIVERY_THEN_PAYMENT` ან `PAYMENT_THEN_DELIVERY` |
| `plan.atomicity` | `Plan/Atomicity` | რიცხვი: `ALL_OR_NOTHING`, `COMMIT_FIRST_LEG`, `COMMIT_SECOND_LEG` |
| ** შეტყობინების მიზანი ** | `SttlmTpAndAddtlParams/SctiesMvmntTp` | `DELI` (მიწოდება) ან `RECE` (მიღება); სარკეები, რომლებსაც წარმდგენი მხარე ასრულებს. |
|                                                        | `SttlmTpAndAddtlParams/Pmt` | `APMT` (გადახდის საწინააღმდეგოდ) ან `FREE` (გადახდის გარეშე). |
| `delivery_leg.metadata`, `payment_leg.metadata` | `SctiesLeg/Metadata`, `CashLeg/Metadata` | სურვილისამებრ Norito JSON კოდირებული, როგორც UTF‑8 |

> **ანგარიშსწორების კვალიფიკატორები** – ხიდი ასახავს საბაზრო პრაქტიკას ანგარიშსწორების პირობების კოდების (`SttlmTxCond`), ნაწილობრივი ანგარიშსწორების ინდიკატორების (`PrtlSttlmInd`) და სხვა არჩევითი კვალიფიკატორების კოპირებით Norito მეტამონაცემებიდან I101750X-ში, როდესაც არსებობს I101750X. აღასრულეთ ISO გარე კოდების სიებში გამოქვეყნებული აღრიცხვები, რათა დანიშნულების CSD აღიაროს მნიშვნელობები.

### გადახდა-გადახდის დაფინანსება → `pacs.009`

ფულადი სახსრები ფულადი სახსრებისთვის, რომლებიც აფინანსებს PvP ინსტრუქციას, გაიცემა როგორც FI-to-FI კრედიტი
გადარიცხვები. ხიდი ახდენს ამ გადახდების ანოტაციას, ასე რომ, ქვედა დინების სისტემები აღიარებენ
ისინი აფინანსებენ ფასიანი ქაღალდების ანგარიშსწორებას.

| PvP დაფინანსების სფერო | ISO 20022 გზა | შენიშვნები |
|--------------------------------------------------------------------------------------------|
| `primary_leg.quantity` / {თანხა, ვალუტა} | `IntrBkSttlmAmt` + `IntrBkSttlmCcy` | ინიციატორისგან დებეტირებადი თანხა/ვალუტა. |
| კონტრაგენტის აგენტის იდენტიფიკატორები | `InstgAgt`, `InstdAgt` | აგენტების გაგზავნისა და მიღების BIC/LEI. |
| ანგარიშსწორების მიზანი | `CdtTrfTxInf/PmtTpInf/CtgyPurp/Cd` | დააყენეთ `SECU` ფასიანი ქაღალდებთან დაკავშირებული PvP დაფინანსებისთვის. |
| Norito მეტამონაცემები (ანგარიშის ID, FX მონაცემები) | `CdtTrfTxInf/SplmtryData` | ახორციელებს სრულ AccountId-ს, FX დროის ნიშანს, შესრულების გეგმის მინიშნებებს. |
| ინსტრუქციის იდენტიფიკატორი / სასიცოცხლო ციკლის დაკავშირება | `CdtTrfTxInf/PmtId/InstrId`, `CdtTrfTxInf/RmtInf` | ემთხვევა Norito `settlement_id`, რათა ნაღდი ფული შეესაბამებოდეს ფასიანი ქაღალდების მხარეს. |

JavaScript SDK-ის ISO ხიდი შეესაბამება ამ მოთხოვნას ნაგულისხმევად
`pacs.009` კატეგორიის დანიშნულება `SECU`-მდე; აბონენტებმა შეიძლება გააუქმონ ის სხვა
მოქმედი ISO კოდი არა ფასიანი ქაღალდების საკრედიტო გადარიცხვისას, მაგრამ არასწორია
ღირებულებები უარყოფილია წინასწარ.

თუ ინფრასტრუქტურა მოითხოვს ფასიანი ქაღალდების მკაფიო დადასტურებას, ხიდი
აგრძელებს `sese.025` გამოშვებას, მაგრამ ეს დადასტურება ასახავს ფასიანი ქაღალდების ნაწილს
სტატუსი (მაგ., `ConfSts = ACCP`), ვიდრე PvP „მიზანი“.

### გადახდა-გადახდის დადასტურება → `sese.025`

| PvP ველი | ISO 20022 გზა | შენიშვნები |
|------------------------------------------------------------------------|-------|
| `settlement_id` | `TxId` | სტაბილური სასიცოცხლო ციკლის იდენტიფიკატორი |
| `primary_leg.asset_definition_id` | `SttlmCcy` | ვალუტის კოდი პირველადი ფეხისთვის |
| `primary_leg.quantity` | `SttlmAmt` | ინიციატორის მიერ მიწოდებული თანხა |
| `counter_leg.asset_definition_id` | `AddtlInf` (JSON payload) | ვალუტის მრიცხველის კოდი ჩართულია დამატებით ინფორმაციაში |
| `counter_leg.quantity` | `SttlmQty` | მთვლელი თანხა |
| `plan.order` | `Plan/ExecutionOrder` | იგივე სია დაყენებულია, როგორც DvP |
| `plan.atomicity` | `Plan/Atomicity` | იგივე სია დაყენებულია, როგორც DvP |
| `plan.atomicity` სტატუსი (`ConfSts`) | `ConfSts` | `ACCP` როცა ემთხვევა; ხიდი ავრცელებს უკმარისობის კოდებს უარყოფისას |
| კონტრაქტორის იდენტიფიკატორები | `AddtlInf` JSON | მიმდინარე ხიდი ახორციელებს სრულ AccountId/BIC სერიებს მეტამონაცემებში |

### რეპო გირაოს ჩანაცვლება → `colr.007`

| რეპო ველი / კონტექსტი | ISO 20022 გზა | შენიშვნები |
|---------------------------------------------------------------------------------|-------|
| `agreement_id` (`RepoIsi` / `ReverseRepoIsi`) | `OblgtnId` | რეპო კონტრაქტის იდენტიფიკატორი |
| გირაოს ჩანაცვლება Tx იდენტიფიკატორი | `TxId` | გენერირებული თითო ჩანაცვლებაზე |
| ორიგინალური გირაოს რაოდენობა | `Substitution/OriginalAmt` | ჩანაცვლებამდე დაგირავებული მატჩები |
| ორიგინალური გირაოს ვალუტა | `Substitution/OriginalCcy` | ვალუტის კოდი |
| შემცვლელი გირაოს რაოდენობა | `Substitution/SubstituteAmt` | ჩანაცვლების თანხა |
| გირაოს ვალუტის შემცვლელი | `Substitution/SubstituteCcy` | ვალუტის კოდი |
| ძალაში შესვლის თარიღი (მმართველობის მარჟის განრიგი) | `Substitution/EffectiveDt` | ISO თარიღი (YYYY-MM-DD) |
| თმის შეჭრის კლასიფიკაცია | `Substitution/Type` | ამჟამად `FULL` ან `PARTIAL` მმართველობის პოლიტიკის საფუძველზე |
| მმართველობის მიზეზი / თმის შეჭრის შენიშვნა | `Substitution/ReasonCd` | არასავალდებულო, ახორციელებს მმართველობის დასაბუთებას |

### დაფინანსება და განცხადებები

| Iroha კონტექსტი | ISO 20022 შეტყობინება | რუკების ადგილმდებარეობა |
|----------------------------------------------------|------------------|
| რეპო ნაღდი ფულის აალება / განტვირთვა | `pacs.009` | `IntrBkSttlmAmt`, `IntrBkSttlmCcy`, `IntrBkSttlmDt`, `InstgAgt`, `InstdAgt` დასახლებული DvP/PvP ფეხებიდან |
| ანგარიშსწორების შემდგომი განცხადებები | `camt.054` | გადახდის ფეხის მოძრაობები ჩაწერილი `Ntfctn/Ntry[*]` ქვეშ; ხიდი ინექციებს ledger/ანგარიშის მეტამონაცემებს `SplmtryData`-ში |

### გამოყენების შენიშვნები* ყველა თანხა სერიულირდება Norito რიცხვითი დამხმარეების გამოყენებით (`NumericSpec`)
  მასშტაბის შესაბამისობის უზრუნველსაყოფად აქტივების განმარტებებს შორის.
* `TxId` მნიშვნელობებია `Max35Text` — განახორციელეთ UTF‑8 სიგრძე ≤35 სიმბოლო ადრე
  ISO 20022 შეტყობინებების ექსპორტი.
* BIC უნდა შედგებოდეს 8 ან 11 დიდი ალფაციფრული სიმბოლოსგან (ISO9362); უარყოფა
  Norito მეტამონაცემები, რომლებიც ვერ ამოწმებენ გადახდებს ან ანგარიშსწორებას
  დადასტურებები.
* ანგარიშის იდენტიფიკატორები (AccountId / ChainId) ექსპორტირებულია დამატებით
  მეტამონაცემები, რათა მიმღებმა მონაწილეებმა შეძლონ შერიგება ადგილობრივ ბუღალტრებთან.
* `SupplementaryData` უნდა იყოს კანონიკური JSON (UTF‑8, დახარისხებული კლავიშები, JSON-მშობლიური
  გაქცევა). SDK დამხმარეები ახორციელებენ ამას ხელმოწერების, ტელემეტრიის ჰეშებისა და ISO
  დატვირთვის არქივები რჩება განმსაზღვრელი რეკონსტრუქციების დროს.
* ვალუტის ოდენობა მიჰყვება ISO4217 წილადის ციფრებს (მაგალითად, JPY აქვს 0
  ათწილადები, აშშ დოლარს აქვს 2); ხიდი ამაგრებს Norito ციფრულ სიზუსტეს შესაბამისად.
* CLI ანგარიშსწორების დამხმარეები (`iroha app settlement ... --atomicity ...`) ახლა ასხივებენ
  Norito ინსტრუქციები, რომელთა შესრულების გეგმები ასახულია 1:1-ზე `Plan/ExecutionOrder`-მდე და
  `Plan/Atomicity` ზემოთ.
* ISO დამხმარე (`ivm::iso20022`) ამოწმებს ზემოთ ჩამოთვლილ ველებს და უარყოფს
  შეტყობინებები, სადაც DvP/PvP ფეხები არღვევს ციფრულ სპეციფიკაციებს ან კონტრაგენტის რეციპროციულობას.

### SDK Builder Helpers

- JavaScript SDK ახლა ამჟღავნებს `buildPacs008Message` /
  `buildPacs009Message` (იხ. `javascript/iroha_js/src/isoBridge.js`) ასე რომ კლიენტი
  ავტომატიზაციას შეუძლია სტრუქტურირებული ანგარიშსწორების მეტამონაცემების გარდაქმნა (BIC/LEI, IBAN,
  დანიშნულების კოდები, დამატებითი Norito ველები) დეტერმინისტულ პაკებში XML
  ამ სახელმძღვანელოდან რუკების წესების ხელახალი განხორციელების გარეშე.
- ორივე დამხმარე მოითხოვს მკაფიო `creationDateTime` (ISO‑8601 დროის სარტყელთან ერთად)
  ასე რომ, ოპერატორებმა უნდა ამოიღონ დეტერმინისტული დროის შტამპი სამუშაო ნაკადიდან
  SDK-ს ნაგულისხმევად კედლის საათის დროზე დაშვება.
- `recipes/iso_bridge_builder.mjs` გვიჩვენებს, თუ როგორ დააკავშიროთ ეს დამხმარეები
  CLI, რომელიც აერთიანებს გარემოს ცვლადებს ან JSON კონფიგურაციის ფაილებს, ბეჭდავს
  გენერირებული XML და სურვილისამებრ წარუდგენს მას Torii (`ISO_SUBMIT=1`) ხელახლა გამოყენებით
  იგივე ლოდინის სიჩქარე, როგორც ISO ხიდის რეცეპტი.


### ცნობები

- LuxCSD / Clearstream ISO 20022 ანგარიშსწორების მაგალითები, რომლებიც აჩვენებს `SttlmTpAndAddtlParams/SctiesMvmntTp` (`DELI`/`RECE`) და `Pmt` (`APMT`/`FREE`).<sup>[1](https://www.luxcsd.com/resource/blob/3434074/6f8add4708407a4701055be4dd04846b/c23005-eis-examples-cbf-data.pdf)</sup>
- Clearstream DCP სპეციფიკაციები, რომლებიც მოიცავს ანგარიშსწორების კვალიფიკატორებს (`SttlmTxCond`, `PrtlSttlmInd`).<sup>[2](https://www.clearstream.com/clearstream-en/res-library/market-coverage/instruction-specifications-swift-iso-20022-dcp-mode-ceu-spain-2357008)</sup>
- SWIFT PMPG-ის მითითება, რომელიც გირჩევთ `pacs.009`-ს `CtgyPurp/Cd = SECU`-თან ერთად ფასიანი ქაღალდებთან დაკავშირებული PvP დაფინანსებისთვის.<sup>[3](https://www.swift.com/swift-resource/251897/download)</sup>
- ISO 20022 შეტყობინების განსაზღვრის ანგარიშები იდენტიფიკატორის სიგრძის შეზღუდვებისთვის (BIC, Max35Text).<sup>[4](https://www.iso20022.org/sites/default/files/2020-12/ISO20022_MDRPart2_ChangeOrVerifyAccountIdentification_2020_2021_v1_ForSEGReview.pdf)</sup>
- ANNA DSB მითითებები ISIN ფორმატისა და შემოწმების წესების შესახებ.<sup>[5](https://www.anna-dsb.com/isin/)</sup>

### გამოყენების რჩევები

- ყოველთვის ჩასვით შესაბამისი Norito ფრაგმენტი ან CLI ბრძანება, რათა LLM-მა შეძლოს შემოწმება
  ველების ზუსტი სახელები და რიცხვითი მასშტაბები.
- მოითხოვეთ ციტატები (`provide clause references`) ქაღალდის ბილიკის შესანახად
  შესაბამისობა და აუდიტორის მიმოხილვა.
- გადაიღეთ პასუხის შეჯამება `docs/source/finance/settlement_iso_mapping.md`-ში
  (ან დაკავშირებული დანართები), ასე რომ მომავალ ინჟინრებს არ დასჭირდებათ მოთხოვნის გამეორება.

## ღონისძიების შეკვეთის სათამაშო წიგნები (ISO 20022 ↔ Norito Bridge)

### სცენარი A — გირაოს ჩანაცვლება (რეპო / გირავნობა)

** მონაწილეები: ** გირაოს გამცემი/მიმღები (და/ან აგენტები), მეურვე(ები), CSD/T2S  
**დრო:** ბაზრის შეწყვეტის და T2S დღე/ღამის ციკლებზე; მოაწყეთ ორი ფეხი ისე, რომ ისინი დაასრულონ იმავე დასახლების ფანჯარაში.

#### შეტყობინების ქორეოგრაფია
1. `colr.010` გირაოს ჩანაცვლების მოთხოვნა → გირაოს გამცემი/მიმღები ან აგენტი.  
2. `colr.011` გირაოს ჩანაცვლების პასუხი → მიღება/უარი (არასავალდებულო უარის მიზეზი).  
3. `colr.012` გირაოს ჩანაცვლების დადასტურება → ადასტურებს ჩანაცვლების ხელშეკრულებას.  
4. `sese.023` ინსტრუქციები (ორი ფეხი):  
   - დააბრუნეთ ორიგინალი გირაო (`SctiesMvmntTp=DELI`, `Pmt=FREE`, `SctiesTxTp=COLO`).  
   - ჩააბარეთ შემცვლელი გირაო (`SctiesMvmntTp=RECE`, `Pmt=FREE`, `SctiesTxTp=COLI`).  
   დააკავშირეთ წყვილი (იხ. ქვემოთ).  
5. `sese.024` სტატუსის რჩევები (მიღებული, შესაბამისი, მომლოდინე, წარუმატებელი, უარყოფილი).  
6. `sese.025` დადასტურებები ერთხელ დაჯავშნული.  
7. სურვილისამებრ ნაღდი დელტა (საკომისიო/თმის შეჭრა) → `pacs.009` FI-to-FI კრედიტის გადარიცხვა `CtgyPurp/Cd = SECU`-ით; სტატუსი `pacs.002`-ით, ბრუნდება `pacs.004`-ით.

#### საჭირო აღიარება/სტატუსები
- ტრანსპორტის დონე: კარიბჭეებმა შეიძლება გამოუშვან `admi.007` ან უარი თქვან ბიზნესის დამუშავებამდე.  
- ანგარიშსწორების სიცოცხლის ციკლი: `sese.024` (დამუშავების სტატუსები + მიზეზის კოდები), `sese.025` (საბოლოო).  
- ნაღდი ფულის მხარე: `pacs.002` (`PDNG`, `ACSC`, `RJCT` და ა.შ.), `pacs.004` დაბრუნებისთვის.

#### პირობითობა / განტვირთვა ველები
- `SctiesSttlmTxInstr/Lnkgs` (`WITH`/`BEFO`/`AFTE`) ორი ინსტრუქციის დასაკავშირებლად.  
- `SttlmParams/HldInd` გაგრძელდება კრიტერიუმების დაკმაყოფილებამდე; გამოშვება `sese.030`-ით (`sese.031` სტატუსი).  
- `SttlmParams/PrtlSttlmInd` ნაწილობრივი ანგარიშსწორების გასაკონტროლებლად (`NPAR`, `PART`, `PARC`, `PARQ`).  
- `SttlmParams/SttlmTxCond/Cd` ბაზრის სპეციფიკური პირობებისთვის (`NOMC` და ა.შ.).  
- სურვილისამებრ T2S ფასიანი ქაღალდების პირობითი მიწოდების (CoSD) წესები მხარდაჭერის შემთხვევაში.

#### ცნობები
- SWIFT უზრუნველყოფის მართვა MDR (`colr.010/011/012`).  
- CSD/T2S გამოყენების სახელმძღვანელო (მაგ., DNB, ECB Insights) დაკავშირებისა და სტატუსებისთვის.  
- SMPG ანგარიშსწორების პრაქტიკა, Clearstream DCP სახელმძღვანელოები, ASX ISO სემინარები.

### სცენარი B — FX ფანჯრის დარღვევა (PvP დაფინანსების წარუმატებლობა)

** მონაწილეები: ** კონტრაგენტები და ფულადი აგენტები, ფასიანი ქაღალდების მეურვე, CSD/T2S  
**დრო:** FX PvP ფანჯრები (CLS/ორმხრივი) და CSD-ის გამორთვა; შეინახეთ ფასიანი ქაღალდების ფეხები შეჩერებული ნაღდი ფულის დადასტურების მოლოდინში.

#### შეტყობინების ქორეოგრაფია
1. `pacs.009` FI-to-FI საკრედიტო გადარიცხვა თითო ვალუტაზე `CtgyPurp/Cd = SECU`-ით; სტატუსი `pacs.002`-ის მეშვეობით; გამოძახება/გაუქმება `camt.056`/`camt.029` საშუალებით; თუ უკვე მოწესრიგებულია, `pacs.004` დაბრუნდება.  
2. `sese.023` DvP ინსტრუქცია(ებ)ი `HldInd=true`-ით, ასე რომ ფასიანი ქაღალდების ფეხი ელოდება ფულადი სახსრების დადასტურებას.  
3. სიცოცხლის ციკლის `sese.024` შენიშვნები (მიღებული/შესაბამისი/მოლოდინში).  
4. თუ `pacs.009` ორივე ფეხი მიაღწევს `ACSC`-ს ფანჯრის ვადის ამოწურვამდე → გამოუშვით `sese.030` → `sese.031` (მოდის სტატუსი) → `sese.025` (დადასტურება).  
5. თუ FX ფანჯარა დაირღვა → გააუქმეთ/გაუქმეთ ნაღდი ფული (`camt.056/029` ან `pacs.004`) და გააუქმეთ ფასიანი ქაღალდები (`sese.020` + `sese.027`, ან `camt.056/029` უკვე დადასტურებული წესით).

#### საჭირო აღიარება/სტატუსები
- ნაღდი ფული: `pacs.002` (`PDNG`, `ACSC`, `RJCT`), `pacs.004` დაბრუნებისთვის.  
- ფასიანი ქაღალდები: `sese.024` (მოლოდინში/მარცხის მიზეზები, როგორიცაა `NORE`, `ADEA`), `sese.025`.  
- ტრანსპორტი: `admi.007` / კარიბჭე უარყოფილია ბიზნესის დამუშავებამდე.

#### პირობითობა / განტვირთვა ველები
- `SttlmParams/HldInd` + `sese.030` გამოშვება/გაუქმება წარმატების/მარცხის შემთხვევაში.  
- `Lnkgs` ფასიანი ქაღალდების ინსტრუქციების დასაკავშირებლად ნაღდი ფულისთვის.  
- T2S CoSD წესი, თუ იყენებთ პირობით მიწოდებას.  
- `PrtlSttlmInd` გაუთვალისწინებელი ნაწილების თავიდან ასაცილებლად.  
- `pacs.009`-ზე, `CtgyPurp/Cd = SECU` მიუთითებს ფასიან ქაღალდებთან დაკავშირებულ დაფინანსებაზე.

#### ცნობები
- PMPG / CBPR+ სახელმძღვანელო ფასიანი ქაღალდების პროცესებში გადახდებისთვის.  
- SMPG ანგარიშსწორების პრაქტიკა, T2S შეხედულებები დაკავშირების/დაკავების შესახებ.  
- Clearstream DCP სახელმძღვანელოები, ECMS დოკუმენტაცია ტექნიკური შეტყობინებებისთვის.

### pacs.004 დააბრუნეთ რუკების შენიშვნები

- დაბრუნების მოწყობილობები ახლა ნორმალიზდება `ChrgBr` (`DEBT`/`CRED`/`SHAR`/`SLEV`) და დაბრუნების საკუთრების მიზეზები, რომლებიც გამოვლენილია როგორც I18NI000000, როგორც I18NI000000, მომხმარებლის ხელახალი დაბრუნების მიზეზები, როგორც I18NI000000. კოდები XML კონვერტის ხელახალი ანალიზის გარეშე.
- AppHdr ხელმოწერის ბლოკები `DataPDU` კონვერტებში რჩება იგნორირებული მიღებისას; აუდიტი უნდა ეყრდნობოდეს არხის წარმოშობას და არა ჩაშენებულ XMLDSIG ველებს.

### ხიდის საოპერაციო ჩამონათვალი
- განახორციელეთ ზემოთ მოცემული ქორეოგრაფია (გირაო: `colr.010/011/012 → sese.023/024/025`; FX დარღვევა: `pacs.009 (+pacs.002) → sese.023 held → release/cancel`).  
- განიხილეთ `sese.024`/`sese.025` სტატუსები და `pacs.002` შედეგები, როგორც კარიბჭის სიგნალები; `ACSC` იწვევს გათავისუფლებას, `RJCT` აიძულებს განტვირთვას.  
- პირობითი მიწოდების კოდირება `HldInd`, `Lnkgs`, `PrtlSttlmInd`, `SttlmTxCond` და არჩევითი CoSD წესების მეშვეობით.  
- გამოიყენეთ `SupplementaryData` გარე ID-ების კორელაციისთვის (მაგ., UETR `pacs.009`-ისთვის) საჭიროების შემთხვევაში.  
- შეკავების/განტვირთვის დროის პარამეტრიზაცია საბაზრო კალენდრის/შეწყვეტის მიხედვით; გამოუშვით `sese.030`/`camt.056` გაუქმების ვადამდე, საჭიროების შემთხვევაში დაბრუნება.

### ISO 20022 დატვირთვის ნიმუში (ანოტირებული)

#### გირაოს შემცვლელი წყვილი (`sese.023`) ინსტრუქციის კავშირით

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

გაგზავნეთ დაკავშირებული ინსტრუქცია `SUBST-2025-04-001-B` (FoP მიღება შემცვლელი გირაოს) `SctiesMvmntTp=RECE`, `Pmt=FREE` და `WITH` კავშირით, რომელიც მიუთითებს `SUBST-2025-04-001-A`-ზე. გაათავისუფლეთ ორივე ფეხი შესაბამისი `sese.030`-ით, როგორც კი ჩანაცვლება დამტკიცდება.

#### ფასიანი ქაღალდების ფეხი შეჩერებულია FX დადასტურების მოლოდინში (`sese.023` + `sese.030`)

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
```გაათავისუფლეთ მას შემდეგ, რაც `pacs.009` ორივე ფეხი მიაღწევს `ACSC`-ს:

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

`sese.031` ადასტურებს შეჩერების გათავისუფლებას, რასაც მოჰყვება `sese.025` ფასიანი ქაღალდების ნაწილის დაჯავშნის შემდეგ.

#### PvP დაფინანსების ეტაპი (`pacs.009` ფასიანი ქაღალდების მიზნით)

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

`pacs.002` აკონტროლებს გადახდის სტატუსს (`ACSC` = დადასტურებულია, `RJCT` = უარყოფა). თუ ფანჯარა გატეხილია, გაიხსენეთ `camt.056`/`camt.029` ან გაგზავნეთ `pacs.004` გადახდილი თანხების დასაბრუნებლად.
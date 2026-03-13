---
lang: hy
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

:::note Կանոնական աղբյուր
:::

## կարգավորում ↔ ISO 20022 Դաշտային քարտեզագրում

Այս նշումը նկարագրում է Iroha կարգավորման հրահանգների կանոնական քարտեզագրումը
(`DvpIsi`, `PvpIsi`, ռեպո գրավի հոսքեր) և իրականացված ISO 20022 հաղորդագրությունները
կամրջի մոտ։ Այն արտացոլում է հաղորդագրության փայտամածը, որն իրականացվում է
`crates/ivm/src/iso20022.rs` և ծառայում է որպես հղում, երբ արտադրում է կամ
վավերացնելով Norito ծանրաբեռնվածությունը:

### Հղման տվյալների քաղաքականություն (նույնացուցիչներ և վավերացում)

Այս քաղաքականությունը փաթեթավորում է նույնացուցիչի նախապատվությունները, վավերացման կանոնները և հղումային տվյալները
պարտավորություններ, որոնք Norito ↔ ISO 20022 կամուրջը պետք է կատարի նախքան հաղորդագրություններ ուղարկելը:

**Խարիսխ կետերը ISO հաղորդագրության ներսում.**
- **Գործիքների նույնացուցիչներ** → `delivery_leg.asset_definition_id` ↔ `SctiesLeg/FinInstrmId`
  (կամ գործիքի համարժեք դաշտ):
- **Կողմեր / գործակալներ** → `DlvrgSttlmPties/Pty` և `RcvgSttlmPties/Pty` `sese.*`-ի համար,
  կամ գործակալի կառուցվածքները `pacs.009`-ում:
- **Հաշիվներ** → `…/Acct` տարրեր պահպանման/կանխիկ հաշիվների համար; հայելային մատյան
  `AccountId` `SupplementaryData`-ում:
- **Գույքային նույնացուցիչներ** → `…/OthrId` `Tp/Prtry`-ով և արտացոլված
  `SupplementaryData`. Երբեք մի փոխարինեք կարգավորվող նույնացուցիչները սեփականությամբ:

#### Նույնացուցիչի նախապատվությունը ըստ հաղորդագրությունների ընտանիքի

##### `sese.023` / `.024` / `.025` (արժեթղթերի հաշվարկ)

- **Գործիք (`FinInstrmId`)**
  - Նախընտրելի՝ **ISIN** `…/ISIN`-ի ներքո: Դա CSD/T2S-ի կանոնական նույնացուցիչն է:[^anna]
  - Հետադարձ կապ.
    - **CUSIP** կամ այլ NSIN `…/OthrId/Id`-ի ներքո՝ `Tp/Cd`-ով սահմանված ISO արտաքինից
      կոդերի ցանկ (օրինակ՝ `CUSP`); թողարկողին ներառել `Issr`-ում, երբ լիազորված է։[^iso_mdr]
    - **Norito ակտիվի ID** որպես սեփականություն՝ `…/OthrId/Id`, `Tp/Prtry="NORITO_ASSET_ID"` և
      գրանցեք նույն արժեքը `SupplementaryData`-ում:
  - Ընտրովի նկարագրիչներ՝ **CFI** (`ClssfctnTp`) և **FISN**, որտեղ աջակցվում է հեշտացնելու համար
    հաշտեցում։[^iso_cfi][^iso_fisn]
- **Կուսակցություններ (`DlvrgSttlmPties`, `RcvgSttlmPties`)**
  - Նախընտրելի՝ **BIC** (`AnyBIC/BICFI`, ISO 9362):[^swift_bic]
  - Հետադարձ. **LEI** որտեղ հաղորդագրության տարբերակը բացահայտում է հատուկ LEI դաշտ; եթե
    բացակայում է, կրում է սեփական ID-ներ՝ հստակ `Prtry` պիտակներով և ներառում BIC-ը մետատվյալներում:[^iso_cr]
- **Բնակավայրի / վայրի** → **MIC** վայրի համար և **BIC** CSD-ի համար:[^iso_mic]

##### `colr.010` / `.011` / `.012` և `colr.007` (գրավի կառավարում)

- Հետևեք գործիքի նույն կանոններին, ինչ `sese.*` (նախընտրելի է ISIN):
- Կողմերը լռելյայն օգտագործում են **BIC**; **LEI** ընդունելի է, որտեղ սխեման բացահայտում է այն։[^swift_bic]
- Կանխիկի գումարները պետք է օգտագործեն **ISO 4217** արժույթի կոդերը՝ ճիշտ փոքր միավորներով:[^iso_4217]

##### `pacs.009` / `camt.054` (PvP ֆինանսավորում և քաղվածքներ)

- **Գործակալներ (`InstgAgt`, `InstdAgt`, պարտապաններ/պարտատեր գործակալներ)** → **BIC** ընտրովի
  LEI որտեղ թույլատրվում է։[^swift_bic]
- ** Հաշիվներ **
  - Միջբանկային. նույնականացնել **BIC**-ով և ներքին հաշվի հղումներով:
  - Հաճախորդների առնչությամբ հայտարարություններ (`camt.054`). ներառել **IBAN** երբ առկա է և վավերացնել այն
    (երկարություն, երկրի կանոններ, mod-97 checksum):[^swift_iban]
- **Արժույթ** → **ISO 4217** 3 տառանոց կոդը, հարգեք փոքր միավորի կլորացումը։[^iso_4217]
- **Torii կլանում** → Ներկայացրեք PvP ֆինանսավորման ոտքերը `POST /v2/iso20022/pacs009`-ի միջոցով; կամուրջը
  պահանջում է `Purp=SECU` և այժմ կիրառում է BIC հետիոտնային անցումներ, երբ տեղեկատու տվյալները կազմաձևված են:

#### Վավերացման կանոններ (կիրառել մինչև արտանետումը)

| Նույնացուցիչ | Վավերացման կանոն | Ծանոթագրություններ |
|------------|-----------------|-------|
| **ISIN** | Regex `^[A-Z]{2}[A-Z0-9]{9}[0-9]$` և Luhn (mod-10) ստուգիչ նիշ ըստ ISO 6166 հավելվածի C | Մերժել մինչև կամրջի արտանետումը; նախընտրում են հոսանքին հակառակ հարստացումը։[^anna_luhn] |
| **CUSIP** | Regex `^[A-Z0-9]{9}$` և մոդուլ-10 2 կշռով (նիշերի քարտեզը թվանշաններով) | Միայն այն դեպքում, երբ ISIN-ն անհասանելի է. քարտեզ ANNA/CUSIP հետիոտնային անցումի միջոցով մեկ անգամ սկզբնավորվելով։[^cusip] |
| **LEI** | Regex `^[A-Z0-9]{18}[0-9]{2}$` և mod-97 ստուգիչ նիշ (ISO 17442) | Վավերացրե՛ք GLEIF-ի ամենօրյա դելտա ֆայլերի հետ մինչև ընդունումը։[^gleif] |
| **BIC** | Ռեգեքս `^[A-Z]{4}[A-Z]{2}[A-Z0-9]{2}([A-Z0-9]{3})?$` | Մասնաճյուղի կամընտիր կոդը (վերջին երեք նիշերը): Հաստատեք ակտիվ կարգավիճակը ՀՀ ֆայլերում։[^swift_bic] |
| **MIC** | Պահպանել ISO 10383 RA ֆայլից; ապահովել վայրերի ակտիվությունը (ոչ `!` դադարեցման դրոշակ) | Նշեք շահագործումից հանված MIC-ները մինչև արտանետումը։[^iso_mic] |
| **IBAN** | Երկրին հատուկ երկարություն, մեծատառ այբբենական թվեր, mod-97 = 1 | Օգտագործեք SWIFT-ի կողմից պահպանվող ռեեստրը; մերժել կառուցվածքային առումով անվավեր IBAN-ները։[^swift_iban] |
| **Գույքային հաշվի/կուսակցության ID-ներ** | `Max35Text` (UTF-8, ≤35 նիշ) կտրված բացատով | Կիրառվում է `GenericAccountIdentification1.Id` և `PartyIdentification135.Othr/Id` դաշտերին: Մերժեք 35 նիշը գերազանցող գրառումները, որպեսզի կամրջի օգտակար բեռները համապատասխանեն ISO սխեմաներին: |
| **Վստահված անձի հաշվի նույնացուցիչներ** | Ոչ դատարկ `Max2048Text` `…/Prxy/Id` տակ՝ կամընտիր տիպի կոդերով `…/Prxy/Tp/{Cd,Prtry}`-ում | Պահպանվում է առաջնային IBAN-ի կողքին; վավերացումը դեռևս պահանջում է IBAN-ներ՝ միաժամանակ ընդունելով վստահված անձի բռնակներ (ընտրովի տիպի կոդերով)՝ PvP ռելսերը արտացոլելու համար: |
| **CFI** | Վեց նիշանոց ծածկագիր, մեծատառեր՝ օգտագործելով ISO 10962 դասակարգումը | Ընտրովի հարստացում; ապահովել, որ նիշերը համապատասխանեն գործիքների դասին։[^iso_cfi] |
| **FISN** | Մինչև 35 նիշ, մեծատառ այբբենական և սահմանափակ կետադրական նշաններ | Ընտրովի; կրճատել/նորմալացնել ISO 18774 ուղեցույցի համաձայն։[^iso_fisn] |
| **Արժույթ ** | ISO 4217 3 տառանոց կոդը, սանդղակը որոշվում է փոքր միավորներով | Գումարները պետք է կլորացվեն մինչև թույլատրելի տասնորդականները. ուժի մեջ դնել Norito կողմում։[^iso_4217] |

#### Հետիոտնային անցում և տվյալների պահպանման պարտավորություններ

- Պահպանեք **ISIN ↔ Norito ակտիվի ID** և **CUSIP ↔ ISIN** հետիոտնային անցումներ: Թարմացրեք ամեն գիշեր սկսած
  ANNA/DSB հոսքերը և տարբերակները վերահսկում են CI-ի կողմից օգտագործվող նկարները:[^anna_crosswalk]
- Թարմացրեք **BIC ↔ LEI ** քարտեզագրումները GLEIF հանրային հարաբերությունների ֆայլերից, որպեսզի կամուրջը կարողանա
  անհրաժեշտության դեպքում թողարկեք երկուսն էլ։[^bic_lei]
- Պահպանեք **MIC սահմանումները** կամրջի մետատվյալների կողքին, որպեսզի տեղի վավերացվի
  որոշիչ նույնիսկ այն դեպքում, երբ ՀՀ ֆայլերը փոխվում են կեսօր:[^iso_mic]
- Գրանցեք տվյալների ծագումը (ժամանականիշ + աղբյուր) կամուրջի մետատվյալներում աուդիտի համար: Համառեք
  snapshot-ի նույնացուցիչը թողարկված հրահանգների հետ մեկտեղ:
- Կարգավորեք `iso_bridge.reference_data.cache_dir`, որպեսզի պահպանվի յուրաքանչյուր բեռնված տվյալների կրկնօրինակը
  ծագման մետատվյալների հետ մեկտեղ (տարբերակ, աղբյուր, ժամադրոշմ, ստուգիչ գումար): Սա թույլ է տալիս աուդիտորներին
  և օպերատորները՝ տարբերելու պատմական լրահոսերը նույնիսկ վերին հոսանքով նկարահանումների պտտումից հետո:
- ISO-ի հետիոտնային խաչմերուկի նկարները կլանվում են `iroha_core::iso_bridge::reference_data`-ի կողմից՝ օգտագործելով
  `iso_bridge.reference_data` կոնֆիգուրացիայի բլոկը (ուղիներ + թարմացման ընդմիջում): Չափիչներ
  `iso_reference_status`, `iso_reference_age_seconds`, `iso_reference_records` և
  `iso_reference_refresh_interval_secs` ցուցադրում է աշխատաժամանակի առողջությունը զգուշացման համար: Torii
  կամուրջը մերժում է `pacs.008` ներկայացումները, որոնց գործակալի BIC-ները բացակայում են կազմաձևվածից
  հետիոտնային անցում, վերհանելով `InvalidIdentifier` դետերմինիստական սխալները, երբ կոնտրագենտը
  անհայտ:【crates/iroha_torii/src/iso20022_bridge.rs#L1078】
- IBAN-ի և ISO 4217-ի կապակցումները կիրառվում են նույն շերտում. pacs.008/pacs.009-ը հոսում է հիմա
  թողարկել `InvalidIdentifier` սխալներ, երբ պարտապանի/պարտատիրոջ IBAN-ներում բացակայում են կազմաձևված անունները կամ երբ
  հաշվարկային արժույթը բացակայում է `currency_assets`-ից՝ կանխելով սխալ ձևավորված կամուրջը
  ցուցումներ գրանցամատյան հասնելու համար: IBAN վավերացումը կիրառվում է նաև երկրի համար
  երկարությունները և թվային ստուգիչ թվերը մինչև ISO 7064 mod‑97-ն այնքան կառուցվածքային առումով անվավեր է
  արժեքները վաղաժամ են մերժվում:【crates/iroha_torii/src/iso20022_bridge.rs#L775】【crates/iroha_torii/src/iso20022_bridge.rs#L827】【crates/ivm/src/iso20022_bridge.rs#L775
- CLI-ի կարգավորման օգնականները ժառանգում են նույն պահակային ռելսերը՝ անցում
  `--iso-reference-crosswalk <path>` `--delivery-instrument-id`-ի կողքին DvP ունենալու համար
  նախադիտեք վավերացրեք գործիքների ID-ները՝ նախքան `sese.023` XML ակնարկը հրապարակելը:【crates/iroha_cli/src/main.rs#L3752】
- `cargo xtask iso-bridge-lint` (և CI փաթաթան `ci/check_iso_reference_data.sh`) թաղանթ
  հետիոտնային խաչմերուկի նկարներ և հարմարանքներ: Հրամանը ընդունում է `--isin`, `--bic-lei`, `--mic` և
  `--fixtures`-ը դրոշակում է և աշխատում է `fixtures/iso_bridge/`-ի նմուշային տվյալների հավաքածուներին
  առանց արգումենտների։【xtask/src/main.rs#L146】【ci/check_iso_reference_data.sh#L1】
- IVM օգնականն այժմ ընդունում է իրական ISO 20022 XML ծրարներ (գլուխ.001 + `DataPDU` + `Document`)
  և հաստատում է Բիզնես հավելվածի վերնագիրը `head.001` սխեմայի միջոցով, ուստի `BizMsgIdr`,
  `MsgDefIdr`, `CreDt` և BIC/ClrSysMmbId գործակալները պահպանվում են դետերմինիստորեն. XMLDSig/XAdES
  բլոկները մնում են միտումնավոր բաց թողնված: 

#### Կարգավորող և շուկայական կառուցվածքի նկատառումներ- **T+1 հաշվարկ**. ԱՄՆ/Կանադայի բաժնետոմսերի շուկաները 2024 թվականին տեղափոխվեցին T+1; հարմարեցնել Norito
  պլանավորում և համապատասխանաբար SLA ծանուցումներ։[^sec_t1][^csa_t1]
- **CSDR տույժեր**. հաշվարկների կարգապահության կանոնները կիրառում են դրամական տույժեր. ապահովել Norito
  մետատվյալները ներառում են տույժերի հղումներ հաշտեցման համար:[^csdr]
- **Նույն օրվա բնակավայրերի օդաչուներ**. Հնդկաստանի կարգավորիչը փուլ առ փուլ տեղափոխում է T0/T+0 բնակավայրը. պահել
  կամուրջների օրացույցները թարմացվել են, քանի որ օդաչուները ընդլայնվում են:[^india_t0]
- **Գրավային գնումներ / պահումներ**. վերահսկեք ESMA-ի թարմացումները գնումների ժամանակացույցերի և կամընտիր պահումների վերաբերյալ
  այնպես որ պայմանական առաքումը (`HldInd`) համապատասխանում է վերջին ուղեցույցին։[^csdr]

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

### Առաքում՝ ընդդեմ վճարման → `sese.023`

| DvP դաշտ | ISO 20022 ուղի | Ծանոթագրություններ |
|---------------------------------------------------- |
| `settlement_id` | `TxId` | Կայուն կյանքի ցիկլի նույնացուցիչ |
| `delivery_leg.asset_definition_id` (անվտանգություն) | `SctiesLeg/FinInstrmId` | Կանոնական նույնացուցիչ (ISIN, CUSIP, …) |
| `delivery_leg.quantity` | `SctiesLeg/Qty` | Տասնորդական տող; հարգում է ակտիվների ճշգրտությունը |
| `payment_leg.asset_definition_id` (արժույթ) | `CashLeg/Ccy` | ISO արժույթի կոդը |
| `payment_leg.quantity` | `CashLeg/Amt` | Տասնորդական տող; կլորացված ըստ թվային սպեկտրի |
| `delivery_leg.from` (վաճառող / առաքող կողմ) | `DlvrgSttlmPties/Pty/Bic` | Առաքող մասնակցի BIC *(հաշվի կանոնական ID-ն ներկայումս արտահանվում է մետատվյալներով)* |
| `delivery_leg.from` հաշվի նույնացուցիչ | `DlvrgSttlmPties/Acct` | Ազատ ձև; Norito մետատվյալները կրում են հաշվի ճշգրիտ ID |
| `delivery_leg.to` (գնորդ/ստացող կողմ) | `RcvgSttlmPties/Pty/Bic` | Ընդունող մասնակցի BIC |
| `delivery_leg.to` հաշվի նույնացուցիչ | `RcvgSttlmPties/Acct` | Ազատ ձև; համընկնում է ստացող հաշվի ID |
| `plan.order` | `Plan/ExecutionOrder` | Enum՝ `DELIVERY_THEN_PAYMENT` կամ `PAYMENT_THEN_DELIVERY` |
| `plan.atomicity` | `Plan/Atomicity` | Թվային թիվը՝ `ALL_OR_NOTHING`, `COMMIT_FIRST_LEG`, `COMMIT_SECOND_LEG` |
| **Հաղորդագրության նպատակը** | `SttlmTpAndAddtlParams/SctiesMvmntTp` | `DELI` (առաքում) կամ `RECE` (ստացում); հայելիներ, որոնց ոտքը կատարում է ներկայացնող կողմը: |
|                                                        | `SttlmTpAndAddtlParams/Pmt` | `APMT` (վճարման դիմաց) կամ `FREE` (անվճար): |
| `delivery_leg.metadata`, `payment_leg.metadata` | `SctiesLeg/Metadata`, `CashLeg/Metadata` | Լրացուցիչ Norito JSON կոդավորված որպես UTF‑8 |

> **Հաշվարկային որակավորումներ** – կամուրջը արտացոլում է շուկայական պրակտիկան՝ պատճենելով հաշվարկային պայմանների ծածկագրերը (`SttlmTxCond`), մասնակի հաշվարկային ցուցիչները (`PrtlSttlmInd`) և այլ կամընտիր որակավորումներ Norito մետատվյալներից I1018NI000-ում, երբ առկա է: Կիրառեք ISO արտաքին ծածկագրերի ցուցակներում հրապարակված թվարկումները, որպեսզի նպատակակետ CSD-ն ճանաչի արժեքները:

### Վճարում ընդդեմ վճարման ֆինանսավորման → `pacs.009`

Կանխիկի դիմաց դրամական միջոցները, որոնք ֆինանսավորում են PvP հրահանգը, տրվում են որպես FI-to-FI վարկ
փոխանցումներ. Կամուրջը նշում է այս վճարումները, որպեսզի ներքևի համակարգերը ճանաչեն
նրանք ֆինանսավորում են արժեթղթերի մարումը:

| PvP ֆինանսավորման դաշտ | ISO 20022 ուղի | Ծանոթագրություններ |
|---------------------------------------------------------------------------------------------------|
| `primary_leg.quantity` / {գումար, արժույթ} | `IntrBkSttlmAmt` + `IntrBkSttlmCcy` | Նախաձեռնողից գանձված գումար/արժույթ: |
| Կողմնակից գործակալի նույնացուցիչներ | `InstgAgt`, `InstdAgt` | Գործակալներ ուղարկելու և ընդունող BIC/LEI: |
| Հաշվարկի նպատակը | `CdtTrfTxInf/PmtTpInf/CtgyPurp/Cd` | Արժեթղթերի հետ կապված PvP ֆինանսավորման համար դրեք `SECU`: |
| Norito մետատվյալներ (հաշվի ID, արտարժույթի տվյալներ) | `CdtTrfTxInf/SplmtryData` | Իրականացնում է ամբողջական AccountId, FX ժամադրոշմներ, կատարման պլանի ակնարկներ: |
| Հրահանգի նույնացուցիչ / կյանքի ցիկլի կապող | `CdtTrfTxInf/PmtId/InstrId`, `CdtTrfTxInf/RmtInf` | Համապատասխանում է Norito `settlement_id`-ին, որպեսզի կանխիկի ոտքը համադրվի արժեթղթերի կողմի հետ: |

JavaScript SDK-ի ISO կամուրջը համընկնում է այս պահանջի հետ՝ կանխադրելով
`pacs.009` կատեգորիայի նպատակը մինչև `SECU`; զանգահարողները կարող են փոխարինել այն մեկ ուրիշով
վավեր ISO կոդը ոչ արժեթղթերի վարկային փոխանցումներ ուղարկելիս, բայց անվավեր
արժեքները նախապես մերժվում են:

Եթե ենթակառուցվածքը պահանջում է արժեթղթերի հստակ հաստատում, ապա կամուրջը
շարունակում է արտանետել `sese.025`, սակայն այդ հաստատումն արտացոլում է արժեթղթերի ոտքը
կարգավիճակը (օրինակ՝ `ConfSts = ACCP`), այլ ոչ թե PvP «նպատակը»:

### Վճարման դիմաց վճարման հաստատում → `sese.025`

| PvP դաշտ | ISO 20022 ուղի | Ծանոթագրություններ |
|--------------------------------------------------------------------------|-------|
| `settlement_id` | `TxId` | Կայուն կյանքի ցիկլի նույնացուցիչ |
| `primary_leg.asset_definition_id` | `SttlmCcy` | Արտարժույթի կոդը հիմնական ոտքի |
| `primary_leg.quantity` | `SttlmAmt` | Նախաձեռնողի կողմից առաքված գումարը |
| `counter_leg.asset_definition_id` | `AddtlInf` (JSON ծանրաբեռնվածություն) | Արժույթի հաշվիչի կոդը ներառված է լրացուցիչ տեղեկատվության մեջ |
| `counter_leg.quantity` | `SttlmQty` | Հաշվիչ գումար |
| `plan.order` | `Plan/ExecutionOrder` | Նույն թիվը սահմանված է որպես DvP |
| `plan.atomicity` | `Plan/Atomicity` | Նույն թիվը սահմանված է որպես DvP |
| `plan.atomicity` կարգավիճակ (`ConfSts`) | `ConfSts` | `ACCP`, երբ համընկնում է; կամուրջը թողարկում է ձախողման կոդերը մերժման դեպքում |
| Կողմնակիցների նույնացուցիչներ | `AddtlInf` JSON | Ընթացիկ կամուրջը մետատվյալներում սերիականացնում է AccountId/BIC ամբողջական կրկնօրինակները |

### Repo գրավի փոխարինում → `colr.007`

| Ռեպո դաշտ / համատեքստ | ISO 20022 ուղի | Ծանոթագրություններ |
|------------------------------------------------------------------------------------|----------|
| `agreement_id` (`RepoIsi` / `ReverseRepoIsi`) | `OblgtnId` | Ռեպո պայմանագրի նույնացուցիչ |
| Գրավի փոխարինում Tx նույնացուցիչ | `TxId` | Ստեղծված մեկ փոխարինման |
| Բնօրինակ գրավի քանակ | `Substitution/OriginalAmt` | Փոխարինումից առաջ գրավ դրված խաղերը |
| Օրիգինալ գրավի արժույթ | `Substitution/OriginalCcy` | Արժույթի կոդը |
| Փոխարինող գրավի քանակ | `Substitution/SubstituteAmt` | Փոխարինման գումարը |
| Փոխարինող գրավի արժույթ | `Substitution/SubstituteCcy` | Արժույթի կոդը |
| Ուժի մեջ մտնելու ամսաթիվ (կառավարման մարժայի ժամանակացույց) | `Substitution/EffectiveDt` | ISO ամսաթիվ (ՏՏՏՏ-ԱՄ-ՕՕ) |
| Սանրվածքի դասակարգում | `Substitution/Type` | Ներկայումս `FULL` կամ `PARTIAL` հիմնված կառավարման քաղաքականության |
| Կառավարման պատճառ / սանրվածքի նշում | `Substitution/ReasonCd` | Ընտրովի, իրականացնում է կառավարման հիմնավորում |

### Ֆինանսավորում և հայտարարություններ

| Iroha համատեքստ | ISO 20022 հաղորդագրություն | Քարտեզագրման վայրը |
|----------------------------------------------------------------------------------|
| Repo cash leg ignition / unwind | `pacs.009` | `IntrBkSttlmAmt`, `IntrBkSttlmCcy`, `IntrBkSttlmDt`, `InstgAgt`, `InstdAgt` բնակեցված DvP/PvP ոտքերից |
| Հետհաշվարկային քաղվածքներ | `camt.054` | `Ntfctn/Ntry[*]`-ով գրանցված վճարման ոտքերի շարժումները; կամուրջը ներարկում է մատյան/հաշվի մետատվյալներ `SplmtryData`-ում |

### Օգտագործման նշումներ* Բոլոր գումարները սերիականացված են՝ օգտագործելով Norito թվային օգնականները (`NumericSpec`)
  ապահովել սանդղակի համապատասխանությունը ակտիվների սահմանումներին:
* `TxId` արժեքներն են `Max35Text` — նախքան կիրառեք UTF‑8 երկարությունը ≤35 նիշ
  արտահանում դեպի ISO 20022 հաղորդագրություններ:
* BIC-ները պետք է կազմեն 8 կամ 11 մեծատառ այբբենական թվային նիշ (ISO9362); մերժել
  Norito մետատվյալներ, որոնք ձախողվում են այս ստուգումը նախքան վճարումները կամ վճարումները ուղարկելը
  հաստատումներ։
* Հաշվի նույնացուցիչները (AccountId / ChainId) արտահանվում են լրացուցիչ
  մետատվյալներ, որպեսզի ստացող մասնակիցները կարողանան հաշտվել իրենց տեղական մատյանների հետ:
* `SupplementaryData`-ը պետք է լինի կանոնական JSON (UTF‑8, տեսակավորված ստեղներ, JSON բնիկ
  փախչելով): SDK-ի օգնականները պարտադրում են դա, այնպես որ ստորագրությունները, հեռաչափության հեշերը և ISO-ն
  բեռնվածության արխիվները մնում են որոշիչ վերակառուցման ընթացքում:
* Արժույթի գումարները հետևում են ISO4217 կոտորակի թվանշաններին (օրինակ, JPY-ն ունի 0
  տասնորդականները, ԱՄՆ դոլարը ունի 2); կամուրջը համապատասխանաբար սեղմում է Norito թվային ճշգրտությունը:
* CLI կարգավորման օգնականները (`iroha app settlement ... --atomicity ...`) այժմ արտանետում են
  Norito հրահանգներ, որոնց կատարման պլանները քարտեզագրում են 1:1-ից մինչև `Plan/ExecutionOrder` և
  `Plan/Atomicity` վերևում:
* ISO օգնականը (`ivm::iso20022`) վավերացնում է վերը թվարկված դաշտերը և մերժում
  հաղորդագրություններ, որտեղ DvP/PvP ոտքերը խախտում են թվային բնութագրերը կամ կոնտրագենտի փոխադարձությունը:

### SDK Builder Helpers

- JavaScript SDK-ն այժմ բացահայտում է `buildPacs008Message` /
  `buildPacs009Message` (տես `javascript/iroha_js/src/isoBridge.js`), ուստի հաճախորդ
  ավտոմատացումը կարող է փոխակերպել կառուցվածքային հաշվարկների մետատվյալները (BIC/LEI, IBAN,
  նպատակային կոդեր, լրացուցիչ Norito դաշտեր) XML դետերմինիստական փաթեթների մեջ
  առանց սույն ուղեցույցի քարտեզագրման կանոնների վերակիրառման:
- Երկու օգնականներն էլ պահանջում են հստակ `creationDateTime` (ISO‑8601 ժամային գոտիով)
  այնպես որ օպերատորները պետք է փոխարենը իրենց աշխատանքային հոսքից դետերմինիստական ժամանակի դրոշմ անցնեն
  SDK-ին լռելյայն թողնելով պատի ժամացույցի ժամանակը:
- `recipes/iso_bridge_builder.mjs`-ը ցույց է տալիս, թե ինչպես միացնել այդ օգնականները
  CLI, որը միավորում է միջավայրի փոփոխականները կամ JSON կազմաձևման ֆայլերը, տպում է
  ստեղծվել է XML, և կամայականորեն այն ներկայացնում է Torii (`ISO_SUBMIT=1`)՝ կրկին օգտագործելով
  նույն սպասման արագությունը, ինչ ISO կամուրջի բաղադրատոմսը:


### Հղումներ

- LuxCSD / Clearstream ISO 20022 հաշվարկման օրինակներ, որոնք ցույց են տալիս `SttlmTpAndAddtlParams/SctiesMvmntTp` (`DELI`/`RECE`) և `Pmt` (`APMT`/`FREE`).<sup>[1](https://www.luxcsd.com/resource/blob/3434074/6f8add4708407a4701055be4dd04846b/c23005-eis-examples-cbf-data.pdf)</sup>
- Clearstream DCP բնութագրերը, որոնք ընդգրկում են հաշվարկային որակավորումները (`SttlmTxCond`, `PrtlSttlmInd`):<sup>[2](https://www.clearstream.com/clearstream-en/res-library/market-coverage/instruction-specifications-swift-iso-20022-dcp-mode-ceu-spain-2357008)</sup>
- SWIFT PMPG ուղեցույց, որն առաջարկում է `pacs.009` `CtgyPurp/Cd = SECU`-ով արժեթղթերի հետ կապված PvP ֆինանսավորման համար:<sup>[3](https://www.swift.com/swift-resource/251897/download)</sup>
- ISO 20022 հաղորդագրության սահմանման հաշվետվություններ նույնացուցիչի երկարության սահմանափակումների համար (BIC, Max35Text):<sup>[4](https://www.iso20022.org/sites/default/files/2020-12/ISO20022_MDRPart2_ChangeOrVerifyAccountIdentification_2020_2021_v1_ForSEGReview.pdf)</sup>
- ANNA DSB ուղեցույց ISIN ձևաչափի և ստուգիչ գումարի կանոնների վերաբերյալ:<sup>[5](https://www.anna-dsb.com/isin/)</sup>

### Օգտագործման խորհուրդներ

- Միշտ տեղադրեք համապատասխան Norito հատվածը կամ CLI հրամանը, որպեսզի LLM-ն կարողանա ստուգել
  դաշտերի ճշգրիտ անվանումները և թվային սանդղակները:
- Խնդրեք մեջբերումներ (`provide clause references`)՝ թղթի հետք պահելու համար
  համապատասխանությունը և աուդիտորի վերանայումը:
- Գրեք պատասխանի ամփոփագիրը `docs/source/finance/settlement_iso_mapping.md`-ում
  (կամ կապակցված հավելվածներ), այնպես որ ապագա ինժեներները կարիք չունեն կրկնելու հարցումը:

## Իրադարձությունների պատվիրման գրքույկներ (ISO 20022 ↔ Norito Bridge)

### Սցենար Ա — Գրավի փոխարինում (Ռեպո / Գրավ)

** Մասնակիցներ. ** գրավ տվող/վերցնող (և/կամ գործակալներ), պահառու(ներ), CSD/T2S  
**Ժամանակը՝ ** շուկայական կտրվածքի և T2S օր/գիշերային ցիկլերի համար; կազմակերպել երկու ոտքերը, որպեսզի դրանք ավարտվեն նույն կարգավորման պատուհանում:

#### Հաղորդագրություն խորեոգրաֆիա
1. `colr.010` Գրավի փոխարինման հարցում → գրավ տվող/վերցնող կամ գործակալ:  
2. `colr.011` Գրավի փոխարինման պատասխան → ընդունել/մերժել (մերժման կամընտիր պատճառ):  
3. `colr.012` Գրավի փոխարինման հաստատում → հաստատում է փոխարինման պայմանագիրը:  
4. `sese.023` հրահանգներ (երկու ոտք).  
   - Վերադարձրեք բնօրինակ գրավը (`SctiesMvmntTp=DELI`, `Pmt=FREE`, `SctiesTxTp=COLO`):  
   - Տրամադրել փոխարինող գրավ (`SctiesMvmntTp=RECE`, `Pmt=FREE`, `SctiesTxTp=COLI`):  
   Կապեք զույգը (տես ստորև):  
5. `sese.024` կարգավիճակի վերաբերյալ խորհրդատվություն (ընդունված, համապատասխանեցված, սպասվող, ձախողված, մերժված):  
6. `sese.025` հաստատումներ մեկ անգամ ամրագրվելուց:  
7. Լրացուցիչ կանխիկ դելտա (վճարներ/սանրվածք) → `pacs.009` FI-to-FI վարկային փոխանցում `CtgyPurp/Cd = SECU`-ով; կարգավիճակը `pacs.002`-ի միջոցով, վերադառնում է `pacs.004`-ի միջոցով:

#### Պահանջվող հաստատումներ / կարգավիճակներ
- Տրանսպորտի մակարդակ. դարպասները կարող են արտանետել `admi.007` կամ մերժվել մինչև բիզնեսի մշակումը:  
- Հաշվարկների կյանքի ցիկլը՝ `sese.024` (մշակման կարգավիճակներ + պատճառի ծածկագրեր), `sese.025` (վերջնական):  
- Կանխիկի կողմ՝ `pacs.002` (`PDNG`, `ACSC`, `RJCT` և այլն), `pacs.004` վերադարձի համար:

#### Պայմանականություն / արձակել դաշտերը
- `SctiesSttlmTxInstr/Lnkgs` (`WITH`/`BEFO`/`AFTE`) երկու հրահանգները շղթայելու համար:  
- `SttlmParams/HldInd` պահել մինչև չափանիշների բավարարումը; թողարկում `sese.030`-ի միջոցով (`sese.031` կարգավիճակ):  
- `SttlmParams/PrtlSttlmInd` մասնակի կարգավորումը վերահսկելու համար (`NPAR`, `PART`, `PARC`, `PARQ`):  
- `SttlmParams/SttlmTxCond/Cd` շուկայական հատուկ պայմանների համար (`NOMC` և այլն):  
- կամընտիր T2S Պայմանական Արժեթղթերի Առաքման (CoSD) կանոններ, երբ աջակցվում է:

#### Հղումներ
- SWIFT գրավի կառավարում MDR (`colr.010/011/012`):  
- CSD/T2S օգտագործման ուղեցույցներ (օրինակ՝ DNB, ECB Insights) կապելու և կարգավիճակների համար:  
- SMPG հաշվարկային պրակտիկա, Clearstream DCP ձեռնարկներ, ASX ISO սեմինարներ:

### Սցենար Բ — FX պատուհանի խախտում (PvP ֆինանսավորման ձախողում)

**Մասնակիցներ.** կոնտրագենտներ և դրամական գործակալներ, արժեթղթերի պահառու, CSD/T2S  
**Ժամկետ.** FX PvP պատուհանների (CLS/երկկողմանի) և CSD-ի անջատումներ; պահեք արժեթղթերի ոտքերը՝ սպասելով կանխիկի հաստատմանը:

#### Հաղորդագրություն խորեոգրաֆիա
1. `pacs.009` FI-to-FI վարկային փոխանցում մեկ արժույթով `CtgyPurp/Cd = SECU`-ով; կարգավիճակը `pacs.002`-ի միջոցով; հետ կանչել/չեղարկել `camt.056`/`camt.029`-ի միջոցով; եթե արդեն կարգավորված է, `pacs.004` վերադարձ:  
2. `sese.023` DvP հրահանգ(ներ) `HldInd=true`-ով, որպեսզի արժեթղթերի ոտքը սպասի կանխիկի հաստատմանը:  
3. Կյանքի ցիկլի `sese.024` ծանուցումներ (ընդունված/համապատասխանեցված/սպասող):  
4. Եթե `pacs.009` երկու ոտքերը հասնում են `ACSC`-ին մինչև պատուհանի ժամկետի ավարտը → թողարկեք `sese.030` → `sese.031` (mod կարգավիճակ) → `sese.025` (հաստատում):  
5. Եթե արտարժույթի պատուհանը խախտվում է, → չեղարկեք/հետ կանչեք կանխիկ գումարը (`camt.056/029` կամ `pacs.004`) և չեղարկեք արժեթղթերը (`sese.020` + `sese.027`, կամ `sese.027`, կամ I18NI000003103-ի համար արդեն հաստատված կանոնը):

#### Պահանջվող հաստատումներ / կարգավիճակներ
- Կանխիկ՝ `pacs.002` (`PDNG`, `ACSC`, `RJCT`), `pacs.004` վերադարձի համար:  
- Արժեթղթեր՝ `sese.024` (սպասող/անհաջող պատճառներ, ինչպիսիք են `NORE`, `ADEA`), `sese.025`:  
- Տրանսպորտ. `admi.007` / դարպասը մերժվում է բիզնեսի մշակումից առաջ:

#### Պայմանականություն / արձակել դաշտերը
- `SttlmParams/HldInd` + `sese.030` թողարկում/չեղարկում հաջողության/ձախողման դեպքում:  
- `Lnkgs` արժեթղթերի ցուցումները կանխիկի ոտքին կապելու համար:  
- T2S CoSD կանոն, եթե օգտագործում եք պայմանական առաքում:  
- `PrtlSttlmInd`՝ չնախատեսված մասնակիությունը կանխելու համար:  
- `pacs.009`-ում, `CtgyPurp/Cd = SECU`-ում նշվում է արժեթղթերի հետ կապված ֆինանսավորումը:

#### Հղումներ
- PMPG / CBPR+ ուղեցույց արժեթղթերի գործընթացներում վճարումների համար:  
- SMPG կարգավորման պրակտիկա, T2S պատկերացումներ կապելու/պահումների վերաբերյալ:  
- Clearstream DCP ձեռնարկներ, ECMS փաստաթղթեր տեխնիկական սպասարկման հաղորդագրությունների համար:

### pacs.004 վերադարձնել քարտեզագրման նշումները

- վերադարձի հարմարանքները այժմ նորմալացվում են `ChrgBr` (`DEBT`/`CRED`/`SHAR`/`SLEV`) և սեփականության վերադարձի պատճառները, որոնք բացահայտված են որպես I18NI000000, սպառողների վերադարձի պատճառները, որոնք բացահայտված են որպես I18NI000000: կոդերը՝ առանց XML ծրարը նորից վերլուծելու:
- AppHdr ստորագրության բլոկները `DataPDU` ծրարների ներսում մնում են անտեսված ներթափանցելիս; աուդիտները պետք է հիմնվեն ալիքի ծագման վրա, այլ ոչ թե ներկառուցված XMLDSIG դաշտերի վրա:

### Կամուրջի գործառնական ստուգաթերթ
- Կիրառեք վերը նշված խորեոգրաֆիան (գրավը՝ `colr.010/011/012 → sese.023/024/025`; FX խախտում՝ `pacs.009 (+pacs.002) → sese.023 held → release/cancel`):  
- Վերաբերվեք `sese.024`/`sese.025` կարգավիճակներին և `pacs.002`-ի արդյունքներին որպես մուտքի ազդանշաններ; `ACSC`-ը գործարկում է արձակումը, `RJCT`-ը ստիպում է լիցքաթափվել:  
- Կոդավորեք պայմանական առաքումը `HldInd`, `Lnkgs`, `PrtlSttlmInd`, `SttlmTxCond` և կամընտիր CoSD կանոնների միջոցով:  
- Անհրաժեշտության դեպքում օգտագործեք `SupplementaryData` արտաքին ID-ները (օրինակ՝ UETR `pacs.009`-ի համար) փոխկապակցելու համար:  
- Պարամետրիզացրեք պահման/հանգուցալուծման ժամանակը ըստ շուկայական օրացույցի/հատումների; թողարկել `sese.030`/`camt.056` մինչև չեղարկման վերջնաժամկետները, անհրաժեշտության դեպքում վերադարձ՝ վերադարձ:

### Նմուշ ISO 20022 օգտակար բեռներ (Ծանոթագրված)

#### Գրավի փոխարինման զույգ (`sese.023`) հրահանգների կապակցմամբ

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

Ներկայացրե՛ք կապակցված `SUBST-2025-04-001-B` հրահանգը (FoP ստանալ փոխարինող գրավ)՝ `SctiesMvmntTp=RECE`, `Pmt=FREE` և `WITH` կապով, որը մատնացույց է անում դեպի `SUBST-2025-04-001-A`: Փոխարինումը հաստատվելուց հետո բաց թողեք երկու ոտքերը համապատասխան `sese.030`-ով:

#### Արժեթղթերի ոտքը սպասում է արտարժույթի հաստատման (`sese.023` + `sese.030`)

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
```Թողարկեք, երբ երկու `pacs.009` ոտքերը հասնեն `ACSC`:

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

`sese.031`-ը հաստատում է պահման թողարկումը, որին հաջորդում է `sese.025`, երբ արժեթղթերի ոտքը ամրագրվի:

#### PvP ֆինանսավորման փուլ (`pacs.009` արժեթղթերի նպատակով)

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

`pacs.002`-ը հետևում է վճարման կարգավիճակին (`ACSC` = հաստատված, `RJCT` = մերժում): Եթե ​​պատուհանը խախտված է, հետ կանչեք `camt.056`/`camt.029` կամ ուղարկեք `pacs.004`՝ վճարված միջոցները վերադարձնելու համար:
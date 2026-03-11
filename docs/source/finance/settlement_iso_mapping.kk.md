---
lang: kk
direction: ltr
source: docs/source/finance/settlement_iso_mapping.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d1f1005d6a273ab732a7c7a7adca349c17569fe2e2755b8daccf2186724044f8
source_last_modified: "2026-01-22T16:26:46.568382+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Есеп айырысу ↔ ISO 20022 Field Mapping

Бұл жазба Iroha есеп айырысу нұсқаулары арасындағы канондық салыстыруды түсіреді
(`DvpIsi`, `PvpIsi`, репо кепіл ағындары) және орындалған ISO 20022 хабарлары
көпірмен. Ол іске асырылған хабарламалар құрылымын көрсетеді
`crates/ivm/src/iso20022.rs` және өндіру кезінде анықтама ретінде қызмет етеді немесе
Norito пайдалы жүктемелерін тексеру.

### Анықтамалық деректер саясаты (идентификаторлар және тексеру)

Бұл саясат идентификатор теңшелімдерін, тексеру ережелерін және сілтеме деректерін жинақтайды
Norito ↔ ISO 20022 көпірі хабарларды шығарар алдында орындауы тиіс міндеттемелер.

**ISO хабарламасының ішіндегі бекіту нүктелері:**
- **Құрал идентификаторлары** → `delivery_leg.asset_definition_id` ↔ `SctiesLeg/FinInstrmId`
  (немесе баламалы құрал өрісі).
- **Тараптар/агенттер** → `DlvrgSttlmPties/Pty` және `sese.*` үшін `RcvgSttlmPties/Pty`,
  немесе `pacs.009` ішіндегі агент құрылымдары.
- **Шоттар** → `…/Acct` сақтау/қолма-қол ақша шоттары үшін элементтері; кітапшаның айнасы
  `AccountId`, `SupplementaryData`.
- **Меншікті идентификаторлар** → `…/OthrId` `Tp/Prtry` және шағылыстырылған
  `SupplementaryData`. Ешқашан реттелетін идентификаторларды меншікті идентификаторлармен алмастырмаңыз.

#### Хабарлар тобы бойынша идентификатор таңдауы

##### `sese.023` / `.024` / `.025` (бағалы қағаздар бойынша есеп айырысу)

- **Құрал (`FinInstrmId`)**
  - Таңдаулы: `…/ISIN` астында **ISIN**. Бұл CSD / T2S үшін канондық идентификатор.[^anna]
  - Қайтармалар:
    - **CUSIP** немесе сыртқы ISO стандартынан `Tp/Cd` орнатылған `…/OthrId/Id` астында басқа NSIN
      код тізімі (мысалы, `CUSP`); міндеттелген кезде эмитентті `Issr` ішіне қосыңыз.[^iso_mdr]
    - **Norito актив идентификаторы** меншік иесі ретінде: `…/OthrId/Id`, `Tp/Prtry="NORITO_ASSET_ID"` және
      `SupplementaryData` ішінде бірдей мәнді жазыңыз.
  - Қосымша дескрипторлар: **CFI** (`ClssfctnTp`) және **FISN** жеңілдету үшін қолдау көрсетіледі
    татуластыру.[^iso_cfi][^iso_fisn]
- **Тараптар (`DlvrgSttlmPties`, `RcvgSttlmPties`)**
  - Таңдаулы: **BIC** (`AnyBIC/BICFI`, ISO 9362).[^swift_bic]
  - Қалпына келтіру: **LEI** мұнда хабардың нұсқасы арнайы LEI өрісін көрсетеді; егер
    жоқ болса, анық `Prtry` белгілері бар жеке идентификаторларды алып жүріңіз және метадеректерге BIC қосыңыз.[^iso_cr]
- **Орындау орны/өткізу орны** → Өткізу орны үшін **MIC** және CSD үшін **BIC**.[^iso_mic]

##### `colr.010` / `.011` / `.012` және `colr.007` (кепілді басқару)

- `sese.*` (ISIN қолайлы) сияқты құрал ережелерін орындаңыз.
- Тараптар әдепкі бойынша **BIC** пайдаланады; **LEI** схемасы ашылған жерде қолайлы.[^swift_bic]
- Қолма-қол ақша сомаларында дұрыс кіші бірліктері бар **ISO 4217** валюта кодтары қолданылуы керек.[^iso_4217]

##### `pacs.009` / `camt.054` (PvP қаржыландыру және мәлімдемелер)- **Агенттер (`InstgAgt`, `InstdAgt`, борышкер/кредитор агенттері)** → **BIC** міндетті емес
  Рұқсат етілген жерде LEI.[^swift_bic]
- **Тіркелгілер**
  - Банкаралық: **BIC** және ішкі шот анықтамалары бойынша сәйкестендіріңіз.
  - Тұтынушыға арналған мәлімдемелер (`camt.054`): бар болған кезде **IBAN** қосыңыз және оны растаңыз
    (ұзындығы, ел ережелері, mod-97 бақылау сомасы).[^swift_iban]
- **Валюта** → **ISO 4217** 3 әріптік код, кіші бірлікті дөңгелектеуді сақтаңыз.[^iso_4217]
- **Torii қабылдау** → `POST /v1/iso20022/pacs009` арқылы PvP қаржыландыру аяқтарын жіберу; көпір
  `Purp=SECU` талап етеді және енді анықтамалық деректер конфигурацияланған кезде BIC жаяу жүргіншілер өткелдерін қамтамасыз етеді.

#### Валидация ережелері (шығарылу алдында қолданылады)

| Идентификатор | Валидация ережесі | Ескертпелер |
|------------|-----------------|-------|
| **ISIN** | Regex `^[A-Z]{2}[A-Z0-9]{9}[0-9]$` және Luhn (mod-10) бақылау саны ISO 6166 C қосымшасы | Көпір шығару алдында бас тарту; жоғары ағынды байытуды қалайды.[^anna_luhn] |
| **CUSIP** | Regex `^[A-Z0-9]{9}$` және 2 салмағы бар модуль-10 (таңбалар сандармен салыстырылады) | ISIN қолжетімсіз болғанда ғана; ANNA/CUSIP жаяу жүргіншілер жолы арқылы карта бір рет алынған.[^cusip] |
| **LEI** | Regex `^[A-Z0-9]{18}[0-9]{2}$` және mod-97 тексеру саны (ISO 17442) | Қабылдау алдында GLEIF күнделікті дельта файлдарын растаңыз.[^gleif] |
| **BIC** | Regex `^[A-Z]{4}[A-Z]{2}[A-Z0-9]{2}([A-Z0-9]{3})?$` | Қосымша тармақ коды (соңғы үш таңба). RA файлдарындағы белсенді күйді растаңыз.[^swift_bic] |
| **МИК** | ISO 10383 RA файлынан сақтаңыз; орындардың белсенді болуын қамтамасыз ету (`!` тоқтату жалауы жоқ) | Шығарылғанға дейін қолданыстан шығарылған МИК-терді белгілеңіз.[^iso_mic] |
| **IBAN** | Елге тән ұзындық, бас әріптік-цифрлық, mod-97 = 1 | SWIFT жүргізетін тізілімді пайдаланыңыз; құрылымдық жарамсыз IBAN кодтарын қабылдамау.[^swift_iban] |
| **Меншікті тіркелгі/тарап идентификаторлары** | Кесілген бос орыны бар `Max35Text` (UTF-8, ≤35 таңба) | `GenericAccountIdentification1.Id` және `PartyIdentification135.Othr/Id` өрістеріне қолданылады. Пайдалы жүктемелер ISO схемаларына сәйкес келуі үшін 35 таңбадан асатын жазбаларды қабылдамаңыз. |
| **Прокси тіркелгі идентификаторлары** | `…/Prxy/Tp/{Cd,Prtry}` ішіндегі қосымша түр кодтары бар `…/Prxy/Id` астында бос емес `Max2048Text` | Негізгі IBAN-мен бірге сақталады; валидация PvP рельстерін көрсету үшін прокси дескрипторларды (қосымша түр кодтарымен) қабылдау кезінде әлі де IBAN қажет етеді. |
| **CFI** | ISO 10962 таксономиясын қолданатын алты таңбалы код, бас әріптер | Қосымша байыту; таңбалардың құрал класына сәйкес келетініне көз жеткізіңіз.[^iso_cfi] |
| **FISN** | 35 таңбаға дейін, бас әріптік-сандық және шектеулі тыныс белгілері | Қосымша; ISO 18774 нұсқаулығына сәйкес қысқарту/қалыпты ету.[^iso_fisn] |
| **Валюта** | ISO 4217 3 әріпті код, кіші бірліктермен анықталған масштаб | Сомалар рұқсат етілген ондық бөлшектерге дейін дөңгелектенуі керек; Norito жағында орындау.[^iso_4217] |

#### Жаяу жүргіншілер жолы және деректерге техникалық қызмет көрсету бойынша міндеттемелер- **ISIN ↔ Norito актив идентификаторын** және **CUSIP ↔ ISIN** жаяу жүргіншілер өткелдерін ұстаңыз. Түнде жаңартыңыз
  ANNA/DSB арналары және нұсқасы CI пайдаланатын суреттерді басқарады.[^anna_crosswalk]
- GLEIF қоғамдық байланыс файлдарынан **BIC ↔ LEI** салыстыруларын жаңартыңыз, осылайша көпір
  қажет болғанда екеуін де шығарыңыз.[^bic_lei]
- **MIC анықтамаларын** көпірдің метадеректерімен бірге сақтаңыз, осылайша орынды растаңыз
  RA файлдары күн ортасында өзгерсе де детерминирленген.[^iso_mic]
- Аудит үшін көпір метадеректерінде деректердің шығуын жазыңыз (уақыт белгісі + көз). табыңыз
  шығарылған нұсқаулармен бірге сурет идентификаторы.
- Әрбір жүктелген деректер жиынының көшірмесін сақтау үшін `iso_bridge.reference_data.cache_dir` конфигурациялаңыз
  шығу метадеректерімен бірге (нұсқа, дереккөз, уақыт белгісі, бақылау сомасы). Бұл аудиторларға мүмкіндік береді
  және операторлар тіпті жоғары ағындағы суреттер айналдырылғаннан кейін де тарихи арналарды ажыратады.
- ISO жаяу жүргіншілер жолының суреттері `iroha_core::iso_bridge::reference_data` арқылы қабылданады.
  `iso_bridge.reference_data` конфигурация блогы (жолдар + жаңарту аралығы). Өлшегіштер
  `iso_reference_status`, `iso_reference_age_seconds`, `iso_reference_records` және
  `iso_reference_refresh_interval_secs` ескерту үшін орындалу уақытының күйін көрсетеді. Torii
  көпір `pacs.008` жіберілімдерін қабылдамайды, олардың агенті BIC конфигурацияланғанда жоқ
  жаяу жүргіншілер жолы, контрагент болған кезде детерминирленген `InvalidIdentifier` қателері
  белгісіз.【crates/iroha_torii/src/iso20022_bridge.rs#L1078】
- IBAN және ISO 4217 байланыстырулары бір деңгейде орындалады: pacs.008/pacs.009 ағындары қазір
  борышкер/кредитордың IBAN нөмірлері конфигурацияланған бүркеншік аттар болмаған кезде немесе `InvalidIdentifier` қателерін шығарады
  `currency_assets`-те есеп айырысу валютасы жоқ, бұл дұрыс емес көпірді болдырмайды
  кітапқа қол жеткізу туралы нұсқаулар. IBAN валидациясы елге қатысты да қолданылады
  ISO 7064 mod‑97 өтуге дейінгі ұзындықтар мен сандық тексеру сандары құрылымдық жағынан жарамсыз
  мәндер ерте қабылданбады.【crates/iroha_torii/src/iso20022_bridge.rs#L775】【crates/iroha_torii/src/iso20022_bridge.rs#L827】【crates/ivm/src/iso20012.
- CLI есеп айырысу көмекшілері бірдей қорғаныс рельстерін мұраға алады: өту
  DvP болуы үшін `--iso-reference-crosswalk <path>` `--delivery-instrument-id` бірге
  `sese.023` XML суретін шығарудан бұрын құрал идентификаторларын тексеру.【crates/iroha_cli/src/main.rs#L3752】
- `cargo xtask iso-bridge-lint` (және CI орауыш `ci/check_iso_reference_data.sh`) талшық
  жаяу жүргіншілер өткелінің суреттері мен құрылғылары. Пәрмен `--isin`, `--bic-lei`, `--mic` және
  `--fixtures` жалаушаларды қояды және іске қосылған кезде `fixtures/iso_bridge/` үлгі деректер жиынына түседі
  аргументсіз.【xtask/src/main.rs#L146】【ci/check_iso_reference_data.sh#L1】
- IVM көмекшісі енді нақты ISO 20022 XML конверттерін қабылдайды (бас.001 + `DataPDU` + `Document`)
  және `head.001` схемасы арқылы іскери қолданба тақырыбын растайды, сондықтан `BizMsgIdr`,
  `MsgDefIdr`, `CreDt` және BIC/ClrSysMmbId агенттері детерминирленген түрде сақталады; XMLDSig/XAdES
  блоктар әдейі өткізілмей қалады. Регрессия сынақтары үлгілерді және жаңасын пайдаланадысалыстыруларды қорғауға арналған тақырып конвертінің арматурасы.【crates/ivm/src/iso20022.rs:265】【crates/ivm/src/iso20022.rs:3301】【crates/ivm/src/iso20022.rs:370

#### Нормативтік және нарықтық құрылымдық ойлар

- **T+1 есеп айырысуы**: АҚШ/Канада үлестік нарықтары 2024 жылы T+1 деңгейіне көшті; Norito реттеңіз
  сәйкесінше жоспарлау және SLA ескертулері.[^sec_t1][^csa_t1]
- **CSDR айыппұлдары**: Есеп айырысу тәртібінің ережелері ақшалай өсімпұлдарды жүзеге асырады; Norito қамтамасыз етіңіз
  метадеректер салыстыру үшін айыппұл сілтемелерін жазады.[^csdr]
- **Бір күндік есеп айырысу ұшқыштары**: Үндістанның реттеушісі T0/T+0 есеп айырысуында кезең-кезеңімен жұмыс істейді; сақтау
  көпір күнтізбелері ұшқыштар кеңейген сайын жаңартылды.[^india_t0]
- **Кепілді сатып алулар/ұстаулар**: сатып алу уақыт шкаласы мен қосымша ұстаулар бойынша ESMA жаңартуларын бақылаңыз
  сондықтан шартты жеткізу (`HldInd`) соңғы нұсқауларға сәйкес келеді.[^csdr]

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

### Жеткізу төлемге қарсы → `sese.023`| DvP өрісі | ISO 20022 жолы | Ескертпелер |
|------------------------------------------------------|---------------------------------------|-------|
| `settlement_id` | `TxId` | Тұрақты өмірлік цикл идентификаторы |
| `delivery_leg.asset_definition_id` (қауіпсіздік) | `SctiesLeg/FinInstrmId` | Канондық идентификатор (ISIN, CUSIP, …) |
| `delivery_leg.quantity` | `SctiesLeg/Qty` | Ондық жол; актив дәлдігін құрметтейді |
| `payment_leg.asset_definition_id` (валюта) | `CashLeg/Ccy` | ISO валюта коды |
| `payment_leg.quantity` | `CashLeg/Amt` | Ондық жол; Сандық спецификация | бойынша дөңгелектенеді
| `delivery_leg.from` (сатушы / жеткізуші тарап) | `DlvrgSttlmPties/Pty/Bic` | Жеткізуші қатысушының BIC коды *(есептік жазбаның канондық идентификаторы қазіргі уақытта метадеректерде экспортталған)* |
| `delivery_leg.from` тіркелгі идентификаторы | `DlvrgSttlmPties/Acct` | Еркін форма; Norito метадеректерінде нақты тіркелгі идентификаторы |
| `delivery_leg.to` (сатып алушы / қабылдаушы тарап) | `RcvgSttlmPties/Pty/Bic` | Қабылдаушы қатысушының БСК |
| `delivery_leg.to` тіркелгі идентификаторы | `RcvgSttlmPties/Acct` | Еркін форма; | алушы тіркелгі идентификаторына сәйкес келеді
| `plan.order` | `Plan/ExecutionOrder` | Санақ: `DELIVERY_THEN_PAYMENT` немесе `PAYMENT_THEN_DELIVERY` |
| `plan.atomicity` | `Plan/Atomicity` | Санақ: `ALL_OR_NOTHING`, `COMMIT_FIRST_LEG`, `COMMIT_SECOND_LEG` |
| **Хабарлама мақсаты** | `SttlmTpAndAddtlParams/SctiesMvmntTp` | `DELI` (жеткізу) немесе `RECE` (алу); ұсынушы тарап қай аяқты орындайтынын көрсетеді. |
|                                                        | `SttlmTpAndAddtlParams/Pmt` | `APMT` (төлемге қарсы) немесе `FREE` (төлемсіз). |
| `delivery_leg.metadata`, `payment_leg.metadata` | `SctiesLeg/Metadata`, `CashLeg/Metadata` | Қосымша Norito JSON UTF‑8 ретінде кодталған |

> **Есеп айырысу квалификациялары** – көпір есеп айырысу шартының кодтарын (`SttlmTxCond`), ішінара есеп айырысу көрсеткіштерін (`PrtlSttlmInd`) және Norito метадеректерінен бар болған кезде Norito ішінен басқа да міндетті емес жіктеуіштерді көшіру арқылы нарық тәжірибесін көрсетеді. Тағайындалған CSD мәндерді танитындай, ISO сыртқы код тізімдерінде жарияланған тізімдерді орындаңыз.

### Төлем-төлемге қарсы қаржыландыру → `pacs.009`

PvP нұсқаулығын қаржыландыратын қолма-қол ақша FI-to-FI кредиті ретінде беріледі.
аударымдар. Көпір бұл төлемдерге түсініктеме береді, сондықтан төменгі ағындық жүйелер таниды
олар бағалы қағаздармен есеп айырысуды қаржыландырады.| PvP қаржыландыру өрісі | ISO 20022 жолы | Ескертпелер |
|------------------------------------------------|----------------------------------------------------|-------|
| `primary_leg.quantity` / {сома, валюта} | `IntrBkSttlmAmt` + `IntrBkSttlmCcy` | Бастамашыдан есептен шығарылған сома/валюта. |
| Қарсы тарап агентінің идентификаторлары | `InstgAgt`, `InstdAgt` | Жіберуші және қабылдаушы агенттердің BIC/LEI. |
| Есеп айырысу мақсаты | `CdtTrfTxInf/PmtTpInf/CtgyPurp/Cd` | Бағалы қағаздарға қатысты PvP қаржыландыру үшін `SECU` мәніне орнатыңыз. |
| Norito метадеректері (есептік жазба идентификаторлары, FX деректері) | `CdtTrfTxInf/SplmtryData` | Толық AccountId, FX уақыт белгілерін, орындау жоспарының кеңестерін орындайды. |
| Нұсқау идентификаторы / өмірлік циклді байланыстыру | `CdtTrfTxInf/PmtId/InstrId`, `CdtTrfTxInf/RmtInf` | Norito `settlement_id` сәйкес келеді, сондықтан қолма-қол ақшаның бөлігі бағалы қағаздар жағымен сәйкес келеді. |

JavaScript SDK ISO көпірі әдепкі бойынша осы талапқа сәйкес келеді
`pacs.009` санатының мақсаты `SECU`; қоңырау шалушылар оны басқасымен ауыстыра алады
бағалы қағаздарды емес несиелік аударымдарды шығару кезінде жарамды ISO коды, бірақ жарамсыз
мәндер алдын ала қабылданбайды.

Егер инфрақұрылым нақты бағалы қағаздарды растауды талап етсе, көпір
`sese.025` шығаруды жалғастырады, бірақ бұл растау бағалы қағаздардың бөлігін көрсетеді
PvP «мақсаты» емес, күй (мысалы, `ConfSts = ACCP`).

### Төлемге қарсы төлемді растау → `sese.025`

| PvP өрісі | ISO 20022 жолы | Ескертпелер |
|-----------------------------------------|--------------------------|-------|
| `settlement_id` | `TxId` | Тұрақты өмірлік цикл идентификаторы |
| `primary_leg.asset_definition_id` | `SttlmCcy` | Негізгі кезеңнің валюта коды |
| `primary_leg.quantity` | `SttlmAmt` | Бастамашы жеткізген сома |
| `counter_leg.asset_definition_id` | `AddtlInf` (JSON пайдалы жүктемесі) | Қосымша ақпаратқа енгізілген қарсы валюта коды |
| `counter_leg.quantity` | `SttlmQty` | Есептегіш сома |
| `plan.order` | `Plan/ExecutionOrder` | DvP | сияқты бірдей нөмір орнатылған
| `plan.atomicity` | `Plan/Atomicity` | DvP | сияқты бірдей нөмір орнатылған
| `plan.atomicity` күйі (`ConfSts`) | `ConfSts` | сәйкес кезде `ACCP`; көпір қабылдамау кезінде сәтсіздік кодтарын шығарады |
| Контрагент идентификаторлары | `AddtlInf` JSON | Ағымдағы көпір метадеректердегі толық AccountId/BIC кортеждерін сериялайды |

Мысал (байланыстар, ұстау және нарықтық MIC бар CLI ISO алдын ала қарау):

```sh
iroha app settlement dvp \
  --settlement-id DVP-FIXTURE-1 \
  --delivery-asset security#equities \
  --delivery-quantity 500 \
  --delivery-from i105... \
  --delivery-to i105... \
  --payment-asset usd#fi \
  --payment-quantity 1050000 \
  --payment-from i105... \
  --payment-to i105... \
  --delivery-instrument-id US0378331005 \
  --place-of-settlement-mic XNAS \
  --partial-indicator npar \
  --hold-indicator \
  --settlement-condition NOMC \
  --linkage WITH:PACS009-CLS \
  --linkage BEFO:SUBST-PAIR-B \
  --iso-xml-out sese023_preview.xml
```

### РЕПО кепілін ауыстыру → `colr.007`| Репо өрісі / контекст | ISO 20022 жолы | Ескертпелер |
|------------------------------------------------|----------------------------------|-------|
| `agreement_id` (`RepoIsi` / `ReverseRepoIsi`) | `OblgtnId` | Репо шартының идентификаторы |
| Кепіл ауыстыру Tx идентификаторы | `TxId` | Ауыстыру үшін жасалған |
| Кепілзаттың бастапқы мөлшері | `Substitution/OriginalAmt` | Ауыстыру алдында кепілге қойылған матчтар |
| Бастапқы кепіл валютасы | `Substitution/OriginalCcy` | Валюта коды |
| Қамтамасыз ету сомасын алмастыру | `Substitution/SubstituteAmt` | Ауыстыру сомасы |
| Қамтамасыз ету валютасын алмастыру | `Substitution/SubstituteCcy` | Валюта коды |
| Күшіне ену күні (басқару маржасының кестесі) | `Substitution/EffectiveDt` | ISO күні (ЖЖЖЖ-АА-КК) |
| Шаш қию классификациясы | `Substitution/Type` | Қазіргі уақытта `FULL` немесе `PARTIAL` басқару саясатына негізделген |
| Басқару себебі / шаш қию жазбасы | `Substitution/ReasonCd` | Міндетті емес, басқару негіздемесін жүзеге асырады |
| Шаш қию өлшемі | `Substitution/Haircut` | Сандық; ауыстыру кезінде қолданылған шаш қиюын картаға түсіреді |
| Түпнұсқа/ауыстырмалы құрал идентификаторлары | `Substitution/OriginalFinInstrmId`, `Substitution/SubstituteFinInstrmId` | Әрбір аяқ үшін қосымша ISIN/CUSIP |

### Қаржыландыру және есептер

| Iroha контекст | ISO 20022 хабары | Картаның орналасуы |
|---------------------------------|-------------------|------------------|
| Репо қолма-қол аяқ тұтану / босату | `pacs.009` | `IntrBkSttlmAmt`, `IntrBkSttlmCcy`, `IntrBkSttlmDt`, `InstgAgt`, `InstdAgt` DvP/PvP аяқтарынан толтырылған |
| Есеп айырысудан кейінгі есептер | `camt.054` | `Ntfctn/Ntry[*]` астында жазылған төлем аяқтарының қозғалысы; көпір `SplmtryData` жүйесінде кітап/шот метадеректерін енгізеді |

### Пайдалану туралы ескертпелер* Барлық сомалар Norito сандық көмекшілері арқылы серияланады (`NumericSpec`)
  актив анықтамалары бойынша шкала сәйкестігін қамтамасыз ету.
* `TxId` мәндері `Max35Text` — UTF‑8 ұзындығын ≤35 таңбадан бұрын орындау
  ISO 20022 хабарларына экспорттау.
* BIC 8 немесе 11 бас әріптік-сандық таңба болуы керек (ISO9362); бас тарту
  Norito метадеректері, төлемдер немесе есеп айырысулар алдында бұл тексеруден өтпейді.
  растаулар.
* Тіркелгі идентификаторлары (AccountId / ChainId) қосымшаға экспортталады
  метадеректер, осылайша қабылдаушы қатысушылар өздерінің жергілікті бухгалтерлік кітаптарымен салыстыра алады.
* `SupplementaryData` канондық JSON болуы керек (UTF‑8, сұрыпталған кілттер, JSON-негізі
  қашу). SDK көмекшілері мұны қолтаңбаларды, телеметриялық хэштерді және ISO-ны қамтамасыз етеді
  пайдалы жүктеме мұрағаттары қайта құру кезінде детерминистік болып қалады.
* Валюта сомалары ISO4217 бөлшек сандарына сәйкес келеді (мысалы, JPY-де 0 бар
  ондық бөлшектер, АҚШ долларында 2); көпір сәйкесінше Norito сандық дәлдігін бекітеді.
* CLI есеп айырысу көмекшілері (`iroha app settlement ... --atomicity ...`) енді шығарады
  Norito нұсқауларының орындалу жоспарлары 1:1 мен `Plan/ExecutionOrder` және
  `Plan/Atomicity` жоғарыда.
* ISO көмекшісі (`ivm::iso20022`) жоғарыда аталған өрістерді тексереді және қабылдамайды
  DvP/PvP аяқтары сандық ерекшеліктерді немесе контрагенттің өзара әрекетін бұзатын хабарлар.

### SDK құрастырушы көмекшілері

- JavaScript SDK енді `buildPacs008Message` көрсетеді /
  `buildPacs009Message` (`javascript/iroha_js/src/isoBridge.js` қараңыз) сондықтан клиент
  автоматтандыру құрылымдық есеп айырысу метадеректерін түрлендіре алады (BIC/LEI, IBAN,
  мақсат кодтары, қосымша Norito өрістері) детерминирленген пакеттерге XML
  осы нұсқаулықтағы карта жасау ережелерін қайта орындамай.
- Екі көмекші де анық `creationDateTime` (уақыт белдеуі бар ISO‑8601) қажет етеді.
  сондықтан операторлар оның орнына жұмыс үрдісінен детерминирленген уақыт белгісін жіберуі керек
  SDK әдепкі уақытына қабырға сағатына рұқсат беру.
- `recipes/iso_bridge_builder.mjs` сол көмекшілерді қалай қосу керектігін көрсетеді
  ортаның айнымалы мәндерін немесе JSON конфигурация файлдарын біріктіретін CLI, басып шығарады
  XML жасайды және оны міндетті түрде Torii (`ISO_SUBMIT=1`) жібереді, қайта пайдалану
  ISO көпір рецепті сияқты күту каденциясы.


### Анықтамалар

- LuxCSD / Clearstream ISO 20022 есеп айырысу мысалдары `SttlmTpAndAddtlParams/SctiesMvmntTp` (`DELI`/`RECE`) және `Pmt` көрсетеді (`APMT`/`FREE`).[1](https://www.luxcsd.com/resource/blob/3434074/6f8add4708407a4701055be4dd04846b/c23005-eis-examples-cbf-data.pdf)
- Есеп айырысу квалификацияларын қамтитын Clearstream DCP спецификациялары (`SttlmTxCond`, `PrtlSttlmInd`).[2](https://www.clearstream.com/clearstream-en/res-library/market-coverage/instruction-specifications-swift-iso-20022-dcp-mode-ceu-spain-2357008)
- SWIFT PMPG нұсқаулығы бағалы қағаздарға қатысты PvP қаржыландыру үшін `pacs.009` және `CtgyPurp/Cd = SECU` ұсынатын.[3](https://www.swift.com/swift-resource/251897/download)
- Идентификатор ұзындығы шектеулері үшін ISO 20022 хабар анықтамасының есептері (BIC, Max35Text).[4](https://www.iso20022.org/sites/default/files/2020-12/ISO20022_MDRPart2_ChangeOrVerifyAccountIdentification_2020_2021_v1_ForSEGReview.pdf)
- ISIN пішімі және бақылау сомасы ережелері бойынша ANNA DSB нұсқаулығы.[5](https://www.anna-dsb.com/isin/)

### Пайдалану бойынша кеңестер- LLM тексере алуы үшін әрқашан сәйкес Norito үзіндісін немесе CLI пәрменін қойыңыз.
  нақты өріс атаулары және Сандық масштабтар.
- Қағаз ізін сақтау үшін дәйексөздерді сұрау (`provide clause references`)
  сәйкестік және аудиторлық тексеру.
- `docs/source/finance/settlement_iso_mapping.md` ішінде жауаптың қысқаша мазмұнын түсіріңіз
  (немесе байланыстырылған қосымшалар), сондықтан болашақ инженерлерге сұрауды қайталау қажет емес.

## Оқиғаға тапсырыс беру кітаптары (ISO 20022 ↔ Norito Bridge)

### А сценарийі — Кепілді ауыстыру (Репо/Кепіл)

**Қатысушылар:** кепіл беруші/алушы (және/немесе агенттер), кастодиан(лар), CSD/T2S  
**Уақыт:** нарықтық үзілістерге және T2S күндізгі/түнгі циклдерге; екі аяқты бір реттеу терезесінде аяқтайтындай етіп реттеңіз.

#### Хабарлама хореографиясы
1. `colr.010` Кепілді ауыстыру туралы сұрау → кепіл беруші/алушы немесе агент.  
2. `colr.011` Кепілді ауыстыру жауабы → қабылдау/қабылдамау (қабылдамау себебі).  
3. `colr.012` Кепілді ауыстыруды растау → ауыстыру келісімін растайды.  
4. `sese.023` нұсқаулары (екі аяқ):  
   - Бастапқы кепілді қайтару (`SctiesMvmntTp=DELI`, `Pmt=FREE`, `SctiesTxTp=COLO`).  
   - Ауыстыратын кепілді жеткізу (`SctiesMvmntTp=RECE`, `Pmt=FREE`, `SctiesTxTp=COLI`).  
   Жұпты байланыстырыңыз (төменде қараңыз).  
5. `sese.024` күй кеңестері (қабылданды, сәйкестендірілді, күтуде, орындалмады, қабылданбады).  
6. Брондау жасалғаннан кейін `sese.025` растаулары.  
7. Қосымша қолма-қол ақша дельтасы (алымдар/шаш қию) → `pacs.009` `CtgyPurp/Cd = SECU` арқылы FI-to-FI Credit Transfer; `pacs.002` арқылы күй, `pacs.004` арқылы қайтарылады.

#### Міндетті растаулар / күйлер
- Тасымалдау деңгейі: шлюздер `admi.007` шығаруы немесе бизнес өңдеуден бұрын бас тартуы мүмкін.  
- Есеп айырысу өмірлік циклі: `sese.024` (өңдеу күйлері + себеп кодтары), `sese.025` (соңғы).  
- Қолма-қол ақша жағы: `pacs.002` (`PDNG`, `ACSC`, `RJCT` т.б.), қайтару үшін `pacs.004`.

#### Шарттылық / өрістерді босату
- `SctiesSttlmTxInstr/Lnkgs` (`WITH`/`BEFO`/`AFTE`) екі нұсқаулықты тізбектеу үшін.  
- `SttlmParams/HldInd` критерийлер орындалғанша ұстау; `sese.030` (`sese.031` күйі) арқылы шығарыңыз.  
- `SttlmParams/PrtlSttlmInd` ішінара есептесуді бақылау үшін (`NPAR`, `PART`, `PARC`, `PARQ`).  
- `SttlmParams/SttlmTxCond/Cd` нарыққа тән жағдайлар үшін (`NOMC`, т.б.).  
- Қолдау көрсетілген кезде қосымша T2S шартты бағалы қағаздарды жеткізу (CoSD) ережелері.

#### Әдебиеттер
- SWIFT кепілді басқару MDR (`colr.010/011/012`).  
- Байланыс пен күйлерге арналған CSD/T2S пайдалану нұсқаулықтары (мысалы, DNB, ECB Insights).  
- SMPG есеп айырысу тәжірибесі, Clearstream DCP нұсқаулықтары, ASX ISO семинарлары.

### B сценарийі — FX терезесінің бұзылуы (PvP қаржыландырудың сәтсіздігі)

**Қатысушылар:** контрагенттер мен касса агенттері, бағалы қағаздардың кастодианы, CSD/T2S  
**Уақыт:** FX PvP терезелері (CLS/екі жақты) және CSD үзінділері; бағалы қағаздарды қолма-қол ақшаны растауды күтуде ұстау.#### Хабарлама хореографиясы
1. `CtgyPurp/Cd = SECU` валютасына `pacs.009` FI-to-FI Credit Transfer; `pacs.002` арқылы күй; `camt.056`/`camt.029` арқылы қайтарып алу/бас тарту; егер шешілсе, `pacs.004` қайтарады.  
2. `sese.023` DvP нұсқау(лар)ы `HldInd=true` бар, осылайша бағалы қағаздар бөлігі қолма-қол ақшаны растауды күтеді.  
3. `sese.024` өмірлік циклі туралы ескертулер (қабылданған/сәйкестендірілген/күтуде).  
4. Терезе біткенше `pacs.009` екі аяғы да `ACSC` жетсе → `sese.030` → `sese.031` (мод күйі) → `sese.025` (растау) арқылы шығарыңыз.  
5. FX терезесі бұзылса → қолма-қол ақшаны жою/қайтарып алу (`camt.056/029` немесе `pacs.004`) және бағалы қағаздардан бас тарту (`sese.020` + `sese.027` немесе Norito ережелері расталған болса).

#### Міндетті растаулар / күйлер
- Қолма-қол ақша: қайтару үшін `pacs.002` (`PDNG`, `ACSC`, `RJCT`), `pacs.004`.  
- Бағалы қағаздар: `sese.024` (`NORE`, `ADEA` сияқты күтудегі/сәтсіз себептер), `sese.025`.  
- Тасымалдау: `admi.007` / шлюз бизнес өңдеуден бұрын қабылдамайды.

#### Шарттылық / өрістерді босату
- `SttlmParams/HldInd` + `sese.030` сәтті/сәтсіз жағдайда шығарылым/бас тарту.  
- `Lnkgs` бағалы қағаздар бойынша нұсқауларды кассаның аяғына байлау үшін.  
- Шартты жеткізуді пайдаланған кезде T2S CoSD ережесі.  
- күтпеген бөліктерді болдырмау үшін `PrtlSttlmInd`.  
- `pacs.009`, `CtgyPurp/Cd = SECU` бойынша бағалы қағаздарға қатысты қаржыландыруды белгілейді.

#### Әдебиеттер
- Бағалы қағаздар процестеріндегі төлемдерге арналған PMPG / CBPR+ нұсқауы.  
- SMPG есеп айырысу тәжірибесі, байланыстыру/ұстау туралы T2S түсініктері.  
- Clearstream DCP нұсқаулықтары, техникалық қызмет көрсету хабарламаларына арналған ECMS құжаттамасы.

### pacs.004 салыстыру жазбаларын қайтарады

- қайтару құрылғылары енді `ChrgBr` (`DEBT`/`CRED`/`SHAR`/`SLEV`) қалыпқа келтіреді және жеке қайтару себептері Norito сияқты қайталанады XML конвертін қайта талдаусыз атрибуция және оператор кодтары.
- `DataPDU` конверттеріндегі AppHdr қолтаңба блоктары қабылдау кезінде еленбейді; аудит кірістірілген XMLDSIG өрістерінен гөрі арнаның шығу тегіне сүйенуі керек.

### Көпірді пайдалануды бақылау парағы
- Жоғарыдағы хореографияны орындау (кепілдеме: `colr.010/011/012 → sese.023/024/025`; FX бұзу: `pacs.009 (+pacs.002) → sese.023 held → release/cancel`).  
- `sese.024`/`sese.025` күйлерін және `pacs.002` нәтижелерін қақпа сигналдары ретінде қарастырыңыз; `ACSC` босатуды іске қосады, `RJCT` босатуға мәжбүр етеді.  
- `HldInd`, `Lnkgs`, `PrtlSttlmInd`, `SttlmTxCond` және қосымша CoSD ережелері арқылы шартты жеткізуді кодтау.  
- Қажет болғанда сыртқы идентификаторларды (мысалы, `pacs.009` үшін UETR) корреляциялау үшін `SupplementaryData` пайдаланыңыз.  
- нарықтық күнтізбе/кесінділер бойынша ұстау/босату уақытын параметрлеу; жою мерзіміне дейін `sese.030`/`camt.056` шығарыңыз, қажет болған жағдайда қайтаруларға қайта оралыңыз.

### ISO 20022 пайдалы жүктемелерінің үлгісі (аннотацияланған)

#### Нұсқаулық байланысы бар кепілді ауыстыру жұбы (`sese.023`)

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
```Байланыстырылған нұсқауды `SUBST-2025-04-001-B` (ауыстырмалы кепілді алу) `SctiesMvmntTp=RECE`, `Pmt=FREE` және `SUBST-2025-04-001-A` кері нұсқайтын `WITH` сілтемесін жіберіңіз. Ауыстыру мақұлданғаннан кейін екі аяқты сәйкес `sese.030` арқылы босатыңыз.

#### Бағалы қағаздардың тоқтатылуы FX растауын күтуде (`sese.023` + `sese.030`)

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
```

`pacs.009` аяғы `ACSC` жеткенде босатыңыз:

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

`sese.031` ұстау шығарылымын растайды, содан кейін бағалы қағаздардың аяғы брондалғаннан кейін `sese.025`.

#### PvP қаржыландыру кезеңі (бағалы қағаздар мақсатымен `pacs.009`)

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

`pacs.002` төлем күйін бақылайды (`ACSC` = расталды, `RJCT` = қабылдамау). Терезе бұзылса, `camt.056`/`camt.029` арқылы қайтарып алыңыз немесе есептелген қаражатты қайтару үшін `pacs.004` жіберіңіз.
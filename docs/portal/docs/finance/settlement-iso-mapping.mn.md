---
lang: mn
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

::: Каноник эх сурвалжийг анхаарна уу
:::

## Суурь ↔ ISO 20022 Талбайн зураглал

Энэ тэмдэглэл нь Iroha тооцооны зааврын хоорондох каноник зураглалыг агуулна
(`DvpIsi`, `PvpIsi`, репо барьцааны урсгалууд) болон хэрэгжүүлсэн ISO 20022 мессеж
гүүрээр. Энэ нь хэрэгжүүлсэн мессежийн шатыг тусгадаг
`crates/ivm/src/iso20022.rs` ба үйлдвэрлэхэд лавлагаа болдог
Norito ачааллыг баталгаажуулж байна.

### Лавлах өгөгдлийн бодлого (танигч болон баталгаажуулалт)

Энэ бодлого нь танигчийн сонголт, баталгаажуулалтын дүрэм, лавлагааны өгөгдлийг багцалдаг
Norito ↔ ISO 20022 гүүр нь мессеж гаргахаас өмнө биелүүлэх ёстой.

**ISO мессеж доторх зангуу цэгүүд:**
- **Багажийн танигч** → `delivery_leg.asset_definition_id` ↔ `SctiesLeg/FinInstrmId`
  (эсвэл түүнтэй адилтгах хэрэгслийн талбар).
- **Талууд / агентууд** → `DlvrgSttlmPties/Pty` ба `sese.*`-д зориулсан `RcvgSttlmPties/Pty`,
  эсвэл `pacs.009` дахь агентын бүтэц.
- ** Данс** → `…/Acct` хадгалалт/мөнгөний дансны элементүүд; дэвтэр дээрх толин тусгал
  `AccountId` `SupplementaryData`.
- **Өмчлөлийн таниулбарууд** → `…/OthrId`, `Tp/Prtry` болон толин тусгалтай
  `SupplementaryData`. Зохицуулалттай танигчийг хэзээ ч өмчлөлийн танигчаар сольж болохгүй.

#### Мессежийн бүлгээр таниулах сонголт

##### `sese.023` / `.024` / `.025` (үнэт цаасны төлбөр тооцоо)

- **Хэрэгсэл (`FinInstrmId`)**
  - Давуу эрхтэй: `…/ISIN` дор **ISIN**. Энэ нь CSDs / T2S-ийн стандарт танигч юм.[^anna]
  - Нөхцөл байдал:
    - **CUSIP** эсвэл ISO гадаад стандартаас тохируулсан `Tp/Cd`-тай `…/OthrId/Id` дор бусад NSIN
      кодын жагсаалт (жишээ нь, `CUSP`); Захиргааны үед гаргагчийг `Issr`-д оруулна.[^iso_mdr]
    - **Norito хөрөнгийн ID** өмчийн хувьд: `…/OthrId/Id`, `Tp/Prtry="NORITO_ASSET_ID"`, болон
      `SupplementaryData` дээр ижил утгыг бичнэ үү.
  - Нэмэлт тайлбарлагч: **CFI** (`ClssfctnTp`) болон **FISN** хялбар болгоход дэмжигддэг
    эвлэрэл.[^iso_cfi][^iso_fisn]
- ** Талууд (`DlvrgSttlmPties`, `RcvgSttlmPties`)**
  - Давуу эрхтэй: **BIC** (`AnyBIC/BICFI`, ISO 9362).[^swift_bic]
  - Буцах: **LEI** энд мессежийн хувилбар нь тусгайлан зориулсан LEI талбарыг харуулж байна; хэрэв
    байхгүй бол тодорхой `Prtry` шошготой өмчийн ID-г авч явах ба мета өгөгдөлд BIC-г оруулна уу.[^iso_cr]
- **Суурь суурьшсан газар / газар** → Байрны **MIC**, CSD-ийн **BIC**.[^iso_mic]

##### `colr.010` / `.011` / `.012` ба `colr.007` (барьцаа хөрөнгийн менежмент)

- `sese.*` (ISIN-ийг илүүд үздэг)-тэй ижил хэрэгслийн дүрмийг дагаж мөрдөөрэй.
- Талууд анхдагчаар **BIC** ашигладаг; **LEI** нь схемд илэрсэн тохиолдолд зөвшөөрөгддөг.[^swift_bic]
- Бэлэн мөнгөний дүн нь зөв бага нэгжтэй **ISO 4217** валютын кодыг ашиглах ёстой.[^iso_4217]

##### `pacs.009` / `camt.054` (PvP санхүүжилт ба мэдэгдэл)

- **Төлөөлөгчид (`InstgAgt`, `InstdAgt`, зээлдэгч/зээлдүүлэгч агентууд)** → **BIC** сонголттой
  Зөвшөөрөгдсөн тохиолдолд LEI.[^swift_bic]
- ** Данс **
  - Банк хоорондын: **BIC** болон дотоод дансны лавлагаагаар тодорхойлно.
  - Хэрэглэгчид хандсан мэдэгдлүүд (`camt.054`): байгаа үед **IBAN**-г оруулаад баталгаажуулна уу.
    (урт, улсын дүрэм, mod-97 шалгах нийлбэр).[^swift_iban]
- **Валют** → **ISO 4217** 3 үсэгтэй код, бага нэгжийн бөөрөнхийллийг хүндэтгэ.[^iso_4217]
- **Torii залгих** → `POST /v1/iso20022/pacs009`-ээр дамжуулан PvP санхүүжилтийн хэсгийг илгээх; гүүр
  нь `Purp=SECU` шаарддаг бөгөөд одоо лавлагаа өгөгдлийг тохируулах үед BIC явган хүний гарцыг мөрддөг.

#### Баталгаажуулах дүрмүүд (ялгарахаас өмнө хэрэглэнэ)

| Тодорхойлогч | Баталгаажуулах дүрэм | Тэмдэглэл |
|------------|-----------------|-------|
| **ISIN** | ISO 6166 Хавсралт C-д заасан Regex `^[A-Z]{2}[A-Z0-9]{9}[0-9]$` болон Luhn (mod-10) шалгах цифр | Гүүрний ялгаралтаас өмнө татгалзах; дээшээ баяжуулахыг илүүд үздэг.[^anna_luhn] |
| **CUSIP** | Regex `^[A-Z0-9]{9}$` ба модуль-10 2 жинтэй (тэмдэгтүүдийг цифрээр буулгах) | Зөвхөн ISIN ашиглах боломжгүй үед; ANNA/CUSIP явган хүний ​​гарцаар дамжуулан газрын зураг.[^cusip] |
| **LEI** | Regex `^[A-Z0-9]{18}[0-9]{2}$` ба mod-97 шалгах орон (ISO 17442) | Хүлээн авахаасаа өмнө GLEIF өдөр тутмын дельта файлуудыг баталгаажуулна уу.[^gleif] |
| **BIC** | Regex `^[A-Z]{4}[A-Z]{2}[A-Z0-9]{2}([A-Z0-9]{3})?$` | Нэмэлт салбар код (сүүлийн гурван тэмдэгт). RA файлуудын идэвхтэй статусыг баталгаажуулна уу.[^swift_bic] |
| **МИК** | ISO 10383 RA файлаас хөтлөх; Байршлыг идэвхтэй байлгах (`!` дуусгавар болгох туг байхгүй) | Ашиглалтаас хасагдсан MIC-уудыг ялгаруулахаас өмнө дарцагла.[^iso_mic] |
| **IBAN** | Улс орны онцлог урт, том үсэг, тоо, mod-97 = 1 | SWIFT-ээр хөтлөгдсөн бүртгэлийг ашиглах; бүтцийн хувьд хүчингүй IBAN-аас татгалзах.[^swift_iban] |
| **Өмчлөлийн данс/намын ID** | `Max35Text` (UTF-8, ≤35 тэмдэгт) таслагдсан хоосон зайтай | `GenericAccountIdentification1.Id` болон `PartyIdentification135.Othr/Id` талбаруудад хамаарна. 35 тэмдэгтээс хэтэрсэн оруулгуудаас татгалзаж, ачааллыг ISO схемд нийцүүлэн холбох. |
| **Прокси бүртгэлийн таниулбарууд** | Хоосон биш `Max2048Text` `…/Prxy/Id` дор `…/Prxy/Tp/{Cd,Prtry}` дээр нэмэлт төрлийн кодтой | Үндсэн IBAN-тай хамт хадгалагдсан; Баталгаажуулалт нь PvP rails-ийг тусгах прокси зохицуулагчийг (заавал биш төрлийн кодтой) хүлээн авахын зэрэгцээ IBAN шаарддаг. |
| **CFI** | Зургаан тэмдэгттэй код, ISO 10962 ангилал зүйг ашиглан том үсгээр бичнэ Нэмэлт баяжуулах; тэмдэгтүүд хэрэгслийн ангилалд таарч байгаа эсэхийг шалгаарай.[^iso_cfi] |
| **FISN** | 35 хүртэлх тэмдэгт, том үсэг, тоо, цэг таслал | Нэмэлт; ISO 18774 зааврын дагуу таслах/хэвийн болгох.[^iso_fisn] |
| **Валют** | ISO 4217 3 үсэгтэй код, жижиг нэгжээр тодорхойлогддог масштаб | Дүн нь зөвшөөрөгдсөн аравтын бутархай хүртэл дугуйрсан байх ёстой; Norito талд хэрэгжүүлэх.[^iso_4217] |

#### Явган хүний гарц болон мэдээллийн засвар үйлчилгээний үүрэг

- **ISIN ↔ Norito хөрөнгийн ID** болон **CUSIP ↔ ISIN** явган хүний гарцыг хадгалах. Шөнө бүр шинэчлээрэй
  ANNA/DSB хангамж ба хувилбар нь CI-ийн ашигладаг агшин зуурын агшинг хянадаг.[^anna_crosswalk]
- GLEIF олон нийтийн харилцааны файлуудаас **BIC ↔ LEI** зураглалыг шинэчилснээр гүүр
  шаардлагатай үед хоёуланг нь ялгаруулна.[^bic_lei]
- **MIC-ийн тодорхойлолтыг** гүүрний мета өгөгдлийн хажууд хадгалаарай, ингэснээр газар баталгаажуулалт хийх боломжтой болно
  RA файлууд өдрийн дундуур өөрчлөгдсөн ч гэсэн тодорхойлогддог.[^iso_mic]
- Аудитын гүүрний мета өгөгдөлд өгөгдлийн гарал үүслийг (цаг хугацааны тэмдэг + эх сурвалж) тэмдэглэнэ. -г үргэлжлүүл
  ялгаруулсан зааврын хажууд хормын хувилбарын танигч.
- Ачаалагдсан өгөгдлийн багц бүрийн хуулбарыг хадгалахын тулд `iso_bridge.reference_data.cache_dir`-г тохируулна уу
  гарал үүслийн мета өгөгдлийн хамт (хувилбар, эх сурвалж, цагийн тэмдэг, шалгах нийлбэр). Энэ нь аудиторуудад боломжийг олгодог
  болон дээд урсгалын агшин зуурын агшин зуурын зургуудыг эргүүлсний дараа ч гэсэн түүхэн хангамжийг ялгах операторууд.
- ISO явган хүний гарцын агшин зургийг `iroha_core::iso_bridge::reference_data` ашиглан залгисан.
  `iso_bridge.reference_data` тохиргооны блок (зам + шинэчлэх интервал). Хэмжигч
  `iso_reference_status`, `iso_reference_age_seconds`, `iso_reference_records`, болон
  `iso_reference_refresh_interval_secs` анхааруулга өгөхийн тулд ажиллах үеийн эрүүл мэндийг харуулна. Torii
  bridge нь тохируулсан дээр агент BIC байхгүй `pacs.008` илгээлтээс татгалздаг
  Явган хүний гарц, эсрэг талын оролцогч байгаа үед `InvalidIdentifier` тодорхойлогч алдаа
  үл мэдэгдэх.【crates/iroha_torii/src/iso20022_bridge.rs#L1078】
- IBAN болон ISO 4217 холболтыг нэг давхаргад хэрэгжүүлдэг: pacs.008/pacs.009 одоо урсаж байна
  Өртэй/зээлдүүлэгчийн IBAN-д тохируулсан нэр байхгүй эсвэл `InvalidIdentifier` алдаа гаргадаг
  `currency_assets`-д төлбөр тооцооны валют алга болсон нь буруу гүүр үүсэхээс сэргийлж байна.
  дэвтэрт хүрэх заавар. IBAN баталгаажуулалт нь тухайн улс оронд мөн хамаарна
  ISO 7064 mod‑97-н өмнөх урт ба тоон шалгах цифрүүд нь бүтцийн хувьд хүчингүй
  утгуудаас эрт татгалзсан.【crates/iroha_torii/src/iso20022_bridge.rs#L775】【crates/iroha_torii/src/iso20022_bridge.rs#L827】【crates/ivm/src/iso2002】5.
- CLI суурингийн туслахууд ижил хамгаалалтын хашлагыг өвлөн авдаг: нэвтрүүлэх
  DvP-тэй байхын тулд `--iso-reference-crosswalk <path>` `--delivery-instrument-id`-ийн хамт
  `sese.023` XML агшин агшныг гаргахаас өмнө багажийн ID-г баталгаажуулна уу.【crates/iroha_cli/src/main.rs#L3752】
- `cargo xtask iso-bridge-lint` (болон CI боодол `ci/check_iso_reference_data.sh`) хөвөн
  явган хүний гарцын зураг болон бэхэлгээ. Энэ тушаал нь `--isin`, `--bic-lei`, `--mic`, болон
  `--fixtures` нь дарцаглаж, ажиллаж байх үед `fixtures/iso_bridge/` дээрх жишээ өгөгдлийн багц руу буцдаг.
  аргументгүйгээр.【xtask/src/main.rs#L146】【ci/check_iso_reference_data.sh#L1】
- IVM туслах одоо жинхэнэ ISO 20022 XML дугтуйг (толгой.001 + `DataPDU` + `Document`) залгиж байна.
  `head.001` схемээр дамжуулан Бизнесийн програмын толгой хэсгийг баталгаажуулж, `BizMsgIdr`,
  `MsgDefIdr`, `CreDt` болон BIC/ClrSysMmbId агентууд тодорхой хэмжээгээр хадгалагддаг; XMLDSig/XAdES
  блокуудыг санаатайгаар алгассан хэвээр байна. 

#### Зохицуулалт болон зах зээлийн бүтцийн талаар анхаарах зүйлс- **T+1 төлбөр тооцоо**: АНУ/Канадын хөрөнгийн зах зээл 2024 онд T+1 рүү шилжсэн; Norito тохируулна
  зохих хуваарь болон SLA анхааруулга.[^sec_t1][^csa_t1]
- **CSDR шийтгэл**: Төлбөр тооцооны сахилгын дүрэм нь мөнгөн торгуулийг хэрэгжүүлдэг; Norito баталгаажуулна уу
  мета өгөгдөл нь эвлэрүүлэхийн тулд торгуулийн лавлагааг авдаг.[^csdr]
- **Тухайн өдрийн төлбөр тооцооны нисгэгчид**: Энэтхэгийн зохицуулагч T0/T+0 төлбөр тооцоог үе шаттайгаар хийж байна; байлгах
  Нисгэгчид өргөжин тэлэхийн хэрээр гүүрний хуанли шинэчлэгдсэн.[^india_t0]
- **Барьцааны худалдан авалт / барьцаа**: Худалдан авах хугацаа болон нэмэлт барьцааны талаарх ESMA шинэчлэлтийг хянах
  Тиймээс нөхцөлт хүргэлт (`HldInd`) нь хамгийн сүүлийн үеийн удирдамжтай нийцдэг.[^csdr]

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

### Төлбөрийн эсрэг хүргэлт → `sese.023`

| DvP талбар | ISO 20022 зам | Тэмдэглэл |
|------------------------------------------------------|---------------------------------------|-------|
| `settlement_id` | `TxId` | Тогтвортой амьдралын мөчлөгийн тодорхойлогч |
| `delivery_leg.asset_definition_id` (аюулгүй байдал) | `SctiesLeg/FinInstrmId` | Каноник танигч (ISIN, CUSIP, …) |
| `delivery_leg.quantity` | `SctiesLeg/Qty` | Аравтын тоон мөр; хөрөнгийн нарийвчлалыг хүндэтгэдэг |
| `payment_leg.asset_definition_id` (валют) | `CashLeg/Ccy` | ISO валютын код |
| `payment_leg.quantity` | `CashLeg/Amt` | Аравтын тоон мөр; Тоон үзүүлэлтээр дугуйрсан |
| `delivery_leg.from` (худалдагч / хүргэлтийн тал) | `DlvrgSttlmPties/Pty/Bic` | Хүргэлтийн оролцогчийн BIC *(акаунтын каноник ID одоогоор мета өгөгдөлд экспортлогдсон)* |
| `delivery_leg.from` данс танигч | `DlvrgSttlmPties/Acct` | Чөлөөт хэлбэр; Norito мета өгөгдөл нь яг дансны ID |
| `delivery_leg.to` (худалдан авагч / хүлээн авагч тал) | `RcvgSttlmPties/Pty/Bic` | Хүлээн авагчийн BIC |
| `delivery_leg.to` данс танигч | `RcvgSttlmPties/Acct` | Чөлөөт хэлбэр; дансны ID хүлээн авагчтай таарч байна |
| `plan.order` | `Plan/ExecutionOrder` | Дугаар: `DELIVERY_THEN_PAYMENT` эсвэл `PAYMENT_THEN_DELIVERY` |
| `plan.atomicity` | `Plan/Atomicity` | Дугаар: `ALL_OR_NOTHING`, `COMMIT_FIRST_LEG`, `COMMIT_SECOND_LEG` |
| **Мессежийн зорилго** | `SttlmTpAndAddtlParams/SctiesMvmntTp` | `DELI` (хүргэх) эсвэл `RECE` (хүлээн авах); Өргөдөл гаргагч тал аль хөлийг гүйцэтгэхийг толин тусгал. |
|                                                        | `SttlmTpAndAddtlParams/Pmt` | `APMT` (төлбөрийн эсрэг) эсвэл `FREE` (төлбөргүй). |
| `delivery_leg.metadata`, `payment_leg.metadata` | `SctiesLeg/Metadata`, `CashLeg/Metadata` | UTF‑8 гэж кодлогдсон нэмэлт Norito JSON |

> **Төлбөр тооцооны шалгуур үзүүлэлт** – гүүр нь төлбөр тооцооны нөхцлийн код (`SttlmTxCond`), хэсэгчилсэн төлбөр тооцооны үзүүлэлт (`PrtlSttlmInd`) болон Norito мета өгөгдлийн бусад сонголтын шалгуур үзүүлэлтүүдийг байгаа үед I18NI000001 болгон хуулж зах зээлийн практикийг тусгадаг. ISO гадаад кодын жагсаалтад нийтлэгдсэн тооллогыг мөрдүүлснээр очих газрын CSD утгуудыг таньдаг.

### Төлбөрийн эсрэг төлбөрийн санхүүжилт → `pacs.009`

PvP зааварчилгааг санхүүжүүлдэг бэлэн мөнгөөр FI-to-FI кредит хэлбэрээр олгодог.
шилжүүлэг. Гүүр нь эдгээр төлбөрийг тэмдэглэснээр доод урсгалын системүүд таньдаг
тэд үнэт цаасны төлбөр тооцоог санхүүжүүлдэг.

| PvP санхүүжилтийн талбар | ISO 20022 зам | Тэмдэглэл |
|------------------------------------------------|----------------------------------------------------|-------|
| `primary_leg.quantity` / {хэмжээ, валют} | `IntrBkSttlmAmt` + `IntrBkSttlmCcy` | Санаачлагчаас хасагдсан дүн/валют. |
| Эсрэг талын төлөөлөгчийн танигч | `InstgAgt`, `InstdAgt` | Илгээх болон хүлээн авах агентуудын BIC/LEI. |
| Тооцооны зорилго | `CdtTrfTxInf/PmtTpInf/CtgyPurp/Cd` | Үнэт цаастай холбоотой PvP санхүүжилтийн хувьд `SECU` гэж тохируулсан. |
| Norito мета өгөгдөл (дансны ID, FX өгөгдөл) | `CdtTrfTxInf/SplmtryData` | AccountId, FX цагийн тэмдэг, гүйцэтгэлийн төлөвлөгөөний зөвлөмжийг бүрэн агуулсан. |
| Заавар тодорхойлогч / амьдралын мөчлөгийн холбоос | `CdtTrfTxInf/PmtId/InstrId`, `CdtTrfTxInf/RmtInf` | Norito `settlement_id`-тай таарч байгаа тул бэлэн мөнгөний хөл нь үнэт цаасны талтай тохирно. |

JavaScript SDK-ийн ISO гүүр нь анхдагчаар тохируулснаар энэ шаардлагад нийцдэг
`pacs.009` ангиллын зорилго нь `SECU`; Дуудлага хийгчид үүнийг өөр хүнээр дарж болно
үнэт цаасны бус зээлийн шилжүүлгийг гаргах үед хүчинтэй ISO код, гэхдээ хүчингүй
үнэт зүйлсийг өмнө нь үгүйсгэдэг.

Хэрэв дэд бүтэц нь үнэт цаасны тодорхой баталгаажуулалтыг шаарддаг бол гүүр
`sese.025` ялгарсаар байгаа боловч энэ баталгаа нь үнэт цаасны хөлийг тусгаж байна
PvP "зорилго" гэхээсээ илүү статус (жишээ нь, `ConfSts = ACCP`).

### Төлбөрийн эсрэг төлбөрийн баталгаажуулалт → `sese.025`

| PvP талбар | ISO 20022 зам | Тэмдэглэл |
|-----------------------------------------|--------------------------|-------|
| `settlement_id` | `TxId` | Тогтвортой амьдралын мөчлөгийн тодорхойлогч |
| `primary_leg.asset_definition_id` | `SttlmCcy` | Үндсэн хэсгийн валютын код |
| `primary_leg.quantity` | `SttlmAmt` | Санаачлагчийн хүргэсэн дүн |
| `counter_leg.asset_definition_id` | `AddtlInf` (JSON ачаалал) | Нэмэлт мэдээлэлд суулгагдсан эсрэг валютын код |
| `counter_leg.quantity` | `SttlmQty` | Тоолуурын дүн |
| `plan.order` | `Plan/ExecutionOrder` | DvP |-тэй ижил дугаарыг тохируулсан
| `plan.atomicity` | `Plan/Atomicity` | DvP |-тэй ижил дугаарыг тохируулсан
| `plan.atomicity` статус (`ConfSts`) | `ConfSts` | `ACCP` тохирох үед; гүүр нь татгалзсан үед алдааны кодыг гаргадаг |
| Эсрэг талын танигч | `AddtlInf` JSON | Одоогийн гүүр нь мета өгөгдөлд бүрэн AccountId/BIC суулгацуудыг цуваа болгож байна |

### Репо барьцааны орлуулалт → `colr.007`

| Репо талбар / контекст | ISO 20022 зам | Тэмдэглэл |
|------------------------------------------------|----------------------------------|-------|
| `agreement_id` (`RepoIsi` / `ReverseRepoIsi`) | `OblgtnId` | Репо гэрээний тодорхойлогч |
| Барьцаа орлуулалт Tx тодорхойлогч | `TxId` | Сэлгээгээр үүсгэгдсэн |
| Анхны барьцаа хөрөнгийн хэмжээ | `Substitution/OriginalAmt` | Сэлгээний өмнө барьцаанд тавьсан тоглолт |
| Анхны барьцаа мөнгөн тэмдэгт | `Substitution/OriginalCcy` | Валютын код |
| Барьцаа хөрөнгийн хэмжээг орлуулах | `Substitution/SubstituteAmt` | Орлуулах хэмжээ |
| Барьцааны валютыг орлуулах | `Substitution/SubstituteCcy` | Валютын код |
| Хүчин төгөлдөр болох огноо (засаглалын маржин хуваарь) | `Substitution/EffectiveDt` | ISO огноо (YYYY-MM-DD) |
| Үс засах ангилал | `Substitution/Type` | Одоогийн байдлаар `FULL` эсвэл `PARTIAL` засаглалын бодлогод тулгуурлан |
| Засаглалын шалтгаан / үс засах тэмдэглэл | `Substitution/ReasonCd` | Сонголттой, засаглалын үндэслэлтэй |

### Санхүүжилт ба мэдэгдэл

| Iroha контекст | ISO 20022 мессеж | Газрын зургийн байршил |
|---------------------------------|-------------------|------------------|
| Репо бэлэн мөнгөний хөл гал асаах / тайлах | `pacs.009` | `IntrBkSttlmAmt`, `IntrBkSttlmCcy`, `IntrBkSttlmDt`, `InstgAgt`, `InstdAgt` DvP/PvP хөлөөс хүн амтай |
| Төлбөр тооцооны дараах тайлан | `camt.054` | `Ntfctn/Ntry[*]` дор бүртгэгдсэн төлбөрийн хөлийн хөдөлгөөн; гүүр нь `SplmtryData`-д дэвтэр/дансны мета өгөгдлийг оруулдаг |

### Хэрэглээний тэмдэглэл* Бүх дүнг Norito тоон туслах (`NumericSpec`) ашиглан цуваа болгосон.
  хөрөнгийн тодорхойлолтын дагуу масштабын нийцлийг хангах.
* `TxId` утгууд нь `Max35Text` - UTF‑8 урт ≤35 тэмдэгтээс өмнө хэрэгжүүлэх
  ISO 20022 мессеж рүү экспорт хийж байна.
* BIC нь 8 эсвэл 11 том үсэг тоон тэмдэгт байх ёстой (ISO9362); татгалзах
  Norito мета өгөгдөл нь төлбөр эсвэл тооцоо хийхээс өмнө энэ шалгалтыг бүтэлгүйтсэн
  баталгаажуулалт.
* Дансны таниулбаруудыг (AccountId / ChainId) нэмэлт болгон экспортлодог
  мета өгөгдөл нь хүлээн авч буй оролцогчид орон нутгийн дэвтэртэйгээ эвлэрэх боломжтой.
* `SupplementaryData` каноник JSON байх ёстой (UTF‑8, эрэмбэлэгдсэн түлхүүрүүд, JSON-уугуул
  зугтах). SDK-ийн туслахууд үүнийг гарын үсэг, телеметрийн хэш болон ISO зэрэгт хэрэгжүүлдэг
  ачааны архив нь дахин бүтээх явцад тодорхойлогддог хэвээр байна.
* Валютын дүн нь ISO4217 бутархай цифрүүдийг дагаж мөрддөг (жишээлбэл, JPY нь 0 байна
  аравтын бутархай, доллар нь 2); гүүр нь Norito тоон нарийвчлалын дагуу хавчаарлана.
* CLI тооцооны туслахууд (`iroha app settlement ... --atomicity ...`) одоо ялгаруулж байна
  Norito заавар, тэдгээрийн гүйцэтгэлийн төлөвлөгөө нь 1:1-ээс `Plan/ExecutionOrder` болон
  Дээрх `Plan/Atomicity`.
* ISO туслагч (`ivm::iso20022`) дээр дурдсан талбаруудыг баталгаажуулж, татгалздаг
  DvP/PvP хөл нь тоон үзүүлэлт эсвэл эсрэг талын харилцан үйлчлэлийг зөрчсөн мессежүүд.

### SDK Builder Туслагчид

- JavaScript SDK нь одоо `buildPacs008Message` /
  `buildPacs009Message` (`javascript/iroha_js/src/isoBridge.js`-г үзнэ үү) тиймээс үйлчлүүлэгч
  автоматжуулалт нь бүтэцлэгдсэн тооцооны мета өгөгдлийг (BIC/LEI, IBAN,
  зорилгын кодууд, нэмэлт Norito талбарууд) -ийг детерминистик pacs XML болгон
  энэ гарын авлагаас зураглалын дүрмийг дахин хэрэгжүүлэхгүйгээр.
- Туслагч хоёуланд нь тодорхой `creationDateTime` (цагийн бүстэй ISO‑8601) шаардлагатай.
  тиймээс операторууд ажлын урсгалаасаа тодорхой хугацааны тэмдэгт оруулах ёстой
  SDK-г ханын цагийг анхдагч болгох.
- `recipes/iso_bridge_builder.mjs` нь тэдгээр туслахуудыг хэрхэн холбохыг харуулж байна
  орчны хувьсагч эсвэл JSON тохиргооны файлуудыг нэгтгэдэг CLI нь
  XML-г үүсгэсэн бөгөөд сонголтоор үүнийг Torii (`ISO_SUBMIT=1`) руу дахин ашиглах
  ISO гүүрний жортой ижил хүлээх хэмнэл.


### Лавлагаа

- `SttlmTpAndAddtlParams/SctiesMvmntTp` (`DELI`/`RECE`) болон `Pmt`-ийг харуулсан LuxCSD / Clearstream ISO 20022 тооцооны жишээ (`APMT`/`FREE`).<sup>[1](https://www.luxcsd.com/resource/blob/3434074/6f8add4708407a4701055be4dd04846b/c23005-eis-examples-cbf-data.pdf)</sup>
- Төлбөрийн шалгуур үзүүлэлтүүдийг (`SttlmTxCond`, `PrtlSttlmInd`) хамарсан Clearstream DCP техникийн үзүүлэлтүүд.<sup>[2](https://www.clearstream.com/clearstream-en/res-library/market-coverage/instruction-specifications-swift-iso-20022-dcp-mode-ceu-spain-2357008)</sup>
- Үнэт цаастай холбоотой PvP санхүүжилтэд `CtgyPurp/Cd = SECU`-тэй `CtgyPurp/Cd = SECU`-ийг санал болгож буй SWIFT PMPG заавар.<sup>[3](https://www.swift.com/swift-resource/251897/download)</sup>
- Тодорхойлогчийн уртын хязгаарлалтын (BIC, Max35Text) ISO 20022 зурвасын тодорхойлолтын тайлан.<sup>[4](https://www.iso20022.org/sites/default/files/2020-12/ISO20022_MDRPart2_ChangeOrVerifyAccountIdentification_2020_2021_v1_ForSEGReview.pdf)</sup>
- ISIN формат болон шалгах нийлбэр дүрмийн талаархи ANNA DSB заавар.<sup>[5](https://www.anna-dsb.com/isin/)</sup>

### Хэрэглэх зөвлөмж

- Холбогдох Norito хэсэг эсвэл CLI командыг үргэлж буулгаж, LLM шалгаж болно.
  талбарын нарийн нэрс ба Тоон масштаб.
- Цаасан мөрийг хадгалахын тулд ишлэл (`provide clause references`) хүсэх.
  нийцлийн болон аудиторын хяналт.
- Хариултын хураангуйг `docs/source/finance/settlement_iso_mapping.md` дээр бичнэ үү
  (эсвэл холбосон хавсралтууд) тул ирээдүйн инженерүүд асуулга давтах шаардлагагүй болно.

## Үйл явдлын захиалгын сурах бичиг (ISO 20022 ↔ Norito Bridge)

### Хувилбар А — Барьцааны орлуулалт (Репо / Барьцаа)

**Оролцогчид:** барьцаалагч/хүлээн авагч (болон/эсвэл төлөөлөгч), кастодиан(ууд), CSD/T2S  
**Хугацаа:** зах зээлийн хязгаар ба T2S өдөр/шөнийн мөчлөг тус бүрээр; хоёр хөлийг нэг тооцооны цонхонд дуусгахын тулд зохион байгуул.

#### Мессеж бүжиг
1. `colr.010` Барьцаа хөрөнгө орлуулах хүсэлт → барьцаалагч/хүлээн авагч эсвэл төлөөлөгч.  
2. `colr.011` Барьцаа орлуулалтын хариу → зөвшөөрөх/татгалзах (заавал татгалзах шалтгаан).  
3. `colr.012` Барьцаа орлуулалтын баталгаажуулалт → орлуулах гэрээг баталгаажуулна.  
4. `sese.023` заавар (хоёр хөл):  
   - Анхны барьцааг буцаана (`SctiesMvmntTp=DELI`, `Pmt=FREE`, `SctiesTxTp=COLO`).  
   - Орлуулах барьцаа (`SctiesMvmntTp=RECE`, `Pmt=FREE`, `SctiesTxTp=COLI`) өгөх.  
   Хосыг холбоно уу (доороос үзнэ үү).  
5. `sese.024` статусын зөвлөгөө (хүлээн зөвшөөрсөн, таарсан, хүлээгдэж буй, бүтэлгүйтсэн, татгалзсан).  
6. Нэг удаа захиалсан `sese.025` баталгаажуулалт.  
7. Нэмэлт бэлэн мөнгөний дельта (төлбөр/үс засах) → `pacs.009` `CtgyPurp/Cd = SECU` бүхий FI-to-FI Credit Transfer; `pacs.002`-ээр дамжуулан статус, `pacs.004`-ээр буцаж ирдэг.

#### Шаардлагатай хүлээн зөвшөөрөл / статус
- Тээврийн түвшин: гарцууд нь `admi.007` ялгаруулж эсвэл бизнес боловсруулахаас өмнө татгалзаж болно.  
- Төлбөрийн амьдралын мөчлөг: `sese.024` (боловсруулах төлөв + шалтгаан код), `sese.025` (эцсийн).  
- Бэлэн мөнгөний тал: `pacs.002` (`PDNG`, `ACSC`, `RJCT` гэх мэт), `pacs.004` буцаах.

#### Нөхцөл байдал / талбайнуудыг тайлах
- `SctiesSttlmTxInstr/Lnkgs` (`WITH`/`BEFO`/`AFTE`) хоёр зааврыг холбоно.  
- `SttlmParams/HldInd` шалгуурыг хангах хүртэл барих; `sese.030` (`sese.031` статус) -аар дамжуулан гаргах.  
- Хэсэгчилсэн төлбөр тооцоог хянах `SttlmParams/PrtlSttlmInd` (`NPAR`, `PART`, `PARC`, `PARQ`).  
- `SttlmParams/SttlmTxCond/Cd` зах зээлийн тодорхой нөхцөлд (`NOMC` гэх мэт).  
- Дэмжигдсэн тохиолдолд нэмэлт T2S нөхцөлт үнэт цаас хүргэх (CoSD) дүрэм.

#### Лавлагаа
- SWIFT барьцааны менежмент MDR (`colr.010/011/012`).  
- Холболт болон төлөвт зориулсан CSD/T2S хэрэглээний гарын авлага (жишээ нь, DNB, ECB Insights).  
- SMPG тооцооны дадлага, Clearstream DCP гарын авлага, ASX ISO семинар.

### Хувилбар В — Валютын цонхны зөрчил (PvP санхүүжилтийн алдаа)

**Оролцогчид:** эсрэг талууд болон бэлэн мөнгөний төлөөлөгч, үнэт цаасны кастодиан, CSD/T2S  
**Хугацаа:** FX PvP цонхнууд (CLS/хоёр талын) болон CSD-ийн хязгаар; бэлэн мөнгөний баталгаажуулалт хүлээгдэж буй үнэт цаасны хөлийг хадгалах.

#### Мессеж бүжиг
1. `CtgyPurp/Cd = SECU`-тэй валют бүрт `pacs.009` FI-to-FI Credit Transfer; `pacs.002`-ээр дамжуулан статус; `camt.056`/`camt.029`-ээр дамжуулан эргүүлэн татах/цуцлах; Хэрэв аль хэдийн шийдсэн бол `pacs.004` буцаана.  
2. `sese.023` DvP заавар(ууд) нь `HldInd=true`-тэй тул үнэт цаасны хэсэг нь бэлэн мөнгөний баталгаажуулалтыг хүлээж байна.  
3. Амьдралын мөчлөгийн `sese.024` мэдэгдлүүд (хүлээн зөвшөөрсөн/тохирсон/хүлээгдэж байна).  
4. Хэрэв цонх дуусахаас өмнө `pacs.009` хөл хоёулаа `ACSC` хүрвэл → `sese.030` → `sese.031` (модын төлөв) → `sese.025` (баталгаажуулалт) -аар гаргана уу.  
5. Хэрэв валютын цонхыг зөрчсөн бол → бэлэн мөнгийг цуцлах/ эргүүлэн татах (`camt.056/029` эсвэл `pacs.004`) болон үнэт цаасыг цуцлах (`sese.020` + `sese.027`, эсвэл I18NI00000 зах зээл дээр аль хэдийн батлагдсан бол I18NI0000019).

#### Шаардлагатай хүлээн зөвшөөрөл / статус
- Бэлэн мөнгө: `pacs.002` (`PDNG`, `ACSC`, `RJCT`), `pacs.004` буцаах.  
- Үнэт цаас: `sese.024` (`NORE`, `ADEA` гэх мэт хүлээгдэж буй/ бүтэлгүйтсэн шалтгаанууд), `sese.025`.  
- Тээвэрлэлт: `admi.007` / гарцыг бизнес боловсруулахаас өмнө татгалздаг.

#### Нөхцөл байдал / талбайнуудыг тайлах
- `SttlmParams/HldInd` + `sese.030` амжилт/бүтэлгүйтлийн үед гаргах/цуцлах.  
- `Lnkgs` үнэт цаасны зааварчилгааг бэлэн мөнгөний хөлд уях.  
- Хэрэв нөхцөлт хүргэлтийг ашиглаж байгаа бол T2S CoSD дүрэм.  
- `PrtlSttlmInd`, санамсаргүй хэсэгчилсэн зүйлээс урьдчилан сэргийлэх.  
- `pacs.009`, `CtgyPurp/Cd = SECU` дээр үнэт цаастай холбоотой санхүүжилтийг тэмдэглэдэг.

#### Лавлагаа
- Үнэт цаасны үйл явцад төлбөр хийх PMPG / CBPR+ удирдамж.  
- SMPG төлбөр тооцооны практик, T2S холболт/барьцааны талаарх ойлголт.  
- Clearstream DCP гарын авлага, засвар үйлчилгээний мессежийн ECMS баримт бичиг.

### пакс.004 зураглалын тэмдэглэлийг буцаана

- return fixtures now normalise `ChrgBr` (`DEBT`/`CRED`/`SHAR`/`SLEV`) and proprietary return reasons exposed as `TxInf[*]/RtrdRsn/Prtry`, so bridge consumers can replay fee XML дугтуйг дахин задлахгүйгээр аттрибут болон операторын кодууд.
- `DataPDU` дугтуйн доторх AppHdr гарын үсгийн блокуудыг залгихад үл тоомсорлодог; Аудит нь суулгагдсан XMLDSIG талбараас илүү сувгийн гарал үүсэлтэй байх ёстой.

### Гүүрний ашиглалтын хяналтын хуудас
- Дээрх бүжиг дэглээчийг мөрдүүлэх (барьцаа хөрөнгө: `colr.010/011/012 → sese.023/024/025`; FX зөрчсөн: `pacs.009 (+pacs.002) → sese.023 held → release/cancel`).  
- `sese.024`/`sese.025` төлөв ба `pacs.002` үр дүнг гарцын дохио гэж үзэх; `ACSC` суллахыг идэвхжүүлж, `RJCT` хүчийг сулруулна.  
- `HldInd`, `Lnkgs`, `PrtlSttlmInd`, `SttlmTxCond` болон нэмэлт CoSD дүрмээр дамжуулан нөхцөлт хүргэлтийг кодлох.  
- Шаардлагатай үед гадаад ID-г (жишээ нь, `pacs.009`-ийн UETR) хамааруулахын тулд `SupplementaryData` ашиглана уу.  
- Зах зээлийн хуанли/тайлбараар бариулах/тайлах хугацааг тохируулах; Цуцлах эцсийн хугацаанаас өмнө `sese.030`/`camt.056`-г гаргаж, шаардлагатай үед буцаан олго.

### ISO 20022 ачааллын жишээ (Тэмдэглэгээтэй)

#### Барьцаа орлуулах хос (`sese.023`) зааврын холбоостой

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

Холбогдсон зааврыг `SUBST-2025-04-001-B` (Орлуулах барьцааны FoP хүлээн авах) `SctiesMvmntTp=RECE`, `Pmt=FREE`, `WITH` холбоосыг `SUBST-2025-04-001-A` руу буцааж зааж өгнө үү. Орлуулахыг зөвшөөрсний дараа тохирох `sese.030`-ээр хоёр хөлөө суллана.

#### Үнэт цаасны ханш хүлээгдэж байна FX баталгаажуулалт (`sese.023` + `sese.030`)

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
````pacs.009` хоёр хөл `ACSC` хүрмэгц суллана:

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

`sese.031` барьцааны хувилбарыг баталгаажуулж, үнэт цаасны зах зээлийг захиалсаны дараа `sese.025`.

#### PvP санхүүжилтийн хэсэг (Үнэт цаасны зориулалттай `pacs.009`)

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

`pacs.002` төлбөрийн төлөвийг хянадаг (`ACSC` = батлагдсан, `RJCT` = татгалзах). Хэрэв цонх эвдэрсэн бол `camt.056`/`camt.029`-ээр дамжуулан эргүүлэн татах эсвэл `pacs.004` илгээж, төлбөр тооцоогоо буцаах боломжтой.
---
lang: uz
direction: ltr
source: docs/source/finance/settlement_iso_mapping.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d1f1005d6a273ab732a7c7a7adca349c17569fe2e2755b8daccf2186724044f8
source_last_modified: "2026-01-22T16:26:46.568382+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Hisob-kitob ↔ ISO 20022 dala xaritasi

Ushbu eslatma Iroha hisob-kitob ko'rsatmalari o'rtasidagi kanonik xaritalashni qamrab oladi
(`DvpIsi`, `PvpIsi`, repo garov oqimlari) va bajarilgan ISO 20022 xabarlari
ko'prik bo'ylab. U amalga oshirilgan xabar iskalasini aks ettiradi
`crates/ivm/src/iso20022.rs` va ishlab chiqarishda mos yozuvlar sifatida xizmat qiladi yoki
Norito foydali yuklarni tekshirish.

### Malumot maʼlumotlari siyosati (identifikatorlar va tasdiqlash)

Bu siyosat identifikator sozlamalari, tekshirish qoidalari va maʼlumotnoma maʼlumotlarini toʻplaydi
Norito ↔ ISO 20022 ko'prigi xabarlarni chiqarishdan oldin bajarilishi kerak bo'lgan majburiyatlar.

**ISO xabaridagi bog'lanish nuqtalari:**
- **Asbob identifikatorlari** → `delivery_leg.asset_definition_id` ↔ `SctiesLeg/FinInstrmId`
  (yoki shunga o'xshash asbob maydoni).
- **Tomonlar/agentlar** → `DlvrgSttlmPties/Pty` va `sese.*` uchun `RcvgSttlmPties/Pty`,
  yoki `pacs.009` da agent tuzilmalari.
- **Hisob-kitoblar** → `…/Acct` saqlovchi/naqd pul hisoblari uchun elementlar; daftarni aks ettiring
  `AccountId`, `SupplementaryData`.
- **Xususiy identifikatorlar** → `…/OthrId` va `Tp/Prtry` bilan aks ettirilgan
  `SupplementaryData`. Hech qachon tartibga solinadigan identifikatorlarni xususiy identifikatorlar bilan almashtirmang.

#### Xabarlar oilasi bo'yicha identifikator afzalligi

##### `sese.023` / `.024` / `.025` (qimmatli qog'ozlar bo'yicha hisob-kitob)

- **Asbob (`FinInstrmId`)**
  - Afzal: **ISIN** `…/ISIN` ostida. Bu CSD / T2S uchun kanonik identifikator.[^anna]
  - Qaytarilishlar:
    - **CUSIP** yoki `…/OthrId/Id` ostidagi boshqa NSIN, `Tp/Cd` tashqi ISO standartidan oʻrnatilgan
      kodlar ro'yxati (masalan, `CUSP`); Emitentni `Issr` ga qo‘shish majburiyati berilganda.[^iso_mdr]
    - **Norito aktiv identifikatori** mulkiy: `…/OthrId/Id`, `Tp/Prtry="NORITO_ASSET_ID"` va
      `SupplementaryData` da bir xil qiymatni yozing.
  - Majburiy emas deskriptorlar: **CFI** (`ClssfctnTp`) va **FISN** qulaylik uchun qo'llab-quvvatlanadi
    yarashtirish.[^iso_cfi][^iso_fisn]
- **Tomonlar (`DlvrgSttlmPties`, `RcvgSttlmPties`)**
  - Afzal: **BIC** (`AnyBIC/BICFI`, ISO 9362).[^swift_bic]
  - Qayta tiklash: **LEI**, bunda xabar versiyasi ajratilgan LEI maydonini ko'rsatadi; agar
    yoʻq boʻlsa, aniq `Prtry` yorliqlari boʻlgan xususiy identifikatorlarni olib yuring va metamaʼlumotlarga BIC kiriting.[^iso_cr]
- **Hisoblash joyi/joyi** → joy uchun **MIC** va CSD uchun **BIC**.[^iso_mic]

##### `colr.010` / `.011` / `.012` va `colr.007` (garovni boshqarish)

- `sese.*` bilan bir xil asbob qoidalariga rioya qiling (ISIN afzal).
- Tomonlar sukut bo'yicha **BIC** dan foydalanadilar; **LEI**, agar sxema ko‘rsatsa, qabul qilinadi.[^swift_bic]
- Naqd pul summalarida **ISO 4217** valyuta kodlari, kichik birliklari toʻgʻri boʻlishi kerak.[^iso_4217]

##### `pacs.009` / `camt.054` (PvP moliyalashtirish va bayonotlar)- **Agentlar (`InstgAgt`, `InstdAgt`, qarzdor/kreditor agentlari)** → **BIC** ixtiyoriy
  Ruxsat etilgan joylarda LEI.[^swift_bic]
- **Hisoblar**
  - Banklararo: **BIC** va ichki hisob ma'lumotnomalari bo'yicha aniqlash.
  - Mijoz uchun bayonotlar (`camt.054`): mavjud bo'lganda **IBAN** qo'shing va uni tasdiqlang
    (uzunlik, mamlakat qoidalari, mod-97 nazorat summasi).[^swift_iban]
- **Valyuta** → **ISO 4217** 3 harfli kod, kichik birlik yaxlitlashiga rioya qiling.[^iso_4217]
- **Torii yutish** → `POST /v1/iso20022/pacs009` orqali PvP moliyalashtirish oyoqlarini yuboring; ko'prik
  `Purp=SECU` ni talab qiladi va endi mos yozuvlar ma'lumotlari sozlanganda BIC piyodalar o'tish joylarini qo'llaydi.

#### Validatsiya qoidalari (emissiyadan oldin qo'llaniladi)

| Identifikator | Tasdiqlash qoidasi | Eslatmalar |
|------------|-----------------|-------|
| **ISIN** | Regex `^[A-Z]{2}[A-Z0-9]{9}[0-9]$` va Luhn (mod-10) tekshiruv raqami ISO 6166 C ilovasiga muvofiq | Ko'prik emissiyasidan oldin rad etish; yuqori oqim boyitish afzal.[^anna_luhn] |
| **CUSIP** | Regex `^[A-Z0-9]{9}$` va modul-10 2 ta vaznga ega (belgilar raqamlarga xaritasi) | Faqat ISIN mavjud bo'lmaganda; bir marta manbadan olingan ANNA/CUSIP piyodalar o'tish joyi orqali xarita.[^cusip] |
| **LEI** | Regex `^[A-Z0-9]{18}[0-9]{2}$` va mod-97 tekshirish raqami (ISO 17442) | Qabul qilishdan oldin GLEIF kunlik delta fayllari bilan tasdiqlang.[^gleif] |
| **BIC** | Regex `^[A-Z]{4}[A-Z]{2}[A-Z0-9]{2}([A-Z0-9]{3})?$` | Ixtiyoriy filial kodi (oxirgi uchta belgi). RA fayllaridagi faol holatni tasdiqlang.[^swift_bic] |
| **MIC** | ISO 10383 RA faylidan saqlang; joylar faol bo'lishini ta'minlash (`!` tugatish bayrog'i yo'q) | Emissiyadan oldin foydalanishdan chiqarilgan MIClarni belgilang.[^iso_mic] |
| **IBAN** | Mamlakatga xos uzunlik, bosh harflar, raqamli, mod-97 = 1 | SWIFT tomonidan yuritiladigan reestrdan foydalaning; tizimli ravishda yaroqsiz IBANlarni rad etish.[^swift_iban] |
| **Xususiy hisob/partiya identifikatorlari** | `Max35Text` (UTF-8, ≤35 ta belgi) kesilgan boʻshliq bilan | `GenericAccountIdentification1.Id` va `PartyIdentification135.Othr/Id` maydonlari uchun amal qiladi. 35 belgidan oshgan yozuvlarni rad eting, shuning uchun ko'prik yuklamalari ISO sxemalariga mos keladi. |
| **Proksi hisob identifikatorlari** | `…/Prxy/Id` ostida boʻsh boʻlmagan `Max2048Text`, `…/Prxy/Tp/{Cd,Prtry}` da ixtiyoriy turdagi kodlar | Asosiy IBAN bilan birga saqlanadi; PvP relslarini aks ettirish uchun proksi-dasturlarni (ixtiyoriy turdagi kodlar bilan) qabul qilishda tasdiqlash hali ham IBAN-larni talab qiladi. |
| **CFI** | Olti belgili kod, ISO 10962 taksonomiyasi yordamida katta harflar | Ixtiyoriy boyitish; belgilarning asbob sinfiga mos kelishiga ishonch hosil qiling.[^iso_cfi] |
| **FISN** | 35 tagacha belgi, katta harf-raqam va cheklangan tinish belgilari | Majburiy emas; ISO 18774 yoʻriqnomasi boʻyicha kesish/normallashtirish.[^iso_fisn] |
| **Valyuta** | ISO 4217 3 harfli kod, kichik birliklar bilan belgilanadigan shkala | Miqdorlar ruxsat etilgan o'nli kasrlargacha yaxlitlanishi kerak; Norito tomonida amalga oshirish.[^iso_4217] |

#### Piyodalar o'tish va ma'lumotlarni saqlash majburiyatlari- **ISIN ↔ Norito aktiv ID** va **CUSIP ↔ ISIN** piyodalar o‘tish joylarini saqlang. dan kechasi yangilang
  ANNA/DSB tasmasi va versiyasi CI tomonidan ishlatiladigan oniy tasvirlarni boshqaradi.[^anna_crosswalk]
- GLEIF jamoatchilik bilan aloqalar fayllaridan **BIC ↔ LEI** xaritalarini yangilang, shunda ko'prik
  kerak bo'lganda ikkalasini ham chiqaradi.[^bic_lei]
- **MIC ta'riflarini** ko'prik metama'lumotlari bilan birga saqlang, shuning uchun joyni tekshirish
  RA fayllari kun o'rtasida o'zgarganda ham deterministik.[^iso_mic]
- Audit uchun ko'prik metama'lumotlarida ma'lumotlarning kelib chiqishini (vaqt tamg'asi + manba) yozib oling. Davom eting
  snapshot identifikatori chiqarilgan ko'rsatmalar bilan birga.
- Har bir yuklangan maʼlumotlar toʻplamining nusxasini saqlab qolish uchun `iso_bridge.reference_data.cache_dir` ni sozlang
  kelib chiqish metama'lumotlari bilan bir qatorda (versiya, manba, vaqt tamg'asi, nazorat summasi). Bu auditorlarga imkon beradi
  va operatorlar tarixiy tasmalarni yuqori oqim snapshotlari aylantirilgandan keyin ham farqlash uchun.
- ISO piyodalar oʻtish joyining suratlari `iroha_core::iso_bridge::reference_data` tomonidan qabul qilinadi.
  `iso_bridge.reference_data` konfiguratsiya bloki (yo'llar + yangilash oralig'i). Ko'rsatkichlar
  `iso_reference_status`, `iso_reference_age_seconds`, `iso_reference_records` va
  `iso_reference_refresh_interval_secs` ogohlantirish uchun ish vaqti holatini ochib beradi. Torii
  BIC sozlanganda agenti BIC mavjud boʻlmagan `pacs.008` taqdimnomalarini koʻprik rad etadi.
  piyodalar o'tish joyi, qarama-qarshi tomon bo'lganida deterministik `InvalidIdentifier` xatolar yuzaga keladi
  noma'lum.【crates/iroha_torii/src/iso20022_bridge.rs#L1078】
- IBAN va ISO 4217 ulanishlari bir qavatda amalga oshiriladi: pacs.008/pacs.009 hozirda oqib chiqadi
  Qarzdor/kreditor IBANlarida sozlangan taxalluslar boʻlmasa yoki `InvalidIdentifier` xatolarini chiqaradi
  `currency_assets` dan hisob-kitob valyutasi yo'q, bu noto'g'ri shakllangan ko'prikning oldini oladi
  daftarga erishish bo'yicha ko'rsatmalar. IBAN tekshiruvi har bir mamlakat uchun ham amal qiladi
  ISO 7064 mod‑97 o‘tishidan oldingi uzunliklar va raqamli tekshirish raqamlari tizimli ravishda yaroqsiz
  qiymatlar erta rad etiladi.【crates/iroha_torii/src/iso20022_bridge.rs#L775】【crates/iroha_torii/src/iso20022_bridge.rs#L827】【crates/ivm/src/iso20012.
- CLI turar-joy yordamchilari bir xil himoya relslarini meros qilib olishadi: o'tish
  DvP ga ega bo'lish uchun `--iso-reference-crosswalk <path>` `--delivery-instrument-id` bilan birga
  `sese.023` XML snapshotini chiqarishdan oldin asbob identifikatorlarini tekshirish.【crates/iroha_cli/src/main.rs#L3752】
- `cargo xtask iso-bridge-lint` (va CI o'rami `ci/check_iso_reference_data.sh`) tuklar
  piyodalar o'tish joyining suratlari va moslamalari. Buyruq `--isin`, `--bic-lei`, `--mic` va
  `--fixtures` ishlayotganida bayroqchalar qo'yadi va `fixtures/iso_bridge/` da namunaviy ma'lumotlar to'plamiga qaytadi
  argumentlarsiz.【xtask/src/main.rs#L146】【ci/check_iso_reference_data.sh#L1】
- IVM yordamchisi endi haqiqiy ISO 20022 XML konvertlarini qabul qiladi (head.001 + `DataPDU` + `Document`)
  va Biznes ilovasi sarlavhasini `head.001` sxemasi orqali tasdiqlaydi, shuning uchun `BizMsgIdr`,
  `MsgDefIdr`, `CreDt` va BIC/ClrSysMmbId agentlari deterministik tarzda saqlanadi; XMLDSig/XAdES
  bloklar ataylab o'tkazib yuborilgan bo'lib qoladi. Regressiya testlari namunalarni va yangisini iste'mol qiladiXaritalarni himoya qilish uchun sarlavhali konvert moslamasi.【crates/ivm/src/iso20022.rs:265】【crates/ivm/src/iso20022.rs:3301】【crates/ivm/src/iso20022.rs:370

#### Normativ va bozor tuzilmasi bo'yicha mulohazalar

- **T+1 hisob-kitobi**: AQSh/Kanada qimmatli qog'ozlar bozorlari 2024 yilda T+1 darajasiga ko'tarildi; Norito sozlang
  rejalashtirish va tegishli SLA ogohlantirishlari.[^sec_t1][^csa_t1]
- **CSDR jarimalari**: hisob-kitob intizomi qoidalari naqd jarimalarni qo'llaydi; Norito ishonch hosil qiling
  metamaʼlumotlar yarashtirish uchun jarima havolalarini oladi.[^csdr]
- **Bir kunlik hisob-kitob uchuvchilari**: Hindiston regulyatori T0/T+0 hisob-kitobini bosqichma-bosqich oshirmoqda; ushlab turing
  ko'prik taqvimlari uchuvchilar kengayishi bilan yangilandi.[^india_t0]
- **Garoviy xaridlar / ushlab turish**: sotib olish vaqtlari va ixtiyoriy ushlab turish bo'yicha ESMA yangilanishlarini kuzatib boring
  shuning uchun shartli yetkazib berish (`HldInd`) soʻnggi yoʻriqnomaga mos keladi.[^csdr]

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

### Yetkazib berish-to'lovga qarshi → `sese.023`| DvP maydoni | ISO 20022 yoʻli | Eslatmalar |
|-------------------------------------------------------|---------------------------------------|-------|
| `settlement_id` | `TxId` | Barqaror hayot aylanishi identifikatori |
| `delivery_leg.asset_definition_id` (xavfsizlik) | `SctiesLeg/FinInstrmId` | Kanonik identifikator (ISIN, CUSIP, …) |
| `delivery_leg.quantity` | `SctiesLeg/Qty` | O'nlik qator; aktiv aniqligini hurmat qiladi |
| `payment_leg.asset_definition_id` (valyuta) | `CashLeg/Ccy` | ISO valyuta kodi |
| `payment_leg.quantity` | `CashLeg/Amt` | O'nlik qator; Raqamli belgi bo'yicha yaxlitlangan |
| `delivery_leg.from` (sotuvchi/etkazib beruvchi tomon) | `DlvrgSttlmPties/Pty/Bic` | Etkazib beruvchi ishtirokchining BIC ko'rsatkichi *(hisob qaydnomasi kanonik identifikatori hozirda metama'lumotlarda eksport qilingan)* |
| `delivery_leg.from` hisob identifikatori | `DlvrgSttlmPties/Acct` | Erkin shakl; Norito metama'lumotlari aniq hisob ID |ga ega
| `delivery_leg.to` (xaridor / qabul qiluvchi tomon) | `RcvgSttlmPties/Pty/Bic` | Qabul qiluvchi ishtirokchining BIC |
| `delivery_leg.to` hisob identifikatori | `RcvgSttlmPties/Acct` | Erkin shakl; qabul qiluvchi hisob identifikatoriga mos keladi |
| `plan.order` | `Plan/ExecutionOrder` | Raqam: `DELIVERY_THEN_PAYMENT` yoki `PAYMENT_THEN_DELIVERY` |
| `plan.atomicity` | `Plan/Atomicity` | Raqam: `ALL_OR_NOTHING`, `COMMIT_FIRST_LEG`, `COMMIT_SECOND_LEG` |
| **Xabar maqsadi** | `SttlmTpAndAddtlParams/SctiesMvmntTp` | `DELI` (etkazib berish) yoki `RECE` (qabul qilish); topshiruvchi tomon qaysi oyog'ini bajarayotganini aks ettiradi. |
|                                                        | `SttlmTpAndAddtlParams/Pmt` | `APMT` (to'lovga qarshi) yoki `FREE` (to'lovsiz). |
| `delivery_leg.metadata`, `payment_leg.metadata` | `SctiesLeg/Metadata`, `CashLeg/Metadata` | UTF‑8 sifatida kodlangan ixtiyoriy Norito JSON |

> **Hisob-kitob kvalifikatorlari** – ko‘prik hisob-kitob holati kodlarini (`SttlmTxCond`), qisman hisob-kitob ko‘rsatkichlarini (`PrtlSttlmInd`) va Norito metama’lumotlaridan boshqa ixtiyoriy kvalifikatsiya qiluvchilarni mavjud bo‘lganda Norito ga nusxalash orqali bozor amaliyotini aks ettiradi. Belgilangan CSD qiymatlarni tanib olishi uchun ISO tashqi kod ro'yxatlarida chop etilgan ro'yxatlarni amalga oshiring.

### Toʻlov va toʻlovni moliyalashtirish → `pacs.009`

PvP yo'riqnomasini moliyalashtiradigan naqd pul evaziga FI-to-FI krediti sifatida beriladi.
transferlar. Ko'prik ushbu to'lovlarni izohlaydi, shuning uchun quyi oqim tizimlari tan oladi
ular qimmatli qog'ozlar bo'yicha hisob-kitoblarni moliyalashtiradilar.| PvP moliyalashtirish sohasi | ISO 20022 yoʻli | Eslatmalar |
|------------------------------------------------|----------------------------------------------------|-------|
| `primary_leg.quantity` / {summa, valyuta} | `IntrBkSttlmAmt` + `IntrBkSttlmCcy` | Boshlovchidan debet qilingan summa/valyuta. |
| Qarama-qarshi tomon agenti identifikatorlari | `InstgAgt`, `InstdAgt` | Yuboruvchi va qabul qiluvchi agentlarning BIC/LEI. |
| Hisob-kitob maqsadi | `CdtTrfTxInf/PmtTpInf/CtgyPurp/Cd` | Qimmatli qog'ozlar bilan bog'liq PvP moliyalashtirish uchun `SECU` qiymatini o'rnating. |
| Norito metama'lumotlari (hisob identifikatorlari, FX ma'lumotlari) | `CdtTrfTxInf/SplmtryData` | To'liq AccountId, FX vaqt belgilari, ijro rejasi bo'yicha maslahatlar mavjud. |
| Yo'riqnoma identifikatori / hayot aylanishini bog'lash | `CdtTrfTxInf/PmtId/InstrId`, `CdtTrfTxInf/RmtInf` | Norito `settlement_id` ga mos keladi, shuning uchun pul oyog'i qimmatli qog'ozlar tomoni bilan mos keladi. |

JavaScript SDK ning ISO ko'prigi sukut bo'yicha ushbu talabga mos keladi
`pacs.009` toifasi maqsadi `SECU`; qo'ng'iroq qiluvchilar uni boshqasi bilan bekor qilishi mumkin
Qimmatli qog'ozlar bo'lmagan kredit o'tkazmalarini chiqarishda amaldagi ISO kodi, lekin yaroqsiz
qiymatlar oldindan rad etiladi.

Agar infratuzilma qimmatli qog'ozlarning aniq tasdiqlanishini talab qilsa, ko'prik
`sese.025` chiqarishda davom etmoqda, ammo bu tasdiq qimmatli qog'ozlar oyog'ini aks ettiradi.
PvP "maqsadini" emas, balki holati (masalan, `ConfSts = ACCP`).

### Toʻlovga qarshi toʻlovni tasdiqlash → `sese.025`

| PvP maydoni | ISO 20022 yoʻli | Eslatmalar |
|-----------------------------------------|--------------------------|-------|
| `settlement_id` | `TxId` | Barqaror hayot aylanishi identifikatori |
| `primary_leg.asset_definition_id` | `SttlmCcy` | Asosiy bosqich uchun valyuta kodi |
| `primary_leg.quantity` | `SttlmAmt` | Boshlovchi tomonidan yetkazib berilgan summa |
| `counter_leg.asset_definition_id` | `AddtlInf` (JSON foydali yuk) | Qo'shimcha ma'lumotlarga kiritilgan hisoblagich valyuta kodi |
| `counter_leg.quantity` | `SttlmQty` | Hisoblagich summasi |
| `plan.order` | `Plan/ExecutionOrder` | DvP | bilan bir xil raqam o'rnatilgan
| `plan.atomicity` | `Plan/Atomicity` | DvP | bilan bir xil raqam o'rnatilgan
| `plan.atomicity` holati (`ConfSts`) | `ConfSts` | Mos kelganda `ACCP`; ko'prik rad etishda xato kodlarini chiqaradi |
| Qarama-qarshi tomon identifikatorlari | `AddtlInf` JSON | Joriy ko'prik metama'lumotlarda to'liq AccountId/BIC kortejlarini ketma-ketlashtiradi |

Misol (ulanishlar, ushlab turish va bozor MIC bilan CLI ISO oldindan ko'rish):

```sh
iroha app settlement dvp \
  --settlement-id DVP-FIXTURE-1 \
  --delivery-asset security#equities \
  --delivery-quantity 500 \
  --delivery-from ih58... \
  --delivery-to ih58... \
  --payment-asset usd#fi \
  --payment-quantity 1050000 \
  --payment-from ih58... \
  --payment-to ih58... \
  --delivery-instrument-id US0378331005 \
  --place-of-settlement-mic XNAS \
  --partial-indicator npar \
  --hold-indicator \
  --settlement-condition NOMC \
  --linkage WITH:PACS009-CLS \
  --linkage BEFO:SUBST-PAIR-B \
  --iso-xml-out sese023_preview.xml
```

### Repo garovini almashtirish → `colr.007`| Repo maydoni / kontekst | ISO 20022 yoʻli | Eslatmalar |
|------------------------------------------------|----------------------------------|-------|
| `agreement_id` (`RepoIsi` / `ReverseRepoIsi`) | `OblgtnId` | Repo shartnomasi identifikatori |
| Garovni almashtirish Tx identifikatori | `TxId` | Har bir almashtirish uchun yaratilgan |
| Asl garov miqdori | `Substitution/OriginalAmt` | O'yinlar almashtirishdan oldin garovga qo'yilgan |
| Asl garov valyutasi | `Substitution/OriginalCcy` | Valyuta kodi |
| Garov miqdorini almashtiring | `Substitution/SubstituteAmt` | O'zgartirish miqdori |
| Garov valyutasini almashtiring | `Substitution/SubstituteCcy` | Valyuta kodi |
| Kuchga kirish sanasi (boshqaruv marjasi jadvali) | `Substitution/EffectiveDt` | ISO sanasi (YYYY-AA-DD) |
| Soch kesish tasnifi | `Substitution/Type` | Hozirda `FULL` yoki `PARTIAL` boshqaruv siyosatiga asoslangan |
| Boshqaruv sababi / soch kesilgan eslatma | `Substitution/ReasonCd` | Majburiy emas, boshqaruv asoslarini olib yuradi |
| Soch kesish o'lchami | `Substitution/Haircut` | Raqamli; almashtirish paytida qo'llaniladigan soch turmagini xaritalar |
| Asl/almashtiruvchi asbob identifikatorlari | `Substitution/OriginalFinInstrmId`, `Substitution/SubstituteFinInstrmId` | Har bir oyoq uchun ixtiyoriy ISIN/CUSIP |

### Moliyalashtirish va hisobotlar

| Iroha konteksti | ISO 20022 xabari | Joylashuvni xaritalash |
|---------------------------------|-------------------|------------------|
| Repo naqd oyoq ateşleme / bo'shatish | `pacs.009` | DvP/PvP oyoqlaridan to'ldirilgan `IntrBkSttlmAmt`, `IntrBkSttlmCcy`, `IntrBkSttlmDt`, `InstgAgt`, `InstdAgt` |
| Hisob-kitobdan keyingi hisobotlar | `camt.054` | `Ntfctn/Ntry[*]` ostida qayd etilgan to'lov oyog'ining harakatlari; ko'prik `SplmtryData` daftar/hisob metama'lumotlarini kiritadi |

### Foydalanish bo'yicha eslatmalar* Barcha summalar Norito raqamli yordamchilari (`NumericSpec`) yordamida ketma-ketlashtiriladi.
  aktiv ta'riflari bo'yicha miqyosga muvofiqligini ta'minlash.
* `TxId` qiymatlari `Max35Text` - UTF‑8 uzunligi ≤35 belgidan oldin belgilanishi kerak
  ISO 20022 xabarlariga eksport qilinmoqda.
* BIC 8 yoki 11 ta katta harf-raqamli belgilardan iborat bo'lishi kerak (ISO9362); rad qilish
  Norito metamaʼlumotlari toʻlovlar yoki hisob-kitoblarni amalga oshirishdan oldin bu tekshiruvdan oʻtmagan
  tasdiqlar.
* Hisob identifikatorlari (AccountId / ChainId) qo'shimcha sifatida eksport qilinadi
  metadata, shuning uchun qabul qiluvchi ishtirokchilar o'zlarining mahalliy daftarlari bilan yarashishlari mumkin.
* `SupplementaryData` kanonik JSON bo'lishi kerak (UTF‑8, tartiblangan kalitlar, JSON-native
  qochib ketish). SDK yordamchilari buni imzolar, telemetriya xeshlari va ISO bilan ta'minlaydi
  foydali yuk arxivlari qayta qurishda deterministik bo'lib qoladi.
* Valyuta summalari ISO4217 kasr raqamlariga mos keladi (masalan, JPY 0 ga ega
  o'nli kasrlar, USD 2 ga ega); ko'prik mos ravishda Norito raqamli aniqlikni qisadi.
* CLI hisob-kitob yordamchilari (`iroha app settlement ... --atomicity ...`) endi chiqaradi
  Norito ko'rsatmalarining bajarilishi rejalari 1:1 bilan `Plan/ExecutionOrder` va
  `Plan/Atomicity` yuqorida.
* ISO yordamchisi (`ivm::iso20022`) yuqorida sanab o'tilgan maydonlarni tasdiqlaydi va rad etadi
  DvP/PvP oyoqlari raqamli xususiyatlarni yoki kontragentning o'zaro munosabatini buzgan xabarlar.

### SDK Builder yordamchilari

- JavaScript SDK endi `buildPacs008Message` ni ochadi /
  `buildPacs009Message` (qarang `javascript/iroha_js/src/isoBridge.js`) shuning uchun mijoz
  avtomatlashtirish tuzilgan hisob-kitob metama'lumotlarini (BIC/LEI, IBAN,
  maqsad kodlari, qo'shimcha Norito maydonlari) XML deterministik paketlariga
  ushbu qo'llanmadagi xaritalash qoidalarini qayta bajarmasdan.
- Ikkala yordamchi ham aniq `creationDateTime` (soat mintaqasi bilan ISO‑8601) talab qiladi.
  shuning uchun operatorlar o'zlarining ish oqimidan deterministik vaqt tamg'asini o'tkazishlari kerak
  SDK-ni devor soatiga sukut bo'yicha qo'yish.
- `recipes/iso_bridge_builder.mjs` ushbu yordamchilarni qanday ulash kerakligini ko'rsatadi
  muhit o'zgaruvchilari yoki JSON konfiguratsiya fayllarini birlashtirgan CLI, chop etadi
  XML hosil qiladi va ixtiyoriy ravishda uni Torii (`ISO_SUBMIT=1`) ga yuboradi, qayta ishlatish
  ISO ko'prigi retsepti bilan bir xil kutish kadansi.


### Adabiyotlar

- LuxCSD / Clearstream ISO 20022 hisob-kitob namunalari `SttlmTpAndAddtlParams/SctiesMvmntTp` (`DELI`/`RECE`) va `Pmt` (`APMT`/`FREE`).[1](https://www.luxcsd.com/resource/blob/3434074/6f8add4708407a4701055be4dd04846b/c23005-eis-examples-cbf-data.pdf)
- Hisob-kitob kvalifikatorlarini qamrab oluvchi Clearstream DCP spetsifikatsiyalari (`SttlmTxCond`, `PrtlSttlmInd`).[2](https://www.clearstream.com/clearstream-en/res-library/market-coverage/instruction-specifications-swift-iso-20022-dcp-mode-ceu-spain-2357008)
- Qimmatli qog'ozlar bilan bog'liq PvP moliyalashtirish uchun `CtgyPurp/Cd = SECU` bilan `CtgyPurp/Cd = SECU`ni tavsiya qiluvchi SWIFT PMPG yo'riqnomasi.[3](https://www.swift.com/swift-resource/251897/download)
- Identifikator uzunligi cheklovlari uchun ISO 20022 xabar ta'rifi hisobotlari (BIC, Max35Text).[4](https://www.iso20022.org/sites/default/files/2020-12/ISO20022_MDRPart2_ChangeOrVerifyAccountIdentification_2020_2021_v1_ForSEGReview.pdf)
- ISIN formati va nazorat summasi qoidalari boʻyicha ANNA DSB yoʻriqnomasi.[5](https://www.anna-dsb.com/isin/)

### Foydalanish bo'yicha maslahatlar- LLM tekshira olishi uchun har doim tegishli Norito parchasi yoki CLI buyrug'ini joylashtiring
  aniq maydon nomlari va Raqamli masshtablar.
- Qog'oz izini saqlash uchun iqtiboslarni so'rang (`provide clause references`).
  muvofiqlik va auditor tekshiruvi.
- Javoblar xulosasini `docs/source/finance/settlement_iso_mapping.md` da yozib oling
  (yoki bog'langan qo'shimchalar) shuning uchun kelajakdagi muhandislar so'rovni takrorlashlari shart emas.

## Voqealarni buyurtma qilish kitoblari (ISO 20022 ↔ Norito Bridge)

### A stsenariysi - garovni almashtirish (repo / garov)

**Ishtirokchilar:** garov beruvchi/oluvchi (va/yoki agentlar), saqlovchi(lar), CSD/T2S  
**Vaqt:** bozor chegaralari va T2S kun/tun tsikllari uchun; Ikki oyoqni bir xil o'rnatish oynasida to'ldirish uchun orkestrlang.

#### Xabar xoreografiyasi
1. `colr.010` Garovni almashtirish so'rovi → garov beruvchi/oluvchi yoki agent.  
2. `colr.011` Garovni almashtirish javobi → qabul qilish/rad etish (ixtiyoriy rad etish sababi).  
3. `colr.012` Garovni almashtirishni tasdiqlash → almashtirish kelishuvini tasdiqlaydi.  
4. `sese.023` ko'rsatmalari (ikki oyoqli):  
   - Asl garovni qaytarish (`SctiesMvmntTp=DELI`, `Pmt=FREE`, `SctiesTxTp=COLO`).  
   - O'rinbosar garovni yetkazib berish (`SctiesMvmntTp=RECE`, `Pmt=FREE`, `SctiesTxTp=COLI`).  
   Juftlikni bog'lang (pastga qarang).  
5. `sese.024` holat bo'yicha maslahatlar (qabul qilingan, mos kelgan, kutilayotgan, bajarilmagan, rad etilgan).  
6. Bir marta band qilingan `sese.025` tasdiqlari.  
7. Ixtiyoriy naqd pul deltasi (toʻlov/soch kesish) → `pacs.009` `CtgyPurp/Cd = SECU` bilan FI-to-FI kredit oʻtkazmasi; holati `pacs.002` orqali, `pacs.004` orqali qaytadi.

#### Majburiy tasdiqlar / statuslar
- Transport darajasi: shlyuzlar `admi.007` chiqarishi yoki biznesni qayta ishlashdan oldin rad etishi mumkin.  
- Hisob-kitoblarning hayot aylanishi: `sese.024` (qayta ishlash holatlari + sabab kodlari), `sese.025` (yakuniy).  
- Naqd pul tomoni: `pacs.002` (`PDNG`, `ACSC`, `RJCT` va boshqalar), qaytarish uchun `pacs.004`.

#### Shartlilik / maydonlarni bo'shatish
- Ikkita ko'rsatmalarni zanjirlash uchun `SctiesSttlmTxInstr/Lnkgs` (`WITH`/`BEFO`/`AFTE`).  
- `SttlmParams/HldInd` mezonlar bajarilgunga qadar ushlab turish; `sese.030` (`sese.031` holati) orqali chiqaring.  
- `SttlmParams/PrtlSttlmInd` qisman hisob-kitoblarni nazorat qilish (`NPAR`, `PART`, `PARC`, `PARQ`).  
- `SttlmParams/SttlmTxCond/Cd` bozorga xos shartlar uchun (`NOMC` va boshqalar).  
- Qo'llab-quvvatlansa, ixtiyoriy T2S shartli qimmatli qog'ozlarni yetkazib berish (CoSD) qoidalari.

#### Adabiyotlar
- SWIFT garovini boshqarish MDR (`colr.010/011/012`).  
- Bog'lanish va statuslar uchun CSD/T2S foydalanish qo'llanmalari (masalan, DNB, ECB Insights).  
- SMPG hisob-kitob amaliyoti, Clearstream DCP qo'llanmalari, ASX ISO ustaxonalari.

### B stsenariysi - valyuta oynasining buzilishi (PvP moliyalashda muvaffaqiyatsizlik)

**Ishtirokchilar:** kontragentlar va kassa agentlari, qimmatli qog'ozlar saqlovchisi, CSD/T2S  
**Vaqt:** FX PvP oynalari (CLS/ikki tomonlama) va CSD chegaralari; naqd pulni tasdiqlagunga qadar qimmatli qog'ozlarni ushlab turish.#### Xabar xoreografiyasi
1. `CtgyPurp/Cd = SECU` bilan har bir valyuta uchun `pacs.009` FI-to-FI kredit o‘tkazmasi; holati `pacs.002` orqali; `camt.056`/`camt.029` orqali chaqirib olish/bekor qilish; agar allaqachon o'rnatilgan bo'lsa, `pacs.004` qaytish.  
2. `sese.023` DvP yo'riqnoma(lar)i `HldInd=true` bilan, shuning uchun qimmatli qog'ozlar oyog'i naqd pulni tasdiqlashni kutadi.  
3. Lifecycle `sese.024` bildirishnomalari (qabul qilingan/mos keldi/kutishda).  
4. Agar ikkala `pacs.009` oyoqlari oyna muddati tugaguncha `ACSC` ga yetsa → `sese.030` → `sese.031` (mod holati) → `sese.025` (tasdiqlash) bilan chiqaring.  
5. Agar valyuta oynasi buzilgan bo'lsa → naqd pulni bekor qilish/chaqirib olish (`camt.056/029` yoki `pacs.004`) va qimmatli qog'ozlarni bekor qilish (`sese.020` + `sese.027` yoki `sese.027`, yoki Norito qoidasi allaqachon tasdiqlangan bo'lsa).

#### Majburiy tasdiqlar / statuslar
- Naqd pul: `pacs.002` (`PDNG`, `ACSC`, `RJCT`), qaytarish uchun `pacs.004`.  
- Qimmatli qog'ozlar: `sese.024` (`NORE`, `ADEA` kabi kutilayotgan/muvaffaqiyatsiz sabablar), `sese.025`.  
- Transport: `admi.007` / shlyuz biznesni qayta ishlashdan oldin rad etadi.

#### Shartlilik / maydonlarni bo'shatish
- `SttlmParams/HldInd` + `sese.030` muvaffaqiyat/muvaffaqiyatsizlikda chiqarish/bekor qilish.  
- `Lnkgs` qimmatli qog'ozlar bo'yicha ko'rsatmalarni kassa oyog'iga bog'lash.  
- Agar shartli yetkazib berishdan foydalanilsa, T2S CoSD qoidasi.  
- `PrtlSttlmInd` ko'zda tutilmagan qismlarni oldini olish uchun.  
- `pacs.009`, `CtgyPurp/Cd = SECU` da qimmatli qog'ozlar bilan bog'liq moliyalashtirishni belgilab qo'yadi.

#### Adabiyotlar
- Qimmatli qog'ozlar jarayonlarida to'lovlar uchun PMPG / CBPR+ yo'riqnomasi.  
- SMPG hisob-kitob amaliyotlari, ulanish/ushlash bo'yicha T2S tushunchalari.  
- Clearstream DCP qo'llanmalari, texnik xizmat ko'rsatish xabarlari uchun ECMS hujjatlari.

### pacs.004 xaritalash qaydlarini qaytaradi

- Qaytish moslamalari endi `ChrgBr` (`DEBT`/`CRED`/`SHAR`/`SLEV`) va xususiy qaytarib berish sabablarini `ChrgBr` (`ChrgBr`) bilan normallashtiradi. XML konvertini qayta tahlil qilmasdan atribut va operator kodlari.
- `DataPDU` konvertlari ichidagi AppHdr imzo bloklari qabul qilinganda e'tiborga olinmaydi; audit o'rnatilgan XMLDSIG maydonlariga emas, balki kanal manbasiga tayanishi kerak.

### Ko'prik uchun operatsion nazorat ro'yxati
- Yuqoridagi xoreografiyaga rioya qiling (garov: `colr.010/011/012 → sese.023/024/025`; FX buzilishi: `pacs.009 (+pacs.002) → sese.023 held → release/cancel`).  
- `sese.024`/`sese.025` holati va `pacs.002` natijalarini eshik signallari sifatida qabul qiling; `ACSC` bo'shatishni tetiklaydi, `RJCT` esa bo'shashishga majbur qiladi.  
- `HldInd`, `Lnkgs`, `PrtlSttlmInd`, `SttlmTxCond` va ixtiyoriy CoSD qoidalari orqali shartli yetkazib berishni kodlash.  
- Zarur bo'lganda tashqi identifikatorlarni (masalan, `pacs.009` uchun UETR) korrelyatsiya qilish uchun `SupplementaryData` dan foydalaning.  
- Bozor taqvimi/kesishlar bo'yicha ushlab turish/bo'shatish vaqtini parametrlash; bekor qilish muddatidan oldin `sese.030`/`camt.056` chiqaring, kerak bo'lganda qaytarishga qayting.

### ISO 20022 foydali yuklarining namunasi (Izohlangan)

#### garovni almashtirish juftligi (`sese.023`) ko'rsatmalar bilan bog'langan

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
```Bog'langan ko'rsatmani `SUBST-2025-04-001-B` (o'rnini bosuvchi garovni FoP olish) `SctiesMvmntTp=RECE`, `Pmt=FREE` va `SUBST-2025-04-001-A` ga ishora qiluvchi `WITH` aloqasi bilan yuboring. Almashtirish tasdiqlangandan keyin ikkala oyog'ingizni mos keladigan `sese.030` bilan qo'yib yuboring.

#### FX tasdiqlanishi kutilayotgan qimmatli qog'ozlar to'xtatiladi (`sese.023` + `sese.030`)

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

Ikkala `pacs.009` oyoqlari `ACSC` ga yetgandan keyin qo'yib yuboring:

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

`sese.031` ushlab turishni tasdiqlaydi, keyin esa qimmatli qog'ozlar oyog'i bron qilingandan keyin `sese.025`.

#### PvP moliyalashtirish bosqichi (qimmatli qog'ozlar maqsadi bilan `pacs.009`)

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

`pacs.002` to'lov holatini kuzatib boradi (`ACSC` = tasdiqlangan, `RJCT` = rad etish). Agar oyna buzilgan bo'lsa, `camt.056`/`camt.029` orqali eslang yoki hisoblangan mablag'larni qaytarish uchun `pacs.004` yuboring.
---
lang: az
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

:::Qeyd Kanonik Mənbə
:::

## Hesablaşma ↔ ISO 20022 Sahə Xəritəçəkmə

Bu qeyd Iroha hesablaşma təlimatları arasında kanonik xəritəni əks etdirir
(`DvpIsi`, `PvpIsi`, repo girov axınları) və həyata keçirilən ISO 20022 mesajları
körpünün yanında. Bu tətbiq olunan mesaj iskelesini əks etdirir
`crates/ivm/src/iso20022.rs` və istehsal edərkən istinad kimi xidmət edir və ya
Norito faydalı yüklərin doğrulanması.

### İstinad Məlumat Siyasəti (İdentifikatorlar və Doğrulama)

Bu siyasət identifikator seçimlərini, doğrulama qaydalarını və istinad məlumatlarını paketləşdirir
Norito ↔ ISO 20022 körpüsünün mesajları yaymazdan əvvəl yerinə yetirməli olduğu öhdəliklər.

**ISO mesajı daxilində lövbər nöqtələri:**
- **Alət identifikatorları** → `delivery_leg.asset_definition_id` ↔ `SctiesLeg/FinInstrmId`
  (və ya ekvivalent alət sahəsi).
- **Tərəflər / agentlər** → `DlvrgSttlmPties/Pty` və `sese.*` üçün `RcvgSttlmPties/Pty`,
  və ya `pacs.009`-də agent strukturları.
- **Hesablar** → Saxlama/pul hesabları üçün `…/Acct` elementləri; kitabçanı əks etdirin
  `AccountId`, `SupplementaryData`.
- **Mülkiyyət identifikatorları** → `…/OthrId` ilə `Tp/Prtry` və əks olunur
  `SupplementaryData`. Tənzimlənən identifikatorları heç vaxt özəl olanlarla əvəz etməyin.

#### Mesaj ailəsi üzrə identifikator üstünlükləri

##### `sese.023` / `.024` / `.025` (qiymətli kağızlarla hesablaşma)

- **Alət (`FinInstrmId`)**
  - Üstünlük verilir: `…/ISIN` altında **ISIN**. Bu, CSD / T2S üçün kanonik identifikatordur.[^anna]
  - Geri dönmələr:
    - **CUSIP** və ya xarici ISO-dan təyin edilmiş `Tp/Cd` ilə `…/OthrId/Id` altında digər NSIN
      kod siyahısı (məsələn, `CUSP`); mandat verildikdə emitenti `Issr`-ə daxil edin.[^iso_mdr]
    - **Norito aktiv ID** mülkiyyətçi kimi: `…/OthrId/Id`, `Tp/Prtry="NORITO_ASSET_ID"` və
      eyni dəyəri `SupplementaryData`-də qeyd edin.
  - Əlavə deskriptorlar: **CFI** (`ClssfctnTp`) və **FISN** asanlaşdırmaq üçün dəstəklənir
    barışıq.[^iso_cfi][^iso_fisn]
- **Tərəflər (`DlvrgSttlmPties`, `RcvgSttlmPties`)**
  - Üstünlük verilir: **BIC** (`AnyBIC/BICFI`, ISO 9362).[^swift_bic]
  - Fallback: **LEI** burada mesajın versiyası xüsusi LEI sahəsini ifşa edir; əgər
    yoxdur, aydın `Prtry` etiketləri olan mülkiyyət identifikatorları daşıyın və metadata BIC daxil edin.[^iso_cr]
- **Məskunlaşma yeri / məkan** → məkan üçün **MIC** və CSD üçün **BIC**.[^iso_mic]

##### `colr.010` / `.011` / `.012` və `colr.007` (girovun idarə edilməsi)

- `sese.*` (ISIN-ə üstünlük verilir) ilə eyni alət qaydalarına əməl edin.
- Tərəflər defolt olaraq **BIC**-dən istifadə edirlər; **LEI**, sxemin ifşa etdiyi yerdə məqbuldur.[^swift_bic]
- Nağd pul məbləğləri düzgün kiçik vahidlərlə **ISO 4217** valyuta kodlarından istifadə etməlidir.[^iso_4217]

##### `pacs.009` / `camt.054` (PvP maliyyələşdirmə və bəyanatlar)

- **Agentlər (`InstgAgt`, `InstdAgt`, borclu/kreditor agentləri)** → **BIC** isteğe bağlı
  İcazə verilən yerlərdə LEI.[^swift_bic]
- **Hesablar**
  - Banklararası: **BIC** və daxili hesab arayışları ilə müəyyən edin.
  - Müştəri ilə bağlı bəyanatlar (`camt.054`): mövcud olduqda **IBAN** daxil edin və təsdiq edin
    (uzunluq, ölkə qaydaları, mod-97 yoxlama cəmi).[^swift_iban]
- **Valyuta** → **ISO 4217** 3 hərfli kod, kiçik vahidlərin yuvarlaqlaşdırılmasına riayət edin.[^iso_4217]
- **Torii qəbulu** → `POST /v2/iso20022/pacs009` vasitəsilə PvP maliyyələşdirmə ayaqlarını təqdim edin; körpü
  `Purp=SECU` tələb edir və indi istinad məlumatları konfiqurasiya edildikdə BIC piyada keçidlərini tətbiq edir.

#### Qiymətləndirmə qaydaları (emissiyadan əvvəl tətbiq olunur)

| İdentifikator | Doğrulama qaydası | Qeydlər |
|------------|-----------------|-------|
| **ISIN** | Regex `^[A-Z]{2}[A-Z0-9]{9}[0-9]$` və Luhn (mod-10) yoxlama rəqəmi ISO 6166 Əlavə C | Körpü emissiyasından əvvəl rədd edin; yuxarı zənginləşdirməyə üstünlük verir.[^anna_luhn] |
| **CUSIP** | Regex `^[A-Z0-9]{9}$` və modul-10 2 çəki ilə (simvolların rəqəmlərə xəritəsi) | Yalnız ISIN mövcud olmadıqda; bir dəfə ANNA/CUSIP piyada keçidi vasitəsilə xəritə.[^cusip] |
| **LEI** | Regex `^[A-Z0-9]{18}[0-9]{2}$` və mod-97 yoxlama rəqəmi (ISO 17442) | Qəbul etməzdən əvvəl GLEIF gündəlik delta faylları ilə təsdiqləyin.[^gleif] |
| **BIC** | Regex `^[A-Z]{4}[A-Z]{2}[A-Z0-9]{2}([A-Z0-9]{3})?$` | Könüllü filial kodu (son üç simvol). RA fayllarında aktiv statusu təsdiq edin.[^swift_bic] |
| **MIC** | ISO 10383 RA faylından qoruyun; yerlərin aktiv olmasını təmin edin (`!` dayandırma bayrağı yoxdur) | Emissiyadan əvvəl istismardan çıxarılan MİK-ləri qeyd edin.[^iso_mic] |
| **IBAN** | Ölkəyə məxsus uzunluq, böyük hərf-rəqəm, mod-97 = 1 | SWIFT tərəfindən aparılan reyestrdən istifadə edin; struktur etibarsız IBAN-ları rədd edin.[^swift_iban] |
| **Mülkiyyət hesabı/partiya identifikatorları** | `Max35Text` (UTF-8, ≤35 simvol) kəsilmiş boşluq ilə | `GenericAccountIdentification1.Id` və `PartyIdentification135.Othr/Id` sahələrinə aiddir. 35 simvoldan çox olan girişləri rədd edin ki, körpü yükləri ISO sxemlərinə uyğun olsun. |
| **Proksi hesab identifikatorları** | Qeyri-boş `Max2048Text` `…/Prxy/Id` altında isteğe bağlı tip kodları ilə `…/Prxy/Tp/{Cd,Prtry}` | Əsas IBAN ilə yanaşı saxlanılır; doğrulama hələ də PvP relslərini əks etdirmək üçün proksi tutacaqları (isteğe bağlı tip kodları ilə) qəbul edərkən IBAN tələb edir. |
| **CFI** | ISO 10962 taksonomiyasından istifadə edərək altı simvollu kod, böyük hərflər | Könüllü zənginləşdirmə; simvolların alət sinfinə uyğun olduğundan əmin olun.[^iso_cfi] |
| **FISN** | 35 simvola qədər, böyük hərf-rəqəm və məhdud durğu işarələri | Könüllü; ISO 18774 təlimatına uyğun olaraq kəsin/normallaşdırın.[^iso_fisn] |
| **Valyuta** | ISO 4217 3 hərfli kod, kiçik vahidlərlə müəyyən edilən miqyas | Məbləğlər icazə verilən onluqlara yuvarlaqlaşdırılmalıdır; Norito tərəfində tətbiq edin.[^iso_4217] |

#### Piyada keçidi və məlumatların saxlanması öhdəlikləri

- **ISIN ↔ Norito aktiv ID** və **CUSIP ↔ ISIN** piyada keçidlərini qoruyun. Hər gecə yeniləyin
  ANNA/DSB lentləri və versiya CI tərəfindən istifadə edilən snapşotlara nəzarət edir.[^anna_crosswalk]
- GLEIF ictimai əlaqələr fayllarından **BIC ↔ LEI** xəritələrini yeniləyin ki, körpü
  tələb olunduqda hər ikisini buraxın.[^bic_lei]
- Körpü metadatasının yanında **MIC təriflərini** saxlayın ki, məkanın yoxlanılması mümkün olsun
  RA faylları günün ortasında dəyişdikdə belə deterministikdir.[^iso_mic]
- Audit üçün körpü metadatasında verilənlərin mənşəyini (vaxt damğası + mənbə) qeyd edin. Davam et
  buraxılmış təlimatlarla yanaşı snapshot identifikatoru.
- Hər bir yüklənmiş məlumat dəstinin surətini saxlamaq üçün `iso_bridge.reference_data.cache_dir`-i konfiqurasiya edin
  mənşəli metadata ilə yanaşı (versiya, mənbə, vaxt damgası, yoxlama məbləği). Bu, auditorlara imkan verir
  və operatorlar hətta yuxarı axın görüntüləri fırlanandan sonra da tarixi lentləri fərqləndirir.
- ISO piyada keçidi anlıq görüntüləri istifadə edərək `iroha_core::iso_bridge::reference_data` tərəfindən qəbul edilir
  `iso_bridge.reference_data` konfiqurasiya bloku (yollar + yeniləmə intervalı). Ölçerlər
  `iso_reference_status`, `iso_reference_age_seconds`, `iso_reference_records` və
  `iso_reference_refresh_interval_secs` xəbərdarlıq üçün iş vaxtının sağlamlığını ifşa edir. Torii
  körpü konfiqurasiya edilmiş agent BIC-ləri olmayan `pacs.008` təqdimatlarını rədd edir
  piyada keçidi, qarşı tərəf olduqda deterministik `InvalidIdentifier` xətaları
  naməlum.【crates/iroha_torii/src/iso20022_bridge.rs#L1078】
- IBAN və ISO 4217 bağlamaları eyni səviyyədə tətbiq edilir: pacs.008/pacs.009 indi axır
  Borclu/kreditor IBAN-larında konfiqurasiya edilmiş ləqəblər olmadıqda və ya zaman `InvalidIdentifier` səhvləri buraxın
  hesablaşma valyutası `currency_assets`-də əskikdir, bu da düzgün olmayan körpünün qarşısını alır
  kitabçaya çatmaq üçün təlimatlar. IBAN-ın doğrulanması da ölkəyə xasdır
  ISO 7064 mod‑97 keçidindən əvvəl uzunluqlar və rəqəmsal yoxlama rəqəmləri struktur cəhətdən etibarsızdır
  dəyərlər erkən rədd edilir.【crates/iroha_torii/src/iso20022_bridge.rs#L775】【crates/iroha_torii/src/iso20022_bridge.rs#L827】【crates/ivm/src/iso20012.
- CLI məskunlaşma köməkçiləri eyni qoruyucu relsləri miras alırlar: keçid
  DvP-yə sahib olmaq üçün `--iso-reference-crosswalk <path>` `--delivery-instrument-id` ilə birlikdə
  `sese.023` XML snapşotunu buraxmazdan əvvəl alət identifikatorlarını doğrulayın.【crates/iroha_cli/src/main.rs#L3752】
- `cargo xtask iso-bridge-lint` (və CI sarğı `ci/check_iso_reference_data.sh`) tüy
  piyada keçidi şəkilləri və qurğular. Komanda `--isin`, `--bic-lei`, `--mic` və
  `--fixtures` bayraqlayır və işə salındıqda `fixtures/iso_bridge/`-də nümunə məlumat dəstlərinə qayıdır
  arqumentsiz.【xtask/src/main.rs#L146】【ci/check_iso_reference_data.sh#L1】
- IVM köməkçisi indi real ISO 20022 XML zərflərini qəbul edir (head.001 + `DataPDU` + `Document`)
  və `head.001` sxemi vasitəsilə Biznes Tətbiq Başlığını təsdiq edir, beləliklə `BizMsgIdr`,
  `MsgDefIdr`, `CreDt` və BIC/ClrSysMmbId agentləri deterministik şəkildə qorunur; XMLDSig/XAdES
  bloklar qəsdən atlanır. 

#### Tənzimləmə və bazar strukturu mülahizələri- **T+1 hesablaşma**: ABŞ/Kanada səhm bazarları 2024-cü ildə T+1 səviyyəsinə keçdi; Norito tənzimləyin
  planlaşdırma və müvafiq olaraq SLA xəbərdarlıqları.[^sec_t1][^csa_t1]
- **CSDR cərimələri**: Hesablaşma intizam qaydaları nağd pul cəzalarını tətbiq edir; Norito təmin edin
  metadata uzlaşma üçün cəza arayışlarını çəkir.[^csdr]
- **Eyni gün hesablaşma pilotları**: Hindistanın tənzimləyicisi T0/T+0 hesablaşmasında mərhələli olur; saxlamaq
  pilotlar genişləndikcə körpü təqvimləri yeniləndi.[^india_t0]
- **Girov alışları / saxlanmaları**: alış qrafikləri və isteğe bağlı saxlamalarda ESMA yeniləmələrini izləyin
  şərti çatdırılma (`HldInd`) ən son rəhbərliyə uyğun gəlir.[^csdr]

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

### Çatdırılma-ödənişə qarşı-Ödəniş → `sese.023`

| DvP sahəsi | ISO 20022 yolu | Qeydlər |
|------------------------------------------------------|---------------------------------------|-------|
| `settlement_id` | `TxId` | Stabil həyat dövrü identifikatoru |
| `delivery_leg.asset_definition_id` (təhlükəsizlik) | `SctiesLeg/FinInstrmId` | Kanonik identifikator (ISIN, CUSIP, …) |
| `delivery_leg.quantity` | `SctiesLeg/Qty` | Ondalık sətir; fəxri aktivin dəqiqliyi |
| `payment_leg.asset_definition_id` (valyuta) | `CashLeg/Ccy` | ISO valyuta kodu |
| `payment_leg.quantity` | `CashLeg/Amt` | Ondalık sətir; Rəqəm spesifikasiyasına görə yuvarlaqlaşdırılmış |
| `delivery_leg.from` (satıcı / çatdıran tərəf) | `DlvrgSttlmPties/Pty/Bic` | Çatdıran iştirakçının BIC kodu *(hesabın kanonik identifikatoru hazırda metadatada eksport edilib)* |
| `delivery_leg.from` hesab identifikatoru | `DlvrgSttlmPties/Acct` | Sərbəst forma; Norito metadata dəqiq hesab ID daşıyır |
| `delivery_leg.to` (alıcı / qəbul edən tərəf) | `RcvgSttlmPties/Pty/Bic` | Qəbul edən iştirakçının BIC |
| `delivery_leg.to` hesab identifikatoru | `RcvgSttlmPties/Acct` | Sərbəst forma; hesab ID qəbul edən uyğun gəlir
| `plan.order` | `Plan/ExecutionOrder` | Sayı: `DELIVERY_THEN_PAYMENT` və ya `PAYMENT_THEN_DELIVERY` |
| `plan.atomicity` | `Plan/Atomicity` | Sayı: `ALL_OR_NOTHING`, `COMMIT_FIRST_LEG`, `COMMIT_SECOND_LEG` |
| **Mesajın məqsədi** | `SttlmTpAndAddtlParams/SctiesMvmntTp` | `DELI` (çatdırmaq) və ya `RECE` (qəbul etmək); təqdim edən tərəfin hansı ayağı icra etdiyi güzgülər. |
|                                                        | `SttlmTpAndAddtlParams/Pmt` | `APMT` (ödənişə qarşı) və ya `FREE` (ödənişsiz). |
| `delivery_leg.metadata`, `payment_leg.metadata` | `SctiesLeg/Metadata`, `CashLeg/Metadata` | UTF‑8 kimi kodlaşdırılmış isteğe bağlı Norito JSON |

> **Hesablaşma kvalifikatorları** – körpü hesablaşma vəziyyəti kodlarını (`SttlmTxCond`), qismən hesablaşma göstəricilərini (`PrtlSttlmInd`) və Norito metaməlumatından mövcud olduqda I18NI000001-ə köçürməklə, körpü bazar təcrübəsini əks etdirir. İSO xarici kod siyahılarında dərc edilmiş siyahıları tətbiq edin ki, təyinat CSD dəyərləri tanısın.

### Ödəniş-ödənişin maliyyələşdirilməsi → `pacs.009`

PvP təlimatını maliyyələşdirən nağd pul vəsaitləri FI-to-FI krediti kimi verilir.
köçürmələr. Körpü bu ödənişləri qeyd edir ki, aşağı axın sistemləri tanısın
qiymətli kağızlar üzrə hesablaşmanı maliyyələşdirirlər.

| PvP maliyyələşdirmə sahəsi | ISO 20022 yolu | Qeydlər |
|------------------------------------------------|----------------------------------------------------|-------|
| `primary_leg.quantity` / {məbləğ, valyuta} | `IntrBkSttlmAmt` + `IntrBkSttlmCcy` | Təşəbbüskardan debet edilmiş məbləğ/valyuta. |
| Qarşı tərəf agent identifikatorları | `InstgAgt`, `InstdAgt` | Göndərən və qəbul edən agentlərin BIC/LEI. |
| Qəsəbə məqsədi | `CdtTrfTxInf/PmtTpInf/CtgyPurp/Cd` | Qiymətli kağızlarla əlaqəli PvP maliyyələşdirilməsi üçün `SECU` olaraq təyin edin. |
| Norito metadata (hesab idləri, FX datası) | `CdtTrfTxInf/SplmtryData` | Tam AccountId, FX zaman damgaları, icra planı göstərişləri daşıyır. |
| Təlimat identifikatoru / həyat dövrü əlaqələndirilməsi | `CdtTrfTxInf/PmtId/InstrId`, `CdtTrfTxInf/RmtInf` | Norito `settlement_id` ilə uyğun gəlir, beləliklə, pul ayağı qiymətli kağızlar tərəfi ilə uzlaşır. |

JavaScript SDK-nın ISO körpüsü standart olaraq bu tələbə uyğun gəlir
`pacs.009` kateqoriya məqsədi `SECU`; zəng edənlər onu başqası ilə əvəz edə bilər
qiymətli kağızlar olmayan kredit köçürmələri zamanı etibarlı ISO kodu, lakin etibarsızdır
dəyərlər qabaqcadan rədd edilir.

Bir infrastruktur açıq qiymətli kağızların təsdiqini tələb edirsə, körpü
`sese.025` buraxmaqda davam edir, lakin bu təsdiq qiymətli kağızlar ayağını əks etdirir
statusu (məsələn, `ConfSts = ACCP`) PvP “məqsədi” deyil.

### Ödəniş-ödənişin təsdiqi → `sese.025`

| PvP sahəsi | ISO 20022 yolu | Qeydlər |
|-----------------------------------------|---------------------------|-------|
| `settlement_id` | `TxId` | Stabil həyat dövrü identifikatoru |
| `primary_leg.asset_definition_id` | `SttlmCcy` | Əsas mərhələ üçün valyuta kodu |
| `primary_leg.quantity` | `SttlmAmt` | Təşəbbüskar tərəfindən çatdırılan məbləğ |
| `counter_leg.asset_definition_id` | `AddtlInf` (JSON yükü) | Əks valyuta kodu əlavə məlumatda yerləşdirilmiş |
| `counter_leg.quantity` | `SttlmQty` | Əks məbləğ |
| `plan.order` | `Plan/ExecutionOrder` | DvP | ilə eyni nömrə təyin edildi
| `plan.atomicity` | `Plan/Atomicity` | DvP | ilə eyni nömrə təyin edildi
| `plan.atomicity` statusu (`ConfSts`) | `ConfSts` | Uyğunlaşdıqda `ACCP`; körpü rədd edildikdə uğursuzluq kodları verir |
| Qarşı tərəf identifikatorları | `AddtlInf` JSON | Cari körpü tam AccountId/BIC kordonlarını metadata |

### Repo Girov Əvəzetmə → `colr.007`

| Repo sahəsi / kontekst | ISO 20022 yolu | Qeydlər |
|------------------------------------------------|----------------------------------|-------|
| `agreement_id` (`RepoIsi` / `ReverseRepoIsi`) | `OblgtnId` | Repo müqaviləsi identifikatoru |
| Girov əvəzedicisi Tx identifikatoru | `TxId` | Əvəzetmə başına yaradılan |
| Orijinal girov miqdarı | `Substitution/OriginalAmt` | Əvəzetmədən əvvəl girov qoyulmuş matçlar |
| Orijinal girov valyutası | `Substitution/OriginalCcy` | Valyuta kodu |
| Əvəzedici təminat miqdarı | `Substitution/SubstituteAmt` | Əvəzetmə məbləği |
| Əvəzedici girov valyutası | `Substitution/SubstituteCcy` | Valyuta kodu |
| Qüvvəyə minmə tarixi (idarəetmə marjası cədvəli) | `Substitution/EffectiveDt` | ISO tarixi (YYYY-AA-GG) |
| Saç kəsimi təsnifatı | `Substitution/Type` | Hal-hazırda `FULL` və ya `PARTIAL` idarəetmə siyasətinə əsaslanır |
| İdarəetmə səbəbi / saç kəsmə qeydi | `Substitution/ReasonCd` | Könüllü, idarəetmə əsaslandırmasını daşıyır |

### Maliyyələşdirmə və Hesabatlar

| Iroha kontekst | ISO 20022 mesajı | Xəritəçəkmə yeri |
|---------------------------------|-------------------|------------------|
| Repo pul ayaq alovlanma / açılmaq | `pacs.009` | DvP/PvP ayaqlarından `IntrBkSttlmAmt`, `IntrBkSttlmCcy`, `IntrBkSttlmDt`, `InstgAgt`, `InstdAgt` |
| Hesablaşmadan sonrakı hesabatlar | `camt.054` | `Ntfctn/Ntry[*]` altında qeydə alınan ödəniş ayağı hərəkətləri; körpü `SplmtryData`-də kitab/hesab metadatasını yerləşdirir |

### İstifadə Qeydləri* Bütün məbləğlər Norito rəqəmli köməkçilərdən (`NumericSpec`) istifadə edilərək seriallaşdırılır.
  aktiv tərifləri üzrə miqyas uyğunluğunu təmin etmək.
* `TxId` dəyərləri `Max35Text` - UTF‑8 uzunluğu ≤35 simvoldan əvvəl tətbiq edin
  ISO 20022 mesajlarına eksport edilir.
* BIC-lər 8 və ya 11 böyük hərf-rəqəm simvolu olmalıdır (ISO9362); rədd etmək
  Norito metadata, ödəniş və ya hesablaşmadan əvvəl bu yoxlamadan keçmir
  təsdiqlər.
* Hesab identifikatorları (AccountId / ChainId) əlavə olaraq ixrac edilir
  metadata beləliklə qəbul edən iştirakçılar öz yerli kitab kitabları ilə barışa bilsinlər.
* `SupplementaryData` kanonik JSON olmalıdır (UTF‑8, çeşidlənmiş açarlar, JSON-doğma
  qaçır). SDK köməkçiləri bunu imzalar, telemetriya hashləri və ISO üçün tətbiq edir
  faydalı yük arxivləri yenidən qurma zamanı deterministik olaraq qalır.
* Valyuta məbləğləri ISO4217 fraksiya rəqəmlərinə uyğundur (məsələn, JPY-də 0 var
  ondalık, USD-də 2 var); körpü müvafiq olaraq Norito ədədi dəqiqliyi sıxır.
* CLI hesablaşma köməkçiləri (`iroha app settlement ... --atomicity ...`) indi yayırlar
  Norito təlimatlarının icra planları 1:1 ilə `Plan/ExecutionOrder` və
  `Plan/Atomicity` yuxarıda.
* ISO köməkçisi (`ivm::iso20022`) yuxarıda sadalanan sahələri təsdiqləyir və rədd edir
  DvP/PvP ayaqlarının Rəqəm xüsusiyyətlərini və ya qarşı tərəfin qarşılıqlılığını pozduğu mesajlar.

### SDK Builder Köməkçiləri

- JavaScript SDK indi `buildPacs008Message` / ifşa edir
  `buildPacs009Message` (bax `javascript/iroha_js/src/isoBridge.js`) belə ki, müştəri
  avtomatlaşdırma strukturlaşdırılmış hesablaşma metadatasını çevirə bilər (BIC/LEI, IBAN,
  məqsəd kodları, əlavə Norito sahələri) deterministik paketlərə XML
  bu təlimatdakı xəritəçəkmə qaydalarını yenidən tətbiq etmədən.
- Hər iki köməkçi açıq şəkildə `creationDateTime` tələb edir (saat qurşağı ilə ISO‑8601)
  ona görə də operatorlar yerinə iş prosesindən deterministik vaxt damğası çəkməlidirlər
  SDK-nın standart olaraq divar saatı vaxtına icazə verilməsi.
- `recipes/iso_bridge_builder.mjs` bu köməkçilərin necə qoşulacağını nümayiş etdirir
  mühit dəyişənlərini və ya JSON konfiqurasiya fayllarını birləşdirən CLI
  XML yaradır və istəyə görə onu Torii (`ISO_SUBMIT=1`)-a təqdim edir, təkrar istifadə edir.
  ISO körpü resepti ilə eyni gözləmə kadansı.


### İstinadlar

- `SttlmTpAndAddtlParams/SctiesMvmntTp` (`DELI`/`RECE`) və `Pmt`-i göstərən LuxCSD / Clearstream ISO 20022 hesablaşma nümunələri (`APMT`/`FREE`).<sup>[1](https://www.luxcsd.com/resource/blob/3434074/6f8add4708407a4701055be4dd04846b/c23005-eis-examples-cbf-data.pdf)</sup>
- Hesablaşma kvalifikatorlarını əhatə edən Clearstream DCP spesifikasiyası (`SttlmTxCond`, `PrtlSttlmInd`).<sup>[2](https://www.clearstream.com/clearstream-en/res-library/market-coverage/instruction-specifications-swift-iso-20022-dcp-mode-ceu-spain-2357008)</sup>
- Qiymətli kağızlarla bağlı PvP maliyyələşdirilməsi üçün `CtgyPurp/Cd = SECU` ilə `pacs.009`-i tövsiyə edən SWIFT PMPG təlimatı.<sup>[3](https://www.swift.com/swift-resource/251897/download)</sup>
- İdentifikator uzunluğu məhdudiyyətləri üçün ISO 20022 mesaj tərifi hesabatları (BIC, Max35Text).<sup>[4](https://www.iso20022.org/sites/default/files/2020-12/ISO20022_MDRPart2_ChangeOrVerifyAccountIdentification_2020_2021_v1_ForSEGReview.pdf)</sup>
- ISIN formatı və yoxlama məbləği qaydaları üzrə ANNA DSB təlimatı.<sup>[5](https://www.anna-dsb.com/isin/)</sup>

### İstifadə Məsləhətləri

- LLM-nin yoxlaya bilməsi üçün həmişə müvafiq Norito parçasını və ya CLI əmrini yapışdırın
  dəqiq sahə adları və Rəqəm miqyası.
- Kağız izi saxlamaq üçün sitatlar (`provide clause references`) tələb edin
  uyğunluq və auditor yoxlaması.
- `docs/source/finance/settlement_iso_mapping.md`-də cavab xülasəsini çəkin
  (və ya əlaqəli əlavələr) beləliklə gələcək mühəndislərin sorğunu təkrarlamasına ehtiyac yoxdur.

## Tədbir Sifariş Kitabları (ISO 20022 ↔ Norito Bridge)

### Ssenari A — Girov Əvəzetmə (Repo / Girov)

**İştirakçılar:** girov verən/götürən (və/və ya agentlər), qoruyucu(lar), CSD/T2S  
**Vaxt:** bazar kəsimləri və T2S gündüz/gecə dövrləri üzrə; iki ayağı orkestrləşdirin ki, onlar eyni məskunlaşma pəncərəsində tamamlansınlar.

#### Mesaj xoreoqrafiyası
1. `colr.010` Girov Əvəzetmə Sorğusu → girov verən/götürən və ya agent.  
2. `colr.011` Girov Əvəzetmə Cavabı → qəbul/rədd (istəyə bağlı imtina səbəbi).  
3. `colr.012` Girov Əvəzetmə Təsdiqi → əvəzetmə razılaşmasını təsdiq edir.  
4. `sese.023` təlimatları (iki ayaq):  
   - Orijinal girovun qaytarılması (`SctiesMvmntTp=DELI`, `Pmt=FREE`, `SctiesTxTp=COLO`).  
   - Əvəzedici girov təqdim edin (`SctiesMvmntTp=RECE`, `Pmt=FREE`, `SctiesTxTp=COLI`).  
   Cütlüyünü birləşdirin (aşağıya baxın).  
5. `sese.024` status məsləhətləri (qəbul edildi, uyğunlaşdırıldı, gözlənilən, uğursuz, rədd edildi).  
6. `sese.025` təsdiqləmələri bir dəfə sifariş edildi.  
7. Könüllü nağd pul deltası (rüsumlar/saç kəsimi) → `pacs.009` `CtgyPurp/Cd = SECU` ilə FI-to-FI Kredit Transferi; status `pacs.002` vasitəsilə, `pacs.004` vasitəsilə qaytarılır.

#### Tələb olunan təsdiqlər / statuslar
- Nəqliyyat səviyyəsi: şlüzlər `admi.007` yaya bilər və ya biznes emalından əvvəl rədd edə bilər.  
- Hesablaşmanın həyat dövrü: `sese.024` (emal statusları + səbəb kodları), `sese.025` (son).  
- Nağd pul tərəfi: `pacs.002` (`PDNG`, `ACSC`, `RJCT` və s.), `pacs.004` qaytarılması üçün.

#### Şərtilik / sahələri açın
- `SctiesSttlmTxInstr/Lnkgs` (`WITH`/`BEFO`/`AFTE`) iki təlimatı zəncirləmək üçün.  
- `SttlmParams/HldInd` meyarlara cavab verənə qədər saxlamaq; `sese.030` (`sese.031` statusu) vasitəsilə buraxın.  
- Qismən hesablaşmaya nəzarət etmək üçün `SttlmParams/PrtlSttlmInd` (`NPAR`, `PART`, `PARC`, `PARQ`).  
- Bazara xas şərtlər üçün `SttlmParams/SttlmTxCond/Cd` (`NOMC` və s.).  
- Dəstəkləndikdə Könüllü T2S Şərti Qiymətli Kağızların Çatdırılması (CoSD) qaydaları.

#### İstinadlar
- SWIFT girovun idarə edilməsi MDR (`colr.010/011/012`).  
- Əlaqələndirmə və statuslar üçün CSD/T2S istifadə təlimatları (məsələn, DNB, ECB Insights).  
- SMPG hesablaşma təcrübəsi, Clearstream DCP təlimatları, ASX ISO seminarları.

### Ssenari B — FX pəncərəsinin pozulması (PvP Maliyyələşdirmə Uğursuzluğu)

**İştirakçılar:** qarşı tərəflər və kassa agentləri, qiymətli kağızların mühafizəçisi, CSD/T2S  
**Vaxt:** FX PvP pəncərələri (CLS/ikitərəfli) və CSD kəsmələri; nağd pulun təsdiqini gözləyən qiymətli kağızların ayaqlarını saxla.

#### Mesaj xoreoqrafiyası
1. `CtgyPurp/Cd = SECU` ilə valyuta üzrə `pacs.009` FI-dən FI Kredit Transferi; status `pacs.002` vasitəsilə; `camt.056`/`camt.029` vasitəsilə geri çağırmaq/ləğv etmək; artıq həll olunarsa, `pacs.004` qaytarın.  
2. `sese.023` DvP təlimat(lar)ı `HldInd=true` ilə, beləliklə qiymətli kağızlar ayağı nağd pulun təsdiqini gözləyir.  
3. Həyat dövrü `sese.024` bildirişləri (qəbul edilib/uyğunlaşdırılıb/gözlənilir).  
4. Hər iki `pacs.009` ayağı pəncərənin müddəti bitməzdən əvvəl `ACSC`-ə çatarsa ​​→ `sese.030` → `sese.031` (mod statusu) → `sese.025` (təsdiq) ilə buraxın.  
5. Valyuta pəncərəsi pozulubsa → nağd pulu ləğv edin/geri çağırın (`camt.056/029` və ya `pacs.004`) və qiymətli kağızları ləğv edin (`sese.020` + `sese.027`, və ya I18NI0000 qaydası artıq təsdiqlənibsə).

#### Tələb olunan təsdiqlər / statuslar
- Nağd pul: `pacs.002` (`PDNG`, `ACSC`, `RJCT`), `pacs.004` qaytarılması üçün.  
- Qiymətli Kağızlar: `sese.024` (`NORE`, `ADEA`, `sese.025` kimi gözlənilən/uğursuz səbəblər.  
- Nəqliyyat: `admi.007` / şlüz biznes emalından əvvəl rədd edir.

#### Şərtilik / sahələri açın
- `SttlmParams/HldInd` + `sese.030` buraxılış/uğur/uğursuzluqda ləğv.  
- `Lnkgs` qiymətli kağızlar üzrə təlimatları kassa ayağına bağlamaq üçün.  
- Şərti çatdırılma istifadə edildikdə T2S CoSD qaydası.  
- `PrtlSttlmInd` gözlənilməz hissələrin qarşısını almaq üçün.  
- `pacs.009`, `CtgyPurp/Cd = SECU` qiymətli kağızlarla əlaqəli maliyyələşdirməni qeyd edir.

#### İstinadlar
- Qiymətli kağızlar prosesində ödənişlər üçün PMPG / CBPR+ təlimatı.  
- SMPG hesablaşma təcrübələri, əlaqələndirmə/tutma haqqında T2S anlayışları.  
- Clearstream DCP təlimatları, texniki xidmət mesajları üçün ECMS sənədləri.

### pacs.004 Xəritəçəkmə qeydlərini qaytarın

- geri qaytarma qurğuları indi normallaşdırır `ChrgBr` (`DEBT`/`CRED`/`SHAR`/`SLEV`) və özəl geri qaytarma səbəbləri istehlakçıların təkrar ifşası kimi ifşa olunur I18NI0400, XML zərfini yenidən təhlil etmədən atribut və operator kodları.
- `DataPDU` zərflərindəki AppHdr imza blokları qəbul zamanı nəzərə alınmır; auditlər daxil edilmiş XMLDSIG sahələrinə deyil, kanal mənşəyinə əsaslanmalıdır.

### Körpü üçün əməliyyat yoxlama siyahısı
- Yuxarıdakı xoreoqrafiyanı tətbiq edin (təminat: `colr.010/011/012 → sese.023/024/025`; FX pozuntusu: `pacs.009 (+pacs.002) → sese.023 held → release/cancel`).  
- `sese.024`/`sese.025` statuslarını və `pacs.002` nəticələrini qapı siqnalları kimi qəbul edin; `ACSC` buraxılışa səbəb olur, `RJCT` boşalmağa məcbur edir.  
- `HldInd`, `Lnkgs`, `PrtlSttlmInd`, `SttlmTxCond` və əlavə CoSD qaydaları vasitəsilə şərti çatdırılmanı kodlayın.  
- Tələb olunduqda xarici identifikatorları (məsələn, `pacs.009` üçün UETR) əlaqələndirmək üçün `SupplementaryData` istifadə edin.  
- Bazar təqvimi/kəsikləri ilə gözləmə/açma vaxtı parametrlərini təyin edin; ləğvetmə müddətindən əvvəl `sese.030`/`camt.056` buraxın, lazım gəldikdə geri qaytarın.

### Nümunə ISO 20022 Faydalı Yüklər (İzahlı)

#### Təlimat bağlantısı ilə girov əvəzedici cüt (`sese.023`)

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

`SUBST-2025-04-001-A`-ə işarə edən `SctiesMvmntTp=RECE`, `Pmt=FREE` və `WITH` əlaqəsi ilə əlaqəli `SUBST-2025-04-001-B` (əvəzedici girovun FoP qəbulu) təlimatını təqdim edin. Əvəzetmə təsdiqləndikdən sonra hər iki ayağı uyğun `sese.030` ilə buraxın.

#### Qiymətli kağızlar FX təsdiqini gözləyir (`sese.023` + `sese.030`)

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
```Hər iki `pacs.009` ayağı `ACSC`-ə çatdıqda buraxın:

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

`sese.031` saxlama buraxılışını təsdiqləyir, qiymətli kağızlar bəndi sifariş edildikdən sonra `sese.025`.

#### PvP maliyyələşdirmə ayağı (qiymətli kağızlar məqsədi ilə `pacs.009`)

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

`pacs.002` ödəniş statusunu izləyir (`ACSC` = təsdiq, `RJCT` = rədd). Pəncərə pozulubsa, `camt.056`/`camt.029` vasitəsilə geri çağırın və ya hesablanmış vəsaitləri qaytarmaq üçün `pacs.004` göndərin.
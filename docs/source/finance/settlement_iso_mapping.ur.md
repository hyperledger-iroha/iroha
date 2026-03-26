---
lang: ur
direction: rtl
source: docs/source/finance/settlement_iso_mapping.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d1f1005d6a273ab732a7c7a7adca349c17569fe2e2755b8daccf2186724044f8
source_last_modified: "2026-01-22T15:57:09.830555+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## تصفیہ ↔ آئی ایس او 20022 فیلڈ میپنگ

اس نوٹ نے Iroha تصفیہ ہدایات کے مابین کیننیکل میپنگ کو اپنی گرفت میں لیا ہے
.
پل کے ذریعہ یہ اس پیغام کی سہاروں کی عکاسی کرتا ہے جس میں نافذ کیا گیا ہے
`crates/ivm/src/iso20022.rs` اور تیار کرتے وقت ایک حوالہ کے طور پر کام کرتا ہے
Norito پے لوڈ کی توثیق کرنا۔

### حوالہ ڈیٹا پالیسی (شناخت کنندگان اور توثیق)

یہ پالیسی شناخت کنندہ کی ترجیحات ، توثیق کے قواعد ، اور حوالہ ڈیٹا کو پیک کرتی ہے
ان ذمہ داریوں کو جو Norito ↔ ISO 20022 پل کو پیغامات خارج کرنے سے پہلے نافذ کرنا چاہئے۔

** آئی ایس او پیغام کے اندر اینکر پوائنٹس: **
- ** آلے کی شناخت کرنے والے ** → `delivery_leg.asset_definition_id` ↔ `SctiesLeg/FinInstrmId`
  (یا مساوی آلہ فیلڈ)۔
۔
  یا `pacs.009` میں ایجنٹ کے ڈھانچے۔
- ** اکاؤنٹس ** → `…/Acct` محفوظ کیپنگ/کیش اکاؤنٹس کے لئے عناصر ؛ آئینہ آن لیجر
  `AccountId` `SupplementaryData` میں۔
- ** ملکیتی شناخت کار ** I `Tp/Prtry` کے ساتھ `…/OthrId`
  `SupplementaryData`۔ کبھی بھی باقاعدہ شناخت کاروں کو ملکیتی افراد کے ساتھ تبدیل نہ کریں۔

#### میسج فیملی کے ذریعہ شناخت کنندہ کی ترجیح

######`sese.023` / `.024` / `.025` (سیکیورٹیز آبادکاری)

- ** آلہ (`FinInstrmId`) **
  - ترجیحی: ** isin ** `…/ISIN` کے تحت۔ یہ CSDS / T2S کے لئے کیننیکل شناخت کنندہ ہے۔ [^انا]
  - فال بیکس:
    - ** CUSIP ** یا `…/OthrId/Id` کے تحت `Tp/Cd` کے ساتھ ISO بیرونی سے سیٹ کریں
      کوڈ کی فہرست (جیسے ، `CUSP`) ؛ جب لازمی طور پر `Issr` میں جاری کرنے والے کو شامل کریں۔ [^iso_mdr]
    - ** Norito اثاثہ ID ** بطور ملکیتی: `…/OthrId/Id` ، `Tp/Prtry="NORITO_ASSET_ID"` ، اور
      `SupplementaryData` میں اسی قدر کو ریکارڈ کریں۔
  - اختیاری وضاحتی: ** CFI ** (`ClssfctnTp`) اور ** FISN ** جہاں آسانی کے لئے تعاون کیا گیا ہے
    مفاہمت۔ [^iso_cfi] [^iso_fisn]
- ** پارٹیاں (`DlvrgSttlmPties` ، `RcvgSttlmPties`) **
  - ترجیحی: ** بِک ** (`AnyBIC/BICFI` ، ISO 9362)۔ [^swift_bic]
  - فال بیک: ** لی ** جہاں پیغام کا ورژن ایک سرشار لی فیلڈ کو بے نقاب کرتا ہے۔ اگر
    غیر حاضر ، واضح `Prtry` لیبلوں کے ساتھ ملکیتی IDs لے جائیں اور میٹا ڈیٹا میں BIC شامل کریں۔ [^iso_cr]
- ** آبادکاری / مقام کی جگہ ** → ** مائک ** مقام کے لئے اور ** بیک ** سی ایس ڈی کے لئے۔

######`colr.010` / `.011` / `.012` اور `colr.007` (کولیٹرل مینجمنٹ)

- اسی آلے کے قواعد پر عمل کریں جیسے `sese.*` (ISIN ترجیحی)۔
- پارٹیاں ** BIC ** بطور ڈیفالٹ استعمال کرتی ہیں۔ ** لی ** قابل قبول ہے جہاں اسکیما اس کو بے نقاب کرتا ہے۔ [^سوئفٹ_بک]
- نقد رقم کا استعمال ** ISO 4217 ** کرنسی کوڈ کو صحیح معمولی یونٹوں کے ساتھ کرنا چاہئے۔ [^iso_4217]

######`pacs.009` / `camt.054` (PVP فنڈنگ ​​اور بیانات)- ** ایجنٹوں (`InstgAgt` ، `InstdAgt` ، قرض دہندہ/قرض دہندہ ایجنٹ)
  لی جہاں اجازت دی گئی ہے۔ [^swift_bic]
- ** اکاؤنٹس **
  - انٹربینک: ** BIC ** اور اندرونی اکاؤنٹ کے حوالہ جات کے ذریعہ شناخت کریں۔
  - کسٹمر کا سامنا کرنے والے بیانات (`camt.054`): شامل کریں ** ابن ** جب موجود ہو اور اس کی توثیق کریں
    (لمبائی ، ملک کے قواعد ، Mod-97 چیکسم)۔ [^Swift_iban]
-** کرنسی ** → ** آئی ایس او 4217 ** 3-لیٹر کوڈ ، معمولی یونٹ راؤنڈنگ کا احترام کریں۔ [^iso_4217]
. پل
  `Purp=SECU` کی ضرورت ہوتی ہے اور جب حوالہ ڈیٹا تشکیل دیا جاتا ہے تو اب BIC کراس واک کو نافذ کرتا ہے۔

#### توثیق کے قواعد (اخراج سے پہلے درخواست دیں)

| شناخت کنندہ | توثیق کا قاعدہ | نوٹ |
| ------------ | ----------------- | ------- |
| ** isin ** | ریجیکس `^[A-Z]{2}[A-Z0-9]{9}[0-9]$` اور LUHN (MOD-10) DIGIT چیک کریں ISO 6166 ضمیمہ C | پل کے اخراج سے پہلے مسترد ؛ اپ اسٹریم افزودگی کو ترجیح دیں۔ [^انا_لوہن] |
| ** cusip ** | ریجیکس `^[A-Z0-9]{9}$` اور ماڈیولس -10 2 وزن کے ساتھ (حروف کا نقشہ ہندسے) | صرف اس وقت جب آئین دستیاب نہیں ہے۔ انا/cusip کراس واک کے ذریعے نقشہ ایک بار کھایا جاتا ہے۔ [^cusip] |
| ** لی ** | ریجیکس `^[A-Z0-9]{18}[0-9]{2}$` اور MOD-97 چیک ہندسے (ISO 17442) | قبولیت سے پہلے گلف ڈیلی ڈیلٹا فائلوں کے خلاف توثیق کریں۔ [^gleif] |
| ** بِک ** | ریجیکس `^[A-Z]{4}[A-Z]{2}[A-Z0-9]{2}([A-Z0-9]{3})?$` | اختیاری برانچ کوڈ (آخری تین چارس)۔ RA فائلوں میں فعال حیثیت کی تصدیق کریں۔ [^swift_bic] |
| ** مائک ** | آئی ایس او 10383 را فائل سے برقرار رکھیں۔ یقینی بنائیں کہ مقامات متحرک ہیں (نہیں `!` ٹرمینیشن پرچم) | اخراج سے پہلے پرچم کو ختم کرنے والے mics. [^iso_mic] |
| ** ابن ** | ملک سے مخصوص لمبائی ، اپر کیس الفانومیرک ، Mod-97 = 1 | سوئفٹ کے ذریعہ برقرار رکھنے والی رجسٹری کا استعمال کریں۔ ساختی طور پر غلط Ibans کو مسترد کریں۔ [^Swift_iban] |
| ** ملکیتی اکاؤنٹ/پارٹی آئی ڈی ** | `Max35Text` (UTF-8 ، ≤35 حروف) تراشے ہوئے وائٹ اسپیس کے ساتھ | `GenericAccountIdentification1.Id` اور `PartyIdentification135.Othr/Id` فیلڈز پر لاگو ہوتا ہے۔ 35 حروف سے زیادہ اندراجات کو مسترد کریں لہذا پل پے لوڈز آئی ایس او اسکیموں کے مطابق ہیں۔ |
| ** پراکسی اکاؤنٹ کی شناخت کرنے والے ** | Norito کے تحت Norito کے تحت `…/Prxy/Id` کے تحت غیر خالی `Max2048Text` | پرائمری ابن کے ساتھ ساتھ ذخیرہ ؛ پی وی پی ریلوں کو آئینہ بنانے کے لئے پراکسی ہینڈلز (اختیاری قسم کے کوڈ کے ساتھ) قبول کرتے ہوئے توثیق کے لئے ابھی بھی IBANs کی ضرورت ہے۔ |
| ** CFI ** | چھ کرداروں کا کوڈ ، آئی ایس او 10962 ٹیکسنومی کا استعمال کرتے ہوئے بڑے حرف | اختیاری افزودگی ؛ حروف کو یقینی بنائیں کہ آلے کی کلاس سے میچ کریں۔ [^iso_cfi] |
| ** فیسن ** | 35 حروف تک ، اپر کیس الفانومیرک پلس محدود اوقاف | اختیاری ؛ Truncate/معمول پر عمل کریں ISO 18774 رہنمائی۔ [^iso_fisn] |
| ** کرنسی ** | آئی ایس او 4217 3-لیٹر کوڈ ، معمولی اکائیوں کے ذریعہ طے شدہ اسکیل | اجازت دہندگان کی اجازت کے مطابق رقم لازمی ہے۔ Norito سائیڈ پر نافذ کریں۔ [^iso_4217] |

#### کراس واک اور ڈیٹا کی بحالی کی ذمہ داریوں- برقرار رکھیں ** ISIN ↔ Norito اثاثہ ID ** اور ** CUSIP ↔ ISIN ** کراس واکس۔ رات سے تازہ کاری کریں
  انا/ڈی ایس بی فیڈز اور ورژن سی آئی کے ذریعہ استعمال ہونے والے اسنیپ شاٹس کو کنٹرول کرتے ہیں۔ [^انا_کراس واک]
- ریفریش ** بِک ↔ لی ** گلف پبلک ریلیشن شپ فائلوں سے نقشہ سازی تاکہ پل کرسکیں
  جب ضرورت ہو تو دونوں کو خارج کریں۔ [^bic_lei]
- اسٹور ** مائک تعریفیں ** پل میٹا ڈیٹا کے ساتھ ساتھ پنڈال کی توثیق ہے
  یہاں تک کہ جب RA فائلیں مڈ ڈے میں تبدیل ہوجاتی ہیں۔ [^iso_mic]
- آڈٹ کے لئے برج میٹا ڈیٹا میں ڈیٹا پروویژن (ٹائم اسٹیمپ + ماخذ) ریکارڈ کریں۔ برقرار رکھیں
  خارج ہونے والی ہدایات کے ساتھ ساتھ اسنیپ شاٹ شناخت کنندہ۔
- ہر بھری ہوئی ڈیٹاسیٹ کی ایک کاپی کو برقرار رکھنے کے لئے `iso_bridge.reference_data.cache_dir` تشکیل دیں
  پروویژن میٹا ڈیٹا (ورژن ، ماخذ ، ٹائم اسٹیمپ ، چیکسم) کے ساتھ۔ یہ آڈیٹرز کی اجازت دیتا ہے
  اور آپریٹرز کو تاریخی فیڈ میں فرق کرنے کے لئے اپ اسٹریم اسنیپ شاٹس کے گھومنے کے بعد بھی۔
- آئی ایس او کراس واک اسنیپ شاٹس کا استعمال کرتے ہوئے `iroha_core::iso_bridge::reference_data` کے ذریعہ کھایا گیا ہے
  `iso_bridge.reference_data` کنفیگریشن بلاک (راستے + ریفریش وقفہ)۔ گیجز
  `iso_reference_status` ، `iso_reference_age_seconds` ، `iso_reference_records` ، اور
  `iso_reference_refresh_interval_secs` انتباہ کرنے کے لئے رن ٹائم ہیلتھ کو بے نقاب کریں۔ Torii
  برج `pacs.008` گذارشات کو مسترد کرتا ہے جن کے ایجنٹ BICs تشکیل سے غیر حاضر ہیں
  جب ہم منصب ہوتا ہے تو کراس واک ، سرفیسنگ ڈٹرمینسٹک `InvalidIdentifier` غلطیاں
  نامعلوم. 【کریٹس/آئروہ_ٹوری/ایس آر سی/آئی ایس او 20022_ برج۔ آر ایس#ایل 1078】
- ابان اور آئی ایس او 4217 پابندیوں کو اسی پرت میں نافذ کیا گیا ہے: Pacs.008/Pacs.009 اب بہاؤ
  `InvalidIdentifier` کی غلطیاں خارج کریں جب مقروض/قرض دہندگان IBans میں تشکیل شدہ عرفی یا کب کی کمی ہے
  تصفیہ کرنسی `currency_assets` سے غائب ہے ، جس سے خراب شدہ پل کی روک تھام ہے
  لیجر تک پہنچنے سے ہدایات۔ IBAN کی توثیق بھی ملک سے متعلق لاگو ہوتی ہے
  آئی ایس او 7064 Mod - 97 پاس سے پہلے لمبائی اور عددی چیک ہندسے اتنے ساختی طور پر غلط
  اقدار کو جلد ہی مسترد کردیا جاتا ہے۔ 【کریٹس/آئروہ_ٹوری/ایس آر سی/آئی ایس او 20022_ برج۔
- سی ایل آئی بستی کے مددگار ایک ہی گارڈ ریلوں کا وارث ہیں: پاس
  `--iso-reference-crosswalk <path>` کے ساتھ ساتھ `--delivery-instrument-id` DVP حاصل کرنے کے لئے
  `sese.023` XML اسنیپ شاٹ کو خارج کرنے سے پہلے پیش نظارہ آلہ کی شناخت کی توثیق کریں۔
- `cargo xtask iso-bridge-lint` (اور CI ریپر `ci/check_iso_reference_data.sh`) لنٹ
  کراس واک سنیپ شاٹس اور فکسچر۔ کمانڈ `--isin` ، `--bic-lei` ، `--mic` ، اور قبول کرتا ہے
  `--fixtures` جھنڈے اور واپس نمونہ ڈیٹاسیٹس پر واپس آتے ہیں `fixtures/iso_bridge/` جب چلتے ہیں
  دلائل کے بغیر۔
- IVM مددگار اب اصلی ISO 20022 XML لفافے (ہیڈ .001 + `DataPDU` + `Document`)
  اور `head.001` اسکیما کے ذریعہ بزنس ایپلی کیشن ہیڈر کی توثیق کرتا ہے لہذا `BizMsgIdr` ،
  `MsgDefIdr` ، `CreDt` ، اور BIC/CLRSYSMMBID ایجنٹوں کو عزم کے ساتھ محفوظ کیا گیا ہے۔ XMLDSIG/XADES
  بلاکس جان بوجھ کر چھوڑ دیتے ہیں۔ رجعت ٹیسٹ نمونے اور نئے استعمال کرتے ہیںمیپنگز کی حفاظت کے لئے ہیڈر لفافے کی حقیقت

#### ریگولیٹری اور مارکیٹ ڈھانچے کے تحفظات

- ** ٹی+1 تصفیہ **: یو ایس/کینیڈا ایکویٹی مارکیٹ 2024 میں ٹی+1 میں منتقل ہوگئی۔ Norito کو ایڈجسٹ کریں
  اس کے مطابق شیڈولنگ اور ایس ایل اے الرٹس۔ [^sec_t1] [^CSA_T1]
- ** CSDR جرمانے **: تصفیہ نظم و ضبط کے قواعد نقد جرمانے نافذ کرتے ہیں۔ Norito کو یقینی بنائیں
  میٹا ڈیٹا مفاہمت کے لئے جرمانے کے حوالہ جات پر قبضہ کرتا ہے۔ [^CSDR]
- ** اسی دن کے تصفیے کے پائلٹ **: ہندوستان کا ریگولیٹر T0/T+0 تصفیہ میں مرحلہ وار ہے۔ رکھیں
  پائلٹوں میں توسیع کے ساتھ ہی پل کیلنڈرز تازہ کاری کرتے ہیں۔ [^انڈیا_ٹی 0]
-** کولیٹرل بائ ان / ہولڈز **: خرید ان ٹائم لائنز اور اختیاری ہولڈز پر ESMA کی تازہ کاریوں کی نگرانی کریں
  تو مشروط ترسیل (`HldInd`) تازہ ترین رہنمائی کے ساتھ سیدھ میں ہے۔ [^CSDR]

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

### ترسیل- ورس-ادائیگی → `sese.023`| ڈی وی پی فیلڈ | آئی ایس او 20022 راہ | نوٹ |
| -------------------------------------------------------------------------------------------------------------- | ------- |
| `settlement_id` | `TxId` | مستحکم لائف سائیکل شناخت کنندہ |
| `delivery_leg.asset_definition_id` (سیکیورٹی) | `SctiesLeg/FinInstrmId` | کیننیکل شناخت کنندہ (isin ، cusip ،…) |
| `delivery_leg.quantity` | `SctiesLeg/Qty` | اعشاریہ تار ؛ آنرز اثاثہ صحت سے متعلق |
| `payment_leg.asset_definition_id` (کرنسی) | `CashLeg/Ccy` | آئی ایس او کرنسی کوڈ |
| `payment_leg.quantity` | `CashLeg/Amt` | اعشاریہ تار ؛ گول فی عددی قیاس |
| `delivery_leg.from` (بیچنے والا / فراہمی پارٹی) | `DlvrgSttlmPties/Pty/Bic` | شریک کی فراہمی کا BIC * (اکاؤنٹ کیننیکل ID فی الحال میٹا ڈیٹا میں برآمد کیا جاتا ہے) * |
| `delivery_leg.from` اکاؤنٹ شناخت کنندہ | `DlvrgSttlmPties/Acct` | فری فارم ؛ Norito میٹا ڈیٹا کا عین مطابق اکاؤنٹ ID ہے
| `delivery_leg.to` (خریدار / وصول کرنے والی پارٹی) | `RcvgSttlmPties/Pty/Bic` | حصہ لینے والے کو حاصل کرنے کا BIC |
| `delivery_leg.to` اکاؤنٹ شناخت کنندہ | `RcvgSttlmPties/Acct` | فری فارم ؛ اکاؤنٹ ID وصول کرنے والے میچ |
| `plan.order` | `Plan/ExecutionOrder` | ENUM: `DELIVERY_THEN_PAYMENT` یا `PAYMENT_THEN_DELIVERY` |
| `plan.atomicity` | `Plan/Atomicity` | ENUM: `ALL_OR_NOTHING` ، `COMMIT_FIRST_LEG` ، `COMMIT_SECOND_LEG` |
| ** پیغام کا مقصد ** | `SttlmTpAndAddtlParams/SctiesMvmntTp` | `DELI` (فراہمی) یا `RECE` (وصول) ؛ آئینے میں جو جمع کرانے والی پارٹی نے عمل کیا ہے۔ |
|                                                        | `SttlmTpAndAddtlParams/Pmt` | `APMT` (ادائیگی کے خلاف) یا `FREE` (مفت ادائیگی)۔ |
| `delivery_leg.metadata` ، `payment_leg.metadata` | `SctiesLeg/Metadata` ، `CashLeg/Metadata` | اختیاری Norito JSON کو UTF - 8 | کے بطور انکوڈ کیا گیا

> ** تصفیہ کوالیفائر ** - برج آئینے مارکیٹ پریکٹس تصفیے کے حالت کوڈ (`SttlmTxCond`) ، جزوی تصفیہ اشارے (`PrtlSttlmInd`) ، اور Norito میٹا ڈیٹا سے دوسرے اختیاری کوالیفائر جب `sese.023/025` میں I`sese.023/025` میں آئینہ دار ہے۔ آئی ایس او بیرونی کوڈ کی فہرستوں میں شائع ہونے والے گنتیوں کو نافذ کریں تاکہ منزل کا سی ایس ڈی اقدار کو تسلیم کرے۔

### ادائیگی- ورسیس ادائیگی کی مالی اعانت → `pacs.009`

نقد رقم کے لئے کیش ٹانگیں جو پی وی پی کی ہدایت کو فنڈ دیتے ہیں اسے FI-to-Fi کریڈٹ کے طور پر جاری کیا جاتا ہے
منتقلی پل ان ادائیگیوں کی تشریح کرتا ہے لہذا بہاو نظام کو پہچانتا ہے
وہ سیکیورٹیز کے تصفیے کی مالی اعانت کرتے ہیں۔| پی وی پی فنڈنگ ​​فیلڈ | آئی ایس او 20022 راہ | نوٹ |
| ---------------------------------------------- |
| `primary_leg.quantity` / {رقم ، کرنسی} | `IntrBkSttlmAmt` + `IntrBkSttlmCcy` | انیشیٹر سے رقم/کرنسی ڈیبٹ۔ |
| کاؤنٹرپارٹی ایجنٹ شناخت کنندہ | `InstgAgt` ، `InstdAgt` | ایجنٹ بھیجنے اور وصول کرنے کا BIC/LEI۔ |
| تصفیہ کا مقصد | `CdtTrfTxInf/PmtTpInf/CtgyPurp/Cd` | سیکیورٹیز سے متعلق پی وی پی فنڈنگ ​​کے لئے `SECU` پر سیٹ کریں۔ |
| Norito میٹا ڈیٹا (اکاؤنٹ IDS ، FX ڈیٹا) | `CdtTrfTxInf/SplmtryData` | مکمل اکاؤنٹڈ ، ایف ایکس ٹائم اسٹیمپس ، عملدرآمد کے منصوبے کے اشارے اٹھاتا ہے۔ |
| ہدایات شناخت کنندہ / لائف سائیکل لنکنگ | `CdtTrfTxInf/PmtId/InstrId` ، `CdtTrfTxInf/RmtInf` | Norito `settlement_id` سے میل کھاتا ہے تاکہ نقد ٹانگ سیکیورٹیز سائیڈ کے ساتھ صلح کرے۔ |

جاوا اسکرپٹ ایس ڈی کے کا آئی ایس او برج ڈیفالٹ کرکے اس ضرورت کے ساتھ سیدھ میں ہے
`pacs.009` زمرہ مقصد `SECU` ؛ کال کرنے والے اسے دوسرے کے ساتھ اوور رائڈ کرسکتے ہیں
غیر سکیورٹیز کریڈٹ ٹرانسفر کا اخراج کرتے وقت درست آئی ایس او کوڈ ، لیکن غلط
اقدار کو سامنے سے مسترد کردیا جاتا ہے۔

اگر کسی انفراسٹرکچر کو واضح سیکیورٹیز کی تصدیق کی ضرورت ہوتی ہے تو ، پل
`sese.025` کا اخراج جاری رکھے ہوئے ہے ، لیکن اس تصدیق سے سیکیورٹیز کی ٹانگ کی عکاسی ہوتی ہے
حیثیت (جیسے ، `ConfSts = ACCP`) PVP "مقصد" کے بجائے۔

### ادائیگی- ورسیس ادائیگی کی توثیق → `sese.025`

| پی وی پی فیلڈ | آئی ایس او 20022 راہ | نوٹ |
| ------------------------------------------------------------------------------------------------- | ------- |
| `settlement_id` | `TxId` | مستحکم لائف سائیکل شناخت کنندہ |
| `primary_leg.asset_definition_id` | `SttlmCcy` | پرائمری ٹانگ کے لئے کرنسی کا کوڈ |
| `primary_leg.quantity` | `SttlmAmt` | انیشی ایٹر کے ذریعہ فراہم کردہ رقم |
| `counter_leg.asset_definition_id` | `AddtlInf` (JSON پے لوڈ) | کاؤنٹر کرنسی کوڈ ضمنی معلومات میں سرایت شدہ |
| `counter_leg.quantity` | `SttlmQty` | کاؤنٹر رقم |
| `plan.order` | `Plan/ExecutionOrder` | ایک ہی اینوم سیٹ کے طور پر ڈی وی پی |
| `plan.atomicity` | `Plan/Atomicity` | ایک ہی اینوم سیٹ کے طور پر ڈی وی پی |
| `plan.atomicity` حیثیت (`ConfSts`) | `ConfSts` | `ACCP` جب مماثل ؛ پل مسترد ہونے پر ناکامی کے کوڈز کا اخراج |
| ہم منصب شناخت کنندہ | `AddtlInf` json | میٹا ڈیٹا میں موجودہ برج سیریلائزز مکمل اکاؤنٹڈ/بِک ٹپلس |

مثال (روابط ، ہولڈ ، اور مارکیٹ مائک کے ساتھ سی ایل آئی آئی ایس او پیش نظارہ):

```sh
iroha app settlement dvp \
  --settlement-id DVP-FIXTURE-1 \
  --delivery-asset security#equities \
  --delivery-quantity 500 \
  --delivery-from <katakana-i105-account-id> \
  --delivery-to <katakana-i105-account-id> \
  --payment-asset usd#fi \
  --payment-quantity 1050000 \
  --payment-from <katakana-i105-account-id> \
  --payment-to <katakana-i105-account-id> \
  --delivery-instrument-id US0378331005 \
  --place-of-settlement-mic XNAS \
  --partial-indicator npar \
  --hold-indicator \
  --settlement-condition NOMC \
  --linkage WITH:PACS009-CLS \
  --linkage BEFO:SUBST-PAIR-B \
  --iso-xml-out sese023_preview.xml
```

### ریپو کولیٹرل متبادل → `colr.007`| ریپو فیلڈ / سیاق و سباق | آئی ایس او 20022 راہ | نوٹ |
| ----------------------------------------------------------------------------------------------------------------------------- | ------- |
| `agreement_id` (`RepoIsi` / `ReverseRepoIsi`) | `OblgtnId` | ریپو معاہدہ شناخت کنندہ |
| کولیٹرل متبادل TX شناخت کنندہ | `TxId` | فی متبادل تیار کردہ |
| اصل خودکش مقدار میں | `Substitution/OriginalAmt` | متبادل سے پہلے کے وعدے سے وابستہ کولیٹرل سے ملتے ہیں
| اصل کولیٹرل کرنسی | `Substitution/OriginalCcy` | کرنسی کا کوڈ |
| متبادل کولیٹرل مقدار | `Substitution/SubstituteAmt` | تبدیلی کی رقم |
| متبادل کولیٹرل کرنسی | `Substitution/SubstituteCcy` | کرنسی کا کوڈ |
| مؤثر تاریخ (گورننس مارجن شیڈول) | `Substitution/EffectiveDt` | ISO تاریخ (YYYY-MM-DD) |
| بال کٹوانے کی درجہ بندی | `Substitution/Type` | فی الحال `FULL` یا `PARTIAL` گورننس پالیسی پر مبنی ہے
| گورننس کی وجہ / ہیئر کٹ نوٹ | `Substitution/ReasonCd` | اختیاری ، گورننس عقلیت اٹھاتا ہے
| بال کٹوانے کا سائز | `Substitution/Haircut` | عددی ؛ متبادل کے دوران لاگو بال کٹوانے کے نقشے |
| اصل/متبادل آلہ ids | `Substitution/OriginalFinInstrmId` ، `Substitution/SubstituteFinInstrmId` | ہر ٹانگ کے لئے اختیاری ISIN/CUSIP |

### فنڈنگ ​​اور بیانات

| Iroha سیاق و سباق | آئی ایس او 20022 پیغام | نقشہ سازی کا مقام |
| -------------------------------------- | ------------------------------------------------ |
| ریپو کیش ٹانگ اگنیشن / انوائنڈ | `pacs.009` | `IntrBkSttlmAmt` ، `IntrBkSttlmCcy` ، `IntrBkSttlmDt` ، `InstgAgt` ، `InstdAgt` DVP/PVP ٹانگوں سے آباد ہے |
| آبادکاری کے بعد کے بیانات | `camt.054` | ادائیگی کی ٹانگوں کی نقل و حرکت `Ntfctn/Ntry[*]` کے تحت ریکارڈ کی گئی۔ `SplmtryData` میں برج انجیکشن لیجر/اکاؤنٹ میٹا ڈیٹا

### استعمال نوٹس* Norito عددی مددگار (`NumericSpec`) کا استعمال کرتے ہوئے تمام مقدار میں سیریلائز کیا جاتا ہے
  اثاثوں کی تعریفوں میں پیمانے پر مطابقت کو یقینی بنانے کے لئے۔
* `TxId` اقدار `Max35Text` ہیں - UTF - 8 لمبائی ≤35 حروف کو نافذ کریں
  آئی ایس او 20022 کے پیغامات کو برآمد کرنا۔
* BICS 8 یا 11 بڑے حروف حروف حروف حروف (ISO9362) ہونا چاہئے۔ مسترد
  Norito میٹا ڈیٹا جو ادائیگیوں یا تصفیے سے خارج ہونے سے پہلے اس چیک کو ناکام کرتا ہے
  تصدیق
* اکاؤنٹ کے شناخت کنندگان (اکاؤنٹ آئی ڈی / چینیڈ) کو ضمیمہ میں برآمد کیا جاتا ہے
  میٹا ڈیٹا لہذا شرکاء کو وصول کرنے سے اپنے مقامی لیجرز کے خلاف صلح ہوسکتی ہے۔
* `SupplementaryData` کیننیکل JSON (UTF-8 ، ترتیب شدہ چابیاں ، JSON-native ہونا چاہئے
  فرار)۔ ایس ڈی کے مددگار اس پر دستخط ، ٹیلی میٹری ہیشوں اور آئی ایس او کو نافذ کرتے ہیں
  پے لوڈ آرکائیوز تعمیر نو میں دوبارہ تعمیراتی کام کرتے ہیں۔
* کرنسی کی مقدار ISO4217 فریکشن ہندسوں کی پیروی کرتی ہے (مثال کے طور پر JPY میں 0 ہے
  اعشاریہ ، امریکی ڈالر میں 2) ؛ اس کے مطابق پل Norito عددی صحت سے متعلق کلیمپ کرتا ہے۔
* سی ایل آئی بستی کے مددگار (`iroha app settlement ... --atomicity ...`) اب خارج ہوجاتے ہیں
  Norito ہدایات جن کے عمل درآمد کا منصوبہ ہے نقشہ 1: 1 سے `Plan/ExecutionOrder` اور
  `Plan/Atomicity` اوپر
* آئی ایس او ہیلپر (`ivm::iso20022`) مذکورہ بالا فیلڈز کی توثیق کرتا ہے اور مسترد کرتا ہے
  وہ پیغامات جہاں ڈی وی پی/پی وی پی ٹانگوں میں عددی چشمی یا ہم منصب کے باہمی تعاون کی خلاف ورزی ہوتی ہے۔

### SDK بلڈر مددگار

- جاوا اسکرپٹ SDK اب `buildPacs008Message` / کو بے نقاب کرتا ہے
  `buildPacs009Message` (`javascript/iroha_js/src/isoBridge.js` دیکھیں) تو کلائنٹ
  آٹومیشن ساختی تصفیہ میٹا ڈیٹا (BIC/LEI ، IBANS ،
  مقصد کے کوڈز ، ضمنی Norito فیلڈز) میں عین مطابق PACS XML میں
  اس گائیڈ سے نقشہ سازی کے قواعد کو معاوضہ دیئے بغیر۔
- دونوں مددگاروں کو ایک واضح `creationDateTime` (ٹائم زون کے ساتھ ISO - 8601) کی ضرورت ہوتی ہے۔
  لہذا آپریٹرز کو اس کے بجائے اپنے ورک فلو سے ایک عین مطابق ٹائم اسٹیمپ کو تھریڈ کرنا ہوگا
  SDK کو دیوار گھڑی کے وقت کو ڈیفالٹ ہونے دینا۔
- `recipes/iso_bridge_builder.mjs` یہ ظاہر کرتا ہے کہ ان مددگاروں کو کس طرح تار لگایا جائے
  ایک CLI جو ماحولیاتی متغیرات یا JSON کنفیگ فائلوں کو ضم کرتا ہے ، پرنٹ کرتا ہے
  تیار کردہ XML ، اور اختیاری طور پر اسے Torii (`ISO_SUBMIT=1`) پر جمع کرواتا ہے ، دوبارہ استعمال کرتے ہوئے
  آئی ایس او برج کی ترکیب کی طرح ہی انتظار کیڈینس۔


### حوالہ جات

- لکس سی ایس ڈی/کلیئر اسٹریم آئی ایس او 20022 تصفیے کی مثالوں میں `SttlmTpAndAddtlParams/SctiesMvmntTp` (`DELI`/`RECE`) اور `Pmt` (Norito/`FREE`). (`FREE`). (`FREE`).
- کلیئر اسٹریم ڈی سی پی کی وضاحتیں طے شدہ کوالیفائر (`SttlmTxCond` ، `PrtlSttlmInd`) پر محیط ہیں
- سیکیورٹیز سے متعلق پی وی پی فنڈنگ ​​کے لئے `CtgyPurp/Cd = SECU` کے ساتھ `pacs.009` کی سفارش کرنے والی سوئفٹ پی ایم پی جی رہنمائی۔
- آئی ایس او 20022 شناخت کنندہ لمبائی کی رکاوٹوں (بی آئی سی ، میکس 35 ٹیکسٹ) کے لئے پیغام کی تعریف کی رپورٹیں۔
- ISIN فارمیٹ اور چیکسم کے قواعد پر انا DSB رہنمائی۔

### استعمال کے نکات- ہمیشہ متعلقہ Norito اسنیپٹ یا CLI کمانڈ چسپاں کریں تاکہ LLM معائنہ کرسکے
  عین مطابق فیلڈ کے نام اور عددی ترازو۔
- کاغذی پگڈنڈی رکھنے کے لئے حوالہ جات (`provide clause references`) کی درخواست کریں
  تعمیل اور آڈیٹر کا جائزہ۔
- `docs/source/finance/settlement_iso_mapping.md` میں جواب کا خلاصہ حاصل کریں
  (یا منسلک ضمیمہ) لہذا مستقبل کے انجینئروں کو استفسار کو دہرانے کی ضرورت نہیں ہے۔

## ایونٹ آرڈرنگ پلے بوکس (آئی ایس او 20022 ↔ Norito برج)

### منظر نامہ A - کولیٹرل متبادل (ریپو / عہد)

** شرکاء: ** کولیٹرل دینے والا/لینے والا (اور/یا ایجنٹ) ، کسٹوڈین (ایس) ، سی ایس ڈی/ٹی 2 ایس  
** وقت: ** فی مارکیٹ کٹ آف اور ٹی 2 ایس دن/رات کے چکروں ؛ دونوں پیروں کو آرکیسٹیٹ کریں تاکہ وہ ایک ہی تصفیہ ونڈو میں مکمل ہوں۔

#### پیغام کوریوگرافی
1. `colr.010` کولیٹرل متبادل کی درخواست → کولیٹرل دینے والا/ٹیکر یا ایجنٹ۔  
2. `colr.011` کولیٹرل متبادل ردعمل → قبول/مسترد (اختیاری مسترد ہونے کی وجہ)۔  
3. `colr.012` کولیٹرل متبادل کی تصدیق → متبادل کے معاہدے کی تصدیق کرتی ہے۔  
4. `sese.023` ہدایات (دو ٹانگیں):  
   - اصل کولیٹرل (`SctiesMvmntTp=DELI` ، `Pmt=FREE` ، `SctiesTxTp=COLO`) لوٹائیں۔  
   - متبادل کولیٹرل (`SctiesMvmntTp=RECE` ، `Pmt=FREE` ، `SctiesTxTp=COLI`) فراہم کریں۔  
   جوڑی کو لنک کریں (نیچے ملاحظہ کریں)  
5. `sese.024` اسٹیٹس ایڈوائز (قبول ، مماثل ، زیر التواء ، ناکام ، مسترد)۔  
6. `sese.025` تصدیق ایک بار بک کروانے کے بعد۔  
7. اختیاری نقد ڈیلٹا (فیس/بال کٹوانے) → `pacs.009` FI-TO-FI کریڈٹ ٹرانسفر `CtgyPurp/Cd = SECU` کے ساتھ ؛ `pacs.002` کے ذریعے حیثیت ، `pacs.004` کے ذریعے لوٹتا ہے۔

#### مطلوبہ اعتراف / حیثیت
- ٹرانسپورٹ کی سطح: گیٹ ویز `admi.007` کا اخراج کرسکتے ہیں یا کاروباری پروسیسنگ سے پہلے مسترد کرسکتے ہیں۔  
- تصفیہ لائف سائیکل: `sese.024` (پروسیسنگ اسٹیٹس + وجہ کوڈز) ، `sese.025` (حتمی)۔  
- کیش سائیڈ: `pacs.002` (`PDNG` ، `ACSC` ، `RJCT` وغیرہ) ، واپسی کے لئے `pacs.004`۔

#### مشروط / انوائنڈ فیلڈز
- `SctiesSttlmTxInstr/Lnkgs` (`WITH`/`BEFO`/`AFTE`) دو ہدایات کی زنجیر کے ل .۔  
- `SttlmParams/HldInd` جب تک معیار پورا نہیں ہوتا ہے اس وقت تک ؛ `sese.030` (`sese.031` حیثیت) کے ذریعے جاری کریں۔  
- `SttlmParams/PrtlSttlmInd` جزوی تصفیے پر قابو پانے کے لئے (`NPAR` ، `PART` ، `PARC` ، `PARQ`)۔  
- `SttlmParams/SttlmTxCond/Cd` مارکیٹ سے متعلق حالات (`NOMC` ، وغیرہ) کے لئے۔  
- اختیاری T2S مشروط سیکیورٹیز ڈلیوری (COSD) کے قواعد جب تائید کرتے ہیں۔

#### حوالہ جات
- سوئفٹ کولیٹرل مینجمنٹ MDR (`colr.010/011/012`)۔  
- لنکنگ اور اسٹیٹس کے لئے CSD/T2S استعمال گائیڈ (جیسے ، DNB ، ECB بصیرت)۔  
- ایس ایم پی جی تصفیہ کی مشق ، کلیئر اسٹریم ڈی سی پی دستورالعمل ، اے ایس ایکس آئی ایس او ورکشاپس۔

### منظر نامہ بی - ایف ایکس ونڈو کی خلاف ورزی (پی وی پی فنڈنگ ​​میں ناکامی)

** شرکاء: ** ہم منصب اور نقد ایجنٹ ، سیکیورٹیز کسٹوڈین ، سی ایس ڈی/ٹی 2 ایس  
** وقت: ** ایف ایکس پی وی پی ونڈوز (سی ایل ایس/دو طرفہ) اور سی ایس ڈی کٹ آفس ؛ سیکیورٹیز کی ٹانگوں کو زیر التواء نقد تصدیق پر رکھیں۔#### پیغام کوریوگرافی
1. `pacs.009` FI-TO-FI کریڈٹ ٹرانسفر فی کرنسی `CtgyPurp/Cd = SECU` کے ساتھ ؛ `pacs.002` کے ذریعے حیثیت ؛ `camt.056`/`camt.029` کے ذریعے یاد کریں/منسوخ کریں۔ اگر پہلے ہی طے شدہ ہے تو ، `pacs.004` واپس آجائیں۔  
2. `sese.023` DVP ہدایات (S) `HldInd=true` کے ساتھ لہذا سیکیورٹیز کی ٹانگ نقد تصدیق کا انتظار کرتی ہے۔  
3. لائف سائیکل `sese.024` نوٹسز (قبول شدہ/مماثل/زیر التواء)۔  
4. اگر دونوں `pacs.009` ٹانگوں میں `ACSC` تک پہنچ جاتا ہے اس سے پہلے ونڈو کی میعاد ختم ہوجاتی ہے I `sese.030` → `sese.031` (MOD کی حیثیت) → `sese.025` (تصدیق) کے ساتھ رہائی۔  
5. اگر ایف ایکس ونڈو کی خلاف ورزی کی گئی ہے → منسوخ/یاد کیش (`camt.056/029` یا `pacs.004`) اور سیکیورٹیز کو منسوخ کریں (`sese.020` + `sese.027` ، یا `sese.026` ریورسال اگر پہلے ہی تصدیق شدہ فی مارکیٹ رول)۔

#### مطلوبہ اعتراف / حیثیت
- کیش: `pacs.002` (`PDNG` ، `ACSC` ، `RJCT`) ، واپسی کے لئے `pacs.004`۔  
- سیکیورٹیز: `sese.024` (زیر التواء/ناکام وجوہات جیسے `NORE` ، `ADEA`) ، `sese.025`۔  
- ٹرانسپورٹ: `admi.007` / گیٹ وے بزنس پروسیسنگ سے پہلے مسترد کرتا ہے۔

#### مشروط / انوائنڈ فیلڈز
- `SttlmParams/HldInd` + `sese.030` ریلیز/کامیابی/ناکامی پر منسوخ کریں۔  
- `Lnkgs` سیکیورٹیز کی ہدایات کو نقد ٹانگ سے باندھنے کے لئے۔  
- اگر مشروط ترسیل کا استعمال کرتے ہو تو T2S COSD قاعدہ۔  
- `PrtlSttlmInd` غیر ارادتا پارٹیلوں کو روکنے کے لئے۔  
- `pacs.009` پر ، `CtgyPurp/Cd = SECU` پرچم سیکیورٹیز سے متعلق فنڈنگ۔

#### حوالہ جات
- سیکیورٹیز کے عمل میں ادائیگی کے لئے پی ایم پی جی / سی بی پی آر+ رہنمائی۔  
- ایس ایم پی جی تصفیہ کے طریقوں ، لنک/ہولڈز پر T2S بصیرت۔  
- کلیئر اسٹریم ڈی سی پی دستورالعمل ، بحالی کے پیغامات کے لئے ای سی ایم ایس دستاویزات۔

### pacs.004 ریٹرن میپنگ نوٹ

- ریٹرن فکسچر اب `ChrgBr` کو معمول پر لائیں (`DEBT`/`CRED`/`SHAR`/`SLEV`) اور PROIRITARY RATERSTENTEREST اور آپریٹر کی وجوہات `TxInf[*]/RtrdRsn/Prtry` کے بغیر سامنے ہیں۔
- `DataPDU` لفافوں کے اندر APPHDR دستخطی بلاکس کو نظرانداز کیا جاتا ہے۔ آڈٹ کو ایمبیڈڈ ایکس ایم ایل ڈی ایس آئی جی فیلڈز کے بجائے چینل پروویننس پر انحصار کرنا چاہئے۔

### پل کے لئے آپریشنل چیک لسٹ
- مذکورہ بالا کوریوگرافی کو نافذ کریں (کولیٹرل: `colr.010/011/012 → sese.023/024/025` ؛ FX خلاف ورزی: ​​`pacs.009 (+pacs.002) → sese.023 held → release/cancel`)۔  
- `sese.024`/`sese.025` اسٹیٹس اور `pacs.002` نتائج کو گیٹنگ سگنلز کے طور پر علاج کریں۔ `ACSC` ٹرگرز ریلیز ، `RJCT` فورسز انوائنڈ۔  
- `HldInd` ، `Lnkgs` ، `PrtlSttlmInd` ، `SttlmTxCond` ، اور اختیاری COSD قواعد کے ذریعے انکوڈ مشروط ترسیل۔  
- جب ضرورت ہو تو بیرونی IDs (جیسے ، `pacs.009` کے لئے UERT) کے لئے `SupplementaryData` استعمال کریں۔  
- مارکیٹ کیلنڈر/کٹ آفس کے ذریعہ پیرامیٹرائز ہولڈ/انوائنڈ ٹائمنگ ؛ `sese.030`/`camt.056` جاری کریں ، منسوخی کی آخری تاریخ سے پہلے ، جب ضروری ہو تو واپسی پر فال بیک۔

### نمونہ آئی ایس او 20022 پے لوڈ (تشریح)

#### کولیٹرل متبادل جوڑی (`sese.023`) ہدایات کے ساتھ تعلق کے ساتھ

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
````SctiesMvmntTp=RECE` ، `Pmt=FREE` ، اور `WITH` لنکج کے ساتھ منسلک ہدایات `SUBST-2025-04-001-B` (FOP وصول کریں) ، `WITH` لنکج کو جمع کریں۔ ایک بار متبادل کی منظوری کے بعد دونوں ٹانگوں کو مماثل `sese.030` کے ساتھ جاری کریں۔

#### سیکیورٹیز کی ٹانگ پر ہولڈ پر زیر التواء ایف ایکس تصدیق (`sese.023` + `sese.030`)

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

ایک بار جاری کریں `pacs.009` ٹانگوں میں `ACSC` تک پہنچیں:

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

`sese.031` ہولڈ ریلیز کی تصدیق کرتا ہے ، اس کے بعد سیکیورٹیز کی ٹانگ بک ہونے کے بعد `sese.025` ہوتا ہے۔

#### PVP فنڈنگ ​​ٹانگ (`pacs.009` سیکیورٹیز کے مقصد کے ساتھ)

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

`pacs.002` ادائیگی کی حیثیت سے پتہ چلتا ہے (`ACSC` = تصدیق شدہ ، `RJCT` = مسترد)۔ اگر ونڈو کی خلاف ورزی کی گئی ہے تو ، `camt.056`/`camt.029` کے ذریعے یاد کریں یا آباد فنڈز واپس کرنے کے لئے `pacs.004` بھیجیں۔
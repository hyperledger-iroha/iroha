---
lang: ar
direction: rtl
source: docs/portal/docs/finance/settlement-iso-mapping.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: رسم الخرائط تسوية ISO
العنوان: التسوية ↔ رسم الخرائط الميدانية ISO 20022
Sidebar_label: التسوية ↔ ISO 20022
الوصف: رسم الخرائط الأساسية بين تدفقات التسوية Iroha وجسر ISO 20022.
---

:::ملاحظة المصدر الكنسي
:::

## التسوية ↔ رسم الخرائط الميدانية ISO 20022

تلتقط هذه المذكرة التعيين الأساسي بين تعليمات التسوية Iroha
(`DvpIsi`، `PvpIsi`، تدفقات ضمانات الريبو) ورسائل ISO 20022 التي تم تطبيقها
بالجسر. إنه يعكس سقالات الرسالة المطبقة فيها
`crates/ivm/src/iso20022.rs` ويعمل كمرجع عند إنتاج أو
التحقق من صحة حمولات Norito.

### سياسة البيانات المرجعية (المعرفات والتحقق من صحتها)

تتضمن هذه السياسة تفضيلات المعرفات وقواعد التحقق من الصحة والبيانات المرجعية
الالتزامات التي يجب أن ينفذها الجسر Norito ↔ ISO 20022 قبل إرسال الرسائل.

** نقاط الربط داخل رسالة ISO: **
- **معرفات الأجهزة** → `delivery_leg.asset_definition_id` ↔ `SctiesLeg/FinInstrmId`
  (أو مجال الأداة المكافئة).
- **الأطراف/الوكلاء** → `DlvrgSttlmPties/Pty` و`RcvgSttlmPties/Pty` لـ `sese.*`،
  أو بنيات الوكيل في `pacs.009`.
- **الحسابات** → عناصر `…/Acct` للحفظ/الحسابات النقدية؛ مرآة على دفتر الأستاذ
  `AccountId` في `SupplementaryData`.
- **معرفات الملكية** → `…/OthrId` مع `Tp/Prtry` ومعكوسة في
  `SupplementaryData`. لا تستبدل أبدًا المعرفات المنظمة بمعرفات خاصة.

#### تفضيلات المعرف حسب عائلة الرسائل

##### `sese.023` / `.024` / `.025` (تسوية الأوراق المالية)

- **الصك (`FinInstrmId`)**
  - المفضل: **ISIN** تحت `…/ISIN`. إنه المعرف الأساسي لـ CSDs / T2S.[^anna]
  - الإحتياطيات:
    - **CUSIP** أو NSIN آخر ضمن `…/OthrId/Id` مع `Tp/Cd` المعين من ISO الخارجي
      قائمة الرموز (على سبيل المثال، `CUSP`)؛ قم بتضمين جهة الإصدار في `Issr` عند التفويض.[^iso_mdr]
    - **معرف الأصل Norito** كملكية: `…/OthrId/Id`، و`Tp/Prtry="NORITO_ASSET_ID"`، و
      سجل نفس القيمة في `SupplementaryData`.
  - الواصفات الاختيارية: **CFI** (`ClssfctnTp`) و**FISN** حيث يتم دعمها لتسهيل الأمر
    المصالحة.[^iso_cfi][^iso_fisn]
- **الأطراف (`DlvrgSttlmPties`، `RcvgSttlmPties`)**
  - المفضل: **BIC** (`AnyBIC/BICFI`، ISO 9362).[^swift_bic]
  - الإجراء الاحتياطي: **معرّف الكيان القانوني** حيث يكشف إصدار الرسالة عن حقل معرّف الكيان القانوني المخصص؛ إذا
    غائبة، وتحمل معرفات خاصة مع تسميات `Prtry` واضحة وتضمين BIC في البيانات الوصفية.[^iso_cr]
- **مكان التسوية/المكان** → **MIC** للمكان و **BIC** لـ CSD.[^iso_mic]

##### `colr.010` / `.011` / `.012` و`colr.007` (إدارة الضمانات)

- اتبع نفس قواعد الأداة مثل `sese.*` (يفضل رقم ISIN).
- تستخدم الأطراف **BIC** بشكل افتراضي؛ **معرّف الكيان القانوني** مقبول عندما يعرضه المخطط.[^swift_bic]
- يجب أن تستخدم المبالغ النقدية رموز العملة **ISO 4217** مع الوحدات الثانوية الصحيحة.[^iso_4217]

##### `pacs.009` / `camt.054` (تمويل وبيانات PvP)

- **الوكلاء (`InstgAgt`، `InstdAgt`، وكلاء المدين/الدائن)** → **BIC** مع اختياري
  معرّف الكيان القانوني حيثما كان مسموحًا به.[^swift_bic]
- **الحسابات**
  - Interbank: تحديد بواسطة **BIC** ومراجع الحساب الداخلي.
  - البيانات التي تواجه العملاء (`camt.054`): تتضمن **IBAN** عند وجودها والتحقق من صحتها
    (الطول، قواعد الدولة، المجموع الاختباري mod-97).[^swift_iban]
- **العملة** → **ISO 4217** رمز مكون من 3 أحرف، مع مراعاة تقريب الوحدات الصغيرة.[^iso_4217]
- **عرض Torii** → أرسل تمويل PvP عبر `POST /v2/iso20022/pacs009`؛ الجسر
  يتطلب `Purp=SECU` ويفرض الآن ممرات BIC عند تكوين البيانات المرجعية.

#### قواعد التحقق (تنطبق قبل الانبعاثات)

| المعرف | قاعدة التحقق | ملاحظات |
|------------|-----------------|-------|
| **الرقم الدولي** | رقم التحقق من Regex `^[A-Z]{2}[A-Z0-9]{9}[0-9]$` وLuhn (mod-10) وفقًا لمرفق ISO 6166 C | رفض قبل انبعاث الجسر؛ تفضل التخصيب في المنبع.[^anna_luhn] |
| ** كوسيب ** | Regex `^[A-Z0-9]{9}$` وmodulus-10 مع وزنين (خريطة الأحرف إلى أرقام) | فقط عندما يكون رقم التعريف الدولي غير متوفر؛ الخريطة عبر معبر ANNA/CUSIP بمجرد الحصول عليها.[^cusip] |
| **معرّف الكيان القانوني** | Regex `^[A-Z0-9]{18}[0-9]{2}$` ورقم التحقق mod-97 (ISO 17442) | تحقق من صحة ملفات دلتا اليومية لـ GLEIF قبل القبول.[^gleif] |
| ** بيك ** | ريجكس `^[A-Z]{4}[A-Z]{2}[A-Z0-9]{2}([A-Z0-9]{3})?$` | رمز الفرع الاختياري (آخر ثلاثة أحرف). تأكيد الحالة النشطة في ملفات RA.[^swift_bic] |
| **الميكروفون** | الاحتفاظ بملف ISO 10383 RA؛ تأكد من أن الأماكن نشطة (لا توجد علامة إنهاء `!`) | قم بوضع علامة على MICs التي تم إيقاف تشغيلها قبل الإصدار.[^iso_mic] |
| **رقم الحساب المصرفي الدولي** | الطول الخاص بالبلد، أبجدي رقمي كبير، mod-97 = 1 | استخدام السجل الذي تحتفظ به SWIFT؛ رفض أرقام IBAN غير الصالحة هيكليًا.[^swift_iban] |
| **حساب الملكية/معرفات الطرف** | `Max35Text` (UTF-8، ≥35 حرفًا) مع مسافة بيضاء مشذبة | ينطبق على الحقول `GenericAccountIdentification1.Id` و`PartyIdentification135.Othr/Id`. ارفض الإدخالات التي تتجاوز 35 حرفًا حتى تتوافق حمولات الجسر مع مخططات ISO. |
| **معرفات حساب الوكيل** | `Max2048Text` غير فارغ ضمن `…/Prxy/Id` مع رموز النوع الاختيارية في `…/Prxy/Tp/{Cd,Prtry}` | مخزنة بجانب رقم الحساب المصرفي الدولي (IBAN) الأساسي؛ لا يزال التحقق من الصحة يتطلب أرقام IBAN مع قبول مقابض الوكيل (مع رموز النوع الاختيارية) لعكس مسارات PvP. |
| **CFI** | رمز مكون من ستة أحرف، بأحرف كبيرة باستخدام تصنيف ISO 10962 | الإثراء الاختياري؛ تأكد من تطابق الأحرف مع فئة الآلة.[^iso_cfi] |
| ** فيسن ** | ما يصل إلى 35 حرفًا، وأحرف أبجدية رقمية كبيرة بالإضافة إلى علامات ترقيم محدودة | خياري؛ اقتطاع/تسوية وفقًا لتوجيهات ISO 18774.[^iso_fisn] |
| **العملة** | رمز ISO 4217 مكون من 3 أحرف، ويتم تحديد المقياس بواسطة وحدات ثانوية | يجب تقريب المبالغ إلى الكسور العشرية المسموح بها؛ فرض على الجانب Norito.[^iso_4217] |

#### التزامات عبور المشاة وصيانة البيانات

- الحفاظ على **ISIN ↔ Norito معرف الأصل** و **CUSIP ↔ ISIN** ممرات المشاة. تحديث ليلا من
  تتحكم خلاصات ANNA/DSB وإصدارها في اللقطات التي يستخدمها CI.[^anna_crosswalk]
- قم بتحديث تعيينات **BIC ↔ LEI** من ملفات العلاقات العامة لـ GLEIF حتى يتمكن الجسر من
  ينبعث كلاهما عند الحاجة.[^bic_lei]
- قم بتخزين **تعريفات MIC** جنبًا إلى جنب مع بيانات تعريف الجسر حتى يتم التحقق من صحة المكان
  حتمية حتى عندما تتغير ملفات RA في منتصف النهار.[^iso_mic]
- تسجيل مصدر البيانات (الطابع الزمني + المصدر) في بيانات تعريف الجسر للتدقيق. الإصرار على
  معرف اللقطة بجانب التعليمات الصادرة.
- قم بتكوين `iso_bridge.reference_data.cache_dir` للاحتفاظ بنسخة من كل مجموعة بيانات تم تحميلها
  جنبًا إلى جنب مع بيانات تعريف المصدر (الإصدار، المصدر، الطابع الزمني، المجموع الاختباري). وهذا يسمح لمراجعي الحسابات
  ويقوم المشغلون بتمييز الخلاصات التاريخية حتى بعد تدوير اللقطات الأولية.
- يتم استيعاب لقطات ممرات ISO بواسطة `iroha_core::iso_bridge::reference_data` باستخدام
  كتلة التكوين `iso_bridge.reference_data` (المسارات + الفاصل الزمني للتحديث). مقاييس
  `iso_reference_status`، `iso_reference_age_seconds`، `iso_reference_records`، و
  يعرض `iso_reference_refresh_interval_secs` صحة وقت التشغيل للتنبيه. Torii
  يرفض الجسر عمليات إرسال `pacs.008` التي تكون رموز BIC للوكيل غائبة عن التكوين
  معبر المشاة، وإظهار الأخطاء الحتمية `InvalidIdentifier` عندما يكون الطرف المقابل
  غير معروف.[الصناديق/iroha_torii/src/iso20022_bridge.rs#L1078]
- يتم فرض روابط IBAN وISO 4217 على نفس الطبقة: يتدفق pacs.008/pacs.009 الآن
  إصدار أخطاء `InvalidIdentifier` عندما تفتقر أرقام IBAN للمدين/الدائن إلى الأسماء المستعارة التي تم تكوينها أو عندما
  عملة التسوية مفقودة من `currency_assets`، مما يمنع الجسر التالف
  تعليمات من الوصول إلى دفتر الأستاذ. ينطبق التحقق من رقم IBAN أيضًا على كل بلد
  تعتبر الأطوال وأرقام التحقق الرقمية قبل ISO 7064 mod‑97 غير صالحة من الناحية الهيكلية
  تم رفض القيم مبكرًا. 【crates/iroha_torii/src/iso20022_bridge.rs#L775 】 【crates/iroha_torii/src/iso20022_bridge.rs#L827 】 【crates/ivm/src/iso20022.rs#L1255】
- يرث مساعدو تسوية CLI نفس حواجز الحماية: المرور
  `--iso-reference-crosswalk <path>` بجانب `--delivery-instrument-id` للحصول على DvP
  معاينة التحقق من صحة معرفات الأداة قبل إرسال لقطة `sese.023` XML.
- `cargo xtask iso-bridge-lint` (والمجمع CI `ci/check_iso_reference_data.sh`) الوبر
  لقطات وتركيبات معبر المشاة. يقبل الأمر `--isin`، و`--bic-lei`، و`--mic`، و
  علامات `--fixtures` وتعود إلى مجموعات البيانات النموذجية في `fixtures/iso_bridge/` عند التشغيل
  بدون وسيطات.[xtask/src/main.rs#L146] 【ci/check_iso_reference_data.sh#L1】
- يقوم المساعد IVM الآن باستيعاب مظاريف ISO 20022 XML الحقيقية (head.001 + `DataPDU` + `Document`)
  والتحقق من صحة رأس تطبيق الأعمال عبر مخطط `head.001` لذلك `BizMsgIdr`،
  يتم الاحتفاظ بعوامل `MsgDefIdr` و`CreDt` وBIC/ClrSysMmbId بشكل حتمي؛ XMLDSig/XAdES
  تبقى الكتل تم تخطيها عمدا. 

#### الاعتبارات التنظيمية وهيكل السوق- **تسوية T+1**: انتقلت أسواق الأسهم الأمريكية/الكندية إلى T+1 في عام 2024؛ ضبط Norito
  الجدولة وتنبيهات SLA وفقًا لذلك.[^sec_t1][^csa_t1]
- **عقوبات CSDR**: تفرض قواعد الانضباط في التسوية غرامات نقدية؛ تأكد من Norito
  تلتقط البيانات الوصفية مراجع العقوبة للتسوية.[^csdr]
- **البرامج التجريبية للتسوية في نفس اليوم**: تعمل الهيئة التنظيمية في الهند على تنفيذ تسوية T0/T+0 تدريجيًا؛ احتفظ
  تم تحديث تقاويم الجسر مع توسع الطيارين.[^india_t0]
- ** عمليات الشراء الإضافية / عمليات الحجز الإضافية **: مراقبة تحديثات هيئة الأوراق المالية والأسواق (ESMA) بشأن الجداول الزمنية لعمليات الشراء وعمليات الحجز الاختيارية
  لذا فإن التسليم المشروط (`HldInd`) يتوافق مع أحدث الإرشادات.[^csdr]

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

### التسليم مقابل الدفع → `sese.023`

| حقل DvP | مسار ISO 20022 | ملاحظات |
|--------------------------------------------------------|-------------------------------------------------------|-------|
| `settlement_id` | `TxId` | معرف دورة الحياة المستقر |
| `delivery_leg.asset_definition_id` (الأمان) | `SctiesLeg/FinInstrmId` | المعرف الأساسي (ISIN، CUSIP، …) |
| `delivery_leg.quantity` | `SctiesLeg/Qty` | سلسلة عشرية؛ يكرم دقة الأصول |
| `payment_leg.asset_definition_id` (العملة) | `CashLeg/Ccy` | رمز العملة ISO |
| `payment_leg.quantity` | `CashLeg/Amt` | سلسلة عشرية؛ تقريبًا حسب المواصفات الرقمية |
| `delivery_leg.from` (البائع / طرف التسليم) | `DlvrgSttlmPties/Pty/Bic` | BIC الخاص بتسليم المشارك *(يتم تصدير معرف الحساب الأساسي حاليًا في البيانات الوصفية)* |
| معرف الحساب `delivery_leg.from` | `DlvrgSttlmPties/Acct` | شكل حر؛ تحمل البيانات التعريفية Norito معرف الحساب الدقيق |
| `delivery_leg.to` (المشتري / الطرف المتلقي) | `RcvgSttlmPties/Pty/Bic` | BIC لاستقبال المشارك |
| معرف الحساب `delivery_leg.to` | `RcvgSttlmPties/Acct` | شكل حر؛ يطابق معرف حساب الاستلام |
| `plan.order` | `Plan/ExecutionOrder` | التعداد: `DELIVERY_THEN_PAYMENT` أو `PAYMENT_THEN_DELIVERY` |
| `plan.atomicity` | `Plan/Atomicity` | التعداد: `ALL_OR_NOTHING`، `COMMIT_FIRST_LEG`، `COMMIT_SECOND_LEG` |
| **غرض الرسالة** | `SttlmTpAndAddtlParams/SctiesMvmntTp` | `DELI` (تسليم) أو `RECE` (تلقي)؛ المرايا التي ينفذها الطرف المقدم. |
|                                                        | `SttlmTpAndAddtlParams/Pmt` | `APMT` (مقابل الدفع) أو `FREE` (بدون دفع). |
| `delivery_leg.metadata`، `payment_leg.metadata` | `SctiesLeg/Metadata`، `CashLeg/Metadata` | اختياري Norito JSON مشفر بـ UTF‑8 |

> **مؤهلات التسوية** - يعكس الجسر ممارسات السوق عن طريق نسخ رموز شروط التسوية (`SttlmTxCond`)، ومؤشرات التسوية الجزئية (`PrtlSttlmInd`)، والمؤهلات الاختيارية الأخرى من البيانات التعريفية Norito إلى `sese.023/025` عند وجودها. قم بفرض التعدادات المنشورة في قوائم الأكواد الخارجية لـ ISO حتى تتعرف CSD الوجهة على القيم.

### تمويل الدفع مقابل الدفع → `pacs.009`

يتم إصدار أرجل النقد مقابل النقد التي تمول تعليمات حماية الأصناف النباتية كائتمان من FI إلى FI
التحويلات. يوضح الجسر هذه المدفوعات حتى تتعرف عليها الأنظمة النهائية
يقومون بتمويل تسوية الأوراق المالية.

| مجال تمويل حماية الأصناف النباتية | مسار ISO 20022 | ملاحظات |
|--------------------------------|-----------------------------------------------------|-------|
| `primary_leg.quantity` / {المبلغ، العملة} | `IntrBkSttlmAmt` + `IntrBkSttlmCcy` | المبلغ/العملة المخصومة من البادئ. |
| معرفات وكيل الطرف المقابل | `InstgAgt`، `InstdAgt` | BIC/LEI لوكلاء الإرسال والاستقبال. |
| غرض التسوية | `CdtTrfTxInf/PmtTpInf/CtgyPurp/Cd` | تم التعيين على `SECU` لتمويل حماية الأصناف النباتية المتعلق بالأوراق المالية. |
| البيانات الوصفية Norito (معرفات الحساب، بيانات العملات الأجنبية) | `CdtTrfTxInf/SplmtryData` | يحمل معرف الحساب الكامل، والطوابع الزمنية للعملات الأجنبية، وتلميحات خطة التنفيذ. |
| معرف التعليمات/ربط دورة الحياة | `CdtTrfTxInf/PmtId/InstrId`، `CdtTrfTxInf/RmtInf` | يطابق Norito `settlement_id` بحيث يتصالح الساق النقدية مع جانب الأوراق المالية. |

يتوافق جسر ISO الخاص بـ JavaScript SDK مع هذا المتطلب من خلال الإعداد الافتراضي
غرض الفئة `pacs.009` إلى `SECU`؛ يمكن للمتصلين تجاوزه بآخر
رمز ISO صالح عند إرسال تحويلات ائتمانية غير متعلقة بالأوراق المالية، ولكنه غير صالح
يتم رفض القيم مقدما.

إذا كانت البنية التحتية تتطلب تأكيدًا صريحًا للأوراق المالية، فسيتم استخدام الجسر
يستمر في إصدار `sese.025`، لكن هذا التأكيد يعكس ساق الأوراق المالية
الحالة (على سبيل المثال، `ConfSts = ACCP`) بدلاً من "غرض" حماية الأصناف النباتية.

### تأكيد الدفع مقابل الدفع → `sese.025`

| مجال حماية الأصناف النباتية | مسار ISO 20022 | ملاحظات |
|---------------------------------------------------------------|----------|-------|
| `settlement_id` | `TxId` | معرف دورة الحياة المستقر |
| `primary_leg.asset_definition_id` | `SttlmCcy` | رمز العملة للمحطة الأساسية |
| `primary_leg.quantity` | `SttlmAmt` | المبلغ المسلم بواسطة البادئ |
| `counter_leg.asset_definition_id` | `AddtlInf` (حمولة JSON) | رمز العملة المضاد مضمن في المعلومات التكميلية |
| `counter_leg.quantity` | `SttlmQty` | مبلغ العداد |
| `plan.order` | `Plan/ExecutionOrder` | تم تعيين نفس التعداد مثل DvP |
| `plan.atomicity` | `Plan/Atomicity` | تم تعيين نفس التعداد مثل DvP |
| حالة `plan.atomicity` (`ConfSts`) | `ConfSts` | `ACCP` عند المطابقة؛ يُصدر الجسر رموز الفشل عند الرفض |
| معرفات الطرف المقابل | `AddtlInf` JSON | يقوم الجسر الحالي بإجراء تسلسل لصفوف AccountId/BIC الكاملة في بيانات التعريف |

### استبدال ضمانات الريبو → `colr.007`

| حقل الريبو / السياق | مسار ISO 20022 | ملاحظات |
|--------------------------------------------------|----------------------------------------||
| `agreement_id` (`RepoIsi` / `ReverseRepoIsi`) | `OblgtnId` | معرف عقد الريبو |
| معرف Tx لاستبدال الضمانات | `TxId` | تم إنشاؤها لكل بديل |
| كمية الضمانات الأصلية | `Substitution/OriginalAmt` | المباريات المرهونة بالضمانات قبل الاستبدال |
| العملة الإضافية الأصلية | `Substitution/OriginalCcy` | رمز العملة |
| كمية الضمانات البديلة | `Substitution/SubstituteAmt` | مبلغ الاستبدال |
| عملة الضمان البديلة | `Substitution/SubstituteCcy` | رمز العملة |
| تاريخ النفاذ (جدول هامش الحوكمة) | `Substitution/EffectiveDt` | تاريخ ISO (YYYY-MM-DD) |
| تصنيف قصات الشعر | `Substitution/Type` | حاليًا `FULL` أو `PARTIAL` استنادًا إلى سياسة الإدارة |
| سبب الحكم / مذكرة قص الشعر | `Substitution/ReasonCd` | اختياري، يحمل الأساس المنطقي للحوكمة |

### التمويل والبيانات

| سياق Iroha | رسالة ISO 20022 | تحديد الموقع |
|----------------------------------|-----------------------------------|------------------|
| إشعال / استرخاء ساق الريبو النقدية | `pacs.009` | `IntrBkSttlmAmt`، `IntrBkSttlmCcy`، `IntrBkSttlmDt`، `InstgAgt`، `InstdAgt` يتم ملؤها من أرجل DvP/PvP |
| تصريحات ما بعد التسوية | `camt.054` | حركات ساق الدفع المسجلة تحت `Ntfctn/Ntry[*]`؛ يقوم Bridge بإدخال بيانات تعريف دفتر الأستاذ/الحساب في `SplmtryData` |

### ملاحظات الاستخدام* يتم إجراء تسلسل لجميع المبالغ باستخدام المساعدين الرقميين Norito (`NumericSpec`)
  لضمان توافق المقياس عبر تعريفات الأصول.
* قيم `TxId` هي `Max35Text` - فرض طول UTF-8 أقل من 35 حرفًا قبل
  التصدير إلى رسائل ISO 20022.
* يجب أن تتكون رموز BIC من 8 أو 11 حرفًا أبجديًا رقميًا كبيرًا (ISO9362)؛ رفض
  بيانات التعريف Norito التي تفشل في هذا الفحص قبل إرسال الدفعات أو التسوية
  التأكيدات.
* يتم تصدير معرفات الحساب (AccountId / ChainId) إلى ملفات تكميلية
  البيانات الوصفية حتى يتمكن المشاركون المستقبلون من التوفيق بين دفاتر الأستاذ المحلية الخاصة بهم.
* `SupplementaryData` يجب أن يكون JSON أساسيًا (UTF‑8، مفاتيح مرتبة، JSON أصلي
  الهروب). يقوم مساعدو SDK بفرض ذلك من خلال التوقيعات وتجزئة القياس عن بعد وISO
  تظل أرشيفات الحمولة النافعة حتمية عبر عمليات إعادة البناء.
* تتبع مبالغ العملة أرقام الكسر ISO4217 (على سبيل المثال، الين الياباني لديه 0
  الكسور العشرية، الدولار الأمريكي لديه 2)؛ يتم تثبيت الجسر بدقة رقمية Norito وفقًا لذلك.
* ينبعث الآن مساعدو تسوية CLI (`iroha app settlement ... --atomicity ...`).
  تعليمات Norito التي يتم تعيين خطط تنفيذها 1:1 إلى `Plan/ExecutionOrder` و
  `Plan/Atomicity` أعلاه.
* يقوم مساعد ISO (`ivm::iso20022`) بالتحقق من صحة الحقول المذكورة أعلاه ويرفضها
  الرسائل التي تنتهك فيها أرجل DvP/PvP المواصفات الرقمية أو معاملة الطرف المقابل.

### مساعدو منشئ SDK

- يعرض JavaScript SDK الآن `buildPacs008Message` /
  `buildPacs009Message` (راجع `javascript/iroha_js/src/isoBridge.js`) لذا العميل
  يمكن للأتمتة تحويل البيانات التعريفية للتسوية المنظمة (BIC/LEI، IBANs،
  رموز الغرض، حقول Norito التكميلية) في pacs الحتمية XML
  دون إعادة تطبيق قواعد التعيين من هذا الدليل.
- يتطلب كلا المساعدين رقم `creationDateTime` واضحًا (ISO‑8601 مع المنطقة الزمنية)
  لذلك يجب على المشغلين ربط طابع زمني محدد من سير العمل الخاص بهم بدلاً من ذلك
  السماح لـ SDK بالتوقيت الافتراضي لساعة الحائط.
- يوضح `recipes/iso_bridge_builder.mjs` كيفية توصيل هؤلاء المساعدين
  تقوم واجهة سطر الأوامر (CLI) التي تدمج متغيرات البيئة أو ملفات تكوين JSON، بطباعة ملف
  تم إنشاء XML، وإرساله اختياريًا إلى Torii (`ISO_SUBMIT=1`)، وإعادة استخدامه
  نفس إيقاع الانتظار مثل وصفة جسر ISO.


### المراجع

- أمثلة تسوية LuxCSD / Clearstream ISO 20022 تظهر `SttlmTpAndAddtlParams/SctiesMvmntTp` (`DELI`/`RECE`) و`Pmt` (`APMT`/`FREE`).<sup>[1](https://www.luxcsd.com/resource/blob/3434074/6f8add4708407a4701055be4dd04846b/c23005-eis-examples-cbf-data.pdf)</sup>
- مواصفات Clearstream DCP التي تغطي مؤهلات التسوية (`SttlmTxCond`، `PrtlSttlmInd`).<sup>[2](https://www.clearstream.com/clearstream-en/res-library/market-coverage/instruction-specifications-swift-iso-20022-dcp-mode-ceu-spain-2357008)</sup>
- توصي إرشادات SWIFT PMPG بـ `pacs.009` مع `CtgyPurp/Cd = SECU` لتمويل PvP المتعلق بالأوراق المالية.<sup>[3](https://www.swift.com/swift-resource/251897/download)</sup>
- تقارير تعريف رسالة ISO 20022 لقيود طول المعرف (BIC، Max35Text).<sup>[4](https://www.iso20022.org/sites/default/files/2020-12/ISO20022_MDRPart2_ChangeOrVerifyAccountIdentification_2020_2021_v1_ForSEGReview.pdf)</sup>
- إرشادات ANNA DSB بشأن تنسيق ISIN وقواعد المجموع الاختباري.<sup>[5](https://www.anna-dsb.com/isin/)</sup>

### نصائح الاستخدام

- الصق دائمًا مقتطف Norito أو أمر CLI ذي الصلة حتى تتمكن LLM من الفحص
  أسماء الحقول الدقيقة والمقاييس الرقمية.
- طلب الاستشهادات (`provide clause references`) للاحتفاظ بسجل ورقي لها
  الامتثال ومراجعة المراجع.
- التقط ملخص الإجابة في `docs/source/finance/settlement_iso_mapping.md`
  (أو الملاحق المرتبطة) لذلك لا يحتاج مهندسو المستقبل إلى تكرار الاستعلام.

## أدلة تشغيل ترتيب الأحداث (ISO 20022 ↔ Norito Bridge)

### السيناريو أ - استبدال الضمانات (الريبو / التعهد)

**المشاركين:** مانح/متلقي الضمانات (و/أو الوكلاء)، الوصي (الأمناء)، CSD/T2S  
**التوقيت:** حسب فترات انقطاع السوق ودورات T2S النهارية/الليلية؛ قم بتنسيق الساقين بحيث يكتملان داخل نفس نافذة التسوية.

#### تصميم الرقصات للرسالة
1. `colr.010` طلب استبدال الضمانات ← مانح/متلقي أو وكيل الضمانات.  
2. `colr.011` الاستجابة لاستبدال الضمانات → قبول/رفض (سبب رفض اختياري).  
3. `colr.012` تأكيد استبدال الضمانات → يؤكد اتفاقية الاستبدال.  
4. تعليمات `sese.023` (قدمين):  
   - إرجاع الضمانات الأصلية (`SctiesMvmntTp=DELI`، `Pmt=FREE`، `SctiesTxTp=COLO`).  
   - تسليم الضمانات البديلة (`SctiesMvmntTp=RECE`، `Pmt=FREE`، `SctiesTxTp=COLI`).  
   ربط الزوج (انظر أدناه).  
5. نصائح الحالة `sese.024` (مقبول، مطابق، معلق، فاشل، مرفوض).  
6. تم حجز تأكيدات `sese.025` بمجرد حجزها.  
7. دلتا نقدية اختيارية (الرسوم/قص الشعر) → `pacs.009` تحويل رصيد FI-to-FI باستخدام `CtgyPurp/Cd = SECU`؛ الحالة عبر `pacs.002`، وترجع عبر `pacs.004`.

#### الإقرارات / الحالات المطلوبة
- مستوى النقل: قد تصدر البوابات `admi.007` أو ترفضها قبل معالجة الأعمال.  
- دورة حياة التسوية: `sese.024` (حالات المعالجة + أكواد السبب)، `sese.025` (نهائي).  
- الجانب النقدي: `pacs.002` (`PDNG`، `ACSC`، `RJCT` وما إلى ذلك)، `pacs.004` للمرتجعات.

#### حقول الشرط / الاسترخاء
- `SctiesSttlmTxInstr/Lnkgs` (`WITH`/`BEFO`/`AFTE`) لتسلسل التعليمتين.  
- `SttlmParams/HldInd` للاحتفاظ به حتى استيفاء المعايير؛ الإصدار عبر `sese.030` (حالة `sese.031`).  
- `SttlmParams/PrtlSttlmInd` للتحكم في التسوية الجزئية (`NPAR`، `PART`، `PARC`، `PARQ`).  
- `SttlmParams/SttlmTxCond/Cd` للظروف الخاصة بالسوق (`NOMC`، وما إلى ذلك).  
- قواعد تسليم الأوراق المالية المشروطة (CoSD) الاختيارية T2S عند دعمها.

#### المراجع
- إدارة ضمانات SWIFT MDR (`colr.010/011/012`).  
- أدلة استخدام CSD/T2S (على سبيل المثال، DNB وECB Insights) للربط والحالات.  
- ممارسة تسوية SMPG وأدلة Clearstream DCP وورش عمل ASX ISO.

### السيناريو ب — خرق نافذة العملات الأجنبية (فشل تمويل حماية الأصناف النباتية)

**المشاركين:** الأطراف المقابلة ووكلاء النقد، أمين الأوراق المالية، CSD/T2S  
**التوقيت:** نوافذ FX PvP (CLS/ثنائية) وقطع CSD؛ إبقاء أرجل الأوراق المالية معلقة في انتظار التأكيد النقدي.

#### تصميم الرقصات للرسالة
1. `pacs.009` تحويل رصيد FI-to-FI لكل عملة باستخدام `CtgyPurp/Cd = SECU`؛ الحالة عبر `pacs.002`؛ الاستدعاء/الإلغاء عبر `camt.056`/`camt.029`؛ إذا تمت تسويته بالفعل، فسيتم إرجاع `pacs.004`.  
2. تعليمات (تعليمات) `sese.023` DvP مع `HldInd=true`، لذا تنتظر ساق الأوراق المالية التأكيد النقدي.  
3. إشعارات دورة الحياة `sese.024` (مقبولة/متطابقة/معلقة).  
4. إذا وصل كلا ساقي `pacs.009` إلى `ACSC` قبل انتهاء صلاحية النافذة → حرر باستخدام `sese.030` → `sese.031` (حالة التعديل) → `sese.025` (تأكيد).  
5. إذا تم اختراق نافذة العملات الأجنبية → قم بإلغاء/استدعاء النقد (`camt.056/029` أو `pacs.004`) وإلغاء الأوراق المالية (`sese.020` + `sese.027`، أو عكس `sese.026` إذا تم تأكيده بالفعل وفقًا لقاعدة السوق).

#### الإقرارات / الحالات المطلوبة
- نقدًا: `pacs.002` (`PDNG`، `ACSC`، `RJCT`)، `pacs.004` للمرتجعات.  
- الأوراق المالية: `sese.024` (أسباب معلقة/فشل مثل `NORE`، `ADEA`)، `sese.025`.  
- النقل: `admi.007` / البوابة ترفض قبل معالجة الأعمال.

#### حقول الشرط / الاسترخاء
- إصدار/إلغاء `SttlmParams/HldInd` + `sese.030` عند النجاح/الفشل.  
- `Lnkgs` لربط تعليمات الأوراق المالية بالساق النقدية.  
- قاعدة T2S CoSD في حالة استخدام التسليم المشروط.  
- `PrtlSttlmInd` لمنع الأجزاء غير المقصودة.  
- في `pacs.009`، يشير `CtgyPurp/Cd = SECU` إلى التمويل المتعلق بالأوراق المالية.

#### المراجع
- إرشادات PMPG / CBPR+ للمدفوعات في عمليات الأوراق المالية.  
- ممارسات تسوية SMPG، ورؤى T2S حول الربط/الحجز.  
- أدلة Clearstream DCP ووثائق ECMS لرسائل الصيانة.

### pacs.004 إرجاع ملاحظات التعيين

- تعمل تركيبات الإرجاع الآن على تطبيع `ChrgBr` (`DEBT`/`CRED`/`SHAR`/`SLEV`) وأسباب إرجاع الملكية المكشوفة كـ `TxInf[*]/RtrdRsn/Prtry`، بحيث يمكن لعملاء الجسر إعادة تشغيل إسناد الرسوم ورموز المشغل دون إعادة تحليل مغلف XML.
- تظل كتل توقيع AppHdr داخل مظاريف `DataPDU` متجاهلة عند الاستيعاب؛ يجب أن تعتمد عمليات التدقيق على مصدر القناة بدلاً من حقول XMLDSIG المضمنة.

### قائمة المراجعة التشغيلية للجسر
- فرض تصميم الرقصات أعلاه (الضمان: `colr.010/011/012 → sese.023/024/025`؛ خرق FX: `pacs.009 (+pacs.002) → sese.023 held → release/cancel`).  
- التعامل مع حالات `sese.024`/`sese.025` ونتائج `pacs.002` كإشارات بوابة؛ يؤدي `ACSC` إلى إطلاق الإصدار، ويفرض `RJCT` الاسترخاء.  
- تشفير التسليم المشروط عبر `HldInd`، و`Lnkgs`، و`PrtlSttlmInd`، و`SttlmTxCond`، وقواعد CoSD الاختيارية.  
- استخدم `SupplementaryData` لربط المعرفات الخارجية (على سبيل المثال، UETR لـ `pacs.009`) عند الحاجة.  
- ضبط توقيت التوقف/الاسترخاء حسب تقويم السوق/فترات التوقف؛ قم بإصدار `sese.030`/`camt.056` قبل المواعيد النهائية للإلغاء، وقم بالرجوع إلى عمليات الإرجاع عند الضرورة.

### نموذج لحمولات ISO 20022 (مشروحة)

#### زوج استبدال الضمانات (`sese.023`) مع رابط التعليمات

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

أرسل التعليمات المرتبطة `SUBST-2025-04-001-B` (استلام FoP للضمانات البديلة) مع `SctiesMvmntTp=RECE`، و`Pmt=FREE`، والارتباط `WITH` الذي يشير إلى `SUBST-2025-04-001-A`. حرر كلا الساقين باستخدام `sese.030` المطابق بمجرد الموافقة على الاستبدال.

#### ساق الأوراق المالية معلقة في انتظار تأكيد العملات الأجنبية (`sese.023` + `sese.030`)

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
```حرر بمجرد وصول كلا ساقي `pacs.009` إلى `ACSC`:

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

يؤكد `sese.031` على تحرير التعليق، متبوعًا بـ `sese.025` بمجرد حجز ساق الأوراق المالية.

#### ساق تمويل حماية الأصناف النباتية (`pacs.009` لغرض الأوراق المالية)

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

يتتبع `pacs.002` حالة الدفع (`ACSC` = مؤكد، `RJCT` = رفض). إذا تم اختراق النافذة، فاسترجع عبر `camt.056`/`camt.029` أو أرسل `pacs.004` لإرجاع الأموال المستقرة.
---
lang: ar
direction: rtl
source: docs/account_structure.md
status: complete
translator: manual
source_hash: 7e6a1321c6f8d71ac4b576a55146767fbc488b29c7e21d82bc2e1c55db89769c
source_last_modified: "2025-11-12T00:36:40.117854+00:00"
translation_last_reviewed: 2025-11-14
---

<div dir="rtl">

<!-- docs/account_structure.md (Account Structure RFC) کا عربی ترجمہ -->

# RFC هيكلية الحسابات

**الحالة:** مقبول (ADDR‑1)  
**الجمهور المستهدف:** فرق نموذج البيانات، Torii، Nexus، المحافظ (Wallet) والحوكمة  
**القضايا ذات الصلة:** سيتم تحديدها لاحقًا

## الملخّص

يقترح هذا المستند إعادة تصميم شاملة لمخطط عنونة الحسابات بهدف:

- تقديم عنوان **Iroha Bech32m (IH‑B32)** قابل للقراءة من البشر، مزوّد
  بـ checksum، يربط مميّز السلسلة (chain discriminant) بموقّع الحساب ويقدّم
  تمثيلات نصية حتمية مناسبة للتشغيل البيني.
- تقديم معرّفات نطاق (domain identifiers) عالمية التفرد، مدعومة بسجل يمكن
  الاستعلام عنه عبر Nexus لأغراض التوجيه بين السلاسل.
- توفير طبقات توافقية تبقي على عمل aliasات التوجيه `alias@domain` أثناء
  ترحيل المحافظ، وواجهات الـ API، والعقود إلى الصيغة الجديدة.

## الدوافع

تعتمد المحافظ وأدوات الـ off‑chain حاليًا على aliasات التوجيه الخام من
النمط `alias@domain`. وهذا يسبب مشكلتين رئيسيتين:

1. **غياب ارتباط الشبكة.** لا تحتوي السلسلة النصية على checksum أو
   prefix خاص بالشبكة، لذا يمكن للمستخدم لصق عنوان من شبكة خاطئة دون
   تنبيه فوري. في نهاية المطاف، سيتم رفض المعاملة (بسبب عدم تطابق
   `chain_id`) أو قد تُنفَّذ على حساب غير مقصود إذا كان هذا الحساب موجودًا
   محليًا.
2. **تصادم في أسماء النطاقات.** النطاقات عبارة عن مساحة أسماء فقط ويمكن
   إعادة استخدامها في كل سلسلة. تصبح فدرلة الخدمات (مثل الحاضنين،
   الجسور، وتدفقات العمل cross‑chain) هشّة، لأن `finance` على السلسلة A لا
   يرتبط بـ `finance` على السلسلة B.

نحتاج إلى صيغة عنوان صديقة للبشر تحمي من أخطاء النسخ/اللصق، وإلى
تعيين (mapping) حتمي بين اسم النطاق والسلسلة المرجعية (authoritative).

## الأهداف

- تصميم غلاف عنوان Bech32m مستوحى من IH58 لحسابات Iroha، مع نشر aliasات
  نصية معيارية وفق CAIP‑10.
- ترميز مميّز السلسلة (chain discriminant) المُهيّأ مباشرة داخل كل عنوان،
  وتحديد عملية الحوكمة/التسجيل الخاصة به.
- شرح كيفية إدخال سجل نطاقات عالمي دون كسر النشرات (deployments)
  الحالية، وتحديد قواعد للتطبيع (normalization) ومقاومة الانتحال
  (anti‑spoofing).
- توثيق توقعات التوافقية، وخطوات الترحيل، والأسئلة المفتوحة.

## ما ليس ضمن النطاق

- تنفيذ تحويلات أصول cross‑chain. طبقة التوجيه تعيد فقط السلسلة الهدف.
- تغيير البنية الداخلية لـ `AccountId` (تبقى بصيغة
  `DomainId + PublicKey`).
- حسم حوكمة إصدار النطاقات العالمية. يركّز هذا RFC على نموذج البيانات
  وبدائيات النقل (transport primitives).

## الخلفية

### Alias التوجيه الحالي

```text
AccountId {
    domain: DomainId,   // غلاف فوق Name (سلسلة أحرف قريبة من ASCII)
    signatory: PublicKey // سلسلة multihash (مثل ed0120...)
}

Display / Parse: "<signatory multihash>@<domain name>"

يُعامل هذا التمثيل النصي الآن كـ **alias للحساب**: اختصار للتوجيه يشير
إلى [`AccountAddress`](#2-canonical-address-codecs) الكانوني. يبقى هذا
الشكل مفيدًا لقابلية القراءة البشرية ولحوكمة النطاق، لكنه لم يعد
المعرّف الحاسم للحساب على السلسلة.
```

يعيش `ChainId` خارج `AccountId`. تتحقق العقد من `ChainId` الخاص بالمعاملة
مقابل الإعدادات أثناء القبول
(`AcceptTransactionFail::ChainIdMismatch`) وترفض المعاملات القادمة من
سلاسل أخرى، غير أن سلسلة النص للحساب لا تحمل أي دلالة على الشبكة.

### معرّفات النطاقات

يلفّ `DomainId` حول الاسم `Name` (سلسلة مُطَبَّعة) ويكون محصورًا في
السلسلة المحلية. يمكن لكل سلسلة أن تسجّل نطاقات مثل `wonderland`,
`finance` بشكل مستقل.

### سياق Nexus

يتولى Nexus مسؤولية التنسيق بين المكوّنات (lanes/data‑spaces). حاليًا لا
يمتلك أي مفهوم للتوجيه بين السلاسل بناءً على النطاقات.

## التصميم المقترح

### 1. مميّز سلسلة (Chain Discriminant) حتمي

يتم توسيع البنية `iroha_config::parameters::actual::Common` لتشمل:

```rust
pub struct Common {
    pub chain: ChainId,
    pub chain_discriminant: u16, // حقل جديد، منسّق عالميًا
    // ... الحقول الحالية
}
```

- **قيود:**
  - يجب أن يكون فريدًا لكل شبكة نشطة؛ تتم إدارته عبر سجل عام موقَّع مع
    نطاقات محجوزة صراحة (مثلًا `0x0000–0x0FFF` للبيئات التجريبية،
    `0x1000–0x7FFF` لتخصيصات المجتمع، `0x8000–0xFFEF` لنطاقات معتمدة من
    الحوكمة، و `0xFFF0–0xFFFF` محجوزة).
  - غير قابل للتغيير لسلسلة قيد التشغيل. تغييره يتطلّب hard fork وتحديث
    في السجل.
- **الحوكمة والسجل:** يحتفظ كيان حوكمة متعدد التواقيع بسجل JSON موقَّع
  (ومثبّت على IPFS) يقوم بربط مميّزات السلاسل (discriminants) مع
  aliasات بشرية ومعرّفات CAIP‑2. تحصل العملاء على هذا السجل وتخزّنه في
  cache للتحقّق من metadata الخاصة بالسلسلة وعرضها.
- **الاستخدام:** يتم تمرير القيمة عبر طبقات القبول (state admission)، و
  Torii، وSDKs، وواجهات المحافظ بحيث يتمكّن كل مكوّن من تضمينها أو
  التحقّق منها. يتم تعريضها كجزء من CAIP‑2 (مثلًا `iroha:0x1234`).

<a id="2-canonical-address-codecs"></a>

### 2. ترميزات العنوان الكانونية

يعرض نموذج البيانات في Rust تمثيلًا ثنائيًا كاونونيًا واحدًا
(`AccountAddress`) يمكن تحويله إلى عدة صيغ موجهة للبشر:

- **IH58 (Iroha Base58):** غلاف Base58 يحتوي على مميّز السلسلة (chain
  discriminant). يتحقّق الـ decoder من الـ prefix قبل ترقية الـ payload إلى
  الشكل الكانوني.
- **عرض مضغوط خاص بـ Sora:** أبجدية مخصّصة لـ Sora تحتوي على **105 رمزًا**
  تُبنى عبر إضافة القصيدة イロハ بصيغة half‑width (بما في ذلك ヰ و ヱ) إلى
  مجموعة IH58 ذات الـ 58 حرفًا. تبدأ السلاسل بالـ sentinel `snx1`، وتضمّ
  checksum مشتقًا من Bech32m، وتُسقط prefix الشبكة (يُفترض Sora Nexus من
  خلال الـ sentinel).

  ```text
  IH58  : 123456789ABCDEFGHJKMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz
  Iroha : ｲﾛﾊﾆﾎﾍﾄﾁﾘﾇﾙｦﾜｶﾖﾀﾚｿﾂﾈﾅﾗﾑｳヰﾉｵｸﾔﾏｹﾌｺｴﾃｱｻｷﾕﾒﾐｼヱﾋﾓｾｽ
  ```
- **Hex كاونوني:** تمثيل `0x…` يناسب أغراض debug للغلاف الثنائي الكانوني.

تقوم `AccountAddress::parse_any` باكتشاف الإدخالات المضغوطة أو IH58 أو
hex الكانونية تلقائيًا، وتعيد payload مفكوكًا مع نوع
`AccountAddressFormat` المكتشف. يستدعي Torii هذه الدالة من أجل
`ISO 20022 supplementary addresses` ويخزّن الشكل hex الكانوني، بحيث تبقى
الـ metadata حتمية بصرف النظر عن التمثيل الأصلي.

### 2.1 بنية بايت الترويسة (ADDR‑1a)

يُنظَّم كل payload كاونوني بالشكل `header · domain selector · controller`.
تحتوي `header` على بايت واحد يحدد قواعد الـ parsing للأجزاء التالية:

```text
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

هذا البايت يجمع metadata المخطط (schema) التي يحتاجها الـ decoder:

| البتات | الحقل           | القيم المسموح بها | الخطأ عند المخالفة |
|--------|-----------------|--------------------|--------------------|
| ‎7‑5   | `addr_version`  | `0` (الإصدار 1). القيم `1‑7` محجوزة لإصدارات مستقبلية. | قيم خارج `0‑7` تثير `AccountAddressError::InvalidHeaderVersion`؛ ويجب التعامل مع أي قيمة غير صفرية على أنها غير مدعومة حاليًا. |
| ‎4‑3   | `addr_class`    | `0` = مفتاح منفرد، `1` = multisig. | أي قيمة أخرى تنتج `AccountAddressError::UnknownAddressClass`. |
| ‎2‑1   | `norm_version`  | `1` (Norm v1). القيم `0`، `2`، `3` محجوزة. | قيم خارج `0‑3` تثير `AccountAddressError::InvalidNormVersion`. |
| 0      | `ext_flag`      | يجب أن تكون `0`. | إذا كان البت مفعّلًا تُرفع `AccountAddressError::UnexpectedExtensionFlag`. |

Encoder Rust يكتب دائمًا القيمة `0x02` (إصدار 0، فئة مفتاح منفرد، نسخة
تطبيع 1، وعلم الامتداد ext_flag يساوي صفر).

### 2.2 ترميز محدِّد النطاق (Domain Selector) (ADDR‑1a)

يأتي محدِّد النطاق مباشرة بعد الترويسة ويُنفّذ كـ tagged union:

| Tag   | المعنى                     | الـ payload | ملاحظات |
|-------|----------------------------|------------|---------|
| `0x00` | نطاق افتراضي ضمني           | لا شيء      | يطابق `default_domain_name()` من إعدادات العقدة. |
| `0x01` | Digest لنطاق محلي          | 12 بايت     | digest = `blake2s_mac(key = "SORA-LOCAL-K:v1", canonical_label)[0..12]`. |
| `0x02` | مدخل في السجل العالمي      | 4 بايت      | `registry_id` بصيغة big‑endian؛ محجوز لحين إطلاق السجل العالمي. |

تُطبَّع أسماء النطاقات (UTS‑46 + STD3 + NFC) قبل حساب الـ hash. تؤدي
الـ tags غير المعروفة إلى `AccountAddressError::UnknownDomainTag`. وعند
مقارنة عنوان بدومين معيّن، ينتج عن عدم تطابق المحدِّد
`AccountAddressError::DomainMismatch`.

```text
domain selector
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind, see table)│
└──────────┴──────────────────────────────────────────────┘
```

يقع محدِّد النطاق مباشرة قبل الـ controller، بحيث يستطيع الـ decoder
سيرًا تسلسليًا قراءة بايت tag، ثم payload الخاصة به، ثم الانتقال إلى
بايتات الـ controller.

**أمثلة على المحدِّد (selector):**

- *النطاق الافتراضي الضمني* (`tag = 0x00`): لا payload. مثال على hex
  كاونوني للنطاق الافتراضي مع مفتاح اختباري حتمي:
  `0x0200000120641297079357229f295938a4b5a333de35069bf47b9d0704e45805713d13c201`.
- *Digest محلي* (`tag = 0x01`): الـ payload هي digest بطول 12 بايت. مثال
  (`treasury`, seed `0x01`):
  `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a20fd68c8329fe3bbfbecd26a2d72878cd827f8`.
- *مدخل في السجل العالمي* (`tag = 0x02`): الـ payload هو `registry_id:u32`
  بصيغة big‑endian؛ ما يلي الـ payload مطابق لحالة النطاق الافتراضي
  الضمني؛ الفرق أن المحدِّد يشير إلى سجل عالمي بدلًا من اسم النطاق
  المطبَّع. مثال مع `registry_id = 0x0000_002A` (أي 42 في النظام العشري)
  والمتحكم (controller) الافتراضي:
  `0x02020000002a000120641297079357229f295938a4b5a333de35069bf47b9d0704e45805713d13c201`.

### 2.3 ترميز الـ Controller Payload (ADDR‑1a)

يُعتبر payload الخاص بالـ controller اتحادًا Tagged آخر يُلحَق بعد
محدِّد النطاق:

| Tag   | نوع الـ Controller | التخطيط (Layout) | ملاحظات |
|-------|--------------------|------------------|---------|
| `0x00` | مفتاح منفرد        | `curve_id:u8` · `key_len:u8` · `key_bytes` | `curve_id = 0x01` تعني Ed25519 حاليًا. حقل `key_len` من نوع `u8`؛ القيم الأكبر تؤدي إلى `AccountAddressError::KeyPayloadTooLong`. |
| `0x01` | Multisig           | `version:u8` · `threshold:u16` · `member_count:u8` · (`curve_id:u8` · `weight:u16` · `key_len:u16` · `key_bytes`)\* | يدعم حتى 255 عضوًا (`CONTROLLER_MULTISIG_MEMBER_MAX`). المنحنيات (curves) غير المعروفة ترفع `AccountAddressError::UnknownCurve`، وسياسات multisig غير الصحيحة ترفع `AccountAddressError::InvalidMultisigPolicy`. |

تعرض سياسات الـ multisig أيضًا خريطة CBOR بأسلوب CTAP2 وdigest كاونوني، بحيث
يمكن للـ hosts والـ SDKs التحقّق من الـ controller بصورة حتمية. راجع
`docs/source/references/address_norm_v1.md` (ADDR‑1c) لمعرفة مخطّط البيانات
وآلية الـ hashing والـ fixtures المرجعية.

تُسلسَل بايتات المفاتيح كما تُعيدها `PublicKey::to_bytes` بالضبط؛ ويقوم
الـ decoder بإعادة بناء `PublicKey` ويرفع
`AccountAddressError::InvalidPublicKey` إذا لم تطابق البايتات المنحنى
المعلن.

> **تطبيق صارم لـ Ed25519 (ADDR‑3a):** يجب أن تفك مفاتيح المنحنى
> `0x01` إلى نفس سلسلة البايتات التي يصدرها الموقّع، وألّا تقع في subgroup
> صغيرة الترتيب. ترفض العقد codings غير كاونونية (مثل القيم المختزلة
> modulo `2^255‑19`) والنقاط الضعيفة مثل عنصر الهوية، لذا يجب أن تعكس
> الـ SDKs نفس أخطاء التحقّق قبل إرسال العناوين.

#### 2.3.1 سجل معرّفات المنحنيات (Curve Identifier Registry) (ADDR‑1d)

| ID (`curve_id`) | الخوارزمية | Feature gate | ملاحظات |
|-----------------|------------|--------------|---------|
| `0x00` | محجوز         | — | يجب ألّا يُستخدم؛ الـ decoders يعيدون `ERR_UNKNOWN_CURVE`. |
| `0x01` | Ed25519       | مفعّل دائمًا | الخوارزمية الكانونية v1 (`Algorithm::Ed25519`); المعرّف الوحيد المفعّل في الـ production حاليًا. |
| `0x02` | ML‑DSA (معاينة) | `ml-dsa` | محجوز لـ ADDR‑3؛ غير مفعّل افتراضيًا حتى تتوفر مسارات توقيع ML‑DSA. |
| `0x03` | موضع GOST placeholder | `gost` | محجوز لسيناريوهات الإمداد/التفاوض؛ يجب رفض أي payload يحمل هذا المعرّف حتى توافق الحوكمة على ملف TC26 محدد. |
| `0x0A` | GOST R 34.10‑2012 (256، المجموعة A) | `gost` | محجوز؛ سيُفعّل مع feature التشفير `gost`. |
| `0x0B` | GOST R 34.10‑2012 (256، المجموعة B) | `gost` | محجوز لموافقة حوكمة مستقبلية. |
| `0x0C` | GOST R 34.10‑2012 (256، المجموعة C) | `gost` | محجوز لموافقة حوكمة مستقبلية. |
| `0x0D` | GOST R 34.10‑2012 (512، المجموعة A) | `gost` | محجوز لموافقة حوكمة مستقبلية. |
| `0x0E` | GOST R 34.10‑2012 (512، المجموعة B) | `gost` | محجوز لموافقة حوكمة مستقبلية. |
| `0x0F` | SM2           | `sm`   | محجوز؛ سيتم تفعيله عند استقرار مسار تشفير SM. |

المواضع `0x04–0x09` تبقى غير مستخدمة لمنحنيات إضافية؛ إضافة خوارزمية
جديدة تتطلّب تحديث الـ roadmap وتوفّر دعم مقابِل في الـ SDKs والـ host.
يجب على الـ encoders رفض أي خوارزمية غير مدعومة عبر
`ERR_UNSUPPORTED_ALGORITHM`، ويجب على الـ decoders أن تفشل بسرعة مع
`ERR_UNKNOWN_CURVE` عند مقابلة معرفات غير معروفة (fail‑closed).

يُخزَّن السجل الكانوني (مع تصدير JSON قابل للاستهلاك آليًا) في
`docs/source/references/address_curve_registry.md`. يُستحسن أن تستهلك
الأدوات هذا السجل مباشرة لضمان بقاء معرفات المنحنيات متسقة عبر
الـ SDKs وتدفّقات العمل التشغيلية.

- **تقييد على مستوى الـ SDK:** تأتي الـ SDKs افتراضيًا بدعم Ed25519 فقط.
  يوفر Swift flags ترجمة (`IROHASWIFT_ENABLE_MLDSA`,
  `IROHASWIFT_ENABLE_GOST`, `IROHASWIFT_ENABLE_SM`)؛ يتطلب Java‑SDK استدعاء
  `AccountAddress.configureCurveSupport(...)`، ويستخدم JavaScript‑SDK
  `configureCurveSupport({ allowMlDsa: true, allowGost: true, allowSm2: true })`
  بحيث يجب على المدمجين تفعيل الدعم صراحة قبل إنتاج عناوين بمنحنيات
  إضافية.
- **تقييد على مستوى الـ host:** ترفض `Register<Account>` أي controller
  يستخدم خوارزميات توقيع غائبة عن `crypto.allowed_signing` **أو**
  معرفات منحنيات غير موجودة في `crypto.curves.allowed_curve_ids`، لذا يجب
  على الـ clusters إعلان الدعم (إعدادات + genesis) قبل إتاحة controllers
  مبنية على ML‑DSA/GOST/SM.

### 2.4 قواعد الفشل (ADDR‑1a)

- أي payload أقصر من الترويسة + محدِّد النطاق المطلوبة، أو تحتوي على
  بايتات زائدة، تؤدي إلى `AccountAddressError::InvalidLength` أو
  `AccountAddressError::UnexpectedTrailingBytes`.
- الترويسات التي تضبط `ext_flag` المحجوز أو تعلن عن إصدارات/فئات غير
  مدعومة يجب أن تُرفض باستخدام
  `UnexpectedExtensionFlag` أو `InvalidHeaderVersion` أو
  `UnknownAddressClass`.
- tags غير معروفة للـ selector/controller تثير
  `UnknownDomainTag` أو `UnknownControllerTag`.
- مواد مفاتيح كبيرة جدًا أو معطوبة تثير
  `KeyPayloadTooLong` أو `InvalidPublicKey`.
- controllers من نوع multisig مع أكثر من 255 عضوًا ينتجون
  `MultisigMemberOverflow`.
- تحويلات IME/NFKC: يمكن تطبيع kana Sora ذات half‑width إلى full‑width
  دون كسر عملية الـ decode، لكن sentinel `snx1` وأرقام/أحرف IH58
  ASCII يجب أن تبقى ASCII. sentinels بصيغة full‑width أو بحروف
  case‑folded يولّدون `ERR_MISSING_COMPRESSED_SENTINEL`، وpayloads ASCII
  في full‑width تولّد `ERR_INVALID_COMPRESSED_CHAR`, وأي mismatch في
  checksum ينتشر كـ `ERR_CHECKSUM_MISMATCH`. تغطي اختبارات الخصائص
  (property tests) في
  `crates/iroha_data_model/src/account/address.rs` هذه المسارات لضمان
  فشل حتمي يمكن للـ SDKs والمحافظ الاعتماد عليه.
- Parsing aliasات `address@domain` في Torii والـ SDKs الآن يُصدر نفس
  رموز الأخطاء `ERR_*` متى ما فشلت مدخلات IH58 أو المضغوطة قبل fallback
  إلى alias نصي، بحيث يمكن للعملاء تمرير أسباب منظمة دون الاعتماد على
  تحليل نصي حر.

#### 2.5 المتجهات الثنائية المعيارية

- **النطاق الافتراضي الضمني (`default`، بايت seed يساوي `0x00`)**  
  الـ hex الكانوني:  
  `0x0200000120641297079357229f295938a4b5a333de35069bf47b9d0704e45805713d13c201`.  
  التفكيك: ترويسة `0x02`، محدِّد نطاق `0x00` (افتراضي ضمني)، tag
  controller يساوي `0x00`، معرّف منحنى `0x01` (Ed25519)، طول مفتاح
  `0x20`، ثم حمولة مفتاح بطول 32 بايت.
- **Digest نطاق محلي (`treasury`، بايت seed يساوي `0x01`)**  
  الـ hex الكانوني:  
  `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a20fd68c8329fe3bbfbecd26a2d72878cd827f8`.  
  التفكيك: ترويسة `0x02`، tag محدِّد `0x01` متبوعًا بالـ digest
  `b1 8f e9 c1 ab ba c4 5b 3e 38 fc 5d`، ثم حمولة مفتاح مفرد
  (tag يساوي `0x00`، معرّف منحنى `0x01`، طول `0x20`، و32 بايت لمفتاح
  Ed25519).

تؤكّد اختبارات الوحدة
(`account::address::tests::parse_any_accepts_all_formats`) المتجهات V1
أدناه عبر `AccountAddress::parse_any`، ما يضمن أن الأدوات يمكنها الاعتماد
على نفس الحمولة الكانونية عبر hex وIH58 والصيغة المضغوطة. يمكن إعادة
توليد مجموعة الـ fixtures الموسّعة عبر:

```bash
cargo run -p iroha_data_model --example address_vectors
```

| Domain      | Seed byte | Canonical hex                                                                 | Compressed |
|-------------|-----------|-------------------------------------------------------------------------------|------------|
| default     | `0x00`    | `0x0200000120641297079357229f295938a4b5a333de35069bf47b9d0704e45805713d13c201` | `snx12QGﾈkﾀｱﾚiﾉﾘuﾛWRヱﾏxﾁSuﾁepnhｽvｶrﾓｶ9Tｹｿp3ﾇVWｳｲｾU4N5E5` |
| treasury    | `0x01`    | `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a20fd68c8329fe3bbfbecd26a2d72878cd827f8` | `snx15ｻu6rﾀCヰTGwﾏ1ﾅヱﾌQｲﾖﾘｻYﾃhﾓMQ9CBEﾅﾊﾈｷﾉVRｺnKRwTﾋｼqﾅWrﾎU7ｼiﾍQt1TPGNJ` |
| wonderland  | `0x02`    | `0x0201b8ae571b79c5a80f5834da2b000120ad29ac2c12d4daaa4a2415235f2b01730bff1193dd4a6eaee29e945b01a4a212` | `snx15ｻwﾓyRｿqﾏnMﾀﾙヰKoﾒﾇﾓRKSｷﾗﾕneﾀM3Rabvﾂ1JﾚﾉｺｲﾕｹｺﾀFﾔﾇﾖFSXsﾜCHmB59S5KS` |
| iroha       | `0x03`    | `0x0201de8b36819700c807083608e2000120ce6d4f240893505e112cdc1b83585d8efc271ea6f934c5f6a49217e27e61b9e7` | `snx15ｻﾜxﾀ7Vｱ7QFeｷMﾂLﾉﾃﾏﾓsYヰxﾎﾍﾚﾇｺﾊehjyzXGヰaｿSﾔ1kWｺｾJeﾒAWkwﾋﾐRRQQKXYE` |
| alpha       | `0x04`    | `0x020146be2154ae86826a3fef0ec000012077143459c5b54808313cd57ded18322fc02c4616de930e0e3af578bb509bb5dc` | `snx15ｻ9JヱﾈｿuwU6ｴpﾔﾂﾈRqRSﾗgPﾏrHﾔGﾀqﾂｹfoﾂHwﾉoﾊ4ﾎﾇ74ｼﾕﾎUw8JaU3ﾙJFYHVLUS` |
| omega       | `0x05`    | `0x0201390d946885bc8416b3d30c9d000120e18cbb31e5249ff9205b72fe50e50dcc78fb80e28028bdc4c47bcf63ee61c6b8` | `snx15ｻ3zrﾌuﾚﾄJﾑXQhｸTyN8qfBﾌﾒaTjQpTxPﾊｦNﾚnヱvorHｷﾎkﾈEヱFﾎｻTUﾗhiVqURKRVM` |
| governance  | `0x06`    | `0x0201989eb45a80940d187e2c908f0001208a5bd65d39ba61bde2a87ee10d242bd5575cd02bf687c4b5960d4141698dd76a` | `snx15ｻiｵﾁyVﾕｽbFpDHHuﾇﾉdﾗｲﾌbﾜｸeGｵzﾙzｽﾐﾌﾉQﾃw2ﾕLDﾔｽﾙFﾙﾇﾋBTdUXﾎﾙsｽRDJCHS` |
| validators  | `0x07`    | `0x0201e4ffa58704c69afaeb7cc2d7000120f0f80d8a09aa1276d2e605bc904137f7a52b9c4847b9b5366d4002ca4049daeb` | `snx15ｻﾀLDH6VYﾑNAｾgﾉVﾜtxﾊWTﾂfKｳmU7fWﾍXｱ2JnyﾋE4ﾕghZｱVｶｦﾇaFﾕbr8qﾑR4VEFR` |
| explorer    | `0x08`    | `0x02013b35422c65c2a83c99c523ad00012033af4073c5815cbe5d0fec37cffe02e542302b60e24d8a7c0819f772ca6886f9` | `snx15ｻ4nmｻaﾚﾚPvNLgｿｱv6MHdPﾓWﾍヱpeﾕFﾕmFﾌﾀKhﾉWｴeﾋbｷXMﾎ2ﾃnQﾐﾗﾑﾎBBｻC8P548` |
| soranet     | `0x09`    | `0x0201047d9ea7f5d5dbec3f7bfc58000120cd3c119f6c81e28a2747c531f5cbe8dbc44ed8e16751bc4a467445b272db4097` | `snx15ｱｸヱVQﾂcﾁヱRﾓcApｲﾁﾅﾒvwS8JｳEnQaｿHTdﾒXZﾍvﾆazｿgﾔﾙhF9hcsﾘvNﾌヱJ9MGDNBW` |
| kitsune     | `0x0A`    | `0x0201e91933de397fd7723dc9a76c0001206e4c4188e1b8455ff3369dc163240a5d653f13a6f420fd0edbb23303bad239e7` | `snx15ｻﾚｺヱkfFJfSﾁｼJwﾉLvbpzﾘKmC6ｱSﾇhqｦJB1gﾙwCwﾁﾍeﾉﾔABﾆpqYﾍEﾌｼﾆﾃFNCAT97` |
| da          | `0x0B`    | `0x02016838cf5bb0ce0f3d4f380e1c00012056f02721c153689b09efafd07d8ef7bed2c4a581dd00faa118aed1d51f7a1ad6` | `snx15ｻNﾒ5SﾐRﾉﾐﾃ62ｿ1ｶｷWFKmgﾁﾃｻ3ｸGLpﾄﾆHnXGﾘﾑDﾃJ6ｸXﾐﾂﾊwhｶtｵｾｴﾃ9PﾖDFC3YQ` |

تمت مراجعة هذه المتجهات من قبل مجموعتي عمل نموذج البيانات والتشفير؛
ونطاقها معتمد لـ ADDR‑1a.

### 3. نطاقات عالمية فريدة والتطبيع

انظر أيضًا:
[`docs/source/references/address_norm_v1.md`](source/references/address_norm_v1.md)
للقناة (pipeline) الكانونية Norm v1 المستخدمة عبر Torii ونموذج البيانات
والـ SDKs.

نعيد تعريف `DomainId` كـ tuple معنونة (tagged) على النحو الآتي:

```text
DomainId {
    name: Name,
    authority: GlobalDomainAuthority, // تعداد جديد
}

enum GlobalDomainAuthority {
    LocalChain,                  // القيمة الافتراضية للسلسلة المحلية
    External { chain_discriminant: u16 },
}
```

يغلّف `LocalChain` القيمة الحالية لـ Name للنطاقات المدارة بواسطة السلسلة
الحالية. عند تسجيل نطاق عبر السجل العالمي، نخزّن مميّز السلسلة (chain
discriminant) المالك. يبقى أسلوب العرض/التحليل كما هو في الوقت الحالي،
لكن البنية الموسّعة تسمح بقرارات توجيه (routing) مبنية على السلطة
العالمية للنطاق.

#### 3.1 التطبيع والدفاع ضد الانتحال

تعرّف Norm v1 القناة الكانونية التي يجب على كل مكوّن استخدامها قبل أن
يتم تخزين اسم نطاق أو تضمينه في `AccountAddress`. توجد الجولة الكاملة في
[`docs/source/references/address_norm_v1.md`](source/references/address_norm_v1.md)،
ويعرض الملخّص أدناه الخطوات التي يجب أن تطبّقها المحافظ وTorii والـ SDKs
وأدوات الحوكمة:

1. **التحقق من المدخلات.** رفض السلاسل الفارغة، ومسافات البيض، ومحارف
   الفاصل المحجوزة `@` و`#` و`$`. يتطابق هذا مع الثوابت التي تفرضها
   `Name::validate_str`.
2. **تركيب Unicode NFC.** تطبيق تطبيع NFC (مدعوم من ICU) بحيث تنطبق
   التسلسلات المتكافئة قانونيًا بصورة حتمية
   (مثل `e\u{0301}` ← `é`).
3. **تطبيع UTS‑46.** تمرير خرج NFC عبر UTS‑46 مع
   `use_std3_ascii_rules = true` و`transitional_processing = false`
   مع فرض حدود DNS على الأطوال. النتيجة سلسلة A‑label بالحروف الصغيرة؛
   المدخلات التي تنتهك قواعد STD3 تفشل في هذه المرحلة.
4. **حدود الأطوال.** فرض قيود النمط DNS: يجب أن يكون طول كل label بين
   1 و63 بايت، وألّا يتجاوز طول النطاق الكامل 255 بايت بعد الخطوة 3.
5. **سياسة التشابه (confusable) الاختيارية.** يتم تتبّع فحوصات UTS‑39
   لنسخة Norm v2؛ يمكن للمشغّلين تفعيلها مبكرًا، لكن أي فشل في هذا
   الفحص يجب أن يوقف المعالجة بالكامل.

إذا نجحت جميع المراحل، يتم تخزين سلسلة A‑label بالحروف الصغيرة واستخدامها
للترميز، وإعدادات التكوين، والـ manifests، واستعلامات السجل. تستمد
محددات الـ Local‑digest قيمة الـ 12 بايت عبر
`blake2s_mac(key = "SORA-LOCAL-K:v1", canonical_label)[0..12]` مستخدمة خرج
الخطوة الثالثة. تُرفض كل المحاولات الأخرى (مثل case مختلط أو upper‑case
أو Unicode خام) مع توليد `ParseError` منظّم عند الحد الذي تم عنده إدخال
الاسم.

تُبيّن الـ fixtures الكانونية التي تطبّق هذه القواعد — بما في ذلك
round‑trips لـ punycode وتسلسلات STD3 غير الصحيحة — في
`docs/source/references/address_norm_v1.md`، ويتم عكسها في مجموعات متجهات
الاختبار في CI الخاصة بالـ SDKs ضمن ADDR‑2.

### 4. سجل نطاق Nexus والتوجيه

- **مخطّط السجل:** يحتفظ Nexus بخريطة موقّعة من الشكل
  `DomainName -> ChainRecord`، حيث يحتوي `ChainRecord` على مميّز السلسلة
  وبيانات وصفية اختيارية (مثل عناوين RPC) ودليل سلطة (مثل توقيع متعدد
  الأطراف للحكامة).
- **آلية المزامنة:**  
  - ترسل السلاسل مطالبات نطاق موقّعة إلى Nexus (إما في genesis أو عبر
    تعليمات حوكمة).  
  - ينشر Nexus manifests دورية (JSON موقّع مع جذر Merkle اختياري) عبر
    HTTPS وتخزين معنون بالمحتوى (مثل IPFS). تقوم الـ clients بتثبيت آخر
    manifest والتحقق من التوقيعات.
- **مسار الاستعلام:**  
  - يستقبل Torii معاملة تشير إلى `DomainId`.  
  - إذا كان النطاق غير معروف محليًا، يستعلم Torii manifest المخزَّن في
    الـ cache.  
  - إذا أشار manifest إلى سلسلة أجنبية، تُرفض المعاملة بخطأ حتمي
    `ForeignDomain` مع معلومات السلسلة البعيدة.  
  - إذا كان النطاق غير موجود في Nexus، يعيد Torii الخطأ `UnknownDomain`.
- **نقاط الثقة والتدوير:** توقّع مفاتيح الحوكمة الـ manifests؛ أي تدوير أو
  إبطال يتم نشره كبند manifest جديد. تفرض الـ clients حدود TTL للـ manifest
  (مثل 24 ساعة) وترفض استشارة بيانات قديمة تتجاوز تلك النافذة.
- **أنماط الفشل:** إذا فشل جلب manifest، يعتمد Torii على البيانات
  المخبّأة داخل نافذة الـ TTL؛ وبعد انتهائها يصدر `RegistryUnavailable`
  ويرفض التوجيه بين النطاقات لتجنّب حالة غير متسقة.

### 4.1 ثبات السجل، الأسماء المستعارة، و«شواهد الحذف» (ADDR‑7c)

ينشر Nexus **manifest ملحقًا فقط (append‑only)** بحيث يمكن تدقيق كل تعيين
نطاق أو alias وإعادة تشغيله. يجب على المشغّلين التعامل مع الحزمة الموضّحة
في
[دليل تشغيل manifest العناوين](source/runbooks/address_manifest_ops.md)
كمرجع وحيد للحقيقة؛ إذا كان manifest مفقودًا أو فشل في التحقق، يجب على
Torii رفض حلّ النطاقات المتأثرة.

**دعم الأتمتة:** يعيد الأمر  
`cargo xtask address-manifest verify --bundle <current_dir> --previous <previous_dir>`
تشغيل فحوصات checksum والمخطّط و`previous_digest` كما هي في دليل التشغيل.
ضمّن ناتج الأمر في تذاكر التغيير لإثبات أن رابط `sequence` و
`previous_digest` قد تم التحقق منه قبل نشر الحزمة.

#### ترويسة manifest وعقدة التوقيع

| الحقل | المتطلّب |
|-------|---------|
| `version` | القيمة الحالية `1`. لا يجوز زيادتها إلا مع تحديث مطابق للمواصفة. |
| `sequence` | تُزاد بمقدار **واحد تمامًا** في كل نشر. كاشات Torii ترفض الإصدارات ذات الفجوات أو التراجعات. |
| `generated_ms` + `ttl_hours` | تحدّد حداثة الـ cache (الافتراضي 24 ساعة). إذا انتهت مدة الـ TTL قبل النشر التالي، يحوّل Torii الحالة إلى `RegistryUnavailable`. |
| `previous_digest` | Digest من نوع BLAKE3 (hex) لهيكل manifest السابق. يعيد المتحقّقون حسابه عبر `b3sum` لإثبات الثبات. |
| `signatures` | تُوقَّع الـ manifests عبر Sigstore (`cosign sign-blob`). يجب على فرق التشغيل تشغيل `cosign verify-blob --bundle manifest.sigstore manifest.json` وفرض قيود هوية/جهة إصدار الحوكمة قبل طرحها. |

تنتج أتمتة النشر ملفات `manifest.sigstore` و`checksums.sha256` بجانب جسم
الـ JSON. احتفظ بهذه الملفات معًا عند نسخها إلى SoraFS أو نقاط HTTP حتى
يتمكن المدققون من إعادة خطوات التحقق حرفيًا.

#### أنواع السجلات (Entry types)

| النوع | الغرض | الحقول المطلوبة |
|------|-------|-----------------|
| `global_domain` | يعلن أن النطاق مسجَّل عالميًا ويجب أن يُربط بمميّز سلسلة وprefix لـ IH58. | `{ "domain": "<label>", "chain": "sora:nexus:global", "ih58_prefix": 753, "selector": "global" }` |
| `local_alias` | يتتبّع محدّدات `Local-12` القديمة التي لا تزال تُوجّه محليًا، مع إضافة digest من 12 بايت واختياريًا `alias_label`. | `{ "domain": "<label>", "selector": { "kind": "local", "digest_hex": "<12-byte-hex>" }, "alias_label": "<optional>" }` |
| `tombstone` | يقاعد alias/selector بشكل دائم. مطلوب عند مسح digests من نوع Local‑8 أو إزالة نطاق. | `{ "selector": {…}, "reason_code": "LOCAL8_RETIREMENT" \| …, "ticket": "<governance id>", "replaces_sequence": <number> }` |

يمكن أن تتضمّن سجلات `global_domain` اختياريًا `manifest_url` أو
`sorafs_cid` للإشارة إلى metadata موقّعة للسلسلة، لكن الـ tuple الكانوني
يبقى `{domain, chain, discriminant/ih58_prefix}`. يجب على سجلات
`tombstone` أن تذكر selector المراد تقاعده وتذكرة/وثيقة الحوكمة التي
سمحت بالتغيير بحيث يمكن إعادة بناء أثر التدقيق (audit trail) دون اتصال.

#### سير عمل الـ alias/‏tombstone والقياس (Telemetry)

1. **اكتشاف الانحراف.** استخدم
   `torii_address_local8_total{endpoint}` و
   `torii_address_invalid_total{endpoint,reason}` (المعروضة في
   `dashboards/grafana/address_ingest.json`) للتأكد من عدم قبول سلاسل
   Local‑8 في بيئات الإنتاج قبل اقتراح سجل tombstone.
2. **اشتقاق digests الكانونية.** شغّل  
   `iroha address convert <address-or-account_id> --format json --expect-prefix 753`
   (أو استهلك
   `fixtures/account/address_vectors.json` عبر
   `scripts/account_fixture_helper.py`) لالتقاط قيمة `digest_hex` بدقة.
   يقبل الـ CLI الآن مدخلات من شكل `snx1...@wonderland`؛ ويُظهر ملخّص JSON
   ذلك النطاق في الحقل `input_domain`، كما أن الخيار `--append-domain`
   يعيد تشغيل الترميز المحوَّل كـ `<ih58>@wonderland` لتحديث manifests.
   لعمليات التصدير الخطّية (سطر لكل عنوان) استخدم:
   لتحويل محددات Local جماعيًا إلى صيغ IH58 (أو صيغ مضغوطة/hex/JSON)
   مع تخطّي الصفوف غير المحلية. ولتوفير دليل قابل للاستهلاك في جداول
   البيانات، استخدم:
   لإنتاج ملف CSV (`input,status,format,domain_kind,…`) يبرز محددات Local،
   والترميزات الكانونية، وإخفاقات التحليل في ملف واحد.
3. **إضافة سجلات إلى manifest.** صِغ سجل `tombstone` (وسجل
   `global_domain` اللاحق عند الانتقال إلى السجل العالمي) وتحقّق من صحة
   manifest عبر `cargo xtask address-vectors` قبل طلب التوقيعات.
4. **التحقق والنشر.** اتبع قائمة فحص دليل التشغيل (الـ hashes وSigstore
   وmonotonicity الخاصة بـ `sequence`) قبل تكرار الحزمة إلى SoraFS. يعتمد
   Torii الآن القيمة الافتراضية `torii.strict_addresses = true`، لذا
   تفرض عناقيد الإنتاج عناوين IH58/الصيغ المضغوطة الكانونية فور هبوط
   الحزمة.
5. **المراقبة والتراجع عند الحاجة.** حافظ على لوحات Local‑8 عند الصفر
   لمدة 30 يومًا؛ إذا ظهرت مشكلات، أعد نشر حزمة manifest السابقة وغيّر
   العلم مؤقتًا إلى `torii.strict_addresses=false` فقط في البيئة غير
   الإنتاجية المتأثرة حتى تستقر القياسات.

كل الخطوات أعلاه تشكّل أدلة إلزامية لـ ADDR‑7c: أي manifests لا تحتوي على
حزمة توقيع `cosign` أو قيم `previous_digest` غير المطابقة يجب رفضها
تلقائيًا، ويجب على المشغّلين إرفاق سجلات التحقق بتذاكر التغيير.

</div>

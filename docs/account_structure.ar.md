# هيكل الحساب RFC

**الحالة:** مقبول (ADDR-1)  
**الجمهور:** نموذج البيانات، وTorii، وNexus، وWallet، وفرق الحوكمة  
**القضايا ذات الصلة:** سيتم تحديدها لاحقًا

## ملخص

يصف هذا المستند حزمة معالجة حساب الشحن التي تم تنفيذها
`AccountAddress` (`crates/iroha_data_model/src/account/address.rs`) و
الأدوات المصاحبة. وهو يوفر:

- مجموع اختباري، موجه للإنسان ** عنوان Iroha Base58 (IH58) ** تم إنتاجه بواسطة
  `AccountAddress::to_ih58` الذي يربط سلسلة مميزة بالحساب
  وحدة تحكم وتقدم نماذج نصية حتمية صديقة للتشغيل البيني.
- محددات المجال للمجالات الافتراضية الضمنية والملخصات المحلية، مع
  علامة محدد السجل العالمي المحجوزة للتوجيه المستقبلي المدعوم من Nexus (ملف
  البحث عن التسجيل ** لم يتم شحنه بعد **).

## الدافع

تعتمد المحافظ والأدوات خارج السلسلة على الأسماء المستعارة للتوجيه الخام `alias@domain` (rejected legacy form) اليوم. هذا
له عيبان رئيسيان:

1. **لا يوجد ربط بالشبكة.** لا تحتوي السلسلة على مجموع اختباري أو بادئة سلسلة، لذا فإن المستخدمين
   يمكن لصق عنوان من الشبكة الخاطئة دون الحصول على تعليقات فورية. ال
   سيتم رفض المعاملة في النهاية (عدم تطابق السلسلة)، أو ما هو أسوأ من ذلك، ستنجح
   مقابل حساب غير مقصود إذا كانت الوجهة موجودة محليًا.
2. **تضارب المجال.** النطاقات مخصصة لمساحة الاسم فقط ويمكن إعادة استخدامها في كل منها
   سلسلة. اتحاد الخدمات (الأوصياء والجسور وسير العمل عبر السلسلة)
   يصبح هشًا لأن `finance` في السلسلة A غير مرتبط بـ `finance` في
   سلسلة ب.

نحن بحاجة إلى تنسيق عنوان صديق للإنسان يحمي من أخطاء النسخ/اللصق
وتعيين حتمي من اسم المجال إلى السلسلة الموثوقة.

## الأهداف

- وصف مغلف IH58 Base58 المطبق في نموذج البيانات و
  قواعد التحليل/الاسم المستعار الأساسية التي يتبعها `AccountId` و`AccountAddress`.
- قم بتشفير تمييز السلسلة المكوّنة مباشرةً في كل عنوان و
  تحديد عملية الحوكمة/التسجيل الخاصة بها.
- وصف كيفية تقديم سجل نطاق عالمي دون انقطاع التيار
  عمليات النشر وتحديد قواعد التطبيع/مكافحة الانتحال.

## غير الأهداف

- تنفيذ عمليات نقل الأصول عبر السلسلة. تقوم طبقة التوجيه بإرجاع فقط
  سلسلة الهدف.
- الانتهاء من حوكمة إصدار النطاق العالمي. يركز RFC هذا على البيانات
  النماذج الأولية والنقل.

## الخلفية

### الاسم المستعار للتوجيه الحالي

```
AccountId {
    domain: DomainId,   // wrapper over Name (ASCII-ish string)
    controller: AccountController // single PublicKey or multisig policy
}

Display: canonical IH58 literal (no `@domain` suffix)
Parse accepts:
- Encoded account identifiers only: IH58 (preferred) and `sora` compressed.
- Runtime parsers reject canonical hex (`0x...`), any `@<domain>` suffix, and alias literals such as `label@domain`.

Multihash hex is canonical: varint bytes are lowercase hex, payload bytes are uppercase hex,
and `0x` prefixes are not accepted.

This text form is now treated as an **account alias**: a routing convenience
that points to the canonical [`AccountAddress`](#2-canonical-address-codecs).
It remains useful for human readability and domain-scoped governance, but it is
no longer considered the authoritative account identifier on-chain.
```

`ChainId` يعيش خارج `AccountId`. تتحقق العقد من `ChainId` الخاص بالمعاملة
ضد التكوين أثناء القبول (`AcceptTransactionFail::ChainIdMismatch`)
ورفض المعاملات الأجنبية، ولكن سلسلة الحساب نفسها لا تحمل أي
تلميح الشبكة.

### معرفات المجال

`DomainId` يلتف `Name` (سلسلة تمت تسويتها) ويتم تحديد نطاقه على السلسلة المحلية.
يمكن لكل سلسلة التسجيل `wonderland`، `finance`، وما إلى ذلك بشكل مستقل.

### سياق العلاقة

Nexus مسؤول عن التنسيق بين المكونات (الممرات/مساحات البيانات). ذلك
ليس لديه حاليًا مفهوم لتوجيه المجال عبر السلسلة.

## التصميم المقترح

### 1. تمييز السلسلة الحتمية

`iroha_config::parameters::actual::Common` يكشف الآن:

```rust
pub struct Common {
    pub chain: ChainId,
    pub chain_discriminant: u16, // globally coordinated
    // ... existing fields
}
```

- **القيود:**
  - فريد لكل شبكة نشطة؛ تدار من خلال التسجيل العام الموقع مع
    النطاقات المحجوزة الصريحة (على سبيل المثال، `0x0000–0x0FFF` test/dev، `0x1000–0x7FFF`
    تخصيصات المجتمع، `0x8000–0xFFEF` معتمدة من قبل الإدارة، `0xFFF0–0xFFFF`
    محفوظة).
  - غير قابل للتغيير لسلسلة التشغيل. يتطلب تغييره شوكة صلبة و
    تحديث التسجيل.
- **الحوكمة والتسجيل (مخطط):** سيتم إنشاء مجموعة حوكمة متعددة التوقيعات
  الحفاظ على سجل JSON موقع لتعيين التمييزات للأسماء المستعارة البشرية و
  معرفات CAIP-2. هذا التسجيل ليس جزءًا من وقت التشغيل الذي تم شحنه.
- **الاستخدام:** مترابطة من خلال قبول الدولة، وTorii، وSDKs، وواجهات برمجة تطبيقات المحفظة
  يمكن لكل مكون تضمينه أو التحقق من صحته. يظل التعرض لـ CAIP-2 مستقبلاً
  مهمة التشغيل المتداخل.

### 2. برامج ترميز العناوين الأساسية

يعرض نموذج بيانات Rust تمثيلاً واحدًا للحمولة النافعة
(`AccountAddress`) التي يمكن إصدارها بعدة تنسيقات ذات وجه بشري. IH58 هو
تنسيق الحساب المفضل للمشاركة والمخرجات الأساسية؛ المضغوط
يعد نموذج `sora` ثاني أفضل خيار، وهو خيار Sora فقط لتجربة المستخدم حيث توجد أبجدية كانا
يضيف قيمة. يبقى السداسي عشري الكنسي أداة مساعدة لتصحيح الأخطاء.

- **IH58 (Iroha Base58)** – مظروف Base58 يتضمن السلسلة
  تمييزي. تتحقق أجهزة فك التشفير من صحة البادئة قبل ترقية الحمولة إلى
  الشكل الكنسي.
- **عرض Sora المضغوط** – أبجدية Sora فقط مكونة من **105 رموز** تم إنشاؤها بواسطة
  إلحاق القصيدة نصف العرض イロハ (بما في ذلك ヰ وヱ) بالحرف المكون من 58 حرفًا
  مجموعة IH58. تبدأ السلاسل بالحارس `sora`، وقم بتضمين مشتق من Bech32m
  المجموع الاختباري، واحذف بادئة الشبكة (يُشير الحارس إلى Sora Nexus ضمنيًا).

```
  IH58  : 123456789ABCDEFGHJKMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz
  Iroha : ｲﾛﾊﾆﾎﾍﾄﾁﾘﾇﾙｦﾜｶﾖﾀﾚｿﾂﾈﾅﾗﾑｳヰﾉｵｸﾔﾏｹﾌｺｴﾃｱｻｷﾕﾒﾐｼヱﾋﾓｾｽ
  ```
- **العرافة الأساسية** - ترميز `0x…` للبايت الأساسي سهل التصحيح
  المغلف.

`AccountAddress::parse_encoded` يكتشف تلقائيًا IH58 (المفضل)، أو المضغوط (`sora`، ثاني أفضل)، أو سداسي عشري أساسي
(`0x...` فقط؛ تم رفض السداسية العارية) المدخلات وإرجاع كل من الحمولة النافعة التي تم فك تشفيرها والحمولة المكتشفة
`AccountAddressFormat`. يقوم Torii الآن باستدعاء `parse_encoded` للحصول على ISO 20022 التكميلي
يعالج ويخزن النموذج السداسي الأساسي بحيث تظل البيانات التعريفية حتمية
بغض النظر عن التمثيل الأصلي.

#### 2.1 تخطيط بايت الرأس (ADDR-1a)

يتم وضع كل حمولة أساسية على أنها `header · controller`. ال
`header` عبارة عن بايت واحد ينقل قواعد المحلل اللغوي التي تنطبق على البايتات التي
اتبع:

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

وبالتالي فإن البايت الأول يحزم بيانات تعريف المخطط لأجهزة فك التشفير النهائية:

| بت | المجال | القيم المسموح بها | خطأ في المخالفة |
|------|-------|----------------|----|
| 7-5 | `addr_version` | `0` (الإصدار الأول). القيم `1-7` محجوزة للمراجعات المستقبلية. | القيم الموجودة خارج `0-7` تؤدي إلى تشغيل `AccountAddressError::InvalidHeaderVersion`؛ يجب أن تتعامل التطبيقات مع الإصدارات غير الصفرية على أنها غير مدعومة اليوم. |
| 4-3 | `addr_class` | `0` = مفتاح واحد، `1` = multisig. | ترفع القيم الأخرى `AccountAddressError::UnknownAddressClass`. |
| ٢-١ | `norm_version` | `1` (القاعدة الإصدار 1). القيم `0`، `2`، `3` محجوزة. | القيم الموجودة خارج `0-3` ترفع `AccountAddressError::InvalidNormVersion`. |
| 0 | `ext_flag` | يجب أن يكون `0`. | تعيين رفع البت `AccountAddressError::UnexpectedExtensionFlag`. |

يكتب برنامج تشفير Rust `0x02` لوحدات التحكم ذات المفتاح الواحد (الإصدار 0، الفئة 0،
المعيار v1، تم مسح علامة الامتداد) و`0x0A` لوحدات التحكم المتعددة (الإصدار 0،
الفئة 1، القاعدة v1، تم مسح علامة الامتداد).

#### 2.2 Domainless payload semantics

Canonical payload bytes are domainless: the wire layout is `header · controller`
with no selector segment, no implicit default-domain reconstruction, and no
public decode fallback for legacy scoped-account literals.

Explicit domain context is modeled separately as `ScopedAccountId { account,
domain }` or separate API fields; it is not encoded into `AccountId` payload
bytes.

| Tag | Meaning | Payload | Notes |
|-----|---------|---------|-------|
| `0x00` | Domainless canonical scope | none | Canonical account payloads are domainless; explicit domain context lives outside the address payload. |
| `0x01` | Local domain digest | 12 bytes | Digest = `blake2s_mac(key = "SORA-LOCAL-K:v1", canonical_label)[0..12]`. |
| `0x02` | Global registry entry | 4 bytes | Big-endian `registry_id`; reserved until the global registry ships. |

Domain labels are canonicalised (UTS-46 + STD3 + NFC) before hashing. Unknown tags raise `AccountAddressError::UnknownDomainTag`. When validating an address against a domain, mismatched selectors raise `AccountAddressError::DomainMismatch`.

```
legacy selector segment
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind, see table)│
└──────────┴──────────────────────────────────────────────┘
```

When present, the selector is immediately adjacent to the controller payload, so
a decoder can walk the wire format in order: read the tag byte, read the
tag-specific payload, then move on to the controller bytes.

**Legacy selector examples**

- *Implicit default* (`tag = 0x00`). No payload. Example canonical hex for the default
  domain using the deterministic test key:
  `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29`.
- *Local digest* (`tag = 0x01`). Payload is the 12-byte digest. Example (`treasury` seed
  `0x01`): `0x0201b18fe9c1abbac45b3e38fc5d0001208a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c`.
- *Global registry* (`tag = 0x02`). Payload is a big-endian `registry_id:u32`. The bytes
  that follow the payload are identical to the implicit-default case; the selector simply
  replaces the normalised domain string with a registry pointer. Example using
  `registry_id = 0x0000_002A` (decimal 42) and the deterministic default controller:
  `0x02020000002a000120641297079357229f295938a4b5a333de35069bf47b9d0704e45805713d13c201`.

#### 2.3 ترميزات حمولة وحدة التحكم (ADDR-1a)

حمولة وحدة التحكم هي اتحاد آخر ذو علامة ملحقة بعد محدد المجال:

| العلامة | المراقب المالي | التخطيط | ملاحظات |
|-----|------------|--------|-------|
| `0x00` | مفتاح واحد | `curve_id:u8` · `key_len:u8` · `key_bytes` | `curve_id=0x01` يعين Ed25519 اليوم. `key_len` يحده `u8`؛ ترفع القيم الأكبر `AccountAddressError::KeyPayloadTooLong` (لذلك لا يمكن تشفير المفاتيح العامة ML‑DSA ذات المفتاح الواحد، والتي يزيد حجمها عن 255 بايت، ويجب أن تستخدم multisig). |
| `0x01` | متعدد التوقيع | `version:u8` · `threshold:u16` · `member_count:u8` · (`curve_id:u8` · `weight:u16` · `key_len:u16` · `key_bytes`)\* | يدعم ما يصل إلى 255 عضوًا (`CONTROLLER_MULTISIG_MEMBER_MAX`). منحنيات غير معروفة ترفع `AccountAddressError::UnknownCurve`؛ تظهر السياسات المشوهة كـ `AccountAddressError::InvalidMultisigPolicy`. |

تعرض سياسات Multisig أيضًا خريطة CBOR بنمط CTAP2 والملخص الأساسي لذلك
يمكن للمضيفين ومجموعات SDK التحقق من وحدة التحكم بشكل حتمي. انظر
`docs/source/references/multisig_policy_schema.md` (ADDR-1c) للمخطط،
قواعد التحقق من الصحة وإجراءات التجزئة والتركيبات الذهبية.

يتم تشفير كافة بايتات المفاتيح تمامًا كما تم إرجاعها بواسطة `PublicKey::to_bytes`؛ تقوم وحدات فك التشفير بإعادة إنشاء مثيلات `PublicKey` ورفع `AccountAddressError::InvalidPublicKey` إذا كانت البايتات لا تتطابق مع المنحنى المعلن.

> **Ed25519 تطبيق قانوني (ADDR-3a):** منحنى `0x01` يجب أن يتم فك تشفير المفاتيح إلى سلسلة البايت الدقيقة المنبعثة من المُوقع ويجب ألا تقع في المجموعة الفرعية ذات الترتيب الصغير. ترفض العقد الآن الترميزات غير الأساسية (على سبيل المثال، القيم المخفضة للوحدات `2^255-19`) ونقاط الضعف مثل عنصر الهوية، لذلك يجب أن تظهر حزم SDK أخطاء التحقق من صحة المطابقة قبل إرسال العناوين.

##### 2.3.1 سجل معرف المنحنى (ADDR-1d)

| المعرف (`curve_id`) | الخوارزمية | بوابة الميزة | ملاحظات |
|-----------------|-----------|--------------|-------|
| `0x00` | محفوظة | — | يجب ألا ينبعث؛ سطح أجهزة فك التشفير `ERR_UNKNOWN_CURVE`. |
| `0x01` | إد25519 | — | خوارزمية v1 الأساسية (`Algorithm::Ed25519`); تمكين في التكوين الافتراضي. |
| `0x02` | ML-DSA (ديليثيوم3) | — | يستخدم بايتات المفتاح العام Dilithium3 (1952 بايت). لا يمكن للعناوين أحادية المفتاح تشفير ML‑DSA لأن `key_len` هو `u8`؛ يستخدم multisig `u16` الأطوال. |
| `0x03` | BLS12‑381 (عادي) | `bls` | المفاتيح العامة في G1 (48 بايت)، والتوقيعات في G2 (96 بايت). |
| `0x04` | secp256k1 | — | ECDSA الحتمية على SHA-256؛ تستخدم المفاتيح العامة النموذج المضغوط SEC1 بحجم 33 بايت، وتستخدم التوقيعات التصميم الأساسي `r∥s` الذي يبلغ حجمه 64 بايت. |
| `0x05` | BLS12‑381 (صغير) | `bls` | المفاتيح العامة في G2 (96 بايت)، والتوقيعات في G1 (48 بايت). |
| `0x0A` | GOST R 34.10‑2012 (256، المجموعة أ) | `gost` | متاح فقط عند تمكين ميزة `gost`. |
| `0x0B` | GOST R 34.10‑2012 (256، المجموعة ب) | `gost` | متاح فقط عند تمكين ميزة `gost`. |
| `0x0C` | GOST R 34.10‑2012 (256، المجموعة ج) | `gost` | متاح فقط عند تمكين ميزة `gost`. |
| `0x0D` | GOST R 34.10‑2012 (512، المجموعة أ) | `gost` | متاح فقط عند تمكين ميزة `gost`. |
| `0x0E` | GOST R 34.10‑2012 (512، المجموعة ب) | `gost` | متاح فقط عند تمكين ميزة `gost`. |
| `0x0F` | اس ام 2 | `sm` | طول DistID (u16 BE) + بايتات DistID + مفتاح SM2 غير مضغوط SEC1 سعة 65 بايت؛ متاح فقط عند تمكين `sm`. |

تظل الفتحات `0x06–0x09` غير مخصصة للمنحنيات الإضافية؛ إدخال جديد
تتطلب الخوارزمية تحديث خريطة الطريق ومطابقة تغطية SDK/المضيف. التشفير
يجب رفض أي خوارزمية غير مدعومة باستخدام `ERR_UNSUPPORTED_ALGORITHM`، و
يجب أن تفشل أجهزة فك التشفير بسرعة في المعرفات غير المعروفة باستخدام `ERR_UNKNOWN_CURVE` للمحافظة عليها
السلوك المغلق بالفشل.

السجل الأساسي (بما في ذلك تصدير JSON القابل للقراءة آليًا) موجود حاليًا
[`docs/source/references/address_curve_registry.md`](المصدر/المراجع/address_curve_registry.md).
يجب أن تستهلك الأدوات مجموعة البيانات هذه مباشرةً حتى تظل معرفات المنحنى
متسقة عبر حزم SDK وسير عمل المشغلين.

- **بوابة SDK:** تكون مجموعات SDK الافتراضية هي التحقق/التشفير Ed25519 فقط. يفضح سويفت
  إشارات وقت الترجمة (`IROHASWIFT_ENABLE_MLDSA`، `IROHASWIFT_ENABLE_GOST`،
  `IROHASWIFT_ENABLE_SM`); يتطلب Java/Android SDK
  `AccountAddress.configureCurveSupport(...)`; يستخدم JavaScript SDK
  `configureCurveSupport({ allowMlDsa: true, allowGost: true, allowSm2: true })`.
  يتوفر دعم secp256k1 ولكن لا يتم تمكينه افتراضيًا في JS/Android
  أدوات تطوير البرمجيات؛ يجب على المتصلين الاشتراك بشكل صريح عند إرسال وحدات تحكم غير Ed25519.
- **بوابة المضيف:** `Register<Account>` يرفض وحدات التحكم التي يستخدم الموقعون عليها الخوارزميات
  مفقود من قائمة `crypto.allowed_signing` ** أو ** معرفات المنحنى غائبة عن
  `crypto.curves.allowed_curve_ids`، لذلك يجب أن تعلن المجموعات عن الدعم (التكوين +
  Genesis) قبل إمكانية تسجيل وحدات التحكم ML‑DSA/GOST/SM. وحدة تحكم BLS
  يُسمح دائمًا بالخوارزميات عند تجميعها (تعتمد عليها مفاتيح الإجماع)،
  والتكوين الافتراضي يمكّن Ed25519 + secp256k1.Qrates/iroha_core/src/smartcontracts/isi/domain.rs:32

##### 2.3.2 إرشادات وحدة التحكم Multisig

`AccountController::Multisig` يقوم بتسلسل السياسات عبر
`crates/iroha_data_model/src/account/controller.rs` ويفرض المخطط
موثق في [`docs/source/references/multisig_policy_schema.md`](المصدر/المراجع/multisig_policy_schema.md).
تفاصيل التنفيذ الرئيسية:

- تمت تسوية السياسات والتحقق من صحتها بواسطة `MultisigPolicy::validate()` من قبل
  يجري تضمينها. يجب أن تكون العتبات ≥1 ووزن ≥Σ؛ الأعضاء المكررة هي
  تمت إزالته بشكل حتمي بعد الفرز بواسطة `(algorithm || 0x00 || key_bytes)`.
- يتم تشفير حمولة وحدة التحكم الثنائية (`ControllerPayload::Multisig`).
  `version:u8`، `threshold:u16`، `member_count:u8`، ثم كل عضو
  `(curve_id, weight:u16, key_len:u16, key_bytes)`. هذا هو بالضبط ما
  `AccountAddress::canonical_bytes()` يكتب إلى حمولات IH58 (المفضل)/sora (ثاني أفضل).
- التجزئة (`MultisigPolicy::digest_blake2b256()`) تستخدم Blake2b-256 مع
  `iroha-ms-policy` سلسلة التخصيص بحيث يمكن ربط بيانات الإدارة بـ
  معرف السياسة الحتمية الذي يطابق وحدات بايت وحدة التحكم المضمنة في IH58.
- تغطية المباراة موجودة في `fixtures/account/address_vectors.json` (الحالات
  `addr-multisig-*`). يجب أن تؤكد المحافظ ومجموعات تطوير البرامج (SDK) على سلاسل IH58 الأساسية
  أدناه للتأكد من تطابق برامج التشفير الخاصة بها مع تطبيق Rust.

| معرف الحالة | العتبة / الأعضاء | IH58 حرفي (البادئة `0x02F1`) | سورا مضغوط (`sora`) حرفي | ملاحظات |
|---------|-------------------------------------|--------------------------------|---------|-------|
| `addr-multisig-council-threshold3` | `≥3` الوزن، الأعضاء `(2,1,1)` | `SRfSHsrH3tEmYaaAYyD248F3vfT1oQ3WEGS22MaD8W9bLefF7rsoKLYGcpbcM9EcSus5ZhCAZU7ztn2BCsyeCAdfRncAVmVsipd4ibk6CBLF3Nrzcw8P7VKJg6mtFgEhWVTjfDkUMoc63oeEmaWyV6cyiphwk8ZgKAJUe4TyVtmKm1WWcg7qZ6i` | `sora3vﾑ2zkaoUwﾋﾅGﾘﾚyﾂe3ﾖfﾙヰｶﾘﾉwｷnoWﾛYicaUr3ﾔｲﾖ2Ado3TﾘYQﾉJqﾜﾇｳﾑﾐd8dDjRGｦ3Vﾃ9HcﾀMヰR8ﾎﾖgEqGｵEｾDyc5ﾁ1ﾔﾉ31sUﾑﾀﾖaｸxﾘ3ｲｷMEuFｺｿﾉBQSVQnxﾈeJzrXLヰhｿｹ5SEEﾅPﾂﾗｸdヰﾋ1bUGHｲVXBWNNJ6K` | النصاب القانوني لإدارة مجال المجلس. |
| `addr-multisig-wonderland-threshold2` | `≥2`، الأعضاء `(1,2)` | `3xsmkps1KPBn9dtpE5qHRhHEZCpiAe8d9j6H9A42TV6kc1TpaqdwnSksKgQrsSEHznqvWKBMc1os69BELzkLjsR7EV2gjV14d9JMzo97KEmYoKtxCrFeKFAcy7ffQdboV1uRt` | `sora2ﾖZﾘeｴAdx3ﾂﾉﾔXhnｹﾀ2ﾉｱﾋxﾅﾄﾌヱwﾐmﾊvEﾐCﾏﾎｦ1ﾑHﾋso2GKﾔﾕﾁwﾂﾃP6ﾁｼﾙﾖｺ9ｻｦbﾈ4wFdﾑFヰ3HaﾘｼMｷﾌHWtｷﾋLﾙﾖQ4D3XﾊﾜXmpktﾚｻ5ﾅﾅﾇ1gkﾏsCFQGH9` | مثال أرض العجائب ذات التوقيع المزدوج (الوزن 1 + 2). |
| `addr-multisig-default-quorum3` | `≥3`، الأعضاء `(1,1,1,1)` | `nA2bDNhMqXz7ERkHNoEWbvJGyR1aDRsw32LaUWLgbK3vcpzohmdFCLvdotxUWWDY3aZeX4ptLk4Z6TjF5ossnJm8VrNo6daxmGTkqUyP4MxJxiNyPFxsEE5DLnsoLWUcxaWNpZ76tmkbiGS31Gv8tejKpuiHUMaQ1s5ohWyZvDnpycNkBK8AEfGJqn5yc9zAzfWbVhpDwkPj8ScnzvH1Echr5` | `soraﾐ38ﾅｴｸﾜ8ﾃzwBrqﾘｺ4yﾄv6kqJp1ｳｱﾛｿrzﾄﾃﾘﾒRﾗtV9ｼﾔPｽcヱEﾌVVVｼﾘｲZAｦﾓﾅｦeﾒN76vﾈcuｶuﾛL54rzﾙﾏX2zMﾌRLﾃﾋpﾚpｲcHﾑﾅﾃﾔzｵｲVfAﾃﾚﾎﾚCヰﾔｲｽｦw9ﾔﾕ8bGGkﾁ6sNｼaｻRﾖﾜYﾕﾚU18ﾅHヰﾌuMeﾊtﾂrｿj95Ft8ﾜ3fﾄkNiｴuﾈrCﾐQt8ヱｸｸmﾙﾒgUbﾑEKTTCM` | النصاب القانوني للمجال الافتراضي الضمني المستخدم للإدارة الأساسية.

#### 2.4 قواعد الفشل (ADDR-1a)

- الحمولات الأقصر من الرأس + المحدد المطلوب أو مع وحدات البايت المتبقية تنبعث منها `AccountAddressError::InvalidLength` أو `AccountAddressError::UnexpectedTrailingBytes`.
- يجب رفض الرؤوس التي تحدد `ext_flag` المحجوزة أو تعلن عن إصدارات/فئات غير مدعومة باستخدام `UnexpectedExtensionFlag`، أو `InvalidHeaderVersion`، أو `UnknownAddressClass`.
- علامات محدد/وحدة تحكم غير معروفة ترفع `UnknownDomainTag` أو `UnknownControllerTag`.
- المواد الرئيسية كبيرة الحجم أو المشوهة ترفع `KeyPayloadTooLong` أو `InvalidPublicKey`.
- وحدات تحكم Multisig التي يتجاوز عددها 255 عضوًا ترفع `MultisigMemberOverflow`.
- تحويلات IME/NFKC: يمكن تسوية Sora kana بنصف العرض إلى أشكال العرض الكامل الخاصة بها دون كسر فك التشفير، ولكن يجب أن يظل ASCII `sora` وأرقام/حروف IH58 ASCII. سطح حراس كامل العرض أو مطوي على الحالة `ERR_MISSING_COMPRESSED_SENTINEL`، وترفع حمولات ASCII كاملة العرض `ERR_INVALID_COMPRESSED_CHAR`، وتظهر حالات عدم تطابق المجموع الاختباري كـ `ERR_CHECKSUM_MISMATCH`. تغطي اختبارات الخصائص في `crates/iroha_data_model/src/account/address.rs` هذه المسارات حتى تتمكن مجموعات SDK والمحافظ من الاعتماد على حالات الفشل الحتمية.
- يقوم تحليل Torii وSDK للأسماء المستعارة `address@domain` (rejected legacy form) بإصدار نفس الرموز `ERR_*` عندما تفشل مدخلات IH58 (المفضل)/sora (ثاني أفضل) قبل إرجاع الاسم المستعار (على سبيل المثال، عدم تطابق المجموع الاختباري، عدم تطابق ملخص المجال)، بحيث يمكن للعملاء ترحيل الأسباب المنظمة دون التخمين من سلاسل النثر.
- حمولات المحدد المحلي التي يقل حجمها عن 12 بايت `ERR_LOCAL8_DEPRECATED`، مع الحفاظ على التحويل الثابت من ملخصات Local‑8 القديمة.
- Domainless canonical IH58 literals decode directly to a domainless `AccountId`. Use `ScopedAccountId` only when an interface requires explicit domain context.

#### 2.5 المتجهات الثنائية المعيارية

- **المجال الافتراضي الضمني (`default`، البايت الأولي `0x00`)**  
  الست عشري الأساسي: `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29`.  
  التقسيم: `0x02` الرأس، `0x00` المحدد (افتراضي ضمني)، `0x00` علامة وحدة التحكم، `0x01` معرف المنحنى (Ed25519)، `0x20` طول المفتاح، متبوعًا بحمولة المفتاح 32 بايت.
- **ملخص المجال المحلي (`treasury`، البايت الأولي `0x01`)**  
  الست عشري الأساسي: `0x0201b18fe9c1abbac45b3e38fc5d0001208a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c`.  
  التفصيل: `0x02` الرأس، وعلامة التحديد `0x01` بالإضافة إلى الملخص `b1 8f e9 c1 ab ba c4 5b 3e 38 fc 5d`، متبوعة بحمولة المفتاح الفردي (`0x00`، العلامة `0x01`، معرف المنحنى، `0x20` الطول، 32 بايت مفتاح Ed25519).

تؤكد اختبارات الوحدة (`account::address::tests::parse_encoded_accepts_all_formats`) على متجهات V1 أدناه عبر `AccountAddress::parse_encoded`، مما يضمن أن الأدوات يمكن أن تعتمد على الحمولة الأساسية عبر النماذج السداسية، وIH58 (المفضل)، والنماذج المضغوطة (`sora`، ثاني أفضل النماذج). قم بإعادة إنشاء مجموعة التركيبات الموسعة باستخدام `cargo run -p iroha_data_model --example address_vectors`.

| المجال | بايت البذور | السداسي الكنسي | مضغوط (`sora`) |
|-------------|-----------|-----------|---------|---|
| الافتراضي | `0x00` | `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` | `sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE` |
| الخزانة | `0x01` | `0x0201b18fe9c1abbac45b3e38fc5d0001208a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c` | `sora5ｻu6rﾀCヰTGwﾏ1ﾅヱﾌQｲﾖﾇqCｦヰﾓZQCZRDSSﾅMｱﾙヱｹﾁｸ8ｾeﾄﾛ6C8bZuwﾗｹCZｦRSLQFU` |
| بلاد العجائب | `0x02` | `0x0201b8ae571b79c5a80f5834da2b0001208139770ea87d175f56a35466c34c7ecccb8d8a91b4ee37a25df60f5b8fc9b394` | `sora5ｻwﾓyRｿqﾏnMﾀﾙヰKoﾒﾇﾓQｺﾛyｼ3ｸFHB2F5LyPﾐTMZkｹｼw67ﾋVﾕｻr8ﾉGﾇeEnｻVRNKCS` |
| إيروها | `0x03` | `0x0201de8b36819700c807083608e2000120ed4928c628d1c2c6eae90338905995612959273a5c63f93636c14614ac8737d1` | `sora5ｻﾜxﾀ7Vｱ7QFeｷMﾂLﾉﾃﾏﾓﾀTﾚgSav3Wnｱｵ4ｱCKｷﾛMﾘzヰHiﾐｱ6ﾃﾉﾁﾐZmﾇ2fiﾎX21P4L` |
| ألفا | `0x04` | `0x020146be2154ae86826a3fef0ec0000120ca93ac1705187071d67b83c7ff0efe8108e8ec4530575d7726879333dbdabe7c` | `sora5ｻ9JヱﾈｿuwU6ｴpﾔﾂﾈRqRTds1HﾃﾐｶLVﾍｳ9ﾔhｾNｵVｷyucEﾒGﾈﾏﾍ9sKeﾉDzrｷﾆ742WG1` |
| أوميغا | `0x05` | `0x0201390d946885bc8416b3d30c9d0001206e7a1cdd29b0b78fd13af4c5598feff4ef2a97166e3ca6f2e4fbfccd80505bf1` | `sora5ｻ3zrﾌuﾚﾄJﾑXQhｸTyN8pzwRkWxmjVﾗbﾚﾕヰﾈoｽｦｶtEEﾊﾐ6GPｿﾓﾊｾEhvPｾｻ3XAJ73F` |
| الحكم | `0x06` | `0x0201989eb45a80940d187e2c908f0001208a875fff1eb38451577acd5afee405456568dd7c89e090863a0557bc7af49f17` | `sora5ｻiｵﾁyVﾕｽbFpDHHuﾇﾉdﾗｲﾓﾄRﾋAW3frUCｾ5ｷﾘTwdﾚnｽtQiLﾏｼｶﾅXgｾZmﾒヱH58H4KP` |
| المدققون | `0x07` | `0x0201e4ffa58704c69afaeb7cc2d7000120ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c` | `sora5ｻﾀLDH6VYﾑNAｾgﾉVﾜtxﾊRXLｹﾍﾔﾌLd93GﾔGeｴﾄYrs1ﾂHｸkYxｹwｿyZﾗxyﾎZoXT1S4N` |
| مستكشف | `0x08` | `0x02013b35422c65c2a83c99c523ad0001201398f62c6d1a457c51ba6a4b5f3dbd2f69fca93216218dc8997e416bd17d93ca` | `sora5ｻ4nmｻaﾚﾚPvNLgｿｱv6MHDeEyﾀovﾉJcpvrﾖ6ﾈCQcCNﾇﾜhﾚﾖyFdTwｸｶHEｱ9rWU8FMB` |
| سورانت | `0x09` | `0x0201047d9ea7f5d5dbec3f7bfc58000120fd1724385aa0c75b64fb78cd602fa1d991fdebf76b13c58ed702eac835e9f618` | `sora5ｱｸヱVQﾂcﾁヱRﾓcApｲﾁﾅﾒvﾌﾏfｾNnﾛRJsｿDhﾙuHaﾚｺｦﾌﾍﾈeﾆﾎｺN1UUDｶ6ﾎﾄﾛoRH8JUL` |
| كيتسوني | `0x0A` | `0x0201e91933de397fd7723dc9a76c00012043a72e714401762df66b68c26dfbdf2682aaec9f2474eca4613e424a0fbafd3c` | `sora5ｻﾚｺヱkfFJfSﾁｼJwﾉLvbpSｷﾔMWFMrbｳｸｲｲyヰKGJﾉｻ4ｹﾕrｽhｺｽzSDヰXAN62AD7RGNS` |
| دا | `0x0B` | `0x02016838cf5bb0ce0f3d4f380e1c00012066be7e332c7a453332bd9d0a7f7db055f5c5ef1a06ada66d98b39fb6810c473a` | `sora5ｻNﾒ5SﾐRﾉﾐﾃ62ｿ1ｶｷWFKyF1BcAﾔvｼﾐHqﾙﾐPﾏｴヰ5tｲﾕvnﾙT6ﾀW7mﾔ7ﾇﾗﾂｳ25CXS93` |

تمت المراجعة بواسطة: مجموعة عمل نموذج البيانات، مجموعة عمل التشفير - تمت الموافقة على النطاق لـ ADDR-1a.

##### الأسماء المستعارة المرجعية لـ Sora Nexus

شبكات Sora Nexus الافتراضية هي `chain_discriminant = 0x02F1`
(`iroha_config::parameters::defaults::common::CHAIN_DISCRIMINANT`). ال
`AccountAddress::to_ih58` و`to_compressed_sora` ينبعث منها المساعدون
نماذج نصية متسقة لكل حمولة قانونية. تركيبات مختارة من
`fixtures/account/address_vectors.json` (تم إنشاؤه عبر
`cargo xtask address-vectors`) موضحة أدناه للرجوع إليها بسرعة:

| الحساب / المحدد | IH58 حرفي (البادئة `0x02F1`) | سورا مضغوط (`sora`) حرفي |
|--------------------|--------------------------------|-------------------------|
| `default` المجال (محدد ضمني، أولي `0x00`) | `6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw` | `sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE` (اختياري `@default` لاحقة عند تقديم تلميحات توجيه صريحة) |
| `treasury` (محدد الملخص المحلي، المصدر `0x01`) | `34mSYnCXkCzHXm31UDHh7SJfGvC4QPEhwim8z7sys2iHqXpCwCQkjL8KHvkFLSs1vZdJcb37r` | `sora5ｻu6rﾀCヰTGwﾏ1ﾅヱﾌQｲﾖﾇqCｦヰﾓZQCZRDSSﾅMｱﾙヱｹﾁｸ8ｾeﾄﾛ6C8bZuwﾗｹCZｦRSLQFU` |
| مؤشر التسجيل العمومي (`registry_id = 0x0000_002A`، أي ما يعادل `treasury`) | `3oE9sLeRGP49Cu7mQ1nF4wtKAm29BG4TGLiRsaXe7mhbMP5WZ113nNW1N6RbqF` | `sorakXｹ6NｻﾍﾀﾖSﾜﾖｱ3ﾚ5WﾘﾋQﾅｷｦxgﾛｸcﾁｵﾋkﾋvﾏ8SPﾓﾀｹdｴｴｲW9iCM6AEP` |

تتطابق هذه السلاسل مع تلك الصادرة عن واجهة سطر الأوامر (`iroha tools address convert`)، Torii
الاستجابات (`address_format=ih58|compressed`)، ومساعدي SDK، لذا قم بنسخ/لصق تجربة المستخدم
التدفقات يمكن الاعتماد عليها حرفيا. قم بإلحاق `<address>@<domain>` (rejected legacy form) فقط عندما تحتاج إلى تلميح توجيه صريح؛ اللاحقة ليست جزءًا من الإخراج الأساسي.

#### 2.6 الأسماء المستعارة النصية لقابلية التشغيل البيني (مخطط لها)

- **نمط الاسم المستعار للسلسلة:** `ih:<chain-alias>:<alias@domain>` للسجلات والبشر
  دخول. يجب أن تقوم المحافظ بتحليل البادئة والتحقق من السلسلة المضمنة والحظر
  عدم التطابق.
- **نموذج CAIP-10:** `iroha:<caip-2-id>:<ih58-addr>` لعدم معرفة السلسلة
  التكامل. لم يتم تنفيذ هذا التعيين بعد** في الشحنة
  سلاسل الأدوات.
- **مساعدو الآلة:** نشر برامج الترميز لـ Rust، وTypeScript/JavaScript، وPython،
  وKotlin التي تغطي IH58 والتنسيقات المضغوطة (`AccountAddress::to_ih58`،
  `AccountAddress::parse_encoded`، وما يعادلها من SDK). مساعدي CAIP-10 هم
  العمل المستقبلي.

#### 2.7 الاسم المستعار الحتمي IH58

- **تعيين البادئات:** أعد استخدام `chain_discriminant` كبادئة شبكة IH58.
  `encode_ih58_prefix()` (راجع `crates/iroha_data_model/src/account/address.rs`)
  يُصدر بادئة ذات 6 بت (بايت واحد) للقيم `<64` و14 بت، ثنائية البايت
  نموذج لشبكات أكبر. المهام الرسمية تعيش في
  [`address_prefix_registry.md`](المصدر/المراجع/address_prefix_registry.md);
  يجب أن تحافظ حزم SDK على تسجيل JSON المطابق متزامنًا لتجنب الاصطدامات.
- **مادة الحساب:** يقوم IH58 بتشفير الحمولة الأساسية التي تم إنشاؤها بواسطة
  `AccountAddress::canonical_bytes()` — بايت الرأس، ومحدد المجال، و
  حمولة وحدة التحكم. لا توجد خطوة تجزئة إضافية؛ IH58 يدمج
  حمولة وحدة التحكم الثنائية (مفتاح واحد أو multisig) كما تم إنتاجها بواسطة Rust
  التشفير، وليس خريطة CTAP2 المستخدمة لملخصات سياسة multisig.
- **التشفير:** `encode_ih58()` يربط بايتات البادئة مع البايتات الأساسية
  الحمولة ويلحق مجموع اختباري 16 بت مشتق من Blake2b-512 مع الثابت
  البادئة `IH58PRE` (`b"IH58PRE" || prefix || payload`). والنتيجة هي ترميز Base58 عبر `bs58`.
  يعرض مساعدو CLI/SDK نفس الإجراء، و`AccountAddress::parse_encoded`
  يعكسه عبر `decode_ih58`.

#### 2.8 نواقل الاختبار النصي المعياري

`fixtures/account/address_vectors.json` يحتوي على IH58 كاملاً (المفضل) ومضغوط (`sora`، ثاني أفضل)
حرفية لكل حمولة قانونية. أبرز النقاط:

- **`addr-single-default-ed25519` (Sora Nexus، البادئة `0x02F1`).**  
  IH58 `6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw`، مضغوط (`sora`)
  `sora2QG…U4N5E5`. يُصدر Torii هذه السلاسل الدقيقة من `AccountId`
  تنفيذ `Display` (IH58 الأساسي) و`AccountAddress::to_compressed_sora`.
- **`addr-global-registry-002a` (محدد التسجيل → الخزانة).**  
  IH58 `3oE9sLeRGP49Cu7mQ1nF4wtKAm29BG4TGLiRsaXe7mhbMP5WZ113nNW1N6RbqF`، مضغوط (`sora`)
  `sorakX…CM6AEP`. يوضح أن محددات التسجيل لا تزال تقوم بفك التشفير
  نفس الحمولة الأساسية مثل الملخص المحلي المقابل.
- **حالة الفشل (`ih58-prefix-mismatch`).**  
  تحليل حرف IH58 المشفر بالبادئة `NETWORK_PREFIX + 1` على العقدة
  نتوقع عوائد البادئة الافتراضية
  `AccountAddressError::UnexpectedNetworkPrefix { expected: 753, found: 754 }`
  قبل محاولة توجيه المجال. المباراة `ih58-checksum-mismatch`
  تمارين الكشف عن العبث عبر المجموع الاختباري لـ Blake2b.

#### 2.9 تركيبات الامتثال

يشحن ADDR‑2 حزمة تركيبات قابلة لإعادة التشغيل تغطي الإيجابية والسلبية
السيناريوهات عبر العقدة الأساسية، IH58 (المفضل)، المضغوطة (`sora`، نصف/العرض الكامل)، ضمنية
المحددات الافتراضية، والأسماء المستعارة للسجل العمومي، ووحدات التحكم متعددة التوقيع. ال
يعيش JSON الأساسي في `fixtures/account/address_vectors.json` ويمكن أن يكون كذلك
متجدد مع:

```
cargo xtask address-vectors --out fixtures/account/address_vectors.json
# verify without writing:
cargo xtask address-vectors --verify
```

بالنسبة للتجارب المخصصة (مسارات/تنسيقات مختلفة)، لا يزال المثال الثنائي
متاح:

```
cargo run -p iroha_data_model --example account_address_vectors > fixtures/account/address_vectors.json
```

اختبارات وحدة الصدأ في `crates/iroha_data_model/tests/account_address_vectors.rs`
و`crates/iroha_torii/tests/account_address_vectors.rs`، جنبًا إلى جنب مع JS،
أدوات Swift وAndroid (`javascript/iroha_js/test/address.test.js`،
`IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift`،
`java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java`)،
تستهلك نفس التركيبة لضمان تكافؤ برنامج الترميز عبر مجموعات SDK وقبول Torii.

### 3. النطاقات والتطبيع الفريد عالميًا

راجع أيضًا: [`docs/source/references/address_norm_v1.md`](المصدر/المراجع/address_norm_v1.md)
لخط أنابيب Norm v1 الأساسي المستخدم عبر Torii ونموذج البيانات وحزم تطوير البرامج (SDK).

أعد تعريف `DomainId` كمجموعة ذات علامات:

```
DomainId {
    name: Name,
    authority: GlobalDomainAuthority, // new enum
}

enum GlobalDomainAuthority {
    LocalChain,                  // default for the local chain
    External { chain_discriminant: u16 },
}
```

`LocalChain` يغلف الاسم الحالي للمجالات التي تديرها السلسلة الحالية.
عندما يتم تسجيل النطاق من خلال السجل العالمي، فإننا نستمر في ملكيته
تمييز السلسلة. يبقى العرض/التحليل دون تغيير في الوقت الحالي، ولكن
يسمح الهيكل الموسع باتخاذ قرارات التوجيه.

#### 3.1 التطبيع والدفاعات الانتحال

يحدد Norm v1 المسار الأساسي الذي يجب أن يستخدمه كل مكون قبل النطاق
الاسم مستمر أو مضمن في `AccountAddress`. الإرشادات الكاملة
يعيش في [`docs/source/references/address_norm_v1.md`](source/references/address_norm_v1.md);
يلخص الملخص أدناه الخطوات التي تتبعها المحافظ وTorii وSDK والحوكمة
يجب أن تنفذ الأدوات.

1. **التحقق من صحة الإدخال.** ارفض السلاسل الفارغة والمسافات البيضاء والمحفوظة
   المحددات `@`، `#`، `$`. وهذا يطابق الثوابت التي يفرضها
   `Name::validate_str`.
2. ** تكوين Unicode NFC. ** تطبيق تطبيع NFC المدعوم من ICU بشكل قانوني
   تنهار التسلسلات المكافئة بشكل حتمي (على سبيل المثال، `e\u{0301}` → `é`).
3. **تسوية UTS-46.** قم بتشغيل إخراج NFC من خلال UTS‑46 باستخدام
   `use_std3_ascii_rules = true`، `transitional_processing = false`، و
   تم تمكين فرض طول DNS. والنتيجة هي تسلسل تسمية A صغير؛
   المدخلات التي تنتهك قواعد STD3 تفشل هنا.
4. **حدود الطول.** فرض حدود نمط DNS: يجب أن يكون كل تصنيف من 1 إلى 63
   بايت ويجب ألا يتجاوز النطاق الكامل 255 بايت بعد الخطوة 3.
5. **سياسة اختيارية مربكة.** يتم تعقب عمليات التحقق من البرنامج النصي UTS-39
   نورم الإصدار 2؛ يمكن للمشغلين تمكينها مبكرًا، ولكن الفشل في الفحص يجب أن يتم إحباطه
   معالجة.

إذا نجحت كل مرحلة، فسيتم تخزين سلسلة التسمية A الصغيرة مؤقتًا واستخدامها
ترميز العناوين، والتكوين، والبيانات، وعمليات البحث عن التسجيل. خلاصة محلية
تستمد المحددات قيمتها البالغة 12 بايت كـ `blake2s_mac(key = "SORA-LOCAL-K:v1"،
canonical_label)[0..12]` باستخدام مخرجات الخطوة 3. جميع المحاولات الأخرى (مختلطة
يتم رفض إدخال الحالة، والأحرف الكبيرة، وإدخال Unicode الخام) باستخدام منظم
`ParseError`s عند الحد الذي تم توفير الاسم فيه.

التركيبات الأساسية التي توضح هذه القواعد - بما في ذلك رحلات الذهاب والعودة من Punycode
وتسلسلات STD3 غير الصالحة - مدرجة في
`docs/source/references/address_norm_v1.md` ويتم عكسها في SDK CI
مجموعات المتجهات التي يتم تتبعها ضمن ADDR‑2.

### 4. تسجيل نطاق Nexus وتوجيهه

- **مخطط التسجيل:** يحتفظ Nexus بخريطة موقعة `DomainName -> ChainRecord`
  حيث يتضمن `ChainRecord` تمييز السلسلة وبيانات التعريف الاختيارية (RPC)
  نقاط النهاية)، وإثبات السلطة (على سبيل المثال، التوقيع المتعدد للحوكمة).
- **آلية المزامنة:**
  - تقدم السلاسل مطالبات المجال الموقعة إلى Nexus (إما أثناء التكوين أو عبر
    تعليمات الحوكمة).
  - تنشر Nexus البيانات الدورية (JSON الموقعة بالإضافة إلى جذر Merkle الاختياري)
    عبر HTTPS والتخزين المعنون بالمحتوى (على سبيل المثال، IPFS). العملاء يعلقون
    أحدث البيان والتحقق من التوقيعات.
- **تدفق البحث:**
  - يتلقى Torii معاملة مرجعية `DomainId`.
  - إذا كان المجال غير معروف محليًا، فسيقوم Torii بالاستعلام عن بيان Nexus المخزن مؤقتًا.
  - إذا كان المانيفست يشير إلى سلسلة أجنبية يتم رفض المعاملة معها
    خطأ حتمي `ForeignDomain` ومعلومات السلسلة البعيدة.
  - إذا كان المجال مفقودًا من Nexus، فسيقوم Torii بإرجاع `UnknownDomain`.
- **مرتكزات الثقة والتناوب:** بيانات علامات مفاتيح الحوكمة؛ دوران أو
  يتم نشر الإلغاء كإدخال بيان جديد. العملاء يفرضون البيان
  TTLs (على سبيل المثال، 24 ساعة) ورفض الرجوع إلى البيانات القديمة خارج تلك النافذة.
- **أوضاع الفشل:** إذا فشل استرداد البيان، فسيعود Torii إلى المخزن المؤقت
  البيانات داخل TTL؛ بعد TTL يصدر `RegistryUnavailable` ويرفض
  التوجيه عبر المجال لتجنب حالة غير متناسقة.

### 4.1 ثبات السجل والأسماء المستعارة وشواهد القبور (ADDR-7c)

ينشر Nexus **بيان الإلحاق فقط** بحيث يتم تعيين كل مجال أو اسم مستعار
يمكن تدقيقها واعادتها. يجب على المشغلين التعامل مع الحزمة الموصوفة في
[دليل تشغيل بيان العنوان](source/runbooks/address_manifest_ops.md) باعتباره
المصدر الوحيد للحقيقة: إذا كان البيان مفقودًا أو فشل في التحقق من صحته، فيجب على توري ذلك
رفض حل المجال المتأثر.

دعم الأتمتة: `cargo xtask address-manifest verify --bundle <current_dir> --previous <previous_dir>`
يعيد تشغيل المجموع الاختباري والمخطط وعمليات التحقق من الملخص السابق الموضحة في ملف
runbook. قم بتضمين إخراج الأمر في تذاكر التغيير لإظهار `sequence`
وتم التحقق من صحة الارتباط `previous_digest` قبل نشر الحزمة.

#### العقد الظاهر بالرأس والتوقيع

| المجال | المتطلبات |
|-------|------------|
| `version` | حاليًا `1`. عثرة فقط مع تحديث المواصفات المطابقة. |
| `sequence` | زيادة بمقدار **بالضبط** واحد لكل منشور. ترفض مخابئ Torii المراجعات التي تحتوي على فجوات أو تراجعات. |
| `generated_ms` + `ttl_hours` | إنشاء نضارة ذاكرة التخزين المؤقت (الافتراضي 24 ساعة). إذا انتهت صلاحية TTL قبل النشر التالي، ينقلب Torii إلى `RegistryUnavailable`. |
| `previous_digest` | ملخص BLAKE3 (ست عشري) لنص البيان السابق. يقوم القائمون على التحقق بإعادة حسابه باستخدام `b3sum` لإثبات عدم قابلية التغيير. |
| `signatures` | يتم التوقيع على البيانات عبر Sigstore (`cosign sign-blob`). يجب أن تعمل العمليات على تشغيل `cosign verify-blob --bundle manifest.sigstore manifest.json` وفرض قيود هوية الإدارة/المصدر قبل بدء التشغيل. |

تُصدر أتمتة الإصدار `manifest.sigstore` و`checksums.sha256`
بجانب جسم JSON. احتفظ بالملفات معًا عند النسخ المتطابق إلى SoraFS أو
نقاط نهاية HTTP حتى يتمكن المدققون من إعادة تشغيل خطوات التحقق حرفيًا.

#### أنواع الإدخال

| اكتب | الغرض | الحقول المطلوبة |
|------|---------|----------------|
| `global_domain` | يعلن أن النطاق مسجل عالميًا ويجب تعيينه إلى سلسلة مميزة وبادئة IH58. | `{ "domain": "<label>", "chain": "sora:nexus:global", "ih58_prefix": 753, "selector": "global" }` |
| `tombstone` | يتقاعد الاسم المستعار/المحدد بشكل دائم. مطلوب عند مسح ملخصات Local‑8 أو إزالة مجال. | `{ "selector": {…}, "reason_code": "LOCAL8_RETIREMENT" \| …, "ticket": "<governance id>", "replaces_sequence": <number> }` |

`global_domain` قد تتضمن الإدخالات اختياريًا `manifest_url` أو `sorafs_cid`
لتوجيه المحافظ إلى البيانات الوصفية للسلسلة الموقعة، لكن الصف الأساسي يظل قائمًا
`{domain, chain, discriminant/ih58_prefix}`. `tombstone` السجلات **يجب** الاستشهاد بها
المحدد الذي تم تقاعده وأداة التذكرة/الحوكمة التي سمحت بذلك
التغيير بحيث يكون مسار التدقيق قابلاً لإعادة البناء دون الاتصال بالإنترنت.

#### سير العمل والقياس عن بعد للاسم المستعار/شاهدة القبر

1. **كشف الانحراف.** استخدم `torii_address_local8_total{endpoint}`،
   `torii_address_local8_domain_total{endpoint,domain}`،
   `torii_address_collision_total{endpoint,kind="local12_digest"}`،
   `torii_address_collision_domain_total{endpoint,domain}`،
   `torii_address_domain_total{endpoint,domain_kind}`، و
   `torii_address_invalid_total{endpoint,reason}` (تم تقديمه في
   `dashboards/grafana/address_ingest.json`) لتأكيد عمليات الإرسال المحلية و
   تبقى الاصطدامات المحلية 12 عند الصفر قبل اقتراح علامة مميزة. ال
   تتيح العدادات لكل مجال للمالكين إثبات أن نطاقات التطوير/الاختبار فقط هي التي تصدر Local‑8
   حركة المرور (وتقوم الاصطدامات المحلية 12 بتعيين مجالات التدريج المعروفة) بينما
   يتضمن لوحة **Domain Kind Mix (5m)** حتى تتمكن SREs من رسم بياني لمقدار ذلك
   `domain_kind="local12"` تبقى حركة المرور، و`AddressLocal12Traffic`
   يتم إطلاق التنبيه عندما لا يزال الإنتاج يرى محددات Local-12 على الرغم من
   بوابة التقاعد.
2. **اشتقاق الملخصات الأساسية.** تشغيل
   `iroha tools address convert <address> --format json --expect-prefix 753`
   (أو استخدم `fixtures/account/address_vectors.json` عبر
   `scripts/account_fixture_helper.py`) لالتقاط `digest_hex` بالضبط.
   تقبل واجهة سطر الأوامر IH58، `sora…`، والقيم الأساسية `0x…`؛ إلحاق
   `@<domain>` فقط عندما تحتاج إلى الاحتفاظ بملصق للبيانات.
   يعرض ملخص JSON هذا النطاق عبر الحقل `input_domain`، و
   `legacy  suffix` يعيد تشغيل الترميز المحول كـ `<address>@<domain>` (rejected legacy form) لـ
   الاختلافات الواضحة (هذه اللاحقة عبارة عن بيانات وصفية، وليست معرف حساب أساسي).
   لاستخدام الصادرات الموجهة نحو الخط الجديد
   `iroha tools address normalize --input <file> legacy-selector input mode` لتحويل الكتلة المحلية
   المحددات في نماذج IH58 الأساسية (المفضلة)، أو المضغوطة (`sora`، ثاني أفضل)، أو النماذج السداسية، أو JSON أثناء التخطي
   صفوف غير محلية عندما يحتاج المدققون إلى أدلة مناسبة لجداول البيانات، قم بالتشغيل
   `iroha tools address audit --input <file> --format csv` لإرسال ملخص CSV
   (`input,status,format,domain_kind,…`) الذي يسلط الضوء على المحددات المحلية،
   الترميزات الأساسية وفشل التحليل في نفس الملف.
3. **إلحاق إدخالات البيان.** قم بصياغة السجل `tombstone` (والمتابعة
   `global_domain` قم بالتسجيل عند الترحيل إلى السجل العام) والتحقق من صحته
   البيان مع `cargo xtask address-vectors` قبل طلب التوقيعات.
4. **التحقق والنشر.** اتبع قائمة التحقق من دليل التشغيل (التجزئة، Sigstore،
   رتابة التسلسل) قبل عكس الحزمة إلى SoraFS. توري الآن
   تحديد معايير IH58 (المفضل)/sora (ثاني أفضل) مباشرة بعد وصول الحزمة.
5. **المراقبة والتراجع.** احتفظ بلوحات التصادم المحلية 8 والمحلية 12 في وضع التشغيل
   صفر لمدة 30 يومًا؛ إذا ظهرت التراجعات، قم بإعادة نشر البيان السابق
   فقط في البيئة غير الإنتاجية المتأثرة حتى يستقر القياس عن بعد.

جميع الخطوات المذكورة أعلاه هي دليل إلزامي لـ ADDR‑7c: يظهر بدون
يجب أن تكون حزمة التوقيع `cosign` أو بدون مطابقة قيم `previous_digest`
سيتم رفضه تلقائيًا، ويجب على المشغلين إرفاق سجلات التحقق به
تذاكر التغيير الخاصة بهم.

### 5. بيئة عمل المحفظة وواجهة برمجة التطبيقات

- **عرض الإعدادات الافتراضية:** تعرض المحافظ عنوان IH58 (قصير، ومجمع اختباري)
  بالإضافة إلى المجال الذي تم حله كتسمية تم جلبها من السجل. المجالات هي
  تم وضع علامة واضحة على أنها بيانات تعريف وصفية قد تتغير، في حين أن IH58 هو
  عنوان مستقر.
- **تحديد الإدخال الأساسي:** تقبل Torii وSDKs IH58 (المفضل)/sora (ثاني أفضل)/0x
  العناوين بالإضافة إلى `alias@domain` (rejected legacy form) و `uaid:…` و
  نماذج `opaque:…`، ثم قم بتحويلها إلى IH58 للإخراج. لا يوجد
  تبديل الوضع الصارم؛ يجب أن تظل معرفات الهاتف/البريد الإلكتروني الأولية خارج دفتر الأستاذ
  عبر UAID/تعيينات غير شفافة.
- **منع الأخطاء:** تقوم المحافظ بتحليل بادئات IH58 وفرض تمييز السلسلة
  التوقعات. يؤدي عدم تطابق السلسلة إلى حدوث حالات فشل فادحة من خلال التشخيصات القابلة للتنفيذ.
- **مكتبات الترميز:** Official Rust، وTypeScript/JavaScript، وPython، وKotlin
  توفر المكتبات تشفير/فك تشفير IH58 بالإضافة إلى دعم مضغوط (`sora`) لـ
  تجنب التطبيقات المجزأة. لم يتم شحن تحويلات CAIP-10 بعد.

#### إرشادات إمكانية الوصول والمشاركة الآمنة

- يتم تتبع إرشادات التنفيذ لأسطح المنتج مباشرةً
  `docs/portal/docs/reference/address-safety.md`; الرجوع إلى تلك القائمة المرجعية عندما
  تكييف هذه المتطلبات مع Wallet أو Explorer UX.
- **تدفقات المشاركة الآمنة:** الأسطح التي تنسخ أو تعرض العناوين هي الافتراضية لنموذج IH58 وتكشف عن إجراء "مشاركة" مجاور يقدم كلاً من السلسلة الكاملة ورمز QR المشتق من نفس الحمولة حتى يتمكن المستخدمون من التحقق من المجموع الاختباري بصريًا أو عن طريق المسح. عندما يكون الاقتطاع أمرًا لا مفر منه (على سبيل المثال، الشاشات الصغيرة)، احتفظ ببداية السلسلة ونهايتها، وأضف علامات حذف واضحة، واحتفظ بالعنوان الكامل الذي يمكن الوصول إليه عبر النسخ إلى الحافظة لمنع القص غير المقصود.
- **ضمانات IME:** يجب أن ترفض مدخلات العنوان العناصر التركيبية من لوحات المفاتيح ذات نمط IME/IME. فرض إدخال ASCII فقط، وتقديم تحذير مضمن عند اكتشاف أحرف ذات عرض كامل أو أحرف Kana، وتوفير منطقة لصق نص عادي تزيل علامات الجمع قبل التحقق من الصحة حتى يتمكن المستخدمون اليابانيون والصينيون من تعطيل محرر أسلوب الإدخال (IME) الخاص بهم دون فقدان التقدم.
- **دعم قارئ الشاشة:** توفير تسميات مخفية بشكل مرئي (`aria-label`/`aria-describedby`) تصف أرقام بادئة Base58 الرائدة وتقسم حمولة IH58 إلى مجموعات مكونة من 4 أو 8 أحرف، بحيث تقرأ التكنولوجيا المساعدة الأحرف المجمعة بدلاً من سلسلة تشغيل. أعلن عن نجاح النسخ/المشاركة عبر المناطق المباشرة المهذبة وتأكد من أن معاينات QR تتضمن نصًا بديلًا وصفيًا ("عنوان IH58 لـ <alias> على السلسلة 0x02F1").
- **الاستخدام المضغوط لـ Sora فقط:** قم دائمًا بتسمية العرض المضغوط `sora…` على أنه "Sora فقط" وقم بوضعه خلف تأكيد صريح قبل النسخ. يجب أن ترفض حزم SDK والمحافظ عرض المخرجات المضغوطة عندما لا يكون تمييز السلسلة هو قيمة Sora Nexus ويجب توجيه المستخدمين مرة أخرى إلى IH58 لعمليات النقل بين الشبكات لتجنب إساءة توجيه الأموال.

## قائمة التحقق من التنفيذ

- **مغلف IH58:** البادئة تقوم بتشفير `chain_discriminant` باستخدام المضغوط
  مخطط 6-/14 بت من `encode_ih58_prefix()`، النص هو البايتات الأساسية
  (`AccountAddress::canonical_bytes()`)، والمجموع الاختباري هو أول بايتين
  من Blake2b-512(`b"IH58PRE"` || البادئة || الجسم). الحمولة الكاملة هي Base58-
  تم ترميزه عبر `bs58`.
- **عقد التسجيل:** نشر JSON (وجذر Merkle الاختياري).
  `{discriminant, ih58_prefix, chain_alias, endpoints}` مع TTL لمدة 24 ساعة و
  مفاتيح التدوير.
- **سياسة المجال:** ASCII `Name` اليوم؛ في حالة تمكين i18n، قم بتطبيق UTS-46 لـ
  التطبيع وUTS-39 للفحوصات المربكة. فرض الحد الأقصى للتسمية (63) و
  إجمالي (255) أطوال.
- **المساعدات النصية:** شحن برامج الترميز IH58 ↔ المضغوطة (`sora…`) في Rust،
  TypeScript/JavaScript وPython وKotlin مع ناقلات الاختبار المشتركة (CAIP-10
  تبقى التعيينات العمل في المستقبل).
- **أدوات CLI:** توفير سير عمل محدد للمشغل عبر `iroha tools address convert`
  (راجع `crates/iroha_cli/src/address.rs`)، الذي يقبل IH58/`sora…`/`0x…` الحروف و
  تسميات `<address>@<domain>` (rejected legacy form) اختيارية، افتراضيًا لإخراج IH58 باستخدام بادئة Sora Nexus (`753`)،
  ويصدر فقط الأبجدية المضغوطة الخاصة بـ Sora فقط عندما يطلبها المشغلون صراحةً
  `--format compressed` أو وضع ملخص JSON. يفرض الأمر توقعات البادئة على
  التحليل، ويسجل المجال المقدم (`input_domain` في JSON)، والعلامة `legacy  suffix`
  يعيد تشغيل الترميز المحول كـ `<address>@<domain>` (rejected legacy form) بحيث تظل الفروق الواضحة مريحة.
- **Wallet/explorer UX:** اتبع [إرشادات عرض العنوان](source/sns/address_display_guidelines.md)
  يتم شحنه مع ADDR-6 - قم بتوفير أزرار النسخ المزدوجة، واحتفظ بـ IH58 كحمولة QR، وقم بالتحذير
  المستخدمين أن النموذج `sora…` المضغوط هو Sora فقط وهو عرضة لإعادة كتابة IME.
- **تكامل Torii:** يظهر Cache Nexus احترام TTL، والإصدار
  `ForeignDomain`/`UnknownDomain`/`RegistryUnavailable` بشكل حتمي، و
  keep account-literal parsing encoded-only (`IH58` preferred, `sora…`
  compressed accepted) with canonical IH58 output.

### تنسيقات استجابة توري

- `GET /v1/accounts` يقبل معلمة استعلام اختيارية `address_format` و
  `POST /v1/accounts/query` يقبل نفس الحقل داخل مغلف JSON.
  القيم المدعومة هي:
  - `ih58` (افتراضي) — تصدر الاستجابات حمولات IH58 Base58 الأساسية (على سبيل المثال،
    `6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw`).
  - `compressed` - تصدر الاستجابات عرض Sora فقط `sora…` المضغوط أثناء
    الحفاظ على المرشحات/معلمات المسار الأساسية.
- تُرجع القيم غير الصالحة `400` (`QueryExecutionFail::Conversion`). هذا يسمح
  المحافظ والمستكشفون لطلب سلاسل مضغوطة لـ Sora-only UX بينما
  الاحتفاظ بـ IH58 كإعداد افتراضي قابل للتشغيل البيني.
- قوائم أصحاب الأصول (`GET /v1/assets/{definition_id}/holders`) وJSON الخاصة بهم
  نظيره المغلف (`POST …/holders/query`) يكرم أيضًا `address_format`.
  يقوم الحقل `items[*].account_id` بإصدار قيم حرفية مضغوطة عندما يكون
  تم تعيين حقل المعلمة/المغلف على `compressed`، مما يعكس الحسابات
  نقاط النهاية حتى يتمكن المستكشفون من تقديم مخرجات متسقة عبر الدلائل.
- **الاختبار:** إضافة اختبارات الوحدة لرحلات الذهاب والإياب لجهاز التشفير/وحدة فك التشفير، والسلسلة الخاطئة
  حالات الفشل وعمليات البحث الواضحة؛ إضافة تغطية التكامل في Torii وSDKs
  لتدفقات IH58 نهاية إلى نهاية.

## تسجيل رمز الخطأ

تكشف أجهزة التشفير وأجهزة فك التشفير عن حالات الفشل من خلال
`AccountAddressError::code_str()`. توفر الجداول التالية الرموز الثابتة
أن أدوات تطوير البرامج والمحافظ وأسطح Torii يجب أن تظهر جنبًا إلى جنب مع إمكانية قراءتها بواسطة الإنسان
الرسائل، بالإضافة إلى إرشادات العلاج الموصى بها.

### البناء الكنسي

| الكود | فشل | العلاج الموصى به |
|------|---------|------------------------|
| `ERR_UNSUPPORTED_ALGORITHM` | تلقى برنامج التشفير خوارزمية توقيع غير مدعومة من قبل ميزات التسجيل أو البناء. | تقييد إنشاء الحساب على المنحنيات الممكّنة في التسجيل والتكوين. |
| `ERR_KEY_PAYLOAD_TOO_LONG` | يتجاوز طول حمولة مفتاح التوقيع الحد المدعوم. | تقتصر وحدات التحكم ذات المفتاح الواحد على أطوال `u8`؛ استخدم multisig للمفاتيح العامة الكبيرة (على سبيل المثال، ML‑DSA). |
| `ERR_INVALID_HEADER_VERSION` | إصدار رأس العنوان خارج النطاق المدعوم. | قم بإصدار إصدار الرأس `0` لعناوين V1؛ ترقية أجهزة التشفير قبل اعتماد الإصدارات الجديدة. |
| `ERR_INVALID_NORM_VERSION` | لم يتم التعرف على علامة إصدار التطبيع. | استخدم إصدار التسوية `1` وتجنب تبديل البتات المحجوزة. |
| `ERR_INVALID_IH58_PREFIX` | لا يمكن ترميز بادئة شبكة IH58 المطلوبة. | اختر بادئة ضمن النطاق `0..=16383` الشامل المنشور في سجل السلسلة. |
| `ERR_CANONICAL_HASH_FAILURE` | فشلت تجزئة الحمولة الأساسية. | أعد محاولة العملية؛ إذا استمر الخطأ، فتعامل معه على أنه خطأ داخلي في مكدس التجزئة. |

### فك تشفير التنسيق والكشف التلقائي

| الكود | فشل | العلاج الموصى به |
|------|---------|------------------------|
| `ERR_INVALID_IH58_ENCODING` | تحتوي سلسلة IH58 على أحرف خارج الأبجدية. | تأكد من أن العنوان يستخدم أبجدية IH58 المنشورة وأنه لم يتم اقتطاعه أثناء النسخ/اللصق. |
| `ERR_INVALID_LENGTH` | لا يتطابق طول الحمولة مع الحجم الأساسي المتوقع للمحدد/وحدة التحكم. | قم بتوفير الحمولة الأساسية الكاملة لمحدد المجال وتخطيط وحدة التحكم المحدد. |
| `ERR_CHECKSUM_MISMATCH` | فشل التحقق من صحة المجموع الاختباري IH58 (المفضل) أو المضغوط (`sora`، ثاني أفضل). | إعادة إنشاء العنوان من مصدر موثوق به؛ يشير هذا عادةً إلى خطأ في النسخ/اللصق. |
| `ERR_INVALID_IH58_PREFIX_ENCODING` | بايتات بادئة IH58 مشوهة. | إعادة تشفير العنوان باستخدام برنامج تشفير متوافق؛ لا تقم بتغيير بايت Base58 البادئة يدوياً. |
| `ERR_INVALID_HEX_ADDRESS` | فشل النموذج السداسي العشري الأساسي في فك ترميزه. | قم بتوفير `0x` - سلسلة سداسية ذات طول متساوي مسبوقة بواسطة برنامج التشفير الرسمي. |
| `ERR_MISSING_COMPRESSED_SENTINEL` | النموذج المضغوط لا يبدأ بـ `sora`. | قم ببادئة عناوين Sora المضغوطة بالحارس المطلوب قبل تسليمها إلى أجهزة فك التشفير. |
| `ERR_COMPRESSED_TOO_SHORT` | تفتقر السلسلة المضغوطة إلى أرقام كافية للحمولة والمجموع الاختباري. | استخدم السلسلة المضغوطة الكاملة الصادرة عن برنامج التشفير بدلاً من المقتطفات المقطوعة. |
| `ERR_INVALID_COMPRESSED_CHAR` | تمت مصادفة حرف خارج الأبجدية المضغوطة. | استبدل الحرف بحرف رسومي Base‑105 صالح من الجداول المنشورة ذات العرض الكامل/نصف العرض. |
| `ERR_INVALID_COMPRESSED_BASE` | حاول برنامج التشفير استخدام أساس غير مدعوم. | الإبلاغ عن خطأ ضد برنامج التشفير؛ تم تثبيت الأبجدية المضغوطة على الجذر 105 في V1. |
| `ERR_INVALID_COMPRESSED_DIGIT` | تتجاوز قيمة الرقم حجم الحروف الأبجدية المضغوطة. | تأكد من أن كل رقم موجود ضمن `0..105)`، وأعد إنشاء العنوان إذا لزم الأمر. |
| `ERR_UNSUPPORTED_ADDRESS_FORMAT` | تعذر على الاكتشاف التلقائي التعرف على تنسيق الإدخال. | قم بتوفير IH58 (المفضل)، أو المضغوط (`sora`)، أو السلاسل السداسية `0x` الأساسية عند استدعاء المحللين اللغويين. |

### التحقق من صحة النطاق والشبكة

| الكود | فشل | العلاج الموصى به |
|------|---------|------------------------|
| `ERR_DOMAIN_MISMATCH` | محدد المجال لا يطابق المجال المتوقع. | استخدم عنوانًا تم إصداره للمجال المقصود أو قم بتحديث التوقع. |
| `ERR_INVALID_DOMAIN_LABEL` | فشلت عمليات التحقق من التطبيع لتسمية المجال. | قم بتحديد النطاق باستخدام المعالجة غير الانتقالية UTS-46 قبل التشفير. |
| `ERR_UNEXPECTED_NETWORK_PREFIX` | تختلف بادئة شبكة IH58 التي تم فك تشفيرها عن القيمة التي تم تكوينها. | قم بالتبديل إلى عنوان من السلسلة المستهدفة أو قم بضبط المميز/البادئة المتوقعة. |
| `ERR_UNKNOWN_ADDRESS_CLASS` | لم يتم التعرف على بتات فئة العنوان. | قم بترقية وحدة فك التشفير إلى إصدار يفهم الفئة الجديدة، أو تجنب العبث ببتات الرأس. |
| `ERR_UNKNOWN_DOMAIN_TAG` | علامة محدد المجال غير معروفة. | قم بالتحديث إلى إصدار يدعم نوع المحدد الجديد، أو تجنب استخدام الحمولات التجريبية على عقد V1. |
| `ERR_UNEXPECTED_EXTENSION_FLAG` | تم تعيين بت التمديد المحجوز. | مسح البتات المحجوزة؛ تظل مغلقة حتى يقدمها ABI المستقبلي. |
| `ERR_UNKNOWN_CONTROLLER_TAG` | لم يتم التعرف على علامة حمولة وحدة التحكم. | قم بترقية وحدة فك التشفير للتعرف على أنواع وحدات التحكم الجديدة قبل تحليلها. |
| `ERR_UNEXPECTED_TRAILING_BYTES` | تحتوي الحمولة الأساسية على بايتات زائدة بعد فك التشفير. | إعادة إنشاء الحمولة الأساسية؛ يجب أن يكون الطول الموثق فقط موجودًا. |

### التحقق من صحة حمولة وحدة التحكم

| الكود | فشل | العلاج الموصى به |
|------|---------|------------------------|
| `ERR_INVALID_PUBLIC_KEY` | لا تتطابق بايتات المفاتيح مع المنحنى المعلن. | تأكد من ترميز بايتات المفاتيح تمامًا كما هو مطلوب للمنحنى المحدد (على سبيل المثال، 32 بايت Ed25519). |
| `ERR_UNKNOWN_CURVE` | معرف المنحنى غير مسجل. | استخدم معرف المنحنى `1` (Ed25519) حتى تتم الموافقة على منحنيات إضافية ونشرها في السجل. |
| `ERR_MULTISIG_MEMBER_OVERFLOW` | تعلن وحدة التحكم Multisig عن وجود أعضاء أكثر من المعتمدين. | قم بتقليل عضوية multisig إلى الحد الموثق قبل الترميز. |
| `ERR_INVALID_MULTISIG_POLICY` | فشل التحقق من صحة حمولة سياسة Multisig (العتبة/الأوزان/المخطط). | أعد بناء السياسة بحيث تفي بمخطط CTAP2 وحدود الوزن وقيود العتبة. |

## النظر في البدائل

- **Pure Base58Check (على غرار البيتكوين).** مجموع اختباري أبسط ولكن اكتشاف الأخطاء أضعف
  من المجموع الاختباري IH58 المشتق من Blake2b (`encode_ih58` يقتطع تجزئة 512 بت)
  ويفتقر إلى دلالات البادئة الصريحة لتمييزات 16 بت.
- **تضمين اسم السلسلة في سلسلة المجال (على سبيل المثال، `finance@chain`).** فواصل
- **الاعتماد فقط على توجيه Nexus دون تغيير العناوين.** سيستمر المستخدمون في ذلك
  نسخ/لصق سلاسل غامضة؛ نريد أن يحمل العنوان نفسه السياق.
- **مغلف Bech32m.** متوافق مع QR ويوفر بادئة يمكن للإنسان قراءتها، ولكن
  سوف يختلف عن تطبيق الشحن IH58 (`AccountAddress::to_ih58`)
  وتتطلب إعادة إنشاء جميع التركيبات/حزم تطوير البرامج (SDKs). خريطة الطريق الحالية تحافظ على IH58 +
  دعم مضغوط (`sora`) مع مواصلة البحث في المستقبل
  طبقات Bech32m/QR (تم تأجيل تعيين CAIP-10).

## أسئلة مفتوحة

- تأكد من أن `u16` المميزات بالإضافة إلى النطاقات المحجوزة تغطي الطلب على المدى الطويل؛
  وإلا قم بتقييم `u32` بتشفير متغير.
- وضع اللمسات الأخيرة على عملية إدارة التوقيع المتعدد لتحديثات التسجيل وكيفية القيام بذلك
  تتم معالجة عمليات الإلغاء/التخصيصات منتهية الصلاحية.
- تحديد نظام توقيع البيان الدقيق (على سبيل المثال، Ed25519 متعدد التوقيع) و
  أمان النقل (تثبيت HTTPS، تنسيق تجزئة IPFS) لتوزيع Nexus.
- تحديد ما إذا كان سيتم دعم أسماء النطاقات المستعارة/عمليات إعادة التوجيه لعمليات الترحيل وكيف
  لتسليط الضوء عليها دون كسر الحتمية.
- حدد كيفية وصول عقود Kotodama/IVM إلى مساعدي IH58 (`to_address()`،
  `parse_address()`) وما إذا كان التخزين على السلسلة يجب أن يعرض CAIP-10 على الإطلاق
  التعيينات (اليوم IH58 هو المعيار).
- استكشاف تسجيل سلاسل Iroha في السجلات الخارجية (على سبيل المثال، سجل IH58،
  دليل مساحة اسم CAIP) لمحاذاة النظام البيئي على نطاق أوسع.

## الخطوات التالية

1. وصل ترميز IH58 إلى `iroha_data_model` (`AccountAddress::to_ih58`،
   `parse_encoded`); استمر في نقل التركيبات/الاختبارات إلى كل SDK وقم بإزالة أي منها
   العناصر النائبة Bech32m.
2. قم بتوسيع مخطط التكوين باستخدام `chain_discriminant` واشتقاقه بشكل معقول
  الإعدادات الافتراضية لإعدادات الاختبار/التطوير الحالية. **(تم: `common.chain_discriminant`
  يتم الشحن الآن في `iroha_config`، افتراضيًا هو `0x02F1` مع كل شبكة
  يتجاوز.)**
3. قم بصياغة مخطط تسجيل Nexus وناشر بيان إثبات المفهوم.
4. جمع التعليقات من موفري المحفظة والأمناء بشأن جوانب العامل البشري
   (تسمية HRP، تنسيق العرض).
5. قم بتحديث الوثائق (`docs/source/data_model.md`، مستندات Torii API) بمجرد
   مسار التنفيذ ملتزم.
6. شحن مكتبات الترميز الرسمية (Rust/TS/Python/Kotlin) بالاختبار المعياري
   ناقلات تغطي حالات النجاح والفشل.

---
lang: ar
direction: rtl
source: docs/genesis.md
status: complete
translator: manual
source_hash: 975c403019da0b8700489610d86a75f26886f40f2b4c10963ee8269a68b4fe9b
source_last_modified: "2025-11-12T00:33:55.324259+00:00"
translation_last_reviewed: 2025-11-14
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/genesis.md (Genesis configuration) -->

# تهيئة Genesis

يعرّف ملف `genesis.json` أولى المعاملات التي تُنفَّذ عند بدء تشغيل شبكة
Iroha. الملف عبارة عن كائن JSON يحتوي الحقول التالية:

- `chain`: معرّف سلسلة (chain) فريد.
- `executor` (اختياري): مسار (path) إلى bytecode الخاص بالـ executor
  (ملف `.to`). عند وجوده، يضيف genesis تعليمة `Upgrade` كأول معاملة. عند
  غيابه، لا يتم إجراء upgrade ويُستخدم الـ executor المدمج.
- `ivm_dir`: دليل يحتوي على مكتبات bytecode الخاص بـ IVM. القيمة
  الافتراضية `"."` إذا لم يُذكر.
- `transactions`: قائمة بمعاملات genesis تُنفَّذ تسلسليًا. يمكن أن تحتوي
  كلّ واحدة على:
  - `parameters`: معاملات الشبكة الابتدائية.
  - `instructions`: تعليمات مشفَّرة باستخدام Norito.
  - `ivm_triggers`: triggers مع ملفات تنفيذية لـ IVM.
  - `topology`: طوبولوجيا الـ peers الابتدائية. يمكن أن يتضمن كل إدخال
    `pop_hex` (اختياري) لإثبات الحيازة، لكنه يجب أن يكون موجودًا قبل التوقيع.
- `crypto`: لقطة (snapshot) لإعدادات التشفير، تعكس
  `iroha_config.crypto` (`default_hash`, `allowed_signing`,
  `allowed_curve_ids`, `sm2_distid_default`, `sm_openssl_preview`).
  يعكس الحقل `allowed_curve_ids` القيمة
  `crypto.curves.allowed_curve_ids` بحيث يمكن للـ manifests الإعلان عن
  منحنيات (curves) الـ controllers المدعومة في الـ cluster. تفرض الأدوات
  تركيبات SM صحيحة: manifests التي تذكر `sm2` يجب أن تضبط كذلك قيمة
  الـ hash إلى `sm3-256`، في حين أن الـ builds التي تُجمَّع بدون
  feature `sm` ترفض `sm2` تمامًا.

مثال (خروج الأمر `kagami genesis generate default`، مع اختصار جزء
التعليمات):

```json
{
  "chain": "00000000-0000-0000-0000-000000000000",
  "ivm_dir": "defaults",
  "transactions": [
    {
      "parameters": { "sumeragi": { "block_time_ms": 2000 } },
      "instructions": [78, 82, 84, 48, 0, 0, 19, 123, ...],
      "ivm_triggers": [],
      "topology": []
    }
  ],
  "crypto": {
    "default_hash": "blake2b-256",
    "allowed_signing": ["ed25519"],
    "allowed_curve_ids": [1],
    "sm2_distid_default": "1234567812345678",
    "sm_openssl_preview": false
  }
}
```

### تهيئة كتلة `crypto` لـ SM2/SM3

يمكنك استخدام أداة `xtask` المساعدة لإنتاج جرد المفاتيح (key inventory)
ومقطع إعدادات (snippet) جاهز للّصق في خطوة واحدة:

```bash
cargo xtask sm-operator-snippet \
  --distid CN12345678901234 \
  --json-out sm2-key.json \
  --snippet-out client-sm2.toml
```

يصبح محتوى `client-sm2.toml` كما يلي:

```toml
# Account key material
public_key = "sm2:86264104..."
private_key = "A333F581EC034C1689B750A827E150240565B483DEB28294DDB2089AD925A569"
# public_key_pem = """\
-----BEGIN PUBLIC KEY-----
...
-----END PUBLIC KEY-----
"""
# private_key_pem = """\
-----BEGIN PRIVATE KEY-----
...
-----END PRIVATE KEY-----
"""

[crypto]
default_hash = "sm3-256"
allowed_signing = ["ed25519", "sm2"]  # أزل "sm2" للبقاء في وضع التحقق فقط
allowed_curve_ids = [1]               # أضف curve ids جديدة (مثل 15 لـ SM2) عندما يُسمح بالـ controllers
sm2_distid_default = "CN12345678901234"
# enable_sm_openssl_preview = true  # اختياري: فقط عند استخدام مسار OpenSSL/Tongsuo
```

انسخ قيم `public_key` و`private_key` إلى إعدادات الحساب/العميل، ثم حدِّث
كتلة `crypto` في `genesis.json` بحيث تطابق هذا المقطع (مثل ضبط
`default_hash` على `sm3-256`، وإضافة `"sm2"` إلى `allowed_signing`، وإدراج
`allowed_curve_ids` المناسبة). سيرفض Kagami أي manifest تكون فيه إعدادات
hash/curves وقائمة خوارزميات التوقيع المسموح بها غير متّسقة.

> **ملاحظة:** يمكنك إرسال المقطع إلى stdout باستخدام
> `--snippet-out -` عندما تريد فقط معاينة الإخراج. استخدم `--json-out -`
> لعرض جرد المفاتيح على stdout كذلك.

إذا فضّلت تنفيذ أوامر CLI ذات المستوى المنخفض يدويًا، فالتدفّق المكافئ
هو:

```bash
# 1. إنشاء مواد مفاتيح حتمية (يكتب JSON على القرص)
cargo run -p iroha_cli --features sm -- \
  crypto sm2 keygen \
  --distid CN12345678901234 \
  --output sm2-key.json

# 2. إعادة تكوين المقطع الذي يمكن لصقه في ملفات العميل/الإعدادات
cargo run -p iroha_cli --features sm -- \
  crypto sm2 export \
  --private-key-hex "$(jq -r .private_key_hex sm2-key.json)" \
  --distid CN12345678901234 \
  --snippet-output client-sm2.toml \
  --emit-json --quiet
```

> **ملاحظة:** يُستخدم `jq` في المثال أعلاه لتجنّب خطوة النسخ/اللصق
> اليدوية. إذا لم يكن متوفرًا، افتح `sm2-key.json`، وانسخ الحقل
> `private_key_hex`، ومرّره مباشرةً إلى `crypto sm2 export`.

> **دليل الهجرة:** عند تحويل شبكة قائمة إلى SM2/SM3/SM4، اتبع
> [`docs/source/crypto/sm_config_migration.md`](source/crypto/sm_config_migration.md)
> لتطبيق طبقات الـ overrides على `iroha_config`، وإعادة توليد manifests،
> والتخطيط للـ rollback.

## التوليد والتحقق

1. توليد قالب (template):

   ```bash
   cargo run -p iroha_kagami -- genesis generate \
     [--executor <path/to/executor.to>] \
     [--genesis-public-key <public-key>] \
     > genesis.json
   ```

   - يشير `--executor` (اختياريًا) إلى ملف `.to` خاص بالـ executor
     لـ IVM؛ عند وجوده، يصدر Kagami تعليمة `Upgrade` كأول معاملة.
   - يحدِّد `--genesis-public-key` المفتاح العام المستخدم لتوقيع
     genesis block؛ يجب أن يكون multihash مدعومًا في
     `iroha_crypto::Algorithm` (بما في ذلك متغيّرات GOST TC26 عند
     تفعيل الـ feature المناسب).

2. التحقق أثناء التحرير:

   ```bash
   cargo run -p iroha_kagami -- genesis validate genesis.json
   ```

   يتحقق هذا الأمر من أن `genesis.json` يطابق الـ schema، وأن المعلمات
   صالحة، وأن قيم `Name` صحيحة، وأن تعليمات Norito قابلة لفك التشفير.

3. التوقيع من أجل النشر (deployment):

   ```bash
   cargo run -p iroha_kagami -- genesis sign \
     genesis.json \
     --public-key <PK> \
     --private-key <SK> \
     --out-file genesis.signed.nrt
   ```

   عند بدء تشغيل `irohad` مع `--genesis-manifest-json` فقط (بدون genesis
   block موقّع)، يقوم الـ node بتهيئة إعدادات التشفير runtime تلقائيًا من
   الـ manifest؛ وإذا قدّمت أيضًا genesis block، فيجب أن يتطابق الـ
   manifest والإعدادات تمامًا.

تتحقق `kagami genesis sign` من صحة JSON وتنتج block مشفّرًا بـ Norito جاهزًا
للاستخدام عبر `genesis.file` في إعدادات الـ node. يكون الناتج
`genesis.signed.nrt` بالفعل في صيغة wire معيارية: بايت إصدار واحد يليه
ترويسة Norito تصف layout الحمولة. يجب دائمًا توزيع هذا الإخراج
المؤطَّر. يُفضَّل استخدام اللاحقة `.nrt` للـ payloads الموقّعة؛ إذا لم
تحتج إلى تحديث الـ executor في genesis، يمكنك حذف الحقل `executor` وعدم
توفير ملف `.to`.

## ما الذي يمكن لـ Genesis فعله؟

يدعم genesis العمليات التالية. يقوم Kagami بتجميعها في معاملات وفق
ترتيب محدد بدقة، بحيث تنفّذ جميع الـ peers نفس التسلسل بشكل حتمي:

- **المعلمات (Parameters)**: ضبط القيم الابتدائية لـ Sumeragi (أوقات
  block/commit، والانحراف)، ولـ Block (أقصى عدد من المعاملات)، ولـ
  Transaction (أقصى عدد من التعليمات وحجم bytecode)، ولـ executor
  والعقود الذكية (الوقود، الذاكرة، العمق)، بالإضافة إلى معلمات مخصّصة.
  يزرع Kagami `Sumeragi::NextMode` وpayload `sumeragi_npos_parameters`
  داخل كتلة `parameters`، وتحتوي الكتلة الموقَّعة على تعليمات
  `SetParameter` المُولَّدة بحيث يمكن لتشغيل العقدة (startup) تطبيق
  إعدادات الإجماع اعتمادًا على حالة on‑chain.
- **التعليمات الأصلية (Native Instructions)**: تسجيل/إلغاء تسجيل
  Domain وAccount وAsset Definition؛ سك/حرق/نقل الأصول؛ نقل ملكية
  domains وasset definitions؛ تعديل metadata؛ منح الأذونات (permissions)
  والأدوار (roles).
- **Triggers الخاصة بـ IVM**: تسجيل triggers تُنفِّذ bytecode لـ IVM (انظر
  `ivm_triggers`). تتمّ معالجة executables الخاصة بالـ triggers بشكل نسبي
  إلى `ivm_dir`.
- **الطوبولوجيا (Topology)**: توفير المجموعة الابتدائية من الـ peers عبر
  مصفوفة `topology` (قائمة من `PeerId`) داخل أي معاملة (عادةً الأولى أو
  الأخيرة).
- **تحديث executor (اختياري)**: إذا كان `executor` موجودًا، يُدرِج genesis
  تعليمة `Upgrade` واحدة كأول معاملة؛ وإلا يبدأ genesis مباشرةً من
  المعلمات/التعليمات.

### ترتيب المعاملات

من الناحية المفاهيمية، تتم معالجة معاملات genesis بالترتيب التالي:

1. (اختياري) Upgrade للـ executor  
2. لكل معاملة ضمن `transactions`:
   - تحديثات المعلمات
   - التعليمات الأصلية
   - تسجيل triggers الخاصة بـ IVM
   - إدخالات الطوبولوجيا

يضمن كل من Kagami وكود الـ node هذا الترتيب، بحيث تُطبَّق المعلمات قبل
التعليمات اللاحقة داخل المعاملة نفسها.

## سير العمل الموصى به

- البدء من قالب يُولّده Kagami:
  - ISI مدمجة فقط:  
    `kagami genesis generate --ivm-dir <dir> --genesis-public-key <PK> [--consensus-mode npos] > genesis.json`
  - مع upgrade اختياري للـ executor: إضافة `--executor <path/to/executor.to>`
- `<PK>` هو أي multihash مدعوم في `iroha_crypto::Algorithm`، بما في ذلك
  متغيرات GOST TC26 عندما يُبنى Kagami مع `--features gost` (مثل
  `gost3410-2012-256-paramset-a:...`).
- التحقق أثناء التحرير: `kagami genesis validate genesis.json`
- التوقيع للنشر:
  `kagami genesis sign genesis.json --public-key <PK> --private-key <SK> --out-file genesis.signed.nrt`
- إعداد الـ peers: تعيين `genesis.file` للإشارة إلى ملف Norito الموقَّع
  (مثل `genesis.signed.nrt`) وتعيين `genesis.public_key` إلى نفس `<PK>`
  المستخدم في التوقيع.

ملاحظات:
- يقوم قالب Kagami الافتراضي بتسجيل domain وحسابات مثال، وسكّ بعض
  الأصول، ومنح أقل قدر من الأذونات باستخدام ISIs مدمجة فقط ــ دون الحاجة
  إلى ملف `.to`.
- إذا أضفت upgrade للـ executor، فيجب أن تكون تلك أول معاملة؛ يفرض
  Kagami ذلك عند التوليد/التوقيع.
- استخدم `kagami genesis validate` لاكتشاف قيم `Name` غير صالحة
  (مثل وجود مسافات بيضاء) وتعليمات غير صحيحة قبل التوقيع.

## التشغيل مع Docker/Swarm

تتعامل إعدادات Docker Compose وSwarm المتوفرة مع الحالتين:

- بدون executor: يقوم أمر compose بإزالة الحقل `executor` إذا كان مفقودًا
  أو فارغًا ثم يوقّع الملف.
- مع executor: يقوم بتحويل المسار النسبي للـ executor إلى مسار مطلق داخل
  الـ container ثم يوقّع الملف.

يُبقي هذا النسق عملية التطوير بسيطة على الأجهزة التي لا تحتوي على عينات
IVM جاهزة، مع الاستمرار في إتاحة ترقيات executor عند الحاجة.

</div>

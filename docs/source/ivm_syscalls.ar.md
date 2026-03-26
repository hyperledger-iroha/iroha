---
lang: ar
direction: rtl
source: docs/source/ivm_syscalls.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bcf280df1e00065199d386e07b9fd67d8f94c4046d73cfa3b63d1eec18228cd8
source_last_modified: "2026-01-22T16:01:14.866000+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# IVM Syscall ABI

يحدد هذا المستند أرقام syscall IVM، واصطلاحات استدعاء المؤشر-ABI، ونطاقات الأرقام المحجوزة، والجدول المتعارف عليه لمكالمات النظام التي تواجه العقد والتي يستخدمها خفض Kotodama. وهو يكمل `ivm.md` (الهندسة المعمارية) و`kotodama_grammar.md` (اللغة).

الإصدار
- تعتمد مجموعة مكالمات النظام المعترف بها على حقل رأس الرمز الثانوي `abi_version`. يقبل الإصدار الأول `abi_version = 1` فقط؛ يتم رفض القيم الأخرى عند القبول. الأرقام غير المعروفة لـ `abi_version` النشطة تتوافق بشكل حتمي مع `E_SCALL_UNKNOWN`.
- تحافظ ترقيات وقت التشغيل على `abi_version = 1` ولا تعمل على توسيع أسطح syscall أو المؤشر-ABI.
- تعد تكاليف غاز Syscall جزءًا من جدول الغاز المُصدر والمرتبط بإصدار رأس الرمز الثانوي. راجع `ivm.md` (سياسة الغاز).

نطاقات الترقيم
- `0x00..=0x1F`: VM core/Utility (تتوفر مساعدات تصحيح الأخطاء/الخروج ضمن `CoreHost`؛ أما مساعدو التطوير المتبقيون فهم مضيفون وهميون فقط).
- `0x20..=0x5F`: جسر Iroha الأساسي لـ ISI (مستقر في ABI v1).
- `0x60..=0x7F`: امتداد ISIs مسور بميزات البروتوكول (لا يزال جزءًا من ABI v1 عند التمكين).
- `0x80..=0xFF`: مساعدو المضيف/التشفير والفتحات المحجوزة؛ يتم قبول الأرقام الموجودة في القائمة المسموح بها لـ ABI v1 فقط.

المساعدات الدائمة (ABI v1)
- تعد استدعاءات النظام المساعدة للحالة الدائمة (0x50–0x5A: STATE_{GET,SET,DEL}, ENCODE/DECODE_INT, BUILD_PATH_*, JSON/SCHEMA encode/decode) جزءًا من V1 ABI ومضمنة في حساب `abi_hash`.
- يقوم CoreHost بتوصيل STATE_{GET,SET,DEL} إلى حالة العقد الذكي المتين المدعومة من WSV؛ قد يستمر مضيفو dev/test محليًا ولكن يجب أن يحافظوا على دلالات syscall متطابقة.

اصطلاح استدعاء Pointer-ABI (مكالمات نظام العقد الذكي)
- يتم وضع الوسائط في السجلات `r10+` كقيم `u64` أولية أو كمؤشرات في منطقة INPUT إلى مظاريف Norito TLV غير القابلة للتغيير (على سبيل المثال، `AccountId`، `AssetDefinitionId`، `Name`، `Json`، `NftId`).
- قيم الإرجاع العددية هي `u64` التي تم إرجاعها من المضيف. تتم كتابة نتائج المؤشر بواسطة المضيف في `r10`.

جدول syscall الكنسي (مجموعة فرعية)| عرافة | الاسم | الوسائط (في `r10+`) | عوائد | غاز (قاعدي + متغير) | ملاحظات |
|------|-------------------------------------------|--------------------------------------------------------------------------|-------------|------------------------------|-------|
| 0x1A | SET_ACCOUNT_DETAIL | `&AccountId`، `&Name`، `&Json` | `u64=0` | `G_set_detail + bytes(val)` | يكتب تفاصيل الحساب |
| 0x22 | MINT_ASSET | `&AccountId`، `&AssetDefinitionId`، `&NoritoBytes(Numeric)` | `u64=0` | `G_mint` | النعناع `amount` من الأصول إلى الحساب |
| 0x23 | حرق_الأصول | `&AccountId`، `&AssetDefinitionId`، `&NoritoBytes(Numeric)` | `u64=0` | `G_burn` | يحرق `amount` من الحساب |
| 0x24 | تحويل_الأصول | `&AccountId(from)`، `&AccountId(to)`، `&AssetDefinitionId`، `&NoritoBytes(Numeric)` | `u64=0` | `G_transfer` | التحويلات `amount` بين الحسابات |
| 0x29 | TRANSFER_V1_BATCH_BEGIN | – | `u64=0` | `G_transfer` | بدء نطاق دفعة نقل FASTPQ |
| 0x2A | TRANSFER_V1_BATCH_END | – | `u64=0` | `G_transfer` | تدفق دفعة نقل FASTPQ المتراكمة |
| 0x2B | TRANSFER_V1_BATCH_APPLY | `r10=&NoritoBytes(TransferAssetBatch)` | `u64=0` | `G_transfer` | تطبيق دفعة مشفرة بـ Norito في مكالمة نظام واحدة |
| 0x25 | NFT_MINT_ASSET | `&NftId`، `&AccountId(owner)` | `u64=0` | `G_nft_mint_asset` | يسجل NFT جديد |
| 0x26 | NFT_TRANSFER_ASSET | `&AccountId(from)`، `&NftId`، `&AccountId(to)` | `u64=0` | `G_nft_transfer_asset` | ينقل ملكية NFT |
| 0x27 | NFT_SET_METADATA | `&NftId`، `&Json` | `u64=0` | `G_nft_set_metadata` | تحديثات البيانات التعريفية NFT |
| 0x28 | NFT_BURN_ASSET | `&NftId` | `u64=0` | `G_nft_burn_asset` | بيرنز (يدمر) NFT |
| 0xA1 | SMARTCONTRACT_EXECUTE_QUERY| `r10=&NoritoBytes(QueryRequest)` | `r10=ptr (&NoritoBytes(QueryResponse))` | `G_scq + per_item*items + per_byte*bytes(resp)` | يتم تشغيل الاستعلامات القابلة للتكرار بشكل سريع الزوال؛ تم رفض `QueryRequest::Continue` |
| 0xA2 | CREATE_NFTS_FOR_ALL_USERS | – | `u64=count` | `G_create_nfts_for_all` | المساعد؛ ميزة بوابات || 0xA3 | SET_SMARTCONTRACT_EXECUTION_DEPTH | `depth:u64` | `u64=prev` | `G_set_depth` | مسؤل؛ ميزة بوابات |
| 0xA4 | الحصول على السلطة | – (المضيف يكتب النتيجة) | `&AccountId`| `G_get_auth` | يكتب المضيف المؤشر إلى السلطة الحالية في `r10` |
| 0xF7 | GET_MERKLE_PATH | `addr:u64`، `out_ptr:u64`، اختياري `root_out:u64` | `u64=len` | `G_mpath + len` | يكتب المسار (leaf → root) وبايتات الجذر الاختيارية |
| 0xFA | GET_MERKLE_COMPACT | `addr:u64`، `out_ptr:u64`، اختياري `depth_cap:u64`، اختياري `root_out:u64` | `u64=depth` | `G_mpath + depth` | `[u8 depth][u32 dirs_le][u32 count][count*32 siblings]` |
| 0xFF | GET_REGISTER_MERKLE_COMPACT| `reg_index:u64`، `out_ptr:u64`، اختياري `depth_cap:u64`، اختياري `root_out:u64` | `u64=depth` | `G_mpath + depth` | نفس التخطيط المضغوط لالتزام التسجيل |

إنفاذ الغاز
- يقوم CoreHost بفرض رسوم إضافية على مكالمات نظام ISI باستخدام جدول ISI الأصلي؛ يتم فرض رسوم على عمليات نقل دفعة FASTPQ لكل إدخال.
- تقوم مكالمات نظام ZK_VERIFY بإعادة استخدام جدول غاز التحقق السري (حجم القاعدة + الإثبات).
- SMARTCONTRACT_EXECUTE_QUERY يفرض رسومًا أساسية + لكل عنصر + لكل بايت؛ يضاعف الفرز تكلفة كل عنصر وتضيف الإزاحات غير المصنفة عقوبة لكل عنصر.

ملاحظات
- تشير كافة وسيطات المؤشر إلى مظاريف Norito TLV في منطقة INPUT ويتم التحقق من صحتها عند إلغاء المرجع الأول (`E_NORITO_INVALID` عند حدوث خطأ).
- يتم تطبيق جميع الطفرات عبر المنفذ القياسي لـ Iroha (من خلال `CoreHost`)، وليس مباشرة بواسطة VM.
- يتم تحديد ثوابت الغاز الدقيقة (`G_*`) من خلال جدول الغاز النشط؛ راجع `ivm.md`.

أخطاء
- `E_SCALL_UNKNOWN`: لم يتم التعرف على رقم syscall لـ `abi_version` النشط.
- تنتشر أخطاء التحقق من صحة الإدخال كمصائد VM (على سبيل المثال، `E_NORITO_INVALID` لـ TLVs المشوهة).

المراجع الترافقية
- دلالات الهندسة المعمارية والجهاز الظاهري: `ivm.md`
- اللغة ورسم الخرائط المدمج: `docs/source/kotodama_grammar.md`

مذكرة الجيل
- يمكن إنشاء قائمة كاملة بثوابت syscall من المصدر باستخدام:
  - `make docs-syscalls` → يكتب `docs/source/ivm_syscalls_generated.md`
  - `make check-docs` → يتحقق من أن الجدول الذي تم إنشاؤه محدث (مفيد في CI)
- تظل المجموعة الفرعية أعلاه عبارة عن جدول ثابت ومنظم لمكالمات النظام التي تواجه العقد.

## أمثلة على TLV للمسؤول/الدور (مضيف وهمي)

يوثق هذا القسم أشكال TLV والحد الأدنى من حمولات JSON التي يقبلها مضيف WSV الوهمي لاستدعاءات النظام على نمط المسؤول المستخدمة في الاختبارات. تتبع كافة وسيطات المؤشر المؤشر-ABI (مغلفات Norito TLV الموضوعة في INPUT). قد يستخدم مضيفو الإنتاج مخططات أكثر ثراءً؛ تهدف هذه الأمثلة إلى توضيح الأنواع والأشكال الأساسية.- REGISTER_PEER / UNREGISTER_PEER
  - الوسيطات: `r10=&Json`
  - مثال JSON: `{ "peer": "peer-id-or-info" }`
  - ملاحظة CoreHost: `REGISTER_PEER` يتوقع كائن `RegisterPeerWithPop` JSON مع `peer` + `pop` بايت (`activation_at` اختياري، `expiry_at`، `hsm`)؛ يقبل `UNREGISTER_PEER` سلسلة معرف النظير أو `{ "peer": "..." }`.

- CREATE_TRIGGER / REMOVE_TRIGGER / SET_TRIGGER_ENABLED
  - إنشاء_TRIGGER:
    - الوسيطات: `r10=&Json`
    - الحد الأدنى من JSON: `{ "name": "t1" }` (تم تجاهل الحقول الإضافية بواسطة النموذج الوهمي)
  - REMOVE_TRIGGER:
    - الوسائط: `r10=&Name` (اسم المشغل)
  - SET_TRIGGER_ENABLED:
    - الوسائط: `r10=&Name`، `r11=enabled:u64` (0 = معطل، غير صفر = ممكّن)
  - ملاحظة CoreHost: يتوقع `CREATE_TRIGGER` مواصفات تشغيل كاملة (سلسلة base64 Norito `Trigger` أو
    `{ "id": "<trigger_id>", "action": ... }` مع `action` كسلسلة أساسية64 Norito `Action` أو
    كائن JSON)، ويقوم `SET_TRIGGER_ENABLED` بتبديل مفتاح بيانات تعريف المشغل `__enabled` (مفقود
    الإعدادات الافتراضية للتمكين).

- الأدوار: CREATE_ROLE / DELETE_ROLE / GRANT_ROLE / REVOKE_ROLE
  - إنشاء_دور:
    - الوسائط: `r10=&Name` (اسم الدور)، `r11=&Json` (مجموعة الأذونات)
    - يقبل JSON إما المفتاح `"perms"` أو `"permissions"`، وكل منهما عبارة عن مجموعة سلسلة من أسماء الأذونات.
    - أمثلة:
      -`{ "perms": [ "mint_asset:rose#wonder" ] }`
      -`{ "permissions": [ "read_assets:<i105-account-id>", "transfer_asset:rose#wonder" ] }`
    - بادئات اسم الإذن المدعومة في النسخة الوهمية:
      - `register_domain`، `register_account`، `register_asset_definition`
      -`read_assets:<account_id>`
      -`mint_asset:<asset_definition_id>`
      -`burn_asset:<asset_definition_id>`
      -`transfer_asset:<asset_definition_id>`
  - DELETE_ROLE:
    - الوسيطات: `r10=&Name`
    - يفشل إذا كان لا يزال يتم تعيين هذا الدور لأي حساب.
  - GRANT_ROLE / REVOKE_ROLE:
    - الوسائط: `r10=&AccountId` (الموضوع)، `r11=&Name` (اسم الدور)
  - ملاحظة CoreHost: قد يكون إذن JSON عبارة عن كائن `Permission` كامل (`{ "name": "...", "payload": ... }`) أو سلسلة (الإعدادات الافتراضية للحمولة هي `null`)؛ `GRANT_PERMISSION`/`REVOKE_PERMISSION` يقبل `&Name` أو `&Json(Permission)`.

- إلغاء تسجيل العمليات (المجال/الحساب/الأصول): الثوابت (وهمية)
  - يفشل UNREGISTER_DOMAIN (`r10=&DomainId`) في حالة وجود حسابات أو تعريفات أصول في المجال.
  - يفشل UNREGISTER_ACCOUNT (`r10=&AccountId`) إذا كان الحساب يحتوي على أرصدة غير صفرية أو يمتلك NFTs.
  - يفشل UNREGISTER_ASSET (`r10=&AssetDefinitionId`) في حالة وجود أي أرصدة للأصل.

ملاحظات
- تعكس هذه الأمثلة مضيف WSV الوهمي المستخدم في الاختبارات؛ قد يعرض مضيفو العقدة الحقيقية مخططات إدارية أكثر ثراءً أو يحتاجون إلى تحقق إضافي. لا تزال قواعد المؤشر-ABI سارية: يجب أن تكون TLVs في INPUT، الإصدار=1، ويجب أن تتطابق معرفات النوع، ويجب التحقق من صحة تجزئات الحمولة.
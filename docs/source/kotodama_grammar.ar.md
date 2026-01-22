---
lang: ar
direction: rtl
source: docs/source/kotodama_grammar.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f9d64b88546c924258ef054d8071b38230f3f19c8a7d920f9594b0ecb84252ce
source_last_modified: "2025-12-04T09:32:10.286919+00:00"
translation_last_reviewed: 2026-01-05
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/source/kotodama_grammar.md -->

# قواعد لغة Kotodama ودلالاتها

يحدد هذا المستند نحو لغة Kotodama (التحليل المعجمي والنحو)، وقواعد الأنواع، والدلالات الحتمية، وكيف تُخفض البرامج إلى بايت كود IVM (`.to`) وفق اتفاقيات pointer‑ABI الخاصة بـ Norito. تستخدم مصادر Kotodama الامتداد `.ko`. يُنتج المترجم بايت كود IVM (`.to`) ويمكنه اختياريًا إرجاع مانيڤست.

المحتويات
- نظرة عامة وأهداف
- البنية المعجمية
- الأنواع والليترالات
- الإعلانات والوحدات
- حاوية العقد والبيانات الوصفية
- الدوال والمعاملات
- العبارات
- التعابير
- الدوال المدمجة وبناة pointer‑ABI
- المجموعات والخرائط
- التكرار الحتمي والحدود
- الأخطاء والتشخيصات
- مواءمة توليد الشيفرة إلى IVM
- ABI والرأس والمانيفست
- خارطة الطريق

## نظرة عامة وأهداف

- حتمية: يجب أن تنتج البرامج نتائج متطابقة عبر العتاد؛ لا أعداد عائمة ولا مصادر لا حتمية. كل تفاعل مع المضيف يتم عبر syscalls مع وسائط مُشفّرة بـ Norito.
- قابلية النقل: تستهدف بايت كود Iroha Virtual Machine ‏(IVM) وليس ISA ماديًا. ترميزات شبيهة بـ RISC‑V الظاهرة في المستودع هي تفاصيل تنفيذية لفك ترميز IVM ولا يجب أن تغيّر السلوك المرصود.
- قابلية التدقيق: دلالات صغيرة وصريحة؛ مواءمة واضحة من البنية إلى أوامر IVM وإلى syscalls للمضيف.
- الحدود: الحلقات على بيانات غير محدودة يجب أن تحمل حدودًا صريحة. تكرار الخرائط له قواعد صارمة لضمان الحتمية.

## البنية المعجمية

المسافات البيضاء والتعليقات
- المسافات البيضاء تفصل الرموز وإلا فهي غير ذات دلالة.
- تعليقات السطر تبدأ بـ `//` وتمتد حتى نهاية السطر.
- تعليقات الكتل `/* ... */` لا تتداخل.

المعرفات
- تبدأ بـ `[A-Za-z_]` ثم تتابع بـ `[A-Za-z0-9_]*`.
- حساسة لحالة الأحرف؛ `_` معرف صالح لكنه غير مستحسن.

الكلمات المحجوزة
- `seiyaku`, `hajimari`, `kotoage`, `kaizen`, `state`, `struct`, `fn`, `let`, `const`, `return`, `if`, `else`, `while`, `for`, `in`, `break`, `continue`, `true`, `false`, `permission`, `kotoba`.

العوامل وعلامات الترقيم
- حسابية: `+ - * / %`
- بتّية: `& | ^ ~`، والإزاحات `<< >>`
- مقارنة: `== != < <= > >=`
- منطقية: `&& || !`
- إسناد: `= += -= *= /= %= &= |= ^= <<= >>=`
- متفرقات: `: , ; . :: ->`
- الأقواس: `() [] {}`

الليترالات
- عدد صحيح: عشري (`123`)، سداسي (`0x2A`)، ثنائي (`0b1010`). جميع الأعداد الصحيحة هي 64‑بت موقعة وقت التنفيذ؛ الليترالات بلا لاحقة تُطبّع بالاستدلال أو كـ `int` افتراضيًا.
- نص: بين علامتي اقتباس مزدوجتين مع محارف هروب `\`؛ UTF‑8.
- منطقي: `true`, `false`.

## الأنواع والليترالات

أنواع قياسية
- `int`: 64‑بت مكملة لاثنين؛ الحساب يلتف بترديد 2^64 للجمع/الطرح/الضرب؛ القسمة لها صيغ موقعة/غير موقعة محددة في IVM؛ المترجم يختار العملية المناسبة للدلالة.
- `bool`: قيمة منطقية؛ تُخفض إلى `0`/`1`.
- `string`: سلسلة UTF‑8 غير قابلة للتغيير؛ تُعرض كـ Norito TLV عند تمريرها إلى syscalls؛ داخل VM تُستخدم شرائح بايت وطول.
- `bytes`: حمولة Norito خام؛ اسم بديل لنوع `Blob` في pointer‑ABI لمدخلات الهاش/التشفير/البرهان والـ overlays الدائمة.

أنواع مركبة
- `struct Name { field: Type, ... }` أنواع منتج معرفة من المستخدم. تُستخدم البنية الاستدعائية `Name(a, b, ...)` في التعابير. الوصول للحقل `obj.field` مدعوم ويُخفض داخليًا إلى حقول موضعية بنمط tuple. ABI لحالة durable على السلسلة مُشفّر بـ Norito؛ المترجم يُصدر overlays تعكس ترتيب الـ struct وتختبرها اختبارات حديثة (`crates/iroha_core/tests/kotodama_struct_overlay.rs`) لضمان ثبات التخطيط عبر الإصدارات.
- `Map<K, V>`: خريطة ترابطية حتمية؛ الدلالات تقيّد التكرار والطفرة أثناء التكرار (انظر أدناه).
- `Tuple (T1, T2, ...)`: نوع منتج مجهول بمتغيرات موضعية؛ يستخدم للعودة المتعددة.

أنواع pointer‑ABI الخاصة (جهة المضيف)
- `AccountId`, `AssetDefinitionId`, `Name`, `Json`, `NftId`, `Blob` وغيرها ليست أنواعًا من الدرجة الأولى في وقت التنفيذ. هي بناة تُنتج مؤشرات نوعية غير قابلة للتغيير داخل منطقة INPUT (أغلفة Norito TLV) ولا يمكن استخدامها إلا كوسائط syscalls أو نقلها بين المتغيرات دون طفرة.

استدلال النوع
- ربط `let` المحلي يستدل النوع من المُهيئ. معاملات الدوال يجب أن تكون محددة الأنواع صراحةً. أنواع الإرجاع قد تُستدل ما لم تكن ملتبسة.

## الإعلانات والوحدات

عناصر المستوى الأعلى
- العقود: `seiyaku Name { ... }` تحتوي دوالًا وحالةً وبنىً وبيانات وصفية.
- يسمح بعقود متعددة في الملف لكن يُستحسن تجنب ذلك؛ يُستخدم `seiyaku` رئيسي كمدخل افتراضي في المانيڤستات.
- إعلانات `struct` تُعرّف أنواعًا داخل العقد.

الرؤية
- `kotoage fn` يدل على نقطة دخول عامة؛ الرؤية تؤثر في أذونات الموزع وليس على توليد الشيفرة.

## حاوية العقد والبيانات الوصفية

الصياغة
```
seiyaku Name {
  meta {
    abi_version: 1,
    vector_length: 0,
    max_cycles: 0,
    features: ["zk", "simd"],
  }

  state int counter;

  hajimari() { counter = 0; }

  kotoage fn inc() { counter = counter + 1; }
}
```

الدلالات
- `meta { ... }` تتجاوز افتراضات المترجم لرأس IVM المُصدر: `abi_version`, `vector_length` (0 يعني غير مُحدد)، `max_cycles` (0 يعني افتراض المترجم)، `features` تُفعّل بتات الميزات في الرأس (تتبع ZK، إعلان المتجه). تُتجاهل الميزات غير المدعومة مع تحذير. عند غياب `meta {}` يصدر المترجم `abi_version = 1` ويستخدم افتراضات الخيارات لبقية حقول الرأس.
- `features: ["zk", "simd"]` (مرادفات: `"vector"`) تطلب صراحة بتات الرأس المقابلة. السلاسل غير المعروفة للميزات تُنتج الآن خطأ في المحلل بدل التجاهل.
- `state` تعلن متغيرات العقد. حاليًا يُخفضها المترجم إلى تخزين مؤقت لكل تشغيل (مُخصص عند دخول الدالة)؛ overlays الدائمة المدعومة بالمضيف وتتبع التعارضات ما زالت TODO. لقراءات/كتابات المضيف استخدم المساعدات الصريحة `state_get/state_set/state_del` ومساعدات الخرائط `get_or_insert_default`؛ هذه تمر عبر Norito TLV وتحافظ على الأسماء/ترتيب الحقول ثابتًا للتخزين المستقبلي.
- معرفات `state` محجوزة؛ تظليل اسم `state` في المعاملات أو `let` مرفوض (`E_STATE_SHADOWED`).
- قيم خرائط الحالة ليست من الدرجة الأولى: استخدم معرف الحالة مباشرةً لعمليات الخرائط والتكرار. ربط خرائط الحالة أو تمريرها إلى دوال المستخدم مرفوض (`E_STATE_MAP_ALIAS`).
- خرائط الحالة الدائمة تدعم حاليًا مفاتيح `int` وأنواع pointer‑ABI فقط؛ الأنواع الأخرى للمفاتيح تُرفض في وقت الترجمة.
- حقول الحالة الدائمة يجب أن تكون `int` أو `bool` أو `Json` أو `Blob`/`bytes` أو أنواع pointer‑ABI (بما فيها structs/tuples المركبة من هذه الحقول)؛ `string` غير مدعوم للحالة الدائمة.

## تصريحات المشغلات

تصريحات المشغلات تُلحق بيانات الجدولة الوصفية بمانيفست نقاط الدخول وتُسجّل تلقائيًا عند تفعيل
مثيل العقد (وتُزال عند التعطيل). تُحلّل داخل كتلة `seiyaku`.

الصياغة
```
register_trigger wake {
  call run;
  on time pre_commit;
  repeats 2;
  metadata { tag: "alpha"; count: 1; enabled: true; }
}
```

ملاحظات
- يجب أن يشير `call` إلى `kotoage fn` عامة داخل نفس العقد؛ يمكن تسجيل `namespace::entrypoint`
  اختياريًا في المانيفست لكن ردود الاستدعاء بين العقود مرفوضة حاليًا (محلية فقط).
- المرشحات المدعومة: `time pre_commit` و`time schedule(start_ms, period_ms?)`، بالإضافة إلى
  `execute trigger <name>` لمشغلات الاستدعاء. مرشحات البيانات/الأنابيب غير مدعومة بعد.
- يجب أن تكون قيم metadata ليتيرالات JSON (`string`, `number`, `bool`, `null`) أو `json!(...)`.
- مفاتيح metadata التي يحقنها الـ runtime: `contract_namespace`, `contract_id`,
  `contract_entrypoint`, `contract_code_hash`, `contract_trigger_id`.

## الدوال والمعاملات

الصياغة
- التصريح: `fn name(param1: Type, param2: Type, ...) -> Ret { ... }`
- العامة: `kotoage fn name(...) { ... }`
- المُهيئ: `hajimari() { ... }` (يُستدعى عند النشر بواسطة runtime وليس من VM نفسها).
- خطاف الترقية: `kaizen(args...) permission(Role) { ... }`.

المعاملات والإرجاع
- تُمرر الوسائط في المسجلات `r10..r22` كقيم أو مؤشرات INPUT (Norito TLV) حسب ABI؛ الوسائط الإضافية تُسرب إلى المكدس.
- الدوال تعيد صفرًا أو قيمة قياسية واحدة أو tuple. قيمة الإرجاع الرئيسية في `r10` للقياسي؛ الـ tuples تُجسّد في المكدس/OUTPUT وفق العرف.

## العبارات

- ربط المتغيرات: `let x = expr;`, `let mut x = expr;` (التحوير يُفحص وقت الترجمة؛ الطفرة وقت التنفيذ مسموحة للمحليات فقط).
- الإسناد: `x = expr;` والأشكال المركبة `x += 1;` إلخ. يجب أن تكون الأهداف متغيرات أو فهارس خرائط؛ حقول tuple/struct غير قابلة للتغيير.
- التحكم: `if (cond) { ... } else { ... }`, `while (cond) { ... }`, و `for (init; cond; step) { ... }` بنمط C.
  - مُهيئات وخطوات `for` يجب أن تكون `let name = expr` بسيطة أو عبارات تعبيرية؛ تفكيك معقد مرفوض (`E0005`, `E0006`).
  - نطاق `for`: الروابط من فقرة init مرئية داخل الحلقة وبعدها؛ الروابط المُنشأة في الجسم أو الخطوة لا تخرج من الحلقة.
- المساواة (`==`, `!=`) مدعومة لـ `int`, `bool`, `string`, وقيم pointer‑ABI القياسية (مثل `AccountId`, `Name`, `Blob`/`bytes`, `Json`)؛ لا يمكن مقارنة tuples أو structs أو maps.
- حلقة الخريطة: `for (k, v) in map { ... }` (حتمية؛ انظر أدناه).
- تدفق: `return expr;`, `break;`, `continue;`.
- الاستدعاء: `name(args...);` أو `call name(args...);` (كلاهما مقبول؛ المترجم يطبّعها إلى عبارات استدعاء).
- التأكيدات: `assert(cond);`, `assert_eq(a, b);` تُترجم إلى `ASSERT*` في IVM في البنى غير‑ZK أو إلى قيود ZK في وضع ZK.

## التعابير

الأسبقية (عالية → منخفضة)
1. عضو/فهرسة: `a.b`, `a[b]`
2. أحادية: `! ~ -`
3. ضرب/قسمة/باقي: `* / %`
4. جمع/طرح: `+ -`
5. إزاحات: `<< >>`
6. علاقية: `< <= > >=`
7. مساواة: `== !=`
8. AND/XOR/OR بتّي: `& ^ |`
9. AND/OR منطقي: `&& ||`
10. ثلاثي: `cond ? a : b`

الاستدعاءات والـ tuples
- الاستدعاءات تستخدم وسائط موضعية: `f(a, b, c)`.
- literal للـ tuple: `(a, b, c)` والتفكيك: `let (x, y) = pair;`.
- تفكيك الـ tuple يتطلب أنواع tuple/struct بأريّة مطابقة؛ عدم التطابق يُرفض.

السلاسل والبايتات
- السلاسل UTF‑8؛ الدوال التي تتطلب بايتات خام تقبل مؤشرات `Blob` عبر البناة (انظر Builtins).

## الدوال المدمجة وبناة pointer‑ABI

بناة المؤشرات (تُصدر Norito TLV في INPUT وتعيد مؤشرًا مُنوعًا)
- `account_id(string) -> AccountId*`
- `asset_definition(string) -> AssetDefinitionId*`
- `asset_id(string) -> AssetId*`
- `domain(string) | domain_id(string) -> DomainId*`
- `name(string) -> Name*`
- `json(string) -> Json*`
- `nft_id(string) -> NftId*`
- `blob(bytes|string) -> Blob*`
- `norito_bytes(bytes|string) -> NoritoBytes*`
- `dataspace_id(string|0xhex) -> DataSpaceId*`
- `axt_descriptor(string|0xhex) -> AxtDescriptor*`
- `asset_handle(string|0xhex) -> AssetHandle*`
- `proof_blob(string|0xhex) -> ProofBlob*`

توفر ماكروهات الـ Prelude أ aliases أقصر وتحققًا داخليًا لهذه البناة:
- `account!("ih58...")`, `account_id!("ih58...")`
- `asset_definition!("rose#wonderland")`, `asset_id!("rose#wonderland")`
- `domain!("wonderland")`, `domain_id!("wonderland")`
- `name!("example")`
- `json!("{\"hello\":\"world\"}")` أو ليترالات مُهيكلة مثل `json!{ hello: "world" }`
- `nft_id!("dragon#demo")`, `blob!("bytes")`, `norito_bytes!("...")`

تُوسع الماكروهات إلى البناة أعلاه وترفض الليترالات غير الصحيحة وقت الترجمة.

حالة التنفيذ
- مُنفذ: البناة أعلاه تقبل وسائط سلاسل حرفية وتُخفض إلى أغلفة Norito TLV مُنوّعة في منطقة INPUT. تعيد مؤشرات مُنوّعة غير قابلة للتغيير قابلة للاستخدام كوسائط syscalls. تعابير السلاسل غير الحرفية مرفوضة؛ استخدم `Blob`/`bytes` للمدخلات الديناميكية. `blob`/`norito_bytes` تقبل أيضًا قيم `bytes` وقت التشغيل دون ماكروهات.
- صيغ موسعة:
  - `json(Blob[NoritoBytes]) -> Json*` عبر syscall `JSON_DECODE`.
  - `name(Blob[NoritoBytes]) -> Name*` عبر syscall `NAME_DECODE`.
  - فك ترميز مؤشرات من Blob/NoritoBytes: أي مُنشئ مؤشر (بما في ذلك أنواع AXT) يقبل حمولة `Blob`/`NoritoBytes` ويُخفض إلى `POINTER_FROM_NORITO` مع معرّف النوع المتوقع.
  - تمرير مباشر لأشكال المؤشر: `name(Name) -> Name*`, `blob(Blob) -> Blob*`, `norito_bytes(Blob) -> Blob*`.
  - دعم سكر الطرائق: `s.name()`, `s.json()`, `b.blob()`, `b.norito_bytes()`.

الدوال المدمجة للمضيف/syscall (تُحوّل إلى SCALL؛ الأرقام الدقيقة في ivm.md)
- `mint_asset(AccountId*, AssetDefinitionId*, numeric)`
- `burn_asset(AccountId*, AssetDefinitionId*, numeric)`
- `transfer_asset(AccountId*, AccountId*, AssetDefinitionId*, numeric)`
- `set_account_detail(AccountId*, Name*, Json*)`
- `nft_mint_asset(NftId*, AccountId*)`
- `nft_transfer_asset(AccountId*, NftId*, AccountId*)`
- `nft_set_metadata(NftId*, Json*)`
- `nft_burn_asset(NftId*)`
- `authority() -> AccountId*`
- `register_domain(DomainId*)`
- `unregister_domain(DomainId*)`
- `transfer_domain(AccountId*, DomainId*, AccountId*)`
- `vrf_verify(Blob, Blob, Blob, int variant) -> Blob`
- `vrf_verify_batch(Blob) -> Blob`
- `axt_begin(AxtDescriptor*)`
- `axt_touch(DataSpaceId*, Blob[NoritoBytes]? manifest)`
- `verify_ds_proof(DataSpaceId*, ProofBlob?)`
- `use_asset_handle(AssetHandle*, Blob[NoritoBytes], ProofBlob?)`
- `axt_commit()`
- `contains(Map<K,V>, K) -> bool`

الدوال المدمجة المساندة
- `info(string|int)`: يُصدر حدثًا/رسالة مُهيكلة عبر OUTPUT.
- `hash(blob) -> Blob*`: يعيد هاشًا مُشفّرًا بـ Norito على شكل Blob.
- `build_submit_ballot_inline(election_id, ciphertext, nullifier32, backend, proof, vk) -> Blob*` و `build_unshield_inline(asset, to, amount, inputs32, backend, proof, vk) -> Blob*`: بُناة ISI داخلية؛ يجب أن تكون كل الوسائط ليترالات وقت الترجمة (ليترالات نصية أو بُناة مؤشرات من ليترالات). يجب أن يكون `nullifier32` و`inputs32` بطول 32 بايت تمامًا (سلسلة خام أو hex `0x`)، ويجب أن يكون `amount` غير سالب.
- `schema_info(Name*) -> Json* { "id": "<hex>", "version": N }`
- `pointer_to_norito(ptr) -> NoritoBytes*`: يلف TLV pointer‑ABI الموجود إلى NoritoBytes للتخزين أو النقل.
- `isqrt(int) -> int`: الجذر التربيعي الصحيح (`floor(sqrt(x))`) مُنفذ كأمر IVM.
- `min(int, int) -> int`, `max(int, int) -> int`, `abs(int) -> int`, `div_ceil(int, int) -> int`, `gcd(int, int) -> int`, `mean(int, int) -> int` — مساعدات حسابية مدمجة مدعومة بأوامر IVM الأصلية (القسمة بالتقريب للأعلى تُسبب trap عند القسمة على صفر).

ملاحظات
- الدوال المدمجة شيمات رقيقة؛ المترجم يُخفضها إلى نقل سجلات و `SCALL`.
- بناة المؤشرات نقية: تضمن VM أن Norito TLV في INPUT غير قابل للتغيير طوال مدة الاستدعاء.
 - يمكن استخدام structs ذات الحقول pointer‑ABI (مثل `DomainId`, `AccountId`) لتجميع وسائط syscall بشكل مريح. يطابق المترجم `obj.field` مع السجل/القيمة الصحيحة دون تخصيصات إضافية.

## المجموعات والخرائط

النوع: `Map<K, V>`
- الخرائط داخل الذاكرة (مخصصة عبر `Map::new()` أو مُمررة كمعاملات) تخزن زوج مفتاح/قيمة واحدًا؛ يجب أن تكون المفاتيح والقيم من أنواع بحجم كلمة: `int`, `bool`, `string`, `Blob`, `bytes`, `Json` أو أنواع مؤشرات (مثل `AccountId`, `Name`).
- خرائط الحالة الدائمة (`state Map<...>`) تستخدم مفاتيح/قيم مُشفّرة بـ Norito. المفاتيح المدعومة: `int` أو أنواع مؤشرات. القيم المدعومة: `int`, `bool`, `Json`, `Blob`/`bytes` أو أنواع مؤشرات.
- `Map::new()` يخصص ويصفّر الإدخال الوحيد في الذاكرة (المفتاح/القيمة = 0)؛ للخرائط غير `Map<int,int>` وفّر توصيف نوع صريح أو نوع إرجاع.
- خرائط الحالة ليست قيماً من الدرجة الأولى: لا يمكنك إعادة إسنادها (مثل `M = Map::new()`); حدّث الإدخالات عبر الفهرسة (`M[key] = value`).
- العمليات:
  - الفهرسة: `map[key]` جلب/تعيين القيمة (التعيين عبر syscall المضيف؛ انظر مواءمة API وقت التنفيذ).
  - التحقق من الوجود: `contains(map, key) -> bool` (مساعد مخفّض؛ قد يكون syscall مضمّنًا).
  - التكرار: `for (k, v) in map { ... }` بترتيب حتمي وقواعد طفرة.

قواعد التكرار الحتمي
- مجموعة التكرار هي لقطة المفاتيح عند دخول الحلقة.
- الترتيب تصاعدي صارم وفق الترتيب المعجمي البايتّي لمفاتيح Norito المشفّرة.
- التعديلات الهيكلية (إدراج/إزالة/تفريغ) للخرائط المُكررة أثناء الحلقة تسبب trap حتميًا `E_ITER_MUTATION`.
- مطلوب حدّ: إما حد أقصى مُعلن (`@max_len`) على الخريطة، أو سمة صريحة `#[bounded(n)]`، أو حد صريح عبر `.take(n)`/`.range(..)`؛ وإلا يصدر المترجم `E_UNBOUNDED_ITERATION`.

مساعدات الحدود
- `#[bounded(n)]`: سمة اختيارية على تعبير الخريطة، مثل `for (k, v) in my_map #[bounded(2)] { ... }`.
- `.take(n)`: تكرار أول `n` إدخالات من البداية.
- `.range(start, end)`: تكرار الإدخالات في المجال نصف المفتوح `[start, end)`. الدلالات تعادل `start` و `n = end - start`.

ملاحظات حول الحدود الديناميكية
- الحدود الحرفية: `n`, `start`, و`end` كليترالات عددية مدعومة بالكامل وتُترجم إلى عدد ثابت من التكرارات.
- الحدود غير الحرفية: عندما تُفعّل ميزة `kotodama_dynamic_bounds` في crate `ivm`، يقبل المترجم تعابير ديناميكية لـ `n` و`start` و`end` ويضيف تأكيدات وقت التشغيل للسلامة (غير سالب، `end >= start`). يولّد lowering حتى K تكرارات محروسة بـ `if (i < n)` لتجنّب تنفيذات زائدة للجسم (K الافتراضي = 2). يمكنك ضبط K برمجيًا عبر `CompilerOptions { dynamic_iter_cap, .. }`.
- شغّل `koto_lint` لمراجعة تحذيرات lint في Kotodama قبل الترجمة؛ المترجم الرئيسي يتابع دائمًا بعد التحليل والنوع.
- أكواد الخطأ موثقة في [Kotodama Compiler Error Codes](./kotodama_error_codes.md)؛ استخدم `koto_compile --explain <code>` للشروح السريعة.

## الأخطاء والتشخيصات

تشخيصات وقت الترجمة (أمثلة)
- `E_UNBOUNDED_ITERATION`: حلقة على خريطة بلا حد.
- `E_MUT_DURING_ITER`: طفرة هيكلية للخريطة المُكررة داخل جسم الحلقة.
- `E_STATE_SHADOWED`: الروابط المحلية لا يمكنها تظليل إعلانات `state`.
- `E_BREAK_OUTSIDE_LOOP`: استخدام `break` خارج حلقة.
- `E_CONTINUE_OUTSIDE_LOOP`: استخدام `continue` خارج حلقة.
- `E0005`: مُهيئ for أعقد مما هو مدعوم.
- `E0006`: بند خطوة for أعقد مما هو مدعوم.
- `E_BAD_POINTER_USE`: استخدام نتيجة مُنشئ pointer‑ABI حيث يلزم نوع من الدرجة الأولى.
- `E_UNRESOLVED_NAME`, `E_TYPE_MISMATCH`, `E_ARITY_MISMATCH`, `E_DUP_SYMBOL`.
- الأدوات: `koto_compile` يشغّل lint قبل إصدار البايت كود؛ استخدم `--no-lint` للتجاوز أو `--deny-lint-warnings` لفشل البناء عند أي إخراج lint.

أخطاء VM وقت التشغيل (مختارة؛ القائمة الكاملة في ivm.md)
- `E_NORITO_INVALID`, `E_OOB`, `E_UNALIGNED`, `E_SCALL_UNKNOWN`, `E_ASSERT`, `E_ASSERT_EQ`, `E_ITER_MUTATION`.

رسائل الأخطاء
- التشخيصات تحمل `msg_id` ثابتة تُطابق إدخالات جداول الترجمة `kotoba {}` عند توفرها.

## مواءمة توليد الشيفرة إلى IVM

خط الأنابيب
1. Lexer/Parser ينتجان AST.
2. التحليل الدلالي يحل الأسماء ويتحقق من الأنواع ويملأ جداول الرموز.
3. خفض IR إلى صيغة بسيطة شبيهة بـ SSA.
4. تخصيص المسجلات إلى IVM GPRs (`r10+` للوسائط/العودة حسب الاتفاق)؛ وإراقة إلى المكدس.
5. إصدار البايت كود: خليط من ترميزات IVM الأصلية وترميزات متوافقة مع RV حيث يلزم؛ رأس الميتاداتا مع `abi_version` وfeatures وطول المتجه و`max_cycles`.

أبرز مواءمات
- الحساب والمنطق يخرجان إلى عمليات ALU في IVM.
- التفرع والتحكم يخرجان إلى فروع شرطية وقفزات؛ المترجم يستخدم الأشكال المضغوطة عندما تكون مربحة.
- ذاكرة المحليات تُسرّب إلى مكدس VM؛ يفرض المحاذاة.
- الدوال المدمجة تُخفض إلى نقل سجلات و `SCALL` برقم 8‑بت.
- بناة المؤشرات تضع Norito TLV في منطقة INPUT وتُنتج عناوينها.
- التأكيدات تُخرّج إلى `ASSERT`/`ASSERT_EQ` التي تُطلق trap في التنفيذ غير‑ZK وتُصدر قيودًا في بناء ZK.

قيود الحتمية
- لا أعداد عائمة؛ لا syscalls غير حتمية.
- تسريع SIMD/GPU غير مرئي للبايت كود ويجب أن يكون مطابقًا بت‑بت؛ المترجم لا يُصدر عمليات خاصة بالعتاد.

## ABI والرأس والمانيفست

حقول رأس IVM التي يضبطها المترجم
- `version`: نسخة صيغة بايت كود IVM (major.minor).
- `abi_version`: نسخة جدول syscalls ومخطط pointer‑ABI.
- `feature_bits`: أعلام الميزات (مثل `ZK`, `VECTOR`).
- `vector_len`: طول المتجه المنطقي (0 → غير مُحدد).
- `max_cycles`: حد القبول ولمّاح padding لـ ZK.

المانيفست (ملف جانبي اختياري)
- `code_hash`, `abi_hash`, وبيانات `meta {}` وإصدار المترجم وتلميحات البناء لإعادة الإنتاج.

## خارطة الطريق

- **KD-231 (أبريل 2026):** إضافة تحليل نطاق وقت الترجمة لحدود التكرار كي تكشف الحلقات مجموعات وصول محدودة للمُجَدول.
- **KD-235 (مايو 2026):** تقديم نوع قياسي `bytes` من الدرجة الأولى منفصل عن `string` لبناة المؤشرات ووضوح ABI.
- **KD-242 (يونيو 2026):** توسيع مجموعة أوامر builtins (hash / التحقق من التوقيع) خلف أعلام الميزات مع مسارات احتياطية حتمية.
- **KD-247 (يونيو 2026):** تثبيت `msg_id` للأخطاء والحفاظ على الربط في جداول `kotoba {}` لتشخيصات مُعربة.
### إصدار المانيفست

- يمكن لـ API مترجم Kotodama إرجاع `ContractManifest` بجانب `.to` المُترجم عبر `ivm::kotodama::compiler::Compiler::compile_source_with_manifest`.
- الحقول:
  - `code_hash`: تجزئة بايتات الشيفرة (باستثناء رأس IVM والليترالات) التي يحسبها المترجم لربط الأثر.
  - `abi_hash`: بصمة مستقرة لسطح syscalls المسموح لنسخة `abi_version` للبرنامج (انظر `ivm.md` و `ivm::syscalls::compute_abi_hash`).
- الحقول الاختيارية `compiler_fingerprint` و`features_bitmap` محجوزة للأدوات.
- `entrypoints`: قائمة مرتبة لنقاط الدخول المصدّرة (عامة، `hajimari`، `kaizen`) بما في ذلك سلاسل `permission(...)` المطلوبة وتلميحات مفاتيح القراءة/الكتابة بأفضل جهد من المترجم لكي تتمكن سياسة القبول والمُجَدولات من الاستدلال على الوصول المتوقع لـ WSV.
- المانيفست مخصص لفحوصات القبول وللسجلات؛ انظر `docs/source/new_pipeline.md` لدورة الحياة.

</div>

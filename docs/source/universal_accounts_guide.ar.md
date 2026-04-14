<!-- Auto-generated stub for Arabic (ar) translation. Replace this content with the full translation. -->

---
lang: ar
direction: rtl
source: docs/source/universal_accounts_guide.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 09a308ecbf07f0293add7f35cf4f1a50b5e6d3630b8b37a8f0f45a7cf82d3924
source_last_modified: "2026-03-30T18:22:55.987822+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# دليل الحساب العالمي

يلخص هذا الدليل متطلبات نشر UAID (معرف الحساب العالمي) من
خريطة طريق Nexus وتجميعها في دليل تفصيلي يركز على المشغل + SDK.
ويغطي اشتقاق UAID، وفحص المحفظة/البيان، وقوالب المنظم،
والأدلة التي يجب أن تصاحب كل بيان دليل مساحة تطبيق iroha
Publish` run (roadmap reference: `roadmap.md:2209`).

## 1. مرجع سريع لـ UAID- معرفات UAID هي `uaid:<hex>` حرفية حيث `<hex>` عبارة عن ملخص Blake2b-256 الذي
  تم ضبط LSB على `1`. النوع الكنسي يعيش فيه
  `crates/iroha_data_model/src/nexus/manifest.rs::UniversalAccountId`.
- تحمل سجلات الحساب (`Account` و`AccountDetails`) الآن `uaid` اختياريًا
  الحقل حتى تتمكن التطبيقات من معرفة المعرف دون تجزئة مخصصة.
- يمكن لسياسات معرف الوظيفة المخفية ربط المدخلات الطبيعية التعسفية
  (أرقام الهواتف ورسائل البريد الإلكتروني وأرقام الحسابات وسلاسل الشركاء) إلى معرفات `opaque:`
  ضمن مساحة اسم UAID. القطع الموجودة على السلسلة هي `IdentifierPolicy`،
  `IdentifierClaimRecord`، والفهرس `opaque_id -> uaid`.
- يحتفظ دليل الفضاء بخريطة `World::uaid_dataspaces` التي تربط كل UAID
  إلى حسابات مساحة البيانات المشار إليها بواسطة البيانات النشطة. يعيد Torii استخدام ذلك
  خريطة لواجهات برمجة التطبيقات `/portfolio` و`/uaids/*`.
- ينشر `POST /v1/accounts/onboard` بيان دليل الفضاء الافتراضي لـ
  مساحة البيانات العامة في حالة عدم وجودها، لذلك يتم ربط UAID على الفور.
  يجب أن تحمل سلطات الإعداد الرقم `CanPublishSpaceDirectoryManifest{dataspace=0}`.
- تعرض كافة مجموعات تطوير البرامج (SDK) مساعدين لتحديد معايير UAID الحرفية (على سبيل المثال،
  `UaidLiteral` في Android SDK). يقبل المساعدون الملخصات الخام ذات 64 سداسيًا
  (LSB=1) أو `uaid:<hex>` وأعد استخدام نفس برامج الترميز Norito حتى يتم
  لا يمكن للملخص أن ينجرف عبر اللغات.

## 1.1 سياسات المعرف المخفي

تعد معرفات UAIDs الآن بمثابة مرساة لطبقة الهوية الثانية:- يحدد `IdentifierPolicyId` (`<kind>#<business_rule>`) العالمي
  مساحة الاسم، وبيانات تعريف الالتزام العام، ومفتاح التحقق من المحلل، و
  وضع تطبيع الإدخال الأساسي (`Exact`، `LowercaseTrimmed`،
  `PhoneE164`، أو `EmailAddress`، أو `AccountNumber`).
- تربط المطالبة معرف `opaque:` مشتق واحد بمعرف UAID واحد وواحد بالضبط
  `AccountId` الكنسي بموجب هذه السياسة، لكن السلسلة تقبل فقط
  المطالبة عندما تكون مصحوبة بـ `IdentifierResolutionReceipt` موقعة.
- تظل الدقة هي التدفق `resolve -> transfer`. Torii يحل مشكلة التعتيم
  التعامل مع وإرجاع `AccountId` الأساسي ؛ التحويلات لا تزال تستهدف
  الحساب الأساسي، وليس `uaid:` أو `opaque:` الحرفي مباشرةً.
- يمكن للسياسات الآن نشر معلمات تشفير الإدخال BFV من خلال
  `PolicyCommitment.public_parameters`. عند وجوده، يقوم Torii بالإعلان عنها
  `GET /v1/identifier-policies`، ويمكن للعملاء إرسال مدخلات ملفوفة بـ BFV
  بدلاً من النص العادي. تقوم السياسات المبرمجة بتغليف معلمات BFV في ملف
  حزمة `BfvProgrammedPublicParameters` الأساسية التي تنشر أيضًا ملف
  `ram_fhe_profile` العام؛ تتم ترقية حمولات BFV الخام القديمة إلى ذلك
  الحزمة الأساسية عند إعادة بناء الالتزام.
- تمر مسارات المعرف بنفس رمز الوصول Torii وحد السعر
  الشيكات كنقاط النهاية الأخرى التي تواجه التطبيق. فهي ليست تجاوزا حول وضعها الطبيعي
  سياسة واجهة برمجة التطبيقات.

## 1.2 المصطلحات

تقسيم التسمية مقصود:- `ram_lfe` هو تجريد الوظيفة المخفية الخارجية. ويغطي السياسة
  التسجيل والالتزامات والبيانات الوصفية العامة وإيصالات التنفيذ و
  وضع التحقق.
- `BFV` هو نظام التشفير المتماثل Brakerski/Fan-Vercauteren الذي يستخدمه
  بعض الواجهات الخلفية `ram_lfe` لتقييم المدخلات المشفرة.
- `ram_fhe_profile` عبارة عن بيانات تعريف خاصة بـ BFV، وليس اسمًا ثانيًا للكل
  ميزة. وهو يصف آلة تنفيذ BFV المبرمجة التي تقوم بحفظ و
  يجب أن تستهدف أدوات التحقق عندما تستخدم السياسة الواجهة الخلفية المبرمجة.

بعبارات ملموسة:

- `RamLfeProgramPolicy` و`RamLfeExecutionReceipt` هما نوعان من طبقات LFE.
- `BfvParameters`، `BfvCiphertext`، `BfvProgrammedPublicParameters`، و
  `BfvRamProgramProfile` هي أنواع طبقات FHE.
- `HiddenRamFheProgram` و`HiddenRamFheInstruction` هي أسماء داخلية لـ
  برنامج BFV المخفي الذي يتم تنفيذه بواسطة الواجهة الخلفية المبرمجة. يبقون على
  جانب FHE لأنها تصف آلية التنفيذ المشفرة بدلاً من
  السياسة الخارجية أو تجريد الاستلام.

## 1.3 هوية الحساب مقابل الأسماء المستعارة

لا يؤدي طرح الحساب العام إلى تغيير نموذج هوية الحساب الأساسي:- يظل `AccountId` هو موضوع الحساب الأساسي بدون مجال.
- قيم `AccountAlias` هي روابط SNS منفصلة أعلى هذا الموضوع. أ
  الاسم المستعار المؤهل للمجال مثل `merchant@hbl.sbp` والاسم المستعار لجذر مساحة البيانات
  مثل `merchant@sbp` يمكن حلهما إلى نفس `AccountId` المتعارف عليه.
- تسجيل الحساب الكنسي دائمًا هو `Account::new(AccountId)` /
  `NewAccount::new(AccountId)`; لا يوجد المجال المؤهل أو المجال المادي
  مسار التسجيل.
- ملكية المجال، وأذونات الاسم المستعار، والسلوكيات الأخرى على نطاق المجال مباشرة
  في حالتها وواجهات برمجة التطبيقات الخاصة بها بدلاً من هوية الحساب نفسها.
- يتبع البحث العام عن الحساب هذا الانقسام: تظل استعلامات الاسم المستعار عامة، بينما
  تظل هوية الحساب الأساسية `AccountId` خالصة.

قاعدة التنفيذ للمشغلين وحزم SDK والاختبارات: ابدأ من الأساسي
`AccountId`، ثم أضف عقود إيجار الاسم المستعار وأذونات مساحة البيانات/المجال وأي شيء آخر
الدولة المملوكة للمجال بشكل منفصل. لا تقم بتجميع حساب مزيف مشتق من اسم مستعار
أو توقع أي حقل مجال مرتبط في سجلات الحساب لمجرد اسم مستعار أو
الطريق يحمل قطعة المجال.

مسارات Torii الحالية:| الطريق | الغرض |
|-------|---------|
| `GET /v1/ram-lfe/program-policies` | يسرد سياسات برنامج RAM-LFE النشطة وغير النشطة بالإضافة إلى بيانات تعريف التنفيذ العامة الخاصة بها، بما في ذلك معلمات BFV `input_encryption` الاختيارية والواجهة الخلفية المبرمجة `ram_fhe_profile`. |
| `POST /v1/ram-lfe/programs/{program_id}/execute` | يقبل بالضبط واحدًا من `{ input_hex }` أو `{ encrypted_input }` ويعيد `RamLfeExecutionReceipt` بالإضافة إلى `{ output_hex, output_hash, receipt_hash }` للبرنامج المحدد. يُصدر وقت التشغيل Torii الحالي إيصالات للواجهة الخلفية BFV المبرمجة. |
| `POST /v1/ram-lfe/receipts/verify` | يتحقق بدون حالة من صحة `RamLfeExecutionReceipt` مقابل سياسة البرنامج المنشورة على السلسلة ويتحقق بشكل اختياري من أن `output_hex` المقدم من المتصل يطابق الإيصال `output_hash`. |
| `GET /v1/identifier-policies` | يسرد مساحات أسماء سياسة الوظائف المخفية النشطة وغير النشطة بالإضافة إلى بيانات التعريف العامة الخاصة بها، بما في ذلك معلمات BFV `input_encryption` الاختيارية، ووضع `normalization` المطلوب للإدخال المشفر من جانب العميل، و`ram_fhe_profile` لسياسات BFV المبرمجة. |
| `POST /v1/accounts/{account_id}/identifiers/claim-receipt` | يقبل بالضبط واحدًا من `{ input }` أو `{ encrypted_input }`. يتم تسوية النص العادي `input` من جانب الخادم؛ يجب أن تتم تسوية BFV `encrypted_input` بالفعل وفقًا لوضع السياسة المنشور. تقوم نقطة النهاية بعد ذلك باشتقاق المقبض `opaque:` وإرجاع إيصال موقع يمكن لـ `ClaimIdentifier` إرساله على السلسلة، بما في ذلك كل من `signature_payload_hex` الخام و`signature_payload`. || `POST /v1/identifiers/resolve` | يقبل بالضبط واحدًا من `{ input }` أو `{ encrypted_input }`. يتم تسوية النص العادي `input` من جانب الخادم؛ يجب أن تتم تسوية BFV `encrypted_input` بالفعل وفقًا لوضع السياسة المنشور. تقوم نقطة النهاية بتحليل المعرف إلى `{ opaque_id, receipt_hash, uaid, account_id, signature }` عند وجود مطالبة نشطة، وتقوم أيضًا بإرجاع الحمولة الموقعة الأساسية كـ `{ signature_payload_hex, signature_payload }`. |
| `GET /v1/identifiers/receipts/{receipt_hash}` | يبحث عن `IdentifierClaimRecord` المستمر المرتبط بتجزئة إيصال حتمية حتى يتمكن المشغلون ومجموعات SDK من تدقيق ملكية المطالبة أو تشخيص حالات فشل إعادة التشغيل/عدم التطابق دون فحص فهرس المعرف الكامل. |

تم تكوين وقت تشغيل التنفيذ قيد التشغيل لـ Torii ضمن
`torii.ram_lfe.programs[*]`، بمفتاح `program_id`. طرق المعرف الآن
أعد استخدام نفس وقت تشغيل RAM-LFE بدلاً من `identifier_resolver` المنفصل
سطح التكوين

دعم SDK الحالي:- `normalizeIdentifierInput(value, normalization)` يطابق الصدأ
  المحددات الأساسية لـ `exact`، `lowercase_trimmed`، `phone_e164`،
  `email_address` و`account_number`.
- يسرد `ToriiClient.listIdentifierPolicies()` بيانات تعريف السياسة، بما في ذلك BFV
  البيانات التعريفية لتشفير الإدخال عندما تنشرها السياسة، بالإضافة إلى البيانات التعريفية التي تم فك تشفيرها
  كائن معلمة BFV عبر `input_encryption_public_parameters_decoded`.
  تعرض السياسات المبرمجة أيضًا `ram_fhe_profile` الذي تم فك تشفيره. هذا المجال
  نطاق BFV عن عمد: فهو يتيح للمحافظ التحقق من السجل المتوقع
  العد وعدد الممرات ووضع التحديد الأساسي والحد الأدنى لمعامل النص المشفر لـ
  الواجهة الخلفية FHE المبرمجة قبل تشفير المدخلات من جانب العميل.
- `getIdentifierBfvPublicParameters(policy)` و
  مساعدة `buildIdentifierRequestForPolicy(policy, { input | encryptedInput })`
  يستهلك المتصلون بـ JS بيانات تعريف BFV المنشورة وينشئون طلبًا مدركًا للسياسة
  الهيئات دون إعادة تنفيذ قواعد معرف السياسة والتطبيع.
- `encryptIdentifierInputForPolicy(policy, input, { seedHex? })` و
  `buildIdentifierRequestForPolicy(policy, { input, encrypt: true })` الآن دعونا
  تقوم محافظ JS بإنشاء مظروف النص المشفر BFV Norito الكامل محليًا من
  معلمات السياسة المنشورة بدلاً من الشحن السداسي للنص المشفر المُعد مسبقًا.
-`ToriiClient.resolveIdentifier({ policyId, input | encryptedInput })`
  يحل معرفًا مخفيًا ويعيد حمولة الإيصال الموقعة،
  بما في ذلك `receipt_hash`، و`signature_payload_hex`، و
  `signature_payload`.
- `ToriiClient.issueIdentifierClaimReceipt(accountId, { PolicyId, input |
  encryptedInput })` issues the signed receipt needed by `ClaimIdentifier`.
- `verifyIdentifierResolutionReceipt(receipt, policy)` يتحقق من الإرجاع
  إيصال مقابل مفتاح حل السياسة من جانب العميل، ويقوم `ToriiClient.getIdentifierClaimByReceiptHash(receiptHash)` بجلب ملف
  سجل المطالبة المستمرة لتدفقات التدقيق/تصحيح الأخطاء اللاحقة.
- يعرض `IrohaSwift.ToriiClient` الآن `listIdentifierPolicies()`،
  `resolveIdentifier(policyId:input:encryptedInputHex:)`,
  `issueIdentifierClaimReceipt(accountId:policyId:input:encryptedInputHex:)`,
  و`getIdentifierClaimByReceiptHash(_)`، بالإضافة إلى
  `ToriiIdentifierNormalization` لنفس الهاتف/البريد الإلكتروني/رقم الحساب
  أوضاع تحديد العنوان القانوني.
- `ToriiIdentifierLookupRequest` و
  `ToriiIdentifierPolicySummary.plaintextRequest(...)` /
  يوفر مساعدو `.encryptedRequest(...)` سطح طلب Swift المكتوب لـ
  حل مكالمات واستلام المطالبة، ويمكن لسياسات Swift الآن استخلاص BFV
  النص المشفر محليًا عبر `encryptInput(...)` / `encryptedRequest(input:...)`.
- `ToriiIdentifierResolutionReceipt.verifySignature(using:)` يؤكد ذلك
  تتطابق حقول الإيصال ذات المستوى الأعلى مع الحمولة الموقعة وتتحقق من
  توقيع المحلل من جانب العميل قبل الإرسال.
- يتم الآن الكشف عن `HttpClientTransport` في Android SDK
  `listIdentifierPolicies()`، `معرف الحل (معرف السياسة، الإدخال،
  EncryptedInputHex)`, `issueIdentifierClaimReceipt(accountId, PolicyId,
  الإدخال، المشفرInputHex)`, and `getIdentifierClaimByReceiptHash(...)`,
  بالإضافة إلى `IdentifierNormalization` لنفس قواعد التحديد الأساسي.
- `IdentifierResolveRequest` و
  `IdentifierPolicySummary.plaintextRequest(...)` /
  يوفر مساعدو `.encryptedRequest(...)` سطح طلب Android المكتوب،
  بينما `IdentifierPolicySummary.encryptInput(...)` /
  `.encryptedRequestFromInput(...)` يشتق مظروف النص المشفر BFV
  محليًا من معلمات السياسة المنشورة.
  يتحقق `IdentifierResolutionReceipt.verifySignature(policy)` من الإرجاع
  توقيع المحلل من جانب العميل.

مجموعة التعليمات الحالية:-`RegisterIdentifierPolicy`
-`ActivateIdentifierPolicy`
- `ClaimIdentifier` (مرتبط بالإيصال؛ تم رفض مطالبات `opaque_id` الأولية)
-`RevokeIdentifier`

توجد الآن ثلاث واجهات خلفية في `iroha_crypto::ram_lfe`:

- الالتزام التاريخي `HKDF-SHA3-512` PRF، و
- مقيم تقاربي سري مدعوم من BFV يستهلك معرفًا مشفرًا من BFV
  فتحات مباشرة. عندما يتم إنشاء `iroha_crypto` بالإعداد الافتراضي
  ميزة `bfv-accel`، يستخدم مضاعفة حلقات BFV حتمية دقيقة
  الواجهة الخلفية لـ CRT-NTT داخليًا؛ يعود تعطيل هذه الميزة إلى
  مسار الكتاب المدرسي العددي مع مخرجات متطابقة، و
- مقيم سري مبرمج مدعوم من BFV يستمد تعليماته
  تتبع التنفيذ بنمط ذاكرة الوصول العشوائي (RAM) عبر السجلات المشفرة وذاكرة النص المشفر
  الممرات قبل اشتقاق المعرف غير الشفاف وتجزئة الإيصال. المبرمجة
  تتطلب الواجهة الخلفية الآن أرضية معامل BFV أقوى من المسار المتقارب، و
  يتم نشر معلماته العامة في حزمة أساسية تتضمن ملف
  ملف تعريف تنفيذ RAM-FHE الذي تستهلكه المحافظ وأجهزة التحقق.

هنا يعني BFV مخطط Brakerski/Fan-Vercauteren FHE المطبق في
`crates/iroha_crypto/src/fhe_bfv.rs`. إنها آلية التنفيذ المشفرة
تستخدمه الواجهات الخلفية المتقاربة والمبرمجة، وليس اسم المخفي الخارجي
تجريد الوظيفةيستخدم Torii الواجهة الخلفية المنشورة بواسطة التزام السياسة. عندما الخلفية BFV
نشطًا، ويتم تسوية طلبات النص العادي ثم تشفيرها من جانب الخادم من قبل
التقييم. يتم تقييم طلبات BFV `encrypted_input` للواجهة الخلفية المتقاربة
بشكل مباشر ويجب أن يتم تطبيعه بالفعل من جانب العميل؛ الخلفية المبرمجة
يقوم بتعريف الإدخال المشفر مرة أخرى إلى BFV الحتمي للمحلل
المغلف قبل تنفيذ برنامج ذاكرة الوصول العشوائي السري بحيث تبقى تجزئات الإيصال
مستقرة عبر النصوص المشفرة المكافئة لغويا.

## 2. اشتقاق معرفات UAID والتحقق منها

هناك ثلاث طرق مدعومة للحصول على UAID:

1. **اقرأها من نماذج الحالة العالمية أو نماذج SDK.** أي `Account`/`AccountDetails`
   الحمولة التي تم الاستعلام عنها عبر Torii تحتوي الآن على الحقل `uaid` الذي تم ملؤه عند
   اختار المشارك في حسابات عالمية.
2. **الاستعلام عن سجلات UAID.** يكشف Torii
   `GET /v1/space-directory/uaids/{uaid}` الذي يقوم بإرجاع روابط مساحة البيانات
   وإظهار البيانات الوصفية التي يستمر بها مضيف دليل الفضاء (انظر
   `docs/space-directory.md` §3 لعينات الحمولة).
3. **اشتقها بشكل حتمي.** عند تشغيل معرفات UAID الجديدة دون الاتصال بالإنترنت، يتم إجراء التجزئة
   بذرة المشارك الأساسية بـ Blake2b-256 وبادئة النتيجة بـ
   `uaid:`. يعكس المقتطف أدناه المساعد الموثق فيه
   `docs/space-directory.md` §3.3:

   ```python
   import hashlib
   seed = b"participant@example"  # canonical address/domain seed
   digest = hashlib.blake2b(seed, digest_size=32).hexdigest()
   print(f"uaid:{digest}")
   ```قم دائمًا بتخزين الحرف الحرفي بأحرف صغيرة وقم بتطبيع المسافة البيضاء قبل التجزئة.
مساعدي CLI مثل `iroha app space-directory manifest scaffold` وAndroid
يطبق المحلل اللغوي `UaidLiteral` نفس قواعد التشذيب حتى تتمكن مراجعات الإدارة من ذلك
التحقق من القيم بدون نصوص برمجية مخصصة.

## 3. فحص مقتنيات وقوائم UAID

مجمع المحفظة الحتمية في `iroha_core::nexus::portfolio`
يعرض كل زوج من الأصول/مساحة البيانات التي تشير إلى UAID. المشغلين وSDKs
يمكن استهلاك البيانات من خلال الأسطح التالية:

| السطح | الاستخدام |
|---------|------|
| `GET /v1/accounts/{uaid}/portfolio` | إرجاع مساحة البيانات → الأصول → ملخصات الرصيد؛ الموصوفة في `docs/source/torii/portfolio_api.md`. |
| `GET /v1/space-directory/uaids/{uaid}` | يسرد معرفات مساحة البيانات + القيم الحرفية للحساب المرتبطة بـ UAID. |
| `GET /v1/space-directory/uaids/{uaid}/manifests` | يوفر سجل `AssetPermissionManifest` الكامل لعمليات التدقيق. |
| `iroha app space-directory bindings fetch --uaid <literal>` | اختصار CLI الذي يلتف نقطة نهاية الارتباط ويكتب JSON اختياريًا على القرص (`--json-out`). |
| `iroha app space-directory manifest fetch --uaid <literal> --json-out <path>` | جلب حزمة JSON الواضحة لحزم الأدلة. |

مثال لجلسة CLI (عنوان URL Torii الذي تم تكوينه عبر `torii_api_url` في `iroha.json`):

```bash
iroha app space-directory bindings fetch \
  --uaid uaid:86e8ee39a3908460a0f4ee257bb25f340cd5b5de72735e9adefe07d5ef4bb0df \
  --json-out artifacts/uaid86/bindings.json

iroha app space-directory manifest fetch \
  --uaid uaid:86e8ee39a3908460a0f4ee257bb25f340cd5b5de72735e9adefe07d5ef4bb0df \
  --json-out artifacts/uaid86/manifests.json
```

قم بتخزين لقطات JSON جنبًا إلى جنب مع تجزئة البيان المستخدمة أثناء المراجعات؛ ال
يقوم مراقب دليل الفضاء بإعادة بناء خريطة `uaid_dataspaces` كلما ظهرت
التنشيط أو انتهاء الصلاحية أو الإلغاء، لذا فإن هذه اللقطات هي أسرع طريقة للإثبات
ما هي الارتباطات التي كانت نشطة في عصر معين.## 4. القدرة على النشر تتجلى بالأدلة

استخدم تدفق CLI أدناه كلما تم طرح بدل جديد. يجب على كل خطوة
الأرض في حزمة الأدلة المسجلة لتوقيع الحوكمة.

1. **قم بتشفير البيان JSON** حتى يتمكن المراجعون من رؤية التجزئة الحتمية من قبل
   تقديم:

   ```bash
   iroha app space-directory manifest encode \
     --json fixtures/space_directory/capability/eu_regulator_audit.manifest.json \
     --out artifacts/eu_regulator_audit.manifest.to \
     --hash-out artifacts/eu_regulator_audit.manifest.hash
   ```

2. **انشر البدل** باستخدام الحمولة النافعة Norito (`--manifest`) أو
   وصف JSON (`--manifest-json`). قم بتسجيل إيصال Torii/CLI الزائد
   تجزئة التعليمات `PublishSpaceDirectoryManifest`:

   ```bash
   iroha app space-directory manifest publish \
     --manifest artifacts/eu_regulator_audit.manifest.to \
     --reason "ESMA wave 2 onboarding"
   ```

3. **التقاط دليل SpaceDirectoryEvent.** اشترك في
   `SpaceDirectoryEvent::ManifestActivated` وقم بتضمين حمولة الحدث
   الحزمة حتى يتمكن المدققون من التأكد من وقت حدوث التغيير.

4. **إنشاء حزمة تدقيق** تربط البيان بملف تعريف مساحة البيانات الخاص به و
   خطاف القياس عن بعد:

   ```bash
   iroha app space-directory manifest audit-bundle \
     --manifest artifacts/eu_regulator_audit.manifest.to \
     --profile fixtures/space_directory/profile/cbdc_lane_profile.json \
     --out-dir artifacts/eu_regulator_audit_bundle
   ```

5. **التحقق من الارتباطات عبر Torii** (`bindings fetch` و`manifests fetch`) و
   قم بأرشفة ملفات JSON هذه باستخدام حزمة التجزئة + أعلاه.

قائمة التحقق من الأدلة:

- [ ] تجزئة البيان (`*.manifest.hash`) موقعة من قبل الموافق على التغيير.
- [ ] إيصال CLI/Torii لمكالمة النشر (stdout أو `--json-out`).
- [ ] `SpaceDirectoryEvent` تفعيل الحمولة النافعة.
- [ ] تدقيق دليل الحزمة مع ملف تعريف مساحة البيانات، والخطافات، ونسخة البيان.
- [ ] الارتباطات + لقطات البيان التي تم جلبها من Torii بعد التنشيط.يعكس هذا المتطلبات الواردة في `docs/space-directory.md` §3.2 أثناء إعطاء SDK
أصحاب صفحة واحدة للإشارة إليها أثناء مراجعات الإصدار.

## 5. قوالب البيان التنظيمي/الإقليمي

استخدم تركيبات الريبو كنقاط بداية عند ظهور القدرة على الصياغة
للمنظمين أو المشرفين الإقليميين. يوضحون كيفية تحديد نطاق السماح/الرفض
القواعد وشرح مذكرات السياسة التي يتوقعها المراجعون.

| لاعبا أساسيا | الغرض | أبرز الأحداث |
|---------|--------|------------|
| `fixtures/space_directory/capability/eu_regulator_audit.manifest.json` | خلاصة تدقيق ESMA/ESRB. | بدلات القراءة فقط لـ `compliance.audit::{stream_reports, request_snapshot}` مع رفض المكاسب في عمليات نقل التجزئة للحفاظ على معرفات UAID التنظيمية سلبية. |
| `fixtures/space_directory/capability/jp_regulator_supervision.manifest.json` | حارة الإشراف JFSA. | إضافة بدل `cbdc.supervision.issue_stop_order` محدد (نافذة PerDay + `max_amount`) ورفض صريح لـ `force_liquidation` لفرض عناصر التحكم المزدوجة. |

عند استنساخ هذه التركيبات، قم بالتحديث:

1. معرفات `uaid` و`dataspace` لمطابقة المشارك والمسار الذي تقوم بتمكينه.
2. نوافذ `activation_epoch`/`expiry_epoch` بناءً على جدول الإدارة.
3. حقول `notes` مع مراجع سياسة الجهة التنظيمية (مقالة MiCA، JFSA
   دائرية، الخ).
4. النوافذ المسموح بها (`PerSlot`، `PerMinute`، `PerDay`) والاختيارية
   `max_amount` أحرف استهلالية بحيث تفرض حزم SDK نفس الحدود التي يفرضها المضيف.

## 6. ملاحظات الترحيل لعملاء SDKيجب أن يتم الترحيل إلى عمليات تكامل SDK الحالية التي تشير إلى معرفات الحساب لكل مجال
الأسطح المتمحورة حول UAID الموصوفة أعلاه. استخدم قائمة التحقق هذه أثناء الترقيات:

  معرفات الحساب. بالنسبة إلى Rust/JS/Swift/Android، يعني هذا الترقية إلى الإصدار الأحدث
  صناديق مساحة العمل أو تجديد روابط Norito.
- **مكالمات API:** استبدل استعلامات المحفظة على نطاق المجال بـ
  `GET /v1/accounts/{uaid}/portfolio` ونقاط نهاية البيان/الربط.
  يقبل `GET /v1/accounts/{uaid}/portfolio` استعلام `asset_id` اختياري
  المعلمة عندما تحتاج المحافظ إلى مثيل أصل واحد فقط. مساعدي العملاء من هذا القبيل
  مثل `ToriiClient.getUaidPortfolio` (JS) وAndroid
  `SpaceDirectoryClient` يلتف بالفعل حول هذه المسارات؛ تفضلهم على مفصل
  رمز HTTP.
- **التخزين المؤقت والقياس عن بعد:** إدخالات ذاكرة التخزين المؤقت بواسطة UAID + مساحة البيانات بدلاً من الخام
  معرفات الحساب، وإصدار قياس عن بعد يُظهر حرف UAID حتى تتمكن العمليات من ذلك
  قم بمحاذاة السجلات مع أدلة دليل الفضاء.
- **معالجة الأخطاء:** تعرض نقاط النهاية الجديدة أخطاء تحليل UAID الصارمة
  موثقة في `docs/source/torii/portfolio_api.md`؛ سطح تلك الرموز
  حرفيًا حتى تتمكن فرق الدعم من فرز المشكلات دون خطوات إعادة الإنتاج.
- **الاختبار:** قم بتوصيل التركيبات المذكورة أعلاه (بالإضافة إلى بيانات UAID الخاصة بك)
  في مجموعات اختبار SDK لإثبات Norito ذهابًا وإيابًا وتقييمات البيان
  تطابق تنفيذ المضيف.

## 7. المراجع- `docs/space-directory.md` - دليل تشغيل المشغل الذي يحتوي على تفاصيل أعمق لدورة الحياة.
- `docs/source/torii/portfolio_api.md` - مخطط REST لمحفظة UAID و
  نقاط النهاية الواضحة.
- `crates/iroha_cli/src/space_directory.rs` — تنفيذ واجهة سطر الأوامر (CLI) المشار إليه في
  هذا الدليل.
- `fixtures/space_directory/capability/*.manifest.json` - المنظم، والتجزئة، و
  قوالب بيان CBDC جاهزة للاستنساخ.
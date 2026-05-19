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
  الاسم المستعار المؤهل للمجال مثل `merchant@banka.paynet` والاسم المستعار لجذر مساحة البيانات
  مثل `merchant@paynet` يمكن حلهما إلى نفس `AccountId` المتعارف عليه.
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

Current Torii routes:

| Route | Purpose |
|-------|---------|
| `GET /v1/ram-lfe/program-policies` | Lists active and inactive RAM-LFE program policies plus their public execution metadata, including optional BFV `input_encryption` parameters and the programmed-backend `ram_fhe_profile`. |
| `POST /v1/ram-lfe/programs/{program_id}/execute` | Accepts `{ encrypted_input }` only and returns the stateless `RamLfeExecutionReceipt`, `{ output_ciphertext, output_hash, receipt_hash }`, and no plaintext output. The current Torii runtime issues receipts for the programmed BFV backend. |
| `POST /v1/ram-lfe/receipts/verify` | Statelessly validates a `RamLfeExecutionReceipt` against the published on-chain program policy and optionally checks that a caller-supplied encrypted `output_hex` matches the receipt `output_hash`. |
| `GET /v1/identifier-policies` | Lists active and inactive hidden-function policy namespaces plus their public metadata, including optional BFV `input_encryption` parameters, the required `normalization` mode for encrypted client-side input, and `ram_fhe_profile` for programmed BFV policies. |
| `POST /v1/accounts/{account_id}/identifiers/claim-receipt` | Accepts `{ policy_id, encrypted_input, output_opening }`. The BFV `encrypted_input` must already be normalized according to the published policy mode. The endpoint derives the `opaque:` handle from the verified external `RamLfeOutputOpening` and returns a signed receipt that `ClaimIdentifier` can submit on-chain. |
| `POST /v1/identifiers/resolve` | Accepts `{ policy_id, encrypted_input, output_opening }`. The endpoint re-evaluates the encrypted input, verifies the external output opening, derives the `opaque:` handle from the opened output hash, and returns a nested `{ payload, attestation }` receipt when an active claim exists. |
| `GET /v1/identifiers/receipts/{receipt_hash}` | Looks up the persisted `IdentifierClaimRecord` bound to a deterministic receipt hash so operators and SDKs can audit claim ownership or diagnose replay / mismatch failures without scanning the full identifier index. |

Torii's in-process execution runtime is configured under
`torii.ram_lfe.programs[*]`, keyed by `program_id`. The identifier routes now
reuse that same RAM-LFE runtime instead of a separate `identifier_resolver`
config surface.

Current SDK support:

- `normalizeIdentifierInput(value, normalization)` matches the Rust
  canonicalizers for `exact`, `lowercase_trimmed`, `phone_e164`,
  `email_address`, and `account_number`.
- `ToriiClient.listIdentifierPolicies()` lists policy metadata, including BFV
  input-encryption metadata when the policy publishes it, plus a decoded
  BFV parameter object via `input_encryption_public_parameters_decoded`.
  Programmed policies also expose the decoded `ram_fhe_profile`. That field is
  intentionally BFV-scoped: it lets wallets verify the expected register
  count, lane count, canonicalization mode, and minimum ciphertext modulus for
  the programmed FHE backend before encrypting client-side input.
- `getIdentifierBfvPublicParameters(policy)` and
  `buildIdentifierRequestForPolicy(policy, { encryptedInput | input,
  encrypt: true, outputOpening })` help JS callers consume published BFV
  metadata and build policy-aware encrypted request bodies without
  reimplementing policy-id and normalization rules.
- `encryptIdentifierInputForPolicy(policy, input, { seedHex? })` and
  `buildIdentifierRequestForPolicy(policy, { input, encrypt: true,
  outputOpening })` now let JS wallets construct the full BFV Norito
  ciphertext envelope locally from published policy parameters instead of
  shipping prebuilt ciphertext hex.
- `ToriiClient.resolveIdentifier({ policyId, encryptedInput, outputOpening })`
  resolves a hidden identifier and returns the signed nested
  `{ payload, attestation }` receipt.
- `ToriiClient.issueIdentifierClaimReceipt(accountId, { policyId,
  encryptedInput, outputOpening })` issues the signed receipt needed by
  `ClaimIdentifier`.
- `verifyIdentifierResolutionReceipt(receipt, policy)` verifies the returned
  receipt against the policy resolver key on the client side, and
  `ToriiClient.getIdentifierClaimByReceiptHash(receiptHash)` fetches the
  persisted claim record for later audit/debug flows.
- `IrohaSwift.ToriiClient` now exposes `listIdentifierPolicies()`,
  `resolveIdentifier(policyId:encryptedInputHex:outputOpening:)`,
  `issueIdentifierClaimReceipt(accountId:policyId:encryptedInputHex:outputOpening:)`,
  and `getIdentifierClaimByReceiptHash(_)`, plus
  `ToriiIdentifierNormalization` for the same phone/email/account-number
  canonicalization modes.
- `ToriiIdentifierLookupRequest` and encrypted request helpers provide the
  typed Swift request surface for resolve and claim-receipt calls, and Swift
  policies can now derive the BFV ciphertext locally via `encryptInput(...)`.
- `ToriiIdentifierResolutionReceipt.verifySignature(using:)` validates that
  the top-level receipt fields match the signed payload and verifies the
  resolver signature client-side before submission.
- `HttpClientTransport` in the Android SDK now exposes
  `listIdentifierPolicies()`, encrypted-only `resolveIdentifier(...)`,
  encrypted-only `issueIdentifierClaimReceipt(...)`, and
  `getIdentifierClaimByReceiptHash(...)`,
  plus `IdentifierNormalization` for the same canonicalization rules.
- `IdentifierResolveRequest` and encrypted request helpers provide the typed
  Android request surface, while `IdentifierPolicySummary.encryptInput(...)`
  derives the BFV ciphertext envelope locally from published policy
  parameters.
  `IdentifierResolutionReceipt.verifySignature(policy)` verifies the returned
  resolver signature client-side.

Current instruction set:

- `RegisterIdentifierPolicy`
- `ActivateIdentifierPolicy`
- `ClaimIdentifier` (receipt-bound; raw `opaque_id` claims are rejected)
- `RevokeIdentifier`

Three backends now exist in `iroha_crypto::ram_lfe`:

- the historical commitment-bound `HKDF-SHA3-512` PRF, and
- a BFV-backed secret affine evaluator that consumes BFV-encrypted identifier
  slots directly. When `iroha_crypto` is built with the default
  `bfv-accel` feature, BFV ring multiplication uses an exact deterministic
  CRT-NTT backend internally; disabling that feature falls back to the
  scalar schoolbook path with identical outputs, and
- a BFV-backed secret programmed evaluator that derives an instruction-driven
  RAM-style execution trace over encrypted registers and ciphertext memory
  lanes before deriving the opaque identifier and receipt hash. The programmed
  backend now requires a stronger BFV modulus floor than the affine path, and
  its public parameters are published in a canonical bundle that includes the
  RAM-FHE execution profile consumed by wallets and verifiers.

Here BFV means the Brakerski/Fan-Vercauteren FHE scheme implemented in
`crates/iroha_crypto/src/fhe_bfv.rs`. It is the encrypted-execution mechanism
used by the affine and programmed backends, not the name of the outer hidden
function abstraction.

Torii uses the backend published by the policy commitment. For the first
release, RAM-LFE and hidden-identifier routes are encrypted-only: Torii does
not accept plaintext inputs, does not hold BFV secret keys, and does not
decrypt input or output ciphertexts. Identifier claim and resolve requests must
include an externally signed `RamLfeOutputOpening`; the `opaque:` identifier is
derived from the verified opened-output hash, not from Torii-side plaintext or
from the ciphertext hash alone.

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
   digest = bytearray(hashlib.blake2b(seed, digest_size=32).digest())
   digest[-1] |= 1
   print(f"uaid:{digest.hex()}")
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

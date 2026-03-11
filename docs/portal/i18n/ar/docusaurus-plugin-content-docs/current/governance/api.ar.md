---
lang: ar
direction: rtl
source: docs/portal/docs/governance/api.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

الحالة: مسودة/ تصور لمرافقة مهام تنفيذ الوتيرة. قد تأخذ الصيغ خلال التنفيذ. الحامية وسياسة RBAC الإلكترونية القياسية؛ يمكن لتوريي التوقيع/مراسلة المعاملات عندما يتم توفير `authority` و`private_key`، ويبني العملاء فريدونها إلى `/transaction`.

نظرة عامة
- نقاط جميع النهائية JSON. تفاعلات الانتاج، تتضمن الردود `tx_instructions` - مصفوفة من تعليمة هيكلية واحدة او اكثر:
  - `wire_id`: تم تعريف السجل لنوع التعليمة
  - `payload_hex`: بايتات محمولة Norito (ست عشرية)
- اذا تم توفير `authority` و`private_key` (او `private_key` في بطاقات الاقتراع DTOs)، يقوم Torii بالتوقيع والارسال ويعيد أيضا `tx_instructions`.
- بخلاف ذلك، يجمع العملاء التوقيع على المعاملة باستخدام السلطة وchain_id الخاصة بهم، ثم يوقعون ويرسلون POST إلى `/transaction`.
- تغطية SDK:
- Python (`iroha_python`): `ToriiClient.get_governance_proposal_typed` يعيد `GovernanceProposalResult` (يوحد استهلاك الحالة/النوع)، و`ToriiClient.get_governance_referendum_typed` يعيد `GovernanceReferendumResult`، و`ToriiClient.get_governance_tally_typed` يعيد `GovernanceTally`، و`ToriiClient.get_governance_locks_typed` يعيد `GovernanceLocksResult`، و`ToriiClient.get_governance_unlock_stats_typed` يعيد `GovernanceUnlockStats`، و`ToriiClient.list_governance_instances_typed` يعيد `GovernanceInstancesPage`، فارضا وصولا بنمط مكتوب عبر سطح المكتب مع امثلة في استخدام README.
- عميل Python خفيف (`iroha_torii_client`): `ToriiClient.finalize_referendum` و`ToriiClient.enact_proposal` يعيدان حزم `GovernanceInstructionDraft` typed (تغلف هيكل `tx_instructions` من Torii)، تعريف JSON يدوي عند تركيب سكربتات Finalize/Enact.
- JavaScript (`@iroha/iroha-js`): `ToriiClient` يعرض مساعدين مكتوبين للمقترحات، والاستفتاءات، وتالي، ولوكس، واحصاءات فتح، والان `listGovernanceInstances(namespace, options)` مع نقاط نهاية المجلس (`getGovernanceCouncilCurrent`, `governanceDeriveCouncilVrf`, `governancePersistCouncil`, `getGovernanceCouncilAudit`) كي ينشط عملاء Node.js من ترقيم الصفحات لـ `/v1/gov/instances/{ns}` وتشغيل تدفقات VRF باستمرار سرد عددات العقود الموجودة.

نقاط النهاية

- المشاركة `/v1/gov/proposals/deploy-contract`
  - الطلب (JSON):
    {
      "مساحة الاسم": "التطبيقات"،
      "contract_id": "my.contract.v1",
      "code_hash": "blake2b32:..." | "...64 سداسيًا"،
      "abi_hash": "blake2b32:..." | "...64 سداسيًا"،
      "abi_version": "1"،
      "نافذة": { "سفلي": 12345، "علوي": 12400}،
      "السلطة": "i105...؟",
      "مفتاح_خاص": "...؟"
    }
  -الرد (JSON):
    { "ok": صحيح، "proposal_id": "...64hex"، "tx_instructions": [{ "wire_id": "..."، "payload_hex": "..." }] }
  - التحقق: تقوم بتوحيد `abi_hash` للنسخة `abi_version` المقدمة وترفض عدم التطابق. لـ `abi_version = "v1"` القيمة محددة هي `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))`.

عقود API (نشر)
- المشاركة `/v1/contracts/deploy`
  - الطلب: { "authority": "i105..."، "private_key": "..."، "code_b64": "..." }
  - السلوك: يحسب `code_hash` من برنامج IVM و`abi_hash` من ترويسة `abi_version`، ثم يرسل `RegisterSmartContractCode` (البيان) و`RegisterSmartContractBytes` (بايتات `.to`) كاملة) نيابة عن `authority`.
  -الرد: { "ok": صحيح، "code_hash_hex": "..."، "abi_hash_hex": "..." }
  - ذي صلة:
    - الحصول على `/v1/contracts/code/{code_hash}` -> إعادة البيان المخزن
    - الحصول على `/v1/contracts/code-bytes/{code_hash}` -> إعادة `{ code_b64 }`
- المشاركة `/v1/contracts/instance`
  - الطلب: { "authority": "i105..."، "private_key": "..."، "namespace": "apps"، "contract_id": "calc.v1"، "code_b64": "..." }
  - السلوك: ينشر بايتات المقاولين ويفعل الاتصال فورا `(namespace, contract_id)` عبر `ActivateContractInstance`.
  -الرد: { "ok": true، "namespace": "apps"، "contract_id": "calc.v1"، "code_hash_hex": "..."، "abi_hash_hex": "..." }

خدمة الاسماء المستعارة
- المشاركة `/v1/aliases/voprf/evaluate`
  - الطلب: { "blinded_element_hex": "..." }
  -رد: { "evaluated_element_hex": "...128hex", "backend": "blake2b512-mock" }
    - `backend` عاكس الضوء. القيمة الحالية: `blake2b512-mock`.
  - آراء: مقيم mock حتمي يطبق Blake2b512 مع فصل مجال `iroha.alias.voprf.mock.v1`. يتم إجراء الاختبارات المخصصة حتى يتم توصيل خط إنتاج VOPRF عبر Iroha.
  - الأخطاء: HTTP `400` عند ادخال hex غير صالح. أعيد Torii ظرف Norito `ValidationFail::QueryFailed::Conversion` مع رسالة خطا من وحدة فك الترميز.
- المشاركة `/v1/aliases/resolve`
  - الطلب: { "الاسم المستعار": "GB82 WEST 1234 5698 7654 32" }
  -الرد: { "الاسم المستعار": "GB82WEST12345698765432"، "account_id": "i105..."، "index": 0، "source": "iso_bridge" }
  - ملاحظات: يتطلب تشغيل جسر ISO (`[iso_bridge.account_aliases]` في `iroha_config`). يقوم Torii بتطبيع الاسماء عبر ازالة الفراغات وتحويلها الى حرف كبير قبل البحث. أعاد 404 عندما يكون الاسم غير موجود و503 عندما يكون وقت التشغيل الخاص بـ ISO Bridge معطلا.
- المشاركة `/v1/aliases/resolve_index`
  - الطلب: { "الفهرس": 0 }
  -الرد: { "index": 0, "alias": "GB82WEST12345698765432", "account_id": "i105...", "source": "iso_bridge" }
  - ملاحظات: مؤشرات الاسماء تعين بشكل حتمي حسب الوضع (0-based). يمكن تخزين الردود دون اتصال بالإنترنت لإنشاء مسارات تدقيق لاحداث التصديق الخاصة بالاسماء.

بحجم الكود
- العامل المخصص: `max_contract_code_bytes` (JSON u64)
  - يتحكم في الحد الاقصى لذلك (بالبايت) لعقد العقود على العقود.
  - الافتراضي: 16 ميجابايت. لا تعترض على `RegisterSmartContractBytes` عندما يتجاوز حجم الصورة `.to` الحد مع خطا الاحتمال invariant.
  - يمكن للمشغلين التقدم عبر `SetParameter(Custom)` مع `id = "max_contract_code_bytes"` وقابل للطباعة الرقمية.

- المشاركة `/v1/gov/ballots/zk`
  - الطلب: { "authority": "i105..."، "private_key": "...؟"، "chain_id": "..."، "election_id": "e1"، "proof_b64": "..."، "public": {...} }
  -الرد: { "ok": صحيح، "مقبول": صحيح، "tx_instructions": [{...}] }
  - ملاحظات:
    - عندما تتضمن المدخلات العامة للدائرة `owner` و`amount` و`duration_blocks`، وتتحقق البرهان من VK المضبوطة، ينشئ عقدة او يمدد قفل للتكامل لـ `election_id` مع ذلك `owner`. الاتجاهات الحالية مجهولة (`unknown`)؛ تحديث المبلغ/انتهاء الصلاحية فقط. عادات التصويت الرتيبة: المبلغ وانتهاء الصلاحية في المستشفى فقط (تطبق العقدة max(amount, prev.amount) وmax(expiry, prev.expiry)).
    - عادات التصويت ZK التي تحاول تقليل المبلغ او انتهاء الصلاحية يتم رفضها من جهة معينة مع البرمجة `BallotRejected`.
    - يجب عقد مؤتمر `ZK_VOTE_VERIFY_BALLOT` قبل ادراج `SubmitBallot`؛ ويفترض أنها مزلاج لمرة واحدة.

- المشاركة `/v1/gov/ballots/plain`
  - الطلب: { "authority": "i105..."، "private_key": "...؟"، "chain_id": "..."، "referendum_id": "r1"، "owner": "i105..."، "amount": "1000"، "duration_blocks": 6000، "direction": "Aye|Nay|امتناع" }
  -الرد: { "ok": صحيح، "مقبول": صحيح، "tx_instructions": [{...}] }
  - آراء: عادات التصويت فقط - لا يمكن لتصويت جديد مؤثر تقليل المبلغ أو انتهاء الصلاحية لقفل موجود. يجب ان يساوي `owner` حسب الرغبة. الحد الادني لمدة طويلة `conviction_step_blocks`.

- المشاركة `/v1/gov/finalize`
  - الطلب: { "referendum_id": "r1"، "proposal_id": "...64hex"، "authority": "i105...؟"، "private_key": "...؟" }
  -الرد: { "ok": true, "tx_instructions": [{ "wire_id": "...FinalizeReferendum", "payload_hex": "..." }] }
  - الاثر على الهيكل (الهيكل): هيكل زجاجي كامل نشر معتمد يدرج `ContractManifest` ادنى بمفتاح `code_hash` مع `abi_hash` الخبرة ويضع الاقتراحات بحالة Enacted. اذا كان هناك بيان موجود لـ `code_hash` المجال `abi_hash` مختلف، يتم رفض التنفيذ.
  - ملاحظات:
    - لا مسابقة ZK، يجب أن تعقد مؤتمرات للاتصال `ZK_VOTE_VERIFY_TALLY` قبل تنفيذ `FinalizeElection`؛ ويفترض أنها مزلاج لمرة واحدة. يرفض `FinalizeReferendum` الاستفتاءات ZK حتى يتم انهاء حصته للانتخابات.
    - الاغلاق التلقائي عند `h_end` يصدر الموافقة/الرفض فقط للاستفتاءات عادي؛ يبقى استفتاءات ZK مغلق حتى يتم إرسال حصيلة منته ويجري تنفيذ `FinalizeReferendum`.
    - فحوصات الإقبال تستخدم موافقة+رفض فقط؛ الامتناع لا يجب ضمنا الإقبال.

- المشاركة `/v1/gov/enact`
  - الطلب: { "proposal_id": "...64hex"، "preimage_hash": "...64hex؟"، "window": { "lower": 0، "upper": 0}؟، "authority": "i105...؟"، "private_key": "...؟" }
  -الرد: { "ok": true, "tx_instructions": [{ "wire_id": "...EnactReferendum", "payload_hex": "..." }] }
  - آراء: مقدمة Torii للموقع المناسب عندما تختار `authority`/`private_key`; وإعادة هيكلة لتوقيع العملاء وارساله. الـ preimage اختيارية واليا معلوماتية.

- احصل على `/v1/gov/proposals/{id}`
  - المسار `{id}`: مُعرّف الاقتراحات السداسي (64 حرفًا)
  -الرد: { "وجد": منطقي، "اقتراح": { ... }؟ }

- احصل على `/v1/gov/locks/{rid}`
  - المسار `{rid}`: string لمعرف الاستفتاء
  -الرد: { "وجد": منطقي، "referendum_id": "rid"، "locks": { ... }؟ }

- احصل على `/v1/gov/council/current`
  -الرد: { "epoch": N, "members": [{ "account_id": "..." }, ...] }
  - ملاحظات: جدول المجلس المحفوظ اذا كان موجودا؛ ويشتق بديلا حطمياً باستخدام اصل الحصة المضبوطة والعتبات (يعكس مواصفات VRF حتى يشهد معادل VRF على السباق).

- POST `/v1/gov/council/derive-vrf` (الميزة: gov_vrf)
  - الطلب: { "حجم اللجنة": 21، "العصر": 123؟ , "candidates": [{ "account_id": "..."، "variant": "Normal|Small"، "pk_b64": "..."، "proof_b64": "..." }, ...] }
  - يتصرف: يتحقق من برهان VRF لكل مرشح مقابل المدخل المسجل المشتق من `chain_id` و`epoch` ومنارة اخر تجزئة للبلوك؛ يرتب حسب البايتات الخرج تنازلي مع كاسرات تعادل؛ ويعيد اعلى `committee_size` من الاعضاء. لا يتم الحفظ.
  -الرد: { "epoch": N, "members": [{ "account_id": "..." } ...], "total_candidates": M, "تم التحقق": K }
  - ملاحظات: عادي = pk في G1، إثبات في G2 (96 بايت). صغير = pk في G2، إثبات في G1 (48 بايت). المدخلات مفصولة بالمجال ضخمة `chain_id`.

### تهربات ال تور (iroha_config `gov.*`)

يتم ضبط المجلس الاحتياطي الذي يستخدمه Torii عندما لا يوجد قائمة محفوظ عبر `iroha_config`:

```toml
[gov]
  vk_ballot.backend = "halo2/ipa"
  vk_ballot.name    = "ballot_v1"
  vk_tally.backend  = "halo2/ipa"
  vk_tally.name     = "tally_v1"
  plain_voting_enabled = false
  conviction_step_blocks = 100
  max_conviction = 6
  approval_q_num = 1
  approval_q_den = 2
  min_turnout = 0
  parliament_committee_size = 21
  parliament_term_blocks = 43200
  parliament_min_stake = 1
  parliament_eligibility_asset_id = "SORA#stake"
```

استبدال البيئة المكافئة:

```
GOV_VK_BACKEND=halo2/ipa
GOV_VK_NAME=ballot_v1
GOV_PARLIAMENT_COMMITTEE_SIZE=21
GOV_PARLIAMENT_TERM_BLOCKS=43200
GOV_PARLIAMENT_MIN_STAKE=1
GOV_PARLIAMENT_ELIGIBILITY_ASSET_ID=SORA#stake
GOV_ALIAS_TEU_MINIMUM=0
GOV_ALIAS_FRONTIER_TELEMETRY=true
````parliament_committee_size` يحدد عدد الأجزاء الاحتياطية المعادين عندما لا يوجد مجلس محفوظ، و`parliament_term_blocks` يحدد طول الحقبة المستخدمة لا يشتقاق بذرة (`epoch = floor(height / term_blocks)`)، و`parliament_min_stake` يفرض الحد الادنى من الحصة (بوحدات صغيرة) على اصل الاهلية، و`parliament_eligibility_asset_id` يحدد اي رصيد اصل يتم مسحه عند بناء مجموعة غير ذلك.

تأكد من VK للتجاوز بلا: التحقق من بطاقات الاقتراع يتطلب دائما مفتاح التحقق `Active` ببايتات مضمنة، ولا يجب ان تعتمد البيئات على اختبارات التخطي للتخطي التحقق.

RBAC
- التنفيذ على تتطلب صلاحيات:
  - المقترحات: `CanProposeContractDeployment{ contract_id }`
  - أوراق الاقتراع: `CanSubmitGovernanceBallot{ referendum_id }`
  - التشريع: `CanEnactGovernance`
  - إدارة المجلس (مستقبلا): `CanManageParliament`

مساحات الأسماء محمية
- المعامل مخصص `gov_protected_namespaces` (مصفوفة JSON من السلاسل) تقوم بالنشر إلى مساحات الأسماء المحددة.
- على العملاء تتضمن مفاتيح البيانات الوصفية للمعاملة عند النشر إلى مساحات الأسماء المحمية:
  - `gov_namespace`: مساحة الاسم الهدف (مثلا "apps")
  - `gov_contract_id`: العقد الدائري داخل مساحة الاسم
- `gov_manifest_approvers`: مصفوفة JSON اختيارية من معرفات الحساب للمقررين. عندما أعلن عن البيان لمسار ما هو النصاب القانوني أكبر من واحد، يتطلب قبولًا مخصصًا بالإضافة إلى العناصر المدرجة في النصاب القانوني الخاص بالبيان.
- سجل عدادات القبول التليميترية عبر `governance_manifest_admission_total{result}` لتمييز الفائزات الناجحة عن تجارب `missing_manifest` و`non_validator_authority` و`quorum_rejected` و`protected_namespace_rejected` و`runtime_hook_rejected`.
- سجل إنفاذ المسار التليميتري عبر `governance_manifest_quorum_total{outcome}` (القيم `satisfied` / `rejected`) حتى يبدأون في تدقيق طلبات الاستفسار.
- ليس المسارات القائمة المسموح بها بـ مساحات الأسماء المنشورة في البيانات. اي فورا تعين `gov_namespace` يجب ان توفر `gov_contract_id`، ويجب ان يظهر مساحة الاسم في مجموعة `protected_namespaces` للبيان. يتم إرسال الرفض `RegisterSmartContractCode` بدون هذه البيانات الوصفية عندما تكون محمية مفعلة.
- يفترض وجود وجود زجاجي للتركيبة `(namespace, contract_id, code_hash, abi_hash)`؛ والإخفاق في التحقق من صحة إثبات NotPermited.

وقت تشغيل الخطاف
- يمكن لـ البيان الخاص بالمسار إعلان `hooks.runtime_upgrade` لبوابة تعليمات تعليمات التشغيل (`ProposeRuntimeUpgrade`, `ActivateRuntimeUpgrade`, `CancelRuntimeUpgrade`).
- وجود هوك:
  - `allow` (bool, افتراضي `true`): عندما يكون `false` يتم رفض جميع تعليمات ترقية وقت التشغيل.
  - `require_metadata` (bool, افتراضي `false`): يتطلب إدخال البيانات الوصفية بواسطة `metadata_key`.
  - `metadata_key` (سلسلة): اسم البيانات الوصفية الذي يفرضه هوك. الافتراضي `gov_upgrade_id` عندما تكون البيانات الوصفية مطلوبة او هناك قائمة السماح.
  - `allowed_ids` (مصفوفة من السلاسل): القائمة المسموح بها اختياري لقيم البيانات الوصفية (بعد القطع). يرفض عندما لا تكون القيمة المدرجة.
- عندما يكون الخطاف موجودا، يفرض القبول في الطابور على البيانات الوصفية قبل الدخول الشرعي للطابور. البيانات الوصفية سكل، القيم الفارغة، او القيم المسموح بها تنتج خطا NotPermit حطتميا.
- تتابع النتائج التليميترية عبر `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}`.
-المعاملات التي تفي بالخطاف يجب أن تتضمن البيانات الوصفية `gov_upgrade_id=<value>` (او الاختيار في البيان) مع أي موافقات مدققين مطلوبين بواسطة النصاب الخاص بالبيان.

نقطة النهاية
- POST `/v1/gov/protected-namespaces` - يطبق `gov_protected_namespaces` مباشرة على العقدة.
  - الطلب: { "مساحات الأسماء": ["apps"، "system"] }
  -الرد: { "موافق": صحيح، "مطبق": 1 }
  - آراء: مخصص للادارة/الاختبار؛ يلزم رمز API إذا كان مضبوطا. للانتاج، يفضل ارسال موقعة مع `SetParameter(Custom)`.

مساعدات CLI
-`iroha --output-format text app gov deploy audit --namespace apps [--contains calc --hash-prefix deadbeef]`
  - تأتي مثيلات العقود لمساحة الاسم ويتحقق من:
    - Torii يخزن bytecode لكل `code_hash`، وان دايجست Blake2b-32 يطابق `code_hash`.
    - البيان المخزن تحت `/v1/contracts/code/{code_hash}` لقيم `code_hash` و`abi_hash` متطابقة.
    -يوجد صيغه رقمية للتركيبة `(namespace, contract_id, code_hash, abi_hash)` مستحيلة بنفس معرف التجزئة الذي تستخدمه العقدة.
  - تقرير يخرج JSON يحتوي على `results[]` لكل عقد (القضايا، ملخصات البيان/الكود/الاقتراح) بالإضافة إلى ملخص سطر واحد الا اذا تم قبضه (`--no-summary`).
  - مفيد لدقيق مساحات الأسماء المحمية او التحقق من تدفقات النشر الخاضعة للرقابة.
-`iroha app gov deploy-meta --namespace apps --contract-id calc.v1 [--approver i105... --approver i105...]`
  - يصدر هيكل JSON لمستخدم البيانات الوصفية عند نشر النشر إلى مساحات الأسماء المحمية، بما في ذلك `gov_manifest_approvers` الاختيارية لمتطلبات المعركة النصاب القانوني للبيان.
- `iroha app gov vote --mode zk --referendum-id <id> --proof-b64 <b64> [--owner i105... --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]` — تلميحات القفل مطلوبة عندما يكون `min_bond_amount > 0`، وأي مجموعة تلميحات محمية يجب أن تضيف `owner` و`amount` و`duration_blocks`.
  - التحقق من صحة معرفات الحساب الأساسية، والتحقق من تلميحات الإبطال ذات 32 بايت، ودمج التلميحات في `public_inputs_json` (مع `--public <path>` للتجاوزات الإضافية).
  - يُشتق المبطل من التزام الإثبات (الإدخال العام) بالإضافة إلى `domain_tag` و`chain_id` و`election_id`؛ تم التحقق من صحة `--nullifier` مقابل الدليل عند تقديمه.
  - الملخص في سطر واحد الان يعرض `fingerprint=<hex>` حطميا مشغلا من `CastZkBallot` المشفر مع تلميحات المفككة (`owner`, `amount`, `duration_blocks`, `direction` عند توفيرها).
  - ردود CLI تضع تعليقات على `tx_instructions[]` مع `payload_fingerprint_hex` بالاضافة الى ملء مفكوكة كي تتحقق الادوات التالية من الهيكل بدون اعادة تنفيذ فك ترميز Norito.
  - توفير تلميحات للـ قفل مسموح للعقدة باصدار احداث `LockCreated`/`LockExtended` للـ ZK ballots بمجرد ان تسجل القيم نفسها.
-`iroha app gov vote --mode plain --referendum-id <id> --owner i105... --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - الـ aliases `--lock-amount`/`--lock-duration-blocks` احترام اسماء الأعلام الخاصة بـ ZK للسكربتات .
  - نتيجة الملخص يعكس `vote --mode zk` إضافة بصمة الإصبع للتعليمات المشفرة وحقول الاقتراع المؤقتة (`owner`, `amount`, `duration_blocks`, `direction`)، لتكيد سريعاً قبل توقيع الهيكل.

قائمة المثيلات
- GET `/v1/gov/instances/{ns}` - يسرد مثيلات العقد العضوية لمساحة الاسم.
  - معلمات الاستعلام:
    - `contains`: التصفية حسب السلسلة الفرعية من `contract_id` (حساس لحالة الأحرف)
    - `hash_prefix`: التصفية حسب المبادئ السداسية لـ `code_hash_hex` (أحرف صغيرة)
    - `offset` (الافتراضي 0)، `limit` (الافتراضي 100، الحد الأقصى 10_000)
    - `order`: واحد من `cid_asc` (افتراضي)، `cid_desc`، `hash_asc`، `hash_desc`
  -الرد: { "مساحة الاسم": "ns", "instances": [{ "contract_id": "...", "code_hash_hex": "..." }, ...], "total": N, "offset": n, "limit": m }
  - SDK المساعد: `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) أو `ToriiClient.list_governance_instances_typed("apps", ...)` (Python).

فتح المسح (المشغل/التدقيق)
- احصل على `/v1/gov/unlocks/stats`
  -الرد: { "height_current": H, "expired_locks_now": n, "referenda_with_expired": m, "last_sweep_height": S }
  - ملاحظات: `last_sweep_height` تعكس ارتفاع بلوك تم مسح أقفال منتهية وتخزينها. `expired_locks_now` يحسب عبر المسح الأرشيفي بقفل `expiry_height <= height_current`.
- المشاركة `/v1/gov/ballots/zk-v1`
  - الطلب (نمط DTO v1):
    {
      "السلطة": "i105..."،
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "...؟",
      "election_id": "ref-1",
      "الواجهة الخلفية": "halo2/ipa"،
      "envelope_b64": "AAECAwQ=",
      "root_hint": "0x...64hex؟",
      "مالك": "i105...؟",
      "nullifier": "blake2b32:...64hex؟"
    }
  -الرد: { "ok": صحيح، "مقبول": صحيح، "tx_instructions": [{...}] }

- المشاركة `/v1/gov/ballots/zk-v1/ballot-proof` (الميزة: `zk-ballot`)
  - يقبل JSON `BallotProof` مباشرة ويعيد هيكل `CastZkBallot`.
  - الطلب:
    {
      "السلطة": "i105..."،
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "...؟",
      "election_id": "ref-1",
      "الاقتراع": {
        "الواجهة الخلفية": "halo2/ipa"،
        "envelope_bytes": "AAECAwQ=", //base64 لحاوية ZK1 او H2*
        "root_hint": خالية، // سلسلة سداسية اختيارية ذات 32 بايت (جذر الأهلية)
        "owner": null, // اختيار معرف الحساب عندما تلتزم بـ Owner
        "nullifier": سلسلة سداسية عشرية اختيارية ذات 32 بايت فارغة (تلميح مُبطل)
      }
    }
  -الرد:
    {
      "حسنًا": صحيح،
      "مقبول": صحيح،
      "السبب": "إنشاء هيكل عظمي للمعاملة"،
      "تعليمات_النص": [
        { "wire_id": "CastZkBallot"، "payload_hex": "..." }
      ]
    }
  - ملاحظات:
    - يقوم بربط `root_hint`/`owner`/`nullifier` الاختيارية من الاقتراع الى `public_inputs_json` لـ `CastZkBallot`.
    - يعاد ترميز bytes الخاصة بالمغلف كـ base64 في payload التعليمية.
    - الحصول على `reason` الى `submitted transaction` عندما يقدم Torii الاقتراع.
    - هذه نقطة النهاية متاحة فقط عندما تكون الميزة `zk-ballot` مفعلا.

مسار التحقق من CastZkBallot
- `CastZkBallot` ويفيك تميز برهان base64 سنشير ويرفض حمولات الفارغة او المشوهة (`BallotRejected` مع `invalid or empty proof`).
- يحل محل يحل مفتاح التحقق للاستفتاء (`vk_ballot`) او لتفعيلات ال واجب ويطلب ان يكون مسجلا موجودا و`Active` ويحمل بايتات مضمنة.
- البايتات الخاصة بمفتاح التحقق المخزن يعاد التجزئة لها عبر `hash_vk`؛ اي لا تتطابق مع الالتزام يوقف التنفيذ قبل التحقق من الحماية من إدخالات سجل مع البث (`BallotRejected` مع `verifying key commitment mismatch`).
- بايتات خاصة بالبرهان ترسل إلى مستخدم الواجهة الخلفية عبر `zk::verify_backend`؛ ضبط النصوص غير الصالحة كـ `BallotRejected` مع `invalid proof` وفشلت التعليمية بشكل حتمي.
- يجب أن يكشف الدليل عن التزام الاقتراع وجذر الأهلية كمدخلات عامة؛ يجب أن يتطابق الجذر مع `eligible_root` الخاص بالانتخاب، ويجب أن يتطابق المبطل المشتق مع أي تلميح مقدم.
- البراهين الناجح `BallotAccepted`; nullifiers المتكررة او شديدة اهلية متأخرة او تعطل القفل المستمر في انتاج اسباب الرفض المذكورة سابقا في هذا المستند.

## خلل في عدم التحكم في العداد والتباين

### سير عمل تقطيع وسجنيأتي الاجماع `Evidence` مرمزا بـ Norito عندما ينتهك م. تصل كل حمولة الى `EvidenceStore` في الذاكرة، ولم تكن معروفة يتم تجسيدها في خريطة `consensus_evidence` المدعومة بـ WSV. يتم تسجيل تسجيل الاقدم من `sumeragi.npos.reconfig.evidence_horizon_blocks` (الافتراضي `7200` بلوك) كي يبقى الارشيف محدودا، لكن رفض التسجيل للمشغلين. تحترم الأدلة الموجودة في الأفق أيضًا `sumeragi.npos.reconfig.activation_lag_blocks` (الافتراضي `1`) وتأخير التقطيع `sumeragi.npos.reconfig.slashing_delay_blocks` (الافتراضي `259200`)؛ يمكن للحوكمة إلغاء العقوبات باستخدام `CancelConsensusEvidencePenalty` قبل تطبيق القطع.

الأسلحة المعترف بها تقابل واحدا مع `EvidenceKind`; المميزات ويفترضها نموذج البيانات:

```rust
use iroha_data_model::block::consensus::EvidenceKind;

let offences = [
    EvidenceKind::DoublePrepare,
    EvidenceKind::DoubleCommit,
    EvidenceKind::InvalidQc,
    EvidenceKind::InvalidProposal,
    EvidenceKind::Censorship,
];

for (expected, kind) in offences.iter().enumerate() {
    assert_eq!(*kind as u16, expected as u16);
}
```

- **DoublePrepare/DoubleCommit** - المتابع للتوقيع على hashes متعارضة مع tuple `(phase,height,view,epoch)`.
- **InvalidQc** - قام مجمع ببث شهادة الالتزام شكله يفشل في الفحوصات الحتمية (مثلا bitmap موقع كامل).
- **InvalidProposal** - قدم قائد بلوكا يفشل في التحقق من عدم صلاحية (مثلا يكسر قاعدة السلسلة المقفلة).
- **الرقابة** — تُظهر إيصالات التقديم الموقعة معاملة لم يتم اقتراحها/الالتزام بها مطلقًا.

يمكن للمشغلين والادوات فحص واعادة بث الحمولات عبر:

- Torii: `GET /v1/sumeragi/evidence` و`GET /v1/sumeragi/evidence/count`.
- سطر الأوامر: `iroha ops sumeragi evidence list`، `... count`، و`... submit --evidence-hex <payload>`.

يجب أن يتم تصنيف البايتات الخاصة بالدليل القانوني:

1. **جمع الحمولة** قبل ان تتقادم. ارشفة بايت Norito خام مع بيانات وصفية خاصة بـ height/view.
2. **تجهيز السلطات** عبر تضمين الحمولة في الاستفتاء او تعليمة سودو (مثل `Unregister::peer`). تنفيذ عملية التحقق من الحمولة؛ الأدلة المشوهة او القديمة ترفض حتمياً.
3. **جدولة طوبولوجيا المتابعة** حتى لا يتأخر المتسابق المتخلف عن العودة فورا. المدنية النموذجية تضع `SetParameter(Sumeragi::NextMode)` و`SetParameter(Sumeragi::ModeActivationHeight)` مع قائمة محدث.
4. **تدقيق النتائج** عبر `/v1/sumeragi/evidence` و`/v1/sumeragi/status` ودائما ان عداد الأدلة اختراق وان ال تور لتحكم الازالة.

### السلسلة الاجماعية

تشمل الاجماع ان تقوم مجموعة المتبرعين الخارجيين بانهاء بلوك الحد قبل ان تبدا المجموعة الجديدة بالاقتران. لنفترض أن وقت التشغيل الأساسي عبارة عن مقترحات مزدوجة:

- يجب ان يتم الالتزام بـ `SumeragiParameter::NextMode` و`SumeragiParameter::ModeActivationHeight` في **نفس البلوك**. يجب ان يكون `mode_activation_height` أكبر حجمًا من البلوك الذي يحمل التحديث، بما في ذلك ما يمنحه القدرة على بلوكا واحدا من التأخر.
- `sumeragi.npos.reconfig.activation_lag_blocks` (الافتراضي `1`) هو حارس تهيئة يمنع عمليات التسليم بدون تأخير:
- `sumeragi.npos.reconfig.slashing_delay_blocks` (الافتراضي `259200`) يؤخر خفض الإجماع حتى تتمكن الإدارة من إلغاء العقوبات قبل تطبيقها.

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```

- يعرض الـ runtime وCLI المعاملات التي تم تنظيمها عبر `/v1/sumeragi/params` و`iroha --output-format text ops sumeragi params` حتى يسمح لهم بالتحقق من ارتفاعات التفعيل وقوائم المدققين.
- يجب ان تقوم بتمتّع الـتـُحكم بما يلي:
  1. انهاء قرار الازالة (او الاستعادة) العلاج بـ الأدلة.
  2. جدولة تفاعل تهيئة مع `mode_activation_height = h_current + activation_lag_blocks`.
  3. المراقبة `/v1/sumeragi/status` حتى يتبدل `effective_consensus_mode` عند درجة الارتفاع.

اي سكربت يتحكم في نشاط المسكرين او يطبق التقطيع **يجب الا** يحاول عدم تفعيل التأخر أو يحذف اتفاقيات التسليم؛ يتم رفض تلك المعاملات وتترك الشبكة على الوضع السابق.

## اسطح التليميترية

- وجود معايير Prometheus لنشاط التنشيط:
  - `governance_proposals_status{status}` (مقياس) يتتبع تعداد المقترحات حسب الحالة.
  - `governance_protected_namespace_total{outcome}` (counter) يزيد عندما يسمح او يرفض القبول هناك ضمن مساحات الأسماء محمية.
  - `governance_manifest_activations_total{event}` (counter) سجل ادخالات البيان (`event="manifest_inserted"`) وربط مساحة الاسم (`event="instance_bound"`).
- `/status` كائن حيا `governance` يعكس عداد المقترحات، بعد عن اجمالي مساحات الأسماء المحمية، ويسرد تفعيلات البيان الطويل (مساحة الاسم، معرف العقد، الكود/تجزئة ABI، ارتفاع الكتلة، الطابع الزمني للتنشيط). يمكن للمشغلين استطلاع هذا الحقل لتكيد ان التشريعات حديثة وبوابات مساحات الاسماء المحمية مطبقة.
- قالب Grafana (`docs/source/grafana_governance_constraints.json`) وrunbook التليميترية في `telemetry.md` يوضحان كيفية ربط التنبيهات لقترحات العالقة، وتفعيلات البيان، او رفض مساحات الاسم المحمية غير المستخدمة أثناء ترقيات runtime.
---
lang: ar
direction: rtl
source: docs/portal/docs/governance/api.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

الحالة: Borrador/boceto لمرافقة مهام تنفيذ الإدارة. يمكن تغيير الأشكال أثناء التنفيذ. الحتمية والسياسة RBAC لها قيود معيارية؛ Torii يمكن تثبيت/إرسال المعاملات عند تقديم `authority` و`private_key`، على عكس العملاء الذين أنشأوا وإرسال `/transaction`.

استئناف
- جميع نقاط النهاية تعمل على تطوير JSON. بالنسبة للتدفقات التي تنتج المعاملات، تتضمن الإجابات `tx_instructions` - مجموعة واحدة أو أكثر من التعليمات التالية:
  - `wire_id`: معرف التسجيل لنوع التعليمات
  - `payload_hex`: بايتات الحمولة Norito (ست عشري)
- إذا تم تقديم `authority` و`private_key` (o `private_key` وDTOs de الاقتراع)، Torii ثابت وأرسل المعاملة وواحدة من `tx_instructions`.
- على العكس من ذلك، يقوم العملاء بتصنيع معاملة موقعة تستخدم السلطة الخاصة بهم وchain_id، ويمتلكون شركة ويحصلون على POST a `/transaction`.
- كوبرتورا دي SDK:
- بايثون (`iroha_python`): `ToriiClient.get_governance_proposal_typed` devuelve `GovernanceProposalResult` (حالة/نوع Normaliza Campos)، `ToriiClient.get_governance_referendum_typed` devuelve `GovernanceReferendumResult`، `ToriiClient.get_governance_tally_typed` devuelve `GovernanceTally`، `ToriiClient.get_governance_locks_typed` مزدوج `GovernanceLocksResult`، `ToriiClient.get_governance_unlock_stats_typed` مزدوج `GovernanceUnlockStats`، و`ToriiClient.list_governance_instances_typed` مزدوج `GovernanceInstancesPage`، إمكانية الوصول إلى نوع على كل سطح الإدارة مع أمثلة الاستخدام في README.
- عميل Python البسيط (`iroha_torii_client`): `ToriiClient.finalize_referendum` و `ToriiClient.enact_proposal` يطور الحزم من النوع `GovernanceInstructionDraft` (يتضمن النمط `tx_instructions` من Torii)، ضروري دليل Parseo JSON عند استخدام البرامج النصية Componen Flujos Finalize/Enact.
- JavaScript (`@iroha/iroha-js`): يعرض `ToriiClient` أنواع المساعدة للمقترحات، والاستفتاءات، والفرز، والأقفال، وفتح الإحصائيات، والآن `listGovernanceInstances(namespace, options)` مع نقاط نهاية المجلس (`getGovernanceCouncilCurrent`، `governanceDeriveCouncilVrf`، `governancePersistCouncil`, `getGovernanceCouncilAudit`) حتى يتمكن عملاء Node.js من صفحة `/v1/gov/instances/{ns}` وإجراء التدفقات المستجيبة بواسطة VRF جنبًا إلى جنب مع قائمة مثيلات العقود الموجودة.

نقاط النهاية

- المشاركة `/v1/gov/proposals/deploy-contract`
  - التماس (JSON):
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
  - الرد (JSON):
    { "ok": صحيح، "proposal_id": "...64hex"، "tx_instructions": [{ "wire_id": "..."، "payload_hex": "..." }] }
  - التحقق من صحة: العقد الأساسية `abi_hash` لـ `abi_version` تم توفيرها وإعادة ضبطها. بالنسبة إلى `abi_version = "v1"`، القيمة المتوقعة هي `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))`.

API للعقود (نشر)
- المشاركة `/v1/contracts/deploy`
  - طلب: { "authority": "i105..."، "private_key": "..."، "code_b64": "..." }
  - النسبة: حساب `code_hash` لجسم البرنامج IVM و`abi_hash` للرأس `abi_version`، وإرسال `RegisterSmartContractCode` (البيان) و`RegisterSmartContractBytes` (بايت `.to` كاملة) بالاسم `authority`.
  - الرد: { "ok": true، "code_hash_hex": "..."، "abi_hash_hex": "..." }
  - العلاقة:
    - احصل على `/v1/contracts/code/{code_hash}` -> قم بإعادة البيان المتراكم
    - احصل على `/v1/contracts/code-bytes/{code_hash}` -> devuelve `{ code_b64 }`
- المشاركة `/v1/contracts/instance`
  - طلب: { "authority": "i105..."، "private_key": "..."، "namespace": "apps"، "contract_id": "calc.v1"، "code_b64": "..." }
  - التوافق: فتح الرمز الثانوي المقدم وتنشيط الخريطة `(namespace, contract_id)` عبر `ActivateContractInstance`.
  - الرد: { "ok": true، "namespace": "apps"، "contract_id": "calc.v1"، "code_hash_hex": "..."، "abi_hash_hex": "..." }

خدمة الاسم المستعار
- المشاركة `/v1/aliases/voprf/evaluate`
  - طلب: { "blinded_element_hex": "..." }
  - الرد: { "evaluated_element_hex": "...128hex"، "backend": "blake2b512-mock" }
    - `backend` يعكس تنفيذ المُقيم. الشجاعة الفعلية: `blake2b512-mock`.
  - الملاحظات: المُقيم هو المحدد الذي يطبق Blake2b512 مع فصل السيادة `iroha.alias.voprf.mock.v1`. تم تصميم أدوات الاختبار حتى يتم توصيل خط أنابيب الإنتاج VOPRF إلى Iroha.
  - الأخطاء: HTTP `400` عند إدخال نموذج سداسي عشري خاطئ. Torii قم بإنشاء مظروف Norito `ValidationFail::QueryFailed::Conversion` مع رسالة خطأ في وحدة فك التشفير.
- المشاركة `/v1/aliases/resolve`
  - التماس: { "الاسم المستعار": "GB82 WEST 1234 5698 7654 32" }
  - الرد: { "الاسم المستعار": "GB82WEST12345698765432"، "account_id": "i105..."، "index": 0، "source": "iso_bridge" }
  - الملاحظات: مطلوب التدريج المرحلي لجسر ISO في وقت التشغيل (`[iso_bridge.account_aliases]` en `iroha_config`). Torii تطبيع الاسم المستعار لإزالة المساحات والتحرك إلى أقصى حد قبل البحث. قم بإعادة النظر في 404 عندما يكون الاسم المستعار غير موجود و 503 عندما يتم إلغاء تأهيل جسر ISO لوقت التشغيل.
- المشاركة `/v1/aliases/resolve_index`
  - طلب: { "الفهرس": 0 }
  - الرد: { "index": 0, "alias": "GB82WEST12345698765432"، "account_id": "i105..."، "source": "iso_bridge" }
  - الملاحظات: يتم تعيين مؤشرات الأسماء المستعارة بطريقة محددة حسب ترتيب التكوين (يعتمد على 0). يمكن للعملاء تخزين الردود مؤقتًا في وضع عدم الاتصال لإنشاء عمليات الاستماع لأحداث المصادقة على الأسماء المستعارة.

Tope de tamano de codego
- المعلمة المخصصة: `max_contract_code_bytes` (JSON u64)
  - التحكم في الحجم الأقصى المسموح به (بالبايت) لتخزين تشفير العقود عبر السلسلة.
  - الافتراضي: 16 ميجابايت. تم إعادة عقد العقد `RegisterSmartContractBytes` عندما تتجاوز الصورة `.to` الجزء العلوي مع وجود خطأ انتهاك ثابت.
  - يمكن للمشغلين ضبط `SetParameter(Custom)` مع `id = "max_contract_code_bytes"` وحمولة رقمية.

- المشاركة `/v1/gov/ballots/zk`
  - التماس: { "authority": "i105..."، "private_key": "...؟"، "chain_id": "..."، "election_id": "e1"، "proof_b64": "..."، "public": {...} }
  - الرد: { "ok": صحيح، "مقبول": صحيح، "tx_instructions": [{...}] }
  - ملاحظات:
    - عند تضمين إدخالات الدائرة العامة `owner` و`amount` و`duration_blocks` واختبار التحقق من تكوين VK، تقوم العقدة بإنشاء أو توسيع حظر الإدارة لـ `election_id` مع هذا هو `owner`. الاتجاه الدائم الخفي (`unknown`); فقط قم بتحديث المبلغ/انتهاء الصلاحية. تعتبر عمليات الإلغاء رتيبة: يتم زيادة المبلغ وانتهاء الصلاحية فقط (العقدة تنطبق على الحد الأقصى (المبلغ، المبلغ السابق) والحد الأقصى (انتهاء الصلاحية، انتهاء الصلاحية السابق)).
    - تتم عمليات إلغاء ZK التي تهدف إلى تقليل المبلغ أو انتهاء الصلاحية عند إعادة تشغيل الخادم باستخدام التشخيص `BallotRejected`.
    - يجب استدعاء تنفيذ العقد `ZK_VOTE_VERIFY_BALLOT` قبل إرفاقه `SubmitBallot`; يطلب المضيفون مزلاجًا مرة واحدة فقط.

- المشاركة `/v1/gov/ballots/plain`
  - التماس: { "authority": "i105..."، "private_key": "...؟"، "chain_id": "..."، "referendum_id": "r1"، "owner": "i105..."، "amount": "1000"، "duration_blocks": 6000، "direction": "Aye|Nay|امتناع" }
  - الرد: { "ok": صحيح، "مقبول": صحيح، "tx_instructions": [{...}] }
  - الملاحظات: عمليات الإلغاء هي مجرد تمديد - لا يمكن لبطاقة اقتراع جديدة تقليل المبلغ أو انتهاء صلاحية الحظر الموجود. يجب أن يكون `owner` مساوٍ لسلطة المعاملة. الحد الأدنى للمدة هو `conviction_step_blocks`.

- المشاركة `/v1/gov/finalize`
  - التماس: { "referendum_id": "r1"، "proposal_id": "...64hex"، "authority": "i105...؟"، "private_key": "...؟" }
  - الإجابة: { "ok": true، "tx_instructions": [{ "wire_id": "...FinalizeReferendum"، "payload_hex": "..." }] }
  - التأثير على السلسلة (الفعلي): نشر اقتراح نشر صحيح يتم إدراجه على الحد الأدنى `ContractManifest` مع الفصل `code_hash` مع `abi_hash` المتوقع ووضع علامة على العرض كما تم تفعيله. إذا كان هناك بيان لـ `code_hash` مع `abi_hash` مختلف، فسيتم إعادة إصداره.
  - ملاحظات:
    - لانتخاب ZK، يجب أن تتصل مسارات العقد `ZK_VOTE_VERIFY_TALLY` قبل التنفيذ `FinalizeElection`؛ يطلب المضيفون مزلاجًا مرة واحدة فقط. `FinalizeReferendum` تمت الإشارة إليه من قبل ZK حتى يتم الانتهاء من قائمة الانتخابات.
    - يُصدر القفل التلقائي في `h_end` الموافقة/الرفض فقط للإشارة إلى عادي؛ تظل مراجع ZK مغلقة حتى يتم إرسال نتيجة نهائية وتنفيذ `FinalizeReferendum`.
    - يتم استخدام تسويات الإقبال منفردًا بالموافقة+الرفض؛ الامتناع عن التصويت لا يستحق الإقبال.

- المشاركة `/v1/gov/enact`
  - طلب: { "proposal_id": "...64hex"، "preimage_hash": "...64hex؟"، "window": { "lower": 0، "upper": 0}؟، "authority": "i105...؟"، "private_key": "...؟" }
  - الإجابة: { "ok": true، "tx_instructions": [{ "wire_id": "...EnactReferendum"، "payload_hex": "..." }] }
  - الملاحظات: Torii أرسل المعاملة المؤكدة عندما يتم تقديمها `authority`/`private_key`؛ على العكس من ذلك، يجب أن يكون هناك أسلوب لجعل العملاء يثبتون ويحسدونهم. الصورة المسبقة اختيارية ومعلوماتية بالفعل.

- احصل على `/v1/gov/proposals/{id}`
  - المسار `{id}`: معرف العرض السداسي (64 حرفًا)
  - الرد: { "تم العثور عليه": منطقي، "اقتراح": { ... }؟ }- احصل على `/v1/gov/locks/{rid}`
  - المسار `{rid}`: معرف سلسلة الاستفتاء
  - الرد: { "تم العثور عليه": منطقي، "referendum_id": "rid"، "locks": { ... }؟ }

- احصل على `/v1/gov/council/current`
  - الرد: { "epoch": N, "members": [{ "account_id": "..." }, ...] }
  - الملاحظات: devuelve el Council المستمر عندما يكون موجودًا؛ على النقيض من ذلك، يتم الحصول على استجابة محددة باستخدام أصول الحصة المكونة والمظلة (تشير إلى مواصفات VRF التي تختبر VRF في الحياة إذا استمرت على السلسلة).

- POST `/v1/gov/council/derive-vrf` (الميزة: gov_vrf)
  - التماس: { "committee_size": 21، "epoch": 123؟ , "candidates": [{ "account_id": "..."، "variant": "Normal|Small"، "pk_b64": "..."، "proof_b64": "..." }, ...] }
  - الملاءمة: التحقق من اختبار VRF لكل مرشح مقابل الإدخال الكنسي المشتق من `chain_id` و`epoch` ومنارة تجزئة الكتلة الأخيرة؛ ترتيب بايتات الإخراج من خلال أدوات كسر التعادل؛ devuelve los top `committee_size` miembros. لا استمرار.
  - الرد: { "epoch": N, "members": [{ "account_id": "..." } ...], "total_candidates": M, "تم التحقق": K }
  - الملاحظات: عادي = pk en G1، إثبات en G2 (96 بايت). صغير = pk en G2، إثبات en G1 (48 بايت). المدخلات منفصلة حسب المساحة وتتضمن `chain_id`.

### افتراضيات الحاكم (iroha_config `gov.*`)

مجلس الاستجابة المستخدم لـ Torii عندما لا توجد قائمة مستمرة يتم إعدادها عبر `iroha_config`:

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

تجاوزات المعادلتين الداخليتين:

```
GOV_VK_BACKEND=halo2/ipa
GOV_VK_NAME=ballot_v1
GOV_PARLIAMENT_COMMITTEE_SIZE=21
GOV_PARLIAMENT_TERM_BLOCKS=43200
GOV_PARLIAMENT_MIN_STAKE=1
GOV_PARLIAMENT_ELIGIBILITY_ASSET_ID=SORA#stake
GOV_ALIAS_TEU_MINIMUM=0
GOV_ALIAS_FRONTIER_TELEMETRY=true
```

`parliament_committee_size` يحد من عدد أعضاء الاستجابة عندما لا يستمر مجلس القش، `parliament_term_blocks` يحدد طول العصر المستخدم لاشتقاق البذور (`epoch = floor(height / term_blocks)`)، `parliament_min_stake` يطبق الحد الأدنى من الحصة (في الوحدات) الحد الأدنى) حول الأصول المؤهلة، ثم حدد `parliament_eligibility_asset_id` بحيث يتم مسح رصيد الأصول من خلال إنشاء مجموعة المرشحين.

لا يحتوي التحقق من إدارة VK على تجاوز: يتطلب التحقق من بطاقات الاقتراع دائمًا التحقق الرئيسي `Active` مع البايتات المضمنة، ولا يعتمد الموجهون على تبديل الاختبار لحذف التحقق.

RBAC
- يتطلب التشغيل على السلسلة الحصول على إذن:
  - المقترحات: `CanProposeContractDeployment{ contract_id }`
  - أوراق الاقتراع: `CanSubmitGovernanceBallot{ referendum_id }`
  - التشريع: `CanEnactGovernance`
  - إدارة المجلس (المستقبل): `CanManageParliament`

مساحات الأسماء المحمية
- المعلمة المخصصة `gov_protected_namespaces` (مصفوفة سلاسل JSON) قادرة على بوابة القبول للنشر في قوائم مساحات الأسماء.
- يجب أن يشتمل العملاء على مفاتيح بيانات وصفية للمعاملات لنشر موجهة إلى مساحات أسماء محمية:
  - `gov_namespace`: كائن مساحة الاسم (على سبيل المثال، "apps")
  - `gov_contract_id`: معرف العقد المنطقي داخل مساحة الاسم
- `gov_manifest_approvers`: مصفوفة JSON الاختيارية لمعرفات حسابات التحقق. عندما يعلن بيان المسار عن نصاب قانوني أكبر من واحد، يتطلب القبول تفويضًا بالمعاملة أكثر من الحسابات المدرجة لإرضاء نصاب البيان.
- يعرض القياس عن بعد مقاييس الدخول عبر `governance_manifest_admission_total{result}` ليقوم المشغلون بتمييز مخرجات المسارات `missing_manifest` و`non_validator_authority` و`quorum_rejected` و`protected_namespace_rejected` و`runtime_hook_rejected`.
- يعرض القياس عن بعد مسار التنفيذ عبر `governance_manifest_quorum_total{outcome}` (القيم `satisfied` / `rejected`) حتى يقوم المشغلون بمراجعة التوقعات الخاطئة.
- تنطبق الممرات على القائمة المسموح بها لمساحات الأسماء المنشورة في بياناتها. يجب أن تتطابق أي معاملة تظهر `gov_namespace` مع `gov_contract_id`، ويجب أن تظهر مساحة الاسم في المجموعة `protected_namespaces` من البيان. يتم حفظ البيانات الوصفية الخاصة بالأشخاص `RegisterSmartContractCode` عند تأهيل الحماية.
- القبول يعني أن هناك اقتراحًا حكوميًا تم تفعيله للمجموع `(namespace, contract_id, code_hash, abi_hash)`؛ على العكس من ذلك، فإن التحقق من الصحة يفشل مع خطأ غير مسموح به.

خطافات ترقية وقت التشغيل
- يمكن لبيانات المسار الإعلان عن `hooks.runtime_upgrade` للتحكم في تعليمات ترقية وقت التشغيل (`ProposeRuntimeUpgrade`، `ActivateRuntimeUpgrade`، `CancelRuntimeUpgrade`).
- مجال الخطاف:
  - `allow` (منطقي، الافتراضي `true`): عندما يكون `false`، يتم إعادة تأهيل جميع تعليمات ترقية وقت التشغيل.
  - `require_metadata` (منطقي، افتراضي `false`): يعرض إدخال بيانات التعريف المحددة بواسطة `metadata_key`.
  - `metadata_key` (سلسلة): اسم البيانات الوصفية المطبق بواسطة الخطاف. الافتراضي `gov_upgrade_id` عندما يتطلب بيانات وصفية أو قائمة مسموح بها.
  - `allowed_ids` (صفيف السلاسل): القائمة المسموح بها لقيم البيانات الوصفية الاختيارية (تقطيع إضافي). Rechaza cuando el valor provisto no esta listado.
- عندما يتم تقديم الخطاف، يتم تطبيق سياسة البيانات الوصفية قبل إجراء المعاملة بين الكولا. تؤدي البيانات الوصفية الخاطئة أو القيم الفارغة أو القيم المستقبلية للقائمة المسموح بها إلى حدوث خطأ محدد غير مسموح به.
- نتائج القياس عن بعد عبر `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}`.
- يجب أن تتضمن المعاملات التي تكمل الخطاف البيانات الوصفية `gov_upgrade_id=<value>` (أو المفتاح المحدد للبيان) بالإضافة إلى أي موافقة على المصادقات المطلوبة للنصاب القانوني للبيان.

نقطة نهاية الراحة
- POST `/v1/gov/protected-namespaces` - يتم تطبيق `gov_protected_namespaces` مباشرة في العقدة.
  - طلب: { "مساحات الأسماء": ["apps"، "system"] }
  - الرد: { "موافق": صحيح، "مطبق": 1 }
  - الملاحظات: فكرة للإدارة/الاختبار؛ يتطلب رمز API إذا تم تكوينه. للإنتاج، اختر إرسال معاملة مؤكدة مع `SetParameter(Custom)`.

مساعدين CLI
-`iroha --output-format text app gov deploy audit --namespace apps [--contains calc --hash-prefix deadbeef]`
  - الحصول على مثيلات العقد لمساحة الاسم والتحقق مما يلي:
    - Torii رمز البايت كود لكل `code_hash`، ويتوافق مع Blake2b-32 مع `code_hash`.
    - البيان الموضح أدناه `/v1/contracts/code/{code_hash}` يبلغ عن القيم `code_hash` y `abi_hash` بالصدفة.
    - توجد اقتراح إدارة تم سنه لـ `(namespace, contract_id, code_hash, abi_hash)` مشتق من نفس تجزئة معرف الاقتراح الذي يستخدم العقدة.
  - قم بإصدار تقرير JSON مع `results[]` بموجب عقد (قضايا، سير ذاتية للبيان/الكود/الاقتراح) بالإضافة إلى استئناف لخط إطلاق يتفوق (`--no-summary`).
  - استخدام لمراجعة مساحات الأسماء المحمية أو التحقق من تدفقات نشر الضوابط من أجل الإدارة.
-`iroha app gov deploy-meta --namespace apps --contract-id calc.v1 [--approver i105... --approver i105...]`
  - قم بإنشاء مجموعة JSON من البيانات التعريفية المستخدمة عند نشر مساحات أسماء محمية، بما في ذلك `gov_manifest_approvers` الاختيارية لإرضاء قواعد نصاب البيان.
- `iroha app gov vote --mode zk --referendum-id <id> --proof-b64 <b64> [--owner i105... --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]` — تلميحات القفل إلزامية عندما `min_bond_amount > 0`، وأي مجموعة من التلميحات المقدمة تشمل `owner`، و`amount`، و`duration_blocks`.
  - التحقق من صحة معرفات الحساب الأساسية، والتحقق من تلميحات الإبطال ذات 32 بايت، ودمج التلميحات في `public_inputs_json` (مع `--public <path>` للتجاوزات الإضافية).
  - يُشتق المبطل من التزام الإثبات (الإدخال العام) بالإضافة إلى `domain_tag` و`chain_id` و`election_id`؛ تم التحقق من صحة `--nullifier` مقابل الدليل عند تقديمه.
  - السيرة الذاتية للخط الآن تعرض `fingerprint=<hex>` محددًا مشتقًا من `CastZkBallot` مشفرًا مع تلميحات مفكوكة (`owner`، `amount`، `duration_blocks`، `direction` عندما يتم توفيره).
  - تشير إجابات CLI إلى `tx_instructions[]` مع `payload_fingerprint_hex` وهي عبارة عن مجالات تم فك تشفيرها حتى تتمكن الأدوات المتلقية للمعلومات من التحقق من الإجراء بدون إعادة تنفيذ فك التشفير Norito.
  - قم بإثبات تلميحات القفل التي تسمح للعقدة بإصدار أحداث `LockCreated`/`LockExtended` لبطاقات الاقتراع ZK مرة واحدة حيث توضح الدائرة نفس القيم.
-`iroha app gov vote --mode plain --referendum-id <id> --owner i105... --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - يشير الاسم المستعار `--lock-amount`/`--lock-duration-blocks` إلى أسماء علامات ZK لمساواة البرامج النصية.
  - نتيجة استئناف المراجعة `vote --mode zk` بما في ذلك بصمة التعليمات المقننة ومجالات الاقتراع المقروءة (`owner`، `amount`، `duration_blocks`، `direction`)، تأكيد الاستلام قم بالسرعة قبل تثبيت الهيكل.

قائمة الحالات
- الحصول على `/v1/gov/instances/{ns}` - قائمة مثيلات عقد التنشيط لمساحة الاسم.
  - معلمات الاستعلام:
    - `contains`: مرشح للسلسلة الفرعية لـ `contract_id` (حساس لحالة الأحرف)
    - `hash_prefix`: مرشح سداسي عشري مسبقًا لـ `code_hash_hex` (أحرف صغيرة)
    - `offset` (الافتراضي 0)، `limit` (الافتراضي 100، الحد الأقصى 10_000)
    - `order`: واحد من `cid_asc` (افتراضي)، `cid_desc`، `hash_asc`، `hash_desc`
  - الرد: { "مساحة الاسم": "ns"، "المثيلات": [{ "contract_id": "..."، "code_hash_hex": "..." }، ...]، "total": N، "offset": n، "limit": m }
  - SDK المساعد: `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) أو `ToriiClient.list_governance_instances_typed("apps", ...)` (Python).حاجز إلغاء القفل (المشغل/مراجعة الحسابات)
- احصل على `/v1/gov/unlocks/stats`
  - الإجابة: { "height_current": H، "expired_locks_now": n، "referenda_with_expired": m، "last_sweep_height": S }
  - الملاحظات: `last_sweep_height` تشير إلى ارتفاع الكتلة الأحدث عندما تنتهي صلاحية الأقفال. `expired_locks_now` يتم مسح سجلات القفل باستخدام `expiry_height <= height_current`.
- المشاركة `/v1/gov/ballots/zk-v1`
  - طلب (DTO estilo v1):
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
  - الرد: { "ok": صحيح، "مقبول": صحيح، "tx_instructions": [{...}] }

- المشاركة `/v1/gov/ballots/zk-v1/ballot-proof` (الميزة: `zk-ballot`)
  - اقبل ملف JSON `BallotProof` مباشرة وقم بتحويله إلى `CastZkBallot`.
  - التماس:
    {
      "السلطة": "i105..."،
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "...؟",
      "election_id": "ref-1",
      "الاقتراع": {
        "الواجهة الخلفية": "halo2/ipa"،
        "envelope_bytes": "AAECAwQ=", // حاوية قاعدة 64 ZK1 أو H2*
        "root_hint": خالية، // سلسلة سداسية اختيارية ذات 32 بايت (جذر الأهلية)
        "owner": null, // معرف الحساب اختياري عندما تقوم الدائرة بتسوية المالك
        "nullifier": سلسلة سداسية عشرية اختيارية ذات 32 بايت فارغة (تلميح مُبطل)
      }
    }
  - الرد:
    {
      "حسنًا": صحيح،
      "مقبول": صحيح،
      "السبب": "إنشاء هيكل عظمي للمعاملة"،
      "تعليمات_النص": [
        { "wire_id": "CastZkBallot"، "payload_hex": "..." }
      ]
    }
  - ملاحظات:
    - الخادم Mapea `root_hint`/`owner`/`nullifier` اختياري من الاقتراع إلى `public_inputs_json` لـ `CastZkBallot`.
    - تتم إعادة تشفير بايتات المغلف باسم Base64 لحمولة التعليمات.
    - يتم الرد على `reason` من خلال `submitted transaction` عندما يتم إرسال Torii إلى بطاقة الاقتراع.
    - نقطة النهاية هذه متاحة فقط عندما تكون الميزة `zk-ballot` مؤهلة.

طريقة التحقق من CastZkBallot
- `CastZkBallot` يقوم بفك تشفير اختبار base64 وتوفير الحمولات الفارغة أو الخاطئة (`BallotRejected` مع `invalid or empty proof`).
- يجيب المضيف على مفتاح التحقق من الاقتراع من الاستفتاء (`vk_ballot`) أو الإعدادات الافتراضية ويطلب وجود السجل، حتى `Active`، ويرفع البايتات المضمنة.
- يتم إعادة تجزئة وحدات البايت من المفتاح الذي تم التحقق منه باستخدام `hash_vk`؛ أي إلغاء تسوية يوقف التنفيذ المسبق للتحقق من الحماية ضد إدخالات التسجيل المزورة (`BallotRejected` مع `verifying key commitment mismatch`).
- يتم إرسال وحدات البايت التجريبية إلى الواجهة الخلفية المسجلة عبر `zk::verify_backend`؛ تظهر النسخ غير الصالحة مثل `BallotRejected` مع `invalid proof` وستفشل التعليمات بالتأكيد.
- يجب أن يكشف الدليل عن التزام الاقتراع وجذر الأهلية كمدخلات عامة؛ يجب أن يتطابق الجذر مع `eligible_root` الخاص بالانتخاب، ويجب أن يتطابق المبطل المشتق مع أي تلميح مقدم.
- Pruebas Exitosas emiten `BallotAccepted`؛ تؤدي عوامل الإلغاء المكررة أو جذور الأهلية القديمة أو تراجعات القفل إلى إنتاج أسباب الاسترداد الموجودة سابقًا في هذا المستند.

## سلوك خاطئ ومتفق عليه

### عملية التقطيع والسجن

يتم إرسال الموافقة على `Evidence` المشفر إلى Norito عندما يتم التحقق من صحة البروتوكول. يتم تجميع كل الحمولة على `EvidenceStore` في الذاكرة، وإذا لم تكن موجودة مسبقًا، فسيتم إنشاؤها على الخريطة `consensus_evidence` التي تمت معالجتها بواسطة WSV. يتم إعادة تسجيل السجلات السابقة إلى `sumeragi.npos.reconfig.evidence_horizon_blocks` (وحدات `7200` الافتراضية) لتمكين الأرشيف من حفظه، ولكن يتم تسجيله للمشغلين. تحترم الأدلة الموجودة في الأفق أيضًا `sumeragi.npos.reconfig.activation_lag_blocks` (الافتراضي `1`) وتأخير التقطيع `sumeragi.npos.reconfig.slashing_delay_blocks` (الافتراضي `259200`)؛ يمكن للحوكمة إلغاء العقوبات باستخدام `CancelConsensusEvidencePenalty` قبل تطبيق القطع.

يتم تعيين الجرائم التي تم التعرف عليها على `EvidenceKind`؛ التمييزيون عبارة عن مؤسسات ومُحسّنة وفقًا لنموذج البيانات:

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

- **DoublePrepare/DoubleCommit** - المدقق الثابت للتجزئات المتعارضة لنفس المجموعة `(phase,height,view,epoch)`.
- **InvalidQc** - مجمع ثرثرة وشهادة التزام بصيغة سقوط الشيكات المحددة (على سبيل المثال، صورة نقطية للشركات الفارغة).
- **اقتراح غير صالح** - قائد يقترح كتلة تؤدي إلى فشل البنية الهيكلية (على سبيل المثال، فتح نظام السلسلة المقفلة).
- **الرقابة** — تُظهر إيصالات التقديم الموقعة معاملة لم يتم اقتراحها/الالتزام بها مطلقًا.

يمكن للمشغلين والأدوات فحص الحمولات وإعادة بثها من خلال:

- Torii: `GET /v1/sumeragi/evidence` و`GET /v1/sumeragi/evidence/count`.
- سطر الأوامر: `iroha ops sumeragi evidence list`، `... count`، و`... submit --evidence-hex <payload>`.

يجب أن تقوم الإدارة بتسليم بايتات الأدلة مثل اختبار Canonica:

1. **استعادة الحمولة** قبل أن تخرج. أرشفة وحدات البايت Norito الأساسية جنبًا إلى جنب مع البيانات الوصفية للارتفاع/العرض.
2. **إعداد العقوبة** تضمين الحمولة في استفتاء أو تعليمات Sudo (على سبيل المثال، `Unregister::peer`). يؤدي الإخراج إلى إعادة صحة الحمولة؛ الأدلة غير شكلية أو رانسيا يتم طلبها بشكل حاسم.
3. **برمجة طوبولوجيا التسلسل** حتى لا يتمكن مخالف التحقق من إعادة فتح الوسيط. يتم تضمين الأنواع الرئيسية من `SetParameter(Sumeragi::NextMode)` و`SetParameter(Sumeragi::ModeActivationHeight)` مع تحديث القائمة.
4. **تدقيق النتائج** عبر `/v1/sumeragi/evidence` و`/v1/sumeragi/status` للتأكد من أن جهاز حفظ الأدلة متقدم وأن الإدارة تنطبق على الإزالة.

### توثيق التوافق المشترك

يضمن الإجماع أن مجموعة المصادقين تبرز بشكل نهائي كتلة الحدود قبل أن تمثل المجموعة الجديدة مؤيدًا. وقت التشغيل يفرض النظام عبر المعلمات الباريدوس:

- يجب التأكيد على `SumeragiParameter::NextMode` و`SumeragiParameter::ModeActivationHeight` في **الحظر نفسه**. `mode_activation_height` يجب أن يكون على وجه التحديد أكبر من أن ارتفاع الكتلة التي يحملها التحديث، يوفر كتلة تأخير على الأقل.
- `sumeragi.npos.reconfig.activation_lag_blocks` (الافتراضي `1`) هو حارس التكوين الذي يسمح بعمليات التسليم مع تأخر اللون:
- `sumeragi.npos.reconfig.slashing_delay_blocks` (الافتراضي `259200`) يؤخر خفض الإجماع حتى تتمكن الإدارة من إلغاء العقوبات قبل تطبيقها.

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```

- يعرض وقت التشغيل وCLI المعلمات التي تم تنظيمها عبر `/v1/sumeragi/params` و`iroha --output-format text ops sumeragi params`، حتى يؤكد المشغلون ارتفاعات التنشيط وقوائم المدققين.
- أتمتة الإدارة بشكل دائم:
  1. قم بإتمام قرار الإزالة (أو إعادة التثبيت) بناءً على الأدلة.
  2. قم بتغطية إعادة تكوين التسلسل باستخدام `mode_activation_height = h_current + activation_lag_blocks`.
  3. قم بمراقبة `/v1/sumeragi/status` حتى يتم تغيير `effective_consensus_mode` إلى الارتفاع المتوقع.

أي نص برمجي يستخدم عمليات التحقق أو تطبيق القطع **لا داعي** لمحاولة التنشيط مع تأخير ذلك وحذف معلمات التسليم؛ يتم إعادة صياغة هذه المعاملات وتسليمها باللون الأحمر في الوضع السابق.

## سطوح القياس عن بعد

- مقاييس Prometheus المصدرة لنشاط الإدارة:
  - `governance_proposals_status{status}` (المقياس) يعرض بيانات الحالة.
  - `governance_protected_namespace_total{outcome}` (العداد) يزداد عندما يسمح لك قبول مساحات الأسماء المحمية أو إعادة نشرها.
  - `governance_manifest_activations_total{event}` (العداد) تسجيلات إدراج البيان (`event="manifest_inserted"`) وروابط مساحة الاسم (`event="instance_bound"`).
- يتضمن `/status` كائن `governance` الذي يعكس بيانات المقترحات، ويبلغ إجمالي مساحات الأسماء المحمية، ويسرد أحدث التنشيطات (مساحة الاسم، ومعرف العقد، وتجزئة الكود/ABI، وارتفاع الكتلة، والطابع الزمني للتنشيط). يمكن للمشغلين الرجوع إلى هذا المجال للتأكد من أن عمليات النشر قد تم تحديثها وأن بوابات مساحات الأسماء المحمية قابلة للتطبيق.
- يتم عرض لوحة Grafana (`docs/source/grafana_governance_constraints.json`) ودليل القياس عن بعد في `telemetry.md` كتنبيهات كابلية لنشر هجمات أو عمليات تنشيط خاطئة أو عمليات إلغاء مساحات الأسماء المحمية أثناء ترقيات وقت التشغيل.
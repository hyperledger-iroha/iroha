---
lang: ar
direction: rtl
source: docs/portal/docs/governance/api.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

الحالة: rascunho/esboco para acompanhar as tarefas de تنفيذ الحكم. كأشكال يمكن أن يتم تنفيذها أثناء التنفيذ. الحتمية والسياسة RBAC لها قيود معيارية؛ يمكن لـ Torii إجراء تحويلات/مقياس فرعي عند `authority` و`private_key`، وهو ما يتناقض مع تصميم العملاء والقياس الفرعي لـ `/transaction`.

فيساو جيرال
- جميع نقاط نهاية نظام التشغيل retornam JSON. بالنسبة للتدفقات التي تنتج المعاملات، حيث تتضمن الإجابات `tx_instructions` - مجموعة من أو أكثر من التعليمات التالية:
  - `wire_id`: معرف التسجيل لنوع التعليمات
  - `payload_hex`: بايتات الحمولة Norito (ست عشري)
- إذا كان `authority` و`private_key` للمرشحين (أو `private_key` في DTOs لبطاقات الاقتراع)، Torii، قم بإجراء التحويل وإرجاع `tx_instructions`.
- في حالة العكس، يقوم العملاء بتوقيع المعاملات الموقعة باستخدام السلطة الخاصة بهم e chain_id، depois assinam e fazem POST para `/transaction`.
- كوبرتورا دي SDK:
- بايثون (`iroha_python`): `ToriiClient.get_governance_proposal_typed` retorna `GovernanceProposalResult` (حالة/نوع Normaliza Campos)، `ToriiClient.get_governance_referendum_typed` retorna `GovernanceReferendumResult`، `ToriiClient.get_governance_tally_typed` retorna `GovernanceTally`، إرجاع `ToriiClient.get_governance_locks_typed` إرجاع `GovernanceLocksResult` و`ToriiClient.get_governance_unlock_stats_typed` إرجاع `GovernanceUnlockStats` وإرجاع `ToriiClient.list_governance_instances_typed` `GovernanceInstancesPage`، إمكانية الوصول إلى كل سطح الإدارة مع أمثلة الاستخدام لا التمهيدي.
- عميل Python المستوى (`iroha_torii_client`): `ToriiClient.finalize_referendum` و`ToriiClient.enact_proposal` إعادة تدوير الحزم إلى `GovernanceInstructionDraft` (المغلفة أو المضمنة `tx_instructions` إلى Torii)، دليل التحليل المتنقل de JSON عندما تقوم البرامج النصية بتكوين التدفقات النهائية/التفعيل.
- JavaScript (`@iroha/iroha-js`): يعرض `ToriiClient` أنواع المساعدة للمقترحات، والاستفتاءات، والإحصاءات، والأقفال، وفتح الإحصائيات، وما إلى ذلك `listGovernanceInstances(namespace, options)` ونقاط النهاية التي تقوم بها (`getGovernanceCouncilCurrent`، `governanceDeriveCouncilVrf`، `governancePersistCouncil`، `getGovernanceCouncilAudit`) حتى يتمكن عملاء Node.js من وضع الصفحة `/v1/gov/instances/{ns}` وتوجيه سير العمل عبر VRF جنبًا إلى جنب مع قائمة مثيلات العقد الموجودة.

نقاط النهاية

- المشاركة `/v1/gov/proposals/deploy-contract`
  - المتطلبات (JSON):
    {
      "مساحة الاسم": "التطبيقات"،
      "contract_id": "my.contract.v1",
      "code_hash": "blake2b32:..." | "...64 سداسيًا"،
      "abi_hash": "blake2b32:..." | "...64 سداسيًا"،
      "abi_version": "1"،
      "نافذة": { "سفلي": 12345، "علوي": 12400}،
      "السلطة": "ih58...؟",
      "مفتاح_خاص": "...؟"
    }
  - الرد (JSON):
    { "ok": صحيح، "proposal_id": "...64hex"، "tx_instructions": [{ "wire_id": "..."، "payload_hex": "..." }] }
  - التحقق: os nos canonizam `abi_hash` para o `abi_version` fornecido e rejeitam divergencias. لـ `abi_version = "v1"`، أو القيمة المتوقعة و`hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))`.

API للعقود (نشر)
- المشاركة `/v1/contracts/deploy`
  - المتطلبات: { "authority": "ih58..."، "private_key": "..."، "code_b64": "..." }
  - التوافق: حساب `code_hash` من جسم البرنامج IVM و`abi_hash` من الرأس `abi_version`، بعد أن يخضع `RegisterSmartContractCode` (البيان) و`RegisterSmartContractBytes` (بايتات كاملة `.to`) بالاسم `authority`.
  - الرد: { "ok": true، "code_hash_hex": "..."، "abi_hash_hex": "..." }
  - العلاقة:
    - احصل على `/v1/contracts/code/{code_hash}` -> إعادة إنشاء البيان
    - احصل على `/v1/contracts/code-bytes/{code_hash}` -> ريتورنا `{ code_b64 }`
- المشاركة `/v1/contracts/instance`
  - المتطلبات: { "authority": "ih58..."، "private_key": "..."، "مساحة الاسم": "apps"، "contract_id": "calc.v1"، "code_b64": "..." }
  - التوافق: نشر الرمز الثانوي للتنفيذ والتفعيل فورًا أو التعيين إلى `(namespace, contract_id)` عبر `ActivateContractInstance`.
  - الإجابة: { "ok": true، "namespace": "apps"، "contract_id": "calc.v1"، "code_hash_hex": "..."، "abi_hash_hex": "..." }

خدمة الاسم المستعار
- المشاركة `/v1/aliases/voprf/evaluate`
  - المتطلبات: { "blinded_element_hex": "..." }
  - الإجابة: { "evaluated_element_hex": "...128hex"، "backend": "blake2b512-mock" }
    - `backend` يعيد تنفيذ التطبيق. الشجاعة الحقيقية: `blake2b512-mock`.
  - الملاحظات: مدقق حتمي وهمي يتم تطبيقه على Blake2b512 مع فصل السيادة `iroha.alias.voprf.mock.v1`. تم تصميم أدوات الاختبار لخط أنابيب الإنتاج VOPRF ليكون متكاملاً مع Iroha.
  - الأخطاء: إدخال HTTP `400` غير صحيح. Torii يعيد المغلف Norito `ValidationFail::QueryFailed::Conversion` كرسالة خطأ في وحدة فك الترميز.
- المشاركة `/v1/aliases/resolve`
  - المتطلبات: { "الاسم المستعار": "GB82 WEST 1234 5698 7654 32" }
  - الرد: { "الاسم المستعار": "GB82WEST12345698765432"، "account_id": "ih58..."، "index": 0، "source": "iso_bridge" }
  - الملاحظات: اطلب مرحلة تشغيل جسر ISO (`[iso_bridge.account_aliases]` في `iroha_config`). Torii تسوية الأسماء المستعارة وإزالة النطاقات وتحويلها إلى أكبر عدد من الأسماء المستعارة قبل إجراء البحث. ارجع 404 عندما يكون الاسم المستعار موجودًا و 503 عندما يكون جسر ISO لوقت التشغيل معطلاً.
- المشاركة `/v1/aliases/resolve_index`
  - المتطلبات: { "الفهرس": 0 }
  - الرد: { "index": 0, "alias": "GB82WEST12345698765432"، "account_id": "ih58..."، "source": "iso_bridge" }
  - الملاحظات: مؤشرات الاسم المستعار الخاصة بصيغة التحديد من خلال ترتيب التكوين (على أساس 0). يمكن للعملاء تخزين الردود مؤقتًا في وضع عدم الاتصال لإنشاء سلسلة من جلسات الاستماع لأحداث التحقق من الأسماء المستعارة.

حدود حجم التشفير
- المعلمة المخصصة: `max_contract_code_bytes` (JSON u64)
  - التحكم في الحجم الأقصى المسموح به (البايتات) لتخزين تشفير العقود على السلسلة.
  - الافتراضي: 16 ميجابايت. نحن نستجيب `RegisterSmartContractBytes` عندما يتجاوز حجم الصورة `.to` الحد مع وجود خطأ ثابت.
  - يمكن للمشغلين ضبط الحمولة عبر `SetParameter(Custom)` مع `id = "max_contract_code_bytes"` والحمولة الرقمية.

- المشاركة `/v1/gov/ballots/zk`
  - المتطلبات: { "authority": "ih58..."، "private_key": "...؟"، "chain_id": "..."، "election_id": "e1"، "proof_b64": "..."، "عام": {...} }
  - الرد: { "ok": صحيح، "مقبول": صحيح، "tx_instructions": [{...}] }
  - ملاحظات:
    - عند تضمين مدخلات الدائرة العامة `owner`، و`amount`، و`duration_blocks`، وإثبات التحقق من تكوين VK، أو عدم إنشاء أو إنشاء قفل إدارة لـ `election_id` com esse `owner`. A direcao permanece oculta (`unknown`)؛ كمية المبلغ/انتهاء الصلاحية sao atualizados. إعادة التصويت رتيبة: المبلغ e انتهاء الصلاحية apenas aumentam (o لا يوجد تطبيق max(amount, prev.amount) e max(expiry, prev.expiry)).
    - إعادة التصويت لـ ZK الذي سيسمح لك بتخفيض المبلغ أو انتهاء الصلاحية بدون خادم مع التشخيص `BallotRejected`.
    - تنفيذ عقد الاتصال `ZK_VOTE_VERIFY_BALLOT` قبل تسجيل `SubmitBallot`; يستضيف المضيفون مزلاجًا واحدًا في وقت واحد.

- المشاركة `/v1/gov/ballots/plain`
  - المتطلبات: { "authority": "ih58..."، "private_key": "...؟"، "chain_id": "..."، "referendum_id": "r1"، "owner": "ih58..."، "amount": "1000"، "duration_blocks": 6000، "direction": "Aye|Nay|امتناع" }
  - الرد: { "ok": صحيح، "مقبول": صحيح، "tx_instructions": [{...}] }
  - الملاحظات: إعادة التصويت مرة واحدة فقط - بطاقة اقتراع جديدة لا يمكنها تقليل المبلغ أو انتهاء الصلاحية لقفل الوجود. يجب أن يكون `owner` بمثابة سلطة المعاملات. الحد الأدنى من دوراكاو و`conviction_step_blocks`.

- المشاركة `/v1/gov/finalize`
  - المتطلبات: { "referendum_id": "r1"، "proposal_id": "...64hex"، "authority": "ih58...؟"، "private_key": "...؟" }
  - الإجابة: { "ok": true، "tx_instructions": [{ "wire_id": "...FinalizeReferendum"، "payload_hex": "..." }] }
  - التأثير على السلسلة (السقالة الحالية): نشر اقتراح نشر معتمد داخل الحد الأدنى `ContractManifest` مع `code_hash` مع `abi_hash` المنتظر ووضع علامة على الاقتراح كما تم سنه. إذا كان البيان موجودًا لـ `code_hash` مع `abi_hash` مختلفًا، أو سنًا وتم تجديده.
  - ملاحظات:
    - بالنسبة إلى ZK، يتم تنفيذ الكاميرات بموجب `ZK_VOTE_VERIFY_TALLY` قبل التنفيذ `FinalizeElection`؛ يفرض المضيفون مزلاجًا للاستخدام الفريد. `FinalizeReferendum` يشير إلى ZK وقد تم الانتهاء من نتيجة هذا الاختيار.
    - الحفظ التلقائي في `h_end` يصدر الموافقة/الرفض فقط للرجوع إلى عادي؛ تم إغلاق مراجع ZK بشكل دائم حتى يتم إرسال القائمة النهائية وسيتم تنفيذ `FinalizeReferendum`.
    - كما checagens دي الإقبال usam apenas الموافقة + الرفض؛ الامتناع عن التصويت على الإقبال.

- المشاركة `/v1/gov/enact`
  - المتطلبات: { "proposal_id": "...64hex"، "preimage_hash": "...64hex؟"، "window": { "lower": 0، "upper": 0}؟، "authority": "ih58...؟"، "private_key": "...؟" }
  - الإجابة: { "ok": true، "tx_instructions": [{ "wire_id": "...EnactReferendum"، "payload_hex": "..." }] }
  - الملاحظات: Torii قم بإرسال رسالة نصية عندما `authority`/`private_key` sao fornecidos؛ Caso Contrario Retorna um esqueleto para Clientes assinarem e submeterem. صورة مسبقة واختيارية ومعلوماتية عالية.

- احصل على `/v1/gov/proposals/{id}`
  - المسار `{id}`: معرف الاقتراح سداسي عشري (64 حرفًا)
  - الرد: { "تم العثور عليه": منطقي، "اقتراح": { ... }؟ }

- احصل على `/v1/gov/locks/{rid}`
  - المسار `{rid}`: سلسلة معرف الاستفتاء
  - الرد: { "تم العثور عليه": منطقي، "referendum_id": "rid"، "locks": { ... }؟ }- احصل على `/v1/gov/council/current`
  - الرد: { "epoch": N, "members": [{ "account_id": "..." }, ...] }
  - الملاحظات: العودة إلى المجلس المستمر عندما يتم تقديمه؛ يتم استخلاص الحالة المعاكسة من حتمية احتياطية باستخدام أصول الحصة التي تم تكوينها والعتبات (يتم تحديدها بواسطة VRF محدد أو اختبار VRF من خلال إنتاج نفس المنتجات المستمرة على السلسلة).

- POST `/v1/gov/council/derive-vrf` (الميزة: gov_vrf)
  - المتطلبات: { "committee_size": 21، "epoch": 123؟ , "candidates": [{ "account_id": "..."، "variant": "Normal|Small"، "pk_b64": "..."، "proof_b64": "..." }, ...] }
  - التوافق: التحقق من اختبار VRF لكل مرشح ضد الإدخال الكنسي المشتق من `chain_id` و`epoch` وتجزئة الكتلة الأخيرة؛ ترتيب بايتات السعادة من قواطع التعادل؛ retorna os أعلى `committee_size` الأغشية. ناو تستمر.
  - الرد: { "epoch": N, "members": [{ "account_id": "..." } ...], "total_candidates": M, "تم التحقق": K }
  - الملاحظات: عادي = pk em G1، إثبات em G2 (96 بايت). صغير = pk em G2، إثبات em G1 (48 بايت). المدخلات منفصلة حسب المجال وتشمل `chain_id`.

### افتراضيات الإدارة (iroha_config `gov.*`)

المجلس الاحتياطي يستخدم Torii عند عدم وجود قائمة مستمرة ومعلمات عبر `iroha_config`:

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

تجاوزات البيئة المحيطة المكافئة:

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

`parliament_committee_size` يحدد عدد الأعضاء الاحتياطية التي تم إرجاعها عند استمرار المجلس، `parliament_term_blocks` يحدد اشتراك العصر المستخدم لاستخراج البذور (`epoch = floor(height / term_blocks)`)، `parliament_min_stake` تطبيق الحد الأدنى من الحصة (في الحد الأدنى من الوحدات) لا توجد أصول مؤهلة، ويتم تحديد `parliament_eligibility_asset_id` كمصدر للأصول ويتم مسحه عند إنشاء مجموعة من المرشحين.

التحقق من إدارة VK لتجاوز هذا الأمر: يتطلب التحقق من الاقتراع دائمًا وجود `Active` مع البايتات المضمنة، ويجب أن تعتمد البيئة المحيطة على تبديل الاختبار للتحقق.

RBAC
- أذونات Execucao on-chain exige:
  - المقترحات: `CanProposeContractDeployment{ contract_id }`
  - أوراق الاقتراع: `CanSubmitGovernanceBallot{ referendum_id }`
  - التشريع: `CanEnactGovernance`
  - إدارة المجلس (المستقبل): `CanManageParliament`

مساحات الأسماء المحمية
- المعلمة المخصصة `gov_protected_namespaces` (مصفوفة سلاسل JSON) قادرة على بوابة القبول لنشر قوائم مساحات الأسماء.
- يجب على العملاء تضمين بيانات وصفية للمعاملات لنشر مساحات الأسماء المحمية:
  - `gov_namespace`: مساحة الاسم alvo (على سبيل المثال، "apps")
  - `gov_contract_id`: معرف العقد المنطقي داخل مساحة الاسم
- `gov_manifest_approvers`: مصفوفة JSON الاختيارية لمعرفات حسابات التحقق. عندما يعلن بيان المسار عن النصاب القانوني الأكبر، يتطلب القبول الحصول على سلطة لإجراء المعاملات أكثر من خلال قائمة البيانات لإرضاء نصاب البيان.
- يعرض القياس عن بعد مقاييس الدخول عبر `governance_manifest_admission_total{result}` حتى يتمكن مشغلو التوزيع من قبول تسجيلات الدخول الناجحة `missing_manifest`، `non_validator_authority`، `quorum_rejected`، `protected_namespace_rejected` e `runtime_hook_rejected`.
- يعرض القياس عن بعد طريق التنفيذ عبر `governance_manifest_quorum_total{outcome}` (القيم `satisfied` / `rejected`) لكي يقوم المشغلون بمراجعة الحسابات الخاطئة.
- يتم تطبيق الممرات على القائمة المسموح بها لمساحات الأسماء المنشورة في بياناتها. أي معاملة تحدد `gov_namespace` يجب أن تكون `gov_contract_id`، ويجب أن تظهر مساحة الاسم في `protected_namespaces` في البيان. يتم إرسال `RegisterSmartContractCode` دون تسجيل البيانات الوصفية عند حمايتها.
- القبول يؤثر على وجود اقتراح للحوكمة تم إقراره من أجل المجموعة `(namespace, contract_id, code_hash, abi_hash)`؛ السبب يتعارض مع خطأ فالهاكا غير مسموح به.

خطافات ترقية وقت التشغيل
- يمكن لبيانات المسار الإعلان عن `hooks.runtime_upgrade` للحصول على تعليمات ترقية وقت التشغيل (`ProposeRuntimeUpgrade`، `ActivateRuntimeUpgrade`، `CancelRuntimeUpgrade`).
- كامبوس دو هوك:
  - `allow` (منطقي، الافتراضي `true`): عند `false`، جميع تعليمات ترقية وقت التشغيل متاحة أيضًا.
  - `require_metadata` (منطقي، الافتراضي `false`): اطلب إدخال بيانات تعريف محددة لـ `metadata_key`.
  - `metadata_key` (سلسلة): اسم خطاف البيانات الوصفية المطبق. الافتراضي `gov_upgrade_id` عندما يتم طلب البيانات الوصفية أو القائمة المسموح بها.
  - `allowed_ids` (صفيف السلاسل): القائمة المسموح بها لقيم البيانات الوصفية الاختيارية (اقتصاص apos). تمتع عندما تكتسب الشجاعة في هذه القائمة.
- عندما يتم تقديمه، يتم تطبيق سياسة البيانات الوصفية قبل إجراء أي تحويل على الملف. البيانات الوصفية الصحيحة أو القيم البيضاء أو الموجودة في القائمة المسموح بها قد تؤدي إلى خطأ حتمي غير مسموح به.
- نتائج القياس عن بعد راستريا عبر `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}`.
- المعاملات التي ترضي أو يجب أن تتضمن البيانات الوصفية `gov_upgrade_id=<value>` (أو أي بيان محدد) بالإضافة إلى الحصول على موافقة المصدقين المطلوبين على نصاب البيان.

نقطة نهاية الراحة
- POST `/v1/gov/protected-namespaces` - يتم تطبيق `gov_protected_namespaces` مباشرة لا لا.
  - المتطلبات: { "مساحات الأسماء": ["apps"، "system"] }
  - الرد: { "موافق": صحيح، "مطبق": 1 }
  - الملاحظات: موجهة إلى المشرف/الاختبار؛ اطلب رمزًا مميزًا لواجهة برمجة التطبيقات (API) إذا تم تكوينها. لإنتاج، اختر إرسال عملية نقل عبر `SetParameter(Custom)`.

مساعدين CLI
-`iroha --output-format text app gov deploy audit --namespace apps [--contains calc --hash-prefix deadbeef]`
  - ابحث عن مثيلات العقد لمساحة الاسم واعرض ما يلي:
    - Torii رمز البايت كود لكل `code_hash`، ويتوافق ملخص Blake2b-32 مع `code_hash`.
    - البيان المُخزَّن في `/v1/contracts/code/{code_hash}` هو تقرير `code_hash` و`abi_hash`.
    - يوجد اقتراح للحوكمة تم سنه لـ `(namespace, contract_id, code_hash, abi_hash)` مشتق من نفس تجزئة الاقتراح-id que o no usa.
  - قم بإصدار علاقة JSON مع `results[]` بموجب عقد (القضايا، واستئنافات البيان/الكود/الاقتراح) بالإضافة إلى ملخص آخر على الأقل (`--no-summary`).
  - الاستفادة من مساحات الأسماء المحمية أو التحقق من تدفقات النشر التي يتم التحكم فيها من خلال الإدارة.
-`iroha app gov deploy-meta --namespace apps --contract-id calc.v1 [--approver ih58... --approver ih58...]`
  - قم بإنشاء مجموعة بيانات تعريف JSON صغيرة تستخدم في عمليات نشر المقياس الفرعي في مساحات الأسماء المحمية، بما في ذلك `gov_manifest_approvers` الاختيارية لتلبية متطلبات النصاب القانوني للبيان.
- `iroha app gov vote --mode zk --referendum-id <id> --proof-b64 <b64> [--owner ih58... --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]` — يتم قفل تلميحات القفل عند `min_bond_amount > 0`، وأي مجموعة من التلميحات المطلوبة يجب أن تتضمن `owner` و`amount` و`duration_blocks`.
  - التحقق من صحة معرفات الحساب الأساسية، والتحقق من تلميحات الإبطال ذات 32 بايت، ودمج التلميحات في `public_inputs_json` (مع `--public <path>` للتجاوزات الإضافية).
  - يُشتق المبطل من التزام الإثبات (الإدخال العام) بالإضافة إلى `domain_tag` و`chain_id` و`election_id`؛ تم التحقق من صحة `--nullifier` مقابل الدليل عند تقديمه.
  - يعرض ملخص هذا الخط `fingerprint=<hex>` المشتق الحتمي من `CastZkBallot` المشفر جنبًا إلى جنب مع تلميحات فك التشفير (`owner`، `amount`، `duration_blocks`، `direction` عندما يتعلق الأمر بالقتل).
  - كرد على CLI Anotam `tx_instructions[]` مع `payload_fingerprint_hex` تم فك تشفير المزيد من المجالات حتى تتمكن الأجهزة النهائية من التحقق من عدم فك تشفير Norito.
  - تتيح تلميحات القفل عدم إصدار أحداث `LockCreated`/`LockExtended` لبطاقات الاقتراع ZK كما أن الدائرة تعرض نفس القيم.
-`iroha app gov vote --mode plain --referendum-id <id> --owner ih58... --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - الأسماء المستعارة `--lock-amount`/`--lock-duration-blocks` تحدد أسماء أعلام ZK لشريط النص.
  - رسالة الخلاصة `vote --mode zk` تتضمن بصمة الإصبع للتعليمات المقننة ومجالات الاقتراع القانونية (`owner`، `amount`، `duration_blocks`، `direction`)، تأكيد العرض قم بالسرعة قبل البدء أو التسلق.

قائمة الحالات
- الحصول على `/v1/gov/instances/{ns}` - قائمة مثيلات العقود الجديدة لمساحة الاسم.
  - معلمات الاستعلام:
    - `contains`: مرشح للسلسلة الفرعية لـ `contract_id` (حساس لحالة الأحرف)
    - `hash_prefix`: مرشح للبادئة السداسية `code_hash_hex` (أحرف صغيرة)
    - `offset` (الافتراضي 0)، `limit` (الافتراضي 100، الحد الأقصى 10_000)
    - `order`: أم `cid_asc` (افتراضي)، `cid_desc`، `hash_asc`، `hash_desc`
  - الإجابة: { "مساحة الاسم": "ns"، "المثيلات": [{ "contract_id": "..."، "code_hash_hex": "..." }، ...]، "total": N، "offset": n، "limit": m }
  - SDK المساعد: `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) أو `ToriiClient.list_governance_instances_typed("apps", ...)` (Python).

Varredura de unlocks (المشغل/مراجعة الحسابات)
- احصل على `/v1/gov/unlocks/stats`
  - الإجابة: { "height_current": H، "expired_locks_now": n، "referenda_with_expired": m، "last_sweep_height": S }
  - الملاحظات: `last_sweep_height` يعكس ارتفاع الكتلة الأحدث عند انتهاء صلاحية الأقفال المتنوعة والمستمرة. `expired_locks_now` ويتم حسابه عن طريق إعادة تسجيل القفل مع `expiry_height <= height_current`.
- المشاركة `/v1/gov/ballots/zk-v1`
  - المتطلبات (DTO estilo v1):
    {
      "السلطة": "ih58..."،
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "...؟",
      "election_id": "ref-1",
      "الواجهة الخلفية": "halo2/ipa"،
      "envelope_b64": "AAECAwQ=",
      "root_hint": "0x...64hex؟",
      "مالك": "ih58...؟",
      "nullifier": "blake2b32:...64hex؟"
    }
  - الرد: { "ok": صحيح، "مقبول": صحيح، "tx_instructions": [{...}] }- المشاركة `/v1/gov/ballots/zk-v1/ballot-proof` (الميزة: `zk-ballot`)
  - قم بتسجيل الدخول إلى JSON `BallotProof` مباشرة وإعادته إلى `CastZkBallot`.
  - المتطلبات:
    {
      "السلطة": "ih58..."،
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "...؟",
      "election_id": "ref-1",
      "الاقتراع": {
        "الواجهة الخلفية": "halo2/ipa"،
        "envelope_bytes": "AAECAwQ=", // base64 للحاوية ZK1 أو H2*
        "root_hint": خالية، // سلسلة سداسية اختيارية ذات 32 بايت (جذر الأهلية)
        "owner": null, // معرف الحساب اختياري عند اختراق الدائرة للمالك
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
    - خادم الخريطة `root_hint`/`owner`/`nullifier` خيار الاقتراع لـ `public_inputs_json` في `CastZkBallot`.
    - يتم إعادة تشفير البايتات مثل Base64 لحمولة التعليمات.
    - تم الرد على `reason` لـ `submitted transaction` عند إرسال Torii إلى بطاقة الاقتراع.
    - نقطة النهاية هذه متوفرة، لذا فهي متاحة عندما تكون الميزة `zk-ballot` جاهزة.

طريقة التحقق من CastZkBallot
- `CastZkBallot` يقوم بفك تشفير قاعدة 64 التجريبية ويعيد الحمولات النافعة أو المشوهة (`BallotRejected` مع `invalid or empty proof`).
- قرر المضيف إجراء عملية التحقق من الاقتراع من خلال الاستفتاء (`vk_ballot`) أو الإعدادات الافتراضية للإدارة والطلب من وجود السجل، وهذا `Active` ومحتوى البايتات المضمنة.
- وحدات البايت التي تم التحقق منها تم تخزينها من خلال إعادة التجزئة مع `hash_vk`؛ أي عدم تطابق في الالتزام يوقف التنفيذ قبل التحقق من أجل الحماية من إدخالات السجلات المزورة (`BallotRejected` مع `verifying key commitment mismatch`).
- وحدات البايت التجريبية من خلال تسجيل الواجهة الخلفية عبر `zk::verify_backend`؛ تظهر النسخ غير الصالحة مثل `BallotRejected` مع `invalid proof` والتعليمات الخاطئة لشكل الحتمية.
- يجب أن يكشف الدليل عن التزام الاقتراع وجذر الأهلية كمدخلات عامة؛ يجب أن يتطابق الجذر مع `eligible_root` الخاص بالانتخاب، ويجب أن يتطابق المبطل المشتق مع أي تلميح مقدم.
- بروفاس بيم سوسيدداس إميتيم `BallotAccepted`؛ تستمر عمليات الإبطال المكررة، أو جذور الأهلية القديمة، أو تراجع القفل في الإنتاج على النحو الذي يؤدي إلى إزالة الرفض الموجود الموضح سابقًا في هذا المستند.

## ماو سلوك المصادقة والتوافق

### تدفق التقطيع والسجن

قم بإصدار `Evidence` المشفر في Norito عند التحقق من صحة البروتوكول. يتم وضع كل حمولة على `EvidenceStore` في الذاكرة ويتم تحريرها وتصنيعها على خريطة `consensus_evidence` التي تمت معالجتها بواسطة WSV. تم تسجيل المزيد من السجلات التي `sumeragi.npos.reconfig.evidence_horizon_blocks` (الكتل `7200` الافتراضية) للحفاظ على الملف المحدود، بالإضافة إلى الطلب والتسجيل للمشغلين. تحترم الأدلة الموجودة في الأفق أيضًا `sumeragi.npos.reconfig.activation_lag_blocks` (الافتراضي `1`) وتأخير التقطيع `sumeragi.npos.reconfig.slashing_delay_blocks` (الافتراضي `259200`)؛ يمكن للحوكمة إلغاء العقوبات باستخدام `CancelConsensusEvidencePenalty` قبل تطبيق القطع.

تم التعرف على Ofensas Mapeiam um-para-um para `EvidenceKind`؛ نموذج بيانات التمييز والضرائب:

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

- **DoublePrepare/DoubleCommit** - أو التحقق من صحة التجزئة المتضاربة لنفس المجموعة `(phase,height,view,epoch)`.
- **InvalidQc** - ثرثرة المجمع للحصول على شهادة التزام بطريقة صحيحة في التحقق من المحددات (على سبيل المثال، الصورة النقطية للموقعين).
- **اقتراح غير صالح** - زعيم يدعم الكتلة التي تؤدي إلى التحقق من صحتها (على سبيل المثال، تعديل السلسلة المقفلة).
- **الرقابة** — تُظهر إيصالات التقديم الموقعة معاملة لم يتم اقتراحها/الالتزام بها مطلقًا.

يمكن للمشغلين والأدوات فحص الحمولات وإعادة بثها عبر:

- Torii: `GET /v1/sumeragi/evidence` و`GET /v1/sumeragi/evidence/count`.
- سطر الأوامر: `iroha ops sumeragi evidence list`، `... count`، و`... submit --evidence-hex <payload>`.

يجب أن تقوم الإدارة بترجمة وحدات البايت من الأدلة مثل الدليل الكنسي:

1. **التخلص من الحمولة** قبل انتهاء الصلاحية. قم بحفظ وحدات البايت Norito المجمعة مع البيانات الوصفية للارتفاع/العرض.
2. **إعداد عقوبة** تضمين أو حمولة في استفتاء أو تعليمات سودو (على سبيل المثال، `Unregister::peer`). إعادة تنفيذ الحمولة؛ الأدلة غير صحيحة أو قديمة ويتم تجديدها بشكل حاسم.
3. **جدولة المرافقة** حتى لا يتمكن مدقق الأشعة من العودة على الفور. تم تسجيل أنواع التدفقات `SetParameter(Sumeragi::NextMode)` و`SetParameter(Sumeragi::ModeActivationHeight)` مع تحديث القائمة.
4. **تدقيق النتائج** عبر `/v1/sumeragi/evidence` و`/v1/sumeragi/status` لضمان أن مراقب الأدلة مقدم وأن الإدارة ستطبق على إزالة.

### تسلسل متفق عليه

يضمن الإجماع أن مجموعة مصادقي السعادة تنهي كتلة الحدود قبل أن يأتي الاتحاد الجديد لصالحهم. يتم تطبيق وقت التشغيل مرة أخرى عبر المعلمات الباريدوس:

- `SumeragiParameter::NextMode` و`SumeragiParameter::ModeActivationHeight` يجب أن يتم تأكيده بدون **mesmo bloco**. يجب أن يكون `mode_activation_height` أكبر بكثير من أن ارتفاع الكتلة الذي يحمل التحديث، يؤدي إلى أقل كتلة من التأخير.
- `sumeragi.npos.reconfig.activation_lag_blocks` (الافتراضي `1`) وحماية التكوين الذي يعيق عمليات التسليم مع التأخر الصفري:
- `sumeragi.npos.reconfig.slashing_delay_blocks` (الافتراضي `259200`) يؤخر خفض الإجماع حتى تتمكن الإدارة من إلغاء العقوبات قبل تطبيقها.

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```

- يعرض وقت التشغيل وCLI المعلمات التي تم تنظيمها عبر `/v1/sumeragi/params` و`iroha --output-format text ops sumeragi params`، حتى يؤكد المشغلون ارتفاعات التنشيط وقوائم المدققين.
- يجب أن يكون الحكم الذاتي دائمًا:
  1. قم بإتمام قرار الإزالة (أو إعادة الدمج) المقدم من خلال الأدلة.
  2. قم بتسجيل إعادة تكوين المرافقة باستخدام `mode_activation_height = h_current + activation_lag_blocks`.
  3. مراقب `/v1/sumeragi/status` A18NI00000300X مبزل على ارتفاع منتظر.

كل نص يقوم بتدوير المدققين أو تطبيق القطع **لا يجب**** إيقاف التأخر الصفري أو حذف معلمات التسليم؛ هذه المعاملات هي عبارة عن تجديدات وإخراجها من الوضع السابق.

## سطوح القياس عن بعد

- مقاييس تصدير Prometheus لمستوى الإدارة:
  - `governance_proposals_status{status}` (المقياس) يلتقط رسائل الاقتراحات الخاصة بالحالة.
  - `governance_protected_namespace_total{outcome}` (العداد) يزداد عند السماح بمساحات الأسماء المحمية أو إعادة نشرها.
  - `governance_manifest_activations_total{event}` (العداد) سجل إدراج البيان (`event="manifest_inserted"`) وروابط مساحة الاسم (`event="instance_bound"`).
- `/status` يتضمن كائنًا `governance` يعرض كرسائل مقترحة، تتعلق بجميع مساحات الأسماء المحمية وقائمة أحدث البيانات (مساحة الاسم، معرف العقد، تجزئة الكود/ABI، ارتفاع الكتلة، الطابع الزمني للتنشيط). يمكن للمشغلين استشارة هذا المجال للتأكد من أن التشريعات التي تم تحديثها تظهر وأن بوابات مساحات الأسماء المحمية يتم إرسالها إلى التطبيق.
- نموذج Grafana (`docs/source/grafana_governance_constraints.json`) ودليل القياس عن بعد في `telemetry.md` يحتوي على تنبيهات بسيطة لمقترحات مقترحات أو عبارات تنشيطية أو حذف مساحات الأسماء المحمية أثناء ترقيات وقت التشغيل.
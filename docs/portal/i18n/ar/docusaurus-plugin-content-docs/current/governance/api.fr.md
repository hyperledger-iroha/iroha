---
lang: ar
direction: rtl
source: docs/portal/docs/governance/api.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

الحالة: brouillon/esquisse لمرافقة مهام تنفيذ الحوكمة. يمكن أن تتغير الأشكال عند التنفيذ. إن الحتمية والسياسة في RBAC هي قيود معيارية؛ Torii يمكن أن يوقع/يقوم بالمعاملات عندما `authority` و`private_key` يتم توفيرها، دون أن يتم بناء العملاء ويصلون إلى `/transaction`.

لا شيء
- جميع نقاط النهاية مطلوبة من JSON. من أجل التدفق الذي ينتج المعاملات، تتضمن الردود `tx_instructions` - لوحة من التعليمات أو أكثر من ذلك:
  - `wire_id`: معرف التسجيل لنوع التعليمات
  - `payload_hex`: بايتات الحمولة Norito (ست عشري)
- Si `authority` et `private_key` sont fournis (ou `private_key` sur les DTO de ballots)، Torii يوقع ويكمل المعاملة ويراجع عندما `tx_instructions`.
- حسنًا، يقوم العملاء بتجميع معاملة موقعة باستخدام السلطة وسلسلة_id، ثم التوقيع وPOST مقابل `/transaction`.
- كوفيرتور SDK:
- بايثون (`iroha_python`): `ToriiClient.get_governance_proposal_typed` renvoie `GovernanceProposalResult` (تطبيع حالة/نوع les champs)، `ToriiClient.get_governance_referendum_typed` renvoie `GovernanceReferendumResult`، `ToriiClient.get_governance_tally_typed` renvoie `GovernanceTally`، `ToriiClient.get_governance_locks_typed` رنفو `GovernanceLocksResult`، `ToriiClient.get_governance_unlock_stats_typed` رينفو `GovernanceUnlockStats`، و`ToriiClient.list_governance_instances_typed` رينفوي `GovernanceInstancesPage`، فرض نوع وصول على كل سطح الإدارة باستخدام أمثلة الاستخدام في README.
- عميل Python Leger (`iroha_torii_client`): `ToriiClient.finalize_referendum` و`ToriiClient.enact_proposal` يستعيد أنواع الحزم `GovernanceInstructionDraft` (التي تشمل المفتاح `tx_instructions` من Torii)، مما يمنع تحليل JSON مانويل عندما تتكون البرامج النصية من التدفق النهائي/التفعيل.
- JavaScript (`@iroha/iroha-js`): يعرض `ToriiClient` أنواع المساعدين للمقترحات والاستفتاءات والسجلات والأقفال وفتح الإحصائيات وصيانة `listGovernanceInstances(namespace, options)` بالإضافة إلى مجلس نقاط النهاية (`getGovernanceCouncilCurrent`، `governanceDeriveCouncilVrf`، `governancePersistCouncil`، `getGovernanceCouncilAudit`) لتمكين عملاء Node.js من إرسال صفحات `/v1/gov/instances/{ns}` وتوجيه سير العمل VRF بالتوازي مع قائمة مثيلات العقود الموجودة.

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
      "السلطة": "ih58...؟",
      "مفتاح_خاص": "...؟"
    }
  - الرد (JSON):
    { "ok": صحيح، "proposal_id": "...64hex"، "tx_instructions": [{ "wire_id": "..."، "payload_hex": "..." }] }
  - التحقق من الصحة: تم إدخال البيانات الأساسية `abi_hash` من أجل `abi_version` وإلغاء التناقضات. بالنسبة إلى `abi_version = "v1"`، فإن قيمة الحضور هي `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))`.

عقود واجهة برمجة التطبيقات (النشر)
- المشاركة `/v1/contracts/deploy`
  - الطلب: { "authority": "ih58..."، "private_key": "..."، "code_b64": "..." }
  - السلوك: حساب `code_hash` من مجموعة البرنامج IVM و`abi_hash` من خلال `abi_version`، ومن ثم `RegisterSmartContractCode` (بيان) و`RegisterSmartContractBytes` (اكتملت وحدات البايت `.to`) لـ `authority`.
  - الرد: { "ok": صحيح، "code_hash_hex": "..."، "abi_hash_hex": "..." }
  - الكذب:
    - احصل على `/v1/contracts/code/{code_hash}` -> راجع المخزون
    - احصل على `/v1/contracts/code-bytes/{code_hash}` -> renvoie `{ code_b64 }`
- المشاركة `/v1/contracts/instance`
  - الطلب: { "authority": "ih58..."، "private_key": "..."، "namespace": "apps"، "contract_id": "calc.v1"، "code_b64": "..." }
  - السلوك: نشر الرمز الثانوي وتفعيل رسم الخرائط `(namespace, contract_id)` عبر `ActivateContractInstance` على الفور.
  - الرد: { "ok": true، "namespace": "apps"، "contract_id": "calc.v1"، "code_hash_hex": "..."، "abi_hash_hex": "..." }

خدمة الأسماء المستعارة
- المشاركة `/v1/aliases/voprf/evaluate`
  - الطلب: { "blinded_element_hex": "..." }
  - الرد: { "evaluated_element_hex": "...128hex"، "backend": "blake2b512-mock" }
    - `backend` يعكس تنفيذ المقيم. القيمة الفعلية: `blake2b512-mock`.
  - ملاحظات: أداة تقييم وهمية لتحديد Blake2b512 مع فصل المجال `iroha.alias.voprf.mock.v1`. لنبدأ باختبار الإنتاج حتى يعتمد خط أنابيب الإنتاج VOPRF على Iroha.
  - الأخطاء: HTTP `400` على شكل إدخال سداسي عشري خاطئ. Torii قم بإعادة ظرف Norito `ValidationFail::QueryFailed::Conversion` مع رسالة خطأ في وحدة فك التشفير.
- المشاركة `/v1/aliases/resolve`
  - الطلب: { "الاسم المستعار": "GB82 WEST 1234 5698 7654 32" }
  - الرد: { "الاسم المستعار": "GB82WEST12345698765432"، "account_id": "ih58..."، "index": 0، "source": "iso_bridge" }
  - ملاحظات: يتطلب التدريج المرحلي لجسر ISO في وقت التشغيل (`[iso_bridge.account_aliases]` في `iroha_config`). Torii يقوم بتطبيع الأسماء المستعارة عن طريق إزالة المسافات ويواصل العمل بشكل كبير قبل البحث. العودة 404 إذا كان الاسم المستعار غائبًا و 503 إذا تم تعطيل جسر ISO لوقت التشغيل.
- المشاركة `/v1/aliases/resolve_index`
  - الطلب: { "الفهرس": 0 }
  - الرد: { "index": 0, "alias": "GB82WEST12345698765432"، "account_id": "ih58..."، "source": "iso_bridge" }
  - ملاحظات: يتم تعيين فهرس الأسماء المستعارة تحديدًا حسب ترتيب التكوين (يعتمد على 0). يمكن للعملاء إجراء تخزين مؤقت خارج الخط لإنشاء ممرات التدقيق لأحداث المصادقة على الأسماء المستعارة.

Cap de taille de code
- المعلمة المخصصة: `max_contract_code_bytes` (JSON u64)
  - التحكم في الحد الأقصى المسموح به من الحجم (بالبايت) لمخزون رمز العقد على السلسلة.
  - الافتراضي: 16 ميجابايت. تتكرر الأحداث `RegisterSmartContractBytes` عندما يتجاوز حجم الصورة `.to` الغطاء بسبب خطأ ثابت.
  - يمكن للمشغلين الضبط عبر `SetParameter(Custom)` مع `id = "max_contract_code_bytes"` وحمولة رقمية.

- المشاركة `/v1/gov/ballots/zk`
  - الطلب: { "authority": "ih58..."، "private_key": "...؟"، "chain_id": "..."، "election_id": "e1"، "proof_b64": "..."، "public": {...} }
  - الرد: { "موافق": صحيح، "مقبول": صحيح، "tx_instructions": [{...}] }
  - ملاحظات:
    - عندما تشتمل المدخلات العامة للدائرة على `owner` و`amount` و`duration_blocks`، ويتم التحقق مسبقًا من تكوين VK، ثم يتم إنشاء قفل حوكمة لـ `election_id` مع هذا `owner`. الاتجاه يكمن في ذاكرة التخزين المؤقت (`unknown`); المبلغ/انتهاء الصلاحية لم يخطئ يومًا. إعادة التصويت هي رتيبة: المبلغ وانتهاء الصلاحية ليسا خطًا إضافيًا (le noeud applique max(amount, prev.amount) et max(expiry, prev.expiry)).
    - يتم رفض عمليات إعادة التصويت ZK التي تؤدي إلى تقليل المبلغ أو انتهاء الصلاحية بواسطة التشخيص `BallotRejected`.
    - تنفيذ العقد الذي يجب تقديمه `ZK_VOTE_VERIFY_BALLOT` قبل ملف `SubmitBallot`؛ يفرض المضيفون مزلاجًا مرة واحدة فقط.

- المشاركة `/v1/gov/ballots/plain`
  - الطلب: { "authority": "ih58..."، "private_key": "...؟"، "chain_id": "..."، "referendum_id": "r1"، "owner": "ih58..."، "amount": "1000"، "duration_blocks": 6000، "direction": "Aye|Nay|امتناع" }
  - الرد: { "موافق": صحيح، "مقبول": صحيح، "tx_instructions": [{...}] }
  - ملاحظات: يتم تمديد عمليات إعادة التصويت بشكل منفصل - لا يمكن أن تؤدي بطاقة الاقتراع الجديدة إلى تقليل المبلغ أو انتهاء صلاحية القفل الموجود. يؤدي `owner` إلى مساواة تفويض المعاملة. الحد الأدنى من المدة هو `conviction_step_blocks`.

- المشاركة `/v1/gov/finalize`
  - الطلب: { "referendum_id": "r1"، "proposal_id": "...64hex"، "authority": "ih58...؟"، "private_key": "...؟" }
  - الرد: { "ok": true، "tx_instructions": [{ "wire_id": "...FinalizeReferendum"، "payload_hex": "..." }] }
  - التأثير على السلسلة (السقالة الفعلية): قم بتفعيل اقتراح نشر معتمد، أدخل `ContractManifest` الحد الأدنى cle `code_hash` مع l'`abi_hash`، ثم قم بتمييز الاقتراح الذي تم تفعيله. إذا كان هناك بيان موجود من أجل `code_hash` مع `abi_hash` مختلف، فسيتم رفض التشريع.
  - ملاحظات:
    - من أجل انتخابات ZK، يجب استدعاء سلاسل العقود `ZK_VOTE_VERIFY_TALLY` قبل التنفيذ `FinalizeElection`؛ يفرض المضيفون استخدامًا فريدًا. `FinalizeReferendum` قم بإعادة تسجيل الاستفتاءات ZK حتى لا يتم الانتهاء من الحساب.
    - الإغلاق التلقائي لـ `h_end` يمنح الموافقة/الرفض فريدًا للاستفتاءات العادية؛ تبقى استفتاءات ZK مغلقة حتى يتم الانتهاء من الحساب ويتم تنفيذ `FinalizeReferendum`.
    - تستخدم عمليات التحقق من الإقبال فقط على الموافقة+الرفض؛ الامتناع عن التصويت لا يحسب نسبة الإقبال.

- المشاركة `/v1/gov/enact`
  - الطلب: { "proposal_id": "...64hex"، "preimage_hash": "...64hex؟"، "window": { "lower": 0، "upper": 0}؟، "authority": "ih58...؟"، "private_key": "...؟" }
  - الرد: { "ok": true، "tx_instructions": [{ "wire_id": "...EnactReferendum"، "payload_hex": "..." }] }
  - ملاحظات: Torii هو علامة المعاملة عند `authority`/`private_key`؛ Sinon il renvoie un squelette pour التوقيع وإيصال العميل. الصورة المسبقة هي خيار وإعلام فوري.

- احصل على `/v1/gov/proposals/{id}`
  - المسار `{id}`: معرف الاقتراح سداسي عشري (64 حرفًا)
  - الرد: { "تم العثور عليه": منطقي، "اقتراح": { ... }؟ }- احصل على `/v1/gov/locks/{rid}`
  - المسار `{rid}`: معرف السلسلة للاستفتاء
  - الرد: { "تم العثور عليه": منطقي، "referendum_id": "rid"، "locks": {... }؟ }

- احصل على `/v1/gov/council/current`
  - الرد: { "epoch": N, "members": [{ "account_id": "..." }, ...] }
  - ملاحظات: renvoie le Council يستمر إذا كان موجودًا؛ من المؤكد أنها تستمد تحديدًا احتياطيًا من خلال تكوين أصول الحصة والتتبعات (تعكس مواصفات VRF حتى يتم ضبط VRF بشكل مباشر على السلسلة).

- POST `/v1/gov/council/derive-vrf` (الميزة: gov_vrf)
  - الطلب: { "committee_size": 21، "epoch": 123؟ , "candidates": [{ "account_id": "..."، "variant": "Normal|Small"، "pk_b64": "..."، "proof_b64": "..." }, ...] }
  - السلوك: التحقق من صحة VRF لكل مرشح ضد الإدخال الكنسي المشتق من `chain_id` و`epoch` ومنارة التجزئة الأخيرة للكتلة؛ حاول استخدام وحدات البايت المتساوية مع أدوات الكسر الفاصلة؛ قم بإعادة تسمية الأعضاء `committee_size` الأعلى. لا تستمر.
  - الرد: { "epoch": N, "members": [{ "account_id": "..." } ...], "total_candidates": M, "تم التحقق منه": K }
  - ملاحظات: عادي = pk en G1، إثبات en G2 (96 بايت). صغير = pk en G2، إثبات en G1 (48 بايت). المدخلات منفصلة حسب المجال وتتضمن `chain_id`.

### افتراضيات الحكم (iroha_config `gov.*`)

يستخدم المجلس الاحتياطي على قدم المساواة Torii عندما لا توجد معلمة قائمة موجودة عبر `iroha_config`:

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

تجاوزات معادلات البيئة:

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

`parliament_committee_size` يحد من عدد الاجتماعات الاحتياطية للأعضاء عندما لا يستمر المجلس، `parliament_term_blocks` يحدد طول الفترة المستخدمة لاشتقاق البذور (`epoch = floor(height / term_blocks)`)، `parliament_min_stake` يفرض الحد الأدنى من الحصة (بالوحدات) الحد الأدنى) على الأصول المؤهلة، و`parliament_eligibility_asset_id` تحديد ما يتم مسحه من الأصول عند إنشاء مجموعة المرشحين.

التحقق من حوكمة VK غير تجاوز: يتطلب التحقق من بطاقة الاقتراع دائمًا المفتاح `Active` مع البايتات المضمنة، ولا تحتاج البيئات إلى استخدام مفاتيح الاختبار لإجراء التحقق.

RBAC
- يتطلب التنفيذ على السلسلة أذونات:
  - المقترحات: `CanProposeContractDeployment{ contract_id }`
  - أوراق الاقتراع: `CanSubmitGovernanceBallot{ referendum_id }`
  - التشريع: `CanEnactGovernance`
  - إدارة المجلس (المستقبل): `CanManageParliament`

مساحات الأسماء المحمية
- المعلمة المخصصة `gov_protected_namespaces` (مصفوفة سلاسل JSON) النشطة لبوابة القبول للنشر في قوائم مساحات الأسماء.
- يجب أن يتضمن العملاء عناصر تعريفية للمعاملة من أجل النشر عبر مساحات الأسماء المحمية:
  - `gov_namespace`: le namespace cible (على سبيل المثال، "apps")
  - `gov_contract_id`: معرف منطق العقد في مساحة الاسم
- `gov_manifest_approvers`: معرفات مدققي حسابات مصفوفة JSON الاختيارية. عندما يعلن بيان المسار عن النصاب القانوني > 1، يتطلب القبول تفويضًا للمعاملة بالإضافة إلى قوائم الحسابات لإرضاء نصاب البيان.
- يكشف جهاز القياس عن بعد عن حواسيب الدخول عبر `governance_manifest_admission_total{result}` ليوضح أن المشغلين يميزون عمليات إعادة استخدام chemins `missing_manifest` و`non_validator_authority` و`quorum_rejected` و`protected_namespace_rejected` وما إلى ذلك `runtime_hook_rejected`.
- يعرض القياس عن بعد نظام الإنفاذ عبر `governance_manifest_quorum_total{outcome}` (القيم `satisfied` / `rejected`) لمراجعة الموافقات المتاحة.
- تُلحق الممرات بقائمة مساحات الأسماء المنشورة في بياناتها. يجب أن تقوم جميع المعاملة التي تم إصلاحها `gov_namespace` بتقديم `gov_contract_id`، وستظهر مساحة الاسم في المجموعة `protected_namespaces` في البيان. تم رفض الطلبات `RegisterSmartContractCode` بدون بيانات التعريف هذه عندما تكون الحماية نشطة.
- القبول يفرض اقتراحًا للحوكمة تم تفعيله من أجل المجموعة `(namespace, contract_id, code_hash, abi_hash)`؛ دون أن يكون صدى التحقق من الصحة مع وجود خطأ غير مسموح به.

خطافات ترقية وقت التشغيل
- يمكن لبيانات المسار أن تعلن `hooks.runtime_upgrade` لجمع تعليمات ترقية وقت التشغيل (`ProposeRuntimeUpgrade`، `ActivateRuntimeUpgrade`، `CancelRuntimeUpgrade`).
- شانز دو هوك:
  - `allow` (منطقي، الافتراضي `true`): عندما `false`، يتم رفض جميع تعليمات ترقية وقت التشغيل.
  - `require_metadata` (منطقي، افتراضي `false`): قم بإدخال محدد بيانات التعريف على أساس `metadata_key`.
  - `metadata_key` (سلسلة): اسم البيانات الوصفية مزخرف على شكل خطاف. الافتراضي `gov_upgrade_id` عندما تكون البيانات الوصفية مطلوبة أو عندما تكون القائمة المسموح بها موجودة.
  - `allowed_ids` (صفيف السلاسل): البيانات الوصفية لخيارات القائمة المسموح بها (ما بعد القطع). أرجع عندما لا تكون القيمة الأربعة مدرجة.
- عندما يكون الخطاف موجودًا، يتم تطبيق إدخال الملف على البيانات الوصفية السياسية قبل إدخال المعاملة في الملف. تؤدي البيانات الوصفية الدائمة، أو قيم مقاطع الفيديو أو القائمة المسموح بها، إلى حدوث خطأ محدد غير مسموح به.
- تتبع النتائج عن بعد عبر `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}`.
- تتضمن المعاملات المرضية للخطاف البيانات الوصفية `gov_upgrade_id=<value>` (أو المفتاح المحدد حسب البيان) بالإضافة إلى موافقات المدققين المطلوبة حسب النصاب القانوني للبيان.

نقطة نهاية السلعة
- POST `/v1/gov/protected-namespaces` - قم بتطبيق `gov_protected_namespaces` مباشرة على noeud.
  - الطلب: { "مساحات الأسماء": ["apps"، "system"] }
  - الرد: { "موافق": صحيح، "مطبق": 1 }
  - ملاحظات: تحديد المشرف/الاختبار؛ يتطلب un token API إذا تم تكوينه. من أجل الإنتاج، تفضل توقيع معاملة مع `SetParameter(Custom)`.

مساعدين CLI
-`iroha --output-format text app gov deploy audit --namespace apps [--contains calc --hash-prefix deadbeef]`
  - استعادة نسخ العقد لمساحة الاسم والتحقق مما يلي:
    - Torii يحتوي على الكود الثانوي لكل `code_hash`، ويتوافق Blake2b-32 مع `code_hash`.
    - بيان المخزون من `/v1/contracts/code/{code_hash}` يتوافق مع القيم `code_hash` و`abi_hash`.
    - يوجد اقتراح حوكمة تم تفعيله من أجل `(namespace, contract_id, code_hash, abi_hash)` مشتق من تجزئة معرف الاقتراح الذي يستخدمه الجديد.
  - فرز تقرير JSON مع `results[]` بموجب عقد (القضايا، السيرة الذاتية للبيان/الكود/الاقتراح) بالإضافة إلى استئناف في une ligne sauf قمع (`--no-summary`).
  - مفيد لمراجعة مساحات الأسماء المحمية أو التحقق من سير العمل من خلال نشر الضوابط حسب الإدارة.
-`iroha app gov deploy-meta --namespace apps --contract-id calc.v1 [--approver ih58... --approver ih58...]`
  - استخدم مجموعة البيانات الوصفية JSON في عمليات النشر في مساحات الأسماء المحمية، بما في ذلك خيارات `gov_manifest_approvers` لتلبية قواعد النصاب القانوني للبيان.
- `iroha app gov vote --mode zk --referendum-id <id> --proof-b64 <b64> [--owner ih58... --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]` — تتطلب تلميحات القفل `min_bond_amount > 0`، وتشتمل مجموعة التلميحات الأربعة على `owner` و`amount` و`duration_blocks`.
  - التحقق من صحة معرفات الحساب الأساسية، والتحقق من تلميحات الإبطال ذات 32 بايت، ودمج التلميحات في `public_inputs_json` (مع `--public <path>` للتجاوزات الإضافية).
  - يُشتق المبطل من التزام الإثبات (الإدخال العام) بالإضافة إلى `domain_tag` و`chain_id` و`election_id`؛ تم التحقق من صحة `--nullifier` مقابل الدليل عند تقديمه.
  - تعرض السيرة الذاتية في خط واحد `fingerprint=<hex>` محددًا مشتقًا من `CastZkBallot` ويتم تشفير التلميحات أيضًا (`owner`, `amount`, `duration_blocks`, `direction`) سي فورنيس).
  - تشير ردود CLI إلى `tx_instructions[]` مع `payload_fingerprint_hex` بالإضافة إلى رموز فك التشفير الرئيسية حتى تتمكن الأدوات النهائية من التحقق من الضغط بدون إعادة تنفيذ فك تشفير Norito.
  - قم بإدراج تلميحات القفل للسماح ببدء تشغيل الأحداث `LockCreated`/`LockExtended` لبطاقات الاقتراع ZK مرة واحدة حيث تعرض الدائرة القيم الرمزية.
-`iroha app gov vote --mode plain --referendum-id <id> --owner ih58... --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - الاسم المستعار `--lock-amount`/`--lock-duration-blocks` يعكس أسماء العلامات ZK لتكافؤ البرمجة النصية.
  - تعكس عملية الاستئناف `vote --mode zk` بما في ذلك بصمة التعليمات المشفرة وأبطال الاقتراع المسموح بها (`owner`، `amount`، `duration_blocks`، `direction`)، للتأكيد السريع قبل التوقيع سكيليت.

قائمة المثيلات
- احصل على `/v1/gov/instances/{ns}` - قم بإدراج مثيلات العقود النشطة لمساحة اسم.
  - معلمات الاستعلام:
    - `contains`: مرشح من السلسلة الفرعية `contract_id` (حساس لحالة الأحرف)
    - `hash_prefix`: مرشح بالبادئة السداسية `code_hash_hex` (أحرف صغيرة)
    - `offset` (الافتراضي 0)، `limit` (الافتراضي 100، الحد الأقصى 10_000)
    - `order`: أحد `cid_asc` (افتراضي)، `cid_desc`، `hash_asc`، `hash_desc`
  - الرد: { "مساحة الاسم": "ns"، "المثيلات": [{ "contract_id": "..."، "code_hash_hex": "..." }، ...]، "total": N، "offset": n، "limit": m }
  - SDK المساعد: `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) أو `ToriiClient.list_governance_instances_typed("apps", ...)` (Python).عمليات فتح القفل (المشغل/المراجعة)
- احصل على `/v1/gov/unlocks/stats`
  - الرد: { "height_current": H، "expired_locks_now": n، "referenda_with_expired": m، "last_sweep_height": S }
  - ملاحظات: `last_sweep_height` يعكس ارتفاع الكتلة الأحدث أو تنتهي صلاحية الأقفال وتتضاءل وتستمر. يتم حساب `expired_locks_now` من خلال مسح تسجيلات القفل باستخدام `expiry_height <= height_current`.
- المشاركة `/v1/gov/ballots/zk-v1`
  - الطلب (نمط DTO الإصدار 1):
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
  - الرد: { "موافق": صحيح، "مقبول": صحيح، "tx_instructions": [{...}] }

- المشاركة `/v1/gov/ballots/zk-v1/ballot-proof` (الميزة: `zk-ballot`)
  - اقبل JSON `BallotProof` مباشرة وأعد الضغط على `CastZkBallot`.
  - الطلب:
    {
      "السلطة": "ih58..."،
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "...؟",
      "election_id": "ref-1",
      "الاقتراع": {
        "الواجهة الخلفية": "halo2/ipa"،
        "envelope_bytes": "AAECAwQ=", // base64 من حاوية ZK1 أو H2*
        "root_hint": خالية، // سلسلة سداسية اختيارية ذات 32 بايت (جذر الأهلية)
        "owner": null, // AccountId optionnel إذا كانت الدائرة تلتزم بالمالك
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
    - خريطة خادم الاقتراع `root_hint`/`owner`/`nullifier` لخيارات بطاقة الاقتراع مقابل `public_inputs_json` لـ `CastZkBallot`.
    - يتم إعادة تشفير بايتات المغلف باستخدام Base64 لحمولة التعليمات.
    - تم تمرير الاستجابة `reason` إلى `submitted transaction` عند Torii بعد إجراء الاقتراع.
    - تتوفر نقطة النهاية هذه بشكل فريد إذا كانت الميزة `zk-ballot` نشطة.

ساحة التحقق CastZkBallot
- `CastZkBallot` قم بفك تشفير القاعدة الأساسية 64 الأربعة وأعد تحميل الحمولة المرئية أو غير المرغوب فيها (`BallotRejected` مع `invalid or empty proof`).
- يقوم المضيف بإعادة صياغة مفتاح التحقق من الاقتراع بعد الاستفتاء (`vk_ballot`) أو الإعدادات الافتراضية للإدارة والطلب من وجود التسجيل، لذلك `Active` ونقل البايتات المضمنة.
- يتم إعادة تجزئة بايتات مفتاح التحقق من المخزون باستخدام `hash_vk`؛ عدم تطابق الالتزام بإيقاف التنفيذ قبل التحقق لحماية إدخالات التسجيل التالفة (`BallotRejected` مع `verifying key commitment mismatch`).
- يتم إرسال وحدات البايت المسبقة إلى الواجهة الخلفية للتسجيل عبر `zk::verify_backend`؛ يتم إعادة إنشاء النسخ غير الصالحة في `BallotRejected` مع `invalid proof` وتردد التعليمات تحديدها.
- يجب أن يكشف الدليل عن التزام الاقتراع وجذر الأهلية كمدخلات عامة؛ يجب أن يتطابق الجذر مع `eligible_root` الخاص بالانتخاب، ويجب أن يتطابق المبطل المشتق مع أي تلميح مقدم.
- Les preuves reussies emettent `BallotAccepted`; الإبطال المزدوج، أو جذور حدود الأهلية، أو تراجعات القفل المستمرة تؤدي إلى أسباب رفض الوجود التي تحدد المزيد من الارتفاع في هذا المستند.

## قناة Mauvaise للمصدقين والإجماع المشترك

### سير العمل في التقطيع والسجن

تم تشفير الإجماع على `Evidence` في Norito عندما تم التحقق من صحة البروتوكول. تصل كل حمولة في `EvidenceStore` في الذاكرة، وإذا تم تحريرها، فإنها تتحقق في الخريطة `consensus_evidence` adossee في WSV. يتم رفض التسجيلات الإضافية القديمة التي `sumeragi.npos.reconfig.evidence_horizon_blocks` (الكتل `7200` الافتراضية) لحماية الأرشيف المنقولة، ولكن يتم تسجيلها للمشغلين. تحترم الأدلة الموجودة في الأفق أيضًا `sumeragi.npos.reconfig.activation_lag_blocks` (الافتراضي `1`) وتأخير التقطيع `sumeragi.npos.reconfig.slashing_delay_blocks` (الافتراضي `259200`)؛ يمكن للحوكمة إلغاء العقوبات باستخدام `CancelConsensusEvidencePenalty` قبل تطبيق القطع.

تم تعيين الجرائم التي يتم إعادة عرضها على `EvidenceKind`؛ إن التمييزات هي إسطبلات وتفرض نموذج بيانات متساويًا:

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

- **DoublePrepare/DoubleCommit** - تم التحقق من علامة التجزئة المتعارضة من أجل المجموعة `(phase,height,view,epoch)`.
- **InvalidQc** - مجمع ثرثرة وشهادة التزام لا تشبه صدى عمليات التحقق المحددة (على سبيل المثال، صورة نقطية لتوقيعات الفيديو).
- **اقتراح غير صالح** - يقترح قائد كتلة تعكس بنية التحقق من الصحة (على سبيل المثال، انتهاك قانون السلسلة المقفلة).
- **الرقابة** — تُظهر إيصالات التقديم الموقعة معاملة لم يتم اقتراحها/الالتزام بها مطلقًا.

يمكن للمشغلين والإخراج فحص الحمولات وإعادة بثها عبر:

- Torii: `GET /v1/sumeragi/evidence` و`GET /v1/sumeragi/evidence/count`.
- سطر الأوامر: `iroha ops sumeragi evidence list`، `... count`، و`... submit --evidence-hex <payload>`.

يجب أن تجعل الإدارة بايتات الأدلة بمثابة قانون قانوني:

1. **تجميع الحمولة** قبل أن تنتهي صلاحيتها. يتم أرشفة وحدات البايت Norito باستخدام ارتفاع/عرض البيانات التعريفية.
2. **إعداد العقوبة** وحظر الحمولة في استفتاء أو تعليمات Sudo (على سبيل المثال، `Unregister::peer`). يؤدي التنفيذ إلى إعادة صلاحية الحمولة؛ الأدلة السيئة الشكل أو القديمة ترفض الحتمية.
3. **مخطط المتابعة** حتى لا يتمكن المدقق الخاطئ من العودة على الفور. يتم إرسال التدفقات النموذجية إلى `SetParameter(Sumeragi::NextMode)` و`SetParameter(Sumeragi::ModeActivationHeight)` مع القائمة خلال يوم واحد.
4. **تدقيق النتائج** عبر `/v1/sumeragi/evidence` و`/v1/sumeragi/status` للتأكد من أن حساب الأدلة مقدمًا وأن الإدارة ستطبق على التراجع.

### تسلسل الإجماع المشترك

ويضمن الإجماع المشترك أن مجموعة المدققين تعمل على وضع اللمسات الأخيرة على كتلة الحدود قبل أن تبدأ المجموعة الجديدة في تقديم مقترح. يفرض وقت التشغيل القاعدة من خلال إعدادات الإعدادات:

- `SumeragiParameter::NextMode` و`SumeragiParameter::ModeActivationHeight` يتم تنفيذهما في **meme bloc**. `mode_activation_height` يجب أن تكون متفوقًا بشكل صارم على ارتفاع الكتلة التي تفتح الباب على مدار اليوم، دون أن تتأخر كثيرًا.
- `sumeragi.npos.reconfig.activation_lag_blocks` (الافتراضي `1`) هو حارس التكوين الذي يمنع عمليات التسليم من تأخر صفر:
- `sumeragi.npos.reconfig.slashing_delay_blocks` (الافتراضي `259200`) يؤخر خفض الإجماع حتى تتمكن الإدارة من إلغاء العقوبات قبل تطبيقها.

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```

- يعرض وقت التشغيل وCLI المعلمات التي تم تنظيمها عبر `/v1/sumeragi/params` و`iroha --output-format text ops sumeragi params`، حتى يؤكد المشغلون على خطوات التنشيط وقوائم المدققين.
- أتمتة الحكم يجب أن تكون دائمًا:
  1. إنهاء قرار العودة (أو إعادة الإدماج) المدعوم بالأدلة.
  2. أدخل إعادة تكوين المتابعة باستخدام `mode_activation_height = h_current + activation_lag_blocks`.
  3. المراقب `/v1/sumeragi/status` حتى `effective_consensus_mode` يتطلع إلى أعلى مستوى.

كل النص الذي يقوم بتدوير المدققين أو تطبيق شرطة مائلة **لا تفعل ذلك** قم بتنشيط التنشيط بفارق صفر أو حذف معلمات التسليم؛ تم رفض هذه المعاملات وتركت الشبكة في الوضع السابق.

## أسطح القياس عن بعد

- المقاييس Prometheus المصدرة لنشاط الحكم:
  - `governance_proposals_status{status}` (المقياس) يناسب محاسبي المقترحات حسب الحالة.
  - `governance_protected_namespace_total{outcome}` (العداد) يزيد عند قبول مساحات الأسماء المحمية أو إعادة نشرها.
  - `governance_manifest_activations_total{event}` (العداد) قم بتسجيل إدراجات البيان (`event="manifest_inserted"`) وروابط مساحة الاسم (`event="instance_bound"`).
- يشتمل `/status` على كائن `governance` الذي يعيد حسابات المقترحات، ويربط جميع مساحات الأسماء المحمية، ويسرد عمليات التنشيط الأخيرة للبيان (مساحة الاسم، ومعرف العقد، وتجزئة الكود/ABI، وارتفاع الكتلة، والطابع الزمني للتنشيط). يمكن للمشغلين أن يتعرفوا على هذا الأمر للتأكد من أن التشريعات لا تظهر في يوم البيانات وأن بوابات مساحات الأسماء المحمية يتم فرضها.
- قالب Grafana (`docs/source/grafana_governance_constraints.json`) ودليل القياس عن بعد في `telemetry.md` يُعلق بكابل تنبيهات لمقترحات الحظر، أو تنشيط البيانات المفقودة، أو رفض عدم الاهتمام بمساحات الأسماء المحمية أثناء ترقيات وقت التشغيل.
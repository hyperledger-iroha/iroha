---
lang: ar
direction: rtl
source: docs/portal/docs/governance/api.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

الحالة: تشيرنوفيك/مجمع للموافقة على تحقيق الإدارة. يمكن أن يتم تغيير النماذج في أي مكان. الحسم والسياسة يجسدان RBAC المعايير التنظيمية؛ يمكن لـ Torii تقديم/إدارة المعاملات عند الحصول على `authority` و`private_key`، حيث يشترك العملاء انتقل إلى `/transaction`.

فكرة
- جميع نقاط النهاية تستجيب لـ JSON. بالنسبة للخطوات التي تجري المعاملات، تتضمن الإجابة `tx_instructions` - مجموعة كبيرة أو عدة هياكل من التعليمات:
  - `wire_id`: تعليمات نوع المعرف الاحتياطي
  - `payload_hex`: الحمولة النافعة Norito (ست عشرية)
- إذا تم تقديم `authority` و`private_key` (أو `private_key` في بطاقات الاقتراع DTO)، فإن Torii يلصق وينفذ يتم إجراء المعاملة وكل شيء بشكل صحيح `tx_instructions`.
- يشترك العملاء في البداية في المعاملة الموقعة مع سلطتهم وchain_id، ثم يرسلونها وينشرونها في `/transaction`.
- إنشاء SDK:
- Python (`iroha_python`): `ToriiClient.get_governance_proposal_typed` تم الاتصال بـ `GovernanceProposalResult` (تسوية حالة/نوع القطب)، `ToriiClient.get_governance_referendum_typed` تم الاتصال بـ `GovernanceReferendumResult`، `ToriiClient.get_governance_tally_typed` يعبر عن `GovernanceTally`، `ToriiClient.get_governance_locks_typed` يعبر عن `GovernanceLocksResult`، `ToriiClient.get_governance_unlock_stats_typed` يعبر عن `GovernanceUnlockStats`، و تم تفعيل `ToriiClient.list_governance_instances_typed` من خلال `GovernanceInstancesPage`، وهو نوع مخصص للتسليم عبر جميع الحوكمة الشاملة من خلال الاستخدام التجريبي في ملف README.
- عميل Python البسيط (`iroha_torii_client`): `ToriiClient.finalize_referendum` و`ToriiClient.enact_proposal` يقوم بتكوين الحزم النموذجية `GovernanceInstructionDraft` (الموافقة على ذلك) Torii الهيكل العظمي `tx_instructions`)، احذف التوزيع الدقيق لـ JSON من خلال إنهاء/تفعيل التدفقات.
- JavaScript (`@iroha/iroha-js`): يوفر `ToriiClient` أنواعًا من المساعدين للمقترحات والاستفتاءات وعمليات الفرز والأقفال وفتح الإحصائيات، وما إلى ذلك `listGovernanceInstances(namespace, options)` بالإضافة إلى نقاط نهاية المجلس (`getGovernanceCouncilCurrent`، `governanceDeriveCouncilVrf`، `governancePersistCouncil`، `getGovernanceCouncilAudit`)، بحيث يمكن لعملاء Node.js زيارة `/v2/gov/instances/{ns}` والبدء سير العمل المدعوم من VRF إلى جانب حالات التعاقد القائمة على النظام.

نقاط النهاية

- المشاركة `/v2/gov/proposals/deploy-contract`
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
  - الاستجابة (JSON):
    { "ok": صحيح، "proposal_id": "...64hex"، "tx_instructions": [{ "wire_id": "..."، "payload_hex": "..." }] }
  - التحقق من الصحة: قم بتسجيل الدخول إلى `abi_hash` من أجل `abi_version` وتجاوز الحاجة. ل`abi_version = "v1"` عذرًا - `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))`.

واجهة برمجة تطبيقات العقود (النشر)
- المشاركة `/v2/contracts/deploy`
  - الطلب: { "authority": "i105..."، "private_key": "..."، "code_b64": "..." }
  - السلوك: انتقل إلى `code_hash` من خلال برامج IVM و`abi_hash` من خلال `abi_version`، ثم قم بالتصحيح `RegisterSmartContractCode` (البيان) و`RegisterSmartContractBytes` (البيانات الكاملة `.to`) من `authority`.
  - الرد: { "ok": صحيح، "code_hash_hex": "..."، "abi_hash_hex": "..." }
  - ذات صلة:
    - احصل على `/v2/contracts/code/{code_hash}` -> بيان الضمان المصرح به
    - احصل على `/v2/contracts/code-bytes/{code_hash}` -> أدخل `{ code_b64 }`
- المشاركة `/v2/contracts/instance`
  - الطلب: { "authority": "i105..."، "private_key": "..."، "مساحة الاسم": "apps"، "contract_id": "calc.v1"، "code_b64": "..." }
  - السلوك: قم باستخدام الرمز الثانوي المفضل وقم بتنشيط رسم الخرائط `(namespace, contract_id)` من خلال `ActivateContractInstance`.
  - الاستجابة: { "ok": true، "namespace": "apps"، "contract_id": "calc.v1"، "code_hash_hex": "..."، "abi_hash_hex": "..." }

خدمة الاسم المستعار
- المشاركة `/v2/aliases/voprf/evaluate`
  - الطلب: { "blinded_element_hex": "..." }
  - الاستجابة: { "evaluated_element_hex": "...128hex"، "backend": "blake2b512-mock" }
    - `backend` ينفذ التقييم. الاسم الحقيقي: `blake2b512-mock`.
  - ملاحظات: تحديد السعر الوهمي، أول Blake2b512 مع فصل المجال `iroha.alias.voprf.mock.v1`. ممتاز لأدوات الاختبار، لأن خط أنابيب الإنتاج VOPRF غير مقيد بـ Iroha.
  - الأخطاء: HTTP `400` في حالة عدم وجود مفتاح سداسي عشري. يتم إرسال Torii إلى Norito مغلف `ValidationFail::QueryFailed::Conversion` مع وحدة فك التشفير الخاصة به.
- المشاركة `/v2/aliases/resolve`
  - الطلب: { "الاسم المستعار": "GB82 WEST 1234 5698 7654 32" }
  - الاستجابة: { "الاسم المستعار": "GB82WEST12345698765432"، "account_id": "i105..."، "index": 0، "source": "iso_bridge" }
  - ملاحظات: تحتاج إلى التدريج المرحلي لجسر ISO (`[iso_bridge.account_aliases]` в `iroha_config`). يقوم Torii بتطبيع الاسم المستعار وإجراء الاختبارات والدخول إلى السجل النهائي. قم بالموافقة على 404 في الاسم المستعار و 503 عند إلغاء وقت تشغيل جسر ISO.
- المشاركة `/v2/aliases/resolve_index`
  - الطلب: { "الفهرس": 0 }
  - الاستجابة: { "الفهرس": 0، "الاسم المستعار": "GB82WEST12345698765432"، "account_id": "i105..."، "source": "iso_bridge" }
  -ملاحظات: فهرس الاسم المستعار يُسمى محددًا حسب التكوينات (المستندة إلى 0). يمكن للعملاء الرد على الأسئلة دون الاتصال بالإنترنت من أجل تعزيز مسار التدقيق من خلال الاسم المستعار للمصادقة.

الحد الأقصى لحجم الرمز
- المعلمة المخصصة: `max_contract_code_bytes` (JSON u64)
  - إدارة الحد الأقصى لحجم العقد (في المنازل) على السلسلة.
  - الافتراضي: 16 ميجابايت. يتم إلغاء استنساخ `RegisterSmartContractBytes` عندما يتم تحديد حجم `.to` للحد الأقصى، مع انتهاك ثابت للغاية.
  - يمكن للمشغلين الإزالة من خلال `SetParameter(Custom)` مع `id = "max_contract_code_bytes"` والحمولة الصافية.

- المشاركة `/v2/gov/ballots/zk`
  - الطلب: { "authority": "i105..."، "private_key": "...؟"، "chain_id": "..."، "election_id": "e1"، "proof_b64": "..."، "public": {...} }
  - الاستجابة: { "موافق": صحيح، "مقبول": صحيح، "tx_instructions": [{...}] }
  - ملاحظات:
    - عندما تتضمن مخططات المياه العامة `owner` و`amount` و`duration_blocks`، ويتم التحقق من الإثبات باستخدام VK القوي، لا قم بإنشاء أو توفير قفل الإدارة لـ `election_id` باستخدام هذا العنصر `owner`. ضبط سكريتو (`unknown`); يتم تحديد المبلغ/انتهاء الصلاحية فقط. المبالغ المكررة رتيبة: المبلغ وانتهاء الصلاحية لا يزيدان عن حدهما (نقطة أولية max(amount, prev.amount) و max(expiry, prev.expiry)).
    - إعادة التصويت لـ ZK، أو تقليل المبلغ أو انتهاء الصلاحية، أو إلغاء استنساخ الخادم باستخدام التشخيص `BallotRejected`.
    - يجب توصيل العقد `ZK_VOTE_VERIFY_BALLOT` إلى البريد `SubmitBallot`; يفرض المضيفون مزلاجًا فريدًا.

- المشاركة `/v2/gov/ballots/plain`
  - الطلب: { "authority": "i105..."، "private_key": "...؟"، "chain_id": "..."، "referendum_id": "r1"، "owner": "i105..."، "amount": "1000"، "duration_blocks": 6000، "direction": "Aye|Nay|امتناع" }
  - الاستجابة: { "موافق": صحيح، "مقبول": صحيح، "tx_instructions": [{...}] }
  - ملاحظات: إعادة التصويت فقط على التوسيع - لا يمكن لبطاقة الاقتراع الجديدة تقليل المبلغ أو انتهاء صلاحية القفل الحالي. `owner` مطلوب الالتزام بمعاملات السلطة. الحد الأدنى من الإنفاق - `conviction_step_blocks`.

- المشاركة `/v2/gov/finalize`
  - الطلب: { "referendum_id": "r1"، "proposal_id": "...64hex"، "authority": "i105...؟"، "private_key": "...؟" }
  - الاستجابة: { "ok": true، "tx_instructions": [{ "wire_id": "...FinalizeReferendum"، "payload_hex": "..." }] }
  - التأثير على السلسلة (السقالة الحالية): تفعيل اقتراح النشر بشكل فعال الحد الأدنى `ContractManifest`، الوصول إلى `code_hash`، مع `abi_hash` و помечает الاقتراح كيف تم تفعيله. إذا كان البيان متاحًا لـ `code_hash` مع `abi_hash` آخر، فسيتم إلغاء تفعيل التشريع.
  - ملاحظات:
    - بالنسبة لعقود انتخابات ZK، يتم تسجيل `ZK_VOTE_VERIFY_TALLY` إلى `FinalizeElection`؛ يفرض المضيفون مزلاجًا فريدًا. `FinalizeReferendum` يقوم بإلغاء استنساخ مراجع ZK، حتى لا يتم الانتهاء من اختيار العدد.
    - الموافقة على `h_end` تكتب تمت الموافقة/الرفض فقط للمرجع العادي; تبقى استفتاءات ZK مغلقة، حتى لا يتم الانتهاء من الحساب النهائي و`FinalizeReferendum`.
    - تستخدم نسبة الإقبال على التصويت فقط الموافقة+الرفض؛ الامتناع عن التصويت لا يؤثر على الإقبال.

- المشاركة `/v2/gov/enact`
  - الطلب: { "proposal_id": "...64hex"، "preimage_hash": "...64hex؟"، "window": { "lower": 0، "upper": 0}؟، "authority": "i105...؟"، "private_key": "...؟" }
  - الاستجابة: { "ok": true، "tx_instructions": [{ "wire_id": "...EnactReferendum"، "payload_hex": "..." }] }
  - ملاحظات: يقوم Torii بإجراء معاملة مباشرة عند كتابة `authority`/`private_key`؛ إنها في كثير من الأحيان هيكل عظمي لتقديم الدعم للعملاء والتعامل معهم. Preimage اختيارية ولها طابع المعلومات الخاص بها.

- احصل على `/v2/gov/proposals/{id}`
  - المسار `{id}`: معرف الاقتراح الست عشري (64 حرفًا)
  - الاستجابة: { "تم العثور عليه": منطقي، "اقتراح": { ... }؟ }

- احصل على `/v2/gov/locks/{rid}`
  - المسار `{rid}`: سلسلة معرف الاستفتاء
  - الاستجابة: { "تم العثور عليه": منطقي، "referendum_id": "rid"، "أقفال": {... }؟ }

- احصل على `/v2/gov/council/current`
  - الاستجابة: { "العصر": N، "الأعضاء": [{ "account_id": "..." }، ...] }
  - ملاحظات: نفذ المجلس المستمر بعد الانتهاء؛ يتم هنا استخلاص احتياطي محدد باستخدام أصول وعتبات حصص كبيرة (تحدد مواصفات VRF لهذه الأغراض، حيث لا يتم الاحتفاظ ببراهين VRF على السلسلة).- POST `/v2/gov/council/derive-vrf` (الميزة: gov_vrf)
  - الطلب: { "committee_size": 21، "epoch": 123؟ , "candidates": [{ "account_id": "..."، "variant": "Normal|Small"، "pk_b64": "..."، "proof_b64": "..." }, ...] }
  - السلوك: التحقق من إثبات VRF كشرط للإدخال القانوني، المتوفر من `chain_id`، `epoch` ومنارة تجزئة الكتلة اللاحقة؛ فرز وحدات البايت الناتجة حسب فاصل التعادل; возвращает أعلى `committee_size` участников. لا تتورط.
  - الاستجابة: { "epoch": N, "members": [{ "account_id": "..." } ...], "total_candidates": M, "تم التحقق منه": K }
  - ملاحظات: عادي = pk в G1، إثبات в G2 (96 بايت). صغير = pk в G2، دليل в G1 (48 بايت). يتم تغيير المدخلات بشكل كبير وتتضمن `chain_id`.

### افتراضيات الإدارة (iroha_config `gov.*`)

المجلس الاحتياطي، يستخدم Torii عند الكشف عن القائمة المستمرة، تتم المعلمة من خلال `iroha_config`:

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

التغطية المؤقتة المكافئة:

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

`parliament_committee_size` يقوم باستعادة سلاسة احتياطية من خلال مجلس المراجعة، `parliament_term_blocks` يزود سلاسل اشتقاق البذور (`epoch = floor(height / term_blocks)`)، `parliament_min_stake` تحتاج إلى الحد الأدنى من الحصة (في الحد الأدنى من الوحدات) للأصول المؤهلة، ويتم اختيار `parliament_eligibility_asset_id` عند فحص رصيد الأصول دعم عدد كبير من المرشحين.

التحقق من حوكمة VK بدون تجاوز: التحقق من بطاقات الاقتراع يجب عليك دائمًا التحقق من `Active` من المفتاح مع البايتات المضمنة، ولا يتطلب الحفظ استخدام تبديل التبديل للاختبار، لنشرها التحقق من ذلك.

RBAC
- يجب أن يكون الاتصال عبر السلسلة مختلفًا:
  - المقترحات: `CanProposeContractDeployment{ contract_id }`
  - أوراق الاقتراع: `CanSubmitGovernanceBallot{ referendum_id }`
  - التشريع: `CanEnactGovernance`
  - إدارة المجلس (المستقبل): `CanManageParliament`

مساحات الأسماء المحمية
- تتضمن المعلمة المخصصة `gov_protected_namespaces` (مجموعة سلاسل JSON) بوابة القبول للنشر في مساحات الأسماء المسبقة.
- يحتاج العملاء إلى تضمين مفاتيح البيانات التعريفية للمعاملات للنشر، المناسبة في مساحات الأسماء المحمية:
  - `gov_namespace`: مساحة الاسم الجميلة (على سبيل المثال، "التطبيقات")
  - `gov_contract_id`: مساحة الاسم لمعرف العقد المنطقي
- `gov_manifest_approvers`: مدقق معرفات حساب صفيف JSON الاختياري. عندما يكتمل بيان المسار النصاب القانوني > 1، يتطلب القبول إجراء معاملات السلطة بالإضافة إلى الحسابات المسبقة لبيان النصاب القانوني.
- يوفر جهاز القياس عن بعد عدادات القبول من خلال `governance_manifest_admission_total{result}`، بحيث يتم قبول المشغلين الناجحين من `missing_manifest`، `non_validator_authority`، `quorum_rejected`، `protected_namespace_rejected`، و`runtime_hook_rejected`.
- يُظهر جهاز القياس عن بعد مسار التنفيذ عبر `governance_manifest_quorum_total{outcome}` (الاسم `satisfied` / `rejected`)، مما يسمح لمشغل الصوت بالاستماع إلى الموافقات غير الضرورية.
- تفرض الممرات القائمة المسموح بها لمساحة الاسم، ويتم نشرها في البيانات. أي معاملة ترغب في تعزيزها `gov_namespace`، وتقترح `gov_contract_id`، وتخلق مساحة الاسم في المجموعة `protected_namespaces` البيان. سيتم إلغاء عمليات الإرسال `RegisterSmartContractCode` بدون بيانات التعريف هذه، عند تضمين الحماية.
- يجب أن يكون القبول سببًا في اقتراح الحوكمة الذي تم تفعيله لـ Tuple `(namespace, contract_id, code_hash, abi_hash)`; تنتهي عملية التحقق من الصحة بشكل غير مسموح به.

خطافات ترقية وقت التشغيل
- يمكن أن تتوافق بيانات Lane مع `hooks.runtime_upgrade` لتعليمات ترقية وقت تشغيل البوابة (`ProposeRuntimeUpgrade`، `ActivateRuntimeUpgrade`، `CancelRuntimeUpgrade`).
- خطاف بوليا:
  - `allow` (منطقي، الافتراضي `true`): عند `false`، سيتم إلغاء جميع تعليمات ترقية وقت التشغيل.
  - `require_metadata` (منطقي، افتراضي `false`): بيانات تعريف الإدخال المطلوبة، يتم تحديد `metadata_key`.
  - `metadata_key` (سلسلة): تحتوي على البيانات التعريفية، والربط القسري. الافتراضي `gov_upgrade_id`، عندما تكون البيانات الوصفية مطلوبة أو موجودة في القائمة المسموح بها.
  - `allowed_ids` (صفيف من السلاسل): القائمة المسموح بها الاختيارية للبيانات الوصفية المميزة (القطع التالي). تم إغلاقه عندما لا يتم إدخال الإشعارات المسبقة في القائمة.
- عندما يقوم الخطاف بالموافقة، تقوم مراقبة القبول بفرض سياسة بيانات التعريف لنشر المعاملات في الجهة المقابلة. تؤدي البيانات الوصفية الناتجة، أو الإيقاف المستمر أو الإيقاف في القائمة المسموح بها إلى تحديد NotPermit.
- قياس المسافة يراقب النتائج من خلال `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}`.
- المعاملات، ربط الموافقة المسبقة عن علم، تضمين البيانات الوصفية `gov_upgrade_id=<value>` (أو مفتاح، البيان المسبق) مع أي موافقات المدقق، trebуемыми النصاب القانوني.

نقطة نهاية الراحة
- POST `/v2/gov/protected-namespaces` - قم باستبدال `gov_protected_namespaces` على العقدة.
  - الطلب: { "مساحات الأسماء": ["apps"، "system"] }
  - الرد: { "موافق": صحيح، "مطبق": 1 }
  - ملاحظات: مسبق للمسؤول/الاختبار؛ تحتاج إلى رمز واجهة برمجة التطبيقات (API) عند التكوين. في الإنتاج المقترح للمعاملة مع `SetParameter(Custom)`.

مساعدي CLI
-`iroha --output-format text app gov deploy audit --namespace apps [--contains calc --hash-prefix deadbeef]`
  - احصل على مثيلات التعاقد لمساحة الاسم وتحقق مما يلي:
    - Torii هو رمز ثانوي لكل من `code_hash`، وهو Blake2b-32 يلخص `code_hash`.
    - البيان تحت `/v2/contracts/code/{code_hash}` يطابق `code_hash` و`abi_hash`.
    - قم بإعداد اقتراح الإدارة الذي تم تفعيله لـ `(namespace, contract_id, code_hash, abi_hash)`، واشتقاق العناصر مثل تجزئة معرف الاقتراح لاستخدامها.
  - قم بإرجاع JSON إلى `results[]` من خلال العقد (القضايا وملخصات البيان/الكود/الاقتراح) والملخص الفردي، إذا لم يكن مناسبًا (`--no-summary`).
  - مفيد لتدقيق مساحات الأسماء المحمية أو التحقق من سير عمل النشر الذي تسيطر عليه الإدارة.
-`iroha app gov deploy-meta --namespace apps --contract-id calc.v1 [--approver i105... --approver i105...]`
  - قم باستخدام بيانات تعريف هيكل JSON للنشر في مساحات الأسماء المحمية، بما في ذلك `gov_manifest_approvers` الاختياري لبيان النصاب القانوني الصحيح.
- `iroha app gov vote --mode zk --referendum-id <id> --proof-b64 <b64> [--owner i105... --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]` - تلميحات القفل المخصصة لـ `min_bond_amount > 0`، وأي تلميحات أخرى تتضمن `owner` و`amount` و `duration_blocks`.
  - التحقق من صحة معرفات الحساب الأساسية، والتحقق من تلميحات الإبطال ذات 32 بايت، ودمج التلميحات في `public_inputs_json` (مع `--public <path>` للتجاوزات الإضافية).
  - يُشتق المبطل من التزام الإثبات (الإدخال العام) بالإضافة إلى `domain_tag` و`chain_id` و`election_id`؛ تم التحقق من صحة `--nullifier` مقابل الدليل عند تقديمه.
  - ملخص موحد لتنسيق تحديد `fingerprint=<hex>` من التشفير `CastZkBallot` مع تلميحات فك التشفير (`owner`، `amount`، `duration_blocks`، `direction` عند الانتهاء).
  - يجيب CLI على التعليق `tx_instructions[]` على `payload_fingerprint_hex` ومعامل فك التشفير، بحيث يمكن للأدوات النهائية التحقق من الهيكل العظمي بدون رجعة تحقيق فك التشفير Norito.
  - تتيح تلميحات القفل المسبقة حذف `LockCreated`/`LockExtended` لبطاقات اقتراع ZK، عندما يتم تمييز المخطط أو تمييزه.
-`iroha app gov vote --mode plain --referendum-id <id> --owner i105... --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - الاسم المستعار `--lock-amount`/`--lock-duration-blocks` يرسل أعلام ZK للتكافؤ في النصوص البرمجية.
  - ملخص يخرج `vote --mode zk`، بما في ذلك تعليمات تشفير بصمة الإصبع وبطاقة الاقتراع (`owner`، `amount`، `duration_blocks`, `direction`)، يتم التحقق من صحة ما قبل الهيكل العظمي.

قائمة المثيلات
- احصل على `/v2/gov/instances/{ns}` - سجل عقود المقاولات النشطة لمساحة الاسم.
  - معلمات الاستعلام:
    - `contains`: عامل التصفية عبر السلسلة الفرعية `contract_id` (حساس لحالة الأحرف)
    - `hash_prefix`: مرشح للبادئة السداسية `code_hash_hex` (أحرف صغيرة)
    - `offset` (الافتراضي 0)، `limit` (الافتراضي 100، الحد الأقصى 10_000)
    - `order`: `cid_asc` (افتراضي)، `cid_desc`، `hash_asc`، `hash_desc`
  - الاستجابة: { "مساحة الاسم": "ns"، "المثيلات": [{ "contract_id": "..."، "code_hash_hex": "..." }، ...]، "total": N، "offset": n، "limit": m }
  - مساعد SDK: `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) أو `ToriiClient.list_governance_instances_typed("apps", ...)` (Python).

فتح عملية المسح (المشغل/المراجعة)
- احصل على `/v2/gov/unlocks/stats`
  - الاستجابة: { "height_current": H، "expired_locks_now": n، "referenda_with_expired": m، "last_sweep_height": S }
  - ملاحظات: `last_sweep_height` يزيل ارتفاع الكتلة الأخير، عندما يتم اجتياح الأقفال منتهية الصلاحية واستمرارها. `expired_locks_now` يتم نسخ سجلات القفل من خلال المسح الضوئي باستخدام `expiry_height <= height_current`.
- المشاركة `/v2/gov/ballots/zk-v1`
  - الطلب (DTO بنمط v1):
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
  - الاستجابة: { "موافق": صحيح، "مقبول": صحيح، "tx_instructions": [{...}] }- المشاركة `/v2/gov/ballots/zk-v1/ballot-proof` (الميزة: `zk-ballot`)
  - استخدم `BallotProof` JSON واستخدم الهيكل العظمي `CastZkBallot`.
  - الطلب:
    {
      "السلطة": "i105..."،
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "...؟",
      "election_id": "ref-1",
      "الاقتراع": {
        "الواجهة الخلفية": "halo2/ipa"،
        "envelope_bytes": "AAECAwQ=", // قاعدة 64 حاوية ZK1 أو H2*
        "root_hint": خالية، // سلسلة سداسية اختيارية ذات 32 بايت (جذر الأهلية)
        "المالك": فارغ، // معرف الحساب الاختياري عندما يتم إصلاح مالك الدائرة
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
    - خريطة الخادم الاختيارية `root_hint`/`owner`/`nullifier` من الاقتراع في `public_inputs_json` لـ `CastZkBallot`.
    - يتم تشفير بايتات المغلف مرة أخرى إلى Base64 لتعليمات الحمولة النافعة.
    - `reason` يشير إلى `submitted transaction`، عندما يقوم Torii بإجراء الاقتراع.
    - يمكن توفير نقطة النهاية هذه فقط باستخدام الميزة `zk-ballot`.

مسار التحقق CastZkBallot
- يقوم `CastZkBallot` بفك تشفير قاعدة 64 المسبقة وإلغاء استنساخ الحمولات الثابتة/الصغيرة (`BallotRejected` مع `invalid or empty proof`).
- يقوم المضيف بفحص مفتاح التحقق من الاقتراع من خلال الاستفتاء (`vk_ballot`) أو الإعدادات الافتراضية للحوكمة ويجب أن يقوم بكتابة الإعدادات، `Active` والتوصيل المضمن بايت.
- يتم إعادة ربط وحدات بايت التحقق من المفتاح تلقائيًا باستخدام `hash_vk`؛ أي التزام غير ضروري يؤدي إلى التحقق من الحماية من إدخالات التسجيل اللاحقة (`BallotRejected` مع `verifying key commitment mismatch`).
- يتم نقل بايتات الإثبات إلى الواجهة الخلفية المسجلة عبر `zk::verify_backend`؛ يتم تحويل النصوص غير الصالحة إلى `BallotRejected` إلى `invalid proof` ويتم تطبيق تعليمات محددة.
- يجب أن يكشف الدليل عن التزام الاقتراع وجذر الأهلية كمدخلات عامة؛ يجب أن يتطابق الجذر مع `eligible_root` الخاص بالانتخاب، ويجب أن يتطابق المبطل المشتق مع أي تلميح مقدم.
- البراهين الجيدة تتضمن `BallotAccepted`; تعمل الإبطال التالي، أو جذور الأهلية المكتملة، أو قفل التراجع على تعزيز الأسباب الفعلية، على نطاق الوصف.

## المدققون الجدد والتوافق المتطور

### عملية التقطيع والسجن

توافق الآراء على Norito-encoded `Evidence` عند التحقق من صحة البروتوكول. يتم إرسال بعض الحمولة النافعة إلى `EvidenceStore` في الذاكرة، وإذا كانت جديدة، فإنها تتجسد في `consensus_evidence` المدعومة من WSV. يتم إلغاء استنساخ الصفحة الأولى `sumeragi.npos.reconfig.evidence_horizon_blocks` (كتل `7200` الافتراضية) من أجل تخزين الأرشيف بشكل أساسي، ولكن لا يتم تسجيل الدخول إليه المشغلين. تحترم الأدلة الموجودة في الأفق أيضًا `sumeragi.npos.reconfig.activation_lag_blocks` (الافتراضي `1`) وتأخير التقطيع `sumeragi.npos.reconfig.slashing_delay_blocks` (الافتراضي `259200`)؛ يمكن للحوكمة إلغاء العقوبات باستخدام `CancelConsensusEvidencePenalty` قبل تطبيق القطع.

يتم عرض الضجيج المناسب بواسطة واحد إلى `EvidenceKind`; التمييز المستقر والمثبت بشكل طبيعي في نموذج البيانات:

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

- **DoublePrepare/DoubleCommit** - يقوم المدقق بإرسال قائمة التعارضات لشخص واحد `(phase,height,view,epoch)`.
- **InvalidQc** - قام المجمّع بإلغاء شهادة الالتزام، بحيث لا يؤدي النموذج إلى التحقق من التحديد (على سبيل المثال، صورة نقطية ثابتة للموقع).
- **اقتراح غير صالح** - كتلة مقترحة تمنع التحقق من صحة البنية (على سبيل المثال، تقترح قاعدة السلسلة المقفلة).
- **الرقابة** — تُظهر إيصالات التقديم الموقعة معاملة لم يتم اقتراحها/الالتزام بها مطلقًا.

يمكن للمشغلين والأدوات إعادة شحن الحمولات وإعادة إرسالها مرة أخرى عبر:

- Torii: `GET /v2/sumeragi/evidence` و`GET /v2/sumeragi/evidence/count`.
- سطر الأوامر: `iroha ops sumeragi evidence list`، `... count`، `... submit --evidence-hex <payload>`.

تعمل الحوكمة على توزيع وحدات بايت الأدلة مثل التوثيق القانوني:

1. **اكتشف الحمولة** حتى تصل إلى مرحلة الإنشاء. أرشفة جميع وحدات البايت Norito مع بيانات تعريف الارتفاع/العرض.
2. **قم بإدخال الحمولة** قم بإدراج الحمولة في الاستبيان أو تعليمات Sudo (على سبيل المثال، `Unregister::peer`). تمكين الحمولة التحقق من صحتها مرة أخرى. تمنع الأدلة المشوهة أو التي لا معنى لها التحديد.
3. ** قم بتخطيط طبولوجيا المتابعة ** لكي لا يكون المدقق الضخم دخانًا مؤقتًا. توجد أنواع الخطوط `SetParameter(Sumeragi::NextMode)` و`SetParameter(Sumeragi::ModeActivationHeight)` مع قائمة عادية.
4. **نتائج التدقيق** من خلال `/v2/sumeragi/evidence` و`/v2/sumeragi/status`، للتأكد من أن الأدلة الحكيمة فاسدة وتطبيقات الحوكمة.

### Последовательность توافق مشترك

ويضمن الإجماع المشترك أن المدققين المختلفين قد وضعوا اللمسات النهائية على كتلة جيدة إلى حد ما عندما تبدأ مجموعة جديدة. يفرض وقت التشغيل بشكل صحيح من خلال معلمات الطرف:

- `SumeragiParameter::NextMode` و `SumeragiParameter::ModeActivationHeight` يجب الالتزام بها في **تلك الكتلة**. `mode_activation_height` يجب أن تكون كتلة ذات قوة أكبر، والتي ستوفر لك الحد الأدنى من تأخر الكتلة.
- `sumeragi.npos.reconfig.activation_lag_blocks` (الافتراضي `1`) - هذا هو حارس التكوين الذي يسمح بالتسليم دون الحاجة إلى:
- `sumeragi.npos.reconfig.slashing_delay_blocks` (الافتراضي `259200`) يؤخر خفض الإجماع حتى تتمكن الإدارة من إلغاء العقوبات قبل تطبيقها.

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```

- يعرض وقت التشغيل وCLI المعلمات المرحلية من خلال `/v2/sumeragi/params` و`iroha --output-format text ops sumeragi params`، بحيث يقوم المشغلون بفحص ارتفاعات التنشيط ومدققي القائمة.
- أتمتة الحكم مطلوبة دائمًا:
  1. الانتهاء من اتخاذ قرار بشأن الاستبعادات (أو الإثباتات)، تقديم الأدلة.
  2. نشر متابعة إعادة التكوين باستخدام `mode_activation_height = h_current + activation_lag_blocks`.
  3. قم بمراقبة `/v2/sumeragi/status` حتى الإيقاف `effective_consensus_mode` على طول الطريق.

أي نص برمجي يقوم بإعادة التحقق من الصحة أو استخدام التقطيع، ** لا يتطلب ** التنشيط بدون تأخير أو إيقاف معلمات التسليم؛ يتم إلغاء تشفير هذه المعاملات وإيقافها في النظام السابق.

## أسطح القياس عن بعد

- Prometheus metrics تصدير النشاط الحكم:
  - `governance_proposals_status{status}` (المقياس) يتتبع مجموعة من المقترحات المتعلقة بالحالة.
  - يتم تحديد `governance_protected_namespace_total{outcome}` (العداد) عند السماح/رفض القبول لمساحات الأسماء المحمية.
  - `governance_manifest_activations_total{event}` (العداد) يقوم بإصلاح بيانات البيان (`event="manifest_inserted"`) وروابط مساحة الاسم (`event="instance_bound"`).
- يتضمن `/status` الكائن `governance`، الذي يتضمن مقترحات علامات التبويب، وإدراج الإجماليات في مساحات الأسماء المحمية وإلغاء تنشيطات البيان غير الضرورية (مساحة الاسم، معرف العقد، الكود/تجزئة ABI، ارتفاع الكتلة، الطابع الزمني للتنشيط). يمكن للمشغلين أن يتجاهلوا هذا القطب لمراقبة ما هي التشريعات التي تظهر وما هي بوابات مساحة الاسم المحمية التي تخضع لها.
- يوضح قالب Grafana (`docs/source/grafana_governance_constraints.json`) ودليل القياس عن بعد في `telemetry.md` كيفية إنشاء مراجعة للمقترحات الرائعة وتنشيطات البيان المحتملة أو التجاوزات الجديدة مساحات الأسماء المحمية خلال ترقيات وقت التشغيل.
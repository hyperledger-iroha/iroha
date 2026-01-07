---
id: api
lang: ar
direction: rtl
source: docs/portal/docs/governance/api.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

الحالة: مسودة/تصور لمرافقة مهام تنفيذ الحوكمة. قد تتغير الصيغ اثناء التنفيذ. الحتمية وسياسة RBAC قيود معيارية؛ يمكن لتوريي توقيع/ارسال المعاملات عندما يتم توفير `authority` و`private_key`، والا يبني العملاء المعاملة ويقدمونها الى `/transaction`.

نظرة عامة
- جميع نقاط النهاية تعيد JSON. لمسارات انتاج المعاملات، تتضمن الردود `tx_instructions` - مصفوفة من تعليمة هيكلية واحدة او اكثر:
  - `wire_id`: معرّف السجل لنوع التعليمة
  - `payload_hex`: بايتات حمولة Norito (hex)
- اذا تم توفير `authority` و`private_key` (او `private_key` في DTOs ballots)، يقوم Torii بالتوقيع والارسال ويعيد ايضا `tx_instructions`.
- خلاف ذلك، يجمع العملاء SignedTransaction باستخدام authority وchain_id الخاصة بهم، ثم يوقعون ويرسلون POST الى `/transaction`.
- تغطية SDK:
- Python (`iroha_python`): `ToriiClient.get_governance_proposal_typed` يعيد `GovernanceProposalResult` (يوحد حقول status/kind)، و`ToriiClient.get_governance_referendum_typed` يعيد `GovernanceReferendumResult`، و`ToriiClient.get_governance_tally_typed` يعيد `GovernanceTally`، و`ToriiClient.get_governance_locks_typed` يعيد `GovernanceLocksResult`، و`ToriiClient.get_governance_unlock_stats_typed` يعيد `GovernanceUnlockStats`، و`ToriiClient.list_governance_instances_typed` يعيد `GovernanceInstancesPage`، فارضا وصولا بنمط typed عبر سطح الحوكمة مع امثلة استخدام في README.
- عميل Python خفيف (`iroha_torii_client`): `ToriiClient.finalize_referendum` و`ToriiClient.enact_proposal` يعيدان حزم `GovernanceInstructionDraft` typed (تغلف هيكل `tx_instructions` من Torii)، لتجنب تحليل JSON اليدوي عند تركيب سكربتات Finalize/Enact.
- JavaScript (`@iroha/iroha-js`): `ToriiClient` يعرض helpers typed للمقترحات، والاستفتاءات، وtallies، وlocks، واحصاءات unlock، والان `listGovernanceInstances(namespace, options)` مع نقاط نهاية council (`getGovernanceCouncilCurrent`, `governanceDeriveCouncilVrf`, `governancePersistCouncil`, `getGovernanceCouncilAudit`) كي يتمكن عملاء Node.js من ترقيم الصفحات لـ `/v1/gov/instances/{ns}` وتشغيل تدفقات VRF بجانب سرد مثيلات العقود الموجودة.

نقاط النهاية

- POST `/v1/gov/proposals/deploy-contract`
  - الطلب (JSON):
    {
      "namespace": "apps",
      "contract_id": "my.contract.v1",
      "code_hash": "blake2b32:..." | "...64hex",
      "abi_hash": "blake2b32:..." | "...64hex",
      "abi_version": "1",
      "window": { "lower": 12345, "upper": 12400 },
      "authority": "ih58…@wonderland?",
      "private_key": "...?"
    }
  - الرد (JSON):
    { "ok": true, "proposal_id": "...64hex", "tx_instructions": [{ "wire_id": "...", "payload_hex": "..." }] }
  - التحقق: تقوم العقد بتوحيد `abi_hash` لنسخة `abi_version` المقدمة وترفض عدم التطابق. لـ `abi_version = "v1"` القيمة المتوقعة هي `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))`.

API العقود (deploy)
- POST `/v1/contracts/deploy`
  - الطلب: { "authority": "alice@wonderland", "private_key": "...", "code_b64": "..." }
  - السلوك: يحسب `code_hash` من جسم برنامج IVM و`abi_hash` من ترويسة `abi_version`، ثم يرسل `RegisterSmartContractCode` (manifest) و`RegisterSmartContractBytes` (بايتات `.to` كاملة) نيابة عن `authority`.
  - الرد: { "ok": true, "code_hash_hex": "...", "abi_hash_hex": "..." }
  - ذي صلة:
    - GET `/v1/contracts/code/{code_hash}` -> يعيد manifest المخزن
    - GET `/v1/contracts/code-bytes/{code_hash}` -> يعيد `{ code_b64 }`
- POST `/v1/contracts/instance`
  - الطلب: { "authority": "alice@wonderland", "private_key": "...", "namespace": "apps", "contract_id": "calc.v1", "code_b64": "..." }
  - السلوك: ينشر البايتات المقدمة ويُفعل فورا ربط `(namespace, contract_id)` عبر `ActivateContractInstance`.
  - الرد: { "ok": true, "namespace": "apps", "contract_id": "calc.v1", "code_hash_hex": "...", "abi_hash_hex": "..." }

خدمة الاسماء المستعارة
- POST `/v1/aliases/voprf/evaluate`
  - الطلب: { "blinded_element_hex": "..." }
  - الرد: { "evaluated_element_hex": "...128hex", "backend": "blake2b512-mock" }
    - `backend` يعكس تنفيذ المقيم. القيمة الحالية: `blake2b512-mock`.
  - ملاحظات: مقيم mock حتمي يطبق Blake2b512 مع فصل مجال `iroha.alias.voprf.mock.v1`. مخصص لادوات الاختبار حتى يتم توصيل خط VOPRF الانتاجي عبر Iroha.
  - الاخطاء: HTTP `400` عند ادخال hex غير صالح. يعيد Torii ظرف Norito `ValidationFail::QueryFailed::Conversion` مع رسالة خطا من decoder.
- POST `/v1/aliases/resolve`
  - الطلب: { "alias": "GB82 WEST 1234 5698 7654 32" }
  - الرد: { "alias": "GB82WEST12345698765432", "account_id": "...@...", "index": 0, "source": "iso_bridge" }
  - ملاحظات: يتطلب تشغيل ISO bridge (`[iso_bridge.account_aliases]` في `iroha_config`). يقوم Torii بتطبيع الاسماء عبر ازالة الفراغات وتحويلها الى احرف كبيرة قبل البحث. يعيد 404 عندما يكون الاسم غير موجود و503 عندما يكون runtime الخاص بـ ISO bridge معطلا.
- POST `/v1/aliases/resolve_index`
  - الطلب: { "index": 0 }
  - الرد: { "index": 0, "alias": "GB82WEST12345698765432", "account_id": "...@...", "source": "iso_bridge" }
  - ملاحظات: مؤشرات الاسماء تعين بشكل حتمي حسب ترتيب التكوين (0-based). يمكن للعملاء تخزين الردود offline لبناء مسارات تدقيق لاحداث attestation الخاصة بالاسماء.

حد حجم الكود
- المعامل المخصص: `max_contract_code_bytes` (JSON u64)
  - يتحكم في الحد الاقصى المسموح (بالبايت) لتخزين كود العقود على السلسلة.
  - الافتراضي: 16 MiB. ترفض العقد `RegisterSmartContractBytes` عندما يتجاوز حجم صورة `.to` الحد مع خطا انتهاك invariant.
  - يمكن للمشغلين التعديل عبر `SetParameter(Custom)` مع `id = "max_contract_code_bytes"` وحمولة رقمية.

- POST `/v1/gov/ballots/zk`
  - الطلب: { "authority": "alice@wonderland", "private_key": "...?", "chain_id": "...", "election_id": "e1", "proof_b64": "...", "public": {...} }
  - الرد: { "ok": true, "accepted": true, "tx_instructions": [{...}] }
  - ملاحظات:
    - عندما تتضمن المدخلات العامة للدائرة `owner` و`amount` و`duration_blocks`، وتتحقق البرهان من VK المضبوطة، ينشئ العقدة او يمدد lock للحوكمة لـ `election_id` مع ذلك `owner`. تبقى الاتجاهات مخفية (`unknown`)؛ ويتم تحديث amount/expiry فقط. اعادات التصويت monotonic: amount وexpiry تزداد فقط (تطبق العقدة max(amount, prev.amount) وmax(expiry, prev.expiry)).
    - اعادات التصويت ZK التي تحاول تقليل amount او expiry يتم رفضها من جهة الخادم مع تشخيصات `BallotRejected`.
    - يجب على تنفيذ العقد استدعاء `ZK_VOTE_VERIFY_BALLOT` قبل ادراج `SubmitBallot`; ويفرض المضيفون latch لمرة واحدة.

- POST `/v1/gov/ballots/plain`
  - الطلب: { "authority": "alice@domain", "private_key": "...?", "chain_id": "...", "referendum_id": "r1", "owner": "alice@domain", "amount": "1000", "duration_blocks": 6000, "direction": "Aye|Nay|Abstain" }
  - الرد: { "ok": true, "accepted": true, "tx_instructions": [{...}] }
  - ملاحظات: اعادات التصويت تمتد فقط - لا يمكن لballot جديد تقليل amount او expiry لقفل موجود. يجب ان يساوي `owner` سلطة المعاملة. الحد الادنى للمدة هو `conviction_step_blocks`.

- POST `/v1/gov/finalize`
  - الطلب: { "referendum_id": "r1", "proposal_id": "...64hex", "authority": "ih58…@wonderland?", "private_key": "...?" }
  - الرد: { "ok": true, "tx_instructions": [{ "wire_id": "...FinalizeReferendum", "payload_hex": "..." }] }
  - الاثر على السلسلة (الهيكل الحالي): تنفيذ اقتراح deploy معتمد يدرج `ContractManifest` ادنى بمفتاح `code_hash` مع `abi_hash` المتوقع ويضع الاقتراح بحالة Enacted. اذا كان هناك manifest موجود لـ `code_hash` بقيمة `abi_hash` مختلفة، يتم رفض التنفيذ.
  - ملاحظات:
    - لانتخابات ZK، يجب على مسارات العقد استدعاء `ZK_VOTE_VERIFY_TALLY` قبل تنفيذ `FinalizeElection`; ويفرض المضيفون latch لمرة واحدة. يرفض `FinalizeReferendum` الاستفتاءات ZK حتى يتم انهاء tally للانتخابات.
    - الاغلاق التلقائي عند `h_end` يصدر Approved/Rejected فقط للاستفتاءات Plain؛ تبقى استفتاءات ZK Closed حتى يتم ارسال tally منته ويجري تنفيذ `FinalizeReferendum`.
    - فحوصات turnout تستخدم approve+reject فقط؛ abstain لا يحتسب ضمن turnout.

- POST `/v1/gov/enact`
  - الطلب: { "proposal_id": "...64hex", "preimage_hash": "...64hex?", "window": { "lower": 0, "upper": 0 }?, "authority": "ih58…@wonderland?", "private_key": "...?" }
  - الرد: { "ok": true, "tx_instructions": [{ "wire_id": "...EnactReferendum", "payload_hex": "..." }] }
  - ملاحظات: يقدم Torii المعاملة الموقعة عندما تتوفر `authority`/`private_key`; والا يعيد هيكلا لتوقيع العملاء وارساله. الـ preimage اختيارية وحاليا معلوماتية.

- GET `/v1/gov/proposals/{id}`
  - المسار `{id}`: معرّف الاقتراح hex (64 chars)
  - الرد: { "found": bool, "proposal": { ... }? }

- GET `/v1/gov/locks/{rid}`
  - المسار `{rid}`: string لمعرف الاستفتاء
  - الرد: { "found": bool, "referendum_id": "rid", "locks": { ... }? }

- GET `/v1/gov/council/current`
  - الرد: { "epoch": N, "members": [{ "account_id": "..." }, ...] }
  - ملاحظات: يعيد council المحفوظ اذا كان موجودا؛ والا يشتق بديلا حتميا باستخدام اصل stake المضبوط والعتبات (يعكس مواصفات VRF حتى تثبت ادلة VRF الحية على السلسلة).

- POST `/v1/gov/council/derive-vrf` (feature: gov_vrf)
  - الطلب: { "committee_size": 21, "epoch": 123? , "candidates": [{ "account_id": "...", "variant": "Normal|Small", "pk_b64": "...", "proof_b64": "..." }, ...] }
  - السلوك: يتحقق من برهان VRF لكل مرشح مقابل المدخل القانوني المشتق من `chain_id` و`epoch` ومنارة اخر hash للبلوك؛ يرتب حسب bytes الخرج desc مع كاسرات تعادل؛ ويعيد اعلى `committee_size` من الاعضاء. لا يتم الحفظ.
  - الرد: { "epoch": N, "members": [{ "account_id": "..." } ...], "total_candidates": M, "verified": K }
  - ملاحظات: Normal = pk في G1، proof في G2 (96 bytes). Small = pk في G2، proof في G1 (48 bytes). المدخلات مفصولة بالمجال وتضم `chain_id`.

### افتراضات الحوكمة (iroha_config `gov.*`)

يتم ضبط council الاحتياطي الذي يستخدمه Torii عندما لا يوجد roster محفوظ عبر `iroha_config`:

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

بدائل البيئة المكافئة:

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

`parliament_committee_size` يحد عدد اعضاء fallback المعادين عندما لا يوجد council محفوظ، و`parliament_term_blocks` يحدد طول الحقبة المستخدمة لاشتقاق seed (`epoch = floor(height / term_blocks)`)، و`parliament_min_stake` يفرض الحد الادنى من stake (بوحدات صغرى) على اصل الاهلية، و`parliament_eligibility_asset_id` يحدد اي رصيد اصل يتم مسحه عند بناء مجموعة المرشحين.

تحقق VK للحوكمة بلا bypass: التحقق من ballots يتطلب دائما مفتاح تحقق `Active` ببايتات inline، ولا يجب ان تعتمد البيئات على تبديلات اختبارية لتخطي التحقق.

RBAC
- التنفيذ على السلسلة يتطلب صلاحيات:
  - Proposals: `CanProposeContractDeployment{ contract_id }`
  - Ballots: `CanSubmitGovernanceBallot{ referendum_id }`
  - Enactment: `CanEnactGovernance`
  - Council management (مستقبلا): `CanManageParliament`

Namespaces محمية
- المعامل المخصص `gov_protected_namespaces` (JSON array من strings) يفعل gating لعمليات deploy الى namespaces المحددة.
- على العملاء تضمين مفاتيح metadata للمعاملة عند deploy الى namespaces المحمية:
  - `gov_namespace`: namespace الهدف (مثلا "apps")
  - `gov_contract_id`: معرف العقد المنطقي داخل namespace
- `gov_manifest_approvers`: JSON array اختياري من account IDs للمدققين. عندما يعلن manifest لمسار ما quorum اكبر من واحد، يتطلب admission سلطة المعاملة بالاضافة الى الحسابات المدرجة لتلبية quorum الخاص بالmanifest.
- تكشف التليمترية عدادات admission عبر `governance_manifest_admission_total{result}` لتمييز القبولات الناجحة عن مسارات `missing_manifest` و`non_validator_authority` و`quorum_rejected` و`protected_namespace_rejected` و`runtime_hook_rejected`.
- تكشف التليمترية مسار enforcement عبر `governance_manifest_quorum_total{outcome}` (القيم `satisfied` / `rejected`) حتى يتمكن المشغلون من تدقيق الموافقات المفقودة.
- تفرض المسارات allowlist الخاصة بـ namespaces المنشورة في manifests. اي معاملة تعين `gov_namespace` يجب ان توفر `gov_contract_id`، ويجب ان يظهر namespace في مجموعة `protected_namespaces` للmanifest. يتم رفض ارسال `RegisterSmartContractCode` بدون هذه metadata عندما تكون الحماية مفعلة.
- يفرض admission وجود اقتراح حوكمة Enacted للتركيبة `(namespace, contract_id, code_hash, abi_hash)`؛ والا تفشل عملية التحقق بخطا NotPermitted.

Hooks ترقية runtime
- يمكن لـ manifests الخاصة بالمسار اعلان `hooks.runtime_upgrade` لبوابة تعليمات ترقية runtime (`ProposeRuntimeUpgrade`, `ActivateRuntimeUpgrade`, `CancelRuntimeUpgrade`).
- حقول hook:
  - `allow` (bool, الافتراضي `true`): عندما تكون `false` يتم رفض جميع تعليمات ترقية runtime.
  - `require_metadata` (bool, الافتراضي `false`): يتطلب مدخل metadata المحدد بواسطة `metadata_key`.
  - `metadata_key` (string): اسم metadata الذي يفرضه hook. الافتراضي `gov_upgrade_id` عندما تكون metadata مطلوبة او هناك allowlist.
  - `allowed_ids` (array من strings): allowlist اختياري لقيم metadata (بعد trim). يرفض عندما لا تكون القيمة المقدمة مدرجة.
- عندما يكون hook موجودا، يفرض admission في الطابور سياسة metadata قبل دخول المعاملة للطابور. metadata المفقودة، القيم الفارغة، او القيم خارج allowlist تنتج خطا NotPermitted حتميا.
- تتابع التليمترية النتائج عبر `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}`.
- المعاملات التي تفي بالhook يجب ان تتضمن metadata `gov_upgrade_id=<value>` (او المفتاح المحدد في manifest) مع اي موافقات مدققين مطلوبة بواسطة quorum الخاص بالmanifest.

Endpoint للراحة
- POST `/v1/gov/protected-namespaces` - يطبق `gov_protected_namespaces` مباشرة على العقدة.
  - الطلب: { "namespaces": ["apps", "system"] }
  - الرد: { "ok": true, "applied": 1 }
  - ملاحظات: مخصص للادارة/الاختبار؛ يتطلب API token اذا كان مضبوطا. للانتاج، يفضل ارسال معاملة موقعة مع `SetParameter(Custom)`.

مساعدات CLI
- `iroha gov audit-deploy --namespace apps [--contains calc --hash-prefix deadbeef --summary-only]`
  - يجلب مثيلات العقود للnamespace ويتحقق من:
    - Torii يخزن bytecode لكل `code_hash`، وان digest Blake2b-32 يطابق `code_hash`.
    - manifest المخزن تحت `/v1/contracts/code/{code_hash}` يبلغ بقيم `code_hash` و`abi_hash` متطابقة.
    - يوجد اقتراح حوكمة enacted للتركيبة `(namespace, contract_id, code_hash, abi_hash)` مشتق بنفس hashing proposal-id الذي تستخدمه العقدة.
  - يخرج تقرير JSON يحتوي `results[]` لكل عقد (issues، ملخصات manifest/code/proposal) بالاضافة الى ملخص سطر واحد الا اذا تم تعطيله (`--no-summary`).
  - مفيد لتدقيق namespaces المحمية او التحقق من تدفقات deploy الخاضعة للحوكمة.
- `iroha gov deploy-meta --namespace apps --contract-id calc.v1 [--approver validator@wonderland --approver bob@wonderland]`
  - يصدر هيكل JSON للـ metadata المستخدم عند نشر deploy الى namespaces المحمية، بما في ذلك `gov_manifest_approvers` الاختيارية لتلبية قواعد quorum للmanifest.
- `iroha gov vote-zk --election-id <id> --proof-b64 <b64> [--owner <account>@<domain> --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]` — تلميحات القفل مطلوبة عندما يكون `min_bond_amount > 0`، وأي مجموعة تلميحات مقدمة يجب أن تتضمن `owner` و`amount` و`duration_blocks`.
  - Validates canonical account ids, canonicalizes 32-byte nullifier hints, and merges the hints into `public_inputs_json` (with `--public <path>` for additional overrides).
  - The nullifier is derived from the proof commitment (public input) plus `domain_tag`, `chain_id`, and `election_id`; `--nullifier` is validated against the proof when supplied.
  - الملخص في سطر واحد يعرض الان `fingerprint=<hex>` حتميا مشتقا من `CastZkBallot` المشفر مع hints المفككة (`owner`, `amount`, `duration_blocks`, `direction` عند توفيرها).
  - ردود CLI تضع تعليقات على `tx_instructions[]` مع `payload_fingerprint_hex` بالاضافة الى حقول مفكوكة كي تتحقق الادوات اللاحقة من الهيكل بدون اعادة تنفيذ فك ترميز Norito.
  - توفير hints للـ lock يسمح للعقدة باصدار احداث `LockCreated`/`LockExtended` للـ ZK ballots بمجرد ان تكشف الدائرة القيم نفسها.
- `iroha gov vote-plain --referendum-id <id> --owner <account>@<domain> --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - الـ aliases `--lock-amount`/`--lock-duration-blocks` تعكس اسماء flags الخاصة بـ ZK لتحقيق تماثل السكربتات.
  - ناتج الملخص يعكس `vote-zk` باضافة fingerprint للتعليمة المشفرة وحقول ballot المقروءة (`owner`, `amount`, `duration_blocks`, `direction`)، لتاكيد سريع قبل توقيع الهيكل.

قائمة المثيلات
- GET `/v1/gov/instances/{ns}` - يسرد مثيلات العقود النشطة لnamespace.
  - Query params:
    - `contains`: تصفية بحسب substring من `contract_id` (case-sensitive)
    - `hash_prefix`: تصفية بحسب بادئة hex لـ `code_hash_hex` (lowercase)
    - `offset` (default 0), `limit` (default 100, max 10_000)
    - `order`: واحد من `cid_asc` (default), `cid_desc`, `hash_asc`, `hash_desc`
  - الرد: { "namespace": "ns", "instances": [{ "contract_id": "...", "code_hash_hex": "..." }, ...], "total": N, "offset": n, "limit": m }
  - Helper SDK: `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) او `ToriiClient.list_governance_instances_typed("apps", ...)` (Python).

مسح unlocks (المشغل/التدقيق)
- GET `/v1/gov/unlocks/stats`
  - الرد: { "height_current": H, "expired_locks_now": n, "referenda_with_expired": m, "last_sweep_height": S }
  - ملاحظات: `last_sweep_height` يعكس اخر ارتفاع بلوك تم فيه مسح locks منتهية وتخزينها. `expired_locks_now` يحسب عبر مسح سجلات lock ذات `expiry_height <= height_current`.
- POST `/v1/gov/ballots/zk-v1`
  - الطلب (DTO نمط v1):
    {
      "authority": "alice@wonderland",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "...?",
      "election_id": "ref-1",
      "backend": "halo2/ipa",
      "envelope_b64": "AAECAwQ=",
      "root_hint": "0x...64hex?",
      "owner": "ih58…@wonderland?",
      "nullifier": "blake2b32:...64hex?"
    }
  - الرد: { "ok": true, "accepted": true, "tx_instructions": [{...}] }

- POST `/v1/gov/ballots/zk-v1/ballot-proof` (feature: `zk-ballot`)
  - يقبل JSON `BallotProof` مباشرة ويعيد هيكل `CastZkBallot`.
  - الطلب:
    {
      "authority": "alice@wonderland",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "...?",
      "election_id": "ref-1",
      "ballot": {
        "backend": "halo2/ipa",
        "envelope_bytes": "AAECAwQ=",   // base64 لحاوية ZK1 او H2*
        "root_hint": null,                // optional 32-byte hex string (eligibility root)
        "owner": null,                    // AccountId اختياري عندما تلتزم الدائرة بـ owner
        "nullifier": null                 // optional 32-byte hex string (nullifier hint)
      }
    }
  - الرد:
    {
      "ok": true,
      "accepted": true,
      "reason": "build transaction skeleton",
      "tx_instructions": [
        { "wire_id": "CastZkBallot", "payload_hex": "..." }
      ]
    }
  - ملاحظات:
    - يقوم الخادم بربط `root_hint`/`owner`/`nullifier` الاختيارية من ballot الى `public_inputs_json` لـ `CastZkBallot`.
    - يعاد ترميز bytes الخاصة بالenvelope كـ base64 في payload التعليمة.
    - تتغير `reason` الى `submitted transaction` عندما يقدم Torii ballot.
    - هذا endpoint متاح فقط عندما يكون feature `zk-ballot` مفعلا.

مسار التحقق من CastZkBallot
- `CastZkBallot` يفك ترميز برهان base64 المقدم ويرفض الحمولات الفارغة او المشوهة (`BallotRejected` مع `invalid or empty proof`).
- المضيف يحل مفتاح التحقق للballot من referendum (`vk_ballot`) او افتراضات الحوكمة ويتطلب ان يكون السجل موجودا و`Active` ويحمل bytes inline.
- bytes الخاصة بمفتاح التحقق المخزن يعاد hashing لها عبر `hash_vk`; اي عدم تطابق للالتزام يوقف التنفيذ قبل التحقق للحماية من ادخالات سجل معبث بها (`BallotRejected` مع `verifying key commitment mismatch`).
- bytes الخاصة بالبرهان ترسل الى backend المسجل عبر `zk::verify_backend`; تظهر النصوص غير الصالحة كـ `BallotRejected` مع `invalid proof` وتفشل التعليمة بشكل حتمي.
- The proof must expose a ballot commitment and eligibility root as public inputs; the root must match the election’s `eligible_root`, and the derived nullifier must match any provided hint.
- البراهين الناجحة تصدر `BallotAccepted`; nullifiers المكررة او جذور اهلية قديمة او تراجعات lock تستمر في انتاج اسباب الرفض المذكورة سابقا في هذا المستند.

## سوء سلوك المدققين والتوافق المشترك

### سير عمل slashing وjailing

يصدر الاجماع `Evidence` مرمزا بـ Norito عندما ينتهك مدقق البروتوكول. تصل كل حمولة الى `EvidenceStore` في الذاكرة، وان لم تكن معروفة يتم تجسيدها في خريطة `consensus_evidence` المدعومة بـ WSV. يتم رفض السجلات الاقدم من `sumeragi.npos.reconfig.evidence_horizon_blocks` (الافتراضي `7200` بلوك) كي يبقى الارشيف محدودا، لكن الرفض يسجل للمشغلين.

الانتهاكات المعترف بها تقابل واحدا لواحد مع `EvidenceKind`; المميزات ثابتة ويفرضها نموذج البيانات:

```rust
use iroha_data_model::block::consensus::EvidenceKind;

let offences = [
    EvidenceKind::DoublePrepare,
    EvidenceKind::DoubleCommit,
    EvidenceKind::InvalidCommitCertificate,
    EvidenceKind::InvalidProposal,
    EvidenceKind::DoubleExecVote,
];

for (expected, kind) in offences.iter().enumerate() {
    assert_eq!(*kind as u16, expected as u16);
}
```

- **DoublePrepare/DoubleCommit** - المدقق وقع hashes متعارضة لنفس tuple `(phase,height,view,epoch)`.
- **DoubleExecVote** - تصويتات تنفيذ متعارضة تعلن جذور حالة لاحقة مختلفة.
- **InvalidCommitCertificate** - قام مجمع ببث commit certificate شكله يفشل الفحوص الحتمية (مثلا bitmap موقعين فارغ).
- **InvalidProposal** - قدم قائد بلوكا يفشل التحقق البنيوي (مثلا يكسر قاعدة locked-chain).

يمكن للمشغلين والادوات فحص واعادة بث الحمولات عبر:

- Torii: `GET /v1/sumeragi/evidence` و`GET /v1/sumeragi/evidence/count`.
- CLI: `iroha sumeragi evidence list`, `... count`, و`... submit --evidence-hex <payload>`.

يجب على الحوكمة اعتبار bytes الخاصة بالevidence دليلا قانونيا:

1. **جمع الحمولة** قبل ان تتقادم. ارشفة bytes Norito الخام مع metadata الخاصة بـ height/view.
2. **تجهيز العقوبة** عبر تضمين الحمولة في referendum او تعليمة sudo (مثل `Unregister::peer`). تعيد عملية التنفيذ التحقق من الحمولة؛ evidence المشوهة او القديمة ترفض حتميا.
3. **جدولة طوبولوجيا المتابعة** حتى لا يتمكن المدقق المخالف من العودة فورا. التدفقات النموذجية تضع `SetParameter(Sumeragi::NextMode)` و`SetParameter(Sumeragi::ModeActivationHeight)` مع roster محدث.
4. **تدقيق النتائج** عبر `/v1/sumeragi/evidence` و`/v1/sumeragi/status` لضمان ان عداد evidence تقدم وان الحوكمة طبقت الازالة.

### تسلسل الاجماع المشترك

يضمن الاجماع المشترك ان تقوم مجموعة المدققين الخارجة بانهاء بلوك الحد قبل ان تبدا المجموعة الجديدة بالاقتراح. يفرض الـ runtime القاعدة عبر معاملات مزدوجة:

- يجب ان يتم الالتزام بـ `SumeragiParameter::NextMode` و`SumeragiParameter::ModeActivationHeight` في **نفس البلوك**. يجب ان تكون `mode_activation_height` اكبر تماما من ارتفاع البلوك الذي حمل التحديث، بما يوفر على الاقل بلوكا واحدا من lag.
- `sumeragi.npos.reconfig.activation_lag_blocks` (الافتراضي `1`) هو guard تهيئة يمنع hand-offs بدون lag:

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```

- يعرض الـ runtime وCLI المعاملات staged عبر `/v1/sumeragi/params` و`iroha sumeragi params --summary` حتى يتمكن المشغلون من التحقق من ارتفاعات التفعيل وقوائم المدققين.
- يجب ان تقوم اتمتة الحوكمة دائما بما يلي:
  1. انهاء قرار الازالة (او الاستعادة) المدعوم بـ evidence.
  2. جدولة اعادة تهيئة متابعة مع `mode_activation_height = h_current + activation_lag_blocks`.
  3. مراقبة `/v1/sumeragi/status` حتى يتبدل `effective_consensus_mode` عند الارتفاع المتوقع.

اي سكربت يدير تدوير المدققين او يطبق slashing **يجب الا** يحاول تفعيل بدون lag او يحذف معاملات hand-off؛ يتم رفض تلك المعاملات وتترك الشبكة على الوضع السابق.

## اسطح التليمترية

- تصدر مقاييس Prometheus نشاط الحوكمة:
  - `governance_proposals_status{status}` (gauge) يتتبع تعداد المقترحات حسب الحالة.
  - `governance_protected_namespace_total{outcome}` (counter) يزيد عندما يسمح او يرفض admission لنشر ضمن namespaces محمية.
  - `governance_manifest_activations_total{event}` (counter) يسجل ادخالات manifest (`event="manifest_inserted"`) وربط namespace (`event="instance_bound"`).
- `/status` يتضمن كائنا `governance` يعكس تعداد المقترحات، ويبلغ عن اجمالي namespaces المحمية، ويسرد تفعيلات manifest الاخيرة (namespace، contract id، code/ABI hash، block height، activation timestamp). يمكن للمشغلين استطلاع هذا الحقل لتاكيد ان enactments حدثت manifests وان بوابات namespaces المحمية مطبقة.
- قالب Grafana (`docs/source/grafana_governance_constraints.json`) وrunbook التليمترية في `telemetry.md` يوضحان كيفية ربط التنبيهات للمقترحات العالقة، وتفعيلات manifest المفقودة، او رفضات namespaces المحمية غير المتوقعة اثناء ترقيات runtime.

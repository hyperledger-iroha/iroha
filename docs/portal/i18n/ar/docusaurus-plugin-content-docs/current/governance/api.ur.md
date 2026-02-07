---
lang: ar
direction: rtl
source: docs/portal/docs/governance/api.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

الحالة: نفاذية كامو مصنوعة من مادة كربونية وألياف/اسكيتش. تم التعامل مع التعاملات المختلفة مع التكنولوجيات الجديدة. الحتمية و RBAC بالياري پاپندياں؛ قم بالبدء في `authority` و`private_key` إلى Torii عبر الإنترنت عبر الإنترنت عبر الإنترنت عبر الإنترنت إلى `/transaction` كرتے ہيں.

جائزة
- جميع نقاط النهاية JSON واپس کرتے ہیں. تتضمن ٹرانزیکشن بنانے وے فلو کے لئے إجابات `tx_instructions` مجموعة من الهياكل العظمية للتعليمات متعددة أو متعددة:
  - `wire_id`: تعليمات معرف التسجيل
  - `payload_hex`: Norito بايت الحمولة (ست عشري)
- إذا كان `authority` و`private_key` (أو DTOs الاقتراع `private_key`) فانتقل إلى Torii عبر الإنترنت وبطاقة الائتمان والمزيد `tx_instructions` هاتف محمول.
- تم التحقق من سلطة شبكة العملاء ومعرف السلسلة الذي تم توقيعه من خلال المعاملة الموقعة، عبر الإنترنت عبر `/transaction` في بطاقة POST.
- تغطية SDK:
- بايثون (`iroha_python`): `ToriiClient.get_governance_proposal_typed` `GovernanceProposalResult` واپس کرتا ہے (حقول الحالة/النوع کو تطبيع کرتا ہے)، `ToriiClient.get_governance_referendum_typed` `GovernanceReferendumResult` واپس کرتا ہے، `ToriiClient.get_governance_tally_typed` `GovernanceTally` بطاقة الاتصال، `ToriiClient.get_governance_locks_typed` `GovernanceLocksResult` بطاقة الاتصال، `ToriiClient.get_governance_unlock_stats_typed` `GovernanceUnlockStats` بطاقة الاتصال، و `ToriiClient.list_governance_instances_typed` `GovernanceInstancesPage` وابس كرتا، جس سے بورے سطح الإدارة، الوصول المكتوب ملتا ہے وقراءة أمثلة الاستخدام.
- عميل Python خفيف الوزن (`iroha_torii_client`): `ToriiClient.finalize_referendum` و`ToriiClient.enact_proposal` المكتوب بحزم `GovernanceInstructionDraft` وحزم الورق (Torii أو `tx_instructions` الهيكل العظمي الملتف) تم الانتهاء من ذلك)، هذه البرامج النصية إنهاء/تفعيل التدفقات بنات وقت تحليل JSON اليدوي.
- JavaScript (`@iroha/iroha-js`): مقترحات `ToriiClient`، والاستفتاءات، والإحصاءات، والأقفال، وفتح الإحصائيات کے لئے المساعدين المكتوبين دیتا ہے، اور اب `listGovernanceInstances(namespace, options)` کے ساتھ نقاط نهاية المجلس (`getGovernanceCouncilCurrent`, `governanceDeriveCouncilVrf`، `governancePersistCouncil`، `getGovernanceCouncilAudit`) بالإضافة إلى عملاء Node.js `/v1/gov/instances/{ns}` الذين يقومون بترقيم الصفحات ومهام سير العمل المدعومة بـ VRF وقائمة مثيلات العقد المتاحة چلا سکيں.

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
  - الاستجابة (JSON):
    { "ok": صحيح، "proposal_id": "...64hex"، "tx_instructions": [{ "wire_id": "..."، "payload_hex": "..." }] }
  - التحقق من الصحة: العقد من `abi_version` إلى `abi_hash` تقوم بتحديد البطاقة وعدم تطابقها لرفض البطاقة. `abi_version = "v1"` هذه القيمة المتوقعة `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))` .

واجهة برمجة تطبيقات العقود (النشر)
- المشاركة `/v1/contracts/deploy`
  - الطلب: { "authority": "ih58..."، "private_key": "..."، "code_b64": "..." }
  - السلوك: IVM برنامج `code_hash` والرأس `abi_version` Sے `abi_hash` نکالتا ہے، ھر `RegisterSmartContractCode` (البيان) و `RegisterSmartContractBytes` (مكمل `.to` بايت) `authority` طرف سپمٹ كرتا.
  - الرد: { "ok": صحيح، "code_hash_hex": "..."، "abi_hash_hex": "..." }
  - ذات صلة:
    - احصل على `/v1/contracts/code/{code_hash}` -> سجل بيانات البيان
    - احصل على `/v1/contracts/code-bytes/{code_hash}` -> `{ code_b64 }`
- المشاركة `/v1/contracts/instance`
  - الطلب: { "authority": "ih58..."، "private_key": "..."، "مساحة الاسم": "apps"، "contract_id": "calc.v1"، "code_b64": "..." }
  - السلوك: نشر رمز البايت كود كرتا و`ActivateContractInstance` ذریعے `(namespace, contract_id)` رسم الخرائط فعالا کرتا ہے.
  - الاستجابة: { "ok": true، "namespace": "apps"، "contract_id": "calc.v1"، "code_hash_hex": "..."، "abi_hash_hex": "..." }

خدمة الاسم المستعار
- المشاركة `/v1/aliases/voprf/evaluate`
  - الطلب: { "blinded_element_hex": "..." }
  - الاستجابة: { "evaluated_element_hex": "...128hex"، "backend": "blake2b512-mock" }
    - `backend` تنفيذ المقيم ظہر کرتا ہے۔ القيمة الموجودة: `blake2b512-mock`۔
  - ملاحظات: المقيم الوهمي الحتمي جو Blake2b512 کو فصل المجال `iroha.alias.voprf.mock.v1` کے ساتھ تطبيق کرتا ہے۔ يتم استخدام أدوات الاختبار هذه عند إنتاج خط أنابيب VOPRF Iroha ولا يوجد سلك.
  - الأخطاء: إدخال سداسي عشري مشوه پر HTTP `400`۔ Torii Norito `ValidationFail::QueryFailed::Conversion` رسالة خطأ في المغلف ووحدة فك التشفير واپس کرتا ہے۔
- المشاركة `/v1/aliases/resolve`
  - الطلب: { "الاسم المستعار": "GB82 WEST 1234 5698 7654 32" }
  - الاستجابة: { "الاسم المستعار": "GB82WEST12345698765432"، "account_id": "ih58..."، "index": 0، "source": "iso_bridge" }
  - ملاحظات: مرحلة تشغيل جسر ISO درکار ہے (`[iso_bridge.account_aliases]` في `iroha_config`). Torii مسافة بيضاء وأحرف كبيرة للبحث هنا. الاسم المستعار ليس 404 ووقت تشغيل جسر ISO هو 503 دیتا.
- المشاركة `/v1/aliases/resolve_index`
  - الطلب: { "الفهرس": 0 }
  - الاستجابة: { "الفهرس": 0، "الاسم المستعار": "GB82WEST12345698765432"، "account_id": "ih58..."، "source": "iso_bridge" }
  - ملاحظات: ترتيب تكوين المؤشرات الاسم المستعار يتوافق مع طریقے سے تعيين ہوتے ہیں (على أساس 0). ذاكرة التخزين المؤقت للشبكة دون اتصال بالإنترنت أحداث التصديق على الاسم المستعار مسارات التدقيق الخاصة بنا.

الحد الأقصى لحجم الرمز
- المعلمة المخصصة: `max_contract_code_bytes` (JSON u64)
  - تخزين أكواد العقد عبر السلسلة لعدد كبير جدًا من الجوائز (البايتات) من مركز التحكم.
  - الافتراضي: 16 ميجا بايت۔ لا تزال الصورة `.to` تحتوي على عقد `RegisterSmartContractBytes` وهي خطأ انتهاك ثابت وترفض البطاقة.
  - عوامل التشغيل `SetParameter(Custom)` هي `id = "max_contract_code_bytes"` والحمولة الرقمية.

- المشاركة `/v1/gov/ballots/zk`
  - الطلب: { "authority": "ih58..."، "private_key": "...؟"، "chain_id": "..."، "election_id": "e1"، "proof_b64": "..."، "public": {...} }
  - الاستجابة: { "موافق": صحيح، "مقبول": صحيح، "tx_instructions": [{...}] }
  - ملاحظات:
    - تتضمن دائرة المدخلات العامة `owner`, `amount`, `duration_blocks` خوارزمية وإثبات تكوين VK الذي يمنع التحقق من العقدة `election_id` لقفل الحوكمة لدينا أو بڑهاتا ہے۔ اتجاه چھپی رہتی ہے (`unknown`); تم تحديد المبلغ/انتهاء الصلاحية فقط. إعادة التصويت رتيبة: المبلغ وانتهاء الصلاحية يصرفان (العقدة القصوى (المبلغ، المبلغ السابق) والحد الأقصى (انتهاء الصلاحية، السابق.انتهاء الصلاحية) لغتا).
    - يقوم ZK بإعادة التصويت على المبلغ أو انتهاء صلاحية تشخيصات `BallotRejected` من جانب الخادم ثم يتم رفضه.
    - تنفيذ العقد کو `SubmitBallot` أدرج کرنے سے پہلے `ZK_VOTE_VERIFY_BALLOT` کال کرنا المطلوب ہے؛ المضيفون يفرضون مزلاج طلقة واحدة.

- المشاركة `/v1/gov/ballots/plain`
  - الطلب: { "authority": "ih58..."، "private_key": "...؟"، "chain_id": "..."، "referendum_id": "r1"، "owner": "ih58..."، "amount": "1000"، "duration_blocks": 6000، "direction": "Aye|Nay|امتناع" }
  - الاستجابة: { "موافق": صحيح، "مقبول": صحيح، "tx_instructions": [{...}] }
  - ملاحظات: إعادة التصويت للتمديد فقط - لا يوجد قفل للمبلغ أو انتهاء الصلاحية لبطاقة الاقتراع الحالية. `owner` هي سلطة المعاملات التي يمكنك القيام بها. كم مدت `conviction_step_blocks` .

- المشاركة `/v1/gov/finalize`
  - الطلب: { "referendum_id": "r1"، "proposal_id": "...64hex"، "authority": "ih58...؟"، "private_key": "...؟" }
  - الاستجابة: { "ok": true، "tx_instructions": [{ "wire_id": "...FinalizeReferendum"، "payload_hex": "..." }] }
  - التأثير على السلسلة (السقالة الحالية): تم نشر اقتراح لتفعيل `code_hash` الحد الأدنى من المفاتيح `ContractManifest` يشمل العرض الجديد وتفعيل `abi_hash` واقتراح تم تفعيله ہے۔ إذا كان `code_hash` مختلفًا `abi_hash` والبيان واضحًا، فما عليك سوى رفض التشريع.
  - ملاحظات:
    - انتخابات ZK مسارات العقد `FinalizeElection` سے پہلے `ZK_VOTE_VERIFY_TALLY` كال كرنا المطلوبة ہے؛ المضيفون يفرضون مزلاج طلقة واحدة. `FinalizeReferendum` استفتاءات ZK التي تم رفضها في الوقت الحالي ولم يتم الانتهاء من حصيلة الانتخابات بعد.
    - الإغلاق التلقائي `h_end` استفتاءات عادية تمت الموافقة عليها/رفض الانبعاثات؛ استفتاءات ZK تم إغلاق المراجعة بعد الانتهاء من إرسال الحصيلة النهائية ولم يتم تنفيذ `FinalizeReferendum`.
    - الشيكات الإقبال فقط الموافقة على رفض الاستخدام؛ الامتناع عن التصويت لا يهم.

- المشاركة `/v1/gov/enact`
  - الطلب: { "proposal_id": "...64hex"، "preimage_hash": "...64hex؟"، "window": { "lower": 0، "upper": 0}؟، "authority": "ih58...؟"، "private_key": "...؟" }
  - الاستجابة: { "ok": true، "tx_instructions": [{ "wire_id": "...EnactReferendum"، "payload_hex": "..." }] }
  - ملاحظات: جب `authority`/`private_key` فراہم ہوں تو Torii معاملة موقعة سبمٹ کرتا ہے؛ لقد تم إنشاء برنامج كمبيوتر محمول وهيكل عظمي لشبكة الإنترنت والإنترنت. Preimage اختيار وحقيقة معلومات ہے۔

- احصل على `/v1/gov/proposals/{id}`
  - المسار `{id}`: معرف الاقتراح الست عشري (64 حرفًا)
  - الاستجابة: { "تم العثور عليه": منطقي، "اقتراح": { ... }؟ }

- احصل على `/v1/gov/locks/{rid}`
  - المسار `{rid}`: سلسلة معرف الاستفتاء
  - الاستجابة: { "تم العثور عليه": منطقي، "referendum_id": "rid"، "أقفال": {... }؟ }

- احصل على `/v1/gov/council/current`
  - الاستجابة: { "العصر": N، "الأعضاء": [{ "account_id": "..." }، ...] }
  - ملاحظات: المجلس موجود وموجود ومثبت أصول الحصة التي تم تكوينها والعتبات تستخدم لاشتقاق احتياطي حتمي (تعكس مواصفات VRF هذه القيمة وما زالت أدلة VRF المباشرة على السلسلة غير موجودة).- POST `/v1/gov/council/derive-vrf` (الميزة: gov_vrf)
  - الطلب: { "committee_size": 21، "epoch": 123؟ , "candidates": [{ "account_id": "..."، "variant": "Normal|Small"، "pk_b64": "..."، "proof_b64": "..." }, ...] }
  - السلوك: VRFproof `chain_id`, `epoch` وأحدث منارة تجزئة الكتلة هي المدخلات المتعارف عليها التي تمنع التحقق من كرتا؛ بايتات الإخراج التي تحدد ترتيب الفواصل الفاصلة التي يتم فرزها مرة أخرى؛ أعلى أعضاء `committee_size` واپس كرتا ہے۔ الإصرار لا يهم.
  - الاستجابة: { "epoch": N, "members": [{ "account_id": "..." } ...], "total_candidates": M, "تم التحقق منه": K }
  - ملاحظات: عادي = pk في G1، إثبات في G2 (96 بايت). صغير = pk في G2، والدليل في G1 (48 بايت). تشمل المدخلات مفصولة بالمجال و`chain_id`.

### افتراضيات الإدارة (iroha_config `gov.*`)

Torii لا يمكن استخدام القائمة المستمرة للمجلس الاحتياطي `iroha_config` لتحديد معلمات هذه القائمة:

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

تجاوزات البيئة المكافئة:

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

`parliament_committee_size` الأعضاء الاحتياطيون الذين يبلغ عددهم الحد الأقصى للكرتا ہے قبل المجلس المستمر لا ہہو، `parliament_term_blocks` طول العصر يحدد كرتا ہے اشتقاق البذور من أجل الاستخدام ہوتا ہے (`epoch = floor(height / term_blocks)`)، `parliament_min_stake` أصل الأهلية لكل حصة (أصغر الوحدات) يفرض قرضًا، و`parliament_eligibility_asset_id` منتخب قرضًا ومرشحًا للنساء عندما يقوم بفحص رصيد الأصول.

التحقق من الحوكمة VK لا يتم تجاوزه: التحقق من الاقتراع `Active` التحقق من المفتاح والبايتات المضمنة التي يتم طلبها، والبيئات التي يتم تبديلها للاختبار فقط للحظر.

RBAC
- أذونات التنفيذ على السلسلة:
  - المقترحات: `CanProposeContractDeployment{ contract_id }`
  - أوراق الاقتراع: `CanSubmitGovernanceBallot{ referendum_id }`
  - التشريع: `CanEnactGovernance`
  - إدارة المجلس (المستقبل): `CanManageParliament`

مساحات الأسماء المحمية
- المعلمة المخصصة `gov_protected_namespaces` (صفيف JSON من السلاسل) مساحات الأسماء المدرجة يمكن نشرها لبوابة القبول النشطة کرتا ہے۔
- يقوم العملاء بمساحات الأسماء المحمية قبل نشر مفاتيح بيانات تعريف المعاملات التي تشمل:
  - `gov_namespace`: مساحة الاسم الهدف (مثال: "apps")
  - `gov_contract_id`: مساحة الاسم کے اندر معرف العقد المنطقي
- `gov_manifest_approvers`: مجموعة JSON اختيارية لمعرفات حساب المدقق. في المسار النصاب الواضح > 1 أعلن عن قبولك لسلطة المعاملات والحسابات المدرجة دون إضافة النصاب القانوني إلى هذا النصاب القانوني.
- القياس عن بعد `governance_manifest_admission_total{result}` عدادات القبول الشاملة التي يعترف بها مشغلو الكاميرا `missing_manifest`، `non_validator_authority`، `quorum_rejected`، `protected_namespace_rejected`، و `runtime_hook_rejected` فرق كبير.
- القياس عن بعد `governance_manifest_quorum_total{outcome}` (القيم `satisfied` / `rejected`) وهو مسار التنفيذ الفعّال بالإضافة إلى الموافقات المفقودة لتدقيق الحسابات.
- تظهر قوائم الممرات بشكل شائع القائمة المسموح بها لمساحة الاسم لفرض القيود. يوجد أيضًا موقع فرانزيز `gov_namespace` `gov_contract_id` في هذا المكان، ومساحة الاسم تظهر ما هو `protected_namespaces`. `RegisterSmartContractCode` يتم إرسال البيانات الوصفية، عند تمكين الحماية، يتم رفضها.
- القبول بات کو فرض القانون رقم `(namespace, contract_id, code_hash, abi_hash)` کے لئے مقترح الحوكمة المعتمد موجود؛ خطأ التحقق من الصحة غير مسموح به ثم فشل.

خطافات ترقية وقت التشغيل
- بيان المسار `hooks.runtime_upgrade` يعلن عن تعليمات ترقية وقت التشغيل (`ProposeRuntimeUpgrade`، `ActivateRuntimeUpgrade`، `CancelRuntimeUpgrade`) وبوابة ما إلى ذلك.
- حقول الخطاف:
  - `allow` (منطقي، افتراضي `true`): ج `false` وترفض جميع تعليمات ترقية وقت التشغيل.
  - `require_metadata` (منطقي، افتراضي `false`): `metadata_key` يتوافق مع إدخال بيانات التعريف درکار ہے۔
  - `metadata_key` (سلسلة): ربط کا اسم البيانات التعريفية المفروضة ۔ الافتراضي `gov_upgrade_id` مطلوب بيانات التعريف أو القائمة المسموح بها موجودة ہو۔
  - `allowed_ids` (صفيف من السلاسل): قيم البيانات التعريفية في القائمة المسموح بها الاختيارية (الاقتطاع بعد)۔ إذا كانت هذه القيمة فلا داعي لرفضها.
- الخطاف موجود ويمكنك من الدخول إلى قائمة الانتظار من خلال سياسة البيانات الوصفية التي تطبقها جانج ستابل. البيانات الوصفية المفقودة أو القيم الفارغة أو القائمة المسموح بها ذات قيم حتمية خطأ غير مسموح به.
- القياس عن بعد `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}` تتبع نتائج النتائج.
- ربط مفتاح التشفير والبيانات التعريفية `gov_upgrade_id=<value>` (أو مفتاح محدد بالبيان) يتضمن النصاب القانوني الواضح الذي يتوافق مع المدققين الذين تم قبول الموافقات أيضًا.

نقطة نهاية الراحة
- POST `/v1/gov/protected-namespaces` - `gov_protected_namespaces` الذي يمكنك من خلاله تطبيق العقدة مرة أخرى.
  - الطلب: { "مساحات الأسماء": ["apps"، "system"] }
  - الرد: { "موافق": صحيح، "مطبق": 1 }
  - ملاحظات: المشرف/الاختبار کے لئے ہے؛ إذا قمت بتكوين رمز واجهة برمجة التطبيقات الخاص بك، قم بتفعيله. الإنتاج لـ `SetParameter(Custom)` تم توقيع المعاملة التجارية.

مساعدي CLI
-`iroha --output-format text app gov deploy audit --namespace apps [--contains calc --hash-prefix deadbeef]`
  - تقوم مثيلات العقد بمساحة الاسم بإحضار العقد والتحقق من العقد:
    - Torii و`code_hash` يتطابق مع الكود الثانوي لرمز البايت، وهو Blake2b-32 Digest `code_hash` يتطابق مع الكرتا.
    - `/v1/contracts/code/{code_hash}` يوجد بيان موجود مطابق لقيم `code_hash` و`abi_hash`.
    - `(namespace, contract_id, code_hash, abi_hash)` يتم اشتقاق مقترح الحوكمة المعتمد من خلال تجزئة معرف الاقتراح ويتم استخدام العقدة.
  - `results[]` الذي سجل تقرير JSON (المشاكل وملخصات البيان/الكود/الاقتراح) وخلاصة الإنترنت (إذا لم يكن هناك `--no-summary`).
  - مساحات الأسماء المحمية التي يتم تدقيقها أو نشر سير العمل الخاضعة لرقابة الإدارة يتم تصديقها بشكل مفعم.
-`iroha app gov deploy-meta --namespace apps --contract-id calc.v1 [--approver ih58... --approver ih58...]`
  - تتيح مساحات الأسماء المحمية نشر هيكل بيانات تعريف JSON، كما أنها تحتوي على `gov_manifest_approvers` الاختياري الذي يتضمن قواعد النصاب القانوني الواضحة.
- `iroha app gov vote --mode zk --referendum-id <id> --proof-b64 <b64> [--owner ih58... --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]` — `min_bond_amount > 0` ما عليك سوى تلميحات القفل السابقة، وإدراج تلميحات أخرى في `owner` و`amount` و `duration_blocks` يشتمل على شيء ضروري.
  - التحقق من صحة معرفات الحساب الأساسية، والتحقق من تلميحات الإبطال ذات 32 بايت، ودمج التلميحات في `public_inputs_json` (مع `--public <path>` للتجاوزات الإضافية).
  - يُشتق المبطل من التزام الإثبات (الإدخال العام) بالإضافة إلى `domain_tag` و`chain_id` و`election_id`؛ تم التحقق من صحة `--nullifier` مقابل الدليل عند تقديمه.
  - ایك لاين خلاص اب حتمية `fingerprint=<hex>` دکھاتا و جو المشفر `CastZkBallot` ستشتق یوتا ہے، سے تلميحات مفككة (`owner`, `amount`, `duration_blocks`، `direction` ج ب فراہم ہوں).
  - استجابات CLI من `tx_instructions[]` إلى `payload_fingerprint_hex` والحقول التي تم فك تشفيرها تضيف تعليقات توضيحية إلى البطاقة وتضيف هيكلًا عظميًا لأدوات المصب وفك تشفير Norito للتحقق من صحة النص.
  - تلميحات القفل الخاصة بعقدة اقتراع ZK في `LockCreated`/`LockExtended` تنبعث منها أحداث دارة كهربية وقيم تعرضها.
-`iroha app gov vote --mode plain --referendum-id <id> --owner ih58... --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - `--lock-amount`/`--lock-duration-blocks` الأسماء المستعارة لأعلام ZK التي تعكس البطاقة وتكافؤ البرمجة النصية.
  - ملخص الإخراج `vote --mode zk` کی تخطيط بصمة التعليمات المشفرة وحقول الاقتراع القابلة للقراءة (`owner`, `amount`, `duration_blocks`, `direction`) يشمل كرتا ہے، جس سے التوقيع سے پہلے فوری تصدیق ہو جاتی ہے۔

قائمة المثيلات
- الحصول على `/v1/gov/instances/{ns}` - مساحة الاسم لمثيلات العقد النشطة کی فہرست۔
  - معلمات الاستعلام:
    - `contains`: `contract_id` سلسلة فرعية تتوافق مع عامل التصفية (حساس لحالة الأحرف)
    - `hash_prefix`: `code_hash_hex` البادئة السداسية المطابقة للمرشح (أحرف صغيرة)
    - `offset` (الافتراضي 0)، `limit` (الافتراضي 100، الحد الأقصى 10_000)
    - `order`: `cid_asc` (افتراضي)، `cid_desc`، `hash_asc`، `hash_desc`
  - الاستجابة: { "مساحة الاسم": "ns"، "المثيلات": [{ "contract_id": "..."، "code_hash_hex": "..." }، ...]، "total": N، "offset": n، "limit": m }
  - مساعد SDK: `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) أو `ToriiClient.list_governance_instances_typed("apps", ...)` (Python)۔

فتح عملية المسح (المشغل/المراجعة)
- احصل على `/v1/gov/unlocks/stats`
  - الاستجابة: { "height_current": H، "expired_locks_now": n، "referenda_with_expired": m، "last_sweep_height": S }
  - ملاحظات: `last_sweep_height` سب سے حالیہ ارتفاع الكتلة دکھاتا ہے جہاں أقفال منتهية الصلاحية تكتسح وتستمر کئے گئے۔ `expired_locks_now` هو قفل السجلات ومسحها ضوئيًا ومسحها ضوئيًا `expiry_height <= height_current`.
- المشاركة `/v1/gov/ballots/zk-v1`
  - الطلب (DTO بنمط v1):
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
  - الاستجابة: { "موافق": صحيح، "مقبول": صحيح، "tx_instructions": [{...}] }- المشاركة `/v1/gov/ballots/zk-v1/ballot-proof` (الميزة: `zk-ballot`)
  - `BallotProof` JSON تم قبولها للموافقة على `CastZkBallot` الهيكل العظمي واپس کرتا.
  - الطلب:
    {
      "السلطة": "ih58..."،
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "...؟",
      "election_id": "ref-1",
      "الاقتراع": {
        "الواجهة الخلفية": "halo2/ipa"،
        "envelope_bytes": "AAECAwQ=", // ZK1 أو H2* حاوية base64
        "root_hint": خالية، // سلسلة سداسية اختيارية ذات 32 بايت (جذر الأهلية)
        "owner": null, // معرف الحساب الاختياري الذي يلتزم به مالك الدائرة کرے
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
    - الخادم الاختياري `root_hint`/`owner`/`nullifier` هو بطاقة اقتراع `CastZkBallot` إلى `public_inputs_json` خريطة كرتا.
    - بايتات المغلف لحمولة التعليمات التي يتم ترميزها مرة أخرى إلى base64.
    - قم بإرسال بطاقة الاقتراع Torii إلى `reason` بدلاً من `submitted transaction` أو جاتا.
    - تم تمكين نقطة النهاية هذه بفضل ميزة `zk-ballot`.

مسار التحقق CastZkBallot
- `CastZkBallot` قم بفك ترميز الإثبات الأساسي 64 للكرتا والحمولات الفارغة أو الخاسرة التي ترفض الكرتا (`BallotRejected` مع `invalid or empty proof`).
- استفتاء المضيف (`vk_ballot`) أو افتراضيات الحكم، وهو عبارة عن بطاقة اقتراع للتحقق من حل المفتاح، وتم طلب كرتا وتسجيل موجود، `Active`، والبايتات المضمنة.
- بايتات مفتاح التحقق المخزنة `hash_vk` ستكرر التجزئة مرة أخرى؛ عدم تطابق الالتزام والتحقق من التنفيذ والتلاعب بإدخالات التسجيل (`BallotRejected` مع `verifying key commitment mismatch`).
- بايتات الإثبات `zk::verify_backend` هي الواجهة الخلفية المسجلة للإرسال؛ النصوص غير الصالحة `BallotRejected` مع `invalid proof` تم إجراؤها وفشلت التعليمات بشكل حتمي.
- يجب أن يكشف الدليل عن التزام الاقتراع وجذر الأهلية كمدخلات عامة؛ يجب أن يتطابق الجذر مع `eligible_root` الخاص بالانتخاب، ويجب أن يتطابق المبطل المشتق مع أي تلميح مقدم.
- البراهين الناجحة `BallotAccepted` تنبعث منها کرتے ہیں؛ يمكن أيضًا استخدام عوامل الإلغاء المكررة، أو جذور الأهلية التي لا معنى لها، أو قفل الانحدارات، بالإضافة إلى أسباب الرفض.

## سوء سلوك المدقق والإجماع المشترك

### سير عمل التقطيع والسجن

إجماع على توافق بروتوكول التحقق من الصحة الذي ينبعث منه كرتا Norito-encoded `Evidence`. ستتحقق الحمولة النافعة في الذاكرة `EvidenceStore` وإذا لم يتم إضافة خريطة `consensus_evidence` المدعومة من WSV. `sumeragi.npos.reconfig.evidence_horizon_blocks` (كتل `7200` الافتراضية) يتم رفض سجل البيانات ويتم تقييد الأرشيف، مع رفض المشغلين لتسجيل الدخول. تحترم الأدلة الموجودة في الأفق أيضًا `sumeragi.npos.reconfig.activation_lag_blocks` (الافتراضي `1`) وتأخير التقطيع `sumeragi.npos.reconfig.slashing_delay_blocks` (الافتراضي `259200`)؛ يمكن للحوكمة إلغاء العقوبات باستخدام `CancelConsensusEvidencePenalty` قبل تطبيق القطع.

الجرائم المعترف بها `EvidenceKind` هي خريطة فردية؛ التمييز ونموذج البيانات الذي ينفذ:

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

- **DoublePrepare/DoubleCommit** - أداة التحقق من الصحة `(phase,height,view,epoch)` لتجميع التجزئات المتعددة.
- **InvalidQc** - لا يحتوي المجمع على شهادة التزام ثرثرة مما قد يؤدي إلى فشل عمليات التحقق الحتمية (مثل الصورة النقطية الفارغة للموقع).
- **اقتراح غير صالح** - يقترح القائد أيسا بلاك أن يفشل التحقق من الصحة الهيكلية (مثل قاعدة السلسلة المقفلة).
- **الرقابة** — تُظهر إيصالات التقديم الموقعة معاملة لم يتم اقتراحها/الالتزام بها مطلقًا.

يقوم المشغلون وحمولات الأدوات بفحص وإعادة البث:

- Torii: `GET /v1/sumeragi/evidence` و`GET /v1/sumeragi/evidence/count`.
- سطر الأوامر: `iroha ops sumeragi evidence list`، و`... count`، و`... submit --evidence-hex <payload>`.

الحوكمة هي بايتات الأدلة والدليل القانوني التي تعالجها:

1. تنتهي صلاحية مجموعة الحمولة الصافية ** منذ فترة طويلة. Norito بايت خام لبيانات تعريف الارتفاع/العرض التي يتم تخزينها في الأرشيف.
2. **لعبة مرحلة الجزاء** الحمولة النافعة للاستفتاء أو تعليمات sudo يمكن تضمينها (مثل `Unregister::peer`). تنفيذ الحمولة النافعة کو دوبارہ التحقق من صحة کرتا ہے؛ الأدلة المشوهة أو التي لا معنى لها ترفض بشكل قاطع ہوتی ہے۔
3. **متابعة الجدول الزمني للطوبولوجيا** لا يتم استخدام أداة التحقق من المخالفة على الفور. التدفقات العامة `SetParameter(Sumeragi::NextMode)` و`SetParameter(Sumeragi::ModeActivationHeight)` قائمة محدثة وهي قائمة انتظار مستمرة.
4. ** نتائج التدقيق ** `/v1/sumeragi/evidence` و `/v1/sumeragi/status` لا تؤدي إلى إزالة الأدلة المضادة والحوكمة.

### التسلسل بالإجماع المشترك

الإجماع المشترك هو ضمانة مجموعة المدقق المنتهية ولايته كتلة الحدود وضع اللمسات الأخيرة على مجموعة جديدة تقترح البدء. معلمات وقت التشغيل التي تحددها أو قاعدة فرضها:

- `SumeragiParameter::NextMode` و`SumeragiParameter::ModeActivationHeight` الذي **أسود** يلتزم بهذا. `mode_activation_height` يتم تحديث اللون الأسود بارتفاعه بدقة، مما يؤدي إلى تأخير طويل جدًا.
- حارس التكوين `sumeragi.npos.reconfig.activation_lag_blocks` (الافتراضي `1`) وعمليات التسليم بدون تأخير:
- `sumeragi.npos.reconfig.slashing_delay_blocks` (الافتراضي `259200`) يؤخر خفض الإجماع حتى تتمكن الإدارة من إلغاء العقوبات قبل تطبيقها.

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```

- تعمل المعلمات المرحلية لوقت التشغيل وCLI مثل `/v1/sumeragi/params` و`iroha --output-format text ops sumeragi params` على تحديد ارتفاعات تنشيط المشغلين وقوائم التحقق من الصحة.
- أتمتة الحكم:
  1. دليل الإزالة (أو الإعادة) فيصل وضع اللمسات الأخيرة على الإزالة.
  2.`mode_activation_height = h_current + activation_lag_blocks` يتم متابعة قائمة انتظار إعادة التكوين.
  3. `/v1/sumeragi/status` لم يتم تحديد الارتفاع المتوقع قبل `effective_consensus_mode` للمفتاح.

تقوم أيضًا أدوات التحقق من صحة البرنامج النصي بتدوير النص أو تطبيق القطع على ** تفعيل بدون تأخير ** أو معلمات التسليم التي لا تحذف أي شيء؛ ترفض هذه المعاملات "بوابة واحدة" ووضع "عمل جديد".

## أسطح القياس عن بعد

- تصدير نشاط حوكمة مقاييس Prometheus إلى:
  - `governance_proposals_status{status}` (المقياس) مقترحات حالة گنتی کے حساب سے المسار کرتا ہے۔
  - `governance_protected_namespace_total{outcome}` (العداد) هو الوقت الذي يتم فيه زيادة مساحات الأسماء المحمية التي يتم نشر القبول أو السماح بها أو رفضها.
  - `governance_manifest_activations_total{event}` (العداد) إدراجات البيان (`event="manifest_inserted"`) وارتباطات مساحة الاسم (`event="instance_bound"`) سجل کرتا ہے۔
- `/status` يتضمن كائن `governance` عدد المقترحات التي تعكس العناصر، وتقرير إجماليات مساحة الاسم المحمية، وقائمة عمليات تنشيط البيان الأخيرة (مساحة الاسم، ومعرف العقد، وتجزئة الكود/ABI، وارتفاع الكتلة، والطابع الزمني للتنشيط). المشغلون الميدانيون الذين يقومون باستقصاء مراقبة القوانين والتشريعات لا يظهرون بوابات مساحة الاسم المحمية وبوابات مساحة الاسم المحمية.
- قالب Grafana (`docs/source/grafana_governance_constraints.json`) و`telemetry.md` يحتويان على مقترحات عالقة، أو تنشيطات بيان مفقودة، أو ترقيات وقت التشغيل أثناء رفض مساحة الاسم المحمية غير المتوقعة، أو تنبيهات الأسلاك جايیں۔
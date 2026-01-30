---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/confidential-assets.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
title: الاصول السرية وتحويلات ZK
description: مخطط Phase C للدوران shielded والسجلات وضوابط المشغلين.
slug: /nexus/confidential-assets
---
<!--
SPDX-License-Identifier: Apache-2.0
-->
# تصميم الاصول السرية وتحويلات ZK

## الدوافع
- تقديم تدفقات اصول shielded اختيارية حتى تتمكن الدومينات من حفظ خصوصية المعاملات دون تغيير الدوران الشفاف.
- الحفاظ على التنفيذ الحتمي عبر عتاد مدققين غير متجانس مع ابقاء توافق Norito/Kotodama ABI v1.
- تزويد المدققين والمشغلين بضوابط دورة الحياة (تفعيل، تدوير، سحب) للدوائر والمعلمات التشفيرية.

## نموذج التهديدات
- المدققون honest-but-curious: ينفذون الاجماع بامانة لكنهم يحاولون فحص ledger/state.
- مراقبو الشبكة يرون بيانات الكتل والمعاملات المرسلة عبر gossip؛ لا نفترض وجود قنوات gossip خاصة.
- خارج النطاق: تحليل حركة المرور خارج الدفتر، خصوم كميون (يتابعون في PQ roadmap)، وهجمات توفر ledger.

## نظرة عامة على التصميم
- يمكن للاصول اعلان *shielded pool* اضافة الى الارصدة الشفافة؛ يتم تمثيل الدوران shielded عبر commitments تشفيرية.
- تغلف notes `(asset_id, amount, recipient_view_key, blinding, rho)` مع:
  - Commitment: `Comm = Pedersen(params_id || asset_id || amount || recipient_view_key || blinding)`.
  - Nullifier: `Null = Poseidon(domain_sep || nk || rho || asset_id || chain_id)` مستقل عن ترتيب notes.
  - Encrypted payload: `enc_payload = AEAD_XChaCha20Poly1305(ephemeral_shared_key, note_plaintext)`.
- تنقل المعاملات payloads `ConfidentialTransfer` المرمزة بنوريتو وتحتوي:
  - Public inputs: Merkle anchor، nullifiers، commitments جديدة، asset id، نسخة الدائرة.
  - Payloads مشفرة للمستلمين والمدققين الاختياريين.
  - Zero-knowledge proof تؤكد حفظ القيمة والملكية والتفويض.
- يتم التحكم في verifying keys ومجموعات المعلمات عبر سجلات على الدفتر مع نوافذ تفعيل؛ ترفض العقد التحقق من proofs تشير الى ادخالات مجهولة او مسحوبة.
- تلتزم رؤوس الاجماع ب digest ميزات السرية النشطة كي لا تقبل الكتل الا عند تطابق حالة السجلات والمعلمات.
- بناء proofs يستخدم Halo2 (Plonkish) بدون trusted setup؛ Groth16 او انواع SNARK اخرى غير مدعومة عمدا في v1.

### Fixtures حتمية

اغلفة memo السرية تشحن الان مع fixture قانوني في `fixtures/confidential/encrypted_payload_v1.json`. تلتقط مجموعة البيانات envelope v1 صحيحا مع عينات سلبية تالفة حتى تتمكن SDKs من اثبات تطابق التحليل. اختبارات Rust data-model (`crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs`) وسويت Swift (`IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift`) تحمل fixture مباشرة، لضمان توافق Norito encoding وسطوح الاخطاء وتغطية الانحدار مع تطور الكودك.

يمكن لـ Swift SDKs الان اصدار تعليمات shield بدون glue JSON مخصص: انشئ `ShieldRequest` مع commitment note بطول 32 بايت، payload مشفر، وdebit metadata، ثم استدع `IrohaSDK.submit(shield:keypair:)` (او `submitAndWait`) لتوقيع المعاملة وتمريرها عبر `/v1/pipeline/transactions`. يقوم المساعد بالتحقق من اطوال commitments، ويمرر `ConfidentialEncryptedPayload` الى Norito encoder، ويعكس layout `zk::Shield` الموضح ادناه حتى تبقى المحافظ متزامنة مع Rust.

## Commitments الاجماع و gating القدرات
- تكشف رؤوس الكتل `conf_features = { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`؛ يشارك digest في hash الاجماع ويجب ان يساوي عرض السجل المحلي لقبول الكتلة.
- يمكن للحوكمة تجهيز الترقيات ببرمجة `next_conf_features` مع `activation_height` مستقبلي؛ وحتى ذلك الارتفاع يجب على منتجي الكتل الاستمرار في اصدار digest السابق.
- يجب على عقد المدققين التشغيل مع `confidential.enabled = true` و `assume_valid = false`. ترفض فحوصات البدء الانضمام الى مجموعة المدققين اذا فشل اي شرط او اختلفت `conf_features` محليا.
- بيانات P2P handshake الان تشمل `{ enabled, assume_valid, conf_features }`. يتم رفض peers الذين يعلنون ميزات غير متوافقة بخطأ `HandshakeConfidentialMismatch` ولا يدخلون ابدا في دوران الاجماع.
- نتائج التوافق بين المدققين، observers، والـ peers القديمة تلتقط في مصفوفة handshake ضمن [Node Capability Negotiation](#node-capability-negotiation). تظهر فشل handshake كـ `HandshakeConfidentialMismatch` وتبقي الـ peer خارج دوران الاجماع حتى يتطابق digest.
- يمكن للمراقبين غير المدققين ضبط `assume_valid = true`؛ يطبقون دلتا سرية بشكل اعمى دون التأثير على سلامة الاجماع.

## سياسات الاصول
- يحمل كل تعريف اصل `AssetConfidentialPolicy` يحدده المنشئ او عبر الحوكمة:
  - `TransparentOnly`: الوضع الافتراضي؛ يسمح فقط بتعليمات شفافة (`MintAsset`, `TransferAsset`, الخ) وترفض العمليات shielded.
  - `ShieldedOnly`: يجب ان تستخدم كل الاصدارات والتحويلات تعليمات سرية؛ يحظر `RevealConfidential` حتى لا تظهر الارصدة علنا.
  - `Convertible`: يمكن للحاملين نقل القيمة بين التمثيل الشفاف والـ shielded باستخدام تعليمات on/off-ramp ادناه.
- تتبع السياسات FSM مقيد لمنع تعلق الاموال:
  - `TransparentOnly → Convertible` (تمكين فوري لـ shielded pool).
  - `TransparentOnly → ShieldedOnly` (يتطلب انتقالا معلقا ونافذة تحويل).
  - `Convertible → ShieldedOnly` (تاخير ادنى الزامى).
  - `ShieldedOnly → Convertible` (يتطلب خطة هجرة لضمان بقاء notes قابلة للصرف).
  - `ShieldedOnly → TransparentOnly` غير مسموح الا اذا كان shielded pool فارغا او قامت الحوكمة بترميز هجرة تزيل السرية عن notes المتبقية.
- تعليمات الحوكمة تضبط `pending_transition { new_mode, effective_height, previous_mode, transition_id, conversion_window }` عبر ISI `ScheduleConfidentialPolicyTransition` ويمكنها الغاء التغييرات المجدولة بـ `CancelConfidentialPolicyTransition`. يضمن تحقق mempool عدم عبور اي معاملة لارتفاع الانتقال، ويفشل الادراج بشكل حتمي اذا تغير فحص السياسة في منتصف الكتلة.
- يتم تطبيق الانتقالات المعلقة تلقائيا عند فتح كتلة جديدة: عند دخول ارتفاع الكتلة نافذة التحويل (لترقيات `ShieldedOnly`) او الوصول الى `effective_height`، يقوم runtime بتحديث `AssetConfidentialPolicy` وتحديث metadata `zk.policy` ومسح المدخل المعلق. اذا بقي عرض شفاف عند نضج انتقال `ShieldedOnly`، يلغي runtime التغيير ويسجل تحذيرا مع بقاء الوضع السابق.
- مقابض التهيئة `policy_transition_delay_blocks` و `policy_transition_window_blocks` تفرض اشعارا ادنى وفترات سماح للسماح بتحويل محافظ حول التبديل.
- `pending_transition.transition_id` يعمل ايضا كـ audit handle؛ يجب على الحوكمة ذكره عند انهاء او الغاء الانتقالات حتى يتمكن المشغلون من ربط تقارير on/off-ramp.
- `policy_transition_window_blocks` افتراضيه 720 (حوالي 12 ساعة مع block time 60s). تحد العقد طلبات الحوكمة التي تحاول اشعارا اقصر.
- Genesis manifests وتدفقات CLI تعرض السياسات الحالية والمعلقة. منطق admission يقرأ السياسة وقت التنفيذ لتاكيد ان كل تعليمة سرية مصرح بها.
- قائمة تحقق الهجرة - انظر “Migration sequencing” ادناه لخطة ترقية على مراحل يتبعها Milestone M0.

#### مراقبة الانتقالات عبر Torii

تستعلم المحافظ والمدققون `GET /v1/confidential/assets/{definition_id}/transitions` لفحص `AssetConfidentialPolicy` النشطة. يحتوي payload JSON دائما على asset id القانوني، اخر ارتفاع كتلة ملاحظ، `current_mode` للسياسة، الوضع الفعال عند ذلك الارتفاع (نوافذ التحويل تبلغ مؤقتا `Convertible`)، ومعرفات معلمات `vk_set_hash`/Poseidon/Pedersen المتوقعة. عند وجود انتقال حوكمة معلق يتضمن الرد ايضا:

- `transition_id` - audit handle المعاد من `ScheduleConfidentialPolicyTransition`.
- `previous_mode`/`new_mode`.
- `effective_height`.
- `conversion_window` و `window_open_height` المشتق (الكتلة التي يجب ان تبدأ فيها المحافظ التحويل لقطع ShieldedOnly).

مثال رد:

```json
{
  "asset_id": "rose#wonderland",
  "block_height": 4217,
  "current_mode": "Convertible",
  "effective_mode": "Convertible",
  "vk_set_hash": "8D7A4B0A95AB1C33F04944F5D332F9A829CEB10FB0D0797E2D25AEFBAAF1155D",
  "poseidon_params_id": 7,
  "pedersen_params_id": 11,
  "pending_transition": {
    "transition_id": "BF2C6F9A4E9DF389B6F7E5E6B5487B39AE00D2A4B7C0FBF2C9FEF6D0A961C8ED",
    "previous_mode": "Convertible",
    "new_mode": "ShieldedOnly",
    "effective_height": 5000,
    "conversion_window": 720,
    "window_open_height": 4280
  }
}
```

يشير رد `404` الى عدم وجود تعريف اصل مطابق. عند عدم وجود انتقال مجدول يكون الحقل `pending_transition` مساوي لـ `null`.

### آلة حالات السياسة

| الوضع الحالي       | الوضع التالي       | المتطلبات                                                                 | التعامل مع effective_height                                                                                         | ملاحظات                                                                                     |
|--------------------|------------------|-------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------|
| TransparentOnly    | Convertible      | فعلت الحوكمة ادخالات سجل verifier/parameter. قدّم `ScheduleConfidentialPolicyTransition` مع `effective_height ≥ current_height + policy_transition_delay_blocks`. | ينفذ الانتقال بالضبط عند `effective_height`؛ يصبح shielded pool متاحا فوريا.                   | المسار الافتراضي لتمكين السرية مع ابقاء التدفقات الشفافة.               |
| TransparentOnly    | ShieldedOnly     | كما سبق، بالاضافة الى `policy_transition_window_blocks ≥ 1`.                                                         | يدخل runtime تلقائيا `Convertible` عند `effective_height - policy_transition_window_blocks`؛ يتحول الى `ShieldedOnly` عند `effective_height`. | يوفر نافذة تحويل حتمية قبل تعطيل التعليمات الشفافة.   |
| Convertible        | ShieldedOnly     | انتقال مجدول مع `effective_height ≥ current_height + policy_transition_delay_blocks`. يجب على الحوكمة توثيق (`transparent_supply == 0`) عبر audit metadata؛ ويفرض runtime ذلك عند القطع. | نفس دلالات النافذة كما اعلاه. اذا كان العرض الشفاف غير صفري عند `effective_height` يتم اجهاض الانتقال بـ `PolicyTransitionPrerequisiteFailed`. | يقفل الاصل في دوران سري بالكامل.                                     |
| ShieldedOnly       | Convertible      | انتقال مجدول؛ لا يوجد سحب طارئ نشط (`withdraw_height` غير مضبوط).                                    | يتبدل الوضع عند `effective_height`؛ تعاد فتح reveal ramps بينما تبقى shielded notes صالحة.                           | يستخدم لنوافذ الصيانة او مراجعات المدققين.                                          |
| ShieldedOnly       | TransparentOnly  | يجب على الحوكمة اثبات `shielded_supply == 0` او تجهيز خطة `EmergencyUnshield` موقعة (تتطلب تواقيع مدققين). | يفتح runtime نافذة `Convertible` قبل `effective_height`؛ عند الارتفاع تفشل التعليمات السرية بقوة ويعود الاصل الى وضع شفاف فقط. | خروج كملاذ اخير. يتم الغاء الانتقال تلقائيا اذا تم صرف اي note سرية خلال النافذة. |
| Any                | Same as current  | `CancelConfidentialPolicyTransition` يمسح التغيير المعلق.                                                        | يتم حذف `pending_transition` فورا.                                                                          | يحافظ على الوضع الراهن؛ مذكور للاكتمال.                                             |

الانتقالات غير المدرجة اعلاه ترفض عند تقديمها للحوكمة. يتحقق runtime من الشروط المسبقة قبل تطبيق الانتقال المجدول مباشرة؛ فشل الشروط يعيد الاصل الى وضعه السابق ويطلق `PolicyTransitionPrerequisiteFailed` عبر التليمتري واحداث الكتلة.

### Migration sequencing

1. **Prepare registries:** فعّل كل مدخلات verifier والمعلمات المشار اليها في السياسة المستهدفة. تعلن العقد `conf_features` الناتجة حتى يتمكن peers من التحقق من التوافق.
2. **Stage the transition:** قدّم `ScheduleConfidentialPolicyTransition` مع `effective_height` يراعي `policy_transition_delay_blocks`. عند الانتقال نحو `ShieldedOnly` حدد نافذة تحويل (`window ≥ policy_transition_window_blocks`).
3. **Publish operator guidance:** سجّل `transition_id` المعاد ووزع runbook للـ on/off-ramp. تشترك المحافظ والمدققون في `/v1/confidential/assets/{id}/transitions` لمعرفة ارتفاع فتح النافذة.
4. **Window enforcement:** عند فتح النافذة يحول runtime السياسة الى `Convertible`، ويصدر `PolicyTransitionWindowOpened { transition_id }` ويبدأ في رفض طلبات الحوكمة المتعارضة.
5. **Finalize or abort:** عند `effective_height` يتحقق runtime من الشروط المسبقة (عرض شفاف صفر، عدم وجود سحب طارئ، الخ). النجاح يقلب السياسة للوضع المطلوب؛ الفشل يطلق `PolicyTransitionPrerequisiteFailed`، يمسح الانتقال المعلق، ويترك السياسة دون تغيير.
6. **Schema upgrades:** بعد نجاح الانتقال ترفع الحوكمة نسخة مخطط الاصل (مثلا `asset_definition.v2`) وتتطلب ادوات CLI حقل `confidential_policy` عند تسلسل manifests. توجه وثائق ترقية genesis المشغلين لاضافة اعدادات السياسة وبصمات registry قبل اعادة تشغيل المدققين.

الشبكات الجديدة التي تبدأ مع تمكين السرية ترمز السياسة المطلوبة مباشرة في genesis. مع ذلك تتبع نفس قائمة التحقق عند تغيير الاوضاع بعد الاطلاق كي تبقى نوافذ التحويل حتمية وتمتلك المحافظ وقتا للتكيف.

### نسخ Norito manifests والتفعيل

- يجب ان تتضمن Genesis manifests `SetParameter` للمفتاح المخصص `confidential_registry_root`. يكون payload هو Norito JSON مطابق لـ `ConfidentialRegistryMeta { vk_set_hash: Option<String> }`: احذف الحقل (`null`) عندما لا توجد ادخالات verifier نشطة، والا قدم سلسلة hex بطول 32 بايت (`0x…`) تساوي الهاش الناتج من `compute_vk_set_hash` على تعليمات verifier في manifest. ترفض العقد البدء اذا كان المعامل مفقودا او الهاش لا يطابق كتابات السجل المرمزة.
- يضمن `ConfidentialFeatureDigest::conf_rules_version` on-wire نسخة تخطيط manifest. لشبكات v1 يجب ان يبقى `Some(1)` ويساوي `iroha_config::parameters::defaults::confidential::RULES_VERSION`. عند تطور القواعد ارفع الثابت، اعادة توليد manifests، واطلاق البيناريات معا؛ خلط النسخ يجعل المدققين يرفضون الكتل بـ `ConfidentialFeatureDigestMismatch`.
- ينبغي ان تجمع Activation manifests تحديثات registry وتغييرات دورة حياة المعلمات وانتقالات السياسة بحيث يبقى digest متسقا:
  1. طبق طفرات registry المخططة (`Publish*`, `Set*Lifecycle`) في عرض حالة offline واحسب digest بعد التفعيل باستخدام `compute_confidential_feature_digest`.
  2. اصدِر `SetParameter::custom(confidential_registry_root, {"vk_set_hash": "0x…"})` باستخدام الهاش المحسوب حتى يتمكن peers المتاخرون من استعادة digest الصحيح حتى اذا فاتتهم تعليمات registry الوسيطة.
  3. ارفق تعليمات `ScheduleConfidentialPolicyTransition`. يجب على كل تعليمة ان تقتبس `transition_id` الصادر من الحوكمة؛ manifests التي تنساه سترفضها runtime.
  4. احفظ بايتات manifest وبصمة SHA-256 وdigest المستخدم في خطة التفعيل. يتحقق المشغلون من الثلاثة قبل التصويت لتجنب الانقسام.
- عندما تتطلب عمليات الاطلاق cut-over مؤجلا، سجل الارتفاع المستهدف في معامل مخصص مرافق (مثلا `custom.confidential_upgrade_activation_height`). هذا يعطي المدققين دليلا مشفرا بنوريتو على ان المدققين احترموا نافذة الاشعار قبل سريان تغيير digest.

## دورة حياة verifier والمعلمات
### ZK Registry
- يخزن ledger `ZkVerifierEntry { vk_id, circuit_id, version, proving_system, curve, public_inputs_schema_hash, vk_hash, vk_len, max_proof_bytes, gas_schedule_id, activation_height, deprecation_height, withdraw_height, status, metadata_uri_cid, vk_bytes_cid }` حيث `proving_system` حاليا ثابت على `Halo2`.
- ازواج `(circuit_id, version)` فريدة عالميا؛ يحافظ السجل على فهرس ثانوي للبحث حسب metadata الدائرة. محاولات تسجيل زوج مكرر ترفض عند admission.
- يجب ان يكون `circuit_id` غير فارغ ويجب توفير `public_inputs_schema_hash` (عادة hash Blake2b-32 لترميز الادخال العام القانوني للـ verifier). ترفض admission السجلات التي تهمل هذه الحقول.
- تعليمات الحوكمة تشمل:
  - `PUBLISH` لاضافة مدخل `Proposed` بmetadata فقط.
  - `ACTIVATE { vk_id, activation_height }` لجدولة تفعيل المدخل عند حدود epoch.
  - `DEPRECATE { vk_id, deprecation_height }` لتحديد اخر ارتفاع يمكن فيه للـ proofs الاشارة الى المدخل.
  - `WITHDRAW { vk_id, withdraw_height }` لاغلاق طارئ؛ الاصول المتاثرة تجمد الانفاق السري بعد withdraw height حتى تتفعل ادخالات جديدة.
- Genesis manifests تصدر تلقائيا معامل `confidential_registry_root` بقيمة `vk_set_hash` المطابقة للادخالات النشطة؛ يتحقق التحقق من هذا digest مقابل حالة السجل المحلية قبل انضمام العقدة للاجماع.
- تسجيل او تحديث verifier يتطلب `gas_schedule_id`؛ يفرض التحقق ان يكون مدخل السجل `Active` وموجودا في فهرس `(circuit_id, version)` وان توفر Halo2 proofs `OpenVerifyEnvelope` يطابق `circuit_id` و`vk_hash` و`public_inputs_schema_hash` في سجل السجل.

### Proving Keys
- تبقى proving keys خارج الدفتر لكن تشير اليها معرفات content-addressed (`pk_cid`, `pk_hash`, `pk_len`) منشورة مع metadata verifier.
- تقوم Wallet SDKs بجلب بيانات PK والتحقق من الهاش وحفظها محليا.

### Pedersen & Poseidon Parameters
- سجلات منفصلة (`PedersenParams`, `PoseidonParams`) تعكس ضوابط دورة حياة verifier، ولكل منها `params_id`، هاشات المولدات/الثوابت، وارتفاعات التفعيل/الاستبدال/السحب.
- تفصل commitments والهاشات المجال عبر `params_id` حتى لا تعيد تدوير المعلمات استخدام انماط بت من مجموعات قديمة؛ يدمج الـ ID في commitments notes وعلامات مجال nullifier.

## الترتيب الحتمي و nullifiers
- يحافظ كل اصل على `CommitmentTree` مع `next_leaf_index`; تضيف الكتل commitments بترتيب حتمي: تمر المعاملات بترتيب الكتلة؛ داخل كل معاملة تمر مخرجات shielded تصاعديا حسب `output_idx` المتسلسل.
- `note_position` مشتق من ازاحات الشجرة لكنه **ليس** جزءا من nullifier؛ يستخدم فقط لمسارات العضوية ضمن proof witness.
- يضمن تصميم PRF ثبات nullifier عند reorgs؛ يربط مدخل PRF `{ nk, note_preimage_hash, asset_id, chain_id, params_id }`، وتستند anchors الى جذور Merkle تاريخية محدودة بـ `max_anchor_age_blocks`.

## تدفق ledger
1. **MintConfidential { asset_id, amount, recipient_hint }**
   - يتطلب سياسة اصل `Convertible` او `ShieldedOnly`; يتحقق admission من سلطة الاصل، يجلب `params_id` الحالي، يعين `rho`، يصدر commitment ويحدث Merkle tree.
   - يصدر `ConfidentialEvent::Shielded` مع commitment جديد، فرق Merkle root، وhash استدعاء المعاملة للتدقيق.
2. **TransferConfidential { asset_id, proof, circuit_id, version, nullifiers, new_commitments, enc_payloads, anchor_root, memo }**
   - يتحقق syscall في VM من proof باستخدام مدخل السجل؛ يضمن host ان nullifiers غير مستخدمة، commitments تضاف حتميا، وanchor حديث.
   - يسجل ledger ادخالات `NullifierSet`، ويحفظ payloads مشفرة للمستلمين/المدققين، ويصدر `ConfidentialEvent::Transferred` ملخصا nullifiers والمخرجات المرتبة وproof hash وMerkle roots.
3. **RevealConfidential { asset_id, proof, circuit_id, version, nullifier, amount, recipient_account, anchor_root }**
   - متاحة فقط للاصول `Convertible`; تتحقق proof ان قيمة note تساوي المبلغ المكشوف، يضيف ledger الرصيد الشفاف ويحرق shielded note بوسم nullifier كمصروف.
   - يصدر `ConfidentialEvent::Unshielded` مع المبلغ العلني وnullifiers المستهلكة ومعرفات proof وhash استدعاء المعاملة.

## اضافات data model
- `ConfidentialConfig` (قسم تهيئة جديد) مع علم التفعيل، `assume_valid`, مقابض gas/limits، نافذة anchor، وverifier backend.
- `ConfidentialNote`, `ConfidentialTransfer`, و `ConfidentialMint` مخططات Norito مع بايت نسخة صريح (`CONFIDENTIAL_ASSET_V1 = 0x01`).
- `ConfidentialEncryptedPayload` يغلف AEAD memo bytes في `{ version, ephemeral_pubkey, nonce, ciphertext }`، وافتراضيا `version = CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1` لتخطيط XChaCha20-Poly1305.
- توجد متجهات اشتقاق المفاتيح القانونية في `docs/source/confidential_key_vectors.json`; كل من CLI وTorii endpoint يرجعان اليها في اختبارات الانحدار.
- يحصل `asset::AssetDefinition` على `confidential_policy: AssetConfidentialPolicy { mode, vk_set_hash, poseidon_params_id, pedersen_params_id, pending_transition }`.
- يحفظ `ZkAssetState` ربط `(backend, name, commitment)` لـ transfer/unshield verifiers؛ يرفض التنفيذ proofs التي لا تطابق verifying key المسجل (مرجعا او ضمنيا).
- يتم تخزين `CommitmentTree` (لكل اصل مع frontier checkpoints)، و `NullifierSet` بالمفتاح `(chain_id, asset_id, nullifier)`، و `ZkVerifierEntry` و `PedersenParams` و `PoseidonParams` في world state.
- يحتفظ mempool بهياكل `NullifierIndex` و `AnchorIndex` المؤقتة للكشف المبكر عن التكرار والتحقق من عمر anchor.
- تحديثات مخطط Norito تشمل ترتيبًا قانونيًا لـ public inputs؛ اختبارات round-trip تضمن حتمية الترميز.
- تثبت round-trip للـ encrypted payload عبر unit tests (`crates/iroha_data_model/src/confidential.rs`). ستضيف متجهات المحافظ لاحقا AEAD transcripts قانونية للمدققين. يوثق `norito.md` ترويسة on-wire للـ envelope.

## تكامل IVM و syscall
- تقديم syscall `VERIFY_CONFIDENTIAL_PROOF` يقبل:
  - `circuit_id`, `version`, `scheme`, `public_inputs`, `proof` والنتيجة `ConfidentialStateDelta { asset_id, nullifiers, commitments, enc_payloads }`.
  - يقوم syscall بتحميل metadata verifier من السجل، يفرض حدود الحجم/الوقت، يحسب gas حتمي، ولا يطبق delta الا عند نجاح proof.
- يوفر host trait read-only `ConfidentialLedger` لاسترجاع snapshots لجذر Merkle وحالة nullifier؛ توفر مكتبة Kotodama helpers لتجميع witness والتحقق من schema.
- تم تحديث وثائق pointer-ABI لتوضيح layout proof buffer ومقابض registry.

## تفاوض قدرات العقد
- يعلن handshake `feature_bits.confidential` مع `ConfidentialFeatureDigest { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`. مشاركة المدقق تتطلب `confidential.enabled=true` و`assume_valid=false` ومعرفات verifier backend متطابقة وdigest متطابق؛ الفروقات تفشل handshake بـ `HandshakeConfidentialMismatch`.
- يدعم config `assume_valid` لعقد observers فقط: عند تعطيله، تؤدي تعليمات السرية الى `UnsupportedInstruction` حتمي بلا panic؛ عند تمكينه، يطبق observers دلتا الحالة المعلنة دون التحقق من proofs.
- يرفض mempool المعاملات السرية اذا كانت القدرة المحلية معطلة. تتجنب فلاتر gossip ارسال المعاملات shielded الى peers غير متوافقة بينما تعيد توجيه معرفات verifier غير المعروفة بشكل اعمى ضمن حدود الحجم.

### مصفوفة توافق handshake

| اعلان الطرف البعيد | النتيجة لعقد المدققين | ملاحظات المشغل |
|----------------------|-----------------------------|----------------|
| `enabled=true`, `assume_valid=false`, backend متطابق, digest متطابق | مقبول | يصل peer الى حالة `Ready` ويشارك في proposal وvote وRBC fan-out. لا يتطلب تدخل يدوي. |
| `enabled=true`, `assume_valid=false`, backend متطابق, digest قديم او مفقود | مرفوض (`HandshakeConfidentialMismatch`) | يجب على الطرف البعيد تطبيق تفعيل السجلات/المعلمات المعلقة او انتظار `activation_height` المجدولة. حتى التصحيح يبقى قابلا للاكتشاف لكنه لا يدخل دوران الاجماع. |
| `enabled=true`, `assume_valid=true` | مرفوض (`HandshakeConfidentialMismatch`) | يتطلب المدققون تحقق proofs؛ قم بضبط الطرف البعيد كمراقب عبر Torii فقط او اجعل `assume_valid=false` بعد تمكين التحقق الكامل. |
| `enabled=false`, حذف حقول handshake (بناء قديم) او اختلاف verifier backend | مرفوض (`HandshakeConfidentialMismatch`) | peers قديمة او محدثة جزئيا لا يمكنها الانضمام لشبكة الاجماع. قم بترقيتها وتأكد من تطابق backend + digest قبل اعادة الاتصال. |

العقد المراقِبة التي تتجاوز تحقق proofs عمدا يجب الا تفتح اتصالات اجماع مع مدققين يعملون ببوابات قدرات. يمكنها استيعاب الكتل عبر Torii او واجهات الارشفة، لكن شبكة الاجماع ترفضها حتى تعلن قدرات متوافقة.

### سياسة تقليم Reveal والاحتفاظ بـ Nullifier

يجب على دفاتر السرية الاحتفاظ بتاريخ كاف لاثبات حداثة notes واعادة تشغيل تدقيقات الحوكمة. السياسة الافتراضية التي تفرضها `ConfidentialLedger` هي:

- **Nullifier retention:** الاحتفاظ بـ nullifiers المصروفة لمدة *ادنى* `730` يوما (24 شهرا) بعد ارتفاع الصرف، او لفترة اطول اذا فرضها المنظم. يمكن للمشغلين تمديدها عبر `confidential.retention.nullifier_days`. يجب ان تبقى nullifiers ضمن النافذة قابلة للاستعلام عبر Torii حتى يتمكن المدققون من اثبات عدم وجود double-spend.
- **Reveal pruning:** تقوم Reveals الشفافة (`RevealConfidential`) بتقليم commitments المرتبطة فورا بعد اكتمال الكتلة، لكن nullifier المصروف يبقى خاضعا لقاعدة الاحتفاظ. تسجل احداث reveal (`ConfidentialEvent::Unshielded`) المبلغ العلني والمستلم وhash proof كي لا تتطلب اعادة بناء الكشوف التاريخية ciphertext المقتطع.
- **Frontier checkpoints:** تحافظ frontier commitments على checkpoints متحركة تغطي الاكبر من `max_anchor_age_blocks` ونافذة الاحتفاظ. لا تقوم العقد بضغط checkpoints الاقدم الا بعد انتهاء كل nullifiers في تلك الفترة.
- **Stale digest remediation:** اذا رُفع `HandshakeConfidentialMismatch` بسبب انحراف digest، يجب على المشغلين (1) التحقق من تطابق نوافذ الاحتفاظ عبر الكتلة، (2) تشغيل `iroha_cli app confidential verify-ledger` لاعادة توليد digest مقابل مجموعة nullifier المحتفظ بها، و(3) اعادة نشر manifest المحدث. يجب استعادة اي nullifiers حذفت مبكرا من التخزين البارد قبل اعادة الانضمام للشبكة.

وثق التعديلات المحلية في operations runbook؛ سياسات الحوكمة التي تمد نافذة الاحتفاظ يجب ان تحدث تهيئة العقد وخطط التخزين الارشيفي بالتزامن.

### تدفق الاخلاء والاستعادة

1. اثناء الاتصال، يقارن `IrohaNetwork` القدرات المعلنة. اي عدم تطابق يرفع `HandshakeConfidentialMismatch`؛ يغلق الاتصال ويبقى peer في discovery queue دون ترقيته الى `Ready`.
2. تظهر المشكلة في سجل خدمة الشبكة (مع digest والـ backend للطرف البعيد)، ولا يقوم Sumeragi بجدولة peer للتقديم او التصويت.
3. يعالج المشغلون الامر بمحاذاة سجلات verifier ومجموعات المعلمات (`vk_set_hash`, `pedersen_params_id`, `poseidon_params_id`) او بتهيئة `next_conf_features` مع `activation_height` متفق عليها. عندما يتطابق digest ينجح handshake التالي تلقائيا.
4. اذا تمكن peer قديم من بث كتلة (مثلا عبر archival replay)، يرفضها المدققون حتميا بـ `BlockRejectionReason::ConfidentialFeatureDigestMismatch` للحفاظ على اتساق ledger عبر الشبكة.

### Replay-safe handshake flow

1. كل محاولة خروج تخصص مادة مفاتيح Noise/X25519 جديدة. الـ handshake payload الموقع (`handshake_signature_payload`) يربط المفاتيح العامة المؤقتة المحلية والبعيدة، عنوان socket المعلن والمشفّر بنوريتو، ومعرف السلسلة عند التجميع مع `handshake_chain_id`. يتم تشفير الرسالة بـ AEAD قبل ارسالها.
2. يعيد المستجيب حساب payload بترتيب مفاتيح peer/local المعكوس ويتحقق من توقيع Ed25519 المضمن في `HandshakeHelloV1`. بما ان كلا المفتاحين المؤقتين والعنوان المعلن ضمن نطاق التوقيع، فاعادة تشغيل رسالة ملتقطة ضد peer اخرى او استعادة اتصال قديم تفشل حتميا.
3. تنتقل اعلام السرية و `ConfidentialFeatureDigest` داخل `HandshakeConfidentialMeta`. يقارن المستقبل tuple `{ enabled, assume_valid, verifier_backend, digest }` مع `ConfidentialHandshakeCaps` المحلية؛ اي عدم تطابق ينهي handshake بـ `HandshakeConfidentialMismatch` قبل انتقال النقل الى `Ready`.
4. يجب على المشغلين اعادة حساب digest (عبر `compute_confidential_feature_digest`) واعادة تشغيل العقد بسياسات/سجلات محدثة قبل اعادة الاتصال. peers التي تعلن digests قديمة تستمر في الفشل، مانعة دخول حالة قديمة الى مجموعة المدققين.
5. نجاحات وفشل handshakes تحدث عدادات `iroha_p2p::peer` القياسية (`handshake_failure_count` وغيرها) وتصدر سجلات منظمة مع وسم معرف peer البعيد وبصمة digest. راقب هذه المؤشرات لاكتشاف محاولات replay او سوء التهيئة اثناء rollout.

## ادارة المفاتيح و payloads
- تسلسل اشتقاق المفاتيح لكل حساب:
  - `sk_spend` → `nk` (nullifier key)، `ivk` (incoming viewing key)، `ovk` (outgoing viewing key)، `fvk`.
- تستخدم payloads notes المشفرة AEAD مع مفاتيح مشتركة مشتقة من ECDH؛ يمكن ارفاق auditor view keys اختيارية الى outputs حسب سياسة الاصل.
- اضافات CLI: `confidential create-keys`, `confidential send`, `confidential export-view-key`, ادوات للمدققين لفك memos، والمساعد `iroha app zk envelope` لانتاج/فحص envelopes Norito دون اتصال. يعرض Torii نفس تدفق الاشتقاق عبر `POST /v1/confidential/derive-keyset` ويعيد اشكالا hex وbase64 لكي تستطيع المحافظ جلب هياكل المفاتيح برمجيا.

## الغاز، الحدود، وضوابط DoS
- جدول gas حتمي:
  - Halo2 (Plonkish): اساس `250_000` gas + `2_000` gas لكل public input.
  - `5` gas لكل بايت proof، مع رسوم لكل nullifier (`300`) ولكل commitment (`500`).
  - يمكن للمشغلين تجاوز هذه الثوابت عبر تهيئة العقد (`confidential.gas.{proof_base, per_public_input, per_proof_byte, per_nullifier, per_commitment}`)؛ تنتشر التغييرات عند البدء او اعادة تحميل التهيئة وتطبق حتميا عبر الكتلة.
- حدود صارمة (افتراضات قابلة للضبط):
- `max_proof_size_bytes = 262_144`.
- `max_nullifiers_per_tx = 8`, `max_commitments_per_tx = 8`, `max_confidential_ops_per_block = 256`.
- `verify_timeout_ms = 750`, `max_anchor_age_blocks = 10_000`. proofs التي تتجاوز `verify_timeout_ms` تقطع التعليمة حتميا (تصويتات الحوكمة تصدر `proof verification exceeded timeout` و`VerifyProof` يعيد خطا).
- حصص اضافية تضمن الحيوية: `max_proof_bytes_block`, `max_verify_calls_per_tx`, `max_verify_calls_per_block`, و `max_public_inputs` تحد builders الكتل؛ `reorg_depth_bound` (≥ `max_anchor_age_blocks`) يتحكم في احتفاظ frontier checkpoints.
- يرفض runtime المعاملات التي تتجاوز هذه الحدود لكل معاملة او لكل كتلة، ويصدر اخطاء `InvalidParameter` حتمية مع ابقاء حالة ledger دون تغيير.
- يرشح mempool المعاملات السرية مسبقا حسب `vk_id` وطول proof وعمر anchor قبل استدعاء verifier للحفاظ على حدود الموارد.
- يتوقف التحقق حتميا عند timeout او تجاوز الحدود؛ تفشل المعاملات باخطاء واضحة. backends SIMD اختيارية لكنها لا تغير حساب gas.

### خطوط اساس المعايرة وبوابات القبول
- **Reference platforms.** يجب ان تغطي معايرات الاداء ثلاث ملفات عتاد ادناه. اي معايرة لا تلتقط جميع الملفات ترفض اثناء المراجعة.

  | الملف | المعمارية | CPU / Instance | اعلام المترجم | الغاية |
  | --- | --- | --- | --- | --- |
  | `baseline-simd-neutral` | `x86_64` | AMD EPYC 7B12 (32c) او Intel Xeon Gold 6430 (24c) | `RUSTFLAGS="-C target-feature=-avx,-avx2,-fma"` | تأسيس قيم ارضية بدون تعليمات متجهة؛ يستخدم لضبط جداول تكلفة fallback. |
  | `baseline-avx2` | `x86_64` | Intel Xeon Gold 6430 (24c) | default release | يتحقق من مسار AVX2؛ يفحص ان تسريعات SIMD ضمن تسامح gas المحايد. |
  | `baseline-neon` | `aarch64` | AWS Graviton3 (c7g.4xlarge) | default release | يضمن بقاء backend NEON حتمي ومتوافق مع جداول x86. |

- **Benchmark harness.** يجب انتاج كل تقارير معايرة الغاز باستخدام:
  - `CRITERION_HOME=target/criterion cargo bench -p iroha_core isi_gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>`
  - `cargo test -p iroha_core bench_repro -- --ignored` لتأكيد fixture الحتمية.
  - `CRITERION_HOME=target/criterion cargo bench -p ivm gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>` كلما تغيرت تكاليف VM opcode.

- **Fixed randomness.** صدّر `IROHA_CONF_GAS_SEED=conf-gas-seed-2026Q1` قبل تشغيل البنش حتى يتحول `iroha_test_samples::gen_account_in` الى مسار `KeyPair::from_seed` الحتمي. يطبع harness `IROHA_CONF_GAS_SEED_ACTIVE=…` مرة واحدة؛ اذا غاب المتغير يجب ان تفشل المراجعة. يجب على اي ادوات معايرة جديدة احترام هذا المتغير عند ادخال عشوائية اضافية.

- **Result capture.**
  - ارفع Criterion summaries (`target/criterion/**/raw.csv`) لكل ملف الى artefact الاصدار.
  - خزّن المقاييس المشتقة (`ns/op`, `gas/op`, `ns/gas`) في [Confidential Gas Calibration ledger](./confidential-gas-calibration) مع git commit ونسخة المترجم المستخدمة.
  - احتفظ بآخر baseline اثنين لكل ملف؛ احذف اللقطات الاقدم بعد اعتماد التقرير الاحدث.

- **Acceptance tolerances.**
  - يجب ان تبقى فروقات الغاز بين `baseline-simd-neutral` و`baseline-avx2` ضمن ≤ ±1.5%.
  - يجب ان تبقى فروقات الغاز بين `baseline-simd-neutral` و`baseline-neon` ضمن ≤ ±2.0%.
  - المقترحات التي تتجاوز هذه الحدود تتطلب تعديلات جدول او RFC يشرح الفجوة وخطة التخفيف.

- **Review checklist.** يتحمل مقدمو الطلب مسؤولية:
  - تضمين `uname -a` ومقتطفات `/proc/cpuinfo` (model, stepping) و `rustc -Vv` في سجل المعايرة.
  - التحقق من ظهور `IROHA_CONF_GAS_SEED` في خرج البنش (البنش يطبع seed النشط).
  - ضمان تطابق pacemaker وميزات confidential verifier مع production (`--features confidential,telemetry` عند تشغيل البنش مع Telemetry).

## التهيئة والعمليات
- يضيف `iroha_config` قسم `[confidential]`:
  ```toml
  [confidential]
  enabled = true
  assume_valid = false
  verifier_backend = "ark_bls12_381"
  max_proof_size_bytes = 262144
  max_nullifiers_per_tx = 8
  max_commitments_per_tx = 8
  max_confidential_ops_per_block = 256
  verify_timeout_ms = 750
  max_anchor_age_blocks = 10000
  max_proof_bytes_block = 1048576
  max_verify_calls_per_tx = 4
  max_verify_calls_per_block = 128
  max_public_inputs = 32
  reorg_depth_bound = 10000
  policy_transition_delay_blocks = 100
  policy_transition_window_blocks = 200
  tree_roots_history_len = 10000
  tree_frontier_checkpoint_interval = 100
  registry_max_vk_entries = 64
  registry_max_params_entries = 32
  registry_max_delta_per_block = 4
  ```
- تصدر التليمتري مقاييس مجمعة: `confidential_proof_verified`, `confidential_verifier_latency_ms`, `confidential_proof_bytes_total`, `confidential_nullifier_spent`, `confidential_commitments_appended`, `confidential_mempool_rejected_total{reason}`, و `confidential_policy_transitions_total` دون كشف بيانات plaintext.
- سطوح RPC:
  - `GET /confidential/capabilities`
  - `GET /confidential/zk_registry`
  - `GET /confidential/params`

## استراتيجية الاختبار
- الحتمية: خلط المعاملات عشوائيا داخل الكتل ينتج نفس Merkle roots وnullifier sets.
- تحمل reorg: محاكاة reorg متعدد الكتل مع anchors؛ تبقى nullifiers مستقرة وترفض anchors القديمة.
- ثوابت الغاز: تاكد من نفس استخدام الغاز عبر عقد مع او بدون تسريع SIMD.
- اختبار الحدود: proofs عند سقوف الحجم/الغاز، الحد الاقصى للمدخلات/المخرجات، وتطبيق timeout.
- دورة الحياة: عمليات الحوكمة لتفعيل/استبدال verifier والمعلمات، واختبارات صرف الدوران.
- Policy FSM: الانتقالات المسموح بها/المرفوضة، تأخيرات pending transition، ورفض mempool حول effective heights.
- طوارئ السجل: سحب طارئ يجمد الاصول المتاثرة عند `withdraw_height` ويرفض proofs بعدها.
- Capability gating: المدققون مع `conf_features` غير متطابقة يرفضون الكتل؛ observers مع `assume_valid=true` يواكبون دون التأثير على الاجماع.
- تكافؤ الحالة: عقد validator/full/observer تنتج نفس state roots على السلسلة القانونية.
- Negative fuzzing: proofs تالفة، payloads كبيرة جدا، وتصادمات nullifier ترفض حتميا.

## الهجرة والتوافق
- طرح محكوم بالميزة: حتى اكتمال Phase C3، `enabled` افتراضيه `false`؛ تعلن العقد قدراتها قبل الانضمام الى مجموعة المدققين.
- الاصول الشفافة غير متاثرة؛ التعليمات السرية تتطلب ادخالات السجل والتفاوض على القدرات.
- العقد المترجمة دون دعم السرية ترفض الكتل المعنية حتميا؛ لا يمكنها الانضمام لمجموعة المدققين لكنها قد تعمل كمراقبين مع `assume_valid=true`.
- Genesis manifests تشمل ادخالات سجل اولية، مجموعات معلمات، سياسات سرية للاصول، ومفاتيح مدققين اختيارية.
- يتبع المشغلون runbooks المنشورة لتدوير السجل وانتقالات السياسة والسحب الطارئ للحفاظ على ترقية حتمية.

## اعمال متبقية
- قياس مجموعات معلمات Halo2 (حجم الدائرة، استراتيجية lookup) وتسجيل النتائج في calibration playbook حتى يمكن تحديث افتراضات gas/timeout مع تحديث `confidential_assets_calibration.md` القادم.
- انهاء سياسات الافصاح للمدققين وواجهات selective-viewing المرتبطة، وربط سير العمل المعتمد في Torii بعد توقيع مسودة الحوكمة.
- توسيع مخطط witness encryption ليغطي مخرجات متعددة المستلمين وmemos مجمعة، وتوثيق تنسيق envelope لمطوري SDK.
- طلب مراجعة امنية خارجية للدوائر والسجلات واجراءات تدوير المعلمات وارشفة النتائج بجانب تقارير التدقيق الداخلية.
- تحديد واجهات API لتسوية spentness للمدققين ونشر ارشاد نطاق view-key حتى ينفذ مزودو المحافظ نفس دلالات الاثبات.

## مراحل التنفيذ
1. **Phase M0 — Stop-Ship Hardening**
   - ✅ اشتقاق nullifier يتبع الان تصميم Poseidon PRF (`nk`, `rho`, `asset_id`, `chain_id`) مع فرض ترتيب commitments الحتمي في تحديثات ledger.
   - ✅ يفرض التنفيذ حدود حجم proofs وحصص العمليات السرية لكل معاملة/كتلة، رافضا المعاملات المتجاوزة باخطاء حتمية.
   - ✅ يعلن P2P handshake `ConfidentialFeatureDigest` (backend digest + بصمات السجل) ويفشل عدم التطابق حتميا عبر `HandshakeConfidentialMismatch`.
   - ✅ ازالة panics في مسارات تنفيذ السرية واضافة role gating للعقد غير المتوافقة.
   - ⚪ فرض ميزانيات timeout للـ verifier وحدود عمق reorg للـ frontier checkpoints.
     - ✅ تطبيق ميزانيات timeout؛ proofs التي تتجاوز `verify_timeout_ms` تفشل الان حتميا.
     - ✅ احترام `reorg_depth_bound` وقص checkpoints الاقدم من النافذة مع الحفاظ على snapshots حتمية.
   - تقديم `AssetConfidentialPolicy` وpolicy FSM وبوابات enforcement لتعليمات mint/transfer/reveal.
   - الالتزام بـ `conf_features` في رؤوس الكتل ورفض مشاركة المدققين عند اختلاف digestات registry/parameter.
2. **Phase M1 — Registries & Parameters**
   - شحن سجلات `ZkVerifierEntry` و`PedersenParams` و`PoseidonParams` مع عمليات حوكمة ومرساة genesis وادارة الكاش.
   - توصيل syscall لفرض registry lookups وgas schedule IDs وschema hashing وفحوصات الحجم.
   - شحن صيغة payload المشفر v1، متجهات اشتقاق مفاتيح المحافظ، ودعم CLI لادارة مفاتيح السرية.
3. **Phase M2 — Gas & Performance**
   - تنفيذ جدول gas حتمي، عدادات لكل كتلة، وحزم قياس مع تليمتري (verify latency، احجام proofs، رفض mempool).
   - تقوية CommitmentTree checkpoints وLRU loading وnullifier indices للاحمال متعددة الاصول.
4. **Phase M3 — Rotation & Wallet Tooling**
   - تمكين قبول proofs متعددة المعلمات والنسخ؛ دعم activation/deprecation بقيادة الحوكمة مع runbooks انتقال.
   - تقديم تدفقات هجرة لـ wallet SDK/CLI، سير عمل مسح المدققين، وادوات تسوية spentness.
5. **Phase M4 — Audit & Ops**
   - توفير تدفقات مفاتيح المدققين، واجهات selective disclosure، وrunbooks تشغيلية.
   - جدولة مراجعة تشفير/امن خارجية ونشر النتائج في `status.md`.

كل مرحلة تحدث معالم roadmap والاختبارات المرتبطة لضمان التنفيذ الحتمي لشبكة البلوكتشين.

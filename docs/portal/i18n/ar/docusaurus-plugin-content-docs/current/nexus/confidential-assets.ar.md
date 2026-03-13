---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/confidential-assets.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
العنوان: الاصول السرية وتحويلات ZK
الوصف: مخطط المرحلة ج الأوروبين المحمي والسجلات وضوابط التشغيل.
سبيكة: /nexus/Confidential-assets
---
<!--
SPDX-License-Identifier: Apache-2.0
-->
# تصميم الاصول السرية وتحويلات ZK

## الدوافع
- تقديم تدفقات اصول محمية اختيارية اعتبارا من البدء حتى الدومينات من حفظ خصوصية التعاملات دون تغيير الاتجاه.
- متابعة التنفيذ الحرصي عبر التأخر المستمر غير المكتمل مع استمرار التوافق Norito/Kotodama ABI v1.
- تزويد الممولين والمشغلين بضوابط دورة الحياة (تفعيل، تدفق، سحب) للدوائر والمعلمات السرطانية.

##نموذج
- المأمونون الصادقون لكن الفضوليون: ينفذون الاجماع بامانة واختر فحص دفتر الأستاذ/الولاية.
- لا تهتم بالشبكة لبيانات الكتل والمعاملات المرسلة عبر القيل والقال؛ لا نفترض وجود كل القيل والقال خاصة.
- خارج النطاق: تحليل حركة المرور خارج الدفتر، خصوم كميون (يتابعون في PQ roadmap)، والغرض من دفتر الأستاذ.

## نظرة عامة على التصميم
- يمكن للاصول اعلان *مسبح محمي* اضافة الى الارصدة يقترب؛ يتم تمثيل المحمية عبر التزامات الترجمة.
- تغلف الملاحظات `(asset_id, amount, recipient_view_key, blinding, rho)` مع:
  - الالتزام: `Comm = Pedersen(params_id || asset_id || amount || recipient_view_key || blinding)`.
  - الإلغاء: `Null = Poseidon(domain_sep || nk || rho || asset_id || chain_id)` مستقل عن ملاحظات الترتيب.
  - الحمولة المشفرة: `enc_payload = AEAD_XChaCha20Poly1305(ephemeral_shared_key, note_plaintext)`.
- اتصالات اتصالات الحمولات `ConfidentialTransfer` الرمزية بنوريتو:
  - المدخلات العامة: مرساة ميركل، النواقص، الالتزامات الجديدة، معرف الأصل، نسخة النسخ.
  - مشفرة الحمولات المستلمة والمقررين الاختياريين.
  - إثبات المعرفة الصفرية يؤكد الحفاظ على القيمة والملكية والطرح.
- يتم التحكم في التحقق من المفاتيح ومجموعات المعلمات عبر السجلات على الدفتر مع نافذة تفعيل؛ ترفض الاعتراف من البراهين لتشير الى ادخالات مجهولة او مسحوبة.
- تلتزم القدرة الاجماعية ب الهضمت صلاحيات العضوية كي لا تحتفظ بالكتل الا عند تطابق حالة تسجيل والمعلمات.
- استخدامات بناء البراهين Halo2 (Plonkish) بدون إعداد موثوق به؛ Groth16 انواع SNARK وغيرها غير مدعومة عمدا في v1.

### تركيبات حتمية

اغلفة مذكرة سرية تشحن الان مع تركيبات قانونية في `fixtures/confidential/encrypted_payload_v1.json`. التقط مجموعة البيانات المغلف v1 صحيحا مع سيئة تالفة حتى البدء في تطوير SDKs من اثبات تطابق التحليل. زيت بيانات Rust (`crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs`) وسويت Swift (`IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift`) يتحمل التركيب مباشرة، وتوافق Norito ترميز وسطوح الاخطاء وتغطية الانحدار مع خطوط كودك.

يمكن لـ Swift SDKs الان اصدار تعليمات درع بدون غراء JSON مخصص: منشئ `ShieldRequest` مع مذكرة التزام بطول 32 بايت، حمولة مشفر، وبيانات التعريف debit، ثم الدقة `IrohaSDK.submit(shield:keypair:)` (او `submitAndWait`) للتوقيع الشرعي وتمريرها عبر `/v2/pipeline/transactions`. يقوم بالتحقق من جميع الالتزامات، ويمرر `ConfidentialEncryptedPayload` الى Norito التشفير، ويعكس التخطيط `zk::Shield` الموضح ادناه حتى يبقى متزامنا مع الصدأ.

## الالتزامات الاجماع و gating الفان
- لافتة الكتل الكتلية `conf_features = { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`؛ يشارك في الملخص في التجزئة الاجماع ويجب ان يساوي عرض السجل المحلي لقبول القبول.
- يمكن للشبكة الترقية ببرمجة `next_conf_features` مع `activation_height` مستقبلية؛ وحتى ذلك يجب الارتفاع على إنتاج الكتل بعد إصدار الملخص السابق.
- يجب على عقد المحكمين التشغيل مع `confidential.enabled = true` و `assume_valid = false`. لا تسمح فحوصات الانضمام إلى مجموعة المنهيين إذا فشل أي شرط أو اختفت `conf_features` محليا.
- بيانات المصافحة P2P الان تشمل `{ enabled, assume_valid, conf_features }`. يتم رفض أقرانهم الذين يعلنون عن خدمات غير متوافقة بخطأ `HandshakeConfidentialMismatch` ولا يدخلون ابدا في مخالفات الاجماع.
- نتائج التوافق بين المحاضرين، المراقبين، والـ الأقران القديمة الالتقاط في مصفوفة المصافحة ضمن [Node Capability Negotiation](#node-capability-negotiation). توقف فشل المصافحة كـ `HandshakeConfidentialMismatch` وتبقى الـ Peer خارج دوران الاجماع حتى يتطابق الهضم.
- يمكن للمراقب غير المدققين ضبط `assume_valid = true`؛ يطبقون دلتا سرية بشكل عام على التأثير على سلامة الاجماع.

## سياسات الاصول
- عصر كل تعريف اصل `AssetConfidentialPolicy` يحدده المنشئ او عبر التصفح:
  -`TransparentOnly`: الوضع الافتراضي؛ يُسمح فقط بتعليمات الطلاء (`MintAsset`, `TransferAsset`, الخ) ورفض العمليات المحمية.
  - `ShieldedOnly`: يجب ان يستخدم كل الاصدارات والتحويلات تعليمات سرية؛ يحظر `RevealConfidential` حتى لا يرصد الرصد علنا.
  - `Convertible`: يمكن للحامل نقل القيمة بين التباين الشفاف والـ Shielded باستخدام تعليمات on/off-ramp ادناه.
- بعد الالتزام بتقييد ولايات ميكرونيزيا الموحدة لمنع الاهتمام بالاموال:
  - `TransparentOnly → Convertible` (تمكين فوري لـ Shielded Pool).
  - `TransparentOnly → ShieldedOnly` (يتطلب انتقالا معلقا وافذة تحويل).
  - `Convertible → ShieldedOnly` (تاخير ادنى الزامى).
  - `ShieldedOnly → Convertible` (يطلب خطة هجرة ومذكرات بقاء صالحة للصرف).
  - `ShieldedOnly → TransparentOnly` غير محظوظ الا اذا كان Shielded Pool كاملا او تقوم بترميز هجرة تزيل السرية عن الملاحظات المتبقية.
- تعليمات الـ تضبط `pending_transition { new_mode, effective_height, previous_mode, transition_id, conversion_window }` عبر ISI `ScheduleConfidentialPolicyTransition` ويمكنها الغاء التغييرات المجدولة بـ `CancelConfidentialPolicyTransition`. بما في ذلك التحقق من عدم وجود عذراء في زيارة أي ارتفاع في الانتقال، وفشل الادراج بشكل كامل حتمي إذا حدث تغير سياسي في وسط الجزيرة.
- يتم تطبيق الانتقالات المعلقة تلقائيا عند فتح كتلة جديدة: عند دخول ارتفاع نافذة التحويل (لترقيات `ShieldedOnly`) او الوصول الى `effective_height`، يقوم وقت التشغيل بالتالي `AssetConfidentialPolicy` وتحديث البيانات الوصفية `zk.policy` ومسح. إذا بقيت عرضًا شفافًا عند انتقال اللون الأحمر الشفاف `ShieldedOnly`، يغي وقت التشغيل التغيير ويسجل خروجا مع بقاء الوضع السابق.
- مقابض التهيئة `policy_transition_delay_blocks` و `policy_transition_window_blocks` إذا اشعارا ادنى وفترات سماح يسمح بتحكم حول التبديل.
- `pending_transition.transition_id` ويعمل أيضاً كـ مقبض التدقيق؛ يجب على المساهم ذكره عند انهاء او الغاء الانتقالات حتى يقوموا بربط التقارير على/خارج الطريق المنحدر.
- `policy_transition_window_blocks` افتراضيه 720 (حوالي 12 ساعة مع وقت الكتلة 60 ثانية). المتحدة تعقد طلبات التورم التي تحاول اشعارا اقصر.
- تظهر جينيسيس وSlumpat CLI نموذجًا دقيقًا ومعلقًا. القبول المنطقى أمر بالغ الدقة وقت التنفيذ لتاكيد ان كل تعليمة سرية مصرح بها.
- قائمة التحقق من الهجرة - انظر “Migration sequencing” ادناه لخطة الترقية على مراحل مراحلها Milestone M0.

#### متابعة الانتقالات عبر Torii

اختبار المقاومة والمقررون `GET /v2/confidential/assets/{definition_id}/transitions` لفحص `AssetConfidentialPolicy` الذاتي. يحتوي على payload JSON دائما على معرف الأصول، وآخر كتلة ملاحظ، `current_mode` للسياسة، الوضع الفعال عند ذلك الارتفاع (نوافذ البديل للتعبيرتا `Convertible`)، ومعرفات معلمات `vk_set_hash`/Poseidon/Pedersen. عند وجود انتقال إلى المدرج الرد أيضا:

- `transition_id` - مقبض التدقيق المعاد من `ScheduleConfidentialPolicyTransition`.
-`previous_mode`/`new_mode`.
-`effective_height`.
- `conversion_window` و `window_open_height` المشتق (الكتلة التي يجب ان تبدأ فيها المقاطعة تتحول لقطع ShieldedOnly).

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

يشير رد `404` الى عدم وجود تعريف اصل مطابق. عند عدم وجود انتقال مجدول الحقل `pending_transition` مساوي لـ `null`.

### شروط السياسة

| الوضع الحالي | الوضع التالي | المتطلبات | التعامل مع ارتفاع فعال | تعليقات |
|--------------------|------------------|-------------------------------------------------------------------|
| شفاف فقط | للتحويل | تم القيام بإدخالات سجل التحقق/المعلمة. و`ScheduleConfidentialPolicyTransition` مع `effective_height ≥ current_height + policy_transition_delay_blocks`. | ينفذ الانتقال بالضبط عند `effective_height`؛ تصبح محمية بركة متاحا فوريا.                   | الطريقة الافتراضية لحماية البيانات الشخصية مع بقاءك في باريس تلامس.               |
| شفاف فقط | محمية فقط | كما سبق، بالإضافة إلى `policy_transition_window_blocks ≥ 1`.                                                         | يُدخل وقت التشغيل رسميًا `Convertible` عند `effective_height - policy_transition_window_blocks`؛ يتحول الى `ShieldedOnly` عند `effective_height`. | يوفر نافذة تحويل حتمية قبل التعاقد المباشر.   |
| للتحويل | محمية فقط | انتقال مجدول مع `effective_height ≥ current_height + policy_transition_delay_blocks`. لا بد من الانتقام (`transparent_supply == 0`) من خلال تدقيق البيانات الوصفية؛ ويفترض وقت التشغيل ذلك عند القطع. | نفس دلالات النافذة كما اه. اذا كان العرض شفاف غير صفري عند `effective_height` يتم اجهاض الانتقال بـ `PolicyTransitionPrerequisiteFailed`. | يقفل الاصل في سري بالكامل.                                     |
| محمية فقط | للتحويل | انتقال مجدول؛ لا يوجد طارئ جديد (`withdraw_height` غير مضبوط).                                    | يتبدل الوضع عند `effective_height`؛ تعاد فتح تكشف المنحدرات بينما تبقى محمية الملاحظات صالحة.                           | يستخدم لنوافذ الصيانة او مراجعات المرقمين.                                          |
| محمية فقط | شفاف فقط | يجب أن يتم إثبات إثبات `shielded_supply == 0` أو نظام الخبرة `EmergencyUnshield` موقعة (تتطلب تواقيع مدققين). | يفتح نافذة وقت التشغيل `Convertible` قبل `effective_height`؛ عند الارتفاع تفشل أسرار السرية ويعود الاصل الى وضع شفاف فقط. | رحيل كملاذ اخير. يتم الانتقال إلى الغاء تلقائيًا إذا تم إنفاق أي ملاحظة سرية خلال النافذة. |
| أي | نفس الحالي | `CancelConfidentialPolicyTransition` يمسح التغيير المعلق.                                                        | يتم حذف `pending_transition` نهائيًا.                                                                          | الحفاظ على الوضع الراهن؛ مذكور للاكتمال.                                             |الانتقالات غير المدرجة إعلاه ترفض عند تقديمها للتو. يتحقق وقت التشغيل من الشروط المسبقة قبل تطبيق الانتقال المجدول مباشرة؛ فشل الشروط يعيد الاصل الى الاستخدام السابق ويطلق `PolicyTransitionPrerequisiteFailed` عبر الليمتري واحداث الحرية.

### تسلسل الهجرة

1. **إعداد السجلات:** فعال كل مدقق المدخلات والمعلمات المعالم إليها في الخريطة الجغرافية. تعلن عن انعقاد `conf_features` حتى يتطلب الحصول على شهادة أقران من التوافق.
2. **مرحلة الانتقال:** `ScheduleConfidentialPolicyTransition` مع `effective_height` يراعي `policy_transition_delay_blocks`. عند الانتقال نحو `ShieldedOnly` حدد نافذة التحويل (`window ≥ policy_transition_window_blocks`).
3. **نشر إرشادات المشغل:** سجّل `transition_id` المعادي وتوزيع runbook للـ on/off-ramp. تشترك في المحافظات والأحكام في `/v2/confidential/assets/{id}/transitions` عند الوصول إلى أعلى النافذة.
4. **تطبيق النافذة:** عند فتح النافذة لتحويل وقت التشغيل الجغرافي إلى `Convertible`، ويصدر `PolicyTransitionWindowOpened { transition_id }` في الرفض لطلبات الترشيح المتعارضة.
5. **إنهاء أو إحباط:** عند `effective_height` يتحقق وقت التشغيل من المتطلبات المسبقة (عرض الصفر الواضح، لا يوجد حاجة للسحب، الخ). النجاح في قلب السياسة لوضع الطلب؛ غلاف المنتج `PolicyTransitionPrerequisiteFailed`، يمسح التنقل بالتعليق، ويترك السياسة دون تغيير.
6. **ترقيات المخطط:** بعد نجاح الانتقال إلى قراءة نسخة المخطط الاصل (مثلا `asset_definition.v2`) وتطلب ادوات CLI بحث `confidential_policy` عند سلسلة البيانات. نتيجة لتطور وثائق التكوين، يتم البدء في إضافة إعدادات السياسة وبصمات التسجيل قبل إعادة تشغيل المقررين.

الشبكات الجديدة التي تبدأ مع السرية السرية ترمز السياسة الأساسية مباشرة في Genesis. مع ذلك تتبع نفس تسجيل التحقق عند تغيير الاوضاع بعد الاطلاق كي تستمر النوافذ لتبديل حتمية وتملك عدم الاستقرار وقتا للتكيف.

###النسخة Norito تظهر والتفعيل

- يجب ان تتضمن Genesis Manifests `SetParameter` للمفتاح الرئيسي `confidential_registry_root`. يكون الحمولة هو Norito JSON مطابق لـ `ConfidentialRegistryMeta { vk_set_hash: Option<String> }`: احذف الحقل (`null`) عندما لا توجد ادخالات التحقق مراقب، والاقدم سلسلة سداسي عشرية بطول 32 بايت (`0x…`) تساوي الهاش الناتج من `compute_vk_set_hash` على تعليمات التحقق في البيان. لنكن مستعدين إذًا كان العامل مفقودًا أو الهاش لا يتوافق مع كتاب السجل الرمزي.
- ضمان `ConfidentialFeatureDigest::conf_rules_version` بيان تخطيط النسخة on-wire. لشبكات v1 يجب ان يبقى `Some(1)` ويساوي `iroha_config::parameters::defaults::confidential::RULES_VERSION`. عندما تبدأ تعليمات رفع المواد الصلبة، إعادة توليد البيانات، إطلاق البيناريات معًا؛ خلط الجنين المقررين يرفضون الكتل بـ `ConfidentialFeatureDigestMismatch`.
-يجب أن يكون هناك الكثير من بيانات التنشيط لتحديثات السجل وتغييرات دورة حياة المعلمات وانتقالات السياسة بحيث يبقى ملخص متسقا:
  1. طبق اطفال التسجيل الجيد (`Publish*`, `Set*Lifecycle`) في حالة عرض الحالة دون اتصال واحسب الملخص بعد التفعيل باستخدام `compute_confidential_feature_digest`.
  2. اصدِر `SetParameter::custom(confidential_registry_root, {"vk_set_hash": "0x…"})` باستخدام الهاش المحسوب حتى يأخذ الأقران المتاخرون من استعادة الهضم الصحيح حتى اذا فاتتهم تعليمات التسجيل الوسيطة.
  3. ارفق التعليمات `ScheduleConfidentialPolicyTransition`. يجب على كل تعليمة ان تقتبس `transition_id` قوية من الالتالى؛ البيانات التي تنساه سترفضها وقت التشغيل.
  4. احفظ بايتات البيان وبصمة SHA-256 وملخص المستخدم في خطة التفعيل. وأطلقوا من الثلاثة قبل التصويت.
- عندما تتطلب عمليات الاطلاق قطع أكثر، الارتفاع المستهدف في متطلبات المرافق (مثلا `custom.confidential_upgrade_activation_height`). هذا يعطي المدققين دليلا مشفرا بنوريتو على ان المدقق يراقبوا نافذة الاشعار قبل سريان دايجست.

## دورة حياة التحقق والمعلمات
### سجل ZK
- يخزن دفتر الأستاذ `ZkVerifierEntry { vk_id, circuit_id, version, proving_system, curve, public_inputs_schema_hash, vk_hash, vk_len, max_proof_bytes, gas_schedule_id, activation_height, deprecation_height, withdraw_height, status, metadata_uri_cid, vk_bytes_cid }` حيث `proving_system` ثابت حاليًا على `Halo2`.
- زواج `(circuit_id, version)` عالميًا؛ تحافظ على السجل على فهرس ثانوي حسب البيانات الوصفية. معنى تسجيل الزوج مكرر يرفض عند القبول.
- يجب ان يكون `circuit_id` غير كامل ويجب توفير `public_inputs_schema_hash` (عادة hash Blake2b-32 ليميز الادخال الناجح العام للـ verifier). ويرفض القبول التي تهم هذه.
- تعليمات الـتشمل:
  - `PUBLISH` لا يتم إدخال إضافة `Proposed` بالبيانات الوصفية فقط.
  - `ACTIVATE { vk_id, activation_height }` لجدول تفعيل المدخل عند حدود العصر.
  - `DEPRECATE { vk_id, deprecation_height }` أقصى ارتفاع يمكن فيه للبرهان الاشارة الى المدخلات.
  -`WITHDRAW { vk_id, withdraw_height }` لا نهائي؛ الاصول الماثرة سكايلي الانفاق السري بعد سحب الارتفاع حتى تتفعل ادخالات جديدة.
- Genesis واضح التوقيع على `confidential_registry_root` حجم `vk_set_hash` المطابقة للادخالات العضوية؛ لقد تحقق التحقق من هذه الملخص مقابل حالة السجل المحلي قبل الأعضاء الجدد للاجماع.
- تسجيل او تحديث verifier يتطلب `gas_schedule_id`؛ يفرض التحقق ان يكون السجل `Active` دخول وموجودا في فهرس `(circuit_id, version)` وان توفر إثباتات Halo2 `OpenVerifyEnvelope` يطابق `circuit_id` و`vk_hash` و`public_inputs_schema_hash` في السجل المسجل.

### إثبات المفاتيح
- يبقى إثبات المفاتيح خارج الدفتر لكن تشير إلى معرفات content-addressed (`pk_cid`, `pk_hash`, `pk_len`) منشورة مع مدقق البيانات الوصفية.
- تقوم Wallet SDKs بجلب بيانات PK والتحقق من الهاش وحفظها محليا.

### معلمات بيدرسن وبوسيدون
- سجلات استقطاب المستثمرين (`PedersenParams`, `PoseidonParams`) احترام ضوابط دورة التحقق، لذلك فإن منها `params_id`، هاشات المولدات/الثوابت، لتحديد حياة التفعيل/الاستبدال/السحب.
- تفصل الالتزامات والهاشات المجال عبر `params_id` حتى لا يوجد مبدأ المعلمات استخدام انماط بت من مجموعات قديمة؛ يدمج الـ ID في مذكرات الالتزامات وعلامات المجال المبطل.

## ترتيب الحتمي و النواقض
- تحافظ على كل اصل على `CommitmentTree` مع `next_leaf_index`; التزامات كتلة الكتل بترتيب حتمي: معاملات المعاملات بترتيب الكتلة؛ داخل كلوفر ولم تتم عزلات المحمية حسب الرقم `output_idx` المتسلسله.
- `note_position` مشتق من اشجار الشجرة لكنه **ليس** جزء من النقض؛ يستخدم فقط تارات العضوية ضمن شاهد الإثبات.
- ضمان تصميم PRF ثبات nullifier عند إعادة التنظيم؛ ربط مدخل PRF `{ nk, note_preimage_hash, asset_id, chain_id, params_id }`، وتستند المراسي إلى ضرورة قدرة ميركل المحدودة بـ `max_anchor_age_blocks`.

## دفتر الأستاذ المتدفق
1. **MintConfidential { معرف الأصول، المبلغ، المستلم_تلميح }**
   - يتطلب نقل اصل `Convertible` او `ShieldedOnly`; في حين أن القبول من سلطة الاصل، يأتي `params_id` الحالي، يعين `rho`، ويأتي الالتزام ويحدث شجرة ميركل.
   - يصدر `ConfidentialEvent::Shielded` مع التزام جديد، فرق Merkle root، وhash للمعاملة المخصصة للتدقيق.
2. **TransferConfidential { معرف الأصول، الإثبات، معرف_الدائرة، الإصدار، الإبطال، التزامات_جديدة، enc_payloads، مرساة_جذر، مذكرة }**
   - ويتحقق syscall في VM من إثبات استخدام السجل؛ يضمن المضيف ان مبطلات غير مستخدمة، الالتزامات تضاف حطمية، ومثبت حديث.
   - تسجيل دفتر الأستاذ ادخالات `NullifierSet`، ويحفظ الحمولة مشفرة للمستلمين/المشرف، ويصدر `ConfidentialEvent::Transferred` ملخصا nullifiers والمخرجات بنجاح وإثبات التجزئة وجذور Merkle.
3. **RevealConfidential { معرف الأصول، الإثبات، معرف_الدائرة، الإصدار، المبطل، المبلغ، حساب_المستلم، جذر_المرساة }**
   - ببساطة للاصول `Convertible`; يحقق إثبات ان قيمة المذكرة تساوي المكشوف، تحميل دفتر الأستاذ الرصيد الشفاف ويحرق المذكرة المحمية بسم nullifier كمصروف.
   - يحصل `ConfidentialEvent::Unshielded` مع السعال الديكي ومبطلات الدواء ومعرفات الإثبات والمعاملة المخصصة.

## نموذج بيانات الاضافات
- `ConfidentialConfig` (قسم تهيئة جديد) مع علم التفعيل، `assume_valid`، متحكمات الغاز/الحدود، مرساة النافذة، الواجهة الخلفية للتحقق.
- `ConfidentialNote` و`ConfidentialTransfer` و`ConfidentialMint` مخططات Norito مع نسخة بيضاء صريحة (`CONFIDENTIAL_ASSET_V1 = 0x01`).
- `ConfidentialEncryptedPayload` يطبع بايتات مذكرة EAAD في `{ version, ephemeral_pubkey, nonce, ciphertext }`، وافتراضيا `version = CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1` لتخطيط XChaCha20-Poly1305.
- لا توجد اتجاهات اشتقاق المفاتيح القانونية في `docs/source/confidential_key_vectors.json`; كل من CLI وTorii endpoint يرجعان إليها في حالة القميص.
- حصل على `asset::AssetDefinition` على `confidential_policy: AssetConfidentialPolicy { mode, vk_set_hash, poseidon_params_id, pedersen_params_id, pending_transition }`.
- يحفظ `ZkAssetState` ربط `(backend, name, commitment)` لـ أدوات التحقق من النقل/إلغاء الحماية؛ رفض تنفيذ البراهين التي لا تتطابق مع التحقق من مفتاح المستخدم (مرجعا او ضمنيا).
- يتم تخزين `CommitmentTree` (لكل اصل مع نقاط التفتيش الحدودية)، و `NullifierSet` بالمفتاح `(chain_id, asset_id, nullifier)`، و `ZkVerifierEntry` و `PedersenParams` و `PoseidonParams` في حالة العالم.
- التأكيد على هياكل mempool `NullifierIndex` و `AnchorIndex` والمخيم المبكر عن التكرار والتحقق من عمر مرساة.
- تحديثات مخطط Norito تشمل ترتيبًا قانونيًا لـ المدخلات العامة؛ تحسين الجودة ذهاباً وإياباً بحيث حتمية الترميز.
- تثبت ذهاباً وإياباً للحمولة المشفرة عبر اختبارات الوحدة (`crates/iroha_data_model/src/confidential.rs`). إلى الأمام إلى حدود محددة لاحقًا يوثق `norito.md` ترويسة on-wire للـ Envelope.

## تكامل IVM و syscall
- تقديم syscall `VERIFY_CONFIDENTIAL_PROOF` يقبل:
  - `circuit_id`, `version`, `scheme`, `public_inputs`, `proof` والنتيجة `ConfidentialStateDelta { asset_id, nullifiers, commitments, enc_payloads }`.
  - يقوم syscall بتحميل البيانات الوصفية للتحقق من السجل، يفرض النطاق/الوقت، ولا يحسب الغاز حتمي، يطبق دلتا الا عند نجاح الدليل.
- يوفر سمة المضيف للقراءة فقط `ConfidentialLedger` لاسترجاع اللقطات لجذر Merkle وحالة nullifier؛ توفر مكتبة Kotodama helpers لجميع الشهود والتحقق من المخطط.
- تم تحديث وثائق pointer-ABI لتوضيح المخزن المؤقت لإثبات التخطيط وإيقاف التسجيل.

## التفاوض على العقد
- أعلن عن المصافحة `feature_bits.confidential` مع `ConfidentialFeatureDigest { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`. مشاركة المتطلب المطلوب `confidential.enabled=true` و`assume_valid=false` ومعرفات verifier backend متطابقة وهضم متطابقة؛ الفروقات فشل المصافحة بـ `HandshakeConfidentialMismatch`.
- يدعم config `assume_valid` لعقد المراقبين فقط: عند التسكه، تعليمات التعليمات السرية الى `UnsupportedInstruction` حتمي بلا ذعر؛ عند استعماله، يطبق المراقبون دلتا البائع دون التحقق من البراهين.
- رفض المعاملات السرية اذا كانت قادرة على المجازفة. تجنب فلاتر ثرثرة التعاملات المحمية إلى الأقران غير المتوافقين أثناء توجيه معرفات التحقق غير المعروفة بشكل عام ضمن نطاق النطاق.

### مصفوفة توافق المصافحة| اعلان البعيد | النتيجة لعقد المعقد | أفكار لتشغيل |
|----------------------|----------------------------|----------------|
| `enabled=true`, `assume_valid=false`, الخلفية متطابقة, الهضم متطابق | مقبول | يصل نظير الى حالة `Ready` ويشارك في الاقتراح والتصويت وRBC fan-out. لا تحتاج إلى يد. |
| `enabled=true`, `assume_valid=false`, backend متطابق, الهضم قديم او مفقود | مرفوض (`HandshakeConfidentialMismatch`) | يجب على الطرف البعيد تطبيق تسجيل/المعلمات المعلقة او توقع `activation_height` المجدولة. حتى يبقى التصحيح قابلا للاكتشاف لكنه لم يدخل خلال الاجماع. |
| `enabled=true`، `assume_valid=true` | مرفوض (`HandshakeConfidentialMismatch`) | يلزم المدققون التحقق من البراهين؛ قم بضبط الطرف البعيد كمراقب عبر Torii فقط او اجعل `assume_valid=false` بعد استخدام التحقق الكامل. |
| `enabled=false`, حذف المصافحة (بناء قديم) او الاختلاف verifier backend | مرفوض (`HandshakeConfidentialMismatch`) | Peers قديمة او محدثة جزئيا لا الانضمام لشبكة الاجماع. قم بترقيتها وتأكد من تطابق الواجهة الخلفية + الملخص قبل إعادة الاتصال. |

العقد المراقِبة التي تتجاوز عدد التحقق من البراهين عمدا يجب الا تفتح اتصالات اجماع مع ممرضين يعملون ببوابات القدرات. يقدر استيعاب الكتل عبر Torii او واجهات الارشفة، لكن شبكة الاجماع ترفضها حتى تعلن قدرات مرغوبة.

### بولي سليم Reveal والاحتفاظ بـ Nullifier

يجب على دفاتر حقوق الضحايا بتاريخ كاف لاثبات حداثة الملاحظات واعادة تشغيل تدقيقات ال تورنتو. الراغبين في تصميمها `ConfidentialLedger` هي:

- **الاحتفاظ بالبطلان:** إصلاح بـ nullifiers المصروفة للتدريب *ادنى* `730` يوما (24 شهرا) بعد ارتفاع الصرف، او لاطول اذا فرضها. يمكن للمشغلين تمديداتها عبر `confidential.retention.nullifier_days`. يجب ان تستمر المبطلات ضمن النافذة للاستعلام عبر Torii حتى المنهي عن إثبات عدم وجود إنفاق مزدوج.
- **كشف التقليم:** أنت تكشف تتواصل (`RevealConfidential`) بتقليم الالتزامات ثم فورا بعد الدستور، لكن المبطل المعتمد يظل ينتظرعا لقاعدة الإصلاح. بسبب حداثة الكشف (`ConfidentialEvent::Unshielded`)أكوا العلني والمستلم وhash إثبات كي لا تتطلب إعادة بناء الكشوف النص المشفر التاريخي المقتطع.
- **نقاط التفتيش الحدودية:** تحافظ على التزامات الحدود على نقاط التفتيش لتغطية الاكبر من `max_anchor_age_blocks` ونافذة الإصلاح. لا تبدأ بضغط نقاط التفتيش الا بعد انتهاء كل المبطلات في تلك الفترة.
- **معالجة الملخص الذي لا معنى له:** اذا رُفع `HandshakeConfidentialMismatch` بسبب انحراف الملخص، يجب تشغيلهم (1) التحقق من تطابق نوافذ الإصلاح عبر الجزيرة، (2) `iroha_cli app confidential verify-ledger` عدم إعادة إنشاء الملخص مقابل مجموعة الإبطال المحتفظ بها، و(3) إعادة نشر البيان المحدث. يجب استعادة أي مُبطلات تم حذفها بالكامل من التجديد قبل إعادة الانضمام للشبكة.

الثقة المحلية في دليل العمليات؛ يجب أن تتحدث السياسات التي تهدف إلى توفير الراحة للجميع عن تهيئة وخطط التخزين الارشيفية بالتزامن.

### تدفق الخلاص والاستعادة

1. أثناء الهاتف، يقارن `IrohaNetwork` الينة. اي لا يتوافق مع `HandshakeConfidentialMismatch`؛ يغلق الاتصال ويبقى النظير في قائمة انتظار الاكتشاف دون ترقيته الى `Ready`.
2. تذكير المشكلة في سجل خدمة الشبكة (مع الملخص والـ backend للطرف البعيد)، ولا يقوم Sumeragi بجدولة النظير لتقديم او التصويت.
3. تم تشغيلها بسبب الأمر بملفات المحققين ومجموعات المعلمات (`vk_set_hash`, `pedersen_params_id`, `poseidon_params_id`) او بتهيئة `next_conf_features` مع `activation_height` متفق عليها. عندما يتطابق الهضم بنجاح المصافحة التالية تلقائيا.
4. اذا تمكن النظير قديم من بث الكتلة (مثلا عبر الإعادة الأرشيفية)، يرفضها المترددون حتمي بـ `BlockRejectionReason::ConfidentialFeatureDigestMismatch` على اتساق دفتر الأستاذ عبر الشبكة.

### إعادة تدفق المصافحة الآمنة

1. كل محاولة لتجربة لوحة المفاتيح Noise/X25519 جديدة. الـ حمولة المصافحة الموقع (`handshake_signature_payload`) الارتباط المفاتيح العامة المؤقتة المؤقتة والبعيدة، عنوان المقبس ولدى والمشفّر بنوريتو، ومعرف الهيكل عند التركيب مع `handshake_chain_id`. تتم ترجمة الرسالة بـ AEAD قبل إرسالها.
2. استعادة المستجيب حساب الحمولة بترتيب مفاتيح النظير/المعيكوس المحلي ويتحقق من التوقيع Ed25519 المضمن في `HandshakeHelloV1`. بما في ذلك كلا المفتاحين المؤقتين والعنوان ضمن نطاق التوقيع، فاعادة تشغيل رسالة ملتقطة ضد نظير اخرى او استعادة اتصال قديم تفشل حطميا.
3. لا يوجد اعلام سري و `ConfidentialFeatureDigest` داخل `HandshakeConfidentialMeta`. يقارن المستقبل tuple `{ enabled, assume_valid, verifier_backend, digest }` مع `ConfidentialHandshakeCaps` المحلية؛ اي لا تطابق ينهي المصافحة بـ `HandshakeConfidentialMismatch` قبل انتقال النقل الى `Ready`.
4. يجب على البدء في إعادة حساب ملخص (عبر `compute_confidential_feature_digest`) واعادة تشغيل سياسات/سجلات محدثة قبل إعادة الاتصال. أقرانهم الذين يعلنون عن خلاصات تستمر حتى نهاية العام، يمنعون دخول حالة طويلة حتى مجموعة المنتهيين.
5. نجاح قياسي وفشل في المصافحة تحدث العدادات `iroha_p2p::peer` (`handshake_failure_count` وغيرها) ومؤشر ارشيف المنظمة مع تحديد معرف النظير البعيد وبصمة الإصبع Digest. راقب هذه المؤشرات المستخدمة في إعادة التشغيل او سوء التهيئة أثناء الطرح.

## إدارة المهام والحمولات
- السلسلة اشتقاق المفاتيح لكل حساب:
  - `sk_spend` → `nk` (مفتاح الإلغاء)، `ivk` (مفتاح العرض الوارد)، `ovk` (مفتاح العرض الصادر)، `fvk`.
- تستخدم ملاحظات الحمولات النافعة المشفرة AEAD مع مفاتيح التعاون التعاونية من ECDH؛ ويمكن ارفاق المدقق عرض المفاتيح اختيارية الى المخرجات حسب الطلب لبني الاصل.
- اضافات CLI: `confidential create-keys`, `confidential send`, `confidential export-view-key`, ادوات للمقررين لفك المذكرات، والمساعد `iroha app zk envelope` لانتاج/فحص المغلفات Norito دون اتصال. يعرض Torii نفس تدفق الاشتقاق عبر `POST /v2/confidential/derive-keyset` ويعيد اشكالا hex وbase64 لكي يحافظ على هياكل المفاتيح برمجيا.

## الغاز، الحدود، وضوابط DoS
- جدول الغاز حتمي :
  - Halo2 (بلونكيش): أساس `250_000` غاز + `2_000` غاز لكل مدخل عام.
  - `5` مقاوم لضغط الغاز لكل لتر، مع لكل مُبطل (`300`) التزام (`500`).
  - يمكن للمشغلين تجاوز هذه الثوابت عبر تهيئة الحفل (`confidential.gas.{proof_base, per_public_input, per_proof_byte, per_nullifier, per_commitment}`)؛ تتماشى مع التغييرات عند البدء او إعادة تحميل المؤثرات وطبقت حطميا عبر الحرية.
- النطاق المسموح به (افتراضات قابلة للضبط):
-`max_proof_size_bytes = 262_144`.
- `max_nullifiers_per_tx = 8`، `max_commitments_per_tx = 8`، `max_confidential_ops_per_block = 256`.
- `verify_timeout_ms = 750`، `max_anchor_age_blocks = 10_000`. البراهين التي تجاوزت `verify_timeout_ms` للتحكم في هتميا (تصويتات التورم `proof verification exceeded timeout` و`VerifyProof` تعيد خطا).
- حجز مضمون صحي: `max_proof_bytes_block`, `max_verify_calls_per_tx`, `max_verify_calls_per_block`, و `max_public_inputs` تحد بناء الكتل؛ `reorg_depth_bound` (≥ `max_anchor_age_blocks`) يتحكم في الاحتفاظ بنقاط التفتيش الحدودية.
- رفض المعاملات التي تتجاوز هذه الحدود لكل الحدود او لكل كتلة، ويصدر اخطاء `InvalidParameter` حتمية مع بقاء حالة دفتر الأستاذ دون تغيير.
- يرش ميمبول التعاملات السرية مسبقة الصنع `vk_id` وطول الإثبات وعمر مرساة قبل الاتصال المتحقق من الوكالة على حدود الموارد.
- تم التحقق من التحقق حتمياً عند انتهاء المهلة تجاوز الحدود؛ فشل المعاملات مع أخطاء القضاء. backends SIMD اختيارية ولكن لم يتغير حساب الغاز.

### خطوط أساسية للتجديد وبوابات التكيف
- **المنصات المرجعية.** يجب ان تغطي معايرات الاداء ثلاث ملفات جير ادناه. اي تجديد لا تلتقط جميع الملفات والعكس صحيح.

  | الملف | توريه | وحدة المعالجة المركزية / المثيل | اعلام مترجم | النهاية |
  | --- | --- | --- | --- | --- |
  | `baseline-simd-neutral` | `x86_64` | AMD EPYC 7B12 (32c) او Intel Xeon Gold 6430 (24c) | `RUSTFLAGS="-C target-feature=-avx,-avx2,-fma"` | تحديد قواعد أرضية بدون تعليمات متجهة؛ يستخدم لضبط جداول التكلفة الاحتياطية. |
  | `baseline-avx2` | `x86_64` | إنتل زيون جولد 6430 (24 سي) | الإصدار الافتراضي | يحقق من مسار AVX2؛ يفحص ان تسريعات SIMD ضمن تسامح غاز العادل. |
  | `baseline-neon` | `aarch64` | AWS Graviton3 (c7g.4xlarge) | الإصدار الافتراضي | ضمان البقاء backend NEON حتمي ومتوافق مع جداول x86. |

- **الأداة المعيارية.** يجب إنتاج كل التقارير وتعديل الغاز باستخدام:
  -`CRITERION_HOME=target/criterion cargo bench -p iroha_core isi_gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>`
  - `cargo test -p iroha_core bench_repro -- --ignored` لتأكيد التركيبات الحتمية.
  - `CRITERION_HOME=target/criterion cargo bench -p ivm gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>` كلما تغيرت تكاليف رمز التشغيل VM.

- **العشوائية الثابتة.** صدّر `IROHA_CONF_GAS_SEED=conf-gas-seed-2026Q1` قبل تشغيل البنش حتى يتحول `iroha_test_samples::gen_account_in` الى مسار `KeyPair::from_seed` حطمي. يطبع تسخير `IROHA_CONF_GAS_SEED_ACTIVE=…` مرة واحدة؛ إذا كان حساب المتغير يجب ان يفشل في المراجعة. ويجب على أي ادوات تجديد استعادة هذا المتغير عند ادخال محمية بشكل جديد.

- **التقاط النتائج.**
  - ارفع ملخصات المعايير (`target/criterion/**/raw.csv`) لكل ملف الى artefact الاصدار.
  - خُنّ المقاييس المشتقة (`ns/op`, `gas/op`, `ns/gas`) في [سجل معايرة الغاز السري](./confidential-gas-calibration) مع git Commit ونسخة مترجمة مستعملة.
  - احتفظ بآخر خط الأساس لكل ملف؛ احذف المقطع الاقدم بعد الاعتماد على التقرير الاحدث.

- **تفاوتات القبول.**
  - يجب ان تبقى فروقات الغاز بين `baseline-simd-neutral` و`baseline-avx2` ضمن ≥ ±1.5%.
  - يجب ان تبقى فروقات الغاز بين `baseline-simd-neutral` و`baseline-neon` ضمن ≥ ±2.0%.
  - اقترحات التي تتجاوز هذه الحدود مطالبات جدول او RFC يشرح الجمعية المتعددة وما لها من حقوق.

- **قائمة مراجعة المراجعة.** يتحمل مسئولية مقدمو:
  - تضمين `uname -a` ومقتطفات `/proc/cpuinfo` (النموذج، الخطوة) و `rustc -Vv` في سجل التجديد.
  - التحقق من ظهور `IROHA_CONF_GAS_SEED` في الخارج البنش (البنش يطبع البذور المفضلة).
  - ضمان تطابق جهاز تنظيم ضربات القلب ويتميز بمدقق سري مع الإنتاج (`--features confidential,telemetry` عند تشغيل البنش مع القياس عن بعد).

## التهيئة للظروف
- التحميل `iroha_config` قسم `[confidential]`:
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
- يزداد التليمتري مقاييس مجمعة: `confidential_proof_verified`, `confidential_verifier_latency_ms`, `confidential_proof_bytes_total`, `confidential_nullifier_spent`, `confidential_commitments_appended`, `confidential_mempool_rejected_total{reason}`, و `confidential_policy_transitions_total` دون كشف البيانات العادية.
- سطوح RPC:
  -`GET /confidential/capabilities`
  -`GET /confidential/zk_registry`
  -`GET /confidential/params`## استراتيجية الاختبار
- الحتمية: خلط المعاملات مباشرة بين الكتل الناتجة نفس جذور ميركل ومجموعات مبطلات المواد.
- تحمل إعادة التنظيم: محاكاة إعادة تنظيم كتل متعددة مع المراسي؛ تبقى مبطلات العمر وترفض المراسي القديمة.
- ثوابت الغاز: متأكد من نفس استخدام عبر عقد مع او بدون تسريع SIMD.
- اختبار الحدود: البراهين عند سقوف الحجم/الغاز، الحد الاقصى للمدخلات/المخرجات، تطبيقات المهلة.
- دورة الحياة: عمليات الصلاحيات لتفعيل/إصلاح المدقق والمعلمات، والتحقق من صحتها.
- سياسة FSM: الانتقالات العامة/المرفوضة، تأخيرات في انتظار الانتقال، ورفض mempool حول الارتفاعات الفعالة.
- طوارئ السجل: سحب طارئ لإجمد الاصول المتاثرة عند `withdraw_height` ويرفض البراهين تماما.
- بوابة القدرة: المتمرسون مع `conf_features` غير متطابقة يرفضون الكتل؛ المراقبون مع `assume_valid=true` يواكبون دون التأثير على الاجماع.
- تعاون الحالة: عقد المصادق/الكامل/المراقب ينتج نفس جذور الدولة على الوثيقة القانونية.
- التشويش السلبي: البراهين تالفة، الحمولات كبيرة جدا، وتصادمات لاغية ترفض حتمياً.

## الهجرة والتوافق
- خطة محكوم بالميزة: حتى القانوني Phase C3، `enabled` افتراضيه `false`؛ تعلن عن قدراتها قبل الانضمام الى مجموعة المتبرعين.
- الاصول تلامس غير متأثرة؛ تتطلب التعليمات السرية إدخالات السجل والتفاوض على التطبيق.
- العقد المترجمة دون دعم الحظر الشامل للكتل هتميا؛ لا يجوز الانضمام لعدد من المقررين لكنها قد تتصل كمراقبين مع `assume_valid=true`.
- تتضمن بيانات التكوين ادخالات السجل الأولية، مجموعات معلمات، سياسات سرية للاصول، ومفاتيح امتارين اختيارية.
- يستخدم لتدوير كتب التشغيل المنشورة لسجل وانتقالات السياسة وسحب الطارئ الطارئ على ترقية حتمية.

##اعمال متبقية
- قياس مجموعات معلمات Halo2 (حجم الدائرة، استراتيجية البحث) وتسجيل النتائج في دليل التشغيل للمعايرة حتى يمكن تحديث عدادات الغاز/المهلة مع تحديث `confidential_assets_calibration.md` القادمة.
- انهاء سياسة الاشراف على المشرفين وواجهات المشاهدة الانتقائية، وربط سير العمل المعتمد في Torii بعد توقيع مسودة ال تور.
- ينتهي مخطط شاهد التشفير ليغطي المخرجات المتعددة المستلمين والمذكرات المجمعة، وتوثيق المغلف لمطوري SDK.
- طلب المعلم تفصيلية أمنية للدوائر والسجلات وإجراءات دراماتيكية وارشفة النتائج استكمال التقارير الداخلية والخارجية.
- التعرف على واجهات برمجة التطبيقات (API) لتسويق الإنفاق للمشرفين وارشاد نطاق مفتاح العرض حتى ينفذوا الاختبارات نفس دلالات الاثبات.

## مراحل التنفيذ
1. **المرحلة M0 — تصلب إيقاف السفينة**
   - ✅ اشتقاق nullifier يستخدم الان تصميم Poseidon PRF (`nk`, `rho`, `asset_id`, `chain_id`) مع فرض ترتيب الالتزامات حتمي في تحديثات دفتر الأستاذ.
   - ✅ يفرض التنفيذ حدود حجم البراهين وحصص العمليات السرية لكل تصفية/كتلة، رافضا المعاملات المتجاوزة باخطاء حتمية.
   - ✅ تأكيد مصافحة P2P `ConfidentialFeatureDigest` (ملخص الواجهة الخلفية + سجل البصمات) ويفشل عدم التطابق حطميًا عبر `HandshakeConfidentialMismatch`.
   - ✅ ازالة الذعر في مسارات تنفيذ السرية واضافة بوابة الدور للعقد غير المتوافقة.
   - ⚪ افتراض المهلة الزمنية للـ التحقق وحدود عمق إعادة التنظيم لـ نقاط التفتيش الحدودية.
     - ✅ تطبيق مؤقتات الحرارة؛ البراهين التي تتجاوز `verify_timeout_ms` تفشل الان حطميا.
     - ✅ احترام `reorg_depth_bound` وقص نقاط التفتيش الاقدم من النافذة مع التمتع باللقطات حتمية.
   - تقديم `AssetConfidentialPolicy` وسياسة FSM وبوابات إنفاذ تعليمات النعناع/النقل/الكشف.
   - الالتزام بـ `conf_features` في الكتلة الكتلية ورفض مشاركة المدققين عند إختلاف ملخصات التسجيل/المعلمة.
2. **المرحلة M1 — السجلات والمعلمات**
   - شحن السجلات `ZkVerifierEntry` و`PedersenParams` و`PoseidonParams` مع عمليات تورتر ومرساة genesis وادارة الكاش.
   - توصيل syscall لفرض عمليات البحث عن التسجيل ومعرفات جدول الغاز وتجزئة المخطط والفحوصات الحجمية.
   - شحن صيغة الحمولة المشفر v1، متجهات اشتقاق مفاتيح المحافظة، ودعم CLI لادارة مفاتيح السرية.
3. **المرحلة M2 — الغاز والأداء**
   - تنفيذ خطة غاز حتمي، عدادات لكل كتلة، وحزم قياس مع تليمتري (التحقق من الكمون، اثباتات، رفض الذاكرة).
   - تقوية نقاط تفتيش CommitmentTree وLRU upload وnullifier indices للاحمال متعددة الاصول.
4. **المرحلة M3 — أدوات التدوير والمحفظة**
   - يجب أن تقبل البراهين متعددة المعلمات والنسخ؛ دعم التنشيط/الإهمال بمساعدة دفاتر التشغيل.
   - تقديم تدفقات هجرة لـ المحفظة SDK/CLI، سير عمل المسح المدققين، ودوات اتفاقية الإنفاق.
5. **المرحلة م4 — التدقيق والعمليات**
   - توفير تدفقات المفاتيح المعتمدة، واجهات الإفصاح الانتقائي، والسجلات التشغيلية.
   - جدول تفصيلي ترجمة/امن ونشر النتائج في `status.md`.

كل مرحلة تعرف معالم وخريطة الطريق اختباراتات حتى التنفيذ هتمي لشبكة البلوكشين.
---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/confidential-assets.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
العنوان: Ativos confidenciais e Transferencias ZK
الوصف: مخطط المرحلة C للتداول الشامل والسجلات وضوابط المشغل.
سبيكة: /nexus/Confidential-assets
---
<!--
SPDX-License-Identifier: Apache-2.0
-->
# Design de ativos Confidenciais e Transferencias ZK

## موتيفاكاو
- استمتع بتدفقات الأنشطة العمياء التي يمكنك الاشتراك فيها للحفاظ على خصوصية المعاملات دون تغيير الشفافية.
- التحكم في تنفيذ عمليات التحقق من الأجهزة غير المتجانسة والحفاظ على Norito/Kotodama ABI v1.
- توفير المراجعين والمشغلين الذين يتحكمون في دورة الحياة (تنشيط، تدوير، إعادة تشغيل) للدوائر ومعلمات التشفير.

## نموذج التهديد
- المصدقون صادقون ولكن فضوليون: تنفيذ الموافقة المسبقة عن علم بشكل صحيح بالإضافة إلى فحص دفتر الأستاذ/الحالة.
- المراقبون من ريدي فيم دادوس دي بلوك إي ترانساكويس ثرثرة؛ لا أعتقد أن هناك قنوات خاصة بالقيل والقال.
- منتديات الهدف: تحليل حركة المرور خارج دفتر الأستاذ، والخصوم الكميين (المصاحبين بدون خريطة طريق PQ)، وهجمات التوفر في دفتر الأستاذ.

## تصميم فيساو جيرال
- يمكن للأصول الإعلان عن *تجمع محمي* على الرغم من الأرصدة الشفافة الموجودة؛ يتم تداولها وتمثيلها من خلال الالتزامات المشفرة.
- ملاحظات مغلف `(asset_id, amount, recipient_view_key, blinding, rho)` كوم:
  - الالتزام: `Comm = Pedersen(params_id || asset_id || amount || recipient_view_key || blinding)`.
  - الإلغاء: `Null = Poseidon(domain_sep || nk || rho || asset_id || chain_id)`، مستقل عن ترتيب الملاحظات.
  - تشفير الحمولة: `enc_payload = AEAD_XChaCha20Poly1305(ephemeral_shared_key, note_plaintext)`.
- حمولات نقل Transacoes `ConfidentialTransfer` المشفرة في Norito المتنافسة:
  - المدخلات العامة: مرساة Merkle، والإبطال، والالتزامات الجديدة، ومعرف الأصول، وعكس الدائرة.
  - الحمولات المشفرة للمستلمين والمراجعين الاختياريين.
  - إثبات عدم المعرفة بأن atesta يحافظ على الشجاعة والملكية والتفويض.
- التحقق من المفاتيح ومجموعات المعلمات التي يتم التحكم فيها عبر السجلات الموجودة في دفتر الأستاذ مع سجلات التنشيط؛ تطلب العقد الأدلة الصالحة التي تشير إلى الإدخالات المحذوفة أو المجددة.
- تهدد رؤوس التوافق أو ملخص السعة السرية بحيث يتم تجميع الكتل عندما تتزامن حالة التسجيل والمعلمات.
- يتم إنشاء البراهين باستخدام مكدس Halo2 (Plonkish) وهو إعداد موثوق به؛ Groth16 أو المتغيرات الأخرى SNARK تم تصميمها بشكل غير مدعوم على الإصدار 1.

### تحديد المباريات

مغلفات المذكرات السرية الآن ترسل إلى وحدة تثبيت Canon في `fixtures/confidential/encrypted_payload_v1.json`. تلتقط مجموعة البيانات مظروفًا v1 إيجابيًا مع بعض العيوب السلبية حتى تتمكن أدوات تطوير البرامج (SDK) من تأكيد تكافؤ التحليل. تقوم الاختبارات بنموذج البيانات في Rust (`crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs`) ومجموعة Swift (`IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift`) التي يتم تحميلها أو تثبيتها مباشرة، مما يضمن ترميز Norito، كسطح للخطأ وتغطية التراجع بشكل دائم أثناء تطور برنامج الترميز.

SDKs Swift Agora podem emitir instrucoes Shield sem الغراء JSON مفصل: البناء
`ShieldRequest` مع التزام بـ 32 بايت، أو حمولة مشفرة وبيانات وصفية للدين،
قم بالاتصال بـ `IrohaSDK.submit(shield:keypair:)` (ou `submitAndWait`) للتثبيت والتسجيل
Transacao عبر `/v1/pipeline/transactions`. يا مساعد التحقق من التزام الالتزام،
أدخل `ConfidentialEncryptedPayload` بدون برنامج تشفير Norito، وقم بالتبديل أو التخطيط `zk::Shield`
وصف أدناه لكيفية تثبيت المحفظة على موقع Rust.

## التزامات الإجماع وبوابة القدرة
- رؤوس الكتل المعرضة `conf_features = { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`؛ o ملخص المشاركة في التجزئة المتفق عليها ويجب أن تكون متساوية مع التسجيل المحلي للحصول على الكتلة.
- يمكن لبرنامج الحوكمة إعداد ترقيات `next_conf_features` com um `activation_height` المستقبل؛ بعد أن ارتفع هذا الارتفاع، يجب على منتجي الكتلة أن يستمروا في إصداره أو هضمه مسبقًا.
- عقد التحقق من صحة تشغيل DEVEM com `confidential.enabled = true` و`assume_valid = false`. تم اقتراح عمليات فحص بدء التشغيل حتى لا يتم تعيين مدقق إذا كانت الحالة خاطئة أو إذا كانت `conf_features` متباينة محلية.
- البيانات الوصفية للمصافحة P2P تتضمن `{ enabled, assume_valid, conf_features }`. أقرانهم الذين أعلنوا عن ميزات غير متوافقة قد تم قبولهم مع `HandshakeConfidentialMismatch` ولم يدخلوا في توافق الآراء.
- نتائج المصافحة بين المدققين والمراقبين والأقران الملتقطين في مصفوفة المصافحة في [التفاوض بشأن قدرة العقدة](#node-capability-negotiation). تعرض أخطاء المصافحة `HandshakeConfidentialMismatch` وتحافظ على نظيرك في تبادل الآراء حتى يتزامن الملخص.
- المراقبون ليس لديهم مصادقة يمكن تحديدها `assume_valid = true`؛ إن تطبيق دلتا الثقة بدون أدلة مؤكدة، لكنه لا يؤثر على ضمان التوافق.

## سياسة الأصول
- كل تعريف للأصول المنقولة `AssetConfidentialPolicy` محدد من قبل المنشئ أو عبر الإدارة:
  - `TransparentOnly`: الوضع الافتراضي؛ تعليمات شفافة (`MintAsset`، `TransferAsset`، وما إلى ذلك) تسمح بالتشغيل والتشغيل المحمي.
  - `ShieldedOnly`: جميع عمليات الإرسال والتحويل يجب أن تستخدم تعليمات سرية؛ `RevealConfidential` ويمنع عدم ظهور التوازنات بشكل علني.
  - `Convertible`: يمكن للحاملين تحريك القيمة بين الأشكال الشفافة والمحمية باستخدام تعليمات التشغيل/الخروج من المنحدر.
- السياسة مستمرة في تقييد ولايات ميكرونيزيا الموحدة لتجنب التراكمات العميقة:
  - `TransparentOnly -> Convertible` (التأهيل الفوري لحمام السباحة المحمي).
  - `TransparentOnly -> ShieldedOnly` (يطلب تحويل المكالمات ورسائل المحادثة).
  - `Convertible -> ShieldedOnly` (تأخير الحد الأدنى من obrigatorio).
  - `ShieldedOnly -> Convertible` (مخطط الهجرة المطلوب لتدوين الملاحظات المستمرة للمضي قدمًا).
  - `ShieldedOnly -> TransparentOnly` ويمنعك من استخدام المجموعة المحمية أو تدوين الحوكمة لترحيل الملاحظات المعلقة.
- تحدد تعليمات الإدارة `pending_transition { new_mode, effective_height, previous_mode, transition_id, conversion_window }` عبر o ISI `ScheduleConfidentialPolicyTransition` ويمكنها إلغاء برامج Mudancas com `CancelConfidentialPolicyTransition`. تضمن صحة الذاكرة عدم وصول أي معاملة إلى ارتفاع في التحويلات، وتتضمن خطأً حتميًا في حالة إجراء فحص سياسي لا يساعد على منع الكتلة.
- التحويلات المعلقة يتم تطبيقها تلقائيًا عند فتح كتلة جديدة: عند الارتفاع في صفحة المحادثة (للترقيات `ShieldedOnly`) أو عند استخدام `effective_height`، أو تحديث وقت التشغيل `AssetConfidentialPolicy`، قم بتحديث البيانات الوصفية `zk.policy` ويتحرك إلى الداخل. إذا تم توفير شفافية دائمة عند إجراء تحويل `ShieldedOnly`، أو إيقاف وقت التشغيل للتغيير وتسجيل إخطار، والحفاظ على الوضع السابق.
- توفر مقابض التكوين `policy_transition_delay_blocks` و`policy_transition_window_blocks` الحد الأدنى من فترات التسامح للسماح بإجراء تحويلات المحفظة في حالة حدوث تغيير.
- `pending_transition.transition_id` يعمل أيضًا كمقبض للسماعة؛ يجب أن تشير الإدارة إلى إنهاء أو إلغاء التحويلات للمشغلين المرتبطين بعلاقات التشغيل/الخروج.
- `policy_transition_window_blocks` الافتراضي هو 720 (حوالي 12 ساعة من وقت الكتلة لمدة 60 ثانية). تطلب العقد المحدودة الحوكمة التي يجب نصحها بشكل أفضل.
- سفر التكوين يظهر e Fluxos CLI expoem politicas atuais e pendentes. منطق القبول السياسي في زمن التنفيذ للتأكد من أن كل تعليم سري معتمد.
- قائمة التحقق من الهجرة - إصدار "Migration sequencing" متضمنًا خطة الترقية في المراحل التي يرافقها Milestone M0.

#### مراقبة الترانزيكو عبر Torii

تستشير المحافظ والمراجعين `GET /v1/confidential/assets/{definition_id}/transitions` للتحقق من `AssetConfidentialPolicy`. تشتمل حمولة JSON دائمًا على معرف الأصل الكنسي، وآخر كتلة ملحوظة، أو `current_mode` من السياسة، أو أسلوب فعال ليس مرتفعًا (تقارير المحادثة المؤقتة `Convertible`)، والمعرفات المنتظرة `vk_set_hash`/بوسيدون/بيدرسن. عندما تكون هناك عملية انتقال للحوكمة تنتظر الرد أيضًا:

- `transition_id` - مقبض الاستماع المرجع إلى `ScheduleConfidentialPolicyTransition`.
-`previous_mode`/`new_mode`.
-`effective_height`.
- `conversion_window` و `window_open_height` مشتق (أو يتم إنشاء كتلة واحدة من المحافظ من خلال إجراء محادثة للقطع ShieldedOnly).

مثال على الرد:

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

يشير الرد `404` إلى عدم وجود تعريف للأصول المقابلة. عندما لا يكون هناك جدول أعمال أو مجال `pending_transition` و`null`.

### آلة الوضع السياسي| الوضع الحقيقي | الوضع القريب | المتطلبات الأساسية | علاج الارتفاع الفعال | نوتاس |
|--------------------|------------------|-------------------------------------------------------------------|
| شفاف فقط | للتحويل | تتيح الإدارة إدخالات سجل التحقق/المعلمات. مقياس فرعي `ScheduleConfidentialPolicyTransition` com `effective_height >= current_height + policy_transition_delay_blocks`. | يتم تنفيذ النقل بدقة في `effective_height`؛ يمكن توفير خدمة حمام السباحة المحمي على الفور.               | Caminho default para habilitar Confidencialidade Mantendo Fluxos شفافة.          |
| شفاف فقط | محمية فقط | هذا هو الحال، ولكن `policy_transition_window_blocks >= 1`.                                                         | وقت التشغيل يدخل تلقائيًا إلى `Convertible` في `effective_height - policy_transition_window_blocks`؛ مودا ل `ShieldedOnly` في `effective_height`. | قم بإعداد محادثة محددة مسبقًا من خلال تعليمات شفافة.   |
| للتحويل | محمية فقط | برنامج Transicao com `effective_height >= current_height + policy_transition_delay_blocks`. شهادة الحوكمة DEVE (`transparent_supply == 0`) عبر البيانات الوصفية للسمع؛ تطبيق وقت التشغيل لا يوجد أي انقطاع. | Semantica de janela identica. إذا قمت بتوريد شفاف لـ Nao-Zero في `effective_height`، فسيتم إيقاف النقل مع `PolicyTransitionPrerequisiteFailed`. | العمل أو الأصول المتداولة سرية تمامًا.                                      |
| محمية فقط | للتحويل | برنامج Transicao؛ SEM الانسحاب الطارئ (`withdraw_height` غير محدد).                              | الوضع الحالي هو `effective_height`; تكشف عن المنحدرات reabrem enquanto Notes blindadas permanecem validas.             | يتم استخدامه لتحديثات الصيانة أو مراجعات المراجعين.                                |
| محمية فقط | شفاف فقط | يجب على الحوكمة اختبار `shielded_supply == 0` أو إعداد خطة `EmergencyUnshield` (انضمامات المدقق المطلوبة). | وقت التشغيل قبل بداية `Convertible` قبل `effective_height`؛ على ارتفاع، يتم إرجاع التعليمات السرية بشكل صارم وإعادة الأصول إلى الوضع الشفاف فقط. | سعيدة بالعودة الأخيرة. يتم إلغاء التحويل تلقائيًا إذا كانت أي ملاحظة سرية لغاستا خلال جانيلا. |
| أي | نفس الحالي | `CancelConfidentialPolicyTransition` يتأرجح في وضع معلق.                                                    | تمت إزالة `pending_transition` على الفور.                                                                        | Mantem o الوضع الراهن؛ الأكثر اكتمالا.                                             |

Transicos no listadas acima sao rejeitadas durante submissao de الحكم. تحقق من المتطلبات الأساسية لوقت التشغيل قبل تطبيق أي برنامج نقل؛ يتم نقل الأصل إلى الوضع السابق وإصدار `PolicyTransitionPrerequisiteFailed` عبر القياس عن بعد وأحداث الكتلة.

### تسلسل الهجرة

1. **إعداد السجلات:** قم بتوفير جميع مدخلات التحقق والمعلمات المرجعية ذات الصلة بالسياسة. تعلن العقد عن نتيجة `conf_features` حتى يتمكن أقرانها من التحقق من الترابط.
2. **جدول النقل:** مقياس فرعي `ScheduleConfidentialPolicyTransition` com um `effective_height` que respeite `policy_transition_delay_blocks`. للتحريك لـ `ShieldedOnly`، حدد صفحة المحادثة (`window >= policy_transition_window_blocks`).
3. ** الدليل العام للمشغلين: ** المسجل أو `transition_id` المعاد تدويره ودفتر التشغيل الدائري للتشغيل/الإيقاف. تستخدم المحافظ والمراجع `/v1/confidential/assets/{id}/transitions` لفتح فتحة النافذة.
4. **تطبيق الملف:** عندما يتم فتح الملف، يتغير وقت التشغيل إلى السياسة لـ `Convertible`، ويصدر `PolicyTransitionWindowOpened { transition_id }`، ويبدأ في تلقي طلبات الحكم المتعارضة.
5. **الإنتهاء أو الإيقاف:** في `effective_height`، أو وقت التشغيل للتحقق من المتطلبات الأساسية (إمدادات شفافة صفر، بدون إيقاف الطوارئ، وما إلى ذلك). النجاح في السياسة من أجل الطريقة المطلوبة؛ ينبعث `PolicyTransitionPrerequisiteFailed`، ويتوقف عن الحركة ويفك السياسة دون تغيير.
6. **ترقيات المخطط:** بعد الانتقال إلى المسار الصحيح، تعمل الإدارة على تعزيز عكس مخطط الأصل (على سبيل المثال، `asset_definition.v2`) وأدوات CLI مثل `confidential_policy` إلى بيانات التسلسل. تعمل مستندات ترقية Genesis لمشغلي الأدوات على إضافة الإعدادات السياسية وبصمات التسجيل قبل إعادة إنشاء المدققين.

Redes novas que iniciam with confidencialidade habilita characteration a politica Desejada directamente en Genesis. نحن هنا نتبع قائمة التحقق هنا عندما نتيح لك وضعيات ما بعد الإطلاق لتتمكن من إنشاء محادثة في نفس الوقت المحدد والمحفظة من خلال ضبط الوقت.

### الإصدار والتفعيل للبيان Norito

- يظهر Genesis DEVEM متضمنًا `SetParameter` لمفتاح مخصص `confidential_registry_root`. الحمولة الصافية Norito JSON التي تتوافق مع `ConfidentialRegistryMeta { vk_set_hash: Option<String> }`: حذف النطاق (`null`) عند عدم إدخال أي شيء، أو استخدام سلسلة سداسية عشرية من 32 بايت (`0x...`) مماثلة للتجزئة المنتجة وفقًا لـ `compute_vk_set_hash`، لم يتم إرسال أي بيان بشأن تعليمات التحقق. تبدأ العقد في حالة فشل المعلمة أو اختلاف التجزئة عن كتابات التسجيل المشفرة.
- يتضمن `ConfidentialFeatureDigest::conf_rules_version` الموجود على السلك نموذجًا واضحًا للتخطيط. لإعادة الإصدار 1 من DEVE الدائم `Some(1)` ويساوي `iroha_config::parameters::defaults::confidential::RULES_VERSION`. عندما تتطور مجموعة القواعد، وتزيدها بشكل ثابت، وتجديد البيانات، وتنفيذ طرح الثنائيات في خطوة مقفلة؛ Misturar versoes faz validadores rejeitarem blocos com `ConfidentialFeatureDigestMismatch`.
- يظهر التنشيط DEVEM Agrupar Updates de Register، و Mudancas de Ciclo de Vida de Parametros، و Transicos de Politica Para Manter o ملخص متسق:
  1. قم بتطبيق تغييرات سجل المخططات (`Publish*`، `Set*Lifecycle`) في وضع عدم الاتصال وحساب ملخص الوضع مع `compute_confidential_feature_digest`.
  2. قم بإصدار `SetParameter::custom(confidential_registry_root, {"vk_set_hash": "0x..."})` باستخدام التجزئة المحسوبة حتى يتمكن أقرانك المختطفون من استعادة أو استيعاب المعلومات الصحيحة في نفس الوقت من خلال السماح للوسطاء بالتعليمات.
  3. تعليمات Anexar `ScheduleConfidentialPolicyTransition`. كل تعليمات يجب أن يتم إصدارها من خلال `transition_id` من أجل الحوكمة؛ يظهر أنه سيتم تجديده بشكل كبير في وقت التشغيل.
  4. استمر في إظهار وحدات البايت، باستخدام بصمة الإصبع SHA-256 واستخدام الملخص بشكل غير مخطط له. يتحقق المشغلون من ثلاث مصنوعات قبل التصويت أو البيان لتجنب المشاركة.
- عندما تطلب عمليات الطرح قطعًا مختلفًا، قم بتسجيل ارتفاع عالٍ في معلمة مخصصة مصاحبة (على سبيل المثال `custom.confidential_upgrade_activation_height`). لقد أدى هذا إلى قيام المراجعين بتوثيق اختبار في Norito حيث قام المدققون بإدراج سجل التحذير المسبق قبل الدخول في التأثير.

## سلسلة من أدوات التحقق والمعلمات
### سجل ZK
- مخزن دفتر الأستاذ `ZkVerifierEntry { vk_id, circuit_id, version, proving_system, curve, public_inputs_schema_hash, vk_hash, vk_len, max_proof_bytes, gas_schedule_id, activation_height, deprecation_height, withdraw_height, status, metadata_uri_cid, vk_bytes_cid }` الموجود على `proving_system` يتم إصلاحه حاليًا في `Halo2`.
- باريس `(circuit_id, version)` sao globalmente unicos؛ o قم بالتسجيل في مؤشر ثانوي للبحث عن البيانات الوصفية للدائرة. تجارب التسجيل مكررة خلال فترة القبول.
- `circuit_id` يجب أن يكون نافذًا و`public_inputs_schema_hash` يجب أن يكون مفعلًا (عادةً ما يتم استخدام تجزئة Blake2b-32 لترميز المدخلات العامة للتحقق). القبول مسجل في السجلات التي تحذف هذه المجالات.
- تتضمن تعليمات الحوكمة ما يلي:
  - `PUBLISH` لإضافة مدخلة `Proposed` إلى البيانات الوصفية.
  - `ACTIVATE { vk_id, activation_height }` للبرمجة في وقت محدد.
  - `DEPRECATE { vk_id, deprecation_height }` لتمييز البراهين النهائية المرتفعة يمكن الرجوع إليها.
  - `WITHDRAW { vk_id, withdraw_height }` لفصل الطوارئ؛ الأصول afetados congelam Gastos confidenciais apos سحب الارتفاع أكل novas entradas ativarem.
- يُظهر Genesis إصدارًا تلقائيًا لمعلمة مخصصة `confidential_registry_root` cujo `vk_set_hash` تتزامن مع com كـ entradas ativas؛ رحلة التحقق من الصحة هي ملخص مع الحالة المحلية التي تقوم بالتسجيل فيها قبل أن تتمكن العقدة من الدخول بدون توافق.
- المسجل أو التحقق من طلب `gas_schedule_id`؛ للتحقق من أن إدخال التسجيل هو `Active`، لا يوجد مؤشر `(circuit_id, version)`، ويثبت Halo2 من `OpenVerifyEnvelope` من `circuit_id`، `vk_hash`، e `public_inputs_schema_hash` يتوافق مع التسجيل.

### إثبات المفاتيح
- إثبات المفاتيح موجود خارج دفتر الأستاذ كمراجع للمعرفات ذات المحتوى المعنون (`pk_cid`، `pk_hash`، `pk_len`) المنشورة بعد التحقق من البيانات الوصفية.
- تقوم محفظة SDK بمسح بيانات PK والتحقق من التجزئة وذاكرة التخزين المؤقت المحلية.### باراميتروس بيدرسن وبوسيدون
- السجلات المنفصلة (`PedersenParams`، `PoseidonParams`) تتضمن ضوابط دورة حياة أدوات التحقق، كل منها على شكل `params_id`، تجزئة المولدات/الثوابت، التنشيط، الإيقاف، وسحب الارتفاعات.
- الالتزامات والتجزئة منفصلة عن النطاقات حسب `params_id` حتى لا يؤدي تدوير المعلمات إلى إعادة استخدام وحدات البت من المجموعات المهملة؛ o المعرّف والمضمّن في التزامات الملاحظات وعلامات سلطة الإبطال.
- دوائر تدعم اختيار متعدد المعلمات في وقت التحقق؛ مجموعات المعلمات المهملة يمكن إنفاقها دائمًا على `deprecation_height`، ويتم حذف المجموعات من جديد تمامًا في `withdraw_height`.

## تحديد العناصر وإبطالها
- Cada الأصول mantem um `CommitmentTree` com `next_leaf_index`؛ الالتزامات المتزايدة للكتل حسب الترتيب المحدد: تكرار المعاملات حسب ترتيب الكتلة؛ في كل عملية تحويل، تكون المخرجات محمية بواسطة `output_idx` التسلسلي الصاعد.
- `note_position` e derivado dos offsets da arvore mas **nao** faz Parte do nullifier; إنها مسارات العضوية التي تشهد الإثبات.
- استقرار إبطال إعادة التنظيم وضمان تصميم PRF؛ o إدخال PRF Vincula `{ nk, note_preimage_hash, asset_id, chain_id, params_id }`، والمثبتات المرجعية لجذور Merkle التاريخية المحدودة حسب `max_anchor_age_blocks`.

## فلوكسو دو ليدجر
1. **MintConfidential { معرف الأصول، المبلغ، المستلم_تلميح }**
   - طلب سياسة الأصول `Convertible` أو `ShieldedOnly`؛ قبول التحقق التلقائي من الأصول، استرداد `params_id` atual، amostra `rho`، التزام الالتزام، atualiza a arvore Merkle.
   - قم بإصدار `ConfidentialEvent::Shielded` مع الالتزام الجديد ودلتا جذر Merkle وتجزئة الاتصال من خلال مسارات تدقيق الحسابات.
2. **TransferConfidential { معرف الأصول، الإثبات، معرف_الدائرة، الإصدار، الإبطال، التزامات_جديدة، enc_payloads، مرساة_جذر، مذكرة }**
   - إثبات التحقق من Syscall VM باستخدام إدخال التسجيل؛ يضمن المضيف إبطال أي التزامات غير مستخدمة، والتزامات غير محددة ومثبتة مؤخرًا.
   - يسجل دفتر الأستاذ الإدخالات `NullifierSet`، ويخزن الحمولات النافعة المشفرة للمستلمين/المراجعين ويصدر `ConfidentialEvent::Transferred` لاسترجاع الإبطال، والمخرجات المطلوبة، وتجزئة الإثبات، وجذور Merkle.
3. **RevealConfidential { معرف الأصول، الإثبات، معرف_الدائرة، الإصدار، المبطل، المبلغ، حساب_المستلم، جذر_المرساة }**
   - توفير الموارد اللازمة للأصول `Convertible`؛ دليل على أن قيمة الملاحظة موجودة أو تم الكشف عنها، أو رصيد رصيد دفتر الأستاذ شفاف، كما أن المذكرة محمية بماركاندو أو مُبطلة مثل الغازتو.
   - قم بإصدار `ConfidentialEvent::Unshielded` مع مجموعة عامة ومبطلات مستهلكة ومعرفات إثبات وتجزئة دعوة المعاملات.

## نموذج بيانات Adicoes ao
- `ConfidentialConfig` (مجدد التكوين) مع علامة التأهيل، `assume_valid`، مقابض الغاز/الحدود، قفل المرساة، أداة التحقق الخلفية.
- مخططات `ConfidentialNote` و`ConfidentialTransfer` و`ConfidentialMint` Norito مع بايت النسخ الواضح (`CONFIDENTIAL_ASSET_V1 = 0x01`).
- يتضمن `ConfidentialEncryptedPayload` بايتات مذكرة AEAD مع `{ version, ephemeral_pubkey, nonce, ciphertext }`، مع `version = CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1` الافتراضي للتخطيط XChaCha20-Poly1305.
- المتجهات الأساسية لاشتقاق المفاتيح الحية في `docs/source/confidential_key_vectors.json`؛ tanto o CLI من حيث نقطة النهاية Torii تتراجع عن التركيبات المضادة.
- `asset::AssetDefinition` غانها `confidential_policy: AssetConfidentialPolicy { mode, vk_set_hash, poseidon_params_id, pedersen_params_id, pending_transition }`.
- `ZkAssetState` مستمر أو ملزم `(backend, name, commitment)` للتحقق من النقل/إلغاء الحماية؛ يتم تنفيذ الأدلة من خلال التحقق من المرجع الرئيسي أو المضمنة التي لا تتوافق مع سجل الالتزام.
- `CommitmentTree` (لنقاط التفتيش الحدودية للأصول)، `NullifierSet` مع `(chain_id, asset_id, nullifier)`، `ZkVerifierEntry`، `PedersenParams`، `PoseidonParams` مزروعة في الدولة العالمية.
- إعدادات الذاكرة المؤقتة `NullifierIndex` و`AnchorIndex` للكشف المبكر عن النسخ المكررة والتحقق من حالة التثبيت.
- تتضمن تحديثات المخطط Norito طلب Canonico للمدخلات العامة؛ اختبارات ذهابًا وإيابًا تضمن تحديد التشفير.
- رحلات ذهاب وإياب للحمولة النافعة المشفرة التي تم إصلاحها عبر اختبارات الوحدة (`crates/iroha_data_model/src/confidential.rs`). ستساعدك ناقلات محفظة المرافقة على إضافة نصوص AED Canonicos للمراجعين. `norito.md` مستند أو رأس متصل بالسلك للمظروف.

## التكامل IVM واتصال النظام
- إدخال اتصال النظام `VERIFY_CONFIDENTIAL_PROOF`:
  - `circuit_id`، `version`، `scheme`، `public_inputs`، `proof`، و`ConfidentialStateDelta { asset_id, nullifiers, commitments, enc_payloads }` الناتج.
  - قم بإجراء استدعاء البيانات التعريفية للتحقق من التسجيل، وتطبيق حدود الحجم/الإيقاع، وتحديد غاز الكوبرا، وبالتالي فإن التطبيق أو الدلتا يثبت نجاحه.
- يعرض المضيف سمة `ConfidentialLedger` للقراءة فقط لاستعادة لقطات جذر Merkle وحالة الإبطال؛ توفر المكتبة Kotodama مساعدات لتجميع الشاهد والتحقق من المخطط.
- تم تحديث مستندات المؤشر - منتدى ABI لإلغاء تخطيط المخزن المؤقت للتدقيق ومقابض التسجيل.

## تقليص قدرات العقدة
- إعلان المصافحة `feature_bits.confidential` junto com `ConfidentialFeatureDigest { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`. يطلب المشاركون من المدققين `confidential.enabled=true`، `assume_valid=false`، ومعرفات الواجهة الخلفية للتحقق من المتطابقات والخلاصات التي تتوافق معها؛ عدم التطابق falham أو المصافحة com `HandshakeConfidentialMismatch`.
- التكوين يدعم `assume_valid` فقط للمراقبين: عندما تكون غير قادر على العثور على تعليمات سرية من `UnsupportedInstruction` تحدد دون الذعر؛ عندما يكون المراقبون مؤهلين، يطبقون الدلتا المعلنة دون إثباتات.
- يحتفظ Mempool بالمعاملات السرية إذا كانت القدرة المحلية غير قابلة للتعطيل. تمنع مرشحات القيل والقال إرسال المعاملات المحمية للأقران غير المتوافقين أثناء قيامهم بجمع معرفات متصلة للتحقق من عدم اليقين داخل حدود الحجم.

### ماتريز دي المصافحة

| الإعلان عن بعد | النتيجة لعقد التحقق من الصحة | ملاحظات المشغل |
|----------------------|----------------------------|----------------|
| `enabled=true`، `assume_valid=false`، مطابقة الواجهة الخلفية، مطابقة الملخص | أسيتو | النظير الذي شاهد الحالة `Ready` وشارك في العرض والتصويت وRBC المنتشر. مطلوب دليل Nenhuma Acao. |
| `enabled=true`، `assume_valid=false`، مطابقة الواجهة الخلفية، ملخص قديم أو مباشر | ريجيتادو (`HandshakeConfidentialMismatch`) | يجب عليك عن بعد تطبيق وظائف التسجيل/المعلمات المعلقة أو حماية برنامج `activation_height`. قم بالتصحيح, أو قم باكتشاف العقدة التالية بعد ذلك حتى داخل توافق الآراء. |
| `enabled=true`، `assume_valid=true` | ريجيتادو (`HandshakeConfidentialMismatch`) | تتطلب عمليات التحقق من صحة الأدلة؛ قم بتكوين جهاز التحكم عن بعد كمراقب com Torii فقط للدخول أو وضع `assume_valid=false` حتى تتمكن من التحقق بالكامل. |
| `enabled=false`، المجالات المحذوفة (الإنشاء غير المكتمل)، أو الواجهة الخلفية للتحقق المختلفة | ريجيتادو (`HandshakeConfidentialMismatch`) | يمكن للأقران غير المكتملين أو المحدثين جزئيًا الدخول إلى منطقة التوافق. قم بتنشيط الإصدار التلقائي وتأكد من أن الواجهة الخلفية مصفوفة + ملخص مطابق قبل إعادة الاتصال. |

لا ينبغي للمراقبين الذين يعتزمون التحقق من البراهين أن يفتحوا اتصالات توافقية تتعارض مع المصدقين مع بوابات القدرة. يمكنها دمج الكتل عبر Torii أو واجهات برمجة التطبيقات (APIs) للملفات، ولكن بعد الحصول على موافقة عليها يتم الإعلان عن القدرات المتوافقة.

### سياسة تقليم الكشف والاحتفاظ بالإبطال

يجب أن ترجع دفاتر الأستاذ السرية تاريخيًا بدرجة كافية لإثبات تدوين الملاحظات وإعادة إنتاج محاضر الحوكمة. افتراضي سياسي، تم تطبيقه بواسطة `ConfidentialLedger`، e:

- **الإبطال:** زيادة إبطال الغازات لمدة *الحد الأدنى* لـ `730` لمدة (24 شهرًا) بعد زيادة المعدة، أو فرض قيود تنظيمية أكبر. يمكن للمشغلين إرسال رسالة عبر `confidential.retention.nullifier_days`. يتم إبطال المزيد من الإضافات الجديدة التي يتم استشارة DEVEM بها دائمًا عبر Torii حتى يتمكن المدققون من إثبات إمكانية الإنفاق المزدوج.
- **تقليم الكشف:** يكشف الشفافية (`RevealConfidential`) عن الالتزامات المرتبطة فورًا بعد الانتهاء من الكتلة، ولكن يستمر إبطال الاستهلاك في الالتزام بفترة الاحتفاظ الحالية. تقوم الأحداث `ConfidentialEvent::Unshielded` بتسجيل مجموعة عامة ومستلمين وتجزئة الإثبات لإعادة بناء الكشف عن التاريخ دون وجود نص مشفر.
- **نقاط التفتيش الحدودية:** نقاط تفتيش حدود الالتزام التي تدور حول الكوبريندو أو الأكبر بين `max_anchor_age_blocks` وجانب الحفظ. العقد تضغط نقاط التفتيش القديمة قبل أن تنتهي جميع الإبطالات بفاصل زمني.
- **إصلاح الهضم القديم:** إذا حدث `HandshakeConfidentialMismatch` من أجل انحراف الهضم، يجب على المشغلين (1) التحقق من أن سجلات الاحتفاظ بالإبطال موجودة في كتلة، (2) قضيب `iroha_cli app confidential verify-ledger` لتجديد أو هضم ضد مجموعة الإبطال المستردة، e (3) إعادة النشر أو تحديث البيان. يمكن أن تكون الإبطالات سابقة لأوانها بمثابة مطاعم للتخزين البارد قبل العودة إلى الوراء.

تجاوز المستند المكاني ليس دليل التشغيل؛ يجب على سياسة الحوكمة التي تعتمد على قائمة الاحتفاظ أن يتم تحديث تكوين العقدة وخطط تخزين الأرشيف بشكل كامل.

### تدفق الإخلاء والاسترداد1. خلال الاتصال الهاتفي، `IrohaNetwork` مقارنة بالقدرات المعلنة. Qualquer غير متطابق levanta `HandshakeConfidentialMismatch`; يتم إجراء الاتصال والمتابعة والمزامنة الدائمة في ملف الاكتشاف دون ترقية إلى `Ready`.
2. لا يظهر سجل خدمة الخدمة (بما في ذلك الملخص عن بعد والواجهة الخلفية)، ولا يوجد Sumeragi جدول أعمال أو نظير للاقتراح أو التصويت.
3. يقوم المشغلون بإصلاح سجلات التحقق ومجموعات المعلمات (`vk_set_hash`، `pedersen_params_id`، `poseidon_params_id`) أو برمجة `next_conf_features` من خلال `activation_height`. بمجرد أن يتزامن الهضم، أو المصافحة التالية تنجح تلقائيًا.
4. إذا كان أحد الأقران القديم يقوم بتمويل كتلة (على سبيل المثال، عبر إعادة تشغيل الملف)، يتم التحقق من صحتها أو إعادة تحديدها بواسطة `BlockRejectionReason::ConfidentialFeatureDigestMismatch`، والحفاظ على حالة دفتر الأستاذ بشكل متسق على الإنترنت.

### تدفق المصافحة بشكل آمن ضد الإعادة

1. كل مادة عازلة للخارج من الضوضاء/X25519 جديدة. يتم ربط حمولة المصافحة (`handshake_signature_payload`) كأعمدة عامة مؤقتة محلية وعن بعد، أو يتم إغلاق مأخذ التوصيل المشفر في Norito، عند تجميعها مع `handshake_chain_id`، أو معرف السلسلة. رسالة مشفرة بواسطة AEAD قبل البحث عن العقدة.
2. يقوم المستجيب بإعادة حساب الحمولة حسب ترتيب عكس النظير/المحلي والتحقق من اغتيال Ed25519 المضمن في `HandshakeHelloV1`. مثل السفراء كأشخاص صغار وإعلان عن جزء من سلطة الاغتيال، إعادة تشغيل رسالة تم التقاطها ضد أقران آخرين أو استعادة اتصال قديم بشكل حاسم.
3. علامات السعة السرية e o `ConfidentialFeatureDigest` عبر `HandshakeConfidentialMeta`. يقارن المستقبل بين `{ enabled, assume_valid, verifier_backend, digest }` و`ConfidentialHandshakeCaps` المحلي؛ أي عدم تطابق هو ما يعنيه `HandshakeConfidentialMismatch` قبل النقل العابر لـ `Ready`.
4. يقوم مشغلو DEVEM بإعادة حساب الملخص (عبر `compute_confidential_feature_digest`) وإعادة تشغيل العقد مع السجلات/السياسات التي تم تحديثها قبل إعادة الاتصال. يعلن الأقران عن خلاصات قديمة مستمرة من خلال المصافحة أو المصافحة، لتجنب أن تكون الحالة قديمة مرة أخرى دون تحديد أداة التحقق.
5. النجاح والنجاح في تحسين مصافحة المصافحة `iroha_p2p::peer` (`handshake_failure_count`، مساعدي تصنيف الأخطاء) وإصدار السجلات المُنشأة بواسطة معرف النظير عن بعد وبصمة الإصبع. قم بمراقبة المؤشرات لاكتشاف عمليات إعادة التشغيل أو التكوينات الخاطئة أثناء بدء التشغيل.

## إدارة المفاتيح والحمولات
- Hierarquia de derivacao por account:
  - `sk_spend` -> `nk` (مفتاح الإلغاء)، `ivk` (مفتاح العرض الوارد)، `ovk` (مفتاح العرض الصادر)، `fvk`.
- حمولات الملاحظات المشفرة باستخدام AEAD com المفاتيح المشتركة المشتقة من ECDH؛ عرض مفاتيح خيارات المدقق التي يمكن أن تضيف مخرجات تتوافق مع الأصول السياسية.
- دعم لـ CLI: `confidential create-keys`، `confidential send`، `confidential export-view-key`، أدوات التدقيق لوصف المذكرات، والمساعد `iroha app zk envelope` للمنتج/فحص المغلفات Norito دون الاتصال بالإنترنت. يعرض Torii نفس تدفق الاشتقاق عبر `POST /v1/confidential/derive-keyset`، ويعيد الأشكال السداسية وbase64 لتتمكن المحافظ من البحث عن التسلسل الهرمي للمفتاح برمجيًا.

## الغاز والحدود والضوابط DoS
- جدول تحديد الغاز:
  - Halo2 (بلونكيش): غاز أساسي `250_000` + غاز `2_000` للمدخلات العامة.
  - `5` بايت مقاوم للغاز، المزيد من الشحنات للإبطال (`300`) والالتزام (`500`).
  - مشغلي podem sobrescrever essas Constantes عبر configuracao do العقدة (`confidential.gas.{proof_base, per_public_input, per_proof_byte, per_nullifier, per_commitment}`)؛ لا يتم نشر النشرات عند بدء التشغيل أو إعادة التحميل السريع من خلال شريط التكوين والتطبيقات المحددة بدون كتلة.
- حدود التحمل (الإعدادات الافتراضية):
-`max_proof_size_bytes = 262_144`.
- `max_nullifiers_per_tx = 8`، `max_commitments_per_tx = 8`، `max_confidential_ops_per_block = 256`.
- `verify_timeout_ms = 750`، `max_anchor_age_blocks = 10_000`. تؤدي الأدلة التي تتجاوز `verify_timeout_ms` إلى إحباط التعليمات الحتمية (بطاقة الاقتراع الخاصة بالحكم، البند `proof verification exceeded timeout`، `VerifyProof` ترجع الخطأ).
- حصص إضافية تضمن الحيوية: `max_proof_bytes_block`، `max_verify_calls_per_tx`، `max_verify_calls_per_block`، e `max_public_inputs` منشئو الكتل المحدودة؛ `reorg_depth_bound` (>= `max_anchor_age_blocks`) يحكم نقاط التفتيش الحدودية.
- يؤدي تنفيذ وقت التشغيل الآن إلى إعادة المعاملات التي تتجاوز حدود المعاملات أو الكتلة، مما يؤدي إلى حدوث أخطاء `InvalidParameter` في المحددات والحفاظ على حالة دفتر الأستاذ غير المعدلة.
- تقوم الذاكرة بتصفية المعاملات السرية مسبقًا بواسطة `vk_id`، وإثبات الإثبات ووظيفة التثبيت قبل استدعاء أو التحقق للحفاظ على استخدام الموارد المحدودة.
- التحقق من تحديد المهلة أو انتهاك الحد؛ Transacoes falham com erros الصريحة. تعد الواجهات الخلفية SIMD اختيارية ولكنها لا تغير محاسبة الغاز.

### خطوط الأساس للمعايرة وبوابات الاستخلاص
- **المنصات المرجعية.** أدوات المعايرة DEVEM تقوم بجمع ثلاثة من الكماليات. Rodadas sem todos os perfis sao rejeitadas na review.

  | الملف الشخصي | معمارية | وحدة المعالجة المركزية / مثيل | أعلام المترجم | اقتراح |
  | --- | --- | --- | --- | --- |
  | `baseline-simd-neutral` | `x86_64` | AMD EPYC 7B12 (32c) أو Intel Xeon Gold 6430 (24c) | `RUSTFLAGS="-C target-feature=-avx,-avx2,-fma"` | Estabelece valores piso sem intrinsics vetoriais؛ يستخدم لضبط اللوحات الاحتياطية المخصصة. |
  | `baseline-avx2` | `x86_64` | إنتل زيون جولد 6430 (24 سي) | الافراج الافتراضي | التحقق من مسار AVX2؛ تأكد من أن نظام التشغيل SIMD موجود داخل التسامح مع الغاز المحايد. |
  | `baseline-neon` | `aarch64` | AWS Graviton3 (c7g.4xlarge) | الافراج الافتراضي | ضمان أن الواجهة الخلفية NEON ستحدد دائمًا وتحافظ على جداول x86. |

- **الحزام المعياري.** جميع علاقات معايرة الغاز DEVEM يتم إنتاجها عبر:
  -`CRITERION_HOME=target/criterion cargo bench -p iroha_core isi_gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>`
  - `cargo test -p iroha_core bench_repro -- --ignored` لتأكيد تحديد التركيب.
  - `CRITERION_HOME=target/criterion cargo bench -p ivm gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>` يستمر في تشغيل مخصص كود التشغيل VM.

- **إصلاح العشوائية.** تصدير `IROHA_CONF_GAS_SEED=conf-gas-seed-2026Q1` قبل وضع مقاعد `iroha_test_samples::gen_account_in` لتحديد المسار `KeyPair::from_seed`. يا تسخير imprime `IROHA_CONF_GAS_SEED_ACTIVE=...` uma vez; إذا كان هناك variavel faltar، قم بمراجعة DEVE falhar. أي استخدام جديد للمعايرة يجب أن يستمر، مما يعني تقديم العشوائية المساعدة.

- **التقاط النتائج.**
  - تحميل معيار الملخصات (`target/criterion/**/raw.csv`) لكل ملف بدون إصدار مصطنع.
  - تخزين المقاييس المشتقة (`ns/op`، `gas/op`، `ns/gas`) في [دفتر الأستاذ السري لمعايرة الغاز](./confidential-gas-calibration) جنبًا إلى جنب مع الالتزام بالبوابة والعكس من استخدام المترجم.
  - مانتر أوس دويس آخر خطوط الأساس من أجل الملف الشخصي؛ قم بمسح اللقطات القديمة مرة أخرى للتحقق من صحتها أو ربطها مرة أخرى.

- **التسامح في التناول.**
  - دلتا الغاز بين `baseline-simd-neutral` و`baseline-avx2` DEVEM الدائم <= +/-1.5%.
  - دلتا الغاز بين `baseline-simd-neutral` و`baseline-neon` DEVEM الدائم <= +/-2.0%.
  - تتطلب اقتراحات المعايرة التي تتجاوز هذه الحدود إجراء تعديلات على الجدول الزمني أو توضيح RFC للتناقض والتخفيف منه.

- **قائمة مراجعة المراجعة.** يقدم المرسلون إجاباتهم من أجل:
  - يتضمن `uname -a`، و`/proc/cpuinfo` (النموذج، والخطوة)، و`rustc -Vv` بدون سجل معايرة.
  - التحقق مما إذا كان `IROHA_CONF_GAS_SEED` يظهر على المقعد (حيث تقوم المقاعد بوضع البذور).
  - ضمان وجود أعلام لجهاز تنظيم ضربات القلب والتحقق من المنتجات السرية (`--features confidential,telemetry` على مقاعد القيادة مع القياس عن بعد).

## تكوين العمليات
- `iroha_config` إضافة إلى `[confidential]`:
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
- القياس عن بعد يصدر مقاييس متفق عليها: `confidential_proof_verified`، `confidential_verifier_latency_ms`، `confidential_proof_bytes_total`، `confidential_nullifier_spent`، `confidential_commitments_appended`، `confidential_mempool_rejected_total{reason}`، e `confidential_policy_transitions_total`، se تصدير دادوس إم كلارو.
- السطوح RPC:
  -`GET /confidential/capabilities`
  -`GET /confidential/zk_registry`
  -`GET /confidential/params`

## استراتيجية الخصيتين
- الحتمية: التحويلات العشوائية داخل الكتل التي تشمل جذور Merkle ومجموعات إبطال متطابقة.
- المرونة في إعادة التنظيم: عمليات إعادة تنظيم مماثلة متعددة الكتل؛ المبطلات الدائمة والمثبتات قديمة أو متجددة.
- ثبات الغاز: التحقق من استخدام الغاز المتطابق بين العقد مثل SIMD المتسارع.
- اختبار الحدود: إثبات عدم وجود حجم/غاز، والحد الأقصى لعدد مرات الدخول/الخروج، وإنفاذ المهلة.
- دورة الحياة: عمليات الحوكمة للتنشيط/الإيقاف للتحقق والمعلمات، واختبارات الأداء عند الدوران.
- سياسة ولايات ميكرونيزيا الموحدة: عمليات النقل المسموح بها/السلبية، وتأخير النقل المعلق، واستعادة الذاكرة من خلال الارتفاعات الفعلية.
- حالات الطوارئ الخاصة بالتسجيل: يتم سحب الأصول الطارئة من خلال `withdraw_height` ويتم إثباتها بعد ذلك.
- بوابة القدرة: validadores com `conf_features` divergentes rejeitam blocos؛ المراقبون com `assume_valid=true` يرافقونهم بتوافق الآراء.
- معادلة الحالة: العقد المدقق/الكامل/المراقب تنتج جذور حالة متطابقة في القانون الكنسي.
- سلبيات غامضة: إثباتات مشوهة، حمولات زائدة الأبعاد، وانهيارات إبطالية متجددة بشكل محدد.##ميغراكاو
- علامة ميزة بدء التشغيل com: نهاية المرحلة C3، `enabled` الافتراضي هو `false`؛ تعلن العقد عن قدراتها قبل الدخول ولم يتم تعيين أداة التحقق من صحتها.
- الأصول شفافة وليس ساو أفيتادوس؛ تتطلب التعليمات السرية إدخالات التسجيل وتداول القدرات.
- العقد المجمعة لا تدعم الكتل السرية ذات الصلة المحددة؛ لا يمكن إدخال أي مجموعة من المصادقة ولكن يمكن تشغيلها كمراقبين com `assume_valid=true`.
- تشتمل بيانات Genesis على مدخلات بدء التسجيل ومجموعات المعلمات والسياسات السرية للأصول ومفاتيح المدقق الاختياري.
- يقوم المشغلون بتتبع دفاتر التشغيل المنشورة لتدوير التسجيل، والانتقالات السياسية، والانسحاب في حالات الطوارئ من أجل زيادة ترقيات المحددات.

## Trabalho pendente
- معيار معلمات Halo2 (حجم الدائرة وإستراتيجية البحث) وتسجيل النتائج في قواعد اللعبة المعايرة بحيث يتم تحديث الإعدادات الافتراضية للغاز/المهلة إلى جانب التحديث التالي لـ `confidential_assets_calibration.md`.
- وضع اللمسات النهائية على سياسة الكشف عن مدقق الحسابات وواجهات برمجة التطبيقات (APIs) الخاصة بالعرض الانتقائي للشركاء، والاتصال أو الموافقة على سير العمل في Torii أثناء مسودة الحوكمة التي سيتم تفكيكها.
- إرسال طلب تشفير شاهد لطباعة مخرجات متعددة المستلمين ومذكرات دفعة واحدة وتوثيق أو تنسيق مغلف لمنفذي SDK.
- إجراء مراجعة للتأمين الخارجي للدوائر والسجلات وإجراءات تغيير المعلمات وحفظ البيانات من خلال روابط الاستماع الداخلية.
- تخصيص واجهات برمجة التطبيقات (APIs) لتسوية الإنفاق للمدققين ونشر دليل رؤية مفتاح العرض حتى يتمكن بائعو المحفظة من تنفيذها كمجموعة من دلالات المصادقة.

## مراحل التنفيذ
1. **المرحلة M0 - تصلب إيقاف السفينة**
   - [x] تم إلغاء الإلغاء التالي لتصميم Poseidon PRF (`nk`, `rho`, `asset_id`, `chain_id`) مع ترتيب تحديد الالتزامات المطبقة على دفتر الأستاذ المحدث.
   - [x] تنفيذ حدود حجم الإثبات والحصص السرية للمعاملات/للكتلة، وإرجاع المعاملات للميزانية مع الأخطاء المحددة.
   - [x] إعلان المصافحة P2P `ConfidentialFeatureDigest` (ملخص الواجهة الخلفية + بصمات التسجيل) وعدم التطابق بشكل محدد عبر `HandshakeConfidentialMismatch`.
   - [x] إزالة الذعر في مسارات التنفيذ السرية وإضافة الأدوار للعقد غير المتوافقة.
   - [ ] تطبيق ميزانيات المهلة للتحقق وحدود عمق إعادة تنظيم نقاط التفتيش الحدودية.
     - [x] ميزانيات مهلة التحقق المطبقة؛ البراهين التي تتجاوز `verify_timeout_ms` قبل أن تكون حتمية.
     - [x] سيتم استئناف نقاط التفتيش الحدودية `reorg_depth_bound`، وستتمكن من نقاط التفتيش الأحدث التي يتم تكوينها والحفاظ على اللقطات المحددة.
   - تقديم `AssetConfidentialPolicy`، سياسة FSM وبوابات التنفيذ لتعليمات النعناع/النقل/الكشف.
   - قم بتنفيذ `conf_features` في رؤوس الكتلة واستعادة المشاركة في عمليات التحقق عندما تكون خلاصات التسجيل/المعلمات متباينة.
2. **المرحلة M1 - السجلات والمعلمات**
   - فتح السجلات `ZkVerifierEntry` و`PedersenParams` و`PoseidonParams` مع عمليات الحوكمة وتعزيز التكوين وإدارة ذاكرة التخزين المؤقت.
   - قم بتوصيل نظام الاتصال لبدء عمليات البحث عن السجل ومعرفات جدول الغاز وتجزئة المخطط وفحوصات الحجم.
   - إرسال تنسيق الحمولة المشفرة v1، واشتقاق مفاتيح المحفظة، ودعم CLI لإدارة الأشياء السرية.
3. **المرحلة M2 - أداء الغاز**
   - تنفيذ جدول تحديد الغاز، وعدادات الكتلة، وأدوات قياس القياس عن بعد (زمن الوصول للتحقق، ومستوى الإثبات، واستعادة الذاكرة).
   - نقاط تفتيش Endurecer CommitmentTree وشحن LRU ومؤشرات الإبطال لأحمال العمل متعددة الأصول.
4. **المرحلة M3 - تدوير أدوات المحفظة**
   - القدرة على الحصول على البراهين متعددة المعلمات والنسخ المتعددة؛ support ativacao/deprecacao guiada por Government com runbooks de transicao.
   - قم بتشغيل تدفقات ترحيل SDK/CLI وسير عمل فحص المدقق وأدوات تسوية الإنفاق.
5. **المرحلة م4 - تدقيق العمليات الإلكترونية**
   - سير عمل مفاتيح التدقيق، وواجهات برمجة التطبيقات (API) للكشف الانتقائي، ودفاتر التشغيل.
   - مراجعة جدول التشفير الخارجي/التأمين ونشر الخطوات على `status.md`.

كل مرحلة من مراحل تحقيق المعالم الرئيسية لخارطة الطريق والاختبارات المرتبطة بها لضمان ضمانات التنفيذ في مجال blockchain الحقيقي.
---
lang: es
direction: ltr
source: docs/portal/docs/nexus/confidential-assets.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: الاصول السرية وتحويلات ZK
descripción: مخطط Phase C للدوران blindado والسجلات وضوابط المشغلين.
slug: /nexus/activos-confidenciales
---
<!--
SPDX-License-Identifier: Apache-2.0
-->
# تصميم الاصول السرية وتحويلات ZK

## الدوافع
- تقديم تدفقات اصول blinded اختيارية حتى تتمكن الدومينات من حفظ خصوصية المعاملات دون تغيير الدوران الشفاف.
- Para obtener más información, consulte el documento Norito/Kotodama ABI v1.
- تزويد المدققين والمشغلين بضوابط دورة الحياة (تفعيل، تدوير، سحب) للدوائر والمعلمات التشفيرية.

## نموذج التهديدات
- المدققون honesto-pero-curioso: ينفذون الاجماع بامانة لكنهم يحاولون فحص libro mayor/estado.
- مراقبو الشبكة يرون بيانات الكتل والمعاملات المرسلة عبر chismes؛ لا نفترض وجود قنوات chismes خاصة.
- خارج النطاق: تحليل حركة المرور خارج الدفتر، خصوم كميون (يتابعون في PQ roadmap), y وهجمات توفر libro mayor.## نظرة عامة على التصميم
- يمكن للاصول اعلان *piscina protegida* اضافة الى الارصدة الشفافة؛ يتم تمثيل الدوران blindado عبر compromisos تشفيرية.
- Notas `(asset_id, amount, recipient_view_key, blinding, rho)` de:
  - Compromiso: `Comm = Pedersen(params_id || asset_id || amount || recipient_view_key || blinding)`.
  - Anulador: `Null = Poseidon(domain_sep || nk || rho || asset_id || chain_id)` مستقل عن ترتيب notas.
  - Carga útil cifrada: `enc_payload = AEAD_XChaCha20Poly1305(ephemeral_shared_key, note_plaintext)`.
- تنقل المعاملات cargas útiles `ConfidentialTransfer` المرمزة بنوريتو وتحتوي:
  - Aportes públicos: ancla de Merkle, anuladores, compromisos, identificación de activos, نسخة الدائرة.
  - Cargas útiles مشفرة للمستلمين والمدققين الاختياريين.
  - Prueba de conocimiento cero تؤكد حفظ القيمة والملكية والتفويض.
- يتم التحكم في verificación de claves ومجموعات المعلمات عبر سجلات على الدفتر مع نوافذ تفعيل؛ ترفض العقد التحقق من pruebas تشير الى ادخالات مجهولة او مسحوبة.
- تلتزم رؤوس الاجماع ب digest ميزات السرية النشطة كي لا تقبل الكتل الا عند تطابق حالة السجلات والمعلمات.
- بناء pruebas يستخدم Halo2 (Plonkish) بدون configuración confiable؛ Groth16 y SNARK اخرى غير مدعومة عمدا في v1.

### Calendario

اغلفة memo السرية تشحن الان مع قانوني في `fixtures/confidential/encrypted_payload_v1.json`. تلتقط مجموعة البيانات sobre v1 صحيحا مع عينات سلبية تالفة حتى تتمكن SDKs من اثبات تطابق التحليل. Modelo de datos de Rust (`crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs`) y Swift (`IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift`) para la codificación del dispositivo y la codificación Norito وتغطية الانحدار مع تطور الكودك.Para los SDK de Swift, incluye escudo y pegamento JSON. Nombre: `ShieldRequest`, nota de compromiso, 32 notas, carga útil y metadatos de débito. Utilice `IrohaSDK.submit(shield:keypair:)` (او `submitAndWait`) para conectar y desconectar `/v2/pipeline/transactions`. يقوم المساعد بالتحقق من اطوال compromisos, y يمرر `ConfidentialEncryptedPayload`, Norito codificador, y diseño `zk::Shield`, موضح ادناه حتى تبقى المحافظ متزامنة مع Rust.

## Compromisos الاجماع و Gating القدرات
- تكشف رؤوس الكتل `conf_features = { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`؛ يشارك digest في hash الاجماع ويجب ان يساوي عرض السجل المحلي لقبول الكتلة.
- مكن للحوكمة تجهيز الترقيات ببرمجة `next_conf_features` مع `activation_height` مستقبلي؛ وحتى ذلك الارتفاع يجب على منتجي الكتل الاستمرار في اصدار digest السابق.
- يجب على عقد المدققين التشغيل مع `confidential.enabled = true` e `assume_valid = false`. Para obtener más información, consulte el enlace del fabricante `conf_features`.
- Protocolo de enlace P2P basado en `{ enabled, assume_valid, conf_features }`. يتم رفض peers الذين يعلنون ميزات غير متوافقة بخطأ `HandshakeConfidentialMismatch` ولا يدخلون ابدا في دوران الاجماع.
- نتائج التوافق بين المدققين، observadores, y والـ pares القديمة تلتقط في مصفوفة handshake ضمن [Negociación de capacidad de nodo] (#node-capability-negotiation). Hay un apretón de manos en `HandshakeConfidentialMismatch` y un par está en un resumen.
- يمكن للمراقبين غير المدققين ضبط `assume_valid = true`؛ يطبقون دلتا سرية بشكل اعمى دون التأثير على سلامة الاجماع.## سياسات الاصول
- يحمل كل تعريف اصل `AssetConfidentialPolicy` يحدده المنشئ او عبر الحوكمة:
  - `TransparentOnly`: الوضع الافتراضي؛ يسمح فقط بتعليمات شفافة (`MintAsset`, `TransferAsset`, الخ) y ترفض العمليات blindados.
  - `ShieldedOnly`: يجب ان تستخدم كل الاصدارات والتحويلات تعليمات سرية؛ يحظر `RevealConfidential` حتى لا تظهر الارصدة علنا.
  - `Convertible`: يمكن للحاملين نقل القيمة بين التمثيل الشفاف y باستخدام تعليمات rampas de entrada/salida.
- تتبع السياسات FSM مقيد لمنع تعلق الاموال:
  - `TransparentOnly → Convertible` (تمكين فوري لـ piscina blindada).
  - `TransparentOnly → ShieldedOnly` (يتطلب انتقالا معلقا ونافذة تحويل).
  - `Convertible → ShieldedOnly` (تاخير ادنى الزامى).
  - `ShieldedOnly → Convertible` (يتطلب خطة هجرة لضمان بقاء notas قابلة للصرف).
  - `ShieldedOnly → TransparentOnly` غير مسموح الا اذا كان piscina blindada فارغا او قامت الحوكمة بترميز هجرة تزيل السرية عن notas المتبقية.
- تعليمات الحوكمة تضبط `pending_transition { new_mode, effective_height, previous_mode, transition_id, conversion_window }` عبر ISI `ScheduleConfidentialPolicyTransition` ويمكنها الغاء التغييرات المجدولة بـ `CancelConfidentialPolicyTransition`. يضمن تحقق mempool عدم عبور اي معاملة لارتفاع الانتقال، ويفشل الادراج بشكل حتمي اذا تغير فحص السياسة في منتصف الكتلة.
- يتم تطبيق الانتقالات المعلقة تلقائيا عند فتح كتلة جديدة: عند دخول ارتفاع الكتلة نافذة التحويل (لترقيات `ShieldedOnly`) y el nombre de `effective_height`, el tiempo de ejecución de `AssetConfidentialPolicy` y los metadatos de `zk.policy` y los de `zk.policy`. Esta es la configuración del tiempo de ejecución `ShieldedOnly`, el tiempo de ejecución y el tiempo de ejecución del programa.- Adaptador de corriente `policy_transition_delay_blocks` e `policy_transition_window_blocks`. التبديل.
- `pending_transition.transition_id` يعمل ايضا كـ control de auditoría؛ يجب على الحوكمة ذكره عند انهاء او الغاء الانتقالات حتى يتمكن المشغلون من ربط تقارير rampa de entrada/salida.
- `policy_transition_window_blocks` افتراضيه 720 (hay 12 minutos con un tiempo de bloque de 60 segundos). تحد العقد طلبات الحوكمة التي تحاول اشعارا اقصر.
- Génesis manifiesta وتدفقات CLI تعرض السياسات الحالية والمعلقة. منطق admisión يقرأ السياسة وقت التنفيذ لتاكيد ان كل تعليمة سرية مصرح بها.
- قائمة تحقق الهجرة - انظر “Secuenciación de migración” ادناه لخطة ترقية على مراحل يتبعها Milestone M0.

#### مراقبة الانتقالات عبر Torii

تستعلم المحافظ y المدققون `GET /v2/confidential/assets/{definition_id}/transitions` لفحص `AssetConfidentialPolicy` النشطة. Carga útil JSON دائما على ID de activo القانوني، اخر ارتفاع كتلة ملاحظ، `current_mode` للسياسة، الوضع الفعال عند ذلك الارتفاع (نوافذ التحويل تبلغ مؤقتا `Convertible`), y معرفات معلمات `vk_set_hash`/Poseidon/Pedersen المتوقعة. عند وجود انتقال حوكمة معلق يتضمن الرد ايضا:

- `transition_id` - identificador de auditoría المعاد من `ScheduleConfidentialPolicyTransition`.
- `previous_mode`/`new_mode`.
- `effective_height`.
- `conversion_window` e `window_open_height` المشتق (الكتلة التي يجب ان تبدأ فيها المحافظ التحويل لقطع ShieldedOnly).

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

يشير رد `404` الى عدم وجود تعريف اصل مطابق. Utilice el conector `pending_transition` para conectar el conector `null`.

### آلة حالات السياسة| الوضع الحالي | الوضع التالي | المتطلبات | التعامل مع efectividad_height | ملاحظات |
|--------------------|------------------|-------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------------------------------|-------------------------------------------------------------------------------------|
| Sólo transparente | Descapotable | فعلت الحوكمة ادخالات سجل verificador/parámetro. Utilice `ScheduleConfidentialPolicyTransition` para `effective_height ≥ current_height + policy_transition_delay_blocks`. | ينفذ الانتقال بالضبط عند `effective_height`؛ يصبح piscina protegida متاحا فوريا.                   | المسار الافتراضي لتمكين السرية مع ابقاء التدفقات الشفافة.               |
| Sólo transparente | Sólo blindado | كما سبق، بالاضافة الى `policy_transition_window_blocks ≥ 1`.                                                         | El tiempo de ejecución es `Convertible` y `effective_height - policy_transition_window_blocks`. Utilice `ShieldedOnly` y `effective_height`. | يوفر نافذة تحويل حتمية قبل تعطيل التعليمات الشفافة.   || Descapotable | Sólo blindado | Esta es la versión `effective_height ≥ current_height + policy_transition_delay_blocks`. يجب على الحوكمة توثيق (`transparent_supply == 0`) عبر metadatos de auditoría؛ ويفرض runtime ذلك عند القطع. | نفس دلالات النافذة كما اعلاه. Esta es la conexión entre `effective_height` y la conexión entre `PolicyTransitionPrerequisiteFailed`. | يقفل الاصل في دوران سري بالكامل.                                     |
| Sólo blindado | Descapotable | انتقال مجدول؛ لا يوجد سحب طارئ نشط (`withdraw_height` غير مضبوط).                                    | يتبدل الوضع عند `effective_height`؛ تعاد فتح revelan rampas بينما تبقى notas blindadas صالحة.                           | يستخدم لنوافذ الصيانة او مراجعات المدققين.                                          |
| Sólo blindado | Sólo transparente | Utilice el conector `shielded_supply == 0` y el conector `EmergencyUnshield` (desbloqueado). | Tiempo de ejecución de `Convertible` o `effective_height` عند الارتفاع تفشل التعليمات السرية بقوة ويعود الاصل الى وضع شفاف فقط. | خروج كملاذ اخير. يتم الغاء الانتقال تلقائيا اذا تم صرف اي note سرية خلال النافذة. |
| Cualquiera | Igual que la actual | `CancelConfidentialPolicyTransition` يمسح التغيير المعلق.                                                        | يتم حذف `pending_transition` فورا.                                                                          | يحافظ على الوضع الراهن؛ مذكور للاكتمال.                                             |الانتقالات غير المدرجة اعلاه ترفض عند تقديمها للحوكمة. يتحقق runtime من الشروط المسبقة قبل تطبيق الانتقال المجدول مباشرة؛ Conecte el cable de alimentación y el conector `PolicyTransitionPrerequisiteFailed` para conectarlo y conectarlo.

### Secuenciación de migración

1. **Preparar registros:** فعّل كل مدخلات verifier والمعلمات المشار اليها في السياسة المستهدفة. تعلن العقد `conf_features` الناتجة حتى يتمكن peers من التحقق من التوافق.
2. **Etapa la transición:** قدّم `ScheduleConfidentialPolicyTransition` مع `effective_height` يراعي `policy_transition_delay_blocks`. Utilice el dispositivo `ShieldedOnly` y utilice el dispositivo (`window ≥ policy_transition_window_blocks`).
3. **Publicar guía del operador:** سجّل `transition_id` المعاد ووزع runbook للـ rampa de entrada/salida. Utilice el `/v2/confidential/assets/{id}/transitions` para conectar y desconectar el dispositivo.
4. **Aplicación de ventanas:** عند فتح النافذة يحول runtime السياسة الى `Convertible`, ويصدر `PolicyTransitionWindowOpened { transition_id }` ويبدأ في رفض طلبات الحوكمة المتعارضة.
5. **Finalizar o cancelar:** عند `effective_height` يتحقق runtime من الشروط المسبقة (عرض شفاف صفر، عدم وجود سحب طارئ، الخ). النجاح يقلب السياسة للوضع المطلوب؛ Asegúrese de que `PolicyTransitionPrerequisiteFailed` esté conectado a la red eléctrica y de que esté conectado.
6. **Actualizaciones de esquema:** بعد نجاح الانتقال ترفع الحوكمة نسخة مخطط الاصل (مثلا `asset_definition.v2`) وتتطلب ادوات CLI حقل `confidential_policy` Se manifiesta عند تسلسل. توجه وثائق ترقية genesis المشغلين لاضافة اعدادات السياسة وبصمات registro قبل اعادة تشغيل المدققين.الشبكات الجديدة التي تبدأ مع تمكين السرية ترمز السياسة المطلوبة مباشرة في genesis. مع ذلك تتبع نفس قائمة التحقق عند تغيير الاوضاع بعد الاطلاق كي تبقى نوافذ التحويل حتمية وتمتلك المحافظ وقتا للتكيف.

### نسخ Norito manifiesta والتفعيل- يجب ان تتضمن Génesis manifiesta `SetParameter` للمفتاح المخصص `confidential_registry_root`. Carga útil de Norito JSON compatible con `ConfidentialRegistryMeta { vk_set_hash: Option<String> }`: احذف الحقل (`null`) عندما لا توجد ادخالات verifier نشطة، Y es un código hexadecimal de 32 bits (`0x…`) que contiene el verificador de `compute_vk_set_hash`. ترفض العقد البدء اذا كان المعامل مفقودا او الهاش لا يطابق كتابات السجل المرمزة.
- يضمن `ConfidentialFeatureDigest::conf_rules_version` manifiesto de conexión en línea. La v1 está basada en `Some(1)` y `iroha_config::parameters::defaults::confidential::RULES_VERSION`. عند تطور القواعد ارفع الثابت، اعادة توليد manifiestos, واطلاق البيناريات معا؛ La configuración del sistema se realiza mediante `ConfidentialFeatureDigestMismatch`.
- ينبغي ان تجمع Manifiestos de activación تحديثات registro وتغييرات دورة حياة المعلمات وانتقالات السياسة بحيث يبقى resumen متسقا:
  1. طبق طفرات registro المخططة (`Publish*`, `Set*Lifecycle`) في عرض حالة fuera de línea y resumen بعد التفعيل باستخدام `compute_confidential_feature_digest`.
  2. اصدِر `SetParameter::custom(confidential_registry_root, {"vk_set_hash": "0x…"})` باستخدام الهاش المحسوب حتى يتمكن peers المتاخرون من استعادة digest الصحيح حتى اذا فاتتهم تعليمات registro الوسيطة.
  3. ارفق تعليمات `ScheduleConfidentialPolicyTransition`. يجب على كل تعليمة ان تقتبس `transition_id` الصادر من الحوكمة؛ manifiesta el tiempo de ejecución.
  4. Utilice el manifiesto y SHA-256 y el resumen de la información. يتحقق المشغلون من الثلاثة قبل التصويت لتجنب الانقسام.- عندما تتطلب عمليات الاطلاق cut-over مؤجلا، سجل الارتفاع المستهدف في معامل مخصص مرافق (مثلا `custom.confidential_upgrade_activation_height`). هذا يعطي المدققين دليلا مشفرا بنوريتو على ان المدققين احترموا نافذة الاشعار قبل سريان تغيير digest.

## دورة حياة verificador والمعلمات
### Registro ZK
- Libro mayor `ZkVerifierEntry { vk_id, circuit_id, version, proving_system, curve, public_inputs_schema_hash, vk_hash, vk_len, max_proof_bytes, gas_schedule_id, activation_height, deprecation_height, withdraw_height, status, metadata_uri_cid, vk_bytes_cid }` y `proving_system` y el libro mayor `Halo2`.
- ازواج `(circuit_id, version)` فريدة عالميا؛ يحافظ السجل على فهرس ثانوي للبحث حسب metadatos الدائرة. محاولات تسجيل زوج مكرر ترفض عند admisión.
- يجب ان يكون `circuit_id` غير فارغ ويجب توفير `public_inputs_schema_hash` (hash Blake2b-32 para el verificador). ترفض admisión السجلات التي تهمل هذه الحقول.
- تعليمات الحوكمة تشمل:
  - `PUBLISH` لاضافة مدخل `Proposed` بmetadata فقط.
  - `ACTIVATE { vk_id, activation_height }` لجدولة تفعيل المدخل عند حدود época.
  - `DEPRECATE { vk_id, deprecation_height }` لتحديد اخر ارتفاع يمكن فيه للـ pruebas الاشارة الى المدخل.
  - `WITHDRAW { vk_id, withdraw_height }` لاغلاق طارئ؛ الاصول المتاثرة تجمد الانفاق السري بعد retirar altura حتى تتفعل ادخالات جديدة.
- Génesis manifiesta تصدر تلقائيا معامل `confidential_registry_root` بقيمة `vk_set_hash` المطابقة للادخالات النشطة؛ يتحقق التحقق من هذا digest مقابل حالة السجل المحلية قبل انضمام العقدة للاجماع.
- تسجيل او تحديث verificador يتطلب `gas_schedule_id`؛ Pruebas de Halo2 `OpenVerifyEnvelope` y pruebas de Halo2 `OpenVerifyEnvelope` يطابق `circuit_id` و`vk_hash` و`public_inputs_schema_hash` في سجل السجل.### Claves de demostración
- تبقى claves de prueba خارج الدفتر لكن تشير اليها معرفات contenido dirigido (`pk_cid`, `pk_hash`, `pk_len`) منشورة مع verificador de metadatos.
- تقوم Wallet SDKs بجلب بيانات PK والتحقق من الهاش وحفظها محليا.

### Parámetros de Pedersen y Poseidón
- سجلات منفصلة (`PedersenParams`, `PoseidonParams`) تعكس ضوابط دورة حياة verifier, y لكل منها `params_id`, هاشات المولدات/الثوابت، وارتفاعات التفعيل/الاستبدال/السحب.
- Compromisos de compromiso y compromisos `params_id` حتى لا تعيد تدوير المعلمات استخدام انماط بت من مجموعات قديمة؛ يدمج الـ ID في notas de compromiso وعلامات مجال anulador.

## الترتيب الحتمي y anuladores
- يحافظ كل اصل على `CommitmentTree` مع `next_leaf_index`; تضيف الكتل compromisos بترتيب حتمي: تمر المعاملات بترتيب الكتلة؛ داخل كل معاملة تمر مخرجات تصاعديا حسب `output_idx` المتسلسل.
- `note_position` مشتق من ازاحات الشجرة لكنه **ليس** جزءا من nullifier؛ يستخدم فقط لمسارات العضوية ضمن testigo de prueba.
- يضمن تصميم PRF ثبات anulador عند reorgs؛ يربط مدخل PRF `{ nk, note_preimage_hash, asset_id, chain_id, params_id }`, وتستند Anchors الى جذور Merkle تاريخية محدودة بـ `max_anchor_age_blocks`.## libro mayor
1. **MintConfidential {id_activo, monto, sugerencia_destinatario}**
   - يتطلب سياسة اصل `Convertible` او `ShieldedOnly`; يتحقق admisión من سلطة الاصل، يجلب `params_id` الحالي، يعين `rho`, يصدر compromiso y Merkle árbol.
   - يصدر `ConfidentialEvent::Shielded` مع compromiso جديد، فرق Merkle root، وhash استدعاء المعاملة للتدقيق.
2. **TransferConfidential {id_activo, prueba, id_circuito, versión, anuladores, nuevos_compromisos, enc_payloads, raíz_ancla, memo}**
   - يتحقق syscall في VM من prueba باستخدام مدخل السجل؛ Hay host y anuladores, compromisos y anclas.
   - Libro mayor `NullifierSet`, cargas útiles y anuladores `ConfidentialEvent::Transferred` والمخرجات المرتبة وproof hachís وraíces de Merkle.
3. **RevelarConfidencial {id_activo, prueba, id_circuito, versión, anulador, monto, cuenta_destinatario, raíz_ancla}**
   - متاحة فقط للاصول `Convertible`; تتحقق prueba ان قيمة nota تساوي المبلغ المكشوف، يضيف libro mayor الرصيد الشفاف ويحرق nota blindada بوسم anulador كمصروف.
   - `ConfidentialEvent::Unshielded` contiene anulificadores, pruebas y hash.## modelo de datos اضافات
- `ConfidentialConfig` (قسم تهيئة جديد) مع علم التفعيل، `assume_valid`, مقابض gas/limits, نافذة Anchor, y verifier backend.
- `ConfidentialNote`, `ConfidentialTransfer`, y `ConfidentialMint` de la serie Norito de la marca (`CONFIDENTIAL_ASSET_V1 = 0x01`).
- `ConfidentialEncryptedPayload`, bytes de notas AEAD, `{ version, ephemeral_pubkey, nonce, ciphertext }` y `version = CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1`, XChaCha20-Poly1305.
- توجد متجهات اشتقاق المفاتيح القانونية في `docs/source/confidential_key_vectors.json`; El punto final CLI y Torii está conectado a la red.
- يحصل `asset::AssetDefinition` a `confidential_policy: AssetConfidentialPolicy { mode, vk_set_hash, poseidon_params_id, pedersen_params_id, pending_transition }`.
- يحفظ `ZkAssetState` ربط `(backend, name, commitment)` لـ verificadores de transferencia/desprotección؛ يرفض التنفيذ pruebas التي لا تطابق clave de verificación المسجل (مرجعا او ضمنيا).
- يتم تخزين `CommitmentTree` (para los puntos de control fronterizos), و `NullifierSet` بالمفتاح `(chain_id, asset_id, nullifier)`, و `ZkVerifierEntry` و `PedersenParams` y `PoseidonParams` son el estado mundial.
- Mempool incluye `NullifierIndex` e `AnchorIndex` para crear un anclaje.
- تحديثات مخطط Norito تشمل ترتيبًا قانونيًا لـ entradas públicas؛ اختبارات ida y vuelta تضمن حتمية الترميز.
- Pruebas unitarias de carga útil cifrada de ida y vuelta (`crates/iroha_data_model/src/confidential.rs`). ستضيف متجهات المحافظ لاحقا Transcripciones de la AEAD قانونية للمدققين. يوثق `norito.md` ترويسة on-wire للـ sobre.## Actualiza IVM y syscall
- Llamada al sistema `VERIFY_CONFIDENTIAL_PROOF`:
  - `circuit_id`, `version`, `scheme`, `public_inputs`, `proof` y `ConfidentialStateDelta { asset_id, nullifiers, commitments, enc_payloads }`.
  - يقوم syscall بتحميل verificador de metadatos من السجل، يفرض حدود الحجم/الوقت، يحسب gas حتمي، ولا يطبق delta الا عند نجاح prueba.
- Rasgo de host de solo lectura `ConfidentialLedger` para instantáneas de Merkle y anulador Los ayudantes Kotodama son testigos y esquemas.
- تم تحديث وثائق puntero-ABI لتوضيح búfer de prueba de diseño y registro.

## تفاوض قدرات العقد
- Hay un apretón de manos `feature_bits.confidential` o `ConfidentialFeatureDigest { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`. Verificador de backend `confidential.enabled=true` e `assume_valid=false` y verificador backend y resumen de datos Hay un apretón de manos en `HandshakeConfidentialMismatch`.
- يدعم config `assume_valid` لعقد observers فقط: عند تعطيله، تؤدي تعليمات السرية الى `UnsupportedInstruction` حتمي بلا pánico؛ عند تمكينه، يطبق observadores دلتا الحالة المعلنة دون التحقق من pruebas.
- يرفض mempool المعاملات السرية اذا كانت القدرة المحلية معطلة. تتجنب فلاتر chismes ارسال المعاملات blindados الى pares غير متوافقة بينما تعيد توجيه معرفات verificador غير المعروفة بشكل اعمى ضمن حدود الحجم.

### مصفوفة توافق apretón de manos| اعلان الطرف البعيد | النتيجة لعقد المدققين | ملاحظات المشغل |
|---------------------|--------------------------------|----------------|
| `enabled=true`, `assume_valid=false`, backend متطابق, resumen متطابق | مقبول | يصل peer الى حالة `Ready` ويشارك في propuesta, votación y distribución de RBC. لا يتطلب تدخل يدوي. |
| `enabled=true`, `assume_valid=false`, backend متطابق, resumen قديم او مفقود | hombre (`HandshakeConfidentialMismatch`) | يجب على الطرف البعيد تطبيق تفعيل السجلات/المعلمات المعلقة او انتظار `activation_height` المجدولة. حتى التصحيح يبقى قابلا للاكتشاف لكنه لا يدخل دوران الاجماع. |
| `enabled=true`, `assume_valid=true` | hombre (`HandshakeConfidentialMismatch`) | يتطلب المدققون تحقق pruebas؛ قم بضبط الطرف البعيد كمراقب عبر Torii فقط او اجعل `assume_valid=false` بعد تمكين التحقق الكامل. |
| `enabled=false`, apretón de manos (handshake) y backend del verificador | hombre (`HandshakeConfidentialMismatch`) | compañeros قديمة او محدثة جزئيا لا يمكنها الانضمام لشبكة الاجماع. قم بترقيتها وتأكد من تطابق backend + resumen قبل اعادة الاتصال. |

العقد المراقِبة التي تتجاوز تحقق pruebas عمدا يجب الا تفتح اتصالات اجماع مع مدققين يعملون ببوابات قدرات. يمكنها استيعاب الكتل عبر Torii او واجهات الارشفة, لكن شبكة الاجماع ترفضها حتى تعلن قدرات متوافقة.

### سياسة تقليم Revelar والاحتفاظ بـ Nulificador

يجب على دفاتر السرية الاحتفاظ بتاريخ كاف لاثبات حداثة notas y تشغيل تدقيقات الحوكمة. El mensaje de texto `ConfidentialLedger` es el siguiente:- **Retención de anulador:** الاحتفاظ بـ anuladores المصروفة لمدة *ادنى* `730` يوما (24 شهرا) بعد ارتفاع الصرف، او لفترة اطول اذا فرضها المنظم. يمكن للمشغلين تمديدها عبر `confidential.retention.nullifier_days`. Hay anuladores que contienen anuladores Torii que funcionan con doble gasto.
- **Revelar poda:** تقوم Revela الشفافة (`RevealConfidential`) بتقليم compromisos المرتبطة فورا بعد اكتمال الكتلة، لكن anulador المصروف يبقى خاضعا لقاعدة الاحتفاظ. تسجل احداث revelar (`ConfidentialEvent::Unshielded`) المبلغ العلني والمستلم وhashproof كي لا تتطلب اعادة بناء الكشوف التاريخية ciphertext المقتطع.
- **Puntos de control fronterizos:** تحافظ compromisos fronterizos على puntos de control متحركة تغطي الاكبر من `max_anchor_age_blocks` ونافذة الاحتفاظ. لا تقوم العقد بضغط puntos de control الاقدم الا بعد انتهاء كل anuladores في تلك الفترة.
- **Remediación de resumen obsoleto:** اذا رُفع `HandshakeConfidentialMismatch` بسبب انحراف digest، يجب على المشغلين (1) التحقق من تطابق نوافذ الاحتفاظ عبر الكتلة، (2) تشغيل `iroha_cli app confidential verify-ledger` لاعادة توليد digest مقابل مجموعة nullifier المحتفظ بها، و(3) اعادة نشر manifest المحدث. يجب استعادة اي anuladores حذفت مبكرا من التخزين البارد قبل اعادة الانضمام للشبكة.

Descripción detallada del runbook de operaciones سياسات الحوكمة التي تمد نافذة الاحتفاظ يجب ان تحدث تهيئة العقد وخطط التخزين الارشيفي بالتزامن.

### تدفق الاخلاء والاستعادة1. اثناء الاتصال، يقارن `IrohaNetwork` القدرات المعلنة. اي عدم تطابق يرفع `HandshakeConfidentialMismatch`؛ Hay una cola de descubrimiento de pares en el enlace `Ready`.
2. تظهر المشكلة في سجل خدمة الشبكة (مع digest والـ backend للطرف البعيد), y للا يقوم Sumeragi بجدولة peer للتقديم او التصويت.
3. Verificador y verificador de datos (`vk_set_hash`, `pedersen_params_id`, `poseidon_params_id`) y بتهيئة `next_conf_features` مع `activation_height` متفق عليها. عندما يتطابق resumen ينجح apretón de manos التالي تلقائيا.
4. اذا تمكن peer قديم من بث كتلة (مثلا عبر repetición de archivo), يرفضها المدققون حتميا بـ `BlockRejectionReason::ConfidentialFeatureDigestMismatch` للحفاظ على اتساق libro mayor عبر الشبكة.

### Flujo de protocolo de enlace seguro para reproducción1. Haga clic en el botón Noise/X25519. Carga útil de protocolo de enlace (`handshake_signature_payload`) Conexión de sockets y conexiones de protocolo de enlace Utilice `handshake_chain_id`. يتم تشفير الرسالة بـ AEAD قبل ارسالها.
2. يعيد المستجيب حساب payload بترتيب مفاتيح peer/local المعكوس ويتحقق من توقيع Ed25519 المضمن في `HandshakeHelloV1`. بما ان كلا المفتاحين المؤقتين والعنوان المعلن ضمن نطاق التوقيع، فاعادة تشغيل رسالة ملتقطة ضد peer اخرى او استعادة اتصال قديم تفشل حتميا.
3. Coloque el botón de encendido y el `ConfidentialFeatureDigest` en `HandshakeConfidentialMeta`. La tupla `{ enabled, assume_valid, verifier_backend, digest }` y la tupla `ConfidentialHandshakeCaps` اي عدم تطابق ينهي apretón de manos بـ `HandshakeConfidentialMismatch` قبل انتقال النقل الى `Ready`.
4. يجب على المشغلين اعادة حساب digest (عبر `compute_confidential_feature_digest`) واعادة تشغيل العقد بسياسات/سجلات محدثة قبل اعادة الاتصال. pares التي تعلن resúmenes قديمة تستمر في الفشل، مانعة دخول حالة قديمة الى مجموعة المدققين.
5. نجاحات وفشل handshakes تحدث عدادات `iroha_p2p::peer` القياسية (`handshake_failure_count` وغيرها) وتصدر سجلات منظمة مع وسم معرف peer البعيد وبصمة resumen. راقب هذه المؤشرات لاكتشاف محاولات repetición y lanzamiento de او سوء التهيئة اثناء.## ادارة المفاتيح y cargas útiles
- تسلسل اشتقاق المفاتيح لكل حساب:
  - `sk_spend` → `nk` (clave anuladora), `ivk` (clave de visualización entrante), `ovk` (clave de visualización saliente), `fvk`.
- Notas de cargas útiles de تستخدم المشفرة AEAD مع مفاتيح مشتركة مشتقة من ECDH؛ يمكن ارفاق claves de vista de auditor اختيارية الى salidas حسب سياسة الاصل.
- CLI: `confidential create-keys`, `confidential send`, `confidential export-view-key`, memorandos y notas `iroha app zk envelope` لانتاج/فحص sobres Norito دون اتصال. Utilice Torii para conectar `POST /v2/confidential/derive-keyset` y hexadecimal y base64 para conectar archivos. المفاتيح برمجيا.## الغاز، الحدود، وضوابط DoS
- جدول gas حتمي:
  - Halo2 (Plonkish): اساس `250_000` gas + `2_000` gas لكل entrada pública.
  - `5` gas لكل بايت prueba, مع رسوم لكل anulador (`300`) y compromiso (`500`).
  - يمكن للمشغلين تجاوز هذه الثوابت عبر تهيئة العقد (`confidential.gas.{proof_base, per_public_input, per_proof_byte, per_nullifier, per_commitment}`)؛ تنتشر التغييرات عند البدء او اعادة تحميل التهيئة وتطبق حتميا عبر الكتلة.
- حدود صارمة (افتراضات قابلة للضبط):
- `max_proof_size_bytes = 262_144`.
- `max_nullifiers_per_tx = 8`, `max_commitments_per_tx = 8`, `max_confidential_ops_per_block = 256`.
- `verify_timeout_ms = 750`, `max_anchor_age_blocks = 10_000`. pruebas التي تتجاوز `verify_timeout_ms` تقطع التعليمة حتميا (تصويتات الحوكمة تصدر `proof verification exceeded timeout` و`VerifyProof` يعيد خطا).
- Nombres de los constructores: `max_proof_bytes_block`, `max_verify_calls_per_tx`, `max_verify_calls_per_block`, y `max_public_inputs` son constructores `reorg_depth_bound` (≥ `max_anchor_age_blocks`) يتحكم في احتفاظ puestos de control fronterizos.
- Tiempo de ejecución de inicio de sesión de inicio de sesión y de inicio de sesión `InvalidParameter` ابقاء حالة libro mayor دون تغيير.
- يرشح mempool المعاملات السرية مسبقا حسب `vk_id` وطول prueba وعمر ancla قبل استدعاء verificador للحفاظ على حدود الموارد.
- يتوقف التحقق حتميا عند timeout او تجاوز الحدود؛ تفشل المعاملات باخطاء واضحة. backends SIMD اختيارية لكنها لا تغير حساب gas.

### خطوط اساس المعايرة وبوابات القبول
- **Plataformas de referencia.** يجب ان تغطي معايرات الاداء ثلاث ملفات عتاد ادناه. اي معايرة لا تلتقط جميع الملفات ترفض اثناء المراجعة.| الملف | المعمارية | CPU/instancia | اعلام المترجم | الغاية |
  | --- | --- | --- | --- | --- |
  | `baseline-simd-neutral` | `x86_64` | AMD EPYC 7B12 (32c) e Intel Xeon Gold 6430 (24c) | `RUSTFLAGS="-C target-feature=-avx,-avx2,-fma"` | تأسيس قيم ارضية بدون تعليمات متجهة؛ يستخدم لضبط جداول تكلفة respaldo. |
  | `baseline-avx2` | `x86_64` | Intel Xeon Gold 6430 (24c) | versión predeterminada | يتحقق من مسار AVX2؛ يفحص ان تسريعات SIMD ضمن تسامح gas المحايد. |
  | `baseline-neon` | `aarch64` | AWS Graviton3 (c7g.4xlarge) | versión predeterminada | Este es el backend NEON y es compatible con x86. |

- **Arnés de referencia.** يجب انتاج كل تقارير معايرة الغاز باستخدام:
  - `CRITERION_HOME=target/criterion cargo bench -p iroha_core isi_gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>`
  - `cargo test -p iroha_core bench_repro -- --ignored` Accesorio de montaje en superficie.
  - `CRITERION_HOME=target/criterion cargo bench -p ivm gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>` كلما تغيرت تكاليف Código de operación de VM.

- **Aleatoriedad fija.** صدّر `IROHA_CONF_GAS_SEED=conf-gas-seed-2026Q1` قبل تشغيل البنش حتى يتحول `iroha_test_samples::gen_account_in` الى مسار `KeyPair::from_seed` الحتمي. Arnés de seguridad `IROHA_CONF_GAS_SEED_ACTIVE=…` Hombre y mujer اذا غاب المتغير يجب ان تفشل المراجعة. يجب على اي ادوات معايرة جديدة احترام هذا المتغير عند ادخال عشوائية اضافية.

- **Captura de resultados.**
  - ارفع Resúmenes de criterios (`target/criterion/**/raw.csv`) لكل ملف الى artefacto الاصدار.
  - خزّن المقاييس المشتقة (`ns/op`, `gas/op`, `ns/gas`) en [Libro mayor de calibración de gas confidencial](./confidential-gas-calibration) con git commit y المترجم المستخدمة.
  - احتفظ بآخر baseline اثنين لكل ملف؛ احذف اللقطات الاقدم بعد اعتماد التقرير الاحدث.- **Tolerancias de aceptación.**
  - Los ajustes de temperatura entre `baseline-simd-neutral` e `baseline-avx2` son ≤ ±1,5%.
  - Los ajustes de temperatura entre `baseline-simd-neutral` e `baseline-neon` son ≤ ±2,0%.
  - Las aplicaciones de RFC y las aplicaciones de RFC.

- **Revisar lista de verificación.** يتحمل مقدمو الطلب مسؤولية:
  - تضمين `uname -a` ومقتطفات `/proc/cpuinfo` (modelo, paso a paso) و `rustc -Vv` في سجل المعايرة.
  - التحقق من ظهور `IROHA_CONF_GAS_SEED` في خرج البنش (البنش يطبع semilla النشط).
  - ضمان تطابق marcapasos وميزات verificador confidencial مع producción (`--features confidential,telemetry` عند تشغيل البنش مع Telemetría).

## التهيئة والعمليات
- Aquí `iroha_config` frente a `[confidential]`:
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
- Nombre del fabricante: `confidential_proof_verified`, `confidential_verifier_latency_ms`, `confidential_proof_bytes_total`, `confidential_nullifier_spent`, `confidential_commitments_appended`, `confidential_mempool_rejected_total{reason}`, Y `confidential_policy_transitions_total` es un texto sin formato.
- سطوح RPC:
  - `GET /confidential/capabilities`
  - `GET /confidential/zk_registry`
  - `GET /confidential/params`## استراتيجية الاختبار
- الحتمية: خلط المعاملات عشوائيا داخل الكتل ينتج نفس Conjuntos de anulación y raíces de Merkle.
- تحمل reorg: محاكاة reorg متعدد الكتل مع anclajes؛ تبقى anuladores مستقرة y anclajes القديمة.
- ثوابت الغاز: تاكد من نفس استخدام الغاز عبر عقد مع او بدون تسريع SIMD.
- اختبار الحدود: pruebas عند سقوف الحجم/الغاز، الحد الاقصى للمدخلات/المخرجات، وتطبيق tiempo de espera.
- دورة الحياة: عمليات الحوكمة لتفعيل/استبدال verificador والمعلمات، واختبارات صرف الدوران.
- Política FSM: الانتقالات المسموح بها/المرفوضة، تأخيرات pendiente de transición, ورفض mempool حول alturas efectivas.
- طوارئ السجل: سحب طارئ يجمد الاصول المتاثرة عند `withdraw_height` ويرفض بعدها.
- Control de capacidad: المدققون مع `conf_features` غير متطابقة يرفضون الكتل؛ observadores مع `assume_valid=true` يواكبون دون التأثير على الاجماع.
- تكافؤ الحالة: عقد validador/completo/observador تنتج نفس raíces estatales على السلسلة القانونية.
- Fuzzing negativo: pruebas تالفة، payloads كبيرة جدا، وتصادمات anulador ترفض حتميا.## الهجرة والتوافق
- طرح محكوم بالميزة: حتى اكتمال Phase C3, `enabled` افتراضيه `false`؛ تعلن العقد قدراتها قبل الانضمام الى مجموعة المدققين.
- الاصول الشفافة غير متاثرة؛ التعليمات السرية تتطلب ادخالات السجل والتفاوض على القدرات.
- العقد المترجمة دون دعم السرية ترفض الكتل المعنية حتميا؛ لا يمكنها الانضمام لمجموعة المدققين لكنها قد تعمل كمراقبين مع `assume_valid=true`.
- Génesis manifiesta تشمل ادخالات سجل اولية، مجموعات معلمات، سياسات سرية للاصول، ومفاتيح مدققين اختيارية.
- يتبع المشغلون runbooks المنشورة لتدوير السجل وانتقالات السياسة والسحب الطارئ للحفاظ على ترقية حتمية.

## اعمال متبقية
- قياس مجموعات معلمات Halo2 (حجم الدائرة، استراتيجية) y تسجيل النتائج في calibration playbook حتى يمكن تحديث افتراضات gas/timeout مع تحديث `confidential_assets_calibration.md` القادم.
- انهاء سياسات الافصاح للمدققين وواجهات visualización selectiva المرتبطة، وربط سير العمل المعتمد في Torii بعد توقيع مسودة الحوكمة.
- توسيع مخطط cifrado de testigos ليغطي مخرجات متعددة المستلمين وmemos مجمعة، وتوثيق تنسيق sobre لمطوري SDK.
- طلب مراجعة امنية خارجية للدوائر والسجلات واجراءات تدوير المعلمات وارشفة النتائج بجانب تقارير التدقيق الداخلية.
- تحديد واجهات API لتسوية للمدققين للمدققين ونشر ارشاد نطاق view-key حتى ينفذ مزودو المحافظ نفس دلالات الاثبات.## مراحل التنفيذ
1. **Fase M0 — Endurecimiento de parada de envío**
   - ✅ اشتقاق anulador يتبع الان تصميم Poseidon PRF (`nk`, `rho`, `asset_id`, `chain_id`) مع فرض ترتيب compromisos الحتمي في تحديثات libro mayor.
   - ✅ يفرض التنفيذ حدود حجم pruebas وحصص العمليات السرية لكل معاملة/كتلة، رافضا المعاملات المتجاوزة باخطاء حتمية.
   - ✅ يعلن P2P handshake `ConfidentialFeatureDigest` (resumen backend + بصمات السجل) y عدم التطابق حتميا عبر `HandshakeConfidentialMismatch`.
   - ✅ ازالة pánicos في مسارات تنفيذ السرية واضافة rol gating للعقد غير المتوافقة.
   - فرض ميزانيات tiempo de espera del verificador y reorganización de los puntos de control fronterizos.
     - ✅ تطبيق ميزانيات tiempo de espera؛ pruebas التي تتجاوز `verify_timeout_ms` تفشل الان حتميا.
     - ✅ احترام `reorg_depth_bound` y puntos de control الاقدم من النافذة مع الحفاظ على instantáneas حتمية.
   - Aplicación `AssetConfidentialPolicy` y política FSM y aplicación de la ley mint/transfer/reveal.
   - Haga clic en `conf_features` para configurar el registro/parámetro del resumen.
2. **Fase M1: Registros y parámetros**
   - Las baterías `ZkVerifierEntry`, `PedersenParams` y `PoseidonParams` están conectadas a la fuente de alimentación, a la génesis y a la conexión.
   - توصيل syscall لفرض búsquedas de registro وgas programación IDs وschema hash وفحوصات الحجم.
   - La carga útil de la versión 1 está disponible para la instalación de CLI y la configuración de la CLI.
3. **Fase M2: Gas y rendimiento**- تنفيذ جدول حتمي، عدادات لكل كتلة، وحزم قياس مع تليمتري (verifique la latencia, احجام pruebas, رفض mempool).
   - تقوية Puntos de control de CommitmentTree و LRU cargando وíndices anulados للاحمال متعددة الاصول.
4. **Fase M3: Rotación y herramientas de cartera**
   - تمكين قبول pruebas متعددة المعلمات والنسخ؛ Esta activación/obsolescencia está relacionada con los runbooks.
   - تقديم تدفقات هجرة لـ wallet SDK/CLI, سير عمل مسح المدققين، وادوات تسوية.
5. **Fase M4: Auditoría y operaciones**
   - توفير تدفقات مفاتيح المدققين، واجهات divulgación selectiva, وrunbooks تشغيلية.
   - جدولة مراجعة تشفير/امن خارجية ونشر النتائج في `status.md`.

كل مرحلة تحدث معالم roadmap والاختبارات المرتبطة لضمان التنفيذ الحتمي لشبكة البلوكتشين.
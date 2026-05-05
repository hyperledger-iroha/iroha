---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/confidential-assets.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : الاصول السرية وتحويلات ZK
description: مخطط Phase C للدوران blindé والسجلات وضوابط المشغلين.
slug : /nexus/actifs-confidentiels
---
<!--
SPDX-License-Identifier: Apache-2.0
-->
# تصميم الاصول السرية وتحويلات ZK

## الدوافع
- تقديم تدفقات اصول blinded اختيارية حتى تتمكن الدومينات من حفظ خصوصية المعاملات دون تغيير الدوران الشفاف.
- Vous pouvez également utiliser le logiciel Norito/Kotodama ABI v1.
- تزويد المدققين والمشغلين بضوابط دورة الحياة (تفعيل، تدوير، سحب) للدوائر والمعلمات التشفيرية.

## نموذج التهديدات
- المدققون honnête-mais-curieux : ينفذون الاجماع بامانة لكنهم يحاولون فحص grand livre/état.
- مراقبو الشبكة يرون بيانات الكتل والمعاملات المرسلة عبر potins؛ Il y a beaucoup de potins.
- خارج النطاق : تحليل حركة المرور خارج الدفتر، خصوم كميون (يتابعون في PQ roadmap) et وهجمات توفر ledger.## نظرة عامة على التصميم
- يمكن للاصول اعلان *piscine blindée* اضافة الى الارصدة الشفافة؛ يتم تمثيل الدوران a protégé les engagements عبر تشفيرية.
- Notes `(asset_id, amount, recipient_view_key, blinding, rho)` sur :
  - Engagement : `Comm = Pedersen(params_id || asset_id || amount || recipient_view_key || blinding)`.
  - Nullifier : `Null = Poseidon(domain_sep || nk || rho || asset_id || chain_id)` مستقل عن ترتيب notes.
  - Charge utile cryptée : `enc_payload = AEAD_XChaCha20Poly1305(ephemeral_shared_key, note_plaintext)`.
- Charges utiles chargées `ConfidentialTransfer` pour les charges utiles :
  - Entrées publiques : ancre Merkle, annulateurs, engagements, identifiant d'actif, نسخة الدائرة.
  - Charges utiles مشفرة للمستلمين والمدققين الاختياريين.
  - Preuve de connaissance nulle تؤكد حفظ القيمة والملكية والتفويض.
- يتم التحكم في vérifier les clés ومجموعات المعلمات عبر سجلات على الدفتر مع نوافذ تفعيل؛ ترفض العقد التحقق من proofs تشير الى ادخالات مجهولة او مسحوبة.
- تلتزم رؤوس الاجماع ب digest ميزات السرية النشطة كي لا تقبل الكتل الا عند تطابق حالة السجلات والمعلمات.
- Preuves de Halo2 (Plonkish) pour configuration fiable Groth16 et SNARK sont disponibles dans la version v1.

### Calendrier حتمية

Le mémo de l'article concerne le luminaire du `fixtures/confidential/encrypted_payload_v1.json`. L'enveloppe v1 est également compatible avec les SDK et les SDK. Utilisez le modèle de données Rust (`crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs`) et Swift (`IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift`) pour le montage en utilisant l'encodage Norito. الاخطاء وتغطية الانحدار مع تطور الكودك.Les SDK Swift utilisent Shield pour la colle JSON comme : `ShieldRequest` pour la note d'engagement de 32 pour la charge utile et les métadonnées de débit. Utilisez `IrohaSDK.submit(shield:keypair:)` (`submitAndWait`) pour vous connecter à `/v1/pipeline/transactions`. يقوم المساعد بالتحقق من اطوال engagements, ويمرر `ConfidentialEncryptedPayload` الى Norito codeur, ويعكس layout `zk::Shield` الموضح ادناه Il s'agit d'une application de Rust.

## Engagements et contrôle des engagements
- تكشف رؤوس الكتل `conf_features = { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`؛ يشارك digest في hash الاجماع ويجب ان يساوي عرض السجل المحلي لقبول الكتلة.
- يمكن للحوكمة تجهيز الترقيات ببرمجة `next_conf_features` مع `activation_height` مستقبلي؛ وحتى ذلك الارتفاع يجب على منتجي الكتل الاستمرار في اصدار digest السابق.
- يجب على عقد المدققين التشغيل مع `confidential.enabled = true` et `assume_valid = false`. ترفض فحوصات البدء الانضمام الى مجموعة المدققين اذا فشل اي شرط او اختلفت `conf_features` محليا.
- La poignée de main P2P est utilisée par `{ enabled, assume_valid, conf_features }`. يتم رفض peers الذين يعلنون ميزات غير متوافقة بخطأ `HandshakeConfidentialMismatch` ولا يدخلون ابدا في دوران الاجماع.
- Prise en charge des observateurs et des pairs pour la prise de contact avec [Négociation de capacité de nœud] (#node-capability-negotiation). Il s'agit de la poignée de main `HandshakeConfidentialMismatch` et du peer خارج دوران الاجماع حتى يتطابق digest.
- يمكن للمراقبين غير المدققين ضبط `assume_valid = true`؛ يطبقون دلتا سرية بشكل اعمى دون التأثير على سلامة الاجماع.## سياسات الاصول
- يحمل كل تعريف اصل `AssetConfidentialPolicy` يحدده المنشئ او عبر الحوكمة:
  - `TransparentOnly` : Nom de la personne يسمح فقط بتعليمات شفافة (`MintAsset`, `TransferAsset`, الخ) وترفض العمليات blindé.
  - `ShieldedOnly` : يجب ان تستخدم كل الاصدارات والتحويلات تعليمات سرية؛ يحظر `RevealConfidential` حتى لا تظهر الارصدة علنا.
  - `Convertible` : يمكن للحاملين نقل القيمة بين التمثيل الشفاف والـ blindage باستخدام تعليمات on/off-ramp ادناه.
- تتبع السياسات FSM مقيد لمنع تعلق الاموال:
  - `TransparentOnly → Convertible` (piscine blindée).
  - `TransparentOnly → ShieldedOnly` (يتطلب انتقالا معلقا ونافذة تحويل).
  - `Convertible → ShieldedOnly` (تاخير ادنى الزامى).
  - `ShieldedOnly → Convertible` (يتطلب خطة هجرة لضمان بقاء notes قابلة للصرف).
  - `ShieldedOnly → TransparentOnly` est un élément de piscine blindée et des notes supplémentaires.
- تعليمات الحوكمة تضبط `pending_transition { new_mode, effective_height, previous_mode, transition_id, conversion_window }` عبر ISI `ScheduleConfidentialPolicyTransition` ويمكنها الغاء التغييرات المجدولة بـ `CancelConfidentialPolicyTransition`. يضمن تحقق mempool عدم عبور اي معاملة لارتفاع الانتقال، ويفشل الادراج بشكل حتمي اذا تغير فحص السياسة في منتصف الكتلة.
- يتم تطبيق الانتقالات المعلقة تلقائيا عند فتح كتلة جديدة: عند دخول ارتفاع الكتلة نافذة التحويل (pour `ShieldedOnly`) et pour le module `effective_height`, pour le runtime avec `AssetConfidentialPolicy` et les métadonnées `zk.policy` pour le module `ShieldedOnly`. المعلق. Il s'agit d'un système d'exécution `ShieldedOnly` qui est également utilisé pour le runtime.- مقابض التهيئة `policy_transition_delay_blocks` et `policy_transition_window_blocks` تفرض اشعارا ادنى وفترات سماح للسماح بتحويل محافظ حول التبديل.
- `pending_transition.transition_id` pour la poignée d'audit يجب على الحوكمة ذكره او الغاء الانتقالات حتى يتمكن المشغلون من ربط تقارير on/off-ramp.
- `policy_transition_window_blocks` à 720 (12 heures avec temps de bloc 60 s). تحد العقد طلبات الحوكمة التي تحاول اشعارا اقصر.
- Genesis manifeste وتدفقات CLI تعرض السياسات الحالية والمعلقة. منطق admission يقرأ السياسة وقت التنفيذ لتاكيد ان كل تعليمة سرية مصرح بها.
- قائمة تحقق الهجرة - انظر « Migration Sequencing » ادناه لخطة ترقية على مراحل يتبعها Milestone M0.

#### مراقبة الانتقالات عبر Torii

Utilisez la fonction `GET /v1/confidential/assets/{definition_id}/transitions` pour `AssetConfidentialPolicy`. La charge utile JSON est associée à l'identifiant d'actif et est associée à `current_mode`. ذلك الارتفاع (نوافذ التحويل تبلغ مؤقتا `Convertible`) et ومعرفات معلمات `vk_set_hash`/Poseidon/Pedersen المتوقعة. عند وجود انتقال حوكمة معلق يتضمن الرد ايضا:

- `transition_id` - descripteur d'audit المعاد من `ScheduleConfidentialPolicyTransition`.
-`previous_mode`/`new_mode`.
-`effective_height`.
- `conversion_window` et `window_open_height` المشتق (الكتلة التي يجب ان تبدأ فيها المحافظ التحويل لقطع ShieldedOnly).

مثال رد:

```json
{
  "asset_id": "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
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

يشير رد `404` الى عدم وجود تعريف اصل مطابق. Vous devez utiliser le modèle `pending_transition` pour `null`.

### آلة حالات السياسة| الوضع الحالي | الوضع التالي | المتطلبات | Définition de effective_height | ملاحظات |
|----------------------------------------------------|-------------------------------------------------------------------------------------------------|----------------------------------------------------------------------------------------------------------------------------------|-----------------------------------------------------------------------------------------------|
| Transparent uniquement | Cabriolet | Il s'agit d'un vérificateur/paramètre. Il s'agit de `ScheduleConfidentialPolicyTransition` ou `effective_height ≥ current_height + policy_transition_delay_blocks`. | ينفذ الانتقال بالضبط عند `effective_height`؛ يصبح piscine protégée متاحا فوريا.                   | المسار الافتراضي لتمكين السرية مع ابقاء التدفقات الشفافة.               |
| Transparent uniquement | Blindé uniquement | Il s'agit d'un `policy_transition_window_blocks ≥ 1`.                                                         | Utiliser le runtime pour `Convertible` ou `effective_height - policy_transition_window_blocks` Il s'agit de `ShieldedOnly` ou `effective_height`. | يوفر نافذة تحويل حتمية قبل تعطيل التعليمات الشفافة.   || Cabriolet | Blindé uniquement | انتقال مجدول مع `effective_height ≥ current_height + policy_transition_delay_blocks`. يجب على الحوكمة توثيق (`transparent_supply == 0`) pour les métadonnées d'audit؛ Le runtime est également disponible. | نفس دلالات النافذة كما اعلاه. اذا كان العرض الشفاف غير صفري عند `effective_height` يتم اجهاض الانتقال بـ `PolicyTransitionPrerequisiteFailed`. | يقفل الاصل في دوران سري بالكامل.                                     |
| Blindé uniquement | Cabriolet | انتقال مجدول؛ لا يوجد سحب طارئ نشط (`withdraw_height` غير مضبوط).                                    | يتبدل الوضع عند `effective_height`؛ Vous pouvez révéler des rampes et des notes protégées.                           | يستخدم لنوافذ الصيانة او مراجعات المدققين.                                          |
| Blindé uniquement | Transparent uniquement | يجب على الحوكمة اثبات `shielded_supply == 0` او تجهيز خطة `EmergencyUnshield` موقعة (تطلب تواقيع مدققين). | Utiliser le runtime `Convertible` pour `effective_height` عند الارتفاع تفشل التعليمات السرية بقوة ويعود الاصل الى وضع شفاف فقط. | خروج كملاذ اخير. يتم الغاء الانتقال تلقائيا اذا تم صرف اي note سرية خلال النافذة. |
| N'importe quel | Identique à l'actuel | `CancelConfidentialPolicyTransition` يمسح التغيير المعلق.                                                        | يتم حذف `pending_transition` فورا.                                                                          | يحافظ على الوضع الراهن؛ مذكور للاكتمال.                                             |الانتقالات غير المدرجة اعلاه ترفض عند تقديمها للحوكمة. يتحقق runtime من الشروط المسبقة قبل تطبيق الانتقال المجدول مباشرة؛ فشل الشروط يعيد الاصل الى وضعه السابق ويطلق `PolicyTransitionPrerequisiteFailed` عبر التليمتري واحداث الكتلة.

### Séquençage de la migration

1. **Préparer les registres :** فعّل كل مدخلات verifier والمعلمات المشار اليها في السياسة المستهدفة. تعلن العقد `conf_features` الناتجة حتى يتمكن peers من التحقق من التوافق.
2. **Étapez la transition :** قدّم `ScheduleConfidentialPolicyTransition` ou `effective_height` ou `policy_transition_delay_blocks`. عند الانتقال نحو `ShieldedOnly` حدد نافذة تحويل (`window ≥ policy_transition_window_blocks`).
3. **Publiez les instructions de l'opérateur :** Téléchargez le runbook `transition_id` et la rampe d'accès/sortie. تشترك المحافظ والمدققون في `/v1/confidential/assets/{id}/transitions` لمعرفة ارتفاع فتح النافذة.
4. **Application de la fenêtre :** Utilisez le runtime pour `Convertible` et `PolicyTransitionWindowOpened { transition_id }` pour créer des applications. الحوكمة المتعارضة.
5. **Finaliser ou abandonner :** Utilisez le moteur d'exécution `effective_height` pour le moment (ou le runtime). النجاح يقلب السياسة للوضع المطلوب؛ Il s'agit du `PolicyTransitionPrerequisiteFailed`, qui est en cours de réalisation.
6. **Mises à niveau du schéma :** Utilisez la ligne de commande pour télécharger la CLI (`asset_definition.v2`) et la CLI. `confidential_policy` est un manifeste manifeste. توجه وثائق ترقية genesis المشغلين لاضافة اعدادات السياسة وبصمات registre قبل اعادة تشغيل المدققين.La genèse est une histoire de la genèse. مع ذلك تتبع نفس قائمة التحقق عند تغيير الاوضاع بعد الاطلاق كي تبقى نوافذ التحويل حتمية وتمتلك المحافظ وقتا للتكيف.

### نسخ Norito manifeste et- يجب ان تتضمن Genesis manifeste `SetParameter` للمفتاح المخصص `confidential_registry_root`. La charge utile de Norito JSON est compatible avec `ConfidentialRegistryMeta { vk_set_hash: Option<String> }` : `null`) pour le vérificateur Il s'agit d'un hexadécimal de 32 bits (`0x…`) qui utilise le vérificateur de manifeste pour `compute_vk_set_hash`. ترفض العقد البدء اذا كان المعامل مفقودا او الهاش لا يطابق كتابات السجل المرمزة.
- Le manifeste `ConfidentialFeatureDigest::conf_rules_version` sur fil est disponible. La version v1 est celle de `Some(1)` et `iroha_config::parameters::defaults::confidential::RULES_VERSION`. عند تطور القواعد ارفع الثابت، اعادة توليد manifestes, واطلاق البيناريات معا؛ خلط النسخ يجعل المدققين يرفضون الكتل بـ `ConfidentialFeatureDigestMismatch`.
- ينبغي ان تجمع Activation manifestes تحديثات registre وتغييرات دورة حياة المعلمات وانتقالات السياسة بحيث يبقى digest متسقا:
  1. Créer un registre de registre (`Publish*`, `Set*Lifecycle`) pour accéder hors ligne et digérer le résumé de la connexion `compute_confidential_feature_digest`.
  2. اصدِر `SetParameter::custom(confidential_registry_root, {"vk_set_hash": "0x…"})` باستخدام الهاش المحسوب حتى يتمكن peers المتاخرون من استعادة digest الصحيح حتى اذا فاتتهم تعليمات registre الوسيطة.
  3. ارفق تعليمات `ScheduleConfidentialPolicyTransition`. يجب على كل تعليمة ان تقتبس `transition_id` الصادر من الحوكمة؛ manifeste le runtime d'exécution.
  4. احفظ بايتات manifest وبصمة SHA-256 وdigest المستخدم في خطة التفعيل. يتحقق المشغلون من الثلاثة قبل التصويت لتجنب الانقسام.- Utilisez le cut-over pour installer le système de coupure (`custom.confidential_upgrade_activation_height`). هذا يعطي المدققين دليلا مشفرا بنوريتو على ان المدققين احترموا نافذة الاشعار قبل سريان تغيير digest.

## دورة حياة vérificateur والمعلمات
### Registre ZK
- يخزن ledger `ZkVerifierEntry { vk_id, circuit_id, version, proving_system, curve, public_inputs_schema_hash, vk_hash, vk_len, max_proof_bytes, gas_schedule_id, activation_height, deprecation_height, withdraw_height, status, metadata_uri_cid, vk_bytes_cid }` ou `proving_system` ou `Halo2`.
- ازواج `(circuit_id, version)` فريدة عالميا؛ Il s'agit d'une métadonnée de métadonnées. محاولات تسجيل زوج مكرر ترفض عند admission.
- Il s'agit du code `circuit_id` et du code `public_inputs_schema_hash` (hachage Blake2b-32 pour le vérificateur). ترفض admission السجلات التي تهمل هذه الحقول.
- تعليمات الحوكمة تشمل:
  - `PUBLISH` pour les métadonnées `Proposed`.
  - `ACTIVATE { vk_id, activation_height }` لجدولة تفعيل المدخل عند حدود époque.
  - `DEPRECATE { vk_id, deprecation_height }` لتحديد اخر ارتفاع يمكن فيه للـ preuves الاشارة الى المدخل.
  - `WITHDRAW { vk_id, withdraw_height }` pour la lecture La hauteur de retrait est définie comme étant la hauteur de retrait.
- Genesis manifestes `confidential_registry_root` pour `vk_set_hash` pour les personnes qui ont besoin يتحقق التحقق من هذا digest مقابل حالة السجل المحلية قبل انضمام العقدة للاجماع.
- تسجيل او تحديث vérificateur يتطلب `gas_schedule_id`؛ يفرض التحقق ان يكون مدخل السجل `Active` et `(circuit_id, version)` et pour les preuves Halo2 `OpenVerifyEnvelope` يطابق `circuit_id` و`vk_hash` و`public_inputs_schema_hash` في سجل السجل.### Prouver les clés
- Les clés de preuve sont utilisées pour les clés adressées au contenu (`pk_cid`, `pk_hash`, `pk_len`) avec le vérificateur de métadonnées.
- Il s'agit de SDK Wallet pour PK et pour les utilisateurs.

### Paramètres de Pedersen et Poséidon
- Fichiers de vérification (`PedersenParams`, `PoseidonParams`) pour vérifier le vérificateur et `params_id`, pour المولدات/الثوابت، وارتفاعات التفعيل/الاستبدال/السحب.
- Les engagements en matière d'engagements et d'engagements `params_id` sont en rapport avec les engagements en matière d'assurance maladie. Il y a des notes d'engagement et des notes d'engagement et il y a un annulateur.

## الترتيب الحتمي et annulateurs
- يحافظ كل اصل على `CommitmentTree` ou `next_leaf_index` ; تضيف الكتل engagements بترتيب حتمي: تمر المعاملات بترتيب الكتلة؛ داخل كل معاملة تمر مخرجات blinded تصاعديا حسب `output_idx` المتسلسل.
- `note_position` مشتق من ازاحات الشجرة لكنه **ليس** جزءا من nullifier؛ يستخدم فقط لمسارات العضوية ضمن témoin de preuve.
- يضمن تصميم PRF ثبات nullifier عند reorgs؛ Le PRF `{ nk, note_preimage_hash, asset_id, chain_id, params_id }` et les ancres de Merkle sont connectés au `max_anchor_age_blocks`.## تدفق grand livre
1. **MintConfidential {asset_id, montant, destinataire_hint }**
   - يتطلب سياسة اصل `Convertible` et `ShieldedOnly` ; يتحقق admission من سلطة الاصل، يجلب `params_id` الحالي، يعين `rho`, يصدر engagement et Merkle Tree.
   - يصدر `ConfidentialEvent::Shielded` pour l'engagement, فرق Merkle root, وhash استدعاء المعاملة للتدقيق.
2. **TransferConfidential {asset_id, proof, circuit_id, version, nullifiers, new_commitments, enc_payloads, Anchor_root, memo }**
   - syscall يتحقق pour VM avec preuve en cas de problème Il y a l'hôte et les annulateurs, ainsi que les engagements, les ancres et les ancres.
   - Le grand livre utilise les `NullifierSet` et les charges utiles ainsi que les annulateurs `ConfidentialEvent::Transferred`. والمخرجات المرتبة وproof hash وMerkle racines.
3. **RevealConfidential {asset_id, preuve, circuit_id, version, nullifier, montant, destinataire_account, Anchor_root }**
   - متاحة فقط للاصول `Convertible` ; تتحقق preuve ان قيمة note تساوي المبلغ المكشوف، يضيف ledger الرصيد الشفاف ويحرق blindage note بوسم nullifier كمصروف.
   - يصدر `ConfidentialEvent::Unshielded` مع المبلغ العلني وnullifiers المستهلكة ومعرفات proof وhash استدعاء المعاملة.## Modèle de données اضافات
- `ConfidentialConfig` (قسم تهيئة جديد) avec `assume_valid`, pour le gaz/limites, l'ancre et le backend du vérificateur.
- `ConfidentialNote`, `ConfidentialTransfer` et `ConfidentialMint` pour Norito pour votre appareil (`CONFIDENTIAL_ASSET_V1 = 0x01`).
- `ConfidentialEncryptedPayload` contient les octets de mémo AEAD dans `{ version, ephemeral_pubkey, nonce, ciphertext }` et `version = CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1` pour XChaCha20-Poly1305.
- توجد متجهات اشتقاق المفاتيح القانونية في `docs/source/confidential_key_vectors.json` ; Le point de terminaison CLI وTorii est disponible pour les utilisateurs.
- يحصل `asset::AssetDefinition` à `confidential_policy: AssetConfidentialPolicy { mode, vk_set_hash, poseidon_params_id, pedersen_params_id, pending_transition }`.
- Voir `ZkAssetState` ou `(backend, name, commitment)` pour les vérificateurs de transfert/non-blindage. يرفض التنفيذ preuves التي لا تطابق vérification de la clé المسجل (مرجعا او ضمنيا).
- يتم تخزين `CommitmentTree` (pour les points de contrôle frontaliers) et `NullifierSet` pour `(chain_id, asset_id, nullifier)` et `ZkVerifierEntry` et `PedersenParams` et `PoseidonParams` dans l'état mondial.
- Utilisez mempool pour `NullifierIndex` et `AnchorIndex` pour créer une ancre.
- تحديثات مخطط Norito تشمل ترتيبًا قانونيًا لـ entrées publiques؛ اختبارات aller-retour تضمن حتمية الترميز.
- Un aller-retour pour les charges utiles chiffrées et les tests unitaires (`crates/iroha_data_model/src/confidential.rs`). ستضيف متجهات المحافظ لاحقا AEAD transcriptions قانونية للمدققين. Il s'agit d'une enveloppe filaire `norito.md`.## تكامل IVM et appel système
- L'appel système `VERIFY_CONFIDENTIAL_PROOF` est utilisé :
  - `circuit_id`, `version`, `scheme`, `public_inputs`, `proof` et `ConfidentialStateDelta { asset_id, nullifiers, commitments, enc_payloads }`.
  - Il s'agit d'un vérificateur de métadonnées utilisé par syscall pour une preuve de delta de gaz et de delta.
- Pour le trait d'hôte en lecture seule `ConfidentialLedger` pour les instantanés et pour Merkle et l'annulateur. J'utilise les assistants Kotodama pour le témoin et le schéma.
- J'utilise le pointeur-ABI pour la mise en page de tampon de preuve et le registre.

## تفاوض قدرات العقد
- Pour la poignée de main `feature_bits.confidential` ou `ConfidentialFeatureDigest { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`. Utilisez `confidential.enabled=true` et `assume_valid=false` pour le backend du vérificateur et le digest. La poignée de main est prise en charge par `HandshakeConfidentialMismatch`.
- يدعم config `assume_valid` pour les observateurs: عند تعطيله، تؤدي تعليمات السرية الى `UnsupportedInstruction` حتمي بلا panic؛ Il y a des observateurs qui sont des preuves.
- يرفض mempool المعاملات السرية اذا كانت القدرة المحلية معطلة. غير متوافقة بينما تعيد توجيه معرفات verifier غير المعروفة بشكل اعمى ضمن حدود الحجم.

### مصفوفة توافق poignée de main| اعلان الطرف البعيد | النتيجة لعقد المدققين | ملاحظات المشغل |
|----------------------|----------------------------------|----------------|
| `enabled=true`, `assume_valid=false`, backend, résumé, | مقبول | Il s'agit de la proposition de vote et de diffusion de RBC. لا يتطلب تدخل يدوي. |
| `enabled=true`, `assume_valid=false`, backend متطابق, digest قديم او مفقود | مرفوض (`HandshakeConfidentialMismatch`) | يجب على الطرف البعيد تطبيق تفعيل السجلات/المعلمات المعلقة او انتظار `activation_height` المجدولة. حتى التصحيح يبقى قابلا للاكتشاف لكنه لا يدخل دوران الاجماع. |
| `enabled=true`, `assume_valid=true` | مرفوض (`HandshakeConfidentialMismatch`) | يتطلب المدققون تحقق preuves؛ قم بضبط الطرف البعيد كمراقب عبر Torii فقط او اجعل `assume_valid=false` بعد تمكين التحقق الكامل. |
| `enabled=false`, poignée de main (pour vous) et backend du vérificateur | مرفوض (`HandshakeConfidentialMismatch`) | pairs قديمة او محدثة جزئيا لا يمكنها الانضمام لشبكة الاجماع. Il s'agit d'un backend + digest basé sur la fonction backend. |

العقد المراقِبة التي تتجاوز تحقق proofs عمدا يجب الا تفتح اتصالات اجماع مع مدققين يعملون ببوابات قدرات. يمكنها استيعاب الكتل عبر Torii او واجهات الارشفة، لكن شبكة الاجماع ترفضها حتى تعلن قدرات متوافقة.

### سياسة تقليم Reveal et Nullifier

يجب على دفاتر السرية الاحتفاظ بتاريخ كاف لاثبات حداثة notes واعادة تشغيل تدقيقات الحوكمة. La description de l'article `ConfidentialLedger` est :- **Rétention des nullificateurs :** Annulateurs activés *ادنى* `730` aujourd'hui (24 janvier) pour la mise à jour et la suppression des annulations اذا فرضها المنظم. يمكن للمشغلين تمديدها عبر `confidential.retention.nullifier_days`. يجب ان تبقى nullifiers ضمن النافذة قابلة للاستعلام عبر Torii حتى يتمكن المدققون من اثبات عدم وجود double dépense.
- **Révéler l'élagage :** تقوم Révèle la taille (`RevealConfidential`) pour les engagements المرتبطة فورا بعد اكتمال الكتلة، لكن nullifier المصروف يبقى خاضعا لقاعدة الاحتفاظ. La méthode révèle (`ConfidentialEvent::Unshielded`) la preuve de hachage et la preuve de hachage pour le texte chiffré.
- **Points de contrôle frontaliers :** تحافظ engagements frontaliers على points de contrôle متحركة تغطي الاكبر من `max_anchor_age_blocks` ونافذة الاحتفاظ. Il y a des points de contrôle comme des nullificateurs et des nullificateurs.
- **Remédiation du résumé obsolète :** اذا رُفع `HandshakeConfidentialMismatch` بسبب انحراف digest, يجب على المشغلين (1) التحقق من تطابق نوافذ الاحتفاظ عبر الكتلة، (2) تشغيل `iroha_cli app confidential verify-ledger` لاعادة توليد digest مقابل مجموعة nullifier المحتفظ بها، و(3) اعادة نشر manifest المحدث. Il y a des nullificateurs qui sont plus utiles que les nullificateurs.

وثق التعديلات المحلية في Operations Runbook؛ سياسات الحوكمة التي تمد نافذة الاحتفاظ يجبان تحدث تهيئة العقد وخطط التخزين الارشيفي بالتزامن.

### تدفق الاخلاء والاستعادة1. اثناء الاتصال، يقارن `IrohaNetwork` القدرات المعلنة. اي عدم تطابق يرفع `HandshakeConfidentialMismatch`؛ Il s'agit d'un homologue dans la file d'attente de découverte avec `Ready`.
2. Créer un lien vers le serveur (avec résumé et backend vers le serveur) et Sumeragi vers un homologue او التصويت.
3. Le vérificateur et le vérificateur (`vk_set_hash`, `pedersen_params_id`, `poseidon_params_id`) et Utilisez `next_conf_features` pour `activation_height`. عندما يتطابق digest ينجح handshake التالي تلقائيا.
4. اذا تمكن peer قديم من بث كتلة (مثلا عبر archivage replay)، يرفضها المدققون حتميا بـ `BlockRejectionReason::ConfidentialFeatureDigestMismatch` للحفاظ على اتساق ledger عبر الشبكة.

### Flux de négociation sécurisé pour la relecture1. Sélectionnez l'option Noise/X25519 pour utiliser Noise/X25519. Charge utile de prise de contact (`handshake_signature_payload`) Prise en charge de la charge utile de prise de contact et prise en charge du socket ومعرف السلسلة عند التجميع مع `handshake_chain_id`. يتم تشفير الرسالة بـ AEAD قبل ارسالها.
2. La charge utile de la charge utile est transférée vers un réseau homologue/local avec l'aide de Ed25519 dans `HandshakeHelloV1`. بما ان كلا المفتاحين المؤقتين والعنوان المعلن ضمن نطاق التوقيع، فاعادة تشغيل رسالة ملتقطة ضد peer اخرى او استعادة اتصال قديم تفشل حتميا.
3. Utilisez le lien `ConfidentialFeatureDigest` pour `HandshakeConfidentialMeta`. Utiliser le tuple `{ enabled, assume_valid, verifier_backend, digest }` pour `ConfidentialHandshakeCaps` Il s'agit d'une poignée de main selon `HandshakeConfidentialMismatch` pour la prise de contact `Ready`.
4. يجب على المشغلين اعادة حساب digest (عبر `compute_confidential_feature_digest`) et تشغيل العقد بسياسات/سجلات محدثة قبل اعادة الاتصال. peers التي تعلن digests قديمة تستمر في الفشل، مانعة دخول حالة قديمة الى مجموعة المدققين.
5. Poignées de main et poignées de main `iroha_p2p::peer` (`handshake_failure_count`) et les clés معرف peer البعيد وبصمة digest. Il est possible de rejouer la relecture et de déployer le déploiement.## ادارة المفاتيح et charges utiles
- تسلسل اشتقاق المفاتيح لكل حساب:
  - `sk_spend` → `nk` (clé d'annulation) ، `ivk` (clé de visualisation entrante) ، `ovk` (clé de visualisation sortante) ، `fvk`.
- Notes sur les charges utiles de l'AEAD et de l'ECDH؛ Il s'agit des clés de vue de l'auditeur et des sorties.
- Lignes CLI : `confidential create-keys`, `confidential send`, `confidential export-view-key`, fonctions pour les mémos et `iroha app zk envelope`. Les enveloppes Norito sont disponibles. يعرض
- جدول gaz حتمي:
  - Halo2 (Plonkish) : gaz `250_000` + gaz `2_000` pour la contribution du public.
  - `5` gaz pour la preuve, pour l'annulation (`300`) et l'engagement (`500`).
  - يمكن للمشغلين تجاوز هذه الثوابت عبر تهيئة العقد (`confidential.gas.{proof_base, per_public_input, per_proof_byte, per_nullifier, per_commitment}`)؛ تنتشر التغييرات عند البدء او اعادة تحميل التهيئة وتطبق حتميا عبر الكتلة.
- حدود صارمة (افتراضات قابلة للضبط):
-`max_proof_size_bytes = 262_144`.
- `max_nullifiers_per_tx = 8`, `max_commitments_per_tx = 8`, `max_confidential_ops_per_block = 256`.
- `verify_timeout_ms = 750`, `max_anchor_age_blocks = 10_000`. preuves التي تتجاوز `verify_timeout_ms` تقطع التعليمة حتميا (تصويتات الحوكمة تصدر `proof verification exceeded timeout` و`VerifyProof` يعيد خطا).
- Exemples de constructeurs : `max_proof_bytes_block`, `max_verify_calls_per_tx`, `max_verify_calls_per_block` et `max_public_inputs`. `reorg_depth_bound` (≥ `max_anchor_age_blocks`) يتحكم في احتفاظ postes de contrôle frontaliers.
- يرفض runtime المعاملات التي تتجاوز هذه الحدود لكل معاملة او لكل كتلة، ويصدر اخطاء `InvalidParameter` حتمية مع ابقاء حالة ledger دون تغيير.
- يرشح mempool المعاملات السرية مسبقا حسب `vk_id` et proof وعمر Anchor قبل استدعاء Verifier للحفاظ على حدود الموارد.
- يتوقف التحقق حتميا عند timeout او تجاوز الحدود؛ تفشل المعاملات باخطاء واضحة. backends SIMD est utilisé pour le gaz.

### خطوط اساس المعايرة وبوابات القبول
- **Plateformes de référence.** يجب ان تغطي معايرات الاداء ثلاث ملفات عتاد ادناه. اي معايرة لا تلتقط جميع الملفات ترفض اثناء المراجعة.| الملف | المعمارية | Processeur/Instance | اعلام المترجم | الغاية |
  | --- | --- | --- | --- | --- |
  | `baseline-simd-neutral` | `x86_64` | AMD EPYC 7B12 (32c) او Intel Xeon Gold 6430 (24c) | `RUSTFLAGS="-C target-feature=-avx,-avx2,-fma"` | تأسيس قيم ارضية بدون تعليمات متجهة؛ يستخدم لضبط جداول تكلفة repli. |
  | `baseline-avx2` | `x86_64` | Intel Xeon Or 6430 (24c) | version par défaut | يتحقق من مسار AVX2؛ يفحص ان تسريعات SIMD ضمن تسامح gas المحايد. |
  | `baseline-neon` | `aarch64` | AWS Graviton3 (c7g.4xlarge) | version par défaut | Le backend NEON est compatible avec x86. |

- **Harnais de référence.** يجب انتاج كل تقارير معايرة الغاز باستخدام :
  -`CRITERION_HOME=target/criterion cargo bench -p iroha_core isi_gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>`
  - `cargo test -p iroha_core bench_repro -- --ignored` pour luminaire.
  - `CRITERION_HOME=target/criterion cargo bench -p ivm gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>` pour l'opcode VM.

- ** Caractère aléatoire fixe. ** صدّر `IROHA_CONF_GAS_SEED=conf-gas-seed-2026Q1` قبل تشغيل البنش حتى يتحول `iroha_test_samples::gen_account_in` الى مسار `KeyPair::from_seed` الحتمي. harnais `IROHA_CONF_GAS_SEED_ACTIVE=…` مرة واحدة؛ اذا غاب المتغير يجب ان تفشل المراجعة. يجب على اي ادوات معايرة جديدة احترام هذا المتغير عند ادخال عشوائية اضافية.

- **Capture des résultats.**
  - ارفع Résumés des critères (`target/criterion/**/raw.csv`) pour l'artefact.
  - خزّن المقاييس المشتقة (`ns/op`, `gas/op`, `ns/gas`) dans [Confidential Gas Calibration ledger](./confidential-gas-calibration) avec git commit ونسخة المترجم المستخدمة.
  - احتفظ بآخر baseline اثنين لكل ملف؛ احذف اللقطات الاقدم بعد اعتماد التقرير الاحدث.- **Tolérances d'acceptation.**
  - يجب ان تبقى فروقات الغاز بين `baseline-simd-neutral` و`baseline-avx2` ≤ ±1,5%.
  - يجب ان تبقى فروقات الغاز بين `baseline-simd-neutral` و`baseline-neon` ≤ ±2,0%.
  - Les liens entre les réseaux et les RFC sont pris en charge par les RFC et les réseaux.

- **Liste de contrôle de révision.** يتحمل مقدمو الطلب مسؤولية :
  - Utilisez `uname -a` et `/proc/cpuinfo` (modèle, pas à pas) et `rustc -Vv` pour votre appareil.
  - Il s'agit du `IROHA_CONF_GAS_SEED` dans la graine (de la graine).
  - Il s'agit d'un stimulateur cardiaque et d'un vérificateur confidentiel en production (`--features confidential,telemetry` pour la télémétrie).

## التهيئة والعمليات
- Pour `iroha_config` par rapport à `[confidential]` :
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
- Numéros de téléphone suivants : `confidential_proof_verified`, `confidential_verifier_latency_ms`, `confidential_proof_bytes_total`, `confidential_nullifier_spent`, `confidential_commitments_appended`, `confidential_mempool_rejected_total{reason}` et `confidential_policy_transitions_total` sont des textes en clair.
- Par RPC :
  -`GET /confidential/capabilities`
  -`GET /confidential/zk_registry`
  -`GET /confidential/params`## استراتيجية الاختبار
- Éléments : Les éléments suivants sont associés aux ensembles d'annulation de racines Merkle.
- تحمل reorg: محاكاة reorg متعدد الكتل مع ancres؛ تبقى nullifiers مستقرة وترفض ancres القديمة.
- ثوابت الغاز: تاكد من نفس استخدام الغاز عبر عقد مع او بدون تسريع SIMD.
- اختبار الحدود: preuves عند سقوف الحجم/الغاز، الحد الاقصى للمدخلات/المخرجات، وتطبيق timeout.
- دورة الحياة: عمليات الحوكمة لتفعيل/استبدال verifier والمعلمات، واختبارات صرف الدوران.
- Politique FSM : Politique de transition en attente, de mempool et de hauteurs effectives.
- طوارئ السجل: سحب طارئ يجمد الاصول المتاثرة عند `withdraw_height` et preuves بعدها.
- Gating de capacité : Nom de l'utilisateur `conf_features` pour le produit observateurs مع `assume_valid=true` يواكبون دون التأثير على الاجماع.
- Fonctions : est un validateur/complet/observateur qui contient des racines d'état et des racines d'état.
- Fuzzing négatif : preuves des charges utiles et du nullificateur.## الهجرة والتوافق
- طرح محكوم بالميزة: حتى اكتمال Phase C3, `enabled` افتراضيه `false`؛ تعلن العقد قدراتها قبل الانضمام الى مجموعة المدققين.
- الاصول الشفافة غير متاثرة؛ التعليمات السرية تتطلب ادخالات السجل والتفاوض على القدرات.
- العقد المترجمة دون دعم السرية ترفض الكتل المعنية حتميا؛ Pour que le problème se produise, vous devez contacter `assume_valid=true`.
- Genesis manifeste des manifestations de la nature et de la nature.
- يتبع المشغلون runbooks المنشورة لتدوير السجل وانتقالات السياسة والسحب الطارئ للحفاظ على ترقية حتمية.

## اعمال متبقية
- Vous avez utilisé Halo2 (recherche de données) et le playbook d'étalonnage est également disponible. gas/timeout مع تحديث `confidential_assets_calibration.md` القادم.
- Les options de visualisation sélective et de visualisation sélective sont également disponibles pour Torii. مسودة الحوكمة.
- Vous pouvez utiliser le chiffrement des témoins et les éléments d'enveloppe du SDK.
- طلب مراجعة امنية خارجية للدوائر والسجلات واجراءات تدوير المعلمات وارشفة النتائج بجانب تقارير التدقيق الداخلية.
- L'API de gestion des dépenses et la clé de vue sont également utilisées pour les dépenses.## مراحل التنفيذ
1. **Phase M0 — Durcissement d'arrêt du navire**
   - ✅ اشتقاق nullifier يتبع الان تصميم Poseidon PRF (`nk`, `rho`, `asset_id`, `chain_id`) pour les engagements الحتمي في تحديثات grand livre.
   - ✅ يفرض التنفيذ حدود حجم preuves وحصص العمليات السرية لكل معاملة/كتلة، رافضا المعاملات المتجاوزة باخطاء حتمية.
   - ✅ يعلن Poignée de main P2P `ConfidentialFeatureDigest` (backend digest + بصمات السجل) et عدم التطابق حتميا عبر `HandshakeConfidentialMismatch`.
   - ✅ ازالة paniques في مسارات تنفيذ السرية واضافة role gating للعقد غير المتوافقة.
   - ⚪ فرض ميزانيات timeout للـ vérificateur وحدود عمق réorganisation للـ points de contrôle frontaliers.
     - ✅ تطبيق ميزانيات timeout؛ preuves التي تتجاوز `verify_timeout_ms` تفشل الان حتميا.
     - ✅ احترام `reorg_depth_bound` et points de contrôle الاقدم من النافذة مع الحفاظ على instantanés حتمية.
   - تقديم `AssetConfidentialPolicy` وpolicy FSM وبوابات application لتعليمات mint/transfer/reveal.
   - Fichier `conf_features` dans le registre/paramètre de résumé de résumé.
2. **Phase M1 — Registres et paramètres**
   - شحن سجلات `ZkVerifierEntry` و`PedersenParams` و`PoseidonParams` مع عمليات حوكمة ومرساة Genesis وادارة الكاش.
   - Permet d'appeler syscall pour les recherches dans le registre, les identifiants de planification de gaz, le hachage de schéma et les tâches.
   - La charge utile de la version v1 est compatible avec les fonctionnalités de la CLI et la CLI.
3. **Phase M2 — Gaz et performances**- Utiliser le gas pour vérifier les preuves et utiliser le pool de mémoire (vérifier la latence).
   - Points de contrôle CommitmentTree et chargement LRU et indices d'annulation pour les applications.
4. **Phase M3 — Outillage de rotation et de portefeuille**
   - تمكين قبول preuves متعددة المعلمات والنسخ؛ L'activation/dépréciation est désormais compatible avec les runbooks.
   - Vous pouvez utiliser le SDK/CLI du portefeuille et utiliser les dépenses.
5. **Phase M4 — Audit et opérations**
   - Il y a des informations sur la divulgation sélective et les runbooks.
   - جدولة مراجعة تشفير/امن خارجية ونشر النتائج في `status.md`.

Il s'agit de la feuille de route et des informations sur la feuille de route.
<!-- Auto-generated stub for Arabic (ar) translation. Replace this content with the full translation. -->

---
lang: ar
direction: rtl
source: docs/source/soracloud/manifest_schemas.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a0724ed92da90d8d78a7095b5fc75523ea5dd4a1c885059e28c1bbb8118d1f8c
source_last_modified: "2026-03-26T06:12:11.480497+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# مخططات بيان SoraCloud V1

تحدد هذه الصفحة أول مخططات Norito الحتمية لـ SoraCloud
النشر على Iroha 3:

-`SoraContainerManifestV1`
-`SoraServiceManifestV1`
-`SoraStateBindingV1`
-`SoraDeploymentBundleV1`
-`AgentApartmentManifestV1`
-`FheParamSetV1`
-`FheExecutionPolicyV1`
-`FheGovernanceBundleV1`
-`FheJobSpecV1`
-`DecryptionAuthorityPolicyV1`
-`DecryptionRequestV1`
-`CiphertextQuerySpecV1`
-`CiphertextQueryResponseV1`
-`SecretEnvelopeV1`
-`CiphertextStateRecordV1`

تعريفات الصدأ موجودة في `crates/iroha_data_model/src/soracloud.rs`.

تعد سجلات وقت التشغيل الخاصة للنموذج الذي تم تحميله بمثابة طبقة منفصلة عن عمد
تظهر بيانات نشر SCR هذه. ينبغي عليهم تمديد طائرة نموذج Soracloud
وإعادة استخدام `SecretEnvelopeV1` / `CiphertextStateRecordV1` للبايتات المشفرة
والحالة الأصلية للنص المشفر، بدلاً من تشفيرها كخدمة/حاوية جديدة
يتجلى. انظر `uploaded_private_models.md`.

## النطاق

تم تصميم هذه البيانات لـ `IVM` + Sora Container Runtime المخصص
اتجاه (SCR) (لا يوجد WASM، ولا توجد تبعية Docker في قبول وقت التشغيل).- يلتقط `SoraContainerManifestV1` هوية الحزمة القابلة للتنفيذ، ونوع وقت التشغيل،
  سياسة القدرة والموارد وإعدادات اختبار دورة الحياة والصريحة
  يتم تصدير ملف التكوين المطلوب إلى بيئة وقت التشغيل أو المراجعة المُثبتة
  شجرة.
- يلتقط `SoraServiceManifestV1` هدف النشر: هوية الخدمة،
  تجزئة/إصدار بيان الحاوية المشار إليه، والتوجيه، وسياسة الطرح، و
  روابط الدولة.
- يلتقط `SoraStateBindingV1` نطاق وحدود كتابة الحالة الحتمية
  (بادئة مساحة الاسم، وضع قابلية التغيير، وضع التشفير، حصص العناصر/الإجمالي).
- `SoraDeploymentBundleV1` حاوية الأزواج + بيانات الخدمة وإنفاذها
  فحوصات القبول الحتمية (ربط التجزئة الواضحة، ومحاذاة المخطط، و
  القدرة / الاتساق الملزم).
- يلتقط `AgentApartmentManifestV1` سياسة وقت تشغيل الوكيل المستمر:
  الحدود القصوى للأدوات، والحدود القصوى للسياسة، وحدود الإنفاق، وحصة الولاية، وخروج الشبكة، و
  سلوك الترقية.
- يلتقط `FheParamSetV1` مجموعات معلمات FHE المُدارة بواسطة الإدارة:
  معرفات الواجهة الخلفية/المخطط الحتمية، ملف تعريف المعامل، الأمان/العمق
  الحدود وارتفاعات دورة الحياة (`activation`/`deprecation`/`withdraw`).
- يلتقط `FheExecutionPolicyV1` حدود تنفيذ النص المشفر الحتمية:
  أحجام الحمولة المقبولة، ومروحة الإدخال/الإخراج، وأغطية العمق/الدوران/التمهيد،
  ووضع التقريب الكنسي.
- `FheGovernanceBundleV1` يجمع بين مجموعة المعلمات وسياسة الحتمية
  التحقق من صحة القبول.- يلتقط `FheJobSpecV1` قبول/تنفيذ مهمة النص المشفر الحتمي
  الطلبات: فئة التشغيل، والتزامات الإدخال المطلوبة، ومفتاح الإخراج، والمحدودة
  طلب العمق/التدوير/التمهيد مرتبط بسياسة + مجموعة معلمات.
- `DecryptionAuthorityPolicyV1` يلتقط سياسة الإفصاح المُدارة بواسطة الحوكمة:
  وضع السلطة (خدمة العميل مقابل خدمة العتبة)، النصاب القانوني/الأعضاء المعتمدين،
  بدل كسر الزجاج، ووضع علامات على الولاية القضائية، ومتطلبات إثبات الموافقة،
  حدود TTL وعلامات التدقيق الأساسية.
- `DecryptionRequestV1` يلتقط محاولات الكشف المرتبطة بالسياسة:
  مرجع مفتاح النص المشفر (`binding_name` + `state_key` + الالتزام)،
  التبرير، علامة الاختصاص القضائي، تجزئة أدلة الموافقة الاختيارية، TTL،
  نية/سبب كسر الزجاج، وربط تجزئة الحوكمة.
- `CiphertextQuerySpecV1` يلتقط غرض الاستعلام المحدد للنص المشفر فقط:
  نطاق الخدمة/الربط، مرشح البادئة الرئيسية، حد النتائج المحدود، البيانات الوصفية
  مستوى الإسقاط، وتبديل إدراج إثبات.
- يلتقط `CiphertextQueryResponseV1` مخرجات الاستعلام ذات الكشف المصغر:
  المراجع الرئيسية الموجهة نحو الملخص، وبيانات تعريف النص المشفر، وإثباتات التضمين الاختيارية،
  وسياق الاقتطاع/التسلسل على مستوى الاستجابة.
- يلتقط `SecretEnvelopeV1` مادة الحمولة المشفرة نفسها:
  وضع التشفير، ومعرف/إصدار المفتاح، والرقم، وبايت النص المشفر، و
  التزامات النزاهة.
- يلتقط `CiphertextStateRecordV1` إدخالات الحالة الأصلية للنص المشفردمج البيانات الوصفية العامة (نوع المحتوى، وعلامات السياسة، والالتزام، وحجم الحمولة)
  مع `SecretEnvelopeV1`.
- يجب أن تعتمد حزم النماذج الخاصة التي تم تحميلها بواسطة المستخدم على النص المشفر الأصلي
  السجلات:
  تعيش قطع الوزن/التكوين/المعالج المشفرة في الحالة، أثناء تسجيل النموذج،
  يبقى نسب الوزن وتجميع الملفات الشخصية وجلسات الاستدلال ونقاط التفتيش
  سجلات Soracloud من الدرجة الأولى.

## الإصدار

-`SORA_CONTAINER_MANIFEST_VERSION_V1 = 1`
-`SORA_SERVICE_MANIFEST_VERSION_V1 = 1`
-`SORA_STATE_BINDING_VERSION_V1 = 1`
-`SORA_DEPLOYMENT_BUNDLE_VERSION_V1 = 1`
-`AGENT_APARTMENT_MANIFEST_VERSION_V1 = 1`
-`FHE_PARAM_SET_VERSION_V1 = 1`
-`FHE_EXECUTION_POLICY_VERSION_V1 = 1`
-`FHE_GOVERNANCE_BUNDLE_VERSION_V1 = 1`
-`FHE_JOB_SPEC_VERSION_V1 = 1`
-`DECRYPTION_AUTHORITY_POLICY_VERSION_V1 = 1`
-`DECRYPTION_REQUEST_VERSION_V1 = 1`
-`CIPHERTEXT_QUERY_SPEC_VERSION_V1 = 1`
-`CIPHERTEXT_QUERY_RESPONSE_VERSION_V1 = 1`
-`CIPHERTEXT_QUERY_PROOF_VERSION_V1 = 1`
-`SECRET_ENVELOPE_VERSION_V1 = 1`
-`CIPHERTEXT_STATE_RECORD_VERSION_V1 = 1`

التحقق من الصحة يرفض الإصدارات غير المدعومة مع
`SoraCloudManifestError::UnsupportedVersion`.

## قواعد التحقق الحتمية (V1)- بيان الحاوية:
  - يجب أن يكون `bundle_path` و`entrypoint` فارغين.
  - `healthcheck_path` (إذا تم تعيينه) يجب أن يبدأ بـ `/`.
  - قد يشير `config_exports` إلى التكوينات المعلنة فقط
    `required_config_names`.
  - يجب أن تستخدم أهداف env config-export أسماء متغيرات البيئة الأساسية
    (`[A-Za-z_][A-Za-z0-9_]*`).
  - يجب أن تظل أهداف ملف التصدير والتكوين نسبية، وأن تستخدم الفواصل `/`، و
    يجب ألا يحتوي على مقاطع `.` أو `..` فارغة.
  - يجب ألا تستهدف عمليات تصدير التكوين نفس env var أو مسار الملف النسبي أكثر
    من مرة.
- بيان الخدمة:
  - يجب أن يكون `service_version` غير فارغ.
  - يجب أن يتطابق `container.expected_schema_version` مع مخطط الحاوية v1.
  - `rollout.canary_percent` يجب أن يكون `0..=100`.
  - `route.path_prefix` (إذا تم تعيينه) يجب أن يبدأ بـ `/`.
  - يجب أن تكون أسماء ربط الحالة فريدة.
- ملزمة الدولة:
  - يجب أن يكون الرقم `key_prefix` غير فارغ وأن يبدأ بـ `/`.
  -`max_item_bytes <= max_total_bytes`.
  - لا يمكن لروابط `ConfidentialState` استخدام تشفير النص العادي.
- حزمة النشر:
  - يجب أن يتطابق `service.container.manifest_hash` مع التشفير الأساسي
    تجزئة بيان الحاوية.
  - يجب أن يتطابق `service.container.expected_schema_version` مع مخطط الحاوية.
  - تتطلب روابط الحالة القابلة للتغيير `container.capabilities.allow_state_writes=true`.
  - تتطلب الطرق العامة رقم `container.lifecycle.healthcheck_path`.
- بيان شقة الوكيل:
  - يجب أن يتطابق `container.expected_schema_version` مع مخطط الحاوية v1.
  - يجب أن تكون أسماء إمكانيات الأداة غير فارغة وفريدة من نوعها.- يجب أن تكون أسماء إمكانات السياسة فريدة.
  - يجب أن تكون أصول حد الإنفاق غير فارغة وفريدة من نوعها.
  - `max_per_tx_nanos <= max_per_day_nanos` لكل حد إنفاق.
  - يجب أن تتضمن سياسة الشبكة في القائمة المسموح بها مضيفين فريدين وغير فارغين.
- مجموعة معلمات FHE:
  - يجب أن يكون `backend` و`ciphertext_modulus_bits` فارغين.
  - يجب أن يكون حجم كل بت لمعامل النص المشفر ضمن `2..=120`.
  - يجب أن يكون ترتيب سلسلة معامل النص المشفر غير متزايد.
  - يجب أن يكون `plaintext_modulus_bits` أصغر من أكبر معامل النص المشفر.
  -`slot_count <= polynomial_modulus_degree`.
  -`max_multiplicative_depth < ciphertext_modulus_bits.len()`.
  - يجب أن يكون ترتيب ارتفاع دورة الحياة صارمًا:
    `activation < deprecation < withdraw` عند وجوده.
  - متطلبات حالة دورة الحياة:
    - `Proposed` لا يسمح بارتفاعات الإهمال/السحب.
    - يتطلب `Active` `activation_height`.
    - يتطلب `Deprecated` `activation_height` + `deprecation_height`.
    - يتطلب `Withdrawn` `activation_height` + `withdraw_height`.
- سياسة التنفيذ FHE:
  -`max_plaintext_bytes <= max_ciphertext_bytes`.
  -`max_output_ciphertexts <= max_input_ciphertexts`.
  - يجب أن يتطابق ربط مجموعة المعلمات مع `(param_set, version)`.
  - يجب ألا يتجاوز `max_multiplication_depth` عمق مجموعة المعلمات.
  - يرفض قبول السياسة دورة حياة مجموعة المعلمات `Proposed` أو `Withdrawn`.
- حزمة حوكمة FHE:
  - التحقق من صحة توافق السياسة + مجموعة المعلمات كحمولة قبول حتمية واحدة.
- مواصفات وظيفة FHE:
  - يجب أن يكون `job_id` و`output_state_key` فارغين (`output_state_key` يبدأ بـ `/`).- يجب أن تكون مجموعة الإدخال غير فارغة ويجب أن تكون مفاتيح الإدخال عبارة عن مسارات أساسية فريدة.
  - القيود الخاصة بالعملية صارمة (`Add`/`Multiply` متعدد المدخلات،
    `RotateLeft`/`Bootstrap` مدخل واحد، مع مقابض عمق/دوران/تمهيد متبادلة).
  - يفرض القبول المرتبط بالسياسة ما يلي:
    - تطابق معرفات السياسة/المعلمات والإصدارات.
    - عدد/البايتات المدخلة، والعمق، والتدوير، وحدود التمهيد تقع ضمن الحدود القصوى للسياسة.
    - تتوافق بايتات الإخراج المتوقعة الحتمية مع حدود النص المشفر للسياسة.
- سياسة سلطة فك التشفير:
  - يجب أن يكون `approver_ids` غير فارغ، وفريدًا، ومرتبًا بشكل صارم معجميًا.
  - يتطلب وضع `ClientHeld` معتمدًا واحدًا بالضبط، `approver_quorum=1`،
    و`allow_break_glass=false`.
  - يتطلب وضع `ThresholdService` اثنين على الأقل من المعتمدين و
    `approver_quorum <= approver_ids.len()`.
  - يجب أن يكون `jurisdiction_tag` غير فارغ ويجب ألا يحتوي على أحرف تحكم.
  - يجب أن يكون `audit_tag` غير فارغ ويجب ألا يحتوي على أحرف تحكم.
- طلب فك التشفير:
  - يجب أن تكون `request_id` و`state_key` و`justification` فارغة
    (`state_key` يبدأ بـ `/`).
  - يجب أن يكون `jurisdiction_tag` غير فارغ ويجب ألا يحتوي على أحرف تحكم.
  - مطلوب `break_glass_reason` عندما يكون `break_glass=true` ويجب حذفه عندما
    `break_glass=false`.
  - القبول المرتبط بالسياسة يفرض المساواة في اسم السياسة، ولا يطلب TTLتتجاوز `policy.max_ttl_blocks`، المساواة في علامة الاختصاص القضائي، كسر الزجاج
    النابضة، ومتطلبات أدلة الموافقة عندما
    `policy.require_consent_evidence=true` للطلبات غير القابلة للكسر.
- مواصفات استعلام النص المشفر:
  - يجب أن يكون `state_key_prefix` غير فارغ وأن يبدأ بـ `/`.
  - `max_results` محدد بشكل حتمي (`<=256`).
  - إسقاط البيانات التعريفية صريح (`Minimal` ملخص فقط مقابل `Standard` مرئي).
- الرد على استعلام النص المشفر:
  - `result_count` يجب أن يساوي عدد الصفوف التسلسلية.
  - يجب ألا يعرض الإسقاط `Minimal` `state_key`؛ يجب أن يكشفه `Standard`.
  - يجب ألا تظهر الصفوف مطلقًا في وضع تشفير النص العادي.
  - يجب أن تتضمن إثباتات التضمين (في حالة وجودها) معرفات مخطط غير فارغة و
    `anchor_sequence >= event_sequence`.
- المظروف السري:
  - يجب أن تكون `key_id` و`nonce` و`ciphertext` فارغة.
  - طول nonce محدد (`<=256` بايت).
  - طول النص المشفر محدد (`<=33554432` بايت).
- سجل حالة النص المشفر:
  - يجب أن يكون `state_key` غير فارغ وأن يبدأ بـ `/`.
  - يجب أن يكون نوع محتوى البيانات التعريفية غير فارغ؛ يجب أن تكون العلامات عبارة عن سلاسل فريدة غير فارغة.
  - `metadata.payload_bytes` يجب أن يساوي `secret.ciphertext.len()`.
  - `metadata.commitment` يجب أن يساوي `secret.commitment`.

## التركيبات الكنسي

يتم تخزين تركيبات Canonical JSON في:-`fixtures/soracloud/sora_container_manifest_v1.json`
-`fixtures/soracloud/sora_service_manifest_v1.json`
-`fixtures/soracloud/sora_state_binding_v1.json`
-`fixtures/soracloud/sora_deployment_bundle_v1.json`
-`fixtures/soracloud/agent_apartment_manifest_v1.json`
-`fixtures/soracloud/fhe_param_set_v1.json`
-`fixtures/soracloud/fhe_execution_policy_v1.json`
-`fixtures/soracloud/fhe_governance_bundle_v1.json`
-`fixtures/soracloud/fhe_job_spec_v1.json`
-`fixtures/soracloud/decryption_authority_policy_v1.json`
-`fixtures/soracloud/decryption_request_v1.json`
-`fixtures/soracloud/ciphertext_query_spec_v1.json`
-`fixtures/soracloud/ciphertext_query_response_v1.json`
-`fixtures/soracloud/secret_envelope_v1.json`
-`fixtures/soracloud/ciphertext_state_record_v1.json`

اختبارات التركيبات/ذهابًا وإيابًا:

-`crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs`
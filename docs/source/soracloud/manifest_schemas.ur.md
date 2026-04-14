<!-- Auto-generated stub for Urdu (ur) translation. Replace this content with the full translation. -->

---
lang: ur
direction: rtl
source: docs/source/soracloud/manifest_schemas.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a0724ed92da90d8d78a7095b5fc75523ea5dd4a1c885059e28c1bbb8118d1f8c
source_last_modified: "2026-03-26T06:12:11.480497+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# SoraCloud V1 مینی فیسٹ اسکیماس

یہ صفحہ SoraCloud کے لیے پہلی تعییناتی Norito اسکیموں کی وضاحت کرتا ہے۔
Iroha 3 پر تعیناتی:

- `SoraContainerManifestV1`
- `SoraServiceManifestV1`
- `SoraStateBindingV1`
- `SoraDeploymentBundleV1`
- `AgentApartmentManifestV1`
- `FheParamSetV1`
- `FheExecutionPolicyV1`
- `FheGovernanceBundleV1`
- `FheJobSpecV1`
- `DecryptionAuthorityPolicyV1`
- `DecryptionRequestV1`
- `CiphertextQuerySpecV1`
- `CiphertextQueryResponseV1`
- `SecretEnvelopeV1`
- `CiphertextStateRecordV1`

زنگ کی تعریفیں `crates/iroha_data_model/src/soracloud.rs` میں رہتی ہیں۔

اپ لوڈ کردہ ماڈل پرائیویٹ رن ٹائم ریکارڈز جان بوجھ کر ایک الگ پرت ہیں۔
یہ SCR تعیناتی ظاہر کرتی ہے۔ انہیں سوراکاؤڈ ماڈل طیارے کو بڑھانا چاہئے۔
اور انکرپٹڈ بائٹس کے لیے `SecretEnvelopeV1` / `CiphertextStateRecordV1` دوبارہ استعمال کریں
اور ciphertext-آبائی حالت، بجائے اس کے کہ نئی سروس/کنٹینر کے بطور انکوڈ کیا جائے۔
ظاہر کرتا ہے دیکھیں `uploaded_private_models.md`۔

## دائرہ کار

یہ مینی فیسٹ `IVM` + حسب ضرورت سورا کنٹینر رن ٹائم کے لیے بنائے گئے ہیں۔
(SCR) سمت (کوئی WASM نہیں، رن ٹائم داخلہ میں Docker انحصار نہیں)۔- `SoraContainerManifestV1` قابل عمل بنڈل شناخت، رن ٹائم کی قسم، کیپچر کرتا ہے
  قابلیت کی پالیسی، وسائل، لائف سائیکل تحقیقات کی ترتیبات، اور واضح
  رن ٹائم ماحول یا ماونٹڈ ریویژن میں مطلوبہ تشکیل برآمد کرتا ہے۔
  درخت
- `SoraServiceManifestV1` تعیناتی کے ارادے کو پکڑتا ہے: سروس کی شناخت،
  حوالہ شدہ کنٹینر مینی فیسٹ ہیش/ورژن، روٹنگ، رول آؤٹ پالیسی، اور
  ریاستی پابندیاں
- `SoraStateBindingV1` ریاستی تحریری دائرہ کار اور حدود کو حاصل کرتا ہے
  (نام کی جگہ کا سابقہ، تغیر پذیری موڈ، انکرپشن موڈ، آئٹم/کل کوٹہ)۔
- `SoraDeploymentBundleV1` جوڑے کنٹینر + سروس ظاہر کرتا ہے اور نافذ کرتا ہے
  ڈیٹرمنسٹک داخلہ چیکس (مینی فیسٹ ہیش لنکیج، اسکیما الائنمنٹ، اور
  قابلیت/ پابند مستقل مزاجی)۔
- `AgentApartmentManifestV1` مستقل ایجنٹ رن ٹائم پالیسی کیپچر کرتا ہے:
  ٹول کیپس، پالیسی کیپس، اخراجات کی حد، ریاستی کوٹہ، نیٹ ورک ایگریس، اور
  رویے کو اپ گریڈ کریں.
- `FheParamSetV1` گورننس کے زیر انتظام FHE پیرامیٹر سیٹ کیپچر کرتا ہے:
  ڈیٹرمنسٹک بیک اینڈ/اسکیم شناخت کنندہ، ماڈیولس پروفائل، سیکورٹی/گہرائی
  حدود، اور لائف سائیکل کی بلندیاں (`activation`/`deprecation`/`withdraw`)۔
- `FheExecutionPolicyV1` deterministic ciphertext پر عمل درآمد کی حدود کو حاصل کرتا ہے:
  تسلیم شدہ پے لوڈ سائز، ان پٹ/آؤٹ پٹ فین ان، گہرائی/روٹیشن/بوٹسٹریپ کیپس،
  اور کیننیکل راؤنڈنگ موڈ۔
- `FheGovernanceBundleV1` جوڑے ایک پیرامیٹر سیٹ اور پالیسی کے تعین کے لیے
  داخلہ کی توثیق- `FheJobSpecV1` deterministic ciphertext جاب میں داخلہ/Execution کیپچر کرتا ہے
  درخواستیں: آپریشن کلاس، آرڈر شدہ ان پٹ وعدے، آؤٹ پٹ کلید، اور پابند
  پالیسی + پیرامیٹر سیٹ سے منسلک گہرائی/روٹیشن/بوٹسٹریپ مانگ۔
- `DecryptionAuthorityPolicyV1` گورننس کے زیر انتظام افشاء کی پالیسی کیپچر کرتا ہے:
  اتھارٹی موڈ (کلائنٹ ہولڈ بمقابلہ حد سروس)، منظور کنندہ کورم/ممبرز،
  بریک گلاس الاؤنس، دائرہ اختیار ٹیگنگ، رضامندی کے ثبوت کی ضرورت،
  TTL حدود، اور کیننیکل آڈٹ ٹیگنگ۔
- `DecryptionRequestV1` پالیسی سے منسلک انکشاف کی کوششوں کو پکڑتا ہے:
  سائفر ٹیکسٹ کلیدی حوالہ (`binding_name` + `state_key` + عزم)،
  جواز، دائرہ اختیار ٹیگ، اختیاری رضامندی-ثبوت ہیش، TTL،
  بریک گلاس کا ارادہ/وجہ، اور گورننس ہیش لنکیج۔
- `CiphertextQuerySpecV1` deterministic ciphertext صرف استفسار کے ارادے کو حاصل کرتا ہے:
  سروس/بائنڈنگ اسکوپ، کلیدی سابقہ فلٹر، باؤنڈڈ رزلٹ کی حد، میٹا ڈیٹا
  پروجیکشن لیول، اور پروف انکلوژن ٹوگل۔
- `CiphertextQueryResponseV1` انکشاف سے کم سے کم استفسار کے نتائج حاصل کرتا ہے:
  ڈائجسٹ پر مبنی کلیدی حوالہ جات، سائفر ٹیکسٹ میٹا ڈیٹا، اختیاری شمولیت کے ثبوت،
  اور رسپانس لیول ٹرنکیشن/سیکونس سیاق و سباق۔
- `SecretEnvelopeV1` انکرپٹڈ پے لوڈ مواد کو خود پکڑتا ہے:
  انکرپشن موڈ، کلیدی شناخت کنندہ/ورژن، نونس، سائفر ٹیکسٹ بائٹس، اور
  سالمیت کے وعدے.
- `CiphertextStateRecordV1` سائفر ٹیکسٹ-آبائی ریاست کے اندراجات کو پکڑتا ہے جوعوامی میٹا ڈیٹا کو یکجا کریں (مواد کی قسم، پالیسی ٹیگز، عزم، پے لوڈ سائز)
  `SecretEnvelopeV1` کے ساتھ۔
- صارف کے اپ لوڈ کردہ پرائیویٹ ماڈل بنڈلز کو ان سائفر ٹیکسٹ-مقامی پر بنانا چاہیے۔
  ریکارڈز:
  خفیہ کردہ وزن/ تشکیل/ پروسیسر کے ٹکڑے ریاست میں رہتے ہیں، جبکہ ماڈل رجسٹری،
  وزن نسب، مرتب پروفائلز، انفرنس سیشنز، اور چوکیاں باقی ہیں۔
  فرسٹ کلاس سوراکاؤڈ ریکارڈز۔

## ورژن بنانا

- `SORA_CONTAINER_MANIFEST_VERSION_V1 = 1`
- `SORA_SERVICE_MANIFEST_VERSION_V1 = 1`
- `SORA_STATE_BINDING_VERSION_V1 = 1`
- `SORA_DEPLOYMENT_BUNDLE_VERSION_V1 = 1`
- `AGENT_APARTMENT_MANIFEST_VERSION_V1 = 1`
- `FHE_PARAM_SET_VERSION_V1 = 1`
- `FHE_EXECUTION_POLICY_VERSION_V1 = 1`
- `FHE_GOVERNANCE_BUNDLE_VERSION_V1 = 1`
- `FHE_JOB_SPEC_VERSION_V1 = 1`
- `DECRYPTION_AUTHORITY_POLICY_VERSION_V1 = 1`
- `DECRYPTION_REQUEST_VERSION_V1 = 1`
- `CIPHERTEXT_QUERY_SPEC_VERSION_V1 = 1`
- `CIPHERTEXT_QUERY_RESPONSE_VERSION_V1 = 1`
- `CIPHERTEXT_QUERY_PROOF_VERSION_V1 = 1`
- `SECRET_ENVELOPE_VERSION_V1 = 1`
- `CIPHERTEXT_STATE_RECORD_VERSION_V1 = 1`

توثیق کے ساتھ غیر تعاون یافتہ ورژن کو مسترد کرتا ہے۔
`SoraCloudManifestError::UnsupportedVersion`۔

## تعییناتی توثیق کے اصول (V1)- کنٹینر مینی فیسٹ:
  - `bundle_path` اور `entrypoint` غیر خالی ہونا چاہیے۔
  - `healthcheck_path` (اگر سیٹ ہے) `/` سے شروع ہونا چاہیے۔
  - `config_exports` صرف اعلان کردہ کنفیگرس کا حوالہ دے سکتا ہے۔
    `required_config_names`۔
  - config-export env اہداف کو کینونیکل ماحول کے متغیر ناموں کا استعمال کرنا چاہیے۔
    (`[A-Za-z_][A-Za-z0-9_]*`)۔
  - کنفگ ایکسپورٹ فائل کے اہداف کو رشتہ دار رہنا چاہیے، `/` سیپریٹرز کا استعمال کریں، اور
    خالی، `.` یا `..` سیگمنٹس پر مشتمل نہیں ہونا چاہیے۔
  - تشکیل برآمدات کو ایک ہی env var یا رشتہ دار فائل پاتھ کو زیادہ نشانہ نہیں بنانا چاہئے۔
    ایک بار سے زیادہ
- سروس مینی فیسٹ:
  - `service_version` غیر خالی ہونا چاہیے۔
  - `container.expected_schema_version` کنٹینر اسکیما v1 سے مماثل ہونا چاہیے۔
  - `rollout.canary_percent` `0..=100` ہونا چاہیے۔
  - `route.path_prefix` (اگر سیٹ ہے) `/` سے شروع ہونا چاہیے۔
  - ریاست کے پابند ناموں کو منفرد ہونا چاہیے۔
- ریاست کا پابند:
  - `key_prefix` غیر خالی ہونا چاہیے اور `/` سے شروع ہونا چاہیے۔
  - `max_item_bytes <= max_total_bytes`۔
  - `ConfidentialState` بائنڈنگز سادہ متن کی خفیہ کاری کا استعمال نہیں کرسکتی ہیں۔
- تعیناتی بنڈل:
  - `service.container.manifest_hash` کینونیکل انکوڈ سے مماثل ہونا چاہیے۔
    کنٹینر مینی فیسٹ ہیش۔
  - `service.container.expected_schema_version` کنٹینر اسکیما سے مماثل ہونا چاہیے۔
  - تغیر پذیر ریاستی پابندیوں کے لیے `container.capabilities.allow_state_writes=true` کی ضرورت ہوتی ہے۔
  - عوامی راستوں کے لیے `container.lifecycle.healthcheck_path` درکار ہے۔
- ایجنٹ اپارٹمنٹ مینی فیسٹ:
  - `container.expected_schema_version` کنٹینر سکیما v1 سے مماثل ہونا چاہیے۔
  - ٹول کی اہلیت کے نام غیر خالی اور منفرد ہونے چاہئیں۔- پالیسی کی اہلیت کے نام منفرد ہونے چاہئیں۔
  - خرچ کی حد کے اثاثے غیر خالی اور منفرد ہونے چاہئیں۔
  - ہر خرچ کی حد کے لیے `max_per_tx_nanos <= max_per_day_nanos`۔
  - اجازت دینے والے نیٹ ورک کی پالیسی میں منفرد غیر خالی میزبانوں کو شامل کرنا ضروری ہے۔
- FHE پیرامیٹر سیٹ:
  - `backend` اور `ciphertext_modulus_bits` غیر خالی ہونا چاہیے۔
  - ہر سائفر ٹیکسٹ ماڈیولس کا بٹ سائز `2..=120` کے اندر ہونا چاہیے۔
  - سائفر ٹیکسٹ ماڈیولس چین آرڈر غیر بڑھنے والا ہونا چاہیے۔
  - `plaintext_modulus_bits` سب سے بڑے سائفر ٹیکسٹ ماڈیولس سے چھوٹا ہونا چاہیے۔
  - `slot_count <= polynomial_modulus_degree`۔
  - `max_multiplicative_depth < ciphertext_modulus_bits.len()`۔
  - لائف سائیکل اونچائی کی ترتیب سخت ہونی چاہیے:
    `activation < deprecation < withdraw` جب موجود ہو۔
  - لائف سائیکل کی حیثیت کے تقاضے:
    - `Proposed` فرسودگی / بلندیوں کو واپس لینے کی اجازت نہیں دیتا ہے۔
    - `Active` کو `activation_height` کی ضرورت ہے۔
    - `Deprecated` کو `activation_height` + `deprecation_height` درکار ہے۔
    - `Withdrawn` کو `activation_height` + `withdraw_height` درکار ہے۔
- FHE عملدرآمد کی پالیسی:
  - `max_plaintext_bytes <= max_ciphertext_bytes`۔
  - `max_output_ciphertexts <= max_input_ciphertexts`۔
  - پیرامیٹر سیٹ بائنڈنگ `(param_set, version)` سے مماثل ہونی چاہیے۔
  - `max_multiplication_depth` پیرامیٹر سیٹ کی گہرائی سے زیادہ نہیں ہونا چاہیے۔
  - پالیسی داخلہ `Proposed` یا `Withdrawn` پیرامیٹر سیٹ لائف سائیکل کو مسترد کرتا ہے۔
- FHE گورننس بنڈل:
  - پالیسی + پیرامیٹر سیٹ کی مطابقت کو ایک تعییناتی داخلہ پے لوڈ کے طور پر توثیق کرتا ہے۔
- FHE کام کی تفصیلات:
  - `job_id` اور `output_state_key` غیر خالی ہونا چاہیے (`output_state_key` `/` سے شروع ہوتا ہے)۔- ان پٹ سیٹ غیر خالی ہونا چاہیے اور ان پٹ کیز منفرد کینونیکل راستے ہونے چاہئیں۔
  - آپریشن کے لیے مخصوص رکاوٹیں سخت ہیں (`Add`/`Multiply` ملٹی ان پٹ،
    `RotateLeft`/`Bootstrap` سنگل ان پٹ، باہمی خصوصی گہرائی/روٹیشن/بوٹسٹریپ نوبس کے ساتھ)۔
  - پالیسی سے منسلک داخلہ نافذ کرتا ہے:
    - پالیسی/پیرام شناخت کنندگان اور ورژن مماثل ہیں۔
    - ان پٹ کاؤنٹ/بائٹس، گہرائی، گردش، اور بوٹسٹریپ کی حدیں پالیسی کیپس کے اندر ہیں۔
    - deterministic متوقع آؤٹ پٹ بائٹس پالیسی سائفر ٹیکسٹ کی حدود کے مطابق ہیں۔
- ڈکرپشن اتھارٹی کی پالیسی:
  - `approver_ids` غیر خالی، منفرد، اور سختی سے لغت کے لحاظ سے ترتیب دیا جانا چاہیے۔
  - `ClientHeld` موڈ کے لیے بالکل ایک منظور کنندہ، `approver_quorum=1`،
    اور `allow_break_glass=false`۔
  - `ThresholdService` موڈ کے لیے کم از کم دو منظور کنندگان کی ضرورت ہے۔
    `approver_quorum <= approver_ids.len()`۔
  - `jurisdiction_tag` غیر خالی ہونا چاہیے اور اس میں کنٹرول کریکٹر نہیں ہونا چاہیے۔
  - `audit_tag` غیر خالی ہونا چاہیے اور اس میں کنٹرول کریکٹر نہیں ہونا چاہیے۔
- ڈکرپشن کی درخواست:
  - `request_id`، `state_key`، اور `justification` غیر خالی ہونا ضروری ہے
    (`state_key` `/` سے شروع ہوتا ہے)۔
  - `jurisdiction_tag` غیر خالی ہونا چاہیے اور اس میں کنٹرول کریکٹر نہیں ہونا چاہیے۔
  - `break_glass_reason` جب `break_glass=true` کی ضرورت ہوتی ہے اور جب اسے چھوڑ دیا جائے
    `break_glass=false`۔
  - پالیسی سے منسلک داخلہ پالیسی نام کی مساوات کو نافذ کرتا ہے، ٹی ٹی ایل کی درخواست نہ کریں۔`policy.max_ttl_blocks` سے تجاوز، دائرہ اختیار ٹیگ مساوات، بریک گلاس
    گیٹنگ، اور رضامندی کے ثبوت کے تقاضے جب
    `policy.require_consent_evidence=true` بغیر ٹوٹے شیشے کی درخواستوں کے لیے۔
- سائفر ٹیکسٹ استفسار کی تفصیلات:
  - `state_key_prefix` غیر خالی ہونا چاہیے اور `/` سے شروع ہونا چاہیے۔
  - `max_results` تعییناتی طور پر پابند ہے (`<=256`)۔
  - میٹا ڈیٹا پروجیکشن واضح ہے (`Minimal` صرف ڈائجسٹ بمقابلہ `Standard` کلید نظر آتا ہے)۔
- سائفر ٹیکسٹ استفسار کا جواب:
  - `result_count` کو سیریلائزڈ قطار کی گنتی کے برابر ہونا چاہیے۔
  - `Minimal` پروجیکشن کو `state_key` کو بے نقاب نہیں کرنا چاہیے؛ `Standard` اسے بے نقاب کرنا چاہیے۔
  - قطاروں کو کبھی بھی سادہ ٹیکسٹ انکرپشن موڈ کی سطح پر نہیں آنا چاہیے۔
  - شمولیت کے ثبوت (جب موجود ہوں) میں غیر خالی اسکیم آئی ڈی اور شامل ہونا ضروری ہے۔
    `anchor_sequence >= event_sequence`۔
- خفیہ لفافہ:
  - `key_id`, `nonce`، اور `ciphertext` غیر خالی ہونا چاہیے۔
  - نونس لمبائی پابند ہے (`<=256` بائٹس)۔
  - سائفر ٹیکسٹ کی لمبائی پابند ہے (`<=33554432` بائٹس)۔
- سائفر ٹیکسٹ اسٹیٹ ریکارڈ:
  - `state_key` غیر خالی ہونا چاہیے اور `/` سے شروع ہونا چاہیے۔
  - میٹا ڈیٹا مواد کی قسم غیر خالی ہونی چاہیے۔ ٹیگز منفرد غیر خالی سٹرنگز ہونے چاہئیں۔
  - `metadata.payload_bytes` `secret.ciphertext.len()` کے برابر ہونا چاہیے۔
  - `metadata.commitment` `secret.commitment` کے برابر ہونا چاہیے۔

## کیننیکل فکسچر

کیننیکل JSON فکسچر یہاں پر محفوظ ہیں:- `fixtures/soracloud/sora_container_manifest_v1.json`
- `fixtures/soracloud/sora_service_manifest_v1.json`
- `fixtures/soracloud/sora_state_binding_v1.json`
- `fixtures/soracloud/sora_deployment_bundle_v1.json`
- `fixtures/soracloud/agent_apartment_manifest_v1.json`
- `fixtures/soracloud/fhe_param_set_v1.json`
- `fixtures/soracloud/fhe_execution_policy_v1.json`
- `fixtures/soracloud/fhe_governance_bundle_v1.json`
- `fixtures/soracloud/fhe_job_spec_v1.json`
- `fixtures/soracloud/decryption_authority_policy_v1.json`
- `fixtures/soracloud/decryption_request_v1.json`
- `fixtures/soracloud/ciphertext_query_spec_v1.json`
- `fixtures/soracloud/ciphertext_query_response_v1.json`
- `fixtures/soracloud/secret_envelope_v1.json`
- `fixtures/soracloud/ciphertext_state_record_v1.json`

فکسچر/راؤنڈ ٹرپ ٹیسٹ:

- `crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs`
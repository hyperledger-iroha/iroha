<!-- Auto-generated stub for Burmese (my) translation. Replace this content with the full translation. -->

---
lang: my
direction: ltr
source: docs/source/soracloud/manifest_schemas.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a0724ed92da90d8d78a7095b5fc75523ea5dd4a1c885059e28c1bbb8118d1f8c
source_last_modified: "2026-03-26T06:12:11.480497+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# SoraCloud V1 Manifest Schemas

ဤစာမျက်နှာသည် SoraCloud အတွက် ပထမဆုံး အဆုံးအဖြတ်ပေးသော Norito အစီအစဉ်များကို သတ်မှတ်ပေးသည်
Iroha 3 တွင် ဖြန့်ကျက်ခြင်း-

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

Rust အဓိပ္ပါယ်ဖွင့်ဆိုချက်များသည် `crates/iroha_data_model/src/soracloud.rs` တွင် နေထိုင်ပါသည်။

အပ်လုဒ်လုပ်ထားသော မော်ဒယ်လ်သီးသန့်-runtime မှတ်တမ်းများသည် ရည်ရွယ်ချက်ရှိရှိ သီးခြားအလွှာတစ်ခုဖြစ်သည်။
ဤ SCR ဖြန့်ကျက်မှုသည် ထင်ရှားသည်။ Soracloud မော်ဒယ်လေယာဉ်ကို တိုးချဲ့သင့်တယ်။
ကုဒ်ဝှက်ထားသော ဘိုက်များအတွက် `SecretEnvelopeV1` / `CiphertextStateRecordV1` ကို ပြန်သုံးပါ
ဝန်ဆောင်မှု/ကွန်တိန်နာအသစ်အဖြစ် ကုဒ်ဝှက်ထားခြင်းထက် စာဝှက်စာသား-ဇာတိပြည်နယ်
ထင်ရှားသည်။ `uploaded_private_models.md` ကိုကြည့်ပါ။

## နယ်ပယ်

ဤဖော်ပြချက်များကို `IVM` + စိတ်ကြိုက် Sora Container Runtime အတွက် ဒီဇိုင်းထုတ်ထားပါသည်။
(SCR) ဦးတည်ချက် (WASM မရှိ၊ Docker အား မှီခိုနေရချိန် ဝင်ခွင့်မရှိ)။- `SoraContainerManifestV1` သည် executable bundle အထောက်အထား၊ runtime အမျိုးအစား၊
  စွမ်းရည်မူဝါဒ၊ အရင်းအမြစ်များ၊ ဘဝသံသရာစုံစမ်းစစ်ဆေးခြင်း ဆက်တင်များနှင့် ပြတ်သားစွာ
  လိုအပ်သော-config များကို runtime ပတ်၀န်းကျင်သို့ တင်ပို့ခြင်း သို့မဟုတ် တပ်ဆင်ထားသော ပြင်ဆင်မှု
  သစ်ပင်။
- `SoraServiceManifestV1` သည် ဖြန့်ကျက်ခြင်းရည်ရွယ်ချက်ကို ဖမ်းယူသည်- ဝန်ဆောင်မှုအထောက်အထား၊
  ရည်ညွှန်းထားသော ကွန်တိန်နာသည် ထင်ရှားသော hash/ဗားရှင်း၊ လမ်းကြောင်းသတ်မှတ်ခြင်း၊ စတင်ခြင်းမူဝါဒနှင့်
  ပြည်နယ်ချည်နှောင်မှု။
- `SoraStateBindingV1` သည် အဆုံးအဖြတ်ပေးသော နိုင်ငံတော်-ရေးထားသော နယ်ပယ်နှင့် ကန့်သတ်ချက်များကို ဖမ်းယူသည်
  (namespace ရှေ့ဆက်၊ ပြောင်းလဲနိုင်သောမုဒ်၊ ကုဒ်ဝှက်မုဒ်၊ ပစ္စည်း/စုစုပေါင်း ခွဲတမ်း)။
- `SoraDeploymentBundleV1` စုံတွဲကွန်တိန်နာ + ဝန်ဆောင်မှုကို ဖော်ပြပြီး ပြဋ္ဌာန်းထားသည်။
  အဆုံးအဖြတ်ပေးသော ဝင်ခွင့်စစ်ဆေးမှုများ (manifest-hash linkage၊ schema alignment နှင့်
  စွမ်းဆောင်နိုင်မှု/စည်းနှောင်မှု ညီညွတ်မှု)။
- `AgentApartmentManifestV1` သည် အမြဲတမ်း အေးဂျင့် runtime မူဝါဒကို ဖမ်းယူသည်-
  ကိရိယာထုပ်များ၊ မူဝါဒထုပ်များ၊ သုံးစွဲမှုကန့်သတ်ချက်များ၊ ပြည်နယ်ခွဲတမ်း၊ ကွန်ရက်ထွက်ပေါက်နှင့်
  အပြုအမူအဆင့်မြှင့်ပါ။
- `FheParamSetV1` သည် အုပ်ချုပ်မှု-စီမံခန့်ခွဲသော FHE ကန့်သတ်ချက်များအား ဖမ်းယူသည်-
  အဆုံးအဖြတ်ပေးသော နောက်ခံအစွန်း/အစီအစဥ် ခွဲခြားသတ်မှတ်မှုများ၊ မော်ဒူလပ်ပရိုဖိုင်၊ လုံခြုံရေး/အတိမ်အနက်
  ဘောင်များနှင့် ဘဝသံသရာ အမြင့်များ (`activation`/`deprecation`/`withdraw`)။
- `FheExecutionPolicyV1` သည် သတ်မှတ်ထားသော ciphertext လုပ်ဆောင်ချက်ကန့်သတ်ချက်များကို ဖမ်းယူသည်-
  ဝန်ခံထားသော payload အရွယ်အစားများ၊ အဝင်/အထွက် ပန်ကာအဝင်၊ အတိမ်အနက်/လှည့်ခြင်း/ bootstrap ထုပ်များ၊
  နှင့် canonical rounding မုဒ်။
- `FheGovernanceBundleV1` သည် အဆုံးအဖြတ်သတ်မှတ်မှုအတွက် ကန့်သတ်ချက်နှင့် မူဝါဒကို ပေါင်းစပ်ထားသည်။
  ဝင်ခွင့်အတည်ပြုချက်။- `FheJobSpecV1` သည် သတ်မှတ်ထားသော ciphertext အလုပ်ဝင်ခွင့်/လုပ်ဆောင်မှုကို ဖမ်းယူသည်
  တောင်းဆိုချက်များ- လည်ပတ်မှု အတန်းအစား၊ ထည့်သွင်းမှု ကတိကဝတ်များ ခိုင်းစေမှု၊ အထွက် သော့နှင့် ကန့်သတ်ထားသည်။
  မူဝါဒ + ကန့်သတ်ချက်တစ်ခုနှင့် ချိတ်ဆက်ထားသော depth/rotation/bootstrap တောင်းဆိုချက်။
- `DecryptionAuthorityPolicyV1` သည် အုပ်ချုပ်ရေး-စီမံခန့်ခွဲသည့် ထုတ်ဖော်မှုမူဝါဒကို ဖမ်းယူသည်-
  အခွင့်အာဏာမုဒ် (Client-held-held vs threshold service)၊ အတည်ပြုသူ အထမြောက်/အဖွဲ့ဝင်များ၊
  မှန်ကွဲခွင့်၊ တရားစီရင်ပိုင်ခွင့် တံဆိပ်ကပ်ခြင်း၊ ခွင့်ပြုချက်-အထောက်အထား လိုအပ်ချက်၊
  TTL ဘောင်များ၊ နှင့် canonical audit tagging။
- `DecryptionRequestV1` သည် မူဝါဒနှင့် ချိတ်ဆက်ထားသော ထုတ်ဖော်ရန် ကြိုးစားမှုများကို ဖမ်းယူသည်-
  ciphertext သော့ရည်ညွှန်းချက် (`binding_name` + `state_key` + ကတိကဝတ်)၊
  အကြောင်းပြချက်၊ တရားစီရင်ပိုင်ခွင့် တဂ်၊ စိတ်ကြိုက်ခွင့်ပြုချက်-အထောက်အထား hash၊ TTL၊
  break-glass ရည်ရွယ်ချက်/အကြောင်းပြချက်၊ နှင့် အုပ်ချုပ်မှု hash linkage။
- `CiphertextQuerySpecV1` သည် သတ်မှတ်ထားသော ciphertext-only query ရည်ရွယ်ချက်ကို ဖမ်းယူသည်-
  ဝန်ဆောင်မှု/စည်းနှောင်မှုနယ်ပယ်၊ သော့-ရှေ့ဆက်စစ်ထုတ်မှု၊ ကန့်သတ်ထားသော ရလဒ်ကန့်သတ်ချက်၊ မက်တာဒေတာ
  ပရိုဂျက်တာအဆင့်၊ နှင့် သက်သေပါဝင်မှု အဖွင့်အပိတ်။
- `CiphertextQueryResponseV1` သည် ထုတ်ဖော်-အနိမ့်ဆုံး မေးခွန်းထုတ်ချက်များကို ဖမ်းယူသည်-
  အချေအတင်အသားပေးသော အဓိကအကိုးအကားများ၊ ciphertext မက်တာဒေတာ၊ ရွေးချယ်နိုင်သော ပါဝင်မှုအထောက်အထားများ၊
  နှင့် တုံ့ပြန်မှုအဆင့် ဖြတ်တောက်ခြင်း/အစီအစဥ်ဆက်စပ်မှု။
- `SecretEnvelopeV1` သည် ကုဒ်ဝှက်ထားသော payload ပစ္စည်းကို ကိုယ်တိုင်ဖမ်းယူသည်-
  ကုဒ်ဝှက်ခြင်းမုဒ်၊ သော့သတ်မှတ်သူ/ဗားရှင်း၊ nonce၊ စာဝှက်စာသားဘိုက်များနှင့်
  သမာဓိကတိကဝတ်။
- `CiphertextStateRecordV1` သည် ciphertext-native state entries များကို ဖမ်းယူသည်။အများသူငှာ မက်တာဒေတာ (အကြောင်းအရာအမျိုးအစား၊ မူဝါဒတဂ်များ၊ ကတိကဝတ်များ၊ ပေးချေမှုအရွယ်အစား)
  `SecretEnvelopeV1` နှင့်
- အသုံးပြုသူ-အပ်လုဒ်လုပ်ထားသော သီးသန့်မော်ဒယ်အစုအဝေးများသည် ဤ ciphertext-native ပေါ်တွင် တည်ဆောက်သင့်သည်။
  မှတ်တမ်းများ
  ကုဒ်ဝှက်ထားသောအလေးချိန်/config/processor အပိုင်းများသည် အခြေအနေတွင်ရှိနေသည်၊ မော်ဒယ်မှတ်ပုံတင်ခြင်း၊
  အလေးချိန်မျိုးရိုး၊ ပရိုဖိုင်များစုစည်းမှု၊ အနုအရင့်အစည်းအဝေးများနှင့် စစ်ဆေးရေးဂိတ်များ ကျန်ရှိနေပါသည်။
  ပထမတန်းစား Soracloud မှတ်တမ်းများ။

## ဗားရှင်းပြောင်းခြင်း။

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

Validation သည် ပံ့ပိုးမထားသော ဗားရှင်းများဖြင့် ငြင်းပယ်သည်။
`SoraCloudManifestError::UnsupportedVersion`။

## သတ်မှတ်အတည်ပြုခြင်းဆိုင်ရာ စည်းမျဉ်းများ (V1)- ကွန်တိန်နာမန်နီးဖက်စ်-
  - `bundle_path` နှင့် `entrypoint` သည် ဗလာမဟုတ်ပါ ။
  - `healthcheck_path` (သတ်မှတ်ထားပါက) `/` ဖြင့် စတင်ရပါမည်။
  - `config_exports` တွင် ကြေညာထားသော configs များကိုသာ ကိုးကားနိုင်ပါသည်။
    `required_config_names`။
  - config-export env ပစ်မှတ်များသည် canonical environment-variable အမည်များကို အသုံးပြုရပါမည်။
    (`[A-Za-z_][A-Za-z0-9_]*`)။
  - config-export ဖိုင်ပစ်မှတ်များသည် ဆက်စပ်နေရမည်၊ `/` ခြားနားချက်များကို အသုံးပြုပါ၊
    အလွတ်၊ `.` သို့မဟုတ် `..` အပိုင်းများ မပါဝင်ရပါ။
  - config တင်ပို့မှုများသည် တူညီသော env var သို့မဟုတ် ဆွေမျိုးဖိုင်လမ်းကြောင်းကို ပို၍ ပစ်မှတ်မထားရပါ။
    တစ်ကြိမ်ထက်
- ဝန်ဆောင်မှုဖော်ပြချက်-
  - `service_version` သည် ဗလာမဟုတ်ပါ ။
  - `container.expected_schema_version` သည် container schema v1 နှင့် ကိုက်ညီရပါမည်။
  - `rollout.canary_percent` သည် `0..=100` ဖြစ်ရမည်။
  - `route.path_prefix` (သတ်မှတ်ထားပါက) `/` ဖြင့် စတင်ရပါမည်။
  - ပြည်နယ် binding အမည်များသည် ထူးခြားမှုရှိရမည်။
- ပြည်နယ်စည်းနှောင်မှု-
  - `key_prefix` သည် ဗလာမဟုတ်ပဲ `/` ဖြင့် စတင်ရပါမည်။
  - `max_item_bytes <= max_total_bytes`။
  - `ConfidentialState` ချိတ်ဆက်မှုများသည် plaintext encryption ကို အသုံးမပြုနိုင်ပါ။
- အသုံးချမှုအစုအဝေး-
  - `service.container.manifest_hash` သည် canonical encoded နှင့် ကိုက်ညီရမည်။
    container manifest hash။
  - `service.container.expected_schema_version` သည် container schema နှင့် ကိုက်ညီရမည်။
  - မပြောင်းလဲနိုင်သော ပြည်နယ်စည်းနှောင်မှု `container.capabilities.allow_state_writes=true` လိုအပ်သည်။
  - အများသူငှာလမ်းကြောင်းများ `container.lifecycle.healthcheck_path` လိုအပ်သည်။
- အေးဂျင့်တိုက်ခန်းဖော်ပြချက်-
  - `container.expected_schema_version` သည် container schema v1 နှင့် ကိုက်ညီရမည်။
  - ကိရိယာစွမ်းဆောင်နိုင်မှုအမည်များသည် ဗလာမဟုတ်သလို ထူးခြားမှုရှိရမည်။- မူဝါဒစွမ်းရည်အမည်များသည် ထူးခြားမှုရှိရမည်။
  - သုံးစွဲမှုကန့်သတ်ထားသော ပိုင်ဆိုင်မှုများသည် အချည်းနှီးမဟုတ်ဘဲ ထူးခြားနေရပါမည်။
  သုံးစွဲမှုကန့်သတ်ချက်တစ်ခုစီအတွက် `max_per_tx_nanos <= max_per_day_nanos`။
  - ခွင့်ပြုစာရင်းကွန်ရက်မူဝါဒတွင် သီးခြားဗလာမဟုတ်သော host များ ပါဝင်ရပါမည်။
- FHE ကန့်သတ်ချက်-
  - `backend` နှင့် `ciphertext_modulus_bits` သည် ဗလာမဟုတ်ပါ ။
  - ciphertext modulus bit-size တစ်ခုစီသည် `2..=120` အတွင်း ဖြစ်ရမည်။
  - ciphertext modulus ကွင်းဆက်အမှာစာသည် တိုးနေသည်မဟုတ်ပေ။
  - `plaintext_modulus_bits` သည် အကြီးဆုံး ciphertext modulus ထက် သေးငယ်ရမည်။
  - `slot_count <= polynomial_modulus_degree`။
  - `max_multiplicative_depth < ciphertext_modulus_bits.len()`။
  - lifecycle အမြင့်အမိန့်ကို တင်းကျပ်ရမည်-
    လက်ရှိအချိန်တွင် `activation < deprecation < withdraw`။
  - ဘဝသံသရာအခြေအနေလိုအပ်ချက်များ
    - `Proposed` သည် ကန့်သတ်ခြင်း/ရုပ်သိမ်းခြင်း အမြင့်များကို ခွင့်မပြုပါ။
    - `Active` `activation_height` လိုအပ်သည်။
    - `Deprecated` `activation_height` + `deprecation_height` လိုအပ်သည်။
    - `Withdrawn` `activation_height` + `withdraw_height` လိုအပ်သည်။
- FHE ကွပ်မျက်ရေးမူဝါဒ-
  - `max_plaintext_bytes <= max_ciphertext_bytes`။
  - `max_output_ciphertexts <= max_input_ciphertexts`။
  - ကန့်သတ်ဘောင်စည်းနှောင်မှုသည် `(param_set, version)` ဖြင့် တူညီရပါမည်။
  - `max_multiplication_depth` သည် ကန့်သတ်ဘောင်၏ အတိမ်အနက်ထက် မကျော်လွန်ရပါ။
  - မူဝါဒဝင်ခွင့်သည် `Proposed` သို့မဟုတ် `Withdrawn` ပါရာမီတာသတ်မှတ်ထားသည့် သက်တမ်းစက်ဝန်းကို ငြင်းပယ်သည်။
- FHE အုပ်ချုပ်မှုအစုအဝေး-
  - အဆုံးအဖြတ်ပေးသော ဝင်ခွင့်ပေးဆောင်မှုတစ်ခုအဖြစ် မူဝါဒ + ကန့်သတ်ချက်နှင့် ကိုက်ညီမှုရှိကြောင်း အတည်ပြုသည်။
- FHE အလုပ် spec-
  - `job_id` နှင့် `output_state_key` သည် ဗလာမဟုတ်ပါ (`output_state_key` သည် `/` နှင့် စတင်သည်)။- ထည့်သွင်းသတ်မှတ်မှုသည် ဗလာမဟုတ်သော ဖြစ်ရမည် ဖြစ်ပြီး ထည့်သွင်းသော့များသည် သီးသန့် canonical လမ်းကြောင်းများ ဖြစ်ရပါမည်။
  - လုပ်ဆောင်ချက်-တိကျသောကန့်သတ်ချက်များသည် တင်းကျပ်သည် (`Add`/`Multiply` ဘက်စုံထည့်သွင်းမှု၊
    `RotateLeft`/`Bootstrap`၊ အပြန်အလှန်သီးသန့် အတိမ်အနက်/လှည့်ခြင်း/bootstrap ခလုတ်များပါရှိသော)။
  - မူဝါဒနှင့် ချိတ်ဆက်ထားသော ဝင်ခွင့်ကို ပြဋ္ဌာန်းသည်-
    - မူဝါဒ/ဘောင်သတ်မှတ်စနစ်များနှင့် ဗားရှင်းများ ကိုက်ညီသည်။
    - ထည့်သွင်းမှုအရေအတွက်/ဘိုက်များ၊ အတိမ်အနက်၊ လည်ပတ်မှုနှင့် bootstrap ကန့်သတ်ချက်များသည် မူဝါဒထုပ်များအတွင်းတွင်ရှိသည်။
    - အဆုံးအဖြတ်ပေးသော ပရိုဂရမ်အထွက်ဘိုက်များသည် မူဝါဒ ciphertext ကန့်သတ်ချက်များနှင့် ကိုက်ညီပါသည်။
- ကုဒ်ဝှက်ခြင်းဆိုင်ရာ အခွင့်အာဏာမူဝါဒ-
  - `approver_ids` သည် အလွတ်မဟုတ်၊ ထူးခြားပြီး တင်းကြပ်စွာ အဘိဓာန်ဖြင့် စီထားရပါမည်။
  - `ClientHeld` မုဒ်တွင် အတည်ပြုသူ တစ်ဦးအတိအကျ လိုအပ်သည်၊ `approver_quorum=1`၊
    နှင့် `allow_break_glass=false`။
  - `ThresholdService` မုဒ်တွင် အတည်ပြုသူ နှစ်ဦးနှင့် အနည်းဆုံး လိုအပ်သည်။
    `approver_quorum <= approver_ids.len()`။
  - `jurisdiction_tag` သည် ဗလာမဟုတ်သည့်အပြင် ထိန်းချုပ်မှုအက္ခရာများ မပါဝင်ရပါ။
  - `audit_tag` သည် အလွတ်မဟုတ်ရမည်ဖြစ်ပြီး ထိန်းချုပ်ရေးအက္ခရာများ မပါဝင်ရပါ။
- ကုဒ်ဝှက်ခြင်းတောင်းဆိုချက်-
  - `request_id`၊ `state_key` နှင့် `justification` သည် ဗလာမဟုတ်ပါ
    (`state_key` သည် `/` ဖြင့် စတင်သည်)။
  - `jurisdiction_tag` သည် ဗလာမဟုတ်သည့်အပြင် ထိန်းချုပ်မှုအက္ခရာများ မပါဝင်ရပါ။
  - `break_glass=true` တွင် `break_glass_reason` လိုအပ်ပြီး မည်သည့်အချိန်တွင် ချန်လှပ်ထားရမည်၊
    `break_glass=false`။
  - မူဝါဒ-ချိတ်ဆက်ထားသော ဝင်ခွင့်သည် မူဝါဒအမည် တန်းတူညီမျှမှုကို ပြဋ္ဌာန်းသည်၊ TTL ကို တောင်းဆိုမည်မဟုတ်ပါ။`policy.max_ttl_blocks` ကျော်လွန်သည်၊ တရားစီရင်ပိုင်ခွင့်-တက်ဂ် တန်းတူရေး၊ မှန်ကွဲ
    ဂိတ်မှူးနှင့် သဘောတူခွင့်ပြုချက်-အထောက်အထား လိုအပ်ချက်များ ရှိသည့်အခါ
    မကွဲသည့်မှန်တောင်းဆိုမှုများအတွက် `policy.require_consent_evidence=true`။
- Ciphertext query spec-
  - `state_key_prefix` သည် ဗလာမဟုတ်ပဲ `/` ဖြင့် စတင်ရပါမည်။
  - `max_results` သည် အဆုံးအဖြတ်အရ ကန့်သတ်ထားသည် (`<=256`)။
  - မက်တာဒေတာပုံဆွဲခြင်းမှာ ရှင်းလင်းပြတ်သားသည် (`Minimal` အချေအတင်သီးသန့်နှင့် `Standard` သော့ဖြင့် မြင်နိုင်သည်)။
- Ciphertext မေးမြန်းမှု တုံ့ပြန်မှု-
  - `result_count` သည် အမှတ်စဉ်အလိုက် အတန်းရေတွက်မှုနှင့် တူညီရပါမည်။
  - `Minimal` ပရောဂျက်သည် `state_key` ကို မဖော်ပြရပါ။ `Standard` အဲဒါကို ဖော်ထုတ်ရမယ်။
  - အတန်းများသည် plaintext ကုဒ်ဝှက်ခြင်းမုဒ်ကို ဘယ်သောအခါမှ မပေါ်စေရပါ။
  - ပါဝင်ခြင်းဆိုင်ရာ အထောက်အထားများ (တင်ပြသည့်အခါ) တွင် ဗလာမဟုတ်သော အစီအစဉ်အိုင်ဒီများနှင့် ပါဝင်ရမည်။
    `anchor_sequence >= event_sequence`။
- လျှို့ဝှက်စာအိတ်
  - `key_id`၊ `nonce` နှင့် `ciphertext` သည် အလွတ်မဟုတ်ရပါ။
  - အလျားသည် အကန့်အသတ်မရှိ (`<=256` bytes)။
  - ciphertext အရှည် (`<=33554432` bytes) ကို ဘောင်ခတ်ထားသည်။
- Ciphertext အခြေအနေ မှတ်တမ်း-
  - `state_key` သည် ဗလာမဟုတ်ပဲ `/` ဖြင့် စတင်ရပါမည်။
  - မက်တာဒေတာ အကြောင်းအရာအမျိုးအစားသည် ဗလာမဟုတ်ပါ; တဂ်များသည် သီးသန့်ဗလာမဟုတ်သော စာကြောင်းများ ဖြစ်ရပါမည်။
  - `metadata.payload_bytes` သည် `secret.ciphertext.len()` နှင့် ညီရပါမည်။
  - `metadata.commitment` သည် `secret.commitment` နှင့် ညီရပါမည်။

## Canonical Fixtures

Canonical JSON တပ်ဆင်မှုများကို အောက်ပါနေရာတွင် သိမ်းဆည်းထားပါသည်။- `fixtures/soracloud/sora_container_manifest_v1.json`
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

Fixture/အသွားအပြန် စမ်းသပ်မှုများ

- `crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs`
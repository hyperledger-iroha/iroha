<!-- Auto-generated stub for Mongolian (mn) translation. Replace this content with the full translation. -->

---
lang: mn
direction: ltr
source: docs/source/soracloud/manifest_schemas.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a0724ed92da90d8d78a7095b5fc75523ea5dd4a1c885059e28c1bbb8118d1f8c
source_last_modified: "2026-03-26T06:12:11.480497+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# SoraCloud V1 манифест схемүүд

Энэ хуудас нь SoraCloud-д зориулсан Norito анхны детерминистик схемүүдийг тодорхойлдог.
Iroha 3 дээр байршуулах:

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

Зэвийн тодорхойлолтууд нь `crates/iroha_data_model/src/soracloud.rs`-д амьдардаг.

Байршуулсан загварын хувийн ажиллах цагийн бичлэгүүд нь зориуд тусдаа давхарга юм
эдгээр SCR байршуулалтын илрэлүүд. Тэд Soracloud загварын онгоцыг сунгах ёстой
болон шифрлэгдсэн байтуудад `SecretEnvelopeV1` / `CiphertextStateRecordV1`-г дахин ашиглах
шинэ үйлчилгээ/контейнер гэж кодлохын оронд шифр текстийн төрөлх төлөв
илэрдэг. `uploaded_private_models.md`-г үзнэ үү.

## Хамрах хүрээ

Эдгээр манифестууд нь `IVM` + захиалгат Sora Container Runtime-д зориулагдсан болно.
(SCR) чиглэл (WASM байхгүй, ажиллах цагийн элсэлтийн Docker хамаарал байхгүй).- `SoraContainerManifestV1` нь гүйцэтгэгдэх багцын таниулбар, ажиллах цагийн төрөл,
  чадварын бодлого, нөөц, амьдралын мөчлөгийн шалгалтын тохиргоо, тодорхой
  шаардлагатай-тохиргоог ажлын цагийн орчин эсвэл суурилуулсан засвар руу экспортлох
  мод.
- `SoraServiceManifestV1` нь байршуулах зорилгыг агуулна: үйлчилгээний таниулбар,
  лавлагаатай контейнерийн манифест хэш/хувилбар, чиглүүлэлт, нэвтрүүлэх бодлого болон
  төрийн холболтууд.
- `SoraStateBindingV1` нь детерминист төлөв бичих хамрах хүрээ болон хязгаарыг тогтоодог
  (нэрийн орон зайн угтвар, хувирах горим, шифрлэлтийн горим, зүйл/нийт квот).
- `SoraDeploymentBundleV1` нь контейнер + үйлчилгээний манифестуудыг нэгтгэж, хэрэгжүүлдэг
  тодорхойлогч элсэлтийн шалгалтууд (манифест-хэш холболт, схемийн тохируулга, ба
  чадвар/хэрэглэх тууштай байдал).
- `AgentApartmentManifestV1` байнгын агентын ажиллах цагийн бодлогыг барьж авдаг:
  хэрэгслийн дээд хязгаар, бодлогын хязгаар, зарцуулалтын хязгаар, муж улсын квот, сүлжээний гарц, болон
  зан төлөвийг сайжруулах.
- `FheParamSetV1` нь засаглалын удирддаг FHE параметрийн багцуудыг агуулна:
  тодорхойлогч арын хэсэг/схемийн тодорхойлогч, модулийн профайл, аюулгүй байдал/гүнзгий
  хязгаар, амьдралын мөчлөгийн өндөр (`activation`/`deprecation`/`withdraw`).
- `FheExecutionPolicyV1` нь детерминист шифрлэгдсэн текстийн гүйцэтгэлийн хязгаарыг тогтоодог:
  зөвшөөрөгдсөн даацын хэмжээ, оролт/гаралтын сэнс, гүн/эргэлт/ачаалагчийн таг,
  ба каноник дугуйлах горим.
- `FheGovernanceBundleV1` нь параметрийн багц болон бодлогыг детерминистикт зориулж холбодог
  элсэлтийн баталгаажуулалт.- `FheJobSpecV1` нь тодорхойлогч шифр текстийн ажилд орох/гүйцэтгэх үйл явцыг бүртгэдэг.
  хүсэлт: үйлдлийн анги, захиалгат оролтын амлалт, гаралтын түлхүүр, хязгаарлагдмал
  бодлого + параметрийн багцтай холбосон гүн/эргэлт/ачаалах эрэлт.
- `DecryptionAuthorityPolicyV1` нь засаглалын удирддаг тодруулгын бодлогыг тусгасан:
  эрх мэдлийн горим (үйлчлүүлэгчийн эзэмшилд байгаа үйлчилгээ ба босго үйлчилгээ), батлах чуулга/гишүүд,
  шил хугарах тэтгэмж, харьяаллын шошго, зөвшөөрөл-нотлох шаардлага,
  TTL хязгаар, каноник аудитын шошго.
- `DecryptionRequestV1` бодлоготой холбоотой илчлэх оролдлогуудыг агуулна:
  шифр текстийн түлхүүрийн лавлагаа (`binding_name` + `state_key` + амлалт),
  үндэслэл, харьяаллын шошго, нэмэлт зөвшөөрөл-нотлох хэш, TTL,
  хагалах зорилго/шалтгаан, засаглалын хэш холболт.
- `CiphertextQuerySpecV1` нь зөвхөн детерминист шифрлэгдсэн асуулгын зорилгыг агуулна:
  үйлчилгээ/холбох хамрах хүрээ, түлхүүрийн угтвар шүүлтүүр, хязгаарлагдмал үр дүнгийн хязгаар, мета өгөгдөл
  проекцын түвшин, нотолгоог оруулах сэлгэх.
- `CiphertextQueryResponseV1` нь тодруулгыг багасгасан асуулгын гаралтыг авдаг:
  задлахад чиглэсэн түлхүүр лавлагаа, шифр текстийн мета өгөгдөл, нэмэлт оруулах баталгаа,
  болон хариултын түвшний тайралт/дарааллын контекст.
- `SecretEnvelopeV1` нь шифрлэгдсэн ачааны материалыг өөрөө авдаг:
  шифрлэлтийн горим, түлхүүр танигч/хувилбар, nonce, шифр текстийн байт болон
  шударга байдлын амлалтууд.
- `CiphertextStateRecordV1` нь шифрлэгдсэн текстийн үндсэн төлөвийн оруулгуудыг авдаг.нийтийн мета өгөгдлийг нэгтгэх (агуулгын төрөл, бодлогын шошго, үүрэг хариуцлага, ачааллын хэмжээ)
  `SecretEnvelopeV1`-тэй.
- Хэрэглэгчийн байршуулсан хувийн загварын багцууд нь эдгээр эх бичвэр дээр суурилсан байх ёстой
  бичлэгүүд:
  Шифрлэгдсэн жин/тохиргоо/процессорын хэсгүүд нь төлөв байдалд амьдардаг бол загварын бүртгэл,
  жингийн удам угсаа, эмхэтгэлийн профайл, дүгнэлтийн сесс, хяналтын цэгүүд хэвээр байна
  нэгдүгээр зэрэглэлийн Soracloud бичлэгүүд.

## Хувилбар

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

Баталгаажуулалт нь дэмжигдээгүй хувилбаруудаас татгалздаг
`SoraCloudManifestError::UnsupportedVersion`.

## Тодорхойлогч баталгаажуулалтын дүрэм (V1)- Контейнерийн манифест:
  - `bundle_path` болон `entrypoint` хоосон биш байх ёстой.
  - `healthcheck_path` (хэрэв тохируулсан бол) `/`-ээр эхлэх ёстой.
  - `config_exports` нь зөвхөн-д заасан тохиргоог лавлаж болно
    `required_config_names`.
  - config-export env зорилтууд нь каноник орчны хувьсагчийн нэрийг ашиглах ёстой
    (`[A-Za-z_][A-Za-z0-9_]*`).
  - config-export файлын зорилтууд харьцангуй байх ёстой, `/` тусгаарлагчийг ашиглах ба
    хоосон, `.` эсвэл `..` сегментийг агуулж болохгүй.
  - тохиргооны экспорт нь ижил env var эсвэл харьцангуй файлын замыг илүү чиглүүлэх ёсгүй
    нэгээс илүү.
- Үйлчилгээний манифест:
  - `service_version` хоосон биш байх ёстой.
  - `container.expected_schema_version` нь контейнер схем v1-тэй тохирч байх ёстой.
  - `rollout.canary_percent` `0..=100` байх ёстой.
  - `route.path_prefix` (хэрэв тохируулсан бол) `/`-ээр эхлэх ёстой.
  - муж улсын заавал байх нэр нь өвөрмөц байх ёстой.
- Төрийн алба:
  - `key_prefix` хоосон биш байх ёстой бөгөөд `/` гэж эхэлнэ.
  - `max_item_bytes <= max_total_bytes`.
  - `ConfidentialState` холболтууд нь энгийн текст шифрлэлтийг ашиглах боломжгүй.
- Байршуулах багц:
  - `service.container.manifest_hash` нь каноник кодлогдсонтой тохирч байх ёстой
    контейнерийн манифест хэш.
  - `service.container.expected_schema_version` нь контейнерийн схемтэй тохирч байх ёстой.
  - Хувиргах төлөвтэй холбоход `container.capabilities.allow_state_writes=true` шаардлагатай.
  - Нийтийн замд `container.lifecycle.healthcheck_path` шаардлагатай.
- Агент орон сууцны манифест:
  - `container.expected_schema_version` нь контейнерийн схем v1-тэй тохирч байх ёстой.
  - Хэрэгслийн чадварын нэр нь хоосон биш, өвөрмөц байх ёстой.- бодлогын чадамжийн нэр нь өвөрмөц байх ёстой.
  - зарцуулалтын хязгаарлагдмал хөрөнгө нь хоосон биш, өвөрмөц байх ёстой.
  - Зардлын хязгаар бүрийн хувьд `max_per_tx_nanos <= max_per_day_nanos`.
  - Зөвшөөрөгдсөн жагсаалтын сүлжээний бодлого нь хоосон бус хосгүй хосуудыг агуулсан байх ёстой.
- FHE параметрийн багц:
  - `backend` болон `ciphertext_modulus_bits` хоосон биш байх ёстой.
  - шифрлэгдсэн текстийн модулийн битийн хэмжээ бүр `2..=120` дотор байх ёстой.
  - шифрлэгдсэн текстийн модулийн хэлхээний дараалал нэмэгдэхгүй байх ёстой.
  - `plaintext_modulus_bits` нь хамгийн том шифрлэгдсэн текстийн модулиас бага байх ёстой.
  - `slot_count <= polynomial_modulus_degree`.
  - `max_multiplicative_depth < ciphertext_modulus_bits.len()`.
  - амьдралын мөчлөгийн өндрийн захиалга хатуу байх ёстой:
    Байгаа үед `activation < deprecation < withdraw`.
  - амьдралын мөчлөгийн төлөв байдалд тавигдах шаардлага:
    - `Proposed` хүчингүй болгох/татах өндрийг зөвшөөрдөггүй.
    - `Active`-д `activation_height` шаардлагатай.
    - `Deprecated`-д `activation_height` + `deprecation_height` шаардлагатай.
    - `Withdrawn`-д `activation_height` + `withdraw_height` шаардлагатай.
- FHE-ийн гүйцэтгэлийн бодлого:
  - `max_plaintext_bytes <= max_ciphertext_bytes`.
  - `max_output_ciphertexts <= max_input_ciphertexts`.
  - параметрийн багц нь `(param_set, version)`-тай тохирч байх ёстой.
  - `max_multiplication_depth` нь параметрийн тогтоосон гүнээс хэтрэхгүй байх ёстой.
  - бодлогын элсэлт нь `Proposed` эсвэл `Withdrawn` параметрийн багцын амьдралын мөчлөгөөс татгалздаг.
- FHE засаглалын багц:
  - бодлого + параметрийн тохируулгыг нэг тодорхойлогч элсэлтийн ачаалал болгон баталгаажуулдаг.
- FHE ажлын онцлог:
  - `job_id` болон `output_state_key` хоосон биш байх ёстой (`output_state_key` нь `/`-р эхэлдэг).- оролтын багц хоосон биш байх ёстой бөгөөд оролтын товчлуурууд нь өвөрмөц каноник зам байх ёстой.
  - үйл ажиллагааны тусгай хязгаарлалтууд нь хатуу (`Add`/`Multiply` олон оролт,
    `RotateLeft`/`Bootstrap` нэг оролттой, бие биенээ үгүйсгэдэг гүн/эргэлт/ачаалах товчлууртай).
  - бодлоготой холбоотой элсэлтийн хэрэгжилт:
    - бодлого/парам танигч болон хувилбарууд таарч байна.
    - оролтын тоо/байт, гүн, эргэлт, ачаалах хязгаарлалт нь бодлогын дээд хязгаарт багтсан байна.
    - детерминистик тооцоолсон гаралтын байт нь бодлогын шифр текстийн хязгаарт тохирно.
- Шифр тайлах эрх бүхий бодлого:
  - `approver_ids` нь хоосон биш, өвөрмөц, толь бичгийн хувьд хатуу эрэмблэгдсэн байх ёстой.
  - `ClientHeld` горимд яг нэг зөвшөөрч шаардлагатай, `approver_quorum=1`,
    болон `allow_break_glass=false`.
  - `ThresholdService` горимд дор хаяж хоёр зөвшөөрөл авагч болон
    `approver_quorum <= approver_ids.len()`.
  - `jurisdiction_tag` хоосон биш байх ба хяналтын тэмдэгт агуулаагүй байх ёстой.
  - `audit_tag` хоосон биш байх ба хяналтын тэмдэгт агуулаагүй байх ёстой.
- Шифр тайлах хүсэлт:
  - `request_id`, `state_key`, `justification` хоосон биш байх ёстой
    (`state_key` нь `/`-ээр эхэлдэг).
  - `jurisdiction_tag` хоосон биш байх ба хяналтын тэмдэгт агуулаагүй байх ёстой.
  - `break_glass_reason` нь `break_glass=true` үед шаардлагатай бөгөөд энэ тохиолдолд орхигдсон байх ёстой.
    `break_glass=false`.
  - бодлоготой холбоотой элсэлт нь бодлогын нэрийн тэгш байдлыг хангадаг, TTL-г хүсээгүй`policy.max_ttl_blocks`-ээс хэтэрсэн, харьяаллын тэгш байдал, шил хагарах
    гарц, болон зөвшөөрөл-нотлох шаардлага хэзээ
    Шилэн хагардаггүй хүсэлтийн хувьд `policy.require_consent_evidence=true`.
- Шифр текстийн асуулгын үзүүлэлт:
  - `state_key_prefix` хоосон биш байх ёстой бөгөөд `/` гэж эхэлнэ.
  - `max_results` нь тодорхой хязгаарлагдмал (`<=256`).
  - мета өгөгдлийн төсөөлөл тодорхой байна (зөвхөн `Minimal` товчлол, `Standard` товчлуур дээр харагдана).
- Шифр текстийн асуулгын хариу:
  - `result_count` нь цуваачилсан мөрийн тоотой тэнцүү байх ёстой.
  - `Minimal` проекц нь `state_key`-ийг ил гаргах ёсгүй; `Standard` үүнийг ил гаргах ёстой.
  - мөрүүд нь хэзээ ч энгийн текст шифрлэлтийн горимд байх ёсгүй.
  - оруулах нотлох баримтууд (хэрэв байгаа бол) хоосон бус схемийн ID-г агуулсан байх ёстой
    `anchor_sequence >= event_sequence`.
- Нууц дугтуй:
  - `key_id`, `nonce`, `ciphertext` хоосон биш байх ёстой.
  - урт нь хязгаарлагдмал (`<=256` байт).
  - шифрлэгдсэн текстийн урт нь хязгаарлагдмал (`<=33554432` байт).
- Шифр текстийн төлөвийн бүртгэл:
  - `state_key` хоосон биш байх ёстой бөгөөд `/` гэж эхэлнэ.
  - мета өгөгдлийн агуулгын төрөл хоосон биш байх ёстой; шошго нь өвөрмөц бус хоосон мөр байх ёстой.
  - `metadata.payload_bytes` `secret.ciphertext.len()`-тэй тэнцүү байх ёстой.
  - `metadata.commitment` `secret.commitment`-тэй тэнцүү байх ёстой.

## Каноник бэхэлгээ

Каноник JSON бэхэлгээг дараах хаягаар хадгална:- `fixtures/soracloud/sora_container_manifest_v1.json`
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

Бэхэлгээний туршилтууд:

- `crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs`
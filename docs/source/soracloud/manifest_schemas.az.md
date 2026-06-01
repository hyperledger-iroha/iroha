<!-- Auto-generated stub for Azerbaijani (az) translation. Replace this content with the full translation. -->

---
lang: az
direction: ltr
source: docs/source/soracloud/manifest_schemas.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a0724ed92da90d8d78a7095b5fc75523ea5dd4a1c885059e28c1bbb8118d1f8c
source_last_modified: "2026-03-26T06:12:11.480497+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# SoraCloud V1 Manifest Sxemləri

Bu səhifə SoraCloud üçün ilk deterministik Norito sxemlərini müəyyən edir.
Iroha 3-də yerləşdirmə:

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

Rust tərifləri `crates/iroha_data_model/src/soracloud.rs`-də yaşayır.

Yüklənmiş model şəxsi iş vaxtı qeydləri qəsdən ayrı bir təbəqədir
bu SCR yerləşdirilməsi təzahür edir. Onlar Soracloud model təyyarəsini uzatmalıdırlar
və şifrələnmiş baytlar üçün `SecretEnvelopeV1` / `CiphertextStateRecordV1`-dən təkrar istifadə edin
və yeni xidmət/konteyner kimi kodlaşdırılmaqdansa, şifrəli mətnin doğma vəziyyəti
təzahür edir. Baxın `uploaded_private_models.md`.

## Əhatə dairəsi

Bu manifestlər `IVM` + xüsusi Sora Konteyner Runtime üçün nəzərdə tutulmuşdur
(SCR) istiqaməti (WASM yoxdur, iş vaxtı qəbulunda Docker asılılığı yoxdur).- `SoraContainerManifestV1` icra edilə bilən paket identifikasiyasını, iş vaxtı növünü ələ keçirir,
  bacarıq siyasəti, resurslar, həyat dövrü araşdırması parametrləri və açıq
  tələb olunan konfiqurasiya iş vaxtı mühitinə və ya quraşdırılmış revizyona ixrac
  ağac.
- `SoraServiceManifestV1` yerləşdirmə niyyətini ələ keçirir: xidmət şəxsiyyəti,
  istinad edilmiş konteyner manifest hash/versiya, marşrutlaşdırma, rollout siyasəti və
  dövlət bağlamaları.
- `SoraStateBindingV1` deterministik dövlət yazma sahəsini və məhdudiyyətlərini ələ keçirir
  (ad sahəsi prefiksi, dəyişkənlik rejimi, şifrələmə rejimi, element/ümumi kvotalar).
- `SoraDeploymentBundleV1` konteyner + xidmət manifestlərini birləşdirir və tətbiq edir
  deterministik qəbul yoxlamaları (manifest-hash əlaqəsi, sxemlərin uyğunlaşdırılması və
  qabiliyyəti/bağlayıcı ardıcıllıq).
- `AgentApartmentManifestV1` davamlı agent işləmə zamanı siyasətini ələ keçirir:
  alət qapaqları, siyasət hədləri, xərcləmə limitləri, dövlət kvotası, şəbəkə çıxışı və
  davranışını təkmilləşdirmək.
- `FheParamSetV1` idarəetmə tərəfindən idarə olunan FHE parametr dəstlərini çəkir:
  deterministic backend/sxem identifikatorları, modul profili, təhlükəsizlik/dərinlik
  sərhədlər və həyat dövrü yüksəklikləri (`activation`/`deprecation`/`withdraw`).
- `FheExecutionPolicyV1` deterministik şifrəli mətn icra məhdudiyyətlərini ələ keçirir:
  qəbul edilmiş yük ölçüləri, giriş/çıxış fan-in, dərinlik/fırlanma/bootstrap qapaqları,
  və kanonik yuvarlaqlaşdırma rejimi.
- `FheGovernanceBundleV1` deterministik üçün parametr dəsti və siyasəti birləşdirir
  qəbulun təsdiqi.- `FheJobSpecV1` deterministik şifrəli mətn işə qəbulu/icrasını çəkir
  sorğular: əməliyyat sinfi, sifarişli giriş öhdəlikləri, çıxış açarı və məhdud
  siyasət + parametr dəsti ilə əlaqəli dərinlik/fırlanma/bootstrap tələbi.
- `DecryptionAuthorityPolicyV1` idarəetmə tərəfindən idarə olunan açıqlama siyasətini əhatə edir:
  səlahiyyət rejimi (müştəri tərəfindən saxlanılan və həddi xidmət), təsdiqləyici kvorum/üzvlər,
  şüşə sındırmaq üçün müavinət, yurisdiksiyanın etiketlənməsi, razılıq-sübut tələbi,
  TTL sərhədləri və kanonik audit etiketləməsi.
- `DecryptionRequestV1` siyasətlə əlaqəli açıqlama cəhdlərini ələ keçirir:
  şifrəli mətn açarına istinad (`binding_name` + `state_key` + öhdəlik),
  əsaslandırma, yurisdiksiya etiketi, isteğe bağlı razılıq-sübut hash, TTL,
  şüşə sındırmaq niyyəti/səbəbi və idarəetmə hash əlaqəsi.
- `CiphertextQuerySpecV1` yalnız deterministik şifrəli mətn sorğu niyyətini ələ keçirir:
  xidmət/məcburi əhatə dairəsi, açar-prefiks filtri, məhdud nəticə limiti, metadata
  proyeksiya səviyyəsi və sübut daxiletmə keçidi.
- `CiphertextQueryResponseV1` açıqlama ilə minimuma endirilmiş sorğu nəticələrini çəkir:
  həzm yönümlü əsas istinadlar, şifrəli mətn metadata, isteğe bağlı daxiletmə sübutları,
  və cavab səviyyəli kəsilmə/ardıcıllıq konteksti.
- `SecretEnvelopeV1` şifrələnmiş faydalı yük materialının özünü çəkir:
  şifrələmə rejimi, açar identifikatoru/versiya, nonce, şifrəli mətn baytları və
  dürüstlük öhdəlikləri.
- `CiphertextStateRecordV1` şifrəli mətnin yerli dövlət qeydlərini tuturictimai metadataları birləşdirin (məzmun növü, siyasət teqləri, öhdəlik, yükün ölçüsü)
  `SecretEnvelopeV1` ilə.
- İstifadəçi tərəfindən yüklənmiş şəxsi model paketləri bu şifrəli mətn əsasında qurulmalıdır
  qeydlər:
  şifrlənmiş çəki/konfiqurasiya/prosessor parçaları vəziyyətdə yaşayır, model qeydi isə,
  çəki xətti, tərtib profilləri, çıxarış seansları və yoxlama nöqtələri qalır
  birinci dərəcəli Soracloud qeydləri.

## Versiyalaşdırma

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

Doğrulama dəstəklənməyən versiyaları rədd edir
`SoraCloudManifestError::UnsupportedVersion`.

## Deterministik Doğrulama Qaydaları (V1)- Konteyner manifest:
  - `bundle_path` və `entrypoint` boş olmamalıdır.
  - `healthcheck_path` (əgər quraşdırılıbsa) `/` ilə başlamalıdır.
  - `config_exports` yalnız elan edilmiş konfiqurasiyalara istinad edə bilər
    `required_config_names`.
  - config-export env hədəfləri kanonik mühit dəyişən adlarından istifadə etməlidir
    (`[A-Za-z_][A-Za-z0-9_]*`).
  - konfiqurasiya-ixrac faylı hədəfləri nisbi qalmalı, `/` ayırıcılarından istifadə etməli və
    boş, `.` və ya `..` seqmentləri ehtiva etməməlidir.
  - konfiqurasiya ixracı eyni env var və ya nisbi fayl yolunu daha çox hədəfləməməlidir
    bir dəfədən çox.
- Xidmət manifest:
  - `service_version` boş olmamalıdır.
  - `container.expected_schema_version` konteyner sxemi v1 uyğun olmalıdır.
  - `rollout.canary_percent` `0..=100` olmalıdır.
  - `route.path_prefix` (əgər quraşdırılıbsa) `/` ilə başlamalıdır.
  - dövlət bağlayıcı adlar unikal olmalıdır.
- Dövlət məcburiliyi:
  - `key_prefix` boş olmamalıdır və `/` ilə başlamalıdır.
  - `max_item_bytes <= max_total_bytes`.
  - `ConfidentialState` bağlamaları açıq mətn şifrələməsindən istifadə edə bilməz.
- Yerləşdirmə paketi:
  - `service.container.manifest_hash` kodlaşdırılmış kanoniklə uyğun olmalıdır
    konteyner manifest hash.
  - `service.container.expected_schema_version` konteyner sxeminə uyğun olmalıdır.
  - Dəyişən dövlət bağlamaları `container.capabilities.allow_state_writes=true` tələb edir.
  - İctimai marşrutlar `container.lifecycle.healthcheck_path` tələb edir.
- Agent mənzil manifestosu:
  - `container.expected_schema_version` konteyner sxeminə v1 uyğun gəlməlidir.
  - alət imkanlarının adları boş və unikal olmalıdır.- siyasət imkanlarının adları unikal olmalıdır.
  - xərc limiti aktivləri boş olmamalı və unikal olmalıdır.
  - Hər xərc limiti üçün `max_per_tx_nanos <= max_per_day_nanos`.
  - icazə siyahısı şəbəkə siyasətinə unikal qeyri-boş hostlar daxil edilməlidir.
- FHE parametr dəsti:
  - `backend` və `ciphertext_modulus_bits` boş olmamalıdır.
  - hər bir şifrəli mətn modulunun bit ölçüsü `2..=120` daxilində olmalıdır.
  - şifrəli mətn modulu zənciri sırası artan olmamalıdır.
  - `plaintext_modulus_bits` ən böyük şifrəli mətn modulundan kiçik olmalıdır.
  - `slot_count <= polynomial_modulus_degree`.
  - `max_multiplicative_depth < ciphertext_modulus_bits.len()`.
  - həyat dövrünün hündürlüyü sifarişi ciddi olmalıdır:
    `activation < deprecation < withdraw` mövcud olduqda.
  - həyat dövrü statusu tələbləri:
    - `Proposed` yüksəkliklərin köhnəlməsinə/çıxarılmasına icazə vermir.
    - `Active` `activation_height` tələb edir.
    - `Deprecated` `activation_height` + `deprecation_height` tələb edir.
    - `Withdrawn` `activation_height` + `withdraw_height` tələb edir.
- FHE icra siyasəti:
  - `max_plaintext_bytes <= max_ciphertext_bytes`.
  - `max_output_ciphertexts <= max_input_ciphertexts`.
  - parametr dəsti bağlaması `(param_set, version)` ilə uyğun olmalıdır.
  - `max_multiplication_depth` parametr təyin edilmiş dərinliyi keçməməlidir.
  - siyasət qəbulu `Proposed` və ya `Withdrawn` parametr setinin həyat dövrünü rədd edir.
- FHE idarəetmə paketi:
  - bir deterministik qəbul yükü kimi siyasət + parametr dəsti uyğunluğunu təsdiqləyir.
- FHE iş xüsusiyyətləri:
  - `job_id` və `output_state_key` boş olmamalıdır (`output_state_key` `/` ilə başlayır).- giriş dəsti boş olmamalıdır və giriş düymələri unikal kanonik yollar olmalıdır.
  - əməliyyat üçün xüsusi məhdudiyyətlər ciddidir (`Add`/`Multiply` çox giriş,
    `RotateLeft`/`Bootstrap` tək giriş, qarşılıqlı eksklüziv dərinlik/fırlanma/bootstrap düymələri ilə).
  - siyasətlə əlaqəli qəbulu həyata keçirir:
    - siyasət/param identifikatorları və versiyaları uyğun gəlir.
    - giriş sayı/bayt, dərinlik, fırlanma və yükləmə məhdudiyyətləri siyasət hədləri daxilindədir.
    - deterministik proqnozlaşdırılan çıxış baytları siyasət şifrəli mətn məhdudiyyətlərinə uyğundur.
- Şifrə açma səlahiyyəti siyasəti:
  - `approver_ids` boş olmamalıdır, unikal olmalıdır və ciddi şəkildə leksikoqrafik olaraq çeşidlənməlidir.
  - `ClientHeld` rejimi tam olaraq bir təsdiqləyici tələb edir, `approver_quorum=1`,
    və `allow_break_glass=false`.
  - `ThresholdService` rejimi ən azı iki təsdiqləyici və tələb edir
    `approver_quorum <= approver_ids.len()`.
  - `jurisdiction_tag` boş olmamalıdır və nəzarət simvollarını ehtiva etməməlidir.
  - `audit_tag` boş olmamalıdır və nəzarət simvollarını ehtiva etməməlidir.
- Şifrənin açılması sorğusu:
  - `request_id`, `state_key` və `justification` boş olmamalıdır
    (`state_key` `/` ilə başlayır).
  - `jurisdiction_tag` boş olmamalıdır və nəzarət simvollarını ehtiva etməməlidir.
  - `break_glass_reason`, `break_glass=true` olduqda tələb olunur və bu zaman buraxılmalıdır
    `break_glass=false`.
  - siyasətlə əlaqəli qəbul siyasət-ad bərabərliyini təmin edir, TTL tələb etmir`policy.max_ttl_blocks`-dən çox, yurisdiksiya etiketi bərabərliyi, şüşə sındırmaq
    gating, və razılıq-sübut tələbləri zaman
    Şüşə sınmayan sorğular üçün `policy.require_consent_evidence=true`.
- Şifrətli mətn sorğusu xüsusiyyətləri:
  - `state_key_prefix` boş olmamalıdır və `/` ilə başlamalıdır.
  - `max_results` deterministik olaraq məhduddur (`<=256`).
  - metadata proyeksiyası açıqdır (yalnız `Minimal` həzm və `Standard` açarı görünür).
- Şifrətli mətn sorğusuna cavab:
  - `result_count` seriyalı sıra sayına bərabər olmalıdır.
  - `Minimal` proyeksiyası `state_key`-i ifşa etməməlidir; `Standard` onu ifşa etməlidir.
  - sətirlər heç vaxt açıq mətn şifrələmə rejiminin səthinə çıxmamalıdır.
  - daxil edilmə sübutları (mövcud olduqda) boş olmayan sxem identifikatorlarını və daxil etməlidir
    `anchor_sequence >= event_sequence`.
- Gizli zərf:
  - `key_id`, `nonce` və `ciphertext` boş olmamalıdır.
  - qeyri-məhdud uzunluq (`<=256` bayt).
  - şifrəli mətn uzunluğu məhduddur (`<=33554432` bayt).
- Şifrətli mətn vəziyyəti qeydi:
  - `state_key` boş olmamalıdır və `/` ilə başlamalıdır.
  - metadata məzmun növü boş olmamalıdır; teqlər unikal qeyri-boş sətirlər olmalıdır.
  - `metadata.payload_bytes` `secret.ciphertext.len()`-ə bərabər olmalıdır.
  - `metadata.commitment` `secret.commitment`-ə bərabər olmalıdır.

## Kanonik qurğular

Kanonik JSON qurğuları aşağıdakı ünvanda saxlanılır:- `fixtures/soracloud/sora_container_manifest_v1.json`
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

Quraşdırma/gediş-dönüş testləri:

- `crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs`
<!-- Auto-generated stub for Uzbek (uz) translation. Replace this content with the full translation. -->

---
lang: uz
direction: ltr
source: docs/source/soracloud/manifest_schemas.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a0724ed92da90d8d78a7095b5fc75523ea5dd4a1c885059e28c1bbb8118d1f8c
source_last_modified: "2026-03-26T06:12:11.480497+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# SoraCloud V1 manifest sxemalari

Bu sahifa SoraCloud uchun birinchi deterministik Norito sxemalarini belgilaydi
Iroha 3 da joylashtirish:

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

Rust ta'riflari `crates/iroha_data_model/src/soracloud.rs` da yashaydi.

Yuklangan modeldagi shaxsiy ish vaqti yozuvlari ataylab alohida qatlam hisoblanadi
bu SCR joylashuvi namoyon bo'ladi. Ular Soracloud model samolyotini kengaytirishlari kerak
va shifrlangan baytlar uchun `SecretEnvelopeV1` / `CiphertextStateRecordV1` dan qayta foydalaning
va yangi xizmat/konteyner sifatida kodlangandan ko'ra, shifrlangan matnning mahalliy holati
namoyon bo'ladi. `uploaded_private_models.md` ga qarang.

## Qo'llash doirasi

Ushbu manifestlar `IVM` + maxsus Sora Container Runtime uchun mo'ljallangan.
(SCR) yo'nalishi (WASM yo'q, ish vaqtiga kirishda Docker bog'liqligi yo'q).- `SoraContainerManifestV1` bajariladigan paket identifikatorini, ish vaqti turini oladi,
  qobiliyat siyosati, resurslar, hayot aylanishini tekshirish sozlamalari va aniq
  zarur konfiguratsiyani ish vaqti muhitiga eksport qilish yoki o'rnatilgan tahrirlash
  daraxt.
- `SoraServiceManifestV1` joylashtirish maqsadini aniqlaydi: xizmat identifikatori,
  havola qilingan konteyner manifest xesh/versiyasi, marshrutlash, tarqatish siyosati va
  davlat bog'lashlari.
- `SoraStateBindingV1` deterministik holat-yozish doirasi va chegaralarini ushlaydi
  (nom maydoni prefiksi, o'zgaruvchanlik rejimi, shifrlash rejimi, element/jami kvotalar).
- `SoraDeploymentBundleV1` konteyner + xizmat ko'rsatish manifestlarini birlashtiradi va amalga oshiradi
  deterministik qabul tekshiruvlari (manifest-xesh aloqasi, sxemani moslashtirish va
  qobiliyat/majburiy izchillik).
- `AgentApartmentManifestV1` doimiy agent ish vaqti siyosatini yozib oladi:
  asboblar chegaralari, siyosat cheklovlari, xarajat chegaralari, davlat kvotasi, tarmoqdan chiqish va
  xatti-harakatni yangilash.
- `FheParamSetV1` boshqaruv tomonidan boshqariladigan FHE parametr to'plamlarini qamrab oladi:
  deterministik backend/sxema identifikatorlari, modul profili, xavfsizlik/chuqurlik
  chegaralar va hayot aylanishi balandliklari (`activation`/`deprecation`/`withdraw`).
- `FheExecutionPolicyV1` deterministik shifrlangan matnni bajarish chegaralarini ushlaydi:
  ruxsat etilgan yuk hajmi, kirish/chiqish fan-in, chuqurlik/aylanish/bootstrap qopqoqlari,
  va kanonik yaxlitlash rejimi.
- `FheGovernanceBundleV1` deterministik uchun parametrlar to'plami va siyosatni birlashtiradi
  kirishni tasdiqlash.- `FheJobSpecV1` deterministik shifrlangan ishni qabul qilish/bajarishni yozib oladi
  so'rovlar: operatsiya sinfi, tartiblangan kirish majburiyatlari, chiqish kaliti va chegaralangan
  siyosat + parametrlar to'plamiga bog'langan chuqurlik/aylanish/bootstrap talabi.
- `DecryptionAuthorityPolicyV1` boshqaruv tomonidan boshqariladigan oshkor qilish siyosatini qamrab oladi:
  vakolat rejimi (mijoz tomonidan ushlab turiladigan va chegara xizmati), tasdiqlovchi kvorum/a'zolar,
  shisha sindirish uchun nafaqa, yurisdiktsiya belgisi, rozilik-dalil talabi,
  TTL chegaralari va kanonik audit teglari.
- `DecryptionRequestV1` siyosat bilan bog'liq oshkor qilish urinishlarini qamrab oladi:
  shifrlangan matn kalitiga havola (`binding_name` + `state_key` + majburiyat),
  asoslash, yurisdiktsiya yorlig'i, ixtiyoriy rozilik-dalil xeshi, TTL,
  sindirish niyati/sabab va boshqaruv xesh aloqasi.
- `CiphertextQuerySpecV1` faqat deterministik shifrlangan matnli so'rovlar maqsadini qamrab oladi:
  xizmat/bog'lash doirasi, kalit-prefiks filtri, cheklangan natija chegarasi, metama'lumotlar
  proyeksiya darajasi va dalil inklyuziyasini almashtirish.
- `CiphertextQueryResponseV1` oshkor etilishi minimallashtirilgan so'rov natijalarini oladi:
  digestga yo'naltirilgan kalit havolalar, shifrlangan matn metama'lumotlari, qo'shimcha kiritish dalillari,
  va javob darajasidagi kesish/ketma-ketlik konteksti.
- `SecretEnvelopeV1` shifrlangan foydali yuk materialining o'zini ushlaydi:
  shifrlash rejimi, kalit identifikatori/versiyasi, nonce, shifrlangan matn baytlari va
  halollik majburiyatlari.
- `CiphertextStateRecordV1` shifrlangan matnning mahalliy holatini qayd qiladiumumiy metadata (kontent turi, siyosat teglari, majburiyat, foydali yuk hajmi)
  `SecretEnvelopeV1` bilan.
- Foydalanuvchi tomonidan yuklangan shaxsiy model to'plamlari ushbu shifrlangan matn asosida tuzilishi kerak
  yozuvlar:
  shifrlangan vazn/konfiguratsiya/protsessor bo'laklari holatda yashaydi, model registrida esa,
  vazn nasl-nasabi, kompilyatsiya profillari, xulosalar seanslari va nazorat nuqtalari qoladi
  birinchi darajali Soracloud yozuvlari.

## Versiyalash

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

Tasdiqlash qo'llab-quvvatlanmaydigan versiyalarni rad etadi
`SoraCloudManifestError::UnsupportedVersion`.

## Deterministik tasdiqlash qoidalari (V1)- Konteyner manifest:
  - `bundle_path` va `entrypoint` bo'sh bo'lmasligi kerak.
  - `healthcheck_path` (agar o'rnatilgan bo'lsa) `/` bilan boshlanishi kerak.
  - `config_exports` faqat e'lon qilingan konfiguratsiyalarga murojaat qilishi mumkin
    `required_config_names`.
  - config-export env maqsadlarida kanonik muhit o'zgaruvchilari nomlaridan foydalanish kerak
    (`[A-Za-z_][A-Za-z0-9_]*`).
  - konfiguratsiya-eksport fayl maqsadlari nisbiy qolishi, `/` ajratgichlaridan foydalanishi va
    bo'sh, `.` yoki `..` segmentlarini o'z ichiga olmaydi.
  - konfiguratsiya eksporti bir xil env var yoki nisbiy fayl yo'lini ko'proq maqsad qilib qo'ymasligi kerak
    bir martadan ko'ra.
- Xizmat manifest:
  - `service_version` bo'sh bo'lmasligi kerak.
  - `container.expected_schema_version` konteyner sxemasi v1 bilan mos kelishi kerak.
  - `rollout.canary_percent` `0..=100` bo'lishi kerak.
  - `route.path_prefix` (agar o'rnatilgan bo'lsa) `/` bilan boshlanishi kerak.
  - davlat bog'lovchi nomlar yagona bo'lishi kerak.
- Davlat majburiyati:
  - `key_prefix` bo'sh bo'lmasligi va `/` bilan boshlanishi kerak.
  - `max_item_bytes <= max_total_bytes`.
  - `ConfidentialState` ulanishlari ochiq matn shifrlashdan foydalana olmaydi.
- Joylashtirish to'plami:
  - `service.container.manifest_hash` kanonik kodlanganga mos kelishi kerak
    konteyner manifest xeshi.
  - `service.container.expected_schema_version` konteyner sxemasiga mos kelishi kerak.
  - O'zgaruvchan holat ulanishlari uchun `container.capabilities.allow_state_writes=true` talab qilinadi.
  - Umumiy yoʻnalishlar uchun `container.lifecycle.healthcheck_path` talab qilinadi.
- Agent kvartira manifest:
  - `container.expected_schema_version` konteyner sxemasi v1 bilan mos kelishi kerak.
  - asbob imkoniyatlari nomlari bo'sh bo'lmagan va yagona bo'lishi kerak.- siyosat qobiliyatlari nomlari noyob bo'lishi kerak.
  - xarajat limiti aktivlari bo'sh bo'lmagan va noyob bo'lishi kerak.
  - Har bir sarf limiti uchun `max_per_tx_nanos <= max_per_day_nanos`.
  - ruxsat etilgan ro'yxat tarmoq siyosati bo'sh bo'lmagan noyob xostlarni o'z ichiga olishi kerak.
- FHE parametrlari to'plami:
  - `backend` va `ciphertext_modulus_bits` bo'sh bo'lmasligi kerak.
  - har bir shifrlangan matn moduli bit o'lchami `2..=120` ichida bo'lishi kerak.
  - shifrlangan matn moduli zanjiri tartibi ortib ketmasligi kerak.
  - `plaintext_modulus_bits` eng katta shifrlangan matn modulidan kichikroq bo'lishi kerak.
  - `slot_count <= polynomial_modulus_degree`.
  - `max_multiplicative_depth < ciphertext_modulus_bits.len()`.
  - hayot tsiklining balandligi buyurtmasi qat'iy bo'lishi kerak:
    `activation < deprecation < withdraw` mavjud bo'lganda.
  - hayot aylanishi holatiga qo'yiladigan talablar:
    - `Proposed` eskirish/olib tashlash balandligini taqiqlaydi.
    - `Active` uchun `activation_height` talab qilinadi.
    - `Deprecated` uchun `activation_height` + `deprecation_height` talab qilinadi.
    - `Withdrawn` uchun `activation_height` + `withdraw_height` talab qilinadi.
- FHE ijro siyosati:
  - `max_plaintext_bytes <= max_ciphertext_bytes`.
  - `max_output_ciphertexts <= max_input_ciphertexts`.
  - parametrlar to'plamini ulash `(param_set, version)` ga mos kelishi kerak.
  - `max_multiplication_depth` parametr o'rnatilgan chuqurlikdan oshmasligi kerak.
  - siyosatni qabul qilish `Proposed` yoki `Withdrawn` parametrlari to'plamining hayot aylanishini rad etadi.
- FHE boshqaruv to'plami:
  - bitta deterministik kirish yuki sifatida siyosat + parametrlar to'plami muvofiqligini tasdiqlaydi.
- FHE ish xususiyatlari:
  - `job_id` va `output_state_key` bo'sh bo'lmasligi kerak (`output_state_key` `/` bilan boshlanadi).- kiritish to'plami bo'sh bo'lmasligi kerak va kirish tugmalari noyob kanonik yo'llar bo'lishi kerak.
  - operatsiyaga xos cheklovlar qat'iy (`Add`/`Multiply` ko'p kirish,
    `RotateLeft`/`Bootstrap` bitta kirish, o'zaro eksklyuziv chuqurlik/aylanish/bootstrap tugmalari bilan).
  - siyosat bilan bog'liq qabul qilish:
    - siyosat/param identifikatorlari va versiyalari mos keladi.
    - kirish soni/baytlar, chuqurlik, aylanish va yuklash chegaralari siyosat chegaralari ichida.
    - deterministik prognozli chiqish baytlari siyosatning shifrlangan matn chegaralariga mos keladi.
- shifrni ochish vakolati siyosati:
  - `approver_ids` bo'sh bo'lmagan, noyob va qat'iy leksikografik tartiblangan bo'lishi kerak.
  - `ClientHeld` rejimi aynan bitta tasdiqlovchini talab qiladi, `approver_quorum=1`,
    va `allow_break_glass=false`.
  - `ThresholdService` rejimi kamida ikkita tasdiqlovchini talab qiladi va
    `approver_quorum <= approver_ids.len()`.
  - `jurisdiction_tag` bo'sh bo'lmasligi va boshqaruv belgilarini o'z ichiga olmaydi.
  - `audit_tag` bo'sh bo'lmasligi va boshqaruv belgilarini o'z ichiga olmaydi.
- shifrni ochish so'rovi:
  - `request_id`, `state_key` va `justification` bo'sh bo'lmasligi kerak
    (`state_key` `/` bilan boshlanadi).
  - `jurisdiction_tag` bo'sh bo'lmasligi va boshqaruv belgilarini o'z ichiga olmaydi.
  - `break_glass_reason`, `break_glass=true` bo'lganda talab qilinadi va qachon o'tkazib yuborilishi kerak
    `break_glass=false`.
  - siyosat bilan bog'liq qabul siyosat nomi tengligini ta'minlaydi, TTL so'ramaydi`policy.max_ttl_blocks` dan yuqori, yurisdiktsiya yorlig'i tengligi, shisha sindirish
    gating, va rozilik-dalil talablar qachon
    Shisha sinmaydigan so'rovlar uchun `policy.require_consent_evidence=true`.
- shifrlangan matn so'rovi spetsifikatsiyasi:
  - `state_key_prefix` bo'sh bo'lmasligi va `/` bilan boshlanishi kerak.
  - `max_results` deterministik chegaralangan (`<=256`).
  - metadata proektsiyasi aniq (faqat `Minimal` dayjest va `Standard` kalitda ko'rinadi).
- shifrlangan matn so'roviga javob:
  - `result_count` seriyali qatorlar soniga teng bo'lishi kerak.
  - `Minimal` proyeksiyasi `state_key` ni ko'rsatmasligi kerak; `Standard` uni ochishi kerak.
  - satrlar hech qachon ochiq matnni shifrlash rejimiga tushmasligi kerak.
  - qo'shish dalillari (mavjud bo'lsa) bo'sh bo'lmagan sxema identifikatorlarini va o'z ichiga olishi kerak
    `anchor_sequence >= event_sequence`.
- maxfiy konvert:
  - `key_id`, `nonce` va `ciphertext` bo'sh bo'lmasligi kerak.
  - bir martalik uzunlik chegaralangan (`<=256` bayt).
  - shifrlangan matn uzunligi chegaralangan (`<=33554432` bayt).
- shifrlangan matn holati yozuvi:
  - `state_key` bo'sh bo'lmasligi va `/` bilan boshlanishi kerak.
  - metadata kontent turi bo'sh bo'lmasligi kerak; teglar noyob bo'sh bo'lmagan satrlar bo'lishi kerak.
  - `metadata.payload_bytes` `secret.ciphertext.len()` ga teng bo'lishi kerak.
  - `metadata.commitment` `secret.commitment` ga teng bo'lishi kerak.

## Kanonik armatura

Kanonik JSON moslamalari quyidagi manzilda saqlanadi:- `fixtures/soracloud/sora_container_manifest_v1.json`
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

Fikstur/qaytish sinovlari:

- `crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs`
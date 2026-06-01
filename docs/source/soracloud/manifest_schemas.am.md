<!-- Auto-generated stub for Amharic (Ethiopian) (am) translation. Replace this content with the full translation. -->

---
lang: am
direction: ltr
source: docs/source/soracloud/manifest_schemas.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a0724ed92da90d8d78a7095b5fc75523ea5dd4a1c885059e28c1bbb8118d1f8c
source_last_modified: "2026-03-26T06:12:11.480497+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# SoraCloud V1 ገላጭ እቅዶች

ይህ ገጽ ለሶራክላውድ የመጀመሪያውን መወሰኛ Norito መርሃግብሮችን ይገልጻል።
በ Iroha 3 ላይ ማሰማራት፡

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

የ Rust ትርጓሜዎች በ `crates/iroha_data_model/src/soracloud.rs` ውስጥ ይኖራሉ።

የተሰቀሉ-ሞዴል የግል-አሂድ ጊዜ መዝገቦች ሆን ተብሎ የተለየ ንብርብር ናቸው።
እነዚህ የ SCR መሰማራት ይገለጻል። የሶራክሎድ ሞዴል አውሮፕላን ማራዘም አለባቸው
እና ለተመሰጠረ ባይት `SecretEnvelopeV1`/`CiphertextStateRecordV1` እንደገና ተጠቀም
እንደ አዲስ አገልግሎት/መያዣ ከመሆን ይልቅ የምስጥር ጽሑፍ-ቤተኛ ሁኔታ
ይገለጣል። `uploaded_private_models.md` ይመልከቱ።

## ወሰን

እነዚህ መግለጫዎች የተነደፉት ለ`IVM` + ብጁ የሶራ ኮንቴይነር የሩጫ ጊዜ ነው።
(SCR) አቅጣጫ (WASM የለም፣ በአሂድ ጊዜ መግቢያ ላይ Docker ጥገኝነት የለም)።- `SoraContainerManifestV1` የሚተገበር የጥቅል ማንነት ፣ የአሂድ ጊዜ አይነት ፣
  የችሎታ ፖሊሲ፣ ግብዓቶች፣ የህይወት ኡደት ፍተሻ ቅንብሮች እና ግልጽ
  ተፈላጊ-ውቅር ወደ የሩጫ ጊዜ አካባቢ ወይም የተገጠመ ክለሳ ወደ ውጭ ይልካል
  ዛፍ.
- `SoraServiceManifestV1` የማሰማራት ዓላማን ይይዛል፡ የአገልግሎት ማንነት፣
  የተጠቀሰው መያዣ አንጸባራቂ ሃሽ/ስሪት፣ ማዘዋወር፣ መልቀቅ ፖሊሲ እና
  የግዛት ማሰሪያዎች.
- `SoraStateBindingV1` የሚወስን የስቴት-ጽሑፍ ወሰን እና ገደቦችን ይይዛል
  (ስም ቦታ ቅድመ ቅጥያ፣ ተለዋዋጭነት ሁነታ፣ የምስጠራ ሁነታ፣ ንጥል/ጠቅላላ ኮታዎች)።
- `SoraDeploymentBundleV1` ባለትዳሮች መያዣ + አገልግሎት ይገለጣል እና ያስፈጽማል
  የሚወስን የመግቢያ ፍተሻዎች (አንጸባራቂ-ሃሽ ትስስር፣ የመርሃግብር አሰላለፍ እና
  የችሎታ / አስገዳጅ ወጥነት).
- `AgentApartmentManifestV1` ቋሚ ወኪል የአሂድ ጊዜ ፖሊሲን ይይዛል፡
  የመሳሪያ ካፕ፣ የፖሊሲ ክዳን፣ የወጪ ገደቦች፣ የግዛት ኮታ፣ የአውታረ መረብ መውጫ እና
  ባህሪን ማሻሻል.
- `FheParamSetV1` በአስተዳደር የሚተዳደሩ የFHE መለኪያ ስብስቦችን ይይዛል፡
  የሚወስን የኋላ / የመርሃግብር መለያዎች ፣ ሞጁሎች መገለጫ ፣ ደህንነት / ጥልቀት
  ወሰኖች፣ እና የህይወት ኡደት ቁመቶች (`activation`/`deprecation`/`withdraw`)።
- `FheExecutionPolicyV1` የሚወስን የምሥጥር ጽሑፍ አፈጻጸም ገደቦችን ይይዛል፡
  የተቀበሉት የመጫኛ መጠኖች፣ የግቤት/ውፅዓት ደጋፊ-ውስጥ፣ ጥልቀት/ማሽከርከር/የቡት ማሰሪያ፣
  እና ቀኖናዊ የማዞሪያ ሁነታ.
- `FheGovernanceBundleV1` ጥንዶች መለኪያ ስብስብ እና የመወሰን ፖሊሲ
  የመግቢያ ማረጋገጫ.- `FheJobSpecV1` የሚወስን የምሥጥር ጽሑፍ ሥራ መግቢያ/አፈፃፀምን ይይዛል
  ጥያቄዎች፡ የክወና ክፍል፣ የታዘዙ የግቤት ቁርጠኝነት፣ የውጤት ቁልፍ እና የተገደበ
  ከፖሊሲ + መለኪያ ስብስብ ጋር የተገናኘ ጥልቀት/ማሽከርከር/የቡት ማንጠልጠያ ፍላጎት።
- `DecryptionAuthorityPolicyV1` በአስተዳደር የሚተዳደር የግልጽነት ፖሊሲን ይይዛል፡-
  ባለስልጣን ሁነታ (በደንበኛ የተያዘ እና የመነሻ አገልግሎት)፣ የድጋፍ ምልአተ ጉባኤ/አባላት፣
  የመስበር መስታወት አበል፣ ስልጣን መለያ መስጠት፣ የፍቃድ-ማስረጃ መስፈርት፣
  የቲቲኤል ገደቦች፣ እና ቀኖናዊ ኦዲት መለያ መስጠት።
- `DecryptionRequestV1` ከፖሊሲ ጋር የተገናኙ የግልጽነት ሙከራዎችን ይይዛል፡-
  የምስጥር ጽሑፍ ቁልፍ ማጣቀሻ (`binding_name` + `state_key` + ቁርጠኝነት) ፣
  ማረጋገጫ፣ የዳኝነት መለያ፣ አማራጭ ፈቃድ-ማስረጃ ሃሽ፣ ቲቲኤል፣
  የመስበር-መስታወት ዓላማ/ምክንያት እና የአስተዳደር ሃሽ ትስስር።
- `CiphertextQuerySpecV1` የሚወስን የምስክሪፕት ጽሑፍ-ብቻ መጠይቅ ሐሳብ ይይዛል፡
  የአገልግሎት/የማሰሪያ ወሰን፣የቁልፍ ቅድመ ቅጥያ ማጣሪያ፣የተወሰነ የውጤት ገደብ፣ሜታዳታ
  የትንበያ ደረጃ፣ እና የማስረጃ ማካተት መቀያየር።
- `CiphertextQueryResponseV1` ይፋ-አነስተኛ የመጠይቅ ውጽዓቶችን ይይዛል፡
  መፈጨት ተኮር ቁልፍ ማጣቀሻዎች፣ የምስጥር ጽሑፍ ዲበ ውሂብ፣ አማራጭ የማካተት ማረጋገጫዎች፣
  እና የምላሽ ደረጃ መቆራረጥ/የቅደም ተከተል አውድ።
- `SecretEnvelopeV1` ኢንክሪፕት የተደረገ የመክፈያ ቁሳቁስ ራሱ ይይዛል፡-
  ምስጠራ ሁነታ፣ ቁልፍ ለዪ/ስሪት፣ ኖንስ፣ የምስጠራ ባይት፣ እና
  የታማኝነት ቁርጠኝነት.
- `CiphertextStateRecordV1` የምስጥር ጽሑፍ-ቤተኛ ግዛት ግቤቶችን ይይዛልይፋዊ ሜታዳታን አጣምር (የይዘት አይነት፣ የመመሪያ መለያዎች፣ ቁርጠኝነት፣ የመጫኛ መጠን)
  ከ `SecretEnvelopeV1` ጋር።
- በተጠቃሚ የተጫኑ የግል ሞዴል ቅርቅቦች በእነዚህ የምስጥር ጽሑፍ-ቤተኛ ላይ መገንባት አለባቸው
  መዝገቦች፡
  የተመሰጠረ የክብደት/የማዋቀር/የፕሮሰሰር ቁርጥራጮች በግዛት ይኖራሉ፣ የሞዴል መዝገብ ሳለ፣
  የክብደት መስመር፣ መገለጫዎችን ያጠናቅራል፣ የመግቢያ ክፍለ ጊዜዎች እና የፍተሻ ነጥቦች ይቀራሉ
  የመጀመሪያ ደረጃ የሶራክሎድ መዝገቦች.

## ስሪት ማውጣት

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

ማረጋገጫ ከ ጋር የማይደገፉ ስሪቶችን ውድቅ ያደርጋል
`SoraCloudManifestError::UnsupportedVersion`.

## የማረጋገጫ ደንቦች (V1)- የመያዣ መግለጫ;
  - `bundle_path` እና `entrypoint` ባዶ ያልሆኑ መሆን አለባቸው።
  - `healthcheck_path` (ከተዋቀረ) በ `/` መጀመር አለበት።
  - `config_exports` የተገለጹ ውቅሮችን ብቻ ሊያመለክት ይችላል።
    `required_config_names`.
  - config-export env ኢላማዎች ቀኖናዊ አካባቢ-ተለዋዋጭ ስሞችን መጠቀም አለባቸው
    (`[A-Za-z_][A-Za-z0-9_]*`)።
  - የማዋቀር-የመላክ ፋይል ኢላማዎች አንጻራዊ መሆን አለባቸው፣ `/` መለያዎችን ይጠቀሙ እና
    ባዶ፣ `.` ወይም `..` ክፍሎችን መያዝ የለበትም።
  - config ወደ ውጭ መላክ ተመሳሳይ env var ወይም አንጻራዊ የፋይል መንገድን የበለጠ ማነጣጠር የለበትም
    ከአንድ ጊዜ በላይ.
- የአገልግሎት መግለጫ;
  - `service_version` ባዶ ያልሆነ መሆን አለበት።
  - `container.expected_schema_version` የመያዣ ንድፍ v1 መዛመድ አለበት.
  - `rollout.canary_percent` `0..=100` መሆን አለበት።
  - `route.path_prefix` (ከተዋቀረ) በ `/` መጀመር አለበት።
  - የግዛት ማሰሪያ ስሞች ልዩ መሆን አለባቸው።
- የግዛት ትስስር;
  - `key_prefix` ባዶ ያልሆነ እና በ`/` መጀመር አለበት።
  - `max_item_bytes <= max_total_bytes`.
  - `ConfidentialState` ማሰሪያዎች ግልጽ ምስጠራን መጠቀም አይችሉም።
- የማሰማራት ጥቅል;
  - `service.container.manifest_hash` ቀኖናዊ ኢንኮድ መዛመድ አለበት።
    መያዣ አንጸባራቂ hash.
  - `service.container.expected_schema_version` ከመያዣው ንድፍ ጋር መዛመድ አለበት።
  - ተለዋዋጭ ግዛት ማሰሪያዎች `container.capabilities.allow_state_writes=true` ያስፈልጋቸዋል።
  - የህዝብ መንገዶች `container.lifecycle.healthcheck_path` ያስፈልጋቸዋል።
- ወኪል አፓርታማ መግለጫ;
  - `container.expected_schema_version` ከመያዣ ንድፍ ጋር መመሳሰል አለበት v1.
  - የመሳሪያ ችሎታ ስሞች ባዶ ያልሆኑ እና ልዩ መሆን አለባቸው።- የመመሪያ ችሎታ ስሞች ልዩ መሆን አለባቸው።
  - ወጪ-ገደብ ንብረቶች ባዶ ያልሆኑ እና ልዩ መሆን አለባቸው።
  - ለእያንዳንዱ የወጪ ገደብ `max_per_tx_nanos <= max_per_day_nanos`።
  - የተፈቀደ ዝርዝር የአውታረ መረብ ፖሊሲ ​​ልዩ ባዶ ያልሆኑ አስተናጋጆችን ማካተት አለበት።
- የ FHE መለኪያ ስብስብ;
  - `backend` እና `ciphertext_modulus_bits` ባዶ ያልሆኑ መሆን አለባቸው።
  - እያንዳንዱ የምስጥር ጽሑፍ ሞዱል ቢት-መጠን በ`2..=120` ውስጥ መሆን አለበት።
  - የምስጥር ጽሑፍ ሞጁሎች ሰንሰለት ቅደም ተከተል የማይጨምር መሆን አለበት።
  - `plaintext_modulus_bits` ከትልቁ የምስጥር ጽሁፍ ሞጁሎች ያነሰ መሆን አለበት።
  - `slot_count <= polynomial_modulus_degree`.
  - `max_multiplicative_depth < ciphertext_modulus_bits.len()`.
  - የሕይወት ዑደት ቁመት ማዘዝ ጥብቅ መሆን አለበት:
    `activation < deprecation < withdraw` ሲገኝ።
  የሕይወት ዑደት ሁኔታ መስፈርቶች;
    - `Proposed` መቋረጥ/ከፍታ ማውጣትን አይፈቅድም።
    - `Active` `activation_height` ያስፈልገዋል።
    - `Deprecated` `activation_height` + `deprecation_height` ያስፈልገዋል።
    - `Withdrawn` `activation_height` + `withdraw_height` ያስፈልገዋል።
- የFHE አፈጻጸም ፖሊሲ፡-
  - `max_plaintext_bytes <= max_ciphertext_bytes`.
  - `max_output_ciphertexts <= max_input_ciphertexts`.
  - የመለኪያ ስብስብ ማሰሪያ በ`(param_set, version)` መዛመድ አለበት።
  - `max_multiplication_depth` ከመለኪያ-ስብስብ ጥልቀት መብለጥ የለበትም።
  - የፖሊሲ መግቢያ `Proposed` ወይም `Withdrawn` በመለኪያ የተቀመጠውን የህይወት ዑደት ውድቅ ያደርጋል።
- የFHE አስተዳደር ጥቅል፡-
  - ፖሊሲን + የመለኪያ-ስብስብ ተኳኋኝነትን እንደ አንድ የሚወስን የመግቢያ ክፍያ ያረጋግጣል።
የ FHE የሥራ ዝርዝር መግለጫ
  - `job_id` እና `output_state_key` ባዶ ያልሆኑ መሆን አለባቸው (`output_state_key` በ `/` ይጀምራል)።- የግቤት ስብስብ ባዶ ያልሆነ እና የግቤት ቁልፎች ልዩ ቀኖናዊ መንገዶች መሆን አለባቸው።
  - ኦፕሬሽን-ተኮር ገደቦች ጥብቅ ናቸው (`Add`/`Multiply` ባለብዙ ግቤት፣
    `RotateLeft`/`Bootstrap` ነጠላ-ግቤት፣ እርስ በርስ የሚጣረስ ጥልቀት/ማሽከርከር/ቡት ማያያዣዎች)።
  - ከፖሊሲ ጋር የተያያዘ ቅበላ ያስፈጽማል፡-
    - ፖሊሲ/ፓራም መለያዎች እና ስሪቶች ይዛመዳሉ።
    - የግቤት ቆጠራ/ባይት፣ ጥልቀት፣ ማሽከርከር እና የቡት ስታራፕ ገደቦች በፖሊሲ ገደቦች ውስጥ ናቸው።
    - የሚወስን የተገመተው የውጤት ባይት የፖሊሲ ምስጥር ጽሁፍ ገደቦችን ያሟላል።
- ዲክሪፕት የማድረግ ስልጣን ፖሊሲ፡-
  - `approver_ids` ባዶ ያልሆነ፣ ልዩ እና በጥብቅ መዝገበ ቃላት የተደረደረ መሆን አለበት።
  - `ClientHeld` ሁነታ በትክክል አንድ አጽዳቂ ይፈልጋል፣ `approver_quorum=1`፣
    እና `allow_break_glass=false`.
  - `ThresholdService` ሁነታ ቢያንስ ሁለት አጽዳቂዎች እና ያስፈልገዋል
    `approver_quorum <= approver_ids.len()`.
  - `jurisdiction_tag` ባዶ ያልሆነ እና የቁጥጥር ቁምፊዎችን መያዝ የለበትም።
  - `audit_tag` ባዶ ያልሆነ እና የቁጥጥር ቁምፊዎችን መያዝ የለበትም።
- የመፍታት ጥያቄ፡-
  - `request_id`፣ `state_key`፣ እና `justification` ባዶ ያልሆኑ መሆን አለባቸው
    (`state_key` በ `/` ይጀምራል)።
  - `jurisdiction_tag` ባዶ ያልሆነ እና የቁጥጥር ቁምፊዎችን መያዝ የለበትም።
  - `break_glass_reason` ያስፈልጋል `break_glass=true` እና ጊዜ መተው አለበት
    `break_glass=false`.
  - ከፖሊሲ ጋር የተገናኘ መግባቱ የፖሊሲ-ስም እኩልነትን ያስፈጽማል፣ TTLን አይጠይቅም።ከ `policy.max_ttl_blocks` በላይ፣ የስልጣን-መለያ እኩልነት፣ መስበር-መስታወት
    መግቢያ፣ እና የፍቃድ-ማስረጃ መስፈርቶች ሲሆኑ
    `policy.require_consent_evidence=true` ላልተሰበር ብርጭቆ ጥያቄዎች።
- የምስጥር ጽሑፍ መጠይቅ ዝርዝር፡-
  - `state_key_prefix` ባዶ ያልሆነ እና በ `/` መጀመር አለበት።
  - `max_results` የተወሰነ ነው (`<=256`)።
  - የሜታዳታ ትንበያ ግልጽ ነው (`Minimal` መፍጨት-ብቻ vs `Standard` ቁልፍ-የሚታይ)።
- የምስጥር ጽሑፍ ጥያቄ ምላሽ፡-
  - `result_count` እኩል ተከታታይ ረድፍ ቆጠራ አለበት.
  - `Minimal` ትንበያ `state_key` ማጋለጥ የለበትም; `Standard` ማጋለጥ አለበት።
  - ረድፎች ግልጽ የጽሑፍ ምስጠራ ሁነታን በፍፁም ማጋለጥ የለባቸውም።
  - የማካተት ማረጋገጫዎች (በሚገኙበት ጊዜ) ባዶ ያልሆኑ የእቅድ መታወቂያዎችን እና ማካተት አለባቸው
    `anchor_sequence >= event_sequence`.
- ሚስጥራዊ ፖስታ;
  - `key_id`፣ `nonce`፣ እና `ciphertext` ባዶ ያልሆኑ መሆን አለባቸው።
  - አንድ ያልሆነ ርዝመት የታሰረ ነው (`<=256` ባይት)።
  - የምስጥር ጽሑፍ ርዝመት የታሰረ ነው (`<=33554432` ባይት)።
- የጽሑፍ ሁኔታ መዝገብ፡-
  - `state_key` ባዶ ያልሆነ እና በ `/` መጀመር አለበት።
  - የሜታዳታ ይዘት አይነት ባዶ ያልሆነ መሆን አለበት; መለያዎች ልዩ ባዶ ያልሆኑ ሕብረቁምፊዎች መሆን አለባቸው።
  - `metadata.payload_bytes` `secret.ciphertext.len()` እኩል መሆን አለበት።
  - `metadata.commitment` `secret.commitment` እኩል መሆን አለበት።

## ቀኖናዊ ቋሚዎች

ቀኖናዊ የJSON እቃዎች በሚከተለው ላይ ተቀምጠዋል፡- `fixtures/soracloud/sora_container_manifest_v1.json`
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

የጉዞ/የጉዞ ሙከራዎች፡-

- `crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs`
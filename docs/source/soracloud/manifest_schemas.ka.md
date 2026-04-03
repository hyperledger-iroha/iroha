<!-- Auto-generated stub for Georgian (ka) translation. Replace this content with the full translation. -->

---
lang: ka
direction: ltr
source: docs/source/soracloud/manifest_schemas.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a0724ed92da90d8d78a7095b5fc75523ea5dd4a1c885059e28c1bbb8118d1f8c
source_last_modified: "2026-03-26T06:12:11.480497+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# SoraCloud V1 მანიფესტის სქემები

ეს გვერდი განსაზღვრავს პირველ განმსაზღვრელ Norito სქემებს SoraCloud-ისთვის
განლაგება Iroha 3-ზე:

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

Rust-ის განმარტებები ცხოვრობს `crates/iroha_data_model/src/soracloud.rs`-ში.

ატვირთული მოდელის პირადი გაშვების ჩანაწერები განზრახ ცალკე ფენაა
ეს SCR განლაგება ვლინდება. მათ უნდა გააგრძელონ Soracloud მოდელის თვითმფრინავი
და ხელახლა გამოიყენეთ `SecretEnvelopeV1` / `CiphertextStateRecordV1` დაშიფრული ბაიტებისთვის
და შიფრული ტექსტის მშობლიური მდგომარეობა, ვიდრე დაშიფრული იყოს როგორც ახალი სერვისი/კონტეინერი
ვლინდება. იხილეთ `uploaded_private_models.md`.

## სფერო

ეს მანიფესტები შექმნილია `IVM` + მორგებული Sora Container Runtime-ისთვის
(SCR) მიმართულება (არ არის WASM, არ არის Docker დამოკიდებულება გაშვების დროს).- `SoraContainerManifestV1` აღწერს შესრულებადი პაკეტის იდენტურობას, გაშვების ტიპს,
  შესაძლებლობების პოლიტიკა, რესურსები, სასიცოცხლო ციკლის გამოძიების პარამეტრები და აშკარა
  საჭირო კონფიგურაციის ექსპორტი გაშვების გარემოში ან დამონტაჟებულ რევიზიაში
  ხე.
- `SoraServiceManifestV1` აღწერს განლაგების განზრახვას: სერვისის იდენტურობა,
  მითითებულ კონტეინერის მანიფესტის ჰეში/ვერსია, მარშრუტიზაცია, გაშვების პოლიტიკა და
  სახელმწიფო სავალდებულო.
- `SoraStateBindingV1` ასახავს დეტერმინისტულ მდგომარეობას ჩაწერის ფარგლებს და საზღვრებს
  (სახელთა სივრცის პრეფიქსი, ცვალებადობის რეჟიმი, დაშიფვრის რეჟიმი, ელემენტი/მთლიანი კვოტები).
- `SoraDeploymentBundleV1` წყვილების კონტეინერი + სერვისი გამოხატავს და ახორციელებს
  დაშვების განმსაზღვრელი შემოწმებები (მანიფესტ-ჰეში კავშირი, სქემის გასწორება და
  უნარი/სავალდებულო თანმიმდევრულობა).
- `AgentApartmentManifestV1` ასახავს მუდმივი აგენტის გაშვების პოლიტიკას:
  ხელსაწყოების ქუდები, პოლიტიკის ლიმიტები, ხარჯვის ლიმიტები, სახელმწიფო კვოტა, ქსელის გამოსვლა და
  განაახლეთ ქცევა.
- `FheParamSetV1` ასახავს მმართველობით მართულ FHE პარამეტრების კომპლექტს:
  დეტერმინისტული backend/სქემის იდენტიფიკატორები, მოდულის პროფილი, უსაფრთხოება/სიღრმე
  საზღვრები და სასიცოცხლო ციკლის სიმაღლეები (`activation`/`deprecation`/`withdraw`).
- `FheExecutionPolicyV1` ასახავს შიფრული ტექსტის შესრულების განმსაზღვრელ საზღვრებს:
  დაშვებული დატვირთვის ზომები, შემავალი/გამომავალი ვენტილატორი, სიღრმე/როტაცია/ჩატვირთვის ქუდები,
  და კანონიკური დამრგვალების რეჟიმი.
- `FheGovernanceBundleV1` აწყვილებს პარამეტრებს და პოლიტიკას დეტერმინისტიკისთვის
  დაშვების დადასტურება.- `FheJobSpecV1` ასახავს დეტერმინისტულ დაშიფრული ტექსტის სამუშაოს მიღებას/შესრულებას
  მოთხოვნები: ოპერაციული კლასი, შეკვეთილი შეყვანის ვალდებულებები, გამომავალი გასაღები და შეზღუდული
  სიღრმე/როტაცია/ჩატვირთვის მოთხოვნა დაკავშირებულია პოლიტიკასთან + პარამეტრებთან.
- `DecryptionAuthorityPolicyV1` ასახავს მმართველობით მართულ გამჟღავნების პოლიტიკას:
  უფლებამოსილების რეჟიმი (კლიენტის მიერ დაწესებული სერვისი ბარიერის წინააღმდეგ), დამამტკიცებელი კვორუმი/წევრები,
  შუშის გატეხვის შემწეობა, იურისდიქციის მონიშვნა, თანხმობა-მტკიცებულების მოთხოვნა,
  TTL საზღვრები და კანონიკური აუდიტის მონიშვნა.
- `DecryptionRequestV1` ასახავს პოლიტიკასთან დაკავშირებულ გამჟღავნების მცდელობებს:
  შიფრული ტექსტის გასაღების მითითება (`binding_name` + `state_key` + ვალდებულება),
  დასაბუთება, იურისდიქციის ტეგი, სურვილისამებრ თანხმობის-მტკიცებულების ჰეში, TTL,
  შუშის გატეხვის განზრახვა/მიზეზი და მმართველობის ჰეშის კავშირი.
- `CiphertextQuerySpecV1` ასახავს მხოლოდ შიფრული ტექსტის მოთხოვნის განზრახვას:
  სერვისის/სავალდებულო სფერო, გასაღების პრეფიქსის ფილტრი, შეზღუდული შედეგის ლიმიტი, მეტამონაცემები
  პროექციის დონე და მტკიცებულების ჩართვის გადართვა.
- `CiphertextQueryResponseV1` ასახავს ინფორმაციის გამჟღავნების მინიმიზაციას შეკითხვის შედეგებს:
  დაიჯესტზე ორიენტირებული საკვანძო ცნობები, შიფრული ტექსტის მეტამონაცემები, არჩევითი ჩართვის მტკიცებულებები,
  და პასუხის დონის შეკვეცა/მიმდევრობის კონტექსტი.
- `SecretEnvelopeV1` იჭერს დაშიფრულ ტვირთის მასალას:
  დაშიფვრის რეჟიმი, გასაღების იდენტიფიკატორი/ვერსია, nonce, შიფრული ტექსტის ბაიტები და
  მთლიანობის ვალდებულებები.
- `CiphertextStateRecordV1` აღწერს შიფრული ტექსტის მშობლიურ მდგომარეობასსაჯარო მეტამონაცემების გაერთიანება (შინაარსის ტიპი, პოლიტიკის ტეგები, ვალდებულება, დატვირთვის ზომა)
  `SecretEnvelopeV1`-ით.
- მომხმარებლის მიერ ატვირთული პირადი მოდელების პაკეტები უნდა ემყარებოდეს ამ შიფრატექსტს
  ჩანაწერები:
  დაშიფრული წონა/კონფიგურაცია/პროცესორის ნაწილები ცხოვრობს მდგომარეობაში, ხოლო მოდელის რეესტრი,
  რჩება წონის ხაზი, პროფილების შედგენა, დასკვნის სესიები და საგუშაგოები
  პირველი კლასის Soracloud ჩანაწერები.

## ვერსია

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

ვალიდაცია უარყოფს მხარდაუჭერელ ვერსიებს
`SoraCloudManifestError::UnsupportedVersion`.

## განმსაზღვრელი ვალიდაციის წესები (V1)- კონტეინერის მანიფესტი:
  - `bundle_path` და `entrypoint` არ უნდა იყოს ცარიელი.
  - `healthcheck_path` (თუ დაყენებულია) უნდა დაიწყოს `/`-ით.
  - `config_exports` შეიძლება მიუთითებდეს მხოლოდ დეკლარირებული კონფიგურაციებზე
    `required_config_names`.
  - config-export env სამიზნეებმა უნდა გამოიყენონ კანონიკური გარემოს ცვლადის სახელები
    (`[A-Za-z_][A-Za-z0-9_]*`).
  - ფაილის კონფიგურაციის ექსპორტის სამიზნეები უნდა დარჩეს შედარებითი, გამოიყენონ `/` გამყოფები და
    არ უნდა შეიცავდეს ცარიელ, `.` ან `..` სეგმენტებს.
  - კონფიგურაციის ექსპორტი არ უნდა იყოს მიზნად ისახავს იგივე env var ან შედარებით ფაილის გზას
    ვიდრე ერთხელ.
- სერვისის მანიფესტი:
  - `service_version` არ უნდა იყოს ცარიელი.
  - `container.expected_schema_version` უნდა ემთხვეოდეს კონტეინერის სქემას v1.
  - `rollout.canary_percent` უნდა იყოს `0..=100`.
  - `route.path_prefix` (თუ დაყენებულია) უნდა დაიწყოს `/`-ით.
  - სახელმწიფო სავალდებულო სახელები უნდა იყოს უნიკალური.
- სახელმწიფო სავალდებულო:
  - `key_prefix` არ უნდა იყოს ცარიელი და დაიწყოს `/`-ით.
  - `max_item_bytes <= max_total_bytes`.
  - `ConfidentialState` საკინძები ვერ გამოიყენებენ უბრალო ტექსტის დაშიფვრას.
- განლაგების ნაკრები:
  - `service.container.manifest_hash` უნდა ემთხვეოდეს კანონიკურ დაშიფრულს
    კონტეინერის მანიფესტი ჰეში.
  - `service.container.expected_schema_version` უნდა ემთხვეოდეს კონტეინერის სქემას.
  - ცვალებადი მდგომარეობის შეკვრა მოითხოვს `container.capabilities.allow_state_writes=true`.
  - საზოგადოებრივი მარშრუტები მოითხოვს `container.lifecycle.healthcheck_path`.
- აგენტი ბინის მანიფესტი:
  - `container.expected_schema_version` უნდა ემთხვეოდეს კონტეინერის სქემას v1.
  - ხელსაწყოების შესაძლებლობების სახელები არ უნდა იყოს ცარიელი და უნიკალური.- პოლიტიკის შესაძლებლობების სახელები უნდა იყოს უნიკალური.
  - ხარჯვის ლიმიტის აქტივები უნდა იყოს არა ცარიელი და უნიკალური.
  - `max_per_tx_nanos <= max_per_day_nanos` თითოეული ხარჯვის ლიმიტისთვის.
  - დაშვებული სიის ქსელის პოლიტიკა უნდა შეიცავდეს უნიკალურ, არაცარიელ ჰოსტებს.
- FHE პარამეტრების ნაკრები:
  - `backend` და `ciphertext_modulus_bits` არ უნდა იყოს ცარიელი.
  - თითოეული შიფრული ტექსტის მოდულის ბიტის ზომა უნდა იყოს `2..=120` ფარგლებში.
  - შიფრული ტექსტის მოდულის ჯაჭვის რიგი არ უნდა იყოს მზარდი.
  - `plaintext_modulus_bits` უნდა იყოს შიფრული ტექსტის უდიდეს მოდულზე პატარა.
  - `slot_count <= polynomial_modulus_degree`.
  - `max_multiplicative_depth < ciphertext_modulus_bits.len()`.
  - სიცოცხლის ციკლის სიმაღლის შეკვეთა უნდა იყოს მკაცრი:
    `activation < deprecation < withdraw` როდესაც იმყოფება.
  - სასიცოცხლო ციკლის სტატუსის მოთხოვნები:
    - `Proposed` არ იძლევა სიმაღლის გაუქმებას/აცილებას.
    - `Active` მოითხოვს `activation_height`.
    - `Deprecated` მოითხოვს `activation_height` + `deprecation_height`.
    - `Withdrawn` მოითხოვს `activation_height` + `withdraw_height`.
- FHE შესრულების პოლიტიკა:
  - `max_plaintext_bytes <= max_ciphertext_bytes`.
  - `max_output_ciphertexts <= max_input_ciphertexts`.
  - პარამეტრების ნაკრების დაკავშირება უნდა შეესაბამებოდეს `(param_set, version)`-ს.
  - `max_multiplication_depth` არ უნდა აღემატებოდეს პარამეტრებში მითითებული სიღრმეს.
  - პოლიტიკის დაშვება უარყოფს `Proposed` ან `Withdrawn` პარამეტრებში მითითებული სიცოცხლის ციკლს.
- FHE მმართველობის ნაკრები:
  - ამოწმებს პოლიტიკის + პარამეტრების კომპლექტის თავსებადობას, როგორც ერთ განმსაზღვრელ დაშვების დატვირთვას.
- FHE სამუშაო სპეციფიკა:
  - `job_id` და `output_state_key` არ უნდა იყოს ცარიელი (`output_state_key` იწყება `/`-ით).- შეყვანის ნაკრები არ უნდა იყოს ცარიელი და შეყვანის გასაღებები უნდა იყოს უნიკალური კანონიკური ბილიკები.
  - ოპერაციის სპეციფიკური შეზღუდვები მკაცრია (`Add`/`Multiply` მრავალ შეყვანა,
    `RotateLeft`/`Bootstrap` ერთჯერადი შეყვანით, ურთიერთგამომრიცხავი სიღრმის/როტაციის/ჩატვირთვის ღილაკებით).
  - პოლიტიკასთან დაკავშირებული დაშვება ახორციელებს:
    - პოლიტიკის/პარამის იდენტიფიკატორები და ვერსიები ემთხვევა.
    - შეყვანის რაოდენობა/ბაიტი, სიღრმე, როტაცია და ჩატვირთვის ლიმიტები პოლიტიკის ლიმიტების ფარგლებშია.
    - დეტერმინისტული პროგნოზირებული გამომავალი ბაიტები შეესაბამება პოლიტიკის შიფრული ტექსტის ლიმიტებს.
- გაშიფვრის უფლებამოსილების პოლიტიკა:
  - `approver_ids` უნდა იყოს არა ცარიელი, უნიკალური და მკაცრად ლექსიკოგრაფიულად დალაგებული.
  - `ClientHeld` რეჟიმს სჭირდება ზუსტად ერთი დამმტკიცებელი, `approver_quorum=1`,
    და `allow_break_glass=false`.
  - `ThresholdService` რეჟიმს სჭირდება მინიმუმ ორი დამმტკიცებელი და
    `approver_quorum <= approver_ids.len()`.
  - `jurisdiction_tag` არ უნდა იყოს ცარიელი და არ უნდა შეიცავდეს საკონტროლო სიმბოლოებს.
  - `audit_tag` არ უნდა იყოს ცარიელი და არ უნდა შეიცავდეს საკონტროლო სიმბოლოებს.
- გაშიფვრის მოთხოვნა:
  - `request_id`, `state_key` და `justification` არ უნდა იყოს ცარიელი
    (`state_key` იწყება `/`-ით).
  - `jurisdiction_tag` არ უნდა იყოს ცარიელი და არ უნდა შეიცავდეს საკონტროლო სიმბოლოებს.
  - `break_glass_reason` საჭიროა, როდესაც `break_glass=true` და უნდა გამოტოვოთ, როდესაც
    `break_glass=false`.
  - პოლიტიკასთან დაკავშირებული დაშვება ახორციელებს პოლიტიკის სახელების თანასწორობას, მოთხოვნა TTL არააღემატება `policy.max_ttl_blocks`-ს, იურისდიქციის ნიშნის თანასწორობას, შუშის გატეხვას
    კარიბჭე და თანხმობა-მტკიცებულებების მოთხოვნები, როდესაც
    `policy.require_consent_evidence=true` მინის არამტვრევადი მოთხოვნებისთვის.
- ციფერტექსტის მოთხოვნის სპეციფიკა:
  - `state_key_prefix` არ უნდა იყოს ცარიელი და დაიწყოს `/`-ით.
  - `max_results` დეტერმინისტულად შემოიფარგლება (`<=256`).
  - მეტამონაცემების პროექცია არის აშკარა (`Minimal` დაიჯესტი მხოლოდ `Standard` გასაღები-ხილული).
- ციფერტექსტის შეკითხვის პასუხი:
  - `result_count` უნდა უტოლდებოდეს სერიული მწკრივების რაოდენობას.
  - `Minimal` პროექცია არ უნდა გამოაშკარავდეს `state_key`; `Standard` უნდა გამოამჟღავნოს იგი.
  - სტრიქონები არასოდეს არ უნდა ჩანდეს ღია ტექსტის დაშიფვრის რეჟიმში.
  - ჩართვის მტკიცებულებები (როდესაც ეს არის) უნდა შეიცავდეს სქემის არაცარიელ იდენტიფიკატორებს და
    `anchor_sequence >= event_sequence`.
- საიდუმლო კონვერტი:
  - `key_id`, `nonce` და `ciphertext` არ უნდა იყოს ცარიელი.
  - არაერთი სიგრძე შემოსაზღვრულია (`<=256` ბაიტი).
  - შიფრული ტექსტის სიგრძე შემოსაზღვრულია (`<=33554432` ბაიტი).
- შიფრატექსტის სახელმწიფო ჩანაწერი:
  - `state_key` არ უნდა იყოს ცარიელი და დაიწყოს `/`-ით.
  - მეტამონაცემების შინაარსის ტიპი არ უნდა იყოს ცარიელი; ტეგები უნდა იყოს უნიკალური, არა ცარიელი სტრიქონები.
  - `metadata.payload_bytes` უნდა იყოს `secret.ciphertext.len()`.
  - `metadata.commitment` უნდა იყოს `secret.commitment`.

## კანონიკური მოწყობილობები

Canonical JSON მოწყობილობები ინახება:- `fixtures/soracloud/sora_container_manifest_v1.json`
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

სამონტაჟო/ორმხრივი ტესტები:

- `crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs`
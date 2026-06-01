<!-- Auto-generated stub for Kazakh (kk) translation. Replace this content with the full translation. -->

---
lang: kk
direction: ltr
source: docs/source/soracloud/manifest_schemas.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a0724ed92da90d8d78a7095b5fc75523ea5dd4a1c885059e28c1bbb8118d1f8c
source_last_modified: "2026-03-26T06:12:11.480497+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# SoraCloud V1 манифест схемалары

Бұл бет SoraCloud үшін бірінші детерминирленген Norito схемаларын анықтайды
Iroha 3 бойынша орналастыру:

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

Rust анықтамалары `crates/iroha_data_model/src/soracloud.rs` ішінде тұрады.

Жүктелген үлгідегі жеке орындалу уақыты жазбалары әдейі бөлек қабат болып табылады
бұл SCR орналастыру көріністері. Олар Soracloud моделінің ұшағын кеңейтуі керек
және шифрланған байттар үшін `SecretEnvelopeV1` / `CiphertextStateRecordV1` қайта пайдаланыңыз
және жаңа қызмет/контейнер ретінде кодтаудың орнына шифрленген мәтіннің төл күйі
көрсетеді. `uploaded_private_models.md` қараңыз.

## Ауқым

Бұл манифесттер `IVM` + реттелетін Sora Container Runtime үшін жасалған
(SCR) бағыты (WASM жоқ, орындау уақытын қабылдауда Docker тәуелділігі жоқ).- `SoraContainerManifestV1` орындалатын бума идентификациясын, орындалу уақыты түрін,
  мүмкіндік саясаты, ресурстар, өмірлік циклді тексеру параметрлері және айқын
  талап етілетін конфигурацияны орындау ортасына немесе орнатылған ревизияға экспорттау
  ағаш.
- `SoraServiceManifestV1` орналастыру мақсатын жазады: қызмет сәйкестендіру,
  сілтеме жасалған контейнер манифест хэш/нұсқасы, маршруттау, шығару саясаты және
  мемлекеттік байланыстар.
- `SoraStateBindingV1` детерминирленген күй-жазу ауқымы мен шектеулерін жазады
  (аттар кеңістігінің префиксі, өзгергіштік режимі, шифрлау режимі, элемент/жалпы квоталар).
- `SoraDeploymentBundleV1` жұптары контейнер + қызмет манифесттері және күшіне енеді
  детерминирленген қабылдау тексерулері (манифест-хэш байланысы, схеманы теңестіру және
  қабілеттілік/байланыстырушылық консистенциясы).
- `AgentApartmentManifestV1` тұрақты агенттің орындалу уақыты саясатын жазады:
  құралдар шектері, саясат шектері, шығыс шектеулері, күй квотасы, желіден шығу және
  мінез-құлықты жаңарту.
- `FheParamSetV1` басқарумен басқарылатын FHE параметр жиындарын түсіреді:
  Детерминирленген сервер/схема идентификаторлары, модуль профилі, қауіпсіздік/тереңдік
  шекаралар және өмірлік цикл биіктіктері (`activation`/`deprecation`/`withdraw`).
- `FheExecutionPolicyV1` детерминирленген шифрлық мәтінді орындау шектеулерін жазады:
  рұқсат етілген пайдалы жүктеме өлшемдері, кіріс/шығыс желдеткіші, тереңдік/айналу/жүктеу қақпақтары,
  және канондық дөңгелектеу режимі.
- `FheGovernanceBundleV1` параметр жиынын және детерминистикалық саясатты қосады
  қабылдауды растау.- `FheJobSpecV1` тапсырманы қабылдауды/орындауды детерминирленген шифрлық мәтінді түсіреді
  сұраныстар: операция класы, реттелген енгізу міндеттемелері, шығыс кілті және шектелген
  саясат + параметр жиынына байланысты тереңдік/айналу/жүктеу сұранысы.
- `DecryptionAuthorityPolicyV1` басқарумен басқарылатын ақпаратты ашу саясатын қамтиды:
  өкілеттік режимі (клиент ұстайтын және шекті қызмет), мақұлдаушы кворум/мүшелер,
  шыны сынықтары, юрисдикция белгілері, келісім-дәлелдер талабы,
  TTL шекаралары және канондық аудит тегтері.
- `DecryptionRequestV1` саясатқа байланысты ашу әрекеттерін түсіреді:
  шифрлық мәтін кілтіне сілтеме (`binding_name` + `state_key` + міндеттеме),
  негіздеме, юрисдикция тегі, қосымша келісім-дәлел хэші, TTL,
  шыны сындыратын ниет/себеп және басқару хэш байланысы.
- `CiphertextQuerySpecV1` тек детерминирленген шифрлық мәтінді сұрау мақсатын жазады:
  қызмет/байланыстыру ауқымы, кілт-префикс сүзгісі, шектелген нәтиже шегі, метадеректер
  проекция деңгейі және дәлелді қосу қосқышы.
- `CiphertextQueryResponseV1` ашуды азайтатын сұрау нәтижелерін түсіреді:
  дайджестке бағытталған негізгі сілтемелер, шифрлық мәтіндік метадеректер, қосымша қосу дәлелдері,
  және жауап деңгейіндегі қысқарту/дәйектілік контексті.
- `SecretEnvelopeV1` шифрланған пайдалы жүктеме материалының өзін түсіреді:
  шифрлау режимі, кілт идентификаторы/нұсқасы, nonnce, шифрленген мәтін байты және
  адалдық міндеттемелері.
- `CiphertextStateRecordV1` шифрланған мәтіндік күй жазбаларын жазадыжалпыға ортақ метадеректерді біріктіру (мазмұн түрі, саясат тегтері, міндеттеме, пайдалы жүктеме өлшемі)
  `SecretEnvelopeV1` көмегімен.
- Пайдаланушы жүктеп салған жеке үлгі жинақтары осы шифрленген мәтін негізінде құрастырылуы керек
  жазбалар:
  шифрланған салмақ/конфигурация/процессор бөліктері күйде тұрады, ал үлгі тізілімі,
  салмақ шежіресі, құрастыру профильдері, қорытынды сессиялары және бақылау нүктелері қалады
  бірінші дәрежелі Soracloud жазбалары.

## Нұсқа жасау

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

Тексеру қолдау көрсетілмейтін нұсқаларды қабылдамайды
`SoraCloudManifestError::UnsupportedVersion`.

## Детерминистік тексеру ережелері (V1)- Контейнерлік манифест:
  - `bundle_path` және `entrypoint` бос болмауы керек.
  - `healthcheck_path` (орнатылған болса) `/` деп басталуы керек.
  - `config_exports` құжатында жарияланған конфигурацияларға ғана сілтеме жасай алады
    `required_config_names`.
  - config-export env мақсаттары канондық орта айнымалы атауларын пайдалануы керек
    (`[A-Za-z_][A-Za-z0-9_]*`).
  - конфигурациялау-экспорттау файлының мақсаттары салыстырмалы болып қалуы, `/` бөлгіштерін пайдалануы және
    бос, `.` немесе `..` сегменттерін қамтымауы керек.
  - конфигурация экспорты бірдей env var немесе салыстырмалы файл жолын көбірек бағыттамауы керек
    бір емес.
- Қызмет манифесті:
  - `service_version` бос болмауы керек.
  - `container.expected_schema_version` v1 контейнер схемасына сәйкес келуі керек.
  - `rollout.canary_percent` `0..=100` болуы керек.
  - `route.path_prefix` (орнатылған болса) `/` арқылы басталуы керек.
  - күйді байланыстыратын атаулар бірегей болуы керек.
- Мемлекеттік міндетті:
  - `key_prefix` бос болмауы керек және `/` деп басталуы керек.
  - `max_item_bytes <= max_total_bytes`.
  - `ConfidentialState` байланыстары ашық мәтінді шифрлауды пайдалана алмайды.
- Орналастыру жинағы:
  - `service.container.manifest_hash` канондық кодталғанға сәйкес келуі керек
    контейнер манифест хэші.
  - `service.container.expected_schema_version` контейнер схемасына сәйкес келуі керек.
  - Өзгермелі күй байланыстары үшін `container.capabilities.allow_state_writes=true` қажет.
  - Қоғамдық бағыттар үшін `container.lifecycle.healthcheck_path` қажет.
- Агенттік пәтер манифесі:
  - `container.expected_schema_version` v1 контейнер схемасына сәйкес келуі керек.
  - құрал мүмкіндіктерінің атаулары бос емес және бірегей болуы керек.- саясат мүмкіндіктерінің атаулары бірегей болуы керек.
  - шығыс лимиті активтер бос емес және бірегей болуы керек.
  - Әрбір шығыс шегі үшін `max_per_tx_nanos <= max_per_day_nanos`.
  - рұқсат етілген тізім желі саясаты бірегей бос емес хосттарды қамтуы керек.
- FHE параметрлер жинағы:
  - `backend` және `ciphertext_modulus_bits` бос болмауы керек.
  - әрбір шифрлық мәтін модулінің бит өлшемі `2..=120` шегінде болуы керек.
  - шифрленген мәтін модулінің тізбек реті өспейтін болуы керек.
  - `plaintext_modulus_bits` ең үлкен шифрлық мәтін модулінен кіші болуы керек.
  - `slot_count <= polynomial_modulus_degree`.
  - `max_multiplicative_depth < ciphertext_modulus_bits.len()`.
  - өмірлік цикл биіктігіне тапсырыс қатаң болуы керек:
    `activation < deprecation < withdraw` болған кезде.
  - өмірлік цикл күйіне қойылатын талаптар:
    - `Proposed` ескіруге/алып тастау биіктіктеріне жол бермейді.
    - `Active` үшін `activation_height` қажет.
    - `Deprecated` үшін `activation_height` + `deprecation_height` қажет.
    - `Withdrawn` үшін `activation_height` + `withdraw_height` қажет.
- FHE орындау саясаты:
  - `max_plaintext_bytes <= max_ciphertext_bytes`.
  - `max_output_ciphertexts <= max_input_ciphertexts`.
  - параметр жиынын байланыстыру `(param_set, version)` сәйкес келуі керек.
  - `max_multiplication_depth` параметр орнатылған тереңдіктен аспауы керек.
  - саясатты қабылдау `Proposed` немесе `Withdrawn` параметр жиынының өмірлік циклін қабылдамайды.
- FHE басқару пакеті:
  - бір детерминирленген рұқсат жүктемесі ретінде саясатты + параметр жиынтығының үйлесімділігін растайды.
- FHE жұмыс ерекшелігі:
  - `job_id` және `output_state_key` бос болмауы керек (`output_state_key` `/` арқылы басталады).- енгізу жиыны бос болмауы керек және енгізу пернелері бірегей канондық жолдар болуы керек.
  - операцияға қатысты шектеулер қатаң (`Add`/`Multiply` көп кіріс,
    `RotateLeft`/`Bootstrap` бір кірісті, өзара ерекше тереңдік/айналу/жүктеу тұтқалары).
  - саясатқа байланысты қабылдауды қамтамасыз етеді:
    - саясат/парам идентификаторлары мен нұсқалары сәйкес келеді.
    - енгізу саны/байттары, тереңдігі, айналуы және жүктеу шектеулері саясаттың шектері шегінде.
    - детерминирленген жобаланған шығыс байттары саясаттың шифрлық мәтін шектеулеріне сәйкес келеді.
- Шифрды шешу жөніндегі уәкілетті саясат:
  - `approver_ids` бос емес, бірегей және қатаң лексикографиялық сұрыпталған болуы керек.
  - `ClientHeld` режимі дәл бір бекітушіні қажет етеді, `approver_quorum=1`,
    және `allow_break_glass=false`.
  - `ThresholdService` режимі кемінде екі мақұлдаушыны және қажет етеді
    `approver_quorum <= approver_ids.len()`.
  - `jurisdiction_tag` бос болмауы керек және басқару таңбаларын қамтымауы керек.
  - `audit_tag` бос болмауы керек және басқару таңбаларын қамтымауы керек.
- Шифрды шешуге сұрау:
  - `request_id`, `state_key` және `justification` бос болмауы керек
    (`state_key` `/` басталады).
  - `jurisdiction_tag` бос болмауы керек және басқару таңбаларын қамтымауы керек.
  - `break_glass_reason` `break_glass=true` кезінде талап етіледі және келесі кезде өткізілмеуі керек
    `break_glass=false`.
  - саясатқа байланысты рұқсат саясат атауының теңдігін қамтамасыз етеді, TTL сұрамайды`policy.max_ttl_blocks` асатын, юрисдикция-тег теңдігі, әйнек сынуы
    қақпақ, және келісім-дәлелдемелер қашан
    Шыны сынбайтын сұраулар үшін `policy.require_consent_evidence=true`.
- Шифрленген мәтінді сұраудың ерекшелігі:
  - `state_key_prefix` бос болмауы керек және `/` деп басталуы керек.
  - `max_results` детерминирленген шектелген (`<=256`).
  - метадеректер проекциясы анық (тек `Minimal` дайджест және `Standard` кілті көрінеді).
- Шифрленген мәтінді сұрауға жауап:
  - `result_count` серияланған жолдар санына тең болуы керек.
  - `Minimal` проекциясы `state_key` көрсетпеуі керек; `Standard` оны ашуы керек.
  - жолдар ешқашан ашық мәтінді шифрлау режимін көрсетпеуі керек.
  - қосу дәлелдері (бар болса) бос емес схема идентификаторларын және қамтуы керек
    `anchor_sequence >= event_sequence`.
- Құпия конверт:
  - `key_id`, `nonce` және `ciphertext` бос болмауы керек.
  - бір реттік ұзындық шектелмейді (`<=256` байт).
  - шифрленген мәтін ұзындығы шектелген (`<=33554432` байт).
- Шифрленген мәтін күйінің жазбасы:
  - `state_key` бос болмауы керек және `/` деп басталуы керек.
  - метадеректер мазмұнының түрі бос болмауы керек; тегтер бірегей бос емес жолдар болуы керек.
  - `metadata.payload_bytes` `secret.ciphertext.len()` тең болуы керек.
  - `metadata.commitment` `secret.commitment` тең болуы керек.

## Канондық арматура

Канондық JSON құрылғылары мына жерде сақталады:- `fixtures/soracloud/sora_container_manifest_v1.json`
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

Бекіту/қайтару сынақтары:

- `crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs`
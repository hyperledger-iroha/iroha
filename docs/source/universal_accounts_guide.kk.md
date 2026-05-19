<!-- Auto-generated stub for Kazakh (kk) translation. Replace this content with the full translation. -->

---
lang: kk
direction: ltr
source: docs/source/universal_accounts_guide.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 09a308ecbf07f0293add7f35cf4f1a50b5e6d3630b8b37a8f0f45a7cf82d3924
source_last_modified: "2026-03-30T18:22:55.987822+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Әмбебап тіркелгі нұсқаулығы

Бұл нұсқаулық UAID (әмбебап тіркелгі идентификаторы) шығару талаптарын төмендетеді
Nexus жол картасы және оларды оператор + SDK бағдарланған шолу арқылы бумалайды.
Ол UAID туындысын, портфолио/манифест инспекциясын, реттеуіш үлгілерін,
және әрбір `iroha қолданбасының кеңістік каталогы манифестімен бірге жүруі керек дәлелдер
publish` run (roadmap reference: `roadmap.md:2209`).

## 1. UAID жылдам анықтамасы- UAID - `uaid:<hex>` литералы, мұнда `<hex>` - Blake2b-256 дайджест, оның
  LSB параметрі `1` күйіне орнатылған. Канондық түрі мекендейді
  `crates/iroha_data_model/src/nexus/manifest.rs::UniversalAccountId`.
- Тіркелгі жазбаларында (`Account` және `AccountDetails`) енді қосымша `uaid` бар.
  өріс, сондықтан қолданбалар идентификаторды тапсырыс бойынша хэштеусіз біле алады.
- Жасырын функция идентификаторының саясаттары ерікті нормаланған кірістерді байланыстыра алады
  (телефон нөмірлері, электрондық пошталар, тіркелгі нөмірлері, серіктес жолдары) `opaque:` идентификаторларына
  UAID аттар кеңістігі астында. Тізбектегі бөліктер `IdentifierPolicy`,
  `IdentifierClaimRecord` және `opaque_id -> uaid` индексі.
- Ғарыштық каталог әрбір UAID-ті байланыстыратын `World::uaid_dataspaces` картасын қолдайды.
  белсенді манифесттермен сілтеме жасалған деректер кеңістігі тіркелгілеріне. Torii оны қайта пайдаланады
  `/portfolio` және `/uaids/*` API интерфейстері үшін карта.
- `POST /v1/accounts/onboard` үшін әдепкі ғарыштық каталог манифестін жариялайды
  ешқайсысы болмаған кезде жаһандық деректер кеңістігі, сондықтан UAID дереу байланыстырылады.
  Борттық органдар `CanPublishSpaceDirectoryManifest{dataspace=0}` ұстауы керек.
- Барлық SDK-лар UAID литералдарын канонизациялау үшін көмекшілерді көрсетеді (мысалы,
  Android SDK ішіндегі `UaidLiteral`). Көмекшілер шикі 64-гекс дайджесттерін қабылдайды
  (LSB=1) немесе `uaid:<hex>` литералы және бірдей Norito кодектерін қайта пайдаланыңыз.
  дайджест тілдер арасында ауыса алмайды.

## 1.1 Жасырын идентификатор саясаттары

UAID енді екінші сәйкестендіру қабатының якоры болып табылады:- Ғаламдық `IdentifierPolicyId` (`<kind>#<business_rule>`) мынаны анықтайды
  аттар кеңістігі, жалпы міндеттеме метадеректері, шешуші растау кілті және
  канондық кірісті қалыпқа келтіру режимі (`Exact`, `LowercaseTrimmed`,
  `PhoneE164`, `EmailAddress` немесе `AccountNumber`).
- Шағым бір туынды `opaque:` идентификаторын дәл бір UAID және біреуімен байланыстырады
  осы саясатқа сәйкес канондық `AccountId`, бірақ тізбек тек
  оған `IdentifierResolutionReceipt` қол қойылған кезде талап қою.
- Ажыратымдылық `resolve -> transfer` ағыны болып қалады. Torii бұлыңғырлықты шешеді
  өңдейді және канондық `AccountId` қайтарады; трансферттер әлі де болса
  канондық тіркелгі, тікелей `uaid:` немесе `opaque:` литералдары емес.
- Саясаттар енді BFV енгізу-шифрлау параметрлерін арқылы жариялай алады
  `PolicyCommitment.public_parameters`. Бұл кезде Torii оларды жарнамалайды
  `GET /v1/identifier-policies` және клиенттер BFV-орапталған енгізуді ұсына алады
  ашық мәтіннің орнына. Бағдарламаланған саясаттар BFV параметрлерін а
  канондық `BfvProgrammedPublicParameters` жинағы, ол сондай-ақ жариялайды
  қоғамдық `ram_fhe_profile`; бұрынғы өңделмеген BFV пайдалы жүктемелері осыған жаңартылады
  міндеттеме қайта құрылған кездегі канондық бума.
- Идентификатор маршруттары бірдей Torii қол жеткізу таңбалауышы мен тарифтік шектеу арқылы өтеді
  басқа қолданбаға қатысты соңғы нүктелер ретінде тексереді. Олар әдеттегіден айналып өтетін жол емес
  API саясаты.

## 1.2 Терминология

Атауды бөлу әдейі:- `ram_lfe` сыртқы жасырын функция абстракциясы. Ол саясатты қамтиды
  тіркеу, міндеттемелер, жалпыға ортақ метадеректер, орындау түбіртектері және
  тексеру режимі.
- `BFV` – пайдаланатын Brakerski/Fan-Vercauteren гомоморфты шифрлау схемасы
  шифрланған енгізуді бағалау үшін кейбір `ram_lfe` серверлері.
- `ram_fhe_profile` - BFV-арнайы метадеректер, жалпыға екінші атау емес
  ерекшелігі. Ол әмияндар және бағдарламаланған BFV орындау машинасын сипаттайды
  саясат бағдарламаланған серверді пайдаланған кезде тексерушілер мақсатты болуы керек.

Нақты сөзбен айтқанда:

- `RamLfeProgramPolicy` және `RamLfeExecutionReceipt` - LFE қабатының түрлері.
- `BfvParameters`, `BfvCiphertext`, `BfvProgrammedPublicParameters`, және
  `BfvRamProgramProfile` - FHE қабатының түрлері.
- `HiddenRamFheProgram` және `HiddenRamFheInstruction` - ішкі атаулар
  бағдарламаланған сервер арқылы орындалатын жасырын BFV бағдарламасы. Олар үстінде қалады
  FHE жағы, өйткені олар шифрланған орындау механизмін сипаттайды
  сыртқы саясат немесе түбіртек абстракциясы.

## 1.3 Бүркеншік аттармен салыстырғанда тіркелгі сәйкестігі

Әмбебап есептік жазбаны шығару канондық тіркелгі сәйкестендіру үлгісін өзгертпейді:- `AccountId` канондық, доменсіз тіркелгі тақырыбы болып қала береді.
- `AccountAlias` мәндері осы тақырыптың үстіндегі бөлек SNS байланыстары болып табылады. А
  `merchant@banka.paynet` сияқты доменге жарамды бүркеншік ат және деректер кеңістігінің түбір бүркеншік аты
  `merchant@paynet` сияқты екеуі де бірдей канондық `AccountId` шеше алады.
- Канондық тіркелгіні тіркеу әрқашан `Account::new(AccountId)` /
  `NewAccount::new(AccountId)`; доменге сәйкес келетін немесе материалдандырылған домен жоқ
  тіркеу жолы.
- Домен иеленуі, бүркеншік ат рұқсаттары және басқа домен ауқымындағы әрекеттер әрекет етеді
  тіркелгі идентификациясының өзінде емес, өз күйінде және API интерфейсінде.
- Жалпы тіркелгіні іздеу осы бөлуден кейін жүзеге асырылады: бүркеншік ат сұраулары жалпыға ортақ болып қалады, ал
  канондық тіркелгі идентификациясы таза `AccountId` болып қалады.

Операторлар, SDK және сынақтар үшін енгізу ережесі: канондық нұсқадан бастаңыз
`AccountId`, содан кейін бүркеншік атын жалға алуды, деректер кеңістігін/домен рұқсаттарын және кез келген нәрсені қосыңыз
доменге тиесілі мемлекет бөлек. Жалған бүркеншік атпен алынған есептік жазбаны синтездемеңіз
немесе бүркеншік ат немесе тіркелгі жазбаларында кез келген байланыстырылған домен өрісін күтіңіз
маршрут домен сегментін тасымалдайды.

Current Torii routes:

| Route | Purpose |
|-------|---------|
| `GET /v1/ram-lfe/program-policies` | Lists active and inactive RAM-LFE program policies plus their public execution metadata, including optional BFV `input_encryption` parameters and the programmed-backend `ram_fhe_profile`. |
| `POST /v1/ram-lfe/programs/{program_id}/execute` | Accepts `{ encrypted_input }` only and returns the stateless `RamLfeExecutionReceipt`, `{ output_ciphertext, output_hash, receipt_hash }`, and no plaintext output. The current Torii runtime issues receipts for the programmed BFV backend. |
| `POST /v1/ram-lfe/receipts/verify` | Statelessly validates a `RamLfeExecutionReceipt` against the published on-chain program policy and optionally checks that a caller-supplied encrypted `output_hex` matches the receipt `output_hash`. |
| `GET /v1/identifier-policies` | Lists active and inactive hidden-function policy namespaces plus their public metadata, including optional BFV `input_encryption` parameters, the required `normalization` mode for encrypted client-side input, and `ram_fhe_profile` for programmed BFV policies. |
| `POST /v1/accounts/{account_id}/identifiers/claim-receipt` | Accepts `{ policy_id, encrypted_input, output_opening }`. The BFV `encrypted_input` must already be normalized according to the published policy mode. The endpoint derives the `opaque:` handle from the verified external `RamLfeOutputOpening` and returns a signed receipt that `ClaimIdentifier` can submit on-chain. |
| `POST /v1/identifiers/resolve` | Accepts `{ policy_id, encrypted_input, output_opening }`. The endpoint re-evaluates the encrypted input, verifies the external output opening, derives the `opaque:` handle from the opened output hash, and returns a nested `{ payload, attestation }` receipt when an active claim exists. |
| `GET /v1/identifiers/receipts/{receipt_hash}` | Looks up the persisted `IdentifierClaimRecord` bound to a deterministic receipt hash so operators and SDKs can audit claim ownership or diagnose replay / mismatch failures without scanning the full identifier index. |

Torii's in-process execution runtime is configured under
`torii.ram_lfe.programs[*]`, keyed by `program_id`. The identifier routes now
reuse that same RAM-LFE runtime instead of a separate `identifier_resolver`
config surface.

Current SDK support:

- `normalizeIdentifierInput(value, normalization)` matches the Rust
  canonicalizers for `exact`, `lowercase_trimmed`, `phone_e164`,
  `email_address`, and `account_number`.
- `ToriiClient.listIdentifierPolicies()` lists policy metadata, including BFV
  input-encryption metadata when the policy publishes it, plus a decoded
  BFV parameter object via `input_encryption_public_parameters_decoded`.
  Programmed policies also expose the decoded `ram_fhe_profile`. That field is
  intentionally BFV-scoped: it lets wallets verify the expected register
  count, lane count, canonicalization mode, and minimum ciphertext modulus for
  the programmed FHE backend before encrypting client-side input.
- `getIdentifierBfvPublicParameters(policy)` and
  `buildIdentifierRequestForPolicy(policy, { encryptedInput | input,
  encrypt: true, outputOpening })` help JS callers consume published BFV
  metadata and build policy-aware encrypted request bodies without
  reimplementing policy-id and normalization rules.
- `encryptIdentifierInputForPolicy(policy, input, { seedHex? })` and
  `buildIdentifierRequestForPolicy(policy, { input, encrypt: true,
  outputOpening })` now let JS wallets construct the full BFV Norito
  ciphertext envelope locally from published policy parameters instead of
  shipping prebuilt ciphertext hex.
- `ToriiClient.resolveIdentifier({ policyId, encryptedInput, outputOpening })`
  resolves a hidden identifier and returns the signed nested
  `{ payload, attestation }` receipt.
- `ToriiClient.issueIdentifierClaimReceipt(accountId, { policyId,
  encryptedInput, outputOpening })` issues the signed receipt needed by
  `ClaimIdentifier`.
- `verifyIdentifierResolutionReceipt(receipt, policy)` verifies the returned
  receipt against the policy resolver key on the client side, and
  `ToriiClient.getIdentifierClaimByReceiptHash(receiptHash)` fetches the
  persisted claim record for later audit/debug flows.
- `IrohaSwift.ToriiClient` now exposes `listIdentifierPolicies()`,
  `resolveIdentifier(policyId:encryptedInputHex:outputOpening:)`,
  `issueIdentifierClaimReceipt(accountId:policyId:encryptedInputHex:outputOpening:)`,
  and `getIdentifierClaimByReceiptHash(_)`, plus
  `ToriiIdentifierNormalization` for the same phone/email/account-number
  canonicalization modes.
- `ToriiIdentifierLookupRequest` and encrypted request helpers provide the
  typed Swift request surface for resolve and claim-receipt calls, and Swift
  policies can now derive the BFV ciphertext locally via `encryptInput(...)`.
- `ToriiIdentifierResolutionReceipt.verifySignature(using:)` validates that
  the top-level receipt fields match the signed payload and verifies the
  resolver signature client-side before submission.
- `HttpClientTransport` in the Android SDK now exposes
  `listIdentifierPolicies()`, encrypted-only `resolveIdentifier(...)`,
  encrypted-only `issueIdentifierClaimReceipt(...)`, and
  `getIdentifierClaimByReceiptHash(...)`,
  plus `IdentifierNormalization` for the same canonicalization rules.
- `IdentifierResolveRequest` and encrypted request helpers provide the typed
  Android request surface, while `IdentifierPolicySummary.encryptInput(...)`
  derives the BFV ciphertext envelope locally from published policy
  parameters.
  `IdentifierResolutionReceipt.verifySignature(policy)` verifies the returned
  resolver signature client-side.

Current instruction set:

- `RegisterIdentifierPolicy`
- `ActivateIdentifierPolicy`
- `ClaimIdentifier` (receipt-bound; raw `opaque_id` claims are rejected)
- `RevokeIdentifier`

Three backends now exist in `iroha_crypto::ram_lfe`:

- the historical commitment-bound `HKDF-SHA3-512` PRF, and
- a BFV-backed secret affine evaluator that consumes BFV-encrypted identifier
  slots directly. When `iroha_crypto` is built with the default
  `bfv-accel` feature, BFV ring multiplication uses an exact deterministic
  CRT-NTT backend internally; disabling that feature falls back to the
  scalar schoolbook path with identical outputs, and
- a BFV-backed secret programmed evaluator that derives an instruction-driven
  RAM-style execution trace over encrypted registers and ciphertext memory
  lanes before deriving the opaque identifier and receipt hash. The programmed
  backend now requires a stronger BFV modulus floor than the affine path, and
  its public parameters are published in a canonical bundle that includes the
  RAM-FHE execution profile consumed by wallets and verifiers.

Here BFV means the Brakerski/Fan-Vercauteren FHE scheme implemented in
`crates/iroha_crypto/src/fhe_bfv.rs`. It is the encrypted-execution mechanism
used by the affine and programmed backends, not the name of the outer hidden
function abstraction.

Torii uses the backend published by the policy commitment. For the first
release, RAM-LFE and hidden-identifier routes are encrypted-only: Torii does
not accept plaintext inputs, does not hold BFV secret keys, and does not
decrypt input or output ciphertexts. Identifier claim and resolve requests must
include an externally signed `RamLfeOutputOpening`; the `opaque:` identifier is
derived from the verified opened-output hash, not from Torii-side plaintext or
from the ciphertext hash alone.

## 2. UAID алу және тексеру

UAID алудың үш қолдауы бар:

1. **Оны әлемдік күй немесе SDK үлгілерінен оқыңыз.** Кез келген `Account`/`AccountDetails`
   Torii арқылы сұралған пайдалы жүктеме енді `uaid` өрісі
   қатысушы әмбебап тіркелгілерді таңдады.
2. **UAID тізілімдерін сұрау.** Torii көрсетеді
   Деректер кеңістігінің байланыстарын қайтаратын `GET /v1/space-directory/uaids/{uaid}`
   Space Directory хостындағы манифест метадеректері сақталады (қараңыз
   Пайдалы жүктеме үлгілері үшін `docs/space-directory.md` §3).
3. **Оны анықтау арқылы шығарыңыз.** Жаңа UAID офлайн жүктегенде, хэш
   канондық қатысушы тұқымы Blake2b-256 және нәтижені префикспен белгілеңіз
   `uaid:`. Төмендегі үзінді құжатталған көмекшіні көрсетеді
   `docs/space-directory.md` §3.3:

   ```python
   import hashlib
   seed = b"participant@example"  # canonical address/domain seed
   digest = bytearray(hashlib.blake2b(seed, digest_size=32).digest())
   digest[-1] |= 1
   print(f"uaid:{digest.hex()}")
   ```Әрқашан литералды кіші әріппен сақтаңыз және хэштеу алдында бос орынды қалыпқа келтіріңіз.
`iroha app space-directory manifest scaffold` және Android сияқты CLI көмекшілері
`UaidLiteral` талдаушысы басқару шолулары үшін бірдей кесу ережелерін қолданады
арнайы сценарийлерсіз мәндерді кросс-тексеру.

## 3. UAID холдингтері мен манифесттерін тексеру

`iroha_core::nexus::portfolio` ішіндегі детерминирленген портфолио агрегаторы
UAID-ге сілтеме жасайтын әрбір актив/деректер кеңістігі жұбын көрсетеді. Операторлар және SDK
деректерді келесі беттер арқылы тұтынуы мүмкін:

| Беткі | Қолданылуы |
|---------|-------|
| `GET /v1/accounts/{uaid}/portfolio` | Деректер кеңістігі → актив → баланс қорытындыларын қайтарады; `docs/source/torii/portfolio_api.md` сипатталған. |
| `GET /v1/space-directory/uaids/{uaid}` | Деректер кеңістігі идентификаторлары + UAID-ге байланыстырылған тіркелгі литералдар тізімі. |
| `GET /v1/space-directory/uaids/{uaid}/manifests` | Аудиттерге арналған толық `AssetPermissionManifest` тарихын қамтамасыз етеді. |
| `iroha app space-directory bindings fetch --uaid <literal>` | Байланыстырудың соңғы нүктесін орап, қосымша JSON дискісіне жазатын CLI таңбашасы (`--json-out`). |
| `iroha app space-directory manifest fetch --uaid <literal> --json-out <path>` | Дәлелдер бумалары үшін манифест JSON бумасын шығарады. |

CLI сеансының мысалы (`iroha.json` ішіндегі `torii_api_url` арқылы конфигурацияланған Torii URL):

```bash
iroha app space-directory bindings fetch \
  --uaid uaid:86e8ee39a3908460a0f4ee257bb25f340cd5b5de72735e9adefe07d5ef4bb0df \
  --json-out artifacts/uaid86/bindings.json

iroha app space-directory manifest fetch \
  --uaid uaid:86e8ee39a3908460a0f4ee257bb25f340cd5b5de72735e9adefe07d5ef4bb0df \
  --json-out artifacts/uaid86/manifests.json
```

JSON суреттерін шолу кезінде пайдаланылған манифест хэшімен бірге сақтаңыз; the
Space Directory бақылаушысы `uaid_dataspaces` картасын кез келген уақытта қайта жасайды
белсендіру, мерзімі біту немесе жою, сондықтан бұл суреттер дәлелдеудің ең жылдам жолы
белгілі бір дәуірде қандай байланыстар белсенді болды.## 4. Жариялау мүмкіндігі дәлелдермен көрінеді

Жаңа жәрдемақы шығарылған кезде төмендегі CLI ағынын пайдаланыңыз. Әрбір қадам керек
басқаруға қол қою үшін жазылған дәлелдер бумасындағы жер.

1. **JSON манифестін кодтаңыз**, сондықтан тексерушілер детерминирленген хэшті бұрын көреді
   ұсыну:

   ```bash
   iroha app space-directory manifest encode \
     --json fixtures/space_directory/capability/eu_regulator_audit.manifest.json \
     --out artifacts/eu_regulator_audit.manifest.to \
     --hash-out artifacts/eu_regulator_audit.manifest.hash
   ```

2. **Жәрдемақыны жариялау** Norito пайдалы жүктемесін (`--manifest`) немесе
   JSON сипаттамасы (`--manifest-json`). Torii/CLI түбіртегін плюс жазыңыз
   `PublishSpaceDirectoryManifest` нұсқау хэші:

   ```bash
   iroha app space-directory manifest publish \
     --manifest artifacts/eu_regulator_audit.manifest.to \
     --reason "ESMA wave 2 onboarding"
   ```

3. **SpaceDirectoryEvent дәлелдерін түсіріңіз.** Жазылу
   `SpaceDirectoryEvent::ManifestActivated` және оқиғаның пайдалы жүктемесін қосыңыз
   Аудиторлар өзгерістің қашан түскенін растай алатындай пакет.

4. **Аудит бумасын жасаңыз** манифестті оның деректер кеңістігі профиліне байланыстырыңыз және
   телеметриялық ілгектер:

   ```bash
   iroha app space-directory manifest audit-bundle \
     --manifest artifacts/eu_regulator_audit.manifest.to \
     --profile fixtures/space_directory/profile/cbdc_lane_profile.json \
     --out-dir artifacts/eu_regulator_audit_bundle
   ```

5. **Байланыстыруларды Torii** (`bindings fetch` және `manifests fetch`) арқылы тексеріңіз және
   сол JSON файлдарын жоғарыдағы хэш + бумасы бар мұрағаттаңыз.

Дәлелдемелерді тексеру тізімі:

- [ ] Өзгерістерді бекітуші қол қойған манифест хэші (`*.manifest.hash`).
- [ ] Жариялау қоңырауы үшін CLI/Torii түбіртегі (stdout немесе `--json-out` артефакті).
- [ ] `SpaceDirectoryEvent` белсендіруді растайтын пайдалы жүктеме.
- [ ] Деректер кеңістігі профилі, ілгектер және манифест көшірмелері бар аудиторлық жинақ каталогы.
- [ ] Байланыстырулар + Torii белсендіруден кейін алынған манифест суреттері.Бұл SDK беру кезінде `docs/space-directory.md` §3.2 талаптарын көрсетеді.
шығарылымды шолу кезінде көрсететін бір беттің иелері.

## 5. Реттеуші/аймақтық манифест үлгілері

Өнеркәсіп мүмкіндіктерін көрсету кезінде бастапқы нүкте ретінде реподағы құрылғыларды пайдаланыңыз
реттеушілер немесе аймақтық қадағалаушылар үшін. Олар рұқсат ету/бас тартуды қалай көрсету керектігін көрсетеді
ережелер мен саясат ескертпелерін шолушылар күтетін түсіндіріңіз.

| Арматура | Мақсаты | Маңызды сәттер |
|---------|---------|------------|
| `fixtures/space_directory/capability/eu_regulator_audit.manifest.json` | ESMA/ESRB аудит арнасы. | `compliance.audit::{stream_reports, request_snapshot}` үшін тек оқуға арналған жеңілдіктер, реттеушінің UAID-терін пассивті ұстау үшін бөлшек аударымдарды қабылдамау ұтысы. |
| `fixtures/space_directory/capability/jp_regulator_supervision.manifest.json` | JFSA бақылау жолы. | Қосарлы басқару элементтерін қолдану үшін шектелген `cbdc.supervision.issue_stop_order` рұқсатын (күніне терезе + `max_amount`) және `force_liquidation` жүйесінде айқын бас тартуды қосады. |

Осы арматураны клондау кезінде мыналарды жаңартыңыз:

1. `uaid` және `dataspace` идентификаторлары сіз қосып жатқан қатысушы мен жолаққа сәйкес келеді.
2. Басқару кестесіне негізделген `activation_epoch`/`expiry_epoch` терезелері.
3. реттеушінің саясат сілтемелері бар `notes` өрістері (MiCA мақаласы, JFSA
   дөңгелек және т.б.).
4. Рұқсат терезелері (`PerSlot`, `PerMinute`, `PerDay`) және қосымша
   `max_amount` жабындары, сондықтан SDK хост сияқты шектеулерді орындайды.

## 6. SDK тұтынушыларына арналған тасымалдау ескертпесіӘр домен тіркелгісінің идентификаторларына сілтеме жасайтын бар SDK интеграциялары көшуі керек
жоғарыда сипатталған UAID-орталық беттер. Жаңартулар кезінде осы бақылау тізімін пайдаланыңыз:

  тіркелгі идентификаторлары. Rust/JS/Swift/Android үшін бұл соңғы нұсқаға жаңартуды білдіреді
  жұмыс кеңістігінің жәшіктері немесе Norito байламдарын қалпына келтіреді.
- **API қоңыраулары:** Домен ауқымындағы портфолио сұрауларын келесімен ауыстырыңыз
  `GET /v1/accounts/{uaid}/portfolio` және манифест/байланыстырудың соңғы нүктелері.
  `GET /v1/accounts/{uaid}/portfolio` қосымша `asset_id` сұрауын қабылдайды
  әмияндарға тек бір актив данасын қажет ететін параметр. Клиент көмекшілері осындай
  `ToriiClient.getUaidPortfolio` (JS) және Android сияқты
  `SpaceDirectoryClient` бұл маршруттарды орап қойған; тапсырыс бергеннен гөрі оларға артықшылық беріңіз
  HTTP коды.
- **Кэштеу және телеметрия:** шикі емес, UAID + деректер кеңістігі арқылы кэш жазбалары
  тіркелгі идентификаторлары және UAID литералын көрсететін телеметрия шығарыңыз, осылайша операциялар орындалады
  журналдарды Space Directory дәлелдерімен қатарластырыңыз.
- **Қателерді өңдеу:** Жаңа соңғы нүктелер қатаң UAID талдау қателерін қайтарады
  `docs/source/torii/portfolio_api.md` құжатталады; сол кодтарды көрсетіңіз
  сөзбе-сөз, сондықтан қолдау топтары қайталау қадамдарынсыз мәселелерді шеше алады.
- **Тестілеу:** Жоғарыда аталған құрылғыларға сым салыңыз (плюс өзіңіздің UAID манифесттері)
  Norito айналу және манифест бағалауларын дәлелдеу үшін SDK сынақ жинақтарына
  хосттың орындалуына сәйкес келеді.

## 7. Әдебиеттер- `docs/space-directory.md` — өмірлік циклі туралы егжей-тегжейлі операторлық кітап.
- `docs/source/torii/portfolio_api.md` — UAID портфолиосына арналған REST схемасы және
  манифест соңғы нүктелер.
- `crates/iroha_cli/src/space_directory.rs` — CLI енгізу сілтемесі
  осы нұсқаулық.
- `fixtures/space_directory/capability/*.manifest.json` — реттеуші, бөлшек сауда және
  CBDC манифест үлгілері клондауға дайын.

<!-- Auto-generated stub for Bashkir (ba) translation. Replace this content with the full translation. -->

---
lang: ba
direction: ltr
source: docs/source/universal_accounts_guide.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 09a308ecbf07f0293add7f35cf4f1a50b5e6d3630b8b37a8f0f45a7cf82d3924
source_last_modified: "2026-03-30T18:22:55.987822+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

18НХ00000026Х

# Универсаль иҫәп яҙмаһы буйынса ҡулланма

Был ҡулланма дистилляция UAID (Универсаль иҫәп яҙмаһы идентификаторы) ролл-аут талаптарын 2019 йылдан.
Nexus юл картаһы һәм уларҙы операторға + SDK йүнәлтелгән проходкаға йыйып ала.
Ул ҡаплай UAID сығарыу, портфолио/манифест тикшерергә, регулятор шаблондар,
һәм дәлилдәр, улар менән бергә булырға тейеш һәр `iroha ҡушымта йыһан-каталог манифест
18НИ00000027Хюл картаһы.мд:2209`).

## 1. УАИД тиҙ белешмә- УАИД - `uaid:<hex>` литералдары, унда `<hex>` - Блейк2б-256 дайджест, уның
  ЛСБ ҡуйылған Norito. Каноник тибы 1942 йылда йәшәй.
  I18НИ00000031Х.
- Иҫәп яҙмалары (`Account` һәм `AccountDetails`) хәҙер опциональ `uaid` йөрөтә
  ялан шулай ҡушымталар өйрәнә ала идентификаторы индивидуаль хеширование.
- Йәшерен функция идентификаторы сәйәсәте үҙ теләге менән нормалаштырылған индереүҙәрҙе бәйләй ала
  (телефон номерҙары, электрон почта, иҫәп номерҙары, партнер телмәрҙәре) `opaque:` идентификаторҙарына
  UAID исемдәр киңлеге аҫтында. Сылбырҙағы киҫәктәр Torii,
  I18НИ00000037Х, һәм I18НИ00000038Х индексы.
- Космос каталогы `World::uaid_dataspaces` картаһын һаҡлай, ул һәр УАИД-ты бәйләй
  әүҙем манифесттар һылтанма яһаған мәғлүмәттәр киңлеге иҫәптәренә. 18НТ00000006Х ҡабаттан ҡуллана, тип
  карта өсөн `/portfolio` һәм `/uaids/*` API-лар.
- `POST /v1/accounts/onboard` 2019 йыл өсөн ғәҙәттәгесә йыһан каталогы манифестын баҫтырып сығара.
  глобаль мәғлүмәттәр киңлеге ҡасан бер ниндәй ҙә юҡ, шуға күрә UAID шунда уҡ бәйләнгән.
  Онбординг органдары `CanPublishSpaceDirectoryManifest{dataspace=0}` тоторға тейеш.
- Бөтә SDK-лар ҙа UAID литералдарын канонлаштырыу өсөн ярҙамсыларҙы асып һала (мәҫәлән,
  `UaidLiteral` в Android SDK). Ярҙамсылар сеймал ҡабул итә 64-гекс дайджест
  (LSB=1) йәки `uaid:<hex>` литералдар һәм ҡабаттан ҡулланыу шул уҡ Norito кодектар шулай
  дайджест телдәр аша дрейфлай алмай.

## 1.1 Йәшерен идентификатор сәйәсәте

UAIDs хәҙер икенсе үҙенсәлек ҡатламы өсөн якорь булып тора:- Глобаль `IdentifierPolicyId` (`<kind>#<business_rule>`) билдәләй
  исемдәр киңлеге, йәмәғәт йөкләмәһе метамағлүмәттәре, резолютор тикшерелгән асҡыс, һәм
  канонический режим нормализации входа (`Exact`, `LowercaseTrimmed`,
  18НИ00000050Х, 18НИ00000051Х, йәки 18НИ00000052Х).
- Дәғүә бер алынған `opaque:` идентификаторын теүәл бер UAID һәм бер
  канонический `AccountId` шул сәйәсәт буйынса, әммә сылбыр ғына ҡабул итә
  дәғүә ҡасан ул ҡул ҡуйылған `IdentifierResolutionReceipt` оҙатыла.
- Ҡарарлыҡ `resolve -> transfer` ағымы булып ҡала. 18НТ00000007Х асыҡ булмағанды хәл итә
  ручка һәм канон `AccountId` ҡайтара; күсермәләр һаман да маҡсатлы
  канон иҫәп яҙмаһы, түгел, `uaid:` йәки `opaque:` литералдары туранан-тура.
- Сәйәсәт хәҙер BFV индереү-шифрлау параметрҙары аша баҫтырып сығара ала
  I18НИ00000060Х. Ҡасан бар, Torii уларҙы рекламалай
  `GET /v1/identifier-policies`, һәм клиенттар тапшырырға мөмкин BFV-уралған индереү
  ябай текст урынына. Программаланған сәйәсәттәр BFV параметрҙарын урап ала.
  канон `BfvProgrammedPublicParameters` пакет, шулай уҡ баҫтырып сығара
  йәмәғәт I18НИ00000063Х; мираҫ сеймал BFV файҙалы йөкләмәләр яңыртыла, тип
  канонический пакет ҡасан йөкләмә яңынан төҙөлә.
- Идентификатор маршруттары шул уҡ Torii рөхсәт-маркер һәм ставка-сик аша үтә
  тикшерә, башҡа ҡушымта-йөҙө менән ос нөктәләре. Улар тирәләй обход түгел, нормаль
  API сәйәсәте.

## 1.2 Терминология

Исем биреүҙең бүленеше ниәтләп эшләнә:- `ram_lfe` - тышҡы йәшерен функциялы абстракция. Ул сәйәсәтте үҙ эсенә ала
  теркәү, йөкләмәләр, йәмәғәт метамағлүмәттәр, башҡарыу квитанциялары, һәм
  тикшерелгән режим.
- `BFV` - Бракерски/Фан-Веркотерен гомоморф шифрлау схемаһы, ул ҡулланыла.
  ҡайһы бер `ram_lfe` бэкэндтар баһалау өсөн шифрланған индереү.
- `ram_fhe_profile` - BFV-ға хас метамағлүмәттәр, бөтә өсөн икенсе исем түгел
  үҙенсәлеге. Ул программаланған BFV башҡарыу машинаһын һүрәтләй, тип янсыҡтар һәм .
  тикшергәндәр сәйәсәт программаланған бэкэнд ҡулланғанда маҡсатлы булырға тейеш.

Аныҡ ҡына әйткәндә:

- `RamLfeProgramPolicy` һәм `RamLfeExecutionReceipt` - ЛФЭ-ҡатлам типтары.
- 18НИ00000070Х, 18НИ00000071Х, 18НИ00000072Х, һәм
  `BfvRamProgramProfile` типтары FHE-слой.
- 18НИ00000074Х һәм 18НИ00000075Х - эске исемдәр
  программаланған бекэнд тарафынан башҡарылған йәшерен BFV программаһы. Улар 1990 йылда ҡала.
  FHE яғы, сөнки улар шифрланған башҡарыу механизмын һүрәтләй, ә
  тышҡы сәйәсәт йәки алыу абстракцияһы.

## 1.3 Иҫәп яҙмаһының идентификаторы һәм псевдонимдар

Универсаль иҫәп яҙмаһы таратыу канон иҫәп яҙмаһы идентификаторы моделен үҙгәртмәй:- `AccountId` канон, доменһыҙ иҫәп яҙмаһы субъекты булып ҡала.
- `AccountAlias` ҡиммәттәре - был предмет өҫтөндә айырым SNS бәйләүҙәре. А
  домен-квалификациялы псевдоним, мәҫәлән, `merchant@banka.paynet` һәм мәғлүмәт киңлеге-тамыр псевдоним
  мәҫәлән, `merchant@paynet` икеһе лә бер үк канонлы `AccountId`-ҡа хәл итә ала.
- Каноник иҫәп яҙмаһын теркәү һәр ваҡыт `Account::new(AccountId)` /
  I18НИ00000082Х; домен-квалификациялы йәки домен-матдилаштырылған юҡ
  теркәү юлы.
- Домен милекселеге, псевдоним рөхсәттәре һәм башҡа домен даирәһендәге тәртиптәр йәшәй
  үҙ дәүләтендә һәм API-ларҙа түгел, ә иҫәп яҙмаһының үҙенсәлеге буйынса.
- Йәмәғәт иҫәбенә эҙләү был бүленеш эҙләй: псевдоним эҙләүҙәре йәмәғәт ҡала, шул уҡ ваҡытта
  канон иҫәп яҙмаһы шәхесе саф `AccountId` булып ҡала.

Операторҙар, SDK һәм тестар өсөн тормошҡа ашырыу ҡағиҙәһе: канондан башлана
`AccountId`, һуңынан өҫтәү псевдоним ҡуртымға, мәғлүмәттәр киңлеге/домен рөхсәттәре, һәм теләһә ниндәй .
доменға эйә булған дәүләт айырым. Ялған псевдонимдан алынған иҫәп яҙмаһын синтезламағыҙ
йәки көтөп теләһә ниндәй бәйләнгән-домен яланында иҫәп яҙмалары ғына, сөнки псевдоним йәки
маршрут домен сегментын йөрөтә.

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

## 2. УАИД-тарҙы сығарыу һәм раҫлау

Өс ярҙам ысулдары бар, алыу өсөн UAID:

1. **Уны донъя дәүләт йәки SDK моделдәренән уҡығыҙ.
   18NT00000014X аша эҙләнгән файҙалы йөк хәҙер `uaid` яланында заполненный, ҡасан
   ҡатнашыусы универсаль иҫәп яҙмаларына инеүҙе һайланы.
2. **УАИД теркәүҙәрен һорау.** Torii фашлай
   `GET /v1/space-directory/uaids/{uaid}`, был мәғлүмәттәр киңлеге бәйләүҙәрен ҡайтара
   һәм асыҡ метамағлүмәттәр Space Directory хост һаҡлана (ҡара:
   18НИ00000180Х §3 өсөн файҙалы йөк өлгөләре).
3. **Уны детерминистик рәүештә сығарыу.** Ҡасан загрузка яңы UAIDs офлайн, хеш .
   канон ҡатнашыусы орлоҡ менән Blake2b-256 һәм һөҙөмтә менән префикс
   I18НИ00000181Х. Түбәндәге өҙөк көҙгө ярҙамсы документлаштырылған 2012 йылда.
   18НИ00000182Х §3.3:

   ```python
   import hashlib
   seed = b"participant@example"  # canonical address/domain seed
   digest = bytearray(hashlib.blake2b(seed, digest_size=32).digest())
   digest[-1] |= 1
   print(f"uaid:{digest.hex()}")
   ```

Һәр ваҡыт һүҙмә-һүҙҙе бәләкәй хәрефтәр менән һаҡлағыҙ һәм хешлау алдынан аҡ бушлыҡты нормалаштырығыҙ.
CLI ярҙамсылары, мәҫәлән, `iroha app space-directory manifest scaffold` һәм Android
`UaidLiteral` парсер шул уҡ ҡағиҙәләрҙе ҡулланыу ҡырҡыу, шулай идара итеү тикшерергә мөмкин
ҡиммәттәрҙе махсус сценарийҙарһыҙ тикшерергә.

## 3. УАИД холдингтарын һәм манифесттарын тикшергән

18NI00000185X-та детерминистик портфель агрегаторы
өҫтө һәр актив/мәғлүмәт киңлеге пары, тип һылтанмалар UAID. Операторҙар һәм SDK-лар
мәғлүмәттәрҙе түбәндәге йөҙҙәр аша ҡуллана ала:

| Ер өҫтө | Ҡулланыу |
|---------|
| 18НИ00000186Х | Мәғлүмәттәр киңлеген ҡайтара → актив → баланс резюмелары; 18НИ00000187Х-ла һүрәтләнгән. |
| 18НИ00000188Х | Исемлектәр мәғлүмәт киңлеге идентификаторҙары + иҫәп яҙмаһы литералдары бәйләнгән UAID. |
| 18НИ00000189Х | Аудит өсөн тулы `AssetPermissionManifest` тарихын бирә. |
| 18НИ00000191Х | CLI ярлыҡ, был бәйләүҙәрҙең аҙаҡҡы нөктәһен уратып ала һәм теләк буйынса JSON-ды дискҡа яҙа (`--json-out`). |
| 18НИ00000193Х | Дәлилдәр пакеттары өсөн манифест JSON пакетын ала. |

Миҫал CLI сеансы (Torii URL-адресы `torii_api_url` аша `iroha.json`-та конфигурацияланған):

18НФ00000022Х

JSON снимоктарын тикшергәндә ҡулланылған манифест хеш менән бергә һаҡлау; был
Йыһан каталогы күҙәтеүсеһе `uaid_dataspaces` картаһын ҡасан ғына күренһә лә яңынан төҙөй
әүҙемләштереү, срогы үткән, йәки кире ҡайтарыу, шуға күрә был снимоктар иң тиҙ ысул иҫбатлау өсөн .
ниндәй бәйләүҙәр билдәле бер эпохала әүҙем булған.## 4. Баҫтырыу мөмкинлеге дәлилдәр менән күренә

Ҡулланыу CLI ағымы түбәндәге һәр ваҡыт яңы пособие йәйелдерелгән. Һәр аҙым тейеш
ер дәлилдәр өйөмө өсөн теркәлгән идара итеү ҡул ҡуйыу.

1. **Кодировка манифест JSON** шулай итеп, рецензенттар күрә детерминистик хеш алдынан
   тапшырыу:

   18НФ00000023Х

2. **Публикация пособие** ҡулланыу йәки Norito файҙалы йөк (`--manifest`) йәки
   JSON тасуирламаһы (Torii). Яҙыу Torii/CLI квитанцияһы плюс
   18NI00000199X инструкцияһы хеш:

   18НФ00000024Х

3. **Космос каталогы ваҡиғаһы дәлилдәрен тотоу.** Яҙылыу
   Norito һәм ваҡиғаның файҙалы йөкләмәһен үҙ эсенә ала.
   өйөмө шулай аудиторҙар раҫлай ала, ҡасан үҙгәрештәр ергә төшкән.

4. **Аудит пакетын генерациялау** манифестты уның мәғлүмәт киңлеге профиленә бәйләү һәм
   телеметрия ҡармаҡтары:

   18НФ00000025Х

5. **Тикшерергә бәйләүҙәр аша Torii** (`bindings fetch` һәм `manifests fetch`) һәм
   архивлау шул JSON файлдар менән хеш + өҫтәге йыйылма.

Дәлилдәрҙең тикшерелгән исемлеге:

- [ ] Үҙгәрештәрҙе раҫлаусы тарафынан ҡул ҡуйылған асыҡ хеш (`*.manifest.hash`).
- [ ] CLI/Torii квитанция өсөн баҫтырыу шылтыратыуы (stdout йәки `--json-out` артефакт).
- [ ] `SpaceDirectoryEvent` файҙалы йөк иҫбатлау активацияһы.
- [ ] Аудит пакеты каталогы менән мәғлүмәттәр киңлеге профиле, ҡармаҡтар һәм манифест күсермәһе.
- [ ] Бәйләүҙәр + манифест снимоктары Torii пост-активациянан алынған.Был көҙгө талаптарын `docs/space-directory.md` §3.2 биргәндә SDK .
хужалары бер бит күрһәтергә ваҡытында релиз тикшерелгән.

## 5. Регулятор/региональ манифест ҡалыптары

Ҡулланыу-репо ҡоролма башланғыс нөктә булараҡ, ҡасан ҡорамалдар мөмкинлектәрен күрһәтә
көйләүселәр йәки төбәк күҙәтеүселәре өсөн. Улар күрһәтә, нисек даирәһе рөхсәт/инҡар итеү
ҡағиҙәләр һәм аңлатыу сәйәсәт иҫкәрмәләр рецензенттар көтә.

| Ҡоролма | Маҡсат | Һөйләшеүҙәр |
|----------|---------|
| 18НИ00000207Х | ESMA/ESRB аудит каналы. | Уҡыу өсөн генә пособиелар `compliance.audit::{stream_reports, request_snapshot}` менән инҡар-еңә ваҡлап һатыу күсермәләрен һаҡлау өсөн регулятор UAIDs пассив. |
| 18НИ00000209Х | JFSA күҙәтеү һыҙаты. | Өҫтәй, ҡапланған `cbdc.supervision.issue_stop_order` пособие (Көнөнә тәҙрә + `max_amount`) һәм асыҡтан-асыҡ кире ҡағыу `force_liquidation` икеләтә контроль үтәү өсөн. |

Был ҡорамалдарҙы клонлағанда яңыртығыҙ:

1. `uaid` һәм `dataspace` ids ҡатнашыусы һәм һыҙат тура килтерергә һеҙ’ы рөхсәт итеү.
2. И18НИ00000215Х/И18НИ00000216Х тәҙрәләр идара итеү графигы нигеҙендә.
3. `notes` яландар менән көйләүсе’сәйәсәт һылтанмалар (MiCA мәҡәлә, JFSA
   түңәрәк һ.б.).
4. Пособие тәҙрәләре (`PerSlot`, `PerMinute`, `PerDay`) һәм теләк буйынса
   `max_amount` ҡапҡастары шулай SDK-лар хост менән бер үк сиктәрҙе үтәй.

## 6. SDK ҡулланыусылар өсөн миграция иҫкәрмәләреБулған SDK интеграциялары, һылтанма буйынса домен иҫәп яҙмаһы идентификаторҙары күсергә тейеш
өҫтә һүрәтләнгән УАИД-үҙәкле өҫтө. Яңыртыу ваҡытында был тикшерелгән исемлекте ҡулланығыҙ:

  иҫәп яҙмаһы идентификаторҙары. Был өсөн был яңыртыу һуңғы
  эш урыны йәшниктәр йәки регенерациялау Norito бәйләүҙәр.
- **API саҡыра:** Домен даирәһендәге портфолио эҙләүҙәрен алмаштырыу
  `GET /v1/accounts/{uaid}/portfolio` һәм манифест/бәйләүҙәр ос нөктәләре.
  `GET /v1/accounts/{uaid}/portfolio` опциональ `asset_id` эҙләүен ҡабул итә
  параметр ҡасан кошелектар тик бер актив экземпляры кәрәк. Клиент ярҙамсылары бындай
  18НИ00000225Х (ДЖС) һәм Андроид
  `SpaceDirectoryClient` был маршруттарҙы урап инде; уларҙы өҫтөнлөк бирегеҙ, ә заказ буйынса
  HTTP коды.
- **Кэшлау һәм телеметрия:** Кэш яҙмалары UAID + мәғлүмәттәр киңлеге урынына сеймал
  иҫәп яҙмаһы ids, һәм телеметрия сығарыу күрһәтеү UAID туранан-тура шулай операциялар мөмкин
  рәткә журналдар менән йыһан каталогы дәлилдәре.
- **Хаталарҙы эшкәртергә:** Яңы ос нөктәләре ҡәтғи UAID анализлау хаталарын ҡайтара
  I18НИ00000227Х-ла документлаштырылған; өҫтө шул кодтарҙы
  һүҙмә-һүҙ шулай ярҙам командалары мәсьәләләрҙе триажлай ала репро аҙымдарһыҙ.
- **Һынау:** Сым өҫтә телгә алынған ҡоролма (плюс үҙ UAID манифесттары)
  SDK тест пакеттарына иҫбатлау өсөн Norito әйләнеш-сәйәхәттәр һәм баһалауҙарҙы күрһәтеү
  тура килтерергә хост тормошҡа ашырыу.

## 7. Һылтанмалар- `docs/space-directory.md` — оператор плейбук менән тәрән йәшәү циклы деталдәре.
- `docs/source/torii/portfolio_api.md` - UAID портфеле өсөн REST схемаһы һәм
  асыҡ ос нөктәләре.
- 18NI00000230X - CLI тормошҡа ашырыу 2012 йылда һылтанма яһала.
  был ҡулланма.
- `fixtures/space_directory/capability/*.manifest.json` — регулятор, ваҡлап һатыу һәм
  CBDC манифест ҡалыптары клонлау өсөн әҙер.

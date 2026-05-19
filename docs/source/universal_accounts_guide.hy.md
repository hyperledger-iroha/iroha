<!-- Auto-generated stub for Armenian (hy) translation. Replace this content with the full translation. -->

---
lang: hy
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

# Ունիվերսալ հաշվի ուղեցույց

Այս ուղեցույցը թորում է UAID-ի (Համընդհանուր հաշվի ID) ներդրման պահանջները
Nexus ճանապարհային քարտեզը և փաթեթավորելով դրանք օպերատորի + SDK-ի վրա կենտրոնացված շրջագայության մեջ:
Այն ընդգրկում է UAID-ի ստացումը, պորտֆելի/մանիֆեստի ստուգումը, կարգավորիչի ձևանմուշները,
և ապացույցները, որոնք պետք է ուղեկցեն յուրաքանչյուր «iroha» հավելվածի տիեզերական գրացուցակի մանիֆեստին
հրապարակել` run (roadmap reference: `ճանապարհային քարտեզ.md:2209`):

## 1. UAID-ի արագ հղում- UAID-ները `uaid:<hex>` բառացի են, որտեղ `<hex>`-ը Blake2b-256-ի մարսողություն է, որի
  LSB-ը դրված է `1`: Կանոնական տեսակը ապրում է
  `crates/iroha_data_model/src/nexus/manifest.rs::UniversalAccountId`.
- Հաշվի գրառումները (`Account` և `AccountDetails`) այժմ ունեն կամընտիր `uaid`
  դաշտ, որպեսզի հավելվածները կարողանան սովորել նույնացուցիչը՝ առանց պատվերով հեշավորման:
- Թաքնված ֆունկցիայի նույնացուցիչի քաղաքականությունը կարող է կապել կամայական նորմալացված մուտքերը
  (հեռախոսահամարներ, էլ. նամակներ, հաշվի համարներ, գործընկերների տողեր) դեպի `opaque:` ID-ներ
  UAID անվանատարածքի տակ: Շղթայի վրա գտնվող կտորներն են՝ `IdentifierPolicy`,
  `IdentifierClaimRecord` և `opaque_id -> uaid` ինդեքսը:
- Space Directory-ը պահպանում է `World::uaid_dataspaces` քարտեզ, որը կապում է յուրաքանչյուր UAID-ին
  ակտիվ մանիֆեստներով հղվող տվյալների տարածքի հաշիվներին: Torii-ը նորից օգտագործում է դա
  քարտեզ `/portfolio` և `/uaids/*` API-ների համար:
- `POST /v1/accounts/onboard`-ը հրապարակում է լռելյայն Տիեզերական գրացուցակի մանիֆեստը
  գլոբալ տվյալների տարածությունը, երբ ոչ մեկը գոյություն չունի, ուստի UAID-ն անմիջապես կապվում է:
  Գործող մարմինները պետք է ունենան `CanPublishSpaceDirectoryManifest{dataspace=0}`:
- Բոլոր SDK-ները բացահայտում են UAID-ի բառացի կանոնականացման օգնականները (օրինակ՝
  `UaidLiteral` Android SDK-ում): Օգնականներն ընդունում են հում 64 վեցանկյուն մարսողություններ
  (LSB=1) կամ `uaid:<hex>` բառացի և նորից օգտագործեք նույն Norito կոդեկները, որպեսզի
  digest-ը չի կարող շեղվել լեզուներով:

## 1.1 Թաքնված նույնացուցիչի քաղաքականություն

UAID-ները այժմ հանդիսանում են երկրորդ ինքնության շերտի խարիսխը.- Համաշխարհային `IdentifierPolicyId` (`<kind>#<business_rule>`) սահմանում է
  անվանատարածք, հանրային պարտավորությունների մետատվյալներ, լուծիչի հաստատման բանալի և
  կանոնական մուտքագրման նորմալացման ռեժիմ (`Exact`, `LowercaseTrimmed`,
  `PhoneE164`, `EmailAddress` կամ `AccountNumber`):
- Հայցը կապում է մեկ ստացված `opaque:` նույնացուցիչը հենց մեկ UAID-ի և մեկի հետ
  կանոնական `AccountId` համաձայն այդ քաղաքականության, բայց շղթան ընդունում է միայն
  պահանջ, երբ այն ուղեկցվում է ստորագրված `IdentifierResolutionReceipt`-ով:
- Բանաձևը մնում է `resolve -> transfer` հոսք: Torii-ը լուծում է անթափանցությունը
  կարգավորել և վերադարձնում է կանոնական `AccountId`; տրանսֆերները դեռևս ուղղված են
  կանոնական հաշիվ, այլ ոչ թե ուղղակիորեն `uaid:` կամ `opaque:`:
- Քաղաքականություններն այժմ կարող են հրապարակել BFV մուտքագրման գաղտնագրման պարամետրերը
  `PolicyCommitment.public_parameters`. Երբ ներկա է, Torii-ը գովազդում է դրանք
  `GET /v1/identifier-policies`, և հաճախորդները կարող են ներկայացնել BFV-ով փաթաթված մուտքագրում
  պարզ տեքստի փոխարեն: Ծրագրավորված քաղաքականությունները BFV պարամետրերը պարուրում են ա
  կանոնական `BfvProgrammedPublicParameters` փաթեթ, որը նաև հրապարակում է
  հանրային `ram_fhe_profile`; ժառանգված չմշակված BFV բեռնատարները արդիականացվում են դրա վրա
  կանոնական փաթեթ, երբ պարտավորությունը վերակառուցվի:
- Նույնացուցիչ երթուղիները անցնում են նույն Torii մուտքի նշանով և տոկոսադրույքի սահմանաչափով
  ստուգումներ, ինչպես հավելվածի մյուս վերջնակետերը: Դրանք նորմալի շուրջ շրջանցիկ չեն
  API քաղաքականություն.

## 1.2 Տերմինաբանություն

Անվանման բաժանումը միտումնավոր է.- `ram_lfe`-ը արտաքին թաքնված ֆունկցիայի աբստրակցիա է: Այն ընդգրկում է քաղաքականությունը
  գրանցում, պարտավորություններ, հանրային մետատվյալներ, կատարողական անդորրագրեր և
  ստուգման ռեժիմ:
- `BFV`-ը Brakerski/Fan-Vercauteren հոմոմորֆ գաղտնագրման սխեման է, որն օգտագործվում է
  որոշ `ram_lfe` հետնամասեր՝ կոդավորված մուտքագրումը գնահատելու համար:
- `ram_fhe_profile`-ը BFV-ին հատուկ մետատվյալ է, այլ ոչ ամբողջի երկրորդ անուն
  հատկանիշ. Այն նկարագրում է ծրագրավորված BFV կատարման մեքենան, որը դրամապանակներ և
  ստուգողները պետք է թիրախավորեն, երբ քաղաքականությունն օգտագործում է ծրագրավորված հետին պլանը:

Կոնկրետ առումով.

- `RamLfeProgramPolicy` և `RamLfeExecutionReceipt` LFE-շերտային տեսակներ են:
- `BfvParameters`, `BfvCiphertext`, `BfvProgrammedPublicParameters` և
  `BfvRamProgramProfile`-ը FHE շերտի տեսակներն են:
- `HiddenRamFheProgram` և `HiddenRamFheInstruction` ներքին անվանումներ են
  թաքնված BFV ծրագիրը, որն իրականացվում է ծրագրավորված հետնամասի կողմից: Նրանք մնում են
  FHE կողմը, քանի որ նրանք ավելի շուտ նկարագրում են գաղտնագրված կատարման մեխանիզմը, քան
  արտաքին քաղաքականությունը կամ ստացականի աբստրակցիան:

## 1.3 Հաշվի նույնականացումն ընդդեմ կեղծանունների

Ունիվերսալ հաշվի թողարկումը չի փոխում կանոնական հաշվի նույնականացման մոդելը՝- `AccountId`-ը մնում է կանոնական, առանց տիրույթի հաշվի առարկա:
- `AccountAlias` արժեքները առանձին SNS կապեր են այդ թեմայի վերևում: Ա
  տիրույթի համար որակավորված կեղծանուններ, ինչպիսիք են `merchant@banka.paynet` և տվյալների տարածության արմատական անուն
  ինչպիսին է `merchant@paynet`-ը, երկուսն էլ կարող են լուծվել նույն կանոնական `AccountId`-ով:
- Կանոնական հաշվի գրանցումը միշտ `Account::new(AccountId)` /
  `NewAccount::new(AccountId)`; չկա տիրույթով որակավորված կամ դոմենային նյութականացված
  գրանցման ուղին.
- Դոմենի սեփականության իրավունքը, այլանունների թույլտվությունները և տիրույթի շրջանակի այլ վարքագիծը գործում են
  իրենց սեփական վիճակում և API-ներում, այլ ոչ թե բուն հաշվի ինքնության վրա:
- Հանրային հաշվի որոնումը հետևում է այդ պառակտմանը. alias հարցումները մնում են հրապարակային, մինչդեռ
  կանոնական հաշվի ինքնությունը մնում է մաքուր `AccountId`:

Օպերատորների, SDK-ների և թեստերի իրականացման կանոն. սկսել կանոնականից
`AccountId`, այնուհետև ավելացրե՛ք վարձակալական անուններ, տվյալների տարածության/տիրույթի թույլտվություններ և ցանկացած
տիրույթին պատկանող պետությունն առանձին։ Մի սինթեզեք կեղծ կեղծանունից ստացված հաշիվ
կամ ակնկալեք որևէ կապակցված տիրույթի դաշտ հաշվի գրառումներում միայն այն պատճառով, որ կեղծանունը կամ
երթուղին կրում է տիրույթի հատված:

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

## 2. UAID-ների ստացում և ստուգում

UAID ստանալու երեք աջակցվող եղանակ կա.

1. **Կարդացեք այն համաշխարհային պետական կամ SDK մոդելներից։** Ցանկացած `Account`/`AccountDetails`
   Torii-ի միջոցով հարցվող ծանրաբեռնվածությունն այժմ ունի `uaid` դաշտը, երբ
   մասնակիցը միացել է ունիվերսալ հաշիվներին:
2. **Հարցրեք UAID-ի գրանցամատյաններին։** Torii-ը բացահայտում է
   `GET /v1/space-directory/uaids/{uaid}`, որը վերադարձնում է տվյալների տարածության կապերը
   և մանիֆեստի մետատվյալները, որոնք պահպանվում են Space Directory-ի հյուրընկալողը (տես
   `docs/space-directory.md` §3 օգտակար բեռի նմուշների համար):
3. **Դետերմինիստական ձևով ստացեք այն։** Նոր UAID-ները օֆլայն բեռնելիս, հաշեք
   կանոնական մասնակցի սերմը Blake2b-256-ով և արդյունքի նախածանցով
   `uaid:`. Ստորև բերված հատվածը արտացոլում է փաստաթղթավորված օգնականը
   `docs/space-directory.md` §3.3:

   ```python
   import hashlib
   seed = b"participant@example"  # canonical address/domain seed
   digest = bytearray(hashlib.blake2b(seed, digest_size=32).digest())
   digest[-1] |= 1
   print(f"uaid:{digest.hex()}")
   ```Միշտ պահեք բառացի բառը փոքրատառով և նորմալացրեք բացատը նախքան հեշելը:
CLI օգնականներ, ինչպիսիք են `iroha app space-directory manifest scaffold`-ը և Android-ը
`UaidLiteral` վերլուծիչը կիրառում է կրճատման նույն կանոնները, որպեսզի կառավարման վերանայումները կարողանան
խաչաձև ստուգեք արժեքները առանց ժամանակավոր սցենարների:

## 3. UAID-ի ունեցվածքի և մանիֆեստների ստուգում

Պորտֆելի դետերմինիստական ագրեգատորը `iroha_core::nexus::portfolio`-ում
ցուցադրում է յուրաքանչյուր ակտիվ/տվյալների տարածք, որը հղում է անում UAID-ին: Օպերատորներ և SDK-ներ
կարող է սպառել տվյալները հետևյալ մակերեսների միջոցով.

| Մակերեւութային | Օգտագործումը |
|---------|-------|
| `GET /v1/accounts/{uaid}/portfolio` | Վերադարձնում է տվյալների տարածություն → ակտիվ → մնացորդի ամփոփագրեր; նկարագրված է `docs/source/torii/portfolio_api.md`-ում: |
| `GET /v1/space-directory/uaids/{uaid}` | Ցուցակում է տվյալների տարածքի ID-ները + հաշվի բառացիները՝ կապված UAID-ի հետ: |
| `GET /v1/space-directory/uaids/{uaid}/manifests` | Ապահովում է `AssetPermissionManifest` ամբողջական պատմությունը աուդիտի համար: |
| `iroha app space-directory bindings fetch --uaid <literal>` | CLI դյուրանցում, որը փաթաթում է կապի վերջնակետը և ցանկության դեպքում գրում է JSON սկավառակը (`--json-out`): |
| `iroha app space-directory manifest fetch --uaid <literal> --json-out <path>` | Վերցնում է մանիֆեստի JSON փաթեթը՝ ապացույցների փաթեթների համար: |

Օրինակ CLI նստաշրջան (Torii URL կազմաձևված `torii_api_url`-ի միջոցով `iroha.json`-ում):

```bash
iroha app space-directory bindings fetch \
  --uaid uaid:86e8ee39a3908460a0f4ee257bb25f340cd5b5de72735e9adefe07d5ef4bb0df \
  --json-out artifacts/uaid86/bindings.json

iroha app space-directory manifest fetch \
  --uaid uaid:86e8ee39a3908460a0f4ee257bb25f340cd5b5de72735e9adefe07d5ef4bb0df \
  --json-out artifacts/uaid86/manifests.json
```

Պահպանեք JSON-ի նկարները ակնարկների ժամանակ օգտագործվող մանիֆեստի հեշի կողքին. որ
Space Directory դիտորդը վերակառուցում է `uaid_dataspaces` քարտեզը, երբ դրսևորվում է
ակտիվացնել, ժամկետը լրանալ կամ չեղարկել, այնպես որ այս լուսանկարներն ապացուցելու ամենաարագ ճանապարհն են
ինչ կապեր են եղել տվյալ դարաշրջանում։## 4. Հրատարակչական կարողությունը դրսևորվում է ապացույցներով

Օգտագործեք ստորև բերված CLI հոսքը, երբ նոր նպաստ է դուրս գալիս: Յուրաքանչյուր քայլ պետք է
հողը ապացույցների փաթեթում, որը գրանցված է կառավարման ստորագրման համար:

1. **Կոդավորեք JSON մանիֆեստը**, որպեսզի վերանայողները տեսնեն նախապես դետերմինիստական հեշը
   ներկայացում:

   ```bash
   iroha app space-directory manifest encode \
     --json fixtures/space_directory/capability/eu_regulator_audit.manifest.json \
     --out artifacts/eu_regulator_audit.manifest.to \
     --hash-out artifacts/eu_regulator_audit.manifest.hash
   ```

2. **Հրապարակեք նպաստը**՝ օգտագործելով կամ Norito օգտակար բեռը (`--manifest`) կամ
   JSON նկարագրությունը (`--manifest-json`): Գրանցեք Torii/CLI անդորրագիրը գումարած
   `PublishSpaceDirectoryManifest` հրահանգի հեշը՝

   ```bash
   iroha app space-directory manifest publish \
     --manifest artifacts/eu_regulator_audit.manifest.to \
     --reason "ESMA wave 2 onboarding"
   ```

3. **Capture SpaceDirectoryEvent ապացույցները։** Բաժանորդագրվեք
   `SpaceDirectoryEvent::ManifestActivated` և ներառեք միջոցառման օգտակար բեռը
   փաթեթը, որպեսզի աուդիտորները կարողանան հաստատել, թե երբ է տեղի ունեցել փոփոխությունը:

4. **Ստեղծեք աուդիտի փաթեթ**՝ կապելով մանիֆեստը տվյալների տարածության պրոֆիլին և
   Հեռաչափական կեռիկներ.

   ```bash
   iroha app space-directory manifest audit-bundle \
     --manifest artifacts/eu_regulator_audit.manifest.to \
     --profile fixtures/space_directory/profile/cbdc_lane_profile.json \
     --out-dir artifacts/eu_regulator_audit_bundle
   ```

5. **Ստուգեք կապերը Torii**-ի միջոցով (`bindings fetch` և `manifests fetch`) և
   արխիվացրեք այդ JSON ֆայլերը վերևում գտնվող հեշ + փաթեթով:

Ապացույցների ստուգաթերթ.

- [ ] Մանիֆեստի հեշ (`*.manifest.hash`) ստորագրված փոփոխությունը հաստատողի կողմից:
- [ ] CLI/Torii անդորրագիր հրապարակման զանգի համար (stdout կամ `--json-out` արտեֆակտ):
- [ ] `SpaceDirectoryEvent` օգտակար բեռի ակտիվացում:
- [ ] Աուդիտ փաթեթի գրացուցակը տվյալների տարածության պրոֆիլով, կեռիկներով և մանիֆեստի պատճենով:
- [ ] Ամրացումներ + մանիֆեստի նկարներ՝ բերված Torii հետակտիվացումից:Սա արտացոլում է `docs/space-directory.md` §3.2-ի պահանջները SDK-ի տրամադրման ժամանակ
սեփականատերերը մեկ էջ պետք է մատնանշեն թողարկման վերանայումների ժամանակ:

## 5. Կարգավորող/տարածաշրջանային մանիֆեստի ձևանմուշներ

Օգտագործեք ռեպո հարմարանքները որպես մեկնարկային կետեր, երբ դրսևորվում է արհեստագործական ունակություններ
կարգավորողների կամ տարածաշրջանային վերահսկողների համար: Նրանք ցույց են տալիս, թե ինչպես կարելի է թույլատրել/մերժել
կանոնները և բացատրեք այն քաղաքականության նշումները, որոնք ակնկալում են վերանայողները:

| Հարմարանք | Նպատակը | Կարևորություններ |
|---------|---------|------------|
| `fixtures/space_directory/capability/eu_regulator_audit.manifest.json` | ESMA/ESRB աուդիտի հոսք: | `compliance.audit::{stream_reports, request_snapshot}`-ի համար միայն կարդալու արտոնություններ՝ մանրածախ փոխանցումների մերժման դեպքում՝ կարգավորող UAID-ները պասիվ պահելու համար: |
| `fixtures/space_directory/capability/jp_regulator_supervision.manifest.json` | JFSA վերահսկողության գոտի. | Ավելացնում է `cbdc.supervision.issue_stop_order` սահմանափակում (PerDay պատուհան + `max_amount`) և `force_liquidation`-ի բացահայտ մերժում՝ կրկնակի հսկողություն կիրառելու համար: |

Այս սարքերը կլոնավորելիս թարմացրեք՝

1. `uaid` և `dataspace` ID-ներ՝ ձեր կողմից միացված մասնակցին և երթուղուն համապատասխանելու համար:
2. `activation_epoch`/`expiry_epoch` պատուհաններ՝ հիմնված կառավարման ժամանակացույցի վրա:
3. `notes` դաշտեր՝ կարգավորիչի քաղաքականության հղումներով (MiCA հոդված, JFSA
   շրջանաձև և այլն):
4. Նպաստների պատուհաններ (`PerSlot`, `PerMinute`, `PerDay`) և կամընտիր
   `max_amount` փակցված է, որպեսզի SDK-ները կիրառեն նույն սահմանները, ինչ հյուրընկալողը:

## 6. Միգրացիոն նշումներ SDK սպառողների համարԳոյություն ունեցող SDK ինտեգրացիաները, որոնք հղում են կատարում յուրաքանչյուր տիրույթի հաշվի ID-ներին, պետք է տեղափոխվեն
վերը նկարագրված UAID-կենտրոնացված մակերեսները: Օգտագործեք այս ստուգաթերթը թարմացումների ժամանակ.

  հաշվի ID-ներ. Rust/JS/Swift/Android-ի համար սա նշանակում է թարմացում մինչև վերջինը
  աշխատանքային տարածքի արկղեր կամ վերականգնող Norito կապանքներ:
- **API զանգեր.** Փոխարինեք տիրույթի շրջանակով պորտֆելի հարցումները
  `GET /v1/accounts/{uaid}/portfolio` և մանիֆեստի/կապման վերջնակետերը:
  `GET /v1/accounts/{uaid}/portfolio` ընդունում է կամընտիր `asset_id` հարցումը
  պարամետր, երբ դրամապանակներին անհրաժեշտ է միայն մեկ ակտիվի օրինակ: Հաճախորդների օգնականները, ինչպիսիք են
  որպես `ToriiClient.getUaidPortfolio` (JS) և Android
  `SpaceDirectoryClient` արդեն փաթաթում են այս երթուղիները. գերադասեք դրանք պատվիրվածից
  HTTP կոդ.
- **Քեշավորում և հեռաչափություն.** Քեշի գրառումներ UAID-ով + տվյալների տարածություն՝ չմշակվածի փոխարեն
  հաշվի ID-ներ և արտանետում հեռաչափություն, որը ցույց է տալիս UAID-ը բառացիորեն, որպեսզի գործողությունները կարողանան
  շարել տեղեկամատյանները Space Directory-ի ապացույցներով:
- **Սխալների մշակում. ** Նոր վերջնակետերը վերադարձնում են UAID-ի վերլուծման խիստ սխալները
  փաստաթղթավորված `docs/source/torii/portfolio_api.md`-ում; բացահայտեք այդ ծածկագրերը
  բառացիորեն, որպեսզի աջակցող թիմերը կարողանան շտկել խնդիրները առանց կրկնօրինակ քայլերի:
- **Փորձարկում. ** Միացրեք վերը նշված հարմարանքները (գումարած ձեր սեփական UAID մանիֆեստները)
  SDK թեստային փաթեթների մեջ՝ Norito հետադարձ ուղևորություններն ապացուցելու և գնահատականները ցուցադրելու համար
  համապատասխանել հյուրընկալող իրականացմանը:

## 7. Հղումներ- `docs/space-directory.md` — օպերատորի խաղագիրք ավելի խորը կյանքի ցիկլի մանրամասներով:
- `docs/source/torii/portfolio_api.md` — REST սխեման UAID պորտֆելի համար և
  բացահայտ վերջնակետեր.
- `crates/iroha_cli/src/space_directory.rs` — CLI իրականացումը նշված է
  այս ուղեցույցը:
- `fixtures/space_directory/capability/*.manifest.json` — կարգավորիչ, մանրածախ և
  CBDC մանիֆեստի ձևանմուշները պատրաստ են կլոնավորման համար:

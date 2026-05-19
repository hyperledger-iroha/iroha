<!-- Auto-generated stub for Georgian (ka) translation. Replace this content with the full translation. -->

---
lang: ka
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

# უნივერსალური ანგარიშის სახელმძღვანელო

ეს სახელმძღვანელო ასახავს UAID-ის (უნივერსალური ანგარიშის ID) მოთხოვნებს
Nexus საგზაო რუკა და ათავსებს მათ ოპერატორზე + SDK ორიენტირებულ გზამკვლევში.
იგი მოიცავს UAID-ის დერივაციას, პორტფელის/მანიფესტის შემოწმებას, მარეგულირებლის შაბლონებს,
და მტკიცებულება, რომელიც უნდა ახლდეს ყველა `iroha app space-directory manifest-ს
public` run (roadmap reference: `roadmap.md:2209`).

## 1. UAID-ის სწრაფი მითითება- UAIDs არის `uaid:<hex>` ლიტერალი, სადაც `<hex>` არის Blake2b-256 დაიჯესტი
  LSB დაყენებულია `1`-ზე. კანონიკური ტიპი ცხოვრობს
  `crates/iroha_data_model/src/nexus/manifest.rs::UniversalAccountId`.
- ანგარიშის ჩანაწერებს (`Account` და `AccountDetails`) ახლა აქვს სურვილისამებრ `uaid`
  ველი, რათა აპლიკაციებმა ისწავლონ იდენტიფიკატორი შეკვეთილი ჰეშირების გარეშე.
- ფარული ფუნქციის იდენტიფიკატორის პოლიტიკას შეუძლია დააკავშიროს თვითნებური ნორმალიზებული შენატანი
  (ტელეფონის ნომრები, ელფოსტა, ანგარიშის ნომრები, პარტნიორის სტრიქონები) `opaque:` ID-ებზე
  UAID სახელთა სივრცის ქვეშ. ჯაჭვის ნაწილები არის `IdentifierPolicy`,
  `IdentifierClaimRecord` და `opaque_id -> uaid` ინდექსი.
- Space Directory ინახავს `World::uaid_dataspaces` რუკას, რომელიც აკავშირებს თითოეულ UAID-ს
  მონაცემთა სივრცის ანგარიშებზე, რომლებიც მითითებულია აქტიური მანიფესტების მიერ. Torii ხელახლა იყენებს ამას
  რუკა `/portfolio` და `/uaids/*` API-ებისთვის.
- `POST /v1/accounts/onboard` აქვეყნებს ნაგულისხმევი Space Directory მანიფესტს
  გლობალური მონაცემთა სივრცე, როდესაც არცერთი არ არსებობს, ამიტომ UAID დაუყოვნებლივ იკვრება.
  საბორტო ორგანოებმა უნდა დაიცვან `CanPublishSpaceDirectoryManifest{dataspace=0}`.
- ყველა SDK ავლენს დამხმარეებს UAID ლიტერალების კანონიკიზაციისთვის (მაგ.,
  `UaidLiteral` Android SDK-ში). დამხმარეები იღებენ ნედლეულ 64 თექვსმეტიან დიჯესტს
  (LSB=1) ან `uaid:<hex>` ლიტერალები და ხელახლა გამოიყენეთ იგივე Norito კოდეკები, ასე რომ
  დაიჯესტი ვერ გადაინაცვლებს ენებზე.

## 1.1 დამალული იდენტიფიკატორის პოლიტიკა

UAID-ები ახლა არის მეორე იდენტურობის ფენის წამყვანი:- გლობალური `IdentifierPolicyId` (`<kind>#<business_rule>`) განსაზღვრავს
  სახელთა სივრცე, საჯარო ვალდებულების მეტამონაცემები, გადამწყვეტის დადასტურების გასაღები და
  კანონიკური შეყვანის ნორმალიზაციის რეჟიმი (`Exact`, `LowercaseTrimmed`,
  `PhoneE164`, `EmailAddress`, ან `AccountNumber`).
- პრეტენზია აკავშირებს ერთ მიღებულ `opaque:` იდენტიფიკატორს ზუსტად ერთ UAID-თან და ერთთან
  კანონიკური `AccountId` ამ პოლიტიკის მიხედვით, მაგრამ ჯაჭვი იღებს მხოლოდ
  პრეტენზია, როდესაც მას ახლავს ხელმოწერილი `IdentifierResolutionReceipt`.
- გარჩევადობა რჩება `resolve -> transfer` ნაკადად. Torii წყვეტს გაუმჭვირვალობას
  ამუშავებს და აბრუნებს კანონიკურ `AccountId`-ს; ტრანსფერები კვლავ მიზნად ისახავს
  კანონიკური ანგარიში და არა პირდაპირ `uaid:` ან `opaque:`.
- პოლიტიკას ახლა შეუძლია BFV შეყვანის დაშიფვრის პარამეტრების გამოქვეყნება
  `PolicyCommitment.public_parameters`. როდესაც არსებობს, Torii აქვეყნებს მათ რეკლამას
  `GET /v1/identifier-policies` და კლიენტებს შეუძლიათ წარადგინონ BFV-შეფუთული შენატანი
  ჩვეულებრივი ტექსტის ნაცვლად. დაპროგრამებული პოლიტიკა ახვევს BFV პარამეტრებს ა
  კანონიკური `BfvProgrammedPublicParameters` პაკეტი, რომელიც ასევე აქვეყნებს
  საჯარო `ram_fhe_profile`; მემკვიდრეობითი ნედლეული BFV დატვირთვები განახლებულია მასზე
  კანონიკური შეკვრა, როდესაც ვალდებულება აღდგება.
- იდენტიფიკატორის მარშრუტები გადის იმავე Torii წვდომის ნიშნით და განაკვეთის ლიმიტით
  ამოწმებს, როგორც სხვა აპის საბოლოო წერტილებს. ისინი არ არიან შემოვლითი გზა ნორმალურის გარშემო
  API პოლიტიკა.

## 1.2 ტერმინოლოგია

დასახელების გაყოფა მიზანმიმართულია:- `ram_lfe` არის გარე ფარული ფუნქციის აბსტრაქცია. ის მოიცავს პოლიტიკას
  რეგისტრაცია, ვალდებულებები, საჯარო მეტამონაცემები, შესრულების ქვითრები და
  გადამოწმების რეჟიმი.
- `BFV` არის Brakerski/Fan-Vercauteren ჰომორფული დაშიფვრის სქემა, რომელსაც იყენებს
  ზოგიერთი `ram_lfe` backends დაშიფრული შეყვანის შესაფასებლად.
- `ram_fhe_profile` არის BFV-სპეციფიკური მეტამონაცემები და არა მთლიანი მეორე სახელი
  თვისება. იგი აღწერს დაპროგრამებულ BFV აღსრულების მანქანას, რომელიც საფულეებს და
  ვერიფიკატორები უნდა იყოს მიზანმიმართული, როდესაც პოლიტიკა იყენებს დაპროგრამებულ ბექენდს.

კონკრეტულად:

- `RamLfeProgramPolicy` და `RamLfeExecutionReceipt` არის LFE ფენის ტიპები.
- `BfvParameters`, `BfvCiphertext`, `BfvProgrammedPublicParameters` და
  `BfvRamProgramProfile` არის FHE ფენის ტიპები.
- `HiddenRamFheProgram` და `HiddenRamFheInstruction` არის შიდა სახელები
  ფარული BFV პროგრამა, რომელიც შესრულებულია დაპროგრამებული ბექენდის მიერ. ისინი რჩებიან
  FHE მხარეს, რადგან ისინი აღწერენ დაშიფრული შესრულების მექანიზმს, ვიდრე
  გარე პოლიტიკა ან ქვითრის აბსტრაქცია.

## 1.3 ანგარიშის იდენტურობა მეტსახელების წინააღმდეგ

უნივერსალური ანგარიშის გაშვება არ ცვლის ანგარიშის კანონიკური იდენტობის მოდელს:- `AccountId` რჩება ანგარიშის კანონიკურ, დომენის გარეშე.
- `AccountAlias` მნიშვნელობები არის ცალკეული SNS შეკვრა ამ საგნის თავზე. ა
  დომენის კვალიფიცირებული მეტსახელი, როგორიცაა `merchant@banka.paynet` და dataspace-root მეტსახელი
  როგორიცაა `merchant@paynet` შეიძლება ორივე გადაჭრას იმავე კანონიკურ `AccountId`-ზე.
- კანონიკური ანგარიშის რეგისტრაცია ყოველთვის არის `Account::new(AccountId)` /
  `NewAccount::new(AccountId)`; არ არსებობს დომენით კვალიფიცირებული ან დომენით მატერიალიზებული
  რეგისტრაციის გზა.
- დომენის საკუთრება, მეტსახელის ნებართვები და დომენის მასშტაბის სხვა ქცევები ცოცხალია
  საკუთარ სახელმწიფოში და API-ებში და არა თავად ანგარიშის იდენტურობაში.
- საჯარო ანგარიშის ძებნა შემდეგნაირად ხდება ამ გაყოფა: ალიასის მოთხოვნები რჩება საჯარო, ხოლო
  კანონიკური ანგარიშის იდენტურობა რჩება სუფთა `AccountId`.

განხორციელების წესი ოპერატორებისთვის, SDK-ებისთვის და ტესტებისთვის: დაიწყეთ კანონიკურიდან
`AccountId`, შემდეგ დაამატეთ მეტსახელის იჯარა, მონაცემთა სივრცის/დომენის ნებართვები და ნებისმიერი
დომენის საკუთრებაში არსებული სახელმწიფო ცალკე. არ მოახდინოთ ყალბი ფსევდონიმით მიღებული ანგარიშის სინთეზირება
ან ველით რაიმე დაკავშირებული დომენის ველს ანგარიშის ჩანაწერებში მხოლოდ იმიტომ, რომ მეტსახელი ან
მარშრუტი ატარებს დომენის სეგმენტს.

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

## 2. UAID-ების გამომუშავება და გადამოწმება

UAID-ის მისაღებად სამი მხარდაჭერილი გზა არსებობს:

1. **წაიკითხეთ მსოფლიო სახელმწიფოს ან SDK მოდელებიდან.** ნებისმიერი `Account`/`AccountDetails`
   Torii-ის მეშვეობით მოთხოვნილი დატვირთვის ველი ახლა შევსებულია `uaid`, როდესაც
   მონაწილემ აირჩია უნივერსალური ანგარიშები.
2. **შეიკითხეთ UAID-ის რეესტრებში.** Torii ასახავს
   `GET /v1/space-directory/uaids/{uaid}`, რომელიც აბრუნებს მონაცემთა სივრცის კავშირებს
   და მანიფესტის მეტამონაცემები, რომელსაც Space Directory მასპინძელი რჩება (იხ
   `docs/space-directory.md` §3 დატვირთვის ნიმუშებისთვის).
3. **მიიღეთ იგი დეტერმინისტულად.** ახალი UAID-ების ოფლაინ ჩატვირთვისას, ჰეშ
   კანონიკური მონაწილე დათესეს Blake2b-256-ით და დააფიქსირეს შედეგი
   `uaid:`. ქვემოთ მოცემული ფრაგმენტი ასახავს დამხმარეს, რომელიც დოკუმენტირებულია
   `docs/space-directory.md` §3.3:

   ```python
   import hashlib
   seed = b"participant@example"  # canonical address/domain seed
   digest = bytearray(hashlib.blake2b(seed, digest_size=32).digest())
   digest[-1] |= 1
   print(f"uaid:{digest.hex()}")
   ```ყოველთვის შეინახეთ ლიტერალი მცირე ასოებით და დაარეგულირეთ სივრცე ჰეშირების წინ.
CLI დამხმარეები, როგორიცაა `iroha app space-directory manifest scaffold` და Android
`UaidLiteral` პარსერი იყენებს იგივე მორთვის წესებს, რათა მმართველობის მიმოხილვამ შეძლოს
გადაამოწმეთ მნიშვნელობები ad hoc სკრიპტების გარეშე.

## 3. UAID ჰოლდინგისა და მანიფესტების შემოწმება

დეტერმინისტული პორტფელის აგრეგატორი `iroha_core::nexus::portfolio`-ში
ასახავს ყველა აქტივს/მონაცემთა სივრცის წყვილს, რომელიც მიმართავს UAID-ს. ოპერატორები და SDK-ები
შეუძლია მონაცემების მოხმარება შემდეგი ზედაპირების მეშვეობით:

| ზედაპირი | გამოყენება |
|---------|-------|
| `GET /v1/accounts/{uaid}/portfolio` | აბრუნებს მონაცემთა სივრცეს → აქტივს → ბალანსის შეჯამებებს; აღწერილია `docs/source/torii/portfolio_api.md`-ში. |
| `GET /v1/space-directory/uaids/{uaid}` | ჩამოთვლის მონაცემთა სივრცის ID-ებს + ანგარიშების ლიტერალებს, რომლებიც დაკავშირებულია UAID-თან. |
| `GET /v1/space-directory/uaids/{uaid}/manifests` | გთავაზობთ სრულ `AssetPermissionManifest` ისტორიას აუდიტებისთვის. |
| `iroha app space-directory bindings fetch --uaid <literal>` | CLI მალსახმობი, რომელიც ახვევს საკინძების ბოლო წერტილს და სურვილისამებრ წერს JSON დისკზე (`--json-out`). |
| `iroha app space-directory manifest fetch --uaid <literal> --json-out <path>` | იღებს მანიფესტის JSON პაკეტს მტკიცებულებების პაკეტებისთვის. |

CLI სესიის მაგალითი (Torii URL კონფიგურირებულია `torii_api_url`-ის მეშვეობით `iroha.json`-ში):

```bash
iroha app space-directory bindings fetch \
  --uaid uaid:86e8ee39a3908460a0f4ee257bb25f340cd5b5de72735e9adefe07d5ef4bb0df \
  --json-out artifacts/uaid86/bindings.json

iroha app space-directory manifest fetch \
  --uaid uaid:86e8ee39a3908460a0f4ee257bb25f340cd5b5de72735e9adefe07d5ef4bb0df \
  --json-out artifacts/uaid86/manifests.json
```

შეინახეთ JSON სნეპშოტები მიმოხილვის დროს გამოყენებული მანიფესტის ჰეშის გვერდით; The
Space Directory დამკვირვებელი აღადგენს `uaid_dataspaces` რუკას, როდესაც ეს გამოჩნდება
გაააქტიურეთ, იწურება ან გააუქმეთ, ასე რომ, ეს კადრები დასამტკიცებლად ყველაზე სწრაფი გზაა
რა კავშირები იყო აქტიური მოცემულ ეპოქაში.## 4. გამოქვეყნების უნარი ვლინდება მტკიცებულებებით

გამოიყენეთ ქვემოთ მოცემული CLI ნაკადი, როდესაც ახალი შემწეობა გამოვა. ყოველი ნაბიჯი უნდა
მიწის ნაკვეთი მტკიცებულებათა პაკეტში, რომელიც ჩაწერილია მმართველობის ხელმოწერისთვის.

1. **დაშიფვრეთ მანიფესტი JSON**, რათა მიმომხილველებმა ადრე დაინახონ დეტერმინისტული ჰეში
   წარდგენა:

   ```bash
   iroha app space-directory manifest encode \
     --json fixtures/space_directory/capability/eu_regulator_audit.manifest.json \
     --out artifacts/eu_regulator_audit.manifest.to \
     --hash-out artifacts/eu_regulator_audit.manifest.hash
   ```

2. **გამოაქვეყნეთ შემწეობა** Norito დატვირთვის (`--manifest`) გამოყენებით ან
   JSON აღწერა (`--manifest-json`). ჩაწერეთ Torii/CLI ქვითარი პლუს
   `PublishSpaceDirectoryManifest` ინსტრუქციის ჰეში:

   ```bash
   iroha app space-directory manifest publish \
     --manifest artifacts/eu_regulator_audit.manifest.to \
     --reason "ESMA wave 2 onboarding"
   ```

3. **SapceDirectoryEvent მტკიცებულებების გადაღება.** გამოწერა
   `SpaceDirectoryEvent::ManifestActivated` და მოიცავს ღონისძიების დატვირთვას
   პაკეტი, რათა აუდიტორებმა დაადასტურონ, როდის მოხდა ცვლილება.

4. ** შექმენით აუდიტის ნაკრები ** აკავშირებს manifest-ს მის მონაცემთა სივრცის პროფილთან და
   ტელემეტრიული კაკვები:

   ```bash
   iroha app space-directory manifest audit-bundle \
     --manifest artifacts/eu_regulator_audit.manifest.to \
     --profile fixtures/space_directory/profile/cbdc_lane_profile.json \
     --out-dir artifacts/eu_regulator_audit_bundle
   ```

5. **შეამოწმეთ შეკვრა Torii** (`bindings fetch` და `manifests fetch`) და
   დაარქივეთ ეს JSON ფაილები ზემოთ ჰეშით + პაკეტით.

მტკიცებულებების ჩამონათვალი:

- [ ] მანიფესტის ჰეში (`*.manifest.hash`) ხელმოწერილია ცვლილების დამმტკიცებლის მიერ.
- [ ] CLI/Torii ქვითარი გამოქვეყნების ზარისთვის (stdout ან `--json-out` არტეფაქტი).
- [ ] `SpaceDirectoryEvent` დატვირთვის დამადასტურებელი გააქტიურება.
- [ ] აუდიტის ნაკრების დირექტორია მონაცემთა სივრცის პროფილით, კაკვებით და მანიფესტის ასლით.
- [ ] Bindings + მანიფესტის სნეპშოტები მოტანილია Torii პოსტ-აქტივაციისგან.ეს ასახავს `docs/space-directory.md` §3.2 მოთხოვნებს SDK-ის მიცემისას
მფლობელებს ერთი გვერდი უნდა მიუთითონ გამოშვების მიმოხილვის დროს.

## 5. მარეგულირებელი/რეგიონული მანიფესტის შაბლონები

გამოიყენეთ in-repo მოწყობილობები, როგორც საწყისი წერტილები, როდესაც გამოვლინდება ხელოსნობის შესაძლებლობები
მარეგულირებელი ან რეგიონალური ზედამხედველებისთვის. ისინი აჩვენებენ, თუ როგორ უნდა მოხდეს დაშვების/უარის ფარგლები
წესები და აუხსენით პოლიტიკის შენიშვნებს, რომლებსაც მიმომხილველები მოელიან.

| მოწყობილობა | დანიშნულება | მაჩვენებლები |
|---------|--------|------------|
| `fixtures/space_directory/capability/eu_regulator_audit.manifest.json` | ESMA/ESRB აუდიტის არხი. | მხოლოდ წაკითხვის შეღავათები `compliance.audit::{stream_reports, request_snapshot}`-ისთვის საცალო გადარიცხვებზე უარყოფით-მოგებით, რათა მარეგულირებელი UAID-ები იყოს პასიური. |
| `fixtures/space_directory/capability/jp_regulator_supervision.manifest.json` | JFSA ზედამხედველობის ჩიხი. | ამატებს შეზღუდული `cbdc.supervision.issue_stop_order` შემწეობას (PerDay ფანჯარა + `max_amount`) და აშკარა უარყოფას `force_liquidation`-ზე ორმაგი კონტროლის განსახორციელებლად. |

ამ მოწყობილობების კლონირებისას განაახლეთ:

1. `uaid` და `dataspace` ID, რომელიც ემთხვევა მონაწილესა და ზოლს, რომელსაც ჩართავთ.
2. `activation_epoch`/`expiry_epoch` ფანჯრები მართვის განრიგის მიხედვით.
3. `notes` ველები მარეგულირებლის პოლიტიკის მითითებით (MiCA სტატია, JFSA
   წრიული და ა.შ.).
4. დამხმარე ფანჯრები (`PerSlot`, `PerMinute`, `PerDay`) და სურვილისამებრ
   `max_amount` იფარება, ასე რომ SDK-ები ახორციელებენ იგივე ლიმიტებს, როგორც მასპინძელი.

## 6. მიგრაციის შენიშვნები SDK მომხმარებლებისთვისარსებული SDK ინტეგრაციები, რომლებზეც მითითებულია თითო დომენის ანგარიშის ID, უნდა გადავიდეს
ზემოთ აღწერილი UAID-ზე ორიენტირებული ზედაპირები. გამოიყენეთ ეს სია განახლებების დროს:

  ანგარიშის ID. Rust/JS/Swift/Android-ისთვის ეს ნიშნავს განახლებას უახლესზე
  სამუშაო სივრცის ყუთები ან Norito საკინძების რეგენერაცია.
- **API ზარები:** შეცვალეთ დომენის ფარგლების პორტფოლიო მოთხოვნებით
  `GET /v1/accounts/{uaid}/portfolio` და მანიფესტის/დაკავშირების ბოლო წერტილები.
  `GET /v1/accounts/{uaid}/portfolio` იღებს არასავალდებულო `asset_id` მოთხოვნას
  პარამეტრი, როდესაც საფულეებს სჭირდებათ მხოლოდ ერთი აქტივის მაგალითი. კლიენტების დამხმარეები ასეთი
  როგორც `ToriiClient.getUaidPortfolio` (JS) და Android
  `SpaceDirectoryClient` უკვე ახვევს ამ მარშრუტებს; უპირატესობა მიანიჭეთ მათ შეკვეთილზე
  HTTP კოდი.
- **ქეშირება და ტელემეტრია:** ქეში ჩანაწერები UAID-ით + მონაცემთა სივრცე ნედლეულის ნაცვლად
  ანგარიშის იდენტიფიკატორი და ასხივებენ ტელემეტრიას, რომელიც აჩვენებს UAID-ს პირდაპირი მნიშვნელობით, ასე რომ ოპერაციებს შეუძლიათ
  დაალაგეთ ჟურნალები Space Directory-ის მტკიცებულებებით.
- **შეცდომის დამუშავება:** ახალი საბოლოო წერტილები აბრუნებს UAID-ის გარჩევის მკაცრ შეცდომებს
  დოკუმენტირებულია `docs/source/torii/portfolio_api.md`-ში; ამოიღეთ ეს კოდები
  სიტყვასიტყვით, რათა დამხმარე გუნდებმა შეძლონ პრობლემების გადაჭრა რეპრო ნაბიჯების გარეშე.
- ** ტესტირება: ** გადაიტანეთ ზემოთ ნახსენები მოწყობილობები (პლუს თქვენი საკუთარი UAID მანიფესტები)
  SDK ტესტის კომპლექტებში Norito ორმხრივი მგზავრობისა და მანიფესტი შეფასებების დასამტკიცებლად
  ემთხვევა მასპინძლის განხორციელებას.

## 7. ლიტერატურა- `docs/space-directory.md` — ოპერატორის სათამაშო წიგნი სასიცოცხლო ციკლის უფრო ღრმა დეტალებით.
- `docs/source/torii/portfolio_api.md` — REST სქემა UAID პორტფოლიოსთვის და
  მანიფესტი საბოლოო წერტილები.
- `crates/iroha_cli/src/space_directory.rs` — CLI განხორციელება მითითებულია
  ამ სახელმძღვანელოს.
- `fixtures/space_directory/capability/*.manifest.json` — რეგულატორი, საცალო და
  CBDC მანიფესტის შაბლონები მზად არის კლონირებისთვის.

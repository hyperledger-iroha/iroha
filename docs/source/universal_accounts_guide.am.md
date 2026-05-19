<!-- Auto-generated stub for Amharic (Ethiopian) (am) translation. Replace this content with the full translation. -->

---
lang: am
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

# ሁለንተናዊ መለያ መመሪያ

ይህ መመሪያ የ UAID (ሁለንተናዊ መለያ መታወቂያ) ልቀት መስፈርቶችን ያስወግዳል
የ Nexus የመንገድ ካርታ እና ጥቅል ወደ ኦፕሬተር + ኤስዲኬ ያተኮረ የእግር ጉዞ።
የ UAID አመጣጥን፣ ፖርትፎሊዮ/ገላጭ ፍተሻን፣ የተቆጣጣሪ አብነቶችን፣
እና ከእያንዳንዱ `iroha መተግበሪያ ቦታ-ማውጫ ዝርዝር መግለጫ ጋር አብሮ መሆን ያለበት ማስረጃ
print` run (roadmap reference: `roadmap.md:2209`)።

## 1. የ UAID ፈጣን ማጣቀሻ- UAIDs `uaid:<hex>` ቀጥተኛ ቃላት ሲሆኑ `<hex>` የ Blake2b-256 መፈጨት
  LSB ወደ `1` ተቀናብሯል። ቀኖናዊው ዓይነት ይኖራል
  `crates/iroha_data_model/src/nexus/manifest.rs::UniversalAccountId`.
- የመለያ መዝገቦች (`Account` እና `AccountDetails`) አሁን አማራጭ `uaid` ይይዛሉ
  መስክ ስለዚህ አፕሊኬሽኖች ያለ ሀሺንግ መለያውን መማር ይችላሉ።
- የተደበቀ ተግባር ለዪ ፖሊሲዎች የዘፈቀደ መደበኛ ግብዓቶችን ማሰር ይችላሉ።
  (ስልክ ቁጥሮች፣ ኢሜይሎች፣ የመለያ ቁጥሮች፣ የአጋር ሕብረቁምፊዎች) ወደ `opaque:` መታወቂያዎች
  በUAID የስም ቦታ። በሰንሰለት ላይ ያሉት ቁርጥራጮች `IdentifierPolicy`፣
  `IdentifierClaimRecord`፣ እና `opaque_id -> uaid` ኢንዴክስ።
- የስፔስ ማውጫ እያንዳንዱን UAID የሚያገናኝ `World::uaid_dataspaces` ካርታ ይይዛል
  በንቁ አንጸባራቂዎች ለተጠቀሱት የውሂብ ቦታ መለያዎች። Torii ያንን እንደገና ይጠቀማል
  ካርታ ለ`/portfolio` እና `/uaids/*` APIs።
- `POST /v1/accounts/onboard` ነባሪ የስፔስ ማውጫ መግለጫ ያትማል
  የአለምአቀፍ የመረጃ ቦታ ምንም በማይኖርበት ጊዜ፣ ስለዚህ UAID ወዲያውኑ ይታሰራል።
  ተሳፋሪ ባለስልጣናት `CanPublishSpaceDirectoryManifest{dataspace=0}` መያዝ አለባቸው።
- ሁሉም ኤስዲኬዎች የ UAID ቃል በቃል እንዲገልጹ ረዳቶችን ያጋልጣሉ (ለምሳሌ፣
  `UaidLiteral` በአንድሮይድ ኤስዲኬ)። ረዳቶቹ ጥሬ 64-ሄክስ መፍጨት ይቀበላሉ
  (LSB=1) ወይም `uaid:<hex>` ቀጥታ ቃላት እና ተመሳሳዩን Norito ኮዴኮችን እንደገና ተጠቀም
  መፍጨት በቋንቋዎች መንሸራተት አይችልም።

## 1.1 የተደበቁ መለያ መመሪያዎች

ዩአይዲዎች አሁን ለሁለተኛው የማንነት ንብርብር መልህቅ ናቸው፡-- ዓለም አቀፍ `IdentifierPolicyId` (`<kind>#<business_rule>`) ይገልጻል
  የስም ቦታ፣ የህዝብ ቁርጠኝነት ሜታዳታ፣ ፈቺ የማረጋገጫ ቁልፍ እና የ
  ቀኖናዊ ግቤት መደበኛ ሁነታ (`Exact`፣ `LowercaseTrimmed`፣
  `PhoneE164`፣ `EmailAddress`፣ ወይም `AccountNumber`)።
- የይገባኛል ጥያቄ አንድን `opaque:` መለያን ከአንድ UAID እና አንድ ጋር ያገናኛል
  ቀኖናዊ `AccountId` በዚያ ፖሊሲ መሠረት፣ ነገር ግን ሰንሰለቱ የሚቀበለው
  የይገባኛል ጥያቄ ከተፈረመ `IdentifierResolutionReceipt` ጋር ሲታጀብ።
- ጥራት የ `resolve -> transfer` ፍሰት ይቀራል። Torii ግልጽ ያልሆነውን ይፈታል።
  ቀኖናዊውን `AccountId` በመያዝ ይመልሳል; ዝውውሮች አሁንም ዒላማው ናቸው
  ቀኖናዊ መለያ፣ በቀጥታ `uaid:` ወይም `opaque:` አይደለም።
- ፖሊሲዎች አሁን የBFV ግቤት-ምስጠራ መለኪያዎችን በዚህ በኩል ማተም ይችላሉ።
  `PolicyCommitment.public_parameters`. ባሉበት ጊዜ፣ Torii ያስተዋውቃቸዋል።
  `GET /v1/identifier-policies`፣ እና ደንበኞች BFV-የተጠቀለለ ግቤት ማስገባት ይችላሉ።
  ግልጽ በሆነ ጽሑፍ ፋንታ. በፕሮግራም የተቀመጡ ፖሊሲዎች የBFV መለኪያዎችን በ ሀ
  ቀኖናዊ `BfvProgrammedPublicParameters` ቅርቅብ እሱ ደግሞ የሚያሳትመው
  የህዝብ `ram_fhe_profile`; የቆዩ ጥሬ BFV ጭነቶች በዚያ ላይ ተሻሽለዋል።
  ቁርጠኝነት እንደገና ሲገነባ ቀኖናዊ ጥቅል።
- የመለያ መንገዶች የሚሄዱት በተመሳሳዩ Torii የመድረሻ ማስመሰያ እና የፍጥነት ገደብ ነው።
  እንደ ሌሎች መተግበሪያ የሚመለከቱ የመጨረሻ ነጥቦችን ይፈትሻል። እነሱ በተለመደው አካባቢ ማለፊያ አይደሉም
  የኤፒአይ ፖሊሲ።

## 1.2 ቃላት

የስያሜ ክፍፍል ሆን ተብሎ የተደረገ ነው፡-- `ram_lfe` ውጫዊ የተደበቀ ተግባር ረቂቅ ነው። ፖሊሲን ይሸፍናል።
  ምዝገባ፣ ቃል ኪዳኖች፣ የህዝብ ሜታዳታ፣ የአፈጻጸም ደረሰኞች እና
  የማረጋገጫ ሁነታ.
- `BFV` የ Brakerski/Fan-Vercauteren ሆሞሞርፊክ ምስጠራ ዘዴ ነው
  አንዳንድ `ram_lfe` የተመሰጠረ ግቤትን ለመገምገም የጀርባ ደጋፊዎች።
- `ram_fhe_profile` BFV-ተኮር ሜታዳታ ነው እንጂ ለጠቅላላው ሁለተኛ ስም አይደለም
  ባህሪ. በፕሮግራም የተያዘውን የ BFV ማስፈጸሚያ ማሽን የኪስ ቦርሳ እና
  ፖሊሲ በፕሮግራም የተያዘለትን የኋላ ክፍል ሲጠቀም አረጋጋጮች ማነጣጠር አለባቸው።

በተጨባጭ ሁኔታ፡-

- `RamLfeProgramPolicy` እና `RamLfeExecutionReceipt` LFE-ንብርብር ዓይነቶች ናቸው።
- `BfvParameters`፣ `BfvCiphertext`፣ `BfvProgrammedPublicParameters`፣ እና
  `BfvRamProgramProfile` FHE-ንብርብር ዓይነቶች ናቸው።
- `HiddenRamFheProgram` እና `HiddenRamFheInstruction` የውስጥ ስሞች ናቸው
  የተደበቀው የ BFV ፕሮግራም በፕሮግራም በተያዘው የኋላ ክፍል የተተገበረ። ላይ ይቆያሉ።
  የFHE ጎን ምክንያቱም ኢንክሪፕት የተደረገውን የማስፈጸሚያ ዘዴን ይገልጻሉ።
  የውጪው ፖሊሲ ወይም ደረሰኝ ረቂቅ.

## 1.3 የመለያ መታወቂያ ከተለዋጭ ስሞች ጋር

ሁለንተናዊ-መለያ መልቀቅ የቀኖናዊ መለያ መለያ ሞዴልን አይለውጠውም፡-- `AccountId` ቀኖናዊ፣ ጎራ የለሽ መለያ ርዕሰ ጉዳይ ሆኖ ይቆያል።
- `AccountAlias` እሴቶች በዚያ ርዕሰ ጉዳይ ላይ የተለያዩ የኤስኤንኤስ ማሰሪያዎች ናቸው። ሀ
  እንደ `merchant@banka.paynet` እና ዳታስፔስ-ስር ተለዋጭ ስም ያሉ ለጎራ ብቃት ያላቸው ተለዋጭ ስሞች
  እንደ `merchant@paynet` ያሉ ሁለቱም ወደ ተመሳሳይ ቀኖናዊ `AccountId` መፍታት ይችላሉ።
- ቀኖናዊ መለያ ምዝገባ ሁል ጊዜ `Account::new(AccountId)` / ነው
  `NewAccount::new(AccountId)`; ለጎራ ብቁ ወይም ጎራ-ቁስ የለም።
  የምዝገባ መንገድ.
- የጎራ ባለቤትነት፣ ተለዋጭ ስም ፈቃዶች እና ሌሎች በጎራ-ተኮር ባህሪዎች ይኖራሉ
  በራሳቸው ግዛት እና ኤፒአይዎች ከመለያው ማንነት ይልቅ.
- የህዝብ መለያ ፍለጋ ተከፍሎ ይከተላል፡ ተለዋጭ መጠይቆች ይፋዊ ሆነው ይቆያሉ።
  ቀኖናዊ መለያ ማንነት ንጹህ `AccountId` ሆኖ ይቆያል።

ለኦፕሬተሮች፣ ኤስዲኬዎች እና ለሙከራዎች የትግበራ ህግ፡ ከቀኖናዊው ይጀምሩ
`AccountId`፣ በመቀጠል ተለዋጭ ስም ሊዝ፣ የውሂብ ቦታ/የጎራ ፈቃዶችን እና ማንኛውንም ያክሉ
የጎራ ባለቤትነት ያለው ግዛት በተናጠል። የሐሰት ተለዋጭ ስም-የተገኘ መለያ አታዋህድ
ወይም ማንኛውንም የተገናኘ-የጎራ መስክ በመለያ መዝገቦች ላይ በተለዋጭ ስም ብቻ ይጠብቁ ወይም
መንገዱ የጎራ ክፍልን ይይዛል።

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

## 2. ዩኤአይዲዎችን ማግኘት እና ማረጋገጥ

UAID ለማግኘት ሶስት የሚደገፉ መንገዶች አሉ፡-

1. **ከዓለም ግዛት ወይም ከኤስዲኬ ሞዴሎች አንብበው።** ማንኛውም `Account`/`AccountDetails`
   በTorii በኩል የተጠየቀው ክፍያ አሁን የ `uaid` መስክ ሲሞላው ተሞልቷል።
   ተሳታፊው ወደ ሁለንተናዊ መለያዎች መርጧል።
2. ** የ UAID መዝገቦችን ይጠይቁ።** Torii ያጋልጣል።
   `GET /v1/space-directory/uaids/{uaid}` ይህም የውሂብ ቦታ ማሰሪያዎችን ይመልሳል
   እና የSpace Directory አስተናጋጁ እንደቀጠለ ሜታዳታ ያሳያል (ይመልከቱ
   `docs/space-directory.md` §3 ለክፍያ ናሙናዎች)።
3. **በመወሰን ያውጡት።** አዳዲስ UAIDዎችን ከመስመር ውጭ ሲያስነሱ ሃሽ
   ቀኖናዊው ተሳታፊ ዘር ከ Blake2b-256 ጋር እና ውጤቱን ቅድመ ቅጥያ ያድርጉ
   `uaid:`. ከታች ያለው ቅንጣቢ በሰነድ የተመለከተውን ረዳት ያሳያል
   `docs/space-directory.md` §3.3፡

   ```python
   import hashlib
   seed = b"participant@example"  # canonical address/domain seed
   digest = bytearray(hashlib.blake2b(seed, digest_size=32).digest())
   digest[-1] |= 1
   print(f"uaid:{digest.hex()}")
   ```ሁልጊዜ ቃል በቃል በትንሽ ፊደላት ያከማቹ እና ከመጥለፍዎ በፊት ነጭ ቦታን መደበኛ ያድርጉት።
እንደ `iroha app space-directory manifest scaffold` እና አንድሮይድ ያሉ የCLI ረዳቶች
`UaidLiteral` ተንታኝ የአስተዳደር ግምገማዎች እንዲችሉ ተመሳሳይ የመቁረጥ ህጎችን ይተገበራሉ
ያለማስታወቂያ ስክሪፕቶች እሴቶችን ፈትሽ።

## 3. የ UAID ይዞታዎችን እና መግለጫዎችን መመርመር

በ`iroha_core::nexus::portfolio` ውስጥ የሚወስነው ፖርትፎሊዮ ሰብሳቢ
UAIDን የሚያጣቅሱትን እያንዳንዱን የንብረት/የዳታ ቦታ ጥንዶችን ይዘረጋል። ኦፕሬተሮች እና ኤስዲኬዎች
ውሂቡን በሚከተሉት ወለሎች መጠቀም ይችላል

| ወለል | አጠቃቀም |
|--------|-------|
| `GET /v1/accounts/{uaid}/portfolio` | የውሂብ ቦታ → ንብረት → ቀሪ ማጠቃለያዎችን ይመልሳል; በ `docs/source/torii/portfolio_api.md` ውስጥ ተገልጿል. |
| `GET /v1/space-directory/uaids/{uaid}` | ከUAID ጋር የተሳሰሩ የውሂብ ቦታ መታወቂያዎችን + የመለያ ቃል በቃል ይዘረዝራል። |
| `GET /v1/space-directory/uaids/{uaid}/manifests` | ለኦዲት ሙሉ የ`AssetPermissionManifest` ታሪክ ያቀርባል። |
| `iroha app space-directory bindings fetch --uaid <literal>` | የ CLI አቋራጭ የማሰሪያውን የመጨረሻ ነጥብ ያጠቃለለ እና እንደ አማራጭ JSON ወደ ዲስክ (`--json-out`) ይጽፋል። |
| `iroha app space-directory manifest fetch --uaid <literal> --json-out <path>` | ለማረጃ ጥቅሎች የሰነድ ሰነዱን JSON ጥቅል ያመጣል። |

ምሳሌ CLI ክፍለ ጊዜ (Torii URL በ`torii_api_url` በ `iroha.json` የተዋቀረ)

```bash
iroha app space-directory bindings fetch \
  --uaid uaid:86e8ee39a3908460a0f4ee257bb25f340cd5b5de72735e9adefe07d5ef4bb0df \
  --json-out artifacts/uaid86/bindings.json

iroha app space-directory manifest fetch \
  --uaid uaid:86e8ee39a3908460a0f4ee257bb25f340cd5b5de72735e9adefe07d5ef4bb0df \
  --json-out artifacts/uaid86/manifests.json
```

በግምገማ ወቅት ጥቅም ላይ ከዋለው አንጸባራቂ ሃሽ ጎን የJSON ቅጽበተ-ፎቶዎችን ያከማቹ። የ
የስፔስ ማውጫ ተመልካች በሚገለጥበት ጊዜ የ`uaid_dataspaces` ካርታውን እንደገና ይገነባል።
ያግብሩ፣ ጊዜው ያበቃል ወይም ይሽሩ፣ ስለዚህ እነዚህ ቅጽበተ-ፎቶዎች ለማረጋገጥ ፈጣኑ መንገድ ናቸው።
በተወሰነ ዘመን ውስጥ ምን ማያያዣዎች ንቁ ነበሩ ።## 4. የማተም ችሎታ ከማስረጃ ጋር ይገለጻል።

አዲስ አበል በሚለቀቅበት ጊዜ ሁሉ ከዚህ በታች ያለውን የCLI ፍሰት ይጠቀሙ። እያንዳንዱ እርምጃ መሆን አለበት
ለአስተዳደር መፈረም በተመዘገበው የማስረጃ ጥቅል ውስጥ መሬት።

1. **አንጸባራቂውን JSON** ገምግሞ ገምጋሚዎች ወሳኙን ሃሽ ከዚህ በፊት እንዲያዩ ያድርጉ
   ማስረከብ፡

   ```bash
   iroha app space-directory manifest encode \
     --json fixtures/space_directory/capability/eu_regulator_audit.manifest.json \
     --out artifacts/eu_regulator_audit.manifest.to \
     --hash-out artifacts/eu_regulator_audit.manifest.hash
   ```

2. **በአይ18NT00000002X ክፍያ (`--manifest`) ወይም በመጠቀም አበል ያትሙ**
   የJSON መግለጫ (`--manifest-json`)። የTorii/CLI ደረሰኝ ይቅዱ
   የ `PublishSpaceDirectoryManifest` መመሪያ ሃሽ፡-

   ```bash
   iroha app space-directory manifest publish \
     --manifest artifacts/eu_regulator_audit.manifest.to \
     --reason "ESMA wave 2 onboarding"
   ```

3. **የSpaceDirectoryEvent ማስረጃን ይያዙ።** ይመዝገቡ
   `SpaceDirectoryEvent::ManifestActivated` እና የክስተት ክፍያ ጭነትን ያካትቱ
   ለውጡ ሲያርፍ ኦዲተሮች ማረጋገጥ እንዲችሉ ጥቅል።

4. **የኦዲት ቅርቅብ ይፍጠሩ** ማኒፌክተሩን ከመረጃ ቦታ መገለጫው ጋር በማያያዝ እና
   ቴሌሜትሪ መንጠቆዎች;

   ```bash
   iroha app space-directory manifest audit-bundle \
     --manifest artifacts/eu_regulator_audit.manifest.to \
     --profile fixtures/space_directory/profile/cbdc_lane_profile.json \
     --out-dir artifacts/eu_regulator_audit_bundle
   ```

5. ** ማሰሪያዎችን በTorii** (`bindings fetch` እና `manifests fetch`) ያረጋግጡ እና
   እነዚያን የJSON ፋይሎች ከላይ ባለው hash + bundle በማህደር ያስቀምጡ።

የማስረጃ ማረጋገጫ ዝርዝር፡-

- [ ] አንጸባራቂ ሃሽ (`*.manifest.hash`) በለውጥ አጽዳቂ የተፈረመ።
- [ ] CLI/Torii ደረሰኝ ለህትመት ጥሪ (stdout ወይም `--json-out` artefact)።
- [] `SpaceDirectoryEvent` ክፍያ ማግበርን ያረጋግጣል።
- [ ] የኦዲት ጥቅል ማውጫ ከዳታ ቦታ መገለጫ፣ መንጠቆዎች እና አንጸባራቂ ቅጂ ጋር።
- [ ] ማያያዣዎች + አንጸባራቂ ቅጽበተ-ፎቶዎች ከ ​​Torii ድህረ ማግበር የተገኙ።ይህ ኤስዲኬ በሚሰጥበት ጊዜ በ`docs/space-directory.md` §3.2 ውስጥ ያሉትን መስፈርቶች ያንጸባርቃል
በመልቀቂያ ግምገማዎች ወቅት ለመጠቆም አንድ ገጽ ባለቤቶች።

## 5. ተቆጣጣሪ/ክልላዊ መግለጫ አብነቶች

የመስራት ችሎታ በሚገለጥበት ጊዜ የውስጠ-ግንባታ መሳሪያዎችን እንደ መነሻ ይጠቀሙ
ለተቆጣጣሪዎች ወይም የክልል ተቆጣጣሪዎች. እንዴት መፍቀድ/መከልከል እንደሚቻል ያሳያሉ
ደንቦች እና ገምጋሚዎች የሚጠብቁትን የፖሊሲ ማስታወሻዎች ያብራሩ.

| ቋሚ | ዓላማ | ዋና ዋና ዜናዎች |
|--------|--------|-----------|
| `fixtures/space_directory/capability/eu_regulator_audit.manifest.json` | ESMA/ESRB የኦዲት ምግብ። | የ `compliance.audit::{stream_reports, request_snapshot}` ተነባቢ-ብቻ አበል በችርቻሮ ዝውውሮች ላይ ከካድ-አሸናፊዎች ጋር የቁጥጥር UAID ዎች ተገብሮ ለማቆየት። |
| `fixtures/space_directory/capability/jp_regulator_supervision.manifest.json` | JFSA የክትትል መስመር። | የ `cbdc.supervision.issue_stop_order` አበል (በቀን መስኮት + `max_amount`) ያክላል እና በ`force_liquidation` ላይ ድርብ መቆጣጠሪያዎችን ለማስፈጸም በግልፅ መካድ። |

እነዚህን መገልገያዎች በሚዘጉበት ጊዜ ያዘምኑ፦

1. `uaid` እና `dataspace` መታወቂያዎች እርስዎ ከሚያነቁት ተሳታፊ እና መስመር ጋር የሚዛመዱ።
2. በአስተዳደር መርሃ ግብር ላይ በመመስረት `activation_epoch`/`expiry_epoch` መስኮቶች.
3. `notes` መስኮች ከተቆጣጣሪው የፖሊሲ ማጣቀሻዎች ጋር (MiCA article፣ JFSA)
   ክብ ወዘተ)።
4. የአበል መስኮቶች (`PerSlot`፣ `PerMinute`፣ `PerDay`) እና አማራጭ
   `max_amount` caps ስለዚህ ኤስዲኬዎች እንደ አስተናጋጁ ተመሳሳይ ገደቦችን ያስፈጽማሉ።

## 6. የስደት ማስታወሻዎች ለኤስዲኬ ተጠቃሚዎችየጎራ መለያ መታወቂያዎችን ያጣቀሱ የኤስዲኬ ውህደቶች ወደ መሰደድ አለባቸው
ከላይ የተገለጹት የ UAID ማዕከሎች። በማሻሻያዎች ጊዜ ይህንን የማረጋገጫ ዝርዝር ይጠቀሙ፡-

  የመለያ መታወቂያዎች. ለ Rust/JS/Swift/Android ይህ ማለት ወደ የቅርብ ጊዜው ማሻሻል ማለት ነው።
  የስራ ቦታ ሳጥኖች ወይም Norito ማሰሪያዎችን በማደስ ላይ።
- ** የኤፒአይ ጥሪዎች፡** በጎራ የተቀመጡ የፖርትፎሊዮ መጠይቆችን ይተኩ
  `GET /v1/accounts/{uaid}/portfolio` እና አንጸባራቂው/የማሰሪያው የመጨረሻ ነጥቦች።
  `GET /v1/accounts/{uaid}/portfolio` አማራጭ የ`asset_id` ጥያቄ ይቀበላል
  የኪስ ቦርሳዎች አንድ የንብረት ምሳሌ ብቻ ሲፈልጉ መለኪያ። የደንበኛ ረዳቶች እንደ
  እንደ `ToriiClient.getUaidPortfolio` (JS) እና አንድሮይድ
  `SpaceDirectoryClient` እነዚህን መንገዶች አስቀድሞ ጠቅልሎታል; ከመጥፎ ይመርጧቸው
  የኤችቲቲፒ ኮድ
- ** መሸጎጫ እና ቴሌሜትሪ፡** መሸጎጫ በ UAID + የውሂብ ቦታ ከጥሬ ይልቅ
  የመለያ መታወቂያዎች እና የ UAID ቃል በቃል ኦፕሬሽኖችን በማሳየት ቴሌሜትሪ ያመነጫሉ።
  ምዝግብ ማስታወሻዎችን ከ Space Directory ማስረጃ ጋር አሰልፍ።
** የስህተት አያያዝ:** አዲስ የመጨረሻ ነጥቦች ጥብቅ የ UAID የመተንተን ስህተቶችን ይመለሳሉ
  በ `docs/source/torii/portfolio_api.md` ውስጥ ተመዝግቧል; እነዚያን ኮዶች ወለል አድርገው
  ቃል በቃል ስለዚህ የድጋፍ ቡድኖች ጉዳዮችን ያለ ምንም እርምጃዎች መለየት ይችላሉ።
- ** ሙከራ: ** ከላይ የተጠቀሱትን እቃዎች (የእራስዎ የ UAID መግለጫዎች ጨምሮ) ሽቦ ያድርጉ
  የ Norito የዙር ጉዞዎችን እና አንጸባራቂ ግምገማዎችን ለማረጋገጥ ወደ ኤስዲኬ የሙከራ ስብስቦች
  ከአስተናጋጁ አተገባበር ጋር ይጣጣሙ.

## 7. ማጣቀሻዎች- `docs/space-directory.md` - ከጠለቀ የህይወት ዑደት ዝርዝር ጋር የኦፕሬተር መጫወቻ መጽሐፍ።
- `docs/source/torii/portfolio_api.md` - ለ UAID ፖርትፎሊዮ የ REST ንድፍ እና
  የመጨረሻ ነጥቦችን አንጸባራቂ።
- `crates/iroha_cli/src/space_directory.rs` - የ CLI ትግበራ በ ውስጥ ተጠቅሷል
  ይህ መመሪያ.
- `fixtures/space_directory/capability/*.manifest.json` - ተቆጣጣሪ፣ ችርቻሮ እና
  CBDC አንጸባራቂ አብነቶች ለክሎኒንግ ዝግጁ ናቸው።

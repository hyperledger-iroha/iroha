<!-- Auto-generated stub for Burmese (my) translation. Replace this content with the full translation. -->

---
lang: my
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

# Universal အကောင့်လမ်းညွှန်

ဤလမ်းညွှန်ချက်သည် UAID (Universal Account ID) ထုတ်ပေးခြင်းဆိုင်ရာ လိုအပ်ချက်များကို ခွဲထုတ်ထားသည်။
Nexus လမ်းပြမြေပုံသည် ၎င်းတို့အား အော်ပရေတာ + SDK အာရုံစူးစိုက်သည့် လမ်းညွှန်ချက်ဖြင့် ထုပ်ပိုးထားသည်။
၎င်းတွင် UAID ဆင်းသက်လာခြင်း၊ အစုစု/ဖော်ပြချက်စစ်ဆေးခြင်း၊ ထိန်းညှိခြင်းပုံစံများ ပါဝင်သည်။
နှင့် `iroha app space-directory manifest တိုင်းနှင့် တွဲရမည့် အထောက်အထားများ
Publish` run (roadmap reference: `roadmap.md:2209`)။

## 1. UAID အမြန်ကိုးကား- UAIDs များသည် `uaid:<hex>` စာလုံးများဖြစ်ပြီး `<hex>` သည် Blake2b-256 ၏အချေအတင်ဖြစ်သည်
  LSB ကို `1` ဟု သတ်မှတ်ထားသည်။ Canonical အမျိုးအစားသည် နေထိုင်သည်။
  `crates/iroha_data_model/src/nexus/manifest.rs::UniversalAccountId`။
- အကောင့်မှတ်တမ်းများ (`Account` နှင့် `AccountDetails`) ယခု စိတ်ကြိုက်ရွေးချယ်နိုင်သော `uaid` ကို ယူဆောင်သွားပါပြီ
  နယ်ပယ်တွင် အပလီကေးရှင်းများက စိတ်ကြိုက် hashing မပါဘဲ identifier ကို လေ့လာနိုင်သည်။
- Hidden-function identifier ပေါ်လစီများသည် ပုံမှန်မဟုတ်သော ပုံမှန်ထည့်သွင်းမှုများကို ချည်နှောင်နိုင်သည်။
  `opaque:` ID များသို့ (ဖုန်းနံပါတ်များ၊ အီးမေးလ်များ၊ အကောင့်နံပါတ်များ၊ ပါတနာစာကြောင်းများ)
  UAID namespace အောက်တွင်။ ကွင်းဆက်အပိုင်းများမှာ `IdentifierPolicy`၊
  `IdentifierClaimRecord` နှင့် `opaque_id -> uaid` အညွှန်း။
- Space Directory သည် UAID တစ်ခုစီနှင့်ချိတ်ဆက်ထားသည့် `World::uaid_dataspaces` မြေပုံကို ထိန်းသိမ်းထားသည်။
  တက်ကြွသောမန်နီးဖက်စ်များမှ ကိုးကားထားသော dataspace အကောင့်များသို့။ Torii သည် ၎င်းကို ပြန်လည်အသုံးပြုသည်။
  `/portfolio` နှင့် `/uaids/*` APIs အတွက်မြေပုံ။
- `POST /v1/accounts/onboard` သည် မူရင်း Space Directory မန်နီးဖက်စ်ကို ထုတ်ဝေသည်။
  ကမ္ဘာလုံးဆိုင်ရာဒေတာနေရာလွတ်မရှိသောအခါ၊ ထို့ကြောင့် UAID ကိုချက်ချင်းချည်နှောင်ထားသည်။
  စတင်လည်ပတ်မည့် အာဏာပိုင်များသည် `CanPublishSpaceDirectoryManifest{dataspace=0}` ကို ကိုင်ဆောင်ထားရပါမည်။
- SDK များအားလုံးသည် UAID စာသားများကို canonicalising အတွက် အထောက်အကူများကို ဖော်ထုတ်ပြသသည် (ဥပမာ၊
  Android SDK တွင် `UaidLiteral`)။ အကူအညီပေးသူများသည် အကြမ်း 64-hex digests ကိုလက်ခံသည်။
  (LSB=1) သို့မဟုတ် `uaid:<hex>` စာသားများနှင့် တူညီသော Norito ကုဒ်ဒစ်များကို ပြန်လည်အသုံးပြုခြင်း
  digest သည် ဘာသာစကားများကို ဖြတ်ကျော်၍မရပါ။

## 1.1 လျှို့ဝှက်သတ်မှတ်မှုမူဝါဒများ

ယခုအခါ UAIDs များသည် ဒုတိယအထောက်အထားအလွှာအတွက် ကျောက်ဆူးဖြစ်လာသည်-- ကမ္ဘာလုံးဆိုင်ရာ `IdentifierPolicyId` (`<kind>#<business_rule>`) မှ သတ်မှတ်သည်
  namespace၊ အများသူငှာ ကတိကဝတ် မက်တာဒေတာ၊ ဖြေရှင်းသူ အတည်ပြုခြင်းကီး နှင့်
  canonical input normalization mode (`Exact`၊ `LowercaseTrimmed`၊
  `PhoneE164`၊ `EmailAddress` သို့မဟုတ် `AccountNumber`)။
- အရေးဆိုမှုတစ်ခုသည် `opaque:` မှဆင်းသက်လာသော Identifier တစ်ခုကို UAID တစ်ခုနှင့် တစ်ခုနှင့် အတိအကျ ပေါင်းစပ်ထားသည်။
  ထိုမူဝါဒအောက်တွင် canonical `AccountId`၊ သို့သော် ကွင်းဆက်ကသာ လက်ခံသည်။
  `IdentifierResolutionReceipt` ဖြင့် လက်မှတ် ရေးထိုးထားသည့်အခါ အရေးဆိုမှု။
- Resolution သည် `resolve -> transfer` စီးဆင်းနေပါသည်။ Torii သည် အလင်းပိတ်ခြင်းကို ဖြေရှင်းပေးသည်။
  ကိုင်တွယ်ပြီး canonical `AccountId` ကို ပြန်ပေးသည်။ လွှဲပြောင်းမှုများကို ပစ်မှတ်ထားဆဲဖြစ်သည်။
  `uaid:` သို့မဟုတ် `opaque:` တိုက်ရိုက်မဟုတ်သော canonical အကောင့်။
- မူဝါဒများသည် ယခု BFV ထည့်သွင်း-ကုဒ်ဝှက်ခြင်းဆိုင်ရာ ကန့်သတ်ဘောင်များကို ထုတ်ဝေနိုင်ပါပြီ။
  `PolicyCommitment.public_parameters`။ လက်ရှိအချိန်တွင် Torii သည် ၎င်းတို့ကို ကြော်ငြာထားသည်။
  `GET /v1/identifier-policies` နှင့် သုံးစွဲသူများသည် BFV-ထုပ်ပိုးထားသော ထည့်သွင်းမှုကို တင်သွင်းနိုင်သည်
  plaintext အစား ပရိုဂရမ်ရေးဆွဲထားသော မူဝါဒများသည် BFV ဘောင်များကို a ဖြင့် ခြုံထားသည်။
  ထုတ်ဝေသော canonical `BfvProgrammedPublicParameters` အတွဲ
  အများသူငှာ `ram_fhe_profile`; အမွေအနှစ်ကုန်ကြမ်း BFV ပေးဆောင်မှုများကို အဆင့်မြှင့်တင်ထားသည်။
  ကတိကဝတ်ကို ပြန်လည်တည်ဆောက်သည့်အခါ canonical အတွဲ။
- identifier လမ်းကြောင်းများသည် တူညီသော Torii access-token နှင့် rate-limit ကိုဖြတ်သန်းသွားသည်
  အခြားအက်ပ်-မျက်နှာစာ အဆုံးမှတ်များအဖြစ် စစ်ဆေးသည်။ သူတို့သည် သာမန်အားဖြင့် ရှောင်ကွင်းခြင်း မဟုတ်ပါ။
  API မူဝါဒ။

## 1.2 ဝေါဟာရများ

အမည်ခွဲခြင်းမှာ ရည်ရွယ်ချက်ရှိရှိ၊- `ram_lfe` သည် အပြင်ဘက် လျှို့ဝှက်လုပ်ဆောင်မှု ဖြစ်သော abstraction ဖြစ်သည်။ ၎င်းသည် မူဝါဒကို အကျုံးဝင်သည်။
  မှတ်ပုံတင်ခြင်း၊ ကတိကဝတ်များ၊ အများသူငှာ မက်တာဒေတာ၊ ကတိကဝတ်ပြေစာများနှင့်
  အတည်ပြုမုဒ်။
- `BFV` သည် Brakerski/Fan-Vercauteren homomorphic encryption scheme မှအသုံးပြုသည်
  ကုဒ်ဝှက်ထားသော ထည့်သွင်းမှုကို အကဲဖြတ်ရန် `ram_lfe` နောက်ခံအချို့။
- `ram_fhe_profile` သည် BFV သီးသန့် မက်တာဒေတာဖြစ်ပြီး တစ်ခုလုံးအတွက် ဒုတိယအမည်မဟုတ်ပါ။
  ထူးခြားချက်။ ၎င်းသည် ပိုက်ဆံအိတ်များနှင့် ပရိုဂရမ်လုပ်ထားသော BFV ကွပ်မျက်ရေးစက်ကို ဖော်ပြသည်။
  မူဝါဒတစ်ခုက ပရိုဂရမ်လုပ်ထားသော နောက်ကွယ်ကို အသုံးပြုသည့်အခါ အတည်ပြုသူများသည် ပစ်မှတ်ထားရမည်။

တိကျသောအသုံးအနှုန်းများတွင်-

- `RamLfeProgramPolicy` နှင့် `RamLfeExecutionReceipt` များသည် LFE-အလွှာ အမျိုးအစားများဖြစ်သည်။
- `BfvParameters`၊ `BfvCiphertext`၊ `BfvProgrammedPublicParameters` နှင့်
  `BfvRamProgramProfile` သည် FHE-အလွှာအမျိုးအစားများဖြစ်သည်။
- `HiddenRamFheProgram` နှင့် `HiddenRamFheInstruction` တို့သည် အတွင်းပိုင်းအမည်များဖြစ်သည်
  ပရိုဂရမ်လုပ်ထားသော နောက်ကွယ်မှ လုပ်ဆောင်သည့် လျှို့ဝှက် BFV ပရိုဂရမ်။ ပေါ်မှာပဲ နေကြ တယ်။
  အဘယ်ကြောင့်ဆိုသော် FHE ဘက်မှ ၎င်းတို့သည် ကုဒ်ဝှက်ထားသော လုပ်ဆောင်မှု ယန္တရားကို ဖော်ပြမည့်အစား၊
  ပြင်ပမူဝါဒ သို့မဟုတ် လက်ခံပိုင်းခြားခြင်း

## 1.3 အကောင့်အထောက်အထားနှင့် နာမည်တူများ

Universal-account စတင်ဖြန့်ချိခြင်းသည် canonical အကောင့်အထောက်အထားပုံစံကို ပြောင်းလဲခြင်းမရှိပါ-- `AccountId` သည် canonical၊ domainless အကောင့်ဘာသာရပ်အဖြစ် ကျန်ရှိနေပါသည်။
- `AccountAlias` တန်ဖိုးများသည် ထိုအကြောင်းအရာ၏ထိပ်တွင် သီးခြား SNS နှောင်ကြိုးများဖြစ်သည်။ တစ်
  `merchant@banka.paynet` နှင့် dataspace-root alias ကဲ့သို့သော ဒိုမိန်းအရည်အချင်းပြည့်မီသော alias
  `merchant@paynet` ကဲ့သို့သော တူညီသော canonical `AccountId` နှစ်ခုလုံးကို ဖြေရှင်းနိုင်သည်။
- Canonical အကောင့်မှတ်ပုံတင်ခြင်းသည် အမြဲတမ်း `Account::new(AccountId)` /
  `NewAccount::new(AccountId)`; domain-qualified သို့မဟုတ် domain-materialized မရှိပါ။
  မှတ်ပုံတင်လမ်းကြောင်း။
- ဒိုမိန်းပိုင်ဆိုင်မှု၊ နာမည်တူခွင့်ပြုချက်များနှင့် အခြားဒိုမိန်းနယ်ပယ်အလိုက် အပြုအမူများ တိုက်ရိုက်ထုတ်လွှင့်သည်။
  အကောင့်ဝိသေသလက္ခဏာကိုယ်တိုင်အပေါ်ထက် ၎င်းတို့၏ကိုယ်ပိုင်ပြည်နယ်နှင့် API များတွင်။
- အများသူငှာ အကောင့်ရှာဖွေမှုတွင် အောက်ပါအတိုင်း ခွဲထားသည်- အမည်တူ စုံစမ်းမှုများသည် အများသူငှာ ရှိနေစဉ်၊
  Canonical အကောင့်အထောက်အထားသည် `AccountId` အစစ်အမှန်ဖြစ်နေဆဲဖြစ်သည်။

အော်ပရေတာများ၊ SDK နှင့် စမ်းသပ်မှုများအတွက် အကောင်အထည်ဖော်မှုစည်းမျဉ်း- canonical မှ စတင်သည်။
`AccountId`၊ ထို့နောက် နာမည်တူငှားရမ်းမှုများ၊ ဒေတာနေရာ/ဒိုမိန်းခွင့်ပြုချက်များနှင့် မည်သည့်အရာကိုမဆို ထည့်ပါ
ဒိုမိန်းပိုင်ပြည်နယ် သီးခြားစီ။ နာမည်အတုမှရရှိသော အကောင့်အတုကို မပေါင်းစပ်ပါနှင့်
သို့မဟုတ် အကောင့် မှတ်တမ်းများပေါ်တွင် ချိတ်ဆက်ထားသော ဒိုမိန်းအကွက်တစ်ခုခုကို မျှော်လင့်ခြင်း သို့မဟုတ် နာမည်တူတစ်ခုကြောင့် ဖြစ်သည်။
လမ်းကြောင်းသည် ဒိုမိန်းအပိုင်းကို သယ်ဆောင်သည်။

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

## 2. UAIDs များရယူခြင်းနှင့် အတည်ပြုခြင်း။

UAID ရရှိရန် ပံ့ပိုးပေးထားသော နည်းလမ်းသုံးမျိုးရှိပါသည်။

1. **ကမ္ဘာ့အခြေအနေ သို့မဟုတ် SDK မော်ဒယ်များမှ ဖတ်ပါ။** မည်သည့် `Account`/`AccountDetails`
   Torii မှတစ်ဆင့် မေးမြန်းထားသော payload သည် ယခုအခါ `uaid` အကွက်ကို ဖြည့်သွင်းသောအခါ၊
   ပါဝင်သူသည် universal အကောင့်များသို့ ရွေးချယ်ခဲ့သည်။
2. ** UAID မှတ်ပုံတင်မှုများကို မေးမြန်းပါ။** Torii မှ ဖော်ထုတ်ပေးသည်
   `GET /v1/space-directory/uaids/{uaid}` သည် dataspace bindings ကို ပြန်ပေးသည်။
   နှင့် Space Directory လက်ခံဆောင်ရွက်ပေးသည့် မက်တာဒေတာကို ထင်ရှားစွာပြသပါ (ကြည့်ရှုပါ။
   payload နမူနာများအတွက် `docs/space-directory.md` §3)။
3. **၎င်းကို တိကျသေချာစွာ ရယူပါ။** UAID အသစ်များကို အော့ဖ်လိုင်းတွင် စတင်ဖွင့်သောအခါ hash၊
   Blake2b-256 ဖြင့် canonical ပါဝင်သူအမျိုးအနွယ်ကို ရလဒ်နှင့်အတူ ရှေ့ဆက်ပါ။
   `uaid:`။ အောက်ဖော်ပြပါ အတိုအထွာသည် မှတ်တမ်းပြုစုထားသည့် အကူအညီကို ရောင်ပြန်ဟပ်သည်။
   `docs/space-directory.md` §3.3-

   ```python
   import hashlib
   seed = b"participant@example"  # canonical address/domain seed
   digest = bytearray(hashlib.blake2b(seed, digest_size=32).digest())
   digest[-1] |= 1
   print(f"uaid:{digest.hex()}")
   ```စာလုံးအသေးဖြင့် အမြဲသိမ်းဆည်းပြီး ဟက်ကင်းမပြုလုပ်မီ နေရာလွတ်များကို ပုံမှန်ဖြစ်အောင် ပြုလုပ်ပါ။
`iroha app space-directory manifest scaffold` နှင့် Android ကဲ့သို့သော CLI အထောက်အကူများ
`UaidLiteral` parser သည် တူညီသော ဖြတ်တောက်ခြင်း စည်းမျဉ်းများကို အသုံးပြု၍ အုပ်ချုပ်ရေးဆိုင်ရာ ပြန်လည်သုံးသပ်မှုများ ပြုလုပ်နိုင်သည်
ad hoc script များမပါဘဲ အပြန်အလှန်စစ်ဆေးသည့်တန်ဖိုးများ။

## 3. UAID ပိုင်ဆိုင်မှုများနှင့် ဖော်ပြချက်များကို စစ်ဆေးခြင်း။

`iroha_core::nexus::portfolio` ရှိ အဆုံးအဖြတ်ပေးသောအစုစုစုစည်းမှု
UAID ကို ရည်ညွှန်းသော ပိုင်ဆိုင်မှု/ဒေတာနေရာ အတွဲတိုင်းကို ဖော်ပြသည်။ အော်ပရေတာများနှင့် SDK များ
အောက်ပါ မျက်နှာပြင်များမှတဆင့် ဒေတာကို စားသုံးနိုင်သည်-

| မျက်နှာပြင် | အသုံးပြုပုံ |
|---------|-------|
| `GET /v1/accounts/{uaid}/portfolio` | dataspace → ပိုင်ဆိုင်မှု → လက်ကျန်အနှစ်ချုပ်များကို ပြန်ပေးသည် ။ `docs/source/torii/portfolio_api.md` တွင်ဖော်ပြထားသည်။ |
| `GET /v1/space-directory/uaids/{uaid}` | UAID နှင့် ချိတ်ဆက်ထားသော dataspace IDs + အကောင့် literals များကို စာရင်းပြုစုပါ။ |
| `GET /v1/space-directory/uaids/{uaid}/manifests` | စာရင်းစစ်များအတွက် `AssetPermissionManifest` မှတ်တမ်းအပြည့်အစုံကို ပံ့ပိုးပေးပါသည်။ |
| `iroha app space-directory bindings fetch --uaid <literal>` | binding endpoint ကို ခြုံပြီး JSON ကို disk (`--json-out`) သို့ စိတ်ကြိုက်ရွေးချယ်ရေးပေးသည့် CLI ဖြတ်လမ်း။ |
| `iroha app space-directory manifest fetch --uaid <literal> --json-out <path>` | အထောက်အထားထုပ်များအတွက် manifest JSON အစုအဝေးကို ရယူပါ။ |

ဥပမာ CLI စက်ရှင် (Torii URL ကို `torii_api_url` မှတဆင့် ပြင်ဆင်သတ်မှတ်ထားသော `iroha.json`)

```bash
iroha app space-directory bindings fetch \
  --uaid uaid:86e8ee39a3908460a0f4ee257bb25f340cd5b5de72735e9adefe07d5ef4bb0df \
  --json-out artifacts/uaid86/bindings.json

iroha app space-directory manifest fetch \
  --uaid uaid:86e8ee39a3908460a0f4ee257bb25f340cd5b5de72735e9adefe07d5ef4bb0df \
  --json-out artifacts/uaid86/manifests.json
```

သုံးသပ်မှုများအတွင်း အသုံးပြုသည့် manifest hash နှင့်အတူ JSON လျှပ်တစ်ပြက်ရိုက်ချက်များကို သိမ်းဆည်းပါ။ အဆိုပါ
အာကာသလမ်းညွှန်ကြည့်ရှုသူသည် `uaid_dataspaces` မြေပုံကို ထင်ရှားသည့်အခါတိုင်း ပြန်လည်တည်ဆောက်သည်
အသက်သွင်းရန်၊ သက်တမ်းကုန်ဆုံးရန် သို့မဟုတ် ပြန်လည်ရုပ်သိမ်းရန်၊ ထို့ကြောင့် ဤလျှပ်တစ်ပြက်ရိုက်ချက်များသည် သက်သေပြရန် အမြန်ဆုံးနည်းလမ်းဖြစ်သည်။
သတ်မှတ်ထားသောခေတ်တွင် မည်သို့သော ချည်နှောင်မှုများ လှုပ်ရှားခဲ့သနည်း။## 4. ထုတ်ဝေခြင်းစွမ်းရည်သည် အထောက်အထားများဖြင့် ထင်ရှားသည်။

ထောက်ပံ့ကြေးအသစ်တစ်ခုထွက်တိုင်း အောက်ပါ CLI စီးဆင်းမှုကို အသုံးပြုပါ။ အဆင့်တိုင်းရှိရမယ်။
အုပ်ချုပ်မှုလက်မှတ်ထိုးရန် မှတ်တမ်းတင်ထားသော အထောက်အထားအစုအဝေးတွင် မြေ။

1. ** manifest JSON ကို ကုဒ်လုပ်ပါ** သို့မှသာ သုံးသပ်သူများသည် အဆုံးအဖြတ်ပေးသော hash ကို မတွေ့မီတွင် မြင်နိုင်သည်။
   တင်ပြချက်-

   ```bash
   iroha app space-directory manifest encode \
     --json fixtures/space_directory/capability/eu_regulator_audit.manifest.json \
     --out artifacts/eu_regulator_audit.manifest.to \
     --hash-out artifacts/eu_regulator_audit.manifest.hash
   ```

2. **Norito payload (`--manifest`) ကို အသုံးပြု၍ **ထောက်ပံ့ကြေးကို ထုတ်ပြန်ခြင်း** သို့မဟုတ်
   JSON ဖော်ပြချက် (`--manifest-json`)။ Torii/CLI ပြေစာပေါင်းကို မှတ်တမ်းတင်ပါ။
   `PublishSpaceDirectoryManifest` ညွှန်ကြားချက် hash-

   ```bash
   iroha app space-directory manifest publish \
     --manifest artifacts/eu_regulator_audit.manifest.to \
     --reason "ESMA wave 2 onboarding"
   ```

3. **Capture SpaceDirectoryEvent အထောက်အထားများ။** စာရင်းသွင်းပါ။
   `SpaceDirectoryEvent::ManifestActivated` နှင့် event payload ကို ထည့်သွင်းပါ။
   အစုအစည်းသည် အပြောင်းအလဲဖြစ်သည့်အခါ စာရင်းစစ်များက အတည်ပြုနိုင်သည်။

4. **စာရင်းစစ်အစုအဝေးတစ်ခုကို ထုတ်ပေးပါ** မန်နီးဖက်စ်ကို ၎င်း၏ဒေတာနေရာပရိုဖိုင်တွင် ချိတ်ပြီး
   တယ်လီမီတာချိတ်များ

   ```bash
   iroha app space-directory manifest audit-bundle \
     --manifest artifacts/eu_regulator_audit.manifest.to \
     --profile fixtures/space_directory/profile/cbdc_lane_profile.json \
     --out-dir artifacts/eu_regulator_audit_bundle
   ```

5. ** Torii** (`bindings fetch` နှင့် `manifests fetch`) မှတစ်ဆင့် စည်းနှောင်မှုကို စစ်ဆေးအတည်ပြုပါ
   အထက်ပါ hash + အတွဲဖြင့် ထို JSON ဖိုင်များကို သိမ်းဆည်းပါ။

သက်သေစာရင်း-

- [ ] Manifest hash (`*.manifest.hash`) သည် ပြောင်းလဲမှုကို အတည်ပြုသူမှ လက်မှတ်ထိုးထားသည်။
- [ ] CLI/Torii ထုတ်ဝေခေါ်ဆိုမှုအတွက် ပြေစာ (stdout သို့မဟုတ် `--json-out` artefact)။
- [ ] `SpaceDirectoryEvent` payload ကို အသက်သွင်းကြောင်း သက်သေပြခြင်း။
- [ ] dataspace ပရိုဖိုင်၊ ချိတ်များနှင့် မန်နီးဖက်စ်မိတ္တူတို့ဖြင့် အတွဲလိုက်လမ်းညွှန်ကို စစ်ဆေးပါ။
- [ ] ချိတ်ဆွဲခြင်း + Torii ကို အသက်သွင်းပြီးနောက်မှ ရယူခဲ့သည့် တွဲဆိုင်းများ + မန်နီးဖက်စ် လျှပ်တစ်ပြက်ပုံများ။SDK ပေးနေစဉ် ၎င်းသည် `docs/space-directory.md` §3.2 ရှိ လိုအပ်ချက်များကို ထင်ဟပ်စေသည်
ထုတ်ဝေမှုသုံးသပ်ချက်များအတွင်း ညွှန်ပြရန် စာမျက်နှာတစ်ခုတည်းကို ပိုင်ဆိုင်သည်။

## 5. Regulator/regional manifest နမူနာများ

ဖန်တီးမှုစွမ်းရည်ကို ထင်ရှားပေါ်လွင်လာသောအခါတွင် in-repo fixtures များကို စမှတ်များအဖြစ် အသုံးပြုပါ။
စည်းကမ်းထိန်းသိမ်းရေးအဖွဲ့ သို့မဟုတ် ဒေသဆိုင်ရာ ကြီးကြပ်ရေးမှူးများအတွက်။ ခွင့်ပြု/ငြင်းဆိုပုံ အတိုင်းအတာကို သရုပ်ပြကြသည်။
စည်းမျဥ်းစည်းကမ်းများနှင့် မူဝါဒမှတ်စုများကို ဝေဖန်သုံးသပ်သူများ မျှော်မှန်းရှင်းပြပါ။

| ခံစစ်မှူး | ရည်ရွယ်ချက် | ပေါ်လွင် |
|---------|---------|------------|
| `fixtures/space_directory/capability/eu_regulator_audit.manifest.json` | ESMA/ESRB စာရင်းစစ်ဖိဒ်။ | စည်းကမ်းထိန်းသိမ်းရေး UAIDs များမတည်မြဲစေရန် လက်လီလွှဲပြောင်းမှုများတွင် ငြင်းဆို-အနိုင်များဖြင့် `compliance.audit::{stream_reports, request_snapshot}` အတွက် ဖတ်ရန်သီးသန့် စရိတ်များ။ |
| `fixtures/space_directory/capability/jp_regulator_supervision.manifest.json` | JFSA ကြီးကြပ်ရေးလမ်းသွား။ | ကန့်သတ်ထားသော `cbdc.supervision.issue_stop_order` ထောက်ပံ့ကြေး (PerDay Window + `max_amount`) နှင့် `force_liquidation` တွင် ထိန်းချုပ်မှုနှစ်ခုကို ကျင့်သုံးရန် အတိအလင်း ငြင်းဆိုချက်တစ်ခု ပေါင်းထည့်သည်။ |

ဤပစ္စည်းများကို ပုံတူပွားသည့်အခါ၊ အပ်ဒိတ်လုပ်ပါ။

1. သင်ဖွင့်ထားသည့် ပါဝင်သူနှင့် လမ်းသွားနှင့် ကိုက်ညီရန် `uaid` နှင့် `dataspace` အိုင်ဒီများ။
2. `activation_epoch`/`expiry_epoch` အုပ်ချုပ်မှုအချိန်ဇယားကိုအခြေခံ၍ ပြတင်းပေါက်များ။
3. `notes` သည် စည်းကမ်းထိန်းသိမ်းရေးအဖွဲ့၏ မူဝါဒအကိုးအကားများပါရှိသည် (MiCA ဆောင်းပါး၊ JFSA
   စက်ဝိုင်း စသဖြင့်)။
4. Allowance windows (`PerSlot`၊ `PerMinute`၊ `PerDay`) နှင့် ရွေးချယ်နိုင်သည်
   `max_amount` ထုပ်များသည် SDK များသည် host ကဲ့သို့ တူညီသောကန့်သတ်ချက်များကို တွန်းအားပေးသည်။

## 6. SDK သုံးစွဲသူများအတွက် ရွှေ့ပြောင်းခြင်းမှတ်စုများဒိုမိန်းတစ်ခုစီကို ရည်ညွှန်းထားသော အကောင့် ID များဆီသို့ ပြောင်းရွှေ့ရမည်ဟု ရည်ညွှန်းထားသော လက်ရှိ SDK ပေါင်းစပ်မှုများ
အထက်တွင်ဖော်ပြထားသော UAID ဗဟိုပြုမျက်နှာပြင်များ။ အဆင့်မြှင့်တင်မှုများ ပြုလုပ်နေစဉ် ဤစစ်ဆေးမှုစာရင်းကို အသုံးပြုပါ-

  အကောင့် IDs Rust/JS/Swift/Android အတွက် ၎င်းသည် နောက်ဆုံးပေါ် အဆင့်မြှင့်တင်ခြင်းကို ဆိုလိုသည်။
  အလုပ်ခွင်သေတ္တာများ သို့မဟုတ် Norito နှောင်ကြိုးများကို ပြန်လည်ထုတ်ပေးခြင်း။
- **API ခေါ်ဆိုမှုများ-** domain-scoped portfolio queries ဖြင့် အစားထိုးပါ။
  `GET /v1/accounts/{uaid}/portfolio` နှင့် manifest/binding အဆုံးမှတ်များ။
  `GET /v1/accounts/{uaid}/portfolio` သည် ရွေးချယ်နိုင်သော `asset_id` မေးမြန်းချက်ကို လက်ခံသည်
  ပိုက်ဆံအိတ်များသည် ပိုင်ဆိုင်မှု သာဓကတစ်ခုသာ လိုအပ်သောအခါ ကန့်သတ်ချက်များ။ Client Helpers ဆိုတာမျိုး
  `ToriiClient.getUaidPortfolio` (JS) နှင့် Android အဖြစ်
  `SpaceDirectoryClient` သည် ဤလမ်းကြောင်းများကို ခြုံပြီးဖြစ်သည်။ စိတ်ကြိုက်ထက် သူတို့ကို ပိုကြိုက်တယ်။
  HTTP ကုဒ်။
- **Caching & telemetry-** အကြမ်းအစား UAID + dataspace မှ Cache entries များကို
  အကောင့် ids များ နှင့် UAID ပကတိအတိုင်း ပြသသော တယ်လီမီတာကို ထုတ်လွှတ်ပြီး လုပ်ငန်းများ လုပ်ဆောင်နိုင်သည်။
  Space Directory အထောက်အထားများဖြင့် မှတ်တမ်းများကို တန်းစီပါ။
- ** ကိုင်တွယ်မှုအမှား-** အဆုံးမှတ်အသစ်များသည် တင်းကျပ်သော UAID ခွဲခြမ်းစိတ်ဖြာမှုအမှားများကို ပြန်ပေးသည်။
  `docs/source/torii/portfolio_api.md` တွင် မှတ်တမ်းတင်ထားသည်။ ထိုကုဒ်များကို ဖော်ပြပါ။
  စကားအပြောအဆိုကြောင့် ပံ့ပိုးကူညီရေးအဖွဲ့များသည် အဆိုအဆင့်များမပါဘဲ ပြဿနာများကို စမ်းသုံးနိုင်သည်။
- **စမ်းသပ်ခြင်း-** အထက်ဖော်ပြပါ ကိရိယာများကို ကြိုးသွယ်တန်းပါ (အပြင် သင့်ကိုယ်ပိုင် UAID သရုပ်ဖော်ချက်များ)
  Norito အသွားအပြန်နှင့် ထင်ရှားသော အကဲဖြတ်ချက်များကို သက်သေပြရန် SDK စမ်းသပ်မှုအစုံသို့
  အိမ်ရှင်အကောင်အထည်ဖော်မှုနှင့် ကိုက်ညီသည်။

## 7. ကိုးကား- `docs/space-directory.md` — ပိုမိုလေးနက်သော ဘဝသံသရာအသေးစိတ်ပါရှိသော အော်ပရေတာဖွင့်စာအုပ်။
- `docs/source/torii/portfolio_api.md` — UAID အစုစုအတွက် REST schema နှင့်
  အဆုံးမှတ်များကိုဖော်ပြသည်။
- `crates/iroha_cli/src/space_directory.rs` — CLI အကောင်အထည်ဖော်မှုကို ရည်ညွှန်းထားသည်။
  ဤလမ်းညွှန်။
- `fixtures/space_directory/capability/*.manifest.json` — ထိန်းညှိခြင်း၊ လက်လီရောင်းချခြင်းနှင့်
  CBDC သရုပ်ဖော်ပုံများ ပုံတူပွားခြင်းအတွက် အဆင်သင့်ဖြစ်နေပါပြီ။

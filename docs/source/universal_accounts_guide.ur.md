<!-- Auto-generated stub for Urdu (ur) translation. Replace this content with the full translation. -->

---
lang: ur
direction: rtl
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

# یونیورسل اکاؤنٹ گائیڈ

یہ گائیڈ UAID (یونیورسل اکاؤنٹ ID) سے رول آؤٹ کی ضروریات کو دور کرتا ہے۔
Nexus روڈ میپ اور انہیں آپریٹر + SDK فوکسڈ واک تھرو میں پیک کرتا ہے۔
اس میں UAID اخذ، پورٹ فولیو/مینی فیسٹ معائنہ، ریگولیٹر ٹیمپلیٹس،
اور ثبوت جو ہر `iroha ایپ اسپیس ڈائرکٹری مینی فیسٹ کے ساتھ ہونا چاہیے۔
publish` run (roadmap reference: `roadmap.md:2209`)۔

## 1. UAID فوری حوالہ- UAIDs `uaid:<hex>` لٹریلز ہیں جہاں `<hex>` ایک Blake2b-256 ڈائجسٹ ہے جس کا
  LSB `1` پر سیٹ ہے۔ کینونیکل قسم میں رہتا ہے۔
  `crates/iroha_data_model/src/nexus/manifest.rs::UniversalAccountId`۔
- اکاؤنٹ کے ریکارڈ (`Account` اور `AccountDetails`) اب اختیاری `uaid` رکھتے ہیں
  فیلڈ تاکہ ایپلی کیشنز بغیر ہیشنگ کے شناخت کنندہ کو سیکھ سکیں۔
- پوشیدہ فنکشن شناخت کنندہ پالیسیاں صوابدیدی نارملائزڈ ان پٹس کو پابند کرسکتی ہیں۔
  (فون نمبرز، ای میلز، اکاؤنٹ نمبرز، پارٹنر سٹرنگز) سے `opaque:` IDs
  UAID نام کی جگہ کے تحت۔ آن چین کے ٹکڑے `IdentifierPolicy` ہیں،
  `IdentifierClaimRecord`، اور `opaque_id -> uaid` انڈیکس۔
- اسپیس ڈائرکٹری ایک `World::uaid_dataspaces` نقشہ برقرار رکھتی ہے جو ہر UAID کو جوڑتی ہے۔
  فعال مینی فیسٹس کے ذریعے حوالہ کردہ ڈیٹا اسپیس اکاؤنٹس میں۔ Torii اسے دوبارہ استعمال کرتا ہے۔
  `/portfolio` اور `/uaids/*` APIs کے لیے نقشہ۔
- `POST /v1/accounts/onboard` اس کے لیے پہلے سے طے شدہ اسپیس ڈائرکٹری مینی فیسٹ شائع کرتا ہے
  عالمی ڈیٹا اسپیس جب کوئی موجود نہ ہو، تو UAID فوری طور پر پابند ہو جاتا ہے۔
  آن بورڈنگ حکام کو `CanPublishSpaceDirectoryManifest{dataspace=0}` رکھنا ضروری ہے۔
- تمام SDKs UAID لٹریلز (جیسے،
  Android SDK میں `UaidLiteral`)۔ مددگار خام 64-ہیکس ہضم قبول کرتے ہیں۔
  (LSB=1) یا `uaid:<hex>` لٹریلز اور وہی Norito کوڈیکس دوبارہ استعمال کریں تاکہ
  ڈائجسٹ زبانوں میں نہیں بڑھ سکتا۔

## 1.1 پوشیدہ شناخت کنندہ پالیسیاں

UAIDs اب دوسری شناختی پرت کے لیے اینکر ہیں:- ایک عالمی `IdentifierPolicyId` (`<kind>#<business_rule>`) کی وضاحت کرتا ہے
  نام کی جگہ، عوامی عزم کا میٹا ڈیٹا، حل کنندہ کی تصدیق کی کلید، اور
  کیننیکل ان پٹ نارملائزیشن موڈ (`Exact`, `LowercaseTrimmed`،
  `PhoneE164`، `EmailAddress`، یا `AccountNumber`)۔
- ایک دعوی ایک اخذ کردہ `opaque:` شناخت کنندہ کو بالکل ایک UAID اور ایک سے منسلک کرتا ہے۔
  اس پالیسی کے تحت کیننیکل `AccountId`، لیکن سلسلہ صرف قبول کرتا ہے
  دعوی کریں جب اس کے ساتھ دستخط شدہ `IdentifierResolutionReceipt` ہو۔
- ریزولوشن `resolve -> transfer` بہاؤ رہتا ہے۔ Torii مبہم کو حل کرتا ہے۔
  کینونیکل `AccountId` کو ہینڈل کرتا ہے اور واپس کرتا ہے۔ منتقلی اب بھی ہدف ہے
  کیننیکل اکاؤنٹ، براہ راست `uaid:` یا `opaque:` لٹریلز نہیں۔
- پالیسیاں اب BFV ان پٹ انکرپشن پیرامیٹرز کے ذریعے شائع کر سکتی ہیں۔
  `PolicyCommitment.public_parameters`۔ موجود ہونے پر، Torii ان کی تشہیر کرتا ہے۔
  `GET /v1/identifier-policies`، اور کلائنٹ BFV لپیٹے ہوئے ان پٹ جمع کر سکتے ہیں
  سادہ متن کے بجائے. پروگرام شدہ پالیسیاں BFV پیرامیٹرز کو a میں لپیٹتی ہیں۔
  canonical `BfvProgrammedPublicParameters` بنڈل جو بھی شائع کرتا ہے۔
  عوامی `ram_fhe_profile`؛ لیگیسی خام BFV پے لوڈز کو اس پر اپ گریڈ کیا گیا ہے۔
  کیننیکل بنڈل جب عزم کو دوبارہ بنایا جاتا ہے۔
- شناخت کنندہ کے راستے اسی Torii رسائی ٹوکن اور شرح کی حد سے گزرتے ہیں
  دوسرے ایپ کا سامنا کرنے والے اختتامی پوائنٹس کی طرح چیک کرتا ہے۔ وہ عام کے ارد گرد ایک بائی پاس نہیں ہیں
  API پالیسی۔

## 1.2 اصطلاحات

نام کی تقسیم جان بوجھ کر کی گئی ہے:- `ram_lfe` بیرونی پوشیدہ فنکشن خلاصہ ہے۔ یہ پالیسی کا احاطہ کرتا ہے۔
  رجسٹریشن، وعدے، عوامی میٹا ڈیٹا، عملدرآمد کی رسیدیں، اور
  تصدیقی موڈ
- `BFV` Brakerski/Fan-Vercauteren homomorphic encryption سکیم ہے
  خفیہ کردہ ان پٹ کا اندازہ کرنے کے لیے کچھ `ram_lfe` بیک اینڈ۔
- `ram_fhe_profile` BFV مخصوص میٹا ڈیٹا ہے، پورے کا دوسرا نام نہیں
  خصوصیت یہ پروگرام شدہ BFV عملدرآمد مشین کی وضاحت کرتا ہے جو بٹوے اور
  جب پالیسی پروگرام شدہ بیک اینڈ کا استعمال کرتی ہے تو تصدیق کنندگان کو ہدف بنانا چاہیے۔

ٹھوس الفاظ میں:

- `RamLfeProgramPolicy` اور `RamLfeExecutionReceipt` LFE-پرت کی اقسام ہیں۔
- `BfvParameters`, `BfvCiphertext`, `BfvProgrammedPublicParameters`، اور
  `BfvRamProgramProfile` FHE-پرت کی اقسام ہیں۔
- `HiddenRamFheProgram` اور `HiddenRamFheInstruction` کے اندرونی نام ہیں
  پوشیدہ BFV پروگرام جو پروگرام شدہ بیک اینڈ کے ذریعہ انجام دیا گیا ہے۔ وہ اس پر رہتے ہیں۔
  ایف ایچ ای کی طرف کیونکہ وہ انکرپٹڈ ایگزیکیوشن میکانزم کی بجائے بیان کرتے ہیں۔
  بیرونی پالیسی یا رسید کا خلاصہ۔

## 1.3 اکاؤنٹ کی شناخت بمقابلہ عرفی نام

یونیورسل اکاؤنٹ رول آؤٹ کیننیکل اکاؤنٹ شناختی ماڈل کو تبدیل نہیں کرتا ہے:- `AccountId` کیننیکل، ڈومین لیس اکاؤنٹ کا موضوع ہے۔
- `AccountAlias` اقدار اس موضوع کے اوپر علیحدہ SNS پابندیاں ہیں۔ اے
  ڈومین کوالیفائیڈ عرف جیسے `merchant@banka.paynet` اور ڈیٹا اسپیس روٹ عرف
  جیسے `merchant@paynet` دونوں ایک ہی کیننیکل `AccountId` کو حل کر سکتے ہیں۔
- کیننیکل اکاؤنٹ کی رجسٹریشن ہمیشہ `Account::new(AccountId)` / ہوتی ہے۔
  `NewAccount::new(AccountId)`; کوئی ڈومین کے لیے اہل یا ڈومین کے لیے مواد نہیں ہے۔
  رجسٹریشن کا راستہ
- ڈومین کی ملکیت، عرفی اجازتیں، اور دیگر ڈومین کے دائرہ کار کے طرز عمل زندہ ہیں۔
  اکاؤنٹ کی شناخت کے بجائے اپنی ریاست اور APIs میں۔
- عوامی اکاؤنٹ کی تلاش اس تقسیم کی پیروی کرتی ہے: عرف کے سوالات عوامی رہتے ہیں، جبکہ
  کیننیکل اکاؤنٹ کی شناخت ایک خالص `AccountId` رہتی ہے۔

آپریٹرز، SDKs اور ٹیسٹوں کے لیے نفاذ کا اصول: کینونیکل سے شروع کریں۔
`AccountId`، پھر عرفی لیز، ڈیٹا اسپیس/ڈومین کی اجازتیں، اور کوئی بھی شامل کریں
الگ سے ڈومین کی ملکیت والی ریاست۔ جعلی عرف سے حاصل کردہ اکاؤنٹ کی ترکیب نہ کریں۔
یا اکاؤنٹ کے ریکارڈ پر کسی بھی منسلک ڈومین فیلڈ کی توقع کریں صرف اس وجہ سے کہ عرف یا
روٹ میں ایک ڈومین سیگمنٹ ہوتا ہے۔

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

## 2. UAIDs اخذ کرنا اور اس کی تصدیق کرنا

UAID حاصل کرنے کے تین معاون طریقے ہیں:

1. **اسے عالمی ریاست یا SDK ماڈلز سے پڑھیں۔** کوئی بھی `Account`/`AccountDetails`
   Torii کے توسط سے پوچھے گئے پے لوڈ میں اب `uaid` فیلڈ ہے جب
   شریک نے یونیورسل اکاؤنٹس کا انتخاب کیا۔
2. **UAID رجسٹریوں سے استفسار کریں۔** Torii بے نقاب
   `GET /v1/space-directory/uaids/{uaid}` جو ڈیٹا اسپیس بائنڈنگز کو لوٹاتا ہے۔
   اور مینی فیسٹ میٹا ڈیٹا اسپیس ڈائرکٹری کا میزبان برقرار رہتا ہے (دیکھیں۔
   پے لوڈ کے نمونوں کے لیے `docs/space-directory.md` §3)۔
3. **اسے قطعی طور پر اخذ کریں۔** نئے UAIDs کو آف لائن بوٹسٹریپ کرتے وقت، ہیش
   Blake2b-256 کے ساتھ کیننیکل شریک بیج اور اس کے ساتھ نتیجہ کا سابقہ لگائیں۔
   `uaid:`۔ ذیل کا ٹکڑا دستاویز میں مددگار کی عکاسی کرتا ہے۔
   `docs/space-directory.md` §3.3:

   ```python
   import hashlib
   seed = b"participant@example"  # canonical address/domain seed
   digest = bytearray(hashlib.blake2b(seed, digest_size=32).digest())
   digest[-1] |= 1
   print(f"uaid:{digest.hex()}")
   ```لٹریل کو ہمیشہ لوئر کیس میں اسٹور کریں اور ہیش کرنے سے پہلے وائٹ اسپیس کو معمول پر رکھیں۔
CLI مددگار جیسے `iroha app space-directory manifest scaffold` اور Android
`UaidLiteral` پارسر تراشنے کے ایک ہی اصول کا اطلاق کرتا ہے تاکہ گورننس کے جائزے
ایڈہاک اسکرپٹ کے بغیر اقدار کو کراس چیک کریں۔

## 3. UAID ہولڈنگز اور مینی فیسٹس کا معائنہ کرنا

`iroha_core::nexus::portfolio` میں ڈیٹرمنسٹک پورٹ فولیو ایگریگیٹر
UAID کا حوالہ دینے والے ہر اثاثہ/ڈیٹا اسپیس جوڑے کو ظاہر کرتا ہے۔ آپریٹرز اور SDKs
درج ذیل سطحوں کے ذریعے ڈیٹا استعمال کر سکتے ہیں:

| سطح | استعمال |
|---------|---------|
| `GET /v1/accounts/{uaid}/portfolio` | ڈیٹا اسپیس → اثاثہ → بیلنس کے خلاصے لوٹاتا ہے۔ `docs/source/torii/portfolio_api.md` میں بیان کیا گیا ہے۔ |
| `GET /v1/space-directory/uaids/{uaid}` | UAID سے منسلک ڈیٹا اسپیس IDs + اکاؤنٹ لٹریلز کی فہرست۔ |
| `GET /v1/space-directory/uaids/{uaid}/manifests` | آڈٹ کے لیے مکمل `AssetPermissionManifest` ہسٹری فراہم کرتا ہے۔ |
| `iroha app space-directory bindings fetch --uaid <literal>` | CLI شارٹ کٹ جو بائنڈنگ اینڈ پوائنٹ کو لپیٹتا ہے اور اختیاری طور پر JSON کو ڈسک (`--json-out`) پر لکھتا ہے۔ |
| `iroha app space-directory manifest fetch --uaid <literal> --json-out <path>` | ثبوت کے پیک کے لیے مینی فیسٹ JSON بنڈل لاتا ہے۔ |

مثال کے طور پر CLI سیشن (`iroha.json` میں `torii_api_url` کے ذریعے Torii URL ترتیب دیا گیا):

```bash
iroha app space-directory bindings fetch \
  --uaid uaid:86e8ee39a3908460a0f4ee257bb25f340cd5b5de72735e9adefe07d5ef4bb0df \
  --json-out artifacts/uaid86/bindings.json

iroha app space-directory manifest fetch \
  --uaid uaid:86e8ee39a3908460a0f4ee257bb25f340cd5b5de72735e9adefe07d5ef4bb0df \
  --json-out artifacts/uaid86/manifests.json
```

JSON سنیپ شاٹس کو جائزوں کے دوران استعمال ہونے والے مینی فیسٹ ہیش کے ساتھ اسٹور کریں۔ دی
اسپیس ڈائرکٹری واچر جب بھی ظاہر ہوتا ہے `uaid_dataspaces` نقشہ دوبارہ بناتا ہے۔
چالو، میعاد ختم، یا منسوخ، لہذا یہ سنیپ شاٹس ثابت کرنے کا تیز ترین طریقہ ہیں۔
کسی مخصوص دور میں کون سی پابندیاں فعال تھیں۔## 4. اشاعت کی صلاحیت ثبوت کے ساتھ ظاہر ہوتی ہے۔

جب بھی نیا الاؤنس جاری کیا جائے تو نیچے دیے گئے CLI کا استعمال کریں۔ ہر قدم ضروری ہے۔
گورننس سائن آف کے لئے ریکارڈ شدہ ثبوت بنڈل میں زمین۔

1. **مینی فیسٹ JSON** کو انکوڈ کریں تاکہ جائزہ لینے والوں کو اس سے پہلے ڈیٹرمنسٹک ہیش نظر آئے
   جمع کرانے:

   ```bash
   iroha app space-directory manifest encode \
     --json fixtures/space_directory/capability/eu_regulator_audit.manifest.json \
     --out artifacts/eu_regulator_audit.manifest.to \
     --hash-out artifacts/eu_regulator_audit.manifest.hash
   ```

2. **الاؤنس شائع کریں** یا تو Norito پے لوڈ (`--manifest`) یا
   JSON تفصیل (`--manifest-json`)۔ Torii/CLI رسید پلس ریکارڈ کریں۔
   `PublishSpaceDirectoryManifest` انسٹرکشن ہیش:

   ```bash
   iroha app space-directory manifest publish \
     --manifest artifacts/eu_regulator_audit.manifest.to \
     --reason "ESMA wave 2 onboarding"
   ```

3. **Capture SpaceDirectoryEvent ثبوت۔** سبسکرائب کریں۔
   `SpaceDirectoryEvent::ManifestActivated` اور ایونٹ پے لوڈ کو شامل کریں۔
   بنڈل تاکہ آڈیٹرز تصدیق کر سکیں کہ تبدیلی کب آئی۔

4. مینی فیسٹ کو اس کے ڈیٹا اسپیس پروفائل کے ساتھ باندھتے ہوئے **ایک آڈٹ بنڈل تیار کریں اور
   ٹیلی میٹری ہکس:

   ```bash
   iroha app space-directory manifest audit-bundle \
     --manifest artifacts/eu_regulator_audit.manifest.to \
     --profile fixtures/space_directory/profile/cbdc_lane_profile.json \
     --out-dir artifacts/eu_regulator_audit_bundle
   ```

5. **Torii** (`bindings fetch` اور `manifests fetch`) کے ذریعے پابندیوں کی تصدیق کریں اور
   ان JSON فائلوں کو اوپر ہیش + بنڈل کے ساتھ آرکائیو کریں۔

ثبوت چیک لسٹ:

- [ ] مینی فیسٹ ہیش (`*.manifest.hash`) تبدیلی کے منظور کنندہ کے دستخط شدہ۔
- [ ] CLI/Torii اشاعت کال کی رسید (stdout یا `--json-out` آرٹ فیکٹ)۔
- [ ] `SpaceDirectoryEvent` پے لوڈ ایکٹیویشن ثابت کر رہا ہے۔
- ڈیٹا اسپیس پروفائل، ہکس، اور مینی فیسٹ کاپی کے ساتھ آڈٹ بنڈل ڈائرکٹری۔
- [ ] بائنڈنگز + مینی فیسٹ اسنیپ شاٹس جو Torii پوسٹ ایکٹیویشن سے حاصل کیے گئے ہیں۔یہ SDK دیتے وقت `docs/space-directory.md` §3.2 میں ضروریات کی عکاسی کرتا ہے۔
ریلیز کے جائزوں کے دوران اشارہ کرنے کے لیے ایک صفحے کے مالکان۔

## 5. ریگولیٹر/علاقائی مینی فیسٹ ٹیمپلیٹس

کرافٹنگ کی صلاحیت ظاہر ہونے پر ان ریپو فکسچر کو ابتدائی پوائنٹس کے طور پر استعمال کریں۔
ریگولیٹرز یا علاقائی نگرانوں کے لیے۔ وہ اسکوپ کی اجازت/انکار کا مظاہرہ کرتے ہیں۔
اصولوں اور پالیسی نوٹوں کی وضاحت کریں جو جائزہ لینے والوں کی توقع ہے۔

| فکسچر | مقصد | جھلکیاں |
|---------|---------|------------|
| `fixtures/space_directory/capability/eu_regulator_audit.manifest.json` | ESMA/ESRB آڈٹ فیڈ۔ | ریگولیٹر UAIDs کو غیر فعال رکھنے کے لیے ریٹیل ٹرانسفرز پر انکار جیت کے ساتھ `compliance.audit::{stream_reports, request_snapshot}` کے لیے صرف پڑھنے کے الاؤنسز۔ |
| `fixtures/space_directory/capability/jp_regulator_supervision.manifest.json` | جے ایف ایس اے کی نگرانی کی لین۔ | ڈوئل کنٹرولز کو نافذ کرنے کے لیے ایک محدود `cbdc.supervision.issue_stop_order` الاؤنس (PerDay window + `max_amount`) اور `force_liquidation` پر واضح انکار شامل کرتا ہے۔ |

ان فکسچر کی کلوننگ کرتے وقت، اپ ڈیٹ کریں:

1. `uaid` اور `dataspace` آئی ڈیز اس شریک اور لین سے مماثل ہیں جنہیں آپ فعال کر رہے ہیں۔
2. گورننس شیڈول پر مبنی `activation_epoch`/`expiry_epoch` ونڈوز۔
3. `notes` ریگولیٹر کے پالیسی حوالوں کے ساتھ فیلڈز (MiCA آرٹیکل، JFSA
   سرکلر، وغیرہ)۔
4. الاؤنس ونڈوز (`PerSlot`, `PerMinute`, `PerDay`) اور اختیاری
   `max_amount` کیپس کرتا ہے لہذا SDKs میزبان کے طور پر وہی حدود نافذ کرتے ہیں۔

## 6. SDK صارفین کے لیے مائیگریشن نوٹسموجودہ SDK انضمام جن کا حوالہ فی ڈومین اکاؤنٹ IDs پر منتقل ہونا ضروری ہے۔
اوپر بیان کردہ UAID مرکوز سطحیں۔ اپ گریڈ کے دوران اس چیک لسٹ کا استعمال کریں:

  اکاؤنٹ آئی ڈیز Rust/JS/Swift/Android کے لیے اس کا مطلب تازہ ترین میں اپ گریڈ کرنا ہے۔
  ورک اسپیس کریٹس یا Norito بائنڈنگز کو دوبارہ تخلیق کرنا۔
- **API کالز:** ڈومین کے دائرہ کار والے پورٹ فولیو سوالات کو اس سے بدل دیں۔
  `GET /v1/accounts/{uaid}/portfolio` اور مینی فیسٹ/بائنڈنگ اینڈ پوائنٹس۔
  `GET /v1/accounts/{uaid}/portfolio` اختیاری `asset_id` استفسار کو قبول کرتا ہے
  پیرامیٹر جب بٹوے کو صرف ایک اثاثہ مثال کی ضرورت ہوتی ہے۔ کلائنٹ مددگار اس طرح
  جیسا کہ `ToriiClient.getUaidPortfolio` (JS) اور Android
  `SpaceDirectoryClient` پہلے ہی ان راستوں کو لپیٹ چکا ہے۔ ان کو اپنی مرضی سے ترجیح دیں۔
  HTTP کوڈ۔
- **کیچنگ اور ٹیلی میٹری:** خام کی بجائے UAID + ڈیٹا اسپیس کے ذریعہ کیش اندراجات
  اکاؤنٹ آئی ڈیز، اور ایمیٹ ٹیلی میٹری کو UAID لٹریل دکھاتا ہے تاکہ آپریشنز ہو سکیں
  اسپیس ڈائرکٹری شواہد کے ساتھ لاگز کو لائن اپ کریں۔
- **خرابی سے نمٹنا:** نئے اختتامی نقطے UAID کی تصریف کی سخت غلطیاں واپس کرتے ہیں۔
  `docs/source/torii/portfolio_api.md` میں دستاویزی؛ ان کوڈز کی سطح
  لفظی طور پر تاکہ سپورٹ ٹیمیں ریپرو اقدامات کے بغیر مسائل کو ٹریج کر سکیں۔
- **ٹیسٹنگ:** اوپر بیان کردہ فکسچر کو وائر کریں (علاوہ آپ کا اپنا UAID مینی فیسٹ)
  SDK ٹیسٹ سویٹس میں Norito راؤنڈ ٹرپس اور واضح تشخیص کو ثابت کرنے کے لیے
  میزبان کے نفاذ سے ملائیں۔

## 7. حوالہ جات- `docs/space-directory.md` — آپریٹر پلے بک جس میں لائف سائیکل کی گہری تفصیل ہے۔
- `docs/source/torii/portfolio_api.md` - UAID پورٹ فولیو کے لیے REST اسکیما اور
  ظاہری نقطہ.
- `crates/iroha_cli/src/space_directory.rs` — CLI نفاذ کا حوالہ دیا گیا ہے۔
  یہ گائیڈ.
- `fixtures/space_directory/capability/*.manifest.json` — ریگولیٹر، ریٹیل، اور
  CBDC مینی فیسٹ ٹیمپلیٹس کلوننگ کے لیے تیار ہیں۔

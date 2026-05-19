<!-- Auto-generated stub for Mongolian (mn) translation. Replace this content with the full translation. -->

---
lang: mn
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

# Бүх нийтийн дансны гарын авлага

Энэхүү гарын авлага нь UAID (Universal Account ID)-ийн танилцуулгад тавигдах шаардлагыг
Nexus замын зураглалыг гаргаж, тэдгээрийг оператор + SDK-д чиглэсэн заавар болгон багцлана.
Энэ нь UAID гаралт, багц/манифест шалгалт, зохицуулагчийн загварууд,
мөн `iroha програмын сансрын лавлах манифест бүрийг дагалдах ёстой нотлох баримтууд
publish` run (roadmap reference: `roadmap.md:2209`).

## 1. UAID хурдан лавлагаа- UAID нь `uaid:<hex>` литерал бөгөөд `<hex>` нь Blake2b-256 дижест бөгөөд
  LSB-г `1` гэж тохируулсан. Каноник төрөл нь амьдардаг
  `crates/iroha_data_model/src/nexus/manifest.rs::UniversalAccountId`.
- Дансны бүртгэлд (`Account` ба `AccountDetails`) одоо нэмэлт `uaid` бичигдсэн байна.
  талбарт оруулснаар программууд тусгай хэшгүйгээр танигчийг сурах боломжтой.
- Далд функц тодорхойлогч бодлого нь дурын хэвийн оролтыг холбож болно
  (утасны дугаар, имэйл, дансны дугаар, түншийн тэмдэгт) `opaque:` ID-ууд
  UAID нэрийн орон зайн дор. Гинжний хэсгүүд нь `IdentifierPolicy`,
  `IdentifierClaimRecord`, `opaque_id -> uaid` индекс.
- Сансрын лавлах нь UAID бүрийг холбосон `World::uaid_dataspaces` газрын зургийг хадгалдаг.
  идэвхтэй манифестуудаар иш татсан өгөгдлийн орон зайн данс руу. Torii үүнийг дахин ашигладаг
  `/portfolio` болон `/uaids/*` API-д зориулсан газрын зураг.
- `POST /v1/accounts/onboard` нь өгөгдмөл сансрын лавлах манифестийг нийтэлдэг.
  дэлхийн өгөгдлийн орон зай байхгүй үед UAID шууд холбогддог.
  Онгоцны эрх баригчид `CanPublishSpaceDirectoryManifest{dataspace=0}` байх ёстой.
- Бүх SDK-ууд UAID литералуудыг каноник болгоход туслах хэрэгслүүдийг гаргадаг (жишээ нь,
  Android SDK дээрх `UaidLiteral`). Туслах ажилтнууд 64-гекс түүхий эдийг хүлээн авдаг
  (LSB=1) эсвэл `uaid:<hex>` литерал ба ижил Norito кодлогчийг дахин ашигла.
  digest нь хэлээр дамжиж чадахгүй.

## 1.1 Нуугдсан танигч бодлого

UAID нь одоо хоёр дахь таних давхаргын зангуу болж байна:- Глобал `IdentifierPolicyId` (`<kind>#<business_rule>`) нь дараахь зүйлийг тодорхойлдог.
  нэрийн орон зай, нийтийн амлалтын мета өгөгдөл, шийдвэрлэгчийн баталгаажуулах түлхүүр болон
  каноник оролтыг хэвийн болгох горим (`Exact`, `LowercaseTrimmed`,
  `PhoneE164`, `EmailAddress`, эсвэл `AccountNumber`).
- Нэхэмжлэл нь нэг үүсмэл `opaque:` танигчийг яг нэг UAID болон нэгтэй холбодог.
  Энэ бодлогын дагуу `AccountId` каноник боловч гинж нь зөвхөн
  гарын үсэг зурсан `IdentifierResolutionReceipt` бичгийг хавсаргасан тохиолдолд нэхэмжлэх.
- Нарийвчлал нь `resolve -> transfer` урсгалтай хэвээр байна. Torii тунгалаг бус байдлыг шийддэг
  каноник `AccountId`-ийг зохицуулж, буцаана; шилжүүлэг зорилтот хэвээр байна
  `uaid:` эсвэл `opaque:` шууд утгаар биш, каноник данс.
- Бодлого нь одоо BFV оролт-шифрлэлтийн параметрүүдийг дамжуулан нийтлэх боломжтой
  `PolicyCommitment.public_parameters`. Байгаа үед Torii тэднийг сурталчилдаг
  `GET /v1/identifier-policies` ба үйлчлүүлэгчид BFV ороосон оролтыг оруулж болно
  энгийн текстийн оронд. Програмчлагдсан бодлого нь BFV параметрүүдийг a
  каноник `BfvProgrammedPublicParameters` багц нь мөн хэвлэгддэг
  нийтийн `ram_fhe_profile`; Хуучин түүхий BFV ачааллыг үүн дээр шинэчилсэн
  амлалт дахин бий болсон үед каноник багц.
- Тодорхойлогч маршрутууд нь ижил Torii хандалтын токен болон хурдны хязгаараар дамждаг.
  бусад апп-д тулгарч буй төгсгөлийн цэгүүд шиг шалгадаг. Тэд ердийнхөөс тойрч гарах зам биш юм
  API бодлого.

## 1.2 Нэр томьёо

Нэрний хуваагдал нь зориудаар:- `ram_lfe` нь далд функцийн гаднах хийсвэрлэл юм. Энэ нь бодлогыг хамардаг
  бүртгэл, амлалт, нийтийн мета өгөгдөл, гүйцэтгэлийн баримт, болон
  баталгаажуулах горим.
- `BFV` нь Brakerski/Fan-Vercauteren гомоморф шифрлэлтийн схем юм.
  шифрлэгдсэн оролтыг үнэлэх зарим `ram_lfe` backends.
- `ram_fhe_profile` нь BFV-д хамаарах мета өгөгдөл бөгөөд бүхэлд нь хоёр дахь нэр биш
  онцлог. Энэ нь түрийвч болон программчлагдсан BFV гүйцэтгэх машин тайлбарлах
  Бодлого нь програмчлагдсан арын хэсгийг ашиглах үед баталгаажуулагч зорилтот байх ёстой.

Тодорхой хэллэгээр:

- `RamLfeProgramPolicy` ба `RamLfeExecutionReceipt` нь LFE давхаргын төрөл юм.
- `BfvParameters`, `BfvCiphertext`, `BfvProgrammedPublicParameters`, болон
  `BfvRamProgramProfile` нь FHE давхаргын төрлүүд юм.
- `HiddenRamFheProgram` ба `HiddenRamFheInstruction` нь дотоод нэр юм.
  програмчлагдсан backend-ээр гүйцэтгэгдсэн далд BFV программ. Тэд дээр үлддэг
  FHE тал, учир нь тэд илүү шифрлэгдсэн гүйцэтгэх механизмыг тайлбарлах
  гадаад бодлого эсвэл баримтын хийсвэрлэл.

## 1.3 Бүртгэлийн таниулбар болон бусад нэр

Бүх нийтийн дансны танилцуулга нь каноник дансны таних загварыг өөрчлөхгүй:- `AccountId` нь каноник, домэйнгүй дансны сэдэв хэвээр байна.
- `AccountAlias` утгууд нь тухайн сэдвийн дээд талд байгаа тусдаа SNS холболтууд юм. А
  `merchant@banka.paynet` болон өгөгдлийн орон зайн эх нэр гэх мэт домэйны шаардлага хангасан нэр
  `merchant@paynet` зэрэг нь хоёулаа ижил каноник `AccountId`-г шийдэж чадна.
- Каноник дансны бүртгэл үргэлж `Account::new(AccountId)` байна /
  `NewAccount::new(AccountId)`; домайны шаардлага хангасан эсвэл домэйн материалжуулсан зүйл байхгүй
  бүртгэлийн зам.
- Домэйн эзэмшил, бусад нэрийн зөвшөөрөл болон бусад домэйны хамрах хүрээний зан үйлүүд амьдардаг
  Бүртгэлийн таниулбараас илүүтэйгээр өөрсдийн төлөв болон API-д.
- Нийтийн акаунтын хайлтыг дараах байдлаар хуваана: нэрийн асуулга нь нийтэд үлддэг
  Каноник дансны таних тэмдэг нь цэвэр `AccountId` хэвээр байна.

Оператор, SDK болон тестийг хэрэгжүүлэх дүрэм: каноникаас эхэлнэ
`AccountId`, дараа нь нэрийн түрээс, дата зай/домэйн зөвшөөрөл болон дурын зүйлийг нэмнэ үү.
тус тусад нь домэйн эзэмшдэг муж. Хуурамч нэрээр үүсгэгдсэн бүртгэлийг нэгтгэж болохгүй
эсвэл дансны бүртгэл дээрх ямар нэгэн холбоотой домэйн талбарыг зөвхөн өөр нэр эсвэл
маршрут нь домэйн сегментийг дамжуулдаг.

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

## 2. UAID-г гаргаж авах, баталгаажуулах

UAID авах гурван дэмжигдсэн арга байдаг:

1. **Үүнийг дэлхийн төлөв эсвэл SDK загвараас уншина уу.** Дурын `Account`/`AccountDetails`
   Torii-ээр асуусан ачааллыг одоо `uaid` талбарт бөглөсөн байна.
   Оролцогч бүх нийтийн дансыг сонгосон.
2. **UAID бүртгэлээс лавлана уу.** Torii илрүүлнэ
   `GET /v1/space-directory/uaids/{uaid}` нь өгөгдлийн орон зайн холболтыг буцаадаг
   болон Сансрын лавлах хостын манифест метадата (харна уу
   Ачааллын дээжийн хувьд `docs/space-directory.md` §3).
3. **Үүнийг тодорхой гаргаарай.** Шинэ UAID-г офлайнаар ачаалах үед хэш
   каноник оролцогчийн үрийг Blake2b-256-тай холбож, үр дүнгийн угтварыг бичнэ
   `uaid:`. Доорх хэсэг нь баримтжуулсан туслагчийг толилуулж байна
   `docs/space-directory.md` §3.3:

   ```python
   import hashlib
   seed = b"participant@example"  # canonical address/domain seed
   digest = bytearray(hashlib.blake2b(seed, digest_size=32).digest())
   digest[-1] |= 1
   print(f"uaid:{digest.hex()}")
   ```Хэшгэхийн өмнө үсгийг жижиг үсгээр бичиж, хоосон зайг хэвийн болго.
`iroha app space-directory manifest scaffold` болон Android зэрэг CLI туслахууд
`UaidLiteral` задлагч ижил шүргэх дүрмийг ашигладаг тул засаглалын тойм
тусгай скриптгүйгээр утгуудыг шалгах.

## 3. UAID-ийн эзэмшил болон манифестуудыг шалгах

`iroha_core::nexus::portfolio` дахь детерминист багцын агрегатор
UAID-д хамаарах хөрөнгө/өгөгдлийн орон зай бүрийг харуулдаг. Операторууд ба SDK
дараах гадаргуугаар дамжуулан өгөгдлийг ашиглаж болно:

| Гадаргуу | Хэрэглээ |
|---------|-------|
| `GET /v1/accounts/{uaid}/portfolio` | Өгөгдлийн орон зай → хөрөнгө → үлдэгдлийн хураангуйг буцаана; `docs/source/torii/portfolio_api.md`-д тайлбарласан. |
| `GET /v1/space-directory/uaids/{uaid}` | UAID-тэй холбогдсон өгөгдлийн орон зайн ID + бүртгэлийн литералуудыг жагсаана. |
| `GET /v1/space-directory/uaids/{uaid}/manifests` | Аудитын бүрэн `AssetPermissionManifest` түүхийг өгдөг. |
| `iroha app space-directory bindings fetch --uaid <literal>` | Холболтын төгсгөлийн цэгийг ороож, JSON-г диск рүү (`--json-out`) бичдэг CLI товчлол. |
| `iroha app space-directory manifest fetch --uaid <literal> --json-out <path>` | Нотлох баримтын багцад зориулсан манифест JSON багцыг татаж авдаг. |

Жишээ CLI сесс (`iroha.json` дээр `torii_api_url`-ээр тохируулсан Torii URL):

```bash
iroha app space-directory bindings fetch \
  --uaid uaid:86e8ee39a3908460a0f4ee257bb25f340cd5b5de72735e9adefe07d5ef4bb0df \
  --json-out artifacts/uaid86/bindings.json

iroha app space-directory manifest fetch \
  --uaid uaid:86e8ee39a3908460a0f4ee257bb25f340cd5b5de72735e9adefe07d5ef4bb0df \
  --json-out artifacts/uaid86/manifests.json
```

JSON агшин агшингуудыг шалгах явцад ашигласан манифест хэшийн хажууд хадгалах; нь
Сансрын лавлах ажиглагч нь илрэх бүрт `uaid_dataspaces` газрын зургийг дахин бүтээдэг.
идэвхжүүлэх, хүчингүй болгох, хүчингүй болгох, тиймээс эдгээр хормын хувилбарууд нь нотлох хамгийн хурдан арга юм.
Тухайн эрин үед ямар холболтууд идэвхтэй байсан.## 4. Нийтлэх чадвар нь нотлох баримтаар илэрдэг

Шинэ тэтгэмж гарах бүрт доорх CLI урсгалыг ашиглаарай. Алхам бүр заавал байх ёстой
засаглалын гарын үсэг зурахаар бүртгэгдсэн нотлох баримтын багцад газар.

1. **Манифест JSON-г кодчилно уу**, ингэснээр хянагчид өмнө нь тодорхойлогч хэшийг харна
   илгээх:

   ```bash
   iroha app space-directory manifest encode \
     --json fixtures/space_directory/capability/eu_regulator_audit.manifest.json \
     --out artifacts/eu_regulator_audit.manifest.to \
     --hash-out artifacts/eu_regulator_audit.manifest.hash
   ```

2. **Тэтгэмжийг нийтлэх** эсвэл Norito (`--manifest`) эсвэл
   JSON тайлбар (`--manifest-json`). Torii/CLI баримтыг нэмж бичнэ үү
   `PublishSpaceDirectoryManifest` зааврын хэш:

   ```bash
   iroha app space-directory manifest publish \
     --manifest artifacts/eu_regulator_audit.manifest.to \
     --reason "ESMA wave 2 onboarding"
   ```

3. **SpaceDirectoryEvent-ийн нотлох баримтыг аваарай.** Бүртгүүлэх
   `SpaceDirectoryEvent::ManifestActivated` ба үйл явдлын ачааллыг оруулах
   аудиторууд өөрчлөлт хэзээ орж ирснийг батлах боломжтой багц.

4. **Аудитын багц үүсгэх** манифестыг өгөгдлийн орон зайн профайл болон
   телеметрийн дэгээ:

   ```bash
   iroha app space-directory manifest audit-bundle \
     --manifest artifacts/eu_regulator_audit.manifest.to \
     --profile fixtures/space_directory/profile/cbdc_lane_profile.json \
     --out-dir artifacts/eu_regulator_audit_bundle
   ```

5. **Torii** (`bindings fetch` болон `manifests fetch`)-ээр дамжуулан холболтыг баталгаажуулна уу.
   эдгээр JSON файлуудыг дээрх хэш + багцаар архивлана.

Нотлох баримт шалгах хуудас:

- [ ] Өөрчлөлтийг баталгаажуулагч гарын үсэг зурсан манифест хэш (`*.manifest.hash`).
- [ ] Нийтлэх дуудлагын CLI/Torii баримт (stdout эсвэл `--json-out` олдвор).
- [ ] `SpaceDirectoryEvent` ачааллыг нотлох идэвхжүүлэлт.
- [ ] Өгөгдлийн орон зайн профайл, дэгээ, манифест хуулбар бүхий багцын лавлахад аудит хийх.
- [ ] Идэвхжүүлсний дараах Torii-аас авчирсан холболтууд + манифест агшин зуурын зургууд.Энэ нь SDK өгөх үед `docs/space-directory.md` §3.2-ын шаардлагыг тусгадаг.
Эзэмшигчид нь хувилбарын үнэлгээний үеэр зааж өгөх нэг хуудас.

## 5. Зохицуулагч/бүс нутгийн манифест загварууд

Урлах чадвар илрэх үед репо дахь бэхэлгээг эхлэх цэг болгон ашиглаарай
зохицуулагчид эсвэл бүс нутгийн хянагчдад зориулсан. Тэд зөвшөөрөх/татгалзах хамрах хүрээг хэрхэн харуулахыг харуулдаг
журмууд болон тоймчдын хүлээж буй бодлогын тэмдэглэлүүдийг тайлбарла.

| Бэхэлгээ | Зорилго | Онцлох үйл явдал |
|---------|---------|------------|
| `fixtures/space_directory/capability/eu_regulator_audit.manifest.json` | ESMA/ESRB аудитын хангамж. | `compliance.audit::{stream_reports, request_snapshot}`-д зориулсан зөвхөн унших боломжтой, зохицуулагчийн UAID-г идэвхгүй байлгахын тулд жижиглэнгийн шилжүүлгийг үгүйсгэх боломжтой. |
| `fixtures/space_directory/capability/jp_regulator_supervision.manifest.json` | JFSA хяналтын эгнээ. | Давхар хяналтыг хэрэгжүүлэхийн тулд `cbdc.supervision.issue_stop_order` хязгаарлагдмал тэтгэмж (Өдөр тутмын цонх + `max_amount`) болон `force_liquidation` дээр тодорхой үгүйсгэлийг нэмдэг. |

Эдгээр бэхэлгээг клон хийхдээ дараахыг шинэчилнэ үү:

1. `uaid` болон `dataspace` id-ууд нь таны идэвхжүүлж буй оролцогч болон эгнээнд таарна.
2. Засаглалын хуваарь дээр үндэслэн `activation_epoch`/`expiry_epoch` цонх.
3. Зохицуулагчийн бодлогын лавлагаа бүхий `notes` талбарууд (MiCA нийтлэл, JFSA)
   дугуй гэх мэт).
4. Тэтгэмжийн цонх (`PerSlot`, `PerMinute`, `PerDay`) болон нэмэлт
   `max_amount` хязгаартай тул SDK нь хосттой ижил хязгаарлалтыг мөрддөг.

## 6. SDK хэрэглэгчдэд зориулсан шилжүүлгийн тэмдэглэлДомэйн дансны ID-д хандсан одоо байгаа SDK интеграцууд руу шилжих ёстой
дээр дурдсан UAID төвтэй гадаргуу. Шинэчлэлтийн үед энэ хяналтын хуудсыг ашиглана уу:

  дансны дугаар. Rust/JS/Swift/Android-ын хувьд энэ нь хамгийн сүүлийн үеийн хувилбар руу шинэчлэх гэсэн үг юм
  ажлын талбайн хайрцаг эсвэл Norito холболтыг сэргээж байна.
- **API дуудлагууд:** Домэйн хамрах хүрээний багцын асуулгыг дараахаар солино
  `GET /v1/accounts/{uaid}/portfolio` ба манифест/холбох төгсгөлийн цэгүүд.
  `GET /v1/accounts/{uaid}/portfolio` нь нэмэлт `asset_id` хүсэлтийг хүлээн авдаг.
  түрийвчэнд зөвхөн нэг хөрөнгийн жишээ хэрэгтэй үед параметр. Үйлчлүүлэгчийн туслахууд ийм
  `ToriiClient.getUaidPortfolio` (JS) болон Android
  `SpaceDirectoryClient` аль хэдийн эдгээр маршрутуудыг боож; захиалгаар хийхээс илүүд үздэг
  HTTP код.
- **Кэш ба телеметр:** Түүхий биш харин UAID + өгөгдлийн орон зайгаар кэш оруулах
  дансны ids, мөн UAID утгыг харуулсан телеметрийг ялгаруулж, үйлдлүүдийг хийх боломжтой
  Сансрын лавлах нотлох баримт бүхий бүртгэлүүдийг жагсаах.
- **Алдаа боловсруулах:** Шинэ төгсгөлийн цэгүүд нь хатуу UAID задлан шинжлэх алдааг буцаана
  `docs/source/torii/portfolio_api.md`-д баримтжуулсан; эдгээр кодуудыг гарга
  Тиймээс тусламжийн багууд дахин давтах алхамгүйгээр асуудлыг шийдвэрлэх боломжтой.
- **Туршилт:** Дээр дурдсан хэрэгслүүдийг холбоно уу (өөрийн UAID манифест)
  Norito хоёр талын аялал болон манифест үнэлгээг батлахын тулд SDK тестийн багц руу оруулна
  хостын хэрэгжилттэй таарч байна.

## 7. Ашигласан материал- `docs/space-directory.md` — амьдралын мөчлөгийн дэлгэрэнгүй мэдээлэл бүхий операторын тоглоомын ном.
- `docs/source/torii/portfolio_api.md` — UAID багцын REST схем болон
  илэрхий төгсгөлийн цэгүүд.
- `crates/iroha_cli/src/space_directory.rs` — CLI хэрэгжилтийг иш татсан
  энэ гарын авлага.
- `fixtures/space_directory/capability/*.manifest.json` — зохицуулагч, жижиглэн худалдаа, болон
  CBDC манифест загварууд клончлоход бэлэн байна.

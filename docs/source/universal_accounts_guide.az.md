<!-- Auto-generated stub for Azerbaijani (az) translation. Replace this content with the full translation. -->

---
lang: az
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

# Universal Hesab Bələdçisi

Bu bələdçi UAID (Universal Account ID) tətbiqi tələblərini distillə edir
Nexus yol xəritəsi və onları operatora + SDK fokuslanmış keçidə paketləyir.
O, UAID törəmə, portfel/manifest yoxlaması, tənzimləyici şablonları,
və hər `iroha app kosmik kataloqu manifestini müşayiət etməli olan sübutlar
publish` run (roadmap reference: `roadmap.md:2209`).

## 1. UAID sürətli arayışı- UAID-lər `uaid:<hex>` hərfidir, burada `<hex>` Blake2b-256 həzmidir
  LSB `1` olaraq təyin edilmişdir. Kanonik tip yaşayır
  `crates/iroha_data_model/src/nexus/manifest.rs::UniversalAccountId`.
- Hesab qeydləri (`Account` və `AccountDetails`) indi isteğe bağlı `uaid` daşıyır
  sahəsində tətbiqlər identifikatoru sifariş vermədən öyrənə bilsin.
- Gizli funksiyalı identifikator siyasətləri ixtiyari normallaşdırılmış girişləri bağlaya bilər
  (telefon nömrələri, e-poçtlar, hesab nömrələri, tərəfdaş sətirləri) `opaque:` ID-lərinə
  UAID ad sahəsi altında. Zəncirdəki parçalar `IdentifierPolicy`,
  `IdentifierClaimRecord` və `opaque_id -> uaid` indeksi.
- Space Directory hər bir UAID-i birləşdirən `World::uaid_dataspaces` xəritəsini saxlayır.
  aktiv manifestlər tərəfindən istinad edilən məlumat məkanı hesablarına. Torii bundan təkrar istifadə edir
  `/portfolio` və `/uaids/*` API üçün xəritə.
- `POST /v1/accounts/onboard` üçün defolt Space Directory manifestini dərc edir
  heç biri mövcud olmadıqda qlobal məlumat məkanı, buna görə də UAID dərhal bağlanır.
  Təyyarə orqanları `CanPublishSpaceDirectoryManifest{dataspace=0}` saxlamalıdır.
- Bütün SDK-lar UAID hərflərinin kanonikləşdirilməsi üçün köməkçiləri ifşa edir (məsələn,
  Android SDK-da `UaidLiteral`). Köməkçilər xam 64 hex həzmləri qəbul edirlər
  (LSB=1) və ya `uaid:<hex>` hərfi və eyni Norito kodeklərindən yenidən istifadə edin ki,
  həzm dillər arasında sürüşə bilməz.

## 1.1 Gizli identifikator siyasətləri

UAID-lər indi ikinci identifikasiya qatının lövbəridir:- Qlobal `IdentifierPolicyId` (`<kind>#<business_rule>`) müəyyən edir
  ad sahəsi, ictimai öhdəlik metadata, həlledicinin doğrulama açarı və
  kanonik giriş normallaşdırma rejimi (`Exact`, `LowercaseTrimmed`,
  `PhoneE164`, `EmailAddress` və ya `AccountNumber`).
- İddia bir əldə edilmiş `opaque:` identifikatorunu tam olaraq bir UAID və birinə bağlayır
  bu siyasətə uyğun olaraq kanonik `AccountId`, lakin zəncir yalnız
  imzalanmış `IdentifierResolutionReceipt` ilə müşayiət edildikdə iddia.
- Qətnamə `resolve -> transfer` axını olaraq qalır. Torii qeyri-şəffaflığı həll edir
  kanonik `AccountId`-i idarə edir və qaytarır; transferləri hələ də hədəfləyir
  kanonik hesab, birbaşa `uaid:` və ya `opaque:` literalları deyil.
- Siyasətlər indi vasitəsilə BFV giriş-şifrləmə parametrlərini dərc edə bilər
  `PolicyCommitment.public_parameters`. Mövcud olduqda, Torii onları reklam edir
  `GET /v1/identifier-policies` və müştərilər BFV ilə sarılmış giriş təqdim edə bilər
  düz mətn əvəzinə. Proqramlaşdırılmış siyasətlər BFV parametrlərini a
  kanonik `BfvProgrammedPublicParameters` paketi də dərc edir
  ictimai `ram_fhe_profile`; köhnə xam BFV yükləri bunun üzərinə yenilənir
  öhdəlik yenidən qurulduqda kanonik paket.
- İdentifikator marşrutları eyni Torii giriş nişanı və tarif limitindən keçir
  digər proqramla əlaqəli son nöqtələr kimi yoxlayır. Onlar normal ətrafında bir bypass deyil
  API siyasəti.

## 1.2 Terminologiya

Adlandırmanın bölünməsi qəsdəndir:- `ram_lfe` xarici gizli funksiya abstraksiyasıdır. Siyasəti əhatə edir
  qeydiyyat, öhdəliklər, ictimai metadata, icra qəbzləri və
  yoxlama rejimi.
- `BFV` tərəfindən istifadə edilən Brakerski/Fan-Vercauteren homomorfik şifrələmə sxemidir.
  şifrələnmiş girişi qiymətləndirmək üçün bəzi `ram_lfe` arxa ucları.
- `ram_fhe_profile` BFV-yə xas metadatadır, bütünlük üçün ikinci ad deyil
  xüsusiyyət. Bu cüzdan və proqramlaşdırılmış BFV icra maşın təsvir edir
  doğrulayıcılar siyasət proqramlaşdırılmış arxa plandan istifadə etdikdə hədəf almalıdır.

Konkret olaraq:

- `RamLfeProgramPolicy` və `RamLfeExecutionReceipt` LFE qat növləridir.
- `BfvParameters`, `BfvCiphertext`, `BfvProgrammedPublicParameters` və
  `BfvRamProgramProfile` FHE qat növləridir.
- `HiddenRamFheProgram` və `HiddenRamFheInstruction` daxili adlardır
  proqramlaşdırılmış backend tərəfindən icra edilən gizli BFV proqramı. Üzərində qalırlar
  FHE tərəfi çünki onlar daha çox şifrəli icra mexanizmini təsvir edirlər
  xarici siyasət və ya qəbz abstraksiya.

## 1.3 Hesab identifikasiyası ləqəblərə qarşı

Universal hesabın tətbiqi kanonik hesab şəxsiyyət modelini dəyişmir:- `AccountId` kanonik, domensiz hesab mövzusu olaraq qalır.
- `AccountAlias` dəyərləri bu mövzunun üstündəki ayrı SNS bağlamalarıdır. A
  `merchant@banka.paynet` və dataspace-kök ləqəbi kimi domenə uyğun ləqəb
  `merchant@paynet` kimi hər ikisi eyni kanonik `AccountId` ilə həll edə bilər.
- Canonical hesab qeydiyyatı həmişə `Account::new(AccountId)` /
  `NewAccount::new(AccountId)`; heç bir domenə uyğunlaşdırılmış və ya domen materiallaşdırılmış yoxdur
  qeydiyyat yolu.
- Domen sahibliyi, ləqəb icazələri və digər domen əhatəli davranışlar canlıdır
  hesab identifikasiyasının özündə deyil, öz vəziyyətində və API-lərində.
- İctimai hesab axtarışı bu bölünmənin ardınca gedir: ləqəb sorğuları açıq qalır
  kanonik hesab şəxsiyyəti təmiz `AccountId` olaraq qalır.

Operatorlar, SDK-lar və testlər üçün icra qaydası: kanonikdən başlayın
`AccountId`, sonra ləqəb icarələri, məlumat məkanı/domen icazələri və hər hansı əlavə edin
ayrıca domenə məxsus dövlət. Saxta ləqəbdən əldə edilən hesabı sintez etməyin
və ya təxəllüs və ya hesab qeydlərində hər hansı əlaqəli domen sahəsini gözləyin
marşrut bir domen seqmentini daşıyır.

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

## 2. UAID-lərin çıxarılması və yoxlanması

UAID əldə etməyin üç dəstəklənən yolu var:

1. **Dünya dövləti və ya SDK modellərindən oxuyun.** İstənilən `Account`/`AccountDetails`
   Torii vasitəsilə sorğulanan faydalı yük indi `uaid` sahəsinə malikdir
   iştirakçı universal hesabları seçdi.
2. **UAID reyestrlərini sorğulayın.** Torii ifşa edir
   Məlumat məkanı bağlamalarını qaytaran `GET /v1/space-directory/uaids/{uaid}`
   və Kosmik Kataloq hostunun açıq metadatasını davam etdirir (bax
   `docs/space-directory.md` faydalı yük nümunələri üçün §3).
3. **Bunu qəti şəkildə əldə edin.** Yeni UAID-ləri oflayn yükləyərkən, hash edin
   Blake2b-256 ilə kanonik iştirakçı toxumu və nəticəyə prefiks qoyun
   `uaid:`. Aşağıdakı parça sənədləşdirilmiş köməkçini əks etdirir
   `docs/space-directory.md` §3.3:

   ```python
   import hashlib
   seed = b"participant@example"  # canonical address/domain seed
   digest = bytearray(hashlib.blake2b(seed, digest_size=32).digest())
   digest[-1] |= 1
   print(f"uaid:{digest.hex()}")
   ```Həmişə hərfi kiçik hərflə saxlayın və hashing etməzdən əvvəl boşluqları normallaşdırın.
`iroha app space-directory manifest scaffold` və Android kimi CLI köməkçiləri
`UaidLiteral` təhlilçisi eyni kəsmə qaydalarını tətbiq edir ki, idarəetmə rəyləri
ad hoc skriptləri olmayan dəyərləri çarpaz yoxlayın.

## 3. UAID holdinqlərinin və manifestlərinin yoxlanması

`iroha_core::nexus::portfolio`-də deterministik portfel toplayıcısı
UAID-ə istinad edən hər bir aktiv/dataspace cütünü üzə çıxarır. Operatorlar və SDK-lar
məlumatları aşağıdakı səthlər vasitəsilə istehlak edə bilər:

| Səthi | İstifadə |
|---------|-------|
| `GET /v1/accounts/{uaid}/portfolio` | Məlumat məkanı → aktiv → balans xülasəsini qaytarır; `docs/source/torii/portfolio_api.md`-də təsvir edilmişdir. |
| `GET /v1/space-directory/uaids/{uaid}` | UAID ilə əlaqəli məlumat məkanı identifikatorlarını + hesab hərflərini siyahıya alır. |
| `GET /v1/space-directory/uaids/{uaid}/manifests` | Auditlər üçün tam `AssetPermissionManifest` tarixçəsini təqdim edir. |
| `iroha app space-directory bindings fetch --uaid <literal>` | Bağlamaların son nöqtəsini saran və əlavə olaraq JSON-u diskə yazan CLI qısayolu (`--json-out`). |
| `iroha app space-directory manifest fetch --uaid <literal> --json-out <path>` | Sübut paketləri üçün manifest JSON paketini gətirir. |

Nümunə CLI sessiyası (`iroha.json`-də `torii_api_url` vasitəsilə konfiqurasiya edilmiş Torii URL):

```bash
iroha app space-directory bindings fetch \
  --uaid uaid:86e8ee39a3908460a0f4ee257bb25f340cd5b5de72735e9adefe07d5ef4bb0df \
  --json-out artifacts/uaid86/bindings.json

iroha app space-directory manifest fetch \
  --uaid uaid:86e8ee39a3908460a0f4ee257bb25f340cd5b5de72735e9adefe07d5ef4bb0df \
  --json-out artifacts/uaid86/manifests.json
```

Baxışlar zamanı istifadə olunan manifest hash ilə yanaşı JSON snapshotlarını saxlayın; the
Space Directory izləyicisi hər dəfə aşkar edildikdə `uaid_dataspaces` xəritəsini yenidən qurur
aktivləşdirin, müddəti bitsin və ya ləğv edin, buna görə də bu anlıq görüntülər sübut etməyin ən sürətli yoludur
müəyyən bir dövrdə hansı bağlamaların aktiv olduğunu.## 4. Nəşretmə qabiliyyəti sübutlarla özünü göstərir

Yeni müavinət təqdim edildikdə aşağıdakı CLI axınından istifadə edin. Hər addım olmalıdır
idarəetmənin imzalanması üçün qeydə alınmış sübut paketində yer.

1. **Manifest JSON-u kodlayın** beləliklə, rəyçilər deterministik hash-i daha əvvəl görsünlər
   təqdim:

   ```bash
   iroha app space-directory manifest encode \
     --json fixtures/space_directory/capability/eu_regulator_audit.manifest.json \
     --out artifacts/eu_regulator_audit.manifest.to \
     --hash-out artifacts/eu_regulator_audit.manifest.hash
   ```

2. **Norito faydalı yükündən (`--manifest`) və ya istifadə edərək, **Müvəqqəti dərc edin**
   JSON təsviri (`--manifest-json`). Torii/CLI qəbzini plus qeyd edin
   `PublishSpaceDirectoryManifest` təlimat hash:

   ```bash
   iroha app space-directory manifest publish \
     --manifest artifacts/eu_regulator_audit.manifest.to \
     --reason "ESMA wave 2 onboarding"
   ```

3. **SpaceDirectoryEvent sübutunu əldə edin.** Abunə olun
   `SpaceDirectoryEvent::ManifestActivated` və hadisə yükünü daxil edin
   paketi beləliklə auditorlar dəyişikliyin nə vaxt baş verdiyini təsdiq edə bilsinlər.

4. **Audit paketi yaradın** manifesti onun məlumat məkanı profilinə birləşdirin və
   telemetriya qarmaqları:

   ```bash
   iroha app space-directory manifest audit-bundle \
     --manifest artifacts/eu_regulator_audit.manifest.to \
     --profile fixtures/space_directory/profile/cbdc_lane_profile.json \
     --out-dir artifacts/eu_regulator_audit_bundle
   ```

5. **Torii** (`bindings fetch` və `manifests fetch`) vasitəsilə bağlamaları yoxlayın və
   həmin JSON fayllarını yuxarıdakı hash + paketi ilə arxivləşdirin.

Sübut yoxlama siyahısı:

- [ ] Dəyişikliyi təsdiqləyən tərəfindən imzalanmış manifest hash (`*.manifest.hash`).
- [ ] Dərc çağırışı üçün CLI/Torii qəbzi (stdout və ya `--json-out` artefaktı).
- [ ] `SpaceDirectoryEvent` faydalı yükün aktivləşdirilməsini sübut edir.
- [ ] Məlumat məkanı profili, qarmaqlar və manifest nüsxəsi ilə paket kataloqunu yoxlayın.
- [ ] Bağlamalar + manifest snapşotları Torii aktivləşdirmədən sonra əldə edilmişdir.Bu, SDK verərkən `docs/space-directory.md` §3.2 tələblərini əks etdirir.
sahibləri buraxılış rəyləri zamanı işarə etmək üçün bir səhifə.

## 5. Tənzimləyici/regional manifest şablonları

Yaratma qabiliyyətinin təzahürləri zamanı repo-daxili qurğulardan başlanğıc nöqtəsi kimi istifadə edin
tənzimləyicilər və ya regional nəzarətçilər üçün. Onlar icazə/inkarın əhatə dairəsini necə nümayiş etdirirlər
qaydaları və şərhçilərin gözlədiyi siyasət qeydlərini izah edin.

| Qurğu | Məqsəd | Əsas məqamlar |
|---------|---------|------------|
| `fixtures/space_directory/capability/eu_regulator_audit.manifest.json` | ESMA/ESRB audit lenti. | Tənzimləyici UAID-ləri passiv saxlamaq üçün pərakəndə köçürmələr üzrə inkar qalibiyyətləri ilə `compliance.audit::{stream_reports, request_snapshot}` üçün yalnız oxumaq üçün müavinətlər. |
| `fixtures/space_directory/capability/jp_regulator_supervision.manifest.json` | JFSA nəzarət zolağı. | `cbdc.supervision.issue_stop_order` icazəsi (Günlük pəncərə + `max_amount`) və ikili nəzarətləri tətbiq etmək üçün `force_liquidation` üzərində açıq inkar əlavə edir. |

Bu qurğuları klonlayarkən, yeniləyin:

1. `uaid` və `dataspace` identifikatorları aktivləşdirdiyiniz iştirakçı və zolağa uyğun gəlir.
2. İdarəetmə cədvəlinə əsaslanan `activation_epoch`/`expiry_epoch` pəncərələri.
3. Tənzimləyicinin siyasət istinadları ilə `notes` sahələri (MiCA məqaləsi, JFSA
   dairəvi və s.).
4. İcazə pəncərələri (`PerSlot`, `PerMinute`, `PerDay`) və isteğe bağlı
   `max_amount` qapaqları var ki, SDK-lar host ilə eyni məhdudiyyətləri tətbiq etsin.

## 6. SDK istehlakçıları üçün köçürmə qeydləriHər domen hesabı ID-lərinə istinad edən mövcud SDK inteqrasiyaları köçməlidir
yuxarıda təsvir olunan UAID mərkəzli səthlər. Təkmilləşdirmələr zamanı bu yoxlama siyahısından istifadə edin:

  hesab identifikatorları. Rust/JS/Swift/Android üçün bu, ən son versiyaya yüksəltmək deməkdir
  iş sahəsi qutuları və ya Norito bağlamalarını bərpa edin.
- **API zəngləri:** Domen əhatəli portfel sorğularını ilə əvəz edin
  `GET /v1/accounts/{uaid}/portfolio` və manifest/bağlama son nöqtələri.
  `GET /v1/accounts/{uaid}/portfolio` isteğe bağlı `asset_id` sorğusunu qəbul edir
  cüzdanların yalnız bir aktiv nümunəsinə ehtiyacı olduqda parametr. Müştəri köməkçiləri belə
  `ToriiClient.getUaidPortfolio` (JS) və Android kimi
  `SpaceDirectoryClient` artıq bu marşrutları əhatə edir; onlara sifariş verməkdən daha çox üstünlük verin
  HTTP kodu.
- **Keşləmə və telemetriya:** Xam yerinə UAID + məlumat məkanı ilə keş girişləri
  hesab identifikatorları və UAID hərfini göstərən telemetriya yaymaq üçün əməliyyatlar edə bilər
  qeydləri Space Directory sübutları ilə sıralayın.
- **Xətanın idarə edilməsi:** Yeni son nöqtələr ciddi UAID təhlil xətalarını qaytarır
  `docs/source/torii/portfolio_api.md`-də sənədləşdirilmiş; bu kodları üzə çıxarın
  dəstək qrupları təkrar addımlar atmadan problemləri həll edə bilsin.
- **Sınaq:** Yuxarıda qeyd olunan qurğuları bağlayın (üstəlik öz UAID manifestləriniz)
  Norito gediş-gəlişlərini və manifest qiymətləndirmələrini sübut etmək üçün SDK test paketlərinə daxil edin
  host tətbiqinə uyğundur.

## 7. İstinadlar- `docs/space-directory.md` — daha dərin həyat dövrü təfərrüatları ilə operator oyun kitabı.
- `docs/source/torii/portfolio_api.md` — UAID portfeli üçün REST sxemi və
  aşkar son nöqtələr.
- `crates/iroha_cli/src/space_directory.rs` - CLI tətbiqinə istinad edilir
  bu bələdçi.
- `fixtures/space_directory/capability/*.manifest.json` — tənzimləyici, pərakəndə satış və
  CBDC manifest şablonları klonlamaya hazırdır.

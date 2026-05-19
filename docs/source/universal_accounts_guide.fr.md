<!-- Auto-generated stub for French (fr) translation. Replace this content with the full translation. -->

---
lang: fr
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

# Guide de compte universel

Ce guide distille les exigences de déploiement de l'UAID (Universal Account ID) à partir de
la feuille de route Nexus et les regroupe dans une présentation pas à pas axée sur l'opérateur + le SDK.
Il couvre la dérivation de l'UAID, l'inspection du portefeuille/manifeste, les modèles de régulateur,
et les preuves qui doivent accompagner chaque manifeste de répertoire spatial de l'application `iroha
publi` run (roadmap reference: `roadmap.md:2209`).

## 1. Référence rapide de l'UAID- Les UAID sont des littéraux `uaid:<hex>` où `<hex>` est un résumé Blake2b-256 dont
  LSB est défini sur `1`. Le type canonique vit dans
  `crates/iroha_data_model/src/nexus/manifest.rs::UniversalAccountId`.
- Les enregistrements de compte (`Account` et `AccountDetails`) comportent désormais un `uaid` en option.
  champ afin que les applications puissent apprendre l'identifiant sans hachage sur mesure.
- Les politiques d'identification de fonction cachée peuvent lier des entrées normalisées arbitraires
  (numéros de téléphone, e-mails, numéros de compte, chaînes de partenaires) vers les identifiants `opaque:`
  sous un espace de noms UAID. Les pièces en chaîne sont `IdentifierPolicy`,
  `IdentifierClaimRecord` et l'index `opaque_id -> uaid`.
- Space Directory maintient une carte `World::uaid_dataspaces` qui relie chaque UAID
  aux comptes d'espace de données référencés par les manifestes actifs. Torii réutilise cela
  mappe pour les API `/portfolio` et `/uaids/*`.
- `POST /v1/accounts/onboard` publie un manifeste Space Directory par défaut pour
  l'espace de données global lorsqu'il n'en existe pas, donc l'UAID est immédiatement lié.
  Les autorités d'intégration doivent détenir le `CanPublishSpaceDirectoryManifest{dataspace=0}`.
- Tous les SDK exposent des aides pour canoniser les littéraux UAID (par exemple,
  `UaidLiteral` dans le SDK Android). Les assistants acceptent les résumés bruts de 64 hex
  (LSB=1) ou les littéraux `uaid:<hex>` et réutilisez les mêmes codecs Norito afin que le
  le résumé ne peut pas dériver d’une langue à l’autre.

## 1.1 Politiques d'identifiant masqué

Les UAID constituent désormais le point d’ancrage d’une deuxième couche d’identité :- Un `IdentifierPolicyId` global (`<kind>#<business_rule>`) définit le
  espace de noms, métadonnées d'engagement public, clé de vérification du résolveur et
  mode de normalisation canonique des entrées (`Exact`, `LowercaseTrimmed`,
  `PhoneE164`, `EmailAddress` ou `AccountNumber`).
- Une réclamation lie un identifiant `opaque:` dérivé à exactement un UAID et un
  canonique `AccountId` en vertu de cette politique, mais la chaîne n'accepte que le
  réclamation lorsqu'elle est accompagnée d'un `IdentifierResolutionReceipt` signé.
- La résolution reste un flux `resolve -> transfer`. Torii résout l'opaque
  gérer et renvoie le canonique `AccountId` ; les transferts ciblent toujours
  compte canonique, pas directement les littéraux `uaid:` ou `opaque:`.
- Les politiques peuvent désormais publier les paramètres de chiffrement d'entrée BFV via
  `PolicyCommitment.public_parameters`. Lorsqu'il est présent, Torii les annonce sur
  `GET /v1/identifier-policies`, et les clients peuvent soumettre des entrées enveloppées dans BFV
  au lieu du texte brut. Les politiques programmées enveloppent les paramètres BFV dans un
  bundle canonique `BfvProgrammedPublicParameters` qui publie également le
  public `ram_fhe_profile` ; Les anciennes charges utiles brutes BFV sont mises à niveau vers celles-ci.
  bundle canonique lorsque l’engagement est reconstruit.
- Les routes d'identification passent par le même jeton d'accès et la même limite de débit Torii.
  vérifie comme les autres points de terminaison côté application. Ils ne constituent pas un contournement de la normale
  Politique API.

## 1.2 Terminologie

La séparation des noms est intentionnelle :- `ram_lfe` est l'abstraction externe des fonctions cachées. Il couvre la politique
  enregistrement, engagements, métadonnées publiques, reçus d'exécution et
  mode de vérification.
- `BFV` est le schéma de cryptage homomorphe Brakerski/Fan-Vercauteren utilisé par
  certains backends `ram_lfe` pour évaluer les entrées chiffrées.
- `ram_fhe_profile` est une métadonnée spécifique à BFV, pas un deuxième nom pour l'ensemble
  fonctionnalité. Il décrit la machine d'exécution programmée BFV qui gère les portefeuilles et
  les vérificateurs doivent cibler le moment où une politique utilise le backend programmé.

Concrètement :

- `RamLfeProgramPolicy` et `RamLfeExecutionReceipt` sont des types de couche LFE.
- `BfvParameters`, `BfvCiphertext`, `BfvProgrammedPublicParameters` et
  `BfvRamProgramProfile` sont des types de couche FHE.
- `HiddenRamFheProgram` et `HiddenRamFheInstruction` sont des noms internes pour
  le programme BFV caché exécuté par le backend programmé. Ils restent sur le
  côté FHE car ils décrivent le mécanisme d’exécution chiffré plutôt que
  la politique externe ou l’abstraction du reçu.

## 1.3 Identité du compte par rapport aux alias

Le déploiement du compte universel ne modifie pas le modèle canonique d'identité du compte :- `AccountId` reste le sujet canonique du compte sans domaine.
- Les valeurs `AccountAlias` sont des liaisons SNS distinctes en plus de ce sujet. Un
  alias qualifié de domaine tel que `merchant@banka.paynet` et un alias racine d'espace de données
  tels que `merchant@paynet` peuvent tous deux se résoudre au même `AccountId` canonique.
- L'enregistrement canonique du compte est toujours `Account::new(AccountId)` /
  `NewAccount::new(AccountId)` ; il n'y a pas de domaine qualifié ou matérialisé par domaine
  chemin d'inscription.
- Propriété du domaine, autorisations d'alias et autres comportements à l'échelle du domaine en direct
  dans leur propre état et leurs API plutôt que sur l'identité du compte elle-même.
- La recherche de compte public suit cette division : les requêtes d'alias restent publiques, tandis que
  l'identité canonique du compte reste un pur `AccountId`.

Règle d'implémentation pour les opérateurs, les SDK et les tests : partir du canonique
`AccountId`, puis ajoutez des baux d'alias, des autorisations d'espace de données/domaine et tout autre
État appartenant au domaine séparément. Ne synthétisez pas un faux compte dérivé d'un pseudonyme
ou attendez-vous à un champ de domaine lié sur les enregistrements de compte simplement parce qu'un alias ou
la route transporte un segment de domaine.

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

## 2. Dérivation et vérification des UAID

Il existe trois méthodes prises en charge pour obtenir un UAID :

1. **Lisez-le à partir des modèles d'état mondial ou du SDK.** Tout `Account`/`AccountDetails`
   La charge utile interrogée via Torii a désormais le champ `uaid` renseigné lorsque le
   le participant a opté pour des comptes universels.
2. **Interrogez les registres UAID.** Torii expose
   `GET /v1/space-directory/uaids/{uaid}` qui renvoie les liaisons de l'espace de données
   et les métadonnées du manifeste que l'hôte Space Directory conserve (voir
   `docs/space-directory.md` §3 pour les échantillons de charge utile).
3. **Dérivez-le de manière déterministe.** Lors du démarrage de nouveaux UAID hors ligne, hachez
   la graine canonique du participant avec Blake2b-256 et préfixez le résultat avec
   `uaid:`. L'extrait ci-dessous reflète l'assistant documenté dans
   `docs/space-directory.md` §3.3 :

   ```python
   import hashlib
   seed = b"participant@example"  # canonical address/domain seed
   digest = bytearray(hashlib.blake2b(seed, digest_size=32).digest())
   digest[-1] |= 1
   print(f"uaid:{digest.hex()}")
   ```Stockez toujours le littéral en minuscules et normalisez les espaces avant le hachage.
Les assistants CLI tels que `iroha app space-directory manifest scaffold` et Android
L'analyseur `UaidLiteral` applique les mêmes règles de découpage afin que les examens de gouvernance puissent
vérifier les valeurs sans scripts ad hoc.

## 3. Inspection des avoirs et des manifestes de l'UAID

L'agrégateur de portefeuille déterministe dans `iroha_core::nexus::portfolio`
fait apparaître chaque paire actif/espace de données qui fait référence à l’UAID. Opérateurs et SDK
peut consommer les données via les surfaces suivantes :

| Surfaces | Utilisation |
|---------|-------|
| `GET /v1/accounts/{uaid}/portfolio` | Renvoie l'espace de données → actif → résumés de solde ; décrit dans `docs/source/torii/portfolio_api.md`. |
| `GET /v1/space-directory/uaids/{uaid}` | Répertorie les ID d'espace de données + les littéraux de compte liés à l'UAID. |
| `GET /v1/space-directory/uaids/{uaid}/manifests` | Fournit l’historique complet `AssetPermissionManifest` pour les audits. |
| `iroha app space-directory bindings fetch --uaid <literal>` | Raccourci CLI qui encapsule le point de terminaison des liaisons et écrit éventuellement le JSON sur le disque (`--json-out`). |
| `iroha app space-directory manifest fetch --uaid <literal> --json-out <path>` | Récupère le bundle JSON manifeste pour les packs de preuves. |

Exemple de session CLI (URL Torii configurée via `torii_api_url` dans `iroha.json`) :

```bash
iroha app space-directory bindings fetch \
  --uaid uaid:86e8ee39a3908460a0f4ee257bb25f340cd5b5de72735e9adefe07d5ef4bb0df \
  --json-out artifacts/uaid86/bindings.json

iroha app space-directory manifest fetch \
  --uaid uaid:86e8ee39a3908460a0f4ee257bb25f340cd5b5de72735e9adefe07d5ef4bb0df \
  --json-out artifacts/uaid86/manifests.json
```

Stockez les instantanés JSON avec le hachage du manifeste utilisé lors des révisions ; le
L'observateur de Space Directory reconstruit la carte `uaid_dataspaces` chaque fois qu'il se manifeste
activer, expirer ou révoquer, ces instantanés constituent donc le moyen le plus rapide de prouver
quelles liaisons étaient actives à une époque donnée.## 4. La capacité de publication se manifeste par des preuves

Utilisez le flux CLI ci-dessous chaque fois qu’une nouvelle allocation est déployée. Chaque étape doit
atterrissent dans l’ensemble des preuves enregistrées pour l’approbation de la gouvernance.

1. **Encodez le manifeste JSON** afin que les réviseurs voient le hachage déterministe avant
   soumission :

   ```bash
   iroha app space-directory manifest encode \
     --json fixtures/space_directory/capability/eu_regulator_audit.manifest.json \
     --out artifacts/eu_regulator_audit.manifest.to \
     --hash-out artifacts/eu_regulator_audit.manifest.hash
   ```

2. **Publiez l'allocation** en utilisant soit la charge utile Norito (`--manifest`), soit
   la description JSON (`--manifest-json`). Enregistrez le reçu Torii/CLI plus
   le hachage de l'instruction `PublishSpaceDirectoryManifest` :

   ```bash
   iroha app space-directory manifest publish \
     --manifest artifacts/eu_regulator_audit.manifest.to \
     --reason "ESMA wave 2 onboarding"
   ```

3. **Capturez les preuves SpaceDirectoryEvent.** Abonnez-vous à
   `SpaceDirectoryEvent::ManifestActivated` et inclure la charge utile de l'événement dans
   le bundle afin que les auditeurs puissent confirmer quand le changement a eu lieu.

4. **Générer un bundle d'audit** liant le manifeste à son profil d'espace de données et
   crochets de télémétrie :

   ```bash
   iroha app space-directory manifest audit-bundle \
     --manifest artifacts/eu_regulator_audit.manifest.to \
     --profile fixtures/space_directory/profile/cbdc_lane_profile.json \
     --out-dir artifacts/eu_regulator_audit_bundle
   ```

5. **Vérifiez les liaisons via Torii** (`bindings fetch` et `manifests fetch`) et
   archivez ces fichiers JSON avec le hash + bundle ci-dessus.

Liste de contrôle des preuves :

- [ ] Hachage du manifeste (`*.manifest.hash`) signé par l'approbateur du changement.
- [ ] Réception CLI/Torii pour l'appel de publication (stdout ou artefact `--json-out`).
- [ ] Activation de la charge utile `SpaceDirectoryEvent` prouvant.
- [ ] Répertoire du bundle d'audit avec profil d'espace de données, hooks et copie du manifeste.
- [ ] Liaisons + instantanés de manifeste récupérés à partir de la post-activation Torii.Cela reflète les exigences de `docs/space-directory.md` §3.2 tout en donnant au SDK
les propriétaires une seule page vers laquelle pointer lors des révisions de versions.

## 5. Modèles de manifestes régulateurs/régionaux

Utilisez les éléments du dépôt comme points de départ lors de la création de manifestes de capacités
pour les régulateurs ou les superviseurs régionaux. Ils montrent comment autoriser/refuser la portée
règles et expliquer les notes de politique attendues par les évaluateurs.

| Luminaire | Objectif | Faits saillants |
|---------|---------|------------|
| `fixtures/space_directory/capability/eu_regulator_audit.manifest.json` | Flux d’audit ESMA/ESRB. | Allocations en lecture seule pour `compliance.audit::{stream_reports, request_snapshot}` avec refus de gains sur les transferts de détail pour maintenir les UAID du régulateur passifs. |
| `fixtures/space_directory/capability/jp_regulator_supervision.manifest.json` | Voie de supervision JFSA. | Ajoute une allocation `cbdc.supervision.issue_stop_order` plafonnée (fenêtre par jour + `max_amount`) et un refus explicite sur `force_liquidation` pour appliquer des contrôles doubles. |

Lors du clonage de ces appareils, mettez à jour :

1. Les identifiants `uaid` et `dataspace` correspondent au participant et à la voie que vous activez.
2. Fenêtres `activation_epoch`/`expiry_epoch` basées sur le planning de gouvernance.
3. Champs `notes` avec les références politiques du régulateur (article MiCA, JFSA
   circulaire, etc.).
4. Fenêtres d'allocation (`PerSlot`, `PerMinute`, `PerDay`) et en option
   `max_amount` plafonne afin que les SDK appliquent les mêmes limites que l'hôte.

## 6. Notes de migration pour les consommateurs du SDKLes intégrations SDK existantes qui font référence aux ID de compte par domaine doivent migrer vers
les surfaces centrées sur l’UAID décrites ci-dessus. Utilisez cette liste de contrôle lors des mises à niveau :

  identifiants de compte. Pour Rust/JS/Swift/Android, cela signifie une mise à niveau vers la dernière version.
  caisses d'espace de travail ou régénération des liaisons Norito.
- **Appels API :** Remplacez les requêtes de portefeuille portant sur le domaine par
  `GET /v1/accounts/{uaid}/portfolio` et les points de terminaison du manifeste/liaison.
  `GET /v1/accounts/{uaid}/portfolio` accepte une requête `asset_id` facultative
  paramètre lorsque les portefeuilles n’ont besoin que d’une seule instance d’actif. Aides aux clients telles que
  comme `ToriiClient.getUaidPortfolio` (JS) et Android
  `SpaceDirectoryClient` enveloppe déjà ces routes ; préférez-les au sur mesure
  Code HTTP.
- **Mise en cache et télémétrie :** Entrées de cache par UAID + espace de données au lieu de brut
  identifiants de compte et émettent une télémétrie montrant le littéral UAID afin que les opérations puissent
  alignez les journaux avec les preuves de Space Directory.
- **Gestion des erreurs :** Les nouveaux points de terminaison renvoient les erreurs d'analyse stricte de l'UAID
  documenté dans `docs/source/torii/portfolio_api.md` ; faire apparaître ces codes
  textuellement afin que les équipes d'assistance puissent trier les problèmes sans étapes de repro.
- **Test :** Câblez les appareils mentionnés ci-dessus (plus vos propres manifestes UAID)
  dans les suites de tests du SDK pour prouver les allers-retours Norito et les évaluations de manifeste
  correspondre à l’implémentation de l’hôte.

## 7. Références- `docs/space-directory.md` — manuel de l'opérateur avec des détails plus détaillés sur le cycle de vie.
- `docs/source/torii/portfolio_api.md` — Schéma REST pour le portefeuille UAID et
  points de terminaison manifestes.
- `crates/iroha_cli/src/space_directory.rs` — Implémentation CLI référencée dans
  ce guide.
- `fixtures/space_directory/capability/*.manifest.json` — régulateur, vente au détail et
  Modèles de manifeste CBDC prêts pour le clonage.

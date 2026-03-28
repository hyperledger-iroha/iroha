---
lang: fr
direction: ltr
source: docs/source/universal_accounts_guide.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f972f8f82b7f4e89c1d48b0dbbc6eb5b73303e2fab0f580ab21e63990ba03af8
source_last_modified: "2026-03-27T19:05:17.617064+00:00"
translation_last_reviewed: 2026-03-28
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

Les UAID constituent désormais le point d’ancrage d’une deuxième couche d’identité :

- Un `IdentifierPolicyId` global (`<kind>#<business_rule>`) définit le
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

La séparation des noms est intentionnelle :

- `ram_lfe` est l'abstraction externe des fonctions cachées. Il couvre la politique
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

Le déploiement du compte universel ne modifie pas le modèle canonique d'identité du compte :

- `AccountId` reste le sujet canonique du compte sans domaine.
- `ScopedAccountId { account, domain }` est un contexte de domaine explicite pour les vues ou
  les enregistrements qui matérialisent un lien de domaine. Ce n'est pas une seconde canonique
  identité.
- Les alias SNS/compte sont des liaisons distinctes en plus de ce sujet. Un
  alias qualifié de domaine tel que `merchant@hbl.sbp` et un alias racine d'espace de données
  tels que `merchant@sbp` peuvent tous deux se résoudre au même `AccountId` canonique.
- `linked_domains` sur les enregistrements de compte stockés est un état dérivé du
  index de domaine de compte. Il décrit les liens actuellement matérialisés pour cela
  sujet ; il ne fait pas partie de l'identifiant canonique.

Règle d'implémentation pour les opérateurs, les SDK et les tests : partir du canonique
`AccountId`, puis ajoutez des baux d'alias, des autorisations d'espace de données/domaine et des autorisations explicites
liens de domaine séparément. Ne synthétisez pas un faux canonique à l'échelle du domaine
compte simplement parce qu’un alias ou une route transporte un segment de domaine.

Itinéraires Torii actuels :

| Itinéraire | Objectif |
|-------|--------------|
| `GET /v1/ram-lfe/program-policies` | Répertorie les stratégies de programme RAM-LFE actives et inactives ainsi que leurs métadonnées d'exécution publiques, y compris les paramètres BFV `input_encryption` facultatifs et le backend programmé `ram_fhe_profile`. |
| `POST /v1/ram-lfe/programs/{program_id}/execute` | Accepte exactement l’un des `{ input_hex }` ou `{ encrypted_input }` et renvoie le `RamLfeExecutionReceipt` sans état plus `{ output_hex, output_hash, receipt_hash }` pour le programme sélectionné. Le runtime Torii actuel émet des reçus pour le backend BFV programmé. |
| `POST /v1/ram-lfe/receipts/verify` | Valide sans état un `RamLfeExecutionReceipt` par rapport à la politique du programme en chaîne publiée et vérifie éventuellement qu'un `output_hex` fourni par l'appelant correspond au reçu `output_hash`. |
| `GET /v1/identifier-policies` | Répertorie les espaces de noms de stratégie de fonction cachée actifs et inactifs ainsi que leurs métadonnées publiques, y compris les paramètres BFV `input_encryption` facultatifs, le mode `normalization` requis pour l'entrée chiffrée côté client et `ram_fhe_profile` pour les stratégies BFV programmées. |
| `POST /v1/accounts/{account_id}/identifiers/claim-receipt` | Accepte exactement l’un des `{ input }` ou `{ encrypted_input }`. Le texte brut `input` est normalisé côté serveur ; BFV `encrypted_input` doit déjà être normalisé selon le mode de stratégie publié. Le point de terminaison dérive ensuite le handle `opaque:` et renvoie un reçu signé que `ClaimIdentifier` peut soumettre en chaîne, comprenant à la fois le `signature_payload_hex` brut et le `signature_payload` analysé. || `POST /v1/identifiers/resolve` | Accepte exactement l’un des `{ input }` ou `{ encrypted_input }`. Le texte en clair `input` est normalisé côté serveur ; BFV `encrypted_input` doit déjà être normalisé selon le mode de stratégie publié. Le point de terminaison résout l'identifiant en `{ opaque_id, receipt_hash, uaid, account_id, signature }` lorsqu'une revendication active existe et renvoie également la charge utile signée canonique sous le nom `{ signature_payload_hex, signature_payload }`. |
| `GET /v1/identifiers/receipts/{receipt_hash}` | Recherche le `IdentifierClaimRecord` persistant lié à un hachage de reçu déterministe afin que les opérateurs et les SDK puissent auditer la propriété des revendications ou diagnostiquer les échecs de relecture/incompatibilité sans analyser l'index complet de l'identifiant. |

Le runtime d'exécution en cours de Torii est configuré sous
`torii.ram_lfe.programs[*]`, saisi par `program_id`. L'identifiant route maintenant
réutiliser ce même environnement d'exécution RAM-LFE au lieu d'un `identifier_resolver` distinct
surface de configuration.

Prise en charge actuelle du SDK :

- `normalizeIdentifierInput(value, normalization)` correspond au Rust
  canonicaliseurs pour `exact`, `lowercase_trimmed`, `phone_e164`,
  `email_address` et `account_number`.
- `ToriiClient.listIdentifierPolicies()` répertorie les métadonnées de stratégie, y compris BFV
  métadonnées de chiffrement d'entrée lorsque la politique les publie, plus un
  Objet paramètre BFV via `input_encryption_public_parameters_decoded`.
  Les politiques programmées exposent également le `ram_fhe_profile` décodé. Ce champ est
  intentionnellement à portée BFV : il permet aux portefeuilles de vérifier le registre attendu
  nombre, nombre de voies, mode de canonisation et module de texte chiffré minimum pour
  le backend FHE programmé avant de chiffrer l’entrée côté client.
- `getIdentifierBfvPublicParameters(policy)` et
  Aide `buildIdentifierRequestForPolicy(policy, { input | encryptedInput })`
  Les appelants JS consomment les métadonnées BFV publiées et créent des requêtes tenant compte des règles.
  organismes sans réimplémenter les règles d’identification des politiques et de normalisation.
- `encryptIdentifierInputForPolicy(policy, input, { seedHex? })` et
  `buildIdentifierRequestForPolicy(policy, { input, encrypt: true })` laisse maintenant
  Les portefeuilles JS construisent localement l'enveloppe de texte chiffré complète BFV Norito à partir de
  paramètres de politique publiés au lieu d'envoyer un texte chiffré hexadécimal prédéfini.
- `ToriiClient.resolveIdentifier({ policyId, input | encryptedInput })`
  résout un identifiant masqué et renvoie la charge utile du reçu signé,
  y compris `receipt_hash`, `signature_payload_hex` et
  `signature_payload`.
- `ToriiClient.issueIdentifierClaimReceipt(accountId, {policyId, input |
  EncryptedInput })` issues the signed receipt needed by `ClaimIdentifier`.
- `verifyIdentifierResolutionReceipt(receipt, policy)` vérifie le retour
  réception contre la clé de résolution de politique du côté client, et`ToriiClient.getIdentifierClaimByReceiptHash(receiptHash)` récupère le
  enregistrement de réclamation persistant pour les flux d’audit/débogage ultérieurs.
- `IrohaSwift.ToriiClient` expose désormais `listIdentifierPolicies()`,
  `resolveIdentifier(policyId:input:encryptedInputHex:)`,
  `issueIdentifierClaimReceipt(accountId:policyId:input:encryptedInputHex:)`,
  et `getIdentifierClaimByReceiptHash(_)`, plus
  `ToriiIdentifierNormalization` pour le même téléphone/e-mail/numéro de compte
  modes de canonisation.
- `ToriiIdentifierLookupRequest` et le
  `ToriiIdentifierPolicySummary.plaintextRequest(...)` /
  Les assistants `.encryptedRequest(...)` fournissent la surface de requête Swift typée pour
  résoudre et réclamer-réception des appels, et les politiques Swift peuvent désormais dériver le BFV
  texte chiffré localement via `encryptInput(...)` / `encryptedRequest(input:...)`.
- `ToriiIdentifierResolutionReceipt.verifySignature(using:)` valide cela
  les champs de réception de niveau supérieur correspondent à la charge utile signée et vérifie le
  signature du résolveur côté client avant la soumission.
- `HttpClientTransport` dans le SDK Android expose désormais
  `listIdentifierPolicies()`, `resolveIdentifier(policyId, entrée,
  EncryptedInputHex)`, `issueIdentifierClaimReceipt(accountId, PolicyId,
  entrée, chiffréInputHex)`, and `getIdentifierClaimByReceiptHash(...)`,
  plus `IdentifierNormalization` pour les mêmes règles de canonisation.
- `IdentifierResolveRequest` et le
  `IdentifierPolicySummary.plaintextRequest(...)` /
  Les assistants `.encryptedRequest(...)` fournissent la surface de requête Android tapée,
  tandis que `IdentifierPolicySummary.encryptInput(...)` /
  `.encryptedRequestFromInput(...)` dérive l'enveloppe de texte chiffré BFV
  localement à partir des paramètres de politique publiés.
  `IdentifierResolutionReceipt.verifySignature(policy)` vérifie le retour
  signature du résolveur côté client.

Jeu d'instructions actuel :-`RegisterIdentifierPolicy`
- `ActivateIdentifierPolicy`
- `ClaimIdentifier` (relié au reçu ; les réclamations brutes `opaque_id` sont rejetées)
- `RevokeIdentifier`

Trois backends existent désormais dans `iroha_crypto::ram_lfe` :

- le PRF `HKDF-SHA3-512` lié à un engagement historique, et
- un évaluateur affine secret soutenu par BFV qui consomme un identifiant crypté par BFV
  emplacements directement. Lorsque `iroha_crypto` est construit avec la valeur par défaut
  Fonctionnalité `bfv-accel`, la multiplication en anneau BFV utilise une méthode déterministe exacte
  Backend CRT-NTT en interne ; la désactivation de cette fonctionnalité revient au
  parcours scolaire scalaire avec des sorties identiques, et
- un évaluateur programmé secret soutenu par BFV qui dérive un
  Trace d'exécution de type RAM sur les registres chiffrés et la mémoire de texte chiffré
  voies avant de dériver l’identifiant opaque et le hachage du reçu. Le programmé
  le backend nécessite désormais un plancher de module BFV plus fort que le chemin affine, et
  ses paramètres publics sont publiés dans un bundle canonique qui comprend le
  Profil d'exécution RAM-FHE consommé par les portefeuilles et les vérificateurs.

Ici, BFV désigne le programme Brakerski/Fan-Vercauteren FHE mis en œuvre en
`crates/iroha_crypto/src/fhe_bfv.rs`. C'est le mécanisme d'exécution crypté
utilisé par les backends affines et programmés, pas le nom du serveur caché externe
abstraction des fonctions.Torii utilise le backend publié par l'engagement de stratégie. Lorsque le backend BFV
est actif, les requêtes en texte clair sont normalisées puis chiffrées côté serveur avant
évaluation. Les requêtes BFV `encrypted_input` pour le backend affine sont évaluées
directement et doit déjà être normalisé côté client ; le backend programmé
canonise l'entrée chiffrée sur le BFV déterministe du résolveur
enveloppe avant d'exécuter le programme RAM secret afin que les hachages de reçu restent
stable à travers des textes chiffrés sémantiquement équivalents.

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
   digest = hashlib.blake2b(seed, digest_size=32).hexdigest()
   print(f"uaid:{digest}")
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
quelles liaisons étaient actives à une époque donnée.

## 4. La capacité de publication se manifeste par des preuves

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
   `SpaceDirectoryEvent::ManifestActivated` et incluez la charge utile de l'événement dans
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

1. Identifiants `uaid` et `dataspace` correspondant au participant et à la voie que vous activez.
2. Fenêtres `activation_epoch`/`expiry_epoch` basées sur le calendrier de gouvernance.
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
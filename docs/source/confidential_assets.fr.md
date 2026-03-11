---
lang: fr
direction: ltr
source: docs/source/confidential_assets.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 969ffd4cee6ee4880d5f754fb36adaf30dde532a29e4c6397cf0f358438bb57e
source_last_modified: "2026-01-22T15:38:30.657840+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
SPDX-License-Identifier: Apache-2.0
-->
# Actifs confidentiels et conception de transfert ZK

## Motivation
- Fournir des flux d'actifs protégés par opt-in afin que les domaines puissent préserver la confidentialité des transactions sans altérer la circulation transparente.
- Fournir aux auditeurs et opérateurs des contrôles de cycle de vie (activation, rotation, révocation) des circuits et des paramètres cryptographiques.

## Modèle de menace
- Les validateurs sont honnêtes mais curieux : ils exécutent fidèlement le consensus mais tentent d'inspecter le grand livre/l'état.
- Les observateurs du réseau voient les données bloquées et les transactions bavardes ; aucune hypothèse de chaînes de potins privées.
- Hors de portée : analyse du trafic hors grand livre, adversaires quantiques (suivis séparément dans la feuille de route PQ), attaques contre la disponibilité du grand livre.

## Aperçu de la conception
- Les actifs peuvent déclarer un *pool protégé* en plus des soldes transparents existants ; la circulation protégée est représentée via des engagements cryptographiques.
- Les notes encapsulent `(asset_id, amount, recipient_view_key, blinding, rho)` avec :
  - Engagement : `Comm = Pedersen(params_id || asset_id || amount || recipient_view_key || blinding)`.
  - Annulateur : `Null = Poseidon(domain_sep || nk || rho || asset_id || chain_id)`, indépendant de l'ordre des notes.
  - Charge utile cryptée : `enc_payload = AEAD_XChaCha20Poly1305(ephemeral_shared_key, note_plaintext)`.
- Les transactions transportent des charges utiles `ConfidentialTransfer` codées en Norito contenant :
  - Entrées publiques : ancre Merkle, annulateurs, nouveaux engagements, identifiant d'actif, version du circuit.
  - Charges utiles cryptées pour les destinataires et les auditeurs facultatifs.
  - Preuve sans connaissance attestant de la conservation de la valeur, de la propriété et de l'autorisation.
- Vérifier que les clés et les jeux de paramètres sont contrôlés via des registres sur grand livre avec des fenêtres d'activation ; les nœuds refusent de valider les preuves qui font référence à des entrées inconnues ou révoquées.
- Les en-têtes de consensus s'engagent sur le résumé des fonctionnalités confidentielles actives, de sorte que les blocs ne sont acceptés que lorsque l'état du registre et celui des paramètres correspondent.
- La construction de preuve utilise une pile Halo2 (Plonkish) sans configuration fiable ; Groth16 ou d'autres variantes de SNARK ne sont intentionnellement pas prises en charge dans la v1.

### Luminaires déterministes

Les enveloppes de mémos confidentiels sont désormais livrées avec un élément canonique à l'adresse `fixtures/confidential/encrypted_payload_v1.json`. L'ensemble de données capture une enveloppe v1 positive ainsi que des échantillons négatifs mal formés afin que les SDK puissent affirmer la parité d'analyse. Les tests du modèle de données Rust (`crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs`) et la suite Swift (`IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift`) chargent tous deux le luminaire directement, garantissant que l'encodage Norito, les surfaces d'erreur et la couverture de régression restent alignés à mesure que le codec évolue.

Les SDK Swift peuvent désormais émettre des instructions de bouclier sans colle JSON sur mesure : construisez un
`ShieldRequest` avec l'engagement de note de 32 octets, la charge utile chiffrée et les métadonnées de débit,
puis appelez `IrohaSDK.submit(shield:keypair:)` (ou `submitAndWait`) pour signer et relayer le
transaction sur `/v1/pipeline/transactions`. L'assistant valide les durées d'engagement,
enfile `ConfidentialEncryptedPayload` dans l'encodeur Norito et reflète le `zk::Shield`
disposition décrite ci-dessous afin que les portefeuilles restent en phase avec Rust.## Engagements de consensus et contrôle des capacités
- Les en-têtes de bloc exposent `conf_features = { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }` ; le résumé participe au hachage de consensus et doit être égal à la vue du registre local pour l'acceptation du bloc.
- La gouvernance peut organiser des mises à niveau en programmant `next_conf_features` avec un futur `activation_height` ; jusqu'à cette hauteur, les producteurs de blocs doivent continuer à émettre le résumé précédent.
- Les nœuds de validation DOIVENT fonctionner avec `confidential.enabled = true` et `assume_valid = false`. Les contrôles de démarrage refusent de rejoindre l'ensemble du validateur si l'une des conditions échoue ou si le `conf_features` local diverge.
- Les métadonnées de négociation P2P incluent désormais `{ enabled, assume_valid, conf_features }`. Les pairs annonçant des fonctionnalités non prises en charge sont rejetés avec `HandshakeConfidentialMismatch` et n’entrent jamais dans une rotation consensuelle.
- Les observateurs non-validateurs peuvent définir `assume_valid = true` ; ils appliquent aveuglément des deltas confidentiels mais n’influencent pas la sécurité du consensus.## Politiques relatives aux actifs
- Chaque définition d'actif porte un `AssetConfidentialPolicy` défini par le créateur ou via la gouvernance :
  - `TransparentOnly` : mode par défaut ; seules les instructions transparentes (`MintAsset`, `TransferAsset`, etc.) sont autorisées et les opérations protégées sont rejetées.
  - `ShieldedOnly` : toutes les émissions et transferts doivent faire l'objet d'instructions confidentielles ; `RevealConfidential` est interdit, les soldes ne sont donc jamais rendus publics.
  - `Convertible` : les détenteurs peuvent déplacer la valeur entre les représentations transparentes et blindées en utilisant les instructions de rampe d'entrée/sortie ci-dessous.
- Les politiques suivent un FSM contraint pour éviter l'échouage des fonds :
  - `TransparentOnly → Convertible` (activation immédiate du pool blindé).
  - `TransparentOnly → ShieldedOnly` (nécessite une fenêtre de transition et de conversion en attente).
  - `Convertible → ShieldedOnly` (délai minimum forcé).
  - `ShieldedOnly → Convertible` (plan de migration requis pour que les billets protégés restent utilisables).
  - `ShieldedOnly → TransparentOnly` n'est pas autorisé à moins que le pool protégé soit vide ou que la gouvernance code une migration qui libère les billets en circulation.
- Les instructions de gouvernance définissent `pending_transition { new_mode, effective_height, previous_mode, transition_id, conversion_window }` via l'ISI `ScheduleConfidentialPolicyTransition` et peuvent abandonner les modifications planifiées avec `CancelConfidentialPolicyTransition`. La validation Mempool garantit qu'aucune transaction ne chevauche la hauteur de transition et que l'inclusion échoue de manière déterministe si une vérification de politique change au milieu du bloc.
- Les transitions en attente sont appliquées automatiquement lorsqu'un nouveau bloc s'ouvre : une fois que la hauteur du bloc entre dans la fenêtre de conversion (pour les mises à niveau `ShieldedOnly`) ou atteint le `effective_height` programmé, le runtime met à jour `AssetConfidentialPolicy`, actualise les métadonnées `zk.policy` et efface l'entrée en attente. Si l’approvisionnement transparent demeure lorsqu’une transition `ShieldedOnly` arrive à échéance, le moteur d’exécution abandonne la modification et enregistre un avertissement, laissant le mode précédent intact.
- Les boutons de configuration `policy_transition_delay_blocks` et `policy_transition_window_blocks` appliquent des délais de préavis et de grâce minimum pour permettre aux portefeuilles de convertir les notes autour du commutateur.
- `pending_transition.transition_id` sert également de handle d'audit ; la gouvernance doit le citer lors de la finalisation ou de l’annulation des transitions afin que les opérateurs puissent corréler les rapports d’entrée/de sortie.
- `policy_transition_window_blocks` est par défaut de 720 (≈12 heures avec un temps de bloc de 60 s). Les nœuds bloquent les demandes de gouvernance qui tentent d’obtenir un préavis plus court.
- Les manifestes Genesis et les flux CLI font apparaître les politiques actuelles et en attente. La logique d'admission lit la stratégie au moment de l'exécution pour confirmer que chaque instruction confidentielle est autorisée.
- Liste de contrôle de migration — voir « Séquençage de la migration » ci-dessous pour le plan de mise à niveau par étapes suivi par Milestone M0.

#### Surveillance des transitions via ToriiLes portefeuilles et les auditeurs interrogent `GET /v1/confidential/assets/{definition_id}/transitions` pour inspecter
le `AssetConfidentialPolicy` actif. La charge utile JSON inclut toujours le code canonique
l'identifiant de l'actif, la dernière hauteur de bloc observée, le `current_mode` de la politique, le mode qui est
efficace à cette hauteur (les fenêtres de conversion signalent temporairement `Convertible`), et le
Identificateurs de paramètres `vk_set_hash`/Poseidon/Pedersen attendus. Les consommateurs du SDK Swift peuvent appeler
`ToriiClient.getConfidentialAssetPolicy` pour recevoir les mêmes données que les DTO tapés sans
décodage manuscrit. Lorsqu’une transition de gouvernance est en attente, la réponse intègre également :

- `transition_id` — handle d'audit renvoyé par `ScheduleConfidentialPolicyTransition`.
-`previous_mode`/`new_mode`.
-`effective_height`.
- `conversion_window` et le dérivé `window_open_height` (le bloc où les portefeuilles doivent
  commencer la conversion pour les basculements ShieldedOnly).

Exemple de réponse :

```json
{
  "asset_id": "rose#wonderland",
  "block_height": 4217,
  "current_mode": "Convertible",
  "effective_mode": "Convertible",
  "vk_set_hash": "8D7A4B0A95AB1C33F04944F5D332F9A829CEB10FB0D0797E2D25AEFBAAF1155D",
  "poseidon_params_id": 7,
  "pedersen_params_id": 11,
  "pending_transition": {
    "transition_id": "BF2C6F9A4E9DF389B6F7E5E6B5487B39AE00D2A4B7C0FBF2C9FEF6D0A961C8ED",
    "previous_mode": "Convertible",
    "new_mode": "ShieldedOnly",
    "effective_height": 5000,
    "conversion_window": 720,
    "window_open_height": 4280
  }
}
```

Une réponse `404` indique qu’aucune définition d’actif correspondante n’existe. Lorsqu'aucune transition n'est
planifié, le champ `pending_transition` est `null`.

### Machine à états de stratégie| Mode actuel | Mode suivant | Conditions préalables | Manutention en hauteur effective | Remarques |
|----------------------------------------------------|-------------------------------------------------------------------------------------------------|----------------------------------------------------------------------------------------------------------------------------------|-----------------------------------------------------------------------------------------------|
| Transparent uniquement | Cabriolet | Governance a activé les entrées de registre des vérificateurs/paramètres. Soumettez `ScheduleConfidentialPolicyTransition` avec `effective_height ≥ current_height + policy_transition_delay_blocks`. | La transition s'exécute exactement à `effective_height` ; la piscine protégée devient disponible immédiatement.                   | Chemin par défaut pour activer la confidentialité tout en gardant des flux transparents.               |
| Transparent uniquement | Blindé uniquement | Comme ci-dessus, plus `policy_transition_window_blocks ≥ 1`.                                                         | Le runtime entre automatiquement `Convertible` à `effective_height - policy_transition_window_blocks` ; passe à `ShieldedOnly` à `effective_height`. | Fournit une fenêtre de conversion déterministe avant que les instructions transparentes ne soient désactivées.   |
| Cabriolet | Blindé uniquement | Transition programmée avec `effective_height ≥ current_height + policy_transition_delay_blocks`. La gouvernance DEVRAIT certifier (`transparent_supply == 0`) via des métadonnées d'audit ; le runtime applique cela lors du basculement. | Sémantique de fenêtre identique à celle ci-dessus. Si l’offre transparente est non nulle à `effective_height`, la transition s’interrompt avec `PolicyTransitionPrerequisiteFailed`. | Verrouille l'actif dans une circulation entièrement confidentielle.                                     |
| Blindé uniquement | Cabriolet | Transition programmée ; pas de retrait d'urgence actif (`withdraw_height` non activé).                                    | L'état s'inverse à `effective_height` ; révéler que les rampes rouvrent tandis que les notes protégées restent valides.                           | Utilisé pour les fenêtres de maintenance ou les examens des auditeurs.                                          |
| Blindé uniquement | Transparent uniquement | La gouvernance doit prouver `shielded_supply == 0` ou mettre en place un plan `EmergencyUnshield` signé (signatures des auditeurs requises). | Le runtime ouvre une fenêtre `Convertible` avant `effective_height` ; en hauteur, les instructions confidentielles échouent et l'actif revient en mode transparent uniquement. | Sortie en dernier recours. La transition s'annule automatiquement si une note confidentielle passe pendant la fenêtre. |
| N'importe quel | Identique à l'actuel | `CancelConfidentialPolicyTransition` efface la modification en attente.                                                        | `pending_transition` supprimé immédiatement.                                                                          | Maintient le statu quo ; montré par souci d’exhaustivité.                                             |Les transitions non répertoriées ci-dessus sont rejetées lors de la soumission de la gouvernance. Le runtime vérifie les prérequis juste avant d'appliquer une transition planifiée ; l'échec des conditions préalables repousse l'actif à son mode précédent et émet `PolicyTransitionPrerequisiteFailed` via la télémétrie et les événements de blocage.

### Séquençage de la migration

2. **Étapez la transition :** Soumettez `ScheduleConfidentialPolicyTransition` avec un `effective_height` qui respecte `policy_transition_delay_blocks`. Lorsque vous passez à `ShieldedOnly`, spécifiez une fenêtre de conversion (`window ≥ policy_transition_window_blocks`).
3. **Publiez les conseils de l'opérateur :** Enregistrez le `transition_id` renvoyé et diffusez un runbook d'accès/de sortie. Les portefeuilles et les auditeurs s'abonnent à `/v1/confidential/assets/{id}/transitions` pour connaître la hauteur d'ouverture de la fenêtre.
4. **Application de la fenêtre :** Lorsque la fenêtre s'ouvre, le moteur d'exécution fait passer la stratégie à `Convertible`, émet `PolicyTransitionWindowOpened { transition_id }` et commence à rejeter les demandes de gouvernance conflictuelles.
5. **Finaliser ou abandonner :** À `effective_height`, le runtime vérifie les prérequis de la transition (zéro approvisionnement transparent, pas de retraits d'urgence, etc.). Une réussite fait basculer la stratégie vers le mode demandé ; l'échec émet `PolicyTransitionPrerequisiteFailed`, efface la transition en attente et laisse la stratégie inchangée.
6. **Mises à niveau du schéma :** Après une transition réussie, la gouvernance remplace la version du schéma d'actif (par exemple, `asset_definition.v2`) et les outils CLI nécessitent `confidential_policy` lors de la sérialisation des manifestes. Les documents de mise à niveau de Genesis demandent aux opérateurs d'ajouter des paramètres de stratégie et des empreintes de registre avant de redémarrer les validateurs.

Les nouveaux réseaux qui commencent par activer la confidentialité codent la politique souhaitée directement dans Genesis. They still follow the checklist above when changing modes post-launch so that conversion windows remain deterministic and wallets have time to adjust.

### Norito Versionnement et activation du manifeste- Les manifestes Genesis DOIVENT inclure un `SetParameter` pour la clé `confidential_registry_root` personnalisée. La charge utile est Norito JSON correspondant à `ConfidentialRegistryMeta { vk_set_hash: Option<String> }` : omettez le champ (`null`) lorsqu'aucune entrée du vérificateur n'est active, sinon fournissez une chaîne hexadécimale de 32 octets (`0x…`) égale au hachage produit par `compute_vk_set_hash` sur les instructions du vérificateur fournies dans le manifeste. Les nœuds refusent de démarrer si le paramètre est manquant ou si le hachage n'est pas d'accord avec les écritures codées dans le registre.
- Le `ConfidentialFeatureDigest::conf_rules_version` en ligne intègre la version de mise en page du manifeste. Pour les réseaux v1, il DOIT rester `Some(1)` et égal à `iroha_config::parameters::defaults::confidential::RULES_VERSION`. Lorsque l'ensemble de règles évolue, modifiez la constante, régénérez les manifestes et déployez les binaires de manière synchronisée ; le mélange des versions amène les validateurs à rejeter les blocs avec `ConfidentialFeatureDigestMismatch`.
- Les manifestes d'activation DEVRAIENT regrouper les mises à jour du registre, les modifications du cycle de vie des paramètres et les transitions de politique afin que le résumé reste cohérent :
  1. Appliquez les mutations de registre planifiées (`Publish*`, `Set*Lifecycle`) dans une vue d'état hors ligne et calculez le résumé post-activation avec `compute_confidential_feature_digest`.
  2. Émettez `SetParameter::custom(confidential_registry_root, {"vk_set_hash": "0x…"})` à l'aide du hachage calculé afin que les homologues en retard puissent récupérer le résumé correct même s'ils manquent les instructions de registre intermédiaires.
  3. Ajoutez les instructions `ScheduleConfidentialPolicyTransition`. Chaque instruction doit citer le numéro de gouvernance `transition_id` ; les manifestes qui l’oublient seront rejetés par le runtime.
  4. Conservez les octets du manifeste, une empreinte digitale SHA-256 et le résumé utilisé dans le plan d'activation. Les opérateurs vérifient les trois artefacts avant de voter le manifeste pour éviter les partitions.
- Lorsque les déploiements nécessitent un basculement différé, enregistrez la hauteur cible dans un paramètre personnalisé associé (par exemple `custom.confidential_upgrade_activation_height`). Cela donne aux auditeurs une preuve codée en Norito que les validateurs ont respecté la fenêtre de notification avant que le changement de résumé ne prenne effet.## Cycle de vie du vérificateur et des paramètres
### Registre ZK
- Ledger stocke `ZkVerifierEntry { vk_id, circuit_id, version, proving_system, curve, public_inputs_schema_hash, vk_hash, vk_len, max_proof_bytes, gas_schedule_id, activation_height, deprecation_height, withdraw_height, status, metadata_uri_cid, vk_bytes_cid }` où `proving_system` est actuellement fixé à `Halo2`.
- Les paires `(circuit_id, version)` sont uniques au monde ; le registre maintient un index secondaire pour les recherches par métadonnées de circuit. Les tentatives d'enregistrement d'un duplicata sont rejetées lors de l'admission.
- `circuit_id` doit être non vide et `public_inputs_schema_hash` doit être fourni (généralement un hachage Blake2b-32 du codage canonique d'entrée publique du vérificateur). L'admission rejette les enregistrements qui omettent ces champs.
- Les instructions de gouvernance comprennent :
  - `PUBLISH` pour ajouter une entrée `Proposed` avec métadonnées uniquement.
  - `ACTIVATE { vk_id, activation_height }` pour planifier l'activation d'une entrée à une limite d'époque.
  - `DEPRECATE { vk_id, deprecation_height }` pour marquer la hauteur finale où les épreuves peuvent faire référence à l'entrée.
  - `WITHDRAW { vk_id, withdraw_height }` pour arrêt d'urgence ; Les actifs concernés gèlent les dépenses confidentielles après la hauteur de retrait jusqu'à ce que de nouvelles entrées soient activées.
- Genesis manifeste l'émission automatique d'un paramètre personnalisé `confidential_registry_root` dont `vk_set_hash` correspond aux entrées actives ; la validation recoupe ce résumé avec l'état du registre local avant qu'un nœud puisse rejoindre le consensus.
- L'enregistrement ou la mise à jour d'un vérificateur nécessite un `gas_schedule_id` ; la vérification impose que l'entrée de registre est `Active`, présente dans l'index `(circuit_id, version)`, et que les preuves Halo2 fournissent un `OpenVerifyEnvelope` dont `circuit_id`, `vk_hash` et `public_inputs_schema_hash` correspondent à l'enregistrement de registre.

### Prouver les clés
- Les clés de preuve restent hors grand livre mais sont référencées par des identifiants adressés par contenu (`pk_cid`, `pk_hash`, `pk_len`) publiés avec les métadonnées du vérificateur.
- Les SDK de portefeuille récupèrent les données PK, vérifient les hachages et mettent en cache localement.

### Paramètres de Pedersen et Poséidon
- Registres séparés (`PedersenParams`, `PoseidonParams`) contrôles du cycle de vie du vérificateur de miroir, chacun avec `params_id`, hachages de générateurs/constantes, activation, dépréciation et hauteurs de retrait.

## Ordre déterministe et annulateurs
- Chaque actif conserve un `CommitmentTree` avec `next_leaf_index` ; les blocs ajoutent des engagements dans un ordre déterministe : itérer les transactions dans l'ordre des blocs ; au sein de chaque transaction, itérez les sorties protégées en remontant `output_idx` sérialisé.
- `note_position` est dérivé des décalages d'arbre mais ne fait **pas** partie de l'annulateur ; il alimente uniquement les chemins d'adhésion au sein du témoin de preuve.
- La stabilité du nullificateur lors des réorganisations est garantie par la conception du PRF ; l'entrée PRF lie `{ nk, note_preimage_hash, asset_id, chain_id, params_id }` et les ancres font référence aux racines historiques de Merkle limitées par `max_anchor_age_blocks`.## Flux du grand livre
1. **MintConfidential {asset_id, montant, destinataire_hint }**
   - Nécessite la politique d'actifs `Convertible` ou `ShieldedOnly` ; l'admission vérifie l'autorité des actifs, récupère le `params_id` actuel, échantillonne `rho`, émet un engagement, met à jour l'arborescence Merkle.
   - Émet `ConfidentialEvent::Shielded` avec le nouvel engagement, le delta racine Merkle et le hachage d'appel de transaction pour les pistes d'audit.
2. **TransferConfidential {asset_id, proof, circuit_id, version, nullifiers, new_commitments, enc_payloads, Anchor_root, memo }**
   - VM syscall vérifie la preuve à l'aide d'une entrée de registre ; l'hôte garantit que les annulateurs sont inutilisés, les engagements ajoutés de manière déterministe et l'ancre est récente.
   - Ledger enregistre les entrées `NullifierSet`, stocke les charges utiles cryptées pour les destinataires/auditeurs et émet `ConfidentialEvent::Transferred` résumant les annulateurs, les sorties ordonnées, le hachage de preuve et les racines Merkle.
3. **RevealConfidential {asset_id, preuve, circuit_id, version, nullifier, montant, destinataire_account, Anchor_root }**
   - Disponible uniquement pour les actifs `Convertible` ; la preuve valide la valeur du billet égale au montant révélé, le grand livre crédite le solde transparent et brûle le billet protégé en marquant l'annulateur dépensé.
   - Émet `ConfidentialEvent::Unshielded` avec le montant public, les annulateurs consommés, les identifiants de preuve et le hachage d'appel de transaction.

## Ajouts de modèles de données
- `ConfidentialConfig` (nouvelle section de configuration) avec indicateur d'activation, `assume_valid`, boutons de gaz/limite, fenêtre d'ancrage, backend de vérification.
- Schémas `ConfidentialNote`, `ConfidentialTransfer` et `ConfidentialMint` Norito avec octet de version explicite (`CONFIDENTIAL_ASSET_V1 = 0x01`).
- `ConfidentialEncryptedPayload` encapsule les octets de mémo AEAD avec `{ version, ephemeral_pubkey, nonce, ciphertext }`, par défaut `version = CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1` pour la disposition XChaCha20-Poly1305.
- Les vecteurs canoniques de dérivation de clé résident dans `docs/source/confidential_key_vectors.json` ; les points de terminaison CLI et Torii régressent par rapport à ces appareils. Les dérivés orientés portefeuille pour l'échelle de dépenses/annulation/visualisation sont publiés dans `fixtures/confidential/keyset_derivation_v1.json` et testés par les tests du SDK Rust + Swift pour garantir la parité entre les langues.
- `asset::AssetDefinition` gagne `confidential_policy: AssetConfidentialPolicy { mode, vk_set_hash, poseidon_params_id, pedersen_params_id, pending_transition }`.
- `ZkAssetState` conserve la liaison `(backend, name, commitment)` pour les vérificateurs de transfert/non-blindage ; l'exécution rejette les preuves dont la clé de vérification référencée ou en ligne ne correspond pas à l'engagement enregistré et vérifie les preuves de transfert/non bouclier par rapport à la clé backend résolue avant de muter l'état.
- `CommitmentTree` (par actif avec points de contrôle frontaliers), `NullifierSet` saisi par `(chain_id, asset_id, nullifier)`, `ZkVerifierEntry`, `PedersenParams`, `PoseidonParams` stockés dans l'état mondial.
- Mempool maintient les structures transitoires `NullifierIndex` et `AnchorIndex` pour une détection précoce des doublons et des contrôles de l'âge d'ancrage.
- Les mises à jour du schéma Norito incluent l'ordre canonique pour les entrées publiques ; les tests aller-retour garantissent le déterminisme du codage.
- Les allers-retours de charge utile cryptés sont verrouillés via des tests unitaires (`crates/iroha_data_model/src/confidential.rs`), et les vecteurs de dérivation de clé de portefeuille ci-dessus ancrent les dérivations d'enveloppe AEAD pour les auditeurs. `norito.md` documente l'en-tête filaire de l'enveloppe.## IVM Intégration et appel système
- Introduire l'appel système `VERIFY_CONFIDENTIAL_PROOF` acceptant :
  - `circuit_id`, `version`, `scheme`, `public_inputs`, `proof` et `ConfidentialStateDelta { asset_id, nullifiers, commitments, enc_payloads }` résultant.
  - Syscall charge les métadonnées du vérificateur à partir du registre, applique des limites de taille/durée, facture du gaz déterministe et n'applique le delta que si la preuve réussit.
- L'hôte expose le trait `ConfidentialLedger` en lecture seule pour récupérer les instantanés racine Merkle et l'état de l'annuleur ; La bibliothèque Kotodama fournit des aides à l'assemblage de témoins et une validation de schéma.
- Documents Pointer-ABI mis à jour pour clarifier la disposition du tampon de preuve et les descripteurs de registre.

## Négociation des capacités du nœud
- La poignée de main annonce `feature_bits.confidential` avec un `ConfidentialFeatureDigest { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`. La participation du validateur nécessite `confidential.enabled=true`, `assume_valid=false`, des identifiants back-end de vérificateur identiques et des résumés correspondants ; les disparités échouent à la prise de contact avec `HandshakeConfidentialMismatch`.
- La configuration prend en charge `assume_valid` pour les nœuds observateurs uniquement : lorsqu'elle est désactivée, la rencontre d'instructions confidentielles produit un `UnsupportedInstruction` déterministe sans panique ; lorsqu'ils sont activés, les observateurs appliquent les deltas d'état déclarés sans vérifier les preuves.
- Mempool rejette les transactions confidentielles si la capacité locale est désactivée. Les filtres Gossip évitent d'envoyer des transactions protégées à des pairs sans capacité de correspondance tout en transmettant aveuglement des identifiants de vérificateur inconnus dans les limites de taille.

### Révéler la politique d'élagage et de conservation des annulateurs

Confidential ledgers must retain enough history to prove note freshness and to
rejouer les audits axés sur la gouvernance. La politique par défaut, appliquée par
`ConfidentialLedger`, est :

- **Rétention des annulateurs :** conserve les annulateurs dépensés pendant *minimum* `730` jours (24
  mois) après la hauteur de dépense, ou la fenêtre imposée par le régulateur si elle est plus longue.
  Les opérateurs peuvent étendre la fenêtre via `confidential.retention.nullifier_days`.
  Les annulateurs plus jeunes que la fenêtre de rétention DOIVENT rester interrogeables via Torii donc
  les auditeurs peuvent prouver une absence à double dépense.
- **Taille révélée :** les révélations transparentes (`RevealConfidential`) taillent le
  les engagements de notes associés immédiatement après la finalisation du bloc, mais le
  l’annulateur consommé reste soumis à la règle de rétention ci-dessus. Lié à la révélation
  les événements (`ConfidentialEvent::Unshielded`) enregistrent le montant public, le destinataire,
  et un hachage de preuve afin que la reconstruction des révélations historiques ne nécessite pas l'élagage
  texte chiffré.
- **Points de contrôle frontaliers :** les frontières d'engagement maintiennent des points de contrôle roulants
  couvrant le plus grand de `max_anchor_age_blocks` et la fenêtre de rétention. Nœuds
  compactez les anciens points de contrôle uniquement après l’expiration de tous les annulateurs dans l’intervalle.
- **Correction du résumé obsolète :** si `HandshakeConfidentialMismatch` est généré en raison
  pour digérer la dérive, les opérateurs doivent (1) vérifier que les fenêtres de rétention de l'annuleur
  alignez-vous sur le cluster, (2) exécutez `iroha_cli app confidential verify-ledger` pour
  régénérer le résumé par rapport à l'ensemble d'annuleurs retenu, et (3) redéployer le
  manifeste actualisé. Tous les annuleurs élagués prématurément doivent être restaurés à partir de
  stockage frigorifique avant de rejoindre le réseau.Documenter les remplacements locaux dans le runbook des opérations ; les politiques de gouvernance s’étendant
la fenêtre de rétention doit mettre à jour la configuration des nœuds et les plans de stockage d'archives dans
pas de verrouillage.

### Flux d'expulsion et de récupération

1. Pendant la numérotation, `IrohaNetwork` compare les fonctionnalités annoncées. Toute inadéquation génère `HandshakeConfidentialMismatch` ; la connexion est fermée et l'homologue reste dans la file d'attente de découverte sans jamais être promu `Ready`.
2. L'échec est signalé via le journal du service réseau (y compris le résumé distant et le backend), et Sumeragi ne planifie jamais le homologue pour une proposition ou un vote.
3. Les opérateurs corrigent le problème en alignant les registres de vérificateurs et les ensembles de paramètres (`vk_set_hash`, `pedersen_params_id`, `poseidon_params_id`) ou en organisant `next_conf_features` avec un `activation_height` convenu. Une fois que le résumé correspond, la prochaine poignée de main réussit automatiquement.
4. Si un homologue obsolète parvient à diffuser un bloc (par exemple, via une relecture d'archives), les validateurs le rejettent de manière déterministe avec `BlockRejectionReason::ConfidentialFeatureDigestMismatch`, gardant ainsi l'état du grand livre cohérent sur l'ensemble du réseau.

### Flux de négociation sécurisé pour la relecture

1. Chaque tentative sortante alloue du nouveau matériel de clé Noise/X25519. La charge utile de prise de contact signée (`handshake_signature_payload`) concatène les clés publiques éphémères locales et distantes, l'adresse de socket annoncée codée en Norito et, lorsqu'elle est compilée avec `handshake_chain_id`, l'identifiant de chaîne. Le message est crypté avec AEAD avant de quitter le nœud.
2. Le répondeur recalcule la charge utile avec l'ordre des clés homologues/locales inversé et vérifie la signature Ed25519 intégrée dans `HandshakeHelloV1`. Étant donné que les clés éphémères et l'adresse annoncée font partie du domaine de signature, la relecture d'un message capturé sur un autre homologue ou la récupération d'une connexion obsolète échoue de manière déterministe à la vérification.
3. Les indicateurs de capacité confidentielle et le `ConfidentialFeatureDigest` se déplacent à l'intérieur du `HandshakeConfidentialMeta`. Le récepteur compare le tuple `{ enabled, assume_valid, verifier_backend, digest }` à son `ConfidentialHandshakeCaps` configuré localement ; toute incompatibilité disparaît plus tôt avec `HandshakeConfidentialMismatch` avant que le transport ne passe à `Ready`.
4. Les opérateurs DOIVENT recalculer le résumé (via `compute_confidential_feature_digest`) et redémarrer les nœuds avec les registres/politiques mis à jour avant de se reconnecter. Les pairs annonçant d’anciens résumés continuent d’échouer lors de la prise de contact, empêchant l’état obsolète de réintégrer l’ensemble du validateur.
5. Les réussites et les échecs de la négociation mettent à jour les compteurs standard `iroha_p2p::peer` (`handshake_failure_count`, aides à la taxonomie des erreurs) et émettent des entrées de journal structurées étiquetées avec l'ID de l'homologue distant et l'empreinte digitale. Surveillez ces indicateurs pour détecter les tentatives de relecture ou les erreurs de configuration lors du déploiement.## Gestion des clés et charges utiles
- Hiérarchie de dérivation des clés par compte :
  - `sk_spend` → `nk` (clé d'annulation), `ivk` (clé de visualisation entrante), `ovk` (clé de visualisation sortante), `fvk`.
- Les charges utiles de notes cryptées utilisent AEAD avec des clés partagées dérivées d'ECDH ; Des clés de vue facultatives de l’auditeur peuvent être attachées aux sorties par politique d’actif.
- CLI additions: `confidential create-keys`, `confidential send`, `confidential export-view-key`, auditor tooling for decrypting memos, and the `iroha app zk envelope` helper for producing/inspecting Norito memo envelopes offline. Torii expose le même flux de dérivation via `POST /v1/confidential/derive-keyset`, renvoyant à la fois les formulaires hexadécimal et base64 afin que les portefeuilles puissent récupérer les hiérarchies de clés par programme.

## Contrôles de gaz, limites et DoS
- Programme de gaz déterministe :
  - Halo2 (Plonkish) : base gaz `250_000` + gaz `2_000` par entrée publique.
  - Gaz `5` par octet de preuve, plus les frais par annulateur (`300`) et par engagement (`500`).
  - Les opérateurs peuvent remplacer ces constantes via la configuration du nœud (`confidential.gas.{proof_base, per_public_input, per_proof_byte, per_nullifier, per_commitment}`) ; les modifications se propagent au démarrage ou lors du rechargement à chaud de la couche de configuration et sont appliquées de manière déterministe à travers le cluster.
- Limites strictes (valeurs par défaut configurables) :
-`max_proof_size_bytes = 262_144`.
- `max_nullifiers_per_tx = 8`, `max_commitments_per_tx = 8`, `max_confidential_ops_per_block = 256`.
- `verify_timeout_ms = 750`, `max_anchor_age_blocks = 10_000`. Les preuves qui dépassent `verify_timeout_ms` abandonnent l'instruction de manière déterministe (les bulletins de vote de gouvernance émettent `proof verification exceeded timeout`, `VerifyProof` renvoie une erreur).
- Des quotas supplémentaires garantissent la vivacité : générateurs de blocs liés `max_proof_bytes_block`, `max_verify_calls_per_tx`, `max_verify_calls_per_block` et `max_public_inputs` ; `reorg_depth_bound` (≥ `max_anchor_age_blocks`) régit le maintien des points de contrôle frontaliers.
- L'exécution du runtime rejette désormais les transactions qui dépassent ces limites par transaction ou par bloc, émettant des erreurs déterministes `InvalidParameter` et laissant l'état du grand livre inchangé.
- Mempool préfiltre les transactions confidentielles par `vk_id`, la longueur de la preuve et l'âge d'ancrage avant d'appeler le vérificateur pour limiter l'utilisation des ressources.
- La vérification s'arrête de manière déterministe en cas d'expiration du délai ou de violation de limite ; les transactions échouent avec des erreurs explicites. Les backends SIMD sont facultatifs mais ne modifient pas la comptabilité du gaz.

### Lignes de base d'étalonnage et portes d'acceptation
- **Plateformes de référence.** Les analyses d'étalonnage DOIVENT couvrir les trois profils matériels ci-dessous. Les exécutions qui ne parviennent pas à capturer tous les profils sont rejetées lors de l'examen.| Profil | Architecture | Processeur/Instance | Drapeaux du compilateur | Objectif |
  | --- | --- | --- | --- | --- |
  | `baseline-simd-neutral` | `x86_64` | AMD EPYC 7B12 (32c) ou Intel Xeon Gold 6430 (24c) | `RUSTFLAGS="-C target-feature=-avx,-avx2,-fma"` | Établir des valeurs plancher sans intrinsèques de vecteurs ; utilisé pour ajuster les tableaux de coûts de secours. |
  | `baseline-avx2` | `x86_64` | Intel Xeon Or 6430 (24c) | version par défaut | Valide le chemin AVX2 ; vérifie que les accélérations SIMD restent dans la tolérance du gaz neutre. |
  | `baseline-neon` | `aarch64` | AWS Graviton3 (c7g.4xlarge) | version par défaut | Garantit que le backend NEON reste déterministe et aligné sur les calendriers x86. |

- **Faisceau de référence.** Tous les rapports d'étalonnage des gaz DOIVENT être produits avec :
  -`CRITERION_HOME=target/criterion cargo bench -p iroha_core isi_gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>`
  - `cargo test -p iroha_core bench_repro -- --ignored` pour confirmer le montage déterministe.
  - `CRITERION_HOME=target/criterion cargo bench -p ivm gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>` chaque fois que les coûts de l'opcode de la VM changent.

- **Correction du caractère aléatoire.** Exportez `IROHA_CONF_GAS_SEED=conf-gas-seed-2026Q1` avant d'exécuter des bancs afin que `iroha_test_samples::gen_account_in` passe au chemin déterministe `KeyPair::from_seed`. Le harnais imprime `IROHA_CONF_GAS_SEED_ACTIVE=…` une fois ; si la variable est manquante, la révision DOIT échouer. Tout nouvel utilitaire d'étalonnage doit continuer à honorer cette variable d'environnement lors de l'introduction du caractère aléatoire auxiliaire.

- **Capture des résultats.**
  - Téléchargez les résumés des critères (`target/criterion/**/raw.csv`) pour chaque profil dans l'artefact de version.
  - Stockez les métriques dérivées (`ns/op`, `gas/op`, `ns/gas`) dans `docs/source/confidential_assets_calibration.md` ainsi que le commit git et la version du compilateur utilisés.
  - Maintenir les deux dernières lignes de base par profil ; supprimez les anciens instantanés une fois le rapport le plus récent validé.

- **Tolérances d'acceptation.**
  - Les deltas de gaz entre `baseline-simd-neutral` et `baseline-avx2` DOIVENT rester ≤ ±1,5 %.
  - Les deltas de gaz entre `baseline-simd-neutral` et `baseline-neon` DOIVENT rester ≤ ±2,0 %.
  - Les propositions d'étalonnage dépassant ces seuils nécessitent soit des ajustements de calendrier, soit une RFC expliquant l'écart et l'atténuation.

- **Liste de contrôle de révision.** Les soumissionnaires sont responsables de :
  - Y compris les extraits `uname -a`, `/proc/cpuinfo` (modèle, pas) et `rustc -Vv` dans le journal d'étalonnage.
  - Vérification de `IROHA_CONF_GAS_SEED` répercutée dans la sortie du banc (les bancs impriment la graine active).
  - Garantir que les indicateurs de fonctionnalité du stimulateur cardiaque et du vérificateur confidentiel reflètent la production (`--features confidential,telemetry` lors de l'exécution de bancs avec télémétrie).

## Configuration et opérations
- `iroha_config` gagne la section `[confidential]` :
  ```toml
  [confidential]
  enabled = true
  assume_valid = false
  verifier_backend = "ark_bls12_381"
  max_proof_size_bytes = 262144
  max_nullifiers_per_tx = 8
  max_commitments_per_tx = 8
  max_confidential_ops_per_block = 256
  verify_timeout_ms = 750
  max_anchor_age_blocks = 10000
  max_proof_bytes_block = 1048576
  max_verify_calls_per_tx = 4
  max_verify_calls_per_block = 128
  max_public_inputs = 32
  reorg_depth_bound = 10000
  policy_transition_delay_blocks = 100
  policy_transition_window_blocks = 200
  tree_roots_history_len = 10000
  tree_frontier_checkpoint_interval = 100
  registry_max_vk_entries = 64
  registry_max_params_entries = 32
  registry_max_delta_per_block = 4
  ```
- La télémétrie émet des métriques globales : `confidential_proof_verified`, `confidential_verifier_latency_ms`, `confidential_proof_bytes_total`, `confidential_nullifier_spent`, `confidential_commitments_appended`, `confidential_mempool_rejected_total{reason}` et `confidential_policy_transitions_total`, sans jamais exposer de données en texte clair.
- Surfaces RPC :
  -`GET /confidential/capabilities`
  -`GET /confidential/zk_registry`
  -`GET /confidential/params`## Stratégie de test
- Déterminisme : le brassage aléatoire des transactions au sein des blocs produit des racines de Merkle et des ensembles d'annuleurs identiques.
- Résilience de réorganisation : simulez des réorganisations multi-blocs avec des ancres ; les annuleurs restent stables et les ancres périmées sont rejetées.
- Invariants de gaz : vérifiez une utilisation de gaz identique sur tous les nœuds avec et sans accélération SIMD.
- Tests de limites : épreuves aux plafonds de taille/gaz, nombre maximum d'entrées/sorties, application du délai d'attente.
- Cycle de vie : opérations de gouvernance pour l'activation/dépréciation des vérificateurs et des paramètres, tests de dépenses de rotation.
- Politique FSM : transitions autorisées/non autorisées, délais de transition en attente et rejet du pool de mémoire autour des hauteurs effectives.
- Urgences du registre : le retrait d'urgence gèle les actifs concernés à l'adresse `withdraw_height` et rejette ensuite les preuves.
- Contrôle des capacités : validateurs avec des blocs de rejet `conf_features` incompatibles ; les observateurs avec `assume_valid=true` suivent le rythme sans affecter le consensus.
- Equivalence d'état : les nœuds validateurs/complets/observateurs produisent des racines d'état identiques sur la chaîne canonique.
- Fuzzing négatif : les preuves mal formées, les charges utiles surdimensionnées et les collisions d'annuleurs sont rejetées de manière déterministe.

## Travail exceptionnel
- Comparez les jeux de paramètres Halo2 (taille du circuit, stratégie de recherche) et enregistrez les résultats dans le playbook d'étalonnage afin que les valeurs par défaut de gaz/délai d'attente puissent être mises à jour lors de la prochaine actualisation `confidential_assets_calibration.md`.
- Finaliser les politiques de divulgation de l'auditeur et les API de visualisation sélective associées, en câblant le flux de travail approuvé dans Torii une fois le projet de gouvernance approuvé.
- Étendre le schéma de chiffrement témoin pour couvrir les sorties multi-destinataires et les mémos par lots, en documentant le format d'enveloppe pour les implémenteurs du SDK.
- Commander un examen de sécurité externe des circuits, des registres et des procédures de rotation des paramètres et archiver les résultats à côté des rapports d'audit interne.
- Spécifiez les API de réconciliation des dépenses de l'auditeur et publiez des instructions sur la portée des clés d'affichage afin que les fournisseurs de portefeuilles puissent mettre en œuvre la même sémantique d'attestation.## Phase de mise en œuvre
1. **Phase M0 — Durcissement d'arrêt du navire**
   - ✅ La dérivation du nullificateur suit désormais la conception Poséidon PRF (`nk`, `rho`, `asset_id`, `chain_id`) avec un ordre d'engagement déterministe appliqué dans les mises à jour du grand livre.
   - ✅ L'exécution applique des plafonds de taille d'épreuve et des quotas confidentiels par transaction/par bloc, rejetant les transactions dépassant le budget avec des erreurs déterministes.
   - ✅ La poignée de main P2P annonce `ConfidentialFeatureDigest` (résumé du backend + empreintes digitales du registre) et échoue de manière déterministe en cas d'incompatibilité via `HandshakeConfidentialMismatch`.
   - ✅ Supprimez les paniques dans les chemins d'exécution confidentiels et ajoutez un contrôle de rôle pour les nœuds sans capacité de correspondance.
   - ⚪ Appliquer les budgets de délai d'attente des vérificateurs et les limites de profondeur de réorganisation pour les points de contrôle frontaliers.
     - ✅ Budgets de délai d'attente de vérification appliqués ; les preuves dépassant `verify_timeout_ms` échouent désormais de manière déterministe.
     - ✅ Les points de contrôle frontaliers respectent désormais `reorg_depth_bound`, éliminant les points de contrôle plus anciens que la fenêtre configurée tout en conservant des instantanés déterministes.
   - Introduire `AssetConfidentialPolicy`, la politique FSM et les portes d'application pour les instructions de création/transfert/révélation.
   - Validez `conf_features` dans les en-têtes de bloc et refusez la participation du validateur lorsque les résumés de registre/paramètre divergent.
2. **Phase M1 — Registres et paramètres**
   - Atterrissez les registres `ZkVerifierEntry`, `PedersenParams` et `PoseidonParams` avec opérations de gouvernance, ancrage de genèse et gestion du cache.
   - Câblez un appel système pour exiger des recherches dans le registre, des identifiants de programme de gaz, un hachage de schéma et des vérifications de taille.
   - Expédiez le format de charge utile crypté v1, les vecteurs de dérivation de clés de portefeuille et la prise en charge CLI pour la gestion des clés confidentielles.
3. **Phase M2 — Gaz et performances**
   - Mettre en œuvre un calendrier de gaz déterministe, des compteurs par bloc et des harnais de référence avec télémétrie (vérifier la latence, les tailles d'épreuve, les rejets de mempool).
   - Renforcez les points de contrôle CommitmentTree, le chargement LRU et les indices d'annulation pour les charges de travail multi-actifs.
4. **Phase M3 — Outillage de rotation et de portefeuille**
   - Permettre l'acceptation de preuves multi-paramètres et multi-versions ; prendre en charge l’activation/la dépréciation basées sur la gouvernance avec des runbooks de transition.
   - Fournissez des flux de migration SDK/CLI de portefeuille, des flux de travail d'analyse d'auditeur et des outils de rapprochement des dépenses.
5. **Phase M4 — Audit et opérations**
   - Fournir des flux de travail clés pour l'auditeur, des API de divulgation sélective et des runbooks opérationnels.
   - Planifier un examen externe de la cryptographie/sécurité et publier les résultats dans `status.md`.

Chaque phase met à jour les jalons de la feuille de route et les tests associés pour maintenir des garanties d'exécution déterministes pour le réseau blockchain.

### SDK et couverture des luminaires (phase M1)

La charge utile cryptée v1 est désormais livrée avec des appareils canoniques afin que chaque SDK produise le
mêmes enveloppes Norito et hachages de transaction. Les objets dorés vivent dans
`fixtures/confidential/wallet_flows_v1.json` et sont exercés directement par le
Suites Rust et Swift (`crates/iroha_data_model/tests/confidential_wallet_fixtures.rs`,
`IrohaSwift/Tests/IrohaSwiftTests/ConfidentialWalletFixturesTests.swift`) :

```bash
# Rust parity (verifies the signed hex + hash for every case)
cargo test -p iroha_data_model confidential_wallet_fixtures

# Swift parity (builds the same envelopes via TxBuilder/NativeBridge)
cd IrohaSwift && swift test --filter ConfidentialWalletFixturesTests
```Chaque appareil enregistre l'identifiant du cas, l'hexagone de transaction signé et les informations attendues.
hachage. Lorsque l'encodeur Swift ne peut pas encore produire le boîtier, `zk-transfer-basic` est
toujours contrôlé par le constructeur `ZkTransfer` : la suite de tests émet `XCTSkip` afin que le
La feuille de route indique clairement quels flux nécessitent encore des liaisons. Mise à jour du luminaire
le fichier sans modifier la version du format fera échouer les deux suites, conservant les SDK
et implémentation de référence Rust en étape verrouillée.

#### Constructeurs Swift
`TxBuilder` expose des assistants asynchrones et basés sur le rappel pour chaque
demande confidentielle (`IrohaSwift/Sources/IrohaSwift/TxBuilder.swift:1183`).
Les constructeurs misent sur les exports `connect_norito_bridge`
(`crates/connect_norito_bridge/src/lib.rs:3337`,
`IrohaSwift/Sources/IrohaSwift/NativeBridge.swift:1014`) donc le généré
les charges utiles correspondent aux encodeurs hôtes Rust octet par octet. Exemple :

```swift
let account = AccountId.make(publicKey: keypair.publicKey, domain: "wonderland")
let request = RegisterZkAssetRequest(
    chainId: chainId,
    authority: account,
    assetDefinitionId: "rose#wonderland",
    zkParameters: myZkParams,
    ttlMs: 60_000
)
let envelope = try TxBuilder(client: client)
    .buildRegisterZkAsset(request: request, keypair: keypair)
try await TxBuilder(client: client)
    .submit(registerZkAsset: request, keypair: keypair)
```

Le blindage/déblindage suit le même schéma (`submit(shield:)`,
`submit(unshield:)`), et les tests des appareils Swift réexécutent les constructeurs avec
matériel de clé déterministe pour garantir que les hachages de transaction générés restent
égaux à ceux stockés dans `wallet_flows_v1.json`.

#### Constructeurs JavaScript
Le SDK JavaScript reflète les mêmes flux via les assistants de transaction exportés
de `javascript/iroha_js/src/transaction.js`. Des constructeurs tels que
`buildRegisterZkAssetTransaction` et `buildRegisterZkAssetInstruction`
(`javascript/iroha_js/src/instructionBuilders.js:1832`) normaliser la clé de vérification
identifiants et émettent des charges utiles Norito que l'hôte Rust peut accepter sans aucun
adaptateurs. Exemple :

```js
import {
  buildRegisterZkAssetTransaction,
  signTransaction,
  ToriiClient,
} from "@hyperledger/iroha";

const unsigned = buildRegisterZkAssetTransaction({
  registration: {
    authority: "i105...",
    assetDefinitionId: "rose#wonderland",
    zkParameters: {
      commit_params: "vk_shield",
      reveal_params: "vk_unshield",
    },
    metadata: { displayName: "Rose (Shielded)" },
  },
  chainId: "00000000-0000-0000-0000-000000000000",
});
const signed = signTransaction(unsigned, myKeypair);
await new ToriiClient({ baseUrl: "https://torii" }).submitTransaction(signed);
```

Les constructeurs de bouclier, de transfert et de non-bouclier suivent le même modèle, donnant à JS
aux appelants la même ergonomie que Swift et Rust. Tests sous
`javascript/iroha_js/test/transactionBuilder.test.js` couvre la normalisation
logique tandis que les appareils ci-dessus maintiennent la cohérence des octets de transaction signés.

### Télémétrie et surveillance (Phase M2)

La phase M2 exporte désormais la santé du CommitmentTree directement via Prometheus et Grafana :

- `iroha_confidential_tree_commitments`, `iroha_confidential_tree_depth`, `iroha_confidential_root_history_entries` et `iroha_confidential_frontier_checkpoints` exposent la frontière Merkle en direct par actif tandis que `iroha_confidential_root_evictions_total` / `iroha_confidential_frontier_evictions_total` comptent les trims LRU appliqués par `zk.root_history_cap` et la fenêtre de profondeur du point de contrôle.
- `iroha_confidential_frontier_last_checkpoint_height` et `iroha_confidential_frontier_last_checkpoint_commitments` publient la hauteur + le nombre d'engagements du point de contrôle frontalier le plus récent afin que les exercices de réorganisation et les annulations puissent prouver que les points de contrôle avancent et conservent le volume de charge utile attendu.
- La carte Grafana (`dashboards/grafana/confidential_assets.json`) comprend une série de profondeurs, des panneaux de taux d'expulsion et les widgets de cache de vérification existants afin que les opérateurs puissent prouver que la profondeur de CommitmentTree ne s'effondre jamais, même en cas de désabonnement aux points de contrôle.
- L'alerte `ConfidentialTreeDepthZero` (dans `dashboards/alerts/confidential_assets_rules.yml`) se déclenche une fois les engagements respectés mais la profondeur signalée reste à zéro pendant cinq minutes.

Vous pouvez vérifier les métriques localement avant de câbler Grafana :

```bash
curl -s http://127.0.0.1:8180/metrics \
  | rg 'iroha_confidential_(tree_(commitments|depth)|root_history_entries|frontier_(checkpoints|last_checkpoint_height|last_checkpoint_commitments)|root_evictions_total|frontier_evictions_total){asset_id="xor#wonderland"}'
```

Associez-le à `rg 'iroha_confidential_tree_depth'` sur le même scratch pour confirmer que la profondeur augmente avec les nouveaux engagements, tandis que les compteurs d'expulsions n'augmentent que lorsque l'historique limite les entrées. Ces valeurs doivent correspondre à l'exportation du tableau de bord Grafana que vous attachez aux ensembles de preuves de gouvernance.

#### Télémétrie et alertes du programme de gazLa phase M2 intègre également les multiplicateurs de gaz configurables dans le pipeline de télémétrie afin que les opérateurs puissent prouver que chaque validateur partage les mêmes coûts de vérification avant d'approuver une version :

- `iroha_confidential_gas_base_verify` reflète `confidential.gas.proof_base` (par défaut `250_000`).
- `iroha_confidential_gas_per_public_input`, `iroha_confidential_gas_per_proof_byte`, `iroha_confidential_gas_per_nullifier` et `iroha_confidential_gas_per_commitment` reflètent leurs boutons respectifs dans `ConfidentialConfig`. Les valeurs sont mises à jour au démarrage et à chaque rechargement à chaud de la configuration ; `irohad` (`crates/irohad/src/main.rs:1591,1642`) transmet le planning actif via `Telemetry::set_confidential_gas_schedule`.

Grattez les jauges à côté des métriques CommitmentTree pour confirmer que les boutons sont identiques entre les pairs :

```bash
# compare active multipliers across validators
for host in validator-a validator-b validator-c; do
  curl -s "http://$host:8180/metrics" \
    | rg 'iroha_confidential_gas_(base_verify|per_public_input|per_proof_byte|per_nullifier|per_commitment)'
done
```

Le tableau de bord Grafana `confidential_assets.json` comprend désormais un panneau « Gas Schedule » qui restitue les cinq jauges et met en évidence les divergences. Les règles d'alerte dans `dashboards/alerts/confidential_assets_rules.yml` couvrent :
- `ConfidentialGasMismatch` : vérifie le maximum/min de chaque multiplicateur sur toutes les cibles et pages de scrape lorsqu'elles divergent pendant plus de 3 minutes, invitant les opérateurs à aligner `confidential.gas` via un rechargement à chaud ou un redéploiement.
- `ConfidentialGasTelemetryMissing` : avertit lorsque Prometheus ne peut gratter aucun des cinq multiplicateurs pendant 5 minutes, indiquant une cible de grattage manquante ou une télémétrie désactivée.

Gardez le PromQL suivant à portée de main pour les enquêtes sur appel :

```promql
# ensure every multiplier matches across validators (uses the same projection as the alert)
(max without(instance, job) (iroha_confidential_gas_per_public_input)
  - min without(instance, job) (iroha_confidential_gas_per_public_input)) == 0
```

L’écart doit rester nul en dehors des déploiements de configuration contrôlés. Lors du changement de table de gaz, capturez les éraflures avant/après, joignez-les à la demande de modification et mettez à jour `docs/source/confidential_assets_calibration.md` avec les nouveaux multiplicateurs afin que les examinateurs de gouvernance puissent lier les preuves télémétriques au rapport d'étalonnage.
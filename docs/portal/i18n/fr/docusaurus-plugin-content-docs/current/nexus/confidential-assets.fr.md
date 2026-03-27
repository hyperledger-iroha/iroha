---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/confidential-assets.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : Actifs confidentiels et transferts ZK
description : Blueprint Phase C pour la circulation aveugle, les registres et les contrôles opérateurs.
slug : /nexus/actifs-confidentiels
---
<!--
SPDX-License-Identifier: Apache-2.0
-->
# Conception des actifs confidentiels et transferts ZK

## Motivation
- Livrer des flux d'actifs blindes opt-in afin que les domaines préservent la confidentialité transactionnelle sans modifier la circulation transparente.
- Garder une exécution déterministe sur matériel hétérogène de validateurs et conserver Norito/Kotodama ABI v1.
- Donner aux auditeurs et opérateurs des contrôles de cycle de vie (activation, rotation, révocation) pour les circuits et paramètres cryptographiques.

## Modèle de menace
- Les validateurs sont honnêtes-mais-curieux : ils exécutent le consensus fidèlement mais tentent d'inspecteur ledger/state.
- Les observateurs du réseau voient les données de bloc et les transactions commérées ; Aucune hypothèse de canaux de gossip privés.
- Hors périmètre : analyse de trafic off-ledger, adversaires quantiques (suivi sous la roadmap PQ), attaques de disponibilité du ledger.## Vue d'ensemble du design
- Les actifs peuvent déclarer un *shielded pool* en plus des soldes transparentes existantes ; la circulation blindee est représentée via des engagements cryptographiques.
- Les notes encapsulentes `(asset_id, amount, recipient_view_key, blinding, rho)` avec :
  - Engagement : `Comm = Pedersen(params_id || asset_id || amount || recipient_view_key || blinding)`.
  - Annulateur : `Null = Poseidon(domain_sep || nk || rho || asset_id || chain_id)`, indépendant de l'ordre des notes.
  - Chiffre de charge utile : `enc_payload = AEAD_XChaCha20Poly1305(ephemeral_shared_key, note_plaintext)`.
- Les transactions transportent des payloads `ConfidentialTransfer` codent Norito contenant :
  - Entrées publiques : ancre Merkle, annulateurs, nouveaux engagements, identifiant d'actif, version de circuit.
  - Payloads chiffres pour destinataires et auditeurs optionnels.
  - Preuve connaissance zéro attestant de la conservation de la valeur, de la propriété et de l'autorisation.
- Les clés de vérification et les ensembles de paramètres sont contrôlés via des registres sur grand livre avec fenêtres d'activation ; les noeuds refusant de valider les preuves qui référencent des entrées inconnues ou revoquées.
- Les headers de consensus engagent le résumé de fonctionnalité confidentielle active afin que les blocs soient acceptés seulement si l'état registre/paramètres correspond.
- La construction des preuves utilise une pile Halo2 (Plonkish) sans configuration fiable ; Groth16 ou autres variantes SNARK sont intentionnellement non supportées en v1.

### Calendriers déterministesLes enveloppes de mémo confidentiel livrent désormais un luminaire canonique à `fixtures/confidential/encrypted_payload_v1.json`. Le dataset capture une enveloppe v1 positive plus des enchantillons malformes afin que les SDK puissent affirmer la parité de parsing. Les tests du data-model Rust (`crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs`) et la suite Swift (`IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift`) chargent directement le luminaire, garantissant que l'encodage Norito, les surfaces d'erreur et la couverture de régression restent alignées pendant l'évolution du codec.

Les SDK Swift peuvent maintenant émettre des instructions Shield sans colle JSON sur mesure : construire un
`ShieldRequest` avec l'engagement 32-byte, le chiffre payload et la métadonnée de débit,
appeler puis `IrohaSDK.submit(shield:keypair:)` (ou `submitAndWait`) pour signer et relayer la
transaction via `/v1/pipeline/transactions`. Le helper valide les longueurs d'engagement,
insérez `ConfidentialEncryptedPayload` dans l'encodeur Norito, et reflète le layout `zk::Shield`
ci-dessous afin que les portefeuilles restent synchronisés avec Rust.## Engagements de consensus et gestion des capacités
- Les en-têtes de blocs exposant `conf_features = { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }` ; le digest participe au hash de consensus et doit égaler la vue locale du registre pour accepter un bloc.
- La gouvernance peut mettre en scène des mises à niveau en programmant `next_conf_features` avec un `activation_height` futur ; jusqu'à cette hauteur, les producteurs de blocs doivent continuer à émettre le digest précédent.
- Les noeuds validateurs DOIVENT fonctionnent avec `confidential.enabled = true` et `assume_valid = false`. Les chèques de démarrage refusant l'entrée dans le set de validateurs si l'une des conditions font écho ou si le `conf_features` local diverge.
- Les métadonnées de handshake P2P incluent `{ enabled, assume_valid, conf_features }`. Les pairs annonçant des fonctionnalités non prises en charge sont rejetées avec `HandshakeConfidentialMismatch` et n'entrent jamais dans la rotation de consensus.
- Les résultats de handshake entre validateurs, observateurs et pairs sont capturés dans la matrice de handshake sous [Node Capability Negociation](#node-capability-negotiation). Les echec de handshake exposent `HandshakeConfidentialMismatch` et gardent le peer hors de la rotation de consensus jusqu'à ce que son digest corresponde.
- Les observateurs non validateurs peuvent définir `assume_valid = true` ; ils appliquent les deltas confidentiels à l'aveugle mais n'influencent pas la sécurité du consensus.## Politiques d'actifs
- Chaque définition d'actif transporte un `AssetConfidentialPolicy` défini par le créateur ou via la gouvernance :
  - `TransparentOnly` : mode par défaut ; seules les instructions transparentes (`MintAsset`, `TransferAsset`, etc.) sont autorisées et les opérations protégées sont rejetées.
  - `ShieldedOnly` : toute émission et tout transfert doivent utiliser des instructions confidentielles ; `RevealConfidential` est interdit pour que les soldes ne soient jamais exposées publiquement.
  - `Convertible` : les supports peuvent déplacer la valeur entre les représentations transparentes et blindées via les instructions on/off-ramp ci-dessous.
- Les politiques suivent une contrainte FSM pour éviter les fonds bloqués :
  - `TransparentOnly -> Convertible` (activation immédiate du pool blindé).
  - `TransparentOnly -> ShieldedOnly` (nécessite une transition pendante et une fenêtre de conversion).
  - `Convertible -> ShieldedOnly` (délai minimum imposé).
  - `ShieldedOnly -> Convertible` (plan de migration requis pour que les notes protégées restent dépensables).
  - `ShieldedOnly -> TransparentOnly` est interdit sauf si le pool blindé est vide ou si la gouvernance encode une migration qui de-shield les notes restantes.- Les instructions de gouvernance fixent `pending_transition { new_mode, effective_height, previous_mode, transition_id, conversion_window }` via l ISI `ScheduleConfidentialPolicyTransition` et peuvent annuler des changements de programmes avec `CancelConfidentialPolicyTransition`. La validation mempool assure qu'aucune transaction ne chevauche la hauteur de transition et l'inclusion fait écho déterministiquement si un contrôle de politique changerait au milieu du bloc.
- Les transitions pendantes sont appliquées automatiquement à l'ouverture d'un nouveau bloc : une fois que la hauteur entre dans la fenêtre de conversion (pour les mises à niveau `ShieldedOnly`) ou atteint `effective_height`, le runtime avec le jour `AssetConfidentialPolicy`, rafraichit la métadonnée `zk.policy` et efface l'entrée pendante. Si la fourniture transparente reste non nulle lorsqu'une transition `ShieldedOnly` arrive, le runtime annule le changement et enregistre un avertissement, laissant le mode précédent intact.
- Les boutons de configuration `policy_transition_delay_blocks` et `policy_transition_window_blocks` imposent un préavis minimum et des périodes de grâce pour permettre aux portefeuilles de se convertir autour du switch.
- `pending_transition.transition_id` sert également de poignée d'audit ; la gouvernance doit le citer lors de la finalisation ou de l'annulation afin que les opérateurs puissent correler les rapports on/off-ramp.
- `policy_transition_window_blocks` par défaut à 720 (~12 heures avec temps de blocage 60 s). Les nœuds serrent les demandes de gouvernance qui tentent un préavis plus court.- Genesis manifestes et flux CLI exposant les politiques courantes et pendantes. La logique d admission lit la politique au moment de l exécution pour confirmer que chaque instruction confidentielle est autorisée.
- Checklist de migration - voir "Migration sequencing" ci-dessous pour le plan d'upgrade par étapes suivies par le Milestone M0.

#### Suivi des transitions via Torii

Wallets et auditeurs interrogent `GET /v1/confidential/assets/{definition_id}/transitions` pour inspecteur l `AssetConfidentialPolicy` actif. Le payload JSON inclut toujours l'asset id canonique, la dernière hauteur de bloc observée, le `current_mode` de la politique, le mode effectif à cette hauteur (les fenêtres de conversion rapportent temporairement `Convertible`), et les identifiants attendus de `vk_set_hash`/Poseidon/Pedersen. Quand une transition de gouvernance est en attente la réponse embed aussi:

- `transition_id` - handle d'audit rendu par `ScheduleConfidentialPolicyTransition`.
-`previous_mode`/`new_mode`.
-`effective_height`.
- `conversion_window` et le `window_open_height` dérivent (le bloc ou les wallets doivent commencer la conversion pour les cut-overs ShieldedOnly).

Exemple de réponse :

```json
{
  "asset_id": "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
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

Une réponse `404` indique qu'aucune définition d'actif correspondant n'existe. Lorsqu'aucune transition n'est planifiée le champ `pending_transition` est `null`.

### Machine d'états de politique| Mode actuel | Mode suivant | Prérequis | Gestion de la hauteur efficace | Remarques |
|----------------------------------------------------|-------------------------------------------------------------------------------------------------|----------------------------------------------------------------------------------------------------------------------------------|-----------------------------------------------------------------------------------------------|
| Transparent uniquement | Cabriolet | Gouvernance a active les entrées de registre de vérificateur/paramètres. Soumettre `ScheduleConfidentialPolicyTransition` avec `effective_height >= current_height + policy_transition_delay_blocks`. | La transition s'exécute exactement un `effective_height` ; la piscine protégée devient disponible immédiatement.        | Chemin par défaut pour activer la confidentialité tout en gardant les flux transparents. |
| Transparent uniquement | Blindé uniquement | Idem ci-dessus, plus `policy_transition_window_blocks >= 1`.                                                         | Le runtime entre automatiquement entre `Convertible` et `effective_height - policy_transition_window_blocks` ; passer un `ShieldedOnly` à un `effective_height`. | Fenêtre de conversion déterministe avant de désactiver les instructions transparentes. || Cabriolet | Blindé uniquement | Programme de transition avec `effective_height >= current_height + policy_transition_delay_blocks`. Gouvernance certificateur DEVRAIT (`transparent_supply == 0`) via métadonnées d'audit ; le runtime l applique au cut-over. | Sémantique de fenêtre identique. Si l'alimentation transparente est non nulle à `effective_height`, la transition avorte avec `PolicyTransitionPrerequisiteFailed`. | Verrouille l actif en circulation entièrement confidentielle.                             |
| Blindé uniquement | Cabriolet | Programme de transition ; aucun retrait d'urgence actif (`withdraw_height` non défini).                                  | L etat bascule a `effective_height`; les rampes révèlent rouvrent tandis que les notes blindées restent valides.       | À utiliser pour les fenêtres de maintenance ou les revues d auditeurs.                               |
| Blindé uniquement | Transparent uniquement | La gouvernance doit prouver `shielded_supply == 0` ou préparer un plan `EmergencyUnshield` signé (signatures d'auditeur candidats). | Le runtime ouvre une fenêtre `Convertible` avant `effective_height` ; à la hauteur, les instructions confidentielles échouent durement et l'actif revient au mode transparent-only. | Sortie de dernier recours. La transition s'auto-annule si une note confidentielle est dépensée pendant la fenêtre. || N'importe quel | Identique à l'actuel | `CancelConfidentialPolicyTransition` nettoyer le changement en attente.                                              | `pending_transition` est retiré immédiatement.                                                                       | Maintenir le statu quo ; indique pour complétude.                                         |

Les transitions non répertoriées ci-dessus sont rejetées lors de la soumission gouvernance. Le runtime vérifie les prérequis juste avant d'appliquer une transition programme; en cas d'échec, il repousse l'actif au mode précédent et emet `PolicyTransitionPrerequisiteFailed` via télémétrie et événements de bloc.

### Séquençage de la migration1. **Préparer les registres:** activer toutes les entrées vérificateur et paramètres référencés par la politique cible. Les noeuds annoncent le `conf_features` résultant pour que les pairs vérifient la cohérence.
2. **Planifier la transition :** soumettre `ScheduleConfidentialPolicyTransition` avec un `effective_height` respectant `policy_transition_delay_blocks`. En allant vers `ShieldedOnly`, préciser une fenêtre de conversion (`window >= policy_transition_window_blocks`).
3. **Publier la guidance Operator:** enregistre le `transition_id` retourne et diffuse un runbook on/off-ramp. Les portefeuilles et auditeurs s abonnent à `/v1/confidential/assets/{id}/transitions` pour connaître la hauteur d ouverture de fenêtre.
4. **Application de la fenêtre :** à l'ouverture, le runtime bascule la politique en `Convertible`, emet `PolicyTransitionWindowOpened { transition_id }`, et commence à rejeter les demandes de gouvernance en conflit.
5. **Finaliser ou avorter :** a `effective_height`, le runtime vérifie les prérequis (supply transparent zero, pas de retrait d'urgence, etc.). En succès, la politique passe au mode demande ; en echec, `PolicyTransitionPrerequisiteFailed` est émis, la transition pendante est nette et la politique reste changée.6. **Mises à jour du schéma :** après une transition russe, la gouvernance augmente la version de schéma de l'actif (par ex `asset_definition.v2`) et l'outillage CLI exige `confidential_policy` lors de la sérialisation des manifestes. Les documents de mise à niveau de Genesis instruisent les opérateurs à ajouter les paramètres de politique et d'empreintes de registre avant de redémarrer les validateurs.

Les nouveaux réseaux qui se démarquent de la confidentialité active codent la politique désirée directement dans la genèse. Ils suivent quand même la checklist ci-dessus lors des changements post-lancement afin que les fenêtres de conversion restent déterministes et que les portefeuilles impliquent le temps de s'ajuster.

### Versionnement et activation des manifestes Norito- Les genesis manifestes DOIVENT incluent un `SetParameter` pour la clé personnalisée `confidential_registry_root`. Le payload est un Norito JSON correspondant à `ConfidentialRegistryMeta { vk_set_hash: Option<String> }` : omettre le champ (`null`) quand aucune entrée n'est active, sinon fournir une chaîne hex 32-byte (`0x...`) égale au hash produit par `compute_vk_set_hash` sur les instructions de vérificateur du manifeste. Les nœuds refusent de se démarrer si le paramètre manque ou si le hachage diverge des écritures du registre encodées.
- Le on-wire `ConfidentialFeatureDigest::conf_rules_version` embarque la version de layout du manifeste. Pour les réseaux v1 il DOIT rester `Some(1)` et egaler `iroha_config::parameters::defaults::confidential::RULES_VERSION`. Quand le jeu de règles évolue, incrémenter la constante, régénérer les manifestes et déployer les binaires en lock-step ; mélanger les versions fait rejeter les blocs par les validateurs avec `ConfidentialFeatureDigestMismatch`.
- Les activations manifestes DEVRAIENT regrouper les mises à jour de registre, les changements de cycle de vie des paramètres et les transitions de politique afin que le résumé reste cohérent :
  1. Appliquer les mutations de registre planifiées (`Publish*`, `Set*Lifecycle`) dans une vue hors ligne de l'état et calculer le digest post-activation avec `compute_confidential_feature_digest`.
  2. Emmettre `SetParameter::custom(confidential_registry_root, {"vk_set_hash": "0x..."})` en utilisant le hash calcule pour que les pairs en retard récupèrent le digest correct meme s'ils ratent des instructions intermédiaires.3. Ajoutez les instructions `ScheduleConfidentialPolicyTransition`. Chaque instruction doit citer le `transition_id` émis par gouvernance ; les manifestes qui l’oublient seront rejetés par le runtime.
  4. Conserver les octets du manifeste, une empreinte SHA-256 et le résumé utilisé dans le plan d'activation. Les opérateurs vérifient les trois artefacts avant de voter le manifeste pour éviter les cloisons.
- Lorsque les déploiements nécessitent un basculement différent, enregistrez la hauteur de la cible dans un paramètre compagnon personnalisé (par ex `custom.confidential_upgrade_activation_height`). Cela fournit aux auditeurs une preuve Norito codée et que les validateurs ont respecté la fenêtre de préavis avant que le changement de digest prenne effet.## Cycle de vie des vérificateurs et paramètres
### Registre ZK
- Le grand livre stocke `ZkVerifierEntry { vk_id, circuit_id, version, proving_system, curve, public_inputs_schema_hash, vk_hash, vk_len, max_proof_bytes, gas_schedule_id, activation_height, deprecation_height, withdraw_height, status, metadata_uri_cid, vk_bytes_cid }` ou `proving_system` est actuellement fixé à `Halo2`.
- Les paires `(circuit_id, version)` sont globalement uniques ; le registre maintient un index secondaire pour des recherches par métadonnées de circuit. Les tentatives d'enregistrement d'une paire dupliquée sont rejetées lors de l'admission.
- `circuit_id` doit être non vide et `public_inputs_schema_hash` doit être fourni (typiquement un hash Blake2b-32 de l'encodage canonique des entrées publiques du vérificateur). L admission rejette les enregistrements qui omettent ces champs.
- Les instructions de gouvernance incluent :
  - `PUBLISH` pour ajouter une entrée `Proposed` avec métadonnées seulement.
  - `ACTIVATE { vk_id, activation_height }` pour programmer l'activation à une époque limitée.
  - `DEPRECATE { vk_id, deprecation_height }` pour marquer la hauteur finale ou des preuves peuvent référencer l'entrée.
  - `WITHDRAW { vk_id, withdraw_height }` pour arrêt d'urgence ; les actifs touches gèleront les dépenses confidentielles après retrait en hauteur jusqu'à une activation de nouvelles entrées.
- Les genèses se manifestent emettent automatiquement un paramètre personnalisé `confidential_registry_root` dont `vk_set_hash` correspond aux entrées actives ; la validation croise ce digest avec l'état local du registre avant qu'un noeud puisse rejoindre le consensus.- Enregistrer ou mettre à jour un vérificateur requis un `gas_schedule_id` ; la vérification impose que l'entrée soit `Active`, présente dans l'index `(circuit_id, version)`, et que les preuves Halo2 fournissent un `OpenVerifyEnvelope` dont `circuit_id`, `vk_hash`, et `public_inputs_schema_hash` correspondant à l'entrée du registre.

### Prouver les clés
- Les clés de preuve restent hors grand livre mais sont référencées par des identifiants adresses par contenu (`pk_cid`, `pk_hash`, `pk_len`) publiées avec la métadonnée du vérificateur.
- Les SDK wallet récupèrent les PK, vérifient les hashes, et les mettent en cache localement.

### Paramètres Pedersen et Poséidon
- Des registres séparés (`PedersenParams`, `PoseidonParams`) miroirrent les contrôles de cycle de vie des vérificateurs, chacun avec `params_id`, hashes de générateurs/constantes, activation, dépréciation et hauteur de retrait.
- Les engagements et hachages font une séparation de domaine par `params_id` afin que la rotation de paramètres ne réutilise jamais des modèles de bits d'ensembles dépréciés ; l ID est embarque dans les notes d’engagements et les tags de domaine nullifier.
- Les circuits supportant la sélection multi-paramètres à la vérification ; les ensembles de paramètres dépréciés restent dépensables jusqu'à leur `deprecation_height`, et les ensembles retraités sont rejetées exactement à `withdraw_height`.## Ordre déterministe et annulateurs
- Chaque actif maintient un `CommitmentTree` avec `next_leaf_index`; les blocs ajoutent les engagements dans un ordre déterministe : itérer les transactions dans l'ordre de bloc ; dans chaque transaction itérer les sorties blindées par `output_idx` sérialisées ascendantes.
- `note_position` dérive des offsets de l'arbre mais **ne** fait pas partie du nullifier; il ne sert qu'aux chemins d'adhésion dans le témoin de preuve.
- La stabilité des annulateurs sous reorgs est garantie par le design PRF ; l input PRF lie `{ nk, note_preimage_hash, asset_id, chain_id, params_id }`, et les ancres référencent des racines historiques limites de Merkle par `max_anchor_age_blocks`.## Flux du grand livre
1. **MintConfidential {asset_id, montant, destinataire_hint }**
   - Requiert la politique d'actif `Convertible` ou `ShieldedOnly`; l admission verifie l autorité, recupere `params_id`, échantillonne `rho`, emet l engagement, met a jour l arbre Merkle.
   - Emet `ConfidentialEvent::Shielded` avec le nouvel engagement, delta de Merkle root, et hash d'appel de transaction pour audit trails.
2. **TransferConfidential {asset_id, proof, circuit_id, version, nullifiers, new_commitments, enc_payloads, Anchor_root, memo }**
   - Le syscall VM vérifie la preuve via l'entrée du registre ; l'hôte assure que les nullificateurs sont inutilisés, que les engagements sont ajoutés deterministiquement, et que l'ancre est récente.
   - Le grand livre enregistre des entrées `NullifierSet`, stocke les chiffres de charges utiles pour les destinataires/auditeurs et emet `ConfidentialEvent::Transferred` résumant les annulateurs, les ordonnances de sorties, le hachage de preuve et les racines Merkle.
3. **RevealConfidential {asset_id, preuve, circuit_id, version, nullifier, montant, destinataire_account, Anchor_root }**
   - Disponible uniquement pour les actifs `Convertible`; la preuve valide que la valeur de la note égale le montant révélé, le grand livre crédite le solde transparent et brûle la note blindée en marquant le nullifier comme dépense.- Emet `ConfidentialEvent::Unshielded` avec le montant public, annulateurs consommés, identifiants de preuve et hash d'appel de transaction.## Ajouts au modèle de données
- `ConfidentialConfig` (nouvelle section de config) avec drapeau d'activation, `assume_valid`, boutons de gaz/limites, fenêtre d'ancrage, backend de vérificateur.
- Schémas `ConfidentialNote`, `ConfidentialTransfer`, et `ConfidentialMint` Norito avec octet de version explicite (`CONFIDENTIAL_ASSET_V1 = 0x01`).
- `ConfidentialEncryptedPayload` enveloppe des octets mémo AEAD avec `{ version, ephemeral_pubkey, nonce, ciphertext }`, par défaut `version = CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1` pour le layout XChaCha20-Poly1305.
- Les vecteurs canoniques de dérivation de cle vivent dans `docs/source/confidential_key_vectors.json`; le CLI et l'endpoint Torii régressent sur ces luminaires.
- `asset::AssetDefinition` gagne `confidential_policy: AssetConfidentialPolicy { mode, vk_set_hash, poseidon_params_id, pedersen_params_id, pending_transition }`.
- `ZkAssetState` persiste le contraignant `(backend, name, commitment)` pour les vérificateurs transfer/unshield ; l'exécution rejette les preuves dont la clé de vérification référencee ou en ligne ne correspond pas au engagement enregistré.
- `CommitmentTree` (par actif avec postes de contrôle frontaliers), `NullifierSet` cle `(chain_id, asset_id, nullifier)`, `ZkVerifierEntry`, `PedersenParams`, `PoseidonParams` stocks en état mondial.
- Mempool maintient des structures transitoires `NullifierIndex` et `AnchorIndex` pour détection précoce des doublons et contrôles d'âge d'ancrage.
- Les mises à jour de schéma Norito incluent un ordre canonique des entrées publiques ; les tests de round-trip assurent le déterminisme d’encodage.- Les allers-retours de payload chiffrés sont verrouillés via des tests unitaires (`crates/iroha_data_model/src/confidential.rs`). Des vecteurs wallet de suivi attacheront des transcripts AEAD canoniques pour auditeurs. `norito.md` documente l'en-tête sur le fil de l'enveloppe.

## Intégration IVM et syscall
- Introduire le syscall `VERIFY_CONFIDENTIAL_PROOF` acceptant :
  - `circuit_id`, `version`, `scheme`, `public_inputs`, `proof`, et le `ConfidentialStateDelta { asset_id, nullifiers, commitments, enc_payloads }` résultant.
  - Le syscall charge la métadonnée vérificateur depuis le registre, applique des limites de taille/temps, facture un gaz déterministe, et n applique le delta que si la preuve reussit.
- Le host expose un trait read-only `ConfidentialLedger` pour récupérer les snapshots Merkle root et le statut des nullifiers ; la librairie Kotodama fournit des aides d'assemblage de témoin et de validation de schéma.
- Les docs pointer-ABI sont mis à jour pour clarifier la disposition du buffer de preuve et les handles de registre.## Négociation de capacités de noeud
- Le handshake annonce `feature_bits.confidential` avec `ConfidentialFeatureDigest { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`. La participation des validateurs requiert `confidential.enabled=true`, `assume_valid=false`, identifiants de backend verificateurs identiques et digests correspondants ; les décalages font écho au handshake avec `HandshakeConfidentialMismatch`.
- La config supporte `assume_valid` pour les observateurs seulement : quand désactif, rencontrez des instructions confidentielles produit `UnsupportedInstruction` déterministe sans panique ; lorsqu'ils sont actifs, les observateurs appliquent les deltas déclarent sans vérifier les preuves.
- Mempool rejette les transactions confidentielles si la capacité locale est désactivée. Les filtres de gossip évitent d'envoyer des transactions protégées aux pairs sans capacités correspondantes tout en relayant à l'aveugle des vérificateurs d'identité inconnus dans les limites de taille.

### Matrice de poignée de main| Annonce distante | Résultat pour validateurs | Opérateur de notes |
|----------------------|----------------------------------|----------------|
| `enabled=true`, `assume_valid=false`, correspondance backend, correspondance digest | Accepter | Le peer atteint l'état `Ready` et participe à la proposition, au vote, et au fan-out RBC. Aucune action manuelle requise. |
| `enabled=true`, `assume_valid=false`, correspondance backend, résumé obsolète ou absent | Rejeter (`HandshakeConfidentialMismatch`) | La télécommande doit appliquer les activations registre/paramètres en attente ou attendre l `activation_height` planifiée. Tant que corrige, le noeud reste découvrable mais n entre jamais en rotation de consensus. |
| `enabled=true`, `assume_valid=true` | Rejeter (`HandshakeConfidentialMismatch`) | Les validateurs exigent la vérification des preuves ; configurer le distant comme observer avec ingress Torii only ou basculer `assume_valid=false` après activation de la vérification terminée. |
| `enabled=false`, champs omis (build obsolète), ou vérificateur backend différent | Rejeter (`HandshakeConfidentialMismatch`) | Les pairs obsolètes ou partiellement mis à niveau ne peuvent pas rejoindre le réseau de consensus. Mettez-vous à jour vers la release courante et assurez-vous que le tuple backend + digest correspond avant de se reconnecter. |Les observers qui omettent volontairement la verification de proofs ne doivent pas ouvrir de connexions consensus contre les validateurs actifs. Ils peuvent toujours ingérer des blocs via Torii ou des API d'archivage, mais le réseau de consensus les rejette jusqu'à ce qu'ils annoncent des capacités correspondantes.

### Politique de pruning Reveal et retention des nullifiers

Les ledgers confidentiels doivent conserver assez d historique pour prouver la fraicheur des notes et rejouer des audits gouvernance. La politique par defaut, appliquee par `ConfidentialLedger`, est:- **Retention des nullifiers:** conserver les nullifiers dépenses pour un *minimum* de `730` jours (24 mois) après la hauteur de dépense, ou la fenêtre imposée par le régulateur si plus longue. Les opérateurs peuvent étendre la fenêtre via `confidential.retention.nullifier_days`. Les annulateurs plus jeunes que la fenêtre DOIVENT restent interrogeables via Torii afin que les auditeurs prouvent l'absence de double-dépense.
- **Pruning des révélations :** les révélations transparentes (`RevealConfidential`) taillent les engagements associés immédiatement après finalisation du bloc, mais le nullifier consomme reste soumis à la règle de rétention ci-dessus. Les événements `ConfidentialEvent::Unshielded` enregistrent le montant public, le destinataire, et le hash de preuve pour que la reconstruction des révélations historiques ne nécessite pas le chiffré prune.
- **Frontier checkpoints:** les frontières d'engagement maintiennent des checkpoints roulants couvrant le plus grand de `max_anchor_age_blocks` et de la fenêtre de rétention. Les nœuds compactent les points de contrôle plus anciens seulement après expiration de tous les annulateurs dans l'intervalle.- **Remediation digest stale:** si `HandshakeConfidentialMismatch` survient à une cause d'une dérive de digest, les opérateurs doivent (1) vérifier que les fenêtres de rétention des nullifiers sont alignées dans le cluster, (2) lancer `iroha_cli app confidential verify-ledger` pour régénérer le digest sur l'ensemble de nullifiers retenus, et (3) redéployer le manifest rafraichi. Tout annuler prune trop tot doit être restauré depuis le stockage froid avant de rejoindre le réseau.

Documentez les remplacements locaux dans le runbook d'opérations ; les politiques de gouvernance qui étendent la fenêtre de rétention doivent mettre à jour la configuration des noeuds et les plans de stockage d'archivage en lockstep.

### Flux d'expulsion et de récupération1. Pendant le cadran, `IrohaNetwork` comparez les capacités annoncées. Tout décalage niveau `HandshakeConfidentialMismatch` ; la connexion est fermée et le peer reste dans le fichier de découverte sans être promu a `Ready`.
2. L echec est exposé via le log du service reseau (incluant digest remote et backend), et Sumeragi ne planifie jamais le peer pour proposition ou vote.
3. Les opérateurs remédient en alignant les registres vérificateurs et ensembles de paramètres (`vk_set_hash`, `pedersen_params_id`, `poseidon_params_id`) ou en programmant `next_conf_features` avec un `activation_height` convenu. Une fois le digest aligné, la prochaine poignée de main se réinitialise automatiquement.
4. Si un pair stale réussit un diffuseur un bloc (par exemple via replay d archive), les validateurs le rejettent déterministiquement avec `BlockRejectionReason::ConfidentialFeatureDigestMismatch`, gardant l'état du grand livre cohérent dans le réseau.

### Flux de négociation sécurisé pour la relecture1. Chaque tentative sortante alloue un nouveau matériel de clé Noise/X25519. Le payload de handshake signé (`handshake_signature_payload`) concatène les clés publiques éphimères locales et distantes, l'adresse socket annoncée encodée Norito, et - quand compile avec `handshake_chain_id` - l'identifiant de chaîne. Le message est chiffre AEAD avant de quitter le noeud.
2. Le répondeur recalcule la charge utile avec l'ordre de clés peer/local inverse et vérifie la signature Ed25519 embarquée dans `HandshakeHelloV1`. Parce que les deux clés éphimères et l'adresse annoncée font partie du domaine de signature, rejouer un message capture contre un autre pair ou récupérer une connexion périmée écho déterministiquement.
3. Les drapeaux de capacité confidentielle et le `ConfidentialFeatureDigest` voyagent dans `HandshakeConfidentialMeta`. Le récepteur compare le tuple `{ enabled, assume_valid, verifier_backend, digest }` à un fils `ConfidentialHandshakeCaps` local ; tout décalage sort avec `HandshakeConfidentialMismatch` avant que le transport ne passe à `Ready`.
4. Les opérateurs DOIVENT recalculent le digest (via `compute_confidential_feature_digest`) et redemarrer les noeuds avec registres/politiques mises à jour avant de reconnecter. Les pairs annoncant des digests anciens continuaient d'échouer la poignée de main, empechant un état périmé de réentrer dans l'ensemble des validateurs.5. Les succès et échecs de handshake mettent à jour les compteurs standard `iroha_p2p::peer` (`handshake_failure_count`, helpers de taxonomie d'erreur) et emettent des structures de logs avec l'ID du peer distant et l'empreinte du digest. Surveillez ces indicateurs pour détecter les replays ou les mauvaises configurations pendant le déploiement.

## Gestion des clés et charges utiles
- Hiérarchie de dérivation par compte :
  - `sk_spend` -> `nk` (clé d'annulation), `ivk` (clé de visualisation entrante), `ovk` (clé de visualisation sortante), `fvk`.
- Les payloads de notes chiffre es utilisent AEAD avec des clés partagées dérivées par ECDH ; des view clés d auditeur optionnelles peuvent être attachées aux sorties selon la politique de l actif.
- Ajouts CLI : `confidential create-keys`, `confidential send`, `confidential export-view-key`, outillage auditeur pour déchiffrer les mémos, et le helper `iroha app zk envelope` pour produire/inspecter des enveloppes Norito hors ligne.
- Calendrier de gaz déterministe :
  - Halo2 (Plonkish) : base `250_000` gaz + `2_000` gaz par entrée publique.
  - `5` gas par proof byte, plus des charges par nullifier (`300`) et par engagement (`500`).
  - Les opérateurs peuvent surcharger ces constantes via le nœud de configuration (`confidential.gas.{proof_base, per_public_input, per_proof_byte, per_nullifier, per_commitment}`) ; les changements se propagent au démarrage ou lorsque la couche de config hot-reload et sont appliquées déterministiquement dans le cluster.
- Limites durées (configurables par défaut) :
-`max_proof_size_bytes = 262_144`.
-`max_nullifiers_per_tx = 8`, `max_commitments_per_tx = 8`, `max_confidential_ops_per_block = 256`.
- `verify_timeout_ms = 750`, `max_anchor_age_blocks = 10_000`. Les preuves depassant `verify_timeout_ms` abortent l'instruction déterministiquement (les bulletins de gouvernance emettent `proof verification exceeded timeout`, `VerifyProof` retournent une erreur).
- Quotas additionnels assurant la vivacité : `max_proof_bytes_block`, `max_verify_calls_per_tx`, `max_verify_calls_per_block`, et `max_public_inputs` bornent les block builders ; `reorg_depth_bound` (>= `max_anchor_age_blocks`) régit la rétention des postes de contrôle frontaliers.
- L'exécution runtime rejette maintenant les transactions dépassant ces limites par transaction ou par bloc, emettant des erreurs `InvalidParameter` déterministes et laissant l'état du grand livre intact.- Mempool préfiltre les transactions confidentielles par `vk_id`, longueur de preuve et âge d'ancrage avant d'appeler le vérificateur pour borner l'utilisation des ressources.
- La vérification s'arrête déterministiquement sur timeout ou violation de borne ; les transactions échouent avec des erreurs explicites. Les backends SIMD sont optionnels mais ne modifient pas la compatibilité de gas.

### Baselines de calibrage et d'acceptation
- **Plates-formes de référence.** Les runs de calibrage DOIVENT couvrent les trois profils matériels ci-dessous. Les runs ne couvrant pas tous les profils sont rejetées en review.

  | Profil | Architecture | Processeur/Instance | Compilateur de drapeaux | Objectif |
  | --- | --- | --- | --- | --- |
  | `baseline-simd-neutral` | `x86_64` | AMD EPYC 7B12 (32c) ou Intel Xeon Gold 6430 (24c) | `RUSTFLAGS="-C target-feature=-avx,-avx2,-fma"` | Etablit des valeurs plancher sans intrinsèques distinctes; utiliser pour régler les tables de secours. |
  | `baseline-avx2` | `x86_64` | Intel Xeon Or 6430 (24c) | version par défaut | Validez le chemin AVX2; vérifiez que les accélérations SIMD restent dans la tolérance du gaz neutre. |
  | `baseline-neon` | `aarch64` | AWS Graviton3 (c7g.4xlarge) | version par défaut | Assurez-vous que le backend NEON reste déterministe et aligné avec les plannings x86. |- **Harnais de référence.** Tous les rapports de gaz d'étalonnage DOIVENT etre produits avec :
  -`CRITERION_HOME=target/criterion cargo bench -p iroha_core isi_gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>`
  - `cargo test -p iroha_core bench_repro -- --ignored` pour confirmer le luminaire déterministe.
  - `CRITERION_HOME=target/criterion cargo bench -p ivm gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>` quand les couts d'opcode VM changent.

- **Randomness fixe.** Exporter `IROHA_CONF_GAS_SEED=conf-gas-seed-2026Q1` avant de lancer les bancs pour que `iroha_test_samples::gen_account_in` bascule sur la voie déterministe `KeyPair::from_seed`. Le harnais imprime `IROHA_CONF_GAS_SEED_ACTIVE=...` une seule fois; si la variable manque, la revue DOIT echouer. Toute nouvelle utilitaire de calibrage doit continuer à honorer cette env var lors de l introduction d alea auxiliaire.

- **Capture des résultats.**
  - Upload des CV Criterion (`target/criterion/**/raw.csv`) pour chaque profil dans l'artefact de release.
  - Stocker les métriques dérivées (`ns/op`, `gas/op`, `ns/gas`) dans le [Confidential Gas Calibration ledger](./confidential-gas-calibration) avec le commit git et la version de compilateur utilise.
  - Conserver les deux dernières lignes de base par profil ; supprimer les instantanés plus anciens une fois le rapport le plus récent valide.- **Tolérances d'acceptation.**
  - Les deltas de gaz entre `baseline-simd-neutral` et `baseline-avx2` DOIVENT restent <= +/-1,5%.
  - Les deltas de gaz entre `baseline-simd-neutral` et `baseline-neon` DOIVENT restent <= +/-2.0%.
  - Les propositions de calibrage dépassant ces seuils exigeants des ajustements de planning ou un RFC dépendent de l'écart et de l'atténuation.

- **Checklist de review.** Les soumettants sont responsables de :
  - Inclure `uname -a`, extraits de `/proc/cpuinfo` (model, stepping), et `rustc -Vv` dans le log de calibration.
  - Vérifier que `IROHA_CONF_GAS_SEED` apparaît dans la sortie bench (les benchs impriment la seed active).
  - S assurer que les feature flags pacemaker et vérificateur confidentiel miroir la production (`--features confidential,telemetry` lors des benchs avec Telemetry).

## Configuration et opérations
- `iroha_config` ajoute la section `[confidential]` :
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
- Télémétrie emet des métriques agrégées : `confidential_proof_verified`, `confidential_verifier_latency_ms`, `confidential_proof_bytes_total`, `confidential_nullifier_spent`, `confidential_commitments_appended`, `confidential_mempool_rejected_total{reason}`, et `confidential_policy_transitions_total`, sans jamais exposer des données en clair.
- Surfaces RPC :
  -`GET /confidential/capabilities`
  -`GET /confidential/zk_registry`
  -`GET /confidential/params`## Stratégie de tests
- Déterminisme : le brassage aléatoire des transactions dans les blocs donne des racines de Merkle et des ensembles annuleurs identiques.
- Resilience aux reorg : simuler des reorgs multi-blocs avec ancres ; les nullificateurs restent stables et les ancres stales sont rejetées.
- Invariants de gaz : vérifier un usage de gaz identique entre noeuds avec et sans accélération SIMD.
- Tests de limite : preuves aux plafonds de taille/gas, max in/out counts, application des timeouts.
- Cycle de vie : opérations de gouvernance pour activation/dépréciation de vérificateur et paramètres, tests de dépense après rotation.
- Politique FSM : transitions autorisées/interdites, délais de transition pendante, et rejet mempool autour des hauteurs effectives.
- Urgences de registre : retrait d'urgence fige les actifs a `withdraw_height` et rejette les preuves apres.
- Capability gating : validateurs avec `conf_features` mismatched rejettent les blocs ; observers avec `assume_valid=true` suivent sans affecter le consensus.
- Equivalence d'état : noeuds validator/full/observer produisent des racines d'état identiques sur la chaîne canonique.
- Fuzzing négatif : preuves malformées, charges utiles surdimensionnées, et collisions de nullifier rejetées déterministiquement.## Migration
- Déploiement sécurisé : jusqu'à ce que la Phase C3 se termine, `enabled` par défaut à `false` ; les noeuds annoncent leurs capacités avant de rejoindre l'ensemble des validateurs.
- Les actifs transparents ne sont pas affectés ; les instructions confidentielles requièrent des entrées registre et une négociation de capacités.
- Les noeuds compiles sans support confidentiel rejettent les blocs pertinents déterministiquement ; ils ne peuvent pas rejoindre l'ensemble des validateurs mais peuvent fonctionner comme observateurs avec `assume_valid=true`.
- Genesis manifestes inclut les entrées initiales, les ensembles de paramètres, les politiques confidentielles pour les actifs et les clés d'auditeur optionnelles.
- Les opérateurs suivent les runbooks publiés pour rotation de registre, transitions de politique et retrait d'urgence afin de maintenir des mises à niveau déterministes.## Travail restant
- Benchmarker les ensembles de paramètres Halo2 (taille de circuit, stratégie de recherche) et enregistrer les résultats dans le playbook de calibration pour mettre à jour les defaults gas/timeout avec le prochain rafraîchissement `confidential_assets_calibration.md`.
- Finaliser les politiques de divulgation auditeur et les API de select-viewing associés, en cablant le workflow approuver dans Torii une fois le projet de gouvernance signé.
- Etendre le schéma de chiffrement témoin pour couvrir les sorties multi-destinataires et lots de mémos, en documentant le format d'enveloppe pour les implémenteurs SDK.
- Commissionner une revue de sécurité externe des circuits, registres et procédures de rotation des paramètres et archiver les conclusions à la cote des rapports d'audit interne.
- Spécifier des API de réconciliation de dépenses pour les auditeurs et publier la guidance de scope view-key afin que les vendeurs de portefeuilles implémentent les mèmes sémantiques d'attestation.## Mise en œuvre progressive
1. **Phase M0 - Durcissement Stop-Ship**
   - [x] La dérivation de nullifier suit maintenant le design Poseidon PRF (`nk`, `rho`, `asset_id`, `chain_id`) avec un ordonnancement déterministe des engagements imposés dans les mises à jour du grand livre.
   - [x] L'exécution applique des plafonds de taille de preuve et des quotas confidentiels par transaction/par bloc, rejetant les transactions hors budget avec des erreurs déterministes.
   - [x] Le handshake P2P annonce `ConfidentialFeatureDigest` (backend digest + Fingerprints Registry) et fait écho aux mismatches déterministiquement via `HandshakeConfidentialMismatch`.
   - [x] Supprimez les panics dans les chemins d'exécution confidentielles et ajoutez un rôle gating pour les noeuds non pris en charge.
   - [ ] Appliquer les budgets de timeout du vérificateur et les bornes de profondeur de reorg pour les postes de contrôle frontaliers.
     - [x] Budgets de timeout de vérification appliqués ; les preuves dépassant `verify_timeout_ms` échouent maintenant déterministiquement.
     - [x] Les points de contrôle frontaliers respectent maintenant `reorg_depth_bound`, prunant les points de contrôle plus anciens que la fenêtre configurée tout en gardant des instantanés déterministes.
   - Introduire `AssetConfidentialPolicy`, politique FSM et portes d'application pour les instructions mint/transfer/reveal.- Commit `conf_features` dans les headers de bloc et refuser la participation des validateurs lorsque les digests registre/paramètres divergent.
2. **Phase M1 - Registres et paramètres**
   - Livrer les registres `ZkVerifierEntry`, `PedersenParams`, et `PoseidonParams` avec gouvernance des opérations, ancrage genesis et gestion de cache.
   - Câblage du syscall pour imposer des recherches de registre, des identifiants de planification de gaz, un hachage de schéma et des contrôles de taille.
   - Publier le format de payload chiffre v1, vecteurs de dérivation de clés pour portefeuilles, et support CLI pour la gestion des clés confidentielles.
3. **Phase M2 - Gaz et performances**
   - Mettre en œuvre le planning de gas déterministe, compteurs par bloc, et harnais de benchmark avec télémétrie (latence de vérification, tailles de preuve, rejets mempool).
   - Durcir les points de contrôle CommitmentTree, le chargement LRU, et les indices de nullifier pour les workloads multi-assets.
4. **Phase M3 - Portefeuille rotation et outillage**
   - Activer l'acceptation de preuves multi-paramètres et multi-version ; supporter l activation/deprecation pilotée par gouvernance avec runbooks de transition.
   - Livrer les flux de migration SDK/CLI, les workflows de scan auditeur et les outils de réconciliation des dépenses.
5. **Phase M4 - Audit et opérations**
   - Fournir des workflows de clés d'auditeur, des API de divulgation sélective, et des runbooks opérationnels.- Planifier une revue externe cryptographie/sécurité et publier les conclusions dans `status.md`.

Chaque phase met en lumière les jalons du roadmap et les tests associés pour maintenir les garanties d'exécution déterministe du réseau blockchain.
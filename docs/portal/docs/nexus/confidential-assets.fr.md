---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/confidential-assets.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ffe425507ce77dadc54da5eb0a537eb67f0d7353e95c2aaf1f7467ce669845ee
source_last_modified: "2025-11-09T15:13:32.471456+00:00"
translation_last_reviewed: 2025-12-30
---

---
title: Actifs confidentiels et transferts ZK
description: Blueprint Phase C pour la circulation blindee, les registres et les controles operateur.
slug: /nexus/confidential-assets
---
<!--
SPDX-License-Identifier: Apache-2.0
-->
# Design des actifs confidentiels et transferts ZK

## Motivation
- Livrer des flux d actifs blindes opt-in afin que les domaines preservent la privacy transactionnelle sans modifier la circulation transparente.
- Garder une execution deterministe sur hardware heterogene de validateurs et conserver Norito/Kotodama ABI v1.
- Donner aux auditeurs et operateurs des controles de cycle de vie (activation, rotation, revocation) pour les circuits et parametres cryptographiques.

## Modele de menace
- Les validateurs sont honest-but-curious: ils executent le consensus fidelement mais tentent d inspecter ledger/state.
- Les observateurs reseau voient les donnees de bloc et transactions gossipees; aucune hypothese de canaux de gossip prives.
- Hors scope: analyse de trafic off-ledger, adversaires quantiques (suivi sous la roadmap PQ), attaques de disponibilite du ledger.

## Vue d ensemble du design
- Les actifs peuvent declarer un *shielded pool* en plus des balances transparentes existantes; la circulation blindee est representee via des commitments cryptographiques.
- Les notes encapsulent `(asset_id, amount, recipient_view_key, blinding, rho)` avec:
  - Commitment: `Comm = Pedersen(params_id || asset_id || amount || recipient_view_key || blinding)`.
  - Nullifier: `Null = Poseidon(domain_sep || nk || rho || asset_id || chain_id)`, independant de l ordre des notes.
  - Payload chiffre: `enc_payload = AEAD_XChaCha20Poly1305(ephemeral_shared_key, note_plaintext)`.
- Les transactions transportent des payloads `ConfidentialTransfer` encodes Norito contenant:
  - Inputs publics: Merkle anchor, nullifiers, nouveaux commitments, asset id, version de circuit.
  - Payloads chiffres pour recipients et auditeurs optionnels.
  - Preuve zero-knowledge attestant conservation de valeur, ownership et autorisation.
- Les verifying keys et ensembles de parametres sont controles via des registres on-ledger avec fenetres d activation; les noeuds refusent de valider des proofs qui referencent des entrees inconnues ou revoquees.
- Les headers de consensus engagent le digest de fonctionnalite confidentielle actif afin que les blocs soient acceptes seulement si l etat registry/parametres correspond.
- La construction des proofs utilise une pile Halo2 (Plonkish) sans trusted setup; Groth16 ou autres variantes SNARK sont intentionnellement non supportees en v1.

### Fixtures deterministes

Les enveloppes de memo confidentiel livrent desormais un fixture canonique a `fixtures/confidential/encrypted_payload_v1.json`. Le dataset capture une enveloppe v1 positive plus des echantillons malformes afin que les SDKs puissent affirmer la parite de parsing. Les tests du data-model Rust (`crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs`) et la suite Swift (`IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift`) chargent directement le fixture, garantissant que l encoding Norito, les surfaces d erreur et la couverture de regression restent alignees pendant l evolution du codec.

Les SDKs Swift peuvent maintenant emettre des instructions shield sans glue JSON bespoke: construire un
`ShieldRequest` avec le commitment 32-byte, le payload chiffre et la metadata de debit,
puis appeler `IrohaSDK.submit(shield:keypair:)` (ou `submitAndWait`) pour signer et relayer la
transaction via `/v1/pipeline/transactions`. Le helper valide les longueurs de commitment,
insere `ConfidentialEncryptedPayload` dans l encodeur Norito, et reflete le layout `zk::Shield`
ci-dessous afin que les wallets restent synchronises avec Rust.

## Consensus commitments et capability gating
- Les headers de blocs exposent `conf_features = { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`; le digest participe au hash de consensus et doit egaler la vue locale du registry pour accepter un bloc.
- La governance peut mettre en scene des upgrades en programmant `next_conf_features` avec un `activation_height` futur; jusqu a cette hauteur, les producteurs de blocs doivent continuer a emettre le digest precedent.
- Les noeuds validateurs DOIVENT fonctionner avec `confidential.enabled = true` et `assume_valid = false`. Les checks de demarrage refusent l entree dans le set de validateurs si l une des conditions echoue ou si le `conf_features` local diverge.
- La metadata de handshake P2P inclut `{ enabled, assume_valid, conf_features }`. Les peers annoncant des features non prises en charge sont rejetes avec `HandshakeConfidentialMismatch` et n entrent jamais dans la rotation de consensus.
- Les outcomes de handshake entre validateurs, observers et peers sont captures dans la matrice de handshake sous [Node Capability Negotiation](#node-capability-negotiation). Les echec de handshake exposent `HandshakeConfidentialMismatch` et gardent le peer hors de la rotation de consensus jusqu a ce que son digest corresponde.
- Les observers non validateurs peuvent definir `assume_valid = true`; ils appliquent les deltas confidentiels a l aveugle mais n influencent pas la securite du consensus.

## Politiques d actifs
- Chaque definition d actif transporte un `AssetConfidentialPolicy` defini par le createur ou via governance:
  - `TransparentOnly`: mode par defaut; seules les instructions transparentes (`MintAsset`, `TransferAsset`, etc.) sont permises et les operations shielded sont rejetees.
  - `ShieldedOnly`: toute emission et tout transfert doivent utiliser des instructions confidentielles; `RevealConfidential` est interdit pour que les balances ne soient jamais exposees publiquement.
  - `Convertible`: les holders peuvent deplacer la valeur entre representations transparentes et shielded via les instructions on/off-ramp ci-dessous.
- Les politiques suivent un FSM contraint pour eviter les fonds bloques:
  - `TransparentOnly -> Convertible` (activation immediate du shielded pool).
  - `TransparentOnly -> ShieldedOnly` (requiert transition pendante et fenetre de conversion).
  - `Convertible -> ShieldedOnly` (delai minimum impose).
  - `ShieldedOnly -> Convertible` (plan de migration requis pour que les notes shielded restent spendable).
  - `ShieldedOnly -> TransparentOnly` est interdit sauf si le shielded pool est vide ou si la governance encode une migration qui de-shield les notes restantes.
- Les instructions de governance fixent `pending_transition { new_mode, effective_height, previous_mode, transition_id, conversion_window }` via l ISI `ScheduleConfidentialPolicyTransition` et peuvent annuler des changements programmes avec `CancelConfidentialPolicyTransition`. La validation mempool assure qu aucune transaction ne chevauche la hauteur de transition et l inclusion echoue deterministiquement si un check de politique changerait au milieu du bloc.
- Les transitions pendantes sont appliquees automatiquement a l ouverture d un nouveau bloc: une fois que la hauteur entre dans la fenetre de conversion (pour les upgrades `ShieldedOnly`) ou atteint `effective_height`, le runtime met a jour `AssetConfidentialPolicy`, rafraichit la metadata `zk.policy` et efface l entree pendante. Si le supply transparent reste non nul quand une transition `ShieldedOnly` arrive, le runtime annule le changement et loggue un avertissement, laissant le mode precedent intact.
- Les knobs de config `policy_transition_delay_blocks` et `policy_transition_window_blocks` imposent un preavis minimum et des periodes de grace pour permettre aux wallets de convertir autour du switch.
- `pending_transition.transition_id` sert aussi de handle d audit; la governance doit le citer lors de la finalisation ou l annulation afin que les operateurs puissent correler les rapports on/off-ramp.
- `policy_transition_window_blocks` default a 720 (~12 heures avec block time 60 s). Les noeuds clampent les requetes de governance qui tentent un preavis plus court.
- Genesis manifests et flux CLI exposent les politiques courantes et pendantes. La logique d admission lit la politique au moment de l execution pour confirmer que chaque instruction confidentielle est autorisee.
- Checklist de migration - voir "Migration sequencing" ci-dessous pour le plan d upgrade par etapes suivi par le Milestone M0.

#### Monitoring des transitions via Torii

Wallets et auditeurs interrogent `GET /v1/confidential/assets/{definition_id}/transitions` pour inspecter l `AssetConfidentialPolicy` actif. Le payload JSON inclut toujours l asset id canonique, la derniere hauteur de bloc observee, le `current_mode` de la politique, le mode effectif a cette hauteur (les fenetres de conversion rapportent temporairement `Convertible`), et les identifiants attendus de `vk_set_hash`/Poseidon/Pedersen. Quand une transition de governance est en attente la reponse embed aussi:

- `transition_id` - handle d audit renvoye par `ScheduleConfidentialPolicyTransition`.
- `previous_mode`/`new_mode`.
- `effective_height`.
- `conversion_window` et le `window_open_height` derive (le bloc ou les wallets doivent commencer la conversion pour les cut-overs ShieldedOnly).

Exemple de reponse:

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

Une reponse `404` indique qu aucune definition d actif correspondante n existe. Lorsqu aucune transition n est planifiee le champ `pending_transition` est `null`.

### Machine d etats de politique

| Mode actuel       | Mode suivant       | Prerequis                                                                 | Gestion de la hauteur effective                                                                                         | Notes                                                                                     |
|--------------------|------------------|-------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------|
| TransparentOnly    | Convertible      | Governance a active les entrees de registre de verificateur/parametres. Soumettre `ScheduleConfidentialPolicyTransition` avec `effective_height >= current_height + policy_transition_delay_blocks`. | La transition s execute exactement a `effective_height`; le shielded pool devient disponible immediatement.        | Chemin par defaut pour activer la confidentialite tout en gardant les flux transparents. |
| TransparentOnly    | ShieldedOnly     | Idem ci-dessus, plus `policy_transition_window_blocks >= 1`.                                                         | Le runtime entre automatiquement en `Convertible` a `effective_height - policy_transition_window_blocks`; passe a `ShieldedOnly` a `effective_height`. | Fenetre de conversion deterministe avant de desactiver les instructions transparentes. |
| Convertible        | ShieldedOnly     | Transition programmee avec `effective_height >= current_height + policy_transition_delay_blocks`. Governance DEVRAIT certifier (`transparent_supply == 0`) via metadata d audit; le runtime l applique au cut-over. | Semantique de fenetre identique. Si le supply transparent est non nul a `effective_height`, la transition avorte avec `PolicyTransitionPrerequisiteFailed`. | Verrouille l actif en circulation entierement confidentielle.                             |
| ShieldedOnly       | Convertible      | Transition programmee; aucun retrait d urgence actif (`withdraw_height` non defini).                                  | L etat bascule a `effective_height`; les ramps reveal rouvrent tandis que les notes shielded restent valides.       | Utilise pour fenetres de maintenance ou revues d auditeurs.                               |
| ShieldedOnly       | TransparentOnly  | Governance doit prouver `shielded_supply == 0` ou preparer un plan `EmergencyUnshield` signe (signatures d auditeur requises). | Le runtime ouvre une fenetre `Convertible` avant `effective_height`; a la hauteur, les instructions confidentielles echouent durement et l actif revient au mode transparent-only. | Sortie de dernier recours. La transition s auto-annule si une note confidentielle est depensee pendant la fenetre. |
| Any                | Same as current  | `CancelConfidentialPolicyTransition` nettoie le changement en attente.                                              | `pending_transition` est retire immediatement.                                                                       | Maintient le status quo; indique pour completude.                                         |

Les transitions non listees ci-dessus sont rejetees lors de la soumission governance. Le runtime verifie les prerequis juste avant d appliquer une transition programmee; en cas d echec, il repousse l actif au mode precedent et emet `PolicyTransitionPrerequisiteFailed` via telemetry et events de bloc.

### Migration sequencing

1. **Preparer les registres:** activer toutes les entrees verificateur et parametres referencees par la politique cible. Les noeuds annoncent le `conf_features` resultant pour que les peers verifient la coherence.
2. **Planifier la transition:** soumettre `ScheduleConfidentialPolicyTransition` avec un `effective_height` respectant `policy_transition_delay_blocks`. En allant vers `ShieldedOnly`, preciser une fenetre de conversion (`window >= policy_transition_window_blocks`).
3. **Publier la guidance operateur:** enregistrer le `transition_id` retourne et diffuser un runbook on/off-ramp. Wallets et auditeurs s abonnent a `/v1/confidential/assets/{id}/transitions` pour connaitre la hauteur d ouverture de fenetre.
4. **Application de la fenetre:** a l ouverture, le runtime bascule la politique en `Convertible`, emet `PolicyTransitionWindowOpened { transition_id }`, et commence a rejeter les demandes de governance en conflit.
5. **Finaliser ou avorter:** a `effective_height`, le runtime verifie les prerequis (supply transparent zero, pas de retrait d urgence, etc.). En succes, la politique passe au mode demande; en echec, `PolicyTransitionPrerequisiteFailed` est emis, la transition pendante est nettoyee et la politique reste inchangee.
6. **Upgrades de schema:** apres une transition reussie, la governance augmente la version de schema de l actif (par ex `asset_definition.v2`) et le tooling CLI exige `confidential_policy` lors de la serialisation des manifests. Les docs d upgrade genesis instruisent les operateurs a ajouter les settings de politique et empreintes registry avant de redemarrer les validateurs.

Les nouveaux reseaux qui demarrent avec la confidentialite activee codent la politique desiree directement dans genesis. Ils suivent quand meme la checklist ci-dessus lors des changements post-launch afin que les fenetres de conversion restent deterministes et que les wallets aient le temps de s ajuster.

### Versionnement et activation des manifests Norito

- Les genesis manifests DOIVENT inclure un `SetParameter` pour la key custom `confidential_registry_root`. Le payload est un Norito JSON correspondant a `ConfidentialRegistryMeta { vk_set_hash: Option<String> }`: omettre le champ (`null`) quand aucune entree n est active, sinon fournir une chaine hex 32-byte (`0x...`) egale au hash produit par `compute_vk_set_hash` sur les instructions de verificateur du manifest. Les noeuds refusent de demarrer si le parametre manque ou si le hash diverge des ecritures registry encodees.
- Le on-wire `ConfidentialFeatureDigest::conf_rules_version` embarque la version de layout du manifest. Pour les reseaux v1 il DOIT rester `Some(1)` et egaler `iroha_config::parameters::defaults::confidential::RULES_VERSION`. Quand le ruleset evolue, incrementer la constante, regenerer les manifests et deployer les binaires en lock-step; melanger les versions fait rejeter les blocs par les validateurs avec `ConfidentialFeatureDigestMismatch`.
- Les activation manifests DEVRAIENT regrouper mises a jour de registry, changements de cycle de vie des parametres et transitions de politique afin que le digest reste coherent:
  1. Appliquer les mutations de registry planifiees (`Publish*`, `Set*Lifecycle`) dans une vue offline de l etat et calculer le digest post-activation avec `compute_confidential_feature_digest`.
  2. Emmettre `SetParameter::custom(confidential_registry_root, {"vk_set_hash": "0x..."})` en utilisant le hash calcule pour que les peers en retard recuperent le digest correct meme s ils ratent des instructions intermediaires.
  3. Ajouter les instructions `ScheduleConfidentialPolicyTransition`. Chaque instruction doit citer le `transition_id` emis par governance; les manifests qui l oublient seront rejetes par le runtime.
  4. Persister les bytes du manifest, une empreinte SHA-256 et le digest utilise dans le plan d activation. Les operateurs verifient les trois artefacts avant de voter le manifest pour eviter les partitions.
- Quand les rollouts exigent un cut-over differe, enregistrer la hauteur cible dans un parametre custom compagnon (par ex `custom.confidential_upgrade_activation_height`). Cela fournit aux auditeurs une preuve Norito encod ee que les validateurs ont respecte la fenetre de preavis avant que le changement de digest prenne effet.

## Cycle de vie des verificateurs et parametres
### Registre ZK
- Le ledger stocke `ZkVerifierEntry { vk_id, circuit_id, version, proving_system, curve, public_inputs_schema_hash, vk_hash, vk_len, max_proof_bytes, gas_schedule_id, activation_height, deprecation_height, withdraw_height, status, metadata_uri_cid, vk_bytes_cid }` ou `proving_system` est actuellement fixe a `Halo2`.
- Les paires `(circuit_id, version)` sont globalement uniques; le registry maintient un index secondaire pour des recherches par metadata de circuit. Les tentatives d enregistrer un paire dupliquee sont rejetees lors de l admission.
- `circuit_id` doit etre non vide et `public_inputs_schema_hash` doit etre fourni (typiquement un hash Blake2b-32 de l encoding canonique des public inputs du verificateur). L admission rejette les enregistrements qui omettent ces champs.
- Les instructions de governance incluent:
  - `PUBLISH` pour ajouter une entree `Proposed` avec metadata seulement.
  - `ACTIVATE { vk_id, activation_height }` pour programmer l activation a une limite d epoch.
  - `DEPRECATE { vk_id, deprecation_height }` pour marquer la hauteur finale ou des proofs peuvent referencer l entree.
  - `WITHDRAW { vk_id, withdraw_height }` pour shutdown d urgence; les actifs touches geleront les depenses confidentielles apres withdraw height jusqu a activation de nouvelles entrees.
- Les genesis manifests emettent automatiquement un parametre custom `confidential_registry_root` dont `vk_set_hash` correspond aux entrees actives; la validation croise ce digest avec l etat local du registry avant qu un noeud puisse rejoindre le consensus.
- Enregistrer ou mettre a jour un verificateur requiert un `gas_schedule_id`; la verification impose que l entree soit `Active`, presente dans l index `(circuit_id, version)`, et que les proofs Halo2 fournissent un `OpenVerifyEnvelope` dont `circuit_id`, `vk_hash`, et `public_inputs_schema_hash` correspondent a l entree du registry.

### Proving Keys
- Les proving keys restent off-ledger mais sont referencees par des identifiants adresses par contenu (`pk_cid`, `pk_hash`, `pk_len`) publies avec la metadata du verificateur.
- Les SDKs wallet recuperent les PK, verifient les hashes, et les mettent en cache localement.

### Parametres Pedersen et Poseidon
- Des registries separes (`PedersenParams`, `PoseidonParams`) mirrorrent les controles de cycle de vie des verificateurs, chacun avec `params_id`, hashes de generateurs/constantes, activation, deprecation et hauteur de withdraw.
- Les commitments et hashes font une separation de domaine par `params_id` afin que la rotation de parametres ne reutilise jamais des bit patterns d ensembles depreces; l ID est embarque dans les note commitments et les tags de domaine nullifier.
- Les circuits supportent la selection multi-parametres a la verification; les ensembles de parametres depreces restent spendable jusqu a leur `deprecation_height`, et les ensembles retires sont rejetes exactement a `withdraw_height`.

## Ordre deterministe et nullifiers
- Chaque actif maintient un `CommitmentTree` avec `next_leaf_index`; les blocs ajoutent les commitments dans un ordre deterministe: iterer les transactions dans l ordre de bloc; dans chaque transaction iterer les sorties shielded par `output_idx` serialise ascendant.
- `note_position` derive des offsets de l arbre mais **ne** fait pas partie du nullifier; il ne sert qu aux chemins de membership dans le witness de preuve.
- La stabilite des nullifiers sous reorgs est garantie par le design PRF; l input PRF lie `{ nk, note_preimage_hash, asset_id, chain_id, params_id }`, et les anchors referencent des Merkle roots historiques limites par `max_anchor_age_blocks`.

## Ledger flow
1. **MintConfidential { asset_id, amount, recipient_hint }**
   - Requiert la politique d actif `Convertible` ou `ShieldedOnly`; l admission verifie l autorite, recupere `params_id`, echantillonne `rho`, emet le commitment, met a jour l arbre Merkle.
   - Emet `ConfidentialEvent::Shielded` avec le nouveau commitment, delta de Merkle root, et hash d appel de transaction pour audit trails.
2. **TransferConfidential { asset_id, proof, circuit_id, version, nullifiers, new_commitments, enc_payloads, anchor_root, memo }**
   - Le syscall VM verifie la proof via l entree du registry; le host assure que les nullifiers sont inutilises, que les commitments sont ajoutes deterministiquement, et que l anchor est recent.
   - Le ledger enregistre des entrees `NullifierSet`, stocke les payloads chiffres pour recipients/auditeurs et emet `ConfidentialEvent::Transferred` res umant nullifiers, outputs ordonnes, hash de proof et Merkle roots.
3. **RevealConfidential { asset_id, proof, circuit_id, version, nullifier, amount, recipient_account, anchor_root }**
   - Disponible uniquement pour les actifs `Convertible`; la proof valide que la valeur de la note egale le montant revele, le ledger credite le balance transparent et brule la note shielded en marquant le nullifier comme depense.
   - Emet `ConfidentialEvent::Unshielded` avec le montant public, nullifiers consommes, identifiants de proof et hash d appel de transaction.

## Ajouts au data model
- `ConfidentialConfig` (nouvelle section de config) avec flag d activation, `assume_valid`, knobs de gas/limites, fenetre d anchor, backend de verificateur.
- `ConfidentialNote`, `ConfidentialTransfer`, et `ConfidentialMint` schemas Norito avec byte de version explicite (`CONFIDENTIAL_ASSET_V1 = 0x01`).
- `ConfidentialEncryptedPayload` enveloppe des memo bytes AEAD avec `{ version, ephemeral_pubkey, nonce, ciphertext }`, par defaut `version = CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1` pour le layout XChaCha20-Poly1305.
- Les vecteurs canoniques de derivation de cle vivent dans `docs/source/confidential_key_vectors.json`; le CLI et l endpoint Torii regressent sur ces fixtures.
- `asset::AssetDefinition` gagne `confidential_policy: AssetConfidentialPolicy { mode, vk_set_hash, poseidon_params_id, pedersen_params_id, pending_transition }`.
- `ZkAssetState` persiste le binding `(backend, name, commitment)` pour les verifiers transfer/unshield; l execution rejette les proofs dont la verifying key referencee ou inline ne correspond pas au commitment enregistre.
- `CommitmentTree` (par actif avec frontier checkpoints), `NullifierSet` cle `(chain_id, asset_id, nullifier)`, `ZkVerifierEntry`, `PedersenParams`, `PoseidonParams` stockes en world state.
- Mempool maintient des structures transitoires `NullifierIndex` et `AnchorIndex` pour detection precoce des doublons et checks d age d anchor.
- Les updates de schema Norito incluent un ordering canonique des public inputs; les tests de round-trip assurent le determinisme d encoding.
- Les roundtrips d encrypted payload sont verrouilles via unit tests (`crates/iroha_data_model/src/confidential.rs`). Des vecteurs wallet de suivi attacheront des transcripts AEAD canoniques pour auditeurs. `norito.md` documente le header on-wire de l envelope.

## Integration IVM et syscall
- Introduire le syscall `VERIFY_CONFIDENTIAL_PROOF` acceptant:
  - `circuit_id`, `version`, `scheme`, `public_inputs`, `proof`, et le `ConfidentialStateDelta { asset_id, nullifiers, commitments, enc_payloads }` resultant.
  - Le syscall charge la metadata verificateur depuis le registry, applique des limites de taille/temps, facture un gas deterministe, et n applique le delta que si la proof reussit.
- Le host expose un trait read-only `ConfidentialLedger` pour recuperer des snapshots Merkle root et le statut des nullifiers; la librairie Kotodama fournit des helpers d assembly de witness et de validation de schema.
- Les docs pointer-ABI sont mis a jour pour clarifier le layout du buffer de proof et les handles registry.

## Negociation de capacites de noeud
- Le handshake annonce `feature_bits.confidential` avec `ConfidentialFeatureDigest { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`. La participation des validateurs requiert `confidential.enabled=true`, `assume_valid=false`, identifiants de backend verificateur identiques et digests correspondants; les mismatches echouent le handshake avec `HandshakeConfidentialMismatch`.
- La config supporte `assume_valid` pour les observers seulement: quand desactive, rencontrer des instructions confidentielles produit `UnsupportedInstruction` deterministe sans panic; quand active, les observers appliquent les deltas declares sans verifier les proofs.
- Mempool rejette les transactions confidentielles si la capacite locale est desactivee. Les filtres de gossip evitent d envoyer des transactions shielded aux peers sans capacites correspondantes tout en relayant a l aveugle des verifier IDs inconnus dans les limites de taille.

### Matrice de handshake

| Annonce distante | Resultat pour validateurs | Notes operateur |
|----------------------|-----------------------------|----------------|
| `enabled=true`, `assume_valid=false`, backend match, digest match | Accepte | Le peer atteint l etat `Ready` et participe a la proposition, au vote, et au fan-out RBC. Aucune action manuelle requise. |
| `enabled=true`, `assume_valid=false`, backend match, digest stale ou absent | Rejete (`HandshakeConfidentialMismatch`) | Le remote doit appliquer les activations registry/parametres en attente ou attendre l `activation_height` planifie. Tant que corrige, le noeud reste decouvrable mais n entre jamais en rotation de consensus. |
| `enabled=true`, `assume_valid=true` | Rejete (`HandshakeConfidentialMismatch`) | Les validateurs exigent la verification de proofs; configurer le remote comme observer avec ingress Torii only ou basculer `assume_valid=false` apres activation de la verification complete. |
| `enabled=false`, champs omis (build obsolete), ou backend verificateur different | Rejete (`HandshakeConfidentialMismatch`) | Peers obsoletes ou partiellement upgrades ne peuvent pas rejoindre le reseau de consensus. Mettez a jour vers la release courante et assurez que le tuple backend + digest corresponde avant de reconnecter. |

Les observers qui omettent volontairement la verification de proofs ne doivent pas ouvrir de connexions consensus contre les validateurs actifs. Ils peuvent toujours ingester des blocs via Torii ou des APIs d archivage, mais le reseau de consensus les rejette jusqu a ce qu ils annoncent des capacites correspondantes.

### Politique de pruning Reveal et retention des nullifiers

Les ledgers confidentiels doivent conserver assez d historique pour prouver la fraicheur des notes et rejouer des audits gouvernance. La politique par defaut, appliquee par `ConfidentialLedger`, est:

- **Retention des nullifiers:** conserver les nullifiers depenses pour un *minimum* de `730` jours (24 mois) apres la hauteur de depense, ou la fenetre imposee par le regulateur si plus longue. Les operateurs peuvent etendre la fenetre via `confidential.retention.nullifier_days`. Les nullifiers plus jeunes que la fenetre DOIVENT rester interrogeables via Torii afin que les auditeurs prouvent l absence de double-spend.
- **Pruning des reveals:** les reveals transparents (`RevealConfidential`) prunent les commitments associes immediatement apres finalisation du bloc, mais le nullifier consomme reste soumis a la regle de retention ci-dessus. Les events `ConfidentialEvent::Unshielded` enregistrent le montant public, le recipient, et le hash de proof pour que la reconstruction des reveals historiques ne requiere pas le ciphertext prune.
- **Frontier checkpoints:** les frontiers de commitment maintiennent des checkpoints roulants couvrant le plus grand de `max_anchor_age_blocks` et de la fenetre de retention. Les noeuds compactent les checkpoints plus anciens seulement apres expiration de tous les nullifiers dans l intervalle.
- **Remediation digest stale:** si `HandshakeConfidentialMismatch` survient a cause d un drift de digest, les operateurs doivent (1) verifier que les fenetres de retention des nullifiers sont alignees dans le cluster, (2) lancer `iroha_cli confidential verify-ledger` pour regenerer le digest sur l ensemble de nullifiers retenus, et (3) redeployer le manifest rafraichi. Tout nullifier prune trop tot doit etre restaure depuis le stockage froid avant de rejoindre le reseau.

Documentez les overrides locaux dans le runbook d operations; les politiques de governance qui etendent la fenetre de retention doivent mettre a jour la configuration des noeuds et les plans de stockage d archivage en lockstep.

### Eviction et recovery flow

1. Pendant le dial, `IrohaNetwork` compare les capacites annoncees. Tout mismatch leve `HandshakeConfidentialMismatch`; la connexion est fermee et le peer reste dans la file de discovery sans etre promu a `Ready`.
2. L echec est expose via le log du service reseau (incluant digest remote et backend), et Sumeragi ne planifie jamais le peer pour proposition ou vote.
3. Les operateurs remedient en alignant les registries verificateur et ensembles de parametres (`vk_set_hash`, `pedersen_params_id`, `poseidon_params_id`) ou en programmant `next_conf_features` avec un `activation_height` convenu. Une fois le digest aligne, le prochain handshake reussit automatiquement.
4. Si un peer stale reussit a diffuser un bloc (par exemple via replay d archive), les validateurs le rejettent deterministiquement avec `BlockRejectionReason::ConfidentialFeatureDigestMismatch`, gardant l etat du ledger coherent dans le reseau.

### Replay-safe handshake flow

1. Chaque tentative sortante alloue un nouveau materiel de cle Noise/X25519. Le payload de handshake signe (`handshake_signature_payload`) concatene les cles publiques ephimeres locale et distante, l adresse socket annoncee encodee Norito, et - quand compile avec `handshake_chain_id` - l identifiant de chain. Le message est chiffre AEAD avant de quitter le noeud.
2. Le responder recompute le payload avec l ordre de cles peer/local inverse et verifie la signature Ed25519 embarquee dans `HandshakeHelloV1`. Parce que les deux cles ephimeres et l adresse annoncee font partie du domaine de signature, rejouer un message capture contre un autre peer ou recuperer une connexion stale echoue deterministiquement.
3. Les flags de capacite confidentielle et le `ConfidentialFeatureDigest` voyagent dans `HandshakeConfidentialMeta`. Le recepteur compare le tuple `{ enabled, assume_valid, verifier_backend, digest }` a son `ConfidentialHandshakeCaps` local; tout mismatch sort avec `HandshakeConfidentialMismatch` avant que le transport ne passe a `Ready`.
4. Les operateurs DOIVENT recomputer le digest (via `compute_confidential_feature_digest`) et redemarrer les noeuds avec registries/politiques mises a jour avant de reconnecter. Les peers annoncant des digests anciens continuent d echouer le handshake, empechant un etat stale de reentrer dans le set de validateurs.
5. Les succes et echecs de handshake mettent a jour les compteurs standard `iroha_p2p::peer` (`handshake_failure_count`, helpers de taxonomie d erreur) et emettent des logs structures avec l ID du peer distant et l empreinte du digest. Surveillez ces indicateurs pour detecter les replays ou les mauvaises configurations pendant le rollout.

## Gestion des cles et payloads
- Hierarchie de derivation par account:
  - `sk_spend` -> `nk` (nullifier key), `ivk` (incoming viewing key), `ovk` (outgoing viewing key), `fvk`.
- Les payloads de notes chiffre es utilisent AEAD avec des shared keys derivees par ECDH; des view keys d auditeur optionnelles peuvent etre attachees aux outputs selon la politique de l actif.
- Ajouts CLI: `confidential create-keys`, `confidential send`, `confidential export-view-key`, tooling auditeur pour dechiffrer les memos, et le helper `iroha zk envelope` pour produire/inspecter des envelopes Norito offline. Torii expose le meme flux de derivation via `POST /v1/confidential/derive-keyset`, retournant des formes hex et base64 pour que les wallets puissent recuperer les hierarchies de cles programmatiquement.

## Gas, limites et controles DoS
- Schedule de gas deterministe:
  - Halo2 (Plonkish): base `250_000` gas + `2_000` gas par public input.
  - `5` gas par proof byte, plus des charges par nullifier (`300`) et par commitment (`500`).
  - Les operateurs peuvent surcharger ces constantes via la configuration node (`confidential.gas.{proof_base, per_public_input, per_proof_byte, per_nullifier, per_commitment}`); les changements se propagent au demarrage ou quand la couche de config hot-reload et sont appliques deterministiquement dans le cluster.
- Limites dures (defaults configurables):
- `max_proof_size_bytes = 262_144`.
- `max_nullifiers_per_tx = 8`, `max_commitments_per_tx = 8`, `max_confidential_ops_per_block = 256`.
- `verify_timeout_ms = 750`, `max_anchor_age_blocks = 10_000`. Les proofs depassant `verify_timeout_ms` abortent l instruction deterministiquement (ballots governance emettent `proof verification exceeded timeout`, `VerifyProof` retourne une erreur).
- Quotas additionnelles assurent la liveness: `max_proof_bytes_block`, `max_verify_calls_per_tx`, `max_verify_calls_per_block`, et `max_public_inputs` bornent les block builders; `reorg_depth_bound` (>= `max_anchor_age_blocks`) gouverne la retention des frontier checkpoints.
- L execution runtime rejette maintenant les transactions depassant ces limites par transaction ou par bloc, emettant des erreurs `InvalidParameter` deterministes et laissant l etat du ledger intact.
- Mempool prefiltre les transactions confidentielles par `vk_id`, longueur de proof et age de anchor avant d appeler le verificateur pour borner l usage des ressources.
- La verification s arrete deterministiquement sur timeout ou violation de borne; les transactions echouent avec des erreurs explicites. Les backends SIMD sont optionnels mais ne modifient pas la comptabilite de gas.

### Baselines de calibration et acceptance gates
- **Plates-formes de reference.** Les runs de calibration DOIVENT couvrir les trois profils hardware ci-dessous. Les runs ne couvrant pas tous les profils sont rejetes en review.

  | Profil | Architecture | CPU / Instance | Flags compilateur | Objectif |
  | --- | --- | --- | --- | --- |
  | `baseline-simd-neutral` | `x86_64` | AMD EPYC 7B12 (32c) ou Intel Xeon Gold 6430 (24c) | `RUSTFLAGS="-C target-feature=-avx,-avx2,-fma"` | Etablit des valeurs plancher sans intrinsics vectorielles; utilise pour regler les tables de cout fallback. |
  | `baseline-avx2` | `x86_64` | Intel Xeon Gold 6430 (24c) | release par defaut | Valide le path AVX2; verifie que les speedups SIMD restent dans la tolerance du gas neutral. |
  | `baseline-neon` | `aarch64` | AWS Graviton3 (c7g.4xlarge) | release par defaut | Assure que le backend NEON reste deterministe et aligne avec les schedules x86. |

- **Benchmark harness.** Tous les rapports de calibration gas DOIVENT etre produits avec:
  - `CRITERION_HOME=target/criterion cargo bench -p iroha_core isi_gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>`
  - `cargo test -p iroha_core bench_repro -- --ignored` pour confirmer le fixture deterministe.
  - `CRITERION_HOME=target/criterion cargo bench -p ivm gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>` quand les couts d opcode VM changent.

- **Randomness fixe.** Exporter `IROHA_CONF_GAS_SEED=conf-gas-seed-2026Q1` avant de lancer les benches pour que `iroha_test_samples::gen_account_in` bascule sur la voie deterministe `KeyPair::from_seed`. Le harness imprime `IROHA_CONF_GAS_SEED_ACTIVE=...` une seule fois; si la variable manque, la review DOIT echouer. Toute nouvelle utilite de calibration doit continuer a honorer cette env var lors de l introduction d alea auxiliaire.

- **Capture des resultats.**
  - Upload des resumes Criterion (`target/criterion/**/raw.csv`) pour chaque profil dans l artefact de release.
  - Stocker les metriques derivees (`ns/op`, `gas/op`, `ns/gas`) dans le [Confidential Gas Calibration ledger](./confidential-gas-calibration) avec le commit git et la version de compilateur utilises.
  - Conserver les deux derniers baselines par profil; supprimer les snapshots plus anciens une fois le rapport le plus recent valide.

- **Tolerances d acceptance.**
  - Les deltas de gas entre `baseline-simd-neutral` et `baseline-avx2` DOIVENT rester <= +/-1.5%.
  - Les deltas de gas entre `baseline-simd-neutral` et `baseline-neon` DOIVENT rester <= +/-2.0%.
  - Les propositions de calibration depassant ces seuils exigent des ajustements de schedule ou un RFC expliquant l ecart et la mitigation.

- **Checklist de review.** Les submitters sont responsables de:
  - Inclure `uname -a`, extraits de `/proc/cpuinfo` (model, stepping), et `rustc -Vv` dans le log de calibration.
  - Verifier que `IROHA_CONF_GAS_SEED` apparait dans la sortie bench (les benches impriment la seed active).
  - S assurer que les feature flags pacemaker et verificateur confidentiel miroir la production (`--features confidential,telemetry` lors des benches avec Telemetry).

## Config et operations
- `iroha_config` ajoute la section `[confidential]`:
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
- Telemetry emet des metriques agregees: `confidential_proof_verified`, `confidential_verifier_latency_ms`, `confidential_proof_bytes_total`, `confidential_nullifier_spent`, `confidential_commitments_appended`, `confidential_mempool_rejected_total{reason}`, et `confidential_policy_transitions_total`, sans jamais exposer des donnees en clair.
- Surfaces RPC:
  - `GET /confidential/capabilities`
  - `GET /confidential/zk_registry`
  - `GET /confidential/params`

## Strategie de tests
- Determinisme: le shuffling aleatoire des transactions dans les blocs donne des Merkle roots et nullifier sets identiques.
- Resilience aux reorg: simuler des reorgs multi-blocs avec anchors; les nullifiers restent stables et les anchors stale sont rejetes.
- Invariants de gas: verifier un usage de gas identique entre noeuds avec et sans acceleration SIMD.
- Tests de limite: proofs aux plafonds de taille/gas, max in/out counts, enforcement des timeouts.
- Lifecycle: operations de governance pour activation/deprecation de verificateur et parametres, tests de depense apres rotation.
- Policy FSM: transitions autorisees/interdites, delais de transition pendante, et rejet mempool autour des hauteurs effectives.
- Urgences de registry: retrait d urgence fige les actifs a `withdraw_height` et rejette les proofs apres.
- Capability gating: validateurs avec `conf_features` mismatched rejettent les blocs; observers avec `assume_valid=true` suivent sans affecter le consensus.
- Equivalence d etat: noeuds validator/full/observer produisent des roots d etat identiques sur la chaine canonique.
- Fuzzing negatif: proofs malformees, payloads surdimensionnes, et collisions de nullifier rejetees deterministiquement.

## Migration
- Rollout feature-gated: jusqu a ce que Phase C3 se termine, `enabled` default a `false`; les noeuds annoncent leurs capacites avant de rejoindre le set de validateurs.
- Les actifs transparents ne sont pas affectes; les instructions confidentielles requierent des entrees registry et une negociation de capacites.
- Les noeuds compiles sans support confidentiel rejettent les blocs pertinents deterministiquement; ils ne peuvent pas rejoindre le set de validateurs mais peuvent fonctionner comme observers avec `assume_valid=true`.
- Genesis manifests incluent les entrees initiales registry, ensembles de parametres, politiques confidentielles pour les actifs, et des keys d auditeur optionnelles.
- Les operateurs suivent les runbooks publies pour rotation de registry, transitions de politique et retrait d urgence afin de maintenir des upgrades deterministes.

## Travail restant
- Benchmarker les ensembles de parametres Halo2 (taille de circuit, strategie de lookup) et enregistrer les resultats dans le playbook de calibration pour mettre a jour les defaults gas/timeout avec le prochain refresh `confidential_assets_calibration.md`.
- Finaliser les politiques de disclosure auditeur et les APIs de selective-viewing associees, en cablant le workflow approuve dans Torii une fois le draft governance signe.
- Etendre le schema de witness encryption pour couvrir les outputs multi-recipient et memos batches, en documentant le format d envelope pour les implementateurs SDK.
- Commissionner une revue de securite externe des circuits, registries et procedures de rotation des parametres et archiver les conclusions a cote des rapports d audit internes.
- Specifier des APIs de reconciliation de spentness pour auditeurs et publier la guidance de scope view-key afin que les vendors de wallets implementent les memes semantiques d attestation.

## Phasing d implementation
1. **Phase M0 - Stop-Ship Hardening**
   - [x] La derivation de nullifier suit maintenant le design Poseidon PRF (`nk`, `rho`, `asset_id`, `chain_id`) avec un ordering deterministe des commitments impose dans les updates du ledger.
   - [x] L execution applique des plafonds de taille de proof et des quotas confidentiels par transaction/par bloc, rejetant les transactions hors budget avec des erreurs deterministes.
   - [x] Le handshake P2P annonce `ConfidentialFeatureDigest` (backend digest + fingerprints registry) et echoue les mismatches deterministiquement via `HandshakeConfidentialMismatch`.
   - [x] Retirer les panics dans les paths d execution confidentielle et ajouter un role gating pour les noeuds non pris en charge.
   - [ ] Appliquer les budgets de timeout du verificateur et les bornes de profondeur de reorg pour les frontier checkpoints.
     - [x] Budgets de timeout de verification appliques; les proofs depassant `verify_timeout_ms` echouent maintenant deterministiquement.
     - [x] Les frontier checkpoints respectent maintenant `reorg_depth_bound`, prunant les checkpoints plus anciens que la fenetre configuree tout en gardant des snapshots deterministes.
   - Introduire `AssetConfidentialPolicy`, policy FSM et gates d enforcement pour les instructions mint/transfer/reveal.
   - Commit `conf_features` dans les headers de bloc et refuser la participation des validateurs quand les digests registry/parametres divergent.
2. **Phase M1 - Registries et parametres**
   - Livrer les registries `ZkVerifierEntry`, `PedersenParams`, et `PoseidonParams` avec ops governance, ancrage genesis et gestion de cache.
   - Cablage du syscall pour imposer lookups registry, gas schedule IDs, schema hashing, et checks de taille.
   - Publier le format de payload chiffre v1, vecteurs de derivation de keys pour wallets, et support CLI pour la gestion des cles confidentielles.
3. **Phase M2 - Gas et performance**
   - Implementer schedule de gas deterministe, compteurs par bloc, et harnesses de benchmark avec telemetry (latence de verification, tailles de proof, rejets mempool).
   - Durcir CommitmentTree checkpoints, chargement LRU, et indices de nullifier pour workloads multi-asset.
4. **Phase M3 - Rotation et tooling wallet**
   - Activer l acceptation de proofs multi-parametres et multi-version; supporter l activation/deprecation pilotee par governance avec runbooks de transition.
   - Livrer les flows de migration SDK/CLI, workflows de scan auditeur, et tooling de reconciliation de spentness.
5. **Phase M4 - Audit et ops**
   - Fournir des workflows de keys d auditeur, des APIs de selective disclosure, et des runbooks operationnels.
   - Planifier une revue externe cryptographie/securite et publier les conclusions dans `status.md`.

Chaque phase met a jour les milestones du roadmap et les tests associes pour maintenir les garanties d execution deterministe du reseau blockchain.

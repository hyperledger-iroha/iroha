---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/confidential-assets.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : Actifs confidentiels et transferts ZK
description : circulation protégée, registres et commandes de l'opérateur pour la phase C, plan directeur
slug : /nexus/actifs-confidentiels
---
<!--
SPDX-License-Identifier: Apache-2.0
-->
# Actifs confidentiels et conception du transfert ZK

## Motivation
- flux d'actifs protégés par opt-in pour les domaines de sécurité et la circulation pour la confidentialité transactionnelle
- auditeurs, opérateurs, circuits, paramètres cryptographiques, contrôles du cycle de vie (activation, rotation, révocation)

## Modèle de menace
- Validateurs honnêtes mais curieux: consensus fidèlement sur le grand livre/l'état et inspecter les points de vente
- Les observateurs du réseau bloquent les données et les transactions bavardes. chaînes de potins privées کا کوئی hypothèse نہیں۔
- Hors de portée : analyse du trafic hors grand livre, adversaires quantiques (feuille de route PQ et attaques contre la disponibilité du grand livre)## Aperçu de la conception
- Actifs موجودہ soldes transparents کے علاوہ *pool protégé* déclarer کر سکتے ہیں؛ les engagements cryptographiques à circulation protégée représentent ہوتی ہے۔
- Les notes `(asset_id, amount, recipient_view_key, blinding, rho)` encapsulent les détails :
  - Engagement : `Comm = Pedersen(params_id || asset_id || amount || recipient_view_key || blinding)`.
  - Annulateur : `Null = Poseidon(domain_sep || nk || rho || asset_id || chain_id)`, commande de notes سے indépendant۔
  - Charge utile cryptée : `enc_payload = AEAD_XChaCha20Poly1305(ephemeral_shared_key, note_plaintext)`.
- Charges utiles `ConfidentialTransfer` codées par Norito pour les transactions :
  - Entrées publiques : ancre Merkle, annuleurs, nouveaux engagements, identifiant d'actif, version du circuit
  - Destinataires et auditeurs facultatifs pour les charges utiles cryptées
  - Preuve de connaissance nulle, conservation de la valeur, propriété et attestation d'autorisation.
- Vérification des clés et des jeux de paramètres dans les registres du grand livre et des fenêtres d'activation et des fenêtres d'activation. nœuds inconnus ou entrées révoquées, référencer les preuves et valider les preuves
- Les en-têtes de consensus résument les fonctionnalités confidentielles actives et acceptent les blocs de validation et acceptent le registre et la correspondance de l'état des paramètres.
- Construction de preuve pile Halo2 (Plonkish) pour une configuration fiable Groth16 propose des variantes de SNARK v1 actuellement disponibles et non prises en charge

### Luminaires déterministesEnveloppes pour mémos confidentiels `fixtures/confidential/encrypted_payload_v1.json` pour appareil canonique Un ensemble de données contient une enveloppe v1 et plusieurs échantillons mal formés ainsi que des SDK analysant la parité pour les utilisateurs. Tests de modèle de données Rust (`crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs`) avec l'appareil Swift Suite (`IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift`) pour les surfaces d'erreur et le codec de couverture de régression Norito کے ارتقا کے ساتھ aligné رہتے ہیں۔

Les SDK Swift sont une colle JSON sur mesure et des instructions de bouclier émettent des éléments tels que : engagement de note de 32 octets, charge utile chiffrée et métadonnées de débit par `ShieldRequest` pour le compte `/v1/pipeline/transactions` pour signe et relais pour `IrohaSDK.submit(shield:keypair:)` (pour `submitAndWait`) pour Les longueurs d'engagement de l'assistant valident le codeur `ConfidentialEncryptedPayload` et le codeur Norito avec le fil et la disposition du miroir `zk::Shield`. Portefeuilles en cuir Rust avec serrure à pas de verrouillage## Engagements de consensus et contrôle des capacités
- Les en-têtes de bloc `conf_features = { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }` exposent les détails digérer le hachage de consensus est un bon moyen de bloquer l'acceptation et de voir le registre local et de correspondre à un mot de passe
- Gouvernance pour l'étape de mise à niveau `activation_height` et `next_conf_features` pour l'étape de mise à niveau. اس hauteur تک blocs producteurs پچھلا digest émettent کرتے رہتے ہیں۔
- Les nœuds de validation `confidential.enabled = true` et `assume_valid = false` fonctionnent et doivent fonctionner Le validateur de contrôles de démarrage a défini une jointure pour un échec local `conf_features` divergent
- Métadonnées de négociation P2P par `{ enabled, assume_valid, conf_features }` pour le client les fonctionnalités non prises en charge font de la publicité pour les pairs `HandshakeConfidentialMismatch` et rejettent la rotation par consensus et la rotation par consensus
- Observateurs non-validateurs `assume_valid = true` سیٹ کر سکتے ہیں؛ وہ deltas confidentiels کو appliquer aveuglément کرتے ڈالتے۔## Politiques relatives aux actifs
- Définition des actifs, créateur et gouvernance, ensemble et ensemble `AssetConfidentialPolicy`:
  - `TransparentOnly` : mode par défaut؛ Les instructions transparentes (`MintAsset`, `TransferAsset` وغیرہ) ont autorisé le rejet des opérations blindées ہیں۔
  - `ShieldedOnly` : émission et transferts d'instructions confidentielles pour les demandes de paiement `RevealConfidential` ممنوع ہے تاکہ soldes عوامی نہ ہوں۔
  - `Convertible` : supports pour instructions de rampe d'accès/sortie transparents et représentations blindées pour le déplacement de la valeur
- Les politiques contraintes du FSM suivent les fonds bloqués ici :
  - `TransparentOnly → Convertible` (activation de la piscine blindée)
  - `TransparentOnly → ShieldedOnly` (en attente de transition vers la fenêtre de conversion ici)
  - `Convertible → ShieldedOnly` (délai minimum)
  - `ShieldedOnly → Convertible` (plan de migration pour les billets protégés dépensables رہیں)
  - `ShieldedOnly → TransparentOnly` refusé ہے جب تک pool blindé vide نہ ہو یا gouvernance notes en suspens کو non blindé کرنے والی migration encode نہ کرے۔
- Instructions de gouvernance `ScheduleConfidentialPolicyTransition` ISI کے ذریعے `pending_transition { new_mode, effective_height, previous_mode, transition_id, conversion_window }` set کرتی ہیں اور `CancelConfidentialPolicyTransition` pour l'abandon des modifications programmées کر سکتی ہیں۔ Validation Mempool en cours de vérification de la hauteur de transition de transaction et chevauchement en cas de changement de vérification de la politique en cours de bloc pour inclusion déterministe en cas d'échec- Les transitions en attente du bloc sont appliquées automatiquement : la fenêtre de conversion de hauteur de bloc est en cours (mises à niveau ShieldedOnly uniquement) par `effective_height` La mise à jour du runtime `AssetConfidentialPolicy` est terminée et l'actualisation des métadonnées `zk.policy` est terminée et l'entrée en attente est effacée. `ShieldedOnly` transition mature pour l'alimentation transparente et pour l'abandon du changement d'exécution ainsi que pour le journal d'avertissement et le mode précédent pour le mode précédent
- Boutons de configuration `policy_transition_delay_blocks` et `policy_transition_window_blocks` préavis minimum et délais de grâce appliqués pour changer de portefeuille et convertir les notes en un clic
- Poignée d'audit `pending_transition.transition_id` pour le client gouvernance et transitions finalisées et annulation et citations et rapports des opérateurs sur les rampes d'accès et de sortie en corrélation avec les opérateurs
- `policy_transition_window_blocks` par défaut 720 secondes (temps de bloc de 60 s pour 12 heures) Demandes de gouvernance des nœuds et préavis plus court
- Genesis manifeste les flux CLI actuels et les politiques en attente exposent les problèmes. Temps d'exécution de la logique d'admission پر politique پڑھ کر confirmer کرتی ہے کہ ہر instruction confidentielle autorisée ہے۔
- Liste de contrôle de migration — « Séquençage de la migration » et Milestone M0 et plan de mise à niveau par étapes

#### Torii Transitions et surveillancePortefeuilles et auditeurs `GET /v1/confidential/assets/{definition_id}/transitions` et sondage actif `AssetConfidentialPolicy` Charge utile JSON, identifiant d'actif canonique, dernière hauteur de bloc observée, `current_mode`, hauteur en mode effectif (fenêtres de conversion et rapport `Convertible` en mode effectif) et attendu `vk_set_hash`/Poseidon/Pedersen identifiants شامل کرتا ہے۔ La transition de gouvernance en attente et la réponse sont les suivantes :

- `transition_id` - `ScheduleConfidentialPolicyTransition` pour la poignée d'audit
-`previous_mode`/`new_mode`.
-`effective_height`.
- `conversion_window` et `window_open_height` dérivé (et bloquer les portefeuilles et le basculement ShieldedOnly pour la conversion)

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

Réponse `404` en réponse à la définition d'actif correspondant à la définition d'actif correspondante Pour la transition prévue le 18 novembre 2012, le champ `pending_transition` `null` s'est terminé.

### Machine à états de stratégie| Mode actuel | Mode suivant | Conditions préalables | Manutention en hauteur effective | Remarques |
|----------------------------------------------------|-------------------------------------------------------------------------------------------------|----------------------------------------------------------------------------------------------------------------------------------|-----------------------------------------------------------------------------------------------|
| Transparent uniquement | Cabriolet | Les entrées de registre du vérificateur/paramètre de gouvernance sont activées. `ScheduleConfidentialPolicyTransition` soumettre کریں جس میں `effective_height ≥ current_height + policy_transition_delay_blocks` ہو۔ | Transition بالکل `effective_height` پر exécuter ہوتا ہے؛ piscine protégée فوراً دستیاب ہوتا ہے۔                   | Les flux sont activés et la confidentialité est activée et le chemin par défaut est activé.               |
| Transparent uniquement | Blindé uniquement | اوپر والی شرائط کے ساتھ `policy_transition_window_blocks ≥ 1` بھی۔                                                         | Runtime `effective_height - policy_transition_window_blocks` et `Convertible` sont disponibles. `effective_height` et `ShieldedOnly` en français | شفاف instructions désactiver ہونے سے پہلے fenêtre de conversion déterministe دیتا ہے۔   || Cabriolet | Blindé uniquement | `effective_height ≥ current_height + policy_transition_delay_blocks` pour une transition planifiée La gouvernance DEVRAIT auditer les métadonnées کے ذریعے (`transparent_supply == 0`) certifier کرے؛ runtime cut-over pour appliquer le code | اوپر جیسی sémantique des fenêtres۔ `effective_height` pour une alimentation transparente non nulle et une transition `PolicyTransitionPrerequisiteFailed` pour un abandon ou un abandon | actif کو مکمل circulation confidentielle میں serrure کرتا ہے۔                                     |
| Blindé uniquement | Cabriolet | Transition programmée؛ Retrait d'urgence actif (`withdraw_height` non activé)                                    | `effective_height` pour le retournement d'état révéler des rampes دوبارہ کھلتی ہیں جبکہ notes blindées valides رہتے ہیں۔                           | fenêtres de maintenance et avis d'auditeur                                          |
| Blindé uniquement | Transparent uniquement | Gouvernance کو `shielded_supply == 0` ثابت کرنا ہوگا یا signé `EmergencyUnshield` étape du plan کرنا ہوگا (signatures des auditeurs درکار)۔ | Runtime `effective_height` pour la fenêtre `Convertible` اس hauteur پر instructions confidentielles hard-fail ہو جاتی ہیں اور actif mode transparent uniquement | sortie de dernier recours۔ اگر window کے دوران کوئی note confidentielle dépense et transition auto-annulation || N'importe quel | Identique à l'actuel | `CancelConfidentialPolicyTransition` en attente de changement clair کرتا ہے۔                                                        | `pending_transition` فوراً supprimer ہوتا ہے۔                                                                          | statu quo exhaustivité کیلئے شامل۔                                             |

اوپر liste نہ ہونے والے transitions gouvernance soumission پر rejet ہوتے ہیں۔ La transition planifiée à l'exécution s'applique et vérifie les prérequis. échec de l'actif et du mode précédent et des événements de télémétrie/blocage `PolicyTransitionPrerequisiteFailed` émettent des problèmes

### Séquençage de la migration2. **Étapez la transition :** `policy_transition_delay_blocks` et respectez les règles `effective_height` et `ScheduleConfidentialPolicyTransition` soumettre les règles. `ShieldedOnly` pour la fenêtre de conversion et spécifiez (`window ≥ policy_transition_window_blocks`)
3. **Publier les conseils de l'opérateur :** L'enregistrement `transition_id` est effectué et le runbook d'accès/de sortie de la rampe circule. Portefeuilles et auditeurs `/v1/confidential/assets/{id}/transitions` abonnez-vous à la hauteur de la fenêtre ouverte جانتے ہیں۔
4. **Application de la fenêtre :** la fenêtre définit la politique d'exécution du commutateur `Convertible` et le commutateur `PolicyTransitionWindowOpened { transition_id }` émet des demandes de gouvernance conflictuelles et rejette les demandes de gouvernance conflictuelles. کرتا ہے۔
5. **Finaliser ou abandonner :** `effective_height` pour vérifier les conditions préalables à la transition d'exécution (alimentation transparente et retrait d'urgence نہیں وغیرہ)۔ Politique de réussite et mode demandé et retournement de situation échec `PolicyTransitionPrerequisiteFailed` émettre un message en attente de transition effacer un message et une politique inchangée
6. **Mises à niveau du schéma :** Transition vers la version de schéma d'actifs de gouvernance modifiée (version `asset_definition.v2`) et sérialisation du manifeste d'outils CLI et `confidential_policy` nécessitent une version ultérieure ہے۔ Les opérateurs de documents de mise à niveau Genesis et les validateurs redémarrent les paramètres de stratégie et les empreintes digitales du registre ajoutent des informations supplémentaires.Confidentialité activée pour les réseaux politiques souhaitées pour la genèse et l'encodage pour les réseaux Pour lancer et changer de mode, vous devez également suivre la liste de contrôle et suivre les fenêtres de conversion déterministes et les portefeuilles pour ajuster les portefeuilles. کا وقت ہو۔

### Norito Versionnement et activation du manifeste- Genesis manifeste DOIT `confidential_registry_root` clé personnalisée et `SetParameter` inclure کریں۔ Payload Norito JSON et `ConfidentialRegistryMeta { vk_set_hash: Option<String> }` correspondent aux champs suivants : l'entrée du vérificateur est omise dans le champ (`null`) est une chaîne hexadécimale de 32 octets (`0x…`) pour le manifeste et les instructions du vérificateur pour `compute_vk_set_hash` pour le hachage et le hachage. Paramètre manquant ou incompatibilité de hachage lors du démarrage des nœuds
- Version de mise en page du manifeste `ConfidentialFeatureDigest::conf_rules_version` intégrée intégrée réseaux v1 en ligne `Some(1)` MUST en ligne `iroha_config::parameters::defaults::confidential::RULES_VERSION` en ligne L'ensemble de règles évolue et le changement de bosse constant manifeste la régénération des fichiers binaires et le verrouillage des étapes et le déploiement. les versions mélangent les validateurs `ConfidentialFeatureDigestMismatch` les blocs rejettent les blocs
- Les manifestes d'activation, les mises à jour du registre, les modifications du cycle de vie des paramètres et l'ensemble de transitions de politiques DEVRAIT être un résumé cohérent :
  1. Les mutations planifiées du registre (`Publish*`, `Set*Lifecycle`) et la vue de l'état hors ligne s'appliquent également à `compute_confidential_feature_digest` pour le calcul du résumé post-activation.
  2. Le hachage calculé `SetParameter::custom(confidential_registry_root, {"vk_set_hash": "0x…"})` émet des pairs en retard, les instructions de registre intermédiaires manquent et corrigent le résumé, récupèrent et récupèrent.
  3. Les instructions `ScheduleConfidentialPolicyTransition` ajoutent کریں۔ ہر instruction کو citation `transition_id` émise par la gouvernance کرنا لازم ہے؛ Le système manifeste le rejet de l'exécution.4. Octets du manifeste, empreinte digitale SHA-256 et plan d'activation pour le résumé et le résumé Les opérateurs vérifient les artefacts et votent pour les partitions.
- Déploiement du basculement différé pour la hauteur cible et paramètre personnalisé compagnon et enregistrement (`custom.confidential_upgrade_activation_height`) les auditeurs et les preuves codées Norito ainsi que les validateurs et les changements de résumé ainsi que la fenêtre de notification honneur## Cycle de vie du vérificateur et des paramètres
### Registre ZK
- Ledger `ZkVerifierEntry { vk_id, circuit_id, version, proving_system, curve, public_inputs_schema_hash, vk_hash, vk_len, max_proof_bytes, gas_schedule_id, activation_height, deprecation_height, withdraw_height, status, metadata_uri_cid, vk_bytes_cid }` pour le livre `proving_system` pour le `Halo2` pour le livre fixe
- `(circuit_id, version)` paires uniques au monde métadonnées du circuit de registre recherche recherche index secondaire Paire en double رجسٹر کرنے کی کوشش admission پر rejeter ہوتی ہے۔
- `circuit_id` non vide et `public_inputs_schema_hash` non vide (il s'agit d'un vérificateur ou d'un codage canonique à entrée publique par Blake2b-32) hachage)۔ Admission ان فیلڈز کے بغیر records rejet کرتا ہے۔
- Instructions de gouvernance میں شامل ہیں :
  - `PUBLISH`, entrée `Proposed` réservée aux métadonnées uniquement, ajout de کرنے کیلئے۔
  - `ACTIVATE { vk_id, activation_height }`, limite d'époque et calendrier d'activation
  - `DEPRECATE { vk_id, deprecation_height }`, marque de hauteur pour l'entrée des épreuves et référence pour la référence
  - `WITHDRAW { vk_id, withdraw_height }`, arrêt d'urgence Les actifs retirent la hauteur et le gel des dépenses confidentielles et les entrées sont activées.
- Genesis manifeste le paramètre personnalisé `confidential_registry_root` et émet des entrées actives `vk_set_hash` qui correspondent aux entrées actives. nœud de validation کے consensus join سے پہلے digest کو état du registre local کے خلاف recoupement کرتی ہے۔- Registre des vérificateurs et mise à jour `gas_schedule_id` ضروری ہے؛ vérification appliquer l'entrée de registre `Active` et l'index `(circuit_id, version)` pour les preuves Halo2 et `OpenVerifyEnvelope` pour les preuves Halo2 `circuit_id`, `vk_hash`, et `public_inputs_schema_hash` enregistrement de registre correspondent à کرے۔

### Prouver les clés
- Prouver les clés hors grand livre et les identifiants adressés par le contenu (`pk_cid`, `pk_hash`, `pk_len`) et se référer à la page suivante. Les métadonnées du vérificateur sont publiées et publiées.
- Les données PK des SDK de portefeuille récupèrent et les hachages vérifient le cache local et le cache local.

### Paramètres de Pedersen et Poséidon
- Les contrôles du cycle de vie du vérificateur des registres (`PedersenParams`, `PoseidonParams`) mettent en miroir les contrôles des registres et les valeurs `params_id`, les générateurs/constantes et les hachages, les hauteurs d'activation/dépréciation/retrait ہوتے ہیں۔## Ordre déterministe et annulateurs
- L'actif `CommitmentTree` est utilisé pour `next_leaf_index`. bloque les engagements et l'ordre déterministe et ajoute le mot : ordre de blocage et les transactions itèrent. Transactions et sorties sérialisées `output_idx` ascendantes et sorties blindées itérer
- Les décalages d'arbre `note_position` dérivent d'un nullificateur et **نہیں**؛ یہ صرف témoin de preuve میں chemins d'adhésion کے لئے استعمال ہوتا ہے۔
- Reorgs est la conception du PRF de stabilité de l'annuleur et garantit la garantie. Entrée PRF `{ nk, note_preimage_hash, asset_id, chain_id, params_id }` lier les ancres aux racines Merkle et référencer les liens vers `max_anchor_age_blocks` pour les racines## Flux du grand livre
1. **MintConfidential {asset_id, montant, destinataire_hint }**
   - `Convertible` et `ShieldedOnly` politique d'actifs contrôle d'autorité des actifs d'admission et actuel `params_id` récupérer et `rho` échantillon et engagement émettre et mise à jour de l'arbre Merkle
   - `ConfidentialEvent::Shielded` émet un nouvel engagement, un delta racine de Merkle et une piste d'audit et un hachage d'appel de transaction.
2. **TransferConfidential {asset_id, proof, circuit_id, version, nullifiers, new_commitments, enc_payloads, Anchor_root, memo }**
   - Entrée de registre VM syscall et vérification de la preuve hôte یقینی بناتا ہے کہ annulateurs inutilisés ہوں، engagements déterministes طور پر ajouter ہوں، اور ancre récente ہو۔
   - Les entrées du grand livre `NullifierSet` enregistrent les destinataires/auditeurs et le magasin de charges utiles cryptées et `ConfidentialEvent::Transferred` émettent des informations et des annuleurs, des sorties ordonnées, un hachage de preuve, et des annuleurs. Les racines de Merkle résument کرتا ہے۔
3. **RevealConfidential {asset_id, preuve, circuit_id, version, nullifier, montant, destinataire_account, Anchor_root }**
   - Voir les actifs `Convertible` preuve valider کرتا ہے کہ valeur de la note montant révélé کے برابر ہے، grand livre transparent solde crédit کرتا ہے، اور annulateur marque dépensée کر کے note protégée brûler کرتا ہے۔- `ConfidentialEvent::Unshielded` émet des montants publics, des annulateurs consommés, des identifiants de preuve et un hachage d'appel de transaction.## Ajouts de modèles de données
- `ConfidentialConfig` (nouvelle section de configuration) avec indicateur d'activation, `assume_valid`, boutons de gaz/limite, fenêtre d'ancrage, backend de vérification.
- `ConfidentialNote`, `ConfidentialTransfer`, et `ConfidentialMint` Norito octet de version explicite des schémas (`CONFIDENTIAL_ASSET_V1 = 0x01`)
- `ConfidentialEncryptedPayload` octets de mémo AEAD et `{ version, ephemeral_pubkey, nonce, ciphertext }` envelopper les octets par défaut `version = CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1` disposition XChaCha20-Poly1305
- Vecteurs canoniques de dérivation de clé `docs/source/confidential_key_vectors.json` میں ہیں؛ CLI et point de terminaison Torii pour les appareils et la régression en cours
- `asset::AssetDefinition` et `confidential_policy: AssetConfidentialPolicy { mode, vk_set_hash, poseidon_params_id, pedersen_params_id, pending_transition }` en anglais
- Les vérificateurs de transfert/non-blindage `ZkAssetState` et la liaison `(backend, name, commitment)` persistent. exécution et preuves et rejet et référencement en ligne de la clé de vérification de l'engagement enregistré et correspondance avec les preuves
- `CommitmentTree` (par actif avec points de contrôle frontaliers), `NullifierSet` saisi par `(chain_id, asset_id, nullifier)`, `ZkVerifierEntry`, `PedersenParams`, `PoseidonParams` magasin d'état mondial ہوتے ہیں۔
- Détection précoce des doublons Mempool et vérification de l'âge d'ancrage des structures transitoires `NullifierIndex` et `AnchorIndex` maintiennent les structures transitoires
- Mises à jour du schéma Norito avec entrées publiques et ordre canonique les tests aller-retour codant le déterminisme garantissent کرتے ہیں۔- Encrypted payload roundtrips unit tests (`crates/iroha_data_model/src/confidential.rs`) کے ذریعے lock ہیں۔ Follow-up wallet vectors auditors کیلئے canonical AEAD transcripts attach کریں گے۔ `norito.md` envelope کیلئے on-wire header document کرتا ہے۔

## IVM Intégration et appel système
- `VERIFY_CONFIDENTIAL_PROOF` syscall introduce کریں جو قبول کرتا ہے:
  - `circuit_id`, `version`, `scheme`, `public_inputs`, `proof` اور resultant `ConfidentialStateDelta { asset_id, nullifiers, commitments, enc_payloads }`.
  - Registre Syscall et chargement des métadonnées du vérificateur et limites de taille/durée appliquées et charge de gaz déterministe et preuve de succès et application delta.
- Host read-only `ConfidentialLedger` trait expose کرتا ہے جو Merkle root snapshots اور nullifier status retrieve کرتا ہے؛ Kotodama library witness assembly helpers اور schema validation فراہم کرتی ہے۔
- Pointer-ABI docs proof buffer layout اور registry handles واضح کرنے کیلئے update ہوئے ہیں۔## Négociation des capacités du nœud
- Poignée de main `feature_bits.confidential` et `ConfidentialFeatureDigest { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }` annoncent le produit. Participation du validateur `confidential.enabled=true`, `assume_valid=false`, identifiants backend du vérificateur identiques et résumés correspondants incompatibilités `HandshakeConfidentialMismatch` en cas d'échec de la poignée de main
- Configuration des nœuds d'observation pour le support `assume_valid` et : désactivé pour les instructions confidentielles et le déterministe `UnsupportedInstruction` pour éviter la panique. activé ہونے پر les preuves des observateurs vérifient کئے بغیر les deltas d'état s'appliquent کرتے ہیں۔
- Les transactions confidentielles Mempool sont rejetées et la capacité locale est désactivée. Gossip filtre les pairs sans capacités de correspondance, ainsi que les transactions protégées et les identifiants de vérificateurs inconnus, ainsi que les limites de taille et les informations aveugles.

### Révéler la politique d'élagage et de conservation des annulateurs

Grands livres confidentiels et fraîcheur des notes et audits axés sur la gouvernance, relecture et historique, conservation des données `ConfidentialLedger` et politique par défaut :- **Rétention des annulateurs :** les annulateurs dépensés sont également `730` (24 millions) de hauteur de dépense pour le régulateur et le régulateur. et Opérateurs `confidential.retention.nullifier_days` pour l'extension de la fenêtre fenêtre de rétention pour les annulateurs Torii pour les absences interrogeables des auditeurs pour double dépense
- **Révéler l'élagage :** transparent révèle (`RevealConfidential`) pour noter les engagements et la finalisation du bloc et pour élaguer l'annulateur consommé et pour la règle de rétention. رہتا ہے۔ Événements liés à la révélation (`ConfidentialEvent::Unshielded`) montant public, destinataire et enregistrement de hachage de preuve, révélations historiques, reconstruction, texte chiffré élagué, texte chiffré élagué
- **Points de contrôle frontaliers :** points de contrôle roulants aux frontières d'engagement رکھتے ہیں جو `max_anchor_age_blocks` اور fenêtre de rétention میں سے بڑے کو cover کرتے ہیں۔ Les nœuds sont des points de contrôle qui sont compacts et un intervalle et les annuleurs expirent.
- **Remédiation de digestion obsolète :** La dérive de digestion est terminée et `HandshakeConfidentialMismatch` est un opérateur de cluster (1) et les fenêtres de rétention de l'annuleur de cluster s'alignent et les fenêtres de rétention de l'annuleur sont alignées. (2) `iroha_cli app confidential verify-ledger` a conservé l'ensemble d'annuleurs pour digérer la régénération et (3) redéploiement du manifeste actualisé. Annulateurs élagués prématurément et adhésion au réseau, stockage frigorifique et restaurationRemplacements locaux et runbook d'opérations et document fenêtre de rétention et politiques de gouvernance et configuration des nœuds et plans de stockage d'archives et verrouillage et mise à jour et mise à jour

### Flux d'expulsion et de récupération

1. Composez le numéro `IrohaNetwork` et comparez les capacités annoncées. inadéquation `HandshakeConfidentialMismatch` augmenter کرتا ہے؛ connexion en ligne pour la file d'attente de découverte des pairs en ligne pour `Ready`
2. Journal de service réseau de défaillance en surface (résumé à distance et back-end) et Sumeragi peer et proposition et vote et calendrier de vote
3. Les registres de vérification des opérateurs et les ensembles de paramètres (`vk_set_hash`, `pedersen_params_id`, `poseidon_params_id`) s'alignent sur l'étape `next_conf_features` convenue. کے remédiation کرتے ہیں۔ Digest match ہوتے ہی اگلا poignée de main خودکار طور پر réussir کرتا ہے۔
4. Les pairs obsolètes bloquent la diffusion (pour la relecture d'archives) et les validateurs `BlockRejectionReason::ConfidentialFeatureDigestMismatch` sont déterministes et rejettent. État du grand livre du réseau cohérent

### Flux de négociation sécurisé pour la relecture1. ہر tentative sortante nouveau matériel clé Noise/X25519 allouer کرتا ہے۔ Charge utile de poignée de main signée (`handshake_signature_payload`) clés publiques éphémères locales et distantes, adresse de socket annoncée codée en Norito, et `handshake_chain_id` pour compiler et concaténer l'identifiant de chaîne. ہے۔ Nœud de message سے نکلنے سے پہلے AEAD crypté ہوتی ہے۔
2. Ordre des clés locales/homologues du répondeur et inversement pour le recalcul de la charge utile par `HandshakeHelloV1` avec vérification de la signature Ed25519 intégrée. Il y a des clés éphémères et un domaine de signature d'adresse annoncé et un message capturé est un message déterministe ou un échec de relecture. ہوتا ہے۔
3. Indicateurs de capacité confidentielle اور `ConfidentialFeatureDigest` `HandshakeConfidentialMeta` pour les voyages et les voyages Récepteur `{ enabled, assume_valid, verifier_backend, digest }` tuple et local `ConfidentialHandshakeCaps` pour comparer les valeurs inadéquation avec `HandshakeConfidentialMismatch` pour une sortie anticipée
4. Les opérateurs digèrent (`compute_confidential_feature_digest` کے ذریعے) recalculent les registres/politiques mis à jour et les nœuds redémarrent et DOIVENT ہے۔ Les anciens résumés annoncent que la poignée de main des pairs échoue et que l'état est obsolète et que le validateur est défini.5. Les compteurs standard `iroha_p2p::peer` (`handshake_failure_count`) mettent à jour les réussites/échecs de la prise de contact et les entrées de journal structurées émettent des identifiants d'homologues distants et des balises d'empreinte digitale de synthèse. ہیں۔ Les indicateurs et le moniteur de déploiement, les tentatives de relecture et les erreurs de configuration sont également présents.

## Gestion des clés et charges utiles
- Hiérarchie de dérivation des clés par compte :
  - `sk_spend` → `nk` (clé d'annulation), `ivk` (clé de visualisation entrante), `ovk` (clé de visualisation sortante), `fvk`.
- Charges utiles de notes chiffrées AEAD pour les clés partagées dérivées de l'ECDH et les clés partagées dérivées de l'ECDH. facultatif, l'auditeur voit les clés de la politique d'actifs et les sorties sont incluses dans la pièce jointe.
- Ajouts CLI : `confidential create-keys`, `confidential send`, `confidential export-view-key`, les mémos décryptent les outils d'auditeur, ainsi que l'assistant `iroha app zk envelope` et les enveloppes mémo Norito hors ligne. produire/inspecter کرتا ہے۔ Torii `POST /v1/confidential/derive-keyset` flux de dérivation et flux de dérivation pour les formulaires hex/base64 et les portefeuilles les hiérarchies de clés par programme récupèrent کر سکیں۔## Contrôles de gaz, limites et DoS
- Programme de gaz déterministe :
  - Halo2 (Plonkish) : base gaz `250_000` + gaz `2_000` par entrée publique.
  - Gaz `5` par octet de preuve, plus frais par annulateur (`300`) et par engagement (`500`)
  - Opérateurs et constantes de configuration du nœud (`confidential.gas.{proof_base, per_public_input, per_proof_byte, per_nullifier, per_commitment}`) pour remplacer les paramètres de configuration changements de démarrage et de rechargement à chaud de la configuration et propagation et cluster et déterministe et application et application
- Limites strictes (valeurs par défaut configurables) :
-`max_proof_size_bytes = 262_144`.
-`max_nullifiers_per_tx = 8`, `max_commitments_per_tx = 8`, `max_confidential_ops_per_block = 256`.
- `verify_timeout_ms = 750`, `max_anchor_age_blocks = 10_000`. `verify_timeout_ms` سے زیادہ preuves instruction کو déterministe طور پر abort کرتے ہیں (les bulletins de vote de gouvernance `proof verification exceeded timeout` émettent کرتے ہیں، `VerifyProof` retour d'erreur کرتا ہے)۔
- Des quotas supplémentaires garantissent la vivacité des blocs : `max_proof_bytes_block`, `max_verify_calls_per_tx`, `max_verify_calls_per_block`, et `max_public_inputs`, les constructeurs de blocs et les blocs liés. `reorg_depth_bound` (≥ `max_anchor_age_blocks`) la rétention aux points de contrôle frontaliers régit کرتا ہے۔
- Le temps d'exécution par transaction et les limites par bloc dépassent les transactions et les transactions sont rejetées et les erreurs déterministes `InvalidParameter` émettent un état du grand livre inchangé.
- Mempool `vk_id`, longueur de preuve et âge d'ancrage pour les transactions confidentielles et préfiltre pour le vérificateur invoquent l'utilisation des ressources limitée par l'utilisation des ressources.- Vérification déterministe et délai d'attente, violation de limite et arrêt. transactions erreurs explicites کے ساتھ échec ہوتی ہیں۔ Backends SIMD facultatifs pour la comptabilité du gaz modifier les détails

### Lignes de base d'étalonnage et portes d'acceptation
- **Plateformes de référence.** Les exécutions d'étalonnage et les profils matériels couvrent MUST ہے۔ اگر سب profiles capture نہ ہوں تو review rejet کر دے۔

  | Profil | Architecture | Processeur/Instance | Drapeaux du compilateur | Objectif |
  | --- | --- | --- | --- | --- |
  | `baseline-simd-neutral` | `x86_64` | AMD EPYC 7B12 (32c) et Intel Xeon Gold 6430 (24c) | `RUSTFLAGS="-C target-feature=-avx,-avx2,-fma"` | les intrinsèques du vecteur et les valeurs planchers établissent les valeurs fondamentales les tables de coûts de repli s'accordent |
  | `baseline-avx2` | `x86_64` | Intel Xeon Or 6430 (24c) | version par défaut | Validation du chemin AVX2 ici Le SIMD accélère la tolérance aux gaz neutres. |
  | `baseline-neon` | `aarch64` | AWS Graviton3 (c7g.4xlarge) | version par défaut | Backend NEON et planifications déterministes et x86 et planifications alignées entre elles |

- **Harnais de référence.** Les rapports d'étalonnage des gaz DOIVENT être :
  -`CRITERION_HOME=target/criterion cargo bench -p iroha_core isi_gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>`
  - Confirmation du luminaire déterministe `cargo test -p iroha_core bench_repro -- --ignored`
  - `CRITERION_HOME=target/criterion cargo bench -p ivm gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>` pour les coûts de l'opcode VM- ** Caractère aléatoire fixe. ** bancs d'exportation `IROHA_CONF_GAS_SEED=conf-gas-seed-2026Q1` export `iroha_test_samples::gen_account_in` chemin déterministe `KeyPair::from_seed` et commutateur Harnais pour `IROHA_CONF_GAS_SEED_ACTIVE=…` print کرتا ہے؛ variable manquante et la révision DOIT échouer De nouveaux utilitaires d'étalonnage pour le caractère aléatoire auxiliaire introduisent des fonctions et des paramètres d'env var et d'honneur.

- **Capture des résultats.**
  - Résumés des critères (`target/criterion/**/raw.csv`) et profil de sortie d'artefact et téléchargement de fichiers
  - Métriques dérivées (`ns/op`, `gas/op`, `ns/gas`) et [Ledger confidentiel d'étalonnage des gaz] (./confidential-gas-calibration) avec git commit et version du compilateur dans le magasin کریں۔
  - Le profil est défini par les lignes de base et les lignes de base نئی rapport valider ہونے کے بعد پرانی instantanés supprimer کریں۔

- **Tolérances d'acceptation.**
  - `baseline-simd-neutral` et `baseline-avx2` pour des deltas de gaz ≤ ±1,5 %
  - `baseline-simd-neutral` et `baseline-neon` pour des deltas de gaz ≤ ±2,0 %
  - Il y a des seuils, des propositions d'étalonnage et des propositions d'étalonnage, des ajustements de calendrier et des écarts/atténuations, ainsi que des propositions de RFC.- **Liste de contrôle de révision.** Soumissionnaires ذمہ دار ہیں :
  - Extraits `uname -a`, `/proc/cpuinfo` (modèle, étape) et journal d'étalonnage `rustc -Vv`.
  - sortie du banc `IROHA_CONF_GAS_SEED` et écho du système d'impression (bancs d'impression de graines actives)
  - stimulateur cardiaque et vérificateur confidentiel, production de drapeaux et correspondance (`--features confidential,telemetry` pour télémétrie et bancs)

## Configuration et opérations
- `iroha_config` et `[confidential]` sont disponibles :
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
- Les métriques globales de télémétrie émettent des éléments : `confidential_proof_verified`, `confidential_verifier_latency_ms`, `confidential_proof_bytes_total`, `confidential_nullifier_spent`, `confidential_commitments_appended`, `confidential_mempool_rejected_total{reason}`, اور `confidential_policy_transitions_total`, les données en texte clair exposent les données
- Surfaces RPC :
  -`GET /confidential/capabilities`
  -`GET /confidential/zk_registry`
  -`GET /confidential/params`## Stratégie de test
- Déterminisme : blocs et transactions aléatoires mélangeant des racines de Merkle identiques et des ensembles d'annuleurs qui produisent des résultats différents.
- Résilience de réorganisation : les ancres et les réorganisations multiblocs simulent des réorganisations multiblocs. annuleurs stables et ancres périmées rejetées
- Invariants de gaz : accélération SIMD entre les nœuds et vérification de l'utilisation du gaz identique.
- Tests de limites : plafonds de taille/gaz, épreuves, nombre maximum d'entrées/sorties, application du délai d'attente.
- Cycle de vie : vérificateur et activation/dépréciation des paramètres, opérations de gouvernance, tests de dépenses en rotation
- Politique FSM : transitions autorisées/non autorisées, délais de transition en attente, hauteurs effectives et rejet du pool de mémoire.
- Urgences du registre : retrait d'urgence `withdraw_height` pour gel des avoirs concernés کرتا ہے اور اس کے بعد preuves rejet کرتا ہے۔
- Contrôle des capacités : les blocs de validateurs `conf_features` et les validateurs incompatibles rejettent les erreurs. `assume_valid=true` et consensus des observateurs sur la synchronisation des données
- Équivalence d'état : la chaîne canonique des nœuds validateurs/complets/observateurs et les racines d'état identiques produisent des liens
- Fuzzing négatif : preuves mal formées, charges utiles surdimensionnées, collisions d'annuleurs déterministes et rejets de données.## Travail exceptionnel
- Jeux de paramètres Halo2 (taille du circuit, stratégie de recherche) benchmark pour le playbook d'étalonnage et la mise à jour des paramètres par défaut de gaz/d'expiration `confidential_assets_calibration.md`.
- Les politiques de divulgation de l'auditeur et les API de visualisation sélective finalisent l'approbation du projet de gouvernance et le flux de travail approuvé par Torii par fil.
- Schéma de cryptage des témoins pour les sorties multi-destinataires et les mémos groupés pour étendre les implémenteurs du SDK pour les documents au format enveloppe
- Circuits, registres, procédures de rotation des paramètres, commission externe d'examen de la sécurité, conclusions, rapports d'audit interne, archives
- Les API de réconciliation des dépenses de l'auditeur précisent les orientations sur la portée de la clé de vue et publient les fournisseurs de portefeuilles et la sémantique d'attestation implémentent les éléments de preuve.## Phase de mise en œuvre
1. **Phase M0 — Durcissement d'arrêt du navire**
   - ✅ La dérivation de l'annulateur pour la conception Poséidon PRF (`nk`, `rho`, `asset_id`, `chain_id`) suit les mises à jour du grand livre et l'ordre déterministe des engagements à appliquer. ہے۔
   - ✅ Les plafonds de taille de preuve d'exécution et les quotas confidentiels par transaction/par bloc appliquent les transactions dépassant le budget et les erreurs déterministes et le rejet des erreurs.
   - ✅ Poignée de main P2P `ConfidentialFeatureDigest` (résumé du backend + empreintes digitales du registre) annonce des incompatibilités entre les deux et `HandshakeConfidentialMismatch` en cas d'échec déterministe
   - ✅ Chemins d'exécution confidentiels pour les paniques, supprimer les nœuds non pris en charge et le contrôle de rôle ajouter les nœuds non pris en charge
   - ⚪ Les budgets de délai d'attente du vérificateur et les points de contrôle frontaliers et les limites de profondeur de réorganisation sont appliquées.
     - ✅ Les budgets de délai d'attente de vérification sont appliqués ہوئے؛ `verify_timeout_ms` سے تجاوز کرنے والی preuves d'échec déterministe ہوتی ہیں۔
     - ✅ Points de contrôle frontaliers `reorg_depth_bound` respecter la fenêtre configurée et les points de contrôle élaguer les instantanés déterministes des points de contrôle
   - `AssetConfidentialPolicy`, politique FSM, et instructions de création/transfert/révélation pour les portes d'application introduisent کریں۔
   - Les en-têtes de bloc `conf_features` commit et les résumés de registre/paramètre divergent et la participation du validateur refuse.
2. **Phase M1 — Registres et paramètres**- `ZkVerifierEntry`, `PedersenParams`, `PoseidonParams` opérations de gouvernance des registres, ancrage de la genèse et gestion du cache pour les terres agricoles
   - Syscall pour les recherches dans le registre, les identifiants de programme de gaz, le hachage de schéma et les vérifications de taille nécessitent un câble de connexion.
   - Format de charge utile cryptée v1, vecteurs de dérivation de clés de portefeuille, gestion des clés confidentielles et support CLI.
3. **Phase M2 — Gaz et performances**
   - Calendrier de gaz déterministe, compteurs par bloc, télémétrie et faisceaux de référence implémentés (vérifier la latence, tailles de preuve, rejets de pool de mémoire)
   - Points de contrôle CommitmentTree, chargement LRU, indices d'annulation et charges de travail multi-actifs qui durcissent
4. **Phase M3 — Outillage de rotation et de portefeuille**
   - L'acceptation de la preuve multi-paramètres et multi-versions permet activation/dépréciation basée sur la gouvernance et runbooks de transition et prise en charge
   - Les flux de migration du SDK/CLI du portefeuille, les flux de travail d'analyse des auditeurs et les outils de réconciliation des dépenses fournissent des informations supplémentaires.
5. **Phase M4 — Audit et opérations**
   - Workflows clés de l'auditeur, API de divulgation sélective et runbooks opérationnels.
   - Calendrier d'examen externe de la cryptographie/sécurité et des résultats `status.md` à publier

Étapes de la feuille de route de phase et mise à jour des tests de mise à jour et réseau blockchain et garanties d'exécution déterministe.
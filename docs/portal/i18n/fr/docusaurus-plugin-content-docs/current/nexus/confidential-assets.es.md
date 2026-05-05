---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/confidential-assets.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : Actifs confidentiels et transferts ZK
description : Plan de Phase C pour circulation aveugle, registres et contrôles de l'opérateur.
slug : /nexus/actifs-confidentiels
---
<!--
SPDX-License-Identifier: Apache-2.0
-->
# Diseno des actifs confidentiels et transferts ZK

## Motivation
- Entrer des flux d'activités aveugles opt-in pour que les domaines préservent la confidentialité transactionnelle sans modifier la circulation transparente.
- Mantener l'exécution déterminée sur le matériel hétérogène des validateurs et conserver Norito/Kotodama ABI v1.
- Démontrer les contrôles des auditeurs et des opérateurs du cycle de vie (activation, rotation, révocation) pour les circuits et les paramètres cryptographiques.

## Modèle d'amendes
- Les validateurs sont honnêtes mais curieux : ejecutan consenso fielmente pero intentan inspeccionar ledger/state.
- Les Observadores de red ven datos de bloque y transacciones ont bavardé ; on ne se sert pas de canaux de potins privés.
- Fuera de alcance: analyse du trafic hors grand livre, adversaires quantiques (suivi de la feuille de route PQ), attaques de disponibilité du grand livre.## Résumé du projet
- Les actifs peuvent déclarer une *piscine blindée* en plus des équilibres transparents existants ; la circulation aveugle se représente via des engagements cryptographiques.
- Les notes encapsulées `(asset_id, amount, recipient_view_key, blinding, rho)` avec :
  - Engagement : `Comm = Pedersen(params_id || asset_id || amount || recipient_view_key || blinding)`.
  - Annulateur : `Null = Poseidon(domain_sep || nk || rho || asset_id || chain_id)`, indépendant de l'ordre des notes.
  - Charge utile encodée : `enc_payload = AEAD_XChaCha20Poly1305(ephemeral_shared_key, note_plaintext)`.
- Les transactions transportant des charges utiles `ConfidentialTransfer` codifiées avec Norito qui contiennent :
  - Entrées publiques : ancre Merkle, annuleurs, nouveaux engagements, identifiant d'actif, version de circuit.
  - Charges utiles écrites pour les destinataires et les auditeurs optionnels.
  - Prueba zero-knowledge que atesta conservacion de valor, property y autorisation.
- Vérification des clés et des conjuntos de parametros son controlados mediante registros on-ledger con ventanas de activacion ; los nodos rechazan valider les preuves que referencian entradas desconocidas ou revocadas.
- Les en-têtes de consensus compromettent le résumé activé des capacités confidentielles pour que les blocs soient seuls acceptés lorsque l'état des enregistrements et les paramètres coïncident.
- La construction de preuves utilise une pile Halo2 (Plonkish) sans configuration fiable ; Groth16 et d'autres variantes de SNARK sont intentionnellement prises en charge dans la v1.

### Calendriers déterministesLes informations sur le mémo confidentiel incluent désormais un appareil canonique en `fixtures/confidential/encrypted_payload_v1.json`. L'ensemble de données capture un sujet v1 positif, mais il y a beaucoup de négatifs malformés pour que les SDK puissent confirmer la parité d'analyse. Les tests du modèle de données en Rust (`crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs`) et la suite de Swift (`IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift`) effectuent directement le montage, garantissant que l'encodage Norito, la surface d'erreur et la couverture de régression peuvent être alignées de manière permanente lors de l'évolution du codec.

Les SDK de Swift peuvent désormais émettre des instructions Shield sans colle JSON sur mesure : construire un
`ShieldRequest` avec l'engagement de 32 octets, la charge utile encryptée et les métadonnées de débit,
luego lama `IrohaSDK.submit(shield:keypair:)` (ou `submitAndWait`) pour annoncer et envoyer la
transaction sur `/v1/pipeline/transactions`. El helper valide les longitudes de l'engagement,
Enhebra `ConfidentialEncryptedPayload` et l'encodeur Norito, et reflète la disposition `zk::Shield`
décrit ci-dessous pour que les portefeuilles soient synchronisés avec Rust.## Engagements de consensus et contrôle des capacités
- Les en-têtes de blocage exponen `conf_features = { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }` ; le résumé participe au hachage de consensus et doit être identique à la vue locale du registre pour accepter le blocage.
- La gouvernance peut préparer des mises à niveau programmées avec `next_conf_features` avec un futur `activation_height` ; Jusqu'à cette hauteur, les producteurs de blocs doivent émettre le résumé précédent.
- Les nœuds validateurs DEBEN fonctionnent avec `confidential.enabled = true` et `assume_valid = false`. Les chèques de départ sont rechazan unirse al set de validadores si cualquiera falla o si el `conf_features` diverge local.
- Les métadonnées de la poignée de main P2P incluent désormais `{ enabled, assume_valid, conf_features }`. Les pairs qui annoncent les caractéristiques ne sont pas pris en charge par `HandshakeConfidentialMismatch` et n'entrent pas en rotation de consensus.
- Les résultats de la poignée de main entre les validateurs, les observateurs et les pairs sont capturés dans la matrice de poignée de main inférieure [Négociation des capacités du nœud] (#node-capability-negotiation). Les chutes de poignée de main exposent `HandshakeConfidentialMismatch` et restent entre pairs hors de la rotation de consensus jusqu'à ce que votre digestion coïncide.
- Observers no validadores pueden fijar `assume_valid = true` ; Les applications deltas confidentielles à ciegas mais n'ont aucune influence sur la sécurité du consentement.## Politique des actifs
- Chaque définition d'actif relève un `AssetConfidentialPolicy` fixé par le créateur ou via la gouvernance :
  - `TransparentOnly` : modo par défaut ; solo se permiten instrucciones transparentes (`MintAsset`, `TransferAsset`, etc.) et les opérations protégées se rechazan.
  - `ShieldedOnly` : toda emision y transferencias deben usar instrucciones confidentielles ; `RevealConfidential` est interdit pour que les soldes ne soient pas publiés publiquement.
  - `Convertible` : les supports peuvent déplacer la valeur entre les représentations transparentes et blindées en utilisant les instructions de rampe d'accès/sortie d'en bas.
- La politique maintient un FSM restreint pour éviter les fonds variables :
  - `TransparentOnly -> Convertible` (habilitation immédiate de la piscine protégée).
  - `TransparentOnly -> ShieldedOnly` (nécessite une transition pendante et une fenêtre de conversion).
  - `Convertible -> ShieldedOnly` (démora minima obligatoire).
  - `ShieldedOnly -> Convertible` (nécessite un plan de migration pour que les notes aveugles sigan siendo gastables).
  - `ShieldedOnly -> TransparentOnly` ne permet pas que le pool protégé soit vide ou que la gouvernance codifie une migration que des notes aveugles pendantes.- Les instructions de gouvernance suivent `pending_transition { new_mode, effective_height, previous_mode, transition_id, conversion_window }` via l'ISI `ScheduleConfidentialPolicyTransition` et peuvent interrompre les changements programmés avec `CancelConfidentialPolicyTransition`. La validation du mémoire assure que n'importe quelle transaction augmente la hauteur de transition et l'inclusion tombe de forme déterministe si un contrôle de politique change avec le blocage.
- Les transitions pendantes s'appliquent automatiquement lorsqu'un nouveau blocage s'ouvre : une fois que la hauteur entre dans la fenêtre de conversion (pour les mises à niveau `ShieldedOnly`) ou l'emplacement `effective_height`, le runtime actualise `AssetConfidentialPolicy`, actualise les métadonnées. `zk.policy` et nettoyer l’entrée pendante. Si vous fournissez de manière transparente lorsque vous effectuez une transition `ShieldedOnly`, le runtime interrompt le changement et enregistre une annonce, en laissant le mode précédent intact.
- Les boutons de configuration `policy_transition_delay_blocks` et `policy_transition_window_blocks` permettent d'obtenir un avis minimal et des périodes de grâce pour permettre la conversion des portefeuilles lors du changement.
- `pending_transition.transition_id` fonctionne également comme poignée de salle ; la gouvernance doit être citarlo al finalizar o annular transitions para que los operadors correlacionen rapportes de on/off-ramp.
- `policy_transition_window_blocks` par défaut à 720 (environ 12 heures avec un temps de bloc de 60 s). Los nodos limitan solicitudes de gouvernance qui intenten avisos mas cortos.- Genesis manifeste et flujos CLI expose la politique actuelle et pendante. La logique d'admission est la politique au moment de l'expulsion pour confirmer que chaque instruction confidentielle est autorisée.
- Liste de contrôle de migration - voir "Séquençage de la migration" ci-dessous pour le plan de mise à niveau pour les étapes qui suivent le Milestone M0.

#### Surveillance des transitions via Torii

Portefeuilles et auditeurs consultent `GET /v1/confidential/assets/{definition_id}/transitions` pour inspecter l'actif `AssetConfidentialPolicy`. La charge utile JSON inclut toujours l'identifiant d'actif canonique, la dernière hauteur de blocage observée, le `current_mode` de la politique, le mode efficace à cette hauteur (les fenêtres de conversion rapportées temporellement `Convertible`), et les identifiants attendus de `vk_set_hash`/Poséidon/Pedersen. Lorsque vous avez une transition pendant que la réponse comprend également :

- `transition_id` - poignée de salle conçue pour `ScheduleConfidentialPolicyTransition`.
-`previous_mode`/`new_mode`.
-`effective_height`.
- `conversion_window` et le dérivé `window_open_height` (le blocage des portefeuilles doit commencer la conversion pour les basculements ShieldedOnly).

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

Une réponse `404` indique qu'il n'existe pas de définition d'actif coïncidente. Lorsqu'aucune transition de foin n'est programmée sur le terrain `pending_transition` et `null`.

### Machine des états politiques| Mode actuel | Mode suivant | Prérequis | Maniement de hauteur efficace | Notes |
|----------------------------------------------------|-------------------------------------------------------------------------------------------------|----------------------------------------------------------------------------------------------------------------------------------|-----------------------------------------------------------------------------------------------|
| Transparent uniquement | Cabriolet | La gouvernance a activé les entrées du registre des vérificateurs/paramètres. Envoyer `ScheduleConfidentialPolicyTransition` avec `effective_height >= current_height + policy_transition_delay_blocks`. | La transition est exécutée exactement en `effective_height` ; la piscine protégée est disponible immédiatement.        | Par défaut, pour garantir la confidentialité, les flux transparents seront maintenus. |
| Transparent uniquement | Blindé uniquement | Igual que arriba, mas `policy_transition_window_blocks >= 1`.                                                       | Le runtime entre automatiquement dans `Convertible` et `effective_height - policy_transition_window_blocks` ; changer de `ShieldedOnly` et `effective_height`. | Prouvez une fenêtre de conversion déterminée avant de désactiver les instructions transparentes. || Cabriolet | Blindé uniquement | Transition programmée avec `effective_height >= current_height + policy_transition_delay_blocks`. Certificat DEBE de gouvernance (`transparent_supply == 0`) via métadonnées d'auditoire ; Le runtime est appliqué au cut-over. | Semantica de ventana identica a la anterior. Si l'alimentation transparente est sans zéro sur `effective_height`, la transition est interrompue avec `PolicyTransitionPrerequisiteFailed`. | Bloquea el Asset en circulation complètement confidentielle.                                |
| Blindé uniquement | Cabriolet | Transition programmée ; sans retrait d’émergence active (`withdraw_height` non défini).                            | L'état change en `effective_height` ; los révèlent les rampes se reabren mientras las notes blindadas siguen siendo validas. | Utilisé pour les fenêtres de maintenance ou les révisions des auditeurs.                            |
| Blindé uniquement | Transparent uniquement | La gouvernance doit être examinée `shielded_supply == 0` ou préparée par un plan `EmergencyUnshield` firmado (firmas de commissaire aux comptes requis). | Le runtime ouvre une fenêtre `Convertible` avant `effective_height` ; en ceci altura les instructions confidentielles tombent duro et l'actif vuelve en mode transparent uniquement. | Sortie du dernier recurso. La transition s'annule automatiquement si elle se produit en cas de note confidentielle pendant la vente. || N'importe quel | Identique à l'actuel | `CancelConfidentialPolicyTransition` Nettoyer le changement pendant.                                                    | `pending_transition` est éliminé immédiatement.                                                                     | Maintenir le statu quo ; affiché dans son intégralité.                                          |

Les transitions no listadas arriba se rechazane durante el soumission de gouvernance. Le runtime vérifie les prérequis juste avant d’appliquer une transition programmée ; Si vous tombez, développez l'actif de la manière précédente et émettez `PolicyTransitionPrerequisiteFailed` via la télémétrie et les événements de blocage.

### Sécurité de migration1. **Préparer les registres :** activer toutes les entrées de vérificateur et paramètres référencés pour l'objet politique. Les nœuds annoncent le résultat `conf_features` pour que les pairs vérifient la cohérence.
2. **Programmer la transition :** envoyer `ScheduleConfidentialPolicyTransition` avec un `effective_height` qui correspond à `policy_transition_delay_blocks`. Au déplacement vers `ShieldedOnly`, précisez une fenêtre de conversion (`window >= policy_transition_window_blocks`).
3. **Guide public pour les opérateurs :** enregistrez le `transition_id` en développant et en circulaire un runbook de rampe d'accès/sortie. Wallets et auditeurs s'abonnent au `/v1/confidential/assets/{id}/transitions` pour connaître la hauteur d'ouverture de la fenêtre.
4. **Utiliser la fenêtre :** lorsque vous ouvrez la fenêtre, le runtime change la politique en `Convertible`, émet `PolicyTransitionWindowOpened { transition_id }` et répond aux demandes de gouvernance en conflit.
5. **Finaliser ou abandonner :** dans `effective_height`, le runtime vérifie les prérequis de la transition (fournir un zéro transparent, sans urgence, etc.). Si pasa, changez la politique à la mode sollicitée ; si vous tombez, émettez `PolicyTransitionPrerequisiteFailed`, nettoyez la transition pendante et laissez la politique sans changement.6. **Mises à niveau du schéma :** après une transition exitosa, la gouvernance sous la version du schéma de l'actif (par exemple, `asset_definition.v2`) et l'outil CLI nécessitent `confidential_policy` pour sérialiser les manifestes. Les documents de mise à niveau de Genesis instruisent les opérateurs pour ajouter les paramètres politiques et les empreintes digitales du registre avant de réinitialiser les validateurs.

De nouvelles choses qui incitent à la confidentialité habilitée à codifier la politique souhaitée directement dans la genèse. Aussi, suivez la liste de contrôle antérieure au changement des modes post-lancement pour que les fenêtres de conversion soient déterminées et que les portefeuilles aient un temps d'ajustement.

### Version et activation du manifeste Norito- Genesis manifeste DEBEN incluant un `SetParameter` pour la clé personnalisée `confidential_registry_root`. La charge utile est Norito JSON qui ressemble à `ConfidentialRegistryMeta { vk_set_hash: Option<String> }` : omettre le champ (`null`) lorsque les entrées ne sont pas activées, ou prouver une chaîne hexadécimale de 32 octets (`0x...`) similaire au hachage produit par `compute_vk_set_hash` concernant les instructions du vérificateur envoyées dans le manifeste. Les nœuds ne peuvent pas être lancés si le paramètre échoue ou si le hachage diffère des écritures de registre codifiées.
- Le `ConfidentialFeatureDigest::conf_rules_version` en ligne intègre la version de la mise en page du manifeste. Pour redes v1 DEBE permanent `Some(1)` et il est identique à `iroha_config::parameters::defaults::confidential::RULES_VERSION`. Lors de l'évolution de l'ensemble de règles, sous la constante, régénérer les manifestes et afficher les binaires en lock-step ; Mezclar versions font que les validateurs rechacen bloquent avec `ConfidentialFeatureDigestMismatch`.
- L'activation manifeste DEBERIAN agrupar actualizaciones de Registry, cambios de cycle de vida de parametros y transiciones de politica para que el digeste se mantenga cohérente:
  1. Appliquez les modifications planifiées du registre (`Publish*`, `Set*Lifecycle`) dans une vue hors ligne de l'état et calculez le résumé post-activation avec `compute_confidential_feature_digest`.
  2. Emite `SetParameter::custom(confidential_registry_root, {"vk_set_hash": "0x..."})` en utilisant le hachage calculé pour que les pairs attrasados ​​puissent récupérer le résumé correct après avoir suivi les instructions des intermédiaires de registre.3. Annexe aux instructions `ScheduleConfidentialPolicyTransition`. Cada instruccion debe citar el `transition_id` émis par la gouvernance ; manifeste que l'omission sera enregistrée pour le runtime.
  4. Conservez les octets du manifeste, une empreinte digitale SHA-256 et le résumé utilisé dans le plan d'activation. Les opérateurs vérifient les trois artefacts avant de voter le manifeste pour éviter les participations.
- Lorsque les déploiements nécessitent un basculement différent, enregistrez la hauteur objet avec un paramètre personnalisé associé (par exemple `custom.confidential_upgrade_activation_height`). C'est pour les auditeurs une vérification codifiée en Norito pour que les validateurs respectent la fenêtre d'avis avant que le changement de digestion entre en effet.## Cycle de vie des vérificateurs et paramètres
### Registre ZK
- Le grand livre enregistré `ZkVerifierEntry { vk_id, circuit_id, version, proving_system, curve, public_inputs_schema_hash, vk_hash, vk_len, max_proof_bytes, gas_schedule_id, activation_height, deprecation_height, withdraw_height, status, metadata_uri_cid, vk_bytes_cid }` où `proving_system` est actuellement fixé à `Halo2`.
- Pares `(circuit_id, version)` son globalement unique ; Le registre conserve un indice secondaire pour consulter les métadonnées du circuit. Les intentions d'enregistrement d'un duplicado sont rechazanes lors de l'admission.
- `circuit_id` ne doit pas être vide et `public_inputs_schema_hash` doit être fourni (typiquement un hachage Blake2b-32 du codage canonique des entrées publiques du vérificateur). Admission rechaza registros que omiten estos campos.
- Les instructions de gouvernance incluent :
  - `PUBLISH` pour ajouter une entrée `Proposed` uniquement avec des métadonnées.
  - `ACTIVATE { vk_id, activation_height }` pour l'activation du programme à une limite d'époque.
  - `DEPRECATE { vk_id, deprecation_height }` pour marquer la hauteur finale des preuves donde peut référencer l'entrée.
  - `WITHDRAW { vk_id, withdraw_height }` pour l'assistance d'urgence ; les actifs affectés congelan gastos confidentielles despues de retrait hauteur hasta que nuevas entradas se activen.
- Genesis manifeste automatiquement un paramètre personnalisé `confidential_registry_root` lorsque `vk_set_hash` coïncide avec les entrées activées ; la validation cruza est ce résumé contre l'état local du registre avant qu'un nœud puisse unir un consensus.- Registrar ou actualiser un vérificateur requis `gas_schedule_id` ; la vérification exige que l'entrée de registre soit `Active`, présente dans l'indice `(circuit_id, version)`, et que les preuves Halo2 prouvent un `OpenVerifyEnvelope` avec `circuit_id`, `vk_hash`, et `public_inputs_schema_hash` coïncidant avec le registre du registre.

### Prouver les clés
- Les clés de preuve sont conservées hors grand livre mais sont référencées par les identifiants dirigés par le contenu (`pk_cid`, `pk_hash`, `pk_len`) publiées conjointement avec les métadonnées du vérificateur.
- Les SDK du portefeuille obtiennent des données de PK, vérifient les hachages et cachent localement.

### Paramètres Pedersen et Poséidon
- Les registres séparés (`PedersenParams`, `PoseidonParams`) reflètent les contrôles du cycle de vie du vérificateur, chacun avec `params_id`, les hachages des générateurs/constantes, l'activation, la dépréciation et les valeurs de retrait.
- Les engagements et les hachages séparent les domaines par `params_id` pour que la rotation des paramètres ne réutilise jamais les patrons de bits des ensembles obsolètes ; L'ID est intégré dans les engagements de notes et les tags du domaine de nullifier.
- Les circuits supportent la sélection multi-paramètres en temps de vérification ; les ensembles de paramètres obsolètes doivent être réglés jusqu'à votre `deprecation_height`, et les ensembles retirés sont exactement repris dans votre `withdraw_height`.## Orden déterministe et annulateurs
- Chaque actif conserve un `CommitmentTree` avec `next_leaf_index` ; les blocs s'engagent dans un ordre déterministe : itérer les transactions dans l'ordre des blocs ; à l'intérieur de chaque opération de transaction, les sorties sont aveugles par `output_idx` sérialisé ascendant.
- `note_position` est dérivé des compensations de l'arbre mais **non** fait partie du nullificateur ; seul alimenta les routes des membres à l’intérieur du témoin de l’essai.
- La stabilité de l'annulation des réorganisations sous-jacentes est garantie par le projet PRF ; l'entrée PRF enlaza `{ nk, note_preimage_hash, asset_id, chain_id, params_id }`, et les ancres racines référentes Merkle historicos limitées par `max_anchor_age_blocks`.## Flux de grand livre
1. **MintConfidential {asset_id, montant, destinataire_hint }**
   - Exiger une politique d'actif `Convertible` ou `ShieldedOnly` ; admission verifica autoridad de Asset, obtiene `params_id` actuel, muestrea `rho`, émettre un engagement, actualiser l'arbol Merkle.
   - Emite `ConfidentialEvent::Shielded` avec le nouvel engagement, la racine delta de Merkle et le hachage de l'appel de transaction pour les pistes d'audit.
2. **TransferConfidential {asset_id, proof, circuit_id, version, nullifiers, new_commitments, enc_payloads, Anchor_root, memo }**
   - Syscall del VM verifica proof en utilisant l'entrée du registre ; el host asegura annulifie no usados, engagements anexados de forma determinista, Anchor reciente.
   - Le grand livre enregistre les entrées de `NullifierSet`, les charges utiles enregistrées sont écrites pour les destinataires/auditeurs et émettent `ConfidentialEvent::Transferred` en récupérant les annulateurs, les sorties ordonnées, le hachage de preuve et les racines Merkle.
3. **RevealConfidential {asset_id, preuve, circuit_id, version, nullifier, montant, destinataire_account, Anchor_root }**
   - Disponible seul pour les actifs `Convertible` ; la preuve valide que la valeur de la note iguala le montant révélé, le grand livre accrédite la balance transparente et que la note aveugle marque l'annuleur comme gastado.
   - Émite `ConfidentialEvent::Unshielded` avec le monto publico, annuleurs de consommation, identifiants de preuve et hachage de l'appel de transaction.## Ajouts au modèle de données
- `ConfidentialConfig` (nouvelle section de configuration) avec drapeau d'habilitation, `assume_valid`, boutons de gaz/limites, fenêtre d'ancrage, backend de vérificateur.
- Schémas `ConfidentialNote`, `ConfidentialTransfer` et `ConfidentialMint` Norito avec octet de version explicite (`CONFIDENTIAL_ASSET_V1 = 0x01`).
- `ConfidentialEncryptedPayload` contient des octets de mémo AEAD avec `{ version, ephemeral_pubkey, nonce, ciphertext }`, avec `version = CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1` par défaut pour la mise en page XChaCha20-Poly1305.
- Vecteurs canoniques de dérivation de clé vivant en `docs/source/confidential_key_vectors.json` ; tant que la CLI comme le point de terminaison Torii correspond à ces appareils.
- `asset::AssetDefinition` agréga `confidential_policy: AssetConfidentialPolicy { mode, vk_set_hash, poseidon_params_id, pedersen_params_id, pending_transition }`.
- `ZkAssetState` persiste la liaison `(backend, name, commitment)` pour les vérificateurs de transfert/unshield ; l'exécution demande des preuves en vérifiant la clé référencée ou en ligne qui ne coïncide pas avec l'engagement enregistré.
- `CommitmentTree` (pour actifs avec points de contrôle frontaliers), `NullifierSet` avec clé `(chain_id, asset_id, nullifier)`, `ZkVerifierEntry`, `PedersenParams`, `PoseidonParams` almacenados dans l'état mondial.
- Mempool maintient les structures transitoires `NullifierIndex` et `AnchorIndex` pour la détection de la température des duplicados et des contrôles de l'ancre.
- Les mises à jour du schéma Norito incluent l'ordre canonique pour les entrées publiques ; les tests aller-retour assurent le déterminisme du codage.- Roundtrips de payload encriptado quedan fijados via des tests unitaires (`crates/iroha_data_model/src/confidential.rs`). Vecteurs de portefeuille de suivi des transcriptions complémentaires AEAD canonicos para auditeurs. `norito.md` documente l'en-tête sur fil pour l'enveloppe.

## Intégration avec IVM et syscall
- Introduire l'appel système `VERIFY_CONFIDENTIAL_PROOF` en acceptant :
  - `circuit_id`, `version`, `scheme`, `public_inputs`, `proof` et le `ConfidentialStateDelta { asset_id, nullifiers, commitments, enc_payloads }` résultant.
  - Le syscall charge les métadonnées du vérificateur à partir du registre, applique les limites de temps/temps, cobra gas determinista, et applique uniquement le delta si la preuve est sortie.
- L'hôte expose un trait de lecture solo `ConfidentialLedger` pour récupérer des instantanés de la racine Merkle et de l'état d'annulation ; la bibliothèque Kotodama prouve les aides d'assemblage de témoin et de validation de schéma.
- Les documents de pointeur-ABI sont mis à jour pour clarifier la disposition du tampon de preuve et les poignées de registre.## Négociation des capacités du nœud
- La poignée de main annonce `feature_bits.confidential` avec `ConfidentialFeatureDigest { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`. La participation des validateurs nécessite `confidential.enabled=true`, `assume_valid=false`, identifiants backend de vérificateur d'identité et résumés coïncidents ; des divergences surviennent lors de la poignée de main avec `HandshakeConfidentialMismatch`.
- La configuration prend en charge `assume_valid` uniquement pour les nodos observateurs : lorsque cela est déshabilité, découvrez les instructions confidentielles qui produisent `UnsupportedInstruction` déterministe sans panique ; cuando esta habilitado, observateurs appliqués deltas déclarés sans vérifier les preuves.
- Mempool rechaza transacciones confidentielles si la capacité locale est déshabilitée. Les filtres de potins évitent d'envoyer des transactions aveugles à des pairs sans que les capacités coïncident avec les identifiants ciegas d'un vérificateur découvert dans les limites de votre temps.

### Matrice de poignée de main| Annonce à distance | Résultats pour les nœuds validateurs | Notes pour les opérateurs |
|----------------------|-----------------------------|----------------|
| `enabled=true`, `assume_valid=false`, backend coincide, digest coincide | Accepté | Le pair est allé à l'état `Ready` et a participé à sa proposition, votant et diffusant RBC. Aucun manuel d'action n'est requis. |
| `enabled=true`, `assume_valid=false`, backend coincide, digest stale o faltante | Réchazado (`HandshakeConfidentialMismatch`) | El remoto debe aplicar activaciones pendientes de registry/parametros o esperar el `activation_height` programado. Hasta corregir, el nodo sigue visible pero nunca entra en rotación de consenso. |
| `enabled=true`, `assume_valid=true` | Réchazado (`HandshakeConfidentialMismatch`) | Los validadores requieren verificacion de proofs; configura el remoto como observer con ingreso solo Torii o cambia `assume_valid=false` tras habilitar verificacion completa. |
| `enabled=false`, campos omitidos (build desactualizado), o backend de verificador distinto | Réchazado (`HandshakeConfidentialMismatch`) | Les pairs désactualisés ou partiellement actualisés ne peuvent pas s’exprimer au rouge du consensus. Actualisez la version actuelle et assurez-vous que le tuple backend + digest coïncide avant la reconectar. |Les observateurs qui omettent intentionnellement la vérification des preuves ne doivent pas ouvrir des connexions de consensus contre les validateurs avec des portes de capacité. Vous pouvez également bloquer via Torii ou les API d'archivage, mais le rouge de consentement vous demande d'annoncer des capacités coïncidentes.

### Politique d'élagage des révélations et de rétention des annulateurs

Les grands livres confidentiels doivent conserver un historique suffisant pour vérifier la fraîcheur des notes et reproduire les auditoires pulsés par la gouvernance. La politique par défaut, appliquée par `ConfidentialLedger`, est :- **Retencion des nullifiers :** maintenir les nullifiers gastados por un *minimo* de `730` dias (24 meses) après la hauteur de gaz, ou la ventana régulateura obligatoria si es mayor. Les opérateurs peuvent étendre la fenêtre via `confidential.retention.nullifier_days`. Annulateurs plus récents que la ventana DEBEN seguir consultables via Torii pour que les auditeurs vérifient ausencia de double-spend.
- **Élagage des révélations :** les révélations transparentes (`RevealConfidential`) peuvent permettre les engagements associés immédiatement après que le bloc soit finalisé, mais l'annuleur consommé est soumis à la réglementation de rétention antérieure. Les événements liés à la révélation (`ConfidentialEvent::Unshielded`) enregistrent le montant public, le destinataire et le hachage de preuve pour que la reconstruction de l'historique ne nécessite pas le texte chiffré peut-être.
- **Points de contrôle frontaliers :** los frontiers de engagement maintiennent des points de contrôle en roulant que cubren el mayor de `max_anchor_age_blocks` y la ventana de retencion. Les points de contrôle compactés sont plus anciens seulement après que tous les annulateurs soient à l'intérieur de l'intervalle expiré.- **Remédiation en cas de digest obsolète :** si `HandshakeConfidentialMismatch` dérive du digest, les opérateurs doivent (1) vérifier que les fenêtres de rétention des annuleurs coïncident avec le cluster, (2) exécuter `iroha_cli app confidential verify-ledger` pour régénérer le digest contre le ensemble de nullifiers retenus, et (3) redéployer le manifeste actualisé. S'il peut être annulé, il peut être prématuré de le restaurer sans stockage avant de le réintégrer au rouge.

La documentation remplace les paramètres régionaux dans le runbook d'opérations ; les politiques de gouvernance qui étendent la fenêtre de rétention doivent actualiser la configuration des nœuds et des plans de stockage des archives en même temps.

### Flux d'expulsion et de récupération1. Pendant le cadran, `IrohaNetwork` compare les capacités annoncées. Cualquier décalage levanta `HandshakeConfidentialMismatch`; la connexion se cierra et le pair permanent dans la cola de découverte sans être promu au `Ready`.
2. L'erreur est affichée via le journal du service rouge (y compris le résumé à distance et le backend), et Sumeragi n'est pas programmé par l'homologue pour proposer le vote.
3. Les opérateurs corrigent les registres de vérificateur et les ensembles de paramètres (`vk_set_hash`, `pedersen_params_id`, `poseidon_params_id`) ou le programme `next_conf_features` avec un `activation_height` accordé. Une fois que le résumé coïncide, la poignée de main suivante se termine automatiquement.
4. Si un homologue obsolète déclenche un blocage (par exemple, via la relecture de l'archive), les validateurs l'appelleront de forme déterminée avec `BlockRejectionReason::ConfidentialFeatureDigestMismatch`, en gardant l'état du grand livre cohérent dans le rouge.

### Flux de poignée de main sécurisé avant la relecture1. Chaque fois que vous avez l'intention d'attribuer le matériau de la clé Noise/X25519 nouveau. La charge utile de la poignée de main qui est ferme (`handshake_signature_payload`) concatena les clés publiques électroniques locales et distantes, la direction du socket annoncée codifiée en Norito et, lorsqu'elle est compilée avec `handshake_chain_id`, l'identifiant de chaîne. Le message est écrit avec AEAD avant de sortir du nœud.
2. Le récepteur recalcule la charge utile avec l'ordre des clés peer/local inversé et vérifie la société Ed25519 intégrée dans `HandshakeHelloV1`. De sorte que les ambas claves efimeras et la direccion anunciada forman parte del dominio de firma, reproduisent un message capturé contre un autre pair ou récupèrent une connexion périmée suite à la vérification de forme déterministe.
3. Drapeaux de capacité confidentielle et le `ConfidentialFeatureDigest` viajan dentro de `HandshakeConfidentialMeta`. Le récepteur compare le tuple `{ enabled, assume_valid, verifier_backend, digest }` au `ConfidentialHandshakeCaps` local ; cualquier dismatch sale temprano con `HandshakeConfidentialMismatch` avant que le transport ne transite vers `Ready`.
4. Les opérateurs DEBEN recalculent le résumé (via `compute_confidential_feature_digest`) et réinitialisent les nœuds avec les registres/politiques actualisés avant la reconectar. Les pairs qui annoncent digèrent antiguos siguen fallando el handshake, évitant que l'état périmé réintègre l'ensemble des validadores.5. Les sorties et les échecs de la poignée de main actualisent les contadores au niveau `iroha_p2p::peer` (`handshake_failure_count`, aides de taxonomie des erreurs) et émettent des journaux structurés avec l'ID d'homologue à distance et l'empreinte digitale du résumé. Surveillez ces indicateurs pour détecter les replays ou les configurations incorrectes pendant le déploiement.

## Gestion des clés et des charges utiles
- Jerarquia de dérivacion por account:
  - `sk_spend` -> `nk` (clé d'annulation), `ivk` (clé de visualisation entrante), `ovk` (clé de visualisation sortante), `fvk`.
- Charges utiles de notes encriptadas utilisant AEAD avec des clés partagées dérivées de ECDH ; se pueden adjuntar view clés des auditeurs optionnels et des résultats en fonction de la politique de l'actif.
- Ajouts à la CLI : `confidential create-keys`, `confidential send`, `confidential export-view-key`, outils d'auditeur pour décrire des mémos et assistant `iroha app zk envelope` pour produire/inspecter des enveloppes Norito hors ligne.
- Calendrier de gaz déterministe :
  - Halo2 (Plonkish) : base `250_000` gaz + `2_000` gaz par entrée publique.
  - `5` gas por proof byte, mais cargos por nullifier (`300`) et por engagement (`500`).
  - Les opérateurs peuvent écrire ces constantes via la configuration du nœud (`confidential.gas.{proof_base, per_public_input, per_proof_byte, per_nullifier, per_commitment}`) ; Les changements se propagent au démarrage ou lorsque la configuration de rechargement à chaud est appliquée et appliquée de manière déterminante dans le cluster.
- Limites duros (configurables par défaut) :
-`max_proof_size_bytes = 262_144`.
-`max_nullifiers_per_tx = 8`, `max_commitments_per_tx = 8`, `max_confidential_ops_per_block = 256`.
- `verify_timeout_ms = 750`, `max_anchor_age_blocks = 10_000`. Preuves que dépasser `verify_timeout_ms` a interrompu l'instruction de forme déterministe (les bulletins de vote de gouvernance émis en `proof verification exceeded timeout`, `VerifyProof` renvoient une erreur).
- Cuotas adicionales aseguran vivacity: `max_proof_bytes_block`, `max_verify_calls_per_tx`, `max_verify_calls_per_block`, et `max_public_inputs` acotan block builders ; `reorg_depth_bound` (>= `max_anchor_age_blocks`) gère la rétention des points de contrôle frontaliers.
- L'exécution du runtime ahora rechaza transacciones qui dépassent ces limites de transaction ou de blocage, émet des erreurs `InvalidParameter` déterministes et dejando l'état du grand livre sans changement.
- Mémoire préfiltrée pour les transactions confidentielles par `vk_id`, longitude de preuve et date d'ancrage avant d'invoquer le vérificateur pour maintenir l'utilisation des ressources en compte.- La vérification est déterminée par une forme déterminée en cas de dépassement de délai ou de violation des limites ; les transactions tombent avec des erreurs explicites. Les backends SIMD sont optionnels mais ne modifient pas la comptabilité du gaz.

### Lignes de base d'étalonnage et portes d'acceptation
- **Plateformes de référence.** Les corridas de calibrage DEBEN couvrent les trois profils de matériel en bas. Corridas qui ne capturent pas tous les profils se rechazan lors de la révision.

  | Profil | Architecture | CPU / Instanciation | Drapeaux du compilateur | Proposé |
  | --- | --- | --- | --- | --- |
  | `baseline-simd-neutral` | `x86_64` | AMD EPYC 7B12 (32c) ou Intel Xeon Gold 6430 (24c) | `RUSTFLAGS="-C target-feature=-avx,-avx2,-fma"` | Establece valores piso sin intrinsèques vectorielles; utilisez-le pour ajuster les tableaux de coûts de secours. |
  | `baseline-avx2` | `x86_64` | Intel Xeon Or 6430 (24c) | libération par défaut | Valider le chemin AVX2 ; révisez que les accélérateurs SIMD se maintiennent dans la tolérance du gaz neutre. |
  | `baseline-neon` | `aarch64` | AWS Graviton3 (c7g.4xlarge) | libération par défaut | Assurez-vous que le backend NEON est permanent et déterminé avec les horaires x86. |

- **Harnais de référence.** Tous les rapports d'étalonnage de gaz DEBEN produisent avec :
  -`CRITERION_HOME=target/criterion cargo bench -p iroha_core isi_gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>`
  - `cargo test -p iroha_core bench_repro -- --ignored` pour confirmer le détermination du luminaire.
  - `CRITERION_HOME=target/criterion cargo bench -p ivm gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>` lorsque les coûts de l'opcode de la VM changent.- **Randomness fija.** Exporta `IROHA_CONF_GAS_SEED=conf-gas-seed-2026Q1` antes de correr benchs para que `iroha_test_samples::gen_account_in` change a la route déterministe `KeyPair::from_seed`. Le harnais imprime `IROHA_CONF_GAS_SEED_ACTIVE=...` una vez; si la variable échoue, la révision DEBE tombe. Toute nouvelle utilisation de calibrage doit être conforme à cet environnement en introduisant un auxiliaire aléatoire.

- **Capture des résultats.**
  - Subir les résultats de Criterion (`target/criterion/**/raw.csv`) pour chaque profil de l'artefact de sortie.
  - Garder les mesures dérivées (`ns/op`, `gas/op`, `ns/gas`) dans le [Confidential Gas Calibration ledger](./confidential-gas-calibration) avec le commit de git et la version du compilateur utilisé.
  - Maintenir les dernières lignes de base par profil ; éliminer les instantanés mais avant de valider le rapport plus nouveau.

- **Tolérances d'acceptation.**
  - Deltas de gaz entre `baseline-simd-neutral` et `baseline-avx2` DEBEN permanent <= +/-1,5%.
  - Deltas de gaz entre `baseline-simd-neutral` et `baseline-neon` DEBEN permanent <= +/-2.0%.
  - Propriétés de calibrage qui dépassent ces umbrales qui nécessitent d'ajuster le calendrier ou un RFC qui explique l'écart et son atténuation.- **Checklist de révision.** Les soumettants sont responsables de :
  - Inclut `uname -a`, les extraits de `/proc/cpuinfo` (modèle, pas à pas), et `rustc -Vv` dans le journal d'étalonnage.
  - Vérifiez que `IROHA_CONF_GAS_SEED` se trouve sur la sortie du banc (les bancs impriment la graine active).
  - Assurez-vous que les indicateurs de fonctionnalité du stimulateur cardiaque et du vérificateur confidentiel sont en particulier la production (`--features confidential,telemetry` sur les bancs de comparaison avec télémétrie).

## Configuration et opérations
- `iroha_config` ajouter la section `[confidential]` :
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
- Telemetria émet des mesures agrégées : `confidential_proof_verified`, `confidential_verifier_latency_ms`, `confidential_proof_bytes_total`, `confidential_nullifier_spent`, `confidential_commitments_appended`, `confidential_mempool_rejected_total{reason}`, et `confidential_policy_transitions_total`, nunca exposer les données en clair.
- Superficies RPC :
  -`GET /confidential/capabilities`
  -`GET /confidential/zk_registry`
  -`GET /confidential/params`## Stratégie de test
- Déterminisme : le mélange aléatoire des transactions à l'intérieur des blocs produit des racines de Merkle et des ensembles d'annuleurs identiques.
- Resiliencia a reorg : réorgs simulaires multi-blocs avec ancres ; les annulateurs se maintiennent stables et les ancres périmées se rechazan.
- Invariants de gaz : vérifier l'utilisation du gaz identique entre les nœuds avec et sans accélération SIMD.
- Tests de frontière : preuves en techniques de tamano/gas, nombre maximum d'entrées/sorties, application du délai d'attente.
- Cycle de vie : opérations de gouvernance pour l'activation/la dépréciation du vérificateur et des paramètres, tests de gestion pendant la rotation.
- Politique FSM : transiciones permitidas/no permitidas, délais de transition pendant et rechazo de mempool alrededor de alturas efectivas.
- Émergences de registre : retrait des actifs gelés d'émergence affectés en `withdraw_height` et rechaza preuves après.
- Capability gating : validadores con `conf_features` blocs rechazan incompatibles ; les observateurs avec `assume_valid=true` avanzan sin afectar consenso.
- Équivalence de l'état : nœuds validateur/complet/observateur produisant des racines de l'état identique dans la chaîne canonique.
- Fuzzing négatif : preuves mal formées, charges utiles sobredimensionados et colisiones de nullifier se rechazan deterministamente.## Migration
- Indicateur de fonctionnalité de déploiement : jusqu'à ce que la phase C3 soit terminée, `enabled` par défaut est `false` ; les nœuds annoncent les capacités avant de se connecter au validateur.
- Actifs transparents non affectés ; les instructions confidentielles nécessitent des entrées de registre et de négociation de capacités.
- Nodos compilados sin soporte confidential rechazan bloques relevantes de forma determinista ; vous ne pouvez pas utiliser l'ensemble de validateurs mais vous pouvez opérer comme observateurs avec `assume_valid=true`.
- Genesis manifeste les entrées initiales du registre, les ensembles de paramètres, les politiques confidentielles pour les actifs et les clés de l'auditeur optionnel.
- Les opérateurs suivent les runbooks publiés pour la rotation du registre, les transitions politiques et le retrait de l'émergence pour maintenir les mises à niveau déterministes.## Travail pendant
- Benchmarks de paramètres Halo2 (tamano de circuit, stratégie de recherche) et enregistrement des résultats dans le playbook de calibrage pour que les valeurs par défaut de gas/timeout soient actualisées lors du prochain rafraîchissement de `confidential_assets_calibration.md`.
- Finaliser la politique de divulgation de l'auditeur et les API des sociétés de visualisation sélective, en connectant le flux approuvé au Torii une fois que le bureau de gouvernance est ferme.
- Extension du schéma de chiffrement des témoins pour générer des sorties multi-destinataires et des mémos par lots, en documentant le format de l'enveloppe pour les implémenteurs du SDK.
- Encargar une révision de sécurité externe des circuits, registres et procédures de rotation des paramètres et archivage des hallazgos conjointement aux rapports internes des auditoires.
- Préciser les API de réconciliation des dépenses pour les auditeurs et publier le guide d'accès à la clé de vue pour que les fournisseurs de portefeuilles mettent en œuvre les mêmes règles sémantiques de certification.## Phases de mise en œuvre
1. **Phase M0 - Endurecimiento Stop-Ship**
   - [x] La dérivacion de nullifier ahora sigue el diseno Poseidon PRF (`nk`, `rho`, `asset_id`, `chain_id`) avec l'ordre déterministe des engagements forcés dans l'actualisation du grand livre.
   - [x] L'exécution des limites d'application de la preuve et des points confidentiels pour la transaction/pour le blocage, rechazando transacciones fuera de presupuesto con errores deterministas.
   - [x] La poignée de main P2P annonce `ConfidentialFeatureDigest` (résumé du backend + empreintes digitales du registre) et il y a des incompatibilités de forme déterminées via `HandshakeConfidentialMismatch`.
   - [x] Remover panique sur les chemins d'éjection confidentiels et ajoute un rôle de contrôle pour les nœuds sans support.
   - [ ] Appliquer les présupposés de délai d'attente du vérificateur et les limites de profondeur de réorganisation pour les points de contrôle frontaliers.
     - [x] Présupposés de délai d'attente de vérification appliqué ; preuves qui dépassent `verify_timeout_ms` maintenant fallan deterministamente.
     - [x] Points de contrôle frontaliers maintenant respetan `reorg_depth_bound`, je peux utiliser des points de contrôle plus anciens que la fenêtre configurée pour garder des instantanés déterministes.
   - Introduire `AssetConfidentialPolicy`, la politique FSM et les portes d'application pour les instructions d'émission/transfert/révélation.
   - Comprometer `conf_features` dans les en-têtes de blocage et la participation des validateurs lorsque les résumés de registre/paramètres divergent.2. **Phase M1 - Registres et paramètres**
   - Entregar registries `ZkVerifierEntry`, `PedersenParams`, et `PoseidonParams` avec opérations de gouvernance, anclaje de genesis et manejo de cache.
   - Connectez l'appel système pour nécessiter des recherches de registre, des identifiants de calendrier de gaz, un hachage de schéma et des vérifications de routine.
   - Envoyer le format de charge utile crypté v1, les vecteurs de dérivation de clés pour le portefeuille et le support CLI pour la gestion des clés confidentielles.
3. **Phase M2 - Performances gaz et**
   - Implémentation du calendrier de détermination du gaz, des contrôleurs de blocage et des harnais de référence avec télémétrie (latence de vérification, tamanos de preuve, rechazos de mempool).
   - Points de contrôle Endurecer CommitmentTree, charge LRU et indices de nullificateur pour les charges de travail multi-actifs.
4. **Phase M3 - Rotation et outillage du portefeuille**
   - Habiliter l'acceptation de preuves multi-paramètres et multi-versions ; prendre en charge l'activation/la dépréciation impulsive pour la gouvernance avec les runbooks de transition.
   - Entrer les flux de migration dans SDK/CLI, les workflows d'analyse de l'auditeur et les outils de réconciliation des dépenses.
5. **Phase M4 - Audit et opérations**
   - Prouver les workflows des clés d'auditeur, les API de divulgation sélective et les runbooks opérationnels.
   - Révision externe du programme de cryptographie/sécurité et publicité en `status.md`.Chaque phase met à jour les jalons de la feuille de route et les tests associés pour maintenir les garanties d'exécution déterminées dans la blockchain rouge.
---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/confidential-assets.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : Activités confidentielles et activités ZK
la description: Блюпринт Phase C для blindé циркуляции, реестров и операторских контролей.
slug : /nexus/actifs-confidentiels
---
<!--
SPDX-License-Identifier: Apache-2.0
-->
# Conception d'actifs confidentiels et de paramètres ZK

## Motivation
- Ces activités protégées par opt-in, qui peuvent être effectuées dans des domaines privés, ne sont pas autorisées à effectuer une transaction.
- Vérifiez l'utilisation des paramètres de validation et utilisez Norito/Kotodama ABI v1.
- Préparer l'auditeur et le contrôle de l'opérateur pour les cycles (activation, rotation, ouverture) des circuits et des paramètres de cryptographie.

## Modèle угроз
- Валидаторы честные-но-любопытные (honnête-mais-curieux) : utiliser le consensus correctement, но пытаются изучать grand livre/état.
- Наблюдатели сети видят данные блоков и commérages транзакции; Les canaux de potins privés ne sont pas diffusés.
- Il y a des tâches : analyse du trafic hors grand livre, des transactions commerciales (également dans la feuille de route PQ), ainsi que du grand livre.## Обзор дизайна
- Активы могут объявлять *piscine protégée* помимо существующих прозрачных балансов ; engagements protégés de циркуляция представляется криптографическими.
- Notes contenant `(asset_id, amount, recipient_view_key, blinding, rho)` :
  - Engagement : `Comm = Pedersen(params_id || asset_id || amount || recipient_view_key || blinding)`.
  - Annulateur : `Null = Poseidon(domain_sep || nk || rho || asset_id || chain_id)`, nezaвисим от порядка notes.
  - Charge utile cryptée : `enc_payload = AEAD_XChaCha20Poly1305(ephemeral_shared_key, note_plaintext)`.
- Les transferts ne nécessitent pas de charges utiles Norito-codirovannye `ConfidentialTransfer`, correspondant :
  - Entrées publiques : ancre Merkle, annulateurs, nouveaux engagements, identifiant d'actif, circuit de version.
  - Charges utiles optimisées pour les auditeurs professionnels et opérationnels.
  - Preuve de connaissance nulle, подтверждающую сохранение стоимости, propriété et авторизацию.
- Vérification des clés et des paramètres de contrôle du registre sur grand livre avec les activités ; Vous pouvez valider des épreuves pour des raisons d'intérêt ou d'achats automatiques.
- Les comités de consensus avec l'activité Digest sont confidentiels, les blocs sont prévus pour le registre de l'organisation. et paramètres.
- Les preuves de publication utilisent Halo2 (Plonkish) sans configuration fiable ; Groth16 et les autres variantes SNARK ne sont pas disponibles dans la v1.

### Fixations de déterminationLes convertisseurs de mémo confidentiels sont publiés dans le fichier canonique `fixtures/confidential/encrypted_payload_v1.json`. Si vous avez maintenant un convertisseur v1 complet et des applications négatives, le SDK peut vérifier les paramètres de sauvegarde. Les tests de modèle de données Rust (`crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs`) et la suite Swift (`IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift`) fournissent une configuration et une garantie de configuration Norito L'appareil et la régression de l'état de fonctionnement sont liés à de simples codes d'évolution.

Les SDK Swift vous permettent de créer des installations de bouclier sans colle JSON sur mesure : utilisez `ShieldRequest` avec des notes d'engagement de 32 billets, charge utile et métadonnées de débit. Sélectionnez `IrohaSDK.submit(shield:keypair:)` (ou `submitAndWait`) pour pouvoir effectuer et désactiver la transmission à partir de `/v1/pipeline/transactions`. L'aide à la validation de vos engagements, en fournissant `ConfidentialEncryptedPayload` à partir de l'encodeur Norito et de la disposition de référence `zk::Shield`, décrit ce que vous avez prévu. оставались синхронизированы с Rust.## Commentaires sur le consensus et le portail
- Les blocs de construction раскрывают `conf_features = { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }` ; digest участвует в хэше консенсуса и должен совпадать с локальным представлением registre для принятия bloka.
- La gouvernance peut être complétée par le programme `next_conf_features` avec le module `activation_height` ; Pour cela, les producteurs bloquent leur travail en vue de la publication du résumé préliminaire.
- Les validateurs utilisent le logiciel `confidential.enabled = true` et `assume_valid = false`. Les fournisseurs de démarrage ne doivent pas être utilisés dans les validateurs, si l'utilisateur est proche ou local `conf_features`.
- La prise de contact P2P se termine par `{ enabled, assume_valid, conf_features }`. Peers, récupérez les informations dont vous avez besoin, en vous connectant à `HandshakeConfidentialMismatch` et en niikogda dans le consensus rotatif.
- Les résultats sont sélectionnés par les validateurs, les observateurs et les pairs utilisateurs lors de la poignée de main dans la phase [Négociation de capacité de nœud] (#node-capability-negotiation). La poignée de main de la société est basée sur `HandshakeConfidentialMismatch` et s'adresse aux pairs lors d'un accord de rotation, mais le résumé n'est pas compatible.
- Les observateurs, qui ne sont pas des validateurs, peuvent consulter `assume_valid = true` ; Si vous avez des relations confidentielles, vous ne pourrez pas obtenir un accord.## Activités politiques
- Pour activer l'action, il faut `AssetConfidentialPolicy`, en utilisant le système ou la gouvernance :
  - `TransparentOnly` : режим по умолчанию ; Il s'agit uniquement d'instructions de fonctionnement (`MintAsset`, `TransferAsset` et t.d.), d'une opération de fermeture blindée.
  - `ShieldedOnly` : toutes les émissions et toutes les précédentes doivent utiliser des instructions confidentielles ; `RevealConfidential` запрещен, поэтому балансы никогда не становятся публичными.
  - `Convertible` : les utilisateurs peuvent utiliser les instructions de mise en marche et de sortie blindées pour la rampe d'accès/sortie.
- Les politiques qui régissent le FSM ne bloquent pas la crédibilité :
  - `TransparentOnly → Convertible` (piscine blindée non protégée).
  - `TransparentOnly → ShieldedOnly` (réglementation préalable et conversion ultérieure).
  - `Convertible → ShieldedOnly` (mini-chargeur).
  - `ShieldedOnly → Convertible` (il n'y a aucun plan de migration pour que les notes protégées soient supprimées).
  - `ShieldedOnly → TransparentOnly` a été spécifié, si le pool protégé n'est pas disponible ou si la gouvernance ne corrige pas la migration, ce qui nécessite des notes d'installation.
- Les instructions de gouvernance sont définies par `pending_transition { new_mode, effective_height, previous_mode, transition_id, conversion_window }` par ISI `ScheduleConfidentialPolicyTransition` et peuvent être annulées par `CancelConfidentialPolicyTransition`. La garantie de validation mempool garantit que votre transfert ne doit pas être effectué avant, et qu'il n'y a aucun moyen de le déterminer, s'il est fourni. La politique s'est développée autour du bloc.- Planification automatique des opérations en cas d'ouverture du nouveau bloc : vous devez alors effectuer une conversion appropriée (pour les travaux effectués `ShieldedOnly`) ou télécharger `effective_height`, le runtime met à jour `AssetConfidentialPolicy`, ouvre les métadonnées `zk.policy` et ouvre l'entrée en attente. Si lors de l'installation du modèle `ShieldedOnly`, la pré-préparation est programmée, le runtime modifie la pré-configuration et l'enregistrement avant la pré-configuration. прежний режим.
- Les boutons de configuration `policy_transition_delay_blocks` et `policy_transition_window_blocks` permettent une mini-déclaration et des délais de grâce pour permettre la conversion complète des notes. переключения.
- `pending_transition.transition_id` также служит audit handle ; la gouvernance s'occupe de lui avant de finaliser ou de terminer les opérations, les opérateurs peuvent corréler les sorties sur la rampe d'accès/de sortie.
- `policy_transition_window_blocks` pour le 720 (environ 12 heures sur un bloc de 60 secondes). Si vous planifiez la gouvernance, vous devrez faire face à une plus grande gouvernance.
- Genesis manifeste et CLI потоки показывают текущие и ожидающие политики. La logique d'admission est une politique politique lors de l'utilisation, qui concerne les instructions confidentielles.
- Liste de contrôle миграции — см. Le « séquençage de la migration » est destiné au plan de développement suivant, qui suit le Milestone M0.

#### Surveillance des temps passés par ToriiLes écouteurs et les auditeurs utilisent `GET /v1/confidential/assets/{definition_id}/transitions` pour vérifier l'action `AssetConfidentialPolicy`. La charge utile JSON affiche l'ID d'actif canonique, après avoir activé le bloc `current_mode`, politiques, changements et effets sur C'est ce que vous avez fait (la conversion est actuellement passée à `Convertible`) et vous avez sélectionné les paramètres d'identification `vk_set_hash`/Poseidon/Pedersen. Lors de l'ouverture de la gouvernance, voici ce qui suit :

- `transition_id` — poignée d'audit, возвращенный `ScheduleConfidentialPolicyTransition`.
-`previous_mode`/`new_mode`.
-`effective_height`.
- `conversion_window` et `window_open_height` (bloc, les câbles doivent être convertis pour le cut-over ShieldedOnly).

Voici un exemple :

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

Il semble que `404` indique que l'action appropriée n'est pas disponible. Avant, il n'y avait pas de plan, le pôle `pending_transition` étant `null`.

### La grande politique politique| Recueil de documents | Dernier régime | Préparatifs | Taille effective_hauteur | Première |
|----------------------------------------------------|-------------------------------------------------------------------------------------------------|----------------------------------------------------------------------------------------------------------------------------------|-----------------------------------------------------------------------------------------------|
| Transparent uniquement | Cabriolet | Gouvernance активировал записи vérificateur/paramètre de registre. Transférez `ScheduleConfidentialPolicyTransition` à `effective_height ≥ current_height + policy_transition_delay_blocks`. | Il s'agit auparavant du `effective_height` ; piscine protégée становится доступен сразу.                   | Путь по умолчанию для включения конфиденциальности при сохранении прозрачных потоков.               |
| Transparent uniquement | Blindé uniquement | À ce propos, vous avez également `policy_transition_window_blocks ≥ 1`.                                                         | Le temps d'exécution est automatiquement défini dans `Convertible` pour `effective_height - policy_transition_window_blocks` ; Connectez-vous au `ShieldedOnly` pour le `effective_height`. | Il est nécessaire de déterminer la conversion en fonction des instructions d'ouverture.   || Cabriolet | Blindé uniquement | Planification avant `effective_height ≥ current_height + policy_transition_delay_blocks`. La gouvernance DEVRAIT подтвердить (`transparent_supply == 0`) concernant les métadonnées d'audit ; le runtime le prouve lors du cut-over. | C'est ce que vous dites. Si vous ne parvenez pas à vous connecter au `effective_height`, veuillez vous adresser au `PolicyTransitionPrerequisiteFailed`. | Insérez les actifs dans les circuits confidentiels les plus confidentiels.                                     |
| Blindé uniquement | Cabriolet | Запланированный переход; нет активного retrait d'urgence (`withdraw_height` не задан).                                    | Состояние переключается на `effective_height` ; les rampes de révélation sont ouvertes, les notes protégées étant valides.                           | Utilisé pour l'observation ou la vérification des auditeurs.                                          |
| Blindé uniquement | Transparent uniquement | La gouvernance doit fournir le plan `shielded_supply == 0` ou le plan `EmergencyUnshield` (aux auditeurs). | Le temps d'exécution s'ouvre correctement `Convertible` avant `effective_height` ; Pour cela, vous devez consulter les instructions confidentielles de la procédure de vérification et les activer dans le cadre d'une utilisation en cours. | Выход последней инстанции. Avant l'ouverture automatique, il y a une note confidentielle qui se trouve à l'extérieur. || N'importe quel | Identique à l'actuel | `CancelConfidentialPolicyTransition` permet d'obtenir des informations supplémentaires.                                                        | `pending_transition` est nécessaire.                                                                          | Сохраняет STATUS-кво; показано для полноты.                                             |

Bien sûr, vous ne pouvez pas vous engager dans la gouvernance. Runtime проверяет предпосылки прямо перед применением запланированного перехода ; Il est impossible d'activer l'action lors du régime précédent et de désactiver `PolicyTransitionPrerequisiteFailed` dans le bloc télémétrique et actuel.

### Migration ultérieure1. **Activer les paramètres :** Activez votre vérificateur et vos paramètres en fonction de la politique actuelle. Si vous obtenez le numéro `conf_features`, les pairs peuvent vérifier leur compatibilité.
2. **Installer avant :** Établir `ScheduleConfidentialPolicyTransition` avec `effective_height`, en utilisant `policy_transition_delay_blocks`. Lors de la conversion avec `ShieldedOnly`, effectuez une conversion appropriée (`window ≥ policy_transition_window_blocks`).
3. **Écrivez les instructions pour les opérateurs :** Indiquez la rampe d'accès/sortie du runbook `transition_id`. Les écouteurs et les écouteurs sont adaptés au `/v1/confidential/assets/{id}/transitions` pour ouvrir la porte.
4. **Apparition :** Lorsque vous ouvrez bien, le runtime active la politique dans `Convertible`, exécutez `PolicyTransitionWindowOpened { transition_id }` et activez l'ouverture. конфликтующие gouvernance-запросы.
5. **Définition ou suppression :** Dans le runtime `effective_height`, les prévisions sont disponibles (aucune prévision de processus, les retraits d'urgence et les retraits d'urgence ne sont disponibles). т.п.). Il faut absolument que la politique soit dans le cadre du régime actuel ; L'émissaire `PolicyTransitionPrerequisiteFailed` a annoncé la transition en attente et a établi une politique sans changement.
6. **Modification des schémas :** Après la période de gouvernance que vous avez expérimentée, la version active du schéma (par exemple, `asset_definition.v2`), un outil CLI est disponible. `confidential_policy` pour les manifestes de sérialisation. Les documents relatifs à Genesis instruisent les opérateurs de créer des registres politiques et d'ouverture avant de valider les validateurs.Il y a peu de temps, la découverte de la sécurité intime est telle que la politique n'est pas à l'ordre du jour de la genèse. Une fois que vous avez effectué une liste de contrôle pour les réglages après la mise en service, les conversions doivent être effectuées pour les détergents et les composants. успевали адаптироваться.

### Version et activation des manifestes Norito- Genesis manifeste ДОЛЖНЫ включать `SetParameter` для кастомного ключа `confidential_registry_root`. Payload — Norito JSON, correspondant à `ConfidentialRegistryMeta { vk_set_hash: Option<String> }` : ouvrez le champ (`null`), si les entrées du vérificateur sont actives, mais avant 32-байтную hex-stroku (`0x…`), равную хэшу, вычисленному `compute_vk_set_hash` selon les instructions du vérificateur dans le manifeste. Si vous ouvrez le démarrage, si le paramètre est ouvert ou s'il n'est pas compatible avec le registre, vous devez le faire.
- Le manifeste de mise en page de la version `ConfidentialFeatureDigest::conf_rules_version` sur fil est disponible. Pour le site v1, le serveur `Some(1)` et le navigateur `iroha_config::parameters::defaults::confidential::RULES_VERSION` sont installés. Chaque fois que l'ensemble de règles évolue, vous pouvez utiliser la constante, afficher les manifestes et configurer les binaires simultanément ; Veuillez indiquer la version des validateurs en ouvrant les blocs avec `ConfidentialFeatureDigestMismatch`.
- Les manifestes d'activation s'occupent de la mise à jour du registre, de la définition des paramètres et des premières politiques, qui digèrent la cohérence :
  1. Planifiez le registre d'installation (`Publish*`, `Set*Lifecycle`) dans le système d'enregistrement Internet et consultez le résumé après l'activité ici. `compute_confidential_feature_digest`.
  2. Écrivez `SetParameter::custom(confidential_registry_root, {"vk_set_hash": "0x…"})`, en utilisant votre propre résumé, pour que vos pairs puissent vous aider à obtenir le résumé correct, dès que vous y êtes. пропустили промежуточные instructions de registre.3. Téléchargez les instructions `ScheduleConfidentialPolicyTransition`. Каждая инструкция должна цитировать выданный gouvernance `transition_id` ; manifestes, ils sont en cours d'exécution, mais ils ouvrent le temps d'exécution.
  4. Enregistrez le manifeste du SHA-256 et le résumé en l'utilisant dans le plan d'activation. Les opérateurs vérifient chacun des trois articles avant l'achat, ce qui permet de régler le problème.
- Lors du déploiement, vous devez ouvrir le cut-over, en sélectionnant votre paramètre dans le paramètre de serveur client (par exemple, `custom.confidential_upgrade_activation_height`). Il s'agit de la documentation fournie par l'auditeur Norito, qui est également consacrée à l'étude du digest.## Жизненный цикл vérificateur et paramètres
### Registre ZK
- Ledger porte `ZkVerifierEntry { vk_id, circuit_id, version, proving_system, curve, public_inputs_schema_hash, vk_hash, vk_len, max_proof_bytes, gas_schedule_id, activation_height, deprecation_height, withdraw_height, status, metadata_uri_cid, vk_bytes_cid }`, et `proving_system` est fixé à `Halo2`.
- Пары `(circuit_id, version)` глобально уникальны; Le registre fournit des index automatiques pour la sécurité du circuit métadonnée. Les personnes souhaitant s'inscrire sont doublement ouvertes avant l'admission.
- `circuit_id` ne peut pas être utilisé par le vérificateur `public_inputs_schema_hash` (selon Blake2b-32, le canon publié par le vérificateur). L'admission s'effectue sans aucun problème.
- Les instructions de gouvernance comprennent :
  - `PUBLISH` pour la création de l'entrée `Proposed` uniquement avec les métadonnées.
  - `ACTIVATE { vk_id, activation_height }` pour la planification de l'activation de l'entrée à l'époque de la licence.
  - `DEPRECATE { vk_id, deprecation_height }` pour les résultats après votre visite, les preuves peuvent être demandées à l'entrée.
  - `WITHDRAW { vk_id, withdraw_height }` pour l'ouverture externe ; затронутые активы замораживают конфиденциальные траты после retirer la hauteur, пока не активируются новые entrées.
- Genesis manifeste automatiquement les paramètres `confidential_registry_root` avec `vk_set_hash`, correspondant aux entrées actives ; La validation transversale de ce résumé s'effectue dans le registre local de l'entreprise par consensus.- L'enregistrement ou la mise à jour du vérificateur demande `gas_schedule_id` ; Vous avez vérifié que vous avez enregistré le registre `Active`, que vous avez acheté l'index `(circuit_id, version)` et que les preuves Halo2 ont été enregistrées `OpenVerifyEnvelope`. Les `circuit_id`, `vk_hash` et `public_inputs_schema_hash` sont ajoutés au registre.

### Prouver les clés
- Les clés de preuve sont disponibles hors grand livre, mais ne permettent pas d'identifier les identifiants adressés par le contenu (`pk_cid`, `pk_hash`, `pk_len`), en utilisant les métadonnées. vérificateur.
- Les SDK Wallet intègrent les données PK, les prouvent et les stockent localement.

### Paramètres de Pedersen et Poséidon
- Le registre externe (`PedersenParams`, `PoseidonParams`) contrôle le contrôle du vérificateur, conformément à `params_id`, pour vous. générateurs/constants, activations, dépréciation et retrait.
- Engagements et notre décision interne `params_id`, lorsque les paramètres de rotation du pays ne sont pas les seuls modèles de bits des utilisateurs наборов; L'ID est inclus dans les notes d'engagement et dans la zone Annulateur.## Détermination des problèmes et des annulateurs
- Cette activité peut être effectuée par `CommitmentTree` avec `next_leaf_index` ; les blocs prennent des engagements dans le cadre de la détermination du bloc : la transition vers le bloc ; внутри каждой транзакции — sorties blindées pour la connexion série `output_idx`.
- `note_position` vous permet d'accéder aux offres disponibles, mais **non** dans l'annulateur ; он используется только для путей adhésion à témoin de preuve.
- Стабильность nullifier при reorg обеспечивается дизайном PRF ; Le PRF utilise `{ nk, note_preimage_hash, asset_id, chain_id, params_id }`, et les ancres se basent sur l'histoire des racines de Merkle, selon `max_anchor_age_blocks`.## Grand livre des pots
1. **MintConfidential {asset_id, montant, destinataire_hint }**
   - L'activité politique `Convertible` ou `ShieldedOnly` ; l'admission проверяет autorité актива, извлекает текущий `params_id`, сэмпLIрует `rho`, эмитирует engagement, обновляет Merkle tree.
   - Écrivez `ConfidentialEvent::Shielded` avec le nouvel engagement, Merkle root delta et hash pour les transitions pour la piste d'audit.
2. **TransferConfidential {asset_id, proof, circuit_id, version, nullifiers, new_commitments, enc_payloads, Anchor_root, memo }**
   - Syscall VM проверяет preuve по записи registre; hôte убеждается, что annuleurs не использованы, engagements добавлены детерминированно, а Anchor свежий.
   - Le grand livre enregistre les entrées `NullifierSet`, les charges utiles chargées pour les professeurs/auditeurs et les émetteurs `ConfidentialEvent::Transferred`, les annulateurs récapitulatifs, Sorties améliorées, hachage de preuve et racines Merkle.
3. **RevealConfidential {asset_id, preuve, circuit_id, version, nullifier, montant, destinataire_account, Anchor_root }**
   - Доступно только для активов `Convertible` ; preuve s'il vous plaît, que la note равно раскрытой сумме, le grand livre начисляет прозрачный баланс и сжигает blindé note, помечая annulateur как потраченный.
   - Émis par `ConfidentialEvent::Unshielded` avec un résumé public, en utilisant des annuleurs, des preuves d'identification et des hachages pour les transactions.## Ajout du modèle de données
- `ConfidentialConfig` (nouvelle configuration de configuration) avec bouton de verrouillage, `assume_valid`, boutons de gaz/limites, bouton d'ancrage et vérificateur backend.
- `ConfidentialNote`, `ConfidentialTransfer` et `ConfidentialMint` avec Norito avec votre version de batterie (`CONFIDENTIAL_ASSET_V1 = 0x01`).
- `ConfidentialEncryptedPayload` remplace les octets de mémo AEAD dans `{ version, ephemeral_pubkey, nonce, ciphertext }`, en remplaçant `version = CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1` pour la connexion XChaCha20-Poly1305.
- La dérivation de clé vectorielle canonique apparaît dans `docs/source/confidential_key_vectors.json` ; La CLI et le point de terminaison Torii enregistrent cette configuration.
- `asset::AssetDefinition` correspond à `confidential_policy: AssetConfidentialPolicy { mode, vk_set_hash, poseidon_params_id, pedersen_params_id, pending_transition }`.
- `ZkAssetState` сохраняет привязку `(backend, name, commitment)` pour les vérificateurs de transfert/non blindé ; L'utilisation de preuves d'ouverture, les clés référencées ou la clé de vérification en ligne ne correspondent pas à l'engagement d'enregistrement.
- `CommitmentTree` (pour les postes de contrôle frontaliers), `NullifierSet` avec les clés `(chain_id, asset_id, nullifier)`, `ZkVerifierEntry`, `PedersenParams`, `PoseidonParams`. в état mondial.
- Mempool prend en charge les structures de construction `NullifierIndex` et `AnchorIndex` pour la création de documents et la vérification de l'ancre.
- Обновления схемы Norito включают канонический порядок entrées publiques ; les tests aller-retour garantissent la détermination du code.- Tests unitaires de charges utiles aller-retour (`crates/iroha_data_model/src/confidential.rs`). Les fichiers vectoriels doivent permettre de créer des transcriptions canoniques de l'AEAD pour les auditeurs. `norito.md` documente l'en-tête filaire pour l'enveloppe.

## Intégration avec IVM et appel système
- L'appel système `VERIFY_CONFIDENTIAL_PROOF` est basé sur :
  - `circuit_id`, `version`, `scheme`, `public_inputs`, `proof` et le résultat `ConfidentialStateDelta { asset_id, nullifiers, commitments, enc_payloads }`.
  - Syscall prend en charge le vérificateur de métadonnées du registre, définit les limites de valeurs/régularités, indique le gaz déterminant et indique le delta jusqu'à l'utilisation de la preuve.
- L'hôte propose le trait en lecture seule `ConfidentialLedger` pour la suppression des images de la racine Merkle et de l'annuleur de statut ; Bibliothèque Kotodama pour les aides aux témoins et aux procédures de validation.
- Le pointeur de documentation-ABI est activé, qui permet de créer un tampon de preuve de mise en page et des descripteurs de registre.## Согласование возможностей узлов
- Handshake объявляет `feature_bits.confidential` вместе с `ConfidentialFeatureDigest { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`. Le validateur travaille `confidential.enabled=true`, `assume_valid=false`, identifiant le vérificateur backend et le résumé complémentaire ; несоответствия завершают handshake с `HandshakeConfidentialMismatch`.
- La configuration prend en charge `assume_valid` uniquement pour les utilisateurs d'observateurs : il s'agit uniquement des instructions confidentielles données. детерминированный `UnsupportedInstruction` без panic; когда включено, observers применяют объявленные state deltas без проверки proofs.
- Mempool отклоняет конфиденциальные транзакции, если локальная capability отключена. Les filtres Gossip ouvrent la voie aux pairs de transmission protégés et ne peuvent pas supprimer les identifiants de vérificateurs non identifiés dans les limites précédentes.

### Матрица совместимости handshake| Объявление удаленной стороны | Objets pour les validateurs | Opérateur principal |
|-------------------|------------------------------|----------------------|
| `enabled=true`, `assume_valid=false`, solution backend, compatibilité résumé | Principe | Les pairs hébergent la solution `Ready` et participent à la proposition, au vote et à la distribution RBC. Les petits projets ne sont pas difficiles. |
| `enabled=true`, `assume_valid=false`, solution backend, résumé utilisé ou résolu | Clone (`HandshakeConfidentialMismatch`) | Vous pouvez activer à distance les activités de registre/paramètres ou planifier l'enregistrement `activation_height`. Si ce n'est pas prévu, vous devez utiliser un contrat de rotation. |
| `enabled=true`, `assume_valid=true` | Clone (`HandshakeConfidentialMismatch`) | Валидаторы требуют проверку preuves; Connectez l'observateur à distance à l'entrée uniquement à partir de Torii ou fermez `assume_valid=false` après avoir effectué les vérifications générales. |
| `enabled=false`, la poignée de main est disponible (image affichée) ou le vérificateur backend est disponible | Clone (`HandshakeConfidentialMismatch`) | Les pairs ou les pairs les plus honnêtes ne peuvent pas entrer dans un consensus. Assurez-vous de prendre connaissance et de comprendre que le backend + digest est intégré à l'étape précédente. |L'observateur utilise, comme on l'appelle, les preuves de preuve, et ne doit pas ouvrir les portes du consensus sur les validateurs, les robots des capacités. S'il est possible de créer des blocs avec Torii ou une API archivée, vous ne pourrez pas définir un accord de non-conformité, mais vous ne pourrez pas prendre en compte le système. возможности.

### La révélation politique et l'annulation de la rétention

Les grands livres confidentiels doivent conserver l'histoire officielle pour documenter les notes et les auditeurs de gouvernance. La politique de l'État, présentée par `ConfidentialLedger`, est la suivante :- ** Annulateur : ** Supprimer les annuleurs minime `730` aujourd'hui (24 mois) après votre départ ou votre arrivée, si vous avez un problème регулятором. Les opérateurs peuvent le faire à partir du `confidential.retention.nullifier_days`. Les annulateurs, comme je l'ai dit, doivent fournir des services à partir de Torii, ce qui permet aux auditeurs de payer une double dépense.
- **Déclaration :** la révélation (`RevealConfidential`) permet de confirmer les engagements après la finalisation du bloc, sans utiliser d'annuleur остается под правилом rétention выше. Le logiciel Reveal (`ConfidentialEvent::Unshielded`) a été publié pour la première fois, avec une preuve de hachage et une preuve de hachage, qui reconstituent l'histoire révèlent qu'ils ne sont pas encore disponibles. texte chiffré.
- **Points de contrôle frontaliers :** les engagements aux frontières prennent en charge les points de contrôle roulants, ainsi que la rétention. Vous pouvez compacter tous les points de contrôle établis après l'installation de tous les annuleurs dans l'intervalle.
- **Remédiation de l'utilisateur de digest :** si `HandshakeConfidentialMismatch` est en train de lire le résumé, l'opérateur (1) vérifie que l'annuleur de rétention est disponible. Ensuite, (2) mettre `iroha_cli app confidential verify-ledger` pour la fin du résumé sur le nullificateur, et (3) mettre en œuvre le manifeste actuel. Les annulateurs de lubrification, s'ils ont une grande portée, devraient être utilisés dans le bon sens avant de s'installer.Documenter les performances locales dans le runbook des opérations ; gouvernance-politique, расширяющие окно rétention, должны синхронно обновлять конфигурацию узлов и планы архивного хранения.

### Procédures d'évacuation et de transport

1. Lorsque vous composez le numéro `IrohaNetwork`, vous découvrirez les capacités disponibles. L'option `HandshakeConfidentialMismatch` est disponible ; Lors de la connexion, un homologue est placé dans la file d'attente de découverte avant le `Ready`.
2. Le bureau de configuration du service de connexion (avec résumé et backend) et Sumeragi ne planifie pas les pairs pour la proposition ou le vote.
3. Les opérateurs résolvent le problème, vérifient les registres et les paramètres (`vk_set_hash`, `pedersen_params_id`, `poseidon_params_id`) ou les résolvent. `next_conf_features` avec `activation_height`. Pour que le résumé soit parfait, la poignée de main est automatique.
4. Si vous utilisez un pair, vous avez décidé de bloquer le bloc (par exemple, pour la relecture archivistique), les validateurs se sont déconnectés de vous avec `BlockRejectionReason::ConfidentialFeatureDigestMismatch`, сохраняя консистентность состояния ledger по сети.

### Flux de négociation sécurisé pour la relecture1. Si vous avez un problème avec le matériel Noise/X25519. La charge utile de prise de contact (`handshake_signature_payload`) regroupe les clés publiques éphémères locales et externes, l'adresse de socket annoncée Norito et - en cas de combinaison avec `handshake_chain_id` — chaîne d'identification. La connexion à l'AEAD s'est effectuée avant l'ouverture.
2. Déployez la charge utile en utilisant le groupe peer/local et testez Ed25519, publié dans `HandshakeHelloV1`. Vous pouvez obtenir des clés éphémères et des adresses annoncées dans votre domaine, replay pour obtenir des informations sur les pairs ou sur l'utilisation de la connexion. детерминированно проваливает валидацию.
3. Indicateurs de capacité confidentielle et `ConfidentialFeatureDigest` avant `HandshakeConfidentialMeta`. Vous pouvez utiliser le tuple `{ enabled, assume_valid, verifier_backend, digest }` pour votre configuration locale `ConfidentialHandshakeCaps` ; Il est impossible de prendre une poignée de main avec `HandshakeConfidentialMismatch` avant le transport dans `Ready`.
4. Les opérateurs DOS exécutent Digest (`compute_confidential_feature_digest`) et utilisent les registres/politiques les plus récents avant l'entrée en vigueur. подключением. Les pairs, рекламирующие старые digests, продолжают проваливать poignée de main, предотвращая возвращение устаревшего состояния в набор валидаторов.5. Les tentatives et les tentatives de prise de contact mettent en œuvre les schémas standards `iroha_p2p::peer` (`handshake_failure_count`, aides à la taxonomie d'erreur) et émettent des logs structurels. Il s'agit notamment d'une identification par un homologue distant et d'un résumé d'empreintes digitales. Cliquez sur ces indicateurs pour permettre la relecture pop ou les configurations inédites lors du déploiement.

## Mise à jour des clés et des charges utiles
- Clé de dérivation de la hiérarchie sur le compte :
  - `sk_spend` → `nk` (clé d'annulation), `ivk` (clé de visualisation entrante), `ovk` (clé de visualisation sortante), `fvk`.
- Les notes de charges utiles supplémentaires utilisent des clés partagées dérivées d'AEAD et d'ECDH ; Les touches de vue d'auditeur optionnelles peuvent être utilisées pour les sorties en fonction des activités politiques.
- Ajout de la CLI : `confidential create-keys`, `confidential send`, `confidential export-view-key`, outils de vérification pour le mémo et l'aide `iroha app zk envelope`. создания/инспекции Norito enveloppes mémo en ligne. Torii permet la dérivation de flux à partir de `POST /v1/confidential/derive-keyset`, qui prend en charge les formulaires hexadécimaux et base64, que tous les programmes peuvent utiliser иерархии ключей.## Gaz, limites et contrôles DoS
- Calendrier des gaz de détermination :
  - Halo2 (Plonkish) : gaz `250_000` + gaz `2_000` pour l'entrée publique.
  - `5` gaz à l'épreuve du temps, plus par annulateur (`300`) et par engagement (`500`).
  - Les opérateurs peuvent configurer ces constantes à partir de la configuration de votre ordinateur (`confidential.gas.{proof_base, per_public_input, per_proof_byte, per_nullifier, per_commitment}`) ; la configuration à effectuer au démarrage ou la configuration du rechargement à chaud et la configuration du clavier.
- Limites limites (définies pour la mise en service) :
-`max_proof_size_bytes = 262_144`.
- `max_nullifiers_per_tx = 8`, `max_commitments_per_tx = 8`, `max_confidential_ops_per_block = 256`.
- `verify_timeout_ms = 750`, `max_anchor_age_blocks = 10_000`. Preuves, превышающие `verify_timeout_ms`, детерминированно прерывают инструкцию (gouvernance-голосования эмитят `proof verification exceeded timeout`, `VerifyProof` возвращает ошибку).
- Les mots-clés supplémentaires utilisés sont : `max_proof_bytes_block`, `max_verify_calls_per_tx`, `max_verify_calls_per_block` et `max_public_inputs` pour les constructeurs de blocs ; `reorg_depth_bound` (≥ `max_anchor_age_blocks`) met en place des points de contrôle aux frontières de rétention.
- Le temps d'exécution permet d'ouvrir les transactions, de définir ces limites par transaction ou par bloc, et de déterminer les limites `InvalidParameter` et non. изменяя состояние grand livre.
- Mempool pré-filtre les transactions confidentielles sur `vk_id`, la preuve en ligne et l'ancre de votre vérificateur, qui organisent la vérification ressources.- Vérifiez que le délai d'attente est défini ou que les limites sont définies ; Les transitions sont effectuées avec vos clients. Les backends SIMD sont fonctionnels et ne prennent pas en charge la comptabilité.

### Calibres de base et critères de sélection
- **Plateformes de référence.** Калибровочные прогоны ДОЛЖНЫ покрывать три проfilя оборудования ниже. Les programmes hors de chaque profil sont ouverts avant la publication.

  | Profil | Architecture | Processeur/Instance | Compilateur de drapeaux | Назначение |
  | --- | --- | --- | --- | --- |
  | `baseline-simd-neutral` | `x86_64` | AMD EPYC 7B12 (32c) ou Intel Xeon Gold 6430 (24c) | `RUSTFLAGS="-C target-feature=-avx,-avx2,-fma"` | Устанавливает базовые значения без векторных инстRUкций; используется для настройки tableaux de coûts de secours. |
  | `baseline-avx2` | `x86_64` | Intel Xeon Or 6430 (24c) | version par défaut | Validez AVX2 ; vérifiez que SIMD est utilisé avec du gaz neutre. |
  | `baseline-neon` | `aarch64` | AWS Graviton3 (c7g.4xlarge) | version par défaut | Il est garanti que le backend NEON assure la détection et l'utilisation des versions x86. |

- **Harnais de référence.** Все отчеты по калибровке газа ДОЛЖНЫ быть получены с:
  -`CRITERION_HOME=target/criterion cargo bench -p iroha_core isi_gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>`
  - `cargo test -p iroha_core bench_repro -- --ignored` pour l'amélioration de la configuration.
  - `CRITERION_HOME=target/criterion cargo bench -p ivm gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>` dans cette section, vous trouverez ici l'opcode de la VM.- **Correction du caractère aléatoire.** Exportez `IROHA_CONF_GAS_SEED=conf-gas-seed-2026Q1` avant d'entrer dans le banc, puis `iroha_test_samples::gen_account_in` est activé sur le périphérique de détection. `KeyPair::from_seed`. Harnais печатает `IROHA_CONF_GAS_SEED_ACTIVE=…` один раз; Si cela s'est produit de manière temporaire, veuillez procéder à la vérification par le ministère de l'Intérieur. Les nouveaux lubrifiants de calibre spécial devraient permettre à cet env de s'adapter à vos besoins.

- **Capture des résultats.**
  - Ajoutez les résumés de critères (`target/criterion/**/raw.csv`) pour chaque profil dans l'artefact de publication.
  - Enregistrez les mesures fournies (`ns/op`, `gas/op`, `ns/gas`) dans [Confidential Gas Calibration ledger] (./confidential-gas-calibration) dans git commit et dans la version actuelle. compilateur.
  - Vérifiez les deux lignes de base du profil ; N'hésitez pas à visionner tous les films après avoir vérifié le nouveau produit.

- **Tolérances d'acceptation.**
  - Les niveaux de gaz correspondent à `baseline-simd-neutral` et `baseline-avx2` Le débit est ≤ ±1,5 %.
  - Les valeurs de gaz correspondent à `baseline-simd-neutral` et `baseline-neon` Le débit est ≤ ±2,0 %.
  - Les prévisions de calibrage qui s'appliquent à ces informations doivent être appliquées aux résolutions correctives, en utilisant RFC avec la résolution des problèmes et merami.- **Liste de contrôle de révision.** Отправители отвечают за :
  - Vérifiez `uname -a`, sélectionnez `/proc/cpuinfo` (modèle, pas à pas) et `rustc -Vv` dans le journal calibré.
  - Vérifiez que `IROHA_CONF_GAS_SEED` est activé dans votre bac (le bac contient des graines actives).
  - En outre, les indicateurs de fonctionnalité de stimulateur cardiaque et de vérificateur confidentiel sont compatibles avec la production (`--features confidential,telemetry` pour l'utilisation de la télémétrie).

## Configuration et fonctionnement
- `iroha_config` indique le sexe `[confidential]` :
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
- Les mesures télémétriques émettent les valeurs suivantes : `confidential_proof_verified`, `confidential_verifier_latency_ms`, `confidential_proof_bytes_total`, `confidential_nullifier_spent`, `confidential_commitments_appended`, `confidential_mempool_rejected_total{reason}` et `confidential_policy_transitions_total` ne diffusent pas de texte en clair.
- Surfaces RPC :
  -`GET /confidential/capabilities`
  -`GET /confidential/zk_registry`
  -`GET /confidential/params`## Test de stratégie
- Détermination : transmission aléatoire des racines de Merkle et des ensembles d'annulations.
- Устойчивость к reorg: симуляция многоблочных reorg с ancres ; les annuleurs остаются стабильными, а устаревшие les ancres отклоняются.
- Gaz disponible : un gaz spécifique à votre usage et sans l'assistance SIMD.
- Tests approfondis : preuves sur les appareils de chauffage/gaz, максимальные входы/выходы, délai d'attente d'application.
- Thème : gouvernance des opérations pour l'activité/la désactivation du vérificateur et des paramètres, tests de rotation.
- Politique FSM : разрешенные/запрещенные переходы, задержки en attente de transition et отклонения mempool вокруг hauteurs effectives.
- Чрезвычайные ситуации registre: retrait d'urgence замораживает затронутые активы на `withdraw_height` и отклоняет preuves после нее.
- Contrôle des capacités : VALIDATORS с несовпадающими `conf_features` отклоняют bloki ; les observateurs avec `assume_valid=true` продолжают следовать без влияния на консенсус.
- Fonctionnalité exclusive : le validateur/complet/observateur permet d'identifier les racines de l'état dans les sections canoniques.
- Fuzzing négatif : preuves avancées, charges utiles surdimensionnées et annulateur de collisions détectés de manière détectable.## Migration et configuration
- Déploiement limité aux fonctionnalités : pour la phase C3 `enabled` pour le remplacement de `false` ; Nous utilisons les capacités nécessaires avant de les valider.
- Прозрачные активы не затронуты; Les instructions confidentielles permettent de localiser le registre et les capacités de gestion.
- Узлы, собранные без поддержки confidentiels, детерминированно отклоняют сооответствующие bloki ; On ne peut pas s'adresser aux validateurs, mais on peut travailler comme observateurs avec `assume_valid=true`.
- Genesis manifeste toutes les entrées de registre, les paramètres, les politiques confidentielles pour les activités et les clés d'auditeur opérationnelles.
- Les opérateurs sélectionnent les runbooks disponibles pour le registre de rotation, les politiques et le retrait d'urgence, qui permettent de déterminer les résultats.## Незавершенная работа
- Testez le benchmark sur les paramètres de Halo2 (circuit de réglage, recherche de stratégie) et affichez les résultats dans le playbook d'étalonnage, pour éliminer les déficits de gaz/d'expiration en fonction de votre tâche. обновлением `confidential_assets_calibration.md`.
- Prend en compte les politiques de confidentialité de l'auditeur et l'API de visualisation sélective, en ajoutant un flux de travail global à Torii après la gouvernance.
- Améliorez le système de chiffrement des témoins pour les sorties multi-destinataires et les mémos par lots, en fournissant une enveloppe de format aux implémenteurs du SDK.
- Recherchez les circuits d'examen de sécurité, le registre et les paramètres de rotation des procédures et archivez les résultats des rapports d'audit.
- Spécifiez les dépenses de l'API pour les auditeurs et fournissez des conseils sur la portée des touches d'affichage, que les vendeurs ont réalisés pour vos attestations sémantiques.## Étapes de réalisation
1. **Phase M0 — Durcissement d'arrêt du navire**
   - ✅ L'annuleur de dérivation est la conception du Poséidon PRF (`nk`, `rho`, `asset_id`, `chain_id`) pour déterminer les engagements dans le grand livre actuel.
   - ✅ Application de limites de sécurité et de transactions confidentielles par transaction/par bloc, en supprimant les transactions selon les limites детерминированными ошибками.
   - ✅ La poignée de main P2P utilise `ConfidentialFeatureDigest` (résumé du backend + registre d'ouverture) et l'ouverture déterminée n'est pas disponible avec `HandshakeConfidentialMismatch`.
   - ✅ Ajoutez des paniques dans les chemins d'exécution confidentiels et créez un contrôle de rôle pour les utilisateurs non autorisés.
   - ⚪ Применить бюджеты timeout pour le vérificateur et les limites de réorganisation des points de contrôle frontaliers.
     - ✅ Бюджеты timeout для проверки применены; preuves, превышающие `verify_timeout_ms`, теперь детерминированно падают.
     - ✅ Points de contrôle frontaliers теперь учитывают `reorg_depth_bound`, удаляя checkpoints старше заданного окна при сохранении детерминированных снимков.
   - Ввести `AssetConfidentialPolicy`, politique FSM et portes d'application pour l'instruction menthe/transfert/révélation.
   - Enregistrez `conf_features` dans les blocs de sauvegarde et ouvrez les validateurs pour les résumés de registre/paramètres.
2. **Phase M1 — Registres et paramètres**- Supprimer le registre `ZkVerifierEntry`, `PedersenParams`, `PoseidonParams` pour les opérations de gouvernance, l'ancrage de la genèse et la mise en œuvre du système.
   - Prend en charge les appels système pour les recherches de registre, les identifiants de programme de gaz, le hachage de schéma et les vérifications de taille.
   - Publiez le format de charge utile v1, la clé de dérivation des vecteurs et la CLI pour l'utilisation des clés confidentielles.
3. **Phase M2 — Gaz et performances**
   - Réaliser le programme de gaz de détection, les séquences par bloc et les faisceaux de référence par télémétrie (vérifier la latence, les tailles d'épreuve, les rejets de mémoire).
   - Récupérez les points de contrôle CommitmentTree, le chargement LRU et les index d'annulation pour plusieurs activités.
4. **Phase M3 — Outillage de rotation et de portefeuille**
   - Включить подддержку multi-paramètres et preuves multi-versions ; prendre en charge l'activation/dépréciation basée sur la gouvernance et les runbooks de transition.
   - Publier les paramètres de migration pour le SDK/CLI du portefeuille, l'analyse des flux de travail des auditeurs et les dépenses en outils.
5. **Phase M4 — Audit et opérations**
   - Préparer les workflows pour les clés d'auditeur, l'API de divulgation sélective et les runbooks opérationnels.
   - Planifiez la vérification de la cryptographie et l'ouverture des documents dans `status.md`.

À ce moment-là, nous établissons les jalons de la feuille de route et les tests, afin de déterminer les garanties d'utilisation pour l'ensemble de la blockchain.
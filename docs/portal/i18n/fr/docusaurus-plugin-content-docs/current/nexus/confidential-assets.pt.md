---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/confidential-assets.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : Ativos confidenciais e transferencias ZK
description : Plan directeur Phase C pour la circulation aveugle, les registres et les contrôles de l'opérateur.
slug : /nexus/actifs-confidentiels
---
<!--
SPDX-License-Identifier: Apache-2.0
-->
# Conception d'actifs confidentiels et transférentiels ZK

## Motivation
- Entrer des flux d'activité aveugles opt-in pour que les domaines préservent la confidentialité transactionnelle sans modifier la circulation transparente.
- Comment exécuter la détermination du matériel hétérogène des validateurs et conserver Norito/Kotodama ABI v1.
- Fornecer les auditeurs et les opérateurs contrôlent le cycle de vie (ativacao, rotacao, revogacao) pour les circuits et les paramètres cryptographiques.

## Modèle de menace
- Validadores sao honnête-mais-curieux : exécutez le consensus effectivement mais tentez d'inspecter le grand livre/l'État.
- Les Observadores de rede veem dados de bloco e transacoes ont bavardé ; nenhuma supposao de canais privados de gossip.
- Fora de escopo : analyse du trafic hors grand livre, adversaires quantiques (accompagné sans feuille de route PQ), attaques de disponibilité du grand livre.## Visao général faire la conception
- Les actifs peuvent être déclarés dans un *pool protégé* alem dos balances transparentes existentes ; une circulation aveugle et représentée via des engagements cryptographiques.
- Notes encapsulam `(asset_id, amount, recipient_view_key, blinding, rho)` com :
  - Engagement : `Comm = Pedersen(params_id || asset_id || amount || recipient_view_key || blinding)`.
  - Nullifier : `Null = Poseidon(domain_sep || nk || rho || asset_id || chain_id)`, indépendant de l'ordre des notes.
  - Charge utile encodée : `enc_payload = AEAD_XChaCha20Poly1305(ephemeral_shared_key, note_plaintext)`.
- Charges utiles de transport Transacoes `ConfidentialTransfer` codifiées dans Norito selon :
  - Entrées publiques : ancre Merkle, annulateurs, nouveaux engagements, identifiant d'actif, versao de circuito.
  - Charges utiles écrites pour les destinataires et les auditeurs optionnels.
  - Preuve de connaissance nulle que atesta conservacao de valor, property e autorizacao.
- Vérification des clés et des paramètres contrôlés via les registres sur grand livre avec les paramètres d'activation ; nodes recusam validar proofs que referenciam entradas desconhecidas ou revogadas.
- En-têtes de consensus compromis ou résumé de capacité confidentielle pour que les blocs soient ainsi pris en compte lorsque l'état de registre et les paramètres coïncident.
- Construction de preuves à l'aide d'une pile Halo2 (Plonkish) avec une configuration fiable ; Groth16 ou d'autres variantes de SNARK sont intentionnellement prises en charge dans la v1.

### Calendriers déterministesLes enveloppes de mémo confidentiel sont maintenant envoyées à un appareil canonico em `fixtures/confidential/encrypted_payload_v1.json`. L'ensemble de données capture une enveloppe v1 positive mais présente des aspects négatifs et malformés pour que les SDK puissent confirmer la parité d'analyse. Les tests du modèle de données dans Rust (`crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs`) et la suite Swift (`IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift`) chargent directement l'appareil, garantissant l'encodage Norito, en tant que surface d'erreur et couverture de régression permanente en ce qui concerne l'évolution du codec.

Les SDK Swift peuvent désormais émettre des instructions pour la colle sans colle JSON sur mesure : construire un
`ShieldRequest` avec l'engagement de 32 octets, la charge utile encryptée et les métadonnées de débit,
et entao chame `IrohaSDK.submit(shield:keypair:)` (ou `submitAndWait`) pour assassiner et encaminhar a
transacao via `/v1/pipeline/transactions`. O helper valida comprimentos de engagement,
insérer `ConfidentialEncryptedPayload` sans encodeur Norito, et afficher la disposition `zk::Shield`
décrit abaixo pour que les portefeuilles soient adaptés à Rust.## Engagements de consensus et contrôle des capacités
- En-têtes de bloco expoem `conf_features = { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }` ; Le résumé participe au hachage de consensus et doit être similaire au visa local du registre pour l'acitacao de bloco.
- Le module de gouvernance prépare les mises à niveau programmées entre `next_conf_features` et `activation_height` à l'avenir ; mangé essa altura, les producteurs de bloco doivent continuer à émettre ou digérer antérieurement.
- Les nœuds validateurs DEVEM fonctionnent avec `confidential.enabled = true` et `assume_valid = false`. Les vérifications de démarrage ne sont pas entrées dans le validateur si cela est conditionnel ou si la divergence locale `conf_features` est effectuée.
- Une métadonnée de prise de contact P2P agora inclut `{ enabled, assume_valid, conf_features }`. Les pairs qui annoncent des caractéristiques incompatibles sont rejetés avec `HandshakeConfidentialMismatch` et n'entrent jamais dans la rotation de consensus.
- Résultats de la poignée de main entre les validateurs, les observateurs et les pairs qui sont capturés dans la matrice de poignée de main dans [Négociation des capacités du nœud] (#node-capability-negotiation). Les Falhas de handshake expoem `HandshakeConfidentialMismatch` et les forums de pairs de la rotation de consensus ont fait que le résumé coïncidait.
- Les observateurs nao validadores peuvent définir `assume_valid = true` ; aplicam deltas confidenciais sem verificar proofs, mas nao influenciam a seguranca do consenso.## Politique des actifs
- Chaque définition d'actif carrega um `AssetConfidentialPolicy` est définie par le créateur ou via la gouvernance :
  - `TransparentOnly` : mode par défaut ; apenas instrucoes transparentes (`MintAsset`, `TransferAsset`, etc.) sao permitidas e operacoes blinded sao rejeitadas.
  - `ShieldedOnly` : tous les émetteurs et transferts doivent utiliser des instructions confidentielles ; `RevealConfidential` est garanti pour que les soldes ne soient pas affichés publiquement.
  - `Convertible` : les supports peuvent déplacer la valeur entre les représentants transparents et les instructions d'utilisation blindées de l'abaixo de rampe d'accès/de sortie.
- La politique impose au FSM de restreindre ses fonds pour éviter de les encaisser :
  - `TransparentOnly -> Convertible` (habilitation immédiate de la piscine protégée).
  - `TransparentOnly -> ShieldedOnly` (demander un transfert pendant et une conversation).
  - `Convertible -> ShieldedOnly` (délai minimum obligatoire).
  - `ShieldedOnly -> Convertible` (plan de migration requis pour que les notes aveugles continuent à gastaveis).
  - `ShieldedOnly -> TransparentOnly` et il est moins probable que le pool protégé soit en cours ou que la gouvernance codifie une migration des notes aveugles pendantes.
- Les instructions de gouvernance définissent `pending_transition { new_mode, effective_height, previous_mode, transition_id, conversion_window }` via l'ISI `ScheduleConfidentialPolicyTransition` et peuvent abandonner les modifications programmées avec `CancelConfidentialPolicyTransition`. La validation du mémoire garantit que le transport transfrontalier s'effectue à une hauteur de transit et inclut une vérification déterministe de la situation politique dans le cadre du blocus.- Les transferts pendants sont appliqués automatiquement lorsqu'un nouveau bloc s'ouvre : lorsque la hauteur entre dans la fenêtre de conversion (pour les mises à niveau `ShieldedOnly`) ou la mise à jour `effective_height`, ou le runtime actualisé `AssetConfidentialPolicy`, actualise les métadonnées. `zk.policy` et nettoyer l'entrée pendante. Pour fournir une fourniture transparente permanente lorsqu'une transition `ShieldedOnly` est terminée, le runtime interrompt la transition et enregistre un avis, en attendant ou en mode antérieur.
- Les boutons de configuration `policy_transition_delay_blocks` et `policy_transition_window_blocks` impoem aviso minimo e periodos de tolerancia para permitir converses de wallet em torno da mudanca.
- `pending_transition.transition_id` également fonctionnel comme poignée de salle ; la gouvernance doit être citée pour finaliser ou annuler les transitions pour les opérateurs corrélant les relations de rampe d'accès/sortie.
- `policy_transition_window_blocks` par défaut à 720 (~12 heures avec temps de bloc de 60 s). Les nœuds limitent les demandes de gouvernance qui tentent d’être avisées plus rapidement.
- Genesis manifeste et fluxos CLI expoem politicas atuais e pendentes. La logique d'admission de la politique dans le temps d'exécution pour confirmer que chaque instruction confidentielle est autorisée.
- Liste de contrôle de migration - voir "Séquençage de la migration" disponible pour le plan de mise à niveau dans les étapes que le Milestone M0 accompagne.

#### Monitorando transicoes via ToriiPortefeuilles et auditeurs consultent `GET /v1/confidential/assets/{definition_id}/transitions` pour inspecter ou `AssetConfidentialPolicy` ativo. La charge utile JSON inclut toujours l'identifiant d'actif canonique, la dernière hauteur de bloc observée, le `current_mode` de la politique, le mode d'effet de cette hauteur (les nouvelles de conversation rapportent temporairement `Convertible`), et les identifiants attendus de `vk_set_hash`/Poséidon/Pedersen. Quand une transition de gouvernance est en attente de réponse, elle s'engage également :

- `transition_id` - poignée de salle retournée par `ScheduleConfidentialPolicyTransition`.
-`previous_mode`/`new_mode`.
-`effective_height`.
- `conversion_window` et le dérivé `window_open_height` (le bloc où les portefeuilles développent des conversations pour les basculements ShieldedOnly).

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

La réponse `404` indique que la définition du correspondant d'actif existe. Quando nao ha transicao agendada o campo `pending_transition` et `null`.

### Machine des états politiques| Mode actuel | Mode proche | Prérequis | Traitement à haute efficacité | Notes |
|----------------------------------------------------|-------------------------------------------------------------------------------------------------|----------------------------------------------------------------------------------------------------------------------------------|-----------------------------------------------------------------------------------------------|
| Transparent uniquement | Cabriolet | Gouvernance ativou entrées de registre de vérificateur/paramètres. Sous-mètre `ScheduleConfidentialPolicyTransition` avec `effective_height >= current_height + policy_transition_delay_blocks`. | Une transition exécutée exactement dans `effective_height` ; o piscine blindée fica disponible immédiatement.               | Chemin par défaut pour permettre la confidentialité en gardant des flux transparents.          |
| Transparent uniquement | Blindé uniquement | Mesmo acima, mais `policy_transition_window_blocks >= 1`.                                                         | Le runtime entre automatiquement dans `Convertible` dans `effective_height - policy_transition_window_blocks` ; modifié pour `ShieldedOnly` et `effective_height`. | Fornece Janela de conversao déterminista avant de désactiver instrucoes transparentes.   || Cabriolet | Blindé uniquement | Transmission programmée avec `effective_height >= current_height + policy_transition_delay_blocks`. Certificat de gouvernance DEVE (`transparent_supply == 0`) via métadonnées d'auditoire ; L'application d'exécution n'est pas non plus un basculement. | Sémantique de janvier identique. Voir la fourniture transparente pour nao-zero em `effective_height`, une transition interrompue avec `PolicyTransitionPrerequisiteFailed`. | Travail ou actif en circulation totalement confidentiel.                                      |
| Blindé uniquement | Cabriolet | Transicao programmé; sem retrait d’urgence ativo (`withdraw_height` indefinido).                              | L'état est changé en `effective_height` ; révéler les rampes reabrem enquanto notes blindadas permanecem validas.             | Utilisé pour les travaux manuels ou les révisions des auditeurs.                                |
| Blindé uniquement | Transparent uniquement | La gouvernance doit être vérifiée `shielded_supply == 0` ou préparer un plan `EmergencyUnshield` assassiné (assinaturas de auditoridas requeridas). | Le runtime ouvre une fenêtre `Convertible` avant `effective_height` ; En haute, les instructions confidentielles Falham duro et le retour des actifs en mode transparent uniquement. | Saida de dernier recurso. La transition s'auto-annule est quelle que soit la note confidentielle pour gasta durante a janela. || N'importe quel | Identique à l'actuel | `CancelConfidentialPolicyTransition` nettoyer le pendentif pendant.                                                    | `pending_transition` retiré immédiatement.                                                                        | Mantem ou statu quo ; affiché pour la complétude.                                             |

Transicoes nao listadas acima sao rejeitadas lors de la soumission de la gouvernance. Le runtime vérifie les conditions préalables au logo avant d'appliquer une transmission programmée ; Les Falhas dévoluent à l'actif de manière antérieure et émettent `PolicyTransitionPrerequisiteFailed` via la télémétrie et les événements de bloc.

### Séquence de migration1. **Préparer les registres :** indiquer toutes les entrées du vérificateur et les paramètres référencés par la politique d'alvo. Les nœuds annoncent le résultat `conf_features` pour que les pairs vérifient la cohérence.
2. **Agenda du transfert :** sous-mètre `ScheduleConfidentialPolicyTransition` avec `effective_height` qui répond à `policy_transition_delay_blocks`. Pour déménager pour `ShieldedOnly`, précisez une personne de conversation (`window >= policy_transition_window_blocks`).
3. **Guide public pour les opérateurs :** registraire du `transition_id` renvoyé et circulaire d'un runbook sur/de sortie. Portefeuilles et auditeurs assinam `/v1/confidential/assets/{id}/transitions` pour ouvrir la hauteur d'ouverture de Janela.
4. **Appliquer janvier :** lorsque janvier ouvre, le runtime change la politique pour `Convertible`, émet `PolicyTransitionWindowOpened { transition_id }`, et vient rejeter les demandes de gouvernance conflictuelles.
5. **Finaliser ou abandonner :** dans `effective_height`, ou vérifier à l'exécution les prérequis (fournir zéro transparent, sans retrait d'urgence, etc.). Réussir la politique pour le mode sollicité ; falha émet `PolicyTransitionPrerequisiteFailed`, nettoie le transit pendant et deixa la politique inaltérée.
6. **Mises à niveau du schéma :** après une transition bien réussie, la gouvernance augmente la conversion du schéma d'actif (par exemple, `asset_definition.v2`) et l'outil CLI exige `confidential_policy` pour sérialiser les manifestes. Les documents de mise à niveau de Genesis instruisent les opérateurs sur les paramètres politiques et les empreintes digitales du registre avant de réinitialiser les validateurs.Des nouvelles qui ont commencé avec la confidentialité habilitée à codifier la politique voulue directement dans la genèse. Ainda assim seguem a checklist acima quando mudam modos pos-launch para que janelas de conversation sejam déterministas e wallets tenham tempo de ajustar.

### Version et activation du manifeste Norito- Genesis manifeste DEVEM incluant un `SetParameter` pour une clé personnalisée `confidential_registry_root`. La charge utile et le JSON Norito qui correspondent à `ConfidentialRegistryMeta { vk_set_hash: Option<String> }` : omettez le champ (`null`) lorsque vous n'êtes pas entré, ou créez une chaîne hexadécimale de 32 octets (`0x...`) ou le hachage est produit par `compute_vk_set_hash` sur les instructions du vérificateur envoyé sans manifeste. Les nœuds peuvent démarrer avec un paramètre erroné ou avec un hachage divergent des écritures de registre codifiées.
- Le fil `ConfidentialFeatureDigest::conf_rules_version` intègre une versao de mise en page du manifeste. Pour redes v1 DEVE permanent `Some(1)` et similaire à `iroha_config::parameters::defaults::confidential::RULES_VERSION`. Lorsque l'ensemble de règles évolue, augmentez la constante, régénérez les manifestes et exécutez le déploiement des binaires en mode verrouillé ; misturar versoes faz validadores rejeitarem blocos com `ConfidentialFeatureDigestMismatch`.
- L'activation manifeste DEVEM agrupar Updates of Registry, mudancas de cycle de vida de parametros e transicos de politica para manter o digeste cohérent :
  1. Appliquer les modifications des plans de registre (`Publish*`, `Set*Lifecycle`) dans une vue hors ligne de l'état et calculer le résumé des positions actives avec `compute_confidential_feature_digest`.
  2. Émettez `SetParameter::custom(confidential_registry_root, {"vk_set_hash": "0x..."})` en utilisant le hachage calculé pour que les pairs attrasados ​​récupèrent le résumé correto me se perderem instrucoes intermédiaires.3. Anexar instruit `ScheduleConfidentialPolicyTransition`. Cada instrucao deve citar o `transition_id` émis pour la gouvernance ; manifeste que l'exécution sera rejetée lors de l'exécution.
  4. Persister les octets se manifestent, une empreinte digitale SHA-256 et la digestion utilisée dans le plan d'activation. Les opérateurs vérifient les trois artefatos avant de voter ou de manifester pour éviter les partis.
- Lorsque les déploiements exigent un basculement différent, enregistrez une hauteur également dans un paramètre personnalisé associé (par exemple `custom.confidential_upgrade_activation_height`). Cela oblige les auditeurs à prouver qu'ils sont codifiés dans le Norito et que les validateurs honorent la janvier de l'avis avant d'entrer dans l'effet.## Cycle de vie des vérificateurs et paramètres
### Registre ZK
- Le grand livre armazena `ZkVerifierEntry { vk_id, circuit_id, version, proving_system, curve, public_inputs_schema_hash, vk_hash, vk_len, max_proof_bytes, gas_schedule_id, activation_height, deprecation_height, withdraw_height, status, metadata_uri_cid, vk_bytes_cid }` est actuellement `proving_system` et fixé sur `Halo2`.
- Pares `(circuit_id, version)` globalement unique ; Le registre contient un indice secondaire pour les recherches de métadonnées de circuit. Tentativas de registrar um par duplicado sao rejeitadas durante admission.
- `circuit_id` doit être nao vazio et `public_inputs_schema_hash` doit être fourni (typiquement un hachage Blake2b-32 pour l'encodage canonique des entrées publiques du vérificateur). Admission rejeita registros qui omitem esses campos.
- Les instructions de gouvernance comprennent :
  - `PUBLISH` pour ajouter une entrée `Proposed` portant sur les métadonnées.
  - `ACTIVATE { vk_id, activation_height }` pour programmer l'activation à la limite de l'époque.
  - `DEPRECATE { vk_id, deprecation_height }` para marcar a altura final onde proofs podem referenciar a entrada.
  - `WITHDRAW { vk_id, withdraw_height }` pour le déslignage d'urgence ; actifs afetados congelam gastos confidenciais apos retirer hauteur mangé novas entradas ativarem.
- Genesis manifeste automatiquement un paramètre personnalisé `confidential_registry_root` et `vk_set_hash` coïncide avec les entrées actives ; La validation cruza est un résumé de l'état local du registre avant qu'un nœud puisse entrer sans consensus.- Registrar ou actualiser une demande de vérificateur `gas_schedule_id` ; La vérification exige que l'entrée du registre soit `Active`, ne présente aucun indice `(circuit_id, version)`, et que les preuves Halo2 fornecam avec `OpenVerifyEnvelope` avec `circuit_id`, `vk_hash`, et `public_inputs_schema_hash` correspondam au registre du registre.

### Prouver les clés
- Prouver les clés ficam hors grand livre mais aussi référencées par les identificadores au contenu adressé (`pk_cid`, `pk_hash`, `pk_len`) publiées à partir des métadonnées du vérificateur.
- SDK de portefeuille prenant en charge les données de PK, vérifiant les hachages et le cache local.

### Paramètres Pedersen et Poséidon
- Registres séparés (`PedersenParams`, `PoseidonParams`) contrôlant le cycle de vie des vérificateurs, chacun avec `params_id`, hachages de générateurs/constantes, actifs, dépréciés et hauteurs de retrait.
- Les engagements et les hachages séparent les domaines par `params_id` pour que la rotation des paramètres ne réutilise pas les blocs de bits des ensembles obsolètes ; o ID et embutido em engagements de note et tags de dominio de nullifier.
- Les circuits supportent la sélection de plusieurs paramètres au rythme de la vérification ; les ensembles de paramètres obsolètes sont permanents et utilisables avec `deprecation_height`, et les ensembles retirés sont rejetés exactement dans `withdraw_height`.## Ordenacao déterministe et annulateurs
- Cada asset mantem um `CommitmentTree` com `next_leaf_index`; blocos acrescentam commitments em ordem determinista: iterar transacoes na ordem do bloco; dentro de cada transacao iterar outputs shielded por `output_idx` serializado ascendente.
- `note_position` e derivado dos offsets da arvore mas **nao** faz parte do nullifier; ele so alimenta paths de membership dentro do witness da proof.
- A estabilidade do nullifier sob reorgs e garantida pelo design PRF; o input PRF vincula `{ nk, note_preimage_hash, asset_id, chain_id, params_id }`, e anchors referenciam roots Merkle historicos limitados por `max_anchor_age_blocks`.## Flux du grand livre
1. **MintConfidential {asset_id, montant, destinataire_hint }**
   - Demander la politique d'actif `Convertible` ou `ShieldedOnly` ; admission checa autoridade do actif, récupérera `params_id` actuel, amostra `rho`, émettre un engagement, actualiser a arvore Merkle.
   - Emite `ConfidentialEvent::Shielded` avec le nouvel engagement, racine delta de Merkle et hachage de chamada de transacao pour les pistes d'audit.
2. **TransferConfidential {asset_id, proof, circuit_id, version, nullifiers, new_commitments, enc_payloads, Anchor_root, memo }**
   - Syscall VM vérifie la preuve en utilisant l'entrée du registre ; o host garante nullifiers nao usados, engagements anexados deterministicamente e Anchor recente.
   - Le grand livre enregistre les entrées `NullifierSet`, les charges utiles enregistrées pour les destinataires/auditeurs et émettent les annuleurs de résumé `ConfidentialEvent::Transferred`, les sorties ordonnées, le hachage de preuve et les racines Merkle.
3. **RevealConfidential {asset_id, preuve, circuit_id, version, nullifier, montant, destinataire_account, Anchor_root }**
   - Disponivel apenas para actifs `Convertible` ; une preuve valida que la valeur de la note est égale au montant révélé, le solde créditeur du grand livre transparent et queima une note protégée marcando ou annuleur comme gasto.
   - Émite `ConfidentialEvent::Unshielded` avec le montant public, les annulateurs de consommation, les identifiants de preuve et le hachage de la transaction de transaction.## Adicoes et le modèle de données
- `ConfidentialConfig` (nouvelle section de configuration) avec drapeau d'habilitation, `assume_valid`, boutons de gaz/limites, chaîne d'ancrage, backend de vérificateur.
- Schémas `ConfidentialNote`, `ConfidentialTransfer` et `ConfidentialMint` Norito avec octet de vers explicite (`CONFIDENTIAL_ASSET_V1 = 0x01`).
- `ConfidentialEncryptedPayload` implique des octets de mémo AEAD avec `{ version, ephemeral_pubkey, nonce, ciphertext }`, avec `version = CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1` par défaut pour la mise en page XChaCha20-Poly1305.
- Les vecteurs canoniques de dérivation de clé sont présents dans `docs/source/confidential_key_vectors.json` ; tanto o CLI quanto o endpoint Torii régression contre les appareils.
- `asset::AssetDefinition` et `confidential_policy: AssetConfidentialPolicy { mode, vk_set_hash, poseidon_params_id, pedersen_params_id, pending_transition }`.
- `ZkAssetState` persiste ou lie `(backend, name, commitment)` pour les vérificateurs de transfert/unshield ; une exécution de preuves de vérification de la clé référencée ou en ligne correspond à l'engagement enregistré.
- `CommitmentTree` (pour les points de contrôle frontaliers), `NullifierSet` avec `(chain_id, asset_id, nullifier)`, `ZkVerifierEntry`, `PedersenParams`, `PoseidonParams` armazenados em world state.
- Mempool gère les structures transitoires `NullifierIndex` et `AnchorIndex` pour détecter la date limite des duplications et vérifier l'identité de l'ancre.
- Les mises à jour du schéma Norito incluent l'ordre canonique pour les entrées publiques ; tests de garantie aller-retour pour le déterminisme de l'encodage.- Aller-retours de charge utile cryptée ficam fixés via des tests unitaires (`crates/iroha_data_model/src/confidential.rs`). Vecteurs de portefeuille d'accompagnement pour annexer les transcriptions canoniques AEAD pour les auditeurs. `norito.md` documente l'en-tête sur fil pour l'enveloppe.

## Intégration IVM et appel système
- Introduisez l'appel système `VERIFY_CONFIDENTIAL_PROOF` en procédant :
  - `circuit_id`, `version`, `scheme`, `public_inputs`, `proof`, et `ConfidentialStateDelta { asset_id, nullifiers, commitments, enc_payloads }` résultant.
  - Le syscall charge les métadonnées du vérificateur du registre, applique les limites de tamanho/tempo, cobra gas determinista, et donc l'application ou le delta est une preuve de réussite.
- L'hôte expose le trait en lecture seule `ConfidentialLedger` pour récupérer des instantanés de la racine Merkle et de l'état d'annulation ; une bibliothèque Kotodama fournit des aides d'assemblage de témoin et de validation de schéma.
- Documents du forum pointeur-ABI actualisés pour déclarer la mise en page du tampon de preuve et les poignées de registre.## Négociation des capacités du nœud
- La poignée de main annonce `feature_bits.confidential` avec `ConfidentialFeatureDigest { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`. Les validateurs participants demandent `confidential.enabled=true`, `assume_valid=false`, les identifiants du backend du vérificateur d'identité et les résumés correspondant ; ne correspond pas à Falham ou à la poignée de main avec `HandshakeConfidentialMismatch`.
- La configuration prend en charge `assume_valid` pour les observateurs : lorsque vous êtes déstabilisé, vous rencontrez des instructions confidentielles qui gèrent `UnsupportedInstruction` pour déterminer la panique ; quando habilitado, les observateurs ont appliqué les deltas déclarés sem verificar preuves.
- Mempool rejeita transacoes confidenciais se a capacidade local estiver desabilitada. Les filtres à potins évitent d'envoyer des transactions protégées pour les pairs incompatibles en ce qui concerne les identifiants du vérificateur non reconnus dans les limites de tamanho.

### Matrice de poignée de main| Annonce à distance | Résultat pour les nœuds validateurs | Notes de l'opérateur |
|----------------------|----------------------------------|----------------|
| `enabled=true`, `assume_valid=false`, correspondance backend, correspondance digest | Aceito | Les pairs sont au statut `Ready` et participent à la proposition de vote et de diffusion RBC. Nenhuma acao manuel requis. |
| `enabled=true`, `assume_valid=false`, correspondance backend, résumé obsolète ou ausente | Rejeté (`HandshakeConfidentialMismatch`) | La télécommande doit appliquer les pendentifs de registre/paramètres ou sauvegarder le programme `activation_height`. Ate corrigir, o node segue descobrivel mas nunca entra na rotacao de consenso. |
| `enabled=true`, `assume_valid=true` | Rejeté (`HandshakeConfidentialMismatch`) | Les validadores requerem verificacao de proofs ; configurez la télécommande comme observateur avec l'entrée Torii uniquement ou changez `assume_valid=false` pour permettre une vérification complète. |
| `enabled=false`, champs omis (build désaturé), ou backend de vérificateur différent | Rejeté (`HandshakeConfidentialMismatch`) | Les pairs désaturés ou partiellement actualisés ne peuvent entrer dans le réseau de consensus. Actualisez la version actuelle et garantissez que le tuple backend + digest correspond avant la reconectar. |Les observateurs qui, intentionnellement, peuvent vérifier les preuves, doivent ouvrir des liens de consensus contre les validateurs avec les portes de capacité. Il est également possible d'utiliser des blocs via Torii ou des API d'archives, mais en raison du consentement des utilisateurs, ils ont annoncé des capacités compatibles.

### Politique d'élagage de révélation et de maintien de l'annulation

Les grands livres confidentiels doivent être suffisamment historiques pour prouver la fresque des notes et la reproduction des auditoires de gouvernance. Un défaut politique, appliqué par `ConfidentialLedger`, par exemple :- **Retencao de nullifiers:** maintenir les nullifiers gastos por um *minimo* de `730` dias (24 meses) apos a altura de gasto, ou a janela Regulatoria Obrigatoria se for major. Les opérateurs peuvent envoyer Janela via `confidential.retention.nullifier_days`. Annulateurs mais nouveaux que Janela DEVEM consulte en permanence via Torii pour que les auditeurs prouvent l'existence d'une double dépense.
- **Élagage des révélations :** révèle les transparentes (`RevealConfidential`) qui peuvent être associées immédiatement au bloc finalisé, mais l'annulation consommée continue sous réserve de regra de retencao acima. Les événements `ConfidentialEvent::Unshielded` enregistrent le montant public, le destinataire et le hachage de preuve pour que la reconstruction révèle l'historique de l'existence du texte chiffré peut-être.
- **Points de contrôle frontaliers :** les frontières d'engagement mantem checkpoints roulants cobrindo o major entre `max_anchor_age_blocks` et a janela de retencao. Les nœuds compactent les points de contrôle avant l'expiration de tous les annuleurs.
- **Remédiation du digest périmé :** lorsque `HandshakeConfidentialMismatch` corrige la dérive du digest, les opérateurs doivent (1) vérifier que les lignes de rétention des annuleurs ne sont pas alignées sur le cluster, (2) utiliser `iroha_cli app confidential verify-ledger` pour régénérer le digest contre l'ensemble des nullificateurs résiduels, et (3) redéployer le manifeste actualisé. Les annulateurs peuvent être amenés prématurément à restaurer l'entrepôt frigorifique avant de réintégrer le réseau.Le document remplace localement le runbook des opérations ; Les politiques de gouvernance qui s'étendent à la période de rétention doivent actualiser la configuration des nœuds et des plans de stockage des archives en lockstep.

### Flux d'expulsion et de récupération

1. Pendant le cadran, `IrohaNetwork` compare les capacités annoncées. Qualquer inadéquation levanta `HandshakeConfidentialMismatch` ; La connexion et la recherche d'un pair permanent sur le fil de découverte ont été promues par `Ready`.
2. Si aucun journal du service réseau n'apparaît (y compris le résumé à distance et le backend), le Sumeragi n'a ​​pas d'agenda ou de peer pour proposer ou voter.
3. Les opérateurs corrigent les registres du vérificateur et les ensembles de paramètres (`vk_set_hash`, `pedersen_params_id`, `poseidon_params_id`) ou programment `next_conf_features` avec le `activation_height` en accord. Une fois que le résumé coïncide, la prochaine poignée de main leur réussit automatiquement.
4. Si un pair obsolète entraîne le financement d'un bloc (par exemple, via la relecture de l'archive), les validateurs ou les demandeurs de manière déterminée avec `BlockRejectionReason::ConfidentialFeatureDigestMismatch`, en gardant l'état du grand livre cohérent sur le réseau.

### Flux de poignée de main sécurisé contre la relecture1. Cada tentativa outbound aloca materials de chave Noise/X25519 novo. La charge utile de la poignée de main assinée (`handshake_signature_payload`) est liée aux chaînes publiques des données locales et à distance, l'expéditeur du socket annoncé codifié dans Norito et, lorsqu'il est compilé avec `handshake_chain_id`, l'identifiant de la chaîne. Un message et écrit avec AEAD avant de démarrer le nœud.
2. Le répondeur recalcule la charge utile avec la commande de chaves peer/local inversée et vérifie l'Assinatura Ed25519 embutida em `HandshakeHelloV1`. En tant que chaves éphémères et l'annonce annoncée, faites partie du domaine de l'assistanat, rejouez un message capturé contre un autre pair ou récupérez une conversation périmée de manière déterministe.
3. Drapeaux de capacité confidentielle et `ConfidentialFeatureDigest` via le dentro de `HandshakeConfidentialMeta`. O récepteur comparé au tuple `{ enabled, assume_valid, verifier_backend, digest }` avec `ConfidentialHandshakeCaps` local ; quel que soit l'inadéquation avec `HandshakeConfidentialMismatch` avant le transport en transit pour `Ready`.
4. Les opérateurs DEVEM recalculent le résumé (via `compute_confidential_feature_digest`) et réinitialisent les nœuds avec les registres/politiques actualisés avant la reconectar. Les pairs anunciando digèrent les antigos continuam falhando o handshake, evitando que estado stale reentre no set validador.5. Les réussites et les échecs de poignée de main actualisent les contadores `iroha_p2p::peer` (`handshake_failure_count`, assistants de taxonomie des erreurs) et émettent des journaux créés avec l'identification d'homologue à distance et l'empreinte digitale pour le résumé. Surveillez ces indicateurs pour détecter les replays ou les erreurs de configuration pendant le déploiement.

## Gestion des clés et charges utiles
- Hiérarchie de dérivation par compte :
  - `sk_spend` -> `nk` (clé d'annulation), `ivk` (clé de visualisation entrante), `ovk` (clé de visualisation sortante), `fvk`.
- Charges utiles de notes encriptadas usam AEAD avec clés partagées dérivées de ECDH ; voir les clés de l'auditeur opcionais peut être anexadas a sorties conformes à la politique des actifs.
- Ajout de la CLI : `confidential create-keys`, `confidential send`, `confidential export-view-key`, outils d'auditeur pour la description des mémos, et l'assistant `iroha app zk envelope` pour produire/inspecter les enveloppes Norito hors ligne.
- Calendrier de gaz déterministe :
  - Halo2 (Plonkish) : base `250_000` gaz + `2_000` gaz par entrée publique.
  - `5` gas por proof byte, mais cargos por nullifier (`300`) et por engagement (`500`).
  - Les opérateurs peuvent enregistrer ces constantes via la configuration du nœud (`confidential.gas.{proof_base, per_public_input, per_proof_byte, per_nullifier, per_commitment}`) ; Il s'agit d'une propagation sans démarrage ou sans rechargement à chaud de la caméra de configuration et appliquée de manière déterministe sur le cluster.
- Limites durables (configurées par défaut) :
-`max_proof_size_bytes = 262_144`.
-`max_nullifiers_per_tx = 8`, `max_commitments_per_tx = 8`, `max_confidential_ops_per_block = 256`.
- `verify_timeout_ms = 750`, `max_anchor_age_blocks = 10_000`. Preuves que dépasser `verify_timeout_ms` a avorté une instrucao deterministicamente (les bulletins de vote de gouvernance émettent `proof verification exceeded timeout`, `VerifyProof` renvoient une erreur).
- Quotas supplémentaires garantissant la vivacité : `max_proof_bytes_block`, `max_verify_calls_per_tx`, `max_verify_calls_per_block` et `max_public_inputs` constructeurs de blocs limités ; `reorg_depth_bound` (>= `max_anchor_age_blocks`) régit le maintien des points de contrôle frontaliers.
- L'exécution du runtime a récemment rejeté les transactions qui dépassent ses limites de transaction ou de blocage, émettant des erreurs `InvalidParameter` déterministes et en attendant l'état du grand livre modifié.
- Mempool pré-filtre les transactions confidentielles par `vk_id`, tamanho de preuve et identité d'ancre avant d'invoquer ou de vérificateur pour une utilisation des ressources limitées.- Une vérification pour déterminer de manière déterministe le délai d'attente ou le dépassement de limite ; transacoes falham avec erreurs explicites. Backends SIMD est optionnel mais ne modifie pas la comptabilité du gaz.

### Baselines de calibrage et portes d'huile
- **Plataformas de referencia.** Rodadas de calibracao DEVEM cobrir os tres perfis abaixo. Rodadas sem todos os perfis sao rejeitadas na review.

  | Profil | Architecture | CPU / Instanciation | Drapeaux du compilateur | Proposé |
  | --- | --- | --- | --- | --- |
  | `baseline-simd-neutral` | `x86_64` | AMD EPYC 7B12 (32c) ou Intel Xeon Gold 6430 (24c) | `RUSTFLAGS="-C target-feature=-avx,-avx2,-fma"` | Estabelece valores piso sem intrinsèques vetoriais; utilisé pour ajuster les tables de repli du client. |
  | `baseline-avx2` | `x86_64` | Intel Xeon Or 6430 (24c) | version par défaut | Valider le chemin AVX2 ; checa se os ganhos SIMD ficam dentro da tolerancia do gas neutral. |
  | `baseline-neon` | `aarch64` | AWS Graviton3 (c7g.4xlarge) | version par défaut | Garantissez que le backend NEON est permanent et déterminé jusqu'aux horaires x86. |

- **Harnais de référence.** Tous les rapports d'étalonnage du gaz DEVEM sont produits avec :
  -`CRITERION_HOME=target/criterion cargo bench -p iroha_core isi_gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>`
  - `cargo test -p iroha_core bench_repro -- --ignored` pour confirmer la détermination du luminaire.
  - `CRITERION_HOME=target/criterion cargo bench -p ivm gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>` toujours le responsable de l'opcode de la VM.- ** Correction du hasard. ** Exportez `IROHA_CONF_GAS_SEED=conf-gas-seed-2026Q1` avant de monter des bancs pour que `iroha_test_samples::gen_account_in` soit adapté au chemin déterminant `KeyPair::from_seed`. O harnais imprime `IROHA_CONF_GAS_SEED_ACTIVE=...` uma vez; voir un faltar variable, une revue DEVE falhar. Quelle que soit la nouvelle utilisation de l'étalonnage, il faut continuer à honorer cet environnement en introduisant un auxiliaire aléatoire.

- **Capture des résultats.**
  - Télécharger les résumés Critère (`target/criterion/**/raw.csv`) pour chaque profil dans l'artefato de release.
  - Armazenar metricas derivadas (`ns/op`, `gas/op`, `ns/gas`) no [Confidential Gas Calibration ledger](./confidential-gas-calibration) avec le commit git et la version du compilateur utilisé.
  - Manter nos deux dernières lignes de base par profil ; apagar snapshots mais antigos uma vez validado o relatorio plus novo.

- **Tolérances d'huile.**
  - Deltas de gaz entre `baseline-simd-neutral` et `baseline-avx2` DEVEM permanents <= +/-1,5%.
  - Deltas de gaz entre `baseline-simd-neutral` et `baseline-neon` DEVEM permanents <= +/-2,0%.
  - Les propositions d'étalonnage qui dépassent ces seuils nécessitent d'ajuster le calendrier ou une RFC expliquant un écart et une atténuation.- **Liste de contrôle de révision.** Les auteurs sont responsables de :
  - Inclut `uname -a`, les trechos de `/proc/cpuinfo` (modèle, pas à pas), et `rustc -Vv` sans journal de calibrage.
  - Vérifiez que `IROHA_CONF_GAS_SEED` apparaît sur le banc (car les bancs sont imprimés à la graine).
  - Garantir que les indicateurs de fonctionnalité du stimulateur cardiaque et du vérificateur sont des produits confidentiels (`--features confidential,telemetry` sur les bancs de rodage avec télémétrie).

## Configuration et opérations
- `iroha_config` ajouté à la section `[confidential]` :
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
- Telemetria émet des mesures agrégées : `confidential_proof_verified`, `confidential_verifier_latency_ms`, `confidential_proof_bytes_total`, `confidential_nullifier_spent`, `confidential_commitments_appended`, `confidential_mempool_rejected_total{reason}`, et `confidential_policy_transitions_total`, sem expor dados em claro.
- Superficies RPC :
  -`GET /confidential/capabilities`
  -`GET /confidential/zk_registry`
  -`GET /confidential/params`## Stratégie testiculaire
- Déterminisme : randomisation des transactions à l'intérieur des blocs issus des racines de Merkle et des ensembles d'annuleurs identiques.
- Resiliencia a reorg : réorganisations simulées multi-bloco avec ancres ; annuleurs permanents estaveis e ancres obsolètes sao rejeitados.
- Invariants de gaz : vérifier l'utilisation du gaz identique entre les nœuds et avec l'accélération SIMD.
- Tests de limites : preuves sans tamanho/gas, nombre maximum d'entrées/sorties, application du délai d'attente.
- Cycle de vie : opérations de gouvernance pour l'activation/dépréciation du vérificateur et des paramètres, tests de gasto apos rotacao.
- Politique FSM : transicoes permis/négadas, retards de transicao pendente et rejeicao de mempool perto de alturas efetivas.
- Émergences de registre : retrait d'urgence des actifs congelés afetados em `withdraw_height` e rejeita proofs depois.
- Contrôle des capacités : validadores com `conf_features` divergentes rejeitam blocos ; observateurs com `assume_valid=true` accompagnent sem afetar consenso.
- Équivalence de l'état : nœuds validateur/complet/observateur produit des racines de l'état identique à la cadémie canonique.
- Fuzzing négatif : preuves mal formées, charges utiles surdimensionnées et colis de nullificateur sao rejetés de manière déterministe.## Migration
- Indicateur de fonctionnalité de déploiement de com : terminal de phase C3, `enabled` par défaut est `false` ; Les nœuds annoncent les capacités avant l'entrée sans définir le validateur.
- Actifs transparents nao sao afetados; instrucoes confidenciais requerem entradas de Registry e negociacao de capacidades.
- Les nœuds compilados sem soutiennent des blocs confidentiels pertinents de manière déterministe ; nao podem entrerr no set validador mas podem operar como observers com `assume_valid=true`.
- Genesis manifeste des entrées initiales de registre, des ensembles de paramètres, des politiques confidentielles pour les actifs et des clés de l'auditeur optionnel.
- Les opérateurs suivent les runbooks publiés pour la rotation du registre, les transferts politiques et le retrait d'urgence pour les mises à niveau déterministes.## Trabalho pendente
- Benchmark des paramètres Halo2 (tamanho de circuit, stratégie de recherche) et enregistrement des résultats sans playbook de calibrage pour que les valeurs par défaut de gas/timeout soient actualisées juste au prochain rafraîchissement de `confidential_assets_calibration.md`.
- Finaliser la politique de divulgation de l'auditeur et les API de visualisation sélective associées, en connectant le flux de travail approuvé au Torii ainsi que le projet de gouvernance pour l'assassinat.
- Créer un schéma de chiffrement de témoin pour cobrir des sorties multi-destinataires et des mémos par lots, en documentant le format de l'enveloppe pour les implémenteurs du SDK.
- Commissionner une révision de la sécurité externe des circuits, des registres et des procédures de rotation des paramètres et arquivar os achados ao lado dos relatorios internos de auditoria.
- Spécifier les API de réconciliation des dépenses pour les auditeurs et publier le guide d'escopo de view-key pour que les fournisseurs de portefeuille les mettent en œuvre comme mesmas sémantiques de certification.## Phasing de mise en œuvre
1. **Phase M0 - Durcissement Stop-Ship**
   - [x] Derivacao de nullifier segue o design Poseidon PRF (`nk`, `rho`, `asset_id`, `chain_id`) avec la commande déterministe des engagements appliqués aux mises à jour du grand livre.
   - [x] Execucao aplica limites de tamanho de proof e quotas confidenciais por transacao/por bloco, rejeitando transacoes fora de budget com erros deterministas.
   - [x] Handshake P2P annonce `ConfidentialFeatureDigest` (backend digest + empreintes digitales de registre) et de fausses disparités sont déterminées via `HandshakeConfidentialMismatch`.
   - [x] Remover panique les chemins d'exécution confidentiels et un rôle supplémentaire de contrôle pour les nœuds incompatibles.
   - [ ] Appliquer les budgets de délai d'attente pour vérifier et les limites de profondeur de réorganisation pour les points de contrôle frontaliers.
     - [x] Budgets de timeout de verificacao appliqués ; preuves qui dépassent `verify_timeout_ms` agora falham deterministicamente.
     - [x] Points de contrôle frontaliers il y a peu de temps `reorg_depth_bound`, j'ai pu ajouter des points de contrôle plus anciens qu'à janvier configurés et prendre des instantanés déterministes.
   - Introduisez `AssetConfidentialPolicy`, la politique FSM et les portes d'application pour les instructions de création/transfert/révélation.
   - Commit `conf_features` dans nos en-têtes de bloc et récuser la participation des validateurs lorsque les résumés de registre/paramètres divergent.
2. **Phase M1 - Registres et paramètres**- Entrer les registres `ZkVerifierEntry`, `PedersenParams`, et `PoseidonParams` avec les opérations de gouvernance, l'ancrage de la genèse et la gestion du cache.
   - Connectez l'appel système pour effectuer des recherches de registre, des identifiants de planification de gaz, le hachage de schéma et des vérifications de tamanho.
   - Envoi du format de charge utile crypté v1, des anciens dérivés de clés pour le portefeuille et du support CLI pour la gestion des clés confidentielles.
3. **Phase M2 - Performance Gaz e**
   - Mise en œuvre du calendrier de détermination du gaz, des contadores par bloc et des harnais de référence avec télémétrie (latencia de verificacao, tamanhos de proof, rejeicoes de mempool).
   - Points de contrôle Endurecer CommitmentTree, charge LRU et indices de nullificateur pour les charges de travail multi-actifs.
4. **Phase M3 - Rotation et outillage du portefeuille**
   - Habiliter l'acitacao de proofs multi-parametro e multi-versao; prendre en charge les directives d'activation/dépréciation pour la gouvernance avec les runbooks de transition.
   - Entreposer les flux de migration SDK/CLI, les workflows d'analyse de l'auditeur et les outils de réconciliation des dépenses.
5. **Phase M4 - Audit des opérations électroniques**
   - Fornecer workflows de clés d'auditeur, API de divulgation sélective et runbooks opérationnels.
   - Agenda de révision externe de cryptographie/sécurité et de publication publié sous `status.md`.

Chaque phase d'actualisation des jalons de la feuille de route et des tests associés pour assurer l'exécution déterminée de la blockchain rouge.
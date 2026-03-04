---
lang: fr
direction: ltr
source: docs/source/kaigi_privacy_design.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6b7ffca7e960376a2959357cd865d8dab5afa1dfcb959adbc688b6db60977c8f
source_last_modified: "2026-01-04T10:50:53.617088+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Kaigi Confidentialité et conception de relais

Ce document rend compte de l'évolution axée sur la confidentialité qui introduit l'absence de connaissance
des preuves de participation et des relais de style oignon sans sacrifier le déterminisme ou
auditabilité du grand livre.

# Aperçu

La conception s'étend sur trois niveaux :

- **Confidentialité de la liste** : masquez les identités des participants en chaîne tout en gardant les autorisations de l'hôte et la facturation cohérentes.
- **Opacité de l'utilisation** : permet aux hôtes d'enregistrer l'utilisation mesurée sans divulguer publiquement les détails de chaque segment.
- **Relais superposés** : acheminez les paquets de transport via des homologues multi-sauts afin que les observateurs du réseau ne puissent pas savoir quels participants communiquent.

Tous les ajouts restent Norito-first, exécutés sous ABI version 1 et doivent s'exécuter de manière déterministe sur du matériel hétérogène.

# Objectifs

1. Admettez/expulsez les participants à l’aide de preuves sans connaissance afin que le grand livre n’expose jamais les identifiants de compte bruts.
2. Maintenir de solides garanties comptables : chaque événement d'adhésion, de départ et d'utilisation doit toujours être rapproché de manière déterministe.
3. Fournissez des manifestes de relais facultatifs qui décrivent les routes d'oignon pour les canaux de contrôle/données et peuvent être audités en chaîne.
4. Gardez la solution de secours (liste entièrement transparente) opérationnelle pour les déploiements qui ne nécessitent pas de confidentialité.

# Résumé du modèle de menace

- **Adversaires :** observateurs de réseau (FAI), validateurs curieux, opérateurs de relais malveillants et hôtes semi-honnêtes.
- **Actifs protégés :** Identité du participant, calendrier de participation, détails d'utilisation/facturation par segment et métadonnées de routage réseau.
- **Hypothèses :** Les hôtes apprennent toujours le véritable ensemble de participants hors chaîne ; les pairs du grand livre vérifient les preuves de manière déterministe ; les relais superposés ne sont pas fiables mais leur débit est limité ; Les primitives HPKE et SNARK existent déjà dans la base de code.

# Modifications du modèle de données

Tous les types vivent dans `iroha_data_model::kaigi`.

```rust
/// Commitment to a participant identity (Poseidon hash of account + domain salt).
pub struct KaigiParticipantCommitment {
    pub commitment: FixedBinary<32>,
    pub alias_tag: Option<String>,
}

/// Nullifier unique to each join action, prevents double-use of proofs.
pub struct KaigiParticipantNullifier {
    pub digest: FixedBinary<32>,
    pub issued_at_ms: u64,
}

/// Relay path description used by clients to set up onion routing.
pub struct KaigiRelayManifest {
    pub hops: Vec<KaigiRelayHop>,
    pub expiry_ms: u64,
}

pub struct KaigiRelayHop {
    pub relay_id: AccountId,
    pub hpke_public_key: FixedBinary<32>,
    pub weight: u8,
}
```

`KaigiRecord` gagne les champs suivants :

- `roster_commitments: Vec<KaigiParticipantCommitment>` – remplace la liste `participants` exposée une fois le mode de confidentialité activé. Les déploiements classiques peuvent conserver les deux remplis pendant la migration.
- `nullifier_log: Vec<KaigiParticipantNullifier>` – strictement en ajout uniquement, limité par une fenêtre déroulante pour limiter les métadonnées.
- `room_policy: KaigiRoomPolicy` – sélectionne la position d'authentification du spectateur pour la session (les salles `Public` reflètent les relais en lecture seule ; les salles `Authenticated` nécessitent des tickets de spectateur avant de sortir et de transmettre des paquets).
- `relay_manifest: Option<KaigiRelayManifest>` – manifeste structuré codé avec Norito afin que les sauts, les clés HPKE et les poids restent canoniques sans cales JSON.
- Énumération `privacy_mode: KaigiPrivacyMode` (voir ci-dessous).

```rust
pub enum KaigiPrivacyMode {
    Transparent,
    ZkRosterV1,
}
```

`NewKaigi` reçoit les champs facultatifs correspondants afin que les hôtes puissent choisir la confidentialité au moment de la création.


- Les champs utilisent les assistants `#[norito(with = "...")]` pour appliquer le codage canonique (petit-boutiste pour les entiers, sauts triés par position).
- `KaigiRecord::from_new` vide les nouveaux vecteurs et copie tout manifeste de relais fourni.

# Modifications de la surface d'instruction

## Aide au démarrage rapide de la démo

Pour les démonstrations ad hoc et les tests d'interopérabilité, la CLI expose désormais
`iroha kaigi quickstart`. Il:- Réutilise la configuration CLI (domaine `wonderland` + compte) sauf remplacement via `--domain`/`--host`.
- Génère un nom d'appel basé sur l'horodatage lorsque `--call-name` est omis et soumet `CreateKaigi` au point de terminaison Torii actif.
- En option, rejoint automatiquement l'hôte (`--auto-join-host`) afin que les téléspectateurs puissent se connecter immédiatement.
- Émet un résumé JSON contenant l'URL Torii, les identifiants d'appel, la politique de confidentialité/de salle, une commande de jointure prête à copier et le chemin de spool que les testeurs doivent surveiller (par exemple, `storage/streaming/soranet_routes/exit-<relay-id>/kaigi-stream/*.norito`). Utilisez `--summary-out path/to/file.json` pour conserver le blob.

Cet assistant ne remplace **pas** le besoin d'un nœud `irohad --sora` en cours d'exécution : les routes de confidentialité, les fichiers spool et les manifestes de relais restent sauvegardés par un grand livre. Il réduit simplement le passe-partout lors de la création de salles temporaires pour des groupes externes.

### Script de démonstration en une seule commande

Pour un chemin encore plus rapide, il existe un script compagnon : `scripts/kaigi_demo.sh`.
Il effectue les opérations suivantes pour vous :

1. Signe le `defaults/nexus/genesis.json` fourni dans `target/kaigi-demo/genesis.nrt`.
2. Lance `irohad --sora` avec le bloc signé (journaux sous `target/kaigi-demo/irohad.log`) et attend que Torii expose `http://127.0.0.1:8080/status`.
3. Exécute `iroha kaigi quickstart --auto-join-host --summary-out target/kaigi-demo/kaigi_summary.json`.
4. Imprime le chemin d'accès au résumé JSON ainsi qu'au répertoire spool (`storage/streaming/soranet_routes/exit-<relay-id>/kaigi-stream/`) afin que vous puissiez le partager avec des testeurs externes.

Variables d'environnement :

- `TORII_URL` : remplace le point de terminaison Torii par interrogation (`http://127.0.0.1:8080` par défaut).
- `RUN_DIR` — remplace le répertoire de travail (par défaut `target/kaigi-demo`).

Arrêtez la démo en appuyant sur `Ctrl+C` ; l'interruption dans le script met automatiquement fin à `irohad`. Les fichiers spool et le résumé restent sur le disque afin que vous puissiez transmettre les artefacts une fois le processus terminé.

## `CreateKaigi`

- Valide `privacy_mode` par rapport aux autorisations de l'hôte.
- Si un `relay_manifest` est fourni, appliquez ≥3 sauts, des poids non nuls, la présence de la clé HPKE et l'unicité afin que les manifestes en chaîne restent auditables.
- Validez l'entrée `room_policy` des SDK/CLI (`public` vs `authenticated`) et propagez-la au provisionnement SoraNet afin que les caches de relais exposent les catégories GAR correctes (`stream.kaigi.public` vs `stream.kaigi.authenticated`). Les hôtes câblent cela via `iroha kaigi create --room-policy …`, le champ `roomPolicy` du SDK JS ou en définissant `room_policy` lorsque les clients Swift assemblent la charge utile Norito avant la soumission.
- Stocke les journaux d'engagement/annulation vides.

## `JoinKaigi`

Paramètres :

- `proof: ZkProof` (wrapper d'octets Norito) – Preuve Groth16 attestant que l'appelant connaît `(account_id, domain_salt)` dont le hachage Poséidon est égal au `commitment` fourni.
-`commitment: FixedBinary<32>`
-`nullifier: FixedBinary<32>`
- `relay_hint: Option<KaigiRelayHop>` – remplacement facultatif par participant pour le saut suivant.

Étapes d'exécution :

1. Si `record.privacy_mode == Transparent`, retour au comportement actuel.
2. Vérifiez la preuve Groth16 par rapport à l'entrée de registre du circuit `KAIGI_ROSTER_V1`.
3. Assurez-vous que `nullifier` n'apparaît pas dans `record.nullifier_log`.
4. Ajouter les entrées d'engagement/annulation ; si `relay_hint` est fourni, corrigez la vue du manifeste de relais pour ce participant (stockée uniquement dans l'état de session en mémoire, pas en chaîne).##`LeaveKaigi`

Le mode transparent correspond à la logique actuelle.

Le mode privé nécessite :

1. Preuve que l'appelant connaît un engagement en `record.roster_commitments`.
2. Mise à jour du nullificateur prouvant le congé à usage unique.
3. Supprimez les entrées d’engagement/annulation. L'audit préserve les pierres tombales pour les fenêtres de rétention fixes afin d'éviter les fuites structurelles.

##`RecordKaigiUsage`

Étend la charge utile avec :

- `usage_commitment: FixedBinary<32>` – engagement sur le tuple d'utilisation brute (durée, gaz, ID de segment).
- Preuve ZK facultative vérifiant le delta correspond aux journaux cryptés fournis hors grand livre.

Les hôtes peuvent toujours soumettre des totaux transparents ; le mode confidentialité rend uniquement le champ d’engagement obligatoire.

# Vérification et circuits

- `iroha_core::smartcontracts::isi::kaigi::privacy` effectue désormais une liste complète
  vérification par défaut. Il résout `zk.kaigi_roster_join_vk` (jointure) et
  `zk.kaigi_roster_leave_vk` (feuilles) de la configuration,
  recherche le `VerifyingKeyRef` correspondant dans WSV (en s'assurant que l'enregistrement est
  `Active`, les identifiants backend/circuit correspondent et les engagements s'alignent), frais
  comptabilité d'octets et envoi au backend ZK configuré.
- La fonctionnalité `kaigi_privacy_mocks` conserve le vérificateur de stub déterministe afin
  Les tests unitaires/d'intégration et les tâches CI contraintes peuvent s'exécuter sans backend Halo2.
  Les versions de production doivent garder la fonctionnalité désactivée pour appliquer de vraies preuves.
- La caisse émet une erreur de compilation si `kaigi_privacy_mocks` est activé sur un
  version non-test, non-`debug_assertions`, empêchant les versions binaires accidentelles
  de l'expédition avec le talon.
- Les opérateurs doivent (1) enregistrer le vérificateur de liste défini via la gouvernance, et
  (2) définir `zk.kaigi_roster_join_vk`, `zk.kaigi_roster_leave_vk` et
  `zk.kaigi_usage_vk` dans `iroha_config` afin que les hôtes puissent les résoudre au moment de l'exécution.
  Jusqu'à ce que les clés soient présentes, les connexions, les sorties et les appels d'utilisation échouent
  déterministe.
- `crates/kaigi_zk` fournit désormais des circuits Halo2 pour les jointures/départs et l'utilisation de la liste
  engagements aux côtés des compresseurs réutilisables (`commitment`, `nullifier`,
  `usage`). Les circuits de liste exposent la racine de Merkle (quatre petits-boutistes
  membres 64 bits) comme entrées publiques supplémentaires afin que l'hôte puisse vérifier la preuve
  par rapport à la racine de la liste stockée avant vérification. Les engagements d'utilisation sont
  appliqué par `KaigiUsageCommitmentCircuit`, qui lie `(durée, gaz,
  segment)` au hachage du grand livre.
- Entrées du circuit `Join` : `(commitment, nullifier, domain_salt)` et privées
  `(account_id)`. Les entrées publiques incluent `commitment`, `nullifier` et
  quatre membres de la racine Merkle pour l'arbre d'engagement de la liste (la liste
  reste hors chaîne, mais la racine est liée à la transcription).
- Déterminisme : nous fixons les paramètres de Poséidon, les versions de circuits et les index dans le
  registre. Tout changement fait passer `KaigiPrivacyMode` à `ZkRosterV2` avec correspondance
  tests/fichiers dorés.

# Superposition de routage d'oignons

## Inscription relais- Les relais s'auto-enregistrent en tant qu'entrées de métadonnées de domaine `kaigi_relay::<relay_id>`, y compris le matériel de clé HPKE et la classe de bande passante.
- L'instruction `RegisterKaigiRelay` conserve le descripteur dans les métadonnées du domaine, émet un résumé `KaigiRelayRegistered` (avec empreinte digitale HPKE et classe de bande passante) et peut être réinvoquée pour faire pivoter les clés de manière déterministe.
- La gouvernance gère les listes autorisées via les métadonnées de domaine (`kaigi_relay_allowlist`) et les mises à jour d'enregistrement/manifeste de relais imposent l'adhésion avant d'accepter de nouveaux chemins.

## Création de manifeste

- Les hôtes créent des chemins multi-sauts (longueur minimale 3) à partir des relais disponibles. Le manifeste code la séquence d’AccountIds et les clés publiques HPKE requises pour chiffrer l’enveloppe en couches.
- `relay_manifest` stocké sur la chaîne contient des descripteurs de sauts et leur expiration (`KaigiRelayManifest` codé en Norito) ; les clés éphémères réelles et les compensations par session sont échangées hors grand livre à l'aide de HPKE.

## Signalisation et médias

- L'échange SDP/ICE se poursuit via les métadonnées Kaigi mais cryptées par saut. Les validateurs ne voient que le texte chiffré HPKE ainsi que les index d’en-tête.
- Les paquets multimédia transitent par des relais utilisant QUIC avec des charges utiles scellées. Chaque saut déchiffre une couche pour connaître l'adresse du saut suivant ; le destinataire final reçoit le flux multimédia après avoir supprimé toutes les couches.

## Basculement

- Les clients surveillent l'état du relais via l'instruction `ReportKaigiRelayHealth`, qui conserve les commentaires signés dans les métadonnées du domaine (`kaigi_relay_feedback::<relay_id>`), diffuse `KaigiRelayHealthUpdated` et permet à la gouvernance/aux hôtes de raisonner sur la disponibilité actuelle. Lorsqu'un relais échoue, l'hôte émet un manifeste mis à jour et enregistre un événement `KaigiRelayManifestUpdated` (voir ci-dessous).
- Les hôtes appliquent les modifications manifestes dans le grand livre via l'instruction `SetKaigiRelayManifest`, qui remplace le chemin stocké ou l'efface entièrement. Clearing émet un résumé avec `hop_count = 0` afin que les opérateurs puissent observer la transition vers le routage direct.
- Métriques Prometheus (`kaigi_relay_registered_total`, `kaigi_relay_registration_bandwidth_class`, `kaigi_relay_manifest_updates_total`, `kaigi_relay_manifest_hop_count`, `kaigi_relay_health_reports_total`, `kaigi_relay_health_state`, `kaigi_relay_failover_total`, `kaigi_relay_failover_hop_count`) font désormais apparaître le taux de désabonnement des relais, l'état d'intégrité et la cadence de basculement pour les tableaux de bord des opérateurs.

# Événements

Étendre les variantes `DomainEvent` :

- `KaigiRosterSummary` – émis avec les décomptes anonymisés et la liste actuelle
  root chaque fois que la liste change (root est `None` en mode transparent).
- `KaigiRelayRegistered` – émis chaque fois qu'un enregistrement de relais est créé ou mis à jour.
- `KaigiRelayManifestUpdated` – émis lorsque le manifeste du relais change.
- `KaigiRelayHealthUpdated` – émis lorsque les hôtes soumettent un rapport d'état de relais via `ReportKaigiRelayHealth`.
- `KaigiUsageSummary` – émis après chaque segment d'utilisation, exposant uniquement les totaux agrégés.

Les événements sont sérialisés avec Norito, exposant uniquement les hachages et les décomptes d'engagement.L'outil CLI (`iroha kaigi …`) encapsule chaque ISI afin que les opérateurs puissent enregistrer des sessions,
soumettez des mises à jour de la liste, signalez l’état du relais et enregistrez l’utilisation sans effectuer de transactions manuelles.
Les manifestes de relais et les preuves de confidentialité sont chargés à partir de fichiers JSON/hex transmis
le chemin de soumission normal de la CLI, ce qui facilite le script du contrat
admission dans des environnements de préparation.

# Comptabilité du gaz

- Nouvelles constantes dans `crates/iroha_core/src/gas.rs` :
  - `BASE_KAIGI_JOIN_ZK`, `BASE_KAIGI_LEAVE_ZK` et `BASE_KAIGI_USAGE_ZK`
    calibré par rapport aux délais de vérification Halo2 (≈1,6 ms pour la liste
    rejoint/quitte, ≈1,2 ms pour une utilisation sur Apple M2 Ultra). Les suppléments continuent de
    échelle avec taille d’octet de preuve via `PER_KAIGI_PROOF_BYTE`.
- `RecordKaigiUsage` s'engage à payer des frais supplémentaires en fonction de la taille de l'engagement et de la vérification des preuves.
- Le harnais d'étalonnage réutilisera l'infrastructure des actifs confidentiels avec des graines fixes.

# Stratégie de test

- Tests unitaires vérifiant l'encodage/décodage Norito pour `KaigiParticipantCommitment`, `KaigiRelayManifest`.
- Golden tests pour la vue JSON assurant l'ordre canonique.
- Tests d'intégration pour faire tourner un mini-réseau avec (voir
  `crates/iroha_core/tests/kaigi_privacy.rs` pour la couverture actuelle) :
  - Cycles de jointure/sortie privés utilisant des preuves simulées (indicateur de fonctionnalité `kaigi_privacy_mocks`).
  - Relayer les mises à jour du manifeste propagées via des événements de métadonnées.
- Tests d'interface utilisateur Trybuild couvrant une mauvaise configuration de l'hôte (par exemple, manifeste de relais manquant en mode confidentialité).
- Lors de l'exécution de tests unitaires/d'intégration dans des environnements contraints (par exemple, le Codex
  sandbox), exportez `NORITO_SKIP_BINDINGS_SYNC=1` pour contourner la liaison Norito
  vérification de synchronisation appliquée par `crates/norito/build.rs`.

# Plan de migration

1. ✅ Expédiez les ajouts de modèles de données derrière les valeurs par défaut `KaigiPrivacyMode::Transparent`.
2. ✅ Vérification du double chemin filaire : la production désactive `kaigi_privacy_mocks`,
   résout `zk.kaigi_roster_vk` et exécute une véritable vérification de l'enveloppe ; les tests peuvent
   activez toujours la fonctionnalité pour les stubs déterministes.
3. ✅ Présentation de la caisse dédiée `kaigi_zk` Halo2, calibrée au gaz et câblée
   couverture d'intégration pour exécuter des preuves réelles de bout en bout (les simulations sont désormais uniquement des tests).
4. ⬜ Déprécier le vecteur transparent `participants` une fois que tous les consommateurs ont compris les engagements.

# Questions ouvertes

- Définir la stratégie de persistance de l'arbre Merkle : on-chain vs off-chain (penche actuelle : arbre hors chaîne avec engagements racine en chaîne). *(Suivi dans KPG-201.)*
- Déterminez si les manifestes de relais doivent prendre en charge les chemins multiples (chemins redondants simultanés). *(Suivi dans KPG-202.)*
- Clarifier la gouvernance des réputations de relais : avons-nous besoin de réductions ou simplement d'interdictions douces ? *(Suivi dans KPG-203.)*

Ces éléments doivent être résolus avant d’activer `KaigiPrivacyMode::ZkRosterV1` en production.
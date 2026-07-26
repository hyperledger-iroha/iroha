---
lang: fr
direction: ltr
source: docs/source/fraud_monitoring_system.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c8262bacbb15b83bd70c824990e4948832418b59f184bca353eee899e44f4d4
source_last_modified: "2026-01-03T18:07:57.676991+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Système de surveillance de la fraude

Ce document capture la conception de référence pour la capacité partagée de surveillance de la fraude qui accompagnera le grand livre de base. L’objectif est de fournir aux prestataires de services de paiement (PSP) des signaux de risque de haute qualité pour chaque transaction tout en gardant les décisions en matière de garde, de confidentialité et de politique sous le contrôle d’opérateurs désignés en dehors du moteur de règlement.

## Objectifs et critères de réussite
- Fournissez des évaluations du risque de fraude en temps réel (<120 ms 95p, <40 ms médiane) pour chaque paiement touchant le moteur de règlement.
- Préservez la confidentialité des utilisateurs en garantissant que le service central ne traite jamais d'informations personnellement identifiables (PII) et n'ingère que des identifiants pseudonymes et de la télémétrie comportementale.
- Prise en charge des environnements multi-PSP où chaque fournisseur conserve une autonomie opérationnelle mais peut interroger l'intelligence partagée.
- Adaptez-vous continuellement aux nouveaux modèles d'attaque via des modèles supervisés et non supervisés sans introduire de comportement de grand livre non déterministe.
- Fournir des traces de décision vérifiables aux régulateurs et aux examinateurs indépendants sans exposer les portefeuilles ou contreparties sensibles.

## Portée
- **Dans le champ d'application :** Notation des risques de transaction, analyses comportementales, corrélation entre PSP, alertes d'anomalies, hooks de gouvernance et API d'intégration PSP.
- **Hors champ d'application :** Application directe (reste la responsabilité du PSP), contrôle des sanctions (géré par les pipelines de conformité existants) et vérification de l'identité (la gestion des alias couvre cela).

## Exigences fonctionnelles
1. **API Transaction Scoring** : API synchrone que les PSP appellent avant de transmettre un paiement au moteur de règlement, renvoyant un score de risque, un verdict catégorique et des fonctionnalités de raisonnement.
2. **Ingestion d'événements** : flux de résultats de règlement, événements du cycle de vie du portefeuille, empreintes digitales des appareils et commentaires sur la fraude au niveau PSP pour un apprentissage continu.
3. **Gestion du cycle de vie des modèles** : modèles versionnés avec formation hors ligne, déploiement fantôme, déploiement par étapes et prise en charge de la restauration. Une heuristique de repli déterministe doit exister pour chaque fonctionnalité.
4. **Boucle de rétroaction** : les PSP doivent être en mesure de soumettre des cas de fraude confirmés, des faux positifs et des notes de correction. Le système aligne les commentaires sur les caractéristiques de risque et met à jour les analyses.
5. **Contrôles de confidentialité** : Toutes les données stockées et transmises doivent être basées sur des alias. Toute demande contenant des métadonnées d'identité brutes est rejetée et enregistrée.
6. **Rapports de gouvernance** : exportations planifiées de métriques agrégées (détections par PSP, typologies, latence de réponse) ainsi que des API d'enquête ad hoc pour les auditeurs autorisés.
7. **Résilience** : déploiement actif-actif sur au moins deux installations avec vidange et relecture automatiques de la file d'attente. Si le service est dégradé, les PSP se rabattent sur les règles locales sans bloquer le grand livre.## Exigences non fonctionnelles
- **Déterminisme et cohérence** : les scores de risque guident les décisions des PSP mais ne modifient pas l'exécution du grand livre. Les validations du grand livre restent déterministes entre les nœuds.
- **Évolutivité** : maintenez ≥10 000 évaluations de risques par seconde avec une mise à l'échelle horizontale et un partitionnement des messages géré par des identifiants de pseudo-portefeuille.
- **Observabilité** : exposez les métriques (`fraud.scoring_latency_ms`, `fraud.risk_score_distribution`, `fraud.api_error_rate`, `fraud.model_version_active`) et les journaux structurés pour chaque appel de notation.
- **Sécurité** : TLS mutuel entre les PSP et le service central, modules de sécurité matériels pour la signature des enveloppes de réponse, pistes d'audit inviolables.
- **Conformité** : conformez-vous aux exigences LAB/CFT, fournissez des périodes de conservation configurables et intégrez les flux de travail de préservation des preuves.

## Présentation de l'architecture
1. **Couche de passerelle API**
  - Reçoit les demandes de notation et de commentaires via des API HTTP/JSON authentifiées.
   - Effectue la validation du schéma à l'aide des codecs Norito et applique les limites de débit par identifiant PSP.

2. **Service d'agrégation de fonctionnalités**
   - Rejoint les demandes entrantes avec des agrégats historiques (vitesse, modèles géospatiaux, utilisation de l'appareil) stockés dans un magasin de fonctionnalités de séries chronologiques.
   - Prend en charge les fenêtres de fonctionnalités configurables (minutes, heures, jours) à l'aide de fonctions d'agrégation déterministe.

3. **Moteur de risque**
   - Exécute le pipeline de modèles actifs (ensemble d'arbres boostés par gradient, détecteurs d'anomalies, règles).
   - Inclut un ensemble de règles de repli déterministes pour garantir des réponses limitées lorsque les scores du modèle ne sont pas disponibles.
   - Émet des enveloppes `FraudAssessment` avec partition, groupe, fonctionnalités contributives et version du modèle.## Modèles de notation et heuristiques
- **Échelle et bandes de scores** : les scores de risque sont normalisés entre 0 et 1 000. Les bandes sont définies comme : `0–249` (faible), `250–549` (moyen), `550–749` (élevé), `750+` (critique). Les bandes correspondent aux actions recommandées pour les PSP (approbation automatique, intensification, file d'attente pour examen, refus automatique), mais l'application reste spécifique aux PSP.
- **Ensemble modèle** :
  - Les arbres de décision améliorés par gradient ingèrent des fonctionnalités structurées telles que le montant, la vitesse de l'alias/de l'appareil, la catégorie du commerçant, la force d'authentification, le niveau de confiance PSP et les fonctionnalités de graphique multi-portefeuilles.
  - Un détecteur d'anomalies basé sur un auto-encodeur fonctionne sur des vecteurs comportementaux à créneau temporel (cadence de dépense par alias, changement d'appareil, entropie temporelle). Les scores sont calibrés en fonction de l'activité récente des PSP pour limiter la dérive.
  - Les règles de politique déterministes s'exécutent en premier ; leurs sorties alimentent les modèles statistiques sous forme de fonctionnalités binaires/continues afin que l'ensemble puisse apprendre les interactions.
- **Heuristiques de repli** : lorsque l'exécution du modèle échoue, la couche déterministe produit toujours un score limité en agrégeant les pénalités des règles. Chaque règle apporte un poids configurable, additionné puis fixé sur une échelle de 0 à 1 000, garantissant ainsi une latence et une explicabilité dans le pire des cas.
- **Budget de latence** : objectifs de pipeline de notation <20 ms pour la passerelle API + validation, <30 ms pour l'agrégation de fonctionnalités (servies à partir de caches en mémoire avec écriture différée vers des magasins persistants) et <40 ms pour l'évaluation d'ensemble. Le repli déterministe revient dans un délai <10 ms si l'inférence ML dépasse son budget, garantissant que le P95 global reste inférieur à 120 ms.
 - **Budget de latence** : objectifs de pipeline de notation <20 ms pour la passerelle API + validation, <30 ms pour l'agrégation de fonctionnalités (servies à partir de caches en mémoire avec écriture différée vers des magasins persistants) et <40 ms pour l'évaluation d'ensemble. Le repli déterministe revient dans un délai <10 ms si l'inférence ML dépasse son budget, garantissant que le P95 global reste inférieur à 120 ms.## Conception du cache de fonctionnalités en mémoire
- **Disposition des fragments** : les magasins de fonctionnalités sont fragmentés par hachage d'alias 64 bits en fragments `N = 256`. Chaque fragment possède :
  - Un tampon en anneau sans verrouillage pour les deltas de transactions récents (fenêtres de 5 min + 1 h) stockés sous forme de structures de tableaux afin de maximiser la localité de la ligne de cache.
  - Un arbre Fenwick compressé (seaux de 16 bits remplis de bits) pour maintenir les agrégats 24 heures sur 24, 7 jours sur 7 sans recalcul complet.
  - Une carte de hachage à la marelle cartographiant les contreparties → statistiques glissantes (compte, somme, variance, dernier horodatage) plafonnées à 1 024 entrées par alias.
- **Résidence mémoire** : les fragments chauds restent dans la RAM. Pour un univers d'alias de 50 M avec 1 % d'actifs au cours de la dernière heure, la résidence du cache est d'environ 500 000 alias. Avec environ 320 B par alias de métadonnées actives, l'ensemble de travail est d'environ 160 Mo, ce qui est suffisamment petit pour le cache L3 sur les serveurs modernes.
- **Concurrence** : les lecteurs empruntent des références immuables via une récupération basée sur l'époque ; les rédacteurs ajoutent des deltas et mettent à jour les agrégats à l’aide de la comparaison et de l’échange. Cela évite les conflits de mutex et maintient des chemins chauds vers deux opérations atomiques + une poursuite de pointeur délimitée.
- **Prélecture** : l'agent de notation publie le manuel `prefetch_read`, qui indique la prochaine partition d'alias une fois la validation de la demande terminée, masquant la latence de la mémoire principale (~ 80 ns) derrière l'agrégation des fonctionnalités.
- **Write-behind Log** : un WAL par fragment regroupe les deltas toutes les 50 ms (ou 4 Ko) et est vidé vers le magasin durable. Les points de contrôle sont exécutés toutes les 5 minutes pour maintenir les limites de récupération serrées.

### Répartition théorique de la latence (serveur Intel Ice Lake-class, 3,1 GHz)
- **Recherche de fragments + prélecture** : 1 échec de cache (~80 ns) plus calcul de hachage (<10 ns).
- **Itération du tampon en anneau (32 entrées)** : 32 × 2 charges = 64 charges ; avec 32 lignes de cache B et un accès séquentiel, cela reste en L1 → ~ 20 ns.
- **Mises à jour Fenwick (log₂ 2048 ≈ 11 étapes)** : 11 sauts de pointeur ; en supposant que la moitié de L1, la moitié de L2 frappent → ~ 30 ns.
- **Sonde cartographique Hop-scotch (facteur de charge 0,75, 2 sondes)** : 2 lignes de cache, ~2 × 15 ns.
- **Assemblage de fonctionnalités du modèle** : 150 opérations scalaires (<0,1 ns chacune) → ~15 ns.La somme de ces éléments donne environ 160 ns de calcul et environ 120 ns de blocage de mémoire par requête (~ 0,28 µs). Avec quatre processeurs d'agrégation simultanés par cœur, l'étape respecte facilement le budget de 30 ms, même en cas de charge en rafale ; le déploiement réel doit enregistrer des histogrammes à valider (via `fraud.feature_cache_lookup_ms`).
- **Fonctionnalités Windows et agrégation** :
  - Les fenêtres à court terme (5 min, 1 h) et à long terme (24 h, 7 jours) suivent la vitesse des dépenses, la réutilisation des appareils et les degrés des graphiques d'alias.
  - Les fonctionnalités graphiques (par exemple, appareils partagés entre alias, déploiement soudain, nouvelles contreparties dans des clusters à haut risque) s'appuient sur des résumés régulièrement compactés afin que les requêtes restent inférieures à la milliseconde.
  - Les heuristiques de localisation comparent les géobuckets grossiers au comportement historique, en signalant les sauts improbables (par exemple, plusieurs emplacements distants en quelques minutes) à l'aide d'un incrément de risque plafonné basé sur Haversine.
  - Les détecteurs de forme de flux maintiennent des histogrammes roulants des montants entrants/sortants et des contreparties pour repérer les signatures de mélange/tumbling (fan-in rapide suivi d'un fan-out similaire, séquences de sauts cycliques, intermédiaires de courte durée).
- **Catalogue de règles (non exhaustif)** :
  - **Violation de vitesse** : série rapide de transferts de grande valeur dépassant les seuils par alias ou par appareil.
  - **Anomalie du graphique d'alias** : Alias ​​interagit avec un cluster lié à des cas de fraude confirmés ou à des modèles de mules connus.
  - **Réutilisation de l'appareil** : empreinte digitale de l'appareil partagée entre des alias appartenant à différentes cohortes d'utilisateurs PSP sans liaison préalable.
  - **Première valeur élevée** : nouvel alias tentant des montants supérieurs au couloir d'intégration typique de la PSP.
  - **Rétrogradation de l'authentification** : la transaction utilise des facteurs plus faibles que la référence du compte (par exemple, passage de la biométrie au code PIN) sans justification déclarée par la PSP.
  - **Modèle de mélange/tumbling** : Alias ​​participe à des chaînes de fan-in/fan-out élevées avec une synchronisation étroitement couplée, des quantités aller-retour répétitives ou des flux circulaires à travers plusieurs alias dans des fenêtres courtes. La règle augmente le score à l'aide de pics de centralité graphique et de détecteurs de forme de flux ; les cas graves se fixent sur la bande `high` avant même la sortie ML.
  - ** Liste noire des transactions atteintes ** : l'alias ou la contrepartie apparaît sur le flux de liste noire partagé organisé via un vote de gouvernance en chaîne ou une autorité déléguée avec des contrôles `sudo` (par exemple, ordonnances réglementaires, fraude confirmée). Le score se fixe sur la bande `critical` et émet le code de raison `BLACKLIST_MATCH` ; Les PSP doivent enregistrer les remplacements pour audit.
  - **Incompatibilité de signature Sandbox** : PSP soumet une évaluation générée avec une signature de modèle obsolète ; le score passe à `critical` et le hook d'audit se déclenche.
- **Codes de motif** : chaque évaluation comprend des codes de motif lisibles par machine, classés par poids de contribution (par exemple, `VELOCITY_BREACH`, `NEW_DEVICE`, `GRAPH_HIGH_RISK`, `AUTH_DOWNGRADE`). Les PSP peuvent les présenter aux opérateurs ou aux portefeuilles pour la messagerie des utilisateurs.- **Gouvernance des modèles** : l'étalonnage et la définition des seuils suivent des playbooks documentés : les courbes ROC/PR revues trimestriellement, les back-tests contre la fraude étiquetée et les modèles challenger fonctionnent dans l'ombre jusqu'à ce qu'ils soient stables. Toute mise à jour de seuil nécessite une double approbation (opérations frauduleuses + risque indépendant).

## Flux de liste noire provenant de la gouvernance
- **Création en chaîne** : les entrées de la liste noire sont introduites via le sous-système de gouvernance (`iroha_core::smartcontracts::isi::governance`) en tant qu'ISI `BlacklistProposal` qui répertorie les alias, les identifiants PSP ou les empreintes digitales des appareils à bloquer. Les parties prenantes votent en utilisant le système de vote standard ; une fois le quorum atteint, la chaîne émet un enregistrement `GovernanceEvent::BlacklistUpdated` contenant les ajouts/suppressions approuvés plus un `blacklist_epoch` croissant de manière monotone.
- **Chemin sudo délégué** : les actions d'urgence peuvent être exécutées via l'instruction `sudo::Execute`, qui émet le même événement `BlacklistUpdated` mais marque le changement comme `origin = Sudo`. Cela reflète l'historique en chaîne avec une provenance explicite afin que les auditeurs puissent distinguer les votes par consensus des interventions déléguées.
- **Canal de distribution** : le service de pont FMS s'abonne au flux `LedgerEvent` (codé en Norito) et surveille les événements `BlacklistUpdated`. Chaque événement est validé par rapport à la preuve de gouvernance Merkle et vérifié avec la signature de bloc avant d'être appliqué. Les événements sont idempotents ; le FMS maintient le dernier `blacklist_epoch` pour éviter les rediffusions.
- **Application dans FMS** : une fois qu'une mise à jour est acceptée, les entrées sont écrites dans le magasin de règles déterministes (soutenues par un stockage en ajout uniquement avec des journaux d'audit). Le moteur de notation recharge à chaud la liste noire dans les 30 secondes, garantissant que les évaluations ultérieures déclenchent la règle `BLACKLIST_MATCH` et se fixent sur `critical`.
- **Audit et restauration** : la gouvernance peut voter pour supprimer des entrées via le même pipeline. Le FMS conserve des instantanés historiques marqués par `blacklist_epoch` afin que les opérateurs puissent répondre à des questions médico-légales ou rejouer des décisions passées au cours des enquêtes.

4. **Plateforme d'apprentissage et d'analyse**
   - Reçoit les événements de fraude confirmés, les résultats des règlements et les commentaires des PSP via un grand livre en annexe uniquement (par exemple, Kafka + stockage d'objets).
   - Fournit des blocs-notes/tâches hors ligne permettant aux data scientists de recycler les modèles. Les artefacts du modèle sont versionnés et signés avant la promotion.

5. **Portail de gouvernance**
   - Interface restreinte permettant aux auditeurs d'examiner les tendances, de rechercher des évaluations historiques et d'exporter des rapports d'incident.
   - Met en œuvre des contrôles de politique afin que les enquêteurs ne puissent pas accéder aux informations personnelles sans la coopération du PSP.

6. **Adaptateurs d'intégration**
   - SDK légers pour PSP (Rust, Kotlin, Swift, TypeScript) implémentant les requêtes/réponses Norito et la mise en cache locale.
   - Hook du moteur de règlement (au sein de `iroha_core`) qui enregistre les références d'évaluation des risques lorsque les PSP transmettent les transactions après vérification.## Flux de données
1. PSP s'authentifie auprès de la passerelle API et soumet un `RiskQuery` contenant :
   - Identifiants d'alias pour le payeur/bénéficiaire, identifiant d'appareil haché, montant de la transaction, catégorie, compartiment grossier de géolocalisation, indicateurs de confiance PSP et métadonnées de session récente.
2. La passerelle valide la charge utile, l'enrichit avec les métadonnées PSP (niveau de licence, SLA) et les files d'attente pour l'agrégation des fonctionnalités.
3. Le service de fonctionnalités extrait les derniers agrégats, construit le vecteur de modèle et l'envoie au moteur de risque.
4. Le moteur de risque évalue la demande, attache des codes de raison déterministes, signe le `FraudAssessment` et le renvoie au PSP.
5. PSP combine l'évaluation avec ses politiques locales pour approuver, refuser ou authentifier la transaction.
6. Le résultat (approuvé/refusé, fraude confirmée/faux positif) est transmis de manière asynchrone à la plateforme d'apprentissage pour une amélioration continue.
7. Les processus par lots quotidiens regroupent les mesures pour les rapports de gouvernance et envoient des alertes politiques (par exemple, augmentation des cas d'ingénierie sociale) aux tableaux de bord des PSP.

## Intégration avec les composants Iroha
- **Core Host Hooks** : l'admission des transactions applique désormais les métadonnées `fraud_assessment_band` chaque fois que `fraud_monitoring.enabled` et `required_minimum_band` sont définies. L'hôte rejette les transactions manquant le champ ou portant une bande inférieure au minimum configuré, et émet un avertissement déterministe lorsque `missing_assessment_grace_secs` est différent de zéro (fenêtre de grâce dont la suppression est prévue au jalon FM-204 une fois que le vérificateur distant est câblé). Les évaluations doivent également inclure `fraud_assessment_score_bps` ; l'hôte vérifie le score par rapport à la bande déclarée (0-249 ➜ faible, 250-549 ➜ moyen, 550-749 ➜ élevé, 750+ ➜ critique, avec des valeurs en points de base prises en charge jusqu'à 10 000). Lorsque `fraud_monitoring.attesters` est configuré, les transactions doivent attacher un Norito codé en `fraud_assessment_envelope` (base64) et un `fraud_assessment_digest` correspondant (hex). Le démon décode l'enveloppe de manière déterministe, vérifie la signature Ed25519 par rapport au registre des attestateurs, recalcule le résumé sur la charge utile non signée et rejette les disparités afin que seules les évaluations attestées parviennent à un consensus.
- **Configuration** : ajoutez des entrées de configuration sous `iroha_config::fraud_monitoring` pour les points de terminaison du service de risque, les délais d'attente et les bandes d'évaluation requises. Les valeurs par défaut désactivent l’application pour le développement local.| Clé | Tapez | Par défaut | Remarques |
  | --- | --- | --- | --- |
  | `enabled` | booléen | `false` | Interrupteur principal pour les contrôles d'admission ; sans `required_minimum_band`, l'hôte enregistre un avertissement et ignore l'application. |
  | `service_endpoints` | tableau | `[]` | Liste ordonnée des URL de base de services de fraude. Les doublons sont supprimés de manière déterministe ; réservé au prochain vérificateur. |
  | `connect_timeout_ms` | durée | `500` | Millisecondes avant l'abandon des tentatives de connexion ; les valeurs nulles reviennent à la valeur par défaut. |
  | `request_timeout_ms` | durée | `1500` | Millisecondes d'attente d'une réponse du service des risques. |
  | `missing_assessment_grace_secs` | durée | `0` | Fenêtre de grâce permettant les évaluations manquantes ; les valeurs non nulles déclenchent une solution de secours déterministe qui enregistre et autorise la transaction. |
  | `required_minimum_band` | énumération (`low`, `medium`, `high`, `critical`) | `null` | Lorsqu'elles sont définies, les transactions doivent être associées à une évaluation égale ou supérieure à cette bande de gravité ; les valeurs inférieures sont rejetées. Définissez sur `null` pour désactiver le déclenchement même si `enabled` est vrai. |
  | `attesters` | array | `[]` | Registre facultatif des moteurs d’attestation. Une fois remplies, les enveloppes doivent être signées par l’une des clés répertoriées et inclure un résumé correspondant. |

- **Validation** : les tests unitaires dans `crates/iroha_core/tests/fraud_monitoring.rs` couvrent les chemins désactivés, manquants et à bande insuffisante ; `integration_tests::fraud_monitoring_requires_assessment_bands` exerce le flux d'évaluation simulée de bout en bout.

- **Télémétrie** : `iroha_telemetry` exporte les collecteurs destinés à la PSP capturant les décomptes d'évaluation (`fraud_psp_assessments_total{tenant,band,lane,subnet}`), les métadonnées manquantes (`fraud_psp_missing_assessment_total{tenant,lane,subnet,cause}`), les histogrammes de latence (`fraud_psp_latency_ms{tenant,lane,subnet}`), les distributions de scores (`fraud_psp_score_bps{tenant,band,lane,subnet}`), les charges utiles non valides. (`fraud_psp_invalid_metadata_total{tenant,field,lane,subnet}`), les résultats de l'attestation (`fraud_psp_attestation_total{tenant,engine,lane,subnet,status}`) et les inadéquations des résultats (`fraud_psp_outcome_mismatch_total{tenant,direction,lane,subnet}`). Les clés de métadonnées attendues pour chaque transaction sont `fraud_assessment_band`, `fraud_assessment_tenant`, `fraud_assessment_score_bps`, `fraud_assessment_latency_ms`, la paire enveloppe/digest de l'attestation (`fraud_assessment_envelope`, `fraud_assessment_digest`) et la version post-incident. Indicateur `fraud_assessment_disposition` (valeurs : `approved`, `declined`, `manual_review`, `confirmed_fraud`, `false_positive`, `chargeback`, `loss`).
- **Schéma Norito** : définissez les types Norito pour `RiskQuery`, `FraudAssessment` et les rapports de gouvernance. Fournissez des tests aller-retour pour garantir la stabilité du codec.

## Confidentialité et minimisation des données
- Les alias, les identifiants d'appareil hachés et les compartiments de géolocalisation grossière forment l'intégralité du plan de données partagé avec le service central.
- Les PSP conservent la cartographie des alias vers les identités réelles ; aucune cartographie de ce type ne quitte leur périmètre.
- Les modèles de risque fonctionnent uniquement sur des signaux comportementaux pseudonymes ainsi que sur le contexte soumis par le PSP (catégorie de marchand, canal, niveau d'authentification).
- Les exportations d'audit sont agrégées (par exemple, nombres par PSP et par jour). Toute exploration nécessite un double contrôle et une désanonymisation côté PSP.## Opérations et déploiement
- Déployer la plateforme de notation en tant que sous-système dédié géré par un opérateur désigné distinct des opérateurs de nœuds de la banque centrale.
- Fournir des environnements bleu/vert : `fraud-scoring-prod`, `fraud-scoring-shadow`, `fraud-lab`.
- Implémenter des contrôles de santé automatisés (latence de l'API, retard des messages, réussite du chargement du modèle). Si les vérifications de santé échouent, les SDK PSP passent automatiquement en mode local uniquement et avertissent les opérateurs.
- Maintenir les buckets de rétention : stockage à chaud (30 jours en magasin de fonctionnalités), stockage à chaud (1 an en stockage objet), archive à froid (5 ans compressés).

## Collecteurs et tableaux de bord de télémétrie

### Collecteurs requis

- **Prometheus scrape** : activez `/metrics` sur chaque validateur exécutant le profil d'intégration PSP afin que les séries `fraud_psp_*` soient exportées. Les étiquettes par défaut incluent les ID d'espace réservé `subnet="global"` et `lane` afin que les tableaux de bord puissent pivoter une fois le routage multi-sous-réseau expédié.
- **Totaux des évaluations** : `fraud_psp_assessments_total{tenant,band}` compte les évaluations acceptées par tranche de gravité ; des alertes se déclenchent si un locataire arrête de signaler pendant 5 minutes.
- **Métadonnées manquantes** : `fraud_psp_missing_assessment_total{tenant,cause}` distingue les rejets définitifs (`cause="missing"`) des allocations de fenêtre de grâce (`cause="grace"`). Transactions de porte qui tombent à plusieurs reprises dans la catégorie de grâce.
- **Histogramme de latence** : `fraud_psp_latency_ms_bucket` retrace la latence de notation signalée par la PSP. Ciblez  20 % de la moyenne des 30 derniers jours.
- **Métadonnées non valides** : `fraud_psp_invalid_metadata_total{field}` signale les régressions de charge utile PSP (par exemple, ID de locataire manquants, dispositions mal formées) afin que les mises à jour du SDK puissent être déployées rapidement.
- **Statut de l'attestation** : `fraud_psp_attestation_total{tenant,engine,status}` confirme que les enveloppes sont signées et que les résumés correspondent. Alerte si `status!="verified"` augmente pour un locataire ou un moteur.

### Couverture du tableau de bord

- **Aperçu exécutif** : graphique en zones empilées de `fraud_psp_assessments_total` par bande et par locataire, associé à un tableau résumant la latence P95 et les nombres d'inadéquations.
- **Opérations** : panneaux d'histogramme pour `fraud_psp_latency_ms` et `fraud_psp_score_bps` avec comparaison semaine après semaine, plus compteurs à statistique unique pour `fraud_psp_missing_assessment_total` divisés par `cause`.
- **Surveillance des risques** : graphique à barres de `fraud_psp_outcome_mismatch_total` par locataire, tableau détaillé répertoriant les cas `fraud_assessment_disposition=confirmed_fraud` récents où `band` était `low` ou `medium`.
- **Règles d'alerte** :
  - `rate(fraud_psp_missing_assessment_total{cause="missing"}[5m]) > 0` → alerte de recherche de personne (admission rejetant le trafic PSP).
  - `histogram_quantile(0.95, sum(rate(fraud_psp_latency_ms_bucket[10m])) by (le,tenant)) > 150` → rupture de SLO de latence.
  - `sum by (tenant) (rate(fraud_psp_outcome_mismatch_total{direction="missed_fraud"}[1h])) > 0.01` → dérive du modèle / écart politique.

### Attentes en matière de basculement- Les SDK PSP doivent maintenir deux points de terminaison de notation actifs et basculer dans les 15 secondes suivant la détection d'erreurs de transport ou de pics de latence > 200 ms. Le grand livre tolère le trafic de grâce pendant au plus `fraud_monitoring.missing_assessment_grace_secs` ; les opérateurs doivent maintenir le bouton à <= 30 secondes en production.
- Les validateurs enregistrent `fraud_psp_missing_assessment_total{cause="grace"}` en mode repli ; si un locataire reste en grâce pendant plus de 5 minutes, le PSP doit passer à l'examen manuel et ouvrir un incident Sev2 avec l'équipe des opérations de fraude partagée.
- Les déploiements actifs-actifs doivent démontrer une vidange/relecture de la file d'attente lors des exercices de reprise après sinistre. Les métriques de relecture doivent maintenir `fraud_psp_latency_ms` P99 sous 400 ms pour la fenêtre de relecture.

## Liste de contrôle pour le partage de données PSP

1. **Plomberie de télémétrie** : exposez les clés de métadonnées répertoriées ci-dessus pour chaque transaction transmise au grand livre ; les identifiants des locataires doivent être pseudonymes et limités au contrat PSP.
2. **Anonymisation** : confirmez que les hachages d'appareils, les identifiants d'alias et les dispositions sont pseudonymisés avant de quitter le périmètre PSP ; aucune PII ne peut être intégrée dans les métadonnées Norito.
3. **Rapports de latence** : remplissez `fraud_assessment_latency_ms` avec une synchronisation de bout en bout (passerelle vers PSP) afin que les régressions SLA apparaissent immédiatement.
4. **Rapprochement des résultats** : mettez à jour `fraud_assessment_disposition` une fois que les cas de fraude sont confirmés (par exemple, une rétrofacturation publiée) pour que les mesures d'inadéquation restent exactes.
5. **Exercices de basculement** : répétez chaque trimestre à l'aide de la liste de contrôle partagée : vérifiez le basculement automatique des points de terminaison, assurez la journalisation de la fenêtre de grâce et joignez des notes d'exercice à la tâche de suivi déposée par `scripts/ci/schedule_fraud_scoring.sh`.
6. **Validation du tableau de bord** : les équipes opérationnelles PSP doivent examiner les tableaux de bord Prometheus après l'intégration et après chaque exercice de l'équipe rouge pour confirmer que les métriques correspondent aux étiquettes de locataire attendues.

## Considérations de sécurité
- Toutes les réponses sont signées avec des clés matérielles ; Les PSP valident les signatures avant de faire confiance aux scores.
- Limite de débit par alias/appareil pour atténuer les attaques de sondage visant à connaître les limites du modèle.
- Intégrez un filigrane dans les évaluations pour retracer les réponses divulguées sans révéler publiquement l'identité du PSP.
- Organiser des exercices trimestriels de l'équipe rouge en coordination avec le groupe de travail sur la sécurité (jalon 0) et intégrer les résultats dans les mises à jour de la feuille de route.## Phases de mise en œuvre
1. **Phase 0 – Fondations**
   - Finaliser les schémas Norito, l'échafaudage du SDK PSP, le câblage de configuration et le talon de vérification côté grand livre.
   - Créer un moteur de règles déterministes couvrant les contrôles de risques obligatoires (vitesse, vitesse par paire d'alias, réutilisation des appareils).
2. **Phase 1 – MVP de la notation centrale**
   - Déployer un magasin de fonctionnalités, un service de notation et des tableaux de bord de télémétrie.
   - Intégrer la notation en temps réel avec une cohorte PSP limitée ; capturez les mesures de latence et de qualité.
3. **Phase 2 – Analyses avancées**
   - Introduire la détection des anomalies, l'analyse des liens basée sur des graphiques et des seuils adaptatifs.
   - Lancer le portail de gouvernance et les pipelines de reporting par lots.
4. **Phase 3 – Apprentissage continu et automatisation**
   - Automatisez les pipelines de formation/validation de modèles, ajoutez des déploiements Canary et étendez la couverture du SDK.
   - Alignez-vous sur les accords de partage de données entre juridictions et connectez-vous aux futurs ponts multi-sous-réseaux.

## Questions ouvertes
- Quel organisme de réglementation désignera l'opérateur du service anti-fraude, et comment les responsabilités de surveillance sont-elles partagées ?
- Comment les PSP exposent-ils les flux de défis des utilisateurs finaux tout en maintenant une UX cohérente entre les fournisseurs ?
- Quelles technologies améliorant la confidentialité (par exemple, enclaves sécurisées, agrégation homomorphe) devraient être prioritaires une fois que le service de base est stable ?
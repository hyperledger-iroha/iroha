---
lang: fr
direction: ltr
source: docs/portal/docs/da/threat-model.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note مستند ماخذ
ہونے تک دونوں ورژنز کو sync رکھیں۔
:::

# Modèle de menace de disponibilité des données Sora Nexus

_آخری جائزہ: 2026-01-19 -- اگلا شیڈول شدہ جائزہ: 2026-04-19_

Cadence de maintenance : Groupe de travail sur la disponibilité des données (<=90 jours). ہر ریویژن
`status.md` vous permet d'obtenir des tickets d'atténuation active et
artefacts de simulation

## مقصد اور دائرہ کار

Disponibilité des données (DA) pour les diffusions Taikai, les blobs de voie Nexus, pour la gouvernance
artefacts byzantins, نیٹ ورک، اور آپریٹر کی خرابیوں میں بھی قابل بازیافت
رکھتا ہے۔ Modèle de menace DA-1 (architecture et modèle de menace)
Il s'agit de tâches DA en aval (DA-2 et DA-10) de base de référence

Composants concernés :
- Extension d'ingestion Torii DA et rédacteurs de métadonnées Norito.
- Arborescences de stockage blob sauvegardées par SoraFS (niveaux chaud/froid) et politiques de réplication.
- Engagements de bloc Nexus (formats wire, preuves, API light-client).
- Hooks d'application PDP/PoTR et charges utiles DA pour les utilisateurs.
- Workflows des opérateurs (épinglage, expulsion, slashing) et pipelines d'observabilité.
- Approbations de gouvernance et opérateurs DA et contenu et admettre ou expulserHors de portée de ce document :
- Modélisation économique de premier plan (flux de travail DA-7 ci-dessus).
- Protocoles de base SoraFS et modèle de menace SoraFS.
- Ergonomie du SDK client au-delà des considérations liées à la surface des menaces.

## Aperçu architectural1. **Soumission :** Les blobs clients et l'API d'ingestion Torii DA sont soumis pour soumission.
   Les blobs de nœuds et les morceaux sont encodés par les manifestes Norito (type de blob,
   voie, époque, drapeaux de codec) ainsi que des morceaux et un niveau SoraFS chaud pour tous les utilisateurs.
2. **Publicité :** Intentions d'épinglage et registre des conseils de réplication (marché SoraFS)
   Les fournisseurs de stockage et les balises de stratégie ainsi que la rétention chaud/froid
   cibles بتاتے ہیں۔
3. **Engagement :** Engagements blob des séquenceurs Nexus (CID + racines KZG facultatives)
   bloc canonique میں شامل کرتے ہیں۔ Hachage d'engagement des clients légers اور annoncé
   métadonnées pour vérifier la disponibilité et vérifier la disponibilité
4. **Réplication :** Les nœuds de stockage se voient attribuer des partages/morceaux par PDP/PoTR
   défis en matière de politiques et de niveaux chauds/froids pour les données
   promouvoir کرتے ہیں۔
5. **Récupérer :** Consommateurs SoraFS pour les passerelles compatibles DA et pour la récupération de données.
   les preuves vérifient les répliques les demandes de réparation les demandes de réparation
6. **Gouvernance :** Parlement et opérateurs des comités de surveillance du DA, barèmes des loyers,
   اور escalades d'application et approbation کرتی ہے۔ Artefacts de gouvernance اسی DA
   chemin سے گزرتے ہیں تاکہ transparence رہے۔

## Actifs et propriétairesÉchelle d'impact : **Critique** sécurité/activité du grand livre **Remblai DA élevé**
یا clients کو بلاک کرتا ہے؛ **Qualité modérée** et récupérable
**Faible** محدود اثر۔

| Actif | Descriptif | Intégrité | Disponibilité | Confidentialité | Propriétaire |
| --- | --- | --- | --- | --- | --- |
| Blobs DA (morceaux + manifestes) | Taikai, voie, blobs de gouvernance dans SoraFS | Critique | Critique | Modéré | DA WG / Equipe Stockage |
| Norito Manifestes DA | Métadonnées typées décrivant les blobs | Critique | Élevé | Modéré | GT sur le protocole principal |
| Bloquer les engagements | CID + racines KZG à l'intérieur des blocs Nexus | Critique | Élevé | Faible | GT sur le protocole principal |
| Calendriers PDP/PoTR | Cadence d'application pour les répliques DA | Élevé | Élevé | Faible | Équipe de stockage |
| Registre des opérateurs | Fournisseurs et politiques de stockage approuvés | Élevé | Élevé | Faible | Conseil de gouvernance |
| Registres des loyers et des incitations | Entrées de grand livre pour le loyer et les pénalités DA | Élevé | Modéré | Faible | GT Trésorerie |
| Tableaux de bord d'observabilité | SLO DA, profondeur de réplication, alertes | Modéré | Élevé | Faible | SRE / Observabilité |
| Intentions de réparation | Demandes de réhydratation des morceaux manquants | Modéré | Modéré | Faible | Équipe de stockage |

## Adversaires et capacités| Acteur | Capacités | Motivations | Remarques |
| --- | --- | --- | --- |
| Client malveillant | Les blobs mal formés soumettent des manifestes périmés, une relecture et une ingestion de DoS. | Les émissions de Taikai perturbent l'injection de données invalides | Clés privilégiées |
| Nœud de stockage byzantin | Les répliques attribuées sont abandonnées et les preuves PDP/PoTR forgent une connivence. | DA rétention کم کرنا، rent سے بچنا، data otage بنانا۔ | Identifiants d'opérateur valides رکھتا ہے۔ |
| Séquenceur compromis | Les engagements omettent les blocs ou équivoquent la réorganisation des métadonnées. | Les soumissions du DA sont incohérentes | Majorité de consensus سے محدود۔ |
| Opérateur initié | Abus d'accès à la gouvernance et politiques de rétention falsifiées et fuite d'informations d'identification | Gain économique, sabotage | Infrarouge de niveau chaud/froid تک رسائی۔ |
| Adversaire du réseau | Partition des nœuds avec délai de réplication et injection de trafic MITM | La disponibilité des SLO se dégrade | TLS est un moyen de créer des liens lents/drops en temps réel |
| Attaquant d'observabilité | Les tableaux de bord/alertes falsifient les incidents et suppriment les incidents. | Pannes DA en général | Pipeline de télémétrie تک رسائی درکار۔ |

## Limites de confiance- ** Limite d'entrée : ** Client سے Extension Torii DA۔ Limitation du débit d'authentification au niveau de la requête
  Validation de la charge utile ici
- ** Limite de réplication : ** Morceaux de nœuds de stockage et échange de preuves Nœuds
  باہمی authentifié ہیں مگر Byzantin برتاؤ ممکن ہے۔
- ** Limite du grand livre : ** Données de bloc validées pour le stockage hors chaîne ۔ Consensus
  garde d'intégrité et disponibilité et application hors chaîne
- ** Limite de gouvernance : ** Conseil/Parlement et opérateurs, budgets, réductions
  approuver کرتے ہیں۔ Déploiement DA et déploiement de services
- ** Limite d'observabilité : ** Collecte de métriques/journaux et outils de tableaux de bord/alertes
  کو exporter ہونا۔ Pannes de falsification et attaques

## Scénarios de menaces et contrôles

### Attaques de chemin d'ingestion

**Scénario :** Charges utiles Norito mal formées par un client malveillant et soumission de blobs surdimensionnés
کرتا ہے تاکہ ressources épuisées ہوں یا métadonnées invalides شامل ہو۔**Contrôles**
- Validation du schéma Norito avec négociation stricte de version ; les drapeaux inconnus sont rejetés۔
- Torii point de terminaison d'acquisition avec limitation du débit et authentification
- SoraFS chunker avec limites de taille de morceau et codage déterministe
- Pipeline d'admission et correspondance de la somme de contrôle d'intégrité et les manifestes persistent
- Cache de relecture déterministe (`ReplayCache`) Piste Windows `(lane, epoch, sequence)`
  Il y a des marques de hautes eaux sur le disque qui persistent et il y a des doublons/des rediffusions obsolètes
  rejeter کرتا ہے؛ propriété اور fuzz exploite des empreintes digitales divergentes اور hors service
  les soumissions couvrent کرتے ہیں۔ [crates/iroha_core/src/da/replay_cache.rs:1]
  [fuzz/da_replay_cache.rs:1] [crates/iroha_torii/src/da/ingest.rs:1]

**Écarts résiduels**
- Torii ingérer et rejouer l'admission au cache et les threads et les curseurs de séquence
  redémarre کے پار persist کرنا ضروری ہے۔
- Schémas DA Norito pour un faisceau de fuzz dédié (`fuzz/da_ingest_schema.rs`)
  ہے؛ tableaux de bord de couverture et de régression et d'alerte et d'alerte

### Retenue de réplication

**Scénario :** Attributions de broches aux opérateurs de stockage byzantins pour des morceaux





laisser tomber les réponses forgées, la collusion, les défis PDP/PoTR, passer les réponses**Contrôles**
- Les charges utiles DA du calendrier de défi PDP/PoTR étendent la couverture par époque.
- Réplication multi-source avec seuils de quorum؛ récupérer les fragments manquants de l'orchestrateur
  détecter le déclencheur de réparation
- La gouvernance réduit les preuves échouées et les répliques manquantes liées ہے۔
- Travail de rapprochement automatisé (`cargo xtask da-commitment-reconcile`) pour ingérer les reçus
  Les engagements du DA (SignedBlockWire/`.norito`/JSON) et comparer les engagements avec la gouvernance
  Le paquet de preuves JSON émet des tickets manquants/incohérents en cas d'échec
  Voir Alertmanager et la page correspondante.

**Écarts résiduels**
- Harnais de simulation `integration_tests/src/da/pdp_potr.rs` (tests :
  `integration_tests/tests/da/pdp_potr_simulation.rs`) collusion et partition
  scénarios چلاتا ہے؛ DA-5 est doté de surfaces d'épreuve entièrement recouvertes de surfaces de preuve
- Politique d'expulsion de niveau froid avec piste d'audit signée et gouttes secrètes
  روکے جا سکیں۔

### Falsification des engagements

**Scénario :** Les engagements DA du séquenceur compromis sont omis et modifiés.
récupérer les échecs et les incohérences du client léger**Contrôles**
- Propositions de bloc de consensus et files d'attente de soumission DA et vérification croisée pairs
  engagements manquants et propositions rejetées
- Les preuves d'inclusion des clients légers vérifient les poignées de récupération
- Récépissés de soumission et engagements de bloc et piste d'audit
- Travail de rapprochement automatisé (`cargo xtask da-commitment-reconcile`) pour ingérer les reçus
  Les engagements et la comparaison avec la gouvernance et le paquet de preuves JSON émettent entre eux.
  اور tickets manquants/incohérents پر Page Alertmanager ہوتا ہے۔

**Écarts résiduels**
- Travail de réconciliation + crochet Alertmanager سے couverture؛ paquets de gouvernance par défaut میں
  Ingestion du paquet de preuves JSON

### Partition du réseau et censure

**Scénario :** Partition réseau de réplication adverse avec plusieurs nœuds attribués
chunks حاصل نہیں کر پاتے یا PDP/PoTR challenges کا جواب نہیں دے پاتے۔

**Contrôles**
- Exigences des fournisseurs multirégionaux sur divers chemins de réseau
- Défiez la gigue de Windows et le repli du canal de réparation hors bande.
- Profondeur de réplication des tableaux de bord d'observabilité, réussite du défi, récupération de la latence
  seuils d'alerte et moniteur

**Écarts résiduels**
- Événements en direct Taikai et simulations de partitions. tests de trempage ضروری ہیں۔
- Réparer la politique de réservation de bande passante codifiée en anglais

### Abus interne**Scénario :** Les politiques d'accès au registre et de rétention des opérateurs manipulent les fichiers
fournisseurs malveillants et liste blanche et alertes supprimées

**Contrôles**
- Actions de gouvernance signatures multipartites اور Norito-actes notariés مانگتی ہیں۔
- Surveillance des changements de politique, des journaux d'archives et des événements
- Journaux Norito en ajout uniquement du pipeline d'observabilité avec application du chaînage de hachage
- Automatisation trimestrielle de l'examen des accès (`cargo xtask da-privilege-audit`) manifeste/relecture
  dirs (chemins fournis par l'opérateur) scan et manquant/non-répertoire/inscriptible dans le monde entier
  entrées flag کرتا ہے، اور les tableaux de bord du bundle JSON signés کیلئے émettent کرتا ہے۔

**Écarts résiduels**
- Inviolabilité du tableau de bord et instantanés signés

## Registre des risques résiduels| Risque | Probabilité | Impact | Propriétaire | Plan d'atténuation |
| --- | --- | --- | --- | --- |
| Cache de séquence DA-2 سے پہلے DA manifeste la relecture | Possible | Modéré | GT sur le protocole principal | DA-2 cache de séquence + implémentation de validation occasionnelle tests de régression |
| >f compromission des nœuds dans la collusion PDP/PoTR | Peu probable | Élevé | Équipe de stockage | Échantillonnage multi-fournisseurs pour le calendrier de défi dérivé harnais de simulation et validation |
| Écart d’audit d’expulsion de niveau froid | Possible | Élevé | SRE / Equipe Stockage | Expulsions avec journaux d'audit signés + reçus en chaîne en pièce jointe tableaux de bord et moniteur |
| Latence de détection d'omission du séquenceur | Possible | Élevé | GT sur le protocole principal | Recettes nocturnes `cargo xtask da-commitment-reconcile` et engagements (SignedBlockWire/`.norito`/JSON) comparez la gouvernance et la page |
| Taikai diffuse en direct sur la résilience des partitions | Possible | Critique | Réseautage TL | Forets de séparation réparer la réserve de bande passante document SOP de basculement |
| Dérive des privilèges de gouvernance | Peu probable | Élevé | Conseil de gouvernance | `cargo xtask da-privilege-audit` trimestriel (répertoires de manifeste/relecture + chemins supplémentaires) avec JSON signé + porte de tableau de bord ; artefacts d'audit et ancre en chaîne |

## Suivis requis1. DA ingère des schémas Norito et des exemples de vecteurs publient des fichiers (DA-2 میں لے جائیں)۔
2. Relecture du cache et Torii DA ingère le fil de discussion et les curseurs de séquence redémarrent
   کے پار persister کریں۔
3. **Terminé (05/02/2026) :** Harnais de simulation PDP/PoTR pour collusion + partition
   scénarios et exercices de modélisation du backlog QoS. دیکھیں
   `integration_tests/src/da/pdp_potr.rs` (tests : `integration_tests/tests/da/pdp_potr_simulation.rs`)۔
4. **Terminé (2026-05-29) :** `cargo xtask da-commitment-reconcile` reçus d'acquisition
   Les engagements DA (SignedBlockWire/`.norito`/JSON) et comparer les engagements
   `artifacts/da/commitment_reconciliation.json` émet کرتا ہے اور Alertmanager/
   paquets de gouvernance filaires (`xtask/src/da.rs`)
5. **Terminé (2026-05-29) :** Spoule de manifeste/relecture `cargo xtask da-privilege-audit`
   (Chemins fournis par l'opérateur) walk کرتا ہے، manquant/non-répertoire/inscriptible dans le monde entier
   flag d'entrées et tableaux de bord de gouvernance du bundle JSON signés
   ہے (`artifacts/da/privilege_audit.json`)۔

**Où chercher ensuite :**- DA-2 utilise le cache de relecture et la persistance du curseur Mise en œuvre
  `crates/iroha_core/src/da/replay_cache.rs` (logique de cache) et intégration Torii
  `crates/iroha_torii/src/da/ingest.rs` pour `/v2/da/ingest` avec empreinte digitale
  vérifie le fil کرتا ہے۔
- Faisceau de preuve de flux de simulations de streaming PDP/PoTR
  `crates/sorafs_car/tests/sorafs_cli.rs`۔ یہ Flux de requêtes PoR/PDP/PoTR ici
  les scénarios de défaillance couvrent le modèle de menace et le modèle de menace
- Capacité de trempage de réparation pour `docs/source/sorafs/reports/sf2c_capacity_soak.md`
  Matrice de trempage Sumeragi `docs/source/sumeragi_soak_matrix.md`
  (variantes localisées شامل ہیں)۔ یہ registre des risques résiduels des artefacts کے forets
  کو capture کرتے ہیں۔
- Réconciliation + automatisation de l'audit des privilèges `docs/automation/da/README.md` اور
  `cargo xtask da-commitment-reconcile` / `cargo xtask da-privilege-audit` maintenant
  paquets de gouvernance avec pièces jointes de preuves par défaut `artifacts/da/`
  sorties استعمال کریں۔

## Preuves de simulation et modélisation de la qualité de service (2026-02)

Suivi DA-1 #3 مکمل کرنے کیلئے ہم نے `integration_tests/src/da/pdp_potr.rs`
( `integration_tests/tests/da/pdp_potr_simulation.rs` سے couvert ) میں


harnais de simulation déterministe PDP/PoTR یہ exploiter les nœuds et les régions
Comment allouer les probabilités de la feuille de route et les partitions/injection de collusion
Il s'agit d'un suivi des retards PoTR et d'un modèle de retard de réparation et d'un flux d'alimentation.
budget de réparation de haut niveau Scénario par défaut (12 époques, 18 PDP
défis + 2 fenêtres PoTR par époque) et métriques نکلے :<!-- BEGIN_DA_SIM_TABLE -->
<!-- AUTO-GENERATED by scripts/docs/render_da_threat_model_tables.py; do not edit manually. -->
| Métrique | Valeur | Remarques |
| --- | --- | --- |
| Pannes PDP détectées | 48 / 49 (98,0%) | Déclencheur de détection de partitions ایک échec non détecté gigue honnête سے ہے۔ |
| Latence de détection moyenne PDP | 0,0 époques | Défaillances d'origine époque کے اندر ظاہر ہوتے ہیں۔ |
| Défaillances PoTR détectées | 28 / 77 (36,4%) | Détection du déclencheur et du nœud >=2 fenêtres PoTR manquées et du registre des risques résiduels |
| Latence de détection moyenne PoTR | époques 2.0 | Escalade d'archives avec le seuil de retard de l'époque et la correspondance avec les détails |
| Pic de la file d'attente de réparation | 38 manifestes | Backlog pour les partitions et les réparations/époques |
| Latence de réponse p95 | 30 068 ms | Fenêtre de défi de 30 s avec échantillonnage QoS avec gigue de +/-75 ms et réflexion du temps |
<!-- END_DA_SIM_TABLE -->

Sorties des prototypes de tableau de bord DA et lecteur de disque et feuille de route
Critères d'acceptation du "faisceau de simulation + modélisation QoS"

Automatisation `cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`
Il s'agit d'un faisceau partagé et d'un appel par défaut par défaut Norito JSON
`artifacts/da/threat_model_report.json` émet کرتا ہے۔ Fichier des travaux de nuit یہ
consommer des matrices de documents, actualiser et taux de détection, réparer
files d'attente, échantillons de QoS, dérive et alerte.Documents en cours d'actualisation de la table par `make docs-da-threat-model`
`cargo xtask da-threat-model-report` invoque کرتا ہے،
`docs/source/da/_generated/threat_model_report.json` régénérer کرتا ہے، اور
`scripts/docs/render_da_threat_model_tables.py` réécriture de section
ہے۔ `docs/portal` miroir (`docs/portal/docs/da/threat-model.md`) pour passer le miroir
update ہوتا ہے تاکہ دونوں copies sync رہیں۔
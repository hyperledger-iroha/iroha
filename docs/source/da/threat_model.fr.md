---
lang: fr
direction: ltr
source: docs/source/da/threat_model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0bff91e735291e82d0d50b5dad4dfbf2b57af68f2f7067760add5da81fc7f554
source_last_modified: "2026-01-19T07:28:06.298292+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Modèle de menace de disponibilité des données Sora Nexus

_Dernière révision : 2026-01-19 — Prochaine révision programmée : 2026-04-19_

Cadence de maintenance : Groupe de travail sur la disponibilité des données (<=90 jours). Chaque révision doit
apparaissent dans `status.md` avec des liens vers des tickets d'atténuation actifs et des artefacts de simulation.

## Objectif et portée

Le programme Data Availability (DA) conserve les diffusions Taikai, les blobs de voie Nexus et
artefacts de gouvernance récupérables sous les fautes byzantines, de réseau et d’opérateur.
Ce modèle de menace ancre le travail d'ingénierie pour DA-1 (architecture et modèle de menace)
et sert de référence pour les tâches DA en aval (DA-2 à DA-10).

Composants concernés :
- Extension d'ingestion Torii DA et rédacteurs de métadonnées Norito.
-Arborescences de stockage blob soutenues par SoraFS (niveaux chaud/froid) et politiques de réplication.
- Engagements de bloc Nexus (formats wire, preuves, API light-client).
- Hooks d'application PDP/PoTR spécifiques aux charges utiles DA.
- Workflows des opérateurs (épinglage, expulsion, slashing) et pipelines d'observabilité.
- Approbations de gouvernance qui admettent ou expulsent les opérateurs et le contenu DA.

Hors de portée de ce document :
- Modélisation économique complète (capturée dans le flux de travail DA-7).
- Protocoles de base SoraFS déjà couverts par le modèle de menace SoraFS.
- Ergonomie du SDK client au-delà des considérations liées à la surface des menaces.

## Aperçu architectural

1. **Soumission :** Les clients soumettent des blobs via l'API d'ingestion Torii DA. Le nœud
   fragmente les blobs, code les manifestes Norito (type de blob, voie, époque, indicateurs de codec),
   et stocke les morceaux dans le niveau chaud SoraFS.
2. **Publicité :** Les intentions d'épinglage et les conseils de réplication se propagent au stockage
   fournisseurs via le registre (place de marché SoraFS) avec des balises de stratégie qui
   indiquer les objectifs de rétention chaud/froid.
3. **Engagement :** Les séquenceurs Nexus incluent des engagements de blob (CID + KZG en option
   racines) dans le bloc canonique. Les clients légers s'appuient sur le hachage d'engagement et
   métadonnées annoncées pour vérifier la disponibilité.
4. **Réplication :** Les nœuds de stockage extraient les partages/morceaux attribués et satisfont PDP/PoTR
   défis et promouvoir les données entre les niveaux chauds et froids par politique.
5. **Récupérer :** Les consommateurs récupèrent les données via des passerelles SoraFS ou compatibles DA, en vérifiant
   preuves et émettre des demandes de réparation lorsque les répliques disparaissent.
6. **Gouvernance :** Le Parlement et le comité de surveillance du DA approuvent les opérateurs,
   les barèmes de loyer et les escalades d’application. Les artefacts de gouvernance sont stockés
   via le même chemin DA pour garantir la transparence du processus. Les paramètres du loyer
   suivis sous DA-7 sont enregistrés dans `docs/source/da/rent_policy.md` donc les audits
   et les examens d’application peuvent faire référence aux montants XOR exacts appliqués par blob.

## Actifs et propriétaires

Échelle d'impact : **Critique** brise la sécurité/la vivacité du grand livre ; **Élevé** bloque DA
remblai ou clients ; **Modéré** dégrade la qualité mais reste récupérable ;
**Faible** effet limité.| Actif | Descriptif | Intégrité | Disponibilité | Confidentialité | Propriétaire |
| --- | --- | --- | --- | --- | --- |
| Blobs DA (morceaux + manifestes) | Taikai, voie, blobs de gouvernance stockés dans SoraFS | Critique | Critique | Modéré | DA WG / Equipe Stockage |
| Norito Manifestes DA | Métadonnées typées décrivant les blobs | Critique | Élevé | Modéré | GT sur le protocole principal |
| Bloquer les engagements | CID + racines KZG à l'intérieur des blocs Nexus | Critique | Élevé | Faible | GT sur le protocole principal |
| Calendriers PDP/PoTR | Cadence d'application pour les répliques DA | Élevé | Élevé | Faible | Équipe de stockage |
| Registre des opérateurs | Fournisseurs et politiques de stockage approuvés | Élevé | Élevé | Faible | Conseil de gouvernance |
| Registres des loyers et des incitations | Entrées de grand livre pour le loyer et les pénalités DA | Élevé | Modéré | Faible | GT Trésorerie |
| Tableaux de bord d'observabilité | SLO DA, profondeur de réplication, alertes | Modéré | Élevé | Faible | SRE / Observabilité |
| Intentions de réparation | Demandes de réhydratation des morceaux manquants | Modéré | Modéré | Faible | Équipe de stockage |

## Adversaires et capacités

| Acteur | Capacités | Motivations | Remarques |
| --- | --- | --- | --- |
| Client malveillant | Soumettez des blobs mal formés, rejouez des manifestes obsolètes, tentez un DoS lors de l'ingestion. | Perturbez les diffusions Taikai, injectez des données invalides. | Pas de clés privilégiées. |
| Nœud de stockage byzantin | Supprimez les répliques attribuées, falsifiez des preuves PDP/PoTR, collaborez avec d’autres. | Réduisez la rétention DA, évitez les loyers, gardez les données en otage. | Possède des informations d'identification d'opérateur valides. |
| Séquenceur compromis | Omettez les engagements, équivoquez les blocs, réorganisez les métadonnées blob. | Cachez les soumissions du DA, créez des incohérences. | Limité par une majorité consensuelle. |
| Opérateur initié | Abuser de l’accès à la gouvernance, altérer les politiques de rétention, divulguer les informations d’identification. | Gain économique, sabotage. | Accès à l’infrastructure de niveau chaud/froid. |
| Adversaire du réseau | Partitionner les nœuds, retarder la réplication, injecter du trafic MITM. | Réduisez la disponibilité, dégradez les SLO. | Impossible de rompre TLS mais peut supprimer/ralentir les liens. |
| Attaquant d'observabilité | Trafiquer les tableaux de bord/alertes, supprimer les incidents. | Masquer les pannes DA. | Nécessite un accès au pipeline de télémétrie. |

## Limites de confiance

- **Limite d'entrée :** Client vers l'extension DA Torii. Nécessite une authentification au niveau de la demande,
  limitation du débit et validation de la charge utile.
- **Limite de réplication :** Nœuds de stockage échangeant des morceaux et des preuves. Les nœuds sont
  mutuellement authentifiés mais peuvent se comporter de manière byzantine.
- ** Limite du grand livre : ** Données de bloc validées par rapport au stockage hors chaîne. Gardiens du consensus
  l'intégrité, mais la disponibilité nécessite une application hors chaîne.
- **Frontière de gouvernance :** Décisions Conseil/Parlement approuvant les opérateurs,
  les budgets et les réductions. Les pauses ici ont un impact direct sur le déploiement de DA.
- ** Limite d'observabilité : ** Collecte de métriques/journaux exportés vers des tableaux de bord/alertes
  outillage. La falsification masque les pannes ou les attaques.

## Scénarios de menaces et contrôles

### Attaques de chemin d'ingestion**Scénario :** Un client malveillant soumet des charges utiles Norito mal formées ou surdimensionnées
des blobs pour épuiser les ressources ou faire passer clandestinement des métadonnées invalides.

**Contrôles**
- Validation du schéma Norito avec négociation stricte de version ; rejeter les drapeaux inconnus.
- Limitation du débit et authentification au point de terminaison d'ingestion Torii.
- Limites de taille des fragments et codage déterministe appliqués par le chunker SoraFS.
- Le pipeline d'admission ne conserve les manifestes qu'après les correspondances de la somme de contrôle d'intégrité.
- Le cache de relecture déterministe (`ReplayCache`) suit les fenêtres `(lane, epoch, sequence)`, conserve les marques de haute eau sur le disque et rejette les doublons/replays obsolètes ; les harnais de propriété et de fuzz couvrent les empreintes digitales divergentes et les soumissions dans le désordre.

**Écarts résiduels**
- L'ingestion Torii doit intégrer le cache de relecture dans les curseurs de séquence d'admission et persister lors des redémarrages.
- Les schémas DA Norito disposent désormais d'un harnais de fuzz dédié (`fuzz/da_ingest_schema.rs`) pour stresser les invariants de codage/décodage ; les tableaux de bord de couverture doivent alerter si la cible régresse.

### Retenue de réplication

**Scénario :** Les opérateurs de stockage byzantins acceptent les attributions de broches mais abandonnent des morceaux,
réussir les défis PDP/PoTR via des réponses falsifiées ou une collusion.

**Contrôles**
- Le calendrier des défis PDP/PoTR s'étend aux charges utiles DA avec une couverture par époque.
- Réplication multi-sources avec seuils de quorum ; récupérer l'orchestrateur détecte
  fragments manquants et déclenche la réparation.
- Des coupures de gouvernance liées aux preuves échouées et aux répliques manquantes.

**Écarts résiduels**
- Harnais de simulation en `integration_tests/src/da/pdp_potr.rs` (couvert par
  `integration_tests/tests/da/pdp_potr_simulation.rs`) exerce désormais une collusion
  et des scénarios de partition, validant que le calendrier PDP/PoTR détecte
  Comportement byzantin de manière déterministe. Continuez à l'étendre le long du DA-5 jusqu'à
  couvrir de nouvelles surfaces d’épreuve.
- La politique d'expulsion de niveau froid nécessite une piste d'audit signée pour éviter les chutes secrètes.

### Falsification des engagements

**Scénario :** Un séquenceur compromis publie des blocs en omettant ou en modifiant DA
engagements, provoquant des échecs de récupération ou des incohérences légères entre les clients.

**Contrôles**
- Le consensus recoupe les propositions de blocage avec les files d'attente de soumission du DA ; les pairs rejettent
  les propositions manquent des engagements requis.
- Les clients légers vérifient les preuves d'inclusion d'engagement avant de faire apparaître les poignées de récupération.
- Piste d'audit comparant les reçus de soumission avec les engagements de bloc.
- Le travail de rapprochement automatisé (`cargo xtask da-commitment-reconcile`) compare
  ingérer des reçus avec des engagements DA (SignedBlockWire, `.norito` ou JSON),
  émet un ensemble de preuves JSON pour la gouvernance et échoue en cas de manque ou
  tickets incompatibles afin qu'Alertmanager puisse pager en cas d'omission/falsification.

**Écarts résiduels**
- Couvert par le travail de réconciliation + hook Alertmanager ; paquets de gouvernance maintenant
  ingérez le paquet de preuves JSON par défaut.

### Partition du réseau et censure**Scénario :** Un adversaire partitionne le réseau de réplication, empêchant les nœuds de
obtenir des morceaux attribués ou répondre aux défis PDP/PoTR.

**Contrôles**
- Les exigences des fournisseurs multirégionaux garantissent des chemins de réseau diversifiés.
- Les fenêtres de défi incluent la gigue et le recours aux canaux de réparation hors bande.
- Les tableaux de bord d'observabilité surveillent la profondeur de la réplication, la réussite des défis et
  récupérer la latence avec des seuils d’alerte.

**Écarts résiduels**
- Les simulations de partition pour les événements en direct de Taikai sont toujours manquantes ; besoin de tests de trempage.
- Réparer la politique de réservation de bande passante non encore codifiée.

### Abus interne

**Scénario :** L'opérateur ayant accès au registre manipule les politiques de rétention.
met sur liste blanche les fournisseurs malveillants ou supprime les alertes.

**Contrôles**
- Les actions de gouvernance nécessitent des signatures multipartites et des actes notariés Norito.
- Les modifications de politique émettent des événements dans les journaux de surveillance et d'archivage.
- Le pipeline d'observabilité applique les journaux Norito en ajout uniquement avec chaînage de hachage.
- Promenades trimestrielles d'automatisation des examens d'accès (`cargo xtask da-privilege-audit`)
  les répertoires de manifeste/relecture DA (plus les chemins fournis par l'opérateur), les indicateurs
  entrées manquantes/non-répertoire/inscriptibles dans le monde entier, et émet un bundle JSON signé
  pour les tableaux de bord de gouvernance.

**Écarts résiduels**
- La preuve d'inviolabilité du tableau de bord nécessite des instantanés signés.

## Registre des risques résiduels

| Risque | Probabilité | Impact | Propriétaire | Plan d'atténuation |
| --- | --- | --- | --- | --- |
| Relecture des manifestes DA avant l'arrivée du cache de séquence DA-2 | Possible | Modéré | GT sur le protocole principal | Implémenter le cache de séquence + la validation occasionnelle dans DA-2 ; ajouter des tests de régression. |
| Collusion PDP/PoTR lorsque les nœuds >f sont compromis | Peu probable | Élevé | Équipe de stockage | Dériver un nouveau calendrier de défis avec un échantillonnage inter-fournisseurs ; valider via un harnais de simulation. |
| Écart d’audit d’expulsion de niveau froid | Possible | Élevé | SRE / Equipe Stockage | Joignez les journaux d'audit signés et les reçus en chaîne pour les expulsions ; surveiller via des tableaux de bord. |
| Latence de détection d'omission du séquenceur | Possible | Élevé | GT sur le protocole principal | Tous les soirs, `cargo xtask da-commitment-reconcile` compare les reçus par rapport aux engagements (SignedBlockWire/`.norito`/JSON) et la gouvernance des pages sur les tickets manquants ou incompatibles. |
| Résilience de partition pour les diffusions en direct de Taikai | Possible | Critique | Réseautage TL | Exécuter des exercices de partitionnement ; réserver la bande passante de réparation ; SOP de basculement de documents. |
| Dérive des privilèges de gouvernance | Peu probable | Élevé | Conseil de gouvernance | Exécution trimestrielle `cargo xtask da-privilege-audit` (répertoires de manifeste/relecture + chemins supplémentaires) avec JSON signé + porte de tableau de bord ; ancrer les artefacts d’audit sur la chaîne. |

## Suivis requis1. Publiez les schémas et les exemples de vecteurs Norito de DA ingest (transportés dans DA-2).
2. Envoyez le cache de relecture via les curseurs de séquence d'acquisition et de persistance Torii DA lors des redémarrages de nœuds.
3. **Terminé (05/02/2026) :** Le harnais de simulation PDP/PoTR exerce désormais des scénarios de collusion + partition avec la modélisation du backlog de QoS ; voir [`integration_tests/src/da/pdp_potr.rs`](/integration_tests/src/da/pdp_potr.rs) (avec des tests sous `integration_tests/tests/da/pdp_potr_simulation.rs`) pour l'implémentation et les résumés déterministes capturés ci-dessous.
4. **Terminé (2026-05-29) :** `cargo xtask da-commitment-reconcile` compare les reçus d'ingestion aux engagements DA (SignedBlockWire/`.norito`/JSON), émet `artifacts/da/commitment_reconciliation.json` et est connecté aux paquets Alertmanager/gouvernance pour les alertes d'omission/falsification. (`xtask/src/da.rs`).
5. **Terminé (2026-05-29) :** `cargo xtask da-privilege-audit` parcourt le spool de manifeste/relecture (plus les chemins fournis par l'opérateur), signale les entrées manquantes/non-répertoire/inscriptibles et produit un ensemble JSON signé pour les tableaux de bord/examens de gouvernance (`artifacts/da/privilege_audit.json`), comblant ainsi l'écart d'automatisation de l'examen des accès.

**Où chercher ensuite :**

- Le cache de relecture DA et la persistance du curseur ont atterri dans DA-2. Voir le
  implémentation dans `crates/iroha_core/src/da/replay_cache.rs` (logique de cache) et
  l'intégration Torii dans `crates/iroha_torii/src/da/ingest.rs`, qui enfile le
  vérification des empreintes digitales via `/v1/da/ingest`.
- Les simulations de streaming PDP/PoTR sont exercées via le harnais proof-stream dans
  `crates/sorafs_car/tests/sorafs_cli.rs`, couvrant les flux de requêtes PoR/PDP/PoTR
  et des scénarios de défaillance animés dans le modèle de menace.
- Les résultats de trempage de capacité et de réparation vivent sous
  `docs/source/sorafs/reports/sf2c_capacity_soak.md`, tandis que le plus large
  La matrice de trempage Sumeragi est suivie dans `docs/source/sumeragi_soak_matrix.md`
  (variantes localisées incluses). Ces artefacts capturent les exercices de longue durée
  référencés dans le registre des risques résiduels.
- L'automatisation de la réconciliation + de l'audit des privilèges est présente
  `docs/automation/da/README.md` et le nouveau `cargo xtask da-commitment-reconcile`
  / Commandes `cargo xtask da-privilege-audit` ; utiliser les sorties par défaut sous
  `artifacts/da/` lors de la pièce jointe de preuves aux paquets de gouvernance.

## Preuves de simulation et modélisation de la qualité de service (2026-02)

Pour clôturer le suivi #3 de DA-1, nous avons codifié une simulation déterministe PDP/PoTR
harnais sous `integration_tests/src/da/pdp_potr.rs` (couvert par
`integration_tests/tests/da/pdp_potr_simulation.rs`). Le harnais
alloue des nœuds dans trois régions, injecte des partitions/collusion en fonction
les probabilités de la feuille de route, suit les retards du PoTR et alimente un retard de réparation
modèle qui reflète le budget de réparation le plus élevé. Exécution du scénario par défaut
(12 époques, 18 défis PDP + 2 fenêtres PoTR par époque) ont produit le
métriques suivantes :<!-- BEGIN_DA_SIM_TABLE -->
<!-- AUTO-GENERATED by scripts/docs/render_da_threat_model_tables.py; do not edit manually. -->
| Métrique | Valeur | Remarques |
| --- | --- | --- |
| Pannes PDP détectées | 48 / 49 (98,0%) | Les partitions déclenchent toujours la détection ; un seul échec non détecté provient d’une gigue honnête. |
| Latence de détection moyenne PDP | 0,0 époques | Les échecs apparaissent à l’époque d’origine. |
| Défaillances PoTR détectées | 28 / 77 (36,4%) | La détection se déclenche lorsqu'un nœud manque ≥2 fenêtres PoTR, laissant la plupart des événements dans le registre des risques résiduels. |
| Latence de détection moyenne PoTR | époques 2.0 | Correspond au seuil de retard de deux époques intégré à la remontée des archives. |
| Pic de la file d'attente de réparation | 38 manifestes | Le retard augmente lorsque les partitions s’empilent plus rapidement que les quatre réparations disponibles par époque. |
| Latence de réponse p95 | 30 068 ms | Reflète la fenêtre de défi de 30 s avec la gigue de ± 75 ms appliquée pour l'échantillonnage QoS. |
<!-- END_DA_SIM_TABLE -->

Ces sorties pilotent désormais les prototypes de tableaux de bord DA et satisfont aux exigences de « simulation ».
« harnais + modélisation QoS » référencés dans la feuille de route.

L'automatisation réside désormais derrière `cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`, qui appelle le faisceau partagé et
émet Norito JSON vers `artifacts/da/threat_model_report.json` par défaut. Tous les soirs
les tâches consomment ce fichier pour actualiser les matrices de ce document et alerter sur
dérive des taux de détection, des files d’attente de réparation ou des échantillons de QoS.

Pour actualiser le tableau ci-dessus pour les documents, exécutez `make docs-da-threat-model`, qui
invoque `cargo xtask da-threat-model-report`, régénère
`docs/source/da/_generated/threat_model_report.json`, et réécrit cette section
via `scripts/docs/render_da_threat_model_tables.py`. Le miroir `docs/portal`
(`docs/portal/docs/da/threat-model.md`) est mis à jour au cours du même passage afin que les deux
les copies restent synchronisées.
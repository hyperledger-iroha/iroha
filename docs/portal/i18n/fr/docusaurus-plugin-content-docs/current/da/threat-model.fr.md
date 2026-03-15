---
lang: fr
direction: ltr
source: docs/portal/docs/da/threat-model.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Source canonique
Reflète `docs/source/da/threat_model.md`. Gardez les deux versions en synchronisation
:::

# Modèle de menaces Disponibilité des données Sora Nexus

_Dernière révision : 2026-01-19 -- Prochaine révision planifiée : 2026-04-19_

Cadence de maintenance : Groupe de travail sur la disponibilité des données (<=90 jours). Chaque
La révision doit apparaître dans `status.md` avec des liens vers les tickets de
atténuation actifs et les artefacts de simulation.

## Mais et périmètre

Le programme Data Availability (DA) maintient les diffusions Taikai, les blobs
Nexus voie et les artefacts de gouvernance récupérables sous des fautes
byzantins, réseau et opérateur. Ce modèle de menaces ancre le travail
ingénierie pour DA-1 (architecture et modèle de menaces) et sert de baseline
pour les taches DA suivantes (DA-2 à DA-10).

Composants dans le périmètre :
- Extension d'ingest DA Torii et rédacteurs de métadonnées Norito.
- Arbres de stockage de blobs adosses a SoraFS (niveaux chaud/froid) et politiques de
  réplication.
- Engagements de bloc Nexus (formats filaires, preuves, APIs light-client).
- Hooks d'enforcement PDP/PoTR spécifiques aux payloads DA.
- Opérateur de workflows (pinning, eviction, slashing) et pipelines
  d'observabilité.
- Approbations de gouvernance qui admettent ou expulsent les opérateurs et
  contenu DA.Hors portée pour ce document :
- Modélisation économique complète (captée dans le workstream DA-7).
- Protocoles base SoraFS deja couverts par le modèle de menaces SoraFS.
- Ergonomie des clients SDK au-delà des considérations de surface de menace.

## Vue d'ensemble architecturale1. **Soumission :** Les clients soumettent des blobs via l'API d'ingest DA Torii.
   Le noeud découpe les blobs, encode les manifests Norito (type de blob, lane,
   epoch, flags de codec), et stocke les chunks dans le tier hot SoraFS.
2. **Annonce :** Pin intents et astuces de réplication se propagent vers les
   fournisseurs via le registre (SoraFS Marketplace) avec des tags de politique qui
   indiquant les objectifs de rétention chaud/froid.
3. **Engagement :** Les séquenceurs Nexus incluent des engagements de blobs (CID +
   racines KZG optionnelles) dans le bloc canonique. Les clients légers se basent
   sur le hash de engagement et la métadonnée annoncée pour vérificateur
   l'disponibilité.
4. **Réplication :** Les nœuds de stockage tirent les partages/morceaux attribués,
   satisfont les challenges PDP/PoTR, et promettent les données entre niveaux chauds
   et froid selon la politique.
5. **Fetch :** Les consommateurs récupèrent les données via SoraFS ou des gateways
   DA-aware, vérifie les preuves et emettent des demandes de réparation quand
   des répliques disparaissent.
6. **Gouvernance:** Le Parlement et le comité de supervision DA approuvent les
   opérateurs, plannings de loyer et escalades d'exécution. Les artefacts de
   la gouvernance est stockée via la même voie DA pour garantir la transparence.

## Actifs et propriétairesÉchelle d'impact : **Critique** casse la sécurité/vivacité du grand livre; **Onze**
bloque le backfill DA ou les clients; **Modere** dégrade la qualité mais reste
récupérable; **Faible** effet limite.

| Actif | Descriptif | Intégrer | Disponibilité | Confidentialité | Propriétaire |
| --- | --- | --- | --- | --- | --- |
| Blobs DA (morceaux + manifestes) | Blobs Taikai, lane, gouvernance stocks dans SoraFS | Critique | Critique | Modere | DA WG / Equipe Stockage |
| Manifestes Norito DA | Type de métadonnées décrivant les blobs | Critique | Onze | Modere | GT sur le protocole principal |
| Engagements de bloc | CIDs + racines KZG dans les blocs Nexus | Critique | Onze | Faible | GT sur le protocole principal |
| Horaires PDP/PoTR | Cadence d'enforcement pour les répliques DA | Onze | Onze | Faible | Équipe de stockage |
| Opérateur de registre | Fournisseurs de stockage approuvés et politiques | Onze | Onze | Faible | Conseil de gouvernance |
| Registres de loyers et incitations | Registre des entrées pour louer DA et pénalités | Onze | Modere | Faible | GT Trésorerie |
| Tableaux de bord d'observabilité | SLO DA, profondeur de réplication, alertes | Modere | Onze | Faible | SRE / Observabilité |
| Intentions de réparation | Requetes pour réhydrater des morceaux manquants | Modere | Modere | Faible | Équipe de stockage |

## Adversaires et capacités| Acteur | Capacités | Motivations | Remarques |
| --- | --- | --- | --- |
| Client malveillant | Soumettre des blobs malformes, rejouer des manifestes obsolètes, tenter DoS sur l'ingest. | Perturber les émissions Taikai, injecter des données invalides. | Pas de clés privilèges. |
| Noeud de stockage byzantin | Dropper des répliques cessionnaires, faussaire des épreuves PDP/PoTR, complice. | Réduire la rétention DA, éviter la rente, retenir les données. | Possède des identifiants valides. |
| Compromis du séquenceur | Omettre des engagements, équivoker sur les blocs, réordonner les métadonnées. | Cacher des soumissions DA, créer des incohérences. | Limite par la majorité de consensus. |
| Opérateur interne | Abuser de l'accès gouvernance, manipuler les politiques de rétention, fuir des accréditations. | Gain économique, sabotage. | Accès à l'infrastructure chaud/froid. |
| Réseau adverse | Partitionner les noeuds, retarder la réplication, injecter trafic MITM. | Réduire la disponibilité, dégrader les SLO. | Ne peut pas casser TLS mais peut dropper/ralentir les liens. |
| Observabilité attaquante | Tamper les tableaux de bord/alertes, supprimer les incidents. | Cacher les pannes DA. | Nécessite un accès à la télémétrie par pipeline. |

## Frontières de confiance- **Frontiere ingress:** Client vers l'extension DA Torii. Demander un numéro d'authentification
  requête, limitation du débit et validation du payload.
- **Réplication Frontiere :** Noeuds de stockage échangeant des morceaux et des preuves. Les
  les nœuds sont mutuellement authentifiés mais peuvent se comporter en byzantin.
- **Grand livre frontière :** Donnees de bloc commits vs stockage off-chain. Le
  le consensus garde l'intégrité, mais la disponibilité requiert une application
  hors chaîne.
- **Frontière gouvernance :** Décisions Conseil/Parlement approuvant les opérateurs,
  budgets et réductions. Les bris ici impactent directement le déploiement DA.
- **Frontiere observabilité :** Collecte de métriques/logs exportés vers les tableaux de bord et
  outillage d'alerte. Les pannes ou attaques du cache de falsification.

## Scénarios de menace et de contrôle

### Attaques sur le chemin d'ingest

**Scenario :** Client malveillant soumet des payloads Norito malformes ou des
blobs surdimensionnés pour épuiser les ressources ou injecter une métadonnée
invalide.**Contrôles**
- Validation du schéma Norito avec négociation stricte de version ; rejeter les
  drapeaux inconnus.
- Limitation de débit et authentification sur le point final d'ingest Torii.
- Bornes de chunk size et forces déterministes d'encodage par le chunker SoraFS.
- Pipeline d'admission ne persiste les manifestes qu'après match du checksum.
- Replay cache déterministe (`ReplayCache`) suit les fenêtres `(voie, époque,
  séquence)`, persiste des high-water marks sur disque, et rejette les doublons
  et replays obsolètes; harnais propriété et fuzz couvrant les empreintes digitales
  divergences et soumissions hors ordre. [crates/iroha_core/src/da/replay_cache.rs:1]
  [fuzz/da_replay_cache.rs:1] [crates/iroha_torii/src/da/ingest.rs:1]

**Lacunes résiduelles**
- Torii ingest doit relier le replay cache à l'admission et persister les curseurs
  de séquence à travers les redémarrages.
- Les schémas Norito DA ont maintenant un harnais fuzz dédié
  (`fuzz/da_ingest_schema.rs`) pour stresser les invariants d'encode/decode; les
  les tableaux de bord de couverture doivent alerter si la cible régresse.

### Rétention par retenue de réplication

**Scénario :** Les opérateurs de stockage byzantins acceptent les broches mais les dropent
chunks, passant les challenges PDP/PoTR via des réponses forgées ou collusion.**Contrôles**
- Le planning de challenges PDP/PoTR s'étend aux charges utiles DA avec couverture par
  époque.
- Réplication multi-source avec seuils de quorum ; l'orchestrateur détecte les
  shards manquants et declenche la réparation.
- Slashing de gouvernance lie aux preuves échos et aux répliques manquantes.
- Job de réconciliation automatisée (`cargo xtask da-commitment-reconcile`) ici
  comparer les recettes d'ingest avec les engagements DA (SignedBlockWire,
  `.norito` ou JSON), emet un bundle JSON d'evidence pour la gouvernance, et
  echoue sur tickets manquants/mismatches pour qu'Alertmanager puisse pager.

**Lacunes résiduelles**
- Le harnais de simulation dans `integration_tests/src/da/pdp_potr.rs` (couvert
  par `integration_tests/tests/da/pdp_potr_simulation.rs`) exercer des scénarios
  de collusion et partition, validant que le planning PDP/PoTR détecte le
  comportement byzantin de facon déterministe. Continuer à l'étendre avec DA-5
  pour couvrir de nouvelles surfaces de preuve.
- La politique d'expulsion à froid requiert un audit trail signé pour prévenir
  les gouttes furtifs.

### Manipulation des engagements

**Scénario :** Sequencer compromis publie des blocs omettant ou modifiant les
engagements DA, entraînant des échecs de fetch ou des incohérences light-client.**Contrôles**
- Le consensus vérifie les propositions de bloc contre les fichiers de soumission
  DA ; les pairs rejettent les propositions sans engagements requis.
- Les clients légers vérifient les épreuves d'inclusion avant d'exposer les poignées
  de récupérer.
- Audit trail comparant les recettes d'ingest aux engagements de bloc.
- Job de réconciliation automatisée (`cargo xtask da-commitment-reconcile`) ici
  comparer les recettes d'ingest avec les engagements DA (SignedBlockWire,
  `.norito` ou JSON), emet un bundle JSON d'evidence pour la gouvernance, et
  echoue sur tickets manquants ou mismatches pour qu'Alertmanager puisse pager.

**Lacunes résiduelles**
- Couvert par le job de réconciliation + hook Alertmanager ; les paquets de
  la gouvernance ingère maintenant le bundle JSON d'evidence par défaut.

### Partition réseau et censure

**Scenario :** Adversaire partitionne le réseau de réplication, empechant les
noeuds d'obtenir les chunks assignés ou de répondre aux challenges PDP/PoTR.

**Contrôles**
- Exigences de prestataires multi-régions garantissant des chemins réseaux divers.
- Fenêtres de challenge avec jitter et fallback vers des canaux de réparation
  hors-bande.
- Tableaux de bord d'observabilité surveillant la profondeur de réplication, les
  succès de défi et latence de récupération avec seuils d'alerte.**Lacunes résiduelles**
- Simulations de partition pour les événements Taikai live encore manquantes ;
  besoin de tests de trempage.
- Politique de réservation de bande passante de réparation pas encore codifiée.

### Abus interne

**Scénario :** Opérateur avec accès au registre manipule les politiques de
conservation, liste blanche des fournisseurs malveillants ou suppression des alertes.

**Contrôles**
- Actions de gouvernance nécessitant des signatures multipartites et des records
  notarie Norito.
- Les changements de politique emettent des événements vers monitoring et logs
  d'archives.
- Le pipeline d'observabilité impose des logs Norito append-only avec hash
  enchaînement.
- L'automatisation d'audit trimestriel (`cargo xtask da-privilege-audit`) parcourt
  les répertoires manifest/replay (plus les chemins fournis par les opérateurs), signale les
  entrées manquantes/non-directory/world-writable, et emet un bundle JSON signé
  pour les tableaux de bord de gouvernance.

**Lacunes résiduelles**
- La preuve de falsification du tableau de bord nécessite des signes d'instantanés.

## Registre de risques résiduels| Risques | Probabilité | Impact | Propriétaire | Plan d'atténuation |
| --- | --- | --- | --- | --- |
| Replay de manifests DA avant l'arrivée du séquence cache DA-2 | Possible | Modere | GT sur le protocole principal | Cache de séquence d'implémenteur + validation de nonce en DA-2 ; ajouter des tests de régression. |
| Collusion PDP/PoTR quand >f noeuds sont compromis | Peu probable | Onze | Équipe de stockage | Dériver un nouveau calendrier de défis avec échantillonnage multi-fournisseur ; valider via le harnais de simulation. |
| Gap d'audit d'expulsion cold-tier | Possible | Onze | SRE / Equipe Stockage | Attacher des journaux de signalisation et reçus en chaîne pour les expulsions ; moniteur via des tableaux de bord. |
| Latence de détection d'omission de séquenceur | Possible | Onze | GT sur le protocole principal | `cargo xtask da-commitment-reconcile` nocturne compare les recettes vs les engagements (SignedBlockWire/`.norito`/JSON) et page la gouvernance sur les tickets manquants ou mismatches. |
| Partition de résilience pour les streams Taikai live | Possible | Critique | Réseautage TL | Exécuteur des exercices de partition; réserver la bande passante de réparation; documenter SOP de basculement. |
| Dérive de privilège de gouvernance | Peu probable | Onze | Conseil de gouvernance | `cargo xtask da-privilege-audit` trimestriel (répertoires manifest/replay + chemins extra) avec signe JSON + tableau de bord gate ; ancrer les artefacts d'audit en chaîne. |

## Suivis requis1. Publier les schémas Norito d'ingest DA et des vecteurs d'exemple (porte dans
   DA-2).
2. Brancher le replay cache dans l'ingest DA Torii et persister les curseurs de
   séquence à travers les redémarrages.
3. **Termine (2026-02-05):** Le harnais de simulation PDP/PoTR exerce maintenant
   collusion + partition avec modélisation de backlog QoS ; voir
   `integration_tests/src/da/pdp_potr.rs` (tests sous
   `integration_tests/tests/da/pdp_potr_simulation.rs`) pour l'implémentation et
   les résumés déterministes captures ci-dessous.
4. **Termine (2026-05-29) :** `cargo xtask da-commitment-reconcile` comparer les fichiers
   reçus d'ingest aux engagements DA (SignedBlockWire/`.norito`/JSON), emet
   `artifacts/da/commitment_reconciliation.json`, et est branche a
   Alertmanager/paquets de gouvernance pour alertes d'omission/falsification
   (`xtask/src/da.rs`).
5. **Termine (2026-05-29):** `cargo xtask da-privilege-audit` parcourt le spool
   manifest/replay (plus chemins fournis par les opérateurs), signale des entrées
   manquantes/non-directory/world-writable, et produit un bundle JSON signé pour
   dashboards/revues de gouvernance (`artifacts/da/privilege_audit.json`),
   fermant la lacune d’automatisation d’accès.

**Ou regarder ensuite:**- Le replay cache et la persistance des curseurs atterrissent en DA-2. Voir
  l'implémentation dans `crates/iroha_core/src/da/replay_cache.rs` (logique de
  cache) et l'intégration Torii dans `crates/iroha_torii/src/da/ingest.rs`, qui thread
  les contrôles d'empreintes digitales via `/v1/da/ingest`.
- Les simulations de streaming PDP/PoTR sont exercées via le harnais proof-stream
  dans `crates/sorafs_car/tests/sorafs_cli.rs`, couvrant les flux de demande
  PoR/PDP/PoTR et les scénarios de échec animés dans le modèle de menaces.
- Les résultats de capacité et de réparation sont vivants sous
  `docs/source/sorafs/reports/sf2c_capacity_soak.md`, tandis que la matrice de
  Soak Sumeragi plus large est suivi dans `docs/source/sumeragi_soak_matrix.md`
  (variantes localisées incluses). Ces artefacts capturent les exercices à long terme
  référencées dans le registre des risques résiduels.
- L'automatisation réconciliation + privilège-audit vit dans
  `docs/automation/da/README.md` et les nouvelles commandes
  `cargo xtask da-commitment-reconcile`/`cargo xtask da-privilege-audit` ; utiliser
  les sorties par défaut sous `artifacts/da/` lors de l'attachement d'evidence aux
  paquets de gouvernance.

## Preuve de simulation et modélisation QoS (2026-02)Pour fermer le suivi DA-1 #3, nous avons code un harnais de simulation
PDP/PoTR déterministe sous `integration_tests/src/da/pdp_potr.rs` (couvert par
`integration_tests/tests/da/pdp_potr_simulation.rs`). Le harnais alloue des
noeuds sur trois régions, injecte partitions/collusion selon les probabilités du
roadmap, suit le retard PoTR, et alimente un modèle de backlog de réparation
qui reflète le budget de réparation du niveau chaud. L'exécution du scénario par
par défaut (12 époques, 18 défis PDP + 2 fenêtres PoTR par époque) a produit les
métriques suivantes :<!-- BEGIN_DA_SIM_TABLE -->
<!-- AUTO-GENERATED by scripts/docs/render_da_threat_model_tables.py; do not edit manually. -->
| Métrique | Valeur | Remarques |
| --- | --- | --- |
| Défaillances PDP détectées | 48 / 49 (98,0%) | Les cloisons se declenchent encore la détection ; un seul échec non détecté vient d'un jitter honnête. |
| Latence de détection moyenne PDP | 0,0 époques | Les émissions sont des signaux dans l'époque d'origine. |
| Défaillances PoTR détectées | 28 / 77 (36,4%) | La détection se déclenche lorsqu'un nœud taux >=2 fenêtres PoTR, laissant la plupart des événements dans le registre de risques résiduels. |
| Latence de détection moyenne PoTR | époques 2.0 | Correspond au seuil de retard à deux époques intégrées dans l'escalade d'archivage. |
| Pic de la file d'attente de réparation | 38 manifestes | L'arriéré monte quand les cloisons s'empilent plus vite que les quatre réparations disponibles par époque. |
| Latence de réponse p95 | 30 068 ms | Reflète la fenêtre de défi 30 s avec une gigue de +/-75 ms appliquée au QoS d'échantillonnage. |
<!-- END_DA_SIM_TABLE -->

Ces sorties alimentent maintenant les prototypes de tableau de bord DA et satisfont
les critères d'acceptation "faisceau de simulation + modélisation QoS" références dans
la feuille de route.L'automatisation vit derrière
`cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`,
qui appelle le harnais partage et emet Norito JSON vers
`artifacts/da/threat_model_report.json` par défaut. Les jobs nocturnes
consomment ce fichier pour rafraichir les matrices dans ce document et alerter
sur la dérive des taux de détection, des files d’attente de réparation ou des échantillons QoS.

Pour rafraichir la table ci-dessus pour les docs, exécuteur `make docs-da-threat-model`,
qui invoque `cargo xtask da-threat-model-report`, régénérer
`docs/source/da/_generated/threat_model_report.json`, et réécrit cette section
via `scripts/docs/render_da_threat_model_tables.py`. Le miroir `docs/portal`
(`docs/portal/docs/da/threat-model.md`) est mis a jour dans le meme passage pour
que les deux copies restent en synchronisation.
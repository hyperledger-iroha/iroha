---
lang: he
direction: rtl
source: docs/portal/i18n/fr/docusaurus-plugin-content-docs/current/da/threat-model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8937598d9405b6f7817019f1e31cff53e0f0df9dbfcf14b8e1ec57f6c52c0aa1
source_last_modified: "2026-01-19T07:28:06+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: fr
direction: ltr
source: docs/portal/docs/da/threat-model.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Source canonique
Reflete `docs/source/da/threat_model.md`. Gardez les deux versions en sync
:::

# Modele de menaces Data Availability Sora Nexus

_Derniere revision: 2026-01-19 -- Prochaine revision planifiee: 2026-04-19_

Cadence de maintenance: Data Availability Working Group (<=90 jours). Chaque
revision doit apparaitre dans `status.md` avec des liens vers les tickets de
mitigation actifs et les artefacts de simulation.

## But et perimetre

Le programme Data Availability (DA) maintient les broadcasts Taikai, les blobs
Nexus lane et les artefacts de gouvernance recuperables sous des fautes
byzantines, reseau et operateur. Ce modele de menaces ancre le travail
engineering pour DA-1 (architecture et modele de menaces) et sert de baseline
pour les taches DA suivantes (DA-2 a DA-10).

Composants dans le scope:
- Extension d'ingest DA Torii et writers de metadata Norito.
- Arbres de stockage de blobs adosses a SoraFS (tiers hot/cold) et politiques de
  replication.
- Commitments de bloc Nexus (wire formats, proofs, APIs light-client).
- Hooks d'enforcement PDP/PoTR specifiques aux payloads DA.
- Workflows operateur (pinning, eviction, slashing) et pipelines
  d'observabilite.
- Approbations de gouvernance qui admettent ou expulsent les operateurs et
  contenus DA.

Hors scope pour ce document:
- Modelisation economique complete (capturee dans le workstream DA-7).
- Protocoles base SoraFS deja couverts par le modele de menaces SoraFS.
- Ergonomie des SDK clients au-dela des considerations de surface de menace.

## Vue d'ensemble architecturale

1. **Soumission:** Les clients soumettent des blobs via l'API d'ingest DA Torii.
   Le noeud decoupe les blobs, encode les manifests Norito (type de blob, lane,
   epoch, flags de codec), et stocke les chunks dans le tier hot SoraFS.
2. **Annonce:** Pin intents et hints de replication se propagent vers les
   providers via le registry (SoraFS marketplace) avec des tags de politique qui
   indiquent les objectifs de retention hot/cold.
3. **Commitment:** Les sequencers Nexus incluent des commitments de blobs (CID +
   racines KZG optionnelles) dans le bloc canonique. Les light clients se basent
   sur le hash de commitment et la metadata annoncee pour verifier
   l'availability.
4. **Replication:** Les noeuds de stockage tirent les shares/chunks assignes,
   satisfont les challenges PDP/PoTR, et promeuvent les donnees entre tiers hot
   et cold selon la politique.
5. **Fetch:** Les consommateurs recuperent les donnees via SoraFS ou des gateways
   DA-aware, verifient les proofs et emettent des demandes de reparation quand
   des replicas disparaissent.
6. **Gouvernance:** Le Parlement et le comite de supervision DA approuvent les
   operateurs, schedules de rent et escalations d'enforcement. Les artefacts de
   gouvernance sont stockes via la meme voie DA pour garantir la transparence.

## Actifs et proprietaires

Echelle d'impact: **Critique** casse la securite/vivacite du ledger; **Eleve**
bloque le backfill DA ou les clients; **Modere** degrade la qualite mais reste
recuperable; **Faible** effet limite.

| Actif | Description | Integrite | Disponibilite | Confidentialite | Owner |
| --- | --- | --- | --- | --- | --- |
| Blobs DA (chunks + manifests) | Blobs Taikai, lane, gouvernance stockes dans SoraFS | Critique | Critique | Modere | DA WG / Storage Team |
| Manifests Norito DA | Metadata typee decrivant les blobs | Critique | Eleve | Modere | Core Protocol WG |
| Commitments de bloc | CIDs + racines KZG dans les blocs Nexus | Critique | Eleve | Faible | Core Protocol WG |
| Schedules PDP/PoTR | Cadence d'enforcement pour les replicas DA | Eleve | Eleve | Faible | Storage Team |
| Registry operateur | Providers de stockage approuves et politiques | Eleve | Eleve | Faible | Governance Council |
| Registres de rent et incentives | Entrees ledger pour rent DA et penalites | Eleve | Modere | Faible | Treasury WG |
| Dashboards d'observabilite | SLOs DA, profondeur de replication, alertes | Modere | Eleve | Faible | SRE / Observability |
| Intents de reparation | Requetes pour rehydrater des chunks manquants | Modere | Modere | Faible | Storage Team |

## Adversaires et capacites

| Acteur | Capacites | Motivations | Notes |
| --- | --- | --- | --- |
| Client malveillant | Soumettre des blobs malformes, rejouer des manifests obsoletes, tenter DoS sur l'ingest. | Perturber les broadcasts Taikai, injecter des donnees invalides. | Pas de cles privilegiees. |
| Noeud de stockage byzantin | Dropper des replicas assignees, forger des proofs PDP/PoTR, colluder. | Reduire la retention DA, eviter la rent, retenir les donnees. | Possede des credentials valides. |
| Sequencer compromis | Omettre des commitments, equivoker sur les blocs, reordonner la metadata. | Cacher des submissions DA, creer des incoherences. | Limite par la majorite de consensus. |
| Operateur interne | Abuser de l'acces gouvernance, manipuler les politiques de retention, fuir des credentials. | Gain economique, sabotage. | Acces a l'infrastructure hot/cold. |
| Adversaire reseau | Partitioner les noeuds, retarder la replication, injecter trafic MITM. | Reduire l'availability, degrader les SLOs. | Ne peut pas casser TLS mais peut dropper/ralentir les liens. |
| Attaquant observabilite | Tamper dashboards/alertes, supprimer les incidents. | Cacher les outages DA. | Necessite un acces a la pipeline telemetrie. |

## Frontieres de confiance

- **Frontiere ingress:** Client vers l'extension DA Torii. Requiert auth par
  requete, rate limiting et validation du payload.
- **Frontiere replication:** Noeuds de stockage echangent chunks et proofs. Les
  noeuds sont mutuellement authentifies mais peuvent se comporter en byzantin.
- **Frontiere ledger:** Donnees de bloc commits vs stockage off-chain. Le
  consensus garde l'integrite, mais l'availability requiert un enforcement
  off-chain.
- **Frontiere gouvernance:** Decisions Council/Parliament approuvant operateurs,
  budgets et slashing. Les bris ici impactent directement le deploiement DA.
- **Frontiere observabilite:** Collecte metrics/logs exportee vers dashboards et
  alert tooling. Le tampering cache outages ou attaques.

## Scenarios de menace et controles

### Attaques sur le chemin d'ingest

**Scenario:** Client malveillant soumet des payloads Norito malformes ou des
blobs surdimensionnes pour epuiser les ressources ou injecter une metadata
invalide.

**Controles**
- Validation de schema Norito avec negotiation stricte de version; rejeter les
  flags inconnus.
- Rate limiting et authentification sur l'endpoint d'ingest Torii.
- Bornes de chunk size et encoding deterministe forces par le chunker SoraFS.
- Pipeline d'admission ne persiste les manifests qu'apres match du checksum.
- Replay cache deterministe (`ReplayCache`) suit les fenetres `(lane, epoch,
  sequence)`, persiste des high-water marks sur disque, et rejette les duplicates
  et replays obsoletes; harnesses property et fuzz couvrent les fingerprints
  divergents et submissions hors ordre. [crates/iroha_core/src/da/replay_cache.rs:1]
  [fuzz/da_replay_cache.rs:1] [crates/iroha_torii/src/da/ingest.rs:1]

**Lacunes residuelles**
- Torii ingest doit relier le replay cache a l'admission et persister les curseurs
  de sequence a travers les redemarrages.
- Les schemas Norito DA ont maintenant un fuzz harness dedie
  (`fuzz/da_ingest_schema.rs`) pour stresser les invariants d'encode/decode; les
  dashboards de couverture doivent alerter si la cible regress.

### Retention par withholding de replication

**Scenario:** Operateurs de stockage byzantins acceptent les pins mais dropent les
chunks, passant les challenges PDP/PoTR via des reponses forgees ou collusion.

**Controles**
- Le schedule de challenges PDP/PoTR s'etend aux payloads DA avec couverture par
  epoch.
- Replication multi-source avec seuils de quorum; l'orchestrateur detecte les
  shards manquants et declenche la reparation.
- Slashing de gouvernance lie aux proofs echoues et aux replicas manquantes.
- Job de reconciliation automatisee (`cargo xtask da-commitment-reconcile`) qui
  compare les receipts d'ingest avec les commitments DA (SignedBlockWire,
  `.norito` ou JSON), emet un bundle JSON d'evidence pour la gouvernance, et
  echoue sur tickets manquants/mismatches pour que Alertmanager puisse pager.

**Lacunes residuelles**
- Le harness de simulation dans `integration_tests/src/da/pdp_potr.rs` (couvert
  par `integration_tests/tests/da/pdp_potr_simulation.rs`) exerce des scenarios
  de collusion et partition, validant que le schedule PDP/PoTR detecte le
  comportement byzantin de facon deterministe. Continuer a l'etendre avec DA-5
  pour couvrir de nouvelles surfaces de proof.
- La politique d'eviction cold-tier requiert un audit trail signe pour prevenir
  les drops furtifs.

### Manipulation des commitments

**Scenario:** Sequencer compromis publie des blocs omettant ou modifiant les
commitments DA, provoquant des fetch failures ou des incoherences light-client.

**Controles**
- Le consensus verifie les propositions de bloc contre les files de submission
  DA; les peers rejettent les propositions sans commitments requis.
- Les light clients verifient les inclusion proofs avant d'exposer les handles
  de fetch.
- Audit trail comparant les receipts d'ingest aux commitments de bloc.
- Job de reconciliation automatisee (`cargo xtask da-commitment-reconcile`) qui
  compare les receipts d'ingest avec les commitments DA (SignedBlockWire,
  `.norito` ou JSON), emet un bundle JSON d'evidence pour la gouvernance, et
  echoue sur tickets manquants ou mismatches pour que Alertmanager puisse pager.

**Lacunes residuelles**
- Couvert par le job de reconciliation + hook Alertmanager; les paquets de
  gouvernance ingere nt maintenant le bundle JSON d'evidence par defaut.

### Partition reseau et censure

**Scenario:** Adversaire partitionne le reseau de replication, empechant les
noeuds d'obtenir les chunks assignes ou de repondre aux challenges PDP/PoTR.

**Controles**
- Exigences de providers multi-region garantissent des chemins reseau divers.
- Fenetres de challenge avec jitter et fallback vers des canaux de reparation
  hors bande.
- Dashboards d'observabilite surveillent la profondeur de replication, les
  succes de challenge et la latence de fetch avec seuils d'alerte.

**Lacunes residuelles**
- Simulations de partition pour les evenements Taikai live encore manquantes;
  besoin de soak tests.
- Politique de reservation de bande passante de reparation pas encore codifiee.

### Abus interne

**Scenario:** Operateur avec acces au registry manipule les politiques de
retention, whitelist des providers malveillants ou supprime les alertes.

**Controles**
- Actions de gouvernance requierent des signatures multi-party et des records
  notarises Norito.
- Les changements de politique emettent des evenements vers monitoring et logs
  d'archive.
- Le pipeline d'observabilite impose des logs Norito append-only avec hash
  chaining.
- L'automatisation d'audit trimestriel (`cargo xtask da-privilege-audit`) parcourt
  les repertoires manifest/replay (plus paths fournis par operateurs), signale les
  entries manquantes/non-directory/world-writable, et emet un bundle JSON signe
  pour dashboards de gouvernance.

**Lacunes residuelles**
- La preuve de tamper dashboard requiert des snapshots signes.

## Registre de risques residuels

| Risque | Probabilite | Impact | Owner | Plan de mitigation |
| --- | --- | --- | --- | --- |
| Replay de manifests DA avant l'arrivee du sequence cache DA-2 | Possible | Modere | Core Protocol WG | Implementer sequence cache + validation de nonce en DA-2; ajouter des tests de regression. |
| Collusion PDP/PoTR quand >f noeuds sont compromis | Peu probable | Eleve | Storage Team | Deriver un nouveau schedule de challenges avec sampling cross-provider; valider via harness de simulation. |
| Gap d'audit d'eviction cold-tier | Possible | Eleve | SRE / Storage Team | Attacher des logs signes & receipts on-chain pour evictions; monitorer via dashboards. |
| Latence de detection d'omission de sequencer | Possible | Eleve | Core Protocol WG | `cargo xtask da-commitment-reconcile` nocturne compare receipts vs commitments (SignedBlockWire/`.norito`/JSON) et page la gouvernance sur tickets manquants ou mismatches. |
| Resilience partition pour streams Taikai live | Possible | Critique | Networking TL | Executer des drills de partition; reserver la bande passante de reparation; documenter SOP de failover. |
| Derive de privilege de gouvernance | Peu probable | Eleve | Governance Council | `cargo xtask da-privilege-audit` trimestriel (dirs manifest/replay + paths extra) avec JSON signe + gate dashboard; ancrer les artefacts d'audit on-chain. |

## Follow-ups requis

1. Publier les schemas Norito d'ingest DA et des vecteurs d'exemple (porte dans
   DA-2).
2. Brancher le replay cache dans l'ingest DA Torii et persister les curseurs de
   sequence a travers les redemarrages.
3. **Termine (2026-02-05):** Le harness de simulation PDP/PoTR exerce maintenant
   collusion + partition avec modelisation de backlog QoS; voir
   `integration_tests/src/da/pdp_potr.rs` (tests sous
   `integration_tests/tests/da/pdp_potr_simulation.rs`) pour l'implementation et
   les resumes deterministes captures ci-dessous.
4. **Termine (2026-05-29):** `cargo xtask da-commitment-reconcile` compare les
   receipts d'ingest aux commitments DA (SignedBlockWire/`.norito`/JSON), emet
   `artifacts/da/commitment_reconciliation.json`, et est branche a
   Alertmanager/paquets de gouvernance pour alertes d'omission/tampering
   (`xtask/src/da.rs`).
5. **Termine (2026-05-29):** `cargo xtask da-privilege-audit` parcourt le spool
   manifest/replay (plus paths fournis par operateurs), signale des entries
   manquantes/non-directory/world-writable, et produit un bundle JSON signe pour
   dashboards/revues de gouvernance (`artifacts/da/privilege_audit.json`),
   fermant la lacune d'automatisation d'acces.

**Ou regarder ensuite:**

- Le replay cache et la persistence des curseurs ont atterri en DA-2. Voir
  l'implementation dans `crates/iroha_core/src/da/replay_cache.rs` (logique de
  cache) et l'integration Torii dans `crates/iroha_torii/src/da/ingest.rs`, qui thread
  les checks de fingerprint via `/v2/da/ingest`.
- Les simulations de streaming PDP/PoTR sont exercees via le harness proof-stream
  dans `crates/sorafs_car/tests/sorafs_cli.rs`, couvrant les flux de requete
  PoR/PDP/PoTR et les scenarios de failure animes dans le modele de menaces.
- Les resultats de capacity et repair soak vivent sous
  `docs/source/sorafs/reports/sf2c_capacity_soak.md`, tandis que la matrice de
  soak Sumeragi plus large est suivie dans `docs/source/sumeragi_soak_matrix.md`
  (variants localises inclus). Ces artefacts capturent les drills long terme
  referencees dans le registre de risques residuels.
- L'automatisation reconciliation + privilege-audit vit dans
  `docs/automation/da/README.md` et les nouvelles commandes
  `cargo xtask da-commitment-reconcile` / `cargo xtask da-privilege-audit`; utilisez
  les sorties par defaut sous `artifacts/da/` lors de l'attachement d'evidence aux
  paquets de gouvernance.

## Evidence de simulation et modelisation QoS (2026-02)

Pour fermer le follow-up DA-1 #3, nous avons code un harness de simulation
PDP/PoTR deterministe sous `integration_tests/src/da/pdp_potr.rs` (couvert par
`integration_tests/tests/da/pdp_potr_simulation.rs`). Le harness alloue des
noeuds sur trois regions, injecte partitions/collusion selon les probabilites du
roadmap, suit la lateness PoTR, et alimente un modele de backlog de reparation
qui reflecte le budget de reparation du tier hot. L'execution du scenario par
defaut (12 epochs, 18 challenges PDP + 2 fenetres PoTR par epoch) a produit les
metriques suivantes:

<!-- BEGIN_DA_SIM_TABLE -->
<!-- AUTO-GENERATED by scripts/docs/render_da_threat_model_tables.py; do not edit manually. -->
| Metrique | Valeur | Notes |
| --- | --- | --- |
| PDP failures detectees | 48 / 49 (98.0%) | Les partitions declenchent encore la detection; un seul echec non detecte vient d'un jitter honnete. |
| PDP mean detection latency | 0.0 epochs | Les echecs sont signales dans l'epoch d'origine. |
| PoTR failures detectees | 28 / 77 (36.4%) | La detection se declenche quand un noeud rate >=2 fenetres PoTR, laissant la plupart des evenements dans le registre de risques residuels. |
| PoTR mean detection latency | 2.0 epochs | Correspond au seuil de lateness a deux epochs integre dans l'escalation d'archivage. |
| Repair queue peak | 38 manifests | Le backlog monte quand les partitions s'empilent plus vite que les quatre reparations disponibles par epoch. |
| Response latency p95 | 30,068 ms | Reflete la fenetre de challenge 30 s avec jitter de +/-75 ms applique au sampling QoS. |
<!-- END_DA_SIM_TABLE -->

Ces sorties alimentent maintenant les prototypes de dashboard DA et satisfont
les criteres d'acceptation "simulation harness + QoS modelling" references dans
le roadmap.

L'automatisation vit derriere
`cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`,
qui appelle le harness partage et emet Norito JSON vers
`artifacts/da/threat_model_report.json` par defaut. Les jobs nocturnes
consomment ce fichier pour rafraichir les matrices dans ce document et alerter
sur la derive des taux de detection, des queues de reparation ou des samples QoS.

Pour rafraichir la table ci-dessus pour les docs, executer `make docs-da-threat-model`,
qui invoque `cargo xtask da-threat-model-report`, regenere
`docs/source/da/_generated/threat_model_report.json`, et reecrit cette section
via `scripts/docs/render_da_threat_model_tables.py`. Le miroir `docs/portal`
(`docs/portal/docs/da/threat-model.md`) est mis a jour dans le meme passage pour
que les deux copies restent en sync.

---
lang: fr
direction: ltr
source: docs/portal/docs/da/threat-model.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Fuente canonica
Refleja `docs/source/da/threat_model.md`. Mantenga ambas versions fr
:::

# Modèle d'informations sur la disponibilité des données de Sora Nexus

_Révision Ultima : 2026-01-19 -- Révision Proxima programmée : 2026-04-19_

Cadencia de mantenimiento : Groupe de travail sur la disponibilité des données (<=90 jours). Cada
la révision doit apparaître en `status.md` avec des liens vers des tickets d'atténuation activés
et des artefacts de simulation.

## Proposition et opportunité

Le programme de disponibilité des données (DA) maintient les transmissions Taikai, les blobs de
voie Nexus et artefactos de gobernanza récupérables ante fallas bizantinas,
de rouge et des opérateurs. Ce modèle de mesures et le travail d'ingénierie
pour DA-1 (architecture et modèle d'aménagement) et servir de référence pour les tâches
DA postérieur (DA-2 et DA-10).

Composants à l'intérieur de l'échelle :
- Extension de l'ingesta DA en Torii et des rédacteurs de métadonnées Norito.
- Arbres de stockage de blobs respaldados por SoraFS (niveaux chaud/froid) et
  politique de réplication.
- Compromisos de bloques Nexus (formats de fil, preuves, API de client léger).
- Hooks de mise en application PDP/PoTR spécifiques aux charges utiles DA.
- Flux d'opérateurs (épinglage, expulsion, slashing) et pipelines de
  observabilité.
- Aprobaciones de gobernanza que admisen o expulsan operadores y contenido DA.Fuera de alcance para ce documento:
- Modèle économique complet (capturé dans le flux de travail DA-7).
- Base de protocoles de SoraFS et cubes pour le modèle de précautions de SoraFS.
- Ergonomie du SDK des clients en plus des considérations de surface de
  amenaza.

## Panorama architectural1. **Envoi :** Les clients envient les blobs via l'API d'ingesta DA de Torii. El
   nodo trocea blobs, codifica manifeste Norito (type de blob, voie, époque,
   flags de codec), et des morceaux stockés dans le niveau chaud de SoraFS.
2. **Annonce :** Intentions de pin et conseils de réplication se propageant aux fournisseurs
   de stockage via le registre (marché SoraFS) avec des balises politiques
   qui indique les objets de rétention chaud/froid.
3. **Compromis :** Les secuenciadors Nexus incluent des compromis de blob (CID +
   Roots KZG (opionales) et le bloc canonique. Clients légers dépendants du
   hash de compromiso et les métadonnées annoncées pour vérifier la disponibilité.
4. **Réplication :** Nodos de almacenamiento descargan share/morceaux attribués,
   satisfaire les spécifications PDP/PoTR et promulguer des données entre niveaux chauds et froids
   politique.
5. **Récupération :** Les consommateurs récupèrent les données via SoraFS ou les passerelles compatibles DA,
   vérifier les preuves et demander des réparations en cas de disparition
   répliques.
6. **Gobernanza:** Parlamento y el comite de supervision DA aprueban operadores,
   horaires de location et escalades d'exécution. Artefacts de gobernanza se
   Almacenan por la misma ruta DA pour garantir la transparence du processus.

## Actifs et responsablesEscala de impacto : **Critico** rompt la sécurité/vivabilité du grand livre ; **Haut**
bloquea backfill DA ou clients ; **Modéré** dégradation de la qualité mais es
récupérable; **Bajo** effet limité.

| Actif | Description | Intégrité | Disponibilité | Confidentialité | Responsable |
| --- | --- | --- | --- | --- | --- |
| Blobs DA (morceaux + manifestes) | Blobs Taikai, voie et gouvernement occupés en SoraFS | Critique | Critique | Modéré | DA WG / Equipe Stockage |
| Manifestes Norito DA | Type de métadonnées décrivant les blobs | Critique | Alto | Modéré | GT sur le protocole principal |
| Compromis de blocage | CIDs + racines KZG dans les blocs Nexus | Critique | Alto | Bas | GT sur le protocole principal |
| Horaires PDP/PoTR | Cadence de mise en application pour les répliques DA | Alto | Alto | Bas | Équipe de stockage |
| Registre des opérateurs | Fournisseurs de placements agréés et politiques | Alto | Alto | Bas | Conseil de gouvernance |
| Registres de location et d'incitations | Registre des inscriptions pour les locations et pénalités DA | Alto | Modéré | Bas | GT Trésorerie |
| Tableaux de bord d'observabilité | SLO DA, profondeur de réplication, alertes | Modéré | Alto | Bas | SRE / Observabilité |
| Intentions de réparation | Sollicitudes para rehidratar chunks faltantes | Modéré | Modéré | Bas | Équipe de stockage |

## Adversaires et capacités| Acteur | Capacités | Motivations | Notes |
| --- | --- | --- | --- |
| Client malveillant | Envoyer des blobs malformés, rejouer des manifestes obsolètes, tenter un DoS en ingérant. | Interrumpir diffuse Taikai, inyectar datos invalidos. | Sin claves privilegiadas. |
| Noeud de stockage bizantino | Soltar répliques assignées, forjar proofs PDP/PoTR, coludir con otros. | Réduisez la rétention DA, évitez la location, retenez les données comme les rehens. | Posséder des informations d'identification valides pour l'opérateur. |
| Secrétaire compromis | Omettre les compromis, éviter les blocages, réorganiser les métadonnées des blobs. | Ocultar envoie DA, crée une incohérence. | Limité par la majorité du consensus. |
| Opérateur interne | Abuser de l'accès au gouvernement, manipuler les politiques de rétention, filtrer les créances. | Ganancia Economica, sabotage. | Accès à une infrastructure chaud/froid. |
| Adversaire de rouge | Particionar nodos, demorar réplicacion, inyectar trafico MITM. | Réduire la disponibilité, dégrader les SLO. | Je ne peux pas rompre TLS mais je peux soltar/ralentizar liens. |
| Atacante d'observabilité | Tableaux de bord/alertes manipulables, suppression des incidents. | Ocultar caidas DA. | Nécessite un accès au pipeline de télémétrie. |

## Frontières de confiance- **Frontera de ingreso:** Cliente a extension DA de Torii. Exiger une authentification pour
  demande, limitation de débit et validation de la charge utile.
- **Frontera de réplication :** Nodos de stockage de morceaux intercambiens et
  des preuves. Les nœuds s'autorisent mutuellement mais peuvent être transportés de forme
  bizantine.
- **Frontera del Ledger :** Données de blocage compromises ou stockage
  hors chaîne. Le consensus protège l'intégrité, mais la disponibilité est requise
  application hors chaîne.
- **Frontera de gobernanza:** Décisions de Council/Parliament que aprueban
  opérateurs, présupposés et slashing. Les erreurs ici affectent directement le
  despligue DA.
- **Frontière d'observabilité :** Récupération des métriques/journaux exportés vers
  tableaux de bord/outils d’alerte. La manipulation des pannes occultes ou des attaques.

## Scénarios de mesures et de contrôles

### Ataques sur la route de l'ingestion

** Scénario : ** Un client malveillant envoie des charges utiles Norito malformées ou des blobs
sobredimensionados para agotar recursos o contrabandear metadata invalida.**Contrôles**
- Validation du schéma Norito avec négociation stricte des versions ; rechazar
  drapeaux desconocidos.
- Limitation du débit et authentification du point final d'ingestion Torii.
- Limites de taille de morceau et d'encodage déterminés par le chunker SoraFS.
- Le pipeline d'admission persiste seul se manifeste après avoir coïncidé avec la somme de contrôle de
  intégrité.
- Cache de replay determinista (`ReplayCache`) rastrea ventanas `(voie, époque,
  séquence)`, persiste les marques de haute eau en disco, et rechaza des duplicados/replays
  obsolètes; harnais de propriété et fuzz cubren empreintes digitales divergentes et
  envios fuera de orden. [crates/iroha_core/src/da/replay_cache.rs:1]
  [fuzz/da_replay_cache.rs:1] [crates/iroha_torii/src/da/ingest.rs:1]

**Brechas résiduels**
- Torii ingérer doit enregistrer le cache de relecture dans l'admission et persister les curseurs
  de séquence à travers les reinicios.
- Les schémas Norito DA ont maintenant un harnais fuzz dédié
  (`fuzz/da_ingest_schema.rs`) pour créer des invariants d'encodage/décodage ; Los
  les tableaux de bord de couverture doivent alerter si la cible est enregistrée.

### Rétention pour la retenue de réplication

**Escenario :** Les opérateurs d'exploitation bizantinos acceptent les attributions de
pin pero sueltan chunks, pasando desafios PDP/PoTR via respuestas forjadas o
collusion.**Contrôles**
- Le calendrier des projets PDP/PoTR est étendu aux charges utiles DA avec couverture par
  époque.
- Réplication multi-source avec des ombres de quorum ; El Fetch Orchestrator détecte
  les éclats s'effondrent et disparaissent en réparation.
- Slashing de gobernanza vinculado a proofs fallidas y répliques fallantes.
- Travail de réconciliation automatisé (`cargo xtask da-commitment-reconcile`)
  comparer les reçus d'ingesta avec les compromis DA (SignedBlockWire, `.norito` ou
  JSON), émettre un bundle JSON de preuve pour la gouvernance, et les tickets d'entrée
  des erreurs ou des coïncidences concernant la page Alertmanager par omission/altération.

**Brechas résiduels**
- Le harnais de simulation en `integration_tests/src/da/pdp_potr.rs` (cubierto
  par `integration_tests/tests/da/pdp_potr_simulation.rs`) maintenant ejercita
  Scénarios de colusion et de participation, validant que le calendrier PDP/PoTR est détecté
  comportement bizantino de forme déterministe. Siga extendiendolo junto a
  DA-5 pour couvrir de nouvelles surfaces de preuve.
- La politique d'expulsion du niveau froid nécessite une piste d'audit confirmée pour
  prevenir gouttes encubiertos.

### Manipulation des compromis

** Scénario : ** Le sécurisateur comprometido publica bloque omitiendo ou alterando
des compromis, causant des erreurs de récupération ou des incohérences chez les clients légers.**Contrôles**
- El consenso cruza propuestas de bloques con colas de envio DA; pairs rechazan
  propuestas sin compromisos requeridos.
- Les clients ligeros vérifient les preuves d'inclusion avant les poignées d'exponer de récupération.
- Piste d'audit comparant les reçus d'envoi avec des compromis de blocage.
- Travail de réconciliation automatisé (`cargo xtask da-commitment-reconcile`)
  comparer les reçus d'ingesta avec les compromis DA (SignedBlockWire, `.norito` ou
  JSON), émettre un bundle JSON de preuve pour la gouvernance, et les tickets d'entrée
  des erreurs ou des coïncidences concernant la page Alertmanager par omission/altération.

**Brechas résiduels**
- Cubierto por el job de reconciliacion + hook de Alertmanager ; les paquets de
  Allez-y maintenant pour installer le bundle JSON de preuve de défaut.

### Partition de rouge et censure

**Escenario :** L'adversaire participe au rouge de réplication, évitant ainsi les nodos
obtenir des morceaux assignés ou répondre au défi PDP/PoTR.

**Contrôles**
- Requisitos de provenedores multi-région garantizan sentiers de diversos rouges.
- Les ventanas de desafio incluent la gigue et le repli sur les canaux de réparation futurs
  de bande.
- Tableaux de bord d'observabilité surveillés en profondeur de réplication, sortie de
  desafios et latence de récupération avec les parapluies d'alerte.**Brechas résiduels**
- Simulations de participation à des événements en direct de Taikai ; si vous avez besoin
  tests de trempage.
- La politique de réserve de même bande de réparation n'est pas codifiée.

### Abus interne

** Scénario : ** Opérateur avec accès au registre manipulant les politiques de rétention,
la liste blanche prouve des malicios ou des alertes suprêmes.

**Contrôles**
- Les actions gouvernementales nécessitent des entreprises multipartites et des enregistrements Norito
  notariés.
- Les changements politiques émettent des événements à surveiller et des journaux d'archives.
- Le pipeline d'observabilité des journaux d'application Norito en ajout uniquement avec le chaînage de hachage.
- L'automatisation des révisions d'accès trimestriel
  (`cargo xtask da-privilege-audit`) enregistrer les répertoires de manifeste/relecture
  (mas paths provistos por operadores), marca entradas faltantes/no Directorio/
  accessible en écriture dans le monde entier, et émet un bundle JSON créé pour les tableaux de bord d'administration.

**Brechas résiduels**
- Les preuves d'altération des tableaux de bord nécessitent des instantanés confirmés.

## Registre des risques résiduels| Riesgo | Probabilité | Impact | Responsable | Plan d'atténuation |
| --- | --- | --- | --- | --- |
| Replay des manifestes DA avant d'ouvrir le cache de la séquence DA-2 | Possible | Modéré | GT sur le protocole principal | Implémenter le cache de séquence + validation de nonce en DA-2 ; agréger les tests de régression. |
| Colusion PDP/PoTR quand >f nodos se compromis | Improbable | Alto | Équipe de stockage | Dérivez un nouveau calendrier de rendez-vous avec plusieurs fournisseurs multi-fournisseurs ; valider via un harnais de simulation. |
| Brecha de auditoria en expulsion del tier froid | Possible | Alto | SRE / Equipe Stockage | Adjuntar enregistre les firmados et les reçus en chaîne pour les expulsions ; surveiller via des tableaux de bord. |
| Latence de détection d'omission du secuenciador | Possible | Alto | GT sur le protocole principal | `cargo xtask da-commitment-reconcile` comparaison nocturne des reçus et des compromis (SignedBlockWire/`.norito`/JSON) et page d'accueil des billets avant une erreur ou sans coïncidence. |
| Résilience à la participation aux flux en vivo de Taikai | Possible | Critique | Réseautage TL | Ejecutar exercices de partition; réserver également une bande de réparation ; documenter SOP de basculement. |
| Dérive des privilèges de gouvernement | Improbable | Alto | Conseil de gouvernance | `cargo xtask da-privilege-audit` trimestriel (répertoires manifeste/relecture + chemins supplémentaires) avec entreprise JSON + porte de tableau de bord ; anclar artefactos de auditoria en chaîne. |

## Suivis requis1. Schémas publics Norito de l'ingesta DA et des vecteurs d'exemple (se chargent du DA-2).
2. Enregistrer le cache de relecture dans l'ingesta DA de Torii et conserver les curseurs de
   séquence a traves de reinicios de nodos.
3. **Completado (2026-02-05):** Le faisceau de simulation PDP/PoTR est maintenant lancé
   Scénarios de collusion + participation avec le modèle de QoS du backlog ; ver
   `integration_tests/src/da/pdp_potr.rs` (avec tests fr
   `integration_tests/tests/da/pdp_potr_simulation.rs`) pour la mise en œuvre et
   los curriculum vitae deterministas capturados abajo.
4. **Completado (2026-05-29):** `cargo xtask da-commitment-reconcile` Comparaison
   reçus d'ingesta contre compromisos DA (SignedBlockWire/`.norito`/JSON),
   émet `artifacts/da/commitment_reconciliation.json`, et est connecté à
   Alertmanager/paquetes de gouvernement pour alertes d'omission/falsification
   (`xtask/src/da.rs`).
5. **Terminé (2026-05-29) :** `cargo xtask da-privilege-audit` récupérer la bobine
   de manifest/replay (mas paths provistos por operadores), marca entradas
   faute/pas de répertoire/world-writable, et produire un bundle JSON ferme pour
   tableaux de bord/révisions de gobernanza (`artifacts/da/privilege_audit.json`),
   cerrando la brecha de automatización d’acceso.

**Donde mirar après:**- Le cache de relecture et la persistance des curseurs sont terrifiés sur DA-2. Voir la
  implémentation en `crates/iroha_core/src/da/replay_cache.rs` (logique de cache)
  et l'intégration Torii en `crates/iroha_torii/src/da/ingest.rs`, qui enhebra las
  vérifications des empreintes digitales à travers le `/v2/da/ingest`.
- Les simulations de streaming PDP/PoTR sont exécutées via le faisceau proof-stream
  en `crates/sorafs_car/tests/sorafs_cli.rs`, cubriendo flux de sollicitude
  PoR/PDP/PoTR et scénarios de chute animés dans le modèle de mesures.
- Les résultats de capacité et de réparation trempent viven en
  `docs/source/sorafs/reports/sf2c_capacity_soak.md`, pendant que la matrice de
  tremper Sumeragi mas amplia se rastrea en `docs/source/sumeragi_soak_matrix.md`
  (avec variantes localisées). Ces artefacts capturent les forets de grande taille
  durée référencée dans le registre des risques résiduels.
- La automatización de reconciliacion + privilège-audit vive en
  `docs/automation/da/README.md` et les nouvelles commandes
  `cargo xtask da-commitment-reconcile`/`cargo xtask da-privilege-audit` ; utiliser
  les sorties par défaut sont inférieures à `artifacts/da/` en complément des preuves des paquets
  de gobernanza.

## Preuve de simulation et modèle de QoS (2026-02)

Pour vérifier le suivi DA-1 #3, nous codifions un harnais de simulation PDP/PoTR

déterministe bas `integration_tests/src/da/pdp_potr.rs` (cubierto por
`integration_tests/tests/da/pdp_potr_simulation.rs`). Le harnais assigné des nœudsdans trois régions, des participations/colusions segun las probabilidades del
feuille de route, rastrea tardanza PoTR, et alimenta un modèle de backlog de réparation
que reflète le présupposé de réparation du niveau chaud. Exécuter le scénario
par défaut (12 époques, 18 desafios PDP + 2 fenêtres PoTR par époque) produit
les mesures suivantes :

<!-- BEGIN_DA_SIM_TABLE -->
<!-- AUTO-GENERATED by scripts/docs/render_da_threat_model_tables.py; do not edit manually. -->
| Métrique | Valeur | Notes |
| --- | --- | --- |
| Erreurs PDP détectées | 48 / 49 (98,0%) | Les partitions ont une détection disparate ; une seule erreur n'a pas été détectée, mais il y a une gigue honnête. |
| Latence des médias de détection PDP | 0,0 époques | Les chutes apparaissent à l’intérieur de l’époque d’origine. |
| Erreurs PoTR détectées | 28 / 77 (36,4%) | La détection est activée lorsqu'un nœud est >=2 fenêtres PoTR, plaçant la plupart des événements dans le registre des risques résiduels. |
| Latence des médias de détection PoTR | époques 2.0 | Coïncide avec l'ombre de retard de deux époques incorporée dans l'escalade des archives. |
| Pico de cola de réparation | 38 manifestes | L’arriéré disparaît lorsque les partitions s’accumulent plus rapidement que les quatre réparations disponibles à l’époque. |
| Latence de réponse p95 | 30 068 ms | Réfléchissez à la fenêtre de défilement de 30 s avec une gigue de +/-75 ms appliquée pour améliorer la QoS. |
<!-- END_DA_SIM_TABLE -->Ces résultats sont maintenant disponibles pour alimenter les prototypes de tableaux de bord DA et satisfaisants
les critères d'acceptation de "faisceau de simulation + modélisation QoS" référencés

dans la feuille de route.

L'automatisation maintenant vive les inconvénients de
`cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`,
qui appelle le harnais partagé et émet Norito JSON a
`artifacts/da/threat_model_report.json` par défaut. Emplois nocturnes consommés
este archivo para refrescar les matrices dans ce documento y alertar ante deriva

et les tâches de détection, de réparation ou de QoS.

Pour rafraîchir la table d'arrivée pour les documents, éjectez `make docs-da-threat-model`,
qui invoque `cargo xtask da-threat-model-report`, régénérera
`docs/source/da/_generated/threat_model_report.json`, et réécrire cette section
via `scripts/docs/render_da_threat_model_tables.py`. Le miroir `docs/portal`
(`docs/portal/docs/da/threat-model.md`) se met à jour en même temps que

les copies sont synchronisées.
---
lang: fr
direction: ltr
source: docs/portal/docs/da/threat-model.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Fonte canonica
Espelha `docs/source/da/threat_model.md`. Mantenha comme duas versoes em
:::

# Modèle de disponibilité des données de Sora Nexus

_Ultime révision : 2026-01-19 -- Proxima révision programmée : 2026-04-19_

Cadencia de manutencao : Groupe de travail sur la disponibilité des données (<=90 jours). Chaque révision

devez apparaître sur `status.md` avec des liens pour les tickets d'atténuation ativos et
artefatos de simulacao.

## Proposé et écouté

Le programme de disponibilité des données (DA) transmet le Taikai, les blobs de lane
Nexus et les artéfacts de gouvernance récupérés sur les fausses affaires, de rede et de
opérateurs. Ce modèle d'amis ancre le travail d'ingénierie pour DA-1
(architecture et modèle d'ameublement) et sert de base de référence pour les tarefas DA
postérieures (DA-2 et DA-10).

Composants non compris :
- Extension d'ingestao DA pour Torii et rédacteurs de métadonnées Norito.
- Arvores d'armement de blobs pris en charge par SoraFS (niveaux chaud/froid) et
  politique de réplication.
- Engagements de blocos Nexus (formats de fils, preuves, API de niveau client).
- Hooks de mise en application PDP/PoTR spécifiques aux charges utiles DA.
- Workflows des opérateurs (épinglage, expulsion, slashing) et pipelines de
  observabilité.
- Approbations de gouvernance qui admettent ou suppriment les opérateurs et le contenu DA.Forums pour consulter ce document :
- Modelagem Economica Completa (capturada no workstream DA-7).
- Base de protocoles SoraFS et couverture du modèle d'amis SoraFS.
- Ergonomie du SDK du client toujours pris en compte par la surface de l'amie.

## Visao architectural1. **Soumission :** Les clients submettent des blobs via une API d'acquisition DA par Torii. Ô
   nœud diviser les blobs, codifica manifeste Norito (type de blob, voie, époque, drapeaux
   de codec), et des morceaux d'armazena sans niveau chaud font SoraFS.
2. **Annonce :** Intentions d'épinglage et conseils de propagande de réplication pour les fournisseurs de
   stockage via le registre (marché SoraFS) avec des balises politiques indiquant
   métas de rétention chaud/froid.
3. **Engagement :** Les séquenceurs Nexus incluent des engagements de blobs (CID + racines
   KZG opcionais) pas de bloc canonique. Les clients dépendent du hash de
   engagement et les métadonnées annoncées pour vérifier la disponibilité.
4. **Réplication :** Nodos de armazenamento puxam share/chunks attribuidos, atendem
   desafios PDP/PoTR, et promovem dados entre niveaux chauds et froids conformes à la politique.
5. **Récupérer :** Les consommateurs prennent en charge les données de caméra via SoraFS ou les passerelles compatibles DA,
   vérifier les preuves et émettre des procédures de réparation lorsque les répliques disparaissent.
6. **Gouvernance :** Parlamento et o comite de supervisao DA aprovam operadores,
   plannings de loyer et escalades d'exécution. Artefatos de gouvernance sao
   armazenados pela mesma rota DA pour garantir la transparence du processus.

## Actifs et responsables

Escala de impacto : **Critico** quebra seguranca/vivacidade do ledger ; **Alto**bloqueia backfill DA ou clients; **Modéré** dégradation de la qualité plus permanente
récupération; **Baixo** effet limité.

| Ativo | Description | Intégration | Disponibilité | Confidentialité | Responsavel |
| --- | --- | --- | --- | --- | --- |
| Blobs DA (morceaux + manifestes) | Blobs Taikai, voie et gouvernement armazenados em SoraFS | Critique | Critique | Modéré | DA WG / Equipe Stockage |
| Manifestes Norito DA | Types de métadonnées décrivant les blobs | Critique | Alto | Modéré | GT sur le protocole principal |
| Engagements de bloc | CIDs + racines KZG dans les blocs Nexus | Critique | Alto | Bas | GT sur le protocole principal |
| Horaires PDP/PoTR | Cadence de mise en application pour les répliques DA | Alto | Alto | Bas | Équipe de stockage |
| Registre des opérateurs | Fournisseurs de stockage agréés et politiques | Alto | Alto | Bas | Conseil de gouvernance |
| Registres de loyer et d'incitations | Registre des inscriptions pour le loyer DA et pénalités | Alto | Modéré | Bas | GT Trésorerie |
| Tableaux de bord d'observation | SLOs DA, profondeur de réplication, alertes | Modéré | Alto | Bas | SRE / Observabilité |
| Intentions de réparation | Pedidos para reidratar chunks ausentes | Modéré | Modéré | Bas | Équipe de stockage |

## Adversaires et capacités| Ator | Capacités | Motivacos | Notes |
| --- | --- | --- | --- |
| Client malveillant | Submeter blobs malformados, replay des manifestes antigos, tentar DoS no ingest. | Interromper diffuse Taikai, injetar dados invalidos. | Sem chaves privilegiadas. |
| Noeud d'armement bizantino | Drop de répliques attribuées, forjar proofs PDP/PoTR, coludir. | Reduzir retencao DA, evitar rent, reter dados. | Possui credenciais validas de operador. |
| Séquenceur compromis | Omettre les engagements, équivoquer les blocs, réorganiser les métadonnées des blobs. | Ocultar submissao DA, criar incohérence. | Limité à la majorité du consensus. |
| Opérateur interne | Abuser de l'accès à la gouvernance, manipuler la politique de rétention, vazar credenciais. | Ganho Economico, sabotagem. | Accès à une infrastructure chaud/froid. |
| Adversaire de rede | Partitionner les nœuds, atrasar réplicacao, injetar trafego MITM. | Réduire la disponibilité, dégrader les SLO. | Nao quebra TLS peut également déposer des liens/atrasar. |
| Atacante d'observabilité | Tableaux de bord/alertes manipulables, suppression des incidents. | Pannes occultaires DA. | Demander l'accès au pipeline de télémétrie. |

## Fronteiras de confiance- **Frontière d'entrée :** Client pour l'extension DA du Torii. Demander l'authentification
  demande, limitation de débit et validation de la charge utile.
- **Fronteira de replicacao:** Nodos de stockage de morceaux de trocam et d'épreuves. Nœuds OS
  se autenticam mutuamente mas podem se comportar de forma bizantina.
- **Fronteira do ledger :** Dados de bloco commitados vs stockage off-chain.
  Consenso garantit l'intégrité, mais la disponibilité nécessite une application hors chaîne.
- **Fronteira de gouvernement:** Décisoes Conseil/Parlement aprovando operadores,
  orcamentos e slashing. Falhas ici impacte directement le déploiement du DA.
- **Frontière d'observation :** Coleta de metrics/logs exportada para
  tableaux de bord/outils d’alerte. Falsification pour éviter les pannes ou les attaques.

## Scénarios d'ameaca et de contrôles

### Ataques sur le chemin de l'absorption

**Cénario :** Un client malveillant envoie des charges utiles Norito malformados ou blobs
superdimensionados para exaurir recursos ou insérer des métadonnées invalides.**Contrôles**
- Validation du schéma Norito avec négociation estrita de versao ; rejeter les drapeaux
  desconhecidos.
- Limitation du débit et authentification sans point final d'absorption Torii.
- Limites de la taille du chunk et du codage déterminant pour le chunker SoraFS.
- Pipeline d'admission donc persiste manifeste après somme de contrôle d'intégrité
  coïncider.
- Replay cache determinista (`ReplayCache`) rastreia janelas `(voie, époque,
  séquence)`, persiste les marques de haute eau dans la discothèque, et rejette les duplicados/replays
  obsolètes; harnais de propriété et fuzz cobrem empreintes digitales divergentes e
  envoyer des forums de commande. [crates/iroha_core/src/da/replay_cache.rs:1]
  [fuzz/da_replay_cache.rs:1] [crates/iroha_torii/src/da/ingest.rs:1]

** Lacunes résiduelles **
- Torii ingérer le développement ou rejouer le cache pour l'admission et la persistance des curseurs
  séquence durante reinicios.
- Schémas Norito DA maintenant possuem un harnais fuzz dédié
  (`fuzz/da_ingest_schema.rs`) pour gérer les invariants d'encodage/décodage ; système d'exploitation
  les tableaux de bord de couverture développent des alertes pour l'enregistrement de la cible.

### Retenue de retenue de réplication

**Cénario :** Les opérateurs de stockage bizantinos aceitam pins mas dropam chunks,
passer desafios PDP/PoTR via des réponses forjadas ou colusao.**Contrôles**
- Le calendrier des projets PDP/PoTR s'étend aux charges utiles DA avec couverture par
  époque.
- Réplication des seuils de quorum multi-sources ; o les fragments de détection de l'orchestrateur
  faltantes e dispara reparo.
- Slashing de gouvernance vinculado a proofs falhas e répliques fallantes.
- Travail de réconciliation automatisé (`cargo xtask da-commitment-reconcile`) que
  comparer les recettes d'ingestao avec les engagements DA (SignedBlockWire, `.norito` ou
  JSON), émettre un bundle JSON de preuves pour la gouvernance, et falha em tickets
  des erreurs ou des divergences concernant la page Alertmanager en cas d'omission/falsification.

** Lacunes résiduelles **
- Le harnais de simulation em `integration_tests/src/da/pdp_potr.rs` (coberto por
  `integration_tests/tests/da/pdp_potr_simulation.rs`) exercice colusao e
  particulier, validant que le planning PDP/PoTR détecte un comportement bizantino
  de forme déterministe. Continuer estendendo junto com DA-5 para cobrir novas
  superficie de preuve.
- Une politique d'expulsion à froid demande une piste d'audit assassinée pour éviter les gouttes
  encobertos.

### Manipulation des engagements

**Scénario :** Le séquenceur comprometido publica blocos omitindo ou alterando
engagements DA, causando falhas de fetch ou incohérences em clientses niveaux.**Contrôles**
- Consenso cruza propositionstas de bloco com filas de soumissao DA ; pairs rejeitam
  proposetas sem engagements requeridos.
- Les clients doivent vérifier les preuves d'inclusion avant l'exportation et les poignées de récupération.
- Piste d'audit comparant les reçus de soumission avec les engagements de bloco.
- Travail de réconciliation automatisé (`cargo xtask da-commitment-reconcile`) que
  comparer les recettes d'ingestao avec les engagements DA (SignedBlockWire, `.norito` ou
  JSON), émettre un bundle JSON de preuves pour la gouvernance, et falha em tickets
  des erreurs ou des divergences concernant la page Alertmanager en cas d'omission/falsification.

** Lacunes résiduelles **
- Coberto pelo job de reconciliacao + crochet Alertmanager ; paquets de gouvernance
  J'ai maintenant le bundle JSON de preuve par défaut.

### Partie de rede et de censure

**Cénario :** L'adversaire participe au réseau de réplication, empêchant les nœuds de
obtenir des morceaux attribués ou répondre à desafios PDP/PoTR.

**Contrôles**
- Exigences des fournisseurs multirégionaux garantissant des chemins de câbles divers.
- Les paramètres de désafio incluent la gigue et le repli pour les canaux de réparation du forum
  bande.
- Tableaux de bord d'observation, surveillance profonde de réplication, succès de
  desafios et latence de récupération des seuils d’alerte.** Lacunes résiduelles **
- Simulateurs de participation pour les événements Taikai live ainda faltam ; sao nécessaire
  tests de trempage.
- La politique de réserve de bande de réparation est également codifiée.

### Abus interne

**Cénario :** L'opérateur ayant accès au registre manipule les politiques de rétention,
liste blanche des fournisseurs malveillants, ou alertes suprêmes.

**Contrôles**
- Ordres de gouvernement exigeant des assassinats multipartites et registres Norito
  notariés.
- Les événements politiques émettent des événements pour la surveillance et les journaux d'archives.
- Pipeline d'observation des journaux d'applications Norito chaînage de hachage avec ajout uniquement.
- Un automate de révision trimestriel (`cargo xtask da-privilege-audit`) percorre
  diretorios de manifest/replay (mais paths fornecidos por operadores), marque
  entrées fausses/directement/world-writable, et envoi d'un bundle JSON assassiné
  para tableaux de bord de gouvernance.

** Lacunes résiduelles **
- Les preuves de falsification des tableaux de bord nécessitent des instantanés assassinés.

## Registre des risques résiduels| Risco | Probabilité | Impact | Propriétaire | Plan d'atténuation |
| --- | --- | --- | --- | --- |
| Replay des manifestes DA avant le cache de séquence DA-2 | Possif | Modéré | GT sur le protocole principal | Implémenter le cache de séquence + validation occasionnelle dans DA-2 ; ajouter des testicules de régression. |
| Colusao PDP/PoTR quando >f nodes sao comprometidos | Amélioration | Alto | Équipe de stockage | Dérivez un nouveau calendrier de desafios avec un échantillonnage multi-fournisseur ; valider via le harnais de simulation. |
| Écart de salles d'expulsion à froid | Possif | Alto | SRE / Equipe Stockage | Anexar enregistre les assassins et les reçus en chaîne pour les expulsions ; surveiller via des tableaux de bord. |
| Latence de détection d'omission du séquenceur | Possif | Alto | GT sur le protocole principal | `cargo xtask da-commitment-reconcile` indique la comparaison des recettes et des engagements (SignedBlockWire/`.norito`/JSON) et la page régit les tickets erronés ou divergents. |
| Résistance à la participation pour les flux Taikai en direct | Possif | Critique | Réseautage TL | Exécuter des exercices de particao ; réserver une bande de réparation; documenter SOP de basculement. |
| Dérive des privilèges de gouvernance | Amélioration | Alto | Conseil de gouvernance | `cargo xtask da-privilege-audit` trimestral (répertoires manifeste/replay + chemins extras) avec JSON associé + porte de tableau de bord ; ancorar artefatos de auditoria en chaîne. |

## Suivis requis1. Schémas publics Norito de l'ingestao DA et des exemples d'exemple (chargés dans
   DA-2).
2. Encadrer ou rejouer le cache pour ingérer DA par Torii et conserver les curseurs de
   séquence durante reinicios de nodes.
3. **Concluido (2026-02-05):** Le harnais de simulation PDP/PoTR il y a un exercice
   colusao + particao avec la modélisation de la qualité de service du backlog ; voir
   `integration_tests/src/da/pdp_potr.rs` (com teste les
   `integration_tests/tests/da/pdp_potr_simulation.rs`) pour la mise en œuvre
   Resumos Deterministas Capturados Abaixo.
4. **Concluido (2026-05-29) :** Comparaison `cargo xtask da-commitment-reconcile`
   reçus d'ingestao avec engagements DA (SignedBlockWire/`.norito`/JSON),
   émet `artifacts/da/commitment_reconciliation.json`, et est lié à
   Alertmanager/pacotes de gouvernance pour alertes d'omission/falsification
   (`xtask/src/da.rs`).
5. **Concluido (2026-05-29) :** `cargo xtask da-privilege-audit` percer la bobine
   de manifest/replay (mais paths fornecidos por operadores), marca entradas
   Faltantes/nao diretorio/world-writable, et produit bundle JSON assinado para
   tableaux de bord/révisions de gouvernance (`artifacts/da/privilege_audit.json`),
   fechando a lacuna de automacao de acesso.

**Onde olhar a seguir:**- Le cache de relecture et la persistance des curseurs sur le DA-2. Veja un
  implémentation dans `crates/iroha_core/src/da/replay_cache.rs` (logique du cache)
  et une intégration Torii dans `crates/iroha_torii/src/da/ingest.rs`, qui enchaîne les vérifications de
  empreinte digitale via `/v1/da/ingest`.
- En tant que simulations de streaming PDP/PoTR à partir d'exercices via ou Harness Proof-Stream
  dans `crates/sorafs_car/tests/sorafs_cli.rs`, cobrindo fluxos de requisicao
  PoR/PDP/PoTR et scénarios de faux animés sur le modèle des amis.
- Résultats de capacité et réparation tremper vivem em
  `docs/source/sorafs/reports/sf2c_capacity_soak.md`, concerne la matrice de trempage
  Sumeragi plus ample et accompagné par `docs/source/sumeragi_soak_matrix.md`
  (variantes localisées incluses). Esses artefatos capturent les forets de longa
  duracao referenciados no registro de riscos residuais.
- A automacao de reconciliação + privilège-audit vive em
  `docs/automation/da/README.md` et nos commandes
  `cargo xtask da-commitment-reconcile`/`cargo xtask da-privilege-audit` ; utiliser
  comme l'a dit padrao sob `artifacts/da/` à l'annexe des preuves des paquets de
  gouvernance.

## Preuves de simulation et de modélisation QoS (2026-02)Pour rechercher le suivi DA-1 #3, nous codifions un harnais de simulation PDP/PoTR
déterministe sanglot `integration_tests/src/da/pdp_potr.rs` (coberto por
`integration_tests/tests/da/pdp_potr_simulation.rs`). O exploiter les nœuds aloca em
tres regioes, injeta particoes/colusao se conforment aux probabilités de la feuille de route,
accompagner le retard PoTR, et alimenter un modèle de retard de réparation qui reflète le
orcamento de reparo do tier hot. Rodar o cenario default (12 époques, 18 desafios
PDP + 2 janvier PoTR par époque) produit comme suit metricas :

<!-- BEGIN_DA_SIM_TABLE -->
<!-- AUTO-GENERATED by scripts/docs/render_da_threat_model_tables.py; do not edit manually. -->
| Métrique | Valeur | Notes |
| --- | --- | --- |
| Falhas PDP détectés | 48 / 49 (98,0%) | Particoes ainda disparam deteccao; une erreur n'a pas été détectée avec une gigue honnête. |
| Latence des médias de détection PDP | 0,0 époques | Les Falhas surgissent à l’époque de l’origine. |
| Falhas PoTR détectés | 28 / 77 (36,4%) | La détection disparaît lorsqu'un nœud perd >=2 janvier PoTR, et la plupart des événements ne sont pas enregistrés dans le registre des risques résiduels. |
| Latence des médias de détection PoTR | époques 2.0 | Correspond au limite de l'atraso de deux époques embutido na escalacao de arquivo. |
| Pico de fila de réparation | 38 manifestes | L’arriéré augmente lorsque les participants s’emparent plus rapidement que quatre réparations disponibles pour l’époque. |
| Latence de réponse p95 | 30 068 ms | Reflète une période de désafio de 30 s avec une gigue de +/-75 ms appliquée sans QoS d'échantillonnage. |
<!-- END_DA_SIM_TABLE -->Les résultats d'essais alimentent les prototypes du tableau de bord DA et les satisfont
critères de certification de "faisceau de simulation + modélisation QoS" référencés
pas de feuille de route.

Un automacao agora vive por tras de
`cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`,
que le chama ou le harnais soit partagé et émis Norito JSON pour
`artifacts/da/threat_model_report.json` par défaut. Jobs noturnos consomem este
archiver pour actualiser les matrices dans ce document et alerter sur leur dérivé

taxas de détection, fils de réparation ou échantillons QoS.

Pour actualiser le tableau acima pour les documents, exécutez `make docs-da-threat-model`,
qui invoque `cargo xtask da-threat-model-report`, régénérera
`docs/source/da/_generated/threat_model_report.json`, et réenregistrer cette secao via
`scripts/docs/render_da_threat_model_tables.py`. L'espoir `docs/portal`
(`docs/portal/docs/da/threat-model.md`) et actualisé sans même passer pour cela
comme duas copias fiquem em sync.
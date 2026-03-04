---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-tuning.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : réglage de l'orchestrateur
titre : Déploiement et réglage de l’orchestrateur
sidebar_label : Réglage de l'orchestrateur
description : Valeurs par défaut pratiques, conseils d’ajustement et points d’audit pour amener l’orchestrateur multi-source en GA.
---

:::note Source canonique
Reflète `docs/source/sorafs/developer/orchestrator_tuning.md`. Gardez les deux copies alignées jusqu’à ce que la documentation héritée soit retirée.
:::

# Guide de déploiement et de réglage de l’orchestrateur

Ce guide s'appuie sur la [référence de configuration](orchestrator-config.md) et le
[runbook de déploiement multi-source](multi-source-rollout.md). Il explique
comment ajuster l’orchestrateur pour chaque phase de déploiement, comment interpréter
les artefacts du scoreboard et quels signaux de télémétrie doivent être en place
avant d’élargir le trafic. Appliquez ces recommandations de manière cohérente dans
la CLI, les SDK et l’automatisation afin que chaque nœud suive la même politique de
chercher déterministe.

## 1. Jeux de paramètres de base

Partez d’un modèle de configuration partagé et ajustez un petit ensemble de réglages
au fur et à mesure du déploiement. Le tableau ci-dessous capture les valeurs
recommandées pour les phases les plus courantes ; les valeurs non listées retombent
sur les valeurs par défaut de `OrchestratorConfig::default()` et `FetchOptions::default()`.

| Phases | `max_providers` | `fetch.per_chunk_retry_limit` | `fetch.provider_failure_threshold` | `scoreboard.latency_cap_ms` | `scoreboard.telemetry_grace_secs` | Remarques |
|-------|-----------------|-------------------------------|------------------------------------|-----------------------------|-----------------------------------------|-------|
| **Laboratoire / CI** | `3` | `2` | `2` | `2500` | `300` | Un plafond de latence serré et une fenêtre de grâce courte mettent rapidement en évidence une télémétrie bruyante. Gardez des tentatives bas pour révéler les manifestes invalides plus tôt. |
| **Mise en scène** | `4` | `3` | `3` | `4000` | `600` | Reflète les valeurs de production tout en laissant de la marge pour des pairs exploratoires. |
| **Canari** | `6` | `3` | `3` | `5000` | `900` | Correspond aux valeurs par défaut ; défini `telemetry_region` pour permettre aux tableaux de bord de pivoter sur le trafic canari. |
| **Disponibilité générale** | `None` (utiliser tous les éligibles) | `4` | `4` | `5000` | `900` | Augmentez les seuils de tentative et d’échec pour absorber les fautes transitoires tout en conservant le déterminisme via l’audit. |

- `scoreboard.weight_scale` reste sur la valeur par défaut `10_000` sauf si un système aval nécessite une autre résolution entière. Augmenter l’échelle ne change pas l’ordre des fournisseurs ; cela produit simplement une distribution de crédits plus dense.
- Lors du passage d’une phase à l’autre, persistez le bundle JSON et utilisez `--scoreboard-out` pour que la piste d’audit enregistre le jeu de paramètres exacts.

## 2. Hygiène du tableau de bordLe tableau de bord combine les exigences du manifeste, les annonces de fournisseurs et la télémétrie.
Avant d'avancer :

1. **Valider la fraîcheur de la télémétrie.** Assurez-vous que les instantanés référencés par
   `--telemetry-json` ont été capturés dans la fenêtre de grâce configurée. Les entrées
   plus anciennes que `telemetry_grace_secs` échouent avec `TelemetryStale { last_updated }`.
   Traitez cela comme un arrêt dur et rafraîchissez l’export de télémétrie avant de continuer.
2. **Inspecter les raisons d’éligibilité.** Persistez les artefacts via
   `--scoreboard-out=/var/lib/sorafs/scoreboards/preflight.json`. Chaque entrée
   transportez un bloc `eligibility` avec la cause exacte de l’échec. Ne contournez
   pas les écarts de capacité ou les annonces expirées ; corrigez le montant de la charge utile.
3. **Revoir les écarts de poids.** Comparez le champ `normalised_weight` avec la
   version précédente. Des variations >10% correspondent aux changements
   volontaires d’annonces ou de télémétrie et être consignées dans le journal de déploiement.
4. **Archiver les artefacts.** Configurez `scoreboard.persist_path` pour que chaque exécution
   Émettez le snapshot final du tableau de bord. Attachez l’artefact au dossier de release
   avec le manifeste et le bundle de télémétrie.
5. **Consigner la preuve du mix fournisseurs.** La métadonnée de `scoreboard.json` _et_ le
   `summary.json` correspondant doit exposer `provider_count`,
   `gateway_provider_count` et l’étiquette dérivée `provider_mix` afin que les rélecteurs
   pouvoir prouver si l’exécution était `direct-only`, `gateway-only` ou `mixed`. Les
   la passerelle de capture doit être rapporteur `provider_count=0` plus `provider_mix="gateway-only"`,
   tandis que les exécutions mixtes requièrent des comptes non nuls pour les deux sources.
   `cargo xtask sorafs-adoption-check` impose ces champs (et échoue si les comptes/labels
   divergent), alors exécutez-le toujours avec `ci/check_sorafs_orchestrator_adoption.sh`
   ou votre script de capture afin de produire le bundle d’évidence `adoption_report.json`.
   Lorsque des passerelles Torii sont impliquées, conservez `gateway_manifest_id`/`gateway_manifest_cid`
   dans les métadonnées du scoreboard pour que la porte d’adoption puisse corréler l’enveloppe
   du manifeste avec le mix fournisseurs capturé.

Pour des définitions de champs détaillées, voir
`crates/sorafs_car/src/scoreboard.rs` et la structure de CV CLI exposée par
`sorafs_cli fetch --json-out`.

## Référence des flags CLI et SDK

`sorafs_cli fetch` (voir `crates/sorafs_car/src/bin/sorafs_cli.rs`) et le wrapper
`iroha_cli app sorafs fetch` (`crates/iroha_cli/src/commands/sorafs.rs`) partage la même
surface de configuration de l’orchestrateur. Utilisez les drapeaux suivants lors de la
capture de preuves de déploiement ou pour rejouer les luminaires canoniques :

Référence partagée des flags multi-source (gardez l’aide CLI et les docs synchronisées en
éditant uniquement ce fichier) :- `--max-peers=<count>` limite le nombre de fournisseurs éligibles qui passent le filtre du scoreboard. Laissez vide pour streamer depuis tous les fournisseurs éligibles, mettez `1` uniquement pour exercer volontairement le fallback mono-source. Reflète la clé `maxPeers` dans le SDK (`SorafsGatewayFetchOptions.maxPeers`, `SorafsGatewayFetchOptions.max_peers`).
- `--retry-budget=<count>` transmet la limite de tentatives par chunk appliqué par `FetchOptions`. Utilisez la table de déploiement du guide d’ajustement pour les valeurs recommandées ; les exécutions CLI qui collectent des preuves doivent correspondre aux valeurs par défaut du SDK pour garantir la parité.
- `--telemetry-region=<label>` étiquette les séries Prometheus `sorafs_orchestrator_*` (et les relais OTLP) avec un label de région/environnement afin que les tableaux de bord distinguent lab, staging, canari et GA.
- `--telemetry-json=<path>` injecte le snapshot référencé par le scoreboard. Persistez le JSON à côté du scoreboard pour que les auditeurs puissent rejouer l’exécution (et pour que `cargo xtask sorafs-adoption-check --require-telemetry` prouve quel flux OTLP a alimenté la capture).
- `--local-proxy-*` (`--local-proxy-mode`, `--local-proxy-norito-spool`, `--local-proxy-kaigi-spool`, `--local-proxy-kaigi-policy`) active les crochets de l’observateur bridge. Lorsqu'ils sont définis, l'orchestrateur diffuse les chunks via le proxy Norito/Kaigi local afin que les clients navigateur, guard caches et salles Kaigi reçoivent les mêmes reçus que Rust.
- `--scoreboard-out=<path>` (éventuellement avec `--scoreboard-now=<unix_secs>`) persiste le snapshot d'éligibilité pour les auditeurs. Associez toujours le JSON persisté aux artefacts de télémétrie et de manifeste référencés dans le ticket de release.
- `--deny-provider name=ALIAS` / `--boost-provider name=ALIAS:delta` appliquent des ajustements déterministes au-dessus des métadonnées d’annonces. Utilisez ces flags uniquement pour les répétitions ; les déclassements de production doivent passer par des artefacts de gouvernance pour que chaque nœud applique le même paquet de politique.
- `--provider-metrics-out` / `--chunk-receipts-out` conservant les métriques de santé par fournisseur et les reçus de morceaux référencés par la checklist de déploiement ; attachez les deux artefacts lors du dépôt de la preuve d’adoption.

Exemple (en utilisant le luminaire publié) :

```bash
sorafs_cli fetch \
  --plan fixtures/sorafs_orchestrator/multi_peer_parity_v1/plan.json \
  --gateway-provider gw-alpha=... \
  --telemetry-source-label otlp::staging \
  --scoreboard-out artifacts/sorafs_orchestrator/latest/scoreboard.json \
  --json-out artifacts/sorafs_orchestrator/latest/summary.json \
  --provider-metrics-out artifacts/sorafs_orchestrator/latest/provider_metrics.json \
  --chunk-receipts-out artifacts/sorafs_orchestrator/latest/chunk_receipts.json

cargo xtask sorafs-adoption-check \
  --scoreboard artifacts/sorafs_orchestrator/latest/scoreboard.json \
  --summary artifacts/sorafs_orchestrator/latest/summary.json
```

Les SDK consomment la même configuration via `SorafsGatewayFetchOptions` dans le
client Rust (`crates/iroha/src/client.rs`), les liaisons JS
(`javascript/iroha_js/src/sorafs.js`) et le SDK Swift
(`IrohaSwift/Sources/IrohaSwift/SorafsOptions.swift`). Gardez ces helpers alignés
avec les valeurs par défaut de la CLI afin que les opérateurs puissent copier les
politiques dans l’automatisation sans couches de traduction ad hoc.

## 3. Ajustement de la politique de fetch

`FetchOptions` contrôle les tentatives, la concurrence et la vérification. Lors du
réglage :- **Retries :** augmenter `per_chunk_retry_limit` au-delà de `4` accroît le temps
  de récupération mais de masquer des fautes de risque fournisseurs. Préférez
  garder `4` comme plafond et compter sur la rotation des fournisseurs pour
  exposer les mauvais performers.
- **Seuil d’échec :** `provider_failure_threshold` déterminer quand un fournisseur
  est désactivé pour le reste de la session. Alignez cette valeur sur la politique
  de retries : un seuil inférieur au budget de retries force l’orchestrateur à
  éjecter un pair avant que toutes les tentatives ne soient épuisées.
- **Concurrence :** laissez `global_parallel_limit` non défini (`None`) à moins
  qu’un environnement spécifique ne puisse saturer les plages annoncées. Lorsque
  défini, assurez-vous que la valeur soit ≤ à la somme des budgets de streams des
  fournisseurs afin d’éviter la famine.
- **Toggles de vérification :** `verify_lengths` et `verify_digests` doivent rester
  activés en production. Ils assurent le déterminisme lorsque des flottes
  les mixtes de fournisseurs sont actifs ; ne les désactivez que dans des environnements
  de fuzzing isolés.

## 4. Transport et mise en scène d’anonymat

Utilisez les champs `rollout_phase`, `anonymity_policy` et `transport_policy` pour
représente la posture de confidentialité :

- Préférez `rollout_phase="snnet-5"` et laissez la politique d'anonymat par défaut
  suivez les jalons SNNet-5. Remplacez via `anonymity_policy_override` uniquement
  lorsque la gouvernance émet une directive signée.
- Gardez `transport_policy="soranet-first"` comme base tant que SNNet-4/5/5a/5b/6a/7/8/12/13 sont 🈺
  (voir `roadmap.md`). Utilisez `transport_policy="direct-only"` uniquement pour des
  downgrades documentés ou des exercices de conformité, et attendre la revue de
  couverture PQ avant de promouvoir `transport_policy="soranet-strict"` — ce niveau
  échouera rapidement si seuls des relais classiques subsistent.
- `write_mode="pq-only"` ne doit être appliqué que lorsque chaque chemin d’écriture
  (SDK, orchestrateur, outils de gouvernance) peut répondre aux exigences PQ. Durant
  les rollouts, gardez `write_mode="allow-downgrade"` afin que les réponses d'urgence
  pouvoir s’appuyer sur des routes directes pendant que la télémétrie signale la
  dégradation.
- La sélection des gardes et la préparation des circuits s'appuient sur le
  répertoire SoraNet. Fournissez le snapshot signé de `relay_directory` et
  persistez le cache `guard_set` afin que le churn de guards reste dans la fenêtre
  de rétention convenue. L’empreinte du cache enregistrée par `sorafs_cli fetch`
  fait partie de l’évidence de déploiement.

## 5. Hooks de downgrade et de conformité

Deux sous-systèmes de l’orchestrateur permettent de faire respecter la politique sans
intervention manuelle :- **Remédiation des downgrades** (`downgrade_remediation`) : surveiller les événements
  `handshake_downgrade_total` et, après dépassement du `threshold` configuré dans
  `window_secs`, forcer le proxy local en `target_mode` (par défaut métadonnées uniquement).
  Conservez les valeurs par défaut (`threshold=3`, `window=300`, `cooldown=900`) sauf
  si les post-mortems montrent un autre schéma. Documentez toute dérogation dans le
  journal de déploiement et assurez-vous que les tableaux de bord suivent
  `sorafs_proxy_downgrade_state`.
- **Politique de conformité** (`compliance`) : les carve-outs de juridiction et de
  manifeste passer par les listes d’opt-out gérées par la gouvernance. N'intégrez
  jamais d’overrides ad hoc dans le bundle de configuration ; demandez plutôt une
  mise à jour signée de `governance/compliance/soranet_opt_outs.json` et redéployez
  le JSON généré.

Pour les deux systèmes, persistez le bundle de configuration résultant et incluez-le
aux preuves de release afin que les auditeurs puissent retracer les bascules.

## 6. Télémétrie et tableaux de bord

Avant d’élargir le déploiement, confirmez que les signaux suivants sont actifs dans
l'environnement cible :

- `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` —
  doit être à zéro après la fin du canari.
- `sorafs_orchestrator_retries_total` et
  `sorafs_orchestrator_retry_ratio` — doit se stabiliser sous 10% pendant le
  canari et rester sous 5% après GA.
- `sorafs_orchestrator_policy_events_total` — valide que l'étape de déploiement attendue
  est actif (label `stage`) et enregistre les baisses de tension via `outcome`.
-`sorafs_orchestrator_pq_candidate_ratio`/
  `sorafs_orchestrator_pq_deficit_ratio` — suivez l'offre de relais PQ face aux
  attentes de la politique.
- Cibles de log `telemetry::sorafs.fetch.*` — doivent être envoyés à l'agrégateur
  de logs partagés avec des recherches sauvegardées pour `status=failed`.

Chargez le tableau de bord Grafana canonique depuis
`dashboards/grafana/sorafs_fetch_observability.json` (exporté dans le portail sous
**SoraFS → Fetch Observability**) afin que les sélecteurs région/manifest, la heatmap
des tentatives par fournisseur, les histogrammes de latence des chunks et les compteurs
de blocage correspondant à ce que SRE examine lors des burn-ins. Raccordez les règles
Alertmanager dans `dashboards/alerts/sorafs_fetch_rules.yml` et validez la syntaxe
Prometheus avec `scripts/telemetry/test_sorafs_fetch_alerts.sh` (le helper exécute
`promtool test rules` localement ou via Docker). Les transferts d’alertes exigent le
même bloc de routage que le script imprime afin que les opérateurs puissent joindre
la preuve au ticket de déploiement.

### Workflow de burn-in télémétrie

L’élément de feuille de route **SF-6e** exige un rodage de télémétrie de 30 jours avant de
basculer l’orchestrateur multi-source vers ses valeurs GA. Utilisez les scripts du
référentiel pour capturer un bundle d’artefacts reproductibles chaque jour de la
fenêtre :

1. Exécutez `ci/check_sorafs_orchestrator_adoption.sh` avec les variables de burn-in
   défini. Exemple :

   ```bash
   SORAFS_BURN_IN_LABEL=canary-week-1 \
   SORAFS_BURN_IN_REGION=us-east-1 \
   SORAFS_BURN_IN_MANIFEST=manifest-v4 \
   SORAFS_BURN_IN_DAY=7 \
   SORAFS_BURN_IN_WINDOW_DAYS=30 \
   ci/check_sorafs_orchestrator_adoption.sh
   ```Le helper rejoue `fixtures/sorafs_orchestrator/multi_peer_parity_v1`,
   écrit `scoreboard.json`, `summary.json`, `provider_metrics.json`,
   `chunk_receipts.json` et `adoption_report.json` sous
   `artifacts/sorafs_orchestrator/<timestamp>/`, et impose un nombre minimum de
   fournisseurs éligibles via `cargo xtask sorafs-adoption-check`.
2. Lorsque les variables de burn-in sont présentes, le script émet également
   `burn_in_note.json`, capturant le label, l'index de jour, l'id du manifeste,
   la source de télémétrie et les digests des artefacts. Joignez ce JSON au journal
   de rollout afin qu’il soit clair quelle capture a satisfait chaque jour de la
   fenêtre de 30 jours.
3. Importez le tableau Grafana mis à jour (`dashboards/grafana/sorafs_fetch_observability.json`)
   dans l’espace de travail staging/production, taguez-le avec le label de burn-in
   et vérifier que chaque panneau affiche des échantillons pour le manifeste/région testée.
4. Exécutez `scripts/telemetry/test_sorafs_fetch_alerts.sh` (ou `promtool test rules …`)
   chaque fois que `dashboards/alerts/sorafs_fetch_rules.yml` change, afin de documenter
   que le routage des alertes correspond aux métriques exportées pendant le burn-in.
5. Archivez le snapshot du tableau de bord, la sortie du test d'alertes et la file d'attente des logs
   des recherches `telemetry::sorafs.fetch.*` avec les artefacts de l’orchestrateur pour
   que la gouvernance puisse rejouer l’évidence sans extraire de métriques des systèmes en direct.

## 7. Checklist de déploiement

1. Régénérez les scoresboards en CI avec la configuration candidate et capturez les
   artefacts sous contrôle de version.
2. Exécutez le fetch déterministe de luminaires dans chaque environnement (lab, staging,
   canari, production) et attachez les artefacts `--scoreboard-out` et `--json-out`
   au registre de déploiement.
3. Passez en revue les tableaux de bord de télémétrie avec l’ingénieur d’astreinte, en
   vérifiant que toutes les métriques ci-dessus ont des échantillons live.
4. Enregistrez le chemin de configuration final (souvent via `iroha_config`) et le
   commit git du registre de gouvernance utilisé pour les annonces et la conformité.
5. Mettez à jour le tracker de rollout et informez les équipes SDK des nouveaux
   défauts afin que les intégrations client restent alignées.

Suivre ce guide maintenir les déploiements de l’orchestrateur déterministes et
auditables, tout en fournissant des boucles de rétroaction claires pour régler
les budgets de nouvelle tentative, la capacité des fournisseurs et la posture de confidentialité.
---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-tuning.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : réglage de l'orchestrateur
titre : Despliegue y ajuste del orquestador
sidebar_label : Ajuster l'orchestre
description : Valeurs pratiques prédéfinies, guide de réglage et points d'auditoire pour amener l'orquestador multi-origen à GA.
---

:::note Source canonique
Refleja `docs/source/sorafs/developer/orchestrator_tuning.md`. Assurez-vous d'avoir des copies alignées jusqu'à ce que le ensemble de documents héréditaires soit retiré.
:::

# Guide de déroulement et d'ajustement de l'orchestre

Ce guide est basé sur la [référence de configuration](orchestrator-config.md) et le
[runbook de despliegue multi-origen](multi-source-rollout.md). Expliquer
comment ajuster l'orchestre pour chaque phase de déroulement, comment interpréter les
les artefacts du tableau de bord et les signaux de télémétrie doivent être répertoriés avant
pour amplifier le trafic. Appliquer les recommandations de forme cohérentes en la
CLI, le SDK et l'automatisation pour que chaque fois que vous suivez la même politique
récupérer déterministe.

## 1. Base de paramètres

Partie d'une plante de configuration séparée et réglage d'un petit ensemble
de périllas a medida que progresa el despliegue. La tabla suivante reconnaît les
valeurs recommandées pour les phases les plus communes ; les valeurs ne sont pas répertoriées vuelven
aux prédéfinis en `OrchestratorConfig::default()` et `FetchOptions::default()`.

| Phase | `max_providers` | `fetch.per_chunk_retry_limit` | `fetch.provider_failure_threshold` | `scoreboard.latency_cap_ms` | `scoreboard.telemetry_grace_secs` | Notes |
|------|-----------------|-------------------------------|------------------------------------|-----------------------------|-----------------------------------------|-------|
| **Laboratoire / CI** | `3` | `2` | `2` | `2500` | `300` | Une limite de latence et une fenêtre de grâce restreinte exposent la télémétrie rapidement et rapidement. Gardez les reintentos bas pour découvrir les manifestes invalides avant. |
| **Mise en scène** | `4` | `3` | `3` | `4000` | `600` | Réfléchissez aux valeurs de production en laissant une marge aux pairs explorateurs. |
| **Canari** | `6` | `3` | `3` | `5000` | `900` | Igual a los valores por defecto ; configurez `telemetry_region` pour que les tableaux de bord puissent segmenter le trafic canari. |
| **Disponibilité générale** | `None` (utiliser tous les éligibles) | `4` | `4` | `5000` | `900` | Augmentez les parapluies de réintention et les chutes pour absorber les chutes transitoires pendant les auditoires en renforçant le déterminisme. |

- `scoreboard.weight_scale` se maintient dans la valeur prédéterminée `10_000` alors qu'un système d'eau abajo nécessite une autre résolution. Augmenter l'escalade en ne changeant pas l'ordre des fournisseurs ; seul émettre une distribution de crédits plus dense.
- Lors de la migration entre les étapes, conservez le paquet JSON et utilisez `--scoreboard-out` pour que le rastro d'auditoire enregistre l'ensemble exact des paramètres.

## 2. Hygiène du tableau de bord

Le tableau de bord combine les exigences du manifeste, les annonces des fournisseurs et la télémétrie.
Avant d'avancer :1. **Valider la fréquence de la télémétrie.** Assurez-vous que les instantanés référencés par
   `--telemetry-json` sera capturé à l'intérieur de la fenêtre de grâce configurée. Les entrées
   plus antiguas que `telemetry_grace_secs` tombe avec `TelemetryStale { last_updated }`.
   Traite comme un blocage dur et actualise l'exportation de télémétrie avant de continuer.
2. **Inspecciona los motivos de elegibilidad.** Persiste los artefactos con
   `--scoreboard-out=/var/lib/sorafs/scoreboards/preflight.json`. Chaque entrée
   inclut un bloc `eligibility` pour la cause exacte de la chute. Pas de description
   désajustes de capacidades o anuncios expirados ; Corriger la charge utile en amont.
3. **Réviser les changements de peso.** Comparer le champ `normalised_weight` avec le
   libération antérieure. Desplazamientos de peso >10% doivent correspondre à des changements
   délibérés en annonçant la télémétrie et en s'enregistrant dans le journal des déplies.
4. **Archiva los artefactos.** Configurer `scoreboard.persist_path` pour chaque
   l'exécution émet l'instantané final du tableau de bord. Ajouter l'artefact au registre
   de release conjointement avec le manifeste et le paquet de télémétrie.
5. **Enregistrer la preuve de la mezcla des fournisseurs.** Les métadonnées de `scoreboard.json`
   y el `summary.json` correspondiente deben exponer `provider_count`,
   `gateway_provider_count` et l'étiquette dérivée `provider_mix` pour les réviseurs
   Vérifiez si l'éjection fue `direct-only`, `gateway-only` ou `mixed`. Les captures de
   la passerelle signale `provider_count=0` et `provider_mix="gateway-only"`, entre les deux
   les exécutions mixtes nécessitent des conteos sans zéro pour les ambos d'origine. `cargo xtask sorafs-adoption-check`
   impone estos campos (y falla si los conteos/etiquetas no coïnciden), así que ejecútalo
   toujours avec `ci/check_sorafs_orchestrator_adoption.sh` ou votre script de capture pour
   produire le bundle de preuves `adoption_report.json`. Cuando haya passerelles Torii,
   conserve `gateway_manifest_id`/`gateway_manifest_cid` dans les métadonnées du tableau de bord
   pour que la porte d'adoption puisse corréler le contenu du manifeste avec la
   mezcla de provenedores capturada.

Pour les définitions détaillées des champs, consultez
`crates/sorafs_car/src/scoreboard.rs` et la structure du CV CLI affichée par
`sorafs_cli fetch --json-out`.

## Référence des drapeaux de la CLI et du SDK

`sorafs_cli fetch` (ver `crates/sorafs_car/src/bin/sorafs_cli.rs`) et le wrapper
`iroha_cli app sorafs fetch` (`crates/iroha_cli/src/commands/sorafs.rs`) partage le
même superficie de configuration de l'orchestre. Usa los siguientes drapeaux al
capturer les preuves du déroulement ou de la reproduction des matchs canoniques :

Référence compartimentée des drapeaux multi-origines (avec l'aide de CLI et les documents en
la synchronisation édite seul ce fichier) :- `--max-peers=<count>` limite les fournisseurs éligibles au filtre du tableau de bord. Il ne faut pas configurer l'émetteur pour transmettre tous les fournisseurs éligibles et le transférer sur `1` uniquement lorsque vous utilisez intentionnellement le secours d'une seule source. Réfléchissez au risque `maxPeers` dans le SDK (`SorafsGatewayFetchOptions.maxPeers`, `SorafsGatewayFetchOptions.max_peers`).
- `--retry-budget=<count>` revient à la limite de reintentos por chunk qui applique `FetchOptions`. Utilisez le tableau de déploiement et le guide d'ajustement des valeurs recommandées ; les exécutions de CLI qui recopilan les preuves doivent coïncider avec les valeurs par défaut du SDK pour maintenir la parité.
- `--telemetry-region=<label>` étiquette de la série Prometheus `sorafs_orchestrator_*` (et les relais OTLP) avec une étiquette de région/entrée pour que les tableaux de bord séparent le trafic du laboratoire, de la mise en scène, de Canary et de GA.
- `--telemetry-json=<path>` injecte l'instantané référencé par le tableau de bord. Conservez le JSON conjointement avec le tableau de bord pour que les auditeurs puissent reproduire l'exécution (et pour que `cargo xtask sorafs-adoption-check --require-telemetry` vérifie que le flux OTLP alimente la capture).
- `--local-proxy-*` (`--local-proxy-mode`, `--local-proxy-norito-spool`, `--local-proxy-kaigi-spool`, `--local-proxy-kaigi-policy`) permet les crochets du pont de l'observateur. Une fois configuré, l'explorateur transmet des morceaux via le proxy local Norito/Kaigi pour que les clients du navigateur, les caches de garde et la salle Kaigi reçoivent les mêmes recettes émises par Rust.
- `--scoreboard-out=<path>` (en option avec `--scoreboard-now=<unix_secs>`) conserve l'instantané d'éligibilité pour les auditeurs. Gardez toujours le JSON persistant avec les artefacts de télémétrie et les références manifestes sur le ticket de sortie.
- `--deny-provider name=ALIAS` / `--boost-provider name=ALIAS:delta` application ajuste les déterministes sur les métadonnées d'annonces. Usa estas flags solo para ensayos ; Les dégradations de la production doivent être effectuées par des artefacts de gouvernement pour que chaque fois qu'ils appliquent le même paquet politique.
- `--provider-metrics-out` / `--chunk-receipts-out` conserve les paramètres de santé du fournisseur et les recibos de morceaux référencés dans la liste de contrôle de déploiement ; ajouter d'autres objets pour présenter la preuve d'adoption.

Exemple (utiliser le luminaire publié):

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

Le SDK utilise la même configuration intermédiaire `SorafsGatewayFetchOptions` fr
le client Rust (`crates/iroha/src/client.rs`), les liaisons JS
(`javascript/iroha_js/src/sorafs.js`) et le SDK Swift
(`IrohaSwift/Sources/IrohaSwift/SorafsOptions.swift`). Mantén ces aides en
synchronisation des valeurs par défaut de la CLI pour que les opérateurs puissent le faire
copier la politique entre l'automatisation sans capacité de traduction à moyen terme.

## 3. Ajuster la politique de récupération

`FetchOptions` contrôle le comportement de réintention, la simultanéité et la vérification.
À ajuster :- **Rétentions :** Elevar `per_chunk_retry_limit` par encima de `4` augmente le temps
  de récupération mais peut occulter les pertes des fournisseurs. Support préféré `4`
  como techo y confiar en la rotation des fournisseurs pour détecter le bas rendu.
- **Umbral de fallos:** `provider_failure_threshold` détermine par un fournisseur
  se déshabilite pour le reste de la séance. Alinea est sa valeur avec la politique de
  reintentos : un umbral mineur que le présupposé des reintentos oblige l'orquestador
  a expulsar un peer antes de agotar todos los reintentos.
- **Concurrencia :** Déjà `global_parallel_limit` sans configurer (`None`) à moins
  qu'un organisme spécifique ne peut pas saturer les rangs annoncés. Quand je me sens
  configurer, assurer que la valeur de la mer ≤ à la somme des présupposés de
  flux des fournisseurs pour éviter toute erreur.
- **Toggles de vérification :** `verify_lengths` et `verify_digests` doivent être permanents
  habilités en production. Garantizan el determinismo cuando hay flotas mixtas
  de fournisseurs; seulement desactívalos en entornos de fuzzing aislados.

## 4. Étapes de transport et anonymisées

Utilisez les champs `rollout_phase`, `anonymity_policy` et `transport_policy` pour
représenter la position de confidentialité :

- Préférer `rollout_phase="snnet-5"` et permettre que la politique d'anonimato por
  défaut de signature des hits de SNNet-5. Sobrescribe avec `anonymity_policy_override`
  seul lorsque la gouvernance émet une directive ferme.
- Mantén `transport_policy="soranet-first"` comme base pour SNNet-4/5/5a/5b/6a/7/8/12/13 estén 🈺
  (version `roadmap.md`). Usa `transport_policy="direct-only"` seul pour les dégradations
  documentées ou simulacros de cumplimiento, et espérons la révision de la couverture PQ
  avant le promoteur `transport_policy="soranet-strict"`—ce niveau tombera rapidement si
  solo quedan relés classiques.
- `write_mode="pq-only"` ne doit être imposé que lorsque chaque itinéraire d'écriture (SDK,
  orquestador, outillage de gobernanza) peut satisfaire les exigences PQ. Durante
  les déploiements portent `write_mode="allow-downgrade"` pour les réponses de
  L'émergence peut être utile dans les itinéraires directs au milieu de la marque de télémétrie
  dégradation.
- La sélection des gardes et la préparation des circuits dépendent du directeur
  de SoraNet. Proporciona el snapshot firmado de `relay_directory` et persiste la
  cache de `guard_set` pour que le taux de désabonnement des gardes soit conservé à l'intérieur du
  ventana de retención acordada. La huella del cache registrada por
  `sorafs_cli fetch` fait partie des preuves de déploiement.

## 5. Ganchos de dégradation et cumplimiento

Les subsistances de l’orquesteur aident à imposer la politique sans manuel d’intervention :- **Remediación de degradaciones** (`downgrade_remediation`) : surveillance des événements
  `handshake_downgrade_total` et, après que le `threshold` soit configuré
  dépasser dentro de `window_secs`, pour le proxy local al `target_mode` (por
  métadonnées défectueuses uniquement). Mantén los valores predeterminados (`threshold=3`,
  `window=300`, `cooldown=900`) salvo que les post-mortems indiquent un patron
  distinct. Documentez tout remplacement dans le journal de déploiement et assurez-vous que
  les tableaux de bord signalent `sorafs_proxy_downgrade_state`.
- **Política de cumplimiento** (`compliance`) : les exclusions par juridiction et
  manifeste à suivre les listes d'exclusions administrées par la gouvernance.
  Nunca insère des remplacements ad hoc dans le bundle de configuration ; et sur ton lieu,
  solliciter une mise à jour entreprise de
  `governance/compliance/soranet_opt_outs.json` et vuelve pour supprimer le JSON généré.

Pour d'autres systèmes, conservez le paquet de configuration résultant et inclus
en les preuves de libération pour que les auditeurs puissent se rastrear comme
activer les réductions.

## 6. Télémétrie et tableaux de bord

Avant d'élargir le programme, confirmez que les signaux suivants sont activés
dans l'objet de l'événement :

- `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` —
  debe sera zéro après avoir terminé le canari.
- `sorafs_orchestrator_retries_total` et
  `sorafs_orchestrator_retry_ratio` — deben stabilisare por debajo del 10%
  pendante el canary y mantenerse por debajo del 5% tras GA.
- `sorafs_orchestrator_policy_events_total` — valide l'étape de déploiement
  J'espère que cela sera activé (étiquette `stage`) et que les baisses de tension seront enregistrées via `outcome`.
-`sorafs_orchestrator_pq_candidate_ratio`/
  `sorafs_orchestrator_pq_deficit_ratio` — rastrean el suministro de relés PQ
  face aux attentes de la politique.
- Objetivos de log `telemetry::sorafs.fetch.*` — deben fluir al agregador de logs
  compartimenté avec des sacs de protection pour `status=failed`.

Chargez le tableau de bord canonique de Grafana depuis
`dashboards/grafana/sorafs_fetch_observability.json` (exporté vers le portail
bas **SoraFS → Fetch Observability**) pour les sélecteurs de région/manifeste,
la carte thermique des intentions du fournisseur, les histogrammes de latence des morceaux et
les contadores de atascos coïncident avec celui qui révise SRE pendant les burn-ins.
Connectez les règles d'Alertmanager à `dashboards/alerts/sorafs_fetch_rules.yml`
et valide la syntaxe de Prometheus avec `scripts/telemetry/test_sorafs_fetch_alerts.sh`
(l'assistant est automatiquement exécuté localement `promtool test rules` ou Docker).
Les transferts d'alertes nécessitent que le même bloc de routage soit imprimé
le script pour que les opérateurs puissent ajouter la preuve au ticket de déploiement.

### Flux de burn-in de télémétrie

L'élément de la feuille de route **SF-6e** nécessite un burn-in de télémétrie de 30 jours avant
de changer l'orchestre multi-origine à ses valeurs GA. Utiliser les scripts du
dépôt pour capturer un ensemble d'objets reproductibles chaque jour
fenêtre :

1. Exécution `ci/check_sorafs_orchestrator_adoption.sh` avec les variables de
   entorno de burn-in configuré. Exemple :

   ```bash
   SORAFS_BURN_IN_LABEL=canary-week-1 \
   SORAFS_BURN_IN_REGION=us-east-1 \
   SORAFS_BURN_IN_MANIFEST=manifest-v4 \
   SORAFS_BURN_IN_DAY=7 \
   SORAFS_BURN_IN_WINDOW_DAYS=30 \
   ci/check_sorafs_orchestrator_adoption.sh
   ```El assistant reproduit `fixtures/sorafs_orchestrator/multi_peer_parity_v1`,
   écrire `scoreboard.json`, `summary.json`, `provider_metrics.json`,
   `chunk_receipts.json` et `adoption_report.json` bas
   `artifacts/sorafs_orchestrator/<timestamp>/`, et impose un numéro minimum de
   fournisseurs éligibles médiante `cargo xtask sorafs-adoption-check`.
2. Lorsque les variables de burn-in sont présentées, le script est également émis
   `burn_in_note.json`, capture l'étiquette, l'indice du jour, l'identifiant du
   manifeste, la source de télémétrie et les résumés des artefacts. Adjoint
   est ce JSON dans le journal de déploiement pour qu'il soit évident qu'il capture chaque jour
   de la fenêtre de 30 jours.
3. Importer le tableau de Grafana actualisé (`dashboards/grafana/sorafs_fetch_observability.json`)
   dans l'espace de travail de staging/production, étiquette avec l'étiquette de burn-in
   et confirma que chaque panneau doit être affiché pour le manifeste/la région en test.
4. Projet `scripts/telemetry/test_sorafs_fetch_alerts.sh` (ou `promtool test rules …`)
   lorsque vous changez `dashboards/alerts/sorafs_fetch_rules.yml` pour documenter que
   Le routage des alertes coïncide avec les valeurs exportées pendant le rodage.
5. Archiver l'instantané du tableau de bord, la sortie de la vérification des alertes et le
   queue de bûches de las búsquedas `telemetry::sorafs.fetch.*` junto a los
   artefactos del orquestador para que la gobernanza pueda reproduire la
   preuve sans extraer métriques de systèmes en vivo.

## 7. Liste de vérification du déploiement

1. Régénérer les tableaux de bord en CI en utilisant la configuration candidate et la capture
   les artefacts ont un contrôle de versions.
2. Exécutez la recherche déterminante des luminaires à chaque événement (laboratoire, mise en scène,
   canary, production) et en complément des artefacts `--scoreboard-out` et `--json-out`
   au registre de déploiement.
3. Révisez les tableaux de bord de télémétrie avec l'ingénieur de garde, en vous assurant que
   toutes les mesures antérieures peuvent être réalisées en vivo.
4. Enregistrez l'itinéraire de configuration final (normalement via `iroha_config`) et le
   commit git del registro de gobernanza utilisé pour les annonces et les cumuls.
5. Actualisez le tracker de déploiement et notifiez les équipes du SDK sur les
   nouvelles valeurs par défaut pour que les intégrations de clients soient maintenues
   alineadas.

Suivez ce guide pour suivre les missions de l'orchestre déterministe et
auditables, mientras aporta cycles de retroalimentación claros para ajustar
présupposés de réintention, capacité des fournisseurs et posture de confidentialité.
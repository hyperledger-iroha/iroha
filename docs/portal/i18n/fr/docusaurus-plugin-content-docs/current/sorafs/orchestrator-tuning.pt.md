---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-tuning.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : réglage de l'orchestrateur
titre : Déploiement et ajustement de l'orchestre
sidebar_label : Ajuster l'orchestre
description: Padrões praticos, orientação de ajuste et checkpoints de auditoria para levar o orquestrador multi-origem à GA.
---

:::note Fonte canônica
Espelha `docs/source/sorafs/developer/orchestrator_tuning.md`. Mantenha as duas cópias alinhadas até que a documentação alternativa seja aposentada.
:::

# Guide de déploiement et d'ajustement de l'orchestre

Ce guide est basé sur la [référence de configuration](orchestrator-config.md) et non
[runbook de déploiement multi-origem](multi-source-rollout.md). Ele explique
comment ajuster l'explorateur pour chaque phase de déploiement, comment interpréter les
artefatos do scoreboard e quais sinais de telemetria devem estar prontos antes
de amplifier le trafic. Appliquer les recommandations de forme cohérentes dans la CLI, nos
Les SDK et l'automatisation pour que chaque personne soit impliquée dans une politique de récupération déterministe.

## 1. Base des ensembles de paramètres

Partie d'un modèle de configuration partagé et ajustement d'un petit ensemble
des boutons à mesure que le déploiement avance. A tabela abaixo captura os valeurs
recommandés pour les phases les plus communes ; les valeurs ne sont pas répertoriées à ce moment-là
Padrões de `OrchestratorConfig::default()` et `FetchOptions::default()`.

| Phase | `max_providers` | `fetch.per_chunk_retry_limit` | `fetch.provider_failure_threshold` | `scoreboard.latency_cap_ms` | `scoreboard.telemetry_grace_secs` | Notes |
|------|-----------------|-------------------------------|------------------------------------|-----------------------------|-----------------------------------------|-------|
| **Laboratoire / CI** | `3` | `2` | `2` | `2500` | `300` | Une limite de latence et une semaine de grâce sont appliquées rapidement à la télémétrie. Mantenha réessaye de baixos pour révéler des manifestes invalides mais cedo. |
| **Mise en scène** | `4` | `3` | `3` | `4000` | `600` | Les responsables de la production ont été spécialement choisis pour leurs pairs explorateurs. |
| **Canari** | `6` | `3` | `3` | `5000` | `900` | Igual aos padrões; définir `telemetry_region` pour que les tableaux de bord puissent séparer le trafic canário. |
| **Disponibilité générale** | `None` (utiliser tous les éléments élégants) | `4` | `4` | `5000` | `900` | Augmentez les limites de réessai et d'échec pour absorber les erreurs de transition pendant que les auditoires continuent de se renforcer ou de déterminisme. |

- `scoreboard.weight_scale` ne reste pas en place avec `10_000` au moins qu'un système en aval s'étend hors de la résolution entière. Augmenter l'échelle sans modifier l'ordonnance des fournisseurs ; ils émettent simplement une distribution de crédits plus dense.
- Ensuite, migrer entre les étapes, persister ou bundle JSON et utiliser `--scoreboard-out` pour que le trilha de l'auditoire s'enregistre ou l'ensemble des paramètres.

## 2. Tableau de bord de l'hygiène

Le tableau de bord combine les exigences du manifeste, les annonces des fournisseurs et la télémétrie.
Avant d'avancer :1. **Valider la fresque de la télémétrie.** Garantir les instantanés référencés par
   `--telemetry-json` foram capturés à l’intérieur de la fille de grâce configurée. Entrées
   mais antigas que `telemetry_grace_secs` falham com `TelemetryStale { last_updated }`.
   Il s'agit également d'un blocage rigide et d'une exportation de télémétrie avant de démarrer.
2. **Inspecione razões de elegibilidade.** Persista artefatos via
   `--scoreboard-out=/var/lib/sorafs/scoreboards/preflight.json`. Chaque entrée
   Traz un bloc `eligibility` pour une cause exata de faux. Pas de réponse
   désajustes de capacité ou annonces expirées ; corrija ou charge utile en amont.
3. **Révisez les modifications du peso.** Comparez le champ `normalised_weight` avec la version
   antérieur. Les modifications > 10 % doivent être corrélées aux modifications délibérées dans les annonces
   ou la télémétrie et la précision sont enregistrées dans le journal de déploiement.
4. **Arquive artefatos.** Configurer `scoreboard.persist_path` pour chaque exécution
   émet un instantané final du tableau de bord. Anexe ou artefato ao registro de release
   avec le manifeste et le bundle de télémétrie.
5. **Registre evidências de mix deprovoreres.** A metadata de `scoreboard.json` _e_ o
   `summary.json` correspondant à l'exportation de développement `provider_count`, `gateway_provider_count`
   et l'étiquette dérivée `provider_mix` pour que les réviseurs prouvent leur exécution
   `direct-only`, `gateway-only` ou `mixed`. Rapport de passerelle Capturas `provider_count=0`
   et `provider_mix="gateway-only"`, pendant certaines exécutions, certaines exigences ne sont pas contagieuses
   zéro para ambas comme fontes. `cargo xtask sorafs-adoption-check` impõe ces champs
   (et s'il y a une divergence entre les contagènes et les étiquettes), alors exécutez-le toujours avec
   `ci/check_sorafs_orchestrator_adoption.sh` ou votre script de capture pour produire
   le paquet de preuves `adoption_report.json`. Quando gateways Torii estiverem
   Envolvidos, mantenha `gateway_manifest_id`/`gateway_manifest_cid` avec les métadonnées
   tableau de bord pour que la porte d'adoption consiga correlacionar l'enveloppe du manifeste
   com o mix de proveneurs capturés.

Pour les définitions détaillées des champs, voyez
`crates/sorafs_car/src/scoreboard.rs` et structure de CV de l'exposition CLI par
`sorafs_cli fetch --json-out`.

## Référence des drapeaux de la CLI et du SDK

`sorafs_cli fetch` (ver `crates/sorafs_car/src/bin/sorafs_cli.rs`) et wrapper
`iroha_cli app sorafs fetch` (`crates/iroha_cli/src/commands/sorafs.rs`) compartimentham
la même superficie de configuration de l'orchestre. Utiliser les drapeaux suivants entre autres
capturer des preuves de déploiement ou de reproduction des luminaires canoniques :

Référence de comparaison des drapeaux multi-originaux (mantenha ajuda da CLI et os docs
synchronisés en éditant ce fichier):- `--max-peers=<count>` Limita quantos provenores elegíveis sobrevivem ao filtre do scoreboard. Il est également configuré pour faciliter le streaming de tous les fournisseurs élégants et définit `1` seulement lorsque vous exercez délibérément ou de secours une source unique. Utilisez le bouton `maxPeers` avec nos SDK (`SorafsGatewayFetchOptions.maxPeers`, `SorafsGatewayFetchOptions.max_peers`).
- `--retry-budget=<count>` encaminha para o limite de retries por chunk aplicado por `FetchOptions`. Utilisez un tableau de déploiement avec guide d'ajustement pour les valeurs recommandées ; les exécutions de CLI qui montrent des preuves doivent correspondre aux pads des SDK pour maintenir la parité.
- `--telemetry-region=<label>` tourne comme série Prometheus `sorafs_orchestrator_*` (et relais OTLP) avec une étiquette de région/ambiance pour que les tableaux de bord séparent le trafic de laboratoire, de staging, de Canary et de GA.
- `--telemetry-json=<path>` injecta ou instantané référencé sur le tableau de bord. Conservez le JSON à la place du tableau de bord pour que les auditeurs puissent reproduire l'exécution (et pour que `cargo xtask sorafs-adoption-check --require-telemetry` prouve que le flux OTLP alimente la capture).
- `--local-proxy-*` (`--local-proxy-mode`, `--local-proxy-norito-spool`, `--local-proxy-kaigi-spool`, `--local-proxy-kaigi-policy`) permet aux crochets de faire un pont d'observation. Une fois défini, l'explorateur transmet des morceaux envoyés au proxy Norito/Kaigi local pour les clients du navigateur, les caches de garde et salas Kaigi reçoivent mes messages émis par Rust.
- `--scoreboard-out=<path>` (en option avec `--scoreboard-now=<unix_secs>`) conserve l'instantané d'éligibilité pour les auditeurs. Toujours emparer le JSON persistant avec les outils de télémétrie et le manifeste référencé sans ticket de sortie.
- `--deny-provider name=ALIAS` / `--boost-provider name=ALIAS:delta` application ajuste les paramètres déterminants sur les métadonnées d'annonces. Utilisez ces drapeaux apenas para ensaios ; les déclassements de production doivent passer par des artéfacts de gouvernance pour que chaque fois que nous appliquons notre ensemble de mesures politiques.
- `--provider-metrics-out` / `--chunk-receipts-out` reprend les mesures saines du fournisseur et des réceptions de morceaux référencés dans la liste de contrôle de déploiement ; anexe ambos os artefatos ao registrar a evidência de adoção.

Exemple (en utilisant le luminaire publié) :

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

Les SDK utilisent une même configuration via `SorafsGatewayFetchOptions` sans client
Rust (`crates/iroha/src/client.rs`), nos fixations JS
(`javascript/iroha_js/src/sorafs.js`) et pas de SDK Swift
(`IrohaSwift/Sources/IrohaSwift/SorafsOptions.swift`). Mantenha est son aide
la synchronisation avec les pads de la CLI pour que les opérateurs puissent copier les politiques entre
automação sem camadas de tradução sob medida.

## 3. Ajuster la politique de récupération

`FetchOptions` contrôlez la nouvelle tentative, la correspondance et la vérification. Ajuster:- **Nouvelles tentatives :** Elevar `per_chunk_retry_limit` augmente le temps de `4`
  récupération, mais vous pouvez masquer les faux-fournisseurs. Prefira manter `4` comme
  teto et confiar na rotação de provenores para expor os fracos.
- **Limiar de falhas:** `provider_failure_threshold` définit quand un fournisseur est
  désactivé pendant la session restante. Alinhe est une valeur politique de réessayer :
  un peu moins que l'orçamento de retry force l'orchestre à éjecter un pair
  avant d'essayer toutes les tentatives.
- **Concorrência:** Deixe `global_parallel_limit` sem definir (`None`) à moins que
  Une ambiance spécifique ne permet pas de saturer les plages annoncées. Quand je l'ai défini,
  garanta que o valor seja ≤ à soma dos orçamentos de streams dos provideores para
  éviter la famine.
- **Bascule de vérification :** `verify_lengths` et `verify_digests` doivent être permanents
  habilités à la production. Ils garantissent le déterminisme quand il y a des erreurs de
  fournisseurs; desatifs-os apenas em ambientes isolados de fuzzing.

## 4. Station de transport et anonymat

Utilisez les champs `rollout_phase`, `anonymity_policy` et `transport_policy` pour
représenter une position de confidentialité :

- Prefira `rollout_phase="snnet-5"` et permet une politique de protection anonyme
  accompagner les cadres du SNNet-5. Substitution via les apenas `anonymity_policy_override`
  quand la gouvernance émet une diretiva assassinée.
- Mantenha `transport_policy="soranet-first"` comme base quanto SNNet-4/5/5a/5b/6a/7/8/12/13 estiverem 🈺
  (voir `roadmap.md`). Utilisez `transport_policy="direct-only"` quelque part pour les rétrogradations
  documentés ou exercices de conformité et garanties de révision de la couverture PQ antes
  du promoteur `transport_policy="soranet-strict"` — ce niveau est rapide et apenas
  relés classiques permanecerem.
- `write_mode="pq-only"` doit être imposté lorsque chaque chemin d'écriture (SDK,
  orquestrador, outillage de gouvernance) puder satisfazer requisitos PQ. Durante
  déploiements, gestion `write_mode="allow-downgrade"` pour les réponses d'urgence
  Vous pouvez utiliser des rotations directes en ce qui concerne la télémétrie sinaliza ou le déclassement.
- La sélection des gardes et l'organisation des circuits dépendent du directeur SoraNet.
  Forneça o snapshot assinado de `relay_directory` et persiste o cache de `guard_set`
  pour que le taux de désabonnement des gardes soit permanent sur une période de rétention conforme. Une impression
  Le cache numérique enregistré par `sorafs_cli fetch` fait partie des preuves de déploiement.

## 5. Crochets de rétrogradation et de conformité

Dois subsistemas do orquestrador ajudam a impor a política sem intervention manual:- **Remédiation de rétrogradation** (`downgrade_remediation`) : surveiller les événements
  `handshake_downgrade_total` et, après la configuration `threshold`, elle sera dépassée
  `window_secs`, force le proxy local pour `target_mode` (métadonnées uniquement par le contrôleur).
  Mantenha os padrões (`threshold=3`, `window=300`, `cooldown=900`) à moins que
  les révisions d'incidents l'indiquent outro padrão. Documenter qualquer override no
  journal de déploiement et garantie que les tableaux de bord accompagnent `sorafs_proxy_downgrade_state`.
- **Política de conformité** (`compliance`) : exclusions de juridiction et manifeste
  fluem por listas de opt-out Geridas par la gouvernance. Nunca insira remplace le cas échéant
  pas de package de configuration ; à ce moment-là, sollicitez une actualisation assinada de
  `governance/compliance/soranet_opt_outs.json` et réimplantez le générateur JSON.

Pour plus de systèmes, persiste ou regroupe la configuration résultante et inclut-le
nas evidências de release para que auditeurs possam rastrear como as reduções foram
actions.

## 6. Télémétrie et tableaux de bord

Avant d'étendre ou de déployer, confirmez que les prochaines étapes sont actives non
ambiance aussi:

- `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` —
  je dois être à zéro après la conclusion du canari.
- `sorafs_orchestrator_retries_total`e
  `sorafs_orchestrator_retry_ratio` — devem se stabilisar abaixo de 10% durante
  le canari et la réduction permanente de 5 % après l'AG.
- `sorafs_orchestrator_policy_events_total` — validation de l'étape de déploiement attendue
  está ativa (étiquette `stage`) et enregistre les baisses de tension via `outcome`.
-`sorafs_orchestrator_pq_candidate_ratio`/
  `sorafs_orchestrator_pq_deficit_ratio` — accompagnement ou supplément de relais PQ em
  relation avec les attentes politiques.
- Les fichiers de journaux `telemetry::sorafs.fetch.*` — doivent être envoyés à l'agrégateur de journaux
  partagé avec les buscas salvas pour `status=failed`.

Carregue ou tableau de bord Grafana canônico em
`dashboards/grafana/sorafs_fetch_observability.json` (exporté sur le portail
**SoraFS → Fetch Observability**) pour les sélections de région/manifeste, ou
heatmap des tentatives par le fournisseur, les histogrammes de latence des morceaux et les
les interlocuteurs du stand correspondent à la révision du SRE pendant les rodages. Connecter
comme indiqué par Alertmanager em `dashboards/alerts/sorafs_fetch_rules.yml` et valider un
syntaxe de Prometheus avec `scripts/telemetry/test_sorafs_fetch_alerts.sh` (ou assistant
exécutez `promtool test rules` localement ou em Docker). Comme passages d'alertes
exige le même bloc de rotation que le script imprime pour que les opérateurs puissent le faire
annexer les preuves du ticket de déploiement.

### Flux de burn-in de télémétrie

L'élément de la feuille de route **SF-6e** exige un burn-in de télémétrie de 30 jours avant
alterner l'orquestrador multi-origem pour vos parents GA. Utilisez les scripts du système d'exploitation
dépôt pour capturer un paquet d'œuvres d'art reproduites pour chaque jour
Janela :

1. Exécutez `ci/check_sorafs_orchestrator_adoption.sh` avec les variantes de burn-in
   configurés. Exemple :

   ```bash
   SORAFS_BURN_IN_LABEL=canary-week-1 \
   SORAFS_BURN_IN_REGION=us-east-1 \
   SORAFS_BURN_IN_MANIFEST=manifest-v4 \
   SORAFS_BURN_IN_DAY=7 \
   SORAFS_BURN_IN_WINDOW_DAYS=30 \
   ci/check_sorafs_orchestrator_adoption.sh
   ```O assistant reproduz `fixtures/sorafs_orchestrator/multi_peer_parity_v1`,
   gravie `scoreboard.json`, `summary.json`, `provider_metrics.json`,
   `chunk_receipts.json` et `adoption_report.json`
   `artifacts/sorafs_orchestrator/<timestamp>/`, et impose un numéro minimum de
   provenores elegíveis via `cargo xtask sorafs-adoption-check`.
2. Lorsque les différentes variantes de burn-in sont présentées, le script est également émis
   `burn_in_note.json`, capture l'étiquette, l'indice du jour, l'identifiant du manifeste,
   a fonte de telemetria e os digests dos artefatos. Anexe est JSON ao log de
   déploiement pour deixar clair qual capture satisfez chaque jour de janvier de 30 jours.
3. Importer le tableau de bord Grafana actualisé (`dashboards/grafana/sorafs_fetch_observability.json`)
   pour l'espace de travail de staging/production, marquez-le avec l'étiquette de burn-in et confirmez
   que chaque peinture soit amicale pour le manifeste/la région du test.
4. Exécutez `scripts/telemetry/test_sorafs_fetch_alerts.sh` (ou `promtool test rules …`)
   semper que `dashboards/alerts/sorafs_fetch_rules.yml` change pour documenter que
   Le réseau d'alertes correspond aux valeurs exportées pendant le rodage.
5. Archivez l'instantané du tableau de bord, ainsi que le test des alertes et la queue des journaux
   das buscas `telemetry::sorafs.fetch.*` junto aos artefatos do orchestre para
   que la gouvernance peut reproduire des preuves sem extrair métricas de sistemas
   dans la production.

## 7. Checklist de déploiement

1. Régénérer les tableaux de bord dans CI en utilisant la configuration candidate et la capture des systèmes
   artefatos sob controle de versão.
2. Exécutez ou récupérez les éléments déterminants de chaque environnement (laboratoire, mise en scène,
   canary, production) et l'annexe des articles `--scoreboard-out` et `--json-out` entre autres
   registre de déploiement.
3. Réviser les tableaux de bord de télémétrie avec l'ingénieur de l'usine, en garantissant que
   todas as métricas acima tenham amostras ao vivo.
4. Enregistrez le chemin final de configuration (généralement via `iroha_config`) et
   commit git do registro de gouvernance utilisé pour les annonces et la conformité.
5. Actualisez le tracker de déploiement et la notification en tant qu'équipe du SDK sur les nouveaux
   Padrões para manter as integrações de clientses alinhadas.

Suivez cette guide pour les déploiements de l'orchestre déterminant et
Passíveis de auditoria, enquanto fornece cycles de feedback claros para ajustar
orçamentos de retrys, capacité de fournisseurs et posture de confidentialité.
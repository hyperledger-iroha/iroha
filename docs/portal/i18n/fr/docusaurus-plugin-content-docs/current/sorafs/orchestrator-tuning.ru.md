---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-tuning.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : réglage de l'orchestrateur
titre : Déploiement et настройка оркестратора
sidebar_label : Opérateur en chef
description : Mises en pratique pour l'exploitation, les projets de gestion et d'audit pour votre opérateur multi-sources dans GA.
---

:::note Канонический источник
Utilisez `docs/source/sorafs/developer/orchestrator_tuning.md`. Il est possible de copier des copies synchronisées si vous souhaitez obtenir des documents pertinents.
:::

# Руководство по rollout и настройке оркестратора

Ceci est une opération de [configuration de configuration](orchestrator-config.md) et
[déploiement multi-source du runbook](multi-source-rollout.md). Je suis sûr que,
comment configurer l'opérateur pour le déploiement de chaque mode, comment l'interpréter
Des objets de tableau de bord et des signaux télémétriques doivent être affichés pour la période précédente.
расширением трафика. Donnez ensuite des recommandations sur la CLI, le SDK et
Les automatismes qui peuvent être utilisés de manière propre et précise
политике chercher.

## 1. Paramètres de base

Начните собщего шаблона конфигурации и корректируйте небольшой набор ручек по мере
déploiement progressif. La table des nouveaux équipements recommande des solutions pour le travail
распространённых фаз; les paramètres, qui ne sont pas indiqués dans le tableau, sont affichés
`OrchestratorConfig::default()` et `FetchOptions::default()`.

| Faza | `max_providers` | `fetch.per_chunk_retry_limit` | `fetch.provider_failure_threshold` | `scoreboard.latency_cap_ms` | `scoreboard.telemetry_grace_secs` | Première |
|------|-----------------|-------------------------------|------------------------------------|-----------------------------|-----------------------------------------|------------|
| **Laboratoire / CI** | `3` | `2` | `2` | `2500` | `300` | Les limites du service et le courtage grâce à votre téléphone sont disponibles. N'hésitez pas à réessayer, afin de pouvoir afficher des manifestations non commerciales. |
| **Mise en scène** | `4` | `3` | `3` | `4000` | `600` | Les paramètres du produit sont fournis à des pairs expérimentés. |
| **Canari** | `6` | `3` | `3` | `5000` | `900` | Соответствует значениям по умолчанию; Utilisez le `telemetry_region` pour que votre bord puisse utiliser le segment Canary Trafic. |
| **Disponibilité générale** | `None` (pour tous les éligibles) | `4` | `4` | `5000` | `900` | Faites une nouvelle tentative/démarrage pour pouvoir effectuer des opérations de transition, avant de procéder à l'audit pour déterminer la détection. |

- `scoreboard.weight_scale` est installé sur le modèle `10_000`, si le système en aval ne traite pas les problèmes les plus rapides. Увеличение масштаба не меняет порядок провайдеров; Je pense que vous avez plus de chances de répartir les crédits.
- Avant de pouvoir utiliser le bundle JSON et utiliser `--scoreboard-out`, vous devrez vérifier les paramètres les plus précis.

## 2. Tableau de bord de Gigiena

Le tableau de bord indique la présence du manifeste, la surveillance des fournisseurs et la télémétrie.
Produit précédent :1. **Ajoutez les meilleurs télémètres.** Choisissez l'instantané qui apparaît dans
   `--telemetry-json`, vous l'avez trouvé avant la grâce. Записи старше
   `telemetry_grace_secs` s'applique à `TelemetryStale { last_updated }`.
   Renseignez-vous sur ce blocage et sur les télémètres d'exportation avant de procéder à leur production.
2. **Éligibilité aux prix.** Rechercher les articles ici
   `--scoreboard-out=/var/lib/sorafs/scoreboards/preflight.json`. Каждая запись
   Mettez le bloc `eligibility` à votre disposition. Ne vous inquiétez pas
   возможностей или истекшие объявления; исправьте исходный charge utile.
3. **Prouvrez-le.** Réglez le pôle `normalised_weight` avant de le lancer.
   Les paiements > 10 % doivent permettre de déterminer le nom de l'observation ou de la télémétrie
   et vous pouvez facilement ajouter des éléments au déploiement final.
4. **Архивируйте артефACTы.** Настройте `scoreboard.persist_path`, чтобы каждый запуск
   Tableau de bord instantané final publié. Utiliser l'artéfact avec fiabilité
   вместе с манифестом и телеметрическим bundle.
5. **Фиксируйте доказательства микса провайдеров.** Метаданные `scoreboard.json` _и_
   соответствующий `summary.json` должны содержать `provider_count`,
   `gateway_provider_count` et votre étiquette `provider_mix`, que vous pouvez consulter
   Recherchez-le en utilisant `direct-only`, `gateway-only` ou `mixed`. Gateway‑снимки
   veuillez acheter `provider_count=0` et `provider_mix="gateway-only"`, à la fois
   прогоны — ненулевые значения для обеих источников. `cargo xtask sorafs-adoption-check`
   VALIDIRUEт эти поля (и падает при несоответствии счётчиков/лейблов), поэтому запускайте
   C'est le cas avec `ci/check_sorafs_orchestrator_adoption.sh` ou avec votre script,
   чтобы получить bundle `adoption_report.json`. Когда задействованы Torii passerelles,
   Mettez `gateway_manifest_id`/`gateway_manifest_cid` dans le tableau de bord méta, vous
   adoption‑gate мог сопоставить enveloppe manifeste avec зафиксированным миксом провайдеров.

Подробные определения полей см. в
`crates/sorafs_car/src/scoreboard.rs` et résumé de la structure CLI, à votre disposition
`sorafs_cli fetch --json-out`.

## Paramètres des drapeaux CLI et SDK

`sorafs_cli fetch` (см. `crates/sorafs_car/src/bin/sorafs_cli.rs`) et carte
`iroha_cli app sorafs fetch` (`crates/iroha_cli/src/commands/sorafs.rs`) utilisé
Maintenant et votre opérateur de configuration est compétent. Utiliser les drapeaux suivants
при сборе évidence ou воспроизведении канонических luminaires:

Общая справка по флагам multi-source (consultez l'aide CLI et la documentation synchronisée,
правя только этот файл):- `--max-peers=<count>` ограничивает число éligibles-провайдеров, проходящих фильтр tableau de bord. Installez-le pour établir un lien avec tous les fournisseurs éligibles et installez `1` uniquement pour la mise en place d'une solution de secours à source unique. Accédez au fichier `maxPeers` dans le SDK (`SorafsGatewayFetchOptions.maxPeers`, `SorafsGatewayFetchOptions.max_peers`).
- `--retry-budget=<count>` déclenche une nouvelle tentative sur un bloc, par exemple `FetchOptions`. Utilisez le déploiement de la table pour les heures recommandées ; Les programmes CLI, qui collectent des preuves, peuvent également aider à créer le SDK pour la sécurité des parties.
- `--telemetry-region=<label>` est disponible pour la série Prometheus `sorafs_orchestrator_*` (et OTLP-реле) dans la région/l'environnement, à proximité du laboratoire, du staging, du canary et GA‑трафик.
- `--telemetry-json=<path>` инжектит instantané, указанный в tableau de bord. Utilisez JSON pour le tableau de bord, ce que les auditeurs peuvent utiliser (et ce que `cargo xtask sorafs-adoption-check --require-telemetry` a fait pour capturer l'état OTLP).
- `--local-proxy-*` (`--local-proxy-mode`, `--local-proxy-norito-spool`, `--local-proxy-kaigi-spool`, `--local-proxy-kaigi-policy`) pour le crochet et le pont. Lors de l'utilisation de morceaux par l'opérateur dans le proxy local Norito/Kaigi, les clients, les caches de garde et les salles Kaigi ont publié les reçus, c'est-à-dire Rust.
- `--scoreboard-out=<path>` (en option avec `--scoreboard-now=<unix_secs>`) définit l'éligibilité aux instantanés pour les auditeurs. Vous devez utiliser JSON avec des objets télémétriques et des manifestes, disponibles en temps réel.
- `--deny-provider name=ALIAS` / `--boost-provider name=ALIAS:delta` permet de déterminer les corrections apportées aux métadonnées utilisées. Utilisez ce drapeau uniquement pour la répétition ; Le projet de rétrogradation devrait permettre de mettre en place des éléments de gouvernance qui sont utilisés dans l'ensemble des politiques.
- `--provider-metrics-out` / `--chunk-receipts-out` fournissent des mesures pour le fournisseur et les reçus pour les morceaux de la liste de contrôle de déploiement ; Prenez un artéfact pour obtenir des preuves en cas d'adoption.

Exemple (pour l'utilisation d'un luminaire public) :

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

Le SDK utilise votre configuration à partir de `SorafsGatewayFetchOptions` dans le client Rust
(`crates/iroha/src/client.rs`), liaisons JS
(`javascript/iroha_js/src/sorafs.js`) et SDK Swift
(`IrohaSwift/Sources/IrohaSwift/SorafsOptions.swift`). Держите эти хелперы
Avec les logiciels CLI, les opérateurs peuvent copier les politiques dans l'automatisation
без специальных слоёв трансляции.

## 3. Récupérer les politiques politiques

`FetchOptions` effectue une nouvelle tentative, en parallèle et en vérifiant. Pour commencer :- **Nouvelles tentatives :** La tentative `per_chunk_retry_limit` a été effectuée par `4` lors de la récupération,
  Vous ne pouvez pas masquer les problèmes des fournisseurs. Prévoyez le `4` pour le chauffage et
  полагаться на ротацию провайдеров для выявления слабых.
- **Порог отказов:** `provider_failure_threshold` определяет, когда провайдер отключается
  на остаток сессии. Согласуйте это значение с retry-politik: порог ниже бюджета
  réessayer заставляет оркестратор выгнать peer ещё до исчерпания всех tentatives.
- **ПараллеLISм:** оставляйте `global_parallel_limit` désarmé (`None`), si ce n'est du béton
  Vous ne pouvez donc pas ouvrir les diapasons obstrués. Si vous le souhaitez, pensez à ce qui se passe
  ≤ сумме потоковых бюджетов провайдеров, чтобы избежать la famine.
- **Vérifications :** `verify_lengths` et `verify_digests` sont actuellement disponibles.
  dans le produit. Nous garantissons la détermination par les prestataires de services ; отключайте
  их только в изолированных fuzzing‑средах.

## 4. Stations de transport et services anonymes

Utilisez les modèles `rollout_phase`, `anonymity_policy` et `transport_policy`.
описать приватностную позу:

- Lancez le `rollout_phase="snnet-5"` et activez la politique anonyme.
  следовать этапам SNNet-5. Veuillez consulter `anonymity_policy_override` jusqu'à
  когда gouvernance выпускает подписанную директиву.
- Sélectionnez `transport_policy="soranet-first"` à partir de la base, lorsque SNNet-4/5/5a/5b/6a/7/8/12/13 est disponible dans 🈺
  (см. `roadmap.md`). Utilisez `transport_policy="direct-only"` uniquement pour la documentation
  downgrade/комплаенс‑учений и дождитесь ревью PQ‑покрытия перед повышением до
  `transport_policy="soranet-strict"` — Ceci est une mise à jour si vous n'êtes pas en mesure de le faire
  классические реле.
- `write_mode="pq-only"` permet de configurer seulement votre ordinateur en écriture (SDK, orchestrateur,
  outils de gouvernance) способны удовлетворить PQ-требования. Lors du déploiement actuel, veuillez consulter
  `write_mode="allow-downgrade"`, quelles sont les réactions que vous pouvez utiliser
  маршруты, пока телеметрия отмечает déclassement.
- Consultez les gardes et les circuits de mise en scène du catalogue SoraNet. Передавайте подписанный
  instantané `relay_directory` et sauvegarde du cache `guard_set`, les gardes de désabonnement sont installés
  в согласованном rétention‑окне. Cache d'exploitation, cache `sorafs_cli fetch`, disponible
  в déploiement des preuves.

## 5. Les dommages et la complexité

Les opérateurs du système d'exploitation devraient s'occuper de la politique hors des frontières:- **Dégradation des mesures correctives** (`downgrade_remediation`) : отслеживает события
  `handshake_downgrade_total` et, après la précédente `threshold`, avant `window_secs`,
  Utilisez le proxy local dans `target_mode` (pour la gestion des métadonnées uniquement). Сохраняйте
  Chariots (`threshold=3`, `window=300`, `cooldown=900`), sauf s'il s'agit d'un courrier postal
  показывают другой паттерн. Documentez le remplacement dans le panneau de déploiement du journal et
  убедитесь, что дашbordы отслеживают `sorafs_proxy_downgrade_state`.
- **Политика комплаенса** (`compliance`) : exclusions pour la législation et le manifeste
  проходят через списки opt-out, управляемые la gouvernance. Никогда не встраивайте
  remplacement ad hoc dans les configurations groupées ; вместо этого запросите подписанное обновление
  `governance/compliance/soranet_opt_outs.json` et activez le JSON généré.

Pour que votre système puisse configurer cette configuration groupée et l'inclure dans les preuves
Il est clair que les auditeurs peuvent faire avancer les choses.

## 6. Télémétrie et bord

Avant de procéder au déploiement, assurez-vous de sélectionner les signaux actifs en ce moment :

- `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` —
  должен быть нулевым после завершения canari.
- `sorafs_orchestrator_retries_total` et
  `sorafs_orchestrator_retry_ratio` — stabilise jusqu'à 10 % pour Canary
  et je paierai jusqu'à 5 % après l'AG.
- `sorafs_orchestrator_policy_events_total` — activer l'activité des stades olympiques
  déploiement (lien `stage`) et correction des baisses de tension à partir de `outcome`.
-`sorafs_orchestrator_pq_candidate_ratio`/
  `sorafs_orchestrator_pq_deficit_ratio` — отслеживают доступность PQ‑реле относительно
  ожиданий политики.
- L'enregistreur `telemetry::sorafs.fetch.*` — doit simplement être installé dans l'enregistreur de données de votre entreprise
  сохранёнными запросами pour `status=failed`.

Ajouter le canal canonique Grafana‑дашbord из
`dashboards/grafana/sorafs_fetch_observability.json` (exportant sur le support du portail
**SoraFS → Récupérer l'observabilité**), sélectionnez la région/le manifeste, les tentatives de carte thermique
À l'aide du fournisseur, l'histogramme contient des morceaux et des stagnations de sauvegarde.
À ce moment-là, ce que SRE a prouvé lors du rodage. Ajoutez Alertmanager à
`dashboards/alerts/sorafs_fetch_rules.yml` et vérifier la syntaxe Prometheus ici
`scripts/telemetry/test_sorafs_fetch_alerts.sh` (helper автоматически запускает
`promtool test rules` localement ou dans Docker). Pour le transfert d'alerte, vous devez faire appel à tout le monde
bloc de routage, ce script contient des éléments de preuve que les opérateurs peuvent utiliser
к rollout‑tiketу.

### Processus de rodage télémétrique

La feuille de route **SF-6e** nécessite 30 jours de rodage télémétrique avant la clôture
оркестратора multi-sources pour GA‑дефолты. Используйте скрипты репозитория, чтобы
Vous pouvez facilement rechercher les éléments du bundle d'articles pour les produits suivants :

1. Sélectionnez `ci/check_sorafs_orchestrator_adoption.sh` avec les paramètres d'installation
   brûlure. Exemple :

   ```bash
   SORAFS_BURN_IN_LABEL=canary-week-1 \
   SORAFS_BURN_IN_REGION=us-east-1 \
   SORAFS_BURN_IN_MANIFEST=manifest-v4 \
   SORAFS_BURN_IN_DAY=7 \
   SORAFS_BURN_IN_WINDOW_DAYS=30 \
   ci/check_sorafs_orchestrator_adoption.sh
   ```Assistant проигрывает `fixtures/sorafs_orchestrator/multi_peer_parity_v1`,
   записывает `scoreboard.json`, `summary.json`, `provider_metrics.json`,
   `chunk_receipts.json` et `adoption_report.json` dans
   `artifacts/sorafs_orchestrator/<timestamp>/`, et montre un mini-chislo
   éligibles pour les produits `cargo xtask sorafs-adoption-check`.
2. Lorsque le burn-in est activé, le script est `burn_in_note.json`,
   étiquette de fixation, index des noms, manifeste d'identification, systèmes de télécommunications et d'art.
   Utilisez ce JSON dans le déploiement du jour pour que vous puissiez le faire correctement
   день 30-дневного окна.
3. Importez le tableau de bord Grafana (`dashboards/grafana/sorafs_fetch_observability.json`)
   Dans la préparation/production de l'espace de travail, créez votre étiquette de burn-in et créez ce que vous voulez.
   панель отображает выборки для тестируемых manifeste/région.
4. Sélectionnez `scripts/telemetry/test_sorafs_fetch_alerts.sh` (ou `promtool test rules …`)
   En ce qui concerne la modification `dashboards/alerts/sorafs_fetch_rules.yml`, vous devez le faire
   Il s'agit d'alertes de routage personnalisées et de mesures d'exportation vers le burn-in.
5. Enregistrez les images du bord pour tester les alertes et les logos de votre entreprise.
   `telemetry::sorafs.fetch.*` вместе с артефактами оркестратора, чтобы могло
   воспроизвести preuve без обращения к live‑système.

## 7. Déploiement de la liste de contrôle

1. Configurez les tableaux de bord dans CI avec la configuration candidate et sélectionnez les articles sous VCS.
2. Vérifiez les appareils de récupération dans chaque salle (laboratoire, mise en scène, canari, production)
   et utilisez les éléments d'art `--scoreboard-out` et `--json-out` dans le déploiement des fenêtres.
3. Vérifiez les services télémétriques du bord de mer avec les agents de garde, en conséquence, ce que vous faites
   Les mesures de votre véhicule correspondent à vos besoins.
4. Configurez la configuration finale de la configuration (en utilisant `iroha_config`) et git‑commit
   gouvernance‑реестра, использованного для объявлений и комплаенса.
5. Ouvrez le programme de déploiement et utilisez les commandes SDK ou les nouveaux logiciels client.
   les intégrations installent la synchronisation.

Le processus de dépannage de l'entreprise de décontamination et de décontamination
Les écouteurs, spécialement conçus pour les contours de la peau des gens
retry‑бюджетов, ёмкости провайдеров и приватностной позы.
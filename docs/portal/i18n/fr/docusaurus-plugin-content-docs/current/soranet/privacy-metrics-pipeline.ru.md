---
lang: fr
direction: ltr
source: docs/portal/docs/soranet/privacy-metrics-pipeline.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : pipeline de mesures de confidentialité
titre : Convertisseur de mesures privées SoraNet (SNNet-8)
sidebar_label : Convertisseur de mesures privées
description : Les télémètres sont dotés de fonctions privées pour le relais et l'orchestrateur SoraNet.
---

:::note Канонический источник
Consultez `docs/source/soranet/privacy_metrics_pipeline.md`. Si vous souhaitez copier des copies synchronisées, vous ne pourrez pas vous occuper de l'extraction de documents.
:::

# Convertisseur de mesures privées SoraNet

SNNet-8 permet de s'orienter vers la commutation télémétrique privée pour le relais d'exécution. Le relais active la poignée de main et le circuit dans des seaux miniatures et exporte des connecteurs Prometheus, en installant des circuits externes Les utilisateurs et les opérateurs professionnels doivent le visualiser.

## Обзор агрегатора

- La résolution d'exécution est adaptée à `tools/soranet-relay/src/privacy.rs` comme `PrivacyAggregator`.
- Buckets s'active pendant une période de temps minimale (`bucket_secs`, pendant 60 secondes) et se déroule dans le cadre général (`max_completed_buckets`, à partir de 120). Les partages de collectionneurs représentent un arriéré administratif important (`max_share_lag_buckets`, selon l'article 12), qui permet de supprimer les compartiments supprimés, mais pas de les utiliser dans память или маскировали зависшие collectionneurs.
- `RelayConfig::privacy` est compatible avec `PrivacyConfig`, les informations ouvertes (`bucket_secs`, `min_handshakes`, `flush_delay_buckets`, `force_flush_buckets`, `max_completed_buckets`, `max_share_lag_buckets`, `expected_shares`). Lors de l'exécution de la production, le SNNet-8a est sans interférence.
- Les modules d'exécution proposent des assistants typiques : `record_circuit_accepted`, `record_circuit_rejected`, `record_throttle`, `record_throttle_cooldown`, `record_capacity_reject`, `record_active_sample`, `record_verified_bytes` et `record_gar_category`.

## Relais Админ-эндпоинт

Les opérateurs peuvent utiliser le relais d'écoute administrateur pour votre compte `GET /privacy/events`. L'outil JSON est utilisé avec le flux de données (`application/x-ndjson`), avec des charges utiles `SoranetPrivacyEventV1`, en dehors de l'environnement `PrivacyEventBuffer`. Le serveur a la même nouvelle solution que le `privacy.event_buffer_capacity` (par l'intermédiaire du 4096) et s'affiche à l'écran. Vous n'avez pas besoin d'effectuer les livraisons nécessaires pour que les propriétaires puissent s'en occuper. Vous pouvez rechercher les signaux de poignée de main, d'accélérateur, de bande passante vérifiée, de circuit actif et de GAR, avec les connecteurs Prometheus, pour l'archivage des collecteurs en aval. Vous pouvez utiliser le fil d'Ariane de manière privée ou gérer les flux de travail sans interférence.

## Relais de configuration

Les opérateurs déterminent la cadence des télémètres privés dans la configuration du relais de type câble selon la section `privacy` :

```json
{
  "mode": "Entry",
  "listen": "0.0.0.0:443",
  "privacy": {
    "bucket_secs": 60,
    "min_handshakes": 12,
    "flush_delay_buckets": 1,
    "force_flush_buckets": 6,
    "max_completed_buckets": 120,
    "max_share_lag_buckets": 12,
    "expected_shares": 2
  }
}
```

Informations relatives à la compatibilité avec les spécifications SNNet-8 et vérifiées lors de l'installation :| Pôle | Description | По умолчанию |
|------|----------|--------------|
| `bucket_secs` | Ширина каждого окна агрегации (секунды). | `60` |
| `min_handshakes` | Un minimum de déchets avant de sortir le seau. | `12` |
| `flush_delay_buckets` | Количество завершенных seaux перед попыткой flush. | `1` |
| `force_flush_buckets` | Максимальный возраст перед выпуском seau supprimé. | `6` |
| `max_completed_buckets` | Il y a plusieurs compartiments de backlog (il n'y a pas de perte de temps). | `120` |
| `max_share_lag_buckets` | Il est généralement possible d'acquérir des actions de collectionneur. | `12` |
| `expected_shares` | Число Prio actions de collectionneur перед объединением. | `2` |
| `event_buffer_capacity` | Backlog NDJSON событий для admin-post. | `4096` |

L'installation `force_flush_buckets` par rapport à `flush_delay_buckets` permet d'ouvrir les portes ou d'ouvrir la garde pour vérifier la validation des choses. избежать развертываний, которые бы утекали телеметрией на уровне relais.

La limite `event_buffer_capacity` est définie par `/admin/privacy/events`, garantissant que les écrans ne soient pas correctement ouverts.

## Actions de collection Prio

SNNet-8a разворачивает двойные collectionneurs, которые выпускают секретно-разделенные seaux Prio. Orchestrator utilise le pot NDJSON `/privacy/events` pour installer `SoranetPrivacyEventV1` et partager `SoranetPrivacyPrioShareV1`, en commençant par `SoranetSecureAggregator::ingest_prio_share`. Les seaux sont utilisés pour utiliser le relais `PrivacyBucketConfig::expected_shares`, en utilisant le relais. Les actions sont validées pour la gestion des compartiments et pour la forme d'histogrammes avant la soumission dans `SoranetPrivacyBucketMetricsV1`. Si cette poignée de main est compatible avec le `min_contributors`, le seau est exporté vers le `suppressed`, il est possible d'utiliser le relais de l'agrégateur. Si vous êtes à la recherche d'un `suppression_reason`, les opérateurs peuvent ouvrir `insufficient_contributors`, `collector_suppressed`, `collector_window_elapsed`. et `forced_flush_window_elapsed` pour les diagnostics télémétriques. L'achat `collector_window_elapsed` s'est produit, tandis que les actions Prio ont porté sur `max_share_lag_buckets`, ce qui a permis aux collectionneurs de voir les collectionneurs hors de leur vie. installez des accumulateurs dans les locaux.

## Эндпоинты приема Torii

Torii est publié pour les connexions HTTP télémétriques, les relais et les collecteurs peuvent effectuer une activation sans l'utilisation du service client. transport :- `POST /v2/soranet/privacy/event` utilise la charge utile `RecordSoranetPrivacyEventDto`. Il s'agit du `SoranetPrivacyEventV1` et du `source`. Torii vérifie les profils actifs de télémétrie, ouvre la connexion et active HTTP `202 Accepted` sur le site Le convertisseur JSON Norito vous permet d'utiliser le relais (`bucket_start_unix`, `bucket_duration_secs`) et le relais.
- `POST /v2/soranet/privacy/share` contient la charge utile `RecordSoranetPrivacyShareDto`. Il n'y a pas de `SoranetPrivacyPrioShareV1` et je ne souhaite pas acheter `forwarded_by`, les opérateurs peuvent auditionner les collectionneurs de voitures. Les ouvertures HTTP `202 Accepted` avec le convertisseur JSON Norito, le collecteur de résumé, le bucket et le support sont disponibles ; Les appareils de validation sont compatibles avec l'appareil télémétrique `Conversion`, afin de déterminer le mode de fonctionnement du robot d'exploitation. collectionneurs. Un orchestrateur doit utiliser ces partages pour utiliser des relais, en synchronisant le prio-accumulateur Torii avec des compartiments de relais.

Pour l'entreprise, vous avez un profil télémétrique : sur `503 Service Unavailable`, certaines mesures s'affichent. Les clients peuvent utiliser le binaire Norito (`application/x.norito`) ou Norito JSON (`application/x.norito+json`) ; Le serveur automatique s'occupe des extracteurs Torii standard.

## Mesures Prometheus

Le seau d'exportation ne nécessite pas les matériaux `mode` (`entry`, `middle`, `exit`) et `bucket_start`. La mesure du temps est la suivante :

| Métrique | Descriptif |
|--------|-------------|
| `soranet_privacy_circuit_events_total{kind}` | Poignées de main taxonomiques avec `kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}`. |
| `soranet_privacy_throttles_total{scope}` | Prises d'accélérateur avec `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}`. |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` | Le temps de recharge est amélioré et les poignées de main sont limitées. |
| `soranet_privacy_verified_bytes_total` | Il s'agit d'une offre éprouvée par les documents suivants. |
| `soranet_privacy_active_circuits_{avg,max}` | Créez et choisissez des circuits actifs sur le seau. |
| `soranet_privacy_rtt_millis{percentile}` | Оценки Centiles RTT (`p50`, `p90`, `p99`). |
| `soranet_privacy_gar_reports_total{category_hash}` | Хешированные счетчики Governance Action Report, ключом является digest категории. |
| `soranet_privacy_bucket_suppressed` | Les seaux sont extérieurs, mais ceux-ci ne doivent pas être livrés. |
| `soranet_privacy_pending_collectors{mode}` | Les accumulateurs d'actions de collection dans le domaine de l'exploitation, сгруппированные по режиму relais. |
| `soranet_privacy_suppression_total{reason}` | Les seaux supprimés avec `reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}`, qui peuvent être attribués à des ports privés, sont approuvés. |
| `soranet_privacy_snapshot_suppression_ratio` | Si la suppression/la suppression est effectuée après la vidange (0-1), cela s'applique principalement aux alertes de contrôle. |
| `soranet_privacy_last_poll_unixtime` | UNIX-vremя самого свежего успешного опроса (питает alert collector-idle). |
| `soranet_privacy_collector_enabled` | Jauge, sur la touche `0`, le collecteur de confidentialité est ouvert ou non (le collecteur d'alerte est désactivé). |
| `soranet_privacy_poll_errors_total{provider}` | Les utilisateurs utilisent un alias de relais (ils sont connectés à des décodages HTTP ou à des codes d'état inconnus). |

Seaux без наблюдений остаются молчаливыми, сохраняя дашbordы чистыми без создания нулевых окон.

## Recommandations de fonctionnement1. **Дашbordы** - définissez les mesures du groupe sur `mode` et `window_start`. N'hésitez pas à contacter l'acheteur pour résoudre les problèmes de collecteurs ou de relais. Utilisez `soranet_privacy_suppression_total{reason}` pour résoudre les problèmes de suppression et de suppression, pour les collecteurs et pour trois problèmes. Dans Grafana, affichez ce panneau **"Suppression Reasons (5m)"** sur la base de données et la statistique **"Suppressed Bucket %"**, qui apparaît sur `sum(soranet_privacy_bucket_suppressed) / count(...)`. À l'aide de la fenêtre d'ouverture de la fenêtre, les opérateurs ont vu les informations fournies par l'utilisateur. La série **"Collector Share Backlog"** (`soranet_privacy_pending_collectors`) et la statistique **"Snapshot Suppression Ratio"** vous permettent de trouver des collectionneurs et de créer des projets pour vos programmes automatiques.
2. **Avertissement** : activez les programmes privés : le rejet de PoW, le temps de recharge, le RTT et les rejets de capacité. Il y a des monotones dans le seau précédent, qui seront utilisés pour le robot de la maison.
3. **Инцидент-ответ** - сначала полагайтесь на агрегированные данные. Alors que vous êtes au travail du groupe, vous pouvez utiliser les relais pour créer des seaux d'instantanés ou pour vérifier la documentation de votre logo. trafic.
4. **Удержание** - скрейпьте достаточно часто, чтобы не превысить `max_completed_buckets`. Les exportateurs doivent choisir leur installation canonique Prometheus et acheter des seaux locaux après leur ouverture.

## Suppression analytique et programmes automatiques

La société SNNet-8 prévient les démons, car les collecteurs automatiques ont été mis en place, une suppression ayant été interrompue dans les politiques précédentes (≤ 10 % des seaux par relais за любое 30-minutes окно). De nombreux instruments doivent être placés dans le dépôt ; Les opérateurs doivent créer des rituels ordinaires. De nouveaux panneaux de suppression dans Grafana ont été mis en place par PromQL pour que votre commande vous montre ce qu'il en est. прибегнут к ручным запросам.

### PromQL-réceptions pour la suppression des observations

Les opérateurs doivent fournir des services PromQL pour les utilisateurs ; Pour les utilisateurs du bord de mer Grafana (`dashboards/grafana/soranet_privacy_metrics.json`) et Alertmanager :

```promql
/* Suppression ratio per relay mode (30 minute window) */
(
  increase(soranet_privacy_suppression_total{reason=~"insufficient_contributors|collector_suppressed|collector_window_elapsed|forced_flush_window_elapsed"}[30m])
) /
clamp_min(
  increase(soranet_privacy_circuit_events_total{kind="accepted"}[30m]) +
  increase(soranet_privacy_suppression_total[30m]),
1
)
```

```promql
/* Detect new suppression spikes above the permitted minute budget */
increase(soranet_privacy_suppression_total{reason=~"insufficient_contributors|collector_window_elapsed|collector_suppressed"}[5m])
/
clamp_min(
  sum(increase(soranet_privacy_circuit_events_total{kind="accepted"}[5m])),
1
)
```

Utilisez votre solution pour modifier la statistique **"Suppressed Bucket %"** qui n'est plus disponible ; Si vous connectez le détecteur à Alertmanager pour la surveillance de l'affichage, les informations disponibles ne sont pas disponibles.

### CLI pour les buckets de navigation aérienne

Dans l'espace de travail, `cargo xtask soranet-privacy-report` est installé pour l'emplacement NDJSON. Contactez le relais admin-sportsport:

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```

L'aide permet de télécharger `SoranetSecureAggregator`, en supprimant la sortie sur la sortie standard et en utilisant la structure JSON disponible ici `--json-out <path|->`. Lors de l'utilisation de vos ressources et du Live Collector (`--bucket-secs`, `--min-contributors`, `--expected-shares` et d.), l'opérateur vous permet de le faire. L'histoire se rapporte aux drogues provenant de l'incident. Utilisez JSON avec l'écran Grafana pour utiliser SNNet-8 pour supprimer l'analyse audio.### Чеклист первой автоматизированной сессии

Les gouvernements vont devoir préparer une session de suppression automatique. Pour vous aider à utiliser le `--max-suppression-ratio <0-1>`, les CI ou les opérateurs qui ont le plus d'exploitation pour votre entreprise, alors que les seaux supprimés ont été supprimés correctement (par exemple умолчанию 10%) ou когда seaux еще отсутствуют. Pot recommandé :

1. Exportez NDJSON depuis le relais de points de terminaison d'administration et vers l'orchestrateur `/v2/soranet/privacy/event|share` vers `artifacts/sorafs_privacy/<relay>.ndjson`.
2. Trouver un assistant pour la politique politique :

   ```bash
   cargo xtask soranet-privacy-report \
     --input artifacts/sorafs_privacy/relay-a.ndjson \
     --input artifacts/sorafs_privacy/relay-b.ndjson \
     --json-out artifacts/sorafs_privacy/relay_summary.json \
     --max-suppression-ratio 0.10
   ```

   La commande s'occupe de la sécurité et de la sécurité du robot dans un nouvel appartement, et les seaux sont déjà passés **ou** les seaux ne sont pas là les signaux, les signaux télémétriques ne sont pas programmés pour le programme. Les mesures Live devraient vous permettre de savoir que `soranet_privacy_pending_collectors` est en contact avec rien, et `soranet_privacy_snapshot_suppression_ratio` est là pour vous aider dans votre programme.
3. Enregistrez le fichier JSON et le journal CLI dans le paquet de documents SNNet-8 pour le transfert de données spécifique, qui contient les rapports les plus récents. воспроизвести точные ARTEFACTы.

## Следующие шаги (SNNet-8a)

- Intégrez deux collecteurs Prio, ajoutez des partages au runtime, des relais et des collecteurs en ajoutant des charges utiles `SoranetPrivacyBucketMetricsV1`. *(Готово — см. `ingest_privacy_payload` в `crates/sorafs_orchestrator/src/lib.rs` и сопутствующие тесты.)*
- Publier le tableau de bord Prometheus JSON et afficher les alertes, détecter les lacunes de suppression, localiser les collecteurs et les notifications anonymes. *(Готово — см. `dashboards/grafana/soranet_privacy_metrics.json`, `dashboards/alerts/soranet_privacy_rules.yml`, `dashboards/alerts/soranet_policy_rules.yml` et preuves.)*
- Ajouter des éléments de calibre d'art à des fins privées, en détail dans `privacy_metrics_dp.md`, y compris les blocs d'eau et le résumé de gouvernance. *(Готово — notebook et artéfacts générés `scripts/telemetry/run_privacy_dp.py` ; CI wrapper `scripts/telemetry/run_privacy_dp_notebook.sh` utilise le notebook pour le workflow `.github/workflows/release-pipeline.yml` ; résumé du résumé dans `docs/source/status/soranet_privacy_dp_digest.md`.)*

La version actuelle de SNNet-8 est disponible : détection, télémétrie privée, qui est liée à la connexion существующим скрейперам и дашbordам Prometheus. Les éléments de calibre spécial offrent des détails particuliers sur votre ordinateur portable, la fiabilité du flux de travail pour permettre à votre ordinateur portable de fonctionner correctement. Il est nécessaire de surveiller automatiquement les opérations et les analyses d'alerte de suppression.
---
lang: ja
direction: ltr
source: docs/portal/i18n/fr/docusaurus-plugin-content-docs/current/soranet/privacy-metrics-pipeline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a46cea0c9b8b59ed379d698c31182bbe8f40d9e688d7b4593f8398421922565f
source_last_modified: "2026-01-03T18:07:59+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for French (fr) translation. Replace this content with the full translation. -->

---
id: privacy-metrics-pipeline
lang: fr
direction: ltr
source: docs/portal/docs/soranet/privacy-metrics-pipeline.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Source canonique
Reflète `docs/source/soranet/privacy_metrics_pipeline.md`. Gardez les deux copies synchronisées jusqu'à ce que l'ancien ensemble de documentation soit retiré.
:::

# Pipeline des métriques de confidentialité SoraNet

SNNet-8 introduit une surface de télémétrie sensible à la confidentialité pour le runtime du relay. Le relay agrège désormais les événements de handshake et de circuit en buckets d'une minute et n'exporte que des compteurs Prometheus grossiers, gardant les circuits individuels non corrélables tout en donnant aux opérateurs une visibilité exploitable.

## Aperçu de l'agrégateur

- L'implémentation runtime vit dans `tools/soranet-relay/src/privacy.rs` sous `PrivacyAggregator`.
- Les buckets sont indexés par minute d'horloge (`bucket_secs`, par défaut 60 secondes) et stockés dans un anneau borné (`max_completed_buckets`, par défaut 120). Les shares de collecteurs conservent leur propre backlog borné (`max_share_lag_buckets`, par défaut 12) afin que les fenêtres Prio périmées soient vidées en buckets supprimés plutôt que de fuiter la mémoire ou de masquer des collecteurs bloqués.
- `RelayConfig::privacy` se mappe directement sur `PrivacyConfig`, exposant des réglages (`bucket_secs`, `min_handshakes`, `flush_delay_buckets`, `force_flush_buckets`, `max_completed_buckets`, `max_share_lag_buckets`, `expected_shares`). Le runtime de production conserve les valeurs par défaut tandis que SNNet-8a introduit des seuils d'agrégation sécurisée.
- Les modules runtime enregistrent les événements via des helpers typés : `record_circuit_accepted`, `record_circuit_rejected`, `record_throttle`, `record_throttle_cooldown`, `record_capacity_reject`, `record_active_sample`, `record_verified_bytes` et `record_gar_category`.

## Endpoint admin du relay

Les opérateurs peuvent interroger le listener admin du relay pour des observations brutes via `GET /privacy/events`. L'endpoint renvoie du JSON délimité par des nouvelles lignes (`application/x-ndjson`) contenant des payloads `SoranetPrivacyEventV1` reflétés depuis le `PrivacyEventBuffer` interne. Le buffer conserve les événements les plus récents jusqu'à `privacy.event_buffer_capacity` entrées (par défaut 4096) et est vidé à la lecture, donc les scrapers doivent sonder suffisamment souvent pour éviter des trous. Les événements couvrent les mêmes signaux de handshake, throttle, verified bandwidth, active circuit et GAR qui alimentent les compteurs Prometheus, permettant aux collecteurs en aval d'archiver des breadcrumbs sûrs pour la confidentialité ou d'alimenter des workflows d'agrégation sécurisée.

## Configuration du relay

Les opérateurs ajustent la cadence de télémétrie de confidentialité dans le fichier de configuration du relay via la section `privacy`:

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

Les valeurs par défaut des champs correspondent à la spécification SNNet-8 et sont validées au chargement :

| Champ | Description | Défaut |
|-------|-------------|--------|
| `bucket_secs` | Largeur de chaque fenêtre d'agrégation (secondes). | `60` |
| `min_handshakes` | Nombre minimum de contributeurs avant qu'un bucket puisse émettre des compteurs. | `12` |
| `flush_delay_buckets` | Nombre de buckets complétés à attendre avant d'essayer un flush. | `1` |
| `force_flush_buckets` | Âge maximal avant d'émettre un bucket supprimé. | `6` |
| `max_completed_buckets` | Backlog de buckets conservés (évite une mémoire non bornée). | `120` |
| `max_share_lag_buckets` | Fenêtre de rétention des shares collecteur avant suppression. | `12` |
| `expected_shares` | Shares Prio de collecteur requises avant combinaison. | `2` |
| `event_buffer_capacity` | Backlog d'événements NDJSON pour le flux admin. | `4096` |

Définir `force_flush_buckets` plus bas que `flush_delay_buckets`, mettre les seuils à zéro ou désactiver la garde de rétention échoue désormais la validation afin d'éviter des déploiements qui fuiraient la télémétrie par relay.

La limite `event_buffer_capacity` borne aussi `/admin/privacy/events`, garantissant que les scrapers ne puissent pas prendre un retard indéfini.

## Shares de collecteurs Prio

SNNet-8a déploie des collecteurs doubles qui émettent des buckets Prio à partage de secrets. L'orchestrator analyse désormais le flux NDJSON `/privacy/events` pour les entrées `SoranetPrivacyEventV1` et les shares `SoranetPrivacyPrioShareV1`, les transmettant à `SoranetSecureAggregator::ingest_prio_share`. Les buckets émettent une fois que `PrivacyBucketConfig::expected_shares` contributions arrivent, reflétant le comportement du relay. Les shares sont validées pour l'alignement des buckets et la forme de l'histogramme avant d'être combinées en `SoranetPrivacyBucketMetricsV1`. Si le nombre combiné de handshakes tombe sous `min_contributors`, le bucket est exporté comme `suppressed`, reflétant le comportement de l'agrégateur côté relay. Les fenêtres supprimées émettent désormais un label `suppression_reason` afin que les opérateurs puissent distinguer `insufficient_contributors`, `collector_suppressed`, `collector_window_elapsed` et `forced_flush_window_elapsed` lors du diagnostic des trous de télémétrie. La raison `collector_window_elapsed` se déclenche aussi lorsque les shares Prio traînent au-delà de `max_share_lag_buckets`, rendant les collecteurs bloqués visibles sans laisser d'accumulateurs périmés en mémoire.

## Endpoints d'ingestion Torii

Torii expose désormais deux endpoints HTTP protégés par la télémétrie afin que les relays et collecteurs puissent transmettre des observations sans embarquer un transport sur mesure :

- `POST /v1/soranet/privacy/event` accepte un payload `RecordSoranetPrivacyEventDto`. Le corps enveloppe un `SoranetPrivacyEventV1` plus un label `source` optionnel. Torii valide la requête contre le profil de télémétrie actif, enregistre l'événement et répond avec HTTP `202 Accepted` accompagné d'une enveloppe Norito JSON contenant la fenêtre calculée (`bucket_start_unix`, `bucket_duration_secs`) et le mode du relay.
- `POST /v1/soranet/privacy/share` accepte un payload `RecordSoranetPrivacyShareDto`. Le corps transporte un `SoranetPrivacyPrioShareV1` et un indice `forwarded_by` optionnel afin que les opérateurs puissent auditer les flux de collecteurs. Les soumissions réussies renvoient HTTP `202 Accepted` avec une enveloppe Norito JSON résumant le collecteur, la fenêtre de bucket et l'indication de suppression; les échecs de validation correspondent à une réponse de télémétrie `Conversion` afin de préserver un traitement d'erreur déterministe entre collecteurs. La boucle d'événements de l'orchestrator émet désormais ces shares lorsqu'elle interroge les relays, gardant l'accumulateur Prio de Torii synchronisé avec les buckets côté relay.

Les deux endpoints respectent le profil de télémétrie : ils émettent `503 Service Unavailable` lorsque les métriques sont désactivées. Les clients peuvent envoyer des corps Norito binaires (`application/x.norito`) ou Norito JSON (`application/x.norito+json`) ; le serveur négocie automatiquement le format via les extracteurs Torii standard.

## Métriques Prometheus

Chaque bucket exporté porte les labels `mode` (`entry`, `middle`, `exit`) et `bucket_start`. Les familles de métriques suivantes sont émises :

| Metric | Description |
|--------|-------------|
| `soranet_privacy_circuit_events_total{kind}` | Taxonomie des handshakes avec `kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}`. |
| `soranet_privacy_throttles_total{scope}` | Compteurs de throttle avec `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}`. |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` | Durées de cooldown agrégées apportées par des handshakes throttlés. |
| `soranet_privacy_verified_bytes_total` | Bande passante vérifiée issue de preuves de mesure aveugles. |
| `soranet_privacy_active_circuits_{avg,max}` | Moyenne et pic de circuits actifs par bucket. |
| `soranet_privacy_rtt_millis{percentile}` | Estimations des percentiles RTT (`p50`, `p90`, `p99`). |
| `soranet_privacy_gar_reports_total{category_hash}` | Compteurs de Governance Action Report hachés indexés par digest de catégorie. |
| `soranet_privacy_bucket_suppressed` | Buckets retenus parce que le seuil de contributeurs n'a pas été atteint. |
| `soranet_privacy_pending_collectors{mode}` | Accumulateurs de shares de collecteurs en attente de combinaison, groupés par mode de relay. |
| `soranet_privacy_suppression_total{reason}` | Compteurs de buckets supprimés avec `reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}` afin que les dashboards puissent attribuer les trous de confidentialité. |
| `soranet_privacy_snapshot_suppression_ratio` | Ratio supprimé/vidé du dernier drain (0-1), utile pour les budgets d'alerte. |
| `soranet_privacy_last_poll_unixtime` | Horodatage UNIX du dernier poll réussi (alimente l'alerte collector-idle). |
| `soranet_privacy_collector_enabled` | Gauge qui passe à `0` lorsque le collecteur de confidentialité est désactivé ou ne démarre pas (alimente l'alerte collector-disabled). |
| `soranet_privacy_poll_errors_total{provider}` | Échecs de polling groupés par alias de relay (incrémente lors d'erreurs de décodage, d'échecs HTTP ou de codes de statut inattendus). |

Les buckets sans observations restent silencieux, gardant les dashboards propres sans fabriquer de fenêtres remplies de zéros.

## Guidance opérationnelle

1. **Dashboards** - tracer les métriques ci-dessus groupées par `mode` et `window_start`. Mettre en évidence les fenêtres manquantes pour faire remonter les problèmes de collecteur ou de relay. Utiliser `soranet_privacy_suppression_total{reason}` pour distinguer les insuffisances de contributeurs des suppressions pilotées par les collecteurs lors du triage des trous. L'asset Grafana inclut désormais un panneau dédié **"Suppression Reasons (5m)"** alimenté par ces compteurs, plus un stat **"Suppressed Bucket %"** qui calcule `sum(soranet_privacy_bucket_suppressed) / count(...)` par sélection afin que les opérateurs repèrent les dépassements de budget en un coup d'œil. La série **"Collector Share Backlog"** (`soranet_privacy_pending_collectors`) et le stat **"Snapshot Suppression Ratio"** mettent en évidence les collecteurs bloqués et la dérive du budget pendant les exécutions automatisées.
2. **Alerting** - piloter les alertes à partir de compteurs sûrs pour la confidentialité : pics de rejet PoW, fréquence des cooldowns, dérive RTT et rejets de capacité. Comme les compteurs sont monotones à l'intérieur de chaque bucket, des règles de taux simples fonctionnent bien.
3. **Incident response** - s'appuyer d'abord sur les données agrégées. Lorsqu'un débogage plus profond est nécessaire, demander aux relays de rejouer des snapshots de buckets ou d'inspecter des preuves de mesure aveugles au lieu de récolter des journaux de trafic bruts.
4. **Retention** - scraper suffisamment souvent pour éviter de dépasser `max_completed_buckets`. Les exporters doivent traiter la sortie Prometheus comme source canonique et supprimer les buckets locaux une fois transmis.

## Analyse de suppression et exécutions automatisées

L'acceptation SNNet-8 dépend de la démonstration que les collecteurs automatisés restent sains et que la suppression reste dans les limites de la politique (≤10 % des buckets par relay sur toute fenêtre de 30 minutes). Les outils nécessaires pour satisfaire cette exigence sont maintenant livrés avec le dépôt ; les opérateurs doivent les intégrer à leurs rituels hebdomadaires. Les nouveaux panneaux de suppression Grafana reflètent les extraits PromQL ci-dessous, donnant aux équipes d'astreinte une visibilité en direct avant de recourir à des requêtes manuelles.

### Recettes PromQL pour la revue de suppression

Les opérateurs doivent garder les helpers PromQL suivants à portée de main ; les deux sont référencés dans le dashboard Grafana partagé (`dashboards/grafana/soranet_privacy_metrics.json`) et les règles Alertmanager :

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

Utilisez le ratio pour confirmer que le stat **"Suppressed Bucket %"** reste sous le budget de la politique ; branchez le détecteur de pics dans Alertmanager pour un retour rapide lorsque le nombre de contributeurs baisse de manière inattendue.

### CLI de rapport de bucket hors ligne

L'espace de travail expose `cargo xtask soranet-privacy-report` pour des captures NDJSON ponctuelles. Pointez-le sur un ou plusieurs exports admin de relay :

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```

L'outil fait passer la capture par `SoranetSecureAggregator`, imprime un résumé de suppression sur stdout et, en option, écrit un rapport JSON structuré via `--json-out <path|->`. Il respecte les mêmes réglages que le collecteur live (`--bucket-secs`, `--min-contributors`, `--expected-shares`, etc.), permettant aux opérateurs de rejouer des captures historiques sous des seuils différents lors du triage d'un incident. Joignez le JSON avec des captures Grafana afin que la porte d'analyse de suppression SNNet-8 reste auditable.

### Checklist de première exécution automatisée

La gouvernance exige toujours de prouver que la première exécution automatisée a respecté le budget de suppression. L'outil accepte désormais `--max-suppression-ratio <0-1>` afin que la CI ou les opérateurs puissent échouer rapidement lorsque les buckets supprimés dépassent la fenêtre autorisée (10 % par défaut) ou lorsqu'aucun bucket n'est encore présent. Flux recommandé :

1. Exporter du NDJSON depuis les endpoints admin du relay plus le flux `/v1/soranet/privacy/event|share` de l'orchestrator vers `artifacts/sorafs_privacy/<relay>.ndjson`.
2. Exécuter l'outil avec le budget de politique :

   ```bash
   cargo xtask soranet-privacy-report \
     --input artifacts/sorafs_privacy/relay-a.ndjson \
     --input artifacts/sorafs_privacy/relay-b.ndjson \
     --json-out artifacts/sorafs_privacy/relay_summary.json \
     --max-suppression-ratio 0.10
   ```

   La commande imprime le ratio observé et se termine avec un code non nul lorsque le budget est dépassé **ou** lorsqu'aucun bucket n'est prêt, signalant que la télémétrie n'a pas encore été produite pour l'exécution. Les métriques live doivent montrer `soranet_privacy_pending_collectors` se vidant vers zéro et `soranet_privacy_snapshot_suppression_ratio` restant sous le même budget pendant l'exécution.
3. Archiver la sortie JSON et le journal CLI avec le dossier de preuve SNNet-8 avant de basculer le transport par défaut afin que les réviseurs puissent rejouer les artefacts exacts.

## Prochaines étapes (SNNet-8a)

- Intégrer les collecteurs Prio doubles, en connectant leur ingestion de shares au runtime afin que les relays et collecteurs émettent des payloads `SoranetPrivacyBucketMetricsV1` cohérents. *(Fait — voir `ingest_privacy_payload` dans `crates/sorafs_orchestrator/src/lib.rs` et les tests associés.)*
- Publier le dashboard Prometheus partagé et les règles d'alerte couvrant les trous de suppression, la santé des collecteurs et les baisses d'anonymat. *(Fait — voir `dashboards/grafana/soranet_privacy_metrics.json`, `dashboards/alerts/soranet_privacy_rules.yml`, `dashboards/alerts/soranet_policy_rules.yml` et les fixtures de validation.)*
- Produire les artefacts de calibration de confidentialité différentielle décrits dans `privacy_metrics_dp.md`, y compris des notebooks reproductibles et des digests de gouvernance. *(Fait — notebook + artefacts générés par `scripts/telemetry/run_privacy_dp.py`; wrapper CI `scripts/telemetry/run_privacy_dp_notebook.sh` exécute le notebook via le workflow `.github/workflows/release-pipeline.yml`; digest de gouvernance déposé dans `docs/source/status/soranet_privacy_dp_digest.md`.)*

La version actuelle livre la fondation SNNet-8 : une télémétrie déterministe et sûre pour la confidentialité qui s'intègre directement aux scrapers Prometheus et aux dashboards. Les artefacts de calibration de confidentialité différentielle sont en place, le workflow du pipeline de release maintient les sorties du notebook à jour, et le travail restant se concentre sur la surveillance de la première exécution automatisée plus l'extension des analyses d'alerte de suppression.

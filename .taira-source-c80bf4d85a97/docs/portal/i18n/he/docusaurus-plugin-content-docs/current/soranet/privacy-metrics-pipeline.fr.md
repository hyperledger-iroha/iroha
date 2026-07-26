---
lang: he
direction: rtl
source: docs/portal/docs/soranet/privacy-metrics-pipeline.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: privacy-metrics-pipeline
כותרת: Pipeline des métriques de confidentialité SoraNet (SNNet-8)
sidebar_label: Pipeline des métriques de confidentialité
תיאור: Collecte de télémétrie preservant la confidentialité pour les relays and orchestrators SoraNet.
---

:::הערה מקור קנוניק
Reflète `docs/source/soranet/privacy_metrics_pipeline.md`. Gardez les deux copies Syncées jusqu'à ce que l'ancien ensemble de documentation soit retiré.
:::

# Pipeline des métriques de confidentialité SoraNet

SNNet-8 הציג את פני השטח של טלמטריה הגיוני א לה סודיות עבור זמן ריצה של ממסר. Le relay agrège désormais les événements de handshake et de circuit en buckets d'une minute et n'exporte que des compteurs Prometheus grossiers, gardant les circuits individuals non correlables tout and donnant aux opérateurs une visibilité.

## Aperçu de l'agrégateur

- L'implementation runtime vit dans `tools/soranet-relay/src/privacy.rs` sous `PrivacyAggregator`.
- Les buckets sont indexés par minute d'horloge (`bucket_secs`, par défaut 60 secondes) et stockés dans un anneau borné (`max_completed_buckets`, par défaut 120). Les shares de collecteurs conservent leur propre backlog borné (`max_share_lag_buckets`, par défaut 12) afin que les fenêtres Prio périmées soient vidées en buckets supprimés plutôt que de fuiter la mémoire ou de masquer des collecteurs.
- `RelayConfig::privacy` se map directement sur `PrivacyConfig`, exposant des réglages (`bucket_secs`, `min_handshakes`, `flush_delay_buckets`, Prometheus0000306, Prometheus,0003500 `max_share_lag_buckets`, `expected_shares`). Le runtime de production conserve les valeurs par défaut tandis que SNNet-8a introduit des seuils d'agrégation sécurisée.
- מודולי זמן ריצה רושמים ציוד דרך סוגים של עוזרים: `record_circuit_accepted`, `record_circuit_rejected`, `record_throttle`, `record_throttle_cooldown`, Prometheus, Prometheus, `record_verified_bytes` et `record_gar_category`.

## מנהל נקודת קצה של ממסר

Les מפעילי חוקר האזינו מנהל ממסר pour des observations brutes דרך `GET /privacy/events`. L'endpoint renvoie du JSON délimité par des nouvelles lignes (`application/x-ndjson`) contenant des payloads `SoranetPrivacyEventV1` reflétés depuis le `PrivacyEventBuffer` intern. Le buffer conserve les événements les plus récents jusqu'à `privacy.event_buffer_capacity` entrées (par défaut 4096) et est vidé à la lecture, donc les scrapers doivent sonder suffisamment souvent pour éviter des trous. Les événements couvrent les mêmes signaux de לחיצת יד, מצערת, רוחב פס מאומת, מעגל אקטיבי ו-GAR qui alimentent les compteurs Prometheus, קבוע aux collecteurs en aval d'archiver des breadcrumbs sûrs pour ou la work'alimenter sécurisée.

## תצורת ממסר

מפעילי ההפעלה מתאיימים ל-cadence de télémétrie de confidentialité dans le fichier de configuration du relay via la section `privacy`:

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

Les valeurs par défaut des champs correspondent à la spécification SNNet-8 et sont validées au chargement:| אלוף | תיאור | דפאוט |
|-------|-------------|--------|
| `bucket_secs` | Largeur de chaque fenêtre d'agrégation (שניות). | `60` |
| `min_handshakes` | Nombre Minimum de Contribuurs avant qu'un bucket puisse émettre des compteurs. | `12` |
| `flush_delay_buckets` | Nombre de buckets complétés à attendre avant d'essayer un flush. | `1` |
| `force_flush_buckets` | Âge maximal avant d'émettre un bucket supprimé. | `6` |
| `max_completed_buckets` | Backlog de buckets conservés (évite une mémoire non bornée). | `120` |
| `max_share_lag_buckets` | Fenêtre de rétention des מניות אספן אוונג דיכוי. | `12` |
| `expected_shares` | מניות פריו דה אספן מחייב שילוב אוונגני. | `2` |
| `event_buffer_capacity` | Backlog d'événements NDJSON pour le flux admin. | `4096` |

Définir `force_flush_buckets` plus bas que `flush_delay_buckets`, mettre les seuils à zero ou désactiver la garde de rétention échoue désormais la validation afin d'éviter des déploiements qui fuiraient la télémétrie par.

La limite `event_buffer_capacity` borne aussi `/admin/privacy/events`, garantissant que les scrapers ne puissent pas prendre un retard indéfini.

## Shares de collecteurs Prio

SNNet-8a déploie des collecteurs כפול qui émettent des buckets Prio à partage de secrets. L'orchestrator analize désormais le flux NDJSON `/privacy/events` pour les entrées `SoranetPrivacyEventV1` et les shares `SoranetPrivacyPrioShareV1`, les transmettant à `SoranetSecureAggregator::ingest_prio_share`. Les emettent une fois que `PrivacyBucketConfig::expected_shares` תרומות הגיע, reflétant le comportement du relay. Les shares sont validées pour l'alignement des buckets et la forme de l'histogramme avant d'être combinées en `SoranetPrivacyBucketMetricsV1`. לחיצות ידיים משולבות לחיצות ידיים `min_contributors`, דלי יצוא במקור `suppressed`, ממסר ממסר למחסן. Les fenêtres supprimées émettent désormais un label `suppression_reason` afin que les opérateurs puissent distinguer `insufficient_contributors`, `collector_suppressed`, `collector_window_elapsed` et I100NI800s de troulors de diagnostic télémétrie. La raison `collector_window_elapsed` se déclenche aussi lorsque les shares Prio traînent au-delà de `max_share_lag_buckets`, rendant les collecteurs bloqués visibles sans laisser d'accumulateurs périmés en mémoire.

## נקודות קצה לבליעה Torii

Torii לחשוף את נקודות הקצה של נקודת הקצה של HTTP פרוטégés par la télémétrie afin que les relays et collecteurs puissent transmettre des observations sans embarquer un transport sur mesure :- `POST /v1/soranet/privacy/event` מקבל מטען `RecordSoranetPrivacyEventDto`. מעטפת החיל un `SoranetPrivacyEventV1` פלוס un label `source` optionnel. Torii valide la requête contre le profil de télémétrie actif, registre l'événement et répond avec HTTP `202 Accepted` accompagné d'une enveloppe Norito JSON contenantcalcule la (`bucket_start_unix`, `bucket_duration_secs`) et le mode du relay.
- `POST /v1/soranet/privacy/share` מקבל מטען `RecordSoranetPrivacyShareDto`. Le corps transporte un `SoranetPrivacyPrioShareV1` et un indice `forwarded_by` optionnel afin que les opérateurs puissent auditer les flux de collecteurs. Les soumissions réussies renvoient HTTP `202 Accepted` avec une enveloppe Norito JSON résumant le collecteur, la fenêtre de bucket et l'indication de suppression; les échecs de validation correspondent à une réponse de télémétrie `Conversion` afin de préserver un traitement d'erreur déterministe entre collectors. La boucle d'événements de l'orchestrator émet désormais ces מניות lorsqu'elle interroge les relays, gardant l'accumulateur Prio de Torii Syncé avec les buckets côté relay.

Les deux endpoints respectent le profil de télémétrie: ils émettent `503 Service Unavailable` lorsque les métriques sont désactivées. Les clients peuvent envoyer des corps Norito binaires (`application/x.norito`) או Norito JSON (`application/x.norito+json`) ; le serveur négocie automatiquement le format via les extracteurs Torii תקן.

## Métriques Prometheus

דלי צ'אק ייצא את תוויות `mode` (`entry`, `middle`, `exit`) ו-`bucket_start`. Les familles de métriques suivantes sont émises :| מדד | תיאור |
|--------|----------------|
| `soranet_privacy_circuit_events_total{kind}` | Taxonomie des handshakes avec `kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}`. |
| `soranet_privacy_throttles_total{scope}` | מחשבי מצערת avec `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}`. |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` | Durées de cooldown agrégées apportées par des trottles לחיצות ידיים. |
| `soranet_privacy_verified_bytes_total` | Bande passante vérifiée issue de preuves de mesure aveugles. |
| `soranet_privacy_active_circuits_{avg,max}` | Moyenne et pic de circuits actifs par bucket. |
| `soranet_privacy_rtt_millis{percentile}` | אומדנים des percentiles RTT (`p50`, `p90`, `p99`). |
| `soranet_privacy_gar_reports_total{category_hash}` | Compteurs de Governance Action Report יש אינדקסים לקטגוריה. |
| `soranet_privacy_bucket_suppressed` | דליים retenus parce que le seuil de contribuurs n'a pas été atteint. |
| `soranet_privacy_pending_collectors{mode}` | צוברים מניות אספנים בשילוב קבוצות, קבוצות ממסרות. |
| `soranet_privacy_suppression_total{reason}` | Compteurs de buckets supprimés avec `reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}` afin que les לוחות מחוונים puissent attributer les trous de confidentialité. |
| `soranet_privacy_snapshot_suppression_ratio` | Ratio supprimé/vidé du dernier ניקוז (0-1), utile pour les budgets d'alerte. |
| `soranet_privacy_last_poll_unixtime` | Horodatage UNIX du dernier poll réussi (alimente l'alerte אספן-בטלה). |
| `soranet_privacy_collector_enabled` | מד qui pass à `0` lorsque le collecteur de confidentialité est désactivé ou ne démarre pas (alimente l'alerte אספן-נכה). |
| `soranet_privacy_poll_errors_total{provider}` | Échecs de polling groupés par alias de relay (incrémente lors d'erreurs de décodage, d'échecs HTTP ou de codes de statut inattendus). |

Les buckets sans observations restent silencieux, gardant les dashboards propres sans fabriquer de fenêtres remplies de zéros.

## הדרכה מבצעית1. **לוחות מחוונים** - tracer les métriques ci-dessus groupées par `mode` et `window_start`. Mettre en évidence les fenêtres manquantes pour faire remonter les problèmes de collecteur ou de relay. Utiliser `soranet_privacy_suppression_total{reason}` pour distinguer les insuffisances de contribuurs des suppressions pilotées par les collecteurs lors du triage des trous. L'asset Grafana inclut désormais un panneau dédié **"Suppression Reasons (5m)"** alimenté par ces compteurs, plus un stat **"Suppressed Bucket %"** qui calculate `sum(soranet_privacy_bucket_suppressed) / count(...)` par sélection deslection deslection budget définsateur ד'יל. La serie **"Corm Share Backlog"** (`soranet_privacy_pending_collectors`) et le stat **"Snapshot Suppression Ratio"** mettent en évidence les collectors bloqués et la dérive du budget pendant les exécutions automatisées.
2. **התראה** - טייס אזעקות על ידי מחשבים על סודיות: תמונות של דחיית PoW, תדירות התקררות, העברת RTT ו-rejets de capacité. Comme les compteurs sont monotones à l'intérieur de chaque bucket, des règles de taux simples fonctionnent bien.
3. **תגובה לאירוע** - s'appuyer d'abord sur les données agrégées. Lorsqu'un débogage plus profond est nécessaire, דורש aux relays de rejouer des snapshots de buckets או d'inspecter des preuves de mesure aveugles au lieu de récolter des journaux de trafic bruts.
4. **שמירה** - מגרד suffisamment souvent pour éviter de dépasser `max_completed_buckets`. היצואנים לא מוצאים את דרכו Prometheus במקור קנוניק וסופר דליים מוצאים את זה.

## ניתוח דיכוי וביצועים אוטומטיים

L'acceptation SNNet-8 dépend de la démonstration que les collecteurs automatisés restent sains et que la suppression reste dans les limites de la politique (≤10% des buckets par relay sur toute fenêtre de 30 דקות). Les outils nécessaires pour satisfaire cette exigence sont maintenant livrés avec le dépôt; les opérateurs doivent les intégrer à leurs rituels hebdomadaires. Les nouveaux panneaux de suppression Grafana reflètent les extraits PromQL ci-dessous, donnant aux équipes d'astreinte une visibilité en direct avant de recourir à des requêtes manuelles.

### Recettes PromQL pour la revue de suppression

Les opérateurs doivent garder les helpers PromQL suivants à portée de main; les deux sont référencés dans le לוח המחוונים Grafana partagé (`dashboards/grafana/soranet_privacy_metrics.json`) et les règles Alertmanager :

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

Utilisez le ratio pour confirmer que le stat **"דלי מודחק %"** reste sous le budget de la politique; branchez le détecteur de pics dans Alertmanager pour un retour rapide lorsque le nombre de contribuurs baisse de manière inattendue.

### CLI de rapport de bucket hors ligne

L'espace de travail expose `cargo xtask soranet-privacy-report` pour des לוכדת נקודות NDJSON. Pointez-le sur un ou plusieurs ייצוא מנהל ממסר:

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```L'outil fait passer la capture par `SoranetSecureAggregator`, הצג את קורות החיים של דיכוי על סטדאוט et, באופציה, ecrit un rapport JSON structuré דרך `--json-out <path|->`. Il respecte les mêmes réglages que le collecteur בשידור חי (`--bucket-secs`, `--min-contributors`, `--expected-shares`, וכו'), Permettant aux opérateurs de rejouer des captures historiques sous des seuillor dunférent incident. Joignez le JSON avec des captures Grafana ניתן לביקורת.

### רשימת רשימת ביצוע אוטומטית

La governance exige toujours de prouver que la première exécution automatisée a respecté le budget de suppression. L'outil accepte désormais `--max-suppression-ratio <0-1>` afin que la CI ou les opérateurs puissent échouer rapidement lorsque les buckets supprimés dépassent la fenêtre autorisée (10% par défaut) או lorsqu'aucun bucket n'est הדרן présent. המלצת שטף:

1. Exporter du NDJSON depuis les endpoints admin du relay plus le flux `/v1/soranet/privacy/event|share` de l'orchestrator vers `artifacts/sorafs_privacy/<relay>.ndjson`.
2. מנהל תקציב פוליטי:

   ```bash
   cargo xtask soranet-privacy-report \
     --input artifacts/sorafs_privacy/relay-a.ndjson \
     --input artifacts/sorafs_privacy/relay-b.ndjson \
     --json-out artifacts/sorafs_privacy/relay_summary.json \
     --max-suppression-ratio 0.10
   ```

   La commande imprime le ratio observé et se termine avec un code non null lorsque le budget est dépassé **ou** lorsqu'aucun bucket n'est prêt, signalant que la télémétrie n'a pas encore été produite pour l'exécution. Les métriques live doivent montrer `soranet_privacy_pending_collectors` se vidant vers zéro et `soranet_privacy_snapshot_suppression_ratio` restant sous le même budget pendant l'exécution.
3. Archiver la sortie JSON et le journal CLI avec le dossier de preuve SNNet-8 avant de basculer le transport par défaut afin que les réviseurs puissent rejouer les artefacts exacts.

## Prochaines étapes (SNNet-8a)

- Intégrer les collecteurs Prio doubles, and connectant leur ingestion de shares au runtime afin que les relays and collecteurs émettent des payloads `SoranetPrivacyBucketMetricsV1` cohérents. *(Fait — voir `ingest_privacy_payload` dans `crates/sorafs_orchestrator/src/lib.rs` et les tests associés.)*
- Publier le Dashboard Prometheus partagé et les règles d'alerte couvrant les trous de suppression, la santé des collecteurs et les baisses d'anonymat. *(Fait — voir `dashboards/grafana/soranet_privacy_metrics.json`, `dashboards/alerts/soranet_privacy_rules.yml`, `dashboards/alerts/soranet_policy_rules.yml` et les fixtures de validation.)*
- Produire les artefacts de calibration de cfidentialité différentielle décrits dans `privacy_metrics_dp.md`, y compris des reproductibles reproductibles et des digests de gouvernance. *(Fait — מחברת + חפצי אמנות générés par `scripts/telemetry/run_privacy_dp.py`; עטיפה CI `scripts/telemetry/run_privacy_dp_notebook.sh` בצע את המחברת באמצעות זרימת העבודה `.github/workflows/release-pipeline.yml`; digest de governance déposé dans Prometheus.*La version actuelle livre la fondation SNNet-8: une télémétrie déterministe et sûre pour la confidentialité qui s'intègre directement aux scrapers Prometheus et aux לוחות מחוונים. Les artefacts de calibration de confidentialité différentielle sont an place, workflow du pipeline de release maintient les sorties du notebook à jour, and the travail restant se concentre sur la surveillance de la première exécution automatisée plus l'extension des analysis d'alerte depression.
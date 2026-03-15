---
lang: fr
direction: ltr
source: docs/portal/docs/soranet/privacy-metrics-pipeline.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : pipeline de mesures de confidentialité
titre : SoraNet پرائیویسی میٹرکس لائن (SNNet-8)
sidebar_label : پرائیویسی میٹرکس پائپ لائن
description : SoraNet relais et orchestrateurs et systèmes de télémétrie
---

:::note مستند ماخذ
`docs/source/soranet/privacy_metrics_pipeline.md` pour le client پرانے docs کے ختم ہونے تک دونوں نقول ہم وقت رکھیں۔
:::

# SoraNet prend en charge votre projet

Durée d'exécution du relais SNNet-8 pour les applications de télémétrie et de télémétrie relais pour poignée de main et circuit et compteurs Prometheus compteurs جس سے انفرادی circuits unlinkable رہتے ہیں جبکہ آپریٹرز کو قابلِ عمل بصیرت ملتی ہے۔

## Agrégateur en ligne

- exécution et implémentation `tools/soranet-relay/src/privacy.rs` et `PrivacyAggregator` pour le système d'exécution
- seaux et horloge murale avec clé et clé (`bucket_secs`, 60 secondes par défaut) et avec anneau (`max_completed_buckets`, par défaut) 120) میں رکھا جاتا ہے۔ les partages de collecteurs ont supprimé le backlog (`max_share_lag_buckets`, par défaut 12) et les fenêtres Prio ont été supprimées et les buckets ont été supprimés. نہ جائیں۔
- `RelayConfig::privacy` pour les boutons de réglage `PrivacyConfig` avec carte et boutons de réglage (`bucket_secs`, `min_handshakes`, `flush_delay_buckets`, `force_flush_buckets`, `max_completed_buckets`, `max_share_lag_buckets`, `expected_shares`) ظاہر کرتا ہے۔ valeurs par défaut de l'exécution de la production Seuils d'agrégation sécurisés SNNet-8a
- Les assistants typés d'exécution pour les événements suivants sont les suivants : `record_circuit_accepted`, `record_circuit_rejected`, `record_throttle`, `record_throttle_cooldown`, `record_capacity_reject`, `record_active_sample`, `record_verified_bytes`, et `record_gar_category`۔

## Point de terminaison de l'administrateur du relais

آپریٹرز `GET /privacy/events` کے ذریعے relay کے admin auditeur کو sondage کر کے observations brutes لے سکتے ہیں۔ JSON délimité par une nouvelle ligne de point de terminaison (`application/x-ndjson`) et charges utiles `SoranetPrivacyEventV1` et charges utiles `PrivacyEventBuffer` سے miroir ہوتے ہیں۔ tampon pour les événements et les entrées `privacy.event_buffer_capacity` pour les grattoirs (par défaut 4096) et pour lire les drains et les grattoirs et les espaces vides کیلئے کافی بار sondage کرنا چاہئے۔ événements, prise de contact, accélérateur, bande passante vérifiée, circuit actif, signaux GAR, compteurs Prometheus et collecteurs en aval Ajouter du fil d'Ariane à des flux de travail d'agrégation sécurisés et des flux

## Configuration du relais

Configuration du relais pour `privacy` Configuration du relais de cadence de télémétrie:

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

Les champs par défaut des spécifications SNNet-8 sont chargés et chargés et validés:| Champ | Descriptif | Par défaut |
|-------|-------------|---------|
| `bucket_secs` | ہر fenêtre d'agrégation کی چوڑائی (secondes)۔ | `60` |
| `min_handshakes` | Le nombre de contributeurs est élevé et les compteurs de seau émettent plus de 10 000 000 $ | `12` |
| `flush_delay_buckets` | rincer les seaux et les seaux | `1` |
| `force_flush_buckets` | émission de seau supprimée کرنے سے پہلے زیادہ سے زیادہ عمر۔ | `6` |
| `max_completed_buckets` | Plus de backlog de compartiment (mémoire illimitée pour les utilisateurs)۔ | `120` |
| `max_share_lag_buckets` | suppression des actions de collectionneur et fenêtre de rétention | `12` |
| `expected_shares` | combiner les actions de collectionneur Prio | `2` |
| `event_buffer_capacity` | flux d'administration et backlog d'événements NDJSON | `4096` |

`force_flush_buckets` et `flush_delay_buckets` sont des seuils de sécurité pour un garde de rétention et un échec de validation en cas d'échec de validation. Les déploiements en cours et les fuites de télémétrie par relais sont également possibles.

`event_buffer_capacity` pour `/admin/privacy/events` pour les grattoirs reliés et les grattoirs pour les mains جائیں۔

## Actions de collection Prio

Les doubles collecteurs SNNet-8a émettent des particules et des seaux Prio partagés en secret émettent des particules. orchestrator pour `/privacy/events` Flux NDJSON et entrées `SoranetPrivacyEventV1` pour `SoranetPrivacyPrioShareV1` partages et analyse parse pour `SoranetSecureAggregator::ingest_prio_share` میں forward کرتا ہے۔ les seaux émettent généralement des contributions `PrivacyBucketConfig::expected_shares` pour le comportement du relais et le comportement du relais partages et alignement du seau et forme de l'histogramme valider les actions et les actions `SoranetPrivacyBucketMetricsV1` et combiner les actions Le nombre de poignées de main combinées `min_contributors` est un exemple de seau et `suppressed` qui permet d'exporter des données et le comportement de l'agrégateur en relais جیسا ہے۔ les fenêtres supprimées de l'étiquette `suppression_reason` émettent des messages d'erreur `insufficient_contributors`, `collector_suppressed`, `collector_window_elapsed`, et `forced_flush_window_elapsed` Il y a des lacunes dans la télémétrie et il y a des lacunes dans la télémétrie. `collector_window_elapsed` est un incendie pour les actions Prio `max_share_lag_buckets` est un collecteur bloqué pour les collectionneurs bloqués آتے ہیں اور mémoire d'accumulateurs périmés میں نہیں رہتے۔

## Torii Points de terminaison d'ingestion

Torii pour les points de terminaison HTTP contrôlés par télémétrie et les relais et collecteurs pour le transport sur mesure et les observations en avant:- `POST /v1/soranet/privacy/event` et charge utile `RecordSoranetPrivacyEventDto` pour la charge utile corps `SoranetPrivacyEventV1` en option `source` étiquette en option Torii demande un profil de télémétrie pour valider un événement et un événement HTTP `202 Accepted` est disponible Norito Enveloppe JSON et mode relais avec fenêtre de compartiment calculée (`bucket_start_unix`, `bucket_duration_secs`) et mode relais
- `POST /v1/soranet/privacy/share` et charge utile `RecordSoranetPrivacyShareDto` pour la charge utile corps `SoranetPrivacyPrioShareV1` et optionnel `forwarded_by` indice de contrôle des flux de collecteur et audit des flux de collecteur Soumissions HTTP `202 Accepted` avec enveloppe Norito JSON et collecteur, fenêtre de compartiment, indice de suppression et résumé des informations. échecs de validation télémétrie `Conversion` réponse carte carte collecteurs collecteurs gestion déterministe des erreurs gestion des erreurs orchestrator pour boucle d'événements et interrogation de relais pour les actions émettent des actions pour Torii et pour les seaux de relais d'accumulateur Prio pour la synchronisation

Voici le profil de télémétrie des points de terminaison et le lien : métriques désactivées par rapport à `503 Service Unavailable` et le lien ci-dessous. clients Norito binaire (`application/x.norito`) et Norito JSON (`application/x.norito+json`) corps pour les clients serveur standard Torii extracteurs format de fichier pour négocier des fichiers

## Prometheus métriques

Pour le seau exporté `mode` (`entry`, `middle`, `exit`) et les étiquettes `bucket_start` Les familles métriques émettent des valeurs différentes :

| Métrique | Descriptif |
|--------|-------------|
| `soranet_privacy_circuit_events_total{kind}` | taxonomie de poignée de main par `kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}` par ہے۔ |
| `soranet_privacy_throttles_total{scope}` | compteurs de gaz pour `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}` ہے۔ |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` | poignées de main étranglées et temps de recharge مدتوں کا مجموعہ۔ |
| `soranet_privacy_verified_bytes_total` | preuves de mesure en aveugle سے bande passante vérifiée۔ |
| `soranet_privacy_active_circuits_{avg,max}` | ہر bucket میں circuits actifs کا اوسط اور زیادہ سے زیادہ۔ |
| `soranet_privacy_rtt_millis{percentile}` | Estimations centiles RTT (`p50`, `p90`, `p99`)۔ |
| `soranet_privacy_gar_reports_total{category_hash}` | Compteurs hachés du rapport d'action sur la gouvernance et résumé des catégories et clé ہوتے ہیں۔ |
| `soranet_privacy_bucket_suppressed` | Et seaux et seuil de contributeur |
| `soranet_privacy_pending_collectors{mode}` | accumulateurs de parts de collectionneur et combiner des accumulateurs en mode relais en mode relais |
| `soranet_privacy_suppression_total{reason}` | compteurs de seau supprimés par `reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}` pour les lacunes en matière de confidentialité des tableaux de bord et les lacunes dans la confidentialité des tableaux de bord |
| `soranet_privacy_snapshot_suppression_ratio` | آخری drainer کا rapport supprimé/vidé (0-1) ، budgets d'alerte کیلئے مفید۔ |
| `soranet_privacy_last_poll_unixtime` | Il s'agit d'un sondage sur l'horodatage UNIX (alerte d'inactivité du collecteur et d'un horodatage UNIX) |
| `soranet_privacy_collector_enabled` | jauge et `0` pour le collecteur de confidentialité et démarrer le système d'alerte (alerte de désactivation du collecteur) |
| `soranet_privacy_poll_errors_total{provider}` | échecs d'interrogation et alias de relais (erreurs de décodage, échecs HTTP, codes d'état et codes d'état) |Il y a des seaux d'observations et des tableaux de bord, des fenêtres remplies de zéro et des fenêtres remplies de zéro.

## Orientation opérationnelle

1. **Tableaux de bord** - métriques et métriques `mode` et `window_start` pour les métriques `mode` et `window_start`. fenêtres manquantes et mise en surbrillance du collecteur et du relais `soranet_privacy_suppression_total{reason}` Réduire les écarts et le triage du déficit de contributeurs et la suppression par les collecteurs pour résoudre le problème Grafana actif ici dédié **"Suppression Reasons (5m)"** panel فراہم کرتا ہے جو ان counters سے چلتا ہے، ساتھ ہی ایک **"Suppressed Bucket %"** stat Dans `sum(soranet_privacy_bucket_suppressed) / count(...)`, la sélection des cas de violation du budget est en cours. **Série « Collector Share Backlog »** (`soranet_privacy_pending_collectors`) et **« Snapshot Suppression Ratio »** statistiques des collecteurs bloqués et exécutions automatisées et dérive du budget et de la dérive budgétaire.
2. **Alertes** - compteurs sécurisés et alarmes telles que : pics de rejet de PoW, fréquence de refroidissement, dérive RTT et rejets de capacité Il s'agit d'un seau et d'un compteur de compteurs monotones, ainsi que de règles basées sur les taux.
3. **Réponse aux incidents** - پہلے données agrégées پر انحصار کریں۔ Il s'agit du débogage, des relais, des instantanés de seau, de la relecture et des preuves de mesure en aveugle, ainsi que des journaux de trafic bruts.
4. **Rétention** - `max_completed_buckets` pour gratter le papier exportateurs vers la sortie Prometheus et la source canonique vers l'avant vers les compartiments locaux vers les compartiments locaux

## Analyses de suppression et exécutions automatisées

SNNet-8 est un système de collecte automatique de collecteurs automatisés et de suppression de collecteurs automatiques (relais (10% des seaux) (avec une fenêtre de 30 min) اس gate کو پورا کرنے کیلئے درکار outillage اب repo کے ساتھ آتا ہے؛ آپریٹرز کو اسے اپنی ہفتہ وار rituels میں شامل کرنا ہوگا۔ Panneaux de suppression Grafana pour les extraits de code PromQL et la visibilité en direct اس سے پہلے کہ انہیں requêtes manuelles کی ضرورت پڑے۔

### Examen de la suppression des recettes PromQL

Les assistants PromQL sont également des assistants de PromQL. Il existe un tableau de bord Grafana partagé (`dashboards/grafana/soranet_privacy_metrics.json`) et des règles Alertmanager référencées ici :

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

rapport de sortie استعمال کریں تاکہ **"Suppressed Bucket %"** stat پالیسی budget سے نیچے رہے؛ Spike Detector et Alertmanager ont contribué à l'analyse des contributeurs et des membres de notre équipe.

### CLI du rapport de compartiment hors ligne

espace de travail `cargo xtask soranet-privacy-report` et NDJSON capture les captures d'écran Voici les exportations de l'administrateur du relais vers le point suivant :

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```Capture d'aide `SoranetSecureAggregator` pour le flux de données sur la sortie standard et le résumé de suppression du résumé de la suppression. `--json-out <path|->` est un rapport JSON structuré en anglais یہ live collector et boutons (`--bucket-secs`, `--min-contributors`, `--expected-shares`, وغیرہ) et honorer les problèmes de triage میں مختلف seuils کے ساتھ captures historiques rediffusion Porte d'analyse de suppression SNNet-8 et audit pour les captures d'écran JSON et les captures d'écran Grafana en pièce jointe

### پہلی liste de contrôle d'exécution automatisée

Gouvernance اب بھی ثبوت چاہتا ہے کہ پہلی budget de suppression d'exécution d'automatisation helper `--max-suppression-ratio <0-1>` est un outil de gestion de CI qui échoue rapidement pour supprimer la fenêtre autorisée des compartiments supprimés. (10 par défaut) Voici le flux :

1. Point(s) de terminaison d'administrateur de relais pour orchestrateur et flux `/v1/soranet/privacy/event|share` et exportation NDJSON pour `artifacts/sorafs_privacy/<relay>.ndjson` pour le flux de données.
2. budget politique کے ساتھ assistant چلائیں :

   ```bash
   cargo xtask soranet-privacy-report \
     --input artifacts/sorafs_privacy/relay-a.ndjson \
     --input artifacts/sorafs_privacy/relay-b.ndjson \
     --json-out artifacts/sorafs_privacy/relay_summary.json \
     --max-suppression-ratio 0.10
   ```

   Le ratio observé est élevé et la sortie non nulle est supérieure à 40% du budget. ملتا ہے کہ télémétrie ابھی run کیلئے پیدا نہیں ہوئی۔ métriques en direct pour le budget `soranet_privacy_pending_collectors` et le drain pour le budget `soranet_privacy_snapshot_suppression_ratio` رہتا ہے جب run چل رہا ہو۔
3. transport par défaut pour la sortie JSON et le journal CLI et l'ensemble de preuves SNNet-8 et les archives, les réviseurs et les artefacts

## Prochaines étapes (SNNet-8a)

- Les collecteurs Prio doubles intègrent des relais et des partages d'ingestion et d'exécution et des relais de connexion et des collecteurs cohérents, les charges utiles `SoranetPrivacyBucketMetricsV1` émettent des charges utiles `SoranetPrivacyBucketMetricsV1`. *(Fait — `crates/sorafs_orchestrator/src/lib.rs` میں `ingest_privacy_payload` اور متعلقہ tests دیکھیں۔)*
- tableau de bord Prometheus partagé JSON et règles d'alerte pour les lacunes de suppression, la santé des collecteurs et les baisses de tension d'anonymat et la couverture des problèmes *(Terminé — `dashboards/grafana/soranet_privacy_metrics.json`, `dashboards/alerts/soranet_privacy_rules.yml`, `dashboards/alerts/soranet_policy_rules.yml` et appareils de validation ici)*
- `privacy_metrics_dp.md` utilise des artefacts d'étalonnage de confidentialité différentielle pour créer des cahiers reproductibles et des résumés de gouvernance. *(Terminé – artefacts de bloc-notes `scripts/telemetry/run_privacy_dp.py` et wrapper CI `scripts/telemetry/run_privacy_dp_notebook.sh` bloc-notes et workflow `.github/workflows/release-pipeline.yml` et résumé de gouvernance `docs/source/status/soranet_privacy_dp_digest.md` (voir photo)*

La version SNNet-8 est basée sur la télémétrie déterministe et sécurisée pour la confidentialité, ainsi que sur les grattoirs Prometheus et les tableaux de bord. ہوتی ہے۔ les artefacts d'étalonnage de confidentialité différentielle permettent de publier les sorties du bloc-notes du flux de travail du pipeline ainsi que les analyses d'alerte de suppression et d'exécution automatisée. مرکوز ہے۔
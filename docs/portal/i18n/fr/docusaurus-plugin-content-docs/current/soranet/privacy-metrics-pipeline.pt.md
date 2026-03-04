---
lang: fr
direction: ltr
source: docs/portal/docs/soranet/privacy-metrics-pipeline.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : pipeline de mesures de confidentialité
titre : Pipeline de mesures de confidentialité de SoraNet (SNNet-8)
sidebar_label : Pipeline de mesures de confidentialité
description : Collette de télémétrie qui préserve la confidentialité des relais et des orchestrateurs de SoraNet.
---

:::note Fonte canonica
Reflète `docs/source/soranet/privacy_metrics_pipeline.md`. Mantenha ambas comme copies synchronisées.
:::

# Pipeline de mesures de confidentialité de SoraNet

SNNet-8 introduit une surface de télémétrie respectueuse de la confidentialité pour l'exécution du relais. Le relais vient maintenant d'ajouter des événements de poignée de main et de circuit dans des seaux d'une minute et d'exporter des contacts Prometheus grossiers, des circuits individuels desvinculados fournissant une visibilité active aux opérateurs.

## Visa général de l'agrégateur

- Une implémentation du runtime fica em `tools/soranet-relay/src/privacy.rs` comme `PrivacyAggregator`.
- Les buckets sont réglés par minute de montre (`bucket_secs`, 60 secondes par défaut) et armés dans un anneau limité (`max_completed_buckets`, 120 par défaut). Les actions de collectionneur ont leur propre carnet de commandes limité (`max_share_lag_buckets`, par défaut 12) pour que les ménages prio antigas soient utilisés comme des seaux supprimidos em vez de vaz memoria ou mascarar collectors presos.
- `RelayConfig::privacy` mapeia direct para `PrivacyConfig`, boutons de réglage exponents (`bucket_secs`, `min_handshakes`, `flush_delay_buckets`, `force_flush_buckets`, `max_completed_buckets`, `max_share_lag_buckets`, `expected_shares`). Le runtime de production maintient les valeurs par défaut tandis que SNNet-8a introduit des seuils d'agrégation sécurisés.
- Les modules d'enregistrement des événements d'exécution via les aides types : `record_circuit_accepted`, `record_circuit_rejected`, `record_throttle`, `record_throttle_cooldown`, `record_capacity_reject`, `record_active_sample`, `record_verified_bytes`, et `record_gar_category`.

## L'administrateur du point de terminaison effectue le relais

Les opérateurs peuvent consulter l'administrateur de l'auditeur pour relayer les observations brutes via `GET /privacy/events`. Le point de terminaison renvoie un JSON délimité par de nouvelles lignes (`application/x-ndjson`) avec des charges utiles `SoranetPrivacyEventV1` appliquées à l'intérieur `PrivacyEventBuffer`. Le tampon garde les événements les plus récents avec les entrées `privacy.event_buffer_capacity` (par défaut 4096) et le jeu en lecture, puis les grattoirs doivent sonder avec une fréquence suffisante pour éviter les lacunes. Les événements couvrent les paramètres de poignée de main, d'accélérateur, de bande passante vérifiée, de circuit actif et de GAR qui alimentent les contadores Prometheus, permettant aux collecteurs en aval d'archiver le fil d'Ariane en toute sécurité pour la confidentialité ou d'alimenter les flux de travail d'agrégation en toute sécurité.

## Configuration du relais

Les opérateurs ajustent la cadence de télémétrie de confidentialité dans l'archive de configuration du relais via un secao `privacy` :

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

Les valeurs par défaut des champs correspondent aux spécifications SNNet-8 et sont validées sans frais :| Champ | Description | Padrão |
|-------|-----------|--------|
| `bucket_secs` | Longueur de chaque rangée d'agrégats (secondes). | `60` |
| `min_handshakes` | Nombre minimum de contributeurs avant un seau pouvant émettre des contadores. | `12` |
| `flush_delay_buckets` | Les seaux complètent l'attente avant de tenter une chasse d'eau. | `1` |
| `force_flush_buckets` | Idée maximale avant d'émettre un seau supérieur. | `6` |
| `max_completed_buckets` | Backlog de buckets retidos (empêcher la mémoire sem limite). | `120` |
| `max_share_lag_buckets` | Janela de retencao para collector share avant de supprimir. | `12` |
| `expected_shares` | Prio collector partage les exigidos antes de combinar. | `2` |
| `event_buffer_capacity` | Backlog des événements NDJSON pour l'administrateur du flux. | `4096` |

Définissez `force_flush_buckets` plus petit que `flush_delay_buckets`, zéro des seuils, ou désactivez la garde de retenue avant de valider pour éviter les déploiements qui vont télémétrie par relais.

La limite `event_buffer_capacity` et la limite `/admin/privacy/events` garantissent que les grattoirs ne peuvent pas être ficar atrasados ​​indéfiniment.

## Actions de collection Prio

SNNet-8a implanta collectors duplos qui émettent des buckets Prio com partilhamento secreto. L'orchestrateur a maintenant analysé le flux NDJSON `/privacy/events` pour les entrées `SoranetPrivacyEventV1` et les partages `SoranetPrivacyPrioShareV1`, envoyés comme pour `SoranetSecureAggregator::ingest_prio_share`. Les seaux émettent quando chegam `PrivacyBucketConfig::expected_shares` contribue, en particulier le comportement du relais. Comme les actions sont validées pour l'alignement du seau et la forme de l'histogramme avant la combinaison des valeurs dans `SoranetPrivacyBucketMetricsV1`. En cas de contagem combiné de poignées de main ficar abaixo de `min_contributors`, le seau et exporté comme `suppressed`, en particulier le comportement de l'agrégateur sans relais. Les utilisateurs ont récemment émis une étiquette `suppression_reason` pour que les opérateurs puissent distinguer entre `insufficient_contributors`, `collector_suppressed`, `collector_window_elapsed` et `forced_flush_window_elapsed` pour diagnostiquer les lacunes de télémétrie. Le motif `collector_window_elapsed` disparaît également lorsque Prio partage la ficam alem de `max_share_lag_buckets`, les collectionneurs tornando presos visiveis sem deixar acumuladores antigos na memoria.

## Endpoints de l'ingestion du Torii

Torii expose maintenant deux points de terminaison HTTP avec contrôle de télémétrie pour que les relais et les collecteurs puissent encaminhar observer en embauchant un transport sur mesure :- `POST /v1/soranet/privacy/event` prend en charge la charge utile `RecordSoranetPrivacyEventDto`. Le corps implique un `SoranetPrivacyEventV1` mais un label `source` facultatif. Torii valide les exigences relatives au profil de télémétrie actif, enregistre l'événement et répond avec HTTP `202 Accepted` avec une enveloppe Norito JSON en attente d'une semaine calculée (`bucket_start_unix`, `bucket_duration_secs`) et comment faire un relais.
- `POST /v1/soranet/privacy/share` prend en charge la charge utile `RecordSoranetPrivacyShareDto`. Le corps transporte un `SoranetPrivacyPrioShareV1` et un `forwarded_by` facultatif pour que les opérateurs puissent auditer les flux de collecteurs. Les soumissions ont été renvoyées avec succès HTTP `202 Accepted` avec une enveloppe Norito JSON résumant le collecteur, la ligne de seau et la demande de suppression ; Falhas de validacao mapeiam pour une réponse de télémétrie `Conversion` pour préserver le traitement déterministe des erreurs entre les collectionneurs. La boucle des événements de l'orchestrateur émet maintenant ces partages pour interroger les relais, en gardant l'accumulateur avant Torii synchronisé avec les compartiments sans relais.

Les points de terminaison respectent le profil de télémétrie : ils émettent `503 Service Unavailable` lorsque les mesures sont désactivées. Les clients peuvent envoyer des corps Norito binaire (`application/x.norito`) ou Norito JSON (`application/x.norito+json`) ; Le serveur négocie automatiquement le format via les extracteurs padrao do Torii.

## Métriques Prometheus

Chaque seau exporte les étiquettes de chargement `mode` (`entry`, `middle`, `exit`) et `bucket_start`. Comme suit la famille des métriques émises par sa source :

| Métrique | Descriptif |
|--------|-------------|
| `soranet_privacy_circuit_events_total{kind}` | Taxonomie de la poignée de main avec `kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}`. |
| `soranet_privacy_throttles_total{scope}` | Contrôleurs d'accélérateur avec `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}`. |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` | Duracoes de cooldown agregadas por handshakes étranglés. |
| `soranet_privacy_verified_bytes_total` | Bande passante vérifiée par les preuves médicales. |
| `soranet_privacy_active_circuits_{avg,max}` | Médias et pico de circuits actifs par seau. |
| `soranet_privacy_rtt_millis{percentile}` | Estimations du pourcentage RTT (`p50`, `p90`, `p99`). |
| `soranet_privacy_gar_reports_total{category_hash}` | Contadores de Governance Action Report avec hachage par catégorie. |
| `soranet_privacy_bucket_suppressed` | Buckets retidos porque o limiar de contributeidores nao foi atingido. |
| `soranet_privacy_pending_collectors{mode}` | Acumuladores de parts de collection pendentes de combinacao, agrupados por modo de relay. |
| `soranet_privacy_suppression_total{reason}` | Contadores de buckets suprimidos com `reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}` para que les tableaux de bord attribuent des lacunes de confidentialité. |
| `soranet_privacy_snapshot_suppression_ratio` | Razao supprimida/drenada do ultimo drain (0-1), utilisé pour les budgets d'alerte. |
| `soranet_privacy_last_poll_unixtime` | Timestamp UNIX fait un dernier sondage réussi (alimentation ou alerte du collecteur inactif). |
| `soranet_privacy_collector_enabled` | Jauge qui vira `0` lorsque le collecteur de confidentialité est désactivé ou n'a pas été lancé (alimentation ou alerte collecteur désactivé). |
| `soranet_privacy_poll_errors_total{provider}` | Erreurs de sondage configurées par l'alias du relais (incrémentées dans les erreurs de décodage, erreurs HTTP ou codes d'état détectés). |Seaux sem observacoes permanecem silenciosos, mantendo tableaux de bord limpos sem fabricar janelas zeradas.

## Orientation opérationnelle

1. **Tableaux de bord** - tracez les mesures des groupes agrégés par `mode` et `window_start`. Destaquez janelas ausentes para révéler des problèmes de collecteur ou de relais. Utilisez `soranet_privacy_suppression_total{reason}` pour distinguer les contributeurs de suppression orientés vers les collectionneurs des trois lacunes. L'actif Grafana est maintenant envoyé via une peinture dédiée **"Suppression Reasons (5m)"** alimentée par les contadores plus une statistique **"Suppressed Bucket %"** qui calcule `sum(soranet_privacy_bucket_suppressed) / count(...)` en sélectionnant pour que les opérateurs violent rapidement le budget. Une série **"Collector Share Backlog"** (`soranet_privacy_pending_collectors`) et les statistiques **"Snapshot Suppression Ratio"** permettent aux collecteurs de caméras de gérer les coûts et le budget lors de l'exécution automatisée.
2. **Alertes** - déclenche les alarmes des contadores de sécurité de confidentialité : pics de rejet de PoW, fréquence de refroidissement, dérive de RTT et rejets de capacité. Comme les contadores sont monotones dans chaque seau, ils se basent simplement sur les taxons qui fonctionnent comme eux.
3. **Réponse aux incidents** - confie primeiro nos dados agregados. Lorsqu'il est nécessaire de déboguer plus profondément, demandez à ce que les relais reproduisent les instantanés des seaux ou à ce que les tests médicaux soient vérifiés lors de la collecte des journaux de trafic brut.
4. **Rétention** - grattez le visage avec une fréquence suffisante pour éviter de dépasser `max_completed_buckets`. Les exportateurs doivent traiter le dit Prometheus comme fonte canonica e descartar buckets locais depois de encaminhados.

## Analyse de suppression et exécution automatisée

L'essence de SNNet-8 dépend de la démonstration que les collecteurs sont automatisés en permanence et que la suppression est fixée dans les limites de la politique (<=10 % des seaux pour le relais pendant toute la durée de 30 minutes). L'outillage nécessaire pour acquérir cette porte maintenant avec le référentiel ; Les opérateurs doivent intégrer cela à leur rituel semanais. Les nouvelles douleurs de suppression du Grafana reflètent les trois fils PromQL abaixo, ainsi que les équipements de visibilité de la plante avant de les enregistrer précisément en consultant les manuels.

### Reçus PromQL pour la révision de la suppression

Les opérateurs développent les assistants suivants PromQL à mao ; mais nous ne référençons pas le tableau de bord Grafana partagé (`dashboards/grafana/soranet_privacy_metrics.json`) et les informations disponibles sur Alertmanager :

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

Utilisez un rapport dit pour confirmer que la statistique **"Suppressed Bucket %"** permanece abaixo do budget of politica ; Connectez le détecteur de pointes à Alertmanager pour un retour d'information rapide lorsqu'un contagem de contributeurs se produit désespérément.

### CLI du rapport du bucket hors ligne

L'espace de travail expose `cargo xtask soranet-privacy-report` pour capturer NDJSON pontuais. Aponte pour un ou plusieurs exports admin de relay :

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```L'assistant passe à la capture de `SoranetSecureAggregator`, imprime un résumé de suppression sur la sortie standard et, facultativement, grave sur un rapport JSON créé via `--json-out <path|->`. Il est important que les boutons du collecteur soient en direct (`--bucket-secs`, `--min-contributors`, `--expected-shares`, etc.), permettant aux opérateurs de reproduire des captures historiques de seuils différents pour déterminer un incident. Annexe ou JSON avec les captures de Grafana pour que la porte d'analyse de la suppression de SNNet-8 soit vérifiée en permanence.

### Checklist pour la première exécution automatisée

La gouvernance exige également de vérifier que la première exécution automatisée s'étende au budget de suppression. L'assistant vient d'utiliser `--max-suppression-ratio <0-1>` pour que les CI ou les opérateurs tombent rapidement lorsque les buckets sont supérieurs à la plage autorisée (10 % par défaut) ou lorsqu'ils n'ont plus de buckets. Flux recommandé :

1. Exportez NDJSON vers l'administrateur des points de terminaison vers le relais mais vers le flux `/v1/soranet/privacy/event|share` vers l'orchestrateur pour `artifacts/sorafs_privacy/<relay>.ndjson`.
2. Rode o help com o budget de politica:

   ```bash
   cargo xtask soranet-privacy-report \
     --input artifacts/sorafs_privacy/relay-a.ndjson \
     --input artifacts/sorafs_privacy/relay-b.ndjson \
     --json-out artifacts/sorafs_privacy/relay_summary.json \
     --max-suppression-ratio 0.10
   ```

   Le commandant imprime une décision observée et il a un code nul lorsque le budget est dépassé **ou** quand il n'y a plus de seaux immédiatement, signifiant que la télémétrie n'a jamais été produite pour l'exécution. Comme mesures ào vivo devem mostrar `soranet_privacy_pending_collectors` redenando para zero et `soranet_privacy_snapshot_suppression_ratio` ficando abaixo do my budget quanto a execucao ocorre.
3. Archivez ledit JSON et le journal de la CLI avec le paquet de preuves SNNet-8 avant de le transporter par défaut pour que les réviseurs puissent reproduire les artefatos exatos.

## Passons à proximité (SNNet-8a)

- Intégrer les collecteurs Dual Prio, connecter l'acquisition de partages au runtime pour que les relais et les collecteurs émettent des charges utiles `SoranetPrivacyBucketMetricsV1` cohérentes. *(Concluido - voir `ingest_privacy_payload` dans `crates/sorafs_orchestrator/src/lib.rs` et les testicules associés.)*
- Publier le tableau de bord Prometheus partagé et regras de alerta cobrindo lacunes de suppression, saude dos collectionneurs et quedas de anonimato. *(Concluido - voir `dashboards/grafana/soranet_privacy_metrics.json`, `dashboards/alerts/soranet_privacy_rules.yml`, `dashboards/alerts/soranet_policy_rules.yml` et luminaires de validation.)*
- Produisez les artefatos de calibracao de privacidade diferencial descritos em `privacy_metrics_dp.md`, y compris les cahiers de reproduction et les résumés de gouvernance. *(Concluido - notebook et artefatos gerados por `scripts/telemetry/run_privacy_dp.py`; wrapper CI `scripts/telemetry/run_privacy_dp_notebook.sh` exécuté le notebook via le workflow `.github/workflows/release-pipeline.yml`; résumé de gouvernance arquivado em `docs/source/status/soranet_privacy_dp_digest.md`.)*

Une version entre actuellement dans la base de SNNet-8 : télémétrie déterminée et sécurité de la confidentialité qui est directement enregistrée dans les scrapers et les tableaux de bord Prometheus existants. Les outils de calibrage de confidentialité différentielle ne sont pas situés, le workflow de libération du pipeline maintient les sorties du notebook actualisées, et le travail restant pour la surveillance de la première exécution automatisée et l'extension des analyses d'alerte de suppression.
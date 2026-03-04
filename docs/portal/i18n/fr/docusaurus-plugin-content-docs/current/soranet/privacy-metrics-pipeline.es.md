---
lang: fr
direction: ltr
source: docs/portal/docs/soranet/privacy-metrics-pipeline.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : pipeline de mesures de confidentialité
titre : Pipeline de mesures de confidentialité de SoraNet (SNNet-8)
sidebar_label : Pipeline de mesures de confidentialité
description : Télémétrie avec préservation de la confidentialité pour les relais et orchestrateurs de SoraNet.
---

:::note Fuente canonica
Refleja `docs/source/soranet/privacy_metrics_pipeline.md`. Mantengan ambas copias synchronisées hasta que los docs heredados se retraiten.
:::

# Pipeline de métriques de confidentialité de SoraNet

SNNet-8 introduit une surface de télémétrie consciente de la confidentialité pour
le temps d'exécution du relais. Maintenant le relais regroupe des événements de poignée de main et de circuit en
buckets d'une minute et exporta solo contadores Prometheus gruesos, manteniendo
les circuits individuels desvinculados mientras ofrece visibilité accionable
pour les opérateurs.

## Résumé de l'agrégateur

- La mise en œuvre du runtime vive en `tools/soranet-relay/src/privacy.rs` comme
  `PrivacyAggregator`.
- Les buckets sont indexés par minute de la montre (`bucket_secs`, par défaut 60 secondes) et
  se met en place sur un anneau acotado (`max_completed_buckets`, par défaut 120). Les actions
  les collecteurs conservent leur propre backlog acotado (`max_share_lag_buckets`, par défaut 12)
  pour que les ventanas Prio rassis se vacien comme des seaux suprimidos à la place de
  filtrer la mémoire ou masquer les collectionneurs atascados.
- `RelayConfig::privacy` mapea directement vers `PrivacyConfig`, boutons de réglage exponentiels
  (`bucket_secs`, `min_handshakes`, `flush_delay_buckets`, `force_flush_buckets`,
  `max_completed_buckets`, `max_share_lag_buckets`, `expected_shares`). Le runtime
  de production maintient les défauts pendant le SNNet-8a introduire des seuils de
  agrégation sûre.
- Les modules d'exécution enregistrent les événements avec des aides de type :
  `record_circuit_accepted`, `record_circuit_rejected`, `record_throttle`,
  `record_throttle_cooldown`, `record_capacity_reject`, `record_active_sample`,
  `record_verified_bytes`, et `record_gar_category`.

## Administrateur du point de terminaison du relais

Les opérateurs peuvent consulter l'auditeur, l'administrateur du relais pour les observations
crudas via `GET /privacy/events`. Le point de terminaison développe un JSON délimité par
Nouvelles lignes (`application/x-ndjson`) avec charges utiles `SoranetPrivacyEventV1`
reflejados desde el `PrivacyEventBuffer` interno. Le tampon conserve les événements
mais de nouvelles jusqu'aux entrées `privacy.event_buffer_capacity` (par défaut 4096) et se
vacía en la lectura, car les scrapers doivent sonder le suffisant pour
pas de dejar huecos. Los eventos cubren las mismas senales de poignée de main, accélérateur,
bande passante vérifiée, circuit actif et GAR qui alimente les contadores Prometheus,
permettre aux collecteurs en aval d'archiver le fil d'Ariane pour garantir la confidentialité
o Alimenter les flux de travail d'agrégation en toute sécurité.

## Configuration du relais

Les opérateurs ajustent la cadence de télémétrie de confidentialité dans le fichier
configuration du relais via la section `privacy` :

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

Les valeurs par défaut du champ coïncident avec la spécification SNNet-8 et sont validées
chargement :| Champ | Description | Par défaut |
|-------|-------------|---------|
| `bucket_secs` | Ancho de cada ventana de agregación (secondes). | `60` |
| `min_handshakes` | Minimum de contributions avant d'émettre un bucket. | `12` |
| `flush_delay_buckets` | Les seaux sont terminés à espérer avant la chasse d'eau. | `1` |
| `force_flush_buckets` | Il faut maximiser avant d'émettre un seau supérieur. | `6` |
| `max_completed_buckets` | Backlog de buckets retenus (evita memoria illimitada). | `120` |
| `max_share_lag_buckets` | Vente de rétention pour les actions des collectionneurs avant de supprimer. | `12` |
| `expected_shares` | Actions Prio requis avant de combiner. | `2` |
| `event_buffer_capacity` | Backlog NDJSON des événements pour le flux d'administration. | `4096` |

Configurer `force_flush_buckets` par le biais de `flush_delay_buckets`, veuillez
los seuils en zéro ou déshabilitar el guard de retención maintenant falla la
validation pour éviter des plis qui filtrent la télémétrie par relais.

El limite `event_buffer_capacity` tambien acota `/admin/privacy/events`,
assurez-vous que les grattoirs ne soient pas indéfiniment atras.

## Actions du collecteur Prio

SNNet-8a libère des collecteurs doubles qui émettent des seaux avant le partage de secrets.
Maintenant l'orchestrateur analyse le flux NDJSON `/privacy/events` tanto para
entrées `SoranetPrivacyEventV1` comme actions `SoranetPrivacyPrioShareV1`,
reenviandolos a `SoranetSecureAggregator::ingest_prio_share`. Les seaux se
émet lorsque je llegan `PrivacyBucketConfig::expected_shares` contributions,
réfléchir au comportement de l'agrégateur dans le relais. Les actions sont validées
par alinéation du seau et forme de l'histogramme avant de combiner en
`SoranetPrivacyBucketMetricsV1`. Si le conteo combiné de poignées de main peut être utilisé
debajo de `min_contributors`, le seau est exporté comme `suppressed`, réfléchi
le comportement de l'agrégateur dans le relais. Las ventanas suprimidas maintenant
émet une étiquette `suppression_reason` pour que les opérateurs distinguent entre
`insufficient_contributors`, `collector_suppressed`, `collector_window_elapsed`
et `forced_flush_window_elapsed` pour diagnostiquer les huecos de télémétrie. La raison
`collector_window_elapsed` également disparaitre lorsque les actions sont prioritaires pour plus
alla de `max_share_lag_buckets`, haciendo visibles los collectors atascados sin
maintenir les accumulateurs périmés en mémoire.

## Endpoints de l'ingestion Torii

Torii expose maintenant deux points de terminaison HTTP avec télémétrie sécurisée pour les relais et
les collectionneurs reenvien observaciones sin incrustar un transporte sur mesure :- `POST /v1/soranet/privacy/event` accepte une charge utile
  `RecordSoranetPrivacyEventDto`. Le corps enveloppe un `SoranetPrivacyEventV1`
  mais une étiquette facultative `source`. Torii valide la sollicitude contre le
  profil de télémétrie actif, enregistre l'événement et répond avec HTTP
  `202 Accepted` avec une enveloppe Norito JSON qui contient la fenêtre
  calculé pour le seau (`bucket_start_unix`, `bucket_duration_secs`) et le
  mode du relais.
- `POST /v1/soranet/privacy/share` accepte une charge utile `RecordSoranetPrivacyShareDto`.
  Le corps porte un `SoranetPrivacyPrioShareV1` et un indice optionnel `forwarded_by`
  pour que les opérateurs auditent les flux de collectionneurs. Las entregas exitosas
  devuelven HTTP `202 Accepted` avec une enveloppe Norito JSON qui reprend le
  collecteur, la fenêtre du seau et l'indice de suppression ; les chutes de
  validation mapean à une réponse de télémétrie `Conversion` pour préserver
  le manejo de errores determinista en collectionneurs. La boucle d'événement de l'orchestrateur
  maintenant, émets estas partage les relais de la sondea, manteniendo el acumulador
  Prio de Torii synchronisé avec les seaux sur relais.

Les points de terminaison d'Abos respectent le profil de télémétrie : émettent le service `503
Indisponible lorsque les mesures sont déshabilitées. Les clients peuvent envoyer des messages
corps Norito binaire (`application/x.norito`) ou Norito JSON (`application/x.norito+json`) ;
le serveur négocie automatiquement le format via les extracteurs standard de
Torii.

## Métriques Prometheus

Chaque seau exporté porte les étiquettes `mode` (`entry`, `middle`, `exit`) et
`bucket_start`. Vous émettrez les suivants familias de metricas :

| Métrique | Description |
|--------|-------------|
| `soranet_privacy_circuit_events_total{kind}` | Taxonomie de poignée de main avec `kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}`. |
| `soranet_privacy_throttles_total{scope}` | Contrôleurs d'accélérateur avec `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}`. |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` | Durées de recharge ajoutées pour les poignées de main limitées. |
| `soranet_privacy_verified_bytes_total` | Ancho de banda verificado de proofs de medicion cegada. |
| `soranet_privacy_active_circuits_{avg,max}` | Médias et pico de circuits activés par seau. |
| `soranet_privacy_rtt_millis{percentile}` | Estimations du pourcentage de RTT (`p50`, `p90`, `p99`). |
| `soranet_privacy_gar_reports_total{category_hash}` | Contadores GAR hasheados por digest de categoria. |
| `soranet_privacy_bucket_suppressed` | Les seaux sont retenus car ils ne complètent pas l'ombre des contributions. |
| `soranet_privacy_pending_collectors{mode}` | Acumuladores de parts pendientes de combinar, agrupados por modo de relay. |
| `soranet_privacy_suppression_total{reason}` | Contadores de buckets suprimidos con `reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}` para atribuir gaps de privacidad. |
| `soranet_privacy_snapshot_suppression_ratio` | Rapport supérieur/déchargé du dernier drain (0-1), utilisé pour présupposer l'alerte. |
| `soranet_privacy_last_poll_unixtime` | Timestamp UNIX du dernier sondage terminé (alimentant l'alerte collector-idle). |
| `soranet_privacy_collector_enabled` | Jauge que cae a `0` lorsque le collecteur de confidentialité est désactivé ou tombe au démarrage (alimente l'alerte collecteur-désactivé). |
| `soranet_privacy_poll_errors_total{provider}` | Erreurs de sondage associées à l'alias du relais (incrémentées pour les erreurs de décodage, échecs HTTP ou codes indésirables). |Los buckets sin observaciones permanecen silenciosos, manteniendo tableaux de bord
limpios sin fabricar ventanas con ceros.

## Guide opérationnel

1. **Tableaux de bord** - graphique des mesures antérieures agrégées par `mode` et
   `window_start`. Des ventanas faltantes pour révéler les problèmes des collectionneurs
   o relais. Usa `soranet_privacy_suppression_total{reason}` pour distinguer
   carencias de contributeyentes de suppression por collector al triage. L'actif de
   Grafana inclut désormais un panneau dédié **"Suppression Reasons (5m)"**
   alimenté par ces contadores mais une statistique **"Suppressed Bucket %"** que
   calcul `sum(soranet_privacy_bucket_suppressed) / count(...)` par sélection
   pour que les opérateurs aient des brèches de présupposé d'une vue. La série
   ** Collector Share Backlog ** (`soranet_privacy_pending_collectors`) et les statistiques
   **Taux de suppression d'instantanés** Les collecteurs de resaltan atascados et dérive du budget
   lors des exécutions automatisées.
2. **Alertes** - déclenche les alarmes des contadores en toute sécurité : picos de rechazo
   PoW, fréquence de refroidissement, dérive du RTT et rejets de capacité. Côme les
   contadores son monotone dentro de cada bucket, reglas basadas en tasa
   fonctionne bien.
3. **Réponse aux incidents** - confia primero en datos agregados. Lorsque vous en avez besoin
   dépuration profonde, solliciter les relais reproduisant des instantanés de seaux o
   inspeccionar proofs de medicion cegada en lieu et place des journaux de trafic
   crudos.
4. **Rétention** - grattage avec une fréquence suffisante pour ne pas dépasser
   `max_completed_buckets`. Les exportateurs doivent tratar la salida Prometheus como
   la source canonique et descartar buckets locales una vez reenviados.

## Analyse de suppression et d'exécutions automatisées

L'acceptation de SNNet-8 dépend de la démonstration des collecteurs automatisés.
mantienen sanos et que la suppression permanente dans les limites de la politique
(<=10% de buckets por relay en cualquier ventana de 30 minutos). El outillage
il est nécessaire que vous l'incluiez dans le repo ; les opérateurs doivent les intégrer à leur
rituels sémanales. Les nouveaux panneaux de suppression en Grafana reflètent les
snippets PromQL abajo, ando visibilité en vivo a los equipos on-call avant de
récurrir a consultas manuales.

### Récupérez PromQL pour réviser la suppression

Les opérateurs doivent avoir à leur disposition les assistants suivants PromQL ; ambos se
référence dans le compartiment du tableau de bord (`dashboards/grafana/soranet_privacy_metrics.json`)
et les règles d'Alertmanager :

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

Utilisez le ratio pour confirmer que la statistique **"Suppressed Bucket %"** est maintenue
por debajo del presupuesto de politique; connecter le détecteur de pointes à
Alertmanager pour un feedback rapide lorsque le contenu des contributeurs baje de
forme inespérée.

### CLI de rapport hors ligne des buckets

L'espace de travail expose `cargo xtask soranet-privacy-report` pour capturer NDJSON
ponctuels. Apuntalo à un ou plus exporte l'administrateur du relais :

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```L'assistant procède à la capture avec `SoranetSecureAggregator`, imprime un résumé de
suppression sur la sortie standard et éventuellement écrivez un rapport JSON structuré via
`--json-out <path|->`. Respectez les boutons mismos que le collectionneur en vivo
(`--bucket-secs`, `--min-contributors`, `--expected-shares`, etc.), permis
reproduire des captures historiques avec des seuils distincts pour étudier un problème.
Ajouter le JSON avec les captures d'écran de Grafana pour la porte d'analyse
SNNet-8 siga siendo auditable.

### Checklist pour la première exécution automatisée

La gouvernance exige également que la première exécution soit automatisée
présupposé de suppression. L'assistant accepte désormais `--max-suppression-ratio <0-1>`
pour que les opérateurs CI tombent rapidement lorsque les seaux dépassent la
ventana permitida (par défaut 10%) ou lorsqu'aucun seau à foin n'est présenté. Flujo
recommandé:

1. Exporter NDJSON depuis l'administrateur du point de terminaison du relais et le flux
   `/v1/soranet/privacy/event|share` de l'orchestrateur maintenant
   `artifacts/sorafs_privacy/<relay>.ndjson`.
2. Exécutez l'assistant avec le présupposé politique :

   ```bash
   cargo xtask soranet-privacy-report \
     --input artifacts/sorafs_privacy/relay-a.ndjson \
     --input artifacts/sorafs_privacy/relay-b.ndjson \
     --json-out artifacts/sorafs_privacy/relay_summary.json \
     --max-suppression-ratio 0.10
   ```

   Le commandant imprime le ratio observé et vendu avec un code distinct de zéro
   quand il dépasse le présupposé **o** quand il n'y a pas de listes de seaux à foin, indique
   que la télémétrie n'a pas été produite pour l'éjection. Les métriques fr
   vivo deben mostrar `soranet_privacy_pending_collectors` drenando hacia cero y
   `soranet_privacy_snapshot_suppression_ratio` mettre sous le mismo
   presupuesto mientras se ejecuta la corrida.
3. Archivage du JSON de sortie et du journal CLI avec le bundle de preuves SNNet-8
   avant de changer le transport par défaut pour que les réviseurs puissent le faire
   reproduire exactement les artefacts.

## Proximos pasos (SNNet-8a)

- Intégrer les collecteurs en priorité, connecter l'acquisition de partages au runtime
  pour que les relais et collecteurs émettent des charges utiles `SoranetPrivacyBucketMetricsV1`
  cohérents. *(Terminé - ver `ingest_privacy_payload` fr
  `crates/sorafs_orchestrator/src/lib.rs` et tests associés.)*
- Publier le tableau de bord Prometheus compartimenté et le verre d'alerte que cubran
  lacunes de suppression, santé du collectionneur et baisses de tension de l'anonymat. *(Terminé - ver
  `dashboards/grafana/soranet_privacy_metrics.json`,
  `dashboards/alerts/soranet_privacy_rules.yml`,
  `dashboards/alerts/soranet_policy_rules.yml`, et luminaires de validation.)*
- Produire les artefacts de calibrage des descriptions de confidentialité différentielle en
  `privacy_metrics_dp.md`, comprend des cahiers reproductibles et des résumés de
  gouvernance. *(Terminé - carnet + artefacts générés par
  `scripts/telemetry/run_privacy_dp.py` ; Encapsuleur CI
  `scripts/telemetry/run_privacy_dp_notebook.sh` exécute le notebook via le
  flux de travail `.github/workflows/release-pipeline.yml` ; résumé de la gouvernance archivé fr
  `docs/source/status/soranet_privacy_dp_digest.md`.)*

La version actuelle entre dans la base SNNet-8 : télémétrie déterministe et sécurisée
pour la confidentialité qui est intégrée directement aux grattoirs et aux tableaux de bord Prometheus
existant. Les artefacts de calibrage de confidentialité différentielle sont dans la liste,
le flux de travail de libération conserve les fresques des sorties du cahier et le travail
restante se enfoca en monitorear la première émission automatisée et extender
l'analyse des alertes de suppression.
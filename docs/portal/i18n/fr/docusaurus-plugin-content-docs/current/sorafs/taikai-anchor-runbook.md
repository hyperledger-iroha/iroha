---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/taikai-anchor-runbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Runbook d’observabilité de l’ancre Taikai

Cette copie du portail reflète le runbook canonique dans
[`docs/source/taikai_anchor_monitoring.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/taikai_anchor_monitoring.md).
Utilisez-le lors des répétitions d’ancrage des routing-manifests (TRM) SN13-C afin
que les opérateurs SoraFS/SoraNet puissent corréler les artefacts de spool, la
telemetry Prometheus et les preuves de gouvernance sans quitter l’aperçu du portail.

## Portée et owners

- **Programme :** SN13-C — manifests Taikai & ancres SoraNS.
- **Owners :** Media Platform WG, DA Program, Networking TL, Docs/DevRel.
- **Objectif :** Fournir un playbook déterministe pour les alertes Sev 1/Sev 2,
  la validation de la télémétrie et la capture de preuves pendant que les routing
  manifests Taikai avancent à travers les alias.

## Quickstart (Sev 1/Sev 2)

1. **Capturer les artefacts de spool** — copiez les derniers fichiers
   `taikai-anchor-request-*.json`, `taikai-trm-state-*.json` et
   `taikai-lineage-*.json` depuis
   `config.da_ingest.manifest_store_dir/taikai/` avant de redémarrer les workers.
2. **Dumper la télémétrie `/status`** — enregistrez le tableau
   `telemetry.taikai_alias_rotations` pour prouver quelle fenêtre de manifest est active :
   ```bash
   curl -sSf "$TORII/status" | jq '.telemetry.taikai_alias_rotations'
   ```
3. **Vérifier les dashboards et alertes** — chargez
   `dashboards/grafana/taikai_viewer.json` (filtres cluster + stream) et notez si
   des règles dans
   `dashboards/alerts/taikai_viewer_rules.yml` ont déclenché (`TaikaiLiveEdgeDrift`,
   `TaikaiIngestFailure`, `TaikaiCekRotationLag`, événements de santé PoR SoraFS).
4. **Inspecter Prometheus** — exécutez les requêtes de la section « Référence des
   métriques » pour confirmer que la latence/drift d’ingest et les compteurs de
   rotation d’alias se comportent comme prévu. Escalader si
   `taikai_trm_alias_rotations_total` stagne sur plusieurs fenêtres ou si les
   compteurs d’erreurs augmentent.

## Référence des métriques

| Métrique | Objectif |
| --- | --- |
| `taikai_ingest_segment_latency_ms` | Histogramme de latence d’ingest CMAF par cluster/stream (cible : p95 < 750 ms, p99 < 900 ms). |
| `taikai_ingest_live_edge_drift_ms` | Drift live-edge entre l’encodeur et les workers d’ancrage (page si p99 > 1,5 s pendant 10 min). |
| `taikai_ingest_segment_errors_total{reason}` | Compteurs d’erreurs par raison (`decode`, `manifest_mismatch`, `lineage_replay`, …). Toute hausse déclenche `TaikaiIngestFailure`. |
| `taikai_trm_alias_rotations_total{alias_namespace,alias_name}` | Incrémente à chaque acceptation d’un nouveau TRM par `/v2/da/ingest`; utilisez `rate()` pour valider la cadence de rotation. |
| `/status → telemetry.taikai_alias_rotations[]` | Snapshot JSON avec `window_start_sequence`, `window_end_sequence`, `manifest_digest_hex`, `rotations_total` et timestamps pour les bundles de preuve. |
| `taikai_viewer_*` (rebuffer, âge rotation CEK, santé PQ, alertes) | KPIs viewer pour garantir que la rotation CEK + les circuits PQ restent sains pendant les anchors. |

### Snippets PromQL

```promql
histogram_quantile(
  0.99,
  sum by (le) (
    rate(taikai_ingest_segment_latency_ms_bucket{cluster=~"$cluster",stream=~"$stream"}[5m])
  )
)
```

```promql
sum by (reason) (
  rate(taikai_ingest_segment_errors_total{cluster=~"$cluster",stream=~"$stream"}[5m])
)
```

```promql
rate(
  taikai_trm_alias_rotations_total{alias_namespace="sora",alias_name="docs"}[15m]
)
```

## Dashboards et alertes

- **Grafana viewer board :** `dashboards/grafana/taikai_viewer.json` — latence p95/p99,
  drift live-edge, erreurs de segments, âge de rotation CEK, alertes viewer.
- **Grafana cache board :** `dashboards/grafana/taikai_cache.json` — promotions hot/warm/cold
  et refus QoS quand les fenêtres d’alias tournent.
- **Alertmanager rules :** `dashboards/alerts/taikai_viewer_rules.yml` — paging drift,
  avertissements d’échec ingest, lag de rotation CEK et pénalités/cooldowns de santé
  PoR SoraFS. Assurez des receivers pour chaque cluster de production.

## Checklist du bundle de preuves

- Artefacts de spool (`taikai-anchor-request-*`, `taikai-trm-state-*`,
  `taikai-lineage-*`).
- Exécutez `cargo xtask taikai-anchor-bundle --spool <manifest_dir>/taikai --copy-dir <bundle_dir> --signing-key <ed25519_hex>`
  pour émettre un inventaire JSON signé des enveloppes en attente/livrées et copier
  les fichiers request/SSM/TRM/lineage dans le bundle du drill. Le chemin spool par
  défaut est `storage/da_manifests/taikai` depuis `torii.toml`.
- Snapshot `/status` couvrant `telemetry.taikai_alias_rotations`.
- Exports Prometheus (JSON/CSV) pour les métriques ci-dessus sur la fenêtre d’incident.
- Captures Grafana avec filtres visibles.
- IDs Alertmanager référant les déclenchements pertinents.
- Lien vers `docs/examples/taikai_anchor_lineage_packet.md` décrivant le paquet de
  preuve canonique.

## Miroir des dashboards et cadence des drills

Satisfaire l’exigence SN13-C signifie prouver que les dashboards Taikai
viewer/cache sont reflétés dans le portail **et** que le drill d’évidence d’ancrage
fonctionne sur une cadence prévisible.

1. **Mirroring portal.** Lorsque `dashboards/grafana/taikai_viewer.json` ou
   `dashboards/grafana/taikai_cache.json` change, résumez les deltas dans
   `sorafs/taikai-monitoring-dashboards` (ce portail) et notez les checksums JSON
   dans la description du PR portail. Mettez en avant les nouveaux panels/seuils
   pour que les reviewers puissent corréler avec le dossier Grafana géré.
2. **Drill mensuel.**
   - Exécutez le drill le premier mardi de chaque mois à 15:00 UTC afin que la
     preuve soit disponible avant le sync de gouvernance SN13.
   - Capturez les artefacts de spool, la télémétrie `/status` et des captures Grafana
     dans `artifacts/sorafs_taikai/drills/<YYYYMMDD>/`.
   - Journalisez l’exécution avec
     `scripts/telemetry/log_sorafs_drill.sh --scenario taikai-anchor`.
3. **Revue & publication.** Sous 48 heures, examinez alertes/faux positifs avec
   DA Program + NetOps, consignez les follow-ups dans le drill log et liez l’upload
   du bucket gouvernance depuis `docs/source/sorafs/runbooks-index.md`.

Si dashboards ou drills prennent du retard, SN13-C ne peut pas sortir de 🈺 ;
maintenez cette section à jour dès que la cadence ou les attentes d’évidence changent.

## Commandes utiles

```bash
# Snapshot de la télémétrie de rotation d’alias vers un répertoire d’artefacts
curl -sSf "$TORII/status" \
  | jq '{timestamp: now | todate, aliases: .telemetry.taikai_alias_rotations}' \
  > artifacts/taikai/status_snapshots/$(date -u +%Y%m%dT%H%M%SZ).json

# Lister les entrées de spool pour un alias/événement spécifique
find "$MANIFEST_DIR/taikai" -maxdepth 1 -type f -name 'taikai-*.json' | sort

# Inspecter les raisons de mismatch TRM dans le spool log
jq '.error_context | select(.reason == "lineage_replay")' \
  "$MANIFEST_DIR/taikai/taikai-ssm-20260405T153000Z.norito"
```

Maintenez cette copie du portail synchronisée avec le runbook canonique lorsque
la télémétrie d’ancrage Taikai, les dashboards ou les exigences de preuve de
la gouvernance changent.

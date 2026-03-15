---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/taikai-anchor-runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 50261b1f3173cd3916b29c81e85cc92ed8c14c38a0e0296be38397fe9b5c0596
source_last_modified: "2025-11-21T18:08:23.480735+00:00"
translation_last_reviewed: 2026-01-30
---

# ראנבוק תצפית לאנקור Taikai

עותק הפורטל הזה משקף את הראנבוק הקנוני שב-
[`docs/source/taikai_anchor_monitoring.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/taikai_anchor_monitoring.md).
השתמשו בו בעת חזרות אנקור routing-manifest (TRM) של SN13-C, כך שמפעילי SoraFS/SoraNet
יוכלו לקשר בין ארטיפקטי spool, טלמטריית Prometheus וראיות ממשל ללא יציאה מתצוגת הפורטל.

## היקף ובעלים

- **תוכנית:** SN13-C — manifests של Taikai ו-anchors של SoraNS.
- **בעלים:** Media Platform WG, DA Program, Networking TL, Docs/DevRel.
- **מטרה:** לספק playbook דטרמיניסטי להתראות Sev 1/Sev 2, אימות טלמטריה ולכידת ראיות
  בזמן ש-routing manifests של Taikai מתקדמים בין aliases.

## Quickstart (Sev 1/Sev 2)

1. **לכוד ארטיפקטי spool** — העתיקו את הקבצים העדכניים
   `taikai-anchor-request-*.json`, `taikai-trm-state-*.json` ו-
   `taikai-lineage-*.json` מתוך
   `config.da_ingest.manifest_store_dir/taikai/` לפני הפעלה מחדש של workers.
2. **Dump של טלמטריית `/status`** — רשמו את המערך
   `telemetry.taikai_alias_rotations` כדי להוכיח איזו חלון manifest פעיל:
   ```bash
   curl -sSf "$TORII/status" | jq '.telemetry.taikai_alias_rotations'
   ```
3. **בדקו dashboards והתראות** — טענו
   `dashboards/grafana/taikai_viewer.json` (מסנני cluster + stream) ושימו לב אם
   כללים ב-
   `dashboards/alerts/taikai_viewer_rules.yml` הופעלו (`TaikaiLiveEdgeDrift`,
   `TaikaiIngestFailure`, `TaikaiCekRotationLag`, אירועי בריאות PoR של SoraFS).
4. **בדקו Prometheus** — הריצו את השאילתות בסעיף "Metric reference" כדי לוודא
   שה-latency/drift של ingest ומוני rotation של alias מתנהגים כמצופה. הסלימו
   אם `taikai_trm_alias_rotations_total` נתקע במספר חלונות או אם מוני השגיאות עולים.

## Metric reference

| מטריקה | מטרה |
| --- | --- |
| `taikai_ingest_segment_latency_ms` | היסטוגרמת latency של ingest CMAF לפי cluster/stream (יעד: p95 < 750 ms, p99 < 900 ms). |
| `taikai_ingest_live_edge_drift_ms` | drift של live-edge בין encoder ל-anchor workers (page כאשר p99 > 1.5 s ל-10 דקות). |
| `taikai_ingest_segment_errors_total{reason}` | מוני שגיאות לפי סיבה (`decode`, `manifest_mismatch`, `lineage_replay`, ...). כל עליה מפעילה `TaikaiIngestFailure`. |
| `taikai_trm_alias_rotations_total{alias_namespace,alias_name}` | גדל בכל פעם ש-`/v1/da/ingest` מקבל TRM חדש לאליאס; השתמשו ב-`rate()` כדי לאמת קצב רוטציה. |
| `/status → telemetry.taikai_alias_rotations[]` | snapshot JSON עם `window_start_sequence`, `window_end_sequence`, `manifest_digest_hex`, `rotations_total` ו-timestamps עבור evidence bundles. |
| `taikai_viewer_*` (rebuffer, גיל סיבוב CEK, בריאות PQ, התראות) | KPIs של viewer כדי לוודא שסיבוב CEK + מעגלי PQ נשארים בריאים במהלך anchors. |

### PromQL snippets

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

## Dashboards והתראות

- **Grafana viewer board:** `dashboards/grafana/taikai_viewer.json` — latency p95/p99,
  drift של live-edge, שגיאות סגמנטים, גיל סיבוב CEK, alertים של viewer.
- **Grafana cache board:** `dashboards/grafana/taikai_cache.json` — קידומי hot/warm/cold
  וסירובי QoS כאשר חלונות alias מסתובבים.
- **Alertmanager rules:** `dashboards/alerts/taikai_viewer_rules.yml` — paging על drift,
  אזהרות ingest failure, lag בסיבוב CEK ו-penalties/cooldowns של בריאות PoR ב-SoraFS.
  ודאו receivers לכל cluster פרודקשן.

## Evidence bundle checklist

- ארטיפקטי spool (`taikai-anchor-request-*`, `taikai-trm-state-*`,
  `taikai-lineage-*`).
- הריצו `cargo xtask taikai-anchor-bundle --spool <manifest_dir>/taikai --copy-dir <bundle_dir> --signing-key <ed25519_hex>`
  כדי להפיק JSON inventory חתום של envelopes pending/delivered ולהעתיק קבצי
  request/SSM/TRM/lineage אל bundle drill. נתיב ה-spool ברירת מחדל הוא
  `storage/da_manifests/taikai` מתוך `torii.toml`.
- Snapshot `/status` שמכסה `telemetry.taikai_alias_rotations`.
- ייצוא Prometheus (JSON/CSV) עבור המטריקות לעיל בזמן חלון האירוע.
- צילומי Grafana עם פילטרים גלויים.
- IDs של Alertmanager המצביעים על ההתרעות הרלוונטיות.
- קישור ל-`docs/examples/taikai_anchor_lineage_packet.md` שמתאר את evidence packet הקנוני.

## שיקוף dashboards וקצב drills

עמידה בדרישת SN13-C פירושה להוכיח שה-dashboards של Taikai viewer/cache משוקפים
בפורטל **וגם** ש-drill הראיות לאנקור רץ בקצב צפוי.

1. **שיקוף פורטל.** כאשר `dashboards/grafana/taikai_viewer.json` או
   `dashboards/grafana/taikai_cache.json` משתנים, סכמו את הדלתאות ב-
   `sorafs/taikai-monitoring-dashboards` (פורטל זה) וציינו JSON checksums בתיאור
   ה-PR של הפורטל. הדגישו פאנלים/ספים חדשים כדי שקובעי בדיקה יוכלו לקשר לתיקיית
   Grafana המנוהלת.
2. **Drill חודשי.**
   - בצעו את ה-drill ביום שלישי הראשון בכל חודש בשעה 15:00 UTC כדי שהראיות יגיעו
     לפני סנכרון הממשל של SN13.
   - לכדו ארטיפקטי spool, טלמטריית `/status` וצילומי Grafana בתוך
     `artifacts/sorafs_taikai/drills/<YYYYMMDD>/`.
   - תעדו את ההרצה באמצעות
     `scripts/telemetry/log_sorafs_drill.sh --scenario taikai-anchor`.
3. **Review & publish.** בתוך 48 שעות, עברו על alerts/false positives עם
   DA Program + NetOps, רשמו follow-ups ב-drill log וקשרו את העלאת governance bucket
   מתוך `docs/source/sorafs/runbooks-index.md`.

אם dashboards או drills מתעכבים, SN13-C לא יכולה לצאת מ-🈺; שמרו על סעיף זה
מעודכן כאשר הקצב או ציפיות הראיות משתנים.

## פקודות שימושיות

```bash
# Snapshot טלמטריה של סיבוב alias לתיקיית ארטיפקטים
curl -sSf "$TORII/status" \
  | jq '{timestamp: now | todate, aliases: .telemetry.taikai_alias_rotations}' \
  > artifacts/taikai/status_snapshots/$(date -u +%Y%m%dT%H%M%SZ).json

# רשימת ערכי spool ל-alias/event ספציפי
find "$MANIFEST_DIR/taikai" -maxdepth 1 -type f -name 'taikai-*.json' | sort

# בדיקת סיבות mismatch של TRM מתוך spool log
jq '.error_context | select(.reason == "lineage_replay")' \
  "$MANIFEST_DIR/taikai/taikai-ssm-20260405T153000Z.norito"
```

שמרו על סנכרון עותק הפורטל הזה עם הראנבוק הקנוני כאשר טלמטריית anchoring של Taikai,
הדשבורדים או דרישות evidence של הממשל משתנים.

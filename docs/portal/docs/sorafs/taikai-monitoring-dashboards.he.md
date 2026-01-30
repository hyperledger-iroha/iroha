---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/taikai-monitoring-dashboards.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6049b1c4fb42bbfbeaa7fa8f3549c5b7beac1a3e8baec45c0c0ce52f0c3baa2e
source_last_modified: "2025-11-14T09:52:13.533271+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: לוחות ניטור Taikai
description: סיכום פורטל ללוחות Grafana של viewer/cache שמגבים ראיות SN13-C.
---

מוכנות routing-manifest של Taikai (TRM) נשענת על שני לוחות Grafana וההתראות
הנלוות שלהם. עמוד זה משקף את ההיילייטים מתוך
`dashboards/grafana/taikai_viewer.json`, `dashboards/grafana/taikai_cache.json`
ו-`dashboards/alerts/taikai_viewer_rules.yml` כדי ש-reviewers יוכלו לעקוב בלי
לשכפל את הריפו.

## Dashboard viewer (`taikai_viewer.json`)

- **Live edge ו-latency:** הפאנלים מציגים היסטוגרמות latency p95/p99
  (`taikai_ingest_segment_latency_ms`, `taikai_ingest_live_edge_drift_ms`) לפי
  cluster/stream. עקבו אחרי p99 > 900 ms או drift > 1.5 s (מפעיל את
  `TaikaiLiveEdgeDrift`).
- **שגיאות מקטעים:** מפרק `taikai_ingest_segment_errors_total{reason}` כדי לחשוף
  כשלים ב-decode, ניסיונות lineage replay או אי-התאמות manifest. צרפו צילומי מסך
  לאירועי SN13-C כשפאנל זה עולה מעל תחום “warning”.
- **בריאות viewer ו-CEK:** פאנלים שמבוססים על `taikai_viewer_*` עוקבים אחרי גיל
  סבב CEK, תמהיל PQ guard, rebuffer counts ו-roll-ups של התראות. פאנל ה-CEK אוכף
  SLA סבב שהממשל בודק לפני אישור aliases חדשים.
- **Snapshot טלמטריה של aliases:** טבלת `/status → telemetry.taikai_alias_rotations`
  יושבת ישירות על הלוח כדי שהמפעילים יאשרו digests של manifest לפני צירוף ראיות
  ממשל.

## Dashboard cache (`taikai_cache.json`)

- **לחץ לפי tier:** פאנלים מתארים `sorafs_taikai_cache_{hot,warm,cold}_occupancy`
  ו-`sorafs_taikai_cache_promotions_total`. השתמשו בהם כדי לראות אם סבב TRM
  מעמיס tiers ספציפיים.
- **דחיות QoS:** `sorafs_taikai_qos_denied_total` מציף כאשר לחץ cache גורם
  throttling; סמנו ב-drill log בכל פעם שהקצב חורג מאפס.
- **שימוש egress:** מסייע לאשר שיציאות SoraFS עומדות בקצב של Taikai viewers
  כאשר חלונות CMAF מתחלפים.

## התראות ולכידת ראיות

- כללי paging נמצאים ב-`dashboards/alerts/taikai_viewer_rules.yml` וממופים אחד-לאחד
  לפאנלים לעיל (`TaikaiLiveEdgeDrift`, `TaikaiIngestFailure`, `TaikaiCekRotationLag`,
  proof-health warnings). ודאו שכל cluster ב-production מחובר ל-Alertmanager.
- snapshots/צילומי מסך שנלכדו במהלך drills חייבים להישמר תחת
  `artifacts/sorafs_taikai/drills/<YYYYMMDD>/` יחד עם spool files ו-JSON של `/status`.
  השתמשו ב-`scripts/telemetry/log_sorafs_drill.sh --scenario taikai-anchor` כדי
  להוסיף את הביצוע ל-drill log המשותף.
- כאשר dashboards משתנים, כללו את digest SHA-256 של קובץ ה-JSON בתיאור ה-PR של
  הפורטל כדי שמבקרים יוכלו להתאים בין תיקיית Grafana המנוהלת לגרסת הריפו.

## Checklist של חבילת ראיות

סקירות SN13-C מצפות שכל drill או אירוע יספקו את אותם artefacts שמופיעים ב-runbook
של Taikai anchor. לכדו אותם לפי הסדר להלן כדי שהחבילה תהיה מוכנה לביקורת ממשל:

1. העתיקו את הקבצים העדכניים ביותר `taikai-anchor-request-*.json`,
   `taikai-trm-state-*.json`, ו-`taikai-lineage-*.json` מתוך
   `config.da_ingest.manifest_store_dir/taikai/`. artefacts של spool אלו מוכיחים
   איזה routing manifest (TRM) ואיזה חלון lineage היו פעילים. ה-helper
   `cargo xtask taikai-anchor-bundle --spool <dir> --copy-dir <out> --out <out>/anchor_bundle.json [--signing-key <ed25519>]`
   יעתיק את spool files, יפיק hashes ויחתום את הסיכום לפי הצורך.
2. תעדו את פלט `/v1/status` המסונן ל-
   `.telemetry.taikai_alias_rotations[]` ושמרו אותו ליד spool files. ה-reviewers
   משווים את `manifest_digest_hex` והגבולות לחלון עם מצב ה-spool שהועתק.
3. יצאו Prometheus snapshots עבור המטריקות לעיל וצילמו את לוחות viewer/cache עם
   פילטרי cluster/stream הרלוונטיים בתצוגה. שימו את JSON/CSV הגולמי והצילומים
   בתיקיית ה-artefacts.
4. כללו IDs של תקריות Alertmanager (אם קיימים) שמצביעות לכללים ב-
   `dashboards/alerts/taikai_viewer_rules.yml` וציינו האם נסגרו אוטומטית לאחר
   שהמצב נפתר.

אחסנו הכל תחת `artifacts/sorafs_taikai/drills/<YYYYMMDD>/` כדי שביקורות drills
וביקורות ממשל SN13-C יוכלו למשוך ארכיון יחיד.

## Cadence של drills ולוגים

- הריצו Taikai anchor drill ביום שלישי הראשון של כל חודש בשעה 15:00 UTC.
  הלו"ז שומר על ראיות מעודכנות לקראת סנכרון הממשל של SN13.
- לאחר לכידת ה-artefacts לעיל, הוסיפו את הביצוע ל-ledger המשותף עם
  `scripts/telemetry/log_sorafs_drill.sh --scenario taikai-anchor`. ה-helper
  מפיק את רשומת ה-JSON הנדרשת על ידי `docs/source/sorafs/runbooks-index.md`.
- קשרו את ה-artefacts המאוחסנים בכניסה של runbook index והסלימו alerts כושלים או
  רגרסיות dashboard תוך 48 שעות בערוץ Media Platform WG/SRE.
- שמרו את סט צילומי הסיכום של drill (latency, drift, errors, סבב CEK, לחץ cache)
  ליד חבילת ה-spool כדי שמפעילים יוכלו להראות בדיוק כיצד dashboards התנהגו במהלך החזרה.

חזרו ל-[Taikai Anchor Runbook](./taikai-anchor-runbook.md) לפרוצדורת Sev 1 המלאה
ול-checklist של הראיות. עמוד זה מסכם רק את ההנחיות הספציפיות ל-dashboards ש-SN13-C
דורשת לפני היציאה מ-🈺.

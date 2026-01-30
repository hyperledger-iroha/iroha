---
lang: he
direction: rtl
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/sorafs/operations-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 768e9d36586b339f540e7127ba26e1649f4c7669d8e8474448b5702600e5aa68
source_last_modified: "2026-01-03T18:07:59+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Hebrew (he) translation. Replace this content with the full translation. -->

---
id: operations-playbook
lang: he
direction: rtl
source: docs/portal/docs/sorafs/operations-playbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note מקור קנוני
עמוד זה משקף את הראנבוק המתוחזק תחת `docs/source/sorafs_ops_playbook.md`. שמרו על סנכרון שתי הגרסאות עד שמערך תיעוד Sphinx יושלם במלואו.
:::

## הפניות מרכזיות

- נכסי Observability: עיינו בדשבורדים של Grafana תחת `dashboards/grafana/` ובכללי התראות Prometheus ב-`dashboards/alerts/`.
- קטלוג מדדים: `docs/source/sorafs_observability_plan.md`.
- משטחי טלמטריה של האורקסטרטור: `docs/source/sorafs_orchestrator_plan.md`.

## מטריצת הסלמה

| עדיפות | דוגמאות טריגר | On-call ראשי | גיבוי | הערות |
|--------|---------------|--------------|-------|-------|
| P1 | השבתה גלובלית של gateway, שיעור כשל PoR > 5% (15 דקות), backlog של רפליקציה מוכפל כל 10 דקות | Storage SRE | Observability TL | יש לערב את מועצת הממשל אם ההשפעה חוצה 30 דקות. |
| P2 | הפרת SLO של latencies אזוריות ב-gateway, קפיצת retries באורקסטרטור ללא השפעת SLA | Observability TL | Storage SRE | להמשיך rollout אך לחסום manifests חדשים. |
| P3 | התראות לא קריטיות (staleness של manifests, קיבולת 80–90%) | Intake triage | Ops guild | לטפל ביום העסקים הבא. |

## השבתת gateway / זמינות ירודה

**זיהוי**

- התראות: `SoraFSGatewayAvailabilityDrop`, `SoraFSGatewayLatencySlo`.
- Dashboard: `dashboards/grafana/sorafs_gateway_overview.json`.

**פעולות מיידיות**

1. אשרו את ההיקף (ספק יחיד לעומת צי) דרך פאנל קצב הבקשות.
2. העבירו את ניתוב Torii לספקים בריאים (אם מדובר בריבוי ספקים) באמצעות טוגול `sorafs_gateway_route_weights` בקונפיג ops (`docs/source/sorafs_gateway_self_cert.md`).
3. אם כל הספקים מושפעים, הפעילו fallback של “direct fetch” עבור לקוחות CLI/SDK (`docs/source/sorafs_node_client_protocol.md`).

**Triage**

- בדקו שימוש ב-stream tokens מול `sorafs_gateway_stream_token_limit`.
- בדקו לוגים של gateway עבור שגיאות TLS או admission.
- הריצו `scripts/telemetry/run_schema_diff.sh` כדי לוודא שהסכימה שה-gateway מייצא תואמת לגרסה המצופה.

**אפשרויות תיקון**

- אתחלו רק את תהליך ה-gateway המושפע; הימנעו ממחזור כל הקלאסטר אלא אם כמה ספקים נופלים.
- הגדילו זמנית את מגבלת stream tokens ב-10–15% אם מאומתת רוויה.
- הריצו self-cert מחדש (`scripts/sorafs_gateway_self_cert.sh`) לאחר ההתייצבות.

**Post-incident**

- כתבו postmortem P1 באמצעות `docs/source/sorafs/postmortem_template.md`.
- תכננו drill כאוס המשך אם התיקון נשען על התערבויות ידניות.

## קפיצה בכשלי הוכחה (PoR / PoTR)

**זיהוי**

- התראות: `SoraFSProofFailureSpike`, `SoraFSPoTRDeadlineMiss`.
- Dashboard: `dashboards/grafana/sorafs_proof_integrity.json`.
- טלמטריה: `torii_sorafs_proof_stream_events_total` ואירועי `sorafs.fetch.error` עם `provider_reason=corrupt_proof`.

**פעולות מיידיות**

1. הקפיאו אישורי manifests חדשים באמצעות סימון מרשם ה-manifest (`docs/source/sorafs/manifest_pipeline.md`).
2. הודיעו ל-Governance להשהות תמריצים לספקים המושפעים.

**Triage**

- בדקו עומק תור אתגרי PoR מול `sorafs_node_replication_backlog_total`.
- אמתו את pipeline אימות ההוכחות (`crates/sorafs_node/src/potr.rs`) עבור פריסות אחרונות.
- השוו גרסאות firmware של ספקים מול מרשם המפעילים.

**אפשרויות תיקון**

- הפעילו replays של PoR באמצעות `sorafs_cli proof stream` עם ה-manifest האחרון.
- אם ההוכחות נכשלות בעקביות, הסירו את הספק מהסט הפעיל על ידי עדכון מרשם הממשל והכרחת רענון scoreboards של האורקסטרטור.

**Post-incident**

- הריצו את תרחיש drill הכאוס של PoR לפני הפריסה הבאה לפרודקשן.
- תעדו לקחים בתבנית postmortem ועדכנו את checklist הסמכת הספקים.

## עיכוב רפליקציה / גדילת backlog

**זיהוי**

- התראות: `SoraFSReplicationBacklogGrowing`, `SoraFSCapacityPressure`. ייבאו
  `dashboards/alerts/sorafs_capacity_rules.yml` והריצו
  `promtool test rules dashboards/alerts/tests/sorafs_capacity_rules.test.yml`
  לפני promotion כדי ש-Alertmanager ישקף את הספים המתועדים.
- Dashboard: `dashboards/grafana/sorafs_capacity_health.json`.
- Metrics: `sorafs_node_replication_backlog_total`, `sorafs_node_manifest_refresh_age_seconds`.

**פעולות מיידיות**

1. אמתו את היקף ה-backlog (ספק יחיד או צי) ועצרו משימות רפליקציה לא חיוניות.
2. אם ה-backlog ממוקד, הקצו זמנית הזמנות חדשות לספקים חלופיים דרך scheduler הרפליקציה.

**Triage**

- בדקו טלמטריה של האורקסטרטור לזינוקים של retries שעלולים לנפח את ה-backlog.
- ודאו שליעדי האחסון יש headroom מספק (`sorafs_node_capacity_utilisation_percent`).
- עברו על שינויים אחרונים בקונפיגורציה (עדכוני chunk profile, cadence של proofs).

**אפשרויות תיקון**

- הריצו `sorafs_cli` עם `--rebalance` כדי לחלק מחדש את התוכן.
- בצעו scaling אופקי של workers לרפליקציה עבור הספק המושפע.
- הפעילו manifest refresh כדי ליישר חלונות TTL.

**Post-incident**

- תכננו drill קיבולת שממוקד בכשלי רוויה של ספקים.
- עדכנו את תיעוד SLA הרפליקציה ב-`docs/source/sorafs_node_client_protocol.md`.

## קצב תרגולי כאוס

- **רבעוני**: סימולציה משולבת של השבתת gateway + סערת retries של האורקסטרטור.
- **חצי-שנתי**: הזרקת כשלי PoR/PoTR בשני ספקים עם התאוששות.
- **בדיקת spot חודשית**: תרחיש עיכוב רפליקציה עם manifests של staging.
- תעדו את התרגילים ב-log המשותף (`ops/drill-log.md`) באמצעות:

  ```bash
  scripts/telemetry/log_sorafs_drill.sh \
    --scenario "Gateway outage chaos drill" \
    --status pass \
    --ic "Alex Morgan" \
    --scribe "Priya Patel" \
    --notes "Failover to west cluster succeeded" \
    --log ops/drill-log.md \
    --link "docs/source/sorafs/postmortem_template.md"
  ```

- אמתו את ה-log לפני commits עם:

  ```bash
  scripts/telemetry/validate_drill_log.sh
  ```

- השתמשו ב-`--status scheduled` לתרגילים עתידיים, `pass`/`fail` להרצות שהושלמו, ו-`follow-up` כשיש פעולות פתוחות.
- דרסו את היעד באמצעות `--log` עבור dry-runs או אימות אוטומטי; בלעדיו הסקריפט ימשיך לעדכן את `ops/drill-log.md`.

## תבנית postmortem

השתמשו ב-`docs/source/sorafs/postmortem_template.md` עבור כל אירוע P1/P2 ועבור רטרוספקטיבות של תרגולי כאוס. התבנית מכסה ציר זמן, כימות השפעה, גורמים תורמים, פעולות מתקנות ומשימות אימות המשך.

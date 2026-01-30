---
lang: he
direction: rtl
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/nexus/transition-notes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1329d1f7908f59e0b4fb198f11b99094f3ab4ad6a2105987615f7c0d139b85e1
source_last_modified: "2025-11-14T04:43:20.610842+00:00"
translation_last_reviewed: 2026-01-30
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# הערות מעבר של Nexus

יומן זה עוקב אחר העבודה שנותרה של **Phase B - Nexus Transition Foundations** עד לסיום רשימת הבדיקות להשקה מרובת lanes. הוא משלים את רשומות אבני הדרך ב-`roadmap.md` ושומר את הראיות שמוזכרות ב-B1-B4 במקום אחד כדי שהממשל, SRE ומובילי SDK ישתפו את אותו מקור אמת.

## היקף וקצב

- מכסה את ביקורות routed-trace ואת guardrails של טלמטריה (B1/B2), את סט הדלתאות של הקונפיגורציה שאושר על ידי הממשל (B3), ואת המשכי חזרת ההשקה מרובת lanes (B4).
- מחליף את הערת הקצב הזמנית שהיתה כאן קודם; מאז ביקורת Q1 2026 הדוח המפורט נמצא ב-`docs/source/nexus_routed_trace_audit_report_2026q1.md`, בעוד דף זה מחזיק את לוח הזמנים השוטף ואת מרשם המיתונים.
- עדכנו את הטבלאות אחרי כל חלון routed-trace, הצבעת ממשל או חזרת השקה. כאשר ארטיפקטים זזים, שיקפו את המיקום החדש בתוך דף זה כדי שמסמכי המשך (status, dashboards, פורטלי SDK) יוכלו לקשר לעוגן יציב.

## צילום ראיות (2026 Q1-Q2)

| זרם עבודה | ראיות | בעלים | סטטוס | הערות |
|------------|----------|----------|--------|-------|
| **B1 - Routed-trace audits** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`, `docs/examples/nexus_audit_outcomes/` | @telemetry-ops, @governance | הושלם (Q1 2026) | שלושה חלונות ביקורת תועדו; פער ה-TLS של `TRACE-CONFIG-DELTA` נסגר במהלך rerun של Q2. |
| **B2 - Remediation של טלמטריה ו-guardrails** | `docs/source/nexus_telemetry_remediation_plan.md`, `docs/source/telemetry.md`, `dashboards/alerts/nexus_audit_rules.yml` | @sre-core, @telemetry-ops | הושלם | Alert pack, מדיניות diff bot, וגודל אצווה OTLP (`nexus.scheduler.headroom` log + פאנל headroom ב-Grafana) נמסרו; אין waivers פתוחים. |
| **B3 - אישורי דלתאות קונפיגורציה** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`, `defaults/nexus/config.toml`, `defaults/nexus/genesis.json` | @release-eng, @governance | הושלם | ההצבעה GOV-2026-03-19 תועדה; ה-bundle החתום מזין את חבילת הטלמטריה שמוזכרת למטה. |
| **B4 - חזרת השקה multi-lane** | `docs/source/runbooks/nexus_multilane_rehearsal.md`, `docs/source/project_tracker/nexus_rehearsal_2026q1.md`, `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`, `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`, `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-core, @sre-core | הושלם (Q2 2026) | ה-rerun הקנרי של Q2 סגר את מיתון פער ה-TLS; ה-validator manifest + `.sha256` תופסים טווח slots 912-936, workload seed `NEXUS-REH-2026Q2`, ואת hash פרופיל ה-TLS שנרשם ב-rerun. |

## לוח ביקורות routed-trace רבעוני

| Trace ID | חלון (UTC) | תוצאה | הערות |
|----------|--------------|---------|-------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | עבר | Queue-admission P95 נשאר הרבה מתחת ליעד <=750 ms. אין צורך בפעולה. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | עבר | OTLP replay hashes צורפו ל-`status.md`; ה-parity של SDK diff bot אישרה אפס drift. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | נפתר | פער ה-TLS נסגר במהלך rerun של Q2; חבילת הטלמטריה עבור `NEXUS-REH-2026Q2` מתעדת את hash פרופיל ה-TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` (ראו `artifacts/nexus/tls_profile_rollout_2026q2/`) ואפס מפגרים. |
| `TRACE-MULTILANE-CANARY` | 2026-05-05 09:12-10:14 | עבר | Workload seed `NEXUS-REH-2026Q2`; pack טלמטריה + manifest/digest ב-`artifacts/nexus/rehearsals/2026q1/` (slot range 912-936) עם agenda ב-`artifacts/nexus/rehearsals/2026q2/`. |

רבעונים עתידיים צריכים להוסיף שורות חדשות ולהעביר את הרשומות שהושלמו לנספח כאשר הטבלה גדלה מעבר לרבעון הנוכחי. הפנו לסעיף הזה מדוחות routed-trace או מפרוטוקולי ממשל באמצעות העוגן `#quarterly-routed-trace-audit-schedule`.

## מיתונים ופריטי backlog

| פריט | תיאור | בעלים | יעד | סטטוס / הערות |
|------|-------------|-------|--------|----------------|
| `NEXUS-421` | להשלים את הפצת פרופיל ה-TLS שהתעכב במהלך `TRACE-CONFIG-DELTA`, ללכוד ראיות rerun ולסגור את יומן המיתון. | @release-eng, @sre-core | חלון routed-trace של Q2 2026 | נסגר - hash פרופיל ה-TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` נלכד ב-`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256`; ה-rerun אישר שאין מפגרים. |
| `TRACE-MULTILANE-CANARY` prep | לתזמן את חזרת Q2, לצרף fixtures לחבילת הטלמטריה ולהבטיח ש-SDK harnesses עושים reuse ל-helper המאומת. | @telemetry-ops, SDK Program | שיחת תכנון 2026-04-30 | הושלם - agenda מאוחסנת ב-`artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` עם מטא-דטה של slot/workload; reuse של ה-harness תועד ב-tracker. |
| Telemetry pack digest rotation | להריץ `scripts/telemetry/validate_nexus_telemetry_pack.py` לפני כל חזרה/Release ולרשום digests ליד tracker של config delta. | @telemetry-ops | לכל release candidate | הושלם - `telemetry_manifest.json` + `.sha256` הופקו ב-`artifacts/nexus/rehearsals/2026q1/` (slot range `912-936`, seed `NEXUS-REH-2026Q2`); digests הועתקו ל-tracker ולמפתח הראיות. |

## אינטגרציית bundle של config delta

- `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` נשאר תקציר ה-diff הקנוני. כאשר מגיעים `defaults/nexus/*.toml` חדשים או שינויי genesis, עדכנו את ה-tracker קודם ואז שיקפו כאן את העיקר.
- Signed config bundles מזינים את חבילת הטלמטריה של החזרה. החבילה, שמאומתת על ידי `scripts/telemetry/validate_nexus_telemetry_pack.py`, חייבת להתפרסם לצד ראיות config delta כדי שמפעילים יוכלו לשחזר את הארטיפקטים המדויקים ששימשו ב-B4.
- חבילות Iroha 2 נשארות ללא lanes: configs עם `nexus.enabled = false` דוחות כעת overrides של lane/dataspace/routing אלא אם פרופיל Nexus מופעל (`--sora`), לכן הסירו סעיפים `nexus.*` מתבניות single-lane.
- שמרו את יומן ההצבעה הממשלית (GOV-2026-03-19) מקושר גם מה-tracker וגם מהערה זו כדי שהצבעות עתידיות יוכלו להעתיק את הפורמט בלי לגלות מחדש את טקס האישור.

## המשכי חזרת השקה

- `docs/source/runbooks/nexus_multilane_rehearsal.md` מתעד את תוכנית הקנרי, רשימת המשתתפים ושלבי ה-rollback; עדכנו את ה-runbook כאשר טופולוגיית ה-lanes או יצואני הטלמטריה משתנים.
- `docs/source/project_tracker/nexus_rehearsal_2026q1.md` מפרט כל ארטיפקט שנבדק בחזרה של 9 באפריל וכעת כולל הערות/agenda להכנה של Q2. הוסיפו חזרות עתידיות לאותו tracker במקום לפתוח trackers חד-פעמיים כדי לשמור ראיות רציפות.
- פרסמו snippets של OTLP collector ו-exports מ-Grafana (ראו `docs/source/telemetry.md`) בכל פעם שהנחיות batching של ה-exporter משתנות; עדכון Q1 העלה את batch size ל-256 דגימות כדי למנוע התראות headroom.
- ראיות CI/tests של multi-lane נמצאות כעת ב-`integration_tests/tests/nexus/multilane_pipeline.rs` ורצות תחת ה-workflow `Nexus Multilane Pipeline` (`.github/workflows/integration_tests_multilane.yml`), במקום ההפניה שפרשה `pytests/nexus/test_multilane_pipeline.py`; שמרו את ה-hash של `defaults/nexus/config.toml` (`nexus.enabled = true`, blake2b `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) מסונכרן עם ה-tracker בעת רענון bundles של החזרה.

## מחזור חיי lanes בזמן ריצה

- תכניות מחזור חיי lanes בזמן ריצה מאמתות כעת bindings של dataspace ומבטלות כאשר reconciliation של Kura/אחסון מדורג נכשל, תוך השארת הקטלוג ללא שינוי. ה-helpers מנקים relays שמורים עבור lanes שפרשו כדי שסינתזת merge-ledger לא תמחזר proofs מיושנים.
- החילו תכניות דרך helpers של config/lifecycle של Nexus (`State::apply_lane_lifecycle`, `Queue::apply_lane_lifecycle`) כדי להוסיף/לפרוש lanes ללא restart; routing, TEU snapshots ו-registries של manifests נטענים מחדש אוטומטית אחרי תכנית מוצלחת.
- הנחיה למפעילים: כאשר תכנית נכשלת, בדקו dataspaces חסרים או storage roots שלא ניתן ליצור (tiered cold root/ספריות Kura לפי lane). תקנו את נתיבי הבסיס ונסו שוב; תכניות מוצלחות מפיקות מחדש את diff הטלמטריה של lane/dataspace כדי שה-dashboards ישקפו את הטופולוגיה החדשה.

## טלמטריית NPoS וראיות backpressure

הרטרו של חזרת ההשקה של Phase B ביקש לכידות טלמטריה דטרמיניסטיות שמוכיחות שה-pacemaker של NPoS ושכבות ה-gossip נשארות בתוך גבולות ה-backpressure שלהן. ה-harness האינטגרטיבי ב-`integration_tests/tests/sumeragi_npos_performance.rs` מפעיל את התרחישים האלה ומוציא סיכומי JSON (`sumeragi_baseline_summary::<scenario>::...`) כאשר מדדים חדשים נוחתים. הריצו מקומית עם:

```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
```

הגדירו `SUMERAGI_NPOS_STRESS_PEERS`, `SUMERAGI_NPOS_STRESS_COLLECTORS_K` או `SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R` כדי לבחון טופולוגיות לחוצות יותר; ערכי ברירת המחדל משקפים את פרופיל האוספים 1 s/`k=3` ששימש ב-B4.

| תרחיש / test | כיסוי | טלמטריה מרכזית |
| --- | --- | --- |
| `npos_baseline_1s_k3_captures_metrics` | חוסם 12 סבבים עם block time של החזרה כדי לרשום מעטפות latency של EMA, עומקי תורים ו-gauges של redundant-send לפני סריאליזציה של bundle ראיות. | `sumeragi_phase_latency_ema_ms`, `sumeragi_collectors_k`, `sumeragi_redundant_send_r`, `sumeragi_bg_post_queue_depth*`. |
| `npos_queue_backpressure_triggers_metrics` | מציף את תור הטרנזקציות כדי לוודא שה-deferrals של admission מופעלים דטרמיניסטית ושהתור מייצא מוני capacity/saturation. | `sumeragi_tx_queue_depth`, `sumeragi_tx_queue_capacity`, `sumeragi_tx_queue_saturated`, `sumeragi_pacemaker_backpressure_deferrals_total`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_pacemaker_jitter_within_band` | דוגם jitter של ה-pacemaker ו-timeouts של view עד להוכחה שהטווח +/-125 permille נאכף. | `sumeragi_pacemaker_jitter_ms`, `sumeragi_pacemaker_view_timeout_target_ms`, `sumeragi_pacemaker_jitter_frac_permille`. |
| `npos_rbc_store_backpressure_records_metrics` | דוחף payloads גדולים של RBC עד למגבלות soft/hard של ה-store כדי להראות שסשנים ומוני bytes עולים, נסוגים ומתייצבים בלי לעבור את ה-store. | `sumeragi_rbc_store_pressure`, `sumeragi_rbc_store_sessions`, `sumeragi_rbc_store_bytes`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_redundant_send_retries_update_metrics` | מכריח retransmits כדי שמדדי יחס redundant-send ומוני collectors-on-target יתקדמו, וכך מוכיח שהטלמטריה שהרטרו ביקש מחוברת end-to-end. | `sumeragi_collectors_targeted_current`, `sumeragi_redundant_sends_total`. |
| `npos_rbc_chunk_loss_fault_reports_backlog` | מפיל chunks במרווחים דטרמיניסטיים כדי לוודא שמוניטורי backlog מרימים faults במקום לנקז בשקט payloads. | `sumeragi_rbc_backlog_sessions_pending`, `sumeragi_rbc_backlog_chunks_total`, `sumeragi_rbc_backlog_chunks_max`. |

צרפו את שורות ה-JSON שה-harness מדפיס יחד עם scrape של Prometheus שנלכד במהלך הריצה בכל פעם שממשל מבקש ראיות שהתרעות backpressure תואמות לטופולוגיית החזרה.

## רשימת עדכון

1. הוסיפו חלונות routed-trace חדשים והוציאו את הישנים כאשר הרבעונים מתחלפים.
2. עדכנו את טבלת המיתונים אחרי כל מעקב Alertmanager, גם אם הפעולה היא לסגור את הכרטיס.
3. כאשר config deltas משתנים, עדכנו את ה-tracker, את ההערה הזו ואת רשימת ה-digests של telemetry pack באותו pull request.
4. קשרו כאן כל ארטיפקט חדש של חזרה/טלמטריה כדי שעדכוני roadmap עתידיים יוכלו להפנות למסמך יחיד במקום להערות ad-hoc מפוזרות.

## אינדקס ראיות

| נכס | מיקום | הערות |
|-------|----------|-------|
| דוח ביקורת routed-trace (Q1 2026) | `docs/source/nexus_routed_trace_audit_report_2026q1.md` | מקור קנוני לראיות Phase B1; משוכפל לפורטל תחת `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md`. |
| Tracker של config delta | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` | מכיל את סיכומי diff של TRACE-CONFIG-DELTA, ראשי תיבות של reviewers ויומן ההצבעה GOV-2026-03-19. |
| תוכנית remediation של טלמטריה | `docs/source/nexus_telemetry_remediation_plan.md` | מתעד את alert pack, גודל אצוות OTLP ו-guardrails של תקציב ה-export הקשורים ל-B2. |
| Tracker של חזרה multi-lane | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` | מפרט את ארטיפקטי החזרה מ-9 באפריל, manifest/digest של validator, הערות/agenda של Q2 וראיות rollback. |
| Telemetry pack manifest/digest (latest) | `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` (+ `.sha256`) | מתעד slot range 912-936, seed `NEXUS-REH-2026Q2` ו-hashes של ארטיפקטים עבור bundles של ממשל. |
| TLS profile manifest | `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` (+ `.sha256`) | Hash של פרופיל TLS מאושר שנתפס במהלך rerun של Q2; ציינו בנספחי routed-trace. |
| Agenda TRACE-MULTILANE-CANARY | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | הערות תכנון לחזרה של Q2 (חלון, slot range, workload seed, בעלי פעולה). |
| Runbook חזרת השקה | `docs/source/runbooks/nexus_multilane_rehearsal.md` | Checklist תפעולי ל-staging -> execution -> rollback; לעדכן כאשר טופולוגיית lanes או הנחיות exporters משתנות. |
| Telemetry pack validator | `scripts/telemetry/validate_nexus_telemetry_pack.py` | CLI שמוזכר ברטרו B4; לארכב digests לצד ה-tracker בכל שינוי pack. |
| Multilane regression | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` | מאמת `nexus.enabled = true` עבור configs multi-lane, משמר hashes של קטלוג Sora ומקצה נתיבי Kura/merge-log לכל lane (`blocks/lane_{id:03}_{slug}`) דרך `ConfigLaneRouter` לפני פרסום digests של ארטיפקטים. |

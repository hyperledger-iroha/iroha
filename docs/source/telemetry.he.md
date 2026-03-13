<!-- Hebrew translation of docs/source/telemetry.md -->

---
lang: he
direction: rtl
source: docs/source/telemetry.md
status: needs-update
translator: manual
---

<div dir="rtl">

# סקירת טלמטריה ומדדים

סקירה ביפנית זמינה ב-[`telemetry.ja.md`](./telemetry.ja.md).

Iroha חושפת מדדים תואמי Prometheus ותמצית סטטוס בפורמט JSON. עמוד זה מרכז את נקודות הקצה, המדדים החשובים ודוגמאות לשאילתות PromQL ליצירת דשבורדים.

## נקודות קצה

- `/metrics`: טקסט חשיפה של Prometheus. מוסתר כאשר הטלמטריה מושבתת או כאשר הפרופיל אינו מאפשר מדדים יקרים.
- `/status`: סטטוס JSON (מוסתר כאשר הטלמטריה מושבתת). כולל מדדי על (Peers, בלוקים, תורים) וכן מבנה `sumeragi { leader_index, highest_qc_height, locked_qc_height, locked_qc_view, gossip_fallback_total, view_change_proof_{accepted,stale,rejected}_total, block_created_dropped_by_lock_total, block_created_hint_mismatch_total, block_created_proposal_mismatch_total, pacemaker_backpressure_deferrals_total, tx_queue_depth, tx_queue_capacity, tx_queue_saturated, epoch_length_blocks, epoch_commit_deadline_offset, epoch_reveal_deadline_offset, prf_epoch_seed (hex), prf_height, prf_view }` ותצלום `governance`.
- `/v2/sumeragi/new_view` (JSON): מספרי קבלות NEW_VIEW אחרונות לכל `(height, view)` (חלון זיכרון מוגבל; רשומות ישנות נזרקות).
- `/v2/sumeragi/new_view/sse` (SSE): זרם תקופתי של אותו מטען JSON לדשבורדים חיים.
- `/v2/sumeragi/status` (ברירת מחדל Norito): צילום מצב של הקונסנזוס. בקשת `Accept: application/json` מחזירה `{ leader_index, view_change_index, highest_qc { height, view, subject_block_hash }, locked_qc { height, view, subject_block_hash }, tx_queue { depth, capacity, saturated }, epoch { length_blocks, commit_deadline_offset, reveal_deadline_offset }, gossip_fallback_total, block_created_dropped_by_lock_total, block_created_hint_mismatch_total, block_created_proposal_mismatch_total, pacemaker_backpressure_deferrals_total, da_reschedule_total, rbc_store { sessions, bytes, pressure_level, backpressure_deferrals_total, persist_drops_total, evictions_total, recent_evictions[...] }, prf { height, view, epoch_seed }, vrf_penalty_epoch, vrf_committed_no_reveal_total, vrf_no_participation_total, vrf_late_reveals_total, collectors_targeted_{current,last_per_block}, redundant_sends_total, lane_governance { ... }, lane_governance_sealed_total, lane_governance_sealed_aliases, worker_loop { stage, stage_started_ms, last_iteration_ms, queue_depths { vote_rx, block_payload_rx, rbc_chunk_rx, block_rx, consensus_rx, lane_relay_rx, background_rx } } }`.
- `/v2/sumeragi/status/sse` (SSE): זרם ≈1 שנייה עם אותו JSON כמו `/v2/sumeragi/status`.
- `/v2/sumeragi/rbc` (JSON): מדדי RBC: `{ sessions_active, sessions_pruned_total, ready_broadcasts_total, ready_rebroadcasts_skipped_total, deliver_broadcasts_total, payload_bytes_delivered_total, payload_rebroadcasts_skipped_total }`.
- `/v2/sumeragi/rbc/sessions` (JSON): מצב סשני RBC: `{ sessions_active, items: [{ block_hash, height, view, total_chunks, received_chunks, ready_count, delivered, invalid, payload_hash, recovered, lane_backlog: [{ lane_id, tx_count, total_chunks, pending_chunks, rbc_bytes_total }], dataspace_backlog: [{ lane_id, dataspace_id, tx_count, total_chunks, pending_chunks, rbc_bytes_total }] }] }`.
- `/v2/sumeragi/pacemaker` (JSON): טיימרים ותצורת pacemaker: `{ backoff_ms, rtt_floor_ms, jitter_ms, backoff_multiplier, rtt_floor_multiplier, max_backoff_ms, jitter_frac_permille }`.
- `/v2/sumeragi/qc` (ברירת מחדל Norito): צילום Highest/Locked QC (`highest_qc`/`locked_qc`); כולל `subject_block_hash` עבור Highest QC אם ידוע. ניתן לבקש JSON דרך הכותרת `Accept`.
- `/v2/sumeragi/leader` (JSON): צילום מדד מנהיג; במצב NPoS כולל הקשר PRF `{ height, view, epoch_seed }`.
- `/v2/sumeragi/phases` (JSON): זמני שלבים קומפקטיים (ms) ללוחות מחוונים של מפעילים; מחזיר את משכי השלבים האחרונים.
- `/v2/sumeragi/collectors` (JSON): צילום של תוכנית ה-Collectors הדטרמיניסטית שמופקת מהטופולוגיה הקומיטד ומהפרמטרים on-chain; חושף `mode`, ‎`plan (height, view)`‎, ‎`collectors_k`, ‎`redundant_send_r`, ‎`proxy_tail_index`, ‎`min_votes_for_commit`, רשימת האוספים המסודרת ו-`epoch_seed` (ב-הקס) במצב NPoS.
- `/v2/sumeragi/params` (JSON): צילום פרמטרי Sumeragi on-chain `{ block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, redundant_send_r, da_enabled, next_mode, mode_activation_height, chain_height }`.
- `/v2/sumeragi/new_view/json` (JSON): צילום NEW_VIEW `{ ts_ms, items: [{ height, view, count }] }` עם `locked_qc { height, view }` (חלון זיכרון מוגבל; רשומות ישנות נזרקות).

השדות המצטברים `lane_governance_sealed_total` ו-`lane_governance_sealed_aliases` מאפשרים לבדוק מיידית אם קיימות מסילות שעדיין חסומות בגלל חוסר במניפסט ממשל. הן זמינות גם ב-`/v2/sumeragi/status` וגם ב-`iroha_cli --output-format text ops sumeragi status`, וה-CLI מציג את רשימת הכינויים כפי שהם. לצורך בדיקות והרצות CI מומלץ לשלב את `iroha_cli app nexus lane-report --only-missing --fail-on-sealed` כך שתהליכים ייעצרו אוטומטית כשעדיין קיימות מסילות חסומות.

## תצורה

- `telemetry_enabled` (ברירת מחדל true): מתג אב לשליטה כוללת. כאשר false, הדמון אינו מפעיל את משימת הטלמטריה, Torii מסתיר `/metrics` ו-`/status`, וכל אינסטרומנטציה מתבצעת רק אם הפרופיל מאפשר זאת.
- `telemetry_profile` (ברירת מחדל `operator`): אוסף יכולות המגדיר ניתוב Torii ו-Sinks בזמן ריצה. פרופילים שולטים בשלושה דגלים: `metrics`, ‏`expensive_metrics`, ‏`developer_outputs`. כאשר `telemetry_enabled = false`, הפרופיל האפקטיבי הוא `disabled`.

### מטריצת יכולות פרופיל

| פרופיל     | `/status` | `/metrics` | מסלולי מפתחים (`/v2/sumeragi/*`, ‏SSE) | שימוש מיועד |
|------------|-----------|------------|----------------------------------------|-------------|
| `disabled` | לא        | לא         | לא                                     | טלמטריה כבויה לחלוטין |
| `operator` | כן        | לא         | לא                                     | צמתי פרודקשן הזקוקים רק לסטטוס JSON |
| `extended` | כן        | כן         | לא                                     | צמתי הפעלה / QA הזקוקים גם למדדים |
| `developer`| כן        | כן         | כן                                     | סביבת פיתוח או בדיקות |

## אנונימיזציה ושלמות טלמטריה

- `telemetry_redaction.mode` הוא `strict|allowlist|disabled`; בפרופילים `operator`/`extended`/`full` חייבים `strict`.
- סיווג שדות רגישים נקבע לפי רשימות Prefix ו-Keyword; הרשימות המלאות נמצאות ב-`docs/source/telemetry.md`.
- `telemetry_integrity.enabled` מוסיף `chain { seq, prev_hash, hash, signature?, key_id? }`. כאשר `telemetry_integrity.signing_key_hex` מוגדר, נוסף keyed Blake3.
- כדי לשמור על רציפות בין אתחולים הגדירו `telemetry_integrity.state_dir`.

## פרופילי טלמטריה ייעודיים

### Android SDK (AND7)

סעיף **NRPC/AND7** קובע כי ה-SDK לאנדרואיד חייב לשמור על זהות פונקציונלית עם צמתי Rust בכל הקשור לטלמטריה ולשמירת פרטיות. כאשר מציגים התקדמות בפני ממשל או מכינים חבילת תיעוד למפעילים, צרפו את החומרים הבאים:

| ארטיפקט | שימוש |
|---------|-------|
| [`sdk/android/telemetry_redaction.md`](./sdk/android/telemetry_redaction.md) | מסמך המדיניות הראשי – האשינג של authority, טבלת בקטים של התקנים, זמני שימור ותהליך override. |
| [`android_runbook.md`](./android_runbook.md#2-telemetry--redaction) | הוראות תפעול (השחלת `ClientConfig`, בדיקות יצואן, טיפול ב-override). |
| [`sdk/android/readiness/signal_inventory_worksheet.md`](./sdk/android/readiness/signal_inventory_worksheet.md) | מטריצה של בעלי תפקידים, בדיקות ותיעוד הוכחות לכל span/metric. |
| [`sdk/android/telemetry_chaos_checklist.md`](./sdk/android/telemetry_chaos_checklist.md) | רשימת תרחישי הכאוס הרבעונים שעליהם נשענת חותמת הממשל. |
| [`sdk/android/readiness/and7_operator_enablement.md`](./sdk/android/readiness/and7_operator_enablement.md) | מערך ההכשרה (enablement) וה-Knowledge check עבור צוותי התמיכה והאונ-קול. |

**Guardrails מרכזיים**

- האירועים `android.torii.http.request` / `android.torii.http.retry` מבצעים האשינג עם Blake2b-256 תוך שימוש במלח הרבעוני שמוגדר ב-`iroha_config.telemetry.redaction_salt`. עקבו אחרי `android.telemetry.redaction.salt_version` בזמן רוטציות.
- חשיפת פרטי חומרה מוגבלת לבקטים של `android.telemetry.device_profile` (גרסת SDK ראשית + ‎`emulator|consumer|enterprise`). השוו מול ההיסטוגרמה של `node.hardware_profile` ופתחו חקירה אם הפער גדול מ-10%.
- מידע רשת כולל רק `network_type` ו-`roaming`; אין ייצוא של שמות מפעילים. בקשות למידע מנוי צריכות להיפתח כ-override רשמי.
- כל override נרשם בסיגנל `android.telemetry.redaction.override` וגם בקובץ `docs/source/sdk/android/telemetry_override_log.md`. חובה לעדכן את היומן מיד לאחר הפעלה וביטול.

**בדיקות תפעול מומלצות**

- הריצו `scripts/telemetry/check_redaction_status.py` מול הסביבות כדי להפיק את חבילת הסטטוס שמצורפת לתרגילי הכאוס ולדוחות אירועים.
- דוחות כאוס צריכים לכלול צילומי מסך של Grafana ואת הערכים העדכניים של `android.telemetry.redaction.failure` ו-`android.telemetry.redaction.override`, מאוחסנים תחת `docs/source/sdk/android/readiness/labs/`.
- שאילתות PromQL שימושיות:
  - `increase(android.telemetry.redaction.failure_total[5m]) > 0` אמורה להדליק PagerDuty מחוץ לחלון הכאוס.
  - `sum by (device_profile)(android.telemetry.device_profile)` מול המדד `node.hardware_profile` בצמתי Rust מאשר התאמת בקטים.
  - `clamp_min(rate(android.telemetry.redaction.override_total[1h]), 0)` מזין את בקרה חודשית על overrides.

למידע על הסלמות ותמיכה, ראו את [`android_support_playbook.md`](./android_support_playbook.md#8-telemetry-redaction--observability-and7).

## פרופיל ← מסלולי Torii ← Runtime

הפרופיל נבחר באמצעות `iroha_config`. אלגוריתם ההשחלה:

1. בעת האתחול, `Telemetry::new` יוצר ידית Runtime עם מסנני פרופיל.
2. `Torii::new_with_handle` מקבל `routing::MaybeTelemetry` שמצמיד את הידית לפרופיל הפעיל. משתמשים ב-`routing::MaybeTelemetry::from_profile(runtime_handle, profile)` או ב-`disabled()` כאשר טלמטריה אינה זמינה.
3. כאשר הפיצ'ר `telemetry` פעיל, `Torii::new` משמש כעטיפת נוחות המאפסנת פרופיל `operator`. מבחנים יכולים להשתמש ב-`routing::MaybeTelemetry::for_tests()` כדי לקבל ידית טלמטריה בתוך התהליך.

## מדדים עיקריים

### Sumeragi / Pacemaker

- `sumeragi_view_changes_total`: ספירת שינויי View.
- `sumeragi_heartbeat_view_index`: view נוכחי לפי heartbeat.
- `sumeragi_new_view_height`: גובה הבלוק של קבלת NEW_VIEW אחרונה.
- `pacemaker_backpressure_deferrals_total`: מספר דחיות pacemaker בגלל עומס (עומס בתור, backlog של relay/RBC או בלוק ממתין שחוסם הצעה).
- `sumeragi_block_created_dropped_by_lock_total`, ‏`hint_mismatch_total`, ‏`proposal_mismatch_total`: מדדי נשירה של הצעות.
- `sumeragi_da_gate_block_total{reason="missing_local_data"}`: מונה של הפעלות מחדש של עיבוד DA כאשר לא ניתן היה להרכיב availability evidence בזמן.
- `sumeragi_rbc_sessions_active`: סשני RBC פעילים כרגע.
- `sumeragi_collectors_targeted_current`, ‏`sumeragi_collectors_targeted_last_per_block`: אינדיקציה לחלוקת אספנים.
- `sumeragi_vrf_*`: מדדי השתתפות VRF (ללא השלמה, ללא השתתפות, איחורים).
- `sumeragi_membership_view_hash`, ‏`sumeragi_membership_height`, ‏`sumeragi_membership_view`, ‏`sumeragi_membership_epoch`: גיבוב דטרמיניסטי של רשימת המאמתים יחד עם הקשר `(height, view, epoch)` לצורך השוואה ישירה בין צמתים.

### Torii / תורים

- `torii_tx_queue_depth`, ‏`torii_tx_queue_capacity`, ‏`torii_tx_queue_saturated`: מצב תור הטרנזקציות.
- `torii_ws_sessions_active`, ‏`torii_http_requests_total`: עומס Torii.
- היסטוגרמות latency לאנדפוינטים, לדוגמה `torii_submit_block_latency_ms_bucket`.

### צנרת IVM

- `pipeline_stage_ms`: היסטוגרמה של זמני שלבים (תווית `stage` עם `access`,‏`overlays`,‏`dag`,‏`schedule`,‏`apply`,‏`layers_prep`,‏`layers_exec`,‏`layers_merge`).
- `pipeline_dag_vertices`, ‏`pipeline_dag_edges`, ‏`pipeline_conflict_rate_bps`: צורת DAG של הבלוק המאומת האחרון ושיעור קונפליקטים (bp).
- `pipeline_access_set_source_total{source=manifest_hints|entrypoint_hints|prepass_merge|conservative_fallback}`: מוני מקור גזירת Access Set לפי מקור.
- `pipeline_overlay_*`: סטטיסטיקות Overlay (מספר, הוראות, בתים).
- `pipeline_peak_layer_width`, ‏`pipeline_layer_avg_width`, ‏`pipeline_layer_median_width`: נתוני רוחב שכבות.
- `pipeline_scheduler_utilization_pct`: ניצול מקביליות ממוצע.

### פרופילים יקרים

עד הפעלת פרופיל `extended` או `developer`, רק מדדים שאינם יקרים זמינים. מדדים יקרים (למשל היסטוגרמות latency מפורטות) נאספים רק כאשר הפרופיל מאפשר זאת.

## דוגמאות PromQL

### בקרת Pacemaker

```promql
increase(pacemaker_backpressure_deferrals_total[5m])
```

פיק ספייק מרמז על עומס ברשת, עומס בתור, backlog של relay/RBC או בלוק ממתין שחוסם הצעה. אפשר לשלב עם

```promql
histogram_quantile(0.95, sum(rate(sumeragi_phase_latency_ms_bucket[5m])) by (phase, le))
```

כדי לראות אילו שלבים התנפחו.

### מעקב VRF

```promql
increase(sumeragi_vrf_committed_no_reveal_total[15m])
```

מזהה Commit ללא חשיפה. עבור `no_participation_total` או `late_reveals_total` ניתן לייצר התראות נפרדות. הצמידו את המדד לפרמטרים
`vrf_penalty_epoch` ו-`collectors_targeted_current`.

### ניתור מסלולי RBC

```promql
increase(sumeragi_da_gate_block_total{reason="missing_local_data"}[5m])
```

מקבץ ניסיונות חוזרים לפי סיבה (לדוגמה `payload_hash_mismatch`, ‏`expired`).

```promql
max(sumeragi_rbc_sessions_active)
```

לכמות הסשנים הפעילים.

### Queue ו-Backpressure

```promql
max_over_time(torii_tx_queue_depth[1m])
```

```promql
torii_tx_queue_depth / torii_tx_queue_capacity
```

יחס עומס התור. אם מעל 0.8 לאורך זמן, כדאי לבדוק `torii_tx_queue_saturated`.

### Dashboards לדוגמה

- Grafana (Prometheus datasource):
  - גרף `View Changes` עם `increase(sumeragi_view_changes_total[1m])`.
  - טבלת `RBC Sessions` עם שאילתה: `sumeragi_rbc_sessions_active`.
  - לוח `Proposal Drops` המציג את המונים המצטברים לנשירה מהצעות.
- Kibana / Loki: פאנל לוגים מסנן `pacemaker_backpressure` או `proposal mismatch`.

## התראות פעילות

### Pacemaker Backpressure

- **Page**: `increase(pacemaker_backpressure_deferrals_total[5m]) > 0` פעמיים רצופות.
- **Ticket**: `increase(pacemaker_backpressure_deferrals_total[30m]) > 0` למעקב מגמת עומס.

תהליך טיפול:
1. לאסוף `/status` ולבדוק `pacemaker_backpressure_deferrals_total`.
2. לבדוק את עומק תור העסקאות, עומק/סטטוס RBC, ואותות backpressure של relay או בלוקים ממתינים.
3. להשוות זמני שלבים ב-`/v2/sumeragi/phases`.

### חוסר פעילות VRF

- להתריע אם:
  ```promql
  increase(sumeragi_vrf_no_participation_total[15m]) > 0
  ```
  או:
  ```promql
  increase(sumeragi_vrf_committed_no_reveal_total[15m]) > 0
  ```

בדיקת תקלות:
1. לבדוק `/v2/sumeragi/leader` / `/v2/sumeragi/status` לראות מי היעד.
2. לאסוף לוגי VRF ולוודא שחשיפות הגיעו.
3. לעדכן את צוות ההפעלה אם מדובר ברכזים בעייתיים.

### נשירת הצעות

- להתריע על גידול:
  ```promql
  increase(sumeragi_block_created_proposal_mismatch_total[5m]) > 0
  ```
- להוסיף התראה על `block_created_hint_mismatch_total`.

### הפעלות Governance

- להתריע כאשר `increase(governance_manifest_activations_total{event="instance_bound"}[30m]) == 0` במהלך חלון שדרוג.
- לנטר `governance_proposals_status{status="proposed"}` אם נשאר חיובי מעל SLA (למשל 24 שעות) בזמן ש-`status="approved"` לא משתנה.
- לנטר `increase(governance_manifest_admission_total{result!="allowed"}[5m])` כדי לזהות דחיות מתמשכות (manifest חסר, quorum חסר, או מדיניות שנחסמה).

בדיקת תקלות:
1. להריץ `iroha_cli app gov deploy audit --namespace <ns>` ולוודא `code_hash`/`abi_hash`.
2. לבדוק `governance.recent_manifest_activations` ב-`/status`.
3. לנטר `increase(governance_protected_namespace_total{outcome="rejected"}[5m])`
   ו-`increase(governance_manifest_quorum_total{outcome="rejected"}[5m])`.
4. לוודא שהמספרים עבור `approved` ו-`enacted` מתנהגים כמצופה.
5. לנטר `increase(governance_manifest_hook_total{hook="runtime_upgrade", outcome="rejected"}[5m])`
   כדי לזהות טרנזקציות שדרוג שנחסמו על ידי מדיניות המניפסט; במקרה של קפיצה,
   לבדוק את לוגי הקבלה של Torii ולוודא ש־allowlist והמטא־נתונים מתואמים להצעה
   שגרמה להפעלה.

### Torii ZK Attachments & Prover

- מדריך מלא זמין ב-`docs/source/zk/prover_runbook.md`.

**דשבורדים**
- לייבא את `docs/source/grafana_zk_prover.json` עבור עומק תורים, השהיה ותקציב.
- להשוות:
  ```promql
  histogram_quantile(0.95, sum(rate(zk_verify_latency_ms_bucket[5m])) by (le, backend))
  ```
  מול:
  ```promql
  histogram_quantile(0.95, sum(rate(torii_zk_prover_latency_ms_bucket[5m])) by (le))
  ```

**התראות**
- Backlog: `avg_over_time(torii_zk_prover_pending[10m]) > 0`.
- תקציב: `increase(torii_zk_prover_budget_exhausted_total{reason="bytes"}[30m]) > 0`.
- Latency: חריגה של ‎95%‎ מעל `torii.zk_prover_max_scan_millis` ליותר מ-15 דקות.

## מטריקות Nexus

### נראות Torii Norito-RPC

דרישות הנראות של תעבורת Norito-RPC מתועדות ב-`docs/source/torii/norito_rpc_telemetry.md`. המדדים המרכזיים:

- `torii_request_duration_seconds_bucket{scheme}` — היסטוגרמת לטנסי לכל שיטת חיבור; סינון לפי `scheme="norito_rpc"` מזין את פאנלי ה-burn-in.
- `torii_request_failures_total{scheme,code}` — מונה שגיאות שממוין לפי השיטה וקוד ה-HTTP.
- `torii_http_requests_total{content_type,status,method}` — מונה בקשות שמבדיל בין `application/x-norito` ל-JSON לצורכי פריטי התאמה.
- `torii_http_request_duration_seconds_bucket{content_type,method}` — היסטוגרמת לטנסי שמאפשרת להצליב בין JSON ל-Norito.
- `torii_http_response_bytes_total{content_type,method,status}` — מעקב אחרי גדלי תגובות כדי לזהות נסיגות בגודל המטען.
- `torii_norito_decode_failures_total{payload_kind,reason}` — מבטא כמה בקשות נכשלו בבדיקת הכותרת (קסם, checksum, דגלי layout).
- `torii_address_invalid_total{surface,reason}` ו-`torii_address_local8_total{surface}` — נשארים רלוונטיים גם ב-Norito משום שהן כוחניות לגבי כתובות I105/דחוסות.
- מדי חיבורים וגייטינג קיימים (`torii_active_connections_total{scheme}`, `torii_pre_auth_reject_total{reason}`) מוכרחים לכלול את `scheme="norito_rpc"` כדי להבחין בין ערוצים.

חוקי ההתראה שמורים ב-`dashboards/alerts/torii_norito_rpc_rules.yml` (עם בדיקות ב-`dashboards/alerts/tests/torii_norito_rpc_rules.test.yml`). דגשים:

- `ToriiNoritoRpcErrorSpike` — מזהה רצף של שגיאות 5xx במקטע של חמש דקות.
- `ToriiNoritoRpcLatencyDegraded` — נדלק כאשר P95 חוצה ‎750‎ מ״ש.
- `ToriiNoritoRpcSilentTraffic` — מזהיר כאשר לא נרשמות בקשות Norito במשך ‎30‎ דקות (סימן להפניית תעבורה שגויה).

להרצת בדיקות `promtool` מקומית/ב-CI השתמשו ב-`scripts/telemetry/test_torii_norito_rpc_alerts.sh`.

#### נוהל טיפול ב-Norito RPC במצב ירוד

הנוהל הבא מיועד למצבי חריגה במדדי Norito או כאשר אחת ההתראות למעלה הופעלו:

- **אותות ראשיים:** `ToriiNoritoRpcErrorSpike`, ‏`ToriiNoritoRpcLatencyDegraded`, ‏`ToriiNoritoRpcSilentTraffic`, מונה הדקוד `torii_norito_decode_failures_total`, ופאנלי הלטנסי ב-`dashboards/grafana/torii_norito_rpc_observability.json`.
- **אימות מצד ה-SDK:** עבודות ה-CI וה-Mock Harness מדווחות `torii_mock_harness_retry_total`, ‏`torii_mock_harness_duration_ms`, ‏`torii_mock_harness_fixture_version`. קפיצה במדדים אלה מרמזת על רגרסיה בלקוח עוד לפני השפעה על פרודקשן.
- **כלי סיוע:** `python/iroha_python/scripts/run_norito_rpc_smoke.sh` בודק end-to-end ו-`scripts/telemetry/test_torii_norito_rpc_alerts.sh` מאשר שהחוקים נשארו תקפים לאחר תיקון.

**צעדי תגובה מידיים**

1. לאבחן את סוג התקלה:  
   • קפיצת שגיאות – `torii_request_failures_total{scheme="norito_rpc"}`/decoded עולה.  
   • חריגת לטנסי – `histogram_quantile(0.95, ...)` מעל ‎750‎ מ״ש.  
   • תעבורה שקטה – `torii_request_duration_seconds_count` נשאר אפס בעוד HTTP רגיל פעיל.
2. לבדוק יומני Torii עם סינון על `ConnScheme::NoritoRpc` עבור הודעות `schema_mismatch`, ‏TLS שגוי או checksum.
3. לאמת שהפרוקסי/Ingress מעבירים את הכותרת `Content-Type: application/x-norito` באמצעות `curl …/rpc/ping` או הפעלת ה-harness מאזורים שונים; תגובה JSONית מצביעה על הסרה של הכותרת.
4. להשוות עם מדדי ה-SDK. אם רק CI רואה קפיצה – לעצור את רולאאוט הלקוח ולהשאיר את השרת פעיל.

**אסטרטגיות מיתון**

- בלימת לקוחות ספציפיים דרך `torii.preauth_scheme_limits.norito_rpc`.
- התאמת פערי סכמות/fixtures: להשוות `fixtures/norito_rpc/schema_hashes.json` (טבלת DTO→hash) ולהריץ `cargo xtask norito-rpc-fixtures --all` בעת צורך.
- תקלות פרוקסי/MTU: לתקן, ואז להריץ מחדש את ה-smoke test.
- ברנאאוט מלא: אם חייבים חזרה ל-JSON, לשנות `torii.transport.norito_rpc.stage` ל-`canary`/`disabled`, להטעין את Torii, ולהסתמך על `/rpc/capabilities` כדי ש-SDKים יגלו את השינוי. לתעד ב-`docs/source/runbooks/torii_norito_rpc_canary.md`.

**אימות לאחר התיקון**

1. לוודא ש`torii_request_failures_total` ו-`torii_norito_decode_failures_total` חוזרים לבסיס.
2. לבדוק שההתראות ירדו מהלוח ו-`torii_active_connections_total{scheme="norito_rpc"}` התייצב.
3. להריץ מחדש את `python/iroha_python/scripts/run_norito_rpc_smoke.sh` ואת בדיקות החוקים.
4. לצלם פאנלים, לשמור פלטי CLI ולצרף לסטטוס NRPC-2 כך שהראיות זמינות לביקורת.

### ניטור סטיות קונפיגורציה

- `nexus_config_diff_total{knob,profile}` (מונה) – עולה בכל פעם שהתצורה הפעילה חורגת מקונפיגורציית הבסיס חד-הנתיבית. התווית `knob` מציינת את תחום ההבדל (לדוגמה `nexus.lane_catalog.count`, ‏`nexus.routing.rules`, ‏`nexus.da`) ו-`profile` מקבל את הערך `active`.

**התראה לדוגמה**

```
increase(nexus_config_diff_total{profile="active"}[5m]) > 0
```

יש לעקוב אחרי התנאי ולפתוח דף כאשר העלייה מתרחשת מחוץ לחלון תחזוקה. כל עלייה מייצרת גם רשומת יומן `telemetry` בשם `nexus.config.diff` עם מטען Norito JSON (שדות `baseline` ו-`current`), שניתן לבחון במהלך ריענון הקונפיגורציה ובתרגיל `TRACE-TELEMETRY-BRIDGE`.

- `nexus_lane_configured_total` (מד) – מציג כמה רשומות קטלוג Nexus נטענו בצומת הנוכחי. השוואה לערך הצפוי (למשל ‎1‎ בפריסה חד-נתיבית או ‎3‎ בפריסת Nexus מרובת נתיבים) מסייעת לזהות צמתים שהוגדרו בצורה שגויה בטרם ביקורות הנתיב.

**התראה לדוגמה**

```
nexus_lane_configured_total != EXPECTED_LANE_COUNT
```

**תהליך טיפול**
1. לבדוק `torii_zk_prover_pending`, ‏`torii_zk_prover_inflight`, ‏`torii_zk_prover_last_scan_ms`.
2. לעיין בלוגי Torii (היעד `torii::zk_prover`).
3. לאשר ערכים ב-`iroha_config` תחת `[torii]`.
4. להסיר/להריץ מחדש מצורפים בעייתיים עם `iroha_cli app zk attachments delete`.
5. לתעד שינויי סף במחברת האופרטיבית ולעדכן הערות בפאנלים.

### תוצאות ביקורת TRACE

- רשומת `telemetry` בשם `nexus.audit.outcome` נוצרת על ידי `Telemetry::record_audit_outcome` עם סיום כל חלון ביקורת מתוזמן. המטען בפורמט Norito כולל `trace_id`, גובה סלוט (`slot_height`), שם הסוקר (`reviewer`), סטטוס (`status`, כגון `pass`/`fail`/`mitigated`) וקישור מיתון אופציונלי (`mitigation_url`).
- Prometheus מפרסם את `nexus_audit_outcome_total{trace_id,status}` ואת `nexus_audit_outcome_last_timestamp_seconds{trace_id}` לעזר בניטור ודשבורדים.

**תהליך מומלץ**
1. במהלך חזרות TRACE לעקוב אחרי הזרם (`journalctl -u irohad -o json` או גשר OTLP) ולהבטיח שנוצר אירוע לכל חלון.
2. לשמור את מטען ה-JSON ביחד עם ארטיפקטי הביקורת כדי לבסס עקיבות ולקשר למסמך המיתון.
3. להריץ את `scripts/telemetry/check_nexus_audit_outcome.py` (לדוגמה `--trace-id TRACE-TELEMETRY-BRIDGE --window-start 2026-02-24T10:00Z --window-minutes 30`) על היומן. הסקריפט יוודא שהאירוע קיים, יכשיל סטטוסים אסורים (ברירת מחדל `fail`) וישמור את המטען תחת `docs/examples/nexus_audit_outcomes/` לצורכי ביקורת.
4. להגדיר התראה כאשר `status="fail"` או אם לא התקבל אירוע בתוך ‎30‎ דקות מהחלון – כלל Prometheus החדש (`dashboards/alerts/nexus_audit_rules.yml`) מכסה סטטוסים כושלים, בעוד ה- CI צריך להשתמש בסקריפט לעיל כדי לאכוף חלונות חסרי אירוע.

</div>

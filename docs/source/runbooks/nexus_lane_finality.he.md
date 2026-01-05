---

> NOTE: This translation has not yet been updated for the v1 DA availability (advisory). Refer to the English source for current semantics.
lang: he
direction: rtl
source: docs/source/runbooks/nexus_lane_finality.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1d9fbcf5301bd36a0bdc8a010ae214f7b1561373237054ab3c8a45a411cf6c0e
source_last_modified: "2025-12-14T09:53:36.241505+00:00"
translation_last_reviewed: 2025-12-28
---

<div dir="rtl">

# ראנבוק סופיות ליינים של Nexus ואורקל

**סטטוס:** פעיל — עומד ביעד NX-18 (dashboard/runbook).  
**קהל יעד:** Core Consensus WG, SRE/Telemetry, Release Engineering, מובילי on‑call.  
**היקף:** מכסה את SLOs של משך סלוט, קוורום DA, אורקל ובאפר הסדרים שמאבטחים את
הבטחת הסופיות של 1 שנייה. להשתמש יחד עם `dashboards/grafana/nexus_lanes.json` וכלי
הטלמטריה תחת `scripts/telemetry/`.

## Dashboards

- **Grafana (`dashboards/grafana/nexus_lanes.json`)** — מפרסם את לוח “Nexus Lane Finality & Oracles”. הפאנלים עוקבים אחר:
  - `histogram_quantile()` על `iroha_slot_duration_ms` (p50/p95/p99) + מדד הדגימה האחרונה.
  - `iroha_da_quorum_ratio` ו־`increase(sumeragi_da_gate_block_total{reason="missing_local_data"}[5m])` להבלטת churn של DA.
  - פני האורקל: `iroha_oracle_price_local_per_xor`, `iroha_oracle_staleness_seconds`, `iroha_oracle_twap_window_seconds`, `iroha_oracle_haircut_basis_points`.
  - פאנל באפר הסדרים (`iroha_settlement_buffer_xor`) המציג דביטים פר‑ליין מה‑`LaneBlockCommitment`.
- **כללי התראה** — משתמשים מחדש בסעיפי Slot/DA SLO מתוך `ans3.md`. Page כאשר:
  - p95 של משך סלוט > 1000 ms בשני חלונות 5 דקות רצופים,
  - יחס קוורום DA < 0.95 או `increase(sumeragi_da_gate_block_total{reason="missing_local_data"}[5m]) > 0`,
  - staleness של האורקל > 90 שניות או חלון TWAP ≠ 60 שניות,
  - באפר הסדרים < 25 % (soft) / 10 % (hard) לאחר שהמדד מופעל.

## דף עזר למטריקות

| מטריקה | יעד / התראה | הערות |
|--------|----------------|-------|
| `histogram_quantile(0.95, iroha_slot_duration_ms)` | ≤ 1000 ms (hard), 950 ms warning | להשתמש בפאנל הדשבורד או להריץ `scripts/telemetry/check_slot_duration.py` (`--json-out artifacts/nx18/slot_summary.json`) מול יצוא Prometheus שנאסף בזמן chaos. |
| `iroha_slot_duration_ms_latest` | משקף את הסלוט האחרון; לבדוק אם > 1100 ms גם כשהקוואנטילים נראים תקינים. | לייצא את הערך בעת פתיחת אירוע. |
| `iroha_da_quorum_ratio` | ≥ 0.95 בחלון מתגלגל של 30 דקות. | נגזר מ‑DA reschedules במהלך commit בלוקים. |
| `increase(sumeragi_da_gate_block_total{reason="missing_local_data"}[5m])` | צריך להישאר 0 מחוץ לרהרסלים. | כל עלייה מתמשכת נספרת כ‑`missing-availability warning`. |

כל reschedule מפעיל גם אזהרת pipeline ב‑Torii עם `kind = "missing-availability warning"`. ללכוד אירועים אלו יחד עם קפיצת המטריקה כדי לזהות את כותרת הבלוק, ניסיון ה‑retry ומוני requeue בלי לנבור בלוגים של הוולידטורים.【crates/iroha_core/src/sumeragi/main_loop.rs:5164】
| `iroha_oracle_staleness_seconds` | ≤ 60 שניות. התראה ב‑75 שניות. | מציין הזנות TWAP של 60 שניות שהתיישנו. |
| `iroha_oracle_twap_window_seconds` | בדיוק 60 שניות ± 5 שניות. | סטייה = אורקל לא מוגדר נכון. |
| `iroha_oracle_haircut_basis_points` | תואם ל‑tier הנזילות של הליין (0/25/75 bps). | להסלים אם haircuts עולים באופן חריג. |
| `iroha_settlement_buffer_xor` | Soft 25 %, hard 10 %. מתחת ל‑10 % להכריח XOR‑only. | הפאנל מציג דביטים micro‑XOR פר‑ליין/דאטה‑ספייס; לייצא לפני שינוי מדיניות router. |

## Playbook תגובה

### חריגה במשך סלוט
1. אימות דרך הדשבורד + `promql` (p95/p99).  
2. לכידת פלט `scripts/telemetry/check_slot_duration.py --json-out <path>` (וגם snapshot מדדים)
   כדי שמבקרי CXO יוכלו לאשר את שער 1 שנייה.  
3. בדיקת קלטי RCA: עומק תור mempool, reschedules של DA, עקבות IVM.  
4. פתיחת אירוע, לצרף צילום Grafana, ולתזמן chaos drill אם ההחמרה נמשכת.

### ירידת קוורום DA
1. בדיקת `iroha_da_quorum_ratio` ומונה reschedule; לקשור ללוגים `missing-availability warning`.  
2. אם ratio <0.95, לנעול attesters כושלים, להרחיב פרמטרי sampling או לעבור למצב XOR‑only.  
3. להריץ `scripts/telemetry/check_nexus_audit_outcome.py` בזמן routed‑trace rehearsals כדי להוכיח ש־`nexus.audit.outcome` ממשיך לעבור לאחר התיקון.  
4. לארכב bundles של DA receipts יחד עם טיקט האירוע.

### Staleness אורקל / סטייה ב‑haircut
1. להשתמש בפאנלים 5–8 כדי לאמת מחיר, staleness, חלון TWAP ו‑haircut.  
2. אם staleness >90 שניות: להפעיל מחדש או לבצע failover של feed האורקל, ואז להריץ שוב את chaos harness.  
3. במקרה של mismatch: לבדוק את פרופיל הנזילות ושינויים אחרונים בגובירננס; להודיע ל‑treasury אם נדרש טיפול.

### התראות באפר הסדרים
1. להשתמש ב־`iroha_settlement_buffer_xor` (ובתיעודי לילה) כדי לוודא מרווח לפני שינוי מדיניות router.  
2. כאשר המדד חוצה סף, לבצע:
   - **Soft breach (<25 %)**: לערב treasury, לשקול משיכת קווי swap, ולתעד את ההתראה.  
   - **Hard breach (<10 %)**: להכריח XOR‑only, לסרב ל‑subsidised lanes ולתעד ב־`ops/drill-log.md`.  
3. לעיין ב־`docs/source/settlement_router.md` למנופי repo/reverse‑repo.

## ראיות ואוטומציה

- **CI** — לחבר `scripts/telemetry/check_slot_duration.py --json-out artifacts/nx18/slot_summary.json` ו־`scripts/telemetry/nx18_acceptance.py --json-out artifacts/nx18/nx18_acceptance.json <metrics.prom>` לזרימת קבלת RC כך שכל RC יוציא סיכום משך סלוט ותוצאות שערי DA/oracle/buffer לצד snapshot המדדים לעיל. הכלי כבר מופעל מתוך `ci/check_nexus_lane_smoke.sh`.  
- **Parity דשבורד** — להריץ `scripts/telemetry/compare_dashboards.py dashboards/grafana/nexus_lanes.json <prod-export.json>` כדי לוודא שהלוח המפורסם תואם ל‑exports של staging/prod.  
- **ארטיפקטי trace** — במהלך TRACE rehearsals או NX-18 chaos drills, להריץ `scripts/telemetry/check_nexus_audit_outcome.py` כדי לארכב את `nexus.audit.outcome` האחרון (`docs/examples/nexus_audit_outcomes/`). לצרף את הארכיון וצילומי Grafana ל‑drill log.
- **Bundling ראיות סלוט** — אחרי יצירת summary JSON, להריץ `scripts/telemetry/bundle_slot_artifacts.py --metrics <prometheus.tgz-extract>/metrics.prom --summary artifacts/nx18/slot_summary.json --out-dir artifacts/nx18` כך שה־`slot_bundle_manifest.json` יתעד digests SHA‑256 לשני הארטיפקטים. להעלות את הספרייה כפי שהיא עם evidence bundle של RC. צינור ההפצה מפעיל זאת אוטומטית (ניתן לדלג עם `--skip-nexus-lane-smoke`) ומעתיק את `artifacts/nx18/` לפלט הריליס.

## רשימת תחזוקה

- לשמור את `dashboards/grafana/nexus_lanes.json` מסונכרן עם exports של Grafana אחרי כל שינוי סכימה; לתעד עריכות בהודעות commit עם הפניה ל‑NX-18.  
- לעדכן את הראנבוק כאשר נכנסות מטריקות חדשות (למשל gauges של באפר הסדרים) או ספים חדשים.  
- לרשום כל chaos rehearsal (latency סלוט, jitter DA, עצירת אורקל, התרוקנות באפר) באמצעות `scripts/telemetry/log_sorafs_drill.sh --log ops/drill-log.md --program NX-18 --status <status>`.

עמידה בראנבוק זה מספקת את ראיות “dashboards/runbooks למפעיל” הנדרשות ב‑NX-18 ומבטיחה שה‑SLO של הסופיות ניתן לאכיפה לפני Nexus GA.

</div>

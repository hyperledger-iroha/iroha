<!-- Hebrew translation of docs/source/project_tracker/npos_sumeragi_phase_a.md -->

---
lang: he
direction: rtl
source: docs/source/project_tracker/npos_sumeragi_phase_a.md
status: complete
translator: manual
---

<div dir="rtl">

% מעקב NPoS Sumeragi שלב A (דצמבר 2025)

נשלח ב־2025-12-03 — טבלת הרצף הופצה לאישור אצל `@sumeragi-core`,‏ `@telemetry-ops`,‏ `@torii-sdk`,‏ `@governance`,‏ `@qa-consensus`,‏ `@performance-lab`,‏ `@operator-docs`. בעלי המשימות מתבקשים להשיב בשרשור המשותף (#npos-phase-a-sync) עם עדכוני קבלה או סיכוני תלות.

| מזהה כרטיס | אבן דרך | תקציר | בעלי משימה | תלות | יעד סיום | הערות |
|-----------|-----------|---------|--------|--------------|-------------|-------|
| GA-A4.1 | A4 — אספן וצינור אקראיות | להשלמת בחירת אספן מונעת-PRF ולהציג צילום-מצב דטרמיניסטי של הזרע ב-`status`/‏telemetry. | `@sumeragi-core` | מדדי פייסמייקר/DA (A3) | 2026-01-05 | לבצע סקירת דגלי CLI עם `@torii-sdk` לפני מיזוג. |
| GA-A4.2 | A4 — אספן וצינור אקראיות | לחשוף מדדי השתתפות בחשיפה ופיקוד CLI לבדיקות; לפרסם עדכוני מניפסט Norito. | `@telemetry-ops`, `@torii-sdk` | GA-A4.1 | 2026-01-19 | להוסיף תבניות התראה ל-Prometheus על החלקת חשיפה. הושלם (סיכום טלמטריה + CLI נחתו בדצמבר 2025). |
| GA-A4.3 | A4 — אספן וצינור אקראיות | לעגן מבחני שחזור חשיפה מאוחרת ואפס-השתתפות ב-`integration_tests/tests/sumeragi_randomness.rs`. | `@sumeragi-core` | GA-A4.1 | 2026-01-31 | הושלם (מונה הטלמטריה ננעל על ידי `npos_late_vrf_reveal_clears_penalty_and_preserves_seed` ו-`npos_zero_participation_epoch_reports_full_no_participation`). |
| GA-A5.1 | A5 — קונפיגורציה משותפת וראיות | לאכוף שער הפעלה משותף (המערך הישן מחויב, החדש מופעל +1) ולהרחיב כיסוי אינטגרציה. | `@sumeragi-core` | GA-A4.3 | 2026-02-21 | הושלם — מבחני אינטגרציה מכסים עתה את סמנטיקת פער ההפעלה; סיכומי התרגול נשמרו אצל הממשל. |
| GA-A5.2 | A5 — קונפיגורציה משותפת וראיות | לעדכן מסמכי ממשל/CLI לזרימות ענישה וכליאה; להוסיף doctests ל-mdbook. | `@governance`, `@torii-sdk` | GA-A5.1 | 2026-03-05 | הושלם — מסמכים, מסייעי CLI ו-doctests של mdBook עודכנו עם דוגמאות Norito. |
| GA-A5.3 | A5 — קונפיגורציה משותפת וראיות | להרחיב מבחני ראיות לנתיבי כשל (חותם כפול, חתימה מזויפת, שחזור אפוק מיושן). | `@sumeragi-core`, `@qa-consensus` | GA-A5.1 | 2026-03-14 | הושלם — פיקסצ'רים לפאזז והרצות לילה מגנות על חותם כפול, חתימה מזויפת, אופק ישן ותסריטי מניפסט מעורבים. |
| GA-A6.1 | A6 — כלי עזר, תיעוד ואימות | לאוטומט מבחן happy-path עם VRF ל-4 צמתים כולל ספי טלמטריה ואימות שערי RBC. | `@qa-consensus`, `@telemetry-ops` | GA-A5.3 | 2026-04-07 | הושלם — מבחן happy-path של NPoS רץ ב-CI עם ספי פייסמייקר/RBC מתועדים ב-runbook. |
| GA-A6.2 | A6 — כלי עזר, תיעוד ואימות | לקבע בסיס ביצועים של NPoS (בלוקים של 1 שנייה, k=3) ולרשום מדדים ב-`status.md`/במסמכי המפעיל. | `@performance-lab`, `@telemetry-ops` | GA-A6.1 | 2026-04-21 | הושלם — Apple M2 Ultra (‏24 ליבות, ‏192 GB RAM, ‏macOS 15.0); ראו `docs/source/generated/sumeragi_baseline_report.md`. |
| GA-A6.3 | A6 — כלי עזר, תיעוד ואימות | לפרסם מדריכי פתרון תקלות למפעילים עבור RBC/פייסמייקר/מדדי backpressure. | `@operator-docs`, `@telemetry-ops` | GA-A6.1 | 2026-04-28 | הושלם — runbook לפתרון תקלות נוסף ב-`docs/source/telemetry.md:523`; סקריפט `scripts/sumeragi_backpressure_log_scraper.py` מספק כעת קורלציית לוגים אוטומטית לדחיות/RBC. |
| GA-A6.4 | A6 — כלי עזר, תיעוד ואימות | להרחיב את ה-harness הביצועי עם תרחישי backpressure של חנות RBC ואובדן מקטעים (`npos_rbc_store_backpressure_records_metrics`,‏ `npos_rbc_chunk_loss_fault_reports_backlog`). | `@performance-lab`, `@telemetry-ops` | GA-A6.2 | 2026-05-05 | הושלם — נסיונות fan-out חוזרים מכוסים ב-`npos_redundant_send_retries_update_metrics`, ואימות תחומי jitter של פייסמייקר ב-`npos_pacemaker_jitter_within_band` (ראו `cargo test -p integration_tests --test sumeragi_npos_performance`). |

</div>

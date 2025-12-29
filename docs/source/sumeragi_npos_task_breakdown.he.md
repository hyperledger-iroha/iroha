<!-- Hebrew translation of docs/source/sumeragi_npos_task_breakdown.md -->

---
lang: he
direction: rtl
source: docs/source/sumeragi_npos_task_breakdown.md
status: needs-update
translator: manual
---

<div dir="rtl">

> NOTE: This translation has not yet been updated for the v1 DA availability (advisory). Refer to `docs/source/sumeragi_npos_task_breakdown.md` for current semantics.

## פירוק משימות Sumeragi + NPoS

מסמך זה פורט את מפת הדרכים של שלב A למשימות הנדסיות קטנות, כך שנוכל להמשיך להטמיע את Sumeragi/NPoS בצורה הדרגתית. סימוני הסטטוס: `✅` הושלם, `⚙️` בתהליך, `⬜` טרם התחיל, `🧪` דורש בדיקות.

### A2 — אימוץ מסרים ברמת ה-wire
- ✅ הצגת טיפוסי Norito `Proposal`/`Vote`/`Qc` ב-`BlockMessage` והרצת round-trip encode/decode (`crates/iroha_data_model/tests/consensus_roundtrip.rs`).
- ✅ חסימת המסגרות הישנות `BlockSigned/BlockCommitted`; מתג ההגירה הוגדר ל-`false` לפני ההוצאה משימוש.
- ✅ הסרת מתג ההגירה שהחליף בין מסרים ישנים לחדשים; מסלול Vote/QC הוא עתה המסלול היחיד.
- ✅ עדכון נתבי Torii, פקודות CLI וצרכני טלמטריה להשתמש ב-`/v1/sumeragi/*` JSON במקום המסגרות הישנות.
- ✅ כיסוי אינטגרציה מפעיל את `/v1/sumeragi/*` דרך צינור Vote/QC בלבד (`integration_tests/tests/sumeragi_vote_qc_commit.rs`).
- ✅ מחיקת המסגרות הישנות לאחר השגת שוויון פונקציונלי ובדיקות תאימות.

### תוכנית הסרת המסגרות
1. ✅ בדיקות soak מרובות צמתים למשך 72 שעות ב-Telemetry וב-CI; סנאפשוטים הראו תפוקת מציע יציבה ו-QC ללא רגרסיות.
2. ✅ כיסוי אינטגרציה נשען כעת רק על מסלול Vote/QC (`sumeragi_vote_qc_commit.rs`), ומבטיח קונצנזוס גם בצמתים מעורבים.
3. ✅ תיעוד ו-CLI למפעילים אינם מזכירים את המסלול הישן; כל ההכוונה מופנית לטלמטריית Vote/QC.
4. ✅ המסרים, מוני הטלמטריה ומטמוני ה-commit הישנים הוסרו; טבלת התאימות מציגה Vote/QC בלבד.

### A3 — אכיפת מנוע ופייסמייקר
- ✅ חלות אינווריאנטים של Highest/Locked QC בתוך `handle_message` (ראו `block_created_header_sanity`).
- ✅ שער זמינות נתונים מאמת hash של מטען RBC לפני הצבעה (`ensure_block_matches_rbc_payload`); הצבעות זמינות נשלחות רק לאחר DELIVER תואם.
- ✅ שילוב הדרישה ל-PrecommitQC (`require_precommit_qc`) בקונפיגורציית ברירת המחדל והוספת בדיקות שליליות (ברירת מחדל `true`; הבדיקות מכסות גם opt-out).
- ✅ החלפת היוריסטיקות של redundant-send ב-controller הנשען על EMA (`aggregator_retry_deadline` נסמך על EMA חי).
- ✅ עצירת הרכבת הצעות תחת לחץ תורים (`BackpressureGate` עוצר את הפייסמייקר כאשר התור רווי ומדווח על דחיות).
- ✅ קליטת BlockCreated רושמת hints/proposals, אוכפת התאמת כותרת/מטען ומשלבת שער DA בבלוקים ממתינים כך ש-RBC DELIVER הוא תנאי לנתיב הצבעת זמינות (`crates/iroha_core/src/sumeragi/main_loop.rs:3004`, `:1932`; כיסוי רגרסיה: `:5028`, `:5305`).
- ✅ כיסוי live/restart כולל התאוששות RBC לאחר cold start (`integration_tests/tests/sumeragi_da.rs::sumeragi_rbc_session_recovers_after_cold_restart`) וחזרת הפייסמייקר לאחר downtime (`integration_tests/tests/sumeragi_npos_liveness.rs::npos_pacemaker_resumes_after_downtime`).
- ✅ בדיקות ריסטארט/שינוי תצוגה דטרמיניסטיות המכסות התכנסות נעילה (`integration_tests/tests/sumeragi_lock_convergence.rs`).

### A4 — צינור אוספים ואקראיות
- ✅ עזרי רוטציה דטרמיניסטיים לאוספים חיים ב-`collectors.rs`.
- ✅ GA-A4.1 — בחירת אוסף מבוססת PRF מקליטה seed וגובה/תצוגה ב-`/status` ובטלמטריה; hooks של VRF מפיצים הקשר לאחר קומיטים וריביל. בעלי משימה: `@sumeragi-core`. מעקב: `project_tracker/npos_sumeragi_phase_a.md` (נסגר).
- ✅ GA-A4.2 — חשיפת טלמטריית השתתפות בריביל ופוקודות CLI, עדכון מניפסטים של Norito. בעלי משימה: `@telemetry-ops`, `@torii-sdk`. מעקב: `project_tracker/npos_sumeragi_phase_a.md:6`.
- ✅ GA-A4.3 — בדיקות התאוששות ריביל מאוחר ואפוק ללא השתתפות ב-`integration_tests/tests/sumeragi_randomness.rs` (למשל `npos_late_vrf_reveal_clears_penalty_and_preserves_seed`, `npos_zero_participation_epoch_reports_full_no_participation`). מעקב: `project_tracker/npos_sumeragi_phase_a.md:7`.

### A5 — ריקונפיגורציה משותפת וראיות
- ✅ תשתית ראיות, התמדה ב-WSV ו-roundtrip של Norito מכסים מצבי double-vote, הצעה/ QC שגויים ו-double exec, כולל דה-דופ וחיתוך אופק דטרמיניסטיים (`sumeragi::evidence`).
- ✅ GA-A5.1 — הפעלת קונצנזוס משותף (הקבוצה הישנה חותמת, החדשה נכנסת בבלוק הבא) מאכפת בתוספת בדיקות אינטגרציה.
- ✅ GA-A5.2 — עדכון תיעוד ומסלולי CLI לענישה/כליאה, כולל בדיקות mdBook שמכילות את ברירות המחדל ואת ניסוח אופק הראיות.
- ✅ GA-A5.3 — בדיקות שליליות (חתימה כפולה, זיוף חתימה, הפעלת אפוק ישן, מטעני מניפסט מעורבים) ו-fixtures fuzz נכנסו להרצות לילה שמגנות על ולידציית ה-roundtrip.

### A6 — טולרינג, תיעוד ואימות
- ✅ טלמטריה/דיווח RBC זמינים; דו״ח DA מייצר מדדים אמיתיים (כולל מוני פינוי).
- ✅ GA-A6.1 — בדיקת happy-path של 4 פירים NPoS עם VRF רצה ב-CI עם ספים של פייסמייקר/RBC (`integration_tests/tests/sumeragi_npos_happy_path.rs`). בעלי משימה: `@qa-consensus`, `@telemetry-ops`. מעקב: `project_tracker/npos_sumeragi_phase_a.md:11`.
- ✅ GA-A6.2 — קו בסיס ביצועים ל-NPoS (בלוקים של שנייה, k=3) תועד ב-`status.md`/מדריכי מפעיל, בצירוף seeds ופרטי חומרה לשחזור. דו״ח: `docs/source/generated/sumeragi_baseline_report.md`. מעקב: `project_tracker/npos_sumeragi_phase_a.md:12`. הריצה הוקלטה על Apple M2 Ultra (24 ליבות, 192 GB RAM, macOS 15.0) עם הפקודה שב-`scripts/run_sumeragi_baseline.py`.
- ✅ GA-A6.3 — מדריכי תפעול לטלמטריה של RBC/פייסמייקר/לחץ תורים נחתו (`docs/source/telemetry.md:523`), עם אוטומציית ניתוח לוגים כמשימת follow-up. בעלי משימה: `@operator-docs`, `@telemetry-ops`. מעקב: `project_tracker/npos_sumeragi_phase_a.md:13`.
- ✅ תרחישי ביצועים לרשת אחסון RBC/אובדן צ׳אנקים (`npos_rbc_store_backpressure_records_metrics`, `npos_rbc_chunk_loss_fault_reports_backlog`), כיסוי לשליחה עודפת (`npos_redundant_send_retries_update_metrics`) וחסר ג׳יטר חסום (`npos_pacemaker_jitter_within_band`) מבטיחים שה-A6 suite בוחן דחיות soft-limit, הפלות דטרמיניסטיות, טלמטריית שליחה עודפת וג׳יטר פייסמייקר תחת עומס.【integration_tests/tests/sumeragi_npos_performance.rs:633】【:760】【:800】【:639】

### צעדים מידיים
1. ✅ harness ג׳יטר חסום מודד טלמטריית פייסמייקר (`integration_tests/tests/sumeragi_npos_performance.rs::npos_pacemaker_jitter_within_band`).
2. ✅ קידום בדיקות דחיית RBC ב-`npos_queue_backpressure_triggers_metrics` באמצעות עומס דטרמיניסטי על חנות RBC (`integration_tests/tests/sumeragi_npos_performance.rs::npos_queue_backpressure_triggers_metrics`).
3. ✅ הרחבת soak של `/v1/sumeragi/telemetry` לכיסוי אפוקים ארוכים ואוספים אדוורסריים, והשוואת הסנאפשוטים למוני Prometheus (ראו `integration_tests/tests/sumeragi_telemetry.rs::npos_telemetry_soak_matches_metrics_under_adversarial_collectors`).

ניהול הרשימה כאן מאפשר ל-`roadmap.md` להישאר ממוקד באבני דרך תוך שמירה על צ׳ק-ליסט חי. עדכנו וחשבו השלמות עם נחיתת הפאצ׳ים.

</div>

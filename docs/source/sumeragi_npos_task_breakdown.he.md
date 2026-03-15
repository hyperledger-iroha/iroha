---
lang: he
direction: rtl
source: docs/source/sumeragi_npos_task_breakdown.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a1773b8fda6cda00e38b333096bfe5d6f6181c883ece5a62c11a190a09870d29
source_last_modified: "2025-12-12T12:49:53.638997+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- התרגום העברי ל docs/source/sumeragi_npos_task_breakdown.md -->

## פירוק משימות Sumeragi + NPoS

מסמך זה מרחיב את מפת הדרכים של שלב A למשימות הנדסיות קטנות כדי שנוכל להשלים בהדרגה את עבודת Sumeragi/NPoS שנותרה. סימוני הסטטוס: `✅` הושלם, `⚙️` בתהליך, `⬜` לא התחיל, ו-`🧪` דורש בדיקות.

### A2 - אימוץ מסרים ברמת wire
- ✅ חשיפת טיפוסי Norito `Proposal`/`Vote`/`Qc` ב-`BlockMessage` והרצת round-trip encode/decode (`crates/iroha_data_model/tests/consensus_roundtrip.rs`).
- ✅ חסימת המסגרות הישנות `BlockSigned/BlockCommitted`; מתג ההגירה נשאר `false` לפני ההוצאה משימוש.
- ✅ הסרת מתג ההגירה שהחליף בין מסרי הבלוקים הישנים; מסלול Vote/commit certificate הוא כעת המסלול היחיד על wire.
- ✅ עדכון נתבי Torii, פקודות CLI וצרכני הטלמטריה להעדיף snapshots JSON של `/v1/sumeragi/*` על פני מסגרות הבלוק הישנות.
- ✅ כיסוי אינטגרציה מפעיל את נקודות הקצה `/v1/sumeragi/*` אך ורק דרך צינור Vote/commit certificate (`integration_tests/tests/sumeragi_vote_qc_commit.rs`).
- ✅ הסרת המסגרות הישנות לאחר השגת שוויון יכולות ובדיקות תאימות.

### תוכנית הסרת מסגרות
1. ✅ בדיקות soak מרובות צמתים רצו 72 h על חבילות telemetria ו-CI; snapshots של Torii הראו תפוקת proposer יציבה והיווצרות commit certificate ללא רגרסיות.
2. ✅ כיסוי בדיקות אינטגרציה רץ כעת רק על נתיב Vote/commit certificate (`sumeragi_vote_qc_commit.rs`), ומבטיח שעמיתים מעורבים מגיעים לקונצנזוס ללא המסגרות הישנות.
3. ✅ תיעוד למפעילים ועזרת CLI כבר לא מזכירים את נתיב ה-wire הקודם; ההנחיות לאיתור תקלות מצביעות כעת על telemetria של Vote/commit certificate.
4. ✅ וריאנטים ישנים של מסרים, מוני telemetria ומטמוני commit ממתינים הוסרו; מטריצת התאימות משקפת כעת רק את פני השטח Vote/commit certificate.

### A3 - אכיפת מנוע ו-pacemaker
- ✅ אכיפת אינווריאנטים של Lock/Highestcommit certificate ב-`handle_message` (ראו `block_created_header_sanity`).
- ✅ מעקב זמינות נתונים מאמת את hash המטען של RBC בעת רישום המסירה (`Actor::ensure_block_matches_rbc_payload`) כך שסשנים לא תואמים לא ייחשבו כמסורים.
- ✅ שילוב דרישת Precommitcommit certificate (`require_precommit_qc`) בקונפיגורציות ברירת המחדל והוספת בדיקות שליליות (ברירת המחדל כעת `true`; הבדיקות מכסות מסלולי gated ו-opt-out).
- ✅ החלפת heuristics של redundant-send ברמת view בבקרי pacemaker מבוססי EMA (`aggregator_retry_deadline` נגזר כעת מ-EMA חי ומניע את דד-ליינים של redundant send).
- ✅ עצירת הרכבת הצעות תחת backpressure של התור (`BackpressureGate` עוצר כעת את ה-pacemaker כשהתור רווי ורושם deferrals ל-status/telemetry).
- ✅ הצבעות availability נשלחות לאחר אימות ההצעה כאשר DA נדרש (ללא המתנה ל-`DELIVER` מקומי של RBC), ו-evidence של availability נעקב דרך `availability evidence` כהוכחת בטיחות בזמן שה-commit מתקדם ללא המתנה. זה מונע המתנות מעגליות בין הובלת payload להצבעה.
- ✅ כיסוי restart/liveness כולל כעת התאוששות RBC ב-cold-start (`integration_tests/tests/sumeragi_da.rs::sumeragi_rbc_session_recovers_after_cold_restart`) וחידוש pacemaker לאחר downtime (`integration_tests/tests/sumeragi_npos_liveness.rs::npos_pacemaker_resumes_after_downtime`).
- ✅ הוספת בדיקות רגרסיה דטרמיניסטיות ל-restart/view-change המכסות התכנסות lock (`integration_tests/tests/sumeragi_lock_convergence.rs`).

### A4 - צינור collectors ואקראיות
- ✅ עזרי רוטציה דטרמיניסטיים ל-collectors נמצאים ב-`collectors.rs`.
- ✅ GA-A4.1 - בחירת collectors מבוססת PRF כעת רושמת seeds דטרמיניסטיים ו-height/view ב-`/status` וב-telemetria; hooks לרענון VRF מפיצים את ההקשר אחרי commits ו-reveals. בעלי משימה: `@sumeragi-core`. מעקב: `project_tracker/npos_sumeragi_phase_a.md` (נסגר).
- ✅ GA-A4.2 - חשיפת telemetria להשתתפות reveal + פקודות CLI לבדיקת מצב ועדכון manifests של Norito. בעלי משימה: `@telemetry-ops`, `@torii-sdk`. מעקב: `project_tracker/npos_sumeragi_phase_a.md:6`.
- ✅ GA-A4.3 - קידוד התאוששות late-reveal ובדיקות epoch ללא השתתפות ב-`integration_tests/tests/sumeragi_randomness.rs` (`npos_late_vrf_reveal_clears_penalty_and_preserves_seed`, `npos_zero_participation_epoch_reports_full_no_participation`), תוך בדיקת telemetria לניקוי עונשים. בעלי משימה: `@sumeragi-core`. מעקב: `project_tracker/npos_sumeragi_phase_a.md:7`.

### A5 - קונפיגורציה משותפת ו-evidence
- ✅ תשתית evidence, התמדה ב-WSV ו-roundtrip של Norito מכסים כעת double-vote, invalid proposal, invalid commit certificate ו-double exec עם דה-דופ דטרמיניסטי וקיטוע אופק (`sumeragi::evidence`).
- ✅ GA-A5.1 - הפעלת joint-consensus (הסט הישן מבצע commit, הסט החדש מופעל בבלוק הבא) עם כיסוי אינטגרציה ממוקד.
- ✅ GA-A5.2 - עדכון מסמכי governance וזרימות CLI עבור slashing/jailing, יחד עם בדיקות סנכרון mdBook לנעילת ברירות מחדל וניסוח evidence horizon.
- ✅ GA-A5.3 - בדיקות evidence למסלול שלילי (duplicate signer, forged signature, stale epoch replay, mixed manifest payloads) יחד עם fixtures fuzz נכנסו ורצות nightly כדי להגן על אימות Norito roundtrip.

### A6 - טולרינג, תיעוד ואימות
- ✅ טלמטריה/דיווח RBC זמינים; דו"ח DA מייצר מדדים אמיתיים (כולל מוני eviction).
- ✅ GA-A6.1 - בדיקת happy-path של 4 פירים NPoS עם VRF רצה ב-CI עם ספי pacemaker/RBC enforced דרך `integration_tests/tests/sumeragi_npos_happy_path.rs`. בעלי משימה: `@qa-consensus`, `@telemetry-ops`. מעקב: `project_tracker/npos_sumeragi_phase_a.md:11`.
- ✅ GA-A6.2 - קו בסיס ביצועים ל-NPoS (בלוקים של 1 s, k=3) נלכד ופורסם ב-`status.md`/מסמכי מפעיל עם seeds לשחזור ומטריצת חומרה. בעלי משימה: `@performance-lab`, `@telemetry-ops`. דו"ח: `docs/source/generated/sumeragi_baseline_report.md`. מעקב: `project_tracker/npos_sumeragi_phase_a.md:12`. ריצה חיה תועדה על Apple M2 Ultra (24 cores, 192 GB RAM, macOS 15.0) לפי הפקודה שב-`scripts/run_sumeragi_baseline.py`.
- ✅ GA-A6.3 - מדריכי troubleshooting למפעילים עבור RBC/pacemaker/backpressure פורסמו (`docs/source/telemetry.md:523`); קורלציית לוגים מתבצעת כעת ע"י `scripts/sumeragi_backpressure_log_scraper.py`, כדי שמפעילים יוכלו לשלוף צמדי pacemaker deferral/missing-availability ללא grep ידני. בעלי משימה: `@operator-docs`, `@telemetry-ops`. מעקב: `project_tracker/npos_sumeragi_phase_a.md:13`.
- ✅ נוספו תרחישי ביצועים של RBC store/chunk-loss (`npos_rbc_store_backpressure_records_metrics`, `npos_rbc_chunk_loss_fault_reports_backlog`), כיסוי fan-out מיותר (`npos_redundant_send_retries_update_metrics`) ו-harness של jitter מוגבל (`npos_pacemaker_jitter_within_band`) כך שסוויטת A6 בודקת deferrals של soft-limit ב-store, drops דטרמיניסטיים של chunks, טלמטריית redundant-send ורצועות jitter של pacemaker תחת עומס. [integration_tests/tests/sumeragi_npos_performance.rs:633] [integration_tests/tests/sumeragi_npos_performance.rs:760] [integration_tests/tests/sumeragi_npos_performance.rs:800] [integration_tests/tests/sumeragi_npos_performance.rs:639]

### צעדים מידיים
1. ✅ harness של jitter מוגבל מפעיל את מדדי jitter של pacemaker (`integration_tests/tests/sumeragi_npos_performance.rs::npos_pacemaker_jitter_within_band`).
2. ✅ חיזוק assertions של deferral RBC ב-`npos_queue_backpressure_triggers_metrics` באמצעות יצירת לחץ דטרמיניסטי על store RBC (`integration_tests/tests/sumeragi_npos_performance.rs::npos_queue_backpressure_triggers_metrics`).
3. ✅ הרחבת soak של `/v1/sumeragi/telemetry` לכיסוי epochs ארוכים ו-collectors עוינים, עם השוואת snapshots למוני Prometheus לאורך מספר heights. מכוסה ע"י `integration_tests/tests/sumeragi_telemetry.rs::npos_telemetry_soak_matches_metrics_under_adversarial_collectors`.

מעקב אחרי הרשימה כאן שומר על `roadmap.md` ממוקד באבני דרך ובמקביל נותן לצוות רשימת בדיקה חיה לביצוע. עדכנו את הערכים (וסמנו השלמות) עם נחיתת התיקונים.

</div>

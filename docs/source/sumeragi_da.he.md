<!-- Hebrew translation of docs/source/sumeragi_da.md -->

---
lang: he
direction: rtl
source: docs/source/sumeragi_da.md
status: needs-update
translator: manual
---

<div dir="rtl">

# זמינות נתונים ורשת RBC ב-Sumeragi

בדיקות האינטגרציה [`sumeragi_rbc_da_large_payload_four_peers`] ו־[`sumeragi_rbc_da_large_payload_six_peers`] (בקובץ `integration_tests/tests/sumeragi_da.rs`) מקימות רשתות של ארבעה ושישה פירים עם `sumeragi.da.enabled = true` (DA + RBC). כל בדיקה שולחת הוראת לוג בגודל ≥10 MiB, צופה במסירה של RBC ובקומיט, מאמתת שניתן להרכיב קוורום זמינות למטען, ומדפיסה סיכום מובנה הניתן לצריכה בדשבורדים או בכלי רגרסיה.

לסמפול מונחה קליינט-קל של מטעני RBC ראו [`light_client_da.md`](light_client_da.md), המתעד את נקודת הקצה המאומתת `/v1/sumeragi/rbc/sample` ואת המגבלות/תקציבים הנלווים.

### טיימאאוט DA והתראות

כאשר `sumeragi.da.enabled=true`, צינור הקומיט רושם עדות זמינות (`availability evidence` או קוורום RBC `READY`) ועוקב אחריה כ‑advisory (הקומיט אינו ממתין). payload מקומי חסר מתקבל דרך RBC `DELIVER` או BlockCreated/סנכרון בלוקים. חוסר זמינות נרשם לצרכים תפעוליים. `sumeragi_da_gate_block_total{reason="missing_local_data"}` גדל, ו-`status_snapshot().da_reschedule_total` הוא legacy ולכן בדרך כלל נשאר 0.

טיימאאוט הזמינות נגזר מזמני block/commit ומכווני ה-DA, ומשמש רק ללוגים ולהחלטות rebroadcast:
- `sumeragi.advanced.da.quorum_timeout_multiplier` מקדם את `block_time + 3 * commit_time` כאשר DA פעיל (ברירת מחדל `3`).
- `sumeragi.advanced.da.availability_timeout_multiplier` מקדם את חלון טיימאאוט הזמינות במצב DA (ברירת מחדל `2`).
- `sumeragi.advanced.da.availability_timeout_floor_ms` כופה מינימום לחלון הטיימאאוט (ברירת מחדל `2000`, ערך `0` מבטל את הרצפה).
שמרו על ערכים זהים בין הוולידטורים כדי למנוע קצב view-change שונה.

מנגנון resend/abort אוטומטי של RBC שמבוסס על מעקב זמינות הוסר כדי למנוע המתנה מעגלית בין מסירה להצבעה. צומת שרואה `availability evidence` או קוורום RBC `READY` בלי המטען מבצע fetch דטרמיניסטי מהחותמים, ואז נופל לטופולוגיית הקומיט המלאה אם אין הצלחה.

## מדדים שנאספים

- גודל המטען (בייט) והספחת הסקת קצב (MiB/s) ברגע שה-RBC מסמן שהמטען נשלח.
- צילום מצב של סשן RBC (`total_chunks`, ‏`received_chunks`, ‏`ready_count`, ‏`view`, ‏`block_hash`, ‏`recovered`, ‏`lane_backlog`, ‏`dataspace_backlog`) הנשלף מ-`/v1/sumeragi/rbc/sessions`.
- מוני Prometheus לכל peer: ‏`sumeragi_rbc_payload_bytes_delivered_total`, ‏`sumeragi_rbc_deliver_broadcasts_total`, ‏`sumeragi_rbc_ready_broadcasts_total` מתוך `/metrics`.
- מדדי עומס לפי נתיב/מרחב-נתונים שניתן לדגום מ-`/metrics`:
  ‏`sumeragi_rbc_lane_{tx_count,total_chunks,pending_chunks,bytes_total}` עם תווית `lane_id`
  ו-`sumeragi_rbc_dataspace_{tx_count,total_chunks,pending_chunks,bytes_total}` עם תוויות `lane_id`/`dataspace_id`.

## הרצת התרחיש

יש להפעיל טלמטריה כדי שהעזר יוכל לשלוף נתונים מ-`/metrics`. הריצו `scripts/check_norito_bindings_sync.sh` (או `python3 scripts/check_norito_bindings_sync.py`) כדי לוודא שסביבות ה-Norito מסונכרנות; אם לא, תסריט הבנייה יעצור עד לרענון.

```bash
cargo test -p integration_tests \
  sumeragi_rbc_da_large_payload_four_peers -- --nocapture

cargo test -p integration_tests \
  sumeragi_rbc_da_large_payload_six_peers -- --nocapture
```

כל ריצה מדפיסה שורות עם הקידומת `sumeragi_da_summary::<scenario>::{...}` כך שאוטומציה תוכל ללכוד את ה-JSON. ניתן להגדיר `SUMERAGI_DA_ARTIFACT_DIR=/path/to/dir` כדי לשמור את הסיכומים והסנאפשוטים של Prometheus לכל peer. הסקריפט `scripts/run_sumeragi_da.py` מפעיל אוטומטית את ההגדרה בהרצות הלילה ומפיק `sumeragi-da-report.md` באמצעות `cargo run -p build-support --bin sumeragi_da_report`. תהליך הלילה `.github/workflows/sumeragi-da-nightly.yml` מעלה את כל תיקיית הריצה (סיכומים, מדדים, דו״ח Markdown) ל-GitHub Actions כדי שהמפעילים יוכלו לעיין בתוצאות.

התרחישים מפעילים `sumeragi.debug.rbc.force_deliver_quorum_one = true` כך שה‑DELIVER יוצא לאחר READY ראשון. בסביבת ייצור יש להשאיר זאת כבוי כדי לשמור על קוורום READY מלא (2f+1).

## קווי בסיס צפויים

עם `sumeragi.advanced.rbc.chunk_max_bytes = 256 KiB`, הוראה בגודל 10.5 MiB (11 010 048 בייט), ובהנחה ש‑`force_deliver_quorum_one` פעיל, מתקיימים התנאים הבאים:

שים לב: `sumeragi.advanced.rbc.chunk_max_bytes` נחתך בזמן ההפעלה כך ש־`RbcChunk` סריאלי ייכנס לתקרת ה‑plaintext שמתקבלת מ־`network.max_frame_bytes_block_sync` לאחר ניכוי תקורת ההצפנה.

| תרחיש | מספר צ׳אנקים | סף READY | מונים לכל peer | תקציבי זמן |
| --- | --- | --- | --- | --- |
| ארבעה פירים | 42 צ׳אנקים (כולם נדרשים) | READY ≥1 (דיבוג כפוי; רגיל ≥3 עבור ‎f=1) | `payload_bytes_delivered_total ≥ 11 010 048`, ‏`deliver_broadcasts_total = 1`, ‏`ready_broadcasts_total = 1` | `commit_ms` ו-`rbc_deliver_ms` צריכים להישאר בתוך `commit_time_ms` (ברירת מחדל `4000`) |
| שישה פירים | 42 צ׳אנקים | READY ≥1 (דיבוג כפוי; רגיל ≥4 עבור ‎f=2) | זהה לעיל | זהה לעיל |

השארה במקטע הקומיט של 4 שניות מחייבת קצב אפקטיבי של ≥≈2.7 MiB/s. מומלץ להתריע כשזמן ה-DELIVER מתקרב ל-`commit_time_ms`, כאשר הקצב יורד מתחת לסף או כאשר המונים לכל peer מתפצלים (אות לכפיית יתר או צ׳אנקים חסרים).

`cargo run -p build-support --bin sumeragi_da_report [ARTIFACT_DIR]` קורא את `.summary.json` ומפיק דו״ח Markdown עם זמני השהיה, קצב ותמונות מצב לכל ריצה. יש להעביר את נתיב הארטיפקטים כארגומנט או להגדיר `SUMERAGI_DA_ARTIFACT_DIR`. דו״ח הפיקסצ׳ר מ-2025‑10‑05 מציג מדיונים בין ‎3.12s–3.34s לקונסולות RBC, קומיט בתוך ‎4s וקצב ≥3.1 MiB/s.

הערה לסביבת Sandboxed: `scripts/run_sumeragi_da.py` מגדיר `IROHA_SKIP_BIND_CHECKS=1` לפני שהפירים עולים, ומוסיף פיקסצ׳ר (`integration_tests/fixtures/sumeragi_da/default/`). ב-macOS seatbelt מופיעות שגיאות הרשאות בשלב bind המקדמי; המשתנה מאפשר לנסות_bind אמיתי. אם ההרשאות עדיין מונעות חיבורים, הפיקסצ׳ר מנוגן כדי להמשיך לספק נתונים. ניתן לבטל את fallback באמצעות `--disable-fixture-fallback`.

## תקציבי ביצועים

בדיקות ההעמסה כעת אוכפות תקציבים מפורשים בעת הפעלת DA/RBC/SBV‑AM. אותם ערכים מופיעים בסיכום ובדוח Markdown (`BG queue max`, ‏`P2P drops max`). מומלץ להתריע על חריגה:

| מדד | תקציב | אכיפה | הנחיית התרעה |
| --- | --- | --- | --- |
| Latency של RBC (מטען 10.5 MiB) | ≤ 3.6 s | `sumeragi_rbc_da_large_payload_*` | התרעה ב־≥ 3.2 s; בדקו רוויה של אספנים |
| Latency של קומיט | ≤ 4.0 s | אותו תרחיש | התרעה ב־≥ 3.6 s; בדקו דדליין של pacemaker ושינויי תצוגה |
| קצב אפקטיבי | ≥ 2.7 MiB/s | אותו תרחיש | התרעה כשקצב < 3 MiB/s שתי ריצות ברצף |
| עומק תור Post ברקע | ≤ 32 משימות | אותו תרחיש | התרעה ב־≥ 24; מצביע על עומס מצטבר |
| נפילות בתורי P2P | = 0 | אותו תרחיש | התרעה מיידית על ערך > 0; בדיקת קיבולות התורים |

CI הלילי צורך את הסיכומים ומפיק את הדו״ח כך שניתן לעקוב אחר עמידה בתקציבים לאורך זמן.

.. mdinclude:: generated/sumeragi_da_report.md

## תרחישים אדוורסריים

הקובץ `integration_tests/tests/sumeragi_adversarial.rs` מפעיל את כפתורי ה-RBC לבדיקות כאוס:

- `sumeragi_adversarial_chunk_drop`: ‏`sumeragi.debug.rbc.drop_every_nth_chunk = 2` מאמת שעצירה של כל צ׳אנק שני אצל המנהיג מונעת קומיט. הסיכום מודפס כ-`sumeragi_adversarial::chunk_drop::{...}`.
- `sumeragi_adversarial_chunk_reorder`: ‏`sumeragi.debug.rbc.shuffle_chunks = true` מראה שסידור מחדש לא פוגע ביעדי DELIVER/commit.
- `sumeragi_adversarial_witness_corruption`: ‏`sumeragi.debug.rbc.corrupt_witness_ack = true` בודק ש-ack פגום חוסם קומיט אך סשן ה-RBC מושלם.
- `sumeragi_adversarial_duplicate_inits`: ‏`sumeragi.debug.rbc.duplicate_inits = true` מוודא שמטען כפול בתצוגה הבאה נמסר ונרשם בדו״ח.
- `sumeragi_adversarial_chunk_drop_recovery`: מריץ שלב עם `drop_every_nth_chunk` כדי לוודא תקיעה, ואז מפעיל מחדש ללא הכפתור כדי לוודא שחזרה להתנהגות הוגנת מחזירה קומיטים.

לכל התרחישים ניתן להגדיר `SUMERAGI_ADVERSARIAL_ARTIFACT_DIR` כדי לשמור את סיכומי ה-JSON, בדומה לתרחיש המטען הגדול.

</div>

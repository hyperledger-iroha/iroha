<!-- Hebrew translation of docs/source/sumeragi.md -->

---
lang: he
direction: rtl
source: docs/source/sumeragi.md
status: needs-update
translator: manual
---

<div dir="rtl">

## Sumeragi: מימוש נוכחי (v1)

למבט מפורט על משימות ההגירה שנותרו ראו את
[`sumeragi_npos_task_breakdown.md`](sumeragi_npos_task_breakdown.md).

### סקירה
- **תפקידים ורוטציה** – הטופולוגיה המסודרת מחלקת את העמיתים לתפקידים `Leader`, ‏`ValidatingPeer`, ‏`ProxyTail`, ‏`SetBValidator`. לאחר כל קומיט הקבוצה הראשונה (`min_votes_for_commit()`) זזה שמאלה לפי `hash(prev_block_hash) mod min_votes_for_commit()`; שינויי View מסובבים את כל הטופולוגיה כדי לקדם את הלידר. הפונקציה `rotated_for_prev_block_hash(prev_hash)` ב-`network_topology.rs` מספקת רוטציה דטרמיניסטית קלה לביקורת על בסיס האש הבלוק הקודם.
- **מצב K-Collectors** – לכל גובה הטופולוגיה בוחרת דטרמיניסטית K אספנים כקטע רציף שמתחיל ב-`proxy_tail_index()` (כולל) ללא עטיפה; הלידר לעולם אינו נבחר. כאשר K=1 מתקבלת שוב תצורת הפרוקסי-טייל ההיסטורית.
- **First-commit-certificate-wins** – כל אספן ייעודי (כולל Proxy Tail) שמרכיב commit certificate תקף ל-Precommit מפרסם אותו. עמיתים מאמצים את ה-commit certificate התקף הראשון עבור `(height,hash)` ומתעלמים מאיחורים; הקומיט מבוסס מעתה כולו על commit certificates אלו.

### תפקידי נוד (קונפיגורציה)
- `validator` (ברירת מחדל): משתתף בקונצנזוס לפי התפקיד בטופולוגיה הנוכחית.
- `observer`: מוצא מהטופולוגיה (התפקיד הופך `Undefined`), אינו מציע/מצביע/אוסף גם אם משבצת המקור הייתה אספן. מסתנכרן בקאנונית באמצעות Gossip ויכול לקמוט באמצעות commit certificates שהתקבלו.

### דרישות מפתח ולידטור
- ולידטורים חייבים להשתמש במפתחות BLS-Normal ולהציג הוכחת החזקה (PoP). ב‑NPoS מסננים את רשימת ההסכמה לפי הוולידטורים הפעילים במסלול הציבורי מהסטייקינג (כולל לאחר עדכוני commit topology); כאשר אין ולידטורים פעילים, נופלים חזרה ל‑`trusted_peers` לאחר סינון עמיתים ללא BLS-Normal או עם PoP חסר/שגוי.
- commit certificate נושאים חתימת BLS מצטברת על מסר אחיד, מערך חתימות מפורש ומפת חתימות קומפקטית. האימות נשען על החתימות המפורשות; הצבירה והביטמאפ הם ארטיפקטים לאודיט ואסור שישנו סמנטיקה.

### זרימת הודעות (מצב יציב)
- **לידר**: בעת צפייה לבלוק (טרנזקציות בתור) והדד-ליין מתקרב – משדר `BlockCreated`. כאשר התור ריק הפייסמייקר נשאר idle ואינו שולח הצעות heartbeat.
- **דחיית הצעות**: הרכבת הצעות נדחית כאשר יש backpressure בריליי, כאשר קיימות סשנים של RBC שטרם הושלמו עבור ה-tip הפעיל, או כאשר יש בלוקים ממתינים שמאריכים את ה-tip; הפייסמייקר ממתין עד שהעומס מתפנה לפני שמציע שוב.
- **ולידטורים**: מאמתים, מפיקים Availability votes ושולחים Prevote/Precommit לאספנים המיועדים. בטיימאאוט לוקאלי ב-View 0 ניתן לפנות עד `r` אספנים נוספים.
- **אספנים**: מרכזים הצבעות ומפרסמים Availability/Prevote/Precommit commit certificate. ה-commit certificate התקף הראשון עבור `(height,hash)` מנצח; איחורים נדחים.
- **עמיתים מתבוננים**: אינם מצביעים ב-View 0. ב-Views מאוחרים עשויים להצביע; כאשר גוף הבלוק ו-commit certificate תואם זמינים – מבצעים קומיט דטרמיניסטי.
- **מקבלים**: בודקים commit certificate, מעדכנים מעקב Highest/Locked commit certificate ומנסים קומיט כאשר ה-commit certificate והבלוק תואמים.

### כלל הקומיט (פייפליין של שני commit certificate)
- כל ולידטור עוקב אחר `highest_qc` (ה-commit certificate האחרון שנצפה, לרוב לגובה הילד) ואחר `locked_qc` (שומר הבטיחות הנוכחי). הצעה נבנית רק אם `highest_qc` מרחיב את `locked_qc`; אחרת היא מושלכת גם אם קיים רוב NEW_VIEW.
- בלוק בגובה `h` מצטבר כאשר מתקבל commit certificate לגובה `h+1` שהורה שלו תואם ל-commit certificate הנעול בגובה `h`. התנאי הדו-עומקי שומר על הפייפליין ומונע משרשרת מתנגשת להשיג קומיט.
- כך אין צורך בהודעת סופיות נוספת: כאשר commit certificate הילד מגיע הבלוק האב בטוח, וכל נוד המחזיק את הבלוק ושני ה-commit certificate מסמן קומיט. מי שפספס את ה-commit certificate הראשון משלים דרך Gossip/RBC ומגיע לאותה החלטה.
- מכיוון שרק שני גבהים פתוחים בו-זמנית, אספנים, RBC וטלמטריה יכולים לצנר עבודה ללא סכנת קומיטים מתפצלים. האינווריאנטים מיושמים ב-`sumeragi::main_loop`: ‏`ensure_locked_qc_allows` מגן על הרכבת הצעות, בלוקים ממתינים נשמרים בזיכרון והקומיט משחרר `locked_qc` רק לאחר תצפית ב-commit certificate ילד תואם.

### פייסמייקר (שינויי View)
- ב-v1 בוטל מרווח ההצבעה של Observers ב-View 0: עמיתי צפייה אינם מצביעים כלל ב-View 0. בטיימאאוט מקומי הם מבקשים שינוי View ללא הרחבה לפני הרוטציה. התזמונים נשלטים על ידי פרמטרים on-chain: ב-permissioned משתמשים ב-`SumeragiParameters` (`BlockTimeMs`,‏ `CommitTimeMs`), וב‑NPoS ב-`sumeragi_npos_parameters` (`block_time_ms`, `timeouts.timeout_commit_ms`) – הצעת לידר סביב שליש זמן הצינור וקומיט צפוי סביב שני שלישים.

### פרמטרים K / r
- בקונפיג: ‏`sumeragi.collectors_k: usize` (מספר אספנים לגובה, ברירת מחדל 1) ו-`sumeragi.collectors_redundant_send_r: u8` (פיזור שליחות עודפות, ברירת מחדל 1).
- בשרשרת: K ו-r נמצאים ב-`SumeragiParameters` והם המקור הקובע לתכנון הקולקטורים ולפרסום `ConsensusParams`; ערכי הקונפיג מזינים את ברירות המחדל ב-genesis. כאשר פירים מפרסמים K/r שונים, נרשם mismatch אך נשמרים הערכים מהשרשרת.
- פולבק: אם `k` לא מחזיר קולקטורים, ההצבעות נופלות לטופולוגיית ה-commit המלאה; `redundant_send_r` מטופל לפחות כ-1.

### עקרונות טופולוגיה
- טופולוגיה נוצרת רק עבור ולידטורים בעלי PoP ב-BLS. הפונקציות `leader_index()` ו-`proxy_tail_index()` אינן משתמשות בשאריות ומחזירות `TopologySegment` דטרמיניסטי לביקורת.
- `Topology::collectors_for(height)` מחזירה `CollectingPeerSet` נקי וקבוע לכל גובה. כאשר K=1 מתקבל Proxy Tail יחיד – תואם למסלול ההיסטורי.
- `Topology::role_at(index)` מספק את תפקיד העמית, ו-`is_validator(idx)` מאשר חברות לגובה הנוכחי.

### ממשק API
- `iroha_cli sumeragi status --summary` / `GET /v1/sumeragi/status/summary` מציגים טופולוגיה, חלוקת תפקידים, מצב commit certificate, שימוש ב-RBC ואינפורמציה על האפוק (`epoch_len`,‏ `epoch_commit`,‏ `epoch_reveal`).
- `iroha_cli sumeragi params --summary` / `GET /v1/sumeragi/params` מחזירים את פרמטרי הקונצנזוס: ‏`collectors_k`,‏ `collectors_redundant_send_r`, טיימרים, מוד.
- `iroha_cli sumeragi status` / `GET /v1/sumeragi/status` מספקים רשומת Norito מלאה עם היסטוריית תפקידים, View עכשווי, Locked/Highest commit certificate ובלוקים ממתינים.

### עיקרי המימוש
- `sumeragi::main_loop` הוא הלולאה המרכזית שמנהלת עדכוני commit certificate, טיפוס הצעות וקומיט.
- אחסון commit certificate מנוהל באמצעות מבני `HighestQc` ו-`LockedQc` שמפיקים לוגים מפורטים ואירועי Evidence בעת אי-התאמה.
- RBC פועל באופן אסינכרוני; עם הגעת גוף הבלוק וה-commit certificate נבחנת האפשרות לקמוט מידית.
- הפייסמייקר שולט בדופק ובשינוי View בהתאם לטיימרים ומרחיב View רק לאחר עמידה בסף.
- טלמטריה מפרסמת את מדדי `sumeragi_*` ל-Prometheus, כולל עומסי תורים, ראיות ונתוני RBC.

### RBC / DA (זמינות נתונים)
- RBC משתמש בקבוצת האספנים כדי להפיץ את גוף הבלוק. כותרת הבלוק מכילה מזהה סשן ומטא-נתונים.
- `sumeragi.da_enabled = true` עוקב אחרי עדות זמינות (`availability evidence`) אבל לא מעכב קומיט (ל־RBC `DELIVER` מקומי אין תנאי). כאשר עדות זמינות חסרה, `sumeragi_da_gate_block_total{reason="missing_local_data"}` גדל ו־`da_reschedule_total` הוא legacy ולכן בדרך כלל נשאר 0.
- כאשר READY/DELIVER או צ׳אנקים מגיעים לפני INIT, הנוד שומר אותם ב־stash ומבקש מיד את `BlockCreated` החסר (בכפוף ל־backoff של missing‑block).
- `sumeragi.rbc_rebroadcast_sessions_per_tick` מגביל את מספר סשני ה־RBC שנשלחים מחדש בכל tick כדי למנוע סופות שידור כשיש backlog. להאצת התאוששות מעלים את הערך, ובמקרה של עומס תורים מורידים.
- תרחישים של ≥10 MiB נבחנים בסוויטת האינטגרציה ומודדים זמני RBC, קומיט, סף סינון תורים וזרימות P2P. הפרת SLO נכשלת במבחן ומפעילה התרעה.

### דוגמאות CLI לטופולוגיה ותפקידים
- `iroha_cli sumeragi topology --summary` מציג את הרשימה הסדורה והתפקידים.
- `iroha_cli sumeragi collectors --height <h>` מחזיר את אספני הגובה `h`.
- `iroha_cli sumeragi roles --peer <account>` מדווח על התפקיד הנוכחי של עמית מסוים.

### ראיות ו-Slashing
- ראיות הכפלה (`double_vote`, ‏`double_propose`), commit certificate/הצעה לא תקינים ואי-פעילות נשמרות כ-`Evidence`. הן מנוהלות ב-WSV עם אופק גיזום, מופצות ב-Prometheus ומחושבות עבור CLI.
- טרנזקציית Evidence עוברת `validate_evidence`, ואם היא תקפה – הסנקציה מיושמת והמדווח מתוגמל.

### ממשל והחלפת מצבים
- `ConsensusMode` מאפשר בחירה בין מסלול מרושת למסלול NPoS (כאשר זה האחרון זמין).
- שינוי מצב מתבצע דרך `SetSumeragiMode`, מצריך בחירת גובה עתידי בטוח ומאומת דרך `/v1/sumeragi/params`.
- מעבר אופרטיבי: לברר את הגובה, לשדר בקשת שינוי, לעקוב אחר התחייבות ולוודא שהמצב החדש מופעל בגובה שנקבע.

### דפוסי הפעלה למודים
- מסלול Permissioned: סינגל-צינור עם פרוקסי-טייל יחיד, Viewchange קלאסי וקומיט מבוסס `BlockCommitted`.

### CLI / API צ'יטשיט
- `iroha_cli sumeragi params --summary` – בדיקת טיימרים ופרמטרים.
- `iroha_cli sumeragi status --summary` – צילום מצב קצר עם locked/highest view, תור RBC, תור פייסמייקר.
- `iroha_cli sumeragi status` – פלט Norito מלא.
- `GET /v1/sumeragi/status/summary` – גרסת JSON/ Norito.
- `GET /v1/sumeragi/status` – נתוני מצב מלאים כולל בלוקים ממתינים ואינדיקטורים לא evidence.
- `GET /v1/sumeragi/params` – פרמטרים פעילים.

### מדדי טלמטריה
- Counters: ‏`sumeragi_view_change_total`, ‏`sumeragi_backpressure_deferrals_total`, ‏`sumeragi_da_gate_block_total{reason="missing_local_data"}`, ‏`sumeragi_evidence_total`.
- Histograms/Gauges: ‏`sumeragi_collect_witness_ms`, ‏`sumeragi_da_queue_depth`, ‏`sumeragi_pending_blocks`, ‏`sumeragi_locked_qc_height`.
- לוחות מחוונים: טבלאות אספנים לפי קצב הצבעות, מדדי RBC, פייסמייקר EMA, חלונות VRF.

### כיסוי בדיקות
- בדיקות יחידה: כיסוי encode/decode לטיפוסי Norito, אינווריאנטים של טופולוגיה, ניהול commit certificate.
- אינטגרציה: `sumeragi_vote_qc_commit.rs`, ‏`sumeragi_da.rs`, ‏`sumeragi_randomness.rs`, ‏`sumeragi_lock_convergence.rs`, ‏`sumeragi_npos_performance.rs`, ‏`sumeragi_telemetry.rs`.
- CI: הפעלה nightly של בדיקות ביצועים, RBC, ראיות.

### טופולוגיה
- `Topology` שומר את העמיתים בסדר קנוני ומספק עזרי `leader`,‏ `proxy_tail`,‏ `validators`,‏ `observers`.
- ניתן לבנות טופולוגיה מחדש מתוך `SumeragiParameters` וסט העמיתים הקיים כדי לשחזר תצוגות.

### בחירת אספנים
- `collectors_for(height)` מחזיר `CollectingPeerSet` לפי גובה.
- `collectors.extend_for_redundancy(r)` מוסיף שליחות עודפות כאשר `r` > 0.
- התוצאה נצרבת בטלמטריה ועוזרת לאבחון אספנים תקועים.

### ניהול View / Epoch
- `Pacemaker` אחראי על איסוף Viewchange והתקנות view חדש.
- `Epoch` מסמן שינויי ממשל או טופולוגיה; `FetchedTopology` מחזיר את הסנאפשוט המתאים.

### Highest / Locked commit certificate
- `HighestQc` ו-`LockedQc` מנוהלים באמצעות מבנים ברורים. `update_highest_qc` מקבל רק commit certificate מתקדם יותר. `ensure_locked_qc_allows` מוודא שהצעה מרחיבה נעילה או נושאת commit certificate גבוה יותר.

### RBC
- סשני RBC מזוהים באמצעות `RbcSessionId` ומכילים מטא-נתונים על גובה, hash, ומספר חלקים. הם מפיצים את גוף הבלוק ומספקים זמינות נתונים.
- כאשר `sumeragi.da_enabled` פעיל, קומיט תלוי ב-`availability evidence` (ולא בהשלמה מקומית של RBC). RBC משמש כמנגנון הפצה/שחזור של ה-payload.
- רוסטר ה‑INIT נשמר כסנאפשוט לא מאומת; הרוסטר הנגזר מטופולוגיית הקומיט הוא המקור הסמכותי ל‑READY/DELIVER ולאימות חתימה מקומית. INIT שסותר רוסטר נגזר נדחה.
- READY/DELIVER שמגיעים לפני אימות הרוסטר נאגרים (stash), ובקשות ל‑`BlockCreated` חסר נופלות חזרה לטופולוגיית הקומיט. לאחר אימות, אי־התאמה של `roster_hash` גוררת דחייה.
- שידורי READY חוזרים מוגבלים לתת־קבוצה דטרמיניסטית של f+1 (כולל תמיד את המנהיג) כדי לצמצם סערות הודעות.

### הגדרת `proof_policy`
- `proof_policy = "off"` (ברירת מחדל): commit QC בלבד.
- `proof_policy = "zk_parent"`: דרישה להוכחת ZK של ההורה (`zk_finality_k`).

### Telemetry & Backpressure
- `pacemaker_backpressure_deferrals_total` מורה על עיכובים עקב לחץ תורים; `scripts/sumeragi_backpressure_log_scraper.py` קושר בין לוגים למדדים.
- `sumeragi_da_summary::*` מסכם את ריצות ה-RBC ו-SLO. CI בודק את הערכים הללו.
- עקומות EMA מדווחות על טיימרים מומלצים במצב הריצה.

### צינור ראיות
1. CLI או Torii מקבלים Norito Evidence.
2. `validate_evidence` מאמתת חתימות, View וגובה.
3. ראיה תקפה מתווספת ל-WSV, מופקת טלמטריה ומופעלים סנקציות/גילוי.
4. אופק הגיזום מסיר ראיות ישנות (`evidence_horizon`).

### הערות תפעול CLI
- `iroha_cli sumeragi status --summary` מציג `<role, view, locked_qc_height, highest_qc_height, pending_blocks>` יחד עם RBC/פייסמייקר.
- `iroha_cli sumeragi params` מחזיר Norito JSON מלא של הפרמטרים, כולל מצבים, טיימרים ופרמטרי DA.
- שימוש ב-`--summary-only` מספק פלט תמציתי לחדרי בקרה.

### מדריך טלמטריה לאספנים ולעדי־ביצוע

כאשר אספן תקוע או עד ביצוע מאחר, הדבר יופיע כהעדר availability evidence, איסוף DA מתמשך או `collect_witness_ms` גבוה. להלן המדדים המרכזיים:

**לוחות מחוונים מרכזיים**
- `sum(rate(sumeragi_da_votes_ingested_total[1m])) by (collector_idx)` — זיהוי אספנים שלא קולטים קולות.
- `sumeragi_qc_last_latency_ms{kind="availability"}` וההיסטוגרמה `sumeragi_qc_assembly_latency_ms{kind="availability"}` — זמן ההרכבה העדכני והיסטוריית P95.
- `sumeragi_phase_latency_ms{phase="collect_da"}` ו-`{phase="collect_witness"}` (P95 על פני 5 דק') — זמן לאיסוף קולות זמינות ו-ACK עדים.
- `sumeragi_phase_latency_ms{phase="collect_aggregator"}` — פיזור אספנים; קשרו ל-`sumeragi_gossip_fallback_total`, `block_created_dropped_by_lock_total`, `block_created_hint_mismatch_total`, `block_created_proposal_mismatch_total`, `pacemaker_backpressure_deferrals_total`.
- `sumeragi_phase_latency_ema_ms{phase=…}` — ממוצע נע (EMA) לכל פאזת צינור. סטייה בין EMA למדידות גולמיות תחייב בדיקה.
- `/v1/sumeragi/telemetry` / `iroha_cli sumeragi status --summary` — צילום מצב עם קולות פר אספן, זמני commit certificate, backlog של RBC ו-H/L commit certificate.
- `/v1/sumeragi/status/sse` — זרם SSE בקצב ≈1 שנייה לתצוגות חיות.
- `/v1/sumeragi/phases` / `iroha_cli sumeragi phases --summary` — `{propose_ms,…,commit_ms,pipeline_total_ms}` ו-`ema_ms`.
- `docs/source/grafana_sumeragi_overview.json` — דשבורד Grafana מוכן המציג סטיות בגובה commit certificate, מוני דרופ ונתוני VRF.

**ספי התרעה**
- זמני availability evidence: אם `sumeragi_qc_last_latency_ms{kind="availability"}` חוצה `0.6 * commit_time_ms` פעמיים או שה-P95 עובר `0.7 * commit_time_ms` (permissioned: `CommitTimeMs`, ‏NPoS: `timeouts.timeout_commit_ms`).
- קיפאון הצבעות: `sum(rate(sumeragi_da_votes_ingested_total[2m])) == 0` בזמן ש-`sumeragi_rbc_backlog_sessions_pending > 0`.
- עדי ביצוע: `collect_witness_ms` או P95 של `sumeragi_phase_latency_ms{phase="collect_witness"}` עולים על `0.75 * commit_time_ms` (permissioned: `CommitTimeMs`, ‏NPoS: `timeouts.timeout_commit_ms`), או נותרים אפס ≥3 סבבים למרות בלוקים חדשים.
- פיזור אספנים: `collect_aggregator_ms` > `0.5 * sumeragi.npos.timeouts.aggregator_ms` בשלושה סבבים, ‎`sumeragi_redundant_sends_total` > `redundant_send_r`, ‎`rate(sumeragi_gossip_fallback_total[5m]) > 0`, או ‎`increase(block_created_*[5m]) > 0`/`increase(pacemaker_backpressure_deferrals_total[5m]) > 0`.

מדדי השליחה העודפת (`sumeragi_redundant_sends_total`) עולים כאשר DA משדר מחדש מטעני RBC. הבדיקה `npos_redundant_send_retries_update_metrics` מבטיחה שדשבורדים תואמים לציפיות.

**תהליך תחקור**
1. בדקו עם `iroha_cli sumeragi collectors --summary` שהאספן התקוע עדיין מוקצה.
2. נתחו `/v1/sumeragi/telemetry` כדי לאתר אינדקס ללא `votes_ingested`. אם רק אספן יחיד נפגע, ניתן להגדיל זמנית את `collectors_redundant_send_r`.
3. בדקו `sumeragi_bg_post_queue_depth` ו-`p2p_*_throttled_total` לזיהוי עומסי תורים או מגבלות רשת.
4. עבור עדים – עיינו ב-`/v1/torii/zk/prover/reports` ובלוגים כדי לאתר בעיות בפרוברים.
5. כאשר שני האספנים אינם מגיבים, יש לבדוק backlog של RBC, `sumeragi_rbc_store_evictions_total` ו-`rbc_store.recent_evictions` כדי לזהות עומס דיסק או TTL. בנוסף, `iroha_cli sumeragi status --summary` כולל את המונים `lane_governance_sealed_total` / `lane_governance_sealed_aliases`, כך שניתן לאתר מסילות שעדיין חסומות בלי לפענח את כל ה-JSON. בתהליכי שחרור ו-CI מומלץ להריץ גם `iroha_cli nexus lane-report --only-missing --fail-on-sealed` כדי לעצור אוטומטית כאשר יש מסילות שלא שוחררו.
6. לאחר התאוששות, תעדו את האירוע והשיבו פרמטרים לערכי הבסיס.

### צינור האקראיות (VRF)

ה-pacemaker של NPoS מפיק רוטציה של לידרים ואספנים מ-VRF. כל אפוק (`epoch_length_blocks`) מחולק לשני חלונות:

1. **Commit** (`vrf_commit_deadline_offset`) — ולידטורים מגישים `VrfCommit` עם hash של הגילוי.
2. **Reveal** (`vrf_reveal_deadline_offset`) — הוולידטורים חושפים `VrfReveal`; המערכת מאמתת מול הקומיט ומעדכנת ב-WSV.

הזרימה:
- Torii מקבל `POST /v1/sumeragi/vrf/commit` / `/reveal` (Norito JSON עם `epoch`, `signer`, `*_hex`), מאמת ושולח ל-`SumeragiHandle`.
- `handle_vrf_commit`/`reveal` מאמתות חלונות ותנאים, ומעודכנות נשמרות ב-`world.vrf_epochs`.
- ה-seed של האפוק נשאר קבוע; הסנאפשוטים מעדכנים רק השתתפות ולא משנים בחירת לידר/אספנים עד מעבר אפוק.

במעבר אפוק:
1. חישוב קנסות (`committed_no_reveal`, `no_participation`).
2. ערבוב גילויים חוקיים ל-seed הבא `S_e`.
3. התמדת `VrfEpochRecord` והפקת דו"חות/טלמטריה.

הזרע מזין את `deterministic_collectors`, ו-`/v1/sumeragi/collectors` מציג את התכנון עם `(height, view)`.

#### CLI ותפעול
- `iroha_cli sumeragi vrf-epoch --epoch <n>` מציג seed, השתתפות, קנסות ו-offsetים (`--summary` לשורה אחת).
- `iroha_cli sumeragi telemetry --summary` מסכם `reveals_total`, ‏`late_reveals_total`, ‏`committed_no_reveal`. הסרת `--summary` תחזיר JSON מלא.
- `iroha_cli sumeragi params --summary` מאפשר לוודא ערכים כגון `evidence_horizon_blocks`,‏ `activation_lag_blocks`.
- לשליחות ידניות:

  ```bash
  curl -X POST "$TORII/v1/sumeragi/vrf/commit" \
    -H "Content-Type: application/json" \
    -d '{"epoch":42,"signer":1,"commitment_hex":"0x..."}'
  curl -X POST "$TORII/v1/sumeragi/vrf/reveal" \
    -H "Content-Type: application/json" \
    -d '{"epoch":42,"signer":1,"reveal_hex":"0x..."}'
  ```

  `/v1/sumeragi/collectors` ו-`/v1/sumeragi/status` מאפשרים לאמת `collectors[*].peer_id`, ‏`prf_epoch_seed`, ‏`vrf_late_reveals_total`.

**רשימת בדיקה**
- בדקו את `sumeragi status --summary`; אם `vrf_penalty_epoch` או `committed_no_reveal` עולים – הריצו `sumeragi vrf-epoch --epoch <n>`.
- במהלך חלון ה-commit ודאו שכל ולידטור הגיש קובץ (`commitments_total` ו-`vrf.commitments`).
- לפני הדד-ליין לגילוי עקבו אחרי `vrf.reveals_total` ו-`vrf_late_reveals_total`; אם חסר – פנו לוולידטור או פרסמו ידנית.
- ודאו שה-offsetים המתועדים תואמים לתקנון; חריגות מעידות על סטייה.
- במקרה של גילוי מאוחר: בדקו ש-`vrf.late_reveals` עודכן וש-`prf.epoch_seed` לא השתנה.
- במעבר אפוק ודאו שהקנסות אופסו (`vrf_committed_no_reveal_total` עבור הסיינר ירד ל-0) ושהסנאפשוט (`/v1/sumeragi/vrf/epoch/{n}`) מסמן `finalized: true`.
- הקפידו על תקציבי RBC כפי שמוגדרים ב-CI (`sumeragi_rbc_deliver_broadcasts_total` מתקדם, `sumeragi_bg_post_queue_depth` ≤16).

בדיקות `npos_rbc_store_backpressure_records_metrics` ו-`npos_rbc_chunk_loss_fault_reports_backlog` מאמתות את טלמטריית ה-RBC ע״פ `integration_tests/tests/sumeragi_npos_performance.rs`.

#### מדדים והתראות
- `sumeragi_vrf_commit_emitted_total` / `sumeragi_vrf_reveal_emitted_total` — סופרי commits/reveals שהתקבלו.
- `sumeragi_vrf_non_reveal_total` / `sumeragi_vrf_no_participation_total` — קנסות אפוק.
- `sumeragi_prf_epoch_seed` — ה-seed המשויך (מוצג בהקס בסטטוס).
- `sumeragi_prf_context_height` / `_view` — `(height, view)` שנבחרו לסידור.
- `/v1/sumeragi/vrf/epoch/{epoch}` — כולל `seed_hex`, טבלאות השתתפות וקנסות; `sumeragi vrf-epoch --summary` מציג שורה בודדת.
- `/v1/sumeragi/telemetry` מחזיר את השדה `vrf` עם `{found, epoch, finalized, seed_hex, roster_len, participants_total, commitments_total, reveals_total, late_reveals_total, committed_no_reveal[], no_participation[], late_reveals[]}`.
- `/v1/sumeragi/status` מספק `vrf_penalty_epoch`, `vrf_committed_no_reveal_total`, `vrf_no_participation_total`, `vrf_late_reveals_total`; גילויים מאוחרים נרשמים ב-`sumeragi_vrf_reveals_late_total` וב-`world.vrf_epochs[*].late_reveals` אך לא משנים את ה-seed.

#### ספי התראה ל-VRF
- `increase(sumeragi_vrf_no_participation_total[1h]) > 0` או `increase(sumeragi_vrf_non_reveal_total[1h]) > 0` — יש לפתוח קריאה.
- גדילה ב-`vrf_late_reveals_total` מחייבת בדיקת זמינות הוולידטורים או ערוץ התקשורת.
- שינוי ב-`sumeragi_prf_epoch_seed` באמצע אפוק הוא אירוע חמור; נדרש להשוות לסנאפשוטי `vrf-epoch` וללוגים.
### בדיקות Soak מרובות צמתים

לצורך sign-off של Milestone A6 חובה להריץ את תרחישי העומס של
`sumeragi_npos_performance.rs` על כמה טופולוגיות (4/6/8 צמתים), לשמור את
הלוגים וליצור דוח מרוכז. הסקריפט הבא מריץ את כל התרחישים, מצרף
`summary.json` ו-README לכל תרחיש ומייצא ארכיון ZIP שמוכן להעברה לצוותי SRE:

```bash
python3 scripts/run_sumeragi_soak_matrix.py \
  --artifacts-root artifacts/sumeragi-soak-$(date +%Y%m%d-%H%M) \
  --pack artifacts/sumeragi-soak-$(date +%Y%m%d-%H%M)/signoff.zip
```

מידע נוסף וקובץ הבקרה לחתימה מתוארים ב-
[`sumeragi_soak_matrix.md`](sumeragi_soak_matrix.md).

</div>

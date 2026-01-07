<!-- Hebrew translation of docs/source/global_feature_matrix.md -->

---
lang: he
direction: rtl
source: docs/source/global_feature_matrix.md
status: complete
translator: manual
---

<div dir="rtl">

# מטריצת יכולות גלובלית

אגדה: `◉` הושלם · `○` כמעט הושלם · `▲` חלקי · `△` התחלה · `✖︎` טרם התחיל

## קונצנזוס ורשת

| יכולת | מצב | הערות | מקורות |
|-------|------|--------|---------|
| תמיכת Multi-collector K/r ו-First-commit-certificate-wins | ◉ | בחירת אספן דטרמיניסטית, פאן-אאוט עודף ופרמטרים on-chain. | status.md:255; status.md:314 |
| Bacמת זמן בפייסמייקר (backoff, RTT floor, jitter) | ◉ | טיימרים ניתנים לקונפיגורציה עם טווח jitter, טלמטריה ותיעוד. | status.md:251 |
| NEW_VIEW gating ומעקב Highest commit certificate | ◉ | זרימת השליטה מעבירה NEW_VIEW/Evidence והמעקב אחר Highest commit certificate מונוטוני. | status.md:210 |
| availability evidence gating | ○ | קיומי וגם availability evidence זמינות מגבילים קומיט כשה-DA נדרש. | status.md:190 |
| שערי RBC (DA + Reliable Broadcast) | ◉ | הקומיט מחכה ל-`DELIVER` של RBC בצירוף availability evidence. | status.md:283-284 |
| קישור שורשי מצב ב-Commit QC | ◉ | Commit QC כולל parent/post state roots; אין שער execution QC נפרד. | status.md:latest |
| הפצת Evidence ונקודות קצה לאודיט | ◉ | `ControlFlow::Evidence`, API ב-Torii ובדיקות שליליות. | status.md:176; status.md:760-761 |
| טלמטריית RBC (מדדי מוכנות/מסירה) | ◉ | `/v1/sumeragi/rbc*` והיסטוגרמות למפעילים. | status.md:283-284; status.md:772 |
| פרסום פרמטרי קונצנזוס ואימות טופולוגיה | ◉ | נודים מפרסמים `(collectors_k, redundant_send_r)` ומוודאים זהות. | status.md:255 |
| רוטציה לפי האש הבלוק הקודם | ◉ | הפונקציה `rotated_for_prev_block_hash` דטרמיניסטית ונבחנת. | status.md:259 |

## פייפליין, Kura ומצב

| יכולת | מצב | הערות | מקורות |
|-------|------|--------|---------|
| Caps למסלול הקוורנטין וטלמטריה | ◉ | חוגות קונפיגורציה, טיפול Overflow דטרמיניסטי ומדדים. | status.md:263 |
| Knob לכמות עובדים בפייפליין | ◉ | `[pipeline].workers` מוקפץ לאתחול מצב עם בדיקות env. | status.md:264 |
| מסלול שאילתות snapshot (Stored/Ephemeral) | ◉ | Torii משתמש במסלול בלוקינג עם קרס Stored cursor. | status.md:265; status.md:371; status.md:501 |
| Sidecar של טביעת אצבע DAG | ◉ | מאוחסן ב-Kura, מאומת באתחול ומתריע על אי-התאמות. | status.md:106; status.md:349 |
| קשיחות קריאת hash ב-Kura | ◉ | קריאה כ-raw 32B והחזרת roundtrip בלי תלות ב-Norito. | status.md:608; status.md:668 |
| טלמטריית Norito (AoS/NCB) | ◉ | מדד בחירה בין AoS/NCB הוסף. | status.md:156 |
| שאילתות WSV מ-Torii | ◉ | lane ייעודי עם בתי עסק בלוקינג. | status.md:501 |
| chaining של טריגרים by-call | ◉ | טריגרי דאטה משתלשלים מיידית בסדר דטרמיניסטי. | status.md:668 |

## Norito – סריאליזציה וכלים

| יכולת | מצב | הערות | מקורות |
|-------|------|--------|---------|
| מיגרציית Norito JSON | ◉ | Serde הוסר מהפרודקשן והגארד רייל מונע שימוש חדש. | status.md:112; status.md:124 |
| רשימת deny ל-Serde ו-CI | ◉ | וורקפלואו/סקריפטים חוסמים החזרתו. | status.md:218 |
| גולדן של הקודק ו-AoS/NCB | ◉ | גולדן, בדיקות טרנקטיה ותיעוד מסונכרן. | status.md:140-150; status.md:332; status.md:666 |
| כלי מטריצה ל-Norito | ◉ | סקריפט CI לבדיקות smoke של packed seq/struct. | status.md:146; status.md:152 |
| באינדינגים ל-Python/Java | ◉ | קודקים מתוחזקים עם סקריפטי סינכרון. | status.md:74; status.md:81 |
| מסווגי SIMד Stage-1 | ◉ | NEON/AVX2 עם גולדן צולבי ארכיטקטורה ובדיקות אקראיות. | status.md:241 |

## Governance ושדרוגי Runtime

| יכולת | מצב | הערות | מקורות |
|-------|------|--------|---------|
| קבלת שדרוגים (ABI gating) | ◉ | סט ABI פעיל נאכף בקבלה עם שגיאות מבניות ובדיקות. | status.md:196 |
| gating לפריסה בניימספייס מוגן | ▲ | מנגנון הגנה פעיל; UX/מדיניות ממשיכים. | status.md:171 |
| קריאת ממשל ב-Torii | ◉ | `/v1/gov/*` זמינים עם בדיקות Router. | status.md:212 |
| ניהול רישום verifying-key | ◉ | רישום/עדכון/דפרקציה, ארועים ו-CLI filters זמינים. | status.md:236-239; status.md:595; status.md:603 |

## תשתית Zero-Knowledge

| יכולת | מצב | הערות | מקורות |
|-------|------|--------|---------|
| API לאחסון מצורפים | ◉ | `POST/GET/LIST/DELETE` עם מזהים דטרמיניסטיים ובדיקות. | status.md:231 |
| פרובר רקע ודוחות TTL | ▲ | סטאב מאחורי feature flag; השלם pipeline בהמשך. | status.md:212; status.md:233 |
| קשירת hash של envelope ב-CoreHost | ◉ | CoreHost מאמת hash ומציע audit pulses. | status.md:250 |
| gating היסטוריית רוט | ◉ | Snapshots של רוט ב-CoreHost עם היסטוריה חסומה. | status.md:303 |
| ביצוע הצבעות ZK ונעילות | ○ | הפקת נוליפייר, עדכוני נעילה וטוגלים; lifecycle מלא מתקדם. | status.md:126-128; status.md:194-195 |
| פרה-ווריפיקציה של הוכחות | ◉ | בדיקת backend label, דה-דופ והPersist לפני ביצוע. | status.md:348; status.md:602 |
| CLI והתנתקות base64 | ◉ | CLI עבר ל-API חדש עם בדיקות. | status.md:174 |
| Endpoint לטורי הוכחות | ◉ | `/v1/zk/proof/{backend}/{hash}` זמין. | status.md:94 |

## אינטגרציית IVM/Kotodama

| יכולת | מצב | הערות | מקורות |
|-------|------|--------|---------|
| גשר syscall→ISI ב-CoreHost | ○ | פענוח TLV והכנסת תורים פועלים; נדרשת הרחבת כיסוי. | status.md:299-307; status.md:477-486 |
| בנאי מצביעים ו-builtins | ◉ | Builtins של Kotodama מייצרים TLV ו-SCALL עם בדיקות. | status.md:299-301 |
| אימות Pointer-ABI ותיעוד | ◉ | מדיניות TLV נאכפת עם גולדנים ו-doc sync. | status.md:227; status.md:317; status.md:344; status.md:366; status.md:527 |
| gating Syscalls של ZK ב-CoreHost | ◉ | תורים לכל פעולה מאכפים hash לפני ISI. | crates/iroha_core/src/smartcontracts/ivm/host.rs |
| תיעוד Grammar של Kotodama Pointer-ABI | ◉ | מסונכרן עם היישום. | status.md:299-301 |
| מנוע ISO 20022 ו-Bridge לטורי | ◉ | סכימות וה-API `/v1/iso20022/status/{MsgId}` זמינים. | status.md:65-70 |

## האצות חומרתיות

| יכולת | מצב | הערות | מקורות |
|-------|------|--------|---------|
| בדיקות SIMD ל-tail/misalignment | ◉ | בדיקות אקראיות מאשרות התאמה לסקאלר. | status.md:243 |
| Metal/CUDA fallback ובדיקות עצמיות | ◉ | backend GPU בודקים עצמם ומחזירים לשקלארים בעת הצורך. | status.md:244-246 |

## זמן רשת ומצבי קונצנזוס

| יכולת | מצב | הערות | מקורות |
|-------|------|--------|---------|
| שירות זמן רשת (NTS) | ✖︎ | קיים תכנון בלבד. | new_pipeline.md |
| מצב קונצנזוס NPoS | ✖︎ | המימוש טרם החל. | new_pipeline.md; nexus.md |

## מפת דרכים של Nexus

| יכולת | מצב | הערות | מקורות |
|-------|------|--------|---------|
| חוזה Space Directory | ✖︎ | טרם מומש. | nexus.md |
| מניפסט Data Space | ✖︎ | Schema ו-flow עתידיים. | nexus.md |
| ממשל DS וסיבוב ולידטורים | ✖︎ | תכנון קיים; מימוש בהמשך. | nexus.md |
| קומפוזיציית בלוקים Cross-DS | ✖︎ | טרם יושם. | nexus.md |
| אחסון קידוד-מחיקות ל-Kura/WSV | ✖︎ | בבחינת עתיד. | nexus.md |
| מדיניות הוכחות DS | ✖︎ | לא הוטמע. | nexus.md |
| בידוד עמלות/קצוות | ✖︎ | מתוכנן כהמשך. | nexus.md |

## כאוס והזרקת תקלות

| יכולת | מצב | הערות | מקורות |
|-------|------|--------|---------|
| orchestration של Izanami chaosnet | ○ | Izanami מריץ תרחישים ל-asset/NFT/metadata, כולל בדיקות. | crates/izanami/src/instructions.rs |

</div>

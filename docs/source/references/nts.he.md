<!-- Hebrew translation of docs/source/references/nts.md -->

---
lang: he
direction: rtl
source: docs/source/references/nts.md
status: complete
translator: manual
---

<div dir="rtl">

## שירות הזמן הרשתִי (NTS)

Network Time Service מספק אומדן עמיד בפני ביזנטיות של "זמן רשת" עבור טיימרים ודיאגנוסטיקה של הצמתים. הוא אינו משנה את כללי הקונצנזוס או את שעוני מערכת ההפעלה; חותמות הזמן בכותרות הבלוקים נשארות הגורם הסמכותי לקבלת בלוקים.

### סקירה

- דגימה: שליחת ping/pong בסגנון NTP לפעימות מקוונות באופן מחזורי וחישוב ההיסט ו־RTT לכל דגימה.
- צבירה: סינון חריגי RTT גבוהים וחישוב מדיאן מתוקצר עם אמון MAD.
- החלקה (אופציונלית): ממוצע נע מעריכי (EMA) עם הגבלת שיפוע למניעת קפיצות חדות.
- דטרמיניזם: NTS הוא מייעץ בלבד; תוקף הקונצנזוס וקבלת בלוקים אינם מתייעצים ב-NTS.

### תצורה

מקטע TOML ‏`[nts]` (ראו תבנית ב-`docs/source/references/peer.template.toml`):

- `sample_interval_ms` (u64): תקופת הדגימה. ברירת מחדל ≈ 5000.
- `sample_cap_per_round` (usize): מספר מקסימלי של פירים לכל סבב. ברירת מחדל 8.
- `max_rtt_ms` (u64): השלכת דגימות מעל RTT זה. ברירת מחדל 500.
- `trim_percent` (u8): אחוז חיתוך סימטרי (לכל צד) למדיאן. ברירת מחדל 10.
- `per_peer_buffer` (usize): עומק טבעת הדגימה לכל פִיר. ברירת מחדל 16.
- `smoothing_enabled` (bool): הפעלת EMA. ברירת מחדל false.
- `smoothing_alpha` (f64): אלפא של EMA בתחום [0,1]; גבוה = תגובה מהירה יותר. ברירת מחדל 0.2.
- `max_adjust_ms_per_min` (u64): שינוי מרבי מותר בדקות, במילישניות. ברירת מחדל 50.

דוגמה:

```toml
[nts]
sample_interval_ms = 5_000
sample_cap_per_round = 8
max_rtt_ms = 500
trim_percent = 10
per_peer_buffer = 16
smoothing_enabled = true
smoothing_alpha = 0.2
max_adjust_ms_per_min = 50
```

הנחיות למפעילים:

- הגדירו דרך `iroha_config`; קיימים overrides במשתני סביבה עבור פיתוח ו-CI, אך אין להשתמש בהם לתפעול פרודקשן. ברירות המחדל שמרניות ומתאימות לרוב הפריסות.

### נקודות קצה של Torii

- `GET /v1/time/now` → ‏`{ "now": <ms_epoch>, "offset_ms": <i64>, "confidence_ms": <u64> }`
- `GET /v1/time/status` → דיאגנוסטיקה ואספקת התפלגות RTT:
  - ‏`{ "peers": <u64>, "samples": [{"peer","last_offset_ms","last_rtt_ms","count"}, ...], "rtt": {"buckets": [{"le","count"},...], "sum_ms", "count"}, "note": "NTS running" }`

### מדדי טלמטריה

מיוצאים כמדדי Prometheus:

- `nts_offset_ms` (gauge, חתום) — היסט מול השעון המקומי (מוחלק או גולמי).
- `nts_confidence_ms` (gauge) — גבול אמון MAD.
- `nts_peers_sampled` (gauge) — מספר הפירים שתורמים לדגימות האחרונות.
- `nts_rtt_ms_bucket{le="…"}` (gauge) — דליי היסטוגרמה של RTT (ב-ms).
- `nts_rtt_ms_sum` / `nts_rtt_ms_count` — סכום ומניין ההיסטוגרמה.

### התנהגות והבטחות

- דטרמיניזם: החלטות קונצנזוס אינן תלויות בשעון קיר או ב-NTS; המצב הסופי זהה על פני חומרה שונה.
- בטיחות: NTS אינו משנה את שעון ה-OS; הוא מחזיק היסט ביחס לשעון המקומי בלבד.
- ביצועים: הדגימה והצבירה קלות משקל; טבעות לכל פִיר מגבילות שימוש בזיכרון.

### הערות

- צופים וקליינטים קלים יכולים להסתמך על `time/now` כזמן מייעץ; מאמתים משתמשים ב-NTS רק עבור טיימרים וטיים-אאוטים.
- ניתן לכוון את `smoothing_alpha` ואת `max_adjust_ms_per_min` לפי מאפייני העיכוב ברשת אם יש צורך; ברירות המחדל שמרניות.

</div>

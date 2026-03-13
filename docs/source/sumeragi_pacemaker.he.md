<!-- Hebrew translation of docs/source/sumeragi_pacemaker.md -->

---
lang: he
direction: rtl

Note: Defaults now seed a 1s slot with phase timeouts propose/prevote/precommit/commit/DA = 350/450/550/750/650 ms. See the English `sumeragi_pacemaker.md` for the latest pacing details.
source: docs/source/sumeragi_pacemaker.md
status: complete
translator: manual
---

<div dir="rtl">

# פייסמייקר של Sumeragi — טיימרים, backoff וג׳יטר

מסמך זה מתאר את מדיניות הפייסמייקר (הטיימר) ב-Sumeragi ומספק הכוונה ודוגמאות למפעילים. הטיימרים מגדירים מתי המנהיג מציע בלוק חדש ומתי מאמתים מציעים/נכנסים לתצוגה חדשה לאחר חוסר פעילות.

מימוש נוכחי: חלון בסיס המבוסס EMA, backoff, רצפת RTT ורצועת ג׳יטר קונפיגורבילית (`sumeragi.advanced.pacemaker.jitter_frac_permille`). הג׳יטר מיושם באופן דטרמיניסטי לכל צומת ולכל `(height, view)`.

## מושגים
- **חלון בסיס**: ממוצע נע מעריכי של זמני הפאזות (propose, collect_da, collect_prevote, collect_precommit, commit). ה-EMA מאותחל מערכי `sumeragi.advanced.npos.timeouts.*_ms`; עד הצטברות דגימות הוא דומה לערכים המוגדרים. ה-EMA של `collect_aggregator` נחשף לניטור אך אינו נכנס לחלון הפייסמייקר. הערכים המוחלקים זמינים כ-`sumeragi_phase_latency_ema_ms{phase=…}`.
- **מקדם backoff**: ‏`sumeragi.advanced.pacemaker.backoff_multiplier` (ברירת מחדל 1). כל timeout מוסיף `base * multiplier` לחלון.
- **רצפת RTT**: ‏`avg_rtt_ms * sumeragi.advanced.pacemaker.rtt_floor_multiplier` (ברירת מחדל 2). מונעת timeouts אגרסיביים בקישורים עתירי השהיה.
- **תקרה**: ‏`sumeragi.advanced.pacemaker.max_backoff_ms` (ברירת מחדל ‎60 000 ms). מגבלה קשיחה לחלון.

עדכון חלון בעת timeout:
```
window = min(cap, max(window + base * backoff_mul, avg_rtt * rtt_floor_mul))
```
בהעדר דגימות RTT, הרצפה שווה 0.

טלמטריה (ראו telemetry.md):
- זמן ריצה: ‏`sumeragi_pacemaker_backoff_ms`, ‏`sumeragi_pacemaker_rtt_floor_ms`, ‏`sumeragi_phase_latency_ema_ms{phase=…}`
- REST: ‏`/v2/sumeragi/phases` מוסיף `ema_ms` לצד זמני הפאזות הגבוהים, כך שניתן לשרטט את הטרנד גם ללא Prometheus.
- הגדרות: ‏`sumeragi.advanced.pacemaker.backoff_multiplier`, ‏`sumeragi.advanced.pacemaker.rtt_floor_multiplier`, ‏`sumeragi.advanced.pacemaker.max_backoff_ms`

## מדיניות ג׳יטר
כדי למנוע התנהגות עדר (timeouts מסונכרנים), הפייסמייקר מאפשר ג׳יטר קטן סביב החלון האפקטיבי.

ג׳יטר דטרמיניסטי לצומת:
- מקור: ‏`blake2(chain_id || peer_id || height || view)` → ערך 64-bit מוצמד ל-[−J,+J].
- מומלץ: ±10% מן החלון המחושב.
- יש להחיל פעם אחת לכל `(height, view)`; אין לשנות בתוך אותה תצוגה.

פסאודו-קוד:
```text
base = ema_total_ms(view, height)
window = min(cap, max(prev + base * backoff_mul, avg_rtt * rtt_floor_mul))
seed = blake2(chain_id || peer_id || height || view)
u = (seed % 10_000) / 10_000.0
jfrac = 0.10
jitter = (u * 2 - 1) * jfrac * window
window_jittered_ms = clamp(window + jitter, 0, cap)
```

הערות:
- הג׳יטר דטרמיניסטי לצומת ול-`(height, view)` ואינו דורש אקראיות חיצונית.
- שמרו על רצועה קטנה (≤ 10%) לשמירת זמני תגובה.
- הג׳יטר משפיע על live אך לא על safety.

טלמטריה:
- ‏`sumeragi_pacemaker_jitter_ms` — גודל הג׳יטר (ms).
- ‏`sumeragi_pacemaker_jitter_frac_permille` — פס הג׳יטר המוגדר (permille).

## הנחיות למפעיל

LAN/PoA נמוך השהיה
- backoff_multiplier: ‎1–2
- rtt_floor_multiplier: 2
- max_backoff_ms: ‎5 000–10 000
- מטרה: הצעות תכופות ושחזור מהיר.

WAN גיאוגרפי
- backoff_multiplier: ‎2–3
- rtt_floor_multiplier: ‎3–5
- max_backoff_ms: ‎30 000–60 000
- מטרה: להימנע מריצוד אגרסיבי על קישורים עתירי RTT.

רשתות בעלות שונות גבוהה / ניידות
- backoff_multiplier: ‎3–4
- rtt_floor_multiplier: ‎4–6
- max_backoff_ms: ‎60 000
- מטרה: להפחית סינכרון של שינויי תצוגה; שקלו להפעיל ג׳יטר כאשר ניתן לסבול קומיטים איטיים יותר.

## דוגמאות

1) LAN (RTT ממוצע ≈ 2 ms, בסיס 2000 ms, תקרה 10 000 ms)  
   backoff_mul=1, rtt_floor_mul=2 → timeout ראשון: 2000 ms; שני: 4000 ms

2) WAN (RTT ממוצע ≈ 80 ms, בסיס 4000 ms, תקרה 60 000 ms)  
   backoff_mul=2, rtt_floor_mul=3 → timeout ראשון: 8000 ms; שני: 16 000 ms

3) רצועת ג׳יטר 10% (לצורך הדגמה)  
   window=16 000 ms → ג׳יטר בתחום [−1 600, +1 600] ms.

## ניטור
- מגמה של backoff: ‏`avg_over_time(sumeragi_pacemaker_backoff_ms[5m])`
- בחינת רצפת RTT: ‏`avg_over_time(sumeragi_pacemaker_rtt_floor_ms[5m])`
- השוואת EMA מול התפלגויות: ‏`sumeragi_phase_latency_ema_ms{phase=…}` לצד ‏`sumeragi_phase_latency_ms{phase=…}`
- אימות קונפיגורציה: ‏`max(sumeragi.advanced.pacemaker.backoff_multiplier)` וכדומה

## דטרמיניזם ובטיחות
- טיימרים/backoff/ג׳יטר משפיעים רק על זמני הצעות/שינויי תצוגה; הם אינם נוגעים בתוקף חתימות או בכללי commit certificate.
- הקפידו שכל אקראיות תהיה דטרמיניסטית לצומת ול-`(height, view)`; הימנעו מ-RNG של מערכת ההפעלה בנתיבי קונצנזוס קריטיים.

</div>

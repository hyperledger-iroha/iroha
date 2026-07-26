<!-- Hebrew translation of docs/source/iroha_monitor.md -->

---
lang: he
direction: rtl
source: docs/source/iroha_monitor.md
status: complete
translator: manual
---

<div dir="rtl">

# Iroha Monitor

הגרסה החדשה של Iroha Monitor משלבת UI טרמינלי קליל עם אנימציית ASCII בסגנון חג וצלילי אטנרקו. היא ממקדת את החוויה בשני זרימות:

- **Spawn-lite** – יצירת סטאבים זמניים של סטטוס/מטריקות המדמים פירים.
- **Attach** – התחברות ל-EndPoints קיימים של Torii.

המסך מחולק לארבעה אזורים:
1. **כותרת “קו רקיע של Torii”** – שערי טוריי, פוג׳י, גלים וקונסטלציה מתגלגלים בקצב העדכון.
2. **פס סיכום** – סך בלוקים/עסקאות/גז וזמן הרענון.
3. **טבלת פירים ולוג פסטיבל** – השורות משמאל, לוג אזהרות מתגלגל מימין.
4. **מגמת גז אופציונלית** – `--show-gas-trend` מוסיף תרשים ניצוץ לשימוש הגז הכולל.

חידושים עיקריים:
- אנימציית ASCII יפנית (דגים, טוריי, פנסים).
- CLI פשוט (`--spawn-lite`, ‏`--attach`, ‏`--interval`).
- באנר פתיחה עם השמעת אטנרקו (נגן MIDI חיצוני או soft synth מובנה עם `builtin-synth`).
- Flags `--no-theme` / `--no-audio` להרצות CI או smoke מהירות.
- עמודת “מצב רוח” לכל פיר המציגה אזהרה אחרונה/זמן commit/זמן פעילות.

## התחלה מהירה

סטאבים מקומיים:
```bash
cargo run -p iroha_monitor -- --spawn-lite --peers 3
```

התחברות ל-Torii קיימים:
```bash
cargo run -p iroha_monitor -- \
  --attach http://127.0.0.1:8080 http://127.0.0.1:8081 \
  --interval 500
```

ב-CI:
```bash
cargo run -p iroha_monitor -- --spawn-lite --no-theme --no-audio
```

### דגלי CLI
```
--spawn-lite         מפעיל סטאבים מקומיים (ברירת מחדל)
--attach <URL...>    התחברות ל-EndPoints קיימים
--interval <ms>      פרק רענון (ברירת מחדל 800ms)
--peers <N>          כמות סטאבים ב-spawn-lite (ברירת מחדל 4)
--no-theme           לדלג על האנימציה
--no-audio           להשתיק את הפסקול
--midi-player <cmd>  נגן MIDI חיצוני
--midi-file <path>   קובץ MIDI מותאם לנגן החיצוני
--show-gas-trend     להציג תרשים ניצוץ של הגז
--art-speed <1-8>    קצב אנימציה (1=ברירת מחדל)
--art-theme <שם>     night / dawn / sakura
--headless-max-frames <N>
                     להגביל פריימים במצב headless (0 = ללא גבול)
```

## אנימציית פתיחה

ברירת המחדל מנגנת אנימציית ASCII עם נעימת אטנרקו:
1. אם ניתן `--midi-player`, נוצרת MIDI (או `--midi-file`) והריצה עוברת לנגן.
2. אחרת, עם `builtin-synth`, מושמעת המנגינה באמצעות soft synth פנימי.
3. אם השמע כבוי/נכשל, מוצגת האנימציה בלבד.

הסינתיסייזר מבוסס על *MIDI synth design in Rust.pdf*: היצ׳יריקי וריוטקי בנגינה הטרופונית, והשō באקורדים aitake. תווי העיתוי ב-`etenraku.rs` מזינים גם את CPAL וגם את קובץ ה-MIDI.

## סקירת UI

- **כותרת** – נוצרת בכל פריים ע"י `AsciiAnimator`.
- **פס סיכום** – מציג פירים פעילים, סה״כ בלוקים, עסקאות, גז, קצבי רענון ועוד.
- **טבלת פירים** – אליאס/קצה, בלוקים, עסקאות, גודל תור, גז, לטנסי, מצב רוח.
- **Festival whispers** – לוג רץ של אזהרות (שגיאות חיבור, חריגות payload, קצבים איטיים).

מקשים:
- `n` / ימינה / למטה – מעבר לפיר הבא.
- `p` / שמאלה / למעלה – חזרה לפיר הקודם.
- `q` / Esc / Ctrl-C – יציאה ושחזור המסוף.

ה-monitor עושה שימוש ב-crossterm + ratatui עם מסך חלופי ומשיב את המסך בסיום.

## בדיקות Smoke

קרייט זה כולל בדיקות אינטגרציה כגון:
- `spawn_lite_smoke_renders_frames`
- `attach_mode_with_stubs_runs_cleanly`
- `invalid_endpoint_surfaces_warning`
- `status_limit_warning_is_rendered`
- `attach_mode_with_slow_peer_renders_multiple_frames`

</div>

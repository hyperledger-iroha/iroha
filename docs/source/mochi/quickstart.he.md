---
lang: he
direction: rtl
source: docs/source/mochi/quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0f5bc20c9545dd210ea8cd3adec2e8dc9957e99d5939f45afc8c28def7f1c6b1
source_last_modified: "2025-11-20T04:32:51.480819+00:00"
translation_last_reviewed: 2026-01-21
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/source/mochi/quickstart.md -->

# התחלה מהירה ל‑MOCHI

**MOCHI** הוא ה‑desktop supervisor לרשתות Hyperledger Iroha מקומיות. המדריך
מסביר כיצד להתקין דרישות מקדימות, לבנות את היישום, להפעיל את מעטפת egui,
ולהשתמש בכלי ה‑runtime (הגדרות, snapshots, מחיקות) לפיתוח יומיומי.

## דרישות מקדימות

- שרשרת כלים של Rust: `rustup default stable` (ה‑workspace משתמש במהדורה 2024 /
  Rust 1.82+).
- שרשרת כלים פלטפורמית:
  - macOS: Xcode Command Line Tools (`xcode-select --install`).
  - Linux: GCC, pkg-config, OpenSSL headers (`sudo apt install build-essential pkg-config libssl-dev`).
- תלויות ה‑workspace של Iroha:
  - `cargo xtask mochi-bundle` דורש `irohad`, `kagami`, ו‑`iroha_cli` שנבנו.
    בנו אותם פעם אחת באמצעות `cargo build -p irohad -p kagami -p iroha_cli`.
- אופציונלי: `direnv` או `cargo binstall` לניהול בינאריים מקומיים של cargo.

MOCHI מריץ את בינארי ה‑CLI דרך shell. ודאו שהם נגישים באמצעות משתני הסביבה
להלן או זמינים ב‑PATH:

| בינארי   | דריסת סביבה | הערות                                  |
|----------|-------------|----------------------------------------|
| `irohad` | `MOCHI_IROHAD` | מפקח על peers                         |
| `kagami` | `MOCHI_KAGAMI` | יוצר manifest/ snapshots של genesis  |
| `iroha_cli` | `MOCHI_IROHA_CLI` | אופציונלי לתכונות עזר עתידיות |

## בניית MOCHI

משורש הריפו:

```bash
cargo build -p mochi-ui-egui
```

פקודה זו בונה גם את `mochi-core` וגם את ה‑frontend של egui. כדי להפיק חבילת
הפצה, הריצו:

```bash
cargo xtask mochi-bundle
```

משימת ה‑bundle מרכיבה את הבינאריים, המניפסט ותצורות ה‑stub תחת
`target/mochi-bundle`.

## הפעלת מעטפת egui

הריצו את ה‑UI ישירות דרך cargo:

```bash
cargo run -p mochi-ui-egui
```

ברירת המחדל של MOCHI יוצרת preset של peer יחיד בספריית נתונים זמנית:

- שורש נתונים: `$TMPDIR/mochi`.
- פורט בסיס Torii: `8080`.
- פורט בסיס P2P: `1337`.

השתמשו בדגלי CLI כדי לעקוף את ברירות המחדל בעת ההפעלה:

```bash
cargo run -p mochi-ui-egui -- \
  --data-root /path/to/workspace \
  --profile four-peer-bft \
  --torii-start 12000 \
  --p2p-start 13000 \
  --kagami /path/to/kagami \
  --irohad /path/to/irohad
```

משתני סביבה משקפים את אותן דריסות כאשר דגלי CLI מושמטים: הגדירו
`MOCHI_DATA_ROOT`, `MOCHI_PROFILE`, `MOCHI_CHAIN_ID`, `MOCHI_TORII_START`,
`MOCHI_P2P_START`, `MOCHI_RESTART_MODE`, `MOCHI_RESTART_MAX`, או
`MOCHI_RESTART_BACKOFF_MS` כדי לזרוע מראש את ה‑supervisor builder; נתיבי
הבינאריים ממשיכים לכבד `MOCHI_IROHAD`/`MOCHI_KAGAMI`/`MOCHI_IROHA_CLI`,
ו‑`MOCHI_CONFIG` מצביע ל‑`config/local.toml` מפורש.

## הגדרות ושמירה

פתחו את דיאלוג **Settings** מסרגל הכלים של הדשבורד כדי להתאים את תצורת
ה‑supervisor:

- **Data root** — ספריית בסיס לתצורות peer, אחסון, לוגים ו‑snapshots.
- **Torii / P2P base ports** — פורטי התחלה להקצאה דטרמיניסטית.
- **Log visibility** — החלפת ערוצים stdout/stderr/system במציג הלוגים.

כפתורים מתקדמים כמו מדיניות האתחול של ה‑supervisor נמצאים ב‑`config/local.toml`.
הגדירו `[supervisor.restart] mode = "never"` כדי לבטל אתחולים אוטומטיים בזמן
ניפוי תקלות, או התאימו `max_restarts`/`backoff_ms` (דרך קובץ התצורה או דגלי
CLI `--restart-mode`, `--restart-max`, `--restart-backoff-ms`) כדי לשלוט
בהתנהגות הנסיונות.

החלת שינויים בונה מחדש את ה‑supervisor, מאתחלת מחדש peers רצים, וכותבת את
הדריסות אל `config/local.toml`. מיזוג התצורה שומר על מפתחות שאינם קשורים כך
שמשתמשים מתקדמים יכולים לשמר התאמות ידניות לצד ערכים שמנוהלים ע"י MOCHI.

## Snapshots ו‑wipe/re-genesis

דיאלוג **Maintenance** חושף שתי פעולות בטיחות:

- **Export snapshot** — מעתיק אחסון/config/logs של peers ואת מניפסט ה‑genesis הנוכחי
  אל `snapshots/<label>` תחת שורש הנתונים הפעיל. תוויות מנוקות אוטומטית.
- **Restore snapshot** — משחזר אחסון peers, snapshot roots, config, logs ומניפסט
  genesis מחבילה קיימת. `Supervisor::restore_snapshot` מקבל נתיב מוחלט או את שם
  התיקייה המחוטא `snapshots/<label>`; ה‑UI משקף זאת כך ש‑Maintenance → Restore
  יכול לשחזר חבילות ראיות בלי לגעת בקבצים ידנית.
- **Wipe & re-genesis** — עוצר peers רצים, מסיר תיקיות אחסון, מייצר genesis מחדש
  דרך Kagami, ומאתחל מחדש peers כאשר המחיקה מסתיימת.

שני הזרמים מכוסים בבדיקות רגרסיה (`export_snapshot_captures_storage_and_metadata`,
`wipe_and_regenerate_resets_storage_and_genesis`) כדי להבטיח פלטים דטרמיניסטיים.

## לוגים וזרמים

הדשבורד חושף נתונים/מטריקות במבט מהיר:

- **Logs** — עוקב אחרי הודעות מחזור החיים של `irohad` (stdout/stderr/system).
  ניתן להחליף ערוצים ב‑Settings.
- **Blocks / Events** — זרמים מנוהלים שמתחברים מחדש אוטומטית עם backoff מעריכי
  ומצרפים למסגרות תקצירים מפוענחים של Norito.
- **Status** — מבצע polling אל `/status` ומציג sparklines לעומק תור, throughput
  ו‑latency.
- **Startup readiness** — לאחר לחיצה על **Start** (peer יחיד או כל peers), MOCHI
  בודק `/status` עם backoff מוגבל; הבאנר מדווח מתי כל peer מוכן (עם עומק התור שנצפה)
  או מציג את שגיאת Torii אם ה‑readiness חורג מהזמן.

הלשוניות עבור state explorer ו‑composer מספקות גישה מהירה לחשבונות, נכסים,
peers והוראות נפוצות בלי לעזוב את ה‑UI. תצוגת Peers משקפת את שאילתת `FindPeers`
כך שתוכלו לאשר אילו מפתחות ציבוריים רשומים במערך המאמתים לפני הרצת בדיקות
אינטגרציה.

השתמשו בכפתור **Manage signing vault** בסרגל הכלים של ה‑composer כדי לייבא או
לערוך סמכויות חתימה. הדיאלוג כותב רשומות לשורש הרשת הפעיל
(`<data_root>/<profile>/signers.json`), ומפתחות ה‑vault השמורים זמינים מיד
ל‑transaction previews ולשליחות. כאשר ה‑vault ריק ה‑composer חוזר למפתחות הפיתוח
המצורפים כדי שזרימות מקומיות ימשיכו לעבוד. הטפסים מכסים כעת mint/burn/transfer
(כולל receive משתמע), רישום domain/account/asset-definition, מדיניות קבלה לחשבונות,
הצעות multisig, מניפסטים של Space Directory ‏(AXT/AMX), מניפסטי pin של SoraFS,
ופעולות ממשל כגון הענקה או ביטול של תפקידים, כך שמשימות כתיבת roadmap
ניתנות לתרגול ללא כתיבת payloads Norito ידנית.

## ניקוי ותקלות

- עצרו את היישום כדי לסיים peers בפיקוח.
- הסירו את שורש הנתונים (`rm -rf <data_root>`) כדי לאפס את כל המצב.
- אם מיקומי Kagami או irohad משתנים, עדכנו את משתני הסביבה או הריצו את MOCHI עם
  דגלי ה‑CLI המתאימים; דיאלוג Settings ישמור נתיבים חדשים בהחלה הבאה.

למידע נוסף על אוטומציה בדקו `mochi/mochi-core/tests` (בדיקות מחזור חיי supervisor)
ו‑`mochi/mochi-integration` עבור תרחישי Torii מדומים. כדי לשלוח bundles או לחבר
את שולחן העבודה לצינורות CI, עיינו במדריך {doc}`mochi/packaging`.

## שער בדיקות מקומי

הריצו `ci/check_mochi.sh` לפני שליחת patches כדי ששער ה‑CI המשותף יבחן את
שלושת קרייטי MOCHI:

```bash
./ci/check_mochi.sh
```

העזר מריץ `cargo check`/`cargo test` עבור `mochi-core`, `mochi-ui-egui`,
ו‑`mochi-integration`, וכך מזהה סטייה ב‑fixtures (לכידות בלוקים/אירועים קנוניות)
ורגרסיות ב‑egui. אם הסקריפט מדווח על fixtures מיושנים, הריצו מחדש את בדיקות
ה‑regeneration המושתקות, למשל:

```bash
cargo test -p mochi-core regenerate_block_wire_fixture -- --ignored
```

הרצה חוזרת של השער לאחר הרגנרציה מבטיחה שה‑bytes המעודכנים נשארים עקביים לפני
דחיפה.

</div>

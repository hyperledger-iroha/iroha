<!-- Hebrew translation of README.md (Hyperledger Iroha) -->

---
lang: he
direction: rtl
source: README.md
status: complete
translator: manual
---

<div dir="rtl">

# Hyperledger Iroha

[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

Hyperledger Iroha היא ספריית בלוקצ'יין פשוטה ויעילה שנבנתה על בסיס **distributed ledger technology (DLT)**. העיצוב שלה מאמץ את גישת ה"קאיזן" היפנית להסרת עומס מיותר (*muri*) כדי לספק מימוש יציב ומחושל.

Iroha מסייעת לכם לנהל חשבונות, נכסים ואחסון נתונים על הרשת באמצעות חוזים חכמים יעילים, תוך שמירה על עמידות בפני תקלות ביזנטיות ותקלות קריסה כאחת.

> _מצב התיעוד:_ קובץ README זה משקף את מצב סביבת העבודה כפי שנרשם ב־`status.md` (עודכן לאחרונה ב־12-02-2026). עיינו בקובץ זה לקבלת פרטים על בריאות הרכיבים, אבני דרך אחרונות ופעולות להמשך.

לסקירה בעברית, קראו את המסמך הנוכחי. סקירה מקיפה ביפנית זמינה ב־[`README.ja.md`](./README.ja.md). הנחיות על תהליך התרגום ועל כיסוי השפות מצויות ב־[`docs/i18n/README.md`](./docs/i18n/README.md).

## תכונות

Iroha היא ספריית בלוקצ'יין מלאה. ניתן להשתמש בה כדי:

* לבנות נכסים פונג'יביליים ולא־פונג'יביליים באמצעות סכמות Norito דטרמיניסטיות
* לנהל חשבונות משתמשים עם דומיינים היררכיים, מדיניות חתימות מרובות ומטה־נתונים הניתנים להתאמה
* להפעיל חוזים חכמים דרך מהדר Kotodama רב־פקודות היעד ל־Iroha Virtual Machine‏ (IVM), או להסתמך על הוראות מובנות (Special Instructions)
* להפעיל את מסלול הקונצנזוס NPoS Sumeragi עם אקראיות commit/reveal מבוססת VRF, זמינות נתונים מגובהה ב־RBC והוכחות ענישה המונהגות באמצעות ממשל
* להגן על זרימות חסויות בעזרת תקציר תכונות חסויות על השרשרת, רגיסטרי מפתחות מאמתים ומועדי פג תוקף דטרמיניסטיים להוכחות
* לפרוס ברשתות מאובטחות או פתוחות עם טלמטריה, CLI וכלי Norito כחלק מהסטנדרט

Iroha מציעה גם:

* עמידות בפני תקלות ביזנטיות עד 33% של מאמתים פגומים במסגרת מצת NPoS Sumeragi
* ביצוע דטרמיניסטי עם מצב־עולם בזיכרון והתמדה מבוססת Norito
* טלמטריה עשירה, הוכחות ואקראיות מדווחת (ראו [Telemetry](#telemetry))
* ארכיטקטורה מודולרית עם הפרדה ברורה בין הקרייטים ושימוש חוזר בקודק Norito
* תכנון מוכוון־אירועים, מוקלד־חוזקה, המשדר מעל SSE/WebSocket לצד הקרנות Norito JSON

## סקירה כללית

- בדקו את [דרישות המערכת](#דרישות-מערכת) ולמדו כיצד [לבנות, לבדוק ולהריץ את Iroha](#בנייה-בדיקה-והרצה-של-iroha)
- עיינו ב[קרייטים](#אינטגרציה) הכלולים
- למדו כיצד [להגדיר ולהפעיל את Iroha](#תחזוקה)
- עברו על [קובץ ה־genesis](./docs/genesis.md) שמאתחל רשת חדשה
- הבינו את [קיבולות התורים ונתוני המדידה של P2P](./docs/source/p2p.md)
- עברו על [צינור עיבוד העסקאות מקצה לקצה](./docs/source/pipeline.md) ועל מדדי מצת/זמינות הנתונים המסוכמים ב־[new_pipeline.md](./new_pipeline.md)
- קראו את [מדריכי הקונצנזוס, האקראיות וההוכחות של Sumeragi](./docs/source/sumeragi.md) יחד עם ממשק ה־API לממשל (`docs/source/governance_api.md`)
- העמיקו בזרימת Norito ובטלמטריה דרך [docs/source/norito_streaming.md](./docs/source/norito_streaming.md)
- המשיכו ל[קריאה נוספת](#קריאה-נוספת):
  - ממשקי ZK לאפליקציות (צרופות ודוחות מוכיחים): `docs/source/zk_app_api.md`
  - תסריט הבדיקות העשן של CI: `scripts/ci/zk_smoke.sh` (הוראות `register-asset` ו־`shield`)

משאבים קהילתיים:
- [תרמו](./CONTRIBUTING.md) למאגר
- [צרו קשר](./CONTRIBUTING.md#contact) לקבלת תמיכה

## דרישות מערכת

דרישות ה־RAM והאחסון תלויות בשאלה האם אתם בונים את הפרויקט או מפעילים רשת, ובגודל הרשת ובנפח העסקאות. הטבלה הבאה מספקת קווים מנחים:

| שימוש              | CPU                | RAM   | אחסון[^1] |
|--------------------|--------------------|-------|-----------|
| Build (מינימום)    | CPU דו־ליבתי       | 4GB   | 20GB      |
| Build (מומלץ)      | AMD Ryzen™ 5 1600  | 16GB  | 40GB      |
| Deploy (קטן)       | CPU דו־ליבתי       | 8GB+  | 20GB+     |
| Deploy (גדול)      | AMD Epyc™ 64-core  | 128GB | 128GB+    |

[^1]: כל הפעולות מתבצעות בזיכרון, ולכן Iroha יכולה לפעול תאורטית ללא אחסון מתמשך. עם זאת, לאחר כשל חשמל, סנכרון מחדש של הבלוקים מהרשת עלול להימשך זמן, ולכן מומלץ להקצות נפח אחסון.

שיקולי RAM:

* בממוצע תכננו על 5 KiB לכל חשבון. רשת עם 1,000,000 חשבונות צורכת כ־5 GiB.
* כל הוראת `Transfer` או `Mint` צורכת בערך 1 KiB.
* מכיוון שהעסקאות נשמרות בזיכרון, השימוש ב־RAM גדל ליניארית עם קצב העסקאות ומשך הפעילות.

שיקולי CPU:

* קומפילציית Rust מעדיפה מעבדים מרובי ליבות כמו Apple M1™, ‏AMD Ryzen™/Threadripper™/Epyc™ ו־Intel Alder Lake™.
* במערכות עם זיכרון מוגבל ומספר רב של ליבות, הקומפילציה עלולה להיכשל עם `SIGKILL`. השתמשו ב־`cargo build -j <number>` (החליפו את `<number>` במחצית מכמות ה־RAM שלכם כשהיא מעוגלת מטה) כדי להגביל את המקביליות.

## בנייה, בדיקה והרצה של Iroha

### דרישות מקדימות

* [Rust](https://www.rust-lang.org/learn/get-started) (הכלי היציב; בניות פרופיל דורשות nightly בהתאם ל[מדריך הפרופיל](./CONTRIBUTING.md#profiling))
* (אופציונלי) [Docker](https://docs.docker.com/get-docker/)
* (אופציונלי) [Docker Compose](https://docs.docker.com/compose/install/)
* (אופציונלי) דוגמאות בייטקוד מוכנות של IVM עבור בדיקות התלויות בהן. הסקריפט ההיסטורי `scripts/build_ivm.sh` הוסר.

### בניית Iroha

בנו את כל סביבת העבודה עם הכלי היציב:

```bash
cargo build --workspace
```

פקודות עזר אופציונליות:

```bash
# הפעלת קומפילציה אינקרמנטלית
CARGO_INCREMENTAL=1 cargo build

# איסוף דיאגנוסטיקה מפורטת ומדדי זמן
cargo build -vv --timings

# בניית דימוי Docker עדכני
docker build . -t hyperledger/iroha:dev
```

אם דילגתם על בניית Docker, הדימוי הזמין האחרון ישמש בעת העלאת נוד בקונטיינר.

### יצירת דוגמאות לקודק מחדש

כאשר סכמת מודל הנתונים משתנה, צרו מחדש את דוגמאות קודק Norito שבהן משתמש `kagami`:

```bash
cargo run --manifest-path scripts/regenerate_codec_samples/Cargo.toml
```

התוצרים יישמרו ב־`crates/iroha_kagami/samples/codec/`.

### הפעלת בדיקות UI ידנית

בדיקות UI מאמתות דיאגנוסטיקה בזמן קומפילציה ואינן רצות ב־CI. הריצו אותן מקומית כשאתם מבצעים שינויים שמשפיעים על ההודעות:

```bash
cargo test -p iroha_data_model --test ui
```

### בדיקת סביבת העבודה

השתמשו בפקודת הבדיקות של סביבת העבודה כדי להריץ בדיקות יחידה, אינטגרציה ותיעוד:

```bash
cargo test --workspace
```

### הרצת Iroha

לאחר הבנייה, הפעילו רשת מינימלית:

```bash
docker compose up
```

כשמחסנית Docker Compose רצה, התחברו אליה באמצעות [Iroha Client CLI](crates/iroha_cli/README.md):

```bash
cargo run --bin iroha -- --config ./defaults/client.toml
```

### ממשק TUI (`iroha_monitor`) — הצמדה או הפעלה קלה

קיים ממשק TUI בסגנון synthwave למעקב אחר מצב הצמתים ומדדי P2P.

- התחברות לצמתים קיימים (מומלץ):

  ```bash
  # צומת יחיד
  cargo run -p iroha_monitor -- --attach http://127.0.0.1:8080 --use-alice

  # מספר צמתים
  cargo run -p iroha_monitor -- --attach http://127.0.0.1:8080 --attach http://127.0.0.1:8081 --use-alice
  ```

- הפעלה ברירת־מחדל (מייצרת תהליכים משניים):

  ```bash
  cargo run -p iroha_monitor
  ```

### Telemetry

טלמטריה מרכזת מדדים ורמות בריאות. השימוש הבסיסי:

```bash
curl http://127.0.0.1:8080/status
```

ה־API מחזיר Norito bare כברירת מחדל. בקשו JSON בעזרת הכותרת `accept: application/json`. Torii מפעילה כברירת מחדל את הפיצ׳רים `app_api` ו-`transparent_api` ב-`iroha_torii`; `transparent_api` מעביר הלאה את "המבנים השקופים" של מודל הנתונים כך שניתן להקרין את שדות ה-ledger ישירות אל JSON בלי טיפוסי עזר.

### Streaming

Iroha מספקת סטרימים של אירועים ושל בלוקים דרך SSE ו־WebSocket. ראו:

- `docs/source/norito_streaming.md`
- `docs/source/telemetry.md`
- `integration_tests/tests/streaming/mod.rs`

### חותמות זמן

השירות `/status` של Torii מחזיר חותמות זמן עכשוויות, והקרייט `iroha_time` מספק כלי עזר למדידת השהיות ולהערכת סטיות שעון.

## אינטגרציה

Hyperledger Iroha מאורגן כ־Cargo workspace. קרייטים מרכזיים:

* `iroha` – הספרייה העליונה המאחדת פונקציונליות ליבה
* `irohad` – בינארי הדמון שמריץ את הנוד
* `iroha_core`, ‏`iroha_data_model`, ‏`iroha_crypto` – שכבות עיבוד הליבה
* `ivm` – Iroha Virtual Machine
* `iroha_cli` – כלי שורת פקודה ללקוחות
* `iroha_torii` – שרת ה־API
* `iroha_config` – ניהול קונפיגורציה
* `integration_tests` – בדיקות שמכסות זרימות בין רכיבים

## תחזוקה

- [הגדרה וקונפיגורציה](./docs/source/references/configuration.md) – מקורות קונפיגורציה והיררכיה (משתמש → ברירת מחדל → runtime)
- [סכמת Genesis](./docs/genesis.md) – מבנה הגדרות רשת ראשוני
- [שדרוגי Runtime](./docs/source/runtime_upgrades.md) – כיצד לפרוס גרסאות חדשות בבטחה
- [מדיניות אבטחה](./docs/source/security_hardening_requirements.md) – צ'ק־ליסט לייצור
- [ניהול מפתחות](./docs/source/kaigi_privacy_design.md) – פרטיות Kaigi וחלוקת סודות

## קריאה נוספת

- מסמכי Norito ו־זרימת הסטרימינג: `docs/source/norito_streaming.md`, ‏`docs/norito_streaming.md`
- מדריך RBAC ו־NPoS Sumeragi: `docs/source/rbac.md`, ‏`docs/source/sumeragi.md`
- ממשקי Torii: `docs/source/torii_contracts_api.md`, ‏`docs/source/torii_query_cursor_modes.md`
- מסמכי ZK: `docs/source/zk_envelopes.md`, ‏`docs/source/zk1_envelope.md`, ‏`docs/source/zk_app_api.md`
- נתיב Nexus והסטטוס הכללי: `roadmap.md`, ‏`status.md`

## רישיון

Hyperledger Iroha מופצת תחת רישיון Apache 2.0. לפרטים ראו את [LICENSE](./LICENSE).

</div>

---
lang: he
direction: rtl
source: docs/source/crypto/sm_perf_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 493c3c0f6a991b2a5d04f33f97b7e97bff372271c5c57751ff41f5e86d43cbc7
source_last_modified: "2026-01-03T18:07:57.107521+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## לכידת ביצועים של SM ותוכנית בסיס

סטטוס: נוסח — 2025-05-18  
בעלים: Performance WG (מוביל), Infra Ops (תזמון מעבדה), QA Guild (CI gating)  
משימות מפת דרכים קשורות: SM-4c.1a/b, SM-5a.3b, FASTPQ Stage 7 לכידה חוצה-מכשירים

### 1. יעדים
1. רשום חציוני Neoverse ב-`sm_perf_baseline_aarch64_unknown_linux_gnu_{scalar,auto,neon_force}.json`. קווי הבסיס הנוכחיים מיוצאים מהלכידה `neoverse-proxy-macos` תחת `artifacts/sm_perf/2026-03-lab/neoverse-proxy-macos/` (תווית CPU `neoverse-proxy-macos`) עם סובלנות ההשוואה SM3 שהורחבה ל-0.70 עבור aarch64 macOS/Linux. כשזמן Bare-metal נפתח, הפעל מחדש את `scripts/sm_perf_capture_helper.sh --matrix --cpu-label neoverse-n2-b01 --output artifacts/sm_perf/<date>/neoverse-n2-b01` במארח Neoverse וקדם את החציונים המצטברים לתוך קווי הבסיס.  
2. אסוף חציונים תואמים של x86_64 כך ש-`ci/check_sm_perf.sh` יוכל לשמור על שתי הכיתות המארחות.  
3. פרסם נוהל לכידה שניתן לחזור עליו (פקודות, פריסת חפצים, סוקרים) כך ששערי המבצע העתידיים לא יסתמכו על ידע שבטי.

### 2. זמינות חומרה
ניתן להגיע רק למארחים של Apple Silicon (macOS arm64) בסביבת העבודה הנוכחית. לכידת `neoverse-proxy-macos` מיוצאת כנקודת הבסיס הזמנית של לינוקס, אך לכידת חציון Neoverse או x86_64 ממתכת חשופה עדיין מחייבת את חומרת המעבדה המשותפת המנוהלת תחת `INFRA-2751`, שתופעל על ידי Performance WG ברגע שחלון המעבדה נפתח. חלונות הלכידה הנותרים מוזמנים כעת ועוקבים אחריהם בעץ החפצים:

- Neoverse N2 רק-מתכת (מתלה טוקיו B) הוזמן ל-2026-03-12. המפעילים יעשו שימוש חוזר בפקודות מסעיף 3 ויאחסנו חפצים תחת `artifacts/sm_perf/2026-03-lab/neoverse-b01/`.
- x86_64 Xeon (מתלה D של ציריך) הוזמן ל-2026-03-19 עם SMT מושבת כדי להפחית רעש; חפצי אמנות ינחתו תחת `artifacts/sm_perf/2026-03-lab/xeon-d01/`.
- לאחר ששתי הריצות נוחתות, קדם את החציונים ל-JSONs הבסיס והפעל את שער ה-CI ב-`ci/check_sm_perf.sh` (תאריך החלפת יעד: 2026-03-25).

עד לתאריכים אלה, ניתן לרענן רק את קווי הבסיס של macOS arm64 באופן מקומי.### 3. נוהל לכידה
1. **סנכרון שרשרת כלים**  
   ```bash
   rustup override set $(cat rust-toolchain.toml)
   cargo fetch
   ```
2. **צור מטריצת לכידה** (למארח)  
   ```bash
   scripts/sm_perf_capture_helper.sh --matrix \
     --output artifacts/sm_perf/2025-07-lab/${HOSTNAME}
   ```
   העוזר כותב כעת `capture_commands.sh` ו-`capture_plan.json` תחת ספריית היעד. הסקריפט מגדיר נתיבי לכידה `raw/*.json` לכל מצב כך שטכנאי מעבדה יכולים לקבץ את הריצות באופן דטרמיניסטי.
3. **הרץ לכידות**  
   בצע כל פקודה מ-`capture_commands.sh` (או הפעל את המקבילה באופן ידני), וודא שכל מצב פולט כתם JSON מובנה דרך `--capture-json`. ספק תמיד תווית מארח באמצעות `--cpu-label "<model/bin>"` (או `SM_PERF_CPU_LABEL=<label>`) כך שהמטא-נתונים הלכידה וקווי הבסיס הבאים יתעדו את החומרה המדויקת שיצרה את החציונים. העוזר כבר מספק את הנתיב המתאים; עבור ריצות ידניות הדפוס הוא:
   ```bash
   SM_PERF_CAPTURE_LABEL=auto \
   scripts/sm_perf.sh --mode auto \
     --cpu-label "neoverse-n2-lab-b01" \
     --capture-json artifacts/sm_perf/2025-07-lab/${HOSTNAME}/raw/auto.json
   ```
4. **אמת תוצאות**  
   ```bash
   scripts/sm_perf_check \
     artifacts/sm_perf/2025-07-lab/${HOSTNAME}/raw/*.json
   ```
   ודא שהשונות נשארת בטווח של ±3% בין ריצות. אם לא, הפעל מחדש את המצב המושפע ורשום את הניסיון החוזר ביומן.
5. **קדם חציונים**  
   השתמש ב-`scripts/sm_perf_aggregate.py` כדי לחשב חציונים ולהעתיק אותם לקובצי ה-JSON הבסיסיים:
   ```bash
   scripts/sm_perf_aggregate.py \
     artifacts/sm_perf/2025-07-lab/${HOSTNAME}/raw/*.json \
     --output artifacts/sm_perf/2025-07-lab/${HOSTNAME}/aggregated.json
   ```
   קבוצות העזר לוכדות על ידי `metadata.mode`, מאמתות שכל קבוצה חולקת את
   אותו `{target_arch, target_os}` משולש, ופולט סיכום JSON עם ערך אחד
   לכל מצב. החציונים שאמורים לנחות בקבצי הבסיס חיים מתחת
   `modes.<mode>.benchmarks`, בעוד שרשומות הבלוק `statistics` הנלוות
   רשימת המדגם המלאה, מינימום/מקסימום, ממוצע ו-Stdev של אוכלוסיה עבור סוקרים ו-CI.
   ברגע שהקובץ המצטבר קיים, אתה יכול לכתוב אוטומטית את ה-JSONs הבסיסי (עם
   מפת הסובלנות הסטנדרטית) באמצעות:
   ```bash
   scripts/sm_perf_promote_baseline.py \
     artifacts/sm_perf/2025-07-lab/${HOSTNAME}/aggregated.json \
     --out-dir crates/iroha_crypto/benches \
     --target-os unknown_linux_gnu \
     --overwrite
   ```
   עוקף את `--mode` כדי להגביל לקבוצת משנה או `--cpu-label` כדי להצמיד את
   שם מעבד מוקלט כאשר המקור המצטבר משמיט אותו.
   לאחר ששני המארחים לכל ארכיטקטורה מסתיימים, עדכן:
   - `sm_perf_baseline_aarch64_unknown_linux_gnu_{scalar,auto,neon_force}.json`
   - `sm_perf_baseline_x86_64_unknown_linux_gnu_{scalar,auto}.json` (חדש)

   קבצי `aarch64_unknown_linux_gnu_*` משקפים כעת את `m3-pro-native`
   לכידה (תווית המעבד והערות מטא נתונים נשמרו) כך ש-`scripts/sm_perf.sh` יכול
   זיהוי אוטומטי של מארחי aarch64-unknown-linux-gnu ללא דגלים ידניים. כאשר ה
   ריצת מעבדה bare-metal הושלמה, הפעל מחדש `scripts/sm_perf.sh --mode 
   --write-baseline crates/iroha_crypto/benches/sm_perf_baseline_aarch64_unknown_linux_gnu_.json`
   עם הלכידות החדשות כדי להחליף את החציונים הביניים ולהחתים את האמיתי
   תווית מארח.

   > התייחסות: לכידת הסיליקון של אפל 2025 (תווית המעבד `m3-pro-local`) היא
   > בארכיון תחת `artifacts/sm_perf/2025-07-lab/takemiyacStudio.lan/{raw,aggregated.json}`.
   > שיקוף את הפריסה הזו כשאתה מפרסם את חפצי האמנות של Neoverse/x86 כך שהסוקרים
   > יכול להבדיל את התפוקות הגולמיות/מצטברות באופן עקבי.

### 4. פריסת חפצים וחתימה
```
artifacts/sm_perf/
  2025-07-lab/
    neoverse-b01/
      raw/
      aggregated.json
      run-log.md
    neoverse-b02/
      …
    xeon-d01/
    xeon-d02/
```
- `run-log.md` מתעד את ה-hash של הפקודה, גרסת git, אופרטור וכל חריגות.
- קבצי JSON מצטברים מוזנים ישירות לעדכוני הבסיס ומצורפים לסקירת הביצועים ב-`docs/source/crypto/sm_perf_baseline_comparison.md`.
- QA Guild סוקרת את החפצים לפני שינוי קווי היסוד וחותמת ב-`status.md` תחת סעיף הביצועים.### 5. ציר הזמן של CI Gating
| תאריך | אבן דרך | פעולה |
|------|--------|--------|
| 2025-07-12 | לכידת Neoverse הושלמה | עדכן קבצי `sm_perf_baseline_aarch64_*` JSON, הפעל את `ci/check_sm_perf.sh` באופן מקומי, פתח יחסי ציבור עם חפצי אמנות מצורפים. |
| 24-07-2025 | לכידת x86_64 הושלמה | הוסף קבצי בסיס חדשים + שער ב-`ci/check_sm_perf.sh`; ודא שנתיבי CI חוצי קשת צורכים אותם. |
| 27-07-2025 | אכיפת CI | אפשר את זרימת העבודה `sm-perf-gate` לפעול בשתי מחלקות המארחים; מיזוגים נכשלים אם הרגרסיות חורגות מהסובלנות המוגדרות. |

### 6. תלות ותקשורת
- תיאום שינויים בגישה למעבדה באמצעות `infra-ops@iroha.tech`.  
- Performance WG מפרסם עדכונים יומיים בערוץ `#perf-lab` בזמן הפעלת לכידות.  
- QA Guild מכין את ההבדל ההשוואה (`scripts/sm_perf_compare.py`) כדי שהבודקים יוכלו לדמיין דלתות.  
- לאחר מיזוג קווי הבסיס, עדכן את `roadmap.md` (SM-4c.1a/b, SM-5a.3b) ואת `status.md` עם הערות השלמת לכידה.

עם תוכנית זו, עבודת האצת ה-SM משיגה חציונים ניתנים לשחזור, שער CI ושביל ראיות שניתן לעקוב אחריהם, המספקים את פריט הפעולה "חלונות מעבדה עתודה ואמצעי לכידה".

### 7. שער CI ועשן מקומי

- `ci/check_sm_perf.sh` היא נקודת הכניסה הקנונית של CI. הוא מוציא ל-`scripts/sm_perf.sh` עבור כל מצב ב-`SM_PERF_MODES` (ברירת המחדל הוא `scalar auto neon-force`) ומגדיר את `CARGO_NET_OFFLINE=true` כך שספסלי עבודה יפעלו באופן דטרמיניסטי על תמונות ה-CI.  
- `.github/workflows/sm-neon-check.yml` קורא כעת לשער ב-macOS arm64 runner כך שכל בקשת משיכה מפעילה את שלישיית הכוח הסקלרית/אוטומטית/ניאון באמצעות אותו עוזר בשימוש מקומי; מסלול ה-Linux/Neoverse המשלים יתחבר ברגע שה-x86_64 יתפוס קרקע וקווי הבסיס של Neoverse פרוקסי יתרעננו עם ריצת המתכת החשופה.  
- מפעילים יכולים לעקוף את רשימת המצבים באופן מקומי: `SM_PERF_MODES="scalar" bash ci/check_sm_perf.sh` מקצץ את הריצה למעבר יחיד לבדיקת עשן מהירה, בעוד ארגומנטים נוספים (לדוגמה `--tolerance 0.20`) מועברים ישירות ל-`scripts/sm_perf.sh`.  
- `make check-sm-perf` עוטף כעת את השער לנוחות המפתחים; משרות CI יכולות להפעיל את הסקריפט ישירות בזמן שמפתחי macOS מתגייסים לאחור על יעד הביצוע.  
- ברגע שקווי הבסיס של Neoverse/x86_64 נוחתים, אותו סקריפט יקלוט את ה-JSON המתאים דרך לוגיית הזיהוי האוטומטי של המארח שכבר קיים ב-`scripts/sm_perf.sh`, כך שאין צורך בחיווט נוסף בזרימות העבודה מעבר להגדרת רשימת המצבים הרצויה לכל מאגר מארח.

### 8. עוזר רענון רבעוני- הפעל את `scripts/sm_perf_quarterly.sh --owner "<name>" --cpu-label "<label>" [--quarter YYYY-QN] [--output-root artifacts/sm_perf]` כדי להטביע ספרייה עם רבע חותמת כגון `artifacts/sm_perf/2026-Q1/<label>/`. העוזר עוטף את `scripts/sm_perf_capture_helper.sh --matrix` ופולט `capture_commands.sh`, `capture_plan.json`, ו-`quarterly_plan.json` (בעלים + מטא נתונים ברבעון) כך שמפעילי מעבדה יכולים לתזמן ריצות ללא תוכניות בכתב יד.
- בצע את ה-`capture_commands.sh` שנוצר על מארח היעד, צבור את התפוקות הגולמיות עם `scripts/sm_perf_aggregate.py --output <dir>/aggregated.json`, וקדם את החציונים לתוך ה-JSONs הבסיסי באמצעות `scripts/sm_perf_promote_baseline.py --out-dir crates/iroha_crypto/benches --overwrite`. הפעל מחדש את `ci/check_sm_perf.sh` כדי לוודא שהסובלנות נשארות ירוקות.
- כאשר חומרה או שרשראות כלים משתנות, רענן סובלנות/הערות השוואה ב-`docs/source/crypto/sm_perf_baseline_comparison.md`, הדק את סובלנות `ci/check_sm_perf.sh` אם החציונים החדשים מתייצבים, ויישר את כל ספי לוח המחוונים/התראה עם קווי הבסיס החדשים כדי שאזעקות הפעלה יישארו משמעותיות.
- Commit `quarterly_plan.json`, `capture_plan.json`, `capture_commands.sh` וה-JSON המצטבר לצד עדכוני הבסיס; צרף את אותם חפצים לעדכוני הסטטוס/מפת הדרכים לצורך מעקב.
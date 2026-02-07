---
lang: he
direction: rtl
source: docs/source/dev_workflow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3be11bea2cb39520bc3c09f4614555d4ac97760e3590609e2f8df27bf28d1a1a
source_last_modified: "2026-01-18T17:14:31.034360+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# זרימת עבודה לפיתוח סוכנים

ספר הפעלה זה מאחד את מעקות הבטיחות של התורמים ממפת הדרכים של AGENTS כך
תיקונים חדשים עוקבים אחר אותם שערי ברירת מחדל.

## יעדי התחלה מהירה

- הפעל את `make dev-workflow` (עטיפה סביב `scripts/dev_workflow.sh`) כדי לבצע:
  1. `cargo fmt --all`
  2. `cargo clippy --workspace --all-targets --locked -- -D warnings`
  3. `cargo build --workspace --locked`
  4. `cargo test --workspace --locked`
  5. `swift test` מ-`IrohaSwift/`
- `cargo test --workspace` פועל לאורך זמן (לעתים קרובות שעות). לאיטרציות מהירות,
  השתמש ב-`scripts/dev_workflow.sh --skip-tests` או `--skip-swift`, ולאחר מכן הפעל את כל
  רצף לפני המשלוח.
- אם `cargo test --workspace` נתקע על מנעולים של ספריית build, הפעל מחדש עם
  `scripts/dev_workflow.sh --target-dir target/codex-tests` (או סט
  `CARGO_TARGET_DIR` לנתיב מבודד) כדי למנוע מחלוקת.
- כל שלבי המטען משתמשים ב-`--locked` כדי לכבד את מדיניות המאגר של שמירה
  `Cargo.lock` לא נגע. העדיפו להרחיב ארגזים קיימים במקום להוסיף
  חברי חלל עבודה חדשים; לבקש אישור לפני הכנסת ארגז חדש.

## מעקות בטיחות- `make check-agents-guardrails` (או `ci/check_agents_guardrails.sh`) נכשל אם
  סניף משנה את `Cargo.lock`, מציג חברים בסביבת עבודה חדשים או מוסיף חדשים
  תלות. הסקריפט משווה בין עץ העבודה לבין `HEAD`
  `origin/main` כברירת מחדל; הגדר את `AGENTS_BASE_REF=<ref>` כדי לעקוף את הבסיס.
- `make check-dependency-discipline` (או `ci/check_dependency_discipline.sh`)
  מבדל את התלות `Cargo.toml` מול הבסיס ונכשל בארגזים חדשים; להגדיר
  `DEPENDENCY_DISCIPLINE_ALLOW=<dep1,dep2>` כדי להכיר בכוונה
  תוספות.
- `make check-missing-docs` (או `ci/check_missing_docs_guard.sh`) בלוקים חדשים
  כניסות `#[allow(missing_docs)]`, דגלים נגעו בארגזים (הקרובים ביותר `Cargo.toml`)
  ש-`src/lib.rs`/`src/main.rs` שלו חסרים מסמכי `//!` ברמת הארגז, ודוחה חדש
  פריטים ציבוריים ללא מסמכי `///` ביחס ל-Ref הבסיס; להגדיר
  `MISSING_DOCS_GUARD_ALLOW=1` רק עם אישור הבודק. גם השומר
  מאמת ש-`docs/source/agents/missing_docs_inventory.{json,md}` טריים;
  להתחדש עם `python3 scripts/inventory_missing_docs.py`.
- `make check-tests-guard` (או `ci/check_tests_guard.sh`) מסמנים ארגזים שלהם
  פונקציות חלודה שהשתנו חסרות ראיות לבדיקת יחידה. מפות השומרים שינו קווים
  לפונקציות, עובר אם בדיקות הארגז השתנו ב-diff, ואחרות סריקות
  קבצי בדיקה קיימים להתאמת קריאות פונקציה ולכן כיסוי קיים מראש
  סופרים; ארגזים ללא מבחנים תואמים ייכשלו. סט `TEST_GUARD_ALLOW=1`
  רק כאשר השינויים הם באמת ניטרליים למבחן והמבקר מסכים.
- `make check-docs-tests-metrics` (או `ci/check_docs_tests_metrics_guard.sh`)
  אוכפת את מדיניות מפת הדרכים שאבני דרך נעות לצד תיעוד,
  בדיקות, ומדדים/דשבורדים. כאשר `roadmap.md` משתנה ביחס ל
  `AGENTS_BASE_REF`, השומר מצפה לפחות שינוי מסמך אחד, שינוי בדיקה אחד,
  ושינוי מדד/טלמטריה/לוח מחוונים אחד. סט `DOC_TEST_METRIC_GUARD_ALLOW=1`
  רק באישור המבקר.
- `make check-todo-guard` (או `ci/check_todo_guard.sh`) נכשל כאשר סמני TODO
  להיעלם ללא שינויים נלווים במסמכים/בדיקות. הוסף או עדכן כיסוי
  בעת פתרון TODO, או הגדר את `TODO_GUARD_ALLOW=1` להסרות מכוונות.
- `make check-std-only` (או `ci/check_std_only.sh`) בלוקים `no_std`/`wasm32`
  cfgs כך שמרחב העבודה יישאר `std` בלבד. הגדר `STD_ONLY_GUARD_ALLOW=1` רק עבור
  ניסויי CI מאושרים.
- `make check-status-sync` (או `ci/check_status_sync.sh`) שומר על מפת הדרכים פתוחה
  חלק ללא פריטים שהושלמו ודורש `roadmap.md`/`status.md` כדי
  שנה יחד כך שהתוכנית/סטטוס יישארו מיושרים; להגדיר
  `STATUS_SYNC_ALLOW_UNPAIRED=1` רק עבור תיקוני שגיאות הקלדה נדירים לאחר
  הצמדת `AGENTS_BASE_REF`.
- `make check-proc-macro-ui` (או `ci/check_proc_macro_ui.sh`) מריץ את ה-Tybuild
  חבילות ממשק משתמש לארגזי נגזרת/פרוק-מאקרו. הפעל אותו כאשר נוגעים ב-proc-macros to
  לשמור על אבחון `.stderr` יציב ולתפוס רגרסיות של ממשק משתמש מבהלה; להגדיר
  `PROC_MACRO_UI_CRATES="crate1 crate2"` להתמקד בארגזים ספציפיים.
- בנייה מחדש של `make check-env-config-surface` (או `ci/check_env_config_surface.sh`)
  מלאי ה-env-toggle (`docs/source/agents/env_var_inventory.{json,md}`),
  נכשל אם הוא מיושן, **ו** נכשל כאשר מופיעים shims חדש של עטיפות ייצור
  ביחס ל-`AGENTS_BASE_REF` (זוהה אוטומטית; הוגדר במפורש בעת הצורך).
  רענן את הגשש לאחר הוספה/הסרה של חיפושי Env באמצעות
  `python3 scripts/inventory_env_toggles.py --json docs/source/agents/env_var_inventory.json --md docs/source/agents/env_var_inventory.md`;
  השתמש ב-`ENV_CONFIG_GUARD_ALLOW=1` רק לאחר תיעוד כפתורי Env מכווניםבמעקב ההגירה.
- `make check-serde-guard` (או `ci/check_serde_guard.sh`) מחדש את השרת
  מלאי שימוש (`docs/source/norito_json_inventory.{json,md}`) לטמפ'
  מיקום, נכשל אם המלאי המחויב מיושן, ודוחה כל חדש
  ייצור `serde`/`serde_json` פגיעות ביחס ל-`AGENTS_BASE_REF`. סט
  `SERDE_GUARD_ALLOW=1` רק עבור ניסויי CI לאחר הגשת תוכנית הגירה.
- `make guards` אוכפת את מדיניות הסדרת Norito: היא מכחישה חדשות
  שימוש ב-`serde`/`serde_json`, עוזרי AoS אד-הוק ותלות ב-SCALE בחוץ
  ספסלי Norito (`scripts/deny_serde_json.sh`,
  `scripts/check_no_direct_serde.sh`, `scripts/deny_handrolled_aos.sh`,
  `scripts/check_no_scale.sh`).
- **מדיניות ממשק המשתמש של Proc-macro:** כל ארגז proc-macro חייב לשלוח `trybuild`
  רתמה (`tests/ui.rs` עם גלובס עובר/נכשל) מאחורי ה-`trybuild-tests`
  תכונה. הנח דוגמאות של נתיב שמח תחת `tests/ui/pass`, מקרי דחייה תחת
  `tests/ui/fail` עם יציאות `.stderr` מחויבות, ולשמור על אבחון
  לא בפאניקה ויציבה. רענון גופי עם
  `TRYBUILD=overwrite cargo test -p <crate> -F trybuild-tests` (אופציונלי עם
  `CARGO_TARGET_DIR=target-codex` כדי להימנע מהשחתת מבנים קיימים) ו
  הימנע מהסתמכות על בניית כיסוי (צפויים שומרים `cfg(not(coverage))`).
  עבור פקודות מאקרו שאינן פולטות נקודת כניסה בינארית, העדיפו
  `// compile-flags: --crate-type lib` במתקנים כדי לשמור על ממוקד שגיאות. הוסף
  מקרים שליליים חדשים בכל פעם שהאבחון משתנה.
- CI מריץ את תסריטי מעקה הבטיחות דרך `.github/workflows/agents-guardrails.yml`
  אז בקשות משיכה נכשלות במהירות כאשר המדיניות מופרת.
- ה-Git Hook לדוגמה (`hooks/pre-commit.sample`) מפעיל מעקה בטיחות, תלות,
  סקריפטים חסרים-מסמכים, std-only, env-config ו-status-sync כך שתורמים
  לתפוס הפרות מדיניות לפני CI. שמור פירורי לחם של TODO לכל מכוון
  מעקבים במקום לדחות שינויים גדולים בשקט.
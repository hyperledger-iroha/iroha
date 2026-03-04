---
lang: he
direction: rtl
source: CONTRIBUTING.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 71baf5d038cbe6518fd294fcc1b279dff8aaf092e4a83f6159b699a378e51467
source_last_modified: "2025-12-08T10:55:43+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# מדריך תורם

תודה שהקדשת מזמנך לתרום ל-Iroha 2!

אנא קרא את המדריך הזה כדי ללמוד כיצד אתה יכול לתרום ואילו קווים מנחים אנו מצפים ממך לפעול. זה כולל את ההנחיות לגבי קוד ותיעוד, כמו גם המוסכמות שלנו לגבי זרימת עבודה של git.

קריאת הנחיות אלו תחסוך לך זמן מאוחר יותר.

## כיצד אוכל לתרום?

ישנן דרכים רבות שתוכלו לתרום לפרויקט שלנו:

- דווח על [באגים](#reporting-bugs) ו-[חולשות](#reporting-vulnerabilities)
- [הצע שיפורים](#suggesting-improvements) והטמיע אותם
- [שאל שאלות](#asking-questions) וצור קשר עם הקהילה

חדש בפרויקט שלנו? [תרום את התרומה הראשונה שלך](#your-first-code-contribution)!

### TL;DR

- מצא את [ZenHub](https://app.zenhub.com/workspaces/iroha-v2-60ddb820813b9100181fc060/board?repos=181739240).
- מזלג [Iroha](https://github.com/hyperledger-iroha/iroha/tree/main).
- תקן את בעיית הבחירה שלך.
- הקפד לעקוב אחר [מדריכי הסגנון](#style-guides) שלנו לקוד ותיעוד.
- כתוב [בדיקות](https://doc.rust-lang.org/cargo/commands/cargo-test.html). ודא שכולם עוברים (`cargo test --workspace`). אם אתה נוגע בערימת ההצפנה SM, הפעל גם את `cargo test -p iroha_crypto --features "sm sm_proptest"` כדי לבצע את רתמת ה-fuzz/רכוש האופציונלית.
  - הערה: בדיקות המפעילות את ה-executor IVM יסנתזו אוטומטית קוד ביצוע מינימלי ודטרמיניסטי אם `defaults/executor.to` אינו קיים. לא נדרש שלב מקדים להפעלת בדיקות. כדי ליצור את קוד הבתים הקנוני עבור זוגיות, אתה יכול להריץ:
    - `cargo run --manifest-path scripts/generate_executor_to/Cargo.toml`
    - `cargo run --manifest-path scripts/regenerate_codec_samples/Cargo.toml`
- אם תשנה ארגזי נגזר/פרוק-מאקרו, הפעל את חבילות ממשק המשתמש trybuild באמצעות
  `make check-proc-macro-ui` (או
  `PROC_MACRO_UI_CRATES="crate1 crate2" make check-proc-macro-ui`) ורענן
  `.stderr` מתקנים כאשר האבחון משתנה כדי לשמור על יציבות הודעות.
- הפעל את `make dev-workflow` (עטיפה סביב `scripts/dev_workflow.sh`) כדי לבצע fmt/clippy/build/test עם `--locked` בתוספת `swift test`; צפו מ-`cargo test --workspace` שייקח שעות והשתמש ב-`--skip-tests` רק עבור לולאות מקומיות מהירות. ראה `docs/source/dev_workflow.md` עבור ספר ההפעלה המלא.
- לאכוף מעקות בטיחות עם `make check-agents-guardrails` כדי לחסום עריכות `Cargo.lock` וארגזי סביבת עבודה חדשים, `make check-dependency-discipline` כדי להיכשל בתלות חדשה אלא אם כן מותר במפורש, ו-`make check-missing-docs` כדי למנוע פספוסים חדשים של I18NI3400, Chim מסמכים על ארגזים שנגעו בהם, או פריטים ציבוריים חדשים ללא הערות למסמך (השומר מרענן את `docs/source/agents/missing_docs_inventory.{json,md}` דרך `scripts/inventory_missing_docs.py`). הוסף את `make check-tests-guard` כדי שהפונקציות שהשתנו ייכשלו אלא אם כן בדיקות יחידה מתייחסות אליהן (בלוקים מוטבעים של `#[cfg(test)]`/`#[test]` או ארגז `tests/`; ספירות כיסוי קיימות) ו-`make check-docs-tests-metrics` נבדקו כך, עם שינויים ב-Doc, עם מפת הדרכים. מדדים/לוחות מחוונים. שמור על אכיפה של TODO באמצעות `make check-todo-guard` כדי שסמני TODO לא יופלו ללא מסמכים/בדיקות נלווים. `make check-env-config-surface` מייצר מחדש את מלאי ה-env-toggle וכעת נכשל כאשר מופיעים shims **ייצור** חדשים ביחס ל-`AGENTS_BASE_REF`; הגדר את `ENV_CONFIG_GUARD_ALLOW=1` רק לאחר תיעוד תוספות מכוונות במעקב ההגירה. `make check-serde-guard` מרענן את המלאי המסור ונכשל בצילומי מצב מיושנים או ייצור חדש של `serde`/`serde_json`; הגדר `SERDE_GUARD_ALLOW=1` רק עם תוכנית הגירה מאושרת. שמור על דחיות גדולות גלויות באמצעות פירורי לחם וכרטיסי מעקב של TODO במקום לדחות בשקט. הפעל את `make check-std-only` כדי לתפוס את `no_std`/`wasm32` cfgs ו-`make check-status-sync` כדי להבטיח `roadmap.md` פריטים פתוחים יישארו פתוחים בלבד וששינויי מפת הדרכים/סטטוס נוחתים יחד; הגדר את `STATUS_SYNC_ALLOW_UNPAIRED=1` רק לתיקוני שגיאות הקלדה נדירים בסטטוס בלבד לאחר הצמדת `AGENTS_BASE_REF`. עבור קריאה בודדת, השתמש ב-`make agents-preflight` כדי להפעיל את כל מעקות הבטיחות יחד.
- הפעל משמרות סריאליזציה מקומיות לפני דחיפה: `make guards`.
  - זה שולל את `serde_json` ישיר בקוד הייצור, מונע שירותים ישירים חדשים מחוץ לרשימת ההיתרים ומונע עוזרים אד-הוק AoS/NCB מחוץ ל-`crates/norito`.
- אופציונלי הפעלה יבשה של Norito מטריצת תכונות מקומית: `make norito-matrix` (משתמש בתת-קבוצה מהירה).
  - לכיסוי מלא, הפעל את `scripts/run_norito_feature_matrix.sh` ללא `--fast`.
  - לכלול עשן במורד הזרם לכל משולבת (ארגז ברירת מחדל `iroha_data_model`): `make norito-matrix-downstream` או `scripts/run_norito_feature_matrix.sh --fast --downstream [crate]`.
- עבור ארגזי proc-macro, הוסף רתמת ממשק משתמש `trybuild` (`tests/ui.rs` + `tests/ui/pass`/`tests/ui/fail`) ובצע אבחון `.stderr` למקרים הכושלים. שמור על אבחון יציב וללא פאניקה; לרענן גופי עם `TRYBUILD=overwrite cargo test -p <crate> -F trybuild-tests` ולשמור עליהם עם `cfg(all(feature = "trybuild-tests", not(coverage)))`.
- בצע שגרה מראש כמו עיצוב וחידוש חפצים (ראה [`pre-commit.sample`](./hooks/pre-commit.sample))
- כאשר ה-`upstream` מוגדר למעקב [מאגר Hyperledger Iroha](https://github.com/hyperledger-iroha/iroha), `git pull -r upstream main`, I18NI0000017800, I18NIcreate a [משוך ו-[משוך] בקשה](https://github.com/hyperledger-iroha/iroha/compare) לסניף `main`. ודא שהוא עומד ב-[pull request guidelines](#pull-request-etiquette).

### התחלה מהירה של זרימת עבודה של AGENTS

- הפעל את `make dev-workflow` (עטיפה סביב `scripts/dev_workflow.sh`, מתועדת ב-`docs/source/dev_workflow.md`). הוא עוטף את `cargo fmt --all`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo build/test --workspace --locked` (בדיקות יכולות להימשך מספר שעות), ו-`swift test`.
- השתמש ב-`scripts/dev_workflow.sh --skip-tests` או `--skip-swift` עבור איטרציות מהירות יותר; הפעל מחדש את הרצף המלא לפני פתיחת בקשת משיכה.
- מעקות בטיחות: הימנע מנגיעה ב-`Cargo.lock`, הוספת חברי סביבת עבודה חדשים, הצגת תלות חדשה, הוספת shims חדשים `#[allow(missing_docs)]`, השמטת מסמכים ברמת הארגז, דילוג על בדיקות בעת שינוי פונקציות, הפלת סמני TODO ללא מסמכים/בדיקות מחדש, `no_std`/`wasm32` cfgs ללא אישור. הפעל את `make check-agents-guardrails` (או `AGENTS_BASE_REF=origin/main bash ci/check_agents_guardrails.sh`) בתוספת `make check-dependency-discipline`, `make check-missing-docs` (מרענן את `docs/source/agents/missing_docs_inventory.{json,md}`), `make check-tests-guard` (נכשל כאשר פונקציות הבדיקה הקיימות משתנות ביחידה או הוכחות קיימות בבדיקות הייצור משתנות הבדיקות חייבות להתייחס לפונקציה), `make check-docs-tests-metrics` (נכשל כאשר שינויים במפת הדרכים חסרים עדכוני מסמכים/בדיקות/מדדים), `make check-todo-guard`, `make check-env-config-surface` (נכשל במלאי מיושן או חילופי מקף ייצור חדשים; עוקף רק לאחר I108NI020doX ו-updates), `make check-serde-guard` (נכשל במלאי סרדי מיושן או כניסות חדשות לייצור; עוקף עם `SERDE_GUARD_ALLOW=1` רק עם תוכנית הגירה מאושרת) מקומית לאות מוקדם, `make check-std-only` לשומר הסטנדרטי בלבד, ולשמור את I1018NI0000X/I1018NI0000X/I1018NI0000X סנכרון עם `make check-status-sync` (הגדר `STATUS_SYNC_ALLOW_UNPAIRED=1` רק לתיקוני שגיאות הקלדה נדירים בסטטוס בלבד לאחר הצמדת `AGENTS_BASE_REF`). השתמש ב-`make agents-preflight` אם אתה רוצה פקודה אחת שתפעיל את כל הגארדים לפני פתיחת PR.

### דיווח על באגים

*באג* הוא שגיאה, פגם בתכנון, כשל או תקלה ב-Iroha שגורמת לו להפיק תוצאה או התנהגות לא נכונה, בלתי צפויה או לא מכוונת.

אנו עוקבים אחר באגים של Iroha באמצעות [GitHub Issues](https://github.com/hyperledger-iroha/iroha/issues?q=is%3Aopen+is%3Aissue+label%3ABug) המסומנים בתג `Bug`.

כאשר אתה יוצר בעיה חדשה, יש תבנית שתוכל למלא. להלן רשימת הבדיקה של מה עליך לעשות כאשר אתה מדווח על באגים:
- [ ] הוסף את התג `Bug`
- [ ] הסבר את הנושא
- [ ] ספק דוגמה עבודה מינימלית
- [ ] צרף צילום מסך

<details> <summary>דוגמה עבודה מינימלית</summary>

עבור כל באג, עליך לספק [דוגמה עבודה מינימלית](https://en.wikipedia.org/wiki/Minimal_working_example). לדוגמה:

```
# Minting negative Assets with value spec `Numeric`.

I was able to mint negative values, which shouldn't be possible in Iroha. This is bad because <X>.

# Given

I managed to mint negative values by running
<paste the code here>

# I expected

not to be able to mint negative values

# But, I got

<code showing negative value>

<paste a screenshot>
```

</details>

---
**הערה:** בעיות כמו תיעוד מיושן, תיעוד לא מספיק או בקשות לתכונות צריכות להשתמש בתוויות `Documentation` או `Enhancement`. הם לא באגים.

---

### דיווח על פגיעויות

למרות שאנו פועלים באופן יזום במניעת בעיות אבטחה, ייתכן שאתה עלול להיתקל בפגיעות אבטחה לפני שנעשה זאת.

- לפני המהדורה העיקרית הראשונה (2.0) כל הפגיעויות נחשבות לבאגים, אז אל תהסס לשלוח אותן בתור באגים [בעקבות ההוראות למעלה](#reporting-bugs).
- לאחר השחרור הגדול הראשון, השתמש ב[תוכנית הבאג באונטי] (https://hackerone.com/hyperledger) שלנו כדי להגיש נקודות תורפה ולקבל את הפרס שלך.

:קריאה: כדי למזער את הנזק שנגרם מפגיעת אבטחה שלא תוקנה, עליך לחשוף את הפגיעות ישירות ל-Hyperledger בהקדם האפשרי ו**להימנע מחשיפה של אותה פגיעות בפומבי** למשך פרק זמן סביר.

אם יש לך שאלות כלשהן בנוגע לטיפול שלנו בפרצות אבטחה, אנא אל תהסס ליצור קשר עם כל אחד מהמנהלים הפעילים כעת בהודעות פרטיות של Rocket.Chat.

### הצעות שיפורים

צור [בעיה](https://github.com/hyperledger-iroha/iroha/issues/new) ב-GitHub עם התגים המתאימים (`Optimization`, `Enhancement`) ותאר את השיפור שאתה מציע. אתה יכול להשאיר את הרעיון הזה לנו או למישהו אחר לפתח, או שאתה יכול ליישם אותו בעצמך.

אם אתה מתכוון ליישם את ההצעה בעצמך, בצע את הפעולות הבאות:

1. הקצה את הנושא שיצרת לעצמך **לפני** שתתחיל לעבוד עליו.
2. עבוד על התכונה שהצעת ופעל לפי [הנחיות לקוד ותיעוד](#style-guides).
3. כאשר אתה מוכן לפתוח בקשת משיכה, ודא שאתה פועל לפי [הנחיות בקשת משיכה](#pull-request-etiquette) וסמן אותה כמיישמת הבעיה שנוצרה קודם לכן:

   ```
   feat: Description of the feature

   Explanation of the feature

   Closes #1234
   ```

4. אם השינוי שלך דורש שינוי API, השתמש בתג `api-changes`.

   **הערה:** תכונות הדורשות שינויים ב-API עשויות להימשך זמן רב יותר ליישם ולאישור מכיוון שהן דורשות מיצרני ספריות Iroha לעדכן את הקוד שלהם.### שואל שאלות

שאלה היא כל דיון שהוא לא באג ולא תכונה או בקשת אופטימיזציה.

<פרטים> <סיכום> כיצד אוכל לשאול שאלה? </סיכום>

אנא פרסם את שאלותיך אל [אחת מפלטפורמות ההודעות המיידיות שלנו](#contact) כדי שהצוות וחברי הקהילה יוכלו לעזור לך בזמן.

אתה, כחלק מהקהילה הנ"ל, צריך לשקול לעזור גם לאחרים. אם תחליט לעזור, אנא עשה זאת [באופן מכובד](CODE_OF_CONDUCT.md).

</details>

## תרומת הקוד הראשונה שלך

1. מצא בעיה ידידותית למתחילים בין בעיות עם התווית [נושא טוב-ראשון](https://github.com/hyperledger-iroha/iroha/labels/good%20first%20issue).
2. ודא שאף אחד אחר לא עובד על הנושאים שבחרת על ידי בדיקה שזה לא מוקצה לאף אחד.
3. הקצה את הנושא לעצמך כדי שאחרים יוכלו לראות שמישהו עובד על זה.
4. קרא את [מדריך סגנון החלודה](#rust-style-guide) לפני שתתחיל לכתוב קוד.
5. כאשר אתה מוכן לבצע את השינויים שלך, קרא את [ההנחיות למשיכה של בקשה](#pull-request-etiquette).

## משוך בקשת נימוס

אנא [פורק](https://docs.github.com/en/get-started/quickstart/fork-a-repo) את [מאגר](https://github.com/hyperledger-iroha/iroha/tree/main) ו[צור ענף תכונה](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/proposing-changes-to-your-work-with-pull-requests/creating-and-deleting-branches-within-your-repository) עבור התרומות שלך. כאשר עובדים עם **מ"ש ממזלגות**, בדוק את [מדריך זה](https://help.github.com/articles/checking-out-pull-requests-locally).

#### עבודה על תרומת קוד:
- עקוב אחר [מדריך סגנון חלודה](#rust-style-guide) ו[מדריך סגנון תיעוד](#documentation-style-guide).
- ודא שהקוד שכתבת מכוסה במבחנים. אם תיקנת באג, אנא הפוך את דוגמה העבודה המינימלית שמשחזרת את הבאג לבדיקה.
- בעת נגיעה בארגזי נגזר/פרוק-מאקרו, הפעל את `make check-proc-macro-ui` (או
  מסנן עם `PROC_MACRO_UI_CRATES="crate1 crate2"`) אז נסה לבנות מכשירי ממשק משתמש
  הישאר מסונכרן והאבחון נשאר יציב.
- תיעוד ממשקי API ציבוריים חדשים (ברמת הארגז `//!` ו-`///` על פריטים חדשים), והפעל
  `make check-missing-docs` לאימות מעקה הבטיחות. קרא את המסמכים/הבדיקות שאתה
  נוסף בתיאור בקשת המשיכה שלך.

#### ביצוע העבודה שלך:
- עקוב אחר [מדריך סגנון Git](#git-workflow).
- סחוט את ההתחייבויות שלך [או לפני](https://www.git-tower.com/learn/git/faq/git-squash/) או [במהלך המיזוג](https://rietta.com/blog/github-merge-types/).
- אם במהלך הכנת בקשת המשיכה שלך הסניף שלך לא מעודכן, קבע אותו מחדש באופן מקומי עם `git pull --rebase upstream main`. לחלופין, תוכל להשתמש בתפריט הנפתח עבור כפתור `Update branch` ולבחור באפשרות `Update with rebase`.

  כדי להקל על התהליך הזה לכולם, השתדלו לא לקבל יותר מקומץ התחייבויות לבקשת משיכה, והימנעו משימוש חוזר בענפי תכונות.

#### יצירת בקשת משיכה:
- השתמש בתיאור בקשת משיכה מתאים על ידי ביצוע ההנחיות בסעיף [נימוס בקשת משיכה](#pull-request-etiquette). הימנע מסטייה מהנחיות אלה במידת האפשר.
- הוסף [משוך כותרת בקשה] בפורמט מתאים (#pull-request-titles).
- אם אתה מרגיש שהקוד שלך לא מוכן להתמזג, אבל אתה רוצה שהמנהלים יסתכלו דרכו, צור טיוטת בקשה למשיכה.

#### מיזוג העבודה שלך:
- בקשת משיכה חייבת לעבור את כל הבדיקות האוטומטיות לפני מיזוגה. לכל הפחות, הקוד חייב להיות בפורמט, לעבור את כל הבדיקות, כמו גם ללא מוך `clippy` מצטיינים.
- לא ניתן למזג בקשת משיכה ללא שתי ביקורות מאושרות מהמתחילים הפעילים.
- כל בקשת משיכה תודיע אוטומטית לבעלי הקוד. ניתן למצוא רשימה מעודכנת של מתחזקים נוכחיים ב-[MAINTAINERS.md](MAINTAINERS.md).

#### סקירת כללי התנהגות:
- אל תפתור שיחה בעצמך. תן למבקר לקבל החלטה.
- להכיר בהערות ביקורת וליצור קשר עם המבקר (להסכים, לא להסכים, להבהיר, להסביר וכו'). אל תתעלם מהערות.
- להצעות פשוטות לשינוי קוד, אם תחיל אותן ישירות, תוכל לפתור את השיחה.
- הימנע מהחלפת ההתחייבויות הקודמות שלך בעת דחיפה של שינויים חדשים. זה מטשטש את מה שהשתנה מאז הביקורת האחרונה ומאלץ את המבקר להתחיל מאפס. התחייבויות נמחקות לפני מיזוג אוטומטי.

### משוך כותרות בקשה

אנו מנתחים את הכותרות של כל בקשות המשיכה הממוזגות כדי ליצור יומני שינויים. אנו גם בודקים שהכותרת תואמת את המוסכמה באמצעות הסימון *`check-PR-title`*.

כדי לעבור את בדיקת *`check-PR-title`*, כותרת בקשת המשיכה חייבת לעמוד בהנחיות הבאות:

<details> <summary> הרחב כדי לקרוא את הנחיות הכותרת המפורטות</summary>

1. עקוב אחר פורמט [התחייבויות קונבנציונליות](https://www.conventionalcommits.org/en/v1.0.0/#commit-message-with-multi-paragraph-body-and-multiple-footers).

2. אם לבקשת המשיכה יש התחייבות בודדת, כותרת ה-PR צריכה להיות זהה להודעת ההתחייבות.

</details>

### זרימת עבודה של Git

- [Fork](https://docs.github.com/en/get-started/quickstart/fork-a-repo) את [מאגר](https://github.com/hyperledger-iroha/iroha/tree/main) ו-[צור ענף תכונה](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/proposing-changes-to-your-work-with-pull-requests/creating-and-deleting-branches-within-your-repository) עבור התרומות שלך.
- [הגדר את השלט](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/working-with-forks/configuring-a-remote-repository-for-a-fork) כדי לסנכרן את המזלג שלך עם מאגר [Hyperledger Iroha](https://github.com/hyperledger-iroha/iroha/tree/main).
- השתמש ב-[Git Rebase Workflow](https://git-rebase.io/). הימנע משימוש ב-`git pull`. השתמש ב-`git pull --rebase` במקום זאת.
- השתמש ב-[git hooks](./hooks/) המסופקים כדי להקל על תהליך הפיתוח.

פעל לפי הנחיות ההתחייבות הבאות:

- **חתום כל התחייבות**. אם לא, [DCO](https://github.com/apps/dco) לא יאפשר לך להתמזג.

  השתמש ב-`git commit -s` כדי להוסיף אוטומטית את `Signed-off-by: $NAME <$EMAIL>` כשורה האחרונה של הודעת ההתחייבות שלך. השם והדוא"ל שלך צריכים להיות זהים למצוין בחשבון GitHub שלך.

  אנו ממליצים לך גם לחתום על ההתחייבויות שלך עם מפתח GPG באמצעות `git commit -sS` ([למידע נוסף](https://docs.github.com/en/authentication/managing-commit-signature-verification/signing-commits)).

  אתה יכול להשתמש ב[קרס `commit-msg`](./hooks/) כדי לחתום אוטומטית על ההתחייבויות שלך.

- הודעות Commit חייבות לעקוב אחר [commits קונבנציונלי](https://www.conventionalcommits.org/en/v1.0.0/#commit-message-with-multi-paragraph-body-and-multiple-footers) ואת אותה סכימת שמות כמו עבור [pull request titles](#pull-request-titles). המשמעות היא:
  - **השתמש בזמן הווה** ("הוסף תכונה", לא "תכונה הוספה")
  - **השתמש במצב רוח חיוני** ("פרוס למעגן..." לא "פרוס למעגן...")
- כתוב הודעת התחייבות משמעותית.
- נסה לשמור על הודעת התחייבות קצרה.
- אם אתה צריך לקבל הודעת התחייבות ארוכה יותר:
  - הגבל את השורה הראשונה של הודעת ההתחייבות שלך ל-50 תווים או פחות.
  - השורה הראשונה בהודעת ההתחייבות שלך צריכה להכיל את סיכום העבודה שעשית. אם אתה צריך יותר משורה אחת, השאר שורה ריקה בין כל פסקה ותאר את השינויים שלך באמצע. השורה האחרונה חייבת להיות הסימון.
- אם תשנה את הסכימה (בדוק על ידי יצירת הסכימה עם `kagami schema` ו-diff), עליך לבצע את כל השינויים בסכימה ב-commit נפרד עם ההודעה `[schema]`.
- נסו לדבוק בהתחייבות אחת לכל שינוי משמעותי.
  - אם תיקנת מספר בעיות ב-PR אחד, תן להם התחייבויות נפרדות.
  - כפי שהוזכר קודם לכן, שינויים ב-`schema` וב-API צריכים להיעשות ב-commits מתאימות בנפרד משאר העבודה שלך.
  - הוסף בדיקות לפונקציונליות באותו commit כמו אותה פונקציונליות.

## בדיקות ואמות מידה

- כדי להפעיל את הבדיקות מבוססות קוד המקור, בצע את [`cargo test`](https://doc.rust-lang.org/cargo/commands/cargo-test.html) בשורש Iroha. שימו לב שזה תהליך ארוך.
- כדי להפעיל מדדים, בצע את [`cargo bench`](https://doc.rust-lang.org/cargo/commands/cargo-bench.html) מהשורש Iroha. כדי לסייע באיתור באגים בפלטי השוואת ביצועים, הגדר את משתנה הסביבה `debug_assertions` כך: `RUSTFLAGS="--cfg debug_assertions" cargo bench`.
- אם אתה עובד על רכיב מסוים, זכור שכאשר אתה מפעיל את `cargo test` ב-[סביבת עבודה](https://doc.rust-lang.org/cargo/reference/workspaces.html), הוא יריץ רק את הבדיקות עבור אותו סביבת עבודה, שבדרך כלל אינה כוללת [מבחני אינטגרציה](https://www.testingxperts.com/blog/what-is-integration-testing).
- אם ברצונך לבדוק את השינויים שלך ברשת מינימלית, ה-[`docker-compose.yml`](defaults/docker-compose.yml) המסופק יוצר רשת של 4 עמיתים Iroha במיכלי docker שניתן להשתמש בהם כדי לבדוק קונצנזוס והיגיון הקשור להפצת נכסים. אנו ממליצים ליצור אינטראקציה עם הרשת הזו באמצעות [`iroha-python`](https://github.com/hyperledger-iroha/iroha-python), או באמצעות Iroha הלקוח CLI הכלול.
- אין להסיר מבחנים כושלים. אפילו בדיקות שמתעלמות מהן יופעלו בצינור שלנו בסופו של דבר.
- אם אפשר, נא לסמן את הקוד שלך לפני ואחרי ביצוע השינויים שלך, שכן רגרסיה משמעותית בביצועים עלולה לשבור את ההתקנות של משתמשים קיימים.

### בדיקות משמר סידורי

הפעל את `make guards` כדי לאמת את מדיניות המאגר באופן מקומי:

- דחיית רשימת `serde_json` ישירה במקורות ייצור (עדיף `norito::json`).
- איסור תלות/ייבוא ​​ישיר של `serde`/`serde_json` מחוץ לרשימת ההיתרים.
- מנע מחדש של עוזרי AoS/NCB אד-הוק מחוץ ל-`crates/norito`.

### בדיקות ניפוי באגים

<details> <summary> הרחב כדי ללמוד כיצד לשנות את רמת היומן או לכתוב יומנים ל-JSON.</summary>

אם אחת הבדיקות שלך נכשלת, ייתכן שתרצה להפחית את רמת הרישום המקסימלית. כברירת מחדל, Iroha רושם רק הודעות ברמת `INFO`, אך שומר על היכולת לייצר גם יומנים ברמה `DEBUG` וגם `TRACE`. ניתן לשנות הגדרה זו באמצעות משתנה הסביבה `LOG_LEVEL` עבור בדיקות מבוססות קוד, או באמצעות נקודת הקצה `/configuration` באחד מהעמיתים ברשת פרוסה.אמנם יומנים המודפסים ב-`stdout` מספיקים, אך ייתכן שיהיה נוח יותר לייצר יומנים בפורמט `json` לקובץ נפרד ולנתח אותם באמצעות [node-bunyan](https://www.npmjs.com/package/bunyan) או [rust-bunyan](I18NU00000).

הגדר את משתנה הסביבה `LOG_FILE_PATH` למיקום מתאים כדי לאחסן את היומנים ולנתח אותם באמצעות החבילות שלעיל.

</details>

### איתור באגים באמצעות קונסולת טוקיו

<details> <summary> הרחב כדי ללמוד כיצד להדר את Iroha עם תמיכה במסוף טוקיו.</summary>

לפעמים זה עשוי להיות מועיל עבור ניפוי באגים לנתח משימות טוקיו באמצעות [tokio-console](https://github.com/tokio-rs/console).

במקרה זה עליך לקמפל את Iroha עם תמיכה בקונסולת טוקיו כך:

```bash
RUSTFLAGS="--cfg tokio_unstable" cargo build --features tokio-console
```

ניתן להגדיר יציאה למסוף טוקיו באמצעות פרמטר תצורה `LOG_TOKIO_CONSOLE_ADDR` (או משתנה סביבה).
שימוש במסוף טוקיו דורש שרמת היומן תהיה `TRACE`, ניתן להפעיל אותו באמצעות פרמטר תצורה או משתנה סביבה `LOG_LEVEL`.

דוגמה להפעלת Iroha עם תמיכה במסוף טוקיו באמצעות `scripts/test_env.sh`:

```bash
# 1. Compile Iroha
RUSTFLAGS="--cfg tokio_unstable" cargo build --features tokio-console
# 2. Run Iroha with TRACE log level
LOG_LEVEL=TRACE ./scripts/test_env.sh setup
# 3. Access Iroha. Peers will be available on ports 5555, 5556, ...
tokio-console http://127.0.0.1:5555
```

</details>

### יצירת פרופילים

<פרטים> <סיכום> הרחב כדי ללמוד כיצד ליצור פרופיל Iroha. </סיכום>

כדי לייעל את הביצועים, כדאי ליצור פרופיל Iroha.

בניית פרופילים דורשת כרגע שרשרת כלים לילית. כדי להכין אחד, הידור Iroha עם פרופיל `profiling` ותכונה באמצעות `cargo +nightly`:

```bash
RUSTFLAGS="-C force-frame-pointers=on" cargo +nightly -Z build-std build --target your-desired-target --profile profiling --features profiling
```

לאחר מכן הפעל את Iroha וצרף פרופיל לבחירתך ל-Iroha pid.

לחלופין, ניתן לבנות Iroha בתוך docker עם תמיכה בפרופילים ופרופיל Iroha בצורה זו.

```bash
docker build -f Dockerfile.glibc --build-arg="PROFILE=profiling" --build-arg='RUSTFLAGS=-C force-frame-pointers=on' --build-arg='FEATURES=profiling' --build-arg='CARGOFLAGS=-Z build-std' -t iroha:profiling .
```

למשל באמצעות perf (זמין רק בלינוקס):

```bash
# to capture profile
sudo perf record -g -p <PID>
# to analyze profile
sudo perf report
```

כדי להיות מסוגל לצפות בפרופיל של המבצע במהלך פרופיל Iroha, יש להרכיב את ה-executor ללא הפשטת סמלים.
ניתן לעשות זאת על ידי הפעלת:

```bash
# compile executor without optimizations
cargo run --bin kagami -- ivm build ./path/to/executor --out-file executor.to
```

עם תכונת פרופילים מופעלת Iroha חושף את נקודת הקצה לפסילת פרופילי pprof:

```bash
# profile Iroha for 30 seconds and download the profile data
curl host:port/debug/pprof/profile?seconds=30 -o profile.pb
# analyze profile in browser (required installed go)
go tool pprof -web profile.pb
```

</details>

## מדריכי סגנון

אנא עקוב אחר ההנחיות הבאות כאשר אתה תורם קוד לפרויקט שלנו:

### מדריך סגנון Git

:book: [קרא הנחיות git](#git-workflow)

### מדריך סגנון חלודה

<details> <summary> :book: קרא את הנחיות הקוד</summary>

- השתמש ב-`cargo fmt --all` (מהדורה 2024) לעיצוב קוד.

הנחיות קוד:

- אלא אם צוין אחרת, עיין ב[שיטות עבודה מומלצות לחלודה](https://github.com/mre/idiomatic-rust).
- השתמש בסגנון `mod.rs`. [מודולים בעלי שם עצמי](https://rust-lang.github.io/rust-clippy/master/) לא יעברו ניתוח סטטי, למעט בדיקות [`trybuild`](https://crates.io/crates/trybuild).
- השתמש במבנה מודולים של תחום ראשון.

  דוגמה: אל תעשה `constants::logger`. במקום זאת, הפוך את ההיררכיה, שים את האובייקט שעבורו הוא משמש ראשון: `iroha_logger::constants`.
- השתמש ב-[`expect`](https://learning-rust.github.io/docs/unwrap-and-expect/) עם הודעת שגיאה מפורשת או הוכחה לאי-טעות במקום `unwrap`.
- לעולם אל תתעלם משגיאה. אם אתה לא יכול `panic` ולא יכול לשחזר, זה לפחות צריך להיות מתועד ביומן.
- העדיפו להחזיר `Result` במקום `panic!`.
- קבץ פונקציונליות קשורה באופן מרחבי, רצוי בתוך מודולים מתאימים.

  לדוגמה, במקום שיהיה בלוק עם הגדרות `struct` ולאחר מכן `impl`s עבור כל מבנה בודד, עדיף שה-`impl`s יהיו קשורים לאותו `struct` לידו.
- הצהר לפני יישום: הצהרות וקבועים `use` בחלק העליון, בדיקות יחידה למטה.
- נסה להימנע מהצהרות `use` אם השם המיובא משמש פעם אחת בלבד. זה מקל על העברת הקוד שלך לקובץ אחר.
- אל תשתיק מוך `clippy` ללא הבחנה. אם כן, הסבר את הנימוקים שלך בהערה (או הודעת `expect`).
- העדיפו `#[outer_attribute]` עד `#![inner_attribute]` אם אחד מהם זמין.
- אם הפונקציה שלך לא משנה אף אחת מהכניסות שלה (והיא לא אמורה לשנות שום דבר אחר), סמן אותה כ-`#[must_use]`.
- הימנע מ-`Box<dyn Error>` אם אפשר (אנו מעדיפים הקלדה חזקה).
- אם הפונקציה שלך היא גטר/מגדיר, סמן אותה `#[inline]`.
- אם הפונקציה שלך היא בנאי (כלומר, היא יוצרת ערך חדש מפרמטרי הקלט וקוראת `default()`), סמן אותה `#[inline]`.
- הימנע מקשירת הקוד שלך למבני נתונים בטון; `rustc` חכם מספיק כדי להפוך `Vec<InstructionExpr>` ל-`impl IntoIterator<Item = InstructionExpr>` ולהיפך כשצריך.

הנחיות למתן שמות:
- השתמש רק במילים מלאות בשמות *ציבורי* מבנה, משתנה, שיטה, תכונה, קבוע ומודול. עם זאת, קיצורים מותרים אם:
  - השם הוא מקומי (למשל טיעוני סגירה).
  - השם מקוצר על ידי מוסכמות Rust (למשל `len`, `typ`).
  - השם הוא קיצור מקובל (למשל `tx`, `wsv` וכו'); עיין ב[מילון המונחים של הפרויקט](https://docs.iroha.tech/reference/glossary.html) לקיצורים קנוניים.
  - השם המלא היה מוצל על ידי משתנה מקומי (למשל `msg <- message`).
  - השם המלא היה הופך את הקוד למסורבל עם יותר מ-5-6 מילים (למשל `WorldStateViewReceiverTrait -> WSVRecvTrait`).
- אם תשנה מוסכמות שמות, ודא שהשם החדש שבחרת הוא _הרבה_ יותר ברור ממה שהיה לנו קודם.

הנחיות להערות:
- כשאתה כותב הערות שאינן דוקטורטות, במקום לתאר *מה* הפונקציה שלך עושה, נסה להסביר *למה* היא עושה משהו בצורה מסוימת. זה יחסוך לך ולסוקר זמן.
- אתה רשאי להשאיר סמני `TODO` בקוד כל עוד אתה מתייחס לבעיה שיצרת עבורו. אי יצירת בעיה פירושה שהיא לא מתמזגת.

אנו משתמשים בתלות מוצמדת. פעל לפי ההנחיות הבאות לניהול גרסאות:

- אם העבודה שלך תלויה בארגז מסוים, בדוק אם הוא לא הותקן כבר באמצעות [`cargo tree`](https://doc.rust-lang.org/cargo/commands/cargo-tree.html) (השתמש ב-`bat` או `grep`), ונסה להשתמש בגרסה הזו, במקום בגרסה האחרונה.
- השתמש בגרסה המלאה "X.Y.Z" ב-`Cargo.toml`.
- ספק בליטות גרסה ב-PR נפרד.

</details>

### מדריך סגנון תיעוד

<details> <summary> :book: קרא הנחיות תיעוד</summary>


- השתמש בפורמט [`Rust Docs`](https://doc.rust-lang.org/cargo/commands/cargo-doc.html).
- העדיפו את תחביר ההערות בשורה אחת. השתמש ב-`///` מעל מודולים מוטבעים וב-`//!` עבור מודולים מבוססי קבצים.
- אם אתה יכול לקשר למסמכים של מבנה/מודול/פונקציה, עשה זאת.
- אם אתה יכול לספק דוגמה לשימוש, עשה זאת. זה [הוא גם מבחן](https://doc.rust-lang.org/rustdoc/documentation-tests.html).
- אם פונקציה יכולה לשגות או להיכנס לפאניקה, הימנע מפעלים מודאליים. דוגמה: `Fails if disk IO fails` במקום `Can possibly fail, if disk IO happens to fail`.
- אם פונקציה יכולה לשגות או להיכנס לפאניקה מסיבה אחת, השתמש ברשימת תבליטים של תנאי כשל, עם גרסאות `Error` המתאימות (אם יש).
- פונקציות *עשות* דברים. השתמש במצב רוח חיוני.
- מבנים *הם* דברים. תגיע לנקודה. לדוגמה `Log level for reloading from the environment` עדיף על `This struct encapsulates the idea of logging levels, and is used for reloading from the environment`.
- למבנים יש שדות, שגם *הם* דברים.
- מודולים *מכילים* דברים, ואנחנו יודעים את זה. תגיע לנקודה. דוגמה: השתמש ב-`Logger-related traits.` במקום ב-`Module which contains logger-related logic`.


</details>

## איש קשר

חברי הקהילה שלנו פעילים ב:

| שירות | קישור |
|--------------|------------------------------------------------------------------------|
| StackOverflow | https://stackoverflow.com/questions/tagged/hyperledger-iroha |
| רשימת תפוצה | https://lists.lfdecentralizedtrust.org/g/iroha |
| טלגרם | https://t.me/hyperledgeriroha |
| דיסקורד | https://discord.com/channels/905194001349627914/905205848547155968 |

---
<!-- Hebrew translation of docs/source/references/ci_operations.md -->

---
lang: he
direction: rtl
source: docs/source/references/ci_operations.md
status: complete
translator: manual
---

<div dir="rtl">

# תפעול CI — מסלול Nightly Red-Team

## סקירה כללית

- **תהליך עבודה:** ‎`.github/workflows/nightly-red-team.yml`
- **לוח זמנים:** ריצה יומית בשעה ‎03:30‎ UTC (cron של GitHub Actions) עם אפשרות הפעלה ידנית דרך `workflow_dispatch`.
- **משטחי בדיקה:**  
  - רתימת ריליי קונצנזוס — `cargo test -p integration_tests extra_functional::unstable_network::block_after_genesis_is_synced -- --nocapture`
  - מסלול Torii למניפסטי חוזים — `cargo test -p integration_tests contracts::post_and_get_contract_manifest_via_torii -- --nocapture`
  - מגן הגבלת קצב של Torii — `cargo test -p iroha_torii handshake_rate_limited -- --nocapture`
- **תוצרים:** לוגים לכל עומס נכתבים תחת `artifacts/red-team/` ומועלים כארטיפקט Workflow בשם `red-team-logs`.
- **התמודדות עם כשל:** `scripts/report_red_team_failures.py` מעתיק את הלוגים, בונה תבנית Issue (משטח, מיתון, בעלים, תאריך יעד) ופותח Issue ב-GitHub כשאחד העומסים נופל.

## SLA

- **חלון טראיג'**: פחות מ-24 שעות. יש לשייך בעלים, לאשר את ה-issue ולהתחיל מיתון בתוך יום קלנדרי אחד.
- **רמז לבעלים:** הגדרת סוד רפוזיטורי `RED_TEAM_OWNER` עם אזכור צוות (למשל `@sora-org/red-team`) תדאג שהמדווח ייחס את ה-issue לקבוצה הנכונה. בהיעדר סוד משתמשים בברירת המחדל המוגדרת ב-Workflow.

## הוראות הרצה מחדש

1. גשו אל **Actions → Nightly Red Team**.
2. לחצו **Run workflow**, בחרו ענף (ברירת מחדל: `main`) ואשרו. אין צורך בפרמטרים נוספים; ההרצה משתמשת בפקודות המופיעות לעיל.
3. עקבו אחרי הריצה; לאחר שהמיתון מוזג, הריצו שוב כדי לוודא שהמסלול ירוק ורק אז סגרו את ה-issue.

## צ'קליסט טראיג' במקרה כשל

- בדקו את הלוגים שצורפו בארטיפקט `red-team-logs` (`consensus.log`, ‏`manifest.log`, ‏`torii-rate-limit.log`).
- עדכנו או החליפו את הבעלים ב-issue אם השיוך האוטומטי אינו מדויק.
- תעדו ב-issue את שורש התקלה ואת המיתון, והוסיפו קישורים ל-PRים רלוונטיים.
- לאחר שהפתרון מוזג, הריצו מחדש את ה-Workflow ידנית וסגרו את ה-issue רק כשהריצה עברה בהצלחה.

## לוחות מחוונים קשורים

- מדדי ה־CI והפריטיות של Swift מופיעים בלוחות המחוונים הייעודיים המתועדים
- ריצת ה-XCFramework ב-Buildkite מוגדרת בקובץ `.buildkite/xcframework-smoke.yml`, והסקריפט `scripts/ci/run_xcframework_smoke.sh` מפיק את טלמטריית `mobile_ci`. בחנו את ההערה `xcframework-smoke/report` אחרי כל ריצה וטפלו בתקלות לפי לוחות המחוונים. בנוסף, ההרנס רושם מטא-דאטה `ci/xcframework-smoke:<lane>:device_tag`, כך שניתן לקשור תקלות לליין המדויק (לדוגמה `iphone-sim`, ‏`strongbox`) ללא ניתוח של מחרוזות destination.
  ב־`references/ios_metrics.md`. ודאו שכשלי Buildkite שעולים שם מטופלים במקביל
  למסלול ה־red-team כדי לשמור על יישור בין ה־SDKים והמוכנות לשחרור.
- רוטציית פיקצ'רים לאנדרואיד מתבצעת דרך `make android-fixtures`
  (`scripts/android_fixture_regen.sh`) ומיד לאחר מכן `make android-fixtures-check`
  (`ci/check_android_fixtures.sh`). גם בריצות CI יש להקפיד על אותו סדר כדי לוודא
  שהמניפסטים וקבצי ה־`.norito` שנוצרו מחדש נבדקו לפני שמבצעים commit.

</div>
- בהרצת בדיקות האנדרואיד על JVM שולחני ללא Ed25519 מובנה, יש לצרף את BouncyCastle (`org.bouncycastle:bcprov`); ההרנס מספק ספק מדומה ל-CI, אך הוספת הספרייה שומרת על יישור בין הריצות המקומיות והסביבתיות.

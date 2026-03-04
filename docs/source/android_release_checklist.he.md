---
lang: he
direction: rtl
source: docs/source/android_release_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5ee3613b544a847953f5ec152092cb2fe1da35279c5482486513d6b8d6dddf02
source_last_modified: "2026-01-04T11:42:43.398592+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# רשימת שחרור אנדרואיד (AND6)

רשימת בדיקה זו לוכדת את השערים **AND6 — CI & Compliance Hardening**
`roadmap.md` (§עדיפות 5). זה מיישר את מהדורות ה-SDK של Android עם ה-Rust
שחרר את ציפיות ה-RFC על ידי פירוט משרות ה-CI, חפצי תאימות,
עדויות מעבדת מכשיר, וחבילות מקור שיש לצרף לפני GA,
רכבת LTS, או תיקון חם נעה קדימה.

השתמש במסמך זה יחד עם:

- `docs/source/android_support_playbook.md` - יומן שחרור, SLAs ו
  עץ הסלמה.
- `docs/source/android_runbook.md` - ספרי הפעלה שוטפים.
- `docs/source/compliance/android/and6_compliance_checklist.md` - ווסת
  מלאי חפצים.
- `docs/source/release_dual_track_runbook.md` - ניהול שחרור דו-מסלולי.

## 1. שערי במה במבט אחד

| במה | שערים נדרשים | עדות |
|-------|----------------|--------|
| **T−7 ימים (הקפאה מוקדמת)** | לילי `ci/run_android_tests.sh` ירוק למשך 14 ימים; `ci/check_android_fixtures.sh`, `ci/check_android_samples.sh` ו-`ci/check_android_docs_i18n.sh` עוברים; סריקות מוך/תלות בתור. | לוחות מחוונים של BuildKite, דוח הבדלים של רכיבים, תמונות מסך לדוגמה. |
| **T−3 ימים (קידום RC)** | אושרה הזמנת מכשיר-מעבדה; StrongBox attestation CI run (`scripts/android_strongbox_attestation_ci.sh`); חבילות רובולקטריות/מכשירים המופעלות על חומרה מתוזמנת; `./gradlew lintRelease ktlintCheck detekt dependencyGuard` נקי. | מטריצת מכשיר CSV, מניפסט חבילת אישורים, דוחות Gradle מאוחסנים תחת `artifacts/android/lint/<version>/`. |
| **T−1 יום (go/no-go)** | חבילת סטטוס עריכת טלמטריה רענון (`scripts/telemetry/check_redaction_status.py --write-cache`); חפצי תאימות עודכנו לפי `and6_compliance_checklist.md`; חזרת המקור הושלמה (`scripts/android_sbom_provenance.sh --dry-run`). | `docs/source/compliance/android/evidence_log.csv`, סטטוס טלמטריה JSON, יומן ריצה יבשה מקורה. |
| **T0 (גזירה GA/LTS)** | `scripts/publish_android_sdk.sh --dry-run` הושלם; מקור + SBOM חתום; שחרור רשימת ביקורת שיוצאה וצורפה לדקות go/no-go; `ci/sdk_sorafs_orchestrator.sh` עבודה עשן ירוק. | שחרר קבצי RFC מצורפים, חבילת Sigstore, חפצי אימוץ תחת `artifacts/android/`. |
| **T+1 יום (לאחר חתך)** | אומתה מוכנות לתיקון חם (`scripts/publish_android_sdk.sh --validate-bundle`); הבדלי לוח המחוונים נבדקו (`ci/check_android_dashboard_parity.sh`); חבילת ראיות שהועלתה ל-`status.md`. | ייצוא הבדל של לוח המחוונים, קישור לערך `status.md`, חבילת שחרור בארכיון. |

## 2. CI & Quality Gate Matrix| שער | פקודות / סקריפט | הערות |
|------|---------------------|------|
| יחידה + מבחני אינטגרציה | `ci/run_android_tests.sh` (עוטף את `ci/run_android_tests.sh`) | פולט `artifacts/android/tests/test-summary.json` + יומן בדיקה. כולל Norito בדיקות codec, תור, StrongBox fallback ו-Torii רתמת לקוח. חובה מדי לילה ולפני תיוג. |
| שוויון מתקן | `ci/check_android_fixtures.sh` (עוטף את `scripts/check_android_fixtures.py`) | מבטיחה שמתקני Norito מחודשים תואמים את הסט הקנוני של Rust; חבר את ה-JSON הפרש כאשר השער נכשל. |
| אפליקציות לדוגמא | `ci/check_android_samples.sh` | בונה `examples/android/{operator-console,retail-wallet}` ומאמת צילומי מסך מקומיים באמצעות `scripts/android_sample_localization.py`. |
| Docs/I18N | `ci/check_android_docs_i18n.sh` | שומרים README + התחלות מהירות מקומיות. הפעל שוב לאחר שהעריכות של מסמך נחתו בסניף השחרור. |
| זוגיות לוח המחוונים | `ci/check_android_dashboard_parity.sh` | מאשרת כי מדדי CI/מיוצאים מתאימים לעמיתיהם של Rust; נדרש במהלך אימות T+1. |
| SDK אימוץ עשן | `ci/sdk_sorafs_orchestrator.sh` | מפעיל את ריבוי המקורות של Sorafs orchestrator bindings עם ה-SDK הנוכחי. נדרש לפני העלאת חפצי אמנות מבוימים. |
| אימות אישור | `scripts/android_strongbox_attestation_ci.sh --summary-out artifacts/android/attestation/ci-summary.json` | אוסף את חבילות אישור StrongBox/TEE תחת `artifacts/android/attestation/**`; צרף את הסיכום למנות GA. |
| אימות חריץ התקן-מעבדה | `scripts/check_android_device_lab_slot.py --root artifacts/android/device_lab/<slot> --json-out artifacts/android/device_lab/summary.json` | מאמת חבילות מכשור לפני צירוף ראיות לשחרור מנות; CI פועל כנגד חריץ המדגם ב-`fixtures/android/device_lab/slot-sample` (טלמטריה/אישור/תור/יומנים + `sha256sum.txt`). |

> **טיפ:** הוסף את העבודות האלה לצינור `android-release` Buildkite כך
> הקפאת שבועות הפעל מחדש אוטומטית כל שער עם קצה סניף השחרור.

העבודה המאוחדת `.github/workflows/android-and6.yml` מפעילה את המוך,
בדיקות חבילת בדיקה, סיכום אישורים ובדיקות חריץ של מעבדת מכשירים בכל יחסי ציבור/דחיפה
נגיעה במקורות אנדרואיד, העלאת ראיות תחת `artifacts/android/{lint,tests,attestation,device_lab}/`.

## 3. סריקות מוך ותלות

הפעל את `scripts/android_lint_checks.sh --version <semver>` משורש הריפו. ה
הסקריפט מבצע:

```
lintRelease ktlintCheck detekt dependencyGuardBaseline \
:operator-console:lintRelease :retail-wallet:lintRelease
```

- דוחות ופלטי משמר תלות מאוחסנים תחת
  `artifacts/android/lint/<label>/` ו-`latest/` סימלינק לשחרור
  צינורות.
- ממצאי מוך כושלים דורשים תיקון או כניסה במהדורה
  RFC המתעד את הסיכון המקובל (אושר על ידי Release Engineering + Program
  עופרת).
- `dependencyGuardBaseline` מחדש את נעילת התלות; חבר את ההבדל
  לחבילת go/no-go.

## 4. כיסוי מעבדת מכשירים ו-StrongBox

1. הזמן מכשירי Pixel + Galaxy באמצעות מעקב הקיבולת המוזכר ב
   `docs/source/compliance/android/device_lab_contingency.md`. חוסם שחרורים
   אם ` כדי לרענן את דוח האישור.
3. הפעל את מטריצת המכשור (תעד את רשימת החבילות/ABI במכשיר
   גשש). לכוד כשלים ביומן האירועים גם אם ניסיונות חוזרים מצליחים.
4. הגש כרטיס אם נדרשת חזרה ל-Firebase Test Lab; לקשר את הכרטיס
   ברשימת הבדיקה למטה.

## 5. חפצי תאימות וטלמטריה- עקוב אחר `docs/source/compliance/android/and6_compliance_checklist.md` עבור האיחוד האירופי
  והגשות של JP. עדכון `docs/source/compliance/android/evidence_log.csv`
  עם hashes + כתובות אתרים לעבודות Buildkite.
- רענן ראיות לתיקון טלמטריה באמצעות
  `scripts/telemetry/check_redaction_status.py --write-cache \
   --status-url https://android-observability.example/status.json`.
  אחסן את ה-JSON שהתקבל תחת
  `artifacts/android/telemetry/<version>/status.json`.
- רשום את פלט הסכימה הבדל ממנו
  `scripts/telemetry/run_schema_diff.sh --android-config ... --rust-config ...`
  כדי להוכיח זוגיות עם יצואניות חלודה.

## 6. מקור, SBOM ופרסום

1. הפעל יבש את צינור הפרסום:

   ```bash
   scripts/publish_android_sdk.sh \
     --version <semver> \
     --repo-dir artifacts/android/maven/<semver> \
     --dry-run
   ```

2. צור SBOM + Sigstore מקור:

   ```bash
   scripts/android_sbom_provenance.sh \
     --version <semver> \
     --out artifacts/android/provenance/<semver>
   ```

3. צרף את `artifacts/android/provenance/<semver>/manifest.json` וחתום
   `checksums.sha256` ל-RFC המהדורה.
4. בעת קידום למאגר מייבן האמיתי, הפעל מחדש
   `scripts/publish_android_sdk.sh` ללא `--dry-run`, לכיד את הקונסולה
   יומן, והעלה את החפצים שנוצרו ל-`artifacts/android/maven/<semver>`.

## 7. תבנית מנות הגשה

כל מהדורת GA/LTS/תיקון חם צריכה לכלול:

1. **רשימת בדיקה שהושלמה** - העתק את הטבלה של קובץ זה, סמן כל פריט וקישור
   לחפצים תומכים (ריצת Buildkite, יומנים, הבדלי מסמכים).
2. **עדויות מעבדת מכשיר** - סיכום דוח אישור, יומן הזמנות, ו
   כל הפעלת מגירה.
3. **חבילת טלמטריה** - סטטוס עריכה JSON, schema diff, קישור אל
   עדכוני `docs/source/sdk/android/telemetry_redaction.md` (אם יש).
4. **חפצי תאימות** - ערכים שנוספו/עודכנו בתיקיית התאימות
   בתוספת יומן הראיות המרענן CSV.
5. **חבילת מקור** - חתימת SBOM, Sigstore ו-`checksums.sha256`.
6. **סיכום מהדורה** - סקירה של עמוד אחד מצורפת לסיכום `status.md`
   האמור לעיל (תאריך, גרסה, גולת הכותרת של שערים מוותרים).

אחסן את החבילה תחת `artifacts/android/releases/<version>/` ועיין בה
ב-`status.md` וב-RFC המהדורה.

- `scripts/run_release_pipeline.py --publish-android-sdk ...` באופן אוטומטי
  מעתיק את ארכיון המוך האחרון (`artifacts/android/lint/latest`) ואת
  ראיות תאימות היכנס ל-`artifacts/android/releases/<version>/` כך
  לחבילת הגשה יש תמיד מיקום קנוני.

---

**תזכורת:** עדכן את רשימת הבדיקה הזו בכל פעם שעבודות CI חדשות, חפצי תאימות,
או מתווספות דרישות טלמטריה. פריט מפת הדרכים AND6 נשאר פתוח עד ה-
רשימת התיוג והאוטומציה הנלווית מתגלים יציבים למשך שני שחרורים רצופים
רכבות.
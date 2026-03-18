---
lang: he
direction: rtl
source: docs/automation/android/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 27b5ac3c7adb19a87f0b3d076f3c9618b188602898ed3954808ac9f7a52b3a62
source_last_modified: "2025-11-15T13:38:36.954059+00:00"
translation_last_reviewed: 2026-01-21
---

# קו בסיס לאוטומציית תיעוד Android (AND5)

<div dir="rtl">

פריט AND5 במפת הדרכים דורש שאוטומציית תיעוד, לוקליזציה ופרסום תהיה נתונה
לביקורת לפני תחילת AND6 (CI & Compliance). תיקייה זו מתעדת את הפקודות,
התוצרים ומבנה הראיות שעליהם AND5/AND6 מסתמכים, בהתאם לתוכניות שב-
`docs/source/sdk/android/developer_experience_plan.md` ו-
`docs/source/sdk/android/parity_dashboard_plan.md`.

## תהליכים ופקודות

| משימה | פקודה(ות) | תוצרים צפויים | הערות |
|-------|-----------|----------------|-------|
| סנכרון stubs ללוקליזציה | `python3 scripts/sync_docs_i18n.py` (אפשר להעביר `--lang <code>` בכל ריצה) | קובץ לוג תחת `docs/automation/android/i18n/<timestamp>-sync.log` בתוספת קומיטים של stubs מתורגמים | שומר על `docs/i18n/manifest.json` מסונכרן עם ה-stubs המתורגמים; הלוג רושם קודי שפה שנגעו בהם ואת הקומיט שנכנס לקו הבסיס. |
| אימות fixtures + פריטי Norito | `ci/check_android_fixtures.sh` (עוטף `python3 scripts/check_android_fixtures.py --json-out artifacts/android/parity/<stamp>/summary.json`) | העתקת סיכום JSON שנוצר אל `docs/automation/android/parity/<stamp>-summary.json` | בודק payloads ב-`java/iroha_android/src/test/resources`, גיבובי מניפסטים ואורכי fixtures חתומים. צרפו את הסיכום יחד עם ראיות הקצב תחת `artifacts/android/fixture_runs/`. |
| מניפסט דוגמאות והוכחת פרסום | `scripts/publish_android_sdk.sh --version <semver> [--repo-url …]` (מריץ בדיקות + SBOM + provenance) | מטא-דאטה של חבילת provenance והקובץ `sample_manifest.json` מתוך `docs/source/sdk/android/samples/` המאוחסן ב-`docs/automation/android/samples/<version>/` | מחבר בין אפליקציות הדוגמה של AND5 לאוטומציית ההפצה: שמרו את המניפסט שנוצר, גיבוב ה-SBOM ולוג ה-provenance לצורך סקירת בטא. |
| הזנת דשבורד הפריטיות | `python3 scripts/check_android_fixtures.py … --json-out artifacts/android/parity/<stamp>/summary.json` ולאחר מכן `python3 scripts/android_parity_metrics.py --summary <summary> --output artifacts/android/parity/<stamp>/metrics.prom` | העתקת snapshot של `metrics.prom` או יצוא JSON מ-Grafana אל `docs/automation/android/parity/<stamp>-metrics.prom` | מזין את תוכנית הדשבורד כדי ש-AND5/AND7 יוכלו לבדוק מוני הגשות לא תקינות ואימוץ טלמטריה. |

## איסוף ראיות

1. **חתמו הכול בזמן.** תנו לקבצים שמות עם חותמות זמן UTC
   (`YYYYMMDDTHHMMSSZ`) כדי שדשבורדי הפריטיות, פרוטוקולי ממשל והמסמכים
   המפורסמים יפנו לאותו הרצה.
2. **ציינו קומיטים.** כל לוג צריך לכלול את ה-hash של הקומיט של ההרצה וכל
   קונפיגורציה רלוונטית (למשל `ANDROID_PARITY_PIPELINE_METADATA`). כאשר נדרשת
   השחרה בגלל פרטיות, הוסיפו הערה וקישור לכספת מאובטחת.
3. **ארכבו מינימום הקשר.** רק סיכומים מובנים (JSON, `.prom`, `.log`) נכנסים
   לריפו. תוצרים כבדים (חבילות APK, צילומי מסך) צריכים להישאר ב-`artifacts/` או
   באחסון אובייקטים עם hash חתום שמצוין בלוג.
4. **עדכון סטטוס.** כאשר אבני דרך AND5 מתקדמות ב-`status.md`, ציינו את הקובץ
   המתאים (לדוגמה,
   `docs/automation/android/parity/20260324T010203Z-summary.json`) כדי שמבקרים
   יוכלו לאתר את קו הבסיס בלי לחפש בלוגי CI.

עמידה במבנה הזה מספקת את הדרישה של AND6 ל"קווי בסיס docs/automation זמינים
לביקורת" ושומרת על תוכנית התיעוד לאנדרואיד מיושרת עם התוכניות שפורסמו.

</div>

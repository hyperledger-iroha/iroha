---
lang: he
direction: rtl
source: docs/source/compliance/android/device_lab_instrumentation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9d384e21d09f3c4f57b7fc5181d69dc0da739dd6ed4dcb89a57ea58fd29bb898
source_last_modified: "2026-01-03T18:07:59.259775+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# ווי מכשור למכשירי אנדרואיד (AND6)

הפניה זו סוגרת את פעולת מפת הדרכים "שלב את מעבדת המכשיר הנותר /
ווי מכשור לפני בעיטת AND6". זה מסביר איך כל שמור
חריץ מעבדה של מכשיר חייב ללכוד חפצי טלמטריה, תור ואישור, כך ש
רשימת תיוג AND6, יומן ראיות וחבילות ממשל חולקות אותו הדבר
זרימת עבודה דטרמיניסטית. שידוך הערה זו עם הליך ההזמנה
(`device_lab_reservation.md`) ופנקס ריצת הכשל בעת תכנון חזרות.

## יעדים והיקף

- **הוכחה דטרמיניסטית** - כל יציאות המכשור חיים מתחת
  `artifacts/android/device_lab/<slot-id>/` עם SHA-256 מתבטא כך אודיטורים
  יכול להפריד בין צרורות מבלי להפעיל מחדש את הבדיקות.
- **זרימת עבודה ראשונה בסקריפט** - שימוש חוזר בעוזרים הקיימים
  (`ci/run_android_telemetry_chaos_prep.sh`,
  `scripts/android_keystore_attestation.sh`, `scripts/android_override_tool.sh`)
  במקום פקודות adb מותאמות אישית.
- **רשימות הבדיקה נשארות מסונכרנות** - כל ריצה מפנה למסמך זה מה-
  רשימת תיוג AND6 ומצרפת את החפצים
  `docs/source/compliance/android/evidence_log.csv`.

## פריסת חפץ

1. בחר מזהה משבצת ייחודי התואם לכרטיס ההזמנה, למשל.
   `2026-05-12-slot-a`.
2. הזינו את המדריכים הסטנדרטיים:

   ```bash
   export ANDROID_DEVICE_LAB_SLOT=2026-05-12-slot-a
   export ANDROID_DEVICE_LAB_ROOT="artifacts/android/device_lab/${ANDROID_DEVICE_LAB_SLOT}"
   mkdir -p "${ANDROID_DEVICE_LAB_ROOT}"/{telemetry,attestation,queue,logs}
   ```

3. שמור כל יומן פקודות בתוך התיקיה התואמת (למשל.
   `telemetry/status.ndjson`, `attestation/pixel8pro.log`).
4. לכידת SHA-256 מופיעה ברגע שהחריץ נסגר:

   ```bash
   find "${ANDROID_DEVICE_LAB_ROOT}" -type f -print0 | sort -z \
     | xargs -0 shasum -a 256 > "${ANDROID_DEVICE_LAB_ROOT}/sha256sum.txt"
   ```

## מטריצת מכשור

| זרימה | פקודות | מיקום פלט | הערות |
|------|--------|----------------|-------|
| עריכת טלמטריה + חבילת סטטוס | `scripts/telemetry/check_redaction_status.py --status-url <collector> --json-out ${ANDROID_DEVICE_LAB_ROOT}/telemetry/status.ndjson` | `telemetry/status.ndjson`, `telemetry/status.log` | רוץ בתחילת ובסוף המשבצת; חבר את ה-CLI stdout ל-`status.log`. |
| תור ממתין + הכנה לכאוס | `ANDROID_PENDING_QUEUE_EXPORTS="pixel8=${ANDROID_DEVICE_LAB_ROOT}/queue/pixel8.bin" ci/run_android_telemetry_chaos_prep.sh --status-only` | `queue/*.bin`, `queue/*.json`, `queue/*.sha256` | מראות ScenarioD מ-`readiness/labs/telemetry_lab_01.md`; הארך את ה-env var עבור כל מכשיר בחריץ. |
| ביטול תקציר ספר חשבונות | `scripts/android_override_tool.sh digest --out ${ANDROID_DEVICE_LAB_ROOT}/telemetry/override_digest.json` | `telemetry/override_digest.json` | נדרש גם כאשר אין דרישות פעילות; להוכיח את מצב האפס. |
| אישור StrongBox / TEE | `scripts/android_keystore_attestation.sh --device pixel8pro-strongbox-a --out "${ANDROID_DEVICE_LAB_ROOT}/attestation/pixel8pro"` | `attestation/<device>/*.{json,zip,log}` | חזור על הפעולה עבור כל מכשיר שמור (התאימו את השמות ב-`android_strongbox_device_matrix.md`). |
| רגרסיה של אישור CI רתום | `scripts/android_strongbox_attestation_ci.sh --output "${ANDROID_DEVICE_LAB_ROOT}/attestation/ci"` | `attestation/ci/*` | לוכדת את אותן הראיות ש-CI מעלה; לכלול בריצות ידניות לסימטריה. |
| מוך / קו הבסיס של תלות | `ANDROID_LINT_SUMMARY_OUT="${ANDROID_DEVICE_LAB_ROOT}/logs/jdeps-summary.txt" make android-lint` | `logs/jdeps-summary.txt`, `logs/lint.log` | הפעל פעם אחת לכל חלון הקפאה; ציין את הסיכום בחבילות תאימות. |

## נוהל חריץ סטנדרטי1. **טרום טיסה (T-24h)** - אשר את ההזמנה של כרטיס ההתייחסות לכך
   לתעד, לעדכן את ערך מטריצת ההתקן ולזרז את שורש החפץ.
2. **במהלך המשבצת**
   - הפעל תחילה את חבילת הטלמטריה + פקודות ייצוא בתור. עוברים
     `--note <ticket>` עד `ci/run_android_telemetry_chaos_prep.sh` אז היומן
     מפנה למזהה האירוע.
   - הפעל את סקריפטי האישור לכל מכשיר. כאשר הרתמה מייצרת א
     `.zip`, העתק אותו לשורש החפץ והקלט את ה-Git SHA המודפס בכתובת
     סוף התסריט.
   - בצע את `make android-lint` עם נתיב הסיכום שנעקף גם אם CI
     כבר רץ; המבקרים מצפים ליומן לכל משבצת.
3. **אחרי הריצה**
   - צור `sha256sum.txt` ו-`README.md` (הערות בצורה חופשית) בתוך החריץ
     תיקיה המסכמת את הפקודות שבוצעו.
   - הוסף שורה ל-`docs/source/compliance/android/evidence_log.csv` עם ה-
     מזהה חריץ, נתיב מניפסט hash, הפניות ל-Buildkite (אם יש), והאחרון
     אחוז קיבולת התקן-מעבדה מיצוא יומן ההזמנה.
   - קשר את תיקיית החריצים בכרטיס `_android-device-lab`, ה-AND6
     רשימת בדיקה, ודוח שחרור `docs/source/android_support_playbook.md`.

## טיפול בכשלים והסלמה

- אם פקודה כלשהי נכשלת, לכוד את פלט stderr תחת `logs/` ופעל לפי
  סולם הסלמה ב-`device_lab_reservation.md` §6.
- חוסר תור או טלמטריה צריך לציין מיד את סטטוס העקיפה ב
  `docs/source/sdk/android/telemetry_override_log.md` ועיין במזהה החריץ
  כך שהממשל יכול לאתר את התרגיל.
- יש לרשום רגרסיות של אישורים
  `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md`
  עם סדרות ההתקן הכושלות ונתיבי החבילה שנרשמו למעלה.

## רשימת רשימת דיווחים

לפני סימון החריץ הושלם, ודא שההפניות הבאות מעודכנות:

- `docs/source/compliance/android/and6_compliance_checklist.md` - סמן את
  יש להשלים את שורת המכשור ולציין את מזהה החריץ.
- `docs/source/compliance/android/evidence_log.csv` - הוסף/עדכן את הערך באמצעות
  ה-hash וקריאת הקיבולת.
- כרטיס `_android-device-lab` - צרף קישורי אמנות ומזהי עבודה של Buildkite.
- `status.md` - כלול הערה קצרה בתקציר המוכנות הבא לאנדרואיד, כך
  קוראי מפת הדרכים יודעים איזו משבצת הניבה את העדויות האחרונות.

בעקבות תהליך זה שומרים על "ווי מעבדה + מכשור" של AND6
אבן דרך ניתנת לביקורת ומונעת סטייה ידנית בין הזמנה, ביצוע,
ודיווח.
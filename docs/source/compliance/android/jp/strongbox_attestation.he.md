---
lang: he
direction: rtl
source: docs/source/compliance/android/jp/strongbox_attestation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8b8cc2e9de0c4183b51d011f5106a62b212da620d628cfc3b1cb74fe500b95b2
source_last_modified: "2026-01-03T18:07:59.238062+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# StrongBox Attestation Evidence — פריסות ביפן

| שדה | ערך |
|-------|-------|
| חלון הערכה | 2026-02-10 – 2026-02-12 |
| מיקום החפץ | `artifacts/android/attestation/<device-tag>/<date>/` (פורמט חבילה לפי `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md`) |
| כלי לכידה | `scripts/android_keystore_attestation.sh`, `scripts/android_strongbox_attestation_ci.sh`, `scripts/android_strongbox_attestation_report.py` |
| סוקרים | מעבדת חומרה מובילה, תאימות ומשפט (JP) |

## 1. נוהל לכידה

1. בכל מכשיר הרשום במטריצת StrongBox, צור אתגר ותפוס את חבילת האישורים:
   ```bash
   adb shell am instrument -w \
     org.hyperledger.iroha.android/.attestation.CaptureStrongBoxInstrumentation
   scripts/android_keystore_attestation.sh \
     --bundle-dir artifacts/android/attestation/${DEVICE_TAG}/2026-02-12 \
     --trust-root trust-roots/google-strongbox.pem \
     --require-strongbox \
     --output artifacts/android/attestation/${DEVICE_TAG}/2026-02-12/result.json
   ```
2. העבר מטא נתונים של חבילה (`result.json`, `chain.pem`, `challenge.hex`, `alias.txt`) לעץ הראיות.
3. הפעל את עוזר ה-CI כדי לאמת מחדש את כל החבילות במצב לא מקוון:
   ```bash
   scripts/android_strongbox_attestation_ci.sh \
     --root artifacts/android/attestation
   scripts/android_strongbox_attestation_report.py \
     --input artifacts/android/attestation \
     --output artifacts/android/attestation/report_20260212.txt
   ```

## 2. סיכום מכשיר (2026-02-12)

| תג מכשיר | דגם / StrongBox | נתיב צרור | תוצאה | הערות |
|------------|----------------|--------|--------|-------|
| `pixel6-strongbox-a` | Pixel 6 / Tensor G1 | `artifacts/android/attestation/pixel6-strongbox-a/2026-02-12/result.json` | ✅ עבר (מגובה חומרה) | אתגר קשור, תיקון מערכת ההפעלה 2025-03-05. |
| `pixel7-strongbox-a` | Pixel 7 / Tensor G2 | `.../pixel7-strongbox-a/2026-02-12/result.json` | ✅ עבר | מועמד לנתיב CI ראשי; טמפרטורה בתוך המפרט. |
| `pixel8pro-strongbox-a` | Pixel 8 Pro / Tensor G3 | `.../pixel8pro-strongbox-a/2026-02-13/result.json` | ✅ עבר (מבחן חוזר) | רכזת USB-C הוחלפה; Buildkite `android-strongbox-attestation#221` תפס את החבילה החולפת. |
| `s23-strongbox-a` | Galaxy S23 / Snapdragon 8 Gen 2 | `.../s23-strongbox-a/2026-02-12/result.json` | ✅ עבר | פרופיל אישור Knox מיובא 2026-02-09. |
| `s24-strongbox-a` | Galaxy S24 / Snapdragon 8 Gen 3 | `.../s24-strongbox-a/2026-02-13/result.json` | ✅ עבר | פרופיל אישור Knox מיובא; נתיב CI כעת ירוק. |

תגי מכשיר ממפים ל-`docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`.

## 3. רשימת ביקורת

- [x] אמת את `result.json` מציג את `strongbox_attestation: true` ואת שרשרת האישורים לשורש מהימן.
- [x] אשר בתים של האתגר תואמים את Buildkite פועלים `android-strongbox-attestation#219` (סוויפ ראשוני) ו-`#221` (בדיקה חוזרת של Pixel 8 Pro + לכידת S24).
- [x] הפעל מחדש את לכידת Pixel 8 Pro לאחר תיקון החומרה (בעלים: Hardware Lab Lead, הושלם ב-2026-02-13).
- [x] השלם לכידת Galaxy S24 ברגע שמגיע אישור פרופיל Knox (בעלים: Device Lab Ops, הושלם ב-2026-02-13).

## 4. הפצה

- צרף סיכום זה בתוספת קובץ הטקסט האחרון של הדוח לחבילות תאימות של שותפים (רשימת תיוג FISC § תושבות נתונים).
- נתיבי צרור התייחסות בעת תגובה לביקורות הרגולטורים; אל תשדר אישורים גולמיים מחוץ לערוצים מוצפנים.

## 5. יומן שינויים

| תאריך | שנה | מחבר |
|------|--------|--------|
| 2026-02-12 | לכידת חבילת JP ראשונית + דוח. | Device Lab Ops |
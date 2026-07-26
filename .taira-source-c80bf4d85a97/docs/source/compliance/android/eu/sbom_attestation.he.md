---
lang: he
direction: rtl
source: docs/source/compliance/android/eu/sbom_attestation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7d7eb66e5ba171d5c06aefa06ba9bd3e866596bc4efdbe16cb594990f46b5cb7
source_last_modified: "2026-01-04T11:42:43.493867+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# SBOM ואישור מוצא - SDK של Android

| שדה | ערך |
|-------|-------|
| היקף | Android SDK (`java/iroha_android`) + אפליקציות לדוגמה (`examples/android/*`) |
| בעל זרימת עבודה | הנדסת שחרור (אלכסיי מורוזוב) |
| אימות אחרון | 2026-02-11 (Buildkite `android-sdk-release#4821`) |

## 1. זרימת עבודה של דור

הפעל את סקריפט העזר (נוסף עבור אוטומציה של AND6):

```bash
scripts/android_sbom_provenance.sh <sdk-version>
```

התסריט מבצע את הפעולות הבאות:

1. מבצע את `ci/run_android_tests.sh` ו-`scripts/check_android_samples.sh`.
2. מפעיל את מעטפת Gradle תחת `examples/android/` כדי לבנות רכיבי CycloneDX SBOM עבור
   `:android-sdk`, `:operator-console`, ו-`:retail-wallet` עם הציוד המצורף
   `-PversionName`.
3. מעתיק כל SBOM ל-`artifacts/android/sbom/<sdk-version>/` עם שמות קנוניים
   (`iroha-android.cyclonedx.json` וכו').

## 2. מקור וחתימה

אותו סקריפט חותם כל SBOM עם `cosign sign-blob --bundle <file>.sigstore --yes`
ופולט `checksums.txt` (SHA-256) בספריית היעד. הגדר את `COSIGN`
משתנה סביבה אם הבינארי חי מחוץ ל-`$PATH`. לאחר סיום התסריט,
רשום את נתיבי החבילה/הבדיקה בתוספת מזהה הריצה של Buildkite
`docs/source/compliance/android/evidence_log.csv`.

## 3. אימות

כדי לאמת SBOM שפורסם:

```bash
COSIGN_EXPERIMENTAL=1 cosign verify-blob \
  --bundle artifacts/android/sbom/${SDK_VERSION}/operator-console.cyclonedx.json.sigstore \
  --yes artifacts/android/sbom/${SDK_VERSION}/operator-console.cyclonedx.json
```

השווה את SHA הפלט לערך המופיע ב-`checksums.txt`. הבודקים גם מבדלים את ה-SBOM לעומת המהדורה הקודמת כדי להבטיח שדלתות התלות הן מכוונות.

## 4. תמונת מצב של עדות (2026-02-11)

| רכיב | SBOM | SHA-256 | חבילת Sigstore |
|-----------|------|--------|----------------|
| Android SDK (`java/iroha_android`) | `artifacts/android/sbom/0.9.0/iroha-android.cyclonedx.json` | `0fd522b78f9a43b5fd1d6c8ec8b2d980adff5d3c31e30c3c7e1f0f9d7f187a2d` | חבילה `.sigstore` מאוחסנת ליד SBOM |
| דוגמה למסוף המפעיל | `artifacts/android/sbom/0.9.0/operator-console.cyclonedx.json` | `e3e236350adcb5ee4c0a9a4a98c7166c308ebe1d2d5d9ec0a79251afd8c7e1e4` | `.sigstore` |
| מדגם ארנק קמעונאי | `artifacts/android/sbom/0.9.0/retail-wallet.cyclonedx.json` | `4d81352eec6b0f33811f87ec219a3f88949770b8c820035446880b1a1aaed1cc` | `.sigstore` |

*(Hashs שנלכדו מ-Buildkite Run `android-sdk-release#4821`; שחזרו באמצעות פקודת האימות למעלה.)*

## 5. עבודה יוצאת מן הכלל

- אוטומציה של שלבי SBOM + cosign בתוך צינור השחרור לפני GA.
- שיקוף SBOMs לדלי החפצים הציבוריים ברגע ש-AND6 מסמן את רשימת התיוג הושלמה.
- תיאום עם Docs כדי לקשר מיקומי הורדה של SBOM מתוך הערות פרסום הפונות לשותפים.
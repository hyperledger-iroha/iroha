---
lang: he
direction: rtl
source: docs/source/compliance/android/eu/legal_signoff_memo_2026-02.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: eb92b77765ced36213a0bde55581f29d59c262f398c658f35a1fb43a182fe296
source_last_modified: "2026-01-03T18:07:59.201100+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# AND6 EU Legal Sign-off Memo — 2026.1 GA (Android SDK)

## סיכום

- **הפצה / רכבת:** 2026.1 GA (Android SDK)
- **תאריך סקירה:** 2026-04-15
- **יועץ / מבקר:** סופיה מרטינס - ציות ומשפטים
- **היקף:** יעד אבטחה ETSI EN 319 401, סיכום GDPR DPIA, אישור SBOM, AND6 הוכחות מגירה של מעבדת מכשירים
- **כרטיסים משויכים:** `_android-device-lab` / AND6-DR-202602, AND6 מעקב ממשל (`GOV-AND6-2026Q1`)

## רשימת חפצים

| חפץ | SHA-256 | מיקום / קישור | הערות |
|--------|--------|----------------|-------|
| `security_target.md` | `385d17a55579d2b0b365e21090ee081ded79e44655690b2abfbf54068c9b55b0` | `docs/source/compliance/android/eu/security_target.md` | תואם את מזהי ההפצה של 2026.1 GA ודלתות מודל האיומים (תוספות Torii NRPC). |
| `gdpr_dpia_summary.md` | `8ef338a20104dc5d15094e28a1332a604b68bdcfef1ff82fea784d43fdbd10b5` | `docs/source/compliance/android/eu/gdpr_dpia_summary.md` | הפניות מדיניות טלמטריה AND7 (`docs/source/sdk/android/telemetry_redaction.md`). |
| `sbom_attestation.md` | `c2e0de176d4bb8c8e09329e2b9ee5dd93228d3f0def78225c1d8b777a5613f2d` | חבילת `docs/source/compliance/android/eu/sbom_attestation.md` + Sigstore (`android-sdk-release#4821`). | CycloneDX + מקור נבדק; תואם לעבודת Buildkite `android-sdk-release#4821`. |
| יומן ראיות | `0b2d2f9eddada06faa70620f608c3ad1ec38f378d2cbddc24b15d0a83fcc381d` | `docs/source/compliance/android/evidence_log.csv` (שורה `android-device-lab-failover-20260220`) | מאשר גיבוב חבילה שנתפס ביומן + תמונת מצב של קיבולת + הזנת תזכיר. |
| חבילת מגירה של מכשירי מעבדה | `faf32356dfc0bbca1459b14d75f3306ea1c10cb40f3180fe1758ac5105016f85` | `artifacts/android/device_lab_contingency/20260220-failover-drill/` | Hash נלקח מ-`bundle-manifest.json`; כרטיס AND6-DR-202602 מתועד מסירה לחוק/תאימות. |

## ממצאים וחריגים

- לא זוהו בעיות חסימה. חפצי אמנות מתאימים לדרישות ETSI/GDPR; שוויון טלמטריה AND7 צוין בסיכום DPIA ולא נדרשות הקלות נוספות.
- המלצה: עקוב אחר תרגיל DR-2026-05-Q2 המתוכנן (כרטיס AND6-DR-202605) וצרף את הצרור המתקבל ליומן הראיות לפני נקודת ביקורת הממשל הבאה.

## אישור

- **החלטה:** אושרה
- **חתימה / חותמת זמן:** _סופיה מרטינס (חתומה דיגיטלית באמצעות פורטל ממשל, 15-04-2026 14:32 UTC)_
- **בעלי מעקב:** Device Lab Ops (לספק חבילת ראיות DR-2026-05-Q2 לפני 2026-05-31)
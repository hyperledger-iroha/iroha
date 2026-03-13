---
lang: he
direction: rtl
source: docs/source/compliance/android/jp/fisc_controls_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2d8b4c90c94dddd8118fcb9c55f07c25000c6dab1f8d239570402023ab89e844
source_last_modified: "2026-01-03T18:07:59.237724+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# רשימת בקרות אבטחה של FISC - Android SDK

| שדה | ערך |
|-------|-------|
| גרסה | 0.1 (2026-02-12) |
| היקף | Android SDK + כלי מפעיל בשימוש בפריסות פיננסיות ביפנית |
| בעלים | ציות ומשפט (דניאל פארק), מוביל תוכנית אנדרואיד |

## מטריצת בקרה

| בקרת FISC | פירוט יישום | ראיות / הפניות | סטטוס |
|-------------|-----------------------|----------------------|--------|
| **שלמות תצורת המערכת** | `ClientConfig` אוכף גיבוב מניפסט, אימות סכימה וגישה לקריאה בלבד בזמן ריצה. כשלי טעינה מחדש של תצורה פולטות אירועי `android.telemetry.config.reload` המתועדים ב-Runbook. | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`; `docs/source/android_runbook.md` §1–2. | ✅ מיושם |
| **בקרת גישה ואימות** | SDK מכבד את מדיניות Torii TLS ובקשות חתומות `/v2/pipeline`; הפניה לזרימות עבודה של מפעיל מדריך תמיכה §4–5 עבור הסלמה בתוספת ביטול שער באמצעות חפצי Norito חתומים. | `docs/source/android_support_playbook.md`; `docs/source/sdk/android/telemetry_redaction.md` (עקוף זרימת עבודה). | ✅ מיושם |
| **ניהול מפתחות קריפטוגרפיים** ​​| ספקים מועדפים של StrongBox, אימות אישור וכיסוי מטריצת מכשירים מבטיחים תאימות ל-KMS. יציאות רתמת האישורים מאוחסנות תחת `artifacts/android/attestation/` ונוקבות במטריצת המוכנות. | `docs/source/sdk/android/key_management.md`; `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`; `scripts/android_strongbox_attestation_ci.sh`. | ✅ מיושם |
| **רישום, ניטור ושימור** | מדיניות עריכת הטלמטריה מבצעת גיבוב של נתונים רגישים, מרכזת את תכונות המכשיר ואוכפת שמירה (חלונות של 30/7/90/365 ימים). Support Playbook §8 מתאר ספי לוח המחוונים; עקיפות שנרשמו ב-`telemetry_override_log.md`. | `docs/source/sdk/android/telemetry_redaction.md`; `docs/source/android_support_playbook.md`; `docs/source/sdk/android/telemetry_override_log.md`. | ✅ מיושם |
| **ניהול תפעול ושינויים** ​​| נוהל חיתוך של GA (Support Playbook §7.2) בתוספת `status.md` עדכוני מסלול מוכנות לשחרור. עדויות שחרור (SBOM, חבילות Sigstore) מקושרות באמצעות `docs/source/compliance/android/eu/sbom_attestation.md`. | `docs/source/android_support_playbook.md`; `status.md`; `docs/source/compliance/android/eu/sbom_attestation.md`. | ✅ מיושם |
| **תגובה ודיווח על אירועים** | Playbook מגדיר מטריצת חומרה, חלונות תגובה של SLA ושלבי הודעות תאימות; עקיפת טלמטריה + חזרות כאוס מבטיחות שחזור לפני טייסים. | `docs/source/android_support_playbook.md` §§4–9; `docs/source/sdk/android/telemetry_chaos_checklist.md`. | ✅ מיושם |
| **תושבות נתונים / לוקליזציה** | אספני טלמטריה עבור פריסות JP המופעלות באזור טוקיו המאושר; חבילות אישורים של StrongBox המאוחסנות באזור והפנייה מכרטיסי שותפים. תוכנית לוקליזציה מבטיחה שמסמכים זמינים ביפנית לפני בטא (AND5). | `docs/source/android_support_playbook.md` §9; `docs/source/sdk/android/developer_experience_plan.md` §5; `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`. | 🈺 בתהליך (לוקליזציה נמשכת) |

## הערות סוקר

- אמת את ערכי המטריצה של המכשיר עבור Galaxy S23/S24 לפני כניסת שותפים מוסדר (ראה שורות מסמך מוכנות `s23-strongbox-a`, `s24-strongbox-a`).
- ודא שקולטי טלמטריה בפריסות JP אוכפים את אותו היגיון שמירה/עקיפה שהוגדר ב-DPIA (`docs/source/compliance/android/eu/gdpr_dpia_summary.md`).
- קבל אישור מבקרים חיצוניים לאחר ששותפים בנקאיים בודקים רשימת בדיקה זו.
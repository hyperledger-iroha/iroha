---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/repair-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 302b74b4022656e57c2b876a8f15bf5301a593030a18ad1b93780061e5d783ef
source_last_modified: "2026-01-21T19:17:13.232211+00:00"
translation_last_reviewed: 2026-02-07
id: repair-plan
title: SoraFS Repair Automation & Auditor API
sidebar_label: Repair Automation
description: Governance policy, escalation lifecycle, and API expectations for SoraFS repair automation.
translator: machine-google-reviewed
---

:::הערה מקור קנוני
מראות `docs/source/sorafs_repair_plan.md`. שמור את שתי הגרסאות מסונכרנות עד שהסט ספינקס יפרוש.
:::

## מחזור חיים של החלטות ממשל
1. תיקונים מורחבים יוצרים טיוטת הצעת חתך ופותחים את חלון המחלוקת.
2. מצביעי ממשל מגישים הצבעות אישור/דחיה במהלך חלון המחלוקת.
3. ב-`escalated_at_unix + dispute_window_secs` ההחלטה מחושבת באופן דטרמיניסטי: מינימום מצביעים, אישורים עולים על דחיות, ויחס האישורים עומד בסף המניין.
4. החלטות מאושרות פותחות חלון ערעור; ערעורים שנרשמו לפני `approved_at_unix + appeal_window_secs` מסמנים את ההחלטה כערעור.
5. תקרות ענישה חלות על כל ההצעות; הגשות מעל המכסה נדחות.

## מדיניות הסלמה בממשל
מדיניות ההסלמה מקורה ב-`governance.sorafs_repair_escalation` ב-`iroha_config` והיא נאכפת עבור כל הצעת תיקון.

| הגדרה | ברירת מחדל | המשמעות |
|--------|--------|--------|
| `quorum_bps` | 6667 | יחס אישור מינימלי (נקודות בסיס) בין הקולות שנספרו. |
| `minimum_voters` | 3 | המספר המינימלי של בוחרים מובהקים הנדרש להכרעה. |
| `dispute_window_secs` | 86400 | זמן לאחר ההסלמה לפני סיום ההצבעות (שניות). |
| `appeal_window_secs` | 604800 | זמן לאחר האישור שבמהלכו ערעורים מתקבלים (שניות). |
| `max_penalty_nano` | 1,000,000,000 | עונש חתך מרבי מותר עבור הסלמות תיקון (nano-XOR). |

- הצעות שנוצרו על ידי מתזמן מוגבלות ל-`max_penalty_nano`; הגשות מבקר מעל הגבול נדחות.
- רשומות ההצבעה מאוחסנות ב-`repair_state.to` עם סדר דטרמיניסטי (מיון `voter_id`) כך שכל הצמתים מפיקים את אותה חותמת זמן ותוצאה של החלטה.
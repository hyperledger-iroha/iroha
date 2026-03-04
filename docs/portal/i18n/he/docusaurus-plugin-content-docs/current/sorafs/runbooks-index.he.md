---
lang: he
direction: rtl
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/sorafs/runbooks-index.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: eed1c1063d29e029a237e9c5c209772c1e297ab5ab057a57d97d8508d1c6c88c
source_last_modified: "2026-01-03T18:07:59+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: runbooks-index
lang: he
direction: rtl
source: docs/portal/docs/sorafs/runbooks-index.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: אינדקס ראנבוקים למפעילים
sidebar_label: אינדקס ראנבוקים
description: נקודת כניסה קנונית לראנבוקים של מפעילי SoraFS שעברו מיגרציה.
---

> משקף את רישום הבעלים שנמצא תחת `docs/source/sorafs/runbooks/`.
> כל מדריך תפעול חדש של SoraFS חייב להיות מקושר כאן ברגע שהוא מתפרסם בבילד של הפורטל.

השתמשו בדף זה כדי לבדוק אילו ראנבוקים השלימו את ההעברה מעץ המסמכים הישן אל הפורטל.
בכל רשומה מצוינים הבעלות, נתיב המקור הקנוני והעותק בפורטל כך שהסוקרים יוכלו לקפוץ
ישר למדריך הרצוי במהלך תצוגת הבטא.

## מארח תצוגת הבטא

גל DocOps כבר קידם את מארח תצוגת הבטא שאושר על‑ידי הסוקרים בכתובת `https://docs.iroha.tech/`.
כאשר מפנים מפעילים או סוקרים לראנבוק שעבר מיגרציה, השתמשו בשם המארח הזה כדי שיעבדו עם
צילום הפורטל המוגן בבדיקת checksum. נהלי פרסום/rollback נמצאים ב־
[`devportal/preview-host-exposure`](../devportal/preview-host-exposure.md).

| ראנבוק | בעלים | עותק בפורטל | מקור |
|--------|-------|-------------|------|
| השקה של Gateway ו‑DNS | Networking TL, Ops Automation, Docs/DevRel | [`sorafs/gateway-dns-runbook`](./gateway-dns-runbook.md) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
| פלייבוק תפעול SoraFS | Docs/DevRel | [`sorafs/operations-playbook`](./operations-playbook.md) | `docs/source/sorafs/operations_playbook.md` |
| התאמת קיבולת | Treasury / SRE | [`sorafs/capacity-reconciliation`](./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
| תפעול רישום הפינים | Tooling WG | [`sorafs/pin-registry-ops`](./pin-registry-ops.md) | `docs/source/sorafs/pin_registry_ops.md` |
| רשימת בדיקה לתפעול צמתים | Storage Team, SRE | [`sorafs/node-operations`](./node-operations.md) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
| ראנבוק סכסוכים וביטולים | Governance Council | [`sorafs/dispute-revocation-runbook`](./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
| פלייבוק מניפסטים ב‑staging | Docs/DevRel | [`sorafs/staging-manifest-playbook`](./staging-manifest-playbook.md) | `docs/source/sorafs/staging_manifest_playbook.md` |
| תצפית על עוגן Taikai | Media Platform WG / DA Program / Networking TL | [`sorafs/taikai-anchor-runbook`](./taikai-anchor-runbook.md) | `docs/source/taikai_anchor_monitoring.md` |

## רשימת בדיקה לאימות

- [x] בילד הפורטל מקשר לאינדקס הזה (פריט בסרגל הצד).
- [x] כל ראנבוק שעבר מיגרציה מציין את נתיב המקור הקנוני כדי לשמור על תיאום הסוקרים במהלך
  ביקורות המסמכים.
- [x] צינור התצוגה המקדימה של DocOps חוסם מיזוגים כאשר ראנבוק שמופיע ברשימה חסר בפלט הפורטל.

מיגרציות עתידיות (למשל תרגילי כאוס חדשים או נספחי ממשל) צריכות להוסיף שורה לטבלה למעלה
ולעדכן את רשימת הבדיקה של DocOps המוטמעת ב־`docs/examples/docs_preview_request_template.md`.

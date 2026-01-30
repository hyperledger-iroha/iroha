---
lang: he
direction: rtl
source: docs/portal/docs/soranet/gar-operator-onboarding.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 74b0ef4843c441003cd6630f35e0deac4a736adad450270047a739c1b1d0a6fc
source_last_modified: "2025-11-21T13:08:42.404970+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: Onboarding למפעילי GAR
sidebar_label: Onboarding GAR
description: צ'קליסט להפעלת מדיניות compliance של SNNet-9 עם digests של attestations ואיסוף ראיות.
---

השתמשו ב-brief זה כדי לפרוס את תצורת ה-compliance של SNNet-9 בתהליך שחוזר על עצמו וידידותי לאודיט. שלבו אותו עם סקירת תחום השיפוט כדי שכל מפעיל ישתמש באותם digests ובאותו מבנה ראיות.

## שלבים

1. **הרכבת תצורה**
   - יבאו את `governance/compliance/soranet_opt_outs.json`.
   - שלבו את `operator_jurisdictions` עם digests של attestation שפורסמו
     ב-[סקירת תחום השיפוט](gar-jurisdictional-review).
2. **ולידציה**
   - `cargo test -p sorafs_orchestrator -- compliance_policy_parses_from_json`
   - `cargo test -p sorafs_orchestrator -- compliance_example_config_parses`
   - אופציונלי: `cargo xtask soranet-privacy-report --max-suppression-ratio 0.2 --ndjson <privacy-log.ndjson>`
3. **לכידת ראיות**
   - לשמור תחת `artifacts/soranet/compliance/<YYYYMMDD>/`:
     - `config.json` (בלוק compliance סופי)
     - `attestations.json` (URIs + digests)
     - לוגי ולידציה
     - הפניות ל-PDFs/Norito envelopes חתומים
4. **הפעלה**
   - תייגו את ה-rollout (`gar-opt-out-<date>`), פרסו מחדש את תצורות orchestrator/SDK,
     ואשרו שאירועי `compliance_*` מופיעים בלוגים הצפויים.
5. **סגירה**
   - הגישו את חבילת הראיות ל-Governance Council.
   - רשמו את חלון ההפעלה והמאשרים ב-GAR logbook.
   - קבעו תאריכי סקירה הבאים לפי טבלת סקירת תחום השיפוט.

## צ'קליסט מהיר

- [ ] `jurisdiction_opt_outs` תואם לקטלוג הקנוני.
- [ ] Digests של attestation הועתקו בדיוק.
- [ ] פקודות ולידציה הופעלו ונשמרו.
- [ ] חבילת ראיות נשמרה ב-`artifacts/soranet/compliance/<date>/`.
- [ ] תיוג rollout ו-GAR logbook עודכנו.
- [ ] תזכורות לסקירה הבאה הוגדרו.

## ראו גם

- [GAR Jurisdictional Review](gar-jurisdictional-review)
- [GAR Compliance Playbook (source)](../../../source/soranet/gar_compliance_playbook.md)

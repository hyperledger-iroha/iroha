---
lang: he
direction: rtl
source: docs/source/ministry/impact_assessment_tooling.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 89be62d7bb2bb79fd994d207489d310ef4c997be53447fbee8ac1f7b758d3beb
source_last_modified: "2026-01-03T18:07:57.641039+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# כלי להערכת השפעה (MINFO-4b)

התייחסות למפת הדרכים: **MINFO‑4b — כלי להערכת השפעה.**  
בעלים: מועצת ממשל / אנליטיקה

הערה זו מתעדת את הפקודה `cargo xtask ministry-agenda impact` שעכשיו
מייצר את ה-Hash-family Diff האוטומטי הנדרש עבור מנות משאל עם. ה
הכלי צורך הצעות מועצת אג'נדה מאומתות, את הרישום הכפול, ו
תמונת מצב של דחייה/מדיניות אופציונלית כדי שהבודקים יוכלו לראות בדיוק איזה
טביעות אצבע חדשות, שמתנגשות במדיניות הקיימת, וכמה ערכים
כל משפחת חשיש תורמת.

## כניסות

1. **הצעות לסדר יום.** קובץ אחד או יותר שאחריו
   [`docs/source/ministry/agenda_council_proposal.md`](agenda_council_proposal.md).
   העבר אותם במפורש עם `--proposal <path>` או כוון את הפקודה ל-a
   ספרייה דרך `--proposal-dir <dir>` וכל קובץ `*.json` תחת הנתיב הזה
   כלול.
2. **רישום כפול (אופציונלי).** התאמה של קובץ JSON
   `docs/examples/ministry/agenda_duplicate_registry.json`. קונפליקטים הם
   דווח תחת `source = "duplicate_registry"`.
3. **תמונת מצב של מדיניות (אופציונלי).** מניפסט קל משקל שמפרט כל
   טביעת אצבע כבר נאכפת על ידי מדיניות GAR/משרד. המעמיס מצפה ל
   סכימה המוצגת להלן (ראה
   [`docs/examples/ministry/policy_snapshot_example.json`](../../examples/ministry/policy_snapshot_example.json)
   לדוגמא מלאה):

```json
{
  "snapshot_id": "denylist-2026-03",
  "generated_at": "2026-03-31T12:00:00Z",
  "entries": [
    {
      "hash_family": "blake3-256",
      "hash_hex": "…",
      "policy_id": "denylist-2025-014-entry-01",
      "note": "Already quarantined by GAR case CSAM-2025-014."
    }
  ]
}
```

כל ערך שטביעת האצבע שלו `hash_family:hash_hex` תואמת ליעד הצעה הוא
דווח תחת `source = "policy_snapshot"` עם ה-`policy_id` המוזכר.

## שימוש

```bash
cargo xtask ministry-agenda impact \
  --proposal docs/examples/ministry/agenda_proposal_example.json \
  --registry docs/examples/ministry/agenda_duplicate_registry.json \
  --policy-snapshot docs/examples/ministry/policy_snapshot_example.json \
  --out artifacts/ministry/impact/AC-2026-001.json
```

ניתן לצרף הצעות נוספות באמצעות דגלי `--proposal` חוזרים או על ידי
אספקת ספרייה המכילה אצווה שלמה של משאל עם:

```bash
cargo xtask ministry-agenda impact \
  --proposal-dir artifacts/ministry/proposals/2026-03-31 \
  --registry state/agenda_duplicate_registry.json \
  --out artifacts/ministry/impact/2026-03-31.json
```

הפקודה מדפיסה את ה-JSON שנוצר ל-stdout כאשר `--out` מושמט.

## פלט

הדוח הוא חפץ חתום (רשום אותו תחת חבילת משאל העם
`artifacts/ministry/impact/`) עם המבנה הבא:

```json
{
  "format_version": 1,
  "generated_at": "2026-03-31T12:34:56Z",
  "totals": {
    "proposals_analyzed": 4,
    "targets_analyzed": 17,
    "registry_conflicts": 2,
    "policy_conflicts": 1,
    "hash_families": [
      { "hash_family": "blake3-256", "targets": 12, "registry_conflicts": 2, "policy_conflicts": 0 },
      { "hash_family": "sha256", "targets": 5, "registry_conflicts": 0, "policy_conflicts": 1 }
    ]
  },
  "proposals": [
    {
      "proposal_id": "AC-2026-001",
      "action": "add-to-denylist",
      "total_targets": 2,
      "source_path": "docs/examples/ministry/agenda_proposal_example.json",
      "hash_families": [
        { "hash_family": "blake3-256", "targets": 2, "registry_conflicts": 1, "policy_conflicts": 0 }
      ],
      "conflicts": [
        {
          "source": "duplicate_registry",
          "hash_family": "blake3-256",
          "hash_hex": "0d714bed…1338d",
          "reference": "AC-2025-014",
          "note": "Already quarantined."
        }
      ],
      "registry_conflicts": 1,
      "policy_conflicts": 0
    }
  ]
}
```

צרף את ה-JSON הזה לכל תיק משאל עם לצד הסיכום הנייטרלי כך
חברי פאנל, מושבעים ומשקיפים ממשל יכולים לראות את רדיוס הפיצוץ המדויק של
כל הצעה. הפלט הוא דטרמיניסטי (ממוין לפי משפחת חשיש) ובטוח
לכלול בספרי CI/runbooks; אם הרישום הכפול או תמונת המצב של המדיניות משתנה,
הפעל מחדש את הפקודה וצרף את החפץ המרענן לפני פתיחת ההצבעה.

> **השלב הבא:** הזינו את דוח ההשפעה שנוצר לתוך
> [`cargo xtask ministry-panel packet`](referendum_packet.md) אז
> תיק `ReferendumPacketV1` מכיל גם את פירוט משפחת הגיבוב וגם את
> רשימת סכסוכים מפורטת עבור ההצעה הנבדקת.
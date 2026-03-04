---
lang: he
direction: rtl
source: docs/source/soranet/nsc-42-legal.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 190eadf5c923a138ed3a50bdf38b1ba739e94732a9e2637ee9d5ee0e86ec4b78
source_last_modified: "2025-12-09T06:50:20.541828+00:00"
translation_last_reviewed: 2026-01-21
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/source/soranet/nsc-42-legal.md -->

# NSC-42 — חתימת חוקיות לקודק

**סטטוס:** הושלם — הריפו משגר כעת את התשתית החוקית הנדרשת ע"י חוות הדעת NSC‑42.

- `NOTICE` מתעד את עמדת הפטנטים לקידוד אנטרופיה (CABAC, trellis, rANS) ומזהיר
  במפורש שפריסות CABAC דורשות רישוי pool (Via Licensing Alliance / Access Advance).
- `PATENTS.md` מסכם את חובות SEP, את עמדת ה‑claim‑avoidance של trellis, ואת
  מגבלות הבסיס של rANS.
- `EXPORT.md` מתעד סיווג EAR99 עבור CABAC/trellis/rANS, ומסביר שכלי הדחיסה
  הללו מחוץ ל‑Category 5 Part 2 "encryption items".
- `tools/rans/gen_tables.py` ו‑`codec/rans/tables/rans_seed0.toml` מספקים טבלאות
  rANS דטרמיניסטיות ברישיון CC0 המוזכרות ב‑NSC‑55 תוך שמירת נתוני מקוריות
  (seed, commit, checksum).
- gating בבנייה: CABAC מושבת אלא אם הבונה מגדיר `ENABLE_CABAC=1` בסביבה, מה שמוסיף
  את cfg `norito_enable_cabac` לקרייט `norito`. סריקות trellis נשארות מושבתות עד
  לפרסום עיצוב claim‑avoidance.
- gating בזמן ריצה: `[streaming.codec]` אוכף את תנאי CABAC/trellis (מצבי cabac
  שאינם `disabled` דורשים את build cfg; רשימות trellis חייבות להישאר ריקות עד
  הגעת פרופיל claim‑avoidance) וכעת מתעד את מתגי האנטרופיה (`entropy_mode`,
  `bundle_width`, `bundle_accel`, `rans_tables_path`) עם אימות, כך ש‑bundled rANS
  יוכל להתגלגל בבטחה לאחר סגירת NSC‑55; פרופיל bundled דורש גם `ENABLE_RANS_BUNDLES=1`
  בזמן בנייה, בהתאמה ל‑CABAC gate. אנטרופיה bundled מקובעת לרוחבי טבלאות 2–3;
  רוחבים מחוץ לטווח נדחים בזמן parse.

**מעקב:** אין — NSC‑55 יורש ארטיפקטים אלו ומתמקד באימות טבלאות דטרמיניסטיות.

</div>

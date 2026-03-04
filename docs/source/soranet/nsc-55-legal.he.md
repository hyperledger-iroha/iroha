---
lang: he
direction: rtl
source: docs/source/soranet/nsc-55-legal.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b72f94767de6c92f28503526ea37756d4ca5a7c4ca007b3d31939e78296eb7de
source_last_modified: "2025-12-09T06:50:20.541828+00:00"
translation_last_reviewed: 2026-01-21
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/source/soranet/nsc-55-legal.md -->

# NSC-55 — אימות rANS ועמדה פטנטית

**סטטוס:** הושלם — טבלאות דטרמיניסטיות, כלי generator והערות מדיניות קיימים כעת בריפו.

- טבלאות דטרמיניסטיות: `codec/rans/tables/rans_seed0.toml` מאחסן את ארטיפקט
  `SignedRansTablesV1` הקנוני עם הקדשה CC0‑1.0, checksum, seed ו‑commit hash.
  הסקריפט `tools/rans/gen_tables.py` עוטף `cargo run -p xtask -- codec rans-tables`
  כדי שרענונים עתידיים יישארו רב‑שחזוריים.
- עמדה פטנטית: `PATENTS.md` מתעד את היקף rANS הבסיסי ומחריג במפורש וריאנטים
  פטנטיים (Markov‑model table switching, escape‑code mechanisms, תכונות
  US 11,234,023 B2 של Microsoft). פרסום הטבלאות אינו כולל רכיבי תביעה אלה.
- מזכר יצוא: `EXPORT.md` מאשר את סיווג EAR99 (NLR) עבור הטבלאות הדטרמיניסטיות
  משום שאינן מממשות קריפטוגרפיה.
- hooks לאימות: פקודת xtask מפיקה כעת ארטיפקטי TOML ו‑`tools/rans/gen_tables.py --verify`
  ממחזר את המאמת המובנה לבדיקת חתימות/בדיקות‑סכום. harnesses של benchmark
  (ראו `benchmarks/nsc/rans_compare.py`) טוענים טבלאות JSON או TOML ומשווים
  encoders מול הווקטורים שפורסמו.
- חיווט תצורה: `[streaming.codec]` מתעד כעת את נתיב הטבלאות הדטרמיניסטיות ואת
  מתגי האנטרופיה (`rans_tables_path`, `entropy_mode`, `bundle_width`, `bundle_accel`).
  מפעילים שמשנים את הנתיב חייבים לספק ארטיפקטים עם אותו מבנה מניפסט, וה‑parsing
  דוחה הגדרות אנטרופיה לא תקינות או קבצים חסרים. אנטרופיה bundled נשלחת כרגע עם
  טבלאות לרוחבים 2–3; תצורות מחוץ לטווח נדחות.

**צעדים הבאים:** הוסיפו seeds/ארטיפקטים נוספים רק לאחר יצירה מחדש דרך
`tools/rans/gen_tables.py` והתחייבות ה‑TOML המתקבל עם כותרת CC0 שלמה.

</div>

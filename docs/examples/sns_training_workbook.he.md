---
lang: he
direction: rtl
source: docs/examples/sns_training_workbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7d6965998c392217380a1722e49098f831438e2f4499b9e3258398a66f905a35
source_last_modified: "2025-11-15T09:17:29.843566+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/examples/sns_training_workbook.md -->

# תבנית מחברת אימון SNS

השתמשו במחברת זו כחומר חלוקה קנוני לכל מחזור אימון. החליפו placeholders (`<...>`) לפני החלוקה למשתתפים.

## פרטי סשן
- suffix: `<.sora | .nexus | .dao>`
- cycle: `<YYYY-MM>`
- שפה: `<ar/es/fr/ja/pt/ru/ur>`
- מנחה: `<name>`

## מעבדה 1 - יצוא KPI
1. פתחו את דשבורד ה-KPI בפורטל (`docs/portal/docs/sns/kpi-dashboard.md`).
2. סננו לפי suffix `<suffix>` וטווח זמן `<window>`.
3. יצאו snapshots PDF + CSV.
4. רשמו SHA-256 של JSON/PDF שיוצאו כאן: `______________________`.

## מעבדה 2 - תרגיל manifest
1. משכו את manifest לדוגמה מתוך `artifacts/sns/training/<suffix>/<cycle>/manifests/<lang>.json`.
2. אמתו עם `cargo run --bin sns_manifest_check -- --input <file>`.
3. צרו שלד resolver בעזרת `scripts/sns_zonefile_skeleton.py`.
4. הדביקו סיכום diff:
   ```
   <git diff output>
   ```

## מעבדה 3 - סימולציית מחלוקת
1. השתמשו ב-CLI guardian כדי להתחיל freeze (case id `<case-id>`).
2. רשמו את hash המחלוקת: `______________________`.
3. העלו את לוג הראיות ל-`artifacts/sns/training/<suffix>/<cycle>/logs/`.

## מעבדה 4 - אוטומציה של annex
1. יצאו את JSON של דשבורד Grafana והעתיקו ל-`artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json`.
2. הריצו:
   ```bash
   cargo xtask sns-annex      --suffix <suffix>      --cycle <cycle>      --dashboard artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json      --dashboard-artifact artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json      --output docs/source/sns/reports/<suffix>/<cycle>.md      --regulatory-entry docs/source/sns/regulatory/<memo>.md      --portal-entry docs/portal/docs/sns/regulatory/<memo-id>.md
   ```
3. הדביקו את נתיב annex + פלט SHA-256: `________________________________`.

## הערות feedback
- מה היה לא ברור?
- אילו מעבדות חרגו מהזמן?
- באגים ב-tooling שנצפו?

החזירו מחברות מלאות למנחה; הן צריכות להישמר תחת
`artifacts/sns/training/<suffix>/<cycle>/workbooks/`.

</div>

---
lang: he
direction: rtl
source: docs/automation/da/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9e5fce128259ae2b2c40782b3c96c38048fce6f3b4522319bd60b59db87a8252
source_last_modified: "2025-11-15T13:38:36.954059+00:00"
translation_last_reviewed: 2026-01-21
---

# אוטומציית מודל איומי זמינות נתונים (DA-1)

<div dir="rtl">

פריט DA-1 במפת הדרכים ו-`status.md` דורשים לולאת אוטומציה דטרמיניסטית שמייצרת
סיכומי מודל איומי Norito PDP/PoTR המופיעים ב-`docs/source/da/threat_model.md`
ובמראה Docusaurus. תיקייה זו מרכזת את התוצרים שאליהם מתייחסים:

- `cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`
- `.github/workflows/da-threat-model-nightly.yml`
- `make docs-da-threat-model` (מריץ `scripts/docs/render_da_threat_model_tables.py`)
- `cargo xtask da-commitment-reconcile --receipt <path> --block <path> [--json-out <path|->]`
- `cargo xtask da-privilege-audit --config <torii.toml> [--extra-path <path> ...] [--json-out <path|->]`

## זרימה

1. **יצירת הדוח**
   ```bash
   cargo xtask da-threat-model-report \
     --config configs/da/threat_model.toml \
     --out artifacts/da/threat_model_report.json
   ```
   סיכום ה-JSON מתעד את שיעור כשלי הרפליקציה המדומה, ספי ה-chunker וכל הפרת מדיניות
   שהתגלתה על ידי harness PDP/PoTR ב-`integration_tests/src/da/pdp_potr.rs`.
2. **רינדור טבלאות Markdown**
   ```bash
   make docs-da-threat-model
   ```
   זה מריץ את `scripts/docs/render_da_threat_model_tables.py` כדי לכתוב מחדש את
   `docs/source/da/threat_model.md` ואת `docs/portal/docs/da/threat-model.md`.
3. **ארכוב התוצר** על ידי העתקת הדוח (ולוג CLI אופציונלי) אל
   `docs/automation/da/reports/<timestamp>-threat_model_report.json`. כאשר החלטות
   ממשל תלויות בהרצה ספציפית, הוסיפו את hash הקומיט ואת seed של הסימולטור בקובץ
   `<timestamp>-metadata.md` צמוד.

## ציפיות ראיות

- קבצי JSON צריכים להישאר מתחת ל-100 KiB כדי להישאר ב-git. עקבות גדולים יותר
  צריכים להישמר באחסון חיצוני; ציינו את ה-hash החתום בהערת המטא-דטה אם צריך.
- כל קובץ מאורכב חייב לציין seed, נתיב קונפיגורציה וגרסת סימולטור כדי לאפשר
  שחזור מלא של הרצות.
- קישורו לקובץ המאורכב מתוך `status.md` או רשומת roadmap בכל פעם שקריטריוני
  הקבלה של DA-1 מתקדמים, כך שמבקרים יוכלו לאמת את קו הבסיס בלי להריץ מחדש את
  ה-harness.

## התאמת התחייבויות (השמטת הסיקוונסר)

השתמשו ב-`cargo xtask da-commitment-reconcile` כדי להשוות בין קבלות ingest של DA
לבין רישומי התחייבויות DA ולאתר השמטה או מניפולציה של הסיקוונסר:

```bash
cargo xtask da-commitment-reconcile \
  --receipt artifacts/da/receipts/ \
  --block storage/blocks/ \
  --json-out artifacts/da/commitment_reconciliation.json
```

- מקבל קבלות בפורמט Norito או JSON והתחייבויות מ-`SignedBlockWire`, `.norito` או
  חבילות JSON.
- נכשל כאשר חסר טיקט ביומן הבלוקים או כאשר hashes מתפצלים; `--allow-unexpected`
  מתעלם מטיקטים שנמצאים רק בבלוקים כאשר מצמצמים במכוון את סט הקבלות.
- צרפו את ה-JSON שנוצר לחבילות ממשל/Alertmanager עבור התראות השמטה; ברירת המחדל
  היא `artifacts/da/commitment_reconciliation.json`.

## ביקורת הרשאות (סקירת גישה רבעונית)

השתמשו ב-`cargo xtask da-privilege-audit` כדי לסרוק את ספריות manifest/replay של
DA (וכן נתיבים נוספים אופציונליים) ולאתר רשומות חסרות/שאינן ספריות/או בעלות
הרשאות כתיבה עולמיות:

```bash
cargo xtask da-privilege-audit \
  --config configs/torii.dev.toml \
  --extra-path /var/lib/iroha/da-manifests \
  --json-out artifacts/da/privilege_audit.json
```

- קורא את נתיבי ingest של DA מתוך קונפיגורציית Torii ובודק הרשאות Unix כאשר
  זמינות.
- מסמן נתיבים חסרים/שאינם ספריות/בעלי הרשאות כתיבה עולמיות ומחזיר קוד יציאה לא
  אפס כאשר יש בעיות.
- חתמו וצרפו את חבילת ה-JSON (`artifacts/da/privilege_audit.json` כברירת מחדל)
  לחבילות ולדשבורדים של ביקורת הגישה הרבעונית.

</div>

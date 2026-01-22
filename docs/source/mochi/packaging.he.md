---
lang: he
direction: rtl
source: docs/source/mochi/packaging.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 71c7478ed1ad61e08608dfd31275a2583cc9563754b56a3fc17080c79f9ee417
source_last_modified: "2025-11-18T09:42:31.664173+00:00"
translation_last_reviewed: 2026-01-21
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/source/mochi/packaging.md -->

# מדריך אריזה ל‑MOCHI

מדריך זה מסביר כיצד לבנות את חבילת ה‑supervisor לשולחן העבודה של MOCHI,
לבדוק את הארטיפקטים שנוצרו, ולכוון את דריסות ה‑runtime שנשלחות עם החבילה.
הוא משלים את ה‑quickstart בכך שהוא מתמקד באריזה רב‑שחזורית ובשימוש ב‑CI.

## דרישות מקדימות

- שרשרת כלים של Rust (edition 2024 / Rust 1.82+) עם תלויות ה‑workspace שכבר
  נבנו.
- `irohad`, `iroha_cli`, ו‑`kagami` מקומפלים עבור היעד הרצוי. ה‑bundler משתמש
  מחדש בבינאריים מתוך `target/<profile>/`.
- שטח דיסק מספיק לפלט החבילה תחת `target/` או יעד מותאם.

בנו את התלויות פעם אחת לפני הרצת ה‑bundler:

```bash
cargo build -p irohad -p iroha_cli -p iroha_kagami
```

## בניית החבילה

הפעילו את פקודת `xtask` הייעודית משורש הריפו:

```bash
cargo xtask mochi-bundle
```

ברירת המחדל יוצרת חבילת release תחת `target/mochi-bundle/` עם שם קובץ שמבוסס
על מערכת ההפעלה והארכיטקטורה של המארח (למשל,
`mochi-macos-aarch64-release.tar.gz`). השתמשו בדגלים הבאים להתאמה:

- `--profile <name>` – בחירת פרופיל Cargo (`release`, `debug`, או פרופיל מותאם).
- `--no-archive` – שמירה על תיקייה פתוחה בלי ליצור ארכיון `.tar.gz`
  (שימושי לבדיקות מקומיות).
- `--out <path>` – כתיבת חבילות ליעד מותאם במקום `target/mochi-bundle/`.
- `--kagami <path>` – אספקת בינארי `kagami` מוכן שייכלל בארכיון. אם לא מסופק,
  ה‑bundler משתמש (או בונה) את הבינארי מתוך הפרופיל שנבחר.
- `--matrix <path>` – הוספת מטא‑נתוני חבילה לקובץ JSON של מטריצה (נוצר אם חסר)
  כדי שצינורות CI יוכלו לרשום כל ארטיפקט מארח/פרופיל שהופק בהרצה. הרשומות כוללות
  ספריית חבילה, נתיב מניפסט ו‑SHA‑256, מיקום ארכיון אופציונלי, ותוצאת smoke-test
  האחרונה.
- `--smoke` – הרצת `mochi --help` מהחבילה כ‑smoke gate קל לאחר האריזה; כשל
  חושף תלויות חסרות לפני פרסום ארטיפקט.
- `--stage <path>` – העתקת החבילה המוגמרת (והארכיון כשנוצר) לתיקיית staging
  כך שבניות רב‑פלטפורמה יוכלו להפקיד ארטיפקטים במיקום אחד ללא סקריפטים נוספים.

הפקודה מעתיקה `mochi-ui-egui`, `kagami`, `LICENSE`, את תצורת הדוגמה, ואת
`mochi/BUNDLE_README.md` אל תוך החבילה. לצד הבינאריים נוצר `manifest.json`
דטרמיניסטי כדי שמשימות CI יוכלו לעקוב אחר hashes וגדלי קבצים.

## פריסת חבילה ואימות

חבילה פתוחה עוקבת אחר הפריסה המתועדת ב‑`BUNDLE_README.md`:

```
bin/mochi
bin/kagami
config/sample.toml
docs/README.md
manifest.json
LICENSE
```

הקובץ `manifest.json` מציג כל ארטיפקט עם hash SHA‑256 שלו. אמתו את החבילה לאחר
העתקה למערכת אחרת:

```bash
jq -r '.files[] | "\(.sha256)  \(.path)"' manifest.json | sha256sum --check
```

צינורות CI יכולים לשמור במטמון את התיקייה הפתוחה, לחתום על הארכיון, או לפרסם
את המניפסט לצד הערות שחרור. המניפסט כולל את פרופיל היוצר, target triple, וחותמת
זמן יצירה כדי לסייע במעקב provenance.

## דריסות זמן־ריצה

MOCHI מאתר בינאריים מסייעים ומיקומי runtime דרך דגלי CLI או משתני סביבה:

- `--data-root` / `MOCHI_DATA_ROOT` – דריסה של סביבת העבודה עבור קונפיגורציות peer,
  אחסון ולוגים.
- `--profile` – מעבר בין preset‑ים טופולוגיים (`single-peer`,
  `four-peer-bft`).
- `--torii-start`, `--p2p-start` – שינוי פורטי הבסיס המשמשים להקצאת שירותים.
- `--irohad` / `MOCHI_IROHAD` – הצבעה לבינארי `irohad` ספציפי.
- `--kagami` / `MOCHI_KAGAMI` – דריסה של `kagami` שבחבילה.
- `--iroha-cli` / `MOCHI_IROHA_CLI` – דריסה של עזר CLI אופציונלי.
- `--restart-mode <never|on-failure>` – ביטול אתחולים אוטומטיים או אכיפת מדיניות
  backoff מעריכית.
- `--restart-max <attempts>` – דריסת מספר ניסיונות האתחול במצב `on-failure`.
- `--restart-backoff-ms <millis>` – הגדרת backoff בסיסי לאתחולים אוטומטיים.
- `MOCHI_CONFIG` – אספקת נתיב מותאם ל‑`config/local.toml`.

ה‑CLI help (`mochi --help`) מציג את רשימת הדגלים המלאה. דריסות הסביבה נכנסות
לתוקף בעת ההרצה וניתן לשלב אותן עם חלון ההגדרות בתוך ה‑UI.

## רמזים לשימוש ב‑CI

- הריצו `cargo xtask mochi-bundle --no-archive` כדי להפיק תיקייה שניתן לדחוס
  בכלי פלטפורמה (ZIP ל‑Windows, tarballs ל‑Unix).
- תעדו מטא‑נתוני חבילה עם `cargo xtask mochi-bundle --matrix dist/matrix.json`
  כדי שמשימות release יוכלו לפרסם אינדקס JSON יחיד שמונה כל ארטיפקט מארח/פרופיל
  שהופק בצינור.
- השתמשו ב‑`cargo xtask mochi-bundle --stage /mnt/staging/mochi` (או דומה) בכל
  agent בנייה כדי להעלות את החבילה והארכיון לתיקייה משותפת שה‑publishing job
  יכול לצרוך.

</div>

---
lang: he
direction: rtl
source: docs/source/mochi_bundle.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f2dd292b7d15b449f3cec1b79343387a8c23beef3a163367bd5fa8ced8593aae
source_last_modified: "2026-01-03T18:08:00.656311+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# חבילת MOCHI Tooling

MOCHI נשלח עם זרימת עבודה לאריזה קלת משקל כך שמפתחים יכולים לייצר א
חבילת שולחן עבודה ניידת ללא חיווט של סקריפטים CI מותאמים אישית. ה-`xtask`
תת-פקודה מטפלת בהידור, פריסה, גיבוב ו- (אופציונלי) ארכיון
יצירה במכה אחת.

## יצירת חבילה

```bash
cargo xtask mochi-bundle
```

כברירת מחדל, הפקודה בונה קבצי שחרור בינאריים, מרכיבה את החבילה מתחת
`target/mochi-bundle/`, ופולט ארכיון `mochi-<os>-<arch>-release.tar.gz`
לצד `manifest.json` דטרמיניסטי. המניפסט מפרט כל קובץ עם
גודלו ו-hash SHA-256 כך שצינורות CI יכולים להפעיל מחדש את האימות או לפרסם
עדויות. העוזר מבטיח הן את מעטפת שולחן העבודה `mochi` והן את
סביבת עבודה `kagami` בינאריים קיימים כך שיצירת בראשית פועלת מתוך
קופסה.

### דגלים

| דגל | תיאור |
|---------------------|--------------------------------------------------------------------------------|
| `--out <dir>` | עוקף את ספריית הפלט (ברירת המחדל היא `target/mochi-bundle`).         |
| `--profile <name>` | בנה עם פרופיל מטען ספציפי (לדוגמה, `debug` לבדיקות).              |
| `--no-archive` | דלג על ארכיון `.tar.gz`, השאר רק את התיקיה המוכנה.               |
| `--kagami <path>` | השתמש בבינארי `kagami` מפורש במקום לבנות `iroha_kagami`.         |
| `--matrix <path>` | הוסף מטא נתונים של חבילה למטריצת JSON למעקב אחר מקור CI.         |
| `--smoke` | הפעל את `mochi --help` מהצרור הארוז כשער ביצוע בסיסי.      |
| `--stage <dir>` | העתק את החבילה המוגמרת (וארכיון, כאשר קיים) לתיקיית שלב. |

`--stage` מיועד לצינורות CI שבהם כל סוכן בנייה מעלה את שלו
חפצי אמנות למיקום משותף. העוזר יוצר מחדש את ספריית החבילות ו
מעתיק את הארכיון שנוצר לתוך ספריית הסטaging כדי לפרסם עבודות
אסוף פלטים ספציפיים לפלטפורמה ללא סקריפטים של מעטפת.

הפריסה בתוך החבילה פשוטה בכוונה:

```
bin/mochi              # egui desktop executable
bin/kagami             # kagami helper for genesis generation
config/sample.toml     # starter supervisor configuration
docs/README.md         # bundle overview and verification guide
LICENSE                # repository licence
manifest.json          # generated file manifest with SHA-256 digests
```

### עקיפת זמן ריצה

קובץ ההפעלה הארוז `mochi` מקבל עקיפות שורת הפקודה לכל היותר
הגדרות מפקח נפוצות. השתמש בדגלים אלה במקום לערוך
`config/local.toml` בעת ניסוי:

```
./bin/mochi --data-root ./data --profile four-peer-bft \
    --torii-start 12000 --p2p-start 14000 \
    --irohad /path/to/irohad --kagami /path/to/kagami
```

כל ערך CLI מקבל עדיפות על ערכי `config/local.toml` והסביבה
משתנים.

## אוטומציה של תמונת מצב

`manifest.json` מתעד את חותמת הזמן של הדור, טריפל היעד, פרופיל המטען,
ואת מלאי הקבצים המלא. צינורות יכולים לשנות את המניפסט כדי לזהות מתי
מופיעים חפצי אמנות חדשים, העלו את ה-JSON לצד נכסי ההפצה, או בקרו את
hashes לפני קידום חבילה למפעילים.

העוזר אימפוטנטי: הפעלה מחדש של הפקודה מעדכנת את המניפסט ו
מחליף את הארכיון הקודם, שומר את `target/mochi-bundle/` כסינגל
מקור האמת עבור החבילה העדכנית ביותר במכונה הנוכחית.
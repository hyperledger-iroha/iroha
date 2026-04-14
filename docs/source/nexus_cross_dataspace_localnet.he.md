<!-- Auto-generated stub for Hebrew (he) translation. Replace this content with the full translation. -->

---
lang: he
direction: rtl
source: docs/source/nexus_cross_dataspace_localnet.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2324cfc7b086ceb96317eb2260abe41101f17e5c0749d0a1d28ffbf4cb5e8e45
source_last_modified: "2026-02-19T18:33:20.275472+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Nexus הוכחת Localnet חוצה מרחב נתונים

ספר ריצה זה מבצע את הוכחת האינטגרציה Nexus ש:

- מאתחל רשת מקומית בעלת 4 עמיתים עם שני מרחבי נתונים פרטיים מוגבלים (`ds1`, `ds2`),
- מנתב תעבורת חשבון לכל מרחב נתונים,
- יוצר נכס בכל מרחב נתונים,
- מבצע יישוב חילופי אטומיים על פני מרחבי נתונים בשני הכיוונים,
- מוכיח סמנטיקה של החזרה לאחור על ידי הגשת רגל ממומנת בתת-מימון ובדיקת יתרות נשארות ללא שינוי.

המבחן הקנוני הוא:
`nexus::cross_dataspace_localnet::cross_dataspace_atomic_swap_is_all_or_nothing`.

## ריצה מהירה

השתמש בסקריפט העטיפה משורש המאגר:

```bash
scripts/run_nexus_cross_dataspace_atomic_swap.sh
```

התנהגות ברירת מחדל:

- מריץ רק את מבחן ההוכחה בין שטחי נתונים,
- סטים `NORITO_SKIP_BINDINGS_SYNC=1`,
- סטים `IROHA_TEST_SKIP_BUILD=1`,
- משתמש ב-`--test-threads=1`,
- עובר `--nocapture`.

## אפשרויות שימושיות

```bash
scripts/run_nexus_cross_dataspace_atomic_swap.sh --keep-dirs
scripts/run_nexus_cross_dataspace_atomic_swap.sh --no-skip-build
scripts/run_nexus_cross_dataspace_atomic_swap.sh --release
scripts/run_nexus_cross_dataspace_atomic_swap.sh --all-nexus
```

- `--keep-dirs` שומר ספריות עמיתים זמניות (`IROHA_TEST_NETWORK_KEEP_DIRS=1`) לזיהוי פלילי.
- `--all-nexus` מריץ את `mod nexus::` (משנה של שילוב Nexus), לא רק את מבחן ההוכחה.

## שער CI

עוזר CI:

```bash
ci/check_nexus_cross_dataspace_localnet.sh
```

הפוך יעד:

```bash
make check-nexus-cross-dataspace
```

שער זה מבצע את מעטפת ההוכחה הדטרמיניסטית ונכשל בעבודה אם המרחב האטומי חוצה נתונים
תרחיש ההחלפה נסוג.

## פקודות מקבילות ידניות

מבחן הוכחה ממוקד:

```bash
IROHA_TEST_SKIP_BUILD=1 NORITO_SKIP_BINDINGS_SYNC=1 \
  cargo test -p integration_tests --test mod \
  nexus::cross_dataspace_localnet::cross_dataspace_atomic_swap_is_all_or_nothing \
  -- --nocapture --test-threads=1
```

קבוצת משנה מלאה של Nexus:

```bash
IROHA_TEST_SKIP_BUILD=1 NORITO_SKIP_BINDINGS_SYNC=1 \
  cargo test -p integration_tests --test mod nexus:: -- --nocapture --test-threads=1
```

## אותות הוכחה צפויים- המבחן עובר.
- אזהרה צפויה אחת מופיעה עבור חלקת ההתנחלות שנכשלה בכוונה בתת-מימון:
  `settlement leg requires 10000 but only ... is available`.
- קביעות מאזן סופי מצליחות לאחר:
  - החלפה מוצלחת קדימה,
  - החלפה הפוכה מוצלחת,
  - החלפה לא ממומנת בחסר (החזרה לאחור ללא שינוי).

## תמונת מצב אימות נוכחית

החל מ-**19 בפברואר 2026**, זרימת עבודה זו עברה עם:

- בדיקה ממוקדת: `1 passed; 0 failed`,
- קבוצת משנה מלאה של Nexus: `24 passed; 0 failed`.
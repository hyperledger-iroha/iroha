<!-- Auto-generated stub for Hebrew (he) translation. Replace this content with the full translation. -->

---
lang: he
direction: rtl
source: docs/formal/sumeragi/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 56f1412b2db729ba69057ce15ac8bae707310fd5a6d01be2da816fdee18218f7
source_last_modified: "2026-02-23T14:48:46.580877+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Sumeragi דגם רשמי (TLA+ / Apalache)

ספרייה זו מכילה מודל רשמי מוגבל עבור Sumeragi בטיחות וחיוניות של נתיב התחייבות.

## היקף

הדגם לוכד:
- התקדמות שלב (`Propose`, `Prepare`, `CommitVote`, `NewView`, `Committed`),
- ספי הצבעה ומניין (`CommitQuorum`, `ViewQuorum`),
- מניין יתד משוקלל (`StakeQuorum`) לשומרי מחויבות בסגנון NPoS,
- סיבתיות RBC (`Init -> Chunk -> Ready -> Deliver`) עם עדות כותרת/עיכול,
- GST והנחות הוגנות חלשות על פני פעולות התקדמות כנות.

זה מפשט בכוונה פורמטים של חוטים, חתימות ופרטי רשת מלאים.

## קבצים

- `Sumeragi.tla`: דגם ומאפיינים של פרוטוקול.
- `Sumeragi_fast.cfg`: ערכת פרמטרים קטנה יותר ידידותית ל-CI.
- `Sumeragi_deep.cfg`: סט פרמטרי מתח גדול יותר.

## מאפיינים

אינוריאנטים:
- `TypeInvariant`
- `CommitImpliesQuorum`
- `CommitImpliesStakeQuorum`
- `CommitImpliesDelivered`
- `DeliverImpliesEvidence`

נכס זמני:
- `EventuallyCommit` (`[] (gst => <> committed)`), עם קידוד הגינות לאחר ה-GST
  באופן תפעולי ב-`Next` (משגי פסק זמן/מניעת תקלות מופעלים
  פעולות התקדמות). זה שומר על בדיקת הדגם עם Apalache 0.52.x, אשר
  אינו תומך במפעילי הגינות `WF_` בתוך מאפיינים זמניים מסומנים.

## ריצה

משורש המאגר:

```bash
bash scripts/formal/sumeragi_apalache.sh fast
bash scripts/formal/sumeragi_apalache.sh deep
```

### הגדרה מקומית ניתנת לשחזור (אין צורך ב-Docker)התקן את שרשרת הכלים המקומית המוצמדת של Apalache המשמשת את המאגר הזה:

```bash
bash scripts/formal/install_apalache.sh 0.52.2
```

הרץ מזהה אוטומטית את ההתקנה הזו ב:
`target/apalache/toolchains/v0.52.2/bin/apalache-mc`.
לאחר ההתקנה, `ci/check_sumeragi_formal.sh` אמור לעבוד ללא וריאציות מיותרות:

```bash
bash ci/check_sumeragi_formal.sh
```

אם Apalache אינו ב-`PATH`, אתה יכול:

- הגדר את `APALACHE_BIN` לנתיב ההפעלה, או
- השתמש ב-Docker (מופעל כברירת מחדל כאשר `docker` זמין):
  - תמונה: `APALACHE_DOCKER_IMAGE` (ברירת מחדל `ghcr.io/apalache-mc/apalache:latest`)
  - דורש דמון Docker פועל
  - השבת את החזרה עם `APALACHE_ALLOW_DOCKER=0`.

דוגמאות:

```bash
APALACHE_BIN=/opt/apalache/bin/apalache-mc bash scripts/formal/sumeragi_apalache.sh fast
APALACHE_DOCKER_IMAGE=ghcr.io/apalache-mc/apalache:latest bash scripts/formal/sumeragi_apalache.sh deep
```

## הערות

- דגם זה משלים (לא מחליף) בדיקות מודל Rust הניתנות להפעלה ב
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_model_tests.rs`
  ו
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_fairness_model_tests.rs`.
- ההמחאות מוגבלות לערכים קבועים בקבצי `.cfg`.
- PR CI מפעיל את הבדיקות הללו ב-`.github/workflows/pr.yml` באמצעות
  `ci/check_sumeragi_formal.sh`.
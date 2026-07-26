---
lang: he
direction: rtl
source: docs/source/ministry/agenda_council_proposal.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d2a7a47fdf0c80d189c912baafa5d6ce81a17a4c90f2b1797e532989a56f5060
source_last_modified: "2026-01-03T18:07:57.726224+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# סכימת הצעות מועצת סדר היום (MINFO-2a)

התייחסות למפת הדרכים: **MINFO-2a - אימות פורמט הצעה.**

זרימת העבודה של מועצת האג'נדה מאגדת רשימה שחורה שהוגשה על ידי אזרחים ושינוי מדיניות
הצעות לפני שפאנלי הממשל בודקים אותן. מסמך זה מגדיר את
סכימת מטען קנונית, דרישות ראיות וכללי זיהוי כפילות
נצרך על ידי האימות החדש (`cargo xtask ministry-agenda validate`) כך
מציעים יכולים לצבוע הגשות JSON באופן מקומי לפני העלאתן לפורטל.

## סקירת מטען

הצעות לסדר היום משתמשות בסכימה `AgendaProposalV1` Norito
(`iroha_data_model::ministry::AgendaProposalV1`). שדות מקודדים כ-JSON כאשר
הגשה דרך משטחי CLI/פורטל.

| שדה | הקלד | דרישות |
|-------|------|-------------|
| `version` | `1` (u16) | חייב להיות שווה ל-`AGENDA_PROPOSAL_VERSION_V1`. |
| `proposal_id` | מחרוזת (`AC-YYYY-###`) | מזהה יציב; נאכף במהלך האימות. |
| `submitted_at_unix_ms` | u64 | אלפיות שניות מאז תקופת יוניקס. |
| `language` | מחרוזת | תג BCP-47 (`"en"`, `"ja-JP"` וכו'). |
| `action` | enum (`add-to-denylist`, `remove-from-denylist`, `amend-policy`) | פעולה מבוקשת של המשרד. |
| `summary.title` | מחרוזת | ≤256 תווים מומלץ. |
| `summary.motivation` | מחרוזת | מדוע נדרשת הפעולה. |
| `summary.expected_impact` | מחרוזת | תוצאות אם הפעולה מתקבלת. |
| `tags[]` | מחרוזות קטנות | תוויות טריאג' אופציונליות. ערכים מותרים: `csam`, `malware`, `fraud`, `harassment`, `impersonation`, `policy-escalation`, I100NI30X, I100NI00X, `spam`. |
| `targets[]` | חפצים | ערך משפחת hash אחד או יותר (ראה להלן). |
| `evidence[]` | חפצים | קובץ מצורף ראיה אחד או יותר (ראה להלן). |
| `submitter.name` | מחרוזת | שם או ארגון לתצוגה. |
| `submitter.contact` | מחרוזת | דואר אלקטרוני, ידית מטריקס או טלפון; הוסר מלוחות מחוונים ציבוריים. |
| `submitter.organization` | מחרוזת (אופציונלי) | גלוי בממשק המשתמש של הבודקים. |
| `submitter.pgp_fingerprint` | מחרוזת (אופציונלי) | טביעת אצבע גדולה של 40 hex. |
| `duplicates[]` | מחרוזות | הפניות אופציונליות למזהי הצעות שנשלחו בעבר. |

### ערכי יעד (`targets[]`)

כל יעד מייצג תקציר משפחת hash שאליו מתייחס ההצעה.

| שדה | תיאור | אימות |
|-------|-------------|--------|
| `label` | שם ידידותי להקשר של מבקר. | לא ריק. |
| `hash_family` | מזהה Hash (`blake3-256`, `sha256` וכו'). | אותיות/ספרות ASCII/`-_.`, ≤48 תווים. |
| `hash_hex` | תקציר מקודד בהקסס באותיות קטנות. | ≥16 בתים (32 תווים hex) וחייבים להיות hex חוקי. |
| `reason` | תיאור קצר מדוע יש לבצע את העיכול. | לא ריק. |

האימות דוחה צמדי `hash_family:hash_hex` כפולים בתוך אותו
הצעות ודוחות מתנגשות כאשר אותה טביעת אצבע כבר קיימת ב-
רישום כפול (ראה להלן).

### קבצים מצורפים של ראיות (`evidence[]`)

מסמך ערכי ראיות שבו סוקרים יכולים להביא הקשר תומך.| שדה | הקלד | הערות |
|-------|------|-------|
| `kind` | enum (`url`, `torii-case`, `sorafs-cid`, `attachment`) | קובע את דרישות העיכול. |
| `uri` | מחרוזת | כתובת URL של HTTP(S), Torii מזהה מקרה, או SoraFS URI. |
| `digest_blake3_hex` | מחרוזת | נדרש עבור סוגי `sorafs-cid` ו-`attachment`; אופציונלי עבור אחרים. |
| `description` | מחרוזת | טקסט אופציונלי בצורה חופשית עבור סוקרים. |

### רישום כפול

מפעילים יכולים לנהל רישום של טביעות אצבע קיימות כדי למנוע כפילות
מקרים. המאמת מקבל קובץ JSON בצורת:

```json
{
  "entries": [
    {
      "hash_family": "blake3-256",
      "hash_hex": "0d714bed4b7c63c23a2cf8ee9ce6c3cde1007907c427b4a0754e8ad31c91338d",
      "proposal_id": "AC-2025-014",
      "note": "Already handled in 2025-08 incident"
    }
  ]
}
```

כאשר יעד הצעה תואם לערך, המאמת מבטל אלא אם כן
`--allow-registry-conflicts` צוין (אזהרות עדיין נפלטות).
השתמש ב-[`cargo xtask ministry-agenda impact`](impact_assessment_tooling.md) כדי
ליצור את הסיכום המוכן למשאל עם שמצליב את הכפילות
תמונות מצב של רישום ומדיניות.

## שימוש ב-CLI

מוך הצעה יחידה ובדוק אותה מול רישום כפול:

```bash
cargo xtask ministry-agenda validate \
  --proposal docs/examples/ministry/agenda_proposal_example.json \
  --registry docs/examples/ministry/agenda_duplicate_registry.json
```

עברו את `--allow-registry-conflicts` כדי לשדרג לאחור התאמות כפולות לאזהרות כאשר
ביצוע ביקורות היסטוריות.

ה-CLI מסתמך על אותה סכימת Norito ועוזרי אימות שנשלחו ב
`iroha_data_model`, כך ש-SDKs/פורטלים יכולים לעשות שימוש חוזר ב-`AgendaProposalV1::validate`
שיטה להתנהגות עקבית.

## מיון CLI (MINFO-2b)

התייחסות למפת הדרכים: **MINFO-2b — מיון ויומן ביקורת מרובי חריצים.**

סגל מועצת האג'נדה מנוהל כעת באמצעות מיון דטרמיניסטי כך שהאזרחים
יכול לבדוק באופן עצמאי כל הגרלה. השתמש בפקודה החדשה:

```bash
cargo xtask ministry-agenda sortition \
  --roster docs/examples/ministry/agenda_council_roster.json \
  --slots 3 \
  --seed 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef \
  --out artifacts/ministry/agenda_sortition_2026Q1.json
```

- `--roster` - קובץ JSON המתאר כל חבר כשיר:

  ```json
  {
    "format_version": 1,
    "members": [
      {
        "member_id": "citizen:ada",
        "weight": 2,
        "role": "citizen",
        "organization": "Artemis Cooperative"
      },
      {
        "member_id": "citizen:erin",
        "weight": 1,
        "role": "citizen",
        "eligible": false
      }
    ]
  }
  ```

  הקובץ לדוגמה חי ב
  `docs/examples/ministry/agenda_council_roster.json`. שדות אופציונליים (תפקיד,
  ארגון, איש קשר, מטא נתונים) נלכדים בעלה של מרקל כך שמבקרים
  יכול להוכיח את הסגל שהזין את ההגרלה.

- `--slots` - מספר מושבים במועצה למילוי.
- `--seed` - 32-בתים BLAKE3 seed (64 תווי hex קטן) מתועד ב-
  דקות ממשל להגרלה.
- `--out` — נתיב פלט אופציונלי. כאשר מושמט, תקציר JSON מודפס אל
  stdout.

### סיכום פלט

הפקודה פולטת כתם `SortitionSummary` JSON. פלט לדוגמא מאוחסן ב
`docs/examples/ministry/agenda_sortition_summary_example.json`. שדות מפתח:

| שדה | תיאור |
|-------|-------------|
| `algorithm` | תווית מיון (`agenda-sortition-blake3-v1`). |
| `roster_digest` | BLAKE3 + SHA-256 תקצירים של קובץ הסגל (משמש לאשר שביקורות פועלות על אותה רשימת חברים). |
| `seed_hex` / `slots` | הד את כניסות ה-CLI כדי שהאודיטורים יוכלו לשחזר את ההגרלה. |
| `merkle_root_hex` | שורש עץ הסגל מרקל (`hash_node`/`hash_leaf` עוזרים ב-`xtask/src/ministry_agenda.rs`). |
| `selected[]` | ערכים עבור כל משבצת, כולל מטא-נתונים של החברים הקנוניים, אינדקס כשיר, אינדקס סגל מקורי, אנטרופיית ציור דטרמיניסטית, גיבוב עלים ואחיות הוכחה מרקל. |

### אימות תיקו1. קבל את הסגל שאליו מתייחס `roster_path` ואמת את ה-BLAKE3/SHA-256 שלו
   תקצירים תואמים את הסיכום.
2. הפעל מחדש את ה-CLI עם אותו סיד/חריצים/סגל; התוצאה `selected[].member_id`
   ההזמנה צריכה להתאים לסיכום שפורסם.
3. עבור חבר ספציפי, חשב את עלה Merkle באמצעות החבר המסודר JSON
   (`norito::json::to_vec(&sortition_member)`) ומקפלים פנימה כל גיבוב הוכחה. הגמר
   תקציר חייב להיות שווה ל-`merkle_root_hex`. העוזר בסיכום הדוגמה מראה
   כיצד לשלב `eligible_index`, `leaf_hash_hex` ו-`merkle_proof[]`.

חפצי אמנות אלה עומדים בדרישת MINFO-2b לאקראיות הניתנת לאימות,
בחירת k-of-m, והוספה של יומני ביקורת בלבד עד שה-API על השרשרת יחווט.

## הפניה לשגיאת אימות

`AgendaProposalV1::validate` פולט גרסאות `AgendaProposalValidationError`
בכל פעם שמטען נכשל במוך. הטבלה שלהלן מסכמת את הנפוצים ביותר
שגיאות כדי שבודקי פורטל יוכלו לתרגם פלט CLI להנחיה ניתנת לפעולה.| שגיאה | המשמעות | תיקון |
|-------|--------|-------------|
| `UnsupportedVersion { expected, found }` | מטען `version` שונה מהסכימה הנתמכת של המאמת. | צור מחדש את ה-JSON באמצעות חבילת הסכימה העדכנית ביותר כך שהגרסה תתאים ל-`expected`. |
| `MissingProposalId` / `InvalidProposalIdFormat { value }` | `proposal_id` ריק או לא בצורת `AC-YYYY-###`. | מלא מזהה ייחודי בעקבות הפורמט המתועד לפני השליחה מחדש. |
| `MissingSubmissionTimestamp` | `submitted_at_unix_ms` אפס או חסר. | רשום את חותמת הזמן של ההגשה באלפיות שניות של יוניקס. |
| `InvalidLanguageTag { value }` | `language` אינו תג BCP-47 חוקי. | השתמש בתג סטנדרטי כגון `en`, `ja-JP`, או מקום אחר המוכר על ידי BCP‑47. |
| `MissingSummaryField { field }` | אחד מ-`summary.title`, `.motivation` או `.expected_impact` ריק. | ספק טקסט לא ריק עבור שדה הסיכום המצוין. |
| `MissingSubmitterField { field }` | חסר `submitter.name` או `submitter.contact`. | ספק את המטא-נתונים החסרים של המגיש כדי שהבודקים יוכלו ליצור קשר עם המציע. |
| `InvalidTag { value }` | ערך `tags[]` אינו ברשימת ההיתרים. | הסר או שנה את שם התג לאחד מהערכים המתועדים (`csam`, `malware` וכו'). |
| `MissingTargets` | מערך `targets[]` ריק. | ספק לפחות ערך משפחת hash יעד אחד. |
| `MissingTargetLabel { index }` / `MissingTargetReason { index }` | הזנת היעד חסרה את השדות `label` או `reason`. | מלא את השדה הנדרש עבור הערך שנוסף לאינדקס לפני השליחה מחדש. |
| `InvalidHashFamily { index, value }` | תווית `hash_family` לא נתמכת. | הגבל שמות משפחה גיבוב ל-ASCII אלפאנומריות בתוספת `-_`. |
| `InvalidHashHex { index, value }` / `TargetDigestTooShort { index }` | תקציר אינו hex חוקי או קצר מ-16 בתים. | ספק תקציר hex באותיות קטנות (≥32 תווים hex) עבור היעד שצורף לאינדקס. |
| `DuplicateTarget { index, fingerprint }` | תקציר יעד משכפל ערך קודם או טביעת אצבע של הרישום. | הסר כפילויות או מיזוג את הראיות התומכות למטרה אחת. |
| `MissingEvidence` | לא סופקו צירופי ראיות. | צרף לפחות תיעוד ראיות אחד המקשר לחומר העתקה. |
| `MissingEvidenceUri { index }` | הזנת ראיות חסרה את השדה `uri`. | ספק את ה-URI הניתן לשליפה או מזהה מקרה עבור הזנת הראיות שנוספה לאינדקס. |
| `MissingEvidenceDigest { index }` / `InvalidEvidenceDigest { index, value }` | הזנת ראיה הדורשת תקציר (SoraFS CID או קובץ מצורף) חסרה או שיש לה `digest_blake3_hex` לא חוקי. | ספק תקציר BLAKE3 באותיות קטנות של 64 תווים עבור הערך המופיע באינדקס. |

## דוגמאות

- `docs/examples/ministry/agenda_proposal_example.json` - קנוני,
  מטען הצעה נקי מוך עם שני קבצי ראיות.
- `docs/examples/ministry/agenda_duplicate_registry.json` - רישום מתחילים
  מכיל טביעת אצבע אחת ורציונל BLAKE3.

השתמש מחדש בקבצים אלה כתבניות בעת שילוב כלי פורטל או כתיבת CI
בודק הגשות אוטומטיות.
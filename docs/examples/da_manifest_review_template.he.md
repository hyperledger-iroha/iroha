---
lang: he
direction: rtl
source: docs/examples/da_manifest_review_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c5c959bd6654d095d2b3785a02e9c2ec162e699ad985b342760b952e38766a66
source_last_modified: "2025-11-12T19:46:29.811940+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/examples/da_manifest_review_template.md -->

# חבילת ממשל למניפסט זמינות נתונים (תבנית)

השתמשו בתבנית זו כאשר פאנלי הפרלמנט בוחנים מניפסטי DA עבור סבסוד,
Takedown או שינויים בשימור (roadmap DA-10). העתיקו את ה-Markdown לכרטיס הממשל,
מלאו את ה-placeholders, וצירפו את הקובץ המושלם יחד עם מטעני Norito החתומים
וארтеפקטים של CI המוזכרים מטה.

```markdown
## מטא-נתוני המניפסט
- שם / גרסת המניפסט: <string>
- מחלקת blob ותג ממשל: <taikai_segment / da.taikai.live>
- דייג'סט BLAKE3 (hex): `<digest>`
- האש מטען Norito (אופציונלי): `<digest>`
- מעטפת מקור / URL: <https://.../manifest_signatures.json>
- מזהה snapshot למדיניות Torii: `<unix timestamp or git sha>`

## אימות חתימות
- מקור משיכת המניפסט / כרטיס אחסון: `<hex>`
- פקודת/פלט אימות: `cargo run -p sorafs_car --bin sorafs-manifest-stub -- manifest.json --manifest-signatures-in=manifest_signatures.json` (מצורף קטע לוג?)
- `manifest_blake3` כפי שדווח ע"י הכלי: `<digest>`
- `chunk_digest_sha3_256` כפי שדווח ע"י הכלי: `<digest>`
- מולטי-האש של חותמי המועצה:
  - `<did:...>` / `<ed25519 multihash>`
- חותמת זמן אימות (UTC): `<2026-02-20T11:04:33Z>`

## אימות שימור
| שדה | צפוי (מדיניות) | נצפה (מניפסט) | ראיה |
|-----|----------------|----------------|------|
| שימור חם (שניות) | <למשל, 86400> | <ערך> | `<torii.da_ingest.replication_policy dump | CI link>` |
| שימור קר (שניות) | <למשל, 1209600> | <ערך> |  |
| מספר רפליקות נדרש | <ערך> | <ערך> |  |
| מחלקת אחסון | <hot / warm / cold> | <ערך> |  |
| תג ממשל | <da.taikai.live> | <ערך> |  |

## הקשר
- סוג בקשה: <סבסוד | Takedown | רוטציית מניפסט | הקפאה חריגה>
- כרטיס מקור / סימוכין compliance: <קישור או מזהה>
- השפעת סבסוד / שכירות: <שינוי XOR צפוי או "n/a">
- קישור לערעור מודרציה (אם יש): <case_id או קישור>

## סיכום החלטה
- פאנל: <תשתיות | מודרציה | אוצר>
- ספירת הצבעה: `<for>/<against>/<abstain>` (קוורום `<threshold>` הושג?)
- גובה/חלון הפעלה או rollback: `<block/slot range>`
- פעולות המשך:
  - [ ] עדכון לאוצר / ops שכירות
  - [ ] עדכון דוח שקיפות (`TransparencyReportV1`)
  - [ ] תזמון ביקורת buffer

## הסלמה ודיווח
- מסלול הסלמה: <סבסוד | Compliance | הקפאה חריגה>
- קישור / מזהה לדוח שקיפות (אם עודכן): <`TransparencyReportV1` CID>
- חבילת proof-token או סימוכין ComplianceUpdate: <נתיב או מזהה כרטיס>
- דלתא של לוח שכירות / רזרבה (אם יש): <`ReserveSummaryV1` snapshot link>
- URL(s) לצילום טלמטריה: <Grafana permalink או מזהה artefact>
- הערות לפרוטוקול הפרלמנט: <סיכום דדליינים / התחייבויות>

## נספחים
- [ ] מניפסט Norito חתום (`.to`)
- [ ] סיכום JSON / ארטיפקט CI שמוכיח ערכי שימור
- [ ] proof token או חבילת compliance (לטייקדאונים)
- [ ] צילום טלמטריה של buffer (`iroha_settlement_buffer_xor`)
```

שמרו כל חבילה מושלמת תחת רשומת ה-Governance DAG של ההצבעה כדי שבדיקות עתידיות יוכלו
להתייחס לדייג'סט של המניפסט בלי לחזור על הטקס המלא.

</div>

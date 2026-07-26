---
lang: he
direction: rtl
source: docs/source/domain_endorsements.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c337150e6de1efa9f9480ba8126ecd5ada4ed8ee7ee8b70a95fd7f6348f9016
source_last_modified: "2026-01-03T18:08:00.700192+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# המלצות דומיין

אישורי דומיין מאפשרים למפעילים להגדיר יצירת דומיינים ולעשות בהם שימוש חוזר במסגרת הצהרה שנחתמה על ידי ועדה. מטען האישור הוא אובייקט Norito שנרשם בשרשרת, כך שלקוחות יכולים לבדוק מי העיד על איזה תחום ומתי.

## צורת מטען

- `version`: `DOMAIN_ENDORSEMENT_VERSION_V1`
- `domain_id`: מזהה דומיין קנוני
- `committee_id`: תווית ועדה הניתנת לקריאה על ידי אדם
- `statement_hash`: `Hash::new(domain_id.to_string().as_bytes())`
- `issued_at_height` / `expires_at_height`: תוקף מגביל גבהים של בלוק
- `scope`: מרחב נתונים אופציונלי בתוספת חלון `[block_start, block_end]` אופציונלי (כולל) ש**חייב** לכסות את גובה הבלוק המקבל
- `signatures`: חתימות מעל `body_hash()` (אישור עם `signatures = []`)
- `metadata`: מטא נתונים אופציונליים של Norito (מזהי הצעה, קישורי ביקורת וכו')

## אכיפה

- אישורים נדרשים כאשר Nexus מופעל ו-`nexus.endorsement.quorum > 0`, או כאשר מדיניות לכל דומיין מסמנת את הדומיין כנדרש.
- אימות אוכף קשירת גיבוב לתחום/הצהרה, גרסה, חלון חסימה, חברות במרחב הנתונים, תפוגה/גיל ומניין ועדה. החותמים חייבים להיות בעלי מפתחות קונצנזוס חיים עם התפקיד `Endorsement`. שידורים חוזרים נדחים על ידי `body_hash`.
- המלצות המצורפות לרישום דומיין משתמשות במפתח מטא נתונים `endorsement`. אותו נתיב אימות משמש את ההוראה `SubmitDomainEndorsement`, אשר מתעדת אישורים לביקורת מבלי לרשום תחום חדש.

## ועדות ומדיניות

- ניתן לרשום ועדות ברשת (`RegisterDomainCommittee`) או לנגזר מברירות מחדל של תצורה (`nexus.endorsement.committee_keys` + `nexus.endorsement.quorum`, מזהה = `default`).
- מדיניות פר-דומיין מוגדרת באמצעות `SetDomainEndorsementPolicy` (מזהה ועדה, `max_endorsement_age`, דגל `required`). כאשר נעדר, נעשה שימוש בברירות המחדל של Nexus.

## עוזרי CLI

- בנה/חתום על אישור (מוציא Norito JSON ל-stdout):

  ```
  iroha endorsement prepare \
    --domain wonderland \
    --committee-id default \
    --issued-at-height 5 \
    --expires-at-height 25 \
    --block-start 5 \
    --block-end 15 \
    --signer-key <PRIVATE_KEY> --signer-key <PRIVATE_KEY>
  ```

- הגש אישור:

  ```
  iroha endorsement submit --file endorsement.json
  # or: cat endorsement.json | iroha endorsement submit
  ```

- ניהול ממשל:
  - `iroha endorsement register-committee --committee-id jdga --quorum 2 --member <PK> --member <PK> [--metadata path]`
  - `iroha endorsement set-policy --domain wonderland --committee-id jdga --max-endorsement-age 1000 --required`
  - `iroha endorsement policy --domain wonderland`
  - `iroha endorsement committee --committee-id jdga`
  - `iroha endorsement list --domain wonderland`

כשלי אימות מחזירים מחרוזות שגיאה יציבות (אי התאמה למניין, אישור מיושן/פג תוקף, חוסר התאמה של היקף, מרחב נתונים לא ידוע, ועדה חסרה).
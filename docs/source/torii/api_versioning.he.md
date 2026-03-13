---
lang: he
direction: rtl
source: docs/source/torii/api_versioning.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f0e27b4aa44b025737d3e8dfed751fc5a0bc1dd12807f77a887f46ce830025c8
source_last_modified: "2026-01-04T10:50:53.697166+00:00"
translation_last_reviewed: 2026-01-22
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/source/torii/api_versioning.md -->

# גרסאות API של Torii

Torii מנהל משא ומתן על גרסת API סמנטית (`major.minor`) בכל בקשה באמצעות הכותרת
`x-iroha-api-version`. אם הכותרת חסרה, Torii מניח את הגרסה הנתמכת האחרונה ומחזיר
אותה יחד עם מטריצת התמיכה המלאה.

- נתמכות כיום: `1.0`, `1.1` (ברירת מחדל).
- משטחי proof/staking/fee דורשים לפחות `1.1`; גרסאות ישנות מקבלות
  `426 UPGRADE_REQUIRED` עם `code="torii_api_version_too_old"`.
  `1893456000`, 2030-01-01 UTC) וממשיכים לשרת תגובות תוך רישום אזהרה.
- כותרות התגובה כוללות תמיד את `x-iroha-api-version`,
  `x-iroha-api-supported`, ואת `x-iroha-api-min-proof-version` כדי שלקוחות יוכלו
  להסתגל בלי לקבע ערכים בקוד.
- קודי השגיאה יציבים:
  - `torii_api_version_invalid` — הכותרת אינה גרסה סמנטית.
  - `torii_api_version_unsupported` — גרסה שאינה בקבוצת התמיכה של השרת.
  - `torii_api_version_too_old` — גרסה מתחת למינימום לכל משטח.

נקודת הקצה `/v2/api/versions` מציגה את מטריצת התמיכה (`default`,
`supported`, `min_proof_version`, אופציונלי `sunset_unix`) לצורכי אבחון וברירות
מחדל של SDK. לקוחות צריכים לשלוח את הכותרת במפורש; ה‑CLI משתמש כעת ב‑`1.1`
כברירת מחדל ומעביר overrides דרך `torii_api_version` בקובץ `client.toml`.

נקודת הקצה `/api_version` מחזירה את מחרוזת גרסת כותרת הבלוק הפעילה כטקסט פשוט.
אם genesis עדיין לא הוחל, היא מחזירה `503 SERVICE_UNAVAILABLE` והגוף
`genesis not applied`.

</div>

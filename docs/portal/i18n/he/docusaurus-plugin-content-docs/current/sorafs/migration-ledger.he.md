---
lang: he
direction: rtl
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/sorafs/migration-ledger.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9833a1cb2de209498ddafe31d0bef8c4761e9d17f1bd2eaead07237a2ba453e1
source_last_modified: "2026-01-04T10:50:53+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Hebrew (he) translation. Replace this content with the full translation. -->

---
id: migration-ledger
lang: he
direction: rtl
source: docs/portal/docs/sorafs/migration-ledger.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


> מותאם מ-[`docs/source/sorafs/migration_ledger.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/migration_ledger.md).

# יומן ההגירה של SoraFS

יומן זה משקף את יומן שינויי ההגירה שנלכד ב-RFC של ארכיטקטורת SoraFS. הרשומות
מקובצות לפי אבני דרך ומפרטות את חלון התוקף, הצוותים המושפעים והפעולות הנדרשות.
עדכונים לתוכנית ההגירה חייבים לעדכן גם את הדף הזה וגם את ה-RFC
(`docs/source/sorafs_architecture_rfc.md`) כדי לשמור על יישור בין הצרכנים במורד הזרם.

| אבן דרך | חלון תוקף | תקציר שינוי | צוותים מושפעים | פעולות | סטטוס |
|---------|-----------|-------------|----------------|--------|-------|
| M1 | שבועות 7–12 | CI אוכף fixtures דטרמיניסטיים; הוכחות alias זמינות ב-staging; tooling חושף expectation flags מפורשים. | Docs, Storage, Governance | לוודא שה-fixtures נשארים חתומים, לרשום aliases ברישום staging, לעדכן רשימות release עם אכיפת `--car-digest/--root-cid`. | ⏳ ממתין |

פרוטוקולי ישיבות של מישור הבקרה של הממשל המתייחסים לאבני דרך אלה נמצאים תחת
`docs/source/sorafs/`. על הצוותים להוסיף נקודות מתוארכות מתחת לכל שורה כאשר
מתרחשים אירועים משמעותיים (למשל רישומי alias חדשים, retrospectives של תקריות registry)
כדי לספק עקבות ביקורתיות.

## עדכונים אחרונים

- 2025-11-01 — הופץ `migration_roadmap.md` למועצת הממשל ולרשימות המפעילים לסקירה;
  ממתין לאישור בישיבת המועצה הבאה (ref: `docs/source/sorafs/council_minutes_2025-10-29.md`).
- 2025-11-02 — ISI של רישום Pin Registry אוכף כעת אימות chunker/מדיניות משותף דרך
  helpers של `sorafs_manifest`, כך שהנתיבים on-chain נשארים מיושרים עם בדיקות Torii.
- 2026-02-13 — נוספו שלבי rollout של provider advert (R0–R3) ליומן ופורסמו לוחות הבקרה
  והנחיות המפעיל הרלוונטיות
  (`provider_advert_rollout.md`, `grafana_sorafs_admission.json`).

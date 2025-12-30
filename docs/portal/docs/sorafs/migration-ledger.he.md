<!-- Auto-generated stub for Hebrew (he) translation. Replace this content with the full translation. -->

---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/migration-ledger.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0c5ad15044887087c487d93762739fa9241f384a634d178aa76d1dcf8cb1cdb0
source_last_modified: "2025-11-09T14:34:44.965608+00:00"
translation_last_reviewed: 2025-12-30
---

---
title: יומן ההגירה של SoraFS
description: יומן שינוי קנוני העוקב אחר כל אבן דרך של הגירה, בעלי אחריות והמשך נדרש.
---

> מותאם מ-[`docs/source/sorafs/migration_ledger.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/migration_ledger.md).

# יומן ההגירה של SoraFS

יומן זה משקף את יומן שינויי ההגירה שנלכד ב-RFC של ארכיטקטורת SoraFS. הרשומות
מקובצות לפי אבני דרך ומפרטות את חלון התוקף, הצוותים המושפעים והפעולות הנדרשות.
עדכונים לתוכנית ההגירה חייבים לעדכן גם את הדף הזה וגם את ה-RFC
(`docs/source/sorafs_architecture_rfc.md`) כדי לשמור על יישור בין הצרכנים במורד הזרם.

| אבן דרך | חלון תוקף | תקציר שינוי | צוותים מושפעים | פעולות | סטטוס |
|---------|-----------|-------------|----------------|--------|-------|
| M0 | שבועות 1–6 | Fixtures של chunker פורסמו; pipelines מפיקים חבילות CAR + manifest לצד artefacts legacy; נוצרו רשומות ביומן ההגירה. | Docs, DevRel, SDKs | לאמץ `sorafs_manifest_stub` עם expectation flags, לתעד רשומות ביומן זה, לשמר את ה-CDN הישן. | ✅ פעיל |
| M1 | שבועות 7–12 | CI אוכף fixtures דטרמיניסטיים; הוכחות alias זמינות ב-staging; tooling חושף expectation flags מפורשים. | Docs, Storage, Governance | לוודא שה-fixtures נשארים חתומים, לרשום aliases ברישום staging, לעדכן רשימות release עם אכיפת `--car-digest/--root-cid`. | ⏳ ממתין |
| M2 | שבועות 13–20 | Pinning מגובה registry הופך למסלול הראשי; artefacts legacy עוברים לקריאה בלבד; gateways מתעדפים הוכחות registry. | Storage, Ops, Governance | לנתב pinning דרך registry, להקפיא hosts legacy, לפרסם הודעות הגירה למפעילים. | ⏳ ממתין |
| M3 | שבוע 21+ | אכיפת גישה מבוססת alias בלבד; observability מתריעה על parity של registry; CDN legacy מפורק. | Ops, Networking, SDKs | להסיר DNS legacy, לסובב URLs בקאש, לנטר dashboards parity, לעדכן ברירות מחדל של SDK. | ⏳ ממתין |
| R0–R3 | 2025-03-31 → 2025-07-01 | שלבי אכיפה של provider advert: R0 צפייה, R1 אזהרה, R2 אכיפת handles/capabilities קנוניים, R3 ניקוי payloads legacy. | Observability, Ops, SDKs, DevRel | לייבא `grafana_sorafs_admission.json`, לעקוב אחרי checklist המפעיל ב-`provider_advert_rollout.md`, לתזמן חידושי advert 30+ ימים לפני שער R2. | ⏳ ממתין |

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

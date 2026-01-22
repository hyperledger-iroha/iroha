---
lang: he
direction: rtl
source: docs/source/sorafs/migration_ledger.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a0e479cae4018bbbc689fba16e2e59f93af50f1fad35509a65bce80e09e62186
source_last_modified: "2026-01-04T10:50:53.657290+00:00"
translation_last_reviewed: 2026-01-22
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/source/sorafs/migration_ledger.md -->

# יומן נדידת SoraFS

יומן זה משקף את יומן השינויים בנדידה כפי שנלכד ב‑RFC לארכיטקטורת SoraFS.
הרשומות מקובצות לפי אבני דרך ומפרטות את חלון התוקף, הצוותים שהושפעו ופעולות
נדרשות. עדכונים לתכנית הנדידה חייבים לשנות גם קובץ זה וגם את ה‑RFC
(`docs/source/sorafs_architecture_rfc.md`) כדי לשמור על יישור עם צרכנים במורד הזרם.

| אבן דרך | חלון תוקף | תקציר שינוי | צוותים מושפעים | פריטי פעולה | סטטוס |
|-----------|------------------|----------------|----------------|--------------|--------|
| M1 | שבועות 7–12 | CI אוכף fixtures דטרמיניסטיים; הוכחות alias זמינות ב‑staging; כלי העבודה חושפים דגלי ציפייה מפורשים. | Docs, Storage, Governance | ודאו שה‑fixtures נשארים חתומים, רשמו aliases ברשומת staging, עדכנו רשימות שחרור עם אכיפת `--car-digest/--root-cid`. | ⏳ ממתין |

פרוטוקולי control plane של הממשל שמפנים לאבני הדרך הללו נשמרים תחת
`docs/source/sorafs/`. צוותים צריכים להוסיף נקודות תבליט מתוארכות מתחת לכל שורה
כאשר אירועים משמעותיים מתרחשים (למשל רישומי alias חדשים, רטרוספקטיבות על תקלות
ברג'יסטרי) כדי לספק מסלול ביקורת ניתן לאימות.

## עדכונים אחרונים

- 2025-11-01 — הופץ `migration_roadmap.md` למועצת הממשל ולרשימות מפעילים לביקורת; ממתינים לאישור בישיבת המועצה הבאה (ref: מעקב `docs/source/sorafs/council_minutes_2025-10-29.md`).
- 2025-11-02 — רישום ISI של Pin Registry כעת אוכף אימות chunker/מדיניות משותף באמצעות עזרי `sorafs_manifest`, ושומר על יישור נתיבי on-chain מול בדיקות Torii.
- 2026-02-13 — נוספו שלבי rollout של פרסום ספקים (R0–R3) ליומן ופורסמו הדשבורדים והנחיות המפעיל המשויכים (`provider_advert_rollout.md`, `grafana_sorafs_admission.json`).

</div>

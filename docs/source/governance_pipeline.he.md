---
lang: he
direction: rtl
source: docs/source/governance_pipeline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 750f2fef72cdaf0a1160cb4f49158e4f43a11516b735a8c9d5946a524443521c
source_last_modified: "2025-12-27T09:09:07.696213+00:00"
translation_last_reviewed: 2026-01-21
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/source/governance_pipeline.md -->

% Governance Pipeline (Iroha 2 and SORA Parliament)

# מצב נוכחי (v1)
- הצעות ממשל רצות כך: מציע → משאל → ספירה → חקיקה. חלונות משאל וספי השתתפות/אישור
  נאכפים כפי שמתואר ב‑`gov.md`; locks הם extend‑only ומשתחררים בעת פקיעה.
- בחירת פרלמנט משתמשת בהגרלות מבוססות VRF עם סדר דטרמיניסטי וגבולות קדנציה; כאשר
  אין roster שמור, Torii גוזר fallback באמצעות תצורת `gov.parliament_*`.
  gating של מועצה ובדיקות quorum נבדקים ב‑`gov_parliament_bodies` / `gov_pipeline_sla`.
- מצבי הצבעה: ZK (ברירת מחדל, דורש VK `Active` עם bytes inline) ו‑Plain (משקל
  ריבועי). אי‑התאמות מצב נדחות; יצירה/הארכה של lock הן מונוטוניות בשני המצבים
  עם בדיקות רגרסיה עבור הצבעות ZK ו‑Plain מחדש.
- התנהגות לא תקינה של מאמתים מטופלת דרך צינור הראיות (`/v1/sumeragi/evidence*`,
  עזרי CLI) עם העברות joint‑consensus שנאכפות ע"י `NextMode` + `ModeActivationHeight`.
- שמות־מרחב מוגנים, hooks לשדרוג runtime, והודאת governance manifest מתועדים
  ב‑`governance_api.md` ומכוסים בטלמטריה (`governance_manifest_*`,
  `governance_protected_namespace_total`).

# בתנועה / backlog
- פרסום ארטיפקטי הגרלת VRF (seed, proof, roster מסודר, alternates) וקידוד כללי
  החלפה לנעדרים; הוספת fixtures golden להגרלה ולהחלפות.
- אכיפת Stage‑SLA עבור גופי הפרלמנט (rules → agenda → study → review → jury → enact)
  דורשת טיימרים מפורשים, מסלולי הסלמה, ומוני טלמטריה.
- הצבעה בסוד/commit–reveal של policy‑jury וביקורות עמידות לשוחד עדיין לא הוטמעו.
- מכפילי role‑bond, slashing על התנהגות חריגה לגופים בסיכון גבוה, וזמני צינון
  בין תקופות שירות דורשים צנרת תצורה ובדיקות.
- איטום נתיב הממשל ושערי חלון משאל/turnout מתועדים ב‑`gov.md`/`status.md`; שמרו
  את רשומות ה‑roadmap מעודכנות ככל שבדיקות קבלה נוחתות.

</div>

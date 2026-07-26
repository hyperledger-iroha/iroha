---
lang: he
direction: rtl
source: docs/source/project_tracker/norito_streaming_post_mvp.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4e6f0d09f24f7d3111fc53856393a0ad4be93f5a2f45c5150f053f1e153f1ec6
source_last_modified: "2026-01-06T15:14:01.036336+00:00"
translation_last_reviewed: 2026-01-21
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/source/project_tracker/norito_streaming_post_mvp.md -->

<!-- Tracker detailing post-MVP Norito Streaming follow-ups. -->

# Norito Streaming — רשימת משימות לאחר MVP

| מזהה | משימה | בעלים | יעד | סטטוס | צעדים הבאים | הערות |
|----|------|--------|--------|--------|-------------|-------|
| NSC-28b | אכיפת סבילות סנכרון A/V של ±10 ms באמצעות טלמטריית מאמתים ומבקרי סגמנטים | Streaming Runtime TL, Telemetry Ops | Q2 2026 | 🟡 בתכנון | טיוטת מפרט אותות טלמטריה (דליי/סטיות) ותיאום סקירת אינסטרומנטציה למאמתים (שבוע 2 במרץ). | מתווה מפרט: `docs/source/project_tracker/nsc28b_av_sync_telemetry.md`. להוסיף מדדי jitter/drift ב‑`StreamingTelemetry`, גיידינג ל‑`SegmentAuditor`, ולהרחיב בדיקות loopback/impairment לדחייה דטרמיניסטית. |
| NSC-30a | תכנון מסגרת תמריצים ומוניטין לריליי (כלכלה + סכמות) | Streaming WG, Economics WG | Q2 2026 | 🟡 בתכנון | להניע סדנת WG משותפת (9 במרץ) ולשייך כלכלנים למתווה מסמך מודל. | מסמך הכנה לסדנה: `docs/source/project_tracker/nsc30a_relay_incentive_design.md`. להפיק whitepaper תמריצים, סכמות Norito לאישורי ריליי, שדות טלמטריה, וסימולציות churn/כשל. |
| NSC-30b | יישום צינור ניקוד ותמרוץ ריליי מקצה לקצה | Streaming WG, Torii Team | Q3 2026 | ⚪ ממתין ל‑NSC-30a | חסום עד אישור עיצוב NSC-30a; להכין טיוטת הצעת שינוי סכימה ל‑Torii. | תלוי בפלט NSC-30a; לעקוב במסמך זה וב‑Torii backlog לאחר הקפאת העיצוב. |
| NSC-37b | יישום הוכחות ZK לכרטיסים ואימות בצד ה‑host | ZK Working Group, Core Host Team | Q3 2026 | ⚪ ממתין ל‑NSC-37a | להקים branch אבטיפוס ב‑`iroha_zkp_halo2` ולמנות hooks נדרשים ב‑host. | הערות מימוש ירחיבו את `nsc37a_zk_ticket_schema.md` לאחר סיום הסכמה; להבטיח שתכנית האימות תואמת את roadmap של Core Host. |
| NSC-42 | השלמת סקירת פטנטים/דין לקודק מבוסס בלוקים | Legal & Standards | Q2 2026 | 🟢 הושלם | נסגר — חוות דעת משפטית נרשמה והגייטינג נשאר opt‑in. | חתימה תועדה ב‑`docs/source/soranet/nsc-42-legal.md`; CABAC נשאר מאחורי `ENABLE_CABAC` + `[streaming.codec]`, trellis עדיין מושבת, ו‑rANS bundled דורש `ENABLE_RANS_BUNDLES`, עם עמדה משפטית מתועדת ב‑NOTICE/PATENTS/EXPORT. |
| NSC-55 | אימות טבלאות אתחול rANS ופרסום העמדה הפטנטית | Codec Team | Q2 2026 | 🟢 הושלם | נסגר — טבלאות דטרמיניסטיות וחבילות ראיות קיימות בעץ. | טבלאות/גנרטור קנוניים נבדקו וה‑bench/CSV/report נחתו לפי `docs/source/project_tracker/nsc55_rans_validation_plan.md`, המספקים עמדה רבת־שחזור ל‑rANS עבור פריסות CABAC‑אופציונליות. |

</div>

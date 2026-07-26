---
lang: he
direction: rtl
source: docs/source/soranet/templates/downgrade_communication_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0385a25caa5cdefcab7288602522ad165523dc14cd3eb56114f188c119b55679
source_last_modified: "2026-01-03T18:08:01.997331+00:00"
translation_last_reviewed: 2026-01-22
title: תבנית הודעת הורדה בדרגה של SoraNet
summary: נוסח בסיס להודעה למפעילים ולצרכני SDK על הורדות דרגה זמניות של SoraNet.
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/source/soranet/templates/downgrade_communication_template.md -->

**נושא:** הורדת דרגה של SoraNet באזור {{ region }} ({{ incident_id }})

**סיכום:**

- **מתי:** {{ start_time }} UTC – {{ expected_end_time }} UTC
- **היקף:** מעגלים באזור {{ region }} חוזרים זמנית למצב ישיר בזמן שאנו מטפלים ב‑{{ root_cause }}.
- **השפעה:** זמן השהיה מוגבר ואנונימיות מופחתת למשיכות SoraFS; אכיפת GAR נשארת פעילה.

**פעולות מפעיל:**

1. להחיל את ה‑override שפורסם (`transport_policy=direct-only`) עד להודעת התאוששות.
2. לנטר את דשבורדי ה‑brownout (`sorafs_orchestrator_policy_events_total`, `soranet_privacy_circuit_events_total`).
3. לתעד צעדי מיתון ביומן ה‑GAR.

**מסרים ל‑SDK / לקוח:**

- באנר דף סטטוס: "מעגלי SoraNet באזור {{ region }} נמצאים בהורדת דרגה זמנית. התעבורה נשארת פרטית אך לא אנונימית." 
- כותרת API: `Soranet-Downgrade: region={{ region }}; incident={{ incident_id }}`

**עדכון הבא:** {{ follow_up_time }} UTC או מוקדם יותר.

נא להפנות כל שאלה לגשר הממשל (`#soranet-incident`).

</div>

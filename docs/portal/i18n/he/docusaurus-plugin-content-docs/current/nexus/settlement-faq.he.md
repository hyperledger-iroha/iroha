---
lang: he
direction: rtl
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/nexus/settlement-faq.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ec6a9d458c43fee10193ea7eda68d3d1b151113fd15f9cb3f518af77e94e76ef
source_last_modified: "2025-11-14T04:43:20.566836+00:00"
translation_last_reviewed: 2026-01-30
---

העמוד הזה משקף את ה-FAQ הפנימי של settlement (`docs/source/nexus_settlement_faq.md`) כדי שקוראי הפורטל יוכלו לעיין באותה הנחיה בלי לחפש במונו-רפו. הוא מסביר כיצד Settlement Router מטפל בתשלומים, אילו מדדים לנטר, וכיצד ה-SDK צריכים לשלב את מטעני Norito.

## נקודות מרכזיות

1. **מיפוי lane** — כל dataspace מצהיר על `settlement_handle` (`xor_global`, `xor_lane_weighted`, `xor_hosted_custody` או `xor_dual_fund`). עיינו בקטלוג ה-lane העדכני תחת `docs/source/project_tracker/nexus_config_deltas/`.
2. **המרה דטרמיניסטית** — ה-router ממיר את כל ה-settlement ל-XOR דרך מקורות נזילות מאושרי ממשל. lanes פרטיות מממנות מראש מאגרי XOR; haircuts חלים רק כשהמאגר חורג מהמדיניות.
3. **טלמטריה** — עקבו אחרי `nexus_settlement_latency_seconds`, מוני המרה ומדדי haircut. לוחות המחוונים נמצאים ב-`dashboards/grafana/nexus_settlement.json` וההתראות ב-`dashboards/alerts/nexus_audit_rules.yml`.
4. **ראיות** — ארכבו קונפיגים, לוגים של ה-router, יצואי טלמטריה ודוחות התאמה לצורכי ביקורת.
5. **אחריות SDK** — כל SDK חייב לחשוף עזרי settlement, מזהי lane ומקודדי מטען Norito כדי לשמור על התאמה עם ה-router.

## תהליכי דוגמה

| סוג lane | ראיות לאיסוף | מה זה מוכיח |
|-----------|--------------------|----------------|
| פרטית `xor_hosted_custody` | לוג של ה-router + `nexus_settlement_latency_seconds{lane}` + `settlement_router_haircut_total{lane}` | מאגרי CBDC מחייבים XOR דטרמיניסטי וה-haircuts נשארים בתוך המדיניות. |
| ציבורית `xor_global` | לוג של ה-router + הפניה ל-DEX/TWAP + מדדי לטנסי/המרה | מסלול הנזילות המשותף תימחר את ההעברה לפי ה-TWAP שפורסם עם haircut אפס. |
| היברידית `xor_dual_fund` | לוג של ה-router שמציג את החלוקה public מול shielded + מוני טלמטריה | השילוב shielded/public כיבד את יחסי הממשל ותיעד את ה-haircut שהוחל על כל רגל. |

## צריכים עוד פרטים?

- FAQ מלא: `docs/source/nexus_settlement_faq.md`
- מפרט settlement router: `docs/source/settlement_router.md`
- פלייבוק מדיניות CBDC: `docs/source/cbdc_lane_playbook.md`
- Runbook תפעול: [תפעול Nexus](./nexus-operations)

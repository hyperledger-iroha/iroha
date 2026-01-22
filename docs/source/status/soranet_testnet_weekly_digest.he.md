---
lang: he
direction: rtl
source: docs/source/status/soranet_testnet_weekly_digest.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e87ff17c92df5f22db362da249242d7570e9c6e7e53c6d39452061d846cd1757
source_last_modified: "2026-01-03T18:07:58.606006+00:00"
translation_last_reviewed: 2026-01-22
title: תבנית תקציר שבועי של SoraNet Testnet
summary: רשימת בדיקה מובנית לעדכוני סטטוס שבועיים של SNNet-10.
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/source/status/soranet_testnet_weekly_digest.md -->

# תקציר שבועי של SoraNet Testnet — השבוע של {{ week_start }}

## תמונת מצב רשת

- מספר relays: {{ relay_count }} (יעד {{ target_relay_count }})
- relays תואמי PQ: {{ pq_relays }} ({{ pq_ratio }}%)
- גיל ממוצע של סבב guards: {{ guard_rotation_hours }} שעות
- אירועי brownout השבוע: {{ brownout_count }}

## מדדי מפתח

| מדד | ערך | יעד | הערות |
|--------|-------|--------|-------|
| `soranet_privacy_circuit_events_total{kind="downgrade"}` (ל‑דקה) | {{ downgrade_rate }} | 0 | {{ downgrade_notes }} |
| `sorafs_orchestrator_policy_events_total{outcome="brownout"}` (ל‑30 דק׳) | {{ brownout_rate }} | <0.05 | {{ policy_notes }} |
| זמן פתרון חציוני ל‑PoW | {{ pow_median_ms }} ms | ≤300 ms | {{ pow_notes }} |
| RTT של מעגל p95 | {{ rtt_p95_ms }} ms | ≤200 ms | {{ rtt_notes }} |

## תאימות & GAR

- גרסת קטלוג opt-out: {{ opt_out_tag }}
- אישורי תאימות מפעילים שהתקבלו השבוע: {{ compliance_forms }}
- פעולות פתוחות: {{ compliance_actions }}

## תקריות ותרגילים

- [ ] תרגיל brownout בוצע (תאריך/שעה, תוצאה)
- [ ] תבנית תקשורת downgrade הופעלה (קישור לרישום)
- [ ] חזרת rollback בוצעה (תקציר)

## אבני דרך מתקרבות

- {{ milestone_one }}
- {{ milestone_two }}

## הערות והחלטות

- {{ decision_one }}
- {{ decision_two }}

</div>

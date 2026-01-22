---
lang: he
direction: rtl
source: docs/source/soranet/reports/pow_resilience.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c8c1066361e5b9c10dcdd40ba8a6fc99c7c63038eac3900c9b401479f2f4bb12
source_last_modified: "2025-11-15T12:19:16.080861+00:00"
translation_last_reviewed: 2026-01-21
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/source/soranet/reports/pow_resilience.md -->

# דו"ח Soak לעמידות PoW

הבדיקה `volumetric_dos_soak_preserves_puzzle_and_latency_slo` ב‑
`tools/soranet-relay/tests/adaptive_and_puzzle.rs` מפעילה את שער Argon2 של
SNNet-6a תחת עומס מתמשך. ה‑harness מפעיל את יישום `DoSControls` עם חלון burst
של 6 בקשות ו‑SLO של 300 ms ללחיצת יד, תוך שימוש במדיניות הפאזל לייצור
(זיכרון 4 MiB, מסלול יחיד, time cost 1) בדרגת קושי 6.

| שלב | ניסיונות | דגימות latency (ms) | Cooldown | הערות |
|-------|----------|----------------------|----------|-------|
| Burst soak | 6 | 190, 190, 190, 190, 190, 190 | cooldown מרוחק 4 ש׳ | Tickets נוצרים ומאומתים (`puzzle::mint_ticket`/`verify`) תוך שמירה על SLO של 300 ms. |
| Slowloris penalty | 3 | 340, 340, 340 | קנס slowloris של 5 ש׳ | חריגה מה‑SLO שלוש פעמים מפעילה את קנס ה‑slowloris ומרשמת cooldown פעיל במטריקות ה‑relay. |

בשני השלבים, קושי הפאזל משקף את קושי ה‑PoW, ומבטיח שכרטיסי Argon2 נשארים
מיושרים עם החלטות המדיניות האדפטיבית גם תחת נסיונות DoS בנפח גבוה.

</div>

---
lang: he
direction: rtl
source: docs/source/soranet/reports/circuit_stability.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cb8844aac273cd70d69eb927e6c8f22487fc47641bf4bff0afecdc9229898fdf
source_last_modified: "2025-11-15T12:19:16.080861+00:00"
translation_last_reviewed: 2026-01-21
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/source/soranet/reports/circuit_stability.md -->

# דו"ח Soak ליציבות מעגלים

תרגיל ה‑soak הזה מאמת את מנהל מחזור החיים החדש של מעגלי SoraNet שנשלח עבור
SNNet-5. ה‑harness מיושם ב‑`crates/sorafs_orchestrator/src/soranet.rs` תחת
הבדיקה `circuit_manager_soak_maintains_latency_stability` ומדמה שלוש החלפות
Guard עם דגימות latency דטרמיניסטיות.

| סבב | Guard Relay | דגימות (ms)     | ממוצע (ms) | מינימום (ms) | מקסימום (ms) |
|----------|-------------|------------------|--------------|----------|----------|
| 0        | 0x04…04     | 50, 52, 49       | 50.33        | 49       | 52       |
| 1        | 0x04…04     | 51, 53, 50       | 51.33        | 50       | 53       |
| 2        | 0x04…04     | 52, 54, 51       | 52.33        | 51       | 54       |

היסטוריית ההחלפות נשארת בתוך חלון של 2 ms לאורך שלוש התחדשויות, וכך עומדת
בדרישת ה‑roadmap שה‑latency תישאר יציבה לאורך לפחות שלוש החלפות guard.
הבדיקה גם מאמתת שהמנהל רושם טלמטריית סבב דטרמיניסטית ואוכף את ה‑TTL המוגדר.

</div>

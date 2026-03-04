---
lang: he
direction: rtl
source: docs/source/soranet/snnet15_m3_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e16ecf29a5c6178b63a5e347de748a83e53a550f4a902c4709d9c5dd8be6c65c
source_last_modified: "2026-01-30T13:33:43.991472+00:00"
translation_last_reviewed: 2026-01-21
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/source/soranet/snnet15_m3_runbook.md -->

# SNNet-15M3 — מוכנות GA לשער

נוהל זה צורך את ראיות הבטא M2 ומפיק את חבילת ה‑GA עבור ממשל:
תקצירי autoscale/worker, יעד SLA וקישורים להוכחות הבטא.

## צעדים
- הפקת חבילת GA:
  - `cargo xtask soranet-gateway-m3 --m2-summary artifacts/soranet/gateway_m2/beta/gateway_m2_summary.json --autoscale-plan <plan.json> --worker-pack <bundle.tgz> --out artifacts/soranet/gateway_m3 --sla-target 99.95%`
- אימות פלטי GA:
  - `gateway_m3_summary.json` / `.md` מכילים digests של BLAKE3 עבור תוכנית autoscale וחבילת worker.
  - השדה `m2_summary` מפנה לשורש הראיות המדויק של הבטא.
  - `sla_target` מתעד את ה‑SLO לייצור שהוסכם עם SRE.

## רשימת יציאה
- digests של תוכנית autoscale וחבילת worker מוטמעים בסיכום.
- סיכום M2 מיוחס וקפוא.
- יעד SLA מתועד; דשבורדים/שערים עודכנו להתאמה.

</div>

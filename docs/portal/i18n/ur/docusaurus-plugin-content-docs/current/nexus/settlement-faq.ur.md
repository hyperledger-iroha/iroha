---
lang: ur
direction: rtl
source: docs/portal/docs/nexus/settlement-faq.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-settlement-faq
title: Settlement FAQ
description: آپریٹرز کے لیے جوابات جو settlement routing، XOR conversion، ٹیلی میٹری، اور آڈٹ ثبوت کا احاطہ کرتے ہیں۔
---

یہ صفحہ اندرونی settlement FAQ (`docs/source/nexus_settlement_faq.md`) کی عکاسی کرتا ہے تاکہ پورٹل کے قارئین اسی رہنمائی کو mono-repo میں تلاش کیے بغیر دیکھ سکیں۔ یہ وضاحت کرتا ہے کہ Settlement Router ادائیگیوں کو کیسے پروسیس کرتا ہے، کن میٹرکس کی نگرانی کرنی ہے، اور SDKs کو Norito payloads کیسے ضم کرنے چاہیئیں۔

## نمایاں نکات

1. **lane میپنگ** — ہر dataspace ایک `settlement_handle` کا اعلان کرتا ہے (`xor_global`، `xor_lane_weighted`، `xor_hosted_custody` یا `xor_dual_fund`)۔ `docs/source/project_tracker/nexus_config_deltas/` میں تازہ ترین lane catalog دیکھیں۔
2. **متعین تبدیلی** — router تمام settlements کو governance سے منظور شدہ liquidity sources کے ذریعے XOR میں تبدیل کرتا ہے۔ نجی lanes پہلے سے XOR buffers کو فنڈ کرتی ہیں؛ haircuts صرف تب لاگو ہوتے ہیں جب buffers پالیسی سے باہر جائیں۔
3. **ٹیلی میٹری** — `nexus_settlement_latency_seconds`، conversion counters، اور haircut gauges مانیٹر کریں۔ dashboards `dashboards/grafana/nexus_settlement.json` میں اور alerts `dashboards/alerts/nexus_audit_rules.yml` میں ہیں۔
4. **ثبوت** — audits کے لیے configs، router logs، telemetry exports، اور reconciliation reports محفوظ کریں۔
5. **SDK ذمہ داریاں** — ہر SDK کو settlement helpers، lane IDs، اور Norito payload encoders فراہم کرنے ہوں گے تاکہ router کے ساتھ برابری رہے۔

## مثال کے بہاؤ

| lane کی قسم | جمع کرنے والا ثبوت | یہ کیا ثابت کرتا ہے |
|-----------|--------------------|----------------|
| نجی `xor_hosted_custody` | router log + `nexus_settlement_latency_seconds{lane}` + `settlement_router_haircut_total{lane}` | CBDC buffers متعین XOR ڈیبٹ کرتے ہیں اور haircuts پالیسی کے اندر رہتے ہیں۔ |
| عوامی `xor_global` | router log + DEX/TWAP حوالہ + latency/conversion metrics | مشترکہ liquidity راستے نے منتقل شدہ رقم کو شائع شدہ TWAP پر zero haircut کے ساتھ قیمت دی۔ |
| ہائبرڈ `xor_dual_fund` | router log جو public بمقابلہ shielded تقسیم دکھائے + telemetry counters | shielded/public امتزاج نے governance ratios کی پابندی کی اور ہر حصے پر لاگو haircut کو ریکارڈ کیا۔ |

## مزید تفصیل چاہیے؟

- مکمل FAQ: `docs/source/nexus_settlement_faq.md`
- Settlement router spec: `docs/source/settlement_router.md`
- CBDC پالیسی playbook: `docs/source/cbdc_lane_playbook.md`
- Operations runbook: [Nexus operations](./nexus-operations)

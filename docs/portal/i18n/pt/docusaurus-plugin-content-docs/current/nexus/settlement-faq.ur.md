---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/settlement-faq.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-settlement-faq
título: Perguntas frequentes sobre liquidação
description: آپریٹرز کے لیے جوابات جو liquidação roteamento, conversão XOR, ٹیلی میٹری, اور آڈٹ ثبوت کا احاطہ کرتے ہیں۔
---

یہ صفحہ اندرونی FAQ sobre liquidação (`docs/source/nexus_settlement_faq.md`) کو mono-repo میں تلاش کیے بغیر دیکھ سکیں۔ یہ وضاحت کرتا ہے کہ Roteador de liquidação ادائیگیوں کو کیسے پروسیس کرتا ہے, کن میٹرکس کی نگرانی کرنی ہے، اور SDKs کو cargas úteis Norito کیسے ضم کرنے چاہیئیں۔

## نمایاں نکات

1. **lane میپنگ** — ہر dataspace ایک `settlement_handle` کا اعلان کرتا ہے (`xor_global`, `xor_lane_weighted`, `xor_hosted_custody` یا `xor_dual_fund`)۔ `docs/source/project_tracker/nexus_config_deltas/` میں تازہ ترین catálogo de pistas دیکھیں۔
2. **متعین تبدیلی** — roteador تمام liquidações کو governança سے منظور شدہ fontes de liquidez کے ذریعے XOR میں تبدیل کرتا ہے۔ As pistas são os buffers XOR e os buffers XOR cortes de cabelo صرف تب لاگو ہوتے ہیں جب buffers پالیسی سے باہر جائیں۔
3. **ٹیلی میٹری** — `nexus_settlement_latency_seconds`, contadores de conversão, e medidores de corte de cabelo مانیٹر کریں۔ painéis `dashboards/grafana/nexus_settlement.json` میں اور alertas `dashboards/alerts/nexus_audit_rules.yml` میں ہیں۔
4. **ثبوت** — audita configurações de configurações, registros de roteadores, exportações de telemetria e relatórios de reconciliação.
5. **SDK ذمہ داریاں** — ہر SDK کو ajudantes de liquidação, IDs de pista, e codificadores de carga útil Norito فراہم کرنے ہوں گے تاکہ roteador کے ساتھ برابری رہے۔

## مثال کے بہاؤ

| pista کی قسم | Máquinas de lavar e secar | یہ کیا ثابت کرتا ہے |
|-----------|--------------------|----------------|
| Não `xor_hosted_custody` | log do roteador + `nexus_settlement_latency_seconds{lane}` + `settlement_router_haircut_total{lane}` | Buffers CBDC متعین XOR ڈیبٹ کرتے ہیں اور cortes de cabelo پالیسی کے اندر رہتے ہیں۔ |
| Nome `xor_global` | log do roteador + DEX/TWAP حوالہ + métricas de latência/conversão | مشترکہ liquidez راستے نے منتقل شدہ رقم کو شائع شدہ TWAP پر corte de cabelo zero کے ساتھ قیمت دی۔ |
| ہائبرڈ `xor_dual_fund` | log do roteador público público بمقابلہ blindado تقسیم دکھائے + contadores de telemetria | blindado/público امتزاج نے índices de governança کی پابندی کی اور ہر حصے پر لاگو corte de cabelo کو ریکارڈ کیا۔ |

## مزید تفصیل چاہیے؟

- Perguntas frequentes: `docs/source/nexus_settlement_faq.md`
Especificações do roteador de liquidação: `docs/source/settlement_router.md`
- Manual de instruções do CBDC: `docs/source/cbdc_lane_playbook.md`
- Runbook de operações: [operações Nexus](./nexus-operations)
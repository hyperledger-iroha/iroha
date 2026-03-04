---
lang: es
direction: ltr
source: docs/portal/docs/nexus/settlement-faq.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: preguntas frecuentes sobre nexus-settlement
título: Preguntas frecuentes sobre acuerdos
descripción: آپریٹرز کے لیے جوابات جو liquidación de enrutamiento, conversión XOR, ٹیلی میٹری، اور آڈٹ ثبوت کا احاطہ کرتے ہیں۔
---

Preguntas frecuentes sobre acuerdos sobre acuerdos (`docs/source/nexus_settlement_faq.md`) mono-repo میں تلاش کیے بغیر دیکھ سکیں۔ یہ وضاحت کرتا ہے کہ Settlement Router ادائیگیوں کو کیسے پروسیس کرتا ہے، کن میٹرکس کی نگرانی Varios SDK y cargas útiles Norito

## نمایاں نکات

1. **lane میپنگ** — ہر espacio de datos ایک `settlement_handle` کا اعلان کرتا ہے (`xor_global`، `xor_lane_weighted`، `xor_hosted_custody` یا `xor_dual_fund`)۔ `docs/source/project_tracker/nexus_config_deltas/` میں تازہ ترین catálogo de carriles دیکھیں۔
2. **متعین تبدیلی** — enrutador تمام liquidaciones کو gobernanza سے منظور شدہ fuentes de liquidez کے ذریعے XOR میں تبدیل کرتا ہے۔ نجی carriles پہلے سے XOR buffers کو فنڈ کرتی ہیں؛ cortes de pelo صرف تب لاگو ہوتے ہیں جب buffers پالیسی سے باہر جائیں۔
3. **ٹیلی میٹری** — `nexus_settlement_latency_seconds`, contadores de conversión, اور medidores de corte de pelo مانیٹر کریں۔ cuadros de mando `dashboards/grafana/nexus_settlement.json` میں اور alertas `dashboards/alerts/nexus_audit_rules.yml` میں ہیں۔
4. **ثبوت** — auditorías, configuraciones, registros del enrutador, exportaciones de telemetría, informes de conciliación
5. **SDK ذمہ داریاں** — ہر SDK کو liquidation helpers, ID de carril, اور Norito codificadores de carga útil فراہم کرنے ہوں گے تاکہ enrutador کے ساتھ برابری رہے۔

## مثال کے بہاؤ| carril کی قسم | جمع کرنے والا ثبوت | یہ کیا ثابت کرتا ہے |
|-----------|--------------------|----------------|
| Número `xor_hosted_custody` | registro del enrutador + `nexus_settlement_latency_seconds{lane}` + `settlement_router_haircut_total{lane}` | Buffers CBDC متعین XOR ڈیبٹ کرتے ہیں اور cortes de pelo پالیسی کے اندر رہتے ہیں۔ |
| عوامی `xor_global` | registro del enrutador + DEX/TWAP + métricas de latencia/conversión | مشترکہ liquidez راستے نے منتقل شدہ رقم کو شائع شدہ TWAP پر zero haircut کے ساتھ قیمت دی۔ |
| ہائبرڈ `xor_dual_fund` | registro de enrutador جو público بمقابلہ blindado تقسیم دکھائے + contadores de telemetría | blindado/público امتزاج نے ratios de gobernanza کی پابندی کی اور ہر حصے پر لاگو corte de pelo کو ریکارڈ کیا۔ |

## مزید تفصیل چاہیے؟

- Preguntas frecuentes sobre el usuario: `docs/source/nexus_settlement_faq.md`
- Especificación del enrutador de liquidación: `docs/source/settlement_router.md`
- Manual de estrategias de CBDC: `docs/source/cbdc_lane_playbook.md`
- Runbook de operaciones: [operaciones Nexus](./nexus-operations)
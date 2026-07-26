---
lang: es
direction: ltr
source: docs/portal/docs/nexus/settlement-faq.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: preguntas frecuentes sobre nexus-settlement
título: الأسئلة الشائعة للتسوية
descripción: إجابات موجهة للمشغلين تغطي توجيه التسوية وتحويل XOR والقياس عن بعد وأدلة التدقيق.
---

تعكس هذه الصفحة الأسئلة الشائعة الداخلية للتسوية (`docs/source/nexus_settlement_faq.md`) بحيث يستطيع قراء البوابة مراجعة نفس Los archivos adjuntos son mono-repositorio. Utilice el enrutador de liquidación y el SDK de Norito.

## أبرز النقاط

1. **carril de navegación** — Está en el espacio de datos de `settlement_handle` (`xor_global`, `xor_lane_weighted`, `xor_hosted_custody`, `xor_dual_fund`). راجع أحدث كتالوج carril تحت `docs/source/project_tracker/nexus_config_deltas/`.
2. **تحويل حتمي** — El enrutador del dispositivo XOR está conectado al dispositivo. تقوم carriles الخاصة بتمويل مخازن XOR مسبقا؛ ولا تطبق cortes de pelo إلا عندما تنحرف المخازن خارج السياسة.
3. **القياس عن بعد** — راقب `nexus_settlement_latency_seconds` وعدادات التحويل ومقاييس corte de pelo. Utilice el `dashboards/grafana/nexus_settlement.json` y el `dashboards/alerts/nexus_audit_rules.yml`.
4. **الأدلة** — Haga clic en el enrutador y en el enrutador y en el dispositivo.
5. **مسؤوليات SDK** — يجب على كل SDK توفير أدوات مساعدة للتسوية ومعرفات lane y مشفري حمولات Norito للحفاظ Utilice el enrutador.

## أمثلة على التدفقات| نوع carril | الأدلة المطلوبة | ماذا يثبت |
|-----------|--------------------|----------------|
| خاصة `xor_hosted_custody` | Enrutador + `nexus_settlement_latency_seconds{lane}` + `settlement_router_haircut_total{lane}` | Hay una CBDC en XOR y cortes de pelo en el mercado. |
| عامة `xor_global` | سجل الـ router + مرجع DEX/TWAP + مقاييس زمن الاستجابة/التحويل | يثبت أن مسار السيولة المشترك سعّر التحويل وفق TWAP المنشور دون corte de pelo. |
| Aquí `xor_dual_fund` | سجل الـ enrutador يظهر تقسيم público مقابل blindado + عدادات القياس عن بعد | يثبت أن المزج بين blindado/público احترم نسب الحوكمة وسجل corte de pelo المطبق على كل جزء. |

## هل تحتاج مزيدا من التفاصيل؟

- Preguntas frecuentes: `docs/source/nexus_settlement_faq.md`
- Enrutador de liquidación: `docs/source/settlement_router.md`
- Información adicional CBDC: `docs/source/cbdc_lane_playbook.md`
- دليل التشغيل: [عمليات Nexus](./nexus-operations)
---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/settlement-faq.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-settlement-faq
título: الأسئلة الشائعة للتسوية
description: إجابات موجهة للمشغلين تغطي توجيه التسوية وتحويل XOR والقياس عن بعد وأدلة التدقيق.
---

Faça o download do cartão de crédito (`docs/source/nexus_settlement_faq.md`) para obter mais informações Você pode fazer isso no mono-repo. Você deve usar o Settlement Router e o SDK do SDK. Norito.

## أبرز النقاط

1. **lane ** - está no espaço de dados em `settlement_handle` (`xor_global` ou `xor_lane_weighted` ou `xor_hosted_custody` ou `xor_dual_fund`). راجع أحدث كتالوج lane تحت `docs/source/project_tracker/nexus_config_deltas/`.
2. **تحويل حتمي** — يحول الـ router جميع التسويات إلى XOR عبر مصادر السيولة المعتمدة من الحوكمة. تقوم lanes الخاصة بتمويل مخازن XOR مسبقا؛ E cortar cortes de cabelo é uma boa opção para cortes de cabelo.
3. **القياس عن بعد** — راقب `nexus_settlement_latency_seconds` e corte de cabelo e corte de cabelo. Você pode usar o `dashboards/grafana/nexus_settlement.json` e o `dashboards/alerts/nexus_audit_rules.yml`.
4. **الأدلة** — Instale o roteador e o roteador e o roteador sem fio e instale-o التدقيق.
5. **مسؤوليات SDK** — Você pode usar o SDK para instalar a pista e a pista e a rota Norito está conectado ao roteador.

## أمثلة على التدفقات

| Nenhuma pista | الأدلة المطلوبة | Máquinas de lavar |
|-----------|--------------------|----------------|
| Modelo `xor_hosted_custody` | Roteador + `nexus_settlement_latency_seconds{lane}` + `settlement_router_haircut_total{lane}` | Você pode usar o CBDC para usar o XOR e cortar cortes de cabelo por conta própria. |
| Modelo `xor_global` | Roteador de rede + DEX/TWAP + مقاييس زمن الاستجابة/التحويل | يثبت أن مسار السيولة المشترك سعّر التحويل وفق TWAP المنشور دون corte de cabelo. |
| Cabo `xor_dual_fund` | Roteador sem fio público com blindagem + roteador sem fio | Não há nenhum tipo de corte de cabelo protegido/público. |

## هل تحتاج مزيدا من التفاصيل؟

- Perguntas frequentes: `docs/source/nexus_settlement_faq.md`
- Roteador de liquidação padrão: `docs/source/settlement_router.md`
- Nome do CBDC: `docs/source/cbdc_lane_playbook.md`
- Nome de usuário: [Nexus](./nexus-operations)
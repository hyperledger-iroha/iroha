---
lang: he
direction: rtl
source: docs/portal/i18n/pt/docusaurus-plugin-content-docs/current/nexus/settlement-faq.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d7966a2fb6b94b46e457bcf34e49b5e3447c158a11e1a7915ba4c772e7bc10c7
source_last_modified: "2026-01-03T18:08:02+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/settlement-faq.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
id: nexus-settlement-faq
title: FAQ de Settlement
description: Respostas para operadores cobrindo roteamento de settlement, conversao XOR, telemetria e evidencia de auditoria.
---

Esta pagina espelha o FAQ interno de settlement (`docs/source/nexus_settlement_faq.md`) para que os leitores do portal revisem a mesma orientacao sem vasculhar o mono-repo. Explica como o Settlement Router processa pagamentos, quais metricas monitorar e como os SDKs devem integrar os payloads Norito.

## Destaques

1. **Mapeamento de lanes** - cada dataspace declara um `settlement_handle` (`xor_global`, `xor_lane_weighted`, `xor_hosted_custody` ou `xor_dual_fund`). Consulte o catalogo de lanes mais recente em `docs/source/project_tracker/nexus_config_deltas/`.
2. **Conversao deterministica** - o router converte todos os settlements para XOR por meio das fontes de liquidez aprovadas pela governanca. Lanes privadas prefinanciam buffers XOR; haircuts so se aplicam quando os buffers desviam da politica.
3. **Telemetria** - monitore `nexus_settlement_latency_seconds`, contadores de conversao e medidores de haircut. Dashboards ficam em `dashboards/grafana/nexus_settlement.json` e alertas em `dashboards/alerts/nexus_audit_rules.yml`.
4. **Evidencia** - arquive configs, logs do router, exportacoes de telemetria e relatorios de reconciliacao para auditorias.
5. **Responsabilidades do SDK** - cada SDK deve expor helpers de settlement, IDs de lane e codificadores de payloads Norito para manter paridade com o router.

## Fluxos de exemplo

| Tipo de lane | Evidencia a capturar | O que comprova |
|-----------|--------------------|----------------|
| Privada `xor_hosted_custody` | Log do router + `nexus_settlement_latency_seconds{lane}` + `settlement_router_haircut_total{lane}` | Buffers CBDC debitam XOR deterministica e haircuts ficam dentro da politica. |
| Publica `xor_global` | Log do router + referencia DEX/TWAP + metricas de latencia/conversao | O caminho de liquidez compartilhado fixou o preco da transferencia no TWAP publicado com zero haircut. |
| Hibrida `xor_dual_fund` | Log do router mostrando a divisao publico vs shielded + contadores de telemetria | A mistura shielded/publica respeitou os ratios de governanca e registrou o haircut aplicado a cada perna. |

## Precisa de mais detalhes?

- FAQ completo: `docs/source/nexus_settlement_faq.md`
- Especificacao do settlement router: `docs/source/settlement_router.md`
- Playbook de politica CBDC: `docs/source/cbdc_lane_playbook.md`
- Runbook de operacoes: [Operacoes do Nexus](./nexus-operations)

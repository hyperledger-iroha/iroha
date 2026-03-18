---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/settlement-faq.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-settlement-faq
título: FAQ de Liquidação
descrição: Respostas para operadores cobrindo roteamento de liquidação, conversação XOR, telemetria e evidências de auditoria.
---

Esta página reflete o FAQ interno de liquidação (`docs/source/nexus_settlement_faq.md`) para que os leitores do portal revisem a mesma orientação sem vasculhar o mono-repo. Explica como o Settlement Router processa pagamentos, quais métricas monitorar e como os SDKs devem integrar os payloads Norito.

##Destaques

1. **Mapeamento de pistas** - cada espaço de dados declara um `settlement_handle` (`xor_global`, `xor_lane_weighted`, `xor_hosted_custody` ou `xor_dual_fund`). Consulte o catálogo de pistas mais recente em `docs/source/project_tracker/nexus_config_deltas/`.
2. **Conversação determinística** - o roteador converte todos os assentamentos para XOR por meio das fontes de liquidez aprovadas pela governança. Faixas privadas pré-financiam buffers XOR; cortes de cabelo são aplicados quando os buffers desviam da política.
3. **Telemetria** - monitor `nexus_settlement_latency_seconds`, contadores de conversação e medidores de corte de cabelo. Dashboards ficam em `dashboards/grafana/nexus_settlement.json` e alertas em `dashboards/alerts/nexus_audit_rules.yml`.
4. **Evidência** - arquivo de configurações, logs do roteador, exportações de telemetria e relatórios de reconciliação para auditorias.
5. **Responsabilidades do SDK** - Cada SDK deve exportar ajudantes de liquidação, IDs de pista e codificadores de cargas úteis Norito para manter a paridade com o roteador.

## Fluxos de exemplo

| Tipo de pista | Evidência de captura | O que comprova |
|-----------|--------------------|----------------|
| Privada `xor_hosted_custody` | Log do roteador + `nexus_settlement_latency_seconds{lane}` + `settlement_router_haircut_total{lane}` | Buffers CBDC debitam XOR determinístico e cortes de cabelo ficam dentro da política. |
| Publica `xor_global` | Log do roteador + referência DEX/TWAP + métricas de latência/conversação | O caminho de liquidação compartilhado fixou o preço da transferência no TWAP publicado com zero corte de cabelo. |
| Híbrida `xor_dual_fund` | Log do roteador mostrando a divisão pública vs blindado + contadores de telemetria | A mistura blindada/publica respeita os índices de governança e registrados o corte de cabelo aplicado a cada perna. |

## Precisa de mais detalhes?

- FAQ completo: `docs/source/nexus_settlement_faq.md`
- Especificação do roteador de liquidação: `docs/source/settlement_router.md`
- Manual de política CBDC: `docs/source/cbdc_lane_playbook.md`
- Runbook de operações: [Operações do Nexus](./nexus-operations)
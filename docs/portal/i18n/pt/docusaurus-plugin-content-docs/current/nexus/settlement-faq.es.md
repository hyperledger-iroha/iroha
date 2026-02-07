---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/settlement-faq.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-settlement-faq
título: FAQ de Liquidação
descrição: Respostas para operadores que cobrem o processo de liquidação, conversão em XOR, telemetria e evidências de auditoria.
---

Esta página reflete o FAQ interno de liquidação (`docs/source/nexus_settlement_faq.md`) para que os leitores do portal revisem o misma guia sin hurgar no mono-repo. Explica como o Settlement Router processa pagamentos, que métricas monitoram e como o SDK deve integrar as cargas úteis de Norito.

## Destacados

1. **Mapa de pistas** - cada espaço de dados declara um `settlement_handle` (`xor_global`, `xor_lane_weighted`, `xor_hosted_custody` ou `xor_dual_fund`). Consulte o catálogo de pistas mais recentes em `docs/source/project_tracker/nexus_config_deltas/`.
2. **Conversão determinística** - o roteador converte todos os assentamentos em XOR através das fontes de liquidez aprovadas pelo governo. Las lanes privadas pré-financeiras buffers XOR; os cortes de cabelo se aplicam apenas quando os buffers são desviados da política.
3. **Telemetria** - vigilância `nexus_settlement_latency_seconds`, contadores de conversão e medidores de corte de cabelo. Os painéis vivem em `dashboards/grafana/nexus_settlement.json` e os alertas em `dashboards/alerts/nexus_audit_rules.yml`.
4. **Evidência** - arquivo de configurações, logs do roteador, exportações de telemetria e relatórios de conciliação para auditorias.
5. **Responsabilidades de SDK** - cada SDK deve expor ajudantes de liquidação, IDs de pista e codificadores de cargas úteis Norito para manter paridade com o roteador.

## Fluxos de exemplo

| Tipo de pista | Evidência de captura | O que você demonstra |
|-----------|--------------------|----------------|
| Privada `xor_hosted_custody` | Registro do roteador + `nexus_settlement_latency_seconds{lane}` + `settlement_router_haircut_total{lane}` | Os buffers CBDC debitam XOR determinístico e os cortes de cabelo são mantidos dentro da política. |
| Publica `xor_global` | Log do roteador + referência DEX/TWAP + métricas de latência/conversão | A rota de liquidez compartida manteve o preço da transferência para o TWAP publicado com zero corte de cabelo. |
| Híbrida `xor_dual_fund` | Log do roteador que mostra a divisão pública vs blindado + contadores de telemetria | A mezcla protegida/publica respeita as proporções de governança e registra o corte de cabelo aplicado a cada tramo. |

## Precisa de mais detalhes?

- FAQ completo: `docs/source/nexus_settlement_faq.md`
- Especificação do roteador de liquidação: `docs/source/settlement_router.md`
- Manual de política CBDC: `docs/source/cbdc_lane_playbook.md`
- Runbook de operações: [Operações de Nexus](./nexus-operations)
---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/settlement-faq.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-settlement-faq
título: Liquidação de perguntas frequentes
descrição: Respostas para os operadores que cobrem a liquidação de rotas, a conversão XOR, a telemetria e as precauções de auditoria.
---

Esta página representa o FAQ interno de liquidação (`docs/source/nexus_settlement_faq.md`) para que os leitores do portal possam consultar as indicações de memes sem preencher o mono-repo. Ele explique o comentário do Settlement Router sobre os pagamentos, as métricas do observador e o SDK deve integrar as cargas úteis Norito.

## Pontos cles

1. **Mapa de pistas** - cada espaço de dados declara um `settlement_handle` (`xor_global`, `xor_lane_weighted`, `xor_hosted_custody` ou `xor_dual_fund`). Consulte o último catálogo de pistas em `docs/source/project_tracker/nexus_config_deltas/`.
2. **Conversão determinada** - o roteador converte todas as liquidações em XOR por meio das fontes de liquidez aprovadas pelo governo. As pistas privadas pré-financiam os buffers XOR; Os cortes de cabelo não são aplicados quando os buffers são derivados fora da política.
3. **Telemetria** - monitore `nexus_settlement_latency_seconds`, os computadores de conversão e os modelos de corte de cabelo. Os painéis são encontrados em `dashboards/grafana/nexus_settlement.json` e os alertas em `dashboards/alerts/nexus_audit_rules.yml`.
4. **Preuves** - arquiva configurações, logs de roteador, exportações de telemetria e relatórios de reconciliação para auditorias.
5. **Responsabilidades SDK** - cada SDK expõe ajudantes de liquidação, IDs de pista e codificadores de cargas úteis Norito para restaurá-los alinhados com o roteador.

## Fluxo de exemplo

| Tipo de pista | Preuves um colecionador | O que isso prova |
|-----------|--------------------|----------------|
| Privado `xor_hosted_custody` | Log do roteador + `nexus_settlement_latency_seconds{lane}` + `settlement_router_haircut_total{lane}` | Os buffers CBDC debitam um XOR determinado e os cortes de cabelo permanecem na política. |
| Público `xor_global` | Log do roteador + referência DEX/TWAP + métricas de latência/conversão | O caminho de liquidez compartilha um preço fixo de transferência no TWAP publicado com zero corte de cabelo. |
| Híbrido `xor_dual_fund` | Log du roteador montando a repartição pública vs blindada + computadores de telemetria | A mistura blindada/pública respeita as proporções de governança e registra o corte de cabelo aplicado a cada jambe. |

## Quer mais detalhes?

- Perguntas frequentes completas: `docs/source/nexus_settlement_faq.md`
Especificações do roteador de liquidação: `docs/source/settlement_router.md`
- Manual de política CBDC: `docs/source/cbdc_lane_playbook.md`
- Operações de runbook: [Operações Nexus](./nexus-operations)
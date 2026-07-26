---
lang: pt
direction: ltr
source: docs/examples/soranet_incentive_parliament_packet/economic_analysis.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1b453559c05401edc11894e585c8d5ca4b678d4667c1cef0415582e1f7de8246
source_last_modified: "2025-11-05T17:23:10.790688+00:00"
translation_last_reviewed: 2026-01-01
---

# Analise economica - Shadow Run 2025-10 -> 2025-11

Artefato fonte: `docs/examples/soranet_incentive_shadow_run.json` (assinatura + chave publica no mesmo diretorio). A simulacao reprocessou 60 epocas por relay com o motor de recompensas fixado em `RewardConfig` registrado em `reward_config.json`.

## Resumo de distribuicao

- **Pagamentos totais:** 5,160 XOR em 360 epocas recompensadas.
- **Envelope de fairness:** coeficiente Gini 0.121; share do relay superior 23.26%
  (bem abaixo do guardrail de governance 30%).
- **Disponibilidade:** media da frota 96.97%, todos os relays permaneceram acima de 94%.
- **Largura de banda:** media da frota 91.20%, com o menor desempenho em 87.23%
  durante manutencao planejada; penalidades aplicadas automaticamente.
- **Ruido de compliance:** 9 epocas de warning e 3 suspensoes foram observadas e
  convertidas em reducoes de payout; nenhum relay excedeu o cap de 12 warnings.
- **Higiene operacional:** nenhum snapshot de metricas foi pulado por falta de
  config, bonds ou duplicatas; nenhum erro de calculadora foi emitido.

## Observacoes

- Suspensoes correspondem a epocas em que relays entraram em modo de manutencao. O
  motor de payout emitiu payouts zero para essas epocas enquanto preservava a
  trilha de auditoria no JSON do shadow-run.
- Penalidades de warning reduziram 2% dos payouts afetados; a distribuicao
  resultante ainda converge gracas aos pesos de uptime/bandwidth (650/350 per mille).
- A variancia de bandwidth acompanha o heatmap anonimo do guard. O menor desempenho
  (`6666...6666`) reteve 620 XOR na janela, acima do piso 0.6x.
- Alertas sensiveis a latencia (`SoranetRelayLatencySpike`) ficaram abaixo dos
  limites de warning durante toda a janela; dashboards correlatos estao em
  `dashboards/grafana/soranet_incentives.json`.

## Acoes recomendadas antes do GA

1. Continue rodando replays shadow mensais e atualize o set de artefatos e esta
   analise se a composicao da frota mudar.
2. Faca o gate de payouts automaticos na suite de alertas Grafana referenciada na roadmap
   (`dashboards/alerts/soranet_incentives_rules.yml`); copie screenshots para as
   atas de governance ao solicitar renovacao.
3. Reexecute o stress test economico se o reward base, os pesos de uptime/bandwidth ou
   a penalidade de compliance mudar em >=10%.

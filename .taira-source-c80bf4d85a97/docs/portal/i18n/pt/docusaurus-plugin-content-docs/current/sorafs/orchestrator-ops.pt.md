---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-ops.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: orquestrador-ops
título: Runbook de operações do orquestrador SoraFS
sidebar_label: Runbook do orquestrador
description: Guia operacional passo a passo para implantar, monitorar e reverter o orquestrador multi-origem.
---

:::nota Fonte canônica
Esta página espelha `docs/source/sorafs/runbooks/sorafs_orchestrator_ops.md`. Mantenha ambas as cópias sincronizadas.
:::

Este runbook orienta os SREs na preparação, no rollout e na operação do orquestrador de busca multi-origem. Ele complementa o guia de desenvolvimento com procedimentos ajustados para rollouts em produção, incluindo habilitação em fases e bloqueio de peers.

> **Veja também:** O [Runbook de rollout multi-origem](./multi-source-rollout.md) foca em ondas de rollout em toda a frota e na negação emergencial de provedores. Consulte-o para coordenação de governança / estadiamento enquanto usa este documento para as operações diárias do orquestrador.

## 1. Checklist pré-voo

1. **Coletar insumos de provedores**
   - Últimos anúncios de provedores (`ProviderAdvertV1`) e o snapshot de telemetria da frota alvo.
   - Plano de payload (`plan.json`) derivado do manifesto em teste.
2. **Gerar um placar determinístico**

   ```bash
   sorafs_fetch \
     --plan fixtures/plan.json \
     --telemetry-json fixtures/telemetry.json \
     --provider alpha=fixtures/provider-alpha.bin \
     --provider beta=fixtures/provider-beta.bin \
     --provider gamma=fixtures/provider-gamma.bin \
     --scoreboard-out artifacts/scoreboard.json \
     --json-out artifacts/session.summary.json
   ```

   - Valide a lista `artifacts/scoreboard.json` de cada provedor de produção como `eligible`.
   - Arquivar o JSON do resumo junto ao scoreboard; os auditores dependem de contadores de repetição de pedaços para certificar a solicitação de alteração.
3. **Dry-run com fixtures** — Execute o mesmo comando contra os fixtures públicos em `docs/examples/sorafs_ci_sample/` para garantir que o binário do orquestrador corresponda à versão esperada antes de tocar em cargas úteis de produção.

## 2. Procedimento de implementação em fases

1. **Fase canário (≤2 provedores)**
   - Recrie o scoreboard e execute com `--max-peers=2` para restringir o orquestrador a um subconjunto pequeno.
   - Monitore:
     -`sorafs_orchestrator_active_fetches`
     -`sorafs_orchestrator_fetch_failures_total{reason!="retry"}`
     -`sorafs_orchestrator_retries_total`
   - Prossiga quando as taxas de retry permanecerem abaixo de 1% para uma busca completa do manifesto e nenhum provedor acumular falhas.
2. **Fase de rampa (50% dos provedores)**
   - Aumente `--max-peers` e execute novamente com um snapshot de telemetria recente.
   - Persista cada execução com `--provider-metrics-out` e `--chunk-receipts-out`. Retenha os artistas por ≥7 dias.
3. **Lançamento completo**
   - Remoção `--max-peers` (ou definida para a contagem total de elegíveis).
   - Ativo o modo orquestrador nas implantações de clientes: distribuição do scoreboard persistido e do JSON de configuração via seu sistema de gerenciamento de configuração.
   - Atualizar os dashboards para exibir `sorafs_orchestrator_fetch_duration_ms` p95/p99 e histogramas de retry por região.

## 3. Bloqueio e reforço de pares

Use as substituições de política de pontuação do CLI para triagem de provedores não saudáveis sem esperar atualizações de governança.

```bash
sorafs_fetch \
  --plan fixtures/plan.json \
  --telemetry-json fixtures/telemetry.json \
  --provider alpha=fixtures/provider-alpha.bin \
  --provider beta=fixtures/provider-beta.bin \
  --provider gamma=fixtures/provider-gamma.bin \
  --deny-provider=beta \
  --boost-provider=gamma=5 \
  --json-out artifacts/override.summary.json
```- `--deny-provider` remova o alias indicado da consideração na sessão atual.
- `--boost-provider=<alias>=<weight>` aumenta o peso do provedor no agendador. Os valores são somados ao peso normalizado do placar e se aplicam apenas à execução local.
- Registrar as substituições no ticket de incidente e anexar as saídas JSON para que a equipe responsável possa reconciliar o estado quando o problema subjacente for resolvido.

Para mudanças permanentes, ajuste a telemetria de origem (marca o infrator como penalizado) ou atualize o anúncio com orçamentos de fluxo atualizados antes de limpar os overrides do CLI.

## 4. Triagem de falhas

Quando uma busca falha:

1. Capture os seguintes artefatos antes de executar novamente:
   -`scoreboard.json`
   -`session.summary.json`
   -`chunk_receipts.json`
   -`provider_metrics.json`
2. Inspeção `session.summary.json` para uma string de erro legível:
   - `no providers were supplied` → verifique os caminhos dos provedores e os anúncios.
   - `retry budget exhausted ...` → aumentar `--retry-budget` ou remover pares instáveis.
   - `no compatible providers available ...` → auditar os metadados de capacidade de faixa do provedor infrator.
3. Correlacione o nome do provedor com `sorafs_orchestrator_provider_failures_total` e abra um ticket de acompanhamento se a métrica disparar.
4. Reproduza o fetch offline com `--scoreboard-json` e uma telemetria capturada para reproduzir uma falha de forma determinística.

## 5. Reversão

Para reverter um rollout do orquestrador:

1. Distribua uma configuração que defina `--max-peers=1` (desabilite efetivamente o agendamento multi-origem) ou retorne os clientes ao caminho de busca de fonte única.
2. Remova quaisquer substituições `--boost-provider` para que o placar volte a uma ponderação neutra.
3. Continue coletando as análises do orquestrador por pelo menos um dia para confirmar que não há buscas residuais em andamento.

Manter uma captura disciplinada de artistas e lançamentos em fases garante que o orquestrador multi-origem possa ser operado com segurança em frotas heterogêneas de provedores, mantendo os requisitos de observabilidade e auditoria intactos.
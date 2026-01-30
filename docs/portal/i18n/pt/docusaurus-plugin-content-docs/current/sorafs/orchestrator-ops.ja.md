---
lang: ja
direction: ltr
source: docs/portal/i18n/pt/docusaurus-plugin-content-docs/current/sorafs/orchestrator-ops.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dd56aaf442c33890ef9f0509274e42d76d027dd71c95c9a05bce433ecdea76be
source_last_modified: "2025-11-14T04:43:21.991795+00:00"
translation_last_reviewed: 2026-01-30
---

:::note Fonte canônica
Esta página espelha `docs/source/sorafs/runbooks/sorafs_orchestrator_ops.md`. Mantenha ambas as copias sincronizadas.
:::

Este runbook orienta os SREs na preparação, no rollout e na operação do orquestrador de fetch multi-origem. Ele complementa o guia de desenvolvimento com procedimentos ajustados para rollouts em produção, incluindo habilitação em fases e bloqueio de peers.

> **Veja também:** O [Runbook de rollout multi-origem](./multi-source-rollout.md) foca em ondas de rollout em toda a frota e na negação emergencial de provedores. Consulte-o para coordenação de governança / staging enquanto usa este documento para as operações diárias do orquestrador.

## 1. Checklist pré-voo

1. **Coletar insumos de provedores**
   - Últimos anúncios de provedores (`ProviderAdvertV1`) e o snapshot de telemetria da frota alvo.
   - Plano de payload (`plan.json`) derivado do manifesto em teste.
2. **Gerar um scoreboard determinístico**

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

   - Valide se `artifacts/scoreboard.json` lista cada provedor de produção como `eligible`.
   - Arquive o JSON de resumo junto ao scoreboard; auditores dependem dos contadores de retry de chunks ao certificar a solicitação de mudança.
3. **Dry-run com fixtures** — Execute o mesmo comando contra os fixtures públicos em `docs/examples/sorafs_ci_sample/` para garantir que o binário do orquestrador corresponde à versão esperada antes de tocar em payloads de produção.

## 2. Procedimento de rollout em fases

1. **Fase canário (≤2 provedores)**
   - Recrie o scoreboard e execute com `--max-peers=2` para restringir o orquestrador a um subconjunto pequeno.
   - Monitore:
     - `sorafs_orchestrator_active_fetches`
     - `sorafs_orchestrator_fetch_failures_total{reason!="retry"}`
     - `sorafs_orchestrator_retries_total`
   - Prossiga quando as taxas de retry permanecerem abaixo de 1% para um fetch completo do manifesto e nenhum provedor acumular falhas.
2. **Fase de rampa (50% dos provedores)**
   - Aumente `--max-peers` e execute novamente com um snapshot de telemetria recente.
   - Persista cada execução com `--provider-metrics-out` e `--chunk-receipts-out`. Retenha os artefatos por ≥7 dias.
3. **Rollout completo**
   - Remova `--max-peers` (ou defina para a contagem total de elegíveis).
   - Ative o modo orquestrador nos deployments de clientes: distribua o scoreboard persistido e o JSON de configuração via seu sistema de gerenciamento de configuração.
   - Atualize os dashboards para exibir `sorafs_orchestrator_fetch_duration_ms` p95/p99 e histogramas de retry por região.

## 3. Bloqueio e reforço de peers

Use os overrides de política de pontuação do CLI para triagem de provedores não saudáveis sem esperar atualizações de governança.

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
```

- `--deny-provider` remove o alias indicado da consideração na sessão atual.
- `--boost-provider=<alias>=<weight>` aumenta o peso do provedor no agendador. Os valores são somados ao peso normalizado do scoreboard e se aplicam apenas à execução local.
- Registre os overrides no ticket de incidente e anexe as saídas JSON para que a equipe responsável possa reconciliar o estado quando o problema subjacente for resolvido.

Para mudanças permanentes, ajuste a telemetria de origem (marque o infrator como penalizado) ou atualize o anúncio com orçamentos de fluxo atualizados antes de limpar os overrides do CLI.

## 4. Triagem de falhas

Quando um fetch falha:

1. Capture os seguintes artefatos antes de executar novamente:
   - `scoreboard.json`
   - `session.summary.json`
   - `chunk_receipts.json`
   - `provider_metrics.json`
2. Inspecione `session.summary.json` para a string de erro legível:
   - `no providers were supplied` → verifique os caminhos dos provedores e os anúncios.
   - `retry budget exhausted ...` → aumente `--retry-budget` ou remova peers instáveis.
   - `no compatible providers available ...` → audite os metadados de capacidade de faixa do provedor infrator.
3. Correlacione o nome do provedor com `sorafs_orchestrator_provider_failures_total` e abra um ticket de acompanhamento se a métrica disparar.
4. Reproduza o fetch offline com `--scoreboard-json` e a telemetria capturada para reproduzir a falha de forma determinística.

## 5. Rollback

Para reverter um rollout do orquestrador:

1. Distribua uma configuração que defina `--max-peers=1` (desabilita efetivamente o agendamento multi-origem) ou retorne os clientes ao caminho de fetch de fonte única.
2. Remova quaisquer overrides `--boost-provider` para que o scoreboard volte a uma ponderação neutra.
3. Continue coletando as métricas do orquestrador por pelo menos um dia para confirmar que não há fetches residuais em andamento.

Manter a captura disciplinada de artefatos e rollouts em fases garante que o orquestrador multi-origem possa ser operado com segurança em frotas heterogêneas de provedores, mantendo os requisitos de observabilidade e auditoria intactos.

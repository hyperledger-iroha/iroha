---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-ops.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: orquestrador-ops
título: Runbook de operações do orquestrador de SoraFS
sidebar_label: Runbook do orquestrador
descrição: Guia operativo passo a passo para desplegar, supervisionar e reverter o orquestrador multi-origem.
---

:::nota Fonte canônica
Esta página reflete `docs/source/sorafs/runbooks/sorafs_orchestrator_ops.md`. Mantenha ambas as cópias sincronizadas até que o conjunto de documentação Sphinx herdado seja migrado por completo.
:::

Este runbook guia o SRE na preparação, na implementação e na operação do orquestrador de busca de múltiplas origens. Complementa o guia de desenvolvimento com procedimentos ajustados a despliegues de produção, incluindo a habilitação por etapas e o bloqueio de pares.

> **Ver também:** O [Runbook de despliegue multi-origen](./multi-source-rollout.md) é centrado em oleadas de despliegue a nível de flota e na negação de provedores em emergências. Consulta para a coordenação de governo / encenação enquanto usa este documento para as operações diárias do orquestrador.

## 1. Lista de verificação anterior

1. **Recopilar entradas de provedores**
   - Os últimos anúncios de provedores (`ProviderAdvertV1`) e a instantânea de telemetria da frota alvo.
   - O plano de carga útil (`plan.json`) derivado da manifestação abaixo do teste.
2. **Gerar um placar determinista**

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

   - Valide que `artifacts/scoreboard.json` liste cada fornecedor de produção como `eligible`.
   - Arquivar o JSON do currículo junto com o placar; os auditores se apoiam nos contadores de reintenções de pedaços para certificar a solicitação de mudança.
3. **Dry-run con fixtures** — Execute o mesmo comando contra os fixtures públicos em `docs/examples/sorafs_ci_sample/` para garantir que o binário do orquestrador coincida com a versão esperada antes de executar cargas úteis de produção.

## 2. Procedimento de execução por etapas

1. **Etapa canaria (≤2 provadores)**
   - Reconstrua o placar e execute-o com `--max-peers=2` para limitar o orquestrador a um subconjunto pequeno.
   - Supervisão:
     -`sorafs_orchestrator_active_fetches`
     -`sorafs_orchestrator_fetch_failures_total{reason!="retry"}`
     -`sorafs_orchestrator_retries_total`
   - Continue enquanto as tarefas de reintenção são mantidas por baixo de 1% para uma busca completa do manifesto e nenhum provedor acumula falhas.
2. **Etapa de rampa (50% de fornecedores)**
   - Aumente `--max-peers` e execute um instante de telemetria recente.
   - Persista cada execução com `--provider-metrics-out` e `--chunk-receipts-out`. Conservar os artefatos durante ≥7 dias.
3. **Despliegue completo**
   - Elimine `--max-peers` (ou configure o registro completo de elegíveis).
   - Habilita o modo orquestrador nos aplicativos do cliente: distribui o placar persistido e o JSON de configuração por meio de seu sistema de gerenciamento de configuração.
   - Atualiza os painéis para mostrar `sorafs_orchestrator_fetch_duration_ms` p95/p99 e os histogramas de reintenções por região.

## 3. Bloqueio e refúgio de paresUse as anulações da política de pontuação da CLI para classificar provedores não saludáveis ​​sem esperar atualizações de governança.

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

- `--deny-provider` elimina o alias indicado da consideração na sessão real.
- `--boost-provider=<alias>=<weight>` aumenta o peso do planejador do fornecedor. Os valores são somados ao peso normalizado do placar e apenas aplicados à execução local.
- Registre as anulações no ticket de incidente e adicione as saídas JSON para que a equipe responsável possa reconciliar o estado uma vez que o problema subjacente seja corrigido.

Para mudanças permanentes, modifique a telemetria de origem (marca de infrator como penalizado) ou atualize o anúncio com pressupostos de fluxo atualizados antes de limpar as anulações do CLI.

## 4. Diagnóstico de falhas

Quando uma busca falha:

1. Capture os artefatos seguintes antes de retornar à execução:
   -`scoreboard.json`
   -`session.summary.json`
   -`chunk_receipts.json`
   -`provider_metrics.json`
2. Inspecione `session.summary.json` para ver a cadeia de erro legível:
   - `no providers were supplied` → verifica as rotas dos provedores e dos anúncios.
   - `retry budget exhausted ...` → incrementa `--retry-budget` para eliminar peers inestáveis.
   - `no compatible providers available ...` → audita a metainformação de capacidade de rango do provedor infrator.
3. Correlacione o nome do provedor com `sorafs_orchestrator_provider_failures_total` e crie um ticket de acompanhamento se a métrica for diferente.
4. Reproduza a obtenção sem conexão com `--scoreboard-json` e a telemetria capturada para reproduzir a falha de forma determinista.

## 5. Reversão

Para reverter uma resposta do orquestrador:

1. Distribua uma configuração que estabeleça `--max-peers=1` (desabilite o planejamento de múltiplas origens) ou entregue aos clientes a rota de busca herdada de uma única origem.
2. Elimine qualquer anulação `--boost-provider` para que o placar volte para um peso neutro.
3. Continue coletando as informações do orquestrador por pelo menos um dia para confirmar que nenhuma busca é feita no vôo.

Manter uma captura disciplinada de artefatos e seguir etapas garante que o orquestrador de múltiplas origens possa operar de forma segura em frotas heterogêneas de provadores enquanto mantém os requisitos de observabilidade e auditoria.
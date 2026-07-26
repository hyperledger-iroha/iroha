---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-ops.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: orquestrador-ops
título: Runbook d’exploitation do orquestrador SoraFS
sidebar_label: orquestrador de runbook
descrição: Guia operacional para implantar, monitorar e retornar ao arrière no orquestrador multi-fonte.
---

:::nota Fonte canônica
Esta página reflete `docs/source/sorafs/runbooks/sorafs_orchestrator_ops.md`. Gardez as duas cópias sincronizadas junto com o conjunto de documentação Sphinx herdado totalmente migrado.
:::

Este runbook guia o SRE na preparação, na implantação e na exploração do orquestrador de busca de múltiplas fontes. O guia do desenvolvedor é completo com procedimentos adaptados às implantações em produção, e inclui a ativação por etapas e a colocação na lista negra de pares.

> **Veja também:** O [Runbook de implantação multi-fonte](./multi-source-rollout.md) concentra-se nas vagas de implantação na estação do parque e nos recusas de urgência dos fornecedores. Référez-vous-y para a coordenação de governança / encenação usando este documento para as operações diárias do orquestrador.

## 1. Checklist pré-implantação

1. **Colecione as entradas fornecidas**
   - Últimos avisos de fornecimento (`ProviderAdvertV1`) e telemetria instantânea para o fio.
   - Plano de carga útil (`plan.json`) derivado do manifesto testado.
2. **Gerar um placar determinado**

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

   - Verifique se `artifacts/scoreboard.json` repertório cada fornecedor de produção como `eligible`.
   - Arquivar o JSON de síntese com o placar; os auditores solicitam que os auditores tentem novamente os pedaços para a certificação da demanda de troca.
3. **Teste com os equipamentos** — Execute o mesmo comando nos equipamentos públicos de `docs/examples/sorafs_ci_sample/` para garantir que o binário do orquestrador corresponda à versão exibida antes de tocar nas cargas úteis de produção.

## 2. Procedimento de implantação por etapas

1. **Étape canari (≤2 fournisseurs)**
   - Reconstrua o placar e execute-o com `--max-peers=2` para limitar o orquestrador a um pequeno sous-ensemble.
   - Vigilância:
     -`sorafs_orchestrator_active_fetches`
     -`sorafs_orchestrator_fetch_failures_total{reason!="retry"}`
     -`sorafs_orchestrator_retries_total`
   - Despeje uma vez que a taxa de repetição permaneça abaixo de 1% para obter uma busca completa do manifesto e que forneça nenhum acúmulo de cheques.
2. **Étape de montée en charge (50% dos fornecedores)**
   - Aumente `--max-peers` e reinicie com um instante de transmissão recente.
   - Mantenha cada execução com `--provider-metrics-out` e `--chunk-receipts-out`. Conserve os artefatos por ≥7 dias.
3. **Implantação concluída**
   - Suprima `--max-peers` (ou fixe o nome total de fornecedores elegíveis).
   - Ative o modo orquestrador nas implantações de clientes: distribua o placar persistente e o JSON de configuração por meio de seu sistema de gerenciamento de configuração.
   - Crie hoje os quadros de borda para exibir `sorafs_orchestrator_fetch_duration_ms` p95/p99 e os histogramas de repetição por região.

## 3. Mise en liste noire et boosting des pairsUtilize as substituições de política de pontuação do CLI para testar os fornecedores fracassados ​​sem atender às mises no dia do governo.

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

- `--deny-provider` retire o alias indicado da sessão durante o curso.
- `--boost-provider=<alias>=<weight>` aumenta o peso do fornecedor no planejador. Os valores são adicionados ao peso normalizado do placar e não são aplicados no local de execução.
- Registre as substituições no ticket de incidente e execute as saídas JSON para que a equipe responsável possa reconciliar o estado quando o problema for corrigido.

Para alterações permanentes, modifique a fonte de transmissão (marque o fautif como penalisé) ou anuncie no dia seguinte com os orçamentos de fluxo revisados ​​antes de suprimir as substituições da CLI.

## 4. Diagnóstico dos painéis

Lorsqu’un fetch ecoou:

1. Capture os seguintes artefatos antes de relançar:
   -`scoreboard.json`
   -`session.summary.json`
   -`chunk_receipts.json`
   -`provider_metrics.json`
2. Inspecione `session.summary.json` para verificar a cadeia de erros:
   - `no providers were supplied` → verifique os caminhos dos fornecedores e os anúncios.
   - `retry budget exhausted ...` → aumente `--retry-budget` ou suprima pares instáveis.
   - `no compatible providers available ...` → verifique os limites de capacidade da praia do fornecedor fautif.
3. Corrija o nome do fornecedor com `sorafs_orchestrator_provider_failures_total` e crie um ticket de suivi se a métrica monte em flor.
4. Execute a busca fora da linha com `--scoreboard-json` e a captura de tela para reproduzir a verificação de forma determinada.

## 5. Reversão

Para reviver uma implantação do orquestrador:

1. Distribua uma configuração definida como `--max-peers=1` (desative efetivamente a ordenação de múltiplas fontes) ou volte para o caminho de busca do histórico de fontes únicas dos clientes.
2. Suprima toute override `--boost-provider` para que o placar volte a um peso neutro.
3. Continue examinando as métricas do orquestrador durante pelo menos um dia para confirmar se algum resíduo de busca não está no vol.

Manter uma captura disciplinada de artefatos e implantações por etapas garante que o orquestrador multi-fonte possa ser operado com toda a segurança nas frotas heterogêneas de fornecedores, respeitando as exigências de observabilidade e auditoria.
---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/multi-source-rollout.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: implementação de várias fontes
título: Runbook de rollout multi-origem e negação de provedores
sidebar_label: Runbook de implementação de origem múltipla
descrição: Checklist operacional para rollouts multi-origem em fases e negação emergencial de provedores.
---

:::nota Fonte canônica
Esta página espelha `docs/source/sorafs/runbooks/multi_source_rollout.md`. Mantenha ambas as cópias sincronizadas.
:::

##Objetivo

Este runbook orienta os SREs e engenheiros de plantio em dois fluxos críticos:

1. Fazer o rollout do orquestrador multi-origem em ondas controladas.
2. Negar ou despriorizar provedores com mau comportamento sem desestabilizar sessões existentes.

Pressupõe que a pilha de orquestração entregue sob SF-6 já esteja implantada (`sorafs_orchestrator`, API de intervalo de chunks do gateway, exportadores de telemetria).

> **Veja também:** O [Runbook de operações do orquestrador](./orchestrator-ops.md) aprofunda os procedimentos por execução (captura de scoreboard, toggles de rollout em fases, rollback). Use ambas as referências em conjunto durante as mudanças ao vivo.

## 1. Validação pré-voo

1. **Confirmar insumos de governança.**
   - Todos os provedores candidatos deverão publicar envelopes `ProviderAdvertV1` com cargas úteis de capacidade de intervalo e orçamentos de fluxo. Valide via `/v2/sorafs/providers` e compare com os campos de capacidade esperada.
   - Instantâneos de telemetria que fornecem taxas de latência/falha devem ter pelo menos 15 minutos antes de cada execução canária.
2. **Prepare uma configuração.**
   - Persista a configuração JSON do orquestrador na árvore em camadas `iroha_config`:

     ```toml
     [torii.sorafs.orchestrator]
     config_path = "/etc/iroha/sorafs/orchestrator.json"
     ```

     Atualizar o JSON com limites específicos de rollout (`max_providers`, orçamentos de nova tentativa). Use o mesmo arquivo em staging/produção para manter as diferenças mínimas.
3. **Exercitar fixtures canônicas.**
   - Preencha as variáveis de ambiente de manifesto/token e execute o fetch determinístico:

     ```bash
     sorafs_cli fetch \
       --plan fixtures/sorafs_manifest/ci_sample/payload.plan.json \
       --manifest-id "$CANARY_MANIFEST_ID" \
       --provider name=alpha,provider-id="$PROVIDER_ALPHA_ID",base-url=https://gw-alpha.example,stream-token="$PROVIDER_ALPHA_TOKEN" \
       --provider name=beta,provider-id="$PROVIDER_BETA_ID",base-url=https://gw-beta.example,stream-token="$PROVIDER_BETA_TOKEN" \
       --provider name=gamma,provider-id="$PROVIDER_GAMMA_ID",base-url=https://gw-gamma.example,stream-token="$PROVIDER_GAMMA_TOKEN" \
       --max-peers=3 \
       --retry-budget=4 \
       --scoreboard-out artifacts/canary.scoreboard.json \
       --json-out artifacts/canary.fetch.json
     ```

     As variáveis de ambiente devem conter o resumo do payload do manifesto (hex) e tokens de stream codificados em base64 para cada provedor participante do canary.
   - Compare `artifacts/canary.scoreboard.json` com o lançamento anterior. Qualquer novo provedor inelegível ou alteração de peso >10% exige revisão.
4. **Verifique se a telemetria está conectada.**
   - Abra a exportação do Grafana em `docs/examples/sorafs_fetch_dashboard.json`. Garanta que as métricas `sorafs_orchestrator_*` sejam aplicadas em estadiamento antes de prosseguir.

## 2. Negação emergencial de provedores

Siga este procedimento quando um provedor servir pedaços danificados, estourar timeouts de forma persistente ou falha em verificações de conformidade.1. **Capturar evidências.**
   - Exporte o resumo de busca mais recente (saída de `--json-out`). Registre índices de pedaços com falha, aliases de provedores e variações de resumo.
   - Salve trechos de log relevantes dos alvos `telemetry::sorafs.fetch.*`.
2. **Aplicar override imediatamente.**
   - Marque o provedor como penalizado no snapshot de telemetria distribuído ao orquestrador (definido `penalty=true` ou limite `token_health` a `0`). O próximo build do placar excluirá o provedor automaticamente.
   - Para testes de fumaça ad-hoc, passe `--deny-provider gw-alpha` para `sorafs_cli fetch` para treinar o caminho de falha sem esperar a propagação da telemetria.
   - Reimplante o pacote atualizado de telemetria/configuração no ambiente afetado (encenação → canário → produção). Documente a mudança no registro do incidente.
3. **Validar ou substituir.**
   - Execute novamente o fetch do fixture canônico. Confirme que o placar marca o provedor como inelegível com o motivo `policy_denied`.
   - Inspecione `sorafs_orchestrator_provider_failures_total` para garantir que o contador pare de aumentar para o provedor negado.
4. **Escalar banimentos prolongados.**
   - Se o provedor permanecer bloqueado por >24 h, abra um ticket de governança para rotacionar ou suspender seu anúncio. Até a votação passar, mantenha a lista de negação e atualize os snapshots de telemetria para que o provedor não volte ao placar.
5. **Protocolo de reversão.**
   - Para reintegrar o provedor, remover a lista de negação, reimplantar e capturar um novo snapshot do placar. Anexo a mudança ao postmortem do incidente.

## 3. Plano de implementação em fases

| Fase | Escopo | Sinais obrigatórios | Critérios Go/No-Go |
|------|--------|---------------------|---------|
| **Laboratório** | Cluster de integração dedicado | Buscar manual por CLI contra cargas úteis de fixtures | Todos os chunks concluem, contadores de falha de provedor ficam em 0, taxa de tentativas < 5%. |
| **Encenação** | Encenação de plano de controle completo | Painel do Grafana conectado; regras de alerta em modo somente aviso | `sorafs_orchestrator_active_fetches` volta a zero após cada execução de teste; nenhum alerta `warn/critical`. |
| **Canário** | ≤10% do tráfego de produção | Pager silenciado mas telemetria monitorada em tempo real | Razão de tentativas < 10%, falhas de provedores isoladas a pares ruidosos conhecidos, histograma de latência coincide com uma linha de base de estadiamento ±20%. |
| **Disponibilidade geral** | 100% lançam | Regras do pager ativo | Zero erros `NoHealthyProviders` por 24 h, razão de tentativas estáveis, painéis SLA do dashboard em verde. |

Para cada fase:

1. Atualizar o JSON do orquestrador com os `max_providers` e orçamentos de nova tentativa previstos.
2. Execute `sorafs_cli fetch` ou um conjunto de testes de integração do SDK contra o fixture canônico e um manifesto representativo do ambiente.
3. Capture os artistas do placar + resumo e anexos ao registro de lançamento.
4. Revise os dashboards de telemetria com o engenheiro de plantação antes de promovê-los para a próxima fase.## 4. Observabilidade e ganchos de incidente

- **Métricas:** Garanta que o Alertmanager monitore `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` e `sorafs_orchestrator_retries_total`. Um pico arrependido geralmente significa que um provedor está degradando sob carga.
- **Logs:** Rote os alvos `telemetry::sorafs.fetch.*` para o agregador de logs compartilhados. Crie buscas salvas para `event=complete status=failed` para acelerar a triagem.
- **Scoreboards:** Persista cada um dos artefatos de scoreboard em armazenamento de longo prazo. O JSON também serve como trilha de evidência para revisões de conformidade e reversões por fase.
- **Dashboards:** Clone o dashboard Grafana canônico (`docs/examples/sorafs_fetch_dashboard.json`) na pasta de produção com as regras de alerta de `docs/examples/sorafs_fetch_alerts.yaml`.

## 5. Comunicação e documentação

- Registrar cada alteração de deny/boost no changelog de operações com carimbo de data/hora, operador, motivo e incidente associado.
- Notifique as equipes de SDK quando os pesos dos fornecedores ou os orçamentos de nova tentativa mudarem para alinhar as expectativas do lado do cliente.
- Depois que o GA terminar, atualize `status.md` com o resumo do rollout e arquive esta referência de runbook nas notas de lançamento.
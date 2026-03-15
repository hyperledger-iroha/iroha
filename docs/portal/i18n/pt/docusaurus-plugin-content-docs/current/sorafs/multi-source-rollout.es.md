---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/multi-source-rollout.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: implementação de várias fontes
título: Runbook de despliegue multi-origen e negação de fornecedores
sidebar_label: Runbook de origem multi-origem
description: Lista de verificação operativa para aplicações multi-origem por etapas e negação de provedores em emergências.
---

:::nota Fonte canônica
Esta página reflete `docs/source/sorafs/runbooks/multi_source_rollout.md`. Mantenha ambas as cópias alinhadas até que o conjunto de documentação herdado seja retirado.
:::

## Propósito

Este runbook guia o SRE e os engenheiros de proteção através dos fluxos críticos:

1. Desative o orquestrador multi-origem em oleadas controladas.
2. Denegar ou despriorizar provedores que se comportem mal sem desestabilizar as sessões existentes.

Suponha que a pilha de solicitação entregue abaixo do SF-6 esteja desplegada (`sorafs_orchestrator`, API de rango de pedaços do gateway, exportadores de telemetria).

> **Ver também:** O [Runbook de operações do orquestrador](./orchestrator-ops.md) aprofunda os procedimentos de execução (captura de placar, alternância de despliegue por etapas, rollback). Usa ambas as referências em conjunto durante mudanças em vivo.

## 1. Validação prévia

1. **Confirmar entradas de governo.**
   - Todos os provedores candidatos devem publicar sobres `ProviderAdvertV1` com cargas úteis de capacidade de rango e pressupostos de stream. Valído por meio de `/v1/sorafs/providers` e comparado com os campos de capacidade esperados.
   - Os instantes de telemetria que transmitem tarefas de latência/queda devem ter menos de 15 minutos antes de cada execução no Canadá.
2. **Prepare a configuração.**
   - Mantenha a configuração JSON do orquestrador na árvore `iroha_config` pelas seguintes capas:

     ```toml
     [torii.sorafs.orchestrator]
     config_path = "/etc/iroha/sorafs/orchestrator.json"
     ```

     Atualiza o JSON com limites específicos de implementação (`max_providers`, requisitos de reintenção). Usa o mesmo arquivo de encenação/produção para que as diferenças sejam mínimas.
3. **Emitir os fixtures canônicos.**
   - Recarrega as variáveis de ambiente do manifesto/token e executa o determinista de busca:

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

     Las variáveis ​​de ambiente devem conter o resumo da carga útil do manifesto (hex) e os tokens de fluxo codificados em base64 para cada provedor que participa do canário.
   - Compara `artifacts/canary.scoreboard.json` com a liberação anterior. Qualquer provedor novo não elegível ou uma mudança de peso >10% requer revisão.
4. **Verifique se a telemetria está conectada.**
   - Abra a exportação de Grafana para `docs/examples/sorafs_fetch_dashboard.json`. Certifique-se de que as informações `sorafs_orchestrator_*` sejam possíveis na preparação antes de continuar.

## 2. Negação de provedores em emergências

Siga este procedimento quando um provedor entregou pedaços corrompidos, antes do tempo de espera de forma persistente ou falha nas verificações de cumprimento.1. **Capturar evidências.**
   - Exporta o currículo de busca mais recente (saída de `--json-out`). Registra índices de pedaços caídos, alias de provedores e ajustes de resumo.
   - Guarda extratos de logs relevantes dos alvos `telemetry::sorafs.fetch.*`.
2. **Aplicar uma substituição imediatamente.**
   - Marcar ao provedor como penalizado na instantânea de telemetria distribuída ao orquestrador (estabelecido `penalty=true` ou limite `token_health` a `0`). A construção seguinte do placar será excluída automaticamente do provedor.
   - Para testes de humor ad-hoc, passe `--deny-provider gw-alpha` a `sorafs_cli fetch` para iniciar a rota de falha sem esperar a propagação da telemetria.
   - Volte a descarregar o pacote atualizado de telemetria/configuração no ambiente afetado (preparação → canário → produção). Documente a mudança no registro do incidente.
3. **Validar a substituição.**
   - Repete a busca do dispositivo canônico. Confirme que o placar marca o provedor como não elegível com o motivo `policy_denied`.
   - Inspecione `sorafs_orchestrator_provider_failures_total` para garantir que o contador deixe de aumentar para o fornecedor negado.
4. **Escalar bloqueios prolongados.**
   - Se o provedor for bloqueado por mais de 24 horas, abra um ticket de governo para girar ou suspender seu anúncio. Até passar a votação, mantenha a lista de negação e atualize os instantes de telemetria para que o provedor não volte ao placar.
5. **Protocolo de reversão.**
   - Para restabelecer o provedor, elimine a lista de negação, desista e capture uma atualização instantânea do placar. Adjunta a mudança após a morte do incidente.

## 3. Plano de execução por etapas

| Fase | Alcance | Señales requeridas | Critério de Avançar/Não Avançar |
|------|--------|--------------------|----------------------|
| **Laboratório** | Cluster de integração dedicado | Buscar manual por CLI contra cargas úteis de fixtures | Todos os pedaços são completados, os contadores de falhas do provedor permanecem em 0, taxa de reintenções < 5%. |
| **Encenação** | Preparação do plano de controle completo | Painel de Grafana conectado; regras de alertas no modo solo warning | `sorafs_orchestrator_active_fetches` volta a zero após cada execução de teste; nenhum alerta disparado `warn/critical`. |
| **Canário** | ≤10% do tráfico de produção | Pager silenciado, mas telemetria monitorizada em tempo real | Proporção de reintenções < 10%, falhas de fornecedores isolados a pares ruidosos conhecidos, histograma de latência coincide com a linha base de estadiamento ±20%. |
| **Disponibilidade geral** | 100% do lançamento | Regras do pager ativado | Zero erros `NoHealthyProviders` durante 24 horas, proporção de reintenções estável, painéis SLA do painel em verde. |

Para cada fase:1. Atualize o JSON do orquestrador com o `max_providers` e os requisitos de reintenções previstos.
2. Execute `sorafs_cli fetch` ou o conjunto de testes de integração do SDK contra o dispositivo canônico e um manifesto representativo do ambiente.
3. Capture os artefatos do placar e do resumo e adicione-os ao registro de lançamento.
4. Revise os painéis de telemetria com o engenheiro de guarda antes de promovê-los na fase seguinte.

## 4. Observabilidade e ganchos de incidentes

- **Métricas:** Certifique-se do monitor do Alertmanager `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` e `sorafs_orchestrator_retries_total`. Um pico de arrependimento significa que um fornecedor se degrada abaixo da carga.
- **Logs:** Enrole os alvos `telemetry::sorafs.fetch.*` no agregador de logs compartilhado. Crie pesquisas protegidas para `event=complete status=failed` para acelerar a triagem.
- **Placares:** Persista cada artefato de placar armazenado em um largo espaço. O JSON também serve como rastro de evidência para revisões de cumprimento e reversões por etapas.
- **Dashboards:** Clona a tabela Grafana canônica (`docs/examples/sorafs_fetch_dashboard.json`) na pasta de produção com as regras de alerta de `docs/examples/sorafs_fetch_alerts.yaml`.

## 5. Comunicação e documentação

- Registrar cada mudança de negação/reforço no changelog de operações com marca de tempo, operadora, motivo e incidente associado.
- Notificar os equipamentos do SDK quando os pesos dos provedores ou os pressupostos de reintenções mudam para alinhar as expectativas do lado do cliente.
- Depois de completar o GA, atualize `status.md` com o resumo da implementação e arquive esta referência do runbook nas notas da versão.
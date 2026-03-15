---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/multi-source-rollout.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: implementação de várias fontes
título: Runbook de implementação multi-source e mise en liste noire
sidebar_label: implementação de runbook multi-fonte
descrição: Lista de verificação operacional para implantações multi-fonte por etapas e lista negra de emergência de fornecedores.
---

:::nota Fonte canônica
Esta página reflete `docs/source/sorafs/runbooks/multi_source_rollout.md`. Garanta que as duas cópias sejam sincronizadas até que a documentação herdada seja retirada.
:::

## Objetivo

Este runbook guia o SRE e os engenheiros de gerenciamento através de duas críticas de fluxos de trabalho:

1. Implante o orquestrador multifonte por vagas controladas.
2. Insira uma lista negra ou despreze os fornecedores deficientes sem desestabilizar as sessões existentes.

Suponho que a pilha de orquestração liberada sob SF-6 esteja implantada (`sorafs_orchestrator`, API de plage de chunks du gateway, exportadores de télémétrie).

> **Veja também:** O [Runbook de exploração do orquestrador](./orchestrator-ops.md) aborda os procedimentos de execução (captura de placar, basculas de implantação por etapas, reversão). Utilize o conjunto de duas referências para alterações na produção.

## 1. Validação pré-implantação

1. **Confirme as entradas de governo.**
   - Todos os fornecedores candidatos devem publicar os envelopes `ProviderAdvertV1` com cargas úteis de capacidade de plage e orçamentos de fluxo. Validade via `/v1/sorafs/providers` e compare os níveis de capacidade atendidos.
   - Os instantâneos de télémétrie fornecem a taxa de latência/échec deve ser datada de menos de 15 minutos antes de cada execução canary.
2. **Prepare a configuração.**
   - Mantenha a configuração JSON do orquestrador na árvore `iroha_config` em sofás:

     ```toml
     [torii.sorafs.orchestrator]
     config_path = "/etc/iroha/sorafs/orchestrator.json"
     ```

     Atualize o JSON com os limites específicos de implementação (`max_providers`, orçamentos de nova tentativa). Utilize o mesmo arquivo na encenação/produção para preservar as diferenças mínimas.
3. **Exerça os fixtures canônicos.**
   - Registra as variáveis do ambiente manifest/token e lança a busca determinada:

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

     As variáveis ​​de ambiente devem conter o resumo da carga útil do manifesto (hex) e os tokens de fluxo codificados em base64 para cada participante fornecido no canário.
   - Compare `artifacts/canary.scoreboard.json` com a versão anterior. Todo novo fornecedor não é elegível ou uma redução de peso >10% requer uma revisão.
4. **Verifique se a televisão está cabeada.**
   - Abra a exportação Grafana em `docs/examples/sorafs_fetch_dashboard.json`. Certifique-se de que as métricas `sorafs_orchestrator_*` sejam exibidas em estágio antes de serem executadas.

## 2. Mise en liste noire d'urgence des fournisseurs

Siga este procedimento quando um fornecedor apresentar pedaços corrompidos, desde atrasos de atenção persistentes ou ecoados aux controles de conformidade.1. **Capture as prévias.**
   - Exporte o último currículo de busca (sorteio de `--json-out`). Registre o índice de pedaços em cheque, os alias dos fornecedores e as discordâncias do resumo.
   - Guarde os extratos de registros pertinentes aos livros `telemetry::sorafs.fetch.*`.
2. **Aplicar uma substituição imediatamente.**
   - Marque o fornecedor como penalizado no snapshot da televisão distribuído ao orquestrador (definido `penalty=true` ou forçado `token_health` a `0`). O placar prochain exclui automaticamente o fornecedor.
   - Para testes de fumaça ad hoc, passe `--deny-provider gw-alpha` para `sorafs_cli fetch` para executar o caminho de falha sem atender à propagação da televisão.
   - Redeployez le bundle télémétrie/configure mis a jour no ambiente touché (staging → canary → produção). Documente as alterações no diário de incidentes.
3. **Validar a substituição.**
   - Relance a busca do dispositivo canônico. Confirme que o placar marca o fornecedor como inelegível com o motivo `policy_denied`.
   - Inspecione `sorafs_orchestrator_provider_failures_total` para verificar se o contador foi aumentado para o fornecedor recusado.
4. **Escale as interdições de longa duração.**
   - Se o fornecedor permanecer bloqueado >24 horas, abra um ticket de governo para fazer girar ou suspender seu anúncio. Apenas vote, salve a lista de recusas e rafraîchissez os instantâneos de télémétrie para evitar que ele não reintegre o placar.
5. **Protocolo de reversão.**
   - Para restaurar o fornecedor, retirar a lista de recusas, reimplantar e capturar um novo instantâneo do placar. Anexe as alterações post mortem do incidente.

## 3. Plano de implementação por etapas

| Fase | Portée | Requisitos de assinatura | Critérios Go/No-Go |
|-------|--------|----------------|-------------------|
| **Laboratório** | Cluster de integração concluído | Fetch CLI manual contra cargas úteis de fixtures | Todos os pedaços reussissent, les compteurs d'échec fournisseur restant a 0, taux de retry < 5%. |
| **Encenação** | Preparação do plano de controle completo | Dashboard Grafana conectado; regras de alerta no modo somente aviso | `sorafs_orchestrator_active_fetches` retorna a zero após cada execução do teste; nenhum alerta `warn/critical`. |
| **Canário** | ≤10% do tráfego de produção | Pager muet mais télémétrie vigiado em tempo real | Proporção de novas tentativas < 10%, recursos limitados a pares extremamente conhecidos, histograma de latência em conformidade com o estadiamento da linha de base ±20%. |
| **Disponibilidade geral** | 100% do lançamento | Regras do pager ativo | Zero erro `NoHealthyProviders` pendente 24 h, taxa de repetição estável, painéis SLA do painel na vertical. |

Despeje a fase chaque:1. Insira hoje o JSON do orquestrador com o `max_providers` e os orçamentos de nova tentativa anteriores.
2. Lance `sorafs_cli fetch` ou o conjunto de testes de integração SDK com o dispositivo canônico e um manifesto representante do ambiente.
3. Capture os artefatos do placar + resumo e anexe-os ao registro de lançamento.
4. Revise os painéis de controle com o engenheiro de controle antes de promovê-los na fase seguinte.

## 4. Observabilidade e riscos de incidentes

- **Métricas:** Certifique-se de que o Alertmanager monitore `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` e `sorafs_orchestrator_retries_total`. Um pic soudain significa geralmente que um fornecedor se degrada sob carga.
- **Logs :** Obtenha os códigos `telemetry::sorafs.fetch.*` para o registrador de logs compartilhado. Crie pesquisas registradas para `event=complete status=failed` para acelerar a triagem.
- **Placares:** Persistez cada artefato de placar e armazenamento a longo prazo. O JSON também está na pista de auditoria para revisões de conformidade e reversões por etapas.
- **Dashboards:** Clone o quadro Grafana canônico (`docs/examples/sorafs_fetch_dashboard.json`) no dossiê de produção com as regras de alerta de `docs/examples/sorafs_fetch_alerts.yaml`.

## 5. Comunicação e documentação

- Registrar todas as alterações de negação/reforço no changelog de exploração com horodatadores, operadores, motivos e incidentes associados.
- Informe as equipes SDK quando os pesos dos fornecedores ou os orçamentos de nova tentativa forem alterados para alinhar os clientes ao cliente.
- Depois que o GA foi encerrado, traga hoje `status.md` com o currículo do lançamento e arquive esta referência do runbook nas notas de versão.
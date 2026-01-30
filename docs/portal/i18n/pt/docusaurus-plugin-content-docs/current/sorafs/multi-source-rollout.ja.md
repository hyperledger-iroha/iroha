---
lang: ja
direction: ltr
source: docs/portal/i18n/pt/docusaurus-plugin-content-docs/current/sorafs/multi-source-rollout.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8d3f61e54bdeb25a54f485111fd54b20281e4859aa6cc2402c82f47051eaf1cf
source_last_modified: "2026-01-04T10:50:53+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Portuguese (pt) translation. Replace this content with the full translation. -->

---
id: multi-source-rollout
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/multi-source-rollout.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


:::note Fonte canônica
Esta página espelha `docs/source/sorafs/runbooks/multi_source_rollout.md`. Mantenha ambas as copias sincronizadas.
:::

## Objetivo

Este runbook orienta os SREs e engenheiros de plantão em dois fluxos críticos:

1. Fazer o rollout do orquestrador multi-origem em ondas controladas.
2. Negar ou despriorizar provedores com mau comportamento sem desestabilizar sessões existentes.

Pressupõe que a pilha de orquestração entregue sob SF-6 já está implantada (`sorafs_orchestrator`, API de intervalo de chunks do gateway, exportadores de telemetria).

> **Veja também:** O [Runbook de operações do orquestrador](./orchestrator-ops.md) aprofunda os procedimentos por execução (captura de scoreboard, toggles de rollout em fases, rollback). Use ambas as referências em conjunto durante mudanças ao vivo.

## 1. Validação pré-voo

1. **Confirmar insumos de governança.**
   - Todos os provedores candidatos devem publicar envelopes `ProviderAdvertV1` com payloads de capacidade de intervalo e orçamentos de stream. Valide via `/v1/sorafs/providers` e compare com os campos de capacidade esperados.
   - Snapshots de telemetria que fornecem taxas de latência/falha devem ter menos de 15 minutos antes de cada execução canária.
2. **Preparar a configuração.**
   - Persista a configuração JSON do orquestrador na árvore em camadas `iroha_config`:

     ```toml
     [torii.sorafs.orchestrator]
     config_path = "/etc/iroha/sorafs/orchestrator.json"
     ```

     Atualize o JSON com limites específicos do rollout (`max_providers`, budgets de retry). Use o mesmo arquivo em staging/produção para manter as diferenças mínimas.
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

     As variáveis de ambiente devem conter o digest do payload do manifesto (hex) e tokens de stream codificados em base64 para cada provedor participante do canary.
   - Compare `artifacts/canary.scoreboard.json` com o release anterior. Qualquer novo provedor inelegível ou mudança de peso >10% exige revisão.
4. **Verificar que a telemetria está conectada.**
   - Abra a exportação do Grafana em `docs/examples/sorafs_fetch_dashboard.json`. Garanta que as métricas `sorafs_orchestrator_*` apareçam em staging antes de avançar.

## 2. Negação emergencial de provedores

Siga este procedimento quando um provedor servir chunks corrompidos, estourar timeouts de forma persistente ou falhar em verificações de compliance.

1. **Capturar evidências.**
   - Exporte o resumo de fetch mais recente (saída de `--json-out`). Registre índices de chunks com falha, aliases de provedores e divergências de digest.
   - Salve trechos de log relevantes dos targets `telemetry::sorafs.fetch.*`.
2. **Aplicar override imediato.**
   - Marque o provedor como penalizado no snapshot de telemetria distribuído ao orquestrador (defina `penalty=true` ou limite `token_health` a `0`). O próximo build do scoreboard excluirá o provedor automaticamente.
   - Para smoke tests ad-hoc, passe `--deny-provider gw-alpha` para `sorafs_cli fetch` para exercitar o caminho de falha sem esperar a propagação da telemetria.
   - Reimplante o bundle atualizado de telemetria/configuração no ambiente afetado (staging → canary → production). Documente a mudança no log de incidente.
3. **Validar o override.**
   - Reexecute o fetch do fixture canônico. Confirme que o scoreboard marca o provedor como inelegível com o motivo `policy_denied`.
   - Inspecione `sorafs_orchestrator_provider_failures_total` para garantir que o contador pare de aumentar para o provedor negado.
4. **Escalar banimentos prolongados.**
   - Se o provedor permanecer bloqueado por >24 h, abra um ticket de governança para rotacionar ou suspender seu advert. Até a votação passar, mantenha a lista de negação e atualize os snapshots de telemetria para que o provedor não volte ao scoreboard.
5. **Protocolo de rollback.**
   - Para reintegrar o provedor, remova-o da lista de negação, reimplante e capture um novo snapshot do scoreboard. Anexe a mudança ao postmortem do incidente.

## 3. Plano de rollout em fases

| Fase | Escopo | Sinais obrigatórios | Critérios Go/No-Go |
|------|--------|---------------------|--------------------|
| **Lab** | Cluster de integração dedicado | Fetch manual por CLI contra payloads de fixtures | Todos os chunks concluem, contadores de falha de provedor ficam em 0, taxa de retries < 5%. |
| **Staging** | Staging de control-plane completo | Dashboard do Grafana conectado; regras de alerta em modo somente warning | `sorafs_orchestrator_active_fetches` volta a zero após cada execução de teste; nenhum alerta `warn/critical`. |
| **Canary** | ≤10% do tráfego de produção | Pager silenciado mas telemetria monitorada em tempo real | Razão de retries < 10%, falhas de provedores isoladas a peers ruidosos conhecidos, histograma de latência coincide com a baseline de staging ±20%. |
| **Disponibilidade geral** | 100% do rollout | Regras do pager ativas | Zero erros `NoHealthyProviders` por 24 h, razão de retries estável, painéis SLA do dashboard em verde. |

Para cada fase:

1. Atualize o JSON do orquestrador com os `max_providers` e budgets de retry previstos.
2. Execute `sorafs_cli fetch` ou a suite de testes de integração do SDK contra o fixture canônico e um manifesto representativo do ambiente.
3. Capture os artefatos de scoreboard + summary e anexe-os ao registro de release.
4. Revise os dashboards de telemetria com o engenheiro de plantão antes de promover para a próxima fase.

## 4. Observabilidade e ganchos de incidente

- **Métricas:** Garanta que o Alertmanager monitore `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` e `sorafs_orchestrator_retries_total`. Um pico repentino geralmente significa que um provedor está degradando sob carga.
- **Logs:** Roteie os targets `telemetry::sorafs.fetch.*` para o agregador de logs compartilhado. Crie buscas salvas para `event=complete status=failed` para acelerar o triage.
- **Scoreboards:** Persista cada artefato de scoreboard em armazenamento de longo prazo. O JSON também serve como trilha de evidência para revisões de compliance e rollbacks por fase.
- **Dashboards:** Clone o dashboard Grafana canônico (`docs/examples/sorafs_fetch_dashboard.json`) na pasta de produção com as regras de alerta de `docs/examples/sorafs_fetch_alerts.yaml`.

## 5. Comunicação e documentação

- Registre cada mudança de deny/boost no changelog de operações com timestamp, operador, motivo e incidente associado.
- Notifique as equipes de SDK quando os pesos de provedores ou os budgets de retry mudarem para alinhar as expectativas do lado do cliente.
- Depois que a GA terminar, atualize `status.md` com o resumo do rollout e arquive esta referência de runbook nas notas de release.

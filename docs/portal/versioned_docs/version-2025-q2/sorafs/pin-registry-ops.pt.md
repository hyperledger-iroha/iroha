---
lang: pt
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sorafs/pin-registry-ops.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0dc64bb4067d734250852a74a65a2100bd68e5ff35f9e8e9dbf3bd2b86f00cfa
source_last_modified: "2026-01-22T15:38:30.656337+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
id: pin-registry-ops-pt
slug: /sorafs/pin-registry-ops-pt
---

:::nota Fonte Canônica
Espelhos `docs/source/sorafs/runbooks/pin_registry_ops.md`. Mantenha ambas as versões alinhadas entre os lançamentos.
:::

## Visão geral

Este runbook documenta como monitorar e fazer a triagem do registro de pinos SoraFS e seus SLAs (acordos de nível de serviço) de replicação. As métricas são originadas de `iroha_torii` e são exportadas via Prometheus no namespace `torii_sorafs_*`. Torii faz uma amostragem do estado do registro em um intervalo de 30 segundos em segundo plano, para que os painéis permaneçam atualizados mesmo quando nenhum operador estiver pesquisando os endpoints `/v1/sorafs/pin/*`. Importe o painel selecionado (`docs/source/grafana_sorafs_pin_registry.json`) para obter um layout Grafana pronto para uso que mapeia diretamente para as seções abaixo.

## Referência de Métrica

| Métrica | Etiquetas | Descrição |
| ------ | ------ | ----------- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \| `approved` \| `retired`) | Inventário de manifesto na cadeia por estado do ciclo de vida. |
| `torii_sorafs_registry_aliases_total` | — | Contagem de aliases de manifesto ativos registrados no registro. |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \| `completed` \| `expired`) | Backlog de pedidos de replicação segmentado por status. |
| `torii_sorafs_replication_backlog_total` | — | Medidor de conveniência espelhando pedidos `pending`. |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \| `missed` \| `pending`) | Contabilidade de SLA: `met` conta pedidos concluídos dentro do prazo, `missed` agrega conclusões atrasadas + expirações, `pending` reflete pedidos pendentes. |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Latência de conclusão agregada (períodos entre a emissão e a conclusão). |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Janelas de folga de ordens pendentes (prazo menos época de emissão). |

Todos os medidores são redefinidos a cada captura de snapshot, portanto, os painéis devem fazer amostragem na cadência `1m` ou mais rápido.

## Painel Grafana

O painel JSON é fornecido com sete painéis que cobrem os fluxos de trabalho do operador. As consultas estão listadas abaixo para referência rápida se você preferir construir gráficos personalizados.

1. **Ciclo de vida do manifesto** – `torii_sorafs_registry_manifests_total` (agrupado por `status`).
2. **Tendência do catálogo de alias** – `torii_sorafs_registry_aliases_total`.
3. **Fila de pedidos por status** – `torii_sorafs_registry_orders_total` (agrupado por `status`).
4. **Backlog vs pedidos expirados** – combina `torii_sorafs_replication_backlog_total` e `torii_sorafs_registry_orders_total{status="expired"}` para saturação superficial.
5. **Taxa de sucesso do SLA** –

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```

6. **Latência vs folga de prazo** – sobreposição `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` e `torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`. Use as transformações Grafana para adicionar visualizações `min_over_time` quando precisar da folga mínima absoluta, por exemplo:

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. **Pedidos perdidos (taxa de 1h)** –

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## Limites de alerta- **Sucesso de SLA  0**
  Limite: `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - Ação: Inspecione os manifestos de governança para confirmar a rotatividade de fornecedores.
- **Conclusão p95 > média de folga no prazo**
  Limite: `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - Ação: Verifique se os fornecedores estão se comprometendo antes dos prazos; considere emitir reatribuições.

### Exemplo de regras Prometheus

```yaml
groups:
  - name: sorafs-pin-registry
    rules:
      - alert: SorafsReplicationSlaDrop
        expr: sum(torii_sorafs_replication_sla_total{outcome="met"}) /
          clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) < 0.95
        for: 15m
        labels:
          severity: page
        annotations:
          summary: "SoraFS replication SLA below target"
          description: "SLA success ratio stayed under 95% for 15 minutes."

      - alert: SorafsReplicationBacklogGrowing
        expr: torii_sorafs_replication_backlog_total > 10
        for: 10m
        labels:
          severity: page
        annotations:
          summary: "SoraFS replication backlog above threshold"
          description: "Pending replication orders exceeded the configured backlog budget."

      - alert: SorafsReplicationExpiredOrders
        expr: increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0
        for: 0m
        labels:
          severity: ticket
        annotations:
          summary: "SoraFS replication orders expired"
          description: "At least one replication order expired in the last five minutes."
```

## Fluxo de trabalho de triagem

1. **Identificar a causa**
   - Se o SLA perder o pico enquanto o backlog permanecer baixo, concentre-se no desempenho do provedor (falhas de PoR, conclusões tardias).
   - Se o backlog crescer com perdas estáveis, inspecionar a admissão (`/v1/sorafs/pin/*`) para confirmar os manifestos que aguardam aprovação do conselho.
2. **Validar status do provedor**
   - Execute `iroha app sorafs providers list` e verifique se os recursos anunciados correspondem aos requisitos de replicação.
   - Verifique os medidores `torii_sorafs_capacity_*` para confirmar o sucesso do GiB e PoR provisionado.
3. **Reatribuir replicação**
   - Emitir novos pedidos via `sorafs_manifest_stub capacity replication-order` quando a folga do backlog (`stat="avg"`) cair abaixo de 5 épocas (embalagem de manifesto/CAR usa `iroha app sorafs toolkit pack`).
   - Notificar a governança se os aliases não tiverem ligações de manifesto ativas (`torii_sorafs_registry_aliases_total` cai inesperadamente).
4. **Resultado do documento**
   - Registre notas de incidentes no log de operações SoraFS com carimbos de data/hora e resumos de manifestos afetados.
   - Atualize este runbook se novos modos de falha ou painéis forem introduzidos.

## Plano de implementação

Siga este procedimento em etapas ao ativar ou restringir a política de cache de alias na produção:1. **Preparar configuração**
   - Atualize `torii.sorafs_alias_cache` em `iroha_config` (usuário → real) com os TTLs acordados e janelas de graça: `positive_ttl`, `refresh_window`, `hard_expiry`, `negative_ttl`, `revocation_ttl`, `rotation_max_age`, `successor_grace` e `governance_grace`. Os padrões correspondem à política em `docs/source/sorafs_alias_policy.md`.
   - Para SDKs, distribua os mesmos valores por meio de suas camadas de configuração (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` em ligações Rust/NAPI/Python) para que a aplicação do cliente corresponda ao gateway.
2. **Teste na preparação**
   - Implante a alteração de configuração em um cluster de preparo que espelhe a topologia de produção.
   - Execute `cargo xtask sorafs-pin-fixtures` para confirmar se os fixtures de alias canônicos ainda são decodificados e ida e volta; qualquer incompatibilidade implica desvio de manifesto upstream que deve ser resolvido primeiro.
   - Exercite os endpoints `/v1/sorafs/pin/{digest}` e `/v1/sorafs/aliases` com provas sintéticas cobrindo casos recentes, de janela de atualização, expirados e expirados. Valide os códigos de status HTTP, cabeçalhos (`Sora-Proof-Status`, `Retry-After`, `Warning`) e campos de corpo JSON em relação a este runbook.
3. **Ativar em produção**
   - Implemente a nova configuração através da janela de alteração padrão. Aplique-o primeiro a Torii e, em seguida, reinicie os gateways/serviços SDK assim que o nó confirmar a nova política nos logs.
   - Importe `docs/source/grafana_sorafs_pin_registry.json` para Grafana (ou atualize os painéis existentes) e fixe os painéis de atualização do cache de alias no espaço de trabalho NOC.
4. **Verificação pós-implantação**
   - Monitore `torii_sorafs_alias_cache_refresh_total` e `torii_sorafs_alias_cache_age_seconds` por 30 minutos. Os picos nas curvas `error`/`expired` devem estar correlacionados com as janelas de atualização da política; o crescimento inesperado significa que as operadoras devem inspecionar as provas de alias e a integridade do provedor antes de continuar.
   - Confirme se os logs do lado do cliente mostram as mesmas decisões políticas (os SDKs apresentarão erros quando a prova estiver obsoleta ou expirar). A ausência de avisos do cliente indica uma configuração incorreta.
5. **Substituição**
   - Se a emissão do alias ficar atrasada e a janela de atualização disparar com frequência, relaxe temporariamente a política aumentando `refresh_window` e `positive_ttl` na configuração e, em seguida, reimplante. Mantenha `hard_expiry` intacto para que provas verdadeiramente obsoletas ainda sejam rejeitadas.
   - Reverta para a configuração anterior restaurando o instantâneo `iroha_config` anterior se a telemetria continuar a mostrar contagens elevadas de `error` e, em seguida, abra um incidente para rastrear atrasos na geração de alias.

## Materiais Relacionados

- `docs/source/sorafs/pin_registry_plan.md` — roteiro de implementação e contexto de governação.
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` — operações de trabalho de armazenamento, complementa este manual de registro.

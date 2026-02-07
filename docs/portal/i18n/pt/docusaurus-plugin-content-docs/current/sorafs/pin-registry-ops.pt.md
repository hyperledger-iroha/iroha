---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-ops.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pin-registry-ops
título: Operações do Pin Registry
sidebar_label: Operações do Pin Registry
description: Monitorar e fazer triagem do Pin Registry do SoraFS e das métricas de SLA de replicacao.
---

:::nota Fonte canônica
Espelha `docs/source/sorafs/runbooks/pin_registry_ops.md`. Mantenha ambas as versões sincronizadas até a aposentadoria da documentação herdada do Sphinx.
:::

## Visão geral

Este runbook documenta como monitorar e fazer rastreamento do Pin Registry do SoraFS e seus acordos de nível de serviço (SLA) de replicação. As métricas são originadas de `iroha_torii` e são exportadas via Prometheus sob o namespace `torii_sorafs_*`. Torii amostra o estado do registro em um intervalo de 30 segundos em segundo plano, portanto os dashboards permanecem atualizados mesmo quando nenhum operador está consultando os endpoints `/v1/sorafs/pin/*`. Importe o dashboard curado (`docs/source/grafana_sorafs_pin_registry.json`) para um layout do Grafana pronto para uso que mapeia diretamente para as seções abaixo.

## Referência de métricas

| Métrica | Etiquetas | Descrição |
| ------- | ------ | --------- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \| `approved` \| `retired`) | Inventário de manifestos on-chain por estado do ciclo de vida. |
| `torii_sorafs_registry_aliases_total` | - | Contagem de aliases de manifestos ativos registrados no registro. |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \| `completed` \| `expired`) | Backlog de ordens de replicação segmentadas por status. |
| `torii_sorafs_replication_backlog_total` | - | Medidor de conveniência que reflete ordens `pending`. |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \| `missed` \| `pending`) | Contabilidade de SLA: `met` conta ordens concluídas dentro do prazo, `missed` agrega conclusões tardias + expirações, `pending` espelhar ordens pendentes. |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Latência agregada de conclusão (épocas entre emissão e conclusão). |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Janelas de folga de ordens pendentes (prazo menos época de emissão). |

Todos os medidores reiniciam cada snapshot, então os dashboards devem ser amostrados em cadência `1m` ou mais rapidamente.

## Painel do Grafana

O JSON do dashboard inclui sete tarefas que cobrem fluxos de trabalho de operadores. As consultas abaixo servem como referência rápida caso você prefira montar gráficos sob medida.

1. **Ciclo de vida de manifestos** - `torii_sorafs_registry_manifests_total` (agrupado por `status`).
2. **Tendência do catálogo de alias** - `torii_sorafs_registry_aliases_total`.
3. **Fila de pedidos por status** - `torii_sorafs_registry_orders_total` (agrupado por `status`).
4. **Backlog vs ordens expiradas** - combina `torii_sorafs_replication_backlog_total` e `torii_sorafs_registry_orders_total{status="expired"}` para mostrar saturação.
5. **Razão de sucesso de SLA** -

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```6. **Latência vs folga do prazo** - sobrepoe `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` e `torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`. Use transformações do Grafana para adicionar vistas `min_over_time` quando precisar do piso absoluto de folga, por exemplo:

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. **Pedidos perdidos (taxa 1h)** -

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## Limiares de alerta

- **Sucesso de SLA  0**
  - Limiar: `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - Ação: Inspecionar manifestos de governança para confirmar rotatividade de provedores.
- **p95 de conclusão > folga media de prazo**
  - Limiar: `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - Ação: Verificar se os fornecedores estão cumprindo antes dos prazos; considerar retribuições.

### Regras de Prometheus de exemplo

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
          summary: "SLA de replicacao SoraFS abaixo da meta"
          description: "A razao de sucesso do SLA ficou abaixo de 95% por 15 minutos."

      - alert: SorafsReplicationBacklogGrowing
        expr: torii_sorafs_replication_backlog_total > 10
        for: 10m
        labels:
          severity: page
        annotations:
          summary: "Backlog de replicacao SoraFS acima do limiar"
          description: "Ordens de replicacao pendentes excederam o budget configurado."

      - alert: SorafsReplicationExpiredOrders
        expr: increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0
        for: 0m
        labels:
          severity: ticket
        annotations:
          summary: "Ordens de replicacao SoraFS expiradas"
          description: "Pelo menos uma ordem de replicacao expirou nos ultimos cinco minutos."
```

## Fluxo de triagem

1. **Identificador de causa**
   - Se as falhas de SLA aumentarem enquanto o backlog permanecer baixo, foque no desempenho dos provedores (falhas de PoR, conclusões tardias).
   - Se o backlog crescer com faltas estaveis, inspecione a admissão (`/v1/sorafs/pin/*`) para confirmar manifestos aguardando aprovação do conselho.
2. **Validar status dos provedores**
   - Execute `iroha app sorafs providers list` e verifique se as capacidades anunciadas atendem aos requisitos de replicação.
   - Verifique os medidores `torii_sorafs_capacity_*` para confirmar GiB provisionados e sucesso de PoR.
3. **Reatribuir replicacao**
   - Emita novas ordens via `sorafs_manifest_stub capacity replication-order` quando a folga do backlog (`stat="avg"`) cair abaixo de 5 épocas (o empacotamento de manifesto/CAR usa `iroha app sorafs toolkit pack`).
   - Notifique a governanca se os aliases não tenham ligações ativas de manifesto (quedas inesperadas em `torii_sorafs_registry_aliases_total`).
4. **Documento resultado**
   - Registre notas do incidente no log de operações do SoraFS com timestamps e resumos de manifestos afetados.
   - Atualizar este runbook se novos modos de falha ou painéis forem implementados.

## Plano de implementação

Siga este procedimento nas etapas para habilitar ou suportar a política de cache de alias em produção:1. **Preparar configuração**
   - Atualizar `torii.sorafs_alias_cache` em `iroha_config` (usuário -> atual) com os TTLs e janelas de graça acordados: `positive_ttl`, `refresh_window`, `hard_expiry`, `negative_ttl`, `revocation_ttl`, `rotation_max_age`, `successor_grace` e `governance_grace`. Os padrões envolvem a política em `docs/source/sorafs_alias_policy.md`.
   - Para SDKs, distribui os mesmos valores por suas camadas de configuração (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` em binds Rust / NAPI / Python) para que a aplicação do cliente corresponda ao gateway.
2. **Teste de simulação**
   - Implante uma mudança de configuração em um cluster de staging que espele a topologia de produção.
   - Execute `cargo xtask sorafs-pin-fixtures` para confirmar que os fixtures canônicos de alias ainda decodificam e fazem round-trip; qualquer incompatibilidade implica desvio a montante que deve ser resolvido primeiro.
   - Exerça os endpoints `/v1/sorafs/pin/{digest}` e `/v1/sorafs/aliases` com provas sintéticas cobrindo casos frescos, janela de atualização, expirados e expirados. Valide os códigos HTTP, cabeçalhos (`Sora-Proof-Status`, `Retry-After`, `Warning`) e campos do corpo JSON contra este runbook.
3. **Habilitar em produção**
   - Implante uma nova configuração na janela padrão de mudanças. Aplique primeiro ao Torii e depois reinicie gateways/serviços SDK assim que o nó confirmar a nova política nos logs.
   - Importe `docs/source/grafana_sorafs_pin_registry.json` no Grafana (ou atualize dashboards existentes) e corrija os problemas de atualização do cache de alias no workspace do NOC.
4. **Verificação pós-implantação**
   - Monitore `torii_sorafs_alias_cache_refresh_total` e `torii_sorafs_alias_cache_age_seconds` por 30 minutos. Picos nas curvas `error`/`expired` correlacionados com as janelas de atualização; O crescimento inesperado significa que os operadores devem operar testes de alias e saude dos provedores antes de continuar.
   - Confirme que os logs do lado do cliente mostram as mesmas decisões de política (os SDKs emitiram erros quando a prova estava obsoleta ou expirada). Ausência de avisos do cliente indica configuração incorreta.
5. **Substituição**
   - Se a emissao de alias atrasar e a janela de refresh disparar com frequencia, relaxe temporariamente a politica aumentando `refresh_window` e `positive_ttl` na config, depois reimplante. Mantenha `hard_expiry` intacto para que provas realmente obsoletas ainda sejam descartadas.
   - Reverta para a configuração anterior restaurando o snapshot anterior de `iroha_config` se a telemetria continuar mostrando contagens `error` elevadas, então abra um incidente para rastrear atrasos na geração de alias.

## Materiais relacionados

- `docs/source/sorafs/pin_registry_plan.md` - roadmap de implementação e contexto de governança.
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` - operações do trabalhador de armazenamento, complementando este playbook do registro.
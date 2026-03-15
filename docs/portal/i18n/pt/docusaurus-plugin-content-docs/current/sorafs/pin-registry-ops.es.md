---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-ops.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pin-registry-ops
título: Operaciones del Pin Registry
sidebar_label: Operações do Registro Pin
description: Monitore e faça uma triagem do Pin Registry de SoraFS e das métricas de SLA de replicação.
---

:::nota Fonte canônica
Reflexo `docs/source/sorafs/runbooks/pin_registry_ops.md`. Mantenha ambas as versões sincronizadas até que a documentação herdada do Sphinx seja retirada.
:::

## Resumo

Este runbook documenta como monitorar e fazer a triagem do Pin Registry de SoraFS e seus acordos de nível de serviço (SLA) de replicação. As métricas fornecem `iroha_torii` e são exportadas através de Prometheus para o namespace `torii_sorafs_*`. Torii mostra o estado do registro em um intervalo de 30 segundos no segundo plano, para que os painéis sejam mantidos atualizados, mesmo quando nenhum operador estiver consultando os endpoints `/v1/sorafs/pin/*`. Importe o painel curado (`docs/source/grafana_sorafs_pin_registry.json`) para um layout de lista Grafana para usá-lo mapeando diretamente nas seções seguintes.

## Referência de métricas

| Métrica | Etiquetas | Descrição |
| ------ | ------ | ----------- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \| `approved` \| `retired`) | Inventário de manifestos on-chain por estado do ciclo de vida. |
| `torii_sorafs_registry_aliases_total` | — | Conteúdo de aliases de ativos manifestos registrados no registro. |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \| `completed` \| `expired`) | Backlog de ordens de replicação segmentadas por estado. |
| `torii_sorafs_replication_backlog_total` | — | Medidor de conveniência que reflete as ordens `pending`. |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \| `missed` \| `pending`) | Contabilidade de SLA: `met` contas ordenadas completadas dentro do prazo, `missed` agregadas concluídas tarde + expirações, `pending` reflexo de ordens pendentes. |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Latência agregada de finalização (épocas entre emissão e conclusão). |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Ventanas de holgura de ordens pendentes (prazo menos época de emissão). |

Todos os medidores são reiniciados em cada captura de instantâneo, de modo que os painéis devem exibir a cadência `1m` ou mais rapidamente.

## Painel de Grafana

O JSON do painel inclui vários painéis que cobrem os fluxos de trabalho dos operadores. As consultas são listadas abaixo para referência rápida e você prefere construir gráficos de acordo com a medida.

1. **Ciclo de vida de manifestos** – `torii_sorafs_registry_manifests_total` (agrupado por `status`).
2. **Tendência do catálogo de alias** – `torii_sorafs_registry_aliases_total`.
3. **Cola de ordem por estado** – `torii_sorafs_registry_orders_total` (agrupado por `status`).
4. **Backlog vs ordens expiradas** – combina `torii_sorafs_replication_backlog_total` e `torii_sorafs_registry_orders_total{status="expired"}` para mostrar a saturação.
5. **Índice de sucesso do SLA** –

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```6. **Latência vs duração do prazo** – superpone `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` e `torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`. Use transformações de Grafana para obter vistas `min_over_time` quando precisar do piso absoluto de holgura, por exemplo:

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. **Ordenes fallidas (tasa 1h)** –

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## Guarda-chuvas de alerta

- **Éxito do SLA  0**
  - Umbral: `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - Ação: Inspecionar manifestos de governo para confirmar a rotatividade de provedores.
- **p95 de completado > holgura promessa de prazo**
  - Umbral: `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - Ação: Verifique se os fornecedores cumprem o plano antes do prazo; considerar reasignações.

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
          summary: "SLA de replicación de SoraFS por debajo del objetivo"
          description: "El ratio de éxito del SLA se mantuvo por debajo de 95% durante 15 minutos."

      - alert: SorafsReplicationBacklogGrowing
        expr: torii_sorafs_replication_backlog_total > 10
        for: 10m
        labels:
          severity: page
        annotations:
          summary: "Backlog de replicación de SoraFS por encima del umbral"
          description: "Las órdenes pendientes excedieron el presupuesto de backlog configurado."

      - alert: SorafsReplicationExpiredOrders
        expr: increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0
        for: 0m
        labels:
          severity: ticket
        annotations:
          summary: "Órdenes de replicación de SoraFS expiradas"
          description: "Al menos una orden de replicación expiró en los últimos cinco minutos."
```

## Fluxo de teste

1. **Identificador de causa**
   - Se as falhas de SLA sobem enquanto o backlog se mantiver baixo, concentre-se no rendimiento de provedores (falhas de PoR, concluídas tardiamente).
   - Se o backlog crescer com faltas estáveis, inspecione a admissão (`/v1/sorafs/pin/*`) para confirmar os manifestos na espera de aprovação do conselho.
2. **Validar estado de provedores**
   - Execute `iroha app sorafs providers list` e verifique se as capacidades anunciadas cumprem os requisitos de replicação.
   - Revisa os medidores `torii_sorafs_capacity_*` para confirmar GiB provisionados e exito de PoR.
3. **Reasignar replicação**
   - Emite novas ordens via `sorafs_manifest_stub capacity replication-order` quando o backlog do backlog (`stat="avg"`) ocorre por menos de 5 temporadas (o pacote de manifesto/CAR usa `iroha app sorafs toolkit pack`).
   - Notificar a governança se os aliases carecem de ligações de ativos manifestos (caídas inesperadas de `torii_sorafs_registry_aliases_total`).
4. **Documento resultado**
   - Registrar notas de incidente no log de operações de SoraFS com carimbos de data e hora e resumos de manifestos afetados.
   - Atualize este runbook se aparecerem novos modos de falha ou dashboards.

## Plano de despliegue

Siga este procedimento por etapas para habilitar ou suportar a política de cache de alias na produção:1. **Preparar configuração**
   - Atualize `torii.sorafs_alias_cache` em `iroha_config` (usuário -> real) com TTL e janelas de graça acordadas: `positive_ttl`, `refresh_window`, `hard_expiry`, `negative_ttl`, `revocation_ttl`, `rotation_max_age`, `successor_grace` e `governance_grace`. Os padrões coincidem com a política em `docs/source/sorafs_alias_policy.md`.
   - Para SDKs, distribua os mesmos valores por meio de suas capas de configuração (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` em ligações Rust / NAPI / Python) para que o aplicativo cliente coincida com o gateway.
2. **Teste e simulação**
   - Despligue a mudança de configuração em um cluster de teste que reflete a topologia de produção.
   - Execute `cargo xtask sorafs-pin-fixtures` para confirmar que os fixtures canônicos de alias são decodificados e feitos de ida e volta; Qualquer incompatibilidade implica deriva em águas arribas que devem ser resolvidas primeiro.
   - Execute os endpoints `/v1/sorafs/pin/{digest}` e `/v1/sorafs/aliases` com testes sintéticos que cobrem casos frescos, de janela de atualização, expirados e expirados. Valide os códigos HTTP, cabeçalhos (`Sora-Proof-Status`, `Retry-After`, `Warning`) e campos do corpo JSON contra este runbook.
3. **Habilitar em produção**
   - Despligue a nova configuração na janela padrão de mudanças. Aplique primeiro a Torii e depois reinicie gateways/serviços SDK uma vez que o nó confirme a nova política em logs.
   - Importe `docs/source/grafana_sorafs_pin_registry.json` em Grafana (ou atualize os painéis existentes) e fixe os painéis de atualização de cache de alias no espaço de trabalho do NOC.
4. **Verificação pós-despliegue**
   - Monitorea `torii_sorafs_alias_cache_refresh_total` e `torii_sorafs_alias_cache_age_seconds` durante 30 minutos. Picos nas curvas `error`/`expired` devem ser correlacionados com as janelas de atualização; O crescimento inesperado implica que os operadores devem inspecionar os alias e a saúde dos provedores antes de continuar.
   - Confirme que os logs do lado do cliente exibem as decisões políticas erradas (os SDKs mostrarão erros quando o teste estiver obsoleto ou expirado). A ausência de avisos do cliente indica uma configuração incorreta.
5. **Substituição**
   - Se a emissão do alias for retrasada e a janela de atualização for disparada com frequência, relaxe temporalmente a política aumentando `refresh_window` e `positive_ttl` na configuração e volte a desplegar. Mantenha `hard_expiry` intacto para que os testes realmente obsoletos sejam rechaçados.
   - Revise a configuração anterior restaurando o instantâneo anterior de `iroha_config` se a telemetria continuar exibindo conteúdos `error` elevados, depois abra um incidente para rastrear retrocessos na geração de alias.

## Materiais relacionados

- `docs/source/sorafs/pin_registry_plan.md` — roteiro de implementação e contexto de governo.
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` — operações do trabalhador de armazenamento, complementando este playbook de registro.
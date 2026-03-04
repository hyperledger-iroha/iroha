---
id: pin-registry-ops
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-ops.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Fonte canonica
Espelha `docs/source/sorafs/runbooks/pin_registry_ops.md`. Mantenha ambas as versoes sincronizadas ate a aposentadoria da documentacao herdada do Sphinx.
:::

## Visao geral

Este runbook documenta como monitorar e fazer triagem do Pin Registry do SoraFS e seus acordos de nivel de servico (SLA) de replicacao. As metricas se originam de `iroha_torii` e sao exportadas via Prometheus sob o namespace `torii_sorafs_*`. Torii amostra o estado do registry em um intervalo de 30 segundos em segundo plano, portanto os dashboards permanecem atualizados mesmo quando nenhum operador esta consultando os endpoints `/v1/sorafs/pin/*`. Importe o dashboard curado (`docs/source/grafana_sorafs_pin_registry.json`) para um layout do Grafana pronto para uso que mapeia diretamente para as secoes abaixo.

## Referencia de metricas

| Metrica | Labels | Descricao |
| ------- | ------ | --------- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \| `approved` \| `retired`) | Inventario de manifests on-chain por estado do ciclo de vida. |
| `torii_sorafs_registry_aliases_total` | - | Contagem de aliases de manifest ativos registrados no registry. |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \| `completed` \| `expired`) | Backlog de ordens de replicacao segmentado por status. |
| `torii_sorafs_replication_backlog_total` | - | Gauge de conveniencia que espelha ordens `pending`. |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \| `missed` \| `pending`) | Contabilidade de SLA: `met` conta ordens concluidas dentro do deadline, `missed` agrega conclusoes tardias + expiracoes, `pending` espelha ordens pendentes. |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Latencia agregada de conclusao (epocas entre emissao e conclusao). |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Janelas de folga de ordens pendentes (deadline menos epoca de emissao). |

Todos os gauges reiniciam a cada snapshot, entao os dashboards devem amostrar em cadencia `1m` ou mais rapida.

## Dashboard do Grafana

O JSON do dashboard inclui sete paineis que cobrem fluxos de trabalho de operadores. As consultas abaixo servem como referencia rapida caso voce prefira montar graficos sob medida.

1. **Ciclo de vida de manifests** - `torii_sorafs_registry_manifests_total` (agrupado por `status`).
2. **Tendencia do catalogo de alias** - `torii_sorafs_registry_aliases_total`.
3. **Fila de ordens por status** - `torii_sorafs_registry_orders_total` (agrupado por `status`).
4. **Backlog vs ordens expiradas** - combina `torii_sorafs_replication_backlog_total` e `torii_sorafs_registry_orders_total{status="expired"}` para mostrar saturacao.
5. **Razao de sucesso de SLA** -

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```

6. **Latencia vs folga do deadline** - sobrepoe `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` e `torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`. Use transformacoes do Grafana para adicionar vistas `min_over_time` quando precisar do piso absoluto de folga, por exemplo:

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. **Ordens perdidas (taxa 1h)** -

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## Limiares de alerta

- **Sucesso de SLA < 0.95 por 15 min**
  - Limiar: `sum(torii_sorafs_replication_sla_total{outcome="met"}) / clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) < 0.95`
  - Acao: Acionar SRE; iniciar triagem do backlog de replicacao.
- **Backlog pendente acima de 10**
  - Limiar: `torii_sorafs_replication_backlog_total > 10` sustentado por 10 min
  - Acao: Verificar disponibilidade de providers e o scheduler de capacidade do Torii.
- **Ordens expiradas > 0**
  - Limiar: `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - Acao: Inspecionar manifests de governanca para confirmar churn de providers.
- **p95 de conclusao > folga media de deadline**
  - Limiar: `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - Acao: Verificar se os providers estao cumprindo antes dos deadlines; considerar reatribuicoes.

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

1. **Identificar causa**
   - Se os misses de SLA aumentarem enquanto o backlog permanece baixo, foque no desempenho dos providers (falhas de PoR, conclusoes tardias).
   - Se o backlog crescer com misses estaveis, inspecione a admissao (`/v1/sorafs/pin/*`) para confirmar manifests aguardando aprovacao do conselho.
2. **Validar status dos providers**
   - Execute `iroha app sorafs providers list` e verifique se as capacidades anunciadas atendem aos requisitos de replicacao.
   - Verifique os gauges `torii_sorafs_capacity_*` para confirmar GiB provisionados e sucesso de PoR.
3. **Reatribuir replicacao**
   - Emita novas ordens via `sorafs_manifest_stub capacity replication-order` quando a folga do backlog (`stat="avg"`) cair abaixo de 5 epocas (o empacotamento de manifest/CAR usa `iroha app sorafs toolkit pack`).
   - Notifique a governanca se os aliases nao tiverem bindings ativos de manifest (quedas inesperadas em `torii_sorafs_registry_aliases_total`).
4. **Documentar resultado**
   - Registre notas do incidente no log de operacoes do SoraFS com timestamps e digests de manifest afetados.
   - Atualize este runbook se novos modos de falha ou dashboards forem introduzidos.

## Plano de rollout

Siga este procedimento em etapas ao habilitar ou endurecer a politica de cache de alias em producao:

1. **Preparar configuracao**
   - Atualize `torii.sorafs_alias_cache` em `iroha_config` (user -> actual) com os TTLs e janelas de graca acordados: `positive_ttl`, `refresh_window`, `hard_expiry`, `negative_ttl`, `revocation_ttl`, `rotation_max_age`, `successor_grace` e `governance_grace`. Os defaults correspondem a politica em `docs/source/sorafs_alias_policy.md`.
   - Para SDKs, distribua os mesmos valores por suas camadas de configuracao (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` em bindings Rust / NAPI / Python) para que o enforcement do cliente corresponda ao gateway.
2. **Dry-run em staging**
   - Implante a mudanca de configuracao em um cluster de staging que espelhe a topologia de producao.
   - Execute `cargo xtask sorafs-pin-fixtures` para confirmar que os fixtures canonicos de alias ainda decodificam e fazem round-trip; qualquer mismatch implica drift upstream que deve ser resolvido primeiro.
   - Exerca os endpoints `/v1/sorafs/pin/{digest}` e `/v1/sorafs/aliases` com provas sinteticas cobrindo casos fresh, refresh-window, expired e hard-expired. Valide os codigos HTTP, headers (`Sora-Proof-Status`, `Retry-After`, `Warning`) e campos do corpo JSON contra este runbook.
3. **Habilitar em producao**
   - Implante a nova configuracao na janela padrao de mudancas. Aplique primeiro ao Torii e depois reinicie gateways/servicos SDK assim que o node confirmar a nova politica nos logs.
   - Importe `docs/source/grafana_sorafs_pin_registry.json` no Grafana (ou atualize dashboards existentes) e fixe os paineis de refresh do cache de alias no workspace do NOC.
4. **Verificacao pos-deploy**
   - Monitore `torii_sorafs_alias_cache_refresh_total` e `torii_sorafs_alias_cache_age_seconds` por 30 minutos. Picos nas curvas `error`/`expired` devem correlacionar com as janelas de refresh; crescimento inesperado significa que os operadores devem inspecionar provas de alias e a saude dos providers antes de continuar.
   - Confirme que os logs do lado do cliente mostram as mesmas decisoes de politica (os SDKs emitirao erros quando a prova estiver stale ou expirada). Ausencia de warnings do cliente indica configuracao incorreta.
5. **Fallback**
   - Se a emissao de alias atrasar e a janela de refresh disparar com frequencia, relaxe temporariamente a politica aumentando `refresh_window` e `positive_ttl` na config, depois reimplante. Mantenha `hard_expiry` intacto para que provas realmente stale ainda sejam rejeitadas.
   - Reverta para a configuracao anterior restaurando o snapshot anterior de `iroha_config` se a telemetria continuar mostrando contagens `error` elevadas, entao abra um incidente para rastrear atrasos na geracao de alias.

## Materiais relacionados

- `docs/source/sorafs/pin_registry_plan.md` - roadmap de implementacao e contexto de governanca.
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` - operacoes do worker de storage, complementa este playbook do registry.

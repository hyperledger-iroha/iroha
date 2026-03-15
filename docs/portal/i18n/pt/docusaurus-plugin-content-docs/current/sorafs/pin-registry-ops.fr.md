---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-ops.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pin-registry-ops
título: Operations du Pin Registry
sidebar_label: Operações do Registro Pin
description: Monitore e verifique o Pin Registry SoraFS e as métricas SLA de replicação.
---

:::nota Fonte canônica
Reflete `docs/source/sorafs/runbooks/pin_registry_ops.md`. Gardez as duas versões sincronizadas junto com a documentação herdada do Sphinx.
:::

## Vista do conjunto

Este runbook descreve o comentário do observador e testa o Pin Registry SoraFS e seus acordos de nível de serviço (SLA) de replicação. As métricas fornecidas por `iroha_torii` e são exportadas via Prometheus para o namespace `torii_sorafs_*`. Torii altera o estado do registro durante todos os 30 segundos no final do plano, enquanto os painéis permanecem no mesmo lugar quando nenhum operador interroga os endpoints `/v1/sorafs/pin/*`. Importe o painel selecionado (`docs/source/grafana_sorafs_pin_registry.json`) para um layout Grafana pronto para o emprego que corresponde diretamente às seções ci-dessous.

## Referência de métricas

| Métrica | Etiquetas | Descrição |
| ------- | ------ | ----------- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \| `approved` \| `retired`) | Inventaire des manifests on-chain par état de cycle de vie. |
| `torii_sorafs_registry_aliases_total` | — | Nome de aliases de atos manifestos registrados no registro. |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \| `completed` \| `expired`) | Backlog de ordens de replicação segmentado por status. |
| `torii_sorafs_replication_backlog_total` | — | Medidor de mercadoria refletindo as ordens `pending`. |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \| `missed` \| `pending`) | SLA de compatibilidade: `met` calcula as ordens terminadas no final, `missed` concorda com as conclusões tardias + expirações, `pending` reflete as ordens atendidas. |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Latência de conclusão aprovada (épocas entre emissão e conclusão). |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Fenêtres de marge des orderres en attente (prazo em menos de época de emissão). |

Todos os medidores são reinicializados com cada extração de instantâneo, pois os painéis devem ser atualizados com uma cadência `1m` ou mais rápida.

## Painel Grafana

O JSON do painel contém sete painéis que cobrem os fluxos de trabalho dos operadores. Os requisitos são listados aqui para referência rápida se você preferir construir gráficos sob medida.

1. **Ciclo de vida dos manifestos** – `torii_sorafs_registry_manifests_total` (grupo par `status`).
2. **Tendência do catálogo de alias** – `torii_sorafs_registry_aliases_total`.
3. **Arquivo de pedidos por status** – `torii_sorafs_registry_orders_total` (grupo por `status`).
4. **Backlog vs pedidos expirados** – combine `torii_sorafs_replication_backlog_total` e `torii_sorafs_registry_orders_total{status="expired"}` para evidenciar a saturação.
5. **Relação de SLA de Resgate** –

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```6. **Latência vs margem de prazo** – sobrepor `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` e `torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`. Utilize as transformações Grafana para adicionar pontos de vista `min_over_time` quando você precisar de uma planilha absoluta de margem, por exemplo:

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. **Ordens manqués (taux 1h)** –

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## Seusils d'alerte

- **SLA de sucesso  0**
  -Seuil: `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - Ação: Inspecione os manifestos de governo para confirmar a rotatividade dos provedores.
- **p95 de conclusão > marge moyenne de prazo**
  - Seuil: `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - Ação: Verifica se os provedores são válidos antes dos prazos; preveja des reatribuições.

### Exemplos de regras Prometheus

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
          summary: "SLA de réplication SoraFS sous la cible"
          description: "Le ratio de succès SLA est resté sous 95% pendant 15 minutes."

      - alert: SorafsReplicationBacklogGrowing
        expr: torii_sorafs_replication_backlog_total > 10
        for: 10m
        labels:
          severity: page
        annotations:
          summary: "Backlog de réplication SoraFS au-dessus du seuil"
          description: "Les ordres de réplication en attente ont dépassé le budget de backlog configuré."

      - alert: SorafsReplicationExpiredOrders
        expr: increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0
        for: 0m
        labels:
          severity: ticket
        annotations:
          summary: "Ordres de réplication SoraFS expirés"
          description: "Au moins un ordre de réplication a expiré au cours des cinq dernières minutes."
```

## Fluxo de trabalho de triagem

1. **Identificador da causa**
   - Se o SLA de cheques aumentar quando o backlog estiver fraco, concentre-se na análise do desempenho dos provedores (cheques PoR, conclusões tardias).
   - Se o backlog aumentar com os estábulos de cheques, inspecione a admissão (`/v1/sorafs/pin/*`) para confirmar os manifestos e aguardar a aprovação do conselho.
2. **Validar o estado dos provedores**
   - Execute `iroha app sorafs providers list` e verifique se as capacidades anunciadas correspondem às exigências de replicação.
   - Verifique os medidores `torii_sorafs_capacity_*` para confirmar os GiB provisionados e o sucesso do PoR.
3. **Reatribuir a replicação**
   - Executar novos pedidos via `sorafs_manifest_stub capacity replication-order` durante a margem do backlog (`stat="avg"`) desde 5 épocas (o manifesto de embalagem/CAR utiliza `iroha app sorafs toolkit pack`).
   - Notifique o governo se os aliases não passarem pelas ligações dos atos manifestos (baisse inattendue de `torii_sorafs_registry_aliases_total`).
4. **Documente o resultado**
   - Envie notas de incidentes para o diário de operações SoraFS com carimbos de data e hora e resumos de manifestos em questão.
   - Leia este runbook hoje mesmo se novos modos de verificação ou painéis forem introduzidos.

## Plano de implantação

Siga este procedimento por etapas durante a ativação ou durcissement da política de cache de alias em produção:1. **Preparar a configuração**
   - Transfira para o dia `torii.sorafs_alias_cache` em `iroha_config` (usuário -> real) com TTL e janelas de graça convencionais: `positive_ttl`, `refresh_window`, `hard_expiry`, `negative_ttl`, `revocation_ttl`, `rotation_max_age`, `successor_grace` e `governance_grace`. Os valores por padrão correspondentes à política de `docs/source/sorafs_alias_policy.md`.
   - Para os SDKs, difunda os mesmos valores por meio de seus sofás de configuração (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` nas ligações Rust / NAPI / Python) para que o cliente do aplicativo esteja no gateway.
2. **Teste e simulação**
   - Implemente a alteração da configuração em um cluster de teste que reflita a topologia de produção.
   - Execute `cargo xtask sorafs-pin-fixtures` para confirmar que os fixtures canônicos do alias codificado e da fonte continuam em uma viagem de ida e volta; toda divergência implica uma deriva rio acima para corrigir em primeiro lugar.
   - Execute os endpoints `/v1/sorafs/pin/{digest}` e `/v1/sorafs/aliases` com testes sintéticos atualizados, janela de atualização, expirados e expirados. Valide os códigos HTTP, os cabeçalhos (`Sora-Proof-Status`, `Retry-After`, `Warning`) e os campeões do corpo JSON neste runbook.
3. **Ativo em produção**
   - Implemente a nova configuração dependendo da janela de mudança padrão. O aplicativo a bordo do Torii e, em seguida, reinicia os gateways/serviços SDK uma vez que o nó confirma a nova política nos logs.
   - Importe `docs/source/grafana_sorafs_pin_registry.json` em Grafana (ou atualize os painéis existentes) e gere os painéis de atualização do cache de alias no espaço de trabalho NOC.
4. **Verificação pós-implantação**
   - Vigilante `torii_sorafs_alias_cache_refresh_total` e `torii_sorafs_alias_cache_age_seconds` pendente 30 minutos. As fotos nos campos `error`/`expired` devem ser corrigidas com as janelas de atualização; Um croissance desatendido significa que os operadores devem inspecionar as recomendações de alias e a saúde dos fornecedores antes de continuar.
   - Confirme se os registros do cliente apresentam as mesmas decisões políticas (os SDKs permanecem com erros quando a verificação é perdida ou expirada). A ausência de avisos faz com que o cliente indique uma configuração incorreta.
5. **Substituição**
   - Se a emissão do alias atrasar e a janela de atualização diminuir com frequência, reinicie temporariamente a política aumentando `refresh_window` e `positive_ttl` na configuração e depois reimplante. Guarde `hard_expiry` intacto para que as amostras remanescentes sejam sempre rejeitadas.
   - Retornar à configuração anterior no restaurante o snapshot `iroha_config` anterior se a transmissão continuar exibindo as contas `error` superiores, e então abrir um incidente para rastrear o atraso na geração do alias.

## Matériaux liés

- `docs/source/sorafs/pin_registry_plan.md` — roteiro de implementação e contexto de governança.
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` — operações de trabalhadores de estoque, registro completo do manual.
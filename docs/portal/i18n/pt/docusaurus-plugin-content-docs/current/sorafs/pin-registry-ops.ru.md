---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-ops.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pin-registry-ops
título: Operação Pin Registry
sidebar_label: Operação do Registro de Pins
descrição: Monitoramento e triagem Pin Registry SoraFS e métrica SLA репликации.
---

:::nota História Canônica
Verifique `docs/source/sorafs/runbooks/pin_registry_ops.md`. Держите обе версии синхронизированными, пока наследственная документация Sphinx não будет выведена из эксплуатации.
:::

##Obzor

Esta é a descrição do runbook, como monitorar e usar triagem Pin Registry SoraFS e sua solicitação de serviço de serviço (SLA) para replicações. As métricas são postadas em `iroha_torii` e exportadas para Prometheus no namespace `torii_sorafs_*`. Torii опрашивает состояние registro caждые 30 segundos no telefone, os painéis de controle são atualizados atualmente Os operadores não devem atualizar os endpoints `/v2/sorafs/pin/*`. Importar painel de controle (`docs/source/grafana_sorafs_pin_registry.json`) para layout de arquivo Grafana, который напрямую соответствует разделам não.

## Métrica de precisão

| Métrica | Etiquetas | Descrição |
| ------ | ------ | -------- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \| `approved` \| `retired`) | Инвентаризация manifesta-se na cadeia por meio de uma ciclovia. |
| `torii_sorafs_registry_aliases_total` | — | Количество активных alias manifests, записанных в registro. |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \| `completed` \| `expired`) | O backlog gera replicações, segmentação por status. |
| `torii_sorafs_replication_backlog_total` | — | Medidor de emergência, отражающий `pending` заказы. |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \| `missed` \| `pending`) | Учет SLA: `met` считает заказы, завершенные в срок, `missed` агрегирует поздние завершения + teste, `pending` não é compatível. |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Агрегированная латентность завершения (эпохи между выпуском и завершением). |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Окна запаса для незавершенных заказов (prazo menos эпоха выдачи). |

Todos os medidores são usados ​​​​no pull snapshot, os painéis de controle são instalados no local `1m` ou na fonte.

## Painel Grafana

O painel JSON contém o painel de controle, permitindo que você execute o processo de operação. As configurações são boas para o seu negócio, mas você está construindo gráficos personalizados.

1. **Manifestos de ciclo de controle** – `torii_sorafs_registry_manifests_total` (gravado em `status`).
2. **Alias ​​do catálogo de domínio** – `torii_sorafs_registry_aliases_total`.
3. **Configurar o estado no estado** – `torii_sorafs_registry_orders_total` (registro em `status`).
4. **Backlog vs истекшие заказы** – use `torii_sorafs_replication_backlog_total` e `torii_sorafs_registry_orders_total{status="expired"}` para obter sucesso.
5. **SLA de utilização** –

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```6. **Pronto vs prazo de entrega** – escolha `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` e `torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`. Use a conversão Grafana, que é a solução `min_over_time`, que é nova абсолютный нижний предел запаса, por exemplo:

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. **Пропущенные заказы (taxa 1h)** –

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## Alertas de segurança

- **SLA superior  0**
  - Por: `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - Действие: Проверить manifestos de governança, чтобы подтвердить provedores de rotatividade.
- **p95 завершения > средний запас до prazo**
  - Por: `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - Действие: Убедиться, что provedores завершают до prazo; рассмотреть перераспределение.

### Exemplo de exemplo Prometheus

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
          summary: "SLA репликации SoraFS ниже целевого"
          description: "Коэффициент успеха SLA оставался ниже 95% в течение 15 минут."

      - alert: SorafsReplicationBacklogGrowing
        expr: torii_sorafs_replication_backlog_total > 10
        for: 10m
        labels:
          severity: page
        annotations:
          summary: "Backlog репликации SoraFS выше порога"
          description: "Ожидающие заказы репликации превысили настроенный бюджет backlog."

      - alert: SorafsReplicationExpiredOrders
        expr: increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0
        for: 0m
        labels:
          severity: ticket
        annotations:
          summary: "Заказы репликации SoraFS истекли"
          description: "По крайней мере один заказ репликации истек за последние пять минут."
```

## Triagem de fluxo de trabalho

1. **Princípio de segurança**
   - Qualquer proposta de SLA растут, um backlog остается низким, сосредоточьтесь на производительности provedores (сбои PoR, поздние segurança).
   - Если backlog растет при стабильных пропусках, проверьте admissão (`/v2/sorafs/pin/*`), чтобы подтвердить manifestos, ожидающие утверждения совета.
2. **Provedores de status de verificação**
   - Abra `iroha app sorafs providers list` e instale-o para obter as réplicas necessárias.
   - Verifique os medidores `torii_sorafs_capacity_*`, você pode fornecer GiB provisionado e PoR suspeito.
3. **Replicação de replicação**
   - Выпустите новые заказы через `sorafs_manifest_stub capacity replication-order`, когда запас backlog (`stat="avg"`) опустится ниже 5 эпох (manifesto de transferência/CAR usado `iroha app sorafs toolkit pack`).
   - Уведомите governança, если aliases не имеют активных manifestos vinculativos (неожиданное падение `torii_sorafs_registry_aliases_total`).
4. **Resolução de compra**
   - Insira os registros na operação do diário SoraFS com carimbo de data e hora e resumo dos manifestos.
   - Abra este runbook, ou adicione novas regras ou painéis.

##Planeje de развертывания

Verifique o processo de configuração do cache de alias ou use a política de alias cache no produto:1. **Configuração de configuração**
   - Selecione `torii.sorafs_alias_cache` em `iroha_config` (usuário -> real) com TTL padrão e gráficos padrão: `positive_ttl`, `refresh_window`, `hard_expiry`, `negative_ttl`, `revocation_ttl`, `rotation_max_age`, `successor_grace`, `governance_grace`. Verifique a política de segurança em `docs/source/sorafs_alias_policy.md`.
   - Para SDKs распространите те же значения через их конфигурационные слои (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` em ligações Rust / NAPI / Python), clientes de aplicação de segurança совпадал com gateway.
2. **Teste de teste**
   - Defina configurações de configuração no cluster de teste, permitindo que a topologia do produto seja executada.
   - Запустите `cargo xtask sorafs-pin-fixtures`, чтобы убедиться, что канонические alias fixtures по-прежнему декодируются и проходят ida e volta; Não há necessidade de derivar a montante, mas isso não é possível.
   - Прогоните endpoints `/v2/sorafs/pin/{digest}` e `/v2/sorafs/aliases` com prova de синтетическими, покрывающими случаи fresco, janela de atualização, expirado e expirado. Verifique códigos HTTP, cabeçalhos (`Sora-Proof-Status`, `Retry-After`, `Warning`) e um JSON que contém este runbook.
3. **Confira o produto**
   - Configure uma nova configuração com uma configuração padrão. Selecione Torii para configurar gateways/serviços SDK para novas políticas de login você.
   - Importe `docs/source/grafana_sorafs_pin_registry.json` para Grafana (ou atualize painéis de controle) e abra o painel para atualizar o cache de alias no trabalho prosтранстве NOC.
4. **Provерка после развертывания**
   - Monitore `torii_sorafs_alias_cache_refresh_total` e `torii_sorafs_alias_cache_age_seconds` por 30 minutos. As imagens no arquivo `error`/`expired` são atualizadas com atualização automática; Não há necessidade de fornecer aliases de prova e provedores de serviços para produzir.
   - Убедитесь, что клиентские логи показывают те же решения политики (SDKs будут выдавать ошибки, когда prova устарел ou истек). Abra o cliente pré-definido para obter uma nova configuração.
5. **Substituição**
   - Если выпуск alias отстает и окно update часто срабатывает, временно ослабьте политику, увеличив `refresh_window` и `positive_ttl` na configuração, você pode usar a implementação principal. Оставьте `hard_expiry` неизменным, чтобы действительно устаревшие prova продолжали отклоняться.
   - Вернитесь к предыдущей конфигурации, восстановив прежний snapshot `iroha_config`, если телеметрия продолжает показывать повышенные значения `error`, затем откройте инцидент для расследования задержек генерации alias.

## Materiais úteis

- `docs/source/sorafs/pin_registry_plan.md` — permite a realização do cartão e a atualização do contato.
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` — opera o trabalhador de armazenamento, adiciona este registro do playbook.
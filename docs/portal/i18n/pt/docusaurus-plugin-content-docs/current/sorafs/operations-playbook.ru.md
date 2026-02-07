---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/operations-playbook.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: manual de operações
título: Operação do cabo SoraFS
sidebar_label: Operação de operação
description: Руководства по реагированию на инциденты и процедуры хаос-дриллов para o operador SoraFS.
---

:::nota História Canônica
Esta página está aberta, localizada em `docs/source/sorafs_ops_playbook.md`. Tire uma cópia da sincronização, mas a documentação do Sphinx não foi migrada.
:::

## Ключевые ссылки

- Ativos ativados: use os dados Grafana em `dashboards/grafana/` e forneça alertas Prometheus em `dashboards/alerts/`.
- Métrica de catálogo: `docs/source/sorafs_observability_plan.md`.
- Operador de telefonia padrão: `docs/source/sorafs_orchestrator_plan.md`.

## Матрица эскалации

| Prioridade | Exemplos de gatilhos | Atendimento de plantão | Reservar | Nomeação |
|-----------|-------------------|-----------------|--------|------------|
| P1 | Глобальная остановка gateway, уровень отказов PoR > 5% (15 minutos), backlog репликации удваивается каждые 10 minutos | Armazenamento SRE | Observabilidade TL | Dê uma olhada na governança, exceto em 30 minutos. |
| P2 | Ao configurar o SLO regional para o gateway de latência, você tenta novamente o operador sem a validação do SLA | Observabilidade TL | Armazenamento SRE | Implemente o lançamento, não crie novos manifestos. |
| P3 | Alertas de Некритичные алерты (manifestos de obsolescência, емкость 80–90%) | Triagem de admissão | Guilda de operações | Исправить в следующий рабочий день. |

## Gateway de abertura / distribuição de distribuição

**Exibição**

- Alertas: `SoraFSGatewayAvailabilityDrop`, `SoraFSGatewayLatencySlo`.
- Número: `dashboards/grafana/sorafs_gateway_overview.json`.

**Destino indevido**

1. Verifique a taxa de solicitação (ou seja, seu provedor ou seu provedor) na taxa de solicitação do painel.
2. Selecione Torii para um provedor (de vários provedores), executando `sorafs_gateway_route_weights` em ops-configurador (`docs/source/sorafs_gateway_self_cert.md`).
3. Se você abrir seu provedor, ative o fallback “busca direta” para CLI/SDK do cliente (`docs/source/sorafs_node_client_protocol.md`).

**Triagem**

- Prover o uso do token de fluxo `sorafs_gateway_stream_token_limit`.
- Configure o gateway de log no TLS ou no sistema de admissão.
- Abra `scripts/telemetry/run_schema_diff.sh`, чтобы убедиться, что экспортируемая gateway схема соответствует ожидаемой версии.

**Remédios de segurança**

- Перезапускайте только затронутый gateway de processamento; избегайте перезапуска всего кластера, если не падают несколько провайдеров.
- Atualmente, o limite de token de fluxo é de 10 a 15%, exceto o permitido.
- Повторно выполните self-cert (`scripts/sorafs_gateway_self_cert.sh`) após a estabilização.

**Pós-incidente**

- Defina o modelo P1 com o modelo `docs/source/sorafs/postmortem_template.md`.
- Запланируйте последующий хаос-дрилл, если ремедиация опиралась на ручные действия.

## Prova de Всплеск отказов (PoR / PoTR)

**Exibição**

- Alertas: `SoraFSProofFailureSpike`, `SoraFSPoTRDeadlineMiss`.
- Número: `dashboards/grafana/sorafs_proof_integrity.json`.
- Telemetria: `torii_sorafs_proof_stream_events_total` e `sorafs.fetch.error` com `provider_reason=corrupt_proof`.

**Destino indevido**

1. Selecione novos manifestos, пометив реестр manifestos (`docs/source/sorafs/manifest_pipeline.md`).
2. Governança de controle de estímulos para provadores de sucesso.

**Triagem**- Проверьте глубину очереди Desafio PoR относительно `sorafs_node_replication_backlog_total`.
- Validar a prova de prova do pipeline (`crates/sorafs_node/src/potr.rs`) para a configuração mais adequada.
- Сравните версии firmware провайдеров с реестром операторов.

**Remédios de segurança**

- Verifique os replays PoR do `sorafs_cli proof stream` com o manifesto definido.
- Se as provas forem estáveis, a prova de atividade ativa irá garantir a restauração da governança e принудительное обновление placares оркестратора.

**Pós-incidente**

- Abra o cenário do PoR para o produto final.
- Verifique todos os itens no post anterior e obtenha os certificados de verificação da lista de verificação.

## Fazer replicação / backlog de backup

**Exibição**

- Alertas: `SoraFSReplicationBacklogGrowing`, `SoraFSCapacityPressure`. Importar
  `dashboards/alerts/sorafs_capacity_rules.yml` e выполните
  `promtool test rules dashboards/alerts/tests/sorafs_capacity_rules.test.yml`
  Para a promoção, o Alertmanager está disponível na documentação.
- Número: `dashboards/grafana/sorafs_capacity_health.json`.
- Métricas: `sorafs_node_replication_backlog_total`, `sorafs_node_manifest_refresh_age_seconds`.

**Destino indevido**

1. Organize o backlog do servidor (o provedor ou seu flotador) e verifique as novas replicações.
2. Se o backlog for local, você precisará testar novas replicações do agendador.

**Triagem**

- Verifique o operador de telemetria em novas tentativas, pois isso pode gerar backlog.
- Verifique se há espaço livre disponível (`sorafs_node_capacity_utilisation_percent`).
- Проверьте последние изменения конфигурации (perfil de bloco обновления, provas de cadência).

**Remédios de segurança**

- Verifique `sorafs_cli` com a opção `--rebalance` para transferir o conteúdo.
- Масштабируйте replication workers горизонтально для затронутого провайдера.
- Inicialize a atualização do manifesto para usar o okon TTL.

**Pós-incidente**

- Broca de capacidade de expansão, фокусированный на сбоях из-за насыщения провайдеров.
- Abra as cópias de SLA da documentação em `docs/source/sorafs_node_client_protocol.md`.

## Período de tempo

- **Ежеквартально**: совмещенная симуляция остановки gateway + retry storm оркестратора.
- **Sua razão no mundo**: a informação que você tem sobre PoR/PoTR na sua prova é sua.
- **Ежемесячная spot-проверка**: cenário de replicação de manifestos de preparação.
- Exibir exercícios no log do runbook (`ops/drill-log.md`) é o seguinte:

  ```bash
  scripts/telemetry/log_sorafs_drill.sh \
    --scenario "Gateway outage chaos drill" \
    --status pass \
    --ic "Alex Morgan" \
    --scribe "Priya Patel" \
    --notes "Failover to west cluster succeeded" \
    --log ops/drill-log.md \
    --link "docs/source/sorafs/postmortem_template.md"
  ```

- Verifique o log antes da comunicação com o seguinte:

  ```bash
  scripts/telemetry/validate_drill_log.sh
  ```

- Use `--status scheduled` para brocas, `pass`/`fail` para brocas e `follow-up`, exceto o destino aberto.
- Verifique a configuração `--log` para funcionamento a seco ou teste automático; Nenhum script foi produzido para `ops/drill-log.md`.

## Шаблон постмортема

Use `docs/source/sorafs/postmortem_template.md` para o caso P1/P2 e para a detecção de raios-X. Шаблон покрывает таймлайн, количественную оценку влияния, факторы, корректирующие действия и задачи последующей валидации.
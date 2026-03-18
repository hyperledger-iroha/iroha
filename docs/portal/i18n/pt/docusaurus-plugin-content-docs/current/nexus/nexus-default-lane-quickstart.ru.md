---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-default-lane-quickstart
título: Быстрый старт faixa padrão (NX-5)
sidebar_label: Быстрый старт faixa padrão
description: Instale e forneça a pista padrão de fallback em Nexus, use Torii e SDK para colocar lane_id em pistas públicas.
---

:::nota História Canônica
Esta página contém `docs/source/quickstart/default_lane.md`. Selecione uma cópia sincronizada, caso o programa local não seja exibido no portal.
:::

# Быстрый старт pista padrão (NX-5)

> **Roteiro de contato:** NX-5 - via pública padrão de integração. O padrão de backup é o fallback `nexus.routing_policy.default_lane`, os fornecedores REST/gRPC Torii e o SDK que você pode usar опускать `lane_id`, когда трафик относится к канонической via pública. Isso significa que o operador precisa do seu catálogo, verifique o substituto em `/status` e verifique o cliente de начала до конца.

## Предварительные требования

- Capa Sora/Nexus para `irohad` (запуск `irohad --sora --config ...`).
- Para abrir a configuração do repositório, selecione a seção de seleção `nexus.*`.
- `iroha_cli`, instalado no vidro principal.
- `curl`/`jq` (exibido) para processar a carga útil `/status` em Torii.

## 1. Descrição do catálogo e espaço de dados

Definindo pistas e espaços de dados, você pode encontrá-los no site. O fragmento novo (usado em `defaults/nexus/config.toml`) registra uma via pública e usa o alias para o espaço de dados:

```toml
[nexus]
lane_count = 3

[[nexus.lane_catalog]]
index = 0
alias = "core"
description = "Primary execution lane"
dataspace = "universal"

[[nexus.lane_catalog]]
index = 1
alias = "governance"
description = "Governance & parliament traffic"
dataspace = "governance"

[[nexus.lane_catalog]]
index = 2
alias = "zk"
description = "Zero-knowledge attachments"
dataspace = "zk"

[[nexus.dataspace_catalog]]
alias = "universal"
id = 0
description = "Single-lane data space"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "governance"
id = 1
description = "Governance proposals & manifests"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "zk"
id = 2
description = "Zero-knowledge proofs and attachments"
fault_tolerance = 1
```

O cabo `index` é único e não confiável. Espaços de dados de identificação - este 64-битные значения; Em primeiro lugar, você usará a faixa de segurança, esta e a faixa de índice, para terminar.

## 2. Verifique os padrões de comercialização e operação opcional

A seção `nexus.routing_policy` ativa a pista de fallback e permite a implementação da distribuição para a instrução de conexão ou contas profissionais. Se não houver nenhum programa disponível, o agendador irá realizar a transação em `default_lane` e `default_dataspace`. O roteador lógico é instalado em `crates/iroha_core/src/queue/router.rs` e é configurado para usar a política Torii REST/gRPC.

```toml
[nexus.routing_policy]
default_lane = 0                # use the "core" lane when no rules match
default_dataspace = "universal"    # reuse the public dataspace for the fallback

[[nexus.routing_policy.rules]]
lane = 1
dataspace = "governance"
[nexus.routing_policy.rules.matcher]
instruction = "governance"
description = "Route governance instructions to the governance lane"

[[nexus.routing_policy.rules]]
lane = 2
dataspace = "zk"
[nexus.routing_policy.rules.matcher]
instruction = "smartcontract::deploy"
description = "Route contract deployments to the zk lane for proof tracking"
```


## 3. Запустить ноду с примененной политикой

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

Não faça login na política de marketing antes do início. Любые ошибки валидации (отсутствующие индексы, дублирующиеся alias, некорректные ids dataspaces) всплывают до начала fofoca.

## 4. Подтвердить состояние governança para lane

Além disso, como não estamos on-line, use o auxiliar CLI, instale-o, use a faixa padrão para abrir (manifestar a mensagem) e execute-o. com tráfego. Você pode ver o seguinte caminho na pista:

```bash
iroha_cli app nexus lane-report --summary
```

Exemplo de saída:

```
Lane  Alias            Module           Status  Quorum  Validators  Detail
   0  core             parliament       ready      05           07  manifest ok
   1  governance       parliament       ready      05           07  manifest ok
   2  zk               parliament       sealed     03           05  manifest required
```

Se a pista padrão for `sealed`, coloque o runbook de governança para as pistas antes do tema, para liberar o tráfego de tráfego. A bandeira `--fail-on-sealed` foi colocada para CI.

## 5. Carga útil de status de verificação Torii

Ответ `/status` раскрывает e политику маршрутизации, e снимок agendador por pistas. Use `curl`/`jq`, para que você possa usar a configuração correta para умолчанию e provérbio, что fallback lane публикует телеметрию:

```bash
curl -s http://127.0.0.1:8080/status | jq '.nexus.routing_policy'
```

Exemplo de saída:

```json
{
  "default_lane": 0,
  "default_dataspace": "universal",
  "rules": [
    {"lane": 1, "dataspace": "governance", "matcher": {"instruction": "governance"}},
    {"lane": 2, "dataspace": "zk", "matcher": {"instruction": "smartcontract::deploy"}}
  ]
}
```

Aqui estão as configurações do agendador para a pista `0`:

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

Isso é possível, esse instantâneo do TEU, metadaнные alias e bandeiras manifesto соответствуют конфигурации. Essa carga útil é usada no painel Grafana para o painel lane-ingest.

## 6. Verifique a configuração do cliente

- **Rust/CLI.** `iroha_cli` e клиентский crate Rust опускают поле `lane_id`, когда вы не передаете `--lane-id` / `LaneSelector`. O roteador de fila está no mesmo lugar em `default_lane`. Use a bandeira `--lane-id`/`--dataspace-id` para trabalhar sem faixa padrão.
- **JS/Swift/Android.** As configurações do SDK disponíveis são `laneId`/`lane_id` opcionais e alternativas, verificado em `/status`. Держите политику маршрутизации синхронизированной между encenação e produção, чтобы мобильным приложениям не требовались аварийные перенастройки.
- **Testes de pipeline/SSE.** Os filtros de transação são enviados para `tx_lane_id == <u32>` (como `docs/source/pipeline.md`). Подпишитесь на `/v1/pipeline/events/transactions` com um filtro, чтобы доказать, что записи, отправленные без явного lane, приходят под fallback lane id.

## 7. Observabilidade e ganchos de governança- `/status` também é publicado `nexus_lane_governance_sealed_total` e `nexus_lane_governance_sealed_aliases`, o Alertmanager pode ser usado para gerar o manifesto da pista. Deixe esses alertas ativados hoje no devnet.
- O cartão do agendador de telemetria e governança do painel para pistas (`dashboards/grafana/nexus_lanes.json`) contém um alias/slug do catálogo. Você está experimentando alias, experimentando e gerenciando o diretor Kura, seu auditor детерминированные пути (отслеживается no NX-1).
- Парламентские одобрения для default lanes должны включать план rollback. Organize o manifesto hash e forneça o plano de governança com o início rápido do seu runbook de operação, mas as rotas não serão aceitas требуемое состояние.

Para este projeto de teste, você pode usar `nexus.routing_policy.default_lane` para configurar SDK e instalar отключать унаследованные single-lane кодовые пути в сети.
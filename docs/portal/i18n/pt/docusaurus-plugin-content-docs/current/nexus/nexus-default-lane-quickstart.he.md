---
lang: he
direction: rtl
source: docs/portal/i18n/pt/docusaurus-plugin-content-docs/current/nexus/nexus-default-lane-quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7b9dc9e03cf99c85562522065d6f78526306a1c197066e142792912eec1217fd
source_last_modified: "2026-01-22T15:38:30+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
id: nexus-default-lane-quickstart
title: Guia rapida do lane padrao (NX-5)
sidebar_label: Guia rapida do lane padrao
description: Configure e verifique o fallback do lane padrao do Nexus para que Torii e SDKs possam omitir lane_id em lanes publicas.
---

:::note Fonte canonica
Esta pagina espelha `docs/source/quickstart/default_lane.md`. Mantenha ambas as copias alinhadas ate que a revisao de localizacao chegue ao portal.
:::

# Guia rapida do lane padrao (NX-5)

> **Contexto do roadmap:** NX-5 - integracao do lane publico padrao. O runtime agora expoe um fallback `nexus.routing_policy.default_lane` para que endpoints REST/gRPC do Torii e cada SDK possam omitir com seguranca um `lane_id` quando o trafego pertence ao lane publico canonico. Este guia leva operadores a configurar o catalogo, verificar o fallback em `/status` e exercitar o comportamento do cliente de ponta a ponta.

## Prerequisitos

- Um build Sora/Nexus de `irohad` (execute `irohad --sora --config ...`).
- Acesso ao repositorio de configuracao para editar secoes `nexus.*`.
- `iroha_cli` configurado para falar com o cluster alvo.
- `curl`/`jq` (ou equivalente) para inspecionar o payload `/status` do Torii.

## 1. Descreva o catalogo de lanes e dataspaces

Declare os lanes e dataspaces que devem existir na rede. O trecho abaixo (recortado de `defaults/nexus/config.toml`) registra tres lanes publicas mais os alias de dataspace correspondentes:

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

Cada `index` deve ser unico e contiguo. Os ids de dataspace sao valores de 64 bits; os exemplos acima usam os mesmos valores numericos que os indices de lane para maior clareza.

## 2. Defina os padroes de roteamento e as sobreposicoes opcionais

A secao `nexus.routing_policy` controla o lane de fallback e permite sobrescrever o roteamento para instrucoes especificas ou prefixos de conta. Se nenhuma regra corresponder, o scheduler roteia a transacao para `default_lane` e `default_dataspace` configurados. A logica do router vive em `crates/iroha_core/src/queue/router.rs` e aplica a politica de forma transparente as superficies REST/gRPC do Torii.

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

Quando voce adicionar novas lanes no futuro, atualize primeiro o catalogo e depois estenda as regras de roteamento. O lane de fallback deve continuar apontando para o lane publico que concentra a maior parte do trafego de usuarios para que SDKs alternativos permanecam compativeis.

## 3. Inicie um node com a politica aplicada

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

O node registra a politica de roteamento derivada durante o startup. Quaisquer erros de validacao (indices ausentes, alias duplicados, ids de dataspace invalidos) aparecem antes de o gossip iniciar.

## 4. Confirme o estado de governanca do lane

Assim que o node estiver online, use o helper do CLI para verificar se o lane padrao esta selado (manifest carregado) e pronto para trafego. A visao de resumo imprime uma linha por lane:

```bash
iroha_cli app nexus lane-report --summary
```

Example output:

```
Lane  Alias            Module           Status  Quorum  Validators  Detail
   0  core             parliament       ready      05           07  manifest ok
   1  governance       parliament       ready      05           07  manifest ok
   2  zk               parliament       sealed     03           05  manifest required
```

Se o lane padrao mostrar `sealed`, siga o runbook de governanca de lanes antes de permitir trafego externo. A flag `--fail-on-sealed` e util para CI.

## 5. Inspecione os payloads de status do Torii

A resposta `/status` expoe tanto a politica de roteamento quanto o snapshot do scheduler por lane. Use `curl`/`jq` para confirmar os padroes configurados e checar se o lane de fallback esta produzindo telemetria:

```bash
curl -s http://127.0.0.1:8080/status | jq '.nexus.routing_policy'
```

Sample output:

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

Para inspecionar os contadores vivos do scheduler para o lane `0`:

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

Isso confirma que o snapshot de TEU, os metadados de alias e os flags de manifest alinham com a configuracao. O mesmo payload e usado pelos paineis do Grafana para o dashboard de lane-ingest.

## 6. Exercite os padroes do cliente

- **Rust/CLI.** `iroha_cli` e o crate cliente Rust omitem o campo `lane_id` quando voce nao passa `--lane-id` / `LaneSelector`. O router de filas, portanto, cai em `default_lane`. Use flags explicitas `--lane-id`/`--dataspace-id` apenas ao mirar um lane nao padrao.
- **JS/Swift/Android.** As ultimas releases de SDK tratam `laneId`/`lane_id` como opcionais e fazem fallback para o valor anunciado por `/status`. Mantenha a politica de roteamento sincronizada entre staging e producao para que apps moveis nao precisem de reconfiguracoes de emergencia.
- **Pipeline/SSE tests.** Os filtros de eventos de transacao aceitam predicados `tx_lane_id == <u32>` (veja `docs/source/pipeline.md`). Assine `/v2/pipeline/events/transactions` com esse filtro para provar que escritas enviadas sem lane explicito chegam sob o id de lane de fallback.

## 7. Observabilidade e ganchos de governanca

- `/status` tambem publica `nexus_lane_governance_sealed_total` e `nexus_lane_governance_sealed_aliases` para que o Alertmanager avise quando um lane perder seu manifest. Mantenha esses alertas habilitados mesmo em devnets.
- O mapa de telemetria do scheduler e o dashboard de governanca de lanes (`dashboards/grafana/nexus_lanes.json`) esperam os campos alias/slug do catalogo. Se voce renomear um alias, reetiquete os diretorios Kura correspondentes para que auditores mantenham caminhos deterministas (rastreado sob NX-1).
- Aprovacoes parlamentares para lanes padrao devem incluir um plano de rollback. Registre o hash do manifest e a evidencia de governanca junto com este quickstart no seu runbook de operador para que futuras rotacoes nao adivinhem o estado requerido.

Depois que essas verificacoes passarem, voce pode tratar `nexus.routing_policy.default_lane` como a fonte de verdade para a configuracao dos SDKs e comecar a desabilitar os caminhos de codigo alternativos de lane unico na rede.

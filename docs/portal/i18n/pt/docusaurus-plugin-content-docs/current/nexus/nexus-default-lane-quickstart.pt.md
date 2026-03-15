---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-default-lane-quickstart
título: Guia rápida do lane padrão (NX-5)
sidebar_label: Guia rápido do lane padrão
description: Configure e verifique o fallback do padrão de pista do Nexus para que Torii e SDKs possam omitir lane_id em pistas públicas.
---

:::nota Fonte canônica
Esta página espelha `docs/source/quickstart/default_lane.md`. Mantenha ambas as cópias homologadas até que a revisão de localização chegue ao portal.
:::

#Guia rápida do lane padrão (NX-5)

> **Contexto do roadmap:** NX-5 - integração da pista pública padrão. O runtime agora expõe um fallback `nexus.routing_policy.default_lane` para que endpoints REST/gRPC do Torii e cada SDK pode omitir com segurança um `lane_id` quando o tráfego pertence à via pública canônica. Este guia leva os operadores a configurar o catálogo, verificar o fallback em `/status` e treinar o comportamento do cliente de ponta a ponta.

## Pré-requisitos

- Um build Sora/Nexus de `irohad` (executar `irohad --sora --config ...`).
- Acesso ao repositório de configuração para editar seções `nexus.*`.
- `iroha_cli` configurado para falar com o cluster alvo.
- `curl`/`jq` (ou equivalente) para funcionar o payload `/status` do Torii.

## 1. Descreva o catálogo de pistas e espaços de dados

Declare as pistas e espaços de dados que devem existir na rede. O trecho abaixo (recortado de `defaults/nexus/config.toml`) registra três pistas publicadas mais os alias de dataspace correspondentes:

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

Cada `index` deve ser único e contíguo. Os ids de espaço de dados são valores de 64 bits; Os exemplos acima usam os mesmos valores numéricos que os índices de pista para maior esclarecer.

## 2. Defina os padrões de roteamento e as sobreposições específicas

A seção `nexus.routing_policy` controla a via de fallback e permite sobrescrever ou roteamento para instruções específicas ou prefixos de conta. Se nenhuma regra for responsável, o agendador roteia a transação para `default_lane` e `default_dataspace` configurados. A lógica do roteador vive em `crates/iroha_core/src/queue/router.rs` e aplica a política de forma transparente como superfícies REST/gRPC do Torii.

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

Quando você adicionar novas pistas no futuro, atualize primeiro o catálogo e depois estenda as regras de roteamento. A via de fallback deve continuar apontando para a via pública que concentra a maior parte do tráfego de usuários para que SDKs alternativos permaneçam compatíveis.

## 3. Iniciar um nó com a política aplicada

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

O nó registra a política de roteamento derivada durante a inicialização. Quaisquer erros de validação (índices ausentes, alias duplicados, ids de espaço de dados inválidos) aparecem antes de o gossip iniciar.

## 4. Confirme o estado de governança da pista

Assim que o nó estiver online, use o helper do CLI para verificar se o padrão da pista está selado (manifesto carregado) e pronto para trafego. A visão do resumo imprime uma linha por pista:

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

Se o padrão de pista mostrar `sealed`, siga o runbook de governança de pistas antes de permitir o tráfego externo. Um sinalizador `--fail-on-sealed` e utilitário para CI.

## 5. Inspeção de status de cargas úteis do Torii

A resposta `/status` expõe tanto a política de roteamento quanto o snapshot do agendador por lane. Use `curl`/`jq` para confirmar os padrões configurados e verificar se a via de fallback está produzindo telemetria:

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

Para operar os contadores vivos do agendador para a pista `0`:

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

Isso confirma que o snapshot do TEU, os metadados de alias e os flags de manifesto estão alinhados com a configuração. O mesmo payload é usado pelos painéis do Grafana para o painel de controle da pista.

## 6. Exercício dos padrões do cliente- **Rust/CLI.** `iroha_cli` e o crate cliente Rust omitem o campo `lane_id` quando você não passa `--lane-id` / `LaneSelector`. O roteador de filas, portanto, cai em `default_lane`. Use flags explícitas `--lane-id`/`--dataspace-id` apenas ao mirar um lane não padrão.
- **JS/Swift/Android.** As últimas versões do SDK tratam `laneId`/`lane_id` como demonstração e fazem fallback para o valor anunciado por `/status`. Mantenha uma política de roteamento sincronizado entre encenação e produção para que os aplicativos se movam e não precisem de reconfigurações de emergência.
- **Testes de pipeline/SSE.** Os filtros de eventos de transação aceitaram os predicados `tx_lane_id == <u32>` (veja `docs/source/pipeline.md`). Assine `/v1/pipeline/events/transactions` com filtro esse para provar que escritas enviadas sem pista explícita chegam sob o id de pista de fallback.

## 7. Observabilidade e ganchos de governança

- `/status` também publica `nexus_lane_governance_sealed_total` e `nexus_lane_governance_sealed_aliases` para que o Alertmanager avise quando uma pista perder seu manifesto. Mantenha esses alertas habilitados mesmo em devnets.
- O mapa de telemetria do agendador e o dashboard de governança de pistas (`dashboards/grafana/nexus_lanes.json`) esperam os campos alias/slug do catalogo. Se você renomear um alias, reetiquete os diretórios Kura correspondentes para que os auditores mantenham caminhos deterministas (rastreados sob NX-1).
- Aprovações parlamentares para pistas padrão deverão incluir um plano de reversão. Registre o hash do manifesto e a evidência de governança junto com este quickstart no seu runbook de operador para que futuras rotações não adivinhem o estado requerido.

Depois que essas verificações passarem, você pode tratar `nexus.routing_policy.default_lane` como fonte de verdade para a configuração dos SDKs e como desabilitar os caminhos de código alternativos de via única na rede.
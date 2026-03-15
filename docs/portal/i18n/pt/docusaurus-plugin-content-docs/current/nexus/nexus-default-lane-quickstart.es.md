---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-default-lane-quickstart
título: Guia rápido da pista predeterminado (NX-5)
sidebar_label: Guia rápido da pista predeterminada
description: Configure e verifique o fallback da pista predeterminada de Nexus para que Torii e o SDK possam omitir lane_id nas pistas públicas.
---

:::nota Fonte canônica
Esta página reflete `docs/source/quickstart/default_lane.md`. Mantenha ambas as cópias alinhadas até que o barrido de localização chegue ao portal.
:::

# Guia rápido da pista predeterminada (NX-5)

> **Contexto do roteiro:** NX-5 - integração da pista pública predeterminada. O tempo de execução agora expõe um fallback `nexus.routing_policy.default_lane` para que os endpoints REST/gRPC de Torii e cada SDK possam omitir com segurança um `lane_id` quando o tráfego pertence à via pública canônica. Este guia levará os operadores a configurar o catálogo, verificar o fallback em `/status` e executar o comportamento do cliente de extremo a extremo.

## Pré-requisitos

- Uma compilação de Sora/Nexus de `irohad` (executada `irohad --sora --config ...`).
- Acesso ao repositório de configuração para poder editar seções `nexus.*`.
- `iroha_cli` configurado para falar com o objetivo do cluster.
- `curl`/`jq` (ou equivalente) para inspecionar a carga útil `/status` de Torii.

## 1. Descreva o catálogo de pistas e espaços de dados

Declare as pistas e os espaços de dados que devem existir na rede. O fragmento seguinte (recortado de `defaults/nexus/config.toml`) registra três pistas públicas, mas o alias do espaço de dados correspondente:

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

Cada `index` deve ser único e contíguo. Os ids do espaço de dados são valores de 64 bits; os exemplos anteriores usam os mesmos valores numéricos que os índices de pista para maior clareza.

## 2. Configure os valores predeterminados de inscrição e as sobreescrituras opcionais

A seção `nexus.routing_policy` controla a via de fallback e permite escrever o roteiro para instruções específicas ou preferências de conta. Se nenhuma regra coincidir, o agendador inicia a transação nos `default_lane` e `default_dataspace` configurados. A lógica do roteador vive em `crates/iroha_core/src/queue/router.rs` e aplica a política de forma transparente às superfícies REST/gRPC de Torii.

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

Quando mais adelante concorda com novas faixas, atualize primeiro o catálogo e depois estenda as regras de enrutamiento. A via de fallback deve seguir apontando para a via pública que concentra a maior parte do tráfego de usuários para que os SDKs herdados continuem funcionando.

## 3. Arrancar um nó com a política aplicada

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

O nodo registra a política de recrutamento desenvolvida durante a inicialização. Qualquer erro de validação (índices faltantes, alias duplicados, ids de espaço de dados inválidos) é mostrado antes de começar a fofoca.

## 4. Confirme o estado de governança da pista

Uma vez que o nó esteja em linha, use o auxiliar da CLI para verificar se a pista predeterminada está vendida (carga manifestada) e lista para tráfego. La vista de resumen imprime una fila por lane:

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

Se a pista predeterminada mostra `sealed`, siga o runbook de governança de pistas antes de permitir o tráfego externo. O sinalizador `--fail-on-sealed` é útil para CI.

## 5. Inspecione as cargas úteis do estado de Torii

A resposta `/status` expõe tanto a política de enrutamiento quanto a instantânea do agendador por pista. Use `curl`/`jq` para confirmar os valores predeterminados configurados e comprovar que a via de fallback está produzindo telemetria:

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

Para inspecionar os contadores ao vivo do agendador para a pista `0`:

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

Isso confirma que o instante do TEU, os metadados do alias e os sinalizadores do manifesto estão alinhados com a configuração. A mesma carga útil é aquela que usa os painéis de Grafana para o painel de lane-ingest.

## 6. Ejercita os valores predeterminados do cliente- **Rust/CLI.** `iroha_cli` e o cliente da caixa de Rust omitem o campo `lane_id` quando não há passagem `--lane-id` / `LaneSelector`. O roteador de cola por tanto recorre a `default_lane`. Use as bandeiras explícitas `--lane-id`/`--dataspace-id` somente quando aponte para uma pista não predeterminada.
- **JS/Swift/Android.** As últimas versões do SDK tratam `laneId`/`lane_id` como opcionais e fazem fallback ao valor anunciado por `/status`. Mantenha a política de enrutamiento sincronizada entre a preparação e a produção para que os aplicativos móveis não precisem de reconfigurações de emergência.
- **Testes de pipeline/SSE.** Os filtros de eventos de transações aceitam os predicados `tx_lane_id == <u32>` (versão `docs/source/pipeline.md`). Inscreva-se em `/v1/pipeline/events/transactions` com este filtro para demonstrar que as escrituras enviadas sem uma via explícita são negadas abaixo do ID da via de fallback.

## 7. Observabilidade e ganchos de governança

- `/status` também publica `nexus_lane_governance_sealed_total` e `nexus_lane_governance_sealed_aliases` para que o Alertmanager possa avisar quando uma pista perfura seu manifesto. Mantenha esses alertas habilitados mesmo em dispositivos móveis.
- O mapa de telemetria do agendador e o painel de controle de rotas (`dashboards/grafana/nexus_lanes.json`) aguardam os campos alias/slug do catálogo. Se você renomear um alias, volte a etiquetar os diretórios Kura correspondentes para que os auditores mantenham rotas deterministas (seguido abaixo do NX-1).
- As aprovações parlamentares para vias predeterminadas deverão incluir um plano de reversão. Registre o hash do manifesto e a evidência de governança junto com este início rápido em seu runbook do operador para que as rotações futuras não tenham que adivinhar o estado necessário.

Uma vez que essas comprovações podem ser tratadas `nexus.routing_policy.default_lane` como a fonte de verdade para a configuração do SDK e iniciar a desativação das rotas de código herdadas da faixa única na rede.
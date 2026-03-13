---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-default-lane-quickstart
título: Guia rápido da pista por padrão (NX-5)
sidebar_label: Guia rápido da pista por padrão
description: O configurador e verificador do fallback da pista padrão Nexus depois de Torii e o SDK podem ser colocados lane_id nas pistas públicas.
---

:::nota Fonte canônica
Esta página representa `docs/source/quickstart/default_lane.md`. Gardez as duas cópias alinhadas até que a balayage de localização chegue ao portal.
:::

# Guia rápido da pista por padrão (NX-5)

> **Roteiro contextual:** NX-5 - integração da via pública por padrão. O tempo de execução expõe um substituto `nexus.routing_policy.default_lane` para os endpoints REST/gRPC de Torii e cada SDK pode ser usado de forma totalmente segura para um `lane_id` quando o tráfego ocorre na via pública canônica. Este guia acompanha os operadores para configurar o catálogo, verificar o substituto em `/status` e testar o comportamento do cliente de luta em luta.

## Pré-requisito

- Un build Sora/Nexus de `irohad` (lancer `irohad --sora --config ...`).
- Acesse o depósito de configuração para poder modificar as seções `nexus.*`.
- `iroha_cli` configure para falar no cluster cible.
- `curl`/`jq` (ou equivalente) para inspecionar a carga útil `/status` de Torii.

## 1. Descreva o catálogo de pistas e espaços de dados

Declare as faixas e os espaços de dados que devem existir na reserva. O extrato ci-dessous (tire de `defaults/nexus/config.toml`) registra três pistas públicas, assim como o alias do espaço de dados correspondente:

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

Cada `index` é único e contíguo. Os IDs do espaço de dados têm valores de 64 bits; Os exemplos ci-dessus usam os memes com valores numéricos que o índice de lane para mais de clareza.

## 2. Defina os valores padrão de rota e as opções de sobretaxas

A seção `nexus.routing_policy` controla a via de fallback e permite sobrecarregar a rota para instruções específicas ou prefixos de conta. Se alguma regra não corresponder, o agendador rota a transação versão `default_lane` e `default_dataspace` configura. A lógica do roteador está em `crates/iroha_core/src/queue/router.rs` e aplica a política de maneira transparente nas superfícies REST/gRPC de Torii.

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

Quando você adicionar mais tarde nas novas pistas, coloque-o no início do catálogo e depois siga as regras de rota. A pista de fallback deve continuar um ponteiro para a pista pública que porta a maior parte do tráfego do usuário para que o SDK continue funcionando.

## 3. Demarrer uma noé com a política aplicada

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

Le noeud jornalizou a política de rota derivada da demarrage. Todos os erros de validação (índices manquants, alias duplicados, ids de espaço de dados inválidos) foram remontados antes da estreia da fofoca.

## 4. Confirmador do estado de governo da pista

Uma vez que a novata esteja online, use a CLI auxiliar para verificar se a pista padrão está selecionada (taxa manifestada) e preveja o tráfego. A recapitulação da vista mostra uma linha por pista:

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

Se a faixa padrão for exibida `sealed`, siga o guia de governança das faixas antes de autorizar o tráfego externo. A bandeira `--fail-on-sealed` é útil para o CI.

## 5. Inspecione as cargas úteis do status Torii

A resposta `/status` expõe a política de roteamento assim como o instante do agendador por via. Use `curl`/`jq` para confirmar os valores das configurações padrão e verificar se a via de fallback do produto de telemetria:

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

Para inspecionar os computadores ao vivo do agendador para a pista `0`:

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

Isso confirma que o instante TEU, os metadonnees de alias e os indicadores de manifesto estão alinhados com a configuração. A carga útil do meme é utilizada pelos painéis Grafana para o painel de ingestão de faixas.

## 6. Testar os valores padrão dos clientes- **Rust/CLI.** `iroha_cli` e o cliente de caixa Rust omite o campeão `lane_id` quando você não passa por `--lane-id` / `LaneSelector`. O roteador da fila retorna ao `default_lane`. Use os sinalizadores explícitos `--lane-id`/`--dataspace-id` somente quando você abrir uma faixa não padrão.
- **JS/Swift/Android.** As versões anteriores do SDK apresentam `laneId`/`lane_id` como opções e retornam ao valor anunciado por `/status`. Garanta a política de rota sincronizada entre a preparação e a produção para que os aplicativos móveis não precisem de reconfigurações de emergência.
- **Testes de pipeline/SSE.** Os filtros de eventos de transações aceitam os predicados `tx_lane_id == <u32>` (veja `docs/source/pipeline.md`). Conecte-se a `/v2/pipeline/events/transactions` com este filtro para provar que as escrituras enviadas sem a via chegaram explicitamente ao ID da via de fallback.

## 7. Observabilidade e pontos de avanço de governo

- `/status` publica também `nexus_lane_governance_sealed_total` e `nexus_lane_governance_sealed_aliases` para que o Alertmanager possa avertir lorsqu'une lane perd son manifest. Gardez esses alertas ativam meme nos devnets.
- O cartão de telemetria do agendador e o painel de controle das pistas (`dashboards/grafana/nexus_lanes.json`) atendem aos campeões alias/slug do catálogo. Se você renomear um alias, reetiquete os repertórios correspondentes do Kura para que os auditores conservem os caminhos deterministas (suivi sob NX-1).
- As aprovações parlamentares para as faixas padrão devem incluir um plano de reversão. Registre o hash do manifesto e as recomendações de governança na parte deste início rápido em seu operador de runbook para que as rotações futuras não sejam definidas pelo estado necessário.

Depois que essas verificações forem encerradas, você poderá trair `nexus.routing_policy.default_lane` como a fonte de verdade para a configuração do SDK e iniciar a desativação dos caminhos de código mono-lane herites na rede.
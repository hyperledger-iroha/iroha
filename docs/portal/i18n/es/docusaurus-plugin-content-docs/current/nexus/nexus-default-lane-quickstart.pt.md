---
lang: es
direction: ltr
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-carril-predeterminado-inicio rápido
título: Guía rápida del carril padrao (NX-5)
sidebar_label: Guía rápida del carril padrao
Descripción: Configure y verifique el respaldo del carril padrao de Nexus para que Torii y los SDK puedan omitir lane_id en carriles públicos.
---

:::nota Fuente canónica
Esta página espelha `docs/source/quickstart/default_lane.md`. Mantenha ambas as copias alinhadas ate que a revisao de localizacao chegue ao portal.
:::

# Guía rápida del carril padrao (NX-5)

> **Contexto de la hoja de ruta:** NX-5 - integración del carril público padrao. El tiempo de ejecución ahora expone un respaldo `nexus.routing_policy.default_lane` para los puntos finales REST/gRPC de Torii y cada SDK puede omitir con seguridad un `lane_id` cuando el tráfico pertenece al carril público canónico. Esta guía leva a los operadores a configurar el catálogo, verificar el respaldo en `/status` y ejercitar el comportamiento del cliente de ponta a ponta.

##Requisitos previos

- Estoy compilando Sora/Nexus de `irohad` (ejecutar `irohad --sora --config ...`).
- Acceda al repositorio de configuración para editar secos `nexus.*`.
- `iroha_cli` configurado para fallar con el clúster alvo.
- `curl`/`jq` (o equivalente) para inspeccionar la carga útil `/status` a Torii.

## 1. Descripción del catálogo de carriles y espacios de datosDeclare los carriles y espacios de datos que deben existir en la red. O trecho abaixo (recortado de `defaults/nexus/config.toml`) registra tres carriles públicos más los alias de dataspace correspondientes:

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

Cada `index` debe ser único y contiguo. Los ID del espacio de datos son valores de 64 bits; os exemplos acima usam os mesmos valores numericos que os indices de lane para maior clareza.

## 2. Defina los padroes de roteamento e as sobreposicoes opcionais

Un secoo `nexus.routing_policy` controla el carril de respaldo y permite sobrescrever o roteamento para instrucciones específicas o prefijos de contacto. Se nenhuma regra correspondiente, o planificador roteia a transacao para `default_lane` e `default_dataspace` configurados. La lógica del enrutador vive en `crates/iroha_core/src/queue/router.rs` y aplica una política de forma transparente como superficies REST/gRPC en Torii.

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

Quando voce adicionar novas lanes no futuro, atualize primeiro o catalogo e depois estenda as regras de roteamento. El carril de respaldo debe continuar activando el carril público que concentra la mayor parte del tráfico de usuarios para que los SDK alternativos sean compatibles de forma permanente.

## 3. Inicia un nodo con una política aplicada

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

El nodo registra una política de rotación derivada durante el inicio. Quaisquer errores de validación (índices ausentes, alias duplicados, ids de dataspace invalidos) aparecen antes de iniciar el chisme.## 4. Confirme el estado de gobierno del carril

Cuando el nodo esté en línea, utilice el ayudante de CLI para verificar si el carril padrao está sellado (manifiesto carregado) y pronto para trafego. El visado de currículum imprime una línea por carril:

```bash
iroha_cli app nexus lane-report --summary
```

Salida de ejemplo:

```
Lane  Alias            Module           Status  Quorum  Validators  Detail
   0  core             parliament       ready      05           07  manifest ok
   1  governance       parliament       ready      05           07  manifest ok
   2  zk               parliament       sealed     03           05  manifest required
```

Si el carril padrao muestra `sealed`, siga el runbook de gobierno de carriles antes de permitir el tráfico externo. Una bandera `--fail-on-sealed` y útil para CI.

## 5. Inspección del estado de las cargas útiles en Torii

La respuesta `/status` expone tanto la política de rotación como la instantánea del programador por carril. Utilice `curl`/`jq` para confirmar los padroes configurados y comprobar si el carril de respaldo está produciendo telemetría:

```bash
curl -s http://127.0.0.1:8080/status | jq '.nexus.routing_policy'
```

Salida de muestra:

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

Para inspeccionar los contadores vivos del planificador para el carril `0`:

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

Esto también confirma que la instantánea de TEU, los metadados de alias y las banderas de manifiesto alinham con una configuración. La misma carga útil y los pelos usados ​​que aparecen en Grafana para el panel de control de carril.

## 6. Ejercicio de los padroes del cliente- **Rust/CLI.** `iroha_cli` y el cliente de caja Rust elimina el campo `lane_id` cuando no pasa `--lane-id` / `LaneSelector`. O router de filas, portanto, cai em `default_lane`. Utilice banderas explícitas como `--lane-id`/`--dataspace-id` apenas para mirar un carril nao padrao.
- **JS/Swift/Android.** Las últimas versiones del SDK tratan `laneId`/`lane_id` como opciones y opciones de respaldo para el valor anunciado por `/status`. Mantenha a politica de roteamento sincronizada entre staging e producao para que apps moveis nao precisionm de reconfiguracoes de emergencia.
- **Pruebas de tubería/SSE.** Los filtros de eventos de transacao aceitam predicados `tx_lane_id == <u32>` (veja `docs/source/pipeline.md`). Assine `/v1/pipeline/events/transactions` com esse filtro para provar que escritas enviadas sem lane explícito chegam sollozo o id de lane de fallback.

## 7. Observabilidade e ganchos de gobierno- `/status` también publica `nexus_lane_governance_sealed_total` e `nexus_lane_governance_sealed_aliases` para que Alertmanager avise cuando un carril pierda su manifiesto. Mantenha esses alertas habilitadas mesmo em devnets.
- El mapa de telemetría del planificador y el panel de gobierno de carriles (`dashboards/grafana/nexus_lanes.json`) esperanm os campos alias/slug do catalogo. Se voce renomear um alias, reetiquete os diretorios Kura corresponsales para que auditores mantenham caminhos deterministas (rastreado sollozo NX-1).
- Aprovacoes parlamentares para lanes padrao devem incluir um plano de rollback. Registre o hash do manifest e a evidencia de gobierno junto con este inicio rápido en su runbook de operador para que futuras rotaciones nao adivinhem o estado requerido.

Después de que estas verificacoes passarem, voce pode tratar `nexus.routing_policy.default_lane` como una fuente de verdad para configurar dos SDK y comenzar a desactivar los caminos de código alternativos de carril único en la red.
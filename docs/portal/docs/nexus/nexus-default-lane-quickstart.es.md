---
lang: es
direction: ltr
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 569f46d041fd052b32f250bc513d5dd50ee1e38eaff24d4875c075f47209d06c
source_last_modified: "2025-11-10T19:43:33.013605+00:00"
translation_last_reviewed: 2025-12-30
---

---
id: nexus-default-lane-quickstart
title: Guia rapida del lane predeterminado (NX-5)
sidebar_label: Guia rapida del lane predeterminado
description: Configura y verifica el fallback del lane predeterminado de Nexus para que Torii y los SDK puedan omitir lane_id en lanes publicas.
---

:::note Fuente canonica
Esta pagina refleja `docs/source/quickstart/default_lane.md`. Manten ambas copias alineadas hasta que el barrido de localizacion llegue al portal.
:::

# Guia rapida del lane predeterminado (NX-5)

> **Contexto del roadmap:** NX-5 - integracion del lane publico predeterminado. El runtime ahora expone un fallback `nexus.routing_policy.default_lane` para que los endpoints REST/gRPC de Torii y cada SDK puedan omitir con seguridad un `lane_id` cuando el trafico pertenece al lane publico canonico. Esta guia lleva a los operadores a configurar el catalogo, verificar el fallback en `/status` y ejercitar el comportamiento del cliente de extremo a extremo.

## Prerrequisitos

- Un build de Sora/Nexus de `irohad` (ejecuta `irohad --sora --config ...`).
- Acceso al repositorio de configuracion para poder editar secciones `nexus.*`.
- `iroha_cli` configurado para hablar con el cluster objetivo.
- `curl`/`jq` (o equivalente) para inspeccionar el payload `/status` de Torii.

## 1. Describe el catalogo de lanes y dataspaces

Declara los lanes y dataspaces que deben existir en la red. El fragmento siguiente (recortado de `defaults/nexus/config.toml`) registra tres lanes publicas mas los alias de dataspace correspondientes:

```toml
[nexus]
lane_count = 3

[[nexus.lane_catalog]]
index = 0
alias = "core"
description = "Primary execution lane"

[[nexus.lane_catalog]]
index = 1
alias = "governance"
description = "Governance & parliament traffic"

[[nexus.lane_catalog]]
index = 2
alias = "zk"
description = "Zero-knowledge attachments"

[[nexus.dataspace_catalog]]
alias = "global"
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

Cada `index` debe ser unico y contiguo. Los ids de dataspace son valores de 64 bits; los ejemplos anteriores usan los mismos valores numericos que los indices de lane para mayor claridad.

## 2. Configura los valores predeterminados de enrutamiento y las sobreescrituras opcionales

La seccion `nexus.routing_policy` controla el lane de fallback y te permite sobrescribir el enrutamiento para instrucciones especificas o prefijos de cuenta. Si ninguna regla coincide, el scheduler enruta la transaccion al `default_lane` y `default_dataspace` configurados. La logica del router vive en `crates/iroha_core/src/queue/router.rs` y aplica la politica de forma transparente a las superficies REST/gRPC de Torii.

```toml
[nexus.routing_policy]
default_lane = 0                # use the "core" lane when no rules match
default_dataspace = "global"    # reuse the public dataspace for the fallback

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

Cuando mas adelante agregues nuevas lanes, actualiza primero el catalogo y luego extiende las reglas de enrutamiento. El lane de fallback debe seguir apuntando al lane publico que concentra la mayor parte del trafico de usuarios para que los SDKs heredados sigan funcionando.

## 3. Arranca un nodo con la politica aplicada

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

El nodo registra la politica de enrutamiento derivada durante el arranque. Cualquier error de validacion (indices faltantes, alias duplicados, ids de dataspace invalidos) se muestra antes de que comience el gossip.

## 4. Confirma el estado de gobernanza del lane

Una vez que el nodo este en linea, usa el helper del CLI para verificar que el lane predeterminado este sellado (manifest cargado) y listo para trafico. La vista de resumen imprime una fila por lane:

```bash
iroha_cli nexus lane-report --summary
```

Example output:

```
Lane  Alias            Module           Status  Quorum  Validators  Detail
   0  core             parliament       ready      05           07  manifest ok
   1  governance       parliament       ready      05           07  manifest ok
   2  zk               parliament       sealed     03           05  manifest required
```

Si el lane predeterminado muestra `sealed`, sigue el runbook de gobernanza de lanes antes de permitir trafico externo. El flag `--fail-on-sealed` es util para CI.

## 5. Inspecciona los payloads de estado de Torii

La respuesta `/status` expone tanto la politica de enrutamiento como la instantanea del scheduler por lane. Usa `curl`/`jq` para confirmar los valores predeterminados configurados y comprobar que el lane de fallback esta produciendo telemetria:

```bash
curl -s http://127.0.0.1:8080/status | jq '.nexus.routing_policy'
```

Sample output:

```json
{
  "default_lane": 0,
  "default_dataspace": "global",
  "rules": [
    {"lane": 1, "dataspace": "governance", "matcher": {"instruction": "governance"}},
    {"lane": 2, "dataspace": "zk", "matcher": {"instruction": "smartcontract::deploy"}}
  ]
}
```

Para inspeccionar los contadores en vivo del scheduler para el lane `0`:

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

Esto confirma que la instantanea de TEU, los metadatos de alias y los flags de manifest se alinean con la configuracion. El mismo payload es el que usan los paneles de Grafana para el dashboard de lane-ingest.

## 6. Ejercita los valores predeterminados del cliente

- **Rust/CLI.** `iroha_cli` y el crate cliente de Rust omiten el campo `lane_id` cuando no pasas `--lane-id` / `LaneSelector`. El router de colas por lo tanto recurre a `default_lane`. Usa los flags explicitos `--lane-id`/`--dataspace-id` solo cuando apuntes a una lane no predeterminada.
- **JS/Swift/Android.** Las ultimas versiones de los SDK tratan `laneId`/`lane_id` como opcionales y hacen fallback al valor anunciado por `/status`. Manten la politica de enrutamiento sincronizada entre staging y produccion para que las apps moviles no necesiten reconfiguraciones de emergencia.
- **Pipeline/SSE tests.** Los filtros de eventos de transacciones aceptan predicados `tx_lane_id == <u32>` (ver `docs/source/pipeline.md`). Suscribete a `/v1/pipeline/events/transactions` con ese filtro para demostrar que las escrituras enviadas sin un lane explicito llegan bajo el id de lane de fallback.

## 7. Observabilidad y ganchos de gobernanza

- `/status` tambien publica `nexus_lane_governance_sealed_total` y `nexus_lane_governance_sealed_aliases` para que Alertmanager pueda avisar cuando una lane pierde su manifest. Manten esas alertas habilitadas incluso en devnets.
- El mapa de telemetria del scheduler y el dashboard de gobernanza de lanes (`dashboards/grafana/nexus_lanes.json`) esperan los campos alias/slug del catalogo. Si renombras un alias, vuelve a etiquetar los directorios Kura correspondientes para que los auditores mantengan rutas deterministas (seguido bajo NX-1).
- Las aprobaciones parlamentarias para lanes predeterminadas deben incluir un plan de rollback. Registra el hash del manifest y la evidencia de gobernanza junto con este quickstart en tu runbook de operador para que las rotaciones futuras no tengan que adivinar el estado requerido.

Una vez que estas comprobaciones pasen puedes tratar `nexus.routing_policy.default_lane` como la fuente de verdad para la configuracion de los SDK y empezar a deshabilitar las rutas de codigo heredadas de lane unico en la red.

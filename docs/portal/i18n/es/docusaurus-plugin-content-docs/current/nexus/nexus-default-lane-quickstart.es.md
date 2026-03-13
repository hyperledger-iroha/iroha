---
lang: es
direction: ltr
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-carril-predeterminado-inicio rápido
título: Guía rápida del carril predeterminado (NX-5)
sidebar_label: Guía rápida del carril predeterminado
descripción: Configure y verifique el respaldo del carril predeterminado de Nexus para que Torii y los SDK puedan omitir lane_id en carriles públicos.
---

:::nota Fuente canónica
Esta página refleja `docs/source/quickstart/default_lane.md`. Mantenga ambas copias alineadas hasta que el barrido de localización llegue al portal.
:::

# Guía rápida del carril predeterminado (NX-5)

> **Contexto del roadmap:** NX-5 - integración del carril público predeterminado. El runtime ahora exponen un fallback `nexus.routing_policy.default_lane` para que los endpoints REST/gRPC de Torii y cada SDK puedan omitir con seguridad un `lane_id` cuando el tráfico pertenece al carril público canónico. Esta guía lleva a los operadores a configurar el catálogo, verificar el respaldo en `/status` y ejercitar el comportamiento del cliente de extremo a extremo.

##Prerrequisitos

- Un build de Sora/Nexus de `irohad` (ejecuta `irohad --sora --config ...`).
- Acceso al repositorio de configuración para poder editar secciones `nexus.*`.
- `iroha_cli` configurado para hablar con el cluster objetivo.
- `curl`/`jq` (o equivalente) para inspeccionar la carga útil `/status` de Torii.

## 1. Describe el catálogo de carriles y espacios de datos.Declara los carriles y espacios de datos que deben existir en la red. El fragmento siguiente (recortado de `defaults/nexus/config.toml`) registra tres carriles públicos más los alias de dataspace correspondientes:

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

Cada `index` debe ser único y contiguo. Los ids de dataspace son valores de 64 bits; Los ejemplos anteriores usan los mismos valores numéricos que los índices de carril para mayor claridad.

## 2. Configura los valores predeterminados de enrutamiento y las sobreescrituras opcionales

La sección `nexus.routing_policy` controla el carril de respaldo y te permite sobrescribir el enrutamiento para instrucciones especificas o prefijos de cuenta. Si ninguna regla coincide, el planificador enruta la transacción al `default_lane` y `default_dataspace` configurados. La lógica del enrutador vive en `crates/iroha_core/src/queue/router.rs` y aplica la política de forma transparente a las superficies REST/gRPC de Torii.

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

Cuando más adelante agregues nuevos carriles, actualiza primero el catálogo y luego extiende las reglas de enrutamiento. El carril de respaldo debe seguir apuntando al carril público que concentra la mayor parte del tráfico de usuarios para que los SDK heredados sigan funcionando.

## 3. Arranca un nodo con la política aplicada

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```El nodo registra la política de enrutamiento derivada durante el arranque. Cualquier error de validación (índices faltantes, alias duplicados, ids de dataspace invalidos) se muestra antes de que comience el chisme.

## 4. Confirma el estado de gobernanza del carril

Una vez que el nodo este en linea, use el helper del CLI para verificar que el carril predeterminado este sellado (manifiesto cargado) y listo para trafico. La vista de resumen imprime una fila por carril:

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

Si el carril predeterminado muestra `sealed`, sigue el runbook de gobernanza de carriles antes de permitir el tráfico externo. El flag `--fail-on-sealed` es útil para CI.

## 5. Inspecciona los payloads de estado de Torii

La respuesta `/status` exponen tanto la política de enrutamiento como la instantánea del planificador por carril. Utilice `curl`/`jq` para confirmar los valores predeterminados configurados y comprobar que el carril de respaldo está produciendo telemetría:

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

Para inspeccionar los contadores en vivo del planificador para el carril `0`:

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

Esto confirma que la instantánea de TEU, los metadatos de alias y los flags de manifest se alinean con la configuración. El mismo payload es el que usa los paneles de Grafana para el tablero de lane-ingest.## 6. Ejercita los valores predeterminados del cliente

- **Rust/CLI.** `iroha_cli` y el crate cliente de Rust omiten el campo `lane_id` cuando no pasas `--lane-id` / `LaneSelector`. El enrutador de colas por lo tanto recurre a `default_lane`. Usa los flags explícitos `--lane-id`/`--dataspace-id` solo cuando apuntes a una lane no predeterminada.
- **JS/Swift/Android.** Las últimas versiones de los SDK tratan `laneId`/`lane_id` como opcionales y hacen fallback al valor anunciado por `/status`. Manten la política de enrutamiento sincronizada entre puesta en escena y producción para que las apps móviles no necesiten reconfiguraciones de emergencia.
- **Pruebas de tubería/SSE.** Los filtros de eventos de transacciones aceptan predicados `tx_lane_id == <u32>` (ver `docs/source/pipeline.md`). Suscríbase a `/v2/pipeline/events/transactions` con ese filtro para demostrar que las escrituras enviadas sin un carril explícito llegan bajo el id de carril de fallback.

## 7. Observabilidad y ganchos de gobernanza- `/status` también publica `nexus_lane_governance_sealed_total` y `nexus_lane_governance_sealed_aliases` para que Alertmanager pueda avisar cuando un carril pierde su manifiesto. Manten esas alertas habilitadas incluso en devnets.
- El mapa de telemetria del planificador y el tablero de gobernanza de carriles (`dashboards/grafana/nexus_lanes.json`) esperan los campos alias/slug del catalogo. Si renombras un alias, vuelve a etiquetar los directorios Kura correspondientes para que los auditores mantengan rutas deterministas (seguido bajo NX-1).
- Las aprobaciones parlamentarias para carriles predeterminados deben incluir un plan de rollback. Registre el hash del manifest y la evidencia de gobernanza junto con este inicio rápido en su runbook de operador para que las rotaciones futuras no tengan que adivinar el estado requerido.

Una vez que estas comprobaciones pasen puedes tratar `nexus.routing_policy.default_lane` como la fuente de verdad para la configuración de los SDK y empezar a deshabilitar las rutas de código heredadas de lane unico en la red.
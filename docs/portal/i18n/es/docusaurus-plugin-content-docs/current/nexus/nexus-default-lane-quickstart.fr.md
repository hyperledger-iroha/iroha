---
lang: es
direction: ltr
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-carril-predeterminado-inicio rápido
título: Guía rápida de la carrilera por defecto (NX-5)
sidebar_label: Guía rápida del carril por defecto
descripción: Configure y verifique el respaldo del carril por defecto de Nexus después de Torii y el SDK pueda omitir lane_id en los carriles públicos.
---

:::nota Fuente canónica
Esta página informa `docs/source/quickstart/default_lane.md`. Gardez les deux copys alineados justo ce que le balayage de localización llegan sur le portail.
:::

# Guía rápida de la vía por defecto (NX-5)

> **Hoja de ruta contextual:** NX-5: integración de la vía pública por defecto. El tiempo de ejecución expone un respaldo `nexus.routing_policy.default_lane` después de los puntos finales REST/gRPC de Torii y cada SDK puede configurar de forma segura un `lane_id` cuando el tráfico aparece en el carril público canónico. Esta guía acompaña a los operadores para configurar el catálogo, verificar el respaldo en `/status` y probar el comportamiento del cliente de cada momento.

## Requisitos previos

- Un build Sora/Nexus de `irohad` (lancer `irohad --sora --config ...`).
- Acceda al depósito de configuración para poder modificar las secciones `nexus.*`.
- `iroha_cli` configure para parler au cluster cible.
- `curl`/`jq` (o equivalente) para inspeccionar la carga útil `/status` de Torii.

## 1. Decrire le catalog des lanes et des dataspacesDeclare los carriles y espacios de datos que deben existir en el recurso. El extrait ci-dessous (tire de `defaults/nexus/config.toml`) registra tres carriles públicos además de los alias de los correspondientes del espacio de datos:

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

Chaque `index` debe ser único y contiguo. Los ID del espacio de datos tienen valores de 64 bits; les exemples ci-dessus utilisent les memes valeurs numeriques que les index de lane pour plus de clarte.

## 2. Definir los valores de ruta predeterminados y los recargos opcionales

La sección `nexus.routing_policy` controla el carril de respaldo y permite recargar la ruta para las instrucciones específicas o los prefijos de cuenta. Si no hay ninguna regla correspondiente, el programador enruta la transacción según las configuraciones `default_lane` e `default_dataspace`. La lógica del enrutador se encuentra en `crates/iroha_core/src/queue/router.rs` y aplica la política de mantenimiento transparente a las superficies REST/gRPC de Torii.

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

Lorsque vous ajoutez plus tard de nouvelles lanes, mettez d'abord a jour le catalogue, puis etendez les regles de routage. El carril de reserva debe continuar con un puntero hacia el carril público que porta la mayoría del usuario de tráfico después de que el SDK hereda el funcionamiento continuo.

## 3. Demarrer un noeud avec la politique appliquee

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```Le noeud periodise la politique de routage derivee au demarrage. Todos los errores de validación (index manquants, alias dupliques, ids de dataspace invalides) se eliminan antes del debut del chisme.

## 4. Confirmador del estado de gobierno de la calle

Una vez que haya un nudo en línea, utilice el asistente CLI para verificar que el carril por defecto está registrado (cargo manifiesto) y previo al tráfico. La vista recapitulativa muestra una línea por carril:

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

Si el carril que aparece por defecto en el cartel `sealed`, suivez le runbook de gouvernance des lanes avant d'autoriser du trafic externe. La bandera `--fail-on-sealed` es útil para el CI.

## 5. Inspeccione las cargas útiles del estado de Torii

La respuesta `/status` expone la política de ruta además de la instantánea del planificador por carril. Utilice `curl`/`jq` para confirmar los valores configurados por defecto y verificar que el carril de respaldo del producto de telemetría:

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

Para inspeccionar los ordenadores en vivo del planificador para la línea `0`:

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

Esto confirma que el TEU instantáneo, los metadones d'alias y los indicadores de manifiesto están alineados con la configuración. La carga útil del meme se utiliza en los paneles Grafana para el panel de control de carriles.

## 6. Probar los valores por defecto de los clientes- **Rust/CLI.** `iroha_cli` y el cliente de caja Rust omittent le champ `lane_id` cuando no pasas `--lane-id` / `LaneSelector`. El enrutador de cola retomará desde `default_lane`. Utilice las banderas explícitas `--lane-id`/`--dataspace-id` únicamente cuando pueda acceder a un carril no predeterminado.
- **JS/Swift/Android.** Las últimas versiones del SDK incluyen `laneId`/`lane_id` como opciones y retoman el valor anunciado por `/status`. Gardez la política de ruta sincronizada entre la puesta en escena y la producción a fin de que las aplicaciones móviles no pasen necesidad de reconfiguraciones de urgencia.
- **Pruebas de canalización/SSE.** Los filtros de eventos de transacciones aceptan los predicados `tx_lane_id == <u32>` (ver `docs/source/pipeline.md`). Abra `/v1/pipeline/events/transactions` con este filtro para comprobar que las escrituras enviadas sin carril explícito llegan al ID de carril de reserva.

## 7. Observabilidad y puntos de acceso a la gobernanza- `/status` publie aussi `nexus_lane_governance_sealed_total` et `nexus_lane_governance_sealed_aliases` afin qu'Alertmanager puisse avertir lorsqu'une lane perd son manifest. Gardez ces alertas activas meme sur les devnets.
- La tarjeta de telemetría del programador y el panel de control de carriles (`dashboards/grafana/nexus_lanes.json`) atienden los campeones alias/slug du catalogue. Si renommez un alias, reetiquetez les repertoires Kura correspondientes afin que les auditeurs conservent des chemins deterministes (suivi sous NX-1).
- Las aprobaciones parlamentarias para los carriles por defecto deben incluir un plan de reversión. Registre el hash del manifiesto y las preuves de gouvernance en una cuenta de inicio rápido en su runbook operator para que las rotaciones de futuros no sean necesarias para determinar el estado requerido.

Una vez finalizadas estas verificaciones, podrá iniciar `nexus.routing_policy.default_lane` desde la fuente de verificación para la configuración del SDK y comenzar a desactivar los enlaces de código mono-lane heredados en el archivo.
---
lang: es
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sorafs/pin-registry-ops.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0dc64bb4067d734250852a74a65a2100bd68e5ff35f9e8e9dbf3bd2b86f00cfa
source_last_modified: "2026-01-22T15:38:30.656337+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
id: pin-registry-ops-es
slug: /sorafs/pin-registry-ops-es
---

:::nota Fuente canónica
Espejos `docs/source/sorafs/runbooks/pin_registry_ops.md`. Mantenga ambas versiones alineadas en todas las versiones.
:::

## Descripción general

Este runbook documenta cómo monitorear y clasificar el registro de pines SoraFS y sus acuerdos de nivel de servicio (SLA) de replicación. Las métricas se originan en `iroha_torii` y se exportan a través de Prometheus en el espacio de nombres `torii_sorafs_*`. Torii muestra el estado del registro en un intervalo de 30 segundos en segundo plano, por lo que los paneles permanecen actualizados incluso cuando ningún operador está sondeando los puntos finales `/v1/sorafs/pin/*`. Importe el panel seleccionado (`docs/source/grafana_sorafs_pin_registry.json`) para obtener un diseño Grafana listo para usar que se asigna directamente a las secciones siguientes.

## Referencia métrica

| Métrica | Etiquetas | Descripción |
| ------ | ------ | ----------- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \| `approved` \| `retired`) | Inventario de manifiesto en cadena por estado del ciclo de vida. |
| `torii_sorafs_registry_aliases_total` | — | Recuento de alias de manifiesto activos registrados en el registro. |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \| `completed` \| `expired`) | Cartera de pedidos de replicación segmentada por estado. |
| `torii_sorafs_replication_backlog_total` | — | Medidor de conveniencia que refleja los pedidos `pending`. |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \| `missed` \| `pending`) | Contabilidad de SLA: `met` cuenta los pedidos completados dentro del plazo, `missed` agrega finalizaciones tardías + vencimientos, `pending` refleja los pedidos pendientes. |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Latencia de finalización agregada (épocas entre la emisión y la finalización). |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Ventanas de inactividad de orden pendiente (fecha límite menos época de emisión). |

Todos los indicadores se reinician en cada toma de instantánea, por lo que los paneles deben tomar muestras con una cadencia `1m` o más rápida.

## Grafana Panel de control

El panel JSON se entrega con siete paneles que cubren los flujos de trabajo del operador. Las consultas se enumeran a continuación para una referencia rápida si prefiere crear gráficos personalizados.

1. **Ciclo de vida del manifiesto** – `torii_sorafs_registry_manifests_total` (agrupado por `status`).
2. **Tendencia del catálogo de alias** – `torii_sorafs_registry_aliases_total`.
3. **Cola de pedidos por estado** – `torii_sorafs_registry_orders_total` (agrupada por `status`).
4. **Pedidos pendientes versus pedidos vencidos**: combina `torii_sorafs_replication_backlog_total` e `torii_sorafs_registry_orders_total{status="expired"}` para alcanzar la saturación superficial.
5. **Proporción de éxito de SLA** –

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```

6. **Latencia versus retraso en la fecha límite**: superposición `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` e `torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`. Utilice transformaciones Grafana para agregar vistas `min_over_time` cuando necesite el piso absolutamente flojo, por ejemplo:

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. **Pedidos perdidos (tarifa de 1h)** –

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## Umbrales de alerta- **Éxito del SLA  0**
  - Umbral: `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - Acción: Inspeccionar los manifiestos de gobernanza para confirmar la rotación de proveedores.
- **Finalización p95 > promedio de retraso en la fecha límite**
  - Umbral: `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - Acción: Verificar que los proveedores se comprometan antes de los plazos; considere emitir reasignaciones.

### Ejemplo de reglas Prometheus

```yaml
groups:
  - name: sorafs-pin-registry
    rules:
      - alert: SorafsReplicationSlaDrop
        expr: sum(torii_sorafs_replication_sla_total{outcome="met"}) /
          clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) < 0.95
        for: 15m
        labels:
          severity: page
        annotations:
          summary: "SoraFS replication SLA below target"
          description: "SLA success ratio stayed under 95% for 15 minutes."

      - alert: SorafsReplicationBacklogGrowing
        expr: torii_sorafs_replication_backlog_total > 10
        for: 10m
        labels:
          severity: page
        annotations:
          summary: "SoraFS replication backlog above threshold"
          description: "Pending replication orders exceeded the configured backlog budget."

      - alert: SorafsReplicationExpiredOrders
        expr: increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0
        for: 0m
        labels:
          severity: ticket
        annotations:
          summary: "SoraFS replication orders expired"
          description: "At least one replication order expired in the last five minutes."
```

## Flujo de trabajo de clasificación

1. **Identificar la causa**
   - Si el SLA no alcanza el pico mientras el trabajo pendiente sigue siendo bajo, céntrese en el rendimiento del proveedor (fallos de PoR, finalizaciones tardías).
   - Si el retraso crece con errores estables, inspeccione la admisión (`/v1/sorafs/pin/*`) para confirmar los manifiestos en espera de la aprobación del consejo.
2. **Validar estado de proveedor**
   - Ejecute `iroha app sorafs providers list` y verifique que las capacidades anunciadas coincidan con los requisitos de replicación.
   - Verifique los medidores `torii_sorafs_capacity_*` para confirmar el éxito de GiB y PoR aprovisionados.
3. **Reasignar replicación**
   - Emitir nuevos pedidos a través de `sorafs_manifest_stub capacity replication-order` cuando la holgura del trabajo pendiente (`stat="avg"`) cae por debajo de 5 épocas (el manifiesto/empaquetado de CAR usa `iroha app sorafs toolkit pack`).
   - Notificar a la gobernanza si los alias carecen de enlaces de manifiesto activos (`torii_sorafs_registry_aliases_total` cae inesperadamente).
4. **Resultado del documento**
   - Registre notas de incidentes en el registro de operaciones SoraFS con marcas de tiempo y resúmenes de manifiestos afectados.
   - Actualice este runbook si se introducen nuevos modos de falla o paneles.

## Plan de implementación

Siga este procedimiento por etapas al habilitar o ajustar la política de caché de alias en producción:1. **Preparar configuración**
   - Actualice `torii.sorafs_alias_cache` en `iroha_config` (usuario → real) con los TTL y ventanas de gracia acordados: `positive_ttl`, `refresh_window`, `hard_expiry`, `negative_ttl`, `revocation_ttl`, `rotation_max_age`, `successor_grace` y `governance_grace`. Los valores predeterminados coinciden con la política en `docs/source/sorafs_alias_policy.md`.
   - Para los SDK, distribuya los mismos valores a través de sus capas de configuración (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` en enlaces Rust/NAPI/Python) para que la aplicación del cliente coincida con la puerta de enlace.
2. **Ejecución en seco en la puesta en escena**
   - Implementar el cambio de configuración en un clúster provisional que refleje la topología de producción.
   - Ejecute `cargo xtask sorafs-pin-fixtures` para confirmar que los dispositivos de alias canónicos aún se decodifican y realizan el viaje de ida y vuelta; cualquier desajuste implica una deriva manifiesta hacia arriba que debe abordarse primero.
   - Ejercite los puntos finales `/v1/sorafs/pin/{digest}` e `/v1/sorafs/aliases` con pruebas sintéticas que cubran casos nuevos, de ventana de actualización, vencidos y vencidos recientemente. Valide los códigos de estado HTTP, los encabezados (`Sora-Proof-Status`, `Retry-After`, `Warning`) y los campos del cuerpo JSON en este runbook.
3. **Habilitar en producción**
   - Implementar la nueva configuración a través de la ventana de cambio estándar. Aplíquelo primero a Torii, luego reinicie las puertas de enlace/servicios SDK una vez que el nodo confirme la nueva política en los registros.
   - Importe `docs/source/grafana_sorafs_pin_registry.json` a Grafana (o actualice los paneles existentes) y fije los paneles de actualización de caché de alias al espacio de trabajo de NOC.
4. **Verificación posterior a la implementación**
   - Monitorear `torii_sorafs_alias_cache_refresh_total` e `torii_sorafs_alias_cache_age_seconds` durante 30 minutos. Los picos en las curvas `error`/`expired` deberían correlacionarse con las ventanas de actualización de políticas; Un crecimiento inesperado significa que los operadores deben inspeccionar las pruebas de alias y el estado del proveedor antes de continuar.
   - Confirmar que los registros del lado del cliente muestran las mismas decisiones de política (los SDK mostrarán errores cuando la prueba esté obsoleta o caducada). La ausencia de advertencias del cliente indica una mala configuración.
5. **Retroceso**
   - Si la emisión de alias se retrasa y la ventana de actualización se activa con frecuencia, relaje temporalmente la política aumentando `refresh_window` e `positive_ttl` en la configuración y luego vuelva a implementarla. Mantenga `hard_expiry` intacto para que las pruebas verdaderamente obsoletas aún se rechacen.
   - Vuelva a la configuración anterior restaurando la instantánea `iroha_config` anterior si la telemetría continúa mostrando recuentos elevados de `error`, luego abra un incidente para rastrear los retrasos en la generación de alias.

## Materiales relacionados

- `docs/source/sorafs/pin_registry_plan.md`: hoja de ruta de implementación y contexto de gobernanza.
- `docs/source/sorafs/runbooks/sorafs_node_ops.md`: operaciones de trabajo de almacenamiento, complementa este manual de registro.

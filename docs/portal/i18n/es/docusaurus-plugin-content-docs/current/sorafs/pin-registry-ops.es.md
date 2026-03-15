---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-ops.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pin-registry-ops
título: Operaciones del Registro Pin
sidebar_label: Operaciones del Registro de PIN
descripción: Monitorea y hace triaje del Pin Registry de SoraFS y las métricas de SLA de replicacion.
---

:::nota Fuente canónica
Refleja `docs/source/sorafs/runbooks/pin_registry_ops.md`. Mantén ambas versiones sincronizadas hasta que se retire la documentación heredada de Sphinx.
:::

## Resumen

Este runbook documenta cómo monitorear y hacer triaje del Pin Registry de SoraFS y sus acuerdos de nivel de servicio (SLA) de replicación. Las métricas provienen de `iroha_torii` y se exportan vía Prometheus bajo el espacio de nombres `torii_sorafs_*`. Torii muestrea el estado del registro en un intervalo de 30 segundos en el segundo plano, por lo que los paneles se mantienen actualizados incluso cuando ningún operador está consultando los endpoints `/v1/sorafs/pin/*`. Importa el tablero curado (`docs/source/grafana_sorafs_pin_registry.json`) para un diseño de Grafana listo para usar que mapea directamente a las siguientes secciones.

## Referencia de métricas| Métrica | Etiquetas | Descripción |
| ------ | ------ | ----------- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \| `approved` \| `retired`) | Inventario de manifiestos on-chain por estado del ciclo de vida. |
| `torii_sorafs_registry_aliases_total` | — | Conteo de alias de manifiestos activos registrados en el registro. |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \| `completed` \| `expired`) | Backlog de órdenes de replicación segmentado por estado. |
| `torii_sorafs_replication_backlog_total` | — | Calibre de conveniencia que refleja las órdenes `pending`. |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \| `missed` \| `pending`) | Contabilidad de SLA: `met` cuenta órdenes completadas dentro del plazo, `missed` agrega completadas tarde + expiraciones, `pending` refleja órdenes pendientes. |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Latencia agregada de finalización (épocas entre emisión y completada). |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Ventanas de holgura de órdenes pendientes (deadline menos época de emisión). |

Todos los medidores se reinician en cada pull de snapshot, así que los tableros deben muestrear a cadencia `1m` o más rápida.

## Panel de control de GrafanaEl JSON del tablero incluye siete paneles que cubren los flujos de trabajo de los operadores. Las consultas se listan abajo para referencia rápida si prefieres construir gráficos a medida.

1. **Ciclo de vida de manifiestos** – `torii_sorafs_registry_manifests_total` (agrupado por `status`).
2. **Tendencia del catálogo de alias** – `torii_sorafs_registry_aliases_total`.
3. **Cola de órdenes por estado** – `torii_sorafs_registry_orders_total` (agrupado por `status`).
4. **Backlog vs órdenes vencidas** – combina `torii_sorafs_replication_backlog_total` y `torii_sorafs_registry_orders_total{status="expired"}` para mostrar saturación.
5. **Ratio de éxito del SLA** –

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```

6. **Latencia vs holgura de fecha límite** – superpone `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` y `torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`. Usa transformaciones de Grafana para agregar vistas `min_over_time` cuando necesites el piso absoluto de holgura, por ejemplo:

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. **Órdenes fallidas (tasa 1h)** –

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## Umbrales de alerta- **Éxito del SLA  0**
  - Umbral: `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - Acción: Inspeccionar manifiestos de gobernanza para confirmar la rotación de proveedores.
- **p95 de completado > holgura promedio de plazo**
  - Umbral: `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - Acción: Verificar que los proveedores cumplan antes del plazo; considerar reasignaciones.

### Reglas de ejemplo Prometheus

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
          summary: "SLA de replicación de SoraFS por debajo del objetivo"
          description: "El ratio de éxito del SLA se mantuvo por debajo de 95% durante 15 minutos."

      - alert: SorafsReplicationBacklogGrowing
        expr: torii_sorafs_replication_backlog_total > 10
        for: 10m
        labels:
          severity: page
        annotations:
          summary: "Backlog de replicación de SoraFS por encima del umbral"
          description: "Las órdenes pendientes excedieron el presupuesto de backlog configurado."

      - alert: SorafsReplicationExpiredOrders
        expr: increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0
        for: 0m
        labels:
          severity: ticket
        annotations:
          summary: "Órdenes de replicación de SoraFS expiradas"
          description: "Al menos una orden de replicación expiró en los últimos cinco minutos."
```

## Flujo de triaje1. **Identificar causa**
   - Si los misses de SLA suben mientras el backlog se mantiene bajo, enfocarse en el rendimiento de los proveedores (fallas de PoR, completadas tardes).
   - Si el backlog crece con misses estables, inspeccionar la admisión (`/v1/sorafs/pin/*`) para confirmar manifests en espera de aprobación del consejo.
2. **Validar estado de proveedores**
   - Ejecuta `iroha app sorafs providers list` y verifica que las capacidades anunciadas cumplan con los requisitos de replicación.
   - Revisa los calibres `torii_sorafs_capacity_*` para confirmar GiB provisionados y éxito de PoR.
3. **Reasignar replicación**
   - Emite nuevas órdenes vía `sorafs_manifest_stub capacity replication-order` cuando la holgura del backlog (`stat="avg"`) cae por debajo de 5 épocas (el empaquetado de manifest/CAR usa `iroha app sorafs toolkit pack`).
   - Notifica a gobernanza si los alias carecen de vinculaciones de manifiestos activos (caídas inesperadas de `torii_sorafs_registry_aliases_total`).
4. **Resultado documental**
   - Registre notas del incidente en el log de operaciones de SoraFS con timestamps y digests de manifest afectados.
   - Actualiza este runbook si aparecen nuevos modos de falla o paneles.

## Plan de despliegue

Sigue este procedimiento por etapas para habilitar o soportar la política de caché de alias en producción:1. **Preparar configuración**
   - Actualiza `torii.sorafs_alias_cache` en `iroha_config` (usuario -> actual) con los TTL y ventanas de gracia acordados: `positive_ttl`, `refresh_window`, `hard_expiry`, `negative_ttl`, `revocation_ttl`, `rotation_max_age`, `successor_grace` y `governance_grace`. Los defaults coinciden con la política en `docs/source/sorafs_alias_policy.md`.
   - Para SDK, distribuye los mismos valores mediante sus capas de configuración (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` en enlaces Rust / NAPI / Python) para que la aplicación cliente coincida con el gateway.
2. **Ejecución en seco y puesta en escena**
   - Despliega el cambio de configuración en un cluster de staging que refleja la topología de producción.
   - Ejecuta `cargo xtask sorafs-pin-fixtures` para confirmar que los aparatos canónicos de alias aún decodifican y hacen round-trip; cualquier desajuste implica deriva aguas arriba que deben resolverse primero.
   - Ejercita los endpoints `/v1/sorafs/pin/{digest}` y `/v1/sorafs/aliases` con pruebas sintéticas que cubran casos fresco, refresco-ventana, caducado y duro-expirado. Valida los códigos HTTP, encabezados (`Sora-Proof-Status`, `Retry-After`, `Warning`) y campos del cuerpo JSON contra este runbook.
3. **Habilitar en producción**
   - Despliega la nueva configuración en la ventana estándar de cambios. Aplícala primero a Torii y luego reinicia gateways/servicios SDK una vez que el nodo confirme la nueva política en logs.- Importa `docs/source/grafana_sorafs_pin_registry.json` en Grafana (o actualiza los paneles existentes) y fija los paneles de actualización de caché de alias al espacio de trabajo del NOC.
4. **Verificación post-despliegue**
   - Monitorea `torii_sorafs_alias_cache_refresh_total` y `torii_sorafs_alias_cache_age_seconds` durante 30 minutos. Picos en las curvas `error`/`expired` deben correlacionarse con las ventanas de refresco; crecimiento inesperado implica que los operadores deben inspeccionar pruebas de alias y salud de los proveedores antes de continuar.
   - Confirma que los logs del lado del cliente muestran las mismas decisiones de política (los SDK mostrarán errores cuando la prueba esté obsoleta o caducada). La ausencia de advertencias del cliente indica una mala configuración.
5. **Retroceso**
   - Si la emisión de alias se retrasa y la ventana de actualización se dispara con frecuencia, relaja temporalmente la política aumentando `refresh_window` y `positive_ttl` en la configuración, y vuelve a desplegar. Mantén `hard_expiry` intacto para que las pruebas realmente obsoletas sigan rechazándose.
   - Reviértelo a la configuración previa restaurando la instantánea anterior de `iroha_config` si la telemetría sigue mostrando conteos `error` elevados, luego abre un incidente para rastrear retrasos en la generación de alias.

## Materiales relacionados- `docs/source/sorafs/pin_registry_plan.md` — hoja de ruta de implementación y contexto de gobernanza.
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` — operaciones del trabajador de almacenamiento, complementa este manual de registro.
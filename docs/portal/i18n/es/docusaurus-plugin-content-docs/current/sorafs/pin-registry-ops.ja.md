---
lang: ja
direction: ltr
source: docs/portal/i18n/es/docusaurus-plugin-content-docs/current/sorafs/pin-registry-ops.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f23e555e9317bfff0a4d7163c8c42ea029dcf98d231d21822a39f80c039ffa92
source_last_modified: "2026-01-22T15:38:30+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: pin-registry-ops
lang: es
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-ops.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Fuente canónica
Refleja `docs/source/sorafs/runbooks/pin_registry_ops.md`. Mantén ambas versiones sincronizadas hasta que se retire la documentación heredada de Sphinx.
:::

## Resumen

Este runbook documenta cómo monitorear y hacer triaje del Pin Registry de SoraFS y sus acuerdos de nivel de servicio (SLA) de replicación. Las métricas provienen de `iroha_torii` y se exportan vía Prometheus bajo el namespace `torii_sorafs_*`. Torii muestrea el estado del registry en un intervalo de 30 segundos en segundo plano, por lo que los dashboards se mantienen actualizados incluso cuando ningún operador está consultando los endpoints `/v1/sorafs/pin/*`. Importa el dashboard curado (`docs/source/grafana_sorafs_pin_registry.json`) para un layout de Grafana listo para usar que mapea directamente a las secciones siguientes.

## Referencia de métricas

| Métrica | Labels | Descripción |
| ------ | ------ | ----------- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \| `approved` \| `retired`) | Inventario de manifests on-chain por estado del ciclo de vida. |
| `torii_sorafs_registry_aliases_total` | — | Conteo de aliases de manifest activos registrados en el registry. |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \| `completed` \| `expired`) | Backlog de órdenes de replicación segmentado por estado. |
| `torii_sorafs_replication_backlog_total` | — | Gauge de conveniencia que refleja las órdenes `pending`. |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \| `missed` \| `pending`) | Contabilidad de SLA: `met` cuenta órdenes completadas dentro del deadline, `missed` agrega completados tarde + expiraciones, `pending` refleja órdenes pendientes. |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Latencia agregada de finalización (épocas entre emisión y completado). |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Ventanas de holgura de órdenes pendientes (deadline menos época de emisión). |

Todos los gauges se reinician en cada pull de snapshot, así que los dashboards deben muestrear a cadencia `1m` o más rápida.

## Dashboard de Grafana

El JSON del dashboard incluye siete paneles que cubren los flujos de trabajo de operadores. Las consultas se listan abajo para referencia rápida si prefieres construir gráficos a medida.

1. **Ciclo de vida de manifests** – `torii_sorafs_registry_manifests_total` (agrupado por `status`).
2. **Tendencia del catálogo de alias** – `torii_sorafs_registry_aliases_total`.
3. **Cola de órdenes por estado** – `torii_sorafs_registry_orders_total` (agrupado por `status`).
4. **Backlog vs órdenes expiradas** – combina `torii_sorafs_replication_backlog_total` y `torii_sorafs_registry_orders_total{status="expired"}` para mostrar saturación.
5. **Ratio de éxito del SLA** –

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```

6. **Latencia vs holgura de deadline** – superpone `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` y `torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`. Usa transformaciones de Grafana para añadir vistas `min_over_time` cuando necesites el piso absoluto de holgura, por ejemplo:

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. **Órdenes fallidas (tasa 1h)** –

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## Umbrales de alerta

- **Éxito del SLA < 0.95 por 15 min**
  - Umbral: `sum(torii_sorafs_replication_sla_total{outcome="met"}) / clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) < 0.95`
  - Acción: Paginación a SRE; iniciar triaje del backlog de replicación.
- **Backlog pendiente por encima de 10**
  - Umbral: `torii_sorafs_replication_backlog_total > 10` sostenido por 10 min
  - Acción: Revisar disponibilidad de providers y el scheduler de capacidad de Torii.
- **Órdenes expiradas > 0**
  - Umbral: `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - Acción: Inspeccionar manifests de gobernanza para confirmar churn de providers.
- **p95 de completado > holgura promedio de deadline**
  - Umbral: `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - Acción: Verificar que los providers cumplan antes del deadline; considerar reasignaciones.

### Reglas de Prometheus de ejemplo

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

## Flujo de triaje

1. **Identificar causa**
   - Si los misses de SLA suben mientras el backlog se mantiene bajo, enfocarse en el rendimiento de providers (fallas de PoR, completados tardíos).
   - Si el backlog crece con misses estables, inspeccionar la admisión (`/v1/sorafs/pin/*`) para confirmar manifests en espera de aprobación del consejo.
2. **Validar estado de providers**
   - Ejecuta `iroha app sorafs providers list` y verifica que las capacidades anunciadas cumplan los requisitos de replicación.
   - Revisa los gauges `torii_sorafs_capacity_*` para confirmar GiB provisionados y éxito de PoR.
3. **Reasignar replicación**
   - Emite nuevas órdenes vía `sorafs_manifest_stub capacity replication-order` cuando la holgura del backlog (`stat="avg"`) cae por debajo de 5 épocas (el empaquetado de manifest/CAR usa `iroha app sorafs toolkit pack`).
   - Notifica a gobernanza si los aliases carecen de bindings de manifest activos (caídas inesperadas de `torii_sorafs_registry_aliases_total`).
4. **Documentar resultado**
   - Registra notas del incidente en el log de operaciones de SoraFS con timestamps y digests de manifest afectados.
   - Actualiza este runbook si aparecen nuevos modos de falla o dashboards.

## Plan de despliegue

Sigue este procedimiento por etapas al habilitar o endurecer la política de cache de alias en producción:

1. **Preparar configuración**
   - Actualiza `torii.sorafs_alias_cache` en `iroha_config` (user -> actual) con los TTL y ventanas de gracia acordados: `positive_ttl`, `refresh_window`, `hard_expiry`, `negative_ttl`, `revocation_ttl`, `rotation_max_age`, `successor_grace` y `governance_grace`. Los defaults coinciden con la política en `docs/source/sorafs_alias_policy.md`.
   - Para SDKs, distribuye los mismos valores mediante sus capas de configuración (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` en bindings Rust / NAPI / Python) para que la aplicación cliente coincida con el gateway.
2. **Dry-run en staging**
   - Despliega el cambio de configuración en un cluster de staging que refleje la topología de producción.
   - Ejecuta `cargo xtask sorafs-pin-fixtures` para confirmar que los fixtures canónicos de alias aún decodifican y hacen round-trip; cualquier mismatch implica drift aguas arriba que debe resolverse primero.
   - Ejercita los endpoints `/v1/sorafs/pin/{digest}` y `/v1/sorafs/aliases` con pruebas sintéticas que cubran casos fresh, refresh-window, expired y hard-expired. Valida los códigos HTTP, headers (`Sora-Proof-Status`, `Retry-After`, `Warning`) y campos del cuerpo JSON contra este runbook.
3. **Habilitar en producción**
   - Despliega la nueva configuración en la ventana estándar de cambios. Aplícala primero a Torii y luego reinicia gateways/servicios SDK una vez que el nodo confirme la nueva política en logs.
   - Importa `docs/source/grafana_sorafs_pin_registry.json` en Grafana (o actualiza dashboards existentes) y fija los paneles de refresh de cache de alias al workspace del NOC.
4. **Verificación post-despliegue**
   - Monitorea `torii_sorafs_alias_cache_refresh_total` y `torii_sorafs_alias_cache_age_seconds` durante 30 minutos. Picos en las curvas `error`/`expired` deben correlacionarse con las ventanas de refresh; crecimiento inesperado implica que los operadores deben inspeccionar pruebas de alias y salud de providers antes de continuar.
   - Confirma que los logs del lado del cliente muestran las mismas decisiones de política (los SDKs mostrarán errores cuando la prueba esté stale o expirada). La ausencia de warnings del cliente indica una mala configuración.
5. **Fallback**
   - Si la emisión de alias se retrasa y la ventana de refresh se dispara con frecuencia, relaja temporalmente la política aumentando `refresh_window` y `positive_ttl` en la config, y vuelve a desplegar. Mantén `hard_expiry` intacto para que las pruebas realmente stale sigan rechazándose.
   - Reviértelo a la configuración previa restaurando el snapshot anterior de `iroha_config` si la telemetría sigue mostrando conteos `error` elevados, luego abre un incidente para rastrear retrasos en la generación de alias.

## Materiales relacionados

- `docs/source/sorafs/pin_registry_plan.md` — roadmap de implementación y contexto de gobernanza.
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` — operaciones del worker de almacenamiento, complementa este playbook de registry.

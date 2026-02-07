---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-ops.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pin-registry-ops
título: Operaciones del Registro Pin
sidebar_label: Operaciones del Registro Pin
descripción: Vigile y busque el Registro Pin SoraFS y las métricas SLA de replicación.
---

:::nota Fuente canónica
Refleje `docs/source/sorafs/runbooks/pin_registry_ops.md`. Guarde las dos versiones sincronizadas justo al retrato de la documentación heredada de Sphinx.
:::

## Vista del conjunto

Este runbook registra comentarios de vigilancia y prueba el Registro de PIN SoraFS y sus acuerdos de nivel de servicio (SLA) de réplica. Las métricas proporcionadas por `iroha_torii` y se exportan a través de Prometheus en el espacio de nombres `torii_sorafs_*`. Torii Enciende el estado del registro durante 30 segundos en el plan de llegada, mientras que los paneles de control permanecen al día incluso cuando el operador no interroga los puntos finales `/v1/sorafs/pin/*`. Importe el panel de control curado (`docs/source/grafana_sorafs_pin_registry.json`) para un diseño Grafana prêt à l'emploi que corresponde directamente a las secciones ci-dessous.

## Référence des métriques| Métrico | Etiquetas | Descripción |
| ------- | ------ | ----------- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \| `approved` \| `retired`) | Inventaire des manifests on-chain par état de Cycle de vie. |
| `torii_sorafs_registry_aliases_total` | — | Nombre de alias de manifiestos activos registrados en el registro. |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \| `completed` \| `expired`) | Backlog des ordres de replication segmenté par statut. |
| `torii_sorafs_replication_backlog_total` | — | Calibre de mercancías reflejando los pedidos `pending`. |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \| `missed` \| `pending`) | Comptabilité SLA: `met` cuenta las órdenes terminadas en retrasos, `missed` agrega las terminaciones tardías + expiraciones, `pending` reflète las órdenes en atención. |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Latence de complétion agrégée (époques entre émission et complétion). |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Fenêtres de marge des ordres en attente (fecha límite menor que la época de emisión). |

Todos los indicadores se reinicializan cada vez que se extrae una instantánea, cuando los tableros deben cambiarse con una cadencia `1m` o más rápida.

## Panel de control GrafanaEl JSON del panel contiene siete paneles que transmiten los operadores de flujos de trabajo. Les requêtes sont listées ci-dessous pour référence rapide si vous préférez construire des graphiques sur mesure.

1. **Ciclo de vida de manifiestos** – `torii_sorafs_registry_manifests_total` (grupo por `status`).
2. **Tendencia del catálogo de alias** – `torii_sorafs_registry_aliases_total`.
3. **File d'ordres par statut** – `torii_sorafs_registry_orders_total` (agrupado por `status`).
4. **Atrasos frente a pedidos vencidos**: combine `torii_sorafs_replication_backlog_total` e `torii_sorafs_registry_orders_total{status="expired"}` para evidenciar la saturación.
5. **Relación de retorno SLA** –

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```

6. **Latencia vs margen de fecha límite** – superponga `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` y `torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`. Utilice las transformaciones Grafana para agregar imágenes `min_over_time` cuando necesite la plancha absoluta de margen, por ejemplo:

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. **Ordres manqués (taux 1h)** –

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## Seuils d'alerte- **Succès SLA  0**
  - Seuil: `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - Acción: Inspeccionar los manifiestos de gobierno para confirmar la rotación de proveedores.
- **p95 de finalización > margen de fecha límite**
  - Seuil : `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - Acción: Verificar que los proveedores sean válidos antes de los plazos; envisager des réassignations.

### Ejemplos de reglas Prometheus

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
          summary: "SLA de réplication SoraFS sous la cible"
          description: "Le ratio de succès SLA est resté sous 95% pendant 15 minutes."

      - alert: SorafsReplicationBacklogGrowing
        expr: torii_sorafs_replication_backlog_total > 10
        for: 10m
        labels:
          severity: page
        annotations:
          summary: "Backlog de réplication SoraFS au-dessus du seuil"
          description: "Les ordres de réplication en attente ont dépassé le budget de backlog configuré."

      - alert: SorafsReplicationExpiredOrders
        expr: increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0
        for: 0m
        labels:
          severity: ticket
        annotations:
          summary: "Ordres de réplication SoraFS expirés"
          description: "Au moins un ordre de réplication a expiré au cours des cinq dernières minutes."
```

## Eliminación del flujo de trabajo1. **Identificador de la causa**
   - Si las pruebas de SLA aumentan cuando el trabajo pendiente sigue siendo deficiente, concentre el análisis en el rendimiento de los proveedores (pruebas PoR, terminaciones tardías).
   - Si el trabajo pendiente aumenta con los cheques de los establos, inspeccione la admisión (`/v1/sorafs/pin/*`) para confirmar los manifiestos con la ayuda de la aprobación del consejo.
2. **Validar el estado de los proveedores**
   - Exécuter `iroha app sorafs providers list` y verificador de que las capacidades anunciadas corresponden a las exigencias de réplica.
   - Verifique los medidores `torii_sorafs_capacity_*` para confirmar los GiB provisionnés y el éxito PoR.
3. **Réassigner la réplica**
   - Émettre de nouveaux ordres via `sorafs_manifest_stub capacity replication-order` lorsque la marge du backlog (`stat="avg"`) desciende en 5 épocas (l'empaquetage manifest/CAR utiliza `iroha app sorafs toolkit pack`).
   - Notificar la gobernanza si los alias no pasan de vinculaciones de actos manifiestos (baisse inattendue de `torii_sorafs_registry_aliases_total`).
4. **Documente el resultado**
   - Remitente de las notas del incidente en el diario de operaciones SoraFS con marcas de tiempo y resúmenes de manifiestos preocupados.
   - Mettre à jour este runbook si se introducen nuevos modos de control o paneles de control.

## Plan de despliegue

Suivez este procedimiento par étapes lors de l'activation ou du durcissement de la politique de cache d'alias en production:1. **Preparar la configuración**
   - Mettre à jour `torii.sorafs_alias_cache` dans `iroha_config` (usuario -> actual) con los TTL y ventanas de gracia convenus: `positive_ttl`, `refresh_window`, `hard_expiry`, `negative_ttl`, `revocation_ttl`, `rotation_max_age`, `successor_grace` y `governance_grace`. Los valores por defecto corresponden a la política de `docs/source/sorafs_alias_policy.md`.
   - Para los SDK, difunda los mismos valores a través de sus sofás de configuración (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` en los enlaces Rust / NAPI / Python) hasta que la aplicación cliente suive le gateway.
2. **Ejecución en seco y puesta en escena**
   - Implementar el cambio de configuración en un clúster de puesta en escena que refleja la topología de producción.
   - Ejecutador `cargo xtask sorafs-pin-fixtures` para confirmar que los dispositivos canónicos de alias décodent y font siempre son de ida y vuelta; Toda divergencia implica una deriva contracorriente à corregir en primer lugar.
   - Ejecute los puntos finales `/v1/sorafs/pin/{digest}` e `/v1/sorafs/aliases` con las etiquetas sintéticas que incluyen frescura, ventana de actualización, expiración y expiración permanente. Valide los códigos HTTP, los encabezados (`Sora-Proof-Status`, `Retry-After`, `Warning`) y los campos del cuerpo JSON con este runbook.
3. **Activador en producción**- Implemente la nueva configuración desde la ventana de cambio estándar. Al iniciar sesión en Torii, reemplace los gateways/services SDK una vez que no confirme la nueva política en los registros.
   - Importador `docs/source/grafana_sorafs_pin_registry.json` en Grafana (o actualizar los paneles de control existentes) y activar los paneles de actualización del caché de alias en el espacio de trabajo NOC.
4. **Verificación post-despliegue**
   - Vigilante `torii_sorafs_alias_cache_refresh_total` et `torii_sorafs_alias_cache_age_seconds` colgante 30 minutos. Las fotos en las curvas `error`/`expired` deben corregirse con las ventanas de actualización; Un croissance inattendue significa que los operadores no deben inspeccionar las preuves d'alias y la salud de los proveedores antes de continuar.
   - Confirmador de que los registros del cliente guardan las mismas decisiones políticas (los SDK recuerdan errores cuando la compra está caducada o caducada). La ausencia de anuncios en el cliente indica una configuración incorrecta.
5. **Retroceso**
   - Si la emisión de alias se retrasa y la ventana de actualización se desactiva con frecuencia, relaje temporalmente la política y aumente `refresh_window` e `positive_ttl` en la configuración, luego vuelva a implementarlo. Garder `hard_expiry` intacto para que les preuves réellement périmées soient toujours rejetées.- Regrese a la configuración anterior al restaurante la instantánea `iroha_config` anterior si la televisión continúa mostrando las cuentas `error` mayores, luego descubra un incidente para rastrear los cambios de generación de alias.

## Materiaux liés

- `docs/source/sorafs/pin_registry_plan.md`: hoja de ruta de implementación y contexto de gobierno.
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` — operaciones de trabajadores de almacenamiento, registro completo del libro de jugadas.
---
lang: es
direction: ltr
source: docs/source/da/replication_policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 70163ed6740c80c48c78ae918c37d34e0022ab97ffabce6d451bbf85060e24b4
source_last_modified: "2026-01-22T15:38:30.661849+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Política de replicación de disponibilidad de datos (DA-4)

_Estado: En progreso — Propietarios: Core Protocol WG / Equipo de almacenamiento / SRE_

El canal de ingesta de DA ahora impone objetivos de retención deterministas para
cada clase de blob descrita en `roadmap.md` (flujo de trabajo DA-4). Torii se niega a
persistir sobres de retención proporcionados por la persona que llama que no coinciden con el configurado
política, garantizando que cada validador/nodo de almacenamiento conserve la información requerida
número de épocas y réplicas sin depender de la intención del remitente.

## Política predeterminada

| Clase de burbuja | Retención en caliente | Retención de frío | Réplicas requeridas | Clase de almacenamiento | Etiqueta de gobernanza |
|------------|---------------|----------------|-------------------|----------------|----------------|
| `taikai_segment` | 24 horas | 14 días | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 horas | 7 días | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 horas | 180 días | 3 | `cold` | `da.governance` |
| _Predeterminado (todas las demás clases)_ | 6 horas | 30 días | 3 | `warm` | `da.default` |

Estos valores están integrados en `torii.da_ingest.replication_policy` y se aplican a
todas las presentaciones `/v2/da/ingest`. Torii reescribe manifiestos con el impuesto
perfil de retención y emite una advertencia cuando las personas que llaman proporcionan valores no coincidentes para
Los operadores pueden detectar SDK obsoletos.

### Clases de disponibilidad de Taikai

Los manifiestos de enrutamiento de Taikai (metadatos `taikai.trm`) ahora incluyen un
Pista `availability_class` (`Hot`, `Warm` o `Cold`). Cuando esté presente, Torii
selecciona el perfil de retención correspondiente de `torii.da_ingest.replication_policy`
antes de fragmentar la carga útil, lo que permite a los operadores de eventos bajar la categoría inactiva
representaciones sin editar la tabla de políticas globales. Los valores predeterminados son:

| Clase de disponibilidad | Retención en caliente | Retención de frío | Réplicas requeridas | Clase de almacenamiento | Etiqueta de gobernanza |
|--------------------|---------------|----------------|-------------------|----------------|----------------|
| `hot` | 24 horas | 14 días | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 horas | 30 días | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 hora | 180 días | 3 | `cold` | `da.taikai.archive` |

Si el manifiesto omite `availability_class`, la ruta de ingesta vuelve al
Perfil `hot` para que las transmisiones en vivo mantengan su conjunto de réplicas completo. Los operadores pueden
anule estos valores editando el nuevo
Bloque `torii.da_ingest.replication_policy.taikai_availability` en configuración.

## Configuración

La política vive bajo `torii.da_ingest.replication_policy` y expone un
Plantilla *predeterminada* más una serie de anulaciones por clase. Los identificadores de clase son
no distingue entre mayúsculas y minúsculas y acepta `taikai_segment`, `nexus_lane_sidecar`,
`governance_artifact` o `custom:<u16>` para extensiones aprobadas por la gobernanza.
Las clases de almacenamiento aceptan `hot`, `warm` o `cold`.

```toml
[torii.da_ingest.replication_policy.default_retention]
hot_retention_secs = 21600          # 6 h
cold_retention_secs = 2592000       # 30 d
required_replicas = 3
storage_class = "warm"
governance_tag = "da.default"

[[torii.da_ingest.replication_policy.overrides]]
class = "taikai_segment"
[torii.da_ingest.replication_policy.overrides.retention]
hot_retention_secs = 86400          # 24 h
cold_retention_secs = 1209600       # 14 d
required_replicas = 5
storage_class = "hot"
governance_tag = "da.taikai.live"
```

Deje el bloque intacto para ejecutarlo con los valores predeterminados enumerados anteriormente. Para apretar un
clase, actualice la anulación coincidente; para cambiar la línea base para nuevas clases,
editar `default_retention`.Para ajustar clases de disponibilidad específicas de Taikai, agregue entradas en
`torii.da_ingest.replication_policy.taikai_availability`:

```toml
[[torii.da_ingest.replication_policy.taikai_availability]]
availability_class = "warm"
[torii.da_ingest.replication_policy.taikai_availability.retention]
hot_retention_secs = 14400         # 4 h
cold_retention_secs = 604800       # 7 d
required_replicas = 4
storage_class = "warm"
governance_tag = "da.taikai.warm"
```

## Semántica de aplicación

- Torii reemplaza el `RetentionPolicy` proporcionado por el usuario con el perfil aplicado
  antes de la fragmentación o emisión manifiesta.
- Se rechazan los manifiestos prediseñados que declaran un perfil de retención no coincidente.
  con `400 schema mismatch` para que los clientes obsoletos no puedan debilitar el contrato.
- Se registra cada evento de anulación (`blob_class`, política enviada versus esperada)
  para detectar personas que llaman que no cumplen con las normas durante la implementación.

Consulte `docs/source/da/ingest_plan.md` (lista de verificación de validación) para ver la puerta actualizada.
que cubren la aplicación de la retención.

## Flujo de trabajo de nueva replicación (seguimiento de DA-4)

La aplicación de la retención es sólo el primer paso. Los operadores también deben demostrar que
Los manifiestos en vivo y las órdenes de replicación permanecen alineados con la política configurada para que
que SoraFS puede volver a replicar automáticamente blobs que no cumplen los requisitos.

1. **Esté atento a la deriva.** Torii emite
   `overriding DA retention policy to match configured network baseline` siempre que
   una persona que llama envía valores de retención obsoletos. Empareja ese tronco con
   Telemetría `torii_sorafs_replication_*` para detectar réplicas deficientes o retrasadas
   redistribuciones.
2. **Diferencia entre intención y réplicas en vivo.** Utilice el nuevo asistente de auditoría:

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   El comando carga `torii.da_ingest.replication_policy` desde el archivo proporcionado
   config, decodifica cada manifiesto (JSON o Norito) y, opcionalmente, coincide con cualquier
   Cargas útiles `ReplicationOrderV1` mediante resumen de manifiesto. El resumen señala dos
   condiciones:

   - `policy_mismatch`: el perfil de retención del manifiesto difiere del aplicado
     política (esto nunca debería suceder a menos que Torii esté mal configurado).
   - `replica_shortfall`: el orden de replicación en vivo solicita menos réplicas que
     `RetentionPolicy.required_replicas` o proporciona menos asignaciones que su
     objetivo.

   Un estado de salida distinto de cero indica un déficit activo, por lo que la automatización de CI/de guardia
   puede paginar inmediatamente. Adjunte el informe JSON al
   Paquete `docs/examples/da_manifest_review_template.md` para las votaciones del Parlamento.
3. **Activar nueva replicación.** Cuando la auditoría informe un déficit, emita una nueva
   `ReplicationOrderV1` a través de las herramientas de gobierno descritas en
   `docs/source/sorafs/storage_capacity_marketplace.md` y vuelva a ejecutar la auditoría
   hasta que el conjunto de réplicas converja. Para anulaciones de emergencia, empareje la salida CLI
   con `iroha app da prove-availability` para que SRE pueda hacer referencia al mismo resumen
   y evidencia del PPD.

La cobertura de regresión vive en `integration_tests/tests/da/replication_policy.rs`;
la suite envía una política de retención no coincidente a `/v2/da/ingest` y verifica
que el manifiesto recuperado expone el perfil aplicado en lugar de la persona que llama
intención.

## Paneles y telemetría de prueba de salud (puente DA-5)

El elemento de la hoja de ruta **DA-5** requiere que los resultados de cumplimiento de PDP/PoTR sean auditables en
tiempo real. Los eventos `SorafsProofHealthAlert` ahora impulsan un conjunto dedicado de
Métricas de Prometheus:

- `torii_sorafs_proof_health_alerts_total{provider_id,trigger,penalty}`
- `torii_sorafs_proof_health_pdp_failures{provider_id}`
- `torii_sorafs_proof_health_potr_breaches{provider_id}`
-`torii_sorafs_proof_health_penalty_nano{provider_id}`
- `torii_sorafs_proof_health_cooldown{provider_id}`
-`torii_sorafs_proof_health_window_end_epoch{provider_id}`

La placa **SoraFS PDP y PoTR Health** Grafana
(`dashboards/grafana/sorafs_pdp_potr_health.json`) ahora expone esas señales:- *Alertas de salud de prueba por disparador* muestra las tasas de alerta por disparador/indicador de penalización para que
  Los operadores de Taikai/CDN pueden demostrar si los ataques solo PDP, solo PoTR o duales son válidos.
  disparando.
- *Proveedores en Cooldown* informa la suma en vivo de proveedores actualmente bajo un
  Enfriamiento de SorafsProofHealthAlert.
- *Instantánea de la ventana de estado de prueba* fusiona los contadores PDP/PoTR, el monto de la penalización,
  bandera de enfriamiento y época de finalización de la ventana de huelga por proveedor para que los revisores de gobernanza
  Puede adjuntar la tabla a los paquetes de incidentes.

Los runbooks deben vincular estos paneles al presentar pruebas de cumplimiento de la DA; ellos
vincular las fallas del flujo de prueba de CLI directamente con los metadatos de penalización en cadena y
proporcionar el gancho de observabilidad mencionado en la hoja de ruta.
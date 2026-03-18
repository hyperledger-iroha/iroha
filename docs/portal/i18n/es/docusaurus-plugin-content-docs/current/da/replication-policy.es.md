---
lang: es
direction: ltr
source: docs/portal/docs/da/replication-policy.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Fuente canónica
Refleja `docs/source/da/replication_policy.md`. Mantenga ambas versiones en
:::

# Política de replicación de Disponibilidad de Datos (DA-4)

_Estado: En progreso -- Responsables: Core Protocol WG / Equipo de Almacenamiento / SRE_

El pipeline de ingesta DA ahora aplica objetivos de retención deterministas para
cada clase de blob descrita en `roadmap.md` (workstream DA-4). Torii rechaza
persistir sobres de retención provistos por el llamante que no coinciden con la
política configurada, garantizando que cada nodo validador/almacenamiento retiene
el número requerido de épocas y réplicas sin depender de la intención del emisor.

## Política por defecto

| Clase de blob | Retencion caliente | Retencion frio | Réplicas requeridas | Clase de almacenamiento | Etiqueta de gobernanza |
|--------------|---------------|----------------|---------------------|-------------------------|-------------------|
| `taikai_segment` | 24 horas | 14 días | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 horas | 7 días | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 horas | 180 días | 3 | `cold` | `da.governance` |
| _Predeterminado (todas las demás clases)_ | 6 horas | 30 días | 3 | `warm` | `da.default` |Estos valores se incrustan en `torii.da_ingest.replication_policy` y se aplican a
todas las solicitudes `/v1/da/ingest`. Torii reescribe manifests con el perfil de
retencion impuesto y emite una advertencia cuando los llamantes entregan valores
No hay coincidencias para que los operadores detecten SDKs desactualizados.

### Clases de disponibilidad Taikai

Los manifests de enrutamiento Taikai (`taikai.trm`) declaran un
`availability_class` (`hot`, `warm`, o `cold`). Torii aplica la politica
correspondiente antes del fragmentación para que los operadores puedan escalar el
Conteo de réplicas por stream sin editar la tabla global. Valores predeterminados:

| Clase de disponibilidad | Retencion caliente | Retencion frio | Réplicas requeridas | Clase de almacenamiento | Etiqueta de gobernanza |
|---------------------------------|---------------|----------------|---------------------|-------------------------|-------------------|
| `hot` | 24 horas | 14 días | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 horas | 30 días | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 hora | 180 días | 3 | `cold` | `da.taikai.archive` |

Las pistas faltantes usan `hot` por defecto para que las transmisiones en vivo
retengan la política más fuerte. Sobrescribe los defaults vía
`torii.da_ingest.replication_policy.taikai_availability` si su red usa objetivos
diferentes.

## ConfiguraciónLa politica vive bajo `torii.da_ingest.replication_policy` y exponen un template
*default* mas un arreglo de overrides por clase. Los identificadores de clase no
son sensibles a mayus/minus y aceptan `taikai_segment`, `nexus_lane_sidecar`,
`governance_artifact`, o `custom:<u16>` para extensiones aprobadas por gobernanza.
Las clases de almacenamiento aceptan `hot`, `warm`, o `cold`.

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

Deje el bloque intacto para usar los listados predeterminados arriba. Para soportar
una clase, actualice el override correspondiente; para cambiar la base de nuevas
clases, edite `default_retention`.

Las clases de disponibilidad Taikai pueden suscribirse de forma independiente
vía `torii.da_ingest.replication_policy.taikai_availability`:

```toml
[[torii.da_ingest.replication_policy.taikai_availability]]
availability_class = "cold"
[torii.da_ingest.replication_policy.taikai_availability.retention]
hot_retention_secs = 3600          # 1 h
cold_retention_secs = 15552000     # 180 d
required_replicas = 3
storage_class = "cold"
governance_tag = "da.taikai.archive"
```

## Semántica de cumplimiento

- Torii reemplaza el `RetentionPolicy` provisto por el usuario con el perfil
  impuesto antes del fragmentación o la emisión de manifiestos.
- Los manifiestos preconstruidos que declaran un perfil de retención distinto se
  rechazan con `400 schema mismatch` para que los clientes obsoletos no puedan
  debilitar el contrato.
- Cada evento de override se registra (`blob_class`, politica enviada vs esperada)
  para exponer a las personas que llaman no conformes durante el lanzamiento.

Ver [Plan de ingesta de Data Availability](ingest-plan.md) (checklist de validación)
para el portón actualizado que cubre el cumplimiento de retención.

## Flujo de re-replicación (seguimiento DA-4)El cumplimiento de la retención es solo el primer paso. Los operadores tambien deben
probar que los manifiestos en vivo y las órdenes de replicación se mantienen
alineados con la política configurada para que SoraFS pueda volver a replicar blobs
fuera de cumplimiento de forma automática.

1. **Vigile el drift.** Torii emite
   `overriding DA retention policy to match configured network baseline` cuando
   un llamante envía valores de retención desactualizados. Empareje ese log con
   la telemetria `torii_sorafs_replication_*` para detectar faltantes de replica
   o redespliegue demorados.
2. **Diferencia intencion vs replicas en vivo.** Utilice el nuevo ayudante de auditoria:

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   El comando carga `torii.da_ingest.replication_policy` desde la configuración
   provista, decodifica cada manifiesto (JSON o Norito), y opcionalmente empareja
   cargas útiles `ReplicationOrderV1` por resumen de manifiesto. El resumen de la marca dos
   condiciones:

   - `policy_mismatch` - el perfil de retención del manifiesto diverge de la
     politica impuesta (esto no deberia ocurrir salvo que Torii este mal
     configurado).
   - `replica_shortfall` - la orden de replicacion en vivo solicita menos replicas
     que `RetentionPolicy.required_replicas` o entrega menos asignaciones que su
     objetivo.Un status de salida no cero indica un faltante activo para que la automatización
   CI/on-call pueda paginar de inmediato. Adjunte el informe JSON al paquete
   `docs/examples/da_manifest_review_template.md` para votos del Parlamento.
3. **Dispare re-replicacion.** Cuando la auditoria reporte un faltante, emite una
   nueva `ReplicationOrderV1` vía las herramientas de gobernanza descritas en
   [Mercado de capacidad de almacenamiento SoraFS](../sorafs/storage-capacity-marketplace.md)
   y vuelva a ejecutar la auditoria hasta que el conjunto de réplicas converja. Párrafo
   overrides de emergencia, empareje la salida de la CLI con `iroha app da prove-availability`
   para que SRE puedan referenciar el mismo resumen y evidencia PDP.

La cobertura de regresión vive en
`integration_tests/tests/da/replication_policy.rs`; la suite envia una politica de
retencion no coincidente a `/v1/da/ingest` y verifica que el manifiesto obtenido
Expone el perfil impuesto en lugar de la intención del llamante.
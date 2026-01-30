---
lang: he
direction: rtl
source: docs/portal/i18n/es/docusaurus-plugin-content-docs/current/da/replication-policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 625db2c625bf42e0998e03f8c32a77c75b2c3f61143c98c636e99e63238ac600
source_last_modified: "2025-11-14T04:43:19.750457+00:00"
translation_last_reviewed: 2026-01-30
---

:::note Fuente canonica
Refleja `docs/source/da/replication_policy.md`. Mantenga ambas versiones en
:::

# Politica de replicacion de Data Availability (DA-4)

_Estado: En progreso -- Responsables: Core Protocol WG / Storage Team / SRE_

El pipeline de ingesta DA ahora aplica objetivos de retencion deterministas para
cada clase de blob descrita en `roadmap.md` (workstream DA-4). Torii rechaza
persistir envelopes de retencion provistos por el caller que no coincidan con la
politica configurada, garantizando que cada nodo validador/almacenamiento retiene
el numero requerido de epocas y replicas sin depender de la intencion del emisor.

## Politica por defecto

| Clase de blob | Retencion hot | Retencion cold | Replicas requeridas | Clase de almacenamiento | Tag de gobernanza |
|--------------|---------------|----------------|---------------------|-------------------------|-------------------|
| `taikai_segment` | 24 horas | 14 dias | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 horas | 7 dias | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 horas | 180 dias | 3 | `cold` | `da.governance` |
| _Default (todas las demas clases)_ | 6 horas | 30 dias | 3 | `warm` | `da.default` |

Estos valores se incrustan en `torii.da_ingest.replication_policy` y se aplican a
todas las solicitudes `/v1/da/ingest`. Torii reescribe manifests con el perfil de
retencion impuesto y emite una advertencia cuando los callers entregan valores
no coincidentes para que los operadores detecten SDKs desactualizados.

### Clases de disponibilidad Taikai

Los manifests de enrutamiento Taikai (`taikai.trm`) declaran un
`availability_class` (`hot`, `warm`, o `cold`). Torii aplica la politica
correspondiente antes del chunking para que los operadores puedan escalar el
conteo de replicas por stream sin editar la tabla global. Defaults:

| Clase de disponibilidad | Retencion hot | Retencion cold | Replicas requeridas | Clase de almacenamiento | Tag de gobernanza |
|-------------------------|---------------|----------------|---------------------|-------------------------|-------------------|
| `hot` | 24 horas | 14 dias | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 horas | 30 dias | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 hora | 180 dias | 3 | `cold` | `da.taikai.archive` |

Las pistas faltantes usan `hot` por defecto para que las transmisiones en vivo
retengan la politica mas fuerte. Sobrescriba los defaults via
`torii.da_ingest.replication_policy.taikai_availability` si su red usa objetivos
diferentes.

## Configuracion

La politica vive bajo `torii.da_ingest.replication_policy` y expone un template
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

Deje el bloque intacto para usar los defaults listados arriba. Para endurecer
una clase, actualice el override correspondiente; para cambiar la base de nuevas
clases, edite `default_retention`.

Las clases de disponibilidad Taikai pueden sobrescribirse de forma independiente
via `torii.da_ingest.replication_policy.taikai_availability`:

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

## Semantica de enforcement

- Torii reemplaza el `RetentionPolicy` provisto por el usuario con el perfil
  impuesto antes del chunking o la emision de manifests.
- Los manifests preconstruidos que declaran un perfil de retencion distinto se
  rechazan con `400 schema mismatch` para que los clientes obsoletos no puedan
  debilitar el contrato.
- Cada evento de override se registra (`blob_class`, politica enviada vs esperada)
  para exponer callers no conformes durante el rollout.

Ver [Plan de ingesta de Data Availability](ingest-plan.md) (checklist de validacion)
para el gate actualizado que cubre el enforcement de retencion.

## Flujo de re-replicacion (seguimiento DA-4)

El enforcement de retencion es solo el primer paso. Los operadores tambien deben
probar que los manifests en vivo y las ordenes de replicacion se mantienen
alineados con la politica configurada para que SoraFS pueda re-replicar blobs
fuera de cumplimiento de forma automatica.

1. **Vigile el drift.** Torii emite
   `overriding DA retention policy to match configured network baseline` cuando
   un caller envia valores de retencion desactualizados. Empareje ese log con
   la telemetria `torii_sorafs_replication_*` para detectar faltantes de replica
   o redeploys demorados.
2. **Diferencie intencion vs replicas en vivo.** Use el nuevo helper de auditoria:

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   El comando carga `torii.da_ingest.replication_policy` desde la configuracion
   provista, decodifica cada manifest (JSON o Norito), y opcionalmente empareja
   payloads `ReplicationOrderV1` por digest de manifest. El resumen marca dos
   condiciones:

   - `policy_mismatch` - el perfil de retencion del manifest diverge de la
     politica impuesta (esto no deberia ocurrir salvo que Torii este mal
     configurado).
   - `replica_shortfall` - la orden de replicacion en vivo solicita menos replicas
     que `RetentionPolicy.required_replicas` o entrega menos asignaciones que su
     objetivo.

   Un status de salida no cero indica un faltante activo para que la automatizacion
   CI/on-call pueda paginar de inmediato. Adjunte el reporte JSON al paquete
   `docs/examples/da_manifest_review_template.md` para votos del Parlamento.
3. **Dispare re-replicacion.** Cuando la auditoria reporte un faltante, emita una
   nueva `ReplicationOrderV1` via las herramientas de gobernanza descritas en
   [SoraFS storage capacity marketplace](../sorafs/storage-capacity-marketplace.md)
   y vuelva a ejecutar la auditoria hasta que el set de replicas converja. Para
   overrides de emergencia, empareje la salida de la CLI con `iroha app da prove-availability`
   para que SREs puedan referenciar el mismo digest y evidencia PDP.

La cobertura de regresion vive en
`integration_tests/tests/da/replication_policy.rs`; la suite envia una politica de
retencion no coincidente a `/v1/da/ingest` y verifica que el manifest obtenido
expone el perfil impuesto en lugar de la intencion del caller.

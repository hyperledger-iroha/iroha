---
lang: es
direction: ltr
source: docs/portal/docs/da/replication-policy.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Fuente canónica
Reflejo `docs/source/da/replication_policy.md`. Gardez les deux versiones es
:::

# Política de replicación Disponibilidad de datos (DA-4)

_Estado: En curso - Responsables: Core Protocol WG / Equipo de Almacenamiento / SRE_

Le pipe d'ingest DA applique maintenant des objectifs de retención
deterministas para cada clase de blob decrite en `roadmap.md` (flujo de trabajo
DA-4). Torii rechazo de persister des sobres de retención fournis par le
persona que llama qui ne corresponsal pas a la politique configuree, garantissant que
chaque noeud validador/stockage retient le nombre requis d'epoques et de
réplicas sin dependencia de la intención del emetteur.

## Política por defecto

| Clase de blob | Retención caliente | Retención en frío | Solicitudes de réplicas | Clase de almacenamiento | Etiqueta de gobierno |
|---------------|---------------|----------------|-------------------|--------------------|--------------------|
| `taikai_segment` | 24 horas | 14 días | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 horas | 7 días | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 horas | 180 días | 3 | `cold` | `da.governance` |
| _Predeterminado (todas las otras clases)_ | 6 horas | 30 días | 3 | `warm` | `da.default` |Estos valores son enteros en `torii.da_ingest.replication_policy` y apliques.
a todas las presentaciones `/v1/da/ingest`. Torii recrit les manifests avec le
perfil de retención imponer y emitir un aviso cuando las personas que llaman facilitan
Los valores son incoherentes porque los operadores detectan SDK obsoletos.

### Clases de disponibilidad Taikai

Les manifests de routage Taikai (`taikai.trm`) declarant une `availability_class`
(`hot`, `warm`, o `cold`). Torii applique la politique correspondante avant le
fragmentación para que los operadores puedan ajustar las cuentas de réplicas por
Stream sin editor de la tabla global. Valores predeterminados:

| Clase de disponibilidad | Retención caliente | Retención en frío | Solicitudes de réplicas | Clase de almacenamiento | Etiqueta de gobierno |
|------------------------|---------------|----------------|-------------------|--------------------|--------------------|
| `hot` | 24 horas | 14 días | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 horas | 30 días | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 hora | 180 días | 3 | `cold` | `da.taikai.archive` |

Les tips manquants reviennent a `hot` afin que les transmisiones en vivo retiennent la
política la plus fuerte. Reemplace los valores predeterminados a través de
`torii.da_ingest.replication_policy.taikai_availability` si su investigación utiliza
des cibles diferentes.

## ConfiguraciónLa politique vit sous `torii.da_ingest.replication_policy` et exponen una plantilla
*predeterminado* más un cuadro de anulaciones por clase. Los identificadores de clase son
no distingue entre mayúsculas y minúsculas y acepta `taikai_segment`, `nexus_lane_sidecar`,
`governance_artifact`, o `custom:<u16>` para las extensiones aprobadas por la
gobernanza. Se aceptan clases de almacenamiento `hot`, `warm` o `cold`.

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

Deje el bloque intacto para utilizar los valores predeterminados establecidos. Pour durcir une
classe, mettez a jour l'override corresponsal; pour changer la base pour de
nuevas clases, editez `default_retention`.

Las clases de disponibilidad Taikai pueden tener recargos independientes a través de
`torii.da_ingest.replication_policy.taikai_availability`:

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

## Semántica de aplicación

- Torii reemplaza el `RetentionPolicy` proporcionado por el usuario del perfil
  imponer antes de la fragmentación o la emisión de manifiesto.
- Les manifiestos preconstruits qui declarant un profil de retención diferente
  sont rechazados avec `400 schema mismatch` afin que les client obsoletes ne
  puissent pas affaiblir le contrato.
- Chaque event d'override est logge (`blob_class`, política soumise vs asistente)
  para evidenciar que las personas que llaman no conformes durante el lanzamiento.

Ver [Plan de ingesta de disponibilidad de datos](ingest-plan.md) (lista de verificación de validación) para
le gate mis a jour couvrant l'enforcement de retention.

## Flujo de trabajo de replicación repetida (suivi DA-4)La aplicación de la retención no es la primera etapa. Los operadores hacen
También demuestra que los manifiestos viven y las órdenes de replicación permanecen.
se alinea con la política configurada hasta que SoraFS pueda volver a replicar los blobs
fuera de conformidad automática.

1. **Surveillez le drift.** Torii emet
   `overriding DA retention policy to match configured network baseline` cuando
   un llamador soumet des valores de retención obsoletos. Associez ce log avec la
   telemetría `torii_sorafs_replication_*` para reparar los déficits de réplicas
   ou des redespliegues retrasados.
2. **Diferencia entre intención y réplicas en vivo.** Utilice el nuevo asistente de auditoría:

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   El comando charge `torii.da_ingest.replication_policy` después de la configuración
   fournie, decodifica cada manifiesto (JSON o Norito), y asocia la opción
   Las cargas útiles `ReplicationOrderV1` por resumen de manifiesto. La señal de reanudación
   dos condiciones:

   - `policy_mismatch` - el perfil de retención del manifiesto diverge del perfil
     imponer (ceci ne devrait jamais Arrivalr sauf si Torii est mal configurado).
   - `replica_shortfall` - El orden de replicación en vivo requiere menos réplicas
     que `RetentionPolicy.required_replicas` o cuatro menos asignaciones que
     sa cible.Un estado de salida distinto de cero indica una insuficiencia activa después de que
   Automatización CI/buscapersonas de guardia de forma inmediata. Únase a la relación JSON
   en el paquete `docs/examples/da_manifest_review_template.md` para los votos del
   Parlamento.
3. **Declenchez la re-replication.** Quand l'audit signale une insuffisance,
   emettez un nouveau `ReplicationOrderV1` via les outils de gouvernance decrits
   en [SoraFS mercado de capacidad de almacenamiento](../sorafs/storage-capacity-marketplace.md)
   et relancez l'audit jusqu'a convergence du set de replicas. Para anular las anulaciones
   d'urgence, asociez the sortie CLI avec `iroha app da prove-availability` afin que
   les SRE puissent referencer le meme digest et la preuve PDP.

La cobertura de regresión vit en `integration_tests/tests/da/replication_policy.rs`;
la suite soumet una política de retención no conforme a `/v1/da/ingest` et verificar
que le manifest recupere expone le perfil impone plutot que l'intention du caller.
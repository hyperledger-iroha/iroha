---
lang: he
direction: rtl
source: docs/portal/docs/da/replication-policy.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::שימו לב פואנטה קנוניקה
Refleja `docs/source/da/replication_policy.md`. Mantenga ambas versiones en
:::

# Politica de replicacion de Data Availability (DA-4)

_Estado: En progreso -- אחראים: Core Protocol WG / Storage Team / SRE_

El pipeline de ingesta DA ahora aplica objetivos de retencion deterministas para
cada clase de blob decrita en `roadmap.md` (זרם עבודה DA-4). Torii rechaza
persistir envelopes de retencion provistos por el caller que no coincidan con la
politica configurada, garantizando que cada nodo validador/almacenamiento retiene
el numero requerido de epocas y replicas sin depender de la intencion del emisor.

## Politica por defecto

| קלאס דה בלוב | שימור חם | החזקת קור | רפליקות requeridas | Clase de almacenamiento | טאג דה גוברננסה |
|-------------|----------------|----------------|------------------------|------------------------|------------------------|
| `taikai_segment` | 24 שעות | 14 dias | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 הורות | 7 dias | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 הורות | 180 dias | 3 | `cold` | `da.governance` |
| _ברירת מחדל (todas las demas clases)_ | 6 הורות | 30 dias | 3 | `warm` | `da.default` |

Estos valores se incrustan en `torii.da_ingest.replication_policy` y se aplican a
כל התעסקויות `/v2/da/ingest`. Torii כתוב מחדש מניפסטים קו אל פרפיל דה
retencion impuesto y emite una advertencia cuando los callers entregan valores
ללא מקרים עבור מפעילי זיהוי SDKs desactualizados.

### Classes de disponibilidad Taikai

Los manifests de enrutamiento Taikai (`taikai.trm`) הצהיר un
`availability_class` (`hot`, `warm`, או `cold`). Torii אפליקציית פוליטיקה
correspondiente antes del chunking para que los operadores puedan escalar el
conteo de replicas por stream sin editar la tabla global. ברירות מחדל:

| Clase de disponibilidad | שימור חם | החזקת קור | רפליקות requeridas | Clase de almacenamiento | טאג דה גוברננסה |
|------------------------|--------------|----------------|------------------------|------------------------|------------------|
| `hot` | 24 שעות | 14 dias | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 הורות | 30 dias | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 הורה | 180 dias | 3 | `cold` | `da.taikai.archive` |

Las pistas faltantes usan `hot` por defecto para que las transmisiones en vivo
retengan la politica mas fuerte. Sobrescriba los מחדל דרך
`torii.da_ingest.replication_policy.taikai_availability` הוא אובייקט אדום בארה"ב
diferentes.

## תצורה

La politica vive bajo `torii.da_ingest.replication_policy` y expone un template
*ברירת מחדל* mas un arreglo de overrides por clase. Los identificadores de classe no
son sensibles a mayus/minus y aceptan `taikai_segment`, `nexus_lane_sidecar`,
`governance_artifact`, o `custom:<u16>` para extensiones aprobadas por gobernanza.
Las clases de almacenamiento aceptan `hot`, `warm`, או `cold`.

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
```Deje el bloque intacto para usar los defaults listdos arriba. פאר סבלנית
una clase, actualice el override correspondiente; para cambiar la base de nuevas
קלאסים, ערוך `default_retention`.

Las clases de disponibilidad Taikai pueden sobrescribirse de forma independiente
דרך `torii.da_ingest.replication_policy.taikai_availability`:

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

## סמנטיקה דה אכיפה

- Torii reemplaza el `RetentionPolicy` provisto por el usuario con el perfil
  impuesto antes del chunking o la emision de manifests.
- Los manifests preconstruidos que declaran un perfil de retencion distinto se
  rechazan con `400 schema mismatch` para que los clientes obsoletos no puedan
  debilitar el contrato.
- Cada evento de override se registra (`blob_class`, politica enviada vs esperada)
  עבור מתקשרים של exponer לא תואם את משך ההשקה.

Ver [Plan de ingesta de Data Availability](ingest-plan.md) (רשימת בדיקה של אימות)
para el gate actualizado que cubre el enforcement de retencion.

## Flujo de re-replicacion (seguimiento DA-4)

El enforcement de retencion es solo el primer paso. Los Operadores tambien deben
probar que los manifests en vivo y las ordenes de replicacion se mantienen
alineados con la politica configurada para que SoraFS pueda replicar blobs
fuera de cumplimiento de forma automatica.

1. **משמר אל סחף.** Torii פולט
   `overriding DA retention policy to match configured network baseline` cuando
   un caller envia valores de retencion desactualizados. Empareje ese log con
   la telemetria `torii_sorafs_replication_*` עבור detectar faltantes de replica
   o פריסה מחדש של דמורדו.
2. **הבדל כוונות לעומת העתקים en vivo.** השתמש ב-el nuevo helper de auditoria:

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   El comando carga `torii.da_ingest.replication_policy` מותאם לתצורה
   provista, decodifica cada manifest (JSON o Norito), y optionalmente empareja
   מטענים `ReplicationOrderV1` por digest de manifest. El resumen marca dos
   תנאים:

   - `policy_mismatch` - el perfil de retencion del manifest diverge de la
     politica impuesta (esto no deberia ocurrir salvo que Torii este mal
     configurado).
   - `replica_shortfall` - la orden de replicacion en vivo solicita menos replicas
     que `RetentionPolicy.required_replicas` o entrega menos asignaciones que su
     objetivo.

   Un status de salida no cero indica un faltante activo para que la automatizacion
   CI/on-call pueda pager de inmediato. תוספת לדיווח של JSON לכל חבילת
   `docs/examples/da_manifest_review_template.md` para votos del Parlamento.
3. **הסר שכפול מחדש.** Cuando la auditoria reporte un faltante, emita una
   nueva `ReplicationOrderV1` via las herramientas de gobernanza descritas en
   [שוק קיבולת אחסון SoraFS](../sorafs/storage-capacity-marketplace.md)
   y vuelva a ejecutar la auditoria hasta que el set de replicas converja. פסקה
   עוקף את דה emergencia, empareje la salida de la CLI con `iroha app da prove-availability`
   para que SREs puedan referenciar el mismo digest y Evidencia PDP.La cobertura de regresion vive en
`integration_tests/tests/da/replication_policy.rs`; la suite envia una politica de
שימור ללא מקרי א `/v2/da/ingest` y verifica que el manifest obtenido
expone el perfil impuesto en lugar de la intencion del caller.
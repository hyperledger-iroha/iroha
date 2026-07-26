---
lang: es
direction: ltr
source: docs/source/nexus_lanes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 94891050512eaf78f4c0381c0facbeed445a7e7323297070ae537e4d38ca7fe4
source_last_modified: "2025-12-13T05:07:11.953030+00:00"
translation_last_reviewed: 2026-01-01
---

# Modelo de lanes Nexus y particion de WSV

> **Estado:** entregable NX-1 - la taxonomia de lanes, la geometria de configuracion y el layout de storage estan listos para implementacion.  
> **Responsables:** Nexus Core WG, Governance WG  
> **Elemento de roadmap relacionado:** NX-1

Este documento captura la arquitectura objetivo para la capa de consenso multi-lane de Nexus. El objetivo es producir un unico estado mundial determinista mientras se permite que data spaces (lanes) individuales ejecuten conjuntos de validadores publicos o privados con cargas aisladas.

> **Pruebas cross-lane:** Esta nota se centra en geometria y almacenamiento. Los commitments de settlement por lane, el pipeline de relay y las pruebas de merge-ledger requeridas para el roadmap **NX-4** se detallan en [nexus_cross_lane.md](nexus_cross_lane.md).

## Conceptos

- **Lane:** shard logico del ledger Nexus con su propio conjunto de validadores y backlog de ejecucion. Identificada por un `LaneId` estable.
- **Data Space:** bucket de gobernanza que agrupa una o mas lanes que comparten politicas de compliance, ruteo y settlement. Cada dataspace tambien declara `fault_tolerance (f)` usado para dimensionar comites de relay de lane (`3f+1`).
- **Lane Manifest:** metadatos controlados por gobernanza que describen validadores, politica de DA, token de gas, reglas de settlement y permisos de ruteo.
- **Global Commitment:** prueba emitida por una lane que resume nuevas roots de estado, datos de settlement y transferencias cross-lane opcionales. El anillo NPoS global ordena commitments.

## Taxonomia de lanes

Los tipos de lane describen de forma canonica su visibilidad, superficie de gobernanza y hooks de settlement. La geometria de configuracion (`LaneConfig`) captura estos atributos para que nodos, SDKs y tooling puedan razonar sobre el layout sin logica a medida.

| Tipo de lane | Visibilidad | Membresia de validadores | Exposicion WSV | Gobernanza por defecto | Politica de settlement | Uso tipico |
|-------------|------------|--------------------------|---------------|------------------------|------------------------|-----------|
| `default_public` | publico | Permissionless (stake global) | Replica completa de estado | Parlamento SORA | `xor_global` | Ledger publico base |
| `public_custom` | publico | Permissionless o stake-gated | Replica completa de estado | Modulo ponderado por stake | `xor_lane_weighted` | Aplicaciones publicas de alto throughput |
| `private_permissioned` | restringido | Conjunto fijo de validadores (aprobado por gobernanza) | Commitments y proofs | Consejo federado | `xor_hosted_custody` | CBDC, cargas de consorcio |
| `hybrid_confidential` | restringido | Membresia mixta; envuelve pruebas ZK | Commitments + divulgacion selectiva | Modulo de dinero programable | `xor_dual_fund` | Dinero programable con privacidad |

Todos los tipos de lane deben declarar:

- Alias de dataspace - agrupacion legible que enlaza politicas de compliance.
- Handle de gobernanza - identificador resuelto a traves de `Nexus.governance.modules`.
- Handle de settlement - identificador consumido por el settlement router para debitar buffers XOR.
- Metadatos de telemetria opcionales (descripcion, contacto, dominio de negocio) expuestos en `/status` y dashboards.

## Geometria de configuracion de lanes (`LaneConfig`)

`LaneConfig` es la geometria en runtime derivada del catalogo de lanes validado. No reemplaza los manifests de gobernanza; en su lugar entrega identificadores de storage deterministas y pistas de telemetria para cada lane configurada.

```text
LaneConfigEntry {
    lane_id: LaneId,           // stable identifier
    alias: String,             // human-readable alias
    slug: String,              // sanitised alias for file/metric keys
    kura_segment: String,      // Kura segment directory: lane_{id:03}_{slug}
    merge_segment: String,     // Merge-ledger segment: lane_{id:03}_merge
    key_prefix: [u8; 4],       // Big-endian LaneId prefix for WSV key spaces
    shard_id: ShardId,         // WSV/Kura shard binding (defaults to lane_id)
    visibility: LaneVisibility,// public vs restricted lanes
    storage_profile: LaneStorageProfile,
    proof_scheme: DaProofScheme,// DA proof policy (merkle_sha256 default)
}
```

- `LaneConfig::from_catalog` recalcula la geometria cada vez que se carga la configuracion (`State::set_nexus`).
- Los aliases se sanitizan a slugs en minusculas; caracteres consecutivos no alfanumericos se colapsan en `_`. Si el alias produce un slug vacio se usa `lane{id}`.
- Los prefijos de clave garantizan que WSV mantenga rangos por lane disjuntos incluso si el mismo backend es compartido.
- `shard_id` se deriva de la clave metadata del catalogo `da_shard_id` (por defecto `lane_id`) y dirige el journal persistido del cursor de shard para mantener el replay de DA determinista en reinicios o resharding.
- Los nombres de segmento Kura son deterministas entre hosts; los auditores pueden cruzar directorios de segmentos y manifests sin tooling a medida.
- Los segmentos merge (`lane_{id:03}_merge`) contienen las ultimas roots de merge-hint y los commitments de estado global para esa lane.
- Cuando la gobernanza renombra un alias de lane, los nodos reetiquetan automaticamente los directorios `blocks/lane_{id:03}_{slug}` (y snapshots en niveles) para que los auditores siempre vean el slug canonico sin limpieza manual.

## Particion de world-state

- El world state logico de Nexus es la union de los espacios de estado por lane. Las lanes publicas persisten estado completo; las lanes privadas/confidenciales exportan roots de Merkle/commitment al merge ledger.
- El almacenamiento MV prefija cada clave con el prefijo de 4 bytes de la lane desde `LaneConfigEntry::key_prefix`, produciendo claves como `[00 00 00 01] ++ PackedKey`.
- Las tablas compartidas (cuentas, assets, triggers, registros de gobernanza) por lo tanto almacenan entradas agrupadas por prefijo de lane, manteniendo scans de rango deterministas.
- Los metadatos del merge-ledger reflejan el mismo layout: cada lane escribe roots de merge-hint y roots reducidas de estado global en `lane_{id:03}_merge`, permitiendo retencion o eviction dirigida cuando una lane se retira.
- Indices cross-lane (aliases de cuenta, registros de assets, manifests de gobernanza) almacenan pares explicitos `(LaneId, DataSpaceId)`. Estos indices viven en column families compartidas pero usan el prefijo de lane y los ids de dataspace explicitos para mantener lookups deterministas.
- El workflow de merge combina datos publicos con commitments privados usando tuplas `(lane_id, dataspace_id, height, state_root, settlement_root, proof_root)` derivadas de entradas del merge-ledger.

## Particion de Kura y WSV

- **Segmentos Kura**
  - `lane_{id:03}_{slug}` - segmento principal de bloques para la lane (bloques, indices, recibos).
  - `lane_{id:03}_merge` - segmento de merge-ledger que registra roots de estado reducidas y artefactos de settlement.
  - Los segmentos globales (evidencia de consenso, caches de telemetria) siguen compartidos porque son neutrales a lane; sus claves no incluyen prefijos de lane.
- El runtime vigila actualizaciones del catalogo de lanes: las lanes nuevas tienen sus directorios de bloques y merge-ledger provisionados automaticamente bajo `kura/blocks/` y `kura/merge_ledger/`, mientras que las lanes retiradas se archivan bajo `kura/retired/{blocks,merge_ledger}/lane_{id:03}_*`.
- Los snapshots de estado en niveles reflejan el mismo ciclo; cada lane escribe en `<cold_root>/lanes/lane_{id:03}_{slug}`, donde `<cold_root>` es `cold_store_root` (o `da_store_root` cuando `cold_store_root` no esta configurado), y los retiros migran el arbol de directorios a `<cold_root>/retired/lanes/`.
- **Prefijos de clave** - el prefijo de 4 bytes computado desde `LaneId` siempre se antepone a claves MV codificadas. No se usa hashing especifico del host, asi que el orden es identico entre nodos.
- **Layout de block log** - datos de bloque, indice y hashes se anidan bajo `kura/blocks/lane_{id:03}_{slug}/`. Los journals de merge-ledger reutilizan el mismo slug (`kura/merge/lane_{id:03}_{slug}.log`), manteniendo flujos de recuperacion por lane aislados.
- **Politica de retencion** - las lanes publicas retienen cuerpos completos de bloques; las lanes solo con commitments pueden compactar cuerpos antiguos despues de checkpoints porque los commitments son autoritativos. Las lanes confidenciales mantienen journals cifrados en segmentos dedicados para no bloquear otras cargas.
- **Tooling** - `cargo xtask nexus-lane-maintenance --config <path> [--compact-retired]` inspecciona `<store>/blocks` y `<store>/merge_ledger` usando el `LaneConfig` derivado, reporta segmentos activos vs retirados, y archiva directorios/logs retirados bajo `<store>/retired/...` para mantener evidencia determinista. Utilidades de mantenimiento (`kagami`, comandos admin CLI) deben reutilizar el namespace con slug al exponer metricas, labels de Prometheus o al archivar segmentos Kura.

## Presupuestos de almacenamiento

- `nexus.storage.max_disk_usage_bytes` define el presupuesto total en disco que los nodos Nexus deben consumir entre Kura, snapshots frios de WSV, almacenamiento SoraFS y spools de streaming (SoraNet/SoraVPN).
- `nexus.storage.budget_enforce_interval_blocks` define el intervalo (en bloques confirmados) entre los escaneos de aplicacion del presupuesto; 0 = cada bloque.
- Cuando se supera el presupuesto global, la eviction es determinista: se podan los spools de provision de SoraNet en orden lexicografico de ruta, luego los spools de SoraVPN, luego los snapshots frios del tiered-state (del mas antiguo al mas nuevo, con offload a `da_store_root` cuando esta configurado), luego los segmentos retirados de Kura, y por ultimo se expulsan los cuerpos de bloque activos de Kura a `da_blocks/` para rehidratacion con DA al leer.
- `nexus.storage.max_wsv_memory_bytes` limita la capa caliente de WSV propagando el tamano determinista en memoria del WSV en `tiered_state.hot_retained_bytes`; la retencion de gracia puede exceder temporalmente el presupuesto, pero el exceso es observable via telemetria (`state_tiered_hot_bytes`, `state_tiered_hot_grace_overflow_bytes`).
- `nexus.storage.disk_budget_weights` divide el presupuesto de disco entre componentes usando puntos base (debe sumar 10.000). Los topes derivados se aplican a `kura.max_disk_usage_bytes`, `tiered_state.max_cold_bytes`, `sorafs.storage.max_capacity_bytes`, `streaming.soranet.provision_spool_max_bytes` y `streaming.soravpn.provision_spool_max_bytes`.
- La aplicacion del presupuesto de Kura suma los bytes del block-store en segmentos de lanes activas y retiradas e incluye bloques en cola aun no persistidos para evitar sobrepasos durante el retraso de escritura.
- Los spools de aprovisionamiento SoraVPN usan la configuracion `streaming.soravpn` y se limitan de forma independiente del spool de aprovisionamiento de SoraNet.
- Los limites por componente siguen aplicando: cuando un componente tiene un tope explicito no cero, se aplica el menor entre ese tope y el presupuesto Nexus derivado.
- La telemetria de presupuestos usa `storage_budget_bytes_used{component=...}` y `storage_budget_bytes_limit{component=...}` para reportar uso/topes de `kura`, `wsv_hot`, `wsv_cold`, `soranet_spool` y `soravpn_spool`; `storage_budget_exceeded_total{component=...}` incrementa cuando la aplicacion rechaza nuevos datos y los logs emiten una advertencia al operador.
- La telemetria de eviction DA agrega `storage_da_cache_total{component=...,result=hit|miss}` y `storage_da_churn_bytes_total{component=...,direction=evicted|rehydrated}` para seguir la actividad de cache y los bytes movidos de `kura` y `wsv_cold`.
- Kura reporta la misma contabilidad usada durante la admision (bytes en disco mas bloques en cola, incluyendo payloads del merge-ledger cuando estan presentes), por lo que los medidores reflejan presion efectiva y no solo bytes persistidos.

## Routing y APIs

- Los endpoints REST/gRPC de Torii aceptan un `lane_id` opcional; la ausencia implica `lane_default`.
- Los SDKs exponen selectores de lane y mapean aliases amigables a `LaneId` usando el catalogo de lanes.
- Las reglas de routing operan sobre el catalogo validado y pueden elegir tanto lane como dataspace. `LaneConfig` provee aliases amigables para telemetria en dashboards y logs.

## Settlement y fees

- Cada lane paga fees en XOR al conjunto global de validadores. Las lanes pueden cobrar tokens de gas nativos pero deben poner en escrow equivalentes XOR junto a los commitments.
- Las pruebas de settlement incluyen monto, metadata de conversion y prueba de escrow (por ejemplo, transferencia a la vault global de fees).
- El settlement router unificado (NX-3) debita buffers usando los mismos prefijos de lane, para que la telemetria de settlement alinee con la geometria de storage.

## Gobernanza

- Las lanes declaran su modulo de gobernanza via el catalogo. `LaneConfigEntry` mantiene el alias y slug originales para que telemetria y auditorias sean legibles.
- El registro Nexus distribuye manifests de lane firmados que incluyen `LaneId`, binding de dataspace, handle de gobernanza, handle de settlement y metadata.
- Los hooks de runtime-upgrade siguen aplicando politicas de gobernanza (`gov_upgrade_id` por defecto) y registran diffs via el puente de telemetria (eventos `nexus.config.diff`).
- Los manifests de lane definen el pool de validadores de dataspace para lanes administradas; las lanes elegidas por stake derivan su pool de validadores desde registros de staking de lanes publicas.

## Telemetria y status

- `/status` expone aliases de lane, bindings de dataspace, handles de gobernanza y perfiles de settlement, derivados del catalogo y `LaneConfig`.
- Las metricas del scheduler (`nexus_scheduler_lane_teu_*`) muestran aliases/slugs de lane para que los operadores mapeen backlog y presion TEU rapidamente.
- `nexus_lane_configured_total` cuenta el numero de entradas de lane derivadas y se recalcula cuando cambia la configuracion. La telemetria emite diffs firmados cuando cambia la geometria de lanes.
- Los gauges de backlog de dataspace incluyen metadata de alias/descripcion para ayudar a los operadores a asociar presion de cola con dominios de negocio.

## Configuracion y tipos Norito

- `LaneCatalog`, `LaneConfig` y `DataSpaceCatalog` viven en `iroha_data_model::nexus` y proveen estructuras en formato Norito para manifests y SDKs.
- `LaneConfig` vive en `iroha_config::parameters::actual::Nexus` y se deriva automaticamente del catalogo; no requiere encoding Norito porque es un helper interno de runtime.
- La configuracion orientada al usuario (`iroha_config::parameters::user::Nexus`) sigue aceptando descriptores declarativos de lanes y dataspaces; el parseo ahora deriva la geometria y rechaza aliases invalidos o ids de lane duplicados.
- `DataSpaceMetadata.fault_tolerance` controla el tamano del comite de relay de lane; la membresia del comite se muestrea de forma determinista por epoca desde el pool de validadores del dataspace usando la semilla de epoca VRF ligada con `(dataspace_id, lane_id)`.

## Trabajo pendiente

- Integrar actualizaciones del settlement router (NX-3) con la nueva geometria para que los debitos de buffers XOR y los recibos se etiqueten por slug de lane.
- Finalizar el algoritmo de merge (ordenamiento, pruning, deteccion de conflictos) y adjuntar fixtures de regresion para replay cross-lane.
- Agregar hooks de compliance para whitelists/blacklists y politicas de dinero programable (seguido bajo NX-12).

---

*Este documento evolucionara a medida que avancen tareas NX-2 a NX-18. Por favor captura preguntas abiertas en el roadmap o el tracker de gobernanza.*

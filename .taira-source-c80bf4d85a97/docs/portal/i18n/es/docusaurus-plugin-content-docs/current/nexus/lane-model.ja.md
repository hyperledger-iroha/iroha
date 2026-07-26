---
lang: ja
direction: ltr
source: docs/portal/i18n/es/docusaurus-plugin-content-docs/current/nexus/lane-model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ee0966f24329aab91cea9d0b0fdcf9329308267b36f9198a7f98c8b5b3258872
source_last_modified: "2026-01-04T17:32:29+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: es
direction: ltr
source: docs/portal/docs/nexus/lane-model.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
id: nexus-lane-model
title: Modelo de lanes de Nexus
description: Taxonomia logica de lanes, geometria de configuracion y reglas de merge de world-state para Sora Nexus.
---

# Modelo de lanes de Nexus y particionado de WSV

> **Estado:** entregable NX-1 - taxonomia de lanes, geometria de configuracion y layout de storage listos para implementacion.  
> **Owners:** Nexus Core WG, Governance WG  
> **Referencia de roadmap:** NX-1 en `roadmap.md`

Esta pagina del portal refleja el brief canonico `docs/source/nexus_lanes.md` para que operadores de Sora Nexus, owners de SDK y revisores puedan leer la guia de lanes sin entrar en el arbol mono-repo. La arquitectura objetivo mantiene el world state determinista mientras permite que data spaces (lanes) individuales ejecuten conjuntos de validadores publicos o privados con workloads aislados.

## Conceptos

- **Lane:** shard logico del ledger de Nexus con su propio set de validadores y backlog de ejecucion. Identificado por un `LaneId` estable.
- **Data Space:** bucket de governance que agrupa una o mas lanes que comparten politicas de compliance, routing y settlement.
- **Lane Manifest:** metadata controlada por governance que describe validadores, politica de DA, token de gas, reglas de settlement y permisos de routing.
- **Global Commitment:** proof emitida por una lane que resume nuevos roots de estado, datos de settlement y transferencias cross-lane opcionales. El anillo NPoS global ordena commitments.

## Taxonomia de lanes

Los tipos de lane describen de forma canonica su visibilidad, superficie de governance y hooks de settlement. La geometria de configuracion (`LaneConfig`) captura estos atributos para que nodos, SDKs y tooling puedan razonar sobre el layout sin logica bespoke.

| Tipo de lane | Visibilidad | Membresia de validadores | Exposicion WSV | Gobernanza por defecto | Politica de settlement | Uso tipico |
|-----------|------------|----------------------|--------------|--------------------|-------------------|-------------|
| `default_public` | public | Permissionless (global stake) | Replica de estado completa | SORA Parliament | `xor_global` | Ledger publico base |
| `public_custom` | public | Permissionless o stake-gated | Replica de estado completa | Modulo ponderado por stake | `xor_lane_weighted` | Aplicaciones publicas de alto throughput |
| `private_permissioned` | restricted | Set fijo de validadores (aprobado por governance) | Commitments y proofs | Federated council | `xor_hosted_custody` | CBDC, workloads de consorcio |
| `hybrid_confidential` | restricted | Membresia mixta; envuelve ZK proofs | Commitments + disclosure selectiva | Modulo de dinero programable | `xor_dual_fund` | Dinero programable con preservacion de privacidad |

Todos los tipos de lane deben declarar:

- Alias de dataspace - agrupacion legible por humanos que vincula politicas de compliance.
- Handle de governance - identificador resuelto via `Nexus.governance.modules`.
- Handle de settlement - identificador consumido por el settlement router para debitar buffers XOR.
- Metadata opcional de telemetria (descripcion, contacto, dominio de negocio) expuesta via `/status` y dashboards.

## Geometria de configuracion de lanes (`LaneConfig`)

`LaneConfig` es la geometria runtime derivada del catalogo de lanes validado. No reemplaza los manifests de governance; en su lugar provee identificadores de storage deterministas y pistas de telemetria para cada lane configurada.

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

- `LaneConfig::from_catalog` recalcula la geometria cuando se carga la configuracion (`State::set_nexus`).
- Los aliases se sanitizan a slugs en minusculas; caracteres no alfanumericos consecutivos se colapsan en `_`. Si el alias produce un slug vacio, usamos `lane{id}`.
- `shard_id` se deriva de la key de metadata `da_shard_id` (por defecto `lane_id`) y conduce el journal de cursor de shard persistido para mantener el replay de DA determinista entre restarts/resharding.
- Los prefijos de clave aseguran que el WSV mantenga rangos de claves por lane disjuntos aun cuando se comparte el mismo backend.
- Los nombres de segmentos Kura son deterministas entre hosts; auditores pueden verificar directorios de segmento y manifests sin tooling bespoke.
- Los segmentos merge (`lane_{id:03}_merge`) guardan las ultimas roots de merge-hint y commitments de estado global para esa lane.

## Particionado de world-state

- El world state logico de Nexus es la union de espacios de estado por lane. Las lanes publicas persisten estado completo; las lanes privadas/confidential exportan roots Merkle/commitment al merge ledger.
- El almacenamiento MV prefija cada clave con el prefijo de 4 bytes de `LaneConfigEntry::key_prefix`, generando claves como `[00 00 00 01] ++ PackedKey`.
- Las tablas compartidas (accounts, assets, triggers, registros de governance) almacenan entradas agrupadas por prefijo de lane, manteniendo los range scans deterministas.
- La metadata del merge-ledger refleja el mismo layout: cada lane escribe roots de merge-hint y roots de estado global reducido en `lane_{id:03}_merge`, permitiendo retencion o eviction dirigida cuando una lane se retira.
- Los indices cross-lane (account aliases, asset registries, governance manifests) almacenan prefijos de lane explicitos para que los operadores reconcilien entradas rapidamente.
- **Politica de retencion** - las lanes publicas retienen cuerpos de bloque completos; las lanes solo de commitments pueden compactar cuerpos antiguos despues de checkpoints porque los commitments son autoritativos. Las lanes confidential guardan journals cifrados en segmentos dedicados para no bloquear otros workloads.
- **Tooling** - utilidades de mantenimiento (`kagami`, comandos admin de CLI) deben referenciar el namespace con slug al exponer metricas, etiquetas Prometheus o al archivar segmentos Kura.

## Routing y APIs

- Endpoints Torii REST/gRPC aceptan un `lane_id` opcional; la ausencia implica `lane_default`.
- Los SDKs exponen selectores de lane y mapean aliases amigables a `LaneId` usando el catalogo de lanes.
- Las reglas de routing operan sobre el catalogo validado y pueden elegir lane y dataspace. `LaneConfig` provee aliases amigables para telemetria en dashboards y logs.

## Settlement y fees

- Cada lane paga fees XOR al set global de validadores. Las lanes pueden cobrar tokens de gas nativos pero deben hacer escrow de equivalentes XOR junto con commitments.
- Las proofs de settlement incluyen monto, metadata de conversion y prueba de escrow (por ejemplo, transferencia al vault global de fees).
- El settlement router unificado (NX-3) debita buffers usando los mismos prefijos de lane, asi la telemetria de settlement se alinea con la geometria de storage.

## Governance

- Las lanes declaran su modulo de governance via el catalogo. `LaneConfigEntry` lleva el alias y slug originales para mantener legibles la telemetria y los audit trails.
- El registry de Nexus distribuye lane manifests firmados que incluyen el `LaneId`, binding de dataspace, handle de governance, handle de settlement y metadata.
- Los hooks de runtime-upgrade siguen aplicando politicas de governance (`gov_upgrade_id` por defecto) y registran diffs via el telemetry bridge (eventos `nexus.config.diff`).

## Telemetria y status

- `/status` expone aliases de lane, bindings de dataspace, handles de governance y perfiles de settlement, derivados del catalogo y `LaneConfig`.
- Las metricas del scheduler (`nexus_scheduler_lane_teu_*`) muestran aliases/slugs para que los operadores mapeen backlog y presion de TEU rapidamente.
- `nexus_lane_configured_total` cuenta el numero de entradas de lane derivadas y se recalcula cuando cambia la configuracion. La telemetria emite diffs firmados cuando cambia la geometria de lanes.
- Los gauges de backlog de dataspace incluyen metadata de alias/descripcion para ayudar a los operadores a asociar presion de cola con dominios de negocio.

## Configuracion y tipos Norito

- `LaneCatalog`, `LaneConfig`, y `DataSpaceCatalog` viven en `iroha_data_model::nexus` y proveen estructuras en formato Norito para manifests y SDKs.
- `LaneConfig` vive en `iroha_config::parameters::actual::Nexus` y se deriva automaticamente del catalogo; no requiere encoding Norito porque es un helper interno runtime.
- La configuracion de cara al usuario (`iroha_config::parameters::user::Nexus`) sigue aceptando descriptores declarativos de lane y dataspace; el parseo ahora deriva la geometria y rechaza aliases invalidos o IDs de lane duplicados.

## Trabajo pendiente

- Integrar updates del settlement router (NX-3) con la nueva geometria para que los debitos y recibos de buffer XOR se etiqueten por slug de lane.
- Extender tooling admin para listar column families, compactar lanes retiradas e inspeccionar logs de bloques por lane usando el namespace con slug.
- Finalizar el algoritmo de merge (ordering, pruning, conflict detection) y adjuntar fixtures de regresion para replay cross-lane.
- Agregar hooks de compliance para whitelists/blacklists y politicas de dinero programable (seguido bajo NX-12).

---

*Esta pagina seguira rastreando follow-ups de NX-1 a medida que aterricen NX-2 hasta NX-18. Por favor exponga preguntas abiertas en `roadmap.md` o en el tracker de governance para que el portal siga alineado con los docs canonicos.*

---
lang: es
direction: ltr
source: docs/portal/docs/nexus/lane-model.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: modelo-carril-nexus
título: Modelo de carriles de Nexus
descripción: Taxonomía lógica de carriles, geometría de configuración y reglas de fusión de estado mundial para Sora Nexus.
---

# Modelo de carriles de Nexus y particionado de WSV

> **Estado:** entregable NX-1 - taxonomía de carriles, geometría de configuración y diseño de almacenamiento listos para implementación.  
> **Propietarios:** Nexus Grupo de Trabajo Principal, Grupo de Trabajo de Gobernanza  
> **Referencia de hoja de ruta:** NX-1 en `roadmap.md`

Esta página del portal refleja el breve canónico `docs/source/nexus_lanes.md` para que operadores de Sora Nexus, propietarios de SDK y revisores puedan leer la guía de carriles sin entrar en el árbol mono-repo. La arquitectura objetivo mantiene el estado mundial determinista mientras permite que espacios de datos (carriles) individuales ejecuten conjuntos de validadores públicos o privados con cargas de trabajo aisladas.

## Conceptos- **Lane:** shard logico del ledger de Nexus con su propio set de validadores y backlog de ejecucion. Identificado por un `LaneId` estable.
- **Espacio de datos:** grupo de gobernanza que agrupa una o más líneas que comparten políticas de cumplimiento, enrutamiento y liquidación.
- **Lane Manifest:** metadatos controlados por gobernanza que describen validadores, política de DA, token de gas, reglas de liquidación y permisos de enrutamiento.
- **Compromiso Global:** comprobante emitido por un carril que resume nuevas raíces de estado, datos de liquidación y transferencias cross-lane opcionales. El anillo NPoS compromisos de ordena global.

## Taxonomía de carriles

Los tipos de carril descritos de forma canónica su visibilidad, superficie de gobierno y ganchos de asentamiento. La geometría de configuración (`LaneConfig`) captura estos atributos para que nodos, SDK y herramientas puedan razonar sobre el diseño sin lógica personalizada.| Tipo de carril | Visibilidad | Membresía de validadores | Exposición WSV | Gobernanza por defecto | Política de solución | Uso típico |
|-----------|------------|----------------------|--------------|--------------------|-------------------|-------------|
| `default_public` | público | Sin permiso (participación global) | Réplica de estado completa | SORA Parlamento | `xor_global` | Base del libro mayor público |
| `public_custom` | público | Sin permiso o con participación | Réplica de estado completa | Módulo ponderado por estaca | `xor_lane_weighted` | Aplicaciones públicas de alto rendimiento |
| `private_permissioned` | restringido | Conjunto fijo de validadores (aprobado por gobernanza) | Compromisos y pruebas | Consejo federado | `xor_hosted_custody` | CBDC, cargas de trabajo de consorcio |
| `hybrid_confidential` | restringido | Membresía mixta; envuelve pruebas ZK | Compromisos + divulgación selectiva | Módulo de dinero programable | `xor_dual_fund` | Dinero programable con preservación de privacidad |

Todos los tipos de carril deben declarar:

- Alias de dataspace - agrupación legible por humanos que vincula políticas de cumplimiento.
- Handle de Governance - identificador resuelto vía `Nexus.governance.modules`.
- Handle de liquidación - identificador consumido por el router de liquidación para debitar buffers XOR.
- Metadatos opcionales de telemetría (descripción, contacto, dominio de negocio) expuestos vía `/status` y paneles de control.## Geometría de configuración de carriles (`LaneConfig`)

`LaneConfig` es la geometría runtime derivada del catálogo de carriles validado. No reemplazar los manifiestos de gobernanza; en su lugar proporciona identificadores de almacenamiento deterministas y pistas de telemetría para cada carril configurado.

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

- `LaneConfig::from_catalog` recalcula la geometría cuando se carga la configuración (`State::set_nexus`).
- Los alias se sanitizan a slugs en minusculas; caracteres no alfanuméricos consecutivos se colapsan en `_`. Si el alias produce un slug vacio, usamos `lane{id}`.
- `shard_id` se deriva de la clave de metadatos `da_shard_id` (por defecto `lane_id`) y conduce el diario de cursor de shard persistente para mantener la reproducción de DA determinista entre reinicios/resharding.
- Los prefijos de clave aseguran que el WSV mantenga rangos de claves por carril disjuntos aun cuando se comparte el mismo backend.
- Los nombres de segmentos Kura son deterministas entre hosts; auditores pueden directorio verificars de segmento y manifiestos sin herramientas a medida.
- Los segmentos merge (`lane_{id:03}_merge`) guardan las últimas raíces de merge-hint y compromisos de estado global para ese carril.

## Particionado de estado-mundial- El world state logico de Nexus es la union de espacios de estado por lane. Las calles públicas persisten en estado completo; las lanes privadas/confidential exportan root Merkle/commitment al merge ledger.
- El almacenamiento MV prefija cada clave con el prefijo de 4 bytes de `LaneConfigEntry::key_prefix`, generando claves como `[00 00 00 01] ++ PackedKey`.
- Las tablas compartidas (cuentas, activos, activadores, registros de gobernanza) almacenan entradas agrupadas por prefijo de carril, manteniendo los escaneos de rango deterministas.
- Los metadatos del merge-ledger reflejan el mismo diseño: cada carril escribe raíces de merge-hint y raíces de estado global reducido en `lane_{id:03}_merge`, permitiendo retención o desalojo dirigido cuando un carril se retira.
- Los índices cross-lane (alias de cuentas, registros de activos, manifiestos de gobernanza) almacenan prefijos de carril explícitos para que los operadores reconcilien entradas rápidamente.
- **Politica de retencion** - las líneas públicas retienen cuerpos de bloque completos; las lanes solo de compromisos pueden compactar cuerpos antiguos despues de checkpoints porque los compromisos son autoritativos. Las líneas confidenciales guardan diarios cifrados en segmentos dedicados para no bloquear otras cargas de trabajo.- **Tooling** - utilidades de mantenimiento (`kagami`, comandos admin de CLI) deben referenciar el namespace con slug al exponer métricas, etiquetas Prometheus o al archivar segmentos Kura.

## Enrutamiento y API

- Los puntos finales Torii REST/gRPC aceptan un `lane_id` opcional; la ausencia implica `lane_default`.
- Los SDK exponen selectores de carril y mapean alias amigables a `LaneId` usando el catálogo de carriles.
- Las reglas de enrutamiento operan sobre el catálogo validado y pueden elegir carril y espacio de datos. `LaneConfig` proporciona alias amigables para telemetría en paneles y registros.

## Liquidación y honorarios

- Cada carril paga tarifas XOR al conjunto global de validadores. Las líneas pueden cobrar tokens de gas nativos pero deben hacer escrow de equivalentes XOR junto con compromisos.
- Las pruebas de liquidación incluyen monto, metadatos de conversión y prueba de depósito en garantía (por ejemplo, transferencia al vault global de fee).
- El enrutador de liquidación unificado (NX-3) debita buffers usando los mismos prefijos de carril, así la telemetría de liquidación se alinea con la geometría de almacenamiento.

## Gobernanza- Las líneas declaran su módulo de gobernanza a través del catálogo. `LaneConfigEntry` lleva el alias y slug originales para mantener legibles la telemetria y los audit trails.
- El registro de Nexus distribuye carriles manifiestos firmados que incluyen el `LaneId`, vinculación de espacio de datos, manejo de gobernanza, manejo de liquidación y metadatos.
- Los ganchos de runtime-upgrade siguen aplicando políticas de gobernanza (`gov_upgrade_id` por defecto) y registran diferencias a través del puente de telemetría (eventos `nexus.config.diff`).

## Telemetria y estado

- `/status` exponen alias de carril, enlaces de espacio de datos, manijas de gobernanza y perfiles de liquidación, derivados del catálogo y `LaneConfig`.
- Las métricas del planificador (`nexus_scheduler_lane_teu_*`) muestran alias/slugs para que los operadores mapeen backlog y presion de TEU rapidamente.
- `nexus_lane_configured_total` cuenta el número de entradas de carril derivadas y se recalcula cuando cambia la configuración. La telemetria emite diferencias firmadas cuando cambia la geometria de carriles.
- Los indicadores de backlog de dataspace incluyen metadatos de alias/descripción para ayudar a los operadores a asociar presión de cola con dominios de negocio.

## Configuración y tipos Norito- `LaneCatalog`, `LaneConfig`, y `DataSpaceCatalog` viven en `iroha_data_model::nexus` y proveen estructuras en formato Norito para manifiestos y SDK.
- `LaneConfig` vive en `iroha_config::parameters::actual::Nexus` y se deriva automáticamente del catálogo; no requiere codificación Norito porque es un asistente interno en tiempo de ejecución.
- La configuración de cara al usuario (`iroha_config::parameters::user::Nexus`) sigue aceptando descriptores declarativos de carril y espacio de datos; el parseo ahora deriva la geometría y rechaza alias inválidos o ID de carril duplicados.

## Trabajo pendiente

- Integrar actualizaciones del enrutador de liquidación (NX-3) con la nueva geometría para que los débitos y recibos de buffer XOR se etiqueten por slug de lane.
- Administrador de herramientas extendidas para listar familias de columnas, compactar retiradas de carriles e inspeccionar registros de bloques por carril usando el espacio de nombres con slug.
- Finalizar el algoritmo de fusión (ordenamiento, poda, detección de conflictos) y adjuntar elementos de regresión para reproducir cross-lane.
- Agregar ganchos de cumplimiento para listas blancas/listas negras y políticas de dinero programables (seguido bajo NX-12).

---

*Esta página seguirá rastreando seguimientos de NX-1 a medida que aterricen NX-2 hasta NX-18. Por favor exponga preguntas abiertas en `roadmap.md` o en el rastreador de gobernanza para que el portal siga alineado con los documentos canónicos.*
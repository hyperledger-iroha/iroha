---
lang: es
direction: ltr
source: docs/portal/docs/nexus/lane-model.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: modelo-carril-nexus
título: Modelo de carriles Nexus
descripción: Taxonomía lógica de carriles, geometría de configuración y reglas de fusión del estado mundial para Sora Nexus.
---

# Modelo de carriles Nexus y partición WSV

> **Estado:** livrable NX-1: taxonomía de carriles, geometría de configuración y diseño de ajustes de almacenamiento para implementación.  
> **Propietarios:** Nexus Grupo de Trabajo Principal, Grupo de Trabajo de Gobernanza  
> **Hoja de ruta de referencia:** NX-1 en `roadmap.md`

Esta página del portal refleja el breve canonique `docs/source/nexus_lanes.md` según los operadores Sora Nexus, los propietarios del SDK y los lectores pueden leer la guía de carriles sin explorador en el árbol monorrepo. La arquitectura puede garantizar el estado mundial determinante para permitir espacios de datos (carriles) que ejecuten conjuntos de validadores públicos o privados con cargas de trabajo aisladas.

## Conceptos- **Carril:** fragmento logístico del libro mayor Nexus con su propio conjunto de validadores y trabajos pendientes de ejecución. Identificar par un `LaneId` estable.
- **Espacio de datos:** grupo de gobierno que reagrupa uno o más carriles que participan en las políticas de cumplimiento, enrutamiento y liquidación.
- **Manifiesto de carril:** metadatos controlados por validadores decrivantes de gobierno, política DA, token de gas, reglas de liquidación y permisos de enrutamiento.
- **Compromiso global:** preuve emise par une lane qui resume les nouveaux root d etat, les donnees de asentamiento et les transferts cross-lane optionnels. L anneau NPoS global ordonne les engagements.

## Taxonomía de carriles

Los tipos de carril decrivent canoniquement leur visibilite, superficie de gobierno y ganchos de asentamiento. La geometría de configuración (`LaneConfig`) captura estos atributos según los nodos, SDK y herramientas que se pueden utilizar en el diseño sin lógica de medida.| Tipo de carril | Visibilidad | Membresía de validadores | Exposición WSV | Gobernanza por defecto | Política de solución | Uso típico |
|-----------|------------|----------------------|--------------|--------------------|-------------------|-------------|
| `default_public` | público | Sin permiso (participación global) | Réplica del estado completa | SORA Parlamento | `xor_global` | Libro mayor público de base |
| `public_custom` | público | Sin permiso o con participación | Réplica del estado completa | Módulo ponderado por apuesta | `xor_lane_weighted` | Aplicaciones públicas de alto rendimiento |
| `private_permissioned` | restringido | Conjunto fijo de validadores (aprobado por gobierno) | Compromisos y pruebas | Consejo federado | `xor_hosted_custody` | CBDC, cargas de trabajo del consorcio |
| `hybrid_confidential` | restringido | Membresía mixta; enrobe des pruebas ZK | Compromisos + divulgación selectiva | Módulo de bolsillo programable | `xor_dual_fund` | Monnaie programable preservante de la privacidad |

Todos los tipos de carril deben declarar:

- Alias de dataspace - reagrupamiento lisible par humains liant les politiques de cumplimiento.
- Manejar de gobierno - resolución de identificación vía `Nexus.governance.modules`.
- Manejar la liquidación: identifica el enrutador de liquidación para debitar los buffers XOR.
- Metadatos de telemetría opcionales (descripción, contacto, dominio comercial) expuestos a través de `/status` y paneles de control.## Geometría de configuración de carriles (`LaneConfig`)

`LaneConfig` es el tiempo de ejecución de geometría derivado del catálogo de líneas válidas. Il ne remplace pas les manifests de gouvernance; El dispositivo dispone de identificadores de almacenamiento determinados y sugerencias de telemetría para cada carril configurado.

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

- `LaneConfig::from_catalog` recalcula la geometría cuando la configuración está cargada (`State::set_nexus`).
- Los alias sont sanitises en slugs en minuscules; les caracteres no alfanuméricos consecutivos se comprimen en `_`. Si el alias produit un slug vide, en revient a `lane{id}`.
- `shard_id` deriva de la clave de metadatos `da_shard_id` (por defecto, `lane_id`) y controla el diario de cursor de fragmento persiste para guardar la relectura DA que determina entre redemarrages/resharding.
- Los prefijos de cle garantissent que le WSV maintient des plages de cles par lane disjointes meme si le backend est partage.
- Los nombres de los segmentos Kura son determinados entre los hosts; Los auditores pueden verificar los repertorios de segmento y los manifiestos sin herramientas sobre la medida.
- Les segments merge (`lane_{id:03}_merge`) almacenan las últimas raíces de merge-hint et compromisos de estado global para este carril.

## Partición del estado mundial- Le world state logique de Nexus est l union des espaces d etat par lane. Les lanes publiques persistentes l etat complet; les lanes privees/confidential exportent desroots Merkle/commitment vers le merge ledger.
- Le stockage MV prefixe chaque cle avec le prefixe 4 octets de `LaneConfigEntry::key_prefix`, produisant des cles comme `[00 00 00 01] ++ PackedKey`.
- Las tablas de participación (cuentas, activos, activadores, registros de gobierno) almacenan las entradas agrupadas por prefijo de carril, según los escaneos de rango determinados.
- Los metadatos del merge-ledger reflejan el diseño del meme: cada carril escribe las raíces de merge-hint y las raíces del estado global reduit en `lane_{id:03}_merge`, lo que permite una retención o desalojo cuando un carril se retira.
- Los índices entre carriles (alias de cuentas, registros de activos, manifiestos de gobierno) almacenan prefijos de carril explícitos para que los operadores reconcilien rápidamente las entradas.
- **Politique de retención** - les lanes publiques conservan les block body complets; Los compromisos de los carriles sólo pueden compactar los cuerpos antiguos después de los puntos de control. Los compromisos son autoridades. Les lanes confidencial gardent des journals chiffres dans des segments dedies pour ne pas bloquer d otras cargas de trabajo.- **Herramientas**: los utilitarios de mantenimiento (`kagami`, comandos CLI de administrador) deben hacer referencia al espacio de nombres con slug durante la exposición de métricas, etiquetas Prometheus o archivar segmentos Kura.

## Enrutamiento y API

- Los puntos finales Torii REST/gRPC aceptan la opción `lane_id`; l ausencia implique `lane_default`.
- Los SDK exponen los selectores de carriles y asignan los alias de conviviaux vers `LaneId` al utilizar el catálogo de carriles.
- Las reglas de enrutamiento operan en el catálogo válido y pueden elegir el carril y el espacio de datos. `LaneConfig` proporciona alias amigables para la telemetría en los paneles y registros.

## Liquidación y éxito

- Chaque lane paie des frais XOR au set global de validadores. Los carriles pueden cobrar tokens de gas natural pero deben depositar en garantía los equivalentes XOR con los compromisos.
- Las pruebas de liquidación incluyen el montaje, los metadatos de conversión y la preuve d escrow (por ejemplo, transferencia a la bóveda global de frais).
- El enrutador unificado de liquidación (NX-3) carga los buffers al utilizar los prefijos de carril, donde la telemetría de liquidación se alinea con la geometría de almacenamiento.

## Gobernanza- Les lanes declarant leur module de gouvernance via le catalogue. `LaneConfigEntry` porte el alias y el slug d origine para guardar la telemetría y los registros de auditoría lisibles.
- El registro Nexus distribuye los carriles de signos de manifiesto que incluyen `LaneId`, enlace de espacio de datos, identificador de gobierno, identificador de liquidación y metadatos.
- Los ganchos de actualización del tiempo de ejecución continúan aplicando las políticas de gobierno (`gov_upgrade_id` por defecto) y publicando las diferencias a través del puente de telemetría (eventos `nexus.config.diff`).

## Telemetría y estado

- `/status` expone los alias de carril, enlaces de espacio de datos, identificadores de gobierno y perfiles de liquidación, derivados del catálogo y de `LaneConfig`.
- Las métricas del planificador (`nexus_scheduler_lane_teu_*`) muestran alias/slugs según los operadores que pueden mapear rápidamente el backlog y la presión TEU.
- `nexus_lane_configured_total` cuenta el nombre de las entradas de carril derivadas y vuelve a calcular el cambio de configuración. La telemetría emite señales diferentes cuando cambia la geometría de las líneas.
- Los indicadores del espacio de datos atrasado incluyen el alias/descripción de metadatos para ayudar a los operadores a asociar la presión de archivos en los dominios comerciales.

## Configuración y tipos Norito- `LaneCatalog`, `LaneConfig` y `DataSpaceCatalog` viven en `iroha_data_model::nexus` y proporcionan estructuras en formato Norito para manifiestos y SDK.
- `LaneConfig` junto con `iroha_config::parameters::actual::Nexus` y se deriva automáticamente del catálogo; No es necesario codificar Norito para que haya un asistente de ejecución interno.
- La configuración del usuario (`iroha_config::parameters::user::Nexus`) continúa aceptando descriptores declarativos de carril y espacio de datos; El análisis deriva el mantenimiento de la geometría y rechaza los alias inválidos o los ID de carriles duplicados.

## Restante del trabajo de parto

- Integre las actualizaciones del enrutador de liquidación (NX-3) con la nueva geometría después de que los débitos y registros del buffer XOR sean etiquetas según el carril.
- Utilice el administrador de herramientas para enumerar las familias de columnas, compactar los carriles retirados e inspeccionar los registros de bloques por carril a través del espacio de nombres.
- Finalizador del algoritmo de fusión (ordenamiento, poda, detección de conflictos) y adjunto de elementos de regresión para la reproducción entre carriles.
- Agregar ganchos de cumplimiento para listas blancas, listas negras y políticas de dinero programables (suivi sous NX-12).

---*Esta página continúa suivre les follow-ups NX-1 a medida que NX-2 jusqu a NX-18 atterrissent. Merci de remonter les questions ouvertes dans `roadmap.md` ou le tracker de gouvernance afin que le portail reste aline avec les docs canonices.*
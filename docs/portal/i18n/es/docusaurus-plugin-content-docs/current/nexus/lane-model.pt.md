---
lang: es
direction: ltr
source: docs/portal/docs/nexus/lane-model.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: modelo-carril-nexus
título: Modelo de carriles Nexus
descripción: Taxonomía lógica de carriles, geometría de configuración y registros de fusión de estado mundial para Sora Nexus.
---

# Modelo de carriles de Nexus y participación de WSV

> **Estado:** entregavel NX-1: taxonomía de carriles, geometría de configuración y diseño de almacenamiento prontos para implementar.  
> **Propietarios:** Nexus Grupo de Trabajo Principal, Grupo de Trabajo de Gobernanza  
> **Referencia de hoja de ruta:** NX-1 en `roadmap.md`

Esta página del portal muestra el breve canónico `docs/source/nexus_lanes.md` para que los operadores de Sora Nexus, los propietarios del SDK y los revisores puedan leer una guía de carriles sin entrar en el arvore mono-repo. Una arquitectura alvo mantem o determinista del estado mundial permite que los espacios de datos (carriles) individuales ejecuten conjuntos de validadores públicos o privados con cargas de trabajo aisladas.

## Engreimientos- **Lane:** fragmento lógico del libro mayor de Nexus con su propio conjunto de validadores y trabajos pendientes de ejecución. Identificado por un estavel `LaneId`.
- **Espacio de datos:** grupo de gobierno que agrupa uma o más líneas que comparten políticas de cumplimiento, enrutamiento y liquidación.
- **Lane Manifest:** metadatos controlados por la gobernanza que descreve validadores, política de DA, token de gas, registros de liquidación y permisos de enrutamiento.
- **Compromiso Global:** prueba emitida por uma línea que resume novos root de estado, datos de liquidación y transferencias cross-lane opcionales. O anel NPoS compromisos de ordena global.

## Taxonomía de carriles

Los tipos de carril descrevem de forma canónica sua visibilidad, superficie de gobierno y ganchos de asentamiento. La geometría de configuración (`LaneConfig`) captura estos atributos para que nos, SDK y herramientas puedan raciocinar sobre el diseño sin lógica a medida.| Tipo de carril | Visibilidad | Membresía de validadores | Exposicao WSV | Gobernanza por defecto | Política de solución | Uso típico |
|-----------|------------|----------------------|--------------|--------------------|-------------------|-------------|
| `default_public` | público | Sin permiso (participación global) | Réplica de estado completa | SORA Parlamento | `xor_global` | Base del libro mayor público |
| `public_custom` | público | Sin permiso o con participación | Réplica de estado completa | Módulo ponderado por estaca | `xor_lane_weighted` | Aplicaciones públicas de alto rendimiento |
| `private_permissioned` | restringido | Set fixo de validadores (aprovado pelagobernanza) | Compromisos y pruebas | Consejo federado | `xor_hosted_custody` | CBDC, cargas de trabajo de consorcio |
| `hybrid_confidential` | restringido | Membresía equivocada; implican pruebas ZK | Compromisos + divulgación selectiva | Módulo de programación dinámica | `xor_dual_fund` | Dinheiro programavel com preservacao de privacidade |

Todos los tipos de carriles deben declararse:

- Alias de dataspace - agrupamiento legal por humanos que vincula políticas de cumplimiento.
- Handle degobernanza - identificador resuelto vía `Nexus.governance.modules`.
- Handle de liquidación - identificador consumido por el enrutador de liquidación para debitar buffers XOR.
- Metadatos opcionales de telemetría (descricao, contacto, dominio de negocio) expuestos vía `/status` y paneles de control.## Geometría de configuración de carriles (`LaneConfig`)

`LaneConfig` y un tiempo de ejecución de geometría derivada del catálogo de carriles validados. Ele nao substitui os manifests degobernanca; em vez disso fornece identificadores de almacenamiento deterministas e dicas de telemetría para cada carril configurado.

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

- `LaneConfig::from_catalog` recalcula la geometría cuando configura y carga (`State::set_nexus`).
- Alias ​​sao sanitizados em slugs minusculos; caracteres alfanuméricos consecutivos colapsam em `_`. Se o alias produzir um slug vazio, usamos `lane{id}`.
- `shard_id` y derivado de la clave de metadatos `da_shard_id` (predeterminado `lane_id`) y dirige el diario de cursor de fragmento persistente para mantener o reproducir el determinista DA entre reinicios/resharding.
- Prefijos de chave garantem que o WSV mantiene rangos de chave por carril disjuntos mesmo quando o mesmo backend y compartilhado.
- Nombres de segmentos Kura sao deterministas entre hosts; Los auditores pueden verificar los directorios de segmento y los manifiestos sin herramientas a medida.
- Segmentos de merge (`lane_{id:03}_merge`) guardam as ultimasroots de merge-hint e engagements de estado global para aquela lane.

## Partición del estado mundial- O world state logico do Nexus e a uniao de espacios de estado por carril. Lanes publicas persisten estado completo; carriles privados/confidential exportam root Merkle/commitment para o merge ledger.
- El armazenamento MV prefijo cada chave com o prefijo de 4 bytes de `LaneConfigEntry::key_prefix`, gerando chaves como `[00 00 00 01] ++ PackedKey`.
- Tabelas compartilhadas (cuentas, activos, activadores, registros de gobierno) armazenam entradas agrupadas por prefijo de carril, manteniendo los escaneos de rango deterministas.
- Los metadatos del libro mayor de fusión reflejan el diseño del mesmo: cada carril elimina las raíces de la fusión y las raíces de estado global reduzido en `lane_{id:03}_merge`, lo que permite la retención o el desalojo dirigido cuando se retira un carril.
- Índices entre carriles (alias de cuentas, registros de activos, manifiestos de gobernanza) armazenam prefixos de carril explícitos para que los operadores reconciliem entradas rápidamente.
- **Politica de retencao** - carriles públicos retencem corpos de bloco completos; carriles apenas de compromisos podem compactar corpos antigos apos checkpoints porque compromisos sao autoritativos. Lanes confidencial guardam diarios cifrados en segmentos dedicados para no bloquear otras cargas de trabajo.
- **Tooling** - Los utilitarios de mantenimiento (`kagami`, comandos admin de CLI) deben hacer referencia al espacio de nombres con barras para exportar métricas, etiquetas Prometheus o archivos de segmentos Kura.

## Enrutamiento y API- Puntos finales Torii REST/gRPC aceptados como `lane_id` opcional; ausencia implica `lane_default`.
- Los SDK exponen selectores de carriles y mapeiam alias amigaveis para `LaneId` usando el catálogo de carriles.
- Registros de operaciones de enrutamiento sobre el catálogo validado y podem escolher lane y dataspace. `LaneConfig` necesita alias amigos para telemetría en paneles y registros.

## Tarifas electrónicas de liquidación

- Cada carril paga tarifas XOR ao set global de validadores. Lanes podem coletar tokens de gas nativos, pero devem fazer escrow de equivalentes XOR junto con compromisos.
- Pruebas de liquidación que incluyen montante, metadatos de conversación y prueba de depósito en garantía (por ejemplo, transferencia para la bóveda global de tarifas).
- Los buffers de débito del enrutador de liquidación unificado (NX-3) usando los mismos prefijos de carril, así como la telemetría de liquidación se combinan con la geometría de almacenamiento.

## Gobernanza

- Los carriles declaran su módulo de gobierno vía catálogo. `LaneConfigEntry` carga el alias y el slug original para mantener la telemetría y los registros de auditoría legales.
- El registro de Nexus manifiesta la línea de distribución assinados que incluyen `LaneId`, enlace de espacio de datos, identificador de gobierno, identificador de liquidación y metadatos.
- Ganchos de actualización de tiempo de ejecución continua aplicando políticas de gobierno (`gov_upgrade_id` por defecto) y diferencias de registro a través del puente de telemetría (eventos `nexus.config.diff`).

## Telemetría y estado- `/status` expone alias de carril, enlaces de espacio de datos, identificadores de gobierno y pérdidas de liquidación, derivados del catálogo y de `LaneConfig`.
- Métricas del planificador (`nexus_scheduler_lane_teu_*`) exibem alias/slugs para que los operadores mapeiem backlog y presionen de TEU rápidamente.
- `nexus_lane_configured_total` cuenta con el número de entradas de carril derivadas y se recalcula cuando se cambia la configuración. La telemetría emite diferencias assinadas cuando la geometría de carriles se muda.
- Los indicadores de backlog de dataspace incluyen metadatos de alias/descricao para ayudar a los operadores a asociar pressao de fila a dominios de negocio.

## Configuración y tipos Norito

- `LaneCatalog`, `LaneConfig`, e `DataSpaceCatalog` viven en `iroha_data_model::nexus` y necesitan estructuras comparativas con Norito para manifiestos y SDK.
- `LaneConfig` vive en `iroha_config::parameters::actual::Nexus` y derivado automáticamente del catálogo; No requer codificación Norito porque es un asistente interno en tiempo de ejecución.
- La configuración voltada del usuario (`iroha_config::parameters::user::Nexus`) continúa aceitando descripciones declarativas de carril y espacio de datos; El análisis ahora deriva geometría y elimina alias inválidos o ID de carril duplicados.

## Trabajo pendiente- Integrar actualizaciones del enrutador de liquidación (NX-3) con una nueva geometría para que los débitos y recibos de buffer XOR se dejen etiquetados por slug de lane.
- Utilice el administrador de herramientas para listar familias de columnas, compactar carriles aposentados e inspeccionar registros de bloques por carril usando el espacio de nombres con slug.
- Finalizar el algoritmo de fusión (ordenamiento, poda, detección de conflictos) y anexar elementos de regreso para reproducir cross-lane.
- Agregar ganchos de cumplimiento para listas blancas/negras y políticas de dinero programático (acompañado de NX-12).

---

*Esta página continuará rastreando seguimientos de NX-1 conforme NX-2 ate NX-18 aterrissarem. Por favor traga perguntas em aberto para `roadmap.md` ou o tracker degobernanca para que o portal fique alinhado com os docs canonicos.*
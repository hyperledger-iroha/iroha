---
lang: es
direction: ltr
source: docs/portal/docs/nexus/lane-model.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: modelo-carril-nexus
título: Модель carriles Nexus
descripción: Логическая таксономия lanes, geометрия конфигурации и правила слияния world-state для Sora Nexus.
---

# Líneas de modelo Nexus y particiones WSV

> **Status:** поставка NX-1 - таксономия lanes, геометрия конфигурации и раскладка хранения готовы к внедрению.  
> **Propietarios:** Nexus Grupo de Trabajo Principal, Grupo de Trabajo de Gobernanza  
> **Ссылка en hoja de ruta:** NX-1 en `roadmap.md`

Este sitio web contiene un resumen canónico `docs/source/nexus_lanes.md`, los operadores de Sora Nexus, el SDK de los propietarios y los principales proveedores. руководство по lanes без погружения в дерево mono-repo. El arquitecto alemán determina el estado mundial, por lo que existen otros espacios de datos (carriles) que se publican o se publican. validadores de cargas de trabajo aisladas.

## Conceptos- **Lane:** el libro mayor de fragmentos logísticos Nexus para que se realicen validaciones y trabajos pendientes. Идентифицируется стабильным `LaneId`.
- **Espacio de datos:** depósito de gobernanza, carriles de gestión de cumplimiento de políticas, enrutamiento y liquidación de objetivos.
- **Manifiesto de carril:** metadatos de control de gobernanza, validadores de opiniones, política de DA, token de gas, permisos de enrutamiento y liquidación parcial.
- **Compromiso global:** prueba, carril выпускаемая y суммирующая новые raíces estatales, datos de liquidación y transferencias entre carriles opcionales. Глобальное кольцо NPoS упорядочивает compromisos.

## carriles de taxonomía

Tipos de carriles que describen el paisaje, superficie de gobernanza y ganchos de liquidación. Las configuraciones geométricas (`LaneConfig`) incluyen estos atributos, aplicaciones, SDK y herramientas que permiten diseñar diseños con lógicas personalizadas.| Carril tip | Видимость | Membresía валидаторов | Экспозиция WSV | Gobernanza по умолчанию | Acuerdo político | Типичное применение |
|-----------|------------|----------------------|--------------|--------------------|-------------------|-------------|
| `default_public` | público | Sin permiso (participación global) | Réplica de estado completo | SORA Parlamento | `xor_global` | Libro mayor público |
| `public_custom` | público | Sin permiso o controlado por participación | Réplica de estado completo | Módulo ponderado por participación | `xor_lane_weighted` | Rendimiento de las aplicaciones públicas con anuncios publicitarios |
| `private_permissioned` | restringido | Фиксированный набор валидаторов (gobernanza del gobierno) | Compromisos y pruebas | Consejo federado | `xor_hosted_custody` | CBDC, cargas de trabajo masivas |
| `hybrid_confidential` | restringido | Membresía Смешанная; pruebas ZK | Compromisos + divulgación selectiva | Módulo de dinero programable | `xor_dual_fund` | Dinero programable que preserva la privacidad |

Все типы carril обязаны объявлять:

- Alias del espacio de datos: cumplimiento de las políticas de человекочитаемая группировка.
- Controlador de gobernanza - идентификатор, разрешаемый через `Nexus.governance.modules`.
- Identificador de liquidación: identificador, enrutador de liquidación popular para la descripción de buffers XOR.
- Metadatos de telemetría opcionales (descripción, contacto, dominio comercial), información sobre `/status` y paneles de control.## Líneas de configuración geométrica (`LaneConfig`)

`LaneConfig` - geometría de tiempo de ejecución, выводимая из валидированного каталога lanes. Она не заменяет manifiestos de gobernanza; Todo esto incluye identificadores de almacenamiento determinados y sugerencias de telemetría para el carril de configuración.

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

- `LaneConfig::from_catalog` пересчитывает геометрию при загрузке конфигурации (`State::set_nexus`).
- Alias ​​санитизируются в нижний регистр slug; последовательности неалфавитно-цифровых символов схлопываются в `_`. Otro alias de esta babosa es `lane{id}`.
- `shard_id` incluye la clave de metadatos `da_shard_id` (junto con `lane_id`) y actualiza el cursor de fragmentos local, la reproducción de DA está disponible детерминированным между reinicia/reharding.
- Los prefijos de claves están garantizados, ya que WSV proporciona rangos de claves diferentes en el carril, así como el backend.
- Имена Kura segmento детерминированы между hosts; Los auditores pueden componer directorios de segmentos y manifiestos con herramientas personalizadas.
- Fusionar segmentos (`lane_{id:03}_merge`) muestra raíces de sugerencias de fusión y compromisos estatales globales en este carril.

## Партиционирование estado mundial- Estado mundial lógico Nexus - Estos espacios de estado observados en el carril. Carriles públicos сохраняют estado completo; carriles privados/confidenciales экспортируют Merkle/raíces de compromiso en el libro mayor de fusión.
- Almacenamiento MV префиксирует каждый ключ 4-байтовым префиксом `LaneConfigEntry::key_prefix`, получая ключи вида `[00 00 00 01] ++ PackedKey`.
- Tablas compartidas (cuentas, activos, activadores, registros de gobierno) хранят записи, сгруппированные по lane prefix, сохраняя детерминизм range scans.
- Los metadatos del libro mayor de fusión se utilizan en el diseño: el carril incluye raíces de sugerencias de fusión y raíces de estado global reducidas en `lane_{id:03}_merge`, que puede eliminar la retención/desalojo dirigidos en el carril.
- Índices entre carriles (alias de cuentas, registros de activos, manifiestos de gobernanza) que permiten la conciliación de prefijos de carriles.
- **Política de retención** - carriles públicos сохраняют полные cuerpos de bloque; Los carriles de solo compromiso pueden competir con organismos después de los puntos de control, así como con compromisos autorizados. Los carriles confidenciales contienen diarios de texto cifrado en segmentos muy diversos, que no bloquean cargas de trabajo excesivas.
- **Herramientas** - Utilidades de configuración (`kagami`, comandos de administrador CLI) que se adaptan al espacio de nombres slugged con métricas específicas, etiquetas Prometheus o архивировании segmentos de Kura.

## Enrutamiento y API- Puntos finales Torii REST/gRPC принимают опциональный `lane_id`; отсутствие означает `lane_default`.
- Los SDK incluyen selectores de carriles y asignan alias fáciles de usar en `LaneId` de carriles del catálogo.
- Las reglas de enrutamiento se adaptan al catálogo válido y se pueden conectar a carriles y espacios de datos. `LaneConfig` proporciona alias compatibles con telemetría para paneles y registros.

## Liquidación y tarifas

- Каждая lane платит XOR fee global набору валидаторов. Los carriles pueden adquirir tokens de gas nativo, sin equivalentes XOR de depósito en garantía junto con compromisos.
- Pruebas de liquidación del monto del monto, metadatos de conversión y depósito de prueba (por ejemplo, перевод в глобальный bóveda de tarifas).
- El enrutador de liquidación moderno (NX-3) permite almacenar en buffers los prefijos de carril y la telemetría de liquidación se combina con la geometría de almacenamiento.

## Gobernanza

- Carriles объявляют свой módulo de gobernanza через каталог. `LaneConfigEntry` no necesita alias ni slug originales, ni telemetría ni registros de auditoría establecidos.
- El registro Nexus combina los manifiestos de carril, incluye `LaneId`, enlace de espacio de datos, identificador de gobernanza, identificador de liquidación y metadatos.
- Los ganchos de actualización del tiempo de ejecución implementan políticas de gobernanza (`gov_upgrade_id` para el usuario) y registran diferencias en el puente de telemetría (eventos `nexus.config.diff`).

## Telemetría y estado- `/status` incluye alias de carriles, enlaces de espacios de datos, identificadores de gobierno y perfiles de liquidación, registros de catálogos e `LaneConfig`.
- Métricas del programador (`nexus_scheduler_lane_teu_*`) que incluyen alias/slugs, retrasos en el trabajo de los operadores y presión de TEU.
- `nexus_lane_configured_total` считает количество выведенных entradas de carril y пересчитывается при изменении конфигурации. La telemetría permite diferencias según la geometría del carril.
- El backlog del espacio de datos mide los metadatos de alias/descripción, lo que ayuda a que el operador evalúe la presión de la cola en los hogares de negocios.

## Configuración y tipos Norito

- `LaneCatalog`, `LaneConfig`, y `DataSpaceCatalog` conectados a `iroha_data_model::nexus` y anteriores a Norito. Estructuras de manifiestos y SDK.
- `LaneConfig` se conecta a `iroha_config::parameters::actual::Nexus` y se actualizan automáticamente en el catálogo; La codificación Norito no es un problema, ya que es este asistente de tiempo de ejecución exclusivo.
- Configuración de configuración avanzada (`iroha_config::parameters::user::Nexus`) que permite definir la línea de descriptores y el espacio de datos; El análisis de los nombres geométricos y la eliminación de alias no válidos o de ID de carril duplicados.

## Оставшаяся работа- Integrar actualizaciones del enrutador de liquidación (NX-3) con nueva geometría, con débitos y recibos del búfer XOR con slug lane.
- Herramientas de administración mejoradas para familias de columnas específicas, carriles retirados compactos y registros de bloques por carril para espacios de nombres reducidos.
- Finalizar el algoritmo de fusión (ordenamiento, poda, detección de conflictos) y agregar elementos de regresión para la repetición entre carriles.
- Incorporar ganchos de cumplimiento para listas blancas/negras y políticas de dinero programables (трекится под NX-12).

---

*Esta página permite seleccionar seguimientos de NX-1 junto con NX-2 y NX-18. Es posible que pueda registrar datos en `roadmap.md` o en el rastreador de gobernanza, que es un portal portátil de sincronización con canales documentos.*
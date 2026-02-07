---
lang: es
direction: ltr
source: docs/portal/docs/nexus/nexus-refactor-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plan-refactor-nexus
título: Planificador Sora Nexus Ledger
descripción: Зеркало `docs/source/nexus_refactor_plan.md`, описывающее поэтапную зачистку кодовой базы Iroha 3.
---

:::nota Канонический источник
Esta página es `docs/source/nexus_refactor_plan.md`. Deje copias sincronizadas, ya que muchas versiones no se pueden publicar en el portal.
:::

# Plan de refaccion Sora Nexus Ledger

Este documento tiene una hoja de ruta completa para el refactor de Sora Nexus Ledger ("Iroha 3"). Según la estructura de la estructura del repositorio y la regresión, se encuentran en genesis/WSV, según el consenso Sumeragi, activadores смарт-контрактов, consultas de instantáneas, puntero de enlaces de host-ABI y códecs Norito. Цель - прийти когласованной, тестируемой архитектуре, не пытаясь посадить все исправления одним монолитным патчем.## 0. Направляющие принципы
- Сохранять детерминированное поведение на гетерогенном железе; Utilice opciones para seleccionar indicadores de funciones de inclusión y respaldos idénticos.
- Norito - слой сериализации. Любые изменения state/schema должны включать Norito codifica/decodifica pruebas de ida y vuelta y обновления accesorios.
- Конфигурация проходит через `iroha_config` (usuario -> real -> valores predeterminados). Utilice el entorno ad-hoc para alternar entre los usuarios.
- La política ABI está instalada en V1 y no está disponible. Los hosts должны детерминированно отклонять неизвестные tipos de puntero/llamadas al sistema.
- `cargo test --workspace` y pruebas doradas (`ivm`, `norito`, `integration_tests`) configuran bases de datos para la etapa del código.## 1. Снимок топоLOGии репозитория
- `crates/iroha_core`: actores Sumeragi, WSV, cargador de génesis, canalizaciones (consulta, superposición, carriles zk), pegamento para el host de contrato inteligente.
- `crates/iroha_data_model`: consultas y consultas automáticas en cadena.
- `crates/iroha`: API de cliente, CLI independiente, pruebas, SDK.
- `crates/iroha_cli`: CLI del operador, varias API seleccionadas en `iroha`.
- `crates/ivm`: máquina virtual con código de bytes Kotodama, puntos de entrada para la integración del host puntero-ABI.
- `crates/norito`: códec de serialización con adaptadores JSON y backends AoS/NCB.
- `integration_tests`: aserciones de componentes cruzados, покрывающие genesis/bootstrap, Sumeragi, disparadores, paginación y т.д.
- Documentos de descripción de Sora Nexus Ledger (`nexus.md`, `new_pipeline.md`, `ivm.md`), sin fragmentación ni realización частично устарела относительно кода.

## 2. Опорные элементы рефакторинга и вехи### Fase A - Fundamentos y observabilidad
1. **Telemetría WSV + Instantáneas**
   - Actualizar la API de instantáneas canónicas en `state` (rasgo `WorldStateSnapshot`), consultas individuales, Sumeragi y CLI.
   - Utilice `scripts/iroha_state_dump.sh` para determinar instantáneas desde `iroha state dump --format norito`.
2. **Génesis/Determinismo Bootstrap**
   - Pruebe la ingesta de génesis de la tubería, que se encuentra en el proceso de la tubería Norito (`iroha_core::genesis`).
   - Incorporación de integración/regresión, combinación de génesis más bloque pervertido y potencia de raíces WSV idénticas con arm64/x86_64 (трекится в `integration_tests/tests/genesis_replay_determinism.rs`).
3. **Pruebas de fijación entre cajas**
   - Utilice `integration_tests/tests/genesis_json.rs` para validar WSV, invariantes de tubería y ABI en un arnés diferente.
   - El andamio `cargo xtask check-shape`, un problema relacionado con la deriva del esquema (en el trabajo pendiente de herramientas DevEx; junto con el elemento de acción en `scripts/xtask/README.md`).### Fase B: WSV y superficie de consulta
1. **Transacciones de almacenamiento estatal**
   - Свести `state/storage_transactions.rs` к транзакционному адаптеру, который обеспечивает порядок commit y detector de conflictos.
   - Las pruebas unitarias están disponibles para modificar los activos/mundo/disparadores.
2. **Refactorización del modelo de consulta**
   - Paginación permanente/lógica de cursor en componentes permanentes bajo `crates/iroha_core/src/query/`. Coloque las representaciones Norito en `iroha_data_model`.
   - Agregar consultas de instantáneas para activadores, activos y roles según parámetros predeterminados (por ejemplo `crates/iroha_core/tests/snapshot_iterable.rs` para una cuenta de seguridad).
3. **Coherencia de las instantáneas**
   - Tenga en cuenta que la CLI `iroha ledger query` utiliza la ruta de la instantánea y Sumeragi/fetchers.
   - Pruebas de regresión de instantáneas CLI incorporadas en `tests/cli/state_snapshot.rs` (con acceso a funciones para programas avanzados).### Fase C - Tubería Sumeragi
1. **Topología y gestión de época**
   - Вынести `EpochRosterProvider` en feature с реализациями, основанными на WSV stake snapshots.
   - `WsvEpochRosterAdapter::from_peer_iter` es un constructor anterior, utilizado para simulacros en bancos/pruebas.
2. **Simplificación del flujo de consenso**
   - Registre `crates/iroha_core/src/sumeragi/*` en módulos: `pacemaker`, `aggregation`, `availability`, `witness` con tipos de parámetros `consensus`.
   - Mejora el paso de mensajes ad-hoc en sobres típicos Norito y realiza pruebas de propiedades de cambio de vista (trabaja en el trabajo pendiente de mensajería Sumeragi).
3. **Integración de carril/prueba**
   - Согласовать pruebas de carril con compromisos DA y обеспечить единообразный RBC gating.
   - Prueba integral de extremo a extremo `integration_tests/tests/extra_functional/seven_peer_consistency.rs` que prueba la conexión con RBC completo.### Fase D: Contratos inteligentes y hosts Pointer-ABI
1. **Auditoría de límites del anfitrión**
   - Configuración de adaptadores de tipo puntero (`ivm::pointer_abi`) y adaptadores de host (`iroha_core::smartcontracts::ivm::host`).
   - Tabla de punteros actualizada y enlaces de manifiesto de host que contienen los archivos `crates/iroha_core/tests/ivm_pointer_abi_tlv_types.rs` e `ivm_host_mapping.rs`, que proporcionan asignaciones de TLV dorado.
2. **Disparador de zona de pruebas de ejecución**
   - Activadores de configuración, dispositivos de control de velocidad `TriggerExecutor`, gas primario, validación de puntero y registro de eventos.
   - Se incluyen pruebas de regresión para activadores de llamada/tiempo en rutas de error de búsqueda (véase `crates/iroha_core/tests/trigger_failure.rs`).
3. **CLI y alineación del cliente**
   - Observe las operaciones de CLI (`audit`, `gov`, `sumeragi`, `ivm`) que implementan funciones de cliente `iroha`, esto indica deriva.
   - Pruebas de instantáneas CLI JSON realizadas en `tests/cli/json_snapshot.rs`; Además, actualmente, todos los comandos principales se combinan con la referencia JSON canónica.### Fase E: endurecimiento del códec Norito
1. **Registro de esquema**
   - Utilice el registro de esquema Norito en `crates/norito/src/schema/`, para eliminar codificaciones canónicas de los tipos de datos principales.
   - Pruebas de documentación completas, prueba de cargas útiles de muestra de codificación (`norito::schema::SamplePayload`).
2. **Actualización de accesorios dorados**
   - Eliminación de accesorios dorados en `crates/norito/tests/*`, que incluye un nuevo esquema WSV después de la refracción.
   - `scripts/norito_regen.sh` regenerador predeterminado Norito JSON goldens como ayudante `norito_regen_goldens`.
3. **Integración IVM/Norito**
   - Провалидировать сериализацию Kotodama manifiesto de extremo a extremo через Norito, garantizando metadatos ABI de puntero consistentes.
   - `crates/ivm/tests/manifest_roundtrip.rs` utiliza Norito para codificar/decodificar partes de manifiestos.## 3. Сквозные вопросы
- **Estrategia de prueba**: Каждый этап продвигает pruebas unitarias -> pruebas de cajas -> pruebas de integración. Падающие тесты фиксируют текущие регрессии; Nuevas pruebas no son necesarias.
- **Documentación**: После посадки каждой фазы обновлять `status.md` и переносить незакрытые пункты в `roadmap.md`, удаляя завершенные задачи.
- **Parámetros de rendimiento**: Сохранять существующие bancos en `iroha_core`, `ivm` y `norito`; Después de una refracción, puede realizar una regresión excesiva.
- **Marcas de funciones**: Instala cambios a nivel de caja para backends y tres cadenas de herramientas (`cuda`, `zk-verify-batch`). CPU SIMD пути всегда собираются и выбираются во время выполнения; обеспечить детерминированные retrocesos escalares для неподдерживаемого железа.## 4. Ближайшие действия
- Andamio de fase A (rasgo instantáneo + cableado de telemetría) - см. Tareas procesables en la hoja de ruta actualizada.
- Los efectos de auditoría necesarios para `sumeragi`, `state` y `ivm` son los siguientes:
  - `sumeragi`: asignaciones de código muerto que incluyen prueba de cambio de vista de transmisión, estado de reproducción VRF y exportación de telemetría EMA. Aunque está cerrado, no hay muchos entregables de la Fase C para mejorar el flujo de consenso y la integración de carril/prueba.
  - `state`: `Cell` período de limpieza y enrutamiento de telemetría en la pista de telemetría WSV de la Fase A, y un seguimiento de SoA/aplicación paralela en el trabajo pendiente de optimización de canalización de la Fase C.
  - `ivm`: alternancia de funciones CUDA, validación de envolvente y cobertura de Halo2/Metal asignadas a la fase D del límite del host, además de aceleración de GPU del tema exclusivo; Los kernels se instalan en el backlog de GPU de la actualidad.
- Puede enviar RFC entre equipos con este plan de cierre de sesión antes de realizar un código personalizado.

## 5. Открытые вопросы
- ¿Si RBC está instalado de forma opcional en P1 o está instalado en los carriles del libro mayor Nexus? Требуется решение стейкхолдеров.
- ¿Hay grupos de componibilidad DS en P1 o hay otros grupos de pruebas de carril?
- ¿Qué cantidad de archivos canónicos hay para los parámetros ML-DSA-87? Кандидат: новый crate `crates/fastpq_isi` (создание ожидается).

---

_Nueva actualización: 2025-09-12_
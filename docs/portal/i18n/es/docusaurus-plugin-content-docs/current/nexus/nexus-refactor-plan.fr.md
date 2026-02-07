---
lang: es
direction: ltr
source: docs/portal/docs/nexus/nexus-refactor-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plan-refactor-nexus
título: Plan de refactorización del libro mayor Sora Nexus
descripción: Espejo de `docs/source/nexus_refactor_plan.md`, detalla el trabajo de limpieza por fases para la base del código Iroha 3.
---

:::nota Fuente canónica
Esta página refleja `docs/source/nexus_refactor_plan.md`. Gardez les dos copias alineadas justo ce que la edición multilingue llega al portal.
:::

# Plan de refactorización del libro mayor Sora Nexus

Este documento captura la hoja de ruta inmediata del refactor de Sora Nexus Ledger ("Iroha 3"). Refleja la topología actual del depósito y las regresiones observadas en la compatibilidad génesis/WSV, el consenso Sumeragi, los activadores de contratos inteligentes, las solicitudes de instantáneas, las vinculaciones del puntero de host-ABI y los códecs Norito. El objetivo es converger con una arquitectura coherente y comprobable sin necesidad de guardar todas las correcciones en un parche monolítico.## 0. Principios directores
- Preservar un comportamiento determinante sobre el material heterogéneo; Utilice la aceleración única a través de la opción de inclusión de indicadores de funciones con respaldos idénticos.
- Norito es el sofá de serialización. Todo cambio de estado/esquema debe incluir las pruebas de codificación/decodificación de ida y vuelta Norito y las actualizaciones del día.
- La configuración transita por `iroha_config` (usuario -> actual -> valores predeterminados). Suprima los interruptores ambientales ad hoc de las rutas de producción.
- La política ABI reste V1 y no negociable. Los hosts no deben rechazar la determinación de los tipos de puntero/llamadas al sistema inconnus.
- `cargo test --workspace` y las pruebas doradas (`ivm`, `norito`, `integration_tests`) descansan en la puerta de base para cada jalón.## 1. Instantánea de la topología del depósito
- `crates/iroha_core`: actores Sumeragi, WSV, génesis del cargador, canalizaciones (consulta, superposición, carriles zk), host de pegamento de contratos inteligentes.
- `crates/iroha_data_model`: esquema autorizado para donantes y solicitudes en cadena.
- `crates/iroha`: Cliente API utilizado por CLI, pruebas, SDK.
- `crates/iroha_cli`: operador CLI que refleja actualmente muchas API en `iroha`.
- `crates/ivm`: VM de código de bytes Kotodama, puntos de entrada del puntero de integración-ABI del host.
- `crates/norito`: códec de serialización con adaptadores JSON y backends AoS/NCB.
- `integration_tests`: afirmaciones entre componentes couvrant genesis/bootstrap, Sumeragi, activadores, paginación, etc.
- Los documentos descritos dejan los objetivos de Sora Nexus Ledger (`nexus.md`, `new_pipeline.md`, `ivm.md`), pero la implementación está fragmentada y parcialmente obsoleta según la relación con el código.

## 2. Piliers de refactor et jalons### Fase A - Fundaciones y observabilidad
1. **Telemetría WSV + Instantáneas**
   - Establecer una API canónica de instantáneas en `state` (rasgo `WorldStateSnapshot`) utilizada para consultas, Sumeragi y CLI.
   - Utilice `scripts/iroha_state_dump.sh` para producir instantáneas determinadas a través de `iroha state dump --format norito`.
2. **Determinismo Génesis/Bootstrap**
   - Refactorizador de la génesis de la ingestión para pasar por una base única de tubería en Norito (`iroha_core::genesis`).
   - Ajouter une couverture d'integration/regression qui rejoue genesis plus le premier block et verifie des racines WSV identiques entre arm64/x86_64 (suivi dans `integration_tests/tests/genesis_replay_determinism.rs`).
3. **Pruebas de fijación entre cajas**
   - Utilice `integration_tests/tests/genesis_json.rs` para validar los invariantes WSV, pipeline y ABI en un solo arnés.
   - Introduzca un andamio `cargo xtask check-shape` que pánico en la deriva del esquema (suivi dans le backlog DevEx tools; vea el elemento de acción en `scripts/xtask/README.md`).### Fase B - WSV y superficie de solicitudes
1. **Transacciones de almacenamiento estatal**
   - Collapser `state/storage_transactions.rs` es un adaptador de transacciones que aplica el orden de confirmación y la detección de conflictos.
   - Las pruebas unitarias verifican que las modificaciones de activos/mundo/triggers font rollback en caso de control.
2. **Refactor del modelo de solicitudes**
   - Reemplace la lógica de paginación/cursor en los componentes reutilizables bajo `crates/iroha_core/src/query/`. Alinee las representaciones Norito en `iroha_data_model`.
   - Agregar consultas de instantáneas para activadores, activos y roles con un orden determinado (suivi vía `crates/iroha_core/tests/snapshot_iterable.rs` para la cobertura actual).
3. **Consistencia de las instantáneas**
   - Asegúrese de que la CLI `iroha ledger query` utilice el meme chemin de snapshot que Sumeragi/fetchers.
   - Las pruebas de regresión de instantáneas CLI se encuentran en `tests/cli/state_snapshot.rs` (con funciones controladas para ejecutar lentes).### Fase C - Tubería Sumeragi
1. **Topología y gestión de épocas**
   - Extraire `EpochRosterProvider` con implementaciones basadas en instantáneas de participación WSV.
   - `WsvEpochRosterAdapter::from_peer_iter` proporciona un constructor simple y amigable para simulaciones en bancos/pruebas.
2. **Simplificación del flujo de consenso**
   - Reorganizador `crates/iroha_core/src/sumeragi/*` en módulos: `pacemaker`, `aggregation`, `availability`, `witness` con los tipos de particiones bajo `consensus`.
   - Reemplace el mensaje que pasa ad-hoc por los sobres tipo Norito e introduzca las pruebas de propiedad de cambio de vista (suivi dans le backlog message Sumeragi).
3. **Carril de integración/prueba**
   - Alinee las pruebas de carril con los compromisos DA y garantice una puerta uniforme de RBC.
   - La prueba de integración de extremo a extremo `integration_tests/tests/extra_functional/seven_peer_consistency.rs` verifica el mantenimiento del camino con RBC activo.### Fase D: contratos inteligentes y puntero de hosts-ABI
1. **Anfitrión de auditoría de la frontera**
   - Consolide las comprobaciones de tipo de puntero (`ivm::pointer_abi`) y los adaptadores de host (`iroha_core::smartcontracts::ivm::host`).
   - Los asistentes de la tabla de punteros y las vinculaciones del manifiesto del host están cubiertos por `crates/iroha_core/tests/ivm_pointer_abi_tlv_types.rs` e `ivm_host_mapping.rs`, que ejercen las asignaciones TLV golden.
2. **Sandbox de ejecución de activadores**
   - Refactorizador de activadores para pasar por un `TriggerExecutor` común que aplica gas, validación de punteros y registro de eventos.
   - Agregar pruebas de regresión para los activadores de llamada/tiempo que cubren los caminos de control (suivi vía `crates/iroha_core/tests/trigger_failure.rs`).
3. **Alineación CLI y cliente**
   - Asegúrese de que las operaciones CLI (`audit`, `gov`, `sumeragi`, `ivm`) se encuentren en las funciones del cliente partagees `iroha` para evitar la deriva.
   - Las pruebas de instantáneas JSON de CLI viven en `tests/cli/json_snapshot.rs`; Guarde el día para que la salida principal continúe correspondiendo a la referencia JSON canónica.### Fase E - Configuración del códec Norito
1. **Registro de esquemas**
   - Cree un registro de esquemas Norito en `crates/norito/src/schema/` para obtener las codificaciones canónicas de los tipos principales.
   - Agregar pruebas de documentos que verifican la codificación de cargas útiles del ejemplo (`norito::schema::SamplePayload`).
2. **Actualizar los accesorios dorados**
   - Mettre un día en los accesorios dorados `crates/norito/tests/*` para corresponder al nuevo esquema WSV una vez le refactor livre.
   - `scripts/norito_regen.sh` regenera el JSON dorado Norito de manera determinada a través del asistente `norito_regen_goldens`.
3. **Integración IVM/Norito**
   - Validar la serialización de manifiestos Kotodama de extremo a extremo a través de Norito, garantizando un puntero de metadatos ABI coherente.
   - `crates/ivm/tests/manifest_roundtrip.rs` mantiene la partición Norito codifica/decodifica para los manifiestos.## 3. Preocupaciones transversales
- **Estrategia de pruebas**: Pruebas unitarias de promesa de cada fase -> pruebas de cajas -> pruebas de integración. Les tests en echec capturen les regressions actuelles; les nouveaux tests evitant leur retour.
- **Documentación**: Después de cada fase, mettre a jour `status.md` et reporter les items ouverts dans `roadmap.md` tout en supprimant les taches terminees.
- **Parámetros de rendimiento**: Mantener los bancos existentes en `iroha_core`, `ivm` y `norito`; Agregue las medidas de base posteriores al refactor para validar la ausencia de regresiones.
- **Indicadores de funciones**: Conservar los conmutadores en el nivel de caja exclusivo para los backends que requieren cadenas de herramientas externas (`cuda`, `zk-verify-batch`). Los caminos SIMD CPU están siempre construidos y seleccionados en la ejecución; Fournir des fallbacks scalaires deterministes pour le material non supporte.## 4. Acciones inmediatas
- Scaffolding de la Phase A (rasgo instantáneo + telemetría de cableado) - vea los taches accionables en las actualizaciones del día de la hoja de ruta.
- La auditoría reciente de los valores predeterminados para `sumeragi`, `state` y `ivm` revela los puntos siguientes:
  - `sumeragi`: los permisos de código muerto protegen la transmisión de las preferencias de cambio de vista, el estado de reproducción VRF y la telemetría de exportación EMA. Ils restent gates jusqu'a ce que la simplificación del flujo de consenso de la Fase C et les livrables d'integration lane/proof soient livrees.
  - `state`: el nettoyage de `Cell` y la telemetría de ruta pasada por la telemetría de pista WSV de la Fase A, así como las notas SoA/parallel-apply basculent en el backlog de optimización de tubería de la Fase C.
  - `ivm`: la exposición del conmutador CUDA, la validación de sobres y la cobertura Halo2/Metal se asignan al límite del host de trabajo de la Fase D más el tema transversal de aceleración de la GPU; Los núcleos reposan en la acumulación de GPU deben madurar.
- Preparador de un resumen RFC entre equipos de este plan para aprobar antes de liberar los cambios de código invasivo.## 5. Preguntas abiertas
- ¿RBC tiene opciones adicionales después de P1, o es obligatorio para los carriles del libro mayor Nexus? Decisión des partes prenantes requise.
- ¿Doit-on imponer des groupes de composabilite DS en P1 ou les laisser desactives jusqu'a madurite des laneproofs?
- ¿Qué es la localización canónica de los parámetros ML-DSA-87? Candidato: caja nouveau `crates/fastpq_isi` (creación en attente).

---

_Última puesta en marcha del día: 2025-09-12_
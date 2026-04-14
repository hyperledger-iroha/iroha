<!-- Auto-generated stub for Spanish (es) translation. Replace this content with the full translation. -->

---
lang: es
direction: ltr
source: docs/source/kura_wsv_security_performance_audit_20260219.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 194721ce71f5593cc9e4df6313c6e3aa85c5c3dc0e3efe4a28d0ded968c0584a
source_last_modified: "2026-02-19T08:31:06.766140+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Auditoría de rendimiento y seguridad de Kura / WSV (2026-02-19)

## Alcance

Esta auditoría cubrió:

- Persistencia de Kura y rutas presupuestarias: `crates/iroha_core/src/kura.rs`
- Rutas de consulta/compromiso de estado/WSV de producción: `crates/iroha_core/src/state.rs`
- IVM Superficies de host simuladas de WSV (alcance de prueba/desarrollo): `crates/ivm/src/mock_wsv.rs`

Fuera de alcance: cajas no relacionadas y reposiciones de pruebas comparativas de todo el sistema.

## Resumen de riesgos

- Crítico: 0
- Alto: 4
- Mediano: 6
- Bajo: 2

## Hallazgos (ordenados por gravedad)

### Alto

1. **El escritor de Kura entra en pánico por las fallas de E/S (riesgo de disponibilidad del nodo)**
- Componente: Kura
- Tipo: Seguridad (DoS), Fiabilidad
- Detalle: el bucle de escritura entra en pánico ante errores de adición/indexación/fsync en lugar de devolver errores recuperables, por lo que las fallas transitorias del disco pueden terminar el proceso del nodo.
- Pruebas:
  - `crates/iroha_core/src/kura.rs:1697`
  - `crates/iroha_core/src/kura.rs:1724`
  - `crates/iroha_core/src/kura.rs:1845`
  - `crates/iroha_core/src/kura.rs:1854`
  - `crates/iroha_core/src/kura.rs:1860`
- Impacto: la carga remota + la presión del disco local pueden provocar bucles de bloqueo/reinicio.2. **El desalojo de Kura realiza reescrituras completas de datos/índices bajo el mutex `block_store`**
- Componente: Kura
- Tipo: Rendimiento, Disponibilidad
- Detalle: `evict_block_bodies` reescribe `blocks.data` e `blocks.index` mediante archivos temporales mientras mantiene el bloqueo `block_store`.
- Pruebas:
  - Adquisición de bloqueo: `crates/iroha_core/src/kura.rs:834`
  - Bucles de reescritura completos: `crates/iroha_core/src/kura.rs:921`, `crates/iroha_core/src/kura.rs:942`
  - Reemplazo/sincronización atómica: `crates/iroha_core/src/kura.rs:956`, `crates/iroha_core/src/kura.rs:960`
- Impacto: los eventos de desalojo pueden detener las escrituras/lecturas durante períodos prolongados en historias grandes.

3. **El compromiso de estado mantiene `view_lock` aproximado en trabajos de compromiso pesados**
- Componente: Producción WSV
- Tipo: Rendimiento, Disponibilidad
- Detalle: la confirmación de bloque tiene `view_lock` exclusivo mientras se confirman transacciones, hashes de bloque y estado mundial, lo que genera hambre en los lectores bajo bloques pesados.
- Pruebas:
  - Comienza la retención de bloqueo: `crates/iroha_core/src/state.rs:17456`
  - Trabajo dentro de la cerradura: `crates/iroha_core/src/state.rs:17466`, `crates/iroha_core/src/state.rs:17476`, `crates/iroha_core/src/state.rs:17483`
- Impacto: las confirmaciones intensas y sostenidas pueden degradar la capacidad de respuesta de consultas/consenso.4. **IVM Los alias de administrador JSON permiten mutaciones privilegiadas sin comprobaciones de la persona que llama (host de prueba/desarrollo)**
- Componente: host simulado IVM WSV
- Tipo: Seguridad (escalada de privilegios en entornos de prueba/desarrollo)
- Detalle: los controladores de alias JSON se dirigen directamente a métodos de mutación de rol/permiso/par que no requieren tokens de permiso con alcance de la persona que llama.
- Pruebas:
  - Alias de administrador: `crates/ivm/src/mock_wsv.rs:4274`, `crates/ivm/src/mock_wsv.rs:4371`, `crates/ivm/src/mock_wsv.rs:4448`
  - Mutadores no activados: `crates/ivm/src/mock_wsv.rs:1035`, `crates/ivm/src/mock_wsv.rs:1055`, `crates/ivm/src/mock_wsv.rs:855`
  - Nota de alcance en documentos de archivo (intención de prueba/desarrollo): `crates/ivm/src/mock_wsv.rs:295`
- Impacto: los contratos/herramientas de prueba pueden autoelevar e invalidar los supuestos de seguridad en los arneses de integración.

### Medio

5. **Las comprobaciones del presupuesto de Kura vuelven a codificar los bloques pendientes en cada puesta en cola (O(n) por escritura)**
- Componente: Kura
- Tipo: Rendimiento
- Detalle: cada puesta en cola vuelve a calcular los bytes de cola pendientes iterando los bloques pendientes y serializando cada uno a través de una ruta de tamaño de cable canónico.
- Pruebas:
  - Escaneo de cola: `crates/iroha_core/src/kura.rs:2509`
  - Ruta de codificación por bloque: `crates/iroha_core/src/kura.rs:2194`, `crates/iroha_core/src/kura.rs:2525`
  - Llamado en verificación de presupuesto en cola: `crates/iroha_core/src/kura.rs:2580`, `crates/iroha_core/src/kura.rs:2050`
- Impacto: degradación del rendimiento de escritura bajo trabajo pendiente.6. **Las comprobaciones del presupuesto de Kura realizan lecturas repetidas de metadatos del almacén de bloques por cola**
- Componente: Kura
- Tipo: Rendimiento
- Detalle: cada verificación lee el recuento de índices duraderos y la longitud de los archivos mientras bloquea `block_store`.
- Pruebas:
  - `crates/iroha_core/src/kura.rs:2538`
  -`crates/iroha_core/src/kura.rs:2548`
  - `crates/iroha_core/src/kura.rs:2575`
- Impacto: sobrecarga evitable de E/S/bloqueo en la ruta de cola activa.

7. **El desalojo de Kura se activa en línea desde la ruta del presupuesto en cola**
- Componente: Kura
- Tipo: Rendimiento, Disponibilidad
- Detalle: la ruta de puesta en cola puede llamar sincrónicamente al desalojo antes de aceptar nuevos bloques.
- Pruebas:
  - Cadena de llamadas en cola: `crates/iroha_core/src/kura.rs:2050`
  - Llamada de desalojo en línea: `crates/iroha_core/src/kura.rs:2603`
- Impacto: picos de latencia de cola en la ingesta de transacciones/bloques cuando se acerca al presupuesto.

8. **`State::view` puede regresar sin adquirir bloqueo grueso bajo disputa**
- Componente: Producción WSV
- Tipo: Compensación consistencia/rendimiento
- Detalle: en caso de disputa por bloqueo de escritura, el respaldo `try_read` devuelve la vista sin protección gruesa por diseño.
- Pruebas:
  - `crates/iroha_core/src/state.rs:14543`
  - `crates/iroha_core/src/state.rs:14545`
  - `crates/iroha_core/src/state.rs:18301`
- Impacto: vida mejorada, pero quienes llaman deben tolerar una atomicidad entre componentes más débil bajo contención.9. **`apply_without_execution` usa `expect` duro en el avance del cursor DA**
- Componente: Producción WSV
- Tipo: Seguridad (DoS mediante pánico en caso de interrupción invariante), Fiabilidad
- Detalle: el bloque comprometido aplica pánico en la ruta si fallan las invariantes de avance del cursor DA.
- Pruebas:
  - `crates/iroha_core/src/state.rs:17621`
  -`crates/iroha_core/src/state.rs:17625`
- Impacto: los errores latentes de validación/indexación pueden convertirse en fallas que eliminan los nodos.

10. **IVM La llamada al sistema de publicación TLV carece de límite de tamaño de sobre explícito antes de la asignación (host de prueba/desarrollo)**
- Componente: host simulado IVM WSV
- Tipo: Seguridad (memoria DoS), Rendimiento
- Detalle: lee la longitud del encabezado y luego asigna/copia la carga útil TLV completa sin un límite a nivel de host en esta ruta.
- Pruebas:
  - `crates/ivm/src/mock_wsv.rs:3750`
  - `crates/ivm/src/mock_wsv.rs:3755`
  - `crates/ivm/src/mock_wsv.rs:3759`
- Impacto: las cargas útiles de prueba maliciosas pueden forzar grandes asignaciones.

### Bajo

11. **El canal de notificación de Kura no tiene límites (`std::sync::mpsc::channel`)**
- Componente: Kura
- Tipo: Rendimiento/Higiene de la memoria
- Detalle: el canal de notificación puede acumular eventos de activación redundantes durante la presión sostenida del productor.
- Pruebas:
  -`crates/iroha_core/src/kura.rs:552`
- Impacto: el riesgo de crecimiento de la memoria es bajo por tamaño de evento, pero se puede evitar.12. **La cola de sidecar de canalización no está limitada en la memoria hasta que se agota el escritor**
- Componente: Kura
- Tipo: Rendimiento/Higiene de la memoria
- Detalle: la cola de sidecar `push_back` no tiene límite/contrapresión explícita.
- Pruebas:
  - `crates/iroha_core/src/kura.rs:104`
  - `crates/iroha_core/src/kura.rs:3427`
- Impacto: potencial crecimiento de la memoria durante retrasos prolongados en la escritura.

## Cobertura de pruebas existente y lagunas

### kura

- Coberturas existentes:
  - comportamiento del presupuesto de almacenamiento: `store_block_rejects_when_budget_exceeded`, `store_block_rejects_when_pending_blocks_exceed_budget`, `store_block_evicts_when_block_exceeds_budget` (`crates/iroha_core/src/kura.rs:6820`, `crates/iroha_core/src/kura.rs:6949`, `crates/iroha_core/src/kura.rs:6984`)
  - corrección de desalojo y rehidratación: `evict_block_bodies_does_not_truncate_unpersisted`, `evicted_block_rehydrates_from_da_store` (`crates/iroha_core/src/kura.rs:8040`, `crates/iroha_core/src/kura.rs:8126`)
- Brechas:
  - sin cobertura de inyección de fallos para el manejo de fallos de append/index/fsync sin pánico
  - sin pruebas de regresión de rendimiento para grandes colas pendientes y costos de verificación de presupuesto en cola
  - no hay prueba de latencia de desalojo de larga duración bajo contención de bloqueo

### Producción WSV

- Coberturas existentes:
  - comportamiento de reserva de contención: `state_view_returns_when_view_lock_held` (`crates/iroha_core/src/state.rs:18293`)
  - seguridad de orden de bloqueo alrededor del backend escalonado: `state_commit_does_not_hold_tiered_backend_while_waiting_for_view_lock` (`crates/iroha_core/src/state.rs:18321`)
- Brechas:
  - no hay prueba de contención cuantitativa que afirme el tiempo de espera de compromiso máximo aceptable bajo compromisos mundiales intensos
  - no hay prueba de regresión para un manejo sin pánico si las invariantes de avance del cursor DA se rompen inesperadamente

### IVM Host simulado de WSV- Coberturas existentes:
  - Semántica del analizador JSON de permisos y análisis entre pares (`crates/ivm/src/mock_wsv.rs:5234`, `crates/ivm/src/mock_wsv.rs:5332`)
  - Pruebas de humo de llamadas al sistema en torno a la decodificación TLV y la decodificación JSON (`crates/ivm/src/mock_wsv.rs:5962`, `crates/ivm/src/mock_wsv.rs:6078`)
- Brechas:
  - no hay pruebas de rechazo de alias de administrador no autorizado
  - no hay pruebas de rechazo de sobres TLV de gran tamaño en `INPUT_PUBLISH_TLV`
  - No hay pruebas de referencia/barreras de seguridad en torno al costo de clonación de punto de control/restauración

## Plan de remediación priorizado

### Fase 1 (Endurecimiento de alto impacto)

1. Reemplace las ramas `panic!` del escritor Kura con propagación de errores recuperables + señalización de estado degradado.
- Archivos de destino: `crates/iroha_core/src/kura.rs`
- Aceptación:
  - Los fallos de append/index/fsync inyectados no entran en pánico
  - Los errores aparecen a través de telemetría/registro y el escritor sigue siendo controlable.

2. Agregue comprobaciones de sobre delimitadas para la publicación TLV del host simulado IVM y las rutas de sobre JSON.
- Archivos de destino: `crates/ivm/src/mock_wsv.rs`
- Aceptación:
  - las cargas útiles de gran tamaño se rechazan antes del procesamiento intensivo de asignaciones
  - Las nuevas pruebas cubren casos de gran tamaño tanto TLV como JSON.

3. Aplique comprobaciones explícitas de permisos de llamadas para los alias de administrador JSON (o alias de puerta detrás de indicadores estrictos de funciones de solo prueba y documente claramente).
- Archivos de destino: `crates/ivm/src/mock_wsv.rs`
- Aceptación:
  - la persona que llama no autorizada no puede cambiar el rol/permiso/estado de igual a través de alias

### Fase 2 (rendimiento de ruta activa)4. Hacer que la contabilidad del presupuesto de Kura sea incremental.
- Reemplazar el recálculo completo de colas pendientes por puesta en cola con contadores mantenidos actualizados al poner en cola/persistir/eliminar.
- Aceptación:
  - Costo de puesta en cola cerca de O(1) para el cálculo de bytes pendientes
  - el punto de referencia de regresión muestra una latencia estable a medida que crece la profundidad pendiente

5. Reducir el tiempo de espera del bloqueo de desalojo.
- Opciones: compactación segmentada, copia fragmentada con límites de liberación de bloqueo o modo de mantenimiento en segundo plano con bloqueo de primer plano delimitado.
- Aceptación:
  - La latencia de desalojo de gran historial disminuye y las operaciones en primer plano siguen respondiendo

6. Acorte la sección crítica gruesa `view_lock` cuando sea posible.
- Evalúe la división de las fases de confirmación o la toma de instantáneas de deltas escalonados para minimizar las ventanas de retención exclusivas.
- Aceptación:
  - Las métricas de contención demuestran un tiempo de espera reducido de 99p bajo compromisos de bloques pesados.

### Fase 3 (Barandillas operativas)

7. Introducir señalización de estela limitada/unida para el escritor Kura y la contrapresión/tapas de cola de sidecar.
8. Amplíe los paneles de telemetría para:
- Distribuciones de espera/espera `view_lock`
- duración del desalojo y bytes recuperados por ejecución
- latencia en cola de verificación de presupuesto

## Adiciones de prueba sugeridas1. `kura_writer_io_failures_do_not_panic` (unidad, inyección de falla)
2. `kura_budget_check_scales_with_pending_depth` (regresión de rendimiento)
3. `kura_eviction_does_not_block_reads_beyond_threshold` (integración/rendimiento)
4. `state_commit_view_lock_hold_under_heavy_world_commit` (regresión de contención)
5. `state_apply_without_execution_handles_da_cursor_error_without_panic` (resiliencia)
6. `mock_wsv_admin_alias_requires_permissions` (regresión de seguridad)
7. `mock_wsv_input_publish_tlv_rejects_oversize` (protección DoS)
8. `mock_wsv_checkpoint_restore_cost_regression` (comparación de rendimiento)

## Notas sobre alcance y confianza

- Los hallazgos para `crates/iroha_core/src/kura.rs` e `crates/iroha_core/src/state.rs` son hallazgos de la ruta de producción.
- Los hallazgos para `crates/ivm/src/mock_wsv.rs` tienen un alcance explícito del host de prueba/desarrollo, según la documentación a nivel de archivo.
- Esta auditoría en sí no requiere cambios en la versión de ABI.
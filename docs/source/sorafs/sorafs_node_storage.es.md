---
lang: es
direction: ltr
source: docs/source/sorafs/sorafs_node_storage.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0b114fcc91d3b5cb3c23b64a73a2601e70547d1ac2f5e097400034d7ac3dee21
source_last_modified: "2026-01-04T10:50:53.670846+00:00"
translation_last_reviewed: 2026-01-30
---

## Diseño de almacenamiento del nodo SoraFS (borrador)

Esta nota refina cómo un nodo Iroha (Torii) puede optar por unirse a la capa de
availability de datos de SoraFS y dedicar una porción de disco local para
almacenar y servir chunks. Complementa la especificación de descubrimiento
`sorafs_node_client_protocol.md` y el trabajo de fixtures SF-1b al describir la
arquitectura del lado del almacenamiento, los controles de recursos y el
cableado de configuración que debe aterrizar en los caminos de código del nodo
y el gateway. Las prácticas operativas para operadores viven en
`sorafs/runbooks/sorafs_node_ops.md`.

### Objetivos

- Permitir que cualquier validador o proceso auxiliar de Iroha exponga disco
  disponible como proveedor SoraFS sin afectar las responsabilidades del ledger.
- Mantener el módulo de almacenamiento determinista y guiado por Norito:
  manifiestos, planes de chunks, raíces de Prueba de Recuperabilidad (PoR) y
  provider adverts son la fuente de verdad.
- Aplicar cuotas definidas por el operador para que el nodo no agote sus propios
  recursos al aceptar demasiadas solicitudes de pin o fetch.
- Exponer salud/telemetría (muestreo PoR, latencia de fetch de chunks, presión
  de disco) de vuelta a la gobernanza y a los clientes.

### Arquitectura de alto nivel

```
┌──────────────────────────────────────────────────────────────────────┐
│                         Iroha/Torii Node                             │
│                                                                      │
│  ┌──────────────┐      ┌────────────────────┐                        │
│  │  Torii APIs  │◀────▶│   SoraFS Gateway   │◀───────────────┐       │
│  └──────────────┘      │ (Norito endpoints) │                │       │
│                        └────────┬───────────┘                │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Pin Registry   │◀───── manifests   │       │
│                        │ (State / DB)    │                   │       │
│                        └────────┬────────┘                   │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Chunk Storage  │◀──── chunk plans  │       │
│                        │  (ChunkStore)   │                   │       │
│                        └────────┬────────┘                   │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Disk Quota/IO  │─Pin/serve chunks─▶│ Fetch │
│                        │  Scheduler      │                   │ Clients│
│                        └─────────────────┘                   │       │
│                                                                      │
└──────────────────────────────────────────────────────────────────────┘
```

Módulos clave:

- **Gateway**: expone endpoints HTTP Norito para propuestas de pin, solicitudes
  de fetch de chunks, muestreo PoR y telemetría. Valida payloads Norito y
  enruta las solicitudes al chunk store. Reutiliza el stack HTTP de Torii para
  evitar un nuevo daemon.
- **Pin Registry**: el estado de pins de manifiesto rastreado en
  `iroha_data_model::sorafs` e `iroha_core`. Cuando se acepta un manifiesto, el
  registro almacena el digest del manifiesto, el digest del plan de chunk, la
  raíz PoR y las flags de capacidad del proveedor.
- **Chunk Storage**: implementación `ChunkStore` respaldada en disco que ingiere
  manifiestos firmados, materializa planes de chunk usando `ChunkProfile::DEFAULT`
  y persiste chunks bajo un layout determinista. Cada chunk se asocia con una
  huella de contenido y metadatos PoR para que el muestreo pueda revalidar sin
  re-leer el archivo completo.
- **Cuota/Planificador**: aplica límites configurados por el operador (bytes de
  disco máximos, pins pendientes máximos, fetches paralelos máximos, TTL de
  chunks) y coordina el IO para que las tareas del ledger del nodo no se vean
  hambrientas. El planificador también es responsable de servir pruebas PoR y
  solicitudes de muestreo con CPU acotado.

### Configuración

Añada una nueva sección a `iroha_config`:

```toml
[sorafs.storage]
enabled = false
data_dir = "/var/lib/iroha/sorafs"
max_capacity_bytes = "100 GiB"
max_parallel_fetches = 32
max_pins = 10_000
por_sample_interval_secs = 600
alias = "tenant.alpha"            # optional human friendly tag
adverts:
  stake_pointer = "stake.pool.v1:0x1234"
  availability = "hot"
  max_latency_ms = 500
  topics = ["sorafs.sf1.primary:global"]
```

- `enabled`: interruptor de participación. Cuando es false, el gateway devuelve
  503 para endpoints de almacenamiento y el nodo no se anuncia en discovery.
- `data_dir`: directorio raíz para datos de chunks, árboles PoR y telemetría de
  fetch. Por defecto `<iroha.data_dir>/sorafs`.
- `max_capacity_bytes`: límite duro para datos de chunks fijados. Una tarea en
  background rechaza nuevos pins cuando se alcanza el límite.
- `max_parallel_fetches`: tope de concurrencia aplicado por el scheduler para
  equilibrar ancho de banda/IO de disco frente a la carga del validador.
- `max_pins`: número máximo de pins de manifiesto que el nodo acepta antes de
  aplicar expulsión/back pressure.
- `por_sample_interval_secs`: cadencia para trabajos automáticos de muestreo
  PoR. Cada trabajo muestrea `N` hojas (configurable por manifiesto) y emite
  eventos de telemetría. La gobernanza puede escalar `N` de forma determinista
  estableciendo la clave de metadatos de capacidad `profile.sample_multiplier`
  (entero `1-4`). El valor puede ser un único número/cadena o un objeto con
  overrides por perfil, p. ej. `{"default":2,"sorafs.sf2@1.0.0":3}`.
- `adverts`: estructura usada por el generador de provider adverts para poblar
  campos `ProviderAdvertV1` (stake pointer, hints de QoS, topics). Si se omite,
  el nodo usa los defaults del registro de gobernanza.

Cableado de configuración:

- `[sorafs.storage]` se define en `iroha_config` como `SorafsStorage` y se
  carga desde el archivo de config del nodo.
- `iroha_core` e `iroha_torii` pasan la configuración de storage al builder del
  gateway y al chunk store al inicio.
- Existen overrides de entorno para dev/test (`SORAFS_STORAGE_*`,
  `SORAFS_STORAGE_PIN_*`), pero los despliegues en producción deben apoyarse en
  el archivo de config.

### Utilidades CLI

Mientras la superficie HTTP de Torii se termina de cablear, el crate
`sorafs_node` incluye una CLI ligera para que los operadores automaticen
prácticas de ingesta/exportación contra el backend persistente.【crates/sorafs_node/src/bin/sorafs-node.rs:1】

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin \
  --plan-json-out ./plan.json
```

- `ingest` espera un archivo de manifiesto `.to` codificado en Norito y los
  bytes de payload correspondientes. Reconstruye el plan de chunk desde el
  perfil de chunking del manifiesto, aplica paridad de digest, persiste archivos
  de chunks y opcionalmente emite un blob JSON `chunk_fetch_specs` para que las
  herramientas downstream validen el layout.
- `export` acepta un ID de manifiesto y escribe el manifiesto/payload almacenado
  en disco (con JSON de plan opcional) para que los fixtures sigan siendo
  reproducibles entre entornos.

Ambos comandos imprimen un resumen JSON Norito a stdout, lo que facilita
canalizarlo a scripts. La CLI está cubierta por una prueba de integración para
asegurar que manifiestos y payloads hagan round-trip limpio antes de que lleguen
las APIs de Torii.【crates/sorafs_node/tests/cli.rs:1】

> Paridad HTTP
>
> El gateway de Torii ahora expone helpers de solo lectura respaldados por el
> mismo `NodeHandle`:
>
> - `GET /v1/sorafs/storage/manifest/{manifest_id_hex}` — devuelve el manifiesto
>   Norito almacenado (base64) junto con digest/metadatos.【crates/iroha_torii/src/sorafs/api.rs:1207】
> - `GET /v1/sorafs/storage/plan/{manifest_id_hex}` — devuelve el JSON del plan
>   de chunks determinista (`chunk_fetch_specs`) para herramientas downstream.【crates/iroha_torii/src/sorafs/api.rs:1259】
>
> Estos endpoints reflejan la salida de la CLI para que los pipelines puedan
> pasar de scripts locales a sondas HTTP sin cambiar parsers.【crates/iroha_torii/src/sorafs/api.rs:1207】【crates/iroha_torii/src/sorafs/api.rs:1259】

### Ciclo de vida del nodo

1. **Startup**:
   - Si el almacenamiento está habilitado, el nodo inicializa el chunk store con
     el directorio y la capacidad configurados. Esto incluye verificar o crear
     la base de datos de manifiestos PoR y reproducir los manifiestos fijados
     para calentar caches.
   - Registra las rutas del gateway SoraFS (endpoints Norito JSON POST/GET para
     pin, fetch, muestreo PoR y telemetría).
   - Lanza el worker de muestreo PoR y el monitor de cuotas.
2. **Discovery / adverts**:
   - Genera documentos `ProviderAdvertV1` usando la capacidad/estado actuales,
     los firma con la clave aprobada por el consejo y los publica vía el canal
     de discovery disponible.
3. **Flujo de pin**:
   - El gateway recibe un manifiesto firmado (incluye plan de chunk, raíz PoR,
     firmas del consejo). Valida la lista de alias (`sorafs.sf1@1.0.0` requerido)
     y asegura que el plan de chunk coincide con los metadatos del manifiesto.
   - Comprueba cuotas. Si se excederían los límites de capacidad/pins responde
     con un error de política (Norito estructurado).
   - Transmite datos de chunks al `ChunkStore`, verificando digests mientras
     ingerimos. Actualiza los árboles PoR y guarda metadatos del manifiesto en
     el registro.
4. **Flujo de fetch**:
   - Sirve solicitudes de rangos de chunks desde disco. El scheduler aplica
     `max_parallel_fetches` y devuelve `429` cuando está saturado.
   - Emite telemetría estructurada (JSON Norito) con latencia, bytes servidos y
     conteos de errores para monitoreo downstream.
5. **Muestreo PoR**:
   - El worker selecciona manifiestos de forma proporcional al peso (p. ej.,
     bytes almacenados) y ejecuta muestreo determinista usando el árbol PoR del
     chunk store.
   - Persiste resultados para auditorías de gobernanza e incluye resúmenes en
     provider adverts / endpoints de telemetría.
6. **Expulsión / aplicación de cuotas**:
   - Cuando se alcanza la capacidad, el nodo rechaza nuevos pins por defecto.
     Opcionalmente, los operadores pueden configurar políticas de expulsión
     (p. ej., TTL, LRU) una vez acordado el modelo de gobernanza; por ahora el
     diseño asume cuotas estrictas y operaciones de unpin iniciadas por el
     operador.

### Integración de declaraciones de capacidad y scheduling

- Torii ahora retransmite actualizaciones `CapacityDeclarationRecord` desde
  `/v1/sorafs/capacity/declare` al `CapacityManager` embebido, de modo que cada
  nodo construye una vista en memoria de sus asignaciones de chunker y lane
  comprometidas. El manager expone snapshots de solo lectura para telemetría
  (`GET /v1/sorafs/capacity/state`) y aplica reservas por perfil o por lane antes
  de aceptar nuevas órdenes.【crates/sorafs_node/src/capacity.rs:1】【crates/sorafs_node/src/lib.rs:60】
- El endpoint `/v1/sorafs/capacity/schedule` acepta payloads `ReplicationOrderV1`
  emitidos por gobernanza. Cuando la orden apunta al proveedor local, el manager
  verifica scheduling duplicado, valida capacidad de chunker/lane, reserva la
  porción y devuelve un `ReplicationPlan` que describe la capacidad restante
  para que las herramientas de orquestación procedan con la ingesta. Las órdenes
  para otros proveedores se reconocen con una respuesta `ignored` para facilitar
  flujos multi-operador.【crates/iroha_torii/src/routing.rs:4845】
- Los hooks de finalización (p. ej., disparados tras una ingesta exitosa) llaman
  a `POST /v1/sorafs/capacity/complete` para liberar reservas vía
  `CapacityManager::complete_order`. La respuesta incluye un snapshot
  `ReplicationRelease` (totales restantes, residuales de chunker/lane) para que
  las herramientas de orquestación puedan encolar la siguiente orden sin
  sondeo. El trabajo posterior cableará esto al pipeline del chunk store cuando
  llegue la lógica de ingesta.【crates/iroha_torii/src/routing.rs:4885】【crates/sorafs_node/src/capacity.rs:90】
- El `TelemetryAccumulator` embebido puede mutarse mediante
  `NodeHandle::update_telemetry`, permitiendo que workers en background graben
  muestras PoR/uptime y, eventualmente, deriven payloads `CapacityTelemetryV1`
  canónicos sin tocar los internals del scheduler.【crates/sorafs_node/src/lib.rs:142】【crates/sorafs_node/src/telemetry.rs:1】

### Integraciones y trabajo futuro

- **Governanza**: extender `sorafs_pin_registry_tracker.md` con telemetría de
  almacenamiento (tasa de éxito PoR, utilización de disco). Las políticas de
  admisión pueden exigir capacidad mínima o tasa mínima de éxito PoR antes de
  aceptar adverts.
- **SDKs de cliente**: exponer la nueva configuración de almacenamiento
  (límites de disco, alias) para que las herramientas de gestión puedan
  bootstrapping nodos programáticamente.
- **Telemetría**: integrar con el stack de métricas existente (Prometheus /
  OpenTelemetry) para que las métricas de almacenamiento aparezcan en los
  dashboards de observabilidad.
- **Seguridad**: ejecutar el módulo de almacenamiento en un pool de tareas async
  dedicado con back-pressure y considerar sandboxing de lecturas de chunks vía
  io_uring o pools acotados de tokio para evitar que clientes maliciosos agoten
  recursos.

Este diseño mantiene el módulo de almacenamiento opcional y determinista a la
vez que ofrece a los operadores los knobs que necesitan para participar en la
capa de availability de datos de SoraFS. Implementarlo implicará cambios en
`iroha_config`, `iroha_core`, `iroha_torii` y el gateway Norito, además del
_tooling_ de provider adverts.

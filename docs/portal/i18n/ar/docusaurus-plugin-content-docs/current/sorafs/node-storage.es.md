---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/node-storage.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: node-storage
title: Diseño de almacenamiento del nodo SoraFS
sidebar_label: Diseño de almacenamiento del nodo
description: Arquitectura de almacenamiento, cuotas y hooks del ciclo de vida para nodos Torii que alojan datos de SoraFS.
---

:::note Fuente canónica
Esta página refleja `docs/source/sorafs/sorafs_node_storage.md`. Mantén ambas copias sincronizadas hasta que el conjunto de documentación Sphinx heredado se retire.
:::

## Diseño de almacenamiento del nodo SoraFS (Borrador)

Esta nota refina cómo un nodo Iroha (Torii) puede optar por la capa de
availability de datos de SoraFS y dedicar una parte del disco local para
almacenar y servir chunks. Complementa la especificación de discovery
`sorafs_node_client_protocol.md` y el trabajo de fixtures SF-1b al detallar la
arquitectura del lado de storage, controles de recursos y plomería de
configuración que deben aterrizar en el nodo y en las rutas del gateway.
Las prácticas operativas viven en el
[Runbook de operaciones de nodo](./node-operations).

### Objetivos

- Permitir que cualquier validador o proceso auxiliar de Iroha exponga disco
  ocioso como proveedor SoraFS sin afectar las responsabilidades del ledger.
- Mantener el módulo de almacenamiento determinista y guiado por Norito:
  manifests, planes de chunk, raíces Proof-of-Retrievability (PoR) y adverts de
  proveedor son la fuente de verdad.
- Impone cuotas definidas por el operador para que un nodo no agote sus propios
  recursos al aceptar demasiadas solicitudes de pin o fetch.
- Exponer salud/telemetría (muestreo PoR, latencia de fetch de chunks, presión
  de disco) hacia gobernanza y clientes.

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
  enruta las solicitudes hacia el chunk store. Reusa el stack HTTP de Torii
  para evitar un nuevo daemon.
- **Pin Registry**: estado de pin de manifests rastreado en `iroha_data_model::sorafs`
  e `iroha_core`. Cuando se acepta un manifest, el registro almacena el digest
  del manifest, digest del plan de chunk, raíz PoR y flags de capacidad del
  proveedor.
- **Chunk Storage**: implementación `ChunkStore` respaldada por disco que ingiere
  manifests firmados, materializa planes de chunk usando `ChunkProfile::DEFAULT`
  y persiste chunks bajo un layout determinista. Cada chunk se asocia con un
  fingerprint de contenido y metadatos PoR para que el muestreo pueda revalidar
  sin releer el archivo completo.
- **Quota/Scheduler**: impone límites configurados por el operador (bytes máximos
  de disco, pines pendientes máximos, fetches paralelos máximos, TTL de chunk)
  y coordina IO para que las tareas del ledger no se queden sin recursos. El
  scheduler también sirve pruebas PoR y solicitudes de muestreo con CPU acotada.

### Configuración

Agrega una nueva sección a `iroha_config`:

```toml
[sorafs.storage]
enabled = false
data_dir = "/var/lib/iroha/sorafs"
max_capacity_bytes = "100 GiB"
max_parallel_fetches = 32
max_pins = 10_000
por_sample_interval_secs = 600
alias = "tenant.alpha"            # etiqueta opcional legible
adverts:
  stake_pointer = "stake.pool.v1:0x1234"
  availability = "hot"
  max_latency_ms = 500
  topics = ["sorafs.sf1.primary:global"]
```

- `enabled`: toggle de participación. Cuando está en false el gateway responde
  503 para endpoints de storage y el nodo no se anuncia en discovery.
- `data_dir`: directorio raíz para datos de chunk, árboles PoR y telemetría de
  fetch. El default es `<iroha.data_dir>/sorafs`.
- `max_capacity_bytes`: límite duro para datos de chunks fijados. Una tarea de
  fondo rechaza nuevos pins cuando se alcanza el límite.
- `max_parallel_fetches`: tope de concurrencia impuesto por el scheduler para
  equilibrar ancho de banda/IO de disco con la carga del validador.
- `max_pins`: número máximo de pins de manifest que acepta el nodo antes de
  aplicar eviction/back pressure.
- `por_sample_interval_secs`: cadencia para trabajos automáticos de muestreo
  PoR. Cada trabajo muestrea `N` hojas (configurable por manifest) y emite
  eventos de telemetría. La gobernanza puede escalar `N` de forma determinista
  estableciendo la clave de metadata `profile.sample_multiplier` (entero `1-4`).
  El valor puede ser un número/string único o un objeto con overrides por perfil,
  por ejemplo `{"default":2,"sorafs.sf2@1.0.0":3}`.
- `adverts`: estructura usada por el generador de adverts para completar campos
  `ProviderAdvertV1` (stake pointer, hints de QoS, topics). Si se omite el
  nodo usa defaults del registro de gobernanza.

Plomería de configuración:

- `[sorafs.storage]` se define en `iroha_config` como `SorafsStorage` y se carga
  desde el archivo de config del nodo.
- `iroha_core` y `iroha_torii` pasan la configuración de storage hacia el
  builder del gateway y el chunk store en el arranque.
- Existen overrides para dev/test (`SORAFS_STORAGE_*`, `SORAFS_STORAGE_PIN_*`), pero
  los despliegues de producción deben basarse en el archivo de configuración.

### Utilidades de CLI

Mientras la superficie HTTP de Torii se termina de cablear, el crate
`sorafs_node` incluye una CLI liviana para que los operadores puedan
automatizar drills de ingestión/exportación contra el backend persistente.【crates/sorafs_node/src/bin/sorafs-node.rs:1】

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin \
  --plan-json-out ./plan.json
```

- `ingest` espera un manifest `.to` codificado en Norito y los bytes de payload
  correspondientes. Reconstruye el plan de chunk a partir del perfil de
  chunking del manifest, impone paridad de digests, persiste archivos de chunk,
  y opcionalmente emite un blob JSON `chunk_fetch_specs` para que herramientas
  downstream validen el layout.
- `export` acepta un ID de manifest y escribe el manifest/payload almacenado en
  disco (con plan JSON opcional) para que los fixtures sigan siendo reproducibles
  entre entornos.

Ambos comandos imprimen un resumen Norito JSON a stdout, facilitando el uso en
scripts. La CLI está cubierta por una prueba de integración para asegurar que
manifests y payloads se reconstituyen correctamente antes de que lleguen las
APIs de Torii.【crates/sorafs_node/tests/cli.rs:1】

> Paridad HTTP
>
> El gateway Torii ahora expone helpers de solo lectura respaldados por el mismo
> `NodeHandle`:
>
> - `GET /v1/sorafs/storage/manifest/{manifest_id_hex}` — devuelve el manifest
>   Norito almacenado (base64) junto con digest/metadata.【crates/iroha_torii/src/sorafs/api.rs:1207】
> - `GET /v1/sorafs/storage/plan/{manifest_id_hex}` — devuelve el plan de chunk
>   determinista JSON (`chunk_fetch_specs`) para tooling downstream.【crates/iroha_torii/src/sorafs/api.rs:1259】
>
> Estos endpoints reflejan la salida del CLI para que los pipelines puedan
> pasar de scripts locales a probes HTTP sin cambiar parsers.【crates/iroha_torii/src/sorafs/api.rs:1207】【crates/iroha_torii/src/sorafs/api.rs:1259】

### Ciclo de vida del nodo

1. **Arranque**:
   - Si el almacenamiento está habilitado el nodo inicializa el chunk store con
     el directorio y capacidad configurados. Esto incluye verificar o crear la
     base de datos de manifests PoR y reproducir manifests fijados para calentar
     caches.
   - Registrar rutas del gateway SoraFS (endpoints Norito JSON POST/GET para pin,
     fetch, muestreo PoR, telemetría).
   - Lanzar el worker de muestreo PoR y el monitor de cuotas.
2. **Discovery / Adverts**:
   - Generar documentos `ProviderAdvertV1` usando la capacidad/salud actual,
     firmarlos con la clave aprobada por el consejo y publicarlos vía discovery.
     Usa la nueva lista `profile_aliases` para que los handles canónicos y
3. **Flujo de pin**:
   - El gateway recibe un manifest firmado (incluyendo plan de chunk, raíz PoR,
     firmas del consejo). Valida la lista de alias (`sorafs.sf1@1.0.0` requerido)
     y asegura que el plan de chunk coincide con la metadata del manifest.
   - Verifica cuotas. Si capacidad/límites de pins se excederían responde con un
     error de política (Norito estructurado).
   - Streamea datos de chunk hacia `ChunkStore`, verificando digests durante la
     ingestión. Actualiza árboles PoR y almacena metadata del manifest en el
     registro.
4. **Flujo de fetch**:
   - Sirve solicitudes de rango de chunk desde disco. El scheduler impone
     `max_parallel_fetches` y devuelve `429` cuando está saturado.
   - Emite telemetría estructurada (Norito JSON) con latencia, bytes servidos y
     conteos de error para monitoreo downstream.
5. **Muestreo PoR**:
   - El worker selecciona manifests proporcionalmente al peso (por ejemplo, bytes
     almacenados) y ejecuta muestreo determinista usando el árbol PoR del chunk store.
   - Persiste resultados para auditorías de gobernanza e incluye resúmenes en
     adverts de proveedor / endpoints de telemetría.
6. **Expulsión / cumplimiento de cuotas**:
   - Cuando se alcanza la capacidad el nodo rechaza nuevos pins por defecto.
     Opcionalmente, los operadores pueden configurar políticas de expulsión
     (por ejemplo, TTL, LRU) una vez que el modelo de gobernanza se acuerde; por
     ahora el diseño asume cuotas estrictas y operaciones de unpin iniciadas por
     el operador.

### Declaración de capacidad e integración de scheduling

- Torii ahora retransmite actualizaciones de `CapacityDeclarationRecord` desde
  `/v1/sorafs/capacity/declare` hacia el `CapacityManager` embebido, de modo que
  cada nodo construye una vista en memoria de sus asignaciones comprometidas de
  chunker y lane. El manager expone snapshots de solo lectura para telemetría
  (`GET /v1/sorafs/capacity/state`) y aplica reservas por perfil o lane antes de
  aceptar nuevos pedidos.【crates/sorafs_node/src/capacity.rs:1】【crates/sorafs_node/src/lib.rs:60】
- El endpoint `/v1/sorafs/capacity/schedule` acepta payloads `ReplicationOrderV1`
  emitidos por gobernanza. Cuando la orden apunta al proveedor local el manager
  revisa scheduling duplicado, verifica capacidad de chunker/lane, reserva la
  franja y devuelve un `ReplicationPlan` describiendo la capacidad restante para
  que las herramientas de orquestación continúen la ingestión. Las órdenes para
  otros proveedores se reconocen con una respuesta `ignored` para facilitar flujos
  multi-operador.【crates/iroha_torii/src/routing.rs:4845】
- Hooks de completitud (por ejemplo, disparados tras el éxito de la ingestión)
  llaman a `POST /v1/sorafs/capacity/complete` para liberar reservas vía
  `CapacityManager::complete_order`. La respuesta incluye un snapshot
  `ReplicationRelease` (totales restantes, residuales de chunker/lane) para que
  las herramientas de orquestación puedan encolar la siguiente orden sin polling.
  Trabajo futuro cableará esto al pipeline del chunk store una vez que la
  lógica de ingestión aterrice.【crates/iroha_torii/src/routing.rs:4885】【crates/sorafs_node/src/capacity.rs:90】
- El `TelemetryAccumulator` embebido puede mutarse mediante
  `NodeHandle::update_telemetry`, permitiendo que workers en segundo plano
  registren muestras de PoR/uptime y eventualmente deriven payloads canónicos
  `CapacityTelemetryV1` sin tocar internos del scheduler.【crates/sorafs_node/src/lib.rs:142】【crates/sorafs_node/src/telemetry.rs:1】

### Integraciones y trabajo futuro

- **Governanza**: extender `sorafs_pin_registry_tracker.md` con telemetría de
  storage (tasa de éxito PoR, utilización de disco). Las políticas de admisión
  pueden requerir capacidad mínima o tasa mínima de éxito PoR antes de aceptar
  adverts.
- **SDKs de cliente**: exponer la nueva configuración de storage (límites de
  disco, alias) para que herramientas de gestión puedan bootstrapear nodos
  programáticamente.
- **Telemetría**: integrar con el stack de métricas existente (Prometheus /
  OpenTelemetry) para que las métricas de storage aparezcan en dashboards de
  observabilidad.
- **Seguridad**: ejecutar el módulo de storage en un pool dedicado de tareas
  async con back-pressure y considerar sandboxing de lecturas de chunks vía
  io_uring o pools acotados de tokio para evitar que clientes maliciosos agoten
  recursos.

Este diseño mantiene el módulo de almacenamiento opcional y determinista a la
vez que otorga a los operadores los knobs necesarios para participar en la capa
SoraFS de data availability. Implementarlo implicará cambios en `iroha_config`,
`iroha_core`, `iroha_torii` y el gateway Norito, además del tooling de adverts
de proveedor.

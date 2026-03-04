---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/node-storage.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: almacenamiento de nodo
título: Diseño de almacenamiento del nodo SoraFS
sidebar_label: Diseño de almacenamiento del nodo
descripción: Arquitectura de almacenamiento, cuotas y ganchos del ciclo de vida para nodos Torii que alojan datos de SoraFS.
---

:::nota Fuente canónica
Esta página refleja `docs/source/sorafs/sorafs_node_storage.md`. Mantenga ambas copias sincronizadas hasta que el conjunto de documentación Sphinx heredado se retire.
:::

## Diseño de almacenamiento del nodo SoraFS (Borrador)

Esta nota refina cómo un nodo Iroha (Torii) puede optar por la capa de
disponibilidad de datos de SoraFS y dedicar una parte del disco local para
almacenar y servir trozos. Complementa la especificación de descubrimiento
`sorafs_node_client_protocol.md` y el trabajo de accesorios SF-1b al detallar la
arquitectura del lado de almacenamiento, controles de recursos y plomería de
configuración que deben aterrizar en el nodo y en las rutas del gateway.
Las prácticas operativas viven en el
[Runbook de operaciones de nodo](./node-operations).

### Objetivos- Permitir que cualquier validador o proceso auxiliar de Iroha exponga disco
  ocioso como proveedor SoraFS sin afectar las responsabilidades del libro mayor.
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

Módulos clave:- **Gateway**: exponen endpoints HTTP Norito para propuestas de pin, solicitudes
  de fetch de chunks, muestreo PoR y telemetría. Cargas útiles de validación Norito y
  enruta las solicitudes hacia el chunk store. Reutilizar la pila HTTP de Torii
  para evitar un nuevo demonio.
- **Registro de PIN**: estado de pin de manifests rastreado en `iroha_data_model::sorafs`
  y `iroha_core`. Cuando se acepta un manifiesto, el registro almacena el resumen
  del manifest, digest del plan de chunk, raíz PoR y flags de capacidad del
  proveedor.
- **Chunk Storage**: implementación `ChunkStore` respaldada por disco que ingiere
  manifests firmados, materializa planes de chunk usando `ChunkProfile::DEFAULT`
  y persisten fragmentos bajo un diseño determinista. Cada trozo se asocia con un
  huellas dactilares de contenido y metadatos PoR para que el muestreo pueda revalidar
  sin releer el archivo completo.
- **Cuota/Programador**: impone límites configurados por el operador (bytes máximos
  de disco, pines pendientes máximos, fetches paralelos máximos, TTL de chunk)
  y coordina IO para que las tareas del libro mayor no se queden sin recursos. el
  Scheduler también sirve pruebas PoR y solicitudes de muestreo con CPU acotada.

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
```- `enabled`: alternar participación. Cuando está en falso el gateway responde
  503 para endpoints de almacenamiento y el nodo no se anuncia en descubrimiento.
- `data_dir`: directorio raíz para datos de chunk, árboles PoR y telemetría de
  buscar. El valor predeterminado es `<iroha.data_dir>/sorafs`.
- `max_capacity_bytes`: límite duro para datos de trozos fijados. Una tarea de
  fondo rechaza nuevos pines cuando se alcanza el límite.
- `max_parallel_fetches`: tope de concurrencia impuesto por el planificador para
  equilibrar ancho de banda/IO de disco con la carga del validador.
- `max_pins`: número máximo de pines de manifiesto que acepta el nodo antes de
  Aplicar desalojo/contrapresión.
- `por_sample_interval_secs`: cadencia para trabajos automáticos de muestreo
  Por. Cada trabajo muestrea `N` hojas (configurable por manifiesto) y emite
  eventos de telemetría. La gobernanza puede escalar `N` de forma determinista
  estableciendo la clave de metadatos `profile.sample_multiplier` (entero `1-4`).
  El valor puede ser un número/string único o un objeto con overrides por perfil,
  por ejemplo `{"default":2,"sorafs.sf2@1.0.0":3}`.
- `adverts`: estructura usada por el generador de anuncios para completar campos
  `ProviderAdvertV1` (puntero de participación, sugerencias de QoS, temas). Si se omite el
  nodo usa defaults del registro de gobernanza.

Plomería de configuración:- `[sorafs.storage]` se define en `iroha_config` como `SorafsStorage` y se carga
  desde el archivo de configuración del nodo.
- `iroha_core` y `iroha_torii` pasan la configuración de almacenamiento hacia el
  builder del gateway y el chunk store en el arranque.
- Existen anulaciones para dev/test (`SORAFS_STORAGE_*`, `SORAFS_STORAGE_PIN_*`), pero
  los despliegues de producción deben basarse en el archivo de configuración.

### Utilidades de CLI

Mientras la superficie HTTP de Torii se termina de cablear, el crate
`sorafs_node` incluye una CLI liviana para que los operadores puedan
automatizar simulacros de ingestión/exportación contra el backend persistente.【crates/sorafs_node/src/bin/sorafs-node.rs:1】

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin \
  --plan-json-out ./plan.json
```

- `ingest` espera un manifiesto `.to` codificado en Norito y los bytes de carga útil
  correspondientes. Reconstruye el plan de chunk a partir del perfil de
  fragmentación del manifiesto, impone paridad de resúmenes, persiste archivos de fragmentación,
  y opcionalmente emite un blob JSON `chunk_fetch_specs` para que herramientas
  aguas abajo validen el diseño.
- `export` acepta un ID de manifiesto y escribe el manifiesto/carga útil almacenado en
  disco (con plan JSON opcional) para que los aparatos sigan siendo reproducibles
  entre entornos.Ambos comandos imprimen un resumen Norito JSON a stdout, facilitando el uso en
guiones. La CLI está cubierta por una prueba de integración para asegurar que
manifests y payloads se reconstituyen correctamente antes de que lleguen las
API de Torii.【crates/sorafs_node/tests/cli.rs:1】

> Paridad HTTP
>
> El gateway Torii ahora exponen helpers de solo lectura respaldados por el mismo
> `NodeHandle`:
>
> - `GET /v1/sorafs/storage/manifest/{manifest_id_hex}` — devuelve el manifiesto
> Norito almacenado (base64) junto con digest/metadata.【crates/iroha_torii/src/sorafs/api.rs:1207】
> - `GET /v1/sorafs/storage/plan/{manifest_id_hex}` — devuelve el plan de trozo
> determinista JSON (`chunk_fetch_specs`) para herramientas posteriores.【crates/iroha_torii/src/sorafs/api.rs:1259】
>
> Estos endpoints reflejan la salida del CLI para que los pipelines puedan
> pasar de scripts locales a probes HTTP sin cambiar parsers.【crates/iroha_torii/src/sorafs/api.rs:1207】【crates/iroha_torii/src/sorafs/api.rs:1259】

### Ciclo de vida del nodo1. **Arranque**:
   - Si el almacenamiento está habilitado, el nodo inicializa el almacén de fragmentos con
     el directorio y capacidad configurados. Esto incluye verificar o crear la
     base de datos de manifiestos PoR y reproducir manifiestos fijados para calentar
     cachés.
   - Registrador de rutas del gateway SoraFS (endpoints Norito JSON POST/GET para pin,
     fetch, muestreo PoR, telemetría).
   - Lanzar el trabajador de muestreo PoR y el monitor de cuotas.
2. **Descubrimiento/Anuncios**:
   - Generar documentos `ProviderAdvertV1` usando la capacidad/salud actual,
     firmarlos con la clave aprobada por el consejo y publicarlos vía descubrimiento.
     Usa la nueva lista `profile_aliases` para que los handles canónicos y
3. **Flujo de pin**:
   - El gateway recibe un manifiesto firmado (incluyendo plan de chunk, raíz PoR,
     firmas del consejo). Valida la lista de alias (`sorafs.sf1@1.0.0` requerido)
     y asegura que el plan de chunk coincide con los metadatos del manifiesto.
   - Verifica cuotas. Si capacidad/límites de pines se excederían responder con un
     error de política (Norito estructurado).
   - Streamea datos de chunk hacia `ChunkStore`, verificando digests durante la
     ingestión. Actualiza árboles PoR y almacena metadatos del manifiesto en el
     registro.
4. **Flujo de búsqueda**:
   - Sirve solicitudes de rango de trozos desde discoteca. El planificador impone`max_parallel_fetches` y devuelve `429` cuando está saturado.
   - Emite telemetría estructurada (Norito JSON) con latencia, bytes servidos y
     Conteos de error para monitoreo downstream.
5. **Muestreo PoR**:
   - El trabajador selecciona manifiestos proporcionalmente al peso (por ejemplo, bytes
     almacenados) y ejecuta muestreo determinista usando el árbol PoR del chunk store.
   - Persiste resultados para auditorías de gobernanza e incluye resúmenes en
     anuncios de proveedor / endpoints de telemetría.
6. **Expulsión / cumplimiento de cuotas**:
   - Cuando se alcanza la capacidad el nudo rechaza nuevos pines por defecto.
     Opcionalmente, los operadores pueden configurar políticas de expulsión.
     (por ejemplo, TTL, LRU) una vez que el modelo de gobernanza se acuerda; por
     ahora el diseño asume cuotas estrictas y operaciones de unpin iniciadas por
     el operador.

### Declaración de capacidad e integración de programación- Torii ahora retransmite actualizaciones de `CapacityDeclarationRecord` desde
  `/v1/sorafs/capacity/declare` hacia el `CapacityManager` embebido, de modo que
  cada nodo construye una vista en memoria de sus asignaciones comprometidas de
  trozo y carril. El manager expone snapshots de solo lectura para telemetría
  (`GET /v1/sorafs/capacity/state`) y aplica reservas por perfil o carril antes de
  nuevos aceptar pedidos.【crates/sorafs_node/src/capacity.rs:1】【crates/sorafs_node/src/lib.rs:60】
- El endpoint `/v1/sorafs/capacity/schedule` acepta cargas útiles `ReplicationOrderV1`
  emitidos por gobernanza. Cuando la orden apunta al proveedor local el manager
  revisa programación duplicado, verifica capacidad de chunker/lane, reserva la
  franja y devuelve un `ReplicationPlan` describiendo la capacidad restante para
  que las herramientas de orquestación continúan la ingestión. Las órdenes para
  otros proveedores se reconocen con una respuesta `ignored` para facilitar flujos
  multioperador.【crates/iroha_torii/src/routing.rs:4845】
- Hooks de completitud (por ejemplo, disparados tras el éxito de la ingestión)
  llaman a `POST /v1/sorafs/capacity/complete` para liberar reservas vía
  `CapacityManager::complete_order`. La respuesta incluye una instantánea.
  `ReplicationRelease` (totales restantes, residuales de fragmentador/lane) para que
  las herramientas de orquestación puedan colar la siguiente orden sin sondeo.
  Trabajo futuro cableará esto al pipeline del chunk store una vez que lalógica de ingestión aterrice.【crates/iroha_torii/src/routing.rs:4885】【crates/sorafs_node/src/capacity.rs:90】
- El `TelemetryAccumulator` embebido puede mutarse mediante
  `NodeHandle::update_telemetry`, permitiendo que trabajadores en segundo plano
  registren muestras de PoR/uptime y eventualmente derivadas payloads canónicos
  `CapacityTelemetryV1` sin tocar internos del planificador.【crates/sorafs_node/src/lib.rs:142】【crates/sorafs_node/src/telemetry.rs:1】

### Integraciones y trabajo futuro

- **Gobernanza**: extensor `sorafs_pin_registry_tracker.md` con telemetría de
  almacenamiento (tasa de éxito PoR, utilización de disco). Las políticas de admisión
  pueden requerir capacidad mínima o tasa mínima de éxito PoR antes de aceptar
  anuncios.
- **SDK de cliente**: expone la nueva configuración de almacenamiento (límites de
  disco, alias) para que herramientas de gestión puedan arrancar nodos
  programáticamente.
- **Telemetría**: integrar con el stack de métricas existentes (Prometheus /
  OpenTelemetry) para que las métricas de almacenamiento aparezcan en paneles de control
  observabilidad.
- **Seguridad**: ejecuta el módulo de almacenamiento en un pool dedicado de tareas
  async con contrapresión y considerar sandboxing de lecturas de chunks vía
  io_uring o pools acotados de tokio para evitar que clientes maliciosos agoten
  recursos.Este diseño mantiene el módulo de almacenamiento opcional y determinista a la
vez que otorga a los operadores los mandos necesarios para participar en la capa
SoraFS de disponibilidad de datos. Implementarlo implicará cambios en `iroha_config`,
`iroha_core`, `iroha_torii` y el gateway Norito, además del tooling de adverts
de proveedor.
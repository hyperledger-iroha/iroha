---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/node-storage.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: node-storage
כותרת: Diseño de almacenamiento del nodo SoraFS
sidebar_label: Diseño de almacenamiento del nodo
תיאור: Arquitectura de almacenamiento, cuotas y hooks del ciclo de vida para nodos Torii que alojan datas de SoraFS.
---

:::הערה Fuente canónica
Esta página refleja `docs/source/sorafs/sorafs_node_storage.md`. Mantén ambas copias sincronizadas hasta que el conjunto de documentación Sphinx heredado se לפרוש.
:::

## Diseño de almacenamiento del nodo SoraFS (בוראדור)

Esta not refina cómo un nodo Iroha (Torii) puede optar por la capa de
זמינות de datos de SoraFS y dedicar una parte del disco local para
נתחי almacenar y servir. משלים להפרטת הגילוי
`sorafs_node_client_protocol.md` y el trabajo de fixtures SF-1b al detallar la
arquitectura del lado de storage, controls de recursos y plomería de
תצורה que deben aterrizar en el nodo y en las rutas del gateway.
Las prácticas operativas viven en el
[Runbook de operaciones de nodo](./node-operations).

### אובייקטים

- Permitir que cualquier validador o processo auxiliar de Iroha exponga disco
  ocioso como proveedor SoraFS syn afectar las responsabilidades del ספר.
- Mantener el módulo de almacenamiento determinista y guiado por Norito:
  מניפסטים, מטוסי נתח, פרסות הוכחת אחזור (PoR) ופרסומות
  מוכיח בן לה פואנטה דה ורדאד.
- Impone cuotas definidas por el operador para que un nodo no agote sus propios
  recursos al aceptar demasiadas solicitudes de pin o להביא.
- Exponer salud/telemetría (muestreo PoR, latencia de fetch de chunks, presión
  de disco) hacia gobernanza y clientes.

### ארכיטקטורה דה אלטו ניבל

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

- **שער**: חשיפה של נקודות קצה HTTP Norito עבור הצעות סיכה, פניות
  de fetch de chunks, muestreo PoR y telemetria. מטענים של Valida Norito y
  enruta las solicitudes hacia el chunk store. Reusa el stack HTTP de Torii
  para evitar un nuevo daemon.
- **רישום סיכות**: estado de pin de manifests rastreado en `iroha_data_model::sorafs`
  e `iroha_core`. Cuando se acepta un manifest, el registro almacena el digest
  del manifest, digest del plan de chunk, raíz PoR y flags de capacidad del
  מוכיח.
- **אחסון נתחים**: implementación `ChunkStore` respaldada por disco que ingiere
  מפגין firmados, materializa planes de chunk usando `ChunkProfile::DEFAULT`
  y persiste chunks bajo un layout determinista. Cada chunk se asocia con un
  טביעת אצבע de contenido y metadatos PoR para que el muestreo pueda revalidar
  sin releer el archivo completo.
- **מכסה/מתזמן**: impone límites configurados por el operador (bytes máximos
  דה דיסקו, Pines pendientes máximos, fetches parlelos máximos, TTL de chunk)
  y coordina IO para que las tareas del ledger no se queden sin recursos. אל
  מתזמן también sirve pruebas PoR y solicitudes de muestreo con CPU acotada.

### תצורהקטע חדש של `iroha_config`:

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

- `enabled`: החלפת השתתפות. Cuando está en false el gateway response
  503 para endpoints de storage y el nodo no se anuncia en Discovery.
- `data_dir`: directorio raíz para datas de chunk, árboles PoR y telemetría de
  להביא. ברירת המחדל היא `<iroha.data_dir>/sorafs`.
- `max_capacity_bytes`: גבול טווח עבור נתחים של נתחים. Una tarea de
  fondo rechaza nuevos pins cuando se alcanza el límite.
- `max_parallel_fetches`: tope de concurrencia impuesto por el scheduler para
  equilibrar ancho de banda/IO de disco con la carga del validador.
- `max_pins`: número máximo de pins de manifest que acepta el nodo antes de
  פינוי אפליקר/לחץ גב.
- `por_sample_interval_secs`: cadencia para trabajos automáticos de muestreo
  PoR. Cada trabajo muestrea `N` hojas (ניתן להגדרה פור מניפסט) y emite
  eventos de telemetria. La gobernanza puede escalar `N` de forma determinista
  estableciendo la clave de metadata `profile.sample_multiplier` (אנטרו `1-4`).
  El valor puede ser un número/string único o un objeto con overrides por perfil,
  por eemplo `{"default":2,"sorafs.sf2@1.0.0":3}`.
- `adverts`: estructura usada por el generador de adverts para completar campos
  `ProviderAdvertV1` (מצביע על הימור, רמזים ל-QoS, נושאים). סי סה אומיט אל
  ברירת המחדל של nodo usa del registro de gobernanza.

פלומריה דה תצורה:

- `[sorafs.storage]` se define en `iroha_config` como `SorafsStorage` y se carga
  desde el archivo de config del nodo.
- `iroha_core` y `iroha_torii` pasan la configuración de storage hacia el
  builder del gateway y el chunk store en el arranque.
- עקיפות קיימות עבור פיתוח/בדיקה (`SORAFS_STORAGE_*`, `SORAFS_STORAGE_PIN_*`), אבל
  los despliegues de producción deben basarse en el archivo de configuración.

### Utilidades de CLI

Mientras la superficie HTTP de Torii בקצה הכבלים, אל ארגז
`sorafs_node` כולל una CLI liviana para que los operadores puedan
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
  y optionalmente emite un blob JSON `chunk_fetch_specs` para que herramientas
  במורד הזרם validen el layout.
- `export` קבל את זיהוי המניפסט y escribe el manifest/payload almacenado en
  disco (con plan JSON אופציונלי) para que los fixtures sigan siendo reproducibles
  entre entornos.

Ambos comandos imprimen un resumen Norito JSON a stdout, facilitando el uso en
תסריטים. La CLI está cubierta por una prueba de integración para asegurar que
manifests y payloads se reconstituyen correctamente antes de que lleguen las
ממשקי API de Torii.【crates/sorafs_node/tests/cli.rs:1】> Paridad HTTP
>
> שער אל Torii ahora expone helpers de solo lecture respaldados por el mismo
> `NodeHandle`:
>
> - `GET /v2/sorafs/storage/manifest/{manifest_id_hex}` — devuelve el manifest
> Norito almacenado (base64) junto con digest/metadata.【crates/iroha_torii/src/sorafs/api.rs:1207】
> - `GET /v2/sorafs/storage/plan/{manifest_id_hex}` — devuelve el plan de chunk
> determinista JSON (`chunk_fetch_specs`) עבור כלי עבודה במורד הזרם.【crates/iroha_torii/src/sorafs/api.rs:1259】
>
> נקודות הקצה של Estos relejan la salida del CLI para que los pipelines puedan
> pasar de scripts locales a probes HTTP sin cambiar מנתחי.【crates/iroha_torii/src/sorafs/api.rs:1207】【crates/iroha_torii/src/sorafs/api.rs:1259】

### Ciclo de vida del nodo

1. **ארנק**:
   - Si el almacenamiento está habilitado el nodo inicializa el chunk store con
     המדריך והגדרות היכולות. Esto incluye verificar o crear la
     base de datos de manifests PoR y reproducir manifests fijados para calentar
     מטמונים.
   - Rutas del gateway של הרשם SoraFS (נקודות קצה Norito JSON POST/GET עבור סיכה,
     להביא, muestreo PoR, telemetria).
   - Lanzar el worker de muestreo PoR y el monitor de cuotas.
2. **גילוי / פרסומות**:
   - מסמכים כלליים `ProviderAdvertV1` usando la capacidad/salud בפועל,
     firmarlos con la clave aprobada por el consejo y publicarlos vía גילוי.
     ארה"ב לה נואבה רשימה `profile_aliases` עבור que los handles canónicos y
3. **Flujo de pin**:
   - El gateway recibe un manifest firmado (כולל תוכנית נתח, raíz PoR,
     firmas del consejo). תוקף לרשימת הכינוי (`sorafs.sf1@1.0.0` דרישה)
     y asegura que el plan de chunk עולה בקנה אחד עם metadata del manifest.
   - Verifica cuotas. Si capacidad/límites de pins se excederían responde con un
     שגיאה פוליטית (Norito estructurado).
   - Streamea datas de chunk hacia `ChunkStore`, verificando digests durante la
     ingestión. Actualiza árboles PoR y almacena metadata del manifest en el
     רישום.
4. **Flujo de fetch**:
   - Sirve solicitudes de rango de chunk desde disco. אל מתזמן להטיל
     `max_parallel_fetches` y devuelve `429` cuando está saturado.
   - Emite telemetría estructurada (Norito JSON) עם לטיות, בתים שירותים y
     Conteos de error para monitoreo במורד הזרם.
5. **Muestreo PoR**:
   - El worker selecciona manifestes proporcionalmente al peso (por ejemplo, bytes
     almacenados) y ejecuta muestreo determinista usando el árbol PoR del chunk store.
   - Persiste resultados para auditorías de gobernanza e incluye resúmenes en
     adverts de proveedor / נקודות קצה de telemetria.
6. **גירוש / cumplimiento de cuotas**:
   - Cuando se alcanza la capacidad el nodo rechaza nuevos pins por defecto.
     אופציונלי, אופציונליות של מדיניות הגירוש
     (por ejemplo, TTL, LRU) una vez que el modelo de gobernanza se acuerde; פור
     ahora el diseño asume cuotas estrictas y operaciones de unpin iniciadas por
     אל מפעיל.### הצהרת תזמון ושילוב תזמון

- Torii ahora retransmite actualizaciones de `CapacityDeclarationRecord` desde
  `/v2/sorafs/capacity/declare` hacia el `CapacityManager` embebido, de modo que
  cada nodo construye una vista en memoria de sus asignaciones comprometidas de
  chunker y lane. המנהל חושף את צילומי ההרצאה הסולו עבור טלמטריה
  (`GET /v2/sorafs/capacity/state`) y aplica reservas por perfil o lane antes de
  aceptar nuevos pedidos.【crates/sorafs_node/src/capacity.rs:1】【crates/sorafs_node/src/lib.rs:60】
- נקודת קצה El `/v2/sorafs/capacity/schedule` מטענים מטעמים `ReplicationOrderV1`
  emitidos por gobernanza. Cuando la orden apunta al proveeor המנהל המקומי
  revisa scheduling duplicado, verifica capacidad de chunker/lane, reserva la
  franja y devuelve un `ReplicationPlan` תיאור ל-capacidad restante para
  que las herramientas de orquestación continúen la ingestión. Las órdenes para
  otros proveedores se reconocen con una respuesta `ignored` para facilitar flujos
  multi-operador.【crates/iroha_torii/src/routing.rs:4845】
- Hooks de completitud (לפי אימפלו, דיסparados tras el éxito de la ingestión)
  llaman a `POST /v2/sorafs/capacity/complete` para liberar reservas vía
  `CapacityManager::complete_order`. התשובות כוללות תמונת מצב
  `ReplicationRelease` (כולל restantes, residuales de chunker/lane) para que
  las herramientas de orquestación puedan encolar la siguiente orden sin polling.
  Trabajo futuro cableará esto al pipeline del chunk store Una vez que la
  lógica de ingestión aterrice.【crates/iroha_torii/src/routing.rs:4885】【crates/sorafs_node/src/capacity.rs:90】
- El `TelemetryAccumulator` embebido puede mutarse mediante
  `NodeHandle::update_telemetry`, permitiendo que workers in segundo plano
  רישום רשימות של PoR/uptime y eventualmente riveloads payloads canónicos
  `CapacityTelemetryV1` sin tocar internos del scheduler.【crates/sorafs_node/src/lib.rs:142】【crates/sorafs_node/src/telemetry.rs:1】

### אינטגרציות ופיתוח עתידי

- **Governanza**: מאריך `sorafs_pin_registry_tracker.md` עם telemetría de
  אחסון (tasa de éxito PoR, utilización de disco). לאס פוליטיקה דה אדמיסיון
  pueden requerir capacidad mínima o tasa mínima de éxito PoR antes de aceptar
  פרסומות.
- **SDKs de cliente**: exponer la nueva configuración de storage (limites de
  דיסקו, כינוי) para que herramientas de gestión puedan bootstrapear nodos
  programáticamente.
- **Telemetría**: integrar con el stack de métricas existente (Prometheus /
  OpenTelemetry) para que las métricas de storage aparezcan en לוחות מחוונים de
  observabilidad.
- **Seguridad**: ejecutar el módulo de storage en un pool dedicado de tareas
  אסינכרון עם לחץ אחורי וארגז חול שקול את ההרצאות של נתחים דרך
  io_uring o pools acotados de tokio para evitar que clientes maliciosos agoten
  recursos.Este diseño mantiene el módulo de almacenamiento optional y determinista a la
vez que otorga a los operadores los knobs necesarios para participar en la capa
SoraFS זמינות נתונים. Implementarlo implicará cambios en `iroha_config`,
`iroha_core`, `iroha_torii` y el gateway Norito, אקדמיה של כלים לפרסומות
דה מוכיח.
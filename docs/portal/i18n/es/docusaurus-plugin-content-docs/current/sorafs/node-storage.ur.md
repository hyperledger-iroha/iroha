---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/node-storage.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: almacenamiento de nodo
título: Diseño de almacenamiento de nodo SoraFS
sidebar_label: diseño de almacenamiento de nodos
descripción: Host de datos SoraFS Nodos Torii Arquitectura de almacenamiento Cuotas Ganchos de ciclo de vida
---

:::nota مستند ماخذ
:::

## SoraFS Diseño de almacenamiento de nodos (borrador)

یہ نوٹ واضح کرتا ہے کہ Iroha (Torii) nodo کس طرح SoraFS capa de disponibilidad de datos میں opt-in کر سکتا ہے اور disco local کا ایک حصہ fragmentos ذخیرہ/سرو کرنے کے لیے مختص کر سکتا ہے۔ یہ `sorafs_node_client_protocol.md` especificación de descubrimiento اور SF-1b کام کی تکمیل کرتا ہے، اور arquitectura del lado de almacenamiento, controles de recursos, اور configuración de plomería بیان کرتا ہے جو nodo اور rutas de código de puerta de enlace میں شامل ہونا ضروری ہے۔ Taladros عملی آپریشنل
[Runbook de operaciones de nodo](./node-operations) میں موجود ہیں۔

### Metas

- Validador de validación, proceso Iroha auxiliar, disco de repuesto, proveedor SoraFS, exposición de proveedor, libro mayor central, libro mayor, registro de datos
- módulo de almacenamiento determinista, Norito: manifiestos, planes de fragmentos, raíces de prueba de recuperación (PoR), anuncios de proveedores, fuente de verdad
- las cuotas definidas por el operador aplican solicitudes de pin/fetch del nodo کرنا تاکہ بہت زیادہ قبول کر کے اپنے recursos ختم نہ کر لے۔
- salud/telemetría (muestreo PoR, latencia de recuperación de fragmentos, presión del disco) کو gobernanza اور clientes تک واپس پہنچانا۔

### Arquitectura de alto nivel```
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

- **Puerta de enlace**: Los puntos finales HTTP Norito exponen varias propuestas de pines, solicitudes de recuperación de fragmentos, muestreo PoR y telemetría. Las cargas útiles Norito validan las solicitudes de کرتا ہے اور y el almacén de fragmentos میں marshal کرتا ہے۔ نیا daemon بنانے کے بجائے Torii Reutilización de pila HTTP کرتا ہے۔
- **Registro de PIN**: estado del PIN de manifiesto, `iroha_data_model::sorafs` y `iroha_core`, pista de seguimiento Aceptación de manifiesto ہونے پر resumen de manifiesto de registro, resumen de plan de fragmentos, raíz de PoR y registro de indicadores de capacidad del proveedor کرتا ہے۔
- **Almacenamiento de fragmentos**: implementación `ChunkStore` respaldada en disco, ingesta de manifiestos firmados, `ChunkProfile::DEFAULT`, planes de fragmentos materializados, y fragmentos, diseño determinista, persistencia. ہے۔ ہر huella digital de contenido fragmentado اور metadatos PoR کے ساتھ asociar ہوتا ہے تاکہ muestreo بغیر پورا فائل دوبارہ پڑھنے کے revalidar ہو سکے۔
- **Cuota/Programador**: los límites configurados por el operador (bytes máximos de disco, pines pendientes máximos, recuperaciones paralelas máximas, TTL de fragmentos) imponen el cumplimiento de las tareas del libro mayor. Pruebas de PoR del programador, solicitudes de muestreo, CPU limitada, servicio de بھی کرتا ہے۔

### Configuración

`iroha_config` میں نیا sección شامل کریں:

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
```- `enabled`: alternancia de participación۔ falsos puntos finales de almacenamiento de puerta de enlace پر 503 دیتا ہے اور descubrimiento de nodos میں publicidad نہیں کرتا۔
- `data_dir`: fragmentos de datos, árboles PoR y recuperación de telemetría en el directorio raíz. Predeterminado `<iroha.data_dir>/sorafs` ہے۔
- `max_capacity_bytes`: datos de fragmentos anclados کے لیے límite estricto۔ Límite de tareas en segundo plano پر پہنچنے پر نئے pines rechazados کرتی ہے۔
- `max_parallel_fetches`: programador, límite de simultaneidad, ancho de banda/disco, IO, validador de carga de trabajo, equilibrio, equilibrio
- `max_pins`: pines de manifiesto کی máximo تعداد جو nodo قبول کرتا ہے اس سے پہلے desalojo/contrapresión aplicar ہو۔
- `por_sample_interval_secs`: trabajos de muestreo PoR automáticos کی cadencia۔ ہر trabajo `N` deja muestra کرتا ہے (por manifiesto configurable) اور eventos de telemetría emiten کرتا ہے۔ Clave de metadatos de gobernanza `profile.sample_multiplier` (entero `1-4`) سے `N` کو escala determinista کر سکتی ہے۔ Valor ایک número único/cadena یا anulaciones por perfil y objeto ہو سکتا ہے، مثلاً `{"default":2,"sorafs.sf2@1.0.0":3}`۔
- `adverts`: estructura y campo del generador de anuncios del proveedor `ProviderAdvertV1` (puntero de participación, sugerencias de QoS, temas) بھرنے کے لیے استعمال کرتا ہے۔ اگر omitir ہو تو valores predeterminados del registro de gobernanza de nodos استعمال کرتا ہے۔

Configurar plomería:- `[sorafs.storage]` `iroha_config` میں `SorafsStorage` کے طور پر definir ہے اور archivo de configuración de nodo سے cargar ہوتا ہے۔
- `iroha_core` اور `iroha_torii` configuración de almacenamiento کو inicio پر gateway builder اور fragment store میں thread کرتے ہیں۔
- El entorno de desarrollo/prueba anula el software (`SORAFS_STORAGE_*`, `SORAFS_STORAGE_PIN_*`), las implementaciones de producción y el archivo de configuración de confianza.

### Utilidades CLI

Incluye Torii, cable de superficie HTTP, caja `sorafs_node`, CLI delgada, configuración de operadores, backend persistente, ingestión/exportación. script de ejercicios کر سکیں۔【crates/sorafs_node/src/bin/sorafs-node.rs:1】

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin \
  --plan-json-out ./plan.json
```

- `ingest` Norito manifiesto codificado `.to` con bytes de carga útiles coincidentes esperados کرتا ہے۔ یہ manifiesto کے perfil de fragmentación کرتا ہے، reconstrucción del plan de fragmentos کرتا ہے، resumen de paridad aplicar کرتا ہے، los archivos de fragmentos persisten کرتا ہے، اور opcionalmente `chunk_fetch_specs` emisión de blobs JSON کرتا ہے تاکہ diseño de herramientas posteriores control de cordura کر سکے۔
- `export` ID de manifiesto قبول کرتا ہے اور manifiesto almacenado/carga útil کو disco پر لکھتا ہے (plan opcional JSON کے ساتھ) تاکہ entornos de accesorios میں reproducible رہیں۔

دونوں comandos salida estándar پر Norito Resumen JSON پرنٹ کرتے ہیں، جسے scripts میں pipe کرنا آسان ہے۔ Prueba de integración CLI cubierta ہے تاکہ manifiestos/cargas útiles de ida y vuelta درست رہیں جب تک Torii API نہ آئیں۔【crates/sorafs_node/tests/cli.rs:1】> paridad HTTP
>
> Los asistentes de solo lectura de la puerta de enlace Torii exponen el software basado en `NodeHandle`:
>
> - `GET /v1/sorafs/storage/manifest/{manifest_id_hex}` — resumen/metadatos del manifiesto Norito almacenado (base64) کے ساتھ واپس کرتا ہے۔【crates/iroha_torii/src/sorafs/api.rs:1207】
> - `GET /v1/sorafs/storage/plan/{manifest_id_hex}` — plan de fragmentos determinista JSON (`chunk_fetch_specs`) herramientas posteriores کے لیے واپس کرتا ہے۔【crates/iroha_torii/src/sorafs/api.rs:1259】
>
> یہ puntos finales Salida CLI کو espejo کرتے ہیں تاکہ canalizaciones scripts locales سے Sondas HTTP پر بغیر analizador بدلے منتقل ہو سکیں۔【crates/iroha_torii/src/sorafs/api.rs:1207】【crates/iroha_torii/src/sorafs/api.rs:1259】

### Ciclo de vida del nodo1. **Inicio**:
   - almacenamiento habilitado ہو تو directorio configurado del nodo اور capacidad کے ساتھ inicialización del almacén de fragmentos کرتا ہے۔ اس میں PoR base de datos de manifiestos verificar/crear کرنا اور manifiestos anclados reproducir کر کے cachés calientes کرنا شامل ہے۔
   - Registro de rutas de puerta de enlace SoraFS کریں (puntos finales Norito JSON POST/GET para pin, recuperación, muestra de PoR, telemetría) ۔
   - Trabajador de muestreo de PoR y generación de monitor de cuotas کریں۔
2. **Descubrimiento/Anuncios**:
3. **Fijar flujo de trabajo**:
   - Manifiesto firmado de la puerta de enlace وصول کرتا ہے (plan fragmentado, raíz PoR, firmas del consejo شامل) ۔ lista de alias validar کریں (se requiere `sorafs.sf1@1.0.0`) اور fragment plan کو manifiesto metadatos سے coincidir کریں۔
   - Verificación de cuotas کریں۔ Los límites de capacidad/pin exceden el error de política (estructurado Norito)
   - Datos fragmentados کو `ChunkStore` میں stream کریں اور ingest کے دوران resúmenes verificar کریں۔ Actualización de árboles PoR کریں اور registro de metadatos de manifiesto میں tienda کریں۔
4. **Obtener flujo de trabajo**:
   - Las solicitudes de rango de fragmentos de disco sirven کریں۔ El programador `max_parallel_fetches` aplica کرتا ہے اور saturado ہونے پر `429` واپس کرتا ہے۔
   - La telemetría estructurada (Norito JSON) emite una gran latencia, bytes servidos y recuentos de errores y monitoreo descendente.
5. **Muestreo de PoR**:
   - Manifiestos del trabajador کو peso (مثلاً bytes almacenados) کے تناسب سے seleccionar کرتا ہے اور almacén de fragmentos کے árbol PoR سے muestreo determinista کرتا ہے۔- Resultados کو auditorías de gobernanza کے لیے persisten کریں اور anuncios de proveedores/puntos finales de telemetría میں resúmenes شامل کریں۔
6. **Desalojo / aplicación de cuotas**:
   - Capacidad پہنچنے پر nodo predeterminado طور پر نئے pines rechazados کرتا ہے۔ Las políticas de desalojo de operadores opcionales (مثلاً TTL, LRU) configuran el modelo de gobernanza de کر سکتے ہیں جب طے ہو جائے؛ فی الحال diseñar cuotas estrictas اور operaciones de desanclaje iniciadas por el operador فرض کرتا ہے۔

### Declaración de capacidad e integración de programación- Torii اب `/v1/sorafs/capacity/declare` سے `CapacityDeclarationRecord` actualizaciones integradas `CapacityManager` تک relé کرتا ہے، تاکہ ہر nodo اپنی asignaciones de fragmentos/carriles comprometidos کا vista en memoria بنائے۔ Telemetría del administrador کے لیے instantáneas de solo lectura (`GET /v1/sorafs/capacity/state`) exponen کرتا ہے اور نئے pedidos قبول کرنے سے پہلے por perfil یا reservas por carril aplican کرتا ہے۔【crates/sorafs_node/src/capacity.rs:1】【crates/sorafs_node/src/lib.rs:60】
- Cargas útiles `ReplicationOrderV1` emitidas por el gobierno del punto final `ReplicationOrderV1` قبول کرتا ہے۔ جب ordenar proveedor local کو objetivo کرے تو administrador de programación duplicada چیک کرتا ہے، capacidad de fragmentación/carril verificar کرتا ہے، reserva de segmento کرتا ہے، اور `ReplicationPlan` واپس کرتا ہے جو capacidad restante بیان کرے تاکہ ingestión de herramientas de orquestación جاری رکھ سکے۔ دوسرے proveedores کے pedidos کو `ignored` respuesta سے reconocer کیا جاتا ہے تاکہ flujos de trabajo multioperador آسان ہوں۔【crates/iroha_torii/src/routing.rs:4845】
- Ganchos de finalización (مثلاً ingestión کامیاب ہونے کے بعد) `POST /v1/sorafs/capacity/complete` کو hit کرتے ہیں تاکہ `CapacityManager::complete_order` کے ذریعے liberación de reservas ہوں۔ Respuesta میں `ReplicationRelease` instantánea (totales restantes, residuos de fragmentación/carril) شامل ہوتا ہے تاکہ sondeo de herramientas de orquestación کے بغیر اگلا cola de pedidos کر سکے۔ یہ بعد میں tubería de almacenamiento de fragmentos کے ساتھ cable ہوگا جب ingestión lógica tierra ہو جائے۔【crates/iroha_torii/src/routing.rs:4885】【crates/sorafs_node/src/capacity.rs:90】- Integrado `TelemetryAccumulator` کو `NodeHandle::update_telemetry` کے ذریعے mutate کیا جا سکتا ہے، جس سے registros de muestras de PoR/tiempo de actividad de trabajadores en segundo plano کرتے ہیں اور Las cargas útiles canónicas `CapacityTelemetryV1` derivan las partes internas del programador چھیڑے۔【crates/sorafs_node/src/lib.rs:142】【crates/sorafs_node/src/telemetry.rs:1】

### Integraciones y trabajo futuro

- **Gobernanza**: `sorafs_pin_registry_tracker.md` کو telemetría de almacenamiento (tasa de éxito de PoR, utilización del disco) کے ساتھ ampliar کریں۔ Los anuncios de políticas de admisión aceptan کرنے سے پہلے capacidad mínima یا tasa mínima de éxito de PoR requiere کر سکتی ہیں۔
- **SDK de cliente**: la configuración de almacenamiento (límites de disco, alias) expone las herramientas de administración de aplicaciones mediante programación con arranque de nodos.
- **Telemetría**: pila de métricas múltiples (Prometheus / OpenTelemetry) کے ساتھ integrar کریں تاکہ paneles de observabilidad de métricas de almacenamiento میں نظر آئیں۔
- **Seguridad**: módulo de almacenamiento, grupo de tareas asíncrono dedicado, contrapresión, lecturas de fragmentos, io_uring, grupos tokio delimitados, sandbox, etc. تاکہ los recursos de clientes maliciosos agotan نہ کر سکیں۔یہ módulo de almacenamiento de diseño کو opcional اور determinista رکھتا ہے جبکہ operadores کو SoraFS capa de disponibilidad de datos میں حصہ لینے کے لیے ضروری perillas فراہم کرتا ہے۔ Utilice las herramientas de publicidad del proveedor `iroha_config`, `iroha_core`, `iroha_torii` y Norito gateway. میں تبدیلیاں مانگتا ہے۔
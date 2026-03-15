---
lang: es
direction: ltr
source: docs/source/da/ingest_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1bf79d000e0536da04eafac6c0d896b1bf8f0c454e1bf4c4b97ba22c7c7f5db1
source_last_modified: "2026-01-22T15:38:30.661072+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Sora Nexus Plan de ingesta de disponibilidad de datos

_Redactado: 2026-02-20 - Propietario: Core Protocol WG / Equipo de almacenamiento / DA WG_

El flujo de trabajo DA-2 amplía Torii con una API de ingesta de blobs que emite Norito
Metadatos y semillas de replicación SoraFS. Este documento recoge la propuesta
esquema, superficie API y flujo de validación para que la implementación pueda continuar sin
bloqueo de simulaciones pendientes (seguimientos DA-1). Todos los formatos de carga útil DEBEN
utilice códecs Norito; no se permiten alternativas de serde/JSON.

## Metas

- Aceptar manchas grandes (segmentos Taikai, sidecares de carril, artefactos de gobernanza)
  deterministamente sobre Torii.
- Producir manifiestos canónicos Norito que describan el blob, los parámetros del códec,
  perfil de borrado y política de retención.
- Persistir en los metadatos de fragmentos en el almacenamiento activo SoraFS y poner en cola los trabajos de replicación.
- Publicar intenciones de pin + etiquetas de política en el registro y gobierno SoraFS
  observadores.
- Exponer los recibos de admisión para que los clientes recuperen la prueba determinista de la publicación.

## Superficie API (Torii)

```
POST /v2/da/ingest
Content-Type: application/norito+v1
```

La carga útil es un `DaIngestRequest` codificado con Norito. Uso de respuestas
`application/norito+v1` y devolver `DaIngestReceipt`.

| Respuesta | Significado |
| --- | --- |
| 202 Aceptado | Blob en cola para fragmentación/replicación; recibo devuelto. |
| 400 Solicitud incorrecta | Violación de esquema/tamaño (ver comprobaciones de validación). |
| 401 No autorizado | Token de API faltante o no válido. |
| 409 Conflicto | `client_blob_id` duplicado con metadatos que no coinciden. |
| 413 Carga útil demasiado grande | Supera el límite de longitud del blob configurado. |
| 429 Demasiadas solicitudes | Límite de tasa alcanzado. |
| 500 Error interno | Fallo inesperado (registrado + alerta). |

```
GET /v2/da/proof_policies
Accept: application/json | application/x-norito
```

Devuelve un `DaProofPolicyBundle` versionado derivado del catálogo de carriles actual.
El paquete anuncia `version` (actualmente `1`), un `policy_hash` (hash del
lista de políticas ordenada) y entradas `policies` que contienen `lane_id`, `dataspace_id`,
`alias`, y el `proof_scheme` aplicado (`merkle_sha256` hoy; los carriles KZG son
rechazado por ingesta hasta que los compromisos de KZG estén disponibles). El encabezado del bloque ahora
se compromete con el paquete a través de `da_proof_policies_hash`, para que los clientes puedan anclar el
Política activa establecida al verificar compromisos o pruebas de DA. Obtener este punto final
antes de crear pruebas para garantizar que coincidan con la política del carril y la situación actual.
paquete de hash. Los puntos finales de lista de compromiso/prueba llevan el mismo paquete, por lo que los SDK
No es necesario un viaje de ida y vuelta adicional para vincular una prueba al conjunto de políticas activas.

```
GET /v2/da/proof_policy_snapshot
Accept: application/json | application/x-norito
```

Devuelve un `DaProofPolicyBundle` que contiene la lista de políticas ordenada más un
`policy_hash` para que los SDK puedan fijar la versión utilizada cuando se produjo un bloque. el
El hash se calcula sobre la matriz de políticas codificada con Norito y cambia cada vez que se
El `proof_scheme` del carril se actualiza, lo que permite a los clientes detectar la desviación entre
pruebas almacenadas en caché y la configuración de la cadena.

## Esquema Norito propuesto

```rust
/// Top-level ingest request.
pub struct DaIngestRequest {
    pub client_blob_id: BlobDigest,      // submitter-chosen identifier
    pub lane_id: LaneId,                 // target Nexus lane
    pub epoch: u64,                      // epoch blob belongs to
    pub sequence: u64,                   // monotonic sequence per (lane, epoch)
    pub blob_class: BlobClass,           // TaikaiSegment, GovernanceArtifact, etc.
    pub codec: BlobCodec,                // e.g. "cmaf", "pdf", "norito-batch"
    pub erasure_profile: ErasureProfile, // parity configuration
    pub retention_policy: RetentionPolicy,
    pub chunk_size: u32,                 // bytes (must align with profile)
    pub total_size: u64,
    pub compression: Compression,        // Identity, gzip, deflate, or zstd
    pub norito_manifest: Option<Vec<u8>>, // optional pre-built manifest
    pub payload: Vec<u8>,                 // raw blob data (<= configured limit)
    pub metadata: ExtraMetadata,          // optional key/value metadata map
    pub submitter: PublicKey,             // signing key of caller
    pub signature: Signature,             // canonical signature over request
}

pub enum BlobClass {
    TaikaiSegment,
    NexusLaneSidecar,
    GovernanceArtifact,
    Custom(u16),
}

pub struct ErasureProfile {
    pub data_shards: u16,
    pub parity_shards: u16,
    pub chunk_alignment: u16, // chunks per availability slice
    pub fec_scheme: FecScheme,
}

pub struct RetentionPolicy {
    pub hot_retention_secs: u64,
    pub cold_retention_secs: u64,
    pub required_replicas: u16,
    pub storage_class: StorageClass,
    pub governance_tag: GovernanceTag,
}

pub struct ExtraMetadata {
    pub items: Vec<MetadataEntry>,
}

pub struct MetadataEntry {
    pub key: String,
    pub value: Vec<u8>,
    pub visibility: MetadataVisibility, // public vs governance-only
}

pub enum MetadataVisibility {
    Public,
    GovernanceOnly,
}

pub struct DaIngestReceipt {
    pub client_blob_id: BlobDigest,
    pub lane_id: LaneId,
    pub epoch: u64,
    pub blob_hash: BlobDigest,          // BLAKE3 of raw payload
    pub chunk_root: BlobDigest,         // Merkle root after chunking
    pub manifest_hash: BlobDigest,      // Norito manifest hash
    pub storage_ticket: StorageTicketId,
    pub pdp_commitment: Option<Vec<u8>>,     // Norito-encoded PDP bytes
    #[norito(default)]
    pub stripe_layout: DaStripeLayout,   // total_stripes, shards_per_stripe, row_parity_stripes
    pub queued_at_unix: u64,
    #[norito(default)]
    pub rent_quote: DaRentQuote,        // XOR rent + incentives derived from policy
    pub operator_signature: Signature,
}
```> Nota de implementación: las representaciones canónicas de Rust para estas cargas útiles ahora se encuentran en
> `iroha_data_model::da::types`, con envoltorios de solicitud/recibo en `iroha_data_model::da::ingest`
> y la estructura del manifiesto en `iroha_data_model::da::manifest`.

El campo `compression` anuncia cómo las personas que llaman prepararon la carga útil. Torii acepta
`identity`, `gzip`, `deflate` e `zstd`, descomprimiendo de forma transparente los bytes anteriores
hash, fragmentación y verificación de manifiestos opcionales.

### Lista de verificación de validación

1. Verifique que el encabezado de la solicitud Norito coincida con `DaIngestRequest`.
2. Falla si `total_size` difiere de la longitud de carga útil canónica (descomprimida) o excede el máximo configurado.
3. Aplique la alineación `chunk_size` (potencia de dos, = 2.
5. `retention_policy.required_replica_count` debe respetar la línea base de gobernanza.
6. Verificación de firma contra hash canónico (excluido el campo de firma).
7. Rechace el duplicado `client_blob_id` a menos que el hash de carga útil + los metadatos sean idénticos.
8. Cuando se proporcione `norito_manifest`, verifique las coincidencias de esquema + hash recalculadas
   manifiesto después de la fragmentación; de lo contrario, el nodo genera un manifiesto y lo almacena.
9. Aplicar la política de replicación configurada: Torii reescribe el archivo enviado
   `RetentionPolicy` con `torii.da_ingest.replication_policy` (ver
   `replication_policy.md`) y rechaza manifiestos prediseñados cuya retención
   Los metadatos no coinciden con el perfil aplicado.

### Flujo de fragmentación y replicación1. Divida la carga útil en `chunk_size`, calcule BLAKE3 por fragmento + raíz de Merkle.
2. Compile Norito `DaManifestV1` (nueva estructura) que captura compromisos de fragmentos (rol/group_id),
   diseño de borrado (recuentos de paridad de filas y columnas más `ipa_commitment`), política de retención,
   y metadatos.
3. Ponga en cola los bytes del manifiesto canónico en `config.da_ingest.manifest_store_dir`
   (Torii escribe archivos `manifest.encoded` codificados por carril/época/secuencia/ticket/huella digital) por lo que SoraFS
   la orquestación puede ingerirlos y vincular el ticket de almacenamiento a datos persistentes.
4. Publicar intenciones de pin a través de `sorafs_car::PinIntent` con etiqueta de gobernanza + política.
5. Emita el evento Norito `DaIngestPublished` para notificar a los observadores (clientes ligeros,
   gobernanza, análisis).
6. Devuelva `DaIngestReceipt` (firmado por la clave de servicio DA Torii) y agregue el
   Encabezado de respuesta `Sora-PDP-Commitment` que contiene la codificación base64 Norito
   del compromiso derivado para que los SDK puedan almacenar la semilla de muestreo de inmediato.
   El recibo ahora incorpora `rent_quote` (un `DaRentQuote`) y `stripe_layout`
   para que los remitentes puedan revelar las obligaciones XOR, la participación de reserva, las expectativas de bonificación de PDP/PoTR,
   y las dimensiones de la matriz de borrado 2D junto con los metadatos del ticket de almacenamiento antes de comprometer fondos.
7. Metadatos de registro opcionales:
   - `da.registry.alias`: cadena de alias UTF-8 pública y sin cifrar para generar la entrada del registro PIN.
   - `da.registry.owner`: cadena `AccountId` pública y sin cifrar para registrar la propiedad del registro.
   Torii los copia en el `DaPinIntent` generado para que el procesamiento de pines posteriores pueda vincular alias
   y propietarios sin volver a analizar el mapa de metadatos sin procesar; Los valores con formato incorrecto o vacíos se rechazan durante
   validación de ingesta.

## Actualizaciones de almacenamiento/registro

- Amplíe `sorafs_manifest` con `DaManifestV1`, lo que permite el análisis determinista.
- Agregar nueva secuencia de registro `da.pin_intent` con referencia de carga útil versionada
  hash de manifiesto + ID del ticket.
- Actualizar los canales de observabilidad para rastrear la latencia de ingesta, fragmentar el rendimiento,
  trabajo pendiente de replicación y recuentos de errores.
- Las respuestas Torii `/status` ahora incluyen una matriz `taikai_ingest` que muestra las últimas
  Latencia del codificador a la ingesta, deriva del borde en vivo y contadores de errores por (clúster, flujo), habilitando DA-9
  paneles para ingerir instantáneas de estado directamente desde los nodos sin raspar Prometheus.

## Estrategia de prueba- Pruebas unitarias para validación de esquemas, verificación de firmas, detección de duplicados.
- Pruebas doradas que verifican la codificación Norito de `DaIngestRequest`, manifiesto y recibo.
- Arnés de integración que activa el registro simulado SoraFS +, afirmando flujos de fragmentos + pines.
- Pruebas de propiedad que cubren perfiles de borrado aleatorios y combinaciones de retención.
- Fuzzing de las cargas útiles Norito para proteger contra metadatos mal formados.
- Accesorios dorados para cada clase de blobs en vivo.
  `fixtures/da/ingest/manifests/<blob_class>/manifest.{norito.hex,json}` con un fragmento complementario
  listado en `fixtures/da/ingest/sample_chunk_records.txt`. La prueba ignorada
  `regenerate_da_ingest_fixtures` actualiza los dispositivos, mientras
  `manifest_fixtures_cover_all_blob_classes` falla tan pronto como se agrega una nueva variante `BlobClass`
  sin actualizar el paquete Norito/JSON. Esto mantiene Torii, SDK y documentos honestos siempre que DA-2
  acepta una nueva superficie de blob.【fixtures/da/ingest/README.md:1】【crates/iroha_torii/src/da/tests.rs:2902】

## Herramientas CLI y SDK (DA-8)- `iroha app da submit` (nuevo punto de entrada CLI) ahora incluye el generador/editor de ingesta compartida para que los operadores
  puede ingerir manchas arbitrarias fuera del flujo del paquete Taikai. El comando vive en
  `crates/iroha_cli/src/commands/da.rs:1` y consume una carga útil, un perfil de borrado/retención y
  archivos de manifiesto/metadatos opcionales antes de firmar el `DaIngestRequest` canónico con la CLI
  clave de configuración. Las ejecuciones exitosas persisten `da_request.{norito,json}` e `da_receipt.{norito,json}` en
  `artifacts/da/submission_<timestamp>/` (anular mediante `--artifact-dir`) para que los artefactos de liberación puedan
  registre los bytes Norito exactos utilizados durante la ingesta.
- El comando predeterminado es `client_blob_id = blake3(payload)` pero acepta anulaciones a través de
  `--client-blob-id`, respeta los mapas JSON de metadatos (`--metadata-json`) y los manifiestos generados previamente
  (`--manifest`) y admite `--no-submit` para preparación fuera de línea más `--endpoint` para personalización
  Anfitriones Torii. El JSON del recibo se imprime en la salida estándar además de escribirse en el disco, cerrando el
  Requisito de herramientas DA-8 “submit_blob” y desbloqueo del trabajo de paridad del SDK.
- `iroha app da get` agrega un alias centrado en DA para el orquestador de fuentes múltiples que ya impulsa
  `iroha app sorafs fetch`. Los operadores pueden apuntar a artefactos de manifiesto + plan de fragmentos (`--manifest`,
  `--plan`, `--manifest-id`) **o** simplemente pase un ticket de almacenamiento Torii a través de `--storage-ticket`. cuando el
  Se utiliza la ruta del ticket, la CLI extrae el manifiesto de `/v2/da/manifests/<ticket>` y persiste el paquete.
  bajo `artifacts/da/fetch_<timestamp>/` (anular con `--manifest-cache-dir`), deriva el **manifiesto
  hash** para `--manifest-id` y luego ejecuta el orquestador con el `--gateway-provider` suministrado.
  lista. La verificación de la carga útil aún depende del resumen CAR/`blob_hash` integrado mientras que la identificación de la puerta de enlace es
  ahora el hash del manifiesto para que los clientes y validadores compartan un único identificador de blob. Todos los mandos avanzados de
  la superficie del buscador SoraFS intacta (sobres de manifiesto, etiquetas de cliente, cachés de protección, transporte de anonimato)
  anulaciones, exportación de marcadores y rutas `--output`), y el punto final del manifiesto se puede anular mediante
  `--manifest-endpoint` para hosts Torii personalizados, por lo que las comprobaciones de disponibilidad de un extremo a otro se encuentran completamente bajo el
  Espacio de nombres `da` sin duplicar la lógica del orquestador.
- `iroha app da get-blob` extrae manifiestos canónicos directamente desde Torii a través de `GET /v2/da/manifests/{storage_ticket}`.
  El comando ahora etiqueta los artefactos con el hash de manifiesto (blob id), escribiendo
  `manifest_{manifest_hash}.norito`, `manifest_{manifest_hash}.json` y `chunk_plan_{manifest_hash}.json`
  bajo `artifacts/da/fetch_<timestamp>/` (o un `--output-dir` proporcionado por el usuario) mientras se repite exactamente
  Se requiere la invocación `iroha app da get` (incluido `--manifest-id`) para la recuperación del orquestador de seguimiento.
  Esto mantiene a los operadores fuera de los directorios del spool de manifiesto y garantiza que el buscador siempre use el
  artefactos firmados emitidos por Torii. El cliente JavaScript Torii refleja este flujo a través de
  `ToriiClient.getDaManifest(storageTicketHex)` mientras que Swift SDK ahora expone
  `ToriiClient.getDaManifestBundle(...)`. Ambos devuelven los bytes Norito decodificados, el JSON del manifiesto, el hash del manifiesto,y plan de fragmentos para que las personas que llaman al SDK puedan hidratar las sesiones de Orchestrator sin tener que gastar en la CLI y Swift
  Los clientes también pueden llamar a `fetchDaPayloadViaGateway(...)` para canalizar esos paquetes a través del sistema nativo.
  Envoltorio del orquestador SoraFS. 【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240】
- Las respuestas `/v2/da/manifests` ahora aparecen `manifest_hash` y ambos asistentes CLI + SDK (`iroha app da get`,
  `ToriiClient.fetchDaPayloadViaGateway` y los contenedores de puerta de enlace Swift/JS) tratan este resumen como el
  identificador de manifiesto canónico mientras continúa verificando las cargas útiles con el hash CAR/blob integrado.
- `iroha app da rent-quote` calcula el alquiler determinista y los desgloses de incentivos para un tamaño de almacenamiento suministrado
  y ventana de retención. El asistente consume el `DaRentPolicyV1` activo (bytes JSON o Norito) o
  el valor predeterminado incorporado, valida la política e imprime un resumen JSON (`gib`, `months`, metadatos de la política,
  y los campos `DaRentQuote`) para que los auditores puedan citar cargos XOR exactos dentro de las actas de gobierno sin
  escribir guiones ad hoc. El comando ahora también emite un resumen `rent_quote ...` de una línea antes del JSON.
  carga útil para hacer que los registros de la consola y los runbooks sean más fáciles de escanear cuando se generan cotizaciones durante incidentes.
  Pase `--quote-out artifacts/da/rent_quotes/<stamp>.json` (o cualquier otra ruta)
  para conservar el resumen bastante impreso y usar `--policy-label "governance ticket #..."` cuando el
  el artefacto necesita citar un paquete de votación/configuración específico; La CLI recorta etiquetas personalizadas y rechaza etiquetas en blanco.
  cadenas para mantener los valores `policy_source` significativos en los paquetes de evidencia. Ver
  `crates/iroha_cli/src/commands/da.rs` para el subcomando e `docs/source/da/rent_policy.md`
  para el esquema de política.【crates/iroha_cli/src/commands/da.rs:1】【docs/source/da/rent_policy.md:1】
- La paridad del registro de PIN ahora se extiende a los SDK: `ToriiClient.registerSorafsPinManifest(...)` en el
  JavaScript SDK crea la carga útil exacta utilizada por `iroha app sorafs pin register`, haciendo cumplir canónico
  metadatos fragmentados, políticas de fijación, pruebas de alias y resúmenes de sucesores antes de PUBLICAR en
  `/v2/sorafs/pin/register`. Esto evita que los robots de CI y la automatización desembolsen la CLI cuando
  grabar registros de manifiesto, y el ayudante se envía con cobertura TypeScript/README para que el DA-8
  La paridad de herramientas “enviar/obtener/probar” se cumple completamente en JS junto con Rust/Swift. 【javascript/iroha_js/src/toriiClient.js:1045】 【javascript/iroha_js/test/toriiClient.test.js:788】
- `iroha app da prove-availability` encadena todo lo anterior: toma un ticket de almacenamiento, descarga el
  paquete de manifiesto canónico, ejecuta el orquestador de múltiples fuentes (`iroha app sorafs fetch`) contra el
  lista `--gateway-provider` suministrada, persiste la carga útil descargada + el marcador en
  `artifacts/da/prove_availability_<timestamp>/` e inmediatamente invoca el asistente PoR existente
  (`iroha app da prove`) utilizando los bytes recuperados. Los operadores pueden modificar las perillas del orquestador.
  (`--max-peers`, `--scoreboard-out`, anulaciones de puntos finales de manifiesto) y el muestreador de prueba
  (`--sample-count`, `--leaf-index`, `--sample-seed`) mientras que un solo comando produce los artefactos
  esperado por las auditorías DA-5/DA-9: copia de carga útil, evidencia de marcador y resúmenes de prueba JSON.- `da_reconstruct` (nuevo en DA-6) lee un manifiesto canónico más el directorio del fragmento emitido por el fragmento
  store (diseño `chunk_{index:05}.bin`) y vuelve a ensamblar de manera determinista la carga útil mientras verifica
  cada compromiso de Blake3. La CLI se encuentra bajo `crates/sorafs_car/src/bin/da_reconstruct.rs` y se envía como
  parte del paquete de herramientas SoraFS. Flujo típico:
  1. `iroha app da get-blob --storage-ticket <ticket>` para descargar `manifest_<manifest_hash>.norito` y el plan fragmentado.
  2. `iroha app sorafs fetch --manifest manifest_<manifest_hash>.json --plan chunk_plan_<manifest_hash>.json --output payload.car`
     (o `iroha app da prove-availability`, que escribe los artefactos de búsqueda en
     `artifacts/da/prove_availability_<ts>/` y persiste los archivos por fragmento dentro del directorio `chunks/`).
  3. `cargo run -p sorafs_car --features cli --bin da_reconstruct --manifest manifest_<manifest_hash>.norito --chunks-dir ./artifacts/da/prove_availability_<ts>/chunks --output reconstructed.bin --json-out summary.json`.

  Un dispositivo de regresión vive bajo `fixtures/da/reconstruct/rs_parity_v1/` y captura el manifiesto completo
  y matriz de fragmentos (datos + paridad) utilizada por `tests::reconstructs_fixture_with_parity_chunks`. Regenerarlo con

  ```sh
  cargo test -p sorafs_car --features da_harness regenerate_da_reconstruct_fixture_assets -- --ignored --nocapture
  ```

  El aparato emite:

  - `manifest.{norito.hex,json}`: codificaciones canónicas `DaManifestV1`.
  - `chunk_matrix.json`: filas ordenadas de índice/desplazamiento/longitud/resumen/paridad para referencias de documentos/pruebas.
  - `chunks/`: segmentos de carga útil `chunk_{index:05}.bin` para fragmentos de datos y de paridad.
  - `payload.bin`: carga útil determinista utilizada por la prueba de arnés con reconocimiento de paridad.
  - `commitment_bundle.{json,norito.hex}`: muestra `DaCommitmentBundle` con un compromiso KZG determinista para documentos/pruebas.

  El arnés rechaza fragmentos faltantes o truncados, verifica el hash Blake3 de carga útil final con `blob_hash`,
  y emite un blob JSON resumido (bytes de carga útil, recuento de fragmentos, ticket de almacenamiento) para que CI pueda afirmar la reconstrucción
  evidencia. Esto cierra el requisito DA-6 de una herramienta de reconstrucción determinista que los operadores y el control de calidad
  Los trabajos pueden invocarse sin necesidad de cablear scripts personalizados.

## Resumen de resolución TODO

Se han implementado y verificado todos los TODO de ingesta previamente bloqueados:- **Sugerencias de compresión**: Torii acepta etiquetas proporcionadas por la persona que llama (`identity`, `gzip`, `deflate`,
  `zstd`) y normaliza las cargas útiles antes de la validación para que el hash del manifiesto canónico coincida con el
  bytes descomprimidos.【crates/iroha_torii/src/da/ingest.rs:220】【crates/iroha_data_model/src/da/types.rs:161】
- **Cifrado de metadatos solo de gobernanza**: Torii ahora cifra los metadatos de gobernanza con el
  clave ChaCha20-Poly1305 configurada, rechaza etiquetas que no coinciden y muestra dos explícitos
  perillas de configuración (`torii.da_ingest.governance_metadata_key_hex`,
  `torii.da_ingest.governance_metadata_key_label`) para mantener la rotación determinista. 【crates/iroha_torii/src/da/ingest.rs:707】【crates/iroha_config/src/parameters/actual.rs:1662】
- **Transmisión de carga útil grande**: la ingesta de varias partes está en vivo. Los clientes transmiten determinista
  Sobres `DaIngestChunk` codificados por `client_blob_id`, Torii valida cada segmento y los organiza
  bajo `manifest_store_dir`, y reconstruye atómicamente el manifiesto una vez que aterriza la bandera `is_last`,
  eliminando los picos de RAM que se observan con las cargas de una sola llamada.【crates/iroha_torii/src/da/ingest.rs:392】
- **Versión de manifiesto**: `DaManifestV1` lleva un campo `version` explícito y Torii lo rechaza.
  versiones desconocidas, lo que garantiza actualizaciones deterministas cuando se envían nuevos diseños de manifiesto.【crates/iroha_data_model/src/da/types.rs:308】
- **Enganches PDP/PoTR**: los compromisos de PDP se derivan directamente del almacén de fragmentos y persisten
  además de manifiestos para que los programadores DA-5 puedan lanzar desafíos de muestreo a partir de datos canónicos; el
  El encabezado `Sora-PDP-Commitment` ahora se envía con `/v2/da/ingest` e `/v2/da/manifests/{ticket}`.
  respuestas para que los SDK conozcan inmediatamente el compromiso firmado al que harán referencia futuras sondas.
- **Diario del cursor de fragmentos**: los metadatos del carril pueden especificar `da_shard_id` (el valor predeterminado es `lane_id`), y
  Sumeragi ahora persiste el `(epoch, sequence)` más alto por `(shard_id, lane_id)` en
  `da-shard-cursors.norito` junto al carrete DA, por lo que los reinicios eliminan carriles reconstruidos/desconocidos y mantienen
  repetición determinista. El índice del cursor de fragmentos en memoria ahora falla rápidamente en compromisos para
  carriles no asignados en lugar de utilizar de forma predeterminada la identificación del carril, lo que genera errores de avance del cursor y de reproducción
  explícita y la validación de bloques rechaza las regresiones de cursor de fragmento con un
  `DaShardCursorViolation` motivo + etiquetas de telemetría para operadores. El inicio/puesta al día ahora detiene DA
  indexa la hidratación si Kura contiene un carril desconocido o un cursor en regresión y registra la infracción
  altura del bloque para que los operadores puedan remediar antes de servir a DA estado.【crates/iroha_config/src/parameters/actual.rs】【crates/iroha_core/src/da/shard_cursor.rs】【crates/iroha_core/src/sumeragi/main_loop.rs】【crates/iroha_core/src/state.rs】【crates/iroha_core/src/block.rs】【docs/source/nexus_lanes.md:47】
- **Telemetría de retraso del cursor de fragmento**: el medidor `da_shard_cursor_lag_blocks{lane,shard}` informa cómohasta ahora, un fragmento sigue la altura que se está validando. Los carriles faltantes/obsoletos/desconocidos establecen el retraso en el
  altura requerida (o delta), y los avances exitosos la restablecen a cero para que el estado estable permanezca plano.
  Los operadores deben dar la alarma en caso de retrasos distintos de cero, inspeccionar el carrete/diario DA para detectar el carril infractor,
  y verifique el catálogo de carriles para ver si se vuelve a fragmentar accidentalmente antes de reproducir el bloque para borrar el
  brecha.
- **Carriles de cálculo confidenciales**: carriles marcados con
  `metadata.confidential_compute=true` y un `confidential_key_version` se tratan como
  SMPC/rutas DA cifradas: Sumeragi aplica resúmenes de manifiesto/carga útil distintos de cero y tickets de almacenamiento,
  rechaza perfiles de almacenamiento de réplica completa e indexa la versión de política + ticket SoraFS sin
  exponer bytes de carga útil. Los recibos se hidratan de Kura durante la repetición para que los validadores recuperen los mismos
  metadatos de confidencialidad después del reinicio.【crates/iroha_config/src/parameters/actual.rs】【crates/iroha_core/src/da/confidential.rs】【crates/iroha_core/src/da/confidential_store.rs】【crates/iroha_core/src/state.rs】

## Notas de implementación- El punto final `/v2/da/ingest` de Torii ahora normaliza la compresión de la carga útil, impone el caché de reproducción,
  fragmenta de manera determinista los bytes canónicos, reconstruye `DaManifestV1` y elimina la carga útil codificada
  en `config.da_ingest.manifest_store_dir` para la orquestación SoraFS antes de emitir el recibo; el
  El controlador también adjunta un encabezado `Sora-PDP-Commitment` para que los clientes puedan capturar el compromiso codificado.
  inmediatamente.【crates/iroha_torii/src/da/ingest.rs:220】
- Después de persistir el canónico `DaCommitmentRecord`, Torii ahora emite un
  Archivo `da-commitment-schedule-<lane>-<epoch>-<sequence>-<ticket>.norito` al lado del spool de manifiesto.
  Cada entrada agrupa el registro con los bytes sin procesar Norito `PdpCommitment` para que los generadores de paquetes DA-3 y
  Los programadores DA-5 ingieren entradas idénticas sin volver a leer manifiestos o almacenes de fragmentos. 【crates/iroha_torii/src/da/ingest.rs:1814】
- Los asistentes del SDK exponen los bytes del encabezado PDP sin obligar a cada cliente a volver a implementar el análisis Norito:
  `iroha::da::{decode_pdp_commitment_header, receipt_pdp_commitment}` tapa Óxido, el Python `ToriiClient`
  ahora exporta `decode_pdp_commitment_header`, y `IrohaSwift` envía ayudantes coincidentes para que sean móviles
  los clientes pueden guardar el programa de muestreo codificado inmediatamente.【crates/iroha/src/da.rs:1】【python/iroha_torii_client/client.py:1】【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:1】
- Torii también expone `GET /v2/da/manifests/{storage_ticket}` para que los SDK y los operadores puedan recuperar manifiestos
  y fragmentar planes sin tocar el directorio de spool del nodo. La respuesta devuelve los bytes Norito.
  (base64), JSON de manifiesto representado, un blob JSON `chunk_plan` listo para `sorafs fetch`, más el contenido relevante
  resúmenes hexadecimales (`storage_ticket`, `client_blob_id`, `blob_hash`, `chunk_root`) para que las herramientas posteriores puedan
  alimenta al orquestador sin volver a calcular los resúmenes y emite el mismo encabezado `Sora-PDP-Commitment` para
  respuestas de ingesta espejo. Pasar `block_hash=<hex>` como parámetro de consulta devuelve un resultado determinista.
  `sampling_plan` con raíz en `block_hash || client_blob_id` (compartido entre validadores) que contiene el
  `assignment_hash`, la `sample_window` solicitada y las tuplas `(index, role, group)` de muestra que abarcan
  todo el diseño de franja 2D para que los muestreadores y validadores de PoR puedan reproducir los mismos índices. la muestra
  mezcla `client_blob_id`, `chunk_root` e `ipa_commitment` en el hash de asignación; `iroha aplicación da get
  --block-hash ` now writes `sampling_plan_.json` junto al manifiesto + plan de fragmentos con
  el hash se conserva y los clientes JS/Swift Torii exponen el mismo `assignment_hash_hex` para que los validadores
  y los probadores comparten un único conjunto de sondas deterministas. Cuando Torii devuelve un plan de muestreo, `iroha app da
  probar-disponibilidad` now reuses that deterministic probe set (seed derived from `sample_seed`) en su lugar
  de muestreo ad hoc para que los testigos de PoR se alineen con las asignaciones de validador incluso si el operador omite un
  `--block-hash` anular.【crates/iroha_torii_shared/src/da/sampling.rs:1】【crates/iroha_cli/src/commands/da.rs:523】【javascript/iroha_js/src/toriiClient.js:15903】【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:170】

### Flujo de transmisión de carga útil grandeLos clientes que necesitan ingerir activos mayores que el límite de solicitud única configurado inician una
sesión de streaming llamando a `POST /v2/da/ingest/chunk/start`. Torii responde con un
`ChunkSessionId` (BLAKE3-derivado de los metadatos del blob solicitado) y el tamaño del fragmento negociado.
Cada solicitud `DaIngestChunk` posterior lleva:

- `client_blob_id` — idéntico al `DaIngestRequest` final.
- `chunk_session_id`: vincula los sectores a la sesión en ejecución.
- `chunk_index` e `offset`: aplican ordenamiento determinista.
- `payload`: hasta el tamaño del fragmento negociado.
- `payload_hash`: hash BLAKE3 del segmento para que Torii pueda validar sin almacenar en búfer todo el blob.
- `is_last`: indica el segmento terminal.

Torii persiste los sectores validados en `config.da_ingest.manifest_store_dir/chunks/<session>/` y
registra el progreso dentro del caché de reproducción para respetar la idempotencia. Cuando llega el último segmento, Torii
vuelve a ensamblar la carga útil en el disco (transmitiendo a través del directorio de fragmentos para evitar picos de memoria),
calcula el manifiesto/recibo canónico exactamente como con las cargas de un solo disparo y finalmente responde a
`POST /v2/da/ingest` consumiendo el artefacto preparado. Las sesiones fallidas se pueden abortar explícitamente o
se recolectan como basura después de `config.da_ingest.replay_cache_ttl`. Este diseño mantiene el formato de red.
Compatible con Norito, evita protocolos reanudables específicos del cliente y reutiliza la canalización de manifiesto existente
sin cambios.

**Estado de implementación.** Los tipos canónicos Norito ahora se encuentran en
`crates/iroha_data_model/src/da/`:

- `ingest.rs` define `DaIngestRequest`/`DaIngestReceipt`, junto con el
  Contenedor `ExtraMetadata` utilizado por Torii.【crates/iroha_data_model/src/da/ingest.rs:1】
- `manifest.rs` aloja `DaManifestV1` e `ChunkCommitment`, que Torii emite después
  La fragmentación se completa. 【crates/iroha_data_model/src/da/manifest.rs:1】
- `types.rs` proporciona alias compartidos (`BlobDigest`, `RetentionPolicy`,
  `ErasureProfile`, etc.) y codifica los valores de política predeterminados documentados a continuación. 【crates/iroha_data_model/src/da/types.rs:240】
- Los archivos de spool de manifiesto llegan a `config.da_ingest.manifest_store_dir`, listos para la orquestación SoraFS.
  observador para acceder a la admisión de almacenamiento. 【crates/iroha_torii/src/da/ingest.rs:220】
- Sumeragi impone la disponibilidad del manifiesto al sellar o validar paquetes DA:
  los bloques fallan en la validación si al spool le falta el manifiesto o el hash difiere
  del compromiso.【crates/iroha_core/src/sumeragi/main_loop.rs:5335】【crates/iroha_core/src/sumeragi/main_loop.rs:14506】

La cobertura de ida y vuelta para las cargas útiles de solicitud, manifiesto y recibo se rastrea en
`crates/iroha_data_model/tests/da_ingest_roundtrip.rs`, asegurando el códec Norito
permanece estable en todas las actualizaciones. 【crates/iroha_data_model/tests/da_ingest_roundtrip.rs:1】

**Valores predeterminados de retención.** La gobernanza ratificó la política de retención inicial durante
SF-6; Los valores predeterminados aplicados por `RetentionPolicy::default()` son:- nivel activo: 7 días (`604_800` segundos)
- nivel frío: 90 días (`7_776_000` segundos)
- réplicas requeridas: `3`
- clase de almacenamiento: `StorageClass::Hot`
- etiqueta de gobernanza: `"da.default"`

Los operadores aguas abajo deben anular estos valores explícitamente cuando un carril adopta
requisitos más estrictos.

## Artefactos a prueba de cliente Rust

Los SDK que integran el cliente Rust ya no necesitan desembolsarse en la CLI para
producir el paquete canónico PoR JSON. El `Client` expone dos ayudantes:

- `build_da_proof_artifact` devuelve la estructura exacta generada por
  `iroha app da prove --json-out`, incluidas las anotaciones de manifiesto/carga útil proporcionadas
  vía [`DaProofArtifactMetadata`].【crates/iroha/src/client.rs:3638】
- `write_da_proof_artifact` envuelve el constructor y conserva el artefacto en el disco.
  (bonito JSON + nueva línea final de forma predeterminada) para que la automatización pueda adjuntar el archivo
  a publicaciones o paquetes de evidencia de gobernanza.【crates/iroha/src/client.rs:3653】

### Ejemplo

```rust
use iroha::{
    da::{DaProofArtifactMetadata, DaProofConfig},
    Client,
};

let client = Client::new(config);
let manifest = client.get_da_manifest_bundle(storage_ticket)?;
let payload = std::fs::read("artifacts/da/payload.car")?;
let metadata = DaProofArtifactMetadata::new(
    "artifacts/da/manifest.norito",
    "artifacts/da/payload.car",
);

// Build the JSON artefact in-memory.
let artifact = client.build_da_proof_artifact(
    &manifest,
    &payload,
    &DaProofConfig::default(),
    &metadata,
)?;

// Persist it next to other DA artefacts.
client.write_da_proof_artifact(
    &manifest,
    &payload,
    &DaProofConfig::default(),
    &metadata,
    "artifacts/da/proof_summary.json",
    true,
)?;
```

La carga útil JSON que sale del asistente coincide con la CLI hasta los nombres de los campos
(`manifest_path`, `payload_path`, `proofs[*].chunk_digest`, etc.), por lo que existen
la automatización puede diferenciar/parquet/cargar el archivo sin ramas específicas del formato.

## Punto de referencia de verificación de prueba

Utilice el arnés de referencia de prueba de DA para validar los presupuestos de los verificadores en cargas útiles representativas antes
Apriete de tapones a nivel de bloque:

- `cargo xtask da-proof-bench` reconstruye el almacén de fragmentos a partir del par manifiesto/carga útil, muestra PoR
  hojas y verificación de tiempos contra el presupuesto configurado. Los metadatos de Taikai se completan automáticamente y el
  El arnés vuelve a caer en un manifiesto sintético si el par de dispositivos es inconsistente. Cuando `--payload-bytes`
  se establece sin un `--payload` explícito, el blob generado se escribe en
  `artifacts/da/proof_bench/payload.bin` para que los dispositivos permanezcan intactos.【xtask/src/da.rs:1332】【xtask/src/main.rs:2515】
- Los informes predeterminados son `artifacts/da/proof_bench/benchmark.{json,md}` e incluyen pruebas/ejecución, total y
  tiempos de prueba, tasa de aprobación del presupuesto y un presupuesto recomendado (110% de la iteración más lenta) para
  alinearse con `zk.halo2.verifier_budget_ms`. 【artifacts/da/proof_bench/benchmark.md:1】
- Última ejecución (carga útil sintética de 1 MiB, fragmentos de 64 KiB, 32 pruebas/ejecución, 10 iteraciones, presupuesto de 250 ms)
  recomendó un presupuesto de verificador de 3 ms con el 100 % de las iteraciones dentro del límite. 【artifacts/da/proof_bench/benchmark.md:1】
- Ejemplo (genera una carga útil determinista y escribe ambos informes):

```shell
cargo xtask da-proof-bench \
  --payload-bytes 1048576 \
  --sample-count 32 \
  --iterations 10 \
  --budget-ms 250 \
  --json-out artifacts/da/proof_bench/benchmark.json \
  --markdown-out artifacts/da/proof_bench/benchmark.md
```

El ensamblaje de bloques aplica los mismos presupuestos: `sumeragi.da_max_commitments_per_block` y
`sumeragi.da_max_proof_openings_per_block` controla el paquete DA antes de que se incruste en un bloque, y
cada compromiso debe llevar un `proof_digest` distinto de cero. El guardia trata la longitud del paquete como la
cuenta de apertura de prueba hasta que los resúmenes de prueba explícitos se enhebran a través del consenso, manteniendo el
Objetivo de apertura ≤128 aplicable en el límite del bloque. 【crates/iroha_core/src/sumeragi/main_loop.rs:6573】

## Manejo y reducción de fallas de PoRLos trabajadores de almacenamiento ahora muestran rachas de fallas de PoR y recomendaciones de barras vinculadas junto a cada una
veredicto. Los fallos consecutivos por encima del umbral de ataque configurado emiten una recomendación de que
incluye el par proveedor/manifiesto, la longitud de la racha que desencadenó la barra y la propuesta
penalización computada a partir de la fianza del proveedor e `penalty_bond_bps`; Las ventanas de enfriamiento (segundos) se mantienen.
barras duplicadas por disparos en el mismo incidente.【crates/sorafs_node/src/lib.rs:486】【crates/sorafs_node/src/config.rs:89】【crates/sorafs_node/src/bin/sorafs-node.rs:343】

- Configurar umbrales/enfriamiento a través del generador de trabajadores de almacenamiento (los valores predeterminados reflejan la gobernanza
  política de sanciones).
- Las recomendaciones de barra diagonal se registran en el resumen del veredicto JSON para que el gobierno y los auditores puedan adjuntarlo.
  a paquetes de pruebas.
- El diseño de franjas y los roles por fragmento ahora se transmiten a través del punto final del pin de almacenamiento de Torii.
  (campos `stripe_layout` + `chunk_roles`) y persistió en el trabajador de almacenamiento para que
  Los auditores/herramientas de reparación pueden planificar reparaciones de filas/columnas sin volver a derivar el diseño desde aguas arriba.

### Colocación + arnés de reparación

`cargo run -p sorafs_car --bin da_reconstruct -- --manifest <path> --chunks-dir <dir>` ahora
calcula un hash de ubicación sobre `(index, role, stripe/column, offsets)` y realiza primero la fila y luego
Reparación de la columna RS(16) antes de reconstruir la carga útil:

- La ubicación predeterminada es `total_stripes`/`shards_per_stripe` cuando está presente y vuelve al fragmento
- Los fragmentos faltantes/dañados se reconstruyen primero con la paridad de filas; Los huecos restantes se reparan con
  paridad de franja (columna). Los fragmentos reparados se vuelven a escribir en el directorio de fragmentos y el archivo JSON
  El resumen captura el hash de ubicación más los contadores de reparación de filas/columnas.
- Si la paridad fila+columna no puede satisfacer el conjunto faltante, el arnés falla rápidamente con lo irrecuperable
  índices para que los auditores puedan señalar manifiestos irreparables.
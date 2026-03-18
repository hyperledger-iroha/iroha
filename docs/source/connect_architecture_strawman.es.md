---
lang: es
direction: ltr
source: docs/source/connect_architecture_strawman.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a1a6bcc6bca3d7f70b82e35734b71d706ac46d8dc9c728351fabbd8a61dd3f31
source_last_modified: "2026-01-04T10:50:53.610255+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Conectar sesión Arquitectura Strawman (Swift / Android / JS)

Esta propuesta de pajaro describe el diseño compartido para los flujos de trabajo de Nexus Connect.
en los SDK de Swift, Android y JavaScript. Se pretende apoyar la
Taller entre SDK de febrero de 2026 y captura de preguntas abiertas antes de la implementación.

> Última actualización: 2026-01-29  
> Autores: Líder de SDK de Swift, TL de redes de Android, Líder de JS  
> Estado: Borrador para revisión del consejo (modelo de amenazas + alineación de retención de datos agregada el 12 de marzo de 2026)

## Metas

1. Alinear el ciclo de vida de la sesión de billetera ↔ dApp, incluido el arranque de la conexión,
   aprobaciones, solicitudes de firma y desmontaje.
2. Defina el esquema del sobre Norito (abrir/aprobar/firmar/controlar) compartido por todos
   SDK y garantizar la paridad con `connect_norito_bridge`.
3. Dividir responsabilidades entre transporte (WebSocket/WebRTC) y cifrado.
   (Norito Connect frames + intercambio de claves) y capas de aplicación (fachadas SDK).
4. Garantizar un comportamiento determinista en todas las plataformas móviles y de escritorio, incluidas
   almacenamiento en búfer y reconexión fuera de línea.

## Ciclo de vida de la sesión (alto nivel)

```
┌────────────┐      ┌─────────────┐      ┌────────────┐
│  dApp SDK  │←────→│  Connect WS │←────→│ Wallet SDK │
└────────────┘      └─────────────┘      └────────────┘
      │                    │                    │
      │ 1. open (app→wallet) frame (metadata, permissions, chain_id)
      │────────────────────────────────────────>│
      │                    │                    │
      │                    │ 2. route frame     │
      │                    │────────────────────│
      │                    │                    │
      │                    │     3. approve frame (wallet pk, account,
      │                    │        permissions, proof/attest)
      │<────────────────────────────────────────│
      │                    │                    │
      │ 4. sign request    │                    │
      │────────────────────────────────────────>│
      │                    │                    │
      │                    │ 5. sign result     │
      │                    │────────────────────│
      │                    │                    │
      │ 6. control frames for reject/close, error propagation, heartbeats.
```

## Esquema de sobre/Norito

Todos los SDK DEBEN utilizar el esquema canónico Norito definido en `connect_norito_bridge`:

- `EnvelopeV1` (abrir/aprobar/firmar/controlar)
- `ConnectFrameV1` (marcos de texto cifrado con carga útil AEAD)
- Control codes:
  - `open_ext` (metadatos, permisos)
  - `approve_ext` (cuenta, permisos, comprobantes, firma)
  - `reject`, `close`, `ping/pong`, `error`

Swift envió previamente codificadores JSON de marcador de posición (`ConnectCodec.swift`). A partir de abril de 2026, el SDK
siempre usa el puente Norito y falla cuando falta el XCFramework, pero este hombre de paja
todavía captura el mandato que condujo a la integración del puente:

| Función | Descripción | Estado |
|----------|-------------|--------|
| `connect_norito_encode_control_open_ext` | marco abierto dApp | Implementado en puente |
| `connect_norito_encode_control_approve_ext` | Aprobación de billetera | Implementado |
| `connect_norito_encode_envelope_sign_request_tx/raw` | Solicitudes de firma | Implementado |
| `connect_norito_encode_envelope_sign_result_ok/err` | Resultados de signos | Implementado |
| `connect_norito_decode_*` | Análisis de billeteras/dApps | Implementado |

### Trabajo requerido

- Swift: Reemplace los ayudantes JSON del marcador de posición `ConnectCodec` con llamadas de puente y superficie
  contenedores escritos (`ConnectFrame`, `ConnectEnvelope`) utilizando los tipos Norito compartidos. ✅ (abril de 2026)
- Android/JS: asegúrese de que existan los mismos contenedores; alinear códigos de error y claves de metadatos.
- Compartido: cifrado de documentos (intercambio de claves X25519, AEAD) con derivación de claves consistente
  según la especificación Norito y proporcione pruebas de integración de muestra utilizando el puente Rust.

## Contrato de transporte- Transporte primario: WebSocket (`/v1/connect/ws?sid=<session_id>`).
- Futuro opcional: WebRTC (TBD): fuera del alcance del testaferro inicial.
- Estrategia de reconexión: retroceso exponencial con fluctuación total (base 5 s, máximo 60 s); constantes compartidas en Swift, Android y JS para que los reintentos sigan siendo predecibles.
- Cadencia de ping/pong: 30 segundos de latido con tolerancia a tres pings perdidos antes de volver a conectarse; JS limita el intervalo mínimo a 15 segundos para satisfacer las reglas de limitación del navegador.
- Ganchos de inserción: el SDK de billetera de Android expone la integración FCM opcional para reactivaciones, mientras que JS permanece basado en encuestas (limitaciones documentadas para los permisos de inserción del navegador).
- Responsabilidades del SDK:
  - Mantener latidos de ping/pong (evitar agotar las baterías del móvil).
  - Almacenar en búfer los fotogramas salientes cuando no hay conexión (cola limitada, persistente para dApp).
- Proporcionar API de flujo de eventos (Swift Combine `AsyncStream`, Android Flow, iter asíncrono JS).
- La superficie vuelve a conectar los ganchos y permite volver a suscribirlos manualmente.
- Redacción de telemetría: solo emite contadores a nivel de sesión (hash `sid`, dirección,
  ventana de secuencia, profundidad de la cola) con sales documentadas en la telemetría de Connect
  guía; Los encabezados/claves nunca deben aparecer en registros o cadenas de depuración.

## Cifrado y gestión de claves

### Identificadores de sesión y sales

- `sid` es un identificador de 32 bytes derivado de `BLAKE2b-256("iroha-connect|sid|" || chain_id || app_ephemeral_pk || nonce16)`.  
  Las DApps lo calculan antes de llamar a `/v1/connect/session`; las billeteras lo hacen eco en marcos `approve` para que ambos lados puedan ingresar diarios y telemetría de manera consistente.
- La misma sal alimenta cada paso de derivación de claves para que los SDK nunca dependan de la entropía recopilada de la plataforma host.

### Manejo de claves efímeras

- Cada sesión utiliza material clave X25519 nuevo.  
  Swift lo almacena en Keychain/Secure Enclave a través de `ConnectCrypto`, las billeteras de Android están predeterminadas en StrongBox (recurriendo a los almacenes de claves respaldados por TEE) y JS requiere una instancia WebCrypto de contexto seguro o el complemento nativo `iroha_js_host`.
- Los marcos abiertos incluyen la clave pública efímera de dApp más un paquete de certificación opcional. Las aprobaciones de billetera devuelven la clave pública de la billetera y cualquier certificación de hardware necesaria para los flujos de cumplimiento.
- Las cargas útiles de certificación siguen el esquema aceptado:  
  `attestation { platform, evidence_b64, statement_hash }`.  
  Los navegadores pueden omitir el bloqueo; las billeteras nativas lo incluyen siempre que se utilizan claves respaldadas por hardware.

### Teclas direccionales y AEAD

- Los secretos compartidos se amplían con HKDF-SHA256 (a través de los ayudantes del puente Rust) y cadenas de información separadas por dominio:
  - `iroha-connect|k_app` → aplicación → tráfico de billetera.
  - `iroha-connect|k_wallet` → billetera → tráfico de aplicaciones.
- AEAD es ChaCha20-Poly1305 para el sobre v1 (`connect_norito_bridge` expone ayudas en cada plataforma).  
  Los datos asociados equivalen a `("connect:v1", sid, dir, seq_le, kind=ciphertext)`, por lo que se detecta manipulación de los encabezados.
- Los nonces se derivan del contador de secuencia de 64 bits (`nonce[0..4]=0`, `nonce[4..12]=seq_le`). Las pruebas de ayuda compartidas garantizan que las conversiones BigInt/UInt se comporten de manera idéntica en todos los SDK.

### Apretón de manos de rotación y recuperación- La rotación sigue siendo opcional, pero el protocolo está definido: las dApps emiten una trama `Control::RotateKeys` cuando los contadores de secuencia se acercan al protector de envoltura, las billeteras responden con la nueva clave pública más un reconocimiento firmado, y ambas partes derivan inmediatamente nuevas claves direccionales sin cerrar la sesión.
- La pérdida de clave del lado de la billetera activa el mismo protocolo de enlace seguido de un control `resume` para que las dApps sepan que deben vaciar el texto cifrado almacenado en caché que apuntaba a la clave retirada.

Para conocer las alternativas históricas de CryptoKit, consulte `docs/connect_swift_ios.md`; Kotlin y JS tienen referencias coincidentes en `docs/connect_kotlin_ws*.md`.

## Permisos y pruebas

- Los manifiestos de permiso deben recorrer la estructura Norito compartida exportada por el puente.  
  Campos:
  - `methods` — verbos (`sign_transaction`, `sign_raw`, `submit_proof`,…).  
  - `events`: suscripciones a las que se permite adjuntar la dApp.  
  - `resources`: filtros opcionales de cuenta/activos para que las billeteras puedan acceder al alcance.  
  - `constraints`: ID de cadena, TTL o controles de política personalizados que la billetera aplica antes de firmar.
- Los metadatos de cumplimiento van junto con los permisos:
  - El `attachments[]` opcional contiene referencias de archivos adjuntos Norito (paquetes KYC, recibos de reguladores).  
  - `compliance_manifest_id` vincula la solicitud a un manifiesto previamente aprobado para que los operadores puedan auditar la procedencia.
- Las respuestas de Wallet utilizan los códigos acordados:
  - `user_declined`, `permissions_mismatch`, `compliance_failed`, `internal_error`.  
  Cada uno puede llevar un `localized_message` para sugerencias de UI más un `reason_code` legible por máquina.
- Los marcos de aprobación incluyen la cuenta/controlador seleccionado, el eco de permiso, el paquete de pruebas (prueba o atestación ZK) y cualquier cambio de política (por ejemplo, `offline_queue_enabled`).  
  Los rechazos reflejan el mismo esquema con `proof` vacío pero aún registran el `sid` para auditabilidad.

## Fachadas SDK

| SDK | API propuesta | Notas |
|-----|--------------|-------|
| Rápido | `ConnectClient`, `ConnectSession`, `ConnectRequest`, `ConnectApproval` | Reemplace los marcadores de posición con contenedores escritos + transmisiones asíncronas. |
| Androide | Corrutinas de Kotlin + clases selladas para marcos | Alinee con la estructura Swift para mayor portabilidad. |
| JS | Iteradores asíncronos + enumeraciones TypeScript para tipos de fotogramas | Proporcionar SDK compatible con paquetes (navegador/nodo). |

### Comportamientos comunes- `ConnectSession` organiza el ciclo de vida:
  1. Establezca WebSocket y realice un apretón de manos.
  2. Intercambiar marcos de apertura/aprobación.
  3. Manejar solicitudes/respuestas de firmas.
  4. Emitir eventos a la capa de aplicación.
- Proporcionar ayudantes de alto nivel:
  - `requestSignature(tx, metadata)`
  - `approveSession(account, permissions)`
  - `reject(reason)`
  - `cancelRequest(hash)` – emite un marco de control reconocido por la billetera.
- Manejo de errores: asigne códigos de error Norito a errores específicos del SDK; incluir
  códigos específicos de dominio para la interfaz de usuario que utilizan la taxonomía compartida (`Transport`, `Codec`, `Authorization`, `Timeout`, `QueueOverflow`, `Internal`). La guía de telemetría e implementación básica de Swift se encuentra en [`connect_error_taxonomy.md`](connect_error_taxonomy.md) y es la referencia para la paridad de Android/JS.
- Emitir enlaces de telemetría para profundidad de cola, recuentos de reconexión y latencia de solicitudes (`connect.queue_depth`, `connect.reconnects_total`, `connect.latency_ms`).
 
## Números de secuencia y control de flujo

- Cada dirección mantiene un contador `sequence` dedicado de 64 bits que comienza en cero cuando se abre la sesión. El ayudante compartido escribe incrementos de abrazadera y activa un protocolo de enlace de rotación de tecla `ConnectError.sequenceOverflow` + mucho antes de que el contador se ajuste.
- Los nonces y los datos asociados hacen referencia al número de secuencia, por lo que los duplicados se pueden rechazar sin analizar las cargas útiles. Los SDK deben almacenar `{sid, dir, seq, payload_hash}` en sus diarios para que la deduplicación sea determinista entre las reconexiones.
- Las billeteras anuncian contrapresión a través de una ventana lógica (marcos de control `FlowControl`). Las DApps salen de la cola solo cuando hay un token de ventana disponible; las billeteras emiten nuevos tokens después de procesar el texto cifrado para mantener las canalizaciones limitadas.
- La reanudación de la negociación es explícita: ambas partes emiten `Control::Resume { seq_app_max, seq_wallet_max, queue_depths }` después de volver a conectarse para que los observadores puedan verificar cuántos datos se reenviaron y si los diarios contienen lagunas.
- Los conflictos (por ejemplo, dos cargas útiles con el mismo `(sid, dir, seq)` pero diferentes hashes) escalan a `ConnectError.Internal` y fuerzan un nuevo `sid` para evitar una divergencia silenciosa.

## Modelo de amenazas y alineación de retención de datos- **Superficies consideradas:** Transporte WebSocket, codificación/decodificación de puente Norito,
  persistencia de diarios, exportadores de telemetría y devoluciones de llamadas orientadas a aplicaciones.
- **Objetivos principales:** proteger los secretos de sesión (claves X25519, claves AEAD derivadas,
  contadores de nonce/secuencia) de fugas en registros/telemetría, evitar la reproducción y
  ataques de degradación y retención limitada de diarios e informes de anomalías.
- **Mitigaciones codificadas:**
  - Las revistas sólo contienen texto cifrado; Los metadatos almacenados se limitan a hashes y longitud.
    campos, marcas de tiempo y números de secuencia.
  - Las cargas útiles de telemetría redactan cualquier contenido de encabezado/carga útil e incluyen solo
    hashes salados de `sid` más contadores agregados; lista de verificación de redacción compartida
    entre SDK para lograr la paridad de auditoría.
  - Los registros de sesión se rotan y caducan después de 7 días de forma predeterminada. Las carteras exponen
    una perilla `connectLogRetentionDays` (SDK predeterminado 7) y documente el comportamiento
    por lo que las implementaciones reguladas pueden fijar ventanas más estrictas.
  - Uso indebido de la API Bridge (faltan enlaces, texto cifrado corrupto, secuencia no válida)
    devuelve errores escritos sin hacer eco de cargas útiles o claves sin procesar.

Las preguntas pendientes de revisión se rastrean en `docs/source/sdk/swift/connect_workshop.md`
y se resolverá en el acta del consejo; una vez cerrado el muñeco de paja será
promovido de borrador a aceptado.

## Almacenamiento en búfer y reconexiones sin conexión

### Contrato de diario

Cada SDK mantiene un diario de solo anexos por sesión para que la dApp y la billetera
puede poner en cola fotogramas sin conexión, reanudarlos sin pérdida de datos y proporcionar pruebas
para telemetría. El contrato refleja los tipos de puente Norito, por lo que el mismo byte
La representación sobrevive en las pilas móviles/JS.- Los diarios viven bajo un identificador de sesión hash (`sha256(sid)`), lo que produce dos
  archivos por sesión: `app_to_wallet.queue` y `wallet_to_app.queue`. Usos rápidos
  un contenedor de archivos en espacio aislado, Android almacena los archivos a través de `Room`/`FileChannel`,
  y JS escribe en IndexedDB; Todos los formatos son binarios y estables endian.
- Cada registro se serializa como `ConnectJournalRecordV1`:
  - `direction: u8` (`0 = app→wallet`, `1 = wallet→app`)
  - `sequence: u64`
  - `payload_hash: [u8; 32]` (Blake3 de texto cifrado + encabezados)
  - `ciphertext_len: u32`
  - `received_at_ms: u64`
  - `expires_at_ms: u64`
  - `ciphertext: [u8; ciphertext_len]` (marco exacto Norito ya envuelto en AEAD)
- Las revistas almacenan texto cifrado palabra por palabra. Nunca volvemos a cifrar la carga útil; AEAD
  Los encabezados ya autentican las claves de dirección, por lo que la persistencia se reduce a
  fsyncing el registro adjunto.
- Una estructura `ConnectQueueState` en la memoria refleja los metadatos del archivo (profundidad,
  bytes utilizados, secuencia más antigua/más reciente). Alimenta a los exportadores de telemetría y a los
  Ayudante `FlowControl`.
- Los diarios tienen un límite de 32 fotogramas/1MiB de forma predeterminada; golpear la tapa expulsa el
  entradas más antiguas (`reason=overflow`). `ConnectFeatureConfig.max_queue_len`
  anula estos valores predeterminados por implementación.
- Las revistas retienen datos durante 24h (`expires_at_ms`). GC en segundo plano elimina obsoleto
  segmentos con entusiasmo para que la huella en el disco permanezca limitada.
- Seguridad contra fallas: agregar, sincronizar y actualizar la memoria reflejada _antes_ de notificar
  la persona que llama. Al iniciar, los SDK escanean el directorio, validan las sumas de verificación de registros,
  y reconstruir `ConnectQueueState`. La corrupción hace que el historial infractor sea
  omitido, marcado mediante telemetría y, opcionalmente, puesto en cuarentena para volcados de soporte.
- Debido a que el texto cifrado ya satisface el sobre de privacidad Norito, el único
  Los metadatos adicionales registrados son la identificación de la sesión con hash. Aplicaciones que buscan más
  privacidad puede optar por `telemetry_opt_in = false`, que almacena revistas pero
  Redacta las exportaciones de profundidad de la cola y deshabilita el uso compartido de `sid` en los registros.
- Los SDK exponen `ConnectQueueObserver` para que las billeteras/dApps puedan inspeccionar la profundidad de la cola,
  drenajes y resultados de GC; este enlace alimenta las IU de estado sin analizar los registros.

### Reproducir y reanudar la semántica

1. Al volver a conectarse, los SDK emiten `Control::Resume` con `{seq_app_max,
   seq_wallet_max, queued_app, queued_wallet, journal_hash}`. El hash es el
   Resumen Blake3 de la revista de solo anexos para que los pares que no coinciden puedan detectar la deriva.
2. El par receptor compara la carga útil del currículum con su estado, solicita
   retransmisión cuando existen espacios y reconoce los fotogramas reproducidos a través de
   `Control::ResumeAck`.
3. Los fotogramas reproducidos siempre respetan el orden de inserción (`sequence` luego tiempo de escritura).
   Los SDK de billetera DEBEN aplicar contrapresión emitiendo tokens `FlowControl` (también
   registrado) para que las dApps no puedan inundar la cola mientras están fuera de línea.
4. Los diarios almacenan texto cifrado palabra por palabra, por lo que la reproducción simplemente bombea los bytes grabados.
   de vuelta a través del transporte y el decodificador. No se permite la recodificación por SDK.

### Flujo de reconexión1. El transporte restablece WebSocket y negocia un nuevo intervalo de ping.
2. La dApp reproduce los fotogramas en cola en orden, respetando la contrapresión de la billetera.
   (`ConnectSession.nextControlFrame()` produce tokens `FlowControl`).
3. Wallet descifra los resultados almacenados en el búfer, verifica la monotonicidad de la secuencia y
   reproduce aprobaciones/resultados pendientes.
4. Ambos lados emiten un control `resume` que resume `seq_app_max`, `seq_wallet_max`,
   y profundidades de cola para telemetría.
5. Las tramas duplicadas (que coinciden con `sequence` + `payload_hash`) se reconocen y eliminan; Los conflictos generan `ConnectError.Internal` y desencadenan un reinicio forzado de la sesión.

### Modos de falla

- Si la sesión se considera obsoleta (`offline_timeout_ms`, por defecto 5 minutos),
  los marcos almacenados en búfer se purgan y el SDK genera `ConnectError.sessionExpired`.
- En caso de corrupción del diario, los SDK intentan reparar una única decodificación Norito; en
  Si falla, sueltan el diario y emiten telemetría `connect.queue_repair_failed`.
- La falta de coincidencia de secuencia activa `ConnectError.replayDetected` y fuerza una nueva
  apretón de manos (reinicio de sesión con el nuevo `sid`).

### Plan de almacenamiento en búfer sin conexión y controles del operador

El entregable del taller requiere un plan documentado para que cada SDK envíe el mismo
comportamiento fuera de línea, flujo de remediación y superficies de evidencia. El plan a continuación es
común en Swift (`ConnectSessionDiagnostics`), Android
(`ConnectDiagnosticsSnapshot`) y JS (`ConnectQueueInspector`).

| Estado | Gatillo | Respuesta automática | Anulación manual | Bandera de telemetría |
|-------|---------|--------------------|-----------------|----------------|
| `Healthy` | Uso de cola  5/min | Pausar nuevas solicitudes de firma y emitir tokens de control de flujo a la mitad | Las aplicaciones pueden llamar a `clearOfflineQueue(.app|.wallet)`; SDK rehidrata el estado del par una vez en línea | Calibre `connect.queue_state=\"throttled\"`, `connect.queue_watermark` |
| `Quarantined` | Uso ≥ `disk_watermark_drop` (predeterminado 85%), corrupción detectada dos veces o `offline_timeout_ms` excedido | Detener el almacenamiento en búfer, generar `ConnectError.QueueQuarantined`, requerir reconocimiento del operador | `ConnectSessionDiagnostics.forceReset()` elimina diarios después de exportar el paquete | Contador `connect.queue_state=\"quarantined\"`, `connect.queue_quarantine_total` |- Los umbrales viven en `ConnectFeatureConfig` (`disk_watermark_warn`,
  `disk_watermark_drop`, `max_disk_bytes`, `offline_timeout_ms`). cuando un anfitrión
  omite un valor, los SDK vuelven a sus valores predeterminados y registran una advertencia para que las configuraciones
  Se puede auditar desde la telemetría.
- Los SDK exponen `ConnectQueueObserver` más ayudas de diagnóstico:
  - Swift: `ConnectSessionDiagnostics.snapshot()` produce `{estado, profundidad, bytes,
    motivo}` and `exportJournalBundle(url:)` persiste en ambas colas para obtener soporte.
  - Android: `ConnectDiagnostics.snapshot()` + `exportJournalBundle(path)`.
  - JS: `ConnectQueueInspector.read()` devuelve la misma estructura y un identificador de blob
    ese código de interfaz de usuario se puede cargar en las herramientas de soporte Torii.
- Cuando una aplicación alterna `offline_queue_enabled=false`, los SDK se agotan inmediatamente y
  purgue ambos diarios, marque el estado como `Disabled` y emita un terminal
  evento de telemetría. La preferencia de cara al usuario se refleja en el Norito
  marco de aprobación para que los pares sepan si pueden reanudar los marcos almacenados en búfer.
- Los operadores ejecutan `connect queue inspect --sid <sid>` (contenedor CLI alrededor del SDK
  diagnóstico) durante las pruebas de caos; este comando imprime las transiciones de estado,
  historial de marcas de agua y evidencia de reanudación para que las revisiones de gobernanza no dependan de
  herramientas específicas de la plataforma.

### Flujo de trabajo del paquete de pruebas

Los equipos de soporte y cumplimiento se basan en evidencia determinista al realizar auditorías.
comportamiento fuera de línea. Por lo tanto, cada SDK implementa la misma exportación de tres pasos:

1. `exportJournalBundle(..)` escribe `{app_to_wallet,wallet_to_app}.queue` más un
   manifiesto que describe el hash de compilación, los indicadores de características y las marcas de agua del disco.
2. `exportQueueMetrics(..)` emite las últimas 1000 muestras de telemetría para que los paneles
   se puede reconstruir fuera de línea. Los ejemplos incluyen la identificación de la sesión con hash cuando el
   el usuario optó por participar.
3. El asistente CLI comprime ambas exportaciones y adjunta un archivo de metadatos Norito firmado.
   (`ConnectQueueEvidenceV1`) para que la ingesta de Torii pueda archivar el paquete en SoraFS.

Los paquetes que no superan la validación se rechazan con `connect.evidence_invalid`
telemetría para que el equipo del SDK pueda reproducir y parchear el exportador.

## Telemetría y diagnóstico- Emitir eventos JSON Norito a través de exportadores compartidos de OpenTelemetry. Métricas obligatorias:
  - `connect.queue_depth{direction}` (medidor) alimentado por `ConnectQueueState`.
  - `connect.queue_bytes{direction}` (calibre) para huella respaldada en disco.
  - `connect.queue_dropped_total{reason}` (contador) para `overflow|ttl|repair`.
  - `connect.offline_flush_total{direction}` (contador) incrementa cuando hay colas
    drenaje sin transporte; las fallas incrementan `connect.offline_flush_failed`.
  -`connect.replay_success_total`/`connect.replay_error_total`.
  - Histograma `connect.resume_latency_ms` (tiempo entre reconexión y estabilización)
    estado) más `connect.resume_attempts_total`.
  - Histograma `connect.session_duration_ms` (por sesión completada).
  - Eventos estructurados `connect.error` con `code`, `fatal`, `telemetry_profile`.
- Los exportadores DEBEN adjuntar etiquetas `{platform, sdk_version, feature_hash}` para que
  Los paneles se pueden dividir según la compilación del SDK. El hash `sid` es opcional y solo
  emitido cuando la opción de telemetría es verdadera.
- Los enlaces a nivel de SDK muestran los mismos eventos para que las aplicaciones puedan exportar más detalles:
  - Rápido: `ConnectSession.addObserver(_:) -> ConnectEvent`.
  - Androide: `Flow<ConnectEvent>`.
  - JS: iterador asíncrono o callback.
- Control de CI: los trabajos Swift ejecutan `make swift-ci`, Android usa `./gradlew sdkConnectCi`,
  y JS ejecuta `npm run test:connect` para que la telemetría/paneles permanezcan verdes antes
  fusionar cambios de Connect.
- Los registros estructurados incluyen los hash `sid`, `seq`, `queue_depth` e `sid_epoch`.
  valores para que los operadores puedan correlacionar los problemas de los clientes. Los diarios que fallan en la reparación emiten
  Eventos `connect.queue_repair_failed{reason}` más una ruta de volcado de memoria opcional.

### Ganchos de telemetría y evidencia de gobernanza

- `connect.queue_state` también funciona como indicador de riesgo de la hoja de ruta. Grupo de paneles
  por `{platform, sdk_version}` y renderizar el tiempo en estado para que la gobernanza pueda tomar muestras
  evidencia de simulacro mensual antes de aprobar implementaciones por etapas.
- `connect.queue_watermark` e `connect.queue_bytes` alimentan la puntuación de riesgo de Connect
  (`risk.connect.offline_buffer`), que busca automáticamente SRE cuando hay más de
  El 5% de las sesiones pasan >10 minutos en `Throttled`.
- Los exportadores adjuntan `feature_hash` a cada evento para que las herramientas del auditor puedan confirmar
  que el códec Norito + plan fuera de línea coincida con la compilación revisada. El CI del SDK falla
  rápido cuando la telemetría informa un hash desconocido.
- El muñeco de paja todavía requiere un apéndice del modelo de amenazas; cuando las métricas exceden el
  umbrales de política, los SDK emiten eventos `connect.policy_violation` que resumen los
  Sid infractor (hash), estado y acción resuelta (`drain|purge|quarantine`).
- La evidencia capturada a través de `exportQueueMetrics` aterriza en el mismo espacio de nombres SoraFS.
  como los artefactos del runbook de Connect para que los revisores del consejo puedan rastrear cada simulacro
  volver a muestras de telemetría específicas sin solicitar registros internos.

## Propiedad y responsabilidades del marco| Marco / Control | Propietario | Dominio de secuencia | ¿El diario persistió? | Etiquetas de telemetría | Notas |
|-----------------|-------|-----------------|--------------------|------------------|-------|
| `Control::Open` | dApp | `seq_app` | ✅ (`app_to_wallet`) | `event=open` | Lleva metadatos + mapa de bits de permiso; las billeteras reproducen la última apertura antes de las indicaciones. |
| `Control::Approve` | Cartera | `seq_wallet` | ✅ (`wallet_to_app`) | `event=approve` | Incluye cuenta, comprobantes, firmas. Incrementos de la versión de metadatos registrados aquí. |
| `Control::Reject` | Cartera | `seq_wallet` | ✅ | `event=reject`, `reason` | Mensaje localizado opcional; La dApp elimina las solicitudes de firma pendientes. |
| `Control::Close` (inicio) | dApp | `seq_app` | ✅ | `event=close`, `initiator=app` | Wallet reconoce con su propio `Close`. |
| `Control::Close` (reconocimiento) | Cartera | `seq_wallet` | ✅ | `event=close`, `initiator=wallet` | Confirma desmontaje; GC elimina los muñones una vez que ambos lados persisten en el marco. |
| `SignRequest` | dApp | `seq_app` | ✅ | `event=sign_request`, `payload_hash` | Hash de carga útil registrado para la detección de conflictos de reproducción. |
| `SignResult` | Cartera | `seq_wallet` | ✅ | `event=sign_result`, `status=ok|err` | Incluye hash BLAKE3 de bytes firmados; las fallas generan `ConnectError.Signing`. |
| `Control::Error` | Wallet (la mayoría) / dApp (transporte) | dominio propietario coincidente | ✅ | `event=error`, `code` | Los errores fatales obligan a reiniciar la sesión; marcas de telemetría `fatal=true`. |
| `Control::RotateKeys` | Cartera | `seq_wallet` | ✅ | `event=rotate_keys`, `reason` | Anuncia nuevas claves de dirección; dApp responde con `RotateKeysAck` (registrado en el lado de la aplicación). |
| `Control::Resume` / `ResumeAck` | Ambos | dominio local solamente | ✅ | `event=resume`, `direction=app|wallet` | Resume la profundidad de la cola + estado de secuencia; El resumen del diario hash ayuda al diagnóstico. |

- Las claves de cifrado direccionales permanecen simétricas por función (`app→wallet`, `wallet→app`).
  Las propuestas de rotación de billeteras se anuncian a través de `Control::RotateKeys` y dApps.
  reconocer emitiendo `Control::RotateKeysAck`; ambos cuadros deben golpear el disco
  antes de que las teclas se intercambien para evitar espacios en la repetición.
- Los archivos adjuntos de metadatos (iconos, nombres localizados, pruebas de cumplimiento) están firmados por
  la billetera y almacenado en caché por la dApp; las actualizaciones requieren un nuevo marco de aprobación con
  incrementado `metadata_version`.
- Se hace referencia a la matriz de propiedad anterior en los documentos del SDK, por lo que CLI/web/automation
  Los clientes siguen los mismos contratos y valores predeterminados de instrumentación.

## Preguntas abiertas

1. **Descubrimiento de sesiones**: ¿Necesitamos códigos QR o protocolo de enlace fuera de banda como WalletConnect? (Trabajo futuro.)
2. **Multifirma**: ¿Cómo se representan las aprobaciones multifirma? (Amplíe el resultado de la firma para admitir varias firmas).
3. **Cumplimiento**: ¿Qué campos son obligatorios para los flujos regulados (según hoja de ruta)? (Espere la orientación del equipo de cumplimiento).
4. **Empaquetado de SDK**: ¿Deberíamos incluir el código compartido (por ejemplo, códecs Norito Connect) en una caja multiplataforma? (Por determinar).

## Próximos pasos- Hacer circular este testaferro al consejo del SDK (reunión de febrero de 2026).
- Recopile comentarios sobre preguntas abiertas y actualice el documento en consecuencia.
- Desglose de la implementación del cronograma por SDK (hitos de Swift IOS7, Android AND7, JS Connect).
- Seguimiento del progreso a través de la lista activa de la hoja de ruta; actualice `status.md` una vez que se ratifique a Strawman.
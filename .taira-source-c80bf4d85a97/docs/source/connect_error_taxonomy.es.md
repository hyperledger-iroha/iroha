---
lang: es
direction: ltr
source: docs/source/connect_error_taxonomy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bd6ab18725af866ffd2589a3fa7bd7130bab1d5efef2920408baa8e338d5d068
source_last_modified: "2026-01-03T18:08:01.837162+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Taxonomía de errores de conexión (línea de base Swift)

Esta nota rastrea IOS-CONNECT-010 y documenta la taxonomía de errores compartida para
Nexus Conecte SDK. Swift ahora exporta el contenedor canónico `ConnectError`,
que asigna todas las fallas relacionadas con Connect a una de seis categorías para que la telemetría,
Los paneles y la copia de UX se mantienen alineados en todas las plataformas.

> Última actualización: 2026-01-15  
> Propietario: Líder de Swift SDK (administrador de taxonomía)  
> Estado: Implementaciones de Swift + Android + JavaScript **aterrizadas**; integración de cola pendiente.

## Categorías

| Categoría | Propósito | Fuentes típicas |
|----------|---------|-----------------|
| `transport` | Fallos de transporte WebSocket/HTTP que finalizan una sesión. | `URLError(.cannotConnectToHost)`, `ConnectClient.ClientError.closed` |
| `codec` | Fallos de serialización/puente al codificar/decodificar tramas. | `ConnectEnvelopeError.invalidPayload`, `DecodingError` |
| `authorization` | Fallos de TLS/certificación/política que requieren reparación por parte del usuario o del operador. | `URLError(.secureConnectionFailed)`, Torii 4xx respuestas |
| `timeout` | Vencimientos inactivos/fuera de línea y vigilancia (TTL de cola, tiempo de espera de solicitud). | `URLError(.timedOut)`, `ConnectQueueError.expired` |
| `queueOverflow` | Señales de contrapresión FIFO para que las aplicaciones puedan descargar la carga sin problemas. | `ConnectQueueError.overflow(limit:)` |
| `internal` | Todo lo demás: mal uso del SDK, falta del puente Norito, diarios corruptos. | `ConnectSessionError.missingDecryptionKeys`, `ConnectCryptoError.*` |

Cada SDK publica un tipo de error que se ajusta a la taxonomía y expone
atributos de telemetría estructurados: `category`, `code`, `fatal` y opcional
metadatos (`http_status`, `underlying`).

## Mapeo rápido

Swift exporta `ConnectError`, `ConnectErrorCategory` y protocolos auxiliares en
`IrohaSwift/Sources/IrohaSwift/ConnectError.swift`. Error de conexión todo público
los tipos se ajustan a `ConnectErrorConvertible`, por lo que las aplicaciones pueden llamar a `error.asConnectError()`
y reenviar el resultado a las capas de telemetría/registro.| Error rápido | Categoría | Código | Notas |
|-------------|----------|------|-------|
| `ConnectClient.ClientError.alreadyStarted` | `internal` | `client.already_started` | Indica doble `start()`; error del desarrollador. |
| `ConnectClient.ClientError.closed` | `transport` | `client.closed` | Generado al enviar/recibir después de un cierre. |
| `ConnectClient.ClientError.unknownPayload` | `codec` | `client.unknown_payload` | WebSocket entregó carga útil textual mientras esperaba binario. |
| `ConnectSessionError.streamEnded` | `transport` | `session.stream_ended` | La contraparte cerró la transmisión inesperadamente. |
| `ConnectSessionError.missingDecryptionKeys` | `internal` | `session.missing_direction_keys` | La aplicación olvidó configurar claves simétricas. |
| `ConnectEnvelopeError.invalidPayload` | `codec` | `envelope.invalid_payload` | A la carga útil Norito le faltan campos obligatorios. |
| `ConnectEnvelopeError.unknownPayloadKind` | `codec` | `envelope.unknown_payload_kind` | Carga útil futura vista por el SDK antiguo. |
| `ConnectCodecError.*` | `codec` | `codec.bridge_unavailable` / `codec.encode_failed` / `codec.decode_failed` | Falta el puente Norito o no se pudieron codificar/decodificar bytes de trama. |
| `ConnectCryptoError.*` | `internal` | `crypto.*` | Puente no disponible o longitudes de clave que no coinciden. |
| `ConnectQueueError.overflow(limit:)` | `queueOverflow` | `queue.overflow` | La longitud de la cola sin conexión superó el límite configurado. |
| `URLError(.timedOut)` | `timeout` | `network.timeout` | Aparecido por `URLSessionWebSocketTask`. |
| `URLError` Casos TLS | `authorization` | `network.tls_failure` | Fallos de negociación ATS/TLS. |
| `DecodingError` / `EncodingError` | `codec` | `codec.*` | La decodificación/codificación JSON falló en otras partes del SDK; El mensaje utiliza el contexto del decodificador Swift. |
| Cualquier otro `Error` | `internal` | `unknown_error` | Todo incluido garantizado; espejos de mensajes `LocalizedError`. |

Bloqueo de pruebas unitarias (`IrohaSwift/Tests/IrohaSwiftTests/ConnectErrorTests.swift`)
el mapeo para que futuros refactorizadores no puedan cambiar silenciosamente categorías o códigos.

### Ejemplo de uso

```swift
do {
    try await session.nextEnvelope()
} catch {
    let connectError = error.asConnectError()
    telemetry.emit(event: "connect.error",
                   attributes: connectError.telemetryAttributes(fatal: true))
    logger.error("Connect failure: \(connectError.code) – \(connectError.message)")
}
```

## Telemetría y paneles

El SDK de Swift proporciona `ConnectError.telemetryAttributes(fatal:httpStatus:)`
que devuelve el mapa de atributos canónicos. Los exportadores deben enviar estos
atributos en eventos OTEL `connect.error` con extras opcionales:

```
attributes = {
  "category": "transport",
  "code": "client.closed",
  "fatal": "true",
  "http_status": "101" // optional
  "underlying": "URLError(Code=-1005 ...)" // optional
}
```

Los paneles correlacionan los contadores `connect.error` con la profundidad de la cola (`connect.queue_depth`)
y volver a conectar histogramas para detectar regresiones sin necesidad de espeleología de registros.

## mapeo de AndroidEl SDK de Android exporta `ConnectError`, `ConnectErrorCategory`, `ConnectErrorTelemetryOptions`,
`ConnectErrorOptions` y las utilidades auxiliares en
`org.hyperledger.iroha.android.connect.error`. Los ayudantes de estilo constructor convierten cualquier `Throwable`
en una carga útil compatible con la taxonomía, inferir categorías a partir de excepciones de transporte/TLS/códec,
y exponer atributos de telemetría deterministas para que OpenTelemetry/pilas de muestreo puedan consumir el
resultado sin adaptadores personalizados.【java/iroha_android/src/main/java/org/hyperledger/iroha/android/connect/error/ConnectError.java:8】【java/iroha_android/src/main/java/org/hyperledger/iroha/android/connect/error/ConnectErrors.java:25】
`ConnectQueueError` ya implementa `ConnectErrorConvertible`, emitiendo el queueOverflow/timeout
categorías para condiciones de desbordamiento/caducidad para que la instrumentación de cola fuera de línea pueda conectarse al mismo flujo. 【java/iroha_android/src/main/java/org/hyperledger/iroha/android/connect/error/ConnectQueueError.java:5】
El archivo README del SDK de Android ahora hace referencia a la taxonomía y muestra cómo encapsular las excepciones de transporte.
antes de emitir telemetría, manteniendo la guía de la dApp alineada con la línea base de Swift.【java/iroha_android/README.md:167】

## mapeo de JavaScript

Los clientes Node.js/browser importan `ConnectError`, `ConnectQueueError`, `ConnectErrorCategory` y
`connectErrorFrom()` de `@iroha/iroha-js`. El asistente compartido inspecciona los códigos de estado HTTP,
Códigos de error de nodo (socket, TLS, tiempo de espera), nombres `DOMException` y fallas de códec para emitir el
mismas categorías/códigos documentados en esta nota, mientras que las definiciones de TypeScript modelan la telemetría
anulaciones de atributos para que las herramientas puedan emitir eventos OTEL sin conversión manual.【javascript/iroha_js/src/connectError.js:1】【javascript/iroha_js/index.d.ts:840】
El SDK README documenta el flujo de trabajo y enlaza con esta taxonomía para que los equipos de aplicaciones puedan
copie los fragmentos de instrumentación palabra por palabra. 【javascript/iroha_js/README.md:1387】

## Próximos pasos (cross-SDK)

- **Integración de la cola:** una vez que se envía la cola fuera de línea, asegúrese de retirar/eliminar la lógica
  muestra los valores `ConnectQueueError` para que la telemetría de desbordamiento siga siendo confiable.
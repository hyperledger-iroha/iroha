---
lang: es
direction: ltr
source: docs/source/connect_architecture_feedback.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 097ea58d49f48d059cda762cd719bc62f0b2d6f6ddecedef3f9bac030ae46aec
source_last_modified: "2026-01-03T18:07:58.674563+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

#! Lista de verificación de comentarios sobre la arquitectura de Connect

Esta lista de verificación captura las preguntas abiertas de Connect Session Architecture
hombre de paja que requieren información de los clientes potenciales de Android y JavaScript antes del
Taller entre SDK de febrero de 2026. Úselo para recopilar comentarios de forma asincrónica, realizar un seguimiento
propiedad y desbloquear la agenda del taller.

> La columna Estado/Notas capturó las respuestas finales de los clientes potenciales de Android y JS a partir de
> la sincronización previa al taller de febrero de 2026; vincular nuevos temas de seguimiento en línea si se toman decisiones
> evolucionar.

## Ciclo de vida y transporte de la sesión

| Tema | Propietario de Android | Propietario de JS | Estado / Notas |
|-------|---------------|----------|----------------|
| Estrategia de retroceso de reconexión de WebSocket (exponencial versus lineal limitada) | Redes de Android TL | Líder JS | ✅ Se acordó un retroceso exponencial con jitter, con un límite de 60 s; JS refleja las mismas constantes para la paridad navegador/nodo. |
| Valores predeterminados de capacidad del búfer sin conexión (hombre de paja actual: 32 fotogramas) | Redes de Android TL | Líder JS | ✅ Valor predeterminado de 32 cuadros confirmado con anulación de configuración; Android persiste a través de `ConnectQueueConfig`, JS respeta `window.connectQueueMax`. |
| Notificaciones de reconexión estilo push (FCM/APNS frente a sondeo) | Redes de Android TL | Líder JS | ✅ Android expondrá el gancho FCM opcional para aplicaciones de billetera; JS sigue basándose en encuestas con un retroceso exponencial, teniendo en cuenta las limitaciones de inserción del navegador. |
| Barandillas de cadencia de ping/pong para clientes móviles | Redes de Android TL | Líder JS | ✅ Ping estandarizado de 30 segundos con tolerancia a fallos 3×; Android equilibra el impacto de Doze, JS se limita a ≥15 segundos para evitar la limitación del navegador. |

## Cifrado y gestión de claves

| Tema | Propietario de Android | Propietario de JS | Estado / Notas |
|-------|---------------|----------|----------------|
| Expectativas de almacenamiento clave X25519 (StrongBox, contextos seguros WebCrypto) | Android Cripto TL | Líder JS | ✅ Android almacena X25519 en StrongBox cuando está disponible (recurre a TEE); JS exige WebCrypto de contexto seguro para dApps, recurriendo al puente nativo `iroha_js_host` en Node. |
| ChaCha20-Poly1305 intercambio de gestión nonce entre SDK | Android Cripto TL | Líder JS | ✅ Adopte la API de contador `sequence` compartida con protección envolvente de 64 bits y pruebas compartidas; JS usa contadores BigInt para igualar el comportamiento de Rust. |
| Esquema de carga útil de certificación respaldada por hardware | Android Cripto TL | Líder JS | ✅ Esquema finalizado: `attestation { platform, evidence_b64, statement_hash }`; JS opcional (navegador), Node utiliza el enlace del complemento HSM. |
| Flujo de recuperación de billeteras perdidas (apretón de manos de rotación de claves) | Android Cripto TL | Líder JS | ✅ Se acepta el protocolo de enlace de rotación de billetera: la dApp emite el control `rotate`, la billetera responde con una nueva clave pública + reconocimiento firmado; JS vuelve a introducir las claves del material WebCrypto inmediatamente. |

## Paquetes de permisos y pruebas| Tema | Propietario de Android | Propietario de JS | Estado / Notas |
|-------|---------------|----------|----------------|
| Esquema de permisos mínimos (métodos/eventos/recursos) para GA | Modelo de datos de Android TL | Líder JS | ✅ Línea base GA: `methods`, `events`, `resources`, `constraints`; JS alinea los tipos de TypeScript con el manifiesto de Rust. |
| Carga útil de rechazo de billetera (`reason_code`, mensajes localizados) | Redes de Android TL | Líder JS | ✅ Códigos finalizados (`user_declined`, `permissions_mismatch`, `compliance_failed`, `internal_error`) más `localized_message` opcional. |
| Campos opcionales del paquete de prueba (adjuntos de cumplimiento/KYC) | Modelo de datos de Android TL | Líder JS | ✅ Todos los SDK aceptan `attachments[]` opcional (Norito `AttachmentRef`) e `compliance_manifest_id`; no se requiere ningún cambio de comportamiento. |
| Alineación del esquema JSON Norito frente a estructuras generadas por puente | Modelo de datos de Android TL | Líder JS | ✅ Decisión: preferir estructuras generadas por puentes; La ruta JSON permanece solo para depuración, JS mantiene el adaptador `Value`. |

## Fachadas SDK y forma API

| Tema | Propietario de Android | Propietario de JS | Estado / Notas |
|-------|---------------|----------|----------------|
| Paridad de interfaces asíncronas de alto nivel (`Flow`, iteradores asíncronos) | Redes de Android TL | Líder JS | ✅ Android expone `Flow<ConnectEvent>`; JS utiliza `AsyncIterable<ConnectEvent>`; ambos se asignan a `ConnectEventKind` compartido. |
| Mapeo de taxonomía de errores (`ConnectError`, subclases escritas) | Redes de Android TL | Líder JS | ✅ Adopte la enumeración compartida {`Transport`, `Codec`, `Authorization`, `Timeout`, `QueueOverflow`, `Internal`} con detalles de carga útil específicos de la plataforma. |
| Semántica de cancelación para solicitudes de señales en vuelo | Redes de Android TL | Líder JS | ✅ Se introdujo el control `cancelRequest(hash)`; Ambos SDK muestran corrutinas/promesas cancelables con respecto al reconocimiento de la billetera. |
| Ganchos de telemetría compartidos (eventos, denominación de métricas) | Redes de Android TL | Líder JS | ✅ Nombres de métricas alineados: `connect.queue_depth`, `connect.latency_ms`, `connect.reconnects_total`; exportadores de muestra documentados. |

## Persistencia y registro en diario sin conexión

| Tema | Propietario de Android | Propietario de JS | Estado / Notas |
|-------|---------------|----------|----------------|
| Formato de almacenamiento para fotogramas en cola (binario Norito frente a JSON) | Modelo de datos de Android TL | Líder JS | ✅ Almacene el binario Norito (`.to`) en todas partes; JS utiliza IndexedDB `ArrayBuffer`. |
| Política de retención de revistas y límites de tamaño | Redes de Android TL | Líder JS | ✅ Retención predeterminada 24h y 1MiB por sesión; Configurable a través de `ConnectQueueConfig`. |
| Resolución de conflictos cuando ambas partes repiten fotogramas | Redes de Android TL | Líder JS | ✅ Utilice `sequence` + `payload_hash`; los duplicados se ignoran, los conflictos activan `ConnectError.Internal` con un evento de telemetría. |
| Telemetría para profundidad de cola y éxito de reproducción | Redes de Android TL | Líder JS | ✅ Emite calibre `connect.queue_depth` y contador `connect.replay_success_total`; Ambos SDK se conectan al esquema de telemetría Norito compartido. |

## Picos de implementación y referencias- **Accesorios de Rust Bridge:** `crates/connect_norito_bridge/src/lib.rs` y las pruebas asociadas cubren las rutas canónicas de codificación/decodificación utilizadas por cada SDK.
- **Arnés de demostración de Swift:** Ejercicios `examples/ios/NoritoDemoXcode/NoritoDemoXcodeTests/ConnectViewModelTests.swift` Conecte flujos de sesión con transportes simulados.
- **Gating CI rápido:** ejecute `make swift-ci` al actualizar los artefactos de Connect para validar la paridad de dispositivos, las fuentes del panel y los metadatos de Buildkite `ci/xcframework-smoke:<lane>:device_tag` antes de compartirlos con otros SDK.
- **Pruebas de integración del SDK de JavaScript:** `javascript/iroha_js/test/integrationTorii.test.js` valida el estado de conexión/ayudantes de sesión con Torii.
- **Notas sobre la resiliencia del cliente Android:** `java/iroha_android/README.md:150` documenta los experimentos de conectividad actuales que inspiraron los valores predeterminados de cola/retroceso.

## Artículos de preparación para el taller

- [x] Android: asigna una persona de contacto para cada fila de la tabla anterior.
- [x] JS: asigna una persona de contacto para cada fila de la tabla anterior.
- [x] Recopile enlaces a picos de implementación o experimentos existentes.
- [x] Programar una revisión previa al trabajo antes del consejo de febrero de 2026 (reservado para el 29 de enero de 2026 a las 15:00 UTC con Android TL, JS Lead, Swift Lead).
- [x] Actualización `docs/source/connect_architecture_strawman.md` con respuestas aceptadas.

## Paquete de lectura previa

- ✅ Paquete registrado bajo `artifacts/connect/pre-read/20260129/` (generado a través de `make docs-html` después de actualizar el muñeco de paja, las guías del SDK y esta lista de verificación).
- 📄 Resumen + pasos de distribución en vivo en `docs/source/project_tracker/connect_architecture_pre_read.md`; incluya el enlace en la invitación al taller de febrero de 2026 y en el recordatorio `#sdk-council`.
- 🔁 Al actualizar el paquete, actualice la ruta y el hash dentro de la nota de lectura previa y archive el anuncio en `status.md` en los registros de preparación de IOS7/AND7.
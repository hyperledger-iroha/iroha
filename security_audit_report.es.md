<!-- Auto-generated stub for Spanish (es) translation. Replace this content with the full translation. -->

---
lang: es
direction: ltr
source: security_audit_report.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4343a8f72823a2539a2e79e80a1c7162d455e03b450e2fd613f01ac5c61aec63
source_last_modified: "2026-03-26T13:01:48.980087+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Informe de auditoría de seguridad

Fecha: 2026-03-26

## Resumen ejecutivo

Esta auditoría se centró en las superficies de mayor riesgo en el árbol actual: flujos HTTP/API/auth Torii, transporte P2P, API de manejo de secretos, protecciones de transporte SDK y la ruta del desinfectante de archivos adjuntos.

Encontré 6 problemas procesables:

- 2 hallazgos de gravedad alta
- 4 hallazgos de gravedad media

Los problemas más importantes son:

1. Torii actualmente registra encabezados de solicitudes entrantes para cada solicitud HTTP, lo que puede exponer tokens de portador, tokens de API, tokens de sesión/arranque del operador y marcadores mTLS reenviados a los registros.
2. Varias rutas públicas Torii y SDK aún admiten el envío de valores `private_key` sin procesar al servidor para que Torii pueda iniciar sesión en nombre de la persona que llama.
3. Varias rutas "secretas" se tratan como órganos de solicitud ordinarios, incluida la derivación de semillas confidencial y la autenticación de solicitudes canónicas en algunos SDK.

## Método

- Revisión estática de las rutas de manejo de secretos Torii, P2P, cripto/VM y SDK
- Comandos de validación específicos:
  - `cargo check -p iroha_torii --lib --message-format short` -> pasar
  - `cargo check -p iroha_p2p --message-format short` -> pasar
  - `cargo test -p iroha_torii --lib confidential_derive_keyset_endpoint_roundtrip -- --nocapture` -> pasar
  - `cargo deny check advisories bans sources --hide-inclusion-graph` -> aprobado, solo advertencias de versión duplicada
- No completado en este pase:
  - compilación/prueba/clippy del espacio de trabajo completo
  - Conjuntos de pruebas Swift/Gradle
  - Validación del tiempo de ejecución CUDA/Metal

## Hallazgos

### SA-001 Alto: Torii registra encabezados de solicitudes confidenciales a nivel mundialImpacto: cualquier implementación que envíe solicitud de seguimiento puede filtrar tokens de portador/API/operador y material de autenticación relacionado en los registros de la aplicación.

Evidencia:

- `crates/iroha_torii/src/lib.rs:20752` habilita `TraceLayer::new_for_http()`
- `crates/iroha_torii/src/lib.rs:20753` habilita `DefaultMakeSpan::default().include_headers(true)`
- Los nombres de encabezado confidenciales se utilizan activamente en otras partes del mismo servicio:
  - `crates/iroha_torii/src/operator_auth.rs:40`
  - `crates/iroha_torii/src/operator_auth.rs:41`
  - `crates/iroha_torii/src/operator_auth.rs:42`
  - `crates/iroha_torii/src/operator_auth.rs:43`

Por qué esto es importante:

- `include_headers(true)` registra los valores completos del encabezado entrante en intervalos de seguimiento.
- Torii acepta material de autenticación en encabezados como `Authorization`, `x-api-token`, `x-iroha-operator-session`, `x-iroha-operator-token` e `x-forwarded-client-cert`.
- Por lo tanto, un compromiso del receptor de registros, una recopilación de registros de depuración o un paquete de soporte pueden convertirse en un evento de divulgación de credenciales.

Remediación recomendada:

- Dejar de incluir encabezados de solicitud completos en los tramos de producción.
- Agregue redacción explícita para encabezados sensibles a la seguridad si aún es necesario el registro de encabezados para la depuración.
- Trate el registro de solicitudes/respuestas como secreto de forma predeterminada, a menos que los datos estén positivamente incluidos en la lista de permitidos.

### SA-002 Alto: Las API públicas Torii aún aceptan claves privadas sin formato para la firma del lado del servidor

Impacto: se anima a los clientes a transmitir claves privadas sin procesar a través de la red para que el servidor pueda firmar en su nombre, creando un canal de exposición secreto innecesario en las capas de API, SDK, proxy y memoria del servidor.

Evidencia:- La documentación de la ruta de gobernanza anuncia explícitamente la firma del lado del servidor:
  - `crates/iroha_torii/src/gov.rs:495`
- La implementación de la ruta analiza la clave privada proporcionada y firma en el lado del servidor:
  - `crates/iroha_torii/src/gov.rs:1088`
  - `crates/iroha_torii/src/gov.rs:1091`
  - `crates/iroha_torii/src/gov.rs:1123`
  - `crates/iroha_torii/src/gov.rs:1125`
- Los SDK serializan activamente `private_key` en cuerpos JSON:
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:47`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:50`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:260`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:261`

Notas:

- Este patrón no está aislado de una familia de rutas. El árbol actual contiene el mismo modelo de conveniencia en gestión, efectivo fuera de línea, suscripciones y otras DTO orientadas a aplicaciones.
- Las comprobaciones de transporte solo HTTPS reducen el transporte accidental de texto sin formato, pero no resuelven el manejo de secretos del lado del servidor ni el riesgo de exposición de memoria o registro.

Remediación recomendada:

- Desaprobar todas las solicitudes de DTO que transporten datos `private_key` sin procesar.
- Requerir que los clientes firmen localmente y envíen firmas o transacciones/sobres completamente firmados.
- Elimine los ejemplos `private_key` de OpenAPI/SDK después de una ventana de compatibilidad.

### SA-003 Medio: La derivación de clave confidencial envía material semilla secreto a Torii y lo repite

Impacto: La API confidencial de derivación de claves convierte el material inicial en datos de carga útil de solicitud/respuesta normales, lo que aumenta la posibilidad de divulgación de semillas a través de servidores proxy, middleware, registros, seguimientos, informes de fallos o uso indebido del cliente.

Evidencia:- La solicitud acepta material de semilla directamente:
  - `crates/iroha_torii/src/routing.rs:2736`
  - `crates/iroha_torii/src/routing.rs:2738`
  -`crates/iroha_torii/src/routing.rs:2740`
- El esquema de respuesta hace eco de la semilla tanto en hexadecimal como en base64:
  - `crates/iroha_torii/src/routing.rs:2745`
  - `crates/iroha_torii/src/routing.rs:2746`
  - `crates/iroha_torii/src/routing.rs:2747`
- El controlador vuelve a codificar explícitamente y devuelve la semilla:
  - `crates/iroha_torii/src/routing.rs:2797`
  - `crates/iroha_torii/src/routing.rs:2801`
  - `crates/iroha_torii/src/routing.rs:2802`
  - `crates/iroha_torii/src/routing.rs:2804`
- El SDK de Swift expone esto como un método de red normal y conserva la semilla repetida en el modelo de respuesta:
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4716`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4717`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4718`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:11912`
  -`IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:11926`

Remediación recomendada:

- Prefiera la derivación de clave local en el código CLI/SDK y elimine por completo la ruta de derivación remota.
- Si la ruta debe permanecer, nunca devolver la semilla en la respuesta y marcar los cuerpos portadores de semillas como sensibles en todas las guardas de transporte y rutas de telemetría/tala.

### SA-004 Medio: La detección de sensibilidad de transporte del SDK tiene puntos ciegos para material secreto que no es `private_key`

Impacto: algunos SDK aplicarán HTTPS para solicitudes `private_key` sin procesar, pero aún permitirán que otro material de solicitud sensible a la seguridad viaje a través de HTTP inseguro o a hosts que no coincidan.

Evidencia:- Swift trata los encabezados de autenticación de solicitudes canónicas como sensibles:
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:4`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:7`
  -`IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:8`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:9`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:10`
- Pero Swift todavía solo coincide con el cuerpo en `"private_key"`:
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:18`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:66`
  -`IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:69`
- Kotlin solo reconoce los encabezados `authorization` e `x-api-token`, luego recurre a la misma heurística del cuerpo `"private_key"`:
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:50`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:53`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:58`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:61`
- Java/Android tiene la misma limitación:
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:24`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:25`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:100`
  -`java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:104`
- Los firmantes de solicitudes canónicas de Kotlin/Java generan encabezados de autenticación adicionales que no están clasificados como confidenciales por sus propios guardias de transporte:
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:17`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:18`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:51`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:25`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:26`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:27`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:28`

Remediación recomendada:

- Reemplazar el escaneo corporal heurístico con una clasificación de solicitud explícita.
- Trate los encabezados de autenticación canónicos, los campos semilla/frase de contraseña, los encabezados de mutación firmados y cualquier campo futuro que contenga secretos como confidenciales por contrato, no por coincidencia de subcadena.
- Mantenga las reglas de sensibilidad alineadas en Swift, Kotlin y Java.

### SA-005 Medio: El archivo adjunto "sandbox" es solo un subproceso más `setrlimit`Impacto: el desinfectante de archivos adjuntos se describe y se informa como "en zona protegida", pero la implementación es solo una bifurcación/ejecutiva del binario actual con límites de recursos. Un analizador o un exploit de archivo aún se ejecutaría con el mismo usuario, vista del sistema de archivos y privilegios de proceso/red ambiental que Torii.

Evidencia:

- El camino exterior marca el resultado como zona de pruebas después de generar un niño:
  - `crates/iroha_torii/src/zk_attachments.rs:756`
  - `crates/iroha_torii/src/zk_attachments.rs:760`
  - `crates/iroha_torii/src/zk_attachments.rs:776`
  - `crates/iroha_torii/src/zk_attachments.rs:782`
- El niño utiliza por defecto el ejecutable actual:
  - `crates/iroha_torii/src/zk_attachments.rs:913`
  - `crates/iroha_torii/src/zk_attachments.rs:919`
- El subproceso vuelve explícitamente a `AttachmentSanitizerMode::InProcess`:
  - `crates/iroha_torii/src/zk_attachments.rs:1794`
  - `crates/iroha_torii/src/zk_attachments.rs:1803`
- El único refuerzo aplicado es CPU/espacio de direcciones `setrlimit`:
  - `crates/iroha_torii/src/zk_attachments.rs:1845`
  -`crates/iroha_torii/src/zk_attachments.rs:1850`
  - `crates/iroha_torii/src/zk_attachments.rs:1851`
  - `crates/iroha_torii/src/zk_attachments.rs:1872`

Remediación recomendada:

- Implemente un entorno limitado de sistema operativo real (por ejemplo, espacios de nombres/seccomp/landlock/aislamiento estilo cárcel, pérdida de privilegios, sin red, sistema de archivos restringido) o deje de etiquetar el resultado como `sandboxed`.
- Trate el diseño actual como "aislamiento de subprocesos" en lugar de "espacio aislado" en API, telemetría y documentos hasta que exista un verdadero aislamiento.

### SA-006 Medio: Los transportes P2P TLS/QUIC opcionales desactivan la verificación de certificadosImpacto: cuando `quic` o `p2p_tls` está habilitado, el canal proporciona cifrado pero no autentica el punto final remoto. Un atacante activo en la ruta aún puede retransmitir o terminar el canal, frustrando las expectativas de seguridad normales que los operadores asocian con TLS/QUIC.

Evidencia:

- QUIC documenta explícitamente la verificación de certificados permisivo:
  - `crates/iroha_p2p/src/transport.rs:12`
  - `crates/iroha_p2p/src/transport.rs:13`
  - `crates/iroha_p2p/src/transport.rs:14`
  - `crates/iroha_p2p/src/transport.rs:15`
- El verificador QUIC acepta incondicionalmente el certificado del servidor:
  - `crates/iroha_p2p/src/transport.rs:33`
  - `crates/iroha_p2p/src/transport.rs:35`
  - `crates/iroha_p2p/src/transport.rs:44`
  - `crates/iroha_p2p/src/transport.rs:112`
  - `crates/iroha_p2p/src/transport.rs:114`
  - `crates/iroha_p2p/src/transport.rs:115`
- El transporte TLS sobre TCP hace lo mismo:
  - `crates/iroha_p2p/src/transport.rs:229`
  - `crates/iroha_p2p/src/transport.rs:232`
  - `crates/iroha_p2p/src/transport.rs:241`
  - `crates/iroha_p2p/src/transport.rs:279`
  - `crates/iroha_p2p/src/transport.rs:281`
  - `crates/iroha_p2p/src/transport.rs:282`

Remediación recomendada:

- Verifique los certificados de pares o agregue un enlace de canal explícito entre el protocolo de enlace firmado de capa superior y la sesión de transporte.
- Si el comportamiento actual es intencional, cambie el nombre o documente la característica como transporte cifrado no autenticado para que los operadores no la confundan con la autenticación de pares TLS completa.

## Orden de remediación recomendada1. Solucione SA-001 inmediatamente redactando o deshabilitando el registro de encabezados.
2. Diseñar y enviar un plan de migración para SA-002 para que las claves privadas sin procesar dejen de cruzar el límite de la API.
3. Eliminar o limitar la ruta remota de obtención de claves confidenciales y clasificar los cuerpos portadores de semillas como sensibles.
4. Alinear las reglas de sensibilidad del transporte del SDK en Swift/Kotlin/Java.
5. Decida si el saneamiento de archivos adjuntos necesita una zona de pruebas real o un cambio de nombre/alcance honesto.
6. Aclarar y reforzar el modelo de amenazas P2P TLS/QUIC antes de que los operadores habiliten aquellos transportes que esperan TLS autenticado.

## Notas de validación

- `cargo check -p iroha_torii --lib --message-format short` pasó.
- `cargo check -p iroha_p2p --message-format short` pasó.
- `cargo deny check advisories bans sources --hide-inclusion-graph` pasó después de ejecutar fuera del sandbox; emitió advertencias de versión duplicada pero informó `advisories ok, bans ok, sources ok`.
- Durante esta auditoría se inició una prueba Torii enfocada para la ruta confidencial de derivación del conjunto de claves, pero no se completó antes de que se redactara el informe; Independientemente, el hallazgo está respaldado por la inspección directa de la fuente.
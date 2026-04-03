<!-- Auto-generated stub for Spanish (es) translation. Replace this content with the full translation. -->

---
lang: es
direction: ltr
source: security_best_practices_report.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6055090fbfab93113392610b3eb78ef0a0e97d6fef4a94292c5609ec5562e925
source_last_modified: "2026-03-26T06:12:11.487656+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Informe de mejores prácticas de seguridad

Fecha: 2026-03-25

## Resumen ejecutivo

Actualicé el informe anterior Torii/Soracloud con el espacio de trabajo actual
código y amplió la revisión en el servidor de mayor riesgo, SDK y
superficies de criptografía/serialización. Esta auditoría confirmó inicialmente tres
Problemas de ingreso/autenticación: dos de gravedad alta y uno de gravedad media.
Esos tres hallazgos ahora están cerrados en el árbol actual por la remediación.
se describe a continuación. Se confirma la revisión de seguimiento del transporte y el envío interno
nueve problemas adicionales de gravedad media: un enlace de identidad P2P saliente
brecha, una degradación predeterminada de TLS P2P saliente, dos errores de límite de confianza Torii en
entrega de webhook y envío interno de MCP, un transporte sensible entre SDK
brecha en los clientes Swift, Java/Android, Kotlin y JS, uno SoraFS
brecha de política de proxy confiable/IP de cliente, un proxy local SoraFS
brecha de enlace/autenticación, un respaldo de texto plano de búsqueda geográfica de telemetría de pares,
y una brecha de clave de velocidad/bloqueo de falla de apertura de IP remota con autenticación de operador. esos
Los hallazgos posteriores también se cierran en el árbol actual.Los cuatro hallazgos de Soracloud informados anteriormente sobre claves privadas sin procesar durante
HTTP, ejecución de proxy de lectura local solo interna, tiempo de ejecución público no medido
la reserva y el arrendamiento de conexión de IP remota ya no están disponibles en el código actual.
Estos están marcados como cerrados/reemplazados a continuación con referencias de código actualizadas.Esto siguió siendo una auditoría centrada en códigos en lugar de un ejercicio exhaustivo del equipo rojo.
Prioricé las rutas de ingreso y solicitud de autenticación Torii accesibles externamente, luego
IVM, `iroha_crypto`, `norito`, el SDK de Swift/Android/JS comprobado al azar
ayudantes de firma de solicitudes, la ruta geográfica de telemetría de pares y el SoraFS
asistentes de proxy de estación de trabajo más la política de IP de cliente de pin/puerta de enlace SoraFS
superficies. No hay ningún problema confirmado en vivo de esa entrada/autenticación, política de salida,
Geo de telemetría entre pares, valores predeterminados de transporte P2P de muestra, envío de MCP, SDK de muestra
transporte, bloqueo de autenticación de operador/codificación de velocidad, SoraFS proxy de confianza/IP de cliente
La política o el segmento de proxy local permanecen después de las correcciones en este informe.
El endurecimiento del seguimiento también amplió los conjuntos de verdades de inicio cerrados por fallas para
rutas del acelerador IVM CUDA/Metal muestreadas; ese trabajo no confirmó una nueva
problema de apertura fallida. El Metal Ed25519 muestreado
la ruta de la firma ahora se restaura en este host después de corregir múltiples derivas ref10
puntos en los puertos Metal/CUDA: manejo de punto de base positivo en verificación,
la constante `d2`, la ruta de reducción exacta `fe_sq2`, el final perdido
`fe_mul` paso de transporte y la normalización del campo postoperatorio faltante que permitió que la extremidad
Los límites se desplazan a través de la escalera escalar. Cobertura de regresión de metales enfocada ahora
mantiene la canalización de firma habilitada y verifica `[true, false]` en elacelerador contra la ruta de referencia de la CPU. La verdad de las startups muestreadas se establece ahora
También sondee directamente el vector vivo (`vadd64`, `vand`, `vxor`, `vor`) y
núcleos por lotes AES de una sola ronda tanto en Metal como en CUDA antes de esos backends
permanecer habilitado. El escaneo de dependencias posterior agregó siete hallazgos de terceros en vivo
al trabajo pendiente, pero desde entonces el árbol actual ha eliminado ambos activos `tar`
avisos eliminando la dependencia `xtask` Rust `tar` y reemplazando la
`iroha_crypto` `libsodium-sys-stable` pruebas de interoperabilidad con respaldo de OpenSSL
equivalentes. El árbol actual también ha reemplazado las dependencias directas de PQ.
marcado en ese barrido, migrando `soranet_pq`, `iroha_crypto` e `ivm` desde
`pqcrypto-dilithium` / `pqcrypto-kyber` a
`pqcrypto-mldsa` / `pqcrypto-mlkem` conservando el ML-DSA / existente
Superficie API ML-KEM. Un pase de dependencia posterior el mismo día fijó el espacio de trabajo
Versiones `reqwest` / `rustls` a versiones de parches parcheadas, lo que mantiene
`rustls-webpki` en la línea fija `0.103.10` en la resolución actual. el unico
Las excepciones restantes a la política de dependencia son las dos transitivas no mantenidas.
macro cajas (`derivative`, `paste`), que ahora se aceptan explícitamente en
`deny.toml` porque no existe una actualización segura y eliminarlas requeriría
reemplazar o vender múltiples pilas ascendentes. ElEl trabajo restante del acelerador es
validación en tiempo de ejecución de la corrección CUDA reflejada y la verdad CUDA expandida configurada en
un host con soporte de controlador CUDA en vivo, no una corrección confirmada o falla de apertura
problema en el árbol actual.

## Alta gravedad

### SEC-05: La verificación de solicitud canónica de la aplicación superó los umbrales multifirma (cerrado el 24 de marzo de 2026)

Impacto:

- Cualquier clave de miembro único de una cuenta controlada por múltiples firmas puede autorizar
  Solicitudes orientadas a la aplicación que se supone que requieren un umbral o una ponderación.
  quórum.
- Esto afecta a todos los puntos finales que confían en `verify_canonical_request`, incluido
  Ingreso de mutación firmado por Soracloud, acceso a contenido y ZK de cuenta firmada
  arrendamiento adjunto.

Evidencia:

- `verify_canonical_request` expande un controlador multigrado al miembro completo
  lista de claves públicas y acepta la primera clave que verifica la solicitud
  firma, sin evaluar umbral ni peso acumulado:
  `crates/iroha_torii/src/app_auth.rs:198-210`.
- El modelo de política multifirma real lleva tanto un `threshold` como un ponderado.
  miembros, y rechaza políticas cuyo umbral exceda el peso total:
  `crates/iroha_data_model/src/account/controller.rs:92-95`,
  `crates/iroha_data_model/src/account/controller.rs:163-178`,
  `crates/iroha_data_model/src/account/controller.rs:188-196`.
- El asistente está en la ruta de autorización para el ingreso de la mutación de Soracloud en
  `crates/iroha_torii/src/lib.rs:2141-2157`, acceso a cuenta firmada de contenido en
  `crates/iroha_torii/src/content.rs:359-360` y arrendamiento adjunto en
  `crates/iroha_torii/src/lib.rs:7962-7968`.

Por qué esto es importante:- El firmante de la solicitud se considera la autoridad de la cuenta para la admisión HTTP.
  pero la implementación degrada silenciosamente las cuentas multifirma a "cualquier
  El miembro puede actuar solo."
- Eso convierte una capa de firma HTTP de defensa en profundidad en una autorización
  omisión para cuentas protegidas con múltiples firmas.

Recomendación:

- Rechace las cuentas controladas por múltiples firmas en la capa de autenticación de la aplicación hasta que
  existe el formato de testigo adecuado o ampliar el protocolo para que la solicitud HTTP
  lleva y verifica un conjunto completo de testigos multifirma que satisface el umbral y
  peso.
- Agregue regresiones que cubran el middleware de mutación de Soracloud, la autenticación de contenido y ZK.
  archivos adjuntos para firmas multifirma por debajo del umbral.

Estado de remediación:

- Cerrado en el código actual al no cerrarse en cuentas controladas por múltiples firmas en
  `crates/iroha_torii/src/app_auth.rs`.
- El verificador ya no acepta la semántica "cualquier miembro puede firmar" para
  autorización HTTP multifirma; Las solicitudes multifirma se rechazan hasta que
  Existe un formato de testigo que satisface el umbral.
- La cobertura de regresión ahora incluye un caso de rechazo multifirma dedicado en
  `crates/iroha_torii/src/app_auth.rs`.

## Alta gravedad

### SEC-06: Las firmas de solicitudes canónicas de la aplicación se podían reproducir indefinidamente (cerrado el 24 de marzo de 2026)

Impacto:- Una solicitud válida capturada se puede reproducir porque el mensaje firmado no tiene
  marca de tiempo, nonce, caducidad o caché de reproducción.
- Esto puede repetir solicitudes de mutación de Soracloud que cambian de estado y reemitir
  operaciones de contenido/archivos adjuntos vinculadas a la cuenta mucho después del cliente original
  los pretendía.

Evidencia:

- Torii define la solicitud canónica de la aplicación como única
  `METHOD + path + sorted query + body hash` en
  `crates/iroha_torii/src/app_auth.rs:1-17` y
  `crates/iroha_torii/src/app_auth.rs:74-89`.
- El verificador acepta sólo `X-Iroha-Account` e `X-Iroha-Signature` y no
  no imponer la actualización ni mantener un caché de reproducción:
  `crates/iroha_torii/src/app_auth.rs:137-218`.
- Los asistentes JS, Swift y Android SDK generan el mismo encabezado propenso a reproducción
  par sin campos nonce/marca de tiempo:
  `javascript/iroha_js/src/canonicalRequest.js:50-82`,
  `IrohaSwift/Sources/IrohaSwift/CanonicalRequest.swift:41-68`, y
  `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:67-106`.
- La ruta de firma del operador de Torii ya utiliza el patrón más fuerte
  Falta la ruta de acceso a la aplicación: marca de tiempo, nonce y caché de reproducción en
  `crates/iroha_torii/src/operator_signatures.rs:1-21` y
  `crates/iroha_torii/src/operator_signatures.rs:266-294`.

Por qué esto es importante:

- HTTPS por sí solo no impide la reproducción mediante un proxy inverso, un registrador de depuración,
  host del cliente comprometido o cualquier intermediario que pueda registrar solicitudes válidas.
- Debido a que se implementa el mismo esquema en todos los SDK de clientes principales, la reproducción
  La debilidad es sistémica y no sólo del servidor.

Recomendación:- Agregue material actualizado firmado a las solicitudes de autenticación de aplicaciones, como mínimo una marca de tiempo
  y nonce, y rechazar tuplas obsoletas o reutilizadas con un caché de reproducción limitado.
- Versione explícitamente el formato de solicitud canónica de la aplicación para que Torii y los SDK puedan
  desaprobar el antiguo esquema de dos encabezados de forma segura.
- Agregue regresiones que demuestren el rechazo de la repetición de las mutaciones y el contenido de Soracloud.
  acceso y archivo adjunto CRUD.

Estado de remediación:

- Cerrado en código actual. Torii ahora requiere el esquema de cuatro encabezados
  (`X-Iroha-Account`, `X-Iroha-Signature`, `X-Iroha-Timestamp-Ms`,
  `X-Iroha-Nonce`) y firma/verifica
  `METHOD + path + sorted query + body hash + timestamp + nonce` en
  `crates/iroha_torii/src/app_auth.rs`.
- La validación de frescura ahora impone una ventana de reloj sesgado limitada, valida
  forma de nonce y rechaza los nonce reutilizados con un caché de reproducción en memoria cuyo
  Las perillas tienen superficie a través de `crates/iroha_config/src/parameters/{defaults,actual,user}.rs`.
- Los asistentes JS, Swift y Android ahora emiten el mismo formato de cuatro encabezados en
  `javascript/iroha_js/src/canonicalRequest.js`,
  `IrohaSwift/Sources/IrohaSwift/CanonicalRequest.swift`, y
  `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java`.
- La cobertura de regresión ahora incluye verificación de firma positiva más repetición.
  casos de rechazo de marca de tiempo obsoleta y de frescura faltante en
  `crates/iroha_torii/src/app_auth.rs`.

## Gravedad media

### SEC-07: La aplicación de mTLS confió en un encabezado reenviado falsificado (cerrado el 24 de marzo de 2026)

Impacto:- Las implementaciones que dependen de `require_mtls` se pueden omitir si Torii está directamente
  accesible o el proxy frontal no elimina los datos proporcionados por el cliente.
  `x-forwarded-client-cert`.
- El problema depende de la configuración, pero cuando se activa se convierte en un reclamo
  requisito de certificado de cliente en una verificación de encabezado simple.

Evidencia:

- La activación Norito-RPC aplica `require_mtls` llamando
  `norito_rpc_mtls_present`, que sólo comprueba si
  `x-forwarded-client-cert` existe y no está vacío:
  `crates/iroha_torii/src/lib.rs:1897-1926`.
- Los flujos de inicio de sesión/arranque de autenticación del operador llaman a `check_common`, que solo rechaza
  cuando `mtls_present(headers)` es falso:
  `crates/iroha_torii/src/operator_auth.rs:562-570`.
- `mtls_present` también es solo un registro `x-forwarded-client-cert` no vacío
  `crates/iroha_torii/src/operator_auth.rs:1212-1216`.
- Esos controladores de autenticación de operador todavía están expuestos como rutas en
  `crates/iroha_torii/src/lib.rs:16658-16672`.

Por qué esto es importante:

- Una convención de encabezado reenviado sólo es confiable cuando Torii se encuentra detrás de un
  proxy reforzado que elimina y reescribe el encabezado. El código no verifica
  ese supuesto de implementación en sí.
- Los controles de seguridad que dependen silenciosamente de la higiene del proxy inverso son fáciles de implementar.
  configurar incorrectamente durante los cambios de ruta de preparación, canary o respuesta a incidentes.

Recomendación:- Preferir la aplicación directa del Estado del transporte cuando sea posible. Si un apoderado debe ser
  utilizado, confíe en un canal proxy autenticado a Torii y requiera una lista de permitidos
  o certificación firmada de ese proxy en lugar de presencia de encabezado sin formato.
- Documente que `require_mtls` no es seguro en los oyentes Torii directamente expuestos.
- Agregar pruebas negativas para entrada `x-forwarded-client-cert` falsificada en Norito-RPC
  y rutas de arranque con autenticación de operador.

Estado de remediación:

- Cerrado en el código actual vinculando la confianza del encabezado reenviado al proxy configurado
  CIDR en lugar de solo presencia de encabezado sin formato.
- `crates/iroha_torii/src/limits.rs` ahora proporciona el compartido
  Puerta `has_trusted_forwarded_header(...)` y ambas Norito-RPC
  (`crates/iroha_torii/src/lib.rs`) y autenticación de operador
  (`crates/iroha_torii/src/operator_auth.rs`) úselo con el par TCP que llama
  dirección.
- `iroha_config` ahora expone `mtls_trusted_proxy_cidrs` para ambos
  autenticación de operador e Norito-RPC; los valores predeterminados son solo de bucle invertido.
- La cobertura de regresión ahora rechaza la entrada `x-forwarded-client-cert` falsificada de
  un control remoto que no es de confianza tanto en la autenticación del operador como en el asistente de límites compartidos.

## Gravedad media

### SEC-08: Los marcados P2P salientes no vincularon la clave autenticada con la identificación del par previsto (cerrado el 25 de marzo de 2026)

Impacto:- Un marcado saliente al par `X` podría completarse como cualquier otro par `Y` cuya clave
  firmó con éxito el protocolo de enlace de la capa de aplicación, porque el protocolo de enlace
  autenticó "una clave en esta conexión" pero nunca verificó que la clave fuera
  la identificación del par al que el actor de la red pretendía llegar.
- En superposiciones autorizadas, las comprobaciones posteriores de topología/lista de permitidos aún eliminan el
  clave incorrecta, por lo que esto fue principalmente un error de sustitución/accesibilidad en lugar de
  que un error de suplantación de consenso directo. En superposiciones públicas podría permitir que un
  La dirección comprometida, la respuesta DNS o el punto final de retransmisión sustituyen por uno diferente.
  identidad del observador en un dial de salida.

Evidencia:

- El estado del par saliente almacena el `peer_id` previsto en
  `crates/iroha_p2p/src/peer.rs:5153-5179`, pero el antiguo flujo de apretón de manos
  eliminó ese valor antes de la verificación de la firma.
- `GetKey::read_their_public_key` verificó la carga útil del protocolo de enlace firmado y
  luego construyó inmediatamente un `Peer` a partir de la clave pública remota anunciada
  en `crates/iroha_p2p/src/peer.rs:6266-6355`, sin comparación con el
  `peer_id` suministrado originalmente a `connecting(...)`.
- La misma pila de transporte desactiva explícitamente el certificado TLS/QUIC
  verificación para P2P en `crates/iroha_p2p/src/transport.rs`, vinculando así el
  La clave autenticada en la capa de aplicación para la identificación del par deseado es la clave
  verificación de identidad en conexiones salientes.

Por qué esto es importante:- El diseño coloca intencionalmente la autenticación entre pares por encima del transporte.
  capa, lo que hace que la clave de apretón de manos verifique el único enlace de identidad duradero
  en diales salientes.
- Sin esa verificación, la capa de red podría tratar silenciosamente "con éxito"
  autenticado a algún par" como equivalente a "llegamos al par que marcamos",
  lo cual es una garantía más débil y puede distorsionar el estado de topología/reputación.

Recomendación:

- Llevar el `peer_id` saliente previsto a través de las etapas de protocolo de enlace firmado y
  falla cerrada si la clave remota verificada no coincide.
- Mantenga una regresión enfocada que demuestre un apretón de manos firmado válidamente por parte del
  La clave incorrecta se rechaza mientras que un protocolo de enlace firmado normal aún tiene éxito.

Estado de remediación:

- Cerrado en código actual. `ConnectedTo` y los estados de protocolo de enlace posteriores ahora
  llevar el `PeerId` saliente esperado, y
  `GetKey::read_their_public_key` rechaza una clave autenticada que no coincide con
  `HandshakePeerMismatch` en `crates/iroha_p2p/src/peer.rs`.
- La cobertura de regresión enfocada ahora incluye
  `outgoing_handshake_rejects_unexpected_peer_identity` y el existente
  ruta positiva `handshake_v1_defaults_to_trust_gossip` en
  `crates/iroha_p2p/src/peer.rs`.

### SEC-09: La entrega de webhook HTTPS/WSS se volvió a resolver con nombres de host examinados en el momento de la conexión (cerrado el 25 de marzo de 2026)

Impacto:- Entrega segura de webhook, respuestas DNS de destino validadas contra el webhook
  política de salida, pero luego descartó esas direcciones examinadas y dejó que el cliente
  La pila resuelve el nombre de host nuevamente durante la conexión HTTPS o WSS real.
- Un atacante que pueda influir en el DNS entre la validación y el tiempo de conexión podría
  potencialmente volver a vincular un nombre de host previamente permitido a un servidor privado o bloqueado.
  destino exclusivo del operador y omitir la protección de webhook basada en CIDR.

Evidencia:

- El guardia de salida resuelve y filtra las direcciones de destino candidatas en
  `crates/iroha_torii/src/webhook.rs:1746-1829` y las rutas de entrega seguras
  pase esas listas de direcciones examinadas a los ayudantes HTTPS/WSS.
- El antiguo asistente HTTPS creó un cliente genérico con la URL original.
  host en `crates/iroha_torii/src/webhook.rs` y no vinculó la conexión
  al conjunto de direcciones examinadas, lo que significó que la resolución DNS se produjo nuevamente dentro
  el cliente HTTP.
- El antiguo ayudante de WSS también se llama `tokio_tungstenite::connect_async(url)`
  contra el nombre de host original, que también volvió a resolver el host en lugar de
  reutilizar la dirección ya aprobada.

Por qué esto es importante:

- Las listas permitidas de destino solo funcionan si la dirección que se verificó es la indicada
  el cliente realmente se conecta.
- Volver a resolver después de la aprobación de la política crea una brecha de vinculación de DNS/TOCTOU en un
  camino en el que los operadores probablemente confiarán para una contención estilo SSRF.

Recomendación:- Fije las respuestas DNS examinadas en la ruta de conexión HTTPS real y al mismo tiempo preserve
  el nombre de host original para la validación de certificado/SNI.
- Para WSS, conecte el socket TCP directamente a una dirección examinada y ejecute TLS
  protocolo de enlace websocket sobre esa transmisión en lugar de llamar a un protocolo basado en nombre de host
  conector de conveniencia.

Estado de remediación:

- Cerrado en código actual. `crates/iroha_torii/src/webhook.rs` ahora deriva
  `https_delivery_dns_override(...)` y
  `websocket_pinned_connect_addr(...)` del conjunto de direcciones examinadas.
- La entrega HTTPS ahora usa `reqwest::Client::builder().resolve_to_addrs(...)`
  por lo que el nombre de host original permanece visible para TLS mientras la conexión TCP está activa.
  anclado a las direcciones ya aprobadas.
- La entrega de WSS ahora abre un `TcpStream` sin procesar en una dirección examinada y realiza
  `tokio_tungstenite::client_async_tls_with_config(...)` sobre esa transmisión,
  lo que evita una segunda búsqueda de DNS después de la validación de la política.
- La cobertura de regresión ahora incluye
  `https_delivery_dns_override_pins_vetted_domain_addresses`,
  `https_delivery_dns_override_skips_ip_literals`, y
  `websocket_pinned_connect_addr_pins_secure_delivery_when_guarded` en
  `crates/iroha_torii/src/webhook.rs`.

### SEC-10: Bucle invertido sellado de despacho de ruta interna de MCP y privilegio de lista de permitidos heredado (cerrado el 25 de marzo de 2026)

Impacto:- Cuando se habilitó Torii MCP, el envío de herramientas internas reescribió cada solicitud como
  loopback independientemente de la persona que llama. Rutas que confían en el CIDR de la persona que llama para
  Por lo tanto, el privilegio o la omisión de limitación podrían considerar el tráfico MCP como
  `127.0.0.1`.
- El problema estaba controlado por la configuración porque MCP está deshabilitado de forma predeterminada y el
  Las rutas afectadas aún dependen de una lista de permitidos o una confianza de loopback similar.
  política, pero convirtió a MCP en un puente de escalada de privilegios una vez que esos
  Las perillas se habilitaron juntas.

Evidencia:

- `dispatch_route(...)` en `crates/iroha_torii/src/mcp.rs` previamente insertado
  `x-iroha-remote-addr: 127.0.0.1` y loopback sintético `ConnectInfo` para
  cada solicitud enviada internamente.
- `iroha.parameters.get` está expuesto en la superficie del MCP en modo de solo lectura, y
  `/v1/parameters` omite la autenticación normal cuando la IP de la persona que llama pertenece al
  lista de permitidos configurada en `crates/iroha_torii/src/lib.rs:5879-5888`.
- `apply_extra_headers(...)` también aceptó entradas `headers` arbitrarias de
  la persona que llama MCP, por lo que se reservan encabezados de confianza internos como
  `x-iroha-remote-addr` e `x-forwarded-client-cert` no fueron explícitamente
  protegido.

Por qué esto es importante:- Las capas de puentes internas deben preservar el límite de confianza original. Reemplazo
  la persona que llama real con loopback trata efectivamente a cada persona que llama MCP como un
  cliente interno una vez que la solicitud cruza el puente.
- El error es sutil porque el perfil MCP visible externamente aún puede verse
  es de solo lectura, mientras que la ruta HTTP interna ve un origen más privilegiado.

Recomendación:

- Preservar la IP de la persona que llama que ya recibió la solicitud externa `/v1/mcp`
  desde el middleware de dirección remota de Torii y sintetizar `ConnectInfo` desde
  ese valor en lugar de bucle invertido.
- Trate los encabezados de confianza de solo ingreso, como `x-iroha-remote-addr` y
  `x-forwarded-client-cert` como encabezados internos reservados para que las personas que llaman MCP no puedan
  contrabandearlos o anularlos a través del argumento `headers`.

Estado de remediación:

- Cerrado en código actual. `crates/iroha_torii/src/mcp.rs` ahora deriva el
  IP remota enviada internamente desde la solicitud externa inyectada
  encabezado `x-iroha-remote-addr` y sintetiza `ConnectInfo` a partir de ese real
  IP de la persona que llama en lugar de loopback.
- `apply_extra_headers(...)` ahora elimina tanto `x-iroha-remote-addr` como
  `x-forwarded-client-cert` como encabezados internos reservados, por lo que las personas que llaman MCP
  No se puede falsificar la confianza de loopback/proxy de ingreso mediante argumentos de herramienta.
- La cobertura de regresión ahora incluye
  `dispatch_route_preserves_inbound_remote_addr_for_internal_allowlist_checks`
  `dispatch_route_blocks_remote_addr_spoofing_from_extra_headers`, y
  `apply_extra_headers_blocks_reserved_internal_headers` en
  `crates/iroha_torii/src/mcp.rs`.### SEC-11: Los clientes del SDK permitieron solicitar material confidencial a través de transportes inseguros o entre hosts (cerrado el 25 de marzo de 2026)

Impacto:

- Los clientes Swift, Java/Android, Kotlin y JS muestreados no
  Trate constantemente todas las formas de solicitud sensibles como sensibles al transporte.
  Dependiendo del ayudante, las personas que llaman podrían enviar encabezados de token de API/portador, sin formato
  `private_key*` Campos JSON o material de firma de autenticación canónica de la aplicación
  `http` / `ws` simple o mediante anulaciones de URL absolutas entre hosts.
- En el cliente JS específicamente, se agregaron encabezados `canonicalAuth` después
  `_request(...)` terminó sus controles de transporte y solo carrocería `private_key`
  JSON no se considera transporte sensible en absoluto.

Evidencia:- Swift ahora centraliza la guardia en
  `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift` y lo aplica desde
  `IrohaSwift/Sources/IrohaSwift/NoritoRpcClient.swift`,
  `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift`, y
  `IrohaSwift/Sources/IrohaSwift/ConnectClient.swift`; antes de este pase
  esos ayudantes no compartían una sola puerta de la política de transporte.
- Java/Android ahora centraliza la misma política en
  `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java`
  y lo aplica desde `NoritoRpcClient.java`, `ToriiRequestBuilder.java`,
  `OfflineToriiClient.java`, `SubscriptionToriiClient.java`,
  `stream/ToriiEventStreamClient.java`, y
  `websocket/ToriiWebSocketClient.java`.
- Kotlin ahora refleja esa política en
  `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt`
  y lo aplica desde el cliente JVM/request-builder/event-stream / correspondiente
  superficies de websocket.
- JS `ToriiClient._request(...)` ahora trata `canonicalAuth` más cuerpos JSON
  que contiene campos `private_key*` como material de transporte sensible en
  `javascript/iroha_js/src/toriiClient.js` y la forma del evento de telemetría en
  `javascript/iroha_js/index.d.ts` ahora registra `hasSensitiveBody` /
  `hasCanonicalAuth` cuando se utiliza `allowInsecure`.

Por qué esto es importante:

- Los asistentes móviles, de navegador y de desarrollo local a menudo apuntan a desarrollo/ensayo mutable
  URL base. Si el cliente no ancla solicitudes confidenciales al configurado
  esquema/host, una conveniente anulación de URL absoluta o una base HTTP simple puede
  convierta el SDK en una ruta de degradación de solicitud firmada o exfiltración secreta.
- El riesgo es más amplio que los tokens al portador. JSON `private_key` sin procesar y fresco
  Las firmas de autenticación canónica también son sensibles a la seguridad en el cable y
  no deberíamos pasar por alto silenciosamente la política de transportes.

Recomendación:- Centralizar la validación del transporte en cada SDK y aplicarla antes de la E/S de la red.
  para todas las formas de solicitudes sensibles: encabezados de autenticación, firma de autenticación canónica de aplicaciones,
  y JSON `private_key*` sin formato.
- Mantenga `allowInsecure` solo como una trampilla de escape local/de desarrollo explícita y emita
  telemetría cuando las personas que llaman optan por utilizarla.
- Agregue regresiones enfocadas en los creadores de solicitudes compartidas en lugar de solo en
  métodos de conveniencia de nivel superior, por lo que los futuros ayudantes heredan la misma guardia.

Estado de remediación:- Cerrado en código actual. Los ejemplos de Swift, Java/Android, Kotlin y JS
  Los clientes ahora rechazan transportes inseguros o entre hosts para los datos sensibles.
  solicitar formas anteriores a menos que la persona que llama opte por participar en el programa documentado solo para desarrolladores
  modo inseguro.
- Las regresiones enfocadas en Swift ahora cubren encabezados de autenticación Norito-RPC inseguros.
  Transporte inseguro de conexión websocket y solicitud raw-`private_key` Torii
  cuerpos.
- Las regresiones enfocadas en Kotlin ahora cubren encabezados de autenticación Norito-RPC inseguros.
  cuerpos `private_key` fuera de línea/suscripción, encabezados de autenticación SSE y websocket
  encabezados de autenticación.
- Las regresiones centradas en Java/Android ahora cubren la autenticación Norito-RPC insegura
  encabezados, cuerpos `private_key` fuera de línea/suscripción, encabezados de autenticación SSE y
  encabezados de autenticación websocket a través del arnés compartido de Gradle.
- Las regresiones centradas en JS ahora cubren las regresiones inseguras y entre hosts.
  Solicitudes de cuerpo `private_key` más solicitudes `canonicalAuth` inseguras en
  `javascript/iroha_js/test/transportSecurity.test.js`, mientras
  `javascript/iroha_js/test/toriiCanonicalAuth.test.js` ahora da positivo
  ruta de autenticación canónica en una URL base segura.

### SEC-12: El proxy QUIC local SoraFS aceptó enlaces sin bucle invertido sin autenticación de cliente (cerrado el 25 de marzo de 2026)

Impacto:- `LocalQuicProxyConfig.bind_addr` anteriormente se podía configurar en `0.0.0.0`, un
  IP de LAN, o cualquier otra dirección sin loopback, que expusiera la dirección "local"
  Proxy de estación de trabajo como escucha QUIC accesible de forma remota.
- Ese oyente no autenticó a los clientes. Cualquier par accesible que pueda
  completar la sesión QUIC/TLS y enviar un protocolo de enlace que coincida con la versión podría
  luego abra las secuencias `tcp`, `norito`, `car` o `kaigi`, según cuál
  Se configuraron los modos de puente.
- En el modo `bridge`, eso convirtió la mala configuración del operador en un TCP remoto
  superficie de retransmisión y transmisión de archivos locales en la estación de trabajo del operador.

Evidencia:

- `LocalQuicProxyConfig::parsed_bind_addr(...)` en
  `crates/sorafs_orchestrator/src/proxy.rs` anteriormente solo analizaba el socket
  dirección y no rechazó interfaces sin loopback.
- `spawn_local_quic_proxy(...)` en el mismo archivo inicia un servidor QUIC con un
  certificado autofirmado e `.with_no_client_auth()`.
- `handle_connection(...)` aceptó cualquier cliente cuyo `ProxyHandshakeV1`
  La versión coincidió con la versión única del protocolo admitido y luego ingresó el
  bucle de flujo de aplicaciones.
- `handle_tcp_stream(...)` marca valores arbitrarios de `authority` a través de
  `TcpStream::connect(...)`, mientras que `handle_norito_stream(...)`,
  `handle_car_stream(...)` e `handle_kaigi_stream(...)` transmiten archivos locales
  desde directorios de spool/caché configurados.

Por qué esto es importante:- Un certificado autofirmado protege la identidad del servidor sólo si el cliente
  decide verificarlo. No autentica al cliente. Una vez que el apoderado
  era accesible fuera del bucle invertido, la ruta del protocolo de enlace era solo de versión
  admisión.
- La API y los documentos describen este asistente como un proxy de estación de trabajo local para
  integraciones de navegador/SDK, por lo que permitir direcciones de enlace accesibles de forma remota era
  una discrepancia en los límites de confianza, no un modo de servicio remoto previsto.

Recomendación:

- Fallo cerrado en cualquier `bind_addr` sin bucle invertido, por lo que el ayudante actual no puede ser
  expuesto más allá de la estación de trabajo local.
- Si la exposición remota al proxy alguna vez se convierte en un requisito del producto, introduzca
  autenticación explícita del cliente/admisión de capacidad primero en lugar de
  relajando la guardia vinculante.

Estado de remediación:

- Cerrado en código actual. `crates/sorafs_orchestrator/src/proxy.rs` ahora
  rechaza direcciones de enlace sin bucle invertido con `ProxyError::BindAddressNotLoopback`
  antes de que comience el oyente QUIC.
- La documentación del campo de configuración en
  `docs/source/sorafs/developer/orchestrator.md` y
  `docs/portal/docs/sorafs/orchestrator-config.md` ahora documenta
  `bind_addr` como solo bucle invertido.
- La cobertura de regresión ahora incluye
  `spawn_local_quic_proxy_rejects_non_loopback_bind_addr` y el existente
  prueba de puente local positiva
  `proxy::tests::tcp_stream_bridge_transfers_payload` en
  `crates/sorafs_orchestrator/src/proxy.rs`.

### SEC-13: TLS sobre TCP P2P saliente degradado silenciosamente a texto sin formato de forma predeterminada (cerrado el 25 de marzo de 2026)

Impacto:- Habilitar `network.tls_enabled=true` en realidad no aplicaba solo TLS
  transporte de salida a menos que los operadores también descubran y establezcan
  `tls_fallback_to_plain=false`.
- Por lo tanto, cualquier error en el protocolo de enlace TLS o tiempo de espera en la ruta de salida
  bajó el dial a TCP de texto sin formato de forma predeterminada, lo que eliminó el transporte
  Confidencialidad e integridad contra atacantes en el camino o mal comportamiento.
  cajas intermedias.
- El protocolo de enlace de la aplicación firmada aún autenticaba la identidad del par, por lo que
  se trataba de una rebaja de la política de transporte en lugar de una elusión de la suplantación de identidad entre pares.

Evidencia:

- `tls_fallback_to_plain` por defecto es `true` en
  `crates/iroha_config/src/parameters/user.rs`, por lo que el respaldo estaba activo
  a menos que los operadores lo anulen explícitamente en config.
- `Connecting::connect_tcp(...)` en `crates/iroha_p2p/src/peer.rs` intenta un
  Marcación TLS siempre que se configura `tls_enabled`, pero en caso de errores o tiempos de espera de TLS
  registra una advertencia y recurre a TCP de texto sin formato siempre que
  `tls_fallback_to_plain` está habilitado.
- La configuración de muestra orientada al operador en `crates/iroha_kagami/src/wizard.rs` y
  los documentos de transporte público P2P en `docs/source/p2p*.md` también se anuncian
  respaldo de texto sin formato como comportamiento predeterminado.

Por qué esto es importante:- Una vez que los operadores encienden TLS, la expectativa más segura es cerrar fallas: si TLS
  No se puede establecer una sesión, el dial debería fallar en lugar de hacerlo silenciosamente.
  perdiendo protecciones de transporte.
- Dejar la degradación activada de forma predeterminada hace que la implementación sea sensible a
  peculiaridades de la ruta de red, interferencia de proxy e interrupción activa del protocolo de enlace en un
  manera que es fácil pasarla por alto durante la implementación.

Recomendación:

- Mantenga la reserva de texto sin formato como un control de compatibilidad explícito, pero de forma predeterminada
  `false`, por lo que `network.tls_enabled=true` significa solo TLS a menos que el operador opte por
  en un comportamiento de degradación.

Estado de remediación:

- Cerrado en código actual. `crates/iroha_config/src/parameters/user.rs` ahora
  El valor predeterminado es `tls_fallback_to_plain` a `false`.
- La instantánea del dispositivo de configuración predeterminada, la configuración de muestra Kagami y la configuración predeterminada
  Los ayudantes de prueba P2P/Torii ahora reflejan ese tiempo de ejecución reforzado predeterminado.
- Los documentos `docs/source/p2p*.md` duplicados ahora describen el respaldo de texto sin formato como
  una suscripción explícita en lugar del valor predeterminado enviado.

### SEC-14: La búsqueda geográfica de telemetría entre pares recurrió silenciosamente a HTTP de terceros en texto sin formato (cerrado el 25 de marzo de 2026)

Impacto:- Habilitar `torii.peer_geo.enabled=true` sin un punto final explícito causado
  Torii para enviar nombres de host de pares a un texto sin formato integrado
  Servicio `http://ip-api.com/json/...`.
- Que filtró objetivos de telemetría de pares en un HTTP de terceros no autenticado
  dependencia y permitir que cualquier atacante en ruta o feed de punto final comprometido falsifique
  metadatos de ubicación nuevamente en Torii.
- La función estaba habilitada, pero los documentos de telemetría públicos y la configuración de muestra
  anunció el valor predeterminado incorporado, lo que hizo que el patrón de implementación fuera inseguro
  probablemente una vez que los operadores habilitaron las búsquedas geográficas entre pares.

Evidencia:

- `crates/iroha_torii/src/telemetry/peers/monitor.rs` previamente definido
  `DEFAULT_GEO_ENDPOINT = "http://ip-api.com/json"` y
  `construct_geo_query(...)` usó ese valor predeterminado siempre que
  `GeoLookupConfig.endpoint` era `None`.
- El monitor de telemetría de pares genera `collect_geo(...)` siempre que
  `geo_config.enabled` es verdadero en
  `crates/iroha_torii/src/telemetry/peers/monitor.rs`, por lo que el texto sin formato
  Se podía acceder al respaldo en el código de tiempo de ejecución enviado en lugar del código de solo prueba.
- La configuración predeterminada es `crates/iroha_config/src/parameters/defaults.rs` y
  `crates/iroha_config/src/parameters/user.rs` deja `endpoint` sin configurar y el
  documentos de telemetría duplicados en `docs/source/telemetry*.md` plus
  `docs/source/references/peer.template.toml` documentó explícitamente la
  respaldo incorporado cuando los operadores habilitaron la función.

Por qué esto es importante:- La telemetría de pares no debe salir silenciosamente de los nombres de host de pares a través de HTTP en texto sin formato.
  a un servicio de terceros una vez que los operadores activan una bandera de conveniencia.
- Un valor predeterminado inseguro oculto también socava la revisión de cambios: los operadores pueden
  habilitar la búsqueda geográfica sin darse cuenta de que han introducido externos
  divulgación de metadatos de terceros y manejo de respuestas no autenticadas.

Recomendación:

- Eliminar el valor predeterminado del punto final geográfico incorporado.
- Requerir un punto final HTTPS explícito cuando las búsquedas geográficas de pares estén habilitadas y
  De lo contrario, omita las búsquedas.
- Mantenga regresiones enfocadas que demuestren que los puntos finales faltantes o que no son HTTPS fallan al cerrarse.

Estado de remediación:

- Cerrado en código actual. `crates/iroha_torii/src/telemetry/peers/monitor.rs`
  ahora rechaza los puntos finales faltantes con `MissingEndpoint`, rechaza los que no son HTTPS
  puntos finales con `InsecureEndpoint` y omite la búsqueda geográfica entre pares en lugar de
  recurriendo silenciosamente a un servicio integrado de texto sin formato.
- `crates/iroha_config/src/parameters/user.rs` ya no inyecta un implícito
  punto final en el momento del análisis, por lo que el estado de configuración no establecido permanece explícito durante todo el proceso.
  camino a la validación en tiempo de ejecución.
- Los documentos de telemetría duplicados y la configuración de muestra canónica en
  `docs/source/references/peer.template.toml` ahora indica que
  `torii.peer_geo.endpoint` debe configurarse explícitamente con HTTPS cuando el
  La función está habilitada.
- La cobertura de regresión ahora incluye
  `construct_geo_query_requires_explicit_endpoint`,
  `construct_geo_query_rejects_non_https_endpoint`,
  `collect_geo_requires_explicit_endpoint_when_enabled`, y
  `collect_geo_rejects_non_https_endpoint` en
  `crates/iroha_torii/src/telemetry/peers/monitor.rs`.### SEC-15: La política de pin y puerta de enlace SoraFS carecía de resolución de IP de cliente compatible con proxy confiable (cerrado el 25 de marzo de 2026)

Impacto:

- Las implementaciones de Torii con proxy inverso podrían anteriormente colapsar distintos SoraFS
  llamantes a la dirección del socket proxy al evaluar el CIDR de pin de almacenamiento
  listas permitidas, limitaciones de pin de almacenamiento por cliente y cliente de puerta de enlace
  huellas dactilares.
- Eso debilitó los controles de abuso en `/v1/sorafs/storage/pin` y SoraFS.
  La descarga de puerta de enlace aparece en topologías comunes de proxy inverso al hacer
  varios clientes comparten un depósito o una identidad de lista permitida.
- El enrutador predeterminado aún inyecta metadatos remotos internos, por lo que esto no fue un
  nueva omisión de ingreso no autenticada, pero era una brecha real en el límite de confianza
  para implementaciones con reconocimiento de proxy y un exceso de confianza en el límite del controlador del
  encabezado de IP remoto interno.

Evidencia:- `crates/iroha_config/src/parameters/user.rs` y
  `crates/iroha_config/src/parameters/actual.rs` anteriormente no tenía ningún general
  Perilla `torii.transport.trusted_proxy_cidrs`, por lo que cliente canónico con reconocimiento de proxy
  La resolución de IP no se podía configurar en el límite de ingreso general Torii.
- `inject_remote_addr_header(...)` en `crates/iroha_torii/src/lib.rs`
  previamente sobrescribió el encabezado interno `x-iroha-remote-addr` de
  `ConnectInfo` solo, lo que eliminó metadatos de IP de cliente reenviados confiables desde
  proxies inversos reales.
- `PinSubmissionPolicy::enforce(...)` en
  `crates/iroha_torii/src/sorafs/pin.rs` y
  `gateway_client_fingerprint(...)` en `crates/iroha_torii/src/sorafs/api.rs`
  no compartió un paso de resolución de IP canónica con reconocimiento de proxy confiable en el
  límite del controlador.
- Aceleración mediante pasador de almacenamiento en `crates/iroha_torii/src/sorafs/pin.rs` también codificada
  solo en el token al portador cada vez que había un token presente, lo que significaba múltiples
  los clientes proxy que compartían un token pin válido se vieron obligados a utilizar la misma tarifa
  bucket incluso después de que se distinguieran las IP de sus clientes.

Por qué esto es importante:

- Los servidores proxy inversos son un patrón de implementación normal para Torii. Si el tiempo de ejecución
  no puede distinguir consistentemente un proxy confiable de una persona que llama que no es confiable, IP
  Las listas de permitidos y las limitaciones por cliente dejan de significar lo que los operadores creen que
  decir.
- Las rutas de pin y puerta de enlace SoraFS son superficies explícitamente sensibles al abuso, por lo que
  colapsar a las personas que llaman en la IP proxy o confiar excesivamente en reenvíos obsoletos
  Los metadatos son operativamente significativos incluso cuando la ruta base aún
  requiere otra admisión.

Recomendación:- Agregue una superficie de configuración general Torii `trusted_proxy_cidrs` y resuelva el problema.
  IP de cliente canónico una vez desde `ConnectInfo` más cualquier reenvío preexistente
  encabezado solo cuando el par del socket está en esa lista de permitidos.
- Reutilizar esa resolución IP canónica dentro de las rutas del controlador SoraFS en lugar de
  confiar ciegamente en el encabezado interno.
- Limitaciones de pin de almacenamiento de tokens compartidos por token más IP de cliente canónico
  cuando ambos están presentes.

Estado de remediación:

- Cerrado en código actual. `crates/iroha_config/src/parameters/defaults.rs`,
  `crates/iroha_config/src/parameters/user.rs`, y
  `crates/iroha_config/src/parameters/actual.rs` ahora expone
  `torii.transport.trusted_proxy_cidrs`, por defecto a una lista vacía.
- `crates/iroha_torii/src/lib.rs` ahora resuelve la IP del cliente canónico con
  `limits::ingress_remote_ip(...)` dentro del middleware de entrada y reescritura
  el encabezado interno `x-iroha-remote-addr` solo desde servidores proxy confiables.
- `crates/iroha_torii/src/sorafs/pin.rs` y
  `crates/iroha_torii/src/sorafs/api.rs` ahora resuelve las IP de clientes canónicos
  contra `state.trusted_proxy_nets` en el límite del controlador para el pin de almacenamiento
  huella digital del cliente de política y puerta de enlace, por lo que las rutas directas del controlador no pueden
  confiar excesivamente en metadatos de IP reenviados obsoletos.
- La limitación del pin de almacenamiento ahora codifica los tokens de portador compartidos por `token + canonical
  IP del cliente` cuando ambos están presentes, preservando los depósitos por cliente para compartir
  fichas de pin.
- La cobertura de regresión ahora incluye
  `limits::tests::ingress_remote_ip_preserves_trusted_forwarded_header`,
  `limits::tests::ingress_remote_ip_ignores_forwarded_header_from_untrusted_peer`,
  `sorafs::pin::tests::rate_key_scopes_shared_tokens_by_ip`,
  `storage_pin_rate_limit_uses_forwarded_client_ip_from_trusted_proxy`,
  `car_range_rate_limit_uses_forwarded_client_ip_from_trusted_proxy`, y el
  accesorio de configuración
  `torii_transport_trusted_proxy_cidrs_default_to_empty`.### SEC-16: El bloqueo de autenticación del operador y la codificación de límite de velocidad volvieron a un depósito anónimo compartido cuando faltaba el encabezado de IP remoto inyectado (Cerrado el 25 de marzo de 2026)

Impacto:

- La admisión de autenticación del operador ya recibió la IP del socket aceptada, pero el
  La clave de bloqueo/límite de velocidad lo ignoró y colapsó las solicitudes en un espacio compartido.
  depósito `"anon"` siempre que el encabezado interno `x-iroha-remote-addr` fuera
  ausente.
- Esa no fue una nueva omisión de entrada pública en el enrutador predeterminado, porque
  El middleware de ingreso reescribe el encabezado interno antes de que se ejecuten estos controladores.
  Seguía siendo una verdadera brecha en los límites de confianza abierta para
  rutas del controlador, pruebas directas y cualquier ruta futura que llegue
  `OperatorAuth` antes del middleware de inyección.
- En esos casos, una persona que llama podría consumir el presupuesto de límite de tarifa o bloquear a otros
  llamantes que deberían haber sido aislados por IP de origen.

Evidencia:- `OperatorAuth::check_common(...)` en
  `crates/iroha_torii/src/operator_auth.rs` ya recibido
  `remote_ip: Option<IpAddr>`, pero anteriormente se llamaba `auth_key(headers)`
  y eliminó la IP de transporte por completo.
- `auth_key(...)` en `crates/iroha_torii/src/operator_auth.rs` anteriormente
  analizó solo `limits::REMOTE_ADDR_HEADER` y de lo contrario devolvió `"anon"`.
- El ayudante general Torii en `crates/iroha_torii/src/limits.rs` ya tenía
  la regla correcta de resolución de errores cerrados en ` Effective_remote_ip (headers,
  remoto)`, prefiriendo el encabezado canónico inyectado pero recurriendo al
  IP de socket aceptada cuando las invocaciones directas del controlador omiten el middleware.

Por qué esto es importante:

- El estado de bloqueo y límite de velocidad debe coincidir con la misma identidad efectiva de la persona que llama
  que el resto de Torii utiliza para decisiones políticas. Recurrir a una situación compartida
  Un depósito anónimo convierte un salto de metadatos internos faltante en un cliente cruzado
  interferencia en lugar de localizar el efecto en la verdadera persona que llama.
- La autenticación del operador es un límite sensible al abuso, por lo que incluso un límite de gravedad media
  Vale la pena cerrar explícitamente el problema de la colisión de cubos.

Recomendación:

- Derive la clave de autenticación del operador de `limits:: Effective_remote_ip(headers,
  remote_ip)` por lo que el encabezado inyectado aún gana cuando está presente, pero es directo
  las invocaciones del controlador recurren a la dirección de transporte en lugar de `"anon"`.
- Mantenga `"anon"` solo como respaldo final cuando tanto el encabezado interno como el
  la IP de transporte no está disponible.

Estado de remediación:- Cerrado en código actual. `crates/iroha_torii/src/operator_auth.rs` ahora llama
  `auth_key(headers, remote_ip)` de `check_common(...)` y `auth_key(...)`
  ahora deriva la clave de bloqueo/límite de velocidad de
  `limits::effective_remote_ip(headers, remote_ip)`.
- La cobertura de regresión ahora incluye
  `operator_auth_key_uses_remote_ip_when_internal_header_missing` y
  `operator_auth_key_prefers_injected_header_over_transport_remote_ip` en
  `crates/iroha_torii/src/operator_auth.rs`.

## Hallazgos cerrados o reemplazados del informe anterior

- Hallazgo anterior de Soracloud con clave privada sin formato: cerrado. Entrada de mutación actual
  rechaza los campos en línea `authority` / `private_key` en
  `crates/iroha_torii/src/soracloud.rs:5305-5308`, vincula el firmante HTTP al
  procedencia de la mutación en `crates/iroha_torii/src/soracloud.rs:5310-5315`, y
  devuelve borradores de instrucciones de transacción en lugar de enviar al servidor un documento firmado
  transacción en `crates/iroha_torii/src/soracloud.rs:5556-5565`.
- Hallazgo anterior de ejecución de proxy de lectura local solo interna: cerrado. Público
  La resolución de ruta ahora omite los controladores no públicos y de actualización/actualización privada en
  `crates/iroha_torii/src/soracloud.rs:8445-8463` y el tiempo de ejecución rechaza
  rutas de lectura local no públicas en
  `crates/irohad/src/soracloud_runtime.rs:5906-5923`.
- Hallazgo de respaldo no medido en tiempo de ejecución público anterior: cerrado tal como está escrito. Público
  El ingreso en tiempo de ejecución ahora impone límites de velocidad y topes a bordo en
  `crates/iroha_torii/src/lib.rs:8837-8852` antes de resolver una ruta pública en
  `crates/iroha_torii/src/lib.rs:8858-8860`.
- Hallazgo anterior de arrendamiento de adjuntos de IP remota: cerrado. Arrendamiento de archivos adjuntos ahora
  requiere una cuenta firmada verificada en
  `crates/iroha_torii/src/lib.rs:7962-7968`.
  El arrendamiento adjunto heredó previamente SEC-05 y SEC-06; esa herencia
  está cerrado por las soluciones actuales de autenticación de aplicaciones anteriores.## Hallazgos de dependencia- `cargo deny check advisories bans sources --hide-inclusion-graph` ahora se ejecuta
  directamente contra el `deny.toml` rastreado y ahora informa tres en vivo
  hallazgos de dependencia de un archivo de bloqueo del espacio de trabajo generado.
- Los avisos `tar` ya no están presentes en el gráfico de dependencia activa:
  `xtask/src/mochi.rs` ahora usa `Command::new("tar")` con un argumento fijo
  vector, y `iroha_crypto` ya no extrae `libsodium-sys-stable` para
  Pruebas de interoperabilidad Ed25519 después de cambiar esas comprobaciones a OpenSSL.
- Hallazgos actuales:
  - `RUSTSEC-2024-0388`: `derivative` no tiene mantenimiento.
  - `RUSTSEC-2024-0436`: `paste` no tiene mantenimiento.
- Triaje de impacto:
  - Los avisos `tar` informados anteriormente están cerrados para los activos
    gráfico de dependencia. `cargo tree -p xtask -e normal -i tar`,
    `cargo tree -p iroha_crypto -e all -i tar`, y
    `cargo tree -p iroha_crypto -e all -i libsodium-sys-stable` ahora todos fallan
    con "la especificación de ID del paquete... no coincide con ningún paquete", y
    `cargo deny` ya no informa `RUSTSEC-2026-0067` o
    `RUSTSEC-2026-0068`.
  - Los avisos de reemplazo directo de PQ informados anteriormente ahora están cerrados en
    el árbol actual. `crates/soranet_pq/Cargo.toml`,
    `crates/iroha_crypto/Cargo.toml` e `crates/ivm/Cargo.toml` ahora dependen
    en `pqcrypto-mldsa` / `pqcrypto-mlkem`, y el ML-DSA / ML-KEM tocado
    Las pruebas de tiempo de ejecución aún pasan después de la migración.
  - El aviso `rustls-webpki` informado anteriormente ya no está activo en
    la resolución actual. El espacio de trabajo ahora fija `reqwest` / `rustls` allanzamientos de parches parcheados que mantienen `rustls-webpki` en `0.103.10`, que es
    fuera del rango de recomendación.
  - `derivative` e `paste` no se utilizan directamente en la fuente del espacio de trabajo.
    Entran transitivamente a través de la pila BLS/arkworks debajo
    `w3f-bls` y varias otras cajas ascendentes, por lo que eliminarlas requiere
    cambios ascendentes o de pila de dependencia en lugar de una limpieza macro local.
    El árbol actual ahora acepta esos dos avisos explícitamente en
    `deny.toml` con motivos registrados.

## Notas de cobertura- Servidor/tiempo de ejecución/config/redes: SEC-05, SEC-06, SEC-07, SEC-08, SEC-09,
  SEC-10, SEC-12, SEC-13, SEC-14, SEC-15 y SEC-16 se confirmaron durante
  la auditoría y ahora están cerrados en el árbol actual. Endurecimiento adicional en
  el árbol actual ahora también hace que falle la admisión de sesión/conexión websocket
  cerrado cuando falta el encabezado IP remoto inyectado interno, en lugar de
  estableciendo de forma predeterminada esa condición en bucle invertido.
- IVM/cripto/serialización: no se han confirmado hallazgos adicionales de esta auditoría
  rebanada. La evidencia positiva incluye la puesta a cero de material clave confidencial en
  `crates/iroha_crypto/src/confidential.rs:53-60` y Soranet PoW con reconocimiento de reproducción
  validación de boleto firmado en `crates/iroha_crypto/src/soranet/pow.rs:823-879`.
  El endurecimiento de seguimiento ahora también rechaza la salida del acelerador con formato incorrecto en dos
  Rutas Norito muestreadas: `crates/norito/src/lib.rs` valida JSON acelerado
  Las cintas de etapa 1 anteriores a `TapeWalker` eliminan las referencias y ahora también requieren
  ayudantes Metal/CUDA Stage-1 cargados dinámicamente para demostrar la paridad con el
  constructor de índice estructural escalar antes de la activación, y
  `crates/norito/src/core/gpu_zstd.rs` valida las longitudes de salida informadas por GPU
  antes de truncar los buffers de codificación/decodificación. `crates/norito/src/core/simd_crc64.rs`
  ahora también autoprueba los ayudantes GPU CRC64 cargados dinámicamente contra el
  respaldo canónico antes de que `hardware_crc64` confíe en ellos, por lo que está mal formado
  las bibliotecas auxiliares fallan al cerrarse en lugar de cambiar silenciosamente la suma de comprobación Noritocomportamiento. Los resultados de ayuda no válidos ahora retroceden en lugar de liberarse en pánico
  acumulaciones o paridad de suma de comprobación a la deriva. En el lado IVM, acelerador muestreado
  Las puertas de inicio ahora también cubren CUDA Ed25519 `signature_kernel`, CUDA BN254
  add/sub/mul kernels, CUDA `sha256_leaves` / `sha256_pairs_reduce`, el live
  Kernels por lotes CUDA vector/AES (`vadd64`, `vand`, `vxor`, `vor`,
  `aesenc_batch`, `aesdec_batch`) y el Metal correspondiente
  `sha256_leaves`/vector/AES kernels por lotes antes de que esas rutas sean confiables. el
  La ruta de la firma Metal Ed25519 muestreada ahora también está de vuelta
  dentro del acelerador en vivo configurado en este host: la falla de paridad anterior fue
  arreglado restaurando la normalización ligada a extremidades ref10 a través de la escalera escalar,
  y la regresión de Metal enfocada ahora verifica `[s]B`, `[h](-A)`, el
  escalera de potencia de dos puntos base y verificación por lotes completa `[true, false]`
  en Metal contra la ruta de referencia de la CPU. La fuente CUDA reflejada cambia
  Compile bajo `--features cuda --tests` y la verdad de inicio de CUDA se establece ahora
  falla al cerrarse si los núcleos de hoja/par de Merkle en vivo se alejan de la CPU
  camino de referencia. La validación de CUDA en tiempo de ejecución sigue estando limitada al host en este
  ambiente.
- SDK/ejemplos: SEC-11 se confirmó durante la muestra centrada en el transporte
  pasar a través de los clientes Swift, Java/Android, Kotlin y JS, y esoEl hallazgo ahora está cerrado en el árbol actual. JS, Swift y Android
  Los asistentes de solicitud canónica también se han actualizado a la nueva versión.
  esquema de cuatro encabezados consciente de la frescura.
  La revisión del transporte de streaming QUIC de muestra tampoco produjo una transmisión en vivo.
  búsqueda de tiempo de ejecución en el árbol actual: `StreamingClient::connect(...)`,
  `StreamingServer::bind(...)`, y los ayudantes de negociación de capacidad son
  actualmente ejercido solo desde el código de prueba en `crates/iroha_p2p` y
  `crates/iroha_core`, por lo que el verificador autofirmado permisivo en ese asistente
  Actualmente, la ruta es solo de prueba/ayudante en lugar de una superficie de ingreso enviada.
- Los ejemplos y las aplicaciones de muestra móviles se revisaron solo en un nivel de verificación puntual y
  no deben considerarse como auditados exhaustivamente.

## Brechas de validación y cobertura- `cargo deny check advisories bans sources --hide-inclusion-graph` ahora se ejecuta
  directamente con el `deny.toml` rastreado. Bajo esa ejecución del esquema actual,
  `bans` e `sources` están limpios, mientras que `advisories` falla con los cinco
  hallazgos de dependencia enumerados anteriormente.
- Se aprobó la validación de limpieza del gráfico de dependencia para los hallazgos cerrados `tar`:
  `cargo tree -p xtask -e normal -i tar`,
  `cargo tree -p iroha_crypto -e all -i tar`, y
  `cargo tree -p iroha_crypto -e all -i libsodium-sys-stable` ahora todos reportan
  "La especificación de ID del paquete... no coincide con ningún paquete", mientras que
  `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 cargo test -p xtask create_archive_packages_bundle_directory -- --nocapture`,
  `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 cargo check -p xtask`,
  `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 cargo test -p iroha_crypto ed25519_verify -- --nocapture`,
  y
  `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 cargo test -p iroha_crypto ed25519_sign -- --nocapture`
  todo pasó.
- `bash scripts/fuzz_smoke.sh` ahora ejecuta sesiones reales de libFuzzer a través de
  `cargo +nightly fuzz`, pero la mitad IVM del script no terminó dentro
  este paso porque la primera compilación nocturna para `tlv_validate` todavía estaba en
  progreso en el traspaso. Desde entonces, esa construcción se ha completado lo suficiente como para ejecutar el
  binario libFuzzer generado directamente:
  `cd crates/ivm/fuzz && ./target/aarch64-apple-darwin/release/tlv_validate -runs=200 -rss_limit_mb=3072 -max_total_time=15`
  ahora llega al bucle de ejecución libFuzzer y sale limpiamente después de 200 ejecuciones desde un
  corpus vacío. El Norito se completó a medias con éxito después de arreglar el problema.
  deriva de arnés/manifiesto y la compilación `json_from_json_equiv` fuzz-target
  romper.
- La validación de corrección Torii ahora incluye:
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo check -p iroha_torii --lib`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib --no-run`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib verify_accepts_valid_signature -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib verify_rejects_replayed_nonce -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib operator_auth_rejects_forwarded_mtls_from_untrusted_proxy -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_HOME=/tmp/iroha-cargo-home-operator-auth-key CARGO_TARGET_DIR=/tmp/iroha-codex-target-operator-auth-key cargo test -p iroha_torii --lib operator_auth_key_ -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_HOME=/tmp/iroha-cargo-home-operator-auth-key CARGO_TARGET_DIR=/tmp/iroha-codex-target-operator-auth-key cargo test -p iroha_torii --lib operator_auth_rejects_forwarded_mtls_from_untrusted_proxy -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib trusted_forwarded_header_requires_proxy_membership -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-webhook-https cargo check -p iroha_torii --lib --features app_api_https`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-webhook-https cargo test -p iroha_torii --lib https_delivery_dns_override_ --features app_api_https -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-webhook2 cargo test -p iroha_torii --lib websocket_pinned_connect_addr_pins_secure_delivery_when_guarded -- --nocapture`- `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-mcp cargo test -p iroha_torii --lib dispatch_route_preserves_inbound_remote_addr_for_internal_allowlist_checks -- --nocapture`
  -`NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-mcp cargo test -p iroha_torii --lib dispatch_route_blocks_remote_addr_spoofing_from_extra_headers -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-mcp cargo test -p iroha_torii --lib apply_extra_headers_blocks_reserved_internal_headers -- --nocapture`
  - el más estrecho `--no-default-features --features app_api,app_api_https` Torii
    La matriz de prueba todavía tiene fallas de compilación existentes no relacionadas en DA/
    Código de prueba de biblioteca controlado por Soracloud, por lo que este pase validó el envío
    ruta MCP de función predeterminada y la ruta del webhook `app_api_https` en lugar de
    reclamando cobertura completa de características mínimas.
- La validación de corrección de Trusted-proxy/SoraFS ahora incluye:
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib limits::tests:: -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib sorafs::pin::tests:: -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib storage_pin_requires_token_and_respects_allowlist_and_rate_limit -- --nocapture`
  -`CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib storage_pin_rate_limit_uses_forwarded_client_ip_from_trusted_proxy -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib car_range_rate_limits_repeated_clients -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib car_range_rate_limit_uses_forwarded_client_ip_from_trusted_proxy -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_config --test fixtures torii_transport_trusted_proxy_cidrs_default_to_empty -- --nocapture`
- La validación de corrección P2P ahora incluye:
  -`NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-p2p cargo test -p iroha_p2p --lib --no-run`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-p2p cargo test -p iroha_p2p --lib outgoing_handshake_rejects_unexpected_peer_identity -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-p2p cargo test -p iroha_p2p --lib handshake_v1_defaults_to_trust_gossip -- --nocapture`
- La validación de corrección del proxy local SoraFS ahora incluye:
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-proxy cargo test -p sorafs_orchestrator spawn_local_quic_proxy_rejects_non_loopback_bind_addr -- --nocapture`
  - `/tmp/iroha-codex-target-proxy/debug/deps/sorafs_orchestrator-b3be10a343598c7b --exact proxy::tests::tcp_stream_bridge_transfers_payload --nocapture`
- La validación de corrección predeterminada de P2P TLS ahora incluye:
  - `CARGO_HOME=/tmp/iroha-cargo-home-sec13 CARGO_TARGET_DIR=/tmp/iroha-codex-target-config-tls2 cargo test -p iroha_config tls_fallback_defaults_to_tls_only -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-sec13 CARGO_TARGET_DIR=/tmp/iroha-codex-target-p2p-sec13 cargo test -p iroha_p2p --lib start_rejects_tls_without_feature_when_tls_only_outbound -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-sec13 CARGO_TARGET_DIR=/tmp/iroha-codex-target-p2p-sec13 cargo test -p iroha_p2p --lib tls_only_dial_requires_p2p_tls_feature_when_no_fallback -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_HOME=/tmp/iroha-cargo-home-sec13 CARGO_TARGET_DIR=/tmp/iroha-codex-target-torii-connect-gating cargo test -p iroha_torii --test connect_gating --no-run`
- La validación de corrección del lado del SDK ahora incluye:
  - `node --test javascript/iroha_js/test/canonicalRequest.test.js javascript/iroha_js/test/toriiCanonicalAuth.test.js`
  - `cd IrohaSwift && swift test --filter CanonicalRequestTests`
  - `cd IrohaSwift && swift test --filter 'NoritoRpcClientTests/testCallRejectsInsecureAuthorizationHeader'`
  -`cd IrohaSwift && swift test --filter 'ConnectClientTests/testBuildsConnectWebSocketRequestRejectsInsecureTransport'`
  - `cd IrohaSwift && swift test --filter 'ToriiClientTests/testCreateSubscriptionPlanRejectsInsecureTransportForPrivateKeyBody'`
  - `cd kotlin && ./gradlew :core-jvm:test --tests org.hyperledger.iroha.sdk.client.TransportSecurityClientTest --console=plain`
  - `cd java/iroha_android && JAVA_HOME=$(/usr/libexec/java_home -v 21) ANDROID_HOME=~/Library/Android/sdk ANDROID_SDK_ROOT=~/Library/Android/sdk ./gradlew android:compileDebugUnitTestJavaWithJavac --console=plain`
  - `cd java/iroha_android && JAVA_HOME=$(/usr/libexec/java_home -v 21) ANDROID_HOME=~/Library/Android/sdk ANDROID_SDK_ROOT=~/Library/Android/sdk ANDROID_HARNESS_MAINS=org.hyperledger.iroha.android.client.NoritoRpcClientTests,org.hyperledger.iroha.android.client.OfflineToriiClientTests,org.hyperledger.iroha.android.client.SubscriptionToriiClientTests,org.hyperledger.iroha.android.client.stream.ToriiEventStreamClientTests,org.hyperledger.iroha.android.client.websocket.ToriiWebSocketClientTests ./gradlew :core:test --tests org.hyperledger.iroha.android.GradleHarnessTests --console=plain`
  - `node --test javascript/iroha_js/test/transportSecurity.test.js javascript/iroha_js/test/toriiCanonicalAuth.test.js`
  - `node --test javascript/iroha_js/test/toriiSubscriptions.test.js`
- La validación de seguimiento Norito ahora incluye:
  - `python3 scripts/check_norito_bindings_sync.py`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p norito validate_accel_rejects_out_of_bounds_offsets -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p norito validate_accel_rejects_non_structural_offsets -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p norito try_gpu_encode_rejects_invalid_success_length -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p norito try_gpu_decode_rejects_invalid_success_length -- --nocapture`
  - `bash scripts/fuzz_smoke.sh` (Norito apunta a `json_parse_string`,
    `json_parse_string_ref`, `json_skip_value` y `json_from_json_equiv`pasado después de las correcciones de arnés/objetivo)
- La validación de seguimiento difuso IVM ahora incluye:
  - `cd crates/ivm/fuzz && ./target/aarch64-apple-darwin/release/tlv_validate -runs=200 -rss_limit_mb=3072 -max_total_time=15`
- La validación de seguimiento del acelerador IVM ahora incluye:
  - `xcrun -sdk macosx metal -c crates/ivm/src/metal_ed25519.metal -o /tmp/metal_ed25519.air`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-cuda2 cargo check -p ivm --features cuda --tests`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-cuda-check cargo check -p ivm --features cuda --tests`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo check -p ivm --features metal --tests`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo test -p ivm --features metal --lib metal_bitwise_single_vector_matches_scalar -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo test -p ivm --features metal --lib metal_aes_batch_matches_scalar -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo test -p ivm --features metal --lib metal_ed25519_batch_matches_cpu -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo test -p ivm --features metal --lib metal_sha256_leaves_matches_cpu -- --nocapture`
- La ejecución de pruebas de biblioteca CUDA enfocadas sigue estando limitada por el entorno en este host:
  `CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-cuda2 cargo test -p ivm --features cuda --lib selftest_covers_ -- --nocapture`
  todavía no se puede vincular porque los símbolos del controlador CUDA (`cu*`) no están disponibles.
- La validación del tiempo de ejecución de Focused Metal ahora se ejecuta completamente en el acelerador en este
  host: la canalización de firmas Ed25519 de muestra permanece habilitada durante el inicio
  autopruebas, y `metal_ed25519_batch_matches_cpu` verifica `[true, false]`
  directamente en Metal contra la ruta de referencia de la CPU.
- No volví a ejecutar un barrido de prueba de Rust en el espacio de trabajo completo, `npm test` completo o el
  Suites Swift/Android completas durante este paso de remediación.

## Trabajo pendiente de remediación priorizada

### Próximo tramo- Supervisar los reemplazos ascendentes de la transitiva explícitamente aceptada.
  `derivative` / `paste` macro deuda y eliminar las excepciones `deny.toml` cuando
  actualizaciones seguras están disponibles sin desestabilizar el BLS / Halo2 / PQ /
  Pilas de dependencia de la interfaz de usuario.
- Vuelva a ejecutar el script fuzz-smoke IVM nocturno completo en un caché cálido para
  `tlv_validate` / `kotodama_lower` tienen resultados registrados estables junto al
  objetivos Norito ahora verdes. Ahora se completa una ejecución binaria directa `tlv_validate`.
  pero el humo nocturno con guión completo sigue siendo excepcional.
- Vuelva a ejecutar el segmento de autoprueba CUDA lib-test enfocado en un host con el controlador CUDA
  bibliotecas instaladas, por lo que se valida el conjunto de verdad de inicio CUDA ampliado
  más allá de `cargo check` y la corrección de normalización Ed25519 reflejada más la
  Se ejecutan nuevas sondas de inicio de vector/AES en tiempo de ejecución.
- Vuelva a ejecutar suites JS/Swift/Android/Kotlin más amplias una vez que alcance el nivel de suite no relacionada
  Los bloqueadores en esta rama se borran, por lo que la nueva solicitud canónica y
  Los guardias de seguridad del transporte están cubiertos más allá de las pruebas de ayuda específicas mencionadas anteriormente.
- Decidir si la historia multifirma de autenticación de aplicaciones a largo plazo debe permanecer
  cerrar fallas o desarrollar un formato de testigo multifirma HTTP de primera clase.

### Monitor- Continuar la revisión centrada en la aceleración de hardware/rutas inseguras `ivm`
  y los límites restantes de transmisión/cripto `norito`. El JSON Etapa-1
  y las transferencias de ayuda de GPU zstd ahora se han endurecido para fallar cerradas
  versiones de lanzamiento y los conjuntos de verdad de inicio del acelerador IVM de muestra ahora están
  más amplio, pero la revisión más amplia del determinismo/inseguridad aún está abierta.
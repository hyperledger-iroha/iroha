---
lang: es
direction: ltr
source: docs/source/crypto/sm_program.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 08e2e1e4a54390d9142d6788aad2385e93282a33423b9fc7f3418e3633f3f86a
source_last_modified: "2026-01-23T18:50:10.586502+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! Resumen de la arquitectura de habilitación SM2/SM3/SM4 para Hyperledger Iroha v2.

# Resumen de arquitectura del programa SM

## Propósito
Defina el plan técnico, la postura de la cadena de suministro y los límites de riesgo para introducir la criptografía nacional china (SM2/SM3/SM4) en toda la pila Iroha v2, preservando al mismo tiempo la ejecución determinista y la auditabilidad.

## Alcance
- **Rutas críticas de consenso:** `iroha_crypto`, `iroha`, `irohad`, host IVM, intrínsecos Kotodama.
- **SDK de cliente y herramientas:** Rust CLI, Kagami, SDK de Python/JS/Swift, utilidades de génesis.
- **Configuración y serialización:** perillas `iroha_config`, etiquetas de modelo de datos Norito, manejo de manifiestos, actualizaciones multicódec.
- **Pruebas y cumplimiento:** Unidad/propiedad/suites de interoperabilidad, arneses Wycheproof, perfiles de rendimiento, orientación sobre exportación/regulación. *(Estado: pila SM respaldada por RustCrypto fusionada; paquete fuzz `sm_proptest` opcional y arnés de paridad OpenSSL disponibles para CI extendida.)*

Fuera de alcance: algoritmos PQ, aceleración de host no determinista en rutas de consenso; Las compilaciones wasm/`no_std` están retiradas.

## Entradas y entregables del algoritmo
| Artefacto | Propietario | Vencimiento | Notas |
|----------|-------|-----|-------|
| Diseño de funciones del algoritmo SM (`SM-P0`) | Grupo de Trabajo sobre Cripto | 2025-02 | Control de funciones, auditoría de dependencia, registro de riesgos. |
| Integración central de Rust (`SM-P1`) | Crypto WG / Modelo de datos | 2025-03 | Ayudantes de verificación/hash/AEAD basados ​​en RustCrypto, extensiones Norito, accesorios. |
| Firma + llamadas al sistema de VM (`SM-P2`) | Programa Núcleo/SDK IVM | 2025-04 | Envoltorios de firma deterministas, llamadas al sistema, cobertura Kotodama. |
| Habilitación opcional de proveedores y operaciones (`SM-P3`) | Grupo de Trabajo sobre Operaciones y Rendimiento de Plataforma | 2025-06 | Backend OpenSSL/Tongsuo, intrínsecos de ARM, telemetría, documentación. |

## Bibliotecas seleccionadas
- **Primario:** Cajas RustCrypto (`sm2`, `sm3`, `sm4`) con la función `rfc6979` habilitada y SM3 vinculado a nonces deterministas.
- **FFI opcional:** API del proveedor OpenSSL 3.x o Tongsuo para implementaciones que requieren pilas certificadas o motores de hardware; Función cerrada y deshabilitada de forma predeterminada en binarios de consenso.### Estado de integración de la biblioteca principal
- `iroha_crypto::sm` expone hash SM3, verificación SM2 y ayudantes SM4 GCM/CCM bajo la característica unificada `sm`, con rutas de firma RFC6979 deterministas disponibles para los SDK a través de `Sm2PrivateKey`.【crates/iroha_crypto/src/sm.rs:1049】【crates/iroha_crypto/src/sm.rs:1128】【crates/iroha_crypto/src/sm.rs:1236】
- Las etiquetas Norito/Norito-JSON y los asistentes multicódec cubren firmas/claves públicas SM2 y cargas útiles SM3/SM4 para que las instrucciones se serialicen de manera determinista. hosts.【crates/iroha_data_model/src/isi/registry.rs:407】【crates/iroha_data_model/tests/sm_norito_roundtrip.rs:12】
- Los conjuntos de respuestas conocidas validan la integración de RustCrypto (`sm3_sm4_vectors.rs`, `sm2_negative_vectors.rs`) y se ejecutan como parte de los trabajos de funciones `sm` de CI, manteniendo la verificación determinista mientras los nodos continúan firmando con Ed25519.【crates/iroha_crypto/tests/sm3_sm4_vectors.rs:15】【crates/iroha_crypto/tests/sm2_negative_vectors.rs:1】
- Validación de compilación de funciones opcional `sm`: `cargo check -p iroha_crypto --features sm --locked` (frío 7,9 s/cálido 0,23 s) e `cargo check -p iroha_crypto --no-default-features --features "std sm" --locked` (1,0 s) tienen éxito; Al habilitar la función se agregan 11 cajas (`base64ct`, `ghash`, `opaque-debug`, `pem-rfc7468`, `pkcs8`, `polyval`, `primeorder`, `sm2`, `sm3`, `sm4`, `sm4-gcm`). Hallazgos registrados en `docs/source/crypto/sm_rustcrypto_spike.md`.【docs/source/crypto/sm_rustcrypto_spike.md:1】
- Los dispositivos de verificación negativa de BouncyCastle/GmSSL se encuentran bajo `crates/iroha_crypto/tests/fixtures/sm/sm2_negative_vectors.json`, lo que garantiza que los casos de falla canónica (r=0, s=0, discrepancia de ID distintivo, clave pública manipulada) permanezcan alineados con los ampliamente implementados. proveedores.【crates/iroha_crypto/tests/sm2_negative_vectors.rs:1】【crates/iroha_crypto/tests/fixtures/sm/sm2_negative_vectors.json:1】
- `sm-ffi-openssl` ahora compila la cadena de herramientas OpenSSL 3.x suministrada (función `openssl` caja `vendored`), por lo que las versiones preliminares y las pruebas siempre apuntan a un proveedor moderno con capacidad SM incluso cuando el sistema LibreSSL/OpenSSL carece de algoritmos SM.【crates/iroha_crypto/Cargo.toml:59】
- `sm_accel` ahora detecta AArch64 NEON en tiempo de ejecución y enhebra los ganchos SM3/SM4 a través del envío x86_64/RISC-V mientras respeta la perilla de configuración `crypto.sm_intrinsics` (`auto`/`force-enable`/`force-disable`). Cuando los back-ends vectoriales están ausentes, el despachador aún enruta a través de la ruta escalar de RustCrypto para que los bancos y los cambios de políticas se comporten de manera consistente en todos los hosts. 【crates/iroha_crypto/src/sm.rs:702】【crates/iroha_crypto/src/sm.rs:733】

### Norito Superficies de esquema y modelo de datos| Norito tipo / consumidor | Representación | Restricciones y notas |
|--------------------------------|----------------|---------------------|
| `Sm3Digest` (`iroha_crypto::Sm3Digest`) | Simple: blob de 32 bytes · JSON: cadena hexadecimal en mayúsculas (`"4F4D..."`) | Envoltura de tupla canónica Norito `[u8; 32]`. La decodificación JSON/Bare rechaza longitudes ≠ 32. Viajes de ida y vuelta cubiertos por `sm_norito_roundtrip::sm3_digest_norito_roundtrip`. |
| `Sm2PublicKey` / `Sm2Signature` | Blobs con prefijo multicódec (`0x1306` provisional) | Las claves públicas codifican puntos SEC1 sin comprimir; las firmas son `(r∥s)` (32 bytes cada una) con protectores de análisis DER. |
| `Sm4Key` | Desnudo: blob de 16 bytes | Envoltura de puesta a cero expuesta a Kotodama/CLI. La serialización JSON se omite deliberadamente; los operadores deben pasar claves a través de blobs (contratos) o CLI hexadecimal (`--key-hex`). |
| Operandos `sm4_gcm_seal/open` | Tupla de 4 blobs: `(key, nonce, aad, payload)` | Clave = 16 bytes; nonce = 12 bytes; longitud de etiqueta fijada en 16 bytes. Devuelve `(ciphertext, tag)`; Kotodama/CLI emite ayudas tanto hexadecimales como base64.【crates/ivm/tests/sm_syscalls.rs:728】 |
| Operandos `sm4_ccm_seal/open` | Tupla de 4 blobs (clave, nonce, aad, carga útil) + longitud de etiqueta en `r14` | Nonce 7–13 bytes; longitud de la etiqueta ∈ {4,6,8,10,12,14,16}. La función `sm` expone el CCM detrás del indicador `sm-ccm`. |
| Kotodama intrínsecos (`sm::hash`, `sm::seal_gcm`, `sm::open_gcm`,…) | Mapa de SCALL arriba | La validación de entradas refleja las reglas del host; Los tamaños mal formados aumentan `ExecutionError::Type`. |

Referencia rápida de uso:
- **Hashing SM3 en contratos/pruebas:** `Sm3Digest::hash(b"...")` (Rust) o Kotodama `sm::hash(input_blob)`. JSON espera 64 caracteres hexadecimales.
- **SM4 AEAD a través de CLI:** `iroha tools crypto sm4 gcm-seal --key-hex <32 hex> --nonce-hex <24 hex> --plaintext-hex …` produce pares de etiquetas y texto cifrado hexadecimal/base64. Descifre con `gcm-open` coincidente.
- **Cadenas multicodec:** Las claves/firmas públicas SM2 se analizan desde/hacia la cadena multibase aceptada por `PublicKey::from_str`/`Signature::from_bytes`, lo que permite que los manifiestos Norito y los ID de cuenta lleven signatarios de SM.

Los consumidores de modelos de datos deben tratar las claves y etiquetas SM4 como blobs transitorios; nunca persista las claves sin procesar en la cadena. Los contratos deben almacenar solo texto cifrado/etiquetas de salida o resúmenes derivados (por ejemplo, SM3 de la clave) cuando se requiere una auditoría.

### Cadena de suministro y licencias
| Componente | Licencia | Mitigación |
|-----------|---------|-----------|
| `sm2`, `sm3`, `sm4` | Apache-2.0/MIT | Realice un seguimiento de las confirmaciones ascendentes, del proveedor si se requieren liberaciones sincronizadas, programe una auditoría de terceros antes de que el validador firme GA. |
| `rfc6979` | Apache-2.0/MIT | Ya utilizado en otros algoritmos; confirme la unión determinista de `k` con el resumen de SM3. |
| Opcional OpenSSL/Tongsuo | Apache-2.0 / estilo BSD | Mantener detrás la función `sm-ffi-openssl`, requerir la inclusión explícita del operador y la lista de verificación de empaquetado. |### Marcas de funciones y propiedad
| Superficie | Predeterminado | Mantenedor | Notas |
|---------|---------|------------|-------|
| `iroha_crypto/sm-core`, `sm-ccm`, `sm` | Apagado | Grupo de Trabajo sobre Cripto | Habilita las primitivas RustCrypto SM; `sm` incluye ayudas de CCM para clientes que requieren cifrado autenticado. |
| `ivm/sm` | Apagado | Equipo central IVM | Crea llamadas al sistema SM (`sm3_hash`, `sm2_verify`, `sm4_gcm_*`, `sm4_ccm_*`). La activación del host deriva de `crypto.allowed_signing` (presencia de `sm2`). |
| `iroha_crypto/sm_proptest` | Apagado | Grupo de trabajo de control de calidad/cripto | Arnés de prueba de propiedad que cubre firmas/etiquetas con formato incorrecto. Habilitado solo en CI extendida. |
| `crypto.allowed_signing` + `default_hash` | `["ed25519"]`, `blake2b-256` | Grupo de trabajo de configuración / Grupo de trabajo de operadores | La presencia del hash `sm2` más `sm3-256` habilita llamadas al sistema/firmas SM; Al eliminar `sm2` se vuelve al modo de solo verificación. |
| Opcional `sm-ffi-openssl` (vista previa) | Apagado | Operaciones de plataforma | Función de marcador de posición para la integración del proveedor OpenSSL/Tongsuo; permanece inhabilitado hasta que lleguen los SOP de certificación y embalaje. |

La política de red ahora expone `network.require_sm_handshake_match` y
`network.require_sm_openssl_preview_match` (ambos predeterminados son `true`). Borrar cualquiera de las banderas permite
implementaciones mixtas donde los observadores solo Ed25519 se conectan a validadores habilitados para SM; los desajustes son
registrado en `WARN`, pero los nodos de consenso deben mantener los valores predeterminados habilitados para evitar errores accidentales.
divergencia entre pares conscientes de SM y deshabilitados.
La CLI muestra estos cambios a través de la actualización del protocolo de enlace de la aplicación `iroha_cli sorafs
--allow-sm-handshake-mismatch` and `--allow-sm-openssl-preview-mismatch`, or the matching `--require-*`
banderas para restablecer una aplicación estricta.#### Vista previa de OpenSSL/Tongsuo (`sm-ffi-openssl`)
- **Alcance.** Crea un shim de proveedor solo de vista previa (`OpenSslProvider`) que valida la disponibilidad del tiempo de ejecución de OpenSSL y expone el hash SM3 respaldado por OpenSSL, la verificación SM2 y el cifrado/descifrado SM4-GCM sin dejar de estar habilitado. Los binarios de consenso deben seguir utilizando la ruta RustCrypto; El backend de FFI está estrictamente habilitado para pilotos de firma/verificación de borde.
- **Requisitos previos de compilación.** Compile con `cargo build -p iroha_crypto --features "sm sm-ffi-openssl"` y garantice los enlaces de la cadena de herramientas con OpenSSL/Tongsuo 3.0+ (`libcrypto` con soporte SM2/SM3/SM4). Se desaconsejan los enlaces estáticos; Prefiera bibliotecas dinámicas administradas por el operador.
- **Prueba de humo del desarrollador.** Ejecute `scripts/sm_openssl_smoke.sh` para ejecutar `cargo check -p iroha_crypto --features "sm sm-ffi-openssl"` seguido de `cargo test -p iroha_crypto --features "sm sm-ffi-openssl" --test sm_openssl_smoke -- --nocapture`; el asistente se salta automáticamente cuando los encabezados de desarrollo OpenSSL ≥3 no están disponibles (o falta `pkg-config`) y muestra la salida de humo para que los desarrolladores puedan ver si la verificación SM2 se ejecutó o volvió a la implementación de Rust.
- **Rust scaffolding.** El módulo `openssl_sm` ahora enruta el hash SM3, la verificación SM2 (ZA prehash + SM2 ECDSA) y el cifrado/descifrado SM4 GCM a través de OpenSSL con errores estructurados que cubren alternancias de vista previa y longitudes de clave/nonce/etiqueta no válidas; SM4 CCM permanece exclusivamente oxidado hasta que aterrizan cuñas FFI adicionales.
- **Comportamiento de omisión.** Cuando los encabezados o bibliotecas OpenSSL ≥3.0 están ausentes, la prueba de humo imprime un banner de omisión (a través de `-- --nocapture`) pero aún así sale exitosamente para que CI pueda distinguir las brechas ambientales de las regresiones genuinas.
- **Valores de seguridad en tiempo de ejecución.** La vista previa de OpenSSL está deshabilitada de forma predeterminada; habilítelo a través de la configuración (`crypto.enable_sm_openssl_preview` / `OpenSslProvider::set_preview_enabled(true)`) antes de intentar utilizar la ruta FFI. Mantenga los clústeres de producción en modo de solo verificación (omita `sm2` de `allowed_signing`) hasta que el proveedor se gradúe, confíe en el respaldo determinista de RustCrypto y limite los pilotos de firma a entornos aislados.
- **Lista de verificación de empaquetado.** Documente la versión del proveedor, la ruta de instalación y los hashes de integridad en los manifiestos de implementación. Los operadores deben proporcionar scripts de instalación que instalen la compilación OpenSSL/Tongsuo aprobada, registrarla en el almacén de confianza del sistema operativo (si es necesario) y fijar las actualizaciones detrás de las ventanas de mantenimiento.
- **Próximos pasos.** Los hitos futuros agregan enlaces deterministas SM4 CCM FFI, trabajos de ahumado de CI (consulte `ci/check_sm_openssl_stub.sh`) y telemetría. Realice un seguimiento del progreso en SM-P3.1.x en `roadmap.md`.

#### Resumen de propiedad del código
- **Crypto WG:** `iroha_crypto`, accesorios SM, documentación de cumplimiento.
- **IVM Core:** implementaciones de llamadas al sistema, intrínsecos de Kotodama, activación de host.
- **Config WG:** קונפיגורציית `crypto.allowed_signing`/`default_hash`, ולידציית מניפסט, חיווט קבלה.
- **Programa SDK:** Herramientas compatibles con SM en CLI/Kagami/SDK, dispositivos compartidos.
- **Platform Ops & Performance WG:** ganchos de aceleración, telemetría, habilitación de operadores.

## Guía de migración de configuraciónLos operadores que pasen de redes exclusivas de Ed25519 a implementaciones habilitadas para SM deberían
seguir el proceso por etapas en
[`sm_config_migration.md`](sm_config_migration.md). La guía cubre la construcción.
validación, capas `iroha_config` (`defaults` → `user` → `actual`), génesis
regeneración a través de anulaciones `kagami` (por ejemplo, `kagami genesis generate --allowed-signing sm2 --default-hash sm3-256`), validación previa al vuelo y reversión
planificación para que las instantáneas de configuración y los manifiestos se mantengan consistentes en todo el
flota.

## Política determinista
- Aplicar nonces derivados de RFC6979 para todas las rutas de firma SM2 en SDK y firma de host opcional; Los verificadores solo aceptan codificaciones canónicas r∥s.
- La comunicación del plano de control (streaming) sigue siendo Ed25519; SM2 se limita a firmas de planos de datos a menos que la gobernanza apruebe la expansión.
- Intrínsecos (ARM SM3/SM4) restringidos a operaciones deterministas de verificación/hash con detección de funciones en tiempo de ejecución y respaldo de software.

## Norito y plan de codificación
1. Amplíe las enumeraciones de algoritmos en `iroha_data_model` con `Sm2PublicKey`, `Sm2Signature`, `Sm3Digest`, `Sm4Key`.
2. Serializar firmas SM2 como matrices `r∥s` de ancho fijo big-endian (32+32 bytes) para evitar ambigüedades en DER; Conversiones manejadas en adaptadores. *(Listo: implementado en los asistentes `Sm2Signature`; Norito/JSON ida y vuelta implementados).*
3. Registre identificadores multicódec (`sm3-256`, `sm2-pub`, `sm4-key`) si utiliza multiformatos, actualice accesorios y documentos. *(Progreso: `sm2-pub` código provisional `0x1306` ahora validado con claves derivadas; códigos SM3/SM4 pendientes de asignación final, rastreados a través de `sm_known_answers.toml`.)*
4. Actualice las pruebas de oro Norito que cubren viajes de ida y vuelta y rechazo de codificaciones con formato incorrecto (r o s cortas/largas, parámetros de curva no válidos).## Plan de integración de host y VM (SM-2)
1. Implementar la llamada al sistema `sm3_hash` del lado del host que refleje el ajuste de hash GOST existente; reutilice `Sm3Digest::hash` y exponga rutas de error deterministas. *(Aterrizado: el host devuelve Blob TLV; consulte la implementación de `DefaultHost` y la regresión de `sm_syscalls.rs`).*
2. Amplíe la tabla de llamadas al sistema de VM con `sm2_verify` que acepta firmas r∥s canónicas, valida identificaciones distintivas y asigna fallas a códigos de retorno deterministas. *(Listo: host + Kotodama intrínsecos devuelven `1/0`; el conjunto de regresión ahora cubre firmas truncadas, claves públicas con formato incorrecto, TLV que no son blobs y cargas útiles `distid` UTF-8/vacías/no coincidentes.)*
3. Proporcione llamadas al sistema `sm4_gcm_seal`/`sm4_gcm_open` (y opcionalmente CCM) con un tamaño de etiqueta/nonce explícito (RFC 8998). *(Listo: GCM usa nonces fijos de 12 bytes + etiquetas de 16 bytes; CCM admite nonces de 7 a 13 bytes con longitudes de etiquetas {4,6,8,10,12,14,16} controladas a través de `r14`; Kotodama las expone como `sm::seal_gcm/open_gcm` y `sm::seal_ccm/open_ccm`.) Documente la política de reutilización en el manual del desarrollador.*
4. Conecte los contratos de humo Kotodama y las pruebas de integración IVM que cubran casos positivos y negativos (etiquetas alteradas, firmas con formato incorrecto, algoritmos no compatibles). *(Hecho a través de `crates/ivm/tests/kotodama_sm_syscalls.rs` reflejando regresiones de host para SM3/SM2/SM4.)*
5. Actualice las listas permitidas de llamadas al sistema, las políticas y los documentos ABI (`crates/ivm/docs/syscalls.md`) y actualice los manifiestos con hash después de agregar las nuevas entradas.

### Estado de integración de host y VM
- DefaultHost, CoreHost y WsvHost exponen las llamadas al sistema SM3/SM2/SM4 y las controlan en `sm_enabled`, devolviendo `PermissionDenied` cuando el indicador de tiempo de ejecución está falso.【crates/ivm/src/host.rs:915】【crates/ivm/src/core_host.rs:833】【crates/ivm/src/mock_wsv.rs:2307】
- La activación `crypto.allowed_signing` se realiza a través de canalización/ejecutor/estado para que los nodos de producción opten por participar de manera determinista a través de la configuración; agregar `sm2` alterna la disponibilidad del asistente SM.`【crates/iroha_core/src/smartcontracts/ivm/host.rs:170】【crates/iroha_core/src/state.rs:7673】【crates/iroha_core/src/executor.rs:683】
- La cobertura de regresión ejercita rutas habilitadas y deshabilitadas (DefaultHost/CoreHost/WsvHost) para hash SM3, verificación SM2 y sello/apertura SM4 GCM/CCM. flujos.【crates/ivm/tests/sm_syscalls.rs:129】【crates/ivm/tests/sm_syscalls.rs:733】【crates/ivm/tests/sm_syscalls.rs:1036】

## Hilos de configuración
- Agregue `crypto.allowed_signing`, `crypto.default_hash`, `crypto.sm2_distid_default` y el `crypto.enable_sm_openssl_preview` opcional a `iroha_config`. Asegúrese de que la tubería de características del modelo de datos refleje la caja criptográfica (`iroha_data_model` expone `sm` → `iroha_crypto/sm`).
- Conectar la configuración a las políticas de admisión para que los archivos de manifiestos/génesis definan los algoritmos permitidos; El plano de control sigue siendo Ed25519 de forma predeterminada.### Trabajo de CLI y SDK (SM-3)
1. **Torii CLI** (`crates/iroha_cli`): agregue keygen/importación/exportación SM2 (con reconocimiento de distid), ayudantes de hash SM3 y comandos de cifrado/descifrado SM4 AEAD. Actualice indicaciones y documentos interactivos.
2. **Herramientas de Génesis** (`xtask`, `scripts/`): permite que los manifiestos declaren algoritmos de firma permitidos y hashes predeterminados, falla rápidamente si SM está habilitado sin los controles de configuración correspondientes. *(Hecho: `RawGenesisTransaction` ahora lleva un bloque `crypto` con `default_hash`/`allowed_signing`/`sm2_distid_default`; `ManifestCrypto::validate` e `kagami genesis validate` rechazan configuraciones SM inconsistentes y valores predeterminados/manifiesto de génesis anuncia la instantánea.)*
3. **Superficies SDK**:
   - Rust (`iroha_client`): expone ayudantes de firma/verificación SM2, hash SM3, envoltorios AEAD SM4 con valores predeterminados deterministas.
   - Python/JS/Swift: refleja la API de Rust; reutilice accesorios preparados en `sm_known_answers.toml` para pruebas entre idiomas.
4. Documente el flujo de trabajo del operador para habilitar SM en los inicios rápidos de CLI/SDK y asegúrese de que las configuraciones JSON/YAML acepten las nuevas etiquetas de algoritmo.

#### Progreso de la CLI
- `cargo run -p iroha_cli --features sm -- crypto sm2 keygen --distid CN12345678901234` ahora emite una carga útil JSON que describe el par de claves SM2 junto con un fragmento `client.toml` (`public_key_config`, `private_key_hex`, `distid`). El comando acepta `--seed-hex` para generación determinista y refleja la derivación RFC 6979 utilizada por los hosts.
- `cargo xtask sm-operator-snippet --distid CN12345678901234` envuelve el flujo de keygen/exportación, escribiendo las mismas salidas `sm2-key.json`/`client-sm2.toml` en un solo paso. Utilice `--json-out <path|->`/`--snippet-out <path|->` para redirigir archivos o transmitirlos a la salida estándar, eliminando la dependencia `jq` para la automatización.
- `iroha_cli tools crypto sm2 import --private-key-hex <hex> [--distid ...]` deriva los mismos metadatos del material existente para que los operadores puedan validar las identificaciones distintivas antes de la admisión.
- `iroha_cli tools crypto sm2 export --private-key-hex <hex> --emit-json` imprime el fragmento de configuración (incluida la guía `allowed_signing`/`sm2_distid_default`) y, opcionalmente, reemite el inventario de claves JSON para secuencias de comandos.
- `iroha_cli tools crypto sm3 hash --data <string>` realiza un hash de cargas útiles arbitrarias; `--data-hex` / `--file` cubren entradas binarias y el comando informa resúmenes hexadecimales y base64 para herramientas de manifiesto.
- `iroha_cli tools crypto sm4 gcm-seal --key-hex <KEY> --nonce-hex <NONCE> --plaintext-hex <PT>` (e `gcm-open`) empaquetan los asistentes SM4-GCM del host y muestran `ciphertext_hex`/`tag_hex` o cargas útiles de texto sin formato. `sm4 ccm-seal` / `sm4 ccm-open` proporcionan la misma UX para CCM con validación de longitud nonce (7–13 bytes) y longitud de etiqueta (4,6,8,10,12,14,16); Ambos comandos emiten opcionalmente bytes sin formato al disco.## Estrategia de prueba
### Pruebas unitarias/de respuestas conocidas
- Vectores GM/T 0004 y GB/T 32905 para SM3 (por ejemplo, `"abc"`).
- Vectores GM/T 0002 y RFC 8998 para SM4 (bloque + GCM/CCM).
- Ejemplos de GM/T 0003/GB/T 32918 para SM2 (valor Z, verificación de firma), incluido el ejemplo del anexo 1 con ID `ALICE123@YAHOO.COM`.
- Ficha de puesta en escena del equipamiento provisional: `crates/iroha_crypto/tests/fixtures/sm_known_answers.toml`.
- El conjunto de regresión SM2 derivado de Wycheproof (`crates/iroha_crypto/tests/sm2_wycheproof.rs`) ahora incluye un corpus de 52 casos que superpone dispositivos deterministas (Anexo D, semillas de SDK) con negativos de inversión de bits, manipulación de mensajes y firmas truncadas. El JSON desinfectado reside en `crates/iroha_crypto/tests/fixtures/wycheproof_sm2.json` y `sm2_fuzz.rs` lo consume directamente para que tanto los escenarios de ruta feliz como de manipulación permanezcan alineados en las ejecuciones de propiedades/fuzz. 벡터들은 표준 곡선뿐만 아니라 영역도 다루며, 필요 시내logging `Sm2PublicKey` 검증 이후 BigInt 백업 루틴이 추적을 완료합니다.
- `cargo xtask sm-wycheproof-sync --input <wycheproof-sm2.json>` (o `--input-url <https://…>`) recorta de manera determinista cualquier caída ascendente (etiqueta de generador opcional) y reescribe `crates/iroha_crypto/tests/fixtures/wycheproof_sm2.json`. Hasta que C2SP publique el corpus oficial, descargue las bifurcaciones manualmente y aliméntelas a través del asistente; normaliza claves, recuentos y indicadores para que los revisores puedan razonar sobre las diferencias.
- SM2/SM3 Norito ida y vuelta validados en `crates/iroha_data_model/tests/sm_norito_roundtrip.rs`.
- Regresión de llamada al sistema del host SM3 en `crates/ivm/tests/sm_syscalls.rs` (función SM).
- SM2 verifica la regresión de syscall en `crates/ivm/tests/sm_syscalls.rs` (casos de éxito + fracaso).

### Pruebas de propiedad y regresión
- Proptest para SM2 rechazando curvas inválidas, r/s no canónicos y reutilización de nonces. *(Disponible en `crates/iroha_crypto/tests/sm2_fuzz.rs`, cerrado detrás de `sm_proptest`; habilitado a través de `cargo test -p iroha_crypto --features "sm sm_proptest"`.)*
- Vectores Wycheproof SM4 (modo bloque/AES) adaptados para modos variados; realice un seguimiento aguas arriba para las adiciones de SM2. `sm3_sm4_vectors.rs` ahora practica cambios de bits de etiquetas, etiquetas truncadas y manipulación de texto cifrado tanto para GCM como para CCM.

### Interoperabilidad y rendimiento
- RustCrypto ↔ OpenSSL/Tongsuo parity suite para firma/verificación SM2, resúmenes SM3 y SM4 ECB/GCM vive en `crates/iroha_crypto/tests/sm_cli_matrix.rs`; invocarlo con `scripts/sm_interop_matrix.sh`. Los vectores de paridad CCM ahora se ejecutan en `sm3_sm4_vectors.rs`; El soporte de la matriz CLI seguirá una vez que las CLI ascendentes expongan los ayudantes de CCM.
- El asistente SM3 NEON ahora ejecuta la ruta de compresión/relleno Armv8 de extremo a extremo con control de tiempo de ejecución a través de `sm_accel::is_sm3_enabled` (función + anulaciones de entorno reflejadas en SM3/SM4). Los resúmenes dorados (cero/`"abc"`/bloque largo + longitudes aleatorias) y las pruebas de desactivación forzada mantienen la paridad con el backend escalar de RustCrypto, y el microbanco Criterion (`crates/sm3_neon/benches/digest.rs`) captura el rendimiento escalar frente a NEON en hosts AArch64.
- Arnés de rendimiento que refleja `scripts/gost_bench.sh` para comparar Ed25519/SHA-2 con SM2/SM3/SM4 y validar los umbrales de tolerancia.#### Arm64 Baseline (Apple Silicon local; Criterio `sm_perf`, actualizado el 5 de diciembre de 2025)
- `scripts/sm_perf.sh` ahora ejecuta el banco de criterios y aplica medianas contra `crates/iroha_crypto/benches/sm_perf_baseline.json` (registrado en aarch64 macOS; tolerancia del 25 % de forma predeterminada, los metadatos de referencia capturan el triple del host). El nuevo indicador `--mode` permite a los ingenieros capturar puntos de datos escalares, NEON y `sm-neon-force` sin editar el script; el paquete de captura actual (JSON sin formato + resumen agregado) se encuentra en `artifacts/sm_perf/2026-03-lab/m3pro_native/` y marca cada carga útil con `cpu_label="m3-pro-native"`.
- Los modos de aceleración ahora seleccionan automáticamente la línea base escalar como objetivo de comparación. `scripts/sm_perf.sh` subprocesos de `--compare-baseline/--compare-tolerance/--compare-label` a `sm_perf_check`, emitiendo deltas por punto de referencia contra la referencia escalar y fallando cuando la desaceleración excede el umbral configurado. Las tolerancias por punto de referencia desde la línea base impulsan la protección de comparación (SM3 tiene un límite del 12% en la línea base escalar de Apple, mientras que el delta de comparación SM3 ahora permite hasta un 70% contra la referencia escalar para evitar fluctuaciones); Las líneas de base de Linux reutilizan el mismo mapa de comparación porque se exportan desde la captura `neoverse-proxy-macos`, y las ajustaremos después de una ejecución de Neoverse sin sistema operativo si las medianas difieren. Pase `--compare-tolerance` explícitamente al capturar límites más estrictos (por ejemplo, `--compare-tolerance 0.20`) y use `--compare-label` para anotar hosts de referencia alternativos.
- Las líneas de base registradas en la máquina de referencia de CI ahora se encuentran en `crates/iroha_crypto/benches/sm_perf_baseline_aarch64_macos_scalar.json`, `sm_perf_baseline_aarch64_macos_auto.json` e `sm_perf_baseline_aarch64_macos_neon_force.json`. Actualícelos con `scripts/sm_perf.sh --mode scalar --write-baseline`, `--mode auto --write-baseline` o `--mode neon-force --write-baseline` (configure `SM_PERF_CPU_LABEL` antes de capturar) y archive el JSON generado junto con los registros de ejecución. Mantenga el resultado auxiliar agregado (`artifacts/.../aggregated.json`) con el PR para que los revisores puedan auditar cada muestra. Las líneas base de Linux/Neoverse ahora se envían en `sm_perf_baseline_aarch64_unknown_linux_gnu_{mode}.json`, promovidas desde `artifacts/sm_perf/2026-03-lab/neoverse-proxy-macos/aggregated.json` (etiqueta de CPU `neoverse-proxy-macos`, tolerancia de comparación SM3 0,70 para aarch64 macOS/Linux); Vuelva a ejecutarlo en hosts Neoverse básicos cuando esté disponible para ajustar las tolerancias.
- Los archivos JSON de referencia ahora pueden llevar un objeto `tolerances` opcional para reforzar las barreras de seguridad según el punto de referencia. Ejemplo:
  ```json
  {
    "benchmarks": { "...": 12.34 },
    "tolerances": {
      "sm4_vs_chacha20poly1305_encrypt/sm4_gcm_encrypt": 0.08,
      "sm3_vs_sha256_hash/sm3_hash": 0.12
    }
  }
  ```
  `sm_perf_check` aplica estos límites fraccionarios (8 % y 12 % en el ejemplo) mientras utiliza la tolerancia CLI global para cualquier punto de referencia que no aparezca en la lista.
- Los guardias de comparación también pueden respetar `compare_tolerances` en la línea base de comparación. Utilice esto para permitir un delta más flexible con respecto a la referencia escalar (por ejemplo, `\"sm3_vs_sha256_hash/sm3_hash\": 0.70` en la línea base escalar) mientras mantiene el `tolerances` primario estricto para verificaciones de línea base directas.- Las líneas base registradas de Apple Silicon ahora se envían con barandillas de concreto: las operaciones SM2/SM4 permiten una deriva del 12 al 20 % dependiendo de la variación, mientras que las comparaciones SM3/ChaCha se ubican en un 8 al 12 %. La tolerancia `sm3` de la línea de base escalar ahora se ajusta a 0,12; los archivos `unknown_linux_gnu` reflejan la exportación `neoverse-proxy-macos` con el mismo mapa de tolerancia (comparación SM3 0.70) y notas de metadatos que indican que se envían para la puerta de Linux hasta que esté disponible una repetición completa de Neoverse.
- Firma SM2: 298 µs por operación (Ed25519: 32 µs) ⇒ ~9,2 veces más lento; verificación: 267 µs (Ed25519: 41 µs) ⇒ ~6,5 veces más lento.
- Hashing SM3 (carga útil de 4 KB): 11,2 µs, paridad efectiva con SHA-256 a 11,3 µs (≈356 MiB/s frente a 353 MiB/s).
- Sello/abierto SM4-GCM (carga útil de 1 KiB, nonce de 12 bytes): 15,5 µs frente a ChaCha20-Poly1305 a 1,78 µs (≈64 MiB/s frente a 525 MiB/s).
- Artefactos de referencia (`target/criterion/sm_perf*`) capturados para reproducibilidad; las líneas de base de Linux provienen de `artifacts/sm_perf/2026-03-lab/neoverse-proxy-macos/` (etiqueta de CPU `neoverse-proxy-macos`, tolerancia de comparación SM3 0,70) y se pueden actualizar en hosts Neoverse sin sistema operativo (`SM-4c.1`) una vez que se abre el tiempo de laboratorio para ajustar las tolerancias.

#### Lista de verificación de captura entre arquitecturas
- Ejecute `scripts/sm_perf_capture_helper.sh` **en la máquina de destino** (estación de trabajo x86_64, servidor Neoverse ARM, etc.). Pase `--cpu-label <host>` para sellar las capturas y (cuando se ejecuta en modo matricial) para completar previamente el plan/comandos generados para la programación del laboratorio. El asistente imprime comandos específicos del modo que:
  1. ejecutar la suite Criterion con el conjunto de funciones correcto, y
  2. escriba medianas en `crates/iroha_crypto/benches/sm_perf_baseline_${arch}_${os}_${mode}.json`.
- Primero capture la línea de base escalar y luego vuelva a ejecutar el asistente para `auto` (y `neon-force` en plataformas AArch64). Utilice un `SM_PERF_CPU_LABEL` significativo para que los revisores puedan rastrear los detalles del host en los metadatos JSON.
- Después de cada ejecución, archive el directorio `target/criterion/sm_perf*` sin procesar e inclúyalo en el PR junto con las líneas base generadas. Ajuste las tolerancias por punto de referencia tan pronto como se estabilicen dos ejecuciones consecutivas (consulte `sm_perf_baseline_aarch64_macos_*.json` para obtener formato de referencia).
- Registre las medianas + tolerancias en esta sección y actualice `status.md`/`roadmap.md` cuando se cubra una nueva arquitectura. Las líneas base de Linux ahora se registran desde la captura `neoverse-proxy-macos` (los metadatos indican la exportación a la puerta aarch64-unknown-linux-gnu); Vuelva a ejecutarlo en hosts Neoverse/x86_64 sin sistema operativo como seguimiento cuando esas ranuras de laboratorio estén disponibles.

#### Intrínsecos ARMv8 SM3/SM4 frente a rutas escalares
`sm_accel` (ver `crates/iroha_crypto/src/sm.rs:739`) proporciona la capa de distribución en tiempo de ejecución para los asistentes SM3/SM4 respaldados por NEON. La función está protegida en tres niveles:| Capa | Controlar | Notas |
|-------|---------|-------|
| Tiempo de compilación | `--features sm` (ahora incorpora `sm-neon` automáticamente en `aarch64`) o `sm-neon-force` (pruebas/puntos de referencia) | Construye los módulos NEON y vincula `sm3-neon`/`sm4-neon`. |
| Detección automática en tiempo de ejecución | `sm4_neon::is_supported()` | Solo es cierto en CPU que exponen equivalentes AES/PMULL (por ejemplo, Apple serie M, Neoverse V1/N2). Las máquinas virtuales que enmascaran NEON o FEAT_SM4 recurren al código escalar. |
| Anulación del operador | `crypto.sm_intrinsics` (`auto`/`force-enable`/`force-disable`) | Despacho basado en configuración aplicado al inicio; use `force-enable` solo para crear perfiles en entornos confiables y prefiera `force-disable` al validar respaldos escalares. |

**Envolvente de rendimiento (Apple M3 Pro; medianas registradas en `sm_perf_baseline_aarch64_macos_{mode}.json`):**

| Modo | Resumen SM3 (4KiB) | Sello SM4-GCM (1KiB) | Notas |
|------|-------------------|----------------------|-------|
| Escalar | 11,6 µs | 15,9 µs | Ruta determinista de RustCrypto; Se utiliza en todos los lugares donde se compila la función `sm` pero NEON no está disponible. |
| NEÓN automático | ~2,7 veces más rápido que el escalar | ~2,3 veces más rápido que el escalar | Los núcleos NEON actuales (SM-5a.2c) amplían el cronograma cuatro palabras a la vez y utilizan distribución en cola dual; las medianas exactas varían según el host, así que consulte los metadatos JSON de referencia. |
| Fuerza NEÓN | Refleja NEON auto pero desactiva completamente el respaldo | Igual que NEON auto | Ejercido vía `scripts/sm_perf.sh --mode neon-force`; mantiene la CI honesta incluso en hosts que por defecto estarían en modo escalar. |

**Determinismo y orientación de implementación**
- Los intrínsecos nunca cambian los resultados observables: `sm_accel` devuelve `None` cuando la ruta acelerada no está disponible, por lo que se ejecuta el asistente escalar. Por lo tanto, las rutas del código de consenso siguen siendo deterministas siempre que la implementación escalar sea correcta.
- **No** controle la lógica empresarial sobre si se utilizó la ruta NEON. Trate la aceleración puramente como una sugerencia de rendimiento y exponga el estado solo mediante telemetría (por ejemplo, indicador `sm_intrinsics_enabled`).
- Ejecute siempre `ci/check_sm_perf.sh` (o `make check-sm-perf`) después de tocar el código SM para que el arnés Criterion valide las rutas escalares y aceleradas utilizando las tolerancias integradas en cada JSON de referencia.
- Al realizar evaluaciones comparativas o depurar, prefiera la perilla de configuración `crypto.sm_intrinsics` a los indicadores en tiempo de compilación; La recompilación con `sm-neon-force` deshabilita por completo el respaldo escalar, mientras que `force-enable` simplemente empuja la detección del tiempo de ejecución.
- Documentar la política elegida en las notas de la versión: las compilaciones de producción deben dejar la política en `Auto`, lo que permite que cada validador descubra capacidades de hardware de forma independiente y al mismo tiempo comparta los mismos artefactos binarios.
- Evite enviar archivos binarios que combinen elementos intrínsecos de proveedores vinculados estáticamente (por ejemplo, bibliotecas SM4 de terceros) a menos que respeten el mismo flujo de envío y prueba; de lo contrario, nuestras herramientas de referencia no detectarán las regresiones de rendimiento.#### x86_64 Línea base de Rosetta (Apple M3 Pro; capturado el 1 de diciembre de 2025)
- Las líneas de base se encuentran en `crates/iroha_crypto/benches/sm_perf_baseline_x86_64_macos_{scalar,auto,neon_force}.json` (cpu_label=`m3-pro-rosetta`), con capturas sin procesar y agregadas en `artifacts/sm_perf/2026-03-lab/m3pro_rosetta/`.
- Las tolerancias por punto de referencia en x86_64 están establecidas en 20 % para SM2, 15 % para Ed25519/SHA-256 y 12 % para SM4/ChaCha. `scripts/sm_perf.sh` ahora establece de forma predeterminada la tolerancia de comparación de aceleración al 25% en hosts que no son AArch64, por lo que escalar versus automático se mantiene ajustado y deja la holgura de 5.25 en AArch64 para la línea de base compartida `m3-pro-native` hasta que llegue una repetición de Neoverse.

| Punto de referencia | Escalar | Automático | Fuerza de neón | Automático frente a escalar | Neón vs Escalar | Neón vs Auto |
|-----------|--------|------|------------|----------------|---------------|--------------|
| sm2_vs_ed25519_sign/ed25519_sign |    57,43 |  57,12 |      55,77 |          -0,53% |         -2,88% |        -2,36% |
| sm2_vs_ed25519_sign/sm2_sign |   572,76 | 568,71 |     557,83 |          -0,71% |         -2,61% |        -1,91% |
| sm2_vs_ed25519_verify/verify/ed25519 |    69,03 |  68,42 |      66,28 |          -0,88% |         -3,97% |        -3,12% |
| sm2_vs_ed25519_verify/verify/sm2 |   521,73 | 514,50 |     502.17 |          -1,38% |         -3,75% |        -2,40% |
| sm3_vs_sha256_hash/sha256_hash |    16,78 |  16,58 |      16.16 |          -1,19% |         -3,69% |        -2,52% |
| sm3_vs_sha256_hash/sm3_hash |    15,78 |  15.51 |      15.04 |          -1,71% |         -4,69% |        -3,03% |
| sm4_vs_chacha20poly1305_decrypt/chacha20poly1305_decrypt |     1,96 |   1,97 |       1,97 |           0,39% |          0,16% |        -0,23% |
| sm4_vs_chacha20poly1305_decrypt/sm4_gcm_decrypt |    16.26 |  16.38 |      16.26 |           0,72% |         -0,01% |        -0,72% |
| sm4_vs_chacha20poly1305_encrypt/chacha20poly1305_encrypt |     1,96 |   2.00 |       1,93 |           2,23% |         -1,14% |        -3,30% |
| sm4_vs_chacha20poly1305_encrypt/sm4_gcm_encrypt |    16.60 |  16,58 |      16.15 |          -0,10% |         -2,66% |        -2,57% |

#### x86_64/otros objetivos que no son aarch64
- Las compilaciones actuales todavía incluyen solo la ruta escalar determinista RustCrypto en x86_64; mantenga `sm` habilitado pero **no** inyecte kernels AVX2/VAES externos hasta que aterrice SM-4c.1b. La política de tiempo de ejecución refleja ARM: por defecto es `Auto`, respeta `crypto.sm_intrinsics` y muestra los mismos indicadores de telemetría.
- Las capturas de Linux/x86_64 aún no se han registrado; reutilice el asistente en ese hardware y coloque las medianas en `sm_perf_baseline_x86_64_unknown_linux_gnu_{mode}.json` junto con las líneas base de Rosetta y el mapa de tolerancia anteriores.**Errores comunes**
1. **Instancias ARM virtualizadas:** Muchas nubes exponen NEON pero ocultan las extensiones SM4/AES que verifica `sm4_neon::is_supported()`. Espere la ruta escalar en esos entornos y capture las líneas base de rendimiento en consecuencia.
2. **Anulaciones parciales:** La combinación de valores `crypto.sm_intrinsics` persistentes entre ejecuciones genera lecturas de rendimiento inconsistentes. Documente la anulación prevista en el ticket del experimento y restablezca la configuración antes de capturar nuevas líneas de base.
3. **Paridad de CI:** Algunos ejecutores de macOS no permiten el muestreo de rendimiento basado en contadores mientras NEON está activo. Mantenga las salidas `scripts/sm_perf_capture_helper.sh` adjuntas a los RP para que los revisores puedan confirmar que se ejerció la ruta acelerada incluso si el corredor oculta esos contadores.
4. **Variantes futuras de ISA (SVE/SVE2):** Los núcleos actuales asumen formas de carril NEON. Antes de migrar a SVE/SVE2, extienda `sm_accel::NeonPolicy` con una variante dedicada para que podamos mantener alineados los controles de CI, telemetría y operador.

Los elementos de acción rastreados en SM-5a/SM-4c.1 garantizan que CI capture pruebas de paridad para cada nueva arquitectura, y la hoja de ruta se mantiene en 🈺 hasta que las líneas base de Neoverse/x86 y las tolerancias NEON versus escalares converjan.

## Notas regulatorias y de cumplimiento

### Estándares y referencias normativas
- **GM/T 0002-2012** (SM4), **GM/T 0003-2012** + **GB/T 32918 series** (SM2), **GM/T 0004-2012** + **GB/T 32905/32907** (SM3) y **RFC 8998** rigen las definiciones de algoritmos, vectores de prueba y enlaces KDF que nuestros dispositivos consumir.【docs/source/crypto/sm_vectors.md#L79】
- El informe de cumplimiento en `docs/source/crypto/sm_compliance_brief.md` vincula estos estándares con las responsabilidades de presentación/exportación de los equipos de ingeniería, SRE y legales; mantenga ese resumen actualizado cada vez que se revise el catálogo de GM/T.

### Flujo de trabajo regulatorio de China continental
1. **Presentación de producto (开发备案):** Antes de enviar binarios habilitados para SM desde China continental, envíe el manifiesto de artefacto, los pasos de compilación deterministas y la lista de dependencias a la administración provincial de criptografía. Las plantillas de archivo y la lista de verificación de cumplimiento se encuentran en `docs/source/crypto/sm_compliance_brief.md` y en el directorio de archivos adjuntos (`sm_product_filing_template.md`, `sm_sales_usage_filing_template.md`, `sm_export_statement_template.md`).
2. **Presentación de ventas/uso (销售/使用备案):** Los operadores que ejecutan nodos habilitados para SM en tierra deben registrar su alcance de implementación, postura de administración de claves y plan de telemetría. Adjunte manifiestos firmados más instantáneas de métricas `iroha_sm_*` al presentar.
3. **Pruebas acreditadas:** Los operadores de infraestructura crítica pueden exigir informes de laboratorio certificados. Proporcione scripts de compilación reproducibles, exportaciones de SBOM y artefactos de interoperabilidad/Wycheproof (ver más abajo) para que los auditores posteriores puedan reproducir los vectores sin alterar el código.
4. **Seguimiento de estado:** Registre las presentaciones completadas en el ticket de liberación e `status.md`; las presentaciones faltantes bloquean la promoción de pilotos de solo verificación a pilotos de firma.### Postura de exportación y distribución
- Trate los binarios con capacidad SM como elementos controlados según la **Categoría 5 EAR de EE. UU., Parte 2** y el **Reglamento UE 2021/821, Anexo 1 (5D002)**. La publicación de la fuente continúa calificando para las excepciones de código abierto/ENC, pero la redistribución a destinos embargados aún requiere una revisión legal.
- Los manifiestos de lanzamiento deben incluir una declaración de exportación que haga referencia a la base ENC/TSU y enumerar los identificadores de compilación de OpenSSL/Tongsuo si la vista previa de FFI está empaquetada.
- Preferir el embalaje regional local (por ejemplo, espejos continentales) cuando los operadores necesitan distribución en tierra para evitar problemas de transferencia transfronteriza.

### Documentación y evidencia del operador
- Combine este resumen de arquitectura con la lista de verificación de implementación en `docs/source/crypto/sm_operator_rollout.md` y la guía de presentación de cumplimiento en `docs/source/crypto/sm_compliance_brief.md`.
- Mantenga sincronizado el inicio rápido de génesis/operador en `docs/genesis.md`, `docs/genesis.he.md` e `docs/genesis.ja.md`; El flujo de trabajo de la CLI SM2/SM3 es la fuente de verdad de cara al operador para generar manifiestos `crypto`.
- Archivar la procedencia de OpenSSL/Tongsuo, la salida `scripts/sm_openssl_smoke.sh` y los registros de paridad `scripts/sm_interop_matrix.sh` con cada paquete de lanzamiento para que los socios de cumplimiento y auditoría tengan artefactos deterministas.
- Actualice `status.md` cada vez que cambie el alcance del cumplimiento (nuevas jurisdicciones, finalización de presentaciones o decisiones de exportación) para mantener visible el estado del programa.
- Siga las revisiones de preparación por etapas (`SM-RR1`–`SM-RR3`) capturadas en `docs/source/release_dual_track_runbook.md`; La promoción entre las fases de solo verificación, piloto y firma GA requiere los artefactos enumerados allí.

## Recetas de interoperabilidad

### RustCrypto ↔ Matriz OpenSSL/Tongsuo
1. Asegúrese de que las CLI de OpenSSL/Tongsuo estén disponibles (`IROHA_SM_CLI="openssl /opt/tongsuo/bin/openssl"` permite la selección explícita de herramientas).
2. Ejecute `scripts/sm_interop_matrix.sh`; invoca `cargo test -p iroha_crypto --test sm_cli_matrix --features sm` y ejercita la firma/verificación SM2, los resúmenes SM3 y los flujos SM4 ECB/GCM contra cada proveedor, omitiendo cualquier CLI que esté ausente.【scripts/sm_interop_matrix.sh#L1】
3. Archive los archivos `target/debug/deps/sm_cli_matrix*.log` resultantes con los artefactos de la versión.

### Vista previa de OpenSSL Smoke (Puerta de embalaje)
1. Instale los encabezados de desarrollo OpenSSL ≥3.0 y asegúrese de que `pkg-config` pueda ubicarlos.
2. Ejecute `scripts/sm_openssl_smoke.sh`; el asistente ejecuta `cargo check`/`cargo test --test sm_openssl_smoke`, ejercitando hash SM3, verificación SM2 y viajes de ida y vuelta SM4-GCM a través del backend de FFI (el arnés de prueba habilita la vista previa explícitamente).【scripts/sm_openssl_smoke.sh#L1】
3. Trate cualquier falla sin salto como un bloqueador de liberación; capturar la salida de la consola para evidencia de auditoría.

### Actualización determinista del accesorio
- Regenere los accesorios SM (`sm_vectors.md`, `fixtures/sm/…`) antes de cada presentación de cumplimiento, luego vuelva a ejecutar la matriz de paridad y el arnés de humo para que los auditores reciban transcripciones deterministas nuevas junto con las presentaciones.## Preparación para la auditoría externa
- `docs/source/crypto/sm_audit_brief.md` empaqueta el contexto, alcance, cronograma y contactos para la revisión externa.
- Los artefactos de auditoría se encuentran bajo `docs/source/crypto/attachments/` (registro de humo de OpenSSL, instantánea del árbol de carga, exportación de metadatos de carga, procedencia del kit de herramientas) e `fuzz/sm_corpus_manifest.json` (semillas de fuzz SM deterministas obtenidas de vectores de regresión existentes). En macOS, el registro de humo registra actualmente una ejecución omitida porque el ciclo de dependencia del espacio de trabajo impide `cargo check`; Las compilaciones de Linux sin el ciclo ejercitarán completamente la vista previa del backend.
- Distribuido a los líderes de Crypto WG, Platform Ops, Security y Docs/DevRel el 30 de enero de 2026 para su alineación antes del envío de la RFQ.

### Estado del compromiso de auditoría

- **Trail of Bits (práctica de criptografía CN)** — Declaración de trabajo ejecutada el **2026-02-21**, inicio **2026-02-24**, ventana de trabajo de campo **2026-02-24–2026-03-22**, informe final pendiente **2026-04-15**. Punto de control de estado semanal todos los miércoles a las 09:00 UTC con el líder de Crypto WG y el enlace de Ingeniería de Seguridad. Consulte [`sm_audit_brief.md`](sm_audit_brief.md#engagement-status) para contactos, entregables y archivos adjuntos de evidencia.
- **NCC Group APAC (espacio de contingencia)**: reservó la ventana de mayo de 2026 como una revisión de seguimiento/paralela en caso de que hallazgos adicionales o solicitudes de reguladores requieran una segunda opinión. Los detalles del compromiso y los enlaces de escalada se registran junto con la entrada Trail of Bits en `sm_audit_brief.md`.

## Riesgos y mitigaciones

Registro completo: consulte [`sm_risk_register.md`](sm_risk_register.md) para obtener detalles
puntuación de probabilidad/impacto, activadores de seguimiento e historial de aprobación. el
El resumen a continuación rastrea los elementos principales que surgieron en la ingeniería de lanzamiento.
| Riesgo | Gravedad | Propietario | Mitigación |
|------|----------|-------|------------|
| Falta de auditoría externa para las cajas RustCrypto SM | Alto | Grupo de Trabajo sobre Cripto | Rastro de contrato de Bits/NCC Group, mantener solo verificación hasta que se acepte el informe de auditoría. |
| Regresiones nonce deterministas entre SDK | Alto | Líderes del programa SDK | Comparta accesorios a través del SDK CI; hacer cumplir la codificación canónica r∥s; agregue pruebas de integración entre SDK (seguidas en SM-3c). |
| Errores específicos de ISA en intrínsecos | Medio | Grupo de Trabajo sobre Rendimiento | Los elementos intrínsecos de la puerta de funciones requieren cobertura de CI en ARM y mantienen el respaldo del software. Matriz de validación de hardware mantenida en `sm_perf.md`. |
| La ambigüedad en el cumplimiento retrasa la adopción | Medio | Documentos y enlace legal | Publicar el informe de cumplimiento y la lista de verificación del operador (SM-6a/SM-6b) ante la Asamblea General; recopilar información legal. Lista de verificación de presentación enviada en `sm_compliance_brief.md`. |
| Deriva del backend de FFI con actualizaciones de proveedores | Medio | Operaciones de plataforma | Fijar versiones de proveedores, agregar pruebas de paridad, mantener la opción de backend de FFI hasta que el empaquetado se estabilice (SM-P3). |## Preguntas abiertas / Seguimientos
1. Seleccione socios auditores independientes con experiencia en algoritmos SM en Rust.
   - **Respuesta (24/02/2026):** La práctica de criptografía CN de Trail of Bits firmó la SOW de auditoría primaria (inicio el 24/02/2026, entrega el 15/04/2026) y NCC Group APAC tiene un espacio de contingencia en mayo para que los reguladores puedan solicitar una segunda revisión sin reabrir la contratación. El alcance de la participación, los contactos y las listas de verificación se encuentran en [`sm_audit_brief.md`](sm_audit_brief.md#engagement-status) y se reflejan en `sm_audit_vendor_landscape.md`.
2. Continúe el seguimiento en sentido ascendente para obtener un conjunto de datos oficial de Wycheproof SM2; el espacio de trabajo actualmente envía un conjunto seleccionado de 52 cajas (accesorios deterministas + cajas de manipulación sintetizadas) y lo introduce en `sm2_wycheproof.rs`/`sm2_fuzz.rs`. Actualice el corpus a través de `cargo xtask sm-wycheproof-sync` una vez que llegue el JSON ascendente.
   - Seguimiento de las suites de vectores negativos Bouncy Castle y GmSSL; importe a `sm2_fuzz.rs` una vez que se haya autorizado la licencia para complementar el corpus existente.
3. Definir la telemetría de referencia (métricas, registros) para el seguimiento de la adopción de SM.
4. Decida si el valor predeterminado de SM4 AEAD es GCM o CCM para la exposición Kotodama/VM.
5. Realice un seguimiento de la paridad de RustCrypto/OpenSSL para el ejemplo del anexo 1 (ID `ALICE123@YAHOO.COM`): confirme la compatibilidad de la biblioteca con la clave pública publicada y `(r, s)` para que los dispositivos puedan promocionarse a pruebas de regresión.

## Elementos de acción
- [x] Finalizar la auditoría de dependencia y la captura en el rastreador de seguridad.
- [x] Confirmar la participación del socio de auditoría para las cajas RustCrypto SM (seguimiento de SM-P0). Trail of Bits (práctica de criptografía CN) posee la revisión principal con fechas de inicio/entrega registradas en `sm_audit_brief.md`, y NCC Group APAC retuvo un espacio de contingencia de mayo de 2026 para satisfacer los seguimientos de los reguladores o de gobernanza.
- [x] Ampliar la cobertura de Wycheproof para casos de manipulación SM4 CCM (SM-4a).
- [x] Conectar dispositivos de firma SM2 canónicos a través de SDK y conectarlos a CI (SM-3c/SM-1b.1); custodiado por `scripts/check_sm2_sdk_fixtures.py` (ver `ci/check_sm2_sdk_fixtures.sh`).

## Apéndice de Cumplimiento (Criptografía Comercial Estatal)

- **Clasificación:** SM2/SM3/SM4 se envía bajo el régimen de *criptografía comercial estatal* de China (Ley de Criptografía de la República Popular China, Art.3). Enviar estos algoritmos en el software Iroha **no** coloca el proyecto en los niveles central/común (secreto de estado), pero los operadores que los utilizan en implementaciones de la República Popular China deben seguir las obligaciones de presentación de criptografía comercial y MLPS.【docs/source/crypto/sm_chinese_crypto_law_brief.md:14】
- **Linaje de estándares:** Alinear la documentación pública con las conversiones oficiales GB/T de las especificaciones GM/T:

| Algoritmo | Referencia GB/T | Origen GM/T | Notas |
|-----------|----------------|-------------|-------|
| SM2 | GB/T32918 (todas las piezas) | GM/T0003 | Firma digital ECC + intercambio de claves; Iroha expone la verificación en nodos centrales y la firma determinista en los SDK. |
| SM3 | ES/T32905 | GM/T0004 | hash de 256 bits; hash determinista a través de rutas escalares y aceleradas ARMv8. |
| SM4 | ES/T32907 | GM/T0002 | cifrado de bloques de 128 bits; Iroha proporciona ayudas de GCM/CCM y garantiza la paridad big-endian entre implementaciones. |- **Manifiesto de capacidad:** El punto final Torii `/v1/node/capabilities` anuncia la siguiente forma JSON para que los operadores y las herramientas puedan consumir el manifiesto SM mediante programación:

```json
{
  "abi_version": 1,
  "data_model_version": 1,
  "crypto": {
    "sm": {
      "enabled": true,
      "default_hash": "sm3-256",
      "allowed_signing": ["ed25519"],
      "sm2_distid_default": "1234567812345678",
      "openssl_preview": false,
      "acceleration": {
        "scalar": true,
        "neon_sm3": false,
        "neon_sm4": false,
        "policy": "auto"
      }
    }
  }
}
```

El subcomando CLI `iroha runtime capabilities` muestra la misma carga útil localmente, imprimiendo un resumen de una línea junto con el anuncio JSON para la recopilación de pruebas de cumplimiento.

- **Documentación entregable:** publique notas de la versión y SBOM que identifiquen los algoritmos/estándares anteriores y mantenga el informe de cumplimiento completo (`sm_chinese_crypto_law_brief.md`) junto con los artefactos de la versión para que los operadores puedan adjuntarlo a las presentaciones provinciales.【docs/source/crypto/sm_chinese_crypto_law_brief.md:59】
- **Transferencia del operador:** recordar a los implementadores que MLPS2.0/GB/T39786-2021 requiere evaluaciones de aplicaciones criptográficas, SOP de administración de claves SM y retención de evidencia durante ≥6 años; indíqueles la lista de verificación del operador en el informe de cumplimiento.【docs/source/crypto/sm_chinese_crypto_law_brief.md:43】【docs/source/crypto/sm_chinese_crypto_law_brief.md:74】

##Plan de comunicación
- **Audiencia:** Miembros principales de Crypto WG, ingeniería de lanzamiento, junta de revisión de seguridad, líderes del programa SDK.
- **Artefactos:** `sm_program.md`, `sm_lock_refresh_plan.md`, `sm_vectors.md`, `sm_wg_sync_template.md`, extracto de la hoja de ruta (SM-0 .. SM-7a).
- **Canal:** Agenda de sincronización semanal de Crypto WG + correo electrónico de seguimiento que resume los elementos de acción y solicita aprobación para la actualización de bloqueos y la admisión de dependencias (borrador distribuido el 19 de enero de 2025).
- **Propietario:** Líder de Crypto WG (delegado aceptable).
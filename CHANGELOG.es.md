---
lang: es
direction: ltr
source: CHANGELOG.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 26f5115a14476de15fbc8f26c5a9807954df6884763a818b2bc98ec6cfe1a4cc
source_last_modified: "2026-01-04T13:46:50.705991+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Registro de cambios

[Unreleased]: https://github.com/hyperledger-iroha/iroha/compare/v2.0.0-rc.2.0...HEAD
[2.0.0-rc.2.0]: https://github.com/hyperledger-iroha/iroha/releases/tag/v2.0.0-rc.2.0

Todos los cambios notables en este proyecto se documentarán en este archivo.

## [Inédito]- Dejar caer la cuña ESCALA; `norito::codec` ahora se implementa con la serialización nativa Norito.
- Reemplace los usos de `parity_scale_codec` con `norito::codec` en todas las cajas.
- Comenzar a migrar herramientas a la serialización nativa Norito.
- Elimine la dependencia `parity-scale-codec` restante del espacio de trabajo a favor de la serialización nativa Norito.
- Reemplace las derivaciones de rasgos SCALE residuales con implementaciones nativas Norito y cambie el nombre del módulo de códec versionado.
- Fusionar `iroha_config_base_derive` e `iroha_futures_derive` en `iroha_derive` con macros controladas por funciones.
- *(multifirma)* Rechazar firmas directas de autoridades multifirma con un código/motivo de error estable, aplicar límites de TTL multifirma en retransmisores anidados y mostrar límites de TTL en la CLI antes del envío (paridad de SDK pendiente).
- Mueva las macros de procedimiento de FFI a `iroha_ffi` y elimine la caja `iroha_ffi_derive`.
- *(schema_gen)* Elimina la característica `transparent_api` innecesaria de la dependencia `iroha_data_model`.
- *(data_model)* Almacene en caché el normalizador NFC de ICU para el análisis `Name` para reducir la sobrecarga de inicialización repetida.
- 📚 Inicio rápido de Document JS, resolución de configuración, flujo de trabajo de publicación y receta compatible con la configuración para el cliente Torii.
- *(IrohaSwift)* Elevar los objetivos mínimos de implementación a iOS 15/macOS 12, adoptar la simultaneidad Swift en las API del cliente Torii y marcar los modelos públicos como `Sendable`.
- *(IrohaSwift)* Se agregaron `ToriiDaProofSummaryArtifact` e `DaProofSummaryArtifactEmitter.emit` para que las aplicaciones Swift puedan crear/emitir paquetes de pruebas DA compatibles con CLI sin tener que gastar en CLI, completos con documentos y pruebas de regresión que cubren tanto en memoria como en disco. flujos de trabajo.【F:IrohaSwift/Sources/IrohaSwift/ToriiDaProofSummaryArtifact.swift:1】【F:IrohaSwift/Tests/IrohaSwiftTests/ToriiDaProofSummaryArtifactTests.swift:1】【F:docs/source/sdk/swift/index.md:260】
- *(data_model/js_host)* Corrija la serialización de la opción Kaigi eliminando el indicador de reutilización de archivos de `KaigiParticipantCommitment`, agregue pruebas de ida y vuelta nativas y elimine el respaldo de decodificación JS para que las instrucciones de Kaigi ahora realicen el viaje de ida y vuelta con Norito antes. envío.【F:crates/iroha_data_model/src/kaigi.rs:128】【F:crates/iroha_js_host/src/lib.rs:1379】【F:javascript/iroha_js/test/instructionBuilders.test.js:30】
- *(javascript)* Permitir que las personas que llaman `ToriiClient` eliminen encabezados predeterminados (pasando `null`) para que `getMetrics` cambie limpiamente entre texto JSON e Prometheus Aceptar encabezados.【F:javascript/iroha_js/src/toriiClient.js:488】【F:javascript/iroha_js/src/toriiClient.js:761】
- *(javascript)* Se agregaron ayudas iterables para NFT, saldos de activos por cuenta y titulares de definiciones de activos (con definiciones, documentos y pruebas de TypeScript), por lo que la paginación Torii ahora cubre la aplicación restante. puntos finales.【F:javascript/iroha_js/src/toriiClient.js:105】【F:javascript/iroha_js/index.d.ts:80】【F:javascript/iroha_js/test/toriiClient.test.js:365】【F:javascript/iroha_js/README.md:470】
- *(javascript)* Se agregaron instrucciones de gobernanza/constructores de transacciones más una receta de gobernanza para que los clientes de JS puedan implementar propuestas, votaciones, promulgaciones y persistencia del consejo de principio a fin. fin.【F:javascript/iroha_js/src/instructionBuilders.js:1012】【F:javascript/iroha_js/src/transaction.js:1082】【F:javascript/iroha_js/recipes/governance.mjs:1】
- *(javascript)* Se agregaron ayudantes de envío/estado ISO 20022 pacs.008 y una receta coincidente, lo que permite a las personas que llaman JS ejercer el puente ISO Torii sin HTTP personalizado. fontanería.【F:javascript/iroha_js/src/toriiClient.js:888】【F:javascript/iroha_js/index.d.ts:706】【F:javascript/iroha_js/recipes/iso_bridge.mjs:1】- *(javascript)* Se agregaron ayudantes del constructor pacs.008/pacs.009 además de una receta basada en configuración para que las personas que llaman JS puedan sintetizar cargas útiles ISO 20022 con metadatos BIC/IBAN validados antes de acceder al puente.【F:javascript/iroha_js/src/isoBridge.js:1】【F:javascript/iroha_js/test/isoBridge.test.js:1】【F:javascript/iroha_js/recipes/iso_bridge_builder.mjs:1】【F:javascript/iroha_js/index.d.ts:1】
- *(javascript)* Se completó el ciclo de ingesta/búsqueda/prueba de DA: `ToriiClient.fetchDaPayloadViaGateway` ahora deriva automáticamente identificadores de fragmentación (a través del nuevo enlace `deriveDaChunkerHandle`), los resúmenes de prueba opcionales reutilizan el `generateDaProofSummary` nativo y el archivo README/tips/pruebas se actualizaron para que los llamadores del SDK puedan reflejar `iroha da get-blob/prove-availability` sin medida fontanería.【F:javascript/iroha_js/src/toriiClient.js:1123】【F:javascript/iroha_js/src/dataAvailability.js:1】【F:javascrip t/iroha_js/test/toriiClient.test.js:1454】【F:javascript/iroha_js/index.d.ts:3275】【F:javascript/iroha_js/README.md:760】
- *(javascript/js_host)* Los metadatos del marcador `sorafsGatewayFetch` ahora registran el ID/CID del manifiesto de la puerta de enlace cada vez que se utilizan proveedores de puerta de enlace para que los artefactos de adopción se alineen con las capturas de CLI.
- *(torii/cli)* Aplicar cruces peatonales ISO: Torii ahora rechaza los envíos de `pacs.008` con BIC de agentes desconocidos y la vista previa de DvP CLI valida `--delivery-instrument-id` a través de `--iso-reference-crosswalk`.【F:crates/iroha_torii/src/iso20022_bridge.rs:704】【F:crates/iroha_cli/src/main.rs:3892】
- *(torii)* Agregar ingesta de efectivo PvP a través de `POST /v1/iso20022/pacs009`, aplicando verificaciones de datos de referencia `Purp=SECU` y BIC antes de las transferencias de edificios.
- *(herramientas)* Se agregó `cargo xtask iso-bridge-lint` (más `ci/check_iso_reference_data.sh`) para validar instantáneas ISIN/CUSIP, BIC↔LEI y MIC junto con los accesorios del repositorio. 【F:xtask/src/main.rs:146】【F:ci/check_iso_reference_data.sh:1】
- *(javascript)* Publicación de npm reforzada mediante la declaración de metadatos del repositorio, una lista explícita de archivos permitidos, `publishConfig` habilitado para procedencia, un registro de cambios/protección de prueba `prepublishOnly` y un flujo de trabajo de GitHub Actions que ejercita el Nodo 18/20 en CI【F:javascript/iroha_js/package.json:1】【F:javascript/iroha_js/scripts/check-changelog.mjs:1】【F:docs/source/sdk/js/publishing.md:1】【F:.github/workflows/javascript-sdk.yml:1】
- *(ivm/cuda)* El campo BN254 add/sub/mul ahora se ejecuta en los nuevos kernels CUDA con procesamiento por lotes del lado del host a través de `bn254_launch_kernel`, lo que permite la aceleración de hardware para dispositivos Poseidon y ZK y al mismo tiempo preserva el determinismo. alternativas.【F:crates/ivm/cuda/bn254.cu:1】【F:crates/ivm/src/cuda.rs:66】【F:crates/ivm/src/cuda.rs:1244】

## [2.0.0-rc.2.0] - 2025-05-08

### 🚀 Características

- *(cli)* Agregue `iroha transaction get` y otros comandos importantes (#5289)
- [**última hora**] Separar activos fungibles y no fungibles (#5308)
- [**rompiendo**] Finaliza bloques no vacíos permitiendo bloques vacíos después de ellos (#5320)
- Exponer tipos de telemetría en esquema y cliente (#5387)
- *(iroha_torii)* Códigos auxiliares para puntos finales controlados por funciones (#5385)
- Agregar métricas de tiempo de confirmación (#5380)

### 🐛 Corrección de errores

- Revisar distintos de ceros (#5278)
- Errores tipográficos en archivos de documentación (#5309)
- *(cripto)* Exponer captador `Signature::payload` (#5302) (#5310)
- *(core)* Agregar controles de presencia de rol antes de otorgarlo (#5300)
- *(núcleo)* Reconectar par desconectado (#5325)
- Se corrigieron pytests relacionados con los activos de la tienda y NFT (#5341)
- *(CI)* Se corrigió el flujo de trabajo de análisis estático de Python para poesía v2 (#5374)
- El evento de transacción caducada aparece después de la confirmación (#5396)

### 💼 Otro- Incluir `rust-toolchain.toml` (#5376)
- Advertir en `unused`, no en `deny` (#5377)

### 🚜 Refactorizar

- Paraguas Iroha CLI (#5282)
- *(iroha_test_network)* Utilice un formato bonito para los registros (#5331)
- [**última hora**] Simplifique la serialización de `NumericSpec` en `genesis.json` (#5340)
- Mejorar el registro de conexiones p2p fallidas (#5379)
- Revertir `logger.level`, agregar `logger.filter`, extender rutas de configuración (#5384)

### 📚 Documentación

- Agregar `network.public_address` a `peer.template.toml` (#5321)

### ⚡ Rendimiento

- *(kura)* Evitar escrituras de bloques redundantes en el disco (#5373)
- Almacenamiento personalizado implementado para hashes de transacciones (#5405)

### ⚙️ Tareas varias

- Arreglar el uso de poesía (#5285)
- Eliminar constantes redundantes de `iroha_torii_const` (#5322)
- Eliminar `AssetEvent::Metadata*` no utilizado (#5339)
- Versión de acción Bump Sonarqube (#5337)
- Eliminar permisos no utilizados (#5346)
- Agregar paquete descomprimido a ci-image (#5347)
- Corregir algunos comentarios (#5397)
- Mover las pruebas de integraciones desde la caja `iroha` (#5393)
- Deshabilitar el trabajo defectdojo (#5406)
- Agregar aprobación de DCO para confirmaciones faltantes
- Reorganizar los flujos de trabajo (segundo intento) (#5399)
- No ejecute Pull Request CI al enviar a principal (#5415)

<!-- generated by git-cliff -->

## [2.0.0-rc.1.3] - 2025-03-07

### Agregado

- finalizar bloques no vacíos permitiendo bloques vacíos después de ellos (#5320)

## [2.0.0-rc.1.2] - 2025-02-25

### Fijo

- Los pares reinscritos ahora se reflejan correctamente en la lista de pares (#5327)

## [2.0.0-rc.1.1] - 2025-02-12

### Agregado

- agregue `iroha transaction get` y otros comandos importantes (#5289)

## [2.0.0-rc.1.0] - 2024-12-06

### Agregado

- implementar proyecciones de consultas (#5242)
- usar ejecutor persistente (#5082)
- agregar tiempos de espera de escucha a iroha cli (#5241)
- agregar el punto final API /peers a torii (#5235)
- dirección p2p independiente (#5176)
- mejorar la utilidad y usabilidad de multifirma (#5027)
- proteger `BasicAuth::password` para que no se imprima (#5195)
- orden descendente en la consulta `FindTransactions` (#5190)
- introducir el encabezado del bloque en cada contexto de ejecución de contrato inteligente (#5151)
- tiempo de confirmación dinámico basado en el índice de cambio de vista (#4957)
- definir el conjunto de permisos predeterminado (#5075)
- agregar implementación de Niche para `Option<Box<R>>` (#5094)
- predicados de transacción y bloque (#5025)
- informar la cantidad de artículos restantes en consulta (#5016)
- tiempo discreto acotado (#4928)
- agregar operaciones matemáticas faltantes a `Numeric` (#4976)
- validar mensajes de sincronización de bloques (#4965)
- filtros de consulta (#4833)

### Cambiado

- simplificar el análisis de ID de pares (#5228)
- mover el error de transacción fuera de la carga útil del bloque (#5118)
- cambiar el nombre de JsonString a Json (#5154)
- agregar entidad de cliente a contratos inteligentes (#5073)
- líder como servicio de pedidos de transacciones (#4967)
- hacer que kura suelte bloques viejos de la memoria (#5103)
- use `ConstVec` para obtener instrucciones en `Executable` (#5096)
- mensajes de chismes como máximo una vez (#5079)
- reducir el uso de memoria de `CommittedTransaction` (#5089)
- hacer que los errores del cursor de consulta sean más específicos (#5086)
- reorganizar cajas (#4970)
- introducir la consulta `FindTriggers`, eliminar `FindTriggerById` (#5040)
- no dependa de firmas para la actualización (#5039)
- cambiar el formato de los parámetros en genesis.json (#5020)
- enviar solo prueba de cambio de vista actual y anterior (#4929)
- deshabilite el envío de mensajes cuando no esté listo para evitar un bucle ocupado (#5032)
- mover la cantidad total de activos a la definición de activos (#5029)
- firmar sólo el encabezado del bloque, no toda la carga útil (#5000)
- use `HashOf<BlockHeader>` como tipo de hash de bloque (#4998)
- simplificar `/health` y `/api_version` (#4960)
- cambie el nombre de `configs` a `defaults`, elimine `swarm` (#4862)

### Fijo- aplanar el rol interno en json (#5198)
- corrige las advertencias `cargo audit` (#5183)
- agregar verificación de rango al índice de firmas (#5157)
- Corregir ejemplo de macro de modelo en documentos (#5149)
- cerrar ws correctamente en bloques/flujo de eventos (#5101)
- verificación de pares confiables fallida (#5121)
- comprobar que el siguiente bloque tenga altura +1 (#5111)
- Corregir la marca de tiempo del bloque génesis (#5098)
- Se corrige la compilación `iroha_genesis` sin la función `transparent_api` (#5056)
- manejar correctamente `replace_top_block` (#4870)
- arreglar la clonación del ejecutor (#4955)
- mostrar más detalles del error (#4973)
- use `GET` para flujo de bloques (#4990)
- mejorar el manejo de transacciones en cola (#4947)
- evitar mensajes de bloqueo de sincronización de bloques redundantes (#4909)
- evitar interbloqueo en el envío simultáneo de mensajes grandes (#4948)
- eliminar la transacción caducada del caché (#4922)
- arreglar la URL torii con la ruta (#4903)

### Eliminado

- eliminar la API basada en módulos del cliente (#5184)
- eliminar `riffle_iter` (#5181)
- eliminar dependencias no utilizadas (#5173)
- eliminar el prefijo `max` de `blocks_in_memory` (#5145)
- eliminar estimación de consenso (#5116)
- eliminar `event_recommendations` del bloque (#4932)

### Seguridad

## [2.0.0-pre-rc.22.1] - 2024-07-30

### Fijo

- se agregó `jq` a la imagen de la ventana acoplable

## [2.0.0-pre-rc.22.0] - 2024-07-25

### Agregado

- especificar parámetros en cadena explícitamente en génesis (#4812)
- permitir turbofish con múltiples `Instruction` (#4805)
- reimplementar transacciones multifirma (#4788)
- implementar parámetros en cadena integrados y personalizados (#4731)
- mejorar el uso de instrucciones personalizadas (#4778)
- hacer que los metadatos sean dinámicos mediante la implementación de JsonString (#4732)
- permitir que varios pares envíen el bloque de génesis (#4775)
- suministre `SignedBlock` en lugar de `SignedTransaction` al par (#4739)
- instrucciones personalizadas en el ejecutor (#4645)
- ampliar la CLI del cliente para solicitar consultas json (#4684)
 - agregar soporte de detección para `norito_decoder` (#4680)
- generalizar el esquema de permisos al modelo de datos del ejecutor (#4658)
- Se agregaron permisos de activación de registro en el ejecutor predeterminado (#4616)
 - soporte JSON en `norito_cli`
- introducir el tiempo de espera de inactividad p2p

### Cambiado

- reemplace `lol_alloc` con `dlmalloc` (#4857)
- cambie el nombre de `type_` a `type` en el esquema (#4855)
- reemplace `Duration` con `u64` en el esquema (#4841)
- utilice EnvFilter similar a `RUST_LOG` para iniciar sesión (#4837)
- mantener el bloque de votación cuando sea posible (#4828)
- migrar de warp a axum (#4718)
- modelo de datos de ejecutor dividido (#4791)
- modelo de datos superficiales (#4734) (#4792)
- no enviar clave pública con firma (#4518)
- cambiar el nombre de `--outfile` a `--out-file` (#4679)
- cambiar el nombre del servidor y cliente iroha (#4662)
- cambiar el nombre de `PermissionToken` a `Permission` (#4635)
- rechazar `BlockMessages` con entusiasmo (#4606)
- hacer que `SignedBlock` sea inmutable (#4620)
- cambiar el nombre de TransactionValue a CommittedTransaction (#4610)
- autenticar cuentas personales por ID (#4411)
- utilizar formato multihash para claves privadas (#4541)
 - cambiar el nombre de `parity_scale_decoder` a `norito_cli`
- enviar bloques a los validadores del Conjunto B
- hacer `Role` transparente (#4886)
- derivar hash de bloque del encabezado (#4890)

### Fijo- verificar que la autoridad sea propietaria del dominio para transferir (#4807)
- eliminar la inicialización doble del registrador (#4800)
- Se corrigió la convención de nomenclatura para activos y permisos (#4741)
- actualizar el ejecutor en una transacción separada en el bloque génesis (#4757)
- valor predeterminado correcto para `JsonString` (#4692)
- mejorar el mensaje de error de deserialización (#4659)
- No entre en pánico si la clave pública Ed25519Sha512 pasada no tiene una longitud válida (#4650)
- use el índice de cambio de vista adecuado en la carga del bloque de inicio (#4612)
- no ejecute prematuramente activadores de tiempo antes de su marca de tiempo `start` (#4333)
- soporte `https` para `torii_url` (#4601) (#4617)
- eliminar serde(aplanar) de SetKeyValue/RemoveKeyValue (#4547)
- el conjunto de disparadores está serializado correctamente
- revocar los `PermissionToken` eliminados en `Upgrade<Executor>` (#4503)
- informar el índice de cambio de vista correcto para la ronda actual
- eliminar los activadores correspondientes en `Unregister<Domain>` (#4461)
- comprobar la clave del pub génesis en la ronda génesis
- impedir el registro de dominio o cuenta de génesis
- eliminar permisos de roles al cancelar el registro de la entidad
- Los metadatos de activación son accesibles en contratos inteligentes.
- use el bloqueo rw para evitar una vista de estado inconsistente (#4867)
- mango de tenedor blando en instantánea (#4868)
- arreglar MinSize para ChaCha20Poly1305
- agregar límites a LiveQueryStore para evitar un uso elevado de memoria (#4893)

### Eliminado

- eliminar la clave pública de la clave privada ed25519 (#4856)
- eliminar kura.lock (#4849)
- revertir los sufijos `_ms` e `_bytes` en la configuración (#4667)
- eliminar el sufijo `_id` e `_file` de los campos de génesis (#4724)
- eliminar activos de índice en AssetsMap por AssetDefinitionId (#4701)
- eliminar el dominio de la identidad del activador (#4640)
- eliminar la firma de génesis de Iroha (#4673)
- eliminar `Visit` enlazado desde `Validate` (#4642)
- eliminar `TriggeringEventFilterBox` (#4866)
- eliminar `garbage` en el protocolo de enlace p2p (#4889)
- eliminar `committed_topology` del bloque (#4880)

### Seguridad

- proteger contra la fuga de secretos

## [2.0.0-pre-rc.21] - 2024-04-19

### Agregado

- incluir la identificación del activador en el punto de entrada del activador (#4391)
- exponer el evento establecido como campos de bits en el esquema (#4381)
- Introducir el nuevo `wsv` con acceso granular (#2664)
- agregar filtros de eventos para eventos `PermissionTokenSchemaUpdate`, `Configuration` y `Executor`
- introducir el "modo" de instantánea (#4365)
- permitir otorgar/revocar permisos de roles (#4244)
- introducir tipos numéricos de precisión arbitraria para activos (eliminar todos los demás tipos numéricos) (#3660)
- límite de combustible diferente para el Ejecutor (#3354)
- integrar el perfilador pprof (#4250)
- agregar el subcomando de activos en la CLI del cliente (#4200)
- Permisos `Register<AssetDefinition>` (#4049)
- agregue `chain_id` para evitar ataques de repetición (#4185)
- agregar subcomandos para editar los metadatos del dominio en la CLI del cliente (#4175)
- implementar operaciones de configuración, eliminación y obtención de tiendas en la CLI del cliente (#4163)
- contar contratos inteligentes idénticos para activadores (#4133)
- agregar subcomando en la CLI del cliente para transferir dominios (#3974)
- soporte de cortes en caja en FFI (#4062)
- git commit SHA a la CLI del cliente (#4042)
- Macro de proceso para el texto estándar del validador predeterminado (#3856)
- Se introdujo el generador de solicitudes de consulta en la API del cliente (#3124)
- consultas diferidas dentro de contratos inteligentes (#3929)
- Parámetro de consulta `fetch_size` (#3900)
- instrucción de transferencia de tienda de activos (#4258)
- protección contra la filtración de secretos (#3240)
- deduplicar activadores con el mismo código fuente (#4419)

### Cambiado- aumentar la cadena de herramientas de óxido a todas las noches-2024-04-18
- enviar bloques a los validadores del Conjunto B (#4387)
- dividir eventos de canalización en eventos de bloque y transacción (#4366)
- cambie el nombre de la sección de configuración `[telemetry.dev]` a `[dev_telemetry]` (#4377)
- hacer que los tipos `Action` e `Filter` no sean genéricos (#4375)
- mejorar la API de filtrado de eventos con un patrón de creación (#3068)
- unificar varias API de filtro de eventos, introducir una API de creación fluida
- cambiar el nombre de `FilterBox` a `EventFilterBox`
- cambiar el nombre de `TriggeringFilterBox` a `TriggeringEventFilterBox`
- mejorar la denominación de filtros, p. `AccountFilter` -> `AccountEventFilter`
- reescribir la configuración según el RFC de configuración (#4239)
- ocultar la estructura interna de las estructuras versionadas de la API pública (#3887)
- introducir temporalmente un orden predecible después de demasiados cambios de vista fallidos (#4263)
- utilizar tipos de clave concretos en `iroha_crypto` (#4181)
- cambios en la vista dividida de los mensajes normales (#4115)
- hacer que `SignedTransaction` sea inmutable (#4162)
- exportar `iroha_config` a través de `iroha_client` (#4147)
- exportar `iroha_crypto` a través de `iroha_client` (#4149)
- exportar `data_model` a través de `iroha_client` (#4081)
- eliminar la dependencia `openssl-sys` de `iroha_crypto` e introducir backends tls configurables en `iroha_client` (#3422)
- reemplace el EOF `hyperledger/ursa` sin mantenimiento con la solución interna `iroha_crypto` (#3422)
- optimizar el rendimiento del ejecutor (#4013)
- actualización de pares de topología (#3995)

### Fijo

- eliminar los activadores correspondientes en `Unregister<Domain>` (#4461)
- eliminar permisos de roles al cancelar el registro de una entidad (#4242)
- afirmar que la transacción de génesis está firmada por la clave de publicación de génesis (#4253)
- introducir tiempo de espera para pares que no responden en p2p (#4267)
- impedir el registro de dominio o cuenta de génesis (#4226)
- `MinSize` para `ChaCha20Poly1305` (#4395)
- iniciar la consola cuando `tokio-console` esté habilitado (#4377)
- separe cada elemento con `\n` y cree de forma recursiva directorios principales para los registros de archivos `dev-telemetry`
- impedir el registro de cuentas sin firmas (#4212)
- La generación de pares de claves ahora es infalible (#4283)
- dejar de codificar claves `X25519` como `Ed25519` (#4174)
- realizar validación de firma en `no_std` (#4270)
- llamar a métodos de bloqueo dentro del contexto asíncrono (#4211)
- revocar los tokens asociados al cancelar el registro de la entidad (#3962)
- error de bloqueo asíncrono al iniciar Sumeragi
- fijo `(get|set)_config` 401 HTTP (#4177)
- Nombre del archivador `musl` en Docker (#4193)
- impresión de depuración de contrato inteligente (#4178)
- actualización de topología al reiniciar (#4164)
- registro de nuevo par (#4142)
- orden de iteración predecible en cadena (#4130)
- rediseñar el registrador y la configuración dinámica (#4100)
- desencadenar atomicidad (#4106)
- problema de ordenación de mensajes del almacén de consultas (#4057)
- configure `Content-Type: application/x-norito` para puntos finales que respondan usando Norito

### Eliminado

- Parámetro de configuración `logger.tokio_console_address` (#4377)
- `NotificationEvent` (#4377)
- Enumeración `Value` (#4305)
- Agregación MST de iroha (#4229)
- clonación para ISI y ejecución de consultas en contratos inteligentes (#4182)
- Funciones `bridge` e `dex` (#4152)
- eventos aplanados (#3068)
- expresiones (#4089)
- referencia de configuración generada automáticamente
- Ruido `warp` en registros (#4097)

### Seguridad

- evitar la suplantación de claves de publicación en p2p (#4065)
- asegúrese de que las firmas `secp256k1` que salen de OpenSSL estén normalizadas (#4155)

## [2.0.0-pre-rc.20] - 2023-10-17

### Agregado- Transferir propiedad `Domain`
- Permisos de propietario `Domain`
- Agregue el campo `owned_by` a `Domain`
- analizar el filtro como JSON5 en `iroha_client_cli` (#3923)
- Se agregó soporte para el uso de tipo propio en enumeraciones serde parcialmente etiquetadas.
- Estandarizar API de bloque (#3884)
- Implementar el modo de inicio kura `Fast`
- Agregar encabezado de descargo de responsabilidad de iroha_swarm
- soporte inicial para instantáneas WSV

### Fijo

- Se corrigió la descarga del ejecutor en update_configs.sh (#3990)
- Rustc adecuado en devShell
- Se corrigió la grabación de repeticiones `Trigger`.
- Arreglar transferencia `AssetDefinition`
- Reparar `RemoveKeyValue` para `Domain`
- Corregir el uso de `Span::join`
- Se corrigió el error de falta de coincidencia de topología (#3903)
- Se corrigió el punto de referencia `apply_blocks` y `validate_blocks`.
- `mkdir -r` con ruta de almacenamiento, sin ruta de bloqueo (#3908)
- No fallar si el directorio existe en test_env.py
- Se corrigió la cadena de documentos de autenticación/autorización (#3876)
- Mejor mensaje de error para la consulta de error de búsqueda
- Agregar la clave pública de la cuenta de génesis a la ventana acoplable de desarrollo.
- Comparar la carga útil del token de permiso como JSON (#3855)
- Arreglar `irrefutable_let_patterns` en la macro `#[model]`
- Permitir que Génesis ejecute cualquier ISI (#3850)
- Corregir la validación de génesis (#3844)
- Arreglar topología para 3 o menos pares
- Corregir cómo se calcula el histograma tx_amounts.
- Prueba de descamación `genesis_transactions_are_validated()`
- Generación de validador predeterminado
- Arreglar el apagado elegante de iroha.

### Refactorizar- eliminar dependencias no utilizadas (#3992)
- aumentar las dependencias (#3981)
- Cambiar el nombre del validador a ejecutor (#3976)
- Quitar `IsAssetDefinitionOwner` (#3979)
- Incluir código de contrato inteligente en el espacio de trabajo (#3944)
- Fusionar puntos finales de API y telemetría en un solo servidor
- mover la expresión len de la API pública al núcleo (#3949)
- Evitar clonar en la búsqueda de roles.
- Consultas de rango para roles.
- Mover roles de cuenta a `WSV`
- Cambiar el nombre de ISI de *Box a *Expr (#3930)
- Eliminar el prefijo 'Versionado' de los contenedores versionados (#3913)
- mover `commit_topology` a la carga útil del bloque (#3916)
- Migrar la macro `telemetry_future` a syn 2.0
- Registrado con Identificable en los límites de ISI (#3925)
- Agregar soporte genérico básico a `derive(HasOrigin)`
- Limpiar la documentación de las API de Emitter para hacer feliz a clippy
- Agregar pruebas para la macro derivar (HasOrigin), reducir la repetición en derivar (IdEqOrdHash), corregir el informe de errores en estable
- Mejore los nombres, simplifique los .filter_maps repetidos y elimine los .except innecesarios en derivar (Filtro)
- Hacer que PartiallyTaggedSerialize/Deserialize use cariño
- Hacer derivar (IdEqOrdHash) usar querido, agregar pruebas
- Hacer derivar (Filtro) usar cariño
- Actualización iroha_data_model_derive para usar syn 2.0
- Agregar pruebas unitarias de condición de verificación de firma
- Permitir sólo un conjunto fijo de condiciones de verificación de firma.
- Generalizar ConstBytes en un ConstVec que contenga cualquier secuencia constante
- Utilice una representación más eficiente para los valores de bytes que no cambian
- Almacenar wsv finalizado en una instantánea
- Agregar actor `SnapshotMaker`
- La limitación del análisis del documento se deriva de las macros de proceso.
- limpiar comentarios
- extraer una utilidad de prueba común para analizar atributos en lib.rs
- use parse_display y actualice Attr -> nombres de atributos
- permitir el uso de coincidencia de patrones en los argumentos de la función ffi
- reducir la repetición en el análisis de atributos getset
- cambiar el nombre de Emitter::into_token_stream a Emitter::finish_token_stream
- Utilice parse_display para analizar tokens getset
- Corregir errores tipográficos y mejorar los mensajes de error.
- iroha_ffi_derive: usa darling para analizar atributos y usa syn 2.0
- iroha_ffi_derive: reemplaza proc-macro-error con manyhow
- Simplifica el código del archivo de bloqueo de kura
- hacer que todos los valores numéricos se serialicen como cadenas literales
- Separar Kagami (#3841)
- Reescribir `scripts/test-env.sh`
- Diferenciar entre contratos inteligentes y puntos de entrada de activación
- Elide `.cloned()` en `data_model/src/block.rs`
- actualice `iroha_schema_derive` para usar syn 2.0

## [2.0.0-pre-rc.19] - 2023-08-14

### Agregado- Hyperledger#3309 Mejora el tiempo de ejecución de IVM para mejorar
- hyperledger#3383 Implementar macro para analizar direcciones de socket en tiempo de compilación
- hyperledger#2398 Agregar pruebas de integración para filtros de consulta
- Incluir el mensaje de error real en `InternalError`
- Uso de `nightly-2023-06-25` como cadena de herramientas predeterminada
- hyperledger#3692 Migración del validador
- [Pasantía DSL] hyperledger#3688: Implementar aritmética básica como macro de proceso
- hyperledger#3371 Validador dividido `entrypoint` para garantizar que los validadores ya no sean vistos como contratos inteligentes
- Instantáneas WSV de Hyperledger#3651, que permiten abrir un nodo Iroha rápidamente después de una falla
- hyperledger#3752 Reemplace `MockValidator` con un validador `Initial` que acepte todas las transacciones
- hyperledger#3276 Agregue una instrucción temporal llamada `Log` que registra una cadena especificada en el registro principal del nodo Iroha
- hyperledger#3641 Hacer que la carga útil del token de permiso sea legible para humanos
- hyperledger#3324 Agregar comprobaciones y refactorización `burn` relacionadas con `iroha_client_cli`
- hyperledger#3781 Validar transacciones de génesis
- hyperledger#2885 Diferenciar entre eventos que pueden y no usarse como desencadenantes
- hyperledger#2245 Compilación basada en `Nix` del binario del nodo iroha como `AppImage`

### Fijo

- Hyperledger#3613 Regresión que podría permitir que se acepten transacciones firmadas incorrectamente
- Rechazar la topología de configuración incorrecta temprano
- hyperledger#3445 Corregir la regresión y hacer que `POST` en el punto final `/configuration` vuelva a funcionar
- hyperledger#3654 Se corrige `iroha2` `Dockerfiles` basado en `glibc` que se implementará
- hyperledger#3451 Corrección de compilación `docker` en Macs de silicio de Apple
- hyperledger#3741 Soluciona el error `tempfile` en `kagami validator`
- hyperledger#3758 Se corrigió la regresión donde no se podían construir cajas individuales, pero se podían construir como parte del espacio de trabajo.
- Hyperledger#3777 La laguna del parche en el registro de roles no se está validando
- hyperledger#3805 Corrección Iroha que no se apaga después de recibir `SIGTERM`

### Otro

- hyperledger#3648 Incluye verificación `docker-compose.*.yml` en los procesos de CI
- Mueva la instrucción `len()` de `iroha_data_model` a `iroha_core`
- hyperledger#3672 Reemplace `HashMap` con `FxHashMap` en macros de derivación
- hyperledger#3374 Unificar comentarios de documento de error e implementación `fmt::Display`
- hyperledger#3289 Utilice la herencia del espacio de trabajo de Rust 1.70 en todo el proyecto
- hyperledger#3654 Agregue `Dockerfiles` para compilar iroha2 en `GNU libc <https://www.gnu.org/software/libc/>`_
- Introducir `syn` 2.0, `manyhow` e `darling` para proc-macros
- semilla hyperledger#3802 Unicode `kagami crypto`

## [2.0.0-pre-rc.18]

### Agregado

- hyperledger#3468: Cursor del lado del servidor, que permite una paginación reentrante evaluada de forma diferida, lo que debería tener importantes implicaciones positivas en el rendimiento para la latencia de las consultas.
- hyperledger#3624: tokens de permiso de uso general; específicamente
  - Los tokens de permisos pueden tener cualquier estructura.
  - La estructura del token se describe automáticamente en `iroha_schema` y se serializa como una cadena JSON.
  - El valor del token está codificado en `Norito`.
  - como consecuencia de este cambio, la convención de nomenclatura del token de permiso se trasladó de `snake_case` a `UpeerCamelCase`.
- hyperledger#3615 Preservar wsv después de la validación

### Fijo- hyperledger#3627 La atomicidad de las transacciones ahora se aplica mediante la clonación del `WorlStateView`
- hyperledger#3195 Ampliar el comportamiento de pánico cuando se recibe una transacción de génesis rechazada
- hyperledger#3042 Corregir mensaje de solicitud incorrecta
- hyperledger#3352 Divida el flujo de control y el mensaje de datos en canales separados
- hyperledger#3543 Mejora la precisión de las métricas

## 2.0.0-pre-rc.17

### Agregado

- hyperledger#3330 Ampliar la deserialización `NumericValue`
- soporte de hyperledger#2622 `u128`/`i128` en FFI
- hyperledger#3088 Introducir limitación de cola para evitar DoS
- variantes de comando hyperledger#2373 `kagami swarm file` y `kagami swarm dir` para generar archivos `docker-compose`
- Hyperledger#3597 Análisis de token de permiso (lado Iroha)
- hyperledger#3353 Elimina `eyre` de `block.rs` enumerando condiciones de error y usando errores fuertemente tipados
- hyperledger#3318 Intercalar transacciones rechazadas y aceptadas en bloques para preservar el orden de procesamiento de las transacciones

### Fijo

- hyperledger#3075 Pánico en transacción no válida en `genesis.json` para evitar que se procesen transacciones no válidas
- hyperledger#3461 Manejo adecuado de los valores predeterminados en la configuración predeterminada
- hyperledger#3548 Arreglar el atributo transparente `IntoSchema`
- hyperledger#3552 Arreglar la representación del esquema de ruta del validador
- hyperledger#3546 Solución para que los activadores de tiempo se atasquen
- hyperledger#3162 Prohibir altura 0 en solicitudes de transmisión en bloque
- Prueba inicial de macro de configuración
- Hyperledger#3592 Solución para archivos de configuración que se actualizan en `release`
- hyperledger#3246 No involucres a `Set B validators <https://github.com/hyperledger-iroha/iroha/blob/main/docs/source/iroha_2_whitepaper.md#2-system-architecture>`_ sin `fault <https://en.wikipedia.org/wiki/Byzantine_fault>`_
- hyperledger#3570 Muestra correctamente los errores de consulta de cadenas del lado del cliente
- hyperledger#3596 `iroha_client_cli` muestra bloques/eventos
- hyperledger#3473 Hacer que `kagami validator` funcione desde fuera del directorio raíz del repositorio de iroha

### Otro

- hyperledger#3063 Asigna la transacción `hash` a la altura del bloque en `wsv`
- `HashOf<T>` fuertemente tipado en `Value`

## [2.0.0-pre-rc.16]

### Agregado

- subcomando hyperledger#2373 `kagami swarm` para generar `docker-compose.yml`
- hyperledger#3525 Estandarizar API de transacciones
- hyperledger#3376 Agregar Iroha Client CLI `pytest <https://docs.pytest.org/en/7.4.x/>`_ marco de automatización
- hyperledger#3516 Conservar el hash del blob original en `LoadedExecutable`

### Fijo

- hyperledger#3462 Agregue el comando de activo `burn` a `client_cli`
- hyperledger#3233 Tipos de errores de refactorización
- hyperledger#3330 Se corrigió la regresión, implementando manualmente `serde::de::Deserialize` para `partially-tagged <https://serde.rs/enum-representations.html>`_ `enums`
- hyperledger#3487 Devuelve los tipos que faltan en el esquema
- hyperledger#3444 Devuelve el discriminante al esquema
- hyperledger#3496 Corrección del análisis de campos `SocketAddr`
- hyperledger#3498 Arreglar la detección de bifurcación suave
- hyperledger#3396 Almacenar bloque en `kura` antes de emitir un evento de bloque confirmado

### Otro

- hyperledger#2817 Elimina la mutabilidad interior de `WorldStateView`
- hiperledger#3363 Refactorización de la API de Génesis
- Refactorizar las pruebas existentes y complementarlas con nuevas pruebas de topología.
- Cambie de `Codecov <https://about.codecov.io/>`_ a `Coveralls <https://coveralls.io/>`_ para cobertura de prueba
- hyperledger#3533 Cambie el nombre de `Bool` a `bool` en el esquema

## [2.0.0-pre-rc.15]

### Agregado- hyperledger#3231 Validador monolítico
- hyperledger#3015 Soporte para optimización de nichos en FFI
- hyperledger#2547 Agregar logotipo a `AssetDefinition`
- hyperledger#3274 Agregue a `kagami` un subcomando que genera ejemplos (con respaldo a LTS)
- hiperledger#3415 `Nix <https://nixos.wiki/wiki/Flakes>`_ escama
- hyperledger#3412 Mover los chismes de transacciones a un actor separado
- hyperledger#3435 Presentar al visitante `Expression`
- hyperledger#3168 Proporcionar el validador de génesis como un archivo separado
- hyperledger#3454 Hacer que LTS sea el predeterminado para la mayoría de las operaciones y documentación de Docker
- hyperledger#3090 Propagar parámetros en cadena desde blockchain a `sumeragi`

### Fijo

- hyperledger#3330 Se corrigió la deserialización de enumeración sin etiquetar con hojas `u128` (reportada a RC14)
- Hyperledger#2581 redujo el ruido en los registros.
- hyperledger#3360 Arreglar el punto de referencia `tx/s`
- hyperledger#3393 Romper el bucle de interbloqueo de comunicación en `actors`
- hyperledger#3402 Arreglar la compilación `nightly`
- hyperledger#3411 Manejar adecuadamente la conexión simultánea entre pares
- hyperledger#3440 Desaprobar las conversiones de activos durante la transferencia, en su lugar manejadas por contratos inteligentes
- hyperledger#3408: Arreglar la prueba `public_keys_cannot_be_burned_to_nothing`

### Otro

- hyperledger#3362 Migrar a actores `tokio`
- hyperledger#3349 Elimina `EvaluateOnHost` de contratos inteligentes
- hyperledger#1786 Agregue tipos nativos `iroha` para direcciones de socket
- Deshabilitar caché IVM
- Vuelva a habilitar la caché IVM
- Cambiar el nombre del validador de permisos a validador
- hyperledger#3388 Convertir `model!` en una macro de atributos a nivel de módulo
- hyperledger#3370 Serializa `hash` como cadena hexadecimal
- Mover `maximum_transactions_in_block` de la configuración `queue` a `sumeragi`
- Desaprobar y eliminar el tipo `AssetDefinitionEntry`
- Cambie el nombre de `configs/client_cli` a `configs/client`
- Actualización `MAINTAINERS.md`

## [2.0.0-pre-rc.14]

### Agregado

- modelo de datos hyperledger#3127 `structs` opaco por defecto
- Hyperledger#3122 usa `Algorithm` para almacenar la función de resumen (colaborador de la comunidad)
- La salida de Hyperledger#3153 `iroha_client_cli` es legible por máquina
- hyperledger#3105 Implementar `Transfer` para `AssetDefinition`
- Se agregó el evento de canalización de caducidad de Hyperledger#3010 `Transaction`.

### Fijo

- revisión de hyperledger#3113 de pruebas de red inestable
- hyperledger#3129 Corrección de deserializacion `Parameter`
- hyperledger#3141 Implementar manualmente `IntoSchema` para `Hash`
- hyperledger#3155 Se corrigió el gancho de pánico en las pruebas, evitando el punto muerto
- hyperledger#3166 No ver cambios en estado inactivo, lo que mejora el rendimiento
- hyperledger#2123 Volver a la deseriaización/deseriaización de PublicKey desde multihash
- hyperledger#3132 Agregar validador de nuevo parámetro
- hyperledger#3249 Dividir hashes de bloques en versiones parciales y completas
- hyperledger#3031 Corrige la UI/UX de los parámetros de configuración faltantes
- Hyperledger#3247 Se eliminó la inyección de fallas de `sumeragi`.

### Otro

- Agregue `#[cfg(debug_assertions)]` faltante para corregir fallas espurias
- hyperledger#2133 Reescribir la topología para estar más cerca del documento técnico
- Eliminar la dependencia `iroha_client` de `iroha_core`
- hiperledger#2943 Derivar `HasOrigin`
- hyperledger#3232 Compartir metadatos del espacio de trabajo
- hiperledger#3254 Refactor `commit_block()` y `replace_top_block()`
- Utilice un controlador de asignación predeterminado estable
- hyperledger#3183 Cambie el nombre de los archivos `docker-compose.yml`
- Se mejoró el formato de visualización `Multihash`.
- hyperledger#3268 Identificadores de artículos únicos a nivel mundial
- Nueva plantilla de relaciones públicas

## [2.0.0-pre-rc.13]

### Agregado- hyperledger#2399 Configurar parámetros como ISI.
- hyperledger#3119 Agregar métrica `dropped_messages`.
- hyperledger#3094 Generar red con pares `n`.
- hyperledger#3082 Proporcionar datos completos en el evento `Created`.
- hyperledger#3021 Importación de puntero opaco.
- hyperledger#2794 Rechazar enumeraciones sin campo con discriminadores explícitos en FFI.
- hyperledger#2922 Agregue `Grant<Role>` al génesis predeterminado.
- hyperledger#2922 Omitir el campo `inner` en la deserialización json `NewRole`.
- hyperledger#2922 Omitir `object(_id)` en la deserialización json.
- hyperledger#2922 Omitir `Id` en la deserialización json.
- hyperledger#2922 Omitir `Identifiable` en la deserialización json.
- hyperledger#2963 Agregue `queue_size` a las métricas.
- Hyperledger#3027 implementa el archivo de bloqueo para Kura.
- hyperledger#2813 Kagami genera la configuración de pares predeterminada.
- hiperledger#3019 Soporte JSON5.
- hyperledger#2231 Generar API contenedora de FFI.
- hyperledger#2999 Acumular firmas de bloque.
- Hyperledger#2995 Detección de bifurcación suave.
- hyperledger#2905 Ampliar las operaciones aritméticas para admitir `NumericValue`
- hyperledger#2868 Emite la versión iroha y confirma el hash en los registros.
- hyperledger#2096 Consulta por el importe total del activo.
- hyperledger#2899 Agregue el subcomando de instrucciones múltiples en 'client_cli'
- hyperledger#2247 Elimina el ruido de comunicación de websocket.
- hyperledger#2889 Agregar soporte de transmisión en bloque en `iroha_client`
- hyperledger#2280 Produce eventos de permiso cuando se otorga o revoca una función.
- hyperledger#2797 Enriquecer eventos.
- hyperledger#2725 Reintroducir el tiempo de espera en `submit_transaction_blocking`
- hyperledger#2712 Pruebas de configuración de configuración.
- Hyperledger#2491 Compatibilidad con enumeraciones en FFi.
- hyperledger#2775 Genera diferentes claves en génesis sintética.
- Hyperledger#2627 Finalización de configuración, punto de entrada de proxy, kagami docgen.
- hyperledger#2765 Generar génesis sintética en `kagami`
- hyperledger#2698 Corregir mensaje de error poco claro en `iroha_client`
- hyperledger#2689 Agregar parámetros de definición de token de permiso.
- hyperledger#2502 Almacenar hash GIT de compilación.
- hyperledger#2672 Agregue la variante y los predicados `ipv4Addr`, `ipv6Addr`.
- hyperledger#2626 Implementar derivación `Combine`, dividir macros `config`.
- hyperledger#2586 `Builder` e `LoadFromEnv` para estructuras proxy.
- hyperledger#2611 Derive `TryFromReprC` e `IntoFfi` para estructuras opacas genéricas.
- hyperledger#2587 Divida `Configurable` en dos rasgos. #2587: Divida `Configurable` en dos rasgos
- hyperledger#2488 Agregar soporte para implicaciones de rasgos en `ffi_export`
- hyperledger#2553 Agregar clasificación a las consultas de activos.
- hyperledger#2407 Activadores de parametrización.
- hyperledger#2536 Introducir `ffi_import` para clientes FFI.
- hyperledger#2338 Agregar instrumentación `cargo-all-features`.
- Opciones del algoritmo de la herramienta Hyperledger#2564 Kagami.
- hyperledger#2490 Implementar ffi_export para funciones independientes.
- hyperledger#1891 Validar la ejecución del disparador.
- hyperledger#1988 Derivar macros para Identificable, Eq, Hash, Ord.
- biblioteca bindgen hyperledger#2434 FFI.
- hyperledger#2073 Prefiera ConstString a String para tipos en blockchain.
- hyperledger#1889 Agregar activadores de ámbito de dominio.
- hyperledger#2098 Consultas de encabezado de bloque. #2098: agregar consultas de encabezado de bloque
- hyperledger#2467 Agregar el subcomando de concesión de cuenta en iroha_client_cli.
- hyperledger#2301 Agregue el hash de bloque de la transacción al consultarla.
 - hyperledger#2454 Agregue un script de compilación a la herramienta decodificadora Norito.
- hyperledger#2061 Derivar macro para filtros.- hyperledger#2228 Agregar variante no autorizada al error de consulta de contratos inteligentes.
- hyperledger#2395 Añade pánico si no se puede aplicar la génesis.
- hyperledger#2000 No permitir nombres vacíos. #2000: No permitir nombres vacíos
 - hyperledger#2127 Agregue una verificación de integridad para garantizar que se consuman todos los datos decodificados por el códec Norito.
- hyperledger#2360 Haga que `genesis.json` sea opcional nuevamente.
- hyperledger#2053 Agregue pruebas a todas las consultas restantes en la cadena de bloques privada.
- hyperledger#2381 Unificar el registro `Role`.
- hyperledger#2053 Agregar pruebas a las consultas relacionadas con activos en la cadena de bloques privada.
- hyperledger#2053 Agregar pruebas a 'private_blockchain'
- hyperledger#2302 Agregue la consulta auxiliar 'FindTriggersByDomainId'.
- hyperledger#1998 Agregar filtros a las consultas.
- hyperledger#2276 Incluye el hash del bloque actual en BlockHeaderValue.
- hyperledger#2161 Identificador de identificador y fns de FFI compartidos.
- agregar ID de identificador e implementar equivalentes FFI de rasgos compartidos (Clone, Eq, Ord)
- hyperledger#1638 `configuration` subárbol de documentos de retorno.
- hyperledger#2132 Agregar macro de proceso `endpointN`.
- hyperledger#2257 Revocar<Rol> emite el evento RoleRevoked.
- hyperledger#2125 Agregar consulta FindAssetDefinitionById.
- hyperledger#1926 Agregue manejo de señales y apagado elegante.
- hyperledger#2161 genera funciones FFI para `data_model`
- hyperledger#1149 El recuento de archivos bloqueados no supera los 1.000.000 por directorio.
- hyperledger#1413 Agregar punto final de versión API.
- Hyperledger#2103 admite consultas de bloques y transacciones. Agregar consulta `FindAllTransactions`
- hyperledger#2186 Agregar transferencia ISI para `BigQuantity` e `Fixed`.
- hyperledger#2056 Agregue una caja de macros de proceso de derivación para `AssetValueType` `enum`.
- hyperledger#2100 Agregar consulta para encontrar todas las cuentas con activos.
- hyperledger#2179 Optimizar la ejecución del disparador.
- hyperledger#1883 Elimina los archivos de configuración integrados.
- Hyperledger#2105 maneja errores de consulta en el cliente.
- hyperledger#2050 Agregar consultas relacionadas con roles.
- hyperledger#1572 Tokens de permiso especializados.
- hyperledger#2121 Verificar que el par de claves sea válido cuando se construye.
 - hyperledger#2003 Introducir la herramienta decodificadora Norito.
- hyperledger#1952 Agregue un punto de referencia TPS como estándar para optimizaciones.
- hyperledger#2040 Agregar prueba de integración con límite de ejecución de transacciones.
- hyperledger#1890 Introducir pruebas de integración basadas en casos de uso de Orillion.
- hyperledger#2048 Agregar archivo de cadena de herramientas.
- hyperledger#2100 Agregar consulta para encontrar todas las cuentas con activos.
- hyperledger#2179 Optimizar la ejecución del disparador.
- hyperledger#1883 Elimina los archivos de configuración integrados.
- hyperledger#2004 Prohibir que `isize` e `usize` se conviertan en `IntoSchema`.
- Hyperledger#2105 maneja errores de consulta en el cliente.
- hyperledger#2050 Agregar consultas relacionadas con roles.
- hyperledger#1572 Tokens de permiso especializados.
- hyperledger#2121 Verificar que el par de claves sea válido cuando se construye.
 - hyperledger#2003 Introducir la herramienta decodificadora Norito.
- hyperledger#1952 Agregue un punto de referencia TPS como estándar para optimizaciones.
- hyperledger#2040 Agregar prueba de integración con límite de ejecución de transacciones.
- hyperledger#1890 Introducir pruebas de integración basadas en casos de uso de Orillion.
- hyperledger#2048 Agregar archivo de cadena de herramientas.
- hyperledger#2037 Introducir activadores de confirmación previa.
- hyperledger#1621 Introducir activadores por llamada.
- hyperledger#1970 Agregar punto final de esquema opcional.
- hyperledger#1620 Introducir activadores basados ​​en el tiempo.
- hyperledger#1918 Implementar autenticación básica para `client`
- hyperledger#1726 Implementar un flujo de trabajo de relaciones públicas de lanzamiento.
- hyperledger#1815 Hacer que las respuestas a las consultas estén más estructuradas por tipo.- Hyperledger#1928 implementa la generación de registros de cambios usando `gitchangelog`
- Hyperledger#1902 Script de configuración de 4 pares sin sistema operativo.

  Se agregó una versión de setup_test_env.sh que no requiere docker-compose y usa la compilación de depuración de Iroha.
- hyperledger#1619 Introducir desencadenadores basados ​​en eventos.
- hyperledger#1195 Cerrar una conexión websocket limpiamente.
- hyperledger#1606 Agregue un enlace ipfs al logotipo del dominio en la estructura del dominio.
- hyperledger#1754 Agregar CLI del inspector Kura.
- hyperledger#1790 Mejorar el rendimiento mediante el uso de vectores basados ​​en pila.
- hyperledger#1805 Colores de terminal opcionales para errores de pánico.
- hiperledger#1749 `no_std` en `data_model`
- hyperledger#1179 Agregar instrucción de revocación de permiso o rol.
- hyperledger#1782 hace que iroha_crypto no_std sea compatible.
- hyperledger#1172 Implementar eventos de instrucción.
- hyperledger#1734 Validar `Name` para excluir espacios en blanco.
- hyperledger#1144 Agregar anidamiento de metadatos.
- #1210 Bloquear streaming (lado del servidor).
- hyperledger#1331 Implementar más métricas `Prometheus`.
- hyperledger#1689 Corregir dependencias de funciones. #1261: Agregue hinchazón de carga.
- Hyperledger#1675 usa tipo en lugar de estructura contenedora para elementos versionados.
- hyperledger#1643 Espere a que los pares confirmen la génesis en las pruebas.
- hiperledger#1678 `try_allocate`
- hyperledger#1216 Agregar punto final Prometheus. #1216: implementación inicial del punto final de métricas.
- hyperledger#1238 Actualizaciones a nivel de registro en tiempo de ejecución. Se creó la recarga básica basada en el punto de entrada `connection`.
- formato de título de hyperledger#1652 PR.
- Agregue el número de pares conectados a `Status`

  - Revertir "Eliminar cosas relacionadas con la cantidad de pares conectados"

  Esto revierte la confirmación b228b41dab3c035ce9973b6aa3b35d443c082544.
  - Aclarar que `Peer` tiene una clave pública verdadera solo después del protocolo de enlace
  - `DisconnectPeer` sin pruebas
  - Implementar la ejecución de pares para cancelar el registro.
  - Agregar el subcomando (des)registrar pares a `client_cli`
  - Rechazar reconexiones de un par no registrado por su dirección

  Después de que su par se dé de baja y desconecte a otro par,
  su red escuchará las solicitudes de reconexión del par.
  Lo único que puedes saber al principio es la dirección cuyo número de puerto es arbitrario.
  Así que recuerde al par no registrado por la parte que no sea el número de puerto.
  y rechazar la reconexión desde allí
- Agregue el punto final `/status` a un puerto específico.

### Correcciones- hyperledger#3129 Corrección de des/serialización `Parameter`.
- hyperledger#3109 Impide el modo de suspensión `sumeragi` después de un mensaje independiente del rol.
- hyperledger#3046 Asegúrese de que Iroha pueda iniciar correctamente en vacío
  `./storage`
- hyperledger#2599 Elimina las pelusas del vivero.
- hyperledger#3087 Recoge votos de los validadores del Conjunto B después del cambio de vista.
- hyperledger#3056 Se corrigió el bloqueo del punto de referencia `tps-dev`.
- hyperledger#1170 Implementar el manejo de soft-fork estilo clonación-wsv.
- hyperledger#2456 Haz que el bloque de génesis sea ilimitado.
- hyperledger#3038 Vuelva a habilitar multifirma.
- hyperledger#2894 Corrige la deserialización de la variable env `LOG_FILE_PATH`.
- hyperledger#2803 Devuelve el código de estado correcto para errores de firma.
- hyperledger#2963 `Queue` elimina las transacciones correctamente.
- hyperledger#0000 Vergen rompiendo CI.
- hyperledger#2165 Elimina la inquietud de la cadena de herramientas.
- hyperledger#2506 Arreglar la validación del bloque.
- hyperledger#3013 Validadores de grabación en cadena adecuados.
- hyperledger#2998 Elimina el código de cadena no utilizado.
- hyperledger#2816 Traslada la responsabilidad del acceso a los bloques a kura.
- hyperledger#2384 Reemplazar decode con decode_all.
- hyperledger#1967 Reemplace ValueName con Name.
- hyperledger#2980 Corregir valor de bloque tipo ffi.
- hyperledger#2858 Introduce parking_lot::Mutex en lugar de std.
- hyperledger#2850 Se corrigió la deserialización/decodificación de `Fixed`
- hyperledger#2923 Devuelve `FindError` cuando `AssetDefinition` no lo hace
  existir.
- hiperledger#0000 Corrección `panic_on_invalid_genesis.sh`
- hyperledger#2880 Cierre la conexión websocket correctamente.
- hyperledger#2880 Arreglar la transmisión en bloque.
- hyperledger#2804 `iroha_client_cli` enviar bloqueo de transacciones.
- hyperledger#2819 Sacar miembros no esenciales de WSV.
- Se corrigió el error de recursividad de serialización de expresiones.
- hyperledger#2834 Mejorar la sintaxis taquigráfica.
- hyperledger#2379 Agrega la capacidad de volcar nuevos bloques Kura a blocks.txt.
- hyperledger#2758 Agregar estructura de clasificación al esquema.
-CI.
- hyperledger#2548 Advertencia sobre un archivo génesis grande.
- hyperledger#2638 Actualiza `whitepaper` y propaga los cambios.
- hyperledger#2678 Se corrigieron las pruebas en la rama provisional.
- hyperledger#2678 Se corrigió la interrupción de las pruebas al forzar el cierre de Kura.
- hyperledger#2607 Refactor del código sumeragi para mayor simplicidad y
  correcciones de robustez.
- hyperledger#2561 Reintroducir cambios de vista al consenso.
- hyperledger#2560 Agregar nuevamente block_sync y desconexión de pares.
- hyperledger#2559 Agregar cierre del hilo sumeragi.
- hyperledger#2558 Validar génesis antes de actualizar el wsv desde kura.
- hyperledger#2465 Reimplementar el nodo sumeragi como estado de subproceso único
  máquina.
- hyperledger#2449 Implementación inicial de la reestructuración Sumeragi.
- hyperledger#2802 Se corrigió la carga del entorno para la configuración.
- hyperledger#2787 Notificar a todos los oyentes que se apaguen en caso de pánico.
- hyperledger#2764 Eliminar el límite del tamaño máximo de mensaje.
- #2571: Mejor experiencia de usuario de Kura Inspector.
- hyperledger#2703 Corrige errores del entorno de desarrollo de Orillion.
- Corregir error tipográfico en un comentario de documento en esquema/src.
- hyperledger#2716 Hacer pública la duración del tiempo de actividad.
- hyperledger#2700 Exportar `KURA_BLOCK_STORE_PATH` en imágenes acoplables.
- hyperledger#0 Elimina `/iroha/rust-toolchain.toml` del constructor
  imagen.
- hiperledger#0 Corrección `docker-compose-single.yml`
- hyperledger#2554 Genera error si la semilla `secp256k1` es inferior a 32
  bytes.
- hyperledger#0 Modifique `test_env.sh` para asignar almacenamiento para cada par.
- hyperledger#2457 Cerrar kura por la fuerza en las pruebas.
- hyperledger#2623 Corrección de prueba de documentación para VariantCount.
- Actualizar un error esperado en las pruebas ui_fail.
- Corregir comentario de documento incorrecto en validadores de permisos.- hyperledger#2422 Ocultar claves privadas en la respuesta del punto final de configuración.
- hyperledger#2492: Se corrigió que no todos los activadores ejecutados coincidieran con un evento.
- hyperledger#2504 Se corrigió el punto de referencia tps fallido.
- hyperledger#2477 Se corrigió el error cuando no se contaban los permisos de los roles.
- hyperledger#2416 Arreglar pelusas en el brazo de macOS.
- hyperledger#2457 Se corrigió la debilidad de las pruebas relacionadas con el cierre por pánico.
  #2457: Agregar apagado en configuración de pánico
- hyperledger#2473 analiza rustc --version en lugar de RUSTUP_TOOLCHAIN.
- hyperledger#1480 Apagado por pánico. #1480: Agregar gancho de pánico para salir del programa en caso de pánico
- hyperledger#2376 Kura simplificado, sin asíncrono, dos archivos.
- Error de compilación de Hyperledger#0000 Docker.
- hyperledger#1649 elimina `spawn` de `do_send`
- hyperledger#2128 Se corrigió la construcción e iteración de `MerkleTree`.
- hyperledger#2137 Preparar pruebas para contexto multiproceso.
- hyperledger#2227 Implementar el registro y la cancelación del registro de activos.
- hyperledger#2081 Se corrigió el error de concesión de roles.
- hyperledger#2358 Agregar versión con perfil de depuración.
- hyperledger#2294 Agrega generación de flamegraph a oneshot.rs.
- hyperledger#2202 Se corrigió el campo total en la respuesta a la consulta.
- hyperledger#2081 Se corrigió el caso de prueba para otorgar el rol.
- hyperledger#2017 Se corrigió la cancelación del registro de roles.
- hyperledger#2303 Arreglar que los pares de Docker-compose no se cierran correctamente.
- hyperledger#2295 Se corrigió el error que activaba la cancelación del registro.
- Hyperledger#2282 mejora la FFI derivada de la implementación de getset.
- hyperledger#1149 Elimina el código nocheckin.
- hyperledger#2232 Haga que Iroha imprima un mensaje significativo cuando génesis tenga demasiadas isi.
- hyperledger#2170 Se corrigió el contenedor acoplable integrado en las máquinas M1.
- hyperledger#2215 Hacer que el 20/04/2022 sea nocturno opcional para `cargo build`
- hyperledger#1990 Habilite el inicio de pares a través de env vars en ausencia de config.json.
- hyperledger#2081 Arreglar el registro de roles.
- hyperledger#1640 Generar config.json y genesis.json.
- hyperledger#1716 Se corrigió el error de consenso con f=0 casos.
- hyperledger#1845 Los activos no acuñables solo se pueden acuñar una vez.
- hyperledger#2005 Corrección `Client::listen_for_events()` que no cierra la transmisión de WebSocket.
- hyperledger#1623 Crea un RawGenesisBlockBuilder.
- hyperledger#1917 Agregar macro easy_from_str_impl.
- hyperledger#1990 Habilite el inicio de pares a través de env vars en ausencia de config.json.
- hyperledger#2081 Arreglar el registro de roles.
- hyperledger#1640 Generar config.json y genesis.json.
- hyperledger#1716 Se corrigió el error de consenso con f=0 casos.
- hyperledger#1845 Los activos no acuñables solo se pueden acuñar una vez.
- hyperledger#2005 Corrección `Client::listen_for_events()` que no cierra la transmisión de WebSocket.
- hyperledger#1623 Crea un RawGenesisBlockBuilder.
- hyperledger#1917 Agregar macro easy_from_str_impl.
- hyperledger#1922 Mover crypto_cli a herramientas.
- hyperledger#1969 Haga que la función `roles` forme parte del conjunto de funciones predeterminado.
- argumentos de CLI de revisión de hyperledger#2013.
- hyperledger#1897 Eliminar usize/isize de la serialización.
- hyperledger#1955 Se corrigió la posibilidad de pasar `:` dentro de `web_login`
- hyperledger#1943 Agregar errores de consulta al esquema.
- hyperledger#1939 Funciones adecuadas para `iroha_config_derive`.
- Hyperledger#1908 corrige el manejo de valor cero para el script de análisis de telemetría.
- hyperledger#0000 Hacer que la prueba de documento ignorada implícitamente se ignore explícitamente.
- hyperledger#1848 Evita que las claves públicas se quemen hasta quedar reducidas a nada.
- Hyperledger#1811 agregó pruebas y comprobaciones para desduplicar claves de pares confiables.
- hyperledger#1821 agrega IntoSchema para MerkleTree y VersionedValidBlock, corrige los esquemas HashOf y SignatureOf.- hyperledger#1819 Eliminar el rastreo del informe de errores en la validación.
- Hyperledger#1774 registra el motivo exacto de los errores de validación.
- hyperledger#1714 Compara PeerId solo por clave.
- hyperledger#1788 Reducir el uso de memoria de `Value`.
- Hyperledger#1804 corrige la generación de esquemas para HashOf, SignatureOf, agrega prueba para garantizar que no falten esquemas.
- Hyperledger#1802 Mejoras en la legibilidad del registro.
  - el registro de eventos se movió al nivel de seguimiento
  - ctx eliminado de la captura de registros
  - los colores del terminal se vuelven opcionales (para una mejor salida de registros a archivos)
- hyperledger#1783 Se corrigió el punto de referencia torii.
- Hyperledger#1772 Corrección después del #1764.
- Hyperledger#1755 Correcciones menores para #1743, #1725.
  - Reparar archivos JSON según el cambio de estructura #1743 `Domain`
- Correcciones de consenso en Hyperledger#1751. #1715: Correcciones de consenso para manejar cargas elevadas (#1746)
  - Ver correcciones de manejo de cambios
  - Ver pruebas de cambios independientes de hashes de transacciones particulares
  - Reducción del paso de mensajes.
  - Recopilar votos de cambio de vista en lugar de enviar mensajes de inmediato (mejora la resiliencia de la red)
  - Utilice completamente el marco Actor en Sumeragi (programe mensajes para usted mismo en lugar de generar tareas)
  - Mejora la inyección de fallas para pruebas con Sumeragi
  - Acerca el código de prueba al código de producción.
  - Elimina envoltorios demasiado complicados
  - Permite que Sumeragi use el contexto del actor en el código de prueba.
- hyperledger#1734 Actualizar génesis para que se ajuste a la nueva validación de dominio.
- hyperledger#1742 Se devolvieron errores concretos en las instrucciones `core`.
- Hyperledger#1404 Verificar que esté arreglado.
- hyperledger#1636 Elimina `trusted_peers.json` y `structopt`
  #1636: Eliminar `trusted_peers.json`.
- Hyperledger#1706 Actualización `max_faults` con actualización de topología.
- hyperledger#1698 Se corrigieron claves públicas, documentación y mensajes de error.
- Emisiones de acuñación (1593 y 1405) edición 1405

### Refactorizar- Extraer funciones del bucle principal de sumeragi.
- Refactorizar `ProofChain` a nuevo tipo.
- Quitar `Mutex` de `Metrics`
- Eliminar la función nocturna adt_const_generics.
- hyperledger#3039 Introducir el búfer de espera para las firmas múltiples.
- Simplificar sumeragi.
- hyperledger#3053 Arreglar pelusas cortantes.
- hyperledger#2506 Agregar más pruebas sobre la validación de bloques.
- Eliminar `BlockStoreTrait` en Kura.
- Actualizar pelusas para `nightly-2022-12-22`
- hyperledger#3022 Eliminar `Option` en `transaction_cache`
- hyperledger#3008 Agregar valor de nicho en `Hash`
- Actualizar pelusas a 1.65.
- Añadir pequeñas pruebas para aumentar la cobertura.
- Eliminar código muerto de `FaultInjection`
- Llame a p2p con menos frecuencia desde sumeragi.
- hyperledger#2675 Validar nombres/identificadores de elementos sin asignar Vec.
- hyperledger#2974 Evita la suplantación de bloques sin una revalidación completa.
- `NonEmpty` más eficiente en combinadores.
- hyperledger#2955 Eliminar bloque del mensaje BlockSigned.
- hyperledger#1868 Impide que se envíen transacciones validadas
  entre pares.
- hyperledger#2458 Implementar API de combinador genérico.
- Agregar carpeta de almacenamiento en gitignore.
- Hyperledger#2909 Codifica puertos para nextest.
- hyperledger#2747 Cambiar API `LoadFromEnv`.
- Mejorar los mensajes de error en caso de fallo de configuración.
- Agregar ejemplos adicionales a `genesis.json`
- Eliminar las dependencias no utilizadas antes del lanzamiento de `rc9`.
- Finalizar la eliminación de pelusa en el nuevo Sumeragi.
- Extraer subprocedimientos en el bucle principal.
- hyperledger#2774 Cambiar el modo de generación de génesis `kagami` de bandera a
  subcomando.
- hiperledger#2478 Agregar `SignedTransaction`
- hyperledger#2649 Elimina la caja `byteorder` de `Kura`
- Cambiar el nombre de `DEFAULT_BLOCK_STORE_PATH` de `./blocks` a `./storage`
- hyperledger#2650 Agregue `ThreadHandler` para apagar los submódulos de iroha.
- hyperledger#2482 Almacenar tokens de permiso `Account` en `Wsv`
- Agregar nuevas pelusas a 1.62.
- Mejorar los mensajes de error `p2p`.
- Hyperledger#2001 `EvaluatesTo` verificación de tipo estático.
- hyperledger#2052 Hacer que los tokens de permiso se puedan registrar con definición.
  #2052: Implementar PermissionTokenDefinition
- Asegúrese de que todas las combinaciones de funciones funcionen.
- hyperledger#2468 Elimina el superrasgo de depuración de los validadores de permisos.
- hyperledger#2419 Eliminar `drop`s explícitos.
- hyperledger#2253 Agregar rasgo `Registrable` a `data_model`
- Implementar `Origin` en lugar de `Identifiable` para los eventos de datos.
- hyperledger#2369 Validadores de permisos de refactorización.
- hyperledger#2307 Hacer que `events_sender` en `WorldStateView` no sea opcional.
- hyperledger#1985 Reducir el tamaño de la estructura `Name`.
- Añadir más `const fn`.
- Realizar pruebas de integración utilizando `default_permissions()`
- agregar envoltorios de tokens de permiso en private_blockchain.
- hyperledger#2292 Eliminar `WorldTrait`, eliminar genéricos de `IsAllowedBoxed`
- hyperledger#2204 Hacer que las operaciones relacionadas con activos sean genéricas.
- hyperledger#2233 Reemplace `impl` con `derive` para `Display` e `Debug`.
- Mejoras estructurales identificables.
- hyperledger#2323 Mejora el mensaje de error de inicio de kura.
- hyperledger#2238 Agregar generador de pares para pruebas.
- hyperledger#2011 Parámetros de configuración más descriptivos.
- hyperledger#1896 Simplifica la implementación de `produce_event`.
- Refactorizar alrededor de `QueryError`.
- Mover `TriggerSet` a `data_model`.
- Hyperledger#2145 refactoriza el lado `WebSocket` del cliente, extrae lógica de datos pura.
- eliminar el rasgo `ValueMarker`.
- hyperledger#2149 Exponer `Mintable` e `MintabilityError` en `prelude`
- Hyperledger#2144 rediseña el flujo de trabajo http del cliente y expone la API interna.- Pasar a `clap`.
- Crear binario `iroha_gen`, consolidando documentos, esquema_bin.
- hyperledger#2109 Establece la prueba `integration::events::pipeline`.
- Hyperledger#1982 encapsula el acceso a las estructuras `iroha_crypto`.
- Agregar el constructor `AssetDefinition`.
- Eliminar `&mut` innecesario de la API.
- encapsular el acceso a las estructuras del modelo de datos.
- Hyperledger#2144 rediseña el flujo de trabajo http del cliente y expone la API interna.
- Pasar a `clap`.
- Crear binario `iroha_gen`, consolidando documentos, esquema_bin.
- hyperledger#2109 Establece la prueba `integration::events::pipeline`.
- Hyperledger#1982 encapsula el acceso a las estructuras `iroha_crypto`.
- Agregar el constructor `AssetDefinition`.
- Eliminar `&mut` innecesario de la API.
- encapsular el acceso a las estructuras del modelo de datos.
- Núcleo, `sumeragi`, funciones de instancia, `torii`
- Hyperledger#1903 mueve la emisión de eventos a los métodos `modify_*`.
- Dividir el archivo `data_model` lib.rs.
- Agregar referencia wsv a la cola.
- hyperledger#1210 Secuencia de eventos dividida.
  - Mover la funcionalidad relacionada con transacciones al modelo de datos/módulo de transacciones
- hyperledger#1725 Eliminar el estado global en Torii.
  - Implementar `add_state macro_rules` y eliminar `ToriiState`
- Corregir error de linter.
- Limpieza de Hyperledger#1661 `Cargo.toml`.
  - Clasificar las dependencias de carga.
- hyperledger#1650 ordenar `data_model`
  - Mover World a WSV, corregir función de roles, derivar IntoSchema para CommittedBlock
- Organización de archivos `json` y readme. Actualice el archivo Léame para que se ajuste a la plantilla.
- 1529: registro estructurado.
  - Mensajes de registro de refactorización
- `iroha_p2p`
  - Añadir privatización p2p.

### Documentación

- Actualice el archivo Léame de la CLI del cliente Iroha.
- Actualizar fragmentos de tutoriales.
- Agregue 'sort_by_metadata_key' a la especificación API.
- Actualizar enlaces a la documentación.
- Amplíe el tutorial con documentos relacionados con activos.
- Eliminar archivos doc obsoletos.
- Revisar la puntuación.
- Mover algunos documentos al repositorio de tutoriales.
- Informe de descamación para rama de puesta en escena.
- Generar registro de cambios para pre-rc.7.
- Informe de descamación del 30 de julio.
- Versiones de choque.
- Actualización de prueba de descamación.
- hyperledger#2499 Corregir mensajes de error client_cli.
- hyperledger#2344 Generar CHANGELOG para 2.0.0-pre-rc.5-lts.
- Agregar enlaces al tutorial.
- Actualizar información sobre git hooks.
- redacción de la prueba de descamación.
- Hyperledger#2193 Actualización de la documentación del cliente Iroha.
- Hyperledger#2193 Actualización de la documentación CLI Iroha.
- hyperledger#2193 Actualización README para macro crate.
 - hyperledger#2193 Actualización de la documentación de la herramienta decodificadora Norito.
- Hyperledger#2193 Actualización de la documentación Kagami.
- hyperledger#2193 Actualizar la documentación de los puntos de referencia.
- hyperledger#2192 Revisar las pautas de contribución.
- Corregir referencias rotas en el código.
- Hyperledger#1280 Documento Iroha métricas.
- hyperledger#2119 Agregue orientación sobre cómo recargar en caliente Iroha en un contenedor Docker.
- hyperledger#2181 Revisar README.
- hyperledger#2113 Funciones del documento en archivos Cargo.toml.
- hyperledger#2177 Limpiar la salida de gitchangelog.
- hyperledger#1991 Agregar archivo Léame al inspector de Kura.
- hyperledger#2119 Agregue orientación sobre cómo recargar en caliente Iroha en un contenedor Docker.
- hyperledger#2181 Revisar README.
- hyperledger#2113 Funciones del documento en archivos Cargo.toml.
- hyperledger#2177 Limpiar la salida de gitchangelog.
- hyperledger#1991 Agregar archivo Léame al inspector de Kura.
- generar el último registro de cambios.
- Generar registro de cambios.
- Actualizar archivos README obsoletos.
- Se agregaron documentos faltantes a `api_spec.md`.

### Cambios de CI/CD- Agregue cinco corredores autohospedados más.
- Agregue una etiqueta de imagen normal para el registro de Soramitsu.
- Solución alternativa para libgit2-sys 0.5.0. Vuelva a 0.4.4.
- Intente utilizar una imagen basada en arco.
- Actualice los flujos de trabajo para trabajar en un nuevo contenedor solo nocturno.
- Eliminar los puntos de entrada binarios de la cobertura.
- Cambie las pruebas de desarrollo a ejecutores autohospedados de Equinix.
- hyperledger#2865 Elimina el uso del archivo tmp de `scripts/check.sh`
- hyperledger#2781 Agregar compensaciones de cobertura.
- Desactivar las pruebas de integración lenta.
- Reemplazar la imagen base con la caché de Docker.
- hyperledger#2781 Agregar función principal de confirmación de codecov.
- Mover trabajos a corredores de github.
- Hyperledger#2778 Comprobación de configuración del cliente.
- hyperledger#2732 Agregar condiciones para actualizar las imágenes base iroha2 y agregar
  Etiquetas de relaciones públicas.
- Arreglar la creación de imágenes nocturnas.
- Solucionar el error `buildx` con `docker/build-push-action`
- Primeros auxilios en caso de avería `tj-actions/changed-files`
- Habilitar publicación secuencial de imágenes, después del n.° 2662.
- Agregar registro portuario.
- Etiqueta automática `api-changes` y `config-changes`
- Confirmar hash en imagen, archivo de cadena de herramientas nuevamente, aislamiento de UI,
  seguimiento de esquemas.
- Realizar flujos de trabajo de publicación secuenciales y complementos al #2427.
- hyperledger#2309: Vuelva a habilitar las pruebas de documentos en CI.
- hyperledger#2165 Elimina la instalación de codecov.
- Mover a un nuevo contenedor para evitar conflictos con los usuarios actuales.
 - Hyperledger#2158 Actualización `parity_scale_codec` y otras dependencias. (códec Norito)
- Arreglar la construcción.
- hyperledger#2461 Mejorar iroha2 CI.
- Actualización `syn`.
- mover la cobertura a un nuevo flujo de trabajo.
- versión inversa de inicio de sesión de Docker.
- Eliminar la especificación de versión de `archlinux:base-devel`
- Actualización de Dockerfiles y Codecov, reutilización y simultaneidad de informes.
- Generar registro de cambios.
- Agregar el archivo `cargo deny`.
- Agregue la rama `iroha2-lts` con el flujo de trabajo copiado de `iroha2`
- hyperledger#2393 Actualiza la versión de la imagen base Docker.
- hyperledger#1658 Agregar verificación de documentación.
- Mejora de versiones de cajas y eliminación de dependencias no utilizadas.
- Eliminar informes de cobertura innecesarios.
- hyperledger#2222 Pruebas divididas según si implica cobertura o no.
- hiperledger#2153 Corrección #2154.
- La versión golpea todas las cajas.
- Arreglar la canalización de implementación.
- hyperledger#2153 Cobertura reparada.
- Agregar documentación de verificación y actualización de génesis.
- Eliminar óxido, moho y todas las noches a 1,60, 1,2,0 y 1,62 respectivamente.
- disparadores load-rs.
- hiperledger#2153 Corrección #2154.
- La versión golpea todas las cajas.
- Arreglar la canalización de implementación.
- hyperledger#2153 Cobertura reparada.
- Agregar documentación de verificación y actualización de génesis.
- Eliminar óxido, moho y todas las noches a 1.60, 1.2.0 y 1.62 respectivamente.
- disparadores load-rs.
- load-rs: liberar activadores de flujo de trabajo.
- Arreglar el flujo de trabajo push.
- Agregar telemetría a las funciones predeterminadas.
- agregue la etiqueta adecuada para impulsar el flujo de trabajo en principal.
- corregir pruebas fallidas.
- hyperledger#1657 Actualizar imagen a Rust 1.57. #1630: Vuelva a los corredores autohospedados.
- Mejoras de CI.
- Cobertura cambiada para usar `lld`.
- Corrección de dependencia de CI.
- Mejoras en la segmentación de CI.
- Utiliza una versión fija de Rust en CI.
- Se corrigió la publicación Docker y el CI push de iroha2-dev. Mover cobertura y banca a PR
- Elimine la compilación Iroha completa innecesaria en la prueba de la ventana acoplable CI.

  La compilación Iroha se volvió inútil ya que ahora se realiza en la propia imagen de la ventana acoplable. Por lo tanto, el CI solo crea el cli del cliente que se utiliza en las pruebas.
- Agregar soporte para la rama iroha2 en el proceso de CI.
  - las pruebas largas solo se realizaron en PR en iroha2
  - publicar imágenes de Docker solo desde iroha2
- Cachés de CI adicionales.

### Ensamblaje web


### Mejoras en la versión- Versión anterior a la rc.13.
- Versión anterior a rc.11.
- Versión a RC.9.
- Versión a RC.8.
- Actualizar versiones a RC7.
- Preparativos previos al lanzamiento.
- Actualización del molde 1.0.
- Dependencias de choque.
- Actualización api_spec.md: corrige los cuerpos de solicitud/respuesta.
- Actualice la versión Rust a 1.56.0.
- Actualizar la guía de contribución.
- Actualice README.md e `iroha/config.json` para que coincidan con la nueva API y el formato de URL.
- Actualice el destino de publicación de la ventana acoplable a hyperledger/iroha2 #1453.
- Actualiza el flujo de trabajo para que coincida con el principal.
- Actualice las especificaciones de API y corrija el punto final de salud.
- Actualización de óxido a 1.54.
- Docs(iroha_crypto): actualice los documentos `Signature` y alinee los argumentos de `verify`
- La versión Ursa pasó de 0.3.5 a 0.3.6.
- Actualizar flujos de trabajo para nuevos corredores.
- Actualice el dockerfile para almacenamiento en caché y compilaciones de ci más rápidas.
- Actualizar la versión libssl.
- Actualizar dockerfiles y async-std.
- Corregir clippy actualizado.
- Actualiza la estructura de activos.
  - Soporte para instrucciones clave-valor en activos
  - Tipos de activos como enumeración
  - Vulnerabilidad de desbordamiento en la corrección del activo ISI
- Guía de contribución de actualizaciones.
- Actualización de biblioteca desactualizada.
- Actualizar el documento técnico y solucionar problemas de pelusa.
- Actualice la biblioteca pepino_rust.
- Actualizaciones README para generación de claves.
- Actualizar los flujos de trabajo de Github Actions.
- Actualizar los flujos de trabajo de Github Actions.
- Actualizar requisitos.txt.
- Actualizar common.yaml.
- Actualizaciones de documentos de Sara.
- Actualizar la lógica de instrucciones.
- Actualización del documento técnico.
- Actualiza la descripción de las funciones de red.
- Actualización del documento técnico basado en los comentarios.
- Separación de actualización de WSV y migración a Scale.
- Actualizar gitignore.
- Actualizar ligeramente la descripción de kura en WP.
- Actualizar la descripción sobre kura en el documento técnico.

### Esquema

- hyperledger#2114 Compatibilidad con colecciones ordenadas en esquemas.
- hyperledger#2108 Agregar paginación.
- hyperledger#2114 Compatibilidad con colecciones ordenadas en esquemas.
- hyperledger#2108 Agregar paginación.
- Hacer compatible el esquema, la versión y la macro con no_std.
- Corregir firmas en el esquema.
- Representación alterada de `FixedPoint` en el esquema.
- Se agregó `RawGenesisBlock` a la introspección del esquema.
- Se cambiaron los modelos de objetos para crear el esquema IR-115.

### Pruebas

- Hyperledger#2544 Pruebas documentales tutoriales.
- hyperledger#2272 Agregar pruebas para la consulta 'FindAssetDefinitionById'.
- Agregar pruebas de integración `roles`.
- Estandarizar el formato de las pruebas de interfaz de usuario, mover las pruebas de interfaz de usuario derivadas para derivar cajas.
- Corregir pruebas simuladas (futuros errores desordenados).
- Se eliminó la caja DSL y se movieron las pruebas a `data_model`.
- Asegúrese de que las pruebas de red inestable pasen para obtener un código válido.
- Se agregaron pruebas a iroha_p2p.
- Captura registros en pruebas a menos que la prueba falle.
- Agregue encuestas para las pruebas y corrija las pruebas que rara vez fallan.
- Pruebas de configuración paralela.
- Elimina la raíz de las pruebas iroha init e iroha_client.
- Corrige las advertencias de las pruebas y agrega comprobaciones a ci.
- Corregir errores de validación `tx` durante las pruebas comparativas.
- hyperledger#860: Iroha Consultas y pruebas.
- Guía ISI personalizada Iroha y pruebas de pepino.
- Agregar pruebas para clientes no estándar.
- Cambios y pruebas de registro de puentes.
- Pruebas de consenso con simulacro de red.
- Uso de directorio temporal para la ejecución de pruebas.
- Bancas prueban casos positivos.
- Funcionalidad inicial de Merkle Tree con pruebas.
- Pruebas fijas e inicialización de World State View.

### Otro- Mover la parametrización a los rasgos y eliminar los tipos de FFI IR.
- Agregue soporte para uniones, introduzca `non_robust_ref_mut` * implemente la conversión FFI de cadena constante.
- Mejorar IdOrdEqHash.
- Eliminar FilterOpt::BySome de la (des)serialización.
- No hacer transparente.
- Hacer que ContextValue sea transparente.
- Hacer expresión::etiqueta sin formato opcional.
- Agregue transparencia para algunas instrucciones.
- Mejorar (des)serialización de RoleId.
- Mejorar (des)serialización del validador::Id.
- Mejorar (des)serialización de PermissionTokenId.
- Mejorar (des)serialización de TriggerId.
- Mejorar (des)serialización de identificadores de (definición) de activos.
- Mejorar (des)serialización de AccountId.
- Mejorar (des)serialización de Ipfs y DomainId.
- Eliminar la configuración del registrador de la configuración del cliente.
- Agregar soporte para estructuras transparentes en FFI.
- Refactorizar &Opción<T> a Opción<&T>
- Corregir advertencias cortantes.
- Agregue más detalles en la descripción del error `Find`.
- Se corrigieron las implementaciones `PartialOrd` e `Ord`.
- Utilice `rustfmt` en lugar de `cargo fmt`
- Eliminar la característica `roles`.
- Utilice `rustfmt` en lugar de `cargo fmt`
- Compartir workdir como un volumen con instancias de desarrollo de Docker.
- Eliminar el tipo asociado a Diff en Ejecutar.
- Utilice codificación personalizada en lugar de retorno multival.
- Eliminar serde_json como dependencia de iroha_crypto.
- Permitir solo campos conocidos en el atributo de versión.
- Aclarar diferentes puertos para puntos finales.
- Eliminar la derivación `Io`.
- Documentación inicial de key_pairs.
- Volver a los corredores autohospedados.
- Arreglar nuevas pelusas en el código.
- Eliminar i1i1 de los mantenedores.
- Agregar documentación de actor y correcciones menores.
- Encuesta en lugar de impulsar los últimos bloques.
- Eventos de estado de transacciones probados para cada uno de los 7 pares.
- `FuturesUnordered` en lugar de `join_all`
- Cambie a GitHub Runners.
- Utilice VersionedQueryResult frente a QueryResult para el punto final /query.
- Reconectar la telemetría.
- Arreglar la configuración del dependabot.
- Agregue el gancho git commit-msg para incluir el cierre de sesión.
- Arreglar la tubería de empuje.
- Actualizar dependabot.
- Detectar marca de tiempo futura en el envío de cola.
- hyperledger#1197: Kura maneja errores.
- Agregar instrucción de pares para cancelar el registro.
- Agregue nonce opcional para distinguir transacciones. Cierre #1493.
- Se eliminó `sudo` innecesario.
- Metadatos para dominios.
- Se corrigieron los rebotes aleatorios en el flujo de trabajo `create-docker`.
- Se agregó `buildx` según lo sugerido por la canalización fallida.
- hyperledger#1454: corrige la respuesta de error de consulta con código de estado específico y sugerencias.
- hyperledger#1533: buscar transacción por hash.
- Se corrigió el punto final `configure`.
- Agregar verificación de minabilidad de activos basada en booleanos.
- Adición de primitivas criptográficas escritas y migración a criptografía con seguridad de tipos.
- Mejoras en el registro.
- hyperledger#1458: agregue el tamaño del canal del actor a la configuración como `mailbox`.
- hyperledger#1451: Agregar advertencia sobre configuración incorrecta si `faulty_peers = 0` y `trusted peers count > 1`
- Agregar controlador para obtener hash de bloque específico.
- Se agregó una nueva consulta FindTransactionByHash.
- hyperledger#1185: cambia el nombre y la ruta de las cajas.
- Corregir registros y mejoras generales.
- hyperledger#1150: agrupa 1000 bloques en cada archivo
- Prueba de estrés en cola.
- Corrección del nivel de registro.
- Agregar especificación de encabezado a la biblioteca del cliente.
- Solución de falla de pánico en la cola.
- Cola de reparación.
- Corrección de la versión de lanzamiento del dockerfile.
- Reparación del cliente Https.
- Aceleración ci.
- 1. Se eliminaron todas las dependencias de Ursa, excepto iroha_crypto.
- Se corrigió el desbordamiento al restar duraciones.
- Hacer públicos los campos en el cliente.
- Empuje Iroha2 a Dockerhub todas las noches.
- Corregir códigos de estado http.
- Reemplace iroha_error con thiserror, eyre y color-eyre.
- Sustituir cola por un travesaño.- Eliminar algunos restos de pelusa inútiles.
- Introduce metadatos para definiciones de activos.
- Eliminación de argumentos de la caja test_network.
- Eliminar dependencias innecesarias.
- Arreglar iroha_client_cli::eventos.
- hyperledger#1382: Elimina la implementación de red antigua.
- hyperledger#1169: precisión agregada para los activos.
- Mejoras en la puesta en marcha entre pares:
  - Permite cargar la clave pública de génesis solo desde env
  - La ruta de configuración, génesis y Trusted_peers ahora se puede especificar en los parámetros cli
- hyperledger#1134: Integración de Iroha P2P.
- Cambie el punto final de la consulta a POST en lugar de GET.
- Ejecutar on_start en actor de forma sincrónica.
- Migrar a warp.
- Reelaborar el compromiso con correcciones de errores del corredor.
- Revertir la confirmación "Introduce varias correcciones de intermediarios" (9c148c33826067585b5868d297dcdd17c0efe246)
- Introduce múltiples correcciones de intermediarios:
  - Cancelar la suscripción del corredor al detener al actor
  - Admite múltiples suscripciones del mismo tipo de actor (anteriormente TODO)
  - Se corrigió un error por el cual el corredor siempre se ponía a sí mismo como identificación del actor.
- Error del corredor (exhibición de prueba).
- Agregar derivaciones para el modelo de datos.
- Quitar rwlock del torii.
- Verificaciones de permisos de consultas OOB.
- hyperledger#1272: Implementación de recuentos de pares,
- Comprobación recursiva de permisos de consulta dentro de las instrucciones.
- Programar paradas de actores.
- hyperledger#1165: Implementación de recuentos de pares.
- Verifique los permisos de consulta por cuenta en el punto final torii.
- Se eliminó la exposición del uso de CPU y memoria en las métricas del sistema.
 - Reemplace JSON con Norito para mensajes WS.
- Almacenar prueba de cambios de vista.
- Hyperledger#1168: Se agregó registro si la transacción no pasa la condición de verificación de firma.
- Se corrigieron pequeños problemas, se agregó código de escucha de conexión.
- Introducir el generador de topología de red.
- Implementar red P2P para Iroha.
- Agrega métrica de tamaño de bloque.
- El rasgo PermissionValidator cambió de nombre a IsAllowed. y otros cambios de nombre correspondientes
- Correcciones de socket web de especificaciones API.
- Elimina dependencias innecesarias de la imagen de la ventana acoplable.
- Fmt usa Crate import_granularity.
- Introduce el validador de permisos genérico.
- Migrar al marco del actor.
- Cambiar el diseño del broker y agregar algunas funcionalidades a los actores.
- Configura comprobaciones de estado de codecov.
- Utiliza cobertura basada en fuente con grcov.
- Se corrigió el formato de múltiples argumentos de compilación y se redeclaró ARG para contenedores de compilación intermedios.
- Introduce el mensaje Suscripción aceptada.
- Eliminar activos de valor cero de las cuentas después de operarlas.
- Se corrigió el formato de argumentos de compilación de Docker.
- Se corrigió el mensaje de error si no se encontraba el bloque secundario.
- Se agregó OpenSSL suministrado para compilar y corrige la dependencia de pkg-config.
- Se corrigió el nombre del repositorio para dockerhub y la diferencia de cobertura.
- Se agregó texto de error claro y nombre de archivo si no se podía cargar TrustedPeers.
- Se cambiaron entidades de texto a enlaces en documentos.
- Se corrigió el secreto de nombre de usuario incorrecto en la publicación Docker.
- Corregir pequeños errores tipográficos en el documento técnico.
- Permite el uso de mod.rs para una mejor estructura de archivos.
- Mueva main.rs a una caja separada y otorgue permisos para blockchain pública.
- Agregar consultas dentro del cliente cli.
- Migrar de clap a structopts para cli.
- Limitar la telemetría a la prueba de red inestable.
- Mover rasgos al módulo de contratos inteligentes.
- Sed -i "s/world_state_view/wsv/g"
- Mover los contratos inteligentes a un módulo independiente.
- Corrección de errores de longitud del contenido de red Iroha.
- Agrega almacenamiento local de tareas para la identificación del actor. Útil para la detección de interbloqueos.
- Agregar prueba de detección de punto muerto a CI
- Agregar macro de introspección.
- Elimina la ambigüedad de los nombres de los flujos de trabajo y también corrige el formato.
- Cambio de api de consulta.
- Migración de async-std a tokio.
- Agregar análisis de telemetría a ci.- Agregar telemetría de futuros para iroha.
- Agregue futuros de iroha a cada función asíncrona.
- Agregue futuros de iroha para la observabilidad del número de encuestas.
- Implementación y configuración manual agregadas a README.
- Reparación del reportero.
- Agregar macro de mensaje derivado.
- Agregar un marco de actor simple.
- Agregar configuración de dependabot.
- Agregue buenos reporteros de pánico y errores.
- Migración de la versión Rust a 1.52.1 y correcciones correspondientes.
- Generar bloqueo de tareas intensivas de CPU en subprocesos separados.
- Utilice Unique_port y cargo-lints de crates.io.
- Corrección para WSV sin bloqueo:
  - elimina Dashmaps innecesarios y bloquea la API
  - corrige un error con una cantidad excesiva de bloques creados (las transacciones rechazadas no se registraron)
  - Muestra la causa completa de los errores
- Agregar suscriptor de telemetría.
- Consultas de roles y permisos.
- Mover bloques de kura a wsv.
- Cambiar a estructuras de datos sin bloqueo dentro de WSV.
- Corrección del tiempo de espera de la red.
- Reparación del punto final de salud.
- Introduce roles.
- Agregue imágenes push de Docker desde la rama de desarrollo.
- Agregue linting más agresivo y elimine los pánicos del código.
- Reelaboración del rasgo Ejecutar para obtener instrucciones.
- Eliminar el código antiguo de iroha_config.
- IR-1060 agrega comprobaciones de concesión para todos los permisos existentes.
- Se corrigió ulimit y tiempo de espera para iroha_network.
- Corrección de prueba de tiempo de espera de Ci.
- Eliminar todos los activos cuando se eliminó su definición.
- Se solucionó el pánico de WSV al agregar activos.
- Elimina Arc y Rwlock para canales.
- Reparación de red Iroha.
- Los validadores de permisos utilizan referencias en los controles.
- Subvención de Instrucción.
- Configuración agregada para límites de longitud de cadenas y validación de identificaciones para NewAccount, Domain y AssetDefinition IR-1036.
- Sustituir el registro por la biblioteca de seguimiento.
- Agregue ci check para documentos y deniegue la macro dbg.
- Introduce permisos concedibles.
- Agregar caja iroha_config.
- Agregue @alerdenisov como propietario del código para aprobar todas las solicitudes de fusión entrantes.
- Corrección de la verificación del tamaño de la transacción durante el consenso.
- Revertir la actualización de async-std.
- Reemplazar algunas constantes con potencia de 2 IR-1035.
- Agregar consulta para recuperar el historial de transacciones IR-1024.
- Agregar validación de permisos para almacenamiento y reestructuración de validadores de permisos.
- Agregue una nueva cuenta para registrar la cuenta.
- Agregar tipos para la definición de activos.
- Introduce límites de metadatos configurables.
- Introduce metadatos de transacciones.
- Agregar expresiones dentro de las consultas.
- Agregue lints.toml y corrija las advertencias.
- Separe los pares confiables de config.json.
- Se corrigió el error tipográfico en la URL de la comunidad Iroha 2 en Telegram.
- Corregir advertencias cortantes.
- Introduce compatibilidad con metadatos de valores clave para la cuenta.
- Agregar versionado de bloques.
- Corrección de repeticiones de pelusa.
- Agregar expresiones mul,div,mod,raise_to.
- Agregue into_v* para versiones.
- Sustituir Error::msg con macro de error.
- Reescribir iroha_http_server y corregir los errores de torii.
 - Actualiza la versión Norito a 2.
- Descripción de versiones del documento técnico.
- Paginación infalible. Soluciona los casos en los que la paginación puede ser innecesaria debido a errores, y no devuelve colecciones vacías.
- Agregar derivación (Error) para enumeraciones.
- Arreglar la versión nocturna.
- Agregar caja iroha_error.
- Mensajes versionados.
- Introduce primitivas de control de versiones de contenedores.
- Corregir puntos de referencia.
- Agregar paginación.
- Agregar decodificación de codificación varint.
- Cambiar la marca de tiempo de la consulta a u128.
- Agregar enumeración RejectionReason para eventos de canalización.
- Elimina líneas obsoletas de archivos génesis. El destino fue eliminado del registro ISI en confirmaciones anteriores.
- Simplifica el registro y cancelación del registro de ISI.
- Se corrigió el tiempo de espera de confirmación que no se enviaba en la red de 4 pares.
- Topología aleatoria en la vista de cambios.- Agregar otros contenedores para la macro de derivación FromVariant.
- Agregar soporte MST para cliente cli.
- Agregue la macro FromVariant y el código base de limpieza.
- Agregue i1i1 a los propietarios del código.
- Transacciones de chismes.
- Agregue longitud para instrucciones y expresiones.
- Agregue documentos para bloquear tiempo y confirmar parámetros de tiempo.
- Se reemplazaron los rasgos Verificar y Aceptar con TryFrom.
- Introducir la espera sólo por el número mínimo de pares.
- Agregar acción de github para probar la API con iroha2-java.
- Agregue génesis para docker-compose-single.yml.
- Condición de verificación de firma predeterminada para la cuenta.
- Agregar prueba para cuentas con múltiples firmantes.
- Agregar soporte de API de cliente para MST.
- Construir en la ventana acoplable.
- Agregar génesis a Docker Compose.
- Introducir MST condicional.
- Agregar impl. wait_for_active_peers.
- Agregar prueba para el cliente isahc en iroha_http_server.
- Especificaciones de la API del cliente.
- Ejecución de consultas en Expresiones.
- Integra expresiones e ISI.
- Expresiones para ISI.
- Arreglar los puntos de referencia de configuración de la cuenta.
- Agregar configuración de cuenta para el cliente.
- Arreglar `submit_blocking`.
- Se envían eventos de canalización.
- Conexión de socket web del cliente Iroha.
- Separación de eventos para eventos de canalización y datos.
- Prueba de integración de permisos.
- Agregue comprobaciones de permisos para grabar y acuñar.
- Dar de baja el permiso ISI.
- Arreglar puntos de referencia para las relaciones públicas de estructura mundial.
- Introducir la estructura mundial.
- Implementar el componente de carga del bloque génesis.
- Introducir la cuenta de génesis.
- Introducir el generador de validadores de permisos.
- Agregar etiquetas a los PR de Iroha2 con Github Actions.
- Introducir el marco de permisos.
- Límite de número de cola tx tx y correcciones de inicialización Iroha.
- Envolver Hash en una estructura.
- Mejorar el nivel de registro:
  - Agregar registros de nivel de información al consenso.
  - Marcar los registros de comunicación de red como nivel de seguimiento.
  - Elimine el vector de bloque de WSV ya que es una duplicación y muestra toda la cadena de bloques en los registros.
  - Establecer el nivel de registro de información como predeterminado.
- Eliminar referencias mutables de WSV para su validación.
- Incremento de la versión Heim.
- Agregue pares de confianza predeterminados a la configuración.
- Migración de API de cliente a http.
- Agregar transferencia isi a CLI.
- Configuración de instrucciones relacionadas con pares Iroha.
- Implementación de métodos de ejecución y pruebas ISI faltantes.
- Análisis de parámetros de consulta de URL
- Añadir `HttpResponse::ok()`, `HttpResponse::upgrade_required(..)`
- Reemplazo de modelos antiguos de Instrucción y Consulta con enfoque DSL Iroha.
- Agregar soporte para firmas BLS.
- Introducir la caja del servidor http.
- Parcheado libssl.so.1.0.0 con enlace simbólico.
- Verifica la firma de la cuenta para la transacción.
- Refactorizar etapas de transacciones.
- Mejoras iniciales en los dominios.
- Implementar prototipo DSL.
- Mejorar los puntos de referencia Torii: deshabilitar el inicio de sesión en los puntos de referencia, agregar afirmación de índice de éxito.
- Mejorar el proceso de cobertura de pruebas: reemplaza `tarpaulin` con `grcov`, publica el informe de cobertura de pruebas en `codecov.io`.
- Arreglar el tema RTD.
- Entrega de artefactos para subproyectos de iroha.
- Introducir `SignedQueryRequest`.
- Corregir un error con la verificación de firma.
- Soporte de transacciones de reversión.
- Imprimir el par de claves generado como json.
- Admite el par de claves `Secp256k1`.
- Soporte inicial para diferentes algoritmos criptográficos.
- Funciones DEX.
- Reemplace la ruta de configuración codificada con el parámetro cli.
- Corrección del flujo de trabajo maestro de banco.
- Prueba de conexión del evento Docker.
- Guía del monitor Iroha y CLI.
- Mejoras en el CLI de eventos.
- Filtro de eventos.
- Conexiones de eventos.
- Corrección en el flujo de trabajo maestro.
- IDT para iroha2.
- Hash de raíz del árbol Merkle para transacciones en bloque.
- Publicación en Docker Hub.
- Funcionalidad CLI para Maintenance Connect.
- Funcionalidad CLI para Maintenance Connect.
- Eprintln para registrar la macro.- Mejoras de registro.
- IR-802 Suscripción a cambios de estados de bloques.
- Envío de eventos de transacciones y bloques.
- Mueve el manejo de mensajes Sumeragi a impl. de mensajes.
- Mecanismo General de Conexión.
- Extraiga entidades de dominio Iroha para clientes no estándar.
- Transacciones TTL.
- Transacciones máximas por configuración de bloque.
- Almacenar hashes de bloques invalidados.
- Sincronizar bloques en lotes.
- Configuración de la funcionalidad de conexión.
- Conéctese a la funcionalidad Iroha.
- Correcciones de validación de bloques.
- Sincronización de bloques: diagramas.
- Conéctese a la funcionalidad Iroha.
- Puente: eliminar clientes.
- Sincronización de bloques.
- Agregar Peer ISI.
- Cambio de nombre de comandos a instrucciones.
- Punto final de métricas simples.
- Puente: obtenga puentes registrados y activos externos.
- Docker prueba de redacción en proceso.
- No hay suficientes votos en la prueba Sumeragi.
- Encadenamiento de bloques.
- Puente: manejo manual de transferencias externas.
- Punto final de mantenimiento simple.
- Migración a serde-json.
- Desminado ISI.
- Agregar clientes puente, AddSignatory ISI y permiso CanAddSignatory.
- Sumeragi: correcciones TODO relacionadas con pares en el conjunto b.
- Valida el bloque antes de iniciar sesión en Sumeragi.
- Puentear activos externos.
- Validación de firma en mensajes Sumeragi.
- Almacén de activos binarios.
- Reemplace el alias de PublicKey con el tipo.
- Preparar cajas para publicar.
- Lógica de votos mínimos dentro de NetworkTopology.
- Refactorización de validación de TransactionReceipt.
- Cambio de activador de OnWorldStateViewChange: IrohaQuery en lugar de Instrucción.
- Separar la construcción de la inicialización en NetworkTopology.
- Agregar instrucciones especiales Iroha relacionadas con eventos Iroha.
- Manejo del tiempo de espera de creación de bloques.
- Glosario y documentación sobre cómo agregar el módulo Iroha.
- Reemplace el modelo de puente codificado por el modelo original Iroha.
- Introducir la estructura NetworkTopology.
- Agregar entidad de permiso con transformación de Instrucciones.
- Sumeragi Mensajes en el módulo de mensajes.
- Funcionalidad Genesis Block para Kura.
- Agregue archivos README para cajas Iroha.
- Puente y RegistroBridge ISI.
- El trabajo inicial con Iroha cambia los oyentes.
- Inyección de comprobaciones de permisos en OOB ISI.
- Corrección de varios pares Docker.
- Ejemplo de ventana acoplable punto a punto.
- Manejo de recibos de transacciones.
- Permisos Iroha.
- Módulo para Dex y cajas para Bridges.
- Arreglar la prueba de integración con la creación de activos con varios pares.
- Reimplementación del modelo de Activos en EC-S-.
- Confirmar el manejo del tiempo de espera.
- Bloquear encabezado.
- Métodos relacionados con ISI para entidades de dominio.
- Enumeración del modo Kura y configuración de Trusted Peers.
- Regla de desprendimiento de documentación.
- Agregar bloque comprometido.
- Desacoplamiento de kura de `sumeragi`.
- Verifique que las transacciones no estén vacías antes de la creación del bloque.
- Vuelva a implementar las instrucciones especiales Iroha.
- Benchmarks para transacciones y transiciones de bloques.
- Ciclo de vida de transacciones y estados reelaborados.
- Bloquea el ciclo de vida y los estados.
- Se corrigió el error de validación, ciclo de bucle `sumeragi` sincronizado con el parámetro de configuración block_build_time_ms.
- Encapsulación del algoritmo Sumeragi dentro del módulo `sumeragi`.
- Módulo de simulación para caja de red Iroha implementado vía canales.
- Migración a API async-std.
- Función de simulación de red.
- Limpieza de código relacionado asíncrono.
- Optimizaciones de rendimiento en el bucle de procesamiento de transacciones.
- La generación de pares de claves se extrajo del inicio Iroha.
- Empaquetado Docker del ejecutable Iroha.- Introducir el escenario básico Sumeragi.
- Cliente CLI Iroha.
- Caída de iroha tras ejecución en grupo en banco.
- Integrar `sumeragi`.
- Cambie la implementación de `sort_peers` a rand aleatorio sembrado con hash de bloque anterior.
- Eliminar el contenedor de mensajes en el módulo de pares.
- Encapsule información relacionada con la red dentro de `torii::uri` e `iroha_network`.
- Agregar instrucción Peer implementada en lugar del manejo de código rígido.
- Comunicación entre pares a través de una lista de pares de confianza.
- Encapsulación del manejo de solicitudes de red dentro de Torii.
- Encapsulación de lógica criptográfica dentro del módulo criptográfico.
- Signo de bloque con marca de tiempo y hash de bloque anterior como carga útil.
- Funciones criptográficas ubicadas en la parte superior del módulo y funcionan con el firmante ursa encapsulado en Signature.
- Sumeragi inicial.
- Validación de las instrucciones de transacción en el clon de la vista del estado mundial antes de enviarlo a la tienda.
- Verificar firmas en la aceptación de la transacción.
- Corrección de error en Solicitar deserialización.
- Implementación de firma Iroha.
- Se eliminó la entidad Blockchain para limpiar el código base.
- Cambios en la API de Transacciones: mejor creación y trabajo con solicitudes.
- Se corrigió el error que crearía bloques con un vector de transacción vacío.
- Transacciones pendientes a plazo.
 - Se corrigió el error con un byte faltante en el paquete TCP codificado u128 Norito.
- Macros de atributos para seguimiento de métodos.
- Módulo P2p.
- Uso de iroha_network en torii y cliente.
- Agregar nueva información ISI.
- Alias ​​de tipo específico para el estado de la red.
- Cuadro<dyn Error> reemplazado por String.
- Red escucha con estado.
- Lógica de validación inicial de transacciones.
- Caja Iroha_network.
- Derivar macro para rasgos Io, IntoContract e IntoQuery.
- Implementación de consultas para el cliente Iroha.
- Transformación de Mandos en contratos ISI.
- Agregar diseño propuesto para multifirma condicional.
- Migración a espacios de trabajo de Cargo.
- Migración de módulos.
- Configuración externa mediante variables de entorno.
- Manejo de solicitudes Get y Put para Torii.
- Corrección de Github ci.
- Cargo-make limpia los bloques después de la prueba.
- Introducir el módulo `test_helper_fns` con una función para limpiar directorio con bloques.
- Implementar validación vía árbol merkle.
- Eliminar el derivado no utilizado.
- Propagar async/await y corregir `wsv::put` no esperado.
- Utilice la unión desde la caja `futures`.
- Implementar la ejecución del almacén paralelo: la escritura en el disco y la actualización de WSV se realizan en paralelo.
- Utilice referencias en lugar de propiedad para la (des)serialización.
- Expulsión de código de archivos.
- Utilice ursa::blake2.
- Regla sobre mod.rs en la guía de contribución.
- Hash de 32 bytes.
- Hachís Blake2.
- El disco acepta referencias para bloquear.
- Refactorización del módulo de comandos y Merkle Tree Inicial.
- Estructura de módulos refactorizada.
- Formato correcto.
- Agregar comentarios de documentos a read_all.
- Implementar `read_all`, reorganizar las pruebas de almacenamiento y convertir las pruebas con funciones asíncronas en pruebas asíncronas.
- Eliminar captura mutable innecesaria.
- Revisar el problema, solucionar el clippy.
- Quitar tablero.
- Agregar verificación de formato.
- Agregar ficha.
- Crear rust.yml para acciones de github.
- Introducir el prototipo de almacenamiento en disco.
- Transferencia de prueba y funcionalidad de activos.
- Agregar inicializador predeterminado a las estructuras.
- Cambiar el nombre de la estructura MSTCache.
- Agregar préstamo olvidado.
- Esquema inicial del código iroha2.
- API Kura inicial.
- Agregue algunos archivos básicos y también publique el primer borrador del documento técnico que describe la visión de iroha v2.
- Rama básica de iroha v2.

## [1.5.0] - 2022-04-08

### Cambios de CI/CD
- Eliminar Jenkinsfile y JenkinsCI.

### Agregado- Agregar implementación de almacenamiento RocksDB para Burrow.
- Introducir la optimización del tráfico con Bloom-filter
- Actualice la red del módulo `MST` para que se ubique en el módulo `OS` en `batches_cache`.
- Proponer optimización del tráfico.

### Documentación

- Arreglar la construcción. Agregue diferencias de base de datos, prácticas de migración, punto final de verificación de estado, información sobre la herramienta iroha-swarm.

### Otro

- Corrección de requisitos para la compilación de documentos.
- Recortar la documentación de lanzamiento para resaltar el elemento de seguimiento crítico restante.
- Se corrigió 'comprobar si existe la imagen de la ventana acoplable' /compilar todo skip_testing.
- /construir todo skip_testing.
- /build skip_testing; Y más documentos.
- Añadir `.github/_README.md`.
- Quitar `.packer`.
- Eliminar cambios en el parámetro de prueba.
- Utilice un nuevo parámetro para omitir la etapa de prueba.
- Agregar al flujo de trabajo.
- Eliminar el envío del repositorio.
- Agregar despacho de repositorio.
- Agregar parámetro para probadores.
- Eliminar el tiempo de espera `proposal_delay`.

## [1.4.0] - 2022-01-31

### Agregado

- Agregar estado del nodo de sincronización
- Agrega métricas para RocksDB
- Agregar interfaces de verificación de estado a través de http y métricas.

### Correcciones

- Se corrigieron familias de columnas en Iroha v1.4-rc.2
- Agregue un filtro de floración de 10 bits en Iroha v1.4-rc.1

### Documentación

- Agregue zip y pkg-config a la lista de departamentos de compilación.
- Actualizar archivo Léame: corrige enlaces rotos al estado de la compilación, guía de compilación, etc.
- Arreglar la configuración y las métricas Docker.

### Otro

- Actualizar la etiqueta acoplable de GHA.
- Se corrigieron errores de compilación Iroha 1 al compilar con g++11.
- Reemplace `max_rounds_delay` con `proposal_creation_timeout`.
- Actualice el archivo de configuración de muestra para eliminar los parámetros de conexión de base de datos antiguos.
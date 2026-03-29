---
lang: es
direction: ltr
source: docs/source/universal_accounts_guide.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f972f8f82b7f4e89c1d48b0dbbc6eb5b73303e2fab0f580ab21e63990ba03af8
source_last_modified: "2026-03-27T19:05:17.617064+00:00"
translation_last_reviewed: 2026-03-28
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Guía de cuentas universales

Esta guía resume los requisitos de implementación de UAID (ID de cuenta universal) de
la hoja de ruta Nexus y los empaqueta en un tutorial centrado en el operador + SDK.
Cubre derivación de UAID, inspección de cartera/manifiesto, plantillas de regulador,
y la evidencia que debe acompañar a cada manifiesto del directorio espacial de la aplicación `iroha
publicar` run (roadmap reference: `roadmap.md:2209`).

## 1. Referencia rápida de la UAID- Los UAID son literales `uaid:<hex>` donde `<hex>` es un resumen Blake2b-256 cuyo
  LSB está configurado en `1`. El tipo canónico vive en
  `crates/iroha_data_model/src/nexus/manifest.rs::UniversalAccountId`.
- Los registros de cuentas (`Account` e `AccountDetails`) ahora llevan un `uaid` opcional
  campo para que las aplicaciones puedan aprender el identificador sin hash personalizado.
- Las políticas de identificadores de funciones ocultas pueden vincular entradas normalizadas arbitrarias.
  (números de teléfono, correos electrónicos, números de cuenta, cadenas de socios) a los ID `opaque:`
  bajo un espacio de nombres UAID. Las piezas en cadena son `IdentifierPolicy`,
  `IdentifierClaimRecord` y el índice `opaque_id -> uaid`.
- Space Directory mantiene un mapa `World::uaid_dataspaces` que vincula cada UAID
  a las cuentas de espacio de datos a las que hacen referencia los manifiestos activos. Torii reutiliza eso
  mapa para las API `/portfolio` e `/uaids/*`.
- `POST /v1/accounts/onboard` publica un manifiesto de directorio espacial predeterminado para
  el espacio de datos global cuando no existe ninguno, por lo que el UAID se vincula inmediatamente.
  Las autoridades de incorporación deben tener `CanPublishSpaceDirectoryManifest{dataspace=0}`.
- Todos los SDK exponen ayudas para canonizar literales UAID (por ejemplo,
  `UaidLiteral` en el SDK de Android). Los ayudantes aceptan resúmenes crudos de 64 hex.
  (LSB=1) o literales `uaid:<hex>` y reutilice los mismos códecs Norito para que
  El resumen no puede variar entre idiomas.

## 1.1 Políticas de identificadores ocultos

Los UAID son ahora el ancla de una segunda capa de identidad:

- Un `IdentifierPolicyId` (`<kind>#<business_rule>`) global define el
  espacio de nombres, metadatos de compromiso público, clave de verificación del resolutor y el
  modo de normalización de entrada canónica (`Exact`, `LowercaseTrimmed`,
  `PhoneE164`, `EmailAddress` o `AccountNumber`).
- Una reclamación vincula un identificador `opaque:` derivado a exactamente un UAID y un
  canónico `AccountId` bajo esa política, pero la cadena solo acepta el
  reclamación cuando va acompañada de un `IdentifierResolutionReceipt` firmado.
- La resolución sigue siendo un flujo `resolve -> transfer`. Torii resuelve el opaco
  maneja y devuelve el canónico `AccountId`; Las transferencias todavía apuntan a
  cuenta canónica, no literales `uaid:` o `opaque:` directamente.
- Las políticas ahora pueden publicar parámetros de cifrado de entrada BFV a través de
  `PolicyCommitment.public_parameters`. Cuando están presentes, Torii los anuncia en
  `GET /v1/identifier-policies`, y los clientes pueden enviar entradas envueltas en BFV
  en lugar de texto sin formato. Las políticas programadas envuelven los parámetros BFV en un
  paquete canónico `BfvProgrammedPublicParameters` que también publica el
  público `ram_fhe_profile`; Las cargas útiles BFV sin procesar heredadas se actualizan a eso.
  paquete canónico cuando se reconstruye el compromiso.
- Las rutas del identificador pasan por el mismo token de acceso y límite de velocidad Torii.
  verificaciones como otros puntos finales orientados a aplicaciones. No son un bypass alrededor de lo normal.
  Política de API.

## 1.2 Terminología

La división de nombres es intencional:

- `ram_lfe` es la abstracción de función oculta externa. Cubre póliza
  registro, compromisos, metadatos públicos, recibos de ejecución y
  modo de verificación.
- `BFV` es el esquema de cifrado homomórfico Brakerski/Fan-Vercauteren utilizado por
  algunos backends `ram_lfe` para evaluar la entrada cifrada.
- `ram_fhe_profile` son metadatos específicos de BFV, no un segundo nombre para el conjunto
  característica. Describe la máquina de ejecución BFV programada que carteras y
  Los verificadores deben apuntar cuando una política utiliza el backend programado.

En términos concretos:

- `RamLfeProgramPolicy` e `RamLfeExecutionReceipt` son tipos de capa LFE.
- `BfvParameters`, `BfvCiphertext`, `BfvProgrammedPublicParameters` y
  `BfvRamProgramProfile` son tipos de capa FHE.
- `HiddenRamFheProgram` e `HiddenRamFheInstruction` son nombres internos para
  el programa BFV oculto ejecutado por el backend programado. Se quedan en el
  Lado FHE porque describen el mecanismo de ejecución cifrado en lugar de
  la política exterior o la abstracción del recibo.

## 1.3 Identidad de cuenta versus alias

La implementación de la cuenta universal no cambia el modelo de identidad de la cuenta canónica:

- `AccountId` sigue siendo el sujeto de cuenta canónico sin dominio.
- `ScopedAccountId { account, domain }` es un contexto de dominio explícito para vistas o
  registros que materializan un enlace de dominio. No es un segundo canónico
  identidad.
- Los alias de SNS/cuenta son enlaces separados además de ese tema. un
  alias calificado de dominio como `merchant@hbl.sbp` y un alias de raíz de espacio de datos
  como `merchant@sbp` pueden resolverse en el mismo `AccountId` canónico.
- `linked_domains` en los registros de cuentas almacenados se deriva del estado del
  índices de dominio de cuenta. Describe enlaces actualmente materializados para ese
  sujeto; no forma parte del identificador canónico.

Regla de implementación para operadores, SDK y pruebas: comenzar desde lo canónico
`AccountId`, luego agregue arrendamientos de alias, permisos de dominio/espacio de datos y datos explícitos.
enlaces de dominio por separado. No sintetice un canónico falso con alcance de dominio
cuenta simplemente porque un alias o ruta lleva un segmento de dominio.

Rutas actuales Torii:

| Ruta | Propósito |
|-------|---------|
| `GET /v1/ram-lfe/program-policies` | Enumera las políticas del programa RAM-LFE activas e inactivas además de sus metadatos de ejecución pública, incluidos los parámetros BFV `input_encryption` opcionales y el backend programado `ram_fhe_profile`. |
| `POST /v1/ram-lfe/programs/{program_id}/execute` | Acepta exactamente uno de `{ input_hex }` o `{ encrypted_input }` y devuelve el `RamLfeExecutionReceipt` sin estado más `{ output_hex, output_hash, receipt_hash }` para el programa seleccionado. El tiempo de ejecución actual Torii emite recibos para el backend BFV programado. |
| `POST /v1/ram-lfe/receipts/verify` | Valida sin estado un `RamLfeExecutionReceipt` con la política del programa en cadena publicada y, opcionalmente, verifica que un `output_hex` proporcionado por la persona que llama coincida con el recibo `output_hash`. |
| `GET /v1/identifier-policies` | Enumera los espacios de nombres de políticas de funciones ocultas activas e inactivas más sus metadatos públicos, incluidos los parámetros BFV `input_encryption` opcionales, el modo `normalization` requerido para entradas cifradas del lado del cliente e `ram_fhe_profile` para políticas BFV programadas. |
| `POST /v1/accounts/{account_id}/identifiers/claim-receipt` | Acepta exactamente uno de `{ input }` o `{ encrypted_input }`. El texto sin formato `input` está normalizado en el lado del servidor; BFV `encrypted_input` ya debe estar normalizado según el modo de política publicado. Luego, el punto final deriva el identificador `opaque:` y devuelve un recibo firmado que `ClaimIdentifier` puede enviar en cadena, incluido el `signature_payload_hex` sin procesar y el `signature_payload` analizado. || `POST /v1/identifiers/resolve` | Acepta exactamente uno de `{ input }` o `{ encrypted_input }`. El texto sin formato `input` está normalizado en el lado del servidor; BFV `encrypted_input` ya debe estar normalizado según el modo de política publicado. El punto final resuelve el identificador en `{ opaque_id, receipt_hash, uaid, account_id, signature }` cuando existe un reclamo activo y también devuelve la carga útil canónica firmada como `{ signature_payload_hex, signature_payload }`. |
| `GET /v1/identifiers/receipts/{receipt_hash}` | Busca el `IdentifierClaimRecord` persistente vinculado a un hash de recibo determinista para que los operadores y los SDK puedan auditar la propiedad del reclamo o diagnosticar fallas de reproducción/no coincidencia sin escanear el índice de identificador completo. |

El tiempo de ejecución en proceso de Torii está configurado en
`torii.ram_lfe.programs[*]`, codificado por `program_id`. El identificador enruta ahora
reutilice el mismo tiempo de ejecución RAM-LFE en lugar de un `identifier_resolver` separado
superficie de configuración.

Soporte actual de SDK:

- `normalizeIdentifierInput(value, normalization)` coincide con Rust
  canonicalizadores para `exact`, `lowercase_trimmed`, `phone_e164`,
  `email_address` e `account_number`.
- `ToriiClient.listIdentifierPolicies()` enumera metadatos de políticas, incluido BFV
  metadatos de cifrado de entrada cuando la política los publica, además de un decodificado
  Objeto de parámetro BFV a través de `input_encryption_public_parameters_decoded`.
  Las políticas programadas también exponen el `ram_fhe_profile` decodificado. ese campo es
  intencionalmente con alcance BFV: permite que las billeteras verifiquen el registro esperado
  recuento, recuento de carriles, modo de canonicalización y módulo mínimo de texto cifrado para
  el backend FHE programado antes de cifrar la entrada del lado del cliente.
- `getIdentifierBfvPublicParameters(policy)` y
  `buildIdentifierRequestForPolicy(policy, { input | encryptedInput })` ayuda
  Las personas que llaman JS consumen metadatos BFV publicados y crean solicitudes basadas en políticas
  organismos sin volver a implementar reglas de normalización y identificación de políticas.
- `encryptIdentifierInputForPolicy(policy, input, { seedHex? })` y
  `buildIdentifierRequestForPolicy(policy, { input, encrypt: true })` ahora deja
  Las billeteras JS construyen el sobre de texto cifrado BFV Norito completo localmente desde
  parámetros de política publicados en lugar de enviar texto cifrado hexadecimal prediseñado.
- `ToriiClient.resolveIdentifier({ policyId, input | encryptedInput })`
  resuelve un identificador oculto y devuelve la carga útil del recibo firmado,
  incluyendo `receipt_hash`, `signature_payload_hex` y
  `signature_payload`.
- `ToriiClient.issueIdentifierClaimReceipt(cuentaId, {políticaId, entrada |
  entrada cifrada})` issues the signed receipt needed by `ClaimIdentifier`.
- `verifyIdentifierResolutionReceipt(receipt, policy)` verifica el retorno
  recibo contra la clave de resolución de políticas en el lado del cliente, y`ToriiClient.getIdentifierClaimByReceiptHash(receiptHash)` recupera el
  registro de reclamo persistente para flujos de auditoría/depuración posteriores.
- `IrohaSwift.ToriiClient` ahora expone `listIdentifierPolicies()`,
  `resolveIdentifier(policyId:input:encryptedInputHex:)`,
  `issueIdentifierClaimReceipt(accountId:policyId:input:encryptedInputHex:)`,
  y `getIdentifierClaimByReceiptHash(_)`, más
  `ToriiIdentifierNormalization` para el mismo teléfono/correo electrónico/número de cuenta
  Modos de canonicalización.
- `ToriiIdentifierLookupRequest` y el
  `ToriiIdentifierPolicySummary.plaintextRequest(...)` /
  Los ayudantes `.encryptedRequest(...)` proporcionan la superficie de solicitud Swift escrita para
  resolver y reclamar llamadas de recepción, y las políticas de Swift ahora pueden derivar el BFV
  texto cifrado localmente a través de `encryptInput(...)` / `encryptedRequest(input:...)`.
- `ToriiIdentifierResolutionReceipt.verifySignature(using:)` valida que
  los campos de recibo de nivel superior coinciden con la carga útil firmada y verifica la
  Firma del solucionador en el lado del cliente antes del envío.
- `HttpClientTransport` en el SDK de Android ahora se expone
  `listIdentifierPolicies()`, `resolveIdentifier(policyId, entrada,
  encryptedInputHex)`, `issueIdentifierClaimReceipt(cuentaId, políticaId,
  entrada, cifradoInputHex)`, and `getIdentifierClaimByReceiptHash(...)`,
  más `IdentifierNormalization` para las mismas reglas de canonicalización.
- `IdentifierResolveRequest` y el
  `IdentifierPolicySummary.plaintextRequest(...)` /
  Los ayudantes `.encryptedRequest(...)` proporcionan la superficie de solicitud de Android escrita,
  mientras que `IdentifierPolicySummary.encryptInput(...)` /
  `.encryptedRequestFromInput(...)` deriva el sobre de texto cifrado BFV
  localmente a partir de los parámetros de política publicados.
  `IdentifierResolutionReceipt.verifySignature(policy)` verifica el retorno
  Firma de resolución del lado del cliente.

Conjunto de instrucciones actual:

- `RegisterIdentifierPolicy`
- `ActivateIdentifierPolicy`
- `ClaimIdentifier` (con recibo; se rechazan las reclamaciones sin procesar `opaque_id`)
- `RevokeIdentifier`

Ahora existen tres backends en `iroha_crypto::ram_lfe`:

- el PRF histórico vinculado al compromiso `HKDF-SHA3-512`, y
- un evaluador afín secreto respaldado por BFV que consume un identificador cifrado por BFV
  ranuras directamente. Cuando `iroha_crypto` se construye con el valor predeterminado
  Característica `bfv-accel`, la multiplicación del anillo BFV utiliza un determinista exacto
  backend CRT-NTT internamente; desactivar esa característica vuelve al
  ruta escalar del libro escolar con resultados idénticos, y
- un evaluador programado secreto respaldado por BFV que deriva una instrucción basada en
  Seguimiento de ejecución estilo RAM sobre registros cifrados y memoria de texto cifrado
  carriles antes de derivar el identificador opaco y el hash del recibo. lo programado
  El backend ahora requiere un piso de módulo BFV más fuerte que la ruta afín, y
  sus parámetros públicos se publican en un paquete canónico que incluye el
  Perfil de ejecución RAM-FHE consumido por billeteras y verificadores.

Aquí BFV significa el plan FHE Brakerski/Fan-Vercauteren implementado en
`crates/iroha_crypto/src/fhe_bfv.rs`. Es el mecanismo de ejecución cifrado.
utilizado por los backends afines y programados, no el nombre del oculto externo
abstracción de funciones.Torii utiliza el backend publicado por el compromiso de política. Cuando el backend de BFV
está activo, las solicitudes de texto sin formato se normalizan y luego se cifran en el lado del servidor antes
evaluación. Se evalúan las solicitudes BFV `encrypted_input` para el backend afín
directamente y ya debe estar normalizado en el lado del cliente; el backend programado
canonicaliza la entrada cifrada nuevamente en el BFV determinista del solucionador
sobre antes de ejecutar el programa secreto de RAM para que los hashes de recibo permanezcan
estable en textos cifrados semánticamente equivalentes.

## 2. Derivar y verificar UAID

Hay tres formas admitidas de obtener una UAID:

1. **Léalo desde el estado mundial o los modelos SDK.** Cualquiera `Account`/`AccountDetails`
   La carga útil consultada a través de Torii ahora tiene el campo `uaid` completado cuando el
   El participante optó por cuentas universales.
2. **Consultar los registros de la UAID.** Torii expone
   `GET /v1/space-directory/uaids/{uaid}` que devuelve los enlaces del espacio de datos
   y los metadatos manifiestos persisten en el host del directorio espacial (consulte
   `docs/space-directory.md` §3 para muestras de carga útil).
3. **Dérivalo de manera determinista.** Al iniciar nuevos UAID sin conexión, hash
   la semilla del participante canónico con Blake2b-256 y prefije el resultado con
   `uaid:`. El siguiente fragmento refleja la ayuda documentada en
   `docs/space-directory.md` §3.3:

   ```python
   import hashlib
   seed = b"participant@example"  # canonical address/domain seed
   digest = hashlib.blake2b(seed, digest_size=32).hexdigest()
   print(f"uaid:{digest}")
   ```Guarde siempre el literal en minúsculas y normalice los espacios en blanco antes del hash.
Ayudantes de CLI como `iroha app space-directory manifest scaffold` y Android
El analizador `UaidLiteral` aplica las mismas reglas de recorte para que las revisiones de gobernanza puedan
Verifique los valores sin secuencias de comandos ad hoc.

## 3. Inspección de tenencias y manifiestos de la UAID

El agregador de cartera determinista en `iroha_core::nexus::portfolio`
muestra cada par de activo/espacio de datos que hace referencia al UAID. Operadores y SDK
Puede consumir los datos a través de las siguientes superficies:

| Superficie | Uso |
|---------|-------|
| `GET /v1/accounts/{uaid}/portfolio` | Devuelve espacio de datos → activo → resúmenes de saldo; descrito en `docs/source/torii/portfolio_api.md`. |
| `GET /v1/space-directory/uaids/{uaid}` | Enumera los ID de espacio de datos + literales de cuenta vinculados al UAID. |
| `GET /v1/space-directory/uaids/{uaid}/manifests` | Proporciona el historial completo de `AssetPermissionManifest` para auditorías. |
| `iroha app space-directory bindings fetch --uaid <literal>` | Acceso directo de CLI que envuelve el punto final de enlaces y, opcionalmente, escribe el JSON en el disco (`--json-out`). |
| `iroha app space-directory manifest fetch --uaid <literal> --json-out <path>` | Obtiene el paquete JSON de manifiesto para los paquetes de pruebas. |

Ejemplo de sesión CLI (URL Torii configurada a través de `torii_api_url` en `iroha.json`):

```bash
iroha app space-directory bindings fetch \
  --uaid uaid:86e8ee39a3908460a0f4ee257bb25f340cd5b5de72735e9adefe07d5ef4bb0df \
  --json-out artifacts/uaid86/bindings.json

iroha app space-directory manifest fetch \
  --uaid uaid:86e8ee39a3908460a0f4ee257bb25f340cd5b5de72735e9adefe07d5ef4bb0df \
  --json-out artifacts/uaid86/manifests.json
```

Almacene las instantáneas JSON junto con el hash del manifiesto utilizado durante las revisiones; el
El observador de Space Directory reconstruye el mapa `uaid_dataspaces` cada vez que se manifiesta
activar, caducar o revocar, por lo que estas instantáneas son la forma más rápida de demostrar
qué vinculaciones estaban activas en una época determinada.

## 4. La capacidad editorial se manifiesta con evidencia

Utilice el flujo de CLI a continuación cada vez que se implemente una nueva asignación. Cada paso debe
terreno en el paquete de evidencia registrado para la aprobación de la gobernanza.

1. **Codifique el JSON del manifiesto** para que los revisores vean el hash determinista antes.
   presentación:

   ```bash
   iroha app space-directory manifest encode \
     --json fixtures/space_directory/capability/eu_regulator_audit.manifest.json \
     --out artifacts/eu_regulator_audit.manifest.to \
     --hash-out artifacts/eu_regulator_audit.manifest.hash
   ```

2. **Publique la asignación** utilizando la carga útil Norito (`--manifest`) o
   la descripción JSON (`--manifest-json`). Registre el recibo Torii/CLI más
   el hash de instrucción `PublishSpaceDirectoryManifest`:

   ```bash
   iroha app space-directory manifest publish \
     --manifest artifacts/eu_regulator_audit.manifest.to \
     --reason "ESMA wave 2 onboarding"
   ```

3. **Capture evidencia de SpaceDirectoryEvent.** Suscríbase a
   `SpaceDirectoryEvent::ManifestActivated` e incluir la carga útil del evento en
   el paquete para que los auditores puedan confirmar cuándo se realizó el cambio.

4. **Generar un paquete de auditoría** vinculando el manifiesto a su perfil de espacio de datos y
   ganchos de telemetría:

   ```bash
   iroha app space-directory manifest audit-bundle \
     --manifest artifacts/eu_regulator_audit.manifest.to \
     --profile fixtures/space_directory/profile/cbdc_lane_profile.json \
     --out-dir artifacts/eu_regulator_audit_bundle
   ```

5. **Verifique los enlaces a través de Torii** (`bindings fetch` y `manifests fetch`) y
   Archive esos archivos JSON con el paquete hash + anterior.

Lista de verificación de evidencia:

- [] Hash de manifiesto (`*.manifest.hash`) firmado por el aprobador del cambio.
- [] Recibo CLI/Torii de la llamada de publicación (salida estándar o artefacto `--json-out`).
- [] Carga útil `SpaceDirectoryEvent` que prueba la activación.
- [] Directorio del paquete de auditoría con perfil de espacio de datos, enlaces y copia del manifiesto.
- [] Enlaces + instantáneas de manifiesto obtenidas de Torii después de la activación.Esto refleja los requisitos de `docs/space-directory.md` §3.2 mientras proporciona SDK
propietarios una sola página a la que señalar durante las revisiones de lanzamiento.

## 5. Plantillas de manifiestos regionales/reguladores

Utilice los accesorios del repositorio como puntos de partida al crear manifiestos de capacidad
para reguladores o supervisores regionales. Demuestran cómo permitir/denegar el alcance
reglas y explicar las notas de política que esperan los revisores.

| Calendario | Propósito | Aspectos destacados |
|---------|---------|------------|
| `fixtures/space_directory/capability/eu_regulator_audit.manifest.json` | Feed de auditoría de ESMA/ESRB. | Asignaciones de solo lectura para `compliance.audit::{stream_reports, request_snapshot}` con denegaciones de ganancias en transferencias minoristas para mantener pasivos a los UAID reguladores. |
| `fixtures/space_directory/capability/jp_regulator_supervision.manifest.json` | Carril de supervisión de JFSA. | Agrega una asignación `cbdc.supervision.issue_stop_order` limitada (ventana por día + `max_amount`) y una denegación explícita en `force_liquidation` para aplicar controles duales. |

Al clonar estos aparatos, actualice:

1. Id. `uaid` e `dataspace` que coincidan con el participante y el carril que está habilitando.
2. Ventanas `activation_epoch`/`expiry_epoch` basadas en el cronograma de gobernanza.
3. Campos `notes` con las referencias de políticas del regulador (artículo MiCA, JFSA
   circulares, etcétera).
4. Ventanas de asignación (`PerSlot`, `PerMinute`, `PerDay`) y opcionales
   `max_amount` limita para que los SDK apliquen los mismos límites que el host.

## 6. Notas de migración para consumidores de SDKLas integraciones de SDK existentes que hacían referencia a ID de cuenta por dominio deben migrar a
las superficies centradas en la UAID descritas anteriormente. Utilice esta lista de verificación durante las actualizaciones:

  identificadores de cuenta. Para Rust/JS/Swift/Android esto significa actualizar a la última versión
  cajas de espacio de trabajo o regeneración de enlaces Norito.
- **Llamadas API:** Reemplace las consultas de cartera con ámbito de dominio con
  `GET /v1/accounts/{uaid}/portfolio` y los puntos finales de manifiesto/enlaces.
  `GET /v1/accounts/{uaid}/portfolio` acepta una consulta `asset_id` opcional
  parámetro cuando las billeteras solo necesitan una única instancia de activo. Ayudantes de clientes como
  como `ToriiClient.getUaidPortfolio` (JS) y Android
  `SpaceDirectoryClient` ya completa estas rutas; prefiérelos a los hechos a medida
  Código HTTP.
- **Almacenamiento en caché y telemetría:** Entradas en caché por UAID + espacio de datos en lugar de sin formato
  ID de cuenta y emite telemetría que muestra el literal UAID para que las operaciones puedan
  alinee los registros con la evidencia del Directorio Espacial.
- **Manejo de errores:** Los nuevos puntos finales devuelven los estrictos errores de análisis de UAID
  documentado en `docs/source/torii/portfolio_api.md`; sacar a la luz esos códigos
  palabra por palabra para que los equipos de soporte puedan clasificar los problemas sin pasos de reproducción.
- **Pruebas:** Conecte los dispositivos mencionados anteriormente (más sus propios manifiestos UAID)
  en conjuntos de pruebas de SDK para probar evaluaciones de manifiestos y viajes de ida y vuelta de Norito
  coincidir con la implementación del host.

## 7. Referencias- `docs/space-directory.md`: manual del operador con detalles más profundos del ciclo de vida.
- `docs/source/torii/portfolio_api.md` — Esquema REST para cartera UAID y
  puntos finales manifiestos.
- `crates/iroha_cli/src/space_directory.rs`: implementación de CLI a la que se hace referencia en
  esta guía.
- `fixtures/space_directory/capability/*.manifest.json` — regulador, minorista y
  Plantillas de manifiesto CBDC listas para clonación.
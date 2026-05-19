<!-- Auto-generated stub for Spanish (es) translation. Replace this content with the full translation. -->

---
lang: es
direction: ltr
source: docs/source/universal_accounts_guide.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 09a308ecbf07f0293add7f35cf4f1a50b5e6d3630b8b37a8f0f45a7cf82d3924
source_last_modified: "2026-03-30T18:22:55.987822+00:00"
translation_last_reviewed: 2026-04-02
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

Los UAID son ahora el ancla de una segunda capa de identidad:- Un `IdentifierPolicyId` (`<kind>#<business_rule>`) global define el
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

La división de nombres es intencional:- `ram_lfe` es la abstracción de función oculta externa. Cubre póliza
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

La implementación de la cuenta universal no cambia el modelo de identidad de la cuenta canónica:- `AccountId` sigue siendo el sujeto de cuenta canónico sin dominio.
- Los valores `AccountAlias` son enlaces SNS separados además de ese tema. un
  alias calificado de dominio como `merchant@banka.paynet` y un alias de raíz de espacio de datos
  como `merchant@paynet` pueden resolverse en el mismo `AccountId` canónico.
- El registro de cuenta canónica es siempre `Account::new(AccountId)` /
  `NewAccount::new(AccountId)`; no hay dominio calificado ni dominio materializado
  ruta de registro.
- Propiedad del dominio, permisos de alias y otros comportamientos relacionados con el dominio en vivo
  en su propio estado y API en lugar de en la identidad de la cuenta en sí.
- La búsqueda de cuentas públicas sigue esa división: las consultas de alias permanecen públicas, mientras que
  La identidad de la cuenta canónica sigue siendo pura `AccountId`.

Regla de implementación para operadores, SDK y pruebas: comenzar desde lo canónico
`AccountId`, luego agregue arrendamientos de alias, permisos de dominio/espacio de datos y cualquier
dominio propiedad del estado por separado. No sintetice una cuenta falsa derivada de un alias
o esperar cualquier campo de dominio vinculado en los registros de cuentas sólo porque un alias o
La ruta lleva un segmento de dominio.

Current Torii routes:

| Route | Purpose |
|-------|---------|
| `GET /v1/ram-lfe/program-policies` | Lists active and inactive RAM-LFE program policies plus their public execution metadata, including optional BFV `input_encryption` parameters and the programmed-backend `ram_fhe_profile`. |
| `POST /v1/ram-lfe/programs/{program_id}/execute` | Accepts `{ encrypted_input }` only and returns the stateless `RamLfeExecutionReceipt`, `{ output_ciphertext, output_hash, receipt_hash }`, and no plaintext output. The current Torii runtime issues receipts for the programmed BFV backend. |
| `POST /v1/ram-lfe/receipts/verify` | Statelessly validates a `RamLfeExecutionReceipt` against the published on-chain program policy and optionally checks that a caller-supplied encrypted `output_hex` matches the receipt `output_hash`. |
| `GET /v1/identifier-policies` | Lists active and inactive hidden-function policy namespaces plus their public metadata, including optional BFV `input_encryption` parameters, the required `normalization` mode for encrypted client-side input, and `ram_fhe_profile` for programmed BFV policies. |
| `POST /v1/accounts/{account_id}/identifiers/claim-receipt` | Accepts `{ policy_id, encrypted_input, output_opening }`. The BFV `encrypted_input` must already be normalized according to the published policy mode. The endpoint derives the `opaque:` handle from the verified external `RamLfeOutputOpening` and returns a signed receipt that `ClaimIdentifier` can submit on-chain. |
| `POST /v1/identifiers/resolve` | Accepts `{ policy_id, encrypted_input, output_opening }`. The endpoint re-evaluates the encrypted input, verifies the external output opening, derives the `opaque:` handle from the opened output hash, and returns a nested `{ payload, attestation }` receipt when an active claim exists. |
| `GET /v1/identifiers/receipts/{receipt_hash}` | Looks up the persisted `IdentifierClaimRecord` bound to a deterministic receipt hash so operators and SDKs can audit claim ownership or diagnose replay / mismatch failures without scanning the full identifier index. |

Torii's in-process execution runtime is configured under
`torii.ram_lfe.programs[*]`, keyed by `program_id`. The identifier routes now
reuse that same RAM-LFE runtime instead of a separate `identifier_resolver`
config surface.

Current SDK support:

- `normalizeIdentifierInput(value, normalization)` matches the Rust
  canonicalizers for `exact`, `lowercase_trimmed`, `phone_e164`,
  `email_address`, and `account_number`.
- `ToriiClient.listIdentifierPolicies()` lists policy metadata, including BFV
  input-encryption metadata when the policy publishes it, plus a decoded
  BFV parameter object via `input_encryption_public_parameters_decoded`.
  Programmed policies also expose the decoded `ram_fhe_profile`. That field is
  intentionally BFV-scoped: it lets wallets verify the expected register
  count, lane count, canonicalization mode, and minimum ciphertext modulus for
  the programmed FHE backend before encrypting client-side input.
- `getIdentifierBfvPublicParameters(policy)` and
  `buildIdentifierRequestForPolicy(policy, { encryptedInput | input,
  encrypt: true, outputOpening })` help JS callers consume published BFV
  metadata and build policy-aware encrypted request bodies without
  reimplementing policy-id and normalization rules.
- `encryptIdentifierInputForPolicy(policy, input, { seedHex? })` and
  `buildIdentifierRequestForPolicy(policy, { input, encrypt: true,
  outputOpening })` now let JS wallets construct the full BFV Norito
  ciphertext envelope locally from published policy parameters instead of
  shipping prebuilt ciphertext hex.
- `ToriiClient.resolveIdentifier({ policyId, encryptedInput, outputOpening })`
  resolves a hidden identifier and returns the signed nested
  `{ payload, attestation }` receipt.
- `ToriiClient.issueIdentifierClaimReceipt(accountId, { policyId,
  encryptedInput, outputOpening })` issues the signed receipt needed by
  `ClaimIdentifier`.
- `verifyIdentifierResolutionReceipt(receipt, policy)` verifies the returned
  receipt against the policy resolver key on the client side, and
  `ToriiClient.getIdentifierClaimByReceiptHash(receiptHash)` fetches the
  persisted claim record for later audit/debug flows.
- `IrohaSwift.ToriiClient` now exposes `listIdentifierPolicies()`,
  `resolveIdentifier(policyId:encryptedInputHex:outputOpening:)`,
  `issueIdentifierClaimReceipt(accountId:policyId:encryptedInputHex:outputOpening:)`,
  and `getIdentifierClaimByReceiptHash(_)`, plus
  `ToriiIdentifierNormalization` for the same phone/email/account-number
  canonicalization modes.
- `ToriiIdentifierLookupRequest` and encrypted request helpers provide the
  typed Swift request surface for resolve and claim-receipt calls, and Swift
  policies can now derive the BFV ciphertext locally via `encryptInput(...)`.
- `ToriiIdentifierResolutionReceipt.verifySignature(using:)` validates that
  the top-level receipt fields match the signed payload and verifies the
  resolver signature client-side before submission.
- `HttpClientTransport` in the Android SDK now exposes
  `listIdentifierPolicies()`, encrypted-only `resolveIdentifier(...)`,
  encrypted-only `issueIdentifierClaimReceipt(...)`, and
  `getIdentifierClaimByReceiptHash(...)`,
  plus `IdentifierNormalization` for the same canonicalization rules.
- `IdentifierResolveRequest` and encrypted request helpers provide the typed
  Android request surface, while `IdentifierPolicySummary.encryptInput(...)`
  derives the BFV ciphertext envelope locally from published policy
  parameters.
  `IdentifierResolutionReceipt.verifySignature(policy)` verifies the returned
  resolver signature client-side.

Current instruction set:

- `RegisterIdentifierPolicy`
- `ActivateIdentifierPolicy`
- `ClaimIdentifier` (receipt-bound; raw `opaque_id` claims are rejected)
- `RevokeIdentifier`

Three backends now exist in `iroha_crypto::ram_lfe`:

- the historical commitment-bound `HKDF-SHA3-512` PRF, and
- a BFV-backed secret affine evaluator that consumes BFV-encrypted identifier
  slots directly. When `iroha_crypto` is built with the default
  `bfv-accel` feature, BFV ring multiplication uses an exact deterministic
  CRT-NTT backend internally; disabling that feature falls back to the
  scalar schoolbook path with identical outputs, and
- a BFV-backed secret programmed evaluator that derives an instruction-driven
  RAM-style execution trace over encrypted registers and ciphertext memory
  lanes before deriving the opaque identifier and receipt hash. The programmed
  backend now requires a stronger BFV modulus floor than the affine path, and
  its public parameters are published in a canonical bundle that includes the
  RAM-FHE execution profile consumed by wallets and verifiers.

Here BFV means the Brakerski/Fan-Vercauteren FHE scheme implemented in
`crates/iroha_crypto/src/fhe_bfv.rs`. It is the encrypted-execution mechanism
used by the affine and programmed backends, not the name of the outer hidden
function abstraction.

Torii uses the backend published by the policy commitment. For the first
release, RAM-LFE and hidden-identifier routes are encrypted-only: Torii does
not accept plaintext inputs, does not hold BFV secret keys, and does not
decrypt input or output ciphertexts. Identifier claim and resolve requests must
include an externally signed `RamLfeOutputOpening`; the `opaque:` identifier is
derived from the verified opened-output hash, not from Torii-side plaintext or
from the ciphertext hash alone.

## 2. Derivar y verificar UAID

Hay tres formas admitidas de obtener una UAID:

1. **Léalo desde el estado mundial o los modelos SDK.** Cualquier `Account`/`AccountDetails`
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
   digest = bytearray(hashlib.blake2b(seed, digest_size=32).digest())
   digest[-1] |= 1
   print(f"uaid:{digest.hex()}")
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
qué vinculaciones estaban activas en una época determinada.## 4. La capacidad editorial se manifiesta con evidencia

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
| `fixtures/space_directory/capability/eu_regulator_audit.manifest.json` | Feed de auditoría de ESMA/ESRB. | Asignaciones de solo lectura para `compliance.audit::{stream_reports, request_snapshot}` con denegación de ganancias en transferencias minoristas para mantener pasivos a los UAID reguladores. |
| `fixtures/space_directory/capability/jp_regulator_supervision.manifest.json` | Carril de supervisión de JFSA. | Agrega una asignación limitada `cbdc.supervision.issue_stop_order` (ventana por día + `max_amount`) y una denegación explícita en `force_liquidation` para aplicar controles duales. |

Al clonar estos aparatos, actualice:

1. Id. `uaid` e `dataspace` que coincidan con el participante y el carril que está habilitando.
2. Ventanas `activation_epoch`/`expiry_epoch` basadas en el cronograma de gobernanza.
3. Campos `notes` con las referencias de políticas del regulador (artículo MiCA, JFSA
   circulares, etcétera).
4. Ventanas de asignación (`PerSlot`, `PerMinute`, `PerDay`) y opcionales
   `max_amount` tiene límites para que los SDK apliquen los mismos límites que el host.

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

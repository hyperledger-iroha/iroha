---
lang: es
direction: ltr
source: docs/source/data_model_and_isi_spec.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 55ac770cf80229c23d6067ef1ab312422c76fb928a08e8cad8c040bdab396016
source_last_modified: "2026-01-28T18:33:51.650362+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Modelo de datos Iroha v2 e ISI: especificación derivada de la implementación

Esta especificación se realiza mediante ingeniería inversa a partir de la implementación actual en `iroha_data_model` e `iroha_core` para ayudar en la revisión del diseño. Las rutas entre comillas invertidas apuntan al código autorizado.

## Alcance
- Define entidades canónicas (dominios, cuentas, activos, NFT, roles, permisos, pares, activadores) y sus identificadores.
- Describe instrucciones de cambio de estado (ISI): tipos, parámetros, condiciones previas, transiciones de estado, eventos emitidos y condiciones de error.
- Resume la gestión de parámetros, transacciones y serialización de instrucciones.

Determinismo: toda la semántica de instrucciones son transiciones de estado puro sin comportamiento dependiente del hardware. La serialización utiliza Norito; El código de bytes de la máquina virtual utiliza IVM y se valida en el lado del host antes de la ejecución en cadena.

---

## Entidades e Identificadores
Los ID tienen formas de cadena estables con `Display`/`FromStr` de ida y vuelta. Las reglas de nombres prohíben los espacios en blanco y los caracteres `@ # $` reservados.- `Name` — identificador textual validado. Reglas: `crates/iroha_data_model/src/name.rs`.
- `DomainId` — `name`. Dominio: `{ id, logo, metadata, owned_by }`. Constructores: `NewDomain`. Código: `crates/iroha_data_model/src/domain.rs`.
- `AccountId`: las direcciones canónicas se producen a través de `AccountAddress` (I105 / `i105` comprimido/hexadecimal) y Torii normaliza las entradas a través de `AccountAddress::parse_encoded`. I105 es el formato de cuenta preferido; el formulario `i105` es el segundo mejor para UX exclusivo de Sora. La conocida cadena `alias` (rejected legacy form) se conserva únicamente como alias de enrutamiento. Cuenta: `{ id, metadata }`. Código: `crates/iroha_data_model/src/account.rs`.
- Política de admisión de cuentas: los dominios controlan la creación implícita de cuentas almacenando un Norito-JSON `AccountAdmissionPolicy` en la clave de metadatos `iroha:account_admission_policy`. Cuando la clave está ausente, el parámetro personalizado a nivel de cadena `iroha:default_account_admission_policy` proporciona el valor predeterminado; cuando eso también está ausente, el valor predeterminado estricto es `ImplicitReceive` (primera versión). Las etiquetas de política `mode` (`ExplicitOnly` o `ImplicitReceive`) más límites de creación opcionales por transacción (predeterminado `16`) y por bloque, un `implicit_creation_fee` opcional (cuenta de grabación o hundimiento), `min_initial_amounts` por definición de activo y un `default_role_on_create` opcional (otorgado después de `AccountCreated`, rechaza con `DefaultRoleError` si falta). Génesis no puede optar por participar; Las políticas deshabilitadas/no válidas rechazan instrucciones de estilo recibo para cuentas desconocidas con `InstructionExecutionError::AccountAdmission`. Las cuentas implícitas sellan los metadatos `iroha:created_via="implicit"` antes de `AccountCreated`; Los roles predeterminados emiten un seguimiento `AccountRoleGranted`, y las reglas de referencia del propietario del ejecutor permiten que la nueva cuenta gaste sus propios activos/NFT sin roles adicionales. Código: `crates/iroha_data_model/src/account/admission.rs`, `crates/iroha_core/src/smartcontracts/isi/account_admission.rs`.
- `AssetDefinitionId` — `asset#domain`. Definición: `{ id, spec: NumericSpec, mintable: Mintable, logo, metadata, owned_by, total_quantity }`. Código: `crates/iroha_data_model/src/asset/definition.rs`.
- `AssetId`: canonical encoded literal `norito:<hex>` (legacy textual forms are not supported in first release).
- `NftId` — `nft$domain`. NFT: `{ id, content: Metadata, owned_by }`. Código: `crates/iroha_data_model/src/nft.rs`.
- `RoleId` — `name`. Rol: `{ id, permissions: BTreeSet<Permission> }` con el constructor `NewRole { inner: Role, grant_to }`. Código: `crates/iroha_data_model/src/role.rs`.
- `Permission` — `{ name: Ident, payload: Json }`. Código: `crates/iroha_data_model/src/permission.rs`.
- `PeerId`/`Peer`: identidad del par (clave pública) y dirección. Código: `crates/iroha_data_model/src/peer.rs`.
- `TriggerId` — `name`. Activador: `{ id, action }`. Acción: `{ executable, repeats, authority, filter, metadata }`. Código: `crates/iroha_data_model/src/trigger/`.
- `Metadata` — `BTreeMap<Name, Json>` con inserción/extracción marcada. Código: `crates/iroha_data_model/src/metadata.rs`.
- Patrón de suscripción (capa de aplicación): los planes son entradas `AssetDefinition` con metadatos `subscription_plan`; las suscripciones son registros `Nft` con metadatos `subscription`; la facturación se ejecuta mediante activadores de tiempo que hacen referencia a NFT de suscripción. Consulte `docs/source/subscriptions_api.md` y `crates/iroha_data_model/src/subscription.rs`.
- **Primitivas criptográficas** (característica `sm`):- `Sm2PublicKey` / `Sm2Signature` reflejan el punto SEC1 canónico + codificación `r∥s` de ancho fijo para SM2. Los constructores imponen la pertenencia a la curva y la semántica de identificación distintiva (`DEFAULT_DISTID`), mientras que la verificación rechaza escalares mal formados o de alto rango. Código: `crates/iroha_crypto/src/sm.rs` y `crates/iroha_data_model/src/crypto/mod.rs`.
  - `Sm3Hash` expone el resumen GM/T 0004 como un nuevo tipo `[u8; 32]` serializable por Norito que se utiliza siempre que aparecen hashes en manifiestos o telemetría. Código: `crates/iroha_data_model/src/crypto/hash.rs`.
  - `Sm4Key` representa claves SM4 de 128 bits y se comparte entre llamadas al sistema host y dispositivos de modelo de datos. Código: `crates/iroha_data_model/src/crypto/symmetric.rs`.
  Estos tipos se encuentran junto a las primitivas Ed25519/BLS/ML-DSA existentes y están disponibles para los consumidores de modelos de datos (Torii, SDK, herramientas de génesis) una vez que la función `sm` está habilitada.

Rasgos importantes: `Identifiable`, `Registered`/`Registrable` (patrón de construcción), `HasMetadata`, `IntoKeyValue`. Código: `crates/iroha_data_model/src/lib.rs`.

Eventos: Cada entidad tiene eventos emitidos en caso de mutaciones (crear/eliminar/cambiar propietario/cambiar metadatos, etc.). Código: `crates/iroha_data_model/src/events/`.

---

## Parámetros (Configuración de cadena)
- Familias: `SumeragiParameters { block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }`, `BlockParameters { max_transactions }`, `TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes }`, `SmartContractParameters { fuel, memory, execution_depth }`, más `custom: BTreeMap`.
- Enumeraciones individuales para diferencias: `SumeragiParameter`, `BlockParameter`, `TransactionParameter`, `SmartContractParameter`. Agregador: `Parameters`. Código: `crates/iroha_data_model/src/parameter/system.rs`.

Parámetros de configuración (ISI): `SetParameter(Parameter)` actualiza el campo correspondiente y emite `ConfigurationEvent::Changed`. Código: `crates/iroha_data_model/src/isi/transparent.rs`, ejecutor en `crates/iroha_core/src/smartcontracts/isi/world.rs`.

---

## Serialización y registro de instrucciones
- Rasgo principal: `Instruction: Send + Sync + 'static` con `dyn_encode()`, `as_any()`, estable `id()` (el valor predeterminado es el nombre de tipo concreto).
- `InstructionBox`: Envoltorio `Box<dyn Instruction>`. Clon/Eq/Ord operan en `(type_id, encoded_bytes)`, por lo que la igualdad es por valor.
- Norito serde para `InstructionBox` se serializa como `(String wire_id, Vec<u8> payload)` (vuelve a `type_name` si no hay ID de cable). La deserialización utiliza identificadores de mapeo globales `InstructionRegistry` para constructores. El registro predeterminado incluye todos los ISI integrados. Código: `crates/iroha_data_model/src/isi/{mod.rs,registry.rs}`.

---

## ISI: tipos, semántica, errores
La ejecución se implementa a través de `Execute for <Instruction>` en `iroha_core::smartcontracts::isi`. A continuación se enumeran los efectos públicos, las condiciones previas, los eventos emitidos y los errores.

### Registrarse / Darse de baja
Tipos: `Register<T: Registered>` e `Unregister<T: Identifiable>`, con tipos de suma `RegisterBox`/`UnregisterBox` que cubren objetivos concretos.

- Registrar pares: se inserta en el conjunto de pares mundiales.
  - Condiciones previas: no debe existir ya.
  - Eventos: `PeerEvent::Added`.
  - Errores: `Repetition(Register, PeerId)` si está duplicado; `FindError` en búsquedas. Código: `core/.../isi/world.rs`.

- Registrar dominio: compila desde `NewDomain` con `owned_by = authority`. No permitido: dominio `genesis`.
  - Condiciones previas: inexistencia del dominio; no `genesis`.
  - Eventos: `DomainEvent::Created`.
  - Errores: `Repetition(Register, DomainId)`, `InvariantViolation("Not allowed to register genesis domain")`. Código: `core/.../isi/world.rs`.- Registrar cuenta: compilaciones desde `NewAccount`, no permitidas en el dominio `genesis`; La cuenta `genesis` no se puede registrar.
  - Condiciones previas: el dominio debe existir; inexistencia de cuenta; no en el dominio de génesis.
  - Eventos: `DomainEvent::Account(AccountEvent::Created)`.
  - Errores: `Repetition(Register, AccountId)`, `InvariantViolation("Not allowed to register account in genesis domain")`. Código: `core/.../isi/domain.rs`.

- Registrar AssetDefinition: compila desde el constructor; establece `owned_by = authority`.
  - Condiciones previas: inexistencia de definición; dominio existe.
  - Eventos: `DomainEvent::AssetDefinition(AssetDefinitionEvent::Created)`.
  - Errores: `Repetition(Register, AssetDefinitionId)`. Código: `core/.../isi/domain.rs`.

- Registrar NFT: compilaciones del constructor; establece `owned_by = authority`.
  - Condiciones previas: inexistencia de NFT; dominio existe.
  - Eventos: `DomainEvent::Nft(NftEvent::Created)`.
  - Errores: `Repetition(Register, NftId)`. Código: `core/.../isi/nft.rs`.

- Registrar función: se compila a partir de `NewRole { inner, grant_to }` (primer propietario registrado mediante asignación de funciones de cuenta), almacena `inner: Role`.
  - Condiciones previas: inexistencia de roles.
  - Eventos: `RoleEvent::Created`.
  - Errores: `Repetition(Register, RoleId)`. Código: `core/.../isi/world.rs`.

- Registrar disparador: almacena el disparador en el disparador apropiado establecido por tipo de filtro.
  - Condiciones previas: si el filtro no se puede minar, `action.repeats` debe ser `Exactly(1)` (de lo contrario, `MathError::Overflow`). Se prohíben identificaciones duplicadas.
  - Eventos: `TriggerEvent::Created(TriggerId)`.
  - Errores: `Repetition(Register, TriggerId)`, `InvalidParameterError::SmartContract(..)` en fallas de conversión/validación. Código: `core/.../isi/triggers/mod.rs`.

- Anular el registro de pares/dominio/cuenta/definición de activos/NFT/rol/disparador: elimina el objetivo; emite eventos de eliminación. Eliminaciones en cascada adicionales:
  - Anular registro de dominio: elimina todas las cuentas del dominio, sus funciones, permisos, contadores de secuencia de transmisión, etiquetas de cuenta y enlaces UAID; elimina sus activos (y metadatos por activo); elimina todas las definiciones de activos en el dominio; elimina las NFT en el dominio y cualquier NFT propiedad de las cuentas eliminadas; elimina los activadores cuyo dominio de autoridad coincide. Eventos: `DomainEvent::Deleted`, más eventos de eliminación por elemento. Errores: `FindError::Domain` si falta. Código: `core/.../isi/world.rs`.
  - Anular registro de cuenta: elimina los permisos, las funciones, el contador de secuencia de transmisión, la asignación de etiquetas de cuenta y los enlaces UAID de la cuenta; elimina los activos propiedad de la cuenta (y los metadatos por activo); elimina los NFT propiedad de la cuenta; elimina los activadores cuya autoridad es esa cuenta. Eventos: `AccountEvent::Deleted`, más `NftEvent::Deleted` por NFT eliminado. Errores: `FindError::Account` si falta. Código: `core/.../isi/domain.rs`.
  - Anular el registro de AssetDefinition: elimina todos los activos de esa definición y sus metadatos por activo. Eventos: `AssetDefinitionEvent::Deleted` e `AssetEvent::Deleted` por activo. Errores: `FindError::AssetDefinition`. Código: `core/.../isi/domain.rs`.
  - Dar de baja NFT: elimina NFT. Eventos: `NftEvent::Deleted`. Errores: `FindError::Nft`. Código: `core/.../isi/nft.rs`.
  - Anular registro de rol: primero revoca el rol de todas las cuentas; luego elimina el rol. Eventos: `RoleEvent::Deleted`. Errores: `FindError::Role`. Código: `core/.../isi/world.rs`.
  - Anular registro del activador: elimina el activador si está presente; La cancelación del registro duplicada produce `Repetition(Unregister, TriggerId)`. Eventos: `TriggerEvent::Deleted`. Código: `core/.../isi/triggers/mod.rs`.

### Menta / Quemar
Tipos: `Mint<O, D: Identifiable>` e `Burn<O, D: Identifiable>`, en cajas como `MintBox`/`BurnBox`.- Activo (Numérico) mint/burn: ajusta saldos y definiciones `total_quantity`.
  - Condiciones previas: el valor `Numeric` debe satisfacer `AssetDefinition.spec()`; perfecto permitido por `mintable`:
    - `Infinitely`: siempre permitido.
    - `Once`: permitido exactamente una vez; la primera casa de moneda convierte `mintable` a `Not` y emite `AssetDefinitionEvent::MintabilityChanged`, además de un `AssetDefinitionEvent::MintabilityChangedDetailed { asset_definition, minted_amount, authority }` detallado para auditabilidad.
    - `Limited(n)`: permite operaciones adicionales de mint `n`. Cada menta exitosa disminuye el contador; cuando llega a cero, la definición cambia a `Not` y emite los mismos eventos `MintabilityChanged` que los anteriores.
    - `Not`: error `MintabilityError::MintUnmintable`.
  - Cambios de estado: crea activo si falta en menta; elimina la entrada de activos si el saldo llega a cero al quemarse.
  - Eventos: `AssetEvent::Added`/`AssetEvent::Removed`, `AssetDefinitionEvent::MintabilityChanged` (cuando `Once` o `Limited(n)` agota su asignación).
  - Errores: `TypeError::AssetNumericSpec(Mismatch)`, `MathError::Overflow`/`NotEnoughQuantity`. Código: `core/.../isi/asset.rs`.

- Repeticiones del disparador mint/burn: cambia el recuento de `action.repeats` para un disparador.
  - Condiciones previas: en perfecto estado, el filtro debe ser minable; la aritmética no debe desbordarse ni desbordarse.
  - Eventos: `TriggerEvent::Extended`/`TriggerEvent::Shortened`.
  - Errores: `MathError::Overflow` en menta no válida; `FindError::Trigger` si falta. Código: `core/.../isi/triggers/mod.rs`.

### Transferencia
Tipos: `Transfer<S: Identifiable, O, D: Identifiable>`, en caja como `TransferBox`.

- Activo (Numérico): restar del origen `AssetId`, agregar al destino `AssetId` (misma definición, cuenta diferente). Eliminar activo de origen puesto a cero.
  - Condiciones previas: el activo fuente existe; El valor satisface `spec`.
  - Eventos: `AssetEvent::Removed` (origen), `AssetEvent::Added` (destino).
  - Errores: `FindError::Asset`, `TypeError::AssetNumericSpec`, `MathError::NotEnoughQuantity/Overflow`. Código: `core/.../isi/asset.rs`.

- Propiedad del dominio: cambia `Domain.owned_by` a la cuenta de destino.
  - Condiciones previas: ambas cuentas existen; dominio existe.
  - Eventos: `DomainEvent::OwnerChanged`.
  - Errores: `FindError::Account/Domain`. Código: `core/.../isi/domain.rs`.

- Propiedad de AssetDefinition: cambia `AssetDefinition.owned_by` a la cuenta de destino.
  - Condiciones previas: ambas cuentas existen; la definición existe; la fuente debe poseerlo actualmente.
  - Eventos: `AssetDefinitionEvent::OwnerChanged`.
  - Errores: `FindError::Account/AssetDefinition`. Código: `core/.../isi/account.rs`.

- Propiedad de NFT: cambia `Nft.owned_by` a la cuenta de destino.
  - Condiciones previas: ambas cuentas existen; NFT existe; la fuente debe poseerlo actualmente.
  - Eventos: `NftEvent::OwnerChanged`.
  - Errores: `FindError::Account/Nft`, `InvariantViolation` si la fuente no es propietaria del NFT. Código: `core/.../isi/nft.rs`.

### Metadatos: establecer/eliminar valor-clave
Tipos: `SetKeyValue<T>` y `RemoveKeyValue<T>` con `T ∈ { Domain, Account, AssetDefinition, Nft, Trigger }`. Se proporcionan enumeraciones en caja.

- Conjunto: inserta o reemplaza `Metadata[key] = Json(value)`.
- Quitar: elimina la llave; error si falta.
- Eventos: `<Target>Event::MetadataInserted` / `MetadataRemoved` con los valores antiguos/nuevos.
- Errores: `FindError::<Target>` si el objetivo no existe; `FindError::MetadataKey` sobre falta una clave para su extracción. Código: `crates/iroha_data_model/src/isi/transparent.rs` e implicaciones de ejecutor por objetivo.### Permisos y Roles: Conceder/Revocar
Tipos: `Grant<O, D>` e `Revoke<O, D>`, con enumeraciones en cuadros para `Permission`/`Role` hacia/desde `Account`, y `Permission` hacia/desde `Role`.

- Otorgar permiso a la cuenta: agrega `Permission` a menos que ya sea inherente. Eventos: `AccountEvent::PermissionAdded`. Errores: `Repetition(Grant, Permission)` si está duplicado. Código: `core/.../isi/account.rs`.
- Revocar permiso de cuenta: se elimina si está presente. Eventos: `AccountEvent::PermissionRemoved`. Errores: `FindError::Permission` si está ausente. Código: `core/.../isi/account.rs`.
- Otorgar función a la cuenta: inserta la asignación `(account, role)` si está ausente. Eventos: `AccountEvent::RoleGranted`. Errores: `Repetition(Grant, RoleId)`. Código: `core/.../isi/account.rs`.
- Revocar rol de cuenta: elimina la asignación si está presente. Eventos: `AccountEvent::RoleRevoked`. Errores: `FindError::Role` si está ausente. Código: `core/.../isi/account.rs`.
- Otorgar permiso al rol: reconstruye el rol con el permiso agregado. Eventos: `RoleEvent::PermissionAdded`. Errores: `Repetition(Grant, Permission)`. Código: `core/.../isi/world.rs`.
- Revocar permiso del rol: reconstruye el rol sin ese permiso. Eventos: `RoleEvent::PermissionRemoved`. Errores: `FindError::Permission` si está ausente. Código: `core/.../isi/world.rs`.

### Activadores: Ejecutar
Tipo: `ExecuteTrigger { trigger: TriggerId, args: Json }`.
- Comportamiento: pone en cola un `ExecuteTriggerEvent { trigger_id, authority, args }` para el subsistema de activación. La ejecución manual solo se permite para activadores de llamada (filtro `ExecuteTrigger`); el filtro debe coincidir y la persona que llama debe ser la autoridad de acción desencadenante o tener `CanExecuteTrigger` para esa autoridad. Cuando un ejecutor proporcionado por el usuario está activo, el ejecutor en tiempo de ejecución valida la ejecución del desencadenador y consume el presupuesto de combustible del ejecutor de la transacción (base `executor.fuel` más metadatos opcionales `additional_fuel`).
- Errores: `FindError::Trigger` si no está registrado; `InvariantViolation` si lo llama una persona que no es autoridad. Código: `core/.../isi/triggers/mod.rs` (y pruebas en `core/.../smartcontracts/isi/mod.rs`).

### Actualizar e iniciar sesión
- `Upgrade { executor }`: migra el ejecutor utilizando el código de bytes `Executor` proporcionado, actualiza el ejecutor y su modelo de datos, emite `ExecutorEvent::Upgraded`. Errores: empaquetado como `InvalidParameterError::SmartContract` en caso de error de migración. Código: `core/.../isi/world.rs`.
- `Log { level, msg }`: emite un registro de nodo con el nivel dado; sin cambios de estado. Código: `core/.../isi/world.rs`.

### Modelo de error
Sobre común: `InstructionExecutionError` con variantes para errores de evaluación, errores de consulta, conversiones, entidad no encontrada, repetición, acuñabilidad, matemáticas, parámetro no válido y violación de invariante. Las enumeraciones y los ayudantes se encuentran en `crates/iroha_data_model/src/isi/mod.rs` en `pub mod error`.

---## Transacciones y ejecutables
- `Executable`: ya sea `Instructions(ConstVec<InstructionBox>)` o `Ivm(IvmBytecode)`; El código de bytes se serializa como base64. Código: `crates/iroha_data_model/src/transaction/executable.rs`.
- `TransactionBuilder`/`SignedTransaction`: construye, firma y empaqueta un ejecutable con metadatos, `chain_id`, `authority`, `creation_time_ms`, `ttl_ms` opcional e `nonce`. Código: `crates/iroha_data_model/src/transaction/`.
- En tiempo de ejecución, `iroha_core` ejecuta lotes `InstructionBox` a través de `Execute for InstructionBox`, transmitiendo al `*Box` apropiado o a una instrucción concreta. Código: `crates/iroha_core/src/smartcontracts/isi/mod.rs`.
- Presupuesto de validación del ejecutor en tiempo de ejecución (ejecutor proporcionado por el usuario): base `executor.fuel` de los parámetros más metadatos de transacción opcionales `additional_fuel` (`u64`), compartido entre validaciones de instrucciones/activadores dentro de la transacción.

---

## Invariantes y Notas (de pruebas y guardias)
- Protecciones de Génesis: no se puede registrar el dominio `genesis` o cuentas en el dominio `genesis`; La cuenta `genesis` no se puede registrar. Código/pruebas: `core/.../isi/world.rs`, `core/.../smartcontracts/isi/mod.rs`.
- Los activos numéricos deben satisfacer su `NumericSpec` en menta/transferencia/quema; La falta de coincidencia de especificaciones produce `TypeError::AssetNumericSpec`.
- Mintabilidad: `Once` permite una sola menta y luego cambia a `Not`; `Limited(n)` permite exactamente mentas `n` antes de pasar a `Not`. Los intentos de prohibir la acuñación en `Infinitely` causan `MintabilityError::ForbidMintOnMintable`, y la configuración de `Limited(0)` produce `MintabilityError::InvalidMintabilityTokens`.
- Las operaciones de metadatos son clave exacta; eliminar una clave inexistente es un error.
- Los filtros de activación pueden no ser minables; entonces `Register<Trigger>` solo permite repeticiones `Exactly(1)`.
- Activar la ejecución de puertas de la clave de metadatos `__enabled` (bool); los valores predeterminados que faltan son activados y los activadores deshabilitados se omiten en las rutas de datos/hora/por llamada.
- Determinismo: toda aritmética utiliza operaciones comprobadas; under/overflow devuelve errores matemáticos escritos; los saldos cero eliminan las entradas de activos (sin estado oculto).

---## Ejemplos prácticos
- Acuñación y transferencia:
  - `Mint::asset_numeric(10, asset_id)` → agrega 10 si lo permiten las especificaciones/capacidad de acuñación; eventos: `AssetEvent::Added`.
  - `Transfer::asset_numeric(asset_id, 5, to_account)` → mueve 5; eventos para eliminación/adición.
- Actualizaciones de metadatos:
  - `SetKeyValue::account(account_id, "avatar".parse()?, json)` → insertar; eliminación a través de `RemoveKeyValue::account(...)`.
- Gestión de roles/permisos:
  - `Grant::account_role(role_id, account)`, `Grant::role_permission(perm, role)` y sus homólogos `Revoke`.
- Ciclo de vida del disparador:
  - `Register::trigger(Trigger::new(id, Action::new(exec, repeats, authority, filter)))` con control de minabilidad implícito por filtro; `ExecuteTrigger::new(id).with_args(&args)` debe coincidir con la autoridad configurada.
  - Los activadores se pueden desactivar configurando la clave de metadatos `__enabled` en `false` (faltan los valores predeterminados habilitados); alternar a través de `SetKeyValue::trigger` o la llamada al sistema IVM `set_trigger_enabled`.
  - El almacenamiento de activadores se repara durante la carga: se eliminan los identificadores duplicados, los identificadores no coincidentes y los activadores que hacen referencia a códigos de bytes faltantes; Se vuelven a calcular los recuentos de referencias de códigos de bytes.
  - Si falta el código de bytes IVM de un activador en el momento de la ejecución, el activador se elimina y la ejecución se trata como no operativa con un resultado de error.
  - Los desencadenantes agotados se eliminan inmediatamente; si se encuentra una entrada agotada durante la ejecución, se elimina y se trata como faltante.
- Actualización de parámetros:
  - `SetParameter(SumeragiParameter::BlockTimeMs(2500).into())` actualiza y emite `ConfigurationEvent::Changed`.

---

## Trazabilidad (fuentes seleccionadas)
 - Núcleo del modelo de datos: `crates/iroha_data_model/src/{account.rs,domain.rs,asset/**,nft.rs,role.rs,permission.rs,metadata.rs,trigger/**,parameter/**}`.
 - Definiciones y registro ISI: `crates/iroha_data_model/src/isi/{mod.rs,register.rs,transfer.rs,mint_burn.rs,transparent.rs,registry.rs}`.
 - Ejecución ISI: `crates/iroha_core/src/smartcontracts/isi/{mod.rs,world.rs,domain.rs,account.rs,asset.rs,nft.rs,triggers/**}`.
 - Eventos: `crates/iroha_data_model/src/events/**`.
 - Transacciones: `crates/iroha_data_model/src/transaction/**`.

Si desea que esta especificación se expanda a una API representada/tabla de comportamiento o se vincule a cada evento/error concreto, diga la palabra y la ampliaré.
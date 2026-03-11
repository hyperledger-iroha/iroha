---
lang: es
direction: ltr
source: docs/source/data_model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3c110536e456d6582c2dd2bd72a71fef25e3f43f7f369b3f1c0ce802564f0dbd
source_last_modified: "2026-01-28T18:33:51.649272+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Modelo de datos Iroha v2: análisis profundo

Este documento explica las estructuras, identificadores, rasgos y protocolos que forman el modelo de datos Iroha v2, tal como se implementa en la caja `iroha_data_model` y se utiliza en todo el espacio de trabajo. Está destinado a ser una referencia precisa que usted puede revisar y proponer actualizaciones.

## Alcance y Fundamentos

- Propósito: proporcionar tipos canónicos para objetos de dominio (dominios, cuentas, activos, NFT, roles, permisos, pares), instrucciones de cambio de estado (ISI), consultas, activadores, transacciones, bloques y parámetros.
- Serialización: todos los tipos públicos derivan códecs Norito (`norito::codec::{Encode, Decode}`) y esquema (`iroha_schema::IntoSchema`). JSON se utiliza de forma selectiva (por ejemplo, para cargas útiles HTTP e `Json`) detrás de los indicadores de funciones.
- Nota de IVM: Ciertas validaciones de tiempo de deserialización están deshabilitadas cuando se dirigen a la máquina virtual Iroha (IVM), ya que el host realiza la validación antes de invocar contratos (consulte los documentos de caja en `src/lib.rs`).
- Puertas FFI: algunos tipos están anotados condicionalmente para FFI a través de `iroha_ffi` detrás de `ffi_export`/`ffi_import` para evitar gastos generales cuando no se necesita FFI.

## Rasgos fundamentales y ayudantes

- `Identifiable`: Las entidades tienen un `Id` y un `fn id(&self) -> &Self::Id` estables. Debe derivarse con `IdEqOrdHash` para compatibilidad con mapas/conjuntos.
- `Registrable`/`Registered`: muchas entidades (por ejemplo, `Domain`, `AssetDefinition`, `Role`) utilizan un patrón de creación. `Registered` vincula el tipo de tiempo de ejecución a un tipo de constructor liviano (`With`) adecuado para transacciones de registro.
- `HasMetadata`: Acceso unificado a un mapa clave/valor `Metadata`.
- `IntoKeyValue`: asistente de división de almacenamiento para almacenar `Key` (ID) e `Value` (datos) por separado para reducir la duplicación.
- `Owned<T>`/`Ref<'world, K, V>`: Envoltorios ligeros utilizados en almacenamientos y filtros de consulta para evitar copias innecesarias.

## Nombres e identificadores

- `Name`: Identificador textual válido. No permite espacios en blanco ni caracteres reservados `@`, `#`, `$` (utilizados en ID compuestos). Construible vía `FromStr` con validación. Los nombres se normalizan a Unicode NFC en el análisis (las ortografías canónicamente equivalentes se tratan como idénticas y se almacenan compuestas). El nombre especial `genesis` está reservado (marcado sin distinguir entre mayúsculas y minúsculas).
- `IdBox`: Un sobre tipo suma para cualquier ID admitido (`DomainId`, `AccountId`, `AssetDefinitionId`, `AssetId`, `NftId`, `PeerId`, `TriggerId`, `RoleId`, `Permission`, `CustomParameterId`). Útil para flujos genéricos y codificación Norito como un solo tipo.
- `ChainId`: Identificador de cadena opaco utilizado para protección de reproducción en transacciones.Formas de cadena de ID (ida y vuelta con `Display`/`FromStr`):
- `DomainId`: `name` (por ejemplo, `wonderland`).
- `AccountId`: identificador canónico codificado a través de `AccountAddress`, que expone I105, Sora comprimido (`i105`) y códecs hexadecimales canónicos (`AccountAddress::to_i105`, `to_i105`, `canonical_hex`, `parse_encoded`). I105 es el formato de cuenta preferido; el formulario `i105` es el segundo mejor para UX exclusivo de Sora. El alias de enrutamiento amigable para los humanos `alias` (rejected legacy form) se conserva para UX, pero ya no se trata como el identificador autorizado. Torii normaliza las cadenas entrantes a través de `AccountAddress::parse_encoded`. Los ID de cuenta admiten controladores de clave única y multifirma.
- `AssetDefinitionId`: `asset#domain` (por ejemplo, `xor#soramitsu`).
- `AssetId`: canonical encoded literal `norito:<hex>` (legacy textual forms are not supported in first release).
- `NftId`: `nft$domain` (por ejemplo, `rose$garden`).
- `PeerId`: `public_key` (la igualdad entre pares es por clave pública).

## Entidades

### Dominio
- `DomainId { name: Name }` – nombre único.
- `Domain { id, logo: Option<IpfsPath>, metadata: Metadata, owned_by: AccountId }`.
- Constructor: `NewDomain` con `with_logo`, `with_metadata`, luego `Registrable::build(authority)` establece `owned_by`.

### Cuenta
- `AccountId { domain: DomainId, controller: AccountController }` (controlador = clave única o política multifirma).
- `Account { id, metadata, label?, uaid? }`: `label` es un alias estable opcional utilizado por los registros de nueva clave, `uaid` lleva el [ID de cuenta universal] Nexus opcional (./universal_accounts_guide.md).
- Constructor: `NewAccount` vía `Account::new(id)`; `HasMetadata` tanto para el constructor como para la entidad.

### Definiciones de activos y activos
- `AssetDefinitionId { domain: DomainId, name: Name }`.
- `AssetDefinition { id, spec: NumericSpec, mintable: Mintable, logo: Option<IpfsPath>, metadata, owned_by: AccountId, total_quantity: Numeric }`.
  - `Mintable`: `Infinitely` | `Once` | `Limited(u32)` | `Not`.
  - Constructores: `AssetDefinition::new(id, spec)` o conveniencia `numeric(id)`; configuradores para `metadata`, `mintable`, `owned_by`.
- `AssetId { account: AccountId, definition: AssetDefinitionId }`.
- `Asset { id, value: Numeric }` con `AssetEntry`/`AssetValue` fácil de almacenar.
- `AssetTotalQuantityMap = BTreeMap<AssetDefinitionId, Numeric>` expuesto para API de resumen.

### NFT
- `NftId { domain: DomainId, name: Name }`.
- `Nft { id, content: Metadata, owned_by: AccountId }` (el contenido son metadatos de clave/valor arbitrarios).
- Constructor: `NewNft` vía `Nft::new(id, content)`.

### Roles y permisos
- `RoleId { name: Name }`.
- `Role { id, permissions: BTreeSet<Permission> }` con constructor `NewRole { inner: Role, grant_to: AccountId }`.
- `Permission { name: Ident, payload: Json }`: el `name` y el esquema de carga útil deben alinearse con el `ExecutorDataModel` activo (ver a continuación).

### Compañeros
- `PeerId { public_key: PublicKey }`.
- `Peer { address: SocketAddr, id: PeerId }` y forma de cadena analizable `public_key@address`.### Primitivas criptográficas (característica `sm`)
- `Sm2PublicKey` e `Sm2Signature`: puntos compatibles con SEC1 y firmas `r∥s` de ancho fijo para SM2. Los constructores validan la pertenencia a la curva y las identificaciones distintivas; La codificación Norito refleja la representación canónica utilizada por `iroha_crypto`.
- `Sm3Hash`: `[u8; 32]` nuevo tipo que representa el resumen GM/T 0004, utilizado en manifiestos, telemetría y respuestas de llamadas al sistema.
- `Sm4Key`: contenedor de claves simétricas de 128 bits compartido entre llamadas al sistema del host y dispositivos del modelo de datos.
Estos tipos se ubican junto a las primitivas Ed25519/BLS/ML-DSA existentes y pasan a formar parte del esquema público una vez que el espacio de trabajo se construye con `--features sm`.

### Desencadenantes y eventos
- `TriggerId { name: Name }` y `Trigger { id, action: action::Action }`.
- `action::Action { executable: Executable, repeats: Repeats, authority: AccountId, filter: EventFilterBox, metadata }`.
  - `Repeats`: `Indefinitely` o `Exactly(u32)`; Servicios de ordenamiento y agotamiento incluidos.
  - Seguridad: `TriggerCompleted` no se puede utilizar como filtro de acción (validado durante (des)serialización).
- `EventBox`: tipo de suma para eventos de canalización, lote de canalización, datos, tiempo, activación de ejecución y activación completada; `EventFilterBox` refleja eso para suscripciones y filtros de activación.

## Parámetros y configuración

- Familias de parámetros del sistema (todas `Default`ed, captadores de acarreo y conversión a enumeraciones individuales):
- `SumeragiParameters { block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }`.
  - `BlockParameters { max_transactions: NonZeroU64 }`.
  - `TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes }`.
  -`SmartContractParameters { fuel, memory, execution_depth }`.
- `Parameters` agrupa todas las familias y un `custom: BTreeMap<CustomParameterId, CustomParameter>`.
- Enumeraciones de un solo parámetro: `SumeragiParameter`, `BlockParameter`, `TransactionParameter`, `SmartContractParameter` para actualizaciones e iteraciones similares a diferencias.
- Parámetros personalizados: definidos por el ejecutor, transportados como `Json`, identificados por `CustomParameterId` (un `Name`).

## ISI (Iroha Instrucciones especiales)

- Rasgo principal: `Instruction` con `dyn_encode`, `as_any` y un identificador estable por tipo `id()` (el valor predeterminado es el nombre del tipo concreto). Todas las instrucciones son `Send + Sync + 'static`.
- `InstructionBox`: contenedor `Box<dyn Instruction>` de propiedad con clon/eq/ord implementado mediante ID de tipo + bytes codificados.
- Las familias de instrucción integrada se organizan en:
  - `mint_burn`, `transfer`, `register` y un paquete de ayudantes `transparent`.
  - Escriba enumeraciones para metaflujos: `InstructionType`, sumas en cuadros como `SetKeyValueBox` (dominio/cuenta/asset_def/nft/trigger).
- Errores: modelo de errores enriquecido bajo `isi::error` (errores de tipo de evaluación, errores de búsqueda, acuñabilidad, matemáticas, parámetros no válidos, repetición, invariantes).
- Registro de instrucciones: la macro `instruction_registry!{ ... }` crea un registro de decodificación en tiempo de ejecución codificado por nombre de tipo. Utilizado por el clon `InstructionBox` y el serde Norito para lograr una (des)serialización dinámica. Si no se ha configurado explícitamente ningún registro a través de `set_instruction_registry(...)`, en el primer uso se instala de forma diferida un registro predeterminado integrado con todos los ISI principales para mantener los archivos binarios sólidos.

## Transacciones- `Executable`: ya sea `Instructions(ConstVec<InstructionBox>)` o `Ivm(IvmBytecode)`. `IvmBytecode` se serializa como base64 (nuevo tipo transparente sobre `Vec<u8>`).
- `TransactionBuilder`: construye una carga útil de transacción con `chain`, `authority`, `creation_time_ms`, `time_to_live_ms` opcional y `nonce`, `metadata` y un `Executable`.
  - Ayudantes: `with_instructions`, `with_bytecode`, `with_executable`, `with_metadata`, `set_nonce`, `set_ttl`, `set_creation_time`, `sign`.
- `SignedTransaction` (versionado con `iroha_version`): lleva `TransactionSignature` y carga útil; proporciona hash y verificación de firma.
- Puntos de entrada y resultados:
  - `TransactionEntrypoint`: `External(SignedTransaction)` | `Time(TimeTriggerEntrypoint)`.
  - `TransactionResult` = `Result<DataTriggerSequence, TransactionRejectionReason>` con ayudas de hash.
  - `ExecutionStep(ConstVec<InstructionBox>)`: un único lote ordenado de instrucciones en una transacción.

## Bloques

- `SignedBlock` (versionado) encapsula:
  - `signatures: BTreeSet<BlockSignature>` (de validadores),
  - `payload: BlockPayload { header: BlockHeader, transactions: Vec<SignedTransaction> }`,
  - `result: BlockResult` (estado de ejecución secundario) que contiene `time_triggers`, árboles Merkle de entrada/resultado, `transaction_results` e `fastpq_transcripts: BTreeMap<Hash, Vec<TransferTranscript>>`.
- Utilidades: `presigned`, `set_transaction_results(...)`, `set_transaction_results_with_transcripts(...)`, `header()`, `signatures()`, `hash()`, `add_signature`, `replace_signatures`.
- Raíces de Merkle: los puntos de entrada de las transacciones y los resultados se confirman a través de árboles de Merkle; resultado La raíz de Merkle se coloca en el encabezado del bloque.
- Las pruebas de inclusión de bloques (`BlockProofs`) exponen tanto las pruebas Merkle de entrada/resultado como el mapa `fastpq_transcripts` para que los probadores fuera de la cadena puedan recuperar los deltas de transferencia asociados con un hash de transacción.
- Los mensajes `ExecWitness` (transmitidos a través de Torii y respaldados por chismes de consenso) ahora incluyen tanto `fastpq_transcripts` como `fastpq_batches: Vec<FastpqTransitionBatch>` listo para probar con `public_inputs` integrado (dsid, slot, root, perm_root, tx_set_hash). por lo que los probadores externos pueden ingerir filas FASTPQ canónicas sin volver a codificar las transcripciones.

## Consultas

- Dos sabores:
  - Singular: implementar `SingularQuery<Output>` (por ejemplo, `FindParameters`, `FindExecutorDataModel`).
  - Iterable: implemente `Query<Item>` (por ejemplo, `FindAccounts`, `FindAssets`, `FindDomains`, etc.).
- Formularios borrados:
  - `QueryBox<T>` es un `Query<Item = T>` en caja y borrado con un serde Norito respaldado por un registro global.
  - `QueryWithFilter<T> { query, predicate, selector }` empareja una consulta con un predicado/selector DSL; se convierte en una consulta iterable borrada a través de `From`.
- Registro y códecs:
  - `query_registry!{ ... }` crea un registro global que asigna tipos de consulta concretos a constructores por nombre de tipo para decodificación dinámica.
  - `QueryRequest = Singular(SingularQueryBox) | Start(QueryWithParams) | Continue(ForwardCursor)` y `QueryResponse = Singular(..) | Iterable(QueryOutput)`.
  - `QueryOutputBatchBox` es un tipo de suma sobre vectores homogéneos (por ejemplo, `Vec<Account>`, `Vec<Name>`, `Vec<AssetDefinition>`, `Vec<BlockHeader>`), además de ayudantes de tupla y extensión para una paginación eficiente.
- DSL: Implementado en `query::dsl` con rasgos de proyección (`HasProjection<PredicateMarker>` / `SelectorMarker`) para predicados y selectores verificados en tiempo de compilación. Una característica `fast_dsl` expone una variante más ligera si es necesario.

## Ejecutor y Extensibilidad- `Executor { bytecode: IvmBytecode }`: el paquete de código ejecutado por el validador.
- `ExecutorDataModel { parameters: CustomParameters, instructions: BTreeSet<Ident>, permissions: BTreeSet<Ident>, schema: Json }` declara el dominio definido por el ejecutor:
  - Parámetros de configuración personalizados,
  - Identificadores de instrucciones personalizados,
  - Identificadores de tokens de permiso,
  - Un esquema JSON que describe tipos personalizados para herramientas del cliente.
- Existen ejemplos de personalización en `data_model/samples/executor_custom_data_model` que demuestran:
  - Token de permiso personalizado a través de derivación `iroha_executor_data_model::permission::Permission`,
  - Parámetro personalizado definido como un tipo convertible a `CustomParameter`,
  - Instrucciones personalizadas serializadas en `CustomInstruction` para su ejecución.

### Instrucción personalizada (ISI definida por el ejecutor)

- Tipo: `isi::CustomInstruction { payload: Json }` con identificación de cable estable `"iroha.custom"`.
- Propósito: sobre para instrucciones específicas del ejecutor en redes privadas/consorcios o para creación de prototipos, sin bifurcar el modelo de datos públicos.
- Comportamiento predeterminado del ejecutor: el ejecutor integrado en `iroha_core` no ejecuta `CustomInstruction` y entrará en pánico si se encuentra. Un ejecutor personalizado debe reducir `InstructionBox` a `CustomInstruction` e interpretar de manera determinista la carga útil en todos los validadores.
- Norito: codifica/decodifica vía `norito::codec::{Encode, Decode}` con esquema incluido; la carga útil `Json` se serializa de forma determinista. Los viajes de ida y vuelta son estables siempre que el registro de instrucciones incluya `CustomInstruction` (es parte del registro predeterminado).
- IVM: Kotodama se compila en el código de bytes IVM (`.to`) y es la ruta recomendada para la lógica de la aplicación. Utilice únicamente `CustomInstruction` para extensiones de nivel de ejecutor que aún no se pueden expresar en Kotodama. Garantice el determinismo y binarios ejecutores idénticos entre pares.
- No para redes públicas: no lo use para cadenas públicas donde los ejecutores heterogéneos corren el riesgo de bifurcaciones de consenso. Prefiera proponer un nuevo ISI integrado cuando necesite funciones de plataforma.

## Metadatos

- `Metadata(BTreeMap<Name, Json>)`: almacén de clave/valor adjunto a múltiples entidades (`Domain`, `Account`, `AssetDefinition`, `Nft`, activadores y transacciones).
- API: `contains`, `iter`, `get`, `insert` y (con `transparent_api`) `remove`.

## Características y determinismo

- Funciones de control de API opcionales (`std`, `json`, `transparent_api`, `ffi_export`, `ffi_import`, `fast_dsl`, `http`, `fault_injection`).
- Determinismo: toda la serialización utiliza la codificación Norito para ser portátil en todo el hardware. El código de bytes IVM es un blob de bytes opaco; la ejecución no debe introducir reducciones no deterministas. El host valida las transacciones y proporciona entradas a IVM de forma determinista.

### API transparente (`transparent_api`)- Propósito: expone el acceso completo y mutable a las estructuras/enumeraciones `#[model]` para componentes internos como Torii, ejecutores y pruebas de integración. Sin él, esos elementos son intencionalmente opacos, por lo que los SDK externos solo ven constructores seguros y cargas útiles codificadas.
- Mecánica: la macro `iroha_data_model_derive::model` reescribe cada campo público con `#[cfg(feature = "transparent_api")] pub` y guarda una copia privada para la compilación predeterminada. Al habilitar la función se invierten esos cfgs, por lo que la desestructuración de `Account`, `Domain`, `Asset`, etc. se vuelve legal fuera de sus módulos de definición.
- Detección de superficie: la caja exporta una constante `TRANSPARENT_API: bool` (generada en `transparent_api.rs` o `non_transparent_api.rs`). El código descendente puede verificar esta bandera y bifurcarse cuando necesita recurrir a ayudantes opacos.
- Habilitación: agregue `features = ["transparent_api"]` a la dependencia en `Cargo.toml`. Las cajas de espacio de trabajo que necesitan la proyección JSON (por ejemplo, `iroha_torii`) reenvían la bandera automáticamente, pero los consumidores externos deben mantenerla apagada a menos que controlen la implementación y acepten la superficie API más amplia.

## Ejemplos rápidos

Cree un dominio y una cuenta, defina un activo y cree una transacción con instrucciones:

```rust
use iroha_data_model::prelude::*;
use iroha_crypto::KeyPair;
use iroha_primitives::numeric::Numeric;

// Domain
let domain_id: DomainId = "wonderland".parse().unwrap();
let new_domain = Domain::new(domain_id.clone()).with_metadata(Metadata::default());

// Account
let kp = KeyPair::random();
let account_id = AccountId::new(domain_id.clone(), kp.public_key().clone());
let new_account = Account::new(account_id.clone()).with_metadata(Metadata::default());

// Asset definition and an asset for the account
let asset_def_id: AssetDefinitionId = "xor#wonderland".parse().unwrap();
let new_asset_def = AssetDefinition::numeric(asset_def_id.clone())
    .with_metadata(Metadata::default());
let asset_id = AssetId::new(asset_def_id.clone(), account_id.clone());
let asset = Asset::new(asset_id.clone(), Numeric::from(100));

// Build a transaction with instructions (pseudo-ISI; exact ISI types live under `isi`)
let chain_id: ChainId = "dev-chain".parse().unwrap();
let tx = TransactionBuilder::new(chain_id, account_id.clone())
    .with_instructions(vec![ /* Register/ Mint/ Transfer instructions here */ ])
    .sign(kp.private_key());
```

Consulta de cuentas y activos con el DSL:

```rust
use iroha_data_model::prelude::*;

let predicate = query::dsl::CompoundPredicate::build(|p| {
    p.equals("metadata.tier", 1_u32)
        .exists("metadata.display_name")
});
let selector = query::dsl::SelectorTuple::default();
let q: QueryBox<QueryOutputBatchBox> =
    QueryWithFilter::new(
        Box::new(query::account::FindAccounts),
        predicate,
        selector,
    ).into();
// Encode and send via Torii; decode on server using the query registry
```

Utilice el código de bytes de contrato inteligente IVM:

```rust
use iroha_data_model::prelude::*;

let bytecode = IvmBytecode::from_compiled(include_bytes!("contract.to").to_vec());
let tx = TransactionBuilder::new("dev-chain".parse().unwrap(), account_id.clone())
    .with_bytecode(bytecode)
    .sign(kp.private_key());
```

## Versionado

- `SignedTransaction`, `SignedBlock` e `SignedQuery` son estructuras canónicas codificadas con Norito. Cada uno implementa `iroha_version::Version` para prefijar su carga útil con la versión ABI actual (actualmente `1`) cuando se codifica mediante `EncodeVersioned`.

## Notas de revisión/Posibles actualizaciones

- Consulta DSL: considere documentar un subconjunto estable de cara al usuario y ejemplos de filtros/selectores comunes.
- Familias de instrucciones: amplíe los documentos públicos que enumeran las variantes ISI integradas expuestas por `mint_burn`, `register`, `transfer`.

---
Si alguna parte necesita más profundidad (por ejemplo, catálogo ISI completo, lista completa de registro de consultas o campos de encabezado de bloque), hágamelo saber y ampliaré esas secciones en consecuencia.
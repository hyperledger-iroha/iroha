---
lang: es
direction: ltr
source: docs/source/ivm_isi_kotodama_alignment.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3f40329b9968530dea38745b49f7fee4d55aeb461e515e6f97b5b5986cb27e3f
source_last_modified: "2026-01-21T10:20:35.513444+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# IVM ⇄ ISI ⇄ Modelo de datos ⇄ Kotodama — Revisión de alineación

Este documento audita cómo el conjunto de instrucciones de la máquina virtual Iroha (IVM) y la superficie de llamada al sistema se asignan a las instrucciones especiales (ISI) Iroha y a `iroha_data_model`, y cómo Kotodama se compila en esa pila. Identifica las lagunas actuales y propone mejoras concretas para que las cuatro capas encajen de forma determinista y ergonómica.

Nota sobre el destino del código de bytes: los contratos inteligentes Kotodama se compilan en el código de bytes de la máquina virtual Iroha (IVM) (`.to`). No apuntan a “risc5”/RISC-V como una arquitectura independiente. Cualquier codificación tipo RISC-V a la que se haga referencia aquí es parte del formato de instrucción mixta de IVM y sigue siendo un detalle de implementación.

## Alcance y fuentes
- IVM: `crates/ivm/src/{instruction.rs,ivm.rs,syscalls.rs,host.rs,mock_wsv.rs}` y `crates/ivm/docs/*`.
- ISI/Modelo de datos: `crates/iroha_data_model/src/isi/*`, `crates/iroha_core/src/smartcontracts/isi/*` y documentos `docs/source/data_model_and_isi_spec.md`.
- Kotodama: `crates/kotodama_lang/src/*`, documentos en `crates/ivm/docs/*`.
- Integración central: `crates/iroha_core/src/{state.rs,executor.rs,smartcontracts/ivm/cache.rs}`.

Terminología
- "ISI" se refiere a tipos de instrucciones integradas que mutan el estado mundial a través del ejecutor (por ejemplo, RegisterAccount, Mint, Transfer).
- “Syscall” se refiere a IVM `SCALL` con un número de 8 bits que delega al host las operaciones del libro mayor.

---

## Mapeo actual (tal como se implementó)

### IVM Instrucciones
- Los ayudantes de aritmética, memoria, flujo de control, criptografía, vector y ZK se definen en `instruction.rs` y se implementan en `ivm.rs`. Estos son autónomos y deterministas; Las rutas de aceleración (SIMD/Metal/CUDA) tienen respaldos de CPU.
- El límite del sistema/host se realiza a través de `SCALL` (código de operación 0x60). Los números se enumeran en `syscalls.rs` e incluyen operaciones mundiales (registro/anulación de registro de dominio/cuenta/activo, acuñación/quema/transferencia, operaciones de rol/permiso, activadores) además de ayudantes (`GET_PRIVATE_INPUT`, `COMMIT_OUTPUT`, `GET_MERKLE_PATH`, etc.).

### Capa de host
- El rasgo `IVMHost::syscall(number, &mut IVM)` vive en `host.rs`.
- DefaultHost implementa solo ayudantes que no son de contabilidad (asignación, crecimiento de montón, entradas/salidas, ayudantes de prueba ZK, descubrimiento de características); NO realiza mutaciones de estado mundial.
- Existe una demostración `WsvHost` en `mock_wsv.rs` que asigna un subconjunto de operaciones de activos (Transferencia/Mint/Burn) a un pequeño WSV en memoria utilizando `AccountId`/`AssetDefinitionId` a través de asignaciones ad hoc de enteros→ID en registros x10..x13.

### ISI y modelo de datos
- Los tipos y la semántica de ISI integrados se implementan en `iroha_core::smartcontracts::isi::*` y se documentan en `docs/source/data_model_and_isi_spec.md`.
- `InstructionBox` utiliza un registro con “ID de cable” estables y codificación Norito; El envío de ejecución nativa es la ruta del código actual en el núcleo.### Integración principal de IVM
- `State::execute_trigger(..)` clona el `IVM` almacenado en caché, adjunta un `CoreHost::with_accounts_and_args` y luego llama a `load_program` + `run`.
- `CoreHost` implementa `IVMHost`: las llamadas al sistema con estado se decodifican mediante el diseño TLV de puntero-ABI, se asignan a ISI integrado (`InstructionBox`) y se ponen en cola. Una vez que la VM regresa, el host entrega esos ISI al ejecutor habitual para que los permisos, invariantes, eventos y telemetría sigan siendo idénticos a la ejecución nativa. Las llamadas al sistema auxiliares que no tocan WSV aún delegan en `DefaultHost`.
- `executor.rs` continúa ejecutando ISI integrado de forma nativa; La migración del ejecutor del validador a IVM sigue siendo un trabajo futuro.

### Kotodama → IVM
- Existen piezas de interfaz (lexer/parser/minimal semantics/IR/regalloc).
- Codegen (`kotodama::compiler`) emite un subconjunto de operaciones IVM y utiliza `SCALL` para operaciones de activos:
  - `MintAsset` → establecer x10=cuenta, x11=activo, x12=&NoritoBytes(Numérico); `SCALL SYSCALL_MINT_ASSET`.
  - `BurnAsset`/`TransferAsset` similar (cantidad pasada como puntero NoritoBytes (numérico)).
- Las demostraciones `koto_*_demo.rs` muestran el uso de `WsvHost` con índices enteros asignados a ID para realizar pruebas rápidas.

---

## Brechas y desajustes

1) Cobertura y paridad del host central
- Estado: `CoreHost` ahora existe en el núcleo y traduce muchas llamadas al sistema del libro mayor a ISI que se ejecutan a través de la ruta estándar. La cobertura aún está incompleta (por ejemplo, algunas llamadas al sistema de roles/permisos/activadores son códigos auxiliares) y se requieren pruebas de paridad para garantizar que el ISI en cola produzca el mismo estado/eventos que la ejecución nativa.

2) Superficie de Syscall frente a denominación y cobertura del modelo de datos/ISI
- NFT: las llamadas al sistema ahora exponen nombres canónicos `SYSCALL_NFT_*` alineados con `iroha_data_model::nft`.
- Roles/Permisos/Disparadores: existe una lista de llamadas al sistema, pero no hay una implementación de referencia o una tabla de mapeo que vincule cada llamada a un ISI concreto en el núcleo.
- Parámetros/semántica: algunas llamadas al sistema no especifican la codificación de parámetros (ID escritas versus punteros) o la semántica de gas; La semántica de ISI está bien definida.

3) ABI para pasar datos escritos a través del límite de VM/host
- Los TLV de Pointer‑ABI ahora se decodifican en `CoreHost` (`decode_tlv_typed`), lo que proporciona una ruta determinista para ID, metadatos y cargas JSON. Queda trabajo para garantizar que cada llamada al sistema documente los tipos de puntero esperados y que Kotodama emita los TLV correctos (incluido el manejo de errores cuando la política rechaza un tipo).

4) Coherencia en el mapeo de errores y gases
- Los códigos de operación IVM cargan el gas por operación; CoreHost ahora devuelve gas adicional para las llamadas al sistema ISI utilizando el programa de gas nativo (incluidas las transferencias por lotes y el puente ISI del proveedor), y ZK verifica que las llamadas al sistema reutilicen el programa de gas confidencial. DefaultHost aún mantiene costos mínimos para la cobertura de prueba.
- Las superficies de error difieren: IVM devuelve `VMError::{OutOfGas,PermissionDenied,...}`; ISI devuelve las categorías `InstructionExecutionError` (`Find`, `Repetition`, `InvariantViolation`, `Math`, `Type`, `Mintability`, `InvalidParameter`).5) Determinismo en las trayectorias de aceleración
- IVM vector/CUDA/Metal tienen respaldos de CPU, pero algunas operaciones permanecen reservadas (`SETVL`, PARBEGIN/PAREND) y aún no forman parte del núcleo determinista.
- Los árboles Merkle difieren entre IVM y el nodo (`ivm::merkle_tree` vs `iroha_crypto::MerkleTree`): ya aparece un elemento de unificación en `roadmap.md`.

6) Superficie del lenguaje Kotodama frente a la semántica del libro mayor prevista
- El compilador emite un pequeño subconjunto; la mayoría de las características del lenguaje (estado/estructuras, activadores, permisos, parámetros escritos/devoluciones) aún no están conectadas al modelo host/ISI.
- No hay tipificación de capacidad/efecto para garantizar que las llamadas al sistema sean legales para la autoridad.

---

## Recomendaciones (pasos concretos)

### A. Implementar un host IVM de producción en el núcleo
- Agregar el módulo `iroha_core::smartcontracts::ivm::host` implementando `ivm::host::IVMHost`.
- Para cada llamada al sistema en `ivm::syscalls`:
  - Decodificar argumentos a través de una ABI canónica (ver B.), construir el ISI integrado correspondiente o llamar directamente a la misma lógica central, ejecutarlo contra `StateTransaction` y asignar errores de manera determinista a un código de retorno IVM.
  - Cargue el gas de forma determinista utilizando una tabla por llamada al sistema definida en el núcleo (y expuesta a IVM a través de `SYSCALL_GET_PARAMETER` si es necesario en el futuro). Inicialmente, devuelva el gas adicional fijo del anfitrión para cada llamada.
- Inserte `authority: &AccountId` e `&mut StateTransaction` en el host para que las comprobaciones de permisos y los eventos sean idénticos a los del ISI nativo.
- Actualice `State::execute_trigger(ExecutableRef::Ivm)` para conectar este host antes que `vm.run()` y devolver la misma semántica `ExecutionStep` que ISI (los eventos ya se emiten en el núcleo; se debe validar el comportamiento consistente).

### B. Definir una ABI determinista de VM/host para valores escritos
- Utilice Norito en el lado de la VM para argumentos estructurados:
  - Pasar punteros (en x10..x13, etc.) a regiones de memoria de VM que contienen valores codificados con Norito para tipos como `AccountId`, `AssetDefinitionId`, `Numeric`, `Metadata`.
  - El host lee bytes a través de los ayudantes de memoria `IVM` y los decodifica con Norito (`iroha_data_model` ya deriva `Encode/Decode`).
- Agregue ayudas mínimas en el codegen Kotodama para serializar ID literales en grupos de códigos/constantes o para preparar marcos de llamadas en la memoria.
- Las cantidades son `Numeric` y se pasan como punteros de NoritoBytes; otros tipos complejos también pasan por puntero.
- Documente esto en `crates/ivm/docs/calling_convention.md` y agregue ejemplos.### C. Alinear el nombre y la cobertura de las llamadas al sistema con ISI/modelo de datos
- Cambie el nombre de las llamadas al sistema relacionadas con NFT para mayor claridad: los nombres canónicos ahora siguen el patrón `SYSCALL_NFT_*` (`SYSCALL_NFT_MINT_ASSET`, `SYSCALL_NFT_SET_METADATA`, etc.).
- Publicar una tabla de mapeo (doc + comentarios de código) de cada llamada al sistema a la semántica central de ISI, que incluye:
  - Parámetros (registros frente a punteros), condiciones previas esperadas, eventos y asignaciones de errores.
  - Cargos de gas.
- Asegúrese de que haya una llamada al sistema para cada ISI integrado que pueda invocarse desde Kotodama (dominios, cuentas, activos, roles/permisos, activadores, parámetros). Si un ISI debe permanecer privilegiado, documentarlo y aplicarlo mediante comprobaciones de permisos en el host.

### D. Unificar errores y gas
- Agregue una capa de traducción en el host: asigne `InstructionExecutionError::{Find,Repetition,InvariantViolation,Math,Type,Mintability,InvalidParameter}` a códigos `VMError` específicos o una convención de resultados extendida (por ejemplo, configure `x10=0/1` y use un `VMError::HostRejected { code }` bien definido).
- Introducir una mesa de gas en el núcleo para llamadas al sistema; duplicarlo en los documentos IVM; asegúrese de que los costos sean predecibles en función del tamaño de los insumos e independientes de la plataforma.

### E. Determinismo y primitivas compartidas
- Completar la unificación del árbol Merkle (ver hoja de ruta) y eliminar/alias `ivm::merkle_tree` a `iroha_crypto` con hojas y pruebas idénticas.
- Mantenga reservado `SETVL`/PARBEGIN/PAREND` hasta que se implementen comprobaciones de determinismo de extremo a extremo y una estrategia de planificación determinista; documento que IVM ignora estas sugerencias hoy.
- Garantizar que las rutas de aceleración produzcan resultados idénticos byte por byte; cuando no sea posible, proteger las funciones con una prueba que garantice la equivalencia de respaldo de la CPU.

### F. Cableado del compilador Kotodama
- Ampliar codegen al ABI canónico (B.) para ID y parámetros complejos; deje de usar mapas de demostración de números enteros → ID.
- Agregue asignaciones integradas directamente a las llamadas al sistema ISI más allá de los activos (dominios/cuentas/roles/permisos/activadores) con nombres claros.
- Agregar comprobaciones de capacidad en tiempo de compilación y anotaciones `permission(...)` opcionales; recurrir a errores del host en tiempo de ejecución cuando la prueba estática no es posible.
- Agregue pruebas unitarias en `crates/ivm/tests/kotodama.rs` que compilan y ejecutan pequeños contratos de extremo a extremo utilizando un host de prueba que decodifica argumentos Norito y muta un WSV temporal.

### G. Documentación y ergonomía del desarrollador.
- Actualice `docs/source/data_model_and_isi_spec.md` con la tabla de mapeo de llamadas al sistema y notas ABI.
- Agregue un nuevo documento "Guía de integración de host IVM" en `crates/ivm/docs/` que describe cómo implementar un `IVMHost` sobre un `StateTransaction` real.
- Aclarar en `README.md` y crear documentos que Kotodama apunta al código de bytes IVM `.to` y que las llamadas al sistema son el puente hacia el estado mundial.

---

## Tabla de mapeo sugerida (borrador inicial)

Subconjunto representativo: finalizar y ampliar durante la implementación del host.- SYSCALL_REGISTER_DOMAIN(id: ptr DomainId) → Registro ISI
- SYSCALL_REGISTER_ACCOUNT(id: ptr AccountId) → Registro ISI
- SYSCALL_REGISTER_ASSET(id: ptr AssetDefinitionId, mintable: u8) → Registro ISI
- SYSCALL_MINT_ASSET(cuenta: ptr AccountId, activo: ptr AssetDefinitionId, monto: ptr NoritoBytes(Numérico)) → ISI Mint
- SYSCALL_BURN_ASSET(cuenta: ptr AccountId, activo: ptr AssetDefinitionId, cantidad: ptr NoritoBytes(Numérico)) → ISI Burn
- SYSCALL_TRANSFER_ASSET(de: ptr AccountId, a: ptr AccountId, activo: ptr AssetDefinitionId, cantidad: ptr NoritoBytes(Numérico)) → Transferencia ISI
- SYSCALL_TRANSFER_V1_BATCH_BEGIN() / SYSCALL_TRANSFER_V1_BATCH_END() → ISI TransferAssetBatch (abrir/cerrar el alcance; las entradas individuales se reducen a través de `transfer_asset`)
- SYSCALL_TRANSFER_V1_BATCH_APPLY(&NoritoBytes) → Enviar un lote precodificado cuando los contratos ya serializaron las entradas fuera de la cadena
- SYSCALL_NFT_MINT_ASSET(id: ptr NftId, propietario: ptr AccountId) → Registro ISI
- SYSCALL_NFT_TRANSFER_ASSET(de: ptr AccountId, a: ptr AccountId, id: ptr NftId) → Transferencia ISI
- SYSCALL_NFT_SET_METADATA(id: ptr NftId, contenido: ptr Metadatos) → ISI SetKeyValue
- SYSCALL_NFT_BURN_ASSET(id: ptr NftId) → ISI Unregister
- SYSCALL_CREATE_ROLE(id: ptr RoleId, rol: ptr Role) → Registro ISI
- SYSCALL_GRANT_ROLE(cuenta: ptr AccountId, rol: ptr RoleId) → ISI Grant
- SYSCALL_REVOKE_ROLE(cuenta: ptr AccountId, rol: ptr RoleId) → ISI Revoke
- SYSCALL_SET_PARAMETER(param: ptr Parámetro) → ISI SetParameter

Notas
- “ptr T” significa un puntero en un registro a bytes codificados con Norito para T, almacenados en la memoria de la VM; el host lo decodifica en el tipo `iroha_data_model` correspondiente.
- Convención de retorno: conjuntos exitosos `x10=1`; La falla establece `x10=0` y puede generar `VMError::HostRejected` para errores fatales.

---

## Riesgos y plan de implementación
- Comience cableando el host para un conjunto limitado (Activos + Cuentas) y agregue pruebas enfocadas.
- Mantener la ejecución nativa de ISI como ruta autorizada mientras madura la semántica del host; ejecute ambas rutas en un "modo sombra" en las pruebas para afirmar efectos y eventos finales idénticos.
- Una vez validada la paridad, habilite el host IVM para los activadores IVM en producción; Más adelante considere enrutar también las transacciones regulares a través de IVM.

---

## Trabajo excepcional
- Finalice los ayudantes Kotodama que pasan punteros codificados con Norito (`crates/ivm/src/kotodama_std.rs`) y expóngalos a través de la CLI del compilador.
- Publicar la tabla de gas de syscall (incluidas las llamadas al sistema auxiliares) y mantener la aplicación/pruebas de CoreHost alineadas con ella.
- ✅ Se agregaron accesorios Norito de ida y vuelta que cubren el argumento de puntero ABI; consulte `crates/iroha_data_model/tests/norito_pointer_abi_roundtrip.rs` para conocer la cobertura del puntero NFT y del manifiesto mantenida en CI.